//! Render the full Photocraft UI offscreen (no window, no focus stealing) and save a PNG.
//!
//! ```sh
//! cargo run --release -p photocraft-ui-egui --example snapshot -- \
//!     --out ui.png --size 1440x900 --scale 2 --open photo.jpg \
//!     --script '[["ui.set", {"tool": "type"}], ["ui.menu.invoke", {"id": "image.imageSize"}]]'
//! ```
//!
//! `--system-theme light|dark|none` supplies a deterministic OS appearance for theme captures.
//!
//! `--safe-gpu` draws the canvas on the CPU path, like the app's `--safe-gpu` launch.
//! `--wayland-notice` previews the native file drag-and-drop guidance shown in Wayland sessions.
//! `--custom-titlebar` draws the Windows/Linux title bar (caption buttons in the top bar).
//!
//! `--monitor 1366x768 --window-top 31` simulates the display the window is on (in points) and
//! where its content starts on it, e.g. a window running under a Windows taskbar.
//!
//! `--script` is a JSON array of `[method, params]` control-protocol calls (see
//! docs/control-protocol.md), applied in order with a few frames between them.
//! `--right-click-at X,Y` opens a screen-space context menu after the script, including panel
//! and document-tab menus that are outside the document-coordinate control pointer.
//! `--click-at X,Y` opens a screen-space menu (for example the top Select menu) after the script.

use photocraft_ui_egui::control::{ControlRequest, Outcome, handle};
use photocraft_ui_egui::{PhotocraftApp, Services};
use serde_json::Value;

/// Visual acceptance fixture: deliberately has no HTTP client or AI model.
struct MockAi(Vec<photocraft_engine::prefs::ComfyUiModel>);
impl photocraft_engine::ai_cmds::AiBackend for MockAi {
    fn available_models(&self, _: &str) -> photocraft_engine::Result<Vec<photocraft_engine::prefs::ComfyUiModel>> {
        Ok(self.0.clone())
    }
    fn generate(
        &self,
        request: &photocraft_engine::ai_cmds::AiRequest,
        ctx: &photocraft_engine::jobs::JobCtx,
    ) -> photocraft_engine::Result<photocraft_raster::Rgba8Image> {
        ctx.check()?;
        let mut image = request.image.clone();
        let cutout = request.prompt == photocraft_engine::ai_cmds::REMOVE_BACKGROUND_PROMPT;
        let width = image.width as usize;
        let height = image.height as usize;
        for (i, pixel) in image.pixels.chunks_exact_mut(4).enumerate() {
            if cutout {
                let (x, y) = (i % width, i / width);
                let inside = (width / 4..width * 3 / 4).contains(&x) && (height / 4..height * 3 / 4).contains(&y);
                pixel.copy_from_slice(&[40, 160, 210, if inside { 255 } else { 0 }]);
            } else if request.mode == photocraft_engine::ai_cmds::EditMode::PromptEdit || pixel.get(3).copied().unwrap_or(255) < 255 {
                pixel.copy_from_slice(&[40, 160, 210, 255]);
            } else if let Some(alpha) = pixel.get_mut(3) {
                *alpha = 255;
            }
        }
        Ok(image)
    }
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = arg(&args, "--out").unwrap_or_else(|| "snapshot.png".into());
    let (w, h) = arg(&args, "--size")
        .and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?))))
        .unwrap_or((1440.0, 900.0));
    let scale: f32 = arg(&args, "--scale").and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let wayland_notice = args.iter().any(|a| a == "--wayland-notice");
    let mock_ai = args.iter().any(|a| a == "--mock-ai");
    let comfy_ai = args.iter().any(|a| a == "--comfy-ai");
    let script: Vec<(String, Value)> =
        arg(&args, "--script").map(|s| serde_json::from_str::<Vec<(String, Value)>>(&s).expect("--script must be [[method, params], …]")).unwrap_or_default();

    let services = Services {
        import: Some(Box::new(|name: &str, bytes: &[u8], depth: usize| {
            photocraft_io::import_with_svg_group_depth(name, bytes, depth).map(|r| (r.document, r.warnings)).map_err(|e| e.to_string())
        })),
        export: Some(Box::new(|doc: &photocraft_doc::Document, path: &str, settings: &photocraft_ui_egui::ExportSettings| {
            let mut opts = photocraft_io::ExportOptions::default();
            if let Some(q) = settings.jpeg_quality {
                opts.encode.jpeg_quality = q;
            }
            opts.encode.webp_lossless = settings.webp_lossless;
            if let Some(q) = settings.webp_quality {
                opts.encode.webp_quality = q;
            }
            photocraft_io::export(doc, path, &opts).map(|r| (r.bytes, r.warnings)).map_err(|e| e.to_string())
        })),
        write: Some(Box::new(|path: &str, bytes: &[u8]| photocraft_format::atomic_write(std::path::Path::new(path), bytes).map_err(|e| e.to_string()))),
        ..Default::default()
    };
    let open = arg(&args, "--open");
    let safe_gpu = args.iter().any(|a| a == "--safe-gpu");
    // `--background-jobs`: long commands run as background jobs, as in the desktop app (#210).
    let background_jobs = args.iter().any(|a| a == "--background-jobs");
    let custom_titlebar = args.iter().any(|a| a == "--custom-titlebar");
    // `--settle-ms N`: keep rendering frames for N ms before the capture (e.g. mid-job).
    let settle_ms: u64 = arg(&args, "--settle-ms").and_then(|s| s.parse().ok()).unwrap_or(0);
    let mock_models = match arg(&args, "--mock-ai-model") {
        Some(name) => photocraft_engine::prefs::ComfyUiModel::ALL.into_iter().filter(|model| model.name() == name).collect(),
        None => photocraft_engine::prefs::ComfyUiModel::ALL.to_vec(),
    };
    let gpu_setup = egui_kittest::wgpu::default_wgpu_setup();
    #[cfg(target_os = "windows")]
    let gpu_setup = {
        let mut setup = gpu_setup;
        if std::env::var_os("WGPU_BACKEND").is_none()
            && let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut setup
        {
            // Match native startup: skip AMD's crashing OpenGL adapter enumeration.
            create.instance_descriptor.backends = eframe::egui_wgpu::wgpu::Backends::PRIMARY;
        }
        setup
    };
    let mut harness =
        egui_kittest::Harness::builder().with_size(egui::vec2(w, h)).with_pixels_per_point(scale).with_max_steps(64).wgpu_setup(gpu_setup).build_eframe(
            move |cc| {
                PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
                let mut services = services;
                services.is_wayland = wayland_notice;
                services.xwayland_command = wayland_notice.then(|| "WAYLAND_DISPLAY= photocraft".into());
                let mut session = photocraft_engine::Session::new();
                if mock_ai {
                    session.ai_backend = Some(std::sync::Arc::new(MockAi(mock_models)));
                } else if comfy_ai {
                    session.ai_backend = Some(std::sync::Arc::new(photocraft_engine::comfyui::ComfyUi));
                }
                let mut app = PhotocraftApp::new(session, services);
                app.background_jobs = background_jobs;
                app.custom_titlebar = custom_titlebar;
                // `--safe-gpu`: the CPU canvas, as the desktop app's `--safe-gpu` launch.
                if safe_gpu {
                    app.perf.gpu_info.selected = "cpu".into();
                    app.perf.gpu_info.canvas = "cpu".into();
                    app.perf.gpu_info.fallback = Some("--safe-gpu: CPU renderer for this launch".into());
                    if let Some(rs) = cc.wgpu_render_state.as_ref() {
                        app.perf.gpu_info.set_adapter(&rs.adapter.get_info());
                    }
                } else if let Some(rs) = cc.wgpu_render_state.as_ref() {
                    app.set_wgpu(rs.clone());
                }
                if let Some(path) = &open {
                    app.open_path(path).expect("open --open file");
                }
                app
            },
        );
    if let Some((mw, mh)) = arg(&args, "--monitor").and_then(|s| s.split_once('x').and_then(|(a, b)| Some((a.parse::<f32>().ok()?, b.parse::<f32>().ok()?)))) {
        let top: f32 = arg(&args, "--window-top").and_then(|s| s.parse().ok()).unwrap_or(0.0);
        let v = harness.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default();
        v.monitor_size = Some(egui::vec2(mw, mh));
        v.inner_rect = Some(egui::Rect::from_min_size(egui::pos2(0.0, top), egui::vec2(w, h)));
        v.maximized = Some(false);
    }
    if let Some(appearance) = arg(&args, "--system-theme") {
        harness.input_mut().system_theme = match appearance.as_str() {
            "light" => Some(egui::Theme::Light),
            "dark" => Some(egui::Theme::Dark),
            "none" => None,
            _ => {
                eprintln!("--system-theme must be light, dark or none");
                std::process::exit(2);
            }
        };
    }
    harness.run_steps(4);
    let ctx = harness.ctx.clone();
    let timing = std::env::var_os("SNAPSHOT_TIMING").is_some();
    for (method, params) in script {
        let t0 = std::time::Instant::now();
        let label = format!("{method} {}", params.get("command").and_then(Value::as_str).unwrap_or(""));
        let (req, _rx) = ControlRequest::new(&method, params);
        let outcome = handle(harness.state_mut(), &ctx, &req);
        if let Outcome::Done(v) = &outcome
            && v.get("ok") == Some(&Value::Bool(false))
        {
            eprintln!("{method}: {v}");
        }
        // The harness doesn't call raw_input_hook: feed queued synthetic input step by step.
        loop {
            let step = harness.state_mut().take_synthetic_step();
            if step.is_empty() {
                break;
            }
            for e in step {
                harness.event(e);
            }
            harness.step();
        }
        harness.run_steps(4);
        if timing {
            eprintln!("{:>8.1} ms  {label}", t0.elapsed().as_secs_f64() * 1000.0);
        }
    }
    for (flag, button) in [("--click-at", egui::PointerButton::Primary), ("--right-click-at", egui::PointerButton::Secondary)] {
        let Some((x, y)) = arg(&args, flag).and_then(|s| s.split_once(',').and_then(|(x, y)| Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?)))) else {
            continue;
        };
        let pos = egui::pos2(x, y);
        harness.event(egui::Event::PointerMoved(pos));
        harness.step();
        harness.event(egui::Event::PointerButton { pos, button, pressed: true, modifiers: egui::Modifiers::NONE });
        harness.step();
        harness.event(egui::Event::PointerButton { pos, button, pressed: false, modifiers: egui::Modifiers::NONE });
        harness.run_steps(4);
    }
    let t_settle = std::time::Instant::now();
    while t_settle.elapsed() < std::time::Duration::from_millis(settle_ms) {
        harness.step();
    }
    // Let fade animations settle.
    for _ in 0..12 {
        harness.step();
    }
    let (req, _rx) = ControlRequest::new("ui.inspect", Value::Null);
    if let Outcome::Done(v) = handle(harness.state_mut(), &ctx, &req) {
        println!("perf: {}", v["result"]["perf"]["timings"]);
        if comfy_ai || mock_ai {
            println!("ai: {}", v["result"]["ai"]);
        }
    }
    println!(
        "language: {}",
        serde_json::json!({"preference": harness.state().session.prefs().interface.language, "resolved": photocraft_ui_egui::i18n::current().code()})
    );
    let img = harness.render().expect("render");
    img.save(&out).expect("save png");
    println!("wrote {out} ({}×{})", img.width(), img.height());
}

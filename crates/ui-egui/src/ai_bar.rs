//! Contextual, native prompt bar. Edits are dispatched through the engine registry.

use egui::{Rect, Stroke, pos2, vec2};
use photocraft_engine::ai_cmds::AiBackend;
use photocraft_engine::ai_cmds::{EditMode, MAX_PROMPT_BYTES, edit_mode, selection_signature, supported_document};
use photocraft_raster::Surface;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex, PoisonError, Weak};

use crate::PhotocraftApp;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AiUi {
    pub open: bool,
    pub prompt: String,
    pub mode: Option<EditMode>,
    pub rect: Option<[f32; 4]>,
    pub available: bool,
    pub checking: bool,
    #[serde(rename = "availabilityError")]
    pub availability_error: Option<String>,
    #[serde(skip)]
    document: Option<u64>,
    #[serde(skip)]
    signature: Option<u64>,
    // Keep old COW tiles alive: a recycled address must not look like an unchanged mask.
    #[serde(skip)]
    selection: Option<Surface>,
    #[serde(skip)]
    focus: bool,
}

impl PartialEq for AiUi {
    fn eq(&self, other: &Self) -> bool {
        self.open == other.open
            && self.prompt == other.prompt
            && self.mode == other.mode
            && self.rect == other.rect
            && self.available == other.available
            && self.checking == other.checking
            && self.availability_error == other.availability_error
    }
}

#[derive(Default)]
pub(crate) struct ServiceProbe {
    endpoint: String,
    backend: Option<Weak<dyn AiBackend>>,
    next_check: f64,
    pending: Option<ProbeSlot>,
}

type ProbeSlot = Arc<Mutex<Option<Result<(), String>>>>;

/// One bounded, read-only background check; never wait for HTTP on the UI thread.
pub(crate) fn poll_service(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let Some(backend) = app.session.ai_backend.clone() else {
        app.ai_probe = ServiceProbe::default();
        app.ui.ai.available = false;
        app.ui.ai.checking = false;
        app.ui.ai.availability_error = None;
        return;
    };
    let endpoint = app.session.prefs().integrations.comfyui_url.clone();
    let identity = Arc::downgrade(&backend);
    if app.ai_probe.endpoint != endpoint || app.ai_probe.backend.as_ref().is_none_or(|old| !old.ptr_eq(&identity)) {
        app.ai_probe = ServiceProbe { endpoint: endpoint.clone(), backend: Some(identity), ..Default::default() };
        app.ui.ai.available = false;
        app.ui.ai.checking = false;
        app.ui.ai.availability_error = None;
    }
    let now = ctx.input(|i| i.time);
    if !now.is_finite() {
        return;
    }
    let result = app.ai_probe.pending.as_ref().and_then(|slot| slot.lock().unwrap_or_else(PoisonError::into_inner).take());
    if let Some(result) = result {
        app.ai_probe.pending = None;
        app.ai_probe.next_check = now + 15.0;
        app.ui.ai.checking = false;
        app.ui.ai.available = result.is_ok();
        app.ui.ai.availability_error = result.err();
        if !app.ui.ai.available {
            app.ui.ai.open = false;
            app.ui.ai.rect = None;
        }
    }
    if app.ai_probe.pending.is_none() && now >= app.ai_probe.next_check {
        let slot = Arc::new(Mutex::new(None));
        app.ai_probe.pending = Some(slot.clone());
        app.ui.ai.checking = true;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let repaint = ctx.clone();
            let spawned = std::thread::Builder::new().name("photocraft-ai-check".into()).spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| backend.check_available(&endpoint)))
                    .map_err(|_| "AI availability check failed".to_owned())
                    .and_then(|r| r.map_err(|e| e.to_string()));
                *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
                repaint.request_repaint();
            });
            if let Err(error) = spawned {
                app.ai_probe.pending = None;
                app.ai_probe.next_check = now + 15.0;
                app.ui.ai.checking = false;
                app.ui.ai.available = false;
                app.ui.ai.availability_error = Some(error.to_string());
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(backend.check_available(&endpoint).map_err(|e| e.to_string()));
            ctx.request_repaint();
        }
    }
    if app.ai_probe.pending.is_none() {
        let wait_ms = ((app.ai_probe.next_check - now).clamp(0.05, 15.0) * 1000.0) as u64;
        ctx.request_repaint_after(std::time::Duration::from_millis(wait_ms));
    }
}

pub(crate) fn clear_document(state: &mut AiUi) {
    let (available, checking) = (state.available, state.checking);
    let error = state.availability_error.take();
    *state = AiUi { available, checking, availability_error: error, ..Default::default() };
}

pub fn open(app: &mut PhotocraftApp) -> Result<(), String> {
    let doc = &app.session.active().ok_or("no document open")?.doc;
    supported_document(doc)?;
    sync(&mut app.ui.ai, doc);
    app.ui.ai.open = true;
    app.ui.ai.focus = true;
    Ok(())
}

pub fn set_prompt(app: &mut PhotocraftApp, prompt: String) -> Result<(), String> {
    let doc = &app.session.active().ok_or("no document open")?.doc;
    supported_document(doc)?;
    sync(&mut app.ui.ai, doc);
    app.ui.ai.prompt = prompt;
    Ok(())
}

fn sync(state: &mut AiUi, doc: &photocraft_doc::Document) {
    if state.document != Some(doc.id.0) {
        clear_document(state);
        state.document = Some(doc.id.0);
    }
    let signature = selection_signature(doc);
    if state.signature != Some(signature) {
        state.mode = Some(edit_mode(doc));
        state.signature = Some(signature);
        state.selection = doc.selection.clone();
    }
}

fn label(mode: EditMode) -> &'static str {
    match mode {
        EditMode::PromptEdit => tl!("Prompt Edit"),
        EditMode::GenerativeFill => tl!("Generative Fill"),
    }
}

pub fn show(app: &mut PhotocraftApp, ui: &egui::Ui, index: usize, canvas: Rect, image: Rect, selection: Option<Rect>) {
    if !app.ui.ai.available {
        app.ui.ai.rect = None;
        return;
    }
    let Some(st) = app.session.documents().get(index) else { return };
    let doc = st.doc.clone();
    if !app.ui.dialogs.is_empty() || supported_document(&doc).is_err() || canvas.width() < 180.0 || canvas.height() < 100.0 {
        app.ui.ai.rect = None;
        return;
    }
    sync(&mut app.ui.ai, &doc);
    let mode = app.ui.ai.mode.unwrap_or(EditMode::PromptEdit);
    let anchor = if mode == EditMode::GenerativeFill { selection.unwrap_or(image) } else { image };
    // Scrollbar tracks must remain reachable beneath this foreground Area.
    let canvas = canvas.shrink2(vec2(20.0, 20.0));
    let width = if app.ui.ai.open { 440.0f32 } else { 200.0f32 }.min(canvas.width() - 16.0);
    let button_text = ui.painter().layout(
        label(mode).to_owned(),
        egui::TextStyle::Button.resolve(ui.style()),
        crate::theme::Tokens::get(ui.ctx()).text,
        (width - 32.0).max(1.0),
    );
    let height = if app.ui.ai.open { 128.0 } else { (button_text.size().y + 8.0).max(24.0) + 16.0 };
    if canvas.height() < height + 16.0 {
        app.ui.ai.rect = None;
        return;
    }
    let x = (anchor.center().x - width * 0.5).clamp(canvas.left() + 8.0, canvas.right() - width - 8.0);
    let y = (anchor.bottom() + 10.0).clamp(canvas.top() + 8.0, (canvas.bottom() - height - 8.0).max(canvas.top() + 8.0));
    let busy = app.session.job_on(doc.id).is_some();
    let tokens = crate::theme::Tokens::get(ui.ctx());
    let mut generate = false;
    let mut close = false;
    let area = egui::Area::new(egui::Id::new(("photocraft-ai-bar", doc.id.0))).order(egui::Order::Foreground).fixed_pos(pos2(x, y)).show(ui.ctx(), |ui| {
        egui::Frame::new()
            .fill(tokens.card)
            .stroke(Stroke::new(1.0, tokens.card_border))
            .corner_radius(tokens.radius_sm)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(width - 16.0);
                if !app.ui.ai.open {
                    if ui.add_enabled(!busy, egui::Button::new(label(mode)).min_size(vec2(width - 16.0, 24.0))).clicked() {
                        app.ui.ai.open = true;
                        app.ui.ai.focus = true;
                    }
                    return;
                }
                let hint = match mode {
                    EditMode::PromptEdit => tl!("Describe changes to the whole image"),
                    EditMode::GenerativeFill => tl!("Describe changes, or leave blank to remove"),
                };
                let response = egui::ScrollArea::vertical()
                    .max_height(60.0)
                    .show(ui, |ui| {
                        ui.add_enabled(
                            !busy,
                            egui::TextEdit::multiline(&mut app.ui.ai.prompt)
                                .hint_text(hint)
                                .desired_rows(2)
                                .desired_width(f32::INFINITY)
                                .char_limit(MAX_PROMPT_BYTES),
                        )
                    })
                    .inner;
                if app.ui.ai.focus {
                    response.request_focus();
                    app.ui.ai.focus = false;
                }
                ui.horizontal(|ui| {
                    ui.label("Qwen Image 2.1");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let valid = mode == EditMode::GenerativeFill || !app.ui.ai.prompt.trim().is_empty();
                        generate = ui.add_enabled(!busy && valid && app.ui.ai.prompt.len() <= MAX_PROMPT_BYTES, egui::Button::new(tl!("Generate"))).clicked();
                        close = ui.add_enabled(!busy, egui::Button::new(tl!("Cancel"))).clicked();
                    });
                });
            });
    });
    let r = area.response.rect;
    app.ui.ai.rect = Some([r.min.x, r.min.y, r.max.x, r.max.y]);
    if close {
        app.ui.ai.open = false;
    }
    if generate {
        let params = json!({"prompt":app.ui.ai.prompt,"name":label(mode)});
        if app.run(mode.command(), params).is_ok() {
            app.ui.ai.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Services;
    use photocraft_engine::Session;

    struct FakeAvailability(std::sync::atomic::AtomicBool);
    impl AiBackend for FakeAvailability {
        fn check_available(&self, _: &str) -> photocraft_engine::Result<()> {
            if self.0.load(std::sync::atomic::Ordering::Relaxed) { Ok(()) } else { Err(photocraft_engine::EngineError::Other("missing model".into())) }
        }
        fn generate(
            &self,
            _: &photocraft_engine::ai_cmds::AiRequest,
            _: &photocraft_engine::jobs::JobCtx,
        ) -> photocraft_engine::Result<photocraft_raster::Rgba8Image> {
            panic!("availability must not generate")
        }
    }

    fn finish_probe(app: &mut PhotocraftApp, ctx: &egui::Context) {
        for _ in 0..200 {
            poll_service(app, ctx);
            if !app.ui.ai.checking {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("availability check did not finish");
    }

    #[test]
    fn readiness_survives_document_changes_and_hides_the_bar_on_failure() {
        let backend = Arc::new(FakeAvailability(std::sync::atomic::AtomicBool::new(true)));
        let mut session = Session::new();
        session.ai_backend = Some(backend.clone());
        let mut app = PhotocraftApp::new(session, Services::default());
        let ctx = egui::Context::default();
        poll_service(&mut app, &ctx);
        assert!(!app.ui.ai.available);
        assert!(app.ui.ai.checking);
        finish_probe(&mut app, &ctx);
        assert!(app.ui.ai.available);
        clear_document(&mut app.ui.ai);
        assert!(app.ui.ai.available);
        app.run("file.new", json!({"width":20,"height":20})).unwrap();
        open(&mut app).unwrap();
        assert!(app.ui.ai.available);
        app.run("file.new", json!({"width":30,"height":30})).unwrap();
        open(&mut app).unwrap();
        assert!(app.ui.ai.available);
        backend.0.store(false, std::sync::atomic::Ordering::Relaxed);
        app.ai_probe.next_check = 0.0;
        poll_service(&mut app, &ctx);
        finish_probe(&mut app, &ctx);
        assert!(!app.ui.ai.available);
        assert!(!app.ui.ai.open);
        assert_eq!(app.ui.ai.availability_error.as_deref(), Some("missing model"));
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
            show(&mut app, ui, 0, rect, rect, None);
        });
        output.textures_delta.clear();
        assert!(app.ui.ai.rect.is_none());
        backend.0.store(true, std::sync::atomic::Ordering::Relaxed);
        app.session.edit_prefs(|p| p.integrations.comfyui_url = "http://127.0.0.1:8189".into());
        poll_service(&mut app, &ctx);
        assert!(!app.ui.ai.available);
        finish_probe(&mut app, &ctx);
        assert!(app.ui.ai.available);
        app.session.ai_backend = None;
        poll_service(&mut app, &ctx);
        assert!(!app.ui.ai.available);
        assert!(!app.ui.ai.checking);
    }

    #[test]
    fn mode_tracks_selection_and_open_does_not_mutate_document() {
        let mut app = PhotocraftApp::new(Session::new(), Services::default());
        assert!(open(&mut app).is_err());
        app.run("file.new", json!({"width":20,"height":20})).unwrap();
        let revision = app.session.active().unwrap().revision;
        open(&mut app).unwrap();
        assert_eq!(app.ui.ai.mode, Some(EditMode::PromptEdit));
        assert_eq!(app.session.active().unwrap().revision, revision);
        app.run("select.rect", json!({"x":2,"y":2,"width":4,"height":4})).unwrap();
        open(&mut app).unwrap();
        assert_eq!(app.ui.ai.mode, Some(EditMode::GenerativeFill));
        app.run("select.all", json!({})).unwrap();
        open(&mut app).unwrap();
        assert_eq!(app.ui.ai.mode, Some(EditMode::PromptEdit));
    }
}

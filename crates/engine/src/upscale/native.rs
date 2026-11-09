//! Native process transport and document commit for local upscaling.
use super::{COMMAND, EngineError, Model, Result, Session, Settings, Value, json, settings};
use crate::ai_cmds::cutout_external::{Backend, Kind, discover, local_file};
use photocraft_algo::resample::Resample;
use photocraft_color::{ColorMode, PixelFormat};
use photocraft_doc::{Document, Layer};
use photocraft_geom::Rect;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn error(e: impl std::fmt::Display) -> EngineError {
    EngineError::Other(e.to_string())
}

pub(super) fn catalog() -> Vec<Model> {
    let Some(b) = discover(Kind::Upscale) else { return Vec::new() };
    catalog_in(&b.root).unwrap_or_default()
}
fn catalog_in(root: &Path) -> Result<Vec<Model>> {
    let file = local_file(root, "catalog.json").ok_or_else(|| error("Upscaler catalog is missing"))?;
    if std::fs::metadata(&file).map_err(error)?.len() > 65536 {
        return Err(error("Upscaler catalog is too large"));
    }
    let models: Vec<Model> = serde_json::from_slice(&std::fs::read(file).map_err(error)?).map_err(error)?;
    if models.len() > 32 {
        return Err(error("Too many upscaler models"));
    }
    Ok(models
        .into_iter()
        .filter(|m| {
            valid_id(&m.id)
                && !m.name.is_empty()
                && m.name.len() <= 160
                && [2, 4].contains(&m.scale)
                && m.file.as_ref().is_none_or(|p| local_file(root, p).is_some())
                && m.required.len() <= 32
                && m.required.iter().all(|p| local_file(root, p).is_some())
        })
        .collect())
}
fn valid_id(v: &str) -> bool {
    !v.is_empty() && v.len() <= 64 && v.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}
fn command(backend: &Backend) -> Command {
    let mut cmd = Command::new(&backend.executable);
    cmd.args(&backend.args).current_dir(&backend.root).env("PHOTOCRAFT_BACKEND_ROOT", &backend.root).stdin(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    cmd
}
pub(super) fn devices() -> Result<Value> {
    let b = discover(Kind::Upscale).ok_or_else(|| error("No upscaler installed"))?;
    let dir = tempfile::tempdir().map_err(error)?;
    let stdout = dir.path().join("probe.json");
    let mut cmd = command(&b);
    cmd.arg("--probe").stdout(Stdio::from(std::fs::File::create(&stdout).map_err(error)?)).stderr(Stdio::null());
    let mut child = cmd.spawn().map_err(error)?;
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().map_err(error)? {
            break s;
        }
        if start.elapsed() > Duration::from_secs(15) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error("GPU detection timed out"));
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    if !status.success() {
        return Err(error("GPU runtime detection failed"));
    }
    read_json(&stdout)
}
fn read_json(path: &Path) -> Result<Value> {
    if std::fs::metadata(path).map_err(error)?.len() > 65536 {
        return Err(error("Backend metadata is too large"));
    }
    serde_json::from_slice(&std::fs::read(path).map_err(error)?).map_err(error)
}
pub(super) fn run(s: &mut Session, p: &Value) -> Result<Value> {
    let b = discover(Kind::Upscale).ok_or_else(|| error("No local upscaler installed"))?;
    let models = catalog_in(&b.root)?;
    let prefs = &s.prefs().integrations.upscale;
    let id = match p.get("model") {
        Some(v) => v.as_str().ok_or_else(|| error("model must be an id"))?,
        None => &prefs.model,
    };
    let model =
        if id.is_empty() { models.first() } else { models.iter().find(|m| m.id == id) }.cloned().ok_or_else(|| error("Select an installed upscaling model"))?;
    let opts = match p.get("options") {
        Some(v) => serde_json::from_value(v.clone()).map_err(error)?,
        None => settings(prefs, &model),
    };
    run_with(s, b, model, opts)
}
fn run_with(s: &mut Session, backend: Backend, model: Model, opts: Settings) -> Result<Value> {
    opts.validate(&model).map_err(error)?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    if !matches!(st.doc.mode, ColorMode::Rgb | ColorMode::Grayscale | ColorMode::Cmyk | ColorMode::Lab) || st.doc.size.width == 0 || st.doc.size.height == 0 {
        return Err(error("Upscaling requires a nonempty RGB, Gray, CMYK or Lab document"));
    }
    let nw = st.doc.size.width.checked_mul(model.scale).ok_or_else(|| error("Upscale width overflow"))?;
    let nh = st.doc.size.height.checked_mul(model.scale).ok_or_else(|| error("Upscale height overflow"))?;
    // Device kernels address planar RGB and padded tiles with signed 32-bit indices.
    let padded_pixels = u64::from(nw).saturating_add(1280).saturating_mul(u64::from(nh).saturating_add(1280));
    if padded_pixels > (i32::MAX as u64 / 3) {
        return Err(error("Upscaled image is too large for GPU indexing"));
    }
    let extra = st.doc.pixel_format().bytes_per_pixel() as u64;
    crate::image_cmds::check_working_budget(COMMAND, &st.doc, nw, nh, extra, " before upscaling")?;
    crate::memory_gate::ensure(
        "upscale the image",
        crate::memory_gate::image_size_need(&st.doc, nw, nh).saturating_add(u64::from(nw).saturating_mul(u64::from(nh)).saturating_mul(extra)),
    )?;
    let base = st.doc.clone();
    let scale = f64::from(model.scale);
    crate::jobs::run(
        s,
        "Upscale",
        true,
        move |ctx| {
            let mut doc = (*base).clone();
            let temp = tempfile::Builder::new().prefix("photocraft-upscale-").tempdir().map_err(error)?;
            let dir = temp.path();
            let input = dir.join("input.rgb");
            let alpha = dir.join("input.alpha");
            prepare(&doc, &input, &alpha, ctx)?;
            let req = json!({"version":1,"model":model.id,"width":doc.size.width,"height":doc.size.height,"scale":model.scale,"options":opts,
            "inputRgb":input,"inputAlpha":alpha,"outputRgb":dir.join("output.rgb"),"outputAlpha":dir.join("output.alpha"),"progress":dir.join("progress.json")});
            let request = dir.join("request.json");
            let response = dir.join("response.json");
            let cancel = dir.join("cancel");
            std::fs::write(&request, serde_json::to_vec(&req).map_err(error)?).map_err(error)?;
            execute(&backend, &request, &response, &cancel, ctx)?;
            let result = read_json(&response)?;
            if result.get("width").and_then(Value::as_u64) != Some(u64::from(nw)) || result.get("height").and_then(Value::as_u64) != Some(u64::from(nh)) {
                return Err(error("Upscaler returned unexpected dimensions"));
            }
            let has_alpha = result.get("hasAlpha").and_then(Value::as_bool).ok_or_else(|| error("Upscaler omitted its alpha format"))?;
            let layer = read_layer(&doc, nw, nh, &dir.join("output.rgb"), has_alpha.then_some(dir.join("output.alpha")).as_deref(), ctx)?;
            ctx.check()?;
            ctx.progress(0.96, "Resizing document");
            crate::image_cmds::resample_doc(&mut doc, COMMAND, nw, nh, Resample::Bicubic)?;
            for source in &mut doc.layers {
                source.visible = false;
            }
            let id = layer.id;
            doc.layers.push(layer);
            Ok((doc, id))
        },
        move |s, (doc, id)| {
            s.set_sampler_map(photocraft_geom::Affine { m: [scale, 0.0, 0.0, scale, 0.0, 0.0] });
            s.edit("Upscale", move |d, active| {
                *d = doc;
                *active = Some(id);
                Ok(())
            })?;
            Ok(json!({"layer":id.0,"width":nw,"height":nh}))
        },
    )
}
fn prepare(doc: &Document, rgb: &Path, alpha: &Path, ctx: &crate::jobs::JobCtx) -> Result<()> {
    let mut rgb = BufWriter::new(std::fs::File::create(rgb).map_err(error)?);
    let mut alpha = BufWriter::new(std::fs::File::create(alpha).map_err(error)?);
    let transform = photocraft_cms::Transform::new(
        &crate::color_cmds::composite_profile(doc),
        photocraft_cms::Builtin::Srgb.profile(),
        photocraft_cms::Intent::RelativeColorimetric,
        true,
    )
    .map_err(error)?;
    photocraft_compose::render_bands(doc, doc.bounds(), 32, |band| -> Result<()> {
        ctx.check()?;
        let mut converted = vec![0.0; band.px.len() * 4];
        transform.convert_f32(band.px.as_flattened(), 4, &mut converted, 4, true);
        for (pixel, source) in converted.chunks_exact(4).zip(&band.px) {
            if source.iter().any(|v| !v.is_finite()) || pixel.iter().any(|v| !v.is_finite()) {
                return Err(error("Invalid source pixel"));
            }
            if doc.depth == photocraft_color::SampleType::F32 && doc.mode == ColorMode::Rgb && source.iter().take(3).any(|v| *v < -0.0001 || *v > 1.0001) {
                return Err(error("HDR values are not supported by this SDR upscaler"));
            }
            for v in pixel.iter().take(3) {
                rgb.write_all(&v.clamp(0.0, 1.0).to_le_bytes()).map_err(error)?;
            }
            alpha.write_all(&source[3].clamp(0.0, 1.0).to_le_bytes()).map_err(error)?;
        }
        Ok(())
    })?;
    rgb.flush().map_err(error)?;
    alpha.flush().map_err(error)
}
fn execute(b: &Backend, request: &Path, response: &Path, cancel: &Path, ctx: &crate::jobs::JobCtx) -> Result<()> {
    let err = request.with_file_name("stderr.log");
    let mut cmd = command(b);
    cmd.env("PHOTOCRAFT_BACKEND_REQUEST", request)
        .env("PHOTOCRAFT_BACKEND_RESULT", response)
        .env("PHOTOCRAFT_BACKEND_CANCEL", cancel)
        .stdout(Stdio::null())
        .stderr(Stdio::from(std::fs::File::create(&err).map_err(error)?));
    let mut child = cmd.spawn().map_err(error)?;
    let start = Instant::now();
    let mut stopping = None;
    let status = loop {
        if let Some(s) = child.try_wait().map_err(error)? {
            break s;
        }
        if (ctx.cancelled() || start.elapsed() > Duration::from_secs(1800)) && stopping.is_none() {
            let _ = std::fs::write(cancel, b"cancel");
            stopping = Some(Instant::now());
        }
        if stopping.is_some_and(|t: Instant| t.elapsed() > Duration::from_secs(2)) {
            let _ = child.kill();
            let _ = child.wait();
            return if ctx.cancelled() { Err(EngineError::Cancelled) } else { Err(error("Upscaler timed out")) };
        }
        if let Ok(progress) = read_json(&request.with_file_name("progress.json"))
            && let Some(v) = progress.get("value").and_then(Value::as_f64).filter(|v| v.is_finite())
        {
            ctx.progress((v as f32).clamp(0.0, 0.94), progress.get("message").and_then(Value::as_str).unwrap_or("Upscaling"));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    ctx.check()?;
    if !status.success() {
        let mut bytes = Vec::new();
        if let Ok(mut f) = std::fs::File::open(err) {
            if let Ok(size) = f.metadata().map(|m| m.len()) {
                let _ = f.seek(SeekFrom::Start(size.saturating_sub(4096)));
            }
            let _ = f.take(4096).read_to_end(&mut bytes);
        }
        return Err(error(format!("Upscaler failed: {}", String::from_utf8_lossy(&bytes))));
    }
    Ok(())
}
fn read_layer(doc: &Document, w: u32, h: u32, rgb: &Path, alpha: Option<&Path>, ctx: &crate::jobs::JobCtx) -> Result<Layer> {
    let count = u64::from(w) * u64::from(h);
    if std::fs::metadata(rgb).map_err(error)?.len() != count * 12 {
        return Err(error("Upscaler RGB length mismatch"));
    }
    if let Some(a) = alpha
        && std::fs::metadata(a).map_err(error)?.len() != count * 4
    {
        return Err(error("Upscaler alpha length mismatch"));
    }
    let mut rgb = BufReader::new(std::fs::File::open(rgb).map_err(error)?);
    let mut alpha = alpha.map(std::fs::File::open).transpose().map_err(error)?.map(BufReader::new);
    let fmt = PixelFormat::new(doc.mode, doc.depth, true);
    let mut layer = Layer::raster("Upscale", fmt);
    let transform = photocraft_cms::Transform::new(
        photocraft_cms::Builtin::Srgb.profile(),
        &crate::color_cmds::document_profile(doc),
        photocraft_cms::Intent::RelativeColorimetric,
        true,
    )
    .map_err(error)?;
    let mut bytes = vec![0u8; w as usize * 12];
    let mut ab = vec![0u8; w as usize * 4];
    for y in 0..h {
        ctx.check()?;
        rgb.read_exact(&mut bytes).map_err(error)?;
        if let Some(a) = &mut alpha {
            a.read_exact(&mut ab).map_err(error)?;
        }
        let mut rgba = Vec::with_capacity(w as usize * 4);
        let colors = bytes.chunks_exact(12);
        let alphas = ab.chunks_exact(4);
        for (px, a) in colors.zip(alphas) {
            for b in px.chunks_exact(4) {
                let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                if !v.is_finite() {
                    return Err(error("Nonfinite upscaler RGB"));
                }
                rgba.push(v.clamp(0.0, 1.0));
            }
            let a = if alpha.is_some() { f32::from_le_bytes([a[0], a[1], a[2], a[3]]) } else { 1.0 };
            if !a.is_finite() || !(0.0..=1.0).contains(&a) {
                return Err(error("Invalid upscaler alpha"));
            }
            rgba.push(a);
        }
        let mut converted = vec![0.0; w as usize * fmt.channels()];
        transform.convert_f32(&rgba, 4, &mut converted, fmt.channels(), true);
        crate::pixels_mut(&mut layer)?.write_region(Rect::new(0, y as i32, w as i32, y as i32 + 1), &converted);
    }
    Ok(layer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_worker() {
        let Ok(request) = std::env::var("PHOTOCRAFT_BACKEND_REQUEST") else { return };
        let req: Value = serde_json::from_slice(&std::fs::read(request).unwrap()).unwrap();
        if req["model"] == "invalid" {
            std::fs::write(std::env::var("PHOTOCRAFT_BACKEND_RESULT").unwrap(), b"{\"width\":1,\"height\":1,\"hasAlpha\":false}").unwrap();
            return;
        }
        let w = req["width"].as_u64().unwrap() * req["scale"].as_u64().unwrap();
        let h = req["height"].as_u64().unwrap() * req["scale"].as_u64().unwrap();
        let rgb = [0.5f32.to_le_bytes(), 0.25f32.to_le_bytes(), 0.75f32.to_le_bytes()].concat().repeat((w * h) as usize);
        std::fs::write(req["outputRgb"].as_str().unwrap(), rgb).unwrap();
        std::fs::write(std::env::var("PHOTOCRAFT_BACKEND_RESULT").unwrap(), serde_json::to_vec(&json!({"width":w,"height":h,"hasAlpha":false})).unwrap())
            .unwrap();
    }

    #[test]
    fn result_resizes_document_and_undo_restores_it() {
        let temporary = tempfile::tempdir().unwrap();
        let backend = Backend {
            root: temporary.path().to_path_buf(),
            executable: std::env::current_exe().unwrap(),
            args: vec!["--exact".into(), "upscale::native::tests::mock_worker".into(), "--nocapture".into()],
            label: "SR".into(),
        };
        for depth in [8, 16, 32] {
            let mut session = Session::new();
            session.execute("file.new", json!({"width":7,"height":5,"depth":depth,"background":"white"})).unwrap();
            let before = session.active().unwrap().doc.clone();
            let model =
                Model { id: "test".into(), name: "Test".into(), scale: 2, restoration: false, defaults: Settings::default(), file: None, required: vec![] };
            run_with(&mut session, backend.clone(), model, Settings::default()).unwrap();
            let doc = &session.active().unwrap().doc;
            assert_eq!((doc.size.width, doc.size.height), (14, 10));
            assert_eq!(doc.layers.len(), before.layers.len() + 1);
            assert!(doc.layers.iter().take(before.layers.len()).all(|layer| !layer.visible));
            session.execute("edit.undo", json!({})).unwrap();
            let doc = &session.active().unwrap().doc;
            assert_eq!((doc.size.width, doc.size.height), (7, 5));
            assert_eq!(doc.layers.len(), before.layers.len());
            assert_eq!(doc.layers[0].visible, before.layers[0].visible);
        }
    }

    #[test]
    fn invalid_backend_result_does_not_change_document() {
        let temp = tempfile::tempdir().unwrap();
        let backend = Backend {
            root: temp.path().to_path_buf(),
            executable: std::env::current_exe().unwrap(),
            args: vec!["--exact".into(), "upscale::native::tests::mock_worker".into(), "--nocapture".into()],
            label: "SR".into(),
        };
        let mut session = Session::new();
        session.execute("file.new", json!({"width":7,"height":5,"background":"white"})).unwrap();
        let before = session.active().unwrap().doc.clone();
        let model =
            Model { id: "invalid".into(), name: "Invalid".into(), scale: 2, restoration: false, defaults: Settings::default(), file: None, required: vec![] };
        assert!(run_with(&mut session, backend, model, Settings::default()).is_err());
        let after = &session.active().unwrap().doc;
        assert_eq!(after.size, before.size);
        assert_eq!(after.layers.len(), before.layers.len());
        assert_eq!(after.layers[0].visible, before.layers[0].visible);
    }
}

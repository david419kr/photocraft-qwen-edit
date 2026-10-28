//! Local, user-installed external cutout backends.
use super::{ColorMode, Deserialize, Document, EngineError, JobCtx, Layer, LayerMask, PixelFormat, Rect, Result, Session, Surface, Value, json};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Deserialize)]
struct Manifest {
    version: u32,
    id: String,
    label: String,
    executable: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    required: Vec<String>,
}

#[derive(Clone, Debug)]
struct Backend {
    root: PathBuf,
    executable: PathBuf,
    args: Vec<String>,
    label: String,
}

fn local_file(root: &Path, relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    if relative.is_empty() || !path.components().all(|part| matches!(part, Component::Normal(_))) {
        return None;
    }
    let resolved = root.join(path).canonicalize().ok()?;
    (resolved.starts_with(root) && resolved.is_file()).then_some(resolved)
}

fn discover_in(parent: &Path) -> Option<Backend> {
    let mut entries: Vec<_> = std::fs::read_dir(parent).ok()?.take(101).filter_map(std::result::Result::ok).collect();
    if entries.len() > 100 {
        return None;
    }
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let Ok(root) = entry.path().canonicalize() else { continue };
        let Ok(base) = parent.canonicalize() else { return None };
        if !root.starts_with(&base) || !root.is_dir() {
            continue;
        }
        let Some(manifest_path) = local_file(&root, "backend.json") else { continue };
        let Ok(metadata) = std::fs::metadata(&manifest_path) else { continue };
        if metadata.len() > 65_536 {
            continue;
        }
        let Ok(bytes) = std::fs::read(manifest_path) else { continue };
        let Ok(manifest) = serde_json::from_slice::<Manifest>(&bytes) else { continue };
        if manifest.version != 1
            || manifest.id.is_empty()
            || manifest.id.len() > 64
            || !manifest.id.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
            || manifest.label.trim().is_empty()
            || manifest.label.chars().count() > 8
            || manifest.label.chars().any(char::is_control)
            || manifest.args.len() > 32
            || manifest.args.iter().any(|a| a.len() > 4096 || a.contains('\0'))
            || manifest.required.len() > 128
            || manifest.required.iter().any(|p| local_file(&root, p).is_none())
        {
            continue;
        }
        let Some(executable) = local_file(&root, &manifest.executable) else { continue };
        return Some(Backend { root, executable, args: manifest.args, label: manifest.label });
    }
    None
}

fn discover() -> Option<Backend> {
    type DiscoveryCache = Mutex<Option<(Instant, Option<Backend>)>>;
    static CACHE: OnceLock<DiscoveryCache> = OnceLock::new();
    let mut cache = CACHE.get_or_init(|| Mutex::new(None)).lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((at, backend)) = cache.as_ref()
        && at.elapsed() < Duration::from_secs(2)
    {
        return backend.clone();
    }
    let backend = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|p| p.join("cutout-backends"))).and_then(|p| discover_in(&p));
    *cache = Some((Instant::now(), backend.clone()));
    backend
}

pub(crate) fn label() -> Option<String> {
    discover().map(|backend| backend.label)
}

fn mask_from_png(bytes: &[u8], width: u32, height: u32, ctx: &JobCtx) -> Result<Surface> {
    use photocraft_codecs::{ChannelLayout, DecodeOptions, Format, Limits, SampleType};
    let pixels = u64::from(width) * u64::from(height);
    let limits = Limits { max_width: width, max_height: height, max_pixels: pixels, max_alloc: pixels.saturating_mul(16) };
    let image = photocraft_codecs::decode_as_with(Format::Png, bytes, &DecodeOptions { limits, ..Default::default() })
        .map_err(|e| EngineError::Other(format!("Invalid external mask: {e}")))?;
    if image.width() != width || image.height() != height || image.layout() != ChannelLayout::Gray || image.sample_type() != SampleType::U8 {
        return Err(EngineError::Other("External mask has incorrect size or format".into()));
    }
    let fmt = PixelFormat::new(ColorMode::Grayscale, photocraft_color::SampleType::F32, false);
    let mut surface = Surface::new(fmt);
    let mut any = false;
    for (y, row) in image.data().chunks_exact(width as usize).enumerate() {
        ctx.check()?;
        let values: Vec<f32> = row.iter().map(|v| f32::from(*v) / 255.0).collect();
        any |= row.iter().any(|v| *v > 0);
        surface.write_region(Rect::new(0, y as i32, width as i32, y as i32 + 1), &values);
    }
    if !any {
        return Err(EngineError::Other("External backend returned an empty mask".into()));
    }
    Ok(surface)
}

fn source_png(doc: &Document, id: photocraft_doc::LayerId) -> Result<Vec<u8>> {
    let original = doc.layer(id).ok_or(EngineError::NoLayer(id))?;
    let source = original.surface().ok_or(EngineError::NoLayer(id))?;
    let mut layer = Layer::raster("Cutout input", source.format());
    *crate::pixels_mut(&mut layer)? = source.clone();
    let mut isolated = doc.clone();
    isolated.layers = vec![layer];
    isolated.selection = None;
    photocraft_io::export(&isolated, "device-input.png", &Default::default())
        .map(|f| f.bytes)
        .map_err(|e| EngineError::Other(format!("Cannot prepare cutout input: {e}")))
}

fn spawn_mask(backend: &Backend, input: &[u8], max_output: u64, ctx: &JobCtx) -> Result<Vec<u8>> {
    let temporary = tempfile::Builder::new().prefix("photocraft-cutout-").tempdir().map_err(|e| EngineError::Other(e.to_string()))?;
    let src = temporary.path().join("source.png");
    let mask = temporary.path().join("mask.png");
    let cancel = temporary.path().join("cancel");
    let errors = temporary.path().join("error.log");
    std::fs::write(&src, input).map_err(|e| EngineError::Other(e.to_string()))?;
    let log = std::fs::File::create(&errors).map_err(|e| EngineError::Other(e.to_string()))?;
    ctx.check()?;
    let mut command = Command::new(&backend.executable);
    command
        .args(&backend.args)
        .current_dir(&backend.root)
        .env("PHOTOCRAFT_CUTOUT_ROOT", &backend.root)
        .env("PHOTOCRAFT_CUTOUT_INPUT", &src)
        .env("PHOTOCRAFT_CUTOUT_OUTPUT", &mask)
        .env("PHOTOCRAFT_CUTOUT_CANCEL", &cancel)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log));
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command.spawn().map_err(|e| EngineError::Other(format!("Cannot start external backend: {e}")))?;
    ctx.progress(0.15, "Removing background");
    let started = Instant::now();
    let mut cancel_started = None;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| EngineError::Other(e.to_string()))? {
            break status;
        }
        if (ctx.cancelled() || started.elapsed() > Duration::from_secs(600)) && cancel_started.is_none() {
            let _ = std::fs::write(&cancel, b"cancel");
            cancel_started = Some(Instant::now());
        }
        if cancel_started.is_some_and(|t| t.elapsed() > Duration::from_secs(10)) {
            let _ = child.kill();
            let _ = child.wait();
            return if ctx.cancelled() { Err(EngineError::Cancelled) } else { Err(EngineError::Other("External backend timed out".into())) };
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    ctx.check()?;
    if !status.success() {
        use std::io::Read;
        let mut bytes = Vec::new();
        if let Ok(file) = std::fs::File::open(errors) {
            let _ = file.take(8192).read_to_end(&mut bytes);
        }
        let tail: String = String::from_utf8_lossy(&bytes).chars().rev().take(2000).collect::<String>().chars().rev().collect();
        return Err(EngineError::Other(format!("External backend failed: {tail}")));
    }
    let metadata = std::fs::metadata(&mask).map_err(|e| EngineError::Other(format!("Cannot read external mask: {e}")))?;
    if metadata.len() > max_output {
        return Err(EngineError::Other("External mask is too large".into()));
    }
    std::fs::read(mask).map_err(|e| EngineError::Other(format!("Cannot read external mask: {e}")))
}

pub(crate) fn run(s: &mut Session, id: photocraft_doc::LayerId) -> Result<Value> {
    let backend = discover().ok_or_else(|| EngineError::Other("External cutout backend is unavailable".into()))?;
    run_with(s, id, backend)
}

fn run_with(s: &mut Session, id: photocraft_doc::LayerId, backend: Backend) -> Result<Value> {
    crate::jobs::edit_job(
        s,
        "Remove Background",
        move |doc, _, ctx| {
            let input = source_png(doc, id)?;
            let max_output = u64::from(doc.size.width).saturating_mul(u64::from(doc.size.height)).saturating_mul(16).saturating_add(1_048_576);
            let output = spawn_mask(&backend, &input, max_output, ctx)?;
            let surface = mask_from_png(&output, doc.size.width, doc.size.height, ctx)?;
            let mask = crate::commands::mask_at_depth(LayerMask { surface, ..LayerMask::reveal_all() }, doc.depth)?;
            crate::extra_cmds::background_to_layer_for_mask(doc, id);
            doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?.mask = Some(mask);
            doc.selection = None;
            Ok(())
        },
        move |()| json!({"layer":id.0,"backend":"external"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use photocraft_codecs::{ChannelLayout, EncodeOptions, Format, Image};

    #[test]
    fn mock_backend_process() {
        let Ok(output) = std::env::var("PHOTOCRAFT_CUTOUT_OUTPUT") else { return };
        assert!(std::env::var("PHOTOCRAFT_CUTOUT_INPUT").is_ok());
        let image = Image::from_u8(2, 2, ChannelLayout::Gray, vec![0, 255, 128, 0]).unwrap();
        std::fs::write(output, photocraft_codecs::encode(&image, Format::Png, &EncodeOptions::default()).unwrap()).unwrap();
    }

    #[test]
    fn external_process_preserves_pixels_and_undoes_at_all_depths() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Backend {
            root: dir.path().to_path_buf(),
            executable: std::env::current_exe().unwrap(),
            args: vec!["--exact".into(), "ai_cmds::cutout_external::tests::mock_backend_process".into()],
            label: "Test".into(),
        };
        for depth in [8, 16, 32] {
            let mut s = Session::new();
            s.execute("file.new", json!({"width":2,"height":2,"depth":depth,"background":"white"})).unwrap();
            let id = s.active().unwrap().active_layer.unwrap();
            let bounds = s.active().unwrap().doc.bounds();
            let before = s.active().unwrap().doc.layer(id).unwrap().surface().unwrap().read_region(bounds);
            run_with(&mut s, id, backend.clone()).unwrap();
            let layer = s.active().unwrap().doc.layer(id).unwrap();
            assert_eq!(layer.surface().unwrap().read_region(bounds), before);
            assert!((layer.mask.as_ref().unwrap().surface.sample_channel(1, 0, 0) - 1.0).abs() < 1e-6);
            s.undo();
            assert!(s.active().unwrap().doc.layer(id).unwrap().mask.is_none());
            s.redo();
            assert!(s.active().unwrap().doc.layer(id).unwrap().mask.is_some());
        }
        let ctx = JobCtx::new();
        ctx.cancel();
        assert!(matches!(spawn_mask(&backend, &[], 1024, &ctx), Err(EngineError::Cancelled)));
    }

    #[test]
    fn discovers_only_complete_contained_backends() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("example");
        std::fs::create_dir_all(&root).unwrap();
        let manifest = |executable: &str| json!({"version":1,"id":"example","label":"Test","executable":executable,"required":["model.dat"]}).to_string();
        std::fs::write(root.join("backend.json"), manifest("worker.exe")).unwrap();
        assert!(discover_in(dir.path()).is_none());
        std::fs::write(root.join("worker.exe"), []).unwrap();
        std::fs::write(root.join("model.dat"), []).unwrap();
        assert_eq!(discover_in(dir.path()).unwrap().label, "Test");
        std::fs::write(root.join("backend.json"), manifest("../worker.exe")).unwrap();
        assert!(discover_in(dir.path()).is_none());
    }

    #[test]
    fn mask_accepts_exact_grayscale_and_rejects_bad_geometry() {
        let image = Image::from_u8(2, 2, ChannelLayout::Gray, vec![0, 255, 128, 0]).unwrap();
        let png = photocraft_codecs::encode(&image, Format::Png, &EncodeOptions::default()).unwrap();
        let mask = mask_from_png(&png, 2, 2, &JobCtx::new()).unwrap();
        assert_eq!(mask.read_region(Rect::new(0, 0, 2, 1)), vec![0.0, 1.0]);
        assert!(mask_from_png(&png, 1, 2, &JobCtx::new()).is_err());
    }
}

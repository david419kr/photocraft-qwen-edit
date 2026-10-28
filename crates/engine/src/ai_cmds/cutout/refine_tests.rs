use super::*;
use std::sync::Mutex;

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "manual real Turbo4 inference; set PHOTOCRAFT_QWEN_PROBE_INPUT and PHOTOCRAFT_QWEN_PROBE_OUTPUT"]
fn real_turbo4_cutout_probe() {
    let input = std::env::var("PHOTOCRAFT_QWEN_PROBE_INPUT").expect("explicit probe image required");
    let output = std::env::var("PHOTOCRAFT_QWEN_PROBE_OUTPUT").expect("explicit probe output directory required");
    let bytes = std::fs::read(&input).unwrap();
    let imported = photocraft_io::import(&input, &bytes).unwrap();
    let mut s = Session::new();
    s.ai_backend = Some(Arc::new(crate::comfyui::ComfyUi::default()));
    s.add_document(imported.document, None);
    s.execute("prefs.set", json!({"path":"integrations.comfyuiModel","value":"turboInt8"})).unwrap();
    let begin = std::time::Instant::now();
    let refine = std::env::var("PHOTOCRAFT_QWEN_PROBE_REFINE").map(|v| v.parse::<bool>().expect("true or false required")).unwrap_or(true);
    let result = s.execute(REMOVE_BACKGROUND_COMMAND, json!({"steps":4,"seed":2026102819u64,"experimentalRefine":refine}));
    std::fs::create_dir_all(&output).unwrap();
    let report = json!({"seconds":begin.elapsed().as_secs_f64(),"steps":4,"result":result.as_ref().map_err(ToString::to_string)});
    std::fs::write(std::path::Path::new(&output).join("result.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    result.unwrap();
    let png = photocraft_io::export(&s.active().unwrap().doc, "result.png", &Default::default()).unwrap();
    std::fs::write(std::path::Path::new(&output).join("result.png"), png.bytes).unwrap();
    println!("{report}");
}

#[derive(Default)]
struct Backend {
    requests: Mutex<Vec<AiRequest>>,
    fail: usize,
    cancel: usize,
}
impl AiBackend for Backend {
    fn generate(&self, r: &AiRequest, ctx: &JobCtx) -> Result<Rgba8Image> {
        self.requests.lock().unwrap().push(r.clone());
        let n = self.requests.lock().unwrap().len();
        if n == self.fail {
            return Err(EngineError::Other("refinement failure".into()));
        }
        if n == self.cancel {
            ctx.cancel();
        }
        if let Some(g) = &r.guide {
            assert!(r.reference.is_some());
            assert!(r.prompt.contains("<image3>"));
            assert!(g.pixels.as_chunks::<4>().0.iter().all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255));
        } else {
            assert!(r.reference.is_none());
        }
        let mut out = r.image.clone();
        for p in out.pixels.as_chunks_mut::<4>().0 {
            p.copy_from_slice(&[255, 0, 255, 128]);
        }
        Ok(out)
    }
}

fn fixture(depth: u32, mode: &str, backend: Arc<Backend>) -> Session {
    let mut s = Session::new();
    s.ai_backend = Some(backend);
    s.execute("file.new", json!({"width":4097,"height":33,"depth":depth,"mode":mode,"background":"#b4a08c"})).unwrap();
    s
}

#[test]
fn normal_background_removal_never_enables_experimental_tiles() {
    let backend = Arc::new(Backend::default());
    let mut s = fixture(8, "rgb", backend.clone());
    let revision = s.active().unwrap().revision;
    for value in [json!(null), json!(1), json!("true"), json!([])] {
        assert!(s.execute(REMOVE_BACKGROUND_COMMAND, json!({"experimentalRefine":value})).is_err());
        assert_eq!(s.active().unwrap().revision, revision);
    }
    let result = s.execute(REMOVE_BACKGROUND_COMMAND, json!({})).unwrap();
    assert_eq!(result["experimentalRefine"], false);
    assert_eq!(result["refinementRegions"], json!([]));
    let requests = backend.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].guide.is_none());
    assert!(requests[0].reference.is_none());
    assert_eq!(s.active().unwrap().doc.size.width, 4097);
}

#[test]
fn refinement_keeps_native_formats_color_and_layer_identity_with_one_undo() {
    for mode in ["rgb", "gray", "cmyk", "lab"] {
        for depth in [8, 16, 32] {
            let backend = Arc::new(Backend::default());
            let mut s = fixture(depth, mode, backend.clone());
            let before = s.active().unwrap().doc.clone();
            let id = before.layers[0].id;
            let result = s.execute(REMOVE_BACKGROUND_COMMAND, json!({"seed":777,"experimentalRefine":true})).unwrap();
            let st = s.active().unwrap();
            assert_eq!(st.doc.size, before.size);
            assert_eq!(st.doc.depth, before.depth);
            assert_eq!(st.doc.icc_profile, before.icc_profile);
            assert_eq!(st.doc.layers.len(), before.layers.len());
            assert_eq!(st.active_layer, Some(id));
            assert_eq!(st.doc.layers[0].surface().unwrap().read_region(before.bounds()), before.layers[0].surface().unwrap().read_region(before.bounds()));
            assert!((st.doc.layers[0].mask.as_ref().unwrap().value(1440, 17) - 128.0 / 255.0).abs() < 0.003);
            assert_eq!(result["refinementRegions"].as_array().unwrap().len(), 3);
            assert_eq!(backend.requests.lock().unwrap().len(), 4);
            s.execute("edit.undo", json!({})).unwrap();
            assert!(s.active().unwrap().doc.layers[0].mask.is_none());
            s.execute("edit.redo", json!({})).unwrap();
            assert!(s.active().unwrap().doc.layers[0].mask.is_some());
        }
    }
}

#[test]
fn refinement_reroll_freezes_source_and_uses_stable_new_group_seeds() {
    let backend = Arc::new(Backend::default());
    let mut s = fixture(32, "rgb", backend.clone());
    s.execute(REMOVE_BACKGROUND_COMMAND, json!({"seed":123,"experimentalRefine":true})).unwrap();
    s.execute(REGENERATE_COMMAND, json!({})).unwrap();
    let requests = backend.requests.lock().unwrap();
    assert_eq!(requests.len(), 8);
    for i in 0..4 {
        assert_eq!(requests[i].image.pixels, requests[i + 4].image.pixels);
        assert_ne!(requests[i].seed, requests[i + 4].seed);
    }
    for i in 1..4 {
        assert_eq!(requests[i].seed, super::super::fill_batch::seed(123, i));
    }
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);
}

#[test]
fn crop_origins_and_existing_raster_alpha_are_not_shifted_or_multiplied_twice() {
    let backend = Arc::new(Backend::default());
    let mut s = fixture(32, "rgb", backend.clone());
    s.edit("RGBA source", |doc, _| {
        let pixels = doc.layers[0].surface_mut().unwrap();
        pixels.write_pixel(1600, 10, &[1.0, 0.0, 0.0, 0.5]);
        pixels.write_pixel(1601, 10, &[0.0, 1.0, 0.0, 0.0]);
        Ok(())
    })
    .unwrap();
    let source = s.active().unwrap().doc.clone();
    s.execute(REMOVE_BACKGROUND_COMMAND, json!({"experimentalRefine":true})).unwrap();
    let layer = &s.active().unwrap().doc.layers[0];
    assert_eq!(layer.mask.as_ref().unwrap().value(1600, 10), 1.0);
    assert_eq!(layer.mask.as_ref().unwrap().value(1601, 10), 0.0);
    assert_eq!(layer.surface().unwrap().pixel(1600, 10), source.layers[0].surface().unwrap().pixel(1600, 10));
    let requests = backend.requests.lock().unwrap();
    for (request, tile) in requests.iter().skip(1).zip(tiles::plan(source.bounds()).unwrap()) {
        if tile.context.contains(1600, 10) {
            let x = (1600 - tile.context.x0) as usize;
            let y = (10 - tile.context.y0) as usize;
            let i = (y * request.image.width as usize + x) * 4;
            assert_eq!(&request.image.pixels[i..i + 4], &[255, 0, 0, 128]);
        }
    }
}

#[test]
fn partial_failure_cancel_and_region_cap_preserve_the_original_mask_and_redo() {
    for cancel in [false, true] {
        let backend = Arc::new(Backend { fail: if cancel { 0 } else { 3 }, cancel: if cancel { 3 } else { 0 }, ..Default::default() });
        let mut s = fixture(16, "rgb", backend.clone());
        s.edit("old mask", |doc, _| {
            doc.layers[0].mask = Some(LayerMask::hide_all());
            Ok(())
        })
        .unwrap();
        s.edit("redo", |doc, _| {
            doc.layers[0].visible = false;
            Ok(())
        })
        .unwrap();
        s.execute("edit.undo", json!({})).unwrap();
        let revision = s.active().unwrap().revision;
        assert!(s.execute(REMOVE_BACKGROUND_COMMAND, json!({"experimentalRefine":true})).is_err());
        assert_eq!(backend.requests.lock().unwrap().len(), 3);
        assert_eq!(s.active().unwrap().revision, revision);
        assert!(s.active().unwrap().history.can_redo());
        assert_eq!(s.active().unwrap().doc.layers[0].mask.as_ref().unwrap().value(1, 1), 0.0);
        assert!(s.ai_runs.is_empty());
    }
    let backend = Arc::new(Backend::default());
    let mut s = fixture(8, "rgb", backend.clone());
    s.execute("file.new", json!({"width":6000,"height":6000,"background":"white"})).unwrap();
    assert!(s.execute(REMOVE_BACKGROUND_COMMAND, json!({"experimentalRefine":true})).is_err());
    assert!(backend.requests.lock().unwrap().is_empty());
    assert!(!s.active().unwrap().history.can_undo());
}

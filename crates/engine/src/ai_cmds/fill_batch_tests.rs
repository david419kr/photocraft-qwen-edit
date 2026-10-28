use super::*;
use std::sync::Mutex;

#[derive(Default)]
struct Backend {
    requests: Mutex<Vec<AiRequest>>,
    fail: usize,
    cancel: usize,
}
impl AiBackend for Backend {
    fn generate(&self, r: &AiRequest, ctx: &JobCtx) -> Result<Rgba8Image> {
        let mut requests = self.requests.lock().unwrap();
        requests.push(r.clone());
        let count = requests.len();
        drop(requests);
        if self.fail == count {
            return Err(EngineError::Other("second region failed".into()));
        }
        if self.cancel == count {
            ctx.cancel();
        }
        let mut output = r.image.clone();
        for p in output.pixels.as_chunks_mut::<4>().0 {
            for v in p.iter_mut().take(3) {
                *v = v.saturating_sub(30);
            }
            p[3] = 255;
        }
        Ok(output)
    }
}

fn fixture(depth: u32, mode: &str, backend: Arc<Backend>) -> Session {
    let mut s = Session::new();
    s.ai_backend = Some(backend);
    s.execute("file.new", json!({"width":960,"height":384,"depth":depth,"mode":mode,"background":"#b4a08c"})).unwrap();
    s.edit("Selection", |doc, _| {
        let mut mask = Surface::new(PixelFormat::new(ColorMode::Grayscale, photocraft_color::SampleType::F32, false));
        mask.fill_rect(Rect::new(32, 100, 52, 120), &[1.0]);
        mask.fill_rect(Rect::new(850, 200, 870, 224), &[1.0]);
        mask.write_pixel(32, 100, &[0.37]);
        mask.write_pixel(870, 224, &[1e-8]);
        doc.selection = Some(mask);
        Ok(())
    })
    .unwrap();
    s
}

#[test]
fn batch_preserves_source_coverage_exterior_and_one_undo_at_native_depths() {
    for depth in [8, 16, 32] {
        let backend = Arc::new(Backend::default());
        let mut s = fixture(depth, "rgb", backend.clone());
        let original = s.active().unwrap().doc.clone();
        let before = photocraft_compose::flatten(&original);
        let revision = s.active().unwrap().revision;
        let result = s.execute("edit.generativeFill", json!({"prompt":"","seed":123})).unwrap();
        assert_eq!(result["passes"].as_array().unwrap().len(), 2);
        assert_eq!(s.active().unwrap().revision, revision + 1);
        assert_eq!(s.active().unwrap().doc.layers.len(), original.layers.len() + 1);
        let st = s.active().unwrap();
        let layer = st.doc.layer(st.active_layer.unwrap()).unwrap();
        let expected = fill::prepare_groups(&original, &JobCtx::new()).unwrap();
        let requests = backend.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        for (request, input) in requests.iter().zip(&expected) {
            assert_eq!(request.image.pixels, input.image.pixels);
            assert!(request.reference.is_none());
        }
        assert_eq!(requests[1].seed, fill_batch::seed(123, 1));
        drop(requests);
        let after = photocraft_compose::flatten(&st.doc);
        for y in 0..384 {
            for x in 0..960 {
                let coverage = original.selection.as_ref().unwrap().sample_channel(x, y, 0);
                assert_eq!(layer.mask.as_ref().unwrap().surface.sample_channel(x, y, 0), coverage);
                if coverage == 0.0 {
                    assert_eq!(after.px[y as usize * 960 + x as usize], before.px[y as usize * 960 + x as usize]);
                }
            }
        }
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(s.active().unwrap().doc.layers.len(), original.layers.len());
        assert_eq!(photocraft_compose::flatten(&s.active().unwrap().doc).px, before.px);
        s.execute("edit.redo", json!({})).unwrap();
        assert_eq!(photocraft_compose::flatten(&s.active().unwrap().doc).px, after.px);
    }
}

#[test]
fn batch_reroll_freezes_inputs_and_poisson_remains_local_reversible_and_not_cumulative() {
    let backend = Arc::new(Backend::default());
    let mut s = fixture(16, "cmyk", backend.clone());
    s.execute("edit.generativeFill", json!({"prompt":"","seed":999})).unwrap();
    let layer = s.active().unwrap().active_layer.unwrap();
    let baseline = s.active().unwrap().doc.layer(layer).unwrap().surface().unwrap().read_region(Rect::new(0, 0, 960, 384));
    s.execute(harmonize::COMMAND, json!({"strength":50})).unwrap();
    let halfway = s.active().unwrap().doc.layer(layer).unwrap().surface().unwrap().read_region(Rect::new(0, 0, 960, 384));
    assert_ne!(halfway, baseline);
    s.execute(harmonize::COMMAND, json!({"strength":100})).unwrap();
    s.execute(harmonize::COMMAND, json!({"strength":50})).unwrap();
    assert_eq!(s.active().unwrap().doc.layer(layer).unwrap().surface().unwrap().read_region(Rect::new(0, 0, 960, 384)), halfway);
    s.execute(harmonize::COMMAND, json!({"strength":0})).unwrap();
    assert_eq!(s.active().unwrap().doc.layer(layer).unwrap().surface().unwrap().read_region(Rect::new(0, 0, 960, 384)), baseline);
    s.execute("prefs.set", json!({"path":"integrations.comfyuiModel","value":"qwenImage21"})).unwrap();
    s.execute(REGENERATE_COMMAND, json!({})).unwrap();
    assert_eq!(s.active().unwrap().active_layer, Some(layer));
    assert_eq!(s.active().unwrap().doc.layers.len(), 2);
    let requests = backend.requests.lock().unwrap();
    assert_eq!(requests.len(), 4);
    for index in 0..2 {
        assert_eq!(requests[index].image.pixels, requests[index + 2].image.pixels);
        assert_ne!(requests[index].seed, requests[index + 2].seed);
    }
    assert_eq!(requests[3].seed, fill_batch::seed(requests[2].seed, 1));
}

#[test]
fn failed_or_cancelled_later_pass_does_not_apply_partial_results_or_destroy_redo() {
    for cancel in [false, true] {
        let backend = Arc::new(Backend { fail: if cancel { 0 } else { 2 }, cancel: if cancel { 2 } else { 0 }, ..Default::default() });
        let mut s = fixture(8, "rgb", backend.clone());
        s.edit("Redo fixture", |doc, _| {
            doc.layers[0].visible = false;
            Ok(())
        })
        .unwrap();
        s.execute("edit.undo", json!({})).unwrap();
        let before = s.active().unwrap().doc.clone();
        let revision = s.active().unwrap().revision;
        assert!(s.execute("edit.generativeFill", json!({"prompt":""})).is_err());
        assert_eq!(backend.requests.lock().unwrap().len(), 2);
        assert_eq!(s.active().unwrap().revision, revision);
        assert!(s.active().unwrap().history.can_redo());
        assert_eq!(s.active().unwrap().doc.layers.len(), before.layers.len());
        assert_eq!(photocraft_compose::flatten(&s.active().unwrap().doc).px, photocraft_compose::flatten(&before).px);
        assert!(s.ai_runs.is_empty());
    }
}

#[test]
fn nonempty_fill_prompt_keeps_single_request() {
    let backend = Arc::new(Backend::default());
    let mut s = fixture(8, "rgb", backend.clone());
    s.execute("edit.generativeFill", json!({"prompt":"draw some flowers"})).unwrap();
    assert_eq!(backend.requests.lock().unwrap().len(), 1);
}

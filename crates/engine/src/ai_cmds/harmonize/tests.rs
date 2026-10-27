use super::*;
use crate::ai_cmds::{AiBackend, AiRequest};

struct Shift;
impl AiBackend for Shift {
    fn generate(&self, request: &AiRequest, _: &JobCtx) -> Result<Rgba8Image> {
        let mut result = request.image.clone();
        for pixel in result.pixels.as_chunks_mut::<4>().0 {
            for value in pixel.iter_mut().take(3) {
                *value = value.saturating_sub(30);
            }
            pixel[3] = 255;
        }
        Ok(result)
    }
}

fn fixture(depth: u32) -> Session {
    fixture_mode(depth, "rgb")
}

fn fixture_mode(depth: u32, mode: &str) -> Session {
    let mut session = Session::new();
    session.ai_backend = Some(Arc::new(Shift));
    session.execute("file.new", json!({"width":32,"height":32,"depth":depth,"mode":mode,"background":"#b4a08c"})).unwrap();
    session.execute("select.rect", json!({"x":8,"y":8,"width":12,"height":12})).unwrap();
    session.execute("edit.generativeFill", json!({"prompt":"","seed":123})).unwrap();
    session
}

#[test]
fn grayscale_cmyk_and_lab_preview_and_zero_restoration_keep_native_formats() {
    for mode in ["gray", "cmyk", "lab"] {
        for depth in [8, 16, 32] {
            let mut s = fixture_mode(depth, mode);
            let original = pixels(&s);
            let st = s.active().unwrap();
            let source = preview_source(&s, st.active_layer).unwrap();
            let preview = preview(&st.doc, st.active_layer, &json!({"strength":40}), &source, &JobCtx::new()).unwrap();
            s.execute(COMMAND, json!({"strength":40})).unwrap();
            assert_eq!(photocraft_compose::flatten(&preview).px, photocraft_compose::flatten(&s.active().unwrap().doc).px);
            s.execute(COMMAND, json!({"strength":0})).unwrap();
            assert_eq!(pixels(&s), original, "{mode} {depth}");
        }
    }
}

fn pixels(s: &Session) -> Vec<f32> {
    let st = s.active().unwrap();
    let LayerContent::Raster(surface) = &st.doc.layer(st.active_layer.unwrap()).unwrap().content else { panic!("raster") };
    surface.read_region(Rect::new(8, 8, 20, 20))
}

#[test]
fn explicit_only_preview_strength_reentry_undo_and_no_accumulation_at_all_depths() {
    for depth in [8, 16, 32] {
        let mut s = fixture(depth);
        let layer = s.active().unwrap().active_layer.unwrap();
        let original = pixels(&s);
        let source = preview_source(&s, Some(layer)).unwrap();
        let before = s.active().unwrap().doc.clone();
        let revision = s.active().unwrap().revision;
        let half = preview(&before, Some(layer), &json!({"strength":50}), &source, &JobCtx::new()).unwrap();
        assert_eq!(s.active().unwrap().revision, revision, "preview does not edit history");
        assert_eq!(pixels(&s), original);
        s.ai_backend = None; // Postprocessing is local; an AI connection is not required.
        s.execute(COMMAND, json!({"strength":50})).unwrap();
        assert_eq!(photocraft_compose::flatten(&s.active().unwrap().doc).px, photocraft_compose::flatten(&half).px);
        let halfway = pixels(&s);
        s.execute(COMMAND, json!({"strength":100})).unwrap();
        let full = pixels(&s);
        assert!(full[0] > original[0] + 0.05);
        let tolerance = if depth == 8 { 0.005 } else { 0.0001 };
        assert!((halfway[0] - (original[0] + full[0]) * 0.5).abs() < tolerance);
        s.execute(COMMAND, json!({"strength":50})).unwrap();
        assert_eq!(pixels(&s), halfway, "always starts from original generated pixels");
        assert_eq!(current_strength(&s, layer), Some(50.0));
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(pixels(&s), full);
        assert_eq!(current_strength(&s, layer), Some(100.0));
        s.execute("edit.redo", json!({})).unwrap();
        assert_eq!(pixels(&s), halfway);
        assert_eq!(current_strength(&s, layer), Some(50.0));
        s.execute(COMMAND, json!({"strength":0})).unwrap();
        assert_eq!(pixels(&s), original, "zero restores exact original result");
        let after = photocraft_compose::flatten(&s.active().unwrap().doc);
        let old = photocraft_compose::flatten(&before);
        for y in 0..32 {
            for x in 0..32 {
                if !(8..20).contains(&x) || !(8..20).contains(&y) {
                    assert_eq!(after.px[y * 32 + x], old.px[y * 32 + x]);
                }
            }
        }
        assert_eq!(identity(s.active().unwrap().doc.layer(layer).unwrap()), identity(before.layer(layer).unwrap()));
    }
}

#[test]
fn mask_feather_and_selection_are_not_changed_by_postprocessing() {
    let mut s = fixture(16);
    s.edit("Feather and hole", |doc, _| {
        let mask = doc.selection.as_mut().unwrap();
        mask.fill_rect(Rect::new(10, 10, 12, 12), &[0.0]);
        mask.fill_rect(Rect::new(8, 8, 10, 9), &[0.4]);
        Ok(())
    })
    .unwrap();
    s.execute("edit.generativeFill", json!({"prompt":"","seed":456})).unwrap();
    let layer = s.active().unwrap().active_layer.unwrap();
    let original_mask = s.active().unwrap().doc.layer(layer).unwrap().mask.as_ref().unwrap().surface.read_region(Rect::new(0, 0, 32, 32));
    s.execute("select.deselect", json!({})).unwrap();
    s.execute(COMMAND, json!({"strength":65})).unwrap();
    assert!(s.active().unwrap().doc.selection.is_none());
    assert_eq!(original_mask, s.active().unwrap().doc.layer(layer).unwrap().mask.as_ref().unwrap().surface.read_region(Rect::new(0, 0, 32, 32)));
    assert!(super::super::regeneration(&s).is_some(), "explicit color correction keeps regeneration available");
}

#[test]
fn older_generated_layers_remain_available_but_retouching_and_locks_block_overwrite() {
    let mut s = fixture(8);
    let first = s.active().unwrap().active_layer.unwrap();
    s.execute("edit.generativeFill", json!({"prompt":"new","seed":234})).unwrap();
    assert_ne!(s.active().unwrap().active_layer, Some(first));
    s.select_layer(first).unwrap();
    assert!(available(&s, first));
    s.edit("Retouch", |doc, _| {
        let LayerContent::Raster(pixels) = &mut doc.layer_mut(first).unwrap().content else { panic!("raster") };
        pixels.fill_rect(Rect::new(10, 10, 11, 11), &[1.0, 0.0, 0.0, 1.0]);
        Ok(())
    })
    .unwrap();
    assert!(!available(&s, first));
    assert!(s.execute(COMMAND, json!({"strength":50})).is_err());
    s.execute("edit.undo", json!({})).unwrap();
    assert!(available(&s, first));
    s.edit("Lock", |doc, _| {
        doc.layer_mut(first).unwrap().locks.pixels = true;
        Ok(())
    })
    .unwrap();
    assert!(!available(&s, first));
    s.execute("edit.undo", json!({})).unwrap();
    s.edit("Locked group", |doc, _| {
        let position = doc.layers.iter().position(|layer| layer.id == first).unwrap();
        let child = doc.layers.remove(position);
        let mut group = Layer::group("Locked", vec![child]);
        group.locks.all = true;
        doc.layers.push(group);
        Ok(())
    })
    .unwrap();
    assert!(!available(&s, first), "inherited pixel locks also protect generated layers");
    assert!(s.execute(COMMAND, json!({"strength":50})).is_err());
    s.execute("edit.undo", json!({})).unwrap();
    s.execute("file.new", json!({"width":32,"height":32})).unwrap();
    assert!(s.execute(COMMAND, json!({})).is_err());
    assert!(s.set_active(0));
    assert!(available(&s, first));
    s.close(0);
    assert!(s.ai_fill_sources.is_empty());
}

#[test]
fn invalid_params_and_cancellation_do_not_change_pixels_or_history() {
    let mut s = fixture(8);
    let before = pixels(&s);
    let revision = s.active().unwrap().revision;
    for params in [json!([]), json!({"strength":-1}), json!({"strength":101}), json!({"strength":"50"}), json!({"strength":null}), json!({"unexpected":1})] {
        assert!(s.execute(COMMAND, params).is_err());
        assert_eq!(s.active().unwrap().revision, revision);
    }
    let ctx = JobCtx::new();
    ctx.cancel();
    s.set_inline_job_ctx(Some(ctx));
    assert!(s.execute(COMMAND, json!({"strength":50})).is_err());
    assert_eq!(s.active().unwrap().revision, revision);
    assert_eq!(pixels(&s), before);
}

#[test]
fn bottom_bar_is_only_immediate_after_fill_and_never_returns_after_other_actions() {
    let mut s = fixture(8);
    let layer = s.active().unwrap().active_layer.unwrap();
    assert!(immediate(&s));
    s.note_action();
    assert!(!immediate(&s));
    assert!(available(&s, layer), "context-menu entry remains available");
    s.execute("edit.generativeFill", json!({"prompt":"","seed":22})).unwrap();
    assert!(immediate(&s));
    s.execute("select.deselect", json!({})).unwrap();
    assert!(!immediate(&s));
    s.execute("edit.undo", json!({})).unwrap();
    assert!(!immediate(&s), "undo does not revive an expired suggestion");
}

#[test]
fn malformed_internal_buffers_and_missing_known_context_return_errors() {
    let s = fixture(8);
    let st = s.active().unwrap();
    let source = preview_source(&s, st.active_layer).unwrap();
    let mut invalid = Source::new(source.doc.clone(), source.original.clone(), source.generated.clone(), source.region.clone(), source.baseline.clone());
    invalid.generated.pixels.pop();
    assert!(invalid.full_strength(&JobCtx::new()).is_err());
    invalid.generated = source.generated.clone();
    for pixel in invalid.original.pixels.as_chunks_mut::<4>().0 {
        pixel[3] = 0;
    }
    assert!(invalid.full_strength(&JobCtx::new()).unwrap_err().to_string().contains("No surrounding"));
}

#[test]
fn reused_identity_keys_cannot_overwrite_retouching() {
    let mut s = fixture(8);
    let layer = s.active().unwrap().active_layer.unwrap();
    s.edit("Retouch", |doc, _| {
        doc.layer_mut(layer).unwrap().surface_mut().unwrap().fill_rect(Rect::new(10, 10, 11, 11), &[1.0, 0.0, 0.0, 1.0]);
        Ok(())
    })
    .unwrap();
    let st = s.active().unwrap();
    let (doc, key) = (st.doc.id, identity(st.doc.layer(layer).unwrap()).unwrap());
    // Simulate a stale identity after the allocator reuses a freed tile address.
    s.ai_fill_sources.get_mut(&(doc, layer)).unwrap().states.insert(key, None);
    let before = pixels(&s);
    let revision = s.active().unwrap().revision;
    assert!(s.execute(COMMAND, json!({"strength":50})).is_err());
    assert_eq!(pixels(&s), before);
    assert_eq!(s.active().unwrap().revision, revision);
}

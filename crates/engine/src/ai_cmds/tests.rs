use super::*;
use photocraft_color::SampleType;
use std::sync::Arc;

struct Solid;
impl AiBackend for Solid {
    fn generate(&self, request: &AiRequest, ctx: &JobCtx) -> Result<Rgba8Image> {
        ctx.check()?;
        let mut image = request.image.clone();
        for pixel in image.pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[40, 180, 90, 255]);
        }
        Ok(image)
    }
}

fn session(depth: u32) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width":17,"height":13,"depth":depth,"background":"white"})).unwrap();
    s.ai_backend = Some(Arc::new(Solid));
    s
}

#[test]
fn prompt_edit_adds_top_layer_preserves_original_and_undo_redo() {
    for depth in [8, 16, 32] {
        let mut s = session(depth);
        let before = s.active().unwrap().doc.clone();
        let result = s.execute("edit.promptEdit", json!({"prompt":"  배경을 파란색으로 바꿔 주세요  ","seed":7})).unwrap();
        let doc = &s.active().unwrap().doc;
        assert_eq!(doc.size, before.size);
        assert_eq!(doc.depth, before.depth);
        assert_eq!(doc.layers.len(), before.layers.len() + 1);
        assert_eq!(doc.layers.last().unwrap().id.0, result["layer"]);
        assert_eq!(doc.layers.first().unwrap().id, before.layers.first().unwrap().id);
        assert!(doc.layers.last().unwrap().mask.is_none());
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(s.active().unwrap().doc.layers.len(), before.layers.len());
        s.execute("edit.redo", json!({})).unwrap();
        assert_eq!(s.active().unwrap().doc.layers.len(), before.layers.len() + 1);
    }
}

#[test]
fn ring_is_not_all_and_partial_fill_keeps_exact_feather_once() {
    let mut s = session(16);
    s.edit("Selection", |doc, _| {
        let mut mask = Surface::with_default(PixelFormat::GRAY8, &[1.0]);
        mask.write_pixel(8, 6, &[0.0]);
        mask.write_pixel(7, 6, &[0.5]);
        doc.selection = Some(mask);
        Ok(())
    })
    .unwrap();
    assert_eq!(edit_mode(&s.active().unwrap().doc), EditMode::GenerativeFill);
    let source = s.active().unwrap().doc.clone();
    s.execute("edit.generativeFill", json!({"prompt":""})).unwrap();
    let doc = &s.active().unwrap().doc;
    let layer = doc.layers.last().unwrap();
    let mask = layer.mask.as_ref().unwrap();
    assert_eq!(mask.surface.sample_channel(8, 6, 0), 0.0);
    assert_eq!(mask.surface.sample_channel(7, 6, 0), source.selection.as_ref().unwrap().sample_channel(7, 6, 0));
    assert_eq!(layer.surface().unwrap().sample_channel(7, 6, 3), 1.0);
    let a = photocraft_compose::render(&source, Rect::new(8, 6, 9, 7));
    let b = photocraft_compose::render(doc, Rect::new(8, 6, 9, 7));
    assert_eq!(a.px, b.px);
    assert!(doc.selection.is_some());
}

#[test]
fn explicit_all_uses_prompt_edit_and_empty_prompt_removes_only_in_fill() {
    let mut s = session(8);
    s.execute("select.all", json!({})).unwrap();
    assert_eq!(edit_mode(&s.active().unwrap().doc), EditMode::PromptEdit);
    assert_eq!(shaped_prompt(EditMode::PromptEdit, "  unchanged\n "), "  unchanged\n ");
    assert!(shaped_prompt(EditMode::GenerativeFill, " \n\t").starts_with("Remove the content"));
    assert!(shaped_prompt(EditMode::GenerativeFill, "달을 추가").contains("달을 추가"));
    assert!(s.execute("edit.generativeFill", json!({"prompt":""})).is_err());
}

#[test]
fn padding_keeps_rgb_under_transparent_mask_and_restores_original_size() {
    let mut s = session(8);
    s.execute("select.rect", json!({"x":2,"y":3,"width":4,"height":5})).unwrap();
    let prepared = prepare(&s.active().unwrap().doc, EditMode::GenerativeFill, &JobCtx::new()).unwrap();
    assert_eq!(prepared.content_size, [17, 13]);
    assert_eq!([prepared.image.width, prepared.image.height], [32, 32]);
    let px = |x: usize, y: usize| &prepared.image.pixels[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4];
    assert_eq!(px(3, 4), [255, 255, 255, 0]);
    assert_eq!(px(0, 0), [255, 255, 255, 255]);
    assert_eq!(px(31, 31)[3], 255);
    s.execute("edit.generativeFill", json!({"prompt":""})).unwrap();
    assert_eq!(s.active().unwrap().doc.size.width, 17);
}

#[test]
fn selection_identity_survives_a_pixel_edit_and_detects_selection_change() {
    let mut s = session(8);
    s.execute("select.rect", json!({"x":0,"y":0,"width":5,"height":5})).unwrap();
    let signature = selection_signature(&s.active().unwrap().doc);
    let retained = s.active().unwrap().doc.selection.clone();
    s.execute("layer.new.layer", json!({})).unwrap();
    assert_eq!(signature, selection_signature(&s.active().unwrap().doc));
    s.execute("select.rect", json!({"x":0,"y":0,"width":6,"height":5})).unwrap();
    assert_ne!(signature, selection_signature(&s.active().unwrap().doc));
    drop(retained);
}

#[test]
fn malformed_requests_do_not_edit_or_call_a_server() {
    let mut s = session(8);
    let revision = s.active().unwrap().revision;
    for params in [
        Value::Null,
        json!([]),
        json!({}),
        json!({"prompt":3}),
        json!({"prompt":""}),
        json!({"prompt":"x","seed":-1}),
        json!({"prompt":"x","name":false}),
        json!({"prompt":"x".repeat(MAX_PROMPT_BYTES+1)}),
    ] {
        assert!(s.execute("edit.promptEdit", params).is_err());
        assert_eq!(s.active().unwrap().revision, revision);
    }
    s.ai_backend = None;
    assert!(s.execute("edit.promptEdit", json!({"prompt":"x"})).is_err());
    assert_eq!(s.active().unwrap().revision, revision);
}

#[test]
fn gray_cmyk_and_lab_keep_their_depth_and_document_profile() {
    for mode in [ColorMode::Grayscale, ColorMode::Cmyk, ColorMode::Lab] {
        for depth in [SampleType::U8, SampleType::U16, SampleType::F32] {
            let mut s = session(8);
            s.edit("Mode", |doc, _| {
                doc.mode = mode;
                doc.depth = depth;
                Ok(())
            })
            .unwrap();
            s.execute("edit.promptEdit", json!({"prompt":"a change"})).unwrap();
            let doc = &s.active().unwrap().doc;
            assert_eq!(doc.mode, mode);
            assert_eq!(doc.depth, depth);
            assert_eq!(doc.layers.last().unwrap().surface().unwrap().format(), PixelFormat::new(mode, depth, true));
        }
    }
}

struct Fail;
impl AiBackend for Fail {
    fn generate(&self, _: &AiRequest, _: &JobCtx) -> Result<Rgba8Image> {
        Err(EngineError::Other("mock failure".into()))
    }
}

#[test]
fn backend_failure_leaves_document_and_selection_unchanged() {
    let mut s = session(8);
    s.ai_backend = Some(Arc::new(Fail));
    let revision = s.active().unwrap().revision;
    assert!(s.execute("edit.promptEdit", json!({"prompt":"change"})).is_err());
    assert_eq!(revision, s.active().unwrap().revision);
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);
}

#[test]
fn background_completion_returns_to_the_original_document() {
    let mut s = session(8);
    let original = s.active().unwrap().doc.id;
    let crate::jobs::Started::Job(job) = s.start("edit.promptEdit", json!({"prompt":"change"})).unwrap() else { panic!("expected background job") };
    assert!(s.execute("edit.fill", json!({"color":"#ff0000"})).is_err());
    s.execute("file.new", json!({"width":3,"height":3})).unwrap();
    let other = s.active().unwrap().doc.id;
    s.wait_job(job).unwrap();
    assert_eq!(s.active().unwrap().doc.id, other);
    assert_eq!(s.documents().iter().find(|d| d.doc.id == original).unwrap().doc.layers.len(), 2);
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);
}

#[test]
fn cancelling_a_background_edit_never_applies_its_layer() {
    let mut s = session(8);
    let crate::jobs::Started::Job(job) = s.start("edit.promptEdit", json!({"prompt":"change"})).unwrap() else { panic!("expected background job") };
    assert!(s.cancel_job(job));
    for _ in 0..10 {
        s.poll_jobs();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);
    assert!(!s.active().unwrap().history.can_undo());
}

#[test]
fn empty_selection_is_equivalent_to_no_selection() {
    let mut s = session(8);
    s.edit("Empty selection", |d, _| {
        d.selection = Some(Surface::new(PixelFormat::GRAY8));
        Ok(())
    })
    .unwrap();
    assert_eq!(edit_mode(&s.active().unwrap().doc), EditMode::PromptEdit);
}

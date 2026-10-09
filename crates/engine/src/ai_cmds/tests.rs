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
fn t2i_only_for_transparent_or_pure_white_without_partial_selection() {
    for depth in [8, 16, 32] {
        let mut s = session(depth);
        assert_eq!(contextual_mode(&s, s.active().unwrap()), EditMode::PromptGenerate);
        s.execute("select.all", json!({})).unwrap();
        assert_eq!(contextual_mode(&s, s.active().unwrap()), EditMode::PromptGenerate);
        s.execute("select.rect", json!({"x":2,"y":2,"width":5,"height":5})).unwrap();
        assert_eq!(contextual_mode(&s, s.active().unwrap()), EditMode::GenerativeFill);
        assert!(s.execute("edit.promptGenerate", json!({"prompt":"blue"})).is_err());
        s.execute("select.deselect", json!({})).unwrap();
        s.edit("one pixel", |doc, _| {
            doc.layers.first_mut().unwrap().surface_mut().unwrap().write_pixel(16, 12, &[0.0, 0.0, 0.0, 1.0]);
            Ok(())
        })
        .unwrap();
        assert_eq!(contextual_mode(&s, s.active().unwrap()), EditMode::PromptEdit);
    }
    for bg in ["transparent", "black"] {
        let mut s = Session::new();
        s.execute("file.new", json!({"width":17,"height":13,"background":bg})).unwrap();
        assert_eq!(blank_canvas(&s.active().unwrap().doc), bg == "transparent");
    }
    let mut s = session(32);
    s.edit("almost white", |doc, _| {
        let bounds = doc.bounds();
        doc.layers.first_mut().unwrap().surface_mut().unwrap().fill_rect(bounds, &[0.999, 0.999, 0.999, 1.0]);
        Ok(())
    })
    .unwrap();
    assert!(!blank_canvas(&s.active().unwrap().doc));
}

#[test]
fn t2i_canvas_size_is_exact_after_padding_and_is_not_edit_resolution_limited() {
    let mut s = session(8);
    s.execute("file.new", json!({"width":1601,"height":97,"background":"transparent"})).unwrap();
    let result = s.execute("edit.promptGenerate", json!({"prompt":"blue","seed":25})).unwrap();
    assert_eq!(result["workingSize"], json!([1632, 128]));
    let doc = &s.active().unwrap().doc;
    assert_eq!([doc.size.width, doc.size.height], [1601, 97]);
    let bounds = doc.layers.last().unwrap().surface().unwrap().content_bounds();
    assert_eq!(bounds, doc.bounds());
    assert!(s.execute("edit.promptGenerate", json!({"prompt":""})).is_err());
    s.execute("file.new", json!({"width":1,"height":20000,"background":"transparent"})).unwrap();
    assert!(s.execute("edit.promptGenerate", json!({"prompt":"blue"})).is_err());
}

#[test]
fn regeneration_keeps_source_prompt_model_mask_and_replaces_one_layer_with_undo() {
    struct Capture(Arc<std::sync::Mutex<Vec<AiRequest>>>);
    impl AiBackend for Capture {
        fn generate(&self, request: &AiRequest, _: &JobCtx) -> Result<Rgba8Image> {
            let mut calls = self.0.lock().unwrap();
            calls.push(request.clone());
            let value = if calls.len() % 2 == 1 { 35 } else { 185 };
            let mut image = request.image.clone();
            for pixel in image.pixels.chunks_exact_mut(4) {
                pixel.copy_from_slice(&[value, 80, 120, 255]);
            }
            Ok(image)
        }
    }
    for mode in [EditMode::PromptGenerate, EditMode::PromptEdit, EditMode::GenerativeFill] {
        let mut s = session(8);
        let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
        s.ai_backend = Some(Arc::new(Capture(calls.clone())));
        if mode == EditMode::GenerativeFill {
            s.execute("select.rect", json!({"x":2,"y":2,"width":5,"height":5})).unwrap();
        }
        s.execute("prefs.set", json!({"path":"integrations.comfyuiModel","value":"noctQv4"})).unwrap();
        let result = s.execute(mode.command(), json!({"prompt":"blue","seed":42})).unwrap();
        let before = s.active().unwrap().doc.clone();
        s.execute("prefs.set", json!({"path":"integrations.comfyuiModel","value":"qwenImage21"})).unwrap();
        let rerolled = s.execute(REGENERATE_COMMAND, json!({})).unwrap();
        assert_eq!(rerolled["layer"], result["layer"]);
        assert_ne!(rerolled["seed"], result["seed"]);
        assert_eq!(s.active().unwrap().doc.layers.len(), before.layers.len());
        let received = calls.lock().unwrap();
        assert_eq!(received.len(), 2);
        assert_eq!(received[0].image.pixels, received[1].image.pixels);
        assert_eq!(received[0].prompt, received[1].prompt);
        assert_eq!(received[1].model, crate::prefs::ComfyUiModel::NoctQv4);
        assert_eq!(received[0].mode, received[1].mode);
        drop(received);
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(
            s.active().unwrap().doc.layers.last().unwrap().surface().unwrap().sample_channel(0, 0, 0),
            before.layers.last().unwrap().surface().unwrap().sample_channel(0, 0, 0)
        );
        s.execute("edit.redo", json!({})).unwrap();
        assert!(s.execute(REGENERATE_COMMAND, json!({"seed":42})).is_err());
        s.execute("select.rect", json!({"x":1,"y":1,"width":2,"height":2})).unwrap();
        assert!(s.execute(REGENERATE_COMMAND, json!({})).is_err());
    }
}

#[test]
fn regenerate_failure_preserves_previous_result_and_closed_document_drops_snapshot() {
    let mut s = session(8);
    assert!(s.execute(REGENERATE_COMMAND, json!({})).is_err());
    s.execute("edit.promptEdit", json!({"prompt":"blue"})).unwrap();
    let before = s.active().unwrap().doc.clone();
    s.ai_backend = Some(Arc::new(Fail));
    assert!(s.execute(REGENERATE_COMMAND, json!({})).is_err());
    assert!(Arc::ptr_eq(&before, &s.active().unwrap().doc));
    assert!(regeneration(&s).is_some());
    s.close(0);
    assert!(s.ai_runs.is_empty());
}

#[test]
fn preference_selects_models_for_edit_fill_and_background_removal() {
    struct Capture(Arc<std::sync::Mutex<Vec<crate::prefs::ComfyUiModel>>>);
    impl AiBackend for Capture {
        fn generate(&self, request: &AiRequest, _: &JobCtx) -> Result<Rgba8Image> {
            self.0.lock().unwrap().push(request.model);
            Ok(request.image.clone())
        }
    }
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut s = session(8);
    s.ai_backend = Some(Arc::new(Capture(calls.clone())));
    s.execute("prefs.set", json!({"path":"integrations.comfyuiModel","value":"noctQv4"})).unwrap();
    s.execute("edit.promptEdit", json!({"prompt":"blue"})).unwrap();
    s.execute(REMOVE_BACKGROUND_COMMAND, json!({})).unwrap();
    s.execute("select.rect", json!({"x":2,"y":2,"width":5,"height":5})).unwrap();
    s.execute("edit.generativeFill", json!({"prompt":""})).unwrap();
    s.execute("select.deselect", json!({})).unwrap();
    s.execute("prefs.set", json!({"path":"integrations.comfyuiModel","value":"qwenImage21"})).unwrap();
    s.execute("edit.promptEdit", json!({"prompt":"red"})).unwrap();
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            crate::prefs::ComfyUiModel::NoctQv4,
            crate::prefs::ComfyUiModel::NoctQv4,
            crate::prefs::ComfyUiModel::NoctQv4,
            crate::prefs::ComfyUiModel::QwenImage21
        ]
    );
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
        assert!(!doc.layers.first().unwrap().visible);
        assert!(doc.layers.last().unwrap().visible);
        assert!(doc.layers.last().unwrap().mask.is_none());
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(s.active().unwrap().doc.layers.len(), before.layers.len());
        assert!(s.active().unwrap().doc.layers.first().unwrap().visible);
        s.execute("edit.redo", json!({})).unwrap();
        assert_eq!(s.active().unwrap().doc.layers.len(), before.layers.len() + 1);
        assert!(!s.active().unwrap().doc.layers.first().unwrap().visible);
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
    let removal = shaped_prompt(EditMode::GenerativeFill, " \n\t");
    assert!(removal.contains("Completely remove the marked shapes"));
    assert!(removal.contains("edit annotation, not part of the artwork"));
    assert!(removal.contains("bright-green paint in <image1>"));
    assert!(!removal.contains("<image2>"));
    assert!(!removal.contains("white region"));
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

struct Transparent;
impl AiBackend for Transparent {
    fn generate(&self, request: &AiRequest, _: &JobCtx) -> Result<Rgba8Image> {
        assert_eq!(request.image.pixels.get(..4), Some([255, 255, 255, 255].as_slice()));
        assert_eq!(request.prompt, REMOVE_BACKGROUND_PROMPT);
        assert_eq!(request.mode, EditMode::PromptEdit);
        assert!(!request.removal);
        let mut image = request.image.clone();
        for p in image.pixels.chunks_exact_mut(4) {
            p.copy_from_slice(&[60, 120, 180, 0]);
        }
        Ok(image)
    }
}

#[test]
fn background_removal_hides_all_source_roots_and_undo_restores_visibility() {
    for depth in [8, 16, 32] {
        let mut s = session(depth);
        s.execute("layer.new.layer", json!({"name":"Visible overlay"})).unwrap();
        s.execute("layer.new.layer", json!({"name":"Already hidden"})).unwrap();
        s.edit("Hide", |d, _| {
            d.layers.last_mut().unwrap().visible = false;
            Ok(())
        })
        .unwrap();
        let before = s.active().unwrap().doc.clone();
        let visibility: Vec<_> = before.layers.iter().map(|l| l.visible).collect();
        s.ai_backend = Some(Arc::new(Transparent));
        s.execute(REMOVE_BACKGROUND_COMMAND, json!({})).unwrap();
        let doc = &s.active().unwrap().doc;
        assert!(doc.layers.iter().take(before.layers.len()).all(|l| !l.visible));
        assert!(doc.layers.last().unwrap().visible);
        assert_eq!(doc.layers.last().unwrap().name, "Remove Background");
        assert!(photocraft_compose::render(doc, doc.bounds()).px.iter().all(|p| p[3] == 0.0));
        assert_eq!(doc.layers.first().unwrap().surface().unwrap().pixel(0, 0), before.layers.first().unwrap().surface().unwrap().pixel(0, 0));
        s.execute("edit.undo", json!({})).unwrap();
        assert_eq!(s.active().unwrap().doc.layers.iter().map(|l| l.visible).collect::<Vec<_>>(), visibility);
        s.execute("edit.redo", json!({})).unwrap();
        assert!(s.active().unwrap().doc.layers.iter().take(before.layers.len()).all(|l| !l.visible));
    }
}

#[test]
fn failed_background_removal_does_not_hide_sources() {
    let mut s = session(8);
    s.ai_backend = Some(Arc::new(Fail));
    for p in [Value::Null, json!({"seed":-1}), json!({})] {
        assert!(s.execute(REMOVE_BACKGROUND_COMMAND, p).is_err());
        assert!(s.active().unwrap().doc.layers.first().unwrap().visible);
        assert_eq!(s.active().unwrap().doc.layers.len(), 1);
    }
}

struct CheckRemovalIntent(bool);
impl AiBackend for CheckRemovalIntent {
    fn generate(&self, request: &AiRequest, ctx: &JobCtx) -> Result<Rgba8Image> {
        assert_eq!(request.removal, self.0);
        if self.0 {
            assert!(request.prompt.contains("edit annotation"));
        } else if request.mode == EditMode::GenerativeFill {
            assert!(request.prompt.contains("white region indicated by <image2>"));
        }
        Solid.generate(request, ctx)
    }
}

#[test]
fn only_empty_fill_prompts_select_the_removal_reference() {
    let mut s = session(8);
    s.execute("select.rect", json!({"x":2,"y":3,"width":4,"height":5})).unwrap();
    for prompt in ["", " \n\t", "달을 추가"] {
        s.ai_backend = Some(Arc::new(CheckRemovalIntent(prompt.trim().is_empty())));
        s.execute("edit.generativeFill", json!({"prompt":prompt})).unwrap();
    }
    s.execute("select.deselect", json!({})).unwrap();
    s.ai_backend = Some(Arc::new(CheckRemovalIntent(false)));
    s.execute("edit.promptEdit", json!({"prompt":"change the image"})).unwrap();
}

#[test]
fn backend_failure_leaves_document_and_selection_unchanged() {
    let mut s = session(8);
    s.ai_backend = Some(Arc::new(Fail));
    let revision = s.active().unwrap().revision;
    assert!(s.execute("edit.promptEdit", json!({"prompt":"change"})).is_err());
    assert_eq!(revision, s.active().unwrap().revision);
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);
    assert!(s.active().unwrap().doc.layers.first().unwrap().visible);
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

struct DriftRemoval;
impl AiBackend for DriftRemoval {
    fn generate(&self, request: &AiRequest, ctx: &JobCtx) -> Result<Rgba8Image> {
        ctx.check()?;
        let mut image = request.image.clone();
        for pixel in image.pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[96, 128, 160, 255]);
        }
        // A new texture detail must survive colour harmonisation.
        let offset = (6 * image.width as usize + 6) * 4;
        image.pixels.get_mut(offset..offset + 4).unwrap().copy_from_slice(&[104, 128, 160, 255]);
        Ok(image)
    }
}

#[test]
fn fill_matches_background_without_restoring_removed_content_and_keeps_texture() {
    for depth in [8, 16, 32] {
        for (x, y) in [(2, 2), (0, 0)] {
            let mut s = session(depth);
            s.execute("edit.fill", json!({"color":"#406080"})).unwrap();
            s.execute("select.rect", json!({"x":x,"y":y,"width":10,"height":8})).unwrap();
            s.execute("edit.fill", json!({"color":"#ffffff"})).unwrap();
            let before = s.active().unwrap().doc.clone();
            s.ai_backend = Some(Arc::new(DriftRemoval));
            s.execute("edit.generativeFill", json!({"prompt":""})).unwrap();
            let doc = &s.active().unwrap().doc;
            let rendered = photocraft_compose::render(doc, doc.bounds());
            let background = rendered.px[5 * 17 + 5];
            let texture = rendered.px[6 * 17 + 6];
            for (actual, expected) in background.iter().take(3).zip([64.0, 96.0, 128.0]) {
                assert!((*actual - expected / 255.0).abs() < 2.0 / 255.0, "{depth}: {background:?}");
            }
            assert!((texture[0] - background[0] - 8.0 / 255.0).abs() < 2.0 / 255.0);
            let outside = Rect::new(15, 11, 17, 13);
            assert_eq!(photocraft_compose::render(doc, outside).px, photocraft_compose::render(&before, outside).px);
            assert_eq!(doc.layers.first().unwrap().surface().unwrap().pixel(5, 5), before.layers.first().unwrap().surface().unwrap().pixel(5, 5));
            s.execute("edit.undo", json!({})).unwrap();
            assert_eq!(s.active().unwrap().doc.layers.len(), before.layers.len());
        }
    }
}

#[test]
fn fill_blending_excludes_padding_and_handles_missing_context_bad_buffers_and_cancel() {
    let mut s = session(8);
    s.execute("select.all", json!({})).unwrap();
    let input = prepare(&s.active().unwrap().doc, EditMode::GenerativeFill, &JobCtx::new()).unwrap();
    let generated = Solid
        .generate(
            &AiRequest {
                image: input.image.clone(),
                prompt: "".into(),
                mode: EditMode::GenerativeFill,
                removal: true,
                endpoint: "".into(),
                seed: 1,
                model: crate::prefs::ComfyUiModel::default(),
            },
            &JobCtx::new(),
        )
        .unwrap();
    let blended = harmonize_fill(&generated, &input.image, input.content_size, &JobCtx::new()).unwrap();
    assert!(blended.chunks_exact(3).all(|p| (p[0] - 40.0 / 255.0).abs() < 1e-6));
    assert!(harmonize_fill(&generated, &input.image, [0, 13], &JobCtx::new()).is_err());
    let mut malformed = generated.clone();
    malformed.pixels.pop();
    assert!(harmonize_fill(&malformed, &input.image, input.content_size, &JobCtx::new()).is_err());
    let absurd = Rgba8Image { width: u32::MAX, height: u32::MAX, pixels: Vec::new() };
    assert!(harmonize_fill(&absurd, &absurd, input.content_size, &JobCtx::new()).is_err());
    let ctx = JobCtx::new();
    ctx.cancel();
    assert!(matches!(harmonize_fill(&generated, &input.image, input.content_size, &ctx), Err(EngineError::Cancelled)));
}

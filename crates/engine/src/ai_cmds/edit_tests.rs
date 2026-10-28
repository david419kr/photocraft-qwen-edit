use super::*;
use std::sync::Mutex;

fn source(w: u32, h: u32) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width":w,"height":h,"background":"white"})).unwrap();
    s.ai_backend = Some(Arc::new(super::tests::Solid));
    s
}

fn attach(s: &mut Session, color: &str) {
    let mut input = Session::new();
    input.execute("file.new", json!({"width":33,"height":17,"background":color})).unwrap();
    let file = photocraft_io::export(&input.active().unwrap().doc, "reference.png", &Default::default()).unwrap();
    s.execute(reference::SET, json!({"name":"reference.png","data":photocraft_paint::tile::b64_encode(&file.bytes)})).unwrap();
}

#[test]
fn references_are_frozen_for_reroll_and_do_not_edit_document_history() {
    struct Capture(Mutex<Vec<AiRequest>>);
    impl AiBackend for Capture {
        fn generate(&self, r: &AiRequest, ctx: &JobCtx) -> Result<Rgba8Image> {
            self.0.lock().unwrap().push(r.clone());
            super::tests::Solid.generate(r, ctx)
        }
    }
    let mut s = source(64, 64);
    let revision = s.active().unwrap().revision;
    attach(&mut s, "#c04020");
    assert_eq!(s.active().unwrap().revision, revision);
    assert!(!s.active().unwrap().history.can_undo());
    let first = reference::current(&s).unwrap();
    assert_eq!(first.content, [33, 17]);
    let backend = Arc::new(Capture(Mutex::new(Vec::new())));
    s.ai_backend = Some(backend.clone());
    s.execute("edit.promptEdit", json!({"prompt":"use the reference texture"})).unwrap();
    attach(&mut s, "#2040c0");
    s.execute(reference::CLEAR, json!({})).unwrap();
    s.execute(REGENERATE_COMMAND, json!({})).unwrap();
    let requests = backend.0.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        assert!(Arc::ptr_eq(request.reference.as_ref().unwrap(), &first.image));
        assert!(request.prompt.contains("<image2>"));
    }
    drop(requests);
    s.close(0);
    assert!(s.ai_references.is_empty());
}

#[test]
fn prompt_edit_resize_requires_exact_current_approval_and_commits_once() {
    let mut s = source(3000, 1800);
    let original = s.active().unwrap().doc.clone();
    let plan = edit_preflight(&s).unwrap();
    assert_eq!(plan["needsResizeApproval"], true);
    assert!(s.execute("edit.promptEdit", json!({"prompt":"change"})).is_err());
    assert_eq!(s.active().unwrap().doc.size, original.size);
    assert!(!s.active().unwrap().history.can_undo());
    let result = s.execute("edit.promptEdit", json!({"prompt":"change","resizeApproval":plan["approval"]})).unwrap();
    let target = serde_json::from_value::<[u32; 2]>(plan["targetSize"].clone()).unwrap();
    assert_eq!([s.active().unwrap().doc.size.width, s.active().unwrap().doc.size.height], target);
    assert_eq!(result["contentSize"], plan["targetSize"]);
    s.execute(REGENERATE_COMMAND, json!({})).unwrap();
    assert_eq!(s.active().unwrap().doc.layers.len(), 2);
    s.execute("edit.undo", json!({})).unwrap();
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.active().unwrap().doc.size, original.size);
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!([s.active().unwrap().doc.size.width, s.active().unwrap().doc.size.height], target);
}

#[test]
fn failed_or_cancelled_resize_never_changes_native_document_or_redo() {
    for cancel in [false, true] {
        let mut s = source(3000, 1800);
        s.edit("fixture", |doc, _| {
            doc.layers[0].surface_mut().unwrap().write_pixel(5, 5, &[1.0, 0.0, 0.0, 1.0]);
            Ok(())
        })
        .unwrap();
        s.execute("edit.undo", json!({})).unwrap();
        s.active_mut().unwrap().color_samplers = vec![[1200.0, 800.0]];
        let before = s.active().unwrap().doc.clone();
        let revision = s.active().unwrap().revision;
        let plan = edit_preflight(&s).unwrap();
        s.ai_backend = Some(Arc::new(super::tests::Fail));
        let params = json!({"prompt":"change","resizeApproval":plan["approval"]});
        if cancel {
            let crate::jobs::Started::Job(id) = s.start("edit.promptEdit", params).unwrap() else { panic!("job expected") };
            s.cancel_job(id);
            let _ = s.wait_job(id);
        } else {
            assert!(s.execute("edit.promptEdit", params).is_err());
        }
        assert_eq!(s.active().unwrap().doc.size, before.size);
        assert_eq!(s.active().unwrap().revision, revision);
        assert!(s.active().unwrap().history.can_redo());
        assert_eq!(s.active().unwrap().doc.layers.len(), 1);
        assert_eq!(s.active().unwrap().color_samplers, vec![[1200.0, 800.0]]);
    }
}

#[test]
fn approved_resize_preserves_native_formats_profile_and_maps_samplers_only_once() {
    for mode in ["rgb", "gray", "cmyk", "lab"] {
        for depth in [8, 16, 32] {
            let mut s = Session::new();
            s.ai_backend = Some(Arc::new(super::tests::Solid));
            s.execute("file.new", json!({"width":4101,"height":65,"mode":mode,"depth":depth,"background":"#b4a08c"})).unwrap();
            if mode == "rgb" {
                s.edit("P3 fixture", |doc, _| {
                    doc.icc_profile = Some(Builtin::DisplayP3.profile().to_bytes());
                    Ok(())
                })
                .unwrap();
            }
            s.active_mut().unwrap().color_samplers = vec![[3100.0, 40.0]];
            let before = s.active().unwrap().doc.clone();
            let plan = edit_preflight(&s).unwrap();
            assert_eq!(plan["needsResizeApproval"], true);
            s.execute("edit.promptEdit", json!({"prompt":"change","resizeApproval":plan["approval"]})).unwrap();
            let after = &s.active().unwrap().doc;
            assert_eq!(after.mode, before.mode);
            assert_eq!(after.depth, before.depth);
            assert_eq!(after.icc_profile, before.icc_profile);
            assert_eq!(after.layers[0].surface().unwrap().format(), before.layers[0].surface().unwrap().format());
            let expected = [3100.0 * (f64::from(after.size.width) / 4101.0), 40.0 * (f64::from(after.size.height) / 65.0)];
            let mapped = s.active().unwrap().color_samplers.clone();
            for (actual, want) in mapped[0].into_iter().zip(expected) {
                assert!((actual - want).abs() < 1e-9);
            }
            s.execute(REGENERATE_COMMAND, json!({})).unwrap();
            assert_eq!(s.active().unwrap().color_samplers, mapped);
            s.execute("edit.undo", json!({})).unwrap();
            s.execute("edit.undo", json!({})).unwrap();
            assert_eq!(s.active().unwrap().doc.size, before.size);
            assert_eq!(s.active().unwrap().doc.icc_profile, before.icc_profile);
        }
    }
}

#[test]
fn approval_cannot_be_reused_after_document_or_reference_change() {
    let mut s = source(3000, 1800);
    let plan = edit_preflight(&s).unwrap();
    attach(&mut s, "#204080");
    assert!(s.execute("edit.promptEdit", json!({"prompt":"change","resizeApproval":plan["approval"]})).is_err());
    s.execute("file.new", json!({"width":64,"height":64})).unwrap();
    assert!(s.execute("edit.promptEdit", json!({"prompt":"change","resizeApproval":plan["approval"]})).is_err());
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);
}

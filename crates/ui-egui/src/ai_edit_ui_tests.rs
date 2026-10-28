use super::*;
use photocraft_engine::ai_cmds::{AiBackend, AiRequest, reference};
use photocraft_engine::jobs::JobCtx;
use photocraft_raster::Rgba8Image;
use std::sync::Arc;

struct Echo;
impl AiBackend for Echo {
    fn generate(&self, r: &AiRequest, ctx: &JobCtx) -> photocraft_engine::Result<Rgba8Image> {
        ctx.check()?;
        Ok(r.image.clone())
    }
}
fn app() -> PhotocraftApp {
    let mut s = photocraft_engine::Session::new();
    s.ai_backend = Some(Arc::new(Echo));
    let mut app = PhotocraftApp::new(s, Default::default());
    app.run("file.new", json!({"width":3000,"height":1800,"background":"#809090"})).unwrap();
    app
}

#[test]
fn resize_dialog_cancel_confirm_and_stale_document_are_automatable() {
    let mut app = app();
    let old = app.session.active().unwrap().doc.size;
    let reply = app.run("edit.promptEdit", json!({"prompt":"change","steps":4})).unwrap();
    let id = reply["dialog"].as_u64().unwrap();
    assert_eq!(app.session.active().unwrap().doc.size, old);
    crate::dialogs::cancel(&mut app, id).unwrap();
    assert!(app.ui.ai.open);
    assert!(!app.session.active().unwrap().history.can_undo());
    let reply = app.run("edit.promptEdit", json!({"prompt":"change","steps":4})).unwrap();
    crate::dialogs::confirm(&mut app, reply["dialog"].as_u64().unwrap()).unwrap();
    assert!(app.session.active().unwrap().doc.size.width < old.width);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(app.session.active().unwrap().doc.size, old);
    let reply = app.run("edit.promptEdit", json!({"prompt":"change"})).unwrap();
    app.run("file.new", json!({"width":64,"height":64})).unwrap();
    assert!(crate::dialogs::confirm(&mut app, reply["dialog"].as_u64().unwrap()).is_err());
    assert_eq!(app.session.active().unwrap().doc.layers.len(), 1);
}

#[test]
fn existing_file_picker_attaches_reference_without_opening_a_document() {
    let mut app = app();
    let count = app.session.documents().len();
    let revision = app.session.active().unwrap().revision;
    let mut reference_doc = photocraft_engine::Session::new();
    reference_doc.execute("file.new", json!({"width":32,"height":32,"background":"#c08030"})).unwrap();
    let png = photocraft_io::export(&reference_doc.active().unwrap().doc, "ref.png", &Default::default()).unwrap().bytes;
    app.services.file_dialog = Some(Box::new(move |request, _, reply| {
        assert!(matches!(request, crate::file_dialog::FileDialogRequest::Open { multiple: false, .. }));
        reply.send(Some(crate::file_dialog::FileDialogAnswer::Contents("ref.png".into(), png.clone())));
    }));
    app.run(reference::SET, json!({})).unwrap();
    let ctx = egui::Context::default();
    app.poll_file_dialog(&ctx, None);
    app.poll_file_dialog(&ctx, None);
    crate::ai_edit_ui::sync_reference(&mut app);
    assert_eq!(app.session.documents().len(), count);
    assert_eq!(app.session.active().unwrap().revision, revision);
    assert_eq!(app.ui.ai.reference.as_ref().unwrap()["name"], "ref.png");
    app.run(reference::CLEAR, json!({})).unwrap();
    crate::ai_edit_ui::sync_reference(&mut app);
    assert!(app.ui.ai.reference.is_none());
}

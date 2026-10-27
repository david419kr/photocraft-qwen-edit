use super::*;
use egui_kittest::{Harness, kittest::Queryable};
use photocraft_engine::ai_cmds::{AiBackend, AiRequest, harmonize};

struct Shift;
impl AiBackend for Shift {
    fn generate(&self, request: &AiRequest, _: &photocraft_engine::jobs::JobCtx) -> photocraft_engine::Result<photocraft_raster::Rgba8Image> {
        let mut image = request.image.clone();
        for pixel in image.pixels.as_chunks_mut::<4>().0 {
            for v in pixel.iter_mut().take(3) {
                *v = v.saturating_sub(30);
            }
            pixel[3] = 255;
        }
        Ok(image)
    }
}

fn fixture() -> PhotocraftApp {
    let mut session = photocraft_engine::Session::new();
    session.ai_backend = Some(Arc::new(Shift));
    let mut app = PhotocraftApp::new(session, Default::default());
    app.run("file.new", json!({"width":32,"height":32,"background":"#b4a08c"})).unwrap();
    app.run("select.rect", json!({"x":8,"y":8,"width":12,"height":12})).unwrap();
    app.run("edit.generativeFill", json!({"prompt":"","seed":1})).unwrap();
    app.ui.ai.available = true;
    app
}

#[test]
fn original_comparison_is_preview_only_and_confirm_cancel_and_reopen_are_consistent() {
    let mut app = fixture();
    let original = app.session.active().unwrap().doc.clone();
    let layer = app.session.active().unwrap().active_layer;
    let source = harmonize::preview_source(&app.session, layer).unwrap();
    let id = open(&mut app, harmonize::COMMAND).unwrap();
    assert_eq!(app.ui.dialog_mut(id).unwrap().fields["strength"], json!(100.0));
    let fields = &mut app.ui.dialog_mut(id).unwrap().fields;
    fields.insert("strength".into(), json!(35));
    fields.insert("__compareOriginal".into(), json!(true));
    let compare = preview_params_of(fields);
    let commit = params_of(fields);
    assert_eq!(compare, json!({"strength":0}));
    assert_eq!(commit, json!({"strength":35}));
    let ctx = photocraft_engine::jobs::JobCtx::new();
    let preview = preview_document_for_target(
        &original,
        layer,
        harmonize::COMMAND,
        &compare,
        1,
        None,
        photocraft_engine::channel_cmds::ChannelTarget::Composite,
        &[],
        &Default::default(),
        Some(&source),
    )
    .unwrap();
    assert_eq!(photocraft_compose::flatten(&preview).px, photocraft_compose::flatten(&original).px);
    let expected = harmonize::preview(&original, layer, &commit, &source, &ctx).unwrap();
    let reduced = preview_document_for_target(
        &original,
        layer,
        harmonize::COMMAND,
        &commit,
        4,
        None,
        photocraft_engine::channel_cmds::ChannelTarget::Composite,
        &[],
        &Default::default(),
        Some(&source),
    )
    .unwrap();
    assert_eq!(photocraft_compose::flatten(&reduced).px, photocraft_compose::flatten(&crate::proxy::proxy_document(&expected, 4)).px);
    crate::dialogs::confirm(&mut app, id).unwrap();
    assert_eq!(photocraft_compose::flatten(&app.session.active().unwrap().doc).px, photocraft_compose::flatten(&expected).px);
    let revision = app.session.active().unwrap().revision;
    let id = open(&mut app, harmonize::COMMAND).unwrap();
    assert_eq!(app.ui.dialog_mut(id).unwrap().fields["strength"], json!(35.0));
    assert_eq!(app.ui.dialog_mut(id).unwrap().fields["__compareOriginal"], json!(false));
    app.ui.dialog_mut(id).unwrap().fields.insert("strength".into(), json!(95));
    app.ui.close_dialog(id);
    assert_eq!(app.session.active().unwrap().revision, revision, "cancel adds no edit");
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(photocraft_compose::flatten(&app.session.active().unwrap().doc).px, photocraft_compose::flatten(&original).px);
}

#[test]
fn bottom_bar_and_layer_menu_open_the_same_dialog_in_english_and_korean() {
    for lang in [crate::i18n::Lang::EN, crate::i18n::Lang::from_pref("ko")] {
        for menu in [false, true] {
            let mut app = fixture();
            if menu {
                let background = app.session.active().unwrap().doc.layers[0].id;
                app.session.select_layer(background).unwrap();
            }
            let mut h = Harness::builder().with_size(egui::vec2(1100.0, 720.0)).build_ui_state(
                move |ui, app| {
                    if !ui.ctx().fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                        return;
                    }
                    crate::i18n::with_language(lang, || {
                        if menu {
                            let layer = app.session.active().unwrap().doc.layers.last().unwrap().clone();
                            let mut actions = Vec::new();
                            crate::layer_menu_ui::show(app, ui, &layer, false, &mut actions);
                            for (command, params) in actions {
                                if params.is_null() {
                                    crate::menus::invoke(app, ui.ctx(), &command, json!({})).unwrap();
                                } else {
                                    app.run(&command, params).unwrap();
                                }
                            }
                        } else {
                            let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1100.0, 720.0));
                            let image = egui::Rect::from_min_size(egui::pos2(60.0, 60.0), egui::vec2(800.0, 400.0));
                            crate::ai_bar::show(app, ui, 0, canvas, image, Some(image));
                        }
                    });
                },
                app,
            );
            PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::ALL[0]);
            h.run_steps(4);
            let label = crate::i18n::with_language(lang, || tl!("Match Surrounding Colors").to_owned());
            let button = h.get_by_label(&label);
            if !menu {
                let rect = h.state().ui.ai.rect.unwrap();
                assert!(rect[3] - rect[1] <= 44.0, "the post-fill bar stays on one row: {rect:?}");
                let remove = crate::i18n::with_language(lang, || tl!("Remove").to_owned());
                assert!(h.query_by_label(&remove).is_none(), "the color action replaces Remove");
            }
            assert!(button.rect().width() > 80.0);
            assert!(button.rect().right() <= 1100.0);
            assert!(button.rect().bottom() <= 720.0, "{lang:?} menu={menu}: {:?}", button.rect());
            button.click();
            h.run_steps(3);
            assert!(!h.state().ui.dialogs.is_empty(), "{lang:?} menu={menu}: no dialog after button click");
            assert_eq!(h.state().ui.dialogs.last().unwrap().fields["__command"], harmonize::COMMAND);
        }
    }
}

#[test]
fn subsequent_actions_hide_the_bottom_action_without_losing_context_menu_access() {
    let app = fixture();
    let mut h = Harness::builder().with_size(egui::vec2(1100.0, 720.0)).build_ui_state(
        |ui, app| {
            if !ui.ctx().fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                return;
            }
            let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1100.0, 720.0));
            let image = egui::Rect::from_min_size(egui::pos2(60.0, 60.0), egui::vec2(800.0, 400.0));
            crate::ai_bar::show(app, ui, 0, canvas, image, Some(image));
        },
        app,
    );
    PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::ALL[0]);
    h.run_steps(4);
    assert!(h.query_by_label(harmonize::LABEL).is_some());
    let layer = h.state().session.active().unwrap().active_layer.unwrap();
    h.state_mut().run("select.deselect", json!({})).unwrap();
    h.run_steps(4);
    assert!(h.query_by_label(harmonize::LABEL).is_none());
    h.state_mut().run("select.rect", json!({"x":8,"y":8,"width":12,"height":12})).unwrap();
    h.state_mut().session.select_layer(layer).unwrap();
    h.run_steps(4);
    assert!(h.query_by_label(harmonize::LABEL).is_none(), "same selection and layer do not revive the bar");
    assert!(harmonize::available(&h.state().session, layer));
    let ctx = h.ctx.clone();
    crate::menus::invoke(h.state_mut(), &ctx, harmonize::COMMAND, json!({})).unwrap();
    assert!(!h.state().ui.dialogs.is_empty());
}

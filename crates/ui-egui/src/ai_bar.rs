//! Contextual, native prompt bar. Edits are dispatched through the engine registry.

use egui::{Rect, Stroke, pos2, vec2};
use photocraft_engine::ai_cmds::AiBackend;
use photocraft_engine::ai_cmds::{EditMode, MAX_PROMPT_BYTES, contextual_mode, regeneration, selection_signature, supported_document};
use photocraft_engine::prefs::ComfyUiModel;
use photocraft_raster::Surface;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Arc, Mutex, PoisonError, Weak};

use crate::PhotocraftApp;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AiUi {
    pub open: bool,
    pub prompt: String,
    pub mode: Option<EditMode>,
    pub rect: Option<[f32; 4]>,
    pub available: bool,
    pub checking: bool,
    #[serde(rename = "availabilityError")]
    pub availability_error: Option<String>,
    #[serde(rename = "availableModels")]
    pub available_models: Vec<ComfyUiModel>,
    #[serde(skip)]
    document: Option<u64>,
    #[serde(skip)]
    signature: Option<u64>,
    // Keep old COW tiles alive: a recycled address must not look like an unchanged mask.
    #[serde(skip)]
    selection: Option<Surface>,
    #[serde(skip)]
    focus: bool,
    #[serde(skip)]
    submit: Option<u64>,
}

impl PartialEq for AiUi {
    fn eq(&self, other: &Self) -> bool {
        self.open == other.open
            && self.prompt == other.prompt
            && self.mode == other.mode
            && self.rect == other.rect
            && self.available == other.available
            && self.checking == other.checking
            && self.availability_error == other.availability_error
            && self.available_models == other.available_models
    }
}

#[derive(Default)]
pub(crate) struct ServiceProbe {
    endpoint: String,
    backend: Option<Weak<dyn AiBackend>>,
    next_check: f64,
    pending: Option<ProbeSlot>,
}

type ProbeSlot = Arc<Mutex<Option<Result<Vec<ComfyUiModel>, String>>>>;

/// One bounded, read-only background check; never wait for HTTP on the UI thread.
pub(crate) fn poll_service(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let Some(backend) = app.session.ai_backend.clone() else {
        app.ai_probe = ServiceProbe::default();
        app.ui.ai.available = false;
        app.ui.ai.checking = false;
        app.ui.ai.availability_error = None;
        app.ui.ai.available_models.clear();
        return;
    };
    let endpoint = app.session.prefs().integrations.comfyui_url.clone();
    let identity = Arc::downgrade(&backend);
    if app.ai_probe.endpoint != endpoint || app.ai_probe.backend.as_ref().is_none_or(|old| !old.ptr_eq(&identity)) {
        app.ai_probe = ServiceProbe { endpoint: endpoint.clone(), backend: Some(identity), ..Default::default() };
        app.ui.ai.available = false;
        app.ui.ai.checking = false;
        app.ui.ai.availability_error = None;
        app.ui.ai.available_models.clear();
    }
    let now = ctx.input(|i| i.time);
    if !now.is_finite() {
        return;
    }
    let result = app.ai_probe.pending.as_ref().and_then(|slot| slot.lock().unwrap_or_else(PoisonError::into_inner).take());
    if let Some(result) = result {
        app.ai_probe.pending = None;
        app.ai_probe.next_check = now + 15.0;
        app.ui.ai.checking = false;
        match result {
            Ok(models) => {
                app.ui.ai.available = !models.is_empty();
                app.ui.ai.available_models = models;
                app.ui.ai.availability_error = None;
            }
            Err(error) => {
                app.ui.ai.available = false;
                app.ui.ai.available_models.clear();
                app.ui.ai.availability_error = Some(error);
            }
        }
        if !app.ui.ai.available {
            app.ui.ai.open = false;
            app.ui.ai.rect = None;
        }
    }
    if app.ai_probe.pending.is_none() && now >= app.ai_probe.next_check {
        let slot = Arc::new(Mutex::new(None));
        app.ai_probe.pending = Some(slot.clone());
        app.ui.ai.checking = true;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let repaint = ctx.clone();
            let spawned = std::thread::Builder::new().name("photocraft-ai-check".into()).spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| backend.available_models(&endpoint)))
                    .map_err(|_| "AI availability check failed".to_owned())
                    .and_then(|r| r.map_err(|e| e.to_string()));
                *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
                repaint.request_repaint();
            });
            if let Err(error) = spawned {
                app.ai_probe.pending = None;
                app.ai_probe.next_check = now + 15.0;
                app.ui.ai.checking = false;
                app.ui.ai.available = false;
                app.ui.ai.availability_error = Some(error.to_string());
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(backend.available_models(&endpoint).map_err(|e| e.to_string()));
            ctx.request_repaint();
        }
    }
    if app.ai_probe.pending.is_none() {
        let wait_ms = ((app.ai_probe.next_check - now).clamp(0.05, 15.0) * 1000.0) as u64;
        ctx.request_repaint_after(std::time::Duration::from_millis(wait_ms));
    }
}

pub(crate) fn clear_document(state: &mut AiUi) {
    let (available, checking) = (state.available, state.checking);
    let error = state.availability_error.take();
    let available_models = std::mem::take(&mut state.available_models);
    *state = AiUi { available, checking, availability_error: error, available_models, ..Default::default() };
}

fn model_ready(app: &PhotocraftApp, model: ComfyUiModel) -> bool {
    // Manually enabled/mock frontends have no probe result; live probes never mark an empty list ready.
    app.ui.ai.available && (app.ui.ai.available_models.is_empty() || app.ui.ai.available_models.contains(&model))
}

fn select_model(app: &mut PhotocraftApp, model: ComfyUiModel) -> Result<(), String> {
    app.run("prefs.set", json!({"path":"integrations.comfyuiModel","value":model.name()}))?;
    crate::prefs_ui::save_preferences(app)
}

fn model_selector(app: &mut PhotocraftApp, ui: &mut egui::Ui, document: u64) {
    let mut selected = app.session.prefs().integrations.comfyui_model;
    let old = selected;
    egui::ComboBox::from_id_salt(("ai-model", document))
        .selected_text(selected.label())
        .width(105.0)
        .show_ui(ui, |ui| {
            for model in ComfyUiModel::ALL {
                ui.add_enabled_ui(model_ready(app, model), |ui| {
                    ui.selectable_value(&mut selected, model, model.label());
                });
            }
        })
        .response
        .on_hover_text(selected.filename());
    if selected != old {
        match select_model(app, selected) {
            Ok(()) => app.ui.ai.focus = app.ui.ai.open,
            Err(error) => {
                app.ui.status = error;
                app.ui.status_error = true;
            }
        }
    }
}

pub fn open(app: &mut PhotocraftApp) -> Result<(), String> {
    let st = app.session.active().ok_or("no document open")?;
    supported_document(&st.doc)?;
    let mode = contextual_mode(&app.session, st);
    sync(&mut app.ui.ai, &st.doc, mode);
    app.ui.ai.open = true;
    app.ui.ai.focus = true;
    Ok(())
}

pub fn set_prompt(app: &mut PhotocraftApp, prompt: String) -> Result<(), String> {
    let st = app.session.active().ok_or("no document open")?;
    supported_document(&st.doc)?;
    let mode = contextual_mode(&app.session, st);
    sync(&mut app.ui.ai, &st.doc, mode);
    app.ui.ai.prompt = prompt;
    Ok(())
}

fn sync(state: &mut AiUi, doc: &photocraft_doc::Document, mode: EditMode) {
    if state.document != Some(doc.id.0) {
        clear_document(state);
        state.document = Some(doc.id.0);
    }
    let signature = selection_signature(doc);
    state.mode = Some(mode);
    if state.signature != Some(signature) {
        state.signature = Some(signature);
        state.selection = doc.selection.clone();
    }
}

fn label(mode: EditMode) -> &'static str {
    match mode {
        EditMode::PromptGenerate => tl!("Prompt Generate"),
        EditMode::PromptEdit => tl!("Prompt Edit"),
        EditMode::GenerativeFill => tl!("Generative Fill"),
    }
}

fn quick_label(mode: EditMode) -> &'static str {
    match mode {
        EditMode::PromptGenerate => "",
        EditMode::PromptEdit => tl!("Remove Background"),
        EditMode::GenerativeFill => tl!("Remove"),
    }
}

fn quick_action(app: &mut PhotocraftApp, mode: EditMode) -> Result<(), String> {
    let result = match mode {
        EditMode::PromptGenerate => Err("enter a prompt to generate an image".into()),
        EditMode::PromptEdit => app.run(photocraft_engine::ai_cmds::REMOVE_BACKGROUND_COMMAND, json!({"name":quick_label(mode)})),
        EditMode::GenerativeFill => app.run(mode.command(), json!({"prompt":"","name":quick_label(mode)})),
    };
    result.map(|_| {
        app.ui.ai.open = false;
    })
}

fn prompt_id(document: u64) -> egui::Id {
    egui::Id::new(("photocraft-ai-prompt", document))
}

/// Reserve Ctrl+Enter before global shortcuts and the multiline editor see it.
pub(crate) fn keys(app: &mut PhotocraftApp, ctx: &egui::Context) -> bool {
    if !app.ui.ai.open || !app.ui.ai.available || app.ui.ai.rect.is_none() || !app.ui.dialogs.is_empty() || app.discard.is_some() {
        return false;
    }
    let Some(doc) = app.session.active() else { return false };
    let id = doc.doc.id.0;
    if ctx.memory(|m| m.has_focus(prompt_id(id))) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter)) {
        app.ui.ai.submit = Some(id);
        return true;
    }
    false
}

pub fn show(app: &mut PhotocraftApp, ui: &egui::Ui, index: usize, canvas: Rect, image: Rect, selection: Option<Rect>) {
    let submitted = app.ui.ai.submit.take();
    if !app.ui.ai.available {
        app.ui.ai.rect = None;
        return;
    }
    let Some(st) = app.session.documents().get(index) else { return };
    let classified = contextual_mode(&app.session, st);
    let doc = st.doc.clone();
    if !app.ui.dialogs.is_empty() || supported_document(&doc).is_err() || canvas.width() < 180.0 || canvas.height() < 100.0 {
        app.ui.ai.rect = None;
        return;
    }
    sync(&mut app.ui.ai, &doc, classified);
    let mode = app.ui.ai.mode.unwrap_or(EditMode::PromptEdit);
    let anchor = if mode == EditMode::GenerativeFill { selection.unwrap_or(image) } else { image };
    // Scrollbar tracks must remain reachable beneath this foreground Area.
    let canvas = canvas.shrink2(vec2(20.0, 20.0));
    let repeat = regeneration(&app.session);
    let width = (if repeat.is_some() { 476.0f32 } else { 440.0f32 }).min(canvas.width() - 16.0);
    let buttons = if mode == EditMode::PromptGenerate { 1.0 } else { 2.0 };
    let button_width = ((width - 16.0 - 115.0 - if repeat.is_some() { 32.0 } else { 0.0 } - ui.spacing().item_spacing.x * (buttons + 1.0)) / buttons).max(1.0);
    let text_width = if app.ui.ai.open { width - 32.0 } else { button_width - 16.0 };
    let button_text =
        ui.painter().layout(label(mode).to_owned(), egui::TextStyle::Button.resolve(ui.style()), crate::theme::Tokens::get(ui.ctx()).text, text_width.max(1.0));
    let quick_text = ui.painter().layout(
        quick_label(mode).to_owned(),
        egui::TextStyle::Button.resolve(ui.style()),
        crate::theme::Tokens::get(ui.ctx()).text,
        text_width.max(1.0),
    );
    let button_height = (button_text.size().y.max(quick_text.size().y) + 8.0).max(24.0);
    let height = if app.ui.ai.open { 128.0 } else { button_height + 16.0 };
    if canvas.height() < height + 16.0 {
        app.ui.ai.rect = None;
        return;
    }
    let x = (anchor.center().x - width * 0.5).clamp(canvas.left() + 8.0, canvas.right() - width - 8.0);
    let y = (anchor.bottom() + 10.0).clamp(canvas.top() + 8.0, (canvas.bottom() - height - 8.0).max(canvas.top() + 8.0));
    let busy = app.session.job_on(doc.id).is_some();
    let ready = model_ready(app, app.session.prefs().integrations.comfyui_model);
    let tokens = crate::theme::Tokens::get(ui.ctx());
    let mut generate = false;
    let mut quick = false;
    let mut close = false;
    let mut reroll = false;
    let area = egui::Area::new(egui::Id::new(("photocraft-ai-bar", doc.id.0))).order(egui::Order::Foreground).fixed_pos(pos2(x, y)).show(ui.ctx(), |ui| {
        egui::Frame::new()
            .fill(tokens.card)
            .stroke(Stroke::new(1.0, tokens.card_border))
            .corner_radius(tokens.radius_sm)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(width - 16.0);
                if !app.ui.ai.open {
                    ui.add_enabled_ui(!busy, |ui| {
                        ui.horizontal(|ui| {
                            model_selector(app, ui, doc.id.0);
                            if ui.add_sized(vec2(button_width, button_height), egui::Button::new(label(mode)).wrap()).clicked() {
                                app.ui.ai.open = true;
                                app.ui.ai.focus = true;
                            }
                            if mode != EditMode::PromptGenerate {
                                quick =
                                    ui.add_enabled(ready, egui::Button::new(quick_label(mode)).min_size(vec2(button_width, button_height)).wrap()).clicked();
                            }
                            if let Some((_, model)) = repeat {
                                ui.add_enabled_ui(model_ready(app, model), |ui| {
                                    reroll =
                                        crate::icons::button(ui, "rotate-cw", 24.0, false, &format!("{} · {}", tl!("Regenerate"), model.label())).clicked();
                                });
                            }
                        })
                    });
                    return;
                }
                let hint = match mode {
                    EditMode::PromptGenerate => tl!("Describe the image to generate"),
                    EditMode::PromptEdit => tl!("Describe changes to the whole image"),
                    EditMode::GenerativeFill => tl!("Describe changes, or leave blank to remove"),
                };
                let response = egui::ScrollArea::vertical()
                    .max_height(60.0)
                    .show(ui, |ui| {
                        ui.add_enabled(
                            !busy,
                            egui::TextEdit::multiline(&mut app.ui.ai.prompt)
                                .id(prompt_id(doc.id.0))
                                .hint_text(hint)
                                .desired_rows(2)
                                .desired_width(f32::INFINITY)
                                .char_limit(MAX_PROMPT_BYTES),
                        )
                    })
                    .inner;
                if app.ui.ai.focus {
                    response.request_focus();
                    app.ui.ai.focus = false;
                }
                let valid = mode == EditMode::GenerativeFill || !app.ui.ai.prompt.trim().is_empty();
                let can_generate = !busy && ready && valid && app.ui.ai.prompt.len() <= MAX_PROMPT_BYTES;
                generate = submitted == Some(doc.id.0) && can_generate;
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy, |ui| model_selector(app, ui, doc.id.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        generate |= ui.add_enabled(can_generate, egui::Button::new(tl!("Generate"))).clicked();
                        close = ui.add_enabled(!busy, egui::Button::new(tl!("Cancel"))).clicked();
                        if let Some((_, model)) = repeat {
                            ui.add_enabled_ui(!busy && model_ready(app, model), |ui| {
                                reroll = crate::icons::button(ui, "rotate-cw", 24.0, false, &format!("{} · {}", tl!("Regenerate"), model.label())).clicked();
                            });
                        }
                    });
                });
            });
    });
    let r = area.response.rect;
    app.ui.ai.rect = Some([r.min.x, r.min.y, r.max.x, r.max.y]);
    if close {
        app.ui.ai.open = false;
    }
    if quick {
        let _ = quick_action(app, mode);
    }
    if reroll && !close {
        if app.run(photocraft_engine::ai_cmds::REGENERATE_COMMAND, json!({})).is_ok() {
            app.ui.ai.open = false;
        }
        return;
    }
    if generate && !close {
        let params = json!({"prompt":app.ui.ai.prompt,"name":label(mode)});
        if app.run(mode.command(), params).is_ok() {
            app.ui.ai.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Services;
    use photocraft_engine::Session;

    #[test]
    fn model_choice_is_written_immediately_and_restored_in_a_new_session() {
        let saved = Arc::new(Mutex::new(String::new()));
        let sink = saved.clone();
        let mut app = PhotocraftApp::new(
            Session::new(),
            Services {
                save_prefs: Some(Box::new(move |text| {
                    *sink.lock().unwrap() = text.into();
                    Ok(())
                })),
                ..Default::default()
            },
        );
        app.run("file.new", json!({"width":32,"height":32})).unwrap();
        let revision = app.session.active().unwrap().revision;
        select_model(&mut app, ComfyUiModel::NoctQv4).unwrap();
        assert_eq!(app.session.active().unwrap().revision, revision);
        let mut reopened = Session::new();
        reopened.load_prefs_json(&saved.lock().unwrap()).unwrap();
        assert_eq!(reopened.prefs().integrations.comfyui_model, ComfyUiModel::NoctQv4);
        let mut older = Session::new();
        older.load_prefs_json(r#"{"integrations":{"comfyuiUrl":"http://127.0.0.1:8188"}}"#).unwrap();
        assert_eq!(older.prefs().integrations.comfyui_model, ComfyUiModel::QwenImage21);
        assert!(reopened.execute("prefs.set", json!({"path":"integrations.comfyuiModel","value":"arbitrary.safetensors"})).is_err());
        app.ui.ai.available = true;
        app.ui.ai.available_models = vec![ComfyUiModel::NoctQv4];
        clear_document(&mut app.ui.ai);
        assert!(model_ready(&app, ComfyUiModel::NoctQv4));
        assert!(!model_ready(&app, ComfyUiModel::QwenImage21));
    }

    struct CaptureGenerate(Arc<Mutex<Vec<bool>>>);
    impl AiBackend for CaptureGenerate {
        fn generate(
            &self,
            request: &photocraft_engine::ai_cmds::AiRequest,
            _: &photocraft_engine::jobs::JobCtx,
        ) -> photocraft_engine::Result<photocraft_raster::Rgba8Image> {
            self.0.lock().unwrap().push(request.removal);
            let mut image = request.image.clone();
            for pixel in image.pixels.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
            Ok(image)
        }
    }

    #[test]
    fn one_click_actions_use_background_preset_and_blank_fill_without_changing_drafts() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut session = Session::new();
        session.ai_backend = Some(Arc::new(CaptureGenerate(calls.clone())));
        let mut app = PhotocraftApp::new(session, Services::default());
        app.run("file.new", json!({"width":20,"height":20})).unwrap();
        app.ui.ai.prompt = "saved draft".into();
        quick_action(&mut app, EditMode::PromptEdit).unwrap();
        assert_eq!(app.session.journal.last().unwrap().0, photocraft_engine::ai_cmds::REMOVE_BACKGROUND_COMMAND);
        assert!(!app.session.active().unwrap().doc.layers.first().unwrap().visible);
        assert_eq!(app.ui.ai.prompt, "saved draft");
        app.run("select.rect", json!({"x":2,"y":2,"width":5,"height":5})).unwrap();
        let old_visible: Vec<_> = app.session.active().unwrap().doc.layers.iter().map(|l| l.visible).collect();
        quick_action(&mut app, EditMode::GenerativeFill).unwrap();
        assert_eq!(app.session.journal.last().unwrap().0, "edit.generativeFill");
        assert_eq!(*calls.lock().unwrap(), vec![false, true]);
        assert_eq!(app.ui.ai.prompt, "saved draft");
        assert_eq!(app.session.active().unwrap().doc.layers.iter().take(old_visible.len()).map(|l| l.visible).collect::<Vec<_>>(), old_visible);
    }

    #[test]
    fn ctrl_enter_submits_only_the_focused_prompt_and_uses_button_validation() {
        use egui_kittest::Harness;
        let calls = Arc::new(Mutex::new(Vec::new()));
        let mut session = Session::new();
        session.ai_backend = Some(Arc::new(CaptureGenerate(calls.clone())));
        let mut app = PhotocraftApp::new(session, Services::default());
        app.run("file.new", json!({"width":20,"height":20})).unwrap();
        app.ui.ai.available = true;
        open(&mut app).unwrap();
        set_prompt(&mut app, "make the sky blue".into()).unwrap();
        let mut h = Harness::builder().with_size(egui::vec2(800.0, 600.0)).build_ui_state(
            |ui, app| {
                if !ui.ctx().fonts(|f| f.families().contains(&egui::FontFamily::Name("medium".into()))) {
                    return;
                }
                crate::shortcuts::handle(app, ui.ctx());
                let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
                let image = egui::Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(400.0, 300.0));
                show(app, ui, 0, canvas, image, None);
            },
            app,
        );
        PhotocraftApp::setup_context(&h.ctx, crate::theme::ThemeKind::ALL[0]);
        h.run_steps(4);
        h.key_press(egui::Key::Enter);
        h.run_steps(4);
        assert!(h.state().ui.ai.prompt.contains('\n'));
        assert!(calls.lock().unwrap().is_empty());
        h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Enter);
        h.run_steps(4);
        assert_eq!(*calls.lock().unwrap(), vec![false]);
        assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 2);
        open(h.state_mut()).unwrap();
        set_prompt(h.state_mut(), "".into()).unwrap();
        h.run_steps(4);
        h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Enter);
        h.run_steps(4);
        assert_eq!(calls.lock().unwrap().len(), 1); // Empty whole-image prompt is disabled.
        h.state_mut().run("select.rect", json!({"x":2,"y":2,"width":5,"height":5})).unwrap();
        open(h.state_mut()).unwrap();
        h.run_steps(4);
        h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Enter);
        h.run_steps(4);
        assert_eq!(*calls.lock().unwrap(), vec![false, true]); // Empty fill still removes.
        assert_eq!(h.state().session.active().unwrap().doc.layers.len(), 3);
        open(h.state_mut()).unwrap();
        h.run_steps(4);
        let id = h.state().session.active().unwrap().doc.id.0;
        h.ctx.memory_mut(|m| m.surrender_focus(prompt_id(id)));
        h.key_press_modifiers(egui::Modifiers::CTRL, egui::Key::Enter);
        h.run_steps(4);
        assert_eq!(calls.lock().unwrap().len(), 2);
    }

    struct FakeAvailability(std::sync::atomic::AtomicBool);
    impl AiBackend for FakeAvailability {
        fn check_available(&self, _: &str) -> photocraft_engine::Result<()> {
            if self.0.load(std::sync::atomic::Ordering::Relaxed) { Ok(()) } else { Err(photocraft_engine::EngineError::Other("missing model".into())) }
        }
        fn generate(
            &self,
            _: &photocraft_engine::ai_cmds::AiRequest,
            _: &photocraft_engine::jobs::JobCtx,
        ) -> photocraft_engine::Result<photocraft_raster::Rgba8Image> {
            panic!("availability must not generate")
        }
    }

    fn finish_probe(app: &mut PhotocraftApp, ctx: &egui::Context) {
        for _ in 0..200 {
            poll_service(app, ctx);
            if !app.ui.ai.checking {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("availability check did not finish");
    }

    #[test]
    fn readiness_survives_document_changes_and_hides_the_bar_on_failure() {
        let backend = Arc::new(FakeAvailability(std::sync::atomic::AtomicBool::new(true)));
        let mut session = Session::new();
        session.ai_backend = Some(backend.clone());
        let mut app = PhotocraftApp::new(session, Services::default());
        let ctx = egui::Context::default();
        poll_service(&mut app, &ctx);
        assert!(!app.ui.ai.available);
        assert!(app.ui.ai.checking);
        finish_probe(&mut app, &ctx);
        assert!(app.ui.ai.available);
        clear_document(&mut app.ui.ai);
        assert!(app.ui.ai.available);
        app.run("file.new", json!({"width":20,"height":20})).unwrap();
        open(&mut app).unwrap();
        assert!(app.ui.ai.available);
        app.run("file.new", json!({"width":30,"height":30})).unwrap();
        open(&mut app).unwrap();
        assert!(app.ui.ai.available);
        backend.0.store(false, std::sync::atomic::Ordering::Relaxed);
        app.ai_probe.next_check = 0.0;
        poll_service(&mut app, &ctx);
        finish_probe(&mut app, &ctx);
        assert!(!app.ui.ai.available);
        assert!(!app.ui.ai.open);
        assert_eq!(app.ui.ai.availability_error.as_deref(), Some("missing model"));
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
            show(&mut app, ui, 0, rect, rect, None);
        });
        output.textures_delta.clear();
        assert!(app.ui.ai.rect.is_none());
        backend.0.store(true, std::sync::atomic::Ordering::Relaxed);
        app.session.edit_prefs(|p| p.integrations.comfyui_url = "http://127.0.0.1:8189".into());
        poll_service(&mut app, &ctx);
        assert!(!app.ui.ai.available);
        finish_probe(&mut app, &ctx);
        assert!(app.ui.ai.available);
        app.session.ai_backend = None;
        poll_service(&mut app, &ctx);
        assert!(!app.ui.ai.available);
        assert!(!app.ui.ai.checking);
    }

    #[test]
    fn mode_tracks_selection_and_open_does_not_mutate_document() {
        let mut app = PhotocraftApp::new(Session::new(), Services::default());
        assert!(open(&mut app).is_err());
        app.run("file.new", json!({"width":20,"height":20})).unwrap();
        let revision = app.session.active().unwrap().revision;
        open(&mut app).unwrap();
        assert_eq!(app.ui.ai.mode, Some(EditMode::PromptGenerate));
        assert_eq!(app.session.active().unwrap().revision, revision);
        app.run("select.rect", json!({"x":2,"y":2,"width":4,"height":4})).unwrap();
        open(&mut app).unwrap();
        assert_eq!(app.ui.ai.mode, Some(EditMode::GenerativeFill));
        app.run("select.all", json!({})).unwrap();
        open(&mut app).unwrap();
        assert_eq!(app.ui.ai.mode, Some(EditMode::PromptGenerate));
    }
}

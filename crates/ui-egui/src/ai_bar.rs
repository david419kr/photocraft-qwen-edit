//! Contextual, native prompt bar. Edits are dispatched through the engine registry.

use egui::{Rect, Stroke, pos2, vec2};
use photocraft_engine::ai_cmds::AiBackend;
use photocraft_engine::ai_cmds::{EditMode, MAX_PROMPT_BYTES, contextual_mode, regeneration, selection_signature, supported_document};
use photocraft_engine::ai_models::{self, Catalog, ModelSettings};
use photocraft_engine::prefs::{ComfyUiModel, ComfyUiTranslationModel};
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
    #[serde(rename = "negativePrompt")]
    pub negative_prompt: String,
    pub cfg: Option<f64>,
    pub catalog: Catalog,
    #[serde(rename = "serviceAvailable")]
    pub service_available: bool,
    /// None uses the selected checkpoint's 25/8 default.
    pub steps: Option<u32>,
    pub mode: Option<EditMode>,
    pub rect: Option<[f32; 4]>,
    pub available: bool,
    pub checking: bool,
    #[serde(rename = "availabilityError")]
    pub availability_error: Option<String>,
    #[serde(rename = "availableModels")]
    pub available_models: Vec<ComfyUiModel>,
    #[serde(rename = "customModels")]
    pub custom_models: Vec<String>,
    #[serde(rename = "modelSettingsOpen")]
    pub model_settings_open: bool,
    #[serde(rename = "modelSettingsText")]
    pub model_settings_text: String,
    pub model_settings_encoders: String,
    pub model_settings_port: u32,
    pub model_settings_configs: std::collections::BTreeMap<String, ModelSettings>,
    pub model_settings_te_tab: bool,
    #[serde(rename = "modelSettingsError")]
    pub model_settings_error: Option<String>,
    #[serde(rename = "translationModels")]
    pub translation_models: Vec<ComfyUiTranslationModel>,
    #[serde(rename = "promptInfoOpen")]
    pub prompt_info_open: bool,
    #[serde(rename = "hasPromptHistory")]
    pub has_prompt_history: bool,
    #[serde(skip)]
    pub(crate) prompt_presence_key: Option<(u64, u64)>,
    #[serde(skip)]
    pub(crate) prompt_records_key: Option<(u64, u64)>,
    #[serde(skip)]
    pub(crate) prompt_records_xmp: Option<String>,
    #[serde(skip)]
    pub(crate) prompt_records: Vec<photocraft_engine::ai_prompts::Entry>,
    #[serde(skip)]
    pub(crate) prompt_info_error: Option<String>,
    #[serde(skip)]
    pub(crate) prompt_info_page: usize,
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
            && self.negative_prompt == other.negative_prompt
            && self.cfg == other.cfg
            && self.catalog == other.catalog
            && self.service_available == other.service_available
            && self.steps == other.steps
            && self.mode == other.mode
            && self.rect == other.rect
            && self.available == other.available
            && self.checking == other.checking
            && self.availability_error == other.availability_error
            && self.available_models == other.available_models
            && self.custom_models == other.custom_models
            && self.model_settings_open == other.model_settings_open
            && self.model_settings_text == other.model_settings_text
            && self.model_settings_encoders == other.model_settings_encoders
            && self.model_settings_port == other.model_settings_port
            && self.model_settings_configs == other.model_settings_configs
            && self.model_settings_te_tab == other.model_settings_te_tab
            && self.model_settings_error == other.model_settings_error
            && self.translation_models == other.translation_models
            && self.prompt_info_open == other.prompt_info_open
            && self.has_prompt_history == other.has_prompt_history
    }
}

#[derive(Default)]
pub(crate) struct ServiceProbe {
    endpoint: String,
    custom_names: String,
    backend: Option<Weak<dyn AiBackend>>,
    next_check: f64,
    pending: Option<ProbeSlot>,
}

struct Availability {
    image: Vec<ComfyUiModel>,
    custom: Vec<String>,
    catalog: Catalog,
    translation: Vec<ComfyUiTranslationModel>,
}
type ProbeSlot = Arc<Mutex<Option<Result<Availability, String>>>>;

fn probe(backend: &dyn AiBackend, endpoint: &str, config: &photocraft_engine::prefs::Integrations) -> Result<Availability, String> {
    let catalog = backend.model_catalog(endpoint, config).map_err(|e| e.to_string())?;
    let mut image = Vec::new();
    let mut custom = Vec::new();
    for name in &catalog.models {
        if ai_models::ready(config, &catalog, name) {
            if let Some(model) = ai_models::builtin(name) {
                image.push(model);
            } else {
                custom.push(name.clone());
            }
        }
    }
    if !custom.is_empty() {
        image.push(ComfyUiModel::Custom);
    }
    let translation = backend.available_translation_models(endpoint).unwrap_or_default();
    Ok(Availability { image, custom, catalog, translation })
}

/// One bounded, read-only background check; never wait for HTTP on the UI thread.
pub(crate) fn poll_service(app: &mut PhotocraftApp, ctx: &egui::Context) {
    let Some(backend) = app.session.ai_backend.clone() else {
        app.ai_probe = ServiceProbe::default();
        app.ui.ai.available = false;
        app.ui.ai.service_available = false;
        app.ui.ai.checking = false;
        app.ui.ai.availability_error = None;
        app.ui.ai.catalog = Catalog::default();
        app.ui.ai.available_models.clear();
        app.ui.ai.custom_models.clear();
        app.ui.ai.translation_models.clear();
        return;
    };
    let endpoint = app.session.prefs().integrations.comfyui_url.clone();
    let config = app.session.prefs().integrations.clone();
    let custom_names = format!("{}|{}|{:?}", config.comfyui_custom_models, config.comfyui_custom_encoders, config.comfyui_model_settings);
    let identity = Arc::downgrade(&backend);
    if app.ai_probe.endpoint != endpoint || app.ai_probe.custom_names != custom_names || app.ai_probe.backend.as_ref().is_none_or(|old| !old.ptr_eq(&identity))
    {
        app.ai_probe = ServiceProbe { endpoint: endpoint.clone(), custom_names: custom_names.clone(), backend: Some(identity), ..Default::default() };
        app.ui.ai.available = false;
        app.ui.ai.service_available = false;
        app.ui.ai.checking = false;
        app.ui.ai.availability_error = None;
        app.ui.ai.catalog = Catalog::default();
        app.ui.ai.available_models.clear();
        app.ui.ai.custom_models.clear();
        app.ui.ai.translation_models.clear();
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
            Ok(Availability { image: models, custom, catalog, translation }) => {
                let single = match models.as_slice() {
                    [model] => Some(*model),
                    _ => None,
                };
                app.ui.ai.available = !models.is_empty();
                app.ui.ai.available_models = models;
                app.ui.ai.service_available = true;
                app.ui.ai.catalog = catalog;
                app.ui.ai.custom_models = custom;
                app.ui.ai.availability_error = None;
                let saved = app.session.prefs().integrations.comfyui_translation_model;
                let next = (!translation.contains(&saved)).then(|| translation.first().copied()).flatten();
                app.ui.ai.translation_models = translation;
                if let Some(model) = next
                    && let Err(error) = set_translation_pref(app, "integrations.comfyuiTranslationModel", json!(model.name()))
                {
                    app.ui.status = error;
                    app.ui.status_error = true;
                }
                if single == Some(ComfyUiModel::Custom) && app.ui.ai.custom_models.len() == 1 {
                    if let Some(name) = app.ui.ai.custom_models.first().cloned()
                        && (app.session.prefs().integrations.comfyui_model != ComfyUiModel::Custom
                            || app.session.prefs().integrations.comfyui_custom_model != name)
                        && let Err(error) = select_custom_model(app, &name)
                    {
                        app.ui.status = error;
                        app.ui.status_error = true;
                    }
                } else if let Some(model) = single.filter(|model| *model != ComfyUiModel::Custom)
                    && model != app.session.prefs().integrations.comfyui_model
                    && let Err(error) = select_model(app, model)
                {
                    app.ui.status = error;
                    app.ui.status_error = true;
                }
            }
            Err(error) => {
                app.ui.ai.available = false;
                app.ui.ai.service_available = false;
                app.ui.ai.catalog = Catalog::default();
                app.ui.ai.available_models.clear();
                app.ui.ai.custom_models.clear();
                app.ui.ai.translation_models.clear();
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
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| probe(backend.as_ref(), &endpoint, &config)))
                    .map_err(|_| "AI availability check failed".to_owned())
                    .and_then(|r| r);
                *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(result);
                repaint.request_repaint();
            });
            if let Err(error) = spawned {
                app.ai_probe.pending = None;
                app.ai_probe.next_check = now + 15.0;
                app.ui.ai.checking = false;
                app.ui.ai.available = false;
                app.ui.ai.service_available = false;
                app.ui.ai.availability_error = Some(error.to_string());
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            *slot.lock().unwrap_or_else(PoisonError::into_inner) = Some(probe(backend.as_ref(), &endpoint, &config));
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
    let catalog = std::mem::take(&mut state.catalog);
    let service_available = state.service_available;
    let custom_models = std::mem::take(&mut state.custom_models);
    let translation_models = std::mem::take(&mut state.translation_models);
    *state = AiUi {
        available,
        checking,
        availability_error: error,
        available_models,
        custom_models,
        translation_models,
        catalog,
        service_available,
        ..Default::default()
    };
}

fn set_translation_pref(app: &mut PhotocraftApp, path: &str, value: serde_json::Value) -> Result<(), String> {
    let old = app.session.prefs().integrations.comfyui_translation_model;
    app.run("prefs.set", json!({"path":path,"value":value}))?;
    crate::prefs_ui::save_preferences(app)?;
    if old != app.session.prefs().integrations.comfyui_translation_model {
        unload_models(app)?;
    }
    Ok(())
}

fn unload_models(app: &mut PhotocraftApp) -> Result<(), String> {
    if app.session.jobs().iter().any(|job| job.command == photocraft_engine::ai_cmds::UNLOAD_MODELS_COMMAND) {
        return Ok(());
    }
    if app.session.ai_backend.is_some() {
        app.run(photocraft_engine::ai_cmds::UNLOAD_MODELS_COMMAND, json!({}))?;
    }
    Ok(())
}

fn translation_controls(app: &mut PhotocraftApp, ui: &mut egui::Ui, document: u64) {
    if app.ui.ai.translation_models.is_empty() {
        return;
    }
    let enabled = app.session.prefs().integrations.comfyui_translate_prompt;
    let mut change = None;
    let toggle = crate::icons::button(ui, "languages", 24.0, enabled, tl!("Translate prompt to English"));
    toggle.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), enabled, tl!("Translate prompt to English")));
    if toggle.clicked() {
        change = Some(("integrations.comfyuiTranslatePrompt", json!(!enabled)));
    }
    if app.ui.ai.translation_models.len() > 1 {
        let mut selected = app.session.prefs().integrations.comfyui_translation_model;
        let old = selected;
        egui::ComboBox::from_id_salt(("ai-translation-model", document))
            .selected_text(selected.label().trim_start_matches("Hy-MT2 "))
            .width(58.0)
            .show_ui(ui, |ui| {
                for model in &app.ui.ai.translation_models {
                    ui.selectable_value(&mut selected, *model, model.label());
                }
            })
            .response
            .on_hover_text(format!("{} · {}", tl!("Prompt translation model"), selected.filename()));
        if selected != old {
            change = Some(("integrations.comfyuiTranslationModel", json!(selected.name())));
        }
    }
    if let Some((path, value)) = change
        && let Err(error) = set_translation_pref(app, path, value)
    {
        app.ui.status = error;
        app.ui.status_error = true;
    }
}

fn model_ready(app: &PhotocraftApp, model: ComfyUiModel) -> bool {
    if model == ComfyUiModel::Custom && app.ui.ai.catalog.models.is_empty() {
        return app.ui.ai.available && app.ui.ai.custom_models.contains(&app.session.prefs().integrations.comfyui_custom_model);
    }
    let name = if model == ComfyUiModel::Custom { app.session.prefs().integrations.comfyui_custom_model.as_str() } else { model.filename() };
    if !app.ui.ai.catalog.models.is_empty() {
        return ai_models::ready(&app.session.prefs().integrations, &app.ui.ai.catalog, name);
    }
    // Manually enabled/mock frontends have no probe result; live probes never mark an empty list ready.
    app.ui.ai.available && (app.ui.ai.available_models.is_empty() || app.ui.ai.available_models.contains(&model))
}

fn select_model(app: &mut PhotocraftApp, model: ComfyUiModel) -> Result<(), String> {
    let changed = model != app.session.prefs().integrations.comfyui_model;
    app.run("prefs.set", json!({"path":"integrations.comfyuiModel","value":model.name()}))?;
    app.ui.ai.steps = None;
    app.ui.ai.cfg = None;
    crate::prefs_ui::save_preferences(app)?;
    if changed {
        unload_models(app)?;
    }
    Ok(())
}

fn select_custom_model(app: &mut PhotocraftApp, name: &str) -> Result<(), String> {
    let changed = app.session.prefs().integrations.comfyui_model == ComfyUiModel::Custom && app.session.prefs().integrations.comfyui_custom_model != name;
    app.run("prefs.set", json!({"path":"integrations.comfyuiCustomModel","value":name}))?;
    select_model(app, ComfyUiModel::Custom)?;
    if changed {
        unload_models(app)?;
    }
    Ok(())
}

fn model_count(app: &PhotocraftApp) -> usize {
    if !app.ui.ai.catalog.models.is_empty() {
        return app.ui.ai.catalog.models.len();
    }
    app.ui.ai.available_models.iter().filter(|model| **model != ComfyUiModel::Custom).count() + app.ui.ai.custom_models.len()
}

pub(crate) fn open_model_settings(app: &mut PhotocraftApp) {
    let config = &app.session.prefs().integrations;
    app.ui.ai.model_settings_text = config.comfyui_custom_models.clone();
    app.ui.ai.model_settings_encoders = config.comfyui_custom_encoders.clone();
    app.ui.ai.model_settings_configs = config.comfyui_model_settings.clone();
    #[cfg(not(target_arch = "wasm32"))]
    {
        app.ui.ai.model_settings_port = endpoint_port(&config.comfyui_url);
    }
    app.ui.ai.model_settings_error = None;
    app.ui.ai.model_settings_open = true;
}

fn model_settings_button(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    if crate::icons::button(ui, "settings", 24.0, app.ui.ai.model_settings_open, tl!("AI model settings")).clicked() {
        open_model_settings(app);
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn endpoint_port(endpoint: &str) -> u32 {
    photocraft_engine::comfyui::endpoint_url(endpoint).ok().and_then(|url| url.port_or_known_default()).map_or(8188, u32::from)
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn with_endpoint_port(endpoint: &str, port: u32) -> Result<String, String> {
    let port = u16::try_from(port).ok().filter(|port| *port != 0).ok_or("Port must be within 1..65535")?;
    let mut url = photocraft_engine::comfyui::endpoint_url(endpoint).map_err(|error| error.to_string())?;
    if url.port_or_known_default() == Some(port) {
        return Ok(endpoint.to_owned());
    }
    url.set_port(Some(port)).map_err(|_| "Invalid server port".to_owned())?;
    Ok(url.to_string())
}

fn model_settings_window(app: &mut PhotocraftApp, ctx: &egui::Context) {
    if !app.ui.ai.model_settings_open {
        return;
    }
    let mut open = true;
    let mut save = false;
    let encoders = photocraft_engine::prefs::custom_model_names(&app.ui.ai.model_settings_encoders).unwrap_or_default();
    let custom = photocraft_engine::prefs::custom_model_names(&app.ui.ai.model_settings_text).unwrap_or_default();
    let mut remove = None;
    egui::Window::new(tl!("AI model settings")).id(egui::Id::new("ai-model-settings")).open(&mut open).default_width(800.0).show(ctx, |ui| {
        #[cfg(not(target_arch = "wasm32"))]
        ui.horizontal(|ui| {
            ui.label(tl!("ComfyUI port"));
            crate::widgets::integer_value_field(ui, &mut app.ui.ai.model_settings_port, 1..=65535, 70.0);
        });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.ui.ai.model_settings_te_tab, false, tl!("Models"));
            ui.selectable_value(&mut app.ui.ai.model_settings_te_tab, true, tl!("Text encoders"));
        });
        ui.separator();
        if app.ui.ai.model_settings_te_tab {
            if ai_models::resolve_encoder(Some(ai_models::DEFAULT_ENCODER), &app.ui.ai.catalog.encoders).is_ok() {
                ui.horizontal(|ui| {
                    ui.add(crate::icons::image("lock", 16.0, crate::theme::Tokens::get(ctx).text));
                    ui.label(ai_models::DEFAULT_ENCODER);
                });
            }
            ui.label(tl!("One .safetensors filename per line. Compatibility is not checked."));
            ui.add(egui::TextEdit::multiline(&mut app.ui.ai.model_settings_encoders).desired_rows(7).desired_width(f32::INFINITY));
        } else {
            let mut names: Vec<_> = ComfyUiModel::ALL
                .into_iter()
                .filter(|m| app.ui.ai.catalog.models.contains(&m.filename().to_owned()))
                .map(|m| m.filename().to_owned())
                .collect();
            names.extend(custom.iter().filter(|name| ai_models::builtin(name).is_none()).cloned());
            egui::ScrollArea::horizontal().show(ui, |ui| {
                egui::Grid::new("ai-model-config-grid").striped(true).show(ui, |ui| {
                    for label in ["Filename", "Text encoder", "CFG", "Steps", "Sampling profile", ""] {
                        ui.label(tl!(label));
                    }
                    ui.end_row();
                    for name in names {
                        let pinned = ai_models::builtin(&name);
                        let defaults = app.session.prefs().integrations.settings_for(&name);
                        let config = app.ui.ai.model_settings_configs.entry(name.clone()).or_insert(defaults);
                        ui.horizontal(|ui| {
                            if pinned.is_some() {
                                ui.add(crate::icons::image("lock", 14.0, crate::theme::Tokens::get(ctx).text));
                            }
                            ui.add_sized(vec2(175.0, 24.0), egui::Label::new(&name).truncate()).on_hover_text(&name);
                        });
                        egui::ComboBox::from_id_salt(("model-te", &name))
                            .selected_text(config.text_encoder.as_deref().unwrap_or("Default"))
                            .width(150.0)
                            .truncate()
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut config.text_encoder, None, "Default");
                                for encoder in &encoders {
                                    ui.selectable_value(&mut config.text_encoder, Some(encoder.clone()), encoder);
                                }
                            });
                        let mut cfg = config.cfg as f32;
                        let fixed = pinned == Some(ComfyUiModel::TurboInt8);
                        if fixed {
                            cfg = 1.0;
                        }
                        ui.add_enabled_ui(!fixed, |ui| {
                            crate::widgets::value_field_prec(ui, &mut cfg, 1.0..=100.0, "", 48.0, 2);
                        });
                        config.cfg = f64::from(cfg);
                        crate::widgets::integer_value_field(ui, &mut config.steps, 1..=1000, 48.0);
                        if let Some(model) = pinned {
                            config.turbo = model.is_turbo();
                            ui.label(if config.turbo { "Turbo" } else { tl!("Normal") });
                        } else {
                            crate::widgets::checkbox(ui, &mut config.turbo, "Turbo");
                        }
                        if pinned.is_none() && crate::icons::button(ui, "trash", 20.0, false, tl!("Delete")).clicked() {
                            remove = Some(name.clone());
                        }
                        ui.end_row();
                    }
                });
            });
            ui.separator();
            ui.label(tl!("Custom model filenames (one per line)"));
            ui.add(egui::TextEdit::multiline(&mut app.ui.ai.model_settings_text).desired_rows(3).desired_width(f32::INFINITY));
        }
        if let Some(error) = &app.ui.ai.model_settings_error {
            ui.label(error);
        }
        save = ui.button(tl!("Save")).clicked();
    });
    if let Some(name) = remove {
        app.ui.ai.model_settings_text = custom.into_iter().filter(|value| value != &name).collect::<Vec<_>>().join("\n");
        app.ui.ai.model_settings_configs.remove(&name);
    }
    app.ui.ai.model_settings_open = open;
    if save {
        match save_model_settings(app) {
            Ok(()) => {
                app.ui.ai.model_settings_open = false;
                app.ui.ai.steps = None;
                app.ui.ai.cfg = None;
            }
            Err(error) => app.ui.ai.model_settings_error = Some(error),
        }
    }
}

fn save_model_settings(app: &mut PhotocraftApp) -> Result<(), String> {
    let models = photocraft_engine::prefs::custom_model_names(&app.ui.ai.model_settings_text)?;
    let encoders = photocraft_engine::prefs::custom_model_names(&app.ui.ai.model_settings_encoders)?;
    let mut config = app.session.prefs().integrations.clone();
    #[cfg(not(target_arch = "wasm32"))]
    {
        config.comfyui_url = with_endpoint_port(&config.comfyui_url, app.ui.ai.model_settings_port)?;
    }
    config.comfyui_custom_models = models.iter().filter(|name| ai_models::builtin(name).is_none()).cloned().collect::<Vec<_>>().join("\n");
    config.comfyui_custom_encoders = encoders.join("\n");
    config.comfyui_model_settings = app.ui.ai.model_settings_configs.clone();
    config.comfyui_model_settings.retain(|name, _| ai_models::builtin(name).is_some() || models.contains(name));
    for settings in config.comfyui_model_settings.values_mut() {
        if settings.text_encoder.as_ref().is_some_and(|name| name != ai_models::DEFAULT_ENCODER && !encoders.contains(name)) {
            settings.text_encoder = None;
        }
        settings.validate()?;
    }
    if config.comfyui_model == ComfyUiModel::Custom && !models.contains(&config.comfyui_custom_model) {
        config.comfyui_model = ComfyUiModel::QwenImage21;
        config.comfyui_custom_model.clear();
    }
    app.run("prefs.set", json!({"path":"integrations","value":config}))?;
    crate::prefs_ui::save_preferences(app)
}

fn current_cfg(app: &PhotocraftApp) -> f64 {
    if app.session.prefs().integrations.comfyui_model == ComfyUiModel::TurboInt8 {
        1.0
    } else {
        app.ui.ai.cfg.unwrap_or(app.session.prefs().integrations.selected_settings().cfg)
    }
}

fn cfg_controls(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let mut cfg = current_cfg(app) as f32;
    ui.add_enabled_ui(app.session.prefs().integrations.comfyui_model != ComfyUiModel::TurboInt8, |ui| {
        if crate::widgets::value_field_prec(ui, &mut cfg, 1.0..=100.0, "", 42.0, 2).on_hover_text("CFG").changed() {
            app.ui.ai.cfg = Some(f64::from(cfg));
        }
    });
}

fn negative_id(document: u64) -> egui::Id {
    egui::Id::new(("photocraft-ai-negative", document))
}

fn model_selector(app: &mut PhotocraftApp, ui: &mut egui::Ui, document: u64) {
    if model_count(app) == 1 {
        return;
    }
    let selected_model = app.session.prefs().integrations.comfyui_model;
    let mut selected =
        if selected_model == ComfyUiModel::Custom { app.session.prefs().integrations.comfyui_custom_model.clone() } else { selected_model.name().to_owned() };
    let old = selected.clone();
    egui::ComboBox::from_id_salt(("ai-model", document))
        .selected_text(if selected_model == ComfyUiModel::Custom { selected.trim_end_matches(".safetensors") } else { selected_model.label() })
        .width(135.0)
        .truncate()
        .show_ui(ui, |ui| {
            let models = if app.ui.ai.catalog.models.is_empty() {
                app.ui.ai.available_models.clone()
            } else {
                ComfyUiModel::ALL.into_iter().filter(|model| app.ui.ai.catalog.models.contains(&model.filename().to_owned())).collect()
            };
            for model in models.into_iter().filter(|model| *model != ComfyUiModel::Custom) {
                ui.add_enabled_ui(model_ready(app, model), |ui| {
                    ui.selectable_value(&mut selected, model.name().to_owned(), model.label());
                });
            }
            let custom: Vec<_> = if app.ui.ai.catalog.models.is_empty() {
                app.ui.ai.custom_models.clone()
            } else {
                app.ui.ai.catalog.models.iter().filter(|name| ai_models::builtin(name).is_none()).cloned().collect()
            };
            for name in &custom {
                ui.selectable_value(&mut selected, name.clone(), name.trim_end_matches(".safetensors"));
            }
        })
        .response
        .on_hover_text(if selected_model == ComfyUiModel::Custom { old.clone() } else { selected_model.filename().to_owned() });
    if selected != old {
        let result = match ComfyUiModel::parse(&selected) {
            Some(model) => select_model(app, model),
            None => select_custom_model(app, &selected),
        };
        match result {
            Ok(()) => app.ui.ai.focus = app.ui.ai.open,
            Err(error) => {
                app.ui.status = error;
                app.ui.status_error = true;
            }
        }
    }
}

fn step_controls(app: &mut PhotocraftApp, ui: &mut egui::Ui) {
    let default = app.session.prefs().integrations.selected_settings().steps;
    let mut steps = app.ui.ai.steps.unwrap_or(default);
    if crate::widgets::integer_value_field(ui, &mut steps, 1..=photocraft_engine::ai_cmds::MAX_STEPS, 38.0).on_hover_text(tl!("Sampling steps")).changed() {
        app.ui.ai.steps = Some(steps);
    }
    if crate::icons::button(ui, "rotate-cw", 16.0, false, &format!("{} ({default})", tl!("Reset steps to model default"))).clicked() {
        app.ui.ai.steps = None;
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

pub fn set_steps(app: &mut PhotocraftApp, steps: Option<u32>) -> Result<(), String> {
    let st = app.session.active().ok_or("no document open")?;
    supported_document(&st.doc)?;
    let mode = contextual_mode(&app.session, st);
    sync(&mut app.ui.ai, &st.doc, mode);
    app.ui.ai.steps = steps;
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
        EditMode::PromptEdit => app.run(
            photocraft_engine::ai_cmds::REMOVE_BACKGROUND_COMMAND,
            json!({"name":quick_label(mode),"cfg":current_cfg(app),"negativePrompt":app.ui.ai.negative_prompt,"steps":app.ui.ai.steps.unwrap_or(app.session.prefs().integrations.selected_settings().steps)}),
        ),
        EditMode::GenerativeFill => app.run(
            mode.command(),
            json!({"prompt":"","name":quick_label(mode),"cfg":current_cfg(app),"negativePrompt":app.ui.ai.negative_prompt,"steps":app.ui.ai.steps.unwrap_or(app.session.prefs().integrations.selected_settings().steps)}),
        ),
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
    if !app.ui.ai.open
        || !app.ui.ai.available
        || app.ui.ai.model_settings_open
        || app.ui.ai.rect.is_none()
        || !app.ui.dialogs.is_empty()
        || app.discard.is_some()
    {
        return false;
    }
    let Some(doc) = app.session.active() else { return false };
    let id = doc.doc.id.0;
    if ctx.memory(|m| m.has_focus(prompt_id(id)) || m.has_focus(negative_id(id))) && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Enter)) {
        app.ui.ai.submit = Some(id);
        return true;
    }
    false
}

pub fn show(app: &mut PhotocraftApp, ui: &egui::Ui, index: usize, canvas: Rect, image: Rect, selection: Option<Rect>) {
    model_settings_window(app, ui.ctx());
    let submitted = app.ui.ai.submit.take();
    let Some(st) = app.session.documents().get(index) else { return };
    let doc = st.doc.clone();
    let revision = st.revision;
    let has_history = crate::ai_prompt_info::has_history(app, &doc, revision);
    if !app.ui.ai.available || supported_document(&doc).is_err() {
        app.ui.ai.rect = None;
        if app.ui.ai.service_available {
            let area = egui::Area::new(egui::Id::new(("ai-settings-only", doc.id.0)))
                .order(egui::Order::Foreground)
                .fixed_pos(pos2(image.center().x, (image.bottom() + 10.0).min(canvas.bottom() - 40.0)))
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        model_settings_button(app, ui);
                        if has_history {
                            crate::ai_prompt_info::button(app, ui);
                        }
                    });
                });
            let rect = area.response.rect;
            app.ui.ai.rect = Some([rect.min.x, rect.min.y, rect.max.x, rect.max.y]);
        } else if has_history {
            crate::ai_prompt_info::only_bar(app, ui.ctx(), doc.id.0, canvas, image);
        }
        return;
    }
    let Some(st) = app.session.documents().get(index) else { return };
    let classified = contextual_mode(&app.session, st);
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
    let translation_width = if !app.ui.ai.open || app.ui.ai.translation_models.is_empty() {
        0.0
    } else if app.ui.ai.translation_models.len() > 1 {
        64.0
    } else {
        24.0
    };
    let cfg = current_cfg(app);
    let width = ((if repeat.is_some() { 576.0f32 } else { 540.0f32 }) + translation_width + 112.0).min(canvas.width() - 16.0);
    let buttons = if mode == EditMode::PromptGenerate { 1.0 } else { 2.0 };
    let model_width = if model_count(app) == 1 { 0.0 } else { 145.0 };
    let button_width =
        ((width - 128.0 - model_width - 80.0 - if repeat.is_some() { 32.0 } else { 0.0 } - ui.spacing().item_spacing.x * (buttons + 3.0)) / buttons).max(1.0);
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
    let height = if app.ui.ai.open { if cfg > 1.0 { 184.0 } else { 128.0 } } else { button_height + 16.0 };
    if canvas.height() < height + 16.0 {
        app.ui.ai.rect = None;
        return;
    }
    let x = (anchor.center().x - width * 0.5).clamp(canvas.left() + 8.0, canvas.right() - width - 8.0);
    let y = (anchor.bottom() + 10.0).clamp(canvas.top() + 8.0, (canvas.bottom() - height - 8.0).max(canvas.top() + 8.0));
    let busy = app.session.job_on(doc.id).is_some() || app.session.jobs().iter().any(|job| job.command == photocraft_engine::ai_cmds::UNLOAD_MODELS_COMMAND);
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
                            model_settings_button(app, ui);
                            model_selector(app, ui, doc.id.0);
                            cfg_controls(app, ui);
                            step_controls(app, ui);
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
                            crate::ai_prompt_info::button(app, ui);
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
                if cfg > 1.0 {
                    egui::ScrollArea::vertical().id_salt("ai-negative-scroll").max_height(60.0).show(ui, |ui| {
                        ui.add_enabled(
                            !busy,
                            egui::TextEdit::multiline(&mut app.ui.ai.negative_prompt)
                                .id(negative_id(doc.id.0))
                                .hint_text(tl!("Negative prompt"))
                                .desired_rows(1)
                                .desired_width(f32::INFINITY)
                                .char_limit(MAX_PROMPT_BYTES),
                        );
                    });
                }
                let valid = mode == EditMode::GenerativeFill || !app.ui.ai.prompt.trim().is_empty();
                let can_generate = !busy && ready && valid && app.ui.ai.prompt.len() <= MAX_PROMPT_BYTES && app.ui.ai.negative_prompt.len() <= MAX_PROMPT_BYTES;
                generate = submitted == Some(doc.id.0) && can_generate;
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy, |ui| model_settings_button(app, ui));
                    ui.add_enabled_ui(!busy, |ui| model_selector(app, ui, doc.id.0));
                    ui.add_enabled_ui(!busy, |ui| cfg_controls(app, ui));
                    ui.add_enabled_ui(!busy, |ui| step_controls(app, ui));
                    ui.add_enabled_ui(!busy, |ui| translation_controls(app, ui, doc.id.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        crate::ai_prompt_info::button(app, ui);
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
        let params = json!({"prompt":app.ui.ai.prompt,"name":label(mode),"cfg":current_cfg(app),"negativePrompt":app.ui.ai.negative_prompt,"steps":app.ui.ai.steps.unwrap_or(app.session.prefs().integrations.selected_settings().steps)});
        if app.run(mode.command(), params).is_ok() {
            app.ui.ai.open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn comfy_port_updates_preserve_host_path_and_default_and_reject_invalid_ports() {
        assert_eq!(endpoint_port("http://127.0.0.1:8188"), 8188);
        assert_eq!(with_endpoint_port("http://127.0.0.1:8188", 8188).unwrap(), "http://127.0.0.1:8188");
        assert_eq!(with_endpoint_port("http://localhost:8188/comfy/", 8190).unwrap(), "http://localhost:8190/comfy/");
        assert_eq!(with_endpoint_port("https://[::1]:8188/", 8190).unwrap(), "https://[::1]:8190/");
        assert!(with_endpoint_port("http://127.0.0.1:8188", 0).is_err());
        assert!(with_endpoint_port("http://127.0.0.1:8188", 65536).is_err());
    }
    #[test]
    fn prompt_translation_preferences_persist_and_detection_survives_document_reset() {
        use super::*;
        use crate::Services;
        use photocraft_engine::Session;
        let saved = Arc::new(Mutex::new(String::new()));
        let sink = saved.clone();
        let mut app = PhotocraftApp::new(
            Session::new(),
            Services {
                save_prefs: Some(Box::new(move |text| {
                    *sink.lock().unwrap() = text.to_owned();
                    Ok(())
                })),
                ..Default::default()
            },
        );
        set_translation_pref(&mut app, "integrations.comfyuiTranslatePrompt", json!(true)).unwrap();
        set_translation_pref(&mut app, "integrations.comfyuiTranslationModel", json!(ComfyUiTranslationModel::HyMt7Abliterated.name())).unwrap();
        let mut restored = Session::new();
        restored.load_prefs_json(&saved.lock().unwrap()).unwrap();
        assert!(restored.prefs().integrations.comfyui_translate_prompt);
        assert_eq!(restored.prefs().integrations.comfyui_translation_model, ComfyUiTranslationModel::HyMt7Abliterated);
        app.ui.ai.translation_models = ComfyUiTranslationModel::ALL.to_vec();
        clear_document(&mut app.ui.ai);
        assert_eq!(app.ui.ai.translation_models, ComfyUiTranslationModel::ALL);
        assert!(app.session.prefs().integrations.comfyui_translate_prompt);
    }
    use super::*;
    use crate::Services;
    use photocraft_engine::Session;

    #[test]
    fn model_selectors_unload_once_on_change_but_not_on_toggle_or_reselection() {
        struct Recorder(Arc<Mutex<usize>>);
        impl photocraft_engine::ai_cmds::AiBackend for Recorder {
            fn unload_models(&self, _: &str, _: &photocraft_engine::jobs::JobCtx) -> photocraft_engine::Result<()> {
                *self.0.lock().unwrap() += 1;
                Ok(())
            }
            fn generate(
                &self,
                _: &photocraft_engine::ai_cmds::AiRequest,
                _: &photocraft_engine::jobs::JobCtx,
            ) -> photocraft_engine::Result<photocraft_raster::Rgba8Image> {
                Err(photocraft_engine::EngineError::Other("unused".into()))
            }
        }
        let calls = Arc::new(Mutex::new(0));
        let mut session = Session::new();
        session.ai_backend = Some(Arc::new(Recorder(calls.clone())));
        session.edit_prefs(|prefs| prefs.integrations.comfyui_custom_models = "first.safetensors\nsecond.safetensors".into());
        let mut app = PhotocraftApp::new(session, Services::default());
        select_model(&mut app, ComfyUiModel::QwenImage21).unwrap();
        select_model(&mut app, ComfyUiModel::TurboInt8).unwrap();
        select_model(&mut app, ComfyUiModel::TurboInt8).unwrap();
        select_custom_model(&mut app, "first.safetensors").unwrap();
        select_custom_model(&mut app, "second.safetensors").unwrap();
        select_custom_model(&mut app, "second.safetensors").unwrap();
        set_translation_pref(&mut app, "integrations.comfyuiTranslationModel", json!(ComfyUiTranslationModel::HyMt7Abliterated.name())).unwrap();
        set_translation_pref(&mut app, "integrations.comfyuiTranslatePrompt", json!(true)).unwrap();
        assert_eq!(*calls.lock().unwrap(), 4);
        assert!(app.session.execute(photocraft_engine::ai_cmds::UNLOAD_MODELS_COMMAND, json!({"unexpected":true})).is_err());
    }

    #[test]
    fn model_settings_save_profiles_encoders_and_preserve_negative_draft_on_cfg_change() {
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
        open_model_settings(&mut app);
        app.ui.ai.model_settings_text = "mine.safetensors".into();
        app.ui.ai.model_settings_encoders = "te.safetensors".into();
        app.ui
            .ai
            .model_settings_configs
            .insert("mine.safetensors".into(), ModelSettings { text_encoder: Some("te.safetensors".into()), cfg: 3.5, steps: 17, turbo: true });
        save_model_settings(&mut app).unwrap();
        select_custom_model(&mut app, "mine.safetensors").unwrap();
        assert_eq!(current_cfg(&app), 3.5);
        app.ui.ai.negative_prompt = "blur".into();
        app.ui.ai.cfg = Some(1.0);
        assert_eq!(current_cfg(&app), 1.0);
        assert_eq!(app.ui.ai.negative_prompt, "blur");
        let mut restored = Session::new();
        restored.load_prefs_json(&saved.lock().unwrap()).unwrap();
        assert_eq!(restored.prefs().integrations.selected_settings().steps, 17);
        assert_eq!(restored.prefs().integrations.selected_settings().text_encoder.as_deref(), Some("te.safetensors"));
        select_model(&mut app, ComfyUiModel::TurboInt8).unwrap();
        app.ui.ai.cfg = Some(4.0);
        assert_eq!(current_cfg(&app), 1.0);
    }

    #[test]
    fn custom_model_selection_persists_and_counts_individual_files() {
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
        set_translation_pref(&mut app, "integrations.comfyuiCustomModels", json!("first.safetensors\nsecond.safetensors")).unwrap();
        select_custom_model(&mut app, "second.safetensors").unwrap();
        app.ui.ai.available = true;
        app.ui.ai.available_models = vec![ComfyUiModel::Custom];
        app.ui.ai.custom_models = vec!["first.safetensors".into(), "second.safetensors".into()];
        assert_eq!(model_count(&app), 2);
        assert!(model_ready(&app, ComfyUiModel::Custom));
        let mut reopened = Session::new();
        reopened.load_prefs_json(&saved.lock().unwrap()).unwrap();
        assert_eq!(reopened.prefs().integrations.comfyui_custom_model, "second.safetensors");
        assert_eq!(reopened.prefs().integrations.comfyui_model, ComfyUiModel::Custom);
        app.ui.ai.custom_models.pop();
        assert_eq!(model_count(&app), 1);
        assert!(!model_ready(&app, ComfyUiModel::Custom));
        clear_document(&mut app.ui.ai);
        assert_eq!(app.ui.ai.custom_models, ["first.safetensors"]);
    }

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
        select_model(&mut app, ComfyUiModel::TurboInt8).unwrap();
        assert_eq!(app.session.active().unwrap().revision, revision);
        let mut reopened = Session::new();
        reopened.load_prefs_json(&saved.lock().unwrap()).unwrap();
        assert_eq!(reopened.prefs().integrations.comfyui_model, ComfyUiModel::TurboInt8);
        let mut older = Session::new();
        older.load_prefs_json(r#"{"integrations":{"comfyuiUrl":"http://127.0.0.1:8188"}}"#).unwrap();
        assert_eq!(older.prefs().integrations.comfyui_model, ComfyUiModel::QwenImage21);
        assert!(reopened.execute("prefs.set", json!({"path":"integrations.comfyuiModel","value":"arbitrary.safetensors"})).is_err());
        app.ui.ai.available = true;
        app.ui.ai.available_models = vec![ComfyUiModel::TurboInt8];
        clear_document(&mut app.ui.ai);
        assert!(model_ready(&app, ComfyUiModel::TurboInt8));
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

    struct ModelAvailability {
        models: Mutex<Vec<ComfyUiModel>>,
        requests: Mutex<Vec<ComfyUiModel>>,
    }
    impl AiBackend for ModelAvailability {
        fn available_models(&self, _: &str) -> photocraft_engine::Result<Vec<ComfyUiModel>> {
            Ok(self.models.lock().unwrap().clone())
        }
        fn generate(
            &self,
            request: &photocraft_engine::ai_cmds::AiRequest,
            _: &photocraft_engine::jobs::JobCtx,
        ) -> photocraft_engine::Result<photocraft_raster::Rgba8Image> {
            self.requests.lock().unwrap().push(request.model);
            let mut image = request.image.clone();
            for pixel in image.pixels.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
            Ok(image)
        }
    }

    #[test]
    fn single_detected_model_overrides_saved_choice_and_no_models_hides_bar() {
        for single in ComfyUiModel::ALL {
            let other = ComfyUiModel::ALL.into_iter().find(|model| *model != single).unwrap();
            let backend = Arc::new(ModelAvailability { models: Mutex::new(vec![single]), requests: Mutex::new(Vec::new()) });
            let saved = Arc::new(Mutex::new(Vec::<String>::new()));
            let sink = saved.clone();
            let mut session = Session::new();
            session.ai_backend = Some(backend.clone());
            session.edit_prefs(|prefs| prefs.integrations.comfyui_model = other);
            let mut app = PhotocraftApp::new(
                session,
                Services {
                    save_prefs: Some(Box::new(move |text| {
                        sink.lock().unwrap().push(text.into());
                        Ok(())
                    })),
                    ..Default::default()
                },
            );
            app.run("file.new", json!({"width":20,"height":20})).unwrap();
            let revision = app.session.active().unwrap().revision;
            let ctx = egui::Context::default();
            poll_service(&mut app, &ctx);
            finish_probe(&mut app, &ctx);
            assert!(app.ui.ai.available);
            assert_eq!(app.ui.ai.available_models, vec![single]);
            assert_eq!(app.session.prefs().integrations.comfyui_model, single);
            assert_eq!(app.session.active().unwrap().revision, revision);
            let mut reopened = Session::new();
            reopened.load_prefs_json(saved.lock().unwrap().last().unwrap()).unwrap();
            assert_eq!(reopened.prefs().integrations.comfyui_model, single);
            app.ai_probe.next_check = 0.0;
            poll_service(&mut app, &ctx);
            finish_probe(&mut app, &ctx);
            assert_eq!(saved.lock().unwrap().len(), 1); // Repeated probes do not rewrite preferences.
            app.run("edit.promptGenerate", json!({"prompt":"blue circle"})).unwrap();
            app.run("select.rect", json!({"x":2,"y":2,"width":4,"height":4})).unwrap();
            app.run("edit.generativeFill", json!({"prompt":""})).unwrap();
            assert_eq!(*backend.requests.lock().unwrap(), vec![single, single]);

            select_model(&mut app, other).unwrap();
            *backend.models.lock().unwrap() = ComfyUiModel::ALL.to_vec();
            app.ai_probe.next_check = 0.0;
            poll_service(&mut app, &ctx);
            finish_probe(&mut app, &ctx);
            assert_eq!(app.ui.ai.available_models.len(), ComfyUiModel::ALL.len());
            assert_eq!(app.session.prefs().integrations.comfyui_model, other);
            assert_eq!(saved.lock().unwrap().len(), 2);

            backend.models.lock().unwrap().clear();
            app.ui.ai.open = true;
            app.ui.ai.rect = Some([0.0, 0.0, 400.0, 100.0]);
            app.ai_probe.next_check = 0.0;
            poll_service(&mut app, &ctx);
            finish_probe(&mut app, &ctx);
            assert!(!app.ui.ai.available);
            assert!(!app.ui.ai.open);
            assert!(app.ui.ai.rect.is_none());
            assert_eq!(app.session.prefs().integrations.comfyui_model, other);
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

    #[test]
    fn model_changes_restore_sampling_defaults_without_touching_prompt() {
        let mut app = PhotocraftApp::new(Session::new(), Services::default());
        app.ui.ai.prompt = "unchanged user prompt".into();
        app.ui.ai.steps = Some(13);
        select_model(&mut app, ComfyUiModel::TurboInt8).unwrap();
        assert!(app.ui.ai.steps.is_none());
        assert_eq!(app.session.prefs().integrations.selected_settings().steps, 8);
        app.ui.ai.steps = Some(6);
        select_model(&mut app, ComfyUiModel::QwenImage21).unwrap();
        assert!(app.ui.ai.steps.is_none());
        assert_eq!(app.session.prefs().integrations.selected_settings().steps, 25);
        assert_eq!(app.ui.ai.prompt, "unchanged user prompt");
    }
}

use super::params::bool_field;
use crate::PhotocraftApp;
use serde_json::Value;

pub(super) struct AiSet {
    ai_prompt: Option<String>,
    ai_open: Option<bool>,
    ai_prompt_info: Option<bool>,
    ai_model_settings: Option<bool>,
    model_settings_tab: Option<bool>,
    ai_cfg: Option<Option<f64>>,
    ai_negative: Option<String>,
    ai_steps: Option<Option<u32>>,
}

impl AiSet {
    pub(super) fn parse(app: &PhotocraftApp, p: &Value) -> Result<Self, String> {
        let ai_prompt = match p.get("aiPrompt") {
            Some(v) => Some(
                v.as_str()
                    .filter(|text| text.len() <= photocraft_engine::ai_cmds::MAX_PROMPT_BYTES)
                    .ok_or_else(|| "aiPrompt must be a string of at most 8192 bytes".to_string())?
                    .to_owned(),
            ),
            None => None,
        };
        let ai_open = bool_field(p, "aiOpen")?;
        let ai_prompt_info = bool_field(p, "aiPromptInfo")?;
        let ai_model_settings = bool_field(p, "aiModelSettings")?;
        let model_settings_tab = match p.get("aiModelSettingsTab") {
            Some(value) => Some(match value.as_str() {
                Some("models") => false,
                Some("encoders") => true,
                _ => return Err("Tab must be models or encoders".into()),
            }),
            None => None,
        };
        let ai_cfg = match p.get("aiCfg") {
            Some(Value::Null) => Some(None),
            Some(value) => Some(Some(value.as_f64().filter(|value| value.is_finite() && (1.0..=100.0).contains(value)).ok_or("aiCfg must be within 1..100")?)),
            None => None,
        };
        if ai_cfg.flatten().is_some_and(|value| value != 1.0)
            && app.session.prefs().integrations.comfyui_model == photocraft_engine::prefs::ComfyUiModel::TurboInt8
        {
            return Err("Stock Turbo CFG is fixed at 1".into());
        }
        let ai_negative = match p.get("aiNegativePrompt") {
            Some(value) => {
                Some(value.as_str().filter(|text| text.len() <= photocraft_engine::ai_cmds::MAX_PROMPT_BYTES).ok_or("Invalid negative prompt")?.to_owned())
            }
            None => None,
        };
        let ai_steps = match p.get("aiSteps") {
            Some(Value::Null) => Some(None),
            Some(value) => Some(Some(
                value
                    .as_u64()
                    .filter(|steps| (1..=u64::from(photocraft_engine::ai_cmds::MAX_STEPS)).contains(steps))
                    .ok_or_else(|| "aiSteps must be null or an integer within 1..1000".to_string())? as u32,
            )),
            None => None,
        };
        if ai_prompt_info == Some(true) && app.session.active().is_none() {
            return Err("no document open".into());
        }
        if ai_open == Some(true) || ai_prompt.is_some() || ai_steps.is_some() {
            let state = app.session.active().ok_or_else(|| "no document open".to_string())?;
            photocraft_engine::ai_cmds::supported_document(&state.doc)?;
        }
        Ok(Self { ai_prompt, ai_open, ai_prompt_info, ai_model_settings, model_settings_tab, ai_cfg, ai_negative, ai_steps })
    }
    pub(super) fn apply(self, app: &mut PhotocraftApp) -> Result<(), String> {
        let Self { ai_prompt, ai_open, ai_prompt_info, ai_model_settings, model_settings_tab, ai_cfg, ai_negative, ai_steps } = self;
        if let Some(open) = ai_open {
            if open {
                crate::ai_bar::open(app)?;
            } else {
                app.ui.ai.open = false;
            }
        }
        if let Some(prompt) = ai_prompt {
            crate::ai_bar::set_prompt(app, prompt)?;
        }
        if let Some(steps) = ai_steps {
            crate::ai_bar::set_steps(app, steps)?;
        }
        if let Some(open) = ai_prompt_info {
            app.ui.ai.prompt_info_open = open;
            app.ui.ai.prompt_info_page = 0;
        }
        if let Some(open) = ai_model_settings {
            if open {
                crate::ai_bar::open_model_settings(app);
            } else {
                app.ui.ai.model_settings_open = false;
            }
        }
        if let Some(cfg) = ai_cfg {
            app.ui.ai.cfg = cfg;
        }
        if let Some(negative) = ai_negative {
            app.ui.ai.negative_prompt = negative;
        }
        if let Some(tab) = model_settings_tab {
            app.ui.ai.model_settings_te_tab = tab;
        }
        Ok(())
    }
}

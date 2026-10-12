//! User-owned Qwen checkpoint settings and text-encoder selection; no model loading.
use crate::prefs::{ComfyUiModel, Integrations};
use serde::{Deserialize, Serialize};

pub const DEFAULT_ENCODER: &str = "qwen3vl_8b_int8_convrot.safetensors";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModelSettings {
    pub text_encoder: Option<String>,
    pub cfg: f64,
    pub steps: u32,
    pub turbo: bool,
}
impl Default for ModelSettings {
    fn default() -> Self {
        Self { text_encoder: None, cfg: 1.0, steps: 25, turbo: false }
    }
}
impl ModelSettings {
    pub fn validate(&self) -> Result<(), String> {
        if !self.cfg.is_finite() || !(1.0..=100.0).contains(&self.cfg) {
            return Err("CFG must be within 1..100".into());
        }
        if !(1..=crate::ai_cmds::MAX_STEPS).contains(&self.steps) {
            return Err("Steps must be within 1..1000".into());
        }
        if let Some(name) = &self.text_encoder {
            crate::prefs::custom_model_names(name)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    /// Registered names whose checkpoint file was found, including installed built-ins.
    pub models: Vec<String>,
    /// Unique actual ComfyUI paths, restricted to the default and registered encoders.
    pub encoders: Vec<String>,
    pub turbo_available: bool,
}

pub fn builtin(filename: &str) -> Option<ComfyUiModel> {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    ComfyUiModel::ALL.into_iter().find(|model| model.filename().eq_ignore_ascii_case(base))
}

impl Integrations {
    pub fn selected_filename(&self) -> &str {
        if self.comfyui_model == ComfyUiModel::Custom { &self.comfyui_custom_model } else { self.comfyui_model.filename() }
    }
    pub fn settings_for(&self, filename: &str) -> ModelSettings {
        let preset = builtin(filename);
        let mut settings = self
            .comfyui_model_settings
            .get(filename)
            .cloned()
            .unwrap_or_else(|| ModelSettings { steps: preset.map_or(25, ComfyUiModel::default_steps), ..Default::default() });
        settings.cfg = if settings.cfg.is_finite() { settings.cfg.clamp(1.0, 100.0) } else { 1.0 };
        settings.steps = settings.steps.clamp(1, crate::ai_cmds::MAX_STEPS);
        if let Some(model) = preset {
            settings.turbo = model == ComfyUiModel::TurboInt8;
            if settings.turbo {
                settings.cfg = 1.0;
            }
        }
        settings
    }
    pub fn selected_settings(&self) -> ModelSettings {
        self.settings_for(self.selected_filename())
    }
}

/// Use explicit assignment first. Auto prefers the stock encoder, otherwise exactly one TE.
pub fn resolve_encoder(selected: Option<&str>, detected: &[String]) -> Result<String, String> {
    let find = |name: &str| {
        let normalized = name.replace('\\', "/");
        if let Some(exact) = detected.iter().find(|value| value.replace('\\', "/").eq_ignore_ascii_case(&normalized)) {
            return Ok(exact.clone());
        }
        let matches: Vec<_> = detected
            .iter()
            .filter(|value| {
                value.replace('\\', "/").eq_ignore_ascii_case(&normalized)
                    || value.rsplit(['/', '\\']).next().is_some_and(|base| base.eq_ignore_ascii_case(name))
            })
            .collect();
        match matches.as_slice() {
            [value] => Ok((*value).clone()),
            _ => Err(format!("Text encoder unavailable or ambiguous: {name}")),
        }
    };
    if let Some(name) = selected {
        return find(name);
    }
    if let Ok(name) = find(DEFAULT_ENCODER) {
        return Ok(name);
    }
    match detected {
        [name] => Ok(name.clone()),
        _ => Err("Select a text encoder for this model".into()),
    }
}

pub fn ready(config: &Integrations, catalog: &Catalog, filename: &str) -> bool {
    let settings = config.settings_for(filename);
    catalog.models.iter().any(|name| name == filename)
        && (!settings.turbo || catalog.turbo_available)
        && resolve_encoder(settings.text_encoder.as_deref(), &catalog.encoders).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoder_selection_is_explicit_or_unambiguous_and_never_uses_implicit_aliases() {
        let custom = "folder/custom.safetensors".to_owned();
        assert_eq!(resolve_encoder(None, std::slice::from_ref(&custom)).unwrap(), custom);
        assert!(resolve_encoder(None, &[]).is_err());
        assert!(resolve_encoder(None, &[custom.clone(), "other.safetensors".into()]).is_err());
        assert_eq!(resolve_encoder(None, &[custom.clone(), DEFAULT_ENCODER.into()]).unwrap(), DEFAULT_ENCODER);
        assert_eq!(resolve_encoder(Some("custom.safetensors"), &[custom.clone(), DEFAULT_ENCODER.into()]).unwrap(), custom);
        assert!(resolve_encoder(Some("missing.safetensors"), &[DEFAULT_ENCODER.into()]).is_err());
    }
    #[test]
    fn profiles_preserve_stock_defaults_and_user_overrides_and_turbo_cfg_lock() {
        let mut config = Integrations::default();
        assert_eq!(ComfyUiModel::ALL.len(), 2);
        assert_eq!(config.settings_for(ComfyUiModel::TurboInt8.filename()).steps, 8);
        assert_eq!(config.settings_for("custom.safetensors"), ModelSettings::default());
        config
            .comfyui_model_settings
            .insert("custom.safetensors".into(), ModelSettings { cfg: 3.5, steps: 19, turbo: true, text_encoder: Some("te.safetensors".into()) });
        assert_eq!(config.settings_for("custom.safetensors").cfg, 3.5);
        assert!(config.settings_for("custom.safetensors").turbo);
        config.comfyui_model_settings.insert(ComfyUiModel::TurboInt8.filename().into(), ModelSettings { cfg: 5.0, steps: 7, ..Default::default() });
        assert_eq!(config.settings_for(ComfyUiModel::TurboInt8.filename()).cfg, 1.0);
        assert_eq!(config.settings_for(ComfyUiModel::TurboInt8.filename()).steps, 7);
        assert!(config.settings_for(ComfyUiModel::TurboInt8.filename()).turbo);
    }
}

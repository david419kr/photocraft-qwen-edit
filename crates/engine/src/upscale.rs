//! Native external upscalers. The document changes only after a validated result is ready.
#[cfg(not(target_arch = "wasm32"))]
use crate::Result;
#[cfg(any(not(target_arch = "wasm32"), test))]
use crate::Session;
use crate::{EngineError, commands::CommandSpec};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(any(not(target_arch = "wasm32"), test))]
use serde_json::json;
use std::collections::BTreeMap;

pub const COMMAND: &str = "image.aiUpscale";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub gpu: i32,
    pub tiling: bool,
    pub tile: u32,
    pub overlap: u32,
    pub noise: i32,
    pub alpha: bool,
    pub tta: bool,
    pub batch: u32,
    pub workspace_mib: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self { gpu: -1, tiling: true, tile: 512, overlap: 128, noise: 0, alpha: true, tta: true, batch: 4, workspace_mib: 4096 }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub model: String,
    pub profiles: BTreeMap<String, Settings>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: String,
    pub name: String,
    pub scale: u32,
    #[serde(default)]
    pub restoration: bool,
    #[serde(default)]
    pub defaults: Settings,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub required: Vec<String>,
}

impl Settings {
    pub fn validate(&self, model: &Model) -> std::result::Result<(), String> {
        if !(-1..=63).contains(&self.gpu) || !(512..=8192).contains(&self.workspace_mib) {
            return Err("Invalid GPU or workspace limit".into());
        }
        if model.restoration {
            if !self.tiling || ![64, 256, 400, 640].contains(&self.tile) || ![1, 4, 8, 16, 32].contains(&self.batch) || !(-1..=3).contains(&self.noise) {
                return Err("Invalid restoration options".into());
            }
        } else if ![128, 256, 512, 1024].contains(&self.tile) || (self.tiling && self.overlap >= self.tile) {
            return Err("Overlap must be smaller than the tile".into());
        }
        Ok(())
    }
}

pub fn selected<'a>(prefs: &Preferences, models: &'a [Model]) -> Option<&'a Model> {
    if prefs.model.is_empty() { models.first() } else { models.iter().find(|m| m.id == prefs.model) }
}

pub fn settings(prefs: &Preferences, model: &Model) -> Settings {
    prefs.profiles.get(&model.id).cloned().unwrap_or_else(|| model.defaults.clone())
}

pub fn catalog() -> Vec<Model> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::catalog()
    }
    #[cfg(target_arch = "wasm32")]
    {
        Vec::new()
    }
}

pub fn devices() -> std::result::Result<Value, String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        native::devices().map_err(|e| e.to_string())
    }
    #[cfg(target_arch = "wasm32")]
    {
        Err("Local upscalers are unavailable in this browser".into())
    }
}

pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: COMMAND,
        label: "Upscale",
        menu: &[],
        shortcut: None,
        params: "{model?:id,options?:{gpu,tiling,tile,overlap,noise,alpha,tta,batch,workspaceMib}} → {width,height,layer}; resize current document in one undo step",
        enabled: |s| {
            s.active().ok_or("No document")?;
            if catalog().is_empty() { Err("No local upscaler installed".into()) } else { Ok(()) }
        },
        run: |s, p| {
            if !p.is_object() || p.as_object().is_some_and(|m| m.keys().any(|k| !["model", "options"].contains(&k.as_str()))) {
                return Err(EngineError::Other("Use an object with model/options".into()));
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                native::run(s, p)
            }
            #[cfg(target_arch = "wasm32")]
            {
                let _ = s;
                Err(EngineError::Other("Local upscalers are unavailable in this browser".into()))
            }
        },
        journal: true,
    }]
}

#[cfg(not(target_arch = "wasm32"))]
mod native;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_round_trip_per_model() {
        let mut session = Session::new();
        let mut prefs = Preferences { model: "second".into(), ..Preferences::default() };
        prefs.profiles.insert("first".into(), Settings { tile: 256, overlap: 64, ..Settings::default() });
        prefs.profiles.insert("second".into(), Settings { tile: 640, noise: 2, ..Settings::default() });
        session.execute("prefs.set", json!({"path":"integrations.upscale","value":prefs})).unwrap();
        let saved = session.prefs_to_json();
        let mut reopened = Session::new();
        reopened.load_prefs_json(&saved).unwrap();
        assert_eq!(reopened.prefs().integrations.upscale, prefs);
    }

    #[test]
    fn untiled_mode_ignores_overlap_but_tiled_mode_requires_valid_overlap() {
        let model =
            Model { id: "example".into(), name: "Example".into(), scale: 4, restoration: false, defaults: Settings::default(), file: None, required: vec![] };
        let mut settings = Settings { tile: 128, overlap: 128, tiling: false, ..Settings::default() };
        assert!(settings.validate(&model).is_ok());
        settings.tiling = true;
        assert!(settings.validate(&model).is_err());
    }
}

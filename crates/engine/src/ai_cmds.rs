//! ComfyUI image edits: immutable input, a new top-level layer, and one undo step.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use photocraft_cms::{Builtin, Intent, Transform};
use photocraft_color::{ColorMode, PixelFormat};
use photocraft_doc::{Document, Layer, LayerMask};
use photocraft_geom::Rect;
use photocraft_raster::{Rgba8Image, Surface};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::jobs::JobCtx;
use crate::{EngineError, Result, Session};

mod cutout;
mod fill;
pub mod harmonize;

pub const MAX_DOCUMENT_PIXELS: u64 = 36_000_000;
pub const MAX_PROMPT_BYTES: usize = 8192;
pub const MAX_STEPS: u32 = 1000;
pub const MAX_WORKING_SIDE: u32 = 1536;
pub const REMOVE_BACKGROUND_COMMAND: &str = "edit.aiRemoveBackground";
pub const REGENERATE_COMMAND: &str = "edit.aiRegenerate";
pub const UNLOAD_MODELS_COMMAND: &str = "ai.unloadModels";
const MAX_GENERATION_SIDE: u32 = 16_384;
/// 05_BackgroundRemoval_AutoINT8Cache, generalised beyond its example woman.
pub const REMOVE_BACKGROUND_PROMPT: &str = concat!(
    "Remove only the background from <image1>. Return an RGBA cutout of the main foreground subject or subjects, whatever their category. ",
    "Infer which foreground subjects to keep from the image. Preserve the original image dimensions, positions, shapes, colors, textures and fine details. ",
    "Keep every part belonging to each foreground subject. Keep holes and gaps that reveal the background transparent. ",
    "All opaque foreground surfaces MUST remain fully opaque, alpha 255, including bright, white, pale, dark, reflective and highlighted areas. ",
    "The background MUST have alpha 0. Preserve genuinely translucent foreground materials where present; otherwise intermediate alpha is allowed only along the thin antialiased silhouette boundary. ",
    "Do not turn brightness, color, shading or reflections into transparency. Do not fade opaque surfaces. ",
    "Do not add a background, checkerboard, shadow, outline, text or new objects. Output only the finished RGBA cutout.",
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditMode {
    PromptGenerate,
    PromptEdit,
    GenerativeFill,
}

pub(crate) type ModeCache = std::collections::HashMap<photocraft_doc::DocId, ((u64, u64), EditMode)>;

impl EditMode {
    pub fn command(self) -> &'static str {
        match self {
            Self::PromptGenerate => "edit.promptGenerate",
            Self::PromptEdit => "edit.promptEdit",
            Self::GenerativeFill => "edit.generativeFill",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::PromptGenerate => "Prompt Generate",
            Self::PromptEdit => "Prompt Edit",
            Self::GenerativeFill => "Generative Fill",
        }
    }
}

/// The explicit sRGB8 transport used by the model, independent of the document format.
#[derive(Clone)]
pub struct AiRequest {
    pub image: Rgba8Image,
    pub prompt: String,
    pub mode: EditMode,
    /// Empty-prompt fill reconstructs background rather than editing existing content.
    pub removal: bool,
    pub endpoint: String,
    pub seed: u64,
    pub model: crate::prefs::ComfyUiModel,
    pub custom_model: Option<String>,
    pub text_encoder: String,
    pub cfg: f64,
    pub negative_prompt: String,
    pub turbo: bool,
    pub steps: u32,
}

/// In-memory reroll snapshot; never serialised into document or preferences files.
#[derive(Clone)]
pub(crate) struct LastRun {
    source: Arc<Document>,
    request: AiRequest,
    region: EditRegion,
    name: String,
    result_layer: photocraft_doc::LayerId,
    result_revision: u64,
    prompt_entry: crate::ai_prompts::Entry,
    harmonize: Option<Arc<harmonize::Source>>,
    cutout: Option<photocraft_doc::LayerId>,
}

pub fn regeneration(s: &Session) -> Option<(EditMode, crate::prefs::ComfyUiModel)> {
    let st = s.active()?;
    let last = s.ai_runs.get(&st.doc.id)?;
    (last.result_revision == st.revision && st.doc.layer(last.result_layer).is_some()).then_some((last.request.mode, last.request.model))
}

/// Installed by native frontends; tests use a fake backend and never load an AI model.
pub trait AiBackend: Send + Sync {
    fn unload_models(&self, _endpoint: &str, _ctx: &JobCtx) -> Result<()> {
        Ok(())
    }
    /// Resolve a changed image model before translation loads its own weights.
    fn prepare_models(&self, _request: &AiRequest, _ctx: &JobCtx) -> Result<()> {
        Ok(())
    }
    /// Read-only service/model availability; fake backends need no external service.
    fn check_available(&self, _endpoint: &str) -> Result<()> {
        Ok(())
    }
    fn available_models(&self, endpoint: &str) -> Result<Vec<crate::prefs::ComfyUiModel>> {
        self.check_available(endpoint)?;
        Ok(crate::prefs::ComfyUiModel::ALL.to_vec())
    }
    fn available_models_with_custom(&self, endpoint: &str, _names: &[String]) -> Result<(Vec<crate::prefs::ComfyUiModel>, Vec<String>)> {
        self.available_models(endpoint).map(|models| (models, Vec::new()))
    }
    fn model_catalog(&self, endpoint: &str, config: &crate::prefs::Integrations) -> Result<crate::ai_models::Catalog> {
        let names = crate::prefs::custom_model_names(&config.comfyui_custom_models).map_err(EngineError::Other)?;
        let (models, custom) = self.available_models_with_custom(endpoint, &names)?;
        let mut files: Vec<_> =
            models.into_iter().filter(|model| *model != crate::prefs::ComfyUiModel::Custom).map(|model| model.filename().to_owned()).collect();
        files.extend(custom);
        let mut encoders = vec![crate::ai_models::DEFAULT_ENCODER.to_owned()];
        encoders.extend(crate::prefs::custom_model_names(&config.comfyui_custom_encoders).map_err(EngineError::Other)?);
        Ok(crate::ai_models::Catalog { models: files, encoders, turbo_available: true })
    }
    fn resolve_text_encoder(&self, _endpoint: &str, configured: Option<&str>, _registered: &[String]) -> Result<String> {
        Ok(configured.unwrap_or(crate::ai_models::DEFAULT_ENCODER).to_owned())
    }
    fn available_translation_models(&self, _endpoint: &str) -> Result<Vec<crate::prefs::ComfyUiTranslationModel>> {
        Ok(Vec::new())
    }
    fn translate(&self, _endpoint: &str, _model: crate::prefs::ComfyUiTranslationModel, _prompt: &str, _ctx: &JobCtx) -> Result<String> {
        Err(EngineError::Other("Prompt translation is not configured in this frontend".into()))
    }
    fn generate(&self, request: &AiRequest, ctx: &JobCtx) -> Result<Rgba8Image>;
}

fn bad(command: &str, message: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: command.into(), msg: message.into() }
}

pub fn supported_document(doc: &Document) -> std::result::Result<(), String> {
    let pixels = u64::from(doc.size.width) * u64::from(doc.size.height);
    if pixels == 0 || pixels > MAX_DOCUMENT_PIXELS {
        return Err("AI edits require a nonempty document of at most 36 megapixels".into());
    }
    if !matches!(doc.mode, ColorMode::Rgb | ColorMode::Grayscale | ColorMode::Cmyk | ColorMode::Lab) {
        return Err("AI edits support RGB, Gray, CMYK and Lab documents".into());
    }
    Ok(())
}

/// COW tile identity lets the UI cache selection classification across ordinary paint edits.
pub fn selection_signature(doc: &Document) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    doc.id.0.hash(&mut hash);
    doc.size.width.hash(&mut hash);
    doc.size.height.hash(&mut hash);
    if let Some(mask) = &doc.selection {
        mask.format().hash(&mut hash);
        for value in mask.default_pixel() {
            value.to_bits().hash(&mut hash);
        }
        for (coord, tile) in mask.tiles() {
            coord.hash(&mut hash);
            (tile.bytes().as_ptr() as usize).hash(&mut hash);
        }
    }
    hash.finish()
}

/// Bounds alone cannot distinguish Select All from a ring or a feathered selection.
pub fn edit_mode(doc: &Document) -> EditMode {
    let Some(mask) = &doc.selection else { return EditMode::PromptEdit };
    if mask.default_pixel().first().is_some_and(|v| *v <= 0.0) && mask.content_bounds().intersect(&doc.bounds()).is_empty() {
        return EditMode::PromptEdit;
    }
    if supported_document(doc).is_err() {
        return EditMode::GenerativeFill;
    }
    if mask.tile_count() == 0 && mask.default_pixel().first().is_some_and(|v| v.is_finite() && *v >= 1.0) {
        return EditMode::PromptEdit;
    }
    let channels = mask.format().channels().max(1);
    for coord in doc.bounds().tiles() {
        let values = mask.read_region(coord.rect().intersect(&doc.bounds()));
        if values.chunks_exact(channels).any(|p| p.first().is_none_or(|v| !v.is_finite() || *v < 1.0)) {
            return EditMode::GenerativeFill;
        }
    }
    EditMode::PromptEdit
}

/// Exact appearance test: all fully transparent, or all opaque pure white. Hidden content
/// does not count. Bands bound the temporary composite and stop at the first nonblank pixel.
pub fn blank_canvas(doc: &Document) -> bool {
    if supported_document(doc).is_err() {
        return false;
    }
    let (mut transparent, mut white) = (true, true);
    let completed = photocraft_compose::render_bands(doc, doc.bounds(), 128, |band| -> std::result::Result<(), ()> {
        for pixel in &band.px {
            transparent &= pixel[3].is_finite() && pixel[3] <= 0.0;
            // Allow only floating-point roundoff, not near-white 8/16-bit backgrounds.
            white &= pixel.iter().all(|v| v.is_finite() && (*v - 1.0).abs() <= 1.0e-6);
            if !transparent && !white {
                return Err(());
            }
        }
        Ok(())
    });
    completed.is_ok() && (transparent || white)
}

pub fn contextual_mode(s: &Session, st: &crate::DocState) -> EditMode {
    let key = (st.revision, selection_signature(&st.doc));
    if let Ok(cache) = s.ai_modes.try_borrow()
        && let Some((old, mode)) = cache.get(&st.doc.id)
        && *old == key
    {
        return *mode;
    }
    let selected = edit_mode(&st.doc);
    let mode = if selected == EditMode::PromptEdit && blank_canvas(&st.doc) { EditMode::PromptGenerate } else { selected };
    if let Ok(mut cache) = s.ai_modes.try_borrow_mut() {
        cache.insert(st.doc.id, (key, mode));
    }
    mode
}

pub fn shaped_prompt(mode: EditMode, prompt: &str) -> String {
    if mode != EditMode::GenerativeFill {
        return prompt.into();
    }
    if prompt.trim().is_empty() {
        "Using <image1> as the original image, remove the existing content only in the white region indicated by <image2> and reconstruct the surrounding background there. Keep everything outside the indicated region and its positions unchanged. Maintain continuity of the surrounding line art, textures, colors and translucent overlays at the boundary. Do not include the mask in the result.".into()
    } else {
        format!(
            "Using <image1> as the original image, apply the following edit only in the white region indicated by <image2>:\n{prompt}\nBlend the edited region naturally with the surrounding image. Keep everything outside the indicated region unchanged. Do not include the mask in the result."
        )
    }
}

fn can_edit(s: &Session) -> std::result::Result<(), String> {
    if s.jobs().iter().any(|job| job.command == UNLOAD_MODELS_COMMAND) {
        return Err("AI model unloading is in progress".into());
    }
    supported_document(&s.active().ok_or("no document open")?.doc)
}
fn can_fill(s: &Session) -> std::result::Result<(), String> {
    can_edit(s)?;
    if s.active().is_some_and(|d| d.doc.selection.is_some()) { Ok(()) } else { Err("select an area first".into()) }
}

fn can_generate(s: &Session) -> std::result::Result<(), String> {
    can_edit(s)?;
    let st = s.active().ok_or("no document open")?;
    if contextual_mode(s, st) == EditMode::PromptGenerate {
        Ok(())
    } else {
        Err("text-to-image requires a transparent or pure-white canvas without a partial selection".into())
    }
}

fn can_regenerate(s: &Session) -> std::result::Result<(), String> {
    can_edit(s)?;
    if regeneration(s).is_some() { Ok(()) } else { Err("no unchanged AI result to regenerate".into()) }
}

pub fn specs() -> Vec<CommandSpec> {
    let mut commands: Vec<_> = [EditMode::PromptGenerate, EditMode::PromptEdit, EditMode::GenerativeFill]
        .into_iter()
        .map(|mode| CommandSpec {
            id: mode.command(),
            label: match mode {
                EditMode::PromptGenerate => "Prompt Generate…",
                EditMode::PromptEdit => "Prompt Edit…",
                EditMode::GenerativeFill => "Generative Fill…",
            },
            menu: &["Edit"],
            shortcut: None,
            params: "{\"prompt\":str (blank removes content for Generative Fill),\"cfg\":number? (1..100; stock Turbo fixed 1),\"negativePrompt\":str? (used above CFG 1),\"steps\":u32? (1..1000; model settings default),\"seed\":u64?,\"name\":str?} → {layer,mode,seed,steps,workingSize,workingRect,contentSize}",
            enabled: match mode {
                EditMode::PromptGenerate => can_generate,
                EditMode::PromptEdit => can_edit,
                EditMode::GenerativeFill => can_fill,
            },
            run: match mode {
                EditMode::PromptGenerate => prompt_generate,
                EditMode::PromptEdit => prompt_edit,
                EditMode::GenerativeFill => generative_fill,
            },
            journal: true,
        })
        .collect();
    commands.push(CommandSpec {
        id: UNLOAD_MODELS_COMMAND,
        label: "Unload AI Models",
        menu: &[],
        shortcut: None,
        params: "{} → {unloaded:true}; release all ComfyUI models and workflow caches",
        enabled: |s| s.ai_backend.as_ref().map(|_| ()).ok_or_else(|| "ComfyUI is not configured".into()),
        run: |s, p| {
            if !p.is_object() || p.as_object().is_some_and(|p| !p.is_empty()) {
                return Err(bad(UNLOAD_MODELS_COMMAND, "use an empty object"));
            }
            let backend = s.ai_backend.clone().ok_or_else(|| EngineError::Other("ComfyUI is not configured".into()))?;
            let endpoint = s.prefs().integrations.comfyui_url.clone();
            crate::jobs::run(s, "Unload AI Models", false, move |ctx| backend.unload_models(&endpoint, ctx), |_, ()| Ok(json!({"unloaded":true})))
        },
        journal: false,
    });
    commands.push(CommandSpec {
        id: REGENERATE_COMMAND,
        label: "Regenerate AI Result",
        menu: &["Edit"],
        shortcut: None,
        params: "{} → {layer,mode,seed,workingSize,workingRect,contentSize}; same source/settings, new seed, replaces last result in one undo step",
        enabled: can_regenerate,
        run: regenerate,
        journal: true,
    });
    commands.push(CommandSpec {
        id: "document.aiPrompts",
        label: "AI Prompt History",
        menu: &[],
        shortcut: None,
        params: "{} → {entries:[{mode,regions,original,translated?}]} (ordered AI prompts for the current undo state)",
        enabled: |s| s.active().map(|_| ()).ok_or_else(|| "No document".into()),
        run: |s, p| {
            if !p.is_object() {
                return Err(bad("document.aiPrompts", "params must be an object"));
            }
            let doc = &s.active().ok_or(EngineError::NoDocument)?.doc;
            Ok(json!({"entries":crate::ai_prompts::read(doc.metadata.xmp.as_deref()).map_err(EngineError::Other)?}))
        },
        journal: false,
    });
    commands.push(CommandSpec {
        id: REMOVE_BACKGROUND_COMMAND,
        label: "Remove Background",
        menu: &[],
        shortcut: None,
        params: "{\"steps\":u32?,\"seed\":u64?} → {layer,mode,seed,steps,workingSize,workingRect,contentSize} (Qwen alpha replaces the active pixel layer mask; preserves original pixels and other layers)",
        enabled: cutout::enabled,
        run: remove_background,
        journal: true,
    });
    commands.extend(harmonize::specs());
    commands
}

fn remove_background(s: &mut Session, p: &Value) -> Result<Value> {
    cutout::enabled(s).map_err(EngineError::Other)?;
    let mut params = p.as_object().cloned().ok_or_else(|| bad(REMOVE_BACKGROUND_COMMAND, "params must be an object"))?;
    params.insert("prompt".into(), json!(REMOVE_BACKGROUND_PROMPT));
    params.entry("name").or_insert_with(|| json!("Remove Background"));
    let id = crate::active_layer_of(s).map_err(EngineError::Other)?.id;
    run(s, &Value::Object(params), EditMode::PromptEdit, false, Some(id))
}

fn prompt_edit(s: &mut Session, p: &Value) -> Result<Value> {
    run(s, p, EditMode::PromptEdit, true, None)
}
fn prompt_generate(s: &mut Session, p: &Value) -> Result<Value> {
    can_generate(s).map_err(EngineError::Other)?;
    run(s, p, EditMode::PromptGenerate, true, None)
}
fn generative_fill(s: &mut Session, p: &Value) -> Result<Value> {
    run(s, p, EditMode::GenerativeFill, true, None)
}

fn default_seed() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64)
    }
    #[cfg(target_arch = "wasm32")]
    {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}

fn needs_translation(prompt: &str) -> bool {
    prompt.chars().any(|ch| ch.is_alphabetic() && !ch.is_ascii_alphabetic())
}

fn run(s: &mut Session, p: &Value, mode: EditMode, translate_user_prompt: bool, cutout: Option<photocraft_doc::LayerId>) -> Result<Value> {
    let command = mode.command();
    if !p.is_object() {
        return Err(bad(command, "params must be an object"));
    }
    let prompt = p.get("prompt").and_then(Value::as_str).ok_or_else(|| bad(command, "prompt must be a string"))?;
    if prompt.len() > MAX_PROMPT_BYTES || (mode != EditMode::GenerativeFill && prompt.trim().is_empty()) {
        return Err(bad(command, "enter a prompt of at most 8192 bytes"));
    }
    let seed = match p.get("seed") {
        Some(v) => v.as_u64().ok_or_else(|| bad(command, "seed must be an unsigned integer"))?,
        None => default_seed(),
    };
    let name = match p.get("name") {
        Some(v) => v
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 512)
            .ok_or_else(|| bad(command, "name must be a nonempty string of at most 512 bytes"))?
            .to_owned(),
        None => mode.label().into(),
    };
    let doc = &s.active().ok_or(EngineError::NoDocument)?.doc;
    supported_document(doc).map_err(EngineError::Other)?;
    if cutout.is_none()
        && ((mode == EditMode::PromptGenerate && contextual_mode(s, s.active().ok_or(EngineError::NoDocument)?) != mode)
            || (mode != EditMode::PromptGenerate && edit_mode(doc) != mode))
    {
        return Err(bad(command, "the command does not match the current selection"));
    }
    if mode == EditMode::GenerativeFill && doc.selection.as_ref().is_none_or(|m| m.content_bounds().intersect(&doc.bounds()).is_empty()) {
        return Err(bad(command, "the selected area is empty"));
    }
    let backend = s.ai_backend.clone().ok_or_else(|| EngineError::Other("ComfyUI is not configured in this frontend".into()))?;
    let endpoint = s.prefs().integrations.comfyui_url.clone();
    let model = s.prefs().integrations.comfyui_model;
    let settings = s.prefs().integrations.selected_settings();
    let fill_filter = fill::output_filter(s.prefs().general.image_interpolation);
    let cfg = p
        .get("cfg")
        .map_or(Some(settings.cfg), Value::as_f64)
        .filter(|value| value.is_finite() && (1.0..=100.0).contains(value))
        .ok_or_else(|| bad(command, "CFG must be within 1..100"))?;
    if crate::ai_models::builtin(s.prefs().integrations.selected_filename()) == Some(crate::prefs::ComfyUiModel::TurboInt8) && cfg != 1.0 {
        return Err(bad(command, "Stock Turbo CFG is fixed at 1"));
    }
    let negative = p
        .get("negativePrompt")
        .map_or(Some(""), Value::as_str)
        .filter(|text| text.len() <= MAX_PROMPT_BYTES)
        .ok_or_else(|| bad(command, "Negative prompt must contain at most 8192 bytes"))?;
    let negative = if cfg > 1.0 { negative.to_owned() } else { String::new() };
    let registered_encoders = crate::prefs::custom_model_names(&s.prefs().integrations.comfyui_custom_encoders).map_err(|e| bad(command, e))?;
    let custom_model = if model == crate::prefs::ComfyUiModel::Custom {
        let name = s.prefs().integrations.comfyui_custom_model.trim().to_owned();
        let names = crate::prefs::custom_model_names(&s.prefs().integrations.comfyui_custom_models).map_err(|e| bad(command, e))?;
        if !names.contains(&name) {
            return Err(bad(command, "selected custom model is not registered"));
        }
        Some(name)
    } else {
        None
    };
    let translate_prompt = translate_user_prompt && s.prefs().integrations.comfyui_translate_prompt && needs_translation(prompt);
    let translate_negative = s.prefs().integrations.comfyui_translate_prompt && needs_translation(&negative);
    let translation_model = s.prefs().integrations.comfyui_translation_model;
    let steps = match p.get("steps") {
        Some(value) => {
            value.as_u64().filter(|steps| (1..=u64::from(MAX_STEPS)).contains(steps)).ok_or_else(|| bad(command, "steps must be an integer within 1..1000"))?
                as u32
        }
        None => settings.steps,
    };
    let prompt = prompt.to_owned();
    let source = s.active().ok_or(EngineError::NoDocument)?.doc.clone();
    let label = if cutout.is_some() { "Remove Background" } else { mode.label() };
    crate::jobs::run(
        s,
        label,
        true,
        move |ctx| {
            let mut input = if let Some(id) = cutout { cutout::prepare(&source, id, ctx)? } else { prepare(&source, mode, ctx)? };
            if mode == EditMode::GenerativeFill {
                input.region.filter = fill_filter;
            }
            let original = prompt.clone();
            let mut prompt = prompt;
            let mut translated = None;
            let negative_original = (!negative.is_empty()).then(|| negative.clone());
            let mut negative = negative;
            let mut negative_translated = None;
            let text_encoder = backend.resolve_text_encoder(&endpoint, settings.text_encoder.as_deref(), &registered_encoders)?;
            let mut request = AiRequest {
                image: input.image,
                prompt: String::new(),
                mode,
                removal: mode == EditMode::GenerativeFill && prompt.trim().is_empty(),
                endpoint: endpoint.clone(),
                seed,
                model,
                custom_model,
                text_encoder,
                cfg,
                negative_prompt: String::new(),
                turbo: settings.turbo,
                steps,
            };
            backend.prepare_models(&request, ctx)?;
            if translate_prompt || translate_negative {
                let available = backend.available_translation_models(&endpoint)?;
                if let Some(selected) = available.iter().find(|model| **model == translation_model).or(available.first()) {
                    ctx.check()?;
                    ctx.progress(0.06, "Translating prompt to English");
                    if translate_prompt {
                        prompt = backend.translate(&endpoint, *selected, &prompt, ctx)?;
                        translated = Some(prompt.clone());
                    }
                    if translate_prompt && (prompt.trim().is_empty() || prompt.len() > MAX_PROMPT_BYTES) {
                        return Err(EngineError::Other("Translated prompt must contain 1..8192 bytes".into()));
                    }
                    if translate_negative {
                        negative = backend.translate(&endpoint, *selected, &negative, ctx)?;
                        if negative.trim().is_empty() || negative.len() > MAX_PROMPT_BYTES {
                            return Err(EngineError::Other("Invalid translated negative prompt".into()));
                        }
                        negative_translated = Some(negative.clone());
                    }
                }
            }
            ctx.check()?;
            request.prompt = shaped_prompt(mode, &prompt);
            request.negative_prompt = negative;
            let generated = backend.generate(&request, ctx)?;
            let layer = if let Some(id) = cutout {
                cutout::result(&source, id, &generated, &input.region, ctx)?
            } else {
                result_layer(&source, &generated, &input.region, &name, mode, ctx)?
            };
            let prompt_entry =
                crate::ai_prompts::Entry { mode, regions: input.region.prompt_regions.clone(), original, translated, negative_original, negative_translated };
            let harmonize = (mode == EditMode::GenerativeFill)
                .then(|| Arc::new(harmonize::Source::new(source.clone(), request.image.clone(), generated, input.region.clone(), layer.clone())));
            let last = LastRun { source, request, region: input.region, name, result_layer: layer.id, result_revision: 0, prompt_entry, harmonize, cutout };
            Ok((layer, last))
        },
        move |s, (layer, last)| apply_result(s, layer, last, None, label),
    )
}

fn regenerate(s: &mut Session, p: &Value) -> Result<Value> {
    if !p.is_object() || p.get("seed").is_some() {
        return Err(bad(REGENERATE_COMMAND, "use an object without a seed; regeneration always chooses a new seed"));
    }
    can_regenerate(s).map_err(EngineError::Other)?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let mut last = s.ai_runs.get(&st.doc.id).cloned().ok_or_else(|| bad(REGENERATE_COMMAND, "no previous AI generation"))?;
    let old_seed = last.request.seed;
    last.request.seed = default_seed();
    if last.request.seed == old_seed {
        last.request.seed = old_seed.wrapping_add(1);
    }
    let replace = last.result_layer;
    let backend = s.ai_backend.clone().ok_or_else(|| EngineError::Other("ComfyUI is not configured in this frontend".into()))?;
    crate::jobs::run(
        s,
        "Regenerate AI Result",
        true,
        move |ctx| {
            let generated = backend.generate(&last.request, ctx)?;
            let layer = if let Some(id) = last.cutout {
                cutout::result(&last.source, id, &generated, &last.region, ctx)?
            } else {
                result_layer(&last.source, &generated, &last.region, &last.name, last.request.mode, ctx)?
            };
            last.harmonize = (last.request.mode == EditMode::GenerativeFill)
                .then(|| Arc::new(harmonize::Source::new(last.source.clone(), last.request.image.clone(), generated, last.region.clone(), layer.clone())));
            Ok((layer, last))
        },
        move |s, (layer, last)| apply_result(s, layer, last, Some(replace), "Regenerate AI Result"),
    )
}

fn apply_result(s: &mut Session, mut layer: Layer, mut last: LastRun, replace: Option<photocraft_doc::LayerId>, label: &str) -> Result<Value> {
    let mode = last.request.mode;
    if let Some(id) = replace {
        layer.id = id;
    }
    let id = layer.id;
    let prompt_entry = last.prompt_entry.clone();
    let cutout = last.cutout;
    s.edit(label, move |doc, active| {
        let prompt_xmp =
            if replace.is_none() { Some(crate::ai_prompts::append(doc.metadata.xmp.as_deref(), &prompt_entry).map_err(EngineError::Other)?) } else { None };
        if let Some(target) = cutout {
            cutout::check(doc, target).map_err(EngineError::Other)?;
            crate::extra_cmds::background_to_layer_for_mask(doc, target);
            doc.layer_mut(target).ok_or(EngineError::NoLayer(target))?.mask = layer.mask;
            doc.selection = None;
        } else if let Some(old) = replace {
            let target = doc.layers.iter_mut().find(|l| l.id == old).ok_or_else(|| EngineError::Other("previous AI layer was removed".into()))?;
            *target = layer;
        } else {
            if mode != EditMode::GenerativeFill {
                for source in &mut doc.layers {
                    source.visible = false;
                }
            }
            doc.layers.push(layer);
        }
        *active = Some(id);
        if let Some(xmp) = prompt_xmp {
            doc.metadata.xmp = Some(xmp.into());
        }
        Ok(())
    })?;
    let st = s.active().ok_or(EngineError::NoDocument)?;
    last.result_layer = id;
    last.result_revision = st.revision;
    let bounds = last.region.bounds;
    let result = json!({"layer":id.0,"mode":mode,"seed":last.request.seed,"steps":last.request.steps,"workingSize":[last.request.image.width,last.request.image.height],
        "workingRect":[bounds.x0,bounds.y0,bounds.x1,bounds.y1],"contentSize":last.region.content_size});
    let doc_id = st.doc.id;
    if let Some(source) = &last.harmonize {
        harmonize::remember(s, id, source.clone());
    }
    s.ai_runs.insert(doc_id, last);
    Ok(result)
}

struct Prepared {
    image: Rgba8Image,
    region: EditRegion,
}

#[derive(Clone)]
struct EditRegion {
    bounds: Rect,
    content_size: [u32; 2],
    mask: Option<Surface>,
    prompt_regions: Vec<crate::ai_prompts::Region>,
    filter: photocraft_algo::resample::Resample,
}

impl EditRegion {
    fn canvas(doc: &Document, content_size: [u32; 2]) -> Self {
        Self { bounds: doc.bounds(), content_size, mask: None, prompt_regions: Vec::new(), filter: photocraft_algo::resample::Resample::Bilinear }
    }
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn prepare(doc: &Document, mode: EditMode, ctx: &JobCtx) -> Result<Prepared> {
    ctx.check()?;
    supported_document(doc).map_err(EngineError::Other)?;
    ctx.progress(0.02, "Preparing image and selection");
    if mode == EditMode::GenerativeFill {
        return fill::prepare(doc, ctx);
    }
    if mode == EditMode::PromptGenerate {
        let (w, h) = (doc.size.width, doc.size.height);
        if w == 0 || h == 0 || w > MAX_GENERATION_SIDE || h > MAX_GENERATION_SIDE {
            return Err(EngineError::Other("text-to-image canvas sides must be within 1..16384 pixels".into()));
        }
        let (pw, ph) = (w.div_ceil(32) * 32, h.div_ceil(32) * 32);
        if u64::from(pw) * u64::from(ph) > MAX_DOCUMENT_PIXELS + 1_048_576 {
            return Err(EngineError::Other("padded generation canvas is too large".into()));
        }
        return Ok(Prepared { image: Rgba8Image::new(pw, ph), region: EditRegion::canvas(doc, [w, h]) });
    }
    let scale = (MAX_WORKING_SIDE as f64 / doc.size.width.max(doc.size.height) as f64).min(1.0);
    let w = (doc.size.width as f64 * scale).round().max(1.0) as u32;
    let h = (doc.size.height as f64 * scale).round().max(1.0) as u32;
    let pw = w.div_ceil(32) * 32;
    let ph = h.div_ceil(32) * 32;
    let buf = photocraft_compose::render_reduced(doc, w, h);
    let transform = Transform::new(&crate::color_cmds::composite_profile(doc), Builtin::Srgb.profile(), Intent::RelativeColorimetric, true)
        .map_err(|e| EngineError::Other(e.to_string()))?;
    let mut image = Rgba8Image::new(pw, ph);
    for (y, row) in image.pixels.chunks_exact_mut(pw as usize * 4).enumerate() {
        ctx.check()?;
        for (x, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let sx = x.min(w as usize - 1);
            let sy = y.min(h as usize - 1);
            let rgba = buf.px.get(sy * w as usize + sx).ok_or_else(|| EngineError::Other("incomplete image composite".into()))?;
            let mut converted = [0.0; 4];
            transform.convert_f32(rgba, 4, &mut converted, 4, true);
            let alpha = rgba.get(3).copied().unwrap_or(1.0).clamp(0.0, 1.0);
            for (out, value) in pixel.iter_mut().take(3).zip(converted.iter()) {
                *out = byte(*value * alpha + 1.0 - alpha);
            }
            if let Some(a) = pixel.get_mut(3) {
                *a = 255;
            }
        }
    }
    Ok(Prepared { image, region: EditRegion::canvas(doc, [w, h]) })
}

fn result_layer(doc: &Document, image: &Rgba8Image, region: &EditRegion, name: &str, mode: EditMode, ctx: &JobCtx) -> Result<Layer> {
    ctx.check()?;
    let [w, h] = region.content_size;
    let bounds = region.bounds;
    if w == 0
        || h == 0
        || w > MAX_GENERATION_SIDE
        || h > MAX_GENERATION_SIDE
        || bounds.is_empty()
        || !doc.bounds().contains_rect(&bounds)
        || (mode == EditMode::GenerativeFill && (region.mask.is_none() || w > MAX_WORKING_SIDE || h > MAX_WORKING_SIDE))
    {
        return Err(EngineError::Other("invalid AI result geometry".into()));
    }
    let expected_w = w.div_ceil(32) * 32;
    let expected_h = h.div_ceil(32) * 32;
    if image.width != expected_w || image.height != expected_h || image.pixels.len() != expected_w as usize * expected_h as usize * 4 {
        return Err(EngineError::Other("ComfyUI returned an unexpected image size".into()));
    }
    ctx.progress(0.95, "Adding generated layer");
    let fmt = PixelFormat::new(doc.mode, doc.depth, true);
    let transform = Transform::new(Builtin::Srgb.profile(), &crate::color_cmds::document_profile(doc), Intent::RelativeColorimetric, true)
        .map_err(|e| EngineError::Other(e.to_string()))?;
    let mut source = Surface::new(PixelFormat::RGBA32F);
    for (y, row) in image.pixels.chunks_exact(expected_w as usize * 4).take(h as usize).enumerate() {
        let mut values: Vec<f32> =
            row.get(..w as usize * 4).ok_or_else(|| EngineError::Other("short generated row".into()))?.iter().map(|v| *v as f32 / 255.0).collect();
        if mode == EditMode::GenerativeFill {
            for pixel in values.as_chunks_mut::<4>().0.iter_mut() {
                if let Some(alpha) = pixel.get_mut(3) {
                    *alpha = 1.0;
                }
            }
        }
        source.write_region(Rect::new(0, y as i32, w as i32, y as i32 + 1), &values);
    }
    let resized = photocraft_algo::resample::resize_surface_in_canvas(
        &source,
        bounds.width() as f64 / w as f64,
        bounds.height() as f64 / h as f64,
        region.filter,
        Rect::new(0, 0, w as i32, h as i32),
    );
    let mut surface = Surface::new(fmt);
    let output = region.mask.as_ref().map_or(bounds, |mask| mask.content_bounds().intersect(&bounds));
    for y in output.y0..output.y1 {
        ctx.check()?;
        let r = Rect::new(output.x0, y, output.x1, y + 1);
        let src = resized.read_region(r.translate(-bounds.x0, -bounds.y0));
        let mut converted = vec![0.0; output.width() as usize * fmt.channels()];
        transform.convert_f32(&src, 4, &mut converted, fmt.channels(), true);
        surface.write_region(r, &converted);
    }
    let mut layer = Layer::raster(name, fmt);
    *crate::pixels_mut(&mut layer)? = surface;
    if mode == EditMode::GenerativeFill {
        layer.mask = region.mask.clone().map(|surface| LayerMask { surface, ..LayerMask::reveal_all() });
    }
    Ok(layer)
}

#[cfg(test)]
mod tests;

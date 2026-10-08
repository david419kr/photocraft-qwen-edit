//! ComfyUI image edits: immutable input, a new top-level layer, and one undo step.

use std::hash::{Hash, Hasher};

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

pub const MAX_DOCUMENT_PIXELS: u64 = 36_000_000;
pub const MAX_PROMPT_BYTES: usize = 8192;
pub const MAX_WORKING_SIDE: u32 = 1536;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditMode {
    PromptEdit,
    GenerativeFill,
}

impl EditMode {
    pub fn command(self) -> &'static str {
        match self {
            Self::PromptEdit => "edit.promptEdit",
            Self::GenerativeFill => "edit.generativeFill",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::PromptEdit => "Prompt Edit",
            Self::GenerativeFill => "Generative Fill",
        }
    }
}

/// The explicit sRGB8 transport used by the model, independent of the document format.
pub struct AiRequest {
    pub image: Rgba8Image,
    pub prompt: String,
    pub mode: EditMode,
    /// Empty-prompt fill reconstructs background rather than editing existing content.
    pub removal: bool,
    pub endpoint: String,
    pub seed: u64,
}

/// Installed by native frontends; tests use a fake backend and never load an AI model.
pub trait AiBackend: Send + Sync {
    /// Read-only service/model availability; fake backends need no external service.
    fn check_available(&self, _endpoint: &str) -> Result<()> {
        Ok(())
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
    let channels = mask.format().channels().max(1);
    for coord in doc.bounds().tiles() {
        let values = mask.read_region(coord.rect().intersect(&doc.bounds()));
        if values.chunks_exact(channels).any(|p| p.first().is_none_or(|v| !v.is_finite() || *v < 1.0)) {
            return EditMode::GenerativeFill;
        }
    }
    EditMode::PromptEdit
}

pub fn shaped_prompt(mode: EditMode, prompt: &str) -> String {
    if mode == EditMode::PromptEdit {
        return prompt.into();
    }
    if prompt.trim().is_empty() {
        "The bright-green paint in <image1> marks the region to erase; it is an edit annotation, not part of the artwork. Completely remove the marked shapes and reconstruct only the surrounding background in their place. Leave no object, outline or silhouette in the marked region. Keep all unmarked artwork and its positions unchanged. The result must contain no annotation paint.".into()
    } else {
        format!(
            "Using <image1> as the original image, apply the following edit only in the white region indicated by <image2>:\n{prompt}\nBlend the edited region naturally with the surrounding image. Keep everything outside the indicated region unchanged. Do not include the mask in the result."
        )
    }
}

fn can_edit(s: &Session) -> std::result::Result<(), String> {
    supported_document(&s.active().ok_or("no document open")?.doc)
}
fn can_fill(s: &Session) -> std::result::Result<(), String> {
    can_edit(s)?;
    if s.active().is_some_and(|d| d.doc.selection.is_some()) { Ok(()) } else { Err("select an area first".into()) }
}

pub fn specs() -> Vec<CommandSpec> {
    [EditMode::PromptEdit, EditMode::GenerativeFill]
        .into_iter()
        .map(|mode| CommandSpec {
            id: mode.command(),
            label: match mode {
                EditMode::PromptEdit => "Prompt Edit…",
                EditMode::GenerativeFill => "Generative Fill…",
            },
            menu: &["Edit"],
            shortcut: None,
            params: "{\"prompt\":str (blank removes content for Generative Fill),\"seed\":u64?,\"name\":str?} → {layer,mode,seed,workingSize}",
            enabled: if mode == EditMode::PromptEdit { can_edit } else { can_fill },
            run: if mode == EditMode::PromptEdit { prompt_edit } else { generative_fill },
            journal: true,
        })
        .collect()
}

fn prompt_edit(s: &mut Session, p: &Value) -> Result<Value> {
    run(s, p, EditMode::PromptEdit)
}
fn generative_fill(s: &mut Session, p: &Value) -> Result<Value> {
    run(s, p, EditMode::GenerativeFill)
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

fn run(s: &mut Session, p: &Value, mode: EditMode) -> Result<Value> {
    let command = mode.command();
    if !p.is_object() {
        return Err(bad(command, "params must be an object"));
    }
    let prompt = p.get("prompt").and_then(Value::as_str).ok_or_else(|| bad(command, "prompt must be a string"))?;
    if prompt.len() > MAX_PROMPT_BYTES || (mode == EditMode::PromptEdit && prompt.trim().is_empty()) {
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
    if edit_mode(doc) != mode {
        return Err(bad(command, "the command does not match the current selection"));
    }
    if mode == EditMode::GenerativeFill && doc.selection.as_ref().is_none_or(|m| m.content_bounds().intersect(&doc.bounds()).is_empty()) {
        return Err(bad(command, "the selected area is empty"));
    }
    let backend = s.ai_backend.clone().ok_or_else(|| EngineError::Other("ComfyUI is not configured in this frontend".into()))?;
    let endpoint = s.prefs().integrations.comfyui_url.clone();
    let prompt = prompt.to_owned();
    crate::jobs::edit_job(
        s,
        mode.label(),
        move |doc, active, ctx| {
            let input = prepare(doc, mode, ctx)?;
            let size = [input.image.width, input.image.height];
            let request = AiRequest {
                image: input.image,
                prompt: shaped_prompt(mode, &prompt),
                mode,
                removal: mode == EditMode::GenerativeFill && prompt.trim().is_empty(),
                endpoint,
                seed,
            };
            let generated = backend.generate(&request, ctx)?;
            ctx.check()?;
            let layer = result_layer(doc, &generated, &request.image, input.content_size, &name, mode, ctx)?;
            let id = layer.id;
            doc.layers.push(layer);
            *active = Some(id);
            Ok(json!({"layer":id.0,"mode":mode,"seed":seed,"workingSize":size}))
        },
        |v| v,
    )
}

struct Prepared {
    image: Rgba8Image,
    content_size: [u32; 2],
}

fn byte(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

fn prepare(doc: &Document, mode: EditMode, ctx: &JobCtx) -> Result<Prepared> {
    ctx.check()?;
    ctx.progress(0.02, "Preparing image and selection");
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
        for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
            let sx = x.min(w as usize - 1);
            let sy = y.min(h as usize - 1);
            let rgba = buf.px.get(sy * w as usize + sx).ok_or_else(|| EngineError::Other("incomplete image composite".into()))?;
            let mut converted = [0.0; 4];
            transform.convert_f32(rgba, 4, &mut converted, 4, true);
            let alpha = rgba.get(3).copied().unwrap_or(1.0).clamp(0.0, 1.0);
            for (out, value) in pixel.iter_mut().take(3).zip(converted.iter()) {
                *out = byte(*value * alpha + 1.0 - alpha);
            }
            let coverage = if mode == EditMode::GenerativeFill && x < w as usize && y < h as usize {
                doc.selection.as_ref().map_or(0.0, |m| {
                    let mx = ((x as f64 + 0.5) * doc.size.width as f64 / w as f64 - 0.5).round() as i32;
                    let my = ((y as f64 + 0.5) * doc.size.height as f64 / h as f64 - 0.5).round() as i32;
                    m.sample_channel(mx, my, 0).clamp(0.0, 1.0)
                })
            } else {
                0.0
            };
            if !coverage.is_finite() {
                return Err(EngineError::Other("invalid selection coverage".into()));
            }
            if let Some(a) = pixel.get_mut(3) {
                *a = byte(1.0 - coverage);
            }
        }
    }
    Ok(Prepared { image, content_size: [w, h] })
}

/// Source-gradient Poisson blending: interpolate only the outside colour mismatch.
/// The original RGB beneath the removal mask must never become a boundary condition.
fn harmonize_fill(image: &Rgba8Image, reference: &Rgba8Image, size: [u32; 2], ctx: &JobCtx) -> Result<Vec<f32>> {
    ctx.check()?;
    let [w, h] = size;
    let valid = w > 0
        && h > 0
        && w <= MAX_WORKING_SIDE
        && h <= MAX_WORKING_SIDE
        && image.width == w.div_ceil(32) * 32
        && image.height == h.div_ceil(32) * 32
        && reference.width == image.width
        && reference.height == image.height;
    if !valid {
        return Err(EngineError::Other("invalid image buffers for fill blending".into()));
    }
    let expected = u64::from(image.width) * u64::from(image.height) * 4;
    if image.pixels.len() as u64 != expected || reference.pixels.len() as u64 != expected {
        return Err(EngineError::Other("invalid image buffers for fill blending".into()));
    }
    let (w, h) = (w as usize, h as usize);
    let stride = image.width as usize * 4;
    let mut rgb = Vec::with_capacity(w * h * 3);
    let mut difference = Vec::with_capacity(w * h * 3);
    let mut mask = Vec::with_capacity(w * h);
    // Exclude the synthetic right/bottom padding: it is not unselected image context.
    for (row, original) in image.pixels.chunks_exact(stride).zip(reference.pixels.chunks_exact(stride)).take(h) {
        ctx.check()?;
        let row = row.get(..w * 4).ok_or_else(|| EngineError::Other("short generated row".into()))?;
        let original = original.get(..w * 4).ok_or_else(|| EngineError::Other("short reference row".into()))?;
        for (pixel, original) in row.chunks_exact(4).zip(original.chunks_exact(4)) {
            mask.push(original.get(3).copied().unwrap_or(255) < 255);
            for (generated, source) in pixel.iter().take(3).zip(original.iter()) {
                rgb.push(*generated as f32 / 255.0);
                difference.push((*source as f32 - *generated as f32) / 255.0);
            }
        }
    }
    if !mask.iter().any(|v| *v) || mask.iter().all(|v| *v) {
        return Ok(rgb); // No usable exterior at model resolution: preserve the generated result.
    }
    let correction = ctx
        .stage(0.91, 0.94, "Adding generated layer", |ctl| photocraft_algo::poisson::membrane_fill_with(w, h, 3, &difference, &mask, ctl))
        .map_err(|_| EngineError::Cancelled)?;
    ctx.check()?;
    for ((pixel, correction), selected) in rgb.chunks_exact_mut(3).zip(correction.chunks_exact(3)).zip(mask) {
        if selected {
            for (value, delta) in pixel.iter_mut().zip(correction) {
                *value = (*value + delta).clamp(0.0, 1.0);
            }
        }
    }
    Ok(rgb)
}

fn result_layer(doc: &Document, image: &Rgba8Image, reference: &Rgba8Image, size: [u32; 2], name: &str, mode: EditMode, ctx: &JobCtx) -> Result<Layer> {
    let [w, h] = size;
    let expected_w = w.div_ceil(32) * 32;
    let expected_h = h.div_ceil(32) * 32;
    if image.width != expected_w || image.height != expected_h || image.pixels.len() != expected_w as usize * expected_h as usize * 4 {
        return Err(EngineError::Other("ComfyUI returned an unexpected image size".into()));
    }
    let harmonized = if mode == EditMode::GenerativeFill { Some(harmonize_fill(image, reference, size, ctx)?) } else { None };
    ctx.progress(0.95, "Adding generated layer");
    let fmt = PixelFormat::new(doc.mode, doc.depth, true);
    let transform = Transform::new(Builtin::Srgb.profile(), &crate::color_cmds::document_profile(doc), Intent::RelativeColorimetric, true)
        .map_err(|e| EngineError::Other(e.to_string()))?;
    let mut source = Surface::new(PixelFormat::RGBA32F);
    for (y, row) in image.pixels.chunks_exact(expected_w as usize * 4).take(h as usize).enumerate() {
        let mut values: Vec<f32> =
            row.get(..w as usize * 4).ok_or_else(|| EngineError::Other("short generated row".into()))?.iter().map(|v| *v as f32 / 255.0).collect();
        if let Some(rgb) = &harmonized {
            let stride = w as usize * 3;
            let rgb = rgb.get(y * stride..(y + 1) * stride).ok_or_else(|| EngineError::Other("short blended row".into()))?;
            for (pixel, rgb) in values.chunks_exact_mut(4).zip(rgb.chunks_exact(3)) {
                pixel.get_mut(..3).ok_or_else(|| EngineError::Other("short blended pixel".into()))?.copy_from_slice(rgb);
            }
        }
        source.write_region(Rect::new(0, y as i32, w as i32, y as i32 + 1), &values);
    }
    let resized = photocraft_algo::resample::resize_surface_in_canvas(
        &source,
        doc.size.width as f64 / w as f64,
        doc.size.height as f64 / h as f64,
        photocraft_algo::resample::Resample::Bilinear,
        Rect::new(0, 0, w as i32, h as i32),
    );
    let mut surface = Surface::new(fmt);
    for y in 0..doc.size.height as i32 {
        ctx.check()?;
        let r = Rect::new(0, y, doc.size.width as i32, y + 1);
        let src = resized.read_region(r);
        let mut converted = vec![0.0; doc.size.width as usize * fmt.channels()];
        transform.convert_f32(&src, 4, &mut converted, fmt.channels(), true);
        surface.write_region(r, &converted);
    }
    let mut layer = Layer::raster(name, fmt);
    *crate::pixels_mut(&mut layer)? = surface;
    if mode == EditMode::GenerativeFill {
        layer.mask = doc.selection.clone().map(|surface| LayerMask { surface, ..LayerMask::reveal_all() });
    }
    Ok(layer)
}

#[cfg(test)]
mod tests;

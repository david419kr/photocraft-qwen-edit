//! Optional, session-local Poisson postprocessing of generated fill layers.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

use photocraft_doc::{DocId, Document, Layer, LayerContent, LayerId};
use photocraft_geom::Rect;
use photocraft_raster::{Rgba8Image, Surface};
use serde_json::{Value, json};

use super::{EditMode, EditRegion};
use crate::commands::CommandSpec;
use crate::jobs::JobCtx;
use crate::{EngineError, Result, Session};

pub const COMMAND: &str = "layer.aiMatchSurroundings";
pub const LABEL: &str = "Match Surrounding Colors";
pub(crate) type Records = HashMap<(DocId, LayerId), Record>;

pub(crate) struct Record {
    source: Arc<Source>,
    // Tile identities follow COW Undo/Redo without retaining another copy of every applied result.
    states: HashMap<u64, Option<f64>>,
    created_revision: u64,
    created_action: u64,
}

pub struct Source {
    doc: Arc<Document>,
    original: Rgba8Image,
    generated: Rgba8Image,
    region: EditRegion,
    baseline: Layer,
    corrected: OnceLock<Surface>,
}

fn error(message: impl Into<String>) -> EngineError {
    EngineError::Other(message.into())
}

fn surface_identity(surface: &Surface, hash: &mut impl Hasher) {
    surface.format().hash(hash);
    for value in surface.default_pixel() {
        value.to_bits().hash(hash);
    }
    for (coord, tile) in surface.tiles() {
        coord.hash(hash);
        (tile.bytes().as_ptr() as usize).hash(hash);
    }
}

fn identity(layer: &Layer) -> Option<u64> {
    let LayerContent::Raster(pixels) = &layer.content else { return None };
    if layer.vector_mask.is_some() {
        return None;
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    surface_identity(pixels, &mut hash);
    layer.mask.is_some().hash(&mut hash);
    if let Some(mask) = &layer.mask {
        surface_identity(&mask.surface, &mut hash);
        mask.enabled.hash(&mut hash);
        mask.linked.hash(&mut hash);
        mask.density.to_bits().hash(&mut hash);
        mask.feather.to_bits().hash(&mut hash);
    }
    Some(hash.finish())
}

fn same_pixels(a: &Surface, b: &Surface) -> bool {
    a.format() == b.format()
        && a.default_pixel() == b.default_pixel()
        && a.tile_count() == b.tile_count()
        && a.tiles().zip(b.tiles()).all(|((pa, a), (pb, b))| pa == pb && (std::ptr::eq(a.bytes(), b.bytes()) || a.bytes() == b.bytes()))
}

impl Source {
    pub(super) fn new(doc: Arc<Document>, original: Rgba8Image, generated: Rgba8Image, region: EditRegion, baseline: Layer) -> Self {
        Self { doc, original, generated, region, baseline, corrected: OnceLock::new() }
    }

    fn check_document(&self, doc: &Document) -> Result<()> {
        if doc.id != self.doc.id
            || doc.size != self.doc.size
            || doc.mode != self.doc.mode
            || doc.depth != self.doc.depth
            || doc.icc_profile != self.doc.icc_profile
        {
            return Err(error("The generated image's size or color profile has changed"));
        }
        Ok(())
    }

    fn full_strength(&self, ctx: &JobCtx) -> Result<Surface> {
        ctx.check()?;
        if let Some(pixels) = self.corrected.get() {
            return Ok(pixels.clone());
        }
        let [width, height] = self.region.content_size;
        let (w, h) = (width as usize, height as usize);
        let count = w.checked_mul(h).filter(|n| *n > 0 && *n <= (super::MAX_WORKING_SIDE as usize).pow(2)).ok_or_else(|| error("Invalid fill size"))?;
        let pw = self.original.width as usize;
        let expected = pw.checked_mul(self.original.height as usize).and_then(|n| n.checked_mul(4)).ok_or_else(|| error("Invalid fill buffer"))?;
        if width > self.original.width
            || height > self.original.height
            || self.generated.width != self.original.width
            || self.generated.height != self.original.height
            || self.original.pixels.len() != expected
            || self.generated.pixels.len() != expected
        {
            return Err(error("Invalid fill buffer"));
        }
        let mut difference = Vec::with_capacity(count * 3);
        let mut hole = Vec::with_capacity(count);
        for y in 0..h {
            ctx.check()?;
            let range = y * pw * 4..(y * pw + w) * 4;
            let original = self.original.pixels.get(range.clone()).ok_or_else(|| error("Invalid source row"))?;
            let generated = self.generated.pixels.get(range).ok_or_else(|| error("Invalid generated row"))?;
            for (a, b) in original.as_chunks::<4>().0.iter().zip(generated.as_chunks::<4>().0) {
                let selected = a[3] < 255;
                hole.push(selected);
                difference.extend(a.iter().zip(b).take(3).map(|(a, b)| if selected { 0.0 } else { (f32::from(*a) - f32::from(*b)) / 255.0 }));
            }
        }
        if hole.iter().all(|selected| *selected) {
            return Err(error("No surrounding pixels are available for color matching"));
        }
        let correction = ctx
            .stage(0.05, 0.7, "Matching surrounding colors", |ctl| photocraft_algo::poisson::membrane_fill_with(w, h, 3, &difference, &hole, ctl))
            .map_err(|e| error(e.to_string()))?;
        let mut image = self.generated.clone();
        for y in 0..h {
            ctx.check()?;
            let row = image.pixels.get_mut(y * pw * 4..(y * pw + w) * 4).ok_or_else(|| error("Invalid result row"))?;
            let offsets = correction.get(y * w * 3..(y + 1) * w * 3).ok_or_else(|| error("Invalid correction row"))?;
            for (pixel, offset) in row.as_chunks_mut::<4>().0.iter_mut().zip(offsets.as_chunks::<3>().0) {
                for (v, delta) in pixel.iter_mut().take(3).zip(offset) {
                    *v = ((f32::from(*v) / 255.0 + delta).clamp(0.0, 1.0) * 255.0).round() as u8;
                }
            }
        }
        let layer = super::result_layer(&self.doc, &image, &self.region, "", EditMode::GenerativeFill, ctx)?;
        let LayerContent::Raster(pixels) = layer.content else { return Err(error("Invalid fill layer")) };
        ctx.check()?;
        let _ = self.corrected.set(pixels.clone());
        Ok(pixels)
    }

    fn output(&self, strength: f64, ctx: &JobCtx) -> Result<Surface> {
        ctx.check()?;
        let LayerContent::Raster(baseline) = &self.baseline.content else { return Err(error("Invalid fill layer")) };
        if strength == 0.0 {
            return Ok(baseline.clone());
        }
        let corrected = self.full_strength(ctx)?;
        if strength == 100.0 {
            return Ok(corrected);
        }
        let amount = strength as f32 / 100.0;
        let mut result = baseline.clone();
        let bounds = baseline.content_bounds().intersect(&self.doc.bounds());
        let channels = baseline.format().channels();
        for y in bounds.y0..bounds.y1 {
            ctx.check()?;
            let area = Rect::new(bounds.x0, y, bounds.x1, y + 1);
            let mut row = baseline.read_region(area);
            let full = corrected.read_region(area);
            for (a, b) in row.chunks_exact_mut(channels).zip(full.chunks_exact(channels)) {
                // Alpha and the layer mask remain the original generated layer's.
                for (a, b) in a.iter_mut().zip(b).take(channels.saturating_sub(1)) {
                    *a += (*b - *a) * amount;
                }
            }
            result.write_region(area, &row);
        }
        Ok(result)
    }
}

pub(super) fn remember(s: &mut Session, layer: LayerId, source: Arc<Source>) {
    if let Some((doc, revision, key)) = s.active().and_then(|st| st.doc.layer(layer).and_then(identity).map(|key| (st.doc.id, st.revision, key))) {
        s.ai_fill_sources
            .insert((doc, layer), Record { source, states: HashMap::from([(key, None)]), created_revision: revision, created_action: s.action_epoch });
    }
}

/// The bottom-bar suggestion lasts only until the next action after fill completion.
pub fn immediate(s: &Session) -> bool {
    let Some(st) = s.active() else { return false };
    let Some(layer) = st.active_layer else { return false };
    record(s, layer).is_ok_and(|(entry, _)| entry.created_revision == st.revision && entry.created_action == s.action_epoch)
}

pub fn tracked(s: &Session, layer: LayerId) -> bool {
    s.active().is_some_and(|st| s.ai_fill_sources.contains_key(&(st.doc.id, layer)))
}

fn record(s: &Session, layer: LayerId) -> Result<(&Record, u64)> {
    let st = s.active().ok_or(EngineError::NoDocument)?;
    let record = s.ai_fill_sources.get(&(st.doc.id, layer)).ok_or_else(|| error("Select a fill layer generated in this session"))?;
    record.source.check_document(&st.doc)?;
    let target = st.doc.layer(layer).ok_or_else(|| error("Generated layer was removed"))?;
    crate::commands::check_pixels_unlocked(&st.doc, layer)?;
    let key = identity(target)
        .filter(|key| record.states.contains_key(key))
        .ok_or_else(|| error("The generated layer's pixels or mask have changed; undo those edits first"))?;
    Ok((record, key))
}

pub fn available(s: &Session, layer: LayerId) -> bool {
    record(s, layer).is_ok()
}

pub fn current_strength(s: &Session, layer: LayerId) -> Option<f64> {
    let (record, key) = record(s, layer).ok()?;
    Some(record.states.get(&key).copied().flatten().unwrap_or(100.0))
}

pub fn preview_source(s: &Session, layer: Option<LayerId>) -> Option<Arc<Source>> {
    Some(record(s, layer?).ok()?.0.source.clone())
}

fn strength(params: &Value) -> Result<f64> {
    if !params.is_object() || params.as_object().is_some_and(|p| p.keys().any(|key| key != "strength")) {
        return Err(error("Use {strength:0..100}"));
    }
    params
        .get("strength")
        .map_or(Some(100.0), Value::as_f64)
        .filter(|v| v.is_finite() && (0.0..=100.0).contains(v))
        .ok_or_else(|| error("Strength must be within 0..100"))
}

/// The shared preview/commit computation. The caller freezes the source and target revision.
pub fn preview(doc: &Document, layer: Option<LayerId>, params: &Value, source: &Source, ctx: &JobCtx) -> Result<Document> {
    source.check_document(doc)?;
    let pixels = source.output(strength(params)?, ctx)?;
    let mut result = doc.clone();
    let target = result.layer_mut(layer.ok_or_else(|| error("No generated layer selected"))?).ok_or_else(|| error("Generated layer was removed"))?;
    target.content = LayerContent::Raster(pixels);
    Ok(result)
}

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: COMMAND,
        label: LABEL,
        menu: &[],
        shortcut: None,
        params: "{\"strength\":0..100 %=100} → {layer,strength}; optional Poisson correction from the original generated fill, one undo step",
        enabled: |s| {
            let layer = s.active().and_then(|st| st.active_layer).ok_or("No generated layer selected")?;
            record(s, layer).map(|_| ()).map_err(|e| e.to_string())
        },
        run: |s, p| {
            let amount = strength(p)?;
            let layer = s.active().and_then(|st| st.active_layer).ok_or_else(|| error("No generated layer selected"))?;
            let (entry, key) = record(s, layer)?;
            let source = entry.source.clone();
            let previous = entry.states.get(&key).copied().flatten().unwrap_or(0.0);
            let current = s.active().and_then(|st| st.doc.layer(layer)).and_then(Layer::surface).ok_or_else(|| error("Invalid generated layer"))?.clone();
            crate::jobs::run(
                s,
                "Match Surrounding Colors",
                true,
                move |ctx| {
                    // A purged history can let an allocator reuse an old tile address. Verify
                    // actual pixels before replacing them, not just the fast UI identity key.
                    let expected = source.output(previous, ctx)?;
                    if !same_pixels(&current, &expected) {
                        return Err(error("The generated layer's pixels changed; color matching would overwrite them"));
                    }
                    drop(expected);
                    source.output(amount, ctx)
                },
                move |s, pixels| {
                    let (entry, _) = record(s, layer)?;
                    let source = entry.source.clone();
                    s.edit("Match Surrounding Colors", move |doc, _| {
                        source.check_document(doc)?;
                        let target = doc.layer_mut(layer).ok_or_else(|| error("Generated layer was removed"))?;
                        target.content = LayerContent::Raster(pixels);
                        Ok(())
                    })?;
                    let (doc_id, revision, key) = {
                        let st = s.active().ok_or(EngineError::NoDocument)?;
                        (st.doc.id, st.revision, st.doc.layer(layer).and_then(identity).ok_or_else(|| error("Invalid generated layer"))?)
                    };
                    if let Some(record) = s.ai_fill_sources.get_mut(&(doc_id, layer)) {
                        record.states.insert(key, Some(amount));
                    }
                    if let Some(last) = s.ai_runs.get_mut(&doc_id)
                        && last.result_layer == layer
                    {
                        last.result_revision = revision;
                    }
                    Ok(json!({"layer":layer.0,"strength":amount}))
                },
            )
        },
        journal: true,
    }]
}

#[cfg(test)]
mod tests;

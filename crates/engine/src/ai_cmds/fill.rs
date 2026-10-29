//! Selection-local input and inverse mapping; model guide and composite coverage are distinct.

use super::{
    Builtin, ColorMode, Document, EditRegion, EngineError, Intent, JobCtx, Layer, LayerMask, PixelFormat, Prepared, Rect, Result, Rgba8Image, Surface,
    Transform, byte, resolution,
};
use photocraft_algo::resample::{Resample, resize_surface_in_canvas};
use photocraft_color::SampleType;

const MIN_CROP: u32 = 64;
const MIN_WORKING_SIDE: u32 = 512;
mod groups;

pub(super) fn prepare_groups(doc: &Document, ctx: &JobCtx) -> Result<Vec<Prepared>> {
    let (selected, prompt_regions) = selection_info(doc, ctx)?;
    groups::plan(doc, selected, ctx)?.into_iter().map(|area| prepare_area(doc, area, prompt_regions.clone(), ctx)).collect()
}

pub(super) fn output_filter(interpolation: crate::prefs::Interpolation) -> Resample {
    use crate::prefs::Interpolation;
    match interpolation {
        Interpolation::Nearest => Resample::Nearest,
        Interpolation::Bilinear => Resample::Bilinear,
        Interpolation::Bicubic => Resample::Bicubic,
        Interpolation::BicubicSmoother => Resample::BicubicSmoother,
        Interpolation::BicubicSharper => Resample::BicubicSharper,
        Interpolation::BicubicAutomatic => Resample::BicubicAutomatic,
        Interpolation::PreserveDetails => Resample::PreserveDetails,
    }
}

fn gray_float() -> PixelFormat {
    PixelFormat::new(ColorMode::Grayscale, SampleType::F32, false)
}

fn selection_info(doc: &Document, ctx: &JobCtx) -> Result<(Rect, Vec<crate::ai_prompts::Region>)> {
    let selection = doc.selection.as_ref().ok_or_else(|| EngineError::Other("select an area first".into()))?;
    let available =
        if selection.default_pixel().first().is_some_and(|v| *v != 0.0) { doc.bounds() } else { selection.content_bounds().intersect(&doc.bounds()) };
    let mut bounds = Rect::EMPTY;
    let mut weights = [0u64; 9];
    let channels = selection.format().channels().max(1);
    for y in available.y0..available.y1 {
        ctx.check()?;
        let row = selection.read_region(Rect::new(available.x0, y, available.x1, y + 1));
        for (x, pixel) in (available.x0..available.x1).zip(row.chunks_exact(channels)) {
            let value = pixel.first().copied().unwrap_or(0.0);
            if !value.is_finite() {
                return Err(EngineError::Other("invalid selection coverage".into()));
            }
            let weight = byte(value);
            if value > 0.0 {
                bounds = bounds.union(&Rect::new(x, y, x + 1, y + 1));
                let cell = (y as u64 * 3 / u64::from(doc.size.height)).min(2) * 3 + (x as u64 * 3 / u64::from(doc.size.width)).min(2);
                if let Some(total) = weights.get_mut(cell as usize) {
                    *total = total.saturating_add(u64::from(weight.max(1)));
                }
            }
        }
    }
    if bounds.is_empty() {
        return Err(EngineError::Other("the selected area has no representable coverage".into()));
    }
    let max = weights.iter().copied().max().unwrap_or(0);
    let regions = weights
        .into_iter()
        .enumerate()
        .filter(|(_, weight)| *weight > 0 && *weight >= max / 10)
        .filter_map(|(cell, _)| crate::ai_prompts::Region::ALL.get(cell).copied())
        .collect();
    Ok((bounds, regions))
}

fn crop(selection: Rect, canvas: Rect) -> Rect {
    let (w, h) = (f64::from(selection.width()), f64::from(selection.height()));
    // Retain the old square context for compact selections; long selections keep rectangular
    // context without starving the short axis of nearby structure.
    let diameter = ((((w + h).powi(2) + 60.0 * w * h).sqrt() - w - h) / 2.0).round() as u32;
    let selected_side = selection.width().max(selection.height());
    let padding = (diameter / 2).saturating_mul(2);
    let (width, height) = if u64::from(selected_side) * 3 <= u64::from(selection.width().min(selection.height())) * 4 {
        let side = selected_side.saturating_add(padding).max(MIN_CROP);
        (side, side)
    } else {
        let mut width = selection.width().saturating_add(padding).max(MIN_CROP);
        let mut height = selection.height().saturating_add(padding).max(MIN_CROP);
        if width > height.saturating_mul(4) {
            height = width.div_ceil(4);
        }
        if height > width.saturating_mul(4) {
            width = height.div_ceil(4);
        }
        // Minimum-size clamping can otherwise make an elongated tiny selection square.
        if width == height {
            if selection.width() > selection.height() {
                width = width.saturating_add(1);
            } else {
                height = height.saturating_add(1);
            }
        }
        (width, height)
    };
    let width = width.min(canvas.width());
    let height = height.min(canvas.height());
    let x = ((i64::from(selection.x0) + i64::from(selection.x1) - i64::from(width)) / 2).clamp(i64::from(canvas.x0), i64::from(canvas.x1) - i64::from(width))
        as i32;
    let y = ((i64::from(selection.y0) + i64::from(selection.y1) - i64::from(height)) / 2).clamp(i64::from(canvas.y0), i64::from(canvas.y1) - i64::from(height))
        as i32;
    Rect::from_xywh(x, y, width, height)
}

pub(super) fn prepare(doc: &Document, ctx: &JobCtx) -> Result<Prepared> {
    let (selected, prompt_regions) = selection_info(doc, ctx)?;
    prepare_area(doc, selected, prompt_regions, ctx)
}

fn prepare_area(doc: &Document, selected: Rect, prompt_regions: Vec<crate::ai_prompts::Region>, ctx: &JobCtx) -> Result<Prepared> {
    let bounds = crop(selected, doc.bounds());
    let frame = resolution::Budget::EDIT
        .plan([bounds.width(), bounds.height()], MIN_WORKING_SIDE.min(doc.size.width.max(doc.size.height)))
        .ok_or_else(|| EngineError::Other("invalid fill frame".into()))?;
    let [w, h] = frame.content;
    let local = Rect::from_xywh(0, 0, bounds.width(), bounds.height());
    let (sx, sy) = (f64::from(w) / f64::from(bounds.width()), f64::from(h) / f64::from(bounds.height()));
    let transform = Transform::new(&crate::color_cmds::composite_profile(doc), Builtin::Srgb.profile(), Intent::RelativeColorimetric, true)
        .map_err(|e| EngineError::Other(e.to_string()))?;
    let mut source = Surface::new(PixelFormat::RGBA32F);
    photocraft_compose::render_bands(doc, bounds, 128, |band| -> Result<()> {
        ctx.check()?;
        let pixels: Vec<f32> = band.px.into_iter().flatten().collect();
        let mut converted = vec![0.0; pixels.len()];
        transform.convert_f32(&pixels, 4, &mut converted, 4, true);
        source.write_region(band.rect.translate(-bounds.x0, -bounds.y0), &converted);
        Ok(())
    })?;
    ctx.check()?;
    let reduced = resize_surface_in_canvas(&source, sx, sy, Resample::Bicubic, local);
    drop(source);
    ctx.check()?;

    // Conservative coverage uses the very same source pixel edges as RGB resampling. A thin
    // selection must not disappear between nearest sample centers or through negative cubic lobes.
    let selection = doc.selection.as_ref().ok_or_else(|| EngineError::Other("select an area first".into()))?;
    let channels = selection.format().channels().max(1);
    let mut coverage = vec![0.0f32; w as usize * h as usize];
    // Final coverage stays in document coordinates and keeps the user's feather exactly once.
    let mut mask = Surface::new(gray_float());
    for y in selected.y0..selected.y1 {
        ctx.check()?;
        let area = Rect::new(selected.x0, y, selected.x1, y + 1);
        let row = selection.read_region(area);
        let values: Vec<f32> = row.chunks_exact(channels).map(|p| p.first().copied().unwrap_or(0.0).clamp(0.0, 1.0)).collect();
        mask.write_region(area, &values);
        let y0 = (f64::from(y - bounds.y0) * sy).floor() as usize;
        let y1 = (f64::from(y + 1 - bounds.y0) * sy).ceil() as usize;
        for (x, pixel) in (selected.x0..selected.x1).zip(row.chunks_exact(channels)) {
            if pixel.first().copied().unwrap_or(0.0) <= 0.0 {
                continue;
            }
            let x0 = (f64::from(x - bounds.x0) * sx).floor() as usize;
            let x1 = ((f64::from(x + 1 - bounds.x0) * sx).ceil() as usize).min(w as usize);
            for my in y0..y1.min(h as usize) {
                if let Some(span) = coverage.get_mut(my * w as usize + x0..my * w as usize + x1) {
                    span.fill(1.0);
                }
            }
        }
    }
    mask.prune();
    let (pw, ph) = (w.div_ceil(32) * 32, h.div_ceil(32) * 32);
    let mut image = Rgba8Image::new(pw, ph);
    for y in 0..ph {
        ctx.check()?;
        let py = y.min(h - 1);
        let rgb = reduced.read_region(Rect::new(0, py as i32, w as i32, py as i32 + 1));
        let guide_row =
            coverage.get(py as usize * w as usize..(py as usize + 1) * w as usize).ok_or_else(|| EngineError::Other("incomplete fill mask".into()))?;
        let output = image
            .pixels
            .get_mut(y as usize * pw as usize * 4..(y as usize + 1) * pw as usize * 4)
            .ok_or_else(|| EngineError::Other("incomplete fill image".into()))?;
        for (x, pixel) in output.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let x_source = x.min(w as usize - 1);
            let rgba = rgb.as_chunks::<4>().0.get(x_source).ok_or_else(|| EngineError::Other("incomplete fill composite".into()))?;
            let alpha = rgba[3].clamp(0.0, 1.0);
            for (out, value) in pixel.iter_mut().take(3).zip(rgba.iter()) {
                *out = byte(*value * alpha + 1.0 - alpha);
            }
            pixel[3] = if x < w as usize && y < h { byte(1.0 - guide_row.get(x).copied().unwrap_or(0.0)) } else { 255 };
        }
    }
    Ok(Prepared { image, region: EditRegion { bounds, content_size: [w, h], mask: Some(mask), prompt_regions, filter: Resample::Bicubic } })
}

/// Copy disjoint native patches without reapplying selection coverage or resampling.
pub(super) fn merge(doc: &Document, layers: Vec<Layer>, name: &str, ctx: &JobCtx) -> Result<Layer> {
    if layers.is_empty() || layers.len() > groups::MAX_GROUPS {
        return Err(EngineError::Other("invalid fill group count".into()));
    }
    let mut result = Layer::raster(name, PixelFormat::new(doc.mode, doc.depth, true));
    let mut mask = Surface::new(gray_float());
    let mut occupied = Vec::new();
    for layer in layers {
        ctx.check()?;
        let pixels = layer.surface().ok_or_else(|| EngineError::Other("invalid fill pixels".into()))?;
        let coverage = &layer.mask.as_ref().ok_or_else(|| EngineError::Other("missing fill coverage".into()))?.surface;
        let bounds = coverage.content_bounds().intersect(&doc.bounds());
        if occupied.iter().any(|old: &Rect| !old.intersect(&bounds).is_empty()) {
            return Err(EngineError::Other("overlapping fill group support".into()));
        }
        occupied.push(bounds);
        for y in bounds.y0..bounds.y1 {
            ctx.check()?;
            let row = Rect::new(bounds.x0, y, bounds.x1, y + 1);
            crate::pixels_mut(&mut result)?.write_region(row, &pixels.read_region(row));
            mask.write_region(row, &coverage.read_region(row));
        }
    }
    result.mask = Some(LayerMask { surface: mask, ..LayerMask::reveal_all() });
    Ok(result)
}

#[cfg(test)]
mod groups_tests;
#[cfg(test)]
mod tests;

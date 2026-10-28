//! Sampler state is a coarse RGBA matte; RGB conditioning remains the original crop.
use super::*;

pub(super) fn prepare(source: &Rgba8Image, coarse: &Surface, bounds: Rect, ctx: &JobCtx) -> Result<AlphaSampling> {
    let [w, h] = [bounds.width(), bounds.height()];
    if !resolution::Budget::EDIT.accepts_content([w, h])
        || resolution::padded([w, h]) != Some([source.width, source.height])
        || source.pixels.len() != source.width as usize * source.height as usize * 4
    {
        return Err(EngineError::Other("Invalid alpha sampling geometry".into()));
    }
    let (width, height) = (w as usize, h as usize);
    let pw = source.width as usize;
    let values = coarse.read_region(bounds);
    let mut initial = source.clone();
    for (y, row) in initial.pixels.chunks_exact_mut(pw * 4).enumerate() {
        ctx.check()?;
        for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let a = *values.get(y.min(height - 1) * width + x.min(width - 1)).ok_or_else(|| EngineError::Other("Missing coarse alpha sample".into()))?;
            if !a.is_finite() {
                return Err(EngineError::Other("Invalid coarse alpha".into()));
            }
            p[3] = byte(a).min(p[3]);
        }
    }
    let mut foreground = Vec::with_capacity(width * height);
    let mut edge = vec![0.0; width * height];
    let pixels = source.pixels.as_chunks::<4>().0;
    for y in 0..height {
        ctx.check()?;
        for x in 0..width {
            let p = pixels.get(y * pw + x).ok_or_else(|| EngineError::Other("Missing source sample".into()))?;
            let a = initial.pixels.get((y * pw + x) * 4 + 3).copied().ok_or_else(|| EngineError::Other("Missing alpha sample".into()))?;
            foreground.push(if a >= 128 { 1.0 } else { 0.0 });
            for neighbour in [x.checked_add(1).filter(|v| *v < width).map(|v| y * pw + v), y.checked_add(1).filter(|v| *v < height).map(|v| v * pw + x)]
                .into_iter()
                .flatten()
            {
                let q = pixels.get(neighbour).ok_or_else(|| EngineError::Other("Missing neighbour sample".into()))?;
                if p.iter().zip(q).take(3).any(|(a, b)| a.abs_diff(*b) > 26)
                    && let Some(v) = edge.get_mut(y * width + x)
                {
                    *v = 1.0;
                }
            }
        }
    }
    // The binary core is for work planning, never a threshold applied to final opacity.
    let grown = photocraft_algo::matting::morph(&foreground, width, height, 32, true);
    ctx.check()?;
    let eroded = photocraft_algo::matting::morph(&foreground, width, height, 32, false);
    ctx.check()?;
    let detail = photocraft_algo::matting::morph(&edge, width, height, 8, true);
    ctx.check()?;
    let mut noise = Rgba8Image::new(source.width, source.height);
    for (y, row) in noise.pixels.chunks_exact_mut(pw * 4).enumerate() {
        ctx.check()?;
        for (x, p) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let selected = if x < width && y < height {
                let i = y * width + x;
                let a = initial.pixels.get((y * pw + x) * 4 + 3).copied().unwrap_or(0);
                grown.get(i).copied().unwrap_or(0.0) > eroded.get(i).copied().unwrap_or(0.0)
                    || (a > 12 && a < 243)
                    || detail.get(i).copied().unwrap_or(0.0) > 0.0
            } else {
                false
            };
            let value = if selected { 255 } else { 0 };
            p.copy_from_slice(&[value, value, value, 255]);
        }
    }
    Ok(AlphaSampling { initial, noise, denoise: 0.25 })
}

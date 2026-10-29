//! Native cores share context halos; only alpha is assembled, never generated colour.
use super::{AlphaSampling, ColorMode, EngineError, JobCtx, MAX_DOCUMENT_PIXELS, PixelFormat, Rect, Result, Rgba8Image, Surface, resolution};

const CORE: u32 = 1536;
const OVERLAP: u32 = 128;
const HALO: i32 = 256;
const STRIDE: u32 = CORE - OVERLAP;
const MAX_TILES: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Tile {
    pub core: Rect,
    pub context: Rect,
}

pub(super) fn plan(bounds: Rect) -> Result<Vec<Tile>> {
    if bounds.x0 != 0 || bounds.y0 != 0 || bounds.is_empty() || u64::from(bounds.width()) * u64::from(bounds.height()) > MAX_DOCUMENT_PIXELS {
        return Err(EngineError::Other("Invalid background removal dimensions".into()));
    }
    if resolution::Budget::EDIT.accepts_content([bounds.width(), bounds.height()]) {
        return Ok(Vec::new());
    }
    let count = |length: u32| if length <= CORE { 1 } else { (length - CORE).div_ceil(STRIDE) + 1 };
    let (nx, ny) = (count(bounds.width()), count(bounds.height()));
    if u64::from(nx) * u64::from(ny) > MAX_TILES as u64 {
        return Err(EngineError::Other("This image requires too many background-removal regions; the original layer was preserved".into()));
    }
    let mut tiles = Vec::new();
    for y in 0..ny {
        for x in 0..nx {
            let x0 = bounds.x0 + (x * STRIDE) as i32;
            let y0 = bounds.y0 + (y * STRIDE) as i32;
            let core = Rect::new(x0, y0, (x0 + CORE as i32).min(bounds.x1), (y0 + CORE as i32).min(bounds.y1));
            let context = Rect::new(core.x0 - HALO, core.y0 - HALO, core.x1 + HALO, core.y1 + HALO).intersect(&bounds);
            if !resolution::Budget::EDIT.accepts_content([context.width(), context.height()]) {
                return Err(EngineError::Other("Invalid background-removal tile budget".into()));
            }
            tiles.push(Tile { core, context });
        }
    }
    Ok(tiles)
}

fn weight(position: i32, start: i32, end: i32, limit_start: i32, limit_end: i32) -> f32 {
    let left = if start > limit_start { ((position - start) as f32 + 0.5) / OVERLAP as f32 } else { 1.0 };
    let right = if end < limit_end { ((end - position) as f32 - 0.5) / OVERLAP as f32 } else { 1.0 };
    left.min(right).clamp(f32::EPSILON, 1.0)
}

pub(super) struct Blend {
    bounds: Rect,
    sum: Vec<f32>,
    weight: Vec<f32>,
    extremes: Vec<u8>,
}

impl Blend {
    pub fn new(bounds: Rect) -> Result<Self> {
        let count = (bounds.width() as usize)
            .checked_mul(bounds.height() as usize)
            .filter(|n| *n > 0 && *n <= MAX_DOCUMENT_PIXELS as usize)
            .ok_or_else(|| EngineError::Other("Invalid mask allocation".into()))?;
        Ok(Self { bounds, sum: vec![0.0; count], weight: vec![0.0; count], extremes: vec![0; count] })
    }

    pub fn add(&mut self, tile: Tile, image: &Rgba8Image, restore: Option<(&AlphaSampling, &Surface)>, ctx: &JobCtx) -> Result<()> {
        let Some([pw, ph]) = resolution::padded([tile.context.width(), tile.context.height()]) else {
            return Err(EngineError::Other("Invalid mask tile".into()));
        };
        if !self.bounds.contains_rect(&tile.context)
            || !tile.context.contains_rect(&tile.core)
            || tile.core.is_empty()
            || !resolution::Budget::EDIT.accepts_content([tile.context.width(), tile.context.height()])
            || image.width != pw
            || image.height != ph
            || image.pixels.len() != pw as usize * ph as usize * 4
        {
            return Err(EngineError::Other("Unexpected background-removal tile dimensions".into()));
        }
        if restore.is_some_and(|(s, c)| s.noise.width != pw || s.noise.height != ph || s.noise.pixels.len() != image.pixels.len() || c.channels() != 1) {
            return Err(EngineError::Other("Invalid alpha restoration geometry".into()));
        }
        for y in tile.core.y0..tile.core.y1 {
            ctx.check()?;
            let original = restore.map(|(_, coarse)| coarse.read_region(Rect::new(tile.core.x0, y, tile.core.x1, y + 1)));
            let wy = weight(y, tile.core.y0, tile.core.y1, self.bounds.y0, self.bounds.y1);
            for x in tile.core.x0..tile.core.x1 {
                let index = (y - self.bounds.y0) as usize * self.bounds.width() as usize + (x - self.bounds.x0) as usize;
                let input = ((y - tile.context.y0) as usize * pw as usize + (x - tile.context.x0) as usize) * 4 + 3;
                let a = if restore.is_some_and(|(sampling, _)| sampling.noise.pixels.get(input - 3) == Some(&0)) {
                    original
                        .as_ref()
                        .and_then(|r| r.get((x - tile.core.x0) as usize))
                        .copied()
                        .ok_or_else(|| EngineError::Other("Missing original coarse alpha".into()))?
                } else {
                    f32::from(*image.pixels.get(input).ok_or_else(|| EngineError::Other("Short mask tile".into()))?) / 255.0
                };
                if !a.is_finite() {
                    return Err(EngineError::Other("Invalid restored alpha".into()));
                }
                let w = wy * weight(x, tile.core.x0, tile.core.x1, self.bounds.x0, self.bounds.x1);
                *self.sum.get_mut(index).ok_or_else(|| EngineError::Other("Invalid mask coverage".into()))? += w * a;
                *self.weight.get_mut(index).ok_or_else(|| EngineError::Other("Invalid mask coverage".into()))? += w;
                let flags = self.extremes.get_mut(index).ok_or_else(|| EngineError::Other("Invalid mask coverage".into()))?;
                *flags |= if a < 0.1 {
                    1
                } else if a > 0.9 {
                    2
                } else {
                    0
                };
            }
        }
        Ok(())
    }

    pub fn finish(self, ctx: &JobCtx) -> Result<Surface> {
        let mut surface = Surface::new(PixelFormat::new(ColorMode::Grayscale, photocraft_color::SampleType::F32, false));
        for y in self.bounds.y0..self.bounds.y1 {
            ctx.check()?;
            let start = (y - self.bounds.y0) as usize * self.bounds.width() as usize;
            let end = start + self.bounds.width() as usize;
            let sums = self.sum.get(start..end).ok_or_else(|| EngineError::Other("Invalid mask row".into()))?;
            let weights = self.weight.get(start..end).ok_or_else(|| EngineError::Other("Invalid mask row".into()))?;
            let extremes = self.extremes.get(start..end).ok_or_else(|| EngineError::Other("Invalid mask row".into()))?;
            let mut row = Vec::with_capacity(self.bounds.width() as usize);
            for ((sum, w), flags) in sums.iter().zip(weights).zip(extremes) {
                if !w.is_finite() || *w <= 0.0 || !sum.is_finite() {
                    return Err(EngineError::Other("Background-removal regions did not cover the image; the original layer was preserved".into()));
                }
                // Never conceal opposite foreground decisions as fabricated 50% opacity.
                if *flags == 3 {
                    return Err(EngineError::Other("Background-removal regions disagree about the subject; the original layer was preserved".into()));
                }
                row.push((*sum / *w).clamp(0.0, 1.0));
            }
            surface.write_region(Rect::new(self.bounds.x0, y, self.bounds.x1, y + 1), &row);
        }
        Ok(surface)
    }
}

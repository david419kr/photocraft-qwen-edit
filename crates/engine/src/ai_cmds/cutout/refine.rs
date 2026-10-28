//! Global identity plus a crop-local alpha guide, using the same Qwen model and job.
use super::*;

const PROMPT: &str = "Refine the background-removal alpha for <image1>, which is a detail crop of the complete original in <image2>. <image2> provides the global foreground identity and scene context. <image3> is the current grayscale alpha guide for exactly the crop in <image1>: white keeps foreground, black removes background, gray is partial opacity. Correct the guide using fine details visible in <image1> and the global context. Keep every part belonging to the globally selected foreground subjects, regardless of category. Remove only background and gaps. All opaque foreground surfaces must stay fully opaque regardless of brightness or shading; preserve genuinely translucent materials. Return only the RGBA cutout of <image1> at its original crop dimensions, positions and colors. Do not output the full image, guide, collage, text or extra objects.";

fn guide(opacity: &Surface, bounds: Rect, ctx: &JobCtx) -> Result<Rgba8Image> {
    let frame =
        resolution::Budget::REFERENCE.plan([bounds.width(), bounds.height()], 0).ok_or_else(|| EngineError::Other("Invalid refinement guide size".into()))?;
    let [w, h] = frame.content;
    let local = opacity.translated(-bounds.x0, -bounds.y0, bounds);
    let resized = resize_surface_in_canvas(
        &local,
        f64::from(w) / f64::from(bounds.width()),
        f64::from(h) / f64::from(bounds.height()),
        Resample::Bilinear,
        Rect::from_xywh(0, 0, bounds.width(), bounds.height()),
    );
    let [pw, ph] = frame.padded;
    let mut image = Rgba8Image::new(pw, ph);
    for y in 0..ph {
        ctx.check()?;
        let sy = y.min(h - 1);
        let row = resized.read_region(Rect::new(0, sy as i32, w as i32, sy as i32 + 1));
        let output = image
            .pixels
            .get_mut(y as usize * pw as usize * 4..(y + 1) as usize * pw as usize * 4)
            .ok_or_else(|| EngineError::Other("Invalid refinement guide row".into()))?;
        for (x, p) in output.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let value = byte(*row.get(x.min(w as usize - 1)).ok_or_else(|| EngineError::Other("Invalid refinement guide sample".into()))?);
            p.copy_from_slice(&[value, value, value, 255]);
        }
    }
    Ok(image)
}

pub(super) fn generate(doc: &Document, id: LayerId, backend: &dyn AiBackend, request: &AiRequest, region: &EditRegion, ctx: &JobCtx) -> Result<Layer> {
    let plan = tiles::plan(doc.bounds())?;
    if plan.is_empty() {
        let output = backend.generate(request, ctx)?;
        return result(doc, id, &output, region, ctx);
    }
    let coarse_ctx = ctx.subtask(0.04, 0.18, "Finding foreground");
    let output = backend.generate(request, &coarse_ctx)?;
    let coarse = opacity(doc, &output, region, &coarse_ctx)?;
    drop(output);
    let context = Arc::new(request.image.clone());
    let mut blend = tiles::Blend::new(doc.bounds())?;
    let count = plan.len() as f32;
    // Cover every native core. A saturated coarse alpha cannot prove that a tiny subject,
    // hole or line vanished during downsampling; it is not a calibrated confidence score.
    for (index, tile) in plan.into_iter().enumerate() {
        ctx.check()?;
        let pass_ctx = ctx.subtask(
            0.18 + 0.75 * index as f32 / count,
            0.18 + 0.75 * (index + 1) as f32 / count,
            &format!("Refining region {}/{}", index + 1, count as usize),
        );
        let input = prepare_rect(doc, id, tile.context, [tile.context.width(), tile.context.height()], &pass_ctx)?;
        let mut pass = request.clone();
        pass.image = input.image;
        pass.reference = Some(context.clone());
        pass.guide = Some(Arc::new(guide(&coarse, tile.context, &pass_ctx)?));
        pass.prompt = PROMPT.into();
        pass.seed = super::super::fill_batch::seed(request.seed, index + 1);
        let result = backend.generate(&pass, &pass_ctx)?;
        blend.add(tile, &result, &pass_ctx)?;
    }
    ctx.check()?;
    let alpha = blend.finish(ctx)?;
    from_opacity(doc, id, alpha, ctx)
}

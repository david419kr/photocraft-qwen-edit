//! Global identity plus a crop-local alpha guide, using the same Qwen model and job.
use super::*;

const PROMPT: &str = "Refine the background-removal alpha for <image1>, which is a detail crop of the complete original in <image2>. <image2> provides the global foreground identity and scene context. <image3> is the current grayscale alpha guide for exactly the crop in <image1>: white keeps foreground, black removes background, gray is partial opacity. Correct the guide using fine details visible in <image1> and the global context. Keep every part belonging to the globally selected foreground subjects, regardless of category. Remove only background and gaps. All opaque foreground surfaces must stay fully opaque regardless of brightness or shading; preserve genuinely translucent materials. Return only the RGBA cutout of <image1> at its original crop dimensions, positions and colors. Do not output the full image, guide, collage, text or extra objects.";

pub(super) fn guide(initial: &Rgba8Image, ctx: &JobCtx) -> Result<Rgba8Image> {
    let [w, h] = [initial.width, initial.height];
    if !resolution::Budget::EDIT.accepts_frame([w, h]) || initial.pixels.len() != w as usize * h as usize * 4 {
        return Err(EngineError::Other("Invalid guide source frame".into()));
    }
    let frame = resolution::Budget::REFERENCE.plan([w, h], 0).ok_or_else(|| EngineError::Other("Invalid refinement guide size".into()))?;
    let [pw, ph] = frame.padded;
    let mut local = Surface::new(PixelFormat::new(ColorMode::Grayscale, photocraft_color::SampleType::F32, false));
    for (y, row) in initial.pixels.chunks_exact(w as usize * 4).enumerate() {
        ctx.check()?;
        let values: Vec<f32> = row.as_chunks::<4>().0.iter().map(|p| f32::from(p[3]) / 255.0).collect();
        local.write_region(Rect::new(0, y as i32, w as i32, y as i32 + 1), &values);
    }
    // Resize the entire padded canvas. Independently padding the content again shifts
    // normalized guide coordinates on rectangular/odd-sized crops.
    let resized = resize_surface_in_canvas(&local, f64::from(pw) / f64::from(w), f64::from(ph) / f64::from(h), Resample::Bilinear, Rect::from_xywh(0, 0, w, h));
    let mut image = Rgba8Image::new(pw, ph);
    for y in 0..ph {
        ctx.check()?;
        let row = resized.read_region(Rect::new(0, y as i32, pw as i32, y as i32 + 1));
        let output = image
            .pixels
            .get_mut(y as usize * pw as usize * 4..(y + 1) as usize * pw as usize * 4)
            .ok_or_else(|| EngineError::Other("Invalid refinement guide row".into()))?;
        for (x, p) in output.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let value = byte(*row.get(x).ok_or_else(|| EngineError::Other("Invalid refinement guide sample".into()))?);
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
        pass.alpha_sampling = Some(Arc::new(sampling::prepare(&pass.image, &coarse, tile.context, &pass_ctx)?));
        pass.reference = Some(context.clone());
        pass.guide =
            Some(Arc::new(guide(&pass.alpha_sampling.as_ref().ok_or_else(|| EngineError::Other("Missing initial alpha".into()))?.initial, &pass_ctx)?));
        pass.prompt = PROMPT.into();
        pass.seed = super::super::fill_batch::seed(request.seed, index + 1);
        let has_work = pass.alpha_sampling.as_ref().is_some_and(|s| {
            let core = tile.core.translate(-tile.context.x0, -tile.context.y0);
            (core.y0..core.y1).any(|y| {
                let start = (y as usize * s.noise.width as usize + core.x0 as usize) * 4;
                s.noise.pixels.get(start..start + core.width() as usize * 4).is_some_and(|row| row.as_chunks::<4>().0.iter().any(|p| p[0] > 0))
            })
        });
        let result = if has_work {
            backend.generate(&pass, &pass_ctx)?
        } else {
            pass.alpha_sampling.as_ref().map(|s| s.initial.clone()).ok_or_else(|| EngineError::Other("Missing initial alpha".into()))?
        };
        blend.add(tile, &result, pass.alpha_sampling.as_deref().map(|s| (s, &coarse)), &pass_ctx)?;
    }
    ctx.check()?;
    let alpha = blend.finish(ctx)?;
    from_opacity(doc, id, alpha, ctx)
}

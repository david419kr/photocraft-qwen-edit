//! Qwen predicts coverage; the native source raster is never replaced by generated RGB.
use super::*;
use photocraft_algo::resample::{Resample, resize_surface_in_canvas};
use photocraft_doc::{LayerContent, LayerId};

pub(super) fn check(doc: &Document, id: LayerId) -> std::result::Result<(), String> {
    let layer = doc.layer(id).ok_or("No active layer")?;
    if !matches!(layer.content, LayerContent::Raster(_)) {
        return Err("AI background removal requires a pixel layer".into());
    }
    let locks = doc.effective_locks(id);
    if locks.all || locks.pixels {
        return Err("The layer or its parent is locked".into());
    }
    Ok(())
}

pub(super) fn enabled(s: &Session) -> std::result::Result<(), String> {
    can_edit(s)?;
    check(&s.active().ok_or("No document")?.doc, crate::active_layer_of(s)?.id)
}

pub(super) fn prepare(doc: &Document, id: LayerId, ctx: &JobCtx) -> Result<Prepared> {
    check(doc, id).map_err(EngineError::Other)?;
    // Ignore existing masks, visibility, styles and other layers when detecting the subject.
    let original = doc.layer(id).ok_or(EngineError::NoLayer(id))?;
    let mut layer = Layer::raster("Cutout input", original.surface().ok_or(EngineError::NoLayer(id))?.format());
    *crate::pixels_mut(&mut layer)? = original.surface().ok_or(EngineError::NoLayer(id))?.clone();
    let mut isolated = doc.clone();
    isolated.layers = vec![layer];
    isolated.selection = None;
    super::prepare(&isolated, EditMode::PromptEdit, ctx)
}

pub(super) fn result(doc: &Document, id: LayerId, image: &Rgba8Image, region: &EditRegion, ctx: &JobCtx) -> Result<Layer> {
    ctx.check()?;
    let [w, h] = region.content_size;
    let bounds = region.bounds;
    if w == 0 || h == 0 || w > MAX_WORKING_SIDE || h > MAX_WORKING_SIDE || bounds != doc.bounds() {
        return Err(EngineError::Other("Invalid cutout geometry".into()));
    }
    let (pw, ph) = (w.div_ceil(32) * 32, h.div_ceil(32) * 32);
    if image.width != pw || image.height != ph || image.pixels.len() != pw as usize * ph as usize * 4 {
        return Err(EngineError::Other("ComfyUI returned an unexpected cutout size".into()));
    }
    let fmt = PixelFormat::new(ColorMode::Grayscale, photocraft_color::SampleType::F32, false);
    let mut alpha = Surface::new(fmt);
    let mut any = false;
    for (y, row) in image.pixels.chunks_exact(pw as usize * 4).take(h as usize).enumerate() {
        ctx.check()?;
        let values: Vec<f32> = row
            .as_chunks::<4>()
            .0
            .iter()
            .take(w as usize)
            .map(|p| {
                any |= p[3] != 0;
                f32::from(p[3]) / 255.0
            })
            .collect();
        alpha.write_region(Rect::new(0, y as i32, w as i32, y as i32 + 1), &values);
    }
    if !any {
        return Err(EngineError::Other("Qwen returned an empty foreground mask; the original layer was preserved".into()));
    }
    let surface = resize_surface_in_canvas(
        &alpha,
        bounds.width() as f64 / f64::from(w),
        bounds.height() as f64 / f64::from(h),
        Resample::Bilinear,
        Rect::new(0, 0, w as i32, h as i32),
    );
    let mask = crate::commands::mask_at_depth(LayerMask { surface, ..LayerMask::reveal_all() }, doc.depth)?;
    let mut layer = doc.layer(id).ok_or(EngineError::NoLayer(id))?.clone();
    layer.mask = Some(mask);
    Ok(layer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Coverage(AtomicUsize);
    impl AiBackend for Coverage {
        fn generate(&self, request: &AiRequest, _: &JobCtx) -> Result<Rgba8Image> {
            // The red overlay and prior zero mask must not affect subject detection.
            assert_eq!(request.image.pixels.get(..4), Some([255, 255, 255, 255].as_slice()));
            let a = if self.0.fetch_add(1, Ordering::Relaxed) == 0 { 127 } else { 64 };
            let mut result = request.image.clone();
            for p in result.pixels.as_chunks_mut::<4>().0 {
                p.copy_from_slice(&[255, 0, 255, a]);
            }
            Ok(result)
        }
    }

    #[test]
    fn preserves_native_pixels_masks_other_layers_and_reroll_undo() {
        for depth in [8, 16, 32] {
            let mut s = super::super::tests::session(depth);
            let id = s.active().unwrap().doc.layers[0].id;
            s.edit("Prior mask", |d, _| {
                d.layer_mut(id).unwrap().mask = Some(LayerMask::hide_all());
                Ok(())
            })
            .unwrap();
            let original = s.active().unwrap().doc.layer(id).unwrap().surface().unwrap().read_region(s.active().unwrap().doc.bounds());
            s.execute("layer.new.layer", json!({"name":"Overlay"})).unwrap();
            let overlay = s.active().unwrap().active_layer.unwrap();
            s.edit("Red overlay", |d, _| {
                d.layer_mut(overlay).unwrap().surface_mut().unwrap().write_region(Rect::new(0, 0, 17, 13), &[1.0, 0.0, 0.0, 1.0].repeat(17 * 13));
                Ok(())
            })
            .unwrap();
            s.execute("layer.select", json!({"layer":id.0})).unwrap();
            s.execute("select.rect", json!({"x":0,"y":0,"width":2,"height":2})).unwrap();
            s.ai_backend = Some(Arc::new(Coverage(AtomicUsize::new(0))));
            s.execute(REMOVE_BACKGROUND_COMMAND, json!({})).unwrap();
            let d = &s.active().unwrap().doc;
            assert_eq!(d.layers.len(), 2);
            assert!(d.layer(overlay).unwrap().visible);
            assert!(d.selection.is_none());
            let l = d.layer(id).unwrap();
            assert_eq!(l.surface().unwrap().read_region(d.bounds()), original);
            assert!((l.mask.as_ref().unwrap().value(0, 0) - 127.0 / 255.0).abs() < 0.005);
            s.execute(REGENERATE_COMMAND, json!({})).unwrap();
            assert!((s.active().unwrap().doc.layer(id).unwrap().mask.as_ref().unwrap().value(0, 0) - 64.0 / 255.0).abs() < 0.005);
            s.execute("edit.undo", json!({})).unwrap();
            assert!((s.active().unwrap().doc.layer(id).unwrap().mask.as_ref().unwrap().value(0, 0) - 127.0 / 255.0).abs() < 0.005);
            s.execute("edit.undo", json!({})).unwrap();
            assert_eq!(s.active().unwrap().doc.layer(id).unwrap().mask.as_ref().unwrap().value(0, 0), 0.0);
            assert!(s.active().unwrap().doc.selection.is_some());
            s.execute("edit.redo", json!({})).unwrap();
            assert_eq!(s.active().unwrap().doc.layer(id).unwrap().surface().unwrap().read_region(s.active().unwrap().doc.bounds()), original);
        }
    }

    #[test]
    fn locked_layer_leaves_source_intact() {
        let mut s = super::super::tests::session(8);
        let id = s.active().unwrap().active_layer.unwrap();
        s.edit("Lock", |d, _| {
            d.layer_mut(id).unwrap().locks.pixels = true;
            Ok(())
        })
        .unwrap();
        s.ai_backend = Some(Arc::new(Coverage(AtomicUsize::new(0))));
        assert!(s.execute(REMOVE_BACKGROUND_COMMAND, json!({})).is_err());
        assert!(s.active().unwrap().doc.layer(id).unwrap().mask.is_none());
        s.edit("Locked parent", |d, _| {
            let mut child = d.layers.remove(0);
            child.locks.pixels = false;
            let mut group = Layer::group("Locked group", vec![child]);
            group.locks.pixels = true;
            d.layers = vec![group];
            Ok(())
        })
        .unwrap();
        assert!(s.execute(REMOVE_BACKGROUND_COMMAND, json!({})).is_err());
    }

    #[test]
    fn native_formats_and_original_alpha_survive_generated_rgb() {
        for mode in [ColorMode::Rgb, ColorMode::Grayscale, ColorMode::Cmyk, ColorMode::Lab] {
            for depth in [photocraft_color::SampleType::U8, photocraft_color::SampleType::U16, photocraft_color::SampleType::F32] {
                let mut s = super::super::tests::session(8);
                s.edit("Native raster", |d, active| {
                    d.mode = mode;
                    d.depth = depth;
                    let fmt = PixelFormat::new(mode, depth, true);
                    let mut layer = Layer::raster("Native", fmt);
                    let mut pixel = vec![0.23; fmt.channels()];
                    *pixel.last_mut().unwrap() = 0.4;
                    layer.surface_mut().unwrap().write_region(d.bounds(), &pixel.repeat(17 * 13));
                    *active = Some(layer.id);
                    d.layers = vec![layer];
                    Ok(())
                })
                .unwrap();
                let before = s.active().unwrap().doc.layers[0].surface().unwrap().read_region(Rect::new(0, 0, 17, 13));
                s.execute(REMOVE_BACKGROUND_COMMAND, json!({})).unwrap();
                let d = &s.active().unwrap().doc;
                let layer = &d.layers[0];
                assert_eq!(layer.surface().unwrap().format(), PixelFormat::new(mode, depth, true));
                assert_eq!(layer.surface().unwrap().read_region(d.bounds()), before);
                assert_eq!(layer.mask.as_ref().unwrap().surface.format().sample, depth);
                assert_eq!(d.layers.len(), 1);
            }
        }
    }

    #[test]
    fn invalid_dimensions_do_not_modify_layer() {
        struct Short;
        impl AiBackend for Short {
            fn generate(&self, _: &AiRequest, _: &JobCtx) -> Result<Rgba8Image> {
                Ok(Rgba8Image::new(1, 1))
            }
        }
        let mut s = super::super::tests::session(8);
        s.ai_backend = Some(Arc::new(Short));
        let revision = s.active().unwrap().revision;
        assert!(s.execute(REMOVE_BACKGROUND_COMMAND, json!({})).is_err());
        assert_eq!(s.active().unwrap().revision, revision);
        assert!(s.active().unwrap().doc.layers[0].mask.is_none());
    }
}

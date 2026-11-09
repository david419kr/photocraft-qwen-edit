//! Local selected-content removal. Preserve native source pixels and alpha outside the edit.
use super::cutout_external::{self, Backend, Kind};
use super::{Builtin, Document, EngineError, Intent, JobCtx, Layer, PixelFormat, Rect, Result, Session, Transform, Value, byte, json};
use photocraft_codecs::{ChannelLayout, DecodeOptions, EncodeOptions, Format, Image, Limits, SampleType};

fn png(image: &Image) -> Result<Vec<u8>> {
    photocraft_codecs::encode(image, Format::Png, &EncodeOptions::default()).map_err(|e| EngineError::Other(e.to_string()))
}

pub(super) fn run(s: &mut Session) -> Result<Value> {
    let backend = cutout_external::discover(Kind::Erase).ok_or_else(|| EngineError::Other("Local selection-removal backend is unavailable".into()))?;
    run_with(s, backend)
}

fn run_with(s: &mut Session, backend: Backend) -> Result<Value> {
    crate::jobs::edit_job(
        s,
        "Remove Selected Content",
        move |doc, active, ctx| {
            super::supported_document(doc).map_err(EngineError::Other)?;
            let selection = doc.selection.as_ref().ok_or_else(|| EngineError::Other("Select an area first".into()))?;
            let (w, h) = (doc.size.width, doc.size.height);
            let mut mask = Vec::with_capacity(w as usize * h as usize);
            for y in 0..h {
                ctx.check()?;
                let values = selection.read_region(Rect::new(0, y as i32, w as i32, y as i32 + 1));
                for value in values.chunks_exact(selection.channels()).filter_map(|v| v.first()) {
                    if !value.is_finite() {
                        return Err(EngineError::Other("Invalid selection coverage".into()));
                    }
                    mask.push(byte(*value));
                }
            }
            if !mask.iter().any(|v| *v > 0) {
                return Err(EngineError::Other("The selected area is empty".into()));
            }
            let composite = photocraft_compose::render_reduced(doc, w, h);
            let to_srgb = Transform::new(&crate::color_cmds::composite_profile(doc), Builtin::Srgb.profile(), Intent::RelativeColorimetric, true)
                .map_err(|e| EngineError::Other(e.to_string()))?;
            let mut input = Vec::with_capacity(w as usize * h as usize * 4);
            for row in composite.px.chunks_exact(w as usize) {
                ctx.check()?;
                let mut converted = vec![0.0; row.len() * 4];
                to_srgb.convert_f32(row.as_flattened(), 4, &mut converted, 4, true);
                input.extend(converted.into_iter().map(byte));
            }
            let source = png(&Image::from_u8(w, h, ChannelLayout::Rgba, input).map_err(|e| EngineError::Other(e.to_string()))?)?;
            let selected = png(&Image::from_u8(w, h, ChannelLayout::Gray, mask.clone()).map_err(|e| EngineError::Other(e.to_string()))?)?;
            let max_output = u64::from(w) * u64::from(h) * 16 + 1_048_576;
            let output = cutout_external::spawn(&backend, &source, Some(&selected), max_output, ctx)?;
            let layer = result(doc, &composite.px, &mask, &output, ctx)?;
            let id = layer.id;
            // A full composite replaces the visible source: stacking it would double partial alpha.
            for source in &mut doc.layers {
                source.visible = false;
            }
            doc.layers.push(layer);
            *active = Some(id);
            Ok(id)
        },
        |id| json!({"layer":id.0,"backend":"externalErase"}),
    )
}

fn result(doc: &Document, source: &[[f32; 4]], mask: &[u8], bytes: &[u8], ctx: &JobCtx) -> Result<Layer> {
    let (w, h) = (doc.size.width, doc.size.height);
    let pixels = u64::from(w) * u64::from(h);
    let limits = Limits { max_width: w, max_height: h, max_pixels: pixels, max_alloc: pixels * 16 };
    let image = photocraft_codecs::decode_as_with(Format::Png, bytes, &DecodeOptions { limits, ..Default::default() })
        .map_err(|e| EngineError::Other(format!("Invalid local removal image: {e}")))?;
    if image.width() != w || image.height() != h || image.layout() != ChannelLayout::Rgba || image.sample_type() != SampleType::U8 {
        return Err(EngineError::Other("Local removal image has incorrect size or format".into()));
    }
    if source.len() != pixels as usize || mask.len() != pixels as usize {
        return Err(EngineError::Other("Invalid removal input geometry".into()));
    }
    let profile = crate::color_cmds::document_profile(doc);
    let generated = Transform::new(Builtin::Srgb.profile(), &profile, Intent::RelativeColorimetric, true).map_err(|e| EngineError::Other(e.to_string()))?;
    let fmt = PixelFormat::new(doc.mode, doc.depth, true);
    // Keep native CMYK inks and out-of-gamut Lab values: an RGB round trip would
    // change unselected pixels when the original layers are hidden.
    ctx.check()?;
    let mut surface = photocraft_compose::composite_to_surface(doc, doc.bounds(), fmt, None);
    for (y, ((src, coverage), out)) in
        source.chunks_exact(w as usize).zip(mask.chunks_exact(w as usize)).zip(image.data().chunks_exact(w as usize * 4)).enumerate()
    {
        ctx.check()?;
        let row = Rect::new(0, y as i32, w as i32, y as i32 + 1);
        let mut native = surface.read_region(row);
        let rgba: Vec<f32> = out.iter().map(|v| f32::from(*v) / 255.0).collect();
        let mut edited = vec![0.0; native.len()];
        generated.convert_f32(&rgba, 4, &mut edited, fmt.channels(), true);
        for (((pixel, changed), selected), old) in native.chunks_exact_mut(fmt.channels()).zip(edited.chunks_exact(fmt.channels())).zip(coverage).zip(src) {
            if *selected > 0 {
                pixel.copy_from_slice(changed);
            }
            if let Some(alpha) = pixel.last_mut() {
                *alpha = old[3];
            }
        }
        surface.write_region(row, &native);
    }
    let mut layer = Layer::raster("Remove", fmt);
    *crate::pixels_mut(&mut layer)? = surface;
    Ok(layer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_erase_process() {
        let Ok(output) = std::env::var("PHOTOCRAFT_CUTOUT_OUTPUT") else { return };
        assert!(std::path::Path::new(&std::env::var("PHOTOCRAFT_CUTOUT_SELECTION").unwrap()).is_file());
        let image = Image::from_u8(2, 2, ChannelLayout::Rgba, [255, 0, 0, 0].repeat(4)).unwrap();
        std::fs::write(output, png(&image).unwrap()).unwrap();
    }

    #[test]
    fn local_removal_is_independent_of_comfy_and_undoable_at_every_depth() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Backend {
            root: dir.path().into(),
            executable: std::env::current_exe().unwrap(),
            args: vec!["--exact".into(), "ai_cmds::erase_external::tests::mock_erase_process".into()],
            label: "W".into(),
        };
        for depth in [8, 16, 32] {
            for mode in ["rgb", "gray", "cmyk", "lab"] {
                let mut s = Session::new();
                s.execute("file.new", json!({"width":2,"height":2,"depth":depth,"mode":mode,"background":"white"})).unwrap();
                s.execute("select.rect", json!({"x":0,"y":0,"width":1,"height":1})).unwrap();
                assert!(s.ai_backend.is_none());
                let before = s.active().unwrap().doc.clone();
                run_with(&mut s, backend.clone()).unwrap();
                let state = s.active().unwrap();
                assert_eq!(state.doc.layers.len(), 2);
                assert!(!state.doc.layers[0].visible && state.doc.layers[1].visible);
                assert_eq!(
                    state.doc.layers[0].surface().unwrap().read_region(before.bounds()),
                    before.layers[0].surface().unwrap().read_region(before.bounds())
                );
                let image = photocraft_compose::render_reduced(&state.doc, 2, 2);
                let was = photocraft_compose::render_reduced(&before, 2, 2);
                for (value, original) in image.px[1].iter().zip(was.px[1]) {
                    assert!((value - original).abs() < 0.02, "{mode}/{depth}: outside selection");
                }
                assert!(image.px[0][3] > 0.99, "backend cannot erase alpha");
                s.undo();
                assert_eq!(s.active().unwrap().doc.layers.len(), 1);
                assert!(s.active().unwrap().doc.layers[0].visible);
                s.redo();
                assert_eq!(s.active().unwrap().doc.layers.len(), 2);
            }
        }
        let ctx = JobCtx::new();
        ctx.cancel();
        assert!(matches!(cutout_external::spawn(&backend, &[], Some(&[]), 1024, &ctx), Err(EngineError::Cancelled)));
    }

    #[test]
    fn removal_preserves_unselected_native_cmyk_and_lab_colors() {
        for depth in [8, 16, 32] {
            for mode in ["cmyk", "lab"] {
                for alpha in [1.0, 0.5] {
                    let mut s = Session::new();
                    s.execute("file.new", json!({"width":2,"height":2,"depth":depth,"mode":mode})).unwrap();
                    let mut doc = (*s.active().unwrap().doc).clone();
                    let fmt = PixelFormat::new(doc.mode, doc.depth, true);
                    let mut layer = Layer::raster("Colored source", fmt);
                    // Nontrivial black separation and a Lab color outside sRGB cannot
                    // survive conversion through the display composite unchanged.
                    let pixel = if mode == "cmyk" { vec![0.2, 0.6, 0.1, 0.4, alpha] } else { vec![0.6, 1.0, 0.0, alpha] };
                    crate::pixels_mut(&mut layer).unwrap().write_region(doc.bounds(), &pixel.repeat(4));
                    let expected = layer.surface().unwrap().pixel(1, 0);
                    doc.layers = vec![layer];
                    let source = photocraft_compose::render_reduced(&doc, 2, 2);
                    let output = png(&Image::from_u8(2, 2, ChannelLayout::Rgba, [255, 0, 0, 0].repeat(4)).unwrap()).unwrap();
                    let result = result(&doc, &source.px, &[255, 0, 0, 0], &output, &JobCtx::new()).unwrap();
                    let surface = result.surface().unwrap();
                    for (x, y) in [(1, 0), (0, 1), (1, 1)] {
                        for (actual, original) in surface.pixel(x, y).iter().zip(&expected) {
                            assert!((actual - original).abs() < 1.0e-5, "{mode}/{depth}/{alpha}: unselected native channel changed");
                        }
                    }
                    assert!((surface.pixel(0, 0).last().unwrap() - expected.last().unwrap()).abs() < 1.0e-5, "source alpha must survive the edit");
                }
            }
        }
    }

    #[test]
    fn removal_rejects_bad_output_and_preserves_partial_alpha() {
        let mut s = Session::new();
        s.execute("file.new", json!({"width":2,"height":2,"depth":32,"background":"white"})).unwrap();
        let doc = &s.active().unwrap().doc;
        let source = vec![[0.1, 0.2, 0.3, 0.5]; 4];
        let output = png(&Image::from_u8(2, 2, ChannelLayout::Rgba, [255, 0, 0, 255].repeat(4)).unwrap()).unwrap();
        let layer = result(doc, &source, &[255, 128, 0, 0], &output, &JobCtx::new()).unwrap();
        assert_eq!(layer.surface().unwrap().pixel(0, 0).last().copied(), Some(0.5));
        assert_eq!(layer.surface().unwrap().pixel(1, 0).last().copied(), Some(0.5));
        assert!(result(doc, &source, &[0; 4], b"invalid", &JobCtx::new()).is_err());
        let wrong_size = png(&Image::from_u8(1, 1, ChannelLayout::Rgba, vec![0; 4]).unwrap()).unwrap();
        assert!(result(doc, &source, &[0; 4], &wrong_size, &JobCtx::new()).is_err());
        assert!(s.execute(super::super::REMOVE_SELECTION_COMMAND, json!({"bad":1})).is_err());
        assert!(s.execute(super::super::REMOVE_SELECTION_COMMAND, json!({})).is_err());
        assert_eq!(s.active().unwrap().doc.layers.len(), 1);
    }
}

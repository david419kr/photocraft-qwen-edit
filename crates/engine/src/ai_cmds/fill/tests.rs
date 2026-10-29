use super::*;
use crate::ai_cmds::{AiBackend, AiRequest, Arc, EditMode, REGENERATE_COMMAND, Session, json, result_layer, selection_signature};

fn document(width: u32, height: u32, selected: Rect) -> Document {
    let mut session = Session::new();
    session.execute("file.new", json!({"width":width,"height":height,"background":"white"})).unwrap();
    session.execute("select.rect", json!({"x":selected.x0,"y":selected.y0,"width":selected.width(),"height":selected.height()})).unwrap();
    session.active().unwrap().doc.as_ref().clone()
}

#[test]
fn context_preserves_origin_and_shifts_at_edges_without_clipping_selection() {
    let canvas = Rect::new(0, 0, 6000, 4000);
    let selected = Rect::new(1400, 900, 1528, 1028);
    assert_eq!(crop(selected, canvas), Rect::new(1208, 708, 1720, 1220));
    for selected in [Rect::new(0, 0, 19, 31), Rect::new(5979, 3967, 6000, 4000), Rect::new(7, 123, 5999, 124)] {
        let context = crop(selected, canvas);
        assert!(canvas.contains_rect(&context));
        assert!(context.contains_rect(&selected));
        assert!(context.width() >= MIN_CROP && context.height() >= MIN_CROP);
    }
}

#[test]
fn fixed_size_off_canvas_selection_keeps_its_outline_but_limits_fill_to_canvas() {
    let mut session = Session::new();
    session.execute("file.new", json!({"width":100,"height":100,"background":"white"})).unwrap();
    for selected in [Rect::new(50, 25, 150, 75), Rect::new(-50, -25, 50, 25), Rect::new(-25, -25, 125, 125), Rect::new(150, 25, 250, 75)] {
        session
            .execute("select.rect", json!({"x":selected.x0,"y":selected.y0,"width":selected.width(),"height":selected.height(),"clipToCanvas":false}))
            .unwrap();
        let doc = &session.active().unwrap().doc;
        let visible = selected.intersect(&doc.bounds());
        let input = prepare(doc, &JobCtx::new());
        if visible.is_empty() {
            assert!(input.is_err());
        } else {
            assert_eq!(selection_info(doc, &JobCtx::new()).unwrap().0, visible);
            let input = input.unwrap();
            assert!(doc.bounds().contains_rect(&input.region.bounds));
            let mask = input.region.mask.as_ref().unwrap();
            assert_eq!(mask.content_bounds(), visible);
            assert_eq!(mask.sample_channel(selected.x0, selected.y0, 0), if doc.bounds().contains(selected.x0, selected.y0) { 1.0 } else { 0.0 });
        }
        // Preparing a bounded AI request must not clip the movable selection in the document.
        assert_eq!(doc.selection.as_ref().unwrap().content_bounds(), selected);
    }
}

#[test]
fn elongated_selections_keep_rectangular_context_and_cross_axis_margin() {
    let canvas = Rect::new(0, 0, 4000, 4000);
    for selected in [Rect::new(500, 1900, 2500, 2000), Rect::new(1900, 500, 2000, 2500)] {
        let context = crop(selected, canvas);
        assert!(canvas.contains_rect(&context));
        assert!(context.contains_rect(&selected));
        assert_ne!(context.width(), context.height());
        assert!(context.width() > selected.width());
        assert!(context.height() > selected.height());
        assert!(context.width().max(context.height()) <= 4 * context.width().min(context.height()));
    }
}

#[test]
fn square_crop_includes_exact_four_by_three_but_not_one_pixel_beyond() {
    let canvas = Rect::new(0, 0, 4000, 4000);
    for (w, h, square) in [
        (399, 300, true),
        (400, 300, true),
        (401, 300, false),
        (300, 399, true),
        (300, 400, true),
        (300, 401, false),
        (4, 3, true),
        (5, 3, false),
        (3, 4, true),
        (3, 5, false),
    ] {
        let selected = Rect::from_xywh(1000, 1000, w, h);
        let context = crop(selected, canvas);
        assert_eq!(context.width() == context.height(), square, "selection {w}x{h}: {context:?}");
        assert!(canvas.contains_rect(&context));
        assert!(context.contains_rect(&selected));
        assert!(context.width() > w && context.height() > h);
    }
}

#[test]
fn implicit_selected_background_uses_canvas_bounds_and_invalid_coverage_is_rejected() {
    let mut doc = document(2200, 1800, Rect::new(0, 0, 1, 1));
    let mut mask = Surface::with_default(gray_float(), &[1.0]);
    mask.write_pixel(100, 100, &[0.0]);
    doc.selection = Some(mask);
    let (selected, regions) = selection_info(&doc, &JobCtx::new()).unwrap();
    assert_eq!(selected, doc.bounds());
    assert_eq!(regions.len(), 9);
    let mut mask = Surface::new(gray_float());
    mask.write_pixel(10, 10, &[f32::NAN]);
    doc.selection = Some(mask);
    assert!(selection_info(&doc, &JobCtx::new()).is_err());
}

#[test]
fn guide_has_no_extra_dilation_and_composite_keeps_fractional_coverage() {
    let mut doc = document(64, 64, Rect::new(31, 32, 33, 33));
    let mut selected = Surface::new(gray_float());
    selected.write_pixel(31, 32, &[0.25]);
    selected.write_pixel(32, 32, &[0.75]);
    selected.write_pixel(30, 31, &[1.0e-5]);
    doc.selection = Some(selected);
    let input = prepare(&doc, &JobCtx::new()).unwrap();
    assert_eq!(input.region.bounds, doc.bounds());
    assert_eq!(input.region.content_size, [64, 64]);
    let mask = input.region.mask.as_ref().unwrap();
    assert_eq!(mask.sample_channel(31, 32, 0), 0.25);
    assert_eq!(mask.sample_channel(32, 32, 0), 0.75);
    assert_eq!(mask.sample_channel(30, 31, 0), 1.0e-5);
    for y in 0..64 {
        for x in 0..64 {
            let expected = if [(31, 32), (32, 32), (30, 31)].contains(&(x, y)) { 0 } else { 255 };
            assert_eq!(input.image.pixels[((y * 64 + x) * 4 + 3) as usize], expected, "guide {x},{y}");
            assert_eq!(mask.sample_channel(x, y, 0), doc.selection.as_ref().unwrap().sample_channel(x, y, 0));
        }
    }
}

#[test]
fn local_fill_allocates_model_resolution_to_selection_and_records_document_regions() {
    let doc = document(2200, 1800, Rect::new(1400, 900, 1528, 1028));
    let input = prepare(&doc, &JobCtx::new()).unwrap();
    assert_eq!(input.region.bounds, Rect::new(1208, 708, 1720, 1220));
    assert_eq!(input.region.content_size, [512, 512]);
    assert_eq!(input.region.prompt_regions, vec![crate::ai_prompts::Region::Center, crate::ai_prompts::Region::Right]);
    let mask = input.region.mask.as_ref().unwrap();
    assert_eq!(mask.sample_channel(1400, 900, 0), 1.0);
    assert_eq!(mask.sample_channel(1398, 940, 0), 0.0);
    assert_eq!(mask.sample_channel(1380, 940, 0), 0.0);
    assert_eq!(mask.sample_channel(0, 0, 0), 0.0);
    let center = (192 * input.image.width as usize + 192) * 4;
    assert_eq!(&input.image.pixels[center..center + 4], &[255, 255, 255, 0]);
    assert_eq!(input.image.pixels[3], 255);
    assert_eq!(input.image.pixels.iter().skip(3).step_by(4).filter(|a| **a == 0).count(), 128 * 128);
}

#[test]
fn translucent_lineart_outside_selection_survives_an_adversarial_model_at_every_depth() {
    struct Dark;
    impl AiBackend for Dark {
        fn generate(&self, request: &AiRequest, _: &JobCtx) -> Result<Rgba8Image> {
            let mut output = request.image.clone();
            for pixel in output.pixels.as_chunks_mut::<4>().0.iter_mut() {
                pixel.copy_from_slice(&[16, 24, 32, 200]);
            }
            Ok(output)
        }
    }
    for depth in [8, 16, 32] {
        let mut s = Session::new();
        s.execute("file.new", json!({"width":65,"height":65,"depth":depth,"background":"transparent"})).unwrap();
        s.edit("Translucent line-art fixture", |doc, _| {
            let mut mask = Surface::new(gray_float());
            for y in 18..34 {
                for x in 23..28 {
                    mask.write_pixel(x, y, &[1.0]);
                }
            }
            for y in 38..49 {
                for x in 35..40 {
                    mask.write_pixel(x, y, &[1.0]);
                }
            }
            mask.write_pixel(24, 24, &[0.0]);
            mask.write_pixel(27, 26, &[0.5]);
            let mut surface = Surface::new(PixelFormat::new(ColorMode::Rgb, doc.depth, true));
            let mut values = Vec::with_capacity(65 * 65 * 4);
            for y in 0..65 {
                for x in 0..65 {
                    let curve = 20.0 + 8.0 * (y as f32 / 13.0).sin();
                    let mut color = if (x as f32 - curve).abs() < 1.2 { [0.08, 0.04, 0.06] } else { [0.55, 0.25, 0.35] };
                    if (17..51).contains(&x) && (10..56).contains(&y) {
                        for value in &mut color {
                            *value = *value * 0.6 + 0.4;
                        }
                    }
                    if mask.sample_channel(x, y, 0) > 0.0 {
                        color = [0.85, 0.1, 0.1];
                    }
                    if x == 18 && y == 25 {
                        color = [1.25, 0.8, 0.4];
                    }
                    values.extend(color);
                    values.push(1.0);
                }
            }
            surface.write_region(doc.bounds(), &values);
            *crate::pixels_mut(doc.layers.first_mut().unwrap()).unwrap() = surface;
            doc.selection = Some(mask);
            Ok(())
        })
        .unwrap();
        let before = s.active().unwrap().doc.clone();
        let input = prepare(&before, &JobCtx::new()).unwrap();
        assert_eq!(input.region.content_size, [65, 65]);
        let selected_offset = (22 * input.image.width as usize + 23) * 4;
        assert_eq!(input.image.pixels[selected_offset + 3], 0);
        assert!(input.image.pixels[selected_offset] > 200);
        assert!(input.image.pixels[selected_offset + 1] < 40);
        let selection = before.selection.as_ref().unwrap();
        let original = photocraft_compose::render(&before, before.bounds());
        s.ai_backend = Some(Arc::new(Dark));
        s.execute("edit.generativeFill", json!({"prompt":"","seed":1})).unwrap();
        let doc = &s.active().unwrap().doc;
        let layer = doc.layers.last().unwrap();
        let rendered = photocraft_compose::render(doc, doc.bounds());
        for y in 0..65 {
            for x in 0..65 {
                let weight = selection.sample_channel(x, y, 0);
                assert_eq!(layer.mask.as_ref().unwrap().surface.sample_channel(x, y, 0), weight);
                if weight == 0.0 {
                    let index = (y * 65 + x) as usize;
                    assert_eq!(rendered.px[index], original.px[index], "{depth}: known pixel {x},{y}");
                }
            }
        }
        for (channel, value) in [16.0, 24.0, 32.0].into_iter().enumerate() {
            assert!((layer.surface().unwrap().sample_channel(23, 22, channel) - value / 255.0).abs() < 1.0 / 255.0);
        }
        assert_eq!(layer.surface().unwrap().sample_channel(23, 22, 3), 1.0);
        assert_eq!(doc.selection.as_ref().unwrap().read_region(doc.bounds()), selection.read_region(before.bounds()));
    }
}

#[test]
fn tiny_and_disconnected_selections_survive_noninteger_reduction() {
    let mut doc = document(3073, 2049, Rect::new(1, 1, 2, 2));
    let mut mask = Surface::new(gray_float());
    mask.write_pixel(1, 1, &[1.0]);
    mask.write_pixel(3001, 2001, &[1.0 / 255.0]);
    doc.selection = Some(mask);
    let input = prepare(&doc, &JobCtx::new()).unwrap();
    assert_eq!(input.region.bounds, doc.bounds());
    let [w, h] = input.region.content_size;
    assert!(w > 2048 && h > 1024);
    assert!(resolution::Budget::EDIT.accepts_frame([input.image.width, input.image.height]));
    for (x, y) in [(1, 1), (3001, 2001)] {
        let mx = (x as f64 * f64::from(w) / 3073.0).floor() as usize;
        let my = (y as f64 * f64::from(h) / 2049.0).floor() as usize;
        assert_eq!(input.image.pixels[(my * input.image.width as usize + mx) * 4 + 3], 0);
        assert_eq!(input.region.mask.as_ref().unwrap().sample_channel(x, y, 0), doc.selection.as_ref().unwrap().sample_channel(x, y, 0));
    }
    assert_eq!(input.region.prompt_regions, vec![crate::ai_prompts::Region::TopLeft]);
}

#[test]
fn transparent_context_is_gated_and_padding_never_becomes_an_edit() {
    let mut doc = document(31, 1200, Rect::new(5, 499, 15, 533));
    doc.layers.clear();
    let input = prepare(&doc, &JobCtx::new()).unwrap();
    let mask = input.region.mask.as_ref().unwrap();
    assert_eq!(mask.sample_channel(5, 499, 0), 1.0);
    assert_eq!(mask.sample_channel(4, 499, 0), 0.0);
    let [w, h] = input.region.content_size;
    assert!(w < input.image.width);
    for y in 0..input.image.height {
        for x in 0..input.image.width {
            if x >= w || y >= h {
                assert_eq!(input.image.pixels[((y * input.image.width + x) * 4 + 3) as usize], 255);
            }
        }
    }
    let ctx = JobCtx::new();
    ctx.cancel();
    assert!(matches!(prepare(&doc, &ctx), Err(EngineError::Cancelled)));
}

#[test]
fn inverse_maps_nonzero_origin_and_noninteger_scale_without_stretching_padding() {
    for depth in [8, 16, 32] {
        let mut doc = document(2400, 1600, Rect::new(700, 700, 800, 800));
        doc.depth = match depth {
            8 => SampleType::U8,
            16 => SampleType::U16,
            _ => SampleType::F32,
        };
        let bounds = Rect::new(123, 77, 2131, 1414);
        let (w, h) = (1536u32, 1023u32);
        let (pw, ph) = (w, h.div_ceil(32) * 32);
        let mut image = Rgba8Image::new(pw, ph);
        for y in 0..ph {
            for x in 0..pw {
                let offset = ((y * pw + x) * 4) as usize;
                image.pixels[offset..offset + 4].copy_from_slice(&[byte(x as f32 / w as f32), byte(y as f32 / h as f32), 0, 255]);
            }
        }
        let mut mask = Surface::new(gray_float());
        let paint = Rect::new(700, 700, 800, 800);
        mask.write_region(paint, &vec![1.0; 10000]);
        let region = EditRegion { bounds, content_size: [w, h], mask: Some(mask), prompt_regions: vec![], filter: Resample::Bicubic };
        let layer = result_layer(&doc, &image, &region, "test", EditMode::GenerativeFill, &JobCtx::new()).unwrap();
        let surface = layer.surface().unwrap();
        for (x, y) in [(700, 700), (799, 799), (751, 733)] {
            let r = (x as f64 - bounds.x0 as f64 + 0.5) / bounds.width() as f64 - 0.5 / w as f64;
            let g = (y as f64 - bounds.y0 as f64 + 0.5) / bounds.height() as f64 - 0.5 / h as f64;
            assert!((surface.sample_channel(x, y, 0) as f64 - r).abs() < 2.0 / 255.0);
            assert!((surface.sample_channel(x, y, 1) as f64 - g).abs() < 2.0 / 255.0);
        }
        assert_eq!(surface.sample_channel(0, 0, 3), 0.0);
        assert_eq!(layer.mask.as_ref().unwrap().surface.sample_channel(699, 700, 0), 0.0);
    }
}

#[test]
fn inverse_keeps_thin_stroke_centers_at_multiple_fractional_phases() {
    let doc = document(2400, 1600, Rect::new(700, 700, 800, 800));
    let bounds = Rect::new(123, 77, 2131, 1414);
    let (w, h) = (1536u32, 1023u32);
    let xs = [472u32, 478, 484, 490, 496];
    let ys = [482u32, 494, 506, 518, 530];
    let mut image = Rgba8Image::new(w, 1024);
    for y in 0..1024 {
        for x in 0..w {
            let offset = ((y * w + x) * 4) as usize;
            image.pixels[offset..offset + 4].copy_from_slice(&[0, if xs.contains(&x) { 255 } else { 0 }, if ys.contains(&y) { 255 } else { 0 }, 255]);
        }
    }
    let mut mask = Surface::new(gray_float());
    mask.write_region(Rect::new(700, 700, 800, 800), &vec![1.0; 10000]);
    let region = EditRegion { bounds, content_size: [w, h], mask: Some(mask), prompt_regions: vec![], filter: Resample::Bicubic };
    let layer = result_layer(&doc, &image, &region, "strokes", EditMode::GenerativeFill, &JobCtx::new()).unwrap();
    let surface = layer.surface().unwrap();
    for x in xs {
        let expected = f64::from(bounds.x0) + (f64::from(x) + 0.5) * f64::from(bounds.width()) / f64::from(w) - 0.5;
        let first = expected.floor() as i32 - 2;
        let peak = (first..first + 6).max_by(|a, b| surface.sample_channel(*a, 750, 1).total_cmp(&surface.sample_channel(*b, 750, 1))).unwrap();
        assert!((f64::from(peak) - expected).abs() <= 0.52, "stroke {x}, expected {expected}, peak {peak}");
    }
    for y in ys {
        let expected = f64::from(bounds.y0) + (f64::from(y) + 0.5) * f64::from(bounds.height()) / f64::from(h) - 0.5;
        let first = expected.floor() as i32 - 2;
        let peak = (first..first + 6).max_by(|a, b| surface.sample_channel(750, *a, 2).total_cmp(&surface.sample_channel(750, *b, 2))).unwrap();
        assert!((f64::from(peak) - expected).abs() <= 0.52, "stroke {y}, expected {expected}, peak {peak}");
    }
}

#[test]
fn input_reduction_uses_the_same_crop_edge_centers_as_the_inverse() {
    let mut doc = document(3073, 2049, Rect::new(1, 1, 2, 2));
    let mut selected = Surface::new(gray_float());
    selected.write_pixel(1, 1, &[1.0]);
    selected.write_pixel(3001, 2001, &[1.0]);
    doc.selection = Some(selected);
    let mut pixels = Surface::new(PixelFormat::RGBA8);
    let black = [0.0, 0.0, 0.0, 1.0].repeat(3073);
    for y in 0..2049 {
        pixels.write_region(Rect::new(0, y, 3073, y + 1), &black);
        pixels.write_pixel(1540, y, &[0.0, 1.0, 0.0, 1.0]);
    }
    *crate::pixels_mut(doc.layers.last_mut().unwrap()).unwrap() = pixels;
    let input = prepare(&doc, &JobCtx::new()).unwrap();
    let [w, h] = input.region.content_size;
    let expected = (1540.0 + 0.5) * f64::from(w) / 3073.0 - 0.5;
    let peak = (0..w).max_by_key(|x| input.image.pixels[(((h / 2) * input.image.width + *x) * 4 + 1) as usize]).unwrap();
    assert!((f64::from(peak) - expected).abs() <= 0.52, "expected {expected}, peak {peak}");
    let mut mass = 0u64;
    let mut moment = 0u64;
    for x in 0..w {
        let green = u64::from(input.image.pixels[(((h / 2) * input.image.width + x) * 4 + 1) as usize]);
        mass += green;
        moment += u64::from(x) * green;
    }
    assert!(mass > 0);
    let centroid = moment as f64 / mass as f64;
    assert!((centroid - expected).abs() < 0.03, "expected {expected}, centroid {centroid}");
}

#[test]
fn thin_selection_has_visible_unselected_context() {
    let doc = document(2200, 1800, Rect::new(500, 900, 1500, 901));
    let input = prepare(&doc, &JobCtx::new()).unwrap();
    let bounds = input.region.bounds;
    let mask = input.region.mask.as_ref().unwrap();
    assert_eq!(mask.sample_channel(bounds.x0, 900, 0), 0.0);
    assert_eq!(mask.sample_channel(bounds.x1 - 1, 900, 0), 0.0);
    assert_eq!(mask.sample_channel(500, 900, 0), 1.0);
}

#[test]
fn command_and_reroll_keep_the_same_crop_filter_selection_and_history_location() {
    struct Identity;
    impl AiBackend for Identity {
        fn generate(&self, request: &AiRequest, _: &JobCtx) -> Result<Rgba8Image> {
            let mut output = request.image.clone();
            for alpha in output.pixels.iter_mut().skip(3).step_by(4) {
                *alpha = 255;
            }
            Ok(output)
        }
    }
    let mut s = Session::new();
    s.execute("file.new", json!({"width":2200,"height":1800,"background":"white"})).unwrap();
    s.execute("select.rect", json!({"x":1400,"y":900,"width":128,"height":128})).unwrap();
    s.execute("prefs.set", json!({"path":"general.imageInterpolation","value":"nearestNeighbor"})).unwrap();
    s.ai_backend = Some(Arc::new(Identity));
    let selected = selection_signature(&s.active().unwrap().doc);
    let first = s.execute("edit.generativeFill", json!({"prompt":"blue","seed":7})).unwrap();
    assert_eq!(first["workingRect"], json!([1208, 708, 1720, 1220]));
    assert_eq!(first["contentSize"], json!([512, 512]));
    let doc = &s.active().unwrap().doc;
    let id = doc.id;
    assert_eq!(selection_signature(doc), selected);
    let stored = s.ai_runs.get(&id).unwrap();
    assert_eq!(stored.region.filter, Resample::Nearest);
    let mask = stored.region.mask.as_ref().unwrap().read_region(doc.bounds());
    let history = crate::ai_prompts::read(doc.metadata.xmp.as_deref()).unwrap();
    assert_eq!(history[0].regions, vec![crate::ai_prompts::Region::Center, crate::ai_prompts::Region::Right]);
    s.execute("prefs.set", json!({"path":"general.imageInterpolation","value":"bilinear"})).unwrap();
    let second = s.execute(REGENERATE_COMMAND, json!({})).unwrap();
    assert_eq!(second["workingRect"], first["workingRect"]);
    assert_eq!(second["layer"], first["layer"]);
    assert_eq!(s.ai_runs.get(&id).unwrap().region.filter, Resample::Nearest);
    assert_eq!(s.active().unwrap().doc.layers.len(), 2);
    assert_eq!(s.active().unwrap().doc.layers.last().unwrap().mask.as_ref().unwrap().surface.read_region(s.active().unwrap().doc.bounds()), mask);
    s.execute("edit.undo", json!({})).unwrap();
    s.execute("edit.undo", json!({})).unwrap();
    assert_eq!(s.active().unwrap().doc.layers.len(), 1);
    assert!(s.active().unwrap().doc.layers[0].visible);
    assert_eq!(selection_signature(&s.active().unwrap().doc), selected);
    s.execute("edit.redo", json!({})).unwrap();
    assert_eq!(s.active().unwrap().doc.layers.len(), 2);
}

#[test]
#[ignore = "manual release CPU benchmark; no model or service requests"]
fn benchmark_large_document_preparation() {
    for (width, height) in [(6000, 4000), (7200, 5000)] {
        let doc = document(width, height, Rect::new(2500, 1500, 2700, 1700));
        let start = std::time::Instant::now();
        let legacy = super::super::prepare(&doc, EditMode::PromptEdit, &JobCtx::new()).unwrap();
        let legacy_time = start.elapsed();
        let start = std::time::Instant::now();
        let local = prepare(&doc, &JobCtx::new()).unwrap();
        let local_time = start.elapsed();
        println!(
            "{}MP whole-canvas input={legacy_time:?}, ROI input={local_time:?}, sourceROI={:?}, model={:?}, oldModel={}x{}",
            u64::from(width) * u64::from(height) / 1_000_000,
            local.region.bounds,
            local.region.content_size,
            legacy.image.width,
            legacy.image.height
        );
    }
}

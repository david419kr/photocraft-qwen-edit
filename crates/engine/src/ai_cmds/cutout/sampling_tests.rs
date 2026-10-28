use super::*;

#[test]
fn rectangular_guide_maps_the_padded_canvas_without_a_second_content_padding_shift() {
    let mut image = Rgba8Image::new(2016, 608);
    for y in 0..608usize {
        for x in 0..2016usize {
            image.pixels[(y * 2016 + x) * 4 + 3] = if x < 1000 && y < 400 { 255 } else { 0 };
        }
    }
    let guide = refine::guide(&image, &JobCtx::new()).unwrap();
    assert!(resolution::Budget::REFERENCE.accepts_frame([guide.width, guide.height]));
    let x = (f64::from(guide.width) * 1000.0 / 2016.0).floor() as usize;
    let y = (f64::from(guide.height) * 400.0 / 608.0).floor() as usize;
    let sample = |xx, yy| guide.pixels[(yy * guide.width as usize + xx) * 4];
    assert!(sample(x - 2, y / 2) > 240);
    assert!(sample(x + 2, y / 2) < 15);
    assert!(sample(x / 2, y - 2) > 240);
    assert!(sample(x / 2, y + 2) < 15);
    let mut malformed = image;
    malformed.pixels.pop();
    assert!(refine::guide(&malformed, &JobCtx::new()).is_err());
}

#[test]
fn work_mask_preserves_partial_opacity_padding_and_includes_details_missing_from_coarse() {
    let ctx = JobCtx::new();
    let bounds = Rect::new(35, 71, 548, 328);
    let mut source = Rgba8Image::new(544, 288);
    for p in source.pixels.as_chunks_mut::<4>().0 {
        p.copy_from_slice(&[90, 100, 110, 255]);
    }
    // A fine remote line is absent from the coarse matte; its RGB edge still receives work.
    for y in 0..257 {
        let i = (y * 544 + 470) * 4;
        source.pixels[i..i + 4].copy_from_slice(&[250, 250, 250, 255]);
    }
    let mut coarse = Surface::new(PixelFormat::new(ColorMode::Grayscale, photocraft_color::SampleType::F32, false));
    coarse.fill_rect(Rect::new(100, 100, 200, 220), &[1.0]);
    coarse.fill_rect(Rect::new(290, 140, 340, 200), &[0.37]);
    let s = sampling::prepare(&source, &coarse, bounds, &ctx).unwrap();
    assert_eq!(s.denoise, 0.25);
    let at = |image: &Rgba8Image, x: usize, y: usize| image.pixels[(y * image.width as usize + x) * 4];
    assert_eq!(at(&s.noise, 470, 80), 255);
    assert_eq!(at(&s.noise, 10, 200), 0);
    assert_eq!(at(&s.noise, 280, 100), 255);
    for y in 0..288 {
        for x in 0..544 {
            let i = (y * 544 + x) * 4;
            assert_eq!(&s.initial.pixels[i..i + 3], &source.pixels[i..i + 3]);
            assert_eq!(s.noise.pixels[i + 3], 255);
            if x >= 513 || y >= 257 {
                assert_eq!(s.noise.pixels[i], 0);
            }
        }
    }
    assert_eq!(s.initial.pixels[(100 * 544 + 280) * 4 + 3], byte(0.37));
    ctx.cancel();
    assert!(sampling::prepare(&source, &coarse, bounds, &ctx).is_err());
}

#[test]
fn unworked_native_alpha_keeps_float_precision_instead_of_rounding_through_png() {
    let bounds = Rect::new(0, 0, 64, 32);
    let mut source = Rgba8Image::new(64, 32);
    source.pixels.fill(255);
    let mut coarse = Surface::new(PixelFormat::new(ColorMode::Grayscale, photocraft_color::SampleType::F32, false));
    coarse.fill_rect(bounds, &[0.123456]);
    let mut s = sampling::prepare(&source, &coarse, bounds, &JobCtx::new()).unwrap();
    for p in s.noise.pixels.as_chunks_mut::<4>().0 {
        p.copy_from_slice(&[0, 0, 0, 255]);
    }
    let tile = tiles::Tile { core: bounds, context: bounds };
    let mut blend = tiles::Blend::new(bounds).unwrap();
    blend.add(tile, &source, Some((&s, &coarse)), &JobCtx::new()).unwrap();
    let mask = blend.finish(&JobCtx::new()).unwrap();
    assert_eq!(mask.sample_channel(15, 15, 0), 0.123456);
    let mut malformed = s.clone();
    malformed.noise.pixels.pop();
    let mut b = tiles::Blend::new(bounds).unwrap();
    assert!(b.add(tile, &source, Some((&malformed, &coarse)), &JobCtx::new()).is_err());
}

#[test]
fn initial_latent_does_not_increase_existing_source_alpha_and_bad_geometry_fails() {
    let bounds = Rect::new(0, 0, 64, 32);
    let mut source = Rgba8Image::new(64, 32);
    for p in source.pixels.as_chunks_mut::<4>().0 {
        p.copy_from_slice(&[80, 90, 100, 64]);
    }
    let mut coarse = Surface::new(PixelFormat::new(ColorMode::Grayscale, photocraft_color::SampleType::F32, false));
    coarse.fill_rect(bounds, &[0.75]);
    let s = sampling::prepare(&source, &coarse, bounds, &JobCtx::new()).unwrap();
    assert!(s.initial.pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 64));
    source.pixels.pop();
    assert!(sampling::prepare(&source, &coarse, bounds, &JobCtx::new()).is_err());
}

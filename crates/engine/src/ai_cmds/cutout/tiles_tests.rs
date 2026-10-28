use super::tiles::*;
use super::*;

fn image(tile: Tile, alpha: impl Fn(i32, i32) -> u8) -> Rgba8Image {
    let [w, h] = resolution::padded([tile.context.width(), tile.context.height()]).unwrap();
    let mut im = Rgba8Image::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let i = (y as usize * w as usize + x as usize) * 4;
            im.pixels[i..i + 4].copy_from_slice(&[255, 0, 255, alpha(x as i32 + tile.context.x0, y as i32 + tile.context.y0)]);
        }
    }
    im
}

#[test]
fn planning_covers_native_edges_with_halos_and_caps_before_any_inference() {
    for size in [[2048, 2048], [2752, 1536], [2400, 1792]] {
        assert!(plan(Rect::from_xywh(0, 0, size[0], size[1])).unwrap().is_empty());
    }
    let bounds = Rect::new(0, 0, 6000, 4000);
    let tiles = plan(bounds).unwrap();
    assert_eq!(tiles.len(), 15);
    for tile in &tiles {
        assert!(tile.context.contains_rect(&tile.core));
        assert!(bounds.contains_rect(&tile.context));
        assert!(resolution::Budget::EDIT.accepts_content([tile.context.width(), tile.context.height()]));
    }
    assert_eq!(tiles.first().unwrap().core.x0, 0);
    assert_eq!(tiles.last().unwrap().core.x1, 6000);
    assert_eq!(tiles.last().unwrap().core.y1, 4000);
    assert!(plan(Rect::new(0, 0, 6000, 6000)).is_err());
    assert!(plan(Rect::EMPTY).is_err());
    assert!(plan(Rect::new(1, 0, 6001, 4000)).is_err());
}

#[test]
fn normalized_overlap_has_no_holes_or_alpha_over_accumulation() {
    let bounds = Rect::new(0, 0, 4097, 33);
    let plan = plan(bounds).unwrap();
    assert_eq!(plan.len(), 3);
    let mut blend = Blend::new(bounds).unwrap();
    let expected = |x: i32, _: i32| 60 + (x % 100) as u8;
    for tile in plan {
        blend.add(tile, &image(tile, expected), None, &JobCtx::new()).unwrap();
    }
    let mask = blend.finish(&JobCtx::new()).unwrap();
    for y in 0..33 {
        for x in 0..4097 {
            assert!((mask.sample_channel(x, y, 0) - f32::from(expected(x, y)) / 255.0).abs() < 1e-6, "{x},{y}");
        }
    }
}

#[test]
fn opposite_classifications_missing_coverage_bad_buffers_and_cancel_never_produce_a_mask() {
    let bounds = Rect::new(0, 0, 4097, 32);
    let tiles = plan(bounds).unwrap();
    let mut blend = Blend::new(bounds).unwrap();
    for (i, tile) in tiles.iter().enumerate() {
        blend.add(*tile, &image(*tile, |_, _| if i == 0 { 0 } else { 255 }), None, &JobCtx::new()).unwrap();
    }
    assert!(blend.finish(&JobCtx::new()).unwrap_err().to_string().contains("disagree"));
    let mut blend = Blend::new(bounds).unwrap();
    blend.add(tiles[0], &image(tiles[0], |_, _| 128), None, &JobCtx::new()).unwrap();
    assert!(blend.finish(&JobCtx::new()).unwrap_err().to_string().contains("cover"));
    let mut blend = Blend::new(bounds).unwrap();
    let mut bad = image(tiles[0], |_, _| 128);
    bad.pixels.pop();
    assert!(blend.add(tiles[0], &bad, None, &JobCtx::new()).is_err());
    let ctx = JobCtx::new();
    ctx.cancel();
    assert!(blend.add(tiles[0], &image(tiles[0], |_, _| 128), None, &ctx).is_err());
    assert!(blend.finish(&ctx).is_err());
}

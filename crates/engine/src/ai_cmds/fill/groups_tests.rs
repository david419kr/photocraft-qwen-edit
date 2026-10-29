use super::*;
use serde_json::json;

fn fixture(rects: &[Rect]) -> Document {
    let mut s = super::super::tests::session(32);
    s.execute("file.new", json!({"width":1024,"height":768,"background":"white"})).unwrap();
    let mut doc = (*s.active().unwrap().doc).clone();
    let mut mask = Surface::new(gray_float());
    for rect in rects {
        mask.fill_rect(*rect, &[1.0]);
    }
    doc.selection = Some(mask);
    doc
}

fn plan(doc: &Document) -> Vec<Rect> {
    let selected = selection_info(doc, &JobCtx::new()).unwrap().0;
    groups::plan(doc, selected, &JobCtx::new()).unwrap()
}

#[test]
fn distant_components_split_but_nearby_strokes_and_overlapping_context_merge() {
    let a = Rect::new(20, 20, 40, 40);
    let b = Rect::new(900, 650, 920, 670);
    assert_eq!(plan(&fixture(&[a, b])), vec![a, b]);
    let c = Rect::new(45, 20, 60, 40);
    let parts = plan(&fixture(&[a, c, b]));
    assert_eq!(parts, vec![a.union(&c), b]);
    assert!(!crop(a, Rect::new(0, 0, 1024, 768)).intersect(&crop(c, Rect::new(0, 0, 1024, 768))).is_empty());
}

#[test]
fn weak_feather_bridges_are_never_discarded_or_split() {
    let a = Rect::new(20, 20, 40, 40);
    let b = Rect::new(900, 650, 920, 670);
    let mut doc = fixture(&[a, b]);
    let mask = doc.selection.as_mut().unwrap();
    mask.fill_rect(Rect::new(30, 30, 910, 31), &[1e-8]);
    mask.fill_rect(Rect::new(909, 30, 910, 660), &[1e-8]);
    assert_eq!(plan(&doc), vec![a.union(&b)]);
}

#[test]
fn diagonal_connectivity_and_tiny_dots_keep_native_support() {
    let a = Rect::new(20, 20, 21, 21);
    let b = Rect::new(21, 21, 22, 22);
    let c = Rect::new(900, 650, 901, 651);
    let doc = fixture(&[a, b, c]);
    assert_eq!(plan(&doc), vec![a.union(&b), c]);
    let inputs = prepare_groups(&doc, &JobCtx::new()).unwrap();
    for y in 0..768 {
        for x in 0..1024 {
            let original = doc.selection.as_ref().unwrap().sample_channel(x, y, 0);
            let sum: f32 = inputs.iter().map(|p| p.region.mask.as_ref().unwrap().sample_channel(x, y, 0)).sum();
            assert_eq!(sum, original);
        }
    }
}

#[test]
fn too_many_groups_dense_support_and_cancel_use_bounded_paths() {
    let many = [Rect::new(10, 10, 11, 11), Rect::new(300, 10, 301, 11), Rect::new(600, 10, 601, 11), Rect::new(900, 10, 901, 11), Rect::new(10, 650, 11, 651)];
    assert_eq!(plan(&fixture(&many)).len(), 1);
    let mut doc = fixture(&[]);
    let mask = doc.selection.as_mut().unwrap();
    for y in (0..768).step_by(3) {
        for x in (0..1024).step_by(3) {
            mask.fill_rect(Rect::new(x, y, x + 1, y + 1), &[1.0]);
        }
    }
    assert_eq!(plan(&doc).len(), 1);
    let ctx = JobCtx::new();
    ctx.cancel();
    assert!(groups::plan(&doc, doc.bounds(), &ctx).is_err());
}

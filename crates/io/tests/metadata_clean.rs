use photocraft_codecs::{ChannelLayout, Image, Metadata};
use photocraft_io::{ExportOptions, XmpEmbed, export, import};

#[test]
fn metadata_none_removes_private_chunks_and_alpha_bits_only_in_the_export() {
    let image = Image::from_u8(4, 1, ChannelLayout::Rgba, vec![30, 60, 90, 0, 30, 60, 90, 2, 30, 60, 90, 254, 30, 60, 90, 255])
        .unwrap()
        .with_icc(Some(photocraft_cms::Builtin::Srgb.profile().to_bytes().to_vec()))
        .with_meta(Metadata {
            xmp: Some("<private>prompt history</private>".into()),
            text: vec![("parameters".into(), "secret prompt".into())],
            exif: Some(vec![1, 2, 3]),
            dpi: Some((300.0, 300.0)),
            ..Default::default()
        });
    let png = photocraft_codecs::encode(&image, photocraft_codecs::Format::Png, &Default::default()).unwrap();
    let doc = import("private.png", &png).unwrap().document;
    let original = doc.clone();
    let kept = photocraft_codecs::decode(&export(&doc, "png", &ExportOptions::default()).unwrap().bytes).unwrap();
    let options = ExportOptions { xmp: XmpEmbed::None, ..Default::default() };
    let cleaned = photocraft_codecs::decode(&export(&doc, "png", &options).unwrap().bytes).unwrap();
    assert!(cleaned.icc.is_none());
    assert_eq!(cleaned.meta, Metadata::default());
    for (before, after) in kept.data().as_chunks::<4>().0.iter().zip(cleaned.data().as_chunks::<4>().0) {
        assert_eq!(before[..3], after[..3]);
        assert_eq!(after[3], before[3] | 1);
    }
    assert_eq!(doc, original);
    assert!(kept.meta.xmp.is_some());
    assert!(kept.meta.exif.is_some());
    assert!(!kept.meta.text.is_empty());
}

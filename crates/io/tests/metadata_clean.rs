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

#[test]
fn background_exports_keep_metadata_none_and_indexed_all_contracts() {
    use photocraft_color::{Color, ColorMode, SampleType};
    use photocraft_doc::{ColorTable, Document, Size};
    let mut doc = Document::with_background("indexed", Size::new(2, 1), ColorMode::Rgb, SampleType::U8, Color::rgba(1.0, 0.0, 0.0, 1.0));
    doc.mode = ColorMode::Indexed;
    doc.color_table = Some(ColorTable { colors: vec![[255, 0, 0]], transparent: None });
    doc.resolution_dpi = 300.0;
    doc.file_encoding.png_stereo = Some(photocraft_codecs::PngStereo::Horizontal.mode());
    doc.metadata.xmp = Some("<private>한국어 prompt history</private>".into());
    let original = doc.clone();
    for policy in [XmpEmbed::All, XmpEmbed::None] {
        let mut opts = ExportOptions { xmp: policy, ..Default::default() };
        photocraft_io::apply_file_encoding(&doc, &mut opts.encode);
        let foreground = export(&doc, "png", &opts).unwrap();
        let background = photocraft_io::export_with(&doc, "png", &opts, &photocraft_raster::Interrupt::NONE).unwrap();
        assert_eq!(foreground.bytes, background.bytes);
        let decoded = photocraft_codecs::decode(&background.bytes).unwrap();
        assert_eq!(photocraft_codecs::png_stereo(&background.bytes), Some(photocraft_codecs::PngStereo::Horizontal));
        if policy == XmpEmbed::All {
            assert_eq!(decoded.meta.xmp, doc.metadata.xmp);
            assert!((decoded.meta.dpi.unwrap().0 - 300.0).abs() < 0.01);
        } else {
            assert_eq!(decoded.meta, Metadata::default());
            assert!(decoded.icc.is_none());
        }
    }
    assert_eq!(doc, original);
}

#[test]
fn exif_only_keeps_camera_metadata_without_prompt_history_or_alpha_scrubbing() {
    use photocraft_color::{Color, ColorMode, SampleType};
    use photocraft_doc::{Document, Size};
    let mut doc = Document::with_background("exif", Size::new(2, 1), ColorMode::Rgb, SampleType::U8, Color::rgba(0.2, 0.4, 0.6, 0.0));
    doc.icc_profile = Some(photocraft_cms::Builtin::Srgb.profile().to_bytes());
    doc.resolution_dpi = 300.0;
    doc.metadata.exif = Some(vec![1, 2, 3].into());
    doc.metadata.xmp = Some("<private>한국어 prompt history</private>".into());
    doc.metadata.text.push(("parameters".into(), "private prompt".into()));
    let original = doc.clone();
    let opts = ExportOptions { xmp: XmpEmbed::ExifOnly, ..Default::default() };
    let bytes = export(&doc, "png", &opts).unwrap().bytes;
    let background = photocraft_io::export_with(&doc, "png", &opts, &photocraft_raster::Interrupt::NONE).unwrap();
    assert_eq!(bytes, background.bytes);
    let decoded = photocraft_codecs::decode(&bytes).unwrap();
    assert_eq!(decoded.meta.exif.as_deref(), doc.metadata.exif.as_deref().map(Vec::as_slice));
    assert!(decoded.meta.xmp.is_none());
    assert!(decoded.meta.text.is_empty());
    assert_eq!(decoded.icc.as_deref(), doc.icc_profile.as_deref().map(Vec::as_slice));
    assert!((decoded.meta.dpi.unwrap().0 - 300.0).abs() < 0.01);
    assert!(decoded.data().as_chunks::<4>().0.iter().all(|p| p[3] == 0));
    assert_eq!(doc, original);
}

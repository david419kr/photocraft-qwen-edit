use photocraft_codecs::{ChannelLayout, EncodeOptions, Format, Image, Metadata, SampleType, decode, encode, encode_png_indexed_clean};

fn clean() -> EncodeOptions {
    EncodeOptions { embed_metadata: false, embed_icc: false, png_scrub_alpha_lsb: true, ..Default::default() }
}

fn chunks(bytes: &[u8]) -> Vec<[u8; 4]> {
    let mut rest = &bytes[8..];
    let mut kinds = Vec::new();
    while !rest.is_empty() {
        let length = u32::from_be_bytes(rest[..4].try_into().unwrap()) as usize;
        kinds.push(rest[4..8].try_into().unwrap());
        rest = &rest[length + 12..];
    }
    kinds
}

#[test]
fn clean_rgba8_erases_alpha_payload_and_all_metadata_without_touching_rgb_or_source() {
    // All possible alpha values include any alpha-LSB signature or payload bit stream.
    let pixels: Vec<u8> = (0..=255).flat_map(|alpha| [23, 67, 101, alpha]).collect();
    let source = Image::from_u8(256, 1, ChannelLayout::Rgba, pixels.clone()).unwrap().with_icc(Some(vec![1, 2, 3])).with_meta(Metadata {
        xmp: Some("<private>AI prompt</private>".into()),
        exif: Some(vec![4, 5, 6]),
        text: vec![("parameters".into(), "secret generation prompt".into())],
        dpi: Some((300.0, 300.0)),
        ..Default::default()
    });
    for interlaced in [false, true] {
        let options = EncodeOptions { png_interlaced: interlaced, ..clean() };
        let bytes = encode(&source, Format::Png, &options).unwrap();
        assert!(chunks(&bytes).iter().all(|kind| matches!(kind, b"IHDR" | b"IDAT" | b"IEND")));
        let restored = decode(&bytes).unwrap();
        assert!(restored.icc.is_none());
        assert_eq!(restored.meta, Metadata::default());
        for (before, after) in pixels.as_chunks::<4>().0.iter().zip(restored.data().as_chunks::<4>().0) {
            assert_eq!(before[..3], after[..3]);
            assert_eq!(after[3], before[3] | 1);
            assert!(after[3].abs_diff(before[3]) <= 1);
        }
    }
    assert_eq!(source.data(), pixels);
    assert!(source.meta.xmp.is_some());
}

#[test]
fn clean_rgba16_scrubs_the_low_bit_after_big_endian_conversion() {
    let alphas = [0, 1, 2, 254, 255, 256, 32766, 32767, 65534, 65535];
    let pixels: Vec<u16> = alphas.into_iter().flat_map(|alpha| [0x1234, 0x5678, 0x9abc, alpha]).collect();
    let source = Image::from_u16(alphas.len() as u32, 1, ChannelLayout::Rgba, &pixels).unwrap();
    let bytes = encode(&source, Format::Png, &clean()).unwrap();
    let restored = decode(&bytes).unwrap();
    assert_eq!(restored.sample_type(), SampleType::U16);
    let got: Vec<u16> = restored.data().as_chunks::<2>().0.iter().map(|pair| u16::from_ne_bytes(*pair)).collect();
    for (before, after) in pixels.as_chunks::<4>().0.iter().zip(got.as_chunks::<4>().0) {
        assert_eq!(before[..3], after[..3]);
        assert_eq!(after[3], before[3] | 1);
        assert!(after[3].abs_diff(before[3]) <= 1);
    }
}

#[test]
fn ordinary_encoding_keeps_alpha_bits_and_metadata() {
    let source = Image::from_u8(2, 1, ChannelLayout::GrayA, vec![120, 0, 200, 254])
        .unwrap()
        .with_meta(Metadata { text: vec![("parameters".into(), "keep me".into())], ..Default::default() });
    let restored = decode(&encode(&source, Format::Png, &EncodeOptions::default()).unwrap()).unwrap();
    assert_eq!(restored.data(), source.data());
    assert_eq!(restored.meta.text, source.meta.text);
    let stripped = decode(&encode(&source, Format::Png, &clean()).unwrap()).unwrap();
    assert_eq!(stripped.data(), &[120, 1, 200, 255]);
}

#[test]
fn palette_transparency_cannot_keep_an_alpha_lsb_payload() {
    let palette = [[10, 20, 30], [90, 100, 110]];
    let bytes = encode_png_indexed_clean(4, 1, &[0, 1, 0, 1], &palette, Some(0)).unwrap();
    assert!(chunks(&bytes).iter().all(|kind| matches!(kind, b"IHDR" | b"PLTE" | b"tRNS" | b"IDAT" | b"IEND")));
    let restored = decode(&bytes).unwrap();
    assert_eq!(restored.data(), &[10, 20, 30, 1, 90, 100, 110, 255, 10, 20, 30, 1, 90, 100, 110, 255]);
}

#[test]
fn clean_opaque_png_does_not_add_alpha_or_change_colour_samples() {
    let source = Image::from_u8(2, 1, ChannelLayout::Rgb, vec![10, 20, 30, 40, 50, 60]).unwrap();
    let restored = decode(&encode(&source, Format::Png, &clean()).unwrap()).unwrap();
    assert_eq!(restored.layout(), ChannelLayout::Rgb);
    assert_eq!(restored.data(), source.data());
}

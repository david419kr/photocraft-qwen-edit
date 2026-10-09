//! Synthetic fixtures produced with independent libaom/libheif encoders (see data/avif/README.md).
#![cfg(feature = "avif-decode")]

use photocraft_codecs::*;

const RGB: &[u8] = include_bytes!("data/avif/rgb.avif");
const RGBA: &[u8] = include_bytes!("data/avif/rgba.avif");
const TEN_BIT: &[u8] = include_bytes!("data/avif/ten-bit.avif");
const TWELVE_BIT: &[u8] = include_bytes!("data/avif/twelve-bit.avif");

#[cfg(feature = "avif")]
#[test]
fn avif_encoder_writes_quality_controlled_rgba_and_reopens_it() {
    let original = decode(RGBA).unwrap();
    for quality in [20, 90, 100] {
        let bytes = encode(&original, Format::Avif, &EncodeOptions { jpeg_quality: quality, ..Default::default() }).unwrap();
        assert_eq!(detect(&bytes), Some(Format::Avif));
        let back = decode(&bytes).unwrap();
        assert_eq!(back.dimensions(), original.dimensions());
        assert_eq!(back.layout(), ChannelLayout::Rgba);
        assert_eq!(back.sample_type(), SampleType::U8);
        assert!((back.get(24, 8, 3) - 128.0 / 255.0).abs() < 0.02);
        assert!(back.get(8, 8, 0) > 0.9 && back.get(24, 8, 1) > 0.9);
    }
}

#[test]
fn independently_encoded_rgb_is_readable() {
    assert!(caps(Format::Avif).read);
    assert_eq!(detect(RGB), Some(Format::Avif));
    let img = decode(RGB).unwrap();
    assert_eq!(img.dimensions(), (32, 16));
    assert_eq!(img.sample_type(), SampleType::U8);
    assert_eq!(img.layout(), ChannelLayout::Rgb);
    assert!(img.get(16, 8, 0) > 0.98);
    assert!(img.get(16, 8, 1) < 0.02 && img.get(16, 8, 2) < 0.02);
}

#[test]
fn auxiliary_alpha_is_straight_and_preserved() {
    let img = decode(RGBA).unwrap();
    assert_eq!(img.dimensions(), (32, 16));
    assert_eq!(img.layout(), ChannelLayout::Rgba);
    assert!(img.get(8, 8, 0) > 0.98 && img.get(8, 8, 3) > 0.99);
    assert!(img.get(24, 8, 1) > 0.98);
    assert!((img.get(24, 8, 3) - 128.0 / 255.0).abs() < 0.005);
}

#[test]
fn premultiplied_colour_is_unassociated_for_photocraft() {
    let source = oxideav_avif::AvifImage::from_rgba8(4, 2, [64, 0, 0, 128].repeat(8)).unwrap();
    let bytes = oxideav_avif::encode(&source, &oxideav_avif::EncodeOptions::default().with_premultiplied_alpha(true)).unwrap();
    let img = decode(&bytes).unwrap();
    assert!((img.get(1, 1, 0) - 0.5).abs() < 0.015);
    assert!((img.get(1, 1, 3) - 128.0 / 255.0).abs() < 0.005);
}

#[test]
fn ten_bit_sdr_keeps_precision_in_u16() {
    let img = decode(TEN_BIT).unwrap();
    assert_eq!(img.sample_type(), SampleType::U16);
    assert_eq!(img.dimensions(), (32, 16));
    assert!((img.get(8, 8, 0) - 0.5).abs() < 0.015);
    let v = u16::from_ne_bytes(img.data().get(..2).unwrap().try_into().unwrap());
    assert_ne!(v % 257, 0, "not 8-bit data expanded to 16 bits");
}

#[test]
fn twelve_bit_sdr_keeps_precision_in_u16() {
    let img = decode(TWELVE_BIT).unwrap();
    assert_eq!(img.sample_type(), SampleType::U16);
    assert!((img.get(8, 8, 0) - 0.5).abs() < 0.015);
    let v = u16::from_ne_bytes(img.data().get(..2).unwrap().try_into().unwrap());
    assert_ne!(v % 257, 0);
}

#[test]
fn untagged_hdr_is_refused_instead_of_assuming_srgb() {
    let mut hdr = TEN_BIT.to_vec();
    let at = hdr.windows(4).position(|w| w == b"nclx").unwrap() + 6;
    hdr[at..at + 2].copy_from_slice(&16_u16.to_be_bytes());
    assert!(matches!(decode(&hdr), Err(CodecError::Unsupported { format: Format::Avif, reason }) if reason.contains("ICC")));
}

#[test]
fn declared_dimensions_and_allocation_limits_are_checked() {
    for limits in [
        Limits { max_width: 31, ..Default::default() },
        Limits { max_height: 15, ..Default::default() },
        Limits { max_pixels: 511, ..Default::default() },
        Limits { max_alloc: 1535, ..Default::default() },
    ] {
        assert!(matches!(decode_with(RGB, &DecodeOptions { limits, ..Default::default() }), Err(CodecError::LimitExceeded(_))));
    }
    assert!(decode_with(RGB, &DecodeOptions { limits: Limits { max_pixels: 512, ..Default::default() }, ..Default::default() }).is_ok());
}

#[test]
fn damaged_and_truncated_files_fail_without_panicking() {
    for end in [0, 12, 24, 50, RGB.len() / 2, RGB.len() - 1] {
        assert!(decode_as(Format::Avif, &RGB[..end]).is_err());
    }
    let mut bad = RGB.to_vec();
    let at = bad.windows(4).position(|w| w == b"ispe").unwrap() + 8;
    bad[at..at + 4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(matches!(decode(&bad), Err(CodecError::LimitExceeded(_))));
}

#[test]
fn png_export_keeps_imported_alpha() {
    let img = decode(RGBA).unwrap();
    let png = encode(&img, Format::Png, &EncodeOptions::default()).unwrap();
    assert_eq!(decode(&png).unwrap().data(), img.data());
}

#[test]
fn metadata_survives_and_exif_does_not_rotate_the_container_twice() {
    let exif = vec![b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0];
    let xmp = b"<x:xmpmeta>synthetic</x:xmpmeta>".to_vec();
    let source = oxideav_avif::AvifImage::from_rgb8(4, 2, vec![100; 24]).unwrap().with_metadata(oxideav_avif::Metadata::new(
        Some(b"synthetic-icc".to_vec()),
        Some(exif.clone()),
        Some(xmp.clone()),
        None,
    ));
    let bytes = oxideav_avif::encode(&source, &oxideav_avif::EncodeOptions::default()).unwrap();
    let decoded = decode(&bytes).unwrap();
    assert_eq!(decoded.dimensions(), (4, 2));
    assert_eq!(decoded.icc.as_deref(), Some(b"synthetic-icc".as_slice()));
    assert_eq!(decoded.meta.xmp.as_deref(), Some("<x:xmpmeta>synthetic</x:xmpmeta>"));
    assert_eq!(exif_orientation(decoded.meta.exif.as_deref().unwrap()), 1);
    let raw = decode_with(&bytes, &DecodeOptions { keep_orientation: true, ..Default::default() }).unwrap();
    assert_eq!(raw.meta.exif, Some(exif));
}

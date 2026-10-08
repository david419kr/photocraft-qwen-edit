//! Bundled, pure-Rust AV1 decode. Container orientation is applied once, never again from EXIF.

use crate::{CodecError, Format, Image, Limits};

#[cfg(not(feature = "avif-decode"))]
pub(crate) fn decode(_bytes: &[u8], _limits: &Limits, _keep_orientation: bool) -> Result<Image, CodecError> {
    Err(CodecError::unsupported(Format::Avif, "AVIF support isn't included in this build of PhotoCraft"))
}

#[cfg(feature = "avif-decode")]
pub(crate) fn decode(bytes: &[u8], limits: &Limits, keep_orientation: bool) -> Result<Image, CodecError> {
    // Contain a third-party parser/decoder panic, as the HEIF reader does.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| decode_inner(bytes, limits, keep_orientation)))
        .unwrap_or_else(|_| Err(CodecError::malformed(Format::Avif, "AVIF decoder rejected damaged data")))
}

#[cfg(feature = "avif-decode")]
fn error(e: oxideav_avif::AvifError) -> CodecError {
    match e {
        oxideav_avif::AvifError::Unsupported(s) => CodecError::unsupported(Format::Avif, s),
        oxideav_avif::AvifError::LimitExceeded(s) => CodecError::LimitExceeded(s),
        other => CodecError::malformed(Format::Avif, other),
    }
}

#[cfg(feature = "avif-decode")]
fn decode_inner(bytes: &[u8], limits: &Limits, keep_orientation: bool) -> Result<Image, CodecError> {
    use crate::{ChannelLayout, DecodeWarning, Metadata, SampleType};

    if bytes.len() as u64 > limits.max_alloc {
        return Err(CodecError::LimitExceeded("AVIF input exceeds max_alloc".into()));
    }
    let info = oxideav_avif::info(bytes).map_err(error)?;
    let layout = if info.has_alpha { ChannelLayout::Rgba } else { ChannelLayout::Rgb };
    let sample = if info.bit_depth > 8 { SampleType::U16 } else { SampleType::U8 };
    limits.check(info.width, info.height, layout, sample)?;
    let mut options = oxideav_avif::DecodeOptions::default();
    options.max_width = Some(limits.max_width.min(16_384));
    options.max_height = Some(limits.max_height.min(16_384));
    options.max_pixels = Some(limits.max_pixels.min(limits.max_alloc / 8));
    options.max_bytes = Some(limits.max_alloc);
    let decoded = oxideav_avif::decode_with(bytes, &options).map_err(error)?;
    let premultiplied = if decoded.has_alpha() && !info.is_sequence { oxideav_avif::inspect(bytes).map_err(error)?.premultiplied_alpha } else { false };
    let mut img = pixels(decoded, limits, premultiplied)?;
    // Metadata travels with the source colour model; container transforms outrank EXIF.
    img.meta = Metadata {
        exif: img.meta.exif.map(|e| if keep_orientation { e } else { crate::upright_exif(&e).into_owned() }),
        xmp: img.meta.xmp.map(|x| if keep_orientation { x } else { crate::upright_xmp(&x).into_owned() }),
        ..Default::default()
    };
    if info.frames > 1 {
        img.warnings.push(DecodeWarning::MoreFrames { total: Some(info.frames) });
    }
    Ok(img)
}

#[cfg(feature = "avif-decode")]
fn pixels(decoded: oxideav_avif::AvifImage, limits: &Limits, premultiplied: bool) -> Result<Image, CodecError> {
    use crate::{ChannelLayout, Metadata, SampleType};
    use oxideav_heif::{Chroma, HeifFrame, HeifPixelFormat, Plane};

    let malformed = |e| CodecError::malformed(Format::Avif, e);
    if !matches!(decoded.bit_depth, 8 | 10 | 12) {
        return Err(CodecError::unsupported(Format::Avif, "only 8, 10 and 12-bit AVIF are supported"));
    }
    let color = decoded.color;
    // Until CICP-to-ICC is available, refuse untagged wide-gamut/HDR rather than display wrong colours.
    if decoded.metadata.icc.is_none() && (!matches!(color.primaries, 1 | 2) || !matches!(color.transfer, 2 | 13)) {
        return Err(CodecError::unsupported(Format::Avif, "wide-gamut or HDR AVIF needs an embedded ICC profile"));
    }
    let layout = if decoded.has_alpha() { ChannelLayout::Rgba } else { ChannelLayout::Rgb };
    let sample = if decoded.bit_depth > 8 { SampleType::U16 } else { SampleType::U8 };
    limits.check(decoded.width, decoded.height, layout, sample)?;
    let monochrome = decoded.format.is_packed_ya() || decoded.format.plane_count() == 1;
    let chroma = if monochrome {
        Chroma::Mono
    } else {
        match decoded.format.chroma_subsampling() {
            (1, 1) => Chroma::Yuv420,
            (1, 0) => Chroma::Yuv422,
            (0, 0) => Chroma::Yuv444,
            _ => return Err(CodecError::unsupported(Format::Avif, "unsupported AVIF chroma layout")),
        }
    };
    let format = HeifPixelFormat::new(chroma, decoded.bit_depth, layout.has_alpha()).map_err(malformed)?;
    let planes = if decoded.format.is_packed_ya() {
        let src = decoded.planes.first().ok_or_else(|| CodecError::malformed(Format::Avif, "missing YA plane"))?;
        let bps = decoded.format.bytes_per_sample();
        let row_bytes = (decoded.width as usize).checked_mul(2 * bps).ok_or_else(|| CodecError::LimitExceeded("AVIF row size overflow".into()))?;
        let mut y = Vec::new();
        let mut a = Vec::new();
        for row in 0..decoded.height as usize {
            let start = row.checked_mul(src.stride).ok_or_else(|| CodecError::LimitExceeded("AVIF row offset overflow".into()))?;
            let end = start.checked_add(row_bytes).ok_or_else(|| CodecError::LimitExceeded("AVIF row end overflow".into()))?;
            let bytes = src.data.get(start..end).ok_or_else(|| CodecError::malformed(Format::Avif, "truncated YA plane"))?;
            for px in bytes.chunks_exact(2 * bps) {
                y.extend_from_slice(px.get(..bps).unwrap_or_default());
                a.extend_from_slice(px.get(bps..).unwrap_or_default());
            }
        }
        vec![Plane { stride: row_bytes / 2, data: y }, Plane { stride: row_bytes / 2, data: a }]
    } else {
        decoded.planes.into_iter().map(|p| Plane { stride: p.stride, data: p.data }).collect()
    };
    let frame = HeifFrame { width: decoded.width, height: decoded.height, format, planes };
    frame.validate().map_err(malformed)?;
    let mut out = Image::new(frame.width, frame.height, layout, sample)?;
    let colr = oxideav_heif::props::Colr::Nclx {
        primaries: color.primaries.into(),
        transfer: color.transfer.into(),
        matrix: color.matrix.into(),
        full_range: color.range.is_full(),
    };
    let coded_max = f64::from((1_u32 << decoded.bit_depth) - 1);
    let max = if sample == SampleType::U8 { 255.0 } else { 65535.0 };
    let mut dest = out.data_mut().chunks_exact_mut(layout.channels() * sample.bytes());
    oxideav_heif::rgb::for_each_pixel(frame.width, frame.height, format, &frame.planes, Some(&colr), |rgb, alpha| {
        let alpha = f64::from(alpha) / coded_max;
        if let Some(px) = dest.next() {
            for (c, dst) in px.chunks_exact_mut(sample.bytes()).enumerate() {
                let value = if c == 3 {
                    alpha
                } else {
                    let color = rgb.get(c).copied().unwrap_or(0.0);
                    if premultiplied { if alpha > 0.0 { color / alpha } else { 0.0 } } else { color }
                };
                let v = (value * max).round().clamp(0.0, max) as u16;
                if sample == SampleType::U8 {
                    dst.copy_from_slice(&[v as u8]);
                } else {
                    dst.copy_from_slice(&v.to_ne_bytes());
                }
            }
        }
    })
    .map_err(malformed)?;
    out.icc = decoded.metadata.icc;
    out.meta = Metadata {
        exif: decoded.metadata.exif,
        xmp: decoded.metadata.xmp.map(String::from_utf8).transpose().map_err(|e| CodecError::malformed(Format::Avif, e))?,
        ..Default::default()
    };
    Ok(out)
}

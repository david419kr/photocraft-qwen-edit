//! XMP in the AVIF Exif item's standard TIFF XMLPacket tag (700).
const MAX_XMP: usize = 16 << 20;

#[cfg(any(feature = "avif", test))]
pub(super) fn exif_packet(xmp: &str) -> Result<Vec<u8>, crate::CodecError> {
    if xmp.len() > MAX_XMP {
        return Err(crate::CodecError::encode(crate::Format::Avif, "XMP packet is too large"));
    }
    let length = u32::try_from(xmp.len()).map_err(|_| crate::CodecError::encode(crate::Format::Avif, "XMP packet is too large"))?;
    let mut out = Vec::new();
    out.try_reserve_exact(26usize.saturating_add(xmp.len())).map_err(|_| crate::CodecError::encode(crate::Format::Avif, "not enough memory for XMP"))?;
    out.extend_from_slice(b"II\x2a\0\x08\0\0\0\x01\0");
    out.extend_from_slice(&700u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&length.to_le_bytes());
    if xmp.len() <= 4 {
        out.extend_from_slice(xmp.as_bytes());
        out.resize(22, 0);
        out.extend_from_slice(&0u32.to_le_bytes());
    } else {
        out.extend_from_slice(&26u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(xmp.as_bytes());
    }
    Ok(out)
}

#[cfg(any(feature = "avif-decode", test))]
pub(super) fn xmp(exif: &[u8]) -> Option<String> {
    let little = match exif.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let u16_at = |offset: usize| -> Option<u16> {
        let value: [u8; 2] = exif.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
        Some(if little { u16::from_le_bytes(value) } else { u16::from_be_bytes(value) })
    };
    let u32_at = |offset: usize| -> Option<u32> {
        let value: [u8; 4] = exif.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
        Some(if little { u32::from_le_bytes(value) } else { u32::from_be_bytes(value) })
    };
    if u16_at(2)? != 42 {
        return None;
    }
    let offset = usize::try_from(u32_at(4)?).ok()?;
    let count = usize::from(u16_at(offset)?);
    for index in 0..count.min(4096) {
        let entry = offset.checked_add(2)?.checked_add(index.checked_mul(12)?)?;
        if u16_at(entry)? != 700 {
            continue;
        }
        if !matches!(u16_at(entry.checked_add(2)?)?, 1 | 2 | 7) {
            return None;
        }
        let length = usize::try_from(u32_at(entry.checked_add(4)?)?).ok()?;
        if length > MAX_XMP {
            return None;
        }
        let field = entry.checked_add(8)?;
        let at = if length <= 4 { field } else { usize::try_from(u32_at(field)?).ok()? };
        return std::str::from_utf8(exif.get(at..at.checked_add(length)?)?).ok().map(|text| text.trim_end_matches('\0').to_owned());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xml_packet_round_trip_handles_unicode_and_short_inline_fields() {
        for xml in ["x", "abcd", "<x>언덕 위에 소녀 🌙</x>"] {
            assert_eq!(xmp(&exif_packet(xml).unwrap()).as_deref(), Some(xml));
        }
        for bytes in [vec![], vec![0; 8], b"II\x2a\0\xff\xff\xff\xff".to_vec()] {
            assert!(xmp(&bytes).is_none());
        }
    }
}

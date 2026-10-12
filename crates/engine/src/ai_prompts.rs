//! Ordered AI prompt provenance, stored in the document's XMP and therefore in undo snapshots.
use crate::ai_cmds::{EditMode, MAX_PROMPT_BYTES};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use photocraft_raster::Rgba8Image;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

const MAX_JSON_BYTES: usize = 8 << 20;
const MAX_ENTRIES: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Region {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}
impl Region {
    pub const ALL: [Self; 9] =
        [Self::TopLeft, Self::Top, Self::TopRight, Self::Left, Self::Center, Self::Right, Self::BottomLeft, Self::Bottom, Self::BottomRight];
    pub fn label(self) -> &'static str {
        match self {
            Self::TopLeft => "Top left",
            Self::Top => "Top center",
            Self::TopRight => "Top right",
            Self::Left => "Middle left",
            Self::Center => "Center",
            Self::Right => "Middle right",
            Self::BottomLeft => "Bottom left",
            Self::Bottom => "Bottom center",
            Self::BottomRight => "Bottom right",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub mode: EditMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<Region>,
    pub original: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_original: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub negative_translated: Option<String>,
}
#[derive(Deserialize)]
struct Packet {
    version: u8,
    entries: Vec<Entry>,
}
#[derive(Serialize)]
struct PacketRef<'a> {
    version: u8,
    entries: &'a [Entry],
}

pub fn read(xmp: Option<&str>) -> Result<Vec<Entry>, String> {
    let Some(payload) = photocraft_io::ai_metadata::payload(xmp)? else { return Ok(Vec::new()) };
    if payload.len() % 2 != 0 || payload.len() > photocraft_io::ai_metadata::MAX_XMP_BYTES {
        return Err("Invalid prompt history size".into());
    }
    let mut compressed = Vec::new();
    compressed.try_reserve_exact(payload.len() / 2).map_err(|_| "Not enough memory for prompt history")?;
    for pair in payload.as_bytes().as_chunks::<2>().0.iter() {
        let [a, b] = pair;
        let a = char::from(*a).to_digit(16).ok_or("Invalid prompt history encoding")?;
        let b = char::from(*b).to_digit(16).ok_or("Invalid prompt history encoding")?;
        compressed.push((a * 16 + b) as u8);
    }
    let mut bytes = Vec::new();
    ZlibDecoder::new(compressed.as_slice())
        .take(MAX_JSON_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Invalid compressed prompt history: {e}"))?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err("Prompt history exceeds the size limit".into());
    }
    let packet: Packet = serde_json::from_slice(&bytes).map_err(|e| format!("Invalid prompt history: {e}"))?;
    if packet.version != 1
        || packet.entries.len() > MAX_ENTRIES
        || packet.entries.iter().any(|entry| {
            entry.original.len() > MAX_PROMPT_BYTES
                || entry.translated.as_ref().is_some_and(|text| text.len() > MAX_PROMPT_BYTES)
                || entry.regions.len() > 9
                || entry.negative_original.as_ref().is_some_and(|text| text.len() > MAX_PROMPT_BYTES)
                || entry.negative_translated.as_ref().is_some_and(|text| text.len() > MAX_PROMPT_BYTES)
        })
    {
        return Err("Unsupported or oversized prompt history".into());
    }
    Ok(packet.entries)
}

pub fn append(xmp: Option<&str>, entry: &Entry) -> Result<String, String> {
    if entry.original.len() > MAX_PROMPT_BYTES || entry.translated.as_ref().is_some_and(|text| text.len() > MAX_PROMPT_BYTES) || entry.regions.len() > 9 {
        return Err("Invalid prompt history entry".into());
    }
    if entry.negative_original.as_ref().is_some_and(|text| text.len() > MAX_PROMPT_BYTES)
        || entry.negative_translated.as_ref().is_some_and(|text| text.len() > MAX_PROMPT_BYTES)
    {
        return Err("Invalid negative prompt history entry".into());
    }
    let mut entries = read(xmp)?;
    if entries.len() >= MAX_ENTRIES {
        return Err("Prompt history contains too many entries".into());
    }
    entries.push(entry.clone());
    let bytes = serde_json::to_vec(&PacketRef { version: 1, entries: &entries }).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err("Prompt history exceeds the size limit".into());
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&bytes).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;
    let capacity = compressed.len().checked_mul(2).ok_or("Prompt history size overflow")?;
    let mut payload = String::new();
    payload.try_reserve_exact(capacity).map_err(|_| "Not enough memory for prompt history")?;
    for byte in compressed {
        payload.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        payload.push(char::from_digit(u32::from(byte & 15), 16).unwrap_or('0'));
    }
    photocraft_io::ai_metadata::with_payload(xmp, &payload)
}

/// Meaningful mask coverage in a 3x3 grid; excludes padding and tiny feather tails.
pub fn regions(image: &Rgba8Image, size: [u32; 2]) -> Vec<Region> {
    let [width, height] = size;
    if width == 0 || height == 0 || image.width < width || image.height < height {
        return Vec::new();
    }
    let Some(stride) = usize::try_from(image.width).ok().and_then(|w| w.checked_mul(4)) else { return Vec::new() };
    if stride == 0 {
        return Vec::new();
    }
    let mut weights = [0u64; 9];
    for (y, row) in image.pixels.chunks_exact(stride).take(height as usize).enumerate() {
        for (x, pixel) in row.as_chunks::<4>().0.iter().take(width as usize).enumerate() {
            let weight = pixel.get(3).map_or(0, |alpha| u64::from(255 - *alpha));
            let cell = (y * 3 / height as usize).min(2) * 3 + (x * 3 / width as usize).min(2);
            if let Some(total) = weights.get_mut(cell) {
                *total = total.saturating_add(weight);
            }
        }
    }
    let max = weights.iter().copied().max().unwrap_or(0);
    weights.into_iter().enumerate().filter(|(_, weight)| *weight > 0 && *weight >= max / 10).filter_map(|(cell, _)| Region::ALL.get(cell).copied()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prompt_history_round_trips_unicode_preserves_foreign_xmp_and_rejects_corruption() {
        let foreign = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><r:RDF xmlns:r=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><r:Description xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:title>원본 제목</dc:title></r:Description></r:RDF></x:xmpmeta>";
        let entry = Entry {
            mode: EditMode::PromptGenerate,
            regions: Vec::new(),
            original: "언덕 위 소녀 <&> 🌙".into(),
            translated: Some("A girl on a hill".into()),
            negative_original: None,
            negative_translated: None,
        };
        let first = append(Some(foreign), &entry).unwrap();
        let second = append(Some(&first), &entry).unwrap();
        assert_eq!(read(Some(&second)).unwrap(), vec![entry.clone(), entry]);
        assert!(second.contains("<dc:title>원본 제목</dc:title>"));
        assert_eq!(second.matches("<pcraft:History ").count(), 1);
        let stripped = photocraft_io::ai_metadata::only_history(Some(&second)).unwrap().unwrap();
        assert_eq!(read(Some(&stripped)).unwrap().len(), 2);
        assert!(!stripped.contains("dc:title"));
        let corrupt = photocraft_io::ai_metadata::with_payload(None, "00").unwrap();
        assert!(read(Some(&corrupt)).is_err());
        assert!(read(Some("<broken urn:photocraft:ai-prompts:1>")).is_err());
        assert!(read(None).unwrap().is_empty());
    }

    #[test]
    fn prompt_history_regions_use_mask_coverage_and_exclude_padding() {
        let mut image = Rgba8Image::new(32, 32);
        for px in image.pixels.as_chunks_mut::<4>().0.iter_mut() {
            px[3] = 255;
        }
        for (x, y) in [(0usize, 0usize), (8, 8)] {
            image.pixels[(y * 32 + x) * 4 + 3] = 0;
        }
        // Padding pixels are not part of the document.
        image.pixels[(20 * 32 + 20) * 4 + 3] = 0;
        assert_eq!(regions(&image, [9, 9]), vec![Region::TopLeft, Region::BottomRight]);
    }
}

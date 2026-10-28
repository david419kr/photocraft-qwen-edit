//! Model-frame budgets are separate from the document's native dimensions.
use serde::{Deserialize, Serialize};

pub const TARGET_PIXELS: u64 = 2048 * 2048;
pub const MAX_PADDED_PIXELS: u64 = 2400 * 1792;
pub const MAX_PADDED_SIDE: u32 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    pub target_pixels: u64,
    pub max_pixels: u64,
    pub max_side: u32,
}

impl Budget {
    pub const EDIT: Self = Self { target_pixels: TARGET_PIXELS, max_pixels: MAX_PADDED_PIXELS, max_side: MAX_PADDED_SIDE };
    /// User-attached reference images follow the same frame budget as the edited document.
    pub const ATTACHED_REFERENCE: Self = Self::EDIT;
    /// Smaller internal guidance frames remain independent of user-attached references.
    pub const REFERENCE: Self = Self { target_pixels: 1024 * 1024, max_pixels: 1024 * 1024, max_side: MAX_PADDED_SIDE };

    pub fn accepts_frame(self, [w, h]: [u32; 2]) -> bool {
        w > 0
            && h > 0
            && w <= self.max_side
            && h <= self.max_side
            && w.is_multiple_of(32)
            && h.is_multiple_of(32)
            && u64::from(w) * u64::from(h) <= self.max_pixels
    }

    pub fn accepts_content(self, content: [u32; 2]) -> bool {
        padded(content).is_some_and(|frame| self.accepts_frame(frame))
    }

    /// Preserve native size when it fits, otherwise fit area and side guards together.
    /// `minimum_long` is only for fill ROI upsampling, never a document resize request.
    pub fn plan(self, native: [u32; 2], minimum_long: u32) -> Option<Frame> {
        let [w, h] = native;
        if w == 0 || h == 0 || self.target_pixels == 0 || self.max_pixels < 1024 || self.max_side < 32 {
            return None;
        }
        let long = w.max(h);
        let minimum = f64::from(minimum_long) / f64::from(long);
        let candidate = scaled(native, minimum.max(1.0));
        if let Some(frame) = self.frame(native, candidate) {
            return Some(frame);
        }
        let scale =
            ((self.target_pixels.min(self.max_pixels) as f64 / (f64::from(w) * f64::from(h))).sqrt()).min(f64::from(self.max_side) / f64::from(long)).min(1.0);
        if let Some(frame) = self.frame(native, scaled(native, scale)) {
            return Some(frame);
        }
        // Padding is a staircase. Search the scale, keeping the original aspect, rather than
        // trimming one axis or silently accepting a frame just over the allocation budget.
        let (mut low, mut high) = (0.0, scale);
        for _ in 0..64 {
            let mid = (low + high) * 0.5;
            if self.accepts_content(scaled(native, mid)) {
                low = mid;
            } else {
                high = mid;
            }
        }
        self.frame(native, scaled(native, low))
    }

    fn frame(self, native: [u32; 2], content: [u32; 2]) -> Option<Frame> {
        let frame = padded(content)?;
        self.accepts_frame(frame).then_some(Frame { native, content, padded: frame })
    }
}

fn scaled([w, h]: [u32; 2], scale: f64) -> [u32; 2] {
    [(f64::from(w) * scale).round().max(1.0) as u32, (f64::from(h) * scale).round().max(1.0) as u32]
}

pub fn padded([w, h]: [u32; 2]) -> Option<[u32; 2]> {
    if w == 0 || h == 0 {
        return None;
    }
    Some([w.checked_add(31)? / 32 * 32, h.checked_add(31)? / 32 * 32])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub native: [u32; 2],
    pub content: [u32; 2],
    pub padded: [u32; 2],
}

impl Frame {
    pub fn requires_resampling(self) -> bool {
        self.content != self.native
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn official_frames_and_native_odd_sizes_are_not_resized() {
        for size in [[2048, 2048], [2400, 1792], [1792, 2400], [2528, 1696], [1696, 2528], [2752, 1536], [1536, 2752], [2047, 2049], [17, 13]] {
            let p = Budget::EDIT.plan(size, 0).unwrap();
            assert_eq!(p.content, size);
            assert!(Budget::EDIT.accepts_frame(p.padded));
            assert!(!p.requires_resampling());
        }
    }

    #[test]
    fn padding_and_side_guard_hold_for_extreme_inputs() {
        for w in [1, 17, 511, 2048, 2401, 4096, 4097, 12000, u32::MAX] {
            for h in [1, 31, 2049, 1792, 4000, u32::MAX] {
                let p = Budget::EDIT.plan([w, h], 0).unwrap();
                assert!(Budget::EDIT.accepts_frame(p.padded), "{p:?}");
                assert!(p.content[0] <= w && p.content[1] <= h);
                let sx = f64::from(p.content[0]) / f64::from(w);
                let sy = f64::from(p.content[1]) / f64::from(h);
                assert!((sx - sy).abs() <= 1.0 / f64::from(w) + 1.0 / f64::from(h));
            }
        }
        assert!(!Budget::EDIT.accepts_content([2401, 1792]));
        assert!(Budget::EDIT.plan([0, 1], 0).is_none());
        assert!(padded([u32::MAX, 32]).is_none());
    }

    #[test]
    fn fill_minimum_is_long_side_and_reference_never_upscales() {
        let p = Budget::EDIT.plan([64, 128], 512).unwrap();
        assert_eq!(p.content, [256, 512]);
        assert_eq!(Budget::REFERENCE.plan([64, 128], 0).unwrap().content, [64, 128]);
        let p = Budget::REFERENCE.plan([6000, 4000], 0).unwrap();
        assert!(Budget::REFERENCE.accepts_frame(p.padded));
    }

    #[test]
    fn attached_reference_uses_edit_budget_without_changing_internal_guides() {
        for size in [[2048, 2048], [2400, 1792], [2752, 1536], [1536, 2752], [17, 13]] {
            let frame = Budget::ATTACHED_REFERENCE.plan(size, 0).unwrap();
            assert_eq!(frame.content, size);
            assert!(Budget::ATTACHED_REFERENCE.accepts_frame(frame.padded));
        }
        let large = Budget::ATTACHED_REFERENCE.plan([6000, 4000], 0).unwrap();
        assert!(Budget::ATTACHED_REFERENCE.accepts_frame(large.padded));
        assert!(u64::from(large.content[0]) * u64::from(large.content[1]) > 4_000_000);
        assert!(!Budget::REFERENCE.accepts_frame([2048, 2048]));
        assert!(Budget::REFERENCE.accepts_frame([1024, 1024]));
    }
}

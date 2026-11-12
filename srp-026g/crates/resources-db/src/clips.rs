// SPDX-License-Identifier: GPL-3.0-or-later
//! The length of an animation clip (`.b-spline`), as the engine takes it.
//!
//! The reader is the engine's `cubic_spline_skeleton_animation::length_in_frames`
//! as `srp-0100b`'s `combat/pose.rs` reads it (PROVEN there by the weapons'
//! reload clips: the TT-33's 65 and 125 frames, the TOZ-34's 80 and 110): the
//! file is
//!
//! ```text
//! u16 bones, u8 events, u8 (unused)
//! bones * 0x48 bytes, events * 0x10 bytes
//! u32 n, then n points of (f32 knot, f32 value): bone 0's x translation
//! ```
//!
//! and the length is the last knot less the first knot (from the fourth on)
//! whose span to the next is not degenerate. The unit is a frame, 30 a second.

/// Spans shorter than this are degenerate.
const EPSILON: f32 = 1e-6;

/// The length of the clip `bytes` in frames, or why it cannot be read. A clip
/// whose first bone has no usable knots is 0.
pub fn length_in_frames(bytes: &[u8]) -> Result<f32, String> {
    let short = || "the clip ends inside its header or its curve".to_owned();
    let bones = u16::from_le_bytes(bytes.get(0..2).ok_or_else(short)?.try_into().unwrap());
    let events = *bytes.get(2).ok_or_else(short)?;
    let at = 4 + usize::from(bones) * 0x48 + usize::from(events) * 0x10;
    let count =
        u32::from_le_bytes(bytes.get(at..at + 4).ok_or_else(short)?.try_into().unwrap()) as usize;
    let first = at + 4;
    let end = count
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(first))
        .ok_or_else(short)?;
    let curve = bytes.get(first..end).ok_or_else(short)?;
    if count < 5 {
        return Ok(0.0);
    }
    let knot = |i: usize| {
        let i = i.min(count - 1) * 8;
        f32::from_le_bytes(curve[i..i + 4].try_into().unwrap())
    };
    let first_span = (3..count - 1).find(|&i| knot(i + 1) - knot(i) >= EPSILON);
    Ok(first_span.map_or(0.0, |start| knot(count - 1) - knot(start)))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A clip of one bone and no events whose first bone's curve has `knots`.
    pub(crate) fn clip(knots: &[f32]) -> Vec<u8> {
        let mut bytes = vec![1, 0, 0, 0];
        bytes.extend(std::iter::repeat_n(0u8, 0x48));
        bytes.extend((knots.len() as u32).to_le_bytes());
        for knot in knots {
            bytes.extend(knot.to_le_bytes());
            bytes.extend(0f32.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn a_clip_is_as_long_as_its_curve_from_the_fourth_knot_to_the_last() {
        let bytes = clip(&[0.0, 0.0, 0.0, 0.0, 10.0, 40.0, 65.0, 65.0]);
        assert_eq!(length_in_frames(&bytes), Ok(65.0));
        let bytes = clip(&[0.0, 0.0, 0.0, 5.0, 10.0, 40.0, 125.0, 125.0]);
        assert_eq!(length_in_frames(&bytes), Ok(120.0));
    }

    #[test]
    fn a_curve_with_nothing_to_span_is_0_and_a_cut_clip_is_refused() {
        assert_eq!(length_in_frames(&clip(&[0.0, 1.0])), Ok(0.0));
        assert_eq!(length_in_frames(&clip(&[0.0; 8])), Ok(0.0));
        let bytes = clip(&[0.0, 0.0, 0.0, 0.0, 10.0, 40.0, 65.0, 65.0]);
        assert!(length_in_frames(&bytes[..bytes.len() - 3]).is_err());
        assert!(length_in_frames(&bytes[..3]).is_err());
        assert!(length_in_frames(&[]).is_err());
    }
}

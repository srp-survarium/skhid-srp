//! STUB for the drawing's proof (feat/reticle-draw); the table is the
//! server side's (`reticle/table.rs` of its branch), whose API this mirrors:
//! `drawn_for_dict(thread, dict) -> Option<Drawn>`.
//!
//! The stub gives every scope the same reticle, stepping every
//! [`STEP_MS`] through [`STEPS`], so that one aimed view shows each of
//! pattern, size, colour and brightness change it.

use core::ffi::CStr;

use crate::vostok::{Thread, clock};

/// A reticle's brightness, 0 to 10 (6 the default, 0.69d0's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Brightness(u8);

impl Brightness {
    pub const DEFAULT: Brightness = Brightness(6);
    pub const MOST: u8 = 10;

    pub fn new(value: u8) -> Option<Brightness> {
        (value <= Self::MOST).then_some(Brightness(value))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// What the exe draws for a scope: its pattern's model, the size's scale,
/// the colour and the brightness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub model: &'static CStr,
    pub scale: f32,
    pub rgb: [f32; 3],
    pub brightness: Brightness,
}

const STEP_MS: u32 = 3000;
const RED: [f32; 3] = [1.0, 0.0, 0.0];
const GREEN: [f32; 3] = [0.0, 1.0, 0.0];
const DOT: &CStr = c"weapons/reticles_069/dot";
const CROSS: &CStr = c"weapons/reticles_069/ncstar_cross";
const STEPS: [(&CStr, f32, [f32; 3], u8); 6] = [
    (DOT, 1.0, RED, 6),
    (DOT, 0.4, RED, 6),
    (CROSS, 1.0, RED, 6),
    (CROSS, 1.0, GREEN, 6),
    (CROSS, 1.0, GREEN, 0),
    (CROSS, 1.0, GREEN, 10),
];

pub fn drawn_for_dict(thread: &Thread, _dict: u16) -> Option<Drawn> {
    let step = (clock::milliseconds(thread) / STEP_MS) as usize % STEPS.len();
    let (model, scale, rgb, brightness) = STEPS[step];
    Some(Drawn { model, scale, rgb, brightness: Brightness::new(brightness)? })
}

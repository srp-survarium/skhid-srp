//! Patch 110: the mouse look's pitch (docs/notes/mouse-pitch.md). The hooks
//! are `mouse_pitch.toml`'s:
//!
//! - on_configs_loaded before user.cfg (`configs.start`, after patch 88): the
//!   console variables `srp_mouse_split` (on|off, off by default) and
//!   `srp_mouse_vertical` (the vertical sensitivity, 0.01 to 10 as the stock
//!   `mouse_sensitivity`, 1 by default), saved in user.cfg;
//! - the Settings' call (`options.callback`, after patches 79 and 83):
//!   `srp_mouse_split` answers `root.srp_check_option(name, label, saved)`,
//!   `srp_mouse_vertical` answers `root.srp_slider_option(name, label,
//!   saved hundredths)`, each saving its argument first (user.cfg at once);
//! - on_mouse_move's pitch step (`mouse.pitch_step`): the window's h / w is a
//!   4:3 window's 0.75 at every window, and the step's sensitivity the
//!   [`Pitch`]'s;
//! - patch 46's death camera asks the pitch's sensitivity
//!   (`mouse.death_pitch`): the same [`Pitch`].
//!
//! Off, every window turns as a 4:3 one did, bit for bit; on with the
//! vertical value equal to the option sensitivity, the same. The input
//! carries the turn: nothing a match computes.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::mouse::{DeathPitch, PitchStep};
use crate::vostok::settings::{ConsoleBool, ConsoleFloat, OptionsCall, save_user_config};
use crate::vostok::{Context, Flow, MousePitchOnOptionsCallExit, NoExit, Thread};

/// A 4:3 window's height over its width: the pitch's h / w at every window.
pub const FOUR_THREE: f32 = 0.75;

/// The yaw's step's factor of the option sensitivity (0x921948, 0.1).
const TENTH: f32 = 0.1;

/// The stock `mouse_sensitivity`'s range (its registration, 0x851EF6:
/// 0.01 to 10), which `srp_mouse_vertical` keeps too.
const LEAST: f32 = 0.01;
const MOST: f32 = 10.0;

pub const SPLIT_NAME: &CStr = c"srp_mouse_split";
pub const VERTICAL_NAME: &CStr = c"srp_mouse_vertical";
const SPLIT_LABEL: &CStr = c"st_srp_mouse_split_option";
const VERTICAL_LABEL: &CStr = c"st_srp_mouse_vertical_option";

static SPLIT: ConsoleBool = ConsoleBool::new();
static VERTICAL: ConsoleFloat = ConsoleFloat::new(1.0, LEAST, MOST);

/// A sensitivity of the Settings' sliders in hundredths, as they snap it:
/// 1 to 1000 (0.01 to 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hundredths(u16);

impl Hundredths {
    pub fn new(n: u32) -> Option<Hundredths> {
        (1..=1000).contains(&n).then_some(Hundredths(n as u16))
    }

    /// The Settings' text of one: decimal digits, nothing else.
    pub fn parse(text: &CStr) -> Option<Hundredths> {
        let digits = text.to_bytes();
        if digits.is_empty() || digits.len() > 4 || !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        Hundredths::new(digits.iter().fold(0, |n, d| n * 10 + u32::from(d - b'0')))
    }

    /// The nearest one to a sensitivity.
    pub fn nearest(sensitivity: Sensitivity) -> Hundredths {
        let n = (f64::from(sensitivity.get()) * 100.0 + 0.5) as u32;
        Hundredths(n.clamp(1, 1000) as u16)
    }

    /// Its value as the stock sliders give theirs to the exe: the slider's
    /// position n x 0.01 (a double, CLIK's Slider.lockValue), as a float.
    pub fn sensitivity(self) -> Sensitivity {
        Sensitivity((f64::from(self.0) * 0.01) as f32)
    }

    /// Its digits into `out`, with the zero.
    pub fn text(self, out: &mut [u8; 8]) -> &CStr {
        let mut n = self.0;
        let mut digits = [0u8; 4];
        let mut len = 0;
        loop {
            digits[len] = b'0' + (n % 10) as u8;
            len += 1;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        for i in 0..len {
            out[i] = digits[len - 1 - i];
        }
        out[len] = 0;
        CStr::from_bytes_until_nul(&out[..=len]).unwrap_or(c"")
    }
}

/// A sensitivity within the stock range (the console keeps the variable's
/// there: a value out of it, or not a number, is stored as the least).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sensitivity(f32);

impl Sensitivity {
    /// The variable's value; the least for one outside the range.
    pub fn of(value: f32) -> Sensitivity {
        Sensitivity(if (LEAST..=MOST).contains(&value) { value } else { LEAST })
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// The pitch's sensitivity: the option sensitivity's (`Same`, one slider for
/// both axes) or its own (`Split`, "Separate vertical sensitivity").
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pitch {
    Same,
    Split(Sensitivity),
}

impl Pitch {
    /// The two variables, read once.
    pub fn now(thread: &Thread) -> Pitch {
        if SPLIT.get(thread) { Pitch::Split(Sensitivity::of(VERTICAL.get(thread))) } else { Pitch::Same }
    }

    /// The pitch's step before the invert and 3/pi: a 4:3 window's h / w x
    /// the pitch's s, the yaw's step s (`Same`) or the handler's fov factor x
    /// the vertical sensitivity x 0.1 (`Split`), in the game's order, so a
    /// vertical value equal to the option sensitivity is `Same` bit for bit.
    pub fn step(self, horizontal: f32, fov_factor: f32) -> f32 {
        let s = match self {
            Pitch::Same => horizontal,
            Pitch::Split(vertical) => fov_factor * vertical.get() * TENTH,
        };
        FOUR_THREE * s
    }

    /// The death camera's: the option sensitivity it loaded (`Same`) or the
    /// vertical one.
    pub fn sensitivity(self, option: f32) -> f32 {
        match self {
            Pitch::Same => option,
            Pitch::Split(vertical) => vertical.get(),
        }
    }
}

pub fn on_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    SPLIT.register(ctx.thread(), SPLIT_NAME);
    VERTICAL.register(ctx.thread(), VERTICAL_NAME);
    Flow::Continue
}

/// The Settings' value of a checkbox: "1" checked, "0" not.
fn choice(text: &CStr) -> Option<bool> {
    match text.to_bytes() {
        b"1" => Some(true),
        b"0" => Some(false),
        _ => None,
    }
}

/// The Settings' two calls of this patch: each saves its argument when it is
/// one (user.cfg at once) and answers the saved value with its label.
pub fn on_options_call(ctx: &mut Context<'_>) -> Flow<MousePitchOnOptionsCallExit> {
    let Some(call) = OptionsCall::at(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let mut label = [0; 0x200];
    if call.method() == SPLIT_NAME {
        if let Some(on) = call.string_argument(0).and_then(choice) {
            SPLIT.set(thread, on);
            save_user_config(thread);
        }
        let label = call.translate(SPLIT_LABEL, &mut label);
        let saved = if SPLIT.get(thread) { c"1" } else { c"0" };
        call.answer(c"root.srp_check_option", &[SPLIT_NAME, label, saved]);
    } else if call.method() == VERTICAL_NAME {
        if let Some(value) = call.string_argument(0).and_then(Hundredths::parse) {
            VERTICAL.set(thread, value.sensitivity().get());
            save_user_config(thread);
        }
        let label = call.translate(VERTICAL_LABEL, &mut label);
        let mut text = [0; 8];
        let saved = Hundredths::nearest(Sensitivity::of(VERTICAL.get(thread))).text(&mut text);
        call.answer(c"root.srp_slider_option", &[VERTICAL_NAME, label, saved]);
    } else {
        return Flow::Continue;
    }
    Flow::Exit(MousePitchOnOptionsCallExit::Done)
}

/// on_mouse_move's pitch step: the replaced lines' XMM1 and XMM2, skipped.
pub fn on_pitch_step(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let pitch = Pitch::now(ctx.thread());
    let Some(step) = PitchStep::at(ctx) else { return Flow::Continue };
    let value = pitch.step(step.horizontal(), step.fov_factor());
    step.set(value);
    Flow::Skip
}

/// Patch 46's drag: XMM1, the option sensitivity, becomes the pitch's.
pub fn on_death_pitch(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let pitch = Pitch::now(ctx.thread());
    let death = DeathPitch::at(ctx);
    let sensitivity = pitch.sensitivity(death.sensitivity());
    death.set(sensitivity);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hundredths_are_the_sliders_digits() {
        assert_eq!(Hundredths::parse(c"1"), Hundredths::new(1));
        assert_eq!(Hundredths::parse(c"123"), Hundredths::new(123));
        assert_eq!(Hundredths::parse(c"1000"), Hundredths::new(1000));
        for bad in [c"", c"0", c"1001", c"12a", c"-5", c"1.5", c" 12", c"00000"] {
            assert_eq!(Hundredths::parse(bad), None, "{bad:?}");
        }
        for n in 1..=1000 {
            let h = Hundredths::new(n).unwrap();
            let mut out = [0; 8];
            assert_eq!(Hundredths::parse(h.text(&mut out)), Some(h));
            assert_eq!(Hundredths::nearest(h.sensitivity()), h);
        }
    }

    /// A slider snapping to 0.01 or 0.1 gives the exe n x 0.01 or m x 0.1 (a
    /// double); a float of either is the hundredths' float.
    #[test]
    fn a_hundredths_float_is_a_sliders_float() {
        for n in 1..=1000u32 {
            let ours = Hundredths::new(n).unwrap().sensitivity().get();
            assert_eq!(ours.to_bits(), ((f64::from(n) * 0.01) as f32).to_bits());
            assert_eq!(ours.to_bits(), ((f64::from(n) / 100.0) as f32).to_bits(), "{n}");
            if n % 10 == 0 {
                assert_eq!(ours.to_bits(), ((f64::from(n / 10) * 0.1) as f32).to_bits(), "{n}");
            }
        }
    }

    #[test]
    fn split_at_the_option_sensitivity_is_same_bit_for_bit() {
        for n in 1..=1000 {
            let sensitivity = Hundredths::new(n).unwrap().sensitivity().get();
            for fov_factor in [1.0f32, 0.25, 0.8333333, 0.6, 1.0 / 1.2] {
                // the game's yaw step, as 0x24026C..0x24027C makes it
                let horizontal = fov_factor * sensitivity * TENTH;
                let same = Pitch::Same.step(horizontal, fov_factor);
                let split = Pitch::Split(Sensitivity::of(sensitivity)).step(horizontal, fov_factor);
                assert_eq!(same.to_bits(), split.to_bits(), "{n} {fov_factor}");
                assert_eq!(same.to_bits(), (0.75 * horizontal).to_bits());
                assert_eq!(Pitch::Split(Sensitivity::of(sensitivity)).sensitivity(sensitivity).to_bits(),
                           Pitch::Same.sensitivity(sensitivity).to_bits());
            }
        }
    }

    #[test]
    fn split_scales_the_pitch_alone() {
        let horizontal = 1.0f32 * 1.0 * TENTH;
        let double = Pitch::Split(Sensitivity::of(2.0)).step(horizontal, 1.0);
        assert_eq!(double, 0.75 * (2.0f32 * TENTH));
        assert_eq!(Pitch::Split(Sensitivity::of(2.0)).sensitivity(1.0), 2.0);
        assert_eq!(Sensitivity::of(f32::NAN).get(), LEAST);
        assert_eq!(Sensitivity::of(20.0).get(), LEAST);
    }

    #[test]
    fn the_settings_value_is_one_or_zero() {
        assert_eq!(choice(c"1"), Some(true));
        assert_eq!(choice(c"0"), Some(false));
        assert_eq!(choice(c"on"), None);
    }
}

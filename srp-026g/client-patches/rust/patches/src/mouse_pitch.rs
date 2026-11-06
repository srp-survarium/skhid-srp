//! Patch 112: the mouse look's pitch (docs/notes/mouse-pitch.md). The hooks
//! are `mouse_pitch.toml`'s:
//!
//! - on_configs_loaded before user.cfg (`configs.start`, after patch 88): the
//!   console variables `srp_mouse_vertical_locked` (on|off, on by default:
//!   "Lock vertical sensitivity") and `srp_mouse_vertical` (the vertical
//!   sensitivity, 0.01 to 10 as the stock `mouse_sensitivity`, 1 by default),
//!   saved in user.cfg;
//! - the Settings' call (`options.callback`, after patches 79 and 83):
//!   `srp_mouse_vertical_locked` answers `root.srp_check_option(name, label,
//!   saved)`, `srp_mouse_vertical` answers `root.srp_slider_option(name,
//!   label, saved hundredths)`, each saving its argument first (user.cfg at
//!   once);
//! - on_mouse_move's pitch step (`mouse.pitch_step`): locked, the stock
//!   formula with h / w fixed at 16:9's 0.5625 (bit for bit the game at a
//!   16:9 window); unlocked, the factor that makes a count's pitch radians
//!   the yaw's at the vertical sensitivity, looking up and down ([`View`]);
//! - patch 46's death camera asks the same [`Pitch`] (`mouse.death_pitch`).
//!
//! The input carries the turn: nothing a match computes.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::mouse::{DeathPitch, PitchStep, View};
use crate::vostok::settings::{ConsoleBool, ConsoleFloat, OptionsCall, save_user_config};
use crate::vostok::{Context, Flow, MousePitchOnOptionsCallExit, NoExit, Thread};

/// 16:9's height over width: the stock pitch step's h / w at every window
/// while the vertical sensitivity is locked (1080 / 1920 is exactly it).
pub const SIXTEEN_NINE: f32 = 0.5625;

/// The pitch step's factor in place of h / w that makes a count's pitch
/// radians the yaw's: a unit is 60 degrees of view up (pi/3: the step x 3/pi
/// x pi/3), 67.5 down (60 / 67.5), a radian in the warmup's orbit (pi/3).
pub fn factor(view: View) -> f32 {
    match view {
        View::Up => 1.0,
        View::Down => 60.0 / 67.5,
        View::Orbit => core::f32::consts::FRAC_PI_3,
    }
}

/// The yaw's step's factor of the option sensitivity (0x921948, 0.1).
const TENTH: f32 = 0.1;

/// The stock `mouse_sensitivity`'s range (its registration, 0x851EF6:
/// 0.01 to 10), which `srp_mouse_vertical` keeps too.
const LEAST: f32 = 0.01;
const MOST: f32 = 10.0;

pub const LOCKED_NAME: &CStr = c"srp_mouse_vertical_locked";
pub const VERTICAL_NAME: &CStr = c"srp_mouse_vertical";
const LOCKED_LABEL: &CStr = c"st_srp_mouse_lock_option";
const VERTICAL_LABEL: &CStr = c"st_srp_mouse_vertical_option";

static LOCKED: ConsoleBool = ConsoleBool::with(true);
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
/// both axes, "Lock vertical sensitivity" ticked) or its own (`Split`, unticked).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pitch {
    Same,
    Split(Sensitivity),
}

impl Pitch {
    /// The two variables, read once.
    pub fn now(thread: &Thread) -> Pitch {
        if LOCKED.get(thread) { Pitch::Same } else { Pitch::Split(Sensitivity::of(VERTICAL.get(thread))) }
    }

    /// The pitch's step before the invert and 3/pi. `Same`: the stock 16:9
    /// one, 0.5625 x the yaw's step s, as the game's `movss` / `mulss` make it.
    /// `Split`: the view's factor x the handler's fov factor x the vertical
    /// sensitivity x 0.1, a count's pitch radians the yaw's at that value.
    pub fn step(self, view: View, horizontal: f32, fov_factor: f32) -> f32 {
        match self {
            Pitch::Same => SIXTEEN_NINE * horizontal,
            Pitch::Split(vertical) => factor(view) * (fov_factor * vertical.get() * TENTH),
        }
    }
}

pub fn on_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    LOCKED.register(ctx.thread(), LOCKED_NAME);
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
    if call.method() == LOCKED_NAME {
        if let Some(on) = call.string_argument(0).and_then(choice) {
            LOCKED.set(thread, on);
            save_user_config(thread);
        }
        let label = call.translate(LOCKED_LABEL, &mut label);
        let saved = if LOCKED.get(thread) { c"1" } else { c"0" };
        call.answer(c"root.srp_check_option", &[LOCKED_NAME, label, saved]);
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
    let value = pitch.step(step.view(), step.horizontal(), step.fov_factor());
    step.set(value);
    Flow::Skip
}

/// Patch 46's drag: locked, its stock 16:9 step; unlocked, the vertical
/// sensitivity at 1:1 (its pitch is in radians: the yaw's step).
pub fn on_death_pitch(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let pitch = Pitch::now(ctx.thread());
    let death = DeathPitch::at(ctx);
    match pitch {
        Pitch::Same => death.locked(),
        Pitch::Split(vertical) => death.split(vertical.get()),
    }
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

    /// Locked: 0.5625 x the game's yaw step, what the game's own lines give
    /// at a 16:9 window (1080 / 1920 = 0.5625 exactly), in every view.
    #[test]
    fn locked_is_the_stock_step_at_16_9() {
        for n in 1..=1000 {
            let sensitivity = Hundredths::new(n).unwrap().sensitivity().get();
            for fov_factor in [1.0f32, 0.25, 0.8333333, 0.6, 1.0 / 1.2] {
                let horizontal = fov_factor * sensitivity * TENTH;
                let stock = (1080.0f32 / 1920.0) * horizontal;
                for view in [View::Up, View::Down, View::Orbit] {
                    assert_eq!(Pitch::Same.step(view, horizontal, fov_factor).to_bits(), stock.to_bits());
                }
            }
        }
    }

    /// Unlocked: a count's pitch radians are the yaw's at the vertical value:
    /// the step x 3/pi (0xA2DAC4) is look-pitch units, a unit 60 degrees up,
    /// 67.5 down, a radian in the orbit.
    #[test]
    fn unlocked_pitch_radians_are_the_yaws() {
        let three_over_pi = 3.0 / core::f64::consts::PI;
        for (view, radians) in [(View::Up, 60f64.to_radians()), (View::Down, 67.5f64.to_radians()), (View::Orbit, 1.0)] {
            for (v, fov_factor) in [(1.0f32, 1.0f32), (0.37, 0.25), (10.0, 0.8333333)] {
                let s = fov_factor * v * TENTH;
                let pitch = f64::from(Pitch::Split(Sensitivity::of(v)).step(view, 0.123, fov_factor)) * three_over_pi * radians;
                assert!((pitch / f64::from(s) - 1.0).abs() < 1e-6, "{view:?} {v} {pitch}");
            }
        }
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

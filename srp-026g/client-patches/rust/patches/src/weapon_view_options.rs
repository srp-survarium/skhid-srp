//! Patch 218: the player's weapon view (`docs/notes/weapon-view-settings.md`).
//! The owner, 2026-11-27: some players prefer the sights as revision 73 drew
//! them; "it shouldn't be old/new, it should be more customizable". The hooks
//! are `weapon_view_options.toml`'s:
//!
//! - on_configs_loaded before user.cfg (`configs.start`): five console
//!   variables saved in user.cfg ([`Variables`]): `srp_weapon_fov` (the held
//!   gun's angle at the hip, a rifle's degrees, 40 to 100, 65 by default),
//!   `srp_weapon_fov_aimed` (fully aimed over iron sights and sights in the
//!   gun's eye plane, 20 to 65, 32.5), `srp_weapon_fov_scope` (fully aimed
//!   through a magnified scope, 16.25 to 32.5, 32.5), `srp_weapon_fov_world`
//!   (on: everything held at the world's projection, as before revision 74;
//!   off by default) and `srp_sight_distance` (centimetres the eye stands
//!   farther from the sight than 0.69d0's, negative nearer, -20 to 50, aimed
//!   over iron sights and sights in the gun's eye plane; 0 by default);
//! - the Settings' call (`options.callback`, after patches 79, 83, 112 and 66):
//!   each slider's name answers `root.srp_slider_option(name, label, saved,
//!   least, most, steps a unit)` in its steps (whole degrees at the hip,
//!   tenths over sights, hundredths in scopes, whole centimetres, a minus
//!   sign before a negative one), the checkbox's `root.srp_check_option(name,
//!   label, saved)`, each saving its argument first when it is one (user.cfg
//!   at once); `srp_weapon_view_texts` answers the window's title, the
//!   sight distance's note, the dictionary's centimetres, the preview's
//!   four views and the buttons' "Default settings" and "Close"
//!   (`root.srp_weapon_view_texts`);
//! - the hands' aim alignment (`hands_ik.aim_alignment` after patch 172, and
//!   its call's return, `hands_ik.aim_aligned`): the eye moved along its own
//!   line by the sight distance ([`crate::attachments::view`]).
//!
//! The defaults are revision 74's view bit for bit: scales of exactly 1, no
//! eye moved. The world's projection, zoom and camera are never touched: the
//! weapon's own projection is the foreground pass's, the eye is where the
//! drawn gun stands against the camera; the shot, the recoil, the spread
//! and what a world holds are the game's. Nothing a match computes.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::log::{Piece, line};
use crate::vostok::settings::{ConsoleBool, ConsoleFloat, OptionsCall, SliderSteps, save_user_config};
use crate::vostok::{Context, Flow, Lock, NoExit, Thread, WeaponViewOptionsOnOptionsCallExit};

/// 0.69d0's first-person angles (`update_camera` 0x2002C2D70 of its exe):
/// 65 degrees at the hip, times a rifle's aimed factor 0.5 fully aimed
/// (84 of the 103 guns; `view_factors_069.tsv`).
pub const HIP_069: f32 = 65.0;
pub const AIMED_069: f32 = 32.5;

/// A slider of this patch: its console variable and Settings name, its
/// label's key, its range, and its steps to a unit (the movie's snap: each
/// default and end a whole number of them, four digits at most).
struct Slider {
    name: &'static CStr,
    label: &'static CStr,
    value: &'static ConsoleFloat,
    least: f32,
    most: f32,
    per_unit: u16,
}

static HIP: ConsoleFloat = ConsoleFloat::new(HIP_069, 40.0, 100.0);
static SIGHTS: ConsoleFloat = ConsoleFloat::new(AIMED_069, 20.0, 65.0);
/// Never wider than 0.69d0's: the magnified scopes' aimed models cover a
/// 16:9 screen only below 7.7 (the PU) to 27.3 degrees (the old PSO-1)
/// from their eyes, so a wider angle shows more of the world past the
/// housing (`weapon-view-settings.md`, "The scopes' surround").
static SCOPES: ConsoleFloat = ConsoleFloat::new(AIMED_069, 16.25, AIMED_069);
static WORLD: ConsoleBool = ConsoleBool::new();
/// The values the log said last.
static SAID: Lock<Option<ViewOptions>> = Lock::new(None);
/// The sight distance's range in centimetres: 20 nearer (the clamps stop a
/// sight sooner where it is near already), 50 farther (a pistol's revision 73
/// eye was 40 cm nearer than 0.69d0's; the arms' reach beyond is unseen).
const NEARER: f32 = -20.0;
const FARTHER: f32 = 50.0;
static DISTANCE: ConsoleFloat = ConsoleFloat::new(0.0, NEARER, FARTHER);

pub const HIP_NAME: &CStr = c"srp_weapon_fov";
pub const SIGHTS_NAME: &CStr = c"srp_weapon_fov_aimed";
pub const SCOPES_NAME: &CStr = c"srp_weapon_fov_scope";
pub const WORLD_NAME: &CStr = c"srp_weapon_fov_world";
pub const DISTANCE_NAME: &CStr = c"srp_sight_distance";
const WORLD_LABEL: &CStr = c"st_srp_weapon_fov_world_option";
const HEADING: &CStr = c"st_srp_weapon_view_heading";
const DISTANCE_NOTE: &CStr = c"st_srp_sight_distance_note";
const CENTIMETRES: &CStr = c"st_srp_unit_cm";
/// The preview's four views (SrpWeaponPreview), then the window's two buttons of the game's own texts:
/// "Default settings" (the Controls tab's) and "Close".
const PREVIEW: [&CStr; 6] = [c"st_srp_weapon_view_hip", c"st_srp_weapon_view_iron", c"st_srp_weapon_view_red_dot",
                             c"st_srp_weapon_view_scope", c"st_options_default_controls", c"st_close"];

static SLIDERS: [Slider; 4] = [
    Slider { name: HIP_NAME, label: c"st_srp_weapon_fov_option", value: &HIP, least: 40.0, most: 100.0, per_unit: 1 },
    Slider { name: SIGHTS_NAME, label: c"st_srp_weapon_fov_aimed_option", value: &SIGHTS, least: 20.0, most: 65.0, per_unit: 10 },
    Slider {
        name: SCOPES_NAME, label: c"st_srp_weapon_fov_scope_option", value: &SCOPES, least: 16.25, most: AIMED_069, per_unit: 100,
    },
    Slider { name: DISTANCE_NAME, label: c"st_srp_sight_distance_option", value: &DISTANCE, least: NEARER, most: FARTHER, per_unit: 1 },
];

/// A held thing's angle over 0.69d0's: 1 is 0.69d0's own, exactly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scale(f32);

/// A float in thousandths, for the log (0 for what is no number or below 0).
pub fn thousandths(value: f32) -> u32 {
    if value.is_nan() || value <= 0.0 { 0 } else { (f64::from(value) * 1000.0 + 0.5) as u32 }
}

impl Scale {
    pub const ONE: Scale = Scale(1.0);

    /// `degrees` (a rifle's) over 0.69d0's `of`; 0.69d0's own where the
    /// value is not a number within `least..=most` (the console's setter
    /// keeps it there; user.cfg's text is not trusted twice).
    fn of(degrees: f32, of: f32, least: f32, most: f32) -> Scale {
        if (least..=most).contains(&degrees) { Scale(degrees / of) } else { Scale::ONE }
    }

    pub fn get(self) -> f32 {
        self.0
    }

    /// The way to `other` at `t` (0 this, 1 `other`): a second sight's switch.
    pub fn toward(self, other: Scale, t: f32) -> Scale {
        if t <= 0.0 || !t.is_finite() {
            return self;
        }
        if t >= 1.0 {
            return other;
        }
        Scale(self.0 + (other.0 - self.0) * t)
    }
}

/// How everything held is projected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    /// At the world's projection, its zoom and the player's `uro_fov`, as
    /// before revision 74 (patch 161's lens off).
    World,
    /// At a projection of its own, as 0.69d0's: each angle a scale of
    /// 0.69d0's, the gun's own factors kept.
    Own { hip: Scale, sights: Scale, scopes: Scale },
}

/// How many metres farther from the sight the eye stands than 0.69d0's
/// (negative: nearer), before the clamps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SightDistance(f32);

impl SightDistance {
    pub const NONE: SightDistance = SightDistance(0.0);

    fn of(centimetres: f32) -> SightDistance {
        if (NEARER..=FARTHER).contains(&centimetres) { SightDistance(centimetres / 100.0) } else { SightDistance::NONE }
    }

    pub fn farther(self) -> f32 {
        self.0
    }
}

/// The player's weapon view, read once a frame (the console's values are
/// the game thread's: the camera reads them, the draw takes what it kept).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewOptions {
    pub projection: Projection,
    pub distance: SightDistance,
}

impl ViewOptions {
    /// Revision 74's.
    pub const DEFAULT: ViewOptions = ViewOptions {
        projection: Projection::Own { hip: Scale::ONE, sights: Scale::ONE, scopes: Scale::ONE },
        distance: SightDistance::NONE,
    };

    /// The five values as the player has them: degrees, on or off, centimetres.
    pub fn of(hip: f32, sights: f32, scopes: f32, world: bool, distance: f32) -> ViewOptions {
        let projection = if world {
            Projection::World
        } else {
            Projection::Own {
                hip: Scale::of(hip, HIP_069, 40.0, 100.0),
                sights: Scale::of(sights, AIMED_069, 20.0, 65.0),
                scopes: Scale::of(scopes, AIMED_069, 16.25, AIMED_069),
            }
        };
        ViewOptions { projection, distance: SightDistance::of(distance) }
    }

    pub fn now(thread: &Thread) -> ViewOptions {
        let options = ViewOptions::of(HIP.get(thread), SIGHTS.get(thread), SCOPES.get(thread), WORLD.get(thread),
                                      DISTANCE.get(thread));
        options.say(thread);
        options
    }

    /// A line when the values first read differ from the ones said last (the first read, an Apply, a
    /// console line): what each variable holds and what the lens makes of it.
    fn say(self, thread: &Thread) {
        let changed = SAID.with(thread, |said| {
            let changed = *said != Some(self);
            *said = Some(self);
            changed
        });
        if !changed {
            return;
        }
        let scale = |name: &CStr| SLIDERS.iter().find(|slider| slider.name == name).map_or(0, |s| thousandths(s.value.get(thread)));
        let (projection, [hip, sights, scopes]) = match self.projection {
            Projection::World => ("the world's projection", [0, 0, 0]),
            Projection::Own { hip, sights, scopes } => ("its own", [hip, sights, scopes].map(|s| thousandths(s.get()))),
        };
        line(&[Piece::Text("weapon view: srp_weapon_fov "), Piece::Dec(scale(HIP_NAME)),
               Piece::Text(", srp_weapon_fov_aimed "), Piece::Dec(scale(SIGHTS_NAME)),
               Piece::Text(", srp_weapon_fov_scope "), Piece::Dec(scale(SCOPES_NAME)),
               Piece::Text(" (thousandths); srp_sight_distance "), Piece::Text(sign(DISTANCE.get(thread))),
               Piece::Dec(thousandths(DISTANCE.get(thread).abs())),
               Piece::Text(" (thousandths of a cm); srp_weapon_fov_world "), Piece::Text(if WORLD.get(thread) { "on" } else { "off" }),
               Piece::Text(": everything held at "), Piece::Text(projection),
               Piece::Text(", the scales of 0.69d0's angles (thousandths) hip "), Piece::Dec(hip),
               Piece::Text(", sights "), Piece::Dec(sights), Piece::Text(", scopes "), Piece::Dec(scopes),
               Piece::Text(", the eye "), Piece::Dec(thousandths(self.distance.farther().abs() * 10.0)),
               Piece::Text(if self.distance.farther() < 0.0 { " tenths of a mm nearer" } else { " tenths of a mm farther" })]);
    }
}

pub fn on_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    for slider in &SLIDERS {
        slider.value.register(thread, slider.name);
    }
    WORLD.register(thread, WORLD_NAME);
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

/// "-" before a negative value in the log, "" else.
fn sign(value: f32) -> &'static str {
    if value < 0.0 { "-" } else { "" }
}

/// A slider's value in its steps, as the movie and the exe tell it each other:
/// one to four decimal digits, a minus sign before a negative one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Steps(i32);

impl Steps {
    fn parse(text: &CStr) -> Option<Steps> {
        let (negative, digits) = match text.to_bytes() {
            [b'-', rest @ ..] => (true, rest),
            all => (false, all),
        };
        if digits.is_empty() || digits.len() > 4 || !digits.iter().all(u8::is_ascii_digit) {
            return None;
        }
        let n = digits.iter().fold(0, |n, d| n * 10 + i32::from(d - b'0'));
        Some(Steps(if negative { -n } else { n }))
    }

    /// Its text into `out`, with the zero.
    fn text(self, out: &mut [u8; 8]) -> &CStr {
        let mut digits = [0; 8];
        let magnitude = SliderSteps::new(self.0.unsigned_abs().min(9999) as u16);
        let bytes = magnitude.text(&mut digits).to_bytes();
        let start = usize::from(self.0 < 0);
        if start == 1 {
            out[0] = b'-';
        }
        out[start..start + bytes.len()].copy_from_slice(bytes);
        out[start + bytes.len()] = 0;
        CStr::from_bytes_until_nul(&out[..=start + bytes.len()]).unwrap_or(c"")
    }
}

impl Slider {
    /// The movie's steps as the value: within the range.
    fn parse(&self, text: &CStr) -> Option<f32> {
        let value = Steps::parse(text)?.0 as f32 / f32::from(self.per_unit);
        (self.least..=self.most).contains(&value).then_some(value)
    }

    /// A value in steps.
    fn steps(&self, value: f32) -> Steps {
        let x = f64::from(value) * f64::from(self.per_unit);
        // to the nearest (no `round` in no_std): half away from 0
        let n = if x < 0.0 { (x - 0.5) as i32 } else { (x + 0.5) as i32 };
        Steps(n.clamp(-9999, 9999))
    }
}

/// The Settings' calls of this patch: each saves its argument when it is
/// one (user.cfg at once) and answers the saved value with its label (a
/// slider its range too).
pub fn on_options_call(ctx: &mut Context<'_>) -> Flow<WeaponViewOptionsOnOptionsCallExit> {
    let Some(call) = OptionsCall::at(ctx) else { return Flow::Continue };
    let thread = ctx.thread();
    let method = call.method();
    let mut label = [0; 0x200];
    if method == HEADING_CALL {
        let (mut note, mut unit) = ([0; 0x200], [0; 0x200]);
        let mut rooms = [[0; 0x200]; PREVIEW.len()];
        let heading = call.translate(HEADING, &mut label);
        let note = call.translate(DISTANCE_NOTE, &mut note);
        let unit = call.translate(CENTIMETRES, &mut unit);
        let [hip, iron, red_dot, scope, default, close] = &mut rooms;
        let texts = [call.translate(PREVIEW[0], hip), call.translate(PREVIEW[1], iron), call.translate(PREVIEW[2], red_dot),
                     call.translate(PREVIEW[3], scope), call.translate(PREVIEW[4], default), call.translate(PREVIEW[5], close)];
        call.answer(c"root.srp_weapon_view_texts",
                    &[heading, note, unit, texts[0], texts[1], texts[2], texts[3], texts[4], texts[5]]);
    } else if method == WORLD_NAME {
        if let Some(on) = call.string_argument(0).and_then(choice) {
            WORLD.set(thread, on);
            save_user_config(thread);
        }
        let label = call.translate(WORLD_LABEL, &mut label);
        let saved = if WORLD.get(thread) { c"1" } else { c"0" };
        call.answer(c"root.srp_check_option", &[WORLD_NAME, label, saved]);
    } else if let Some(slider) = SLIDERS.iter().find(|slider| slider.name == method) {
        if let Some(value) = call.string_argument(0).and_then(|text| slider.parse(text)) {
            slider.value.set(thread, value);
            save_user_config(thread);
        }
        let label = call.translate(slider.label, &mut label);
        let (mut saved, mut least, mut most, mut unit) = ([0; 8], [0; 8], [0; 8], [0; 8]);
        let values = [slider.name, label, slider.steps(slider.value.get(thread)).text(&mut saved),
                      slider.steps(slider.least).text(&mut least), slider.steps(slider.most).text(&mut most),
                      Steps(i32::from(slider.per_unit)).text(&mut unit)];
        call.answer(c"root.srp_slider_option", &values);
    } else {
        return Flow::Continue;
    }
    Flow::Exit(WeaponViewOptionsOnOptionsCallExit::Done)
}

/// The movie's ask for the group's texts.
pub const HEADING_CALL: &CStr = c"srp_weapon_view_texts";

/// `hands_ik.aim_alignment`, after patch 172: the eye's move this frame
/// taken for the call that follows.
pub fn on_eye_shift(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::attachments::view::eye_shift(ctx);
    Flow::Continue
}

/// `hands_ik.aim_aligned`: the alignment the call made, the eye moved.
pub fn on_eye_aligned(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::attachments::view::eye_aligned(ctx);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_0_69d0_s_view_exactly() {
        assert_eq!(ViewOptions::of(65.0, 32.5, 32.5, false, 0.0), ViewOptions::DEFAULT);
        let Projection::Own { hip, sights, scopes } = ViewOptions::DEFAULT.projection else { panic!() };
        assert!([hip, sights, scopes].iter().all(|scale| scale.get().to_bits() == 1.0_f32.to_bits()));
        assert_eq!(ViewOptions::DEFAULT.distance.farther().to_bits(), 0.0_f32.to_bits());
    }

    #[test]
    fn the_values_are_scales_of_0_69d0_s_angles() {
        let options = ViewOptions::of(80.0, 52.0, 16.25, false, -12.0);
        let Projection::Own { hip, sights, scopes } = options.projection else { panic!() };
        assert!((hip.get() - 80.0 / 65.0).abs() < 1e-6 && (sights.get() - 1.6).abs() < 1e-6 && (scopes.get() - 0.5).abs() < 1e-6);
        assert!((options.distance.farther() + 0.12).abs() < 1e-6);
        assert_eq!(ViewOptions::of(80.0, 52.0, 16.25, true, 50.0).projection, Projection::World);
        // outside the ranges, or not numbers: 0.69d0's own
        let odd = ViewOptions::of(f32::NAN, 70.0, 40.0, false, -21.0);
        assert_eq!(odd, ViewOptions::DEFAULT);
        assert_eq!(Scale(1.0).toward(Scale(0.5), 0.5), Scale(0.75));
        assert_eq!(Scale(1.0).toward(Scale(0.5), 0.0), Scale(1.0));
        assert_eq!(Scale(1.0).toward(Scale(0.5), 1.0), Scale(0.5));
    }

    #[test]
    fn the_slider_text_is_its_steps_within_the_range() {
        let sights = &SLIDERS[1];
        assert_eq!(sights.parse(c"325"), Some(32.5));
        assert_eq!(sights.parse(c"650"), Some(65.0));
        assert_eq!(sights.parse(c"199"), None);
        assert_eq!(sights.parse(c"651"), None);
        assert_eq!(sights.parse(c"32.5"), None);
        assert_eq!(sights.steps(32.5), Steps(325));
        assert_eq!(SLIDERS[2].steps(16.25), Steps(1625));
        let distance = &SLIDERS[3];
        assert_eq!(distance.parse(c"0"), Some(0.0));
        assert_eq!(distance.parse(c"-20"), Some(-20.0));
        assert_eq!(distance.parse(c"50"), Some(50.0));
        assert_eq!(distance.parse(c"-21"), None);
        assert_eq!(distance.parse(c"51"), None);
        assert_eq!(distance.parse(c"--5"), None);
        assert_eq!(distance.parse(c"-"), None);
        assert_eq!(SLIDERS[0].parse(c"100"), Some(100.0));
        let mut out = [0; 8];
        assert_eq!(Steps(-20).text(&mut out), c"-20");
        assert_eq!(Steps(0).text(&mut out), c"0");
        assert_eq!(Steps(3250).text(&mut out), c"3250");
        // every default and end a whole number of steps of four digits at most
        for (slider, default) in SLIDERS.iter().zip([HIP_069, AIMED_069, AIMED_069, 0.0]) {
            for value in [slider.least, slider.most, default] {
                let steps = slider.steps(value).0 as f32;
                assert_eq!(steps / f32::from(slider.per_unit), value);
            }
        }
    }
}

//! The view's pitch: the client's unit and the elevation in radians the bots reckon their geometry in.
//!
//! The game keeps the view's pitch (`m_look_pitch`, a world saves it; an input's pitch is a change of it) as a
//! number in `[-1, 1]` and the eye and the bullet follow the animated "Item" bone, which the player's look clip poses
//! by that number: `base_player::look_time_factor_calculator` 0x2FBF90 gives the clip the time `(look_pitch + 1) / 2`
//! of its length (`weapon_core::look_pitch` 0x2E0F60, the vtable slot +8 at 0xA0417C: `clamp(m_look_pitch + the kick's y,
//! -1, 1)`), and the Item bone's forward axis in the stock clips rises -67.24, -33.52, +0.24, +30.25, +60.24 degrees
//! over the time 0, 0.25, 0.5, 0.75, 1 (`player_bots_weapons.md`, "The view's pitch unit"). PROVEN. So a unit is **not** a
//! radian: it is 60.0 degrees up and 67.48 degrees down (straight in each half, a kink at 0; the 0.24 degrees the
//! bone is up at pitch 0 is left out), 127.48 degrees over the whole sweep.
//!
//! The bots reckon in elevations, radians above the horizontal, as `atan2` gives them. The unit is the game's side of
//! the boundary: a world's pitch, an input's pitch turn, the kick. [`Pitch`] is that unit as a type; the two scales
//! ([`PitchScale`]) are the map (the client's) and the identity (the bots as they were, a unit taken for a radian).

use serde::{Deserialize, Serialize};

/// Degrees of elevation per unit of pitch above the horizon and below it.
const UP_DEG_PER_UNIT: f32 = 60.0;
const DOWN_DEG_PER_UNIT: f32 = 67.48;

/// How the view's pitch unit is taken to relate to the elevation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PitchScale {
    /// A unit is a radian (the bots as they were: wrong, 57.30 degrees a unit both ways).
    Radian,
    /// The client's: 60.0 degrees a unit up, 67.48 down.
    #[default]
    Client,
}

/// The view's pitch as the game keeps it: units, `[-1, 1]`, positive up.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Pitch(f32);

impl Pitch {
    /// The unit `units` (a value the game cannot hold is clamped; not a number is level).
    pub fn new(units: f32) -> Self {
        Self(if units.is_finite() { units.clamp(-1.0, 1.0) } else { 0.0 })
    }

    /// The view of base pitch `base` and the weapon's kick `kick` (`clamp(base + kick)`, 0x2E0F60).
    pub fn view(base: f32, kick: f32) -> Self {
        Self::new(base + kick)
    }

    pub fn units(self) -> f32 {
        self.0
    }

    /// The elevation of this pitch, radians.
    pub fn elevation(self, scale: PitchScale) -> f32 {
        match scale {
            PitchScale::Radian => self.0,
            PitchScale::Client => {
                let degrees = if self.0 >= 0.0 { UP_DEG_PER_UNIT } else { DOWN_DEG_PER_UNIT };
                (self.0 * degrees).to_radians()
            }
        }
    }

    /// The pitch that has elevation `radians` (the nearest it can have: the sweep's ends).
    pub fn of_elevation(radians: f32, scale: PitchScale) -> Self {
        match scale {
            PitchScale::Radian => Self::new(radians),
            PitchScale::Client => {
                let degrees = if radians >= 0.0 { UP_DEG_PER_UNIT } else { DOWN_DEG_PER_UNIT };
                Self::new(radians.to_degrees() / degrees)
            }
        }
    }

    /// The pitch turn an input carries to turn the view that is at this pitch by `elevation` radians of elevation (to the
    /// sweep's end at most): the difference of the two pitches, in units (under [`PitchScale::Radian`] the turn as it is).
    pub fn turn_for(self, scale: PitchScale, elevation: f32) -> f32 {
        match scale {
            // As the bots sent it before: the turn as it is, not held to the sweep.
            PitchScale::Radian => elevation,
            PitchScale::Client if elevation == 0.0 => 0.0,
            PitchScale::Client => Self::of_elevation(self.elevation(scale) + elevation, scale).0 - self.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn degrees(pitch: f32) -> f32 {
        Pitch::new(pitch).elevation(PitchScale::Client).to_degrees()
    }

    #[test]
    fn the_clients_sweep_is_the_item_bones() {
        // The stock look clip's Item bone, standing, less the 0.24 degrees at level: -67.48 / +60.00 at the ends.
        assert!((degrees(1.0) - 60.0).abs() < 1e-3 && (degrees(-1.0) + 67.48).abs() < 1e-3);
        assert!((degrees(0.5) - 30.0).abs() < 0.05 && (degrees(-0.5) + 33.74).abs() < 0.05);
        assert_eq!(degrees(0.0), 0.0);
        assert!((degrees(1.0) - degrees(-1.0) - 127.48).abs() < 1e-3);
    }

    #[test]
    fn the_radian_scale_is_the_identity_the_bots_had() {
        for p in [-1.0, -0.3, 0.0, 0.7, 1.0] {
            assert_eq!(Pitch::new(p).elevation(PitchScale::Radian), p);
            assert_eq!(Pitch::of_elevation(p, PitchScale::Radian).units(), p);
        }
        // A turn is the elevation turn, whatever the pitch: the old behaviour.
        assert!((Pitch::new(0.2).turn_for(PitchScale::Radian, -0.1) + 0.1).abs() < 1e-6);
    }

    #[test]
    fn an_elevation_and_its_pitch_invert_each_other() {
        for p in [-1.0, -0.77, -0.01, 0.0, 0.01, 0.4, 1.0] {
            let there = Pitch::new(p).elevation(PitchScale::Client);
            let back = Pitch::of_elevation(there, PitchScale::Client).units();
            assert!((back - p).abs() < 1e-5, "{p}: {back}");
        }
    }

    #[test]
    fn a_turn_is_the_difference_of_two_elevations_across_the_kink() {
        // 15 degrees below the level: the bots' old turn of -15 degrees as a radian left it 17.7 degrees down.
        let down = Pitch::new(0.0).turn_for(PitchScale::Client, (-15.0f32).to_radians());
        assert!((down * 67.48 + 15.0).abs() < 1e-3, "{down}");
        // From 10 degrees above to 5 below: 10 degrees on the 60 side, 5 on the 67.48 side.
        let start = Pitch::of_elevation(10.0f32.to_radians(), PitchScale::Client);
        let across = start.turn_for(PitchScale::Client, (-15.0f32).to_radians());
        assert!((across - (-10.0 / 60.0 - 5.0 / 67.48)).abs() < 1e-5, "{across}");
        // Past the sweep's end it stops there.
        let top = Pitch::new(0.9).turn_for(PitchScale::Client, 1.0);
        assert!((top - 0.1).abs() < 1e-6, "{top}");
    }

    #[test]
    fn what_the_unit_is_not_a_radian_is_the_old_error() {
        // The bug in numbers: a target 15 degrees below the eye, the bot sends -15 degrees as a radian turn.
        let sent = (-15.0f32).to_radians();
        let got = Pitch::new(sent).elevation(PitchScale::Client).to_degrees();
        assert!((got + 17.67).abs() < 0.05, "{got}");
    }
}

//! The client's leg-damage gate on movement (fx-review A325f, PROVEN in the exe; fx-smoke's data).
//!
//! `damage_model` +0x388 counts the body parts holding a `leg_damage` affect (type 4; +- at 0x2E537E / 0x2E5387, recounted on
//! load 0x2E5990); an affect ends at its hit's time + `affect_duration_sec` x 1000 (`leg_damage` 8 s, 0x32AC54), or early by a
//! medkit's `remove_affects`. The movement state machine (built in 0x2FAB70):
//! - count 2: 0x2FBF00 (start the standing walk) is false and 0x2FBDF0 (to on-site) true from 5 moving states: no walk, sprint
//!   or jump at any keys until the earlier affect ends (walk-key worlds still 95.4 % at count 2 against 2.8 %);
//! - count 1: no sprint (`can_sprint` 0x2DBF58 needs 0) and no jump (0x2FC254 needs 0); the leg's -0.2 speed term is
//!   [`super::kin::SpeedTerms::Legs`]'s.
//!
//! A world carries each part's affects with their end times (`motion::Damage::leg_damage_ends`), so the gate holds from the
//! world on to the ends, whatever keys arrive meanwhile.

use crate::v026g::drive::key;

/// What a body's legs let it do: from a world's leg-damage ends, until each ends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LegDamage {
    /// No part holds a `leg_damage` affect.
    #[default]
    Sound,
    /// One part does, until `until` (the world's ms).
    One { until: u32 },
    /// Two do: no move until `first`, then as [`Self::One`] until `second`.
    Both { first: u32, second: u32 },
}

/// What the client takes of the keys at a moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegGate {
    Free,
    /// No sprint, no jump (count 1).
    NoSprintJump,
    /// No move, sprint or jump (count 2): the body stands (it may turn and crouch).
    Stand,
}

/// The keys a moving state needs: the moves, the sprint, the jump.
const MOVES: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT;

impl LegDamage {
    /// From a world at `now`: the parts' `leg_damage` ends (an end at or before `now` is over).
    pub fn of(ends: &[u32], now: u32) -> Self {
        let mut live: Vec<u32> = ends.iter().copied().filter(|&end| end > now).collect();
        live.sort_unstable();
        match live.as_slice() {
            [] => Self::Sound,
            [until] => Self::One { until: *until },
            [first, .., second] => Self::Both { first: *first, second: *second },
        }
    }

    /// The gate at `time` (ms).
    pub fn at(self, time: u32) -> LegGate {
        match self {
            Self::Both { first, .. } if time < first => LegGate::Stand,
            Self::Both { second: until, .. } | Self::One { until } if time < until => LegGate::NoSprintJump,
            _ => LegGate::Free,
        }
    }
}

impl LegGate {
    /// The keys the client acts on: a refused key is as if not held.
    pub fn keys(self, keys: u32) -> u32 {
        match self {
            Self::Free => keys,
            Self::NoSprintJump => keys & !(key::SPRINT | key::JUMP),
            Self::Stand => keys & !(MOVES | key::SPRINT | key::JUMP),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_legs_stand_until_the_first_end_then_one_leg_until_the_second() {
        let legs = LegDamage::of(&[18_000, 16_000], 10_000);
        assert_eq!(legs, LegDamage::Both { first: 16_000, second: 18_000 });
        let run = key::FORWARD | key::SPRINT | key::JUMP | key::CROUCH;
        assert_eq!(legs.at(15_950).keys(run), key::CROUCH, "no move at count 2: the crouch stays");
        assert_eq!(legs.at(16_000).keys(run), key::FORWARD | key::CROUCH, "one leg: walk, no sprint or jump");
        assert_eq!(legs.at(18_000).keys(run), run);
    }

    #[test]
    fn ends_already_over_count_for_nothing() {
        assert_eq!(LegDamage::of(&[9_000], 10_000), LegDamage::Sound);
        assert_eq!(LegDamage::of(&[9_000, 12_000], 10_000), LegDamage::One { until: 12_000 });
        assert_eq!(LegDamage::of(&[], 10_000).at(10_000), LegGate::Free);
    }
}

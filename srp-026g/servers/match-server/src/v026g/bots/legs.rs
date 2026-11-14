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
    /// No move, sprint or jump (count 2): the body stands (it may turn and crouch); [F42] crouched, it walks with the crouch key
    /// and a move key held.
    Stand,
}

/// The keys a moving state needs: the moves, the sprint, the jump.
const MOVES: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT;

/// [F12] A leg wound's end as the client's tick keeps it: the later of its saved end and its leg's last damaging hit + the
/// body's regeneration delay `delay_ms` (0x32A551; [`regeneration_delay_ms`]); a delay of 0 holds nothing past the end. A
/// FORECAST between worlds: the world itself lists a wound for as long as the client holds it ([`end_listed`]).
pub fn end_held(wound: survarium::v026g::match_msgs::motion::LegWound, delay_ms: u32) -> u32 {
    match wound.last_hit {
        Some(hit) if delay_ms > 0 => wound.end.max(hit.saturating_add(delay_ms)),
        _ => wound.end,
    }
}

/// [F12] The end of a wound a world of `now` LISTS: every world lists a wound while the client holds it (the expiry 0x32A600
/// takes it off the part's list), so a listed wound is on at the world's time whatever its saved end (exactness: 21 bot leg
/// wounds were listed past their saved end). Its forecast end ([`end_held`]) while that is still ahead; past it, on until a
/// world no longer lists it (`u32::MAX`: the next world's word replaces it).
pub fn end_listed(wound: survarium::v026g::match_msgs::motion::LegWound, delay_ms: u32, now: u32) -> u32 {
    let held = end_held(wound, delay_ms);
    if held > now { held } else { u32::MAX }
}

/// [F12] A leg's regeneration delay for a body whose Metabolism starts it `sooner_ms` sooner (500 ms a rank:
/// `stock::DrugFactors::regeneration_sooner_ms`): the game data's leg `regeneration_timeout` less it, not under 0 (PROVEN,
/// exactness 18:23Z; the Regeneration booster's 2 s is not taken: the server does not see a booster under way).
pub fn regeneration_delay_ms(sooner_ms: u32) -> u32 {
    let timeout_s = survarium::v026g::damage::human().part("left_leg").map_or(0.0, |part| part.regeneration_timeout);
    ((timeout_s * 1000.0).round() as u32).saturating_sub(sooner_ms)
}

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
            // [F42] The client forces the crouch; with the crouch key held a move key moves the crouched body (exactness L30, PROVEN:
            // 0x2FBDF0 / 0x2FBE50, 16 of 16 pairs of worlds moved).
            Self::Stand if keys & key::CROUCH != 0 => keys & !(key::SPRINT | key::JUMP),
            Self::Stand => keys & !(MOVES | key::SPRINT | key::JUMP),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [F42] With both legs broken the client takes no move without the crouch key; with it held, a move key moves the crouched
    /// body (exactness L30). No sprint or jump either way.
    #[test]
    fn both_legs_broken_move_only_crouched() {
        let moving = key::FORWARD | key::SPRINT | key::JUMP;
        assert_eq!(LegGate::Stand.keys(moving), 0);
        assert_eq!(LegGate::Stand.keys(moving | key::CROUCH), key::FORWARD | key::CROUCH);
    }

    /// [F12] A wound is held the leg's regeneration delay past its last damaging hit: 8 s less 500 ms a Metabolism rank (a
    /// bot's 5 ranks: 5.5 s), the saved end if later; none past the end with no hit or a delay of 0.
    #[test]
    fn a_wound_is_held_the_legs_regeneration_delay_past_its_last_hit() {
        use survarium::v026g::match_msgs::motion::LegWound;
        assert_eq!(regeneration_delay_ms(0), 8_000);
        assert_eq!(regeneration_delay_ms(2_500), 5_500);
        assert_eq!(regeneration_delay_ms(9_000), 0);
        let wound = LegWound { end: 110_000, last_hit: Some(105_000) };
        assert_eq!(end_held(wound, 5_500), 110_500);
        assert_eq!(end_held(wound, 8_000), 113_000);
        assert_eq!(end_held(LegWound { last_hit: Some(100_000), ..wound }, 5_500), 110_000, "the saved end is later");
        assert_eq!(end_held(LegWound { last_hit: None, ..wound }, 5_500), 110_000);
        assert_eq!(end_held(wound, 0), 110_000);
        // A listed wound past its forecast end is on until a world drops it.
        assert_eq!(end_listed(wound, 5_500, 109_000), 110_500);
        assert_eq!(end_listed(wound, 5_500, 112_000), u32::MAX);
        assert_eq!(LegDamage::of(&[end_listed(wound, 5_500, 112_000)], 112_000).at(115_000), LegGate::NoSprintJump);
    }

    #[test]
    fn two_legs_stand_until_the_first_end_then_one_leg_until_the_second() {
        let legs = LegDamage::of(&[18_000, 16_000], 10_000);
        assert_eq!(legs, LegDamage::Both { first: 16_000, second: 18_000 });
        let run = key::FORWARD | key::SPRINT | key::JUMP | key::CROUCH;
        assert_eq!(legs.at(15_950).keys(run & !key::CROUCH), 0, "no move at count 2 standing");
        assert_eq!(legs.at(15_950).keys(run), key::FORWARD | key::CROUCH, "[F42] crouched, it walks: no sprint or jump");
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

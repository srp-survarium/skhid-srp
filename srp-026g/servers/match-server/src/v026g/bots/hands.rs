//! [Gap 7] The client's broken-hands gate on a bot's own weapon (exactness 2026-11-14, PROVEN in the exe; counted on 62
//! hand rows of the recordings).
//!
//! - `damage_model` +0x389 counts the body parts holding a `hand_damage` affect (type 3): `on_broken_limb_affect`
//!   0x2E5370 adds 1 at a hand's affect start and takes 1 at its end (+0x388 is the legs' count, `legs.rs`).
//! - With the count at 2 (both hands):
//!   - no aim (0x2FC2DE) and no grenade throw (0x2E40EC), whatever the weapon;
//!   - a weapon whose config has `parameters.double_handed` neither fires nor reloads: `weapon_core::could_be_used`
//!     0x2E3800 refuses it when +0x389 is 2 and the weapon's +0x5E9 is set (`load_config_parameters` 0x30ABA4..0x30ABC0;
//!     absent = false: `item_params::ItemParams::double_handed`). In the rev-56 data every assault rifle, SMG, bolt
//!     rifle and shotgun is two-handed but the TOZ-66, and of the pistols only the AEK-919K.
//! - One broken hand gates the battery's use only (0x300739; the battery's carrier is nav-team's).
//! - A hand's wound ends as a leg's (`damage_model.md` "Wounds and what they gate"; [`super::legs::end_held`]): at the
//!   later of its saved end and its part's last damaging hit + the part's regeneration delay (8 s for every pool, less
//!   Metabolism's 500 ms a rank), and a world lists it for as long as the client holds it.

/// [`super::legs::end_held`] / [`super::legs::end_listed`] for a hand: a wound's end as the client's tick keeps it, from a
/// world of `now` that lists it with its saved `end` and its part's `last_hit` (0: none).
pub fn end_listed(end: u32, last_hit: u32, delay_ms: u32, now: u32) -> u32 {
    let held = if last_hit != 0 && delay_ms > 0 { end.max(last_hit.saturating_add(delay_ms)) } else { end };
    if held > now { held } else { u32::MAX }
}

/// A hand's regeneration delay for a body whose Metabolism starts it `sooner_ms` sooner: the game data's hand
/// `regeneration_timeout` less it, not under 0 (as [`super::legs::regeneration_delay_ms`] for a leg).
pub fn regeneration_delay_ms(sooner_ms: u32) -> u32 {
    let timeout_s = survarium::v026g::damage::human().part("right_hand").map_or(0.0, |part| part.regeneration_timeout);
    ((timeout_s * 1000.0).round() as u32).saturating_sub(sooner_ms)
}

/// What a body's hands let its weapon do, from a world's hand wounds: until when both are broken.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Hands {
    /// Fewer than two parts hold a `hand_damage` affect.
    #[default]
    Free,
    /// Both do, until the earlier ends (the world's ms).
    Broken { until: u32 },
}

impl Hands {
    /// From a world at `now`: the parts' `hand_damage` ends ([`end_listed`]; an end at or before `now` is over).
    pub fn of(ends: &[u32], now: u32) -> Self {
        let mut live: Vec<u32> = ends.iter().copied().filter(|&end| end > now).collect();
        live.sort_unstable();
        match live.as_slice() {
            [first, _, ..] => Self::Broken { until: *first },
            _ => Self::Free,
        }
    }

    /// Both hands broken at `time`.
    pub fn broken(self, time: u32) -> bool {
        matches!(self, Self::Broken { until } if time < until)
    }
}

/// Whether the weapon item `item` (a weapon config's name) is two-handed: its config's `parameters.double_handed`.
pub fn double_handed(item: &str) -> bool {
    survarium::v026g::dictionary::Dictionary::shared()
        .item_named(item)
        .and_then(|entry| survarium::v026g::item_params::item_params(entry.dict_id))
        .is_some_and(|params| params.double_handed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both hands broken until the earlier wound ends; one, or none, is free; an end at or before the world is over.
    #[test]
    fn both_hands_are_broken_until_the_earlier_end() {
        assert_eq!(Hands::of(&[], 1_000), Hands::Free);
        assert_eq!(Hands::of(&[5_000], 1_000), Hands::Free);
        assert_eq!(Hands::of(&[5_000, 3_000], 1_000), Hands::Broken { until: 3_000 });
        assert_eq!(Hands::of(&[5_000, 900], 1_000), Hands::Free);
        let both = Hands::of(&[5_000, 3_000], 1_000);
        assert!(both.broken(2_999) && !both.broken(3_000));
        // Listed past its saved end: held to the last hit + the delay, then until a world drops it.
        assert_eq!(end_listed(2_000, 1_500, 8_000, 3_000), 9_500);
        assert_eq!(end_listed(2_000, 0, 8_000, 3_000), u32::MAX);
        assert_eq!(end_listed(12_000, 1_500, 8_000, 3_000), 12_000);
    }

    /// The bots' weapons are two-handed by their configs; a one-handed pistol is not.
    #[test]
    fn the_rifles_are_two_handed_and_the_tt_is_not() {
        assert!(double_handed("ak_74u"));
        assert!(!double_handed("tt_33"));
        assert!(!double_handed("no_such_item"));
    }
}

//! The speed level of a stand-in body (`SRP_BENCH_STAND_IN_LEVEL`, on by default, C14): the bodies move at the level their kit gives, as the
//! live server hands it (`game/driven.rs`: `bot_skills::profile_movement_level`, `Bots::set_speed_levels`), not at the nominal speed.
//!
//! The client moves a body at the nominal speed times its movement level, `1 + 0.05 (skill 208) - 0.2 (1 + overweight penalty) t`
//! with `t` the weight's cost over the capacity (PROVEN, read: `movement_speed` 0x2DBB50, the weight call 0x2DAAB5;
//! `docs/notes/player_bots_skills.md`). The stand-in ran every body at 1.0 (a carrier 0.95); the bots' kits give a mean of about 1.03
//! (tier 9-10) to 1.04 (tier 13-15), so the live bots are a few percent faster than the bench's. The bench also hands the bots'
//! tracker the same list, so that a bot's reckoning of a life's first seconds starts at the level its body has, as it does live.
//!
//! The stand-in human is no bot profile: he gets [`HUMAN_LEVEL`].

use super::*;

/// Whether the stand-in bodies move at their kit's level (`SRP_BENCH_STAND_IN_LEVEL`; on by default, as live: `off` or `0` is the nominal speed).
pub(crate) fn enabled() -> bool {
    bench_settings().is_on("SRP_BENCH_STAND_IN_LEVEL")
}

/// The level of the stand-in human's body: the median over live players of their sprint speed over the nominal 5.0 m/s, 1.02 (PROVEN
/// from the data): the saved worlds of 185 live matches of 2026-10-12..14, the game's own `speed` field of the rows with forward + sprint,
/// 841 bodies with 8 rows or more (47,823 rows): per body 1.02 median, 0.978 p10, 0.992 p25, 1.042 p75 and p90, 0.89-1.09 in all (peaks at
/// 1.00: 254 bodies and 1.04: 360), against the bots' kit levels of 1.03-1.04 (`a182-sniper/scripts/human_level_live.py`). The owner's
/// own body is no yardstick (0.97 and 1.11 in his matches). A human's own gear and skills 208 and 209 move it, which the bench does not know
/// of its stand-in; the spread (0.98-1.04) is left out, one number for every human.
pub(crate) const HUMAN_LEVEL: f32 = 1.02;

/// The level of each place's body: a bot's from its profile's first kit (none: the nominal), the human's [`HUMAN_LEVEL`].
pub(crate) fn levels(upgrades: &[Upgrades], human: Option<Place>) -> Vec<Option<f32>> {
    upgrades
        .iter()
        .enumerate()
        .map(|(place, up)| if human == Some(place) { Some(HUMAN_LEVEL) } else { up.level })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bot_moves_at_its_kits_level_and_the_human_at_his_own() {
        let ups = [Upgrades { items: 3, level: Some(0.9) }, Upgrades { items: 0, level: None }, Upgrades { items: 5, level: Some(1.04) }];
        assert_eq!(levels(&ups, Some(2)), vec![Some(0.9), None, Some(HUMAN_LEVEL)]);
        assert_eq!(levels(&ups, None), vec![Some(0.9), None, Some(1.04)]);
    }

    #[test]
    fn the_body_goes_at_the_level_it_was_given() {
        let mut game = Game::new(soak_world(), &[0, 1], 1, None, None);
        assert_eq!(game.bodies[0].speed_level, 1.0);
        game.set_speed_levels(&[Some(0.85), None]);
        assert_eq!((game.bodies[0].speed_level, game.bodies[1].speed_level), (0.85, 1.0));
    }
}

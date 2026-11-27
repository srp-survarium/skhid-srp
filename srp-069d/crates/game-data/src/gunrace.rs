//! Arms Race (gun race, mode 3, "Перегони озброєнь"): the data of `data/gunrace_069d.json`, made by
//! `crates/data-extract` from `gameplay/game_modes/arena_01.options` and `gameplay/players/ai.profiles`.
//!
//! * The matchmaking row: Olivier mall (map 13), variation 0, day, project `arena_01`, level `arena_01/default`, 8-8.
//! * `tiers[equipment level - 1][race level 0..9]` = the kit variants of each race level (`scripting.player_profiles`),
//!   except race level 7: CHOICE: the icicle kits of level 8 instead of the snowball kits,
//!   whose `grenade_set` file is not in the 0.69d resources (the exe faults building such a match; docs/notes/gun-race.md 4).
//!   The exe never reads `player_profiles` (the string is not in survarium.exe, PROVEN): like the PvE bots'
//!   `pve_profiles`, they are the server's data. arena.lua spawns a player with `profiles.player_N` as the options
//!   profile index (action 14) and switches it with action 16, so the options record carries one profile per race
//!   level, in order (docs/notes/gun-race.md).
//!
//! Everything here is PROVEN data; the rules built on it are in `local_match::gunrace` and the lobby.

use serde::Deserialize;

use crate::matches::{AiProfile, ModeRow};

pub const FILE: &str = "gunrace_069d.json";
/// Game mode id of the gun race (the exe's mode table 0x200987060 entry 3 `gunrace`; the `2F` / `AC` mask bit 3).
pub const MODE: u32 = 3;
/// Race levels: arena.lua's `profiles` runs 0..8 and the 9th level-up (`profiles == 9`) wins (action 26).
pub const LEVELS: usize = 9;
/// Equipment levels of `player_profiles` (the outer list; the script's `equipment_level` = options +0x1081).
pub const TIERS: usize = 5;
/// arena.lua knows `player_0` .. `player_7` only (its `alive` / `match` / `profiles` / `player_id` processors have eight
/// variables): at most eight players.
pub const MAX_PLAYERS: usize = 8;
/// arena.lua's `winner_team` of the 10-minute time end (`start_match_instantly`: action 26 `success true, winner_team 9`):
/// nobody won. A race win carries the winner's `player_id` (0..7).
pub const NO_WINNER: u8 = 9;

/// One kit variant of a race level.
#[derive(Debug, Clone, Deserialize)]
pub struct Kit {
    pub name: String,
    pub kit: AiProfile,
}

#[derive(Debug, Clone, Deserialize)]
struct Raw {
    row: (u32, u32, u32, u32, String, String, u32, u32),
    score: u32,
    rules: Vec<u32>,
    script: String,
    tiers: Vec<Vec<Vec<Kit>>>,
}

#[derive(Debug, Clone)]
pub struct GunRaceData {
    pub row: ModeRow,
    pub score: u32,
    pub rules: Vec<u32>,
    pub script: String,
    tiers: Vec<Vec<Vec<Kit>>>,
}

impl GunRaceData {
    pub fn parse(json: &str) -> Result<GunRaceData, serde_json::Error> {
        let r: Raw = serde_json::from_str(json)?;
        let (map, mode, variation, weather, project, map_name, min_players, max_players) = r.row;
        Ok(GunRaceData {
            row: ModeRow { map, mode, variation, weather, project, map_name, min_players, max_players },
            score: r.score,
            rules: r.rules,
            script: r.script,
            tiers: r.tiers,
        })
    }

    /// The kit variants of race level `level` (0..9) at equipment level `tier` (1..=5, clamped).
    pub fn variants(&self, tier: u32, level: usize) -> &[Kit] {
        let t = (tier.clamp(1, TIERS as u32) - 1) as usize;
        self.tiers.get(t).and_then(|l| l.get(level)).map(Vec::as_slice).unwrap_or(&[])
    }
}

#[cfg(test)]
mod tests {

    /// Every kit of every tier and level names items of the dictionary (the race kits are complete) and has a weapon.
    #[test]
    fn every_race_kit_resolves_to_items_with_a_weapon() {
        crate::require_tables!();
        let gd = crate::testing::tables();
        let g = &gd.gunrace;
        assert_eq!((g.row.map, g.row.mode, g.row.project.as_str(), g.row.map_name.as_str()), (13, super::MODE, "arena_01", "arena_01/default"));
        assert_eq!((g.score, g.rules.as_slice()), (10, &[109, 520, 103, 108][..]));
        for tier in 1..=5 {
            for level in 0..super::LEVELS {
                let v = g.variants(tier, level);
                assert!(!v.is_empty(), "tier {tier} level {level}");
                for k in v {
                    let kit = gd.matches.kit(&gd.items, &k.kit);
                    assert!(kit.slots.contains_key(&7), "{} has no weapon", k.name);
                    let w = gd.items.get(kit.items.iter().find(|i| i.0 == kit.slots[&7]).unwrap().1).unwrap();
                    assert!(crate::items::WEAPON_CATS.contains(&w.category), "{}: weapon category {}", k.name, w.category);
                }
            }
        }
        // CHOICE: level 7 carries the icicle kits of level 8 (the data's snowball kits fault the exe)
        for tier in 1..=5 {
            let names = |l: usize| g.variants(tier, l).iter().map(|k| k.name.clone()).collect::<Vec<_>>();
            assert_eq!(names(7), names(8), "tier {tier}");
            assert!(names(7).iter().all(|n| n.contains("icicle")));
        }
        assert_eq!(g.variants(5, 8)[0].name, "level5_icicle");
        assert_eq!(g.variants(9, 8)[0].name, "level5_icicle", "tiers clamp to 5");
        // the row is one of the data's rows (the queue, the host and the options see it)
        assert!(gd.matches.modes().iter().any(|r| *r == g.row));
    }
}

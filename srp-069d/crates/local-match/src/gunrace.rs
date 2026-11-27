//! Arms Race (gun race, mode 3) match options (docs/notes/gun-race.md).
//!
//! What arena.lua needs from the options (PROVEN by the script, `gameplay/scripting/arena.lua`, and the exe):
//! * every player record carries one profile per race level, in order: the script spawns with
//!   `profile = profiles.player_N` (action 14) and moves a player up or down with action 16, from 0 to 8; the 9th
//!   level-up is the win (`profiles == 9`: action 26 `winner_team = player_id.player_N`). So nine profiles, all present
//!   (state 0), the race kits of `scripting.player_profiles[tier][level]` (the exe never reads that list: the server
//!   writes the kits, as for the PvE bots).
//! * humans only: the script spawns a player only from `has_entered_match` (event 18), whose precondition is
//!   `not(E20{player})` (E20 = the record has an AI id, 0x2003C0078): an AI record is never spawned. No bot fill.
//! * at most eight records (`player_0` .. `player_7`).
//!
//! Server choices (CHOICE, no client evidence decides them):
//! * each record's team is its record index (0..7). arena.lua's winner is `player_id.player_N` = N, the exe writes it to
//!   the winner byte `S+0xEBC0` (action 26 -> 0x20035C6D0), and the client reports "my team won" (53 event 5) when that
//!   byte equals its own team (0x20011695E..0x200116988): with team = index exactly the race winner's client reports a
//!   win (LIKELY the original's layout). Two players are never allies: the exe's ally rules compare the team bytes
//!   (friendly fire 0x2003E9CF0, the HUD), so nobody gets ally markers or the "Brothers in Arms" damage cut.
//! * the equipment level (options +0x1081 = the script's `equipment_level`, the kit list's outer index) is one value per
//!   match: the caller's (the lobby: the highest equipment level of the humans).
//! * the kit variant of each race level: drawn per player and level from the match id (a fixed hash, so the options and
//!   the starting-world roster agree), uniform over the listed variants (a name listed twice counts twice).

use std::collections::BTreeMap;

use game_data::gunrace::{self, LEVELS};
use game_data::GameData;

use crate::options::{online_blob, HumanLoadout, HumanProfile, OnlineParams, OnlinePlayer};
use crate::Error;

/// The variant index of race level `level` for record `record` of match `match_id` among `n` variants (splitmix64 of
/// the three values).
pub fn pick(match_id: u32, record: usize, level: usize, n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    let mut z = (u64::from(match_id) << 16 | (record as u64) << 8 | level as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z % n as u64) as usize
}

/// The race kit names of record `record` (one per level), for logs and tests.
pub fn kit_names(gd: &GameData, tier: u32, match_id: u32, record: usize) -> Vec<String> {
    (0..LEVELS)
        .map(|l| {
            let v = gd.gunrace.variants(tier, l);
            v.get(pick(match_id, record, l, v.len())).map(|k| k.name.clone()).unwrap_or_default()
        })
        .collect()
}

/// A human as a gun race record carries him: his name, account, face, premium, games played, skills and boosters, with
/// the nine race kits of `tier` as profiles 0..8 (state 0, enabled), fresh item ids from 1, the kits' modules installed.
/// His own items, upgrades, decals and workshop state are not used (as the tutorial's kits).
pub fn race_human(gd: &GameData, h: &HumanLoadout, tier: u32, match_id: u32, record: usize) -> Result<HumanLoadout, Error> {
    let mut out = HumanLoadout {
        items: vec![],
        slots: BTreeMap::new(),
        slot_rounds: BTreeMap::new(),
        upgrades: BTreeMap::new(),
        attach: vec![],
        removed_defaults: BTreeMap::new(),
        bound: BTreeMap::new(),
        decals: BTreeMap::new(),
        profiles: vec![],
        ..h.clone()
    };
    for level in 0..LEVELS {
        let v = gd.gunrace.variants(tier, level);
        let k = v.get(pick(match_id, record, level, v.len())).ok_or(Error::Invalid("no gun race kit for this level"))?;
        let kit = gd.matches.kit(&gd.items, &k.kit);
        if !kit.slots.contains_key(&7) {
            return Err(Error::Invalid("a gun race kit without a weapon"));
        }
        let base = out.items.len() as u32;
        out.items.extend(kit.items.iter().map(|&(iid, did, c)| (iid + base, did, c)));
        out.attach.extend(kit.attach.iter().map(|(wp, l)| (wp + base, l.iter().map(|(s, m)| (s.clone(), m + base)).collect())));
        let slots: BTreeMap<u8, u32> = kit.slots.iter().map(|(s, iid)| (*s, iid + base)).collect();
        if level == 0 {
            out.slots = slots.clone();
            out.slot_rounds = kit.counts.clone();
        }
        out.profiles.push(HumanProfile { state: 0, enabled: true, id: level as u32 + 1, name: String::new(), slots, rounds: kit.counts });
    }
    Ok(out)
}

/// The players of a gun race match as the options carry them: team = record index, the race kits of `tier`.
pub fn race_players(gd: &GameData, players: &[OnlinePlayer], tier: u32, match_id: u32) -> Result<Vec<OnlinePlayer>, Error> {
    if players.is_empty() || players.len() > gunrace::MAX_PLAYERS {
        return Err(Error::Limit("players of a gun race match (1..=8)"));
    }
    players
        .iter()
        .enumerate()
        .map(|(i, pl)| Ok(OnlinePlayer { team: i as u8, human: race_human(gd, &pl.human, tier, match_id, i)?, hero: pl.hero, pve_budget: (0, 0) }))
        .collect()
}

/// The `match_options` of an online gun race match on the data's gun race row: the humans with their race kits
/// ([`race_players`]), no AI records, `+0x1081` = `tier` (1..=5). Returns the blob and the players as written (the
/// starting world's roster objects come from the same kits: `start_objects::roster_spec`).
pub fn gunrace_options(gd: &GameData, p: &OnlineParams, players: &[OnlinePlayer], tier: u32) -> Result<(Vec<u8>, Vec<OnlinePlayer>), Error> {
    if p.tick_ms == 0 || p.send_period_ms == 0 {
        return Err(Error::Invalid("tick_ms and send_period must not be 0"));
    }
    let row = &gd.gunrace.row;
    if (p.map, p.mode) != (row.map, row.mode) {
        return Err(Error::Invalid("the params name another map / mode than the gun race row"));
    }
    let tier = tier.clamp(1, gunrace::TIERS as u32);
    let race = race_players(gd, players, tier, p.match_id)?;
    let blob = online_blob(gd, p, &race, &row.project, &row.map_name, tier as u8, 0, None, &[])?;
    Ok((blob, race))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{max_delay_ms, MATCH_TYPE_PLAIN, SEND_PERIOD_MS, TICK_MS};

    fn params(gd: &GameData, id: u32) -> OnlineParams {
        let r = &gd.gunrace.row;
        let t = TICK_MS as u8;
        OnlineParams { map: r.map, mode: r.mode, weather: r.variation, match_id: id, tick_ms: t, send_period_ms: SEND_PERIOD_MS, match_type: MATCH_TYPE_PLAIN, max_delay_ms: max_delay_ms(t) }
    }

    fn human(name: &str, team: u8) -> OnlinePlayer {
        let mut h = HumanLoadout::default_loadout();
        h.name = name.into();
        h.skills = BTreeMap::from([("skill_543".to_string(), 1)]);
        OnlinePlayer { team, human: h, hero: None, pve_budget: (0, 0) }
    }

    /// Each record: nine present profiles, profile i = a kit of race level i at the tier, team = record index, the
    /// player's own items gone; the variant draw is fixed per (match, record, level).
    #[test]
    fn every_record_carries_the_nine_race_kits_of_the_tier() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let ps = vec![human("a", 0), human("b", 1), human("c", 0)];
        let race = race_players(&gd, &ps, 3, 77).unwrap();
        for (i, r) in race.iter().enumerate() {
            assert_eq!(r.team, i as u8);
            assert_eq!(r.human.name, ps[i].human.name);
            assert_eq!(r.human.skills, ps[i].human.skills, "skills stay");
            assert_eq!(r.human.profiles.len(), 9);
            let names = kit_names(&gd, 3, 77, i);
            for (l, prof) in r.human.profiles.iter().enumerate() {
                assert_eq!((prof.state, prof.enabled, prof.id), (0, true, l as u32 + 1));
                assert!(names[l].starts_with(&format!("level3_")), "{}", names[l]);
                assert!(gd.gunrace.variants(3, l).iter().any(|k| k.name == names[l]));
                let w = prof.slots[&7];
                assert!(r.human.items.iter().any(|it| it.0 == w), "the weapon is in the item list");
            }
            // the kits' items only (the default test human's Mosin / AK record ids 1..3 are also kit ids; dict ids differ)
            assert!(r.human.items.len() >= 9 * 2);
            assert!(r.human.upgrades.is_empty() && r.human.decals.is_empty());
        }
        assert_eq!(kit_names(&gd, 3, 77, 1), kit_names(&gd, 3, 77, 1));
        assert!(race_players(&gd, &vec![human("x", 0); 9], 1, 1).is_err(), "arena.lua knows eight players");
        assert!(race_players(&gd, &[], 1, 1).is_err());
    }

    /// The options: the gun race project / level strings, +0x1081 = the tier, one record per human, each with nine
    /// profiles (the reader's profile count byte), and the draw spreads over the variants.
    #[test]
    fn gunrace_options_header_and_records() {
        game_data::require_tables!();
        let gd = game_data::testing::tables();
        let ps = vec![human("a", 0), human("b", 1)];
        let (blob, race) = gunrace_options(&gd, &params(&gd, 0x1234), &ps, 4).unwrap();
        assert_eq!(race.len(), 2);
        assert_eq!(blob[5], 2, "n_players");
        assert_eq!(&blob[6..10], &0x1234u32.to_le_bytes());
        assert_eq!(blob[11], 0, "single_player 0");
        assert_eq!(blob[14], 4, "+0x1081 = the tier (the script's equipment_level)");
        let proj = b"arena_01";
        assert_eq!(blob[16] as usize, proj.len());
        assert_eq!(&blob[17..17 + proj.len()], proj);
        let lv = b"arena_01/default";
        let o = 17 + proj.len();
        assert_eq!(&blob[o + 1..o + 1 + lv.len()], lv);
        // the first record: team 0, then name / clan / account / ... (options.rs `player`): its profile count is 9
        let rec = o + 1 + lv.len();
        assert_eq!(blob[rec], 0, "team of record 0");
        let name_len = blob[rec + 1] as usize;
        let after_name = rec + 2 + name_len;
        let clan_len = blob[after_name] as usize;
        // account 8, games 4, +0x2DC 4, ai id 4, 3 bytes, PvE budget 4 + 4 + 2
        let prof_count = after_name + 1 + clan_len + 8 + 12 + 3 + 10;
        assert_eq!(blob[prof_count], 9, "nine profiles");
        assert_eq!(&blob[after_name + 1 + clan_len + 16..after_name + 1 + clan_len + 20], &[0xFF; 4], "a human record");
        // wrong row, bad tier clamps
        let mut p = params(&gd, 1);
        p.mode = 1;
        assert!(gunrace_options(&gd, &p, &ps, 1).is_err());
        assert_eq!(gunrace_options(&gd, &params(&gd, 1), &ps, 0).unwrap().0[14], 1);
        assert_eq!(gunrace_options(&gd, &params(&gd, 1), &ps, 9).unwrap().0[14], 5);
        // the draw is not constant: over 200 matches every level-1 assault rifle variant of tier 1 appears
        let mut seen = std::collections::BTreeSet::new();
        for id in 0..200 {
            seen.insert(kit_names(&gd, 1, id, 0)[0].clone());
        }
        assert_eq!(seen.len(), gd.gunrace.variants(1, 0).len());
    }
}

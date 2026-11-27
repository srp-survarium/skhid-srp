//! The per-player objects of a match's starting world: what the roster puts in the world's level-object list before the
//! level does (PROVEN in the exe unless marked).
//!
//! At match start (0x2002F75B0) every item in the inventory slots 0..22 of every profile that has a character (state 0)
//! adds its world objects, records in options order, before the level registers its own objects. What an item adds
//! depends on its config's `data.type` (inventory cook 0x2002D8320), with
//! `c = (u8)((slot count & 0xFF) * (stack_size & 0xFF))`:
//!
//! * 9, grenade set: `max(5, 2c)` grenades, none when `c == 0`;
//! * 16 (medkits, bandages, antitoxin, painkiller), 2 (booby trap), 3 (mines, C4), 13 (stationary scanner), 14 (healing
//!   totem), 15 (resupply crate): placeable sets of `c` objects; type 16 adds `(u8)(bonus * stack_size)` with the
//!   medkit skill bonus `floor(max(0, modifier 0x79 + 0.5))` of the profile's modifier block, even when the count is 0;
//! * 12 (artefact_sponge): the one grenade it holds (the routing of the pocket item is LIKELY);
//! * every other kind (weapons, armour, ammo, boosters, other artefacts): nothing.
//!
//! The result is a roster spec: records separated by `|`, the state-0 profiles of a record by `,`, each profile its
//! objects in slot order as a letter and a count (none = 1): `M` medkit, `G` grenade, `T` booby trap, `E` explosive
//! trap, `S` scanner, `H` healing totem, `C` resupply crate. The match server turns it into the t = 50 bytes
//! (`match_server::world::RosterObjects`). Example: one human with three profiles of medkit + one rgd-5 is
//! `MG5,MG5,MG5`.

use game_data::GameData;

use std::collections::BTreeMap;

use crate::options::{check_online_bots, match_profiles, online_mods, pve_bots, OnlineBot, OnlinePlayer};
use crate::Error;

/// `data.type` of a grenade item (grenade_set).
pub const DATA_TYPE_GRENADE: u32 = 9;
/// `data.type` of the drugs and medkits (placeable_medkit, with the skill bonus).
pub const DATA_TYPE_MEDKIT: u32 = 16;
/// `data.type` of the artefact_sponge (holds one grenade).
pub const DATA_TYPE_SPONGE: u32 = 12;
/// The inventory slots `set_world` walks (23, 0x2002F7678).
pub const WORLD_SLOTS: u8 = 23;
/// Modifier 0x79 `survival_additional_medkit_modifier`: extra medkits per medkit slot (0x2002D8431).
pub const MEDKIT_BONUS_MODIFIER: u32 = 0x79;

/// The count byte of a set: `(u8)((slot count & 0xFF) * (stack_size & 0xFF))`.
pub fn set_count(count: u16, stack_size: u32) -> u8 {
    ((count & 0xFF) as u8).wrapping_mul(stack_size as u8)
}

/// Grenades a grenade set of `count` (the slot's count) and `stack_size` puts in the world.
pub fn grenade_objects(count: u16, stack_size: u32) -> u32 {
    let c = u32::from(set_count(count, stack_size));
    if c == 0 {
        0
    } else {
        (2 * c).max(5)
    }
}

/// The medkit skill bonus of a profile: `floor(max(0, v + 0.5))` of its modifier 0x79, as a byte.
pub fn medkit_bonus(v: f64) -> u8 {
    let x = (v as f32 + 0.5).max(0.0);
    (x as u32) as u8
}

/// Medkits a type-16 set of `count` and `stack_size` puts in the world with the profile's `bonus`.
pub fn medkit_objects(count: u16, stack_size: u32, bonus: u8) -> u32 {
    u32::from(set_count(count, stack_size).wrapping_add(bonus.wrapping_mul(stack_size as u8)))
}

fn push(out: &mut String, letter: char, n: u32) {
    match n {
        0 => {}
        1 => out.push(letter),
        n => out.push_str(&format!("{letter}{n}")),
    }
}

/// The objects of one state-0 profile: its slots 0..22 in order (`items` = the record's item list).
fn profile_objects(gd: &GameData, items: &[(u32, u16, u32)], slots: &BTreeMap<u8, u32>, counts: &BTreeMap<u8, u32>, bonus: u8) -> Result<String, Error> {
    let mut objs = String::new();
    for (&slot, iid) in slots {
        if slot >= WORLD_SLOTS {
            continue;
        }
        let dict = items.iter().find(|i| i.0 == *iid && i.1 != 0).map(|i| i.1).ok_or(Error::Invalid("slot names an unknown item"))?;
        let Some(item) = gd.items.get(dict) else { continue };
        let count = counts.get(&slot).copied().unwrap_or(0).min(0xFFFF) as u16;
        let ss = item.stack_size;
        let c = u32::from(set_count(count, ss));
        match item.data_type {
            Some(DATA_TYPE_GRENADE) => push(&mut objs, 'G', grenade_objects(count, ss)),
            Some(DATA_TYPE_MEDKIT) => push(&mut objs, 'M', medkit_objects(count, ss, bonus)),
            Some(DATA_TYPE_SPONGE) => push(&mut objs, 'G', 1),
            Some(2) => push(&mut objs, 'T', c),
            Some(3) => push(&mut objs, 'E', c),
            Some(13) => push(&mut objs, 'S', c),
            Some(14) => push(&mut objs, 'H', c),
            Some(15) => push(&mut objs, 'C', c),
            _ => {}
        }
    }
    Ok(objs)
}

/// The roster spec of an online match: every player in options order (the same order and profiles as `online_options`).
pub fn roster_spec(gd: &GameData, players: &[OnlinePlayer]) -> Result<String, Error> {
    let mut records = Vec::with_capacity(players.len());
    for pl in players {
        let h = &pl.human;
        // every profile of a player carries the same modifier block (options.rs `player`)
        let bonus = medkit_bonus(online_mods(gd, h).get(&MEDKIT_BONUS_MODIFIER).copied().unwrap_or(0.0));
        let mut profiles = Vec::new();
        for (state, _, _, _, slots, counts) in match_profiles(Some(h), &h.slots, &h.slot_rounds) {
            if state != 0 {
                continue;
            }
            profiles.push(profile_objects(gd, &h.items, slots, counts, bonus)?);
        }
        records.push(profiles.join(","));
    }
    Ok(records.join("|"))
}

/// The roster spec of an online PvE mission (`pve_options`): the humans, then the 17 script bots of the difficulty with
/// both kit profiles (state 0) and the medkit bonus of their AI skills (none in the PvE data).
pub fn pve_roster_spec(gd: &GameData, humans: &[OnlinePlayer], difficulty: u8) -> Result<String, Error> {
    // the humans as `pve_options` sends them (with the nut pocket)
    let humans: Vec<OnlinePlayer> = humans.iter().map(|pl| OnlinePlayer { human: crate::options::pve_human(&pl.human), ..pl.clone() }).collect();
    let mut records = vec![roster_spec(gd, &humans)?];
    for (b, rec) in pve_bots(gd, difficulty)? {
        let bonus = medkit_bonus(gd.matches.skill_modifiers(&b.ai.skills).get(&MEDKIT_BONUS_MODIFIER).copied().unwrap_or(0.0));
        let profiles: Vec<String> = rec.profiles.iter().map(|p| profile_objects(gd, &rec.items, &p.slots, &p.counts, bonus)).collect::<Result<_, _>>()?;
        records.push(profiles.join(","));
    }
    Ok(records.join("|"))
}

/// The roster spec of an online PvP match with AI players (`online_options_bots`): the humans, then each bot's kit (one
/// profile) with the medkit bonus of its AI skills.
pub fn pvp_roster_spec(gd: &GameData, humans: &[OnlinePlayer], bots: &[OnlineBot]) -> Result<String, Error> {
    check_online_bots(gd, humans.len(), bots)?;
    let mut records = vec![roster_spec(gd, humans)?];
    for b in bots {
        let ai = &gd.matches.ai_players()[b.ai];
        let prof = b.profile(gd).ok_or(Error::Invalid("an AI player without a kit"))?;
        let kit = gd.matches.kit(&gd.items, prof);
        let bonus = medkit_bonus(gd.matches.skill_modifiers(&ai.skills).get(&MEDKIT_BONUS_MODIFIER).copied().unwrap_or(0.0));
        records.push(profile_objects(gd, &kit.items, &kit.slots, &kit.counts, bonus)?);
    }
    Ok(records.join("|"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{HumanLoadout, HumanProfile};
    use std::collections::BTreeMap;

    fn gd() -> GameData {
        game_data::testing::tables()
    }

    fn human(profiles: Vec<(u8, Vec<(u8, u32, u32)>)>, items: Vec<(u32, u16, u32)>) -> OnlinePlayer {
        let mut h = HumanLoadout::default_loadout();
        h.items = items;
        h.profiles = profiles
            .into_iter()
            .enumerate()
            .map(|(i, (state, s))| HumanProfile {
                state,
                enabled: true,
                id: i as u32 + 1,
                name: String::new(),
                slots: s.iter().map(|&(sl, iid, _)| (sl, iid)).collect(),
                rounds: s.iter().map(|&(sl, _, c)| (sl, c)).collect::<BTreeMap<_, _>>(),
            })
            .collect();
        OnlinePlayer { team: 0, human: h, hero: None, pve_budget: (0, 0) }
    }

    /// The exe's count: max(5, 2c), none for c = 0, c is a byte.
    #[test]
    fn grenade_count_follows_the_cook() {
        assert_eq!((grenade_objects(0, 1), grenade_objects(1, 1), grenade_objects(2, 1), grenade_objects(3, 1), grenade_objects(4, 1)), (0, 5, 5, 6, 8));
        assert_eq!(grenade_objects(2, 2), 8);
        assert_eq!(grenade_objects(0x100, 1), 0, "only the low byte of the count");
    }

    /// A captured world's roster: three profiles of medkit_base (slot 13) and one
    /// rgd-5 (slot 14) give `MG5` each; an absent profile adds nothing; the weapon adds nothing.
    #[test]
    fn new_player_kit_gives_a_medkit_and_five_grenades_per_profile() {
        game_data::require_tables!();
        let items = vec![(1, 107, 107), (10, 67, 1), (11, 5, 1)];
        let kit = vec![(7, 1, 0), (13, 10, 1), (14, 11, 1)];
        let p = human(vec![(0, kit.clone()), (0, kit.clone()), (0, kit.clone()), (2, kit)], items);
        assert_eq!(roster_spec(&gd(), &[p.clone(), p]).unwrap(), "MG5,MG5,MG5|MG5,MG5,MG5");
    }

    /// Type 16 is medkits by count x stack_size (bandages and antitoxin stack 2), type 9 grenades by the same count, the
    /// other placeables by their count; slots in order over all pockets.
    #[test]
    fn several_pockets_in_slot_order() {
        game_data::require_tables!();
        let gd = gd();
        let did = |name: &str| gd.items.iter().find(|i| i.cfg_name == name).unwrap().dict_id;
        let items = vec![
            (1, did("antitoxin"), 1),
            (2, did("smoke_grenade"), 3),
            (3, did("bandages"), 2),
            (4, did("rgd-5"), 2),
            (5, did("painkiller"), 1),
            (6, did("pmn_3"), 3),
            (7, did("placeable_resupply_crate"), 1),
        ];
        let p = human(vec![(0, vec![(17, 5, 1), (13, 1, 1), (14, 2, 3), (16, 4, 2), (15, 3, 2), (12, 6, 3), (11, 7, 1)])], items);
        // slot 11 crate, 12 three mines, 13 antitoxin x1 (stack 2), 14 smoke x3, 15 bandages x2 (stack 2), 16 rgd-5 x2,
        // 17 painkiller
        assert_eq!(roster_spec(&gd, &[p]).unwrap(), "CE3M2G6M4G5M");
    }

    /// A PvE roster: the human's record, then 17 bot records of two profiles each; every bot kit has pocket items
    /// (medkits, grenades), so no bot profile is empty, and the difficulties differ.
    #[test]
    fn pve_roster_has_the_17_bots_with_two_profiles() {
        game_data::require_tables!();
        let gd = gd();
        let items = vec![(1, 107, 107), (10, 67, 1), (11, 5, 1)];
        let p = human(vec![(0, vec![(7, 1, 0), (13, 10, 1), (14, 11, 1)])], items);
        let spec = pve_roster_spec(&gd, &[p.clone()], 0).unwrap();
        let recs: Vec<&str> = spec.split('|').collect();
        assert_eq!(recs.len(), 18, "{spec}");
        // the human's medkit + rgd-5 pockets, then the PvE nut pocket (slot 20, count 1): G5
        assert_eq!(recs[0], "MG5G5");
        for r in &recs[1..] {
            let profs: Vec<&str> = r.split(',').collect();
            assert_eq!(profs.len(), 2, "{r}");
            assert!(profs.iter().all(|x| !x.is_empty()), "{r}");
        }
        // pve_bot_1 easy: both kits hold medkit_base x2, rgd-5 x1, smoke_grenade x1 in pockets 1..3 -> M2 G5 G5
        assert_eq!(recs[1], "M2G5G5,M2G5G5");
        assert_ne!(spec, pve_roster_spec(&gd, &[p.clone()], 2).unwrap());
        assert!(pve_roster_spec(&gd, &[p], 3).is_err(), "no bots for difficulty 3");
    }

    /// Modifier 0x79 rounds to the medkit bonus: one more medkit per medkit slot, two per bandage slot (stack 2).
    #[test]
    fn medkit_skill_adds_bonus_times_stack() {
        game_data::require_tables!();
        let gd = gd();
        assert_eq!(gd.matches.modifier_names()[MEDKIT_BONUS_MODIFIER as usize], "survival_additional_medkit_modifier");
        let did = |name: &str| gd.items.iter().find(|i| i.cfg_name == name).unwrap().dict_id;
        let mut p = human(vec![(0, vec![(13, 1, 1), (14, 2, 1)])], vec![(1, did("medkit_base"), 1), (2, did("bandages"), 1)]);
        assert_eq!(roster_spec(&gd, &[p.clone()]).unwrap(), "MM2");
        p.human.extra_modifiers.insert(MEDKIT_BONUS_MODIFIER, 1.0);
        assert_eq!(roster_spec(&gd, &[p]).unwrap(), "M2M4");
        assert_eq!((medkit_bonus(0.4), medkit_bonus(0.5), medkit_bonus(-3.0)), (0, 1, 0));
        assert_eq!(medkit_objects(0, 1, 1), 1, "the bonus even with count 0");
    }
}

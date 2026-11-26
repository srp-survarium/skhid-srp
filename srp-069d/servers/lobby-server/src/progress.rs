//! Progression rules of the account: the new-player profile, the first faction choice and experience with level-up
//! silver. Audit and sources: docs/notes/progression-audit.md.

use accounts::{Currency, Profile, UID_BASE};
use game_data::{GameData, Roll};

/// Slots whose `slot_rounds` count rounds or units placed (same list as `accounts::profile`): ammo and pockets.
const COUNTED: [u8; 12] = [8, 9, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20];

/// What a NEW account has, exactly the data's kit (`items_for_new_player.options`): the faction-free rows plus the
/// armour of `faction`, worn by the equipment profiles the rows name (`profiles_to_wear` 0..=2). Money, experience and reputation start at 0:
/// no file or exe value gives a starting amount (the tutorial awards of `unique_awards.options` are a separate,
/// not yet paid, source). The test starter (1 000 000 silver, level 5, 200 000 reputation in every faction) is
/// [`Profile::starter`], which only tests use.
///
/// Every module of the kit (rows 1703, 1867, 1768, 1771, 1772) gets rarity green "Factory-made" `(312, 1)` and drop grades
/// drawn from the green cells of the general table (`ModuleDrops::green`, rolled with `r`): random green is LIKELY
/// (the client has no state for an owned module without a rarity, and the 0.53 notes call green the default
/// quality; no 0.69d source fixes the grades), the drop grades come from the general table `modules_mods_pair_prob`.
pub fn new_player(gd: &GameData, faction: u8, r: &mut dyn Roll) -> Profile {
    let mut p = Profile::starter();
    p.items.clear();
    p.slots.clear();
    p.slot_rounds.clear();
    p.next_uid = UID_BASE;
    (p.silver, p.gold, p.parts, p.xp) = (0, 0, 0, 0);
    p.faction = faction;
    // a new player starts on the client's nickname screen (9D F = 0), face 0 until chosen there
    p.onboarding = 0;
    p.face = 0;
    p.fac_points = (1..=4).map(|f| (f, 0)).collect();
    for row in gd.progression.kit_for(faction) {
        let stack = gd.items.is_stackable(row.dict_id);
        let uid = match p.items.iter().find(|i| i.dict_id == row.dict_id && stack).map(|i| i.uid) {
            Some(uid) => {
                p.item_mut(uid).unwrap().count += row.buy_amount;
                uid
            }
            None => p.add_item(row.dict_id, if stack { row.buy_amount } else { gd.items.amount_field(row.dict_id, 1) }),
        };
        if !stack {
            crate::chests::give_rarity(gd, &mut p, uid, r);
            // one more unit of a non-stack item is another item
            for _ in 1..row.buy_amount {
                let more = p.add_item(row.dict_id, gd.items.amount_field(row.dict_id, 1));
                crate::chests::give_rarity(gd, &mut p, more, r);
            }
        }
        // `profiles_to_wear` names the equipment profiles (0..=2) that wear the row; each wears it in its own table
        if let Some(slot) = row.slot {
            for &prof in row.profiles_to_wear.iter().filter(|x| (**x as usize) < accounts::equip::PROFILES) {
                let (slots, rounds) = p.profile_slots_mut(prof);
                slots.insert(slot, uid);
                if COUNTED.contains(&slot) {
                    rounds.insert(slot, row.wear_amount);
                }
            }
        }
    }
    // a new player starts with all three profiles (CHOICE: the data kits all three; the original's free-profile promotion
    // period, 94 service types 1..=7, has no known length, so the profiles are kept for good)
    for e in p.equip.profiles.iter_mut() {
        e.owned = true;
    }
    // the newbie's blessing (CHOICE; no data row grants it): `new_player_booster` x its
    // `service_duration_matches`, one used per online match (boosters::use_services)
    let blessing = game_data::vouchers::NEW_PLAYER_BOOSTER;
    if let Some(n) = gd.vouchers.get(blessing).and_then(|r| r.service_duration_matches).filter(|n| *n > 0) {
        p.add_item(blessing, n);
    }
    p
}

/// The faction chosen for the first time (4A 00 in the tutorial step; the exe stores it only while no faction was set):
/// the account has not earned anything yet, so its faction armour becomes the kit of the chosen
/// faction (same slots, same item uids). Returns whether an item changed. Any later switch changes only the active
/// faction.
pub fn first_faction_choice(gd: &GameData, p: &mut Profile, new: u8) -> bool {
    let pristine = p.xp == 0 && p.fac_points.values().all(|&v| v == 0);
    if !pristine || new == p.faction {
        return false;
    }
    let mut changed = false;
    for old_row in gd.progression.kit_for(p.faction).filter(|r| r.faction_id == Some(p.faction)) {
        let Some(new_row) = gd.progression.kit_for(new).find(|r| r.faction_id == Some(new) && r.slot == old_row.slot) else { continue };
        for it in p.items.iter_mut().filter(|i| i.dict_id == old_row.dict_id) {
            it.dict_id = new_row.dict_id;
            it.count = gd.items.amount_field(new_row.dict_id, 1);
            changed = true;
        }
    }
    changed
}

/// The faction-level rule of the pocket items (PROVEN): an item with a
/// dictionary `faction` (1..=4) is open when the account's level in that faction (index of the last reached threshold
/// of `reputation_leveling.options`, `faction_level`) is at least the item's `item_level` (1 for none). Items of no
/// faction are always open. The shop uses [`shop_allows`].
pub fn faction_allows(gd: &GameData, p: &Profile, dict: u16) -> bool {
    let Some(it) = gd.items.get(dict) else { return true };
    if !(1..=4).contains(&it.faction) {
        return true;
    }
    game_data::levels::faction_level(p.points(it.faction as u8)) >= it.level_min1()
}

/// The shop's own buy rule (PROVEN, lobby movie `GangShop.updateItemInfo` / `setShopItems`, `canBuy`): a gold item
/// (8A kind 1) or a league item (kind 7) is always buyable; any other item is buyable when the account's level in the
/// item's faction (1..=4) reaches the item's `item_level` (the tile's `faction_level`, 0x20014485E); an item of no
/// faction uses the best of the four faction levels (`GangShop.getMaxSellerLevel`). Levels count from 1.
pub fn shop_allows(gd: &GameData, p: &Profile, dict: u16) -> bool {
    if gd.shop.price(dict).is_some_and(|pr| pr.kind == 1 || pr.kind == game_data::shop::LEAGUE_KIND) {
        return true;
    }
    let Some(it) = gd.items.get(dict) else { return true };
    let level = |f: u8| game_data::levels::faction_level(p.points(f)).max(1);
    let have = if (1..=4).contains(&it.faction) { level(it.faction as u8) } else { (1..=4).map(level).max().unwrap_or(1) };
    have >= it.item_level
}

/// Add experience; the silver reward of every level reached is paid (`character_leveling`). Returns the silver paid.
pub fn add_xp(gd: &GameData, p: &mut Profile, xp: u32) -> u32 {
    let old = p.xp;
    p.add_xp(xp);
    let silver = gd.levels.level_up_silver(old, p.xp);
    if silver > 0 {
        p.credit(Currency::Silver, u64::from(silver));
    }
    silver
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gd() -> GameData {
        game_data::testing::tables()
    }

    #[test]
    fn a_new_account_has_the_data_kit_and_nothing_else() {
        game_data::require_tables!();
        let g = gd();
        let p = new_player(&g, 1, &mut game_data::SplitMix(1));
        assert_eq!((p.silver, p.gold, p.parts, p.xp), (0, 0, 0, 0));
        assert!(p.fac_points.values().all(|&v| v == 0));
        assert_eq!(g.levels.level_of(p.xp), 1);
        // equipment profiles: all three are owned and worn from the kit (`profiles_to_wear`); profile 0 is current
        assert!(p.equip.profiles.iter().all(|e| e.owned));
        assert_eq!(p.equip.current, 0);
        let worn = |i: u8, slot: u8| p.profile_slots(i).get(&slot).and_then(|u| p.item(*u)).map(|x| x.dict_id);
        let rounds = |i: u8, slot: u8| if i == p.equip.current { p.slot_rounds[&slot] } else { p.equip.profiles[i as usize].rounds[&slot] };
        // profile 0: AKMN (1201) + 200 rounds of 81, profile 1: Mosin (107) + 30 rounds of 104, profile 2: TOZ-66 (17) + 80 of 22
        assert_eq!((worn(0, 7), worn(0, 8), rounds(0, 8)), (Some(1201), Some(81), 200));
        assert_eq!((worn(1, 7), worn(1, 8), rounds(1, 8)), (Some(107), Some(104), 30));
        assert_eq!((worn(2, 7), worn(2, 8), rounds(2, 8)), (Some(17), Some(22), 80));
        for i in 0..3 {
            // TT-33 (18) + 50 rounds of 1178, medkit 67, RGD-5, Scavenger armour in every profile
            assert_eq!((worn(i, 10), worn(i, 9), rounds(i, 9)), (Some(18), Some(1178), 50));
            assert_eq!((worn(i, 13), worn(i, 14)), (Some(67), Some(5)));
            assert_eq!((worn(i, 2), worn(i, 4), worn(i, 6)), (Some(31), Some(45), Some(37)));
        }
        // bought but worn by nobody: PPSh
        assert!(p.items.iter().any(|i| i.dict_id == 106));
        // the ammo of the three profiles' pistol slots is one stack of 150 rounds (three rows of the same item)
        assert_eq!(p.items.iter().find(|i| i.dict_id == 1178).unwrap().count, 150);
        // every row of the data is in the inventory exactly once per unit (faction armour of faction 1 only)
        assert!(!p.items.iter().any(|i| i.dict_id == 29 || i.dict_id == 745));
        // plus the newbie's blessing for the first 3 online matches
        assert_eq!(p.items.iter().filter(|i| i.dict_id == game_data::vouchers::NEW_PLAYER_BOOSTER).map(|i| i.count).collect::<Vec<_>>(), vec![3]);
    }

    /// The five kit modules are green (`(312, 1)`) with drop grades inside the client's green bounds, over many seeds;
    /// the one-modifier rails (1703, 1867) take column 9 of the table.
    #[test]
    fn the_kit_modules_are_green_with_green_grades() {
        game_data::require_tables!();
        let g = gd();
        let d = &g.module_drops;
        for seed in 0..50 {
            let p = new_player(&g, 1 + (seed % 4) as u8, &mut game_data::SplitMix(seed));
            let mut kit: Vec<u16> = p.items.iter().filter(|i| g.items.is_module(i.dict_id)).map(|i| i.dict_id).collect();
            kit.sort_unstable();
            assert_eq!(kit, vec![1703, 1768, 1771, 1772, 1867], "the kit's five modules");
            for it in p.items.iter().filter(|i| g.items.is_module(i.dict_id)) {
                let m = &p.mods[&it.uid];
                assert_eq!(m[0], (312, 1), "{} {m:?}", it.dict_id);
                let (a, b) = d.drop_groups(it.dict_id);
                assert_eq!(m.len(), 1 + a.is_some() as usize + b.is_some() as usize, "{} {m:?}", it.dict_id);
                let (lo_i, hi_i, lo_j, hi_j) = d.client_bounds(0, b.is_none()).unwrap();
                assert!((lo_i..=hi_i).contains(&(m[1].1 as usize)), "{} {m:?}", it.dict_id);
                match b {
                    Some(_) => {
                        assert!((lo_j..=hi_j).contains(&(m[2].1 as usize)), "{} {m:?}", it.dict_id);
                        assert_eq!(d.cell_value(m[1].1 as usize, m[2].1 as usize), 0, "{} {m:?}", it.dict_id);
                    }
                    None => assert_eq!(d.cell_value(m[1].1 as usize, 9), 0, "{} {m:?}", it.dict_id),
                }
            }
        }
    }

    #[test]
    fn first_choice_swaps_the_faction_armour_then_it_is_a_plain_switch() {
        game_data::require_tables!();
        let g = gd();
        let mut p = new_player(&g, 1, &mut game_data::SplitMix(1));
        assert!(first_faction_choice(&g, &mut p, 3));
        let worn = |p: &Profile, slot: u8| p.slots.get(&slot).and_then(|u| p.item(*u)).map(|i| i.dict_id);
        assert_eq!((worn(&p, 2), worn(&p, 4), worn(&p, 6)), (Some(750), Some(751), Some(752)));
        p.faction = 3;
        p.add_points(3, 10);
        assert!(!first_faction_choice(&g, &mut p, 2), "after the first reward the kit stays");
    }

    #[test]
    fn every_reputation_source_credits_the_active_faction_only() {
        game_data::require_tables!();
        let g = gd();
        let mut p = new_player(&g, 1, &mut game_data::SplitMix(1));
        p.faction = 3;
        // chest reward 1164 (opened chests, hunt, login calendar all use chests::grant) and the daily quest chest
        let (_, got) = crate::chests::grant(&g, &mut p, &[game_data::chests::Reward { dict: crate::chests::REPUTATION, amount: 700 }]);
        assert!(got.reputation);
        crate::quests::grant(&g, &mut p, 1164, 300);
        assert_eq!((p.points(3), p.points(1), p.points(2), p.points(4)), (1000, 0, 0, 0));
        // after switching, the next credit goes to the new faction and faction 3 keeps its points
        p.set_faction(2).unwrap();
        crate::quests::grant(&g, &mut p, 1164, 50);
        assert_eq!((p.points(3), p.points(2)), (1000, 50));
    }

    #[test]
    fn match_reputation_is_credited_to_the_active_faction_on_finish() {
        game_data::require_tables!();
        // play::finish_with does `p.add_points(p.faction, o.reputation)`; the amount comes from the data (match_reward)
        let g = gd();
        let mut f = game_data::MatchFacts::local(1, 1, true, true, 2);
        f.kills = Some(vec![game_data::KillFact::default(); 2]);
        let r = g.match_reward(&f, 1, &mut game_data::SplitMix(5));
        assert_eq!(r.rep, 2 * 5 * 4, "2 tdm kills x 5 reputation x match level 1 factor 4");
    }

    #[test]
    fn faction_items_open_with_the_faction_level_of_the_item() {
        game_data::require_tables!();
        let g = gd();
        let mut p = new_player(&g, 1, &mut game_data::SplitMix(1));
        // 65 = painkiller (faction 2, item level 2), 66 = bandages (faction 1, item level 2), 67 = medkit (faction 1, level 1)
        assert!(faction_allows(&g, &p, 67));
        assert!(!faction_allows(&g, &p, 66));
        assert!(!faction_allows(&g, &p, 65), "reputation of faction 1 does not open faction 2");
        p.add_points(1, 5_000);
        assert!(faction_allows(&g, &p, 66));
        assert!(faction_allows(&g, &p, 104), "ammo has no faction");
    }

    #[test]
    fn the_shop_opens_by_the_items_faction_the_best_faction_or_gold() {
        game_data::require_tables!();
        let g = gd();
        let mut p = new_player(&g, 1, &mut game_data::SplitMix(1));
        // 1218 = SPAS-12 (faction 3, item level 3, silver)
        assert!(!shop_allows(&g, &p, 1218));
        p.add_points(1, 80_000);
        assert!(!shop_allows(&g, &p, 1218), "another faction's level does not open a faction item");
        p.add_points(3, 20_000);
        assert!(shop_allows(&g, &p, 1218), "faction 3 level 3");
        // 72 = 12 mm slug (no faction, item level 2): the best of the four faction levels
        let mut q = new_player(&g, 2, &mut game_data::SplitMix(2));
        assert!(!shop_allows(&g, &q, 72));
        q.add_points(4, 5_000);
        assert!(shop_allows(&g, &q, 72), "faction 4 level 2 is the best level");
        // 1213 = NeoStead (gold, item level 2): a gold item is always buyable
        assert_eq!(g.shop.price(1213).map(|pr| pr.kind), Some(1));
        assert!(shop_allows(&g, &new_player(&g, 3, &mut game_data::SplitMix(3)), 1213));
    }

    #[test]
    fn level_up_pays_the_silver_of_every_level_reached() {
        game_data::require_tables!();
        let g = gd();
        let mut p = new_player(&g, 1, &mut game_data::SplitMix(1));
        let paid = add_xp(&g, &mut p, 1030); // levels 2 and 3: 500 + 500
        assert_eq!((paid, p.silver), (1000, 1000));
        assert_eq!(add_xp(&g, &mut p, 10), 0);
        // the cap: level 100 pays its row once, nothing above
        let top = *g.levels.xp_table().last().unwrap();
        let mut q = new_player(&g, 1, &mut game_data::SplitMix(1));
        add_xp(&g, &mut q, top + 1_000_000);
        assert_eq!(g.levels.level_of(q.xp), 100);
    }
}

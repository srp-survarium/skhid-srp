//! The after-match charge of an online PvP match, priced into the three lists of the `97` tail
//! `spent_for_autobuy_money` (exe 0x200183010 -> `fill_match_statistic` 0x20013EF50: `spent_money_ammunition` /
//! `spent_money_pockets` / `spent_money_repair`, shown by the lobby screen as "Ammunition" / "Special items" / "Match
//! costs"). Every price rule is in [`price`].
//!
//! * **Equipment use** ("Match costs", CHOICE): the silver of the player's equipment level
//!   ([`crate::items::equipment_level`]: the highest item level worn over his playable profiles, the client's
//!   0x2003330C0) in [`crate::config::LobbyConfig::equipment_use_costs`], default the OFFICIAL patch 0.55 repair prices
//!   (level 1 0, 2 1000, 3 2000, 4 5000, 5 8000; "The price for repairs is now untied from the item of equipment or
//!   weapons ... depends on the level of equipment"). The 0.69d data's own `equipment_use_costs` (booster_parameters.options)
//!   is 0 at every level; it is used when the setting is empty. It is the "repairs" group (type 2) of the client's own
//!   match participation cost (0x200333210 -> 0x2001BFBC0): one entry with dict 0xFFFF, only
//!   when it costs something (the details window titles a repair entry of dict -1 / 65535 with `btn_faction_equipment`).
//!   The client's pre-match screen reads its OWN data table (zeros) and no message carries another, so it shows 0 there.
//!   The per-item `match_participation_cost` of game.options is not used: the exe never reads it (a pre-0.55 key).
//! * **Pockets** stay unlimited in the match (options storage 3000, ammo-box); the units used are charged after the
//!   match at the shop price ([`ShopTable::pocket_cost`](game_data::ShopTable::pocket_cost), the 8A price of the item),
//!   one entry per dict in "Special items" (CHOICE).
//! * Rounds are free ("Ammunition" stays empty); boosters are paid at the match start (boosters.rs), not here.
//!
//! CHOICES (no source decides): a pocket item priced in gold is not charged; an inactive player is not charged (an inactive
//! player gets no reward and no fine); when the silver does not cover the charge, what he has is taken (never
//! below 0) and the shortfall is logged.

use game_data::{GameData, Wallet};
use protocol::s2c::{Spent, SPENT_AMMO, SPENT_POCKETS, SPENT_REPAIR};

/// One stack's use as the world host reported it (`UsedStack`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackUse {
    pub uid: u32,
    pub dict: u16,
    pub used: u32,
}

/// What one player used in the match (one `PlayerUse` row of the world host's `Used` frame).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MatchUse {
    /// bit p = he spawned with options profile p
    pub profiles: u16,
    /// the weapon dicts of those profiles (slots 7 and 10), each once (reported by the host; not priced)
    pub weapons: Vec<u16>,
    /// his ammo / pocket stacks' use (start storage - end storage - carried at the end), only used > 0
    pub stacks: Vec<StackUse>,
}

/// The priced lists, in the `97` order (index [`SPENT_AMMO`], [`SPENT_POCKETS`], [`SPENT_REPAIR`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Charge {
    pub spent: [Vec<Spent>; 3],
}

impl Charge {
    /// Silver of all three lists.
    pub fn total(&self) -> u64 {
        self.spent.iter().flatten().map(|s| u64::from(s.money)).sum()
    }
}

fn add(list: &mut Vec<Spent>, dict: u16, money: u64) {
    if money == 0 {
        return;
    }
    let money = money.min(u64::from(u32::MAX)) as u32;
    match list.iter_mut().find(|s| s.dict == dict) {
        Some(s) => s.money = s.money.saturating_add(money),
        None => list.push(Spent { money, dict }),
    }
}

/// The dict of the equipment-use entry in "Match costs": the client's own type-2 entry (dict 0xFFFF, 0x2001BFCCD).
pub const EQUIPMENT_USE_DICT: u16 = 0xFFFF;

/// The price rules (module docs): the equipment use silver -> "Match costs" (dict [`EQUIPMENT_USE_DICT`], no entry for 0),
/// the pocket units of `used` (None = the host sent no `Used` row) -> "Special items", "Ammunition" empty.
pub fn price(gd: &GameData, equipment_use: u32, used: Option<&MatchUse>) -> Charge {
    let mut c = Charge::default();
    add(&mut c.spent[SPENT_REPAIR], EQUIPMENT_USE_DICT, u64::from(equipment_use));
    for s in used.map(|u| u.stacks.as_slice()).unwrap_or_default().iter().filter(|s| s.used > 0 && gd.items.is_quick_item(s.dict)) {
        let (wallet, cost) = gd.shop.pocket_cost(s.dict, s.used);
        if wallet == Wallet::Silver {
            add(&mut c.spent[SPENT_POCKETS], s.dict, cost);
        }
    }
    // rounds fired cost nothing (free ammo): no SPENT_AMMO entry
    let _ = SPENT_AMMO;
    c
}

/// The premium ammo boxes the world applied: one per profile the player spawned with whose slot 11 / 12 held one (the
/// world host's `Used` rows of categories 37 / 38; the box applies at the spawn). They are bought
/// beforehand (gold), so they cost nothing here: the stack loses `n` (`(uid, dict, n)`). A PvE mission takes all of
/// them; an online PvP match takes one per box slot ([`match_boxes`]).
pub fn boxes(gd: &GameData, u: &MatchUse) -> Vec<(u32, u16, u32)> {
    u.stacks.iter().filter(|s| s.used > 0 && gd.items.is_ammo_booster(s.dict)).map(|s| (s.uid, s.dict, s.used)).collect()
}

/// The premium ammo boxes an online PvP match takes from a player: ONE per box slot per match (the primary weapon's box,
/// category 37 / slot 11, and the pistol's, category 38 / slot 12), whatever profiles he swapped, also when he left
/// early (online.rs) (CHOICE). Per slot it is the first box the world host reported (its lowest spawned profile).
pub fn match_boxes(gd: &GameData, u: &MatchUse) -> Vec<(u32, u16, u32)> {
    let mut out: Vec<(u32, u16, u32)> = Vec::new();
    for (uid, dict, _) in boxes(gd, u) {
        let cat = gd.items.category(dict);
        if !out.iter().any(|(_, d, _)| gd.items.category(*d) == cat) {
            out.push((uid, dict, 1));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gd() -> GameData {
        game_data::testing::tables()
    }

    #[test]
    fn equipment_use_by_level_no_weapon_charge_and_pockets_by_units() {
        game_data::require_tables!();
        let d = gd();
        let medkit = 760; // science_medkit, a pocket item with a silver 8A price
        let (_, one) = d.shop.pocket_cost(medkit, 1);
        assert!(one > 0);
        let u = MatchUse {
            profiles: 0b11,
            // ak_74n (match_participation_cost 1000 in game.options: not charged), akmn
            weapons: vec![306, 1201, 306],
            stacks: vec![
                StackUse { uid: 100_028, dict: medkit, used: 3 },
                StackUse { uid: 100_029, dict: medkit, used: 1 },
                // 7.62x39 rounds: free
                StackUse { uid: 100_014, dict: 81, used: 412 },
            ],
        };
        // no equipment use (level 0 / 1): no "Match costs" entry, the pockets as above
        for level in [0u8, 1] {
            let c = price(&d, crate::config::LobbyConfig::default().equipment_use_cost(&d, level), Some(&u));
            assert!(c.spent[SPENT_REPAIR].is_empty(), "level {level}");
            assert_eq!(c.spent[SPENT_POCKETS], vec![Spent { money: (4 * one) as u32, dict: medkit }]);
            assert!(c.spent[SPENT_AMMO].is_empty());
            assert_eq!(c.total(), 4 * one);
        }
        // nothing used, nothing reported: nothing charged
        assert_eq!(price(&d, 0, Some(&MatchUse { profiles: 1, weapons: vec![306], stacks: vec![] })), Charge::default());
        assert_eq!(price(&d, 0, None), Charge::default());
    }

    #[test]
    fn the_silver_factor_setting_defaults_to_the_proposal_and_empty_means_the_data() {
        let cfg = crate::config::LobbyConfig::default();
        assert_eq!((0..=6).map(|l| cfg.match_silver_factor(l).unwrap()).collect::<Vec<_>>(), vec![1.0, 1.0, 3.5, 7.0, 10.0, 14.0, 14.0]);
        let empty = crate::config::LobbyConfig { match_silver_factors: vec![], ..Default::default() };
        assert_eq!(empty.match_silver_factor(3), None);
    }

    #[test]
    fn the_equipment_use_follows_the_0_55_table_by_level_and_is_one_entry() {
        game_data::require_tables!();
        let d = gd();
        let cfg = crate::config::LobbyConfig::default();
        // the 0.55 prices by level (0 = nothing worn), one entry of dict 0xFFFF when above 0
        let want = [0u32, 0, 1000, 2000, 5000, 8000];
        for (level, w) in want.iter().enumerate() {
            let c = price(&d, cfg.equipment_use_cost(&d, level as u8), None);
            let entry: Vec<Spent> = if *w > 0 { vec![Spent { money: *w, dict: EQUIPMENT_USE_DICT }] } else { vec![] };
            assert_eq!(c.spent[SPENT_REPAIR], entry, "level {level}");
            assert_eq!(c.total(), u64::from(*w));
        }
        assert_eq!(cfg.equipment_use_cost(&d, 9), 8000, "past the table: its last value");
        // an empty setting uses the 0.69d data (0 at every level); a data table with values is read by level
        let empty = crate::config::LobbyConfig { equipment_use_costs: vec![], ..Default::default() };
        assert!((0..=6).all(|l| empty.equipment_use_cost(&d, l) == 0));
        let mut d2 = gd();
        d2.vouchers.set_equipment_use_costs(vec![0, 0, 1, 2, 3, 4]);
        assert_eq!(empty.equipment_use_cost(&d2, 4), 3);
    }
}

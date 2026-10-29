//! The 0.26g shop: prices from each item's `base_cost` in the dictionary
//! (0.21d's data had none, hence its hand-made `crate::prices`), but for
//! ammo, which costs about half of it by the owner's choice
//! ([`AMMO_PRICES`]).
//!
//! What the 0.26g client does with a price (`lobby_menu.swf`, decompiled
//! with FFDec, and the exe):
//! - The exe hands `base_cost` to the lobby as the item descriptor's `cost`
//!   (0x286950, member name at 0xA015C4). PROVEN.
//! - **A stack's price is for one step of the slider, and ammo's step is a
//!   clip** (`MessageAsk.update_slider`: `_step = clip_size`;
//!   `onSliderUpdate`: `slider / _step * _price`). The lobby uses the
//!   descriptor's `cost` this way when selling
//!   (`InventoryList.sellObject`: `cost / 4` a step), so ammo's `base_cost`
//!   is **per clip**, not per round. PROVEN for the client's use of it;
//!   that the original server charged `base_cost` a clip in its price list
//!   is LIKELY (the same dialog and step show the shop's price).
//! - Selling gives a quarter: `cost / 4`, times `condition / 100` for an item
//!   that is no stack (`InventoryList.sellObject`, `ItemCard`). PROVEN (what
//!   the client shows; the server pays).
//! - A repair costs `(1 - condition / 100) * repair_cost_ratio * cost`, plus
//!   the fix costs of the item's defects (`MessageRepair.init`). PROVEN.
//! - A premium item's `cost` is in gold: the lobby multiplies it by the
//!   premium exchange rate (service price 4, `lobby.md` §2.13) before
//!   taking the quarter.
//!
//! The price list (lobby 0x8A, `lobby.md` §2.10) is records of
//! `u32 cost, u16 dict_id, u8 level, u8 faction_id`. `GangShop.setupShopItems`
//! reads `faction_id` as the seller and `level` as the reputation level with
//! it that buying needs (`canBuy = unlockedLevel[seller] >= level`); seller 0
//! is always buyable. PROVEN.
//!
//! **The client decides nothing here.** The exe keeps the records as a
//! flat array and hands every one to the lobby (0x223190: `faction_id`,
//! `level`, `dictId`, `cost` of each, no grouping). `setupShopItems` makes
//! a shop item of each record under the record's seller, with the record's
//! level as its lock (and the rank's name as the lock's label), and adds the
//! seller to the item descriptor's `sellers`. An item's own config
//! (`faction`, `item_level`) is not consulted for the shop. PROVEN (the
//! ActionScript and the exe's filler). So the seller and the rank of an item
//! are wholly the server's, and **an item may have several records, one a
//! seller**: `descrData.sellers` is an array the lobby grows
//! (`noSeller` / `push`), an owned item shows a seller's icon only when it
//! has one seller (`InventoryList.createItem`), and the shop's seller tabs
//! test every seller of an item (`isSeller`). PROVEN in the ActionScript;
//! not yet seen live. When the window names no seller, the exe names the
//! seller of the first record of the item (0x288BC0).
//!
//! Which level an item needs is not in the client. srp's rule [LIKELY]: the
//! item's `item_level`, less 5 for Renaissance and Edge. The data fits it:
//! Scavenger and Black Market gear has item levels 1..=5 and those factions
//! five levels above 0; Renaissance and Edge gear has 6..=15 and those
//! factions ten. The wiki's lists of January and February 2015 (0.26g to
//! 0.26i; `progression_sources.md` §3) agree with the rule for every weapon
//! and piece of armour but the few in [`EXCEPTIONS`], and they give the
//! sellers and ranks of ammo, drugs, the trap and grenades, which no config
//! has.
//!
//! What is listed [srp's choice where marked]:
//! - weapons and armour whose config names a faction, sold by it at the
//!   rule's level, or as [`EXCEPTIONS`] says;
//! - drugs, the trap and grenades: by the factions at the ranks of
//!   [`EXCEPTIONS`], one record for each faction that sold it;
//! - ammo: by the factions of [`EXCEPTIONS`], **at no rank** by the owner's
//!   choice ([`AMMO_NEEDS_RANK`]). 0.26 sold ammo at a rank (patch note
//!   0.26c) and 0.27a took that back ("Purchase of ammunition no longer
//!   requires reputation"); the ranks of the time stay in the table;
//! - the boosters (the wiki's "Independent Traders") and the one ammo on no
//!   list of the time (9x19 7N21): seller 0 and level 0 [choice for the
//!   7N21: unknown, so sold to everyone as before];
//! - the PP-19-01 Vityaz and the MP-27M Lancaster, weapons whose configs
//!   name no faction: seller 0 and level 0, the Vityaz at the owner's price
//!   of 25,000 ([`OWNER_PRICES`]) [the owner's choice, 2026-10-17; both
//!   still drop, and their premium copies stay in their bundles];
//! - the factions' own camouflages ([`is_shop_camo`]): the faction whose
//!   folder the config is in ([`CAMO_SHOPS`]), at level 0, for `base_cost`
//!   [the owner's choice, 2026-10-17];
//! - the starting Mosin carbine and PPSh-41, which the game prices at 0:
//!   seller 0 and level 0 at the owner's 1,000 each ([`OWNER_PRICES`]) [the
//!   owner's choice, 2026-10-18; new accounts still get both]; the starting
//!   TT-33 the same way at the owner's 500 [2026-10-23];
//! - not listed: anything else with `base_cost` 0 (their ammo is listed at
//!   no cost, see [`sellers_of_in`]; artefacts, money, the New Year 2015 camouflage), premium
//!   items (their cost is gold; they belong to the premium shop, 0x95, or
//!   are sold one by one for gold, `shop_offers`), the other weapons with no
//!   faction (VSS, SR-3M, the SOCOM 16), artefacts (as 0.21d), vouchers,
//!   decals and the drop camouflages (the wiki lists decals at ranks; not
//!   applied);
//! - not sold at all, neither for silver nor for gold ([`NOT_SOLD`]): the
//!   Russian and Belarusian flags and coats of arms, the flag of the USSR
//!   and the red star [the owner's choice, 2026-10-18]. srp does not touch
//!   copies an account already owns (the owner removes them by hand);
//! - scopes: only the scope items the game data has variants for
//!   ([`super::scopes`]), at their `base_cost`, by seller 0 at no rank
//!   [srp: a scope is view-only, so nothing to unlock]. 0.26g's own three
//!   (Leupold, PSO-1, 1P69), the variants and any other category-22 entry
//!   are not sold. The price is the dictionary's, as for any item: the
//!   client shows a quarter of it for selling (`InventoryList.sellObject`),
//!   so a price set anywhere else would not agree with what it shows.

use super::dictionary::{Dictionary, Item};
use super::item_params::{ItemParams, item_params};
use super::scopes::{SCOPE_CATEGORY, ScopeFits};

/// 0.26g faction ids (`factions_dict`), as 0.21d's.
pub use crate::prices::faction;

/// The categories listed with no seller: ammo (9, 18..=21), drugs and
/// boosters (10), traps (11) and grenades (24).
const SUPPLIES: [u8; 8] = [9, 10, 11, 18, 19, 20, 21, 24];
/// Weapons (8, 13..=17) and armour (1..=7).
const GEAR: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 13, 14, 15, 16, 17];
/// Camouflages.
const CAMOUFLAGE: u8 = 25;

/// The `type` of a faction's own camouflage: a "stock" camouflage, sold for
/// silver. The others are `drp_camo` (drops, `base_cost` 1) and `prm_camo`
/// (premium, for gold; the New Year 2015 one costs nothing).
pub const SHOP_CAMO: &str = "stk_camo";

/// The folder the camouflages' configs are filed in, one folder a faction.
pub const CAMO_FOLDER: &str = "gameplay/items/upgrades/camouflages/";

/// Which faction sells the shop camouflages of each folder under
/// [`CAMO_FOLDER`] (the game's own folder names; `renesance` is spelt so).
/// The live data (stock 0.26g with patches 12, 14 and 17) has eight shop
/// camouflages: Black death, Ifrit and Denim of the Black Market, Oakley
/// and Huntsman of Renaissance, Ghost, Quilted fabric and Seeker of the
/// Scavengers. The wiki of January 2015 lists the Scavengers' three at
/// ranks 1, 3 and 5 (`progression_sources.md` §3); the owner's choice
/// (2026-10-17) is no rank. No folder is the Edge's.
pub const CAMO_SHOPS: [(&str, u8); 3] = [
    ("scavengers", faction::SCAVENGERS),
    ("black_market", faction::BLACK_MARKET),
    ("renesance", faction::RENAISSANCE),
];

/// Whether `item` is a faction's own camouflage, which the shop sells: a
/// camouflage of `type` [`SHOP_CAMO`] that is not premium and has a price.
/// Its seller is its folder's ([`camo_shop`]).
pub fn is_shop_camo(item: &Item, params: &ItemParams) -> bool {
    item.category == CAMOUFLAGE
        && params.kind == SHOP_CAMO
        && !item.is_premium
        && item.base_cost > 0
}

/// The faction that sells the camouflage whose config is `cfg_name`: that
/// of its folder ([`CAMO_SHOPS`]), or `None` for a folder no faction has.
/// [`super::tables::check`] refuses data with a shop camouflage of no
/// faction's folder, so none is left out of the shop unseen.
pub fn camo_shop(cfg_name: &str) -> Option<u8> {
    let folder = cfg_name.strip_prefix(CAMO_FOLDER)?.split_once('/')?.0;
    CAMO_SHOPS
        .iter()
        .find(|(name, _)| *name == folder)
        .map(|&(_, faction)| faction)
}

/// One record of the price list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PriceItem {
    /// For one item; for ammo, one clip ([`super::item_params::shop_unit`]).
    pub cost: u32,
    pub dict_id: u16,
    /// The reputation level with `faction_id` that buying needs.
    pub level: u8,
    /// The seller; 0 sells to everyone.
    pub faction_id: u8,
}

impl PriceItem {
    /// The wire size of a record.
    pub const SIZE: usize = 8;

    /// `u32 cost, u16 dict_id, u8 level, u8 faction_id`, little-endian.
    pub fn to_bytes(self) -> [u8; Self::SIZE] {
        let mut bytes = [0; Self::SIZE];
        bytes[..4].copy_from_slice(&self.cost.to_le_bytes());
        bytes[4..6].copy_from_slice(&self.dict_id.to_le_bytes());
        bytes[6] = self.level;
        bytes[7] = self.faction_id;
        bytes
    }
}

/// Where an item is sold when the module's rule does not say it:
/// `(item name, [(seller, reputation level)])`. An ammo's levels are the
/// game's of the time and are not asked for ([`AMMO_NEEDS_RANK`]). Sources
/// are wiki revisions written while 0.26g to 0.26i was live
/// (`progression_sources.md`; `W:<id>` is a revision id of
/// survarium.fandom.com) and the official patch notes (`PN`). PROVEN =
/// stated there and not contradicted by the client's data.
pub const EXCEPTIONS: &[(&str, &[(u8, u8)])] = {
    use faction::{BLACK_MARKET as BM, EDGE, RENAISSANCE as REN, SCAVENGERS as SCAV};
    &[
        // --- Weapons and armour ---
        // 9A-91 and VSK-94: "Fringe Settlers ... Added new weapons: H&K
        // UMP45, 9A-91, VSK-94" (PN 0.25a); ranks 8 and 9 (W:8175, W:9794).
        // Both configs say Renaissance. PROVEN.
        ("a9_91", &[(EDGE, 8)]),
        ("vsk_94", &[(EDGE, 9)]),
        // The chemical protection set and the Amber respirator: rank 2
        // "Determined" (W:8170); item level 3 in the data, and 0.27a "moved
        // to equipment tier 3" what was lower before. PROVEN.
        ("scav_torso_2", &[(SCAV, 2)]),
        ("scav_legs_2", &[(SCAV, 2)]),
        ("scav_gloves_2", &[(SCAV, 2)]),
        ("scav_boots_2", &[(SCAV, 2)]),
        ("scav_resp_1", &[(SCAV, 2)]),
        // Fort 17 and APS: rank 2, "Moved Fort 17 and 9x18 PBM to correct
        // rank" (W:8176, W:8175). PROVEN.
        ("fort_17", &[(REN, 2)]),
        ("aps", &[(EDGE, 2)]),
        // The "Marodeur" container: Black Market, rank 4 (W:8172); its
        // config says Scavengers, item level 3. PROVEN.
        ("bm_back_1", &[(BM, 4)]),
        // The PP-19-01 Vityaz and the MP-27M Lancaster: their configs name
        // no faction, so the rule sells them nowhere. Sold to everyone at
        // no rank, the owner's choice (2026-10-17); the Vityaz at the
        // owner's price ([`OWNER_PRICES`]). Both still drop, and their
        // premium copies stay in bundles 12 and 10.
        ("vityaz", &[(0, 0)]),
        ("izh_27_lancaster", &[(0, 0)]),
        // The STK2 Novice's knit cap (1821): no faction, "it shouldn't
        // belong to any group" (the owner, 2026-10-19). Sold to everyone at
        // no rank, as the two above.
        ("novice_026_helmet", &[(0, 0)]),
        // --- Ammo (W:8170, W:8172, W:8175, W:8176; PROVEN but where
        // said) ---
        ("ammo_12mm_buck", &[(SCAV, 1), (BM, 1)]),
        ("ammo_12mm_slug", &[(SCAV, 3), (BM, 3)]),
        ("ammo_9x18_makarov", &[(SCAV, 2)]),
        ("ammo_9x19p_para", &[(BM, 2)]),
        ("ammo_7.62x51", &[(SCAV, 3)]),
        // 7.62x51 M59.
        ("ammo_7.62x51_ap", &[(SCAV, 5)]),
        ("ammo_.30-30", &[(BM, 3)]),
        ("ammo_.357m", &[(BM, 3)]),
        ("ammo_5.45x39_7n6m", &[(BM, 5)]),
        ("ammo_7.62x39", &[(REN, 1), (EDGE, 1)]),
        ("ammo_9x18_pbm", &[(REN, 2), (EDGE, 2)]),
        // 9x21 SP-10.
        ("ammo_9x21", &[(REN, 3)]),
        ("ammo_45acp", &[(EDGE, 3)]),
        ("ammo_7.62x39_7n23", &[(REN, 4), (EDGE, 4)]),
        // "7.62x39 RPD": LIKELY this item, the only 7.62x39 with a clip of
        // 100 (the RPD's belt).
        ("ammo_7.62x39_m", &[(REN, 4)]),
        // Renaissance: "on sale at the Renaissance Army store on the 7th
        // level of reputation (Captain)" (PN 0.26c, official).
        ("ammo_7.62x54_7n1", &[(REN, 7), (EDGE, 4)]),
        ("ammo_9x19p_7n31", &[(EDGE, 4)]),
        ("ammo_5.45x39_7n10", &[(REN, 5), (EDGE, 5)]),
        ("ammo_pab9", &[(REN, 8), (EDGE, 8)]),
        ("ammo_sp5", &[(REN, 9), (EDGE, 9)]),
        ("ammo_sp6", &[(REN, 9), (EDGE, 9)]),
        ("ammo_5.45x39_7n22", &[(REN, 9)]),
        // 6.5x39 Grendel.
        ("ammo_6.5x39", &[(REN, 10)]),
        ("ammo_12.7x55", &[(EDGE, 10)]),
        // "12.7x54": the list has one name, the data two rounds (PT and
        // VPS); both here [choice].
        ("ammo_12.7x54_pt", &[(EDGE, 10)]),
        ("ammo_12.7x54_vps", &[(EDGE, 10)]),
        // --- Ammo of the weapons from 0.29a3 ([`UNRELEASED`]) [choice,
        // 2026-10-17]: no list of the time has them, and their configs no
        // faction. Sold by the factions whose configs name the weapons
        // that fire it, at the lowest rank the rule sells one of them at
        // (the levels are kept for the table's sake: ammo needs no rank).
        // The FNC, M4A1, HK416 (Black Market) and MSBS Radon (Scavengers).
        ("ammo_5.56x45", &[(SCAV, 5), (BM, 5)]),
        // The MP7 and MP7A1 (Black Market).
        ("ammo_4.6x30", &[(BM, 5)]),
        // The FN P90 (Black Market).
        ("ammo_5.7x28", &[(BM, 5)]),
        // The PKP "Pecheneg" (Renaissance, rank 10).
        ("ammo_7.62x54_7n13", &[(REN, 10)]),
        // 12x70 smallshot: the Vepr-12 (Renaissance) and Saiga-12K (Edge),
        // rank 7 each.
        ("ammo_12mm_buck2", &[(REN, 7), (EDGE, 7)]),
        // --- Drugs, the trap, grenades (the four faction pages and
        // `Items` W:9996; PROVEN) ---
        ("bandages", &[(SCAV, 1)]),
        ("medkit", &[(SCAV, 3)]),
        ("antitoxin", &[(SCAV, 3)]),
        ("base_trap", &[(BM, 1)]),
        // The anesthetic.
        ("painkiller", &[(BM, 3)]),
        ("rgd-5", &[(SCAV, 3), (BM, 3)]),
        // The RDG-M smoke grenade.
        ("rdg-m", &[(REN, 1), (EDGE, 1)]),
    ]
};

/// The owner's prices for a clip of ammo (2026-10-13): `(item name, the
/// game's `base_cost`, srp's price)`. About half the game's, each set by
/// hand and rounded down to a multiple of 5, as the game's own prices are.
/// Every ammo with a price is here; the two the game gives away
/// (7.62x25, 7.62x54R) stay free.
///
/// What the client does with it (`lobby_menu.swf`):
/// - The shop shows this price: its item's cost is the price record's
///   (`GangShop.setupShopItems`: `item.cost = p[i].cost`). PROVEN.
/// - **The kit's ammo window shows the game's own price for a stack the
///   player owns**: an inventory item's cost is the item descriptor's
///   (`InventoryList`: `item_data.cost = descrData.cost`, the exe's
///   `base_cost`), and only `canBuy`, the lock texts and `premiumCost` are
///   taken from the shop's item. PROVEN. So that window shows, and counts
///   what the player can afford by, twice what srp takes; the balance the
///   server sends after is the true one.
/// - Selling pays a quarter of the game's price ([`sell_price`], which the
///   lobby shows from the descriptor), so no price here may be under that
///   quarter: buying and selling back would make silver.
pub const AMMO_PRICES: &[(&str, u32, u32)] = &[
    ("ammo_12mm_buck", 20, 10),
    ("ammo_12mm_slug", 30, 15),
    ("ammo_9x18_makarov", 20, 10),
    ("ammo_9x19p_para", 20, 10),
    ("ammo_.357m", 50, 25),
    ("ammo_.30-30", 50, 25),
    ("ammo_7.62x51", 150, 75),
    ("ammo_7.62x51_ap", 200, 100),
    ("ammo_5.45x39_7n6m", 60, 30),
    ("ammo_9x19_7n21", 50, 25),
    ("ammo_7.62x39", 60, 30),
    ("ammo_9x18_pbm", 50, 25),
    // Half is 32.5.
    ("ammo_9x21", 65, 30),
    ("ammo_45acp", 65, 30),
    ("ammo_7.62x39_m", 150, 75),
    ("ammo_7.62x54_7n1", 220, 110),
    ("ammo_7.62x39_7n23", 80, 40),
    ("ammo_9x19p_7n31", 70, 35),
    // Half is 42.5.
    ("ammo_5.45x39_7n10", 85, 40),
    ("ammo_pab9", 100, 50),
    ("ammo_sp5", 120, 60),
    ("ammo_sp6", 120, 60),
    ("ammo_5.45x39_7n22", 120, 60),
    ("ammo_6.5x39", 140, 70),
    ("ammo_12.7x55", 150, 75),
    ("ammo_12.7x54_pt", 250, 125),
    ("ammo_12.7x54_vps", 300, 150),
    // The ammo of the weapons from 0.29a3 ([`UNRELEASED`]; 2026-10-17, by
    // the same rule, not yet seen by the owner): the game's price is the
    // patch's, its 0.26g peer's a clip.
    ("ammo_5.56x45", 60, 30),
    ("ammo_4.6x30", 70, 35),
    ("ammo_5.7x28", 70, 35),
    // The PKP's belt of 100.
    ("ammo_7.62x54_7n13", 300, 150),
    // Half is 12.5.
    ("ammo_12mm_buck2", 25, 10),
];

/// Items the tables of this module name that come with a game patch not
/// released yet: the weapons from 0.29a3 (dict ids 2201..=2252). Data
/// without them is still srp's ([`super::tables::check`] does not ask for
/// them) and their rows are not used; data with them sells them by the
/// rows. When the patch is released, its names leave this list, and data
/// without them is refused again, as for every other item the code names.
pub const UNRELEASED: &[&str] = &[
    "ammo_5.56x45",
    "ammo_4.6x30",
    "ammo_5.7x28",
    "ammo_7.62x54_7n13",
    "ammo_12mm_buck2",
    // The STK2 Novice's cap (resource-porter pack novice-6).
    "novice_026_helmet",
];

/// Whether buying ammo needs the seller's rank of [`EXCEPTIONS`]. The
/// owner's choice (2026-10-13): no, every ammo is sold to everyone, as the
/// game itself did from 0.27a on. The sellers stay, so the ammo stays in
/// its factions' shop tabs.
///
/// What the client does with a record at level 0 (`lobby_menu.swf`): it is
/// buyable whatever the player's level with the seller
/// (`GangShop.setupShopItems`: `canBuy = _unlockedLevel[seller] >=
/// sellerLevel`, levels start at 0), so the shop shows no lock and the
/// kit's ammo window, which copies `canBuy` from the shop's item, lets the
/// stack grow. PROVEN in the ActionScript.
pub const AMMO_NEEDS_RANK: bool = false;

/// The owner's prices for items other than ammo: `(item name, the game's
/// `base_cost`, srp's price)`.
///
/// - The PP-19-01 Vityaz: 25,000 (2026-10-17), where the game says 12,000.
/// - The Mosin carbine and the PPSh-41: 1,000 each (2026-10-18), sold to
///   everyone at no rank; the game prices neither (`base_cost` 0: they were
///   only the starting weapons, which every new account still gets). Selling
///   one back pays a quarter of the game's 0.
/// - The TT-33: 500 (2026-10-23, the owner's: "it should be there and cost
///   500"), as the Mosin and the PPSh-41: the game prices it at 0 too.
///
/// As with [`AMMO_PRICES`], the shop shows this price (the price record's
/// cost), while what the client shows for an owned item, what selling pays
/// ([`sell_price`]: a quarter of the game's price times the condition, at
/// most 3,000 for the Vityaz, server and client alike) and a repair
/// ([`repair_cost`]) stay the game's: selling back never pays more than the
/// game's quarter.
pub const OWNER_PRICES: &[(&str, u32, u32)] = &[
    ("vityaz", 12000, 25000),
    ("mosin_nagant", 0, 1000),
    ("ppsh", 0, 1000),
    ("tt_33", 0, 500),
];

/// Items no shop sells, whatever the rules above or the gold list
/// (`shop_offers::premium_price_list`) say: `(item name, why)`. The
/// owner's choice of 2026-10-18: no Russian or Belarusian flag or coat of
/// arms is sold, nor the flag of the USSR or the red star. All six are
/// premium decals (`prm_decl`, 300 gold, seller 0), drop nowhere
/// (`drop_weight` 0) and are in no bundle, so they are listed nowhere and a
/// buy of one (`47 00`, silver or gold) is refused as not sold. Selling an
/// owned one back and wearing it are as for any decal: srp takes no copy
/// away (the owner removes them by hand).
///
/// Reviewed 2026-10-18 against stock 0.26g with patches 12..=20: the
/// decals whose name names Russia or Belarus in any of the six
/// localizations, checked against their icons (the Russian tricolour and
/// eagle, the red-green flag and the state emblem of Belarus), and the two
/// Soviet ones by the owner's word. The coat of arms of Lithuania (the
/// Vytis, `decals_5_7`) stays on sale, the owner's word too.
pub const NOT_SOLD: &[(&str, &str)] = &[
    // dict 266, char_decal_4_1: "Flag of USSR" / "Флаг СССР".
    ("decals_4_1", "the flag of the USSR"),
    // dict 267, char_decal_4_2: "Flag of Russia" / "Флаг России".
    ("decals_4_2", "the flag of Russia"),
    // dict 269, char_decal_4_4: "Flag of Belarus" / "Флаг Беларуси".
    ("decals_4_4", "the flag of Belarus"),
    // dict 282, char_decal_5_1: "Red star" / "Красная звезда".
    ("decals_5_1", "the red star (Soviet)"),
    // dict 283, char_decal_5_2: "Coat of arms of Russia" / "Герб России".
    ("decals_5_2", "the coat of arms of Russia"),
    // dict 285, char_decal_5_4: "Coat of arms of Belarus" / "Герб Беларуси".
    ("decals_5_4", "the coat of arms of Belarus"),
];

/// Whether `item` is one of [`NOT_SOLD`].
pub fn is_not_sold(item: &Item) -> bool {
    NOT_SOLD.iter().any(|&(name, _)| name == item.name)
}

/// What the shop takes for a step of `item`: [`AMMO_PRICES`] for ammo,
/// [`OWNER_PRICES`] for what is there, else the game's `base_cost`.
pub fn shop_cost(item: &Item) -> u32 {
    AMMO_PRICES
        .iter()
        .chain(OWNER_PRICES)
        .find(|(name, _, _)| *name == item.name)
        .map_or(item.base_cost, |&(_, _, cost)| cost)
}

/// The reputation level with its faction that buying an item of
/// `item_level` needs (the module's rule), within the faction's levels.
///
/// [open, the owner's to choose; this is the default] The weapons from
/// 0.29a3 give the Scavengers and the Black Market guns of item levels 8
/// to 14 (MSBS Radon, KRISS Vector; Valmet, FNC, M4A1, HK416, MP7, P90),
/// and their ranks end at 5: the rule sells them all at rank 5, beside
/// the factions' own level-5 gear, as it does the Bandit balaclava
/// (level 8, the owner's word of 2026-10-16). The other way would be
/// [`EXCEPTIONS`] rows, or a scale of their levels onto ranks 0 to 5.
pub fn seller_level(dictionary: &Dictionary, faction_id: u8, item_level: u8) -> u8 {
    let level = match faction_id {
        faction::SCAVENGERS | faction::BLACK_MARKET => item_level,
        faction::RENAISSANCE | faction::EDGE => item_level.saturating_sub(5),
        _ => 0,
    };
    level.min(top_level(dictionary, faction_id))
}

/// The last reputation level of a faction.
fn top_level(dictionary: &Dictionary, faction_id: u8) -> u8 {
    dictionary
        .factions
        .iter()
        .find(|entry| entry.id == faction_id)
        .map_or(0, |entry| entry.levels.len().saturating_sub(1)) as u8
}

/// Who sells `item` and at which reputation level: `(seller, level)`, by
/// seller. A shop camouflage by its folder's faction at level 0
/// ([`is_shop_camo`]); scopes by the game data's ([`sellers_of_in`]); else
/// [`EXCEPTIONS`] first, else the module's rule; ammo at level 0
/// ([`AMMO_NEEDS_RANK`]).
pub fn sellers_of(dictionary: &Dictionary, item: &Item, params: &ItemParams) -> Vec<(u8, u8)> {
    sellers_of_in(dictionary, ScopeFits::shared(), item, params)
}

/// [`sellers_of`] with `scopes`: a scope item of a pair with a price is
/// sold by seller 0 at level 0; no other category-22 item is.
pub fn sellers_of_in(
    dictionary: &Dictionary,
    scopes: &ScopeFits,
    item: &Item,
    params: &ItemParams,
) -> Vec<(u8, u8)> {
    if item.is_premium || is_not_sold(item) {
        return Vec::new();
    }
    if is_shop_camo(item, params) {
        return camo_shop(&item.cfg_name).map_or_else(Vec::new, |faction| vec![(faction, 0)]);
    }
    if item.category == SCOPE_CATEGORY {
        return if item.base_cost != 0 && scopes.is_scope(item.dict_id) {
            vec![(0, 0)]
        } else {
            Vec::new()
        };
    }
    let gear = GEAR.contains(&item.category);
    let supply = SUPPLIES.contains(&item.category) && item.is_stack;
    if item.base_cost == 0 {
        // A free supply (the starting weapons' ammo) is listed at no cost by
        // seller 0, who sells to everyone: the client lets a player add to a
        // stack only when the price list has the item (`canBuy`, set by
        // `GangShop.setupShopItems`), and for a price of 0 it allows up to
        // `MAX_FREE_ITEMS_COUNT` (1000) units (`AmmoShareElement.updateStorage`).
        // Without the record the ammo window's storage field is locked (seen
        // in the first live session, 2026-10-12). A free weapon is listed only
        // at an owner's price ([`OWNER_PRICES`]: the starting Mosin,
        // PPSh-41 and TT-33), by seller 0 at no rank.
        let owners = OWNER_PRICES.iter().any(|(name, _, _)| *name == item.name);
        return if supply || (gear && owners) {
            vec![(0, 0)]
        } else {
            Vec::new()
        };
    }
    let exception = EXCEPTIONS.iter().find(|(name, _)| *name == item.name);
    match exception {
        Some(&(_, sellers)) if gear || supply => {
            let ranked = AMMO_NEEDS_RANK || params.kind != "ammo";
            let mut sellers: Vec<(u8, u8)> = sellers
                .iter()
                .map(|&(seller, level)| {
                    let level = if ranked { level } else { 0 };
                    (seller, level.min(top_level(dictionary, seller)))
                })
                .collect();
            sellers.sort_unstable();
            sellers
        }
        _ if gear && params.faction != 0 => {
            let level = seller_level(dictionary, params.faction, params.item_level);
            vec![(params.faction, level)]
        }
        _ if supply => vec![(0, 0)],
        _ => Vec::new(),
    }
}

fn listed<'a>(
    dictionary: &'a Dictionary,
    item: &'a Item,
    params: &ItemParams,
) -> impl Iterator<Item = PriceItem> + 'a {
    let (cost, dict_id) = (shop_cost(item), item.dict_id);
    sellers_of(dictionary, item, params)
        .into_iter()
        .map(move |(faction_id, level)| PriceItem {
            cost,
            dict_id,
            level,
            faction_id,
        })
}

/// The price list of `dictionary` with its items' parameters `params` (by
/// `dict_id`, [`super::item_params::derive`]), by `dict_id` and then by
/// seller.
pub fn price_list_of(dictionary: &Dictionary, params: &[ItemParams]) -> Vec<PriceItem> {
    dictionary
        .items
        .iter()
        .filter_map(|item| {
            let index = params
                .binary_search_by_key(&item.dict_id, |row| row.dict_id)
                .ok()?;
            Some(listed(dictionary, item, &params[index]))
        })
        .flatten()
        .collect()
}

/// The price list the lobby sends, by `dict_id` and then by seller: an
/// item two factions sell has two records.
pub fn price_list() -> &'static [PriceItem] {
    static SHARED: std::sync::OnceLock<Vec<PriceItem>> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| price_list_of(Dictionary::shared(), super::item_params::all()))
}

/// The price list as the 0x8A body unpacks to: `n × 8` bytes.
pub fn price_list_bytes() -> Vec<u8> {
    price_list()
        .iter()
        .flat_map(|price| price.to_bytes())
        .collect()
}

/// The shop's records for `dict_id`, one a seller; empty if it is not sold.
/// Every record of an item has the same cost.
pub fn sellers(dict_id: u16) -> &'static [PriceItem] {
    let list = price_list();
    let start = list.partition_point(|price| price.dict_id < dict_id);
    let end = list.partition_point(|price| price.dict_id <= dict_id);
    &list[start..end]
}

/// The shop's first record for `dict_id`, if it is sold: its cost is the
/// item's whoever sells it. Who may buy is [`record_for`]'s to say.
pub fn price(dict_id: u16) -> Option<&'static PriceItem> {
    sellers(dict_id).first()
}

/// The record an account buys `dict_id` by: of the sellers that sell to it
/// (`reputation_level(faction)` is its level with a faction; seller 0
/// sells to everyone) the one named `preferred` if it is among them, else
/// the first. `Err` is the record that refused: `preferred`'s, else the
/// first; `Ok(None)` when the item is not sold.
///
/// [choice] A buy naming a seller whose rank the account lacks goes through
/// another seller of the same item whose rank it has: the cost is the same,
/// and the client itself names only the first record's seller when its
/// window names none (0x288BC0).
pub fn record_for(
    dict_id: u16,
    preferred: u8,
    reputation_level: impl Fn(u8) -> u8,
) -> Result<Option<&'static PriceItem>, &'static PriceItem> {
    let records = sellers(dict_id);
    let Some(first) = records.first() else {
        return Ok(None);
    };
    let may = |record: &&PriceItem| {
        record.faction_id == 0 || reputation_level(record.faction_id) >= record.level
    };
    let named = records.iter().find(|record| record.faction_id == preferred);
    match named.filter(may).or_else(|| records.iter().find(may)) {
        Some(record) => Ok(Some(record)),
        None => Err(named.unwrap_or(first)),
    }
}

/// What buying `steps` of `dict_id` costs, or `None` if it is not sold.
/// A step is the shop's unit ([`super::item_params::shop_unit`]): a clip of
/// ammo, else one item. The buy dialog sends its count in steps
/// (`MessageAsk`: `count = slider / _step`; PROVEN, `lobby_menu.swf`), so
/// the buyer gets `steps * shop_unit` units.
pub fn buy_cost(dict_id: u16, steps: u32) -> Option<u64> {
    price(dict_id).map(|price| u64::from(price.cost) * u64::from(steps))
}

/// An item's `base_cost` in generic money: a premium item's gold times
/// `premium_exchange_rate` (the rate the server gave the lobby).
fn cost_in_generic_money(item: &Item, premium_exchange_rate: u32) -> u64 {
    let cost = u64::from(item.base_cost);
    if item.is_premium {
        cost * u64::from(premium_exchange_rate)
    } else {
        cost
    }
}

/// What the lobby shows for selling `dict_id`: a quarter of its cost, for a
/// stack a step (`count` units are `count / shop_unit` steps), else times
/// its condition (0..=100).
pub fn sell_price(
    dictionary: &Dictionary,
    dict_id: u16,
    condition_or_count: u32,
    premium_exchange_rate: u32,
) -> u64 {
    let Some(item) = dictionary.item(dict_id) else {
        return 0;
    };
    let quarter = cost_in_generic_money(item, premium_exchange_rate) / 4;
    if item.is_stack {
        quarter * u64::from(condition_or_count / super::item_params::shop_unit(dict_id))
    } else {
        quarter * u64::from(condition_or_count.min(100)) / 100
    }
}

/// What the lobby shows for repairing `dict_id` from `condition` (0..=100)
/// to full, without its defects' fix costs.
pub fn repair_cost(dictionary: &Dictionary, dict_id: u16, condition: u32) -> u32 {
    let (Some(item), Some(params)) = (dictionary.item(dict_id), item_params(dict_id)) else {
        return 0;
    };
    let worn = 1.0 - condition.min(100) as f32 / 100.0;
    (worn * (params.repair_cost_ratio * item.base_cost as f32)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_is_eight_bytes() {
        let record = PriceItem {
            cost: 47500,
            dict_id: 3,
            level: 5,
            faction_id: 3,
        };
        assert_eq!(record.to_bytes(), [0x8C, 0xB9, 0, 0, 3, 0, 5, 3]);
        assert_eq!(price_list_bytes().len(), price_list().len() * 8);
        assert_eq!(price_list_bytes()[..8], price_list()[0].to_bytes());
    }

    #[test]
    fn ammo_costs_the_owners_prices() {
        let dictionary = Dictionary::shared();
        // Every ammo the game prices is in the table, with the game's
        // price as the table says it, and no other item is.
        let priced: Vec<&str> = dictionary
            .items
            .iter()
            .filter(|item| {
                item.base_cost > 0
                    && !item.is_premium
                    && item_params(item.dict_id).is_some_and(|params| params.kind == "ammo")
            })
            .map(|item| item.name.as_str())
            .collect();
        // A row of an item the data lacks is of a patch not released.
        let rows: Vec<&(&str, u32, u32)> = AMMO_PRICES
            .iter()
            .filter(|(name, _, _)| dictionary.item_named(name).is_some())
            .collect();
        assert!(
            AMMO_PRICES
                .iter()
                .all(|row| rows.contains(&row) || UNRELEASED.contains(&row.0))
        );
        assert_eq!(priced.len(), rows.len());
        for &&(name, game, cost) in &rows {
            assert!(priced.contains(&name), "{name}");
            let item = dictionary.item_named(name).unwrap();
            assert_eq!(item.base_cost, game, "{name}");
            assert_eq!(shop_cost(item), cost, "{name}");
            // About half, never over it, in the game's own steps of 5.
            assert!(cost * 2 <= game && cost * 5 >= game * 2, "{name}");
            assert_eq!(cost % 5, 0, "{name}");
            // Selling a clip back pays a quarter of the game's price
            // (`sell_price`): never more than the clip cost.
            let clip = super::super::item_params::shop_unit(item.dict_id);
            let sold = sell_price(dictionary, item.dict_id, clip, 100);
            assert!(sold <= u64::from(cost), "{name}: {sold} for {cost}");
            assert_eq!(
                buy_cost(item.dict_id, 3),
                Some(u64::from(cost) * 3),
                "{name}"
            );
        }
        // The 5.45x39 7N6M: 30 a clip, where the game says 60.
        let rounds = dictionary.item_named("ammo_5.45x39_7n6m").unwrap();
        assert_eq!(price(rounds.dict_id).unwrap().cost, 30);
        // Anything else is the game's.
        let medkit = dictionary.item_named("medkit").unwrap();
        assert_eq!(shop_cost(medkit), medkit.base_cost);
    }

    #[test]
    fn prices_are_the_dictionarys_base_costs() {
        let dictionary = Dictionary::shared();
        let list = price_list();
        // By item, then by seller; no record twice.
        assert!(
            list.windows(2)
                .all(|w| { (w[0].dict_id, w[0].faction_id) < (w[1].dict_id, w[1].faction_id) })
        );
        for record in list {
            let item = dictionary.item(record.dict_id).unwrap();
            let ammo = item_params(record.dict_id).is_some_and(|params| params.kind == "ammo");
            let owners = OWNER_PRICES.iter().any(|(name, _, _)| *name == item.name);
            if ammo && item.base_cost > 0 {
                assert_eq!(record.cost, shop_cost(item), "{}", item.name);
                assert!(record.cost < item.base_cost, "{}", item.name);
            } else if owners {
                assert_eq!(record.cost, shop_cost(item), "{}", item.name);
            } else {
                assert_eq!(record.cost, item.base_cost, "{}", item.name);
            }
            assert!(!item.is_premium, "{}", item.name);
            // Only a stack is listed for nothing (the starting ammo).
            assert!(
                record.cost > 0 || (item.is_stack && record.faction_id == 0),
                "{}",
                item.name
            );
            assert!(record.faction_id <= 4, "{}", item.name);
            // The config's faction sells its gear, but for the exceptions.
            let params = item_params(record.dict_id).unwrap();
            let excepted = EXCEPTIONS.iter().any(|(name, _)| *name == item.name);
            if GEAR.contains(&item.category) && !excepted {
                assert_eq!(record.faction_id, params.faction, "{}", item.name);
                assert_eq!(
                    record.level,
                    seller_level(dictionary, params.faction, params.item_level),
                    "{}",
                    item.name
                );
            }
        }
        // The AK-74M: Renaissance, item level 10.
        let ak = price(3).unwrap();
        assert_eq!((ak.cost, ak.faction_id, ak.level), (47500, 3, 5));
        // The AKS-74U: Black Market's last level.
        let aks = price(13).unwrap();
        assert_eq!((aks.cost, aks.faction_id, aks.level), (12920, 2, 5));
        // The ranks of the January 2015 list.
        let rank = |name: &str| {
            let record = price(dictionary.item_named(name).unwrap().dict_id).unwrap();
            (record.faction_id, record.level)
        };
        assert_eq!(rank("toz_34"), (1, 1));
        assert_eq!(rank("kiparis"), (1, 2));
        assert_eq!(rank("toz_122"), (1, 3));
        assert_eq!(rank("mp_153"), (1, 4));
        assert_eq!(rank("rem_700"), (1, 5));
        assert_eq!(rank("toz_66"), (2, 1));
        assert_eq!(rank("uzi"), (2, 2));
        assert_eq!(rank("akmn"), (3, 1));
        assert_eq!(rank("spas_12"), (3, 2));
        assert_eq!(rank("sks"), (4, 1));
        assert_eq!(rank("vssk"), (4, 10));
        // No level is past its faction's last.
        for record in list.iter().filter(|record| record.faction_id != 0) {
            let levels = dictionary.factions[usize::from(record.faction_id) - 1]
                .levels
                .len();
            assert!(usize::from(record.level) < levels, "{}", record.dict_id);
        }
    }

    /// `(seller, level)` of every record of an item.
    fn sold_by(name: &str) -> Vec<(u8, u8)> {
        let dict_id = Dictionary::shared().item_named(name).unwrap().dict_id;
        sellers(dict_id)
            .iter()
            .map(|record| (record.faction_id, record.level))
            .collect()
    }

    #[test]
    fn the_exceptions_are_items_of_the_dictionary_sold_as_listed() {
        let dictionary = Dictionary::shared();
        for (index, &(name, listed)) in EXCEPTIONS.iter().enumerate() {
            let Some(item) = dictionary.item_named(name) else {
                assert!(UNRELEASED.contains(&name), "no item `{name}`");
                continue;
            };
            assert!(item.base_cost > 0 && !item.is_premium, "{name}");
            assert!(
                GEAR.contains(&item.category) || SUPPLIES.contains(&item.category),
                "{name}"
            );
            assert!(!listed.is_empty() && listed.len() <= 2, "{name}");
            // Once in the table, and each seller once.
            assert!(
                EXCEPTIONS[..index].iter().all(|(other, _)| *other != name),
                "{name}"
            );
            let mut expected = listed.to_vec();
            expected.sort_unstable();
            expected.dedup_by_key(|(seller, _)| *seller);
            assert_eq!(expected.len(), listed.len(), "{name}");
            // A faction at a rank, or seller 0 (everyone) at none.
            assert!(
                expected.iter().all(|&(seller, level)| {
                    ((1..=4).contains(&seller) && level > 0) || (seller, level) == (0, 0)
                }),
                "{name}"
            );
            // Ammo: the listed sellers, at no rank.
            if !AMMO_NEEDS_RANK && item_params(item.dict_id).unwrap().kind == "ammo" {
                expected.iter_mut().for_each(|(_, level)| *level = 0);
            }
            assert_eq!(sold_by(name), expected, "{name}");
        }
        // The names the sources use: the 9A-91 and VSK-94 are dict ids 303
        // and 305.
        assert_eq!(dictionary.item_named("a9_91").unwrap().dict_id, 303);
        assert_eq!(dictionary.item_named("vsk_94").unwrap().dict_id, 305);
    }

    #[test]
    fn the_bandit_balaclava_goes_by_the_rule() {
        // No exception (owner, 2026-10-16): Black Market at its item level,
        // within the faction's ranks 0 to 5. The test patch 15 makes it item
        // level 8, which the rule puts at Black Market's last rank.
        let dictionary = Dictionary::shared();
        assert_eq!(top_level(dictionary, faction::BLACK_MARKET), 5);
        assert_eq!(seller_level(dictionary, faction::BLACK_MARKET, 8), 5);
        let item = dictionary.item_named("bandit_026_mask").unwrap();
        let params = item_params(item.dict_id).unwrap();
        assert_eq!(params.faction, faction::BLACK_MARKET);
        assert_eq!(params.item_level, 8);
        assert_eq!(sold_by("bandit_026_mask"), [(faction::BLACK_MARKET, 5)]);
        assert_eq!(
            sold_by("bandit_026_mask"),
            [(
                faction::BLACK_MARKET,
                seller_level(dictionary, faction::BLACK_MARKET, params.item_level)
            )]
        );
    }

    #[test]
    fn gear_exceptions_move_the_seller_or_the_rank() {
        // Edge sells the 9A-91 and VSK-94, whose configs say Renaissance.
        assert_eq!(sold_by("a9_91"), [(4, 8)]);
        assert_eq!(sold_by("vsk_94"), [(4, 9)]);
        // The chemical set and the respirator: rank 2, item level 3.
        for name in [
            "scav_torso_2",
            "scav_legs_2",
            "scav_gloves_2",
            "scav_boots_2",
            "scav_resp_1",
        ] {
            assert_eq!(sold_by(name), [(1, 2)], "{name}");
        }
        assert_eq!(sold_by("fort_17"), [(3, 2)]);
        assert_eq!(sold_by("aps"), [(4, 2)]);
        // No exception for the Duty exoskeleton either: the rule puts an
        // item of Renaissance's at its level 15 at rank 10.
        assert_eq!(
            seller_level(Dictionary::shared(), faction::RENAISSANCE, 15),
            10
        );
        assert_eq!(sold_by("bm_back_1"), [(2, 4)]);
        // Their neighbours go by the rule.
        assert_eq!(sold_by("scav_torso_1"), [(1, 1)]);
    }

    #[test]
    fn supplies_are_sold_by_the_factions_at_ranks() {
        // Two sellers: two records, by seller. Ammo needs no rank.
        assert_eq!(sold_by("ammo_12mm_buck"), [(1, 0), (2, 0)]);
        assert_eq!(sold_by("ammo_12mm_slug"), [(1, 0), (2, 0)]);
        assert_eq!(sold_by("ammo_7.62x39"), [(3, 0), (4, 0)]);
        assert_eq!(sold_by("ammo_7.62x54_7n1"), [(3, 0), (4, 0)]);
        assert_eq!(sold_by("ammo_sp6"), [(3, 0), (4, 0)]);
        assert_eq!(sold_by("rgd-5"), [(1, 3), (2, 3)]);
        assert_eq!(sold_by("rdg-m"), [(3, 1), (4, 1)]);
        // One seller.
        assert_eq!(sold_by("ammo_5.45x39_7n6m"), [(2, 0)]);
        assert_eq!(sold_by("ammo_5.45x39_7n22"), [(3, 0)]);
        assert_eq!(sold_by("ammo_6.5x39"), [(3, 0)]);
        assert_eq!(sold_by("ammo_12.7x55"), [(4, 0)]);
        assert_eq!(sold_by("bandages"), [(1, 1)]);
        assert_eq!(sold_by("medkit"), [(1, 3)]);
        assert_eq!(sold_by("antitoxin"), [(1, 3)]);
        assert_eq!(sold_by("base_trap"), [(2, 1)]);
        assert_eq!(sold_by("painkiller"), [(2, 3)]);
        // On no list: the 9x19 7N21 stays with seller 0, as the boosters.
        assert_eq!(sold_by("ammo_9x19_7n21"), [(0, 0)]);
        assert_eq!(sold_by("speed_booster"), [(0, 0)]);
        // Every other ammo, drug, trap and grenade has a faction.
        let dictionary = Dictionary::shared();
        let unsold: Vec<&str> = price_list()
            .iter()
            .filter(|record| record.faction_id == 0)
            .map(|record| dictionary.item(record.dict_id).unwrap())
            .filter(|item| SUPPLIES.contains(&item.category))
            .map(|item| item.name.as_str())
            .filter(|name| !name.ends_with("_booster"))
            .collect();
        // With seller 0: the one ammo on no faction's list, and the two
        // free ones (the starting weapons').
        assert_eq!(
            unsold,
            ["ammo_7.62x25", "ammo_9x19_7n21", "ammo_7.62x54r_old"]
        );
        // The free ammo is listed by seller 0 at no cost and no rank.
        assert_eq!(sold_by("ammo_7.62x25"), [(0, 0)]);
        assert_eq!(sold_by("ammo_7.62x54r_old"), [(0, 0)]);
    }

    #[test]
    fn every_ammo_is_sold_to_everyone() {
        let dictionary = Dictionary::shared();
        let ammo: Vec<&Item> = dictionary
            .items
            .iter()
            .filter(|item| item_params(item.dict_id).is_some_and(|params| params.kind == "ammo"))
            .collect();
        assert_eq!(ammo.len(), 29);
        // Known to nobody.
        let nobody = |_: u8| 0;
        for item in ammo {
            let records = sellers(item.dict_id);
            assert!(!records.is_empty(), "{}", item.name);
            assert!(records.iter().all(|r| r.level == 0), "{}", item.name);
            // Whichever seller the client names.
            for seller in 0..=4 {
                assert!(
                    matches!(record_for(item.dict_id, seller, nobody), Ok(Some(_))),
                    "{} from {seller}",
                    item.name
                );
            }
        }
        // Nothing else lost its rank: of the records with a faction, only
        // ammo and the shop camouflages are at level 0.
        for record in price_list().iter().filter(|r| r.faction_id != 0) {
            let kind = item_params(record.dict_id).unwrap().kind;
            let unranked = kind == "ammo" || kind == SHOP_CAMO;
            assert_eq!(record.level == 0, unranked, "{}", record.dict_id);
        }
    }

    #[test]
    fn a_buyer_gets_the_record_of_a_seller_that_sells_to_it() {
        let id = |name: &str| Dictionary::shared().item_named(name).unwrap().dict_id;
        let seller = |result: Result<Option<&'static PriceItem>, &'static PriceItem>| {
            result.map(|record| record.map(|record| record.faction_id))
        };
        // The RGD-5: the Scavengers and the Black Market, level 3 each.
        let grenade = id("rgd-5");
        // Known to nobody: the first record refuses, or the one named.
        let nobody = |_: u8| 0;
        assert_eq!(record_for(grenade, 0, nobody).unwrap_err().faction_id, 1);
        assert_eq!(record_for(grenade, 2, nobody).unwrap_err().faction_id, 2);
        // Known to the Black Market only: it sells, whoever was named.
        let black_market = |faction: u8| if faction == 2 { 3 } else { 0 };
        assert_eq!(seller(record_for(grenade, 0, black_market)), Ok(Some(2)));
        assert_eq!(seller(record_for(grenade, 1, black_market)), Ok(Some(2)));
        assert_eq!(seller(record_for(grenade, 2, black_market)), Ok(Some(2)));
        // Known to both: the one named, else the first.
        let both = |_: u8| 5;
        assert_eq!(seller(record_for(grenade, 2, both)), Ok(Some(2)));
        assert_eq!(seller(record_for(grenade, 0, both)), Ok(Some(1)));
        // Seller 0 sells to everyone; what is not sold has no record.
        assert_eq!(
            seller(record_for(id("speed_booster"), 0, nobody)),
            Ok(Some(0))
        );
        assert_eq!(seller(record_for(id("tt_33"), 0, nobody)), Ok(Some(0)));
        assert_eq!(seller(record_for(id("vss"), 0, both)), Ok(None));
    }

    #[test]
    fn ammo_is_priced_a_clip() {
        // 5.45x39 7N6M: 30 for a clip of 30 (the game's 60, `AMMO_PRICES`).
        let ammo = price(7).unwrap();
        assert_eq!((ammo.cost, ammo.faction_id, ammo.level), (30, 2, 0));
        assert_eq!(buy_cost(7, 1), Some(30));
        assert_eq!(buy_cost(7, 3), Some(90));
        // A medkit is 20, one at a time.
        assert_eq!(buy_cost(67, 3), Some(60));
        assert_eq!(buy_cost(3, 1), Some(47500));
        // Both records of an item cost the same.
        for records in price_list().chunk_by(|a, b| a.dict_id == b.dict_id) {
            assert!(records.iter().all(|record| record.cost == records[0].cost));
        }
    }

    #[test]
    fn what_is_not_sold() {
        let dictionary = Dictionary::shared();
        let sold = |name: &str| price(dictionary.item_named(name).unwrap().dict_id).is_some();
        // The starting weapons are sold at the owner's price
        // (`the_starting_weapons_are_sold_to_everyone`); their ammo is listed, for nothing.
        assert_eq!(
            price(dictionary.item_named("ammo_7.62x25").unwrap().dict_id).map(|r| r.cost),
            Some(0)
        );
        // Premium and bundle weapons, the weapons with no faction but the
        // Vityaz and the Lancaster, scopes, artefacts, money.
        assert!(!sold("ak_74n") && !sold("uzi_premium") && !sold("vityaz_premium"));
        assert!(!sold("vss") && !sold("sr3m") && !sold("socom16"));
        assert!(!sold("scope_leupold") && !sold("artefact_onyx"));
        assert!(!sold("generic_money") && !sold("premium_money"));
        // No decals, vouchers, drop camouflages or the free New Year one:
        // of the cosmetics, only the shop camouflages.
        for record in price_list() {
            let item = dictionary.item(record.dict_id).unwrap();
            if item.category >= 25 {
                assert!(
                    is_shop_camo(item, item_params(item.dict_id).unwrap()),
                    "{}",
                    item.name
                );
            }
        }
        assert!(!sold("camo_black_leather_01") && !sold("camo_khaki_fabric_03"));
        assert!(!sold("camo_new_year"));
        assert_eq!(buy_cost(214, 1), None);
    }

    /// [`NOT_SOLD`]: the six decals, as reviewed (2026-10-18), have no
    /// seller even as a non-premium copy, and selling one back still pays.
    #[test]
    fn the_excluded_flags_and_emblems_are_not_sold() {
        let dictionary = Dictionary::shared();
        let reviewed: Vec<(u16, &str)> = NOT_SOLD
            .iter()
            .map(|&(name, _)| {
                let item = dictionary.item_named(name).unwrap();
                (item.dict_id, item.cfg_name.as_str())
            })
            .collect();
        assert_eq!(
            reviewed,
            [
                (266, "gameplay/items/upgrades/decals/char_decal_4_1.options"),
                (267, "gameplay/items/upgrades/decals/char_decal_4_2.options"),
                (269, "gameplay/items/upgrades/decals/char_decal_4_4.options"),
                (282, "gameplay/items/upgrades/decals/char_decal_5_1.options"),
                (283, "gameplay/items/upgrades/decals/char_decal_5_2.options"),
                (285, "gameplay/items/upgrades/decals/char_decal_5_4.options"),
            ]
        );
        for &(dict_id, _) in &reviewed {
            let item = dictionary.item(dict_id).unwrap();
            let params = item_params(dict_id).unwrap();
            assert!(item.is_premium && item.base_cost == 300 && params.kind == "prm_decl");
            assert!(is_not_sold(item));
            assert!(sellers(dict_id).is_empty() && buy_cost(dict_id, 1).is_none());
            // Were it no premium item, still no seller.
            let silver = Item {
                is_premium: false,
                ..item.clone()
            };
            assert_eq!(sellers_of(dictionary, &silver, params), []);
            // An owned one sells back for a quarter of its 300 gold.
            assert_eq!(sell_price(dictionary, dict_id, 100, 100), 7_500);
        }
        // The other flags and coats of arms are not in it, the coat of arms
        // of Lithuania (the Vytis) included.
        for name in ["decals_4_3", "decals_5_7"] {
            assert!(!is_not_sold(dictionary.item_named(name).unwrap()), "{name}");
        }
    }

    #[test]
    fn selling_gives_a_quarter_and_repair_goes_by_the_wear() {
        let dictionary = Dictionary::shared();
        // The AK-74M at full condition and at 40.
        assert_eq!(sell_price(dictionary, 3, 100, 100), 11875);
        assert_eq!(sell_price(dictionary, 3, 40, 100), 4750);
        // Two clips of 5.45: 15 a clip.
        assert_eq!(sell_price(dictionary, 7, 60, 100), 30);
        // A premium item's cost is gold: the Uzi, 500 gold at 100 a gold.
        let uzi = dictionary.item_named("uzi_premium").unwrap().dict_id;
        assert_eq!(sell_price(dictionary, uzi, 100, 100), 12500);
        // Repair: ratio 0.1 of 47500 for the whole, a half for a half.
        assert_eq!(repair_cost(dictionary, 3, 0), 4750);
        assert_eq!(repair_cost(dictionary, 3, 50), 2375);
        assert_eq!(repair_cost(dictionary, 3, 100), 0);
        assert_eq!(repair_cost(dictionary, 7, 0), 0);
    }

    /// The owner's choice: the starting Mosin carbine and PPSh-41 (2026-10-18,
    /// 1,000 each) and the TT-33 (2026-10-23, 500), which the game prices at
    /// 0, are sold to everyone at no rank; selling one back pays the game's
    /// quarter of 0.
    #[test]
    fn the_starting_weapons_are_sold_to_everyone() {
        let dictionary = Dictionary::shared();
        for (dict_id, name, cost) in STARTERS_SOLD {
            let item = dictionary.item_named(name).unwrap();
            assert_eq!((item.dict_id, item.base_cost), (dict_id, 0), "{name}");
            let records: Vec<_> = price_list()
                .iter()
                .filter(|record| record.dict_id == dict_id)
                .map(|record| (record.faction_id, record.level, record.cost))
                .collect();
            assert_eq!(records, [(0, 0, cost)], "{name}");
            assert_eq!(buy_cost(dict_id, 1), Some(u64::from(cost)), "{name}");
            assert_eq!(sell_price(dictionary, dict_id, 100, 100), 0, "{name}");
        }
    }

    /// The starting weapons the shop sells, by `dict_id`, at the owner's
    /// price: every weapon a new account owns (the Mosin and the PPSh-41 since
    /// 2026-10-18, the TT-33 since 2026-10-23).
    const STARTERS_SOLD: [(u16, &str, u32); 3] = [
        (18, "tt_33", 500),
        (106, "ppsh", 1000),
        (107, "mosin_nagant", 1000),
    ];

    /// The owner's choice (2026-10-17) for what no rule sold: the faction
    /// camouflages and two weapons with no faction.
    const NEWLY_SOLD: [(&str, u16, u8, u32); 10] = [
        // Black Market.
        ("camo_black_leather", 154, faction::BLACK_MARKET, 8000),
        ("camo_brown_leather_05", 162, faction::BLACK_MARKET, 3000),
        ("camo_denim", 163, faction::BLACK_MARKET, 1000),
        // Renaissance.
        ("camo_camouflage_green", 169, faction::RENAISSANCE, 10000),
        ("camo_camouflage_green_01", 170, faction::RENAISSANCE, 10000),
        // Scavengers.
        ("camo_fabric_grey", 184, faction::SCAVENGERS, 8000),
        ("camo_quilted_fabric", 185, faction::SCAVENGERS, 1000),
        ("camo_rubber_green_01", 187, faction::SCAVENGERS, 3000),
        // Everyone: the Vityaz at the owner's price, the Lancaster at the
        // game's.
        ("vityaz", 19, 0, 25000),
        ("izh_27_lancaster", 108, 0, 2500),
    ];

    /// `(dict_id, seller, level, cost)` of each record of `list` for the
    /// items [`NEWLY_SOLD`] names, by `dict_id`.
    fn newly_sold_records(dictionary: &Dictionary, list: &[PriceItem]) -> Vec<(u16, u8, u8, u32)> {
        list.iter()
            .filter(|record| {
                let name = &dictionary.item(record.dict_id).unwrap().name;
                NEWLY_SOLD.iter().any(|(sold, ..)| sold == name)
            })
            .map(|record| (record.dict_id, record.faction_id, record.level, record.cost))
            .collect()
    }

    fn expected_newly_sold() -> Vec<(u16, u8, u8, u32)> {
        let mut expected: Vec<(u16, u8, u8, u32)> = NEWLY_SOLD
            .iter()
            .map(|&(_, dict_id, seller, cost)| (dict_id, seller, 0, cost))
            .collect();
        expected.sort_unstable();
        expected
    }

    #[test]
    fn the_factions_camouflages_and_two_weapons_are_sold_at_no_rank() {
        let dictionary = Dictionary::shared();
        for (name, dict_id, ..) in NEWLY_SOLD {
            assert_eq!(
                dictionary.item_named(name).unwrap().dict_id,
                dict_id,
                "{name}"
            );
        }
        assert_eq!(
            newly_sold_records(dictionary, price_list()),
            expected_newly_sold()
        );
        // Anyone buys them, known to no faction.
        let nobody = |_: u8| 0;
        for (name, dict_id, seller, cost) in NEWLY_SOLD {
            let record = record_for(dict_id, seller, nobody).unwrap().unwrap();
            assert_eq!((record.faction_id, record.cost), (seller, cost), "{name}");
            assert_eq!(buy_cost(dict_id, 2), Some(u64::from(cost) * 2), "{name}");
        }
        // The shop camouflages are the data's stk_camo items with a price,
        // each sold by its folder's faction at its `base_cost`.
        let camos: Vec<&str> = dictionary
            .items
            .iter()
            .filter(|item| is_shop_camo(item, item_params(item.dict_id).unwrap()))
            .map(|item| item.name.as_str())
            .collect();
        assert_eq!(
            camos,
            NEWLY_SOLD[..8]
                .iter()
                .map(|(name, ..)| *name)
                .collect::<Vec<_>>()
        );
        for (name, dict_id, _, cost) in &NEWLY_SOLD[..8] {
            let item = dictionary.item(*dict_id).unwrap();
            assert_eq!(item.base_cost, *cost, "{name}");
            assert_eq!(item_params(*dict_id).unwrap().item_level, 1, "{name}");
        }
        // The Lancaster at the game's price; the Vityaz at the owner's.
        assert_eq!(
            dictionary.item_named("izh_27_lancaster").unwrap().base_cost,
            2500
        );
        let vityaz = dictionary.item_named("vityaz").unwrap();
        assert_eq!((vityaz.base_cost, shop_cost(vityaz)), (12000, 25000));
        // Their premium copies stay out of the silver list.
        assert!(price(dictionary.item_named("vityaz_premium").unwrap().dict_id).is_none());
        let lancaster = dictionary.item_named("izh_27_lancaster_premium").unwrap();
        assert!(price(lancaster.dict_id).is_none());
    }

    #[test]
    fn selling_the_vityaz_back_pays_a_quarter_of_the_games_price() {
        // The shop takes 25,000; selling pays a quarter of the game's
        // 12,000 times the condition, as the lobby shows it from the item's
        // descriptor, and a repair goes by the game's price too.
        let dictionary = Dictionary::shared();
        let vityaz = dictionary.item_named("vityaz").unwrap().dict_id;
        assert_eq!(buy_cost(vityaz, 1), Some(25000));
        assert_eq!(sell_price(dictionary, vityaz, 100, 100), 3000);
        assert_eq!(sell_price(dictionary, vityaz, 50, 100), 1500);
        let ratio = item_params(vityaz).unwrap().repair_cost_ratio;
        assert_eq!(repair_cost(dictionary, vityaz, 0), (ratio * 12000.0) as u32);
        // No owner's price is under the game's quarter: buying and selling
        // back never makes silver.
        for &(name, game, cost) in OWNER_PRICES {
            let item = dictionary.item_named(name).unwrap();
            assert_eq!(item.base_cost, game, "{name}");
            assert_eq!(shop_cost(item), cost, "{name}");
            assert!(u64::from(cost) >= sell_price(dictionary, item.dict_id, 100, 100));
            assert!(
                !AMMO_PRICES.iter().any(|(ammo, ..)| *ammo == name),
                "{name}"
            );
        }
    }

    #[test]
    fn a_shop_camouflages_seller_is_its_folders() {
        let at = |folder: &str| camo_shop(&format!("{CAMO_FOLDER}{folder}/camo_x.options"));
        assert_eq!(at("scavengers"), Some(faction::SCAVENGERS));
        assert_eq!(at("black_market"), Some(faction::BLACK_MARKET));
        assert_eq!(at("renesance"), Some(faction::RENAISSANCE));
        assert_eq!(at("premium"), None);
        assert_eq!(at("edge"), None);
        assert_eq!(
            camo_shop("gameplay/items/upgrades/camouflages/camo_x.options"),
            None
        );
        assert_eq!(
            camo_shop("gameplay/items/weapons/scavengers/x.options"),
            None
        );
        // Of a faction's folder, only the shop camouflage is sold: a drop
        // camouflage (`base_cost` 1), a premium one and one for nothing are
        // not, and the shop camouflage is sold at no rank for its price.
        let dictionary = Dictionary::shared();
        let denim = dictionary.item_named("camo_denim").unwrap();
        let params = *item_params(denim.dict_id).unwrap();
        assert_eq!(params.kind, SHOP_CAMO);
        assert_eq!(
            sellers_of(dictionary, denim, &params),
            [(faction::BLACK_MARKET, 0)]
        );
        let as_kind = |kind: &'static str| ItemParams { kind, ..params };
        assert_eq!(sellers_of(dictionary, denim, &as_kind("drp_camo")), []);
        assert_eq!(sellers_of(dictionary, denim, &as_kind("prm_camo")), []);
        let free = Item {
            base_cost: 0,
            ..denim.clone()
        };
        assert_eq!(sellers_of(dictionary, &free, &params), []);
        let premium = Item {
            is_premium: true,
            ..denim.clone()
        };
        assert_eq!(sellers_of(dictionary, &premium, &params), []);
        // A shop camouflage of no faction's folder: no seller (the start
        // refuses such data, `tables::check`).
        let elsewhere = Item {
            cfg_name: format!("{CAMO_FOLDER}premium/camo_denim.options"),
            ..denim.clone()
        };
        assert_eq!(sellers_of(dictionary, &elsewhere, &params), []);
        // A faction's camouflage whose config names a faction still goes by
        // its folder.
        let named = ItemParams {
            faction: faction::EDGE,
            ..params
        };
        assert_eq!(
            sellers_of(dictionary, denim, &named),
            [(faction::BLACK_MARKET, 0)]
        );
    }

    /// The live server's game data (`SRP_GAME_DATA`: stock 0.26g with
    /// skhid's patches 12, 14 and 17, revision 17's `server-game-data.db`,
    /// sha256 `83d4593a…`): the rules add exactly the ten items of
    /// [`NEWLY_SOLD`] and the three of [`STARTERS_SOLD`] to the 189 the shop
    /// sold before, at those sellers and prices, and nothing else.
    #[test]
    #[ignore = "needs the live server's game data (SRP_GAME_DATA)"]
    fn the_live_data_sells_the_ten_items_and_nothing_else_new() {
        use super::super::game_data::{self, GameData};
        use super::super::item_params::derive;
        let path = std::env::var_os(game_data::GAME_DATA_ENV).expect("SRP_GAME_DATA");
        let data = GameData::open(std::path::Path::new(&path)).unwrap();
        super::super::tables::check(&data).unwrap();
        let dictionary =
            Dictionary::parse(data.file(super::super::dictionary::PATH).unwrap()).unwrap();
        let params = derive(&data, &dictionary).unwrap();
        let list = price_list_of(&dictionary, &params);
        eprintln!("{}: {} records", data.made_of(), list.len());
        assert_eq!(
            newly_sold_records(&dictionary, &list),
            expected_newly_sold()
        );
        // The items sold, against those of the rules before (189 items,
        // the unbuyable-items report of 2026-10-17): the ten more, and the
        // starting Mosin and PPSh-41 (2026-10-18) and TT-33 (2026-10-23).
        let mut sold: Vec<u16> = list.iter().map(|record| record.dict_id).collect();
        sold.dedup();
        assert_eq!(sold.len(), 189 + NEWLY_SOLD.len() + STARTERS_SOLD.len());
        let param = |dict_id: u16| {
            &params[params
                .binary_search_by_key(&dict_id, |r| r.dict_id)
                .unwrap()]
        };
        let before = |dict_id: u16| {
            let item = dictionary.item(dict_id).unwrap();
            let gear = GEAR.contains(&item.category);
            (gear && param(dict_id).faction != 0)
                || SUPPLIES.contains(&item.category)
                || EXCEPTIONS
                    .iter()
                    .any(|(name, sellers)| *name == item.name && sellers[0].0 != 0)
        };
        let new: Vec<&str> = sold
            .iter()
            .filter(|&&dict_id| !before(dict_id))
            .map(|&dict_id| dictionary.item(dict_id).unwrap().name.as_str())
            .collect();
        assert_eq!(new, {
            let mut names: Vec<(u16, &str)> = NEWLY_SOLD
                .iter()
                .map(|&(name, dict_id, ..)| (dict_id, name))
                .chain(STARTERS_SOLD.map(|(dict_id, name, _)| (dict_id, name)))
                .collect();
            names.sort_unstable();
            names.into_iter().map(|(_, name)| name).collect::<Vec<_>>()
        });
        // What stays unsold: the New Year camouflage (no price), the drop
        // camouflages, the other weapons with no faction.
        for name in [
            "camo_new_year",
            "camo_black_leather_01",
            "vss",
            "sr3m",
            "socom16",
        ] {
            let dict_id = dictionary.item_named(name).unwrap().dict_id;
            assert!(!sold.contains(&dict_id), "{name}");
        }
    }

    #[test]
    fn scope_items_with_variants_are_sold_to_everyone_at_their_cost() {
        use super::super::game_data;
        use super::super::scopes::fixtures::{
            self, AK_74N_PSO_1, BROKEN, COBRA, COBRA_COST, LONE_SCOPE, PSO_1, PSO_1_COST,
        };
        let data = fixtures::with_scopes();
        let dictionary = fixtures::dictionary_of(&data);
        let fits = fixtures::fits();
        let sellers = |dict_id: u16| {
            let item = dictionary.item(dict_id).unwrap();
            let config = game_data::config(&data, &item.cfg_name).unwrap().unwrap();
            let params = ItemParams::from_config(dict_id, &config).unwrap();
            sellers_of_in(&dictionary, fits, item, &params)
        };
        // Seller 0, no rank: every player may buy; the dictionary's price.
        for (dict_id, cost) in [(PSO_1, PSO_1_COST), (COBRA, COBRA_COST)] {
            assert_eq!(sellers(dict_id), [(0, 0)]);
            assert_eq!(shop_cost(dictionary.item(dict_id).unwrap()), cost);
            // Selling back pays a quarter, as the client shows.
            assert_eq!(
                sell_price(&dictionary, dict_id, 100, 100),
                u64::from(cost / 4)
            );
        }
        // Not a scope item no variant names, not a variant, not 0.26g's
        // own scopes (which have a base_cost: Leupold 25,000, PSO-1
        // 20,000).
        let mut unsold = vec![LONE_SCOPE, AK_74N_PSO_1, 69, 304, 362];
        unsold.extend(BROKEN);
        for dict_id in unsold {
            assert!(sellers(dict_id).is_empty(), "{dict_id}");
        }
        // The players' data before the patch has no variant: no scope in
        // the list.
        assert!(price_list().iter().all(|record| {
            Dictionary::shared().item(record.dict_id).unwrap().category != SCOPE_CATEGORY
        }));
    }
}

/// The shop with the patch of the weapons from 0.29a3 ([`UNRELEASED`]):
/// run on its server game data,
/// `SRP_TEST_NEW_WEAPONS_DATA=<server-game-data.db> cargo test -p survarium
/// --lib -- --ignored the_weapons_from_0_29a3`.
#[cfg(test)]
mod with_the_weapons_from_0_29a3 {
    use super::super::game_data::GameData;
    use super::super::{dictionary, item_params, tables};
    use super::*;

    #[test]
    #[ignore = "needs the server game data of the patch (SRP_TEST_NEW_WEAPONS_DATA)"]
    fn the_weapons_from_0_29a3_are_sold_by_their_factions_and_their_ammo_by_the_table() {
        let path = std::env::var_os("SRP_TEST_NEW_WEAPONS_DATA").expect("the data's path");
        let data = GameData::open(std::path::Path::new(&path)).unwrap();
        // srp starts on it.
        assert_eq!(tables::check(&data), Ok(()));
        let dictionary = Dictionary::parse(data.file(dictionary::PATH).unwrap()).unwrap();
        let params = item_params::derive(&data, &dictionary).unwrap();
        let params_of = |item: &Item| *params.iter().find(|p| p.dict_id == item.dict_id).unwrap();
        // Every unreleased name is in it.
        for name in UNRELEASED {
            assert!(dictionary.item_named(name).is_some(), "{name}");
        }
        let new: Vec<&Item> = dictionary
            .items
            .iter()
            .filter(|item| (2201..=2252).contains(&item.dict_id))
            .collect();
        assert_eq!(new.len(), 40);
        let sold = |name: &str| {
            let item = dictionary.item_named(name).unwrap();
            sellers_of(&dictionary, item, &params_of(item))
        };
        for item in &new {
            let p = params_of(item);
            let sellers = sellers_of(&dictionary, item, &p);
            if p.kind == "ammo" {
                // The owner's halved price, by the factions of the table,
                // at no rank.
                let &(_, game, cost) = AMMO_PRICES
                    .iter()
                    .find(|(name, _, _)| *name == item.name)
                    .unwrap();
                assert_eq!(item.base_cost, game, "{}", item.name);
                assert_eq!(shop_cost(item), cost, "{}", item.name);
                assert!(!sellers.is_empty(), "{}", item.name);
                assert!(
                    sellers
                        .iter()
                        .all(|&(seller, level)| seller != 0 && level == 0)
                );
            } else if item.is_premium {
                // The premium shop's, in gold (`shop_offers::premium_price_list`).
                assert!(sellers.is_empty() && item.base_cost > 0, "{}", item.name);
            } else if item.base_cost > 0 && p.faction != 0 {
                // The config's faction at the rule's rank.
                let level = seller_level(&dictionary, p.faction, p.item_level);
                assert_eq!(sellers, [(p.faction, level)], "{}", item.name);
                assert_eq!(shop_cost(item), item.base_cost, "{}", item.name);
            } else {
                // The stock sights (no cost) and the NeoStead 2000 (no
                // faction, a bundle weapon in 0.29a3).
                assert!(sellers.is_empty(), "{}", item.name);
            }
        }
        assert_eq!(sold("ammo_5.56x45"), [(1, 0), (2, 0)]);
        assert_eq!(sold("ammo_4.6x30"), [(2, 0)]);
        assert_eq!(sold("ammo_5.7x28"), [(2, 0)]);
        assert_eq!(sold("ammo_7.62x54_7n13"), [(3, 0)]);
        assert_eq!(sold("ammo_12mm_buck2"), [(3, 0), (4, 0)]);
        // The open choice's default: Black Market and Scavenger guns of
        // levels 10 to 14 at those factions' last rank, 5.
        assert_eq!(sold("hk416"), [(2, 5)]);
        assert_eq!(sold("kriss_vector"), [(1, 5)]);
        // Renaissance and Edge by the rule.
        assert_eq!(sold("kedr"), [(4, 0)]);
        assert_eq!(sold("vz58"), [(3, 4)]);
        assert_eq!(sold("vepr_12"), [(3, 7)]);
        assert_eq!(sold("pkp_pecheneg"), [(3, 10)]);
        assert!(sold("neostead_2").is_empty() && sold("eotech_552").is_empty());
    }
}

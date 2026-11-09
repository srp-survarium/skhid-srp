//! The 0.26g shop: prices from each item's `base_cost` in the dictionary
//! (0.21d's data had none, hence its hand-made `crate::prices`), sellers
//! and ranks by the rule below or srp's rules in the game data
//! ([`ShopRules`], `scripts/content-edit/shop.tsv`).
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
//! - There is no repair (the owner, 2026-11-08): the stock one cost
//!   `(1 - condition / 100) * repair_cost_ratio * cost` plus the defects'
//!   fix costs (`MessageRepair.init`, PROVEN).
//! - Every `cost` is silver, a premium item's too [srp, the owner's rule of
//!   2026-11-08: silver is the one currency]. The stock lobby read a
//!   premium item's `cost` as gold and multiplied it by the exchange rate
//!   (service price 4, `lobby.md` §2.13) before taking the quarter; srp's
//!   lobby movie does not, and the live data has every premium item's
//!   `base_cost` at its gold price times [`GOLD_IN_SILVER`]
//!   (`scripts/content-edit/items.tsv`).
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
//! and piece of armour but the few of [`ShopRules`], and they give the
//! sellers and ranks of ammo, drugs, the trap and grenades, which no config
//! has.
//!
//! What is listed [srp's choice where marked]:
//! - weapons and armour: by the faction their config names at the rule's
//!   level, or as [`ShopRules`] says; of no faction, by seller 0 at level
//!   0 (the PP-19-01 Vityaz and the MP-27M Lancaster, 2026-10-17; the
//!   starting Mosin, PPSh-41 and TT-33, 2026-10-18 and 2026-10-23; the STK2
//!   Novice's cap and outfits, 2026-10-19 and 2026-10-31: the owner's
//!   choices), but the VSS, SR-3M, SOCOM 16 and plain Neostead, not sold;
//! - drugs, the trap and grenades: by the factions at the ranks of
//!   [`ShopRules`], one record for each faction that sold it;
//! - a supply whose config names a faction (the coloured medkits,
//!   `docs/notes/medicines.md`; no stock supply names one): by that faction
//!   at the rule's level, as gear;
//! - ammo: by the factions of [`ShopRules`], **at no rank** by the owner's
//!   choice ([`AMMO_NEEDS_RANK`]). 0.26 sold ammo at a rank (patch note
//!   0.26c) and 0.27a took that back ("Purchase of ammunition no longer
//!   requires reputation"); the ranks of the time stay in the table;
//! - the boosters (the wiki's "Independent Traders") and the one ammo on no
//!   list of the time (9x19 7N21): seller 0 and level 0 [choice for the
//!   7N21: unknown, so sold to everyone as before];
//! - the factions' own camouflages ([`is_shop_camo`]): the faction whose
//!   folder the config is in ([`CAMO_SHOPS`]), at level 0, for `base_cost`
//!   [the owner's choice, 2026-10-17];
//! - the badges ([`is_badge`]: the decals of `type` [`BADGE`], which the
//!   game unlocked and no drop or shop gives): as gear, by
//!   [`ShopRules`] (the faction badges, each by its faction at the rank it
//!   is named after) or else by seller 0 at no rank (the rank badges), for
//!   `base_cost` [the owner's choice, 2026-11-07: vanity, so dear];
//! - not listed: anything else with `base_cost` 0 (their ammo is listed at
//!   no cost, see [`sellers_of_in`]; artefacts, money, the New Year 2015 camouflage), premium
//!   items (they belong to the premium shop, 0x95, or are sold one by one
//!   by seller 0, `shop_offers`), artefacts (as 0.21d), vouchers,
//!   the other decals and the drop camouflages;
//! - not sold at all, neither by a faction nor one by one ([`ShopRule::NotSold`]): the
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

use binary_config_parser::tree::ConfigValue;

use super::dictionary::{Dictionary, Item};
use super::item_params::ItemParams;
use super::scopes::{SCOPE_CATEGORY, ScopeFits};

/// Silver for one gold: the game's exchange rate (0.21d's web page sold gold
/// at it, and 0.26g's lobby was told it). Gold is gone [the owner,
/// 2026-11-08]; whatever had a gold price or gave gold was converted at
/// this rate, once: the balances (`session` persist migration 14), the
/// premium items' prices (`items.tsv`), the premium shop's, the upgrade's
/// and the modification change's prices, the gold the levels and drops gave.
pub const GOLD_IN_SILVER: u32 = 100;

/// 0.26g faction ids (`factions_dict`), as 0.21d's.
pub use crate::prices::faction;

/// The categories listed with no seller: ammo (9, 18..=21), drugs and
/// boosters (10), traps (11) and grenades (24).
const SUPPLIES: [u8; 8] = [9, 10, 11, 18, 19, 20, 21, 24];
/// Weapons (8, 13..=17) and armour (1..=7).
const GEAR: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 13, 14, 15, 16, 17];
/// Camouflages.
const CAMOUFLAGE: u8 = 25;
/// Decals.
const DECAL: u8 = 26;

/// The `type` of a badge: a decal the game unlocked by a player's rank or
/// a faction's (`char_decal_1_*`, `char_decal_2_*`), sold as gear
/// ([`is_badge`]). The others are `drp_decl` (drops) and `prm_decl`
/// (premium, `shop_offers`).
pub const BADGE: &str = "unl_decl";

/// Whether `item` is a badge: a decal of `type` [`BADGE`] that is not
/// premium.
pub fn is_badge(item: &Item, params: &ItemParams) -> bool {
    item.category == DECAL && params.kind == BADGE && !item.is_premium
}

/// The `type` of a faction's own camouflage: a "stock" camouflage, sold for
/// silver. The others are `drp_camo` (drops, `base_cost` 1) and `prm_camo`
/// (premium, `shop_offers`; the New Year 2015 one costs nothing).
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

/// srp's shop rules in the game data, made by `scripts/content-edit/shop.tsv`.
pub const SHOP_PATH: &str = "gameplay/srp/shop.options";

/// The sellers' keys of [`SHOP_PATH`].
const SELLERS: [(&str, u8); 4] = [
    ("scav", faction::SCAVENGERS),
    ("bm", faction::BLACK_MARKET),
    ("ren", faction::RENAISSANCE),
    ("edge", faction::EDGE),
];

/// What [`SHOP_PATH`] says of an item, where the module's rule does not say
/// it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShopRule {
    /// `(seller, reputation level)`, by seller: an ammo's levels are not
    /// asked for ([`AMMO_NEEDS_RANK`]).
    Sellers(Vec<(u8, u8)>),
    /// No shop sells it, neither a faction nor the premium list.
    NotSold,
}

/// The rules of [`SHOP_PATH`], by `dict_id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopRules(std::collections::BTreeMap<u16, ShopRule>);

impl ShopRules {
    /// The rules of `config` for the items of `dictionary`: an item the
    /// dictionary lacks, a key that is no seller, a rank past its faction's
    /// last, or an item both sold and not is refused.
    pub fn parse(config: &[u8], dictionary: &Dictionary) -> Result<Self, String> {
        let root =
            ConfigValue::parse(config).map_err(|cause| format!("{SHOP_PATH}: {}", cause.0))?;
        let ConfigValue::Named(entries) = root else {
            return Err(format!("{SHOP_PATH}: expected a named table of item rules"));
        };
        let mut rules = std::collections::BTreeMap::new();
        for (name, keys) in &entries {
            let refuse = |why: String| format!("{SHOP_PATH}: {name}: {why}");
            let item = dictionary
                .item_named(name)
                .ok_or_else(|| refuse("no such item".into()))?;
            let (mut sellers, mut not_sold) = (Vec::new(), false);
            for (key, value) in keys.entries() {
                let seller = SELLERS.iter().find(|(seller, _)| seller == key);
                match (seller, value) {
                    (None, ConfigValue::String(_)) if key == "not_sold" => not_sold = true,
                    (Some(&(_, faction)), &ConfigValue::Integer(rank)) => {
                        let rank = u8::try_from(rank)
                            .ok()
                            .filter(|&rank| rank <= top_level(dictionary, faction))
                            .ok_or_else(|| refuse(format!("{key} rank {rank}")))?;
                        sellers.push((faction, rank));
                    }
                    _ => return Err(refuse(format!("`{key}` is {value:?}"))),
                }
            }
            sellers.sort_unstable();
            let rule = match (not_sold, sellers.is_empty()) {
                (true, true) => ShopRule::NotSold,
                (false, false) => ShopRule::Sellers(sellers),
                _ => return Err(refuse("sold and not sold".into())),
            };
            rules.insert(item.dict_id, rule);
        }
        Ok(Self(rules))
    }

    /// The rules of the game's files ([`super::game_data::shared`]),
    /// parsed once ([`super::tables::check`] refuses data whose rules do
    /// not parse).
    pub fn shared() -> &'static Self {
        static SHARED: std::sync::OnceLock<ShopRules> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| {
            Self::parse(super::game_data::file(SHOP_PATH), Dictionary::shared())
                .expect("srp checks its shop rules at start (tables::check)")
        })
    }

    /// What the rules say of `dict_id`, if anything.
    pub fn rule(&self, dict_id: u16) -> Option<&ShopRule> {
        self.0.get(&dict_id)
    }
}

/// Whether buying ammo needs the seller's rank of [`ShopRules`]. The
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

/// The reputation level with its faction that buying an item of
/// `item_level` needs (the module's rule), within the faction's levels.
///
/// [open, the owner's to choose; this is the default] The weapons from
/// 0.29a3 give the Scavengers and the Black Market guns of item levels 8
/// to 14 (MSBS Radon, KRISS Vector; Valmet, FNC, M4A1, HK416, MP7, P90),
/// and their ranks end at 5: the rule sells them all at rank 5, beside
/// the factions' own level-5 gear, as it does the Bandit balaclava
/// (level 8, the owner's word of 2026-10-16). The other way would be
/// [`ShopRules`] rows, or a scale of their levels onto ranks 0 to 5.
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
/// [`ShopRules`] first, else the module's rule; ammo at level 0
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
    let gear = GEAR.contains(&item.category) || is_badge(item, params);
    let supply = SUPPLIES.contains(&item.category) && item.is_stack;
    if item.base_cost == 0 {
        // A free supply (the starting weapons' ammo) is listed at no cost by
        // seller 0, who sells to everyone: the client lets a player add to a
        // stack only when the price list has the item (`canBuy`, set by
        // `GangShop.setupShopItems`), and for a price of 0 it allows up to
        // `MAX_FREE_ITEMS_COUNT` (1000) units (`AmmoShareElement.updateStorage`).
        // Without the record the ammo window's storage field is locked (seen
        // in the first live session, 2026-10-12).
        return if supply { vec![(0, 0)] } else { Vec::new() };
    }
    if !gear && !supply {
        return Vec::new();
    }
    let ranked = AMMO_NEEDS_RANK || params.kind != "ammo";
    let rank = |level: u8| if ranked { level } else { 0 };
    match ShopRules::shared().rule(item.dict_id) {
        Some(ShopRule::Sellers(sellers)) => sellers
            .iter()
            .map(|&(seller, level)| (seller, rank(level)))
            .collect(),
        // By the faction its config names (a supply's: the coloured
        // medkits) at the rule's level; of no faction, seller 0 at none.
        _ => {
            let level = seller_level(dictionary, params.faction, params.item_level);
            vec![(params.faction, rank(level))]
        }
    }
}

/// Whether `item` is one no shop sells ([`ShopRule::NotSold`]).
pub fn is_not_sold(item: &Item) -> bool {
    ShopRules::shared().rule(item.dict_id) == Some(&ShopRule::NotSold)
}

fn listed<'a>(
    dictionary: &'a Dictionary,
    item: &'a Item,
    params: &ItemParams,
) -> impl Iterator<Item = PriceItem> + 'a {
    let (cost, dict_id) = (item.base_cost, item.dict_id);
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

/// What the lobby shows for selling `steps` of `dict_id`: a quarter of its
/// cost a step (a step: a clip of ammo, else one item;
/// [`super::item_params::shop_unit`]). Items never wear (no repair, the
/// owner 2026-11-08), so their condition counts for nothing.
pub fn sell_price(dictionary: &Dictionary, dict_id: u16, steps: u32) -> u64 {
    dictionary
        .item(dict_id)
        .map_or(0, |item| u64::from(item.base_cost) / 4 * u64::from(steps))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::item_params::item_params;
    use crate::v026g::game_data::key_value_config;

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
    fn a_malformed_shop_root_cannot_silently_drop_all_rules() {
        for root in [ConfigValue::Integer(0), ConfigValue::Indexed(vec![])] {
            let error =
                ShopRules::parse(&root.to_bytes().unwrap(), Dictionary::shared()).unwrap_err();
            assert!(error.contains("expected a named table"), "{error}");
        }
    }

    #[test]
    fn the_shop_rules_are_items_sellers_and_ranks_of_the_data() {
        let dictionary = Dictionary::shared();
        let parse = |table: &str| ShopRules::parse(&key_value_config(table), dictionary);
        let rules =
            parse("key\tvalue\nrgd-5/scav\t3\nrgd-5/bm\t3\ndecals_4_1/not_sold\tUSSR\n").unwrap();
        assert_eq!(
            rules.rule(5),
            Some(&ShopRule::Sellers(vec![(1, 3), (2, 3)]))
        );
        assert_eq!(rules.rule(266), Some(&ShopRule::NotSold));
        assert_eq!(rules.rule(3), None);
        for (table, why) in [
            ("no_such_item/scav\t1", "no such item"),
            ("rgd-5/nobody\t1", "`nobody`"),
            ("rgd-5/scav\t6", "scav rank 6"),
            ("rgd-5/ren\t-1", "ren rank -1"),
            ("rgd-5/scav\t1.5", "`scav`"),
            ("rgd-5/scav\t1\nrgd-5/not_sold\tno", "sold and not sold"),
        ] {
            let error = parse(&format!("key\tvalue\n{table}\n")).unwrap_err();
            assert!(error.contains(why), "{table}: {error}");
        }
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
            assert_eq!(record.cost, item.base_cost, "{}", item.name);
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
            let excepted = ShopRules::shared().rule(record.dict_id).is_some();
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
    fn the_shop_rules_sell_as_they_say() {
        let dictionary = Dictionary::shared();
        let mut sellers_ruled = 0;
        for item in &dictionary.items {
            match ShopRules::shared().rule(item.dict_id) {
                Some(ShopRule::Sellers(listed)) => {
                    // Ammo: the listed sellers, at no rank.
                    let ammo =
                        !AMMO_NEEDS_RANK && item_params(item.dict_id).unwrap().kind == "ammo";
                    let expected: Vec<(u8, u8)> = listed
                        .iter()
                        .map(|&(seller, level)| (seller, if ammo { 0 } else { level }))
                        .collect();
                    assert_eq!(sold_by(&item.name), expected, "{}", item.name);
                    sellers_ruled += 1;
                }
                Some(ShopRule::NotSold) => {
                    assert!(sellers(item.dict_id).is_empty(), "{}", item.name)
                }
                None => {}
            }
        }
        assert!(sellers_ruled > 40, "{sellers_ruled}");
    }

    #[test]
    fn gear_of_no_faction_is_sold_to_everyone_at_no_rank() {
        // The Vityaz, the Lancaster, the starting weapons (and the STK2 cap
        // and outfits, which the test data has not): seller 0, no rank.
        for name in [
            "vityaz",
            "izh_27_lancaster",
            "tt_33",
            "ppsh",
            "mosin_nagant",
        ] {
            assert_eq!(
                item_params(Dictionary::shared().item_named(name).unwrap().dict_id)
                    .unwrap()
                    .faction,
                0
            );
            assert_eq!(sold_by(name), [(0, 0)], "{name}");
        }
        // But what the rules do not sell.
        for name in ["vss", "sr3m", "socom16"] {
            assert!(sold_by(name).is_empty(), "{name}");
        }
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
        // 5.45x39 7N6M: 30 for a clip of 30.
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
        // No other decals, vouchers, drop camouflages or the free New Year
        // one: of the cosmetics, only the shop camouflages and the badges.
        for record in price_list() {
            let item = dictionary.item(record.dict_id).unwrap();
            if item.category >= 25 {
                let params = item_params(item.dict_id).unwrap();
                assert!(
                    is_shop_camo(item, params) || is_badge(item, params),
                    "{}",
                    item.name
                );
            }
        }
        assert!(!sold("decals_3_4") && !sold("decals_4_3"));
        assert!(!sold("camo_black_leather_01") && !sold("camo_khaki_fabric_03"));
        assert!(!sold("camo_new_year"));
        assert_eq!(buy_cost(214, 1), None);
    }

    /// The badges (owner, 2026-11-07): every rank badge to everyone at no
    /// rank, a million each but the Literator (50,000, the owner,
    /// 2026-11-08) and the Flying Hussars; each faction badge by its faction
    /// at the rank it is named after.
    #[test]
    fn the_badges_are_sold_dear() {
        let dictionary = Dictionary::shared();
        let cost = |name: &str| {
            price(dictionary.item_named(name).unwrap().dict_id)
                .unwrap()
                .cost
        };
        for n in 1..=7 {
            let name = format!("decals_1_{n}");
            assert_eq!(sold_by(&name), [(0, 0)], "{name}");
            let price = match n {
                1 => 50_000,
                2 => 10_000_000,
                _ => 1_000_000,
            };
            assert_eq!(cost(&name), price, "{name}");
        }
        assert_eq!(sold_by("decals_1_8"), [(0, 0)]);
        assert_eq!(cost("decals_1_8"), 20_000);
        let (scav, bm, ren, edge) = (
            faction::SCAVENGERS,
            faction::BLACK_MARKET,
            faction::RENAISSANCE,
            faction::EDGE,
        );
        for (name, seller, rank, price) in [
            ("decals_2_1", scav, 1, 50_000),   // Scavengers rookie
            ("decals_2_2", scav, 5, 100_000),  // Scavengers veteran
            ("decals_2_3", bm, 1, 50_000),     // Black Market Recruit
            ("decals_2_4", bm, 5, 100_000),    // Black Market Elite
            ("decals_2_5", ren, 1, 50_000),    // Renaissance Army Soldier
            ("decals_2_6", ren, 10, 100_000),  // Renaissance Army Colonel
            ("decals_2_7", edge, 1, 50_000),   // Fringe Settlers Hunter
            ("decals_2_8", edge, 10, 100_000), // Fringe Settlers Magister
        ] {
            assert_eq!(sold_by(name), [(seller, rank)], "{name}");
            assert_eq!(cost(name), price, "{name}");
        }
    }

    /// The decals no shop sells ([`ShopRule::NotSold`]), as reviewed
    /// (2026-10-18), have no seller even as a non-premium copy, and selling
    /// one back still pays.
    #[test]
    fn the_excluded_flags_and_emblems_are_not_sold() {
        let dictionary = Dictionary::shared();
        let reviewed: Vec<(u16, &str)> = dictionary
            .items
            .iter()
            .filter(|item| item.category == 26 && is_not_sold(item))
            .map(|item| (item.dict_id, item.cfg_name.as_str()))
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
            // 300 gold, in silver.
            assert!(item.is_premium && item.base_cost == 30_000 && params.kind == "prm_decl");
            assert!(is_not_sold(item));
            assert!(sellers(dict_id).is_empty() && buy_cost(dict_id, 1).is_none());
            // Were it no premium item, still no seller.
            let silver = Item {
                is_premium: false,
                ..item.clone()
            };
            assert_eq!(sellers_of(dictionary, &silver, params), []);
            // An owned one sells back for a quarter of its price.
            assert_eq!(sell_price(dictionary, dict_id, 1), 7_500);
        }
        // The other flags and coats of arms are not in it, the coat of arms
        // of Lithuania (the Vytis) included.
        for name in ["decals_4_3", "decals_5_7"] {
            assert!(!is_not_sold(dictionary.item_named(name).unwrap()), "{name}");
        }
    }

    #[test]
    fn selling_gives_a_quarter_a_step() {
        let dictionary = Dictionary::shared();
        // The AK-74M.
        assert_eq!(sell_price(dictionary, 3, 1), 11875);
        // Two clips of 5.45: 7 a clip (a quarter of 30).
        assert_eq!(sell_price(dictionary, 7, 2), 14);
        // A premium item's cost is silver as any other's: the Uzi's 500 gold
        // is 50,000 (`items.tsv`).
        let uzi = dictionary.item_named("uzi_premium").unwrap().dict_id;
        assert_eq!(sell_price(dictionary, uzi, 1), 12_500);
    }

    /// The owner's choice: the starting Mosin carbine and PPSh-41 (2026-10-18,
    /// 1,000 each) and the TT-33 (2026-10-23, 500) are sold to everyone at no
    /// rank; selling one back pays a quarter.
    #[test]
    fn the_starting_weapons_are_sold_to_everyone() {
        let dictionary = Dictionary::shared();
        for (dict_id, name, cost) in STARTERS_SOLD {
            let item = dictionary.item_named(name).unwrap();
            assert_eq!((item.dict_id, item.base_cost), (dict_id, cost), "{name}");
            let records: Vec<_> = price_list()
                .iter()
                .filter(|record| record.dict_id == dict_id)
                .map(|record| (record.faction_id, record.level, record.cost))
                .collect();
            assert_eq!(records, [(0, 0, cost)], "{name}");
            assert_eq!(buy_cost(dict_id, 1), Some(u64::from(cost)), "{name}");
            let quarter = u64::from(cost / 4);
            assert_eq!(sell_price(dictionary, dict_id, 1), quarter, "{name}");
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
        // The Lancaster at the game's price; the Vityaz at the owner's
        // (2026-10-17; the game's was 12,000).
        assert_eq!(
            dictionary.item_named("izh_27_lancaster").unwrap().base_cost,
            2500
        );
        assert_eq!(dictionary.item_named("vityaz").unwrap().base_cost, 25000);
        // Their premium copies stay out of the silver list.
        assert!(price(dictionary.item_named("vityaz_premium").unwrap().dict_id).is_none());
        let lancaster = dictionary.item_named("izh_27_lancaster_premium").unwrap();
        assert!(price(lancaster.dict_id).is_none());
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

    #[test]
    fn a_supply_whose_config_names_a_faction_is_sold_by_it() {
        // The coloured medkits: the stock medkit's row and config, but a
        // faction (docs/notes/medicines.md).
        let dictionary = Dictionary::shared();
        let medkit = dictionary.item_named("medkit").unwrap();
        let params = *item_params(medkit.dict_id).unwrap();
        assert_eq!((params.faction, params.item_level), (0, 3));
        let named = |faction: u8| ItemParams { faction, ..params };
        // No shop rule: the rule's level of the faction.
        let coloured = Item {
            dict_id: 9001,
            name: "blackmarket_medkit".into(),
            ..medkit.clone()
        };
        assert_eq!(
            sellers_of(dictionary, &coloured, &named(faction::BLACK_MARKET)),
            [(faction::BLACK_MARKET, 3)]
        );
        assert_eq!(
            sellers_of(dictionary, &coloured, &named(faction::RENAISSANCE)),
            [(faction::RENAISSANCE, 0)]
        );
        // With no faction: seller 0, as before.
        assert_eq!(sellers_of(dictionary, &coloured, &params), [(0, 0)]);
        // A shop rule still decides.
        assert_eq!(
            sellers_of(dictionary, medkit, &named(faction::EDGE)),
            [(faction::SCAVENGERS, 3)]
        );
        // Ammo of a faction needs no rank.
        let ammo = ItemParams {
            kind: "ammo",
            item_level: 9,
            ..named(faction::BLACK_MARKET)
        };
        assert_eq!(
            sellers_of(dictionary, &coloured, &ammo),
            [(faction::BLACK_MARKET, 0)]
        );
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
            assert_eq!(dictionary.item(dict_id).unwrap().base_cost, cost);
            // Selling back pays a quarter, as the client shows.
            assert_eq!(
                sell_price(&dictionary, dict_id, 1),
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

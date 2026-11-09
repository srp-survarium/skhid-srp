//! What the premium shop (lobby 0x95, `lobby.md` §2.18) offers: premium
//! account time and the bundles of `shop_bundles_dict`.
//!
//! What is the game's own:
//! - **The fourteen offers** are the dictionary's bundles: a 0x95 record
//!   names one by its id (the client looks its name, description and icon
//!   up in `shop_bundles_dict` by the record's u32 at +0x04; 0x2CAC10,
//!   PROVEN), so nothing else can be offered there.
//! - **How long each premium offer lasts**: its description in the
//!   localization (`st_premium_3_day_descr` "(72 hours)", `..._week_descr`
//!   "(7 days)", `..._month_descr` "(30 days)", "(90 days)", "(180 days)",
//!   "(365 days)", `st_premium_day_descr` "(24 hours)"). PROVEN.
//! - **What each bundle holds**: its description (`st_ak74n_bundle_descr`:
//!   "Improved AK-74N, 2000 rounds of 5.45x39 7N6M ammo", and so on).
//!   PROVEN for the names and counts; that an "improved" weapon is the
//!   dictionary's `*_premium` copy and the starter bundles' jacket and
//!   pants the `*_4_premium` ones is LIKELY (they are the only copies with
//!   fixed modifications, and nothing else gives them out).
//!
//! No price is in the client. Every price is silver [the owner,
//! 2026-11-08: gold is gone]: what the sources of the time give in gold,
//! times [`prices::GOLD_IN_SILVER`]. What they give
//! (`docs/notes/progression_sources.md` §6):
//! - **a bundle's price** ([`Bundle::silver`]): the wiki's `Bundles` page of
//!   2015-01-22 (revision 7866, written for 0.26h): AK-74N 1,500, Bandit
//!   2,000, Survivor 2,000, Lancaster 300, Vityaz 700, SR-3M Vikhr 5,000,
//!   VSS Vintorez 5,000 gold. PROVEN (wiki of the time). The premium
//!   copies' own `base_cost` (1,200, 500, 2,500 ...) is not the bundle's
//!   price; the lobby still uses it for selling such an item back.
//!   So 150,000 silver for the AK-74N's, and so on.
//! - **90 days of premium: 2,500 gold**. LIKELY: the official news of the
//!   "winter weekend" sale of January 2015 says three months at 40 % off
//!   save $10, so $25, and 1,000 gold was $10: 250,000 silver.
//!
//! The owner's choice: the other six premium times of
//! [`DEFAULT_PREMIUM_PRICES`] (no price of the time was found).
//! The operator may replace the list (`SRP_PREMIUM_PRICES`, read by
//! `session::v026g::premium`).
//!
//! An offer's id (what `47 02` names) is its bundle id.
//!
//! # Premium items one by one
//!
//! A 0x95 record can only be premium time or a bundle, so the other
//! premium items (the vouchers, the premium camouflages and decals, the
//! AK-74N) go where the client has a place for them: the price list
//! (0x8A), which the shop's window fills its list from whatever the
//! category (`GangShop.setupShopItems`; it has icons for vouchers, decals
//! and camouflages); the stock buy dialog paid gold for an item the
//! dictionary calls premium (`MessageAsk`: `_money_type = isPremium`),
//! srp's lobby movie pays silver for every item. PROVEN for the window.
//! [choice] srp lists every premium item with a price that no bundle
//! holds, at its `base_cost` (silver, `items.tsv`), under seller 0
//! (sold to everyone): [`premium_price_list`]. The items of a bundle are
//! sold by their bundle. The items no shop sells ([`prices::is_not_sold`]: the Russian and
//! Belarusian flags and coats of arms, the flag of the USSR and the red
//! star; the owner's choice of 2026-10-18)
//! are sold neither way.

use super::dictionary::Dictionary;
use super::lobby::{ShopRecord, ShopRecordItem};
use super::prices::{self, PriceItem};

/// `money_type` / `currency_type` of a record: silver, the one currency (1
/// was gold, the lobby's `isPremium`).
pub const SILVER: u32 = 0;
/// A record's kind (+0x0C): premium time for the `set_premium_store` list.
pub const KIND_PREMIUM: u32 = 0;
/// A record's kind: a bundle for the shop's list (`setup_bundle_data`).
pub const KIND_BUNDLE: u32 = 1;
/// An item of a record (+0x20) that is an item of the inventory: the only
/// ones the client lists as a bundle's contents (0x228A00).
pub const ITEM_OF_INVENTORY: u32 = 0;

/// A premium offer: `(bundle id, days)`, longest first. The lobby turns the
/// list round for its window (`LobbyMenu.set_premium_store`:
/// `value.reverse()`), so the window shows the shortest first.
pub const PREMIUM_OFFERS: [(u8, u32); 7] =
    [(6, 365), (5, 180), (4, 90), (3, 30), (2, 7), (1, 3), (7, 1)];

/// What the premium offers cost in silver unless the operator says
/// otherwise, in the order of [`PREMIUM_OFFERS`]: the gold prices below
/// times [`prices::GOLD_IN_SILVER`]. The 90 days are 2,500 gold
/// (LIKELY: $25 in the official sale news of January 2015; see the module).
/// The rest is the owner's choice (2026-10-12), set around the 90 days so
/// that a longer time is cheaper a day: a day 100, 3 days 200, a week 400,
/// a month 1,000, 180 days 4,500, a year 8,000.
pub const DEFAULT_PREMIUM_PRICES: [u32; 7] =
    [800_000, 450_000, 250_000, 100_000, 40_000, 20_000, 10_000];

/// A bundle: its id, its price and what it holds, `(item name, units)`; 1
/// unit of an item that is no stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bundle {
    pub bundle_id: u8,
    /// Its price in silver: in gold, the wiki's `Bundles` page of 2015-01-22 (revision
    /// 7866, 0.26h). PROVEN (wiki of the time).
    pub silver: u32,
    pub items: &'static [(&'static str, u32)],
}

/// The seven bundles of items (ids 8..=14), by id.
pub const BUNDLES: [Bundle; 7] = [
    // "Bandit" bundle: jacket, pants, an enhanced Uzi, 2000 rounds.
    Bundle {
        bundle_id: 8,
        silver: 200_000,
        items: &[
            ("bm_torso_4_premium", 1),
            ("bm_legs_4_premium", 1),
            ("uzi_premium", 1),
            ("ammo_9x19p_para", 2000),
        ],
    },
    Bundle {
        bundle_id: 9,
        silver: 150_000,
        items: &[("ak_74n_premium", 1), ("ammo_5.45x39_7n6m", 2000)],
    },
    // "1000 12x70 rounds, buckshot".
    Bundle {
        bundle_id: 10,
        silver: 30_000,
        items: &[("izh_27_lancaster_premium", 1), ("ammo_12mm_buck", 1000)],
    },
    Bundle {
        bundle_id: 11,
        silver: 500_000,
        items: &[("sr3m_premium", 1), ("ammo_sp6", 2000)],
    },
    Bundle {
        bundle_id: 12,
        silver: 70_000,
        items: &[("vityaz_premium", 1), ("ammo_9x19p_para", 2000)],
    },
    Bundle {
        bundle_id: 13,
        silver: 500_000,
        items: &[("vss_premium", 1), ("ammo_sp5", 1000)],
    },
    // "Survivor" bundle: jacket, pants, an improved Kiparis, 1000 rounds
    // of 9x18 PM.
    Bundle {
        bundle_id: 14,
        silver: 200_000,
        items: &[
            ("scav_torso_4_premium", 1),
            ("scav_legs_4_premium", 1),
            ("kiparis_premium", 1),
            ("ammo_9x18_makarov", 1000),
        ],
    },
];

impl Bundle {
    /// The bundle's items by dict id: `(dict id, units, is a stack)`.
    pub fn contents(&self, dictionary: &Dictionary) -> Vec<(u16, u32, bool)> {
        self.items
            .iter()
            .filter_map(|&(name, units)| {
                let item = dictionary.item_named(name)?;
                Some((item.dict_id, units, item.is_stack))
            })
            .collect()
    }

    /// What it costs in silver ([`Bundle::silver`]).
    pub fn price(&self, _dictionary: &Dictionary) -> u32 {
        self.silver
    }
}

/// What an offer id names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    /// Premium time; `index` is its place in [`PREMIUM_OFFERS`].
    Premium {
        index: usize,
        days: u32,
    },
    Bundle(&'static Bundle),
}

/// The offer `47 02` names, if there is one.
pub fn offer(offer_id: u32) -> Option<Offer> {
    if let Some(index) = PREMIUM_OFFERS
        .iter()
        .position(|&(id, _)| u32::from(id) == offer_id)
    {
        return Some(Offer::Premium {
            index,
            days: PREMIUM_OFFERS[index].1,
        });
    }
    BUNDLES
        .iter()
        .find(|bundle| u32::from(bundle.bundle_id) == offer_id)
        .map(Offer::Bundle)
}

/// The records of 0x95: the premium offers at `premium_prices` (silver, in
/// the order of [`PREMIUM_OFFERS`]), then the bundles.
pub fn records(premium_prices: &[u32; 7]) -> Vec<ShopRecord> {
    let dictionary = Dictionary::shared();
    let premium = PREMIUM_OFFERS
        .iter()
        .zip(premium_prices)
        .map(|(&(bundle_id, _), &price)| ShopRecord {
            id: bundle_id.into(),
            ui_key: bundle_id.into(),
            kind: KIND_PREMIUM,
            value: price,
            // No item is counted for a discount label: srp sells at one
            // price (the label is computed from items of kind 1 at +0x1C,
            // 0x228280).
            items: Vec::new(),
            money_type: SILVER,
        });
    let bundles = BUNDLES.iter().map(|bundle| ShopRecord {
        id: bundle.bundle_id.into(),
        ui_key: bundle.bundle_id.into(),
        kind: KIND_BUNDLE,
        value: bundle.price(dictionary),
        items: bundle
            .contents(dictionary)
            .into_iter()
            .map(|(dict_id, units, _)| ShopRecordItem {
                dict_id: dict_id.into(),
                amount: units,
                kind: ITEM_OF_INVENTORY,
                ..ShopRecordItem::default()
            })
            .collect(),
        money_type: SILVER,
    });
    premium.chain(bundles).collect()
}

/// The premium items the shop sells one by one, by `dict_id`, at their
/// `base_cost` in silver.
pub fn premium_price_list() -> &'static [PriceItem] {
    static SHARED: std::sync::OnceLock<Vec<PriceItem>> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        let dictionary = Dictionary::shared();
        let bundled: Vec<u16> = BUNDLES
            .iter()
            .flat_map(|bundle| bundle.contents(dictionary))
            .map(|(dict_id, _, _)| dict_id)
            .collect();
        dictionary
            .items
            .iter()
            .filter(|item| item.is_premium && item.base_cost > 0)
            .filter(|item| !prices::is_not_sold(item))
            .filter(|item| !bundled.contains(&item.dict_id))
            .map(|item| PriceItem {
                cost: item.base_cost,
                dict_id: item.dict_id,
                level: 0,
                faction_id: 0,
            })
            .collect()
    })
}

/// The price of a premium item sold one by one.
pub fn premium_price(dict_id: u16) -> Option<&'static PriceItem> {
    let list = premium_price_list();
    list.binary_search_by_key(&dict_id, |price| price.dict_id)
        .ok()
        .map(|index| &list[index])
}

/// The whole price list the lobby is sent (0x8A): the shop's
/// ([`prices::price_list`]; an item two factions sell has two records) and
/// the premium items, all in silver, by `dict_id` and seller.
pub fn price_list() -> &'static [PriceItem] {
    static SHARED: std::sync::OnceLock<Vec<PriceItem>> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        let mut list: Vec<PriceItem> = prices::price_list()
            .iter()
            .chain(premium_price_list())
            .copied()
            .collect();
        list.sort_by_key(|price| (price.dict_id, price.faction_id));
        list
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premium_items_are_sold_for_their_price() {
        let dictionary = Dictionary::shared();
        let id = |name: &str| dictionary.item_named(name).unwrap().dict_id;
        // The drop card, at its gold price in silver (`items.tsv`); the
        // other vouchers are sold no more (the owner, 2026-11-08: `shop.tsv`).
        let cost = |name: &str| premium_price(id(name)).map(|price| price.cost);
        assert_eq!(cost("drop_card"), Some(10_000));
        assert_eq!(cost("repair_voucher"), None);
        assert_eq!(cost("rename_account_voucher"), None);
        assert_eq!(cost("premium_voucher_1d"), None);
        assert_eq!(cost("ak_74n"), Some(150_000));
        // A bundle's items are the bundle's to sell; the shop's own items
        // and money are not premium.
        assert_eq!(cost("ak_74n_premium"), None);
        assert_eq!(cost("scav_torso_4_premium"), None);
        assert_eq!(cost("ammo_5.45x39_7n6m"), None);
        assert_eq!(cost("premium_money"), None);
        let list = premium_price_list();
        assert!(list.iter().all(|price| {
            let item = dictionary.item(price.dict_id).unwrap();
            item.is_premium && price.cost == item.base_cost && price.faction_id == 0
        }));
        // Cosmetics are most of them.
        let cosmetics = list
            .iter()
            .filter(|price| matches!(dictionary.item(price.dict_id).unwrap().category, 25 | 26))
            .count();
        assert!(cosmetics > 50, "{cosmetics}");
        // The whole list has each item once for each of its sellers, in
        // order.
        let whole = price_list();
        assert_eq!(whole.len(), prices::price_list().len() + list.len());
        assert!(whole.windows(2).all(|pair| {
            (pair[0].dict_id, pair[0].faction_id) < (pair[1].dict_id, pair[1].faction_id)
        }));
    }

    /// The decals no shop sells (2026-10-18, `prices::is_not_sold`) and the
    /// three vouchers removed (2026-11-08): the premium list is every premium
    /// item with a price that no bundle holds but those nine, and neither
    /// list the lobby is sent has them.
    #[test]
    fn the_premium_list_leaves_out_what_is_not_sold() {
        let dictionary = Dictionary::shared();
        let excluded: Vec<u16> = dictionary
            .items
            .iter()
            .filter(|item| item.is_premium && prices::is_not_sold(item))
            .map(|item| item.dict_id)
            .collect();
        assert_eq!(excluded.len(), 9);
        for &dict_id in &excluded {
            assert_eq!(premium_price(dict_id), None);
            assert!(price_list().iter().all(|price| price.dict_id != dict_id));
        }
        let bundled: Vec<u16> = BUNDLES
            .iter()
            .flat_map(|bundle| bundle.contents(dictionary))
            .map(|(dict_id, _, _)| dict_id)
            .collect();
        let premium: Vec<u16> = dictionary
            .items
            .iter()
            .filter(|item| item.is_premium && item.base_cost > 0)
            .map(|item| item.dict_id)
            .filter(|dict_id| !bundled.contains(dict_id))
            .collect();
        let listed: Vec<u16> = premium_price_list().iter().map(|p| p.dict_id).collect();
        assert_eq!(listed.len(), premium.len() - excluded.len());
        let kept: Vec<u16> = premium
            .into_iter()
            .filter(|dict_id| !excluded.contains(dict_id))
            .collect();
        assert_eq!(listed, kept);
        // The other 77 premium flags and coats of arms stay at their 300
        // gold, in silver.
        let decals = premium_price_list()
            .iter()
            .filter(|price| dictionary.item(price.dict_id).unwrap().category == 26)
            .inspect(|price| assert_eq!(price.cost, 30_000))
            .count();
        assert_eq!(decals, 83 - 6);
    }

    #[test]
    fn every_offer_is_a_bundle_of_the_dictionary() {
        let dictionary = Dictionary::shared();
        let mut ids: Vec<u8> = PREMIUM_OFFERS
            .iter()
            .map(|&(id, _)| id)
            .chain(BUNDLES.iter().map(|bundle| bundle.bundle_id))
            .collect();
        ids.sort_unstable();
        let known: Vec<u8> = dictionary.bundles.iter().map(|bundle| bundle.id).collect();
        assert_eq!(ids, known);
        // The premium offers are the `premium_*` configs, the durations
        // their names'.
        let cfg = |id: u8| {
            let bundle = dictionary.bundles.iter().find(|b| b.id == id).unwrap();
            bundle
                .cfg
                .trim_start_matches("gameplay/bundles/")
                .trim_end_matches("_bundle.options")
                .to_owned()
        };
        let named: Vec<(String, u32)> = PREMIUM_OFFERS
            .iter()
            .map(|&(id, days)| (cfg(id), days))
            .collect();
        let expected = [
            ("premium_1_year", 365),
            ("premium_6_month", 180),
            ("premium_3_month", 90),
            ("premium_1_month", 30),
            ("premium_1_week", 7),
            ("premium_3_day", 3),
            ("premium_1_day", 1),
        ];
        for (index, (name, days)) in expected.into_iter().enumerate() {
            assert_eq!(named[index], (name.to_owned(), days));
        }
        assert_eq!(cfg(9), "ak74n");
        assert_eq!(cfg(14), "scavenger_starter");
    }

    #[test]
    fn bundles_hold_items_of_the_dictionary() {
        let dictionary = Dictionary::shared();
        for bundle in &BUNDLES {
            let contents = bundle.contents(dictionary);
            // Every name is an item.
            assert_eq!(contents.len(), bundle.items.len(), "{bundle:?}");
            // Gear once, ammo by the round.
            for &(dict_id, units, is_stack) in &contents {
                assert_eq!(units == 1, !is_stack, "dict {dict_id}");
            }
            // Its ammo is its weapon's.
            let weapon = contents.iter().find(|&&(dict_id, _, _)| {
                matches!(dictionary.item(dict_id).unwrap().category, 8 | 13..=17)
            });
            let ammo = contents.iter().find(|&&(_, _, is_stack)| is_stack);
            let (weapon, ammo) = (weapon.unwrap().0, ammo.unwrap().0);
            assert!(dictionary.compatible(weapon, ammo), "{bundle:?}");
            assert!(bundle.price(dictionary) > 0);
        }
        let price = |id: u8| {
            BUNDLES
                .iter()
                .find(|bundle| bundle.bundle_id == id)
                .unwrap()
                .price(dictionary)
        };
        // The wiki's seven gold prices of January 2015 in silver, by the
        // bundles' configs.
        let cfg = |id: u8| {
            let bundle = dictionary.bundles.iter().find(|b| b.id == id).unwrap();
            bundle
                .cfg
                .trim_start_matches("gameplay/bundles/")
                .trim_end_matches("_bundle.options")
                .to_owned()
        };
        let mut priced: Vec<(String, u32)> = BUNDLES
            .iter()
            .map(|bundle| (cfg(bundle.bundle_id), price(bundle.bundle_id)))
            .collect();
        priced.sort();
        let expected = [
            ("ak74n", 150_000),
            ("blackmarket_starter", 200_000),
            ("izh27", 30_000),
            ("scavenger_starter", 200_000),
            ("sr3m", 500_000),
            ("vityaz", 70_000),
            ("vss", 500_000),
        ];
        assert_eq!(priced.len(), expected.len());
        for ((name, silver), (expected_name, expected_silver)) in priced.iter().zip(expected) {
            assert_eq!((name.as_str(), *silver), (expected_name, expected_silver));
        }
    }

    #[test]
    fn an_offer_is_found_by_its_bundle_id() {
        assert_eq!(offer(7), Some(Offer::Premium { index: 6, days: 1 }));
        assert_eq!(
            offer(6),
            Some(Offer::Premium {
                index: 0,
                days: 365
            })
        );
        assert!(matches!(offer(9), Some(Offer::Bundle(bundle)) if bundle.bundle_id == 9));
        assert_eq!(offer(0), None);
        assert_eq!(offer(15), None);
    }

    #[test]
    fn the_records_are_the_premium_times_then_the_bundles() {
        let records = records(&DEFAULT_PREMIUM_PRICES);
        assert_eq!(records.len(), 14);
        assert!(records[..7].iter().all(|r| r.kind == KIND_PREMIUM));
        assert!(records[7..].iter().all(|r| r.kind == KIND_BUNDLE));
        assert!(records.iter().all(|r| r.money_type == SILVER));
        assert!(records.iter().all(|r| r.id == r.ui_key));
        // A day, the last of the list (the window's first).
        assert_eq!((records[6].id, records[6].value), (7, 10_000));
        // 90 days: 2,500 gold in silver, the one price with a source.
        assert_eq!((records[2].id, records[2].value), (4, 250_000));
        assert_eq!(PREMIUM_OFFERS[2], (4, 90));
        // The owner's prices for the other times; a longer time costs
        // more in all and less a day.
        assert_eq!(
            DEFAULT_PREMIUM_PRICES,
            [800_000, 450_000, 250_000, 100_000, 40_000, 20_000, 10_000]
        );
        for pair in PREMIUM_OFFERS
            .windows(2)
            .zip(DEFAULT_PREMIUM_PRICES.windows(2))
        {
            let ((longer, shorter), prices) = ((pair.0[0].1, pair.0[1].1), pair.1);
            assert!(longer > shorter && prices[0] > prices[1]);
            assert!(prices[0] * shorter < prices[1] * longer);
        }
        assert!(records[6].items.is_empty());
        // The AK-74N bundle: the rifle and 2000 rounds.
        let ak = records.iter().find(|r| r.id == 9).unwrap();
        assert_eq!(ak.value, 150_000);
        assert_eq!(
            ak.items
                .iter()
                .map(|item| (item.dict_id, item.amount, item.kind))
                .collect::<Vec<_>>(),
            [(338, 1, 0), (7, 2000, 0)]
        );
    }
}

//! The 8A shop table and the money rules of buy / sell / ammo / pockets / modules.
//!
//! 8A record: u32 a (+0x2F0), u32 b (+0x2F4), u16 dict, u16 c (+0x2F8), u8 d (+0x2FC), u8 e, 2 pad.
//! Real prices are the fan-wiki prices of data/shop_069d.tsv (May 2022), not client data. Rows whose `source` is `choice`
//! are our own prices (CHOICE, no 0.69d or wiki price: the pocket consumables of docs/notes/store.md
//! section 5); they load the same way, so an operator changes them in the shop tsv of the `data_dir`.

use std::collections::{BTreeMap, HashMap};

use crate::items::{Item, ItemDb};
use crate::tsv::{Table, int_of_float};

/// One 8A record without the dict id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Price {
    /// buy price (silver, gold for kind 1; spare parts for modules)
    pub a: u32,
    /// sell base (silver)
    pub b: u32,
    /// gold price (modules: BUY_MODULE_PREMIUM)
    pub c: u32,
    /// 0 silver item, 1 gold item
    pub kind: u8,
    /// listed in the shop (hidden items keep a record but e = 0)
    pub for_sale: bool,
}

/// Which wallet pays: silver, gold or spare parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wallet {
    Silver = 0,
    Gold = 1,
    Parts = 2,
    /// league tokens (8A kind 7, the league shop)
    League = 3,
}

#[derive(Debug, Default)]
pub struct ShopTable {
    prices: BTreeMap<u16, Price>,
    clip: HashMap<u16, u32>,
    ammo_unit: HashMap<u16, u32>,
    /// number of items that took a wiki price
    pub real_prices: usize,
    /// items priced by a `choice` row of the tsv (our own price, not a 0.69d or wiki source)
    choice: std::collections::BTreeSet<u16>,
}

/// Python `int(round(x))` (round half to even).
fn py_round(x: f64) -> i64 {
    x.round_ties_even() as i64
}

fn default_price(it: &Item) -> u32 {
    // the dictionary's own cost when it has one, else by item level
    if let Some(c) = it.own_silver_cost() {
        return c;
    }
    let lvl = it.level_min1();
    let c = it.category;
    if crate::items::AMMO_CATS.contains(&c) {
        return 0; // ammo: cost 0 in game.options, supplied
    }
    if crate::items::CONSUMABLE_CATS.contains(&c) {
        return 20 * lvl;
    }
    (if crate::items::WEAPON_CATS.contains(&c) { 3000 } else { 1500 }) * lvl * lvl
}

/// Gold price (8A `c`) of a module whose spare-parts price is `a`: a / 20, integer division (200 parts = 10 gold,
/// 100 = 5). CHOICE: no gold price of modules exists in the 0.69d client or data, the original server sent it in
/// 8A `c`.
pub fn module_gold_price(a: u32) -> u32 {
    a / 20
}

/// 8A kind of a league shop item: the client's `is_league_item` (`d == 7`, 0x1C0123).
pub const LEAGUE_KIND: u8 = 7;

impl ShopTable {
    /// Put the league shop items in: price = league tokens (`a`), kind 7, listed. A league item cannot be sold for
    /// silver (`b` = 0: tokens are not convertible; CHOICE).
    pub fn apply_league(&mut self, shop: &[crate::league::ShopItem]) {
        for s in shop {
            self.prices.insert(s.dict_id, Price { a: s.tokens, b: 0, c: 0, kind: LEAGUE_KIND, for_sale: true });
        }
    }

    /// Build the table: dictionary defaults, wiki prices (`shop_tsv`), module prices.
    pub fn build(items: &ItemDb, shop_tsv: &str) -> ShopTable {
        let mut s = ShopTable::default();
        // defaults for every armour, weapon, ammo and consumable row
        for it in items.iter() {
            let c = it.category;
            let wanted = crate::items::ARMOUR_CATS.contains(&c)
                || crate::items::WEAPON_CATS.contains(&c)
                || crate::items::AMMO_CATS.contains(&c)
                || crate::items::CONSUMABLE_CATS.contains(&c);
            if !wanted {
                continue;
            }
            let p = if it.is_premium() {
                let l = it.level_min1();
                Price { a: 50 * l, b: 12 * l, c: 0, kind: 1, for_sale: true }
            } else {
                let p = default_price(it);
                Price { a: p, b: p / 4, c: 0, kind: 0, for_sale: true }
            };
            s.prices.insert(it.dict_id, p);
        }
        s.load_real_prices(items, shop_tsv);
        // weapon modules: a = spare-parts price = the dictionary `cost` (currency 4); a module whose `cost` is 0 is not
        // sold (drops only, c = 0 too). c = gold price (60 01 action 3, the workshop's "shop" entry) = a / 20:
        // CHOICE, no source in the 0.69d client or data (docs/notes/workshop.md section 2).
        for it in items.iter() {
            if items.is_module(it.dict_id) {
                let a = it.cost_int();
                s.prices.insert(it.dict_id, Price { a, b: 0, c: module_gold_price(a), kind: 0, for_sale: a > 0 });
            }
        }
        // vouchers (category 27), boosters (category 23) and premium ammo boxes (37 / 38): the dictionary's own
        // `cost` + `currency` (1 = gold, 0 = silver); a row with cost 0 is not sold. Boxes: game.options 3 / 7 / 10
        // gold (primary) and 2 / 3 / 5 (pistol); OFFICIAL 2020-01-16 "Each box has a price of 10 gold"
        for it in items.iter() {
            if !matches!(it.category, 23 | 27 | 37 | 38) {
                continue;
            }
            let cost = it.cost_int();
            let p = match it.currency.as_str() {
                "1" if cost > 0 => Price { a: cost, b: 0, c: 0, kind: 1, for_sale: true },
                "0" | "" if cost > 0 => Price { a: cost, b: cost / 4, c: 0, kind: 0, for_sale: true },
                _ => Price { a: 0, b: 0, c: 0, kind: 0, for_sale: false },
            };
            s.prices.insert(it.dict_id, p);
        }
        s
    }

    fn load_real_prices(&mut self, items: &ItemDb, shop_tsv: &str) {
        let t = Table::parse(shop_tsv);
        let mut real: HashMap<u16, (f64, String)> = HashMap::new();
        for r in &t.rows {
            let g = |n: &str| t.get(r, n);
            let Ok(did) = g("dict_id").trim().parse::<u16>() else { continue };
            let clip_cell = g("dict_clip_size");
            if let Some(c) = int_of_float(clip_cell).filter(|_| !clip_cell.is_empty()) {
                self.clip.insert(did, c.max(0) as u32);
            }
            if g("sold") == "1" && !g("price").is_empty() {
                let price: f64 = g("price").trim().parse().unwrap_or(0.0);
                if g("source").trim() == "choice" {
                    self.choice.insert(did);
                }
                real.insert(did, (price, g("currency").to_string()));
                if !clip_cell.is_empty() {
                    let clip: f64 = clip_cell.trim().parse().unwrap_or(0.0);
                    self.ammo_unit.insert(did, py_round(price * clip).max(0) as u32);
                }
            }
        }
        let dids: Vec<u16> = self.prices.keys().copied().collect();
        for did in dids {
            let old = self.prices[&did];
            if items.is_ammo(did) {
                // ammo is free (CHOICE: its cost is the weapon's match participation cost, 0.40
                // "The cost of ammo is included in the cost of weapons"): price 0, sells for 0, never bought in the shop
                // ([`ShopTable::buy_total`]). The record stays listed (for_sale 1): the lobby movie's ammo window takes
                // its choices from the shop's sellable items (AmmoShareControll.showShareComponent ->
                // `_shop.forEachAmmo(findAviableItems)`; createShopItems keeps only `is_sellable` records)
                self.prices.insert(did, Price { a: 0, b: 0, c: 0, kind: 0, for_sale: true });
            } else if let Some((price, cur)) = real.get(&did) {
                let price = py_round(*price).max(0) as u32;
                let p = if cur == "gold" {
                    Price { a: price, b: old.b, c: 0, kind: 1, for_sale: true }
                } else {
                    Price { a: price, b: price / 4, c: 0, kind: 0, for_sale: true }
                };
                self.prices.insert(did, p);
                if !self.choice.contains(&did) {
                    self.real_prices += 1;
                }
            } else {
                // no real price (wiki row or dictionary cost): not sold; the default price above is only a placeholder
                self.prices.insert(did, Price { for_sale: false, ..old });
            }
        }
    }

    /// The item's price is our own CHOICE (a `choice` row of the shop tsv), not a 0.69d or wiki price.
    pub fn is_choice(&self, did: u16) -> bool {
        self.choice.contains(&did)
    }

    /// The items whose price is a CHOICE, in dict id order.
    pub fn choices(&self) -> impl Iterator<Item = u16> + '_ {
        self.choice.iter().copied()
    }

    pub fn price(&self, did: u16) -> Option<&Price> {
        self.prices.get(&did)
    }

    /// All records in dict id order (the 8A message).
    pub fn iter(&self) -> impl Iterator<Item = (u16, &Price)> {
        self.prices.iter().map(|(d, p)| (*d, p))
    }

    pub fn len(&self) -> usize {
        self.prices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.prices.is_empty()
    }

    /// ammo `clip_size` (rounds per price unit), 1 when unknown or 0
    pub fn clip(&self, did: u16) -> u32 {
        self.clip.get(&did).copied().unwrap_or(1).max(1)
    }

    /// Silver for `rounds` of ammo: whole `clip_size` units at the unit price (the client's slider formula).
    pub fn ammo_cost(&self, did: u16, rounds: u32) -> u64 {
        let c = self.clip(did) as u64;
        (rounds as u64).div_ceil(c) * self.price(did).map(|p| p.a as u64).unwrap_or(0)
    }

    /// Wallet that pays for an item (kind 1 = gold).
    pub fn wallet(&self, did: u16) -> Wallet {
        match self.price(did) {
            Some(p) if p.kind == 1 => Wallet::Gold,
            Some(p) if p.kind == LEAGUE_KIND => Wallet::League,
            _ => Wallet::Silver,
        }
    }

    /// Shop buy 46 00: total price of `count` units, None when the item has no price record (refused).
    /// Ammo: None (free, not sold: the ammo slider supplies it); consumables: price x max(1, count); everything else: the
    /// price once.
    pub fn buy_total(&self, items: &ItemDb, did: u16, count: u32) -> Option<u64> {
        if items.is_ammo(did) {
            return None;
        }
        let p = self.price(did).filter(|p| p.for_sale)?;
        let n = count.max(1);
        Some(if items.is_consumable(did) || items.is_ammo_booster(did) {
            p.a as u64 * n as u64
        } else {
            p.a as u64
        })
    }

    /// Sell 46 01: returns (amount sold, silver gained). `owned` = the stack count (or 1);
    /// stacks sell `min(requested, owned)`, everything else exactly 1. Stacks pay per whole clip_size unit.
    pub fn sell_gain(&self, items: &ItemDb, did: u16, requested: u32, owned: u32) -> (u32, u64) {
        let b = self.price(did).map(|p| p.b as u64).unwrap_or(0);
        if items.is_stackable(did) {
            let amount = requested.min(owned);
            (amount, (amount / self.clip(did)) as u64 * b)
        } else {
            (1, b)
        }
    }

    /// 45 00 ammo slider: rounds `short` that must be bought to load an ammo slot beyond the stack.
    /// Returns (silver cost, rounds added to the stack = whole clips).
    pub fn ammo_top_up(&self, did: u16, short: u32) -> (u64, u32) {
        let c = self.clip(did);
        (self.ammo_cost(did, short), short.div_ceil(c) * c)
    }

    /// Is the item listed (8A `for_sale` = 1: the client builds a shop / pocket tile for it)?
    pub fn listed(&self, did: u16) -> bool {
        self.price(did).is_some_and(|p| p.for_sale)
    }

    /// A pocket item's price x `short` units, paid in the item's wallet: the 45 00 pocket buy and the after-match pocket
    /// charge. An item that is not listed (no 0.69d, wiki or `choice` price, e.g. the event specials Nut 1355 and Snowball
    /// 1884) costs 0: its dictionary default is only a placeholder and the client shows no price for it (CHOICE:
    /// unlisted pocket items used in a match are free).
    pub fn pocket_cost(&self, did: u16, short: u32) -> (Wallet, u64) {
        let a = self.price(did).filter(|p| p.for_sale).map(|p| p.a as u64).unwrap_or(0);
        (self.wallet(did), a * short as u64)
    }
}

//! The premium account, the premium shop and the items bought with gold
//! (`docs/notes/lobby.md` §2.17, §2.18; `lobby_requests.md`
//! `47 02`, `47 05`, `47 06`, `46 04`).
//!
//! - **The shop's offers** are `survarium::v026g::shop_offers`: premium
//!   time and the seven bundles, each named by its bundle id. Buying one
//!   ([`buy_offer`]) takes its gold; premium time is added to what is left
//!   ([`add_premium`]), a bundle's items go to the inventory.
//! - **A discount** is the operator's (`SRP_PREMIUM_DISCOUNT`, percent,
//!   default 0: none). With one, a player without premium is offered it at
//!   sign-in (`A9`, answered by `47 06`) and after a match together with
//!   the premium half of the match's pay (`A8`, answered by `47 05`); the
//!   price is the client's own arithmetic, [`discounted`].
//! - **Premium items one by one** ([`buy_item`], `47 00` for an item of
//!   `shop_offers::premium_price_list`): paid in gold.
//! - **Using an item** ([`use_item`], `46 04`): the lobby sends it only
//!   for a voucher of type 3, the one-day premium voucher
//!   (`InventoryList` / `ItemCard`: `usable = category == 27 && (type == 3
//!   || type == 0)`, and type 0, the rename voucher, opens the rename
//!   window instead; PROVEN). It adds the voucher's `ttl_in_hours`, 24.
//!
//! **Gold has no real-money source in srp.** It comes from a new
//! account's start, the levels' rewards, the tutorial's notes, the daily
//! bonus, drops and the operator's web page. The client's "add gold"
//! buttons open a web shop srp does not have.
//!
//! The premium account itself: half again of a match's pay
//! ([`super::rewards`]). Not the third kit: every account has all three
//! since 2026-10-17 ([`Account::kit_usable`]).

use std::sync::OnceLock;

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lobby::requests::{INVENTORY_SLOT, Relocation};
use survarium::v026g::lobby::{Bought, FULL_CONDITION, ShopRecord};
use survarium::v026g::shop_offers::{self, DEFAULT_PREMIUM_PRICES, Offer, PREMIUM_OFFERS};

use super::account::Account;
use super::rules::{self, ActionError};

/// The voucher `46 04` uses, and the premium time it gives
/// (`gameplay/items/vouchers/premium_access_voucher_1d.options`:
/// `voucher_type: 3`, `ttl_in_hours: 24`).
pub const PREMIUM_VOUCHER: &str = "premium_voucher_1d";
pub const PREMIUM_VOUCHER_HOURS: u32 = 24;
/// The voucher a rename may be paid with (`voucher_type: 0`).
pub const RENAME_VOUCHER: &str = "rename_account_voucher";
/// The most a discount may be, in percent: the client divides by what is
/// left of the price (`LobbyMenu.show_premium_discount`).
pub const MAX_DISCOUNT_PERCENT: u8 = 90;
/// Most items one purchase of a premium item that is no stack makes, as
/// the shop's ([`rules::MAX_BOUGHT_AT_ONCE`]).
const MAX_BOUGHT_AT_ONCE: u32 = rules::MAX_BOUGHT_AT_ONCE;

/// What the operator may set of these features (the environment, read
/// once at start; every value has a default).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Gold for each premium offer, in the order of
    /// [`shop_offers::PREMIUM_OFFERS`] (longest first).
    /// `SRP_PREMIUM_PRICES`: seven numbers, **shortest first** (a day,
    /// 3 days, a week, a month, 3, 6 months, a year).
    pub premium_prices: [u32; 7],
    /// `SRP_PREMIUM_DISCOUNT`: percent off premium time for a player
    /// without premium, 0..=90; 0 offers none.
    pub discount_percent: u8,
    /// `SRP_PREMIUM_OFFER_SECONDS`: how long the offer after a match
    /// stands (the client counts it down on its premium button).
    pub offer_seconds: u32,
    /// `SRP_RENAME_COST`: gold for a nickname change (0x8D's fourth
    /// price). Default: what the rename voucher costs in the dictionary.
    pub rename_cost: u32,
    /// `SRP_AMMO_USE=off`: a match takes no ammo off the account
    /// ([`super::autobuy`]).
    pub ammo_use: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let rename_cost = Dictionary::shared()
            .item_named(RENAME_VOUCHER)
            .map_or(0, |item| item.base_cost);
        Self {
            premium_prices: DEFAULT_PREMIUM_PRICES,
            discount_percent: 0,
            offer_seconds: 3600,
            rename_cost,
            ammo_use: true,
        }
    }
}

/// `on` / `off` (also `1` / `0`, `true` / `false`).
fn switch(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "on" | "1" | "true" | "yes" => Some(true),
        "off" | "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

/// `SRP_PREMIUM_PRICES`: seven prices over 0, shortest time first.
fn parse_prices(value: &str) -> Option<[u32; 7]> {
    let prices: Vec<u32> = value
        .split(',')
        .map(|price| price.trim().parse::<u32>().ok().filter(|&price| price > 0))
        .collect::<Option<_>>()?;
    let mut prices: [u32; 7] = prices.try_into().ok()?;
    prices.reverse();
    Some(prices)
}

impl Settings {
    /// The settings `var` gives (the environment's `std::env::var`, or a
    /// test's).
    pub fn read(var: impl Fn(&str) -> Option<String>) -> Self {
        let mut settings = Self::default();
        if let Some(value) = var("SRP_PREMIUM_PRICES") {
            match parse_prices(&value) {
                Some(prices) => settings.premium_prices = prices,
                None => log::error!(
                    "SRP_PREMIUM_PRICES is `{value}`; it must be seven prices over 0, for a day, \
                     3 days, a week, a month, 3 months, 6 months and a year. Using the defaults"
                ),
            }
        }
        if let Some(value) = var("SRP_PREMIUM_DISCOUNT") {
            match value.trim().parse::<u8>() {
                Ok(percent) if percent <= MAX_DISCOUNT_PERCENT => {
                    settings.discount_percent = percent;
                }
                _ => log::error!(
                    "SRP_PREMIUM_DISCOUNT is `{value}`; it must be a percent of 0 to \
                     {MAX_DISCOUNT_PERCENT}. No discount"
                ),
            }
        }
        if let Some(value) = var("SRP_PREMIUM_OFFER_SECONDS") {
            match value.trim().parse::<u32>() {
                Ok(seconds) if seconds > 0 => settings.offer_seconds = seconds,
                _ => log::error!(
                    "SRP_PREMIUM_OFFER_SECONDS is `{value}`; it must be a number of seconds over \
                     0. Using {}",
                    settings.offer_seconds
                ),
            }
        }
        if let Some(value) = var("SRP_RENAME_COST") {
            match value.trim().parse::<u32>() {
                Ok(cost) => settings.rename_cost = cost,
                Err(_) => log::error!(
                    "SRP_RENAME_COST is `{value}`; it must be a price in gold. Using {}",
                    settings.rename_cost
                ),
            }
        }
        for (name, field) in [("SRP_AMMO_USE", &mut settings.ammo_use)] {
            if let Some(value) = var(name) {
                match switch(&value) {
                    Some(on) => *field = on,
                    None => log::error!("{name} is `{value}`; it must be `on` or `off`. Using on"),
                }
            }
        }
        settings
    }

    /// The discount as the share the wire carries.
    pub fn discount(&self) -> f32 {
        f32::from(self.discount_percent) / 100.0
    }

    /// The records of 0x95 at these prices.
    pub fn shop_records(&self) -> Vec<ShopRecord> {
        shop_offers::records(&self.premium_prices)
    }
}

/// The settings of this srp: the environment's, read at first use.
pub fn settings() -> &'static Settings {
    static SHARED: OnceLock<Settings> = OnceLock::new();
    SHARED.get_or_init(|| Settings::read(|name| std::env::var(name).ok()))
}

/// Why the premium shop refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopError {
    /// No offer with that id.
    NoSuchOffer,
    /// A discount the server did not offer.
    NotOffered,
    NotEnoughGold,
    /// The item is not in the inventory.
    NoSuchItem,
    /// The item is not one `46 04` uses.
    NotUsable,
}

impl ShopError {
    /// The game's own text for the refusal.
    pub fn text_key(&self) -> &'static str {
        match self {
            Self::NoSuchOffer | Self::NotOffered => "st_cannot_buy_item",
            Self::NotEnoughGold => "st_cant_buy_premium_message",
            Self::NoSuchItem | Self::NotUsable => "st_error_while_using_premium_access_voucher",
        }
    }
}

impl std::fmt::Display for ShopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?} ({})", self.text_key())
    }
}

impl std::error::Error for ShopError {}

/// Adds premium time: to what is left, or from `now` (Unix seconds).
pub fn add_premium(account: &mut Account, seconds: u64, now: u64) {
    account.premium_until = account.premium_until.max(now).saturating_add(seconds);
}

/// A price with a discount, as the client shows it: `value * (1 -
/// discount) + 0.5`, cut to a whole number (0x28A547, 0x28AC6E; PROVEN).
pub fn discounted(price: u32, discount: f32) -> u32 {
    (f64::from(price) * (1.0 - f64::from(discount)) + 0.5) as u32
}

/// Puts `units` of `dict_id` into the inventory: onto the stack owned, or
/// as a new item (a new item for each unit of what is no stack). Returns
/// what `83 47 00` would list for it.
pub fn grant(account: &mut Account, dict_id: u16, units: u32) -> Vec<Bought> {
    if Dictionary::shared().is_stack(dict_id) {
        let owned = account
            .inventory
            .iter_mut()
            .find(|item| item.dict_id == dict_id);
        let item_id = match owned {
            Some(owned) => {
                owned.condition_or_stack = owned.condition_or_stack.saturating_add(units);
                owned.item_id
            }
            None => account.add_item(dict_id, units),
        };
        vec![Bought {
            dict_id,
            item_id,
            value: units,
        }]
    } else {
        (0..units)
            .map(|_| Bought {
                dict_id,
                item_id: account.add_item(dict_id, FULL_CONDITION),
                value: FULL_CONDITION,
            })
            .collect()
    }
}

/// What an offer gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gave {
    Premium {
        days: u32,
    },
    /// The bundle's items, as they are in the inventory now.
    Bundle {
        bundle_id: u8,
        items: Vec<Bought>,
    },
}

/// A purchase of the premium shop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Purchase {
    /// The gold taken.
    pub cost: u32,
    pub gave: Gave,
}

/// `47 02` (and `47 05`, `47 06` with a `discount` the server offered):
/// buy the offer `offer_id` at `now`. A discount is taken off premium time
/// only: the client lists no bundle with one.
pub fn buy_offer(
    account: &mut Account,
    settings: &Settings,
    offer_id: u32,
    discount: f32,
    now: u64,
) -> Result<Purchase, ShopError> {
    let dictionary = Dictionary::shared();
    let offer = shop_offers::offer(offer_id).ok_or(ShopError::NoSuchOffer)?;
    let cost = match offer {
        Offer::Premium { index, .. } => discounted(settings.premium_prices[index], discount),
        Offer::Bundle(_) if discount != 0.0 => return Err(ShopError::NotOffered),
        Offer::Bundle(bundle) => bundle.price(dictionary),
    };
    if account.premium_money < cost {
        return Err(ShopError::NotEnoughGold);
    }
    account.premium_money -= cost;
    let gave = match offer {
        Offer::Premium { days, .. } => {
            add_premium(account, u64::from(days) * 86_400, now);
            Gave::Premium { days }
        }
        Offer::Bundle(bundle) => Gave::Bundle {
            bundle_id: bundle.bundle_id,
            items: bundle
                .contents(dictionary)
                .into_iter()
                .flat_map(|(dict_id, units, _)| grant(account, dict_id, units))
                .collect(),
        },
    };
    Ok(Purchase { cost, gave })
}

/// Whether `dict_id` is a premium item the shop sells one by one (for
/// gold).
pub fn sells(dict_id: u16) -> bool {
    shop_offers::premium_price(dict_id).is_some()
}

/// `47 00` for a premium item: buy `count` of it for gold. With a
/// `target_slot` that is a kit's slot the first item bought goes into it,
/// as the shop's own purchase does ([`rules::buy`]). All or nothing.
pub fn buy_item(
    account: &mut Account,
    dict_id: u16,
    count: u32,
    target_slot: u8,
    kit_id: u32,
    now: u64,
) -> Result<Vec<Bought>, ActionError> {
    let dictionary = Dictionary::shared();
    let item = dictionary.item(dict_id).ok_or(ActionError::NotSold)?;
    let price = shop_offers::premium_price(dict_id).ok_or(ActionError::NotSold)?;
    if count == 0 || (!item.is_stack && count > MAX_BOUGHT_AT_ONCE) {
        return Err(ActionError::BadCount);
    }
    let cost = u64::from(price.cost) * u64::from(count);
    if cost > u64::from(account.premium_money) {
        return Err(ActionError::NotEnoughPremiumMoney);
    }
    let mut changed = account.clone();
    changed.premium_money -= cost as u32;
    let bought = grant(&mut changed, dict_id, count);
    if target_slot != INVENTORY_SLOT {
        let first = bought[0];
        let relocation = Relocation {
            item_id: first.item_id,
            dict_id,
            source_slot: INVENTORY_SLOT,
            target_slot,
            count: if item.is_stack { first.value } else { 1 },
            count_in_inventory: 0,
        };
        rules::move_items(&mut changed, kit_id, &[relocation], now)?;
    }
    *account = changed;
    Ok(bought)
}

/// What using an item did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Used {
    pub item_id: u32,
    /// How many of the item are left.
    pub left: u32,
    /// The premium hours it added.
    pub premium_hours: u32,
}

/// `46 04 <item>`: use one of an item. Only the one-day premium voucher is
/// used this way (see the module).
pub fn use_item(account: &mut Account, item_id: u32, now: u64) -> Result<Used, ShopError> {
    let dictionary = Dictionary::shared();
    let index = account
        .inventory
        .iter()
        .position(|item| item.item_id == item_id)
        .ok_or(ShopError::NoSuchItem)?;
    let item = &mut account.inventory[index];
    let voucher = dictionary.item_named(PREMIUM_VOUCHER);
    if voucher.is_none_or(|voucher| voucher.dict_id != item.dict_id) || item.condition_or_stack == 0
    {
        return Err(ShopError::NotUsable);
    }
    item.condition_or_stack -= 1;
    let left = item.condition_or_stack;
    if left == 0 {
        account.inventory.remove(index);
        account.sync_kits();
    }
    add_premium(account, u64::from(PREMIUM_VOUCHER_HOURS) * 3600, now);
    Ok(Used {
        item_id,
        left,
        premium_hours: PREMIUM_VOUCHER_HOURS,
    })
}

/// The days of the premium offer `offer_id`, if it is one.
pub fn premium_days(offer_id: u32) -> Option<u32> {
    PREMIUM_OFFERS
        .iter()
        .find(|&&(id, _)| u32::from(id) == offer_id)
        .map(|&(_, days)| days)
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::golden;
    use survarium::v026g::lobby;
    use survarium::v026g::lobby::client_model::{Client, Decoded};

    const NOW: u64 = 1_000_000;

    fn dict(name: &str) -> u16 {
        Dictionary::shared().item_named(name).unwrap().dict_id
    }

    fn rich(gold: u32) -> Account {
        let mut account = Account::new("Alice");
        account.premium_money = gold;
        account
    }

    #[test]
    fn the_settings_have_defaults_and_take_the_operators() {
        let none = Settings::read(|_| None);
        assert_eq!(none, Settings::default());
        assert_eq!(none.premium_prices, DEFAULT_PREMIUM_PRICES);
        assert_eq!(none.discount_percent, 0);
        assert_eq!(none.offer_seconds, 3600);
        // The rename voucher's own price.
        assert_eq!(none.rename_cost, 250);
        assert!(none.ammo_use);

        let set = Settings::read(|name| {
            Some(
                match name {
                    "SRP_PREMIUM_PRICES" => "10, 25,50,150,400,700,1200",
                    "SRP_PREMIUM_DISCOUNT" => "25",
                    "SRP_PREMIUM_OFFER_SECONDS" => "600",
                    "SRP_RENAME_COST" => "0",
                    "SRP_AMMO_USE" => "OFF",
                    _ => return None,
                }
                .to_owned(),
            )
        });
        // Longest first inside.
        assert_eq!(set.premium_prices, [1200, 700, 400, 150, 50, 25, 10]);
        assert_eq!(set.discount_percent, 25);
        assert_eq!(set.discount(), 0.25);
        assert_eq!(set.offer_seconds, 600);
        assert_eq!(set.rename_cost, 0);
        assert!(!set.ammo_use);

        // What is no value is not used.
        let bad = Settings::read(|name| {
            Some(
                match name {
                    "SRP_PREMIUM_PRICES" => "10,20,30",
                    "SRP_PREMIUM_DISCOUNT" => "100",
                    "SRP_PREMIUM_OFFER_SECONDS" => "0",
                    "SRP_RENAME_COST" => "free",
                    "SRP_AMMO_USE" => "maybe",
                    _ => return None,
                }
                .to_owned(),
            )
        });
        assert_eq!(bad, Settings::default());
        assert_eq!(parse_prices("1,2,3,4,5,6,0"), None);
    }

    #[test]
    fn every_refusal_has_a_text_of_the_game() {
        for error in [
            ShopError::NoSuchOffer,
            ShopError::NotOffered,
            ShopError::NotEnoughGold,
            ShopError::NoSuchItem,
            ShopError::NotUsable,
        ] {
            assert!(error.text_key().starts_with("st_"), "{error}");
        }
        assert_eq!(
            ShopError::NotEnoughGold.text_key(),
            "st_cant_buy_premium_message"
        );
    }

    #[test]
    fn the_shop_goes_out_in_a_form_the_client_takes() {
        let records = Settings::default().shop_records();
        let message = lobby::shop(&records).unwrap();
        let mut client = Client::signed_in();
        let Decoded::Shop(read) = client.receive(&message).unwrap() else {
            panic!("not the shop");
        };
        assert_eq!(read, records);
        assert_eq!(read.len(), 14);
        // `95`, 14 records, the stored stream of 14 * 0xD0 bytes.
        let body = message.body();
        assert_eq!(body[..4], [14, 0, 0, 0]);
        let packed = u32::from_le_bytes([body[4], body[5], body[6], body[7]]) as usize;
        assert_eq!(body.len(), 8 + packed);
        assert!(packed > 14 * 0xD0);
    }

    #[test]
    fn premium_time_is_bought_for_gold_and_adds_up() {
        let settings = Settings::default();
        let mut account = rich(350);
        // A day: 100 gold.
        let bought = buy_offer(&mut account, &settings, 7, 0.0, NOW).unwrap();
        assert_eq!(
            bought,
            Purchase {
                cost: 100,
                gave: Gave::Premium { days: 1 }
            }
        );
        assert_eq!(account.premium_money, 250);
        assert_eq!(account.premium_until, NOW + 86_400);
        assert_eq!(account.premium_hours(NOW), Some(24));
        // Three days more are added to what is left.
        buy_offer(&mut account, &settings, 1, 0.0, NOW + 3600).unwrap();
        assert_eq!(account.premium_until, NOW + 4 * 86_400);
        assert_eq!(account.premium_money, 50);
        // Not the gold for a week; nothing changes.
        let before = account.clone();
        assert_eq!(
            buy_offer(&mut account, &settings, 2, 0.0, NOW),
            Err(ShopError::NotEnoughGold)
        );
        assert_eq!(
            buy_offer(&mut account, &settings, 99, 0.0, NOW),
            Err(ShopError::NoSuchOffer)
        );
        assert_eq!(account, before);
        // Premium that ran out starts again from now.
        account.premium_money = 100;
        buy_offer(&mut account, &settings, 7, 0.0, NOW + 30 * 86_400).unwrap();
        assert_eq!(account.premium_until, NOW + 31 * 86_400);
    }

    #[test]
    fn a_discount_is_the_clients_arithmetic() {
        assert_eq!(discounted(250, 0.0), 250);
        assert_eq!(discounted(250, 0.25), 188);
        assert_eq!(discounted(1_200, 0.25), 900);
        assert_eq!(discounted(600, 0.9), 60);
        let settings = Settings::default();
        let mut account = rich(1_000);
        let bought = buy_offer(&mut account, &settings, 2, 0.25, NOW).unwrap();
        // A week: 400 gold, a quarter off.
        assert_eq!(bought.cost, 300);
        assert_eq!(account.premium_money, 700);
        // No bundle has one.
        assert_eq!(
            buy_offer(&mut account, &settings, 9, 0.25, NOW),
            Err(ShopError::NotOffered)
        );
    }

    #[test]
    fn a_bundle_gives_its_items() {
        let settings = Settings::default();
        let mut account = rich(3_000);
        let before = account.inventory.len();
        // The AK-74N bundle: the rifle and 2000 rounds for 1500 gold.
        let bought = buy_offer(&mut account, &settings, 9, 0.0, NOW).unwrap();
        assert_eq!(bought.cost, 1_500);
        let Gave::Bundle { bundle_id, items } = bought.gave else {
            panic!("not a bundle");
        };
        assert_eq!(bundle_id, 9);
        assert_eq!(items.len(), 2);
        assert_eq!(account.inventory.len(), before + 2);
        let rifle = account.item(items[0].item_id).unwrap();
        assert_eq!(rifle.dict_id, dict("ak_74n_premium"));
        assert_eq!(rifle.condition_or_stack, FULL_CONDITION);
        // A golden weapon: its silver bonus is its own first modification
        // (`golden::starting_modifications`).
        assert_eq!(rifle.modifications, [(golden::SILVER_BONUS, false)]);
        let ammo = account.item(items[1].item_id).unwrap().clone();
        assert_eq!(
            (ammo.dict_id, ammo.condition_or_stack),
            (dict("ammo_5.45x39_7n6m"), 2_000)
        );
        assert_eq!(account.premium_money, 1_500);
        // A second one: another rifle, the rounds onto the stack.
        buy_offer(&mut account, &settings, 9, 0.0, NOW).unwrap();
        assert_eq!(account.inventory.len(), before + 3);
        assert_eq!(
            account.item(ammo.item_id).unwrap().condition_or_stack,
            4_000
        );
        // The starter bundle: jacket, pants, the Kiparis and its rounds,
        // 2000 gold.
        account.premium_money = 2_000;
        let Gave::Bundle { items, .. } = buy_offer(&mut account, &settings, 14, 0.0, NOW)
            .unwrap()
            .gave
        else {
            panic!("not a bundle");
        };
        assert_eq!(items.len(), 4);
        assert_eq!(account.premium_money, 0);
        // The Survivor jacket and pants: their premium bonuses are their own
        // first modifications (`golden::starting_modifications`).
        for (gave, name) in items
            .iter()
            .zip(["scav_torso_4_premium", "scav_legs_4_premium"])
        {
            let piece = account.item(gave.item_id).unwrap();
            assert_eq!(piece.dict_id, dict(name));
            assert_eq!(
                piece.modifications,
                [
                    (golden::EXPERIENCE_BONUS, false),
                    (golden::SCAVENGER_REPUTATION_BONUS, false)
                ]
            );
        }
        // The account as it is now goes out whole.
        let mut client = Client::signed_in();
        let push = super::super::wire::after_change(&account, NOW).unwrap();
        assert!(client.receive_all(&push).is_ok());
    }

    #[test]
    fn premium_items_are_bought_for_gold() {
        let mut account = rich(400);
        let silver = account.generic_money;
        let voucher = dict("repair_voucher");
        assert!(sells(voucher));
        assert!(!sells(dict("ak_74n_premium")) && !sells(dict("tt_33")));
        // Three repair kits at 50 gold: one stack.
        let bought = buy_item(&mut account, voucher, 3, INVENTORY_SLOT, 1, NOW).unwrap();
        assert_eq!(bought.len(), 1);
        assert_eq!((bought[0].dict_id, bought[0].value), (voucher, 3));
        assert_eq!(account.premium_money, 250);
        assert_eq!(account.generic_money, silver);
        // Two more go onto it.
        let more = buy_item(&mut account, voucher, 2, INVENTORY_SLOT, 1, NOW).unwrap();
        assert_eq!(more[0].item_id, bought[0].item_id);
        assert_eq!(
            account.item(bought[0].item_id).unwrap().condition_or_stack,
            5
        );
        assert_eq!(account.premium_money, 150);
        // The client takes the answer.
        let mut client = Client::signed_in();
        client
            .receive_all(&super::super::wire::after_change(&account, NOW).unwrap())
            .unwrap();
        assert!(
            client
                .receive(&lobby::permitted_bought(&more).unwrap())
                .is_ok()
        );
        // Refusals change nothing.
        let before = account.clone();
        assert_eq!(
            buy_item(&mut account, voucher, 0, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::BadCount)
        );
        assert_eq!(
            buy_item(&mut account, voucher, 4, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::NotEnoughPremiumMoney)
        );
        assert_eq!(
            buy_item(&mut account, dict("tt_33"), 1, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::NotSold)
        );
        assert_eq!(account, before);
        // The AK-74N, dragged onto the character: into the weapon's slot.
        account.premium_money = 1_500;
        let rifle = buy_item(&mut account, dict("ak_74n"), 1, 10, 1, NOW).unwrap();
        assert_eq!(account.kits[0].slots[10].unwrap().item_id, rifle[0].item_id);
        assert_eq!(account.premium_money, 0);
        // Into a slot it does not fit: no purchase.
        account.premium_money = 1_500;
        let before = account.clone();
        assert!(buy_item(&mut account, dict("ak_74n"), 1, 1, 1, NOW).is_err());
        assert_eq!(account, before);
    }

    /// `prices::NOT_SOLD` (2026-10-18): the Russian and Belarusian flags
    /// and coats of arms, the flag of the USSR and the red star are bought
    /// neither for gold nor for silver.
    #[test]
    fn the_excluded_flags_and_emblems_are_not_bought() {
        let mut account = rich(10_000);
        account.generic_money = 1_000_000;
        let before = account.clone();
        let names = [
            "decals_4_1",
            "decals_4_2",
            "decals_4_4",
            "decals_5_1",
            "decals_5_2",
            "decals_5_4",
        ];
        for name in names {
            let decal = dict(name);
            assert!(!sells(decal), "{name}");
            assert_eq!(
                buy_item(&mut account, decal, 1, INVENTORY_SLOT, 1, NOW),
                Err(ActionError::NotSold)
            );
            assert_eq!(
                rules::buy(&mut account, decal, 1, 0, INVENTORY_SLOT, 1, NOW),
                Err(ActionError::NotSold)
            );
        }
        assert_eq!(account, before);
        // The flag of Ukraine is, for 300 gold.
        assert!(buy_item(&mut account, dict("decals_4_3"), 1, INVENTORY_SLOT, 1, NOW).is_ok());
        assert_eq!(account.premium_money, 9_700);
    }

    #[test]
    fn the_premium_voucher_is_used_up() {
        let mut account = rich(0);
        let voucher = account.add_item(dict(PREMIUM_VOUCHER), 2);
        let pistol = account.kits[0].slots[7].unwrap().item_id;
        assert_eq!(
            use_item(&mut account, voucher, NOW),
            Ok(Used {
                item_id: voucher,
                left: 1,
                premium_hours: 24
            })
        );
        assert_eq!(account.premium_until, NOW + 86_400);
        // The second day is added; the voucher is gone.
        assert_eq!(use_item(&mut account, voucher, NOW).unwrap().left, 0);
        assert_eq!(account.premium_until, NOW + 2 * 86_400);
        assert!(account.item(voucher).is_none());
        assert_eq!(
            use_item(&mut account, voucher, NOW),
            Err(ShopError::NoSuchItem)
        );
        // Nothing else is used this way.
        let card = account.add_item(dict("drop_card"), 1);
        for item in [pistol, card] {
            assert_eq!(use_item(&mut account, item, NOW), Err(ShopError::NotUsable));
        }
        assert_eq!(account.item(card).unwrap().condition_or_stack, 1);
        // The client's own bookkeeping of the answer: one less.
        let mut client = Client::signed_in();
        let mut account = rich(0);
        let voucher = account.add_item(dict(PREMIUM_VOUCHER), 1);
        client
            .receive_all(&super::super::wire::after_change(&account, NOW).unwrap())
            .unwrap();
        use_item(&mut account, voucher, NOW).unwrap();
        assert!(matches!(
            client.receive(&lobby::permitted_item_used(voucher)).unwrap(),
            Decoded::Permitted(lobby::client_model::Permitted::ItemUsed { item_id }) if item_id == voucher
        ));
        assert!(
            client
                .inventory()
                .iter()
                .all(|item| item.item_id != voucher)
        );
        assert_eq!(premium_days(7), Some(1));
        assert_eq!(premium_days(9), None);
    }
}

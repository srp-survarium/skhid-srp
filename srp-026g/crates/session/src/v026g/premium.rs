//! The premium shop and the premium items, all paid in silver (gold is
//! gone, the owner's of 2026-11-08)
//! (`docs/notes/lobby.md` §2.18; `lobby_requests.md` `47 00`, `47 02`).
//!
//! - **No premium account** (the owner, 2026-11-09: "let's do the same with
//!   premium", "no premium state anywhere should exist", "essentially we
//!   should just increase numbers premium does"): what it paid, half again
//!   of a match's rows, every account is paid, by the rates
//!   (`pay::MONEY_RATES`, `pay::DEFAULT_EXPERIENCE_RATE`,
//!   `pay::DEFAULT_REPUTATION_RATE`); nothing sells, gives or shows premium
//!   time (`docs/notes/premium.md`).
//! - **The shop's offers** are `survarium::v026g::shop_offers`: the seven
//!   bundles, each named by its bundle id. Buying one ([`buy_offer`])
//!   takes its silver; its items go to the inventory.
//! - **Premium items one by one** ([`buy_item`], `47 00` for an item of
//!   `shop_offers::premium_price_list`): paid in silver.
//! - **No vouchers but the drop card** (the owner, 2026-11-08: "premium
//!   ticket and repair kit should be removed too", "remove them
//!   completely"): the one-day premium voucher, the rename voucher and the
//!   repair kit are sold by no shop and dropped by no table
//!   (`scripts/content-edit/shop.tsv`, `vouchers-removed-1.json`), nothing
//!   uses them (`46 04`, the only request that used an item, is not read),
//!   and the release that removed them paid their owners their price in
//!   silver, once (`persist::SqliteGameRepository::pay_back_once`).
//!
//! **There is no gold** (the owner, 2026-11-08): every price that was gold
//! is its gold times `prices::GOLD_IN_SILVER` in silver, and what gave gold
//! gives that silver.

use std::sync::OnceLock;

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lobby::requests::{INVENTORY_SLOT, Relocation};
use survarium::v026g::lobby::{Bought, FULL_CONDITION};
use survarium::v026g::shop_offers;

use super::account::Account;
use super::rules::{self, ActionError};

/// Silver for a nickname change unless `SRP_RENAME_COST` says otherwise:
/// what the rename voucher cost (its 250 gold in silver) before the
/// vouchers were removed.
pub const DEFAULT_RENAME_COST: u32 = 25_000;
/// Most items one purchase of a premium item that is no stack makes, as
/// the shop's ([`rules::MAX_BOUGHT_AT_ONCE`]).
const MAX_BOUGHT_AT_ONCE: u32 = rules::MAX_BOUGHT_AT_ONCE;

/// What the operator may set of these features (the environment, read
/// once at start; every value has a default).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// `SRP_RENAME_COST`: silver for a nickname change (0x8D's fourth
    /// price). Default: [`DEFAULT_RENAME_COST`].
    pub rename_cost: u32,
    /// `SRP_AMMO_USE=off`: a match takes no ammo off the account
    /// ([`super::autobuy`]).
    pub ammo_use: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            rename_cost: DEFAULT_RENAME_COST,
            ammo_use: true,
        }
    }
}

impl Settings {
    /// The settings `var` gives (the environment's `std::env::var`, or a
    /// test's).
    pub fn read(var: impl Fn(&str) -> Option<String>) -> Self {
        let mut settings = Self::default();
        if let Some(value) = var("SRP_RENAME_COST") {
            match value.trim().parse::<u32>() {
                Ok(cost) => settings.rename_cost = cost,
                Err(_) => log::error!(
                    "SRP_RENAME_COST is `{value}`; it must be a price in silver. Using {}",
                    settings.rename_cost
                ),
            }
        }
        let ammo_use = var("SRP_AMMO_USE");
        settings.ammo_use =
            crate::switch_value("SRP_AMMO_USE", ammo_use.as_deref(), settings.ammo_use);
        settings
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
    NotEnoughMoney,
}

impl ShopError {
    /// The game's own text for the refusal.
    pub fn text_key(&self) -> &'static str {
        match self {
            Self::NoSuchOffer => "st_cannot_buy_item",
            Self::NotEnoughMoney => "st_not_enougth_money",
        }
    }
}

impl std::fmt::Display for ShopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?} ({})", self.text_key())
    }
}

impl std::error::Error for ShopError {}

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

/// A purchase of the premium shop: a bundle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Purchase {
    /// The silver taken.
    pub cost: u32,
    pub bundle_id: u8,
    /// The bundle's items, as they are in the inventory now.
    pub items: Vec<Bought>,
}

/// `47 02`: buy the bundle `offer_id`.
pub fn buy_offer(account: &mut Account, offer_id: u32) -> Result<Purchase, ShopError> {
    let dictionary = Dictionary::shared();
    let bundle = shop_offers::offer(offer_id).ok_or(ShopError::NoSuchOffer)?;
    let cost = bundle.price(dictionary);
    if account.generic_money < cost {
        return Err(ShopError::NotEnoughMoney);
    }
    account.generic_money -= cost;
    let items = bundle
        .contents(dictionary)
        .into_iter()
        .flat_map(|(dict_id, units, _)| grant(account, dict_id, units))
        .collect();
    Ok(Purchase {
        cost,
        bundle_id: bundle.bundle_id,
        items,
    })
}

/// Whether `dict_id` is a premium item the shop sells one by one.
pub fn sells(dict_id: u16) -> bool {
    shop_offers::premium_price(dict_id).is_some()
}

/// `47 00` for a premium item: buy `count` of it. With a
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
    if cost > u64::from(account.generic_money) {
        return Err(ActionError::NotEnoughMoney);
    }
    let mut changed = account.clone();
    changed.generic_money -= cost as u32;
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

    fn rich(silver: u32) -> Account {
        let mut account = Account::new("Alice");
        account.generic_money = silver;
        account
    }

    #[test]
    fn the_settings_have_defaults_and_take_the_operators() {
        let none = Settings::read(|_| None);
        assert_eq!(none, Settings::default());
        // What the rename voucher cost: its 250 gold, in silver.
        assert_eq!(none.rename_cost, 25_000);
        assert!(none.ammo_use);

        let set = Settings::read(|name| {
            Some(
                match name {
                    "SRP_RENAME_COST" => "0",
                    "SRP_AMMO_USE" => "OFF",
                    _ => return None,
                }
                .to_owned(),
            )
        });
        assert_eq!(set.rename_cost, 0);
        assert!(!set.ammo_use);

        // What is no value is not used.
        let bad = Settings::read(|name| {
            Some(
                match name {
                    "SRP_RENAME_COST" => "free",
                    "SRP_AMMO_USE" => "maybe",
                    _ => return None,
                }
                .to_owned(),
            )
        });
        assert_eq!(bad, Settings::default());
    }

    #[test]
    fn every_refusal_has_a_text_of_the_game() {
        for error in [ShopError::NoSuchOffer, ShopError::NotEnoughMoney] {
            assert!(error.text_key().starts_with("st_"), "{error}");
        }
        assert_eq!(
            ShopError::NotEnoughMoney.text_key(),
            "st_not_enougth_money"
        );
    }

    #[test]
    fn the_shop_goes_out_in_a_form_the_client_takes() {
        let records = shop_offers::records();
        let message = lobby::shop(&records).unwrap();
        let mut client = Client::signed_in();
        let Decoded::Shop(read) = client.receive(&message).unwrap() else {
            panic!("not the shop");
        };
        assert_eq!(read, records);
        assert_eq!(read.len(), 7);
        // `95`, 7 records, the stored stream of 7 * 0xD0 bytes.
        let body = message.body();
        assert_eq!(body[..4], [7, 0, 0, 0]);
        let packed = u32::from_le_bytes([body[4], body[5], body[6], body[7]]) as usize;
        assert_eq!(body.len(), 8 + packed);
        assert!(packed > 7 * 0xD0);
    }

    #[test]
    fn premium_time_is_sold_no_more() {
        let mut account = rich(1_000_000);
        let before = account.clone();
        // A day, three days, a year: the dictionary's premium bundles.
        for offer in [7, 1, 6] {
            assert_eq!(buy_offer(&mut account, offer), Err(ShopError::NoSuchOffer));
        }
        assert_eq!(buy_offer(&mut account, 99), Err(ShopError::NoSuchOffer));
        assert_eq!(account, before);
        // Not the silver for the AK-74N bundle; nothing changes.
        let mut poor = rich(1_000);
        assert_eq!(buy_offer(&mut poor, 9), Err(ShopError::NotEnoughMoney));
        assert_eq!(poor, rich(1_000));
    }

    #[test]
    fn a_bundle_gives_its_items() {
        let mut account = rich(300_000);
        let before = account.inventory.len();
        // The AK-74N bundle: the rifle and 2000 rounds for 1500 gold, in
        // silver.
        let Purchase {
            cost,
            bundle_id,
            items,
        } = buy_offer(&mut account, 9).unwrap();
        assert_eq!(cost, 150_000);
        assert_eq!(bundle_id, 9);
        assert_eq!(items.len(), 2);
        assert_eq!(account.inventory.len(), before + 2);
        let rifle = account.item(items[0].item_id).unwrap();
        // The plain rifle, as any (the owner, 2026-11-14: no golden weapons).
        assert_eq!(rifle.dict_id, dict("ak_74n"));
        assert_eq!(rifle.condition_or_stack, FULL_CONDITION);
        assert!(rifle.modifications.is_empty());
        let ammo = account.item(items[1].item_id).unwrap().clone();
        assert_eq!(
            (ammo.dict_id, ammo.condition_or_stack),
            (dict("ammo_5.45x39_7n6m"), 2_000)
        );
        assert_eq!(account.generic_money, 150_000);
        // A second one: another rifle, the rounds onto the stack.
        buy_offer(&mut account, 9).unwrap();
        assert_eq!(account.inventory.len(), before + 3);
        assert_eq!(
            account.item(ammo.item_id).unwrap().condition_or_stack,
            4_000
        );
        // The starter bundle: jacket, pants, the Kiparis and its rounds,
        // 2000 gold in silver.
        account.generic_money = 200_000;
        let items = buy_offer(&mut account, 14).unwrap().items;
        assert_eq!(items.len(), 4);
        assert_eq!(account.generic_money, 0);
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
        let push = super::super::wire::after_change(&account).unwrap();
        assert!(client.receive_all(&push).is_ok());
    }

    #[test]
    fn premium_items_are_bought_for_silver() {
        let mut account = rich(60_000);
        let voucher = dict("drop_card");
        assert!(sells(voucher));
        assert!(!sells(dict("ak_74n_premium")) && !sells(dict("tt_33")));
        // The removed vouchers are sold no more (the owner, 2026-11-08).
        for removed in ["repair_voucher", "rename_account_voucher", "premium_voucher_1d"] {
            assert!(!sells(dict(removed)), "{removed}");
        }
        // Three drop cards at their 100 gold in silver: one stack.
        let bought = buy_item(&mut account, voucher, 3, INVENTORY_SLOT, 1, NOW).unwrap();
        assert_eq!(bought.len(), 1);
        assert_eq!((bought[0].dict_id, bought[0].value), (voucher, 3));
        assert_eq!(account.generic_money, 30_000);
        // Two more go onto it.
        let more = buy_item(&mut account, voucher, 2, INVENTORY_SLOT, 1, NOW).unwrap();
        assert_eq!(more[0].item_id, bought[0].item_id);
        assert_eq!(
            account.item(bought[0].item_id).unwrap().condition_or_stack,
            5
        );
        assert_eq!(account.generic_money, 10_000);
        // The client takes the answer.
        let mut client = Client::signed_in();
        client
            .receive_all(&super::super::wire::after_change(&account).unwrap())
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
            Err(ActionError::NotEnoughMoney)
        );
        assert_eq!(
            buy_item(&mut account, dict("tt_33"), 1, INVENTORY_SLOT, 1, NOW),
            Err(ActionError::NotSold)
        );
        assert_eq!(account, before);
        // The AK-74N, dragged onto the character: into the weapon's slot.
        account.generic_money = 150_000;
        let rifle = buy_item(&mut account, dict("ak_74n"), 1, 10, 1, NOW).unwrap();
        assert_eq!(account.kits[0].slots[10].unwrap().item_id, rifle[0].item_id);
        assert_eq!(account.generic_money, 0);
        // Into a slot it does not fit: no purchase.
        account.generic_money = 150_000;
        let before = account.clone();
        assert!(buy_item(&mut account, dict("ak_74n"), 1, 1, 1, NOW).is_err());
        assert_eq!(account, before);
    }

    /// `prices::is_not_sold` (2026-10-18): the Russian and Belarusian flags
    /// and coats of arms, the flag of the USSR and the red star are bought
    /// neither one by one nor from a faction.
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
        // The flag of Ukraine is, for its 300 gold in silver.
        assert!(buy_item(&mut account, dict("decals_4_3"), 1, INVENTORY_SLOT, 1, NOW).is_ok());
        assert_eq!(account.generic_money, 970_000);
    }
}

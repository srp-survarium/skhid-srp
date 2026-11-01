//! The admin's account moderation (`docs/notes/account-moderation.md`): an
//! account's silver and gold changed, and its items taken away, from the
//! admin page while srp runs. The rules of the change are here; the page
//! (`web_server`) shows the plan, applies it when confirmed, and writes the
//! account and an [`AdminAction`] ([`super::GameAccounts::write_admin`]).
//!
//! Every change is planned from the account as it is ([`MoneyChange::plan`],
//! [`plan_take`]) and applied under the same lock that planned it, so what
//! is applied is what the plan says. A plan's [`MoneyChange::key`] /
//! [`take_key`] names the account's state it was made from: the page keeps
//! it with the preview's one-time token, and a confirm is applied only when
//! the account still gives the same key, so a change by the game in between
//! changes nothing (and the token, used once, a reload of the confirm).

use survarium::v026g::dictionary::Dictionary;

use super::account::Account;

/// The most items one take names.
pub const MAX_TAKEN_ITEMS: usize = 32;
/// The longest reason kept, in characters.
pub const MAX_REASON: usize = 200;

/// One admin action, as the audit trail keeps it (`g26_admin_actions`) and
/// the log writes it. The reason is the admin's only (no player is ever
/// shown it), but a chat mute's, which its player is told
/// (`docs/notes/admin-chat-mute.md`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdminAction {
    /// Unix seconds.
    pub at: u64,
    /// Who: the admin's address, as the page's log names it.
    pub admin: String,
    pub account_id: u32,
    /// The account's character name then.
    pub account: String,
    /// `money`, `items` or `chat_mute`.
    pub kind: String,
    /// What was done, e.g. `silver -480000 (asked -500000)`.
    pub change: String,
    pub reason: String,
    pub before: String,
    pub after: String,
}

/// A reason as the page takes it: trimmed, control characters out, at most
/// [`MAX_REASON`] characters. `None` for an empty one.
pub fn clean_reason(text: &str) -> Option<String> {
    let reason: String = text
        .trim()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_REASON)
        .collect();
    let reason = reason.trim().to_owned();
    (!reason.is_empty()).then_some(reason)
}

/// `balance` changed by `change`, kept within 0 and `u32::MAX`.
fn moved(balance: u32, change: i64) -> u32 {
    let wanted = i64::from(balance).saturating_add(change);
    u32::try_from(wanted.max(0)).unwrap_or(u32::MAX)
}

/// A change of silver (`generic_money`) and gold (`premium_money`) as it
/// would be made now: each balance before and after. A take of more than
/// the account has leaves 0: the plan says how much is really taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoneyChange {
    pub account_id: u32,
    /// `(before, after)`.
    pub silver: (u32, u32),
    pub gold: (u32, u32),
    /// What was asked, signed.
    pub asked: (i64, i64),
}

impl MoneyChange {
    /// The change of `silver` and `gold` (signed) on `account` as it is.
    pub fn plan(account_id: u32, account: &Account, silver: i64, gold: i64) -> Self {
        let (silver_now, gold_now) = (account.generic_money, account.premium_money);
        Self {
            account_id,
            silver: (silver_now, moved(silver_now, silver)),
            gold: (gold_now, moved(gold_now, gold)),
            asked: (silver, gold),
        }
    }

    /// The silver really added (positive) or taken (negative).
    pub fn silver_change(&self) -> i64 {
        i64::from(self.silver.1) - i64::from(self.silver.0)
    }

    pub fn gold_change(&self) -> i64 {
        i64::from(self.gold.1) - i64::from(self.gold.0)
    }

    /// Whether it changes nothing (nothing asked, or a take from 0).
    pub fn is_nothing(&self) -> bool {
        self.silver.0 == self.silver.1 && self.gold.0 == self.gold.1
    }

    /// Whether the account is as the plan was made from.
    pub fn still_fits(&self, account: &Account) -> bool {
        (account.generic_money, account.premium_money) == (self.silver.0, self.gold.0)
    }

    /// Apply it to the account it was planned on, under the same lock.
    pub fn apply(&self, account: &mut Account) {
        debug_assert!(self.still_fits(account));
        account.generic_money = self.silver.1;
        account.premium_money = self.gold.1;
    }

    /// The account and the change, from the balances: equal for the same
    /// change of the same balances only.
    pub fn key(&self) -> String {
        format!(
            "money:{}:{}>{}:{}>{}",
            self.account_id, self.silver.0, self.silver.1, self.gold.0, self.gold.1
        )
    }

    /// What the audit trail and the log keep of it.
    pub fn action(&self, at: u64, admin: &str, account: &str, reason: &str) -> AdminAction {
        let line = |name: &str, change: i64, asked: i64| {
            if change == asked {
                format!("{name} {change:+}")
            } else {
                format!("{name} {change:+} (asked {asked:+})")
            }
        };
        AdminAction {
            at,
            admin: admin.to_owned(),
            account_id: self.account_id,
            account: account.to_owned(),
            kind: "money".to_owned(),
            change: format!(
                "{}, {}",
                line("silver", self.silver_change(), self.asked.0),
                line("gold", self.gold_change(), self.asked.1)
            ),
            reason: reason.to_owned(),
            before: format!("{} silver, {} gold", self.silver.0, self.gold.0),
            after: format!("{} silver, {} gold", self.silver.1, self.gold.1),
        }
    }
}

/// An item the admin chose to take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    pub item_id: u32,
    /// Units of a stack; `None`: all of it. An item that is no stack is
    /// taken whole either way.
    pub units: Option<u32>,
}

/// One item of a take as it would be made now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Taking {
    pub item_id: u32,
    pub dict_id: u16,
    pub is_stack: bool,
    /// A stack's units, or an item's condition (× 10 000), now.
    pub had: u32,
    /// Units taken of a stack; 1 for an item.
    pub taken: u32,
    /// The ids of the kits that wear it.
    pub worn_in: Vec<u32>,
}

impl Taking {
    /// Whether the item goes whole (a stack down to nothing too).
    pub fn whole(&self) -> bool {
        !self.is_stack || self.taken >= self.had
    }
}

/// Why a take was not planned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TakeRefused {
    /// No item was chosen.
    Nothing,
    /// More than [`MAX_TAKEN_ITEMS`].
    TooMany,
    /// The account has no item of this id (taken or sold meanwhile).
    NoSuchItem(u32),
    /// The same item twice.
    Twice(u32),
    /// 0 units of a stack.
    NoUnits(u32),
}

/// The take of `picks` from `account` as it is. A stack's units past what
/// it has are what it has. All or nothing: any pick that is wrong refuses
/// the whole take.
pub fn plan_take(account: &Account, picks: &[Pick]) -> Result<Vec<Taking>, TakeRefused> {
    if picks.is_empty() {
        return Err(TakeRefused::Nothing);
    }
    if picks.len() > MAX_TAKEN_ITEMS {
        return Err(TakeRefused::TooMany);
    }
    let dictionary = Dictionary::shared();
    let mut plan: Vec<Taking> = Vec::with_capacity(picks.len());
    for pick in picks {
        if plan.iter().any(|taking| taking.item_id == pick.item_id) {
            return Err(TakeRefused::Twice(pick.item_id));
        }
        let item = account
            .item(pick.item_id)
            .ok_or(TakeRefused::NoSuchItem(pick.item_id))?;
        let is_stack = dictionary.is_stack(item.dict_id);
        let had = item.condition_or_stack;
        let taken = if is_stack {
            match pick.units {
                Some(0) => return Err(TakeRefused::NoUnits(pick.item_id)),
                Some(units) => units.min(had),
                None => had,
            }
        } else {
            1
        };
        let worn_in = account
            .kits
            .iter()
            .filter(|kit| kit.slot_of(pick.item_id).is_some())
            .map(|kit| kit.id)
            .collect();
        plan.push(Taking {
            item_id: pick.item_id,
            dict_id: item.dict_id,
            is_stack,
            had,
            taken,
            worn_in,
        });
    }
    Ok(plan)
}

/// The account and the take, from what it holds of each item: equal for the
/// same take of the same items only.
pub fn take_key(account_id: u32, plan: &[Taking]) -> String {
    let items: Vec<String> = plan
        .iter()
        .map(|taking| {
            format!(
                "{}:{}:{}:{}",
                taking.item_id, taking.dict_id, taking.had, taking.taken
            )
        })
        .collect();
    format!("items:{account_id}:{}", items.join(","))
}

/// Apply a take to the account it was planned on, under the same lock: a
/// stack loses the units, anything else (a stack down to nothing too)
/// leaves the inventory, and the kits follow as when an item is sold
/// ([`Account::sync_kits`]): a slot of an item that is gone is emptied in
/// every kit, a stack's slot takes at most what is left, and an upgrade
/// whose cosmetic is gone is taken off the item it was on.
pub fn take(account: &mut Account, plan: &[Taking]) {
    for taking in plan {
        let Some(index) = account
            .inventory
            .iter()
            .position(|item| item.item_id == taking.item_id)
        else {
            continue;
        };
        if taking.whole() {
            account.inventory.remove(index);
        } else {
            let item = &mut account.inventory[index];
            item.condition_or_stack = item.condition_or_stack.saturating_sub(taking.taken);
        }
    }
    account.sync_kits();
}

/// What the audit trail and the log keep of a take.
pub fn take_action(
    plan: &[Taking],
    account_id: u32,
    at: u64,
    admin: &str,
    account: &str,
    reason: &str,
) -> AdminAction {
    let dictionary = Dictionary::shared();
    let name = |dict_id: u16| {
        dictionary
            .item(dict_id)
            .map_or_else(|| format!("dict {dict_id}"), |item| item.name.clone())
    };
    let mut change = Vec::new();
    let mut before = Vec::new();
    let mut after = Vec::new();
    for taking in plan {
        let what = format!(
            "{} (dict {}, item {})",
            name(taking.dict_id),
            taking.dict_id,
            taking.item_id
        );
        if taking.is_stack {
            change.push(format!("{} x{what}", taking.taken));
            before.push(format!("{} x{what}", taking.had));
            after.push(format!("{} x{what}", taking.had - taking.taken));
        } else {
            change.push(what.clone());
            before.push(what);
        }
        if !taking.worn_in.is_empty() {
            let kits: Vec<String> = taking.worn_in.iter().map(u32::to_string).collect();
            if let Some(last) = change.last_mut() {
                *last += &format!(" worn in kit(s) {}", kits.join(" "));
            }
        }
    }
    let list = |items: Vec<String>| {
        if items.is_empty() {
            "-".to_owned()
        } else {
            items.join("; ")
        }
    };
    AdminAction {
        at,
        admin: admin.to_owned(),
        account_id,
        account: account.to_owned(),
        kind: "items".to_owned(),
        change: list(change),
        reason: reason.to_owned(),
        before: list(before),
        after: list(after),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::lobby::FULL_CONDITION;

    fn dict(name: &str) -> u16 {
        Dictionary::shared().item_named(name).unwrap().dict_id
    }

    #[test]
    fn money_is_never_taken_below_zero() {
        let mut account = Account::new("pain");
        account.generic_money = 300;
        account.premium_money = 40;
        let change = MoneyChange::plan(9, &account, -500, 10);
        assert_eq!(change.silver, (300, 0));
        assert_eq!(change.gold, (40, 50));
        assert_eq!((change.silver_change(), change.gold_change()), (-300, 10));
        let action = change.action(1, "t", "pain", "farmed");
        assert_eq!(action.change, "silver -300 (asked -500), gold +10");
        assert_eq!(action.before, "300 silver, 40 gold");
        assert_eq!(action.after, "0 silver, 50 gold");
        let key = change.key();
        change.apply(&mut account);
        assert_eq!((account.generic_money, account.premium_money), (0, 50));
        // The same request on the changed account is another plan.
        let again = MoneyChange::plan(9, &account, -500, 10);
        assert_ne!(again.key(), key);
        assert!(!change.still_fits(&account));
        // Nothing left to take: nothing changes.
        assert!(MoneyChange::plan(9, &account, -500, 0).is_nothing());
        // And no balance passes the top.
        assert_eq!(
            MoneyChange::plan(9, &account, i64::MAX, 0).silver.1,
            u32::MAX
        );
    }

    #[test]
    fn a_reason_is_kept_trimmed_and_short() {
        assert_eq!(clean_reason("   "), None);
        assert_eq!(
            clean_reason(" farmed\nsilver ").as_deref(),
            Some("farmed silver")
        );
        assert_eq!(
            clean_reason(&"я".repeat(500)).unwrap().chars().count(),
            MAX_REASON
        );
    }

    #[test]
    fn part_of_a_stack_is_taken_and_the_kit_takes_what_is_left() {
        let mut account = Account::new("Fasojib");
        let ammo = account.inventory[1].clone();
        assert!(Dictionary::shared().is_stack(ammo.dict_id));
        let kit = account.kits[0].clone();
        let slot = kit.slot_of(ammo.item_id).unwrap();
        let worn = kit.slots[slot].unwrap().count;
        assert!(ammo.condition_or_stack >= 2 && worn >= 2);
        let units = ammo.condition_or_stack - 1;
        let plan = plan_take(
            &account,
            &[Pick {
                item_id: ammo.item_id,
                units: Some(units),
            }],
        )
        .unwrap();
        assert_eq!(plan[0].taken, units);
        assert!(!plan[0].whole());
        assert_eq!(plan[0].worn_in, [kit.id]);
        take(&mut account, &plan);
        assert_eq!(account.item(ammo.item_id).unwrap().condition_or_stack, 1);
        assert_eq!(account.kits[0].slots[slot].unwrap().count, 1);
        assert_eq!(account.kits[0].revision, kit.revision + 1);
        // Units past what is left are what is left: the stack goes.
        let plan = plan_take(
            &account,
            &[Pick {
                item_id: ammo.item_id,
                units: Some(1_000_000),
            }],
        )
        .unwrap();
        assert!(plan[0].whole());
        take(&mut account, &plan);
        assert!(account.item(ammo.item_id).is_none());
        assert_eq!(account.kits[0].slots[slot], None);
    }

    #[test]
    fn an_item_worn_in_every_kit_leaves_every_kit() {
        let mut account = Account::new("Mandalorienc");
        let pistol = account.inventory[0].clone();
        let slot = account.kits[0].slot_of(pistol.item_id).unwrap();
        // Worn by all three kits.
        let worn = account.kits[0].slots[slot];
        for kit in &mut account.kits {
            kit.slots[slot] = worn;
        }
        let other = account.add_item(dict("medkit"), 3);
        let before = account.clone();
        let plan = plan_take(
            &account,
            &[Pick {
                item_id: pistol.item_id,
                units: Some(5),
            }],
        )
        .unwrap();
        assert_eq!(plan[0].taken, 1);
        assert_eq!(plan[0].worn_in.len(), 3);
        take(&mut account, &plan);
        assert!(account.item(pistol.item_id).is_none());
        for (kit, was) in account.kits.iter().zip(&before.kits) {
            assert_eq!(kit.slot_of(pistol.item_id), None);
            assert_eq!(kit.revision, was.revision + 1);
        }
        // Nothing else of it changed, and the account is consistent.
        assert!(account.item(other).is_some());
        assert_eq!(account.inventory.len(), before.inventory.len() - 1);
        let mut repaired = account.clone();
        assert!(!repaired.repair(), "the kits are valid after the take");
        assert_eq!(repaired, account);
    }

    #[test]
    fn a_wrong_pick_refuses_the_whole_take() {
        let account = Account::new("x");
        let first = account.inventory[0].item_id;
        let ammo = account.inventory[1].item_id;
        let pick = |item_id, units| Pick { item_id, units };
        assert_eq!(plan_take(&account, &[]), Err(TakeRefused::Nothing));
        assert_eq!(
            plan_take(&account, &[pick(first, None), pick(999_999, None)]),
            Err(TakeRefused::NoSuchItem(999_999))
        );
        assert_eq!(
            plan_take(&account, &[pick(first, None), pick(first, None)]),
            Err(TakeRefused::Twice(first))
        );
        assert_eq!(
            plan_take(&account, &[pick(ammo, Some(0))]),
            Err(TakeRefused::NoUnits(ammo))
        );
        let many: Vec<Pick> = (1..=MAX_TAKEN_ITEMS as u32 + 1)
            .map(|id| pick(id, None))
            .collect();
        assert_eq!(plan_take(&account, &many), Err(TakeRefused::TooMany));
    }

    #[test]
    fn a_take_key_names_what_the_items_held() {
        let mut account = Account::new("x");
        let whole = account.inventory[0].item_id;
        assert!(!Dictionary::shared().is_stack(account.inventory[0].dict_id));
        let all = [Pick {
            item_id: whole,
            units: None,
        }];
        let plan = plan_take(&account, &all).unwrap();
        let key = take_key(4, &plan);
        assert_eq!(key, take_key(4, &plan_take(&account, &all).unwrap()));
        assert_ne!(key, take_key(5, &plan));
        // Worn down a little: another key.
        account.inventory[0].condition_or_stack = FULL_CONDITION - 1;
        assert_ne!(key, take_key(4, &plan_take(&account, &all).unwrap()));
        take(&mut account, &plan);
        assert_eq!(
            plan_take(&account, &all),
            Err(TakeRefused::NoSuchItem(whole))
        );
        let action = take_action(&plan, 4, 1, "t", "x", "dupe");
        assert_eq!(action.kind, "items");
        assert!(
            action.change.contains(&format!("item {whole}")),
            "{}",
            action.change
        );
    }
}

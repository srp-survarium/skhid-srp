//! A nickname change (`49 03` check, `49 04` change;
//! `docs/notes/lobby_requests.md` §2.1, §4.1).
//!
//! The lobby's rename window (`MessageEnterName`) checks a name as it is
//! typed (`49 03`, answered `83 49 03 <name> <ok> [reason]`) and sends
//! `49 04 <name> <u32>`: the u32 is the item id of the rename voucher the
//! player pays with, or 0 to pay with money (`change_player_nickname(name,
//! radioButtonPremium.selected ? 0 : voucherId)`; PROVEN). The price is
//! silver (gold is gone, the owner's of 2026-11-08), 0x8D's fourth value
//! (`root.set_change_name_cost`). The answer is
//! `83 49 04 <voucher item id or 0>`: the client takes one voucher off its
//! copy. The new name itself reaches the client in `8F`.
//!
//! What a name is in srp: an account signs in with its **login** (the
//! user name it registered, `crate::Account::login`) and is shown by its
//! **name** (`crate::Account::name`, copied to the 0.26g account), which
//! starts as the login's part before an `@`. A rename changes the name
//! only: the sign-in is not touched. [choice] A new name must be shown by
//! no other account and be no other account's login (so nobody can pose as
//! the owner of a login); the check for that is the lobby's, which has the
//! store. The price is `SRP_RENAME_COST` silver, by default the rename
//! voucher's own price in the dictionary (25,000 live: its 250 gold in
//! silver, `items.tsv`).

use survarium::v026g::dictionary::Dictionary;

use super::account::{Account, cap_name};
use super::premium::{RENAME_VOUCHER, Settings};

/// Why a rename is refused once the name itself is good.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameError {
    /// The item named is no rename voucher of the account.
    NoVoucher,
    NotEnoughMoney,
}

impl RenameError {
    /// The game's own text for the refusal.
    pub fn text_key(&self) -> &'static str {
        match self {
            Self::NoVoucher => "st_change_name_error",
            Self::NotEnoughMoney => "st_not_enougth_money",
        }
    }
}

/// How a rename was paid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paid {
    /// The voucher one was taken of; 0 when silver paid.
    pub voucher_item_id: u32,
    pub silver: u32,
}

/// `49 04`: pay for the rename and take the name (cut to what the client's
/// buffers hold). `voucher_item_id` 0 pays in silver. The caller has checked
/// the name.
pub fn rename(
    account: &mut Account,
    settings: &Settings,
    name: &str,
    voucher_item_id: u32,
) -> Result<Paid, RenameError> {
    let paid = if voucher_item_id == 0 {
        if account.generic_money < settings.rename_cost {
            return Err(RenameError::NotEnoughMoney);
        }
        account.generic_money -= settings.rename_cost;
        Paid {
            voucher_item_id: 0,
            silver: settings.rename_cost,
        }
    } else {
        let voucher = Dictionary::shared().item_named(RENAME_VOUCHER);
        let index = account
            .inventory
            .iter()
            .position(|item| {
                item.item_id == voucher_item_id
                    && voucher.is_some_and(|voucher| voucher.dict_id == item.dict_id)
                    && item.condition_or_stack > 0
            })
            .ok_or(RenameError::NoVoucher)?;
        let item = &mut account.inventory[index];
        item.condition_or_stack -= 1;
        if item.condition_or_stack == 0 {
            account.inventory.remove(index);
            account.sync_kits();
        }
        Paid {
            voucher_item_id,
            silver: 0,
        }
    };
    account.name = cap_name(name);
    Ok(paid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rename_is_paid_in_silver_or_with_a_voucher() {
        let settings = Settings::default();
        let mut account = Account::new("Alice");
        account.generic_money = 30_000;
        assert_eq!(
            rename(&mut account, &settings, "Alisa", 0),
            Ok(Paid {
                voucher_item_id: 0,
                silver: 25_000
            })
        );
        assert_eq!(
            (account.name.as_str(), account.generic_money),
            ("Alisa", 5_000)
        );
        // Not the silver for a second one.
        assert_eq!(
            rename(&mut account, &settings, "Alice", 0),
            Err(RenameError::NotEnoughMoney)
        );
        assert_eq!(account.name, "Alisa");
        // A voucher pays instead, one of the stack.
        let dictionary = Dictionary::shared();
        let voucher = account.add_item(dictionary.item_named(RENAME_VOUCHER).unwrap().dict_id, 2);
        let paid = rename(&mut account, &settings, "Alice", voucher).unwrap();
        assert_eq!((paid.voucher_item_id, paid.silver), (voucher, 0));
        assert_eq!(account.item(voucher).unwrap().condition_or_stack, 1);
        assert_eq!(
            (account.name.as_str(), account.generic_money),
            ("Alice", 5_000)
        );
        rename(&mut account, &settings, "Alina", voucher).unwrap();
        assert!(account.item(voucher).is_none());
        // Gone, or not a rename voucher: refused, the name stays.
        let pistol = account.kits[0].slots[7].unwrap().item_id;
        for item in [voucher, pistol, 9_999] {
            assert_eq!(
                rename(&mut account, &settings, "Other", item),
                Err(RenameError::NoVoucher)
            );
        }
        assert_eq!(account.name, "Alina");
        // The name is cut to the client's buffer.
        account.generic_money = 25_000;
        rename(&mut account, &settings, &"x".repeat(100), 0).unwrap();
        assert_eq!(account.name.len(), 63);
        // A price of 0 is free.
        let free = Settings {
            rename_cost: 0,
            ..settings
        };
        assert_eq!(rename(&mut account, &free, "Alice", 0).unwrap().silver, 0);
    }

    #[test]
    fn the_refusals_have_the_games_texts() {
        assert_eq!(RenameError::NoVoucher.text_key(), "st_change_name_error");
        assert_eq!(
            RenameError::NotEnoughMoney.text_key(),
            "st_not_enougth_money"
        );
    }
}

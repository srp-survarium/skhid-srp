//! A nickname change (`49 03` check, `49 04` change;
//! `docs/notes/lobby_requests.md` §2.1, §4.1).
//!
//! The lobby's rename window (`MessageEnterName`) checks a name as it is
//! typed (`49 03`, answered `83 49 03 <name> <ok> [reason]`) and sends
//! `49 04 <name> <u32>`. The u32 was the item id of the rename voucher the
//! player paid with, or 0 for money (`change_player_nickname(name,
//! radioButtonPremium.selected ? 0 : voucherId)`; PROVEN): the vouchers are
//! gone (the owner, 2026-11-08: "for renames remove rename ticket too"), so
//! a rename is paid in silver, 0x8D's fourth value
//! (`root.set_change_name_cost`), and the u32 is not kept. The answer is
//! `83 49 04 0` (no voucher taken). The new name itself reaches the client
//! in `8F`.
//!
//! What a name is in srp: an account signs in with its **login** (the
//! user name it registered, `crate::Account::login`) and is shown by its
//! **name** (`crate::Account::name`, copied to the 0.26g account), which
//! starts as the login's part before an `@`. A rename changes the name
//! only: the sign-in is not touched. [choice] A new name must be shown by
//! no other account and be no other account's login (so nobody can pose as
//! the owner of a login); the check for that is the lobby's, which has the
//! store. The price is `SRP_RENAME_COST` silver, by default 25,000 (what
//! the rename voucher cost: its 250 gold in silver).

use super::account::{Account, cap_name};
use super::premium::Settings;

/// Why a rename is refused once the name itself is good.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameError {
    NotEnoughMoney,
}

impl RenameError {
    /// The game's own text for the refusal.
    pub fn text_key(&self) -> &'static str {
        match self {
            Self::NotEnoughMoney => "st_not_enougth_money",
        }
    }
}

/// `49 04`: pay for the rename in silver and take the name (cut to what the
/// client's buffers hold). Returns the silver paid. The caller has checked
/// the name.
pub fn rename(account: &mut Account, settings: &Settings, name: &str) -> Result<u32, RenameError> {
    if account.generic_money < settings.rename_cost {
        return Err(RenameError::NotEnoughMoney);
    }
    account.generic_money -= settings.rename_cost;
    account.name = cap_name(name);
    Ok(settings.rename_cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rename_is_paid_in_silver() {
        let settings = Settings::default();
        let mut account = Account::new("Alice");
        account.generic_money = 30_000;
        assert_eq!(rename(&mut account, &settings, "Alisa"), Ok(25_000));
        assert_eq!(
            (account.name.as_str(), account.generic_money),
            ("Alisa", 5_000)
        );
        // Not the silver for a second one: refused, the name stays.
        assert_eq!(
            rename(&mut account, &settings, "Alice"),
            Err(RenameError::NotEnoughMoney)
        );
        assert_eq!(account.name, "Alisa");
        // The name is cut to the client's buffer.
        account.generic_money = 25_000;
        rename(&mut account, &settings, &"x".repeat(100)).unwrap();
        assert_eq!(account.name.len(), 63);
        // A price of 0 is free.
        let free = Settings {
            rename_cost: 0,
            ..settings
        };
        assert_eq!(rename(&mut account, &free, "Alice"), Ok(0));
    }

    #[test]
    fn the_refusal_has_the_games_text() {
        assert_eq!(
            RenameError::NotEnoughMoney.text_key(),
            "st_not_enougth_money"
        );
    }
}

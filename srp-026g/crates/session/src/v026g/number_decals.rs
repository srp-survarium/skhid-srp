//! The number decals: "Number 1" to "Number 1000", each the account of that id's (the owner,
//! 2026-11-26: "I will give a number based on the id for every single person (in first 1k)";
//! `docs/notes/number-decals.md`). The accounts there were at the release that added them got
//! theirs once, from `scripts/number-decals/grant.py`. An account of such an id that had no 0.26g
//! account yet gets its number here, when that account is made ([`give_new`]).
//!
//! The decal is the game data's item `decals_num_<NNNN>` (the pack `number-decals`); data without
//! it gives none.

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lobby::FULL_CONDITION;

use super::account::Account;

/// The accounts among the numbers' ids that get none (2026-11-26): 4, a test account, and 590,
/// System (the name the server's notices take). `grant.py` leaves out the same two.
pub const LEFT_OUT: [u32; 2] = [4, 590];

/// A number of the decals, 1 to [`Number::LAST`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Number(u16);

impl Number {
    pub const LAST: u16 = 1000;

    /// The number of the account `account_id`: its id, when that is 1 to 1000 and not one of
    /// [`LEFT_OUT`].
    pub fn of_account(account_id: u32) -> Option<Self> {
        if LEFT_OUT.contains(&account_id) {
            return None;
        }
        u16::try_from(account_id)
            .ok()
            .filter(|id| (1..=Self::LAST).contains(id))
            .map(Self)
    }

    pub fn get(self) -> u16 {
        self.0
    }

    /// The decal's dictionary name, `decals_num_0042` (`scripts/number-decals/names.py`).
    pub fn decal_name(self) -> String {
        format!("decals_num_{:04}", self.0)
    }
}

/// Gives the 0.26g account `account_id`, made now, its number decal: when its id has a number
/// and `dictionary` has that decal, and it does not own it yet. The decal's dictionary id, if
/// one was given.
pub fn give_new(account: &mut Account, account_id: u32, dictionary: &Dictionary) -> Option<u16> {
    let number = Number::of_account(account_id)?;
    let dict_id = dictionary.item_named(&number.decal_name())?.dict_id;
    if account.inventory.iter().any(|item| item.dict_id == dict_id) {
        return None;
    }
    account.add_item(dict_id, FULL_CONDITION);
    Some(dict_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::dictionary::Item;

    fn with_decals(numbers: &[u16]) -> Dictionary {
        let mut dictionary = Dictionary::shared().clone();
        let last = dictionary.items.last().map_or(0, |item| item.dict_id);
        assert!(last < 5001, "the test data ends before the decals' ids");
        dictionary.items.extend(numbers.iter().map(|&n| Item {
            dict_id: 5000 + n,
            name: format!("decals_num_{n:04}"),
            category: 26,
            is_stack: false,
            is_premium: true,
            base_cost: 0,
            cfg_name: format!("gameplay/items/upgrades/decals/char_decal_num_{n:04}.options"),
            like: None,
            start: Vec::new(),
        }));
        dictionary
    }

    /// The number is the id, 1 to 1000, but for the test account and System.
    #[test]
    fn the_number_is_the_account_id() {
        let numbered = [
            (1, 1),
            (3, 3),
            (5, 5),
            (42, 42),
            (589, 589),
            (591, 591),
            (1000, 1000),
        ];
        for (id, number) in numbered {
            assert_eq!(Number::of_account(id).map(Number::get), Some(number));
        }
        for id in [0, 4, 590, 1001, 70_000] {
            assert_eq!(Number::of_account(id), None);
        }
        let name = |id| Number::of_account(id).unwrap().decal_name();
        assert_eq!(
            (name(42), name(1000)),
            ("decals_num_0042".into(), "decals_num_1000".into())
        );
    }

    /// A new account of a numbered id owns its decal, once; another id, or data without the
    /// decal, gives nothing.
    #[test]
    fn a_new_account_gets_its_number_once() {
        let dictionary = with_decals(&[42, 1000]);
        let mut account = Account::new("Carol");
        let before = account.inventory.len();
        assert_eq!(give_new(&mut account, 42, &dictionary), Some(5042));
        let given: Vec<_> = account.inventory[before..]
            .iter()
            .map(|item| (item.dict_id, item.condition_or_stack))
            .collect();
        assert_eq!(given, [(5042, FULL_CONDITION)]);
        assert_eq!(give_new(&mut account, 42, &dictionary), None);
        assert_eq!(account.inventory.len(), before + 1);

        let mut other = Account::new("Dave");
        let owned = other.inventory.len();
        assert_eq!(give_new(&mut other, 1001, &dictionary), None);
        assert_eq!(give_new(&mut other, 590, &dictionary), None);
        assert_eq!(give_new(&mut other, 43, &dictionary), None);
        assert_eq!(give_new(&mut other, 43, Dictionary::shared()), None);
        assert_eq!(other.inventory.len(), owned);
    }
}

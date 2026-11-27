//! The reticle an account chose for each of its scope items (`docs/notes/reticle-choice.md`):
//! free, chosen in the lobby (`46 05` with the scope item as both host and upgrade,
//! [`super::items_rules::place_upgrade_with`]), kept per scope item (`g26_scope_reticles`,
//! `persist::reticles`) and sent before every inventory (`A9`, [`message`]). Every gun with that
//! scope shows it, only in its holder's first-person aimed view: nothing here changes what a
//! match computes or what a game writes into a world.
//!
//! A held reticle is always one of its item's scope lists ([`Reticles`]): it is made against them
//! (a request, a load), a choice the current lists no longer hold reads as the default at load,
//! and a scope item that goes (sold, used up) takes its choice with it ([`ScopeReticles::keep_owned`],
//! from `Account::sync_kits`).

use std::collections::BTreeMap;

use survarium::v026g::lobby::{self, ScopeReticle, ServerMessage, WireError};
use survarium::v026g::reticles::{Numbers, Reticle, Reticles};

use super::account::{Account, Item};

/// The reticles an account chose, by scope item id: only those that are not
/// [`Reticle::DEFAULT`] (an item without one shows the default).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScopeReticles {
    chosen: BTreeMap<u32, Reticle>,
}

impl ScopeReticles {
    /// The reticle of the scope item `item_id`.
    pub fn of(&self, item_id: u32) -> Reticle {
        self.chosen.get(&item_id).copied().unwrap_or(Reticle::DEFAULT)
    }

    /// `item_id` shows `reticle` from now on (made against its scope's lists). Whether it
    /// changed.
    pub fn choose(&mut self, item_id: u32, reticle: Reticle) -> bool {
        let before = self.of(item_id);
        if reticle == Reticle::DEFAULT {
            self.chosen.remove(&item_id);
        } else {
            self.chosen.insert(item_id, reticle);
        }
        before != reticle
    }

    /// The choices that are not the default, by item id.
    pub fn chosen(&self) -> impl Iterator<Item = (u32, Reticle)> + '_ {
        self.chosen.iter().map(|(&item_id, &reticle)| (item_id, reticle))
    }

    /// Drops the choice of every item `inventory` no longer holds.
    pub fn keep_owned(&mut self, inventory: &[Item]) {
        self.chosen
            .retain(|item_id, _| inventory.iter().any(|item| item.item_id == *item_id));
    }

    /// The choices kept in the database (`(item id, numbers)`), each held against its item's
    /// scope lists now: an item the inventory lacks, one with no lists, or numbers its lists do
    /// not hold read as the default (the second, a release that shortened a list; logged).
    pub fn loaded(
        account_id: u32,
        kept: impl IntoIterator<Item = (u32, Numbers)>,
        inventory: &[Item],
        reticles: &Reticles,
    ) -> ScopeReticles {
        let mut loaded = ScopeReticles::default();
        for (item_id, numbers) in kept {
            let lists = inventory
                .iter()
                .find(|item| item.item_id == item_id)
                .and_then(|item| reticles.of(item.dict_id));
            match lists.map(|lists| lists.reticle(numbers)) {
                Some(Some(reticle)) => {
                    loaded.choose(item_id, reticle);
                }
                Some(None) => log::warn!(
                    "account {account_id}: scope item {item_id}'s reticle {numbers:?} is not one \
                     of its lists: the default"
                ),
                None => log::warn!(
                    "account {account_id}: a reticle for item {item_id}, which is no scope item \
                     with reticle lists of the account: dropped"
                ),
            }
        }
        loaded
    }
}

/// The scope items of `account` with lists in `reticles`, and their reticles, by item id: the
/// first [`lobby::reticles::MAX_ITEMS`].
pub fn owned(account: &Account, reticles: &Reticles) -> Vec<ScopeReticle> {
    let mut owned: Vec<ScopeReticle> = account
        .inventory
        .iter()
        .filter(|item| reticles.of(item.dict_id).is_some())
        .map(|item| ScopeReticle {
            item_id: item.item_id,
            dict_id: item.dict_id,
            reticle: account.reticles.of(item.item_id),
        })
        .collect();
    owned.sort_by_key(|item| item.item_id);
    owned.truncate(lobby::reticles::MAX_ITEMS);
    owned
}

/// `A9`: the account's scope items with lists, their reticles and the lists of their scopes
/// (`lobby::reticles`), with `reticles` the game data's.
pub fn message_with(account: &Account, reticles: &Reticles) -> Result<ServerMessage, WireError> {
    let items = owned(account, reticles);
    let mut dicts: Vec<u16> = items.iter().map(|item| item.dict_id).collect();
    dicts.sort_unstable();
    dicts.dedup();
    let lists: Vec<_> = dicts
        .into_iter()
        .filter_map(|dict| reticles.of(dict).map(|lists| (dict, lists)))
        .collect();
    lobby::scope_reticles(&lists, &items)
}

/// [`message_with`] the players' game data's lists.
pub fn message(account: &Account) -> Result<ServerMessage, WireError> {
    message_with(account, Reticles::shared())
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::lobby::FULL_CONDITION;
    use survarium::v026g::lobby::client_model::{Client, Decoded};
    use survarium::v026g::reticles::fixtures;

    /// 0.26g's Leupold and PSO-1: scope items of the test data's dictionary.
    const LEUPOLD: u16 = 69;
    const PSO: u16 = 304;

    fn numbers(pattern: u8, size: u8, colour: u8, brightness: u8) -> Numbers {
        Numbers {
            pattern,
            size,
            colour,
            brightness,
        }
    }

    #[test]
    fn a_default_choice_keeps_nothing_and_a_gone_item_loses_its_choice() {
        let reticles = fixtures::reticles(&[LEUPOLD]);
        let lists = reticles.of(LEUPOLD).unwrap();
        let mut account = Account::new("Scoped");
        let scope = account.add_item(LEUPOLD, FULL_CONDITION);
        let cross = lists.reticle(numbers(1, 2, 1, 3)).unwrap();
        assert!(account.reticles.choose(scope, cross));
        assert!(!account.reticles.choose(scope, cross));
        assert_eq!(account.reticles.of(scope), cross);
        assert!(account.reticles.choose(scope, Reticle::DEFAULT));
        assert_eq!(account.reticles.chosen().count(), 0);
        account.reticles.choose(scope, cross);
        account.inventory.retain(|item| item.item_id != scope);
        account.reticles.keep_owned(&account.inventory);
        assert_eq!(account.reticles.of(scope), Reticle::DEFAULT);
        assert_eq!(account.reticles.chosen().count(), 0);
    }

    #[test]
    fn a_load_keeps_only_what_the_lists_hold() {
        let reticles = fixtures::reticles(&[LEUPOLD]);
        let mut account = Account::new("Loaded");
        let scope = account.add_item(LEUPOLD, FULL_CONDITION);
        let other = account.add_item(LEUPOLD, FULL_CONDITION);
        let no_lists = account.add_item(PSO, FULL_CONDITION);
        let loaded = ScopeReticles::loaded(
            1,
            [
                (scope, numbers(1, 3, 1, 10)),
                // a pattern a later release took away: the default
                (other, numbers(2, 0, 0, 6)),
                (no_lists, numbers(0, 0, 0, 1)),
                (999, numbers(0, 0, 0, 1)),
            ],
            &account.inventory,
            &reticles,
        );
        assert_eq!(loaded.of(scope).numbers(), numbers(1, 3, 1, 10));
        assert_eq!(loaded.of(other), Reticle::DEFAULT);
        assert_eq!(loaded.chosen().count(), 1);
    }

    #[test]
    fn the_message_lists_each_owned_scope_once() {
        let reticles = fixtures::reticles(&[LEUPOLD]);
        let mut account = Account::new("Sent");
        let first = account.add_item(LEUPOLD, FULL_CONDITION);
        let second = account.add_item(LEUPOLD, FULL_CONDITION);
        account.add_item(PSO, FULL_CONDITION);
        let cross = reticles.of(LEUPOLD).unwrap().reticle(numbers(1, 0, 1, 2)).unwrap();
        account.reticles.choose(second, cross);
        let message = message_with(&account, &reticles).unwrap();
        let mut client = Client::new();
        client
            .receive(&lobby::signed_in(0, false))
            .unwrap();
        let Decoded::ScopeReticles { lists, items } = client.receive(&message).unwrap() else {
            panic!("not A9");
        };
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].dict_id, LEUPOLD);
        assert_eq!(
            items,
            vec![
                (first, LEUPOLD, Reticle::DEFAULT.numbers()),
                (second, LEUPOLD, cross.numbers()),
            ]
        );
        // The test data's own lists: none, an empty message.
        assert_eq!(message_with(&account, Reticles::shared()).unwrap().body(), &[0, 0, 0]);
    }
}

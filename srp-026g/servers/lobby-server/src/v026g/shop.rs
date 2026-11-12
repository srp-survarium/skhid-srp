//! The premium shop, the daily bonus and a nickname change: the requests
//! `dispatch` hands over, and what a sign-in and a match's results add for
//! them.
//!
//! The rules are `session::v026g::{premium, bonus, tutorial, names}`; this
//! is who is sent what.
//!
//! | the client | the answer |
//! |---|---|
//! | `47 00` for a premium item | `83 47 00`, then the inventory, the account and the money (silver taken) |
//! | `47 02 <offer> <x>` (a bundle) | `83 47 02`, the inventory, the account, the money |
//! | `49 03 <name>` | `83 49 03 <name> <ok> [reason]` |
//! | `49 04 <name> <ignored>` | `83 49 04 0`, the name (`8F`), the inventory, the account, the money (silver; no voucher) |
//! | `43 00` | `99` |
//!
//! No tutorial (2026-10-19): a step (`4B`) is not answered. No premium
//! account (2026-11-09): no premium time is sold, and no premium offer
//! (`A8`, `A9`) is sent, so `47 05` and `47 06` are not read.
//!
//! A refusal is `84 <op> 00 <the game's text>`.

use session::LockExt;
use session::v026g::{bonus, names, premium, quests};
use survarium::v026g::lobby::requests::{INVENTORY_SLOT, id};
use survarium::v026g::lobby::{self, NameRefusal};

use super::core::{Core, Who};

impl Core<'_> {
    // ---------------------------------------------------------- a sign-in

    /// What a sign-in adds once its pushes are queued: the daily bonus
    /// (`99`; its welcome window on the first sign-in of a day).
    pub(super) fn sign_in_extras(&mut self, who: &Who) {
        let unix = quests::now();
        let daily = {
            let mut account = who.account.locked();
            let due = bonus::window_due(&account, unix);
            let message = bonus::message(&account, due);
            if due && message.is_some() {
                bonus::window_shown(&mut account, unix);
            }
            message
        };
        if let Some(message) = daily {
            self.accounts.mark_dirty(who.account_id);
            self.send_made(who.addr, message);
        }
    }

    /// `43 00`: the daily bonus as it stands, with its window.
    pub(super) fn daily_bonus(&mut self, who: &Who) {
        let message = bonus::message(&who.account.locked(), true);
        match message {
            Some(message) => self.send_made(who.addr, message),
            None => log::info!("{} asked for a daily bonus it has none of yet", who.name),
        }
    }

    // ------------------------------------------------------------ results

    /// What goes with a match's results screen: the daily bonus as the
    /// match left it.
    pub(super) fn after_results(&mut self, who: &Who) {
        let daily = bonus::message(&who.account.locked(), false);
        if let Some(message) = daily {
            self.send_made(who.addr, message);
        }
    }

    // ----------------------------------------------------------- the shop

    /// `47 00` for a premium item, which the shop sells one by one.
    pub(super) fn buy_premium_item(
        &mut self,
        who: &Who,
        dict_id: u16,
        count: u32,
        target_slot: u8,
        kit_id: u32,
    ) {
        let bought = premium::buy_item(
            &mut who.account.locked(),
            dict_id,
            count,
            target_slot,
            kit_id,
            quests::now(),
        );
        match bought {
            Ok(bought) => {
                log::info!("{} bought {count} of premium item {dict_id}", who.name);
                self.send_made(who.addr, lobby::permitted_bought(&bought));
                self.push_account(who);
            }
            Err(error) => self.refuse_purchase(who, &error, error.text_key(), target_slot),
        }
    }

    /// A `47 00` refused: the game's text, and for a buy dropped on a slot
    /// the account again, as the game locked the paper doll for it and only
    /// the account unlocks it (`on_profile_changed` 0x222330).
    pub(super) fn refuse_purchase(
        &mut self,
        who: &Who,
        why: &dyn std::fmt::Display,
        text_key: &str,
        target_slot: u8,
    ) {
        log::info!("{}: purchase refused: {why}", who.name);
        self.deny(who.addr, id::SHOP, text_key);
        if target_slot != INVENTORY_SLOT {
            self.push_account(who);
        }
    }

    /// `47 02 <offer> <x>`: a bundle of the shop (`x`, the results
    /// screen's match when the offer was premium time, means nothing now).
    pub(super) fn buy_offer(&mut self, who: &Who, offer_id: u32) {
        let bought = premium::buy_offer(&mut who.account.locked(), offer_id);
        match bought {
            Ok(purchase) => {
                log::info!(
                    "{} bought offer {offer_id} for {} silver: {:?}",
                    who.name,
                    purchase.cost,
                    purchase.items
                );
                self.send(who.addr, lobby::permitted_offer_bought());
                self.push_account(who);
                // The squad mates see the kits.
                self.squads_dirty = true;
            }
            Err(error) => {
                log::info!("{}: offer {offer_id} refused: {error}", who.name);
                self.deny(who.addr, id::SHOP, error.text_key());
            }
        }
    }

    // ---------------------------------------------------------- nicknames

    /// What is wrong with `nickname` for the account, in the client's own
    /// reasons: the name's own rules, then whether another account is
    /// shown by it or signs in with it.
    pub(super) fn nickname_refusal(&self, nickname: &str, account_id: u32) -> Option<NameRefusal> {
        if let Err(refusal) = names::Nickname::parse(nickname) {
            return Some(refusal);
        }
        self.store
            .nickname_taken(nickname, account_id)
            .then_some(NameRefusal::Taken)
    }

    /// `49 04 <name> <u32>`: a new nickname, paid in silver.
    pub(super) fn change_nickname(&mut self, who: &Who, nickname: &str) {
        let nickname = nickname.trim();
        let nickname = match names::Nickname::parse(nickname) {
            Ok(nickname) => nickname,
            Err(refusal) => {
                log::info!("{}: nickname {nickname:?} refused: {refusal:?}", who.name);
                self.deny(who.addr, id::PROFILE, "st_change_name_error");
                return;
            }
        };
        let renamed = self
            .store
            .rename_026g(self.accounts, &self.shop, who.account_id, &nickname);
        let silver = match renamed {
            Ok(silver) => silver,
            Err(error) => {
                log::info!("{}: the rename is refused: {error:?}", who.name);
                self.deny(who.addr, id::PROFILE, error.text_key());
                return;
            }
        };
        let name = who.account.locked().name.clone();
        if let Some(player) = self.player_mut(who.addr) {
            player.name = name.clone();
        }
        log::info!(
            "{} (account {}) is now named {name} ({silver} silver)",
            who.name,
            who.account_id,
        );
        self.send(who.addr, lobby::permitted_nickname_changed());
        let ranked = self.accounts.ranking().len() as u32;
        self.send(who.addr, lobby::base_info(ranked, &name));
        self.push_account(who);
        // Those who show the name: the squad, and the friends in the lobby.
        self.squads_dirty = true;
        for addr in self.players() {
            let Some(other) = self.who(addr) else {
                continue;
            };
            let is_friend = self
                .store
                .account(other.account_id)
                .is_some_and(|account| account.locked().friends.contains(&who.account_id));
            if is_friend {
                self.push_friends(&other);
            }
        }
    }
}

//! The premium shop, the premium account, the daily bonus, the tutorial
//! and a nickname change: the requests `dispatch` hands over, and what a
//! sign-in and a match's results add for them.
//!
//! The rules are `session::v026g::{premium, bonus, tutorial, names}`; this
//! is who is sent what.
//!
//! | the client | the answer |
//! |---|---|
//! | `47 00` for a premium item | `83 47 00`, then the inventory, the account and the money (gold taken) |
//! | `47 02 <offer> 0` | `83 47 02`, the services (`94`), the inventory, the account, the money |
//! | `47 02 <offer> <match>` (the results screen's button) | the same, and the premium half of the match: skills (`8C`), reputations (`8E`), notices |
//! | `47 05 <offer> <discount> ..` (after `A8`) | `83 47 05` (the client's "premium activated" notice), then as above |
//! | `47 06 <offer> <discount>` (after `A9`; offer -1 declines) | `83 47 02`, then as `47 02` |
//! | `46 04 <item>` | `83 46 04 <item>`, the services, the inventory, the account, the money |
//! | `49 03 <name>` | `83 49 03 <name> <ok> [reason]` |
//! | `49 04 <name> <voucher or 0>` | `83 49 04 <voucher>`, the name (`8F`), the inventory, the account, the money |
//! | `4B 01 <step>` | what the step paid: the inventory, the account, the money, the skills, notices |
//! | `43 00` | `99` |
//!
//! A refusal is `84 <op> 00 <the game's text>`.
//!
//! When a discount is set (`SRP_PREMIUM_DISCOUNT`), a player without
//! premium is sent `A9` after its sign-in and `A8` with a match's results;
//! see `session::v026g::premium`.

use session::LockExt;
use session::v026g::rewards::MatchReport;
use session::v026g::{bonus, names, premium, quests, wire};
use survarium::v026g::lobby::requests::{INVENTORY_SLOT, id};
use survarium::v026g::lobby::{self, NameRefusal};

use super::core::{Core, Who};
use super::dispatch::name_refusal;

/// The offer id `47 06` carries when the player closed the discount's
/// window (`accept_premium_with_discount(-1, -1)`).
const DECLINED: u32 = u32::MAX;
/// How far a discount the client names may be from the one offered: it
/// comes back as the f32 the server sent, or as one the lobby computed
/// from it.
const DISCOUNT_SLACK: f32 = 0.001;

impl Core<'_> {
    // ---------------------------------------------------------- a sign-in

    /// What a sign-in adds once its pushes are queued: the daily bonus
    /// (`99`; its welcome window on the first sign-in of a day) and, with
    /// a discount set, the discount's window for an account without
    /// premium (`A9`).
    pub(super) fn sign_in_extras(&mut self, who: &Who) {
        let unix = quests::now();
        let (daily, premium) = {
            let mut account = who.account.locked();
            let due = bonus::window_due(&account, unix);
            let message = bonus::message(&account, due);
            if due && message.is_some() {
                bonus::window_shown(&mut account, unix);
            }
            (message, account.is_premium(unix))
        };
        if let Some(message) = daily {
            self.accounts.mark_dirty(who.account_id);
            self.send_made(who.addr, message);
        }
        if self.shop.discount_percent > 0 && !premium {
            self.send(who.addr, lobby::premium_discount(self.shop.discount()));
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
    /// match left it, and, with a discount set, the offer of the premium
    /// half the match would have paid (`A8`).
    pub(super) fn after_results(&mut self, who: &Who, report: &MatchReport) {
        let unix = quests::now();
        let (daily, premium) = {
            let account = who.account.locked();
            (bonus::message(&account, false), account.is_premium(unix))
        };
        if let Some(message) = daily {
            self.send_made(who.addr, message);
        }
        let Some(player) = report.player(who.account_id) else {
            return;
        };
        let (money, experience, reputation) = player.reward.premium_bonus;
        let unpaid = !player.reward.premium && (money, experience, reputation) != (0, 0, 0);
        if self.shop.discount_percent == 0 || premium || !unpaid {
            return;
        }
        let seconds = self.shop.offer_seconds;
        self.premium_offers.insert(
            who.account_id,
            (report.match_id, unix + u64::from(seconds)),
        );
        self.send(
            who.addr,
            lobby::premium_post_reward(
                0,
                money,
                experience,
                reputation,
                seconds,
                self.shop.discount(),
            ),
        );
    }

    // ----------------------------------------------------------- the shop

    /// `47 00` for an item the shop sells for gold.
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

    /// Buys an offer and sends what it changed; `ack` is the `83 47`
    /// event. Returns whether it was bought.
    fn buy_offer_at(&mut self, who: &Who, offer_id: u32, discount: f32, ack: u8) -> bool {
        let unix = quests::now();
        let bought = premium::buy_offer(
            &mut who.account.locked(),
            &self.shop,
            offer_id,
            discount,
            unix,
        );
        match bought {
            Ok(purchase) => {
                log::info!(
                    "{} bought offer {offer_id} for {} gold: {:?}",
                    who.name,
                    purchase.cost,
                    purchase.gave
                );
                match ack {
                    2 => {
                        self.send(who.addr, lobby::permitted_offer_bought());
                    }
                    event => self.send_made(who.addr, lobby::permitted_shop_event(event)),
                }
                self.premium_changed(who);
                true
            }
            Err(error) => {
                log::info!("{}: offer {offer_id} refused: {error}", who.name);
                self.deny(who.addr, id::SHOP, error.text_key());
                false
            }
        }
    }

    /// The premium account or the inventory changed by a purchase: the
    /// services (the client never asks for them after one), then the
    /// inventory, the account and the money.
    fn premium_changed(&mut self, who: &Who) {
        let services = wire::services(&who.account.locked(), quests::now());
        self.send(who.addr, services);
        self.push_account(who);
        // The squad mates see the kits.
        self.squads_dirty = true;
    }

    /// The premium half of a match's pay, for an account that bought
    /// premium from the match's results: the account again, its level and
    /// its reputations, and the notices.
    fn claim_premium(&mut self, who: &Who, match_id: u32) {
        let claimed = self.results.claim_premium(
            match_id,
            who.account_id,
            &mut who.account.locked(),
            quests::now(),
        );
        let Some(((money, experience, reputation), notices)) = claimed else {
            log::info!(
                "{}: no premium half of match {match_id:#x} to give",
                who.name
            );
            return;
        };
        log::info!(
            "{} got the premium half of match {match_id:#x}: {money} money, {experience} \
             experience, {reputation} reputation",
            who.name
        );
        self.push_account(who);
        let (skills, reputations) = {
            let account = who.account.locked();
            (wire::level_and_skills(&account), wire::reputations(&account))
        };
        self.send_made(who.addr, skills);
        self.send_made(who.addr, reputations);
        self.notify(who, &notices);
    }

    /// `47 02 <offer> <x>`: an offer of the shop. From the results screen
    /// `x` is the match, whose premium half is then given
    /// (`accept_premium_victory_screen(offer, match)`; PROVEN in the UI).
    pub(super) fn buy_offer(&mut self, who: &Who, offer_id: u32, match_id: u32) {
        if !self.buy_offer_at(who, offer_id, 0.0, 2) {
            return;
        }
        if match_id != 0 && premium::premium_days(offer_id).is_some() {
            self.claim_premium(who, match_id);
            self.premium_offers.remove(&who.account_id);
        }
    }

    /// `47 05 <offer> <discount> <money> <experience> <reputation>
    /// <faction>`: the answer to `A8`, while the offer stands. The amounts
    /// the client names are the ones it was sent; the server gives its own.
    pub(super) fn accept_premium_with_bonuses(
        &mut self,
        who: &Who,
        offer_id: u32,
        discount: f32,
    ) {
        let unix = quests::now();
        let offered = self
            .premium_offers
            .get(&who.account_id)
            .copied()
            .filter(|&(_, until)| unix <= until);
        let good = self.shop.discount_percent > 0
            && (discount - self.shop.discount()).abs() <= DISCOUNT_SLACK
            && premium::premium_days(offer_id).is_some();
        let Some((match_id, _)) = offered.filter(|_| good) else {
            log::info!(
                "{}: no offer of {discount} off premium {offer_id} stands",
                who.name
            );
            self.deny(
                who.addr,
                id::SHOP,
                premium::ShopError::NotOffered.text_key(),
            );
            return;
        };
        if self.buy_offer_at(who, offer_id, self.shop.discount(), 5) {
            self.premium_offers.remove(&who.account_id);
            self.claim_premium(who, match_id);
        }
    }

    /// `47 06 <offer> <discount>`: the answer to `A9`; an offer of -1 is
    /// the window closed. The discount stands for an account without
    /// premium while the operator's setting does.
    pub(super) fn accept_premium_with_discount(&mut self, who: &Who, offer_id: u32, discount: f32) {
        if offer_id == DECLINED {
            return;
        }
        let premium = who.account.locked().is_premium(quests::now());
        let good = self.shop.discount_percent > 0
            && !premium
            && (discount - self.shop.discount()).abs() <= DISCOUNT_SLACK
            && premium::premium_days(offer_id).is_some();
        if !good {
            log::info!(
                "{}: no discount of {discount} on premium {offer_id} is offered",
                who.name
            );
            self.deny(
                who.addr,
                id::SHOP,
                premium::ShopError::NotOffered.text_key(),
            );
            return;
        }
        self.buy_offer_at(who, offer_id, self.shop.discount(), 2);
    }

    /// `46 04 <item>`: use an item (the one-day premium voucher).
    pub(super) fn use_item(&mut self, who: &Who, item_id: u32) {
        let used = premium::use_item(&mut who.account.locked(), item_id, quests::now());
        match used {
            Ok(used) => {
                log::info!(
                    "{} used item {item_id}: {} hours of premium, {} left",
                    who.name,
                    used.premium_hours,
                    used.left
                );
                // The client takes one off its copy; the inventory that
                // follows agrees.
                self.send(who.addr, lobby::permitted_item_used(item_id));
                self.premium_changed(who);
            }
            Err(error) => {
                log::info!("{}: item {item_id} not used: {error}", who.name);
                self.deny(who.addr, id::INVENTORY, error.text_key());
            }
        }
    }

    // ---------------------------------------------------------- nicknames

    /// What is wrong with `nickname` for the account, in the client's own
    /// reasons: the name's own rules, then whether another account is
    /// shown by it or signs in with it.
    pub(super) fn nickname_refusal(&self, nickname: &str, account_id: u32) -> Option<NameRefusal> {
        if let Some(refusal) = name_refusal(nickname, true) {
            return Some(refusal);
        }
        let shown = self.account_named(nickname, account_id).is_some();
        let login = self
            .store
            .account_id_by_name(nickname)
            .is_some_and(|owner| owner != account_id);
        (shown || login).then_some(NameRefusal::Taken)
    }

    /// `49 04 <name> <voucher item or 0>`: a new nickname, paid with a
    /// rename voucher or in gold.
    pub(super) fn change_nickname(&mut self, who: &Who, nickname: &str, voucher_item_id: u32) {
        let nickname = nickname.trim();
        if let Some(refusal) = self.nickname_refusal(nickname, who.account_id) {
            log::info!(
                "{}: the nickname `{nickname}` is refused: {refusal:?}",
                who.name
            );
            self.deny(who.addr, id::PROFILE, "st_change_name_error");
            return;
        }
        let renamed = names::rename(
            &mut who.account.locked(),
            &self.shop,
            nickname,
            voucher_item_id,
        );
        let paid = match renamed {
            Ok(paid) => paid,
            Err(error) => {
                log::info!("{}: the rename is refused: {error:?}", who.name);
                self.deny(who.addr, id::PROFILE, error.text_key());
                return;
            }
        };
        let name = who.account.locked().name.clone();
        // The name others find the account by: the store's.
        if let Some(account) = self.store.account(who.account_id) {
            account.locked().name = name.clone();
            self.store.mark_dirty(who.account_id);
        }
        if let Some(player) = self.player_mut(who.addr) {
            player.name = name.clone();
        }
        log::info!(
            "{} (account {}) is now named {name} ({} gold, voucher {})",
            who.name,
            who.account_id,
            paid.gold,
            paid.voucher_item_id
        );
        self.send(
            who.addr,
            lobby::permitted_nickname_changed(paid.voucher_item_id),
        );
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

//! The requests about one item (`lobby_requests.md`): its cosmetics
//! (`46 01`, `02`, `05`), changing a modification (`46 06`..`08`),
//! upgrading it with a donor (`46 09`) and the repair (`47 03`). The rules
//! are `session::v026g::items_rules`; here is what the client is sent for
//! each, and what it needs to get out of the state its window is in:
//!
//! | request | done | refused |
//! |---|---|---|
//! | `46 01` attach | `83 46 01`, then the inventory and the account; on a weapon the inventory and the account alone | `84 46` `st_cannot_upgrade_item`, then the inventory and the account (the window showed the cosmetic in its slot already) |
//! | `46 02` detach | `83 46 02`, then the same (also for a cosmetic that is gone: sold, Flash's `on_item_sold` asks after the sale); on a weapon the inventory and the account alone | the same |
//! | `46 05` place a decal | nothing: the client wrote the value itself and has no handler for an answer; on a weapon the inventory and the account (patch 47: the lobby builds the weapon again with it) | `84 46`, then the inventory and the account |
//! | `46 06` window opened | `83 46 06`, and `83 46 07` again for a change not chosen yet | `84 46` `st_cannot_select_rerolled_modification` |
//! | `46 07` reroll | `83 46 07` and the money | `84 46`, then `83 46 06`: the window is locked until it is filled again |
//! | `46 08` choose | `83 46 08` (the client changes its copies itself; keeping the old one: the old one into itself) | `84 46`, then `83 46 06` (and `07`) |
//! | `46 09` craft | `83 46 09`, then the inventory, the account and the money | `83 46 09 FE`: the window waits for a result, and shows `st_cannot_craft_item` for this one |
//! | `47 03` repair | `83 47 03`, then the inventory (the client leaves the defects on its copy), the account and the money | `84 47` with the rule's text |
//!
//! A repair paid in silver for an item with defects takes time: when it is
//! over the client is told with an important message, `#ir[<item>]`.
//!
//! A weapon's upgrades are the weapon kit's (`items_rules`,
//! `survarium::v026g::weapon_kit`): up to six, sent packed into three
//! records. The client's own copy of an attach or a detach (`83 46 01`,
//! `83 46 02`) would write a record of a logical slot into its item, a
//! fourth beside the decals and the kit (the client's fatal assert), or
//! take the kit off for its own; so a weapon's change is answered with the
//! inventory and the account alone, which the client takes as they are
//! (patch 48's preview waits for the account).

use session::LockExt;
use session::v026g::items_rules::{self, Detached, ItemError, RerollWindow};
use session::v026g::{quests, wire};
use survarium::v026g::lobby::requests::{RepairPayment, id};
use survarium::v026g::lobby::{self, Crafted};

use super::core::{Core, Who};

impl Core<'_> {
    fn refuse_item(&mut self, who: &Who, op: u8, what: &str, error: ItemError) {
        log::info!("{}: {what} refused: {error}", who.name);
        self.deny(who.addr, op, error.text_key());
    }

    /// The account's kits or what they wear changed in a way its squad
    /// mates see.
    fn item_changed(&mut self, who: &Who) {
        self.push_account(who);
        self.squads_dirty = true;
    }

    /// `host` is a weapon of the account: its upgrades are the weapon
    /// kit's, answered with the inventory and the account alone.
    fn is_weapon(&self, who: &Who, host: u32) -> bool {
        who.account
            .locked()
            .item(host)
            .is_some_and(|item| items_rules::is_weapon(item.dict_id))
    }

    // ------------------------------------------------------- cosmetics

    /// `46 01 <kit> <host> <upgrade> <slot> <u64 0>`.
    pub(super) fn attach_upgrade(&mut self, who: &Who, host: u32, upgrade: u32, slot: u8) {
        let attached =
            items_rules::attach_upgrade(&mut who.account.locked(), host, upgrade, slot);
        let weapon = self.is_weapon(who, host);
        match attached {
            Ok(_) if weapon => None,
            Ok(attached) => self.send(
                who.addr,
                lobby::permitted_upgrade_attached(
                    attached.host_item_id,
                    attached.upgrade_item_id,
                    attached.slot,
                    attached.value,
                    attached.detach_first,
                ),
            ),
            Err(error) => {
                self.refuse_item(who, id::INVENTORY, "an upgrade", error);
                None
            }
        };
        self.item_changed(who);
    }

    /// `46 02 <kit> <upgrade> <host>` (the host is patch 30's).
    pub(super) fn detach_upgrade(&mut self, who: &Who, host: u32, upgrade: u32) {
        let weapon = self.is_weapon(who, host);
        let detached = items_rules::detach_upgrade(&mut who.account.locked(), host, upgrade);
        match detached {
            Ok(_) if weapon => {}
            // Gone: Flash's on_item_sold, after the sale's answer (the
            // cosmetic came off every item with it). It is off: a refusal
            // would show the player an error for the sale.
            Ok(Detached::Off | Detached::Gone) => {
                self.send(who.addr, lobby::permitted_upgrade_detached(upgrade, host));
            }
            Err(error) => self.refuse_item(who, id::INVENTORY, "taking an upgrade off", error),
        }
        self.item_changed(who);
    }

    /// `46 05 <kit> <host> <upgrade> <u64>`: the decal editor's value. The
    /// client has written it into its own copy and reads no answer. A
    /// weapon's: the account too, which builds the weapon of the lobby's
    /// view again with the decal's new place (patch 47; the stock editor
    /// baked an armour's itself).
    pub(super) fn place_upgrade(&mut self, who: &Who, host: u32, upgrade: u32, value: u64) {
        let placed = items_rules::place_upgrade(&mut who.account.locked(), host, upgrade, value);
        match placed {
            Ok(true) => self.item_changed(who),
            Ok(false) => {
                self.accounts.mark_dirty(who.account_id);
                self.squads_dirty = true;
            }
            Err(error) => {
                self.refuse_item(who, id::INVENTORY, "a decal's place", error);
                // The client's copy, as it is here.
                self.push_account(who);
            }
        }
    }

    // ---------------------------------------------------------- reroll

    /// `83 46 06`, and `83 46 07` for a change that waits for its choice.
    fn send_reroll_window(&mut self, who: &Who, window: &RerollWindow) {
        self.send(
            who.addr,
            lobby::permitted_reroll_window(
                window.item_id,
                window.regular_cost,
                window.premium_cost,
                window.changed,
            ),
        );
        if let Some(choices) = window.offer {
            let wait = window.changed.map_or(0, |(_, seconds)| seconds);
            self.send(
                who.addr,
                lobby::permitted_reroll_result(window.item_id, choices, wait),
            );
        }
    }

    /// The window as it is, after a refusal: the client's is locked until
    /// it is filled again.
    fn refill_reroll_window(&mut self, who: &Who, item_id: u32) {
        let window = items_rules::reroll_window(&who.account.locked(), item_id);
        if let Ok(window) = window {
            self.send_reroll_window(who, &window);
        }
    }

    /// `46 06 <item>`.
    pub(super) fn open_reroll(&mut self, who: &Who, item_id: u32) {
        let window = items_rules::reroll_window(&who.account.locked(), item_id);
        match window {
            Ok(window) => self.send_reroll_window(who, &window),
            Err(error) => self.refuse_item(
                who,
                id::INVENTORY,
                &format!("a reroll window for item {item_id}"),
                error,
            ),
        }
    }

    /// `46 07 <item> <modification> <gold>`.
    pub(super) fn reroll_modifier(&mut self, who: &Who, item_id: u32, modification: u32, gold: bool) {
        let rolled = {
            let mut account = who.account.locked();
            items_rules::reroll(
                &mut account,
                item_id,
                modification,
                gold,
                quests::now(),
                &mut self.rng,
            )
            .map(|rolled| (rolled, wire::money(&account)))
        };
        match rolled {
            Ok((rolled, money)) => {
                if rolled.again {
                    log::info!(
                        "{} asks to change modification {modification} of item {item_id}: \
                         the pending offer for {} is shown again, nothing charged",
                        who.name,
                        rolled.replaced
                    );
                } else {
                    log::info!(
                        "{} changes modification {modification} of item {item_id} for {:?}",
                        who.name,
                        rolled.paid
                    );
                }
                self.accounts.mark_dirty(who.account_id);
                self.send(
                    who.addr,
                    lobby::permitted_reroll_result(item_id, rolled.choices, rolled.reroll_seconds),
                );
                self.send(who.addr, money);
                if let Some(player) = self.player_mut(who.addr) {
                    let account = who.account.locked();
                    player.money_sent = (account.generic_money, account.premium_money);
                }
            }
            Err(error) => {
                self.refuse_item(
                    who,
                    id::INVENTORY,
                    &format!("a reroll of item {item_id}"),
                    error,
                );
                self.refill_reroll_window(who, item_id);
            }
        }
    }

    /// `46 08 <item> <old modification> <choice>`. Keeping the old one
    /// is answered as a change of it into itself: the client's copies stay
    /// as they are and the window shows the change over.
    pub(super) fn choose_modifier(&mut self, who: &Who, item_id: u32, old: u32, choice: u32) {
        let chosen = items_rules::choose_modifier(
            &mut who.account.locked(),
            item_id,
            old,
            choice,
        );
        match chosen {
            Ok(chosen) => {
                self.accounts.mark_dirty(who.account_id);
                if chosen.new_modification == chosen.old_modification {
                    log::info!("{} keeps modification {old} of item {item_id}", who.name);
                } else {
                    self.squads_dirty = true;
                }
                let window = chosen.window;
                let (changed, wait) = window.changed.unwrap_or((chosen.new_modification, 0));
                debug_assert_eq!(changed, chosen.new_modification);
                self.send(
                    who.addr,
                    lobby::permitted_modifier_chosen(
                        item_id,
                        chosen.old_modification,
                        chosen.new_modification,
                        [window.regular_cost, window.premium_cost, wait],
                    ),
                );
            }
            Err(error) => {
                self.refuse_item(who, id::INVENTORY, "a modification's choice", error);
                self.refill_reroll_window(who, item_id);
            }
        }
    }

    // ----------------------------------------------------------- craft

    /// `46 09 <item> <donor> <gold>`.
    pub(super) fn craft(&mut self, who: &Who, item_id: u32, donor: u32, gold: bool) {
        let queued = self
            .player(who.addr)
            .is_some_and(|player| player.queued_since.is_some());
        let crafted = items_rules::craft(
            &mut who.account.locked(),
            item_id,
            donor,
            gold,
            queued,
            quests::now(),
            &mut self.rng,
        );
        match crafted {
            Ok(crafted) => {
                log::info!("{} upgraded item {item_id} with {donor}: {crafted:?}", who.name);
                self.send_made(who.addr, lobby::permitted_crafted(item_id, crafted));
                self.item_changed(who);
            }
            Err(why) => {
                log::info!("{}: an upgrade of item {item_id} refused: {why}", who.name);
                self.send_made(who.addr, lobby::permitted_crafted(item_id, Crafted::Failed));
            }
        }
    }

    // ---------------------------------------------------------- repair

    /// `47 03 <item> <kind> [<kit item>]`.
    pub(super) fn repair(&mut self, who: &Who, item_id: u32, payment: RepairPayment) {
        let repaired =
            items_rules::repair(&mut who.account.locked(), item_id, payment, quests::now());
        match repaired {
            Ok(repaired) => {
                self.send_made(who.addr, lobby::permitted_repaired(item_id, repaired));
                self.item_changed(who);
            }
            Err(error) => {
                self.refuse_item(who, id::SHOP, &format!("a repair of item {item_id}"), error)
            }
        }
    }

    /// The pass of every tick: the repairs of a player in the lobby that
    /// are over are told to its client (`#ir[<item>]`), which unlocks the
    /// item.
    pub(super) fn item_timers(&mut self, who: &Who) {
        let done = items_rules::finish_repairs(&mut who.account.locked(), quests::now());
        if done.is_empty() {
            return;
        }
        self.accounts.mark_dirty(who.account_id);
        let notices: Vec<_> = done.into_iter().map(items_rules::repaired_notice).collect();
        self.notify(who, &notices);
    }
}

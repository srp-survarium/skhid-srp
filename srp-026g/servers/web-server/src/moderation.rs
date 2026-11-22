//! The admin's account moderation on the 0.26g page
//! (`docs/notes/account-moderation.md`). The owner, 2026-10-20: "we need the
//! ability to take away items in the admin panel", and the silver that
//! players farmed taken out again (the one currency: gold is gone, the
//! owner's of 2026-11-08).
//!
//! - The Money tab: an account's silver changed by a signed
//!   amount, never below 0; a take of more than it has takes what it has,
//!   and the page says how much that was.
//! - The Take items tab: the account's items listed (name, dictionary id,
//!   item id, units or condition, the kits wearing it, modifications); the
//!   ones ticked are taken whole, or units of a stack. The kits follow as
//!   when an item is sold.
//!
//! Both take the account by the user name it signs in with, the character
//! name (if only one account has it), or `#<account id>` (bare digits are a
//! name), and both go in two steps: a request without `confirm=1` only shows
//! what would change (the balances or items before and after) with a
//! one-time token of that plan ([`Plans`]: kept here with the plan, good
//! for [`PLAN_LIFE`]); the confirm sends the same fields, the token and
//! `confirm=1`, uses the token up, and is applied only when the account,
//! planned again under its lock, gives the plan the token was issued for. A
//! reload of the confirm (even once the account is back as it was), an old
//! preview, a change by the game in between, or fields changed after the
//! preview change nothing and show a new preview.
//! A reason is required; it is the admin's only (the log, the audit trail
//! `g26_admin_actions` and this page), never sent to the player.
//!
//! The change is made on srp's live account (`GameAccounts`, which the
//! lobby and the matches share), written at once with its audit row
//! (`GameAccounts::write_admin`: that account only, the database refusing
//! is an error on the page), and then the account's open game is sent its
//! account again, as after a purchase. No other account is touched.
//!
//! The Mute tab (`docs/notes/admin-chat-mute.md`) is one step: an account's
//! chat muted for some hours or for good, with a reason its player is told,
//! or lifted. The mute is the store's (a `session::Ban`, kept with the
//! account like a ban); each one is an audit row.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use session::v026g::account::Account;
use session::v026g::moderation::{
    self, AdminAction, MoneyChange, Pick, TakeRefused, Taking, plan_take, take, take_key,
};
use session::{Ban, LockExt, ResetWrite};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::modifications::Modifications;

use super::v026g::Game;
use super::{
    AdminView, BadHours, Lang, Request, Web, ban_end, ban_until, clock_utc, date_utc, delta,
    escape, unix_now,
};

/// The audit rows shown with an account.
const HISTORY: usize = 8;
/// How long a preview's confirm is good for.
const PLAN_LIFE: Duration = Duration::from_secs(10 * 60);
/// The most previews waiting for their confirm; past it the oldest goes.
const MAX_PLANS: usize = 256;

/// The previews waiting for their confirm: each one's token (random, used
/// once) with the plan it showed ([`MoneyChange::key`], [`take_key`]) and
/// until when it is good.
#[derive(Default)]
pub(crate) struct Plans(Mutex<HashMap<String, (String, Instant)>>);

impl Plans {
    /// A new token for a preview of the plan `key`; `None` if the system
    /// gave no random bytes (no confirm is offered then).
    fn issue(&self, key: String) -> Option<String> {
        let mut bytes = [0_u8; 16];
        if let Err(error) = openssl::rand::rand_bytes(&mut bytes) {
            log::error!("No random bytes for a moderation preview: {error}");
            return None;
        }
        let token: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let now = Instant::now();
        let mut plans = self.0.locked();
        plans.retain(|_, (_, until)| now < *until);
        if plans.len() >= MAX_PLANS
            && let Some(oldest) = plans
                .iter()
                .min_by_key(|(_, (_, until))| *until)
                .map(|(token, _)| token.clone())
        {
            plans.remove(&oldest);
        }
        plans.insert(token.clone(), (key, now + PLAN_LIFE));
        Some(token)
    }

    /// The plan `token` was issued for, the token used up: `None` for one
    /// never issued, used already, or past [`PLAN_LIFE`].
    fn take(&self, token: &str) -> Option<String> {
        let (key, until) = self.0.locked().remove(token)?;
        (Instant::now() < until).then_some(key)
    }

    /// Every preview past its life, for tests.
    #[cfg(test)]
    pub(crate) fn expire_all(&self) {
        let now = Instant::now();
        for (_, until) in self.0.locked().values_mut() {
            *until = now;
        }
    }
}

/// The account as the page shows it.
#[derive(Clone)]
struct Card {
    id: u32,
    /// The user name it signs in with.
    login: String,
    /// The character name other players see.
    name: String,
    /// A game of it has a lobby or chat connection open.
    online: bool,
}

/// What the Money tab shows.
#[derive(Default)]
pub(crate) struct MoneyView {
    user: String,
    silver: String,
    reason: String,
    shown: Option<MoneyShown>,
}

struct MoneyShown {
    card: Card,
    change: MoneyChange,
    /// The one-time token the confirm carries ([`Plans`]).
    plan: Option<String>,
    /// The change is made (the table is of before and after).
    applied: bool,
    /// The confirm button is offered.
    confirmable: bool,
    history: Result<Vec<AdminAction>, String>,
}

/// What the Take items tab shows.
#[derive(Default)]
pub(crate) struct ItemsView {
    user: String,
    reason: String,
    card: Option<Card>,
    rows: Vec<Row>,
    /// The take planned (or made), and the one-time token its confirm
    /// carries ([`Plans`]).
    plan: Option<(Vec<Taking>, Option<String>)>,
    applied: bool,
    confirmable: bool,
    history: Option<Result<Vec<AdminAction>, String>>,
}

/// One item of the listing.
struct Row {
    item_id: u32,
    dict_id: u16,
    name: String,
    is_stack: bool,
    /// Units of a stack, or the condition in percent.
    amount: u32,
    /// `kit <n>, slot <s>` for each kit wearing it (`n` from 1, as the
    /// game's kit tabs).
    worn: Vec<String>,
    modifications: Vec<String>,
    /// The cosmetics on it (camouflage, decals): their item ids.
    upgrades: Vec<u32>,
    /// Ticked, and the units typed: kept from the request.
    ticked: bool,
    units: String,
}

/// `1 234 567`, as the game writes money.
fn grouped(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(digit);
    }
    if value < 0 { format!("-{out}") } else { out }
}

fn signed(value: i64) -> String {
    if value > 0 {
        format!("+{}", grouped(value))
    } else {
        grouped(value)
    }
}

/// What the page says of a write ([`ResetWrite`]), after `done`.
pub(crate) fn written_note(lang: Lang, written: &ResetWrite) -> String {
    match written {
        ResetWrite::Written => String::new(),
        ResetWrite::Pending => lang.t(" The database was busy: srp's saver writes it within seconds (the history below is shown next time)."),
        ResetWrite::Failed(error) => lang.tf(" Error: the change stands in memory only: the database refused it ({error}). srp retries the write; check its log.", &[("error", &error)]),
    }
}

pub(crate) fn log_write(peer: &str, what: &str, written: &ResetWrite) {
    match written {
        ResetWrite::Written => {}
        ResetWrite::Pending => {
            log::warn!("Admin from {peer}: {what}: the database was busy; the saver writes it")
        }
        ResetWrite::Failed(error) => log::error!(
            "Admin from {peer}: {what}: made in memory but not written: {error}; the saver retries"
        ),
    }
}

impl Web {
    /// The account the admin means: `#<id>`, a user name, or the character
    /// name of one account only. Digits without `#` are a name, never an
    /// id: a name that is not found, names several accounts, or is of an
    /// account that has not signed in to 0.26g says so.
    fn moderated(
        &self,
        game: &Game,
        lang: Lang,
        text: &str,
    ) -> Result<(u32, Arc<Mutex<Account>>), String> {
        let text = text.trim();
        let no_such = || {
            lang.t("No 0.26g account has that id.")
        };
        if let Some(id) = text.strip_prefix('#') {
            let id = id.trim().parse::<u32>().map_err(|_| no_such())?;
            return game
                .accounts
                .get(id)
                .map(|account| (id, account))
                .ok_or_else(no_such);
        }
        if self.store.find_account(text).is_none()
            && game
                .accounts
                .ranking()
                .iter()
                .filter(|row| row.1.eq_ignore_ascii_case(text))
                .count()
                > 1
        {
            return Err(lang.t("More than one account has that character name: type its user name or #id."));
        }
        self.find_026g(game, lang, text)
    }

    /// The account's card. Locks the store's account, then the game's,
    /// one at a time: call it with neither locked.
    fn card(&self, game: &Game, account_id: u32) -> Card {
        let login = self
            .store
            .account(account_id)
            .map(|account| account.locked().login.clone())
            .unwrap_or_default();
        let name = game
            .accounts
            .get(account_id)
            .map(|account| account.locked().name.clone())
            .unwrap_or_default();
        Card {
            id: account_id,
            login,
            name,
            online: self.store.is_online(account_id),
        }
    }

    /// The Money tab of the 0.26g page.
    pub(crate) fn admin_moderate_money(
        &self,
        request: &Request,
        peer: &str,
    ) -> (String, MoneyView) {
        let lang = request.lang;
        let mut view = MoneyView {
            user: request.field("user").to_owned(),
            silver: request.field("silver").to_owned(),
            reason: request.field("money_reason").to_owned(),
            shown: None,
        };
        let Some(game) = &self.game else {
            return (String::new(), view);
        };
        // The token alone (the page's first press): nothing asked yet.
        if view.user.is_empty() {
            return (String::new(), view);
        }
        let (account_id, account) = match self.moderated(game, lang, &view.user) {
            Ok(found) => found,
            Err(message) => return (message, view),
        };
        let Some(silver) = delta(&view.silver) else {
            return (
                lang.t("Silver is a whole number, + or -, or empty."),
                view,
            );
        };
        let reason = moderation::clean_reason(&view.reason);
        let confirm = request.field("confirm") == "1";
        // A confirm uses its token up, applied or not.
        let issued = if confirm {
            self.plans.take(request.field("money_plan"))
        } else {
            None
        };
        let (change, applied) = {
            let mut account = account.locked();
            let change = MoneyChange::plan(account_id, &account, silver);
            let apply = confirm
                && reason.is_some()
                && !change.is_nothing()
                && issued.as_deref() == Some(change.key().as_str());
            if apply {
                change.apply(&mut account);
            }
            (change, apply)
        };
        let card = self.card(game, account_id);
        let mut written = None;
        let message = if applied {
            let reason = reason.as_deref().unwrap_or_default();
            let action = change.action(unix_now(), peer, &card.name, reason);
            log::info!(
                "Admin from {peer}: money of {} ({:?}, account {account_id:#x}): {}; before {}, \
                 after {}; reason {reason:?}",
                card.name,
                card.login,
                action.change,
                action.before,
                action.after
            );
            // Written before the game is told, as a match's rewards are.
            let done = game.accounts.write_admin(account_id, action);
            game.lobby.account_changed(account_id);
            log_write(peer, &format!("money of account {account_id:#x}"), &done);
            let note = written_note(lang, &done);
            written = Some(done);
            view.silver.clear();
            view.reason.clear();
            lang.tf("{0}: silver {1} → {2} ({3}).", &[("0", &card.name), ("1", &grouped(change.silver.0.into())), ("2", &grouped(change.silver.1.into())), ("3", &signed(change.silver_change()))]) + &note
        } else if silver == 0 {
            lang.t("Type the silver to add (+) or take (-). The balance is below.")
        } else if change.is_nothing() {
            lang.t("Nothing would change: the account has none of it left to take.")
        } else if confirm && reason.is_none() {
            lang.t("A reason is required: nothing was changed.")
        } else if confirm && issued.is_none() {
            used_or_old(lang)
        } else if confirm {
            lang.t("The account changed since the preview, or the fields did: nothing was changed. Check the new preview below and apply again.")
        } else if reason.is_none() {
            lang.t("Check the change below; type a reason to apply it.")
        } else {
            lang.t("Check the change below, then press Apply. Nothing is changed until then.")
        };
        let plan = (!applied && !change.is_nothing() && reason.is_some())
            .then(|| self.plans.issue(change.key()))
            .flatten();
        view.shown = Some(MoneyShown {
            card,
            confirmable: plan.is_some(),
            plan,
            change,
            applied,
            history: self.history(game, account_id, written.as_ref()),
        });
        (message, view)
    }

    /// The Take items tab of the 0.26g page.
    pub(crate) fn admin_take_items(&self, request: &Request, peer: &str) -> (String, ItemsView) {
        let lang = request.lang;
        let mut view = ItemsView {
            user: request.field("items_user").to_owned(),
            reason: request.field("items_reason").to_owned(),
            ..ItemsView::default()
        };
        let Some(game) = &self.game else {
            return (String::new(), view);
        };
        if view.user.is_empty() {
            let message = lang.t("Type the account: its user name, its character name, or #id.");
            return (message, view);
        }
        let (account_id, account) = match self.moderated(game, lang, &view.user) {
            Ok(found) => found,
            Err(message) => return (message, view),
        };
        let bad_item = |text: &str| {
            lang.tf(
                "{text} is not an item id: nothing was changed.",
                &[("text", &format!("{text:?}"))],
            )
        };
        let bad_units = || {
            lang.t("Units are a whole number above 0, or empty for all of it.")
        };
        let mut picks = Vec::new();
        for text in request.fields("take") {
            let Ok(item_id) = text.parse::<u32>() else {
                return (bad_item(text), view);
            };
            let units = match request.field(&format!("u{item_id}")) {
                "" => None,
                text => match text.parse::<u32>() {
                    Ok(units) => Some(units),
                    Err(_) => return (bad_units(), view),
                },
            };
            picks.push(Pick { item_id, units });
        }
        let reason = moderation::clean_reason(&view.reason);
        let confirm = request.field("confirm") == "1";
        // A confirm uses its token up, applied or not.
        let issued = if confirm {
            self.plans.take(request.field("items_plan"))
        } else {
            None
        };
        let (planned, applied, rows) = {
            let mut account = account.locked();
            let planned = (!picks.is_empty()).then(|| plan_take(&account, &picks));
            let mut applied = false;
            if let Some(Ok(plan)) = &planned
                && confirm
                && reason.is_some()
                && issued.as_deref() == Some(take_key(account_id, plan).as_str())
            {
                take(&mut account, plan);
                applied = true;
            }
            let kept = |item_id: u32| -> (bool, String) {
                if applied {
                    return (false, String::new());
                }
                let ticked = picks.iter().any(|pick| pick.item_id == item_id);
                let units = request.field(&format!("u{item_id}")).to_owned();
                (ticked, units)
            };
            (planned, applied, rows_of(&account, kept))
        };
        let card = self.card(game, account_id);
        view.rows = rows;
        view.card = Some(card.clone());
        let mut written = None;
        let message = match planned {
            None => lang.t("Tick the items to take (and the units of a stack, or all of it), type a reason, then press Preview."),
            Some(Err(refused)) => refused_text(lang, refused),
            Some(Ok(plan)) => {
                let message = if applied {
                    let reason = reason.as_deref().unwrap_or_default();
                    let action = moderation::take_action(
                        &plan,
                        account_id,
                        unix_now(),
                        peer,
                        &card.name,
                        reason,
                    );
                    log::info!(
                        "Admin from {peer}: items of {} ({:?}, account {account_id:#x}) taken: \
                         {}; before {}; after {}; reason {reason:?}",
                        card.name,
                        card.login,
                        action.change,
                        action.before,
                        action.after
                    );
                    let done = game.accounts.write_admin(account_id, action);
                    game.lobby.account_changed(account_id);
                    log_write(peer, &format!("items of account {account_id:#x}"), &done);
                    let note = written_note(lang, &done);
                    written = Some(done);
                    view.reason.clear();
                    lang.tf("{0}: {1} item(s) taken.", &[("0", &card.name), ("1", &plan.len())]) + &note
                } else if confirm && reason.is_none() {
                    lang.t("A reason is required: nothing was changed.")
                } else if confirm && issued.is_none() {
                    used_or_old(lang)
                } else if confirm {
                    lang.t("The items or the choice changed since the preview: nothing was changed. Check the new preview below and apply again.")
                } else if reason.is_none() {
                    lang.t("Check what would be taken below; type a reason to apply it.")
                } else {
                    lang.t("Check what would be taken below, then press Apply. Nothing is changed until then.")
                };
                let token = (!applied && reason.is_some())
                    .then(|| self.plans.issue(take_key(account_id, &plan)))
                    .flatten();
                view.confirmable = token.is_some();
                view.plan = Some((plan, token));
                message
            }
        };
        view.applied = applied;
        view.history = Some(self.history(game, account_id, written.as_ref()));
        (message, view)
    }

    /// The account's audit rows shown with it. After a write (`written`),
    /// which waited for a busy saver already, it does not wait again: a
    /// request waits [`session::RESET_WRITE_WAIT`] once at most.
    fn history(
        &self,
        game: &Game,
        account_id: u32,
        written: Option<&ResetWrite>,
    ) -> Result<Vec<AdminAction>, String> {
        if let Some(ResetWrite::Pending) = written {
            return Err("the database is busy; shown next time".to_owned());
        }
        game.accounts
            .admin_actions(account_id, HISTORY, written.is_none())
    }

    /// The Mute tab (`docs/notes/admin-chat-mute.md`): an account's chat
    /// muted for some hours or for good, with a reason its player is told,
    /// or the mute lifted. It holds from the account's next line; each one
    /// is in the audit trail.
    pub(crate) fn admin_chat_mute(&self, request: &Request, peer: &str) -> String {
        let lang = request.lang;
        let Some(game) = &self.game else {
            return String::new();
        };
        let user = request.field("mute_user");
        // The token alone (the page's first press): nothing asked yet.
        if user.trim().is_empty() {
            return String::new();
        }
        let now = unix_now();
        let ask = match mute_ask(
            request.field("unmute") == "1",
            request.field("mute_hours"),
            request.field("mute_reason"),
            now,
        ) {
            Ok(ask) => ask,
            Err(MuteRefused::Hours) => return BadHours::text(lang),
            Err(MuteRefused::NoReason) => {
                return lang.t("A reason is required: the player is told it.");
            }
        };
        let (account_id, account) = match self.find_026g(game, lang, user) {
            Ok(found) => found,
            Err(message) => return message,
        };
        let name = account.locked().name.clone();
        drop(account);
        let before = self.store.chat_ban(account_id);
        let (message, after, reason) = match ask {
            MuteAsk::Mute(ban) => {
                let message = lang.tf("{name}'s chat muted {0}.", &[("name", &name), ("0", &ban_end(lang, &ban))]);
                let after = Some(ban.clone());
                let reason = ban.reason.clone();
                self.store.mute_chat(account_id, ban);
                (message, after, reason)
            }
            MuteAsk::Unmute => {
                if !self.store.unmute_chat(account_id) {
                    return lang.tf("{name}'s chat was not muted.", &[("name", &name)]);
                }
                let message = lang.tf("{name}'s chat unmuted.", &[("name", &name)]);
                (message, None, String::new())
            }
        };
        let state = |ban: &Option<Ban>| match ban {
            None => "not muted".to_owned(),
            Some(ban) => ban_end(Lang::En, ban),
        };
        let action = AdminAction {
            at: now,
            admin: peer.to_owned(),
            account_id,
            account: name.clone(),
            kind: "chat_mute".to_owned(),
            change: match &after {
                Some(_) => "chat muted".to_owned(),
                None => "chat unmuted".to_owned(),
            },
            reason,
            before: state(&before),
            after: state(&after),
        };
        log::warn!(
            "Admin from {peer}: {} of {name} ({account_id:#x}): {} -> {}; reason {:?}",
            action.change,
            action.before,
            action.after,
            action.reason
        );
        let done = game.accounts.write_admin(account_id, action);
        log_write(
            peer,
            &format!("chat mute of account {account_id:#x}"),
            &done,
        );
        message + &written_note(lang, &done)
    }
}

/// What the Mute tab asks ([`mute_ask`]).
#[derive(Debug, PartialEq, Eq)]
enum MuteAsk {
    Mute(Ban),
    Unmute,
}

#[derive(Debug, PartialEq, Eq)]
enum MuteRefused {
    /// Not hours a ban takes ([`ban_until`]).
    Hours,
    /// A mute needs a reason: its player is told it.
    NoReason,
}

/// The Mute tab's fields as what they ask, at `now` (Unix seconds): the
/// Unmute button, or a mute of `hours` ([`ban_until`]) for `reason`
/// ([`moderation::clean_reason`]).
fn mute_ask(unmute: bool, hours: &str, reason: &str, now: u64) -> Result<MuteAsk, MuteRefused> {
    if unmute {
        return Ok(MuteAsk::Unmute);
    }
    let until = ban_until(hours, now).map_err(|BadHours| MuteRefused::Hours)?;
    let reason = moderation::clean_reason(reason).ok_or(MuteRefused::NoReason)?;
    Ok(MuteAsk::Mute(Ban { until, reason }))
}

/// The Mute tab's panel.
pub(crate) fn mute_panel(lang: Lang) -> String {
    format!(
        "<div class=\"panel p-mute\">\
         <label for=mu>{}</label><input id=mu name=mute_user maxlength=99>\
         <label for=mh>{}</label><input id=mh name=mute_hours inputmode=numeric placeholder=24>\
         <label for=mr>{}</label><input id=mr name=mute_reason maxlength=100>\
         <small>{}</small><br>\
         <button>{}</button> <button name=unmute value=1 class=quiet>{}</button></div>",
        lang.t("Account: user name or character name"),
        lang.t("Hours (empty: for good)"),
        lang.t("Reason (the player is told it)"),
        lang.t("Nobody gets its chat lines, on any channel; it gets \"muted for &lt;reason&gt;\" (or \"muted until &lt;time&gt; UTC: &lt;reason&gt;\") back instead. Unmute needs the account only."),
        lang.t("Mute"),
        lang.t("Unmute"),
    )
}

/// A confirm whose token is not good (used already, or too old).
fn used_or_old(lang: Lang) -> String {
    lang.t("This preview was used already or is older than 10 minutes: nothing was changed. Check the new preview below and apply again.")
}

fn refused_text(lang: Lang, refused: TakeRefused) -> String {
    match refused {
        TakeRefused::Nothing => lang.t("No item was ticked."),
        TakeRefused::TooMany => lang.tf("At most {0} items at once.", &[("0", &moderation::MAX_TAKEN_ITEMS)]),
        TakeRefused::NoSuchItem(item_id) => lang.tf("The account has no item {item_id} now (sold or taken meanwhile): nothing was changed. Check the list again.", &[("item_id", &item_id)]),
        TakeRefused::Twice(item_id) => lang.tf("Item {item_id} is named twice.", &[("item_id", &item_id)]),
        TakeRefused::NoUnits(item_id) => lang.tf("0 units of item {item_id}: leave it empty for all of it.", &[("item_id", &item_id)]),
    }
}

/// The account's items as the listing shows them; `kept` says whether one
/// stays ticked and the units typed for it.
fn rows_of(account: &Account, kept: impl Fn(u32) -> (bool, String)) -> Vec<Row> {
    let dictionary = Dictionary::shared();
    let modifications = Modifications::shared();
    account
        .inventory
        .iter()
        .map(|item| {
            let entry = dictionary.item(item.dict_id);
            let is_stack = dictionary.is_stack(item.dict_id);
            let worn = account
                .kits
                .iter()
                .enumerate()
                .filter_map(|(index, kit)| {
                    let slot = kit.slot_of(item.item_id)?;
                    let active = if kit.id == account.active_kit {
                        "*"
                    } else {
                        ""
                    };
                    Some(format!("kit {}{active}, slot {slot}", index + 1))
                })
                .collect();
            let modifications = item
                .modifications
                .iter()
                .map(|&(id, defect)| {
                    let name = if defect {
                        modifications.defect(id).map(|row| row.name.as_str())
                    } else {
                        modifications.modification(id).map(|row| row.name.as_str())
                    };
                    let kind = if defect { "defect" } else { "mod" };
                    format!("{kind} {id} {}", name.unwrap_or("?"))
                })
                .collect();
            let (ticked, units) = kept(item.item_id);
            Row {
                item_id: item.item_id,
                dict_id: item.dict_id,
                name: entry.map_or_else(|| "?".to_owned(), |entry| entry.name.clone()),
                is_stack,
                amount: if is_stack {
                    item.condition_or_stack
                } else {
                    session::v026g::rules::shown_condition(item.condition_or_stack)
                },
                worn,
                modifications,
                upgrades: item.upgrades.iter().map(|upgrade| upgrade.id).collect(),
                ticked,
                units,
            }
        })
        .collect()
}

fn card_table(lang: Lang, card: &Card) -> String {
    let online = if card.online {
        lang.t("yes: its game is sent the change at once")
    } else {
        lang.t("no: it sees the change at its next sign-in")
    };
    format!(
        "<tr><th>{}</th><td>{}</td></tr>\
         <tr><th>{}</th><td>{}</td></tr>\
         <tr><th>{}</th><td>{} ({:#x})</td></tr>\
         <tr><th>{}</th><td>{}</td></tr>",
        lang.tc("sign-in", "User name"),
        escape(&card.login),
        lang.t("Character"),
        escape(&card.name),
        lang.t("Account id"),
        card.id,
        card.id,
        lang.t("In the game now"),
        escape(&online),
    )
}

fn history_table(lang: Lang, history: &Result<Vec<AdminAction>, String>) -> String {
    let title = lang.t("Earlier moderation of this account");
    match history {
        Err(error) => format!(
            "<p><small>{title}: {}</small></p>",
            escape(&lang.tf("not shown ({error})", &[("error", &error)]))
        ),
        Ok(rows) if rows.is_empty() => format!(
            "<p><small>{title}: {}</small></p>",
            lang.t("none")
        ),
        Ok(rows) => {
            let lines: String = rows
                .iter()
                .map(|row| {
                    format!(
                        "<tr><td>{} {}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        date_utc(row.at),
                        clock_utc(row.at),
                        escape(&row.kind),
                        escape(&row.change),
                        escape(&row.reason)
                    )
                })
                .collect();
            format!(
                "<h3>{title}</h3><div class=scroll><table><tr><th>UTC</th><th>{}</th><th>{}</th>\
                 <th>{}</th></tr>{lines}</table></div>",
                lang.t("What"),
                lang.tc("what", "Change"),
                lang.t("Reason"),
            )
        }
    }
}

/// The Money tab's panel on the 0.26g page. Its first button is the
/// preview: Enter in any field of the form presses that one, never Apply.
pub(crate) fn money_panel(lang: Lang, view: &AdminView) -> String {
    let money = &view.money;
    let shown = match &money.shown {
        Some(shown) if !view.token.is_empty() => {
            let change = &shown.change;
            let line = |before: u32, after: u32, made: i64, asked: i64| {
                let mut text = format!(
                    "{} → {} ({})",
                    grouped(before.into()),
                    grouped(after.into()),
                    signed(made)
                );
                if made != asked {
                    text += &lang.tf("; {0} asked, never below 0", &[("0", &signed(asked))]);
                }
                text
            };
            let button = if shown.confirmable {
                format!(
                    "<button name=confirm value=1>{}</button>",
                    escape(&lang.tf("Apply to {0}", &[("0", &shown.card.name)]))
                )
            } else {
                String::new()
            };
            format!(
                "<h3>{}</h3><table>{}\
                 <tr><th>{}</th><td>{}</td></tr></table>\
                 {}{button}{}",
                if shown.applied {
                    lang.t("Done")
                } else {
                    lang.t("Preview")
                },
                card_table(lang, &shown.card),
                lang.t("Silver"),
                escape(&line(
                    change.silver.0,
                    change.silver.1,
                    change.silver_change(),
                    change.asked
                )),
                shown
                    .plan
                    .as_ref()
                    .map(|token| format!("<input type=hidden name=money_plan value={token}>"))
                    .unwrap_or_default(),
                history_table(lang, &shown.history),
            )
        }
        _ => String::new(),
    };
    format!(
        "<div class=\"panel p-money\">\
         <label for=u>{}</label><input id=u name=user maxlength=99 value=\"{}\">\
         <label for=s>{}</label><input id=s name=silver placeholder=-100000 value=\"{}\">\
         <label for=mr>{}</label><input id=mr name=money_reason maxlength=200 value=\"{}\">\
         <small>{}</small><br><button>{}</button>{shown}</div>",
        lang.t("Account: user name, character name or #id"),
        escape(&money.user),
        lang.t("Silver, + or -"),
        escape(&money.silver),
        lang.t("Reason (required; only admins see it)"),
        escape(&money.reason),
        lang.t("Preview shows the balance before and after; nothing changes until Apply. A balance never goes below 0. An open game shows the change at once."),
        lang.tc("button", "Preview"),
    )
}

/// The Take items tab's panel.
pub(crate) fn items_panel(lang: Lang, view: &AdminView) -> String {
    let items = &view.items;
    let body = match &items.card {
        Some(card) if !view.token.is_empty() => {
            let plan = match &items.plan {
                Some((plan, token)) => {
                    let lines: String = plan
                        .iter()
                        .map(|taking| {
                            let name = items
                                .rows
                                .iter()
                                .find(|row| row.item_id == taking.item_id)
                                .map(|row| row.name.clone())
                                .or_else(|| {
                                    Dictionary::shared()
                                        .item(taking.dict_id)
                                        .map(|entry| entry.name.clone())
                                })
                                .unwrap_or_default();
                            let what = if !taking.is_stack {
                                lang.t("the item")
                            } else if taking.whole() {
                                lang.tf("all {0} units", &[("0", &grouped(taking.had.into()))])
                            } else {
                                lang.tf("{0} of {1} units ({2} left)", &[("0", &grouped(taking.taken.into())), ("1", &grouped(taking.had.into())), ("2", &grouped((taking.had - taking.taken).into()))])
                            };
                            let kits = if taking.worn_in.is_empty() || !taking.whole() {
                                String::new()
                            } else {
                                lang.t("taken off its kits")
                            };
                            format!(
                                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                                escape(&name),
                                taking.item_id,
                                escape(&what),
                                escape(&kits)
                            )
                        })
                        .collect();
                    let button = if items.confirmable {
                        format!(
                            "<button name=confirm value=1>{}</button>",
                            escape(&lang.tf("Apply: take them from {0}", &[("0", &card.name)]))
                        )
                    } else {
                        String::new()
                    };
                    let token = token
                        .as_ref()
                        .map(|token| format!("<input type=hidden name=items_plan value={token}>"))
                        .unwrap_or_default();
                    format!(
                        "<h3>{}</h3><div class=scroll><table><tr><th>{}</th><th>{}</th>\
                         <th>{}</th><th>{}</th></tr>{lines}</table></div>{token}{button}",
                        if items.applied {
                            lang.t("Taken")
                        } else {
                            lang.t("Would be taken")
                        },
                        lang.t("Item"),
                        lang.t("Item id"),
                        lang.t("Taken"),
                        lang.t("Kits"),
                    )
                }
                None => String::new(),
            };
            let rows: String =
                items
                    .rows
                    .iter()
                    .map(|row| {
                        let amount = if row.is_stack {
                            format!(
                                "{} <input name=u{} value=\"{}\" inputmode=numeric placeholder={} \
                             class=units>",
                                escape(&lang.tf("{0} units", &[("0", &grouped(row.amount.into()))])),
                                row.item_id,
                                escape(&row.units),
                                escape(&lang.t("all")),
                            )
                        } else {
                            format!("{} %", row.amount)
                        };
                        let mut extras = row.modifications.clone();
                        extras.extend(row.upgrades.iter().map(|id| {
                            lang.tf("cosmetic {id}", &[("id", &id)])
                        }));
                        format!(
                            "<tr><td><input type=checkbox name=take value={}{} aria-label=\"{}\">\
                         </td><td>{}</td><td>{}</td><td>{}</td><td>{amount}</td><td>{}</td>\
                         <td>{}</td></tr>",
                            row.item_id,
                            if row.ticked { " checked" } else { "" },
                            escape(&row.name),
                            escape(&row.name),
                            row.dict_id,
                            row.item_id,
                            escape(&row.worn.join("; ")),
                            escape(&extras.join("; ")),
                        )
                    })
                    .collect();
            let history = items
                .history
                .as_ref()
                .map(|history| history_table(lang, history))
                .unwrap_or_default();
            format!(
                "<h3>{}</h3><table>{}</table>{plan}\
                 <h3>{}</h3><p><small>{}</small></p><div class=scroll><table class=items>\
                 <tr><th></th><th>{}</th><th>{}</th><th>{}</th><th>{}</th><th>{}</th>\
                 <th>{}</th></tr>{rows}</table></div>{history}",
                lang.t("Account"),
                card_table(lang, card),
                lang.t("Items"),
                lang.t("Tick what to take. A stack takes the units typed, or all of it when empty. Kit * is the one it plays."),
                lang.t("Name"),
                lang.t("Dict id"),
                lang.t("Item id"),
                lang.t("Units / condition"),
                lang.t("Worn"),
                lang.t("Modifications"),
            )
        }
        _ => String::new(),
    };
    format!(
        "<div class=\"panel p-items\">\
         <label for=iu2>{}</label><input id=iu2 name=items_user maxlength=99 value=\"{}\">\
         <label for=ir>{}</label><input id=ir name=items_reason maxlength=200 value=\"{}\">\
         <small>{}</small><br><button>{}</button>{body}</div>",
        lang.t("Account: user name, character name or #id"),
        escape(&items.user),
        lang.t("Reason (required; only admins see it)"),
        escape(&items.reason),
        lang.t("First the list, then a preview of what is taken; nothing changes until Apply. A worn item is taken off every kit, as when it is sold. An open game shows the change at once."),
        lang.t("List / preview"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mute_tabs_fields_parse_to_what_they_ask() {
        let now = 1_000;
        let mute = |until, reason: &str| {
            Ok(MuteAsk::Mute(Ban {
                until,
                reason: reason.to_owned(),
            }))
        };
        assert_eq!(mute_ask(false, "", " spam ", now), mute(None, "spam"));
        assert_eq!(
            mute_ask(false, " 2 ", "spam", now),
            mute(Some(now + 7200), "spam")
        );
        assert_eq!(
            mute_ask(false, "soon", "spam", now),
            Err(MuteRefused::Hours)
        );
        assert_eq!(mute_ask(false, "2", "  ", now), Err(MuteRefused::NoReason));
        // Control characters go; the reason is one line.
        assert_eq!(mute_ask(false, "", "a\nb", now), mute(None, "a b"));
        // Unmute needs nothing else.
        assert_eq!(mute_ask(true, "soon", "", now), Ok(MuteAsk::Unmute));
    }
}

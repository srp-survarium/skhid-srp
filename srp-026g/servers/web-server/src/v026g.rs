//! The page over the 0.26g stack (`srp::v026g`): the admin acts on the
//! 0.26g accounts ([`GameAccounts`]) and tells the lobby, so an open game
//! shows the change.
//!
//! - Silver and gold are exchanged in the game only: the lobby's exchange
//!   window (`client-patches/exchange`, `47 04`, `rules::exchange_premium`
//!   and `rules::buy_premium`). `GET /exchange`, which the SHOP button of
//!   the launchers already in players' hands opens, says so in the
//!   launcher's language ([`in_the_game_now`]); nothing else of it is left.
//! - The admin's tabs: money, the account (premium time, experience or
//!   level, reputation), an item by its `items_dict` id or name, and the
//!   operator's text (`A5` / `A6`) for everyone in the lobby. Bans,
//!   maintenance and registration are the store's, which the 0.26g login
//!   server and lobby read: they are the same code as for 0.21d.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use session::LockExt;
use session::v026g::GameAccounts;
use session::v026g::account::{self, Account, MAX_LEVEL};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lobby::FULL_CONDITION;

use super::{AdminView, Form, Lang, Request, Response, Stats, Web, apply, delta, page, unix_now};

/// What the page tells the 0.26g lobby. srp joins it to the lobby's inbox
/// (`lobby_server::v026g::Inbox`); this crate does not know the lobby.
pub trait LobbyLink: Send + Sync {
    /// The account was changed here: its client, if it is in the lobby, is
    /// sent the account again.
    fn account_changed(&self, account_id: u32);
    /// The operator's two lines for every client (`A5`), until cleared.
    fn set_admin_text(&self, first: &str, second: &str);
    /// Takes them away (`A6`).
    fn clear_admin_text(&self);
    /// The lines that stand.
    fn admin_text(&self) -> Option<(String, String)>;
}

/// The match server's matches created and not yet ended.
pub trait MatchCounter: Send + Sync {
    fn active_matches(&self) -> usize;
    /// Each of them as it is now, the oldest first.
    fn live_matches(&self) -> Vec<LiveMatch>;
}

/// How far a match is: the lobby's list of the matches has the same, and
/// with it the rules of what is listed (`session::v026g::matches_now`).
pub use session::v026g::matches_now::Phase as LivePhase;

/// A match as the launcher's page shows it: no player is named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveMatch {
    /// The `maps_dict` id: the map and the mode.
    pub map_id: u8,
    pub phase: LivePhase,
    /// Each team's players who have not left.
    pub players: [u8; 2],
    /// Since play began.
    pub played: Option<Duration>,
    /// Until the time limit; the match can end sooner.
    pub time_left: Option<Duration>,
    pub standing: Option<LiveStanding>,
}

/// What a match is taken to last when its map is not known: the time all
/// playable modes have (`maps::MapMode`: 900 s, a 20 s countdown, a 5 s
/// wait).
const UNKNOWN_MAP_LASTS: Duration = Duration::from_secs(900 + 20 + 5);

impl LiveMatch {
    /// The most the match can still last, which `GET /api/stats` gives as
    /// `last_match_ends_in`: once the countdown began, its time left (the
    /// launcher's "Time left"); before that, while its players load, its
    /// map's whole wait, countdown and match time (the loading itself may
    /// take a little longer); nothing once it has its result. It can end
    /// sooner (the tickets, the batteries, everyone leaving).
    pub fn ends_within(&self) -> Duration {
        if !self.phase.is_listed() {
            return Duration::ZERO;
        }
        self.time_left.unwrap_or_else(|| {
            survarium::v026g::maps::map_mode(self.map_id).map_or(UNKNOWN_MAP_LASTS, |map| {
                Duration::from_secs(u64::from(
                    map.wait_time + map.countdown_time + map.match_time,
                ))
            })
        })
    }
}

/// The score the match server last trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LiveStanding {
    /// Batteries brought, or tickets left in a Team Deathmatch, by team.
    pub points: [u16; 2],
    /// The team that leads; `None` for a draw.
    pub leader: Option<usize>,
    /// How old the world is that said so.
    pub age: Duration,
}

/// What makes the page a 0.26g one.
#[derive(Clone)]
pub struct Game {
    pub accounts: Arc<GameAccounts>,
    pub lobby: Arc<dyn LobbyLink>,
    pub matches: Arc<dyn MatchCounter>,
    /// The patches of `resources.db` the players' updater is told to fetch
    /// (`GET /patches`): those the server's game data was made of.
    pub patches: Vec<crate::OfferedPatch>,
    /// "Watch replay": the tokens the lobby gives, which `/replays/<token>`
    /// takes (`session::v026g::replays`); `None`: every such request is
    /// "not found".
    pub replays: Option<Arc<session::v026g::replays::Replays>>,
}

/// The longest line of the operator's text: the client's buffers are 64
/// bytes (`social.md` §5).
pub(crate) const MAX_TEXT_LEN: usize = 63;
/// The most of an item the admin gives at once: items that are no stack,
/// and units of a stack.
const MAX_GIVEN_ITEMS: u32 = 8;
const MAX_GIVEN_UNITS: u32 = 100_000;
/// The factions the lobby shows a reputation for.
const FACTIONS: std::ops::RangeInclusive<u8> = 1..=4;

/// Where the experience of `level` starts.
fn level_start(level: u8) -> u32 {
    let mut total = 0;
    loop {
        let (at, start, next) = account::level_at(total);
        if at >= level || next <= total {
            return start;
        }
        total = next;
    }
}

fn spent_skill_points(account: &Account) -> u32 {
    account
        .skills
        .iter()
        .map(|&(_, points)| u32::from(points))
        .sum()
}

impl Web {
    pub(crate) fn route_026g(&self, request: &Request, peer: &str) -> Response {
        let lang = request.lang;
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/") => page(
                lang,
                Form::Home(Stats {
                    registered: self.store.registered_accounts(),
                    online: self.store.players_present(),
                    maintenance: self.store.minutes_to_maintenance(),
                    registrations_open: self.store.registrations_open(),
                    v026g: true,
                    launcher: crate::launcher::offers_launcher(),
                    installs_game: crate::launcher::offers_game_install(),
                }),
                "",
            ),
            ("GET", "/exchange") => page(lang, Form::InTheGame26(launcher_code(request)), ""),
            ("GET", path) if self.is_admin_page(path, request) => page(
                lang,
                Form::Admin(Box::new(AdminView {
                    g26: true,
                    ..AdminView::default()
                })),
                "",
            ),
            ("POST", path) if self.is_admin_page(path, request) => self.admin(request, peer),
            _ => Response {
                status: "404 Not Found",
                content_type: super::ContentType::Html,
                body: page(lang, Form::None26, &lang.tr("Not found.", "Не знайдено.")).body,
            },
        }
    }

    /// The 0.26g account the admin means by `text`: a user name it signs in
    /// with, or the name other players see if only one account has it.
    fn find_026g(
        &self,
        game: &Game,
        lang: Lang,
        text: &str,
    ) -> Result<(u32, Arc<Mutex<Account>>), String> {
        let text = text.trim();
        let by_nickname = || {
            let mut found = game
                .accounts
                .ranking()
                .into_iter()
                .filter(|row| row.1.eq_ignore_ascii_case(text))
                .map(|row| row.0);
            let first = found.next()?;
            found.next().is_none().then_some(first)
        };
        let Some(account_id) = self.store.find_account(text).or_else(by_nickname) else {
            return Err(lang.tr("No such user.", "Такого користувача немає."));
        };
        match game.accounts.get(account_id) {
            Some(account) => Ok((account_id, account)),
            None => Err(lang.tr(
                "That account has not signed in to the 0.26g lobby yet: it has nothing to change.",
                "Цей акаунт ще не входив у лобі 0.26g: змінювати нічого.",
            )),
        }
    }

    /// The account was changed: written soon, and its open game is told.
    fn changed_026g(&self, game: &Game, account_id: u32) {
        game.accounts.mark_dirty(account_id);
        game.lobby.account_changed(account_id);
    }

    pub(crate) fn admin_money_026g(&self, request: &Request, peer: &str) -> String {
        let lang = request.lang;
        let Some(game) = &self.game else {
            return String::new();
        };
        let user = request.field("user");
        let (account_id, account) = match self.find_026g(game, lang, user) {
            Ok(found) => found,
            Err(message) => return message,
        };
        let (Some(silver), Some(gold)) =
            (delta(request.field("silver")), delta(request.field("gold")))
        else {
            return lang.tr(
                "Silver and gold are whole numbers, + or -, or empty.",
                "Срібло і золото — цілі числа, + або -, або порожньо.",
            );
        };
        let (silver_now, gold_now) = {
            let mut account = account.locked();
            account.generic_money = apply(account.generic_money, silver);
            account.premium_money = apply(account.premium_money, gold);
            (account.generic_money, account.premium_money)
        };
        self.changed_026g(game, account_id);
        log::info!(
            "Admin from {peer}: {user:?} silver {silver:+}, gold {gold:+} \
             (now {silver_now} silver, {gold_now} gold)"
        );
        lang.tr(
            &format!("{user}: now {silver_now} silver and {gold_now} gold."),
            &format!("{user}: тепер {silver_now} срібла і {gold_now} золота."),
        )
    }

    /// Premium time, experience or level, reputation: each field that is
    /// filled is applied; nothing is when one of them is wrong.
    pub(crate) fn admin_account_026g(&self, request: &Request, peer: &str) -> String {
        let lang = request.lang;
        let Some(game) = &self.game else {
            return String::new();
        };
        let user = request.field("acc_user");
        let (account_id, account) = match self.find_026g(game, lang, user) {
            Ok(found) => found,
            Err(message) => return message,
        };
        let bad = lang.tr(
            &format!(
                "Premium hours, experience and reputation are whole numbers, + or -; the level \
                 is 0 to {MAX_LEVEL}; the faction is 1 to 4."
            ),
            &format!(
                "Години преміуму, досвід і репутація — цілі числа, + або -; рівень від 0 до \
                 {MAX_LEVEL}; фракція від 1 до 4."
            ),
        );
        let (Some(hours), Some(experience), Some(reputation)) = (
            delta(request.field("premium_hours")),
            delta(request.field("experience")),
            delta(request.field("reputation")),
        ) else {
            return bad;
        };
        let level = match request.field("level") {
            "" => None,
            text => match text.parse::<u8>() {
                Ok(level) if level <= MAX_LEVEL => Some(level),
                _ => return bad,
            },
        };
        let faction = match request.field("faction") {
            "" if reputation == 0 => None,
            text => match text.parse::<u8>() {
                Ok(faction) if FACTIONS.contains(&faction) => Some(faction),
                _ => return bad,
            },
        };
        let now = unix_now();
        let summary = {
            let mut account = account.locked();
            let mut changed = account.clone();
            // The level, then the experience on top of it.
            if let Some(level) = level {
                changed.exp_total = level_start(level);
            }
            changed.exp_total = apply(changed.exp_total, experience);
            if u32::from(changed.level()) < spent_skill_points(&changed) {
                return lang.tr(
                    "That level is under the skill points the account has spent; reset its \
                     skills in the game first.",
                    "Цей рівень нижчий за витрачені очки навичок; спершу скиньте навички в грі.",
                );
            }
            if hours != 0 {
                let until = i64::try_from(changed.premium_until.max(now)).unwrap_or(i64::MAX);
                let until = until.saturating_add(hours.saturating_mul(3600));
                changed.premium_until = u64::try_from(until)
                    .ok()
                    .filter(|at| *at > now)
                    .unwrap_or(0);
            }
            if let Some(faction) = faction {
                if reputation >= 0 {
                    changed.add_reputation(faction, reputation as u32);
                } else if let Some(entry) = changed
                    .reputations
                    .iter_mut()
                    .find(|(id, _)| *id == faction)
                {
                    entry.1 = apply(entry.1, reputation);
                }
            }
            *account = changed;
            (
                account.level(),
                account.exp_total,
                account.premium_hours(now).unwrap_or(0),
                faction.map(|faction| (faction, account.reputation(faction))),
            )
        };
        self.changed_026g(game, account_id);
        let (level_now, exp_now, premium_now, reputation_now) = summary;
        log::info!(
            "Admin from {peer}: {user:?} premium {hours:+} h, experience {experience:+}, level \
             {level:?}, reputation {reputation:+} with {faction:?} (now level {level_now}, \
             {exp_now} experience, {premium_now} h premium)"
        );
        let mut message = lang.tr(
            &format!(
                "{user}: level {level_now} ({exp_now} experience), {premium_now} h of premium."
            ),
            &format!("{user}: рівень {level_now} ({exp_now} досвіду), {premium_now} год преміуму."),
        );
        if let Some((faction, points)) = reputation_now {
            message += &lang.tr(
                &format!(" Reputation with faction {faction}: {points}."),
                &format!(" Репутація у фракції {faction}: {points}."),
            );
        }
        message
    }

    /// An item of the 0.26g dictionary, by its id or its name: new and
    /// whole, into the inventory.
    pub(crate) fn admin_item_026g(&self, request: &Request, peer: &str) -> String {
        let lang = request.lang;
        let Some(game) = &self.game else {
            return String::new();
        };
        let user = request.field("item_user");
        let (account_id, account) = match self.find_026g(game, lang, user) {
            Ok(found) => found,
            Err(message) => return message,
        };
        let dictionary = Dictionary::shared();
        let wanted = request.field("item");
        // Only an id the dictionary has: the client crashes on any other.
        let item = match wanted.parse::<u16>() {
            Ok(dict_id) => dictionary.item(dict_id),
            Err(_) => dictionary.item_named(wanted),
        };
        let Some(item) = item else {
            return lang.tr(
                "No such item in the 0.26g dictionary (give its id or its exact name).",
                "Такого предмета немає в словнику 0.26g (вкажіть id або точну назву).",
            );
        };
        let most = if item.is_stack {
            MAX_GIVEN_UNITS
        } else {
            MAX_GIVEN_ITEMS
        };
        let count = match request.field("count") {
            "" => 1,
            text => match text.parse::<u32>() {
                Ok(count) if (1..=most).contains(&count) => count,
                _ => {
                    return lang.tr(
                        &format!("The count for this item is 1 to {most}."),
                        &format!("Кількість для цього предмета від 1 до {most}."),
                    );
                }
            },
        };
        {
            let mut account = account.locked();
            if item.is_stack {
                // A stack is one instance: its count grows.
                let dict_id = item.dict_id;
                let owned = account
                    .inventory
                    .iter_mut()
                    .find(|owned| owned.dict_id == dict_id);
                match owned {
                    Some(owned) => {
                        let Some(total) = owned.condition_or_stack.checked_add(count) else {
                            return lang.tr("That stack would overflow.", "Ця купа переповниться.");
                        };
                        owned.condition_or_stack = total;
                    }
                    None => {
                        account.add_item(dict_id, count);
                    }
                }
            } else {
                for _ in 0..count {
                    account.add_item(item.dict_id, FULL_CONDITION);
                }
            }
            account.sync_kits();
        }
        self.changed_026g(game, account_id);
        log::info!(
            "Admin from {peer}: {user:?} given {count} of item {} ({})",
            item.dict_id,
            item.name
        );
        lang.tr(
            &format!(
                "{user}: given {count} of {} (id {}).",
                item.name, item.dict_id
            ),
            &format!(
                "{user}: видано {count} × {} (id {}).",
                item.name, item.dict_id
            ),
        )
    }

    /// The operator's text for everyone in the lobby: set, or cleared.
    pub(crate) fn admin_text_026g(&self, request: &Request, peer: &str) -> String {
        let lang = request.lang;
        let Some(game) = &self.game else {
            return String::new();
        };
        if !request.field("clear_text").is_empty() {
            game.lobby.clear_admin_text();
            log::warn!("Admin from {peer}: cleared the text for all players");
            return lang.tr("The text is cleared.", "Текст прибрано.");
        }
        let (first, second) = (request.field("line1"), request.field("line2"));
        if first.is_empty() && second.is_empty() {
            return String::new();
        }
        let fits = |line: &str| line.len() <= MAX_TEXT_LEN && !line.chars().any(char::is_control);
        if !fits(first) || !fits(second) {
            return lang.tr(
                &format!(
                    "Each line is at most {MAX_TEXT_LEN} bytes (a Cyrillic letter is 2) with no \
                     control characters."
                ),
                &format!(
                    "Кожен рядок — щонайбільше {MAX_TEXT_LEN} байти (кирилична літера — 2), без \
                     керівних символів."
                ),
            );
        }
        game.lobby.set_admin_text(first, second);
        log::warn!("Admin from {peer}: text for all players: {first:?} / {second:?}");
        lang.tr(
            "The text is shown to everyone in the lobby, and to who signs in, until cleared.",
            "Текст показано всім у лобі й тим, хто входить, доки його не приберуть.",
        )
    }
}

/// The language of `/exchange?lang=<code>` as the launcher's SHOP button
/// sends it: `ru`, `de` and `pl` as they are (this page has its text in
/// them too), else the page's own ([`Lang::pick`]: `ua`, `uk`, `en`, the
/// browser's).
pub(crate) fn launcher_code(request: &Request) -> &'static str {
    let query = request
        .target
        .split_once('?')
        .map_or("", |(_, query)| query);
    let chosen = query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == "lang")
        .map_or("", |(_, value)| value);
    match chosen.to_ascii_lowercase().as_str() {
        "ru" => "ru",
        "de" => "de",
        "pl" => "pl",
        _ => request.lang.code(),
    }
}

/// `/exchange`: the exchange is in the game now. The launchers already in
/// players' hands open this address with their SHOP button, so it answers
/// in the launcher's five languages (`code` of [`launcher_code`]).
pub(crate) fn in_the_game_now(code: &str) -> String {
    let (code, title, text) = match code {
        "uk" => (
            "uk",
            "Тепер у грі",
            "Срібло й золото тепер обмінюються в грі: у лобі натисніть на своє золото чи \
             срібло вгорі екрана.",
        ),
        "ru" => (
            "ru",
            "Теперь в игре",
            "Серебро и золото теперь обмениваются в игре: в лобби нажмите на своё золото или \
             серебро вверху экрана.",
        ),
        "de" => (
            "de",
            "Jetzt im Spiel",
            "Silber und Gold werden jetzt im Spiel getauscht: Klicke in der Lobby oben auf dein \
             Gold oder Silber.",
        ),
        "pl" => (
            "pl",
            "Teraz w grze",
            "Srebro i złoto wymienia się teraz w grze: w lobby kliknij swoje złoto lub srebro u \
             góry ekranu.",
        ),
        _ => (
            "en",
            "In the game now",
            "Silver and gold are exchanged in the game now: in the lobby, click your gold or \
             silver at the top of the screen.",
        ),
    };
    format!("<section class=card lang={code}><h1>{title}</h1><p>{text}</p></section>")
}

#[cfg(test)]
mod tests;

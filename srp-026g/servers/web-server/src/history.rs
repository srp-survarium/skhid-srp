//! The site's stats: their page (`/stats`, a tab of its own beside the wiki:
//! a frame its script fills when it is viewed) and the API the script asks,
//! `/api/history/...` (`docs/notes/match_history.md` has every endpoint and
//! its shape): public matches, players, top lists and clans from the match
//! history srp keeps ([`session::v026g::persist::history`]). A request is parsed once into an
//! [`Endpoint`]; every answer is one indexed query of at most
//! [`MatchHistory::MAX_ROWS`] rows (a clan card's members: up to
//! `clans::MAX_MEMBERS`; `player/days`: up to `MAX_DAYS` days).

use serde_json::{Value, json};
use std::time::Instant;

use session::SessionStore;
use session::v026g::replays::{Replays, Status};
use session::v026g::persist::history::{
    ClanKey, ClanLine, Counters, Gear, ItemTotals, KitCounts, KitLine, MatchHistory, MatchInfo,
    MatchSummary,
    Named, Outcome, PlayerLine, PlayerRanking, Presence, Totals, TotalsKey, Winner,
};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::maps;

use crate::icons::{self, IconSet};
use crate::{ContentType, Game, Lang, Response, decode, launcher, modifiers, wiki};

/// Where the API is.
pub(crate) const PREFIX: &str = "/api/history/";
/// The ask for a match's replay: the one endpoint that starts work (a
/// conversion), so the web server says how often an address may ask
/// (`Web::replay_ask_allowed`).
pub(crate) const REPLAY: &str = "/api/history/replay";
/// The stats page.
pub(crate) const PATH: &str = "/stats";
/// The page's policy: the wiki's (`damage::POLICY`), and its script may ask
/// the site for the history's JSON.
pub(crate) const POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; \
    script-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; form-action 'self'";
const SCRIPT: &[u8] = include_bytes!("../stats/history.js");
const SCRIPT_PATH: &str = "/stats/history.js";
const STYLE: &str = include_str!("../stats/style.css");

/// The page's script's address with its version, so it can be kept for good.
fn script_url() -> &'static str {
    static URL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    URL.get_or_init(|| {
        let digest: String = openssl::sha::sha256(SCRIPT)[..6]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("{SCRIPT_PATH}?v={digest}")
    })
}

pub(crate) fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    (path == SCRIPT_PATH).then_some(("text/javascript; charset=utf-8", SCRIPT))
}

/// The page's frame under `header`, in the wiki's look. Its script fills it,
/// as the address says: a player (`?player=<name>`), a match
/// (`?match=<number>`), a clan (`?clan=<tag>`), else the leaderboards.
pub(crate) fn page(lang: Lang, header: &str) -> Response {
    let stats = lang.tr("Stats", "Статистика");
    let body = format!(
        "<style>{STYLE}</style>\
         <p class=breadcrumb><a href=\"{PATH}?lang={code}\">{stats}</a><span id=stats-crumb></span></p>\
         <div class=wiki-heading><div><p class=eyebrow id=stats-eyebrow>{eyebrow}</p>\
         <h1 id=stats-title>{stats}</h1></div></div>\
         <div id=stats><noscript><p class=wiki-intro>{noscript}</p></noscript></div>\
         <script src=\"{}\"></script>",
        script_url(),
        code = lang.code(),
        eyebrow = lang.tr("Public matches", "Публічні матчі"),
        noscript = lang.tr("The stats need JavaScript.", "Статистиці потрібен JavaScript."),
    );
    wiki::framed(lang, &stats, &lang.tr("Skhid", "Схід"), header, &body)
}

/// A page's length when the request names none.
const PAGE: usize = 15;
/// The days a player's days cover when the request names none, and at most.
const DAYS: u64 = 30;
const MAX_DAYS: u64 = 90;
/// How many lines the best single-match scores have.
const BEST: usize = 20;
/// How many weapons and armour pieces a player's favourites list when the
/// request names no limit.
const FAVOURITES: usize = 10;
/// What a leaders' list ranks a player's day by.
type Leading = fn(&Totals) -> u64;
/// The day's leaders' lists: each its name and what it ranks by, most first
/// (ties: the older account), of those above 0.
const LEADERS: [(&str, Leading); 5] = [
    ("kills", |totals| totals.kills),
    ("assists", |totals| totals.counters.assists),
    ("score", |totals| totals.score),
    ("wins", |totals| totals.wins),
    ("played_s", |totals| totals.played_ms / 1000),
];
/// How many players a leaders' list names.
const LEADERS_EACH: usize = 5;
const DAY: u64 = 86_400;
const HOUR: u64 = 3_600;

/// A page of a list: the matches before a match number (all with `None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Page {
    before: Option<u64>,
    limit: usize,
}

/// How a player is named.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PlayerKey {
    Id(u32),
    Name(String),
}

/// How a clan is named.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ClanName {
    Id(u32),
    Tag(String),
}

/// The best scores' period.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Period {
    Hour,
    Day,
}

impl Period {
    fn seconds(self) -> u64 {
        match self {
            Self::Hour => HOUR,
            Self::Day => DAY,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Hour => "hour",
            Self::Day => "day",
        }
    }
}

/// A request of the API, parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Endpoint {
    Player(PlayerKey),
    PlayerMatches { account_id: u32, page: Page },
    PlayerDays { account_id: u32, days: u64 },
    PlayerEquipment { account_id: u32, limit: usize },
    Matches { page: Page, clan: Option<u32> },
    Match(u64),
    Replay(u64),
    Top { by: PlayerRanking, limit: usize },
    Best(Period),
    Today,
    AllTime,
    Clans { by: TotalsKey, limit: usize },
    Clan(ClanName),
}

/// Why a request is not answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    BadRequest,
    NotFound,
    /// srp keeps no database (`SRP_DB=off`).
    NoHistory,
    /// The database refused; logged.
    Failed,
}

impl Refusal {
    fn response(self) -> Response {
        let (status, error) = match self {
            Self::BadRequest => ("400 Bad Request", "bad request"),
            Self::NotFound => ("404 Not Found", "not found"),
            Self::NoHistory => ("503 Service Unavailable", "no history"),
            Self::Failed => ("500 Internal Server Error", "failed"),
        };
        Response {
            status,
            content_type: ContentType::Json,
            body: json!({ "error": error }).to_string(),
        }
    }
}

/// The answer to an address that asks for replays too often.
pub(crate) fn too_many() -> Response {
    Response {
        status: "429 Too Many Requests",
        content_type: ContentType::Json,
        body: json!({ "error": "too many" }).to_string(),
    }
}

impl From<session::persist::PersistError> for Refusal {
    fn from(error: session::persist::PersistError) -> Self {
        log::error!("history: {error}");
        Self::Failed
    }
}

/// The request's parameters, decoded.
struct Query(Vec<(String, String)>);

impl Query {
    fn of(query: &str) -> Self {
        Self(
            query
                .split('&')
                .filter_map(|pair| pair.split_once('='))
                .map(|(key, value)| (decode(key), decode(value)))
                .collect(),
        )
    }

    fn text(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// A number parameter: `None` without it, a refusal when it is no
    /// number.
    fn number<T: std::str::FromStr>(&self, name: &str) -> Result<Option<T>, Refusal> {
        self.text(name)
            .map(|text| text.parse().map_err(|_| Refusal::BadRequest))
            .transpose()
    }

    fn required<T: std::str::FromStr>(&self, name: &str) -> Result<T, Refusal> {
        self.number(name)?.ok_or(Refusal::BadRequest)
    }

    /// `limit`, 1 to [`MatchHistory::MAX_ROWS`].
    fn limit(&self, default: usize) -> Result<usize, Refusal> {
        match self.number::<usize>("limit")? {
            None => Ok(default),
            Some(limit) if (1..=MatchHistory::MAX_ROWS).contains(&limit) => Ok(limit),
            Some(_) => Err(Refusal::BadRequest),
        }
    }

    fn page(&self) -> Result<Page, Refusal> {
        Ok(Page {
            before: self.number("before")?,
            limit: self.limit(PAGE)?,
        })
    }
}

fn totals_key(text: &str) -> Option<TotalsKey> {
    Some(match text {
        "matches" => TotalsKey::Matches,
        "wins" => TotalsKey::Wins,
        "kills" => TotalsKey::Kills,
        _ => return None,
    })
}

fn totals_key_name(key: TotalsKey) -> &'static str {
    match key {
        TotalsKey::Matches => "matches",
        TotalsKey::Wins => "wins",
        TotalsKey::Kills => "kills",
    }
}

fn ranking_name(by: PlayerRanking) -> &'static str {
    match by {
        PlayerRanking::Rating => "rating",
        PlayerRanking::Totals(key) => totals_key_name(key),
        PlayerRanking::Accuracy => "accuracy",
        PlayerRanking::ScoreAvg => "score_avg",
        PlayerRanking::WinRate => "win_rate",
        PlayerRanking::Kd => "kd",
        PlayerRanking::HeadshotRate => "headshot_rate",
    }
}

impl Endpoint {
    /// The request at `path` (under [`PREFIX`]) with `query`.
    fn parse(path: &str, query: &str) -> Result<Self, Refusal> {
        let query = Query::of(query);
        let name = path.strip_prefix(PREFIX).ok_or(Refusal::NotFound)?;
        Ok(match name {
            "player" => match (query.number("id")?, query.text("name")) {
                (Some(id), _) => Self::Player(PlayerKey::Id(id)),
                (None, Some(name)) if !name.trim().is_empty() => {
                    Self::Player(PlayerKey::Name(name.trim().to_owned()))
                }
                _ => return Err(Refusal::BadRequest),
            },
            "player/matches" => Self::PlayerMatches {
                account_id: query.required("id")?,
                page: query.page()?,
            },
            "player/days" => Self::PlayerDays {
                account_id: query.required("id")?,
                days: match query.number("days")? {
                    None => DAYS,
                    Some(days) if (1..=MAX_DAYS).contains(&days) => days,
                    Some(_) => return Err(Refusal::BadRequest),
                },
            },
            "player/equipment" => Self::PlayerEquipment {
                account_id: query.required("id")?,
                limit: query.limit(FAVOURITES)?,
            },
            "matches" => Self::Matches {
                page: query.page()?,
                clan: query.number("clan")?,
            },
            "match" => Self::Match(query.required("id")?),
            "replay" => Self::Replay(query.required("id")?),
            "top" => Self::Top {
                by: match query.text("by") {
                    None | Some("rating") => PlayerRanking::Rating,
                    Some("accuracy") => PlayerRanking::Accuracy,
                    Some("score_avg") => PlayerRanking::ScoreAvg,
                    Some("win_rate") => PlayerRanking::WinRate,
                    Some("kd") => PlayerRanking::Kd,
                    Some("headshot_rate") => PlayerRanking::HeadshotRate,
                    Some(text) => {
                        PlayerRanking::Totals(totals_key(text).ok_or(Refusal::BadRequest)?)
                    }
                },
                limit: query.limit(MatchHistory::MAX_ROWS)?,
            },
            "best" => Self::Best(match query.text("period") {
                None | Some("day") => Period::Day,
                Some("hour") => Period::Hour,
                Some(_) => return Err(Refusal::BadRequest),
            }),
            "today" => Self::Today,
            "totals" => Self::AllTime,
            "clans" => Self::Clans {
                by: match query.text("by") {
                    None => TotalsKey::Matches,
                    Some(text) => totals_key(text).ok_or(Refusal::BadRequest)?,
                },
                limit: query.limit(MatchHistory::MAX_ROWS)?,
            },
            "clan" => match (query.number("id")?, query.text("tag")) {
                (Some(id), _) => Self::Clan(ClanName::Id(id)),
                (None, Some(tag)) if !tag.trim().is_empty() => {
                    Self::Clan(ClanName::Tag(tag.trim().to_owned()))
                }
                _ => return Err(Refusal::BadRequest),
            },
            _ => return Err(Refusal::NotFound),
        })
    }
}

/// The answer to `GET <path>?<query>` (`path` under [`PREFIX`]).
pub(crate) fn response(
    path: &str,
    query: &str,
    lang: Lang,
    store: &SessionStore,
    game: &Game,
    now: u64,
) -> Response {
    let answer = Endpoint::parse(path, query).and_then(|endpoint| {
        let history = game.accounts.history().ok_or(Refusal::NoHistory)?;
        let banned = |account_id: u32| store.account_ban(account_id).is_some();
        answer(&endpoint, history, game.replays.as_deref(), &banned, lang, now)
    });
    match answer {
        Ok(body) => Response {
            status: "200 OK",
            content_type: ContentType::Json,
            body: body.to_string(),
        },
        Err(refusal) => refusal.response(),
    }
}

fn answer(
    endpoint: &Endpoint,
    history: &MatchHistory,
    replays: Option<&Replays>,
    banned: &dyn Fn(u32) -> bool,
    lang: Lang,
    now: u64,
) -> Result<Value, Refusal> {
    Ok(match endpoint {
        Endpoint::Player(key) => {
            let account_id = match key {
                PlayerKey::Id(id) => *id,
                PlayerKey::Name(name) => history.account_by_name(name)?.ok_or(Refusal::NotFound)?,
            };
            let card = history.player(account_id)?.ok_or(Refusal::NotFound)?;
            json!({
                "player": named(&card.player),
                "level": card.level,
                "experience": card.experience,
                "rating": card.rating,
                "totals": totals(&card.totals),
            })
        }
        Endpoint::PlayerMatches { account_id, page } => {
            let rows = history.player_matches(*account_id, page.before, page.limit)?;
            let next = next(page, rows.iter().map(|played| played.summary.id));
            let matches: Vec<Value> = rows
                .iter()
                .map(|played| {
                    let mut value = match_summary(&played.summary, lang);
                    extend(&mut value, line_fields(&played.line));
                    extend(&mut value, json!({ "kits": kits(&played.kits, lang) }));
                    value
                })
                .collect();
            json!({ "matches": matches, "next": next })
        }
        Endpoint::PlayerEquipment { account_id, limit } => {
            let favourites = history.favourites(*account_id, *limit)?;
            let list = |items: &[ItemTotals]| -> Vec<Value> {
                items
                    .iter()
                    .map(|item| {
                        let mut value = item_ref(item.dict_id, lang);
                        extend(&mut value, json!({ "matches": item.matches }));
                        value
                    })
                    .collect()
            };
            json!({
                "weapons": list(&favourites.weapons),
                "armour": list(&favourites.armour),
            })
        }
        Endpoint::PlayerDays { account_id, days } => {
            let today = now - now % DAY;
            let since = today.saturating_sub((days - 1) * DAY);
            let days: Vec<Value> = history
                .player_days(*account_id, since)?
                .into_iter()
                .map(|(day, sum)| {
                    let mut value = json!({ "day": day });
                    extend(&mut value, totals(&sum));
                    value
                })
                .collect();
            json!({ "days": days })
        }
        Endpoint::Matches { page, clan } => {
            let rows = history.matches(page.before, page.limit, *clan)?;
            let next = next(page, rows.iter().map(|summary| summary.id));
            let matches: Vec<Value> = rows.iter().map(|row| match_summary(row, lang)).collect();
            json!({ "matches": matches, "next": next })
        }
        Endpoint::Match(id) => {
            let (summary, players) = history.game_match(*id)?.ok_or(Refusal::NotFound)?;
            let players: Vec<Value> = players
                .iter()
                .map(|player| {
                    let mut value =
                        json!({ "player": named(&player.player), "level": player.level });
                    extend(&mut value, line_fields(&player.line));
                    extend(&mut value, json!({ "kits": kits(&player.kits, lang) }));
                    value
                })
                .collect();
            // Whether the page offers its replay: one made, or a recording
            // to make it from.
            let replay = replays.is_some_and(|replays| {
                replays.has_recording(summary.replay_id.match_id(), Instant::now())
            });
            json!({ "match": match_summary(&summary, lang), "players": players, "replay": replay })
        }
        Endpoint::Replay(id) => {
            let (summary, _) = history.game_match(*id)?.ok_or(Refusal::NotFound)?;
            let asked = match replays {
                Some(replays) => {
                    replays.ask_for_site(summary.replay_id.match_id(), Instant::now())
                }
                None => return Ok(json!({ "status": Status::Off.word() })),
            };
            let mut value = json!({ "status": asked.status.word() });
            if asked.status == Status::Preparing {
                extend(&mut value, json!({ "place": asked.place }));
            }
            if let Some(ready) = &asked.ready {
                extend(
                    &mut value,
                    json!({
                        "url": format!("/replays/{}", ready.token),
                        "size": ready.size,
                        "sha256": ready.sha256,
                    }),
                );
            }
            value
        }
        Endpoint::Top { by, limit } => {
            let players: Vec<Value> = history
                .top(*by, *limit, &|account_id| !banned(account_id))?
                .iter()
                .map(|top| {
                    json!({
                        "player": named(&top.player),
                        "level": top.level,
                        "rating": top.rating,
                        "totals": totals(&top.totals),
                    })
                })
                .collect();
            json!({ "by": ranking_name(*by), "players": players })
        }
        Endpoint::Best(period) => {
            let rows: Vec<Value> = history
                .best(now.saturating_sub(period.seconds()), BEST, &|account_id| {
                    !banned(account_id)
                })?
                .iter()
                .map(|best| {
                    let mut value = json!({
                        "player": named(&best.player),
                        "match": best.summary.id,
                        "ended_at": best.summary.info.ended_at,
                        "level": best.summary.info.match_level,
                        "score": best.line.score,
                        "kills": best.line.kills,
                        "deaths": best.line.deaths,
                    });
                    extend(&mut value, played(&best.summary.info, lang));
                    value
                })
                .collect();
            json!({ "period": period.name(), "rows": rows })
        }
        Endpoint::Today => {
            let today = now - now % DAY;
            let all = history.totals_since(today, &|account_id| !banned(account_id))?;
            let mut leaders = serde_json::Map::new();
            for (name, of) in LEADERS {
                let mut ranked: Vec<&(Named, Totals)> =
                    all.iter().filter(|(_, totals)| of(totals) > 0).collect();
                ranked.sort_by_key(|(player, totals)| {
                    (std::cmp::Reverse(of(totals)), player.account_id)
                });
                let rows: Vec<Value> = ranked
                    .iter()
                    .take(LEADERS_EACH)
                    .map(|(player, totals)| json!({ "player": named(player), "value": of(totals) }))
                    .collect();
                leaders.insert(name.to_owned(), Value::Array(rows));
            }
            json!({ "since": today, "leaders": leaders })
        }
        Endpoint::AllTime => {
            let all = history.all_time()?;
            let counters = &all.totals.counters;
            json!({
                "players": all.players,
                "matches": all.matches,
                "kills": all.totals.kills,
                "assists": counters.assists,
                "shots": counters.shots,
                "hits": counters.hits,
                "accuracy": accuracy(counters.shots, counters.hits),
                "played_s": all.totals.played_ms / 1000,
            })
        }
        Endpoint::Clans { by, limit } => {
            let clans: Vec<Value> = history.clans(*by, *limit)?.iter().map(clan_line).collect();
            json!({ "by": totals_key_name(*by), "clans": clans })
        }
        Endpoint::Clan(name) => {
            let key = match name {
                ClanName::Id(id) => ClanKey::Id(*id),
                ClanName::Tag(tag) => ClanKey::Tag(tag),
            };
            let card = history.clan(key)?.ok_or(Refusal::NotFound)?;
            let members: Vec<Value> = card
                .members
                .iter()
                .map(|member| {
                    json!({
                        "player": named(&member.player),
                        "commander": member.commander,
                        "level": member.level,
                        "rating": member.rating,
                        "totals": totals(&member.totals),
                    })
                })
                .collect();
            json!({
                "clan": {
                    "id": card.clan.id,
                    "name": card.clan.name,
                    "tag": card.clan.tag,
                    "level": card.clan.level,
                    "members": card.clan.members,
                    "leader": card.leader.as_ref().map(named),
                },
                "totals": totals(&card.clan.totals),
                "members": members,
            })
        }
    })
}

/// The `before` of the page after a full one; `None` after a short one.
fn next(page: &Page, ids: impl Iterator<Item = u64>) -> Option<u64> {
    let ids: Vec<u64> = ids.collect();
    (ids.len() == page.limit)
        .then(|| ids.last().copied())
        .flatten()
}

/// Adds `more`'s fields to `value` (both objects).
fn extend(value: &mut Value, more: Value) {
    if let (Value::Object(value), Value::Object(more)) = (value, more) {
        value.extend(more);
    }
}

fn round(value: f64, places: i32) -> f64 {
    let scale = 10f64.powi(places);
    (value * scale).round() / scale
}

fn named(player: &Named) -> Value {
    json!({
        "id": player.account_id,
        "name": player.name,
        "clan": player.clan.as_ref().map(|clan| json!({ "id": clan.id, "tag": clan.tag })),
    })
}

fn kd(kills: u64, deaths: u64) -> f64 {
    round(kills as f64 / deaths.max(1) as f64, 2)
}

/// Hits by shots, three decimals; 0 with no shot.
fn accuracy(shots: u64, hits: u64) -> f64 {
    if shots == 0 {
        0.0
    } else {
        round(hits as f64 / shots as f64, 3)
    }
}

fn counters(counters: &Counters) -> Value {
    json!({
        "assists": counters.assists,
        "headshots": counters.headshots,
        "grenade_kills": counters.grenade_kills,
        "melee_kills": counters.melee_kills,
        "batteries": counters.batteries,
        "captures": counters.capture_hundredths as f64 / 100.0,
        "artefacts": counters.artefacts,
        "shots": counters.shots,
        "hits": counters.hits,
        "accuracy": accuracy(counters.shots, counters.hits),
    })
}

/// A kit's counters of one match: a kit's, never one item's.
fn kit_counts(counts: &KitCounts) -> Value {
    json!({
        "kills": counts.kills,
        "headshots": counts.headshots,
        "shots": counts.shots,
        "hits": counts.hits,
        "accuracy": accuracy(counts.shots, counts.hits),
    })
}

/// An item by its dict id, with its items_dict name (`key`: the wiki's
/// pages open an item by it) and its name in `lang`, from the game data;
/// both `null` for an item the game data no longer has.
fn item_ref(dict_id: u16, lang: Lang) -> Value {
    let item = Dictionary::shared().item(dict_id);
    json!({
        "id": dict_id,
        "key": item.map(|item| item.name.as_str()),
        "name": item.map(|_| modifiers::item_name(lang, dict_id)),
        "icon": item.and_then(|item| icons::url(IconSet::Item, &item.name)),
    })
}

/// The kits a player used in a match: each its place, its counters, and
/// its weapons and armour by slot.
fn kits(kits: &[KitLine], lang: Lang) -> Vec<Value> {
    kits.iter()
        .map(|kit| {
            let of = |gear: Gear| -> Vec<Value> {
                kit.items
                    .iter()
                    .filter(|item| item.slot.gear() == gear)
                    .map(|item| {
                        let mut value = json!({ "slot": item.slot.number() });
                        extend(&mut value, item_ref(item.dict_id, lang));
                        value
                    })
                    .collect()
            };
            let mut value = json!({
                "kit": kit.kit,
                "weapons": of(Gear::Weapon),
                "armour": of(Gear::Armour),
            });
            extend(&mut value, kit_counts(&kit.counts));
            value
        })
        .collect()
}

fn totals(totals: &Totals) -> Value {
    let per_match = |value: u64, places| {
        if totals.matches == 0 {
            0.0
        } else {
            round(value as f64 / totals.matches as f64, places)
        }
    };
    let mut value = json!({
        "matches": totals.matches,
        "wins": totals.wins,
        "losses": totals.losses,
        "kills": totals.kills,
        "deaths": totals.deaths,
        "score": totals.score,
        "kd": kd(totals.kills, totals.deaths),
        "win_rate": per_match(totals.wins, 3),
        "score_avg": per_match(totals.score, 1),
        "played_s": totals.played_ms / 1000,
    });
    extend(&mut value, counters(&totals.counters));
    value
}

fn winner(winner: Winner) -> Value {
    match winner {
        Winner::FirstTeam => json!(0),
        Winner::SecondTeam => json!(1),
        Winner::Draw => json!("draw"),
        // No winning team: its lines' outcomes say who won.
        Winner::MostKills => json!("free_for_all"),
        Winner::Unknown => Value::Null,
    }
}

/// What a match was played on, in `lang`: the map's name, its key in the
/// wiki's Maps page (its project's folder) and its picture as the rooms draw
/// it (all `null` for a map the game data no longer has; the picture also
/// for one the lobby has none of), the mode's name as the rooms name it, and
/// its rules' number (the Maps page's mode).
fn played(info: &MatchInfo, lang: Lang) -> Value {
    let map = maps::map_mode(info.map_id);
    json!({
        "map": map.map(|map| launcher::map_name(lang, map)),
        "map_key": map.and_then(|map| map.project.split('/').next()),
        "map_icon": map
            .and_then(|map| icons::url(IconSet::Map, &format!("{}_{}", map.level, map.weather))),
        "mode": launcher::lobby_mode_name(lang, info.mode),
        "rules": info.mode.mode().rules().number(),
    })
}

fn match_summary(summary: &MatchSummary, lang: Lang) -> Value {
    let info = &summary.info;
    let mut value = json!({
        "id": summary.id,
        "replay_id": summary.replay_id.as_str(),
        "ended_at": info.ended_at,
        "duration_s": info.duration_ms.div_ceil(1000),
        "map_id": info.map_id.get(),
        "level": info.match_level,
        "winner": winner(info.winner),
        "team_points": info.team_points,
        "players": summary.players,
        "eligibility": summary.eligibility.as_str(),
    });
    extend(&mut value, played(info, lang));
    value
}

/// A player's line of a match.
fn line_fields(line: &PlayerLine) -> Value {
    let mut value = json!({
        "team": line.team,
        "place": line.place,
        "outcome": match line.outcome {
            Outcome::Won => "won",
            Outcome::Lost => "lost",
            Outcome::Neither => "neither",
        },
        "left": line.presence == Presence::Left,
        "rating": line.rating,
        "score": line.score,
        "kills": line.kills,
        "deaths": line.deaths,
        "kd": kd(line.kills, line.deaths),
        "played_s": line.played_ms / 1000,
    });
    extend(&mut value, counters(&line.counters));
    value
}

fn clan_line(clan: &ClanLine) -> Value {
    json!({
        "id": clan.id,
        "name": clan.name,
        "tag": clan.tag,
        "level": clan.level,
        "members": clan.members,
        "totals": totals(&clan.totals),
    })
}

#[cfg(test)]
pub(crate) mod tests;

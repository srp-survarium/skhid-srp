//! The site's stats API, `/api/history/...` (`docs/notes/match_history.md`
//! has every endpoint and its shape): public matches, players, top lists
//! and clans from the match history srp keeps
//! ([`session::v026g::persist::history`]). A request is parsed once into an
//! [`Endpoint`]; every answer is one indexed query of at most
//! [`MatchHistory::MAX_ROWS`] rows.

use serde_json::{Value, json};
use session::SessionStore;
use session::v026g::persist::history::{
    ClanKey, ClanLine, Counters, MatchHistory, MatchSummary, Named, PlayerLine, PlayerRanking,
    Presence, Totals, TotalsKey, Winner,
};
use survarium::v026g::maps;

use crate::{ContentType, Game, Lang, Response, decode, launcher};

/// Where the API is.
pub(crate) const PREFIX: &str = "/api/history/";

/// A page's length when the request names none.
const PAGE: usize = 15;
/// The days a player's days cover when the request names none, and at most.
const DAYS: u64 = 30;
const MAX_DAYS: u64 = 90;
/// How many lines the best single-match scores have.
const BEST: usize = 20;
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
    Matches { page: Page, clan: Option<u32> },
    Match(u64),
    Top { by: PlayerRanking, limit: usize },
    Best(Period),
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
        "score" => TotalsKey::Score,
        "headshots" => TotalsKey::Headshots,
        "batteries" => TotalsKey::Batteries,
        "artefacts" => TotalsKey::Artefacts,
        _ => return None,
    })
}

fn totals_key_name(key: TotalsKey) -> &'static str {
    match key {
        TotalsKey::Matches => "matches",
        TotalsKey::Wins => "wins",
        TotalsKey::Kills => "kills",
        TotalsKey::Score => "score",
        TotalsKey::Headshots => "headshots",
        TotalsKey::Batteries => "batteries",
        TotalsKey::Artefacts => "artefacts",
    }
}

fn ranking_name(by: PlayerRanking) -> &'static str {
    match by {
        PlayerRanking::Rating => "rating",
        PlayerRanking::Totals(key) => totals_key_name(key),
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
            "matches" => Self::Matches {
                page: query.page()?,
                clan: query.number("clan")?,
            },
            "match" => Self::Match(query.required("id")?),
            "top" => Self::Top {
                by: match query.text("by") {
                    None | Some("rating") => PlayerRanking::Rating,
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
            "clans" => Self::Clans {
                by: match query.text("by") {
                    None => TotalsKey::Score,
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
        answer(&endpoint, history, &banned, lang, now)
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
            let next = next(page, rows.iter().map(|(summary, _)| summary.id));
            let matches: Vec<Value> = rows
                .iter()
                .map(|(summary, line)| {
                    let mut value = match_summary(summary, lang);
                    extend(&mut value, line_fields(line, summary.info.winner));
                    value
                })
                .collect();
            json!({ "matches": matches, "next": next })
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
                    extend(&mut value, line_fields(&player.line, summary.info.winner));
                    value
                })
                .collect();
            json!({ "match": match_summary(&summary, lang), "players": players })
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
                    let (map, mode) = names(best.summary.info.map_id, lang);
                    json!({
                        "player": named(&best.player),
                        "match": best.summary.id,
                        "ended_at": best.summary.info.ended_at,
                        "level": best.summary.info.match_level,
                        "map": map,
                        "mode": mode,
                        "score": best.line.score,
                        "kills": best.line.kills,
                        "deaths": best.line.deaths,
                    })
                })
                .collect();
            json!({ "period": period.name(), "rows": rows })
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

fn counters(counters: &Counters) -> Value {
    json!({
        "assists": counters.assists,
        "headshots": counters.headshots,
        "grenade_kills": counters.grenade_kills,
        "melee_kills": counters.melee_kills,
        "batteries": counters.batteries,
        "captures": counters.capture_hundredths as f64 / 100.0,
        "artefacts": counters.artefacts,
    })
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
        "losses": totals.matches.saturating_sub(totals.wins),
        "kills": totals.kills,
        "deaths": totals.deaths,
        "score": totals.score,
        "kd": kd(totals.kills, totals.deaths),
        "win_rate": per_match(totals.wins, 3),
        "score_avg": per_match(totals.score, 1),
    });
    extend(&mut value, counters(&totals.counters));
    value
}

fn winner(winner: Winner) -> Value {
    match winner {
        Winner::FirstTeam => json!(0),
        Winner::SecondTeam => json!(1),
        Winner::Draw => json!("draw"),
        Winner::Unknown => Value::Null,
    }
}

/// The map's and the mode's names in `lang`, from the game data; `null`
/// for a map it no longer has.
fn names(map_id: u8, lang: Lang) -> (Option<String>, Option<String>) {
    match maps::map_mode(map_id) {
        Some(map) => (
            Some(launcher::map_name(lang, map)),
            Some(launcher::mode_name(lang, map)),
        ),
        None => (None, None),
    }
}

fn match_summary(summary: &MatchSummary, lang: Lang) -> Value {
    let info = &summary.info;
    let (map, mode) = names(info.map_id, lang);
    json!({
        "id": summary.id,
        "ended_at": info.ended_at,
        "duration_s": info.duration_ms.div_ceil(1000),
        "map_id": info.map_id,
        "map": map,
        "mode": mode,
        "level": info.match_level,
        "winner": winner(info.winner),
        "team_points": info.team_points,
        "players": summary.players,
    })
}

/// A player's line of a match whose winner is `winner`.
fn line_fields(line: &PlayerLine, match_winner: Winner) -> Value {
    let mut value = json!({
        "team": line.team,
        "place": line.place,
        "won": match_winner.team() == Some(line.team),
        "left": line.presence == Presence::Left,
        "rating": line.rating,
        "score": line.score,
        "kills": line.kills,
        "deaths": line.deaths,
        "kd": kd(line.kills, line.deaths),
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
mod tests;

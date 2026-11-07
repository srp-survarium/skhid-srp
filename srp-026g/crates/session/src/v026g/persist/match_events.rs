//! What happened in a match, in order (`docs/notes/match_history.md`,
//! "Events"): the kills, the deaths, the batteries, the captures and the
//! score, each with its match time and how exact that time is. The match
//! server makes the list from the worlds it reads (`match-server`'s
//! `v026g::events`), live or from a recording, and it is written with its
//! match ([`super::history::record`]), keyed by the match's replay id.

use rusqlite::{Connection, Row, Transaction, params};

use crate::persist::PersistError;
use crate::v026g::replays::ReplayId;

/// The schema step after the history's: the events, and one match a
/// replay id (a match kept live is not kept again from its recording).
pub(super) const MIGRATION: &str = "
    CREATE UNIQUE INDEX g26_matches_by_replay ON g26_matches (replay_id);
    -- The lobby names a match by the match server's id.
    CREATE INDEX g26_matches_by_game_id ON g26_matches (game_match_id);
    CREATE TABLE g26_match_events (
        replay_id      TEXT NOT NULL REFERENCES g26_matches(replay_id) ON DELETE CASCADE,
        -- Its order in the match, by at_ms.
        seq            INTEGER NOT NULL,
        -- Match ms: when it was; with after_ms, the latest it can have been.
        at_ms          INTEGER NOT NULL,
        -- None: at_ms is the game's own time of it. Else it was after this
        -- and at at_ms or before: the two worlds that show it.
        after_ms       INTEGER CHECK (after_ms < at_ms),
        kind           TEXT NOT NULL CHECK (kind IN ('kill', 'death', 'battery_found',
                           'battery_delivered', 'capture', 'score', 'end')),
        -- Roster places (0 to 19) and their accounts (0: the server's bot).
        actor          INTEGER,
        actor_account  INTEGER,
        victim         INTEGER,
        victim_account INTEGER,
        -- A capture's team and unit.
        team           INTEGER,
        unit           INTEGER,
        -- Each team's points after a score change, and at the end.
        points0        INTEGER,
        points1        INTEGER,
        CHECK ((actor IS NULL) = (actor_account IS NULL)),
        CHECK ((victim IS NULL) = (victim_account IS NULL)),
        CHECK ((team IS NULL) = (unit IS NULL)),
        CHECK ((points0 IS NULL) = (points1 IS NULL)),
        CHECK (CASE kind
            WHEN 'kill' THEN actor IS NOT NULL AND team IS NULL AND points0 IS NULL
            WHEN 'death' THEN actor IS NULL AND victim IS NOT NULL AND team IS NULL
                AND points0 IS NULL
            WHEN 'capture' THEN actor IS NULL AND victim IS NULL AND team IS NOT NULL
                AND points0 IS NULL
            WHEN 'score' THEN actor IS NULL AND victim IS NULL AND team IS NULL
                AND points0 IS NOT NULL
            WHEN 'end' THEN actor IS NULL AND victim IS NULL AND team IS NULL
                AND points0 IS NOT NULL
            ELSE actor IS NOT NULL AND victim IS NULL AND team IS NULL AND points0 IS NULL
        END),
        PRIMARY KEY (replay_id, seq)
    ) WITHOUT ROWID;
    ";

/// A roster place and the account in it (0: the server's own bot).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Player {
    pub place: u8,
    pub account_id: u32,
}

/// When an event was, in match ms (the match clock, which a replay's
/// time is).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timing {
    /// The game's own time of it, on the 50 ms tick: a kill whose killer
    /// made no other kill since the world before (its
    /// `last_killed_enemy_ms`), and the match's end.
    Exact(u32),
    /// Some time after `after` and at `by` or before (`after < by`): the
    /// two worlds the server read it from, about 5 s apart (the uploads it
    /// asks for, `Timing::refresh`), more where one was not trusted.
    Between { after: u32, by: u32 },
}

impl Timing {
    /// The time it is shown at: the exact one, or the latest it can be.
    pub fn at(self) -> u32 {
        match self {
            Self::Exact(at) | Self::Between { by: at, .. } => at,
        }
    }
}

/// What happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// An enemy killed; the victim when the worlds say whom. No weapon:
    /// the game never sends it (`docs/notes/kill_weapon.md`).
    Kill {
        killer: Player,
        victim: Option<Player>,
    },
    /// A death no enemy's kill was paired with: a suicide, a fall, an
    /// anomaly, a teammate's shot, or a kill the worlds leave open.
    Death { victim: Player },
    /// A battery picked up for the first time (event 19).
    BatteryFound { by: Player },
    /// A battery put into a container (event 20).
    BatteryDelivered { by: Player },
    /// Research: a team took a unit.
    Capture { team: u8, unit: u8 },
    /// The batteries in each team's container changed (modes 2 and 3; in
    /// Team Deathmatch the tickets follow the deaths, Research's points
    /// grow all the time).
    Score { points: [u16; 2] },
    /// The match ended with these points.
    End { points: [u16; 2] },
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kill { .. } => "kill",
            Self::Death { .. } => "death",
            Self::BatteryFound { .. } => "battery_found",
            Self::BatteryDelivered { .. } => "battery_delivered",
            Self::Capture { .. } => "capture",
            Self::Score { .. } => "score",
            Self::End { .. } => "end",
        }
    }
}

/// One event of a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchEvent {
    pub timing: Timing,
    pub kind: EventKind,
}

/// A match's events as the lobby reads them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchEvents {
    pub replay_id: ReplayId,
    /// By time.
    pub events: Vec<MatchEvent>,
}

/// The columns an event is in, by kind.
struct Columns {
    actor: Option<Player>,
    victim: Option<Player>,
    capture: Option<(u8, u8)>,
    points: Option<[u16; 2]>,
}

impl EventKind {
    fn columns(self) -> Columns {
        let none = Columns {
            actor: None,
            victim: None,
            capture: None,
            points: None,
        };
        match self {
            Self::Kill { killer, victim } => Columns {
                actor: Some(killer),
                victim,
                ..none
            },
            Self::Death { victim } => Columns {
                victim: Some(victim),
                ..none
            },
            Self::BatteryFound { by } | Self::BatteryDelivered { by } => Columns {
                actor: Some(by),
                ..none
            },
            Self::Capture { team, unit } => Columns {
                capture: Some((team, unit)),
                ..none
            },
            Self::Score { points } | Self::End { points } => Columns {
                points: Some(points),
                ..none
            },
        }
    }

    fn of(kind: &str, columns: Columns) -> Option<Self> {
        let Columns {
            actor,
            victim,
            capture,
            points,
        } = columns;
        Some(match (kind, actor, victim, capture, points) {
            ("kill", Some(killer), victim, None, None) => Self::Kill { killer, victim },
            ("death", None, Some(victim), None, None) => Self::Death { victim },
            ("battery_found", Some(by), None, None, None) => Self::BatteryFound { by },
            ("battery_delivered", Some(by), None, None, None) => Self::BatteryDelivered { by },
            ("capture", None, None, Some((team, unit)), None) => Self::Capture { team, unit },
            ("score", None, None, None, Some(points)) => Self::Score { points },
            ("end", None, None, None, Some(points)) => Self::End { points },
            _ => return None,
        })
    }
}

/// Writes the events of the match `replay_id`, whose row is written, in
/// order of time (the extractor's order is the worlds', and an exact time
/// can come before the window of an event read with it).
pub(super) fn record(
    transaction: &Transaction,
    replay_id: &ReplayId,
    events: &[MatchEvent],
) -> Result<(), PersistError> {
    let mut sorted = events.to_vec();
    sorted.sort_by_key(|event| event.timing.at());
    let mut insert = transaction.prepare_cached(
        "INSERT INTO g26_match_events (replay_id, seq, at_ms, after_ms, kind, actor,
             actor_account, victim, victim_account, team, unit, points0, points1)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
    )?;
    for (seq, event) in sorted.iter().enumerate() {
        let after = match event.timing {
            Timing::Exact(_) => None,
            Timing::Between { after, .. } => Some(after),
        };
        let columns = event.kind.columns();
        insert.execute(params![
            replay_id.as_str(),
            seq as i64,
            event.timing.at(),
            after,
            event.kind.as_str(),
            columns.actor.map(|player| player.place),
            columns.actor.map(|player| player.account_id),
            columns.victim.map(|player| player.place),
            columns.victim.map(|player| player.account_id),
            columns.capture.map(|(team, _)| team),
            columns.capture.map(|(_, unit)| unit),
            columns.points.map(|points| points[0]),
            columns.points.map(|points| points[1]),
        ])?;
    }
    Ok(())
}

/// The events of the newest match the match server numbered
/// `game_match_id` (its id is random from each start, so it can come
/// again): `None` for no such match. Of any kind of match: who may see
/// them is the caller's to say.
pub(super) fn of_game_match(
    connection: &Connection,
    game_match_id: u32,
) -> rusqlite::Result<Option<MatchEvents>> {
    let mut find = connection.prepare_cached(
        "SELECT replay_id FROM g26_matches WHERE game_match_id = ?1 ORDER BY id DESC LIMIT 1",
    )?;
    let mut rows = find.query([game_match_id])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let text: String = row.get(0)?;
    let replay_id =
        ReplayId::parse(&text).ok_or_else(|| super::history::bad("replay id", &text))?;
    let mut select = connection.prepare_cached(
        "SELECT at_ms, after_ms, kind, actor, actor_account, victim, victim_account, team, unit,
             points0, points1
         FROM g26_match_events WHERE replay_id = ?1 ORDER BY seq",
    )?;
    let events = select
        .query_map([replay_id.as_str()], event_at)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Some(MatchEvents { replay_id, events }))
}

fn event_at(row: &Row) -> rusqlite::Result<MatchEvent> {
    let at: u32 = row.get(0)?;
    let timing = match row.get::<_, Option<u32>>(1)? {
        None => Timing::Exact(at),
        Some(after) => Timing::Between { after, by: at },
    };
    let player = |at: usize| -> rusqlite::Result<Option<Player>> {
        Ok(match (row.get(at)?, row.get(at + 1)?) {
            (Some(place), Some(account_id)) => Some(Player { place, account_id }),
            _ => None,
        })
    };
    let kind: String = row.get(2)?;
    let columns = Columns {
        actor: player(3)?,
        victim: player(5)?,
        capture: match (row.get(7)?, row.get(8)?) {
            (Some(team), Some(unit)) => Some((team, unit)),
            _ => None,
        },
        points: match (row.get(9)?, row.get(10)?) {
            (Some(first), Some(second)) => Some([first, second]),
            _ => None,
        },
    };
    let kind = EventKind::of(&kind, columns).ok_or_else(|| super::history::bad("event", &kind))?;
    Ok(MatchEvent { timing, kind })
}

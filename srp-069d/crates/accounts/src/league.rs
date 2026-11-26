//! Ranked league state of an account (schema v10): the position in the running season, the league token balance and the
//! per-season history. The rules (points per match, tokens per season) live in `game_data::league`; the lobby glue in
//! `lobby-server/src/league.rs`. Notes: docs/notes/league.md.

use rusqlite::{params, Connection, OptionalExtension};

use crate::{AccountId, Error, Store};

/// How one finished season ended for an account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeasonRecord {
    pub season: u16,
    /// league the account stood in when the season ended
    pub league: u8,
    /// highest league reached in the season (the token reward is by this one)
    pub max_league: u8,
    /// tokens the season end paid
    pub tokens: u32,
    /// unix seconds the season ended (month and year of the history entry)
    pub closed_at: i64,
    /// the season-final message (9F type 0, fields A / B) was already sent
    pub notified: bool,
}

/// The ranked state of one account.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LeagueState {
    /// season the numbers below belong to (0 = never played ranked)
    pub season: u16,
    /// current league 1..=K, 0 = no ranked match played in `season` yet
    pub league: u8,
    /// points inside the league
    pub points: u32,
    /// highest league reached in `season`
    pub max_league: u8,
    /// ranked matches counted in `season`
    pub matches: u32,
    /// league tokens held (8B `league`)
    pub tokens: u32,
    /// finished seasons, oldest first
    pub history: Vec<SeasonRecord>,
}

impl LeagueState {
    /// `true` when nothing is stored for the account (no row is written).
    pub(crate) fn is_empty(&self) -> bool {
        *self == LeagueState::default()
    }
}

/// One row of the running season's ladder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LadderRow {
    pub account: AccountId,
    pub name: String,
    pub league: u8,
    pub max_league: u8,
    pub points: u32,
}

pub(crate) fn load_league(conn: &Connection, id: AccountId) -> Result<LeagueState, Error> {
    let mut st = LeagueState::default();
    let row = conn
        .query_row("SELECT season, league, points, max_league, matches, tokens FROM league WHERE account_id = ?1", [id], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?))
        })
        .optional()?;
    if let Some((season, league, points, max, matches, tokens)) = row {
        st.season = u16::try_from(season).map_err(|_| Error::Range("league season"))?;
        st.league = u8::try_from(league).map_err(|_| Error::Range("league"))?;
        st.points = u32::try_from(points).map_err(|_| Error::Range("league points"))?;
        st.max_league = u8::try_from(max).map_err(|_| Error::Range("max league"))?;
        st.matches = u32::try_from(matches).map_err(|_| Error::Range("league matches"))?;
        st.tokens = u32::try_from(tokens).map_err(|_| Error::Range("league tokens"))?;
    }
    st.history = load_history(conn, id)?;
    Ok(st)
}

fn load_history(conn: &Connection, id: AccountId) -> Result<Vec<SeasonRecord>, Error> {
    let mut st = conn.prepare("SELECT season, league, max_league, tokens, closed_at, notified FROM league_history WHERE account_id = ?1 ORDER BY closed_at, season")?;
    let rows = st.query_map([id], |r| {
        Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, i64>(5)?))
    })?;
    let mut out = Vec::new();
    for r in rows {
        let (season, league, max, tokens, closed_at, notified) = r?;
        out.push(SeasonRecord {
            season: u16::try_from(season).map_err(|_| Error::Range("history season"))?,
            league: u8::try_from(league).map_err(|_| Error::Range("history league"))?,
            max_league: u8::try_from(max).map_err(|_| Error::Range("history max league"))?,
            tokens: u32::try_from(tokens).map_err(|_| Error::Range("history tokens"))?,
            closed_at,
            notified: notified != 0,
        });
    }
    Ok(out)
}

pub(crate) fn save_league(tx: &rusqlite::Transaction<'_>, id: AccountId, l: &LeagueState) -> Result<(), Error> {
    if l.is_empty() {
        return Ok(());
    }
    tx.execute(
        "INSERT INTO league (account_id, season, league, points, max_league, matches, tokens) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, l.season, l.league, l.points, l.max_league, l.matches, l.tokens],
    )?;
    let mut st = tx.prepare("INSERT INTO league_history (account_id, season, league, max_league, tokens, closed_at, notified) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)")?;
    for h in &l.history {
        st.execute(params![id, h.season, h.league, h.max_league, h.tokens, h.closed_at, h.notified as i64])?;
    }
    Ok(())
}

impl Store {
    /// The ladder of `season`: accounts that played ranked in it, best first (league, then points, then the older
    /// account). At most `limit` rows.
    pub fn league_ladder(&self, season: u16, limit: usize) -> Result<Vec<LadderRow>, Error> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT l.account_id, COALESCE(a.nick, 'Player' || a.id), l.league, l.max_league, l.points FROM league l JOIN accounts a ON a.id = l.account_id
             WHERE l.season = ?1 AND l.league > 0 ORDER BY l.league DESC, l.points DESC, l.account_id ASC LIMIT ?2",
        )?;
        let rows = st.query_map(params![season, limit as i64], |r| {
            Ok(LadderRow { account: r.get(0)?, name: r.get(1)?, league: r.get(2)?, max_league: r.get(3)?, points: r.get(4)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Place (1-based) of an account on the ladder of `season`; None when it did not play ranked in it.
    pub fn league_place(&self, id: AccountId, season: u16) -> Result<Option<u32>, Error> {
        let conn = self.conn();
        let me: Option<(i64, i64)> = conn
            .query_row("SELECT league, points FROM league WHERE account_id = ?1 AND season = ?2 AND league > 0", params![id, season], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((league, points)) = me else { return Ok(None) };
        let ahead: i64 = conn.query_row(
            "SELECT COUNT(*) FROM league WHERE season = ?1 AND league > 0 AND (league > ?2 OR (league = ?2 AND points > ?3) OR (league = ?2 AND points = ?3 AND account_id < ?4))",
            params![season, league, points, id],
            |r| r.get(0),
        )?;
        Ok(Some(ahead as u32 + 1))
    }

    /// The ranked rows of `season` for the given accounts (the friends and clan lists of the league window).
    pub fn league_rows_of(&self, ids: &[AccountId], season: u16) -> Result<Vec<LadderRow>, Error> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT l.account_id, COALESCE(a.nick, 'Player' || a.id), l.league, l.max_league, l.points FROM league l JOIN accounts a ON a.id = l.account_id
             WHERE l.account_id = ?1 AND l.season = ?2 AND l.league > 0",
        )?;
        let mut out = Vec::new();
        for id in ids {
            let mut rows = st.query_map(params![id, season], |r| {
                Ok(LadderRow { account: r.get(0)?, name: r.get(1)?, league: r.get(2)?, max_league: r.get(3)?, points: r.get(4)? })
            })?;
            if let Some(r) = rows.next() {
                out.push(r?);
            }
        }
        Ok(out)
    }

    /// Season history of any account (the history window of another player, `4B 04`).
    pub fn league_history(&self, id: AccountId) -> Result<Vec<SeasonRecord>, Error> {
        let conn = self.conn();
        load_history(&conn, id)
    }
}

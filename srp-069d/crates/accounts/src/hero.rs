//! Hero of the day (schema v13) and the bought "specifics" (pocket consumables opened with gold).
//! Rules: `lobby-server/src/hero.rs` and `game_data::hero`; notes `docs/notes/hero-of-the-day.md`, `docs/notes/actions.md`.
//!
//! * `hero_results`: one row per finished ranked match (the facts the nominations rank), kept for two operating periods.
//! * `hero_titles`: the top-three rows of a recalculation (`since` = the start of the recalculation slot, `until` =
//!   `since` + the operating period). A title row is both the ranking the window shows and the 24 h status.
//! * `hero_meta`: the last slot that was calculated.
//! * `specifics_opened`: pocket consumables the player opened with `46 09` (the bit set in the `91` mask).

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{load_profile_conn, save_profile_tx, AccountId, Currency, Error, Store};

/// One finished ranked match as the nominations see it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeroResult {
    pub account: AccountId,
    /// unix seconds the match ended
    pub at: i64,
    /// competitive group index (hero_of_the_day.options `competitive_groups`)
    pub group: u8,
    pub score: u32,
    /// the player's longest kill streak of the match
    pub killstreak: u32,
    pub won: bool,
}

/// A place in a nomination (and the status it gives for the operating period).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeroTitle {
    pub account: AccountId,
    pub group: u8,
    /// 1 best player (score), 2 killstreak, 3 winstreak
    pub nomination: u8,
    /// 1..=3
    pub place: u8,
    /// the ranked value (score, kill streak, win streak)
    pub value: u32,
    pub since: i64,
    pub until: i64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SpecificError {
    #[error("not enough gold")]
    NotEnoughGold,
    #[error("already opened")]
    AlreadyOpened,
    #[error("database: {0}")]
    Db(String),
}

impl Store {
    /// Add the result of a ranked match.
    pub fn hero_add_result(&self, r: &HeroResult) -> Result<(), Error> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO hero_results (account_id, at, grp, score, killstreak, won) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![r.account, r.at, r.group, r.score, r.killstreak, r.won],
        )?;
        Ok(())
    }

    /// Results with `at` in `(from, to]`, oldest first (ties by insertion order).
    pub fn hero_results_between(&self, from: i64, to: i64) -> Result<Vec<HeroResult>, Error> {
        let conn = self.conn();
        let mut st = conn.prepare("SELECT account_id, at, grp, score, killstreak, won FROM hero_results WHERE at > ?1 AND at <= ?2 ORDER BY at, id")?;
        let rows = st.query_map(params![from, to], |r| {
            Ok(HeroResult { account: r.get(0)?, at: r.get(1)?, group: r.get(2)?, score: r.get(3)?, killstreak: r.get(4)?, won: r.get::<_, i64>(5)? != 0 })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The last calculated slot (unix seconds), None before the first calculation.
    pub fn hero_slot(&self) -> Result<Option<i64>, Error> {
        let conn = self.conn();
        Ok(conn.query_row("SELECT value FROM hero_meta WHERE key = 'slot'", [], |r| r.get(0)).optional()?)
    }

    /// Store the titles of the slot starting at `slot` and move the slot forward (one transaction). Results older than
    /// `purge_results_before` and titles that ended before `slot` are deleted.
    pub fn hero_commit_slot(&self, slot: i64, titles: &[HeroTitle], purge_results_before: i64) -> Result<(), Error> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for t in titles {
            tx.execute(
                "INSERT OR REPLACE INTO hero_titles (account_id, grp, nomination, place, value, since, until) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![t.account, t.group, t.nomination, t.place, t.value, t.since, t.until],
            )?;
        }
        tx.execute("DELETE FROM hero_results WHERE at <= ?1", [purge_results_before])?;
        tx.execute("DELETE FROM hero_titles WHERE until <= ?1", [slot])?;
        tx.execute("INSERT INTO hero_meta (key, value) VALUES ('slot', ?1) ON CONFLICT(key) DO UPDATE SET value = ?1", [slot])?;
        tx.commit()?;
        Ok(())
    }

    /// The ranking the window shows: the titles of the newest slot that still has living titles (`until > now`).
    pub fn hero_current_ranking(&self, now: i64) -> Result<Vec<HeroTitle>, Error> {
        let conn = self.conn();
        let since: Option<i64> = conn.query_row("SELECT MAX(since) FROM hero_titles WHERE until > ?1", [now], |r| r.get(0))?;
        let Some(since) = since else { return Ok(vec![]) };
        let mut st = conn.prepare(
            "SELECT account_id, grp, nomination, place, value, since, until FROM hero_titles WHERE since = ?1 AND until > ?2 ORDER BY grp, nomination, place",
        )?;
        let rows = st.query_map(params![since, now], row_title)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Living titles of one account (any slot), best place first.
    pub fn hero_titles_of(&self, id: AccountId, now: i64) -> Result<Vec<HeroTitle>, Error> {
        let conn = self.conn();
        titles_of(&conn, id, now)
    }

    /// Pocket consumables the account opened with gold.
    pub fn specifics_opened(&self, id: AccountId) -> Result<Vec<u16>, Error> {
        let conn = self.conn();
        let mut st = conn.prepare("SELECT dict_id FROM specifics_opened WHERE account_id = ?1 ORDER BY dict_id")?;
        let rows = st.query_map([id], |r| r.get::<_, i64>(0))?;
        let mut out = Vec::new();
        for d in rows {
            out.push(u16::try_from(d?).map_err(|_| Error::Range("specific dict id"))?);
        }
        Ok(out)
    }

    /// Open a specific for `gold` gold: the gold is spent and the row written in one transaction.
    pub fn buy_specific(&self, id: AccountId, dict: u16, gold: u32, now: i64) -> Result<(), SpecificError> {
        let db = |e: rusqlite::Error| SpecificError::Db(e.to_string());
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db)?;
        let have: bool = tx
            .query_row("SELECT EXISTS(SELECT 1 FROM specifics_opened WHERE account_id = ?1 AND dict_id = ?2)", params![id, dict], |r| r.get(0))
            .map_err(db)?;
        if have {
            return Err(SpecificError::AlreadyOpened);
        }
        let mut p = load_profile_conn(&tx, id).map_err(|e| SpecificError::Db(e.to_string()))?;
        if p.spend(Currency::Gold, u64::from(gold)).is_err() {
            return Err(SpecificError::NotEnoughGold);
        }
        save_profile_tx(&tx, id, &p).map_err(|e| SpecificError::Db(e.to_string()))?;
        tx.execute("INSERT INTO specifics_opened (account_id, dict_id, opened_at) VALUES (?1, ?2, ?3)", params![id, dict, now]).map_err(db)?;
        tx.commit().map_err(db)?;
        Ok(())
    }
}

fn row_title(r: &rusqlite::Row<'_>) -> rusqlite::Result<HeroTitle> {
    Ok(HeroTitle { account: r.get(0)?, group: r.get(1)?, nomination: r.get(2)?, place: r.get(3)?, value: r.get(4)?, since: r.get(5)?, until: r.get(6)? })
}

/// Living titles of `id`, best place first (used by `Profile::hero`).
pub(crate) fn titles_of(conn: &Connection, id: AccountId, now: i64) -> Result<Vec<HeroTitle>, Error> {
    let mut st = conn.prepare(
        "SELECT account_id, grp, nomination, place, value, since, until FROM hero_titles WHERE account_id = ?1 AND until > ?2 ORDER BY place, nomination, since DESC",
    )?;
    let rows = st.query_map(params![id, now], row_title)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

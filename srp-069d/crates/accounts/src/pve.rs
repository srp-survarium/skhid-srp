//! PvE storage (schema v13, docs/notes/pve.md section 6): finished runs (top 100, the `99` result), the chests of a run
//! per player (idempotent open / reroll) and the wins of a day (the free daily keys).
//!
//! The rules (what a win pays, which chest, what a reroll costs) live in `game_data::pve` and the lobby's `pve.rs`; this
//! module only stores and reads.

use rusqlite::{params, Connection, OptionalExtension};

use crate::{AccountId, Error, Profile, Store, UpdateError};

/// One player of a finished run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PveRunPlayer {
    pub account: AccountId,
    pub score: u16,
    pub kills: u16,
    pub deaths: u16,
    pub sort: u8,
    pub premium: bool,
}

/// A finished PvE run (1..=3 players, one squad).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PveRun {
    pub match_id: u32,
    pub mission: u32,
    pub difficulty: u8,
    pub success: bool,
    pub time_secs: u32,
    pub chests_found: u8,
    pub chests_total: u8,
    pub started_at: i64,
    pub players: Vec<PveRunPlayer>,
}

impl PveRun {
    /// The key of the squad: its account ids, sorted, comma separated (a squad of 1..=3 players is one ladder entry).
    pub fn squad_key(&self) -> String {
        let mut ids: Vec<AccountId> = self.players.iter().map(|p| p.account).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.iter().map(|i| i.to_string()).collect::<Vec<_>>().join(",")
    }
}

/// The key a run is stored under (`pve_results.match_id`, `pve_rewards.match_id`): the wire match id in the low 32 bits
/// and the number of earlier runs with that wire id above them. A PvE match id is the captured world's id, so every run
/// of one maze has the same wire id; the first run's key is the wire id itself.
pub fn pve_run_key(wire_id: u32, n: u32) -> i64 {
    i64::from(wire_id) | (i64::from(n) << 32)
}

/// The wire match id of a run key ([`pve_run_key`]).
pub fn pve_wire_id(run_key: i64) -> u32 {
    run_key as u32
}

/// One ladder entry: the best run of a squad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PveLadderRow {
    pub result_id: i64,
    /// the run's key ([`pve_run_key`]) and its wire match id
    pub run_key: i64,
    pub match_id: u32,
    pub difficulty: u8,
    pub chests_found: u8,
    pub time_secs: u32,
}

/// One chest of a run for one player (`5F 05` opens it, `5F 06` rerolls the contents).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PveRewardRow {
    pub idx: u8,
    /// The closed chest shown on the results screen.
    pub chest_dict: u16,
    /// Equipment tier N (1..=5) and difficulty index of the content table `pve_mission_1_chest_rank_NM`.
    pub tier: u8,
    pub difficulty: u8,
    pub opened: bool,
    /// The item that came out (0 = not rolled yet) and its amount.
    pub reward_dict: u16,
    pub reward_amount: u32,
    pub rerolls: u32,
    /// Gold was paid for opening this chest (idempotence of `5F 05`).
    pub paid: bool,
}

impl Store {
    /// Store a finished run as a NEW run and return its key ([`pve_run_key`]: `run.match_id` with the number of runs of
    /// that wire id stored before, counted in the same transaction). Every call stores: the caller makes sure one match
    /// end is stored once (the lobby's `Running::pve_sent`).
    pub fn pve_add_result(&self, run: &PveRun) -> Result<i64, Error> {
        if run.players.is_empty() || run.players.len() > 3 {
            return Err(Error::Invalid("a PvE run has 1 to 3 players"));
        }
        let squad = run.squad_key();
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let n: i64 = tx.query_row(
            "SELECT COALESCE(MAX(match_id >> 32) + 1, 0) FROM pve_results WHERE (match_id & 4294967295) = ?1",
            [run.match_id],
            |r| r.get(0),
        )?;
        let n = u32::try_from(n).map_err(|_| Error::Invalid("too many PvE runs with one match id"))?;
        let key = pve_run_key(run.match_id, n);
        tx.execute(
            "INSERT INTO pve_results (match_id, mission, difficulty, success, time_secs, chests_found, chests_total, started_at, squad_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![key, run.mission, run.difficulty, run.success as i64, run.time_secs, run.chests_found, run.chests_total, run.started_at, squad],
        )?;
        let id = tx.last_insert_rowid();
        for (slot, p) in run.players.iter().enumerate() {
            tx.execute(
                "INSERT INTO pve_result_players (result_id, account_id, slot, score, kills, deaths, sort, premium) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![id, p.account, slot as i64, p.score, p.kills, p.deaths, p.sort, p.premium as i64],
            )?;
        }
        tx.commit()?;
        Ok(key)
    }

    /// The ladder of `mission`: the best successful run of every squad, best first (more chests, then the shorter time,
    /// then the older run), at most `limit` entries.
    pub fn pve_ladder(&self, mission: u32, limit: usize) -> Result<Vec<PveLadderRow>, Error> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT r.id, r.match_id, r.difficulty, r.chests_found, r.time_secs FROM pve_results r
             WHERE r.success = 1 AND r.mission = ?1 AND NOT EXISTS (
                 SELECT 1 FROM pve_results b WHERE b.success = 1 AND b.mission = r.mission AND b.squad_key = r.squad_key AND
                     (b.chests_found > r.chests_found OR (b.chests_found = r.chests_found AND (b.time_secs < r.time_secs OR (b.time_secs = r.time_secs AND b.id < r.id)))))
             ORDER BY r.chests_found DESC, r.time_secs ASC, r.id ASC LIMIT ?2",
        )?;
        let rows = st.query_map(params![mission, limit as i64], |r| {
            let run_key: i64 = r.get(1)?;
            Ok(PveLadderRow { result_id: r.get(0)?, run_key, match_id: pve_wire_id(run_key), difficulty: r.get(2)?, chests_found: r.get(3)?, time_secs: r.get(4)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The result ids of every run `account` took part in.
    pub fn pve_result_ids_of(&self, account: AccountId) -> Result<Vec<i64>, Error> {
        let conn = self.conn();
        let mut st = conn.prepare("SELECT result_id FROM pve_result_players WHERE account_id = ?1")?;
        let rows = st.query_map([account], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The players of a stored run, in slot order.
    pub fn pve_run_players(&self, result_id: i64) -> Result<Vec<PveRunPlayer>, Error> {
        let conn = self.conn();
        let mut st = conn.prepare("SELECT account_id, score, kills, deaths, sort, premium FROM pve_result_players WHERE result_id = ?1 ORDER BY slot")?;
        let rows = st.query_map([result_id], |r| {
            Ok(PveRunPlayer { account: r.get(0)?, score: r.get(1)?, kills: r.get(2)?, deaths: r.get(3)?, sort: r.get(4)?, premium: r.get::<_, i64>(5)? != 0 })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The stored run of a run key ([`pve_run_key`]); its `match_id` is the wire id.
    pub fn pve_result(&self, run_key: i64) -> Result<Option<(PveRun, i64)>, Error> {
        let match_id = pve_wire_id(run_key);
        let row = {
            let conn = self.conn();
            conn.query_row(
                "SELECT id, mission, difficulty, success, time_secs, chests_found, chests_total, started_at FROM pve_results WHERE match_id = ?1",
                [run_key],
                |r| Ok((r.get::<_, i64>(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
            )
            .optional()?
        };
        let Some((id, mission, difficulty, success, time_secs, chests_found, chests_total, started_at)) = row else { return Ok(None) };
        let players = self.pve_run_players(id)?;
        Ok(Some((PveRun { match_id, mission, difficulty, success: success != 0, time_secs, chests_found, chests_total, started_at, players }, id)))
    }

    /// Store the chests of a run for a player. Rows that exist stay as they are (idempotent).
    pub fn pve_add_rewards(&self, account: AccountId, run: i64, rows: &[PveRewardRow]) -> Result<(), Error> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for r in rows {
            insert_reward(&tx, account, run, r)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The chests of a run (its key, [`pve_run_key`]) for a player, by index.
    pub fn pve_rewards(&self, account: AccountId, run: i64) -> Result<Vec<PveRewardRow>, Error> {
        pve_rewards_conn(&self.conn(), account, run)
    }

    /// Change a player's chests of one run and his profile in ONE transaction (the end screen: free chests at the run end,
    /// `5F 05` unlock, `5F 06` reroll). The stored rows must still be `before`, else nothing is written and the result is
    /// `Rejected(Stale)` (the run end stored twice, or a second press racing the first). Then `f` changes the profile
    /// (its Err writes nothing), and the rows become `after`.
    pub fn pve_change_chests<T, E>(
        &self,
        account: AccountId,
        run: i64,
        before: &[PveRewardRow],
        after: &[PveRewardRow],
        f: impl FnOnce(&mut Profile) -> Result<T, E>,
    ) -> Result<T, UpdateError<ChestRefusal<E>>> {
        self.update_profile_in(account, |conn, p| {
            if pve_rewards_conn(conn, account, run)? != before {
                return Err(UpdateError::Rejected(ChestRefusal::Stale));
            }
            let out = f(p).map_err(|e| UpdateError::Rejected(ChestRefusal::Rejected(e)))?;
            conn.execute("DELETE FROM pve_rewards WHERE account_id = ?1 AND match_id = ?2", params![account, run])
                .map_err(Error::from)?;
            for r in after {
                insert_reward(conn, account, run, r)?;
            }
            Ok(out)
        })
    }

    /// Replace the state of one chest (opened, rolled content, reroll count, paid). False when the chest does not exist.
    pub fn pve_update_reward(&self, account: AccountId, run: i64, r: &PveRewardRow) -> Result<bool, Error> {
        let n = self.conn().execute(
            "UPDATE pve_rewards SET opened = ?4, reward_dict = ?5, reward_amount = ?6, rerolls = ?7, paid = ?8 WHERE account_id = ?1 AND match_id = ?2 AND idx = ?3",
            params![account, run, r.idx, r.opened as i64, r.reward_dict, r.reward_amount, r.rerolls, r.paid as i64],
        )?;
        Ok(n > 0)
    }

    /// The key ([`pve_run_key`]) of the latest stored run `account` took part in (the run the end screen shows), None
    /// without a run. "Latest" = the highest result row id, i.e. the order in which the runs were stored.
    pub fn pve_latest_run(&self, account: AccountId) -> Result<Option<i64>, Error> {
        Ok(self
            .conn()
            .query_row(
                "SELECT r.match_id FROM pve_results r JOIN pve_result_players p ON p.result_id = r.id WHERE p.account_id = ?1 ORDER BY r.id DESC LIMIT 1",
                [account],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Count one more win of `account` on `day` (days since the unix epoch) and return the wins of that day so far.
    pub fn pve_note_win(&self, account: AccountId, day: i64) -> Result<u32, Error> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO pve_daily_wins (account_id, day, wins) VALUES (?1, ?2, 1) ON CONFLICT (account_id, day) DO UPDATE SET wins = wins + 1",
            params![account, day],
        )?;
        Ok(conn.query_row("SELECT wins FROM pve_daily_wins WHERE account_id = ?1 AND day = ?2", params![account, day], |r| r.get(0))?)
    }

    /// Wins of `account` on `day`.
    pub fn pve_wins_on(&self, account: AccountId, day: i64) -> Result<u32, Error> {
        Ok(self
            .conn()
            .query_row("SELECT wins FROM pve_daily_wins WHERE account_id = ?1 AND day = ?2", params![account, day], |r| r.get(0))
            .optional()?
            .unwrap_or(0))
    }
}

/// Why [`Store::pve_change_chests`] wrote nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum ChestRefusal<E> {
    /// The stored rows were not the expected ones.
    Stale,
    /// The profile change refused.
    Rejected(E),
}

fn insert_reward(conn: &Connection, account: AccountId, run: i64, r: &PveRewardRow) -> Result<(), Error> {
    conn.execute(
        "INSERT OR IGNORE INTO pve_rewards (account_id, match_id, idx, chest_dict, tier, difficulty, opened, reward_dict, reward_amount, rerolls, paid)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![account, run, r.idx, r.chest_dict, r.tier, r.difficulty, r.opened as i64, r.reward_dict, r.reward_amount, r.rerolls, r.paid as i64],
    )?;
    Ok(())
}

fn pve_rewards_conn(conn: &Connection, account: AccountId, run: i64) -> Result<Vec<PveRewardRow>, Error> {
    let mut st = conn.prepare(
        "SELECT idx, chest_dict, tier, difficulty, opened, reward_dict, reward_amount, rerolls, paid FROM pve_rewards
         WHERE account_id = ?1 AND match_id = ?2 ORDER BY idx",
    )?;
    let rows = st.query_map(params![account, run], |r| {
        Ok(PveRewardRow {
            idx: r.get(0)?,
            chest_dict: r.get(1)?,
            tier: r.get(2)?,
            difficulty: r.get(3)?,
            opened: r.get::<_, i64>(4)? != 0,
            reward_dict: r.get(5)?,
            reward_amount: r.get(6)?,
            rerolls: r.get(7)?,
            paid: r.get::<_, i64>(8)? != 0,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

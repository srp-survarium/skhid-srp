//! Friends and ignore lists: 4D kind 0 asks, the other side sees an invite (B0 status 4)
//! and accepts; 4E 01 removes; 4E 02/03 ignore / un-ignore; B0 05/06 send the lists.

use rusqlite::{OptionalExtension, TransactionBehavior, params};

use crate::{Account, AccountId, Error, Store, now};

/// How an account relates to the other one in its friend list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendState {
    Friend,
    /// this account asked, the other has not answered
    RequestSent,
    /// the other asked, this account has not answered (shown as an invite)
    RequestReceived,
}

impl FriendState {
    fn from_db(v: i64) -> FriendState {
        match v {
            1 => FriendState::Friend,
            2 => FriendState::RequestSent,
            _ => FriendState::RequestReceived,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendEntry {
    pub id: AccountId,
    pub name: String,
    pub state: FriendState,
    /// unix seconds since the state began
    pub since: i64,
}

impl Store {
    /// `from` asks `to` to be friends. When `to` already asked `from`, they become friends at once.
    /// Returns the new state of `from` towards `to` (`Friend` or `RequestSent`).
    pub fn friend_request(&self, from: AccountId, to: AccountId) -> Result<FriendState, Error> {
        if from == to {
            return Err(Error::SelfTarget);
        }
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for id in [from, to] {
            let ok: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)", [id], |r| r.get(0))?;
            if !ok {
                return Err(Error::NoSuchAccount);
            }
        }
        let cur: Option<i64> =
            tx.query_row("SELECT state FROM friends WHERE account_id = ?1 AND other_id = ?2", [from, to], |r| r.get(0)).optional()?;
        let out = match cur {
            Some(1) => return Err(Error::AlreadyFriends),
            Some(2) => return Err(Error::AlreadyRequested),
            Some(_) => {
                // the other side asked first: accept
                set_pair(&tx, from, to, 1, 1)?;
                FriendState::Friend
            }
            None => {
                set_pair(&tx, from, to, 2, 3)?;
                FriendState::RequestSent
            }
        };
        tx.commit()?;
        Ok(out)
    }

    /// `me` accepts the request `requester` sent.
    pub fn friend_accept(&self, me: AccountId, requester: AccountId) -> Result<(), Error> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let cur: Option<i64> =
            tx.query_row("SELECT state FROM friends WHERE account_id = ?1 AND other_id = ?2", [me, requester], |r| r.get(0)).optional()?;
        if cur != Some(3) {
            return Err(Error::NoRequest);
        }
        set_pair(&tx, me, requester, 1, 1)?;
        tx.commit()?;
        Ok(())
    }

    /// Remove a friendship or a pending request in either direction (decline, cancel, unfriend).
    /// Returns whether anything was there.
    pub fn friend_remove(&self, me: AccountId, other: AccountId) -> Result<bool, Error> {
        let n = self.conn().execute(
            "DELETE FROM friends WHERE (account_id = ?1 AND other_id = ?2) OR (account_id = ?2 AND other_id = ?1)",
            [me, other],
        )?;
        Ok(n > 0)
    }

    /// Friends and pending requests of an account, by name.
    pub fn friends(&self, me: AccountId) -> Result<Vec<FriendEntry>, Error> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT f.other_id, COALESCE(a.nick, 'Player' || a.id), f.state, f.since FROM friends f JOIN accounts a ON a.id = f.other_id
             WHERE f.account_id = ?1 ORDER BY COALESCE(a.nick, 'Player' || a.id)",
        )?;
        let rows = st.query_map([me], |r| {
            Ok(FriendEntry { id: r.get(0)?, name: r.get(1)?, state: FriendState::from_db(r.get(2)?), since: r.get(3)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Are the two accounts friends (accepted)?
    pub fn are_friends(&self, a: AccountId, b: AccountId) -> Result<bool, Error> {
        Ok(self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM friends WHERE account_id = ?1 AND other_id = ?2 AND state = 1)",
            [a, b],
            |r| r.get(0),
        )?)
    }

    /// `me` ignores `other`. Idempotent.
    pub fn ignore_add(&self, me: AccountId, other: AccountId) -> Result<(), Error> {
        if me == other {
            return Err(Error::SelfTarget);
        }
        let conn = self.conn();
        let ok: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM accounts WHERE id = ?1)", [other], |r| r.get(0))?;
        if !ok {
            return Err(Error::NoSuchAccount);
        }
        conn.execute("INSERT OR IGNORE INTO ignores (account_id, ignored_id) VALUES (?1, ?2)", [me, other])?;
        Ok(())
    }

    /// Stop ignoring; returns whether `other` was ignored.
    pub fn ignore_remove(&self, me: AccountId, other: AccountId) -> Result<bool, Error> {
        Ok(self.conn().execute("DELETE FROM ignores WHERE account_id = ?1 AND ignored_id = ?2", [me, other])? > 0)
    }

    /// Accounts `me` ignores, by name.
    pub fn ignored(&self, me: AccountId) -> Result<Vec<Account>, Error> {
        let conn = self.conn();
        let mut st = conn.prepare(
            "SELECT a.id, COALESCE(a.nick, 'Player' || a.id), a.created_at FROM ignores i JOIN accounts a ON a.id = i.ignored_id
             WHERE i.account_id = ?1 ORDER BY COALESCE(a.nick, 'Player' || a.id)",
        )?;
        let rows = st.query_map([me], |r| Ok(Account { id: r.get(0)?, name: r.get(1)?, created_at: r.get(2)? }))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Does `me` ignore `other` (the chat relay drops `other`'s lines for `me`)?
    pub fn is_ignored(&self, me: AccountId, other: AccountId) -> Result<bool, Error> {
        Ok(self.conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM ignores WHERE account_id = ?1 AND ignored_id = ?2)",
            [me, other],
            |r| r.get(0),
        )?)
    }
}

/// Write both sides of a relation.
fn set_pair(tx: &rusqlite::Transaction<'_>, a: AccountId, b: AccountId, state_a: i64, state_b: i64) -> Result<(), Error> {
    let t = now();
    for (x, y, s) in [(a, b, state_a), (b, a, state_b)] {
        tx.execute(
            "INSERT INTO friends (account_id, other_id, state, since) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(account_id, other_id) DO UPDATE SET state = ?3, since = ?4",
            params![x, y, s, t],
        )?;
    }
    Ok(())
}

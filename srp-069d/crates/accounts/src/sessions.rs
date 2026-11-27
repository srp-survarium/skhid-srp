//! Login sessions in the database (`schema::LOGIN_SESSIONS`): the `u32` the login server's `0B` gives the client and the
//! lobby's `2E` redeems. The lobby's `SessionTable` keeps them here so that a server restart does not lose them: the 0.69d
//! client re-sends the same session when it reconnects to the lobby by itself.

use rusqlite::{OptionalExtension, params};

use crate::{AccountId, Error, Store};

impl Store {
    /// Store a new session of `account`, used at unix second `at`. False when the id is taken (the caller draws another).
    pub fn insert_session(&self, session: u32, account: AccountId, at: i64) -> Result<bool, Error> {
        let n = self.conn().execute(
            "INSERT OR IGNORE INTO login_sessions (session, account_id, last_used) VALUES (?1, ?2, ?3)",
            params![i64::from(session), account, at],
        )?;
        Ok(n == 1)
    }

    /// The account and the last use (unix seconds) of a stored session.
    pub fn session(&self, session: u32) -> Result<Option<(AccountId, i64)>, Error> {
        Ok(self
            .conn()
            .query_row("SELECT account_id, last_used FROM login_sessions WHERE session = ?1", [i64::from(session)], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?)
    }

    /// Mark sessions used at unix second `at` (one transaction; a later stored use is kept; unknown ids are skipped).
    pub fn touch_sessions(&self, sessions: &[u32], at: i64) -> Result<(), Error> {
        if sessions.is_empty() {
            return Ok(());
        }
        let mut c = self.conn();
        let tx = c.transaction()?;
        {
            let mut st = tx.prepare_cached("UPDATE login_sessions SET last_used = MAX(last_used, ?2) WHERE session = ?1")?;
            for s in sessions {
                st.execute(params![i64::from(*s), at])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Delete the sessions last used before unix second `before`; returns how many went.
    pub fn purge_sessions(&self, before: i64) -> Result<usize, Error> {
        Ok(self.conn().execute("DELETE FROM login_sessions WHERE last_used < ?1", [before])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_touch_purge() {
        let s = Store::open_in_memory().unwrap();
        s.set_hash_cost(crate::HashCost::FAST_FOR_TESTS);
        let a = s.create_account("sess", "pw").unwrap();
        assert!(s.insert_session(0xDEAD_BEEF, a, 100).unwrap());
        // the id is taken
        assert!(!s.insert_session(0xDEAD_BEEF, a, 200).unwrap());
        assert_eq!(s.session(0xDEAD_BEEF).unwrap(), Some((a, 100)));
        // u32 above i32::MAX round-trips
        assert!(s.insert_session(u32::MAX, a, 100).unwrap());
        assert_eq!(s.session(u32::MAX).unwrap(), Some((a, 100)));
        s.touch_sessions(&[0xDEAD_BEEF, 7], 150).unwrap();
        // an older touch does not move it back
        s.touch_sessions(&[0xDEAD_BEEF], 120).unwrap();
        assert_eq!(s.session(0xDEAD_BEEF).unwrap(), Some((a, 150)));
        assert_eq!(s.purge_sessions(150).unwrap(), 1);
        assert_eq!(s.session(u32::MAX).unwrap(), None);
        assert_eq!(s.session(0xDEAD_BEEF).unwrap(), Some((a, 150)));
    }
}

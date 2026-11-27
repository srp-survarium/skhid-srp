//! Handover from the login server to the lobby.
//!
//! The real client signs in over TCP (login server, `0B` success carries `u32 session`) and then opens the UDP lobby
//! connection with `2E sign-in`: `u32 session, u8 ff, u8 ff` (the session the login server gave it, PROVEN with the
//! real client). The login server calls [`SessionTable::grant`] when it accepts a
//! password; the lobby calls [`SessionTable::redeem`] on 2E. A session is not used up by a 2E: the client keeps it and
//! sends the same 2E again on every lobby reconnect (PROVEN: `network_client::tick` 0x2001F2B5C retries the lobby 5 s
//! after a drop, also after a server restart).
//!
//! A table made with [`SessionTable::persistent`] also keeps the sessions in the account database, so a server restart
//! does not lose them (the client's reconnect after a restart is accepted without a new sign-in). A session expires a
//! time to live after its last use: the grant, a 2E, or the lobby seeing the account
//! connected ([`SessionTable::touch`], every minute while connected and when the connection ends).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use accounts::{AccountId, Store};

/// The default time to live after the last use.
pub const SESSION_TTL: Duration = Duration::from_secs(3600);

pub struct SessionTable {
    /// session -> (account, last use in unix seconds). Every live session of this process is here; the store (if any)
    /// holds them across restarts.
    inner: Mutex<HashMap<u32, (AccountId, i64)>>,
    store: Option<Arc<Store>>,
    /// Accounts with a live lobby connection (the lobby marks them; the login server refuses a second sign-in).
    online: Mutex<HashSet<AccountId>>,
    ttl: i64,
}

impl Default for SessionTable {
    fn default() -> Self {
        SessionTable::with_ttl(SESSION_TTL)
    }
}

/// Unix seconds now.
pub fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

impl SessionTable {
    /// Sessions in memory only (lost on restart).
    pub fn with_ttl(ttl: Duration) -> Self {
        SessionTable { inner: Mutex::new(HashMap::new()), store: None, online: Mutex::new(HashSet::new()), ttl: ttl.as_secs() as i64 }
    }

    /// Sessions also kept in the account database: a restarted server still knows them.
    pub fn persistent(store: Arc<Store>) -> Self {
        SessionTable::persistent_with_ttl(store, SESSION_TTL)
    }

    pub fn persistent_with_ttl(store: Arc<Store>, ttl: Duration) -> Self {
        SessionTable { store: Some(store), ..SessionTable::with_ttl(ttl) }
    }

    fn live(&self, last_used: i64, now: i64) -> bool {
        now - last_used < self.ttl
    }

    /// New random non-zero session for an account (older sessions of the account stay valid until they expire).
    pub fn grant(&self, account: AccountId) -> u32 {
        self.grant_at(account, unix_now())
    }

    /// [`SessionTable::grant`] at unix second `now`.
    pub fn grant_at(&self, account: AccountId, now: i64) -> u32 {
        let mut m = self.inner.lock().unwrap();
        m.retain(|_, (_, t)| now - *t < self.ttl);
        if let Some(st) = &self.store {
            if let Err(e) = st.purge_sessions(now - self.ttl) {
                log::error!("login sessions: purge failed: {e}");
            }
        }
        loop {
            let s: u32 = rand::random();
            if s == 0 || m.contains_key(&s) {
                continue;
            }
            if let Some(st) = &self.store {
                match st.insert_session(s, account, now) {
                    Ok(true) => {}
                    // taken by a session this process does not hold in memory (an earlier run's): draw again
                    Ok(false) => continue,
                    // the sign-in still works until a restart
                    Err(e) => log::error!("login sessions: session of account {account} not stored: {e}"),
                }
            }
            m.insert(s, (account, now));
            return s;
        }
    }

    /// The account of a live session; the 2E counts as a use.
    pub fn redeem(&self, session: u32) -> Option<AccountId> {
        self.redeem_at(session, unix_now())
    }

    /// [`SessionTable::redeem`] at unix second `now`.
    pub fn redeem_at(&self, session: u32, now: i64) -> Option<AccountId> {
        if session == 0 {
            return None;
        }
        let mut m = self.inner.lock().unwrap();
        let known = match m.get(&session) {
            Some(&(a, t)) => Some((a, t)),
            None => match &self.store {
                // granted by an earlier run of the server (the restart case)
                Some(st) => st.session(session).unwrap_or_else(|e| {
                    log::error!("login sessions: lookup failed: {e}");
                    None
                }),
                None => None,
            },
        };
        let (account, last) = known?;
        if !self.live(last, now) {
            return None;
        }
        m.insert(session, (account, now.max(last)));
        drop(m);
        self.store_touch(&[session], now);
        Some(account)
    }

    /// The sessions are in use at unix second `now` (accounts still connected, or a connection that just ended).
    pub fn touch(&self, sessions: &[u32], now: i64) {
        {
            let mut m = self.inner.lock().unwrap();
            for s in sessions {
                if let Some((_, t)) = m.get_mut(s) {
                    *t = (*t).max(now);
                }
            }
        }
        self.store_touch(sessions, now);
    }

    fn store_touch(&self, sessions: &[u32], now: i64) {
        if let Some(st) = &self.store {
            if let Err(e) = st.touch_sessions(sessions, now) {
                log::error!("login sessions: touch failed: {e}");
            }
        }
    }

    /// The lobby saw the account sign in (2E).
    pub fn mark_online(&self, account: AccountId) {
        self.online.lock().unwrap().insert(account);
    }

    /// The lobby dropped the account's connection.
    pub fn mark_offline(&self, account: AccountId) {
        self.online.lock().unwrap().remove(&account);
    }

    pub fn is_online(&self, account: AccountId) -> bool {
        self.online.lock().unwrap().contains(&account)
    }

    /// Sessions this process holds in memory.
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_redeem_expire() {
        let t = SessionTable::with_ttl(Duration::from_secs(10));
        let now = 1_000;
        let a = t.grant_at(7, now);
        let b = t.grant_at(8, now);
        assert_ne!(a, b);
        assert_ne!(a, 0);
        assert_eq!(t.redeem_at(a, now + 9), Some(7));
        // not used up by the first 2E
        assert_eq!(t.redeem_at(a, now + 9), Some(7));
        assert_eq!(t.redeem_at(b, now + 11), None);
        assert_eq!(t.redeem_at(0xDEAD_BEEF, now), None);
        assert_eq!(t.redeem_at(0, now), None);
        // the 2E at +9 was a use: still live at +18, gone at +19
        assert_eq!(t.redeem_at(a, now + 18), Some(7));
        assert_eq!(t.redeem_at(a, now + 28), None);
        // expired sessions are dropped on the next grant
        t.grant_at(9, now + 40);
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn touch_keeps_a_session_alive() {
        let t = SessionTable::with_ttl(Duration::from_secs(10));
        let a = t.grant_at(7, 0);
        t.touch(&[a], 8);
        assert_eq!(t.redeem_at(a, 17), Some(7));
        assert_eq!(t.redeem_at(a, 27), None);
    }

    fn store() -> Arc<Store> {
        let s = Store::open_in_memory().unwrap();
        s.set_hash_cost(accounts::HashCost::FAST_FOR_TESTS);
        Arc::new(s)
    }

    #[test]
    fn persistent_sessions_survive_a_new_table() {
        let st = store();
        let id = st.create_account("restart", "pw").unwrap();
        let old = SessionTable::persistent_with_ttl(st.clone(), Duration::from_secs(10));
        let s = old.grant_at(id, 100);
        old.touch(&[s], 105);
        drop(old);
        // a restarted server: a new table over the same store
        let new = SessionTable::persistent_with_ttl(st.clone(), Duration::from_secs(10));
        assert!(new.is_empty());
        assert_eq!(new.redeem_at(s, 114), Some(id));
        assert_eq!(new.redeem_at(s ^ 1, 114), None);
        // the time to live still counts (from the 2E at 114)
        let again = SessionTable::persistent_with_ttl(st.clone(), Duration::from_secs(10));
        assert_eq!(again.redeem_at(s, 124), None);
        // a memory-only table forgets it
        assert_eq!(SessionTable::default().redeem_at(s, 114), None);
    }
}

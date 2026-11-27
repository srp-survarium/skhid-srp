//! The SteamID64 a Steam-mode client claimed (`schema::STEAM_IDS`), stored per account at each successful Steam-mode
//! sign-in. Information only: the client sends it unverified (anyone can send another player's public SteamID64), so
//! nothing reads it to grant access.

use rusqlite::{OptionalExtension, params};

use crate::{AccountId, Error, Store};

impl Store {
    /// Record the SteamID64 claimed at a Steam-mode sign-in of `account` at unix second `at` (replaces the last one).
    pub fn set_steam_id(&self, account: AccountId, steam_id: u64, at: i64) -> Result<(), Error> {
        self.conn().execute(
            "INSERT INTO steam_ids (account_id, steam_id, last_seen) VALUES (?1, ?2, ?3)
             ON CONFLICT(account_id) DO UPDATE SET steam_id = excluded.steam_id, last_seen = excluded.last_seen",
            params![account, steam_id as i64, at],
        )?;
        Ok(())
    }

    /// The last claimed SteamID64 of `account` and when (unix seconds); None if it never signed in in Steam mode.
    pub fn steam_id(&self, account: AccountId) -> Result<Option<(u64, i64)>, Error> {
        Ok(self
            .conn()
            .query_row("SELECT steam_id, last_seen FROM steam_ids WHERE account_id = ?1", [account], |r| {
                Ok((r.get::<_, i64>(0)? as u64, r.get(1)?))
            })
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_replace_and_full_u64_range() {
        let s = Store::open_in_memory().unwrap();
        s.set_hash_cost(crate::HashCost::FAST_FOR_TESTS);
        let a = s.create_account("steamer", "pw").unwrap();
        assert_eq!(s.steam_id(a).unwrap(), None);
        s.set_steam_id(a, 76561197960265728, 100).unwrap(); // the SteamID64 base: account number 0, no real account
        assert_eq!(s.steam_id(a).unwrap(), Some((76561197960265728, 100)));
        // the next sign-in replaces it; the top bit survives the INTEGER column
        s.set_steam_id(a, u64::MAX - 1, 200).unwrap();
        assert_eq!(s.steam_id(a).unwrap(), Some((u64::MAX - 1, 200)));
    }

    #[test]
    fn an_existing_database_file_without_the_table_opens_and_gets_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.sqlite");
        let a = {
            let s = Store::open(&path).unwrap();
            s.set_hash_cost(crate::HashCost::FAST_FOR_TESTS);
            let a = s.create_account("veteran", "pw").unwrap();
            // a file made before the table existed
            s.conn().execute_batch("DROP TABLE steam_ids;").unwrap();
            a
        };
        let s = Store::open(&path).unwrap();
        assert_eq!(s.steam_id(a).unwrap(), None);
        s.set_steam_id(a, 7, 1).unwrap();
        assert_eq!(s.steam_id(a).unwrap(), Some((7, 1)));
        assert_eq!(s.verify_login("veteran", "pw").unwrap(), a);
    }
}

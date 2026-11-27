# accounts (0.69d)

SQLite store (rusqlite, bundled) of accounts and profiles. One database file per server.

- **Accounts**: `create_account(name, password)`, `verify_login`, `change_password`, `find_account`, `search_accounts`
  (4E 04). Names 1..=32 characters, unique ignoring case. Passwords are argon2id hashes (`HashCost::DEFAULT` = the argon2
  crate default); an unknown name costs the same time as a wrong password. `public_id(account)` is the u64 the
  friend/chat messages carry (`0x5000_0000_0000_0000 | id`, LIKELY convention).
- **Profile**: items with counts (list order kept), profile slots + slot
  rounds, silver / gold / spare parts, xp, workshop modifiers per item, installed modules per weapon slot, module
  bindings, active faction, points per faction, profile version (87 rebuild counter), uid counter.
  `Profile::starter()` = the fresh profile.
- **Atomic changes**: `update_profile(id, |p| ...)` loads, runs the closure and saves in one IMMEDIATE transaction;
  an `Err` from the closure writes nothing. `Profile::spend` / `credit` / `add_item` / `remove_item` (clears slots,
  modifiers, attachments, bindings).
- **Social**: friends (`friend_request` -> `friend_accept`, mutual request = friends, `friend_remove` also declines or
  cancels, `friends` lists `Friend` / `RequestSent` / `RequestReceived`), ignore list (`ignore_add/remove`, `is_ignored`).
- **Importer** for the account state file (`state_069d.json`) of an earlier reference implementation:
  `import_state(id, json)` / `import_state_file` / `create_account_from_state`; missing keys take the
  defaults, a short `money` list is padded, references to missing items are dropped (reported).

Schema: `src/schema.rs` (`PRAGMA user_version` = `schema::VERSION`; an older database is migrated step by step on
open, a newer one is refused). The crate does not depend on `game-data`:
dict ids are plain `u16`, rules (is this item a stack, may it go into this slot) belong to the caller.

Tests: `cargo test -p accounts -- --test-threads=2` (temporary databases; tests lower the argon2 cost with
`set_hash_cost(HashCost::FAST_FOR_TESTS)`, never do that in the server). `tests/store.rs` holds a sample state file.

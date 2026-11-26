//! Migration 13's table, `g26_mode_introductions`: the per-mode allowances
//! of a HUD request for a mode's introduction (the first three Blackout
//! matches), which no shipped HUD sends any more and the lobby no longer
//! answers. Nothing reads or writes the table now; the step stays, so the
//! schema's numbering does, and a database that has the table keeps it.

pub(super) const MIGRATION: &str = "
    CREATE TABLE g26_mode_introductions (
        account_id INTEGER NOT NULL REFERENCES g26_accounts(account_id) ON DELETE CASCADE,
        mode TEXT NOT NULL,
        shown INTEGER NOT NULL CHECK(shown BETWEEN 0 AND 3),
        PRIMARY KEY (account_id, mode)
    );";

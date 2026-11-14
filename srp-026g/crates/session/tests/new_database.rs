//! srp's start on a data folder with no database, as `srp`'s `main` makes
//! its stores. Its own binary: it sets `SRP_DATA_DIR` for the process.

use session::SessionStore;
use session::v026g::GameAccounts;

#[test]
fn a_new_database_starts_and_starts_again() {
    let dir = std::env::temp_dir().join(format!("srp-new-database-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // Safety: the only test of this binary; nothing else reads the environment yet.
    unsafe { std::env::set_var("SRP_DATA_DIR", &dir) };
    for _ in 0..2 {
        let store = SessionStore::from_env(session::DEFAULT_GATHER_WINDOW).unwrap();
        // Makes the 0.26g tables and, without the pay-back's mark, copies
        // the file (`VACUUM INTO`): with the nickname index in it.
        let accounts = GameAccounts::from_env().unwrap();
        store.restore_026g_names(&accounts);
    }
    assert!(dir.join("srp.db").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

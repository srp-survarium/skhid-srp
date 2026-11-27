//! Game tables for tests.
//!
//! The tables are not in the repository: they are built from a game install (`srp extract --out <dir>` or
//! `srp setup`). Tests find them in `SRP_TEST_DATA`, else in the workspace's `data/` folder (ignored by git). A test
//! that needs them starts with [`require_tables!`](crate::require_tables): without tables it returns at once and
//! prints a note, so a checkout without game files (CI) still builds and runs every other test. Set
//! `SRP_REQUIRE_TEST_DATA=1` to make a missing table folder fail those tests instead.

use std::path::PathBuf;
use std::sync::OnceLock;

use crate::GameData;

/// The folder the tests read: `SRP_TEST_DATA`, else the workspace's `data/` when it holds every table.
pub fn dir() -> Option<PathBuf> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let d = match std::env::var_os("SRP_TEST_DATA") {
            Some(d) if !d.is_empty() => PathBuf::from(d),
            _ => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        };
        crate::missing_files(&d).is_empty().then_some(d)
    })
    .clone()
}

/// Are the tables there? (Panics when they are not and `SRP_REQUIRE_TEST_DATA` is set.)
pub fn available() -> bool {
    let ok = dir().is_some();
    if !ok && std::env::var_os("SRP_REQUIRE_TEST_DATA").is_some_and(|v| !v.is_empty() && v != "0") {
        panic!("{}", MISSING);
    }
    ok
}

const MISSING: &str = "no game tables for the tests: build them with `srp extract --game <game folder> --out data` \
                       (or set SRP_TEST_DATA to a folder that holds them)";

/// Note a skipped test (stderr, shown with `--nocapture`).
pub fn skip_note(test: &str) {
    eprintln!("skipped {test}: {MISSING}");
}

/// A fresh copy of every table (a test may change it). Call after [`require_tables!`](crate::require_tables).
pub fn tables() -> GameData {
    let d = dir().expect(MISSING);
    GameData::from_dir(&d).unwrap_or_else(|e| panic!("test tables in {}: {e}", d.display()))
}

/// One shared copy of every table.
pub fn shared() -> &'static GameData {
    static G: OnceLock<GameData> = OnceLock::new();
    G.get_or_init(tables)
}

/// One table file's text.
pub fn read(name: &str) -> String {
    let d = dir().expect(MISSING);
    std::fs::read_to_string(d.join(name)).unwrap_or_else(|e| panic!("{}: {e}", d.join(name).display()))
}

/// A file of reference values next to the tables (in the `SRP_TEST_DATA` folder), or `None` when it is not there.
/// Reference values computed from a game install by an earlier implementation are not in the repository (they hold
/// game data); the tests that compare with them skip without the file.
pub fn reference(name: &str) -> Option<String> {
    std::fs::read_to_string(dir()?.join(name)).ok()
}

/// First line of a test that needs the game tables: returns from the test (it passes, with a note) when there are none.
#[macro_export]
macro_rules! require_tables {
    () => {
        if !$crate::testing::available() {
            $crate::testing::skip_note(module_path!());
            return;
        }
    };
}

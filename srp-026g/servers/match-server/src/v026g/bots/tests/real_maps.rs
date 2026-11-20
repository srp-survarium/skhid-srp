//! The real maps' tests (`SRP_MAPDATA_DIR`) run on the server's game data (`SRP_GAME_DATA`, `server-game-data.db`), as the
//! live server does, or refuse at once. A grid's body (the rim jumps' fastest walk cap) and the bots' kits come from the loaded
//! game data, and the built-in test archive builds other grids than the live server's, so a test on the real maps never falls
//! back to it.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use survarium::v026g::game_data::{self, GAME_DATA_ENV, GameData};

/// The server's game data the file `SRP_GAME_DATA` names, read and checked as srp reads it (`game_data::init`) once a process.
/// Panics when `SRP_GAME_DATA` is unset or empty, when the file fails, and when the process already has other data (a table
/// read first gave it the test archive).
pub(crate) fn game_data() -> &'static GameData {
    static READ: OnceLock<Result<(), String>> = OnceLock::new();
    let path = std::env::var_os(GAME_DATA_ENV).filter(|value| !value.is_empty()).unwrap_or_else(|| {
        panic!(
            "{GAME_DATA_ENV} is unset: a test on the real maps (SRP_MAPDATA_DIR) runs on the server's game data \
             (server-game-data.db), as the live server does: the grids' body (the rim jumps' fastest walk cap) and the bots' \
             kits come from it, and the built-in test archive builds other grids"
        )
    });
    let read = READ.get_or_init(|| match game_data::init() {
        Ok(_) => Ok(()),
        // The process had game data already: right only when it is this file (another test of this process read it first).
        Err(cause) if cause.0.starts_with("a table was used before") => {
            if game_data::shared().origin == Path::new(&path).display().to_string() {
                Ok(())
            } else {
                Err(format!(
                    "{GAME_DATA_ENV} {path:?}: {cause}: the process has {}: a table was read before the game data: run the \
                     real-map tests apart from the unit tests",
                    game_data::shared().origin
                ))
            }
        }
        Err(cause) => Err(format!("{GAME_DATA_ENV} {path:?}: {cause}")),
    });
    if let Err(why) = read {
        panic!("{why}");
    }
    game_data::shared()
}

/// The folder of the real maps' dumps (`SRP_MAPDATA_DIR`; unset or empty: none, the test skips), after [`game_data`].
pub(crate) fn dir() -> Option<PathBuf> {
    let dir = std::env::var_os("SRP_MAPDATA_DIR").filter(|value| !value.is_empty())?;
    game_data();
    Some(PathBuf::from(dir))
}

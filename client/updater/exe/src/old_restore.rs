//! What the updaters of before the patches left beside the updater, deleted
//! (`client/updater/README.md`, "What earlier updaters left").
//!
//! Updaters 0.1.x (the players' ZIPs of 2026-10-14 to 2026-10-16) wrote three
//! Flash movies of `game/resources.db` in place, and kept a record, a journal,
//! copies of the stock bytes and fetched blobs beside the updater; the updaters
//! from 0.2.0 until this change put the stock bytes back once and then
//! deleted all of it.
//!
//! This updater changes no byte of `resources.db`: it only deletes those files
//! where they are still there. The regions they name are the data of
//! `main_menu`, `lobby_menu` and `login_menu`, which `resources.db.patch_12`
//! (listed, so required before PLAY) carries; the game takes a file from the
//! last archive mounted that has it, so whatever bytes the db holds there,
//! torn by a write cut short too, are never read.

use alloc::format;

use crate::install::Layout;
use crate::log::log;
use crate::win;

/// Files beside the updater: the plan of the old ZIPs, the status of a plan
/// that grew the db, the record of the regions written, and the journal of
/// the one being written.
const FILES: [&str; 4] = [
    "updater_skhid.resources",
    "updater_skhid.resources.status",
    "updater_skhid.resources.state",
    "updater_skhid.resources.journal",
];
/// Folders beside the updater: the copies of the stock bytes, and the blobs
/// fetched (with a `.part` of one cut short).
const FOLDERS: [&str; 2] = ["updater_skhid.backup", "updater_skhid.cache"];

/// Deletes what is left of those files. Best effort and quiet when there is
/// nothing: a file Windows does not delete now (open) is tried at the next
/// start, and nothing here fails the update.
pub fn sweep(layout: &Layout) {
    let beside = |name: &str| format!("{}\\{name}", layout.exe_dir);
    for name in FILES {
        remove(&beside(name));
    }
    for folder in FOLDERS {
        let folder = beside(folder);
        if !win::is_folder(&folder) {
            continue;
        }
        for name in win::list(&folder, "*") {
            remove(&format!("{folder}\\{name}"));
        }
        // Only when it is empty.
        win::remove_dir(&folder);
        if !win::exists(&folder) {
            log!("removed {folder} (left by an earlier updater)");
        }
    }
}

fn remove(path: &str) {
    if !win::exists(path) {
        return;
    }
    match win::delete(path) {
        Ok(()) => log!("removed {path} (left by an earlier updater)"),
        Err(code) => log!(
            "{path} (left by an earlier updater) cannot be removed ({}): next time",
            win::error_text(code)
        ),
    }
}

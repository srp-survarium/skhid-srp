//! Patches 27 and 49: resources.db's patches mounted over it at the start,
//! and the ones an exe requires (client_patch.md sections 3.27 and 3.46).
//!
//! query_db's one query of resources.db looks for
//! `..\..\resources.db.patch_<n>` for every n from 1 to 255 first, and
//! writes down, in order, the n of each one that is there. A file that is
//! there and cannot be opened (any error but 2, not found, and 3, path not
//! found), or a required one that is not there, stops the game before
//! anything is queued, with its number and the error. Then resources.db is
//! queued as the game asked, and each patch written down after it, under
//! the same virtual path, descriptor and callback.
//!
//! The file system calls the callback for each mount in that order, on the
//! main thread. A failed patch stops the game with its number (resources.db's
//! failure is the game's own message). Each start-up mount is held here; all
//! but the last return. The last hands resources.db's mount to the game's
//! own callback (engine_world takes its reference and starts the next
//! step), and this one's is let go again as the game lets a mount go. Each
//! patch stays held until the process ends, as resources.db does.

#![forbid(unsafe_code)]

use crate::vostok::mount::{MAX_PATCHES, MountQuery, MountSlot, Mounted, NativePath, fatal, probe, required,
                           unreadable_format};
use crate::vostok::{Context, Flow, GameCell, MountPatchesOnMountedExit, NoExit};

/// A patch's file, from the mount's descriptor (`resources`) and its number.
const PATCH_FORMAT: &core::ffi::CStr = c"../../%s.db.patch_%u";
/// A patch that failed to mount: %u its number.
const FAILED: &core::ffi::CStr = c"Cannot mount resources.db.patch_%u. Please run the Skhid launcher again.";
/// CreateFileA's "not there" errors.
const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_PATH_NOT_FOUND: u32 = 3;

/// The start-up's callbacks still to come (0: no start-up mount pending).
static REMAINING: GameCell<u32> = GameCell::new(0);
/// The start-up's mounts held so far.
static SEEN: GameCell<u32> = GameCell::new(0);
/// The n of each patch found, in order.
static NUMBERS: GameCell<[u8; MAX_PATCHES as usize]> = GameCell::new([0; MAX_PATCHES as usize]);
/// The start-up's mounts, held: resources.db's first, then the patches'.
static MOUNTS: [MountSlot; MAX_PATCHES as usize + 1] = [const { MountSlot::new() }; MAX_PATCHES as usize + 1];

/// `mount.query`: query_db's query of resources.db, and every patch after.
pub fn on_query(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let query = MountQuery::at(ctx);
    let mut path = NativePath::default();
    let mut count = 0;
    for n in 1..=MAX_PATCHES {
        query.format(&mut path, PATCH_FORMAT, n);
        match probe(path.text()) {
            Ok(()) => {
                NUMBERS.update(thread, |mut numbers| {
                    numbers[count] = n as u8;
                    numbers
                });
                count += 1;
            }
            Err(ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) if !required(n) => {}
            Err(error) => fatal(unreadable_format(), n, error),
        }
    }
    SEEN.set(thread, 0);
    REMAINING.set(thread, count as u32 + 1);
    query.queue();
    let numbers = NUMBERS.get(thread);
    for &n in &numbers[..count] {
        query.format(&mut path, PATCH_FORMAT, u32::from(n));
        query.queue_archive(&path);
    }
    Flow::Skip
}

/// `mount.mounted`: a mount's callback.
pub fn on_mounted(ctx: &mut Context<'_>) -> Flow<MountPatchesOnMountedExit> {
    let thread = ctx.thread();
    let remaining = REMAINING.get(thread);
    if remaining == 0 {
        return Flow::Continue; // not a start-up mount: the game's
    }
    let mounted = Mounted::at(ctx);
    let seen = SEEN.get(thread);
    let Some(mount) = mounted.mount() else {
        if seen == 0 {
            return Flow::Continue; // resources.db: the game's own "Cannot mount resources"
        }
        fatal(FAILED, u32::from(NUMBERS.get(thread)[seen as usize - 1]), 0)
    };
    mount.hold();
    MOUNTS[seen as usize].keep(mount);
    SEEN.set(thread, seen + 1);
    REMAINING.set(thread, remaining - 1);
    if remaining == 1 {
        mounted.hand_to_game(&MOUNTS[0]);
        if let Some(resources) = MOUNTS[0].mount(thread) {
            resources.release();
        }
    }
    Flow::Exit(MountPatchesOnMountedExit::Done)
}

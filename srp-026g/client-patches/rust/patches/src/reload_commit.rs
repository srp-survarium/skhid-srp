//! Patch 205: a reload never loses ammunition because it was left (the owner,
//! 2026-11-26: "quite often you put the ammo magazine into the weapon, then do
//! a melee hit and the ammo is reset to 0"; docs/notes/reload-commit.md).
//!
//! - Left before the new magazine is in (a melee, a sprint, a weapon switch, a
//!   grenade, before its clip's `reload_commit` event): the gun keeps what it
//!   had, its old magazine; nothing went to or from the stack. As the game.
//! - Left after: the new magazine counts. At the event the reload loads the
//!   magazine as its end does (a top-up from the stack: what stays of the old
//!   one is in it, nothing lost), and its end, when it comes, loads again,
//!   which changes nothing.
//! - The event is the content's: each magazine reload clip's own seat sound
//!   (content_edit.py reload_commit), as 0.69d0 loads at its clips' `shoot`
//!   event. A clip without it loads at its end, as before.

#![forbid(unsafe_code)]

use crate::vostok::reload::{ChamberOnReload, MagazineReload, ReloadEvent, ReloadEventState, ReloadLoad, AnimationEvent};
use crate::vostok::{Context, Flow, NoExit};

/// An animation-end-aware state initialized (`reload.listen`).
pub fn on_initialize(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(reload) = MagazineReload::at_initialize(ctx) {
        reload.listen(crate::vostok::reload_commit_on_event_entry());
    }
    Flow::Continue
}

/// One read from a world (`reload.listen_again`): as the game subscribes it
/// to its clip's end again.
pub fn on_deserialize(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(reload) = MagazineReload::at_deserialize(ctx) {
        reload.listen(crate::vostok::reload_commit_on_event_entry());
    }
    Flow::Continue
}

/// A reload left or ended (`reload.stop_listening`).
pub fn on_finalize(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(reload) = MagazineReload::at_finalize(ctx) {
        reload.stop_listening();
    }
    Flow::Continue
}

/// The reload's load, at its end or its event (`reload.load_at_end`).
pub fn on_load_at_end(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(load) = ReloadLoad::at(ctx) {
        match load.chamber() {
            ChamberOnReload::Chambered => load.unchamber(),
            ChamberOnReload::No | ChamberOnReload::Empty => {}
        }
    }
    Flow::Continue
}

/// A `reload_commit` event of the reload's clip (`reload.commit_event`).
pub fn on_event(state: &ReloadEventState<'_>, event: &AnimationEvent<'_>) -> u32 {
    match state.event(event) {
        Some(ReloadEvent::Seated) => state.commit(),
        None => {}
    }
    0
}

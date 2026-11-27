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
//! - The clips stay the reload's own. A two-clip reload (314: tactical or
//!   empty; 311: one shell or two) picks its clips from the rounds in the gun
//!   each time the player's animation is selected again (an input, an aim), and
//!   the seat changes those rounds: an empty reload would go on as a tactical
//!   one from its start, its end waited for there. So once the reload has made
//!   its first pick, which the game keeps (m_index_of_animation_to_wait, +134h,
//!   in a world too), every pick after it is that one, as 0.69d0 keeps its own.

#![forbid(unsafe_code)]

use crate::vostok::reload::{AnimationEvent, ChamberOnReload, ClipPick, MagazineReload, ReloadClip, ReloadEvent, ReloadEventState,
                            ReloadLoad};
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

/// A pick the saved one decides: the weapon's after the reload's first (its
/// first's clips), 314's hands (this rebuild's weapon clips, which the game's
/// own pick, the magazine empty, always matched before the seat).
fn keep_saved(pick: Option<ClipPick<'_, '_>>) {
    if let Some(pick) = pick
        && let Some(clip) = pick.saved()
    {
        pick.choose(clip);
    }
}

/// State 314's weapon clips (`reload.pick_empty`).
pub fn on_pick_empty(ctx: &mut Context<'_>) -> Flow<NoExit> {
    keep_saved(ClipPick::magazine(ctx));
    Flow::Continue
}

/// State 311's weapon clips (`reload.pick_two_shells`).
pub fn on_pick_two_shells(ctx: &mut Context<'_>) -> Flow<NoExit> {
    keep_saved(ClipPick::shells(ctx));
    Flow::Continue
}

/// State 314's hands (`reload.pick_empty_hands`).
pub fn on_pick_empty_hands(ctx: &mut Context<'_>) -> Flow<NoExit> {
    keep_saved(ClipPick::magazine_hands(ctx));
    Flow::Continue
}

/// State 311's hands (`reload.pick_two_shells_hands`).
pub fn on_pick_two_shells_hands(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(pick) = ClipPick::shell_hands(ctx)
        && let Some(clip) = shell_hands(pick.saved(), pick.loaded())
    {
        pick.choose(clip);
    }
    Flow::Continue
}

/// The double barrels' hands for the weapon's clips `weapon` and the rounds
/// in the gun now; None keeps the game's own pick.
///
/// The game's (0x30F4F0): two shells' hands when the gun does not hold one
/// round and the stack is not of exactly one. At the reload's start that
/// matches the weapon's clips but in one case: an empty gun with one shell
/// left, the two shells' gun and the one shell's hands. A reload starts only
/// below capacity (0x2E3E01), so with two shells' clips the gun held none, and:
/// - none in it now: the seat has not loaded, the counts are the start's, the
///   game's pick is the first;
/// - one: the seat loaded the stack's last, the one shell's hands as at the
///   start (its stack of one); the game's would be the same;
/// - two: the seat loaded two from a stack of two or more, the two shells'
///   hands as at the start; the game's would be the one shell's after a stack
///   of three.
///
/// With the one shell's clips the gun held one: the one shell's hands, the
/// game's at the start, its two shells' after the seat.
pub fn shell_hands(weapon: Option<ReloadClip>, loaded: u8) -> Option<ReloadClip> {
    match (weapon?, loaded) {
        (ReloadClip::First, _) => Some(ReloadClip::First),
        (ReloadClip::Second, 0) => None,
        (ReloadClip::Second, 1) => Some(ReloadClip::First),
        (ReloadClip::Second, _) => Some(ReloadClip::Second),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_double_barrels_hands_follow_the_reload_start() {
        // no pick yet: the game's
        assert_eq!(shell_hands(None, 0), None);
        // one shell (one in the gun at the start): its hands, before and after the seat
        assert_eq!(shell_hands(Some(ReloadClip::First), 1), Some(ReloadClip::First));
        assert_eq!(shell_hands(Some(ReloadClip::First), 2), Some(ReloadClip::First));
        // two shells: before the seat the game's own (the stack's count is the start's)
        assert_eq!(shell_hands(Some(ReloadClip::Second), 0), None);
        // after it: a stack of one gave one shell, of two or more two
        assert_eq!(shell_hands(Some(ReloadClip::Second), 1), Some(ReloadClip::First));
        assert_eq!(shell_hands(Some(ReloadClip::Second), 2), Some(ReloadClip::Second));
    }
}

//! Patch 76: a shot into a corpse's head puts its player in the dark
//! (the owner, 2026-11-10): the screen black and every sound off, until the
//! player lives again.
//!
//! Each client draws its own ragdoll of a death, so where a corpse's head
//! lies differs between them. The shooter's client decides: its own
//! player's bullet meeting the head of the corpse it draws, half a second
//! or more after that ragdoll began ([`head_hit`], from patch 76's step).
//! It says so in its next input free of quick-use keys, as a command of the
//! input's marker (`weapon_pickup::Command::HeadShot`, the victim's place),
//! which the server relays as any input. Every client reads it where an
//! input is applied ([`on_filter`]); the one whose own player is that
//! place's, and dead, goes dark ([`after_step`]).
//!
//! The simulation reads nothing of the command (patch 78's filter takes the
//! marker and its quick bits out of every input before the game's handling,
//! as it does of any pattern it does not know), and nothing a match computes
//! is written: the dark is this module's own state, the hud movie's black
//! view (`root.set_dead_black`) and XAudio2's mastering voice.

#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicBool, Ordering};

use ::ragdoll::corpse::DeathKey;
use weapon_pickup::{CHORD, Command, Place, QUICK};

use crate::vostok::sound::{SoundTick, Volume};
use crate::vostok::world::Player;
use crate::vostok::{ActionMask, Context, Flow, GameCell, GameWorldUi, InputHandler, NoExit, Thread};

/// What the local player's own client has to tell: nothing, or that its
/// bullet met the head of this place's corpse (`places` the match's, for
/// the server's check).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Notice {
    None,
    HeadShot { victim: Place, places: u8 },
}

/// The local player's sight: seeing, or dark for the death it is dead of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sight {
    Seeing,
    Dark { death: DeathKey },
}

/// What the local player's own world shows of it at a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Own {
    /// No player of its own (a replay), or one that lives.
    Living,
    Dead { place: u8, death: DeathKey },
}

/// The places head shots were applied for since the last step, a bit each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shot(u32);

/// The sight after a step: dark begins when a head shot was applied for the
/// dead player's place, lasts while it is dead of that death, and ends with
/// it (a respawn, a rollback that undoes the death, another death).
fn sight(before: Sight, own: Own, shot: Shot) -> Sight {
    match (own, before) {
        (Own::Living, _) => Sight::Seeing,
        (Own::Dead { place, death }, _) if shot.0 & 1 << place != 0 => Sight::Dark { death },
        (Own::Dead { death, .. }, Sight::Dark { death: of }) if of == death => before,
        (Own::Dead { .. }, _) => Sight::Seeing,
    }
}

/// The input's actions with the notice in them, if this input can carry it:
/// one with no quick-use key and no command of its own (a quick key acts on
/// its press and its release: a command over it would lose one), and a
/// victim the server lets through.
fn carried(notice: Notice, actions: u32) -> Option<u32> {
    let Notice::HeadShot { victim, places } = notice else { return None };
    let command = Command::HeadShot(victim);
    (actions & (QUICK | CHORD) == 0 && command.allowed(places)).then(|| command.encode(actions))
}

static NOTICE: GameCell<Notice> = GameCell::new(Notice::None);
static SHOT: GameCell<Shot> = GameCell::new(Shot(0));
static SIGHT: GameCell<Sight> = GameCell::new(Sight::Seeing);
/// The sound thread's copy of the dark: every sound off while it is set.
static SILENT: AtomicBool = AtomicBool::new(false);
/// Whether the sound thread has the sounds off now (its own: it alone writes it).
static SILENCED: AtomicBool = AtomicBool::new(false);
/// What the hud movie was last told: the movie and its black view.
static TOLD: GameCell<Option<(usize, bool)>> = GameCell::new(None);

/// Patch 76's step saw the local player's own bullet push the head of
/// `victim`'s ragdoll, old enough: the next free input says so. One at a
/// time: a later one waits for no earlier one (the first is told).
pub fn head_hit(thread: &Thread, victim: usize, places: u32) {
    let (Ok(victim), Ok(places)) = (u8::try_from(victim), u8::try_from(places)) else { return };
    if let (Notice::None, Some(victim)) = (NOTICE.get(thread), Place::new(victim)) {
        NOTICE.set(thread, Notice::HeadShot { victim, places });
    }
}

/// The input builder's end (composed from artefact_carry's on_input_end,
/// after patches 78 and 85): the notice goes out in the first input that
/// can carry it.
pub fn on_input_end(handler: &mut InputHandler<'_>, thread: &Thread) {
    if let Some(sent) = carried(NOTICE.get(thread), handler.actions().bits()) {
        handler.set_actions(ActionMask::from_bits(sent));
        NOTICE.set(thread, Notice::None);
    }
}

/// The input's apply point (composed from artefact_carry's `carry.filter`,
/// before patch 78 filters the actions), in every world: a head shot's
/// victim is kept for the step.
pub fn on_filter(player: &Player<'_>, thread: &Thread) {
    if let Some(Command::HeadShot(victim)) = Command::of(player.input_actions()) {
        SHOT.update(thread, |shot| Shot(shot.0 | 1 << victim.get()));
    }
}

/// Patch 76's step, after the simulation's move: the local player's sight
/// from what its world shows of it. A notice whose victim has no ragdoll
/// any more (`gone`) is dropped.
pub fn after_step(thread: &Thread, own: Own, gone: impl Fn(usize) -> bool) {
    let shot = SHOT.get(thread);
    SHOT.set(thread, Shot(0));
    let now = sight(SIGHT.get(thread), own, shot);
    SIGHT.set(thread, now);
    SILENT.store(now != Sight::Seeing, Ordering::Release);
    if let Notice::HeadShot { victim, .. } = NOTICE.get(thread)
        && gone(usize::from(victim.get()))
    {
        NOTICE.set(thread, Notice::None);
    }
}

/// The game unloads its match (game::clear_resources): no step runs past
/// it, so the dark ends here.
pub fn on_unload(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    NOTICE.set(thread, Notice::None);
    SHOT.set(thread, Shot(0));
    SIGHT.set(thread, Sight::Seeing);
    SILENT.store(false, Ordering::Release);
    Flow::Continue
}

/// update_player_icons' start, every frame of a match: the hud movie's
/// black view told when it changes, and again to another movie.
pub fn on_update_icons(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Some(movie) = GameWorldUi::at_update_icons(ctx).and_then(|ui| ui.hud_movie()) {
        let dark = SIGHT.get(thread) != Sight::Seeing;
        let told = TOLD.get(thread);
        let now = Some((movie.id(), dark));
        // a movie never told is not black: it is told only the dark
        let new = told.is_none_or(|(id, _)| id != movie.id());
        if told != now && ((new && !dark) || movie.invoke_bool(c"root.set_dead_black", dark)) {
            TOLD.set(thread, now);
        }
    }
    Flow::Continue
}

/// The sound thread's tick: every sound off while dark.
pub fn on_sound_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let silent = SILENT.load(Ordering::Acquire);
    if SILENCED.load(Ordering::Relaxed) != silent
        && let Some(tick) = SoundTick::at(ctx)
    {
        tick.master_volume(if silent { Volume::SILENT } else { Volume::FULL });
        SILENCED.store(silent, Ordering::Relaxed);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEAD: Own = Own::Dead { place: 3, death: DeathKey(7) };

    #[test]
    fn dark_from_a_head_shot_of_ones_own_corpse_until_one_lives_again() {
        let none = Shot(0);
        let mine = Shot(1 << 3);
        assert_eq!(sight(Sight::Seeing, DEAD, none), Sight::Seeing);
        assert_eq!(sight(Sight::Seeing, DEAD, Shot(1 << 4)), Sight::Seeing, "another place's corpse");
        let dark = sight(Sight::Seeing, DEAD, mine);
        assert_eq!(dark, Sight::Dark { death: DeathKey(7) });
        assert_eq!(sight(dark, DEAD, none), dark, "while dead of that death");
        assert_eq!(sight(dark, DEAD, mine), dark);
        assert_eq!(sight(dark, Own::Living, none), Sight::Seeing, "the respawn, or a rollback");
        assert_eq!(sight(dark, Own::Dead { place: 3, death: DeathKey(9) }, none), Sight::Seeing, "another death");
        assert_eq!(sight(Sight::Seeing, Own::Living, mine), Sight::Seeing, "a living player is not blinded");
    }

    #[test]
    fn a_notice_waits_for_an_input_free_of_quick_keys_and_commands() {
        let victim = Place::new(5).unwrap();
        let notice = Notice::HeadShot { victim, places: 10 };
        assert_eq!(carried(Notice::None, 0x21), None);
        let sent = carried(notice, 0x21 | weapon_pickup::USE).unwrap();
        assert_eq!(Command::of(sent), Some(Command::HeadShot(victim)));
        assert_eq!(sent & !(QUICK | CHORD), 0x21 | weapon_pickup::USE, "the press keeps its own actions");
        assert_eq!(carried(notice, 0x21 | 1 << 14), None, "a quick key down");
        assert_eq!(carried(notice, weapon_pickup::encode(0, victim)), None, "a pickup's input");
        assert_eq!(carried(Notice::HeadShot { victim, places: 5 }, 0), None, "no place of the match");
    }
}

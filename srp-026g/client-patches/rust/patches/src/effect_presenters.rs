//! Patch 117: a game-effect presenter ignores an event for an effect it
//! holds no entry of.
//!
//! base_player::remove(1) (a respawn, a departure, a snapshot's kit change)
//! clears the player's presenter while the effects its game_effect_player
//! plays go on; a damage zone stops its effect on the player when the player
//! leaves it, the tick after. The sound and particle presenters' `on_event`
//! erase the entry their search found, and the search's "none" (-1) makes
//! the erase start one entry before the list: the presenter's own fields are
//! released as an entry's references (a sound presenter's seed freed: the
//! 2026-11-06 crash). The sound presenter's update reads a null entry.
//! The native code runs unchanged when the entry is there.
//!
//! A particle presenter's deserialize links each saved entry to the last
//! effect of its resource its player plays (resolve_link 0x2DDA80): two
//! effects of one resource (two copies of an artefact kind glowing at once)
//! come back with both entries on one. Its stop empties both and
//! erases one, the second effect's stop finds none (ignored above), and
//! the emptied entry is saved with no link. The next read of it reads its
//! effect's +30h at 0 (0x2AC94D, the 2026-11-27 crashes): an entry of no
//! effect is not read back.
#![forbid(unsafe_code)]

use crate::vostok::effect_presenter::{self, ReadEntry, Search};
use crate::vostok::log::{self, Piece::{Hex, Text}};
use crate::vostok::{Context, EffectPresentersOnParticleReadEntryExit, EffectPresentersOnParticleStopExit,
                    EffectPresentersOnSoundStopExit, EffectPresentersOnSoundUpdateExit, Flow};

fn ignored(ctx: &Context<'_>, event: &str) {
    log::refusal(ctx.thread(), &[Text("effect_presenters: "), Text(event),
                                 Text(" for an effect its presenter holds no entry of, ignored")]);
}

pub fn on_sound_stop(ctx: &mut Context<'_>) -> Flow<EffectPresentersOnSoundStopExit> {
    match effect_presenter::sound_stop(ctx) {
        Search::Held => Flow::Continue,
        Search::Missing => {
            ignored(ctx, "a sound's stop");
            Flow::Exit(EffectPresentersOnSoundStopExit::Ignore)
        }
    }
}

pub fn on_sound_update(ctx: &mut Context<'_>) -> Flow<EffectPresentersOnSoundUpdateExit> {
    match effect_presenter::sound_update(ctx) {
        Search::Held => Flow::Continue,
        Search::Missing => {
            ignored(ctx, "a sound's update");
            Flow::Exit(EffectPresentersOnSoundUpdateExit::Ignore)
        }
    }
}

pub fn on_particle_stop(ctx: &mut Context<'_>) -> Flow<EffectPresentersOnParticleStopExit> {
    match effect_presenter::particle_stop(ctx) {
        Search::Held => Flow::Continue,
        Search::Missing => {
            ignored(ctx, "a particle effect's stop");
            Flow::Exit(EffectPresentersOnParticleStopExit::Ignore)
        }
    }
}

pub fn on_particle_read_entry(ctx: &mut Context<'_>) -> Flow<EffectPresentersOnParticleReadEntryExit> {
    match effect_presenter::drop_entry_of_no_effect(ctx) {
        ReadEntry::Plays | ReadEntry::Short => Flow::Continue,
        ReadEntry::Dropped { resource } => {
            log::refusal(ctx.thread(), &[Text("effect_presenters: a saved particle entry of no effect its player plays (resource "),
                                         Hex(resource), Text("), not read back")]);
            Flow::Exit(EffectPresentersOnParticleReadEntryExit::Dropped)
        }
    }
}

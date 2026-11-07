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
//! And a particle presenter whose list is full skips a play: the list is six
//! entries inline (buffer_vector<effect_data>), and its push_back past them
//! asserts "buffer overflow" (the 2026-11-07 crash: a player's stock
//! activation glows with patch 111's leg quarters). The skipped effect plays
//! on in its game_effect_player, unseen; it holds no entry, so its stop and
//! updates are the ignored ones above.
#![forbid(unsafe_code)]

use crate::vostok::effect_presenter::{self, Room, Search};
use crate::vostok::log::{self, Piece::{Hex, Text}};
use crate::vostok::{Context, EffectPresentersOnParticlePlayExit, EffectPresentersOnParticleStopExit,
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

pub fn on_particle_play(ctx: &mut Context<'_>) -> Flow<EffectPresentersOnParticlePlayExit> {
    match effect_presenter::particle_play(ctx) {
        Room::Free => Flow::Continue,
        Room::Full { presenter } => {
            log::refusal(ctx.thread(), &[Text("effect_presenters: a particle effect's play on the full presenter "),
                                         Hex(presenter), Text(" (six effects), skipped")]);
            Flow::Exit(EffectPresentersOnParticlePlayExit::Full)
        }
    }
}

//! Patch 144: the Settings' Game slider (docs/notes/game-volume.md, the
//! owner's rules): the game's own sounds (steps, shots, effects, interface
//! sounds) at General x Game; the music (patch 88's bus) at General and its
//! Music slider only; the voice chat (patch 66, outside the engine) at its
//! own slider only.
//!
//! The stock option names `s_ingame_volume`, a variable stock 0.26g never
//! registered (the option is its only reference in the exe). Registered at
//! `configs.start`; the game's thread copies it every frame (`game.tick`) to
//! the sound thread, whose general pass (`sound_world.general`, the engine's
//! own replaced) gives every active scene its factor
//! ([`GeneralPass::mix`]): one writer of each scene's factor.
//!
//! Presentation only: nothing a match computes.

#![forbid(unsafe_code)]

use crate::music;
use crate::vostok::settings::ConsoleFloat;
use crate::vostok::sound::{GeneralPass, Volume};
use crate::vostok::{Context, Flow, GameVolumeOnGeneralExit, Lock, NoExit};

/// The Settings' Game slider: 0 to 100 as General's (the slider snaps by 1,
/// options_items 0x262D5A), full at first; saved in user.cfg.
static GAME_VOLUME: ConsoleFloat = ConsoleFloat::new(100.0, 0.0, 100.0);

/// The slider's share, 0 to 1, as the game's thread last copied it.
static LEVEL: Lock<Volume> = Lock::new(Volume::FULL);

/// game::on_configs_loaded, before user.cfg: the Game slider's variable.
pub fn on_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    GAME_VOLUME.register(ctx.thread(), c"s_ingame_volume");
    Flow::Continue
}

/// game::tick (every frame): the slider's share to the sound thread.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let level = Volume::new(GAME_VOLUME.get(thread) / 100.0);
    LEVEL.with(thread, |shared| *shared = level);
    Flow::Continue
}

/// The sound tick's general pass: every active scene's factor, the engine's
/// pass skipped.
pub fn on_general(ctx: &mut Context<'_>) -> Flow<GameVolumeOnGeneralExit> {
    let Some(pass) = GeneralPass::at(ctx) else {
        return Flow::Continue;
    };
    let thread = ctx.thread();
    let game = LEVEL.with(thread, |level| *level);
    pass.mix(music::bus(thread), game);
    Flow::Exit(GameVolumeOnGeneralExit::Done)
}

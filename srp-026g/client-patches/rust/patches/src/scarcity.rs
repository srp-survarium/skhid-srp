//! Patch 94: Scarcity, a battery match whose ammo crates refill no one (the
//! rules: docs/notes/scarcity.md; client_patch.md section 3.94). Its
//! starting ammo is the server's: the kits of the static info are cut
//! (`survarium::v026g::scarcity`), so nothing here gives or counts rounds.
//! The hooks are `scarcity.toml`'s, each a `lohook`:
//!
//! - the match's options read (`static_info.options_read`, after the 0xE4's
//!   match_options::deserialize; `replay.options_read`, after a replay's
//!   chunk 1's): srp's block may say the variant Scarcity (tag 2, one byte
//!   2; `vostok::options::variant_in`), kept before the match's world is made;
//! - resupply_zone_core::can_use (0x2F8080) and use_hint (0x2F80C0) at their
//!   starts: in a Scarcity match every zone is as another team's zone is to
//!   the game (zone+54h not the player's team, not 2): can_use answers 0
//!   (`refused`, 0x2F80BB: xor eax,eax / ret 4), so no use starts, and
//!   use_hint the empty key (`no_hint`, 0x2F80DC: mov eax,0x9DB708 "" /
//!   ret 4), so the hud shows no prompt.
//!
//! What a match computes: every game reads the same static info (a replay
//! its own chunk 1), so every game of a match refuses the crates alike.

#![forbid(unsafe_code)]

use crate::vostok::{Context, Flow, GameCell, MatchVariant, NoExit, OptionsRead, Thread};
use crate::vostok::{ScarcityOnCanUseExit, ScarcityOnUseHintExit};

/// The variant of the options last read (a match's or a replay's).
static VARIANT: GameCell<MatchVariant> = GameCell::new(MatchVariant::Plain);

/// What a crate does in a match of `variant`: refused in Scarcity.
fn crates_refuse(variant: MatchVariant) -> bool {
    variant == MatchVariant::Scarcity
}

fn options_read(thread: &Thread, read: Option<OptionsRead>) -> Flow<NoExit> {
    VARIANT.set(thread, read.map_or(MatchVariant::Plain, |read| read.variant()));
    Flow::Continue
}

/// process_static_match_info after its deserialize (0x27B92D).
pub fn on_static_info(ctx: &mut Context) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_static_info(ctx))
}

/// A replay's chunk 1 after its deserialize (0x1F0971).
pub fn on_replay_options(ctx: &mut Context) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_replay(ctx))
}

/// resupply_zone_core::can_use's start (0x2F8080).
pub fn on_can_use(ctx: &mut Context) -> Flow<ScarcityOnCanUseExit> {
    if crates_refuse(VARIANT.get(ctx.thread())) {
        return Flow::Exit(ScarcityOnCanUseExit::Refused);
    }
    Flow::Continue
}

/// resupply_zone_core::use_hint's start (0x2F80C0).
pub fn on_use_hint(ctx: &mut Context) -> Flow<ScarcityOnUseHintExit> {
    if crates_refuse(VARIANT.get(ctx.thread())) {
        return Flow::Exit(ScarcityOnUseHintExit::NoHint);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_scarcity_refuses_the_crates() {
        assert!(crates_refuse(MatchVariant::Scarcity));
        assert!(!crates_refuse(MatchVariant::Blackout));
        assert!(!crates_refuse(MatchVariant::Plain));
    }
}

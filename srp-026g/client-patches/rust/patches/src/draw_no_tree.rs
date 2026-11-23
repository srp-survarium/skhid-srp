//! Patch 196: a player drawn before his first spawn no longer crashes the game.
//!
//! `player::mt_draw` (0x2474A0) ticks a not alive player's (+33Ch 0) animation player up to the
//! player's time (+324h) whenever its tree's time (`tree_actual_time` 0x392370, 0 without a tree)
//! is below it (0x24751C..0x247543). It never tests the tree, and `animation_player::tick`
//! (0x3917D0) hands it to `n_ary_tree::tick`, which reads it at once (0x3A19F9). A place's player
//! that was never spawned nor loaded has no tree and the constructor's time -1 (0x2D6C27): drawn,
//! it faults there (the crash report of 2026-11-22, revision 71: place 12, a newcomer whose place
//! was active at the draw, its animation player as the constructor leaves it).
//!
//! With no tree there is nothing to tick (the 0.10b source's tree is a member: an empty one's tick
//! only takes the time). The draw skips the tick, says so in the patches' log once a player (its
//! place, kit, time and whether it is in the world), and the match plays on (the owner's rule: the
//! game's bookkeeping disagrees and nobody did anything wrong). Only the draw changes: the
//! simulation never comes here.
#![forbid(unsafe_code)]

use crate::vostok::draw::{DeadTick, Unanimated};
use crate::vostok::log::{Piece::{Dec, Hex, Text}, line};
use crate::vostok::world::PlayerId;
use crate::vostok::{Context, DrawNoTreeOnDeadTickExit, Flow, Lock, PLACES};

/// Each place's player last logged (a line once a player).
static LOGGED: Lock<[Option<PlayerId>; PLACES]> = Lock::new([None; PLACES]);

pub fn on_dead_tick(ctx: &mut Context<'_>) -> Flow<DrawNoTreeOnDeadTickExit> {
    match DeadTick::at(ctx) {
        DeadTick::Animated => Flow::Continue,
        DeadTick::NoTree(player) => {
            if first_seen(ctx, &player) {
                line(&[Text("draw_no_tree: place "), Dec(player.place.into()), Text(" kit "), Dec(player.kit.into()),
                       Text(": a not alive player with no animation tree, its draw's tick skipped (player "),
                       Hex(player.id.address()), Text(", time "), Hex(player.time), Text(", spawned "),
                       Hex(player.spawned), Text(if player.inserted { ", in the world)" } else { ", not in the world)" })]);
            }
            Flow::Exit(DrawNoTreeOnDeadTickExit::NoTree)
        }
    }
}

/// Whether this is the first draw of `player` with no tree at its place since another player's.
fn first_seen(ctx: &Context<'_>, player: &Unanimated) -> bool {
    LOGGED.with(ctx.thread(), |logged| match logged.get_mut(usize::from(player.place)) {
        Some(slot) if *slot == Some(player.id) => false,
        Some(slot) => {
            *slot = Some(player.id);
            true
        }
        None => true,
    })
}

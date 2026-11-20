//! Patch 163: room for twice as many textures in the render's fixed
//! texture lists, taken only when the game's is full.
//!
//! - The resource manager's texture names: one for every texture its
//!   effects asked for while the texture is registered (the release of the
//!   texture erases it), 2048; a match whose players wear many different
//!   items asks for more at its load, and push_back 0x1326F0 asserts (the
//!   int3 0x132755). The names are an index of the textures alive, nothing
//!   more: they cost 110h bytes each, the textures behind them are loaded
//!   whatever the list holds.
//! - A scene's streamable textures, 2048, with the async worker's
//!   candidates (one for each, 2048): push_back 0x155F60 asserts (the int3
//!   0x155FC2) a few minutes into such a match. The requests (1024, one
//!   for each candidate whose mips change, asked in one batch) keep their
//!   room, and a batch keeps to it: process_streaming makes room on the
//!   main thread's stack (1 MiB) for every candidate of a batch at once,
//!   14Ch each, before it asks the requests; past 1024 the game asserts
//!   there, past about 2500 the stack would overflow. The rest are asked
//!   in the next batch (the async worker makes the candidates again).
//! - A texture pool's textures, 1024: the texture storage keeps its
//!   textures in pools by size and format, one slot for each texture alive
//!   at that size, and makes one more when none is free; past 1024 push_back
//!   0x3D35B0 asserts (the int3 0x3D360D). With the names' room, such a
//!   match's load filled the 64x64 DXT1 pool next: 1024 textures alive at
//!   their smallest streamed mips.
//!
//! When a push finds a list full in its owner's room, its entries move to
//! a room of ours twice the size (a scene's two lists together), from the
//! process heap: nothing of it is taken before. A scene's rooms go back in
//! its destructor; a pool's live as long as its storage (the game). Past
//! our room the game's assert stays, as when the heap gives none (one line
//! in the log). The names' and the streamable textures' highs get a line
//! from 1024 entries on, every 256 more: how full they get in a match.
#![forbid(unsafe_code)]

use crate::vostok::log::{self, Piece::{Dec, Text}};
use crate::vostok::log::Limited;
use crate::vostok::texture_lists::{Batch, Fill, Names, NotMoved, Place, PoolSlots, Streaming, OWN_NAMES, OWN_POOL_SLOTS,
                                   OWN_STREAMABLE};
use crate::vostok::{Context, Flow, Lock, NoExit};

/// Our rooms: twice the game's.
const NAMES: u32 = 2 * OWN_NAMES;
const POOL_SLOTS: u32 = 2 * OWN_POOL_SLOTS;
const STREAMABLE: u32 = 2 * OWN_STREAMABLE;

/// The first high logged, and every this many entries over the last one.
const FIRST: u32 = 1024;
const STEP: u32 = 256;

/// What a push into a list does first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Push {
    /// It has room.
    Room,
    /// Full in its owner's room: it moves into ours.
    Move,
    /// Full in ours: the game's assert.
    Full,
}

fn push(fill: Fill) -> Push {
    match (fill.place, fill.full()) {
        (_, false) => Push::Room,
        (Place::Own, true) => Push::Move,
        (Place::Ours, true) => Push::Full,
    }
}

/// The highest count of a list logged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum High {
    None,
    Logged(u32),
}

impl High {
    /// `held` entries: the new high to log, if it is one.
    fn reached(&mut self, held: u32) -> Option<u32> {
        let next = match *self {
            High::None => FIRST,
            High::Logged(last) => last + STEP,
        };
        (held >= next).then(|| {
            *self = High::Logged(held);
            held
        })
    }
}

/// The names' high, and the streamable textures' of the scenes alive.
static HIGHS: Lock<(High, High)> = Lock::new((High::None, High::None));

pub fn on_name_push(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(names) = Names::at(ctx) else { return Flow::Continue };
    let fill = names.fill();
    match push(fill) {
        Push::Room => {
            if let Some(held) = HIGHS.with(ctx.thread(), |(high, _)| high.reached(fill.held)) {
                log::line(&[Text("texture_lists: the texture names hold "), Dec(held)]);
            }
        }
        Push::Move => match names.move_to_room(NAMES) {
            Ok(()) => log::line(&[Text("texture_lists: the texture names, full at "), Dec(fill.held),
                                  Text(", moved to a room of "), Dec(NAMES)]),
            Err(NotMoved::NoMemory) => log::line(&[Text("texture_lists: the texture names are full at "), Dec(fill.held),
                                                   Text(" and the heap gave no room")]),
        },
        Push::Full => log::line(&[Text("texture_lists: the texture names are full at "), Dec(fill.held)]),
    }
    Flow::Continue
}

pub fn on_streaming_push(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(streaming) = Streaming::at_push(ctx) else { return Flow::Continue };
    let fill = streaming.streamable();
    match push(fill) {
        Push::Room => {
            if let Some(held) = HIGHS.with(ctx.thread(), |(_, high)| high.reached(fill.held)) {
                log::line(&[Text("texture_lists: a scene streams "), Dec(held), Text(" textures")]);
            }
        }
        Push::Move => match streaming.move_to_rooms(STREAMABLE) {
            Ok(()) => log::line(&[Text("texture_lists: a scene's streamable textures, full at "), Dec(fill.held),
                                  Text(", moved to a room of "), Dec(STREAMABLE)]),
            Err(NotMoved::NoMemory) => log::line(&[Text("texture_lists: a scene's streamable textures are full at "),
                                                   Dec(fill.held), Text(" and the heap gave no room")]),
        },
        Push::Full => log::line(&[Text("texture_lists: a scene's streamable textures are full at "), Dec(fill.held)]),
    }
    Flow::Continue
}

pub fn on_pool_slot_push(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(slots) = PoolSlots::at(ctx) else { return Flow::Continue };
    let fill = slots.fill();
    match push(fill) {
        Push::Room => {}
        Push::Move => match slots.move_to_room(POOL_SLOTS) {
            Ok(()) => log::line(&[Text("texture_lists: the texture pool "), Text(slots.name()), Text(", full at "),
                                  Dec(fill.held), Text(", moved to a room of "), Dec(POOL_SLOTS)]),
            Err(NotMoved::NoMemory) => log::line(&[Text("texture_lists: the texture pool "), Text(slots.name()),
                                                   Text(" is full at "), Dec(fill.held), Text(" and the heap gave no room")]),
        },
        Push::Full => log::line(&[Text("texture_lists: the texture pool "), Text(slots.name()), Text(" is full at "),
                                  Dec(fill.held)]),
    }
    Flow::Continue
}

/// The batches cut to the requests' room, at most a line a second.
static CUT: Limited = Limited::new("cut");

/// A batch of more candidates than the requests hold asks the first ones.
pub fn on_request_batch(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(batch) = Batch::at(ctx) else { return Flow::Continue };
    let (candidates, room) = batch.sizes();
    if candidates > room {
        batch.keep_first(room);
        CUT.line(ctx.thread(), &[Text("texture_lists: "), Dec(candidates), Text(" textures change their mips, "),
                                 Dec(room), Text(" asked now, the rest next")]);
    }
    Flow::Continue
}

pub fn on_scene_lists_destroyed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(streaming) = Streaming::at_destroyed(ctx) {
        if streaming.give_back() == Place::Ours {
            log::line(&[Text("texture_lists: a scene's rooms are given back")]);
        }
        HIGHS.with(ctx.thread(), |(_, high)| *high = High::None);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(held: u32, room: u32, place: Place) -> Fill {
        Fill { held, room, place }
    }

    #[test]
    fn a_full_list_moves_once_then_keeps_the_assert() {
        assert_eq!(push(fill(2047, 2048, Place::Own)), Push::Room);
        assert_eq!(push(fill(2048, 2048, Place::Own)), Push::Move);
        assert_eq!(push(fill(2049, 4096, Place::Ours)), Push::Room);
        assert_eq!(push(fill(4096, 4096, Place::Ours)), Push::Full);
    }

    #[test]
    fn a_high_is_logged_from_1024_and_then_every_256() {
        let mut high = High::None;
        assert_eq!(high.reached(1023), None);
        assert_eq!(high.reached(1024), Some(1024));
        assert_eq!(high.reached(1025), None);
        assert_eq!(high.reached(1279), None);
        assert_eq!(high.reached(1300), Some(1300));
        assert_eq!(high.reached(1000), None);
        assert_eq!(high.reached(1555), None);
        assert_eq!(high.reached(1556), Some(1556));
    }
}

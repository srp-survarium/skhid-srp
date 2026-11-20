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
//!   candidates (one for each, 2048) and the requests (1024): push_back
//!   0x155F60 asserts (the int3 0x155FC2) a few minutes into such a match.
//! - A texture pool's textures, 1024: the texture storage keeps its
//!   textures in pools by size and format, one slot for each texture alive
//!   at that size, and makes one more when none is free; past 1024 push_back
//!   0x3D35B0 asserts (the int3 0x3D360D). With the names' room, such a
//!   match's load filled the 64x64 DXT1 pool next: 1024 textures alive at
//!   their smallest streamed mips.
//!
//! When a push finds a list full in its owner's room, its entries move to
//! a room of ours twice the size (a scene's three lists together), from the
//! process heap: nothing of it is taken before. A scene's rooms go back in
//! its destructor; a pool's live as long as its storage (the game). Past
//! our room the game's assert stays, as when the heap gives none (one line
//! in the log). In a room of ours, each new high of the names or the
//! streamable textures by 256 entries gets a line: how full they get.
#![forbid(unsafe_code)]

use crate::vostok::log::{self, Piece::{Dec, Text}};
use crate::vostok::texture_lists::{Fill, Names, NotMoved, Place, PoolSlots, Streaming, StreamingRooms, OWN_NAMES,
                                   OWN_POOL_SLOTS, OWN_REQUESTED, OWN_STREAMABLE};
use crate::vostok::{Context, Flow, Lock, NoExit};

/// Our rooms: twice the game's.
const NAMES: u32 = 2 * OWN_NAMES;
const POOL_SLOTS: u32 = 2 * OWN_POOL_SLOTS;
const ROOMS: StreamingRooms = StreamingRooms { streamable: 2 * OWN_STREAMABLE, requested: 2 * OWN_REQUESTED };

/// A new high this many entries over the last one logged gets a line.
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

/// The highest count logged of a list in a room of ours (0: none yet).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct High(u32);

impl High {
    /// `held` entries in a room of ours: the new high to log, if it is one.
    fn reached(&mut self, fill: Fill) -> Option<u32> {
        let next = if self.0 == 0 { fill.held } else { self.0 + STEP };
        (fill.place == Place::Ours && fill.held >= next).then(|| {
            self.0 = fill.held;
            fill.held
        })
    }
}

/// The names' high, and the streamable textures' of the scenes alive.
static HIGHS: Lock<(High, High)> = Lock::new((High(0), High(0)));

pub fn on_name_push(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(names) = Names::at(ctx) else { return Flow::Continue };
    let fill = names.fill();
    match push(fill) {
        Push::Room => {
            if let Some(held) = HIGHS.with(ctx.thread(), |(high, _)| high.reached(fill)) {
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
            if let Some(held) = HIGHS.with(ctx.thread(), |(_, high)| high.reached(fill)) {
                log::line(&[Text("texture_lists: a scene streams "), Dec(held), Text(" textures")]);
            }
        }
        Push::Move => match streaming.move_to_rooms(ROOMS) {
            Ok(()) => log::line(&[Text("texture_lists: a scene's streamable textures, full at "), Dec(fill.held),
                                  Text(", moved to a room of "), Dec(ROOMS.streamable), Text(" (requests "),
                                  Dec(ROOMS.requested), Text(")")]),
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

pub fn on_scene_lists_destroyed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(streaming) = Streaming::at_destroyed(ctx) {
        if streaming.give_back() == Place::Ours {
            HIGHS.with(ctx.thread(), |(_, high)| *high = High(0));
        }
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
    fn a_high_is_logged_once_and_then_every_step() {
        let mut high = High(0);
        assert_eq!(high.reached(fill(2047, 2048, Place::Own)), None);
        assert_eq!(high.reached(fill(2048, 4096, Place::Ours)), Some(2048));
        assert_eq!(high.reached(fill(2049, 4096, Place::Ours)), None);
        assert_eq!(high.reached(fill(2100, 4096, Place::Ours)), None);
        assert_eq!(high.reached(fill(2304, 4096, Place::Ours)), Some(2304));
        assert_eq!(high.reached(fill(2000, 4096, Place::Ours)), None);
        assert_eq!(high.reached(fill(2559, 4096, Place::Ours)), None);
        assert_eq!(high.reached(fill(2560, 4096, Place::Ours)), Some(2560));
    }
}

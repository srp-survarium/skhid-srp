//! Patch 122: room for 32 particle effects in each of the game world's
//! presenters, where the game keeps six.
//!
//! A particle presenter plays a player's particle effects from a list with
//! room for six entries inside it; a seventh asserts in push_back 0x2ACC20
//! ("buffer overflow", the int3 at 0x2ACC7F): the rev52/rev53 crashes, a
//! player's list full of patch 111's glows when it activated an artefact.
//! A player can now show 6 artefact glows, 3 activations and Forest
//! Protection's at once (12 quarters with Resonance's shared copies), and the
//! stock effects beside them.
//!
//! - The game world's first-person presenter and its twenty places'
//!   third-person ones are given a [`Room`] of 32 entries as they are built
//!   (game_world's constructor, after each presenter's): one per presenter,
//!   kept by its address, the same room again when the presenter is built
//!   again at that address (the game world is a static object). The lobby's
//!   presenters keep their six.
//! - The presenter's deserialize (a world's load) reads the saved entries
//!   into a local list of six of its own, then swaps it with the
//!   presenter's through a stack room of six: a presenter with a room reads
//!   into the room's second half, and the swap is made in place, entry for
//!   entry, for every presenter.
//!
//! When the table is full or the heap has no room, the presenter keeps the
//! game's six (one refusal line): never more than the game's limit, never a
//! write past a room. Presentation only: nothing a match computes reads it.
#![forbid(unsafe_code)]

use crate::vostok::effect_presenter::{self, Built, NotGiven, PresenterId, Read, Room, Swap};
use crate::vostok::log::{self, Piece::{Dec, Hex, Text}};
use crate::vostok::{Context, Flow, Lock, NoExit, ParticleRoomOnSwapExit, Thread};

/// A presenter's room, in entries (the game's: six).
const ENTRIES: u32 = 32;

/// The presenters given a room: the game world's 21, with room to spare.
const ROWS: usize = 64;

/// One presenter's room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Row {
    presenter: PresenterId,
    room: Room,
}

/// Why a presenter got no room.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NoRoom {
    /// Every row is a presenter's.
    Full,
    /// The heap gave none.
    NoMemory,
}

/// The rooms, a presenter's at most once.
struct Rooms([Option<Row>; ROWS]);

impl Rooms {
    const fn new() -> Rooms {
        Rooms([None; ROWS])
    }

    /// The room `presenter` was given.
    fn of(&self, presenter: PresenterId) -> Option<Room> {
        self.0.iter().flatten().find(|row| row.presenter == presenter).map(|row| row.room)
    }

    /// The room `presenter` has, or a new one in the first free row.
    fn room_for(&mut self, presenter: PresenterId, allocate: impl FnOnce() -> Option<Room>) -> Result<Room, NoRoom> {
        if let Some(room) = self.of(presenter) {
            return Ok(room);
        }
        let free = self.0.iter_mut().find(|row| row.is_none()).ok_or(NoRoom::Full)?;
        let room = allocate().ok_or(NoRoom::NoMemory)?;
        *free = Some(Row { presenter, room });
        Ok(room)
    }
}

static ROOMS: Lock<Rooms> = Lock::new(Rooms::new());

fn give(thread: &Thread, built: Option<Built>) {
    let Some(built) = built else { return };
    let presenter = built.id();
    let refused = |why: &str| {
        log::refusal(thread, &[Text("particle_room: presenter "), Hex(presenter.address()), Text(" keeps the game's six entries: "),
                               Text(why)]);
    };
    match ROOMS.with(thread, |rooms| rooms.room_for(presenter, || Room::allocate(ENTRIES))) {
        Ok(room) => match built.give(room) {
            Ok(()) => {}
            Err(NotGiven::NotEmpty) => refused("its list is not empty"),
        },
        Err(NoRoom::Full) => refused("every row of the table is taken"),
        Err(NoRoom::NoMemory) => refused("the heap gave no room"),
    }
}

pub fn on_first_person_made(ctx: &mut Context<'_>) -> Flow<NoExit> {
    give(ctx.thread(), Built::first_person(ctx));
    Flow::Continue
}

pub fn on_third_person_made(ctx: &mut Context<'_>) -> Flow<NoExit> {
    give(ctx.thread(), Built::third_person(ctx));
    Flow::Continue
}

/// deserialize's local list: in the presenter's room when it has one, so it
/// holds as many as the presenter's list can.
pub fn on_read_room(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(read) = Read::at(ctx) else { return Flow::Continue };
    let room = read.presenter().and_then(|presenter| ROOMS.with(read.thread(), |rooms| rooms.of(presenter)));
    if let Some(room) = room {
        read.give(room);
    }
    Flow::Continue
}

/// The swap, in place; the game's own (through a stack room of six) never runs.
pub fn on_swap(ctx: &mut Context<'_>) -> Flow<ParticleRoomOnSwapExit> {
    match effect_presenter::swap(ctx) {
        Swap::Swapped { .. } => {}
        Swap::NoRoom { kept, kept_room, read, read_room } => {
            log::refusal(ctx.thread(), &[Text("particle_room: a world's load left a presenter's list as it was: "),
                                          Dec(kept), Text(" entries (room "), Dec(kept_room), Text("), "),
                                          Dec(read), Text(" read (room "), Dec(read_room), Text(")")]);
        }
        Swap::NoList => log::refusal(ctx.thread(), &[Text("particle_room: a swap without its lists")]),
    }
    Flow::Exit(ParticleRoomOnSwapExit::Returned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(n: u32) -> Option<Room> {
        Room::for_test(n)
    }

    #[test]
    fn a_presenter_built_again_gets_its_own_room_back() {
        let mut rooms = Rooms::new();
        let (a, b) = (PresenterId::for_test(0xA98B24), PresenterId::for_test(0xA98D44));
        let first = rooms.room_for(a, || room(1)).unwrap();
        assert_eq!(rooms.room_for(b, || room(2)).unwrap(), room(2).unwrap());
        assert_eq!(rooms.room_for(a, || panic!("allocated again")).unwrap(), first);
        assert_eq!(rooms.of(a), Some(first));
        assert_eq!(rooms.of(PresenterId::for_test(4)), None);
    }

    #[test]
    fn a_full_table_or_heap_keeps_the_games_room() {
        let mut rooms = Rooms::new();
        for n in 0..ROWS as u32 {
            rooms.room_for(PresenterId::for_test(0x1000 + n * 4), || room(n + 1)).unwrap();
        }
        assert_eq!(rooms.room_for(PresenterId::for_test(8), || panic!("no row")), Err(NoRoom::Full));
        let mut rooms = Rooms::new();
        assert_eq!(rooms.room_for(PresenterId::for_test(8), || None), Err(NoRoom::NoMemory));
        assert_eq!(rooms.of(PresenterId::for_test(8)), None, "a refused presenter has no row");
    }
}

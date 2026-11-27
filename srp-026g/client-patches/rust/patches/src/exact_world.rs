//! Patch 211: the world of a tick, for the desync capture
//! (`docs/notes/desync_capture.md`).
//!
//! The match server learns that the games disagree from their hash reports
//! (0xDF), each the CRC of a game's event-horizon world, the oldest it
//! keeps: by the time a report is at the server, that tick's world has left
//! the game, and the snapshot upload (0xF6, client_patch.md §3.4) gives the
//! world of the moment. So the game keeps the worlds it hashes:
//!
//! - every world `game_state_history_item::hash` serializes (each tick's as
//!   the simulation makes it, again at a rollback that changes it) goes into
//!   a ring ([`Ring`]): the newest [`SLOTS`] ticks, about 1.6 s;
//! - an upload request of 1 and a tick (`srp_validate::messages::
//!   UploadRequest::Tick`) uploads the ring's world of that tick, with the
//!   snapshot upload's chunks; with no such world, or a request of one byte,
//!   the snapshot upload goes on as before (the event horizon's world).
//!
//! The game says it answers so in its 0xD8 (`EXACT_WORLDS`, the late join's
//! `connect_capability`), so the server sends two bytes only to a game that
//! takes them. Nothing the game simulates, hashes or uploads otherwise
//! changes: the hook only copies the bytes hash() made.
#![forbid(unsafe_code)]

use srp_validate::messages::UploadRequest;

use crate::vostok::exact_world::{Hashed, Request};
use crate::vostok::{Context, ExactWorldOnRequestExit, Flow, Lock, NoExit};

/// The ring's bytes: 640 KiB, about 32 worlds of a full match (20 KB
/// each), ten of the largest a game can make (0xF000, world_frame's
/// `MAX_BODY`). Part of the arena's state, so taken once, with the image.
pub const POOL: usize = 640 * 1024;
/// The ticks it holds at most: 1.6 s. A report reaches the server about a
/// way after the tick left the event horizon (`max_ping` + 250 ms back),
/// and the request comes back a way later: the tick is still held after a
/// round trip of about a second.
pub const SLOTS: usize = 32;
/// A world this far before the newest one held is of another match (or a
/// replay's start): the ring is emptied first.
const NEW_TIMELINE_MS: u32 = 5_000;
const TICK_MS: u32 = 50;
const _: () = assert!(POOL >= world_frame::MAX_BODY);

/// One world held: its time and its bytes' place in the pool.
#[derive(Clone, Copy)]
struct Entry {
    time: u32,
    at: u32,
    len: u32,
}

impl Entry {
    fn overlaps(&self, at: usize, len: usize) -> bool {
        let (start, end) = (self.at as usize, (self.at + self.len) as usize);
        start < at + len && at < end
    }
}

/// The worlds hashed last, a tick each: the newest copy of a tick replaces
/// its older one, a world goes into the pool after the last one (from the
/// pool's start when it does not fit at its end), and the worlds whose
/// bytes it takes, or the oldest past [`SLOTS`], go.
pub struct Ring {
    pool: [u8; POOL],
    /// `entries[..count]`, oldest first.
    entries: [Entry; SLOTS],
    count: usize,
    /// Where the next world goes.
    next: usize,
}

impl Ring {
    /// Every byte zero: the arena's state (no file bytes but its zeros).
    pub const EMPTY: Ring = Ring {
        pool: [0; POOL],
        entries: [Entry { time: 0, at: 0, len: 0 }; SLOTS],
        count: 0,
        next: 0,
    };

    fn held(&self) -> &[Entry] {
        &self.entries[..self.count]
    }

    fn keep(&mut self, mut keep: impl FnMut(&Entry) -> bool) {
        let mut kept = 0;
        for index in 0..self.count {
            let entry = self.entries[index];
            if keep(&entry) {
                self.entries[kept] = entry;
                kept += 1;
            }
        }
        self.count = kept;
    }

    /// The world of `time` hashed: kept as that tick's.
    pub fn put(&mut self, time: u32, body: &[u8]) {
        let len = body.len();
        if len > POOL {
            return;
        }
        let newest = self.held().iter().map(|entry| entry.time).max();
        if newest.is_some_and(|newest| time.saturating_add(NEW_TIMELINE_MS) < newest) {
            self.count = 0;
            self.next = 0;
        }
        let at = if self.next + len <= POOL { self.next } else { 0 };
        self.keep(|entry| entry.time != time && !entry.overlaps(at, len));
        if self.count == SLOTS {
            self.keep({
                let mut first = true;
                move |_| !core::mem::take(&mut first)
            });
        }
        self.pool[at..at + len].copy_from_slice(body);
        self.entries[self.count] = Entry { time, at: at as u32, len: len as u32 };
        self.count += 1;
        self.next = at + len;
    }

    /// The world held of the tick whose byte is `tick` (its time / 50,
    /// modulo 256), the newest such: its time and bytes.
    pub fn get(&self, tick: u8) -> Option<(u32, &[u8])> {
        let entry = self
            .held()
            .iter()
            .filter(|entry| (entry.time / TICK_MS) as u8 == tick)
            .max_by_key(|entry| entry.time)?;
        let (at, len) = (entry.at as usize, entry.len as usize);
        Some((entry.time, &self.pool[at..at + len]))
    }
}

static RING: Lock<Ring> = Lock::new(Ring::EMPTY);

/// hash() serialized a world: keep it.
pub fn on_hashed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(hashed) = Hashed::at(ctx) {
        RING.with(ctx.thread(), |ring| ring.put(hashed.time, hashed.body));
    }
    Flow::Continue
}

/// An upload request: of a tick the ring holds, that tick's world, and the
/// case is done; anything else goes on to the snapshot upload.
pub fn on_request(ctx: &mut Context<'_>) -> Flow<ExactWorldOnRequestExit> {
    let Some(request) = Request::at(ctx) else {
        return Flow::Continue;
    };
    let Some(UploadRequest::Tick(tick)) = UploadRequest::parse(request.payload()) else {
        return Flow::Continue;
    };
    let sent = RING.with(request.thread(), |ring| {
        ring.get(tick).is_some_and(|(time, body)| request.upload(time, body))
    });
    if sent {
        Flow::Exit(ExactWorldOnRequestExit::Done)
    } else {
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::boxed::Box;
    use std::vec::Vec;

    fn world(time: u32, len: usize) -> Vec<u8> {
        (0..len).map(|i| (time as usize + i) as u8).collect()
    }

    fn ring() -> Box<Ring> {
        // Not on the test thread's stack: 640 KiB.
        Box::new(Ring::EMPTY)
    }

    #[test]
    fn a_hashed_tick_is_given_back_byte_for_byte_and_a_missing_one_is_none() {
        let mut ring = ring();
        ring.put(270_700, &world(270_700, 20_000));
        ring.put(270_750, &world(270_750, 21_000));
        let tick = (270_700 / 50) as u8;
        assert_eq!(ring.get(tick), Some((270_700, &world(270_700, 20_000)[..])));
        assert_eq!(ring.get(tick.wrapping_add(2)), None);
    }

    #[test]
    fn a_tick_hashed_again_is_its_newest_world() {
        let mut ring = ring();
        ring.put(1_000, &world(1, 300));
        ring.put(1_050, &world(2, 300));
        ring.put(1_000, &world(3, 400));
        assert_eq!(ring.get(20), Some((1_000, &world(3, 400)[..])));
        assert_eq!(ring.held().len(), 2);
    }

    #[test]
    fn it_holds_the_last_32_ticks_and_the_pool_wraps() {
        let mut ring = ring();
        for tick in 0..100u32 {
            ring.put(10_000 + tick * 50, &world(tick, 20_000));
        }
        assert_eq!(ring.held().len(), SLOTS);
        for tick in 0..100u32 {
            let time = 10_000 + tick * 50;
            let got = ring.get((time / 50) as u8);
            if tick >= 100 - SLOTS as u32 {
                assert_eq!(got, Some((time, &world(tick, 20_000)[..])), "{tick}");
            } else {
                assert_eq!(got, None, "{tick}");
            }
        }
        // The largest worlds: five fit.
        for tick in 0..10u32 {
            ring.put(20_000 + tick * 50, &world(tick, world_frame::MAX_BODY));
        }
        let held: Vec<u32> = ring.held().iter().map(|entry| entry.time).collect();
        assert!(held.len() >= 5 && held.ends_with(&[20_450]), "{held:?}");
        for entry in ring.held() {
            let tick = (entry.time - 20_000) / 50;
            assert_eq!(ring.get((entry.time / 50) as u8), Some((entry.time, &world(tick, world_frame::MAX_BODY)[..])));
        }
    }

    #[test]
    fn a_world_of_another_match_empties_it() {
        let mut ring = ring();
        ring.put(600_000, &world(1, 100));
        ring.put(50, &world(2, 100));
        assert_eq!(ring.held().len(), 1);
        assert_eq!(ring.get((600_000 / 50) as u8), None);
        assert_eq!(ring.get(1), Some((50, &world(2, 100)[..])));
    }
}

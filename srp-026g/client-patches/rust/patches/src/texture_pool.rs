//! Patch 182: a streamed texture that wants a size whose pool slots are all taken takes the
//! largest smaller size that has a free slot (client_patch.md 3.99).
//!
//! The pools are a fixed table of slots per size (60 of 2048x2048 BC1, 30 of BC3, 120 of
//! 1024x1024, ...; `vostok::texture_pool`). Stock, a request for a size whose slots are all
//! taken is cancelled, so the texture keeps the mips it has, however few (often the small ones
//! it was loaded with), until a slot of exactly that size frees: a surface stays blurred while
//! the textures nearer the camera hold every slot of its size. Now it takes the largest size
//! between the two that has a free slot, and asks for the full size again at the next batch.
//! A texture that has no smaller size free, or wants fewer mips, goes on as stock.
//!
//! With `SKHID_TEXTURE_STREAMING_LOG=on` (read once) each such request is logged, once a texture
//! while what it wants and gets stays the same: the texture, the sizes, its pool's slots and
//! whose textures fill them (the scene's, or textures still alive that no scene streams).

#![forbid(unsafe_code)]

use crate::vostok::log::{Piece, Piece::*, parts};
use crate::vostok::texture_pool::{Level, NoSlot, Pool, Want};
use crate::vostok::{Context, Flow, GameCell, Lock, TexturePoolOnNoSlotExit, Thread};

/// The sizes a request may take instead of `wanted`, largest first: each above `resident`.
fn smaller_levels(resident: Level, wanted: Level) -> impl Iterator<Item = Level> {
    (1..wanted.mips.saturating_sub(resident.mips)).filter_map(move |steps| wanted.smaller(steps))
}

/// process_streaming at a request whose size has no free slot (`streaming.no_slot`).
pub fn on_no_slot(ctx: &mut Context<'_>) -> Flow<TexturePoolOnNoSlotExit> {
    let Some(mut request) = NoSlot::at_site(ctx) else {
        return Flow::Continue;
    };
    let want = request.want();
    let took = smaller_levels(want.resident, want.wanted).find(|&level| request.take(level));
    if log_on(ctx.thread()) {
        report(ctx.thread(), &request, want, took);
    }
    match took {
        Some(_) => Flow::Exit(TexturePoolOnNoSlotExit::Next),
        None => Flow::Continue,
    }
}

/// The textures logged, by their index in the scene, with what they wanted and got then.
const LOGGED: usize = 4096;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Logged {
    wanted: u8,
    took: Option<u8>,
}

static LAST: Lock<[Option<Logged>; LOGGED]> = Lock::new([None; LOGGED]);

fn report(thread: &Thread, request: &NoSlot, want: Want, took: Option<Level>) {
    let now = Logged { wanted: want.wanted.mips as u8, took: took.map(|level| level.mips as u8) };
    let fresh = match LAST.with(thread, |last| last.get_mut(want.texture as usize).map(|at| core::mem::replace(at, Some(now)))) {
        Some(before) => before != Some(now),
        None => true,
    };
    if !fresh {
        return;
    }
    let path = core::str::from_utf8(request.path()).unwrap_or("(a path not text)");
    let slots;
    let pool: &[Piece] = match request.pool(want.wanted) {
        Pool::Absent => &[Text(": no pool of that size")],
        Pool::Slots { total, reserved, occupied } => {
            slots = [
                Text(": its pool "),
                Dec(total),
                Text(" slots, "),
                Dec(reserved),
                Text(" reserved, occupied by "),
                Dec(occupied.in_scene),
                Text(" of the scene's, "),
                Dec(occupied.baked_elsewhere),
                Text(" baked and "),
                Dec(occupied.other_elsewhere),
                Text(" other textures it does not stream"),
            ];
            &slots
        }
    };
    let took = took.map(size);
    let outcome: [&[Piece]; 2] = match &took {
        Some(level) => [&[Text("; takes ")], level],
        None => [&[Text("; stays")], &[]],
    };
    parts(&[
        &[Text("texture_pool: "), Text(path), Text(" wants ")],
        &size(want.wanted),
        &[Text(" (format "), Dec(want.format), Text(") from ")],
        &size(want.resident),
        pool,
        outcome[0],
        outcome[1],
    ]);
}

fn size(level: Level) -> [Piece<'static>; 5] {
    [Dec(level.width), Text("x"), Dec(level.height), Text("/"), Dec(level.mips)]
}

/// SKHID_TEXTURE_STREAMING_LOG as the game was started with it, read at the first request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Switch {
    Unread,
    Off,
    On,
}

static LOG: GameCell<Switch> = GameCell::new(Switch::Unread);

fn log_on(thread: &Thread) -> bool {
    let switch = match LOG.get(thread) {
        Switch::Unread => {
            let mut room = [0u8; 8];
            let len = crate::vostok::renderer::get_environment(thread, c"SKHID_TEXTURE_STREAMING_LOG", &mut room);
            let value = room.get(..len).unwrap_or(&[]);
            let read = if value.eq_ignore_ascii_case(b"on") || value == b"1" { Switch::On } else { Switch::Off };
            LOG.set(thread, read);
            read
        }
        known => known,
    };
    switch == Switch::On
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(width: u32, height: u32, mips: u32) -> Level {
        Level { width, height, mips }
    }

    #[test]
    fn the_sizes_between_largest_first() {
        let got: Vec<Level> = smaller_levels(level(64, 64, 5), level(2048, 2048, 10)).collect();
        assert_eq!(got, [level(1024, 1024, 9), level(512, 512, 8), level(256, 256, 7), level(128, 128, 6)]);
        let got: Vec<Level> = smaller_levels(level(256, 128, 6), level(2048, 1024, 9)).collect();
        assert_eq!(got, [level(1024, 512, 8), level(512, 256, 7)]);
    }

    #[test]
    fn none_for_one_mip_more_or_fewer() {
        assert_eq!(smaller_levels(level(1024, 1024, 9), level(2048, 2048, 10)).count(), 0);
        assert_eq!(smaller_levels(level(2048, 2048, 10), level(1024, 1024, 9)).count(), 0);
        assert_eq!(smaller_levels(level(512, 512, 8), level(512, 512, 8)).count(), 0);
    }
}

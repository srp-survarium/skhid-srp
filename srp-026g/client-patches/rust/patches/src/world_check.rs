//! Patch 87: a game parses every world it is sent (0xE8) before its own
//! reader takes it (`docs/notes/world_frame_check.md`). The parse is the
//! match server's own, `world_frame::Frame::parse_state`: the frame walks
//! from the front to exactly its end, no mask bit is past the game's 20
//! slots, and no section nor player state is over the game's buffer for
//! it. A world that does not parse would kill the game in its reader; it is
//! not loaded. Recovery requests a fresh generation; every refusal is logged
//! (`log::refusal`: at most a line a second, counting the ones between).
//!
//! The game checks against all 20 slots ([`Places::ALL`]): the roster of a
//! late join's growing match is not the game's to know. The server checks
//! against the roster.
#![forbid(unsafe_code)]

use world_frame::{Frame, Part, Places, Refusal};

use crate::vostok::breadcrumbs::{self, Crumb, Reading};
use crate::vostok::log::{refusal as log_refusal, Piece};
use crate::vostok::{Context, Flow, Thread, WorldCheckOnRawExit, WorldCheckOnUnpackedExit, WorldLoad};

/// What becomes of a world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    Load,
    Refused,
}

/// `world_load.unpacked`: before the load of an unpacked world (0x27C0D9).
pub fn on_unpacked(ctx: &mut Context<'_>) -> Flow<WorldCheckOnUnpackedExit> {
    match check(ctx.thread(), WorldLoad::at_unpacked(ctx)) {
        Verdict::Load => Flow::Continue,
        Verdict::Refused => Flow::Exit(WorldCheckOnUnpackedExit::Refused),
    }
}

/// `world_load.raw`: before the load of a raw world (0x27C111).
pub fn on_raw(ctx: &mut Context<'_>) -> Flow<WorldCheckOnRawExit> {
    match check(ctx.thread(), WorldLoad::at_raw(ctx)) {
        Verdict::Load => Flow::Continue,
        Verdict::Refused => Flow::Exit(WorldCheckOnRawExit::Refused),
    }
}

/// A world the site could not read is refused before the native load. A world that parses is
/// loaded, patch 85's guns its records name made first
/// (`weapon_pickup::before_world_load`: outside any tick, before the load).
fn check(thread: &Thread, load: Option<WorldLoad<'_>>) -> Verdict {
    let Some(load) = load else {
        log_refusal(thread, &[Piece::Text("world_check: a world at a load with no readable state; not loaded")]);
        return Verdict::Refused;
    };
    let parsed = Frame::parse_state(load.state(), Places::ALL);
    let state = load.state();
    breadcrumbs::record(load.thread(), Crumb::World {
        time: state.get(..4).map_or(0, |t| u32::from_le_bytes([t[0], t[1], t[2], t[3]])),
        bytes: state.len() as u32,
        packed: load.packed(),
        local: load.network_client().local_profile(),
        loaded: parsed.is_ok(),
    });
    breadcrumbs::reading(load.thread(), Reading::World);
    match parsed {
        Ok(frame) => {
            crate::weapon_pickup::before_world_load(&load, &frame);
            Verdict::Load
        }
        Err(refusal) => {
            let len = load.state().len();
            load.discard();
            log(thread, refusal, len);
            Verdict::Refused
        }
    }
}

/// The world's time, then its body: whether a game can load it.
pub fn verdict(state: &[u8]) -> Result<(), Refusal> {
    Frame::parse_state(state, Places::ALL).map(|_| ())
}

/// `srp: world_check: a world refused, <part> at <offset>: <reason> (<n> bytes); ...`.
fn log(thread: &Thread, refusal: Refusal, len: usize) {
    let slot = match refusal.part() {
        Part::Player(slot) => Piece::Dec(u32::from(slot.index())),
        _ => Piece::Text(""),
    };
    log_refusal(thread, &[
        Piece::Text("world_check: a world refused, "),
        Piece::Text(refusal.part().name()),
        Piece::Text(if matches!(refusal.part(), Part::Player(_)) { " " } else { "" }),
        slot,
        Piece::Text(" at "),
        Piece::Hex(refusal.at() as u32),
        Piece::Text(": "),
        Piece::Text(refusal.reason()),
        Piece::Text(" ("),
        Piece::Dec(len as u32),
        Piece::Text(" bytes); not loaded, a new one requested"),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_world_is_its_time_then_its_body() {
        // No players, six empty sections.
        let body = [0u8; 16];
        let mut state = 4500u32.to_le_bytes().to_vec();
        state.extend(body);
        assert_eq!(verdict(&state), Ok(()));
        // The body alone: its mask read as the time, then too short.
        assert!(verdict(&body).is_err());
        assert_eq!(verdict(&[1, 2, 3]), Err(Refusal::NoTime));
        // A mask bit past the game's 20 slots.
        state[4..8].copy_from_slice(&(1u32 << 20).to_le_bytes());
        assert!(matches!(verdict(&state), Err(Refusal::SlotPastPlaces { bit: 20, .. })));
    }
}

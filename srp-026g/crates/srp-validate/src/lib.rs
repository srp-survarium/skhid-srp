//! Allocation-free match-message validation shared by server and client.
//! Extracted from PR #478; runtime integration is a separate change.

#![no_std]
#![forbid(unsafe_code)]
// No panic is reachable in the game's build: every index and every sum is
// checked (the payload's link asserts the panic handler is gone).
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects
    )
)]

// The tests run under the std test harness; the crate itself stays no_std.
#[cfg(test)]
extern crate std;

pub mod memory;
pub mod messages;
pub mod recovery;
pub mod reader;
pub mod refusal;
pub mod report;
pub mod transport;
pub mod world;

pub use refusal::{Part, Reason, Refusal};

/// The roster places of a match: the game's place table holds 20
/// (client_world_check.md section 7), and the masks of a world name them.
pub const MAX_PLAYERS: usize = 20;

/// The player objects of a place, one for each kit ("preset").
pub const KITS: usize = 3;

/// The game modes (`maps::mode` in the server).
pub mod mode {
    /// Batteries.
    pub const BATTERIES: u8 = 2;
    /// Batteries placed over time.
    pub const TIMED_BATTERIES: u8 = 3;
    /// Team Deathmatch.
    pub const TEAM_DEATHMATCH: u8 = 4;
    /// Research (exe patch 53, `research_mode.md`).
    pub const RESEARCH: u8 = 5;
    /// The modes whose statistics and gather rule hold the map's batteries.
    pub const WITH_BATTERIES: [u8; 2] = [BATTERIES, TIMED_BATTERIES];
}

/// The timelimit rule's states (`rules::state` in the server); a world's
/// is at most [`IN_PROCESS`](timelimit::IN_PROCESS).
pub mod timelimit {
    pub const IN_PROCESS: u8 = 3;
}

/// Whether the f32 of these bits is a number (not infinite, not NaN). On
/// the bits, so that the game's build has no float code.
pub const fn finite(bits: u32) -> bool {
    bits & 0x7F80_0000 != 0x7F80_0000
}

/// An f32 that is a number (not infinite, not NaN), kept as its bits so
/// that the game's build has no float code until a caller asks for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Finite(u32);

impl Finite {
    pub const ZERO: Finite = Finite(0);

    /// The float of `bits`, if it is a number.
    pub const fn of_bits(bits: u32) -> Option<Finite> {
        if finite(bits) { Some(Finite(bits)) } else { None }
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub fn get(self) -> f32 {
        f32::from_bits(self.0)
    }
}

// The typed values of the bullets, the statistics and the rules.

/// Bit `n` of `mask` (none past 31).
pub const fn bit(mask: u32, n: u32) -> bool {
    match mask.checked_shr(n) {
        Some(rest) => rest & 1 != 0,
        None => false,
    }
}

/// The bits of `mask` at `n` and past.
pub const fn high(mask: u32, n: u32) -> u32 {
    match mask.checked_shr(n) {
        Some(rest) => rest,
        None => 0,
    }
}

/// An f32 over 0 that is a number: what the statistics' masks name (a
/// value is written only when `comiss` finds it over 0, 0x3353FB,
/// 0x334E3F). Its bits are never 0, so `Option<Positive>` is a u32.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Positive(core::num::NonZeroU32);

impl Positive {
    /// The float of `bits`, if it is a number over 0.
    pub const fn of_bits(bits: u32) -> Option<Positive> {
        // The sign clear, not +0, not infinite or NaN.
        if bits & 0x8000_0000 != 0 || !finite(bits) {
            return None;
        }
        match core::num::NonZeroU32::new(bits) {
            Some(bits) => Some(Positive(bits)),
            None => None,
        }
    }

    pub const fn bits(self) -> u32 {
        self.0.get()
    }

    pub fn get(self) -> f32 {
        f32::from_bits(self.0.get())
    }
}

/// A roster place: one of the [`MAX_PLAYERS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Place(u8);

impl Place {
    /// Place 0.
    pub const FIRST: Place = Place(0);

    /// The place `byte` names, if it is one.
    pub const fn new(byte: u8) -> Option<Place> {
        if (byte as usize) < MAX_PLAYERS {
            Some(Place(byte))
        } else {
            None
        }
    }

    /// A place or none: 0xFF is none (the game's `u8(-1)`).
    pub const fn or_none(byte: u8) -> Option<Option<Place>> {
        match byte {
            u8::MAX => Some(None),
            byte => match Place::new(byte) {
                Some(place) => Some(Some(place)),
                None => None,
            },
        }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// A set of roster places: a u32 with no bit at [`MAX_PLAYERS`] or past.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Places(u32);

impl Places {
    pub const EMPTY: Places = Places(0);

    /// The places of `mask`, if it names no bit past the twenty.
    pub const fn new(mask: u32) -> Option<Places> {
        if high(mask, MAX_PLAYERS as u32) == 0 {
            Some(Places(mask))
        } else {
            None
        }
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub const fn contains(self, place: Place) -> bool {
        bit(self.0, place.0 as u32)
    }

    /// The lowest place it names at `places` or past (the match has the
    /// first `places`).
    pub fn past(self, places: usize) -> Option<usize> {
        (places..MAX_PLAYERS).find(|&place| bit(self.0, place as u32))
    }

    /// Its places, lowest first.
    pub fn iter(self) -> impl Iterator<Item = Place> {
        (0..MAX_PLAYERS as u8)
            .filter(move |&place| bit(self.0, u32::from(place)))
            .map(Place)
    }
}

/// One of a match's two teams: 0 and 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Team {
    First,
    Second,
}

impl Team {
    pub const fn of(byte: u8) -> Option<Team> {
        match byte {
            0 => Some(Team::First),
            1 => Some(Team::Second),
            _ => None,
        }
    }

    /// A team, or 2 for neither (`rules::DRAW` in the server).
    pub const fn or_neither(byte: u8) -> Option<Option<Team>> {
        match byte {
            2 => Some(None),
            byte => match Team::of(byte) {
                Some(team) => Some(Some(team)),
                None => None,
            },
        }
    }
}

/// A bool as the game's bool writer puts it (0x80FA0: `neg al; sbb al,al`
/// at 0x8103D): 0x00 or 0xFF. `None`: any other byte.
pub const fn wire_bool(byte: u8) -> Option<bool> {
    match byte {
        0x00 => Some(false),
        0xFF => Some(true),
        _ => None,
    }
}

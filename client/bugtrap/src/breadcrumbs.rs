// The game's breadcrumbs as text: the ring the client patches keep in the
// exe's arena (srp-026g/client-patches/rust/patches/src/vostok/breadcrumbs.rs,
// which owns the layout: a 64-byte header, then 1024 entries of 32 bytes).
// report.rs finds and copies it out of the crashed game; this file only
// reads those bytes, with no Win32 and no allocation (tests/text_tests.rs).
use core::fmt::{self, Write};

/// The header's second word: its address xor this ("BRCS").
pub const CHECK: u32 = u32::from_le_bytes(*b"BRCS");
pub const HEADER: usize = 64;
pub const ENTRY: usize = 32;
pub const CAPACITY: usize = 1024;
/// The header and every entry: what report.rs copies.
pub const BYTES: usize = HEADER + ENTRY * CAPACITY;
/// The layout this file reads.
const VERSION: u16 = 1;

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}
fn u64_at(b: &[u8], at: usize) -> u64 {
    u32_at(b, at) as u64 | (u32_at(b, at + 4) as u64) << 32
}

/// A match's options as the game read them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub id: u32,
    pub map: u8,
    pub places: u8,
    pub level: u16,
    pub blackout: bool,
    pub replay: bool,
}

/// A recorded event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crumb {
    Match(Match),
    World {
        time: u32,
        bytes: u32,
        packed: bool,
        loaded: bool,
        local: Option<u8>,
    },
    Started,
    Unloaded,
    PlayerMade,
    PlayerGone {
        place: u8,
    },
    Death {
        place: u8,
        life: u32,
        time: u32,
    },
    Pickup {
        event: u8,
        place: u8,
        slot: u8,
        from: u32,
        kit: u32,
        time: u32,
    },
    /// A kind this reporter does not know (a newer game).
    Unknown(u8),
}
impl Crumb {
    /// The kinds, in the order of their numbers 1..=8 (counts per kind).
    pub const NAMES: [&'static str; 8] = [
        "match",
        "world",
        "started",
        "unloaded",
        "player-made",
        "player-gone",
        "death",
        "pickup",
    ];

    fn parse(e: &[u8]) -> Crumb {
        let (kind, a, b) = (e[4], e[5], u16_at(e, 6));
        let v = |i: usize| u32_at(e, 16 + 4 * i);
        match kind {
            1 => Crumb::Match(Match {
                id: v(0),
                map: v(1) as u8,
                places: v(2) as u8,
                level: b,
                blackout: v(3) != 0,
                replay: a != 0,
            }),
            2 => Crumb::World {
                time: v(0),
                bytes: v(1),
                packed: a & 1 != 0,
                loaded: a & 2 != 0,
                local: b.checked_sub(1).map(|p| p as u8),
            },
            3 => Crumb::Started,
            4 => Crumb::Unloaded,
            5 => Crumb::PlayerMade,
            6 => Crumb::PlayerGone { place: a },
            7 => Crumb::Death {
                place: a,
                life: v(0),
                time: v(1),
            },
            8 => Crumb::Pickup {
                event: a,
                place: b as u8,
                slot: (b >> 8) as u8,
                from: v(0),
                kit: v(1),
                time: v(2),
            },
            other => Crumb::Unknown(other),
        }
    }
}

/// weapon_pickup.rs's steps (vostok::breadcrumbs::Pickup), by number.
fn pickup_name(event: u8) -> &'static str {
    match event {
        1 => "switched",
        2 => "let go",
        3 => "back to its drop",
        4 => "restore FAILED",
        5 => "cancelled",
        6 => "let go, replaced",
        7 => "loaded record installed",
        8 => "loaded record NOT installed",
        9 => "loaded pending NOT registered",
        10 => "ignored a switch",
        11 => "waiting for the holster",
        _ => "?",
    }
}

/// What the game was reading last (the header's `reading`): stamped at a
/// world load's start and at each player state's start and end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reading {
    World,
    Player { place: u8, offset: u32 },
    PlayerRead { place: u8, offset: u32 },
}
impl Reading {
    fn parse(what: u32, offset: u32) -> Option<Reading> {
        let place = (what >> 8) as u8;
        match what & 0xFF {
            1 => Some(Reading::World),
            2 => Some(Reading::Player { place, offset }),
            3 => Some(Reading::PlayerRead { place, offset }),
            _ => None,
        }
    }
}

/// The ring as copied: its header's values and its entries' bytes.
pub struct Ring<'a> {
    /// Records made (the last [`CAPACITY`] are kept).
    pub next: u32,
    frequency: u64,
    /// The last match's options, and the local place at the last world load.
    pub last_match: Option<Match>,
    pub local: Option<u8>,
    /// What was being read last, and the counter then.
    pub reading: Option<(Reading, u64)>,
    entries: &'a [u8],
}
impl<'a> Ring<'a> {
    /// The ring copied from `at` (its header's own address): none when the
    /// header is not one this reporter reads.
    pub fn parse(bytes: &'a [u8], at: u32) -> Option<Ring<'a>> {
        if bytes.len() < BYTES
            || u32_at(bytes, 0) != at
            || u32_at(bytes, 4) != at ^ CHECK
            || u16_at(bytes, 8) != VERSION
            || u16_at(bytes, 10) as usize != ENTRY
            || u32_at(bytes, 12) as usize != CAPACITY
        {
            return None;
        }
        let flags = u32_at(bytes, 40);
        Some(Ring {
            next: u32_at(bytes, 16),
            frequency: u64_at(bytes, 24).max(1),
            last_match: (flags & 1 << 31 != 0).then(|| Match {
                id: u32_at(bytes, 32),
                map: bytes[36],
                places: bytes[37],
                level: u16_at(bytes, 38),
                blackout: flags & 1 != 0,
                replay: flags & 2 != 0,
            }),
            local: u32_at(bytes, 44).checked_sub(1).map(|p| p as u8),
            reading: Reading::parse(u32_at(bytes, 48), u32_at(bytes, 52))
                .map(|r| (r, u64_at(bytes, 56))),
            entries: &bytes[HEADER..BYTES],
        })
    }

    /// The kept entries, oldest first: (its counter, the event). One being
    /// written at the crash (its number not yet its own) is left out.
    pub fn entries(&self) -> impl Iterator<Item = (u64, Crumb)> + '_ {
        let next = self.next as usize;
        (next.saturating_sub(CAPACITY)..next).filter_map(move |n| {
            let e = &self.entries[(n % CAPACITY) * ENTRY..][..ENTRY];
            (u32_at(e, 0) as usize == n + 1).then(|| (u64_at(e, 8), Crumb::parse(e)))
        })
    }

    /// Seconds from `counter` to `now` (the counter at the crash).
    fn before(&self, now: u64, counter: u64) -> (u64, u32) {
        let ticks = now.saturating_sub(counter);
        (
            ticks / self.frequency,
            ((ticks % self.frequency) * 1000 / self.frequency) as u32,
        )
    }
}

fn write_match(w: &mut dyn Write, m: &Match) -> fmt::Result {
    write!(
        w,
        "{}{:#x}, map {}, {} places, level {}{}",
        if m.replay { "a replay, " } else { "" },
        // As the server's logs name it (`Match 0x...`): the options carry
        // it with its top bit set (match-server registry::static_id).
        m.id & 0x7FFF_FFFF,
        m.map,
        m.places,
        m.level,
        if m.blackout { ", Blackout" } else { "" }
    )
}

/// `Match:` the last match's options and the local place.
pub fn write_context(w: &mut dyn Write, ring: &Ring) -> fmt::Result {
    w.write_str("Match:     ")?;
    match &ring.last_match {
        Some(m) => write_match(w, m)?,
        None => w.write_str("none since the game started")?,
    }
    if let Some(place) = ring.local {
        write!(w, "; local place {place}")?;
    }
    w.write_str("\r\n")
}

/// `Reading:` a read the game began and had not finished, and how long
/// before `now`: a world before its players, or a player state before its
/// last native field. Nothing once the last player state read was finished
/// (a rollback's history loads stamp them all the time; a crash after one
/// is not inside it).
pub fn write_reading(w: &mut dyn Write, ring: &Ring, now: u64) -> fmt::Result {
    let Some((what, counter)) = ring.reading else {
        return Ok(());
    };
    match what {
        Reading::World => w.write_str("Reading:   a world (0xE8), before its players")?,
        Reading::Player { place, offset } => write!(
            w,
            "Reading:   place {place}'s player state, from byte {offset} of its stream"
        )?,
        Reading::PlayerRead { .. } => return Ok(()),
    }
    let (s, ms) = ring.before(now, counter);
    write!(w, ", {s}.{ms:03} s before the crash\r\n")
}

/// One event's line: seconds before the crash, its kind, what it says.
fn write_entry(
    w: &mut dyn Write,
    ring: &Ring,
    now: u64,
    counter: u64,
    crumb: Crumb,
) -> fmt::Result {
    let (s, ms) = ring.before(now, counter);
    write!(w, "  -{s}.{ms:03} s  ")?;
    match crumb {
        Crumb::Match(m) => {
            w.write_str("match: ")?;
            write_match(w, &m)?;
        }
        Crumb::World {
            time,
            bytes,
            packed,
            loaded,
            local,
        } => {
            write!(
                w,
                "world: {} ms, {} bytes{}, {}",
                time,
                bytes,
                if packed { " packed" } else { "" },
                if loaded { "loaded" } else { "REFUSED" }
            )?;
            if let Some(place) = local {
                write!(w, ", local place {place}")?;
            }
        }
        Crumb::Started => w.write_str("started: the countdown ended")?,
        Crumb::Unloaded => w.write_str("unloaded: the world and menus")?,
        Crumb::PlayerMade => w.write_str("player made")?,
        Crumb::PlayerGone { place } => write!(w, "player gone: place {place}")?,
        Crumb::Death { place, life, time } => {
            write!(w, "death: place {place}, life {life}, at {time} ms")?
        }
        Crumb::Pickup {
            event,
            place,
            slot,
            from,
            kit,
            time,
        } => write!(
            w,
            "pickup: place {place} slot {slot} {}: the gun of place {from} kit {kit}, at {time} ms",
            pickup_name(event)
        )?,
        Crumb::Unknown(kind) => write!(w, "kind {kind}")?,
    }
    w.write_str("\r\n")
}

/// `Events:` how many were recorded and kept, the kept ones per kind, and
/// the last `last` of them.
pub fn write_events(w: &mut dyn Write, ring: &Ring, now: u64, last: usize) -> fmt::Result {
    let mut counts = [0u32; Crumb::NAMES.len()];
    let mut kept = 0;
    for (_, crumb) in ring.entries() {
        kept += 1;
        let kind = match crumb {
            Crumb::Match(_) => 0,
            Crumb::World { .. } => 1,
            Crumb::Started => 2,
            Crumb::Unloaded => 3,
            Crumb::PlayerMade => 4,
            Crumb::PlayerGone { .. } => 5,
            Crumb::Death { .. } => 6,
            Crumb::Pickup { .. } => 7,
            Crumb::Unknown(_) => continue,
        };
        counts[kind] += 1;
    }
    write!(
        w,
        "Events:    {} recorded, the last {kept} kept:",
        ring.next
    )?;
    for (name, n) in Crumb::NAMES.iter().zip(counts) {
        if n != 0 {
            write!(w, " {name} {n}")?;
        }
    }
    if kept > last {
        write!(
            w,
            "\r\n           the last {last}, seconds before the crash:\r\n"
        )?;
    } else {
        w.write_str("\r\n           seconds before the crash:\r\n")?;
    }
    for (counter, crumb) in ring.entries().skip(kept.saturating_sub(last)) {
        write_entry(w, ring, now, counter, crumb)?;
    }
    Ok(())
}

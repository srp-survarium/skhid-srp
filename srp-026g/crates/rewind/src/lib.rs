//! Other players drawn in the past, and a bullet's hit rewound to what its
//! shooter saw (client patch 105, `docs/notes/past_poses.md`): the state
//! every game keeps alike for it, and its arithmetic. Pure Rust, no game
//! bindings: the client patch drives it; the match server reads its record
//! in a world (`survarium::v026g::match_msgs::motion`).
//!
//! - [`Ring`]: where a place's player stood at the last [`RING`] grid points.
//! - [`Declaration`]: the age a shooter's game declares, one byte of a kit
//!   choice (0xDE, relayed as 0xF3): 0..=2 are the kits, 3..=233 are ours.
//! - [`Declared`]: which age is in force for a place's shots, and when.
//! - [`Record`]: both, at the end of the place's player state in a world, so
//!   in every history item, the hash and an uploaded world.
//! - [`Path`]: how far back from where it is a target stood some ms ago,
//!   lerped between the ring's roots.
//!
//! Every game computes the same from the same world: times in integer ms,
//! f32 adds, subtracts, multiplies and divides only (SSE2, no FMA), the
//! game's floats kept bit for bit.

#![no_std]
#![forbid(unsafe_code)]

#[cfg(test)]
extern crate std;

pub type Vec3 = [f32; 3];

/// The world's grid, ms.
pub const TICK_MS: u32 = 50;
/// Grid roots a ring keeps: from a bullet's step's start back past the
/// cap's ceiling ([`Age::MAX`], three steps), one more for the step.
pub const RING: usize = 4;
/// A root further back than this is no root of now's (a world time set
/// back, a ring of another clock): the ring's reach and a step.
const REACH_MS: u32 = (RING as u32 + 1) * TICK_MS;

/// A time of the match's clock, ms. It wraps, as the game's does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Time(pub u32);

impl Time {
    /// The ms from `earlier` to this time; none when `earlier` is the later
    /// one (the wrapping difference read as signed: half the range either
    /// way).
    pub fn since(self, earlier: Time) -> Option<u32> {
        let ms = self.0.wrapping_sub(earlier.0);
        (ms <= i32::MAX as u32).then_some(ms)
    }

    /// Strictly after `earlier`.
    pub fn after(self, earlier: Time) -> bool {
        self.since(earlier).is_some_and(|ms| ms > 0)
    }

    /// On the 50 ms grid (atomic_tick's own test, `time % 50 == 0`).
    pub fn on_grid(self) -> bool {
        self.0.is_multiple_of(TICK_MS)
    }

    /// The ring's slot of a grid time. Across the clock's wrap (2^32 is no
    /// multiple of 50) two times may share a slot early: each root keeps its
    /// time, so a stale one is no root of now's.
    fn slot(self) -> usize {
        (self.0 / TICK_MS) as usize % RING
    }
}

/// A place of a match's roster, 0..20.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place(u8);

impl Place {
    pub const COUNT: u8 = 20;

    pub fn new(place: u8) -> Option<Place> {
        (place < Self::COUNT).then_some(Place(place))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// How old a target is drawn and tested, ms: 0..=[`Age::MAX`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Age(u8);

impl Age {
    pub const ZERO: Age = Age(0);
    /// The most any age is: the cap's ceiling, which the ring covers.
    pub const MAX: Age = Age(150);

    pub const fn new(ms: u32) -> Option<Age> {
        if ms <= Self::MAX.0 as u32 { Some(Age(ms as u8)) } else { None }
    }

    pub const fn ms(self) -> u32 {
        self.0 as u32
    }

    /// At most the match's cap.
    pub fn capped(self, cap: Cap) -> Age {
        self.min(cap.0)
    }
}

/// The match's cap on every age: srp's block in the static info (SRPX, tag
/// [`Cap::TAG`], one byte, ms); [`Cap::DEFAULT`] without one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cap(Age);

impl Cap {
    pub const DEFAULT: Cap = Cap(Age::MAX);
    /// Its entry's tag in srp's block (Blackout's is 2).
    pub const TAG: u8 = 3;

    /// The entry's bytes: one, the cap in ms; more than [`Age::MAX`] is
    /// [`Age::MAX`] (the ring reaches no further). None: not one byte.
    pub fn from_entry(entry: &[u8]) -> Option<Cap> {
        match entry {
            [ms] => Some(Cap(Age::new(u32::from(*ms)).unwrap_or(Age::MAX))),
            _ => None,
        }
    }

    pub fn age(self) -> Age {
        self.0
    }
}

/// One of the ages a declaration carries: [`Level::STEP_MS`] apart, 0..=150.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Level(u8);

impl Level {
    pub const STEP_MS: u32 = 15;
    /// 0, 15, .. 150.
    pub const COUNT: u8 = 11;

    /// The least level that is not younger than `age`.
    pub fn at_least(age: Age) -> Level {
        Level(age.ms().div_ceil(Self::STEP_MS) as u8)
    }

    pub fn age(self) -> Age {
        Age(self.0 * Self::STEP_MS as u8)
    }

    /// `steps` levels younger, none below 0.
    pub fn younger(self, steps: u8) -> Option<Level> {
        self.0.checked_sub(steps).map(Level)
    }
}

/// Whose age a declaration gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Every other place (version 1: the one age a game draws everyone at).
    Every,
    /// One place (a later version's: a target seen fresher by a direct route).
    Place(Place),
}

/// The age a shooter's game declares, in the byte of a kit choice (0xDE
/// `{tick, value}`, relayed as 0xF3 to every game and recorded in a replay):
/// `3 + target x 11 + level`, the target 0 for every place and 1 + p for
/// place p. 0..=2 are the kits (the place's tick 0x321790 takes those and
/// leaves its next kit for any other); 234..=255 are nobody's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Declaration {
    pub target: Target,
    pub level: Level,
}

impl Declaration {
    /// The first byte that is no kit.
    const FIRST: u8 = 3;

    pub fn every(level: Level) -> Declaration {
        Declaration { target: Target::Every, level }
    }

    /// The byte's declaration; none for a kit or a byte nobody sends.
    pub fn decode(value: u8) -> Option<Declaration> {
        let code = value.checked_sub(Self::FIRST)?;
        let (target, level) = (code / Level::COUNT, Level(code % Level::COUNT));
        let target = match target {
            0 => Target::Every,
            place => Target::Place(Place::new(place - 1)?),
        };
        Some(Declaration { target, level })
    }

    pub fn encode(self) -> u8 {
        let target = match self.target {
            Target::Every => 0,
            Target::Place(place) => place.0 + 1,
        };
        Self::FIRST + target * Level::COUNT + self.level.0
    }
}

/// The age in force for a place's shots: world state, the same in every game
/// (a declaration is an input, played at its tick in every game and replay,
/// and this is in the place's record of every world).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Declared {
    /// Nothing declared: every shot's age is 0 (patch 103's hits).
    #[default]
    Never,
    /// `now` for a shot fired after `at` (its declaration's tick), `before`
    /// for one fired at or before it, still in flight.
    Since { before: Age, now: Age, at: Time },
}

impl Declared {
    /// The one transition, once a grid step at its end: the place's newest
    /// kit choice at or before the step's time, if it has one (its time and
    /// byte). A declaration for every place in it is in force for shots after
    /// its time; found again at later steps, it changes nothing. A kit, a
    /// byte nobody sends or (in this version) one place's age: as it was.
    pub fn step(self, newest: Option<(Time, u8)>) -> Declared {
        let Some((at, value)) = newest else { return self };
        let Some(Declaration { target: Target::Every, level }) = Declaration::decode(value) else { return self };
        let age = level.age();
        match self {
            Declared::Never => Declared::Since { before: Age::ZERO, now: age, at },
            Declared::Since { now, at: since, .. } if at.after(since) => Declared::Since { before: now, now: age, at },
            Declared::Since { before, .. } => Declared::Since { before, now: age, at },
        }
    }

    /// The age of `target` for a shot fired at `fired`. One age for every
    /// target in this version; the target is what a later one's state looks up.
    pub fn age_for(self, fired: Time, _target: Place) -> Age {
        match self {
            Declared::Never => Age::ZERO,
            Declared::Since { before, now, at } => {
                if fired.after(at) {
                    now
                } else {
                    before
                }
            }
        }
    }
}

/// Where a place's player stood (his feet: the simulated transform's place,
/// game frame) at a grid point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Root {
    pub at: Time,
    pub feet: Vec3,
}

/// A place's player's roots at the last [`RING`] grid points, a slot each by
/// its grid index.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ring([Option<Root>; RING]);

impl Ring {
    pub const EMPTY: Ring = Ring([None; RING]);

    /// The player stood at `feet` at the end of the grid step to `at`
    /// (written once a step, before the world saves that point). Not a grid
    /// time: nothing.
    pub fn record(&mut self, at: Time, feet: Vec3) {
        if at.on_grid() {
            self.0[at.slot()] = Some(Root { at, feet });
        }
    }

    pub fn roots(&self) -> impl Iterator<Item = Root> + '_ {
        self.0.iter().flatten().copied()
    }
}

/// How far back from where it is now a target stood, `ms` before now:
/// now's feet less that time's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knot {
    pub ms: u32,
    pub back: Vec3,
}

impl Knot {
    /// Where it is: nothing back, 0 ms before now.
    pub const NOW: Knot = Knot { ms: 0, back: [0.0; 3] };
}

/// Where a target stood before now: knots, nearest first (ms ascending);
/// between two a lerp, beyond the last the last (no further back than the
/// ring, his life or the world's reach).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Path {
    knots: [Knot; RING + 1],
    len: usize,
}

impl Path {
    /// A target whose feet are `feet` at `now`: the nearest knot `first` (a
    /// frame's [`Knot::NOW`]; a bullet's step's start: its move in the step,
    /// the step's ms back), then each root of `ring` further back than it,
    /// within reach, after his life began at `life` (his spawn ms: a root
    /// of then may be his last life's).
    pub fn new(ring: &Ring, now: Time, feet: Vec3, first: Knot, life: Time) -> Path {
        let mut path = Path { knots: [first; RING + 1], len: 1 };
        for root in ring.roots() {
            let Some(ms) = now.since(root.at) else { continue };
            if ms <= first.ms || ms > REACH_MS || !root.at.after(life) {
                continue;
            }
            let back = [feet[0] - root.feet[0], feet[1] - root.feet[1], feet[2] - root.feet[2]];
            // in order of ms (distinct: one root a grid time)
            let mut at = path.len;
            while at > 1 && path.knots[at - 1].ms > ms {
                path.knots[at] = path.knots[at - 1];
                at -= 1;
            }
            path.knots[at] = Knot { ms, back };
            path.len += 1;
        }
        path
    }

    /// How far back the target stood `ms` before now: lerped between the
    /// knots either side; nearer than the first, the first's; beyond the
    /// last, the last's.
    pub fn back_at(&self, ms: f32) -> Vec3 {
        let mut newer = self.knots[0];
        // no number of ms, or nearer than the first: the first
        if ms.is_nan() || ms <= newer.ms as f32 {
            return newer.back;
        }
        for &older in &self.knots[1..self.len] {
            let older_ms = older.ms as f32;
            if ms <= older_ms {
                let newer_ms = newer.ms as f32;
                let share = (ms - newer_ms) / (older_ms - newer_ms);
                return core::array::from_fn(|i| newer.back[i] + (older.back[i] - newer.back[i]) * share);
            }
            newer = older;
        }
        newer.back
    }

    /// The furthest back the path reaches, ms.
    pub fn reach(&self) -> u32 {
        self.knots[self.len - 1].ms
    }
}

/// A place's state of this patch in a world: its player's ring and the age
/// in force for its shots, written after his player state's last field (and
/// patch 78's carry), so in every history item, the hash and an uploaded
/// world, and a late joiner or a resynced game holds it from its first tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Record {
    pub ring: Ring,
    pub declared: Declared,
}

impl Record {
    pub const EMPTY: Record = Record { ring: Ring::EMPTY, declared: Declared::Never };
    pub const VERSION: u8 = 1;
    /// version, the ring's mask, four roots {u32 time, f32 x, y, z}, the
    /// declared age's tag, before and now (ms) and its time.
    pub const SIZE: usize = 2 + RING * 16 + 7;
    const DECLARED_AT: usize = 2 + RING * 16;

    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        out[0] = Self::VERSION;
        for (slot, root) in self.ring.0.iter().enumerate() {
            let Some(root) = root else { continue };
            out[1] |= 1 << slot;
            let at = 2 + slot * 16;
            out[at..at + 4].copy_from_slice(&root.at.0.to_le_bytes());
            for (n, v) in root.feet.iter().enumerate() {
                out[at + 4 + 4 * n..at + 8 + 4 * n].copy_from_slice(&v.to_bits().to_le_bytes());
            }
        }
        if let Declared::Since { before, now, at } = self.declared {
            let d = Self::DECLARED_AT;
            out[d] = 1;
            out[d + 1] = before.0;
            out[d + 2] = now.0;
            out[d + 3..d + 7].copy_from_slice(&at.0.to_le_bytes());
        }
        out
    }

    /// The bytes [`Record::encode`] writes, and no others: none for another
    /// version, a root out of its slot or off the grid, an empty slot or a
    /// declared tag that is not zeros, an age over [`Age::MAX`].
    pub fn decode(bytes: &[u8; Self::SIZE]) -> Option<Record> {
        if bytes[0] != Self::VERSION || bytes[1] >> RING != 0 {
            return None;
        }
        let u32_at = |at: usize| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
        let mut ring = Ring::EMPTY;
        for slot in 0..RING {
            let at = 2 + slot * 16;
            if bytes[1] >> slot & 1 == 0 {
                if bytes[at..at + 16].iter().any(|&b| b != 0) {
                    return None;
                }
                continue;
            }
            let time = Time(u32_at(at));
            if !time.on_grid() || time.slot() != slot {
                return None;
            }
            let feet = core::array::from_fn(|n| f32::from_bits(u32_at(at + 4 + 4 * n)));
            ring.0[slot] = Some(Root { at: time, feet });
        }
        let d = Self::DECLARED_AT;
        let declared = match bytes[d] {
            0 if bytes[d + 1..].iter().all(|&b| b == 0) => Declared::Never,
            1 => Declared::Since {
                before: Age::new(u32::from(bytes[d + 1]))?,
                now: Age::new(u32::from(bytes[d + 2]))?,
                at: Time(u32_at(d + 3)),
            },
            _ => return None,
        };
        Some(Record { ring, declared })
    }
}

#[cfg(test)]
mod tests;

//! Where the Sponge spawns: only where the owner put it (2026-10-24). A
//! picked container of an anomaly spawns only the Sponge, the anomaly's
//! other containers its own kinds, an anomaly with no pick is 0.26g's. The
//! picks are content: each map's mode configs' `mode/artefact_spots`,
//! [{core (its full_name), index (in its artefact_containers), container,
//! kind}] (edit list artefact-spots-1, scripts/sponge/spots.py).
//!
//! Three places in the game (the patch's hooks):
//! - the anomaly cook's look-up of a core's `artefacts` (0x2E9D53): a core
//!   with picks reads a copy with {758, its picks} after its own kinds
//!   ([`picks`] counts them);
//! - the core's resolve_links: [`SpotTable::link`] keeps which of its
//!   containers are picked ([`Picks`]);
//! - spawn_artefact 0x31B6C0, the container chosen: [`spawn`] takes the
//!   artefact with 0.26g's draws.
//!
//! A core's spots are its world's: the first cook after a link starts a
//! new generation (a project's cores are all cooked before any is linked),
//! so the table holds the current world's picked cores only, the same in
//! every game whatever its history (a fresh one, a late joiner, a long session).

use core::num::NonZeroU32;

use crate::KIND;

/// Containers a core may have picked (an index of `artefact_containers`).
pub const CONTAINERS: u32 = 32;
/// Picked cores a world may have.
pub const CORES: usize = 64;
/// A core's `artefacts` table the cook may extend: 1 to 16 kinds.
pub const KINDS_MAX: u16 = 16;

/// A row of `mode/artefact_spots` as the config has it (`None` for a key
/// missing or not of its type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row<'a> {
    pub core: Option<&'a [u8]>,
    pub index: Option<u32>,
    pub kind: Option<u32>,
}

impl Row<'_> {
    /// The container it picks on core `name` with `containers` containers.
    fn picks(&self, name: &[u8], containers: u32) -> Option<u32> {
        let index = self.index?;
        (self.core? == name && self.kind? == KIND && index < containers && index < CONTAINERS).then_some(index)
    }
}

/// How many units of the Sponge the cook adds to core `name` with
/// `containers` containers (its picks; 0 for none).
pub fn picks<'a>(rows: impl IntoIterator<Item = Row<'a>>, name: &[u8], containers: u32) -> u32 {
    rows.into_iter().filter(|row| row.picks(name, containers).is_some()).count() as u32
}

/// A core's picked containers (a bit each).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picks(NonZeroU32);

impl Picks {
    /// The picks of core `name` with `containers` containers (after its links).
    pub fn of<'a>(rows: impl IntoIterator<Item = Row<'a>>, name: &[u8], containers: u32) -> Option<Picks> {
        let bits = rows.into_iter().filter_map(|row| row.picks(name, containers)).fold(0u32, |b, i| b | 1 << i);
        NonZeroU32::new(bits).map(Picks)
    }

    pub fn picked(self, index: usize) -> bool {
        index < CONTAINERS as usize && self.0.get() >> index & 1 != 0
    }
}

/// A core, by its address (an id).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Core(pub NonZeroU32);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    core: Core,
    generation: u32,
    picks: Picks,
}

/// A place of the table.
#[repr(C, u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Free,
    Taken(Entry),
}

/// Whether the current world's cores are being cooked or linked.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Cooking,
    Linked,
}

/// The current world's picked cores. `repr(C)`: check_sponge_026g.py reads
/// it (64 places of 16 bytes {tag, core, generation, picks}, then the generation).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpotTable {
    entries: [Place; CORES],
    generation: u32,
    phase: Phase,
}

impl Default for SpotTable {
    fn default() -> Self {
        SpotTable::new()
    }
}

impl SpotTable {
    pub const fn new() -> SpotTable {
        SpotTable { entries: [Place::Free; CORES], generation: 0, phase: Phase::Cooking }
    }

    /// A core is cooked: after a link, the next world's.
    pub fn cooking(&mut self) {
        if self.phase == Phase::Linked {
            self.generation = self.generation.wrapping_add(1);
            self.phase = Phase::Cooking;
        }
    }

    fn current(&self, place: &Place) -> Option<Entry> {
        match *place {
            Place::Taken(e) if e.generation == self.generation => Some(e),
            Place::Free | Place::Taken(_) => None,
        }
    }

    /// Core `core` linked with `picks`: its entry replaced, or gone. More
    /// picked cores than a world has keep none past [`CORES`].
    pub fn link(&mut self, core: Core, picks: Option<Picks>) {
        self.phase = Phase::Linked;
        let generation = self.generation;
        if let Some(at) = (0..CORES).find(|&i| self.current(&self.entries[i]).is_some_and(|e| e.core == core)) {
            self.entries[at] = Place::Free;
        }
        let Some(picks) = picks else { return };
        if let Some(at) = (0..CORES).find(|&i| self.current(&self.entries[i]).is_none()) {
            self.entries[at] = Place::Taken(Entry { core, generation, picks });
        }
    }

    /// The current world's picks of `core`.
    pub fn picks(&self, core: Core) -> Option<Picks> {
        self.entries.iter().find_map(|e| self.current(e).filter(|e| e.core == core)).map(|e| e.picks)
    }
}

/// An artefact of a core's pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Sponge,
    Other,
}

/// One of a core's pool: whether it is free (its game state 0) and its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub free: bool,
    pub kind: Kind,
}

/// What the chosen container may take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allowed {
    /// The core has no picks: 0.26g's choice among all its free artefacts.
    Any,
    /// A picked container: the Sponge only.
    Sponge,
    /// Another container of a core with picks: its own kinds only.
    Others,
}

impl Allowed {
    /// For the container at `index` of its core's containers (`None` past them).
    pub fn of(picks: Option<Picks>, index: Option<usize>) -> Allowed {
        match (picks, index) {
            (None, _) => Allowed::Any,
            (Some(picks), Some(index)) if picks.picked(index) => Allowed::Sponge,
            (Some(_), _) => Allowed::Others,
        }
    }

    fn takes(self, kind: Kind) -> bool {
        match self {
            Allowed::Any => true,
            Allowed::Sponge => kind == Kind::Sponge,
            Allowed::Others => kind == Kind::Other,
        }
    }
}

/// What spawn_artefact does at the chosen container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    /// No artefact free: 0.26g's way (the next delay), no third draw.
    None,
    /// The pool's artefact at this index.
    Take(usize),
    /// Free ones, none the container may take: nothing this time, the core
    /// kept due (spawn_artefact cleared +138h at 0x31B7C1 for a spawn that
    /// did not happen).
    Declined,
}

/// The core's random state after one draw (spawn_artefact's `imul
/// eax,eax,8088405h / inc eax`).
pub const fn draw(random: u32) -> u32 {
    random.wrapping_mul(0x0808_8405).wrapping_add(1)
}

/// spawn_artefact's third draw at a container allowed `allowed`, over the
/// `count` artefacts of the pool (`slot(i)`), its random state `random`:
/// the choice and the random state after (the same when no draw is made).
/// The draw is made whenever any artefact is free, as 0.26g makes it; the
/// pick is among those the container may take.
pub fn spawn(count: usize, slot: impl Fn(usize) -> Slot, allowed: Allowed, random: u32) -> (Choice, u32) {
    if !(0..count).any(|i| slot(i).free) {
        return (Choice::None, random);
    }
    let random = draw(random);
    let mut takes = (0..count).filter(|&i| {
        let s = slot(i);
        s.free && allowed.takes(s.kind)
    });
    let n = takes.clone().count();
    if n == 0 {
        return (Choice::Declined, random);
    }
    let taken = ((u64::from(random) * n as u64) >> 32) as usize;
    match takes.nth(taken) {
        Some(i) => (Choice::Take(i), random),
        None => (Choice::None, random),
    }
}

const _: () = assert!(core::mem::size_of::<Place>() == 16 && core::mem::size_of::<SpotTable>() == 16 * CORES + 8);

#[cfg(test)]
mod tests {
    use super::*;

    fn core(n: u32) -> Core {
        Core(NonZeroU32::new(n).unwrap())
    }

    const ROWS: [Row<'static>; 4] = [
        Row { core: Some(b"logic/a"), index: Some(0), kind: Some(KIND) },
        Row { core: Some(b"logic/a"), index: Some(3), kind: Some(KIND) },
        Row { core: Some(b"logic/a"), index: Some(1), kind: Some(54) },
        Row { core: Some(b"logic/a"), index: Some(9), kind: Some(KIND) },
    ];

    #[test]
    fn picks_count_valid_rows() {
        assert_eq!(picks(ROWS, b"logic/a", 4), 2);
        assert_eq!(picks(ROWS, b"logic/b", 4), 0);
        let p = Picks::of(ROWS, b"logic/a", 4).unwrap();
        assert!(p.picked(0) && p.picked(3) && !p.picked(1) && !p.picked(9));
        assert_eq!(Picks::of(ROWS, b"logic/a", 0), None);
    }

    #[test]
    fn table_is_the_current_worlds() {
        let mut t = SpotTable::new();
        let p = Picks::of(ROWS, b"logic/a", 4);
        t.cooking();
        t.link(core(0x100), p);
        assert_eq!(t.picks(core(0x100)), p);
        // linked again without spots (a later match's core at that address)
        t.link(core(0x100), None);
        assert_eq!(t.picks(core(0x100)), None);
        t.link(core(0x100), p);
        // the next world's cooks: the old entries are free
        t.cooking();
        assert_eq!(t.picks(core(0x100)), None);
        for n in 1..=(CORES as u32 + 5) {
            t.link(core(n), p);
        }
        assert_eq!(t.picks(core(CORES as u32)), p);
        assert_eq!(t.picks(core(CORES as u32 + 1)), None);
    }

    #[test]
    fn spawn_draws_as_the_original() {
        let pool = [
            Slot { free: true, kind: Kind::Other },
            Slot { free: false, kind: Kind::Other },
            Slot { free: true, kind: Kind::Sponge },
        ];
        let at = |i: usize| pool[i];
        // no picks: among both free ones, the draw's high part
        assert_eq!(spawn(3, at, Allowed::Any, 0), (Choice::Take(0), 1));
        assert_eq!(spawn(3, at, Allowed::Any, 0x8000_0000).0, Choice::Take(2));
        assert_eq!(spawn(3, at, Allowed::Sponge, 0), (Choice::Take(2), 1));
        assert_eq!(spawn(3, at, Allowed::Others, 0x8000_0000).0, Choice::Take(0));
        let taken = [Slot { free: true, kind: Kind::Other }, Slot { free: false, kind: Kind::Sponge }];
        assert_eq!(spawn(2, |i| taken[i], Allowed::Sponge, 5), (Choice::Declined, draw(5)));
        let none = [Slot { free: false, kind: Kind::Other }];
        assert_eq!(spawn(1, |i| none[i], Allowed::Any, 5), (Choice::None, 5));
    }
}


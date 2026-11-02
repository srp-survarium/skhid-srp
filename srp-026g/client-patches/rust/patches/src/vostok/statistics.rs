//! The match's statistics: a place's score events (patches 80 and 92) and the
//! batteries (patch 92). Every field is fields_026g.tsv's; this is what they
//! mean.
//!
//! A battery's record (victory_item_event_manager, the shared statistics'
//! +2760h; snapshot_rules.md section 3.2) is the game's 100h bytes, every one
//! of them the game's own. Patch 92 keeps each battery's [`Life`] (how near
//! it has come to each container, the steps its carriers hold) itself, and
//! serializes it after the record's own fields, so a rollback, a late joiner
//! and the server read it as every game has it.

use core::marker::PhantomData;

use super::context::Context;
use super::hook::Thread;
use super::world::{floats, read};
use crate::raw;

/// An event of `match_stats_events_dict_enum` (28 of them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoreEvent(u8);

impl ScoreEvent {
    /// A taken artefact, the one use_execute counts.
    pub const ARTEFACT: ScoreEvent = ScoreEvent(5);
    /// A delivery's share of a battery never delivered before, in
    /// hundredths of a delivery (the game's share_delivery_by_distance
    /// 0x335110 counts it at a put-in).
    pub const BRING_VICTORY_ITEM: ScoreEvent = ScoreEvent(6);
    /// 0.21d's winners' battery bonus: 0.26g counts it nowhere (its
    /// finish_match 0x2F4CF0 counts 11, 12 and 27 only; no site adds to it).
    pub const WINNING_TEAM_VI_BONUS: ScoreEvent = ScoreEvent(16);
}

/// A place of the match: the statistics hold 20 records.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Place(u8);

impl Place {
    const COUNT: usize = 20;

    fn parse(place: u8) -> Option<Place> {
        (usize::from(place) < Self::COUNT).then_some(Place(place))
    }
}

/// A kit: a record counts each event for three.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Kit(u8);

impl Kit {
    const COUNT: usize = 3;

    fn parse(kit: u8) -> Option<Kit> {
        (usize::from(kit) < Self::COUNT).then_some(Kit(kit))
    }
}

/// A place and the kit it plays: one of the 60 cells a record counts each
/// event in, numbered as the game numbers a battery's carried distances
/// (place x 3 + kit).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cell {
    place: Place,
    kit: Kit,
}

impl Cell {
    pub const COUNT: usize = Place::COUNT * Kit::COUNT;

    pub fn of(place: u8, kit: u8) -> Option<Cell> {
        Some(Cell { place: Place::parse(place)?, kit: Kit::parse(kit)? })
    }

    fn index(self) -> usize {
        usize::from(self.place.0) * Kit::COUNT + usize::from(self.kit.0)
    }

    fn at(index: usize) -> Option<Cell> {
        (index < Self::COUNT).then_some(Cell { place: Place((index / Kit::COUNT) as u8), kit: Kit((index % Kit::COUNT) as u8) })
    }
}

/// A player's cells of the match's statistics: its place's record, the
/// kit it plays.
pub(super) struct Score {
    statistics: raw::SharedStatistics,
    cell: Cell,
}

impl Score {
    /// The player's cells in `statistics`; None for a place or kit that is
    /// no record's.
    ///
    /// # Safety
    /// `player` is alive on the hook's thread.
    pub(super) unsafe fn of(statistics: raw::SharedStatistics, player: raw::Player) -> Option<Score> {
        // SAFETY: the caller's: the live player's place and kit bytes.
        let (place, kit) = unsafe { (read(player.place()), read(player.kit())) };
        Some(Score { statistics, cell: Cell::of(place, kit)? })
    }

    /// Counts `amount` of `event` for the place and kit as the game counts
    /// one (0x2F5606, 0x2F5610): its cell + `amount` (wrapping as the game's
    /// u16), then the listener told (place, event, amount) when it is set,
    /// so the scoreboard shows it at once.
    pub(super) fn count(&self, event: ScoreEvent, amount: u16) {
        let Cell { place, kit } = self.cell;
        let cell = usize::from(event.0) * Kit::COUNT + usize::from(kit.0);
        // SAFETY: the statistics are alive for the hook; the place is one of
        // the 20 records, the cell one of its 28 x 3.
        unsafe {
            let count = self.statistics.players().at(usize::from(place.0)).events().at(cell).count();
            count.write_volatile(read(count).wrapping_add(amount));
            let listener = self.statistics.listener();
            if read(listener.function()) != 0 {
                raw::engine_statistics_notify(Some(listener), u32::from(place.0), u32::from(event.0),
                                              u32::from(amount));
            }
        }
    }
}

/// A side of a battery match: whose container is the destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Team {
    Zero,
    One,
}

impl Team {
    fn parse(team: u32) -> Option<Team> {
        match team {
            0 => Some(Team::Zero),
            1 => Some(Team::One),
            _ => None,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Team::Zero => 0,
            Team::One => 1,
        }
    }
}

/// The team of `place` in the match's `options` (on_event's 0x334AD9).
///
/// # Safety
/// `options` are the live match's.
unsafe fn team_of(options: raw::MatchOptions, place: Place) -> Option<Team> {
    // SAFETY: the caller's; the place is one of the 20.
    Team::parse(unsafe { read(options.first_place().at(usize::from(place.0)).team()) })
}

/// How near a container a battery has come: whole steps, at most
/// [`Band::FARTHEST`] (the byte keeps 0 for "untouched").
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Band(u8);

impl Band {
    pub const FARTHEST: Band = Band(u8::MAX - 1);

    /// `steps` whole steps away, the farthest band beyond it.
    pub fn new(steps: u8) -> Band {
        Band(steps.min(Self::FARTHEST.0))
    }

    pub fn steps(self) -> u8 {
        self.0
    }
}

/// A battery's progress in its life: the nearest band it has come to each
/// team's container, by team; None until it is first carried in the life.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress(pub [Option<Band>; 2]);

impl Progress {
    /// Two bytes: 0 untouched, else the band + 1.
    fn parse(bytes: [u8; 2]) -> Progress {
        Progress(bytes.map(|byte| byte.checked_sub(1).map(Band)))
    }

    fn encode(self) -> [u8; 2] {
        self.0.map(|band| band.map_or(0, |band| band.0 + 1))
    }
}

/// The steps each cell has earned toward its own team's container in a
/// battery's life, held unpaid until the battery is delivered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held([u8; Cell::COUNT]);

impl Held {
    pub const NONE: Held = Held([0; Cell::COUNT]);

    /// `steps` more for `cell` (a life's steps are at most a band's 254).
    pub fn add(self, cell: Cell, steps: u8) -> Held {
        let mut held = self.0;
        held[cell.index()] = held[cell.index()].saturating_add(steps);
        Held(held)
    }

    /// The cells holding steps, by cell, and their steps.
    pub fn cells(&self) -> impl Iterator<Item = (Cell, u8)> + '_ {
        self.0.iter().enumerate().filter(|&(_, &steps)| steps > 0).filter_map(|(index, &steps)| Some((Cell::at(index)?, steps)))
    }

    /// `(cell, steps)` pairs: cells of the 60 in rising order, each with
    /// steps.
    fn parse(pairs: &[[u8; 2]]) -> Option<Held> {
        let mut held = Held::NONE;
        let mut last = None;
        for &[cell, steps] in pairs {
            let cell = Cell::at(usize::from(cell))?;
            if steps == 0 || last.is_some_and(|last| last >= cell) {
                return None;
            }
            held.0[cell.index()] = steps;
            last = Some(cell);
        }
        Some(held)
    }
}

/// A battery's life (from the match's start, or its last delivery): how
/// near it has come to each container and what its carriers hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Life {
    pub progress: Progress,
    pub held: Held,
}

impl Life {
    pub const NEW: Life = Life { progress: Progress([None; 2]), held: Held::NONE };

    /// The most bytes a life takes after its record.
    const MAX_BYTES: usize = 3 + 2 * Cell::COUNT;

    /// The progress's two bytes, the count of cells holding steps, then
    /// each one's (cell, steps), the cells in rising order
    /// (snapshot_rules.md section 3.2).
    fn encode(&self) -> ([u8; Self::MAX_BYTES], usize) {
        let mut bytes = [0; Self::MAX_BYTES];
        let [zero, one] = self.progress.encode();
        bytes[0] = zero;
        bytes[1] = one;
        let mut length = 3;
        for (cell, steps) in self.held.cells() {
            bytes[length] = cell.index() as u8;
            bytes[length + 1] = steps;
            length += 2;
            bytes[2] += 1;
        }
        (bytes, length)
    }
}

/// Which statistics' batteries: the address of their first record (an id,
/// never read through).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Batteries(u32);

impl Batteries {
    fn of(manager: raw::VictoryItemEventManager) -> Batteries {
        // SAFETY: the address of the manager's first record, not read.
        Batteries(unsafe { manager.records() }.address())
    }

    /// The battery whose record is `record`; None for a record that is
    /// none of these ten.
    fn battery(self, record: raw::VictoryItemStatus) -> Option<Battery> {
        let offset = usize::try_from(record.address().checked_sub(self.0)?).ok()?;
        let index = offset / raw::VictoryItemStatus::SIZE;
        (offset % raw::VictoryItemStatus::SIZE == 0 && index < BATTERIES).then_some(Battery { of: self, index })
    }
}

/// One of the ten batteries of some [`Batteries`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Battery {
    of: Batteries,
    index: usize,
}

impl Battery {
    pub fn of(self) -> Batteries {
        self.of
    }

    /// Its index, under [`BATTERIES`].
    pub fn index(self) -> usize {
        self.index
    }
}

/// A player carrying a battery, at `battery.carrier_tick`: which battery,
/// its cell, its team and where it stands against both containers.
pub struct BatteryCarry<'t> {
    battery: Battery,
    cell: Cell,
    team: Team,
    distances: [f32; 2],
    thread: PhantomData<&'t Thread>,
}

impl<'t> BatteryCarry<'t> {
    /// The player the site ticks, if it holds one of the match's batteries;
    /// None without a statistics handler or a battery rule (a mode without
    /// batteries), or for a place, kit or team that is none of the match's.
    pub fn at_tick(ctx: &'t Context<'_>) -> Option<BatteryCarry<'t>> {
        // SAFETY: the context is this site's: the player base_player::tick
        // ticks, its held item, world core and statistics alive on the
        // game's thread; the manager's rule, items and options are the ones
        // on_event reads (0x334AC0).
        unsafe {
            let player = raw::site::battery_carrier_tick::player(ctx)?;
            let held = player.held_item()?;
            let shared = player.world_core()?.statistics()?.shared();
            let manager = shared.victory_items();
            let rule = manager.rule()?;
            let first = rule.items_begin()?;
            let count = rule.items_end()?.index_from(first)?.min(BATTERIES);
            let index = (0..count).find(|&i| first.at(i).item().is_some_and(|item| item == held))?;
            let Score { cell, .. } = Score::of(shared, player)?;
            let team = team_of(manager.options()?, cell.place)?;
            let at: [f32; 16] = floats(player.transform());
            let position = [at[12], at[13], at[14]];
            let containers: [[f32; 3]; 2] = [floats(manager.container_0()), floats(manager.container_1())];
            Some(BatteryCarry {
                battery: Battery { of: Batteries::of(manager), index },
                cell,
                team,
                distances: containers.map(|container| distance(position, container)),
                thread: PhantomData,
            })
        }
    }

    pub fn battery(&self) -> Battery {
        self.battery
    }

    pub fn cell(&self) -> Cell {
        self.cell
    }

    pub fn team(&self) -> Team {
        self.team
    }

    /// Metres from the carrier to team 0's container and to team 1's.
    pub fn distances(&self) -> [f32; 2] {
        self.distances
    }
}

/// A battery put into a container, at `battery.delivered`: which battery,
/// whose container it is now in, and the cells its carriers are paid in.
pub struct Delivery<'t> {
    battery: Battery,
    into: Option<Team>,
    statistics: raw::SharedStatistics,
    options: Option<raw::MatchOptions>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Delivery<'t> {
    /// The put-in the site is at; None for a record that is none of the
    /// manager's ten.
    pub fn at(ctx: &'t Context<'_>) -> Option<Delivery<'t>> {
        use raw::site::battery_delivered as site;
        // SAFETY: on_event's record and manager (the shared statistics'
        // +2760h), alive on the game's thread; EAX the putter's team.
        unsafe {
            let manager = site::manager(ctx)?;
            Some(Delivery {
                battery: Batteries::of(manager).battery(site::record(ctx)?)?,
                into: Team::parse(site::team(ctx)),
                statistics: manager.statistics(),
                options: manager.options(),
                thread: PhantomData,
            })
        }
    }

    pub fn battery(&self) -> Battery {
        self.battery
    }

    /// The team whose container the battery is now in; None for a team
    /// that is neither (no game puts one in for it).
    pub fn container(&self) -> Option<Team> {
        self.into
    }

    /// The team of the place `cell` is.
    pub fn team_of(&self, cell: Cell) -> Option<Team> {
        // SAFETY: the manager's match options, alive with it.
        unsafe { team_of(self.options?, cell.place) }
    }

    /// Counts `amount` of `event` for `cell`.
    pub fn count(&self, cell: Cell, event: ScoreEvent, amount: u16) {
        Score { statistics: self.statistics, cell }.count(event, amount)
    }
}

/// The batteries' records: ten (the clear 0x3348A0's loop).
pub const BATTERIES: usize = 10;

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    ragdoll::math::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
}

/// At `battery.match_start`: the batteries whose lives begin.
pub fn started(ctx: &Context<'_>) -> Option<Batteries> {
    // SAFETY: start_match's manager.
    unsafe { raw::site::battery_match_start::manager(ctx) }.map(Batteries::of)
}

/// A battery's record being serialized, at `battery.save`.
pub struct Saving<'t> {
    record: raw::VictoryItemStatus,
    writer: raw::BufferWriter,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Saving<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<Saving<'t>> {
        // SAFETY: serialize's record and live writer.
        unsafe {
            let site = (raw::site::battery_save::record(ctx)?, raw::site::battery_save::writer(ctx)?);
            Some(Saving { record: site.0, writer: site.1, thread: PhantomData })
        }
    }

    /// Which of `batteries` it is; None for a record that is none of
    /// theirs.
    pub fn battery(&self, batteries: Batteries) -> Option<Battery> {
        batteries.battery(self.record)
    }

    /// Writes `life` after the record's own fields.
    pub fn write(self, life: &Life) {
        let (bytes, length) = life.encode();
        super::carry_runtime::write_bytes(self.writer, &bytes[..length]);
    }
}

/// At `battery.load`: the battery deserialized and its life as it was saved
/// (a new life when the bytes are not there or are no life's).
pub fn load(ctx: &Context<'_>) -> Option<(Battery, Life)> {
    use super::carry_runtime::read_bytes;
    use raw::site::battery_load as site;
    // SAFETY: deserialize's record, manager and live reader.
    let (battery, reader) = unsafe {
        (Batteries::of(site::manager(ctx)?).battery(site::record(ctx)?)?, site::reader(ctx))
    };
    let life = reader.and_then(|reader| {
        let [zero, one, count] = read_bytes::<3>(reader)?;
        let count = usize::from(count);
        if count > Cell::COUNT {
            return None;
        }
        let mut pairs = [[0; 2]; Cell::COUNT];
        for pair in &mut pairs[..count] {
            *pair = read_bytes::<2>(reader)?;
        }
        Some(Life { progress: Progress::parse([zero, one]), held: Held::parse(&pairs[..count])? })
    });
    Some((battery, life.unwrap_or(Life::NEW)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_and_kits_are_the_records() {
        assert_eq!(Place::parse(19), Some(Place(19)));
        assert_eq!(Place::parse(20), None);
        assert_eq!(Kit::parse(2), Some(Kit(2)));
        assert_eq!(Kit::parse(3), None);
        assert_eq!(Cell::of(19, 2).map(Cell::index), Some(59));
        assert_eq!(Cell::at(59), Cell::of(19, 2));
        assert_eq!(Cell::at(60), None);
        assert!((0..Cell::COUNT).all(|index| Cell::at(index).map(Cell::index) == Some(index)));
    }

    #[test]
    fn teams_are_zero_and_one() {
        assert_eq!(Team::parse(0), Some(Team::Zero));
        assert_eq!(Team::parse(1), Some(Team::One));
        assert_eq!(Team::parse(2), None);
    }

    #[test]
    fn progress_bytes_are_untouched_or_a_band_plus_one() {
        assert_eq!(Progress::parse([0, 0]), Progress::default());
        assert_eq!(Progress::parse([1, 255]), Progress([Some(Band(0)), Some(Band::FARTHEST)]));
        for byte in 0..=u8::MAX {
            assert_eq!(Progress::parse([byte, 0]).encode(), [byte, 0]);
        }
        assert_eq!(Band::new(u8::MAX), Band::FARTHEST);
    }

    #[test]
    fn a_life_is_its_progress_then_the_cells_holding_steps() {
        let cell = |place, kit| Cell::of(place, kit).unwrap();
        let held = Held::NONE.add(cell(3, 1), 4).add(cell(0, 0), 2).add(cell(3, 1), 1);
        let life = Life { progress: Progress::parse([5, 0]), held };
        let (bytes, length) = life.encode();
        assert_eq!(&bytes[..length], &[5, 0, 2, 0, 2, 10, 5]);
        assert_eq!(Held::parse(&[[0, 2], [10, 5]]), Some(held));
        assert_eq!(Life::NEW.encode().1, 3);
        // Out of order, twice, no steps, no cell: none is a life's.
        for pairs in [&[[10, 5], [0, 2]][..], &[[0, 2], [0, 2]], &[[0, 0]], &[[60, 1]]] {
            assert_eq!(Held::parse(pairs), None, "{pairs:?}");
        }
        let all = (0..Cell::COUNT).fold(Held::NONE, |held, index| held.add(Cell::at(index).unwrap(), 255));
        assert_eq!(Life { progress: Progress::default(), held: all }.encode().1, Life::MAX_BYTES);
    }

    #[test]
    fn a_distance_is_euclidean() {
        assert_eq!(distance([1.0, 2.0, 3.0], [4.0, 6.0, 3.0]), 5.0);
    }
}

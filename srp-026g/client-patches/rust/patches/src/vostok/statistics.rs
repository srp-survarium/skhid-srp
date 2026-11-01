//! The match's statistics: a place's score events (patches 80 and 92) and the
//! batteries' records (patch 92). Every field is fields_026g.tsv's; this is
//! what they mean.
//!
//! A battery's record (victory_item_event_manager, the shared statistics'
//! +2760h; snapshot_rules.md section 3.2) is the game's 100h bytes, of which
//! it writes +F0h..+FDh only. Patch 92 keeps its [`Progress`] in the last two
//! (+FEh, +FFh) and serializes them after the record's own fields, so a
//! rollback, a late joiner and the server read them as every game has them.

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Place(u8);

impl Place {
    fn parse(place: u8) -> Option<Place> {
        (place < 20).then_some(Place(place))
    }
}

/// A kit: a record counts each event for three.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Kit(u8);

impl Kit {
    const COUNT: usize = 3;

    fn parse(kit: u8) -> Option<Kit> {
        (usize::from(kit) < Self::COUNT).then_some(Kit(kit))
    }
}

/// A player's cells of the match's statistics: its place's record, the
/// kit it plays.
pub(super) struct Score {
    statistics: raw::SharedStatistics,
    place: Place,
    kit: Kit,
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
        Some(Score { statistics, place: Place::parse(place)?, kit: Kit::parse(kit)? })
    }

    /// The place, one of the 20 records.
    fn place(&self) -> usize {
        usize::from(self.place.0)
    }

    /// Counts `amount` of `event` for the place and kit as the game counts
    /// one (0x2F5606, 0x2F5610): its cell + `amount` (wrapping as the game's
    /// u16), then the listener told (place, event, amount) when it is set,
    /// so the scoreboard shows it at once.
    pub(super) fn count(&self, event: ScoreEvent, amount: u16) {
        let cell = usize::from(event.0) * Kit::COUNT + usize::from(self.kit.0);
        // SAFETY: the statistics are alive for the hook; the place is one of
        // the 20 records, the cell one of its 28 x 3.
        unsafe {
            let count = self.statistics.players().at(usize::from(self.place.0)).events().at(cell).count();
            count.write_volatile(read(count).wrapping_add(amount));
            let listener = self.statistics.listener();
            if read(listener.function()) != 0 {
                raw::engine_statistics_notify(Some(listener), u32::from(self.place.0), u32::from(event.0),
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

/// A battery's progress in its life (from its match's start, or its last
/// delivery): the nearest band it has come to each team's container, by
/// team; None until it is first carried in the life.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress(pub [Option<Band>; 2]);

impl Progress {
    /// The record's two bytes: 0 untouched, else the band + 1.
    fn parse(bytes: [u8; 2]) -> Progress {
        Progress(bytes.map(|byte| byte.checked_sub(1).map(Band)))
    }

    fn encode(self) -> [u8; 2] {
        self.0.map(|band| band.map_or(0, |band| band.0 + 1))
    }
}

/// A battery's record.
struct Record(raw::VictoryItemStatus);

impl Record {
    fn progress(&self) -> Progress {
        // SAFETY: the live record's own two bytes (Record is made only from
        // a live manager's record or a site's).
        Progress::parse(unsafe { [read(self.0.progress_0()), read(self.0.progress_1())] })
    }

    fn set_progress(&self, progress: Progress) {
        let [zero, one] = progress.encode();
        // SAFETY: as progress; the game's code never writes these two.
        unsafe {
            self.0.progress_0().write_volatile(zero);
            self.0.progress_1().write_volatile(one);
        }
    }
}

/// A player carrying a battery, at `battery.carrier_tick`: its team, where it
/// stands against both containers, the battery's record and its own score.
pub struct BatteryCarry<'t> {
    record: Record,
    score: Score,
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
            let score = Score::of(shared, player)?;
            let team = Team::parse(read(manager.options()?.first_place().at(score.place()).team()))?;
            let at: [f32; 16] = floats(player.transform());
            let position = [at[12], at[13], at[14]];
            let containers: [[f32; 3]; 2] = [floats(manager.container_0()), floats(manager.container_1())];
            Some(BatteryCarry {
                record: Record(manager.records().at(index)),
                score,
                team,
                distances: containers.map(|container| distance(position, container)),
                thread: PhantomData,
            })
        }
    }

    pub fn team(&self) -> Team {
        self.team
    }

    /// Metres from the carrier to team 0's container and to team 1's.
    pub fn distances(&self) -> [f32; 2] {
        self.distances
    }

    pub fn progress(&self) -> Progress {
        self.record.progress()
    }

    pub fn set_progress(&self, progress: Progress) {
        self.record.set_progress(progress)
    }

    /// Counts `amount` of `event` for the carrier.
    pub fn count(&self, event: ScoreEvent, amount: u16) {
        self.score.count(event, amount)
    }
}

/// The batteries' records: ten (the clear 0x3348A0's loop).
const BATTERIES: usize = 10;

fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    ragdoll::math::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2])
}

/// At `battery.match_start`: every record's progress untouched.
pub fn forget_all(ctx: &Context<'_>) {
    // SAFETY: start_match's manager, its ten records.
    let Some(manager) = (unsafe { raw::site::battery_match_start::manager(ctx) }) else { return };
    for index in 0..BATTERIES {
        // SAFETY: one of the manager's ten records.
        Record(unsafe { manager.records().at(index) }).set_progress(Progress::default());
    }
}

/// At `battery.delivered`: the delivered battery's progress untouched.
pub fn forget_delivered(ctx: &Context<'_>) {
    // SAFETY: on_event's record of the battery put in.
    if let Some(record) = unsafe { raw::site::battery_delivered::record(ctx) } {
        Record(record).set_progress(Progress::default());
    }
}

/// At `battery.save`: the record's progress after its own fields.
pub fn save(ctx: &Context<'_>) {
    // SAFETY: serialize's record and live writer.
    let (Some(record), Some(writer)) =
        (unsafe { raw::site::battery_save::record(ctx) }, unsafe { raw::site::battery_save::writer(ctx) })
    else {
        return;
    };
    super::carry_runtime::write_bytes(writer, &Record(record).progress().encode());
}

/// At `battery.load`: the record's progress as it was saved (untouched when
/// the bytes are not there).
pub fn load(ctx: &Context<'_>) {
    // SAFETY: deserialize's record.
    let Some(record) = (unsafe { raw::site::battery_load::record(ctx) }) else { return };
    // SAFETY: deserialize's live reader.
    let bytes = unsafe { raw::site::battery_load::reader(ctx) }.and_then(super::carry_runtime::read_bytes::<2>);
    Record(record).set_progress(bytes.map_or(Progress::default(), Progress::parse));
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
    fn a_distance_is_euclidean() {
        assert_eq!(distance([1.0, 2.0, 3.0], [4.0, 6.0, 3.0]), 5.0);
    }
}

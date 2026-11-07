//! The statistics section (`shared_statistics::serialize` 0x2F5AC0,
//! snapshot_rules.md section 3): a record for each roster place, one for
//! each battery in modes 2 and 3, the 11-byte tail, read into
//! [`Statistics`]. The writers pass each field's expression as text, so the
//! names are the game's own (PROVEN: `.\player_shared_statistics.cpp`
//! 0xA19ED0.., `.\victory_item_event_manager.cpp` 0xA19D68..).
//!
//! A place ([`PlayerStatistics`], 0x335750):
//!
//! ```text
//! 3 x (preset p)  u32 events_mask, then u8( events[e][p] ) for each set
//!                 one-byte event (table 0x920588), u16 for each two-byte
//!                 one (0x9205E0)
//! 3 x (preset p)  u32 inflictors_mask, f32 received_damage[i][p] each bit
//! u32 receivers_mask, u32 last_hit_ms[i] each bit
//! u32 last_killed_enemy_ms, u8 last_hit_enemy_id, u8 u8( killstreak ),
//! f32 newbee_bonus_player_action_coef, u16 longest_killstreak
//! ```
//!
//! A battery ([`BatteryStatus`], 0x334DC0, then exe patch 92's state):
//!
//! ```text
//! 3 x (preset p)  u32 players_mask, f32 carried_distances[j][p] each bit
//! f32 nearest_positions[team_1], [team_2]
//! u8 owner_team, u8 first_finder
//! u8 x2 progress, u8 held count (bit 7: the last taker follows),
//! [u8 last taker], held count x (u8 cell, u8 steps)
//! ```
//!
//! The tail ([`Tail`], 0x2F5B08..0x2F5BCB): u16 x2 `m_team_points`, u8 x2
//! `m_team_victory_items`, u8 `m_winner_team`, u32
//! `m_active_in_match_players_mask`.
//!
//! The domains, each read once:
//!
//! - an events mask names the 28 events only, and holds every bit of the
//!   preset's before it: the writer clears it once (0x33575D) and each
//!   preset only sets bits (0x335778..0x3357A9) (PROVEN);
//! - a damage or a carried distance is a number over 0, and a last hit
//!   time is not 0: the writer sets a bit only for a value over 0
//!   (`comiss` 0x3353FB, 0x334E3F; `cmp .., 0` / `jbe` 0x3354C1) (PROVEN);
//! - the masks name the 20 places (a battery's, the match's places, as
//!   before);
//! - `last_hit_enemy_id`: a place of the match or 0xFF;
//! - the newbie bonus is a number;
//! - `owner_team`: a team, or 3 never delivered (the reset 0x3348D1 sets 3,
//!   a delivery the deliverer's team, 0x334CEA) (PROVEN for the stores;
//!   that a battery mode's players are of teams 0 and 1 only, LIKELY);
//! - `first_finder` and the last taker: a place or 0xFF (the constructor's
//!   0xFF, then a player's id, 0x334B3A, 0x334B4D; not reset between
//!   matches, so a place of an earlier match: bounded by the 20, not the
//!   match);
//! - the held steps: at most 60 cells (place x 3 + kit), rising, each with
//!   a step (patch 92);
//! - `m_winner_team`: a team, 2 a draw, 3 until the first event, 0xFF
//!   before the handler's match init (0x2F4BCD, 0x26D95A) (PROVEN for the
//!   stores).
//!
//! Open: the event counts (a one-byte one wraps, 0x3358E6), the kill
//! streaks, the two nearest positions (a ratio of distances, 0x334C19,
//! which the game can make no number), patch 92's progress, the tail's
//! points and its active mask (written by nothing in modes 2 and 3).
//!
//! The server's own decoder of this section, for the results, is
//! `snapshot::Statistics` (crates/survarium).

use core::num::{NonZeroU8, NonZeroU32};

use super::{Context, Section, battery_records};
use crate::reader::{Reader, Short};
use crate::{Finite, KITS, MAX_PLAYERS, Part, Place, Places, Positive, Reason, Refusal, Team};

/// The score events a record's masks cover (`match_stats_events_dict_enum`).
pub const EVENTS: usize = 28;
/// The events carried as u16 (table 0x9205E0: 6, 17, 18, 22, 23, 26); the
/// others are one byte (table 0x920588), each list in event order.
const WORD_EVENTS: u32 = 1 << 6 | 1 << 17 | 1 << 18 | 1 << 22 | 1 << 23 | 1 << 26;
/// The batteries a match has at most (the gather rule's u16 mask).
pub const MAX_BATTERIES: usize = 16;
/// A battery's held-step cells: place x 3 + kit.
const CELLS: usize = MAX_PLAYERS * KITS;

/// Why a record does not read: it runs short, a mask names more than its
/// values (the section's length is then not the match's), or a field is
/// out of its domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fault {
    Unread,
    Domain(Reason),
}

impl From<Short> for Fault {
    fn from(_: Short) -> Self {
        Fault::Unread
    }
}

/// A preset's events: the counts of the events its mask names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Events {
    mask: u32,
    counts: [u16; EVENTS],
}

impl Events {
    /// `events_mask`: the events with a count in this preset or one before.
    pub fn mask(&self) -> u32 {
        self.mask
    }

    /// `events[event][preset]` (0 when the mask does not name it; a
    /// one-byte event's count wraps at 256); `None` past the 28 events.
    pub fn count(&self, event: usize) -> Option<u16> {
        self.counts.get(event).copied()
    }
}

/// Values by place, each over 0, the ones a mask names.
pub type ByPlace<T> = [Option<T>; MAX_PLAYERS];

/// One roster place's statistics (`player_shared_statistics`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerStatistics {
    events: [Events; KITS],
    received_damage: [ByPlace<Positive>; KITS],
    last_hit_ms: ByPlace<NonZeroU32>,
    last_killed_enemy_ms: u32,
    last_hit_enemy: Option<Place>,
    killstreak: u8,
    newbee_bonus_player_action_coef: Finite,
    longest_killstreak: u16,
}

impl PlayerStatistics {
    const NONE: Self = Self {
        events: [Events {
            mask: 0,
            counts: [0; EVENTS],
        }; KITS],
        received_damage: [[None; MAX_PLAYERS]; KITS],
        last_hit_ms: [None; MAX_PLAYERS],
        last_killed_enemy_ms: 0,
        last_hit_enemy: None,
        killstreak: 0,
        newbee_bonus_player_action_coef: Finite::ZERO,
        longest_killstreak: 0,
    };

    /// The record of `place` at `reader` (0x335750), or why not.
    fn read(reader: &mut Reader, place: usize) -> Result<Self, Fault> {
        let mut read = Self::NONE;
        let mut before = 0u32;
        for events in &mut read.events {
            let mask = reader.u32()?;
            if crate::high(mask, EVENTS as u32) != 0 {
                return Err(Fault::Unread);
            }
            if mask & before != before {
                return Err(Fault::Domain(Reason::StatisticsField {
                    record: "player",
                    index: place,
                    field: "events_mask (drops a bit of the preset before)",
                }));
            }
            before = mask;
            events.mask = mask;
            // The one-byte events in event order, then the two-byte ones.
            for word in [false, true] {
                for (event, count) in events.counts.iter_mut().enumerate() {
                    let event = event as u32;
                    if !crate::bit(mask, event) || crate::bit(WORD_EVENTS, event) != word {
                        continue;
                    }
                    *count = if word {
                        reader.u16()?
                    } else {
                        reader.u8()?.into()
                    };
                }
            }
        }
        let damage = |field| {
            Fault::Domain(Reason::StatisticsField {
                record: "player",
                index: place,
                field,
            })
        };
        for received in &mut read.received_damage {
            *received = by_place(
                reader,
                MAX_PLAYERS,
                || Fault::Domain(Reason::DamageNotNumber { place }),
                || damage("received_damage (not over 0)"),
            )?;
        }
        let receivers = Places::new(reader.u32()?).ok_or(Fault::Unread)?;
        for receiver in receivers.iter() {
            let time = reader.u32()?;
            let slot = read
                .last_hit_ms
                .get_mut(receiver.index())
                .ok_or(Fault::Unread)?;
            *slot = Some(NonZeroU32::new(time).ok_or_else(|| damage("last_hit_ms (0)"))?);
        }
        read.last_killed_enemy_ms = reader.u32()?;
        let enemy = reader.u8()?;
        read.killstreak = reader.u8()?;
        let coef = reader.f32_bits()?;
        read.longest_killstreak = reader.u16()?;
        read.newbee_bonus_player_action_coef =
            Finite::of_bits(coef).ok_or(Fault::Domain(Reason::DamageNotNumber { place }))?;
        read.last_hit_enemy = Place::or_none(enemy).ok_or(Fault::Domain(Reason::LastHitPast {
            place,
            enemy,
            places: MAX_PLAYERS,
        }))?;
        Ok(read)
    }

    /// Preset `kit`'s events.
    pub fn events(&self, kit: usize) -> Option<&Events> {
        self.events.get(kit)
    }

    /// `received_damage[inflictor][kit]`, by inflictor.
    pub fn received_damage(&self, kit: usize) -> Option<&ByPlace<Positive>> {
        self.received_damage.get(kit)
    }

    /// `last_hit_ms[receiver]`: when it last hit each player.
    pub fn last_hit_ms(&self) -> &ByPlace<NonZeroU32> {
        &self.last_hit_ms
    }

    pub fn last_killed_enemy_ms(&self) -> u32 {
        self.last_killed_enemy_ms
    }

    /// `last_hit_enemy_id`; 0xFF is none.
    pub fn last_hit_enemy(&self) -> Option<Place> {
        self.last_hit_enemy
    }

    /// `u8( killstreak )`.
    pub fn killstreak(&self) -> u8 {
        self.killstreak
    }

    pub fn newbee_bonus_player_action_coef(&self) -> Finite {
        self.newbee_bonus_player_action_coef
    }

    pub fn longest_killstreak(&self) -> u16 {
        self.longest_killstreak
    }
}

/// A u32 mask over the first `count` places (a bit past them: the record
/// does not read), then each set bit's value, a number over 0.
fn by_place(
    reader: &mut Reader,
    count: usize,
    not_number: impl Fn() -> Fault,
    not_positive: impl Fn() -> Fault,
) -> Result<ByPlace<Positive>, Fault> {
    let mask = reader.u32()?;
    if crate::high(mask, count.min(MAX_PLAYERS) as u32) != 0 {
        return Err(Fault::Unread);
    }
    let mut values = [None; MAX_PLAYERS];
    for (index, value) in values.iter_mut().enumerate() {
        if !crate::bit(mask, index as u32) {
            continue;
        }
        let bits = reader.f32_bits()?;
        if Finite::of_bits(bits).is_none() {
            return Err(not_number());
        }
        *value = Some(Positive::of_bits(bits).ok_or_else(&not_positive)?);
    }
    Ok(values)
}

/// Who a battery was delivered for last (`owner_team`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerTeam {
    /// 3: not delivered in its life.
    Never,
    Team(Team),
}

/// Whether a battery's record carries the native last taker (patch 92's
/// bit 7; #1155, `battery_taker.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LastTaker {
    NotRecorded,
    /// The place that last took it out of a container, or none.
    Saved(Option<Place>),
}

/// One battery's status (`victory_item_event_manager`'s record).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BatteryStatus {
    carried_distances: [ByPlace<Positive>; KITS],
    nearest_positions: [u32; 2],
    owner_team: OwnerTeam,
    first_finder: Option<Place>,
    progress: [u8; 2],
    last_taker: LastTaker,
    held_steps: [Option<NonZeroU8>; CELLS],
}

impl BatteryStatus {
    const NONE: Self = Self {
        carried_distances: [[None; MAX_PLAYERS]; KITS],
        nearest_positions: [0; 2],
        owner_team: OwnerTeam::Never,
        first_finder: None,
        progress: [0; 2],
        last_taker: LastTaker::NotRecorded,
        held_steps: [None; CELLS],
    };

    /// Battery `number`'s record of a match of `places` places, or why not.
    fn read(reader: &mut Reader, places: usize, number: usize) -> Result<Self, Fault> {
        let domain = |field| {
            Fault::Domain(Reason::StatisticsField {
                record: "battery",
                index: number,
                field,
            })
        };
        let mut read = Self::NONE;
        for carried in &mut read.carried_distances {
            *carried = by_place(
                reader,
                places,
                || domain("carried_distances (no number)"),
                || domain("carried_distances (not over 0)"),
            )?;
        }
        read.nearest_positions = [reader.f32_bits()?, reader.f32_bits()?];
        let owner = reader.u8()?;
        let finder = reader.u8()?;
        read.progress = reader.array::<2>()?;
        let count = reader.u8()?;
        read.last_taker = if count & 0x80 == 0 {
            LastTaker::NotRecorded
        } else {
            LastTaker::Saved(Place::or_none(reader.u8()?).ok_or(Fault::Unread)?)
        };
        let count = usize::from(count & 0x7F);
        if count > CELLS {
            return Err(Fault::Unread);
        }
        let mut previous = None;
        for _ in 0..count {
            let cell = reader.u8()?;
            let steps = NonZeroU8::new(reader.u8()?).ok_or(Fault::Unread)?;
            if previous.is_some_and(|last| last >= cell) {
                return Err(Fault::Unread);
            }
            previous = Some(cell);
            let slot = read
                .held_steps
                .get_mut(usize::from(cell))
                .ok_or(Fault::Unread)?;
            *slot = Some(steps);
        }
        read.owner_team = match owner {
            3 => OwnerTeam::Never,
            team => OwnerTeam::Team(Team::of(team).ok_or_else(|| domain("owner_team"))?),
        };
        read.first_finder = Place::or_none(finder).ok_or_else(|| domain("first_finder"))?;
        Ok(read)
    }

    /// `item_status.carried_distances[player][kit]`, by player.
    pub fn carried_distances(&self, kit: usize) -> Option<&ByPlace<Positive>> {
        self.carried_distances.get(kit)
    }

    /// `item_status.nearest_positions[team_1]`, `[team_2]`: the f32s'
    /// bits, open (the game can write ones that are no number).
    pub fn nearest_positions(&self) -> [u32; 2] {
        self.nearest_positions
    }

    pub fn owner_team(&self) -> OwnerTeam {
        self.owner_team
    }

    /// `item_status.first_finder`; 0xFF is none.
    pub fn first_finder(&self) -> Option<Place> {
        self.first_finder
    }

    /// Patch 92: how near each team's container the battery came in its
    /// life, in 10 m steps (0 not carried yet, else the steps + 1).
    pub fn progress(&self) -> [u8; 2] {
        self.progress
    }

    pub fn last_taker(&self) -> LastTaker {
        self.last_taker
    }

    /// Patch 92: the steps each carrier cell (place x 3 + kit) holds.
    pub fn held_steps(&self) -> &[Option<NonZeroU8>; CELLS] {
        &self.held_steps
    }
}

/// The leader `m_winner_team` names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Winner {
    Team(Team),
    /// 2.
    Draw,
    /// 3: no rule event yet.
    Undecided,
    /// 0xFF: the handler's match init has not run.
    Unset,
}

/// The statistics' last 11 bytes: the standing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tail {
    team_points: [u16; 2],
    team_victory_items: [u8; 2],
    winner_team: Winner,
    active_in_match_players_mask: u32,
}

impl Tail {
    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        let team_points = [reader.u16()?, reader.u16()?];
        let team_victory_items = reader.array::<2>()?;
        let winner = reader.u8()?;
        let active_in_match_players_mask = reader.u32()?;
        let winner_team = match winner {
            2 => Winner::Draw,
            3 => Winner::Undecided,
            u8::MAX => Winner::Unset,
            team => Winner::Team(Team::of(team).ok_or(Fault::Domain(Reason::StatisticsField {
                record: "tail",
                index: 0,
                field: "m_winner_team",
            }))?),
        };
        Ok(Self {
            team_points,
            team_victory_items,
            winner_team,
            active_in_match_players_mask,
        })
    }

    /// `m_team_points`: batteries, tickets or research points by mode.
    pub fn team_points(&self) -> [u16; 2] {
        self.team_points
    }

    /// `m_team_victory_items`.
    pub fn team_victory_items(&self) -> [u8; 2] {
        self.team_victory_items
    }

    pub fn winner_team(&self) -> Winner {
        self.winner_team
    }

    /// `m_active_in_match_players_mask`: mode 4 only.
    pub fn active_in_match_players_mask(&self) -> u32 {
        self.active_in_match_players_mask
    }
}

/// The statistics section, its records borrowed from the world: the
/// places', the batteries', the tail. Each record is read into its type
/// (and its domains held) by [`Statistics::read`]; the view keeps where
/// each one is and hands them out one by one, read again from their bytes,
/// so that a check holds one record at a time (the game's arena runs this
/// on the world loader's stack).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Statistics<'a> {
    players: [&'a [u8]; MAX_PLAYERS],
    places: usize,
    batteries: [&'a [u8]; MAX_BATTERIES],
    battery_count: usize,
    tail: Tail,
}

impl<'a> Statistics<'a> {
    /// The section of a match of `places` places and `batteries` battery
    /// records, read to its exact end, or why not.
    pub fn read(section: &Section<'a>, places: usize, batteries: usize) -> Result<Self, Refusal> {
        let refuse = |reason| Refusal::new(Part::Statistics, section.at, reason);
        if section.bytes.is_empty() {
            return Err(refuse(Reason::StatisticsEmpty));
        }
        let unread = refuse(Reason::StatisticsUnread {
            len: section.bytes.len(),
            places,
            batteries,
        });
        if places > MAX_PLAYERS || batteries > MAX_BATTERIES {
            return Err(unread);
        }
        let fault = |fault| match fault {
            Fault::Unread => unread,
            Fault::Domain(reason) => refuse(reason),
        };
        let bytes = section.bytes;
        let mut reader = Reader::new(bytes, 0);
        // The bytes from `from` to the reader.
        let record =
            |reader: &Reader, from: usize| bytes.get(from..reader.at()).unwrap_or_default();
        let mut players: [&[u8]; MAX_PLAYERS] = [&[]; MAX_PLAYERS];
        for (place, player) in players.iter_mut().take(places).enumerate() {
            let from = reader.at();
            PlayerStatistics::read(&mut reader, place).map_err(fault)?;
            *player = record(&reader, from);
        }
        let mut statuses: [&[u8]; MAX_BATTERIES] = [&[]; MAX_BATTERIES];
        for (number, status) in statuses.iter_mut().take(batteries).enumerate() {
            let from = reader.at();
            BatteryStatus::read(&mut reader, places, number).map_err(fault)?;
            *status = record(&reader, from);
        }
        let tail = Tail::read(&mut reader).map_err(fault)?;
        if !reader.is_done() {
            return Err(unread);
        }
        Ok(Self {
            players,
            places,
            batteries: statuses,
            battery_count: batteries,
            tail,
        })
    }

    /// The places' records, by roster place.
    pub fn players(&self) -> impl Iterator<Item = PlayerStatistics> + 'a {
        let records = self.players;
        (0..self.places).filter_map(move |place| {
            let bytes = records.get(place)?;
            PlayerStatistics::read(&mut Reader::new(bytes, 0), place).ok()
        })
    }

    /// The batteries' records, by battery number; none outside modes 2
    /// and 3.
    pub fn batteries(&self) -> impl Iterator<Item = BatteryStatus> + 'a {
        let (records, places) = (self.batteries, self.places);
        (0..self.battery_count).filter_map(move |number| {
            let bytes = records.get(number)?;
            BatteryStatus::read(&mut Reader::new(bytes, 0), places, number).ok()
        })
    }

    pub fn tail(&self) -> Tail {
        self.tail
    }
}

/// The statistics read ([`Statistics::read`]), and each place's last hit
/// enemy a place of the match.
pub fn statistics(section: &Section, context: &Context) -> Result<(), Refusal> {
    let places = context.places;
    let read = Statistics::read(section, places, battery_records(context))?;
    for (place, player) in read.players().enumerate() {
        if let Some(enemy) = player.last_hit_enemy()
            && enemy.index() >= places
        {
            return Err(Refusal::new(
                Part::Statistics,
                section.at,
                Reason::LastHitPast {
                    place,
                    enemy: enemy.get(),
                    places,
                },
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    /// A place's record: kit 0 has 2 kills (event 0) and 30 bullets fired
    /// (event 22, a u16), kits 1 and 2 keep the mask; damage 12.5 from
    /// place 1 to kit 0; a hit on place 1 at 9000 ms.
    fn player(enemy: u8) -> Vec<u8> {
        let mut out = Vec::new();
        let mask = 1u32 | 1 << 22;
        for kit in 0..3 {
            out.extend(mask.to_le_bytes());
            out.push(if kit == 0 { 2 } else { 0 });
            out.extend(if kit == 0 { 30u16 } else { 0 }.to_le_bytes());
        }
        out.extend(0b10u32.to_le_bytes());
        out.extend(12.5f32.to_le_bytes());
        out.extend([0; 8]);
        out.extend(0b10u32.to_le_bytes());
        out.extend(9_000u32.to_le_bytes());
        out.extend(8_000u32.to_le_bytes());
        out.extend([enemy, 3]);
        out.extend(1.0f32.to_le_bytes());
        out.extend(5u16.to_le_bytes());
        out
    }

    /// A battery delivered for team 1, found by place 0, held by cell 4.
    fn battery() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(0b1u32.to_le_bytes());
        out.extend(40.0f32.to_le_bytes());
        out.extend([0; 8]);
        out.extend(0.5f32.to_le_bytes());
        out.extend(1.0f32.to_le_bytes());
        out.extend([1, 0, 3, 0, 0x81, 0xFF, 4, 2]);
        out
    }

    const TAIL_BYTES: [u8; 11] = [1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];

    fn section(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    fn read(bytes: &[u8], places: usize, batteries: usize) -> Result<Statistics<'_>, Refusal> {
        Statistics::read(&Section { at: 0, bytes }, places, batteries)
    }

    #[test]
    fn every_field_reads_by_name() {
        let bytes = section(&[&player(1), &player(0xFF), &battery(), &TAIL_BYTES]);
        let read = read(&bytes, 2, 1).unwrap();
        let players: Vec<PlayerStatistics> = read.players().collect();
        let first = &players[0];
        assert_eq!(first.events(0).unwrap().count(0), Some(2));
        assert_eq!(first.events(0).unwrap().count(22), Some(30));
        assert_eq!(first.events(2).unwrap().mask(), 1 | 1 << 22);
        assert_eq!(first.received_damage(0).unwrap()[1].unwrap().get(), 12.5);
        assert_eq!(first.last_hit_ms()[1].map(NonZeroU32::get), Some(9_000));
        assert_eq!(first.last_killed_enemy_ms(), 8_000);
        assert_eq!(first.last_hit_enemy(), Place::new(1));
        assert_eq!(players[1].last_hit_enemy(), None);
        assert_eq!((first.killstreak(), first.longest_killstreak()), (3, 5));
        let status = &read.batteries().next().unwrap();
        assert_eq!(status.owner_team(), OwnerTeam::Team(Team::Second));
        assert_eq!(status.first_finder(), Place::new(0));
        assert_eq!(status.progress(), [3, 0]);
        assert_eq!(status.last_taker(), LastTaker::Saved(None));
        assert_eq!(status.held_steps()[4].map(NonZeroU8::get), Some(2));
        assert_eq!(read.tail().team_points(), [1, 0]);
        assert_eq!(read.tail().winner_team(), Winner::Team(Team::First));
    }

    #[test]
    fn the_domains_are_held() {
        let good = section(&[&player(1), &player(0xFF), &battery(), &TAIL_BYTES]);
        assert!(read(&good, 2, 1).is_ok());
        // Kit 1's mask drops kit 0's event 0; a damage of -12.5; an enemy
        // of 20; owner team 2; a first finder of 20; a winner of 4.
        let p = player(1).len();
        assert_eq!(p, 57);
        let b = 2 * p;
        let winner = good.len() - 5;
        for (at, value) in [
            (7, 0x00),
            (28, 0xC1),
            (49, 20),
            (b + 24, 2),
            (b + 25, 20),
            (winner, 4),
        ] {
            let mut bad = good.clone();
            bad[at] = value;
            assert!(read(&bad, 2, 1).is_err(), "{at}");
        }
    }
}

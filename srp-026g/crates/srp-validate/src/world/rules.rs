//! The rules section (snapshot_rules.md section 2), read into [`Rules`]:
//! the rules in the order of the mode's config (the timelimit, the respawn
//! rule, the mode's rule, the kd stats), then the blackboard's two flags.
//! Each rule reads its own bytes; the section has no inner lengths.
//!
//! The rules and their fields (the names are the members the notes give,
//! PROVEN in the readers and writers there):
//!
//! - [`Timelimit`] (`timelimit_rule_core` 0x317F30, 13 bytes): u32
//!   `m_state_start_time_ms` (0xFFFFFFFF until the match is ready), u32
//!   `m_current_time_in_ms` (the world's time), u32 `m_players_mask`, u8
//!   `m_current_state`. Read as one [`Phase`]: the inactive rule has no
//!   start, every other state its start (`on_match_ready` 0x317EE0 sets
//!   the state and the start together; nothing returns to inactive).
//! - [`Respawn`] (`player_respawn_rule_core` 0x316F10): a mask of the
//!   places that wait, `m_player_respawn_times[i]` for each (a time is
//!   written only when it is not 0xFFFFFFFF, 0x316D50: so never that),
//!   `m_respawn_random.seed()`, two `m_inside_objects_count` bytes for each
//!   respawn point of the map (open).
//! - The mode's rule ([`ModeRule`]): [`Gather`] (modes 2 and 3,
//!   `gather_victory_items_rule_core` 0x313CA0, `match_time_vi_spawn_rule_
//!   core` 0x3149B0), [`Tdm`] (`tdm_rule_core` 0x316340), [`Research`]
//!   (exe patch 53).
//! - [`KillsDeaths`] for each place (`kd_stats_rule` 0x323870): u16
//!   kills, u16 deaths, bool online.
//! - The blackboard's `team_1_respawn`, `team_2_respawn` (0x323D60).
//!
//! The domains, each read once:
//!
//! - every bool is 0x00 or 0xFF: what the bool writer 0x80FA0 puts
//!   (`online` 0x3238E7, the two flags 0x323D8A / 0x323DB2, mode 3's
//!   `m_match_started` 0x314B80) (PROVEN). 0x01, which the server's tests
//!   wrote until 2026-10-12, is no longer taken;
//! - the timelimit's state is one of the four, with its start as above;
//! - every place mask names the 20 places (the match's, by the rules);
//! - a battery: no carrier (0xFF: the rule writes no carried one,
//!   0x313D84), a container of the two or 0xFF, then (lying) its two
//!   transforms, velocities and `deactivation_time` numbers, its users
//!   places;
//! - mode 3's `m_available_victory_items_count` at most the map's
//!   batteries (LIKELY: the spawner stops there);
//! - Research: 1 to 3 units, a progress and every mark at most a whole
//!   capture (a mark is the progress or its rest at entry, 0x324350:
//!   research_mode.md; LIKELY), the taking and owning team a team or 2
//!   (nobody), the inside counts their masks' bits.
//!
//! Open: the seeds, the tickets, the shown scores and points (bounded by
//! the map's resources, not in the context), the respawn points' counts,
//! `m_spotted_mask`, the spawn times, and a lying battery's
//! `activation_state`. That is the body's `[+0xE4]`, the dword before
//! `deactivation_time` `[+0xE8]` (0x30162C..0x301644): Bullet's
//! `btCollisionObject::m_activationState1`, whose states are 1 to 5
//! (LIKELY, by the layout). Every game's world has one of them, but the
//! server's own writer of a lying battery (`snapshot::Battery::write`, the
//! scripted client's) puts 0, and it stays as it is in this step: the
//! domain closes when the server's decoders move onto these views.
//!
//! The server's own decoder of this section, for the match's phase and
//! standing, is `snapshot::Rules` (crates/survarium).

use super::{Context, Section, battery_records};
use crate::reader::{Reader, Short};
use crate::{
    Finite, MAX_PLAYERS, Part, Place, Places, Reason, Refusal, Team, mode, timelimit, wire_bool,
};

/// `timelimit_rule_core`'s bytes (0x317F30): start, current time, players
/// mask, state.
const TIMELIMIT: usize = 13;
/// A roster place in `kd_stats_rule` (0x323870): kills, deaths, online.
const KILLS_DEATHS: usize = 5;
/// `tdm_rule_core`'s bytes (0x316340): the tickets, two masks.
const TDM: usize = 12;
/// `items_mask` is a u16.
const MAX_BATTERIES: usize = 16;
/// Every map has a container for each team.
const CONTAINERS: usize = 2;

/// Where the match is (`m_current_state`), with when the state began.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// 0: before the match is ready.
    Inactive,
    /// 1.
    WaitingForPlayers { since_ms: u32 },
    /// 2.
    Countdown { since_ms: u32 },
    /// 3: play.
    InProcess { since_ms: u32 },
}

/// `timelimit_rule_core`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timelimit {
    phase: Phase,
    current_time_ms: u32,
    players: Places,
}

impl Timelimit {
    fn read(bytes: &[u8]) -> Option<Self> {
        let mut reader = Reader::new(bytes, 0);
        let start = reader.u32().ok()?;
        let current_time_ms = reader.u32().ok()?;
        let players = Places::new(reader.u32().ok()?)?;
        let state = reader.u8().ok()?;
        let start = (start != u32::MAX).then_some(start);
        let phase = match (state, start) {
            (0, None) => Phase::Inactive,
            (1, Some(since_ms)) => Phase::WaitingForPlayers { since_ms },
            (2, Some(since_ms)) => Phase::Countdown { since_ms },
            (timelimit::IN_PROCESS, Some(since_ms)) => Phase::InProcess { since_ms },
            _ => return None,
        };
        Some(Self {
            phase,
            current_time_ms,
            players,
        })
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// `m_current_time_in_ms`: the world's own time.
    pub fn current_time_ms(&self) -> u32 {
        self.current_time_ms
    }

    /// `m_players_mask`: the places that entered and have not left.
    pub fn players(&self) -> Places {
        self.players
    }
}

/// `player_respawn_rule_core`, its points borrowed from the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Respawn<'a> {
    waiting: Places,
    times: [Option<u32>; MAX_PLAYERS],
    seed: u32,
    points: &'a [[u8; 2]],
}

impl<'a> Respawn<'a> {
    /// The mask, the times and the seed (what comes before the points),
    /// and their length; `None` if they do not read.
    fn read_head(bytes: &[u8]) -> Option<(Self, usize)> {
        let mut reader = Reader::new(bytes, 0);
        let waiting = Places::new(reader.u32().ok()?)?;
        let mut times = [None; MAX_PLAYERS];
        let mut never = false;
        for place in waiting.iter() {
            let time = reader.u32().ok()?;
            never |= time == u32::MAX;
            *times.get_mut(place.index())? = Some(time);
        }
        let seed = reader.u32().ok()?;
        // A waiting place's time is not 0xFFFFFFFF; as no read, so that the
        // section's split is found as before.
        if never {
            return None;
        }
        Some((
            Self {
                waiting,
                times,
                seed,
                points: &[],
            },
            reader.at(),
        ))
    }

    /// The places that wait for a respawn.
    pub fn waiting(&self) -> Places {
        self.waiting
    }

    /// `m_player_respawn_times[place]`: when a waiting place respawns.
    pub fn time_ms(&self, place: Place) -> Option<u32> {
        self.times.get(place.index()).copied().flatten()
    }

    /// `m_respawn_random.seed()`.
    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// Each respawn point's two `m_inside_objects_count` (team 0's points,
    /// then team 1's, then team 2's).
    pub fn points(&self) -> &'a [[u8; 2]] {
        self.points
    }
}

/// Team Deathmatch's rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tdm {
    team_tickets: [u16; 2],
    alive_players: Places,
    active_players: Places,
}

impl Tdm {
    fn read(bytes: &[u8]) -> Option<Self> {
        let mut reader = Reader::new(bytes, 0);
        let team_tickets = [reader.u16().ok()?, reader.u16().ok()?];
        let alive = reader.u32().ok()?;
        let active = reader.u32().ok()?;
        Some(Self {
            team_tickets,
            alive_players: Places::new(alive)?,
            active_players: Places::new(active)?,
        })
    }

    /// `m_team_tickets`.
    pub fn team_tickets(&self) -> [u16; 2] {
        self.team_tickets
    }

    /// `m_alive_players_mask`.
    pub fn alive_players(&self) -> Places {
        self.alive_players
    }

    /// `m_active_players_mask`.
    pub fn active_players(&self) -> Places {
        self.active_players
    }
}

/// A research unit (`rr_write`): 96 bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResearchUnit {
    progress: u32,
    taking: Option<Team>,
    owner: Option<Team>,
    inside: [Places; 2],
    marks: [u32; MAX_PLAYERS],
}

impl ResearchUnit {
    const LEN: usize = 16 + 4 * MAX_PLAYERS;
    const NONE: Self = Self {
        progress: 0,
        taking: None,
        owner: None,
        inside: [Places::EMPTY; 2],
        marks: [0; MAX_PLAYERS],
    };

    /// The capture of `taking`, 0 to [`Research::FULL`].
    pub fn progress(&self) -> u32 {
        self.progress
    }

    /// Whose progress it is (`None`: nobody).
    pub fn taking(&self) -> Option<Team> {
        self.taking
    }

    /// Whose unit it is (`None`: nobody).
    pub fn owner(&self) -> Option<Team> {
        self.owner
    }

    /// Each team's places inside.
    pub fn inside(&self) -> [Places; 2] {
        self.inside
    }

    /// Each place's mark: its share of the capture when it went in.
    pub fn marks(&self) -> &[u32; MAX_PLAYERS] {
        &self.marks
    }
}

/// Research's rule (exe patch 53, `rr_write` / `rr_read`): two points, two
/// shown scores, the end, the entered places, the unit count, the units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Research {
    points: [u32; 2],
    scores: [u16; 2],
    end_ms: Option<u32>,
    entered: Places,
    units: [ResearchUnit; Research::MAX_UNITS],
    unit_count: usize,
}

impl Research {
    const HEAD: usize = 21;
    /// `RR_MAX_ZONES`.
    const MAX_UNITS: usize = 3;
    /// `RR_FULL`: a unit's whole capture.
    pub const FULL: u32 = 1_000_000_000;

    /// The rule that ends `bytes`: the length its unit count gives, the most
    /// units first. Where it starts, and the rule.
    fn read_back(bytes: &[u8]) -> Option<(usize, Self)> {
        (1..=Self::MAX_UNITS).rev().find_map(|units| {
            let len = ResearchUnit::LEN
                .checked_mul(units)?
                .checked_add(Self::HEAD)?;
            let at = bytes.len().checked_sub(len)?;
            Self::read(bytes.get(at..)?, units).map(|rule| (at, rule))
        })
    }

    /// A rule of `units` units that fills `bytes`, as `rr_read` takes it.
    fn read(bytes: &[u8], units: usize) -> Option<Self> {
        let mut reader = Reader::new(bytes, 0);
        let points = [reader.u32().ok()?, reader.u32().ok()?];
        let scores = [reader.u16().ok()?, reader.u16().ok()?];
        let end = reader.u32().ok()?;
        let entered = Places::new(reader.u32().ok()?)?;
        if usize::from(reader.u8().ok()?) != units {
            return None;
        }
        let mut read = [ResearchUnit::NONE; Self::MAX_UNITS];
        for unit in read.iter_mut().take(units) {
            let progress = reader.u32().ok()?;
            let [taking, owner, first, second] = reader.array::<4>().ok()?;
            let inside = [
                Places::new(reader.u32().ok()?)?,
                Places::new(reader.u32().ok()?)?,
            ];
            let mut marks = [0; MAX_PLAYERS];
            for mark in &mut marks {
                *mark = reader.u32().ok()?;
            }
            if progress > Self::FULL
                || marks.iter().any(|&mark| mark > Self::FULL)
                || u32::from(first) != inside[0].count()
                || u32::from(second) != inside[1].count()
            {
                return None;
            }
            *unit = ResearchUnit {
                progress,
                taking: Team::or_neither(taking)?,
                owner: Team::or_neither(owner)?,
                inside,
                marks,
            };
        }
        reader.is_done().then_some(Self {
            points,
            scores,
            end_ms: (end != u32::MAX).then_some(end),
            entered,
            units: read,
            unit_count: units,
        })
    }

    /// Each team's points, millionths.
    pub fn points(&self) -> [u32; 2] {
        self.points
    }

    /// The points shown, whole.
    pub fn scores(&self) -> [u16; 2] {
        self.scores
    }

    /// When the match ends, once a team has the goal.
    pub fn end_ms(&self) -> Option<u32> {
        self.end_ms
    }

    /// The places that entered and have not left.
    pub fn entered(&self) -> Places {
        self.entered
    }

    /// The units, by `cp_id`.
    pub fn units(&self) -> &[ResearchUnit] {
        self.units.get(..self.unit_count).unwrap_or_default()
    }

    /// Every place the rule names: the entered and each unit's inside.
    pub fn places(&self) -> Places {
        let bits = self.units().iter().fold(self.entered.bits(), |bits, unit| {
            bits | unit.inside[0].bits() | unit.inside[1].bits()
        });
        Places::new(bits).unwrap_or_default()
    }
}

/// A container's id in the rule's vector (`victory_items_containers[..].id`
/// of the map; its team is the map's, `MapMode::container_teams`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainerId(u8);

impl ContainerId {
    fn new(id: u8) -> Option<Self> {
        (usize::from(id) < CONTAINERS).then_some(Self(id))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// Up to twenty places in their order, as a container or a battery lists
/// its users (`u8 count`, then each `owner.id`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceList {
    places: [Place; MAX_PLAYERS],
    len: usize,
}

impl PlaceList {
    fn read(reader: &mut Reader) -> Option<Self> {
        let len = usize::from(reader.u8().ok()?);
        let bytes = reader.take(len).ok()?;
        let mut places = [Place::FIRST; MAX_PLAYERS];
        if len > MAX_PLAYERS {
            return None;
        }
        for (place, byte) in places.iter_mut().zip(bytes) {
            *place = Place::new(*byte)?;
        }
        Some(Self { places, len })
    }

    pub fn as_slice(&self) -> &[Place] {
        self.places.get(..self.len).unwrap_or_default()
    }
}

/// A battery lying in the world (`victory_item_core::serialize` 0x301470).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreeBattery {
    spotted_mask: u32,
    last_safe_transform: [Finite; 16],
    transform: [Finite; 16],
    linear_velocity: [Finite; 3],
    angular_velocity: [Finite; 3],
    deactivation_time: Finite,
    activation_state: u32,
    users: PlaceList,
}

impl FreeBattery {
    /// `m_spotted_mask`.
    pub fn spotted_mask(&self) -> u32 {
        self.spotted_mask
    }

    /// `m_last_safe_transform`: a 4x4 matrix, rows.
    pub fn last_safe_transform(&self) -> [Finite; 16] {
        self.last_safe_transform
    }

    /// `m_transform`.
    pub fn transform(&self) -> [Finite; 16] {
        self.transform
    }

    /// Where it lies: `m_transform`'s translation [LIKELY: its floats
    /// 12..15].
    pub fn position(&self) -> [Finite; 3] {
        let [.., x, y, z, _] = self.transform;
        [x, y, z]
    }

    pub fn linear_velocity(&self) -> [Finite; 3] {
        self.linear_velocity
    }

    pub fn angular_velocity(&self) -> [Finite; 3] {
        self.angular_velocity
    }

    pub fn deactivation_time(&self) -> Finite {
        self.deactivation_time
    }

    /// The body's `m_activationState1` [LIKELY], open (the module doc).
    pub fn activation_state(&self) -> u32 {
        self.activation_state
    }

    /// `m_usable_object_users`.
    pub fn users(&self) -> &[Place] {
        self.users.as_slice()
    }
}

/// A battery as the gather rule has it (reader 0x301660).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Battery {
    /// A player has it: the rule writes nothing of it.
    Carried,
    InContainer(ContainerId),
    Free(FreeBattery),
}

/// A team's container (`victory_items_container_core::serialize`
/// 0x31CE40, reader 0x31CEE0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Container {
    users: PlaceList,
    items: [u8; MAX_BATTERIES],
    item_count: usize,
}

impl Container {
    const NONE: Self = Self {
        users: PlaceList {
            places: [Place::FIRST; MAX_PLAYERS],
            len: 0,
        },
        items: [0; MAX_BATTERIES],
        item_count: 0,
    };

    fn read(reader: &mut Reader) -> Option<Self> {
        let users = PlaceList::read(reader)?;
        let count = reader.u32().ok()? as usize;
        let bytes = reader.take(count).ok()?;
        if count > MAX_BATTERIES || bytes.iter().any(|id| usize::from(*id) >= MAX_BATTERIES) {
            return None;
        }
        let mut items = [0; MAX_BATTERIES];
        for (item, byte) in items.iter_mut().zip(bytes) {
            *item = *byte;
        }
        Some(Self {
            users,
            items,
            item_count: count,
        })
    }

    /// The places using it.
    pub fn users(&self) -> &[Place] {
        self.users.as_slice()
    }

    /// `m_victory_items`: the batteries in it, by number (each under 16).
    pub fn items(&self) -> &[u8] {
        self.items.get(..self.item_count).unwrap_or_default()
    }
}

/// Mode 3's own fields (`match_time_vi_spawn_rule_core`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timed {
    available: u32,
    match_start_ms: u32,
    next_spawn_ms: u32,
    spawn_time_ms: u32,
    spawn_variation_ms: u32,
    started: bool,
}

impl Timed {
    /// `m_available_victory_items_count`: the batteries placed so far.
    pub fn available(&self) -> u32 {
        self.available
    }

    /// `m_match_start_time_ms`.
    pub fn match_start_ms(&self) -> u32 {
        self.match_start_ms
    }

    /// `m_next_spawn_time_ms`.
    pub fn next_spawn_ms(&self) -> u32 {
        self.next_spawn_ms
    }

    /// `m_victory_items_spawn_time_ms`.
    pub fn spawn_time_ms(&self) -> u32 {
        self.spawn_time_ms
    }

    /// `m_victory_items_spawn_variation_ms`.
    pub fn spawn_variation_ms(&self) -> u32 {
        self.spawn_variation_ms
    }

    /// `m_match_started`.
    pub fn started(&self) -> bool {
        self.started
    }
}

/// The gather rule of modes 2 and 3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gather {
    seed: u32,
    team_points: [u8; 2],
    batteries: [Battery; MAX_BATTERIES],
    battery_count: usize,
    containers: [Container; CONTAINERS],
    timed: Option<Timed>,
}

/// Why bytes are no gather rule: they do not read as one (the split is
/// elsewhere), or they do and a field is out of its domain.
enum GatherFault {
    Shape,
    Domain(Reason),
}

impl From<Short> for GatherFault {
    fn from(_: Short) -> Self {
        GatherFault::Shape
    }
}

impl Gather {
    /// `m_spawner_random.seed()`.
    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// `m_team_points`: the batteries in each team's container.
    pub fn team_points(&self) -> [u8; 2] {
        self.team_points
    }

    /// By battery number: the map's batteries, in mode 3 the ones placed.
    pub fn batteries(&self) -> &[Battery] {
        self.batteries.get(..self.battery_count).unwrap_or_default()
    }

    /// By container id.
    pub fn containers(&self) -> &[Container; CONTAINERS] {
        &self.containers
    }

    /// Mode 3's fields.
    pub fn timed(&self) -> Option<Timed> {
        self.timed
    }

    /// A gather rule that fills `bytes` exactly. `batteries` is how many
    /// the mask covers at most (the mode's count).
    fn read(bytes: &[u8], game_mode: u8, batteries: usize) -> Result<Self, GatherFault> {
        let mut reader = Reader::new(bytes, 0);
        let seed = reader.u32()?;
        let team_points = reader.array::<2>()?;
        let available = if game_mode == mode::TIMED_BATTERIES {
            Some(reader.u32()?)
        } else {
            None
        };
        let count = match available {
            Some(available) => (available as usize).min(batteries),
            None => batteries,
        };
        let mask = reader.u16()?;
        if count > MAX_BATTERIES || crate::high(u32::from(mask), count as u32) != 0 {
            return Err(GatherFault::Shape);
        }
        let mut read = [Battery::Carried; MAX_BATTERIES];
        for (number, battery) in read.iter_mut().take(count).enumerate() {
            if crate::bit(u32::from(mask), number as u32) {
                *battery = read_battery(&mut reader, number)?;
            }
        }
        let mut containers = [Container::NONE; CONTAINERS];
        for container in &mut containers {
            *container = Container::read(&mut reader).ok_or(GatherFault::Shape)?;
        }
        // Mode 3: the four times and `m_match_started`.
        let timed = match available {
            Some(available) => Some((
                available,
                [reader.u32()?, reader.u32()?, reader.u32()?, reader.u32()?],
                reader.u8()?,
            )),
            None => None,
        };
        // The teams have no more points than the containers hold, and the
        // batteries in a container say so of themselves. The containers can
        // hold one more than the points: live mode 3 worlds of five matches
        // that ended on time at 9 to 0 (2026-10-22) have all 10 batteries in
        // the winner's container and points of 9, in every game's world (two
        // vouched for them). Why the tenth is in and not counted was not traced
        // [LIKELY the rule's completion, match.md section 26: once `+0x15C` is
        // set the handler counts no put]. The points are not used to index
        // anything; the batteries and containers are, and they are still
        // checked one by one.
        let held = containers.iter().fold(0usize, |held, container| {
            held.saturating_add(container.items().len())
        });
        let [first, second] = team_points;
        let read_batteries = read.get(..count).unwrap_or_default();
        let consistent = usize::from(first).saturating_add(usize::from(second)) <= held
            && containers.iter().enumerate().all(|(id, container)| {
                container.items().iter().all(|item| {
                    matches!(read_batteries.get(usize::from(*item)),
                        Some(Battery::InContainer(container)) if usize::from(container.get()) == id)
                })
            });
        if !reader.is_done() || !consistent {
            return Err(GatherFault::Shape);
        }
        let timed = match timed {
            Some((available, times, started)) => {
                if available as usize > batteries {
                    return Err(GatherFault::Domain(Reason::Batteries {
                        found: available as usize,
                        map: batteries,
                    }));
                }
                let [
                    match_start_ms,
                    next_spawn_ms,
                    spawn_time_ms,
                    spawn_variation_ms,
                ] = times;
                Some(Timed {
                    available,
                    match_start_ms,
                    next_spawn_ms,
                    spawn_time_ms,
                    spawn_variation_ms,
                    started: wire_bool(started).ok_or(GatherFault::Domain(Reason::RulesField {
                        field: "m_match_started",
                    }))?,
                })
            }
            None => None,
        };
        Ok(Self {
            seed,
            team_points,
            batteries: read,
            battery_count: count,
            containers,
            timed,
        })
    }
}

/// A battery the rule's mask names: not carried, so its carrier byte is
/// 0xFF; in a container, or lying with its transforms and users.
fn read_battery(reader: &mut Reader, number: usize) -> Result<Battery, GatherFault> {
    let spotted_mask = reader.u32()?;
    // No carrier, or the rule would not have written it (0x313D84).
    if reader.u8()? != 0xFF {
        return Err(GatherFault::Shape);
    }
    let container = reader.u8()?;
    if container != 0xFF {
        return ContainerId::new(container)
            .map(Battery::InContainer)
            .ok_or(GatherFault::Shape);
    }
    // Two 4x4 transforms, two velocities, `deactivation_time`.
    let mut floats = [0u32; FREE_FLOATS];
    for float in &mut floats {
        *float = reader.u32()?;
    }
    let activation = reader.u32()?;
    let users = PlaceList::read(reader).ok_or(GatherFault::Shape)?;
    let not_number = GatherFault::Domain(Reason::BatteryNotNumber { battery: number });
    let mut numbers = [Finite::ZERO; FREE_FLOATS];
    for (number_of, bits) in numbers.iter_mut().zip(floats) {
        *number_of =
            Finite::of_bits(bits).ok_or(GatherFault::Domain(Reason::BatteryNotNumber {
                battery: number,
            }))?;
    }
    let (last_safe_transform, rest) = numbers
        .split_first_chunk::<16>()
        .ok_or(GatherFault::Shape)?;
    let (transform, rest) = rest.split_first_chunk::<16>().ok_or(GatherFault::Shape)?;
    let (linear_velocity, rest) = rest.split_first_chunk::<3>().ok_or(GatherFault::Shape)?;
    let (angular_velocity, rest) = rest.split_first_chunk::<3>().ok_or(GatherFault::Shape)?;
    let deactivation_time = *rest.first().ok_or(not_number)?;
    Ok(Battery::Free(FreeBattery {
        spotted_mask,
        last_safe_transform: *last_safe_transform,
        transform: *transform,
        linear_velocity: *linear_velocity,
        angular_velocity: *angular_velocity,
        deactivation_time,
        activation_state: activation,
        users,
    }))
}

/// A lying battery's floats: two 4x4 transforms, two float3 velocities,
/// `deactivation_time`.
const FREE_FLOATS: usize = 16 + 16 + 3 + 3 + 1;

/// The mode's own rule, between the respawn rule and the kd stats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModeRule {
    /// Modes 2 and 3.
    Gather(Gather),
    /// Mode 4.
    Tdm(Tdm),
    /// Mode 5.
    Research(Research),
}

/// A roster place in `kd_stats_rule`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KillsDeaths {
    kills: u16,
    deaths: u16,
    online: bool,
}

impl KillsDeaths {
    pub fn kills(&self) -> u16 {
        self.kills
    }

    pub fn deaths(&self) -> u16 {
        self.deaths
    }

    pub fn online(&self) -> bool {
        self.online
    }
}

/// The rules section, read into its rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules<'a> {
    timelimit: Timelimit,
    respawn: Respawn<'a>,
    mode_rule: ModeRule,
    kills_deaths: [KillsDeaths; MAX_PLAYERS],
    places: usize,
    team_respawn: [bool; 2],
}

/// The parts of the section found from its two ends: the timelimit, the
/// kd stats, the two flags, and the bytes between.
struct Ends<'a> {
    timelimit: Timelimit,
    kills_deaths: [KillsDeaths; MAX_PLAYERS],
    team_respawn: [bool; 2],
    middle: &'a [u8],
}

impl<'a> Rules<'a> {
    /// The rules of a world of `context`'s match, or why not.
    ///
    /// The section must start with a timelimit rule of the world's time
    /// and end in kd stats and two flags. The respawn rule has two bytes
    /// for each respawn point of the map, and nothing says how many: in
    /// mode 4 the rest is the tdm rule's 12 bytes; in mode 5 Research's 21
    /// plus 96 a unit (its unit count is its byte 20); in modes 2 and 3 it
    /// is the number with which the gather rule after it reads to the kd
    /// stats exactly (the map's when known, else the smallest that does).
    pub fn read(section: &Section<'a>, context: &Context) -> Result<Self, Refusal> {
        let refuse = |reason| Refusal::new(Part::Rules, section.at, reason);
        let places = context.places;
        let bytes = section.bytes;
        let unread = || {
            refuse(Reason::RulesUnread {
                len: bytes.len(),
                time: context.time,
                places,
            })
        };
        let Ends {
            timelimit,
            kills_deaths,
            team_respawn,
            middle,
        } = Self::ends(bytes, context).ok_or_else(unread)?;
        let head = Respawn::read_head(middle);
        // The respawn rule's points end where the mode's rule starts.
        let respawn_to = |end: usize| {
            let (respawn, head_len) = head?;
            let points = middle.get(head_len..end)?;
            let (points, rest) = points.as_chunks::<2>();
            rest.is_empty().then_some(Respawn { points, ..respawn })
        };
        let (respawn, mode_rule) = match context.game_mode {
            mode::TEAM_DEATHMATCH => match middle.len().checked_sub(TDM) {
                Some(at) => (
                    respawn_to(at),
                    middle.get(at..).and_then(Tdm::read).map(ModeRule::Tdm),
                ),
                None => (None, None),
            },
            mode::RESEARCH => match Research::read_back(middle) {
                Some((at, research)) => (respawn_to(at), Some(ModeRule::Research(research))),
                None => (None, None),
            },
            _ => {
                let batteries = battery_records(context);
                let read = |at: usize| {
                    let bytes = middle.get(at..).ok_or(GatherFault::Shape)?;
                    Gather::read(bytes, context.game_mode, batteries).map(|gather| (at, gather))
                };
                // With the map's respawn points known, the gather rule
                // starts after them: one read, not one at every even offset
                // (which is quadratic in the section's length).
                let found = match head {
                    None => Err(GatherFault::Shape),
                    Some((_, head_len)) => match context.respawn_points {
                        Some(points) => points
                            .checked_mul(2)
                            .and_then(|len| len.checked_add(head_len))
                            .ok_or(GatherFault::Shape)
                            .and_then(read),
                        None => (head_len..=middle.len())
                            .step_by(2)
                            .map(read)
                            .find(|found| !matches!(found, Err(GatherFault::Shape)))
                            .unwrap_or(Err(GatherFault::Shape)),
                    },
                };
                match found {
                    Ok((at, gather)) => (respawn_to(at), Some(ModeRule::Gather(gather))),
                    Err(GatherFault::Domain(reason)) => return Err(refuse(reason)),
                    Err(GatherFault::Shape) => (None, None),
                }
            }
        };
        let Some(respawn) = respawn else {
            return Err(refuse(Reason::NoRespawnAndModeRule));
        };
        let mode_rule = mode_rule.ok_or_else(|| {
            refuse(match context.game_mode {
                mode::TEAM_DEATHMATCH => Reason::NoTdmRule,
                mode::RESEARCH => Reason::NoResearchRule,
                _ => Reason::NoGatherRule,
            })
        })?;
        Ok(Self {
            timelimit,
            respawn,
            mode_rule,
            kills_deaths,
            places,
            team_respawn,
        })
    }

    /// The timelimit of the world's time (a state it can have, no player
    /// outside the roster), the kd stats and the two flags at the end; the
    /// bytes between.
    fn ends(bytes: &'a [u8], context: &Context) -> Option<Ends<'a>> {
        let places = context.places;
        if places > MAX_PLAYERS {
            return None;
        }
        let kd_len = KILLS_DEATHS.checked_mul(places)?;
        let flags_at = bytes.len().checked_sub(2)?;
        let kd_at = flags_at.checked_sub(kd_len)?;
        if kd_at < TIMELIMIT + 8 {
            return None;
        }
        let [first, second] = *bytes.get(flags_at..)?.first_chunk::<2>()?;
        let team_respawn = [wire_bool(first)?, wire_bool(second)?];
        let mut kills_deaths = [KillsDeaths::default(); MAX_PLAYERS];
        let (records, _) = bytes.get(kd_at..flags_at)?.as_chunks::<KILLS_DEATHS>();
        for (record, kd) in records.iter().zip(&mut kills_deaths) {
            let [k0, k1, d0, d1, online] = *record;
            *kd = KillsDeaths {
                kills: u16::from_le_bytes([k0, k1]),
                deaths: u16::from_le_bytes([d0, d1]),
                online: wire_bool(online)?,
            };
        }
        let timelimit = Timelimit::read(bytes.get(..TIMELIMIT)?)?;
        if timelimit.current_time_ms != context.time || timelimit.players.past(places).is_some() {
            return None;
        }
        Some(Ends {
            timelimit,
            kills_deaths,
            team_respawn,
            middle: bytes.get(TIMELIMIT..kd_at)?,
        })
    }

    pub fn timelimit(&self) -> Timelimit {
        self.timelimit
    }

    pub fn respawn(&self) -> Respawn<'a> {
        self.respawn
    }

    pub fn mode_rule(&self) -> &ModeRule {
        &self.mode_rule
    }

    /// By roster place.
    pub fn kills_deaths(&self) -> &[KillsDeaths] {
        self.kills_deaths.get(..self.places).unwrap_or_default()
    }

    /// The blackboard's `team_1_respawn`, `team_2_respawn`: whether each
    /// team may still respawn.
    pub fn team_respawn(&self) -> [bool; 2] {
        self.team_respawn
    }
}

/// The rules read ([`Rules::read`]); the map's respawn points, every place
/// and battery they name within the match.
pub fn rules(section: &Section, context: &Context) -> Result<(), Refusal> {
    let refuse = |reason| Err(Refusal::new(Part::Rules, section.at, reason));
    let places = context.places;
    let read = Rules::read(section, context)?;
    let respawn = read.respawn();
    if let Some(map) = context.respawn_points
        && respawn.points().len() != map
    {
        return refuse(Reason::RespawnPoints {
            found: respawn.points().len(),
            map,
        });
    }
    if let Some(place) = respawn.waiting().past(places) {
        return refuse(Reason::RespawnTimePast { place, places });
    }
    match read.mode_rule() {
        ModeRule::Tdm(tdm) => {
            if tdm.alive_players().past(places).is_some()
                || tdm.active_players().past(places).is_some()
            {
                return refuse(Reason::TdmMaskPast { places });
            }
            Ok(())
        }
        ModeRule::Research(research) => match research.places().past(places) {
            Some(_) => refuse(Reason::ResearchMaskPast { places }),
            None => Ok(()),
        },
        ModeRule::Gather(gather) => match gather_fault(gather, context) {
            Some(reason) => refuse(reason),
            None => Ok(()),
        },
    }
}

/// What in a gather rule names a battery or a place the match does not
/// have.
fn gather_fault(gather: &Gather, context: &Context) -> Option<Reason> {
    let places = context.places;
    let batteries = context.batteries;
    let expected = match gather.timed() {
        Some(timed) => (timed.available() as usize).min(batteries),
        None => batteries,
    };
    if gather.batteries().len() != expected {
        return Some(Reason::Batteries {
            found: gather.batteries().len(),
            map: expected,
        });
    }
    for (container, held) in gather.containers().iter().enumerate() {
        if let Some(user) = held.users().iter().find(|user| user.index() >= places) {
            return Some(Reason::ContainerUserPast {
                container,
                user: user.get(),
                places,
            });
        }
        if let Some(&battery) = held
            .items()
            .iter()
            .find(|item| usize::from(**item) >= batteries)
        {
            return Some(Reason::ContainerBatteryPast {
                container,
                battery,
                batteries,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    /// A timelimit at 539 650 ms with places 0 and 1 in the match.
    const TIMELIMIT: [u8; 13] = [
        0x08, 0x52, 0x00, 0x00, 0x02, 0x3C, 0x08, 0x00, 0x03, 0x00, 0x00, 0x00, 0x03,
    ];
    const TIME: u32 = 539_650;
    /// No one waiting, a seed, three respawn points.
    const RESPAWN: [u8; 14] = [0, 0, 0, 0, 0x44, 0x33, 0x22, 0x11, 0, 0, 0, 0, 0, 0];
    /// Kills, deaths, online for two places, then the two flags.
    const KD_AND_FLAGS: [u8; 12] = [3, 0, 0, 0, 0xFF, 0, 0, 3, 0, 0xFF, 0xFF, 0xFF];

    fn context(game_mode: u8, respawn_points: Option<usize>) -> Context<'static> {
        Context {
            time: TIME,
            places: 2,
            game_mode,
            batteries: 0,
            respawn_points,
            profiles: None,
            artefacts: None,
            objects: None,
            inventory: None,
        }
    }

    /// Research's rule as `rr_write` puts it: team 0 owns unit A, team 1
    /// takes unit B with the places of `inside_b`, unit C is nobody's.
    fn research(units: usize, inside_b: u32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(64_400_000u32.to_le_bytes());
        out.extend(37_500_000u32.to_le_bytes());
        out.extend([64, 0, 38, 0]);
        out.extend(u32::MAX.to_le_bytes());
        out.extend(0b11u32.to_le_bytes());
        out.push(units as u8);
        for unit in 0..units {
            let (progress, taking, owner, inside) = match unit {
                0 => (Research::FULL, 0, 0, [0, 0]),
                1 => (500_000_000, 1, 2, [0, inside_b]),
                _ => (0, 2, 2, [0, 0]),
            };
            out.extend(progress.to_le_bytes());
            out.extend([
                taking,
                owner,
                inside[0].count_ones() as u8,
                inside[1].count_ones() as u8,
            ]);
            out.extend(inside[0].to_le_bytes());
            out.extend(inside[1].to_le_bytes());
            out.extend([0; 4 * MAX_PLAYERS]);
        }
        out
    }

    fn bytes(rule: &[u8]) -> Vec<u8> {
        [&TIMELIMIT[..], &RESPAWN, rule, &KD_AND_FLAGS].concat()
    }

    fn check(rule: &[u8], game_mode: u8, respawn_points: Option<usize>) -> Result<(), Refusal> {
        let bytes = bytes(rule);
        rules(
            &Section {
                at: 0,
                bytes: &bytes,
            },
            &context(game_mode, respawn_points),
        )
    }

    fn reason(result: Result<(), Refusal>) -> Option<Reason> {
        result.err().map(|refusal| refusal.reason)
    }

    #[test]
    fn every_field_reads_by_name() {
        let bytes = bytes(&[97, 0, 100, 0, 3, 0, 0, 0, 3, 0, 0, 0]);
        let section = Section {
            at: 0,
            bytes: &bytes,
        };
        let read = Rules::read(&section, &context(mode::TEAM_DEATHMATCH, Some(3))).unwrap();
        let timelimit = read.timelimit();
        assert_eq!(timelimit.phase(), Phase::InProcess { since_ms: 0x5208 });
        assert_eq!(timelimit.current_time_ms(), TIME);
        assert_eq!(timelimit.players().bits(), 0b11);
        assert_eq!(read.respawn().seed(), 0x1122_3344);
        assert_eq!(read.respawn().points(), &[[0, 0]; 3]);
        let ModeRule::Tdm(tdm) = read.mode_rule() else {
            panic!("no tdm rule")
        };
        assert_eq!(tdm.team_tickets(), [97, 100]);
        assert_eq!(tdm.alive_players().bits(), 3);
        let kd = read.kills_deaths();
        assert_eq!(
            (kd[0].kills(), kd[0].deaths(), kd[0].online()),
            (3, 0, true)
        );
        assert_eq!(
            (kd[1].kills(), kd[1].deaths(), kd[1].online()),
            (0, 3, true)
        );
        assert_eq!(read.team_respawn(), [true, true]);
    }

    #[test]
    fn the_bools_and_the_phase_are_held() {
        let tdm = [97, 0, 100, 0, 3, 0, 0, 0, 3, 0, 0, 0];
        assert_eq!(check(&tdm, mode::TEAM_DEATHMATCH, Some(3)), Ok(()));
        let good = bytes(&tdm);
        let last = good.len() - 1;
        // An online of 0x01, a flag of 0x01, a state of 4, the inactive
        // state with a start.
        for (at, value) in [(last - 2, 0x01), (last, 0x01), (12, 4), (12, 0)] {
            let mut bad = good.clone();
            bad[at] = value;
            let refused = rules(
                &Section { at: 0, bytes: &bad },
                &context(mode::TEAM_DEATHMATCH, Some(3)),
            );
            assert!(refused.is_err(), "{at}");
        }
    }

    #[test]
    fn a_research_rule_reads_with_its_units_and_the_respawn_points() {
        for units in 1..=3 {
            let rule = research(units, 0b10);
            assert_eq!(rule.len(), 21 + 96 * units);
            assert_eq!(check(&rule, mode::RESEARCH, Some(3)), Ok(()), "{units}");
        }
        assert_eq!(
            reason(check(&research(3, 0b10), mode::RESEARCH, Some(4))),
            Some(Reason::RespawnPoints { found: 3, map: 4 })
        );
    }

    #[test]
    fn a_wrong_research_rule_is_refused() {
        // A place past the two of the match inside a unit.
        assert_eq!(
            reason(check(&research(3, 0b100), mode::RESEARCH, None)),
            Some(Reason::ResearchMaskPast { places: 2 })
        );
        // A place past the 20: no rule `rr_read` takes.
        let past = research(3, 1 << 20);
        assert_eq!(
            reason(check(&past, mode::RESEARCH, None)),
            Some(Reason::NoRespawnAndModeRule)
        );
        // A count that is not the places inside, an owner 3, a progress over
        // a whole capture, a mark over a whole capture.
        for (at, value) in [
            (21 + 96 + 7, 2u8),
            (21 + 5, 3),
            (21 + 3, 0x40),
            (21 + 16 + 3, 0x40),
        ] {
            let mut wrong = research(3, 0b10);
            wrong[at] = value;
            assert_eq!(
                reason(check(&wrong, mode::RESEARCH, None)),
                Some(Reason::NoRespawnAndModeRule),
                "{at}"
            );
        }
        // A byte short of its unit count.
        let mut short = research(3, 0b10);
        short.pop();
        assert!(check(&short, mode::RESEARCH, None).is_err());
    }

    #[test]
    fn a_research_rule_is_no_other_modes_rule() {
        let rule = research(3, 0b10);
        assert_eq!(
            reason(check(&rule, mode::BATTERIES, None)),
            Some(Reason::NoRespawnAndModeRule)
        );
        let tdm = [97, 0, 100, 0, 3, 0, 0, 0, 3, 0, 0, 0];
        assert_eq!(check(&tdm, mode::TEAM_DEATHMATCH, Some(3)), Ok(()));
        assert!(check(&tdm, mode::RESEARCH, None).is_err());
    }
}

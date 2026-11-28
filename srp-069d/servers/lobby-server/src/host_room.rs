//! The world host's room for new matches, as the queues use it (docs/notes/online.md section 7.3a, "Loaded maps").
//!
//! The world host loads at most `max_zygotes` maps (one level project each: map x mode x variant) and runs at most
//! `max_matches` matches per map (more only with free memory); a match for a map it cannot load waits there 20 s and is
//! then refused. The hook ([`crate::online::OnlineMatches::host_room`]) gives the host's latest room status (srp asks
//! for it about every 0.5 s and counts its own starts since), and the queues choose with it:
//! * a new map can load (or there is no status: no world host, the link is down for 5 s): the rotation over every served
//!   row, as always, without the loaded maps that take no more match ([`Scope::Rotation`]); the idle maps the next new
//!   map's load evicts are taken only by a group that fits nothing else, so they do get evicted; ranked takes its slot
//!   mode from every served row and is held when all of that mode's maps are full;
//! * no new map can load: only the rows whose map is loaded and takes one more match ([`Scope::Loaded`]); the rotation
//!   keeps its order among them (the mode and the map started the longest ago, a never started one first), the variant
//!   is the least recently started of the loaded ones. Busy maps go first: the map the world host drains
//!   ([`HostStatus::drain`]) and the idle maps are taken only by a group that fits nothing else, so the drained map's
//!   matches end, it turns idle, the world host evicts it and the rotation loads its next map;
//! * neither: nothing starts, the group stays queued ([`Scope::Held`]) and is tried again every second ([`Held`]).
//!
//! The choice is made from the status at every start and keeps no state: as soon as a map can load again, the next
//! start is the rotation's.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::queue::Pick;

pub use world_host_proto::status::{HostStatus, LoadKind, LoadedMap, MatchRoom, NewMap, NewMaps};

/// How often a held group is tried again (the hook's status is read about every 0.5 s).
pub const HELD_RETRY: Duration = Duration::from_secs(1);

/// The world host's latest room status, the hook's own starts since it was read counted in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostRoom {
    /// how old the status is
    pub age: Duration,
    pub status: HostStatus,
}

impl HostRoom {
    /// "status N ms old: ..." for the log lines.
    pub fn describe(&self) -> String {
        format!("world host status {} ms old: {}", self.age.as_millis(), self.status.summary())
    }
}

/// The map the world host drains ([`HostStatus::drain`]: chosen by its front while no new map can load), while it is
/// loaded and busy (an idle one, also one srp closed because it is about to be evicted, is not named).
pub fn drain_map(st: &HostStatus) -> Option<&LoadedMap> {
    if st.new_maps != NewMaps::NoRoom {
        return None;
    }
    st.drain.as_deref().and_then(|d| st.map(d)).filter(|m| m.running > 0 || m.starting > 0)
}

/// The idle maps the next new map's load evicts (the first entry of the new maps, the lowest ranks).
fn evicted_next(st: &HostStatus) -> Vec<&str> {
    let NewMaps::Room(v) = &st.new_maps else { return vec![] };
    let n = v.first().map_or(0, |m| usize::from(m.evicts));
    let mut idle: Vec<&LoadedMap> = st.loaded.iter().filter(|m| m.evict.is_some()).collect();
    idle.sort_by_key(|m| m.evict);
    idle.into_iter().take(n).map(|m| m.project.as_str()).collect()
}

/// A loaded map the queues take only for a group that fits nothing else: while a new map can load, an idle map its load
/// evicts; while none can, an idle map or the drained one (busy maps with room first).
pub fn avoided(st: &HostStatus, project: &str) -> bool {
    let Some(m) = st.map(project) else { return false };
    if st.new_maps.loads() {
        evicted_next(st).contains(&project)
    } else {
        m.evict.is_some() || drain_map(st).is_some_and(|d| d.project == project)
    }
}

/// The rows a queue match may use now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// a new map can load, or there is no status: `rows` = every served row but those of loaded maps that take no more
    /// match; `served` = every served row (a ranked group's slot mode comes from them); `avoid` = rows taken only when no
    /// other fits the group ([`avoided`])
    Rotation { served: Vec<Pick>, rows: Vec<Pick>, avoid: Vec<Pick> },
    /// no new map can load: `open` = the served rows whose map is loaded and takes one more match (never empty);
    /// `served` = every served row; `avoid` = the open rows taken only when no other fits the group ([`avoided`])
    Loaded { served: Vec<Pick>, open: Vec<Pick>, avoid: Vec<Pick> },
    /// no new map can load and no served row's map takes one more match
    Held,
}

impl Scope {
    /// The same scope on the rows of one mode only (the ranked fall-through).
    pub fn of_mode(&self, mode: u32) -> Scope {
        let only = |v: &[Pick]| v.iter().filter(|r| r.mode == mode).copied().collect::<Vec<_>>();
        match self {
            Scope::Rotation { served, rows, avoid } => Scope::Rotation { served: only(served), rows: only(rows), avoid: only(avoid) },
            Scope::Loaded { served, open, avoid } => match only(open) {
                o if o.is_empty() => Scope::Held,
                o => Scope::Loaded { served: only(served), open: o, avoid: only(avoid) },
            },
            Scope::Held => Scope::Held,
        }
    }

    /// The rows a match may be drawn from (none when held).
    pub fn candidates(&self) -> &[Pick] {
        match self {
            Scope::Rotation { rows, .. } => rows,
            Scope::Loaded { open, .. } => open,
            Scope::Held => &[],
        }
    }
}

/// The scope of a queue match: `rows` = the served rows with their level projects, `room` = the hook's status (None:
/// the rotation over all of them).
pub fn scope(rows: &[(Pick, String)], room: Option<&HostRoom>) -> Scope {
    let served = || rows.iter().map(|r| r.0).collect::<Vec<_>>();
    let Some(room) = room else { return Scope::Rotation { served: served(), rows: served(), avoid: vec![] } };
    let st = &room.status;
    let avoid_of = |picks: &[Pick]| rows.iter().filter(|(r, p)| picks.contains(r) && avoided(st, p)).map(|r| r.0).collect::<Vec<_>>();
    if st.new_maps.loads() {
        // a loaded map that takes no more match would refuse it: never picked, even by the rotation
        let rot: Vec<Pick> = rows.iter().filter(|(_, p)| st.map(p).is_none_or(|m| m.room.takes_one())).map(|r| r.0).collect();
        if rot.is_empty() {
            return Scope::Held;
        }
        let avoid = avoid_of(&rot);
        return Scope::Rotation { served: served(), rows: rot, avoid };
    }
    let open: Vec<Pick> = rows.iter().filter(|(_, p)| st.takes_one(p)).map(|r| r.0).collect();
    if open.is_empty() {
        return Scope::Held;
    }
    let avoid = avoid_of(&open);
    Scope::Loaded { served: served(), open, avoid }
}

/// Why a group is held: the world host's state, not its numbers (the log line is written once per kind).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum HoldKind {
    /// a new map can load, but every served row of the group's modes is on a loaded map that takes no more match
    MapsFull,
    /// no new map can load and no loaded map of the group's modes takes another match
    NoRoom,
    /// the base process is not ready: no map loads, and no loaded map of the group's modes takes another match
    BaseNotReady,
}

impl HoldKind {
    /// The kind of a hold under `room` (a hold needs a status: without one nothing is narrowed).
    pub fn of(room: &HostRoom) -> HoldKind {
        match room.status.new_maps {
            NewMaps::BaseNotReady => HoldKind::BaseNotReady,
            NewMaps::Room(ref v) if !v.is_empty() => HoldKind::MapsFull,
            _ => HoldKind::NoRoom,
        }
    }

    fn text(self) -> &'static str {
        match self {
            HoldKind::MapsFull => "every map of its modes is loaded and takes no more match",
            HoldKind::NoRoom => "no loaded map of its modes takes another match and no map can load",
            HoldKind::BaseNotReady => "no loaded map of its modes takes another match and the world host's base process is not ready",
        }
    }
}

/// A queue the world host's room holds: when it is tried again and what was logged (one line per (queue, group key) and
/// kind of hold, not one per try; again after a start).
#[derive(Debug, Default)]
pub struct Held {
    retry: Option<Instant>,
    /// per (queue, group key: a PvP group's mode mask, a PvE difficulty): the kind of hold logged last
    logged: BTreeMap<(&'static str, u32), HoldKind>,
}

impl Held {
    /// A group of the queue `what` (`key`: its mode mask or difficulty) is held for `kind` under `room`: the line is
    /// logged when the kind is new for that queue and key. The queue is tried again [`HELD_RETRY`] after the first hold
    /// since the last try (a hold on every tick, as the bot fill's, does not push the retry later). True = logged.
    pub fn hold(&mut self, now: Instant, what: &'static str, key: u32, kind: HoldKind, room: &HostRoom) -> bool {
        self.retry.get_or_insert(now + HELD_RETRY);
        if self.logged.get(&(what, key)) == Some(&kind) {
            return false;
        }
        log::info!("{what}: held, the group (key {key:#x}) stays queued: {} ({}); tried again every {} s", kind.text(), room.describe(), HELD_RETRY.as_secs());
        self.logged.insert((what, key), kind);
        true
    }

    /// The retry is due (taken: the caller tries once, a new hold sets the next one).
    pub fn due(&mut self, now: Instant) -> bool {
        if self.retry.is_some_and(|t| now >= t) {
            self.retry = None;
            return true;
        }
        false
    }

    /// A match of the queue started: the next hold is logged again.
    pub fn started(&mut self) {
        self.logged.clear();
    }
}

/// The PvE maze of a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Maze {
    /// a new map can load, or there is no status: the lobby's draw (it fits: loaded with room, or it can load)
    Drawn(u8),
    /// a new map can load, but the drawn maze is loaded and takes no more match, or is an idle map the next load evicts
    /// while the other one fits: the other one
    Other(u8),
    /// no new map can load: this loaded maze takes one more match (of two, the one not avoided: busy, not drained; else
    /// the least recently started)
    Loaded(u8),
    /// neither maze fits: the group stays queued
    Held,
}

/// The maze (1 or 2) of a PvE match: `projects` = the level projects of mazes 1 and 2 (None = not in the data), `drawn`
/// = the lobby's draw, `last` = the queue's last start of a project (0 = never).
pub fn pve_maze(projects: [Option<&str>; 2], room: Option<&HostRoom>, drawn: u8, last: impl Fn(&str) -> u64) -> Maze {
    let Some(st) = room.map(|r| &r.status) else { return Maze::Drawn(drawn) };
    let project = |m: u8| projects[usize::from(m - 1)];
    let shunned = |m: u8| project(m).is_some_and(|p| avoided(st, p));
    let other = 3 - drawn;
    if st.new_maps.loads() {
        // loaded with room, or not loaded (it can load); a maze the next load evicts only when the other does not fit
        let fits = |m: u8| project(m).is_some_and(|p| st.map(p).is_none_or(|l| l.room.takes_one()));
        return match (fits(drawn) && !shunned(drawn), fits(other) && !shunned(other)) {
            (true, _) => Maze::Drawn(drawn),
            (false, true) => Maze::Other(other),
            (false, false) if fits(drawn) => Maze::Drawn(drawn),
            (false, false) if fits(other) => Maze::Other(other),
            (false, false) => Maze::Held,
        };
    }
    let open: Vec<u8> = (1u8..=2).filter(|m| project(*m).is_some_and(|p| st.takes_one(p))).collect();
    let preferred: Vec<u8> = open.iter().copied().filter(|m| !shunned(*m)).collect();
    let cands = if preferred.is_empty() { open } else { preferred };
    match cands.as_slice() {
        [] => Maze::Held,
        [m] => Maze::Loaded(*m),
        _ => {
            let seq = |m: u8| project(m).map_or(0, &last);
            match seq(1).cmp(&seq(2)) {
                std::cmp::Ordering::Less => Maze::Loaded(1),
                std::cmp::Ordering::Greater => Maze::Loaded(2),
                std::cmp::Ordering::Equal => Maze::Loaded(drawn),
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A loaded map: idle (evict rank 0) when it runs nothing, else busy; loaded `age` s ago.
    pub(crate) fn map_aged(project: &str, running: u16, room: MatchRoom, age: u32) -> LoadedMap {
        LoadedMap { project: project.into(), running, starting: 0, room, evict: (running == 0).then_some(0), loaded_secs: age }
    }

    pub(crate) fn map(project: &str, running: u16, room: MatchRoom) -> LoadedMap {
        map_aged(project, running, room, 100)
    }

    pub(crate) fn room(new_maps: NewMaps, loaded: Vec<LoadedMap>) -> HostRoom {
        HostRoom { age: Duration::from_millis(300), status: HostStatus { max_zygotes: Some(2), max_matches: 4, free_mb: Some(900), new_maps, loaded, drain: None } }
    }

    /// [`room`] with the world host draining `drain`.
    pub(crate) fn room_draining(new_maps: NewMaps, loaded: Vec<LoadedMap>, drain: &str) -> HostRoom {
        let mut r = room(new_maps, loaded);
        r.status.drain = Some(drain.into());
        r
    }

    /// `n` new maps within the limit, no eviction.
    pub(crate) fn can_load(n: usize) -> NewMaps {
        NewMaps::Room(vec![NewMap { kind: LoadKind::Within, evicts: 0 }; n])
    }

    /// One new map that evicts `evicts` idle maps first.
    pub(crate) fn evicting(evicts: u16) -> NewMaps {
        NewMaps::Room(vec![NewMap { kind: LoadKind::Within, evicts }])
    }

    pub(crate) fn p(map: u32, mode: u32, variation: u32) -> Pick {
        Pick { map, mode, variation, weather: 1 }
    }

    fn rows() -> Vec<(Pick, String)> {
        vec![(p(1, 1, 0), "a".into()), (p(2, 1, 0), "b".into()), (p(2, 1, 1), "b2".into()), (p(3, 2, 0), "c".into())]
    }

    fn all() -> Vec<Pick> {
        rows().iter().map(|r| r.0).collect()
    }

    #[test]
    fn a_map_that_can_load_keeps_the_rotation() {
        assert_eq!(scope(&rows(), None), Scope::Rotation { served: all(), rows: all(), avoid: vec![] }, "no status: as always");
        // the idle c is the map the next load evicts: still a row, taken only by a group that fits nothing else
        let r = room(evicting(1), vec![map("a", 4, MatchRoom::Over), map("c", 0, MatchRoom::Within)]);
        assert_eq!(scope(&rows(), Some(&r)), Scope::Rotation { served: all(), rows: all(), avoid: vec![p(3, 2, 0)] });
        // a free slot first: nothing is evicted for the next load, nothing avoided
        let r = room(can_load(1), vec![map("c", 0, MatchRoom::Within)]);
        assert_eq!(scope(&rows(), Some(&r)), Scope::Rotation { served: all(), rows: all(), avoid: vec![] });
        // a loaded map that takes no more match is left out even then (the host would refuse it)
        let r = room(can_load(3), vec![map("a", 8, MatchRoom::Full)]);
        assert_eq!(scope(&rows(), Some(&r)), Scope::Rotation { served: all(), rows: all()[1..].to_vec(), avoid: vec![] });
    }

    #[test]
    fn no_room_narrows_to_the_loaded_maps_busy_ones_first_or_holds() {
        // the drained a is full anyway; b2 and c busy: nothing avoided
        let r = room_draining(NewMaps::NoRoom, vec![map("a", 8, MatchRoom::Full), map("b2", 4, MatchRoom::Over), map("c", 1, MatchRoom::Within)], "a");
        assert_eq!(scope(&rows(), Some(&r)), Scope::Loaded { served: all(), open: vec![p(2, 1, 1), p(3, 2, 0)], avoid: vec![] });
        // the drained b2 is avoided
        let r = room_draining(NewMaps::NoRoom, vec![map("b2", 4, MatchRoom::Over), map("c", 1, MatchRoom::Within)], "b2");
        assert_eq!(scope(&rows(), Some(&r)), Scope::Loaded { served: all(), open: vec![p(2, 1, 1), p(3, 2, 0)], avoid: vec![p(2, 1, 1)] });
        // an idle map (one over the limit, its eviction gives no new map) is avoided too: busy maps first
        let r = room(NewMaps::NoRoom, vec![map("b2", 4, MatchRoom::Over), map("c", 0, MatchRoom::Within)]);
        assert_eq!(scope(&rows(), Some(&r)), Scope::Loaded { served: all(), open: vec![p(2, 1, 1), p(3, 2, 0)], avoid: vec![p(3, 2, 0)] });
        // the base is not ready: no drain is named (the front names none then; the lobby ignores one anyway)
        let r = room_draining(NewMaps::BaseNotReady, vec![map("c", 1, MatchRoom::Within)], "c");
        assert_eq!(scope(&rows(), Some(&r)), Scope::Loaded { served: all(), open: vec![p(3, 2, 0)], avoid: vec![] });
        // every loaded map full, or a loaded map that is not served (another mode's): held
        let r = room(NewMaps::NoRoom, vec![map("a", 8, MatchRoom::Full), map("other", 1, MatchRoom::Within)]);
        assert_eq!(scope(&rows(), Some(&r)), Scope::Held);
        assert_eq!(scope(&rows(), Some(&room(NewMaps::Room(vec![]), vec![]))), Scope::Held, "an empty list is no room");
        // one mode's rows of a narrowed scope; none of them loaded: held
        let s = Scope::Loaded { served: all(), open: vec![p(2, 1, 1), p(3, 2, 0)], avoid: vec![p(2, 1, 1)] };
        assert_eq!(s.of_mode(2), Scope::Loaded { served: vec![p(3, 2, 0)], open: vec![p(3, 2, 0)], avoid: vec![] });
        assert_eq!(s.of_mode(5), Scope::Held);
        assert!(Scope::Held.candidates().is_empty());
    }

    #[test]
    fn the_drained_map_is_the_front_choice_while_it_is_busy() {
        let maps = vec![map_aged("x", 2, MatchRoom::Within, 50), map_aged("y", 1, MatchRoom::Full, 900), map_aged("z", 0, MatchRoom::Within, 5000)];
        let name = |r: &HostRoom| drain_map(&r.status).map(|m| m.project.clone());
        assert_eq!(name(&room_draining(NewMaps::NoRoom, maps.clone(), "y")), Some("y".into()), "busy, even full");
        assert_eq!(name(&room_draining(NewMaps::NoRoom, maps.clone(), "z")), None, "idle (or closed by srp): not named");
        assert_eq!(name(&room_draining(NewMaps::NoRoom, maps.clone(), "gone")), None, "not loaded");
        assert_eq!(name(&room_draining(can_load(1), maps.clone(), "y")), None, "a map can load");
        assert_eq!(name(&room_draining(NewMaps::BaseNotReady, maps.clone(), "y")), None, "the base is not ready");
        assert_eq!(name(&room(NewMaps::NoRoom, maps)), None, "the front names none");
        // avoided: while a map can load, only the idle maps the next load evicts (the lowest ranks)
        let mut two = vec![map("i0", 0, MatchRoom::Within), map("i1", 0, MatchRoom::Within), map("busy", 2, MatchRoom::Within)];
        two[1].evict = Some(1);
        let st = room(evicting(1), two.clone()).status;
        assert_eq!((avoided(&st, "i0"), avoided(&st, "i1"), avoided(&st, "busy"), avoided(&st, "new")), (true, false, false, false));
        let st = room(NewMaps::Room(vec![NewMap { kind: LoadKind::Within, evicts: 2 }]), two.clone()).status;
        assert_eq!((avoided(&st, "i0"), avoided(&st, "i1")), (true, true));
        // ... and while none can, every idle map and the drained one
        let st = room_draining(NewMaps::NoRoom, two, "busy").status;
        assert_eq!((avoided(&st, "i0"), avoided(&st, "i1"), avoided(&st, "busy")), (true, true, true));
    }

    #[test]
    fn a_hold_is_logged_once_per_queue_group_and_kind_and_retried_every_second() {
        let t0 = Instant::now();
        let ms = Duration::from_millis;
        let mut h = Held::default();
        let no_room = room(NewMaps::NoRoom, vec![map("a", 8, MatchRoom::Full)]);
        assert!(!h.due(t0), "nothing held");
        assert!(h.hold(t0, "queue", 0x6, HoldKind::of(&no_room), &no_room));
        // the numbers change (memory, matches), the kind does not: no new line
        let mut later = no_room.clone();
        later.status.free_mb = Some(777);
        later.age = ms(900);
        assert!(!h.hold(t0 + ms(100), "queue", 0x6, HoldKind::of(&later), &later));
        // the bot fill's line for the same mask is its own; it holds every tick, which does not push the retry later
        assert!(h.hold(t0 + ms(500), "queue with AI players", 0x6, HoldKind::NoRoom, &no_room));
        assert!(!h.hold(t0 + ms(900), "queue with AI players", 0x6, HoldKind::NoRoom, &no_room));
        // the retry: due one second after the first hold, taken once
        assert!(!h.due(t0 + HELD_RETRY - ms(1)));
        assert!(h.due(t0 + HELD_RETRY));
        assert!(!h.due(t0 + HELD_RETRY * 2), "taken: a new hold sets the next one");
        assert!(!h.hold(t0 + HELD_RETRY * 2, "queue", 0x6, HoldKind::NoRoom, &no_room));
        assert!(h.due(t0 + HELD_RETRY * 3));
        // the kind changes: a new line
        let base = room(NewMaps::BaseNotReady, vec![]);
        assert_eq!(HoldKind::of(&base), HoldKind::BaseNotReady);
        assert!(h.hold(t0, "queue", 0x6, HoldKind::of(&base), &base));
        // a start: the next hold is logged again
        h.started();
        assert!(h.hold(t0, "queue", 0x6, HoldKind::BaseNotReady, &base));
        assert_eq!(HoldKind::of(&room(can_load(1), vec![])), HoldKind::MapsFull);
    }

    #[test]
    fn the_pve_maze_fits_the_room_or_holds() {
        let pr = [Some("m1"), Some("m2")];
        let never = |_: &str| 0;
        assert_eq!(pve_maze(pr, None, 2, never), Maze::Drawn(2));
        // a map can load: the draw when it is not loaded or has room, else the other one, else hold
        assert_eq!(pve_maze(pr, Some(&room(can_load(1), vec![])), 1, never), Maze::Drawn(1));
        let full1 = room(can_load(1), vec![map("m1", 8, MatchRoom::Full)]);
        assert_eq!(pve_maze(pr, Some(&full1), 1, never), Maze::Other(2), "the drawn maze is full: the other can load");
        assert_eq!(pve_maze(pr, Some(&full1), 2, never), Maze::Drawn(2));
        let full_both = room(can_load(1), vec![map("m1", 8, MatchRoom::Full), map("m2", 8, MatchRoom::Full)]);
        assert_eq!(pve_maze(pr, Some(&full_both), 1, never), Maze::Held);
        assert_eq!(pve_maze([Some("m1"), None], Some(&full1), 1, never), Maze::Held, "a maze missing from the data never fits");
        // the drawn maze is the idle map the next load evicts: the other (it can load) instead; alone, still the drawn one
        let idle1 = room(evicting(1), vec![map("m1", 0, MatchRoom::Within)]);
        assert_eq!(pve_maze(pr, Some(&idle1), 1, never), Maze::Other(2));
        assert_eq!(pve_maze([Some("m1"), None], Some(&idle1), 1, never), Maze::Drawn(1));
        // no map can load: a loaded maze with room
        let one = room(NewMaps::NoRoom, vec![map("m2", 3, MatchRoom::Within), map("m1", 8, MatchRoom::Full)]);
        assert_eq!(pve_maze(pr, Some(&one), 1, never), Maze::Loaded(2), "the drawn maze is full: the other");
        let both = room(NewMaps::NoRoom, vec![map("m1", 3, MatchRoom::Within), map("m2", 4, MatchRoom::Over)]);
        assert_eq!(pve_maze(pr, Some(&both), 2, |p| if p == "m1" { 5 } else { 9 }), Maze::Loaded(1), "the least recently started");
        assert_eq!(pve_maze(pr, Some(&both), 2, |p| if p == "m1" { 9 } else { 5 }), Maze::Loaded(2));
        assert_eq!(pve_maze(pr, Some(&both), 2, never), Maze::Loaded(2), "a tie keeps the draw");
        // the drained maze (m1) gets no new match while m2 fits, whatever the last starts; alone it still takes the group
        let drained = room_draining(NewMaps::NoRoom, vec![map("m1", 3, MatchRoom::Within), map("m2", 1, MatchRoom::Within)], "m1");
        assert_eq!(pve_maze(pr, Some(&drained), 1, |p| if p == "m1" { 1 } else { 9 }), Maze::Loaded(2));
        let only_drained = room_draining(NewMaps::NoRoom, vec![map("m1", 3, MatchRoom::Within), map("m2", 8, MatchRoom::Full)], "m1");
        assert_eq!(pve_maze(pr, Some(&only_drained), 2, never), Maze::Loaded(1), "never held for draining");
        // an idle maze is avoided too: the busy one with room first
        let idle_m2 = room(NewMaps::NoRoom, vec![map("m1", 3, MatchRoom::Within), map("m2", 0, MatchRoom::Within)]);
        assert_eq!(pve_maze(pr, Some(&idle_m2), 2, never), Maze::Loaded(1));
        let none = room(NewMaps::BaseNotReady, vec![map("m1", 8, MatchRoom::Full)]);
        assert_eq!(pve_maze(pr, Some(&none), 1, never), Maze::Held);
    }
}

//! The world host's room status in srp (lobby-server `host_room`): a background thread asks the daemon for it about
//! every 0.5 s ([`crate::world_host::watch_room`]); the lobby reads the latest copy without waiting ([`RoomBook::room`]),
//! with the matches srp created since that read counted in ([`RoomBook::reserve`]), so two groups do not both take the
//! last slot of a map, or the same idle map twice, before the daemon shows the first.
//!
//! A created match's relay connects to the daemon at once and the daemon counts its Create from its first frame
//! (`starting`), so a reservation is dropped once a status asked [`RESERVE_SETTLE`] or more after it arrived (by then
//! the daemon counts the match), at the latest after [`RESERVE_MAX`]. A failed read keeps the last status; a status
//! older than [`STALE`] counts as none: the queues then choose as without a world host status.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use lobby_server::host_room::{HostRoom, HostStatus, MatchRoom, NewMaps};

/// A reservation is dropped once a status asked this long after it arrived (the relay's connection and Create, with
/// margin).
pub const RESERVE_SETTLE: Duration = Duration::from_secs(1);
/// A reservation is dropped after this long whatever the statuses say (any status fresh enough to be used shows the
/// match by then; this keeps the list short while no status arrives).
pub const RESERVE_MAX: Duration = Duration::from_secs(10);
/// A status older than this is not used (the reader is stuck or the link is down).
pub const STALE: Duration = Duration::from_secs(5);

/// The latest status and the starts since.
#[derive(Default)]
pub struct RoomBook {
    inner: Mutex<Book>,
}

#[derive(Default)]
struct Book {
    latest: Option<Read>,
    /// (project, when the match was created)
    reserved: Vec<(String, Instant)>,
}

/// One status read.
struct Read {
    status: HostStatus,
    /// when it was asked for
    asked: Instant,
}

impl RoomBook {
    /// A status asked for at `asked` arrived: it replaces the last one, and the reservations it already counts end.
    pub fn set(&self, status: HostStatus, asked: Instant) {
        let mut b = self.inner.lock().unwrap();
        b.reserved.retain(|(_, at)| !settled(*at, asked));
        b.latest = Some(Read { status, asked });
    }

    /// A match on `project` was created at `now`: counted until a status counts it.
    pub fn reserve(&self, project: &str, now: Instant) {
        self.inner.lock().unwrap().reserved.push((project.to_string(), now));
    }

    /// The latest status at `now` with the reservations counted in; None without one or when it is older than [`STALE`]
    /// (a failed read keeps the last one until then).
    pub fn room(&self, now: Instant) -> Option<HostRoom> {
        let mut b = self.inner.lock().unwrap();
        b.reserved.retain(|(_, at)| now.saturating_duration_since(*at) < RESERVE_MAX);
        let r = b.latest.as_ref()?;
        let age = now.saturating_duration_since(r.asked);
        if age >= STALE {
            return None;
        }
        let projects: Vec<&str> = b.reserved.iter().filter(|(_, at)| !settled(*at, r.asked)).map(|(p, _)| p.as_str()).collect();
        Some(HostRoom { age, status: with_reserved(&r.status, &projects) })
    }
}

/// A reservation made at `at` is counted by a status asked at `asked`.
fn settled(at: Instant, asked: Instant) -> bool {
    asked.saturating_duration_since(at) >= RESERVE_SETTLE
}

/// `status` with matches created since on `projects` (one entry per match) counted in, as the daemon will see them:
/// * a loaded map's start counts as starting there; it takes one more only while that stays under max_matches (the
///   over-limit room was the daemon's for one match); an idle map that gets one is busy, never evicted;
/// * each other project (once, however many matches) takes the next of the new maps that can still load: the idle maps
///   the daemon evicts for it (the next ones in its order) are closed: no group goes to a map about to go;
/// * the new maps left are the daemon's list replayed over the idle maps still evictable: one whose evictions are no
///   longer there (an idle map got a match) and every one after it is gone.
pub fn with_reserved(status: &HostStatus, projects: &[&str]) -> HostStatus {
    let mut s = status.clone();
    let mut new: Vec<&str> = Vec::new();
    for p in projects {
        match s.loaded.iter_mut().find(|m| m.project == *p) {
            Some(m) => {
                m.starting = m.starting.saturating_add(1);
                let total = u32::from(m.running) + u32::from(m.starting);
                m.room = if total < s.max_matches { MatchRoom::Within } else { MatchRoom::Full };
                m.evict = None;
            }
            None if !new.contains(p) => new.push(p),
            None => {}
        }
    }
    let NewMaps::Room(list) = &s.new_maps else { return s };
    // the idle maps still evictable, in the daemon's order (indexes into s.loaded)
    let mut idle: Vec<usize> = (0..s.loaded.len()).filter(|&i| s.loaded[i].evict.is_some()).collect();
    idle.sort_by_key(|&i| s.loaded[i].evict);
    let (mut next, mut used) = (0usize, 0usize);
    let mut left = Vec::new();
    let mut closed = Vec::new();
    for m in list {
        let need = usize::from(m.evicts);
        if next + need > idle.len() {
            break;
        }
        let victims = &idle[next..next + need];
        next += need;
        if used < new.len() {
            used += 1;
            closed.extend_from_slice(victims);
        } else {
            left.push(*m);
        }
    }
    for i in closed {
        s.loaded[i].room = MatchRoom::Full;
        s.loaded[i].evict = None;
    }
    s.new_maps = if left.is_empty() { NewMaps::NoRoom } else { NewMaps::Room(left) };
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use lobby_server::host_room::{LoadKind, LoadedMap, NewMap};

    fn lm(project: &str, running: u16, room: MatchRoom, evict: Option<u16>) -> LoadedMap {
        LoadedMap { project: project.into(), running, starting: 0, room, evict, loaded_secs: 60 }
    }

    fn st(new_maps: NewMaps, loaded: Vec<LoadedMap>) -> HostStatus {
        HostStatus { max_zygotes: Some(3), max_matches: 4, free_mb: Some(1500), new_maps, loaded, drain: None }
    }

    fn w(evicts: u16) -> NewMap {
        NewMap { kind: LoadKind::Within, evicts }
    }

    fn status() -> HostStatus {
        st(NewMaps::Room(vec![w(0)]), vec![lm("a", 2, MatchRoom::Within, None), lm("b", 4, MatchRoom::Over, None)])
    }

    #[test]
    fn reserved_starts_use_up_the_room() {
        let s = with_reserved(&status(), &["a"]);
        assert_eq!((s.loaded[0].starting, s.loaded[0].room), (1, MatchRoom::Within));
        let s = with_reserved(&status(), &["a", "a"]);
        assert_eq!(s.loaded[0].room, MatchRoom::Full, "2 + 2 = max_matches");
        let s = with_reserved(&status(), &["b"]);
        assert_eq!(s.loaded[1].room, MatchRoom::Full, "the one over the limit is taken");
        // a map that is not loaded takes the one map that can load; a second match on it does not take another
        let s = with_reserved(&status(), &["c", "c"]);
        assert_eq!(s.new_maps, NewMaps::NoRoom);
        assert_eq!(with_reserved(&status(), &[]), status());
        let two = st(NewMaps::Room(vec![w(0), w(0)]), vec![]);
        assert_eq!(with_reserved(&two, &["c"]).new_maps, NewMaps::Room(vec![w(0)]));
        assert_eq!(with_reserved(&st(NewMaps::NoRoom, vec![]), &["c"]).new_maps, NewMaps::NoRoom);
    }

    /// The limit is reached, X is idle, every other map busy: the daemon can load one new map Y by evicting X. Whatever
    /// the order of two starts within a status read, the second never goes where the daemon would refuse it.
    #[test]
    fn an_idle_map_is_room_for_one_start_only() {
        let s0 = st(NewMaps::Room(vec![w(1)]), vec![lm("busy", 3, MatchRoom::Within, None), lm("x", 0, MatchRoom::Within, Some(0))]);
        // Y first: its load evicts X; X is closed for a second group, and no further map can load
        let s = with_reserved(&s0, &["y"]);
        assert_eq!(s.new_maps, NewMaps::NoRoom);
        assert_eq!((s.loaded[1].room, s.loaded[1].evict), (MatchRoom::Full, None), "x is about to go");
        assert!(!s.takes_one("x") && s.takes_one("busy"));
        // X first: it is busy now, nothing can be evicted for Y: no new map, X still takes more
        let s = with_reserved(&s0, &["x"]);
        assert_eq!(s.new_maps, NewMaps::NoRoom);
        assert_eq!((s.loaded[1].starting, s.loaded[1].room, s.loaded[1].evict), (1, MatchRoom::Within, None));
        // two idle maps, two new maps (one eviction each): one start on X leaves one new map, evicting Z
        let s2 = st(NewMaps::Room(vec![w(1), w(1)]), vec![lm("x", 0, MatchRoom::Within, Some(0)), lm("z", 0, MatchRoom::Within, Some(1))]);
        let s = with_reserved(&s2, &["x"]);
        assert_eq!(s.new_maps, NewMaps::Room(vec![w(1)]));
        assert!(s.takes_one("z"), "z is still open: it goes only once a new map is started");
        let s = with_reserved(&s2, &["x", "y"]);
        assert_eq!(s.new_maps, NewMaps::NoRoom);
        assert!(!s.takes_one("z"), "y's load evicts z");
        // a free slot first, then an eviction: the first new map takes the slot, the second evicts X
        let s3 = st(NewMaps::Room(vec![w(0), w(1)]), vec![lm("x", 0, MatchRoom::Within, Some(0))]);
        let s = with_reserved(&s3, &["y"]);
        assert_eq!(s.new_maps, NewMaps::Room(vec![w(1)]));
        assert!(s.takes_one("x"));
        let s = with_reserved(&s3, &["y", "v"]);
        assert_eq!(s.new_maps, NewMaps::NoRoom);
        assert!(!s.takes_one("x"));
    }

    #[test]
    fn a_reservation_lasts_until_a_status_counts_it() {
        let book = RoomBook::default();
        let t0 = Instant::now();
        assert_eq!(book.room(t0), None, "no status yet");
        book.set(status(), t0);
        book.reserve("a", t0 + Duration::from_millis(100));
        book.reserve("a", t0 + Duration::from_millis(200));
        let r = book.room(t0 + Duration::from_millis(300)).unwrap();
        assert_eq!((r.age, r.status.loaded[0].room), (Duration::from_millis(300), MatchRoom::Full));
        // a status asked too soon after them still counts them; one asked RESERVE_SETTLE after the last ends them
        book.set(status(), t0 + Duration::from_millis(500));
        assert_eq!(book.room(t0 + Duration::from_millis(600)).unwrap().status.loaded[0].starting, 2);
        book.set(status(), t0 + Duration::from_millis(1150));
        assert_eq!(book.room(t0 + Duration::from_millis(1200)).unwrap().status.loaded[0].starting, 1, "the first one ended");
        book.set(status(), t0 + Duration::from_millis(1300));
        assert_eq!(book.room(t0 + Duration::from_millis(1400)).unwrap().status, status());
        // without new statuses the list is still cleaned after RESERVE_MAX (any fresh status counts the match by then)
        book.reserve("a", t0 + Duration::from_millis(1500));
        assert_eq!(book.room(t0 + Duration::from_millis(1600)).unwrap().status.loaded[0].starting, 1);
        assert!(book.room(t0 + Duration::from_millis(1500) + RESERVE_MAX).is_none(), "the status is stale by then");
        assert!(book.inner.lock().unwrap().reserved.is_empty());
        // a status is used until it is STALE old (failed reads in between keep it), then none
        book.set(status(), t0);
        assert!(book.room(t0 + STALE - Duration::from_millis(1)).is_some());
        assert!(book.room(t0 + STALE).is_none());
    }
}

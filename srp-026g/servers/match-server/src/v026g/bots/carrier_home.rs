//! [bot traps] A battery carrier's way home forecast from the take on: taken, taken to wait a zone out on the way, or left.
//!
//! A carrier walks without sprint and jumps no higher than its standing jump (`RouteFilter::without_sprint`,
//! [`mapdata::grid::STANDING_JUMP_M`]), at the body's forward walk (`Kinematics::walk`, the client's speeds), off the other
//! sides' spawn areas and the floors the zones shut as it gets to them. Measured (2026-11-25, Vostok's battery cell, seed 2,
//! at ce6f992df, `private/pvb-343-check/bot-traps-20261125`): bot 12 chose the battery at (-8.8, 6.0, -20.5) at 38.2 s with
//! its way home open; a zone that switches came on round it as it took it (it walked out of the zone's floors at 42.8 s),
//! every search for the way home failed from 41.8 s, it searched again for 22 s and let go, and a mate took the battery
//! again. Checks of the way as the zones stood at the choice (a whole search, the regions) passed it at 38.2 s: what shut
//! the way was the zones' schedule during the carry.
//!
//! So the forecast walks the carrier's way home as the schedule will have it ([`forecast`]): the way over the floors open
//! all match (`Tactics::carrier_static_way`, one search a stand and side, its path never pulled straight: a pull spends
//! walk ticks the poll's other routes share), stepped at the carrier's walk from the time it takes the battery. A floor a
//! switching zone shuts when the carrier gets there:
//! - **hold** when the schedule opens it (`Hazards::gate_end`, the zones' spans from the game data) within the carrier's
//!   gate wait (`home_gate_max_ms`) and the floor before it stays dry the while: the carrier waits there and goes on, as a
//!   player waits out a cloud;
//! - **refuse** when it does not, unless the drive's search finds a way round it then (`path::way_exists`, one search at a
//!   shut floor only, its route never made).
//!
//! Collect asks it at the fetch, before the walk in, and again at the take (`collect.rs`), and leaves a refused battery (set
//! aside like a battery no way leads to). Not in the candidates' test: there the forecast runs 30 s and more ahead (the
//! walk there and the carry) and turned batteries a bot could take later into refusals now (Vostok's battery cells, seeds 1
//! and 3: bots 13-89 m off, and a detour search for every battery at every commander pass). A carrier whose way home a zone shut all the same holds still for
//! the schedule's opening (`collect::Strand::Gate`) instead of searching every 0.5 s. Not for a battery taken by a crossing
//! into a zone or an energy core's own walk in: their rules weigh the zones themselves.

use std::sync::Arc;

use mapdata::{NodeId, Vec3};

/// The step the way is walked in, m.
const STEP_M: f32 = 1.0;

/// Static ways kept at most: the oldest goes first.
const KEPT: usize = 64;

/// What a static way holds for: the take's stand floor, the home container's floor, the carrier's side, the zones that
/// never switch off ([`fixed_key`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub stand: NodeId,
    pub home: NodeId,
    pub side: Option<i32>,
    pub fixed: u64,
}

/// The set of zones that never switch off (`Hazards::fixed_zones`), as a key.
pub fn fixed_key(fixed: &[bool]) -> u64 {
    super::nav_log::key(fixed)
}

/// A carrier's way home over the floors open all match: its points, the stand first (none: no such way).
pub type Way = Option<Arc<Vec<Vec3>>>;

/// The carrier's ways home by [`Key`].
#[derive(Clone, Debug, Default)]
pub struct Ways(Vec<(Key, Way)>);

impl Ways {
    /// The way kept for `key`, if one was searched.
    pub fn get(&self, key: Key) -> Option<Way> {
        self.0.iter().find(|(k, _)| *k == key).map(|(_, way)| way.clone())
    }

    pub fn put(&mut self, key: Key, way: Way) {
        self.0.retain(|(k, _)| *k != key);
        if self.0.len() >= KEPT {
            self.0.remove(0);
        }
        self.0.push((key, way));
    }
}

/// How long a search for a way round a shut floor is kept, ms: the cool-down a failed search's goal gets (`unreachable.rs`,
/// measured on the battery cells: 5 s spared 287 of 290 repeated exhaustive failures).
pub const ROUND_KEEP_MS: u32 = 5_000;

/// What a search for a way round holds for: its start floor, the home floor, the side, the zone board
/// (`Hazards::version`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoundKey {
    pub from: NodeId,
    pub home: NodeId,
    pub side: Option<i32>,
    pub hazards: u64,
}

/// The searches for a way round, each with the time it was asked for, kept [`ROUND_KEEP_MS`].
#[derive(Clone, Debug, Default)]
pub struct Rounds(Vec<(RoundKey, bool, u32)>);

impl Rounds {
    /// The answer for `key` asked within [`ROUND_KEEP_MS`] of `t`, if any.
    pub fn get(&self, key: RoundKey, t: u32) -> Option<bool> {
        self.0.iter().find(|(k, _, at)| *k == key && t.abs_diff(*at) < ROUND_KEEP_MS).map(|&(_, round, _)| round)
    }

    pub fn put(&mut self, key: RoundKey, round: bool, t: u32) {
        self.0.retain(|(k, _, _)| *k != key);
        if self.0.len() >= KEPT {
            self.0.remove(0);
        }
        self.0.push((key, round, t));
    }
}

/// The forecast of a carry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Forecast {
    /// Every floor of the way is open as the carrier passes it: home at `home_at`. Also when a zone shuts it and the drive's
    /// search finds a way round (`detour`): the forecast ends there (`home_at`: that time).
    Open { home_at: u32, detour: bool },
    /// A switching zone shuts the way at `at` when the carrier gets there (`reached`); the schedule opens it at `opens`: the
    /// carrier waits on the dry floor before it, and is home at `home_at`.
    Hold { at: Vec3, reached: u32, opens: u32, home_at: u32 },
    /// No way home in the carry's window: none over the floors open all match (`at` none), or a floor at `at` a zone shuts
    /// when the carrier gets there (`reached`) that the schedule does not open within the wait (`opens`: when it does, if
    /// it says), with no way round it then.
    Refuse { at: Option<Vec3>, reached: u32, opens: Option<u32> },
}

impl Forecast {
    pub fn refused(&self) -> bool {
        matches!(self, Self::Refuse { .. })
    }

    /// The log's words, times from `now`.
    pub fn words(&self, now: u32) -> String {
        let s = |t: u32| f64::from(t.saturating_sub(now)) / 1_000.0;
        match *self {
            Self::Open { home_at, detour: false } => format!("open all the way, home in {:.1} s", s(home_at)),
            Self::Open { home_at, detour: true } => {
                format!("a zone shuts the way in {:.1} s, a search finds a way round it", s(home_at))
            }
            Self::Hold { at, reached, opens, home_at } => format!(
                "a zone shuts the way at {} in {:.1} s, the schedule opens it in {:.1} s: hold {:.1} s on the dry floor before it, home in {:.1} s",
                super::nav_log::at(at),
                s(reached),
                s(opens),
                f64::from(opens - reached) / 1_000.0,
                s(home_at)
            ),
            Self::Refuse { at: None, .. } => "no way home over the floors open all match".to_owned(),
            Self::Refuse { at: Some(at), reached, opens } => format!(
                "a zone shuts the way at {} in {:.1} s, {}, and a search finds no way round it",
                super::nav_log::at(at),
                s(reached),
                opens.map_or("the schedule does not open it".to_owned(), |o| format!("the schedule opens it only in {:.1} s", s(o)))
            ),
        }
    }
}

/// What the forecast asks of the floors and the schedule.
pub trait Floors {
    /// The floor at `p`.
    fn node(&self, p: Vec3) -> Option<NodeId>;
    /// Whether a body may be on `node` at `t` (`Hazards::allowed`).
    fn open(&self, node: NodeId, t: u32) -> bool;
    /// The soonest time from `t` a body may be on `node` by the schedule (`Hazards::gate_end`).
    fn opens(&self, node: NodeId, t: u32) -> Option<u32>;
    /// Whether `node` stays open from `t` for `ms` (`Hazards::allowed_for`).
    fn dry_for(&self, node: NodeId, t: u32, ms: u32) -> bool;
    /// Whether the drive's search finds a way from `from` home at `t` (`path::way_exists`).
    fn way_round(&self, from: Vec3, t: u32) -> bool;
}

/// The carry along `way` (its points, the stand first, home last) from `taken` at `speed` m/s: each floor in [`STEP_M`] steps
/// as the carrier gets there, holds of at most `max_hold_ms` in all.
pub fn forecast(way: &[Vec3], taken: u32, speed: f32, max_hold_ms: u32, floors: &impl Floors) -> Forecast {
    let speed = speed.max(0.1);
    let mut t = f64::from(taken);
    let mut held = 0u32;
    let mut hold: Option<(Vec3, u32, u32)> = None;
    let mut prev: Option<(Vec3, NodeId)> = None;
    for leg in way.windows(2) {
        let (a, b) = (leg[0], leg[1]);
        let len = (b - a).length();
        let steps = ((len / STEP_M).ceil() as u32).max(1);
        for i in 1..=steps {
            let p = a + (b - a) * (i as f32 / steps as f32);
            t += f64::from(len / steps as f32 / speed) * 1000.0;
            let Some(node) = floors.node(p) else {
                continue;
            };
            let now = t as u32;
            if floors.open(node, now) {
                prev = Some((p, node));
                continue;
            }
            // Shut when the carrier gets there: a way round it, a wait for the schedule, or no way home.
            let before = prev.map_or(a, |(q, _)| q);
            if floors.way_round(before, now) {
                return Forecast::Open { home_at: now, detour: true };
            }
            let opens = floors.opens(node, now);
            let waits = opens.filter(|&o| {
                held + (o - now) <= max_hold_ms && prev.is_none_or(|(_, before)| floors.dry_for(before, now, o - now))
            });
            let Some(o) = waits else {
                return Forecast::Refuse { at: Some(p), reached: now, opens };
            };
            held += o - now;
            hold.get_or_insert((p, now, o));
            t = f64::from(o);
            prev = Some((p, node));
        }
    }
    let home_at = t as u32;
    match hold {
        Some((at, reached, opens)) => Forecast::Hold { at, reached, opens, home_at },
        None => Forecast::Open { home_at, detour: false },
    }
}

/// The event line of a fetch's forecast for the battery at `at` taken from `stand` (`what`: taken or left).
pub fn line(what: &str, at: Vec3, stand: Vec3, forecast: &Forecast, now: u32) -> String {
    format!(
        "carrier home: battery at {} {what} (stand {}): {} at {:.1} s",
        super::nav_log::at(at),
        super::nav_log::at(stand),
        forecast.words(now),
        f64::from(now) / 1_000.0
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straight way along x, one floor a metre (floor n at x = n); a zone on floors `zone` from `on` to `off` (`u32::MAX`:
    /// on for good); the regions see a way round when `round`.
    struct Line {
        zone: std::ops::Range<u32>,
        on: u32,
        off: u32,
        round: bool,
    }

    impl Floors for Line {
        fn node(&self, p: Vec3) -> Option<NodeId> {
            Some(p.x.round() as NodeId)
        }
        fn open(&self, node: NodeId, t: u32) -> bool {
            !(self.zone.contains(&node) && (self.on..self.off).contains(&t))
        }
        fn opens(&self, node: NodeId, t: u32) -> Option<u32> {
            if self.open(node, t) {
                Some(t)
            } else {
                (self.off != u32::MAX).then_some(self.off)
            }
        }
        fn dry_for(&self, node: NodeId, t: u32, ms: u32) -> bool {
            (t..t + ms).step_by(100).all(|t| self.open(node, t))
        }
        fn way_round(&self, _: Vec3, _: u32) -> bool {
            self.round
        }
    }

    /// 20 m along x at 2 m/s from 1 s: home at 11 s with no zone. A zone on floors 10..12 met at 6 s: not yet on (from 7 s),
    /// open; on from 5 s to 9 s, a hold to 9 s, home at 14 s; opening past the 5 s wait, or never, with no way round:
    /// refused; with a way round: open by it.
    #[test]
    fn a_carry_is_open_held_or_refused_by_the_schedule_as_the_carrier_gets_there() {
        let way = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(20.0, 0.0, 0.0)];
        let at = |zone, on, off, round| forecast(&way, 1_000, 2.0, 5_000, &Line { zone, on, off, round });
        assert_eq!(at(0..0, 0, 0, false), Forecast::Open { home_at: 11_000, detour: false });
        assert_eq!(at(10..12, 7_000, 9_000, false), Forecast::Open { home_at: 11_000, detour: false });
        let held = at(10..12, 5_000, 9_000, false);
        let Forecast::Hold { at: p, reached, opens, home_at } = held else { panic!("{held:?}") };
        assert_eq!((p.x as u32, reached, opens, home_at), (10, 6_000, 9_000, 14_000));
        assert_eq!(
            at(10..12, 5_000, 12_000, false),
            Forecast::Refuse { at: Some(Vec3::new(10.0, 0.0, 0.0)), reached: 6_000, opens: Some(12_000) }
        );
        assert!(at(10..12, 5_000, u32::MAX, false).refused());
        assert_eq!(at(10..12, 5_000, u32::MAX, true), Forecast::Open { home_at: 6_000, detour: true });
    }

    /// A search for a way round is kept for its key, within [`ROUND_KEEP_MS`] of the time asked.
    #[test]
    fn a_way_round_is_kept_for_a_while() {
        let key = RoundKey { from: 1, home: 2, side: Some(0), hazards: 3 };
        let mut kept = Rounds::default();
        kept.put(key, true, 10_000);
        assert_eq!(kept.get(key, 10_000 + ROUND_KEEP_MS - 1), Some(true));
        assert_eq!(kept.get(key, 10_000 - ROUND_KEEP_MS + 1), Some(true));
        assert_eq!(kept.get(key, 10_000 + ROUND_KEEP_MS), None);
        assert_eq!(kept.get(RoundKey { hazards: 4, ..key }, 10_000), None);
    }

    /// A way is kept for its key alone; a new one replaces it; at most [`KEPT`].
    #[test]
    fn a_way_is_kept_for_its_key() {
        let key = Key { stand: 1, home: 2, side: Some(0), fixed: fixed_key(&[true, false]) };
        let mut kept = Ways::default();
        assert_eq!(kept.get(key), None);
        kept.put(key, None);
        assert_eq!(kept.get(key), Some(None));
        assert_eq!(kept.get(Key { side: Some(1), ..key }), None);
        assert_eq!(kept.get(Key { fixed: fixed_key(&[true, true]), ..key }), None);
        kept.put(key, Some(Arc::new(vec![Vec3::new(1.0, 0.0, 0.0)])));
        assert!(kept.get(key).is_some_and(|w| w.is_some()));
        for stand in 10..10 + KEPT as NodeId {
            kept.put(Key { stand, ..key }, None);
        }
        assert_eq!(kept.get(key), None);
        assert_eq!(kept.0.len(), KEPT);
    }
}

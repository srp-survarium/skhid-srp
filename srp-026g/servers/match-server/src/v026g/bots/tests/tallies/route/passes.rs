//! How the bots go through doorways and round stair-landing turns (A66): read from the bodies each tick, nothing in the
//! bots. A *spot* is a place on the route the body must go through with care: the middle of a doorway the route crosses
//! ([`doors::centre_of`]), or a corner of the route that turns 60 degrees or more with a rise of half a metre on one of its
//! legs (a landing). A pass is watched from the tick the body is within [`Kind::radius_m`] of a spot it is going towards to
//! the tick it is that far past it, and counted per kind: the time, the time over the same path at the speed it came in with,
//! the ticks under 1 m/s, the least distance to a wall (and the passes under the 0.41 m the client's controller holds), and
//! the replans in it. Every run prints its own lines, one set per team (`passes_t0_door`, `passes_t1_door`), so the sweep
//! reads them by map and side, and a one-sided run reads the team with the switch against the team without it.

use std::collections::HashMap;

use super::*;
use crate::v026g::bots::doors;
use crate::v026g::bots::tests::door_walk::wall_distance;

/// A turn counts as a landing's from this angle (degrees), with a rise of this much (m) on a leg beside it.
const LANDING_TURN_DEG: f32 = 60.0;
const LANDING_RISE_M: f32 = 0.5;
/// A pass that has not ended after this long (ms) is dropped, counted as lost.
const PASS_LIMIT_MS: u32 = 20_000;
/// The distance to a wall the client's controller holds the body at, m.
const CONTROLLER_HOLD_M: f32 = 0.41;
/// A body that came within this of the spot has gone through it.
const THROUGH_M: f32 = 1.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Door,
    Landing,
}

impl Kind {
    const ALL: [Kind; 2] = [Kind::Door, Kind::Landing];

    fn name(self) -> &'static str {
        match self {
            Kind::Door => "door",
            Kind::Landing => "landing",
        }
    }

    fn radius_m(self) -> f32 {
        match self {
            Kind::Door => 3.0,
            Kind::Landing => 2.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Spot {
    kind: Kind,
    at: Vec3,
}

/// A pass being watched.
#[derive(Clone, Copy, Debug)]
struct Watch {
    team: usize,
    spot: Spot,
    from_ms: u32,
    last_ms: u32,
    last: Vec3,
    enter: Vec3,
    enter_speed: f32,
    path_m: f32,
    slow_ms: u32,
    least_wall_m: f32,
    nearest_m: f32,
    replans: u32,
}

/// The sums of the passes of one kind.
#[derive(Clone, Copy, Debug, Default)]
struct Sums {
    n: u32,
    ms: u64,
    excess_ms: u64,
    path_mm: u64,
    straight_mm: u64,
    slow_ms: u64,
    wall_mm: u64,
    under_hold: u32,
    replans: u32,
    lost: u32,
}

/// A body's spots: those of the route it has, keyed by the route's size and ends.
#[derive(Default)]
struct Spots {
    key: Option<(usize, [i32; 4])>,
    spots: Vec<Spot>,
}

pub(super) struct Passes {
    spots: Vec<Spots>,
    watching: Vec<Option<Watch>>,
    /// By team, then [`Kind`].
    sums: [[Sums; 2]; 2],
    /// `doors::centre_of` of a leg (its ends in quarter metres), kept: the routes replan with the same legs.
    legs: HashMap<[i32; 6], Option<Vec3>>,
}

fn quarter(v: Vec3) -> [i32; 3] {
    [(v.x * 4.0).round() as i32, (v.y * 4.0).round() as i32, (v.z * 4.0).round() as i32]
}

impl Passes {
    pub(super) fn new(bodies: usize) -> Self {
        Self {
            spots: (0..bodies).map(|_| Spots::default()).collect(),
            watching: vec![None; bodies],
            sums: [[Sums::default(); 2]; 2],
            legs: HashMap::new(),
        }
    }

    /// Reads the spots of `route` for the body at `place` if it is not the route read last; whether it was a new one.
    fn read_spots(&mut self, place: usize, route: &path::Route, grid: &mapdata::grid::WalkGrid) -> bool {
        let points = route.points();
        let (Some(first), Some(last)) = (points.first(), points.last()) else {
            return false;
        };
        let (f, l) = (quarter(first.at), quarter(last.at));
        let key = Some((points.len(), [f[0], f[2], l[0], l[2]]));
        if self.spots[place].key == key {
            return false;
        }
        let mut spots = Vec::new();
        for w in points.windows(2) {
            let (a, b) = (w[0].at, w[1].at);
            let (qa, qb) = (quarter(a), quarter(b));
            let centre = *self.legs.entry([qa[0], qa[1], qa[2], qb[0], qb[1], qb[2]]).or_insert_with(|| doors::centre_of(grid, a, b));
            if let Some(at) = centre {
                spots.push(Spot { kind: Kind::Door, at });
            }
        }
        for w in points.windows(3) {
            let (a, b, c) = (w[0].at, w[1].at, w[2].at);
            let (u, v) = ((b.x - a.x, b.z - a.z), (c.x - b.x, c.z - b.z));
            let (lu, lv) = ((u.0 * u.0 + u.1 * u.1).sqrt(), (v.0 * v.0 + v.1 * v.1).sqrt());
            if lu < 1e-3 || lv < 1e-3 {
                continue;
            }
            let turn = ((u.0 * v.0 + u.1 * v.1) / (lu * lv)).clamp(-1.0, 1.0).acos().to_degrees();
            if turn >= LANDING_TURN_DEG && ((b.y - a.y).abs() >= LANDING_RISE_M || (c.y - b.y).abs() >= LANDING_RISE_M) {
                spots.push(Spot { kind: Kind::Landing, at: b });
            }
        }
        self.spots[place] = Spots { key, spots };
        true
    }

    fn end(&mut self, watch: Watch, lost: bool) {
        let sums = &mut self.sums[watch.team][watch.spot.kind as usize];
        if lost {
            sums.lost += 1;
            return;
        }
        let ms = u64::from(watch.last_ms - watch.from_ms);
        let ideal_ms = if watch.enter_speed > 0.5 { (1000.0 * watch.path_m / watch.enter_speed) as u64 } else { ms };
        sums.n += 1;
        sums.ms += ms;
        sums.excess_ms += ms.saturating_sub(ideal_ms);
        sums.path_mm += (watch.path_m * 1000.0) as u64;
        sums.straight_mm += (watch.enter.distance_xz(watch.last) * 1000.0) as u64;
        sums.slow_ms += u64::from(watch.slow_ms);
        sums.wall_mm += (watch.least_wall_m * 1000.0) as u64;
        sums.under_hold += u32::from(watch.least_wall_m < CONTROLLER_HOLD_M);
        sums.replans += watch.replans;
    }
}

impl RouteTopic for Passes {
    fn tick(&mut self, run: &RouteRun, at: &TickAt<'_>) {
        let place = at.place;
        let feet = at.body.feet;
        if !at.body.alive {
            if let Some(watch) = self.watching[place].take() {
                self.end(watch, true);
            }
            return;
        }
        let route = at.bots.bots[place].as_ref().and_then(|bot| bot.route.as_ref()).map(|(route, _)| route);
        let replanned = route.is_some_and(|route| self.read_spots(place, route, at.grid));
        match self.watching[place] {
            Some(mut watch) => {
                let radius = watch.spot.kind.radius_m();
                let dt = at.time.saturating_sub(watch.last_ms);
                let step = feet.distance_xz(watch.last);
                let speed = if dt > 0 { step / (dt as f32 / 1000.0) } else { watch.enter_speed };
                watch.path_m += step;
                if speed < 1.0 {
                    watch.slow_ms += dt;
                }
                watch.replans += u32::from(replanned);
                let to = feet.distance_xz(watch.spot.at);
                watch.nearest_m = watch.nearest_m.min(to);
                if to <= radius {
                    watch.least_wall_m = watch.least_wall_m.min(wall_distance(at.grid, feet));
                }
                watch.last = feet;
                watch.last_ms = at.time;
                let past = (feet.x - watch.spot.at.x) * (watch.spot.at.x - watch.enter.x)
                    + (feet.z - watch.spot.at.z) * (watch.spot.at.z - watch.enter.z)
                    > 0.0;
                let gone = to > radius;
                self.watching[place] = if gone && past && watch.nearest_m <= THROUGH_M {
                    self.end(watch, false);
                    None
                } else if (gone && to > radius + 2.0)
                    || at.time - watch.from_ms > PASS_LIMIT_MS
                    || (feet.y - watch.spot.at.y).abs() > 2.0
                {
                    self.end(watch, true);
                    None
                } else {
                    Some(watch)
                };
            }
            None => {
                let Some(before) = at.before else {
                    return;
                };
                let (dx, dz) = (feet.x - before.x, feet.z - before.z);
                let spot = self.spots[place].spots.iter().copied().find(|spot| {
                    feet.distance_xz(spot.at) <= spot.kind.radius_m()
                        && (feet.y - spot.at.y).abs() <= 2.0
                        && dx * (spot.at.x - feet.x) + dz * (spot.at.z - feet.z) > 0.0
                });
                if let Some(spot) = spot {
                    let speed = (dx * dx + dz * dz).sqrt() / (TICK_MS as f32 / 1000.0);
                    self.watching[place] = Some(Watch {
                        team: usize::from(run.teams[place].unwrap_or(0).min(1)),
                        spot,
                        from_ms: at.time,
                        last_ms: at.time,
                        last: feet,
                        enter: feet,
                        enter_speed: speed,
                        path_m: 0.0,
                        slow_ms: 0,
                        least_wall_m: wall_distance(at.grid, feet),
                        nearest_m: feet.distance_xz(spot.at),
                        replans: 0,
                    });
                }
            }
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        for team in 0..2 {
            for kind in Kind::ALL {
                let s = self.sums[team][kind as usize];
                if s.n + s.lost == 0 {
                    continue;
                }
                let n = s.n.max(1) as f32;
                line.ends.push(format!(
                    "passes_t{team}_{} n {} lost {} mean_s {:.2} excess_s {:.2} path_over_straight {:.2} slow_s {:.2} least_wall_m {:.2} under_hold {} replans {}",
                    kind.name(),
                    s.n,
                    s.lost,
                    s.ms as f32 / n / 1000.0,
                    s.excess_ms as f32 / n / 1000.0,
                    s.path_mm as f32 / (s.straight_mm as f32).max(1.0),
                    s.slow_ms as f32 / n / 1000.0,
                    s.wall_mm as f32 / n / 1000.0,
                    s.under_hold,
                    s.replans,
                ));
            }
        }
    }
}

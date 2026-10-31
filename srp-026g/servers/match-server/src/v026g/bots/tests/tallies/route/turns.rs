//! How the bots round the corners of their routes (A207, `feel.sprint_turns`): read from the bodies and the bots' routes
//! each tick, nothing in the bots. A *corner* is a waypoint of a bot's route whose turn through [`MERGE_M`]
//! ([`Route::turn_through`]) is [`CORNER_DEG`] or more, reached by no jump and leading to none; the waypoints within
//! [`MERGE_M`] after one are part of its turn. It is passed when the body's progress ([`Route::segment`]) goes past it.
//! Each pass is counted by the turn's band (34-60, 60-90, 90-120, 120 degrees and over) and by whether the body held
//! the sprint keys then, and watched for [`WATCH_MS`]: how far the body got to the outside of the corner's legs (an
//! overshoot is [`OVERSHOOT_M`] or more) and whether it fell a metre or more off its route (no drop of it there); the
//! corners on a floor next to a lip or a fall's edge (`WalkGrid::near_lip`, `near_fall`) also apart (A207 x A205: the
//! lip guard and the drop releases must still win over a sprint through a turn). Every fall off the route, by whether the
//! sprint keys were held. And the doorways of the routes (`doors::centre_of` of a leg, the route's door gates, as
//! `passes.rs` finds them): each passed within [`DOOR_M`], and whether at a sprint (A207 x A212: a door wait must be able
//! to stop one). One set of keys per team (`turns_t0_34_60`), so a one-sided run reads the team with the switch against
//! the team without it.

use std::collections::HashMap;

use super::*;
use crate::v026g::bots::doors;
use crate::v026g::bots::kin::{Kinematics, TICK_S};
use crate::v026g::bots::path::Route;

/// A waypoint whose turn is this much or more is a corner, degrees (the motor's `Feel::corner`, 0.6 rad).
const CORNER_DEG: f32 = 34.0;
/// A turn is read over its corner and the legs starting this far after it, metres (`SprintTurns::merge_m`'s default).
const MERGE_M: f32 = 2.0;
/// The bands' upper edges, degrees (the last is open).
const EDGES_DEG: [f32; 3] = [60.0, 90.0, 120.0];
const BANDS: usize = 4;
const BAND_NAMES: [&str; BANDS] = ["34_60", "60_90", "90_120", "120_up"];
/// A pass is watched this long after the corner, ms: 5 m of a sprint.
const WATCH_MS: u32 = 1_000;
/// A body this far or farther to the outside of the corner's legs overshot it, metres.
const OVERSHOOT_M: f32 = 1.0;
/// A doorway is passed when the body comes this near its middle (ground plane, 2 m of height), metres.
const DOOR_M: f32 = 1.0;
/// A doorway passed at this speed or more was entered at a sprint's pace, m/s (a walk is 3.5).
const DOOR_FAST_MS: f32 = 4.2;

/// The route a body follows, as told from the next one, and the segment it was on.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Following {
    first: Vec3,
    end: Vec3,
    points: usize,
    segment: usize,
}

impl Following {
    fn of(route: &Route) -> Self {
        Self { first: route.points()[0].at, end: route.end(), points: route.points().len(), segment: route.segment() }
    }

    fn same(&self, other: &Self) -> bool {
        self.first == other.first && self.end == other.end && self.points == other.points
    }
}

/// A corner passed, watched for [`WATCH_MS`]: its legs (the one into it, the one out and the next if any), which way
/// it turns (left positive), the farthest the body got to its outside, and whether it fell off the route.
#[derive(Clone, Copy, Debug)]
struct Pass {
    at: u32,
    team: usize,
    band: usize,
    sprint: bool,
    legs: [Vec3; 4],
    left: bool,
    /// The corner is on a floor next to a lip or a fall's edge.
    edge: bool,
    out_m: f32,
    fell: bool,
}

impl Pass {
    /// How far `feet` is to the outside of the nearest of the corner's legs, metres (none inside).
    fn outside(&self, feet: Vec3) -> f32 {
        let mut nearest = (f32::INFINITY, 0.0);
        for w in self.legs.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (dx, dz) = (b.x - a.x, b.z - a.z);
            let len2 = dx * dx + dz * dz;
            if len2 < 1e-6 {
                continue;
            }
            let t = (((feet.x - a.x) * dx + (feet.z - a.z) * dz) / len2).clamp(0.0, 1.0);
            let (px, pz) = (a.x + dx * t - feet.x, a.z + dz * t - feet.z);
            let distance = (px * px + pz * pz).sqrt();
            if distance < nearest.0 {
                // The side of the leg the feet are on: positive to the left of its way.
                let side = dx * (feet.z - a.z) - dz * (feet.x - a.x);
                nearest = (distance, side);
            }
        }
        let (distance, side) = nearest;
        let outside = if self.left { side < 0.0 } else { side > 0.0 };
        if distance.is_finite() && outside { distance } else { 0.0 }
    }
}

/// A body's doorways: those of the route it follows, and whether it is within [`DOOR_M`] of each now.
#[derive(Default)]
struct Doors {
    route: Option<Following>,
    at: Vec<Vec3>,
    inside: Vec<bool>,
}

/// By team, band and the sprint at the pass (walking 0, sprinting 1): passes, overshoots, the outside summed in
/// centimetres, falls off the route.
#[derive(Default)]
pub(super) struct Turns {
    following: Vec<Option<Following>>,
    open: Vec<Vec<Pass>>,
    sums: [[[[u64; 4]; 2]; BANDS]; 2],
    /// The same of the corners next to a lip or a fall's edge, by team and sprint.
    edge_sums: [[[u64; 4]; 2]; 2],
    /// By team and the sprint keys at the fall: every fall off the route (at a corner or not), the guard.
    falls: [[u64; 2]; 2],
    /// By team: doorways passed, at a sprint (the keys), at a sprint's pace.
    doors: [[u64; 3]; 2],
    door_watch: Vec<Doors>,
    /// `doors::centre_of` of a leg (its ends in quarter metres), kept: the routes replan with the same legs.
    legs: HashMap<[i32; 6], Option<Vec3>>,
}

impl Turns {
    pub(super) fn new(bodies: usize) -> Self {
        Self {
            following: vec![None; bodies],
            open: vec![Vec::new(); bodies],
            door_watch: (0..bodies).map(|_| Doors::default()).collect(),
            ..Self::default()
        }
    }

    fn close(&mut self, pass: Pass) {
        let add = |sums: &mut [u64; 4]| {
            sums[0] += 1;
            sums[1] += u64::from(pass.out_m >= OVERSHOOT_M);
            sums[2] += (pass.out_m * 100.0).round() as u64;
            sums[3] += u64::from(pass.fell);
        };
        add(&mut self.sums[pass.team][pass.band][usize::from(pass.sprint)]);
        if pass.edge {
            add(&mut self.edge_sums[pass.team][usize::from(pass.sprint)]);
        }
    }

    /// The doorways of the route a body follows (read again when the route is another), and the passes of them: a doorway
    /// is passed when the body comes within [`DOOR_M`] of it.
    fn doorways(&mut self, place: usize, route: &Route, now: &Following, at: &TickAt<'_>, team: usize) {
        let quarter = |v: Vec3| [(v.x * 4.0).round() as i32, (v.y * 4.0).round() as i32, (v.z * 4.0).round() as i32];
        if !self.door_watch[place].route.is_some_and(|was| was.same(now)) {
            let mut doorways = Vec::new();
            for w in route.points().windows(2) {
                let (a, b) = (w[0].at, w[1].at);
                let (qa, qb) = (quarter(a), quarter(b));
                let leg = [qa[0], qa[1], qa[2], qb[0], qb[1], qb[2]];
                doorways.extend(*self.legs.entry(leg).or_insert_with(|| doors::centre_of(at.grid, a, b)));
            }
            for &gate in route.gates() {
                if !doorways.iter().any(|d: &Vec3| d.distance_xz(gate) < 1.0) {
                    doorways.push(gate);
                }
            }
            let inside = vec![false; doorways.len()];
            self.door_watch[place] = Doors { route: Some(*now), at: doorways, inside };
        }
        let feet = at.body.feet;
        let speed = at.before.map_or(0.0, |before| before.distance_xz(feet) / TICK_S);
        let watch = &mut self.door_watch[place];
        for (door, inside) in watch.at.iter().zip(watch.inside.iter_mut()) {
            let now_in = door.distance_xz(feet) <= DOOR_M && (door.y - feet.y).abs() <= 2.0;
            if now_in && !*inside {
                self.doors[team][0] += 1;
                self.doors[team][1] += u64::from(Kinematics::sprints(at.body.keys));
                self.doors[team][2] += u64::from(speed >= DOOR_FAST_MS);
            }
            *inside = now_in;
        }
    }

    /// The corners of `route` passed from waypoint `from` to `to`, by a body of `team` holding `keys` at `time`.
    fn passed(&mut self, place: usize, route: &Route, (from, to): (usize, usize), team: usize, keys: u32, at: &TickAt<'_>) {
        let points = route.points();
        let mut last: Option<Vec3> = None;
        for k in from..=to {
            if k == 0 || k + 1 >= points.len() || points[k].jump.is_some() || points[k + 1].jump.is_some() {
                continue;
            }
            if last.is_some_and(|corner| corner.distance_xz(points[k].at) <= MERGE_M) {
                continue;
            }
            let deg = route.turn_through(k, MERGE_M).to_degrees();
            if deg < CORNER_DEG {
                continue;
            }
            last = Some(points[k].at);
            let (a, b, c) = (points[k - 1].at, points[k].at, points[k + 1].at);
            let d = points.get(k + 2).map_or(c, |p| p.at);
            let left = (b.x - a.x) * (c.z - b.z) - (b.z - a.z) * (c.x - b.x) > 0.0;
            let edge = at.grid.locate(b, 1.0).is_some_and(|node| at.grid.near_lip(node) || at.grid.near_fall(node));
            self.open[place].push(Pass {
                at: at.time,
                team,
                band: EDGES_DEG.iter().take_while(|&&edge| deg >= edge).count(),
                sprint: Kinematics::sprints(keys),
                legs: [a, b, c, d],
                left,
                edge,
                out_m: 0.0,
                fell: false,
            });
        }
    }
}

impl RouteTopic for Turns {
    fn tick(&mut self, run: &RouteRun, at: &TickAt<'_>) {
        let place = at.place;
        let alive = at.body.alive;
        let fell = at.fall.is_some_and(|fall| !fall.planned);
        let team = usize::from(run.teams[place].unwrap_or(0).min(1));
        if fell {
            self.falls[team][usize::from(Kinematics::sprints(at.body.keys))] += 1;
        }
        let mut open = std::mem::take(&mut self.open[place]);
        for pass in &mut open {
            if alive {
                pass.out_m = pass.out_m.max(pass.outside(at.body.feet));
            }
            pass.fell |= fell;
        }
        let (done, watched): (Vec<Pass>, Vec<Pass>) =
            open.into_iter().partition(|pass| !alive || at.time.wrapping_sub(pass.at) >= WATCH_MS);
        self.open[place] = watched;
        for pass in done {
            self.close(pass);
        }
        let route = at.bots.bots.get(place).and_then(Option::as_ref).and_then(|bot| bot.route.as_ref());
        let (Some((route, _)), true) = (route, alive) else {
            self.following[place] = None;
            return;
        };
        if route.points().len() < 2 {
            self.following[place] = None;
            return;
        }
        let now = Following::of(route);
        if let Some(was) = self.following[place].filter(|was| was.same(&now))
            && now.segment > was.segment
        {
            self.passed(place, route, (was.segment + 1, now.segment), team, at.body.keys, at);
        }
        self.doorways(place, route, &now, at, team);
        self.following[place] = Some(now);
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        let open: Vec<Pass> = self.open.iter_mut().flat_map(std::mem::take).collect();
        for pass in open {
            self.close(pass);
        }
        for team in 0..2 {
            let [fall_walk, fall_sprint] = self.falls[team];
            let [door_n, door_sprint, door_fast] = self.doors[team];
            line.tail.push(format!(
                "turns_t{team}_falls_off {} turns_t{team}_falls_off_sprinting {fall_sprint} turns_t{team}_doors n {door_n} sprint {door_sprint} fast {door_fast}",
                fall_walk + fall_sprint
            ));
            let [walk, sprint] = self.edge_sums[team];
            line.tail.push(format!(
                "turns_t{team}_edge n {} sprint {} over_walk {} over_sprint {} out_cm_walk {} out_cm_sprint {} falls_walk {} falls_sprint {}",
                walk[0] + sprint[0],
                sprint[0],
                walk[1],
                sprint[1],
                walk[2],
                sprint[2],
                walk[3],
                sprint[3]
            ));
            for (band, name) in BAND_NAMES.iter().enumerate() {
                let [walk, sprint] = self.sums[team][band];
                line.tail.push(format!(
                    "turns_t{team}_{name} n {} sprint {} over_walk {} over_sprint {} out_cm_walk {} out_cm_sprint {} falls_walk {} falls_sprint {}",
                    walk[0] + sprint[0],
                    sprint[0],
                    walk[1],
                    sprint[1],
                    walk[2],
                    sprint[2],
                    walk[3],
                    sprint[3]
                ));
            }
        }
    }
}

//! Bots standing on a flight of stairs (A240b): the live humans never stopped on one (Vostok Radar Station's spiral stair: 0 stops in
//! 10 traversals), the bots stood there 26 s and 37 s in 24 matches.
//!
//! A floor is on a flight ([`StairFloors`]) when it lies on a `walk` face (the walker mesh's stairs and steep paths,
//! `WalkerBehaviour::Walk`) [`STAIR_MIN_DEG`] to [`STAIR_MAX_DEG`] steep, and the floors a walk reaches within [`AROUND_M`] of it
//! rise [`RISE_M`] or more above it and fall as much below it: the flight, not its ends. (A first count without the face took flat
//! floors beside a step or an object, and hillsides: `flight_look`.) Each tick a living bot's body on such a floor that moved
//! under [`STILL_M`] since the tick before stands there; a stand of [`WAIT_MS`] or more is a wait, counted (with its seconds) by
//! what the bot had when it began: a target (`fighting`), no route (`no_route`), a route with a walk key held (`held`: pinned), or
//! a route without one (`route_still`). A wait of [`LIST_MS`] or more is printed with its place and the bot's play (`FLIGHTWAIT`).

use std::collections::{BTreeMap, HashMap, HashSet};

use mapdata::grid::{LinkKind, NodeId, WalkGrid};

use super::*;

/// A stair's face is this steep at least, degrees (a ramp walked up is under it)…
const STAIR_MIN_DEG: f32 = 20.0;
/// … and at most (a floor face is 60 at most, `grid.rs` `STEEP_NORMAL_Y`).
const STAIR_MAX_DEG: f32 = 60.0;
/// A floor this near a stair face's plane is on it, metres.
const ON_FACE_M: f32 = 0.2;
/// The floors a walk reaches within this of a floor, across, metres…
const AROUND_M: f32 = 1.0;
/// … that rise this far over it and fall as far under it: mid-flight, metres.
const RISE_M: f32 = 0.4;
/// A body that moved under this in a tick stands, metres.
const STILL_M: f32 = 0.02;
/// A stand this long is a wait, ms.
pub(crate) const WAIT_MS: u32 = 2_000;
/// A wait this long is printed, ms.
const LIST_MS: u32 = 5_000;

/// The floors of a map's stair flights (see the module doc), found once a match.
pub(crate) struct StairFloors {
    faces: HashSet<NodeId>,
    flight: HashMap<NodeId, bool>,
}

impl StairFloors {
    pub(crate) fn of(world: &World) -> Self {
        let grid = &world.grid;
        let step = grid.cell_size() * 0.5;
        let mut faces = HashSet::new();
        for t in world.map.walker_triangles() {
            if t.walker != mapdata::WalkerBehaviour::Walk {
                continue;
            }
            let [a, b, c] = t.v;
            let normal = (b - a).cross(c - a);
            let len = normal.length();
            if len < 1e-6 || normal.y.abs() < 1e-6 {
                continue;
            }
            let slope = (normal.y.abs() / len).acos().to_degrees();
            if !(STAIR_MIN_DEG..=STAIR_MAX_DEG).contains(&slope) {
                continue;
            }
            let side = |p: Vec3, q: Vec3, x: f32, z: f32| (q.x - p.x) * (z - p.z) - (q.z - p.z) * (x - p.x);
            let (x0, x1) = (a.x.min(b.x).min(c.x), a.x.max(b.x).max(c.x));
            let (z0, z1) = (a.z.min(b.z).min(c.z), a.z.max(b.z).max(c.z));
            let mut x = x0;
            while x <= x1 {
                let mut z = z0;
                while z <= z1 {
                    let (d1, d2, d3) = (side(a, b, x, z), side(b, c, x, z), side(c, a, x, z));
                    let inside = !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0));
                    if inside {
                        let y = a.y - (normal.x * (x - a.x) + normal.z * (z - a.z)) / normal.y;
                        if let Some(node) = grid.locate(Vec3::new(x, y, z), ON_FACE_M)
                            && (grid.position(node).y - y).abs() <= ON_FACE_M
                        {
                            faces.insert(node);
                        }
                    }
                    z += step;
                }
                x += step;
            }
        }
        Self { faces, flight: HashMap::new() }
    }

    /// Every mid-flight floor's place.
    pub(crate) fn floors(&mut self, grid: &WalkGrid) -> Vec<Vec3> {
        let mut faces: Vec<NodeId> = self.faces.iter().copied().collect();
        faces.sort_unstable();
        faces.into_iter().filter(|&node| self.mid_flight(grid, node)).map(|node| grid.position(node)).collect()
    }

    /// Whether the floor under `feet` is mid-flight.
    pub(crate) fn under(&mut self, grid: &WalkGrid, feet: Vec3) -> bool {
        grid.locate(feet, 1.0).is_some_and(|node| self.mid_flight(grid, node))
    }

    fn mid_flight(&mut self, grid: &WalkGrid, node: NodeId) -> bool {
        if !self.faces.contains(&node) {
            return false;
        }
        *self.flight.entry(node).or_insert_with(|| {
            let at = grid.position(node);
            let (mut low, mut high) = (at.y, at.y);
            let mut seen = vec![node];
            let mut next = vec![node];
            while let Some(floor) = next.pop() {
                for (to, kind) in grid.links(floor) {
                    if kind != LinkKind::Walk || seen.contains(&to) {
                        continue;
                    }
                    let place = grid.position(to);
                    if place.distance_xz(at) > AROUND_M {
                        continue;
                    }
                    seen.push(to);
                    next.push(to);
                    low = low.min(place.y);
                    high = high.max(place.y);
                }
            }
            high - at.y >= RISE_M && at.y - low >= RISE_M
        })
    }
}

/// A stand on a flight: since when, where, and what the bot had then.
#[derive(Clone)]
struct Stand {
    since: u32,
    at: Vec3,
    what: &'static str,
    play: String,
}

pub(super) struct Flights {
    stairs: Option<StairFloors>,
    stands: Vec<Option<Stand>>,
    /// Ticks of a living bot's body on a flight, and of those standing.
    ticks: [u64; 2],
    /// Waits by what, and their seconds.
    waits: BTreeMap<&'static str, (u32, f32)>,
}

impl Flights {
    pub(super) fn new(bodies: usize) -> Self {
        Self { stairs: None, stands: vec![None; bodies], ticks: [0; 2], waits: BTreeMap::new() }
    }

    fn end(&mut self, place: usize, time: u32) {
        let Some(stand) = self.stands[place].take() else {
            return;
        };
        let ms = time.saturating_sub(stand.since);
        if ms < WAIT_MS {
            return;
        }
        let wait = self.waits.entry(stand.what).or_default();
        wait.0 += 1;
        wait.1 += ms as f32 / 1000.0;
        if ms >= LIST_MS {
            eprintln!(
                "FLIGHTWAIT t={} place={place} at=({:.1},{:.1},{:.1}) s={:.1} what={} play={}",
                stand.since,
                stand.at.x,
                stand.at.y,
                stand.at.z,
                ms as f32 / 1000.0,
                stand.what,
                stand.play
            );
        }
    }
}

impl RouteTopic for Flights {
    fn tick(&mut self, _run: &RouteRun, at: &TickAt<'_>) {
        if self.stands.len() <= at.place {
            self.stands.resize(at.place + 1, None);
        }
        let Some(bot) = at.bots.bots[at.place].as_ref() else {
            return;
        };
        let Some(tactics) = at.bots.tactics.as_ref() else {
            return;
        };
        let stairs = self.stairs.get_or_insert_with(|| StairFloors::of(&tactics.world));
        let on = at.body.alive && stairs.under(at.grid, at.body.feet);
        let still = on && at.before.is_some_and(|before| before.distance_xz(at.body.feet) < STILL_M);
        self.ticks[0] += u64::from(on);
        self.ticks[1] += u64::from(still);
        if !still {
            self.end(at.place, at.time);
            return;
        }
        if self.stands[at.place].is_none() {
            let what = if at.body.target.is_some() {
                "fighting"
            } else if bot.route.is_none() {
                "no_route"
            } else if at.body.keys & crate::v026g::bots::stuck::HELD_KEYS != 0 {
                "held"
            } else {
                "route_still"
            };
            let play = bot.assignment.as_ref().map_or("none".into(), |a| format!("{:?}", a.kind));
            self.stands[at.place] = Some(Stand { since: at.time, at: at.body.feet, what, play });
        }
    }

    fn finish(&mut self, game: &Game) {
        for place in 0..self.stands.len() {
            self.end(place, game.tick);
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        let waits: Vec<String> = std::mem::take(&mut self.waits).into_iter().map(|(what, (n, s))| format!("{what}={n}/{s:.0}")).collect();
        line.tail.push(format!(
            "flight_ticks {} flight_still_ticks {} flight_waits {}",
            self.ticks[0],
            self.ticks[1],
            if waits.is_empty() { "-".to_owned() } else { waits.join(",") }
        ));
        self.ticks = [0; 2];
    }
}

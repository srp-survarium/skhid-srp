//! Bots standing on a flight of stairs (A240b): the live humans never stopped on one (Vostok Radar Station's spiral stair: 0 stops in
//! 10 traversals), the bots stood there 26 s and 37 s in 24 matches.
//!
//! A floor is on a flight when the floors a walk reaches within [`AROUND_M`] of it rise [`RISE_M`] or more above it and fall as much
//! below it: a slope of 27 degrees or more both ways (a stair's flight, or its turning landing), not the floors at either end. Each
//! tick a living bot's body on such a floor that moved under [`STILL_M`] since the tick before stands there; a stand of [`WAIT_MS`]
//! or more is a wait, counted (with its seconds) by what the bot had when it began: a target (`fighting`), no route (`no_route`), a
//! route with a walk key held (`held`: pinned), or a route without one (`route_still`). A wait of [`LIST_MS`] or more is printed
//! with its place and the bot's play (`FLIGHTWAIT`).

use std::collections::{BTreeMap, HashMap};

use super::*;

/// The floors a walk reaches within this of a floor, across, metres.
const AROUND_M: f32 = 1.0;
/// … that rise this far over it and fall as far under it: a flight, metres.
const RISE_M: f32 = 0.5;
/// A body that moved under this in a tick stands, metres.
const STILL_M: f32 = 0.02;
/// A stand this long is a wait, ms.
const WAIT_MS: u32 = 2_000;
/// A wait this long is printed, ms.
const LIST_MS: u32 = 5_000;

/// A stand on a flight: since when, where, and what the bot had then.
#[derive(Clone)]
struct Stand {
    since: u32,
    at: Vec3,
    what: &'static str,
    play: String,
}

pub(super) struct Flights {
    /// Per floor, whether it is on a flight (found once).
    flight: HashMap<mapdata::grid::NodeId, bool>,
    stands: Vec<Option<Stand>>,
    /// Ticks of a living bot's body on a flight, and of those standing.
    ticks: [u64; 2],
    /// Waits by what, and their seconds.
    waits: BTreeMap<&'static str, (u32, f32)>,
}

impl Flights {
    pub(super) fn new(bodies: usize) -> Self {
        Self { flight: HashMap::new(), stands: vec![None; bodies], ticks: [0; 2], waits: BTreeMap::new() }
    }

    fn on_flight(&mut self, grid: &mapdata::grid::WalkGrid, feet: Vec3) -> bool {
        let Some(node) = grid.locate(feet, 1.0) else {
            return false;
        };
        *self.flight.entry(node).or_insert_with(|| {
            let at = grid.position(node);
            let (mut low, mut high) = (at.y, at.y);
            let mut seen = vec![node];
            let mut next = vec![node];
            while let Some(floor) = next.pop() {
                for (to, kind) in grid.links(floor) {
                    if kind != mapdata::grid::LinkKind::Walk || seen.contains(&to) {
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
        let on = at.body.alive && self.on_flight(at.grid, at.body.feet);
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

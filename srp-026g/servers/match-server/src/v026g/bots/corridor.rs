//! The first-leg corridor cost (A151): a body that came alive lately plans its way with the regions its mates were in
//! lately costing more, so the second and third bot out of an exit take the next corridor when that is not much longer.
//!
//! The respawn-wave check (fx-cover, 2026-10-29): wave mates walk the same way as live players at 10 s (74 % against
//! 70 %) and as fast (24.3 m against 23.9 m), but of the pairs together at 10 s 21 % of the bots' are apart by 20 s,
//! the live players' 36 %: the bots' goals differ, their routes to them share a longer first leg.
//!
//! The commander tells each bot of the team where the team's bodies were in the last `trail_s` seconds, by region
//! ([`Traffic`]: a bit a mate in the region's mask; what a player sees of his mates, not their intent). A bot younger than
//! `young_s` (not at the match's opening) whose plain way runs `shared_m` metres or more through regions mates were in
//! plans again with a region costing `1 + step` times its length for each such mate (up to `max_mates`) over a room of
//! regions weighed the same way ([`threat::weighed_corridor`], HPA*'s abstract path, then the refined one); the costed
//! way is kept when it is at most `detour` longer than the plain one and runs through fewer such regions.
//!
//! Best practice: Killzone 2's multiplayer bots shape a squad's path cost by the paths of the squads before it ("squad
//! corridors") and give a squad no regroup point another squad uses (Guerrilla, Verweij's bot system slides 58, 67-73).

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering::Relaxed;

use super::features::Feature;
use super::path::Route;
use super::path::RouteHeight;
use super::*;

/// The opening of a match is no respawn: a body that came alive within this of the clock's start plans as it always did
/// (the opening is left out of the life-age buckets too, `tests/tallies/owner/spacing.rs`), ms.
pub const OPENING_MS: u32 = 5_000;
/// How far along its plain way the cost looks, metres, and every how many.
pub const HORIZON_M: f32 = 60.0;
pub const STEP_M: f32 = 2.0;

/// Where a team's bodies were lately, by region: the places (a bit each; a place past 63 is not counted) in each.
#[derive(Debug, Default, PartialEq)]
pub struct Traffic {
    regions: HashMap<u32, u64>,
}

impl Traffic {
    fn bit(place: Place) -> u64 {
        if place < 64 { 1 << place } else { 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// The mates other than `who` that were in `region`.
    pub fn mates_on(&self, region: u32, who: Place) -> u32 {
        self.regions.get(&region).map_or(0, |mask| (mask & !Self::bit(who)).count_ones())
    }

    /// What a region costs `who` times its length: `1 + step` for each mate on it, up to `max` mates.
    pub fn weight(&self, region: u32, who: Place, step: f32, max: u32) -> f32 {
        1.0 + step * self.mates_on(region, who).min(max) as f32
    }
}

/// A team's bodies' regions, kept for `trail_s`: one entry when a body is first seen in a region, and again each second it
/// stays.
#[derive(Default)]
pub struct TeamTrails {
    seen: VecDeque<(u32, Place, u32)>,
    last: HashMap<Place, (u32, u32)>,
}

impl Feature for TeamTrails {}

impl TeamTrails {
    /// This tick's bodies (their places and regions): the traffic of the last `trail_ms`, these bodies' regions now included.
    pub fn tick(&mut self, now: u32, trail_ms: u32, bodies: &[(Place, u32)]) -> Traffic {
        for &(place, region) in bodies {
            match self.last.get(&place) {
                Some(&(was, at)) if was == region && now.saturating_sub(at) < 1_000 => {}
                _ => {
                    self.seen.push_back((now, place, region));
                    self.last.insert(place, (region, now));
                }
            }
        }
        while self.seen.front().is_some_and(|&(at, ..)| now.saturating_sub(at) > trail_ms) {
            self.seen.pop_front();
        }
        let mut regions: HashMap<u32, u64> = HashMap::new();
        for &(_, place, region) in &self.seen {
            *regions.entry(region).or_default() |= Traffic::bit(place);
        }
        for &(place, region) in bodies {
            *regions.entry(region).or_default() |= Traffic::bit(place);
        }
        Traffic { regions }
    }
}

/// What a bot reads of its team's traffic. Not reset at a new life: the commander tells it every tick, and a body that
/// came alive plans its first way on what the last tick told.
#[derive(Default)]
pub struct BotState {
    pub traffic: Arc<Traffic>,
}

impl Feature for BotState {}

/// The metres of the first [`HORIZON_M`] of `route`, sampled every [`STEP_M`], that run through regions `who`'s mates were in.
pub fn shared_m(route: &Route, height: RouteHeight, region_at: impl Fn(Vec3) -> Option<u32>, traffic: &Traffic, who: Place) -> f32 {
    let length = route.length().min(HORIZON_M);
    let (mut metres, mut s) = (0.0, 0.0);
    while s <= length {
        if region_at(route.at(s, height)).is_some_and(|region| traffic.mates_on(region, who) > 0) {
            metres += STEP_M;
        }
        s += STEP_M;
    }
    metres
}

/// What the cost did, by team (the owner tests' `first_leg` topic reads it).
#[derive(Debug, Default)]
pub struct Tally {
    /// Plans of the first way of a respawned body younger than `young_s` (the cost's candidates).
    pub young_plans: [AtomicU64; 2],
    /// ... whose plain way ran `shared_m` metres or more through regions mates were in: the search was made.
    pub searched: [AtomicU64; 2],
    /// The search's ends: a costed way kept; too long (over `detour`); not through fewer regions mates were in; no way; out
    /// of floors.
    pub kept: [AtomicU64; 2],
    pub too_long: [AtomicU64; 2],
    pub not_better: [AtomicU64; 2],
    pub no_way: [AtomicU64; 2],
    pub budget: [AtomicU64; 2],
    /// Floors the searches expanded.
    pub floors: [AtomicU64; 2],
    /// Metres the kept ways are longer than the plain ones, and the metres through regions mates were in before and after
    /// (summed over the kept ways, in millimetres).
    pub detour_mm: [AtomicU64; 2],
    pub shared_before_mm: [AtomicU64; 2],
    pub shared_after_mm: [AtomicU64; 2],
}

impl Tally {
    pub fn add(counter: &[AtomicU64; 2], team: u8) {
        Self::add_n(counter, team, 1);
    }

    pub fn add_n(counter: &[AtomicU64; 2], team: u8, n: u64) {
        counter[usize::from(team.min(1))].fetch_add(n, Relaxed);
    }

    pub fn read(counter: &[AtomicU64; 2]) -> [u64; 2] {
        [counter[0].load(Relaxed), counter[1].load(Relaxed)]
    }
}

/// The costed way of a body that came alive lately, if it is worth taking over its `plain` way: the plain way's metres
/// through regions mates were in lately are looked at (under `shared_m`: nothing), a room of regions is weighed, the floors
/// searched in it with the same weights; kept when at most `detour` longer and through fewer such regions. The floors the
/// search expands are booked to the poll (`floors`, then past them).
#[allow(clippy::too_many_arguments)]
pub(super) fn costed_way(
    tactics: &Tactics,
    bot: &Bot,
    rule: tuning::FirstLegRule,
    plain: &Route,
    ends: (NodeId, NodeId),
    time: u32,
    side: Option<i32>,
    objective: bool,
    filter: &RouteFilter,
    floors: &mut usize,
    overdraft: &mut super::search::Overdraft,
) -> Option<Route> {
    let (grid, regions, tally, team) = (&tactics.world.grid, tactics.regions(), &tactics.corridor_tally, bot.team);
    let traffic = bot.features.corridor.traffic.clone();
    let region_at = |p: Vec3| grid.locate(p, 1.5).map(|node| regions.of(node));
    let heights = bot.motor.heights();
    Tally::add(&tally.young_plans, team);
    let before = shared_m(plain, heights, region_at, &traffic, bot.place);
    if before < rule.shared_m {
        return None;
    }
    Tally::add(&tally.searched, team);
    let (n0, n1) = ends;
    let start = grid.position(n0);
    let weight = |region: u32| traffic.weight(region, bot.place, rule.step, rule.max_mates);
    let Some(room) = threat::weighed_corridor(grid, regions, regions.of(n0), regions.of(n1), weight, |region| weight(region) > 1.0, |region| {
        tactics.region_allowed_from(region, time, side, objective, start)
    }) else {
        Tally::add(&tally.no_way, team);
        return None;
    };
    let region_of = tactics.region_lookup();
    let costed = filter
        .clone()
        .weighing_traffic(traffic.clone(), bot.place, rule.step, rule.max_mates, region_of.clone())
        .keeping_to(Arc::new(room), region_of);
    let mut mine = rule.floors;
    let way = tactics.start_route(n0, n1, time, side, objective, &costed, &mut mine);
    let used = rule.floors - mine;
    overdraft.spend(floors, used);
    Tally::add_n(&tally.floors, team, used as u64);
    match way {
        Ok(Some(round)) if round.length() > plain.length() * (1.0 + rule.detour) => {
            Tally::add(&tally.too_long, team);
            None
        }
        Ok(Some(round)) => {
            let after = shared_m(&round, heights, region_at, &traffic, bot.place);
            if after + STEP_M > before {
                Tally::add(&tally.not_better, team);
                return None;
            }
            Tally::add(&tally.kept, team);
            Tally::add_n(&tally.detour_mm, team, ((round.length() - plain.length()).max(0.0) * 1000.0) as u64);
            Tally::add_n(&tally.shared_before_mm, team, (before * 1000.0) as u64);
            Tally::add_n(&tally.shared_after_mm, team, (after * 1000.0) as u64);
            Some(round)
        }
        Ok(None) => {
            Tally::add(&tally.no_way, team);
            None
        }
        Err(_) => {
            Tally::add(&tally.budget, team);
            None
        }
    }
}

/// A team's commander tick: tells each of the team's bots where the team's bodies were lately.
pub(super) fn tick(tactics: &Tactics, bots: &mut [Option<Bot>], state: &mut TeamState, team: u8, view: &MatchView, now: u32, rule: tuning::FirstLegRule) {
    let grid = &tactics.world.grid;
    let regions = tactics.regions();
    let bodies: Vec<(Place, u32)> = view
        .team(team)
        .filter(|p| p.alive)
        .filter_map(|p| grid.locate(p.feet, 1.5).map(|node| (p.place, regions.of(node))))
        .collect();
    let traffic = Arc::new(state.features.corridor.tick(now, rule.trail_ms, &bodies));
    for bot in bots.iter_mut().flatten().filter(|b| b.team == team) {
        bot.features.corridor.traffic = traffic.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mate_counts_in_a_region_for_the_trail_and_not_for_himself() {
        let mut trails = TeamTrails::default();
        let first = trails.tick(0, 15_000, &[(1, 10), (2, 10), (3, 11)]);
        assert_eq!(first.mates_on(10, 1), 1, "bot 2 is on the region 1 plans through");
        assert_eq!(first.mates_on(10, 5), 2);
        assert_eq!(first.mates_on(12, 5), 0);
        assert!(Traffic::default().is_empty() && !first.is_empty());
        // Bot 2 walks on to 12; the region it left still has it for the trail, then not.
        let later = trails.tick(5_000, 15_000, &[(1, 10), (2, 12), (3, 11)]);
        assert_eq!((later.mates_on(10, 5), later.mates_on(12, 5)), (2, 1));
        let gone = trails.tick(21_000, 15_000, &[(1, 13), (2, 14), (3, 15)]);
        assert_eq!((gone.mates_on(10, 5), gone.mates_on(12, 5), gone.mates_on(13, 5)), (0, 0, 1));
    }

    #[test]
    fn the_cost_is_a_step_a_mate_up_to_the_most() {
        let mut trails = TeamTrails::default();
        let traffic = trails.tick(0, 15_000, &[(1, 4), (2, 4), (3, 4), (4, 4), (5, 4)]);
        assert_eq!(traffic.weight(4, 1, 0.5, 3), 2.5, "three of the four mates count");
        assert_eq!(traffic.weight(4, 1, 0.5, 9), 3.0);
        assert_eq!(traffic.weight(7, 1, 0.5, 3), 1.0, "a region no mate was in costs its length");
        assert_eq!(traffic.weight(4, 99, 0.5, 1), 1.5, "a place past the mask counts as no mate of its own");
    }

    #[test]
    fn a_body_that_stays_is_in_its_region_whatever_the_trail() {
        let mut trails = TeamTrails::default();
        let _ = trails.tick(0, 5_000, &[(1, 3)]);
        let still = trails.tick(30_000, 5_000, &[(1, 3)]);
        assert_eq!(still.mates_on(3, 2), 1, "the trail ran out but the body is there now");
    }
}

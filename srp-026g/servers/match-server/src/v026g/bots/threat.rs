//! Threat spots (K1, `campers.md`): where a shooter has killed the team's
//! bots from, remembered by the team so that routes keep out of his line.
//!
//! Root cause (PROVEN by code, `campers.md` K1): no memory held "a shooter
//! kills from there". A track dies with the bot, the team kept only the
//! victims' spots for 30 s, and no route or spot choice read any danger, so
//! bots walked into a known shooter's line (33 % of the entries into the
//! owner's held lines were fatal).
//!
//! A death of a bot of the team whose killer the team sensed in the 5 s
//! before ([`SENSED_MS`]) adds to a spot at the killer's place; deaths
//! within [`MERGE_M`] of one add up. A spot is *held* when it has killed
//! twice, or once with its shooter sensed on it lately. What a held spot
//! *covers* is the wedge ([`LANE_DEG`]) round the bearing of each of his
//! victims, as far as he sees from his standing eye to each region's
//! middle: his lanes.
//!
//! Routes: the plain route is searched first. If more than [`EXPOSED_M`] of
//! its first [`HORIZON_M`] runs through a lane ([`exposure`], a function of
//! the route and the ground seen, shared with whoever else needs it), a
//! local detour round the lane is searched and spliced in where it is not
//! much longer ([`super::route_filter::RouteFilter::avoiding_threat`], a soft
//! closure; the search is capped, `ThreatTuning::search_floors`); otherwise
//! the plain route is kept and the bot runs through the lane
//! (`ThreatTuning::sprint`): it crosses fast and keeps its task. A route that
//! crosses no lane costs the check and no search. Only what the team's own
//! senses and the kill feed gave goes in: nothing is read from the enemy's
//! state (no cheat).
//!
//! Best practice: Valve's per-team danger from deaths with a decay (Source
//! SDK `nav_area.cpp`), Killzone's costs on links under fire (Beij,
//! Straatman, GDC Europe 2005), Dave Mark's direct-fire threat spread only
//! to what the agent sees (Game AI Pro 2 ch. 30). Where it differs: a soft
//! closure with a fallback instead of a cost, for `Tactics::start_route`
//! takes a yes/no filter (`campers.md` K1 §3); and the plain route first,
//! the avoiding one only past an exposure (Darken's cheap test first, the
//! Close Quarters AI article's flank around the exposed nodes only).

use mapdata::Vec3;

use super::path::Route;
use super::tactics::Tactics;
use super::view::{Place, eye_height};

/// A death adds to a spot within this of it, m.
pub const MERGE_M: f32 = 8.0;
/// A killer sensed this long before the death gives the spot, ms.
pub const SENSED_MS: u32 = 5_000;
/// A spot's strength falls this much a second: one death is forgotten in
/// two minutes (Valve).
pub const DECAY_PER_S: f32 = 1.0 / 120.0;
/// Strength at which a spot is held, and with its shooter sensed on it in
/// the last [`SEEN_MS`], the least that holds it.
pub const HELD_STRENGTH: f32 = 1.5;
pub const HELD_SEEN_STRENGTH: f32 = 0.8;
/// The shooter counts as on the spot within this of it, m, for this long
/// after he was sensed there, ms.
pub const SEEN_ON_M: f32 = 8.0;
pub const SEEN_MS: u32 = 15_000;
/// His death takes this share of the strength (he usually comes back).
pub const SHOOTER_DIED_KEEPS: f32 = 0.5;
/// The reach of a spot: at least this, or the longest kill from it and this
/// more, m.
pub const REACH_FLOOR_M: f32 = 40.0;
pub const REACH_PLUS_M: f32 = 15.0;
/// A spot is cleared when a bot of the team has a line to it within this,
/// m, for [`CLEAR_MS`], and senses no enemy within [`SEEN_ON_M`] of it.
pub const CLEAR_LINE_M: f32 = 40.0;
pub const CLEAR_MS: u32 = 3_000;
/// Teammates asked whether they see a spot empty, at most.
pub const WATCHERS_ASKED: usize = 3;
/// What a spot covers is the wedge of this many degrees either side of the
/// bearing of each of its victims, and what he sees there: the lanes he
/// killed in (44 of the 51 entries after the owner's first kill from a hold
/// on Vostok Radar Station came in a lane he had already killed in,
/// `campers.md` Q2). The first version kept out all he saw within reach, and
/// searched round it at every replan: floors expanded +8 to +20 % and 4.5 to
/// 8.6 % fewer kills in the stand-in camper smoke (seeds 1-4, off against on).
pub const LANE_DEG: f32 = 20.0;
/// A route is searched again round the covered regions when more than this
/// of its first [`HORIZON_M`] runs through them, m.
pub const EXPOSED_M: f32 = 8.0;
pub const HORIZON_M: f32 = 100.0;
/// A local detour round a lane is searched from this far before its first
/// exposed stretch to this far after its last, m: a little more than
/// [`FREE_M`], so the lane itself is closed in the search.
pub const DETOUR_MARGIN_M: f32 = 13.0;
/// The route is sampled this often, m.
pub const STEP_M: f32 = 3.0;
/// Ground this near the spot is in every lane, m.
pub const LANE_CLOSE_M: f32 = 3.0;
/// An enemy sensed this recently counts as there now, ms.
pub const NOW_MS: u32 = 1_000;
/// Rays cast at the regions of a newly held spot in one tick.
pub const RAYS_PER_TICK: usize = 64;
/// Height of the point a ray aims at in a region's middle, m (a chest).
pub const TARGET_HEIGHT_M: f32 = 1.0;
/// Bearings of a spot's victims kept.
const BEARINGS_KEPT: usize = 16;
/// A spot is dropped under this strength.
const GONE_STRENGTH: f32 = 0.05;
/// The floors of a route's ends this near are never kept out, m: a bot in a
/// covered spawn, or going to a covered goal, still moves.
pub const FREE_M: f32 = 10.0;

/// Where a route runs through ground someone sees (a shooter's lane, `K1`;
/// the sight of a shooter who outranges a bot, fighting's X3): the metres
/// exposed in all, and each stretch as (from, to) metres along the route.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Exposure {
    pub metres: f32,
    pub spans: Vec<(f32, f32)>,
}

/// The exposure of the first [`HORIZON_M`] of `route`, sampled every
/// [`STEP_M`]: a sample is exposed when `exposed` says its point is, unless
/// it lies within [`FREE_M`] of one of `free` (the ends of the way: a bot
/// in a covered spawn, or going to a covered goal, still moves). The one
/// function of a route and the ground seen; what is seen is the caller's
/// ([`Covered::exposed_at`] for the threat spots).
pub fn exposure(route: &Route, exposed: impl Fn(Vec3) -> bool, free: &[Vec3]) -> Exposure {
    let length = route.length().min(HORIZON_M);
    let mut out = Exposure::default();
    let mut open: Option<f32> = None;
    let mut s = 0.0;
    while s <= length {
        let p = route.point_at(s);
        let seen = !free.iter().any(|f| f.distance_xz(p) < FREE_M) && exposed(p);
        match (seen, open) {
            (true, None) => open = Some(s),
            (false, Some(from)) => {
                out.spans.push((from, s - STEP_M));
                open = None;
            }
            _ => {}
        }
        out.metres += if seen { STEP_M } else { 0.0 };
        s += STEP_M;
    }
    if let Some(from) = open {
        out.spans.push((from, s - STEP_M));
    }
    out
}

/// Where the first [`HORIZON_M`] of `route` runs through any of the discs
/// `(centre, radius)` (small hazards such as a trap, which sampling every
/// [`STEP_M`] would step over): each leg is tested against each disc on the
/// ground plane, so the spans are exact to the leg. Spans are in metres along
/// the route, in order; overlapping ones are merged.
pub fn hits(route: &Route, discs: &[(Vec3, f32)]) -> Exposure {
    let mut spans: Vec<(f32, f32)> = Vec::new();
    let mut along = 0.0_f32;
    for leg in route.points().windows(2) {
        let (a, b) = (leg[0].at, leg[1].at);
        let length = a.distance_xz(b);
        if along > HORIZON_M {
            break;
        }
        if length > 1e-4 {
            let (dx, dz) = ((b.x - a.x) / length, (b.z - a.z) / length);
            for &(centre, radius) in discs {
                // The leg's distance from `a` where it is within `radius` of
                // the centre: the chord of the circle on the line.
                let (rx, rz) = (centre.x - a.x, centre.z - a.z);
                let t = rx * dx + rz * dz;
                let off2 = rx * rx + rz * rz - t * t;
                if off2 > radius * radius {
                    continue;
                }
                let half = (radius * radius - off2).sqrt();
                let (from, to) = ((t - half).max(0.0), (t + half).min(length));
                if from <= to {
                    spans.push((along + from, along + to));
                }
            }
        }
        along += length;
    }
    spans.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for (from, to) in spans {
        match merged.last_mut() {
            Some(last) if from <= last.1 => last.1 = last.1.max(to),
            _ => merged.push((from, to)),
        }
    }
    let metres = merged.iter().map(|(from, to)| to - from).sum();
    Exposure { metres, spans: merged }
}

#[cfg(test)]
thread_local! {
    static CHECKED: std::cell::Cell<[u64; 2]> = const { std::cell::Cell::new([0; 2]) };
}

/// A route was looked at for exposure; the bench counts them and those
/// exposed ([`EXPOSED_M`]).
#[cfg(test)]
pub fn checked(seen: &Exposure) {
    CHECKED.with(|c| {
        let mut n = c.get();
        n[0] += 1;
        n[1] += u64::from(seen.metres > EXPOSED_M);
        c.set(n);
    });
}

#[cfg(not(test))]
pub fn checked(_: &Exposure) {}

/// The routes counted by [`checked`] so far and those exposed.
#[cfg(test)]
pub fn checks() -> [u64; 2] {
    CHECKED.with(|c| c.get())
}

/// How a route search with the covered regions kept out ended: a way round
/// (or none needed), none (the plain search follows), or one much longer than
/// the way through (which is taken).
#[derive(Clone, Copy, Debug)]
pub enum Searched {
    Round,
    NoWay,
    TooLong,
    /// The floors it may take ran out first.
    Budget,
}

#[cfg(test)]
thread_local! {
    static SEARCHES: std::cell::Cell<[u64; 4]> = const { std::cell::Cell::new([0; 4]) };
}

/// A route search was made with the covered regions kept out as a
/// preference (the bench's count).
#[cfg(test)]
pub fn searched(how: Searched) {
    SEARCHES.with(|c| {
        let mut n = c.get();
        n[how as usize] += 1;
        c.set(n);
    });
}

#[cfg(not(test))]
pub fn searched(_: Searched) {}

#[cfg(test)]
thread_local! {
    static ROUNDED: std::cell::RefCell<std::collections::HashMap<usize, u32>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The bot at `place` took a route round the covered regions at `time` (the
/// bench's count of what follows).
#[cfg(test)]
pub fn rounded(place: usize, time: u32) {
    ROUNDED.with(|m| {
        m.borrow_mut().insert(place, time);
    });
}

#[cfg(not(test))]
pub fn rounded(_: usize, _: u32) {}

/// Whether the bot at `place` took such a route within `ms` before `time`.
#[cfg(test)]
pub fn rounded_within(place: usize, time: u32, ms: u32) -> bool {
    ROUNDED.with(|m| m.borrow().get(&place).is_some_and(|&t| time.saturating_sub(t) <= ms))
}

/// The searches counted by [`searched`] so far, by [`Searched`] in order.
#[cfg(test)]
pub fn searches() -> [u64; 4] {
    SEARCHES.with(|c| c.get())
}

/// What the spots read of the map: what is seen from where, and the
/// regions' middles.
pub trait Eyes {
    fn sees(&self, from: Vec3, to: Vec3) -> bool;
    /// Every region and the point at its middle.
    fn region_points(&self) -> Vec<(u32, Vec3)>;
}

impl Eyes for Tactics {
    fn sees(&self, from: Vec3, to: Vec3) -> bool {
        Tactics::sees(self, from, to)
    }

    fn region_points(&self) -> Vec<(u32, Vec3)> {
        let regions = self.regions();
        (0..regions.len() as u32)
            .map(|r| (r, self.world.grid.position(regions.middle(r))))
            .collect()
    }
}

/// The covered regions kept open for a route from floor `from` to floor
/// `to`: those with a middle within [`FREE_M`] of an end, and the regions
/// of the two ends.
pub fn free_regions(covered: &Covered, tactics: &Tactics, from: mapdata::NodeId, to: mapdata::NodeId) -> Vec<u32> {
    let (grid, regions) = (&tactics.world.grid, tactics.regions());
    let ends = [grid.position(from), grid.position(to)];
    let mut free: Vec<u32> = covered
        .regions()
        .iter()
        .copied()
        .filter(|&r| {
            let middle = grid.position(regions.middle(r));
            ends.iter().any(|end| end.distance_xz(middle) < FREE_M)
        })
        .collect();
    free.extend([regions.of(from), regions.of(to)]);
    free
}

/// Whether `p` lies within [`LANE_DEG`] of the bearing of one of the victims
/// seen from `at`.
fn in_lane(bearings: &[f32], at: Vec3, p: Vec3) -> bool {
    // His own ground has no bearing.
    if p.distance_xz(at) < LANE_CLOSE_M {
        return true;
    }
    let bearing = (p.x - at.x).atan2(p.z - at.z);
    bearings.iter().any(|&b| {
        let d = (bearing - b + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        d.abs() <= LANE_DEG.to_radians()
    })
}

/// One place a shooter killed from.
#[derive(Clone, Debug)]
pub struct Spot {
    pub at: Vec3,
    strength: f32,
    stamp: u32,
    pub shooter: Place,
    /// Where the victims were seen from the spot, yaw.
    pub bearings: Vec<f32>,
    pub reach_m: f32,
    shooter_seen: Option<u32>,
    empty_since: Option<u32>,
    /// The regions within reach still to be cast at, and those seen.
    todo: Option<Vec<(u32, Vec3)>>,
    seen: Vec<u32>,
    cast: bool,
}

impl Spot {
    fn strength_at(&self, now: u32) -> f32 {
        let s = now.saturating_sub(self.stamp) as f32 / 1000.0;
        (self.strength - s * DECAY_PER_S).max(0.0)
    }

    /// Whether the spot is held at `now`.
    pub fn held(&self, now: u32) -> bool {
        let strength = self.strength_at(now);
        strength >= HELD_STRENGTH
            || (strength >= HELD_SEEN_STRENGTH
                && self.shooter_seen.is_some_and(|t| now.saturating_sub(t) <= SEEN_MS))
    }

    /// The regions of his line, once all cast.
    pub fn covered(&self) -> Option<&[u32]> {
        self.cast.then_some(&self.seen[..])
    }
}

/// A team's threat spots.
#[derive(Clone, Debug, Default)]
pub struct ThreatSpots {
    spots: Vec<Spot>,
    /// Changes when the set of regions kept out may have.
    pub version: u32,
}

/// A floor's region.
pub type RegionOf = std::sync::Arc<dyn Fn(mapdata::NodeId) -> u32 + Send + Sync>;

/// The regions (and so the floors of them) the team's held spots see, shared
/// with the bots of the team. A bit by region, not by floor: a set is made
/// again whenever a spot is cast, cleared or lets go (36 times a match in
/// the bench), and a bit by floor cost 8-11 ms of every bot-second.
#[derive(Clone, Default)]
pub struct Covered {
    /// Sorted.
    regions: Vec<u32>,
    /// By region, whether it is covered.
    bits: Vec<bool>,
    region_of: Option<RegionOf>,
}

impl std::fmt::Debug for Covered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Covered").field("regions", &self.regions.len()).finish()
    }
}

impl Covered {
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    pub fn region(&self, region: u32) -> bool {
        self.bits.get(region as usize).copied().unwrap_or(false)
    }

    pub fn floor(&self, node: mapdata::NodeId) -> bool {
        self.region_of.as_ref().is_some_and(|of| self.region(of(node)))
    }

    /// Whether the ground at `p` is covered, on `grid`.
    pub fn exposed_at(&self, grid: &mapdata::WalkGrid, p: Vec3) -> bool {
        grid.locate(p, 1.5).is_some_and(|n| self.floor(n))
    }

    /// The covered regions.
    pub fn regions(&self) -> &[u32] {
        &self.regions
    }

    /// The covered regions of `regions` out of `total`, the region of a floor
    /// given by `region_of`.
    pub fn build(mut regions: Vec<u32>, total: usize, region_of: RegionOf) -> Self {
        regions.sort_unstable();
        regions.dedup();
        let mut bits = vec![false; total];
        for &r in &regions {
            if let Some(bit) = bits.get_mut(r as usize) {
                *bit = true;
            }
        }
        Covered { regions, bits, region_of: Some(region_of) }
    }
}

impl ThreatSpots {
    /// A bot of the team died to `shooter`, whom the team sensed at
    /// `shooter_at` at `sensed`, the victim at `victim_at`.
    pub fn died(&mut self, victim_at: Vec3, shooter: Place, shooter_at: Vec3, sensed: u32, now: u32) {
        let bearing = (victim_at.x - shooter_at.x).atan2(victim_at.z - shooter_at.z);
        let reach = REACH_FLOOR_M.max(shooter_at.distance_xz(victim_at) + REACH_PLUS_M);
        let spot = self
            .spots
            .iter_mut()
            .find(|s| s.shooter == shooter && s.at.distance_xz(shooter_at) <= MERGE_M);
        match spot {
            Some(s) => {
                s.strength = s.strength_at(now) + 1.0;
                s.stamp = now;
                s.shooter_seen = Some(s.shooter_seen.map_or(sensed, |t| t.max(sensed)));
                s.empty_since = None;
                if s.bearings.len() < BEARINGS_KEPT {
                    s.bearings.push(bearing);
                }
                if reach > s.reach_m {
                    s.reach_m = reach;
                    // More of the map is in his line: cast again.
                    s.cast = false;
                    s.todo = None;
                }
            }
            None => self.spots.push(Spot {
                at: shooter_at,
                strength: 1.0,
                stamp: now,
                shooter,
                bearings: vec![bearing],
                reach_m: reach,
                shooter_seen: Some(sensed),
                empty_since: None,
                todo: None,
                seen: Vec::new(),
                cast: false,
            }),
        }
    }

    /// The shooter died: his spots keep half.
    pub fn shooter_died(&mut self, shooter: Place, now: u32) {
        for s in self.spots.iter_mut().filter(|s| s.shooter == shooter) {
            s.strength = s.strength_at(now) * SHOOTER_DIED_KEEPS;
            s.stamp = now;
            s.shooter_seen = None;
        }
    }

    /// The team sensed `shooter` at `at` at `time`.
    pub fn shooter_sensed(&mut self, shooter: Place, at: Vec3, time: u32) {
        for s in self.spots.iter_mut().filter(|s| s.shooter == shooter) {
            if s.at.distance_xz(at) <= SEEN_ON_M && s.shooter_seen.is_none_or(|t| time > t) {
                s.shooter_seen = Some(time);
            }
        }
    }

    /// The spots held at `now`.
    pub fn held(&self, now: u32) -> impl Iterator<Item = &Spot> {
        self.spots.iter().filter(move |s| s.held(now))
    }

    /// Any spot at all.
    pub fn is_empty(&self) -> bool {
        self.spots.is_empty()
    }

    /// A commander tick at `now`: spots too weak are dropped; a spot a
    /// teammate (`watchers`, eye points) has a line to within
    /// [`CLEAR_LINE_M`] for [`CLEAR_MS`] with no enemy (`enemies`, sensed
    /// points) within [`SEEN_ON_M`] of it is cleared; a held spot's regions
    /// are cast at, [`RAYS_PER_TICK`] a tick.
    pub fn tick(&mut self, now: u32, watchers: &[Vec3], enemies: &[Vec3], eyes: &dyn Eyes) {
        let before = self.spots.len();
        let mut changed = false;
        self.spots.retain_mut(|s| {
            if s.strength_at(now) < GONE_STRENGTH {
                return false;
            }
            let at = s.at + Vec3::new(0.0, eye_height(false, 0.0, 0.0), 0.0);
            let empty = !enemies.iter().any(|e| e.distance_xz(s.at) <= SEEN_ON_M) && {
                // The nearest few teammates within reach look: a sight ray each.
                let mut near: Vec<Vec3> = watchers
                    .iter()
                    .copied()
                    .filter(|w| w.distance_xz(s.at) <= CLEAR_LINE_M)
                    .collect();
                near.sort_by(|a, b| a.distance_xz(s.at).total_cmp(&b.distance_xz(s.at)));
                near.iter().take(WATCHERS_ASKED).any(|&w| eyes.sees(w, at))
            };
            if !empty {
                s.empty_since = None;
                return true;
            }
            let since = *s.empty_since.get_or_insert(now);
            now < since + CLEAR_MS
        });
        changed |= self.spots.len() != before;
        for s in self.spots.iter_mut().filter(|s| s.held(now) && !s.cast) {
            let todo = s.todo.get_or_insert_with(|| {
                let reach = s.reach_m;
                s.seen.clear();
                let (at, bearings) = (s.at, s.bearings.clone());
                eyes.region_points()
                    .into_iter()
                    .filter(|(_, p)| p.distance_xz(at) <= reach && in_lane(&bearings, at, *p))
                    .collect()
            });
            let eye = s.at + Vec3::new(0.0, eye_height(false, 0.0, 0.0), 0.0);
            let take = todo.len().min(RAYS_PER_TICK);
            for (region, p) in todo.drain(..take) {
                if eyes.sees(eye, p + Vec3::new(0.0, TARGET_HEIGHT_M, 0.0)) {
                    s.seen.push(region);
                }
            }
            if todo.is_empty() {
                s.cast = true;
                s.todo = None;
                changed = true;
            }
        }
        if changed {
            self.version = self.version.wrapping_add(1);
        }
    }

    /// The regions the held spots see, those cast at so far only when all
    /// of a spot are (a half-cast line keeps nothing out).
    pub fn covered_regions(&self, now: u32) -> Vec<u32> {
        self.held(now)
            .filter_map(Spot::covered)
            .flatten()
            .copied()
            .collect()
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regions 10 m apart on a line; the ones in `blind` are behind a wall.
    struct Line {
        blind: Vec<u32>,
    }

    impl Eyes for Line {
        fn sees(&self, _from: Vec3, to: Vec3) -> bool {
            !self.blind.contains(&((to.x / 10.0).round() as u32))
        }

        fn region_points(&self) -> Vec<(u32, Vec3)> {
            (0..20).map(|r| (r, Vec3::new(r as f32 * 10.0, 0.0, 0.0))).collect()
        }
    }

    fn at(x: f32) -> Vec3 {
        Vec3::new(x, 0.0, 0.0)
    }

    #[test]
    fn deaths_add_up_on_a_spot_two_hold_it_and_time_lets_it_go() {
        let mut spots = ThreatSpots::default();
        // One death, the shooter sensed on the spot 2 s before: held for the
        // 15 s from that sense.
        spots.died(at(30.0), 7, at(0.0), 1_000, 3_000);
        assert_eq!(spots.held(3_000).count(), 1);
        assert_eq!(spots.held(16_000).count(), 1);
        assert_eq!(spots.held(16_001).count(), 0, "one death, unsensed for 15 s");
        // Two deaths hold it without a sense.
        spots.died(at(35.0), 7, at(3.0), 8_000, 10_000);
        assert_eq!(spots.spots.len(), 1);
        assert_eq!(spots.held(60_000).count(), 1);
        // Two minutes on, the 2.0 is down to 1.5 less 1.0: not held.
        assert_eq!(spots.held(10_000 + 120_000).count(), 0);
        // Another shooter on the same ground is another spot.
        spots.died(at(35.0), 8, at(3.0), 11_000, 11_500);
        assert_eq!(spots.spots.len(), 2);
    }

    #[test]
    fn a_shooter_sensed_on_a_weak_spot_holds_it_and_one_off_it_does_not() {
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 1_000);
        assert_eq!(spots.held(20_000).count(), 0);
        spots.shooter_sensed(7, at(2.0), 20_000);
        assert_eq!(spots.held(21_000).count(), 1);
        // Sensed 20 m off the spot: not on it.
        spots.shooter_sensed(7, at(20.0), 40_000);
        assert_eq!(spots.held(41_000).count(), 0);
    }

    #[test]
    fn the_shooters_death_halves_the_spot() {
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 500);
        spots.died(at(30.0), 7, at(0.0), 500, 1_000);
        assert_eq!(spots.held(1_000).count(), 1);
        spots.shooter_died(7, 2_000);
        // 2.0 becomes 1.0, and he is not sensed on it any more.
        assert_eq!(spots.held(2_000).count(), 0);
    }

    #[test]
    fn a_held_spot_casts_at_its_regions_and_keeps_those_seen() {
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 500);
        spots.died(at(30.0), 7, at(0.0), 400, 600);
        let eyes = Line { blind: vec![3, 4] };
        spots.tick(1_000, &[], &[], &eyes);
        // Reach 45 m: regions 0 to 4; 3 and 4 are behind a wall.
        let mut seen = spots.covered_regions(1_000);
        seen.sort_unstable();
        assert_eq!(seen, vec![0, 1, 2]);
        assert!(spots.version > 0);
    }

    #[test]
    fn a_big_reach_is_cast_in_steps_and_keeps_nothing_until_all_is_cast() {
        let mut spots = ThreatSpots::default();
        // 190 m: all 20 regions are within reach and 64 rays a tick finish
        // them at once, so make the field larger through a long line.
        spots.died(at(30.0), 7, at(0.0), 0, 500);
        spots.died(at(30.0), 7, at(0.0), 0, 600);
        struct Wide;
        impl Eyes for Wide {
            fn sees(&self, _: Vec3, _: Vec3) -> bool {
                true
            }
            fn region_points(&self) -> Vec<(u32, Vec3)> {
                (0..150).map(|r| (r, Vec3::new(r as f32 * 0.2, 0.0, 0.0))).collect()
            }
        }
        spots.tick(1_000, &[], &[], &Wide);
        assert!(spots.covered_regions(1_000).is_empty(), "64 of 150 cast");
        spots.tick(1_100, &[], &[], &Wide);
        assert!(spots.covered_regions(1_100).is_empty(), "128 of 150 cast");
        spots.tick(1_200, &[], &[], &Wide);
        assert_eq!(spots.covered_regions(1_200).len(), 150);
    }

    #[test]
    fn a_spot_is_cleared_when_a_watcher_sees_it_empty_for_three_seconds() {
        let mut spots = ThreatSpots::default();
        spots.died(at(30.0), 7, at(0.0), 0, 0);
        let eyes = Line { blind: vec![] };
        let watcher = [at(20.0)];
        spots.tick(1_000, &watcher, &[], &eyes);
        spots.tick(3_900, &watcher, &[], &eyes);
        assert!(!spots.is_empty());
        spots.tick(4_100, &watcher, &[], &eyes);
        assert!(spots.is_empty());
        // An enemy sensed on the spot keeps it.
        spots.died(at(30.0), 7, at(0.0), 5_000, 5_000);
        spots.tick(6_000, &watcher, &[at(1.0)], &eyes);
        spots.tick(9_500, &watcher, &[at(1.0)], &eyes);
        assert!(!spots.is_empty());
        // Nobody near enough to look: kept.
        spots.tick(20_000, &[at(100.0)], &[], &eyes);
        assert!(!spots.is_empty());
    }

    #[test]
    fn a_spot_covers_the_lanes_of_its_victims_not_all_it_sees() {
        let mut spots = ThreatSpots::default();
        // Victims 30 m along +x, then 30 m along +z.
        spots.died(at(30.0), 7, at(0.0), 0, 500);
        spots.died(Vec3::new(0.0, 0.0, 30.0), 7, at(0.0), 0, 600);
        struct Ring;
        impl Eyes for Ring {
            fn sees(&self, _: Vec3, _: Vec3) -> bool {
                true
            }
            fn region_points(&self) -> Vec<(u32, Vec3)> {
                vec![
                    (0, Vec3::new(20.0, 0.0, 0.0)),
                    (1, Vec3::new(20.0, 0.0, 5.0)),
                    (2, Vec3::new(0.0, 0.0, 20.0)),
                    (3, Vec3::new(-20.0, 0.0, 0.0)),
                    (4, Vec3::new(14.0, 0.0, 14.0)),
                    (5, Vec3::new(20.0, 0.0, 9.0)),
                ]
            }
        }
        spots.tick(1_000, &[], &[], &Ring);
        let mut seen = spots.covered_regions(1_000);
        seen.sort_unstable();
        // On +x (0), 14 degrees off it (1), on +z (2); not behind (3), not
        // between the two lanes (4), not 24 degrees off (5).
        assert_eq!(seen, vec![0, 1, 2]);
    }

    #[test]
    fn exposure_is_the_metres_a_route_runs_through_seen_ground_and_where() {
        use crate::v026g::bots::path::Waypoint;
        let wp = |x: f32| Waypoint { at: at(x), node: 0, jump: false, low: false };
        let route = Route::new(vec![wp(0.0), wp(30.0), wp(60.0)]);
        let seen = |p: Vec3| (20.0..35.0).contains(&p.x);
        // Samples every 3 m: 21, 24, 27, 30, 33 are seen.
        let e = exposure(&route, seen, &[]);
        assert_eq!(e.metres, 15.0);
        assert_eq!(e.spans, vec![(21.0, 33.0)]);
        // An end in the middle of it: the ground within 10 m is free.
        assert_eq!(exposure(&route, seen, &[at(30.0)]).metres, 0.0);
        // Two lanes make two spans; the horizon stops the count.
        let two = |p: Vec3| (6.0..10.0).contains(&p.x) || (40.0..44.0).contains(&p.x);
        let e = exposure(&route, two, &[]);
        assert_eq!(e.spans, vec![(6.0, 9.0), (42.0, 42.0)]);
        let long = Route::new(vec![wp(0.0), wp(500.0)]);
        assert_eq!(exposure(&long, |p| p.x > 150.0, &[]).metres, 0.0, "beyond the horizon");
    }

    #[test]
    fn hits_finds_a_small_disc_the_sampling_would_step_over() {
        use crate::v026g::bots::path::Waypoint;
        let wp = |x: f32, z: f32| Waypoint { at: Vec3::new(x, 0.0, z), node: 0, jump: false, low: false };
        let route = Route::new(vec![wp(0.0, 0.0), wp(30.0, 0.0), wp(30.0, 30.0)]);
        // A 0.5 m disc on the first leg at x 10.1, z 0.2: the chord is 2 * sqrt(0.25 - 0.04).
        let e = hits(&route, &[(Vec3::new(10.1, 0.0, 0.2), 0.5)]);
        let chord = 2.0 * (0.25_f32 - 0.04).sqrt();
        assert_eq!(e.spans.len(), 1);
        assert!((e.spans[0].0 - (10.1 - chord / 2.0)).abs() < 1e-3 && (e.metres - chord).abs() < 1e-3, "{e:?}");
        // The sampling at 3 m (0, 3, 6, 9, 12) misses it.
        assert_eq!(exposure(&route, |p| p.distance_xz(Vec3::new(10.1, 0.0, 0.2)) < 0.5, &[]).metres, 0.0);
        // Off the line by more than the radius: none. On the second leg: after 30 m.
        assert!(hits(&route, &[(Vec3::new(10.0, 0.0, 0.6), 0.5)]).spans.is_empty());
        let e = hits(&route, &[(Vec3::new(30.2, 0.0, 12.0), 0.9)]);
        assert!((e.spans[0].0 - (42.0 - (0.81_f32 - 0.04).sqrt())).abs() < 1e-3, "{e:?}");
        // Two discs touching make one span.
        let e = hits(&route, &[(Vec3::new(5.0, 0.0, 0.0), 0.5), (Vec3::new(5.6, 0.0, 0.0), 0.5)]);
        assert_eq!(e.spans.len(), 1);
        assert!((e.metres - 1.6).abs() < 1e-3, "{e:?}");
    }

    #[test]
    fn covered_floors_follow_their_regions() {
        let covered = Covered::build(vec![2, 2, 5], 6, std::sync::Arc::new(|n| n / 2));
        assert!(covered.region(2) && covered.region(5) && !covered.region(3));

        assert!(covered.floor(4) && covered.floor(5) && !covered.floor(6) && !covered.floor(99));
        assert_eq!(covered.regions(), &[2, 5]);
    }
}

//! [lane line] A per-life line inside the lane (Fable task D, fix 9): each life of a bot walks its long routes on a line of its own
//! a few metres beside the planned one, as the live humans walk one lane a few metres apart.
//!
//! The measure (2026-11-27, `private/pvb-343-check/lane-line-20261127/`, `lane-line-humans-v3.py` on the live humans' long legs
//! and the v88 B arm's): in 558 lane groups (start and end 15 m cells), the bots' own line (the bot track nearest the group's
//! other bot tracks), and each human leg on that line's own 15 m lane (the lane test's cells equal: 1,323 legs) by its median
//! offset across it over the middle of the leg. The humans' offsets: |median| 1.19 m (p50), 3.24 (p75), 5.20 (p90); the bots'
//! own 0.10, 0.25, 0.90. Route overlap of a lane group's legs by tolerance:
//!
//! | within | 2 m | 4 m | 8 m | 15 m | 30 m |
//! |---|---|---|---|---|---|
//! | humans | 0.291 | 0.471 | 0.676 | 0.862 | 0.971 |
//! | bots (v88 B) | 0.486 | 0.609 | 0.752 | 0.888 | 0.978 |
//! | bots moved by the humans' offsets ([`HUMAN_LINE_M`], tapered as [`super::path::LINE_TAPER`]) | 0.308 | 0.521 | 0.740 | 0.885 | 0.977 |
//!
//! So the line closes most of the gap at 2 and 4 m and leaves 8 m and wider (lanes, not lines) as they are. A life keeps its line
//! (a route planned again every few seconds keeps its side; a draw per route would switch sides on a walk).

use super::path::{LineCount, Route};
use super::rng::Rng;
use mapdata::grid::controlled::PullWalk;
use mapdata::{NodeId, WalkGrid};

/// The live humans' offsets from the bots' line inside one lane, m, + to the left of the way walked: the percentiles 1-99 of
/// the 1,323 legs of the measure (the module's table). A life draws its line from them ([`draw`]).
pub const HUMAN_LINE_M: [f32; 99] = [
    -6.20, -5.42, -4.47, -3.92, -3.34, -2.83, -2.43, -2.21, -1.92, -1.76, -1.63, -1.48, -1.40, -1.27, -1.20, -1.11, -1.03, -0.93,
    -0.89, -0.84, -0.77, -0.68, -0.62, -0.55, -0.50, -0.47, -0.42, -0.38, -0.33, -0.30, -0.25, -0.20, -0.19, -0.16, -0.13, -0.10,
    -0.06, -0.04, -0.00, 0.02, 0.04, 0.07, 0.11, 0.13, 0.16, 0.18, 0.22, 0.25, 0.30, 0.33, 0.37, 0.40, 0.46, 0.50, 0.54, 0.58, 0.62,
    0.67, 0.72, 0.79, 0.88, 0.96, 1.02, 1.11, 1.17, 1.25, 1.33, 1.42, 1.52, 1.64, 1.79, 1.94, 2.10, 2.23, 2.48, 2.60, 2.75, 2.92,
    3.11, 3.22, 3.40, 3.55, 3.77, 3.88, 4.10, 4.21, 4.36, 4.49, 4.56, 4.74, 4.89, 5.18, 5.35, 5.55, 5.71, 5.95, 6.08, 6.32, 6.77,
];

/// The routes a line is walked on: the measure's legs, over 60 m walked (the lane test's leg split).
pub const LINE_ROUTE_M: f32 = 60.0;

/// A life's line, m: the humans' offset at a uniform quantile between their 1st and 99th percentiles, between table rows
/// linearly.
pub fn draw(rng: &mut Rng) -> f32 {
    let x = rng.unit() * (HUMAN_LINE_M.len() - 1) as f32;
    let i = (x as usize).min(HUMAN_LINE_M.len() - 2);
    HUMAN_LINE_M[i] + (HUMAN_LINE_M[i + 1] - HUMAN_LINE_M[i]) * (x - i as f32)
}

/// A life's own random numbers for its line: apart from the bot's own ([`super::Bot`]'s), so drawing the line changes no other
/// draw of the match.
pub fn rng_for(match_id: u32, place: usize) -> Rng {
    Rng::new(Rng::for_bot(match_id, place).next_u64() ^ 0x6C61_6E65_6C69_6E65)
}

/// `route` on the line `line_m` when it is long enough ([`LINE_ROUTE_M`]), counted.
pub fn walked(route: Route, line_m: f32, grid: &WalkGrid, verdict: &mut dyn FnMut(NodeId, NodeId) -> PullWalk) -> Route {
    if route.length() < LINE_ROUTE_M {
        return route;
    }
    let (route, count) = route.lined(line_m, grid, verdict);
    counts::add(count, line_m);
    route
}

/// The lines walked since the last call ([`take`]): for the server's log.
pub mod counts {
    use super::LineCount;
    use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

    static ROUTES: AtomicU64 = AtomicU64::new(0);
    static LINED: AtomicU64 = AtomicU64::new(0);
    static CORNERS: AtomicU64 = AtomicU64::new(0);
    static MOVED: AtomicU64 = AtomicU64::new(0);
    static PUT_IN: AtomicU64 = AtomicU64::new(0);
    static NO_FLOOR: AtomicU64 = AtomicU64::new(0);
    static NO_LINE: AtomicU64 = AtomicU64::new(0);
    static NO_WALK: AtomicU64 = AtomicU64::new(0);
    /// The lines' |offset| summed over the routes, cm.
    static OFFSET_CM: AtomicU64 = AtomicU64::new(0);

    pub(super) fn add(c: LineCount, line_m: f32) {
        ROUTES.fetch_add(1, Relaxed);
        if c.moved + c.put_in > 0 {
            LINED.fetch_add(1, Relaxed);
        }
        CORNERS.fetch_add(u64::from(c.corners), Relaxed);
        MOVED.fetch_add(u64::from(c.moved), Relaxed);
        PUT_IN.fetch_add(u64::from(c.put_in), Relaxed);
        NO_FLOOR.fetch_add(u64::from(c.refused_floor), Relaxed);
        NO_LINE.fetch_add(u64::from(c.refused_line), Relaxed);
        NO_WALK.fetch_add(u64::from(c.refused_walk), Relaxed);
        OFFSET_CM.fetch_add((line_m.abs() * 100.0) as u64, Relaxed);
    }

    /// The counts, set back to 0: long routes, routes on a line (a point moved or put in), corners movable, moved, put in, tries
    /// refused for no floor, no clear line, no walk; and the lines' mean |offset|, m.
    pub fn take() -> ([u64; 8], f32) {
        let c = [&ROUTES, &LINED, &CORNERS, &MOVED, &PUT_IN, &NO_FLOOR, &NO_LINE, &NO_WALK].map(|c| c.swap(0, Relaxed));
        let cm = OFFSET_CM.swap(0, Relaxed);
        (c, if c[0] == 0 { 0.0 } else { cm as f32 / 100.0 / c[0] as f32 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The draw keeps to the humans' table, both sides, and two lives of one bot draw apart.
    #[test]
    fn a_life_draws_its_line_from_the_humans() {
        let mut rng = rng_for(7, 3);
        let lines: Vec<f32> = (0..4000).map(|_| draw(&mut rng)).collect();
        assert!(lines.iter().all(|&l| (HUMAN_LINE_M[0]..=HUMAN_LINE_M[98]).contains(&l)));
        let mut sorted = lines.clone();
        sorted.sort_by(f32::total_cmp);
        let median = sorted[sorted.len() / 2];
        assert!((median - HUMAN_LINE_M[49]).abs() < 0.15, "the draw's median {median} against the humans' {}", HUMAN_LINE_M[49]);
        assert!(lines.iter().any(|&l| l < -2.0) && lines.iter().any(|&l| l > 2.0));
        assert!(HUMAN_LINE_M.windows(2).all(|w| w[0] <= w[1]), "a table of percentiles");
    }
}

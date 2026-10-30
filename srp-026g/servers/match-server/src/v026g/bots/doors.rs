//! Door centres (A66): where a leg of a route crosses a doorway, the middle of its opening.
//!
//! The planner pulls a route straight over the floors, so at a doorway reached after a turn the leg cuts the jamb's corner
//! over the floors nearest the post: the grid keeps a floor to within `exact_clearance` (0.3175 m) of a wall, and the client's
//! body holds 0.41 m ([`HOLD_M`]), so a body on those floors presses on the jamb (J0, Chemical Plant's 717 door; the walk
//! `tests/door_walk.rs`: 0.32-0.40 m from the jamb at 7 m/s). The middle of the opening is the one place the whole body fits.
//! A corner at the middle of a doorway is the funnel algorithm's portal inset by the agent's radius, found from the floors:
//! no wall triangles are needed, only that the floors across the doorway stop at both sides.

use mapdata::Vec3;
use mapdata::grid::WalkGrid;

/// The distance the client's controller keeps from a wall, metres (the A66 grid note; the body's radius is 0.38).
pub const HOLD_M: f32 = 0.41;
/// The floors across a doorway span at most this much, metres: the opening less the body's width, plus a cell. The 717 door
/// (1.27 m clear) has floors over 1.0 m.
pub const DOOR_SPAN_M: f32 = 1.15;
/// The walls on both sides of a doorway are looked for this far, metres, at this step.
const REACH_M: f32 = 1.0;
const STEP_M: f32 = 0.0625;
/// A doorway is a narrowing: the floors across the leg are this much wider this far before and after it, metres.
const WIDER_BY_M: f32 = 0.4;
const BEYOND_M: f32 = 0.5;
/// The leg is sampled at this step, metres.
const SAMPLE_M: f32 = 0.125;
/// A leg already crossing within this of the middle of the opening is left as it is, metres.
const OFF_CENTRE_M: f32 = 0.08;
/// The height a floor may differ from the leg's at a sample, metres (a ramp's floor is not on the chord).
const HEIGHT_M: f32 = 0.8;
/// The directions the span across is measured in: this many over a half turn.
const DIRECTIONS: usize = 8;

/// Whether there is a floor at `at` (at the height of the leg).
fn floor_at(grid: &WalkGrid, at: Vec3) -> bool {
    grid.floors_in([at.x, at.z, at.x, at.z]).any(|f| (grid.position(f).y - at.y).abs() <= HEIGHT_M)
}

/// How far the floors go from `at` along `(dx, dz)` before they stop, metres; none if they go on past [`REACH_M`].
fn extent(grid: &WalkGrid, at: Vec3, dx: f32, dz: f32) -> Option<f32> {
    let mut d = STEP_M;
    while d <= REACH_M {
        if !floor_at(grid, Vec3::new(at.x + dx * d, at.y, at.z + dz * d)) {
            return Some(d - STEP_M);
        }
        d += STEP_M;
    }
    None
}

/// The narrowest span of floors through `at` across all directions where both sides stop within reach: its width and the
/// offset of the span's middle from `at` (x, z), if there is one.
fn narrowest_span(grid: &WalkGrid, at: Vec3) -> Option<(f32, [f32; 2])> {
    let mut best: Option<(f32, [f32; 2])> = None;
    for k in 0..DIRECTIONS {
        let angle = std::f32::consts::PI * k as f32 / DIRECTIONS as f32;
        let (dx, dz) = (angle.cos(), angle.sin());
        let (Some(forward), Some(back)) = (extent(grid, at, dx, dz), extent(grid, at, -dx, -dz)) else {
            continue;
        };
        let width = forward + back;
        if best.is_none_or(|(w, _)| width < w) {
            let middle = 0.5 * (forward - back);
            best = Some((width, [dx * middle, dz * middle]));
        }
    }
    best
}

/// The middle of the doorway the leg from `a` to `b` crosses, if it crosses one off its middle: a narrowing of the floors
/// across the leg to at most [`DOOR_SPAN_M`], wider by [`WIDER_BY_M`] [`BEYOND_M`] before and after it. The point is on the
/// floors (at the height of the leg there).
pub fn centre_of(grid: &WalkGrid, a: Vec3, b: Vec3) -> Option<Vec3> {
    let length = a.distance_xz(b);
    if length < 2.0 * BEYOND_M {
        return None;
    }
    let (ux, uz) = ((b.x - a.x) / length, (b.z - a.z) / length);
    let at = |t: f32| {
        let s = (t / length).clamp(0.0, 1.0);
        Vec3::new(a.x + ux * t, a.y + (b.y - a.y) * s, a.z + uz * t)
    };
    let steps = (length / SAMPLE_M).floor() as usize;
    // The narrowest span along the leg and the stretch of samples as narrow as it (within 0.1 m).
    let spans: Vec<Option<(f32, [f32; 2])>> = (0..=steps).map(|k| narrowest_span(grid, at(k as f32 * SAMPLE_M))).collect();
    let narrowest = spans.iter().flatten().map(|s| s.0).fold(f32::MAX, f32::min);
    if narrowest > DOOR_SPAN_M {
        return None;
    }
    let as_narrow = |k: usize| spans[k].is_some_and(|s| s.0 <= narrowest + 0.1);
    let first = (0..=steps).find(|&k| as_narrow(k))?;
    let mut last = first;
    while last < steps && as_narrow(last + 1) {
        last += 1;
    }
    // Wider before and after: no span there (open floor) or a wider one.
    let wider = |t: f32| match narrowest_span(grid, at(t)) {
        None => true,
        Some((w, _)) => w >= narrowest + WIDER_BY_M,
    };
    let (t0, t1) = (first as f32 * SAMPLE_M, last as f32 * SAMPLE_M);
    if !(wider(t0 - BEYOND_M) && wider(t1 + BEYOND_M)) {
        return None;
    }
    let mid = (first + last) / 2;
    let (_, offset) = spans[mid]?;
    if (offset[0] * offset[0] + offset[1] * offset[1]).sqrt() < OFF_CENTRE_M {
        return None;
    }
    let p = at(mid as f32 * SAMPLE_M);
    Some(Vec3::new(p.x + offset[0], p.y, p.z + offset[1]))
}

/// A debugging line for a leg (A66): the span at every sample along it, narrowest first.
pub fn explain(grid: &WalkGrid, a: Vec3, b: Vec3) -> String {
    let length = a.distance_xz(b);
    let (ux, uz) = ((b.x - a.x) / length.max(1e-6), (b.z - a.z) / length.max(1e-6));
    let mut out = String::new();
    let mut t = 0.0;
    while t <= length {
        let s = (t / length.max(1e-6)).clamp(0.0, 1.0);
        let at = Vec3::new(a.x + ux * t, a.y + (b.y - a.y) * s, a.z + uz * t);
        match narrowest_span(grid, at) {
            Some((w, o)) => out.push_str(&format!(" {t:.2}:{w:.2}({:.2},{:.2})", o[0], o[1])),
            None => out.push_str(&format!(" {t:.2}:-")),
        }
        t += 0.25;
    }
    out
}

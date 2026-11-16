//! [S3] The line floors on held thin tops: a floor on a rail, a railing's top, a sill or a narrow ledge carries the top's centre
//! line through its cell ([`LineTop`], [`WalkGrid::line_top`]), for the motor to walk on the line (S5) rather than on the cell.
//!
//! Which tops: the thin tops the gap links over one already find ([`WalkGrid::over_thin_top`], [S4]: a top at most
//! [`super::gap_flown::THIN_TOP_M`] along a way no Walk link takes, with a landing past it). A thin top no gap search crosses
//! gets no line: this covers fewer tops than the railings census (`railings-design.md` §3), whose own thresholds are not taken.
//!
//! Each such floor is measured on the walker faces it stands on ([`WalkGrid::line_tops_of`]):
//! - from the floor's own face (found on the gap's way through the cell's centre), across the top in [`ACROSS_WAYS`] ways, in
//!   [`MEASURE_STEP_M`] steps (both a measurement's resolution, not a rule of the game): a step is on the top while a ground face
//!   (a `walk` face, or a `by slope` face no steeper than [`super::Body::min_ground_normal_y`], either side: the client's mesh is
//!   two-sided) is under it no more than the steepest ground's rise over the step from the step before (the top followed as
//!   ground); the narrowest way is the way across;
//! - its width: the steps on it that way times the step; a top wider than `THIN_TOP_M` is none;
//! - its centre line: through the middle of the steps on it, square to the way across, at the top's height there, a piece of a cell's
//!   length; only the floor whose cell holds the line's point carries it;
//! - a line, not a post: the top runs on along its line (measured the same way) further than it is wide;
//! - held: the client's walk tick stands a body set on the centre line and on each edge of the top
//!   ([`super::rest_floors::settle`]: standing, 5 mm over the point, no push; it comes down and rests within
//!   [`super::wall_give_back::STAND_MOVED_M`] of the put across and of the top in height). A top held only at its centre (a
//!   post, a round rail) gets no line.
//!
//! The floors of one top are one segment ([`LineTop::segment`]): those joined by Walk links.

use super::rest_floors::{settle, Settle};
use super::wall_give_back::STAND_MOVED_M;
use super::*;
use crate::controller::tick::Settings;
use crate::controller::Collision;
use crate::sight::RayTree;

/// The step the top is measured in across, m: a measurement's resolution (1 cm), not a rule of the game.
pub const MEASURE_STEP_M: f32 = 0.01;
/// The ways the top is measured across, over a half turn (11.25 deg apart): a measurement's resolution, not a rule of the game.
/// The narrowest is the way across; the line is square to it.
pub const ACROSS_WAYS: usize = 16;

/// A cell of a thin top a gap over one crosses ([`WalkGrid::over_thin_top`]): the cell, the height of the floor on the top
/// there and the gap's way ([`DIRS`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ThinTopCell {
    pub cell: usize,
    pub height: f32,
    pub way: usize,
}

/// What the top is made of: how the client takes it as ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopFace {
    /// A `by slope` face: ground by its slope.
    BySlope,
    /// A `walk` face: ground at any slope.
    Walk,
}

/// [S3] A held thin top's centre line through a floor's cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineTop {
    /// The top's segment: the floors of one top (joined by Walk links) share it.
    pub segment: u32,
    /// The straight piece of the centre line through the floor's cell, at the top's height: its ends in order along the top (the
    /// same order for every floor of a segment).
    pub from: Vec3,
    pub to: Vec3,
    /// The top's width, m.
    pub width: f32,
    pub face: TopFace,
}

impl LineTop {
    const BYTES: usize = 4 + 4 + 12 + 12 + 4 + 1;

    pub(super) fn bytes(node: NodeId, t: &LineTop) -> [u8; Self::BYTES] {
        let mut b = [0u8; Self::BYTES];
        b[..4].copy_from_slice(&node.to_le_bytes());
        b[4..8].copy_from_slice(&t.segment.to_le_bytes());
        for (i, v) in [t.from.x, t.from.y, t.from.z, t.to.x, t.to.y, t.to.z, t.width].into_iter().enumerate() {
            b[8 + 4 * i..12 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
        b[36] = match t.face {
            TopFace::BySlope => 0,
            TopFace::Walk => 1,
        };
        b
    }

    /// A row as [`Self::bytes`] wrote it; a face byte that is neither kind makes a width of NaN, which [`WalkGrid::check`]
    /// refuses.
    pub(super) fn from_bytes(b: [u8; Self::BYTES]) -> (NodeId, LineTop) {
        let word = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
        let f = |i: usize| f32::from_bits(word(i));
        let (face, bad) = match b[36] {
            0 => (TopFace::BySlope, false),
            1 => (TopFace::Walk, false),
            _ => (TopFace::BySlope, true),
        };
        let top = LineTop {
            segment: word(4),
            from: Vec3::new(f(8), f(12), f(16)),
            to: Vec3::new(f(20), f(24), f(28)),
            width: if bad { f32::NAN } else { f(32) },
            face,
        };
        (word(0), top)
    }
}

impl WalkGrid {
    /// [S3] The centre line of the held thin top the floor stands on, if it is one (module documentation).
    pub fn line_top(&self, floor: NodeId) -> Option<&LineTop> {
        self.line_tops.binary_search_by_key(&floor, |&(f, _)| f).ok().map(|i| &self.line_tops[i].1)
    }

    /// Whether the line tops are as [`Self::line_tops_of`] makes them: sorted by floor, one a floor, each with a positive finite
    /// width, distinct ends, its piece's middle in the floor's cell; a segment's floors consecutive by segment number from 0.
    pub(super) fn line_tops_ok(&self) -> bool {
        let n = self.len();
        let mut segments = 0u32;
        self.line_tops.windows(2).all(|w| w[0].0 < w[1].0)
            && self.line_tops.iter().all(|(f, t)| {
                if *f as usize >= n || !(t.width.is_finite() && t.width > 0.0) || t.from == t.to || t.segment > segments {
                    return false;
                }
                segments = segments.max(t.segment + 1);
                let mid = (t.from + t.to) * 0.5;
                let (i, k) = self.cell_xz(self.cell_of[*f as usize] as usize);
                let (cx, cz) = (self.x0 + i as f32 * self.cell, self.z0 + k as f32 * self.cell);
                [mid.x, mid.y, mid.z, t.from.y, t.to.y].iter().all(|v| v.is_finite())
                    && mid.x >= cx
                    && mid.x <= cx + self.cell
                    && mid.z >= cz
                    && mid.z <= cz + self.cell
            })
    }

    /// [S3] The line tops of the final grid's floors on the thin tops `cells` (module documentation), sorted by floor.
    pub(super) fn line_tops_of(&self, cells: &[ThinTopCell], triangles: &[Triangle], world: &Collision, s: &Settings) -> Vec<(NodeId, LineTop)> {
        let mut tops = self.measure_line_tops(cells, triangles, world, s);
        self.number_segments(&mut tops);
        tops
    }

    /// The floors on the thin tops `cells` measured (module documentation), sorted by floor, every segment 0.
    pub(super) fn measure_line_tops(&self, cells: &[ThinTopCell], triangles: &[Triangle], world: &Collision, s: &Settings) -> Vec<(NodeId, LineTop)> {
        let min_up = self.body.min_ground_normal_y;
        let ground_slope = |t: &Triangle| {
            let [a, b, c] = t.v;
            let n = (b - a).cross(c - a);
            n.length() > 1e-9 && n.y.abs() / n.length() >= min_up
        };
        let walk = RayTree::from_corners(triangles.iter().filter(|t| t.walker == WalkerBehaviour::Walk).map(|t| t.v));
        let slope = RayTree::from_corners(triangles.iter().filter(|t| t.walker == WalkerBehaviour::BySlope && ground_slope(t)).map(|t| t.v));
        // The steepest ground's rise over one measuring step (the client's ground limit).
        let rise = MEASURE_STEP_M * (1.0 - min_up * min_up).max(0.0).sqrt() / min_up.max(1e-3);
        // The ground under `p` from `top` down to `low`: its height and kind.
        let under = |p: Vec3, top: f32, low: f32| -> Option<(f32, TopFace)> {
            let o = Vec3::new(p.x, top, p.z);
            let down = Vec3::new(0.0, -1.0, 0.0);
            let reach = top - low;
            let w = walk.ray_hit(o, down, reach).map(|h| (top - h.t, TopFace::Walk));
            let b = slope.ray_hit(o, down, reach).map(|h| (top - h.t, TopFace::BySlope));
            match (w, b) {
                (Some(w), Some(b)) => Some(if w.0 >= b.0 { w } else { b }),
                (w, b) => w.or(b),
            }
        };
        // The floors on the tops, each once (its first cell's way).
        let mut floors: Vec<(NodeId, usize)> = Vec::new();
        for c in cells {
            if let Some(f) = (self.start[c.cell]..self.start[c.cell + 1]).find(|&f| self.height[f as usize] == c.height) {
                floors.push((f, c.way));
            }
        }
        floors.sort_unstable_by_key(|&(f, _)| f);
        floors.dedup_by_key(|&mut (f, _)| f);
        let half = self.cell / 2.0;
        let measured = super::controlled::parallel(&floors, |&(f, way)| {
            let (di, dk) = DIRS[way];
            let l = ((di * di + dk * dk) as f32).sqrt();
            let along = Vec3::new(di as f32 / l, 0.0, dk as f32 / l);
            let (i, k) = self.cell_xz(self.cell_of[f as usize] as usize);
            let centre = Vec3::new(self.x0 + (i as f32 + 0.5) * self.cell, self.height[f as usize], self.z0 + (k as f32 + 0.5) * self.cell);
            // The floor's own face: on the way through the cell's centre, within the cell, the ground at most the steepest ground's
            // rise over the half diagonal under the floor's height (the floor is its faces' highest point in the cell).
            let slack = rise / MEASURE_STEP_M * half * std::f32::consts::SQRT_2;
            let steps = (half / MEASURE_STEP_M).round() as i32;
            let (start, start_y, face) = (-steps..=steps).find_map(|j| {
                let p = centre + along * (j as f32 * MEASURE_STEP_M);
                under(p, centre.y + MEASURE_STEP_M, centre.y - slack).map(|(y, face)| (j, y, face))
            })?;
            let start = centre + along * (start as f32 * MEASURE_STEP_M);
            // From `from` (at height `y`) along `dir` while on the top: the steps taken (at most `limit`) and the last one's
            // height.
            let run = |from: Vec3, y: f32, dir: Vec3, limit: i32| {
                let (mut j, mut y) = (0, y);
                while j < limit {
                    let Some((h, _)) = under(from + dir * ((j + 1) as f32 * MEASURE_STEP_M), y + rise, y - rise) else { break };
                    j += 1;
                    y = h;
                }
                (j, y)
            };
            let max = (super::gap_flown::THIN_TOP_M / MEASURE_STEP_M).round() as i32;
            // The middle of the top on the gap's way: a chord's middle is on the top's centre line, whatever way it crosses.
            let (back, back_y) = run(start, start_y, along * -1.0, 2 * max);
            let (fore, fore_y) = run(start, start_y, along, 2 * max);
            let start = start + along * ((fore - back) as f32 / 2.0 * MEASURE_STEP_M);
            let (start_y, _) = under(start, start_y.max(back_y).max(fore_y) + rise, start_y.min(back_y).min(fore_y) - rise)?;
            // Across the top: the narrowest of [`ACROSS_WAYS`] ways through that middle (both ways out from it).
            let (across, (lo, lo_y), (hi, hi_y)) = (0..ACROSS_WAYS)
                .map(|w| {
                    let t = w as f32 * std::f32::consts::PI / ACROSS_WAYS as f32;
                    let dir = Vec3::new(t.cos(), 0.0, t.sin());
                    (dir, run(start, start_y, dir * -1.0, max), run(start, start_y, dir, max))
                })
                .min_by_key(|&(_, (lo, _), (hi, _))| lo + hi)?;
            let count = lo + hi + 1;
            if count > max {
                return None;
            }
            let width = count as f32 * MEASURE_STEP_M;
            let at = |j: f32| start + across * (j * MEASURE_STEP_M);
            let middle = {
                let p = at((hi - lo) as f32 / 2.0);
                let (y, _) = under(p, start_y.max(lo_y).max(hi_y) + rise, start_y.min(lo_y).min(hi_y) - rise)?;
                Vec3::new(p.x, y, p.z)
            };
            // The line square to the narrowest way across, in one order for every floor (x up, else z up).
            let mut line = Vec3::new(-across.z, 0.0, across.x);
            if line.x < 0.0 || (line.x == 0.0 && line.z < 0.0) {
                line = line * -1.0;
            }
            // The line's point is in this floor's cell (else it is the neighbour cell's floor's line).
            let (cx, cz) = (centre.x - half, centre.z - half);
            if !(middle.x >= cx && middle.x <= cx + self.cell && middle.z >= cz && middle.z <= cz + self.cell) {
                return None;
            }
            // A line, not a post: the top runs on along its line, both ways from its point, further than it is wide.
            let (back, _) = run(middle, middle.y, line * -1.0, count);
            let (on, _) = run(middle, middle.y, line, count);
            if back + on < count {
                return None;
            }
            // Held at the centre line and at each edge.
            let stands = |p: Vec3| match settle(world, s, p) {
                Settle::Rests { put, at } => (at.x - put.x).hypot(at.z - put.z) < STAND_MOVED_M && (at.y - p.y).abs() < STAND_MOVED_M,
                Settle::Jammed | Settle::Moving => false,
            };
            let edge = |j: i32, y: f32| {
                let p = at(j as f32);
                Vec3::new(p.x, y, p.z)
            };
            if !(stands(middle) && stands(edge(-lo, lo_y)) && stands(edge(hi, hi_y))) {
                return None;
            }
            // The piece of the line through the cell.
            Some((middle - line * half, middle + line * half, width, face))
        });
        floors
            .iter()
            .zip(measured)
            .filter_map(|(&(f, _), m)| m.map(|(from, to, width, face)| (f, LineTop { segment: 0, from, to, width, face })))
            .collect()
    }

    /// The segments: the line floors `tops` (sorted by floor) joined by Walk links, numbered by their first floor.
    fn number_segments(&self, tops: &mut [(NodeId, LineTop)]) {
        let index = |f: NodeId| tops.binary_search_by_key(&f, |&(g, _)| g).ok();
        let mut segment: Vec<Option<u32>> = vec![None; tops.len()];
        let mut next = 0;
        for start in 0..tops.len() {
            if segment[start].is_some() {
                continue;
            }
            let mut stack = vec![start];
            segment[start] = Some(next);
            while let Some(i) = stack.pop() {
                for (g, kind) in self.links(tops[i].0) {
                    if kind == LinkKind::Walk
                        && let Some(j) = index(g)
                        && segment[j].is_none()
                    {
                        segment[j] = Some(next);
                        stack.push(j);
                    }
                }
            }
            next += 1;
        }
        for (t, s) in tops.iter_mut().zip(segment) {
            t.1.segment = s.expect("every line floor is numbered");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: [f32; 4] = [0.0, 0.0, 10.0, 10.0];

    fn quad(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> [Triangle; 2] {
        [Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope }, Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope }]
    }

    /// Flat ground and a box along z from 2 to 8, x from `x0` to `x1`, 1 m high.
    fn ground_and_top(x0: f32, x1: f32) -> Vec<Triangle> {
        ground_and_box(x0, x1, 2.0, 8.0)
    }

    /// Flat ground and a box over x `x0`..`x1`, z `z0`..`z1`, 1 m high.
    fn ground_and_box(x0: f32, x1: f32, z0: f32, z1: f32) -> Vec<Triangle> {
        let p = Vec3::new;
        let mut t = Vec::new();
        t.extend(quad(p(0.0, 0.0, 0.0), p(0.0, 0.0, 10.0), p(10.0, 0.0, 10.0), p(10.0, 0.0, 0.0)));
        let h = 1.0;
        t.extend(quad(p(x0, h, z0), p(x0, h, z1), p(x1, h, z1), p(x1, h, z0)));
        t.extend(quad(p(x0, 0.0, z0), p(x0, h, z0), p(x0, h, z1), p(x0, 0.0, z1)));
        t.extend(quad(p(x1, 0.0, z0), p(x1, 0.0, z1), p(x1, h, z1), p(x1, h, z0)));
        t.extend(quad(p(x0, 0.0, z0), p(x1, 0.0, z0), p(x1, h, z0), p(x0, h, z0)));
        t.extend(quad(p(x0, 0.0, z1), p(x0, h, z1), p(x1, h, z1), p(x1, 0.0, z1)));
        t
    }

    /// The top's floors in the cells over x `x` at z 5.125 and 5.375 (where there is one), as thin top cells crossed along +x.
    fn top_cells(g: &WalkGrid, x: f32) -> Vec<ThinTopCell> {
        [5.125, 5.375]
            .iter()
            .filter_map(|&z| {
                let cell = g.cell_index(((x - g.x0) / g.cell).floor() as i32, ((z - g.z0) / g.cell).floor() as i32).expect("in the area");
                let f = (g.start[cell]..g.start[cell + 1]).find(|&f| (g.height[f as usize] - 1.0).abs() < 1e-3)?;
                Some(ThinTopCell { cell, height: g.height[f as usize], way: 0 })
            })
            .collect()
    }

    /// A rail 0.10 m wide (x 4.95..5.05): its floors get its centre line (x 5.0, along z, at its top), its width to the measuring
    /// step, a body is held on the line and on both edges; the two floors are one segment.
    #[test]
    fn a_rail_s_floors_carry_its_centre_line_and_width() {
        let tris = ground_and_top(4.95, 5.05);
        let g = WalkGrid::build(&tris, AREA, 0.25, Body::default());
        let cells = top_cells(&g, 4.975);
        let tops = g.line_tops_of(&cells, &tris, &Collision::build(&tris), &Settings::LIVE);
        assert_eq!(tops.len(), 2, "{tops:?}");
        for (_, t) in &tops {
            assert!((t.width - 0.10).abs() <= MEASURE_STEP_M + 1e-4, "width {}", t.width);
            let mid = (t.from + t.to) * 0.5;
            assert!((mid.x - 5.0).abs() <= MEASURE_STEP_M, "centre {mid:?}");
            assert!((t.from.x - t.to.x).abs() < 1e-4 && t.from.z < t.to.z, "along z, z up: {t:?}");
            assert!((t.from.y - 1.0).abs() < 1e-3 && (t.to.y - 1.0).abs() < 1e-3, "at the top: {t:?}");
            assert_eq!(t.face, TopFace::BySlope);
        }
        assert_eq!(tops[0].1.segment, tops[1].1.segment, "one top, one segment");
    }

    /// A rail 0.10 m wide at 45 deg (its centre line x = z, from (3, 3) to (7, 7)), crossed along +x (aslant): its width is
    /// measured across it, not along the way (0.14 m), and its line runs along it.
    #[test]
    fn an_aslant_rail_is_measured_across_it() {
        let p = Vec3::new;
        let (h, w) = (1.0, 0.05 * std::f32::consts::SQRT_2);
        // The rail's corners: the centre line offset by half the width each side ((+w/2, -w/2) and back, in x and z).
        let (a0, a1) = (p(3.0 + w / 2.0, h, 3.0 - w / 2.0), p(7.0 + w / 2.0, h, 7.0 - w / 2.0));
        let (b0, b1) = (p(3.0 - w / 2.0, h, 3.0 + w / 2.0), p(7.0 - w / 2.0, h, 7.0 + w / 2.0));
        let low = |v: Vec3| p(v.x, 0.0, v.z);
        let mut tris = Vec::new();
        tris.extend(quad(p(0.0, 0.0, 0.0), p(0.0, 0.0, 10.0), p(10.0, 0.0, 10.0), p(10.0, 0.0, 0.0)));
        tris.extend(quad(a0, b0, b1, a1));
        tris.extend(quad(low(a0), a0, a1, low(a1)));
        tris.extend(quad(low(b0), low(b1), b1, b0));
        let g = WalkGrid::build(&tris, AREA, 0.25, Body::default());
        let cell = g.cell_index(19, 19).expect("in the area");
        let f = (g.start[cell]..g.start[cell + 1]).find(|&f| (g.height[f as usize] - h).abs() < 1e-3).expect("a floor on the rail");
        let cells = [ThinTopCell { cell, height: g.height[f as usize], way: 0 }];
        let tops = g.line_tops_of(&cells, &tris, &Collision::build(&tris), &Settings::LIVE);
        assert_eq!(tops.len(), 1, "{tops:?}");
        let t = tops[0].1;
        assert!((t.width - 0.10).abs() <= 2.0 * MEASURE_STEP_M, "width {}", t.width);
        let d = t.to - t.from;
        assert!((d.x - d.z).abs() < 0.02 && d.x > 0.0, "along the rail: {t:?}");
        let mid = (t.from + t.to) * 0.5;
        assert!((mid.x - mid.z).abs() < 2.0 * MEASURE_STEP_M, "on the centre line: {mid:?}");
    }

    /// A top 0.60 m wide (x 4.7..5.3) is wider than a thin top: no line.
    #[test]
    fn a_top_wider_than_a_thin_top_gets_no_line() {
        let tris = ground_and_top(4.7, 5.3);
        let g = WalkGrid::build(&tris, AREA, 0.25, Body::default());
        let cells = top_cells(&g, 4.875);
        assert!(g.line_tops_of(&cells, &tris, &Collision::build(&tris), &Settings::LIVE).is_empty());
    }

    /// A grid with line tops writes and reads back with them (`line_top` answers the same; the digest holds them).
    #[test]
    fn a_grid_s_line_tops_read_back() {
        let tris = ground_and_top(4.95, 5.05);
        let mut g = WalkGrid::build(&tris, AREA, 0.25, Body::default());
        let before = g.digest();
        let cells = top_cells(&g, 4.975);
        g.line_tops = g.line_tops_of(&cells, &tris, &Collision::build(&tris), &Settings::LIVE);
        assert!(!g.line_tops.is_empty());
        assert_ne!(g.digest(), before, "in the digest");
        let dir = std::env::temp_dir().join(format!("srp-grid-line-tops-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(crate::built::grid_file_name(11));
        g.write(&path, 11).unwrap();
        let read = WalkGrid::read(&path, 11, Body::default()).unwrap();
        assert_eq!(read.digest(), g.digest());
        for &(f, t) in &g.line_tops {
            assert_eq!(read.line_top(f), Some(&t));
        }
        assert_eq!(read.line_top(0), g.line_top(0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A post 0.10 m by 0.10 m (x 4.95..5.05, z 5.05..5.15): as long as it is wide, no line.
    #[test]
    fn a_post_gets_no_line() {
        let tris = ground_and_box(4.95, 5.05, 5.05, 5.15);
        let g = WalkGrid::build(&tris, AREA, 0.25, Body::default());
        let cells = top_cells(&g, 4.975);
        assert_eq!(cells.len(), 1, "the post's floor");
        assert!(g.line_tops_of(&cells, &tris, &Collision::build(&tris), &Settings::LIVE).is_empty());
    }

    /// The file row reads back as written; a face byte of no kind is refused (a NaN width).
    #[test]
    fn a_line_top_row_reads_back() {
        let t = LineTop { segment: 3, from: Vec3::new(1.0, 2.0, 3.0), to: Vec3::new(1.0, 2.0, 3.25), width: 0.11, face: TopFace::Walk };
        assert_eq!(LineTop::from_bytes(LineTop::bytes(7, &t)), (7, t));
        let mut b = LineTop::bytes(7, &t);
        b[36] = 2;
        assert!(LineTop::from_bytes(b).1.width.is_nan());
    }
}

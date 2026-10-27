//! Off-mesh gap links (step 4 of the module documentation, after erosion): a jump across
//! a gap to a floor two or more cells on, which no neighbour link makes.
//!
//! Root cause they remove: [`WalkGrid::link`] joins a floor only to the
//! floors of its 8 adjacent cells, and erosion's retargeting only moves
//! links that exist, so a floor across an empty gap gets no link of any
//! kind. Players take such jumps: in the live matches with 12 or more
//! humans, 9,800 samples stood on floors no spawn reached over the grid's
//! links, half of them across a short gap (`reach-record.md`, 2026-10-26).
//!
//! How (Unity's off-mesh link generator walks the mesh's edges and tests
//! a jump's trajectory and its landing; Detour keeps such links apart
//! from the mesh as two-ended connections): from every floor with room to
//! stand, along each of the 8 directions with no walk or jump link and no
//! floor within a step in the next cell, the body is flown along the
//! client's short jump clip ([`SHORT_JUMP_AHEAD`], [`SHORT_JUMP_RISE`]),
//! pressed at each of a range of points behind the take-off's centre, as
//! the bench's stand-in flies it (`air_move`): the body goes to the clip's
//! root unless a face stands across its front edge (a solid between
//! [`AIR_FEET_M`] and [`AIR_HEAD_M`] over the feet), and then stays where
//! it is while the clip lifts it, going on once over the face (so a jump
//! from against a riser is made, as players make it); it lands on the
//! highest floor it comes down to once past the apex, or after the clip
//! falls straight down. A link is made for each
//! window of presses that all land on one level, at least
//! [`GAP_PRESS_SPREAD_M`] either side of its middle (where the motor
//! presses it: [`OffLink::press`]), when the landing has room on along the
//! direction ([`WalkGrid::lands`]) and the run-up behind the take-off is
//! floor. Then, as Recast and Unity sample an edge: one link per metre of
//! edge, direction and level, and only those that save [`GAP_SAVING_M`]
//! or more against the grid's own way (or reach floors it does not).
//!
//! The sprint jump's gaps are not made: no bot sprints into a jump yet
//! (the motor lets go of sprint lining up); they come with it.

use super::*;

/// The client's short jump (`on_site_fwd`, codes 5-12): how far ahead of
/// the press its root is, every 2 of its 24 frames, m. PROVEN (the stock
/// clip's root curve, the jump study; the bench stand-in's `SHORT_CLIP`
/// holds the same numbers, a test there checks it).
pub const SHORT_JUMP_AHEAD: [f32; 13] = [
    0.0, 0.22, 0.44, 0.67, 0.89, 1.11, 1.33, 1.56, 1.78, 2.00, 2.22, 2.44, 2.67,
];
/// … and how high over the floor it left, m (the same frames).
pub const SHORT_JUMP_RISE: [f32; 13] = [
    0.0, 0.0, 0.0, 0.0, 0.36, 0.64, 0.84, 0.96, 1.00, 0.94, 0.77, 0.46, 0.0,
];
/// The root's highest point, m ahead of the press (frame 16).
const APEX_AHEAD_M: f32 = 1.78;

/// The presses looked at, m before the take-off floor's centre: from
/// [`GAP_PRESS_MIN_M`] (the body leaves the floor 0.67 m on, at the clip's
/// jump event: still on the take-off) to [`GAP_PRESS_MAX_M`], every
/// [`GAP_PRESS_STEP_M`]. The clip's root starts at the press (PROVEN, the
/// jump study: root motion), so where the body comes down follows from
/// where it pressed; the motor presses at the link's own
/// ([`OffLink::press`]).
pub const GAP_PRESS_MIN_M: f32 = 0.6;
pub const GAP_PRESS_MAX_M: f32 = 1.6;
pub const GAP_PRESS_STEP_M: f32 = 0.1;
/// A gap link holds for a press this much earlier or later than its own,
/// m. GUESS: about the bots' reckoning error and a tick's walk at a
/// lined-up approach; the client test (J0 `gap` tries) sets it.
pub const GAP_PRESS_SPREAD_M: f32 = 0.2;
/// Floor walked to behind the press, m: the motor's run-up for a gap link
/// starts there (`motor.rs`).
pub const GAP_RUN_UP_M: f32 = 0.4;

/// Where a gap link is pressed: this many centimetres before its take-off
/// floor's centre, along the way to its landing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PressLead(u16);

impl PressLead {
    pub fn from_metres(m: f32) -> Self {
        Self((m * 100.0).round().clamp(0.0, f32::from(u16::MAX)) as u16)
    }

    pub fn metres(self) -> f32 {
        f32::from(self.0) / 100.0
    }

    pub(super) fn centimetres(self) -> u16 {
        self.0
    }

    pub(super) fn from_centimetres(cm: u16) -> Self {
        Self(cm)
    }

    /// Within the presses a build looks at.
    pub(super) fn is_valid(self) -> bool {
        let m = self.metres();
        (GAP_PRESS_MIN_M - 1e-3..=GAP_PRESS_MAX_M + 1e-3).contains(&m)
    }
}

/// How a gap link leaves its take-off.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Takeoff {
    /// The short jump, pressed this far before the take-off's centre.
    Press(PressLead),
    /// Walked off the edge, no key: the body keeps its ground speed over
    /// the edge and falls ([`WALK_OFF_SPEEDS`]).
    WalkOff,
}

impl Takeoff {
    /// In a built file: a press's centimetres, a walk-off 0 (no press is
    /// under [`GAP_PRESS_MIN_M`]).
    pub(super) fn centimetres(self) -> u16 {
        match self {
            Takeoff::Press(lead) => lead.centimetres(),
            Takeoff::WalkOff => 0,
        }
    }

    pub(super) fn from_centimetres(cm: u16) -> Self {
        match cm {
            0 => Takeoff::WalkOff,
            cm => Takeoff::Press(PressLead::from_centimetres(cm)),
        }
    }

    pub(super) fn is_valid(self) -> bool {
        match self {
            Takeoff::Press(lead) => lead.is_valid(),
            Takeoff::WalkOff => true,
        }
    }
}

/// A body walked off an edge keeps its ground speed over it while it
/// falls (PROVEN, J0 g2a, 2026-10-26: 3.5-3.6 m/s through a 1.2 m fall,
/// 7 of 7 onto the bridge where a jump at the edge overshot 7 of 7). A
/// walk-off link is kept only where every one of these speeds, m/s, comes
/// down on one level: a walk (3.0-3.6) and a sprint (5.1, J0), so the
/// bot walks it at whatever pace it has, no key.
pub const WALK_OFF_SPEEDS: [f32; 3] = [3.0, 3.6, 5.1];
/// Floor walked behind a walk-off's take-off, m: the walk's speed is
/// reached in 0.25-0.5 s. GUESS.
pub const WALK_OFF_RUN_UP_M: f32 = 1.0;
/// `fall_params.gravity`, m/s² (PROVEN, `docs/notes/fall_damage.md`).
const GRAVITY: f32 = 9.8;
/// The heights over the feet at which a face holds a body in the air: its
/// feet (a little over) and its head, m (the stand-in's rays, 0.1 and
/// 1.7).
pub const AIR_FEET_M: f32 = 0.1;
pub const AIR_HEAD_M: f32 = 1.7;
/// A gap link is kept only when the grid's own way from its take-off to
/// its landing costs this much more than the link, or there is none, m.
/// GUESS: players jump where it saves a walk round, not to cut a corner
/// (benched against the reach census).
pub const GAP_SAVING_M: f32 = 10.0;
/// A gap link's cost over its length and the kind's penalty, per metre
/// across, m: a miss falls into the gap. GUESS.
pub const GAP_PENALTY_PER_M: f32 = 1.0;
/// At most this many gap links land on one floor: the way back a search
/// keeps holds the link's place among them in 5 bits.
pub const GAP_LINKS_IN_MAX: usize = 32;

/// What the gap pass of a build looked at and why it kept or refused
/// each try (one a floor and a direction with no walk or jump link), for
/// the build's counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GapStats {
    /// Directions tried (no walk or jump link that way, room to stand).
    pub tried: usize,
    /// A floor within a step in the next cell: no gap.
    pub no_gap: usize,
    /// No floor walked to behind for the run-up.
    pub no_run_up: usize,
    /// Held by a face, or nothing under the body within the drop.
    pub no_landing: usize,
    /// Landing in the next cell (the grid's own link).
    pub adjacent: usize,
    /// A press earlier or later lands elsewhere.
    pub narrow_window: usize,
    /// No room to stand or land on.
    pub no_room: usize,
    /// Higher than the body jumps, or within the clip's marginal top.
    pub too_high: usize,
    /// Made; then one a metre of edge kept …
    pub made: usize,
    pub sampled: usize,
    /// … and of those, kept for the walk round they save.
    pub kept: usize,
}

/// Which off-mesh links a build makes
/// ([`WalkGrid::build_kept_with_links`]). Without them a grid is the one
/// built before they were made: the same key, digest and file
/// ([`crate::grid_key_with_links`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OffMeshLinks {
    #[default]
    None,
    /// The gap links of this module.
    Gaps,
}

/// A link that is no neighbour link (an off-mesh connection): from a
/// floor to one 2 or more cells away, of an off-mesh kind
/// ([`LinkKind::is_off_mesh`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OffLink {
    pub from: NodeId,
    pub to: NodeId,
    pub kind: LinkKind,
    /// How it leaves its take-off: pressed, or walked off.
    pub press: Takeoff,
}

/// The short jump's root height `ahead` metres from the press (linear
/// between the clip's samples); `None` past the clip's end.
pub fn short_jump_rise(ahead: f32) -> Option<f32> {
    let end = SHORT_JUMP_AHEAD[SHORT_JUMP_AHEAD.len() - 1];
    if !(0.0..=end).contains(&ahead) {
        return None;
    }
    let i = SHORT_JUMP_AHEAD
        .partition_point(|&a| a <= ahead)
        .clamp(1, SHORT_JUMP_AHEAD.len() - 1);
    let (a0, a1) = (SHORT_JUMP_AHEAD[i - 1], SHORT_JUMP_AHEAD[i]);
    let t = ((ahead - a0) / (a1 - a0)).clamp(0.0, 1.0);
    Some(SHORT_JUMP_RISE[i - 1] + (SHORT_JUMP_RISE[i] - SHORT_JUMP_RISE[i - 1]) * t)
}

/// The cost of an off-mesh link over `across` metres on the ground that
/// falls `descent` metres: its length, the kind's penalty, the gap's
/// ([`GAP_PENALTY_PER_M`]) and, down, the drop's per metre.
pub(super) fn gap_cost(kind: LinkKind, length: f32, across: f32, descent: f32) -> f32 {
    let down = if kind == LinkKind::GapDrop {
        DROP_DESCENT_PENALTY * descent.max(0.0)
    } else {
        0.0
    };
    length + kind.penalty() + GAP_PENALTY_PER_M * across + down
}

impl WalkGrid {
    /// The cell holding the ground point (`x`, `z`), if in the grid.
    fn cell_at(&self, x: f32, z: f32) -> Option<usize> {
        let i = ((x - self.x0) / self.cell).floor() as i32;
        let k = ((z - self.z0) / self.cell).floor() as i32;
        self.cell_index(i, k)
    }

    /// Whether solid lies within `r` of the ground point (`x`, `z`) (the
    /// cells overlapping that square) between heights `low` and `high`.
    fn solid_within(&self, solids: &Solids, x: f32, z: f32, r: f32, low: f32, high: f32) -> bool {
        let lo_i = ((x - r - self.x0) / self.cell).floor() as i32;
        let hi_i = ((x + r - self.x0) / self.cell).floor() as i32;
        let lo_k = ((z - r - self.z0) / self.cell).floor() as i32;
        let hi_k = ((z + r - self.z0) / self.cell).floor() as i32;
        (lo_k..=hi_k).any(|k| {
            (lo_i..=hi_i).any(|i| {
                self.cell_index(i, k).is_some_and(|c| {
                    solids.spans[solids.start[c] as usize..solids.start[c + 1] as usize]
                        .iter()
                        .any(|&(lo, hi)| lo < high && hi > low)
                })
            })
        })
    }

    /// Whether a face stands across the body's front edge at `front`
    /// (its centre; `u` the way it moves), between [`AIR_FEET_M`] and
    /// [`AIR_HEAD_M`] over its feet at `y`: the stand-in's rays ahead
    /// (`air_move`), taken across the body's width so a gap narrower than
    /// the body holds it.
    fn face_ahead(&self, solids: &Solids, front: (f32, f32), u: (f32, f32), y: f32) -> bool {
        let r = self.body.radius;
        let across = (-u.1, u.0);
        let steps = (2.0 * r / (0.5 * self.cell)).ceil() as i32;
        (0..=steps).any(|i| {
            let o = -r + 2.0 * r * i as f32 / steps as f32;
            self.solid_within(solids, front.0 + across.0 * o, front.1 + across.1 * o, 0.0, y + AIR_FEET_M, y + AIR_HEAD_M)
        })
    }

    /// Where a short jump from floor `a` along direction `d`, pressed
    /// `lead` metres before `a`'s centre, comes down: the floor and how far
    /// from `a`'s centre (module documentation). As the bench's stand-in
    /// flies it (`air_move`): the body goes to the clip's root each sample
    /// unless a face across its width is in the way, and then stays where
    /// it is while the clip lifts it, to go on once it is over the face.
    /// `None` when nothing is under it within the harmless drop.
    fn fly(&self, a: usize, d: usize, lead: f32, solids: &Solids) -> Option<(u32, f32)> {
        let from = self.position(a as NodeId);
        let h = self.height[a];
        let (di, dk) = DIRS[d];
        let len = ((di * di + dk * dk) as f32).sqrt();
        let (ux, uz) = (di as f32 / len, dk as f32 / len);
        let ds = 0.5 * self.cell;
        let end = SHORT_JUMP_AHEAD[SHORT_JUMP_AHEAD.len() - 1] - lead;
        let floors = |cell: usize| self.start[cell]..self.start[cell + 1];
        let at = |s: f32| (from.x + ux * s, from.z + uz * s);
        // The highest floor in the cell at `s` between `y` and `top`.
        let landing = |s: f32, y: f32, top: f32| {
            let (px, pz) = at(s);
            let cell = self.cell_at(px, pz)?;
            floors(cell)
                .filter(|&f| {
                    let fy = self.height[f as usize];
                    fy <= top + 0.05 && fy >= y - 1e-4
                })
                .max_by(|&p, &q| self.height[p as usize].total_cmp(&self.height[q as usize]))
        };
        let mut last = h;
        // Where the body is along the way (held behind the root by faces).
        let mut body = 0.0f32;
        let mut x = ds;
        loop {
            if x > end {
                // After the clip: straight down where the body is.
                let f = landing(body, h - self.body.max_drop, last)?;
                return Some((f, body));
            }
            let ahead = x + lead;
            let y = h + short_jump_rise(ahead).unwrap_or(0.0);
            // Coming down onto a floor at the root: landed (before the
            // faces: its own top is under the feet).
            if ahead >= APEX_AHEAD_M
                && let Some(f) = landing(x, y, last)
            {
                return Some((f, x));
            }
            // To the root, unless a face anywhere across the body's width
            // is in the way.
            let mut s = body + ds;
            let held = loop {
                if s > x + 1e-4 {
                    break false;
                }
                if self.face_ahead(solids, at(s + self.body.radius), (ux, uz), y) {
                    break true;
                }
                s += ds;
            };
            if !held {
                body = x;
            } else if ahead >= APEX_AHEAD_M
                && let Some(f) = landing(body, y, last)
            {
                // Held, coming down where it is.
                return Some((f, body));
            }
            last = y;
            x += ds;
        }
    }

    /// Where a body walking off floor `a`'s edge along `d` comes down, when
    /// it is the same level at every speed of [`WALK_OFF_SPEEDS`]: from the
    /// cell's edge it keeps its speed and falls, held by faces as [`Self::fly`]
    /// holds a jump. `None` past the harmless drop or with no common level.
    fn walk_off_landing(&self, a: usize, d: usize, solids: &Solids) -> Option<u32> {
        let mut level: Option<(u32, f32)> = None;
        for speed in WALK_OFF_SPEEDS {
            let to = self.walk_off(a, d, speed, solids)?;
            let y = self.height[to as usize];
            match level {
                None => level = Some((to, y)),
                Some((_, l)) if (y - l).abs() <= SAME_TOP => {}
                Some(_) => return None,
            }
            if speed == WALK_OFF_SPEEDS[1] {
                level = Some((to, y));
            }
        }
        level.map(|(to, _)| to)
    }

    /// One walk-off at `speed`: the body leaves `a`'s cell at its edge on
    /// `a`'s height and falls; the first floor it comes down to.
    fn walk_off(&self, a: usize, d: usize, speed: f32, solids: &Solids) -> Option<u32> {
        let from = self.position(a as NodeId);
        let h = self.height[a];
        let (di, dk) = DIRS[d];
        let len = ((di * di + dk * dk) as f32).sqrt();
        let (ux, uz) = (di as f32 / len, dk as f32 / len);
        let ds = 0.5 * self.cell;
        let floors = |cell: usize| self.start[cell]..self.start[cell + 1];
        let at = |s: f32| (from.x + ux * s, from.z + uz * s);
        // It walks on `a`'s height while a surface of that height is under
        // its centre (past the floors erosion took from the edge), and
        // falls from where it ends.
        let surface_under = |s: f32| {
            let (px, pz) = at(s);
            self.cell_at(px, pz).is_some_and(|c| {
                solids.spans[solids.start[c] as usize..solids.start[c + 1] as usize]
                    .iter()
                    .any(|&(_, hi)| (hi - h).abs() <= 0.06)
            })
        };
        let mut edge = 0.0f32;
        while edge < 2.0 && surface_under(edge + ds) {
            edge += ds;
        }
        let mut last = h;
        let mut x = edge + ds;
        loop {
            let t = (x - edge) / speed;
            let y = h - 0.5 * GRAVITY * t * t;
            if y < h - self.body.max_drop {
                return None;
            }
            // Coming down onto a floor at `x` (before the faces: its own
            // top is under the feet).
            let (px, pz) = at(x);
            let cell = self.cell_at(px, pz)?;
            if let Some(f) = floors(cell)
                .filter(|&f| {
                    let fy = self.height[f as usize];
                    fy <= last + 0.05 && fy >= y - 1e-4
                })
                .max_by(|&p, &q| self.height[p as usize].total_cmp(&self.height[q as usize]))
            {
                return Some(f);
            }
            // Held by a face ahead: it falls where it is.
            if self.face_ahead(solids, at(x + self.body.radius), (ux, uz), y) {
                let (px, pz) = at(x - ds);
                let cell = self.cell_at(px, pz)?;
                return floors(cell)
                    .filter(|&f| {
                        let fy = self.height[f as usize];
                        fy <= y + 0.05 && fy >= h - self.body.max_drop
                    })
                    .max_by(|&p, &q| self.height[p as usize].total_cmp(&self.height[q as usize]));
            }
            last = y;
            x += ds;
        }
    }

    /// The gap links from floor `a` along `d` (module documentation): for
    /// each window of presses [`GAP_PRESS_STEP_M`] apart from
    /// [`GAP_PRESS_MIN_M`] to [`GAP_PRESS_MAX_M`] before `a`'s centre that
    /// all come down on one level two or more cells on, at least
    /// [`GAP_PRESS_SPREAD_M`] either side of its middle, a link pressed at
    /// that middle.
    fn gap_link(&self, wide: &Wide, a: usize, d: usize, solids: &Solids, stats: &mut GapStats, out: &mut Vec<OffLink>) {
        stats.tried += 1;
        let h = self.height[a];
        let (i, k) = self.cell_xz(self.cell_of[a] as usize);
        let (di, dk) = DIRS[d];
        // A floor within a step in the next cell: no gap (a walk cut on a
        // slope, or a floor eroded at a wall).
        let Some(next) = self.cell_index(i + di, k + dk) else {
            return;
        };
        if (self.start[next]..self.start[next + 1])
            .any(|b| (self.height[b as usize] - h).abs() <= self.body.step)
        {
            stats.no_gap += 1;
            return;
        }
        // The floor walked to behind, for the run-up (each press needs its
        // distance and [`GAP_RUN_UP_M`] more).
        let back = d ^ 1;
        let step = self.cell * ((di * di + dk * dk) as f32).sqrt();
        let mut behind = 0.0f32;
        let mut f = a;
        while behind < GAP_PRESS_MAX_M + GAP_RUN_UP_M {
            let b = wide.links[f][back];
            if b == NONE || wide.kinds[f][back] != LinkKind::Walk {
                break;
            }
            behind += step;
            f = b as usize;
        }
        if behind + 1e-3 < GAP_PRESS_MIN_M + GAP_RUN_UP_M {
            stats.no_run_up += 1;
            return;
        }
        // A drop walked off the edge first (J0: where a walk-off lands, a
        // jump overshoots); the presses after it, as for any gap.
        if let Some(to) = (behind + 1e-3 >= WALK_OFF_RUN_UP_M).then(|| self.walk_off_landing(a, d, solids)).flatten() {
            let (ti, tk) = self.cell_xz(self.cell_of[to as usize] as usize);
            let to_y = self.height[to as usize];
            if (ti - i).abs().max((tk - k).abs()) >= 2
                && to_y - h <= -self.body.step
                && self.ceiling[to as usize] - to_y >= self.body.stand_height
                && Self::lands(wide, to as usize, d)
            {
                out.push(OffLink { from: a as NodeId, to, kind: LinkKind::GapDrop, press: Takeoff::WalkOff });
                stats.made += 1;
            }
        }
        let presses = ((GAP_PRESS_MAX_M - GAP_PRESS_MIN_M) / GAP_PRESS_STEP_M).round() as usize + 1;
        // Each press's landing, two or more cells on.
        let landing: Vec<Option<u32>> = (0..presses)
            .map(|p| {
                let lead = GAP_PRESS_MIN_M + p as f32 * GAP_PRESS_STEP_M;
                if lead + GAP_RUN_UP_M > behind + 1e-3 {
                    return None;
                }
                let (to, _) = self.fly(a, d, lead, solids)?;
                let (ti, tk) = self.cell_xz(self.cell_of[to as usize] as usize);
                ((ti - i).abs().max((tk - k).abs()) >= 2).then_some(to)
            })
            .collect();
        if landing.iter().all(Option::is_none) {
            stats.no_landing += 1;
            return;
        }
        let half = (GAP_PRESS_SPREAD_M / GAP_PRESS_STEP_M).round() as usize;
        let mut p = 0;
        while p < presses {
            let Some(first) = landing[p] else {
                p += 1;
                continue;
            };
            let level = self.height[first as usize];
            let mut q = p;
            while q + 1 < presses
                && landing[q + 1].is_some_and(|t| (self.height[t as usize] - level).abs() <= SAME_TOP)
            {
                q += 1;
            }
            // A window wide enough: a press at its middle.
            if q - p >= 2 * half {
                let mid = (p + q) / 2;
                let to = landing[mid].expect("in the window");
                let to_y = self.height[to as usize];
                // No higher than the body jumps, and kept off the clip's
                // marginal top as the grid's jump links are priced there.
                let rise_ok = to_y - h <= self.body.max_jump.min(MARGINAL_JUMP_M);
                if !rise_ok {
                    stats.too_high += 1;
                } else if self.ceiling[to as usize] - to_y >= self.body.stand_height && Self::lands(wide, to as usize, d) {
                    out.push(OffLink {
                        from: a as NodeId,
                        to,
                        kind: if to_y - h > -self.body.step {
                            LinkKind::GapJump
                        } else {
                            LinkKind::GapDrop
                        },
                        press: Takeoff::Press(PressLead::from_metres(GAP_PRESS_MIN_M + mid as f32 * GAP_PRESS_STEP_M)),
                    });
                    stats.made += 1;
                } else {
                    stats.no_room += 1;
                }
            } else {
                stats.narrow_window += 1;
            }
            p = q + 1;
        }
    }

    /// The gap links of the eroded grid, kept and thinned
    /// (module documentation), sorted by take-off then landing.
    pub(super) fn gap_links(&self, wide: &Wide, solids: &Solids, stats: &mut GapStats) -> Vec<OffLink> {
        let n = self.len();
        // A body that does not jump takes none.
        if self.body.max_jump <= 0.0 {
            return Vec::new();
        }
        let mut found: Vec<(OffLink, usize)> = Vec::new();
        let mut scratch: Vec<OffLink> = Vec::new();
        for a in 0..n {
            if self.ceiling[a] - self.height[a] < self.body.stand_height {
                continue;
            }
            for d in 0..8 {
                let b = wide.links[a][d];
                if b != NONE && wide.kinds[a][d] != LinkKind::Drop {
                    continue;
                }
                let before = scratch.len();
                self.gap_link(wide, a, d, solids, stats, &mut scratch);
                found.extend(scratch.drain(before..).map(|link| (link, d)));
            }
        }
        // One a metre of edge, direction and level (the first, in floor
        // order); a drop one a metre of edge and level whatever its way,
        // walked off before pressed (J0: where a walk-off lands, a jump
        // overshoots).
        found.sort_by_key(|&(link, _)| link.press != Takeoff::WalkOff);
        let mut seen = std::collections::HashSet::new();
        found.retain(|&(link, d)| {
            let (p, q) = (self.position(link.from), self.position(link.to));
            seen.insert((
                p.x.floor() as i32,
                p.z.floor() as i32,
                (2.0 * p.y).round() as i32,
                if link.kind == LinkKind::GapDrop { usize::MAX } else { d },
                (2.0 * q.y).round() as i32,
            ))
        });
        stats.sampled = found.len();
        // Only those that save a walk round.
        let mut cost = vec![f32::INFINITY; n];
        let mut touched: Vec<u32> = Vec::new();
        let mut open = Open::default();
        let mut kept: Vec<(OffLink, f32)> = Vec::new();
        for &(link, _) in &found {
            let own = self.off_cost(link);
            if !self.walks_within(wide, link.from, link.to, own + GAP_SAVING_M, &mut cost, &mut touched, &mut open) {
                kept.push((link, own));
            }
        }
        // At most [`GAP_LINKS_IN_MAX`] onto a floor, the cheapest.
        kept.sort_by(|a, b| a.0.to.cmp(&b.0.to).then(a.1.total_cmp(&b.1)));
        let mut out: Vec<OffLink> = Vec::with_capacity(kept.len());
        let mut run = 0usize;
        for (i, &(link, _)) in kept.iter().enumerate() {
            run = if i > 0 && kept[i - 1].0.to == link.to { run + 1 } else { 0 };
            if run < GAP_LINKS_IN_MAX {
                out.push(link);
            }
        }
        out.sort_by_key(|l| (l.from, l.to));
        stats.kept = out.len();
        out
    }

    /// An off-mesh link's cost while the grid is built (no lip or edge
    /// marks yet): [`gap_cost`].
    fn off_cost(&self, link: OffLink) -> f32 {
        let (p, q) = (self.position(link.from), self.position(link.to));
        gap_cost(link.kind, (q - p).length(), p.distance_xz(q), p.y - q.y)
    }

    /// Whether the neighbour links lead from `from` to `to` for at most
    /// `limit` (length, kind penalty and a drop's descent), searched no
    /// further. `cost`, `touched` and `open` are scratch, left clean.
    #[allow(clippy::too_many_arguments)]
    fn walks_within(
        &self,
        wide: &Wide,
        from: NodeId,
        to: NodeId,
        limit: f32,
        cost: &mut [f32],
        touched: &mut Vec<u32>,
        open: &mut Open,
    ) -> bool {
        open.clear();
        cost[from as usize] = 0.0;
        touched.push(from);
        open.push(Entry(0.0, from));
        let mut found = false;
        while let Some(Entry(c, u)) = open.pop() {
            if c > cost[u as usize] {
                continue;
            }
            if u == to {
                found = true;
                break;
            }
            let pu = self.position(u);
            for d in 0..8 {
                let v = wide.links[u as usize][d];
                if v == NONE {
                    continue;
                }
                let kind = wide.kinds[u as usize][d];
                let pv = self.position(v);
                let descent = if kind == LinkKind::Drop {
                    DROP_DESCENT_PENALTY * (pu.y - pv.y).max(0.0)
                } else {
                    0.0
                };
                let nc = c + (pv - pu).length() + kind.penalty() + descent;
                if nc <= limit && nc < cost[v as usize] {
                    if cost[v as usize].is_infinite() {
                        touched.push(v);
                    }
                    cost[v as usize] = nc;
                    open.push(Entry(nc, v));
                }
            }
        }
        for &t in touched.iter() {
            cost[t as usize] = f32::INFINITY;
        }
        touched.clear();
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(x0: f32, z0: f32, x1: f32, z1: f32, y: f32) -> [Triangle; 2] {
        let (a, b, c, d) = (
            Vec3::new(x0, y, z0),
            Vec3::new(x1, y, z0),
            Vec3::new(x1, y, z1),
            Vec3::new(x0, y, z1),
        );
        let walker = WalkerBehaviour::BySlope;
        [Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
    }

    /// A vertical face across x at `x`, from z0 to z1, from y0 to y1.
    fn face_x(x: f32, z0: f32, z1: f32, y0: f32, y1: f32) -> [Triangle; 2] {
        let (a, b, c, d) = (
            Vec3::new(x, y0, z0),
            Vec3::new(x, y0, z1),
            Vec3::new(x, y1, z1),
            Vec3::new(x, y1, z0),
        );
        let walker = WalkerBehaviour::BySlope;
        [Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
    }

    /// A block from x0 to x1 (z 0 to 10), its top at `y`, standing on
    /// `ground`.
    fn block(x0: f32, x1: f32, y: f32, ground: f32) -> Vec<Triangle> {
        let mut t = quad(x0, 0.0, x1, 10.0, y).to_vec();
        t.extend(face_x(x0, 0.0, 10.0, ground, y));
        t.extend(face_x(x1, 0.0, 10.0, ground, y));
        t
    }

    const AREA: [f32; 4] = [0.0, 0.0, 14.0, 10.0];

    /// Two blocks with tops `a` and `b` high, the gap between them from
    /// x 6 to `6 + gap`, over ground at `ground`; and whatever `extra` adds.
    fn world(a: f32, b: f32, gap: f32, ground: f32, extra: &[Triangle]) -> WalkGrid {
        let mut t = world_triangles(a, b, gap, ground);
        t.extend_from_slice(extra);
        WalkGrid::build_with_links(&t, AREA, 0.25, Body::default(), OffMeshLinks::Gaps)
    }

    fn world_triangles(a: f32, b: f32, gap: f32, ground: f32) -> Vec<Triangle> {
        let mut t = quad(0.0, 0.0, 14.0, 10.0, ground).to_vec();
        t.extend(block(0.0, 6.0, a, ground));
        t.extend(block(6.0 + gap, 14.0, b, ground));
        t
    }

    /// The kinds of the links a way from the near block to the far one
    /// takes, if there is a way.
    fn way_across(grid: &WalkGrid, a: f32, b: f32) -> Option<Vec<LinkKind>> {
        let from = grid.locate(Vec3::new(3.0, a, 5.0), 0.5).expect("the near block's floor");
        let to = grid.locate(Vec3::new(11.0, b, 5.0), 0.5).expect("the far block's floor");
        let path = grid.path(from, to)?;
        Some(
            path.nodes
                .windows(2)
                .map(|w| grid.links(w[0]).find(|&(n, _)| n == w[1]).expect("a link").1)
                .collect(),
        )
    }

    #[test]
    fn the_clips_rise_is_read_between_its_samples() {
        assert_eq!(short_jump_rise(1.78), Some(1.0));
        assert_eq!(short_jump_rise(0.5), Some(0.0));
        let r = short_jump_rise(1.89).unwrap();
        assert!((r - 0.97).abs() < 1e-3, "{r}");
        assert_eq!(short_jump_rise(2.7), None);
        assert_eq!(short_jump_rise(-0.1), None);
    }

    /// A level gap of 1 m between blocks 2 m high: the grid's links get a
    /// body down into it and no further; a gap jump gets it across, and a
    /// way takes it.
    #[test]
    fn a_short_gap_between_two_blocks_is_jumped() {
        let grid = world(2.0, 2.0, 1.0, 0.0, &[]);
        assert!(grid.off_links().iter().all(|l| l.kind == LinkKind::GapJump));
        let kinds = way_across(&grid, 2.0, 2.0).expect("a way across");
        assert_eq!(kinds.iter().filter(|k| k.is_off_mesh()).count(), 1, "{kinds:?}");
        assert!(kinds.contains(&LinkKind::GapJump));
        // One link a metre of edge: the blocks are 10 m long.
        let across = grid.off_links().iter().filter(|l| grid.position(l.from).x < 6.0).count();
        assert!((5..=40).contains(&across), "{across}");
    }

    /// Wider than the clip carries: no way across.
    #[test]
    fn a_gap_wider_than_the_jump_is_not_jumped() {
        let grid = world(2.0, 2.0, 2.0, 0.0, &[]);
        assert!(way_across(&grid, 2.0, 2.0).is_none());
        assert!(grid.off_links().iter().all(|l| grid.position(l.to).x < 8.0));
    }

    /// A post over the gap, higher than the jump's head: held, no link.
    #[test]
    fn a_face_in_the_way_holds_the_jump() {
        let post = face_x(6.5, 0.0, 10.0, 0.0, 6.0);
        let grid = world(2.0, 2.0, 1.0, 0.0, &post);
        assert!(way_across(&grid, 2.0, 2.0).is_none());
    }

    /// A wall over the gap with a door narrower than the body (0.5 m):
    /// no way through it; 1.5 m wide (the body and a cell of the solids'
    /// rounding each side): jumped through.
    #[test]
    fn a_door_narrower_than_the_body_holds_the_jump() {
        let door = |w: f32| {
            let mut t = face_x(6.5, 0.0, 5.0 - 0.5 * w, 0.0, 6.0).to_vec();
            t.extend(face_x(6.5, 5.0 + 0.5 * w, 10.0, 0.0, 6.0));
            t
        };
        let grid = world(2.0, 2.0, 1.0, 0.0, &door(0.5));
        assert!(way_across(&grid, 2.0, 2.0).is_none());
        let grid = world(2.0, 2.0, 1.0, 0.0, &door(1.5));
        let kinds = way_across(&grid, 2.0, 2.0).expect("a way through the door");
        assert!(kinds.contains(&LinkKind::GapJump), "{kinds:?}");
    }

    /// A riser 0.8 m high, 0.5 m on from the edge it is jumped from: the
    /// body stands against it while the clip lifts it, then goes on over
    /// it, as the stand-in flies it (and as players jump it).
    #[test]
    fn a_riser_next_to_the_take_off_is_jumped_onto() {
        let grid = world(2.0, 2.8, 0.5, 0.0, &[]);
        let kinds = way_across(&grid, 2.0, 2.8).expect("a way up");
        assert!(kinds.contains(&LinkKind::GapJump), "{kinds:?}");
    }

    /// A lower block across a gap of 1.5 m, 1.5 m down: a gap drop (the
    /// ground 5 m under the near block has no way up to it), pressed early
    /// enough to come down past the gap. Across 2.5 m: none.
    #[test]
    fn a_lower_block_across_a_gap_is_dropped_to() {
        let grid = world(2.0, 0.5, 1.5, -3.0, &[]);
        let kinds = way_across(&grid, 2.0, 0.5).expect("a way across");
        assert!(kinds.contains(&LinkKind::GapDrop), "{kinds:?}");
        let grid = world(2.0, 0.5, 2.5, -3.0, &[]);
        assert!(way_across(&grid, 2.0, 0.5).is_none());
    }

    /// A lower block 1.2 m down across a 1 m gap: walked off (no key), the
    /// body keeps its speed and comes down on it at a walk and a sprint
    /// alike: a gap drop walked off, made before any pressed one.
    #[test]
    fn a_lower_block_close_across_a_gap_is_walked_off_to() {
        let grid = world(2.0, 0.8, 1.0, -3.0, &[]);
        let off = grid
            .off_links()
            .iter()
            .filter(|l| l.kind == LinkKind::GapDrop && l.press == Takeoff::WalkOff)
            .count();
        assert!(off > 0, "{:?}", grid.off_links());
        let kinds = way_across(&grid, 2.0, 0.8).expect("a way across");
        assert!(kinds.contains(&LinkKind::GapDrop), "{kinds:?}");
    }

    /// Up 0.6 m across 0.75 m: within the arc; up 1.2 m: not.
    #[test]
    fn a_higher_block_across_a_gap_is_jumped_to_within_the_arc() {
        let grid = world(2.0, 2.6, 0.75, 0.0, &[]);
        let kinds = way_across(&grid, 2.0, 2.6).expect("a way across");
        assert!(kinds.contains(&LinkKind::GapJump), "{kinds:?}");
        let grid = world(2.0, 3.2, 0.75, 0.0, &[]);
        assert!(way_across(&grid, 2.0, 3.2).is_none());
    }

    /// The gap links read back from a file as written, and every query
    /// over them agrees: links in and out, the way back of a search.
    #[test]
    fn gap_links_are_written_read_and_walked_both_ways() {
        let grid = world(2.0, 2.0, 1.0, 0.0, &[]);
        assert!(!grid.off_links().is_empty());
        let path = std::env::temp_dir().join(format!("gap-links-{}.grid", std::process::id()));
        grid.write(&path, 5).unwrap();
        let back = WalkGrid::read(&path, 5).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(back.digest(), grid.digest());
        assert_eq!(back.off_links(), grid.off_links());
        for l in grid.off_links() {
            assert!(grid.links(l.from).any(|(n, k)| n == l.to && k == l.kind));
            assert!(grid.incoming(l.to).any(|(n, k)| n == l.from && k == l.kind));
            assert!(grid.entered_from(l.to, |n| n == l.from));
            assert!(grid.link_cost(l.from, l.to, l.kind).is_finite());
        }
        // A search's ways back over a gap link lead to its take-off.
        let l = grid.off_links()[0];
        let found = grid.path(l.from, l.to).expect("a way");
        assert_eq!(found.nodes, vec![l.from, l.to]);
    }

    /// Without the gap links (the switch's default) a build is the one
    /// before they were made: no off link, the key of [`crate::grid_key`],
    /// and a file with no table for them, read back as written.
    #[test]
    fn a_build_without_gap_links_is_the_one_before_them() {
        let t = world_triangles(2.0, 2.0, 1.0, 0.0);
        let grid = WalkGrid::build(&t, AREA, 0.25, Body::default());
        assert!(grid.off_links().is_empty());
        assert_eq!(grid.digest(), WalkGrid::build_with_links(&t, AREA, 0.25, Body::default(), OffMeshLinks::None).digest());
        let body = Body::default();
        assert_eq!(
            crate::grid_key(&t, AREA, 0.25, body, &[]),
            crate::grid_key_with_links(&t, AREA, 0.25, body, &[], OffMeshLinks::None)
        );
        assert_ne!(
            crate::grid_key(&t, AREA, 0.25, body, &[]),
            crate::grid_key_with_links(&t, AREA, 0.25, body, &[], OffMeshLinks::Gaps)
        );
        let path = std::env::temp_dir().join(format!("no-gap-links-{}.grid", std::process::id()));
        grid.write(&path, 6).unwrap();
        let back = WalkGrid::read(&path, 6).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(back.digest(), grid.digest());
        assert!(back.off_links().is_empty());
    }
}

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
    /// The sprint jump, pressed this far before the take-off's centre at a
    /// sprint, facing the link's way ([`SPRINT_RUN_UP_M`] of floor run
    /// before the press, turning onto the line at most square to it).
    Sprint(PressLead),
}

/// A sprint link's press in a built file: this bit over its centimetres.
const SPRINT_BIT: u16 = 0x8000;

impl Takeoff {
    /// In a built file: a press's centimetres, a walk-off 0 (no press is
    /// under [`GAP_PRESS_MIN_M`]), a sprint press's with [`SPRINT_BIT`].
    pub(super) fn centimetres(self) -> u16 {
        match self {
            Takeoff::Press(lead) => lead.centimetres(),
            Takeoff::WalkOff => 0,
            Takeoff::Sprint(lead) => SPRINT_BIT | lead.centimetres(),
        }
    }

    pub(super) fn from_centimetres(cm: u16) -> Self {
        match cm {
            0 => Takeoff::WalkOff,
            cm if cm & SPRINT_BIT != 0 => Takeoff::Sprint(PressLead::from_centimetres(cm & !SPRINT_BIT)),
            cm => Takeoff::Press(PressLead::from_centimetres(cm)),
        }
    }

    pub(super) fn is_valid(self) -> bool {
        match self {
            Takeoff::Press(lead) => lead.is_valid(),
            Takeoff::WalkOff => true,
            Takeoff::Sprint(lead) => (0.0..=SPRINT_PRESS_MAX_M + 1e-3).contains(&lead.metres()),
        }
    }

    /// The link's cost over [`gap_cost`]: a sprint jump's run-up and its
    /// lower success ([`SPRINT_GAP_PENALTY`]).
    pub fn penalty(self) -> f32 {
        match self {
            Takeoff::Press(_) | Takeoff::WalkOff => 0.0,
            Takeoff::Sprint(_) => SPRINT_GAP_PENALTY,
        }
    }

    /// The sprint a link takes, metres run at a sprint before its press
    /// (none for a walking jump or a walk-off): what the body's stamina
    /// must hold, with the jump's own cost (the owner's A56 rule).
    pub fn sprint_run_m(self) -> Option<f32> {
        match self {
            Takeoff::Press(_) | Takeoff::WalkOff => None,
            Takeoff::Sprint(_) => Some(SPRINT_RUN_UP_M),
        }
    }
}

/// The client's sprint jump (`sprint_from_left_leg`, `_right_leg`; codes
/// 2-4): its root's way ahead of the clip's start, every 2 of its 24
/// frames, m, and its rise over the start's height for each leg (PROVEN,
/// the stock clips' roots, `sprint-jump-re/FINDINGS.md` §5). The leg is the
/// gait's, not the bot's to choose.
pub const SPRINT_JUMP_AHEAD: [f32; 13] = [
    0.0, 0.38, 0.76, 1.15, 1.54, 1.92, 2.29, 2.65, 3.01, 3.36, 3.71, 4.06, 4.42,
];
pub const SPRINT_JUMP_RISE: [[f32; 13]; 2] = [
    [0.0, 0.0, 0.0, 0.0, 0.57, 1.08, 1.24, 1.28, 1.23, 1.01, 0.70, 0.35, 0.0],
    [0.0, 0.0, 0.0, 0.0, 0.56, 1.03, 1.29, 1.35, 1.25, 1.05, 0.77, 0.42, 0.0],
];
/// The clip's root at its jump event (frame 6), m: the body leaves the
/// ground there (PROVEN, FINDINGS §3).
const SPRINT_LEAVES_M: f32 = 1.17;
/// The sprint clips' rate (5 / 5.5, PROVEN, FINDINGS §3) and their frames
/// a second.
const SPRINT_CLIP_RATE: f32 = 0.909;
const CLIP_FPS: f32 = 30.0;
/// The prepare phase's travel before the clip starts, m: 0-1.64 by the
/// gait's phase at the press, which no bot sees (PROVEN, FINDINGS §3.2;
/// `re-sprint-prepare.md`). A sprint link holds for every one of these.
/// The ends first: a flight that misses misses there.
pub const SPRINT_PREPARE_M: [f32; 5] = [0.0, 1.64, 0.82, 0.41, 1.23];
/// The prepare's root speed, m/s of clip time (5.455, PROVEN), and the
/// body's speed meanwhile: the walk cap (3.5 m/s, PROVEN, FINDINGS §3.2).
const SPRINT_PREPARE_ROOT_MS: f32 = 5.455;
const WALK_CAP_MS: f32 = 3.5;
/// After a jump's clip ends over a gap the body keeps going forward while
/// it falls, m/s: a walking jump 3.0-3.6 (PROVEN, J0 g2b 2026-10-26, two
/// tries: 3.1-3.6 m/s through a 6 m fall; the owner's 4fdb062b 195.8 s,
/// 3.4), a sprint jump 3.5-5.3 (LIKELY: the 8f flights off the woodbox
/// 4.1-5.3 m/s, the owner's 5dbc6cee 331.2 s about 5; the landing clip's
/// root 5.3). A link holds for both ends.
pub const WALK_JUMP_AFTER_CLIP_MS: [f32; 2] = [3.0, 3.6];
pub const SPRINT_JUMP_AFTER_CLIP_MS: [f32; 2] = [3.5, 5.3];
/// The sprint presses looked at, m before the take-off floor's centre:
/// from at the centre to [`SPRINT_PRESS_MAX_M`], every
/// [`SPRINT_PRESS_STEP_M`] (the prepare's 1.64 m spread is far wider).
pub const SPRINT_PRESS_MAX_M: f32 = 3.0;
pub const SPRINT_PRESS_STEP_M: f32 = 0.2;
/// The farthest a sprint jump comes down past its take-off's edge, m: the
/// clip (4.42) from the farthest prepare (1.64) and a fall on of the
/// harmless drop at the fastest speed after the clip (about 2.6). A
/// take-off with no floor in reach there gets no flight.
const SPRINT_REACH_M: f32 = 8.7;
/// A sprint gap link is made over a gap this wide or more with nothing
/// within the harmless drop in it, m: a narrower one the walking jump and
/// the walk-off take, and over ground within the drop the body drops in
/// and walks on.
pub const SPRINT_GAP_MIN_M: f32 = 1.0;
/// A sprint jump comes down where its descent first meets floor, from
/// prepares 1.64 m apart: the landing's floor, walked on from cell to
/// cell, runs on this far or more along its way, m (and [`WalkGrid::lands`]
/// asks two floors more).
const SPRINT_LANDING_RUN_M: f32 = 0.75;
/// A sprint jump takes this much floor run at a sprint before its press,
/// m: a sprint from rest is at speed by 1-2 m (J0 8e, PROVEN); the grid's
/// sprint-jump links ask the same (A55).
pub const SPRINT_RUN_UP_M: f32 = 1.5;
/// A sprint gap link's cost over a walking one's, m of walk. GUESS: the
/// run-up, the stamina and the landing's spread (benched).
pub const SPRINT_GAP_PENALTY: f32 = 3.0;
/// A fall that lands at this vertical speed or more hurts too much, m/s
/// (A86: 12.2 m/s kills; harmless under 9.9, `docs/notes/fall_damage.md`).
pub const FALL_HURTS_MS: f32 = 11.9;

/// One clip's flight, for [`WalkGrid::fly_clip`]: its root's way ahead and
/// rise per sample, the root's way at the jump event, and the root's
/// downward speed at its end (from its last two samples).
struct Clip<'a> {
    ahead: &'a [f32; 13],
    rise: &'a [f32; 13],
    leaves: f32,
    /// Seconds between two samples (2 frames at the clip's rate).
    sample_s: f32,
}

impl Clip<'_> {
    fn rise_at(&self, ahead: f32) -> Option<f32> {
        let end = self.ahead[12];
        if !(0.0..=end).contains(&ahead) {
            return None;
        }
        let i = self.ahead.partition_point(|&a| a <= ahead).clamp(1, 12);
        let (a0, a1) = (self.ahead[i - 1], self.ahead[i]);
        let t = ((ahead - a0) / (a1 - a0)).clamp(0.0, 1.0);
        Some(self.rise[i - 1] + (self.rise[i] - self.rise[i - 1]) * t)
    }

    fn end_fall_ms(&self) -> f32 {
        (self.rise[11] - self.rise[12]) / self.sample_s
    }
}

/// Where a jump's clip leaves a body: come down on a floor during it (the
/// floor, where along, the speed it came down at), or over no floor at the
/// clip's end (where along, the feet's height and its height a sample
/// before, falling at `down` m/s): it falls on from there at a speed
/// after the clip ([`WalkGrid::fall_on`]).
#[derive(Clone, Copy)]
enum Flight {
    Landed(u32, f32, f32),
    Over { body: f32, y: f32, last: f32, down: f32 },
}

/// Seconds from a sprint press to the jump event for a prepare of `prepare`
/// metres: the prepare (`rem / r`, r = 0.5 (s + rem / 0.1), PROVEN,
/// FINDINGS §3.2) and the clip's 6 frames at its rate.
fn sprint_press_to_leave_s(prepare: f32) -> f32 {
    let rem = prepare / SPRINT_PREPARE_ROOT_MS;
    let r = 0.5 * (SPRINT_CLIP_RATE + rem / 0.1);
    let prep = if rem > 0.0 { rem / r } else { 0.0 };
    prep + 6.0 / (CLIP_FPS * SPRINT_CLIP_RATE)
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
    /// … of the made, sprint jumps …
    pub sprint: usize,
    /// … dropped: no way back from the landing to the take-off …
    pub one_way: usize,
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
    /// After the clip it falls on ([`Flight::Over`], [`Self::fall_on`]).
    fn fly(&self, a: usize, d: usize, lead: f32, solids: &Solids) -> Option<Flight> {
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
                // After the clip: on at `after` while it falls, from the
                // clip's last downward speed (J0 g2b).
                let n = SHORT_JUMP_RISE.len();
                let down = (SHORT_JUMP_RISE[n - 2] - SHORT_JUMP_RISE[n - 1]) * CLIP_FPS / 2.0;
                return Some(Flight::Over { body, y: h, last, down });
            }
            let ahead = x + lead;
            let y = h + short_jump_rise(ahead).unwrap_or(0.0);
            // Coming down onto a floor at the root: landed (before the
            // faces: its own top is under the feet), only while no face
            // holds the body back: the root runs on past a wall the body
            // stands at (a wall thinner than a cell is no landing beyond).
            if ahead >= APEX_AHEAD_M
                && body + ds + 1e-3 >= x
                && let Some(f) = landing(x, y, last)
            {
                return Some(Flight::Landed(f, x, 0.0));
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
                return Some(Flight::Landed(f, body, 0.0));
            }
            last = y;
            x += ds;
        }
    }

    /// A body in the air `body` metres along `d` from floor `a`'s centre,
    /// its feet at `y` and falling at `down` m/s, going on at `speed`: the
    /// first floor it comes down to (no higher than `last`, the feet's
    /// height a sample before), where along, and the speed it comes down
    /// at. Held by a face ahead, it falls where it is. `None` past the
    /// harmless drop under `top` (the take-off's height).
    #[allow(clippy::too_many_arguments)]
    fn fall_on(
        &self,
        a: usize,
        d: usize,
        body: f32,
        y: f32,
        down: f32,
        speed: f32,
        top: f32,
        last: f32,
        solids: &Solids,
    ) -> Option<(u32, f32, f32)> {
        let from = self.position(a as NodeId);
        let (di, dk) = DIRS[d];
        let len = ((di * di + dk * dk) as f32).sqrt();
        let (ux, uz) = (di as f32 / len, dk as f32 / len);
        let ds = 0.5 * self.cell;
        let at = |s: f32| (from.x + ux * s, from.z + uz * s);
        let floor_between = |s: f32, low: f32, high: f32| {
            let (px, pz) = at(s);
            let cell = self.cell_at(px, pz)?;
            (self.start[cell]..self.start[cell + 1])
                .filter(|&f| {
                    let fy = self.height[f as usize];
                    fy <= high + 0.05 && fy >= low - 1e-4
                })
                .max_by(|&p, &q| self.height[p as usize].total_cmp(&self.height[q as usize]))
        };
        let bottom = top - self.body.max_drop;
        let (mut x, mut last, mut t) = (body, last, 0.0f32);
        loop {
            // Where it is: a floor it is coming down onto.
            let fy = y - down * t - 0.5 * GRAVITY * t * t;
            if let Some(f) = floor_between(x, fy.max(bottom), last) {
                let fall = (last.max(y) - self.height[f as usize]).max(0.0);
                return Some((f, x, (down * down + 2.0 * GRAVITY * fall).sqrt()));
            }
            if fy < bottom {
                return None;
            }
            // Held by a face ahead: straight down where it is.
            if speed <= 0.0 || self.face_ahead(solids, at(x + self.body.radius), (ux, uz), fy) {
                let f = floor_between(x, bottom, fy)?;
                let fall = (y - self.height[f as usize]).max(0.0);
                return Some((f, x, (down * down + 2.0 * GRAVITY * fall).sqrt()));
            }
            last = fy;
            t += ds / speed;
            x += ds;
        }
    }

    /// Where a [`Flight`] from floor `a` along `d` comes down when the body
    /// goes on at `after` m/s after the clip: the floor, where along, the
    /// speed it comes down at.
    fn land(&self, a: usize, d: usize, flight: Flight, after: f32, solids: &Solids) -> Option<(u32, f32, f32)> {
        match flight {
            Flight::Landed(f, at, speed) => Some((f, at, speed)),
            Flight::Over { body, y, last, down } => self.fall_on(a, d, body, y, down, after, self.height[a], last, solids),
        }
    }

    /// How far along `d` from floor `a`'s centre the surface of `a`'s
    /// height goes on under a body's centre (past the floors erosion took
    /// from the edge), up to 2 m.
    fn edge_ahead(&self, a: usize, d: usize, solids: &Solids) -> f32 {
        let from = self.position(a as NodeId);
        let h = self.height[a];
        let (di, dk) = DIRS[d];
        let len = ((di * di + dk * dk) as f32).sqrt();
        let (ux, uz) = (di as f32 / len, dk as f32 / len);
        let ds = 0.5 * self.cell;
        let surface_under = |s: f32| {
            self.cell_at(from.x + ux * s, from.z + uz * s).is_some_and(|c| {
                solids.spans[solids.start[c] as usize..solids.start[c + 1] as usize]
                    .iter()
                    .any(|&(_, hi)| (hi - h).abs() <= 0.06)
            })
        };
        let mut edge = 0.0f32;
        while edge < 2.0 && surface_under(edge + ds) {
            edge += ds;
        }
        edge
    }

    /// One flight of a jump `clip` whose root starts `start` metres along
    /// `d` from floor `a`'s centre, the body `pre` along when the clip's
    /// jump event lifts it and `sunk` under `a`'s height (fallen off the
    /// edge before it: the rise is then over the fallen height, J0 g2b and
    /// the owner's 5dbc6cee): held by faces as [`Self::fly`] holds it.
    #[allow(clippy::too_many_arguments)]
    fn fly_clip(
        &self,
        a: usize,
        d: usize,
        clip: &Clip,
        start: f32,
        pre: f32,
        sunk: f32,
        solids: &Solids,
    ) -> Option<Flight> {
        let from = self.position(a as NodeId);
        let h = self.height[a] - sunk;
        let (di, dk) = DIRS[d];
        let len = ((di * di + dk * dk) as f32).sqrt();
        let (ux, uz) = (di as f32 / len, dk as f32 / len);
        let ds = 0.5 * self.cell;
        let at = |s: f32| (from.x + ux * s, from.z + uz * s);
        let apex = clip.ahead[(0..13).max_by(|&i, &j| clip.rise[i].total_cmp(&clip.rise[j])).unwrap_or(0)];
        let landing = |s: f32, y: f32, top: f32| {
            let (px, pz) = at(s);
            let cell = self.cell_at(px, pz)?;
            (self.start[cell]..self.start[cell + 1])
                .filter(|&f| {
                    let fy = self.height[f as usize];
                    fy <= top + 0.05 && fy >= y - 1e-4
                })
                .max_by(|&p, &q| self.height[p as usize].total_cmp(&self.height[q as usize]))
        };
        let end = start + clip.ahead[12];
        let mut last = h;
        let mut body = pre;
        let mut x = start + clip.leaves;
        loop {
            if x > end {
                return Some(Flight::Over { body, y: h, last, down: clip.end_fall_ms() });
            }
            let ahead = x - start;
            let y = h + clip.rise_at(ahead).unwrap_or(0.0);
            let fall = |f: u32| ((last.max(y) - self.height[f as usize]).max(0.0) * 2.0 * GRAVITY).sqrt();
            // Coming down at the root onto a floor (a riser's top under the
            // feet), only while no face holds the body back: the sprint
            // clip's root runs far past a wall the body stands at.
            if ahead >= apex
                && body + ds + 1e-3 >= x
                && let Some(f) = landing(x, y, last)
            {
                return Some(Flight::Landed(f, x, fall(f)));
            }
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
            } else if ahead >= apex
                && let Some(f) = landing(body, y, last)
            {
                return Some(Flight::Landed(f, body, fall(f)));
            }
            last = y;
            x += ds;
        }
    }

    /// Where a sprint jump from floor `a` along `d`, pressed `lead` metres
    /// before `a`'s centre, comes down, when it is one level for every
    /// prepare of [`SPRINT_PREPARE_M`], both legs and both speeds after the
    /// clip ([`SPRINT_JUMP_AFTER_CLIP_MS`]), and no landing hurts
    /// ([`FALL_HURTS_MS`]). Between the press and the jump event the body
    /// walks at the walk cap while the root runs ahead (FINDINGS §3.2); past
    /// the edge (`edge` along) it falls meanwhile.
    fn sprint_landing(&self, a: usize, d: usize, lead: f32, edge: f32, solids: &Solids) -> Option<u32> {
        let sample_s = 2.0 / (CLIP_FPS * SPRINT_CLIP_RATE);
        let mut level: Option<(u32, f32)> = None;
        for prepare in SPRINT_PREPARE_M {
            let to_leave = sprint_press_to_leave_s(prepare);
            let start = -lead + prepare;
            let pre = self.walk_until_face(a, d, -lead, (-lead + WALK_CAP_MS * to_leave).min(start + SPRINT_LEAVES_M), solids);
            let sunk = if pre > edge {
                let air = to_leave - (edge + lead).max(0.0) / WALK_CAP_MS;
                0.5 * GRAVITY * air * air
            } else {
                0.0
            };
            if sunk > self.body.step {
                return None;
            }
            for rise in &SPRINT_JUMP_RISE {
                let clip = Clip { ahead: &SPRINT_JUMP_AHEAD, rise, leaves: SPRINT_LEAVES_M, sample_s };
                let flight = self.fly_clip(a, d, &clip, start, pre, sunk, solids)?;
                for after in SPRINT_JUMP_AFTER_CLIP_MS {
                    let (to, along, speed) = self.land(a, d, flight, after, solids)?;
                    // Across the gap, unhurt.
                    if along <= edge || speed >= FALL_HURTS_MS {
                        return None;
                    }
                    let y = self.height[to as usize];
                    match level {
                        None => level = Some((to, y)),
                        Some((_, l)) if (y - l).abs() <= SAME_TOP => {}
                        Some(_) => return None,
                    }
                }
            }
        }
        level.map(|(to, _)| to)
    }

    /// How far a body walking along `d` from `from` towards `to` metres
    /// (along from floor `a`'s centre, on its height) gets before a face
    /// holds it.
    fn walk_until_face(&self, a: usize, d: usize, from: f32, to: f32, solids: &Solids) -> f32 {
        let p = self.position(a as NodeId);
        let (di, dk) = DIRS[d];
        let len = ((di * di + dk * dk) as f32).sqrt();
        let (ux, uz) = (di as f32 / len, dk as f32 / len);
        let ds = 0.5 * self.cell;
        let mut s = from;
        while s + ds <= to {
            let f = s + ds + self.body.radius;
            if self.face_ahead(solids, (p.x + ux * f, p.z + uz * f), (ux, uz), self.height[a]) {
                return s;
            }
            s += ds;
        }
        to.max(from)
    }

    /// Whether [`SPRINT_RUN_UP_M`] of floor runs to `press` from behind or
    /// beside its way `d` (a run that turns onto the line at most square to
    /// it): walked over walk links on its level, never ahead of the press.
    fn sprint_run_up(&self, wide: &Wide, press: usize, d: usize) -> bool {
        let p = self.position(press as NodeId);
        let (di, dk) = DIRS[d];
        let len = ((di * di + dk * dk) as f32).sqrt();
        let (ux, uz) = (di as f32 / len, dk as f32 / len);
        // Straight back first (most run-ups).
        let back = d ^ 1;
        let step = self.cell * len;
        let (mut f, mut run) = (press, 0.0f32);
        while run + 1e-3 < SPRINT_RUN_UP_M {
            let b = wide.links[f][back];
            if b == NONE || wide.kinds[f][back] != LinkKind::Walk || (self.height[b as usize] - p.y).abs() > self.body.step {
                break;
            }
            run += step;
            f = b as usize;
        }
        if run + 1e-3 >= SPRINT_RUN_UP_M {
            return true;
        }
        let mut dist: Vec<(usize, f32)> = vec![(press, 0.0)];
        let mut open: Vec<(usize, f32)> = vec![(press, 0.0)];
        while let Some(i) = (0..open.len()).min_by(|&i, &j| open[i].1.total_cmp(&open[j].1)) {
            let (f, at) = open.swap_remove(i);
            if at + 1e-3 >= SPRINT_RUN_UP_M {
                return true;
            }
            for e in 0..8 {
                let b = wide.links[f][e];
                if b == NONE || wide.kinds[f][e] != LinkKind::Walk {
                    continue;
                }
                let b = b as usize;
                let q = self.position(b as NodeId);
                if (q.y - p.y).abs() > self.body.step || (q.x - p.x) * ux + (q.z - p.z) * uz > 0.25 * self.cell {
                    continue;
                }
                let (ei, ek) = DIRS[e];
                let next = at + self.cell * ((ei * ei + ek * ek) as f32).sqrt();
                match dist.iter_mut().find(|(g, _)| *g == b) {
                    Some((_, old)) if *old <= next => {}
                    Some((_, old)) => {
                        *old = next;
                        open.push((b, next));
                    }
                    None => {
                        dist.push((b, next));
                        open.push((b, next));
                    }
                }
            }
        }
        false
    }

    /// The sprint gap links from floor `a` along `d`, where the walking
    /// jumps and the walk-off made none: for each window of presses
    /// [`SPRINT_PRESS_STEP_M`] apart, from `a`'s centre back over the floor
    /// walked to behind it (`behind` metres) up to [`SPRINT_PRESS_MAX_M`],
    /// that all come down on one level two or more cells on
    /// ([`Self::sprint_landing`]), at least [`GAP_PRESS_SPREAD_M`] either
    /// side of its middle, with a run-up to the middle's floor
    /// ([`Self::sprint_run_up`]), a link pressed there.
    #[allow(clippy::too_many_arguments)]
    fn sprint_gap_link(
        &self,
        wide: &Wide,
        a: usize,
        d: usize,
        behind: f32,
        solids: &Solids,
        stats: &mut GapStats,
        out: &mut Vec<OffLink>,
    ) {
        let h = self.height[a];
        let (i, k) = self.cell_xz(self.cell_of[a] as usize);
        let (di, dk) = DIRS[d];
        let step = self.cell * ((di * di + dk * dk) as f32).sqrt();
        let edge = self.edge_ahead(a, d, solids);
        // A deep gap: nothing to come down on within the harmless drop for
        // [`SPRINT_GAP_MIN_M`] past the edge (else the body drops in and
        // walks on), and a floor in reach past that.
        let from = self.position(a as NodeId);
        let len = ((di * di + dk * dk) as f32).sqrt();
        let (ux, uz) = (di as f32 / len, dk as f32 / len);
        let (low, high) = (h - self.body.max_drop, h + self.body.max_jump.min(MARGINAL_JUMP_M));
        let floors_at = |x: f32| {
            self.cell_at(from.x + ux * x, from.z + uz * x).map(|c| {
                (self.start[c]..self.start[c + 1])
                    .map(|f| self.height[f as usize])
                    .filter(|y| (low..=high).contains(y))
            })
        };
        // The landing's level runs on along the way as far as the prepare
        // spreads the landings ([`SPRINT_LANDING_RUN_M`]).
        let ds = 0.5 * self.cell;
        let mut runs: Vec<(f32, f32, f32)> = Vec::new();
        let mut x = edge;
        let in_reach = loop {
            x += ds;
            if x > edge + SPRINT_REACH_M {
                break false;
            }
            let Some(ys) = floors_at(x + self.body.radius) else {
                break false;
            };
            let mut any = false;
            for y in ys {
                any = true;
                // Walked on from the cell before (a ramp's floor too).
                match runs.iter_mut().find(|(l, _, last)| (y - *l).abs() <= self.body.step && *last + ds + 1e-3 >= x) {
                    Some((l, _, last)) => (*l, *last) = (y, x),
                    None => runs.push((y, x, x)),
                }
            }
            if any && x <= edge + SPRINT_GAP_MIN_M {
                break false;
            }
            if runs.iter().any(|&(_, first, last)| last - first >= SPRINT_LANDING_RUN_M) {
                break true;
            }
        };
        if !in_reach {
            stats.no_landing += 1;
            return;
        }
        // A run-up to any press behind `a` runs on to `a` (behind or beside
        // it all the way): none to `a`, none to a press.
        if !self.sprint_run_up(wide, a, d) {
            stats.no_run_up += 1;
            return;
        }
        let presses = (SPRINT_PRESS_MAX_M.min(behind) / SPRINT_PRESS_STEP_M + 1e-3).floor() as usize + 1;
        let half = (GAP_PRESS_SPREAD_M / SPRINT_PRESS_STEP_M).round() as usize;
        let at = |p: usize| {
            let to = self.sprint_landing(a, d, p as f32 * SPRINT_PRESS_STEP_M, edge, solids)?;
            let (ti, tk) = self.cell_xz(self.cell_of[to as usize] as usize);
            ((ti - i).abs().max((tk - k).abs()) >= 2).then_some(to)
        };
        // Every window of `2 half + 1` presses holds a press of this stride:
        // those first, the rest only next to one that lands.
        let stride = 2 * half + 1;
        let mut landing: Vec<Option<u32>> = vec![None; presses];
        let mut looked = vec![false; presses];
        for p in (0..presses).step_by(stride) {
            landing[p] = at(p);
            looked[p] = true;
        }
        for p in (0..presses).step_by(stride) {
            if landing[p].is_some() {
                for q in p.saturating_sub(2 * half)..(p + 2 * half + 1).min(presses) {
                    if !looked[q] {
                        landing[q] = at(q);
                        looked[q] = true;
                    }
                }
            }
        }
        let mut p = 0;
        while p < presses {
            let Some(first) = landing[p] else {
                p += 1;
                continue;
            };
            let level = self.height[first as usize];
            let mut q = p;
            while q + 1 < presses && landing[q + 1].is_some_and(|t| (self.height[t as usize] - level).abs() <= SAME_TOP) {
                q += 1;
            }
            if q - p >= 2 * half {
                let mid = (p + q) / 2;
                let lead = mid as f32 * SPRINT_PRESS_STEP_M;
                let to = landing[mid].expect("in the window");
                let to_y = self.height[to as usize];
                // The press's floor: walked back from `a` along the way.
                let mut press = a;
                for _ in 0..(lead / step + 1e-3).floor() as usize {
                    match wide.links[press][d ^ 1] {
                        NONE => break,
                        b => press = b as usize,
                    }
                }
                if to_y - h > self.body.max_jump.min(MARGINAL_JUMP_M) {
                    stats.too_high += 1;
                } else if !self.sprint_run_up(wide, press, d) {
                    stats.no_run_up += 1;
                } else if self.ceiling[to as usize] - to_y >= self.body.stand_height && Self::lands(wide, to as usize, d) {
                    out.push(OffLink {
                        from: a as NodeId,
                        to,
                        kind: if to_y - h > -self.body.step { LinkKind::GapJump } else { LinkKind::GapDrop },
                        press: Takeoff::Sprint(PressLead::from_metres(lead)),
                    });
                    stats.made += 1;
                    stats.sprint += 1;
                } else {
                    stats.no_room += 1;
                }
            } else {
                stats.narrow_window += 1;
            }
            p = q + 1;
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

    /// How far from `a`'s centre in direction `d` a body walks on `a`'s height before it falls ([`Self::walk_off`]): while a
    /// surface of that height is under its centre (past the floors erosion took from the edge), up to 2 m.
    fn walk_off_edge(&self, a: usize, d: usize, solids: &Solids) -> f32 {
        let from = self.position(a as NodeId);
        let h = self.height[a];
        let (di, dk) = DIRS[d];
        let len = ((di * di + dk * dk) as f32).sqrt();
        let (ux, uz) = (di as f32 / len, dk as f32 / len);
        let ds = 0.5 * self.cell;
        let surface_under = |s: f32| {
            let (px, pz) = (from.x + ux * s, from.z + uz * s);
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
        edge
    }

    /// [A238] The floor a body comes down on when it slips off `a`'s edge in direction `d` slower than any walk-off is flown for
    /// (`Body::slip_landings`): it goes over where a walk-off does ([`Self::walk_off_edge`], half a cell on) or where the
    /// capsule stops being held by the edge ([`WALK_OFF_REACH_M`] past the floor's centre), whichever is further, and falls about
    /// straight down, onto the highest floor lower than a step and within the harmless drop whose cell overlaps the square of the
    /// body's radius and half a cell around that point, and whose free space reaches above the take-off ([`Self::open_to`]): the bots'
    /// reckoning's slip (`body::under_edge` from `body::SUPPORT_M` under `kin.walk_off_landing = by_speed`).
    ///
    /// [A247] A floor with a slab, a plate or the take-off's own overhang between it and the edge is not one a falling capsule
    /// comes down on (the client sweeps the capsule straight down, `step_down_sweep_test_impl` 0x7EA80). Taking the highest floor
    /// of the square alone put Vector Laboratory's slips on the floors under the 20.6 walkway and under the 17.78 plate over the
    /// 16.21 pillar top, none of which a body falling past the edge meets.
    fn slip_landing(&self, a: usize, d: usize, solids: &Solids) -> Option<u32> {
        let from = self.position(a as NodeId);
        let h = self.height[a];
        let (di, dk) = DIRS[d];
        let len = ((di * di + dk * dk) as f32).sqrt();
        let (ux, uz) = (di as f32 / len, dk as f32 / len);
        let ds = 0.5 * self.cell;
        let s = (self.walk_off_edge(a, d, solids) + ds).max(WALK_OFF_REACH_M);
        let (px, pz) = (from.x + ux * s, from.z + uz * s);
        let r = self.body.radius + ds;
        self.floors_in([px - r, pz - r, px + r, pz + r])
            .filter(|&f| {
                let fy = self.height[f as usize];
                fy < h - self.body.step && fy >= h - self.body.max_drop && self.open_to(f, h)
            })
            .max_by(|&p, &q| self.height[p as usize].total_cmp(&self.height[q as usize]))
    }

    /// [A238] `Body::slip_landings`: each walk-off gap drop's take-off and the floor a slip off it comes down on
    /// ([`Self::slip_landing`]) where that is not the walk-off's own landing level, by take-off: the reach's floods take them
    /// (`Wide::slips`), no link does.
    pub(super) fn slip_landings(&self, wide: &Wide, solids: &Solids) -> Vec<(u32, u32)> {
        let mut slips: Vec<(u32, u32)> = wide
            .off
            .iter()
            .filter(|l| l.press == Takeoff::WalkOff)
            .filter_map(|l| {
                let (fi, fk) = self.cell_xz(self.cell_of[l.from as usize] as usize);
                let (ti, tk) = self.cell_xz(self.cell_of[l.to as usize] as usize);
                let d = DIRS.iter().position(|&dir| dir == ((ti - fi).signum(), (tk - fk).signum()))?;
                let s = self.slip_landing(l.from as usize, d, solids)?;
                ((self.height[s as usize] - self.height[l.to as usize]).abs() > SAME_TOP).then_some((l.from, s))
            })
            .collect();
        slips.sort_unstable();
        slips.dedup();
        slips
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
        let edge = self.walk_off_edge(a, d, solids);
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
        while behind < SPRINT_PRESS_MAX_M.max(GAP_PRESS_MAX_M + GAP_RUN_UP_M) {
            let b = wide.links[f][back];
            if b == NONE || wide.kinds[f][back] != LinkKind::Walk {
                break;
            }
            behind += step;
            f = b as usize;
        }
        // The walking ways first; a sprint jump only where they make none.
        let before = out.len();
        self.walking_gap_link(wide, a, d, behind, solids, stats, out);
        if out.len() == before {
            self.sprint_gap_link(wide, a, d, behind, solids, stats, out);
        }
    }

    /// The walking gap links from floor `a` along `d` with `behind` metres
    /// of floor walked to behind it ([`Self::gap_link`]): the walk-off, and
    /// each window of short-jump presses.
    #[allow(clippy::too_many_arguments)]
    fn walking_gap_link(
        &self,
        wide: &Wide,
        a: usize,
        d: usize,
        behind: f32,
        solids: &Solids,
        stats: &mut GapStats,
        out: &mut Vec<OffLink>,
    ) {
        let h = self.height[a];
        let (i, k) = self.cell_xz(self.cell_of[a] as usize);
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
                // One level whatever the body's speed after the clip.
                let flight = self.fly(a, d, lead, solids)?;
                let (to, _, _) = self.land(a, d, flight, WALK_JUMP_AFTER_CLIP_MS[0], solids)?;
                let (other, _, _) = self.land(a, d, flight, WALK_JUMP_AFTER_CLIP_MS[1], solids)?;
                if (self.height[to as usize] - self.height[other as usize]).abs() > SAME_TOP {
                    return None;
                }
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
        found.sort_by_key(|&(link, _)| match link.press {
            Takeoff::WalkOff => 0,
            Takeoff::Press(_) => 1,
            Takeoff::Sprint(_) => 2,
        });
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
        // Only those with a way back: a link whose landing leads back to its
        // take-off over the grid's links and the links kept (each dropped one
        // can cut another's way back, so until none is). A one-way drop into
        // a pocket would keep the pocket's floors as reached
        // (`keep_reached` follows links both ways) with no planned way out:
        // on Cologne a deck's drops kept 14,684 floors of a yard.
        let before = out.len();
        loop {
            let links: Vec<(OffLink, usize)> = out.iter().map(|&l| (l, 0)).collect();
            let part = self.parts(wide, &links);
            let len = out.len();
            out.retain(|l| part[l.from as usize] == part[l.to as usize]);
            if out.len() == len {
                break;
            }
        }
        stats.one_way = before - out.len();
        stats.kept = out.len();
        out
    }

    /// Each floor's strongly connected part over the grid's links and the
    /// `found` gap links: two floors share one when each leads to the other
    /// (Tarjan's algorithm, without recursion).
    fn parts(&self, wide: &Wide, found: &[(OffLink, usize)]) -> Vec<u32> {
        let n = self.len();
        let mut off: Vec<(u32, u32)> = found.iter().map(|&(l, _)| (l.from, l.to)).collect();
        off.sort_unstable();
        let mut first = Vec::with_capacity(n + 1);
        let mut to: Vec<u32> = Vec::new();
        let mut o = 0;
        for a in 0..n {
            first.push(to.len() as u32);
            to.extend(wide.links[a].iter().copied().filter(|&b| b != NONE));
            while o < off.len() && off[o].0 as usize == a {
                to.push(off[o].1);
                o += 1;
            }
        }
        first.push(to.len() as u32);
        const UNSEEN: u32 = u32::MAX;
        let mut index = vec![UNSEEN; n];
        let mut low = vec![0u32; n];
        let mut part = vec![UNSEEN; n];
        let mut stack: Vec<u32> = Vec::new();
        let mut calls: Vec<(u32, u32)> = Vec::new();
        let (mut next, mut parts) = (0u32, 0u32);
        for root in 0..n as u32 {
            if index[root as usize] != UNSEEN {
                continue;
            }
            calls.push((root, first[root as usize]));
            index[root as usize] = next;
            low[root as usize] = next;
            next += 1;
            stack.push(root);
            while let Some(&mut (v, ref mut e)) = calls.last_mut() {
                if *e < first[v as usize + 1] {
                    let w = to[*e as usize];
                    *e += 1;
                    if index[w as usize] == UNSEEN {
                        index[w as usize] = next;
                        low[w as usize] = next;
                        next += 1;
                        stack.push(w);
                        calls.push((w, first[w as usize]));
                    } else if part[w as usize] == UNSEEN {
                        low[v as usize] = low[v as usize].min(index[w as usize]);
                    }
                    continue;
                }
                calls.pop();
                if let Some(&(u, _)) = calls.last() {
                    low[u as usize] = low[u as usize].min(low[v as usize]);
                }
                if low[v as usize] == index[v as usize] {
                    while let Some(w) = stack.pop() {
                        part[w as usize] = parts;
                        if w == v {
                            break;
                        }
                    }
                    parts += 1;
                }
            }
        }
        part
    }

    /// An off-mesh link's cost while the grid is built (no lip or edge
    /// marks yet): [`gap_cost`].
    fn off_cost(&self, link: OffLink) -> f32 {
        let (p, q) = (self.position(link.from), self.position(link.to));
        gap_cost(link.kind, (q - p).length(), p.distance_xz(q), p.y - q.y) + link.press.penalty()
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
    /// [A238] A lip whose walk-off lands 1.8 m out (2 m down), over a ledge 0.75 m past the edge that no link reaches (Vector
    /// Laboratory's 20.6 lip, its 17.80 landing and its 16.22 ledge), with a way back up from the landing: with
    /// `Body::slip_landings` the ledge is reached (a slip off the walk-off's take-off comes down on it) and a pocket, no walk
    /// returning from it; without, it is not; no link leads onto it either way, so no route plans it.
    #[test]
    fn a_ledge_only_a_slip_gets_to_is_reached_with_the_switch() {
        let walker = WalkerBehaviour::BySlope;
        let mut t: Vec<Triangle> = Vec::new();
        t.extend(quad(0.0, 0.0, 10.0, 8.0, 4.0));
        t.extend(quad(10.75, 0.0, 11.0, 4.0, 0.5));
        t.extend(quad(11.8, 0.0, 20.0, 4.0, 2.0));
        t.push(Triangle { v: [Vec3::new(20.0, 2.0, 0.0), Vec3::new(24.0, 4.0, 0.0), Vec3::new(24.0, 4.0, 4.0)], walker });
        t.push(Triangle { v: [Vec3::new(20.0, 2.0, 0.0), Vec3::new(24.0, 4.0, 4.0), Vec3::new(20.0, 2.0, 4.0)], walker });
        t.extend(quad(24.0, 0.0, 26.0, 8.0, 4.0));
        t.extend(quad(10.0, 6.0, 24.0, 8.0, 4.0));
        let start = [Vec3::new(5.0, 4.0, 2.0)];
        let build = |slip_landings: bool| {
            let body = Body { slip_landings, ..Body::default() };
            WalkGrid::build_kept_with_links(&t, [0.0, -1.0, 27.0, 9.0], 0.25, body, &start, &start, OffMeshLinks::Gaps).0
        };
        let ledge = |grid: &WalkGrid| grid.locate(Vec3::new(10.875, 0.5, 2.0), 0.3).filter(|&n| (grid.position(n).y - 0.5).abs() < 0.05);
        let off = build(false);
        assert!(off.off_links().iter().any(|l| l.press == Takeoff::WalkOff), "the lip's walk-off");
        assert!(ledge(&off).is_none_or(|n| !off.reached(n)), "without the switch the ledge is no floor reached");
        let on = build(true);
        let n = ledge(&on).expect("the ledge is kept");
        assert!(on.reached(n) && !on.returns(n) && on.in_pocket(n), "reached, no way back: a pocket");
        assert!(on.pocket_of(n).is_some());
        // Not a link: no route plans a slip.
        let ledge_floors: Vec<NodeId> = (0..on.len() as NodeId).filter(|&f| (on.position(f).y - 0.5).abs() < 0.05).collect();
        assert!(ledge_floors.iter().all(|&f| !(0..on.len() as NodeId).any(|a| !ledge_floors.contains(&a) && on.links(a).any(|(b, _)| b == f))));
        // The lip itself is as it was: still reached, still returning.
        let lip = on.locate(Vec3::new(9.875, 4.0, 2.0), 0.3).unwrap();
        assert!(on.reached(lip) && on.returns(lip));
    }

    /// [A247] A slip comes down only on a floor open above the take-off: with a shelf under the lip's own top (Vector
    /// Laboratory's floors under the 20.6 walkway and under the 17.78 plate), higher than the ledge and in the square under the
    /// edge, the slip still lands on the ledge past the edge, as a body falling past it does.
    #[test]
    fn a_slip_does_not_come_down_on_a_shelf_under_the_lip() {
        let walker = WalkerBehaviour::BySlope;
        let mut t: Vec<Triangle> = Vec::new();
        // The lip, and a shelf under it reaching to 0.05 m short of its edge, 1.5 m down.
        t.extend(quad(0.0, 0.0, 10.0, 8.0, 4.0));
        t.extend(quad(8.5, 0.0, 9.95, 4.0, 2.5));
        t.extend(quad(10.75, 0.0, 11.0, 4.0, 0.5));
        t.extend(quad(11.8, 0.0, 20.0, 4.0, 2.0));
        t.push(Triangle { v: [Vec3::new(20.0, 2.0, 0.0), Vec3::new(24.0, 4.0, 0.0), Vec3::new(24.0, 4.0, 4.0)], walker });
        t.push(Triangle { v: [Vec3::new(20.0, 2.0, 0.0), Vec3::new(24.0, 4.0, 4.0), Vec3::new(20.0, 2.0, 4.0)], walker });
        t.extend(quad(24.0, 0.0, 26.0, 8.0, 4.0));
        t.extend(quad(10.0, 6.0, 24.0, 8.0, 4.0));
        let start = [Vec3::new(5.0, 4.0, 2.0)];
        let body = Body { slip_landings: true, ..Body::default() };
        let area = [0.0, -1.0, 27.0, 9.0];
        let at = |grid: &WalkGrid, x: f32, y: f32| grid.locate(Vec3::new(x, y, 2.0), 0.3).filter(|&n| (grid.position(n).y - y).abs() < 0.05);
        // Every floor: the shelf is one, under the lip's top; the ledge is open above it.
        let all = WalkGrid::build_with_links(&t, area, 0.25, body, OffMeshLinks::Gaps);
        let shelf = at(&all, 9.875, 2.5).expect("the shelf is a floor under the lip");
        assert!(!all.open_to(shelf, 4.0), "the lip is over the shelf");
        assert!(all.open_to(at(&all, 10.875, 0.5).expect("the ledge"), 4.0), "nothing is over the ledge");
        // The reached floors: the slip lands on the ledge; the shelf, higher and in the square, is not reached, so not kept.
        let kept = WalkGrid::build_kept_with_links(&t, area, 0.25, body, &start, &start, OffMeshLinks::Gaps).0;
        let ledge = at(&kept, 10.875, 0.5).expect("the ledge is kept");
        assert!(kept.reached(ledge) && kept.in_pocket(ledge), "the slip lands on the ledge");
        assert!(at(&kept, 9.875, 2.5).is_none_or(|n| !kept.reached(n)), "and not on the shelf");
    }

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

    /// A way back up from the far block (top `b`) to the near one's (top
    /// `a`) north of the blocks: a ramp of about 30 degrees from z 10 on
    /// the far side, a walk across at `a`, and back onto the near top
    /// between x `near` and `near + 2`. The area it needs.
    fn way_back(a: f32, b: f32, near: f32) -> (Vec<Triangle>, [f32; 4]) {
        let l = 1.8 * (a - b).max(0.5);
        let walker = WalkerBehaviour::BySlope;
        let (p, q, r, t) = (
            Vec3::new(10.0, b, 10.0),
            Vec3::new(12.0, b, 10.0),
            Vec3::new(12.0, a, 10.0 + l),
            Vec3::new(10.0, a, 10.0 + l),
        );
        let mut tris = vec![Triangle { v: [p, q, r], walker }, Triangle { v: [p, r, t], walker }];
        tris.extend(quad(near, 10.0 + l, 12.0, 12.0 + l, a));
        tris.extend(quad(near, 10.0, near + 2.0, 12.0 + l, a));
        (tris, [0.0, 0.0, 14.0, 12.5 + l])
    }

    /// [`world`] with [`way_back`] (no off link is kept without one).
    fn world_back(a: f32, b: f32, gap: f32, ground: f32) -> WalkGrid {
        let mut t = world_triangles(a, b, gap, ground);
        let (back, area) = way_back(a, b, 1.0);
        t.extend(back);
        WalkGrid::build_with_links(&t, area, 0.25, Body::default(), OffMeshLinks::Gaps)
    }

    /// A drop into a yard with no way back up: no link (it would keep the
    /// yard's floors as reached with no planned way out); with a ramp back,
    /// one.
    #[test]
    fn a_drop_with_no_way_back_is_no_link() {
        let grid = world(2.0, 0.5, 1.5, -3.0, &[]);
        assert!(grid.off_links().iter().all(|l| grid.position(l.to).x < 7.5), "{:?}", grid.off_links());
        let grid = world_back(2.0, 0.5, 1.5, -3.0);
        assert!(grid.off_links().iter().any(|l| l.kind == LinkKind::GapDrop && grid.position(l.to).x > 7.5));
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

    /// Whether a link that is no sprint jump lands past `x`.
    fn walked_past(grid: &WalkGrid, x: f32) -> bool {
        grid.off_links().iter().any(|l| !matches!(l.press, Takeoff::Sprint(_)) && grid.position(l.to).x > x)
    }

    /// Wider than the walking clip carries: no walking way across (the
    /// sprint jump's, [`a_wide_gap_down_is_sprint_jumped`]).
    #[test]
    fn a_gap_wider_than_the_jump_is_not_jumped() {
        let grid = world(2.0, 2.0, 2.0, 0.0, &[]);
        assert!(!walked_past(&grid, 8.0));
    }

    /// A post over the gap, higher than the jump's head: held, no link.
    #[test]
    fn a_face_in_the_way_holds_the_jump() {
        let post = face_x(6.5, 0.0, 10.0, 0.0, 6.0);
        let grid = world(2.0, 2.0, 1.0, 0.0, &post);
        assert!(way_across(&grid, 2.0, 2.0).is_none());
    }

    /// A wall thinner than a cell and as tall as the room across level ground (a way round it far off): the
    /// clip's root runs on past the wall and over the floor beyond, but the body is held at the wall's face, so
    /// no link lands beyond it (the Cologne favela's east wall: a link through it kept bots jumping at it).
    #[test]
    fn a_thin_wall_holds_a_jump_whose_root_lands_beyond_it() {
        let area = [0.0, 0.0, 14.0, 30.0];
        // Level ground to x 5.9, a step block 0.5 m up from x 7.3, a way round at z 25-30.
        let mut open = quad(0.0, 0.0, 5.9, 30.0, 0.0).to_vec();
        open.extend(quad(7.3, 0.0, 14.0, 30.0, 0.5));
        open.extend(face_x(7.3, 0.0, 30.0, 0.0, 0.5));
        open.extend(quad(0.0, 25.0, 14.0, 30.0, 0.0));
        let mut walled = open.clone();
        walled.extend(face_x(6.4, 0.0, 25.0, 0.0, 6.0));
        walled.extend(face_x(6.74, 0.0, 25.0, 0.0, 6.0));
        let grid = WalkGrid::build_with_links(&walled, area, 0.25, Body::default(), OffMeshLinks::Gaps);
        assert!(!walked_past(&grid, 7.0), "{}", grid.off_links().len());
        // The same without the wall is jumped (else the wall's test is vacuous).
        let grid = WalkGrid::build_with_links(&open, area, 0.25, Body::default(), OffMeshLinks::Gaps);
        assert!(walked_past(&grid, 7.0), "{}", grid.off_links().len());
    }

    /// A face across z from x0 to x1, from y0 to y1.
    fn face_z(z: f32, x0: f32, x1: f32, y0: f32, y1: f32) -> [Triangle; 2] {
        let (a, b, c, d) = (Vec3::new(x0, y0, z), Vec3::new(x1, y0, z), Vec3::new(x1, y1, z), Vec3::new(x0, y1, z));
        let walker = WalkerBehaviour::BySlope;
        [Triangle { v: [a, b, c], walker }, Triangle { v: [a, c, d], walker }]
    }

    /// A bot can still jump onto a box and from its top over a wall that is too tall to jump from the ground: the wall
    /// stands 0.5 m over the clips' highest rise (the walking clip's 1.00, the sprint clip's 1.35, read from the
    /// client's animations), the box 0.1 m under the walking apex, beside the wall. No link from the ground over the
    /// wall; a way onto the box; a link from the box top landing beyond the wall.
    #[test]
    fn a_box_beside_a_tall_wall_is_jumped_onto_and_from_over_the_wall() {
        let walking = SHORT_JUMP_RISE.iter().copied().fold(0.0f32, f32::max);
        let sprint = SPRINT_JUMP_RISE.iter().flatten().copied().fold(0.0f32, f32::max);
        let (box_h, wall_h) = (walking - 0.1, sprint + 0.5);
        assert!(wall_h > walking + 0.5 && wall_h - box_h < walking, "the wall clears from the box top only");
        let area = [0.0, 0.0, 14.0, 30.0];
        // Ground; a wall 0.34 m thick across x 4.5 for z 0-25 (a way round at z 25-30); a box beside it on the west
        // at z 5-10 (x 0.5-4.2, so 0.3 m from the wall).
        let mut t = quad(0.0, 0.0, 14.0, 30.0, 0.0).to_vec();
        t.extend(face_x(4.5, 0.0, 25.0, 0.0, wall_h));
        t.extend(face_x(4.84, 0.0, 25.0, 0.0, wall_h));
        t.extend(quad(4.5, 0.0, 4.84, 25.0, wall_h));
        t.extend(quad(0.5, 5.0, 4.2, 10.0, box_h));
        t.extend(face_x(0.5, 5.0, 10.0, 0.0, box_h));
        t.extend(face_x(4.2, 5.0, 10.0, 0.0, box_h));
        t.extend(face_z(5.0, 0.5, 4.2, 0.0, box_h));
        t.extend(face_z(10.0, 0.5, 4.2, 0.0, box_h));
        let grid = WalkGrid::build_with_links(&t, area, 0.25, Body::default(), OffMeshLinks::Gaps);
        let at = |f: NodeId| grid.position(f);
        let off = grid.off_links();
        // From the ground, none lands beyond the wall.
        let over_from_ground = off.iter().filter(|l| at(l.from).y < 0.1 && at(l.to).x > 5.0 && at(l.from).x < 4.4).count();
        assert_eq!(over_from_ground, 0, "{over_from_ground} links over the wall from the ground");
        // The box top is reached: a jump link, or a gap link, from the ground to a floor at the box's height.
        let onto_box = (0..grid.len() as NodeId).filter(|&f| at(f).y < 0.1).any(|f| {
            grid.links(f).any(|(n, k)| (k == LinkKind::Jump || k.is_off_mesh()) && (at(n).y - box_h).abs() < 0.05 && at(n).x > 0.5 && at(n).x < 4.2)
        });
        assert!(onto_box, "no link onto the box");
        // From the box top, a link lands beyond the wall.
        let over_from_box = off.iter().filter(|l| (at(l.from).y - box_h).abs() < 0.05 && at(l.to).x > 5.0 && at(l.to).y < 0.1).count();
        assert!(over_from_box > 0, "no link from the box top over the wall: {:?}", off.iter().map(|l| (at(l.from), at(l.to), l.kind)).take(12).collect::<Vec<_>>());
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
        let grid = world_back(2.0, 0.5, 1.5, -3.0);
        let kinds = way_across(&grid, 2.0, 0.5).expect("a way across");
        assert!(kinds.contains(&LinkKind::GapDrop), "{kinds:?}");
        let grid = world_back(2.0, 0.5, 2.5, -3.0);
        assert!(!walked_past(&grid, 8.5));
    }

    /// A lower block 1.2 m down across a 1 m gap: walked off (no key), the
    /// body keeps its speed and comes down on it at a walk and a sprint
    /// alike: a gap drop walked off, made before any pressed one.
    #[test]
    fn a_lower_block_close_across_a_gap_is_walked_off_to() {
        let grid = world_back(2.0, 0.8, 1.0, -3.0);
        let off = grid
            .off_links()
            .iter()
            .filter(|l| l.kind == LinkKind::GapDrop && l.press == Takeoff::WalkOff)
            .count();
        assert!(off > 0, "{:?}", grid.off_links());
        let kinds = way_across(&grid, 2.0, 0.8).expect("a way across");
        assert!(kinds.contains(&LinkKind::GapDrop), "{kinds:?}");
    }

    /// The floor a body walks off an edge from is a lip, as a Drop link's is (a body that strays 0.3 m from it falls), with
    /// `Body::walk_off_lips`: the guards read the lip bits ([`WalkGrid::is_lip`], [`WalkGrid::near_lip`]); a drop shallower
    /// than a lip is none; without the switch the take-offs are no lips (the grid as it was), and a grid read back from its
    /// file is given the switch ([`WalkGrid::read`]).
    #[test]
    fn the_take_off_of_a_walk_off_is_a_lip_with_the_switch() {
        let build = |walk_off_lips: bool| {
            let mut t = world_triangles(2.0, 0.8, 1.0, -3.0);
            let (back, area) = way_back(2.0, 0.8, 1.0);
            t.extend(back);
            let body = Body { walk_off_lips, ..Body::default() };
            (WalkGrid::build_with_links(&t, area, 0.25, body, OffMeshLinks::Gaps), body)
        };
        let (grid, body) = build(true);
        let walk_offs: Vec<_> = grid.off_links().iter().filter(|l| l.press == Takeoff::WalkOff).collect();
        assert!(!walk_offs.is_empty());
        for l in &walk_offs {
            let drop = grid.position(l.from).y - grid.position(l.to).y;
            assert_eq!(grid.is_lip(l.from), drop >= LIP_DROP_M, "{l:?} drops {drop}");
            if drop >= LIP_DROP_M {
                assert!(grid.near_lip(l.from), "{l:?}");
            }
        }
        assert!(walk_offs.iter().any(|l| grid.is_lip(l.from)), "a drop of a lip's height: {walk_offs:?}");
        assert_eq!(grid.hot_mismatches(), 0);
        let path = std::env::temp_dir().join(format!("srp-walk-off-lips-{}.grid", std::process::id()));
        grid.write(&path, 11).unwrap();
        let back = WalkGrid::read(&path, 11, body).unwrap();
        assert!(walk_offs.iter().all(|l| back.is_lip(l.from) == grid.is_lip(l.from)));
        assert_eq!(back.hot_mismatches(), 0);
        let _ = std::fs::remove_file(&path);
        // Without the switch no take-off is a lip, and the key differs.
        let (plain, plain_body) = build(false);
        assert!(plain.off_links().iter().filter(|l| l.press == Takeoff::WalkOff).all(|l| !plain.is_lip(l.from)));
        assert_ne!(plain.digest(), grid.digest(), "the near-lip flags are in the file");
        let key = |body| crate::grid_key_with_links(&[], [0.0, 0.0, 1.0, 1.0], 0.25, body, &[], OffMeshLinks::Gaps);
        assert_ne!(key(body), key(plain_body));
    }

    /// Whether a way from (`x`, `y`, 5) to the far block's floor takes a
    /// sprint gap link.
    fn sprints_across(grid: &WalkGrid, x: f32, y: f32, b: f32) -> bool {
        let from = grid.locate(Vec3::new(x, y, 5.0), 0.5).expect("the near floor");
        let to = grid.locate(Vec3::new(11.0, b, 5.0), 0.5).expect("the far block's floor");
        grid.path(from, to).is_some_and(|path| {
            path.nodes
                .windows(2)
                .any(|w| grid.off_link(w[0], w[1]).is_some_and(|l| matches!(l.press, Takeoff::Sprint(_))))
        })
    }

    /// 3 m across and 1.7 m down over a deep gap (Chemical Plant's rail
    /// end into area C, the owner's sprint jump, ground 8.5 m down): past
    /// the walking jump and the walk-off, within the sprint jump at every
    /// prepare. Over ground within the harmless drop: none (the body drops
    /// in and walks on).
    #[test]
    fn a_wide_gap_down_is_sprint_jumped() {
        let grid = world_back(2.0, 0.3, 3.0, -7.0);
        assert!(!walked_past(&grid, 9.0), "no walking way: {:?}", grid.off_links());
        assert!(sprints_across(&grid, 3.0, 2.0, 0.3), "{:?}", grid.off_links());
        // Too wide even so.
        let grid = world(2.0, 0.3, 6.0, -7.0, &[]);
        assert!(grid.off_links().iter().all(|l| !matches!(l.press, Takeoff::Sprint(_))));
        // Over ground 3 m down.
        let grid = world(2.0, 0.3, 3.0, -1.0, &[]);
        assert!(grid.off_links().iter().all(|l| !matches!(l.press, Takeoff::Sprint(_))));
    }

    /// A ledge 1 m wide across the way but long beside it (a rail's top):
    /// no straight run behind the press, a run along the ledge that turns
    /// onto the line.
    #[test]
    fn a_sprint_jump_is_run_up_along_a_narrow_ledge() {
        let mut t = quad(0.0, 0.0, 14.0, 10.0, -7.0).to_vec();
        t.extend(block(5.0, 6.0, 2.0, -7.0));
        t.extend(block(9.0, 14.0, 0.3, -7.0));
        let (back, area) = way_back(2.0, 0.3, 4.0);
        t.extend(back);
        let grid = WalkGrid::build_with_links(&t, area, 0.25, Body::default(), OffMeshLinks::Gaps);
        assert!(sprints_across(&grid, 5.5, 2.0, 0.3), "{:?}", grid.off_links());
        // A ledge 1 m long beside the way as well: no run-up.
        let mut t = quad(0.0, 0.0, 14.0, 10.0, -7.0).to_vec();
        t.extend(quad(5.0, 4.5, 6.0, 5.5, 2.0));
        t.extend(block(9.0, 14.0, 0.3, -7.0));
        let grid = WalkGrid::build_with_links(&t, AREA, 0.25, Body::default(), OffMeshLinks::Gaps);
        assert!(grid.off_links().iter().all(|l| !matches!(l.press, Takeoff::Sprint(_))), "{:?}", grid.off_links());
    }

    #[test]
    fn a_sprint_press_reads_back_from_its_file_word() {
        for t in [Takeoff::WalkOff, Takeoff::Press(PressLead::from_metres(1.2)), Takeoff::Sprint(PressLead::from_metres(0.0)), Takeoff::Sprint(PressLead::from_metres(2.4))] {
            assert_eq!(Takeoff::from_centimetres(t.centimetres()), t);
            assert!(t.is_valid());
        }
        assert_eq!(Takeoff::Sprint(PressLead::from_metres(0.3)).sprint_run_m(), Some(SPRINT_RUN_UP_M));
        assert_eq!(Takeoff::WalkOff.sprint_run_m(), None);
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
        let back = WalkGrid::read(&path, 5, Body::default()).unwrap();
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
        let back = WalkGrid::read(&path, 6, Body::default()).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(back.digest(), grid.digest());
        assert!(back.off_links().is_empty());
    }
}

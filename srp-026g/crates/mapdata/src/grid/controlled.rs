//! [A313] Links decided by the client's own controller ([`Body::client_controller`]): the walk tick of the 0.26g exe
//! (`crate::controller::tick`, proven against the exe in Unicorn) walks a standing body from floor to floor over the map's
//! faces, and the grid keeps what the body does instead of what the fixed rules (step, slope, erosion) guess.
//!
//! Two passes, after the gap links are made:
//! - **Bridges:** a gap link between floors within a step of each other with one cell between (a crack in the map data no face
//!   covers, as Vostok's 0.25 m slab joint, or a cell holding only a riser, as its gully) is walked when the controller walks
//!   the body across: its take-off becomes a walk-off (a plain leg, no key), where the bots pressed a jump the body does not
//!   need and failed it.
//! - **Checks:** a walk link between floors that have a face no body walks on near them (a wall, a rim, a steep pitch within
//!   the reach of the capsule) stays only when the controller gets the body at least [`CHECK_SHARE`] of the way there from
//!   one of [`STARTS`]; where the client's body stalls from all of them (a 45 deg by-slope rim, a wall corner the grid passes),
//!   the bots' map stops offering the way. The links that fail are handed back; the build takes them away except one that is
//!   a floor's only way in from the seeds and spawns or its only way back (`WalkGrid::cut_keeping_reach`).
//!
//! [A313, fx-zones] Why the starts and the guard: fx-bench's M4 score (5a18068a, ON vs OFF) lost human coverage on Cologne:
//! one camper's 205 samples on a 39 deg mossy log (`mossmoss_wood_02`, x 15-18.8, z 40-46): the check dropped the walk links at
//! both of its ends (the body walked from a cell's centre straight to the next slides off the round log), and the ~70 floors
//! went when the floors no seed gets to were dropped. A single straight walk from the cell's centre is one line of many a body
//! takes; a bridge is added, and a walk link taken away, only on what all three lines agree on.
//!
//! The controller runs whole ticks at [`WALK_MPS`] toward the target floor for the ticks the leg takes and two more.
use super::gap::{OffLink, Takeoff};
use super::*;
use crate::controller::tick::{Controller, Settings, TickInput};
use crate::controller::{Capsule, Collision, Shape};

/// The walk speed the controller is driven at, m/s (a run's root motion; over the client's 3 m/s floor).
pub const WALK_MPS: f32 = 3.5;
/// A checked walk link stays when the body gets this share of the way.
pub const CHECK_SHARE: f32 = 0.5;
/// A floor with less room over it than a standing body is not checked (crouching is not ported), m.
pub const STANDING_ROOM_M: f32 = 1.8;
/// A face no body walks on within this of a floor's capsule makes its links checked, m.
pub const CHECK_REACH_M: f32 = 0.15;
/// The lines the controller walks a leg on: from the floor's centre, and this far either side of it across the leg, m. A
/// check fails, and a bridge is walked, only when every line does.
pub const STARTS: [f32; 3] = [0.0, -0.1, 0.1];
/// The body is at the target when its feet are this near it flat and this near it in height, m.
const AT_FLAT_M: f32 = 0.15;
const AT_HEIGHT_M: f32 = 0.2;
/// A walked bridge is kept only in a run of at least this many walked bridges side by side, each one cell over across the
/// bridge's way (0.75 m at the 0.25 m cell: the body's 0.76 m width). [A313, fx-zones] Cologne day 2 (x 9.7-10, z 32.1-32.6):
/// two bridges up a 0.6 m riser beside a leaning board, where the walks were dropped; bots cut the corner to them one column
/// off and stalled at the board (bench3: 14 presses that freed nothing there, 1 without the pass).
pub const BRIDGE_MIN_RUN: usize = 3;
/// [A364 C2 row 52] The widest level gap a walked bridge is tried over, m: the client walks over a gap between two level floors up
/// to 1.79 m wide at a 3.5 m/s walk (the feet dip 0.15-0.26 m and the step takes the far side; measured with the exact tick,
/// exactness rig `grid-rules` S9, `out/gapwalk.tsv`; crouched 1.34, sprinting 2.6). Each bridge is still walked by the controller
/// before it is kept ([`walk_lines`]); this only bounds the search.
pub const BRIDGE_GAP_M: f32 = 1.79;
/// [A364 C2 row 52] The most cells of no floor a bridge is tried across: [`BRIDGE_GAP_M`] over the cell (0.25 m), along a side.
pub const BRIDGE_MAX_CELLS: usize = 7;

/// What the controller pass did ([`BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlledStats {
    pub bridges_tried: usize,
    pub bridges_walked: usize,
    pub bridges_added: usize,
    /// Bridges the controller walks in a run narrower than [`BRIDGE_MIN_RUN`]: not walked (a gap link stays pressed).
    pub bridges_narrow: usize,
    pub checks: usize,
    pub checks_dropped: usize,
    /// Failed checks kept as a floor's only way in or back (`WalkGrid::cut_keeping_reach`).
    pub checks_kept: usize,
    /// [2c] Of the checks, those walked crouched ([`Body::walk_every_link`]).
    pub checks_crouched: usize,
    /// [2c] Of the failed checks, those the body walks from the floor but not on from the link before ([`Body::walk_every_link`]).
    pub checks_chain_failed: usize,
    /// [2c] Of the failed checks, those walked crouched.
    pub checks_failed_crouched: usize,
    /// [2c] Risers cut by the walk but kept, marked, for no walking way round them ([`ROUND_HOPS`]).
    pub checks_no_way_round: usize,
}

/// [2c] The risers [`WalkGrid::walk_every_link`] keeps for no walking way round them (floor, floor).
type Held = Vec<(NodeId, NodeId)>;
/// [`Body::walk_ceiling_stalls`] The Walk links the walk check passes only crouched, as (from, to).
type Crouched = Vec<(u32, u32)>;

/// [2c] One walk check's answer ([`WalkGrid::walk_link`]).
#[derive(Clone, Copy, Debug, Default)]
struct LinkWalk {
    passes: bool,
    /// The best share of the way the body got from the floor.
    share: f32,
    /// It fails only on from the link before.
    chain_failed: bool,
    /// [`Body::walk_ceiling_stalls`]: how many of the lines the client's tick refused under a low ceiling (crouched too).
    refused: usize,
    /// [`Body::walk_ceiling_stalls`]: how many of the passing lines arrived only crouched (the standing body refused under a low
    /// ceiling, walked again crouched).
    crouched: usize,
}

/// [2c] A face no body walks on within this of a floor's capsule makes its links walked ([`Body::walk_every_link`]), m: the
/// client's move test holds a body 0.42 m off a wall (the cylinder and its margin, Unicorn), 0.04 m past the capsule's 0.38.
pub const CHECK_WIDE_REACH_M: f32 = 0.42 - 0.38 + 0.15;
/// [2c] A Walk link that rises or falls this much or more is walked ([`Body::walk_every_link`]), m.
pub const CHECK_RISE_M: f32 = 0.15;

/// Where a walk starts a standing body on a floor ([`Body::walk_start_clear`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalkStart {
    /// [`out_of_walls`]: out of the faces above the step, by at most [`OUT_OF_WALLS_M`].
    AboveStep,
    /// [`out_of_faces`]: out of every steep face the standing capsule overlaps, by at most the capsule's radius.
    Clear,
}

impl WalkStart {
    fn place(self, world: &Collision, feet: Vec3) -> Vec3 {
        match self {
            WalkStart::AboveStep => out_of_walls(world, feet),
            WalkStart::Clear => out_of_faces(world, feet),
        }
    }
}

/// [`walk_share`] on each of [`STARTS`]: the best share, and whether every line ended at `to`.
fn walk_lines(world: &Collision, s: &Settings, start: WalkStart, from: Vec3, to: Vec3) -> (f32, bool) {
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let l = flat.length();
    let side = if l > 1e-4 { Vec3::new(-flat.z / l, 0.0, flat.x / l) } else { Vec3::ZERO };
    let (mut best, mut all) = (0.0f32, true);
    for off in STARTS {
        let (share, at) = walk_share(world, s, start, from + side * off, to + side * off);
        best = best.max(share);
        all &= at;
    }
    (best, all)
}

/// A floor's feet moved flat out of the faces the client's body would overlap there: the grid keeps floors 0.3175 m from a wall,
/// the body's move test holds 0.42 m (the cylinder and its margin, over the step), and the client never stands inside a wall (its
/// world pushes a body out: `bullet_physics_world::recover_from_penetrations`). At most [`OUT_OF_WALLS_M`] in all.
pub(super) fn out_of_walls(world: &Collision, feet: Vec3) -> Vec3 {
    let mut at = feet;
    let mut moved = 0.0f32;
    for _ in 0..4 {
        let cyl = Shape::Cylinder { centre: at + Vec3::new(0.0, 1.3, 0.0), half: 0.5, radius: 0.38, margin: 0.04 };
        let Some((gap, _, dir)) = world.nearest_to(cyl, 0.0, |f| f.normal.y.abs() < 0.5) else { break };
        let flat = Vec3::new(dir.x, 0.0, dir.z);
        let l = flat.length();
        if gap >= 0.0 || l < 1e-4 {
            break;
        }
        let push = (-gap + 0.002).min(OUT_OF_WALLS_M - moved);
        if push <= 0.0 {
            break;
        }
        at = at + flat * (push / l);
        moved += push;
    }
    at
}

thread_local! {
    /// [A334] [`WalkGrid::pull_walk_fails`]'s answers by (the grid's number, from, to): a pure function of that grid's floors,
    /// walker and body, so caching changes no answer. Cleared at [`PULL_WALKS_MAX`] entries.
    static PULL_WALKS: std::cell::RefCell<std::collections::HashMap<(u64, NodeId, NodeId), bool>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// [A334] This thread's budget of walk ticks for [`WalkGrid::pull_walk_fails`]: set by the bots' poll each tick
/// ([`set_pull_walk_budget`]), so the worst tick's walk work is bounded (a long route's pull back ran hundreds of walks of
/// up to 68 ticks each in one poll: 13 s). Empty until first set: a walk outside a poll is not run.
#[derive(Clone, Copy, Default)]
struct PullWalkBudget {
    /// Ticks still to spend this period.
    left: u32,
    /// Ticks charged this period.
    spent: u32,
    /// The most ticks one period charged since [`pull_walk_ticks_max_take`] last read it.
    most: u32,
}

thread_local! {
    static PULL_WALK_BUDGET: std::cell::Cell<PullWalkBudget> = const {
        std::cell::Cell::new(PullWalkBudget { left: 0, spent: 0, most: 0 })
    };
}

/// [A334] Starts a budget period of `ticks` walk ticks on this thread (the bots' poll calls it once a poll).
pub fn set_pull_walk_budget(ticks: u32) {
    PULL_WALK_BUDGET.with(|b| {
        let old = b.get();
        b.set(PullWalkBudget { left: ticks, spent: 0, most: old.most.max(old.spent) });
    });
}

/// [A334] The walk ticks still left in this thread's period.
pub fn pull_walk_budget_left() -> u32 {
    PULL_WALK_BUDGET.with(|b| b.get().left)
}

/// [A334] The most walk ticks one period charged on this thread since the last call (the bench's per-tick line).
pub fn pull_walk_ticks_max_take() -> u32 {
    PULL_WALK_BUDGET.with(|b| {
        let mut now = b.get();
        let most = now.most.max(now.spent);
        now.most = 0;
        b.set(now);
        most
    })
}

/// [A334] Takes a walk of up to `ticks` from this thread's budget: a walk starts while any of it is left, so a period runs
/// past it by one walk at most (the caller bounds a walk's length). False: the leg is not walked now.
fn take_pull_walk(ticks: u32) -> bool {
    PULL_WALK_BUDGET.with(|b| {
        let mut now = b.get();
        if now.left == 0 {
            return false;
        }
        now.left = now.left.saturating_sub(ticks);
        now.spent = now.spent.saturating_add(ticks);
        b.set(now);
        true
    })
}

/// How many answers [`PULL_WALKS`] keeps before it starts over.
const PULL_WALKS_MAX: usize = 1 << 20;

/// How far [`out_of_walls`] moves a floor's feet at most, m (the 0.42 m hold less the grid's 0.3175 m, and a little).
pub const OUT_OF_WALLS_M: f32 = 0.15;

/// The client's standing capsule: 1.8 m tall, radius 0.38 (`Controller::standing_at`, startup.cfg's stand size 0.76 x 1.8).
const STAND_HEIGHT_M: f32 = 1.8;
const STAND_RADIUS_M: f32 = 0.38;

/// A floor's feet moved flat out of every steep face (|normal y| < 0.5) the client's standing capsule overlaps there, from the
/// feet up, by at most the capsule's radius: where a standing body can be. The client's world pushes a body out of whatever it
/// overlaps (`bullet_physics_world::recover_from_penetrations` 0x763D0), so it never stands inside a ledge lower than the step,
/// which [`out_of_walls`] does not look at (its cylinder starts at the step). [A329]
pub(super) fn out_of_faces(world: &Collision, feet: Vec3) -> Vec3 {
    out_of_faces_of(world, feet, STAND_HEIGHT_M)
}

/// [2c] The client's crouched capsule's height (`Controller::standing_at`'s crouching size, startup.cfg 0.76 x 1.1).
const CROUCH_HEIGHT_M: f32 = 1.1;

/// [`out_of_faces`] for a capsule `height` tall (a crouched body's is [`CROUCH_HEIGHT_M`]: a low roof's steep faces over it
/// are none of its business).
fn out_of_faces_of(world: &Collision, feet: Vec3, height: f32) -> Vec3 {
    let mut at = feet;
    let mut moved = 0.0f32;
    for _ in 0..6 {
        // A hair over the feet, so the floor's own edge is not a face the capsule overlaps.
        let cap = Capsule::standing(at + Vec3::new(0.0, 0.02, 0.0), height, STAND_RADIUS_M);
        let Some((gap, _, dir)) = world.nearest_to(Shape::Capsule(cap), 0.0, |f| f.normal.y.abs() < 0.5) else { break };
        let flat = Vec3::new(dir.x, 0.0, dir.z);
        let l = flat.length();
        if gap >= 0.0 || l < 1e-4 {
            break;
        }
        let push = (-gap + 0.002).min(STAND_RADIUS_M - moved);
        if push <= 0.0 {
            break;
        }
        at = at + flat * (push / l);
        moved += push;
    }
    at
}

/// How far the controller takes a standing body from `from` toward `to` (feet), as a share of the flat way, and whether it
/// ended at `to`.
pub fn walk_share(world: &Collision, s: &Settings, start: WalkStart, from: Vec3, to: Vec3) -> (f32, bool) {
    let mut c = Controller::standing_at(start.place(world, from));
    match walk_on(world, s, &mut c, from, to, 0, AT_HEIGHT_M) {
        // A low ceiling: the client slides under it or crouches: not a stall the pass knows.
        Walked::Ceiling => (1.0, false),
        Walked::At => (1.0, true),
        Walked::AtEnd(share) => (share, true),
        Walked::Short(share) => (share, false),
    }
}

/// [2c] How a walk of one leg ended ([`walk_on`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Walked {
    /// At the leg's end, met at a tick.
    At,
    /// At the leg's end when its ticks ran out, this share of the flat way along ([`walk_share`]'s old answer keeps the
    /// share, which a leg shorter than the arrival radius can end under half of).
    AtEnd(f32),
    /// Short of it, this share of the flat way along.
    Short(f32),
    /// Refused under a low ceiling (a standing body that cannot straighten).
    Ceiling,
}

/// [2c] The stance a walk check walks a link in: standing, or crouched where either floor has no standing room (the bots'
/// routes crouch over such floors, `bots::path`'s low legs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stance {
    Standing,
    Crouched,
}

/// [2c] The client's 0.26g body at `feet` in `stance`, set where a body in that stance can be ([`WalkStart`], out of the
/// faces its capsule overlaps).
pub fn body_at(world: &Collision, start: WalkStart, stance: Stance, feet: Vec3) -> Controller {
    match stance {
        Stance::Standing => Controller::standing_at(start.place(world, feet)),
        Stance::Crouched => {
            let at = match start {
                WalkStart::AboveStep => out_of_walls(world, feet),
                WalkStart::Clear => out_of_faces_of(world, feet, CROUCH_HEIGHT_M),
            };
            // The crouch key held, as the game's input has it before every step (`Controller::crouch_key`, A364).
            let mut c = Controller::standing_at(at);
            c.crouch_key(true);
            c
        }
    }
}

/// [2c] A failing Walk link is cut only when it rises more than this, m, and its walk got under [`CUT_SHARE`] of the way:
/// a riser the body stalls under. The exactness session's walk of 6,701 of v3's cuts on the game's shapes world
/// (FINDINGS-FEED-exactness 2026-11-12T05:24Z): that class is crossed in 6 % (12 % walked on past the end); the classes whose
/// walk went the full way but did not arrive are crossed a little further on in 53-73 %. The bench split the same way
/// (`nav-builder-20261112/bench-2c-exp-risers.txt` against `bench-2c-v4.txt`): with every class cut, falls free +5.5 % (z 6.6)
/// and hurt +29 %; with the risers alone, falls as the tip's (hurt 92 -> 90) and routes stuck -7 % (z -3.7), stuck again
/// -36 % (z -5.9).
pub const CUT_RISE_M: f32 = 0.2;
/// [2c] See [`CUT_RISE_M`].
pub const CUT_SHARE: f32 = 0.5;
/// [2c] How far a walking way round a cut riser is looked for, links (cells of 0.25 m: about 20 m of a straight way). School's
/// batteries in the v5 bench: two risers cut with no walking way round (x -17 z 71, x 66 z 55), searches failed 13 -> 465.
pub const ROUND_HOPS: usize = 80;

/// [2c] The walk check's spare ticks ([`walk_on`]): [`Body::walk_every_link`] asks for arrival, which a body slowed by a
/// wall's slide reaches later; [`walk_share`] has none (half the way needs none). A body held where it is ends a walk at
/// once, so spare ticks cost only a body still moving.
pub const SPARE_TICKS: usize = 8;

/// The controller `c` walked from where it is toward `to` (feet) at [`WALK_MPS`] for the ticks the leg from `from` takes, two
/// more and `spare` more: how it ended, the share counted along `from` -> `to`. At `to` is within [`AT_FLAT_M`] across and
/// from [`AT_HEIGHT_M`] under its height to `above` over it. `c` is left where the walk left it, so a next leg goes on from
/// there.
pub fn walk_on(world: &Collision, s: &Settings, c: &mut Controller, from: Vec3, to: Vec3, spare: usize, above: f32) -> Walked {
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let length = flat.length();
    if length < 1e-4 {
        return Walked::At;
    }
    let dir = flat * (1.0 / length);
    let step = WALK_MPS * 0.05;
    let ticks = (length / step).ceil() as usize + 2 + spare;
    let mut best = 0.0f32;
    for _ in 0..ticks {
        let feet = c.feet();
        let left = Vec3::new(to.x - feet.x, 0.0, to.z - feet.z);
        if left.length() <= AT_FLAT_M && feet.y - to.y >= -AT_HEIGHT_M && feet.y - to.y <= above {
            return Walked::At;
        }
        let along = Vec3::new(feet.x - from.x, 0.0, feet.z - from.z).dot(dir) / length;
        best = best.max(along);
        // Toward the target, at most what is left.
        let go = left.length().min(step);
        c.walk = if left.length() > 1e-6 { left * (go / left.length()) } else { Vec3::ZERO };
        let before = *c;
        if c.step(world, 0.05, s).ceiling {
            return Walked::Ceiling;
        }
        // A step is a function of the controller alone (the world and the settings are fixed), and the next walk of the
        // feet alone: a step that changed nothing is a body held where it is for every tick left, so they change no
        // answer (CPU: a stall at a wall or a riser would walk out all the leg's ticks).
        if *c == before {
            break;
        }
    }
    let feet = c.feet();
    let left = Vec3::new(to.x - feet.x, 0.0, to.z - feet.z);
    let share = best.max(Vec3::new(feet.x - from.x, 0.0, feet.z - from.z).dot(dir) / length);
    if left.length() <= AT_FLAT_M && feet.y - to.y >= -AT_HEIGHT_M && feet.y - to.y <= above {
        Walked::AtEnd(share)
    } else if spare > 0 && walks_on_to_level(world, s, c, to, dir) {
        Walked::At
    } else {
        Walked::Short(share)
    }
}

/// [2c] How far past a link's end a walk check walks on ([`walks_on_to_level`]), m.
const ON_PAST_M: f32 = 1.5;

/// [2c] Whether a body that did not arrive at `to` comes onto its floor's level when walked on toward a point [`ON_PAST_M`]
/// past it along the link's way `dir` (its ticks and [`SPARE_TICKS`] more): its feet within [`AT_HEIGHT_M`] of `to`'s height
/// and no more than [`AT_FLAT_M`] short of `to` along the way. A body walking a route walks on, so the client crosses such a
/// link a little further on. The exactness session's walk of 6,701 of v3's cut links on the game's shapes world
/// (FINDINGS-FEED-exactness 2026-11-12T05:24Z, `private/grid/rig/cut-walk/`): the classes whose walk went the full way but
/// did not arrive come onto the level so in 53-73 % (flat 66 %, rise 58 %, down 63 %), while the links that stall under a riser
/// do in 12 % (the body stays under it: those cuts stand). The same recipe as that walk's.
fn walks_on_to_level(world: &Collision, s: &Settings, c: &mut Controller, to: Vec3, dir: Vec3) -> bool {
    let aim = Vec3::new(to.x + dir.x * ON_PAST_M, to.y, to.z + dir.z * ON_PAST_M);
    let step = WALK_MPS * 0.05;
    let ticks = (ON_PAST_M / step).ceil() as usize + SPARE_TICKS;
    let level_at = |f: Vec3| (f.y - to.y).abs() <= AT_HEIGHT_M && Vec3::new(f.x - to.x, 0.0, f.z - to.z).dot(dir) >= -AT_FLAT_M;
    for _ in 0..ticks {
        if level_at(c.feet()) {
            return true;
        }
        let feet = c.feet();
        let left = Vec3::new(aim.x - feet.x, 0.0, aim.z - feet.z);
        let go = left.length().min(step);
        c.walk = if left.length() > 1e-6 { left * (go / left.length()) } else { Vec3::ZERO };
        c.step(world, 0.05, s);
    }
    level_at(c.feet())
}

/// [A318] One tick of the client's walk for a standing body at `feet` asked to move `by` (flat): the feet it starts from
/// ([`WalkStart`]) and where they end ([`WalkGrid::client_step`]).
pub fn client_step(world: &Collision, s: &Settings, start: WalkStart, feet: Vec3, by: Vec3) -> (Vec3, Vec3) {
    let from = start.place(world, feet);
    let mut c = Controller::standing_at(from);
    c.walk = Vec3::new(by.x, 0.0, by.z);
    c.step(world, 0.05, s);
    (from, c.feet())
}

/// Runs `job` over `items` on the machine's threads, in order.
pub(super) fn parallel<T: Sync, R: Send + Default + Clone>(items: &[T], job: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let chunk = items.len().div_ceil(threads).max(1);
    let mut out = vec![R::default(); items.len()];
    std::thread::scope(|scope| {
        for (part, slot) in items.chunks(chunk).zip(out.chunks_mut(chunk)) {
            let job = &job;
            scope.spawn(move || {
                for (item, r) in part.iter().zip(slot.iter_mut()) {
                    *r = job(item);
                }
            });
        }
    });
    out
}

impl WalkGrid {
    /// [A318] Keeps the map's walker triangles as the client's controller meets them (its body's [`Body::controller`] caster), for
    /// [`Self::client_step`].
    pub fn with_client_walker(mut self, triangles: &[Triangle]) -> Self {
        let (caster, _) = self.body.controller();
        self.walker = Some(std::sync::Arc::new(crate::controller::Collision::build(triangles).with_caster(caster)));
        self
    }

    /// [A334] The grid with the routes' pulled legs walked by the client's walker ([`Self::pull_walk_fails`]); it needs the
    /// walker ([`Self::with_client_walker`]), and without it nothing fails.
    pub fn with_pull_walk(mut self) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        self.pull_walk = Some(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        self
    }

    /// [A334] Whether [`Self::with_pull_walk`] is set.
    pub fn pull_walk(&self) -> bool {
        self.pull_walk.is_some()
    }

    /// [A334] Whether a route's straight leg from floor `a` to floor `b` is one the client's walk tick does not walk: the
    /// controller pass's walk test ([`walk_share`]: a standing body from `a`'s centre at the walk speed, with the exe's
    /// collision rules) with the grid's walker. False without [`Self::with_pull_walk`] or the walker.
    pub fn pull_walk_fails(&self, a: NodeId, b: NodeId) -> bool {
        let (Some(id), Some(world)) = (self.pull_walk, self.walker.as_deref()) else { return false };
        // The answer is the floors' own (the walker and the body are the grid's): kept per thread by the grid's number and the
        // floors, so a leg pulled again (most are: the same ways are planned again and again) costs a look-up.
        let key = (id, a, b);
        if let Some(known) = PULL_WALKS.with(|c| c.borrow().get(&key).copied()) {
            return known;
        }
        // The walk's ticks at most (as [`walk_share`] counts them), taken from this thread's budget up front: past the budget
        // the leg is not walked now (it walks, as with the switch off) and nothing is kept, so a later pull walks it.
        let (pa, pb) = (self.position(a), self.position(b));
        let ticks = (Vec3::new(pb.x - pa.x, 0.0, pb.z - pa.z).length() / (WALK_MPS * 0.05)).ceil() as u32 + 2;
        if !take_pull_walk(ticks) {
            return false;
        }
        let (_, s) = self.body.controller();
        let fails = !walk_share(world, &s, self.body.walk_start(), pa, pb).1;
        PULL_WALKS.with(|c| {
            let mut c = c.borrow_mut();
            if c.len() >= PULL_WALKS_MAX {
                c.clear();
            }
            c.insert(key, fails);
        });
        fails
    }

    /// [A318] The heap [`Self::with_client_walker`] keeps, bytes (0 without it).
    pub fn client_walker_bytes(&self) -> usize {
        self.walker.as_deref().map_or(0, crate::controller::Collision::bytes)
    }

    /// [A318] One tick of the client's walk (`player_step`, the port `controller::tick::Controller::step`: `step_up` 0x7C900, the
    /// move sweep, `step_down` 0x7E1B0) for a standing body at `feet` asked to move `by` (flat, this tick's displacement): where its
    /// feet end, from where the walk starts it ([`Body::walk_start`]). None without [`Self::with_client_walker`].
    pub fn client_step(&self, feet: Vec3, by: Vec3) -> Option<(Vec3, Vec3)> {
        let world = self.walker.as_deref()?;
        let (_, s) = self.body.controller();
        Some(client_step(world, &s, self.body.walk_start(), feet, by))
    }

    /// [A364 P9] A body the reckoning keeps across ticks ([`Controller::tick`]) starts here: a standing controller at `feet`, placed
    /// as the walk starts ([`Body::walk_start`]). None without [`Self::with_client_walker`].
    pub fn kept_controller_at(&self, feet: Vec3) -> Option<Controller> {
        let world = self.walker.as_deref()?;
        Some(Controller::standing_at(self.body.walk_start().place(world, feet)))
    }

    /// A body the reckoning keeps across ticks starts here at a world that saved its controller (`serialize` 0x7F7F0): the
    /// client's own, crouched where the client's is ([`Controller::deserialized`]), not a standing body placed as the walk starts.
    /// None without [`Self::with_client_walker`].
    pub fn kept_controller_saved(&self, saved: &crate::controller::tick::Saved) -> Option<Controller> {
        self.walker.as_deref()?;
        Some(Controller::deserialized(saved))
    }

    /// [A364 P9] One tick of a kept body (`player_step` with what the game's logic hands it: `Controller::tick`) on the grid's
    /// walker with its body's settings; false (and nothing moved) without [`Self::with_client_walker`].
    pub fn kept_tick(&self, body: &mut Controller, input: TickInput) -> bool {
        let Some(world) = self.walker.as_deref() else { return false };
        let (_, s) = self.body.controller();
        body.tick(world, 0.05, &s, input);
        true
    }

    /// [A366] The walkable top of the client's walker (the map's walker triangles) under `at` (flat) from `from_y` down to `reach`
    /// below it: a ray straight down, the first face met whose normal stands within the body's walkable slope
    /// ([`Body::min_ground_normal_y`]). None without [`Self::with_client_walker`] or with no such face: the grid's floors are all
    /// there is to read then.
    pub fn walker_ground_below(&self, at: Vec3, from_y: f32, reach: f32) -> Option<f32> {
        let world = self.walker.as_deref()?;
        let a = Vec3::new(at.x, from_y, at.z);
        let b = Vec3::new(at.x, from_y - reach, at.z);
        let (share, normal) = world.ray(a, b)?;
        (normal.y >= self.body.min_ground_normal_y).then_some(from_y - share * reach)
    }

    /// The controller pass ([`Body::client_controller`], module documentation).
    /// The bridges are made here; the walk links that fail their check are returned (a floor, a direction and the share the body
    /// got), for the build to take away with `cut_keeping_reach`.
    pub(super) fn controlled_links(&self, wide: &mut Wide, triangles: &[Triangle]) -> (ControlledStats, Vec<(u32, usize, f32)>) {
        let body = self.body;
        let (caster, s) = body.controller();
        let start = body.walk_start();
        let world = Collision::build(triangles).with_caster(caster);
        let s = &s;
        let mut stats = ControlledStats::default();
        let cells_apart = |a: usize, b: usize| {
            let (ia, ka) = self.cell_xz(self.cell_of[a] as usize);
            let (ib, kb) = self.cell_xz(self.cell_of[b] as usize);
            (ia - ib).abs().max((ka - kb).abs())
        };
        // Bridges.
        let bridges: Vec<usize> = wide
            .off
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                l.kind == LinkKind::GapJump
                    && !matches!(l.press, Takeoff::WalkOff)
                    && (self.height[l.from as usize] - self.height[l.to as usize]).abs() <= body.step
                    && (2..=BRIDGE_MAX_CELLS as i32 + 1).contains(&cells_apart(l.from as usize, l.to as usize))
            })
            .map(|(i, _)| i)
            .collect();
        stats.bridges_tried = bridges.len();
        let walked = parallel(&bridges, |&i| {
            let l: &OffLink = &wide.off[i];
            walk_lines(&world, s, start, self.position(l.from), self.position(l.to)).1
        });
        let old: Vec<usize> = bridges.iter().zip(walked).filter(|&(_, ok)| ok).map(|(&i, _)| i).collect();
        // Bridges with no link yet: two cells on, a floor within a step, the cell between with no floor near that height (a
        // crack no face covers, a cell holding only a riser), walked by the controller: a walked off-mesh link.
        wide.off.sort_by_key(|l| (l.from, l.to));
        let mut new: Vec<(u32, u32)> = Vec::new();
        // The new bridges over a crack the grid crossed by a drop ([`Body::crack_bridges`]): kept whatever their run's width (the
        // drop they stand beside was as narrow a way).
        let mut over_drop: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
        for a in 0..self.height.len() {
            let (i, k) = self.cell_xz(self.cell_of[a] as usize);
            let ha = self.height[a];
            for (d, &(di, dk)) in DIRS.iter().enumerate() {
                // A side with a link is tried only when it is a drop into the cells between ([A329] a crack: the drop is the way
                // the grid had across it, and the client walks over it; `drop_check` takes such a drop away when the body never
                // drops there).
                let link = wide.links[a][d];
                let drop_at = (link != NONE && wide.kinds[a][d] == LinkKind::Drop).then(|| self.cell_of[link as usize] as usize);
                if link != NONE && !(body.crack_bridges && drop_at.is_some()) {
                    continue;
                }
                // [A364 C2 row 52] Across 1 to [`BRIDGE_MAX_CELLS`] cells with no floor within a step of the take-off's (a level
                // gap the client walks over), to the first cell beyond with one.
                let stride = self.cell * ((di * di + dk * dk) as f32).sqrt();
                for g in 1..=BRIDGE_MAX_CELLS as i32 {
                    let Some(cg) = self.cell_index(i + g * di, k + g * dk) else { break };
                    if (self.start[cg]..self.start[cg + 1]).any(|m| (self.height[m as usize] - ha).abs() <= body.step) {
                        break;
                    }
                    let Some(c3) = self.cell_index(i + (g + 1) * di, k + (g + 1) * dk) else { break };
                    // The gap's span between the two floors' cells, at most: `g` cells of no floor.
                    if g as f32 * stride > BRIDGE_GAP_M + 1e-4 {
                        break;
                    }
                    let into_crack = drop_at.is_some_and(|c| (1..=g).any(|j| self.cell_index(i + j * di, k + j * dk) == Some(c)));
                    if link != NONE && !into_crack {
                        continue;
                    }
                    let mut found = false;
                    for b in self.start[c3]..self.start[c3 + 1] {
                        if (self.height[b as usize] - ha).abs() <= body.step && wide.off.binary_search_by_key(&(a as u32, b), |l| (l.from, l.to)).is_err() {
                            new.push((a as u32, b));
                            found = true;
                            if into_crack {
                                over_drop.insert((a as u32, b));
                            }
                        }
                    }
                    if found || (self.start[c3]..self.start[c3 + 1]).any(|m| (self.height[m as usize] - ha).abs() <= body.step) {
                        break;
                    }
                }
            }
        }
        stats.bridges_tried += new.len();
        let walked = parallel(&new, |&(a, b)| walk_lines(&world, s, start, self.position(a), self.position(b)).1);
        let new: Vec<(u32, u32)> = new.iter().zip(walked).filter(|&(_, ok)| ok).map(|(&p, _)| p).collect();
        // Only the bridges in a run at least [`BRIDGE_MIN_RUN`] wide are walked: a narrower one is crossed only by a body
        // inside its columns, which a bot steering at a point ahead of it does not keep to.
        let all: std::collections::HashSet<(u32, u32)> =
            old.iter().map(|&i| (wide.off[i].from, wide.off[i].to)).chain(new.iter().copied()).collect();
        for &i in &old {
            if self.bridge_run(&all, wide.off[i].from, wide.off[i].to) >= BRIDGE_MIN_RUN {
                wide.off[i].press = Takeoff::WalkOff;
                stats.bridges_walked += 1;
            } else {
                stats.bridges_narrow += 1;
            }
        }
        for &(a, b) in &new {
            if over_drop.contains(&(a, b)) || self.bridge_run(&all, a, b) >= BRIDGE_MIN_RUN {
                wide.off.push(OffLink { from: a, to: b, kind: LinkKind::GapJump, press: Takeoff::WalkOff });
                stats.bridges_walked += 1;
                stats.bridges_added += 1;
            } else {
                stats.bridges_narrow += 1;
            }
        }
        wide.off.sort_by_key(|l| (l.from, l.to));
        if body.walk_every_link {
            let (stats, failing, held, mut crouch) = self.walk_every_link(wide, &world, s, start, stats);
            // The risers with no walking way round stay, marked (the caller marks the cut's kept ones beside them).
            wide.marked = held;
            crouch.sort_unstable();
            wide.crouch = crouch;
            return (stats, failing);
        }
        // Checks: the floors with a face no body walks on near their capsule.
        let n = self.height.len();
        let floors: Vec<usize> = (0..n).collect();
        let near = parallel(&floors, |&a| {
            let feet = self.position(a as NodeId);
            let cap = Capsule::standing(feet, 1.8, 0.38);
            world
                .nearest(cap, CHECK_REACH_M, |f| f.walker == WalkerBehaviour::DontWalk || f.normal.y.abs() < body.min_ground_normal_y)
                .is_some()
        });
        let mut checks: Vec<(usize, usize)> = Vec::new();
        for a in 0..n {
            for d in 0..8 {
                let b = wide.links[a][d];
                let room = |n: usize| self.ceiling[n] - self.height[n] >= STANDING_ROOM_M;
                if b != NONE && wide.kinds[a][d] == LinkKind::Walk && (near[a] || near[b as usize]) && room(a) && room(b as usize) {
                    checks.push((a, d));
                }
            }
        }
        stats.checks = checks.len();
        let shares = parallel(&checks, |&(a, d)| {
            let b = wide.links[a][d];
            walk_lines(&world, s, start, self.position(a as NodeId), self.position(b)).0
        });
        let failing = checks
            .iter()
            .zip(shares)
            .filter(|&(_, share)| share < CHECK_SHARE)
            .map(|(&(a, d), share)| (a as u32, d, share))
            .collect();
        (stats, failing)
    }

    /// [2c] The checks of [`Body::walk_every_link`] (module documentation): the Walk links a body may meet trouble on
    /// ([`Self::walk_checked`]), each walked by [`Self::walk_link`]; the failing ones handed back as the old checks are.
    fn walk_every_link(
        &self,
        wide: &Wide,
        world: &Collision,
        s: &Settings,
        start: WalkStart,
        mut stats: ControlledStats,
    ) -> (ControlledStats, Vec<(u32, usize, f32)>, Held, Crouched) {
        let n = self.height.len();
        let floors: Vec<usize> = (0..n).collect();
        let near = parallel(&floors, |&a| {
            let feet = self.position(a as NodeId);
            let height = if self.ceiling[a] - self.height[a] >= STANDING_ROOM_M { STAND_HEIGHT_M } else { CROUCH_HEIGHT_M };
            let cap = Capsule::standing(feet, height, STAND_RADIUS_M);
            world
                .nearest(cap, CHECK_WIDE_REACH_M, |f| f.walker == WalkerBehaviour::DontWalk || f.normal.y.abs() < s.idle_max_slope_cos)
                .is_some()
        });
        let mut checks: Vec<(usize, usize)> = Vec::new();
        for a in 0..n {
            for d in 0..8 {
                let b = wide.links[a][d];
                if b != NONE && wide.kinds[a][d] == LinkKind::Walk && self.walk_checked(a, b as usize, &near) {
                    checks.push((a, d));
                }
            }
        }
        stats.checks = checks.len();
        stats.checks_crouched = checks.iter().filter(|&&(a, d)| self.stance(a, wide.links[a][d] as usize) == Stance::Crouched).count();
        let walked = parallel(&checks, |&(a, d)| self.walk_link(wide, world, s, start, a, d));
        // [`Body::walk_ceiling_stalls`]: the passing links two of whose lines arrived only crouched: a route crosses them crouched.
        let crouch: Vec<(u32, u32)> = if self.body.walk_ceiling_stalls {
            checks
                .iter()
                .zip(&walked)
                .filter(|(_, w)| w.passes && w.crouched >= 2)
                .map(|(&(a, d), _)| (a as u32, wide.links[a][d]))
                .collect()
        } else {
            Vec::new()
        };
        stats.checks_chain_failed = walked.iter().filter(|w| w.chain_failed).count();
        stats.checks_failed_crouched = checks
            .iter()
            .zip(&walked)
            .filter(|&(&(a, d), w)| !w.passes && self.stance(a, wide.links[a][d] as usize) == Stance::Crouched)
            .count();
        let failing = checks
            .iter()
            .zip(walked)
            .filter(|(_, w)| !w.passes)
            // Only a riser the body stalls under is cut ([`CUT_RISE_M`], [`CUT_SHARE`]): the walk of the others ends at the far
            // floor's place, not under it, and the split bench put the falls 2c added on those cuts.
            .filter(|(ad, w)| {
                (self.height[wide.links[ad.0][ad.1] as usize] - self.height[ad.0] > CUT_RISE_M && w.share < CUT_SHARE)
                    // [`Body::walk_ceiling_stalls`]: two lines refused under a low ceiling: the client stalls there too.
                    || w.refused >= 2
            })
            .map(|(&(a, d), w)| (a as u32, d, w.share))
            .collect::<Vec<_>>();
        // A riser is cut only where the body has another walking way from its foot to its top close by: else the riser is the
        // way the routes there walk (a floor a drop or a jump led to, whose walking way out it is, which the reach guard of
        // [`WalkGrid::cut_keeping_reach`] does not see from the spawns), and a route that may not jump or drop is left none.
        let cut: std::collections::HashSet<(u32, u32)> = failing.iter().map(|&(a, d, _)| (a, wide.links[a as usize][d])).collect();
        let other_way = parallel(&failing, |&(a, d, _)| self.walk_way_round(wide, a, wide.links[a as usize][d], &cut));
        let (round, held): (Vec<_>, Vec<_>) = failing.into_iter().zip(other_way).partition(|&(_, round)| round);
        stats.checks_no_way_round = held.len();
        let held = held.into_iter().map(|((a, d, _), _)| (a, wide.links[a as usize][d])).collect();
        (stats, round.into_iter().map(|(f, _)| f).collect(), held, crouch)
    }

    /// [2c] Whether a walking way leads from `a` to `b` within [`ROUND_HOPS`] links without the links `cut` (Walk links only).
    fn walk_way_round(&self, wide: &Wide, a: u32, b: u32, cut: &std::collections::HashSet<(u32, u32)>) -> bool {
        let mut seen = std::collections::HashSet::from([a]);
        let mut ring = vec![a];
        for _ in 0..ROUND_HOPS {
            let mut next = Vec::new();
            for &x in &ring {
                for d in 0..8 {
                    let y = wide.links[x as usize][d];
                    if y == NONE || wide.kinds[x as usize][d] != LinkKind::Walk || cut.contains(&(x, y)) || !seen.insert(y) {
                        continue;
                    }
                    if y == b {
                        return true;
                    }
                    next.push(y);
                }
            }
            if next.is_empty() {
                return false;
            }
            ring = next;
        }
        false
    }

    /// [2c] Whether [`Body::walk_every_link`] walks the Walk link `a` -> `b`: a face no body walks on (a `dont_walk` one, or
    /// one steeper than the still body's slope, 45 deg) within [`CHECK_WIDE_REACH_M`] of either floor's capsule (`near`), a
    /// rise of [`CHECK_RISE_M`] or more, or a floor with no standing room. A link with none of these is flat open ground,
    /// which the tick walks at once.
    fn walk_checked(&self, a: usize, b: usize, near: &[bool]) -> bool {
        near[a] || near[b] || (self.height[b] - self.height[a]).abs() >= CHECK_RISE_M || self.stance(a, b) == Stance::Crouched
    }

    /// [2c] The stance a route takes over the link `a` -> `b`: crouched where either floor has no standing room.
    fn stance(&self, a: usize, b: usize) -> Stance {
        let room = |n: usize| self.ceiling[n] - self.height[n] >= STANDING_ROOM_M;
        if room(a) && room(b) { Stance::Standing } else { Stance::Crouched }
    }

    /// [2c] One walk check of [`Body::walk_every_link`]: the link out of `a` along `d` walked by the client's tick in its
    /// [`Stance`], on each of [`STARTS`]: from `a` (the body set where it can be), and, where a Walk link leads into `a` along
    /// the same way, on from where walking that link left the body (a body walks a route's links one after another, it is
    /// not set back on each floor's centre). A line passes when the body arrives both ways (a refusal under a low ceiling
    /// says nothing: it passes, unless [`Body::walk_ceiling_stalls`]: then it is the client's own stall, a failure); the link
    /// passes on two lines of three. Arriving is being over the far floor up to a step
    /// over its height: the client's tick steps a body up onto a top the cells keep no floor for (a prop, a curb, the cell's
    /// lower floor beside it) and its step down takes it off again (`cc_step_height` 0.80, startup.cfg). Chemical Plant: of
    /// the walks that got 90 % of the way and did not arrive within 0.2 m, 178,181 of 182,878 stood 0.2-0.8 m over the far
    /// floor, a few hundred higher; asking for 0.2 m cut 52,383 links, 17 times the old check's 3,031.
    fn walk_link(&self, wide: &Wide, world: &Collision, s: &Settings, start: WalkStart, a: usize, d: usize) -> LinkWalk {
        let b = wide.links[a][d] as usize;
        let back = wide.links[a][d ^ 1];
        let before = (back != NONE
            && wide.kinds[a][d ^ 1] == LinkKind::Walk
            && wide.links[back as usize][d] == a as u32
            && wide.kinds[back as usize][d] == LinkKind::Walk)
            .then_some(back as usize);
        let stance = match before {
            Some(q) if self.stance(q, a) == Stance::Crouched => Stance::Crouched,
            _ => self.stance(a, b),
        };
        let (pa, pb) = (self.position(a as NodeId), self.position(b as NodeId));
        let flat = Vec3::new(pb.x - pa.x, 0.0, pb.z - pa.z);
        let l = flat.length();
        let side = if l > 1e-4 { Vec3::new(-flat.z / l, 0.0, flat.x / l) } else { Vec3::ZERO };
        // [`Body::walk_ceiling_stalls`]: a refusal under a low ceiling is the client's own stall, not a pass.
        let stalls = self.body.walk_ceiling_stalls;
        let arrives = |w: Walked| matches!(w, Walked::At | Walked::AtEnd(_)) || (w == Walked::Ceiling && !stalls);
        let (mut lines, mut share, mut chain_failed, mut refused, mut crouched) = (0, 0.0f32, false, 0, 0);
        for off in STARTS {
            let (from, to) = (pa + side * off, pb + side * off);
            let mut c = body_at(world, start, stance, from);
            let mut centre = walk_on(world, s, &mut c, from, to, SPARE_TICKS, self.body.step);
            let mut only_crouched = false;
            if centre == Walked::Ceiling && stance == Stance::Standing {
                let mut c = body_at(world, start, Stance::Crouched, from);
                centre = walk_on(world, s, &mut c, from, to, SPARE_TICKS, self.body.step);
                only_crouched = matches!(centre, Walked::At | Walked::AtEnd(_));
            }
            share = share.max(match centre {
                Walked::Short(x) => x,
                Walked::Ceiling if stalls => 0.0,
                _ => 1.0,
            });
            if !arrives(centre) {
                refused += (centre == Walked::Ceiling) as usize;
                continue;
            }
            let chained = before.is_none_or(|q| {
                let from_q = self.position(q as NodeId) + side * off;
                let mut c = body_at(world, start, stance, from_q);
                // The link before failing is its own check's; from where it leaves the body this one says nothing.
                !matches!(walk_on(world, s, &mut c, from_q, from, SPARE_TICKS, self.body.step), Walked::At | Walked::AtEnd(_))
                    || arrives(walk_on(world, s, &mut c, from, to, SPARE_TICKS, self.body.step))
            });
            if chained {
                lines += 1;
                crouched += only_crouched as usize;
                // Two lines pass: the link passes whatever the third does (its share counts only for a failing link).
                if lines >= 2 {
                    break;
                }
            } else {
                chain_failed = true;
            }
        }
        LinkWalk { passes: lines >= 2, share, chain_failed: chain_failed && lines < 2, refused, crouched }
    }

    /// How many bridges of `all` are side by side with the bridge `a` -> `b`, it included, counted out to [`BRIDGE_MIN_RUN`]
    /// either side: one floor's cell over across the bridge's way each, to a floor one cell over from `b`'s.
    fn bridge_run(&self, all: &std::collections::HashSet<(u32, u32)>, a: u32, b: u32) -> usize {
        let (ia, ka) = self.cell_xz(self.cell_of[a as usize] as usize);
        let (ib, kb) = self.cell_xz(self.cell_of[b as usize] as usize);
        let (dx, dz) = ((ib - ia) as f32, (kb - ka) as f32);
        let l = dx.hypot(dz);
        let (sx, sz) = ((-dz / l).round() as i32, (dx / l).round() as i32);
        let mut run = 1;
        for side in [1, -1] {
            for s in 1..BRIDGE_MIN_RUN as i32 {
                let (ox, oz) = (side * s * sx, side * s * sz);
                let (Some(ca), Some(cb)) = (self.cell_index(ia + ox, ka + oz), self.cell_index(ib + ox, kb + oz)) else { break };
                let found = (self.start[ca]..self.start[ca + 1])
                    .any(|a2| (self.start[cb]..self.start[cb + 1]).any(|b2| all.contains(&(a2, b2))));
                if !found {
                    break;
                }
                run += 1;
            }
        }
        run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad(tris: &mut Vec<Triangle>, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        tris.push(Triangle { v: [a, b, c], walker: WalkerBehaviour::BySlope });
        tris.push(Triangle { v: [a, c, d], walker: WalkerBehaviour::BySlope });
    }

    fn floor(tris: &mut Vec<Triangle>, x0: f32, z0: f32, x1: f32, z1: f32, y: f32) {
        quad(tris, Vec3::new(x0, y, z0), Vec3::new(x0, y, z1), Vec3::new(x1, y, z1), Vec3::new(x1, y, z0));
    }

    /// [A364 C2 row 52] Two level floors across a pit 3 m deep: a gap of 1.0 m is walked over by the controller and bridged (the
    /// client walks level gaps to 1.79 m); one of 2.5 m is not tried. Before, only a gap of one cell was.
    #[test]
    fn a_level_gap_the_client_walks_over_is_bridged() {
        let bridges = |gap: f32| {
            let mut tris = Vec::new();
            floor(&mut tris, 0.0, 0.0, 5.0, 6.0, 0.0);
            floor(&mut tris, 5.0 + gap, 0.0, 12.0, 6.0, 0.0);
            floor(&mut tris, 5.0, 0.0, 5.0 + gap, 6.0, -3.0);
            quad(&mut tris, Vec3::new(5.0, -3.0, 0.0), Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 6.0), Vec3::new(5.0, -3.0, 6.0));
            let x1 = 5.0 + gap;
            quad(&mut tris, Vec3::new(x1, -3.0, 0.0), Vec3::new(x1, -3.0, 6.0), Vec3::new(x1, 0.0, 6.0), Vec3::new(x1, 0.0, 0.0));
            let body = Body { client_controller: true, walk_start_clear: true, crack_bridges: true, exe_collision: true, start_witness: true, ..Body::client() };
            let start = [Vec3::new(1.0, 0.0, 3.0)];
            let (grid, stats) =
                WalkGrid::build_kept_with_links(&tris, [0.0, 0.0, 12.0, 6.0], 0.25, body, &start, &start, OffMeshLinks::Gaps);
            let far = grid.locate(Vec3::new(10.0, 0.0, 3.0), 0.3).is_some_and(|n| grid.reached(n));
            (stats.controlled.bridges_added, far)
        };
        let (added, far) = bridges(1.0);
        assert!(added > 0 && far, "1.0 m: {added} bridges");
        assert_eq!(bridges(2.5).0, 0, "2.5 m is past the walked gap");
    }

    /// [A329] A floor beside a 0.785 m ledge (the level_04 day_4 case): its centre 0.12 m from the ledge's face, the walk up
    /// to the top's floor centre. The old start leaves the body inside the ledge and the controller pops it on top (as the exe
    /// does from there); the clear start puts it where a standing body can be, and the walk stays below, as the exe's walk
    /// from the client's own spot does (Unicorn: blocked; it climbs a vertical face walking up to 0.75 m, not 0.76).
    #[test]
    fn a_floor_beside_a_ledge_higher_than_a_walk_climbs_starts_where_a_body_stands() {
        let mut tris = Vec::new();
        floor(&mut tris, 0.0, -5.0, 5.0, 2.0, 0.0);
        floor(&mut tris, 0.0, 2.0, 5.0, 8.0, 0.785);
        quad(&mut tris, Vec3::new(0.0, 0.0, 2.0), Vec3::new(0.0, 0.785, 2.0), Vec3::new(5.0, 0.785, 2.0), Vec3::new(5.0, 0.0, 2.0));
        let world = Collision::build(&tris);
        let s = Settings::LIVE;
        let (from, to) = (Vec3::new(2.5, 0.0, 1.88), Vec3::new(2.5, 0.785, 2.12));
        let (_, old) = walk_share(&world, &s, WalkStart::AboveStep, from, to);
        let (share, clear) = walk_share(&world, &s, WalkStart::Clear, from, to);
        assert!(old, "the old start climbs it from inside the ledge");
        assert!(!clear && share < CHECK_SHARE, "from where a body stands it does not climb: share {share}");
        assert!(out_of_faces(&world, from).z < 2.0 - STAND_RADIUS_M + 0.01, "pushed out to the capsule's radius");
    }

    /// A floor at 0 and a top 0.6 m up past one 0.25 m cell (x 5.0-5.25), 6 m wide. With `strip`, that cell holds a 0.3 m
    /// step (walked up in two) except over `strip` metres of the width, where it holds only the riser (none: the riser all the
    /// way): the walk-off bridges the pass makes up it, and the ones it walks but leaves pressed as too narrow a run.
    fn bridges_up(strip: Option<f32>) -> (usize, usize) {
        let mut tris = Vec::new();
        floor(&mut tris, 0.0, 0.0, 5.0, 6.0, 0.0);
        floor(&mut tris, 5.25, 0.0, 10.0, 6.0, 0.6);
        quad(&mut tris, Vec3::new(5.25, 0.0, 0.0), Vec3::new(5.25, 0.6, 0.0), Vec3::new(5.25, 0.6, 6.0), Vec3::new(5.25, 0.0, 6.0));
        if let Some(w) = strip {
            let (z0, z1) = (3.0 - w / 2.0, 3.0 + w / 2.0);
            for (a, b) in [(0.0, z0), (z1, 6.0)] {
                floor(&mut tris, 5.0, a, 5.25, b, 0.3);
                quad(&mut tris, Vec3::new(5.0, 0.0, a), Vec3::new(5.0, 0.0, b), Vec3::new(5.0, 0.3, b), Vec3::new(5.0, 0.3, a));
            }
        }
        let body = Body { client_controller: true, ..Body::client() };
        let (grid, stats) = WalkGrid::build_kept_with_links(&tris, [0.0, 0.0, 10.0, 6.0], 0.25, body, &[], &[], OffMeshLinks::Gaps);
        let walked = grid
            .off_links()
            .iter()
            .filter(|l| {
                l.kind == LinkKind::GapJump
                    && l.press == Takeoff::WalkOff
                    && grid.position(l.from).y.abs() < 0.05
                    && (grid.position(l.to).y - 0.6).abs() < 0.05
            })
            .count();
        (walked, stats.controlled.bridges_narrow)
    }

    /// [A329] The level_06 day_2 dip at (18.4, 102.9): a high floor at 0 meets a one-cell dip 0.81 m down only at its corner
    /// (the dip's sides toward it lead down to ground 3 m below, so erosion keeps the dip), and a floor 0.25 m down lies past
    /// the dip on the diagonal. The grid crosses by a drop into the dip, which the client never takes. With `crack_bridges` the
    /// controller walks the body over the dip and the walked bridge is made (a run of one: it stands beside the drop); without,
    /// none.
    fn crack_bridges(on: bool) -> usize {
        let mut tris = Vec::new();
        let (lo, far, deep) = (-0.81, -0.25, -3.0);
        // The high floor (x < 5, z > 3.25), the dip (one cell), the far floor (x > 5, z < 3.25 but the dip), ground below.
        floor(&mut tris, 0.0, 3.25, 5.0, 6.0, 0.0);
        floor(&mut tris, 5.0, 3.0, 5.25, 3.25, lo);
        floor(&mut tris, 5.25, 0.0, 10.0, 3.25, far);
        floor(&mut tris, 5.0, 0.0, 5.25, 3.0, far);
        floor(&mut tris, 0.0, 0.0, 5.0, 3.25, deep);
        floor(&mut tris, 5.0, 3.25, 10.0, 6.0, deep);
        let body = Body { client_controller: true, crack_bridges: on, ..Body::client() };
        let (grid, _) = WalkGrid::build_kept_with_links(&tris, [0.0, 0.0, 10.0, 6.0], 0.25, body, &[], &[], OffMeshLinks::Gaps);
        grid.off_links()
            .iter()
            .filter(|l| {
                let (a, b) = (grid.position(l.from), grid.position(l.to));
                l.press == Takeoff::WalkOff && a.y > -0.3 && b.y > -0.3 && (a.y - b.y).abs() > 0.2
            })
            .count()
    }

    #[test]
    fn a_crack_the_grid_crossed_by_a_drop_gets_the_walked_bridge() {
        assert_eq!(crack_bridges(false), 0, "without the switch the dip has no bridge");
        assert!(crack_bridges(true) >= 1, "with it: {} bridges", crack_bridges(true));
    }

    /// [2c] A passage 1.4 m high (a roof plate over x 4-6): its floors have no standing room. Without
    /// [`Body::walk_every_link`] no link into it is walked (a standing body cannot go there); with it they are walked
    /// crouched, as the route takes them, and kept.
    #[test]
    fn a_low_passage_is_walked_crouched_with_walk_every_link() {
        let mut tris = Vec::new();
        floor(&mut tris, 0.0, 0.0, 10.0, 6.0, 0.0);
        floor(&mut tris, 4.0, 0.0, 6.0, 6.0, 1.4);
        let build = |walk_every_link: bool| {
            let body = Body { client_controller: true, walk_start_clear: true, walk_every_link, ..Body::client() };
            let start = [Vec3::new(1.0, 0.0, 3.0)];
            WalkGrid::build_kept_with_links(&tris, [0.0, 0.0, 10.0, 6.0], 0.25, body, &start, &start, OffMeshLinks::Gaps)
        };
        let (_, off) = build(false);
        assert_eq!(off.controlled.checks_crouched, 0);
        let (grid, on) = build(true);
        assert!(on.controlled.checks_crouched > 0, "{:?}", on.controlled);
        assert_eq!(on.controlled.checks_dropped + on.controlled.checks_kept, 0, "a crouched body walks it: {:?}", on.controlled);
        let under = grid.locate(Vec3::new(5.0, 0.0, 3.0), 0.3).expect("a floor under the roof");
        assert!(!grid.can_stand(under) && grid.reached(under));
    }

    /// [2c] A platform whose only way up is a 0.785 m vertical step (a Walk link with the clients' 0.8 m step, which the
    /// client's tick does not climb: the exe climbs a vertical face up to 0.75 m, Unicorn): with [`Body::walk_every_link`] its
    /// failing links up are kept as the platform's only way and marked; without, none is marked.
    #[test]
    fn a_failing_only_way_up_is_marked_with_walk_every_link() {
        let mut tris = Vec::new();
        floor(&mut tris, 0.0, 0.0, 5.0, 6.0, 0.0);
        floor(&mut tris, 5.0, 0.0, 10.0, 6.0, 0.785);
        quad(&mut tris, Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 0.785, 0.0), Vec3::new(5.0, 0.785, 6.0), Vec3::new(5.0, 0.0, 6.0));
        let build = |walk_every_link: bool| {
            let body = Body { client_controller: true, walk_start_clear: true, walk_every_link, ..Body::client() };
            let start = [Vec3::new(1.0, 0.0, 3.0)];
            WalkGrid::build_kept_with_links(&tris, [0.0, 0.0, 10.0, 6.0], 0.25, body, &start, &start, OffMeshLinks::None)
        };
        let (off, _) = build(false);
        assert_eq!(off.walk_marked_count(), 0);
        let (on, stats) = build(true);
        // Kept: no walking way round the riser (the step is the platform's whole edge).
        assert!(stats.controlled.checks_no_way_round > 0 && stats.controlled.checks_dropped == 0, "{:?}", stats.controlled);
        assert!(on.walk_marked_count() > 0, "{:?}", stats.controlled);
        let top = on.locate(Vec3::new(8.0, 0.785, 3.0), 0.3).expect("the platform");
        assert!(on.reached(top), "kept as the only way up");
        // A marked link is a Walk link of the grid.
        let marked = (0..on.len() as NodeId)
            .flat_map(|a| on.links(a).map(move |(b, kind)| (a, b, kind)))
            .filter(|&(a, b, _)| on.walk_marked(a, b))
            .collect::<Vec<_>>();
        assert!(!marked.is_empty() && marked.iter().all(|&(_, _, kind)| kind == LinkKind::Walk));
    }

    /// [A313] A riser bare its whole width is bridged; bare over a 0.5 m strip only (two columns of the grid, the rest
    /// stepped) it is walked by the controller but not bridged: a bot steering at a point ahead does not keep inside so
    /// narrow a run, and walks the step beside it.
    #[test]
    fn a_narrow_bridge_run_is_not_walked() {
        let (open, _) = bridges_up(None);
        assert!(open >= BRIDGE_MIN_RUN, "the bare riser: {open} bridges");
        let (walked, narrow) = bridges_up(Some(0.5));
        assert_eq!(walked, 0, "the 0.5 m strip");
        assert!(narrow > 0, "the 0.5 m strip had bridges the controller walks, left pressed");
    }
}

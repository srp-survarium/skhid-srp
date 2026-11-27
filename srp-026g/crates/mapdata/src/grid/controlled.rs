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
use super::gap::{OffLink, Share, Takeoff};
use super::*;
use crate::controller::tick::{Controller, Settings, TickInput};
use crate::controller::speed;
use crate::controller::{Capsule, Collision, Shape};
use crate::motion::{self, ClientBody, Input, Kin, Pass};
use super::memo::{Codec, Memo, MemoCodec, MemoInput, Mode, Site, StableIds, Windows};
use crate::digest::Digest128;

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

/// [walk lane] The side line of [`STARTS`] a checked Walk link was walked on when its centre line was not
/// ([`WalkGrid::walk_lane`]): the client's walk tick makes the link only from there. batt-m3's floor 421325 (-38.78, 11.161,
/// -7.295) up to 421323 (-39.03, 11.760, -7.295): the centre and the -0.1 m line stay at 11.1, the +0.1 m line goes up, on the
/// port and on the exe alike (feed_steps_v2 on rig_real_v3, 6 of 6 lines equal); bots walking the centre pushed there at 0 m/s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WalkLane {
    /// [`STARTS`]`[1]` across the link ([`link_side`]).
    Minus,
    /// [`STARTS`]`[2]` across the link.
    Plus,
}

impl WalkLane {
    /// The lane of a side line's offset (one of [`STARTS`] but the centre's).
    fn of_offset(off: f32) -> Option<Self> {
        if off == STARTS[1] {
            Some(Self::Minus)
        } else if off == STARTS[2] {
            Some(Self::Plus)
        } else {
            None
        }
    }

    /// How far across the link ([`link_side`]) the line runs, m.
    pub fn offset_m(self) -> f32 {
        match self {
            Self::Minus => STARTS[1],
            Self::Plus => STARTS[2],
        }
    }

    /// The file's byte.
    pub fn byte(self) -> u8 {
        match self {
            Self::Minus => 1,
            Self::Plus => 2,
        }
    }

    /// The lane of a file's byte; None for any other.
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            1 => Some(Self::Minus),
            2 => Some(Self::Plus),
            _ => None,
        }
    }

    /// The line's ends for the link from `from` to `to`: both moved [`Self::offset_m`] across it.
    pub fn line(self, from: Vec3, to: Vec3) -> (Vec3, Vec3) {
        let side = link_side(from, to);
        (from + side * self.offset_m(), to + side * self.offset_m())
    }
}

/// The unit vector across a link from `from` to `to` on the ground plane that [`STARTS`]' offsets move its lines along (zero
/// for a link with no flat length).
pub fn link_side(from: Vec3, to: Vec3) -> Vec3 {
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let l = flat.length();
    if l > 1e-4 { Vec3::new(-flat.z / l, 0.0, flat.x / l) } else { Vec3::ZERO }
}
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

/// The bridges the controller walked in one pass, by take-off floor: what [`WalkGrid::bridge_run`] counts a run over.
struct Bridges(std::collections::HashMap<u32, Vec<u32>>);

impl Bridges {
    fn of(pairs: impl Iterator<Item = (u32, u32)>) -> Self {
        let mut by: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
        for (a, b) in pairs {
            by.entry(a).or_default().push(b);
        }
        Self(by)
    }

    /// The landings of the walked bridges out of floor `a`.
    fn from(&self, a: u32) -> &[u32] {
        self.0.get(&a).map_or(&[], Vec::as_slice)
    }
}

/// The new bridges over a crack the grid crossed by a drop ([`Body::crack_bridges`]), as (from, to).
type OverDrop = std::collections::HashSet<(u32, u32)>;

/// [later bridges] The side links and off links as the walk check's cut left them: what [`WalkGrid::later_bridges`] tells the
/// floors whose links a later pass changed by.
pub(super) struct SideLinks {
    links: Vec<[u32; 8]>,
    kinds: Vec<[LinkKind; 8]>,
    /// (from, to), sorted.
    off: Vec<(u32, u32)>,
}

impl SideLinks {
    pub(super) fn of(wide: &Wide) -> Self {
        let mut off: Vec<(u32, u32)> = wide.off.iter().map(|l| (l.from, l.to)).collect();
        off.sort_unstable();
        Self { links: wide.links.clone(), kinds: wide.kinds.clone(), off }
    }
}

/// What the controller pass did ([`BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ControlledStats {
    pub bridges_tried: usize,
    pub bridges_walked: usize,
    pub bridges_added: usize,
    /// Bridges the controller walks in a run narrower than [`BRIDGE_MIN_RUN`]: not walked (a gap link stays pressed).
    pub bridges_narrow: usize,
    /// [later bridges] The new-bridge candidates tried again for the floors a later pass cut a link of
    /// ([`WalkGrid::later_bridges`]).
    pub bridges_later_tried: usize,
    /// [later bridges] Of those, the walked bridges added.
    pub bridges_later: usize,
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
    /// [given-back walks] The wall give-back's Walk links the walk check walked ([`WalkGrid::walk_given_back`]).
    pub given_back_checks: usize,
    /// [given-back walks] Of those, the failing ones taken away.
    pub given_back_dropped: usize,
    /// [given-back walks] Of those, the failing ones kept as a floor's only way in or back (`WalkGrid::cut_keeping_reach`), marked.
    pub given_back_kept: usize,
    /// [given-back walks] Of those, the failing ones kept, marked, for no walking way round them ([`ROUND_HOPS`]).
    pub given_back_no_way_round: usize,
}

/// [2c] The risers [`WalkGrid::walk_every_link`] keeps for no walking way round them (floor, floor).
type Held = Vec<(NodeId, NodeId)>;
/// [`Body::walk_ceiling_stalls`] The Walk links the walk check passes only crouched, as (from, to).
type Crouched = Vec<(u32, u32)>;
/// [walk lane] The Walk links the walk check passed only on a side line, as (from, to, lane).
type Lanes = Vec<(u32, u32, WalkLane)>;
/// [`WalkGrid::walk_every_link`]'s answer: the stats, the failing links (floor, direction, share), the held, the crouched and
/// the side-line walks.
type WalkEveryLink = (ControlledStats, Vec<(u32, usize, f32)>, Held, Crouched, Lanes);

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
    /// [walk lane] The first side line that arrived when the centre line did not ([`WalkLane`]); None when the centre arrived
    /// or no line did.
    lane: Option<WalkLane>,
}

/// [body physics S6c] The walk check's results as the memo stores them: every field. 02: [walk lane] the lane.
impl MemoCodec for LinkWalk {
    const TAG: u32 = 0x574c_4b02;
    fn encode(&self, out: &mut Codec<'_>) {
        self.passes.encode(out);
        out.put_f32(self.share);
        self.chain_failed.encode(out);
        out.put_u32(u32::try_from(self.refused).expect("a link's lines fit 32 bits"));
        out.put_u32(u32::try_from(self.crouched).expect("a link's lines fit 32 bits"));
        out.put_u32(u32::from(self.lane.map_or(0, WalkLane::byte)));
    }
    fn decode(input: &mut Codec<'_>) -> Option<Self> {
        Some(LinkWalk {
            passes: bool::decode(input)?,
            share: input.f32()?,
            chain_failed: bool::decode(input)?,
            refused: input.u32()? as usize,
            crouched: input.u32()? as usize,
            lane: match input.u32()? {
                0 => None,
                b => Some(WalkLane::from_byte(u8::try_from(b).ok()?)?),
            },
        })
    }
}

/// [body physics S6c] One job of the walk check's memo site `walk.link` ([`walk_link_job`]): the link's floors (from `a` to `b`,
/// and `before`, the floor of the Walk link into `a` along the same way, the chained walk's start) and the digest of the floors in
/// its window ([`Windows::of_floors`], [`WALK_LINK_WINDOW_CELLS`] about `a`'s cell). The job reads the grid only through those
/// floors' places and rooms ([`WalkGrid::position`], [`WalkGrid::stance`]); the world, the body and the tick's settings are the
/// memo's global inputs.
#[derive(Clone, Debug)]
pub(super) struct WalkLinkItem {
    a: NodeId,
    b: NodeId,
    before: Option<NodeId>,
    window: u128,
}

/// [body physics S6c] The cells about the take-off's cell a walk check's window holds: the link's floors are its own and its
/// neighbours' ([`DIRS`] is one cell over), checked as the items are made.
const WALK_LINK_WINDOW_CELLS: i64 = 1;

crate::memo_ctx! {
    /// [body physics S6c] The walk check site's context: the window (hashed), and the grid, the world, the tick's settings and the
    /// start (the body's, [`Body::walk_start`]), read through the items' windows and the memo's global inputs.
    pub(super) struct WalkLinkCtx<'a> {
        window_cells: i64,
    } through {
        grid: &'a WalkGrid,
        world: &'a Collision,
        s: &'a Settings,
        start: WalkStart,
    }
}

/// [body physics S6c] The walk check site's key of a job: its floors by their stable ids and its window.
fn walk_link_key(_: &WalkLinkCtx<'_>, ids: &StableIds, item: &WalkLinkItem) -> u128 {
    let mut d = Digest128::default();
    ids.id(item.a).feed(&mut d);
    ids.id(item.b).feed(&mut d);
    item.before.map(|q| ids.id(q)).feed(&mut d);
    d.u128(item.window);
    d.finish()
}

/// [body physics S6c] The walk check site's job: [`WalkGrid::walk_link_floors`] of the item's floors.
fn walk_link_job(ctx: &WalkLinkCtx<'_>, item: &WalkLinkItem) -> LinkWalk {
    ctx.grid.walk_link_floors(ctx.world, ctx.s, ctx.start, item.a as usize, item.b as usize, item.before.map(|q| q as usize))
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
pub(crate) fn out_of_walls(world: &Collision, feet: Vec3) -> Vec3 {
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

/// [A334] What [`WalkGrid::pull_walk_verdict`] knows of a pulled leg.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullWalk {
    /// The client's walk tick walks it (or the switch is off).
    Walks,
    /// It does not.
    Fails,
    /// Not walked now: this thread's budget is spent ([`set_pull_walk_budget`]). A pull does not take the leg straight.
    Unknown,
}

thread_local! {
    /// [A334] [`WalkGrid::pull_walk_verdict`]'s answers by (the grid's number, from, to): a pure function of that grid's floors,
    /// walker and body, so caching changes no answer. Cleared at [`PULL_WALKS_MAX`] entries.
    static PULL_WALKS: std::cell::RefCell<std::collections::HashMap<(u64, NodeId, NodeId), bool>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// [A334] This thread's budget of walk ticks for [`WalkGrid::pull_walk_verdict`]: set by the bots' poll each tick
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
    // [body physics S6b] The client's body walked by its keys ([`walk_on`]), going through `from` in its steady walk
    // ([`walking_at`]), as the direct walk at full speed did and a body walking a route does.
    let mut b = walking_at(world, start, from, yaw_toward(from, to));
    match walk_on(world, s, &mut b, Stance::Standing, from, to, 0, AT_HEIGHT_M) {
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

/// [S7] The walk check's speed for a body in `stance`, m/s: the state's desired speed (`default.player`'s `movement_speed`,
/// [`speed::desired`]): standing forward 3.5 ([`WALK_MPS`]), crouched forward 2.5. A crouched body walked at 3.5 is no client's:
/// over `min_walk_speed` (3.0) the client's step takes its other branch (`controller::tick` `step_as`).
pub fn stance_mps(stance: Stance) -> f32 {
    let stance = match stance {
        Stance::Standing => speed::Stance::Stand,
        Stance::Crouched => speed::Stance::Crouch,
    };
    speed::desired(speed::Gait::Move { stance, heading: speed::Heading::Forward, aiming: false })
}

/// [row 6] How far along the way `from` -> `to` (a share of its flat length) feet at `feet` are, counted only while they are on
/// the way's level: no lower than its lower end by more than the arrival band ([`AT_HEIGHT_M`]); 0 under it. A body sliding down
/// a slope beside a riser goes along the way's line too: on Mamayev Kurgan (26.0, 25.5, 91.7) a line of the walk 366489 ->
/// 365709 up 0.36 m slid 1.3 m across and 0.9 m down and scored 0.63 of the way, so the failing riser was not cut
/// ([`CUT_SHARE`]), where the exe's tick gets up it from no line, standing or crouched.
fn way_along(from: Vec3, to: Vec3, feet: Vec3) -> f32 {
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let length = flat.length();
    if length < 1e-4 || feet.y < from.y.min(to.y) - AT_HEIGHT_M {
        return 0.0;
    }
    Vec3::new(feet.x - from.x, 0.0, feet.z - from.z).dot(flat) / (length * length)
}

/// [body physics S6b] The walk check's body at `feet` in `stance`, facing `yaw`: placed where a body in that stance can be
/// ([`body_at`]) with the put's one small downward speed, as the game puts a body (and the exe rig's put writes it): without it
/// the client's step skip (`can_skip_player_step` 0x7C7E0: no walk, no speed) holds a body put up to 3.7 cm over its floor where
/// it was put on the keys' first step (the blend's, which does not move), where the exe's tick steps it down; 13 of S6b-0's 32
/// key-walk feeds parted from the exe there. It is walked at once, as the exe rig's feeds are: a body left to come to rest first
/// slides off a take-off no body stands on (S6b-0 v5, level_05 day 4: 679 of 838 links the rest failed, down to 4.7 m) that a
/// walk from it crosses.
pub fn put_at(world: &Collision, start: WalkStart, stance: Stance, feet: Vec3, yaw: f32) -> ClientBody {
    let mut c = body_at(world, start, stance, feet);
    c.velocity = Vec3::new(0.0, -0.01, 0.0);
    ClientBody::of(c, yaw)
}

/// [body physics S6b] The keys of a walk check's body in `stance`: the crouch key when crouched (as [`body_at`] holds it), and the
/// forward key.
fn stance_keys(stance: Stance) -> u32 {
    motion::keys::FORWARD | if stance == Stance::Crouched { motion::keys::CROUCH } else { 0 }
}

/// [body physics S6b] How far past the far floor a walk check's body aims, m: a body at speed re-aimed at the floor itself, with the
/// client's one-step turn lag (the walk turns by the input before), circles it at 3.5 m/s on a 0.25 m link (S6b-0's chained walks);
/// aimed past it, it walks through.
pub const AIM_PAST_M: f32 = 0.5;

/// [body physics S6b] How many forward inputs a walking body's blend is replayed with ([`walking_at`]): the blend is at its target
/// after six steps (`WalkBlend`, `BLEND_STEPS`), and two more.
const WALKING_INPUTS: usize = 8;

/// [body physics S6b] The walk check's standing body at `feet` already walking forward toward `yaw` in its steady walk: put
/// there ([`put_at`]), then its blend replayed with [`WALKING_INPUTS`] forward inputs ([`ClientBody::from_saved`], the
/// world's body: its walk at full weight, the next step moving by the forward key). A crack is crossed by a body that comes to it
/// walking: one started from rest at a floor's centre 0.25-0.5 m before a 1.0 m gap blends up to 1-2 m/s by the edge and drops
/// into it, where the client walks level gaps to 1.79 m (A364 C2 row 52).
pub fn walking_at(world: &Collision, start: WalkStart, feet: Vec3, yaw: f32) -> ClientBody {
    let put = put_at(world, start, Stance::Standing, feet, yaw);
    let forward = [Input { keys: motion::keys::FORWARD, yaw }; WALKING_INPUTS];
    ClientBody::from_saved(&put.save(), &forward, Kin::CLIENT)
}

/// The walk check's body `b` (a [`ClientBody`]) walked from where it is toward `to` (feet) by the client's keys: the forward key
/// (and the crouch key in `stance`) with the yaw toward a point [`AIM_PAST_M`] past `to` along `from` -> `to`, for the ticks the
/// leg from where the body stands (or from `from`, the longer) takes at the stance's speed ([`stance_mps`]), two more and `spare`
/// more: how it ended, the share counted along `from` -> `to`. At `to` is within [`AT_FLAT_M`] across and from [`AT_HEIGHT_M`]
/// under its height to `above` over it, or past its line along the way within those bands (a moving body walks through a
/// floor). `b` is left where the walk left it, its blend and speed carried, so a next leg goes on from there as a body walking a
/// route does.
#[allow(clippy::too_many_arguments)]
pub fn walk_on(world: &Collision, s: &Settings, b: &mut ClientBody, stance: Stance, from: Vec3, to: Vec3, spare: usize, above: f32) -> Walked {
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let length = flat.length();
    if length < 1e-4 {
        return Walked::At;
    }
    let dir = flat * (1.0 / length);
    let mps = stance_mps(stance);
    // The leg from where the body stands: a start out of the faces ([`body_at`]) can be up to a capsule's radius behind `from`.
    let leg = length.max(b.feet().distance_xz(to));
    let ticks = (leg / (mps * 0.05)).ceil() as usize + 2 + spare;
    let pass = Pass { world, settings: s };
    let keys = stance_keys(stance);
    let aim = to + dir * AIM_PAST_M;
    let at = |feet: Vec3| {
        let rel = Vec3::new(feet.x - to.x, 0.0, feet.z - to.z);
        let along = rel.dot(dir);
        let across = (rel - dir * along).length();
        let level = feet.y - to.y >= -AT_HEIGHT_M && feet.y - to.y <= above;
        level && (rel.length() <= AT_FLAT_M || (along >= 0.0 && across <= AT_FLAT_M))
    };
    let along_on_level = |feet: Vec3| way_along(from, to, feet);
    let mut best = 0.0f32;
    for _ in 0..ticks {
        let feet = b.feet();
        if at(feet) {
            return Walked::At;
        }
        best = best.max(along_on_level(feet));
        let before = *b;
        let t = b.tick(&pass, Kin::CLIENT, Input { keys, yaw: yaw_toward(feet, aim) });
        if t.ceiling {
            return Walked::Ceiling;
        }
        // A step is a function of the body alone (the world and the settings are fixed) and the next input of its feet alone: a
        // step that changed nothing (its controller, blend and input) is a body held where it is for every tick left.
        if *b == before {
            break;
        }
    }
    let feet = b.feet();
    let share = best.max(along_on_level(feet));
    if at(feet) {
        Walked::AtEnd(share)
    } else if spare > 0 && walks_on_to_level(world, s, b, stance, to, dir) {
        Walked::At
    } else {
        Walked::Short(share)
    }
}

/// The yaw from `feet` toward `aim` on the ground.
fn yaw_toward(feet: Vec3, aim: Vec3) -> f32 {
    let (dx, dz) = (aim.x - feet.x, aim.z - feet.z);
    let l = dx.hypot(dz).max(1e-6);
    motion::yaw_of((dx / l, dz / l))
}

/// [2c] How far past a link's end a walk check walks on ([`walks_on_to_level`]), m.
const ON_PAST_M: f32 = 1.5;

/// [2c] Whether a body that did not arrive at `to` comes onto its floor's level when walked on toward a point [`ON_PAST_M`]
/// past it along the link's way `dir` (its ticks and [`SPARE_TICKS`] more), by the client's keys as [`walk_on`]: its feet within
/// [`AT_HEIGHT_M`] of `to`'s height and no more than [`AT_FLAT_M`] short of `to` along the way. A body walking a route walks on,
/// so the client crosses such a link a little further on. The exactness session's walk of 6,701 of v3's cut links on the game's
/// shapes world (FINDINGS-FEED-exactness 2026-11-12T05:24Z, `private/grid/rig/cut-walk/`): the classes whose walk went the full
/// way but did not arrive come onto the level so in 53-73 % (flat 66 %, rise 58 %, down 63 %), while the links that stall under a
/// riser do in 12 % (the body stays under it: those cuts stand).
fn walks_on_to_level(world: &Collision, s: &Settings, b: &mut ClientBody, stance: Stance, to: Vec3, dir: Vec3) -> bool {
    let aim = Vec3::new(to.x + dir.x * ON_PAST_M, to.y, to.z + dir.z * ON_PAST_M);
    let ticks = (ON_PAST_M / (stance_mps(stance) * 0.05)).ceil() as usize + SPARE_TICKS;
    let pass = Pass { world, settings: s };
    let keys = stance_keys(stance);
    let level_at = |f: Vec3| (f.y - to.y).abs() <= AT_HEIGHT_M && Vec3::new(f.x - to.x, 0.0, f.z - to.z).dot(dir) >= -AT_FLAT_M;
    for _ in 0..ticks {
        let feet = b.feet();
        if level_at(feet) {
            return true;
        }
        b.tick(&pass, Kin::CLIENT, Input { keys, yaw: yaw_toward(feet, aim) });
    }
    level_at(b.feet())
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

/// A jump link the controller walks over (a bridge): walked off, on the line. [body physics S5b] An aimed press's aim goes with it:
/// only a press is aimed (the file's reader refuses an aim on any other, `WalkGrid::read`), and an aimed sprint across a gap a
/// walk crosses (Cologne, L1's links to every top) left an aimed walk-off that refused the map's whole grid on load.
pub(super) fn walked_bridge(l: OffLink) -> OffLink {
    OffLink { press: Takeoff::WalkOff, aim: super::gap::Aim::STRAIGHT, ..l }
}

/// Runs `job` over `items` on the machine's threads, in order: each result at its item's place, whatever thread ran it.
///
/// [bench speed] The threads take the items a block at a time from one counter ([`PARALLEL_BLOCKS`] blocks a thread), so a thread
/// whose items are cheap takes more of them. Each thread used to get one fixed share of the items, which are in map order, and
/// the call waited for the thread whose share held the costly part of the map: on 8 threads the rim jumps' flights took 689 s on
/// Rudnya while the threads' mean was 477 s. A job's answer depends on its item only: the jobs read the grid and the world, and
/// a thread's search arrays are reused under a new mark each search and read only where that search reached
/// (`WalkGrid::start_search_reaching`), so the split changes no result (the grids of 8 and 16 threads were the same before this).
///
/// [incremental grids] Called on a thread whose build's clock runs ([`super::timing`]), the call is recorded there by its place in
/// the source (`#[track_caller]`): its items, its jobs' summed and longest seconds, its wall seconds.
#[track_caller]
pub(super) fn parallel<T: Sync, R: Send + Default + Clone>(items: &[T], job: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let site = std::panic::Location::caller();
    let timed = super::timing::running();
    let started = std::time::Instant::now();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let block = items.len().div_ceil(threads * PARALLEL_BLOCKS).max(1);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let workers_used = threads.min(items.len().div_ceil(block));
    let done: Vec<WorkerDone<R>> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..workers_used)
            .map(|_| {
                scope.spawn(|| {
                    let mut taken = Vec::new();
                    let mut times = super::timing::JobTimes::default();
                    loop {
                        let first = next.fetch_add(block, std::sync::atomic::Ordering::Relaxed);
                        if first >= items.len() {
                            break (taken, times);
                        }
                        let end = (first + block).min(items.len());
                        let results = if timed {
                            items[first..end]
                                .iter()
                                .map(|item| {
                                    let began = std::time::Instant::now();
                                    let result = job(item);
                                    times.add(began.elapsed());
                                    result
                                })
                                .collect()
                        } else {
                            items[first..end].iter().map(&job).collect()
                        };
                        taken.push((first, results));
                    }
                })
            })
            .collect();
        workers.into_iter().map(|worker| worker.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))).collect()
    });
    let mut times = super::timing::JobTimes::default();
    let done: Vec<Vec<(usize, Vec<R>)>> = done
        .into_iter()
        .map(|(taken, worker)| {
            times.merge(worker);
            taken
        })
        .collect();
    let mut out = vec![R::default(); items.len()];
    for (first, results) in done.into_iter().flatten() {
        for (at, result) in results.into_iter().enumerate() {
            out[first + at] = result;
        }
    }
    if timed {
        super::timing::record_site(site, items.len(), workers_used, started.elapsed(), times);
    }
    out
}

/// A [`parallel`] worker's results (each block's first item and its results) and its jobs' times.
type WorkerDone<R> = (Vec<(usize, Vec<R>)>, super::timing::JobTimes);

/// How many blocks a thread's even share of [`parallel`]'s items is cut into: fine enough that the last blocks even the threads
/// out (a block of the rim jumps' flights is about 15 items, ~3 s on Rudnya), coarse enough that the counter is not contended.
const PARALLEL_BLOCKS: usize = 256;

impl WalkGrid {
    /// [A318] Keeps the map's walker triangles as the client's controller meets them (its body's [`Body::controller`] caster), for
    /// [`Self::client_step`].
    pub fn with_client_walker(mut self, triangles: &[Triangle]) -> Self {
        let (caster, _) = self.body.controller();
        self.walker = Some(std::sync::Arc::new(crate::controller::Collision::build(triangles).with_caster(caster)));
        self
    }

    /// [A388] Keeps `world` as the client's walker: the game's shapes met by the exe's own cast
    /// ([`crate::controller::Collision::build_exe`]), for [`Self::client_step`] and the reckoning's kept controller.
    pub fn with_client_world(mut self, world: crate::controller::Collision) -> Self {
        self.walker = Some(std::sync::Arc::new(world));
        self
    }

    /// [A334] The grid with the routes' pulled legs walked by the client's walker ([`Self::pull_walk_verdict`]); it needs the
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

    /// [A334] Whether the client's walk tick walks a route's straight leg from floor `a` to floor `b`: the controller pass's walk
    /// test ([`walk_share`]: a standing body from `a`'s centre at the walk speed, with the exe's collision rules) with the grid's
    /// walker ([`PullWalk`]). [`PullWalk::Walks`] without [`Self::with_pull_walk`] or the walker (the switch off); [`PullWalk::Unknown`]
    /// past this thread's budget ([`set_pull_walk_budget`]): the leg is not walked now and nothing is kept, so a later pull walks it.
    /// Before, a leg past the budget read as walked, and a route could keep a leg the client does not walk (level_06 day 4's
    /// (62.03, 33.59, 120.24) -> (62.10, 34.10, 120.62): the bots held forward 4 s and more, the exe's tick gets up at neither the
    /// walk nor the sprint; the budget is 40 ticks a poll for all bots of the match).
    pub fn pull_walk_verdict(&self, a: NodeId, b: NodeId) -> PullWalk {
        let (Some(id), Some(world)) = (self.pull_walk, self.walker.as_deref()) else { return PullWalk::Walks };
        // The answer is the floors' own (the walker and the body are the grid's): kept per thread by the grid's number and the
        // floors, so a leg pulled again (most are: the same ways are planned again and again) costs a look-up.
        let key = (id, a, b);
        if let Some(fails) = PULL_WALKS.with(|c| c.borrow().get(&key).copied()) {
            return if fails { PullWalk::Fails } else { PullWalk::Walks };
        }
        // The walk's ticks at most (as [`walk_share`] counts them), taken from this thread's budget up front.
        let (pa, pb) = (self.position(a), self.position(b));
        let ticks = (Vec3::new(pb.x - pa.x, 0.0, pb.z - pa.z).length() / (WALK_MPS * 0.05)).ceil() as u32 + 2;
        if !take_pull_walk(ticks) {
            return PullWalk::Unknown;
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
        if fails { PullWalk::Fails } else { PullWalk::Walks }
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
    /// walker with its body's settings, and what the step did (its `airborne`: `step_down` 0x7E1B0 found no ground under it);
    /// none (and nothing moved) without [`Self::with_client_walker`].
    pub fn kept_tick(&self, body: &mut Controller, input: TickInput) -> Option<crate::controller::tick::StepNotes> {
        let world = self.walker.as_deref()?;
        let (_, s) = self.body.controller();
        Some(body.tick(world, 0.05, &s, input))
    }

    /// [A388 step 3] The next step of a jump `flying` of a kept body `c` on the grid's walker with its body's settings, heading
    /// `heading` with the air control `air` ([`crate::controller::jump::Flying::step`]). None (nothing moved) without
    /// [`Self::with_client_walker`].
    pub fn kept_fly_step(
        &self,
        c: &mut Controller,
        flying: &mut crate::controller::jump::Flying,
        heading: (f32, f32),
        air: (f32, f32),
    ) -> Option<crate::controller::jump::FlightStep> {
        let world = self.walker.as_deref()?;
        let (_, s) = self.body.controller();
        Some(flying.step(world, &s, c, heading, air))
    }

    /// [A388] Whether the grid keeps the client's walker ([`Self::with_client_walker`], [`Self::with_client_world`]).
    pub fn has_client_walker(&self) -> bool {
        self.walker.is_some()
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
    /// [row 6] `shapes`: the game's shapes ([`Collision::build_exe`]), the world every other pass flies and walks the client's
    /// body on; the walks are checked on them when given, else on the walker triangles with the body's caster (a map with no
    /// shapes). The walker triangles are a primitive's drawn stand-in, not the object the client's controller sweeps: on
    /// Mamayev Kurgan the walk 366489 -> 365709 up the slope at (26.0, 25.5, 91.7) passes the check on them and fails it on the
    /// shapes (best share 0.49 of 0.5), and the exe's own tick does not climb it from the floor's centre or either line (it
    /// slides 0.06-0.37 m down) nor from 5 stuck bots' feet (51 stuck notes there in 1 match).
    pub(super) fn controlled_links(
        &self,
        wide: &mut Wide,
        triangles: &[Triangle],
        shapes: Option<&Collision>,
        memo: &mut Memo,
    ) -> (ControlledStats, Vec<(u32, usize, f32)>) {
        let body = self.body;
        let (caster, s) = body.controller();
        let start = body.walk_start();
        let walker;
        let world: &Collision = match shapes {
            Some(shapes) => shapes,
            None => {
                walker = Collision::build(triangles).with_caster(caster);
                &walker
            }
        };
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
            walk_lines(world, s, start, self.position(l.from), self.position(l.to)).1
        });
        let old: Vec<usize> = bridges.iter().zip(walked).filter(|&(_, ok)| ok).map(|(&i, _)| i).collect();
        // Bridges with no link yet: two cells on, a floor within a step, the cell between with no floor near that height (a
        // crack no face covers, a cell holding only a riser), walked by the controller: a walked off-mesh link.
        wide.off.sort_by_key(|l| (l.from, l.to));
        let floors: Vec<usize> = (0..self.height.len()).collect();
        let (new, over_drop) = self.bridge_candidates(wide, &floors);
        stats.bridges_tried += new.len();
        let walked = parallel(&new, |&(a, b)| walk_lines(world, s, start, self.position(a), self.position(b)).1);
        let new: Vec<(u32, u32)> = new.iter().zip(walked).filter(|&(_, ok)| ok).map(|(&p, _)| p).collect();
        // Only the bridges in a run at least [`BRIDGE_MIN_RUN`] wide are walked: a narrower one is crossed only by a body
        // inside its columns, which a bot steering at a point ahead of it does not keep to.
        let all = Bridges::of(old.iter().map(|&i| (wide.off[i].from, wide.off[i].to)).chain(new.iter().copied()));
        for &i in &old {
            if self.bridge_run(&all, wide.off[i].from, wide.off[i].to) >= BRIDGE_MIN_RUN {
                wide.off[i] = walked_bridge(wide.off[i]);
                stats.bridges_walked += 1;
            } else {
                stats.bridges_narrow += 1;
            }
        }
        for &(a, b) in &new {
            if over_drop.contains(&(a, b)) || self.bridge_run(&all, a, b) >= BRIDGE_MIN_RUN {
                wide.off.push(OffLink { from: a, to: b, kind: LinkKind::GapJump, press: Takeoff::WalkOff, share: Share::UNFLOWN, walk: None, aim: super::gap::Aim::STRAIGHT, way: None });
                stats.bridges_walked += 1;
                stats.bridges_added += 1;
            } else {
                stats.bridges_narrow += 1;
            }
        }
        wide.off.sort_by_key(|l| (l.from, l.to));
        if body.walk_every_link {
            let (stats, failing, held, mut crouch, mut lanes) = self.walk_every_link(wide, world, s, start, stats, memo);
            // The risers with no walking way round stay, marked (the caller marks the cut's kept ones beside them).
            wide.marked = held;
            crouch.sort_unstable();
            wide.crouch = crouch;
            // [walk lane] The links walked only on a side line, with it (a cut one leaves the table when the grid is made).
            lanes.sort_unstable();
            wide.lanes = lanes;
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
            walk_lines(world, s, start, self.position(a as NodeId), self.position(b)).0
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
        memo: &mut Memo,
    ) -> WalkEveryLink {
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
        let walked = self.walk_links_memo(wide, world, s, start, &checks, memo);
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
        // [walk lane] The links whose centre line the client's tick does not walk and a side line it does.
        let lanes: Lanes = checks.iter().zip(&walked).filter_map(|(&(a, d), w)| Some((a as u32, wide.links[a][d], w.lane?))).collect();
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
        (stats, round.into_iter().map(|(f, _)| f).collect(), held, crouch, lanes)
    }

    /// [given-back walks] The Walk links the wall give-back made ([`Body::wall_give_back`]: out of each given-back floor, and
    /// into one from a kept floor) walked as [`Self::walk_every_link`] walks every link: the walk check runs before the
    /// give-back, so its links were never walked. Of those [`Self::walk_checked`] selects, each is walked by [`Self::walk_link`];
    /// a failing one, flat or rising, is taken away where a walking way leads round it ([`Self::walk_way_round`]) and the cut
    /// disconnects no floor from the seeds and spawns ([`WalkGrid::cut_keeping_reach`], the last resorts kept, and with
    /// [`Body::walk_every_link`] the walking ways); the others stay, marked, as the walk check's kept ones do. Vector
    /// Laboratory batteries: the diagonal Walk (-25.99, 16.93, -4.04) -> (-25.74, 16.93, -4.29) is the give-back's (a 30 m box
    /// built with and without it: the floor and the link exist only with it), and the client's body gets 0.066 of the way.
    pub(super) fn walk_given_back(&self, wide: &mut Wide, world: &Collision, seeds: &[Vec3], spawns: &[Vec3], stats: &mut ControlledStats) {
        if wide.given.is_empty() {
            return;
        }
        let (_, s) = self.body.controller();
        let (s, start) = (&s, self.body.walk_start());
        let n = self.height.len();
        let made = |a: usize, d: usize| {
            let b = wide.links[a][d];
            b != NONE && wide.kinds[a][d] == LinkKind::Walk && (wide.given[a] || wide.given[b as usize])
        };
        // The floors at those links' ends, and which have a face no body walks on near them (as `walk_every_link` asks).
        let mut ends = vec![false; n];
        for a in 0..n {
            for d in 0..8 {
                if made(a, d) {
                    ends[a] = true;
                    ends[wide.links[a][d] as usize] = true;
                }
            }
        }
        let floors: Vec<usize> = (0..n).filter(|&a| ends[a]).collect();
        let faced = parallel(&floors, |&a| {
            let feet = self.position(a as NodeId);
            let height = if self.ceiling[a] - self.height[a] >= STANDING_ROOM_M { STAND_HEIGHT_M } else { CROUCH_HEIGHT_M };
            let cap = Capsule::standing(feet, height, STAND_RADIUS_M);
            world
                .nearest(cap, CHECK_WIDE_REACH_M, |f| f.walker == WalkerBehaviour::DontWalk || f.normal.y.abs() < s.idle_max_slope_cos)
                .is_some()
        });
        let mut near = vec![false; n];
        for (&a, f) in floors.iter().zip(faced) {
            near[a] = f;
        }
        let checks: Vec<(usize, usize)> =
            (0..n).flat_map(|a| (0..8).map(move |d| (a, d))).filter(|&(a, d)| made(a, d) && self.walk_checked(a, wide.links[a][d] as usize, &near)).collect();
        stats.given_back_checks = checks.len();
        let walked = parallel(&checks, |&(a, d)| self.walk_link(wide, world, s, start, a, d));
        // [walk lane] Their side-line walks too, as the walk check's.
        wide.lanes.extend(checks.iter().zip(&walked).filter_map(|(&(a, d), w)| Some((a as u32, wide.links[a][d], w.lane?))));
        wide.lanes.sort_unstable();
        let failing: Vec<(u32, usize, f32)> = checks.iter().zip(walked).filter(|(_, w)| !w.passes).map(|(&(a, d), w)| (a as u32, d, w.share)).collect();
        let cut: std::collections::HashSet<(u32, u32)> = failing.iter().map(|&(a, d, _)| (a, wide.links[a as usize][d])).collect();
        let other_way = parallel(&failing, |&(a, d, _)| self.walk_way_round(wide, a, wide.links[a as usize][d], &cut));
        let (round, held): (Vec<_>, Vec<_>) = failing.into_iter().zip(other_way).partition(|&(_, round)| round);
        stats.given_back_no_way_round = held.len();
        let mut marked: Vec<(u32, u32)> = held.into_iter().map(|((a, d, _), _)| (a, wide.links[a as usize][d])).collect();
        let round: Vec<(u32, usize, f32)> = round.into_iter().map(|(f, _)| f).collect();
        let tried: Vec<(u32, u32)> = round.iter().map(|&(a, d, _)| (a, wide.links[a as usize][d])).collect();
        (stats.given_back_dropped, stats.given_back_kept) = self.cut_keeping_reach(wide, round, seeds, spawns, true, self.body.walk_every_link);
        // The kept ones are marked, as the walk check's are: a route may need them, and they are where a body is likely held.
        marked.extend(tried.into_iter().filter(|&(a, b)| (0..DIRS.len()).any(|d| wide.links[a as usize][d] == b && wide.kinds[a as usize][d] == LinkKind::Walk)));
        wide.marked.extend(marked);
        wide.marked.sort_unstable();
        wide.marked.dedup();
    }

    /// [body physics S6c] The walk checks of `checks` (a floor and a direction each, [`Self::walk_link`]) through the memo's site
    /// `walk.link` ([`walk_link_job`]), each result at its check's place. With the memo on, a link's floor outside the window of
    /// its take-off's cell stops the build: the window would not hold every floor its job read.
    fn walk_links_memo(
        &self,
        wide: &Wide,
        world: &Collision,
        s: &Settings,
        start: WalkStart,
        checks: &[(usize, usize)],
        memo: &mut Memo,
    ) -> Vec<LinkWalk> {
        let on = memo.mode() != Mode::Off;
        let windows = on.then(|| Windows::of_floors(self));
        let nx = self.nx.max(1) as i64;
        let within = |a: usize, n: usize| {
            let (ca, cn) = (self.cell_of[a] as i64, self.cell_of[n] as i64);
            (ca % nx - cn % nx).abs() <= WALK_LINK_WINDOW_CELLS && (ca / nx - cn / nx).abs() <= WALK_LINK_WINDOW_CELLS
        };
        let items: Vec<WalkLinkItem> = checks
            .iter()
            .map(|&(a, d)| {
                let (b, before) = Self::link_floors(wide, a, d);
                if on {
                    assert!(
                        within(a, b) && before.is_none_or(|q| within(a, q)),
                        "memo: ERROR site walk.link: a floor of the link {a} -> {b} (before {before:?}) outside its window of \
                         {WALK_LINK_WINDOW_CELLS} cells"
                    );
                }
                let window = windows.as_ref().map_or(0, |w| w.around(self.cell_of[a] as usize, WALK_LINK_WINDOW_CELLS));
                WalkLinkItem { a: a as NodeId, b: b as NodeId, before: before.map(|q| q as NodeId), window }
            })
            .collect();
        let ctx = WalkLinkCtx { window_cells: WALK_LINK_WINDOW_CELLS, grid: self, world, s, start };
        let site = Site { id: "walk.link", source: crate::source::site("walk.link") };
        memo.run(site, self, &ctx, &items, walk_link_key, walk_link_job)
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
        let (b, before) = Self::link_floors(wide, a, d);
        self.walk_link_floors(world, s, start, a, b, before)
    }

    /// [body physics S6c] The floors [`Self::walk_link`] walks for the link out of `a` along `d`: its far floor, and the floor of the
    /// Walk link into `a` along the same way (the chained walk's start), when `a`'s link back along it is that link's reverse.
    fn link_floors(wide: &Wide, a: usize, d: usize) -> (usize, Option<usize>) {
        let b = wide.links[a][d] as usize;
        let back = wide.links[a][d ^ 1];
        let before = (back != NONE
            && wide.kinds[a][d ^ 1] == LinkKind::Walk
            && wide.links[back as usize][d] == a as u32
            && wide.kinds[back as usize][d] == LinkKind::Walk)
            .then_some(back as usize);
        (b, before)
    }

    /// [body physics S6c] [`Self::walk_link`] of the link from `a` to `b`, `before` the chained walk's start: it reads the grid
    /// only through these floors' places and rooms (the memo site `walk.link`'s window).
    fn walk_link_floors(&self, world: &Collision, s: &Settings, start: WalkStart, a: usize, b: usize, before: Option<usize>) -> LinkWalk {
        let stance = match before {
            Some(q) if self.stance(q, a) == Stance::Crouched => Stance::Crouched,
            _ => self.stance(a, b),
        };
        let (pa, pb) = (self.position(a as NodeId), self.position(b as NodeId));
        let side = link_side(pa, pb);
        // [walk lane] Whether the centre line arrived, and the first side line that did.
        let (mut centre_arrived, mut lane) = (false, None);
        // [`Body::walk_ceiling_stalls`]: a refusal under a low ceiling is the client's own stall, not a pass.
        let stalls = self.body.walk_ceiling_stalls;
        let arrives = |w: Walked| matches!(w, Walked::At | Walked::AtEnd(_)) || (w == Walked::Ceiling && !stalls);
        let (mut lines, mut share, mut chain_failed, mut refused, mut crouched) = (0, 0.0f32, false, 0, 0);
        for off in STARTS {
            let (from, to) = (pa + side * off, pb + side * off);
            let yaw = yaw_toward(from, to);
            let mut c = put_at(world, start, stance, from, yaw);
            let mut centre = walk_on(world, s, &mut c, stance, from, to, SPARE_TICKS, self.body.step);
            let mut only_crouched = false;
            // A standing body refused under a low ceiling is walked again crouched; and, [`Body::walk_ceiling_stalls`] (whose
            // crouched walks are recorded, so the route crouches there), one that stops short: a crouched body takes the simple
            // move test (`cc_crouch_move_complex_test` off, 0x7D2A0) and climbs edges a standing body stalls at. On the
            // exe's own tick over captured worlds (exactness S7, `rig_real`), it climbs 11 of 18 live 0.48-0.79 m edges, two of
            // them (0.652 and 0.753 m, Vector Laboratory) that a standing walker cannot, and fails four a walker climbs: an
            // answer per edge, walked here, not a height rule.
            let retry = match centre {
                Walked::Ceiling => true,
                Walked::Short(_) => stalls,
                Walked::At | Walked::AtEnd(_) => false,
            };
            if retry && stance == Stance::Standing {
                let mut c = put_at(world, start, Stance::Crouched, from, yaw);
                let low = walk_on(world, s, &mut c, Stance::Crouched, from, to, SPARE_TICKS, self.body.step);
                only_crouched = matches!(low, Walked::At | Walked::AtEnd(_));
                // A crouched walk that does not arrive leaves the standing answer (its share prices a cut).
                if only_crouched || centre == Walked::Ceiling {
                    centre = low;
                }
            }
            // The stance this line arrived in: the chained walk below walks it so.
            let line = if only_crouched { Stance::Crouched } else { stance };
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
                let mut c = put_at(world, start, line, from_q, yaw_toward(from_q, from));
                // The link before failing is its own check's; from where it leaves the body this one says nothing. [body physics
                // S6b] The body goes on into this link as it left the one before: its blend and speed carried.
                !matches!(walk_on(world, s, &mut c, line, from_q, from, SPARE_TICKS, self.body.step), Walked::At | Walked::AtEnd(_))
                    || arrives(walk_on(world, s, &mut c, line, from, to, SPARE_TICKS, self.body.step))
            });
            if chained {
                match WalkLane::of_offset(off) {
                    None => centre_arrived = true,
                    Some(side_line) => {
                        lane = lane.or(Some(side_line));
                    }
                }
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
        LinkWalk {
            passes: lines >= 2,
            share,
            chain_failed: chain_failed && lines < 2,
            refused,
            crouched,
            lane: lane.filter(|_| !centre_arrived),
        }
    }

    /// The new bridges' candidates out of `floors`: two cells on or more, a floor within a step, the cells between with no floor
    /// near that height, on a side with no link or (with [`Body::crack_bridges`]) a drop into those cells, and no off link of
    /// the pair yet (`wide.off` sorted by take-off then landing). With them, those over a crack the grid crossed by a drop: kept
    /// whatever their run's width (the drop they stand beside was as narrow a way).
    fn bridge_candidates(&self, wide: &Wide, floors: &[usize]) -> (Vec<(u32, u32)>, OverDrop) {
        let body = self.body;
        let mut new: Vec<(u32, u32)> = Vec::new();
        let mut over_drop = OverDrop::new();
        for &a in floors {
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
        (new, over_drop)
    }

    /// [later bridges] The new-bridge search of [`Self::controlled_links`] again, after the passes that cut links once it ran
    /// (the walk-off links, the re-aimed drops, the drop check's cut), for the floors whose side link or off link they changed
    /// since `before`: the same candidates ([`Self::bridge_candidates`]), walked on the same three lines ([`walk_lines`]), kept
    /// by the same run rule ([`Self::bridge_run`] over every walked bridge now), added as walked GapJump WalkOff bridges.
    /// It only adds: a crack the bridge pass skipped for the link it had across it then, and a later pass cut, is walked
    /// over again; a pair that was an off link then (a bridge a later pass tested and took away) is not given back. Batt-m3's spawn exit (level_03 day 2): 99 crack pairs walked on all three lines in a run of 3 or more had no
    /// link at the end; 34 of them a long Walk across the crack when the bridge pass ran (so it tried nothing there), cut by
    /// the re-aimed drops (23) and the drop check's cut (11), and 3 of them a bridge the walk-off links cut. Returns (tried,
    /// added).
    pub(super) fn later_bridges(
        &self,
        wide: &mut Wide,
        triangles: &[Triangle],
        shapes: Option<&Collision>,
        before: &SideLinks,
    ) -> (usize, usize) {
        let n = self.height.len();
        assert_eq!(before.links.len(), n, "the snapshot is of this grid's floors");
        wide.off.sort_by_key(|l| (l.from, l.to));
        let mut changed = vec![false; n];
        for a in 0..n {
            changed[a] = (0..DIRS.len()).any(|d| {
                before.links[a][d] != NONE && (wide.links[a][d] != before.links[a][d] || wide.kinds[a][d] != before.kinds[a][d])
            });
        }
        for &(a, b) in &before.off {
            if wide.off.binary_search_by_key(&(a, b), |l| (l.from, l.to)).is_err() {
                changed[a as usize] = true;
            }
        }
        let floors: Vec<usize> = (0..n).filter(|&a| changed[a]).collect();
        if floors.is_empty() {
            return (0, 0);
        }
        let (mut new, over_drop) = self.bridge_candidates(wide, &floors);
        // A pair that was an off link then and is gone now was taken away by a pass that tested it (the walk-off links' landing
        // test): not given back.
        new.retain(|p| before.off.binary_search(p).is_err());
        if new.is_empty() {
            return (0, 0);
        }
        let body = self.body;
        let (caster, s) = body.controller();
        let start = body.walk_start();
        let walker = shapes.is_none().then(|| Collision::build(triangles).with_caster(caster));
        let world: &Collision = shapes.or(walker.as_ref()).expect("a world");
        let s = &s;
        let walked = parallel(&new, |&(a, b)| walk_lines(world, s, start, self.position(a), self.position(b)).1);
        let tried = new.len();
        let new: Vec<(u32, u32)> = new.iter().zip(walked).filter(|&(_, ok)| ok).map(|(&p, _)| p).collect();
        let cells_apart = |a: u32, b: u32| {
            let (ia, ka) = self.cell_xz(self.cell_of[a as usize] as usize);
            let (ib, kb) = self.cell_xz(self.cell_of[b as usize] as usize);
            (ia - ib).abs().max((ka - kb).abs())
        };
        // Every walked bridge the grid has now, these with them.
        let all = Bridges::of(
            wide.off
                .iter()
                .filter(|l| {
                    l.kind == LinkKind::GapJump
                        && matches!(l.press, Takeoff::WalkOff)
                        && (self.height[l.from as usize] - self.height[l.to as usize]).abs() <= body.step
                        && (2..=BRIDGE_MAX_CELLS as i32 + 1).contains(&cells_apart(l.from, l.to))
                })
                .map(|l| (l.from, l.to))
                .chain(new.iter().copied()),
        );
        let mut added = 0;
        for &(a, b) in &new {
            if over_drop.contains(&(a, b)) || self.bridge_run(&all, a, b) >= BRIDGE_MIN_RUN {
                wide.off.push(OffLink { from: a, to: b, kind: LinkKind::GapJump, press: Takeoff::WalkOff, share: Share::UNFLOWN, walk: None, aim: super::gap::Aim::STRAIGHT, way: None });
                added += 1;
            }
        }
        wide.off.sort_by_key(|l| (l.from, l.to));
        (tried, added)
    }

    /// How many bridges of `all` are side by side with the bridge `a` -> `b`, it included, counted out to [`BRIDGE_MIN_RUN`]
    /// either side: one floor's cell over across the bridge's way each, to a floor in a cell on that column's own line
    /// toward the one over from `b`'s, two cells on or more (the same crack crossed, by a bridge as long or shorter). Before,
    /// only a bridge of the same cells counted, and a longer one beside shorter ones stayed pressed: batt-m3's spawn exit
    /// (level_03 day 2), a one-cell crack at x 59.72 bridged on every row from z 10.205, where the row z 9.955 has no floor at
    /// x 59.47 and its bridge (59.97, 9.955) -> (58.97, 9.955) runs four cells: its run was 1 and the press GapJump stayed,
    /// the route's shortest way out, and the bots' presses there failed; the exe walks it (feed_steps_v2 on rig_real_v3,
    /// 3 of 3 walks over it equal to the port, no air step).
    fn bridge_run(&self, all: &Bridges, a: u32, b: u32) -> usize {
        let (ia, ka) = self.cell_xz(self.cell_of[a as usize] as usize);
        let (ib, kb) = self.cell_xz(self.cell_of[b as usize] as usize);
        let (di, dk) = (ib - ia, kb - ka);
        let (dx, dz) = (di as f32, dk as f32);
        let l = dx.hypot(dz);
        let (sx, sz) = ((-dz / l).round() as i32, (dx / l).round() as i32);
        let steps = di.abs().max(dk.abs());
        let mut run = 1;
        for side in [1, -1] {
            for s in 1..BRIDGE_MIN_RUN as i32 {
                let (ox, oz) = (side * s * sx, side * s * sz);
                let Some(ca) = self.cell_index(ia + ox, ka + oz) else { break };
                // The cells of the column's line, two on to as far as `b`'s column.
                let on_line = |c: usize| {
                    let (i, k) = self.cell_xz(c);
                    (2..=steps).any(|j| {
                        let t = j as f32 / steps as f32;
                        i == ia + ox + (dx * t).round() as i32 && k == ka + oz + (dz * t).round() as i32
                    })
                };
                let found = (self.start[ca]..self.start[ca + 1])
                    .any(|a2| all.from(a2).iter().any(|&b2| on_line(self.cell_of[b2 as usize] as usize)));
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

    /// [row 6] A walk's share counts the way along only on the way's level ([`way_along`]): feet 0.63 of the way along a
    /// 0.36 m riser's line and 0.9 m under its start (the Mamayev Kurgan line's slide) count nothing; at the start's height,
    /// within the arrival band under it, or over it, they count their way along.
    #[test]
    fn a_walk_sliding_down_beside_its_way_has_no_share_of_it() {
        let (from, to) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.36, 0.0));
        assert_eq!(way_along(from, to, Vec3::new(0.63, -0.9, 0.5)), 0.0);
        assert!((way_along(from, to, Vec3::new(0.63, 0.0, 0.5)) - 0.63).abs() < 1e-5);
        assert!((way_along(from, to, Vec3::new(0.4, -0.15, 0.0)) - 0.4).abs() < 1e-5);
        assert!((way_along(from, to, Vec3::new(0.9, 0.36, 0.0)) - 0.9).abs() < 1e-5);
        // A way down: its lower end is the floor of its level.
        assert!((way_along(to, from, Vec3::new(0.5, -0.1, 0.0)) - 0.5).abs() < 1e-5);
    }

    /// [given-back walks] A platform 0.78 m up (z 3-6) with a vertical front face and a ramp up at its west end (x 0-1.5), beside
    /// a wall 2.5 m tall at x 5.2: the front face is a Walk link's rise (under the 0.8 m step) that the client's walk does not
    /// climb (refused from 0.76 m, [`Body::walked_step`]), and the wall's give-back makes such links by the wall, after the walk
    /// check has run. The give-back's walks up the face fail and, with the ramp as their way round, are taken away: no Walk link
    /// up the face is left by the wall (within 1 m of it, where the give-back's floors are) unless it is marked, and the platform
    /// by the wall is still reached. Before, the walk check never walked them and they stayed.
    #[test]
    fn the_wall_give_backs_walks_are_walked() {
        let (wall, top, h) = (5.2, 0.78, 2.5);
        let mut tris = Vec::new();
        floor(&mut tris, 1.5, 0.0, wall, 3.0, 0.0);
        floor(&mut tris, 0.0, 0.0, 1.5, 1.0, 0.0);
        quad(&mut tris, Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, top, 3.0), Vec3::new(1.5, top, 3.0), Vec3::new(1.5, 0.0, 1.0));
        tris.push(Triangle { v: [Vec3::new(1.5, 0.0, 1.0), Vec3::new(1.5, top, 3.0), Vec3::new(1.5, 0.0, 3.0)], walker: WalkerBehaviour::BySlope });
        floor(&mut tris, 0.0, 3.0, wall, 6.0, top);
        quad(&mut tris, Vec3::new(1.5, 0.0, 3.0), Vec3::new(1.5, top, 3.0), Vec3::new(wall, top, 3.0), Vec3::new(wall, 0.0, 3.0));
        quad(&mut tris, Vec3::new(wall, 0.0, 0.0), Vec3::new(wall, h, 0.0), Vec3::new(wall, h, 6.0), Vec3::new(wall, 0.0, 6.0));
        let body = Body {
            erosion: super::super::Erosion::Exact,
            wall_give_back: true,
            give_back_in_cell: true,
            client_controller: true,
            walk_start_clear: true,
            walk_every_link: true,
            exe_collision: true,
            ..Body::client()
        };
        let start = [Vec3::new(3.0, 0.0, 1.0)];
        let area = [0.0, 0.0, 6.0, 6.0];
        let (grid, stats) = WalkGrid::build_kept_with_links(&tris, area, 0.25, body, &start, &start, OffMeshLinks::None);
        println!("GBTEST given back {}; {:?}", stats.wall_given_back, stats.controlled);
        let mut up: Vec<(Vec3, Vec3, bool)> = Vec::new();
        for a in grid.floors_in(area) {
            let p = grid.position(a);
            if p.x < wall - 1.0 || p.y > 0.1 {
                continue;
            }
            for (b, kind) in grid.links(a) {
                let q = grid.position(b);
                if kind == LinkKind::Walk && q.y > top - 0.1 {
                    up.push((p, q, grid.walk_marked(a, b)));
                }
            }
        }
        println!("GBTEST Walk links up the face by the wall: {up:?}");
        assert!(stats.wall_given_back > 0 && stats.controlled.given_back_checks > 0, "{:?}", stats.controlled);
        assert!(stats.controlled.given_back_dropped > 0, "the give-back's walks up the face are taken away: {:?}", stats.controlled);
        assert!(up.iter().all(|&(_, _, marked)| marked), "a Walk link up the face by the wall is left unmarked: {up:?}");
        let by_wall = grid.locate(Vec3::new(wall - 0.4, top, 4.5), 0.3).expect("the platform by the wall");
        assert!(grid.reached(by_wall), "the platform by the wall is reached by the ramp");
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
        // [marked walk] Each is priced as a link no press makes: its length and the tries that fail, each the link again.
        for &(a, b, kind) in &marked {
            let length = (on.position(b) - on.position(a)).length();
            let tries = Share::landed(0).failed_tries();
            assert!(on.link_cost(a, b, kind) >= length * (1.0 + tries) - 1e-3, "{} for {length} m", on.link_cost(a, b, kind));
            assert!(on.link_cost(a, b, kind) <= length * (1.0 + tries) + LinkKind::Walk.penalty() + FALL_EDGE_PENALTY.max(LIP_PENALTY) + 1e-3);
        }
    }

    /// [row 6] The walks are checked on the build's shapes where it has them, not on the walker triangles: a 0.5 m step the
    /// walker triangles hold (walked) is, in the shapes, a 0.785 m vertical one (the client's tick does not climb it): with the
    /// shapes its links up fail and are kept marked as the platform's only way up; with none, none is marked.
    #[test]
    fn the_walks_are_checked_on_the_shapes() {
        let mut walker = Vec::new();
        floor(&mut walker, 0.0, 0.0, 5.0, 6.0, 0.0);
        floor(&mut walker, 5.0, 0.0, 10.0, 6.0, 0.5);
        quad(&mut walker, Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 0.5, 0.0), Vec3::new(5.0, 0.5, 6.0), Vec3::new(5.0, 0.0, 6.0));
        let mut shaped = Vec::new();
        floor(&mut shaped, 0.0, 0.0, 5.0, 6.0, 0.0);
        floor(&mut shaped, 5.0, 0.0, 10.0, 6.0, 0.785);
        quad(&mut shaped, Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 0.785, 0.0), Vec3::new(5.0, 0.785, 6.0), Vec3::new(5.0, 0.0, 6.0));
        let body = Body { client_controller: true, walk_start_clear: true, walk_every_link: true, ..Body::client() };
        let shapes = Collision::build(&shaped).with_caster(body.controller().0);
        let start = [Vec3::new(1.0, 0.0, 3.0)];
        let build = |shapes: Option<&Collision>| {
            WalkGrid::build_kept_with_shapes(&walker, shapes, [0.0, 0.0, 10.0, 6.0], 0.25, body, &start, &start, OffMeshLinks::None)
        };
        let (walked, stats) = build(None);
        assert_eq!(walked.walk_marked_count(), 0, "on the walker triangles the 0.5 m step is walked: {:?}", stats.controlled);
        let (checked, stats) = build(Some(&shapes));
        assert!(checked.walk_marked_count() > 0, "on the shapes the step is 0.785 m: {:?}", stats.controlled);
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

    /// A one-cell crack (x 5.0-5.25, no floor in it) between two level floors 6 m wide, bridged on every row; on the row
    /// z 3.0-3.25 the far floor's first cell (x 5.25-5.5) is missing too, so that row's bridge runs three cells, from
    /// (4.875, 3.125) to (5.625, 3.125), beside the two-cell bridges of the rows next to it. They cross the same crack on its
    /// line, so its run is theirs and it is walked off as they are (batt-m3's spawn exit, level_03 day 2: the row z 9.955).
    #[test]
    fn a_longer_bridge_beside_shorter_ones_over_the_same_crack_is_walked() {
        let mut tris = Vec::new();
        floor(&mut tris, 0.0, 0.0, 5.0, 6.0, 0.0);
        floor(&mut tris, 5.25, 0.0, 10.0, 3.0, 0.0);
        floor(&mut tris, 5.25, 3.25, 10.0, 6.0, 0.0);
        floor(&mut tris, 5.5, 3.0, 10.0, 3.25, 0.0);
        let body = Body { client_controller: true, ..Body::client() };
        let (grid, stats) = WalkGrid::build_kept_with_links(&tris, [0.0, 0.0, 10.0, 6.0], 0.25, body, &[], &[], OffMeshLinks::Gaps);
        let at = |x: f32, z: f32| grid.locate_within(Vec3::new(x, 0.0, z), 0.05, 0.1).expect("a floor");
        let (a, b) = (at(4.875, 3.125), at(5.625, 3.125));
        let link = grid.off_link(a, b);
        assert!(
            link.is_some_and(|l| l.kind == LinkKind::GapJump && l.press == Takeoff::WalkOff),
            "the three-cell bridge: {link:?}, {:?}",
            stats.controlled
        );
        let (a, b) = (at(4.875, 3.375), at(5.375, 3.375));
        assert!(grid.off_link(a, b).is_some_and(|l| l.press == Takeoff::WalkOff), "the row beside it: {:?}", grid.off_link(a, b));
    }

    /// [later bridges] A one-cell crack (x 5.0-5.25, no floor in it) between two level floors 6 m wide, as the bridge pass sees
    /// it on batt-m3's spawn exit (level_03 day 2): each row's floor before the crack has a long Walk over it to the floor
    /// past it, so the pass tries no bridge there; a later pass (the re-aimed drops, the drop check's cut) takes those Walks
    /// away. The search again for the floors that lost them bridges the crack, walked off; with the Walks kept, or nothing
    /// cut, nothing is added.
    #[test]
    fn a_crack_whose_walk_a_later_pass_cuts_is_bridged_again() {
        let mut tris = Vec::new();
        floor(&mut tris, 0.0, 0.0, 5.0, 6.0, 0.0);
        floor(&mut tris, 5.25, 0.0, 10.0, 6.0, 0.0);
        let body = Body { client_controller: true, ..Body::client() };
        let (grid, _) = WalkGrid::build_kept_with_links(&tris, [0.0, 0.0, 10.0, 6.0], 0.25, body, &[], &[], OffMeshLinks::Gaps);
        let at = |x: f32, z: f32| grid.locate_within(Vec3::new(x, 0.0, z), 0.05, 0.1);
        let rows: Vec<(u32, u32)> =
            (2..22).map(|r| 0.125 + 0.25 * r as f32).filter_map(|z| Some((at(4.875, z)?, at(5.375, z)?))).collect();
        assert_eq!(rows.len(), 20, "a floor each side of the crack on every row");
        let n = grid.height.len();
        // The links as the bridge pass saw them: each row's long Walk over the crack (+x, `DIRS[0]`).
        let mut wide = grid.link(&vec![false; n]);
        for &(a, b) in &rows {
            assert_eq!(wide.links[a as usize][0], NONE, "nothing in the crack's cell");
            wide.links[a as usize][0] = b;
            wide.kinds[a as usize][0] = LinkKind::Walk;
        }
        let before = SideLinks::of(&wide);
        // Kept: nothing to bridge again.
        assert_eq!(grid.later_bridges(&mut wide, &tris, None, &before), (0, 0));
        // Cut by a later pass.
        for &(a, _) in &rows {
            wide.links[a as usize][0] = NONE;
        }
        let (tried, added) = grid.later_bridges(&mut wide, &tris, None, &before);
        assert!(tried >= rows.len() && added >= rows.len() - 2, "tried {tried}, added {added}");
        for &(a, b) in &rows[2..rows.len() - 2] {
            let l = wide.off.iter().find(|l| (l.from, l.to) == (a, b));
            assert!(l.is_some_and(|l| l.kind == LinkKind::GapJump && l.press == Takeoff::WalkOff), "row bridge {a} -> {b}: {l:?}");
        }
        // A bridge a later pass took away is not given back.
        let bridged = SideLinks::of(&wide);
        let (a0, b0) = rows[5];
        wide.off.retain(|l| (l.from, l.to) != (a0, b0));
        let (_, readded) = grid.later_bridges(&mut wide, &tris, None, &bridged);
        assert_eq!(readded, 0, "the cut bridge {a0} -> {b0} stays cut");
        // Once bridged, the same snapshot adds nothing more.
        let again = SideLinks::of(&wide);
        assert_eq!(grid.later_bridges(&mut wide, &tris, None, &again), (0, 0));
    }

    /// [S7] The walk check's crouched retry ([`WalkGrid::walk_link`] with [`Body::walk_ceiling_stalls`]) over a 0.785 m vertical
    /// step: the standing body stops short (the exe climbs a vertical face walking up to 0.75 m), the crouched one too (the
    /// port's crouched body, at 3.5 m/s, climbed a vertical face to 0.70 m, not checked in the exe), so the riser is still failing and marked, and no link is recorded
    /// crouched; a 0.65 m step is walked standing and not recorded crouched.
    #[test]
    fn a_step_a_crouched_body_does_not_climb_either_stays_marked() {
        let build = |h: f32| {
            let mut tris = Vec::new();
            floor(&mut tris, 0.0, 0.0, 5.0, 6.0, 0.0);
            floor(&mut tris, 5.0, 0.0, 10.0, 6.0, h);
            quad(&mut tris, Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, h, 0.0), Vec3::new(5.0, h, 6.0), Vec3::new(5.0, 0.0, 6.0));
            let body =
                Body { client_controller: true, walk_start_clear: true, walk_every_link: true, walk_ceiling_stalls: true, ..Body::client() };
            let start = [Vec3::new(1.0, 0.0, 3.0)];
            WalkGrid::build_kept_with_links(&tris, [0.0, 0.0, 10.0, 6.0], 0.25, body, &start, &start, OffMeshLinks::None)
        };
        let (high, _) = build(0.785);
        assert!(high.walk_marked_count() > 0);
        assert_eq!(high.walk_crouched_count(), 0);
        let (low, stats) = build(0.65);
        assert_eq!(low.walk_crouched_count(), 0, "{:?}", stats.controlled);
        assert!(low.reached(low.locate(Vec3::new(8.0, 0.65, 3.0), 0.3).expect("the top")));
    }

    /// [walk lane] batt-m3 (level_03 day_2): the Walk link from floor 421325 (-38.78, 11.161, -7.295) up to 421323 (-39.03,
    /// 11.760, -7.295). On the map's own shapes the walk check's centre line and its z -7.395 line stay under the riser and its
    /// z -7.195 line goes up, as the exe's tick does on each (feed_steps_v2 on rig_real_v3, every line equal to the port): that
    /// line is [`WalkLane::Minus`] of the link, and the check's lane is it.
    #[test]
    #[ignore = "needs SRP_MAPDATA_DIR (the map files with their shapes)"]
    fn a_walk_made_only_from_a_side_line_records_that_line() {
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
            eprintln!("skipping: SRP_MAPDATA_DIR is not set");
            return;
        };
        let map = crate::MapData::from_bytes(&std::fs::read(dir.join("level_03_day_2.mapdata")).unwrap()).unwrap();
        let world = Collision::build_exe(&map).unwrap();
        // The bench's grid body's walk: the exe's collision (its wall normals by rays, the start witness) and the clear start.
        let body = Body { exe_collision: true, start_witness: true, walk_start_clear: true, ..Body::client() };
        let (_, s) = body.controller();
        let (pa, pb) = (Vec3::new(-38.78, 11.161, -7.295), Vec3::new(-39.03, 11.760, -7.295));
        let walk = |(from, to): (Vec3, Vec3)| {
            let mut c = put_at(&world, body.walk_start(), Stance::Standing, from, yaw_toward(from, to));
            walk_on(&world, &s, &mut c, Stance::Standing, from, to, SPARE_TICKS, body.step)
        };
        let up = |w: Walked| matches!(w, Walked::At | Walked::AtEnd(_));
        let centre = walk((pa, pb));
        assert!(!up(centre), "centre: {centre:?}");
        let (minus, plus) = (WalkLane::Minus.line(pa, pb), WalkLane::Plus.line(pa, pb));
        assert!((minus.0.z - -7.195).abs() < 1e-4 && (plus.0.z - -7.395).abs() < 1e-4, "{minus:?} {plus:?}");
        assert!(up(walk(minus)), "z -7.195: {:?}", walk(minus));
        assert!(!up(walk(plus)), "z -7.395: {:?}", walk(plus));
        // The lane the walk check keeps for the link, and its line for the route: its first side line that arrived.
        let lines: Vec<(f32, bool)> = STARTS.iter().map(|&off| (off, up(walk((pa + link_side(pa, pb) * off, pb + link_side(pa, pb) * off))))).collect();
        let lane = if lines[0].1 { None } else { lines.iter().find(|l| l.1).and_then(|l| WalkLane::of_offset(l.0)) };
        assert_eq!(lane, Some(WalkLane::Minus));
    }

    /// [walk lane] A link's lanes are its two side lines of [`STARTS`], across the link; the memo stores a check's lane.
    #[test]
    fn a_links_lanes_are_its_side_lines_and_the_memo_keeps_them() {
        let (a, b) = (Vec3::new(0.0, 1.0, 0.0), Vec3::new(-0.25, 1.6, 0.0));
        assert_eq!(link_side(a, b), Vec3::new(0.0, 0.0, -1.0));
        for (lane, off) in [(WalkLane::Minus, STARTS[1]), (WalkLane::Plus, STARTS[2])] {
            assert_eq!(WalkLane::of_offset(off), Some(lane));
            assert_eq!(WalkLane::from_byte(lane.byte()), Some(lane));
            let (f, t) = lane.line(a, b);
            assert_eq!((f, t), (a + link_side(a, b) * off, b + link_side(a, b) * off));
        }
        assert_eq!(WalkLane::of_offset(STARTS[0]), None);
        assert_eq!(WalkLane::from_byte(0), None);
        let (p, q, r) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 4.0));
        let tris = [
            Triangle { v: [p, q, r], walker: WalkerBehaviour::BySlope },
            Triangle { v: [p, r, Vec3::new(0.0, 0.0, 4.0)], walker: WalkerBehaviour::BySlope },
        ];
        let g = WalkGrid::build(&tris, [0.0, 0.0, 4.0, 4.0], 0.25, Body::default());
        let ids = StableIds::of(&g).expect("stable floor names");
        for lane in [None, Some(WalkLane::Minus), Some(WalkLane::Plus)] {
            let w = LinkWalk { passes: false, share: 1.0, chain_failed: false, refused: 0, crouched: 0, lane };
            let mut out = Codec::writer(&ids);
            w.encode(&mut out);
            let bytes = out.into_bytes();
            let mut input = Codec::reader(&ids, &bytes);
            assert_eq!(LinkWalk::decode(&mut input).expect("decodes").lane, lane);
            assert!(input.done());
        }
    }

    /// [S7] The two live edges a crouched body climbs and a standing walker cannot (exactness's real-memory rig, the exe's own
    /// tick over the captured Vector Laboratory, `notes-s7.md`): the walk check's walk ([`walk_on`]) on the map's own shapes
    /// stops the standing body short and brings the crouched one up, as the exe does.
    #[test]
    #[ignore = "needs SRP_MAPDATA_DIR (the map files with their shapes)"]
    fn the_live_edges_only_a_crouched_body_climbs_are_climbed_crouched() {
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
            eprintln!("skipping: SRP_MAPDATA_DIR is not set");
            return;
        };
        let map = crate::MapData::from_bytes(&std::fs::read(dir.join("level_07_day_4.mapdata")).unwrap()).unwrap();
        let world = Collision::build_exe(&map).unwrap();
        let s = Settings::LIVE;
        // The recorded spot (x, z), the floor and the top's heights, the heading, the start's distance back, the end's ahead.
        for (x, z, floor, top, (ux, uz), back, ahead) in [
            // rec 67803289: 0.652 m (the rig: standing below the top, crouched on it).
            (4.24f32, -11.06f32, 20.761f32, 21.414f32, (-0.759f32, 0.651f32), 1.0f32, 0.5f32),
            // rec 7baa92b5: 0.753 m.
            (-40.91, -27.10, 20.953, 21.635, (-0.984, -0.177), 0.6, 1.0),
        ] {
            let from = Vec3::new(x - ux * back, floor, z - uz * back);
            let to = Vec3::new(x + ux * ahead, top, z + uz * ahead);
            let walk = |stance: Stance| {
                let mut b = put_at(&world, WalkStart::Clear, stance, from, yaw_toward(from, to));
                walk_on(&world, &s, &mut b, stance, from, to, SPARE_TICKS, 0.8)
            };
            assert!(matches!(walk(Stance::Standing), Walked::Short(_)), "({x}, {z}) standing: {:?}", walk(Stance::Standing));
            // [body physics S6b] The crouched body walks up from rest by the client's blend: it may meet the top as its ticks
            // run out ([`Walked::AtEnd`]), an arrival to the walk check as a meeting on the way is.
            let crouched = walk(Stance::Crouched);
            assert!(matches!(crouched, Walked::At | Walked::AtEnd(_)), "({x}, {z}) crouched: {crouched:?}");
        }
    }
}

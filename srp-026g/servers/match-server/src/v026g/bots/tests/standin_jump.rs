use super::*;

/// How long a jump keeps a stand-in body in the air, ms: walking into a
/// floor its floor jumps to ([`jumped_to`]) meanwhile, it gets up on it.
/// GUESS: a bot jumps up its route's ledge 0.8 m short of it
/// (`motor.rs` `jump_key`), 0.25 s at a run; the game's jump is up for
/// about half a second. The stand-in had no jumps: a route up a jump link
/// stood at its foot, and 100 of the 118 held-keys stuck notes of a bench
/// were within a metre of a jump floor (`player_bots_stuck.md` §8.6).
pub(crate) const STAND_IN_AIR_MS: u32 = 600;

/// A jump of the stand-in: how high the feet get over the floor jumped
/// from, and how far ahead a jump with no grid link lands, m.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StandInJump {
    pub(crate) rise_m: f32,
    pub(crate) ahead_m: f32,
}

/// The client's jump standing or walking: the `on_site*` clips' root tops
/// at 0.998 m (PROVEN, the jump study). How far it carries was never
/// measured: GUESS 0.6 m ahead, kept short on purpose.
pub(crate) const STANDING_JUMP: StandInJump = StandInJump {
    rise_m: mapdata::grid::STANDING_JUMP_M,
    ahead_m: 0.6,
};

/// [`STANDING_JUMP`] as the stand-in game jumps it: `SRP_BENCH_JUMP_AHEAD_M`
/// sets how far ahead a jump with no grid link lands (a sensitivity row;
/// the grid lists pocket exits up to 1.0 m across), else the GUESS 0.6 m.
pub(crate) fn standing_jump() -> StandInJump {
    static AHEAD: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    let ahead_m = *AHEAD.get_or_init(|| {
        bench_settings().number("SRP_BENCH_JUMP_AHEAD_M")
            .unwrap_or(STANDING_JUMP.ahead_m)
    });
    StandInJump {
        ahead_m,
        ..STANDING_JUMP
    }
}

/// The client's sprint jump: 1.282-1.350 m by clip (PROVEN, the jump
/// study), played at a press with forward and sprint held at walk speed or
/// more, any weapon, no battery carried (PROVEN,
/// `player_logic_jump_state::initialize` 0x337040). No bot's jump picks it
/// yet. The distance is [`STANDING_JUMP`]'s (GUESS).
pub(crate) const SPRINT_JUMP: StandInJump = StandInJump {
    rise_m: 1.30,
    ahead_m: 0.6,
};

/// Where a stand-in body in the air, walking from `feet` (on `node`) into
/// `to` and stopped, gets up with `jump`:
/// - by the grid's jump link ([`jumped_to`]) when its rise is within the
///   jump: a higher link is not made, as in the client;
/// - else, with no link, on the highest floor (then the nearest) within
///   `ahead_m` ahead on the walk's line and up to the jump's rise: a jump
///   the grid does not plan (out of a pocket, up a ledge a stuck bot hops
///   at).
pub(crate) fn stand_in_jump(
    grid: &mapdata::WalkGrid,
    node: NodeId,
    feet: Vec3,
    to: Vec3,
    jump: StandInJump,
) -> Option<NodeId> {
    let base = grid.position(node).y;
    if let Some(up) = jumped_to(grid, node, feet, to)
        && grid.position(up).y - base <= jump.rise_m
    {
        return Some(up);
    }
    let (dx, dz) = (to.x - feet.x, to.z - feet.z);
    let len = (dx * dx + dz * dz).sqrt();
    if len <= 1e-6 {
        return None;
    }
    let (ux, uz) = (dx / len, dz / len);
    let side = grid.cell_size() * 0.5 + 1e-4;
    let reach = jump.ahead_m;
    let area = [
        feet.x - reach,
        feet.z - reach,
        feet.x + reach,
        feet.z + reach,
    ];
    grid.floors_in(area)
        .filter(|&n| n != node && grid.can_stand(n))
        .filter_map(|n| {
            let at = grid.position(n);
            let (ex, ez) = (at.x - feet.x, at.z - feet.z);
            let ahead = ex * ux + ez * uz;
            let across = (ex * uz - ez * ux).abs();
            let rise = at.y - base;
            (ahead > 0.0 && ahead <= reach && across <= side && rise > 0.0 && rise <= jump.rise_m)
                .then_some((n, at.y, ahead))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.2.total_cmp(&a.2)))
        .map(|(n, _, _)| n)
}

/// The client's sprint lift (A55, J0 8e rows, PROVEN): a body that runs into the face of a top at a sprint (it came to it at
/// [`LIFT_MIN_SPEED_MS`] or more, never from a stand) and is held there with sprint and forward is carried up it after the
/// client's own hold at the face, 0.15-0.3 s.
pub(crate) const LIFT_HOLD_MS: u32 = 300;
/// The speed a body comes to the face at for the lift, m/s: a sprint is 5 m/s; the walk cap is 3.5.
pub(crate) const LIFT_MIN_SPEED_MS: f32 = 3.5;

/// Whether the stand-in lifts (`SRP_BENCH_STAND_IN_LIFT`, on unless `off`).
pub(crate) fn stand_in_lift() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_LIFT").map_or(true, |v| v != "off"))
}

/// Whether the stand-in body is held at a ledge's edge as the client's capsule is (`SRP_BENCH_STAND_IN_EDGE`, on unless `off`): it falls
/// [`body::SUPPORT_M`] past the last floor's centre, not at the lattice's cell boundary.
pub(crate) fn stand_in_edge() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_EDGE").map_or(true, |v| v != "off"))
}

/// Whether the stand-in body past a ledge's edge goes on as the client's capsule does whatever the grid's links say
/// (`SRP_BENCH_STAND_IN_FALL`, on unless `off`; it acts only with [`stand_in_edge`]): [`body::slide_supported`] moves a body
/// past its floor's boundary only toward a drop link of that floor or along a lip's floor, so a body held 0.2-0.4 m past a
/// floor with neither stood there pushing, where the client's capsule walks on while its hemisphere still touches the edge and
/// falls once it does not (`step_down` 0x7E1B0: no contact in the sweep, airborne; PROVEN; the landing straight below the
/// capsule, LIKELY).
pub(crate) fn stand_in_fall() -> bool {
    if let Some(on) = FALL_FORCED.with(std::cell::Cell::get) {
        return on;
    }
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_FALL").map_or(true, |v| v != "off"))
}

thread_local! {
    /// A test's word on [`stand_in_fall`], for its own thread (the tests run one to a thread).
    static FALL_FORCED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// A test puts [`stand_in_fall`] on or off for its own thread (`None`: as the run says): the pinned soaks predate it.
pub(crate) fn force_fall(on: Option<bool>) {
    FALL_FORCED.with(|f| f.set(on));
}

/// What [`past_edge`] did: walked on past the boundary, fell to the floor below, found no floor below.
static EDGE_FALLS: [std::sync::atomic::AtomicU64; 3] = [const { std::sync::atomic::AtomicU64::new(0) }; 3];

pub(crate) fn edge_fall_stats() -> Vec<u64> {
    EDGE_FALLS.iter().map(|c| c.load(std::sync::atomic::Ordering::Relaxed)).collect()
}

/// A stand-in body (on `node`) that the slide stopped short of `to` with no wall in the way (`walls`, the walker's triangles, met
/// above a step: a wall stops the capsule, an edge does not): on to `to` while a floor of its level within [`body::SUPPORT_M`] still
/// holds it, else down on the highest floor under `to` (straight below, within the capsule's width). None at a wall, or with no
/// floor below.
pub(crate) fn past_edge(grid: &mapdata::WalkGrid, walls: &mapdata::RayTree, node: NodeId, feet: Vec3, to: Vec3) -> Option<(NodeId, Vec3)> {
    use std::sync::atomic::Ordering::Relaxed;
    let ahead = feet.distance_xz(to);
    if ahead <= 1e-3 {
        return None;
    }
    let (base, step) = (grid.position(node).y, grid.body().step);
    let dir = Vec3::new((to.x - feet.x) / ahead, 0.0, (to.z - feet.z) / ahead);
    if walls.ray_cast(Vec3::new(feet.x, base + step + 0.1, feet.z), dir, ahead + grid.body().radius).is_some() {
        return None;
    }
    let level = |n: NodeId| (grid.position(n).y - base).abs() <= step;
    let held = std::iter::once(node)
        .chain(grid.links(node).filter(|(_, kind)| !kind.is_leap()).map(|(next, _)| next))
        .filter(|&n| level(n) && grid.position(n).distance_xz(to) <= body::SUPPORT_M)
        .min_by(|&a, &b| grid.position(a).distance_xz(to).total_cmp(&grid.position(b).distance_xz(to)));
    if let Some(n) = held {
        EDGE_FALLS[0].fetch_add(1, Relaxed);
        return Some((n, Vec3::new(to.x, grid.position(n).y, to.z)));
    }
    // Down: the first floor under `to` lower than a step, probing in overlapping height windows (`locate` looks 2.5 m down),
    // within the capsule's width across: the floor at a wall's foot is eroded, and the falling capsule comes down off the face.
    let mut y = base - step - 1.0;
    while y > base - 60.0 {
        if let Some(n) = grid.locate(Vec3::new(to.x, y, to.z), 2.0 * grid.body().radius)
            && grid.position(n).y < base - step
        {
            EDGE_FALLS[1].fetch_add(1, Relaxed);
            return Some((n, grid.position(n)));
        }
        y -= 3.0;
    }
    EDGE_FALLS[2].fetch_add(1, Relaxed);
    None
}

/// Whether a jump's walk before it leaves the floor is held by the walls and the ledge as the walking body is (`SRP_BENCH_STAND_IN_JUMP_WALL`,
/// off unless `on`, and only with `SRP_BENCH_STAND_IN_CORNER=on`): the client's capsule is held at a wall whatever the keys, so a body
/// that presses jump at the wall it is held by does not walk through it in the jump's first moments (A199).
pub(crate) fn stand_in_jump_wall() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN_JUMP_WALL").is_ok_and(|v| v == "on"))
}

/// What a jump's walk on the floor needs of the capsule rule ([`stand_in_jump_wall`], [`super::standin_capsule`]): the walls' sections, whether
/// the rule holds the body (else it only counts the moves it would hold), and the counts of walk ticks and held ones.
pub(crate) struct JumpWall<'a> {
    pub(crate) sections: &'a super::standin_capsule::Sections,
    pub(crate) hold: bool,
    pub(crate) ticks: &'a mut u32,
    pub(crate) held: &'a mut u32,
}

/// The floor a body held at a face (on `node`, at `feet`, walking toward `to`) is lifted up: the jump link ahead
/// ([`jumped_to`]) when its rise is the lift's at most ([`LIFT_MAX_RISE_M`]) and its top deep enough ([`LIFT_TOP_M`]).
pub(crate) fn lift_to(grid: &mapdata::WalkGrid, node: NodeId, feet: Vec3, to: Vec3) -> Option<NodeId> {
    lift_check(grid, node, feet, to).ok()
}

/// What [`lift_check`] found against a lift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoLift {
    /// No jump link ahead of the body toward the point it pushes at.
    NoLink,
    /// The link's rise is over the lift's ([`LIFT_MAX_RISE_M`]).
    TooHigh,
    /// The top is shallower than [`LIFT_TOP_M`].
    TopShort,
}

/// [`lift_to`] with what was against it.
pub(crate) fn lift_check(grid: &mapdata::WalkGrid, node: NodeId, feet: Vec3, to: Vec3) -> Result<NodeId, NoLift> {
    use crate::v026g::bots::motor::{LIFT_MAX_RISE_M, LIFT_TOP_M};
    let up = jumped_to(grid, node, feet, to).ok_or(NoLift::NoLink)?;
    let rise = grid.position(up).y - grid.position(node).y;
    if rise > LIFT_MAX_RISE_M {
        Err(NoLift::TooHigh)
    } else if crate::v026g::bots::path::top_depth(grid, node, up) < LIFT_TOP_M {
        Err(NoLift::TopShort)
    } else {
        Ok(up)
    }
}

/// The rise of the jump link ahead of a body at a face, if there is one.
pub(crate) fn lift_probe(grid: &mapdata::WalkGrid, node: NodeId, feet: Vec3, to: Vec3) -> Option<f32> {
    jumped_to(grid, node, feet, to).map(|up| grid.position(up).y - grid.position(node).y)
}

/// How the stand-in's lifts came out, by the band of the link's rise (up to 0.80, 0.80-0.95, 0.95-1.05, 1.05-1.11, over 1.11,
/// none known) and the outcome ([`LiftOutcome`]).
static LIFT_STATS: [[std::sync::atomic::AtomicU64; 6]; 6] = [const { [const { std::sync::atomic::AtomicU64::new(0) }; 6] }; 6];

#[derive(Clone, Copy, Debug)]
pub(crate) enum LiftOutcome {
    /// Held at the face the client's hold and carried up.
    Made,
    /// The sprint or forward key was let go before the hold was up.
    Released,
    /// Held the hold, and the link's rise is over the lift's.
    TooHigh,
    /// Held the hold, and the top is too shallow.
    TopShort,
    /// Held the hold, and no jump link lay ahead of the point it pushed at.
    NoLink,
    /// Came to the face of a jump link under the sprint speed the client asks.
    SlowContact,
}

pub(crate) fn lift_count(rise: Option<f32>, outcome: LiftOutcome) {
    let band = match rise {
        None => 5,
        Some(r) if r <= 0.8 => 0,
        Some(r) if r <= 0.95 => 1,
        Some(r) if r <= 1.05 => 2,
        Some(r) if r <= 1.11 => 3,
        Some(_) => 4,
    };
    LIFT_STATS[band][outcome as usize].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// The stand-in lift outcomes: `[band][outcome]`, flattened.
pub(crate) fn lift_stats() -> Vec<u64> {
    LIFT_STATS.iter().flatten().map(|c| c.load(std::sync::atomic::Ordering::Relaxed)).collect()
}

/// A stand-in world: a floor at 0 and a ledge `rise` m high from x 10 on,
/// its grid built for `body`.
pub(crate) fn ledge_grid(rise: f32, body: mapdata::Body) -> mapdata::WalkGrid {
    use super::body::tests::{floor, wall_x};
    let mut triangles: Vec<mapdata::Triangle> = floor(0.0, 0.0, 20.0, 10.0, 0.0).to_vec();
    triangles.extend(floor(10.0, 0.0, 20.0, 10.0, rise));
    triangles.extend(wall_x(10.0, 0.0, 10.0).map(|mut t| {
        t.v.iter_mut().for_each(|v| v.y = v.y.min(rise));
        t
    }));
    mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, body)
}

/// Where a stand-in body walking east from x 8 at 0.2 m a tick ends after
/// 40 ticks: in the air all the way with `jump` (one jump), or never.
pub(crate) fn walk_into_ledge(grid: &mapdata::WalkGrid, jump: Option<StandInJump>) -> Vec3 {
    let mut node = grid.locate(Vec3::new(8.0, 0.0, 5.0), 0.5).unwrap();
    let mut feet = grid.position(node);
    let mut jumped = false;
    for _ in 0..40 {
        let to = feet + Vec3::new(0.2, 0.0, 0.0);
        (node, feet, _) = slide(grid, node, feet, to);
        if let Some(jump) = jump
            && !jumped
            && feet.distance_xz(to) > 1e-3
            && let Some(up) = stand_in_jump(grid, node, feet, to, jump)
        {
            node = up;
            feet = grid.position(up);
            jumped = true;
        }
    }
    feet
}

/// A stand-in body walking into a 0.9 m ledge stops at its foot; in the
/// air it gets up on it, by the grid's jump link.
#[test]
pub(crate) fn a_stand_in_body_in_the_air_gets_up_a_ledge() {
    let grid = ledge_grid(0.9, mapdata::Body::default());
    let stopped = walk_into_ledge(&grid, None);
    assert!(stopped.x < 10.0 && stopped.y.abs() < 0.1, "{stopped:?}");
    let up = walk_into_ledge(&grid, Some(STANDING_JUMP));
    assert!(up.x > 12.0 && (up.y - 0.9).abs() < 0.1, "{up:?}");
}

/// A body held at the face of a 0.9 m top with a deep floor on it is lifted up it by the client's sprint lift; a top
/// above the lift's ceiling (1.2 m) is not lifted.
#[test]
pub(crate) fn a_stand_in_lift_takes_a_held_body_up_a_low_top_only() {
    let at_face = |grid: &mapdata::WalkGrid| {
        let node = grid.locate(Vec3::new(9.75, 0.0, 5.0), 0.3).unwrap();
        let feet = grid.position(node);
        (node, feet, feet + Vec3::new(0.25, 0.0, 0.0))
    };
    let low = ledge_grid(0.9, mapdata::Body::default());
    let (node, feet, to) = at_face(&low);
    let up = lift_to(&low, node, feet, to).expect("a 0.9 m top is lifted");
    assert!((low.position(up).y - 0.9).abs() < 0.05, "{:?}", low.position(up));
    let high = ledge_grid(1.2, mapdata::Body::client());
    let (node, feet, to) = at_face(&high);
    assert!(lift_to(&high, node, feet, to).is_none(), "a 1.2 m top is not");
}

/// A grid jump link higher than the client's jump (1.03 m, linked under
/// the grid's 1.05 m) is not made: the body stays at the ledge's foot.
#[test]
pub(crate) fn a_stand_in_jump_does_not_make_a_link_over_the_clients_height() {
    let grid = ledge_grid(1.03, mapdata::Body::default());
    let stayed = walk_into_ledge(&grid, Some(STANDING_JUMP));
    assert!(stayed.x < 10.0 && stayed.y.abs() < 0.1, "{stayed:?}");
}

/// With no grid link (a grid built for a 0.5 m jump), the jump still gets
/// up a 0.9 m ledge; not up a 1.2 m one, which only a sprint jump clears.
#[test]
pub(crate) fn a_stand_in_jump_gets_up_a_ledge_with_no_link_within_its_height() {
    let low = mapdata::Body {
        max_jump: 0.5,
        ..mapdata::Body::default()
    };
    let grid = ledge_grid(0.9, low);
    let up = walk_into_ledge(&grid, Some(STANDING_JUMP));
    assert!(up.x > 10.0 && (up.y - 0.9).abs() < 0.1, "{up:?}");
    let grid = ledge_grid(1.2, low);
    let stayed = walk_into_ledge(&grid, Some(STANDING_JUMP));
    assert!(stayed.x < 10.0 && stayed.y.abs() < 0.1, "{stayed:?}");
    let sprint = walk_into_ledge(&grid, Some(SPRINT_JUMP));
    assert!(
        sprint.x > 10.0 && (sprint.y - 1.2).abs() < 0.1,
        "{sprint:?}"
    );
}

/// Whether the stand-in moves bodies as the client does (the default): no
/// step up over the client's 0.6 m, and a jump that follows the keys held
/// at its press along the game's jump clip ([`Flight`]).
/// `SRP_BENCH_STAND_IN=old` brings back the old stand-in (any step the
/// grid walks, a jump that lifts the body up any floor in reach,
/// [`stand_in_jump`]), only to compare with benches of older bases.
pub(crate) fn honest_stand_in() -> bool {
    static HONEST: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HONEST.get_or_init(|| bench_settings().var("SRP_BENCH_STAND_IN").map_or(true, |v| v != "old"))
}

/// A jump clip's root over the floor it leaves, every 2 of its 24 frames
/// (PROVEN: the stock clips' root curves, private sprint-jump-re
/// `work/jumpcurves.txt` and FINDINGS §3-5): its length, when the body
/// leaves the floor (the clip's `jump` event, frame 6), its rise and how
/// far ahead it has gone, m.
#[derive(Debug)]
pub(crate) struct Clip {
    pub(crate) ms: f32,
    pub(crate) flag_ms: f32,
    pub(crate) rise: [f32; 13],
    pub(crate) ahead: [f32; 13],
}

impl Clip {
    /// How far ahead and how high the root is `ms` into the clip (linear
    /// between the samples).
    pub(crate) fn at(&self, ms: f32) -> (f32, f32) {
        let f = (ms / self.ms).clamp(0.0, 1.0) * 12.0;
        let i = (f.floor() as usize).min(11);
        let t = f - i as f32;
        let lerp = |c: &[f32; 13]| c[i] + (c[i + 1] - c[i]) * t;
        (lerp(&self.ahead), lerp(&self.rise))
    }

    /// When the root is at its highest, ms into the clip.
    pub(crate) fn apex_ms(&self) -> f32 {
        let top = (0..13)
            .max_by(|&a, &b| self.rise[a].total_cmp(&self.rise[b]))
            .unwrap_or(0);
        self.ms * top as f32 / 12.0
    }
}

/// The short jump (`on_site_fwd`, codes 5-12): 0.80 s, 1.0 m up at 1.78 m,
/// 2.67 m forward, even from a standstill. With no movement key the
/// body jumps in place (`on_site`, the same rise). The back and side clips
/// were not measured: GUESS, the forward clip's travel along their way.
pub(crate) static SHORT_CLIP: Clip = Clip {
    ms: 800.0,
    flag_ms: 200.0,
    rise: [
        0.0, 0.0, 0.0, 0.0, 0.36, 0.64, 0.84, 0.96, 1.00, 0.94, 0.77, 0.46, 0.0,
    ],
    ahead: [
        0.0, 0.22, 0.44, 0.67, 0.89, 1.11, 1.33, 1.56, 1.78, 2.00, 2.22, 2.44, 2.67,
    ],
};

/// The sprint jump (`sprint_from_left_leg`, codes 2-4), played at the
/// sprint time scale (0.88 s): 1.28 m up, 4.42 m forward; the right leg's
/// tops at 1.35 m (the left one taken: the bot cannot see the leg). Its
/// diagonals carry 3.12 m on each axis, 4.41 m along the way.
pub(crate) static SPRINT_CLIP: Clip = Clip {
    ms: 880.0,
    flag_ms: 220.0,
    rise: [
        0.0, 0.0, 0.0, 0.0, 0.57, 1.08, 1.24, 1.28, 1.23, 1.01, 0.70, 0.35, 0.0,
    ],
    ahead: [
        0.0, 0.38, 0.76, 1.15, 1.54, 1.92, 2.29, 2.65, 3.01, 3.36, 3.71, 4.06, 4.42,
    ],
};

/// The grid builds its gap links (`mapdata::grid::gap`) by flying the same
/// clip: the numbers must be the same.
#[test]
pub(crate) fn the_grids_gap_links_fly_this_short_clip() {
    assert_eq!(SHORT_CLIP.ahead, mapdata::grid::gap::SHORT_JUMP_AHEAD);
    assert_eq!(SHORT_CLIP.rise, mapdata::grid::gap::SHORT_JUMP_RISE);
}

/// Before the sprint clip, the prepare phase finishes the run's step: 0 to
/// 0.15 s and 0 to 1.64 m, by the gait's phase at the press (PROVEN; the
/// phase is drawn uniform, LIKELY).
pub(crate) const SPRINT_PREPARE_MS: f32 = 150.0;

pub(crate) const SPRINT_PREPARE_M: f32 = 1.64;

/// The walk cap, m/s: the body's speed at the press must be at least this
/// for a sprint jump, and a body on the ground in a jump moves at most
/// this fast (the root runs ahead; the body takes the difference when it
/// leaves the floor), PROVEN (FINDINGS §1.2, §3.2).
pub(crate) const WALK_CAP: f32 = 3.5;

/// A body falling after its clip, m/s².
pub(crate) const GRAVITY: f32 = 9.81;

/// The heights over the feet at which a face holds a body in the air
/// (rays on the walker mesh), m: its feet (a little over, so a floor's
/// own bumps do not), its middle and its head; a body clears an edge once
/// its feet are over it (FINDINGS §5, the capsule rule). The body's
/// radius is the grid's.
pub(crate) const AIR_RAYS: [f32; 3] = [0.1, 0.9, 1.7];

/// A jump under way in the honest stand-in ([`honest_stand_in`]): the
/// body follows the clip's root from where it pressed, along the way of
/// the keys held at the press; on the floor before the clip's jump event
/// (at the walk cap), in the air after it, held by any face in its way
/// (it rises with the clip and catches up once clear: the chase rule,
/// FINDINGS §5). It lands on the highest floor under it once on the way
/// down, or falls after the clip. No air control, no stamina (GUESS: the
/// air control's scale was not read).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Flight {
    /// The press, ms; the feet then; the ground way (unit, or none).
    pub(crate) at: u32,
    pub(crate) from: Vec3,
    pub(crate) dir: [f32; 2],
    pub(crate) clip: &'static Clip,
    /// The sprint jump's prepare phase: its length (ms) and travel (m).
    pub(crate) prepare: (f32, f32),
    /// The floor's height when the body left it.
    pub(crate) base: Option<f32>,
    /// After the clip, falling at this speed, m/s.
    pub(crate) fall: Option<f32>,
}

impl Flight {
    /// The root `ms` after the press: how far ahead, how high, whether
    /// the body has left the floor, and whether the clip is over.
    pub(crate) fn root(&self, ms: f32) -> (f32, f32, bool, bool) {
        let (pm, pd) = self.prepare;
        if ms < pm {
            return (pd * ms / pm, 0.0, false, false);
        }
        let c = ms - pm;
        let (ahead, rise) = self.clip.at(c);
        (pd + ahead, rise, c >= self.clip.flag_ms, c >= self.clip.ms)
    }

    pub(crate) fn descending(&self, ms: f32) -> bool {
        ms - self.prepare.0 >= self.clip.apex_ms()
    }

    pub(crate) fn sprint(&self) -> bool {
        std::ptr::eq(self.clip, &SPRINT_CLIP)
    }
}

/// The jump the keys held at a press make (FINDINGS §4, the codes): none,
/// in place; forward, forward-right or forward-left the sprint jump when
/// sprint is held with none of back, fire, crouch or aim, the body moves
/// at the walk cap or more and may sprint (`can_sprint`: no battery, no
/// stagger), else the short jump; any other way the short jump that way.
/// The clip and the ground way, from the body's yaw.
pub(crate) fn jump_of(keys: u32, yaw: f32, speed: f32, may_sprint: bool) -> (&'static Clip, [f32; 2]) {
    let held = |bit| f32::from(u8::from(keys & bit != 0));
    let ahead = held(key::FORWARD) - held(key::BACK);
    let left = held(key::LEFT) - held(key::RIGHT);
    if ahead == 0.0 && left == 0.0 {
        return (&SHORT_CLIP, [0.0, 0.0]);
    }
    let len = ahead.hypot(left);
    let dir = to_world([ahead / len, left / len], yaw);
    let sprint = ahead > 0.0
        && keys & key::SPRINT != 0
        && keys & (key::BACK | key::FIRE | key::CROUCH | key::AIM) == 0
        && speed >= WALK_CAP
        && may_sprint;
    (if sprint { &SPRINT_CLIP } else { &SHORT_CLIP }, dir)
}

/// The floor a body in the air at `at` comes down on: the highest whose
/// cell holds it at or below `top`; else (its middle over the ground a
/// wall's or an edge's erosion leaves out of the grid) the highest within
/// the body's radius, the nearest of them. Its floor, and where the feet
/// stand on it.
pub(crate) fn air_floor(grid: &mapdata::WalkGrid, at: Vec3, top: f32) -> Option<(NodeId, Vec3)> {
    let half = grid.cell_size() * 0.5 + 1e-4;
    let box_of = |r: f32| [at.x - r, at.z - r, at.x + r, at.z + r];
    let under = grid
        .floors_in(box_of(half))
        .filter(|&n| {
            let p = grid.position(n);
            (p.x - at.x).abs() <= half && (p.z - at.z).abs() <= half && p.y <= top
        })
        .max_by(|&a, &b| grid.position(a).y.total_cmp(&grid.position(b).y));
    if let Some(n) = under {
        return Some((n, Vec3::new(at.x, grid.position(n).y, at.z)));
    }
    let r = grid.body().radius;
    grid.floors_in(box_of(r))
        .filter(|&n| {
            let p = grid.position(n);
            p.distance_xz(at) <= r && p.y <= top
        })
        .max_by(|&a, &b| {
            let (pa, pb) = (grid.position(a), grid.position(b));
            pa.y.total_cmp(&pb.y)
                .then(pb.distance_xz(at).total_cmp(&pa.distance_xz(at)))
        })
        .map(|n| (n, grid.position(n)))
}

/// Where a body in the air at `feet` (its feet at `y`) gets moving toward
/// `(x, z)`: all the way, else along one axis (along a face), else
/// nowhere; a move is held when a ray at one of [`AIR_RAYS`] meets the
/// walker mesh within the move and the body's radius.
pub(crate) fn air_move(walls: &mapdata::RayTree, radius: f32, feet: Vec3, to: (f32, f32), y: f32) -> (f32, f32) {
    let held = |from: (f32, f32), to: (f32, f32)| {
        let (dx, dz) = (to.0 - from.0, to.1 - from.1);
        let len = dx.hypot(dz);
        if len < 1e-6 {
            return false;
        }
        let dir = Vec3::new(dx / len, 0.0, dz / len);
        AIR_RAYS.iter().any(|&h| {
            walls
                .ray_cast(Vec3::new(from.0, y + h, from.1), dir, len + radius)
                .is_some()
        })
    };
    let from = (feet.x, feet.z);
    [to, (to.0, from.1), (from.0, to.1)]
        .into_iter()
        .find(|&t| t != from && !held(from, t))
        .unwrap_or(from)
}

/// How a jump ended: who pressed it and when, where the body came down,
/// and whether it was a sprint jump.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Landing {
    pub(crate) place: Place,
    pub(crate) at: u32,
    pub(crate) to: Vec3,
    pub(crate) sprint: bool,
    /// The vertical speed of the free fall at the landing tick, m/s (`Flight::fall`; none: the clip ended on the floor). The client's fall damage
    /// is a function of it alone (`docs/notes/fall_damage.md`); the bench does not apply it ([`fall_damage`] only counts).
    pub(crate) fall_v: Option<f32>,
}

/// The client's fall damage at a landing speed `v` m/s with `fatal_damage` 1.0 (the resources.db patch): none up to `harmless_height` 5 m
/// (9.9 m/s), `(v - 9.9) / (14 - 9.9)` up to `fatal_height` 10 m (14 m/s), 1.0 beyond (PROVEN, `docs/notes/fall_damage.md`).
pub(crate) fn fall_damage(v: f32) -> f32 {
    ((v - HARMLESS_V) / (FATAL_V - HARMLESS_V)).clamp(0.0, 1.0)
}

/// `sqrt(2 g h)` for `harmless_height` 5 m and `fatal_height` 10 m at the client's gravity 9.8.
const HARMLESS_V: f32 = 9.899_495;
const FATAL_V: f32 = 14.0;
/// A fall of this damage or more kills a body at full health (about 0.6, `fall_damage.md`).
pub(crate) const LETHAL_FALL_DAMAGE: f32 = 0.6;

/// One tick of `body`'s jump ([`Flight`]) at `tick`; the landing when it
/// came down.
///
/// `wall` is the capsule rule for the walk before the jump's first airborne moment ([`stand_in_jump_wall`]).
pub(crate) fn fly(
    grid: &mapdata::WalkGrid,
    walls: &mapdata::RayTree,
    mut wall: Option<JumpWall>,
    body: &mut Body,
    place: Place,
    tick: u32,
) -> Option<Landing> {
    let mut f = body.flight?;
    let Some(node) = body.node else {
        body.flight = None;
        return None;
    };
    let ms = tick.saturating_sub(f.at) as f32;
    let landed = |body: &mut Body, f: &Flight, n: NodeId, feet: Vec3, fall_v: Option<f32>| {
        body.node = Some(n);
        body.feet = feet;
        body.flight = None;
        Some(Landing {
            place,
            at: f.at,
            to: feet,
            sprint: f.sprint(),
            fall_v,
        })
    };
    if let Some(v) = f.fall {
        let v = v + GRAVITY * TICK_S;
        let y = body.feet.y - v * TICK_S;
        match air_floor(grid, body.feet, body.feet.y + 0.05) {
            Some((n, feet)) if feet.y >= y => return landed(body, &f, n, feet, Some(v)),
            Some(_) => {
                body.feet.y = y;
                f.fall = Some(v);
                body.flight = Some(f);
                return None;
            }
            // Nothing under it at all: back on the floor it left (not seen
            // on a bench; kept from falling for ever).
            None => {
                let feet = grid.position(node);
                return landed(body, &f, node, feet, Some(v));
            }
        }
    }
    let (ahead, rise, airborne, over) = f.root(ms);
    let target = (f.from.x + f.dir[0] * ahead, f.from.z + f.dir[1] * ahead);
    if !airborne {
        // On the floor before the jump event: toward the root at the walk
        // cap at most, no step over the client's.
        let (dx, dz) = (target.0 - body.feet.x, target.1 - body.feet.z);
        let len = dx.hypot(dz);
        if len > 1e-6 {
            let k = (WALK_CAP * TICK_S / len).min(1.0);
            let to = Vec3::new(body.feet.x + dx * k, body.feet.y, body.feet.z + dz * k);
            let mut target = to;
            if let Some(wall) = wall.as_mut() {
                let mut segs = Vec::new();
                let mv = (to.x - body.feet.x, to.z - body.feet.z);
                let reach = mv.0.hypot(mv.1) + super::standin_capsule::RADIUS_M + 0.6;
                wall.sections.near((body.feet.x, body.feet.z), body.feet.y, body.keys & key::CROUCH != 0, reach, &mut segs);
                let held = super::standin_capsule::client_move(&segs, (body.feet.x, body.feet.z), mv, super::standin_capsule::RADIUS_M);
                *wall.ticks += 1;
                if (held.to.0 - to.x).hypot(held.to.1 - to.z) > 0.01 {
                    *wall.held += 1;
                    if wall.hold {
                        target = Vec3::new(held.to.0, to.y, held.to.1);
                    }
                }
            }
            let (n, feet, _) = if wall.as_ref().is_some_and(|w| w.hold) && stand_in_edge() {
                super::body::slide_supported(grid, node, body.feet, target, grid.body().step, super::body::SUPPORT_M)
            } else {
                super::body::slide_capped(grid, node, body.feet, target, grid.body().step)
            };
            body.node = Some(n);
            body.feet = feet;
        }
        body.flight = Some(f);
        return None;
    }
    let base = *f.base.get_or_insert(body.feet.y);
    let y = base + rise;
    let was = body.feet.y;
    let (mut x, mut z) = air_move(walls, grid.body().radius, body.feet, target, y);
    // The capsule rule in the air too (A199): the root's step of the first airborne tick is 0.75 m, which the three rays above can pass
    // through a thin wall; the client's capsule is swept along it.
    if let Some(w) = wall.as_ref().filter(|w| w.hold) {
        let mut segs = Vec::new();
        let mv = (x - body.feet.x, z - body.feet.z);
        let reach = mv.0.hypot(mv.1) + super::standin_capsule::RADIUS_M + 0.6;
        // Cut at the floor it left: the sections' heights are above a body standing, and the capsule has the whole of its length in the air.
        w.sections.near((body.feet.x, body.feet.z), base.min(y), false, reach, &mut segs);
        let held = super::standin_capsule::client_move(&segs, (body.feet.x, body.feet.z), mv, super::standin_capsule::RADIUS_M);
        (x, z) = held.to;
    }
    body.feet = Vec3::new(x, y, z);
    if (f.descending(ms) || over)
        && let Some((n, feet)) = air_floor(grid, body.feet, was.max(y) + 0.05)
        && feet.y >= y - 1e-4
    {
        return landed(body, &f, n, feet, None);
    }
    if over {
        f.fall = Some(0.0);
    }
    body.flight = Some(f);
    None
}

/// A press's draw in [0, 1) (the sprint jump's prepare phase): a hash, so
/// the game's draws are not moved.
pub(crate) fn press_draw(tick: u32, place: Place) -> f32 {
    let h = (tick ^ (place as u32).wrapping_mul(0x9E37_79B9)).wrapping_mul(0x85EB_CA6B);
    let h = h ^ (h >> 15);
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// The triangles of [`ledge_grid`]'s world (with `walker` on the ledge's
/// top), for the walker mesh's rays.
pub(crate) fn ledge_triangles(rise: f32, walker: mapdata::WalkerBehaviour) -> Vec<mapdata::Triangle> {
    use super::body::tests::{floor, wall_x};
    let mut triangles: Vec<mapdata::Triangle> = floor(0.0, 0.0, 20.0, 10.0, 0.0).to_vec();
    triangles.extend(floor(10.0, 0.0, 20.0, 10.0, rise).map(|mut t| {
        t.walker = walker;
        t
    }));
    triangles.extend(wall_x(10.0, 0.0, 10.0).map(|mut t| {
        t.v.iter_mut().for_each(|v| v.y = v.y.min(rise));
        t.walker = walker;
        t
    }));
    triangles
}

/// A stand-in body of the honest stand-in at `at` on `grid`, facing east.
pub(crate) fn honest_body(grid: &mapdata::WalkGrid, at: Vec3) -> Body {
    let game = Game::new(soak_world(), &[0], 1, None, None);
    let mut body = game.bodies[0].clone();
    let node = grid.locate(at, 0.5).expect("a floor there");
    body.node = Some(node);
    body.feet = Vec3::new(at.x, grid.position(node).y, at.z);
    body.yaw = -std::f32::consts::FRAC_PI_2;
    body
}

/// `body` jumps with `keys` held at `speed` (m/s, the last tick's): where
/// it comes down, and the highest its feet got over the floor it left.
pub(crate) fn honest_jump(
    grid: &mapdata::WalkGrid,
    walls: &mapdata::RayTree,
    body: Body,
    keys: u32,
    speed: f32,
) -> (Vec3, f32) {
    honest_jump_in(grid, walls, None, body, keys, speed)
}

/// [`honest_jump`] with the walk before the jump held by the capsule rule (`wall`: its sections).
pub(crate) fn honest_jump_in(
    grid: &mapdata::WalkGrid,
    walls: &mapdata::RayTree,
    wall: Option<&super::standin_capsule::Sections>,
    mut body: Body,
    keys: u32,
    speed: f32,
) -> (Vec3, f32) {
    let start = 1_000;
    let (clip, dir) = jump_of(keys, body.yaw, speed, true);
    let u = press_draw(start, 0);
    let prepare = if std::ptr::eq(clip, &SPRINT_CLIP) {
        (SPRINT_PREPARE_MS * u, SPRINT_PREPARE_M * u)
    } else {
        (0.0, 0.0)
    };
    let base = body.feet.y;
    body.flight = Some(Flight {
        at: start,
        from: body.feet,
        dir,
        clip,
        prepare,
        base: None,
        fall: None,
    });
    let mut top = 0.0f32;
    for k in 1..=80 {
        let (mut ticks, mut held) = (0, 0);
        let wall = wall.map(|sections| JumpWall { sections, hold: true, ticks: &mut ticks, held: &mut held });
        let landing = fly(grid, walls, wall, &mut body, 0, start + k * TICK_MS);
        top = top.max(body.feet.y - base);
        if let Some(landing) = landing {
            assert_eq!(landing.to, body.feet);
            return (body.feet, top);
        }
    }
    panic!("no landing: {:?}", body.feet);
}

/// The honest stand-in's jumps against the game's clips (sprint-jump-re
/// `work/jumpcurves.txt`): forward from a standstill 2.67 m and 1.0 m
/// up; in place 1.0 m up where it stood; forward with sprint at a run
/// 4.42 m (and the prepare phase's 0-1.64 m) and 1.28 m up; forward with
/// sprint too slow (under the walk cap) is the short jump.
#[test]
pub(crate) fn an_honest_jump_follows_the_games_clips() {
    let triangles = ledge_triangles(0.9, mapdata::WalkerBehaviour::BySlope);
    let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default());
    let walls = mapdata::RayTree::build(&triangles);
    let from = Vec3::new(2.0, 0.0, 5.0);
    let body = honest_body(&grid, from);
    let (to, top) = honest_jump(&grid, &walls, body.clone(), key::FORWARD | key::JUMP, 0.0);
    assert!((to.x - from.x - 2.67).abs() < 0.15 && to.y.abs() < 0.05, "{to:?}");
    assert!((top - 1.0).abs() < 0.03, "{top}");
    let (to, top) = honest_jump(&grid, &walls, body.clone(), key::JUMP, 0.0);
    assert!(to.distance_xz(from) < 0.05 && (top - 1.0).abs() < 0.03, "{to:?} {top}");
    let run = key::FORWARD | key::SPRINT | key::JUMP;
    let (to, top) = honest_jump(&grid, &walls, body.clone(), run, 5.0);
    let ahead = to.x - from.x;
    assert!((4.3..=4.42 + 1.64 + 0.15).contains(&ahead), "{to:?}");
    assert!((top - 1.28).abs() < 0.03, "{top}");
    let (to, top) = honest_jump(&grid, &walls, body.clone(), run, 3.0);
    assert!((to.x - from.x - 2.67).abs() < 0.15 && (top - 1.0).abs() < 0.03, "{to:?} {top}");
    // Back and to the side: that way.
    let (to, _) = honest_jump(&grid, &walls, body, key::LEFT | key::JUMP, 0.0);
    assert!((to.z - from.z - 2.67).abs() < 0.15 || (from.z - to.z - 2.67).abs() < 0.15, "{to:?}");
}

/// From a standstill at its face, a forward jump gets up a 0.9 m ledge
/// (held by the face, it rises with the clip and goes on once its feet are
/// over the edge); not up a 1.2 m one, which the sprint jump at a run
/// makes; never up one a press with no movement key meets.
#[test]
pub(crate) fn an_honest_jump_gets_up_what_its_clip_clears() {
    for (rise, standing, sprint) in [(0.9, true, true), (1.2, false, true), (1.45, false, false)] {
        let triangles = ledge_triangles(rise, mapdata::WalkerBehaviour::BySlope);
        let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default());
        let walls = mapdata::RayTree::build(&triangles);
        let at_face = honest_body(&grid, Vec3::new(9.5, 0.0, 5.0));
        let (to, _) = honest_jump(&grid, &walls, at_face.clone(), key::FORWARD | key::JUMP, 0.0);
        assert_eq!((to.y - rise).abs() < 0.05, standing, "{rise}: {to:?}");
        assert!(standing || (to.y.abs() < 0.05 && to.x < 10.0), "{rise}: {to:?}");
        let run_up = honest_body(&grid, Vec3::new(7.5, 0.0, 5.0));
        let keys = key::FORWARD | key::SPRINT | key::JUMP;
        let (to, _) = honest_jump(&grid, &walls, run_up, keys, 5.0);
        assert_eq!((to.y - rise).abs() < 0.05, sprint, "{rise} sprint: {to:?}");
        let (to, _) = honest_jump(&grid, &walls, at_face, key::JUMP, 0.0);
        assert!(to.y.abs() < 0.05, "{rise} in place: {to:?}");
    }
}

/// A jump off a ledge's edge comes down on the floor below (it falls
/// after its clip).
#[test]
pub(crate) fn an_honest_jump_off_an_edge_falls_to_the_floor_below() {
    let triangles = ledge_triangles(3.0, mapdata::WalkerBehaviour::BySlope);
    let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default());
    let walls = mapdata::RayTree::build(&triangles);
    let mut body = honest_body(&grid, Vec3::new(10.6, 3.0, 5.0));
    body.yaw = std::f32::consts::FRAC_PI_2;
    let (to, _) = honest_jump(&grid, &walls, body, key::FORWARD | key::JUMP, 0.0);
    assert!(to.y.abs() < 0.05 && to.x < 9.0, "{to:?}");
}

/// The client's fall damage by landing speed (`docs/notes/fall_damage.md`, `fatal_damage` 1.0): none up to 9.9 m/s, linear to 14 m/s, 1 beyond; a
/// fall of 8 m (v = 12.5 m/s) is about 0.64 and kills from full health (0.6), one of 5 m (9.9 m/s) hurts no one.
#[test]
pub(crate) fn the_fall_damage_follows_the_clients_rule_by_landing_speed() {
    assert_eq!(fall_damage(0.0), 0.0);
    assert!(fall_damage(9.89) == 0.0);
    assert!((fall_damage(10.8) - 0.22).abs() < 0.01, "{}", fall_damage(10.8));
    let eight_m = (2.0f32 * 9.8 * 8.0).sqrt();
    assert!((fall_damage(eight_m) - 0.64).abs() < 0.02, "{}", fall_damage(eight_m));
    assert!(fall_damage(eight_m) >= LETHAL_FALL_DAMAGE);
    assert_eq!(fall_damage(14.0), 1.0);
    assert_eq!(fall_damage(30.0), 1.0);
}

/// The honest walk steps up to the client's 0.6 m, and not up the grid's
/// "forced" stair links of up to 0.75 m (`walk` faces) that the old
/// walk took.
#[test]
pub(crate) fn the_honest_walk_steps_no_higher_than_the_client() {
    let walk_east = |grid: &mapdata::WalkGrid, capped: bool| {
        let mut node = grid.locate(Vec3::new(8.0, 0.0, 5.0), 0.5).unwrap();
        let mut feet = grid.position(node);
        for _ in 0..40 {
            let to = feet + Vec3::new(0.2, 0.0, 0.0);
            (node, feet, _) = if capped {
                slide_capped(grid, node, feet, to, grid.body().step)
            } else {
                slide(grid, node, feet, to)
            };
        }
        feet
    };
    let forced = |rise: f32| {
        let triangles = ledge_triangles(rise, mapdata::WalkerBehaviour::Walk);
        let mut triangles = triangles;
        // The floor before the step forced too: the grid's stair rule
        // links two forced floors.
        for t in triangles.iter_mut().take(2) {
            t.walker = mapdata::WalkerBehaviour::Walk;
        }
        mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default())
    };
    let low = forced(0.5);
    assert!((walk_east(&low, true).y - 0.5).abs() < 0.05);
    let high = forced(0.7);
    let old = walk_east(&high, false);
    assert!((old.y - 0.7).abs() < 0.05, "the grid links a 0.7 m forced step: {old:?}");
    let honest = walk_east(&high, true);
    assert!(honest.y.abs() < 0.05 && honest.x < 10.0, "{honest:?}");
}

/// Two blocks (z 0-10) with tops `a` and `b` high, the gap between them
/// from x 6 to `6 + gap`, over ground at `ground`: the triangles.
pub(crate) fn gap_triangles(a: f32, b: f32, gap: f32, ground: f32) -> Vec<mapdata::Triangle> {
    let walker = mapdata::WalkerBehaviour::BySlope;
    let quad = |p: [Vec3; 4]| {
        [
            mapdata::Triangle { v: [p[0], p[1], p[2]], walker },
            mapdata::Triangle { v: [p[0], p[2], p[3]], walker },
        ]
    };
    let top = |x0: f32, x1: f32, y: f32| {
        quad([
            Vec3::new(x0, y, 0.0),
            Vec3::new(x1, y, 0.0),
            Vec3::new(x1, y, 10.0),
            Vec3::new(x0, y, 10.0),
        ])
    };
    let face = |x: f32, y0: f32, y1: f32| {
        quad([
            Vec3::new(x, y0, 0.0),
            Vec3::new(x, y0, 10.0),
            Vec3::new(x, y1, 10.0),
            Vec3::new(x, y1, 0.0),
        ])
    };
    let mut t = top(0.0, 14.0, ground).to_vec();
    t.extend(top(0.0, 6.0, a));
    t.extend(face(6.0, ground, a));
    t.extend(top(6.0 + gap, 14.0, b));
    t.extend(face(6.0 + gap, ground, b));
    t
}

/// The grid's gap links (`mapdata::grid::gap`) east across a gap are
/// made by the honest stand-in's jump: walking at the walk cap, pressed
/// at the link's own point (and the spread earlier or later), with
/// forward held, it comes down on the link's landing level.
#[test]
pub(crate) fn the_grids_gap_links_are_made_by_the_honest_jump() {
    use mapdata::grid::gap::GAP_PRESS_SPREAD_M;
    for (a, b, gap, ground) in [(2.0, 2.0, 1.0, 0.0), (2.0, 2.6, 0.75, 0.0), (2.0, 0.5, 1.5, -3.0)] {
        let mut triangles = gap_triangles(a, b, gap, ground);
        let mut area = [0.0, 0.0, 14.0, 10.0];
        if b < a - 0.5 {
            // A drop is a link only with a way back up (a one-way drop would keep the yard as reached), and a long
            // one, or the links save no walk round: out of the near top north, 20 m on, east, and down a 45 degree
            // ramp onto the far block's north edge.
            let walker = mapdata::WalkerBehaviour::BySlope;
            let plate = |p: [Vec3; 4]| {
                [
                    mapdata::Triangle { v: [p[0], p[1], p[2]], walker },
                    mapdata::Triangle { v: [p[0], p[2], p[3]], walker },
                ]
            };
            let flat = |x0: f32, z0: f32, x1: f32, z1: f32| {
                plate([Vec3::new(x0, a, z0), Vec3::new(x1, a, z0), Vec3::new(x1, a, z1), Vec3::new(x0, a, z1)])
            };
            triangles.extend(flat(1.0, 10.0, 3.0, 30.0));
            triangles.extend(flat(1.0, 28.0, 13.0, 30.0));
            triangles.extend(flat(11.0, 11.5, 13.0, 30.0));
            triangles.extend(plate([
                Vec3::new(11.0, b, 10.0),
                Vec3::new(13.0, b, 10.0),
                Vec3::new(13.0, a, 11.5),
                Vec3::new(11.0, a, 11.5),
            ]));
            area = [0.0, 0.0, 14.0, 32.0];
        }
        let grid = mapdata::WalkGrid::build_with_links(
            &triangles,
            area,
            0.25,
            mapdata::Body::default(),
            mapdata::OffMeshLinks::Gaps,
        );
        let walls = mapdata::RayTree::build(&triangles);
        let east: Vec<_> = grid
            .off_links()
            .iter()
            .filter(|l| {
                let (p, q) = (grid.position(l.from), grid.position(l.to));
                p.x < 6.0 && (q.z - p.z).abs() < 0.01 && (3.0..7.0).contains(&p.z)
            })
            .copied()
            .collect();
        assert!(!east.is_empty(), "{a} {b} {gap}: no gap link east");
        for l in east {
            let (from, to) = (grid.position(l.from), grid.position(l.to));
            // A walk-off has no press to fly (its own test: `gap.rs`).
            let mapdata::grid::gap::Takeoff::Press(press) = l.press else {
                continue;
            };
            let press = press.metres();
            for lead in [press - GAP_PRESS_SPREAD_M, press, press + GAP_PRESS_SPREAD_M] {
                let body = honest_body(&grid, Vec3::new(from.x - lead, from.y, from.z));
                let (down, _) = honest_jump(&grid, &walls, body, key::FORWARD | key::JUMP, WALK_CAP);
                assert!((down.y - to.y).abs() < 0.1, "{a} {b} {gap} lead {lead}: {from:?} -> {to:?} came down at {down:?}");
            }
        }
    }
}

/// [`past_edge`]: a body past the top's last floor at a ledge walks on while that floor is within [`body::SUPPORT_M`] of where it
/// steps, and comes down on the floor below once it is not, with no drop link asked; at the foot of the face (a wall) nothing changes.
#[test]
fn a_body_past_a_ledges_edge_walks_on_within_the_support_and_falls_past_it() {
    use super::body::tests::{floor, wall_x};
    // The ground at 0, a top 2 m high from x 10 on, its face at x 10.
    let mut triangles: Vec<mapdata::Triangle> = floor(0.0, 0.0, 20.0, 10.0, 0.0).to_vec();
    triangles.extend(floor(10.0, 0.0, 20.0, 10.0, 2.0));
    triangles.extend(wall_x(10.0, 0.0, 10.0).map(|mut t| {
        t.v.iter_mut().for_each(|v| v.y = v.y.min(2.0));
        t
    }));
    let grid = mapdata::WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, mapdata::Body::default());
    let walls = mapdata::RayTree::build(&triangles);
    // The top's last floor toward the face along z 5.
    let last = (0..grid.len() as NodeId)
        .filter(|&n| grid.position(n).y > 1.9 && (grid.position(n).z - 5.0).abs() < 0.2)
        .min_by(|&a, &b| grid.position(a).x.total_cmp(&grid.position(b).x))
        .unwrap();
    let edge = grid.position(last);
    let feet = Vec3::new(edge.x - 0.3, edge.y, edge.z);
    let on = past_edge(&grid, &walls, last, feet, Vec3::new(edge.x - 0.4, edge.y, edge.z));
    assert!(on.is_some_and(|(n, f)| n == last && (f.y - edge.y).abs() < 1e-3), "within the support it walks on: {on:?}");
    let down = past_edge(&grid, &walls, last, feet, Vec3::new(edge.x - 0.6, edge.y, edge.z));
    assert!(down.is_some_and(|(_, f)| f.y < 0.5), "past the support it comes down: {down:?}");
    // At the face's foot, walking into it: the wall stops it.
    let foot = (0..grid.len() as NodeId)
        .filter(|&n| grid.position(n).y < 0.1 && (grid.position(n).z - 5.0).abs() < 0.2 && grid.position(n).x < 10.0)
        .max_by(|&a, &b| grid.position(a).x.total_cmp(&grid.position(b).x))
        .unwrap();
    let at = grid.position(foot);
    assert_eq!(past_edge(&grid, &walls, foot, at, at + Vec3::new(0.2, 0.0, 0.0)), None);
}

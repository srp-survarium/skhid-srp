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
    rise_m: 0.998,
    ahead_m: 0.6,
};

/// [`STANDING_JUMP`] as the stand-in game jumps it: `SRP_BENCH_JUMP_AHEAD_M`
/// sets how far ahead a jump with no grid link lands (a sensitivity row;
/// the grid lists pocket exits up to 1.0 m across), else the GUESS 0.6 m.
pub(crate) fn standing_jump() -> StandInJump {
    static AHEAD: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    let ahead_m = *AHEAD.get_or_init(|| {
        std::env::var("SRP_BENCH_JUMP_AHEAD_M")
            .ok()
            .and_then(|v| v.parse().ok())
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
    *HONEST.get_or_init(|| std::env::var("SRP_BENCH_STAND_IN").map_or(true, |v| v != "old"))
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
}

/// One tick of `body`'s jump ([`Flight`]) at `tick`; the landing when it
/// came down.
pub(crate) fn fly(
    grid: &mapdata::WalkGrid,
    walls: &mapdata::RayTree,
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
    let landed = |body: &mut Body, f: &Flight, n: NodeId, feet: Vec3| {
        body.node = Some(n);
        body.feet = feet;
        body.flight = None;
        Some(Landing {
            place,
            at: f.at,
            to: feet,
            sprint: f.sprint(),
        })
    };
    if let Some(v) = f.fall {
        let v = v + GRAVITY * TICK_S;
        let y = body.feet.y - v * TICK_S;
        match air_floor(grid, body.feet, body.feet.y + 0.05) {
            Some((n, feet)) if feet.y >= y => return landed(body, &f, n, feet),
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
                return landed(body, &f, node, feet);
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
            let (n, feet, _) = super::body::slide_capped(grid, node, body.feet, to, grid.body().step);
            body.node = Some(n);
            body.feet = feet;
        }
        body.flight = Some(f);
        return None;
    }
    let base = *f.base.get_or_insert(body.feet.y);
    let y = base + rise;
    let was = body.feet.y;
    let (x, z) = air_move(walls, grid.body().radius, body.feet, target, y);
    body.feet = Vec3::new(x, y, z);
    if (f.descending(ms) || over)
        && let Some((n, feet)) = air_floor(grid, body.feet, was.max(y) + 0.05)
        && feet.y >= y - 1e-4
    {
        return landed(body, &f, n, feet);
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
        let landing = fly(grid, walls, &mut body, 0, start + k * TICK_MS);
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

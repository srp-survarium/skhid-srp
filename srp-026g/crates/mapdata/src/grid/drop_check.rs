//! [A313] The Drop links the client's own walk tick does not take a body down ([`super::Body::drop_check`]).
//!
//! The cells see a Drop where a floor ends and a lower one lies a cell on. The client's capsule does not always go down there: it
//! stands on past the take-off cell on a rim the erosion took from the grid, or on a deck that goes on over the landing.
//!
//! Each Drop link is walked as a bot walks it: the body set on the take-off (out of the walls), steered at the landing at
//! [`WALK_MPS`], then on along the link up to [`WALK_ON_M`] past it (a bot walks on to its next node), then left to settle, on
//! three lines across the link. The link is unfollowed when on every line the body walks on ([`MOVED_M`]) and the feet never
//! leave the take-off's level ([`LEAVE_M`] down); a line whose body cannot move from its start says nothing and follows. Where the body goes down but lands elsewhere (past the landing, on a ledge partway) the link stays: the
//! body did take the drop, and the reckoning needs the link to take it down with the client (a first rule that asked for the
//! landing itself made fx-bench's drift score worse: drop-only landings 0.87 -> 0.99 points).
//!
//! The drops census (A313, every Drop link of rev-48's 16 map/day projects through `controller::tick`, one line from the cell
//! centre): 36,829 of 494,533 links (7.4 %) left the body on the take-off's level.
//!
//! [blocked walk-offs] A line whose body cannot move says nothing of the drop (a start boxed in by the port's placing), except
//! where a wall or a ceiling no gait passes stands on the way ([`walk_line_blocked`]): when that holds on every line and no jump
//! off the edge lands either, no client way goes down there, and the link is taken away ([`super::Body::drop_lands`]: a Drop
//! link cut, a walk-off GapDrop removed). So is a walk-off no client way tried lands from (kept until now: an off-mesh link has
//! no reach guard) where such a wall or ceiling stands on every line. Level 7's S011 (a walk-off GapDrop from (-41.99, 20.88, -7.04), a wall 0.47 m ahead
//! from the knee to over the head) and S083 (a Drop link from (31.51, 22.03, -14.04), a wall 0.01 m ahead, a ceiling 0.13 m
//! over the head): the client's body walked, sprinted and crouched at them at seven headings, from the take-off and with a
//! run-up, and reached neither landing (exe_walkoff_point_v2 on rig_real_v3, 84 and 55 runs).

use super::{DIRS, LIP_DROP_M, LinkKind, NONE, SEED_M, SPAWN_M, Triangle, Vec3, WalkGrid, Wide};
use super::controlled::{STARTS, Stance, WalkStart, body_at, out_of_walls, parallel};
use super::gap_flown::THIN_TOP_M;
use super::gap::{GAP_PRESS_MAX_M, GAP_PRESS_MIN_M, GAP_PRESS_STEP_M, GridWay, PressLead, RELEASE_AT_MAX_M, Share, Takeoff};
use crate::controller::{jump, Collision, Shape};
use crate::controller::tick::{Controller, CrouchInput, Settings, TickInput};

/// The tick, s: the client's step, 50 ms (0x9CC878; every step of the traces is 50 ms). It was the census's 30 a second (C2 row
/// 44); the phases below keep their times.
const DT: f32 = 0.05;
/// A walking body's speed, m/s: `player_params`' `desired` `walk_fwd`, the cap `base_player::max_speed` (0x2DC010) puts on a
/// walking step's vector (docs/notes/sprint_state.md, Speed).
const WALK_MPS: f32 = 3.5;
/// How far under the take-off the feet must go to have left it, m (or half the drop, when less).
const LEAVE_M: f32 = 0.4;
/// How far from its start across a body must get for its line to say anything, m.
const MOVED_M: f32 = 0.3;
/// How far past the landing across the body walks on along the link, m.
const WALK_ON_M: f32 = 1.5;
/// The ticks of each phase at [`DT`]: steering at the landing (3 s), walking on (2 s), settling (1 s).
const STEER_TICKS: usize = 60;
const ON_TICKS: usize = 40;
const SETTLE_TICKS: usize = 20;

impl WalkGrid {
    /// The Drop links of `wide` the client's walk tick does not take a body down, as (take-off, direction, rank 0). Only those
    /// out of a floor a body gets to from the seeds or a spawn are walked: a link out of any other floor is never on a route.
    /// The links are walked on the machine's threads; the result is in link order whatever the threads.
    pub(super) fn unfollowed_drops(&self, wide: &Wide, triangles: &[Triangle], seeds: &[Vec3], spawns: &[Vec3]) -> Vec<(u32, usize, f32)> {
        let n = self.height.len();
        let mut reached = vec![false; n];
        let mut queue: Vec<u32> = seeds
            .iter()
            .filter_map(|&p| self.locate(p, SEED_M))
            .chain(spawns.iter().filter_map(|&p| self.locate(p, SPAWN_M)))
            .collect();
        for &a in &queue {
            reached[a as usize] = true;
        }
        while let Some(a) = queue.pop() {
            for b in wide.out(a as usize).chain(wide.slips_from(a)) {
                if !reached[b as usize] {
                    reached[b as usize] = true;
                    queue.push(b);
                }
            }
        }
        let links: Vec<(u32, usize, Vec3, Vec3)> = (0..n)
            .filter(|&a| reached[a])
            .flat_map(|a| {
                (0..DIRS.len()).filter_map(move |d| {
                    let b = wide.links[a][d];
                    (b != NONE && wide.kinds[a][d] == LinkKind::Drop).then(|| (a as u32, d, self.position(a as u32), self.position(b)))
                })
            })
            .collect();
        if links.is_empty() {
            return Vec::new();
        }
        let (caster, s) = self.body.controller();
        let world = Collision::build(triangles).with_caster(caster);
        let follow = parallel(&links, |l| follows(&world, &s, l.2, l.3));
        links.iter().zip(follow).filter(|(_, ok)| !ok).map(|(l, _)| (l.0, l.1, 0.0)).collect()
    }
}

fn flat(a: Vec3, b: Vec3) -> f32 {
    (a.x - b.x).hypot(a.z - b.z)
}

/// [blocked walk-offs] Whether a wall or a ceiling that no gait of the client's body passes stands on the walk line from `from`
/// toward `to`: the crouched body (the lowest one, [`super::Body::crouch_height`]) above the walk tick's step
/// ([`Settings::step_height`]: a ledge under it is stepped onto), as the move test's cylinder and margin ([`out_of_walls`]),
/// swept flat from the line's start (moved out of the walls, as a walk line's is) half the way to the landing (a one-cell
/// drop's edge), meets a face the walk does not stand on: one steeper than the walk's slope limit, or one facing down.
pub(super) fn walk_line_blocked(world: &Collision, s: &Settings, body: &super::Body, from: Vec3, to: Vec3) -> bool {
    const MARGIN_M: f32 = 0.04;
    let start = out_of_walls(world, from);
    let (low, high) = (s.step_height, body.crouch_height);
    if flat(start, to) < 1e-3 || high - low <= 2.0 * MARGIN_M {
        return false;
    }
    let shape = Shape::Cylinder {
        centre: start + Vec3::new(0.0, 0.5 * (low + high), 0.0),
        half: 0.5 * (high - low) - MARGIN_M,
        radius: body.radius,
        margin: MARGIN_M,
    };
    let way = Vec3::new(to.x - start.x, 0.0, to.z - start.z) * 0.5;
    world.sweep_shape(shape, way, |f| f.normal.y < s.max_slope_cos).is_some()
}

/// Whether the client's walk tick takes a body walking off `from` down to `to` (module documentation): on any of the
/// controller pass's [`STARTS`] lines (across the link), each start moved out of the walls the body would overlap there
/// ([`out_of_walls`]: the grid keeps floors 0.3175 m from a wall, the body's move test 0.42 m). A link is unfollowed only when
/// every line fails, so one line's corner does not decide it (the controller pass's rule, learnt on Cologne's mossy log).
pub(super) fn follows(world: &Collision, s: &Settings, from: Vec3, to: Vec3) -> bool {
    let len = flat(from, to).max(1e-3);
    let side = Vec3::new(-(to.z - from.z) / len, 0.0, (to.x - from.x) / len);
    STARTS.iter().any(|&off| follows_line(world, s, out_of_walls(world, from + side * off), to + side * off))
}

/// One line of [`follows`]: the body set standing on `from`, steered at `to`, then on along the line. It follows when it leaves
/// the take-off's level on the way down ([`left_take_off`]), wherever it then lands: a body that slides past the landing or
/// stops on a ledge partway down still went down the drop, and the reckoning needs the link to take it down (fx-bench's drift
/// score of the first rule, which asked for the landing itself: drop-only landings 0.87 -> 0.99 points, the body held up while
/// the world fell). Unfollowed is a body that walks on ([`MOVED_M`] or more from its start) and stays on the take-off's level: a
/// landing under a deck that goes on, a rim the erosion took. A body that cannot move from its start at all (a start boxed in by
/// walls, every move refused) says nothing of the drop: that line follows (fx-bench's second score: level_07's shallow steps).
fn follows_line(world: &Collision, s: &Settings, from: Vec3, to: Vec3) -> bool {
    let mut body = Controller::standing_at(from);
    let down = left_take_off(from.y, to.y);
    let gone = |feet: Vec3| feet.y <= down;
    let mut moved = 0.0f32;
    // Steered at the landing; over it, on along the line.
    for _ in 0..STEER_TICKS {
        let feet = body.feet();
        if gone(feet) {
            return true;
        }
        moved = moved.max(flat(feet, from));
        let d = flat(feet, to);
        if d <= 0.05 {
            break;
        }
        let step = (WALK_MPS * DT).min(d);
        body.walk = Vec3::new((to.x - feet.x) / d * step, 0.0, (to.z - feet.z) / d * step);
        body.step(world, DT, s);
    }
    let len = flat(from, to).max(1e-3);
    let (ux, uz) = ((to.x - from.x) / len, (to.z - from.z) / len);
    for _ in 0..ON_TICKS {
        let feet = body.feet();
        if gone(feet) {
            return true;
        }
        moved = moved.max(flat(feet, from));
        if flat(feet, to) >= WALK_ON_M {
            break;
        }
        body.walk = Vec3::new(ux * WALK_MPS * DT, 0.0, uz * WALK_MPS * DT);
        body.step(world, DT, s);
    }
    body.walk = Vec3::ZERO;
    for _ in 0..SETTLE_TICKS {
        if gone(body.feet()) {
            return true;
        }
        if !body.step(world, DT, s).airborne {
            break;
        }
    }
    gone(body.feet()) || moved.max(flat(body.feet(), from)) < MOVED_M
}

/// The height under which the feet have left a take-off at `from_y` for a landing at `to_y`: [`LEAVE_M`] down, or half the drop
/// when that is less. The drop script ([`walk_off_rest`]) and the bots' motor (`walk_off.rs`, F3) release on the first
/// standing step at or under it.
pub fn left_take_off(from_y: f32, to_y: f32) -> f32 {
    from_y - LEAVE_M.min(0.5 * (from_y - to_y).max(0.0))
}

/// [feed 67] The step of [`walk_drop`], s: the client's 50 ms step, the recorded steps of the traced match being 50 ms apart
/// (docs/notes/exactness_rig.md, the chain replay: 8,993 steps in 50 chains).
const LAND_DT: f32 = 0.05;
/// [feed 67] The steps a body has to leave the take-off in (3 s), and then to come to stand on the landing (2 s): the bounds of
/// the exactness session's walk (FINDINGS-FEED-exactness 2026-11-12 09:56Z), our choice, not client values.
const LEAVE_STEPS: usize = 60;
const LAND_STEPS: usize = 40;
/// [A1b] The time a body let go at its release is given to come to rest, s: [`LAND_STEPS`] steps of [`LAND_DT`] (the release
/// flight's bound, [`released_rest`]); the motor's let-go ends after it (`walk_off.rs`).
pub const RELEASE_SETTLE_S: f32 = LAND_STEPS as f32 * LAND_DT;
/// [feed 67] Within this of the to-point across, m, the body walks on along the link's line instead of steering at it (a body
/// stopped over a short drop's to-point stays on the rim: the exactness session's first walk, 1,877 of 6,000 on Chemical Plant,
/// FINDINGS-FEED-exactness 2026-11-12 09:56Z). The walk's own value, our choice: under a step at 3.5 m/s (0.175 m) the body
/// would stop short of the to-point instead.
const WALK_ON_WITHIN_M: f32 = 0.3;
/// [feed 67] A body has arrived standing within this of the to-point across, m, and [`ARRIVE_HEIGHT_M`] of its height: the
/// exactness walk's arrival (FINDINGS-FEED-exactness 2026-11-12 09:56Z), our choice; across, two cells (0.5 m) of the grid; in
/// height, the walk check's [`super::controlled`] `AT_HEIGHT_M`.
pub const ARRIVE_FLAT_M: f32 = 0.5;
pub const ARRIVE_HEIGHT_M: f32 = 0.2;
/// [A377] The steps of the client's stop blend: on the k-th step after the walk key is let go the walk is 1 - k/6 of the speed
/// it had (docs/notes/walk_blend.md: the 0.3 s blend, 0x921804, a linear weight; 82 clean stops counted, 1 - k/6).
const STOP_STEPS: usize = 6;

/// [feed 67] How the client's body walking off a Drop link ends ([`walk_drop`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropWalk {
    /// Standing on the to-point's floor ([`ARRIVE_FLAT_M`] across, [`ARRIVE_HEIGHT_M`] in height) within [`LAND_STEPS`] of
    /// leaving the take-off's level.
    Arrives,
    /// Left the take-off's level and came to stand elsewhere: its feet there (a ledge partway, a floor past the landing), and
    /// [S6] the speeds it landed at, m/s down: the body's downward speed at the start of each tick that landed it
    /// (`on_physical_controller_landing` 0x2D8220 gets that tick's starting speed, the exe review of #1969), [item 2] on
    /// walkable ground and on ground too steep to stand on alike ([`crate::controller::tick::LandingGround`]): its landings that
    /// count ([`super::held_floors::Landed`]).
    LandsAt(Vec3, super::held_floors::Landed),
    /// Left the take-off's level and not standing [`LAND_STEPS`] later (falling on, sliding down a steep face).
    Unsettled,
    /// Walked [`MOVED_M`] or more from its start and never left the take-off's level (a rim the erosion took, a deck over the
    /// landing).
    Held,
    /// Never got [`MOVED_M`] from its start: says nothing of the drop.
    NoMove,
}

/// [feed 67] [`super::Body::drop_lands`]: the client's body set standing at `from` ([`body_at`]) walks at 3.5 m/s at `to`,
/// re-aimed each 50 ms step, and on along the link's line once within [`WALK_ON_WITHIN_M`] of it, as a bot walks on to its next
/// node; the exactness session's walk of the Drop links (`private/grid/rig/link-walk`, `kinds.rs` `drop_one`, FINDINGS-FEED-exactness
/// 2026-11-12 09:56Z), its leave rule ([`left_take_off`]) and its arrival.
pub fn walk_drop(world: &Collision, s: &Settings, start: WalkStart, from: Vec3, to: Vec3) -> DropWalk {
    walk_drop_as(world, s, start, Stance::Standing, WALK_MPS, from, to)
}

/// [A389] The crouched walk's speed forward, m/s: `player_params`' crouch forward 2.5 x the body's factor 1.0 (0x337D5E,
/// `docs/notes/crouch_logic.md`). A body walked off a drop crouched comes down nearer the edge than a walking one.
/// A literal from the docs, flagged: no reader of the game data's `player_params` `movement_speed/desired/crouch_fwd` exists
/// in this crate yet, as for the walk's [`WALK_MPS`]. [A1b] The release (`gap::Takeoff::Release`) does not hang on it: it is
/// by position, and a let-go body carries no walk of its own past the stop blend.
pub const CROUCH_MPS: f32 = 2.5;

/// [A389] [`walk_drop`] in `stance` at `speed` m/s (the crouch key held throughout for [`Stance::Crouched`]).
pub fn walk_drop_as(world: &Collision, s: &Settings, start: WalkStart, stance: Stance, speed: f32, from: Vec3, to: Vec3) -> DropWalk {
    let mut c = body_at(world, start, stance, from);
    let key = stance == Stance::Crouched;
    let first = c.feet();
    let down = left_take_off(from.y, to.y);
    let len = flat(from, to).max(1e-4);
    let line = Vec3::new((to.x - from.x) / len, 0.0, (to.z - from.z) / len);
    let mut moved = 0.0f32;
    let mut left: Option<usize> = None;
    let mut landed = super::held_floors::Landed::NONE;
    for n in 1..=LEAVE_STEPS + LAND_STEPS {
        let feet = c.feet();
        let d = flat(feet, to);
        let falling = -c.velocity.y;
        let walk = if d <= WALK_ON_WITHIN_M {
            line * (speed * LAND_DT)
        } else {
            Vec3::new((to.x - feet.x) / d, 0.0, (to.z - feet.z) / d) * (speed * LAND_DT).min(d)
        };
        let notes = c.tick(world, LAND_DT, s, TickInput {
            walk,
            air_control: Vec3::ZERO,
            crouch: CrouchInput::Tested { key, legs_broken: false },
            sprinting: false,
        });
        if let Some(ground) = notes.landing {
            landed = landed.and(ground, falling);
        }
        let feet = c.feet();
        moved = moved.max(flat(feet, first));
        if left.is_none() && feet.y <= down {
            left = Some(n);
        }
        match left {
            None if n >= LEAVE_STEPS => return if moved < MOVED_M { DropWalk::NoMove } else { DropWalk::Held },
            None => {}
            Some(t) => {
                let standing = !c.jumping && c.on_ground(s);
                if standing && flat(feet, to) <= ARRIVE_FLAT_M && (feet.y - to.y).abs() <= ARRIVE_HEIGHT_M {
                    return DropWalk::Arrives;
                }
                if n - t >= LAND_STEPS {
                    return if standing { DropWalk::LandsAt(feet, landed) } else { DropWalk::Unsettled };
                }
            }
        }
    }
    DropWalk::Unsettled
}

/// [F2] A Drop link's key script, as a body plays it and the build flies it ([`walk_off_rest`]): the walk at the link's speed
/// steered at the landing (re-aimed each step; on along the link's line from within [`WALK_ON_WITHIN_M`] or past it), the forward key held
/// over the edge and through the fall, and let go on the first step the body stands below the take-off's level (the jumps'
/// [`jump::STAY_KEYS`]: forward held to the landing, let go there); then nothing until it rests. A [`super::gap::Takeoff::Release`]
/// is the other drop script (let go at the edge, [`released_rest`]).
///
/// The landing a link promises is where this flight RESTS, not a step of a walk carried on past it: [`walk_drop`]'s
/// arrival (standing within [`ARRIVE_FLAT_M`] of the landing on any step while the key is still held and the body walks on)
/// passed links whose body only crossed the landing (owner-16 part 3 F2: of 22 planned falls with a known landing 10 ended
/// lower; the Vector Laboratory battery ledge's drops land 0 of 9 in the exe, 2.3-2.7 m lower).
///
/// [`DropWalk::LandsAt`] with the rest's feet and the landings counted, [`DropWalk::Unsettled`], [`DropWalk::Held`] or
/// [`DropWalk::NoMove`] as [`walk_drop_as`]; never [`DropWalk::Arrives`] (the caller judges the rest against the link's own top).
pub fn walk_off_rest(world: &Collision, s: &Settings, start: WalkStart, stance: Stance, speed: f32, from: Vec3, to: Vec3) -> DropWalk {
    let mut c = body_at(world, start, stance, from);
    let key = stance == Stance::Crouched;
    let first = c.feet();
    let down = left_take_off(from.y, to.y);
    let len = flat(from, to).max(1e-4);
    let line = Vec3::new((to.x - from.x) / len, 0.0, (to.z - from.z) / len);
    let mut moved = 0.0f32;
    let mut left: Option<usize> = None;
    // The let-go's walk and the steps since (the client's stop blend, 1 - k/6 of the last walk, [`STOP_STEPS`]).
    let mut let_go: Option<(Vec3, usize)> = None;
    // Once within [`WALK_ON_WITHIN_M`] of the landing or past it along the line, the walk goes on along the line: a body that
    // passed its waypoint walks on to the next, it does not turn back to it (steered back in the air, the let-go at the
    // landing would carry it back to the take-off's face).
    let mut on_line = false;
    let mut landed = super::held_floors::Landed::NONE;
    for n in 1..=LEAVE_STEPS + LAND_STEPS {
        let feet = c.feet();
        let falling = -c.velocity.y;
        let d = flat(feet, to);
        let past = (feet.x - to.x) * line.x + (feet.z - to.z) * line.z;
        on_line |= d <= WALK_ON_WITHIN_M || past >= 0.0;
        let walk = match let_go {
            Some((last, k)) => last * (1.0 - (k + 1).min(STOP_STEPS) as f32 / STOP_STEPS as f32),
            // [`WALK_ON_M`] past the landing still on the take-off's level: the walk goes no farther (a landing under a deck
            // that goes on is no way down there; the body walking on to the deck's end is another link's).
            None if on_line && left.is_none() && past >= WALK_ON_M => Vec3::ZERO,
            None if on_line => line * (speed * LAND_DT),
            None => Vec3::new((to.x - feet.x) / d, 0.0, (to.z - feet.z) / d) * (speed * LAND_DT).min(d),
        };
        let notes = c.tick(world, LAND_DT, s, TickInput { walk, air_control: Vec3::ZERO, crouch: CrouchInput::Tested { key, legs_broken: false }, sprinting: false });
        if let Some(ground) = notes.landing {
            landed = landed.and(ground, falling);
        }
        let feet = c.feet();
        moved = moved.max(flat(feet, first));
        if left.is_none() && feet.y <= down {
            left = Some(n);
        }
        let standing = !c.jumping && c.on_ground(s);
        let_go = match (let_go, left) {
            (Some((last, k)), _) => Some((last, k + 1)),
            (None, Some(_)) if standing => Some((walk, 0)),
            (None, _) => None,
        };
        match left {
            None if n >= LEAVE_STEPS => return if moved < MOVED_M { DropWalk::NoMove } else { DropWalk::Held },
            None => {}
            Some(t) if n - t >= LAND_STEPS => return if standing { DropWalk::LandsAt(feet, landed) } else { DropWalk::Unsettled },
            Some(_) => {}
        }
    }
    DropWalk::Unsettled
}

/// [A377] Where the client's body comes to stand when it walks off `from` at `to` as [`walk_drop`] walks ([`WALK_MPS`], re-aimed
/// each [`LAND_DT`] step) and the walk key is let go on the step that would reach `to`: the walk then falls off over the stop
/// blend ([`STOP_STEPS`]) along the way it had, then nothing. Its feet [`LAND_STEPS`] steps after the blend when it stands then
/// (on the ground, no jump), else `None`. A bot that stops on a landing, or turns there, does so.
pub fn stop_at_drop(world: &Collision, s: &Settings, start: WalkStart, from: Vec3, to: Vec3) -> Option<Vec3> {
    let mut c = body_at(world, start, Stance::Standing, from);
    let step = WALK_MPS * LAND_DT;
    let len = flat(from, to).max(1e-4);
    let mut way = Vec3::new((to.x - from.x) / len, 0.0, (to.z - from.z) / len);
    let tick = |c: &mut Controller, walk: Vec3| {
        c.tick(world, LAND_DT, s, TickInput {
            walk,
            air_control: Vec3::ZERO,
            crouch: CrouchInput::Tested { key: false, legs_broken: false },
            sprinting: false,
        });
    };
    for _ in 0..LEAVE_STEPS {
        let feet = c.feet();
        let d = flat(feet, to);
        if d <= step {
            break;
        }
        way = Vec3::new((to.x - feet.x) / d, 0.0, (to.z - feet.z) / d);
        tick(&mut c, way * step);
    }
    for k in 1..=STOP_STEPS {
        tick(&mut c, way * (step * (1.0 - k as f32 / STOP_STEPS as f32)));
    }
    // Settled as [`walk_drop`]'s landing is judged, [`LAND_STEPS`] on (a step at the top of a fall reads as on the ground).
    for _ in 0..LAND_STEPS {
        tick(&mut c, Vec3::ZERO);
    }
    (!c.jumping && c.on_ground(s)).then(|| c.feet())
}

/// [A1b] How far under its start a body's step must take its feet to have left the top, m: the exactness rig's own leave test
/// (`grid/rig/mapflights/src/flights.rs` `walk_run`: airborne and 5 mm under the start), by which the release rule was measured.
const RELEASE_LEFT_M: f32 = 0.005;

/// [A1b] Where the release is, m along the way past `from`: the body walked from `from` along `way` (flat, unit) at `speed`
/// m/s in `stance`, from its first step (a steady walk), and the step after which a copy of it stepped once more with the key
/// held would leave the top (airborne and [`RELEASE_LEFT_M`] under the start): exactness `mapflights` §2 A1b, "release on the
/// step before the body leaves the top" (crouched 22 of 26 on the ledge, standing 26 of 26). The centre's distance along the
/// way at the end of that last held step; None when the body never leaves within [`LEAVE_STEPS`] or the release would be past
/// [`RELEASE_AT_MAX_M`].
pub fn release_point(world: &Collision, s: &Settings, start: WalkStart, stance: Stance, speed: f32, from: Vec3, way: Vec3) -> Option<f32> {
    let mut c = body_at(world, start, stance, from);
    let key = stance == Stance::Crouched;
    let top = c.feet();
    let input = TickInput {
        walk: way * (speed * LAND_DT),
        air_control: Vec3::ZERO,
        crouch: CrouchInput::Tested { key, legs_broken: false },
        sprinting: false,
    };
    for _ in 0..LEAVE_STEPS {
        let mut ahead = c;
        let notes = ahead.tick(world, LAND_DT, s, input);
        if notes.airborne && ahead.feet().y < top.y - RELEASE_LEFT_M {
            let at = along(&c, from, way).max(0.0);
            return (at <= RELEASE_AT_MAX_M).then_some(at);
        }
        c = ahead;
    }
    None
}

/// [A1b] The body's centre's distance along `way` (flat, unit) past `from`, m.
fn along(c: &Controller, from: Vec3, way: Vec3) -> f32 {
    (c.feet().x - from.x) * way.x + (c.feet().z - from.z) * way.z
}

/// [A1b] Where the client's body comes to rest when walked from `from` along `way` (flat, unit) at `speed` m/s in `stance` (a
/// steady walk from its first step) and let go once its centre is `at` m along the way: the walk then falls off over the
/// client's stop blend (1 - k/6, [`STOP_STEPS`]; the rig's `tapN` rows, by which A1b was measured, lay the same blend), the
/// stance kept, then nothing. Its feet [`LAND_STEPS`] steps after it left the top ([`left_take_off`] under `from`, for a
/// landing at `to_y`) when it stands then; None when it stays on the top or is still falling.
#[allow(clippy::too_many_arguments)]
pub fn released_rest(world: &Collision, s: &Settings, start: WalkStart, stance: Stance, speed: f32, from: Vec3, way: Vec3, at: f32, to_y: f32) -> Option<Vec3> {
    let mut c = body_at(world, start, stance, from);
    let key = stance == Stance::Crouched;
    let down = left_take_off(from.y, to_y);
    let step = speed * LAND_DT;
    let tick = |c: &mut Controller, f: f32| {
        c.tick(world, LAND_DT, s, TickInput {
            walk: way * (step * f),
            air_control: Vec3::ZERO,
            crouch: CrouchInput::Tested { key, legs_broken: false },
            sprinting: false,
        });
    };
    let mut held = 0;
    while held < LEAVE_STEPS && along(&c, from, way) < at - 1e-4 {
        tick(&mut c, 1.0);
        held += 1;
    }
    let mut left: Option<usize> = None;
    for n in 1..=LEAVE_STEPS + LAND_STEPS {
        let f = (1.0 - n.min(STOP_STEPS) as f32 / STOP_STEPS as f32).max(0.0);
        tick(&mut c, f);
        if left.is_none() && c.feet().y <= down {
            left = Some(n);
        }
        match left {
            None if n >= LEAVE_STEPS => return None,
            Some(t) if n - t >= LAND_STEPS => return (!c.jumping && c.on_ground(s)).then(|| c.feet()),
            _ => {}
        }
    }
    None
}

/// [feed 67] [`WalkGrid::landing_drops`]'s links to cut (take-off, direction, rank 0), re-aims (take-off, direction, new
/// landing) and [A377] lips by where the body stops (take-off, landing).
type Cuts = Vec<(u32, usize, f32)>;
/// [A1b] A re-aim carries the release press that takes the body there, with its share; None for a walk-off.
/// [`WalkGrid::reaim_drops`]: the re-aims onto a (take-off, landing) pair the same pass had already re-aimed (one link kept,
/// [`Self::merged`]) and, of them, which link was kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReaimPairs {
    /// Re-aims folded into a link of the same pair this pass added.
    pub merged: usize,
    /// … a release and a walk-off: the release kept (a flown landing; the walk-off's share is not flown).
    pub release_over_walk_off: usize,
    /// … two releases: the first kept (its share as high or higher) …
    pub releases_kept_first: usize,
    /// … or the later one (a higher share).
    pub releases_kept_later: usize,
    /// Release re-aims onto a pair whose link was there before the pass: that link kept (a release at a share as high or
    /// higher) …
    pub earlier_kept: usize,
    /// … or given the release's press and share.
    pub earlier_replaced: usize,
}

/// (take-off, direction, new landing, the release's press and share, [retarget] the grid way the body was walked along when that
/// is not the line to the new landing: None for a [`Verdict::Reaim`], walked at its landing).
type Reaims = Vec<(u32, usize, u32, Option<(Takeoff, Share)>, Option<GridWay>)>;
type Lips = Vec<(u32, u32)>;
/// [deep walk-offs] The deep landings kept beside a way down within the harmless fall ([`WalkGrid::keep_deep_landings`]):
/// (take-off, the deep landing, the grid way a taught one was come down along).
type DeepAlso = Vec<(u32, u32, Option<GridWay>)>;

/// [A389] What [`WalkGrid::landing_drops`] did ([`super::BuildStats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DropStats {
    /// Drop links walked …
    pub walked: usize,
    /// … re-aimed by a client way down other than the walk at the landing ([`Verdict::Taught`]) …
    pub taught: usize,
    /// … and cut: no client way tried stands on a floor.
    pub cut: usize,
    /// [A389] The gap pass's walk-offs walked ([`WalkGrid::walk_off_links`]), re-aimed, and left standing on no floor.
    pub walk_offs: usize,
    pub walk_offs_reaimed: usize,
    pub walk_offs_unlanded: usize,
    /// [A1b] Of the re-aimed Drop links, those taken down by the walk-off at release ([`Verdict::Pressed`]); of the walk-offs,
    /// those turned into one.
    pub released: usize,
    pub walk_offs_released: usize,
    /// [F2] Of the links walked (Drop links and walk-offs), those whose release was not made because their take-off is a thin
    /// top ([`WalkGrid::on_thin_top`]).
    pub releases_held_thin: usize,
    /// [taught presses] Of the Drop links, those only the crouched walk-off takes down ([`Takeoff::WalkOffCrouched`]), and those
    /// only a jump off the edge does (a short or a sprint jump): each an off-mesh GapDrop carrying that press.
    pub walked_off_crouched: usize,
    pub jumped_down: usize,
    /// [D2] Of the Drop links, those a walk line came to rest from deeper than the harmless fall ([`super::Body::max_drop`]):
    /// the other client ways down within it went first …
    pub past_harmless_fall: usize,
    /// … and of them, those no way within it took down: re-aimed (or taught) to the walk's own deep landing, at its true depth …
    pub past_harmless_fall_kept: usize,
    /// … and [deep walk-offs] those a way within it took down: the deep landing also kept, as an off-mesh GapDrop beside it
    /// ([`WalkGrid::keep_deep_landings`]).
    pub past_harmless_fall_also: usize,
    /// [blocked walk-offs] Of the Drop links, those cut, and of the walk-offs, those removed, where no client way goes down
    /// ([`Verdict::Blocked`]).
    pub blocked: usize,
    pub walk_offs_blocked: usize,
}

/// [feed 67] What [`super::Body::drop_lands`] does with one Drop link.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Verdict {
    /// A line arrives, or a line's body cannot move (it says nothing).
    #[default]
    Keep,
    /// The body lands on another floor of the grid, and walked at that floor it arrives there: the link goes there.
    Reaim(u32),
    /// [A389] No walk line arrives or re-aims as [`Self::Reaim`]; a standing walk line's own landing is this floor: the link (a
    /// standing walk-off) goes there.
    Taught(u32),
    /// [A1b] No walk line arrives, and another client way down comes to rest on this floor (on the link's own top, or lower than
    /// the take-off): the walk-off at release (crouched or standing, the key let go on the step before the body leaves the top),
    /// [taught presses] the crouched walk-off ([`Takeoff::WalkOffCrouched`]) or a jump off the edge (a short or a sprint jump at
    /// a gap lead). The link goes there with that press and its share: no standing walk-off takes the body there.
    Pressed(u32, Takeoff, Share),
    /// No client way down tried stands on a floor of the grid: the walk lines, the crouched walk-off, the short and sprint
    /// jumps at the gap leads.
    Cut,
    /// [blocked walk-offs] No walk line's body moves, a wall or a ceiling no gait passes stands on every line
    /// ([`walk_line_blocked`]), and no jump off the edge lands: no client way goes down there. A Drop link is cut, a walk-off
    /// GapDrop removed.
    Blocked,
}

/// [D2] Where a Drop link's walk line came to rest deeper than the harmless fall ([`super::Body::max_drop`]) under the take-off:
/// the walk's deep landing, at its true depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeepLanding {
    /// Walked at that floor, the body arrives there (as [`Verdict::Reaim`]).
    Reaim(u32),
    /// Walked at it, the body does not arrive; a standing walk line's own rest is there (as [`Verdict::Taught`]).
    Taught(u32),
}

impl DeepLanding {
    fn floor(self) -> u32 {
        match self {
            Self::Reaim(g) | Self::Taught(g) => g,
        }
    }

    /// The Drop link's verdict when no way within the harmless fall takes the body down: this landing.
    fn verdict(self) -> Verdict {
        match self {
            Self::Reaim(g) => Verdict::Reaim(g),
            Self::Taught(g) => Verdict::Taught(g),
        }
    }
}

impl WalkGrid {
    /// [feed 67] Every Drop link, as (take-off, direction, from, to). [body physics S5b] Every one, not only those out of a floor
    /// reached from the seeds or a spawn at this point: a deck reached only through links the earlier passes moved kept its plain
    /// Drops to the floor under its edge, unchecked (the S5a gates: walk-offs 1.5 m from the client body's rest, Rudnya batteries
    /// (71.70, 20.16, 138.47)). A floor no seed reaches in the end goes with [`super::WalkGrid::keep_reached`] anyway.
    fn all_drops(&self, wide: &Wide) -> Vec<(u32, usize, Vec3, Vec3)> {
        let n = self.height.len();
        (0..n)
            .flat_map(|a| {
                (0..DIRS.len()).filter_map(move |d| {
                    let b = wide.links[a][d];
                    (b != NONE && wide.kinds[a][d] == LinkKind::Drop).then(|| (a as u32, d, self.position(a as u32), self.position(b)))
                })
            })
            .collect()
    }

    /// The floors a body gets to from the seeds or a spawn over `wide`'s links and slips.
    pub(super) fn reached_floors(&self, wide: &Wide, seeds: &[Vec3], spawns: &[Vec3]) -> Vec<bool> {
        let n = self.height.len();
        let mut reached = vec![false; n];
        let mut queue: Vec<u32> = seeds
            .iter()
            .filter_map(|&p| self.locate(p, SEED_M))
            .chain(spawns.iter().filter_map(|&p| self.locate(p, SPAWN_M)))
            .collect();
        for &a in &queue {
            reached[a as usize] = true;
        }
        while let Some(a) = queue.pop() {
            for b in wide.out(a as usize).chain(wide.slips_from(a)) {
                if !reached[b as usize] {
                    reached[b as usize] = true;
                    queue.push(b);
                }
            }
        }
        reached
    }

    /// [A389] [`super::Body::drop_lands`]: the gap pass's walk-offs (off-mesh GapDrop links walked off, no key: its fall over the
    /// cells, not the client's) out of a floor a body gets to, walked by the client's body as a Drop link is
    /// ([`Self::drop_verdict`]): a link the body comes to stand elsewhere from goes to that floor (none added where the take-off
    /// has a link there already). One no client way tried stands on a floor stays as it is (an off-mesh link has no reach guard
    /// to cut it by; counted). [A1b] One the walk-off at release takes down ([`Verdict::Released`]) carries that press and share.
    /// Returns (walked, re-aimed, standing on no floor, released, held off a thin top, [blocked walk-offs] removed: no client way
    /// goes down there, [`Verdict::Blocked`]).
    pub(super) fn walk_off_links(&self, wide: &mut Wide, world: &Collision) -> (usize, usize, usize, usize, usize, usize) {
        // [body physics S5b] Every walk-off, as every Drop ([`Self::all_drops`]).
        let todo: Vec<usize> = (0..wide.off.len()).filter(|&i| wide.off[i].press == super::gap::Takeoff::WalkOff).collect();
        if todo.is_empty() {
            return (0, 0, 0, 0, 0, 0);
        }
        let s = self.body.controller().1;
        let start = self.body.walk_start();
        let verdicts = parallel(&todo, |&i| {
            let l = wide.off[i];
            self.drop_verdict(world, &s, start, l.from, self.position(l.from), l.to)
        });
        let (mut moved, mut none, mut released) = (0, 0, 0);
        let mut blocked = vec![false; wide.off.len()];
        let held = verdicts.iter().filter(|(_, held, _)| *held).count();
        for (&i, (v, _, _)) in todo.iter().zip(verdicts) {
            match v {
                Verdict::Keep => {}
                Verdict::Pressed(b, press, share) => {
                    let (from, to) = (wide.off[i].from, wide.off[i].to);
                    if b == to || (b != from && !wide.out(from as usize).any(|x| x == b)) {
                        moved += usize::from(b != to);
                        // [retarget] Pressed along the walk-off's own line: its way, kept where the landing moves.
                        let way = wide.off[i].way.or_else(|| self.grid_way_of(from, to));
                        wide.off[i].to = b;
                        wide.off[i].way = way;
                        wide.off[i].press = press;
                        wide.off[i].share = share;
                        released += usize::from(matches!(press, Takeoff::Release { .. }));
                    }
                }
                Verdict::Reaim(b) | Verdict::Taught(b) => {
                    let (from, to) = (wide.off[i].from, wide.off[i].to);
                    if b != from && !wide.out(from as usize).any(|x| x == b) {
                        // [retarget] A re-aimed link was walked at its new landing (its own line); a taught one came down along
                        // the walk-off's own line: its way.
                        wide.off[i].way = match v {
                            Verdict::Taught(_) => wide.off[i].way.or_else(|| self.grid_way_of(from, to)),
                            _ => None,
                        };
                        wide.off[i].to = b;
                        moved += 1;
                    }
                }
                Verdict::Cut => none += 1,
                Verdict::Blocked => blocked[i] = true,
            }
        }
        let removed = blocked.iter().filter(|&&b| b).count();
        if removed > 0 {
            let mut i = 0;
            wide.off.retain(|_| {
                i += 1;
                !blocked[i - 1]
            });
        }
        if moved > 0 {
            wide.off.sort_by_key(|l| (l.from, l.to));
            wide.off.dedup_by_key(|l| (l.from, l.to));
        }
        (todo.len(), moved, none, released, held, removed)
    }

    /// [feed 67] [`super::Body::drop_lands`]: every Drop link ([`Self::all_drops`]) walked by the client's body to its landing
    /// ([`walk_drop`], on the three [`STARTS`] lines across it). A link is kept when a line arrives (or a line's body cannot
    /// move). When none does and a line's body comes to stand on another floor lower than the take-off, and the body walked at
    /// that floor arrives there, the link is re-aimed at it; else it is cut. Returns the links to cut (take-off, direction,
    /// rank 0) and the re-aims (take-off, direction, new landing). On the machine's threads; in link order.
    ///
    /// [A377] And the links kept or re-aimed whose take-off is a lip by where the body stops ([`Self::stops_below`]), as
    /// (take-off, landing).
    ///
    /// [deep walk-offs] And the deep landings to keep beside a way down within the harmless fall ([`Self::keep_deep_landings`]).
    #[allow(clippy::type_complexity)]
    pub(super) fn landing_drops(
        &self,
        wide: &Wide,
        world: &Collision,
    ) -> (Cuts, Reaims, Lips, DropStats, Vec<(Vec3, Vec3)>, DeepAlso) {
        let links = self.all_drops(wide);
        let mut stats = DropStats { walked: links.len(), ..DropStats::default() };
        if links.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new(), stats, Vec::new(), Vec::new());
        }
        let s = self.body.controller().1;
        let start = self.body.walk_start();
        let verdicts = parallel(&links, |&(a, d, from, to)| {
            let (v, held, deep) = self.drop_verdict(world, &s, start, a, from, wide.links[a as usize][d]);
            let lip = match v {
                Verdict::Keep => self.stops_below(world, &s, start, from, to),
                Verdict::Reaim(b) | Verdict::Taught(b) => self.stops_below(world, &s, start, from, self.position(b)),
                // [A1b] Let go at its release, or a press flown to its rest (a crouched walk-off, a jump): the body rests on its
                // landing, no lip by a stop past it.
                Verdict::Pressed(..) | Verdict::Cut | Verdict::Blocked => false,
            };
            (v, lip, held, deep)
        });
        stats.releases_held_thin = verdicts.iter().filter(|(_, _, held, _)| *held).count();
        stats.past_harmless_fall = verdicts.iter().filter(|(_, _, _, deep)| deep.is_some()).count();
        let mut cut = Vec::new();
        let mut reaim = Vec::new();
        let mut lips = Vec::new();
        // [D2] The Drop links a walk overshot past the harmless fall that no way within it took down, re-aimed to the walk's deep
        // landing (take-off, that landing; for the build's log).
        let mut deep_kept = Vec::new();
        // [deep walk-offs] The deep landings a way within the harmless fall went before: kept beside it, never cut.
        let mut deep_also = Vec::new();
        for (&(a, d, from, _), (v, lip, _, deep)) in links.iter().zip(verdicts) {
            if let Some(deep) = deep {
                if v == deep.verdict() {
                    deep_kept.push((from, self.position(deep.floor())));
                } else {
                    // [retarget] As a re-aim: a taught landing was come down to along the drop's own way.
                    deep_also.push((a, deep.floor(), matches!(deep, DeepLanding::Taught(_)).then(|| GridWay::of_dir(d))));
                }
            }
            match v {
                Verdict::Keep if lip => lips.push((a, wide.links[a as usize][d])),
                Verdict::Keep => {}
                Verdict::Pressed(b, press, share) => {
                    match press {
                        Takeoff::Release { .. } => stats.released += 1,
                        Takeoff::WalkOffCrouched => stats.walked_off_crouched += 1,
                        _ => stats.jumped_down += 1,
                    }
                    reaim.push((a, d, b, Some((press, share)), Some(GridWay::of_dir(d))));
                }
                Verdict::Reaim(b) | Verdict::Taught(b) => {
                    let taught = matches!(v, Verdict::Taught(_));
                    stats.taught += usize::from(taught);
                    // [retarget] A taught landing was come down to along the drop's own way, not on the line to it.
                    reaim.push((a, d, b, None, taught.then(|| GridWay::of_dir(d))));
                    if lip {
                        lips.push((a, b));
                    }
                }
                Verdict::Cut => cut.push((a, d, 0.0)),
                Verdict::Blocked => {
                    stats.blocked += 1;
                    cut.push((a, d, 0.0));
                }
            }
        }
        stats.cut = cut.len() - stats.blocked;
        stats.past_harmless_fall_kept = deep_kept.len();
        stats.past_harmless_fall_also = deep_also.len();
        (cut, reaim, lips, stats, deep_kept, deep_also)
    }

    /// [deep walk-offs] The deep landings [`Self::landing_drops`] kept beside a way down within the harmless fall: each an
    /// off-mesh [`LinkKind::GapDrop`] standing walk-off ([`Takeoff::WalkOff`]) from the take-off to the walk's deep landing, at
    /// its true depth, as before 000f re-aimed the Drop link there (its own slot now holds the way within the fall). The route
    /// prices it by the client's fall ([`super::Body::held_floors`]: a pool's loss is a long way) and refuses it where the fall
    /// kills the body ([`super::Reach::lethal_fall`]), so it is the last way down; players take such falls (School batteries
    /// (-3.30, 26.09, 106.97) -> (-5.55, 17.10, 106.72), 8.99 m). None where the take-off links to that landing already.
    /// Run after [`Self::reaim_drops`]. Returns each link made (take-off, landing), for the build's log.
    pub(super) fn keep_deep_landings(&self, wide: &mut Wide, deep: &DeepAlso) -> Vec<(Vec3, Vec3)> {
        use super::gap::{Aim, OffLink};
        let mut added: Vec<OffLink> = Vec::new();
        for &(a, g, way) in deep {
            if wide.out(a as usize).any(|x| x == g) || added.iter().any(|l| l.from == a && l.to == g) {
                continue;
            }
            added.push(OffLink { from: a, to: g, kind: LinkKind::GapDrop, press: Takeoff::WalkOff, share: Share::UNFLOWN, walk: None, aim: Aim::STRAIGHT, way });
        }
        let made = added.iter().map(|l| (self.position(l.from), self.position(l.to))).collect();
        if !added.is_empty() {
            wide.off.extend(added);
            wide.off.sort_by_key(|l| (l.from, l.to));
        }
        made
    }

    /// [A377] Whether a Drop link's take-off at `from` is a lip by where the client's body stops, its landing `to` being under
    /// [`LIP_DROP_M`] down (deeper, the link alone makes it one, [`WalkGrid::is_lip`]): on any of the [`STARTS`] lines (a body
    /// that strays to a side of the link) the body walked at the landing and let go there ([`stop_at_drop`]) stands
    /// [`LIP_DROP_M`] or more under the take-off, or is still falling. A one-cell ledge under a parapet is such a landing: School
    /// (13.45 21.69 83.47) drops 0.97 m to the ledge at z 83.97 and the body carried over it falls to 16.05.
    fn stops_below(&self, world: &Collision, s: &Settings, start: WalkStart, from: Vec3, to: Vec3) -> bool {
        if from.y - to.y >= LIP_DROP_M {
            return false;
        }
        let len = flat(from, to).max(1e-3);
        let side = Vec3::new(-(to.z - from.z) / len, 0.0, (to.x - from.x) / len);
        STARTS
            .iter()
            .any(|&off| stop_at_drop(world, s, start, from + side * off, to + side * off).is_none_or(|feet| from.y - feet.y >= LIP_DROP_M))
    }

    /// [F2] The floor a drop's flight rests on ([`walk_off_rest`], [`released_rest`]): the floor under the feet within
    /// [`ARRIVE_FLAT_M`] across and [`ARRIVE_HEIGHT_M`] in height, the feet within [`ARRIVE_HEIGHT_M`] of its height.
    fn rest_floor(&self, feet: Vec3) -> Option<u32> {
        self.locate_within(feet, ARRIVE_FLAT_M, ARRIVE_HEIGHT_M).filter(|&g| (self.position(g).y - feet.y).abs() <= ARRIVE_HEIGHT_M)
    }

    /// [feed 67] One link of [`Self::landing_drops`] from `a` (at `from`) to the floor `b`. [F2] Each way down is judged by
    /// where its flight RESTS ([`walk_off_rest`]: the forward key let go on the landing step), on `b`'s own top
    /// ([`WalkGrid::own_top`]): the link is kept where a walk line's rest is there.
    ///
    /// Returns the verdict, whether a release was not made off a thin top, and [D2] the deep landing where a walk line came to
    /// rest deeper than the harmless fall ([`super::Body::max_drop`]).
    fn drop_verdict(&self, world: &Collision, s: &Settings, start: WalkStart, a: u32, from: Vec3, b: u32) -> (Verdict, bool, Option<DeepLanding>) {
        let (mut held, mut deep) = (false, None);
        let v = self.drop_verdict_holding(world, s, start, a, from, b, &mut held, &mut deep);
        (v, held, deep)
    }

    /// [F2] Whether the take-off `a` is a thin top along `way` (flat, unit): its level (within a step,
    /// [`super::Body::step`]) ends within [`THIN_TOP_M`] back along the way, cell by cell (a railing's top, a held floor on a
    /// rail, the railings census's tops). A release off such a top is not made until the port's and the exe's drop flights
    /// agree at rims (#2180's review: off the Vector Laboratory spawner walkway's railing, 114512, the port's release rests on the
    /// ledge, the exe's body never leaves the railing, its top at 21.55 where the grid's floor is at 21.74).
    pub(super) fn on_thin_top(&self, a: u32, way: Vec3) -> bool {
        let p = self.position(a);
        let h = self.height[a as usize];
        let level = |q: Vec3| {
            let i = ((q.x - self.x0) / self.cell).floor() as i32;
            let k = ((q.z - self.z0) / self.cell).floor() as i32;
            self.cell_index(i, k)
                .is_some_and(|c| (self.start[c]..self.start[c + 1]).any(|f| (self.height[f as usize] - h).abs() <= self.body.step))
        };
        let mut d = self.cell;
        while d <= THIN_TOP_M + 1e-4 {
            if !level(p - way * d) {
                return true;
            }
            d += self.cell;
        }
        false
    }

    /// [`Self::drop_verdict`], `held` set when a release was not made off a thin top, `deep` to the deep landing where a walk line
    /// came to rest deeper than the harmless fall.
    #[allow(clippy::too_many_arguments)]
    fn drop_verdict_holding(
        &self,
        world: &Collision,
        s: &Settings,
        start: WalkStart,
        a: u32,
        from: Vec3,
        b: u32,
        held: &mut bool,
        deep: &mut Option<DeepLanding>,
    ) -> Verdict {
        let to = self.position(b);
        let len = flat(from, to).max(1e-3);
        let side = Vec3::new(-(to.z - from.z) / len, 0.0, (to.x - from.x) / len);
        let rest = |stance: Stance, speed: f32, off: f32, at: Vec3| walk_off_rest(world, s, start, stance, speed, from + side * off, at + side * off);
        let on_top = |w: &DropWalk, of: u32| matches!(*w, DropWalk::LandsAt(feet, _) if self.rest_floor(feet).is_some_and(|g| self.own_top(of, g)));
        let walks = STARTS.map(|off| rest(Stance::Standing, WALK_MPS, off, to));
        if walks.iter().any(|w| on_top(w, b)) {
            return Verdict::Keep;
        }
        // [blocked walk-offs] A line whose body cannot move says nothing, unless no gait passes on any line and no jump lands.
        if walks.contains(&DropWalk::NoMove) {
            return if self.no_client_way(world, s, &walks, a, from, to) { Verdict::Blocked } else { Verdict::Keep };
        }
        // [D2] A walk line that overshoots the link's own landing (a ledge) and comes to rest deeper than the harmless fall under
        // the take-off ([`super::Body::max_drop`]) is re-aimed there only when no other client way takes the body down within it
        // (the release, a walk line's landing, the crouched walk-off, a jump off the edge): the deep landing is a real fall, kept
        // at its true depth as the last way down, never cut. School day_4's ledge (15.95, 21.69, 125.47) -> (15.70, 20.72,
        // 125.22), crossed by players, was re-aimed to the ground 5.94 m down (landing at 10.8 m/s) though the walk-off at release
        // lands on the ledge.
        let within_fall = |g: u32| from.y - self.position(g).y <= self.body.max_drop;
        // [D2] The walk's deep landing, judged on every line before any way within the fall returns ([deep walk-offs]: it is
        // kept beside that way): the first line whose rest walked at arrives there, else the first line's own deep rest.
        let deeper = |w: &DropWalk| match *w {
            DropWalk::LandsAt(feet, _) => self.rest_floor(feet).filter(|&g| g != a && self.position(g).y < from.y - self.body.step && !within_fall(g)),
            _ => None,
        };
        *deep = walks
            .iter()
            .zip(STARTS)
            .find_map(|(w, off)| deeper(w).filter(|&g| on_top(&rest(Stance::Standing, WALK_MPS, off, self.position(g)), g)))
            .map(DeepLanding::Reaim)
            .or_else(|| walks.iter().find_map(deeper).map(DeepLanding::Taught));
        for (w, off) in walks.iter().zip(STARTS) {
            let DropWalk::LandsAt(feet, _) = *w else { continue };
            let Some(g) = self.rest_floor(feet) else { continue };
            if g == a || self.position(g).y >= from.y - self.body.step || !within_fall(g) {
                continue;
            }
            if on_top(&rest(Stance::Standing, WALK_MPS, off, self.position(g)), g) {
                return Verdict::Reaim(g);
            }
        }
        // [A389] No cut on the walk alone (a link is cut only where no client action takes the body down there): a walk line's own landing on a floor lower down, then the crouched walk-off, then the jumps off the edge.
        // [D2] Within the harmless fall.
        let lower = |feet: Vec3| self.rest_floor(feet).filter(|&g| g != a && self.position(g).y < from.y - self.body.step && within_fall(g));
        // [A1b] The walk-off at release: on the link's own landing before a walk line's landing elsewhere (the shallow ledge a
        // steady walk overshoots), on another lower floor before the crouched walk-off.
        let thin = self.on_thin_top(a, Vec3::new((to.x - from.x) / len, 0.0, (to.z - from.z) / len));
        let flown = self.released(world, s, start, from, to, lower);
        *held = thin && flown.is_some();
        let release = if thin { None } else { flown };
        if let Some((g, press, share)) = release
            && self.own_top(b, g)
        {
            return Verdict::Pressed(g, press, share);
        }
        if let Some(b) = walks.iter().find_map(|w| if let DropWalk::LandsAt(feet, _) = *w { lower(feet) } else { None }) {
            return Verdict::Taught(b);
        }
        if let Some((g, press, share)) = release {
            return Verdict::Pressed(g, press, share);
        }
        // [taught presses] The crouched walk-off and the jumps off the edge are other key scripts than the standing walk-off: the
        // link carries the one that takes the body down (before, it was stored as a standing walk-off, and kept as the plain
        // Drop link where it came down on the link's own top).
        for off in STARTS {
            let w = rest(Stance::Crouched, CROUCH_MPS, off, to);
            if on_top(&w, b) {
                return Verdict::Pressed(b, Takeoff::WalkOffCrouched, Share::UNFLOWN);
            }
            if let DropWalk::LandsAt(feet, _) = w
                && let Some(g) = lower(feet)
            {
                return Verdict::Pressed(g, Takeoff::WalkOffCrouched, Share::UNFLOWN);
            }
        }
        if let Some((g, press, share)) = self.jumped_down(world, s, a, from, ((to.x - from.x) / len, (to.z - from.z) / len)) {
            return Verdict::Pressed(g, press, share);
        }
        // [D2] No way down within the harmless fall: the walk's own deep landing, at its true depth (re-aimed where walking at it
        // arrives, else taught), as before.
        // [blocked walk-offs] No client way tried goes down; where a wall or a ceiling no gait passes stands on every line, the link
        // is Blocked (a walk-off is then removed, not kept as one no way lands from).
        deep.map_or_else(|| if self.lines_blocked(world, s, from, to) { Verdict::Blocked } else { Verdict::Cut }, DeepLanding::verdict)
    }

    /// [blocked walk-offs] Whether no client way goes down a walk-off from `a` (at `from`) toward `to`, its standing walk lines
    /// `walks` (the [`STARTS`] lines): no line's body moves ([`DropWalk::NoMove`]), a wall or a ceiling no gait passes stands on
    /// every line ([`walk_line_blocked`]), and no jump off the edge lands ([`Self::jumped_down`]).
    fn no_client_way(&self, world: &Collision, s: &Settings, walks: &[DropWalk; 3], a: u32, from: Vec3, to: Vec3) -> bool {
        let len = flat(from, to).max(1e-3);
        walks.iter().all(|w| *w == DropWalk::NoMove)
            && self.lines_blocked(world, s, from, to)
            && self.jumped_down(world, s, a, from, ((to.x - from.x) / len, (to.z - from.z) / len)).is_none()
    }

    /// [blocked walk-offs] Whether a wall or a ceiling no gait passes stands on every [`STARTS`] line from `from` toward `to`
    /// ([`walk_line_blocked`]).
    fn lines_blocked(&self, world: &Collision, s: &Settings, from: Vec3, to: Vec3) -> bool {
        let len = flat(from, to).max(1e-3);
        let side = Vec3::new(-(to.z - from.z) / len, 0.0, (to.x - from.x) / len);
        STARTS.iter().all(|&off| walk_line_blocked(world, s, &self.body, from + side * off, to + side * off))
    }

    /// [blocked walk-offs] The walk-offs of this grid as it stands (its Drop links and walk-off GapDrops) whose landing pass verdict
    /// is [`Verdict::Blocked`] ([`Self::drop_verdict`], on `world`: the walk's own, the game's shapes where the build has them), as
    /// (take-off, landing, kind). For a built grid's census (what the build's landing pass now takes away); on the machine's
    /// threads.
    ///
    /// Each with whether a standing walk line's body could not move (the verdict before this rule: such a link was kept; else
    /// it was cut, a Drop link kept only as a floor's only way, a walk-off kept as it was).
    pub fn walk_offs_with_no_client_way(&self, world: &Collision) -> Vec<(Vec3, Vec3, LinkKind, bool)> {
        let s = self.body.controller().1;
        let start = self.body.walk_start();
        let links: Vec<(u32, u32, LinkKind)> = (0..self.height.len() as u32)
            .flat_map(|a| {
                self.links(a)
                    .filter(move |&(b, kind)| match kind {
                        LinkKind::Drop => true,
                        LinkKind::GapDrop => self.off_link(a, b).is_some_and(|l| l.press.is_walk_off()),
                        _ => false,
                    })
                    .map(move |(b, kind)| (a, b, kind))
            })
            .collect();
        let none = parallel(&links, |&(a, b, _)| {
            let (from, to) = (self.position(a), self.position(b));
            let blocked = self.drop_verdict(world, &s, start, a, from, b).0 == Verdict::Blocked;
            let len = flat(from, to).max(1e-3);
            let side = Vec3::new(-(to.z - from.z) / len, 0.0, (to.x - from.x) / len);
            let held = blocked
                && STARTS.iter().any(|&off| walk_off_rest(world, &s, start, Stance::Standing, WALK_MPS, from + side * off, to + side * off) == DropWalk::NoMove);
            (blocked, held)
        });
        links
            .iter()
            .zip(none)
            .filter(|(_, n)| n.0)
            .map(|(&(a, b, kind), n)| (self.position(a), self.position(b), kind, n.1))
            .collect()
    }

    /// [A1b] The walk-off at release from `from` towards `to` ([`release_point`], [`released_rest`]): crouched at [`CROUCH_MPS`],
    /// then standing at [`WALK_MPS`]. A stance's release lands when its own body rests on a floor `lower` finds. It is flown for
    /// the press's neighbourhood ([`jump::neighbourhood`]: the release `lead` m earlier, the line `side` m to the right, the
    /// heading turned, the fastest kit's speed; a walk has no clip to lift, so a world-height neighbour flies as the release
    /// itself), and its share is the neighbours resting on that floor. The stance with the larger share, crouched on a tie; None
    /// when neither lands.
    fn released(&self, world: &Collision, s: &Settings, start: WalkStart, from: Vec3, to: Vec3, lower: impl Fn(Vec3) -> Option<u32>) -> Option<(u32, Takeoff, Share)> {
        let len = flat(from, to).max(1e-3);
        let way = Vec3::new((to.x - from.x) / len, 0.0, (to.z - from.z) / len);
        let right = Vec3::new(way.z, 0.0, -way.x);
        let mut best: Option<(u32, Takeoff, Share)> = None;
        for (stance, speed) in [(Stance::Crouched, CROUCH_MPS), (Stance::Standing, WALK_MPS)] {
            let Some(at) = release_point(world, s, start, stance, speed, from, way) else { continue };
            let Some(b) = released_rest(world, s, start, stance, speed, from, way, at, to.y).and_then(&lower) else { continue };
            let mut bits: u16 = 0;
            for (i, n) in jump::neighbourhood().into_iter().enumerate() {
                let itself = n.lead == 0.0 && n.side == 0.0 && n.turn == 0.0 && !n.fastest;
                let lands = itself || {
                    let (sin, cos) = n.turn.sin_cos();
                    let heading = Vec3::new(way.x * cos - way.z * sin, 0.0, way.x * sin + way.z * cos);
                    let v = if n.fastest { speed * self.body.fastest_walk_cap / jump::WALK_CAP_MS } else { speed };
                    released_rest(world, s, start, stance, v, from + right * n.side, heading, (at - n.lead).max(0.0), to.y).and_then(&lower)
                        == Some(b)
                };
                bits |= u16::from(lands) << i;
            }
            let share = Share::of_mask(super::gap::Mask::of_bits(bits).expect("a bit a neighbour"));
            let press = Takeoff::Release { at: PressLead::from_metres(at), crouched: stance == Stance::Crouched };
            if best.is_none_or(|(_, _, kept)| share.rank() > kept.rank()) {
                best = Some((b, press, share));
            }
        }
        best
    }

    /// [A389] The floor lower than the take-off `a` (at `from`) by more than a step that a jump off it along `way` lands on and
    /// stays on: the short jump at the body's walk cap, then the sprint jump (every variant on one floor), pressed at each of the
    /// gap links' leads before the take-off's centre (A385's presses, `jump::neighbour_flights` at the press itself, the keys
    /// let go at the landing, A385-FORMAT §1); the first that lands. None from a floor only a crouched body fits (no jump is
    /// pressed from a crouch, rule (e): 0x2FBDC0 -> 0x2FC230 -> `on_ground` 0x7EF90). [taught presses] With the press that lands
    /// and its share, its neighbourhood's flights resting on the same floor ([`jump::neighbourhood`], A385-FORMAT §2), as every
    /// flown press's: the link carries them.
    fn jumped_down(&self, world: &Collision, s: &Settings, a: u32, from: Vec3, way: (f32, f32)) -> Option<(u32, Takeoff, Share)> {
        if !self.can_stand(a) {
            return None;
        }
        let kits = jump::Kits { body: jump::WALK_CAP_MS, fastest: self.body.fastest_walk_cap };
        let own = jump::neighbourhood()[0];
        let below = |feet: Vec3| {
            self.locate_within(feet, self.body.radius, ARRIVE_HEIGHT_M).filter(|&b| {
                let y = self.position(b).y;
                (y - feet.y).abs() <= ARRIVE_HEIGHT_M && y < from.y - self.body.step
            })
        };
        let n = ((GAP_PRESS_MAX_M - GAP_PRESS_MIN_M) / GAP_PRESS_STEP_M).round() as usize;
        // The floor every flight of a press (one, or one per sprint variant) rests on, if one: a sprint press lands only when
        // every variant does (A385-FORMAT §1, rule (b)), here on one top: [body physics S5b] the first variant's floor, every
        // other variant on its top ([`WalkGrid::own_top`]). One floor node was too narrow once the flights fly on to the rest:
        // a variant rocks a cell over first (Cologne TDM (-57.68, 33.24, 16.38): Sprint 1.2's variant 4 rests 0.25 m across and
        // 0.24 m up from the others; in the exe every variant rests there and stays).
        // [jump down] Flown on after the clip's `end_jump` until the body is down (every pass's rule, [`Self::fall_ground`]): off
        // an edge about 3 m high the body is still in the air at the clip's last ground step ([`jump::STAY_STEPS`]).
        let rests = |kind: jump::Kind, lead: f32, n: jump::Neighbour| -> Option<u32> {
            let flights = jump::neighbour_flights_for(world, s, from, way, kind, lead, n, kits, self.fall_ground())?;
            let floors: Vec<Option<u32>> = flights
                .iter()
                .map(|st| jump::landing_and_rest(st, |f| below(f.feet).is_some()).and_then(|(_, rest)| below(rest.feet)))
                .collect();
            floors.first().copied().flatten().filter(|&b| floors.iter().all(|&f| f.is_some_and(|g| self.own_top(b, g))))
        };
        for kind in [jump::Kind::Walk, jump::Kind::Sprint] {
            for i in 0..=n {
                let lead = GAP_PRESS_MIN_M + GAP_PRESS_STEP_M * i as f32;
                let Some(b) = rests(kind, lead, own) else { continue };
                let mut bits: u16 = 0;
                for (k, nb) in jump::neighbourhood().into_iter().enumerate() {
                    let lands = k == 0 || rests(kind, lead, nb).is_some_and(|g| self.own_top(b, g));
                    bits |= u16::from(lands) << k;
                }
                let share = Share::of_mask(super::gap::Mask::of_bits(bits).expect("a bit a neighbour"));
                let press = match kind {
                    jump::Kind::Sprint => Takeoff::Sprint(PressLead::from_metres(lead)),
                    _ => Takeoff::Press(PressLead::from_metres(lead)),
                };
                return Some((b, press, share));
            }
        }
        None
    }

    /// [feed 67] Re-aims Drop links ([`Self::landing_drops`]): the link (take-off, direction) goes to the new landing, in its
    /// direction's slot when the landing is in that neighbour cell, else as an off-mesh walk-off ([`LinkKind::GapDrop`],
    /// [`super::gap::Takeoff::WalkOff`]) in place of the slot's link (none added where the take-off already has one there).
    /// Returns how many were re-aimed.
    ///
    /// [A1b] A re-aim with a release press is always an off-mesh GapDrop carrying it and its share (in its landing's cell too);
    /// an off-mesh link the take-off already has to that landing takes the press (a release one only at a higher share).
    /// [taught presses] The same for every press but the standing walk-off: the crouched walk-off and a jump off the edge too
    /// (the "release" counts of [`ReaimPairs`] count them all).
    ///
    /// One link a (take-off, landing) pair: the links this pass adds are kept apart from `wide.off` (whose lookup,
    /// [`Wide::out`], is a binary search over it sorted) until the end, and looked up there too. Two re-aims of one take-off
    /// onto one landing (two directions, or a walk-off and a release) make one link ([`ReaimPairs`] counts them): a release
    /// press wins over a walk-off (its landing is flown, the walk-off's share is not), and of two releases the higher share.
    pub(super) fn reaim_drops(&self, wide: &mut Wide, reaim: &Reaims) -> (usize, ReaimPairs) {
        use super::gap::{Aim, OffLink, Share, Takeoff};
        let mut added: Vec<OffLink> = Vec::new();
        let mut index: std::collections::HashMap<(u32, u32), usize> = std::collections::HashMap::new();
        let mut pairs = ReaimPairs::default();
        for &(a, d, b, release, way) in reaim {
            let old = wide.links[a as usize][d];
            if let Some((press, share)) = release {
                wide.links[a as usize][d] = NONE;
                let link = OffLink { from: a, to: b, kind: LinkKind::GapDrop, press, share, walk: None, aim: Aim::STRAIGHT, way };
                // [taught presses] A press is any but the standing walk-off: a release, the crouched walk-off, a jump off the edge.
                let pressed = |p: Takeoff| p != Takeoff::WalkOff;
                if let Some(l) = wide.off.iter_mut().find(|l| l.from == a && l.to == b) {
                    // Of two presses the higher share, as within the pass; a walk-off takes the press.
                    if pressed(l.press) && l.share.rank() >= share.rank() {
                        pairs.earlier_kept += 1;
                    } else {
                        pairs.earlier_replaced += 1;
                        *l = OffLink { kind: LinkKind::GapDrop, press, share, walk: None, aim: Aim::STRAIGHT, way, ..*l };
                    }
                } else if let Some(&i) = index.get(&(a, b)) {
                    pairs.merged += 1;
                    match added[i].press {
                        p if pressed(p) && added[i].share.rank() >= share.rank() => pairs.releases_kept_first += 1,
                        p if pressed(p) => {
                            pairs.releases_kept_later += 1;
                            added[i] = link;
                        }
                        _ => {
                            pairs.release_over_walk_off += 1;
                            added[i] = link;
                        }
                    }
                } else if !wide.out(a as usize).any(|x| x == b) {
                    index.insert((a, b), added.len());
                    added.push(link);
                }
                continue;
            }
            if self.cell_of[old as usize] == self.cell_of[b as usize] {
                wide.links[a as usize][d] = b;
                continue;
            }
            wide.links[a as usize][d] = NONE;
            if let Some(&i) = index.get(&(a, b)) {
                pairs.merged += 1;
                if added[i].press != Takeoff::WalkOff {
                    pairs.release_over_walk_off += 1;
                }
            } else if !wide.out(a as usize).any(|x| x == b) {
                index.insert((a, b), added.len());
                added.push(OffLink { from: a, to: b, kind: LinkKind::GapDrop, press: Takeoff::WalkOff, share: Share::UNFLOWN, walk: None, aim: Aim::STRAIGHT, way });
            }
        }
        if !added.is_empty() {
            wide.off.extend(added);
            wide.off.sort_by_key(|l| (l.from, l.to));
        }
        (reaim.len(), pairs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::WalkerBehaviour;

    fn quad(x0: f32, x1: f32, z0: f32, z1: f32, y: f32) -> [Triangle; 2] {
        let t = |v: [Vec3; 3]| Triangle { v, walker: WalkerBehaviour::BySlope };
        [
            t([Vec3::new(x0, y, z0), Vec3::new(x0, y, z1), Vec3::new(x1, y, z1)]),
            t([Vec3::new(x0, y, z0), Vec3::new(x1, y, z1), Vec3::new(x1, y, z0)]),
        ]
    }

    /// [walk-face chain] The client's body (the port's tick) walks up a `walk` face of 84.0 deg (normal up 0.1045, within
    /// `WALK_FACE_CLIMB_NORMAL_Y`) to its top, 6 m and 20 m high, at a walk and slowly, and does not leave the ground at the foot
    /// of one of 84.34 deg (0.0987, past it): the limit the grid's `walk` floors name a body down to (`grid::walk_chain_low`).
    /// (At 6 m the change is between 84.2 deg, 0.1004, climbed, and 84.3 deg, 0.0987, not; a face that close to it is climbed
    /// slowly.)
    #[test]
    fn the_client_walks_up_a_walk_face_to_its_climb_limit_and_no_steeper() {
        let walk = WalkerBehaviour::Walk;
        let t = |v: [Vec3; 3]| Triangle { v, walker: walk };
        let limit = super::super::WALK_FACE_CLIMB_NORMAL_Y;
        assert!(84f32.to_radians().cos() > limit && 84.34f32.to_radians().cos() < limit);
        for (up, climbs) in [(84f32.to_radians().cos(), true), (84.34f32.to_radians().cos(), false)] {
            for high in [6.0f32, 20.0] {
                for mps in [WALK_MPS, 1.0] {
                    let run = high * up / (1.0 - up * up).sqrt();
                    let mut tris: Vec<Triangle> = quad(0.0, 10.0, 0.0, 10.0, 0.0).to_vec();
                    let (a, b, c, d) = (Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 0.0, 10.0), Vec3::new(5.0 + run, high, 10.0), Vec3::new(5.0 + run, high, 0.0));
                    tris.push(t([a, b, c]));
                    tris.push(t([a, c, d]));
                    let (e, f) = (Vec3::new(9.9, high, 10.0), Vec3::new(9.9, high, 0.0));
                    tris.push(t([d, c, e]));
                    tris.push(t([d, e, f]));
                    let world = Collision::build(&tris);
                    let s = Settings::LIVE;
                    let mut c = body_at(&world, WalkStart::Clear, Stance::Standing, Vec3::new(4.5, 0.0, 5.0));
                    let mut top = 0.0f32;
                    for _ in 0..600 {
                        let input = TickInput {
                            walk: Vec3::new(mps * LAND_DT, 0.0, 0.0),
                            air_control: Vec3::ZERO,
                            crouch: CrouchInput::Tested { key: false, legs_broken: false },
                            sprinting: false,
                        };
                        c.tick(&world, LAND_DT, &s, input);
                        top = top.max(c.feet().y);
                    }
                    let case = format!("up {up:.4}, {high} m, {mps} m/s: highest feet {top:.2}");
                    if climbs {
                        assert!(top >= high - 0.05, "climbs: {case}");
                    } else {
                        assert!(top < 0.05, "stays at the foot: {case}");
                    }
                }
            }
        }
    }

    /// Two re-aims of one take-off onto one landing out of its neighbour cells (two directions; a walk-off and a release, in
    /// either order) make ONE off-mesh link, the release's press winning, and the grid's file reads it back (the off-mesh
    /// links' rule, [`WalkGrid::off_links_problem`]): Rudnya day 2's bench grid held two GapDrop links of one pair and every
    /// read refused it.
    #[test]
    fn two_reaims_onto_one_landing_make_one_link_and_the_file_reads_back() {
        let mut tris = Vec::new();
        tris.extend(quad(0.0, 10.0, 0.0, 10.0, 0.0));
        let grid = WalkGrid::build(&tris, [0.0, 0.0, 10.0, 10.0], 0.25, super::super::Body::default());
        let a = grid.locate(Vec3::new(2.0, 0.0, 2.0), 0.3).unwrap();
        let b = grid.locate(Vec3::new(6.0, 0.0, 6.0), 0.3).unwrap();
        let released = Takeoff::Release { at: PressLead::from_metres(0.0), crouched: false };
        let release = Some((released, Share::landed(9)));
        for (first, second) in [(None, None), (release, None), (None, release)] {
            let mut wide = grid.link(&vec![false; grid.len()]);
            // Off-mesh links already there from later floors: a link this pass adds sits after them, out of a binary search's
            // reach until the pass sorts (the old pass then added the pair again).
            let later = |k: u32| super::super::gap::OffLink {
                from: b + k,
                to: a,
                kind: LinkKind::GapDrop,
                press: Takeoff::WalkOff,
                share: Share::UNFLOWN,
                walk: None,
                aim: super::super::gap::Aim::STRAIGHT,
                way: None,
            };
            wide.off = (1..=3).map(later).collect();
            let ds: Vec<usize> = (0..8).filter(|&d| wide.links[a as usize][d] != NONE).take(2).collect();
            assert_eq!(ds.len(), 2);
            let reaim: Reaims = vec![(a, ds[0], b, first, None), (a, ds[1], b, second, None)];
            let (reaimed, pairs) = grid.reaim_drops(&mut wide, &reaim);
            assert_eq!((reaimed, pairs.merged, pairs.release_over_walk_off), (2, 1, usize::from(first.or(second).is_some())));
            let pair: Vec<_> = wide.off.iter().filter(|l| (l.from, l.to) == (a, b)).collect();
            assert_eq!(pair.len(), 1, "{first:?} {second:?}: {pair:?}");
            let press = if first.or(second).is_some() { released } else { Takeoff::WalkOff };
            assert_eq!(pair[0].press, press);
            assert!(wide.off.windows(2).all(|w| (w[0].from, w[0].to) < (w[1].from, w[1].to)));
            let mut built = WalkGrid::build(&tris, [0.0, 0.0, 10.0, 10.0], 0.25, super::super::Body::default());
            built.off = wide.off.clone();
            // The floors' off-mesh marks, as the build sets them with its links.
            for l in &built.off {
                built.flags[l.from as usize] |= super::super::OFF_OUT;
                built.flags[l.to as usize] |= super::super::OFF_IN;
            }
            assert_eq!(built.off_links_problem(), None);
            let path = std::env::temp_dir().join(format!("srp-reaim-{}-{}.bin", std::process::id(), first.is_some()));
            built.write(&path, 7).unwrap();
            let back = WalkGrid::read(&path, 7, super::super::Body::default());
            std::fs::remove_file(&path).ok();
            assert!(back.is_ok(), "{:?}", back.err());
        }
        // A release onto a pair whose release link is already in the table (an earlier pass's): the higher share stays.
        for (there, new, kept) in [(12, 9, 12), (9, 12, 12)] {
            let mut wide = grid.link(&vec![false; grid.len()]);
            let d = (0..8).find(|&d| wide.links[a as usize][d] != NONE).unwrap();
            wide.off = vec![super::super::gap::OffLink {
                from: a,
                to: b,
                kind: LinkKind::GapDrop,
                press: released,
                share: Share::landed(there),
                walk: None,
                aim: super::super::gap::Aim::STRAIGHT,
                way: None,
            }];
            let (_, pairs) = grid.reaim_drops(&mut wide, &vec![(a, d, b, Some((released, Share::landed(new))), None)]);
            assert_eq!((wide.off.len(), wide.off[0].share), (1, Share::landed(kept)));
            assert_eq!((pairs.earlier_kept, pairs.earlier_replaced), if there >= new { (1, 0) } else { (0, 1) });
        }
        // Two releases onto one landing: the higher share is the link, whichever came first.
        for (first, second) in [(9, 12), (12, 9)] {
            let mut wide = grid.link(&vec![false; grid.len()]);
            let ds: Vec<usize> = (0..8).filter(|&d| wide.links[a as usize][d] != NONE).take(2).collect();
            let r = |n| Some((released, Share::landed(n)));
            // [retarget] Each release walked along its own drop's way: the link keeps the winner's.
            let w = |i: usize| Some(GridWay::of_dir(ds[i]));
            let (_, pairs) = grid.reaim_drops(&mut wide, &vec![(a, ds[0], b, r(first), w(0)), (a, ds[1], b, r(second), w(1))]);
            let pair: Vec<_> = wide.off.iter().filter(|l| (l.from, l.to) == (a, b)).collect();
            assert_eq!((pair.len(), pair[0].share, pairs.merged), (1, Share::landed(12), 1));
            assert_eq!(pair[0].way, w(usize::from(second > first)));
        }
    }

    /// A step off a slab onto the ground 2 m below is followed; one whose landing lies under a deck that goes on past it is
    /// not (the body stays on the deck).
    #[test]
    fn a_walk_off_reaches_its_landing_and_a_landing_under_a_deck_is_not_reached() {
        let mut tris = Vec::new();
        tris.extend(quad(-10.0, 10.0, -10.0, 10.0, 0.0));
        tris.extend(quad(-5.0, 0.0, -2.0, 2.0, 2.0));
        let world = Collision::build(&tris);
        let s = Settings::LIVE;
        assert!(follows(&world, &s, Vec3::new(-0.25, 2.0, 0.0), Vec3::new(0.25, 0.0, 0.0)));
        let mut deck = tris.clone();
        deck.extend(quad(0.0, 5.0, -2.0, 2.0, 2.0));
        let world = Collision::build(&deck);
        assert!(!follows(&world, &s, Vec3::new(-0.25, 2.0, 0.0), Vec3::new(0.25, 0.0, 0.0)));
    }

    /// A body that walks off and comes down under the grid's landing (here the ground lies 1 m lower than the landing says) did
    /// take the drop: the link is followed (fx-bench's drift score of the first rule, which cut such links).
    #[test]
    fn a_walk_off_that_lands_under_its_landing_is_followed() {
        let mut tris = Vec::new();
        tris.extend(quad(-10.0, 10.0, -10.0, 10.0, 0.0));
        tris.extend(quad(-5.0, 0.0, -2.0, 2.0, 2.0));
        let world = Collision::build(&tris);
        assert!(follows(&world, &Settings::LIVE, Vec3::new(-0.25, 2.0, 0.0), Vec3::new(0.25, 1.0, 0.0)));
    }

    /// [feed 67] The client's body walked off a slab stands on the ground below: it arrives. With a ledge 1 m down over the
    /// landing it lands on the ledge (the old rule passed it: it went down), and walked at the ledge it arrives there. Under a
    /// deck that goes on it never leaves the take-off's level.
    #[test]
    fn walk_drop_arrives_lands_on_a_ledge_or_is_held() {
        let s = Settings::LIVE;
        let mut tris = Vec::new();
        tris.extend(quad(-10.0, 10.0, -10.0, 10.0, 0.0));
        tris.extend(quad(-5.0, 0.0, -2.0, 2.0, 2.0));
        let (from, to) = (Vec3::new(-0.25, 2.0, 0.0), Vec3::new(0.25, 0.0, 0.0));
        let start = WalkStart::Clear;
        assert_eq!(walk_drop(&Collision::build(&tris), &s, start, from, to), DropWalk::Arrives);

        let mut ledge = tris.clone();
        ledge.extend(quad(0.0, 3.0, -2.0, 2.0, 1.0));
        let world = Collision::build(&ledge);
        let DropWalk::LandsAt(feet, landed) = walk_drop(&world, &s, start, from, to) else { panic!("lands elsewhere") };
        assert_eq!(landed.all, landed.walkable, "no steep ground");
        assert_eq!(landed.all.also, None, "a 1 m drop's landing hurts nothing");
        let speed = landed.all.most;
        assert!((feet.y - 1.0).abs() < 0.1, "{feet:?}");
        // [S6] A 1 m drop lands at the speed `step_down` 0x7E1B0 gives a fall from rest: 0.05 m a tick while slower than
        // 2 m/s (five ticks, 0.25 m), then the speed's own step, gaining 0.49 m/s every tick throughout; the tick that
        // starts at 4.41 m/s is the one that reaches 1 m down (0.887 m before it).
        let c = body_at(&world, start, Stance::Standing, from);
        assert_eq!(speed, fall_from_rest_lands_at(1.0, &s, c.gravity, c.tick), "{speed}");
        assert_eq!(walk_drop(&world, &s, start, from, Vec3::new(0.25, 1.0, 0.0)), DropWalk::Arrives);

        let mut deck = tris.clone();
        deck.extend(quad(0.0, 5.0, -2.0, 2.0, 2.0));
        assert_eq!(walk_drop(&Collision::build(&deck), &s, start, from, to), DropWalk::Held);
    }

    /// [taught presses] A jump off the edge gives the link its press (the short jump or the sprint jump at a gap lead) and its
    /// share (the neighbourhood's flights resting on the same floor), not only the floor it comes down on: slabs 2 m and [jump
    /// down] 3 m and 6 m over the ground, jumped off along +x. Off 3 m the body is still in the air at the clip's last ground
    /// step (0.25 m up at the 24th step); the flight falls on to the ground.
    #[test]
    fn a_jump_off_the_edge_gives_its_press_and_share() {
        for high in [2.0f32, 3.0, 6.0] {
            let mut tris = Vec::new();
            tris.extend(quad(0.0, 10.0, 0.0, 10.0, 0.0));
            tris.extend(quad(0.0, 5.0, 0.0, 10.0, high));
            let grid = WalkGrid::build(&tris, [0.0, 0.0, 10.0, 10.0], 0.25, super::super::Body::default());
            let world = Collision::build(&tris);
            let a = grid.locate(Vec3::new(4.8, high, 5.0), 0.3).unwrap();
            let (b, press, share) =
                grid.jumped_down(&world, &Settings::LIVE, a, grid.position(a), (1.0, 0.0)).unwrap_or_else(|| panic!("{high} m: lands below"));
            assert!(grid.position(b).y < 0.1 && grid.position(b).x > 5.0, "{high} m: {:?}", grid.position(b));
            let lead = match press {
                Takeoff::Press(lead) | Takeoff::Sprint(lead) => lead.metres(),
                other => panic!("{high} m: a jump's press: {other:?}"),
            };
            assert!((GAP_PRESS_MIN_M - 1e-3..=GAP_PRESS_MAX_M + 1e-3).contains(&lead), "{high} m: {lead}");
            assert!(share.flown().is_some_and(|n| n >= 1), "{high} m: {share:?}");
        }
    }

    /// [item 2] A walk off a deck 17 m up falls onto a 65 deg face under it (steeper than the 55 deg a body stands on, met
    /// head-on enough: `-(n . v^) >= already_sliding_on_slope_dp`), slides down it and lands on the ground at its foot. The face
    /// is a landing the client counts at the tick's starting speed (`docs/notes/fall_damage.md` "steep slopes", PROVEN:
    /// `step_down` 0x7E527..0x7E6D1, the speed kept at 0x7E660): 12.25 m/s, 25 of gravity's 0.49 m/s ticks, then the ground at
    /// 11.53 m/s. Both landings' pain (1.55) empties a full pool; the ground's alone (0.64) does not, which is how the build
    /// judged the drop before (DROP 0x5067_0008).
    #[test]
    fn a_steep_face_under_a_walk_off_is_a_landing_that_hurts() {
        use super::super::held_floors::{Landed, Landings, SteepClosed};
        let s = Settings::LIVE;
        let start = WalkStart::Clear;
        let (deck, x0, top, foot_x) = (17.0f32, -2.0f32, 14.0f32, 1.0f32);
        let foot = top - (foot_x - x0) * 65f32.to_radians().tan();
        let mut tris = quad(-5.0, 0.0, -2.0, 2.0, deck).to_vec();
        let t = |v: [Vec3; 3]| Triangle { v, walker: WalkerBehaviour::BySlope };
        tris.push(t([Vec3::new(x0, top, -2.0), Vec3::new(x0, top, 2.0), Vec3::new(foot_x, foot, 2.0)]));
        tris.push(t([Vec3::new(x0, top, -2.0), Vec3::new(foot_x, foot, 2.0), Vec3::new(foot_x, foot, -2.0)]));
        tris.extend(quad(foot_x, 15.0, -2.0, 2.0, foot));
        let world = Collision::build(&tris);
        let (from, aim) = (Vec3::new(-0.25, deck, 0.0), Vec3::new(0.25, deck - 1.0, 0.0));
        let walk = walk_drop(&world, &s, start, from, aim);
        let DropWalk::LandsAt(feet, landed) = walk else { panic!("{walk:?}") };
        assert!((feet.y - foot).abs() < 0.01 && feet.x > foot_x, "on the ground at the face's foot: {feet:?}");
        let c = body_at(&world, start, Stance::Standing, from);
        let (gain, face) = (c.gravity * c.tick, landed.all.most);
        assert!((face / gain - 25.0).abs() < 1e-3, "the face's landing at a tick's starting speed: {landed:?}");
        let ground = landed.all.also.expect("the ground's landing hurts too");
        assert!(ground < face && (ground - 11.53).abs() < 0.01, "{landed:?}");
        assert_eq!(landed.walkable, Landings::one(ground), "on walkable ground, the ground's landing alone");
        let pain = |l: Landings| l.pain(1.0, super::super::SecondLanding::Adds);
        assert!((pain(landed.all) - 1.554).abs() < 0.002 && (pain(landed.walkable) - 0.637).abs() < 0.002, "{landed:?}");
        assert!(SteepClosed::of(from, aim, feet, landed).is_some(), "closed by the face's landing");
        // A walk whose only landing is on walkable ground is no steep closing, however hard.
        let flat = Landed::NONE.and(crate::controller::tick::LandingGround::Walkable, 15.0);
        assert!(SteepClosed::of(from, aim, feet, flat).is_none());
    }

    /// [S6] The downward speed at the start of the tick on which a fall from rest reaches `height` m down, by `step_down`
    /// 0x7E1B0's rule (`Controller::step_down`): a tick that starts slower than `on_ground_max_vertical_speed` moves
    /// `additional_down_speed`, any other its speed x `tick`; gravity adds `gravity` x `tick` to the speed every tick. A fall
    /// off a level floor only: it leaves out the slide acceleration's part of the move and the step's rise, both 0 there.
    fn fall_from_rest_lands_at(height: f32, s: &Settings, gravity: f32, tick: f32) -> f32 {
        let (mut v, mut fell) = (0.0f32, 0.0f32);
        loop {
            let d = if s.on_ground_max_vertical_speed > v.abs() { s.additional_down_speed } else { -v * tick };
            if fell + d >= height {
                return -v;
            }
            fell += d;
            v -= gravity * tick;
        }
    }

    /// [A377] A body walked off a slab at a ledge 0.9 m down and let go there stays on a wide ledge, and goes on down to the
    /// ground 3 m under the slab from a ledge one cell (0.25 m) deep: there the take-off is a lip by where the body stops.
    #[test]
    fn a_body_let_go_on_a_narrow_ledge_falls_past_it() {
        let s = Settings::LIVE;
        let start = WalkStart::Clear;
        let (from, to) = (Vec3::new(-0.25, 3.0, 0.0), Vec3::new(0.125, 2.1, 0.0));
        let slab = |ledge_x1: f32| {
            let mut tris = Vec::new();
            tris.extend(quad(-10.0, 10.0, -10.0, 10.0, 0.0));
            tris.extend(quad(-5.0, 0.0, -2.0, 2.0, 3.0));
            tris.extend(quad(0.0, ledge_x1, -2.0, 2.0, 2.1));
            Collision::build(&tris)
        };
        let wide = stop_at_drop(&slab(3.0), &s, start, from, to).expect("stands");
        assert!((wide.y - 2.1).abs() < 0.1, "{wide:?}");
        let narrow = stop_at_drop(&slab(0.25), &s, start, from, to).expect("stands");
        assert!(narrow.y < 0.1, "{narrow:?}");
    }

    /// [F2] The drop's script flown to its rest ([`walk_off_rest`]: forward held to the landing step, then let go): off a slab
    /// 3 m up (its face down to the ground), a ledge 0.9 m down holds the body when it is 3 m deep, and one 0.25 m deep does not (the body rests on the
    /// ground); off the slab onto open ground it rests on the ground within reach of the landing. Under a deck it is held.
    #[test]
    fn a_drop_is_judged_where_its_flight_rests() {
        let s = Settings::LIVE;
        let start = WalkStart::Clear;
        let (from, to) = (Vec3::new(-0.25, 3.0, 0.0), Vec3::new(0.125, 2.1, 0.0));
        let slab = |ledge_x1: Option<f32>| {
            let mut tris = Vec::new();
            tris.extend(quad(-10.0, 10.0, -10.0, 10.0, 0.0));
            tris.extend(quad(-5.0, 0.0, -2.0, 2.0, 3.0));
            // The slab's face down its edge (a body let go at the landing stands against it, not under the slab's top).
            let t = |v: [Vec3; 3]| Triangle { v, walker: WalkerBehaviour::BySlope };
            let (a, b, c, d) = (Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 2.0), Vec3::new(0.0, 3.0, 2.0), Vec3::new(0.0, 3.0, -2.0));
            tris.extend([t([a, b, c]), t([a, c, d])]);
            if let Some(x1) = ledge_x1 {
                tris.extend(quad(0.0, x1, -2.0, 2.0, 2.1));
            }
            Collision::build(&tris)
        };
        let rest = |world: &Collision, to: Vec3| walk_off_rest(world, &s, start, Stance::Standing, WALK_MPS, from, to);
        let DropWalk::LandsAt(wide, _) = rest(&slab(Some(3.0)), to) else { panic!("rests") };
        assert!((wide.y - 2.1).abs() < 0.05, "{wide:?}");
        let DropWalk::LandsAt(narrow, _) = rest(&slab(Some(0.25)), to) else { panic!("rests") };
        assert!(narrow.y < 0.05, "{narrow:?}");
        let ground = Vec3::new(0.25, 0.0, 0.0);
        let DropWalk::LandsAt(open, _) = rest(&slab(None), ground) else { panic!("rests") };
        assert!(open.y < 0.05 && flat(open, ground) < 2.0, "{open:?}");
        let mut deck = Vec::new();
        deck.extend(quad(-10.0, 10.0, -10.0, 10.0, 0.0));
        deck.extend(quad(-5.0, 5.0, -2.0, 2.0, 3.0));
        assert_eq!(rest(&Collision::build(&deck), Vec3::new(0.25, 0.0, 0.0)), DropWalk::Held);
    }

    /// [F2] A take-off on a top 0.5 m deep along the way (a rail's, a railing's) is a thin top, whose releases are held; one on
    /// the edge of a slab 5 m deep is not.
    #[test]
    fn a_rails_top_is_thin_and_a_slabs_edge_is_not() {
        let mut tris = Vec::new();
        tris.extend(quad(-10.0, 10.0, -10.0, 10.0, 0.0));
        tris.extend(quad(-6.0, -1.0, -2.0, 2.0, 2.0));
        tris.extend(quad(2.0, 2.5, -2.0, 2.0, 2.0));
        let grid = WalkGrid::build(&tris, [-10.0, -10.0, 10.0, 10.0], 0.25, super::super::Body::default());
        let way = Vec3::new(1.0, 0.0, 0.0);
        let rail = grid.locate_within(Vec3::new(2.375, 2.0, 0.0), 0.2, 0.3).expect("a floor on the rail");
        let slab = grid.locate_within(Vec3::new(-1.125, 2.0, 0.0), 0.2, 0.3).expect("a floor on the slab's edge");
        assert!(grid.on_thin_top(rail, way));
        assert!(!grid.on_thin_top(slab, way));
    }

    /// [F2] A drop on a map's game shapes (`Collision::build_exe`): its flights printed (`cargo test -- --ignored --nocapture`):
    /// `DROP_MAP` a format 6 map file, `DROP_FROM` and `DROP_TO` as `x,y,z` (feet). The walk carried on ([`walk_drop`]), the
    /// script flown to its rest ([`walk_off_rest`]) standing and crouched, and the let-go at the release ([`release_point`],
    /// [`released_rest`]) standing and crouched.
    #[test]
    #[ignore]
    fn print_a_maps_drop() {
        let var = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
        let point = |k: &str| {
            let v: Vec<f32> = var(k).split(',').map(|x| x.trim().parse().unwrap()).collect();
            Vec3::new(v[0], v[1], v[2])
        };
        let map = crate::MapData::from_bytes(&std::fs::read(var("DROP_MAP")).unwrap()).unwrap();
        let world = Collision::build_exe(&map).expect("a format 6 map");
        let s = Settings { wall_normal: crate::controller::tick::WallNormal::Rays, ..Settings::LIVE };
        let start = WalkStart::Clear;
        let (from, to) = (point("DROP_FROM"), point("DROP_TO"));
        println!("walk carried on: {:?}", walk_drop(&world, &s, start, from, to));
        for (stance, speed) in [(Stance::Standing, WALK_MPS), (Stance::Crouched, CROUCH_MPS)] {
            println!("{stance:?} script to rest: {:?}", walk_off_rest(&world, &s, start, stance, speed, from, to));
            let way = Vec3::new(to.x - from.x, 0.0, to.z - from.z) * (1.0 / flat(from, to).max(1e-4));
            let at = release_point(&world, &s, start, stance, speed, from, way);
            let rest = at.and_then(|at| released_rest(&world, &s, start, stance, speed, from, way, at, to.y));
            println!("{stance:?} release at {at:?}: rests at {rest:?}");
        }
    }

    /// [blocked walk-offs] The walk line's room ([`walk_line_blocked`]): a take-off at (0, 2, 0) on a deck, the landing at
    /// (0.8, 1, 0) past its edge at x 0.4. A wall across the way 0.3 m ahead, from the deck to 3 m over it, blocks every gait; so
    /// does a ceiling 1.0 m over the deck (under the crouched body, 1.1 m). A ledge 0.5 m high ahead (under the walk tick's
    /// step, 0.8 m) and a ceiling 1.5 m over the deck (over the crouched body) do not; nor does an open way.
    #[test]
    fn a_wall_or_a_low_ceiling_on_the_way_blocks_every_gait_and_a_ledge_or_a_high_ceiling_does_not() {
        let t = |v: [Vec3; 3]| Triangle { v, walker: WalkerBehaviour::BySlope };
        // A wall across x = 0.3 from height `lo` to `hi`, both windings (faces both ways).
        let wall = |lo: f32, hi: f32| {
            let (a, b, c, d) = (Vec3::new(0.3, lo, -2.0), Vec3::new(0.3, lo, 2.0), Vec3::new(0.3, hi, 2.0), Vec3::new(0.3, hi, -2.0));
            [t([a, b, c]), t([a, c, d]), t([a, c, b]), t([a, d, c])]
        };
        // A ceiling slab's underside at height `y`, facing down, over the way.
        let ceiling = |y: f32| {
            let (a, b, c, d) = (Vec3::new(-1.0, y, -2.0), Vec3::new(2.0, y, -2.0), Vec3::new(2.0, y, 2.0), Vec3::new(-1.0, y, 2.0));
            [t([a, b, c]), t([a, c, d])]
        };
        let deck = |extra: &[Triangle]| {
            let mut tris = Vec::new();
            tris.extend(quad(-3.0, 0.4, -2.0, 2.0, 2.0));
            tris.extend(quad(0.4, 3.0, -2.0, 2.0, 1.0));
            tris.extend_from_slice(extra);
            Collision::build(&tris)
        };
        let body = crate::Body::client();
        let s = Settings::LIVE;
        let (from, to) = (Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.8, 1.0, 0.0));
        let blocked = |extra: &[Triangle]| walk_line_blocked(&deck(extra), &s, &body, from, to);
        assert!(!blocked(&[]), "an open way");
        assert!(blocked(&wall(2.0, 5.0)), "a wall from the deck to over the head");
        assert!(!blocked(&wall(2.0, 2.5)), "a ledge under the step");
        assert!(blocked(&ceiling(3.0)), "a ceiling 1.0 m over the deck");
        assert!(!blocked(&ceiling(3.5)), "a ceiling 1.5 m over the deck");
    }

    /// A take-off boxed in by walls (the body cannot move from it on any line) says nothing of the drop: the link is kept.
    #[test]
    fn a_boxed_in_take_off_keeps_its_drop() {
        let wall = |a: Vec3, b: Vec3| {
            let t = |v: [Vec3; 3]| Triangle { v, walker: WalkerBehaviour::BySlope };
            let (a2, b2) = (a + Vec3::new(0.0, 3.0, 0.0), b + Vec3::new(0.0, 3.0, 0.0));
            [t([a, b, b2]), t([a, b2, a2])]
        };
        let mut tris = Vec::new();
        tris.extend(quad(-10.0, 10.0, -10.0, 10.0, 0.0));
        tris.extend(quad(-5.0, 0.0, -2.0, 2.0, 2.0));
        let y = 2.0;
        for (a, b) in [((-0.6, -0.35), (0.1, -0.35)), ((0.1, -0.35), (0.1, 0.35)), ((0.1, 0.35), (-0.6, 0.35)), ((-0.6, 0.35), (-0.6, -0.35))] {
            tris.extend(wall(Vec3::new(a.0, y, a.1), Vec3::new(b.0, y, b.1)));
        }
        let world = Collision::build(&tris);
        assert!(follows(&world, &Settings::LIVE, Vec3::new(-0.25, 2.0, 0.0), Vec3::new(0.25, 0.0, 0.0)));
    }

    /// [body physics S5b] The drop check walks every Drop link, out of a floor no seed reaches at that point too: a deck 3 m up
    /// with no way onto it, the seeds on the ground; its edge's Drops are walked (before, only the Drops out of reached floors
    /// were, and a deck reached only through links an earlier pass moved kept its Drops unchecked).
    #[test]
    fn the_drop_check_walks_the_drops_out_of_a_floor_no_seed_reaches() {
        let mut tris = Vec::new();
        tris.extend(quad(0.0, 12.0, 0.0, 10.0, 0.0));
        tris.extend(quad(0.0, 5.0, 0.0, 10.0, 3.0));
        let body = super::super::Body { drop_check: true, drop_lands: true, held_floors: true, ..super::super::Body::client() };
        let seeds = [Vec3::new(9.0, 0.0, 5.0)];
        let (_, stats) = WalkGrid::build_kept_with_stats(&tris, [0.0, 0.0, 12.0, 10.0], 0.25, body, &seeds, &[]);
        assert!(stats.drops_walked.walked > 0, "the deck's Drops are walked: {:?}", stats.drops_walked);
    }

    /// [D2] A deck with a ledge 1 m under its edge, 0.6 m wide, over the ground 5.6 m under the deck (past the harmless fall,
    /// `Body::max_drop`): the client's body walking off the deck overshoots the ledge and comes to rest on the ground. The Drop
    /// link's own way down is the way within the harmless fall (the release, the crouched walk-off, a jump): along the deck's
    /// edge (away from its ends, where a link along the edge has no ledge under it) every take-off goes down onto the ledge.
    /// [deep walk-offs] The deep rest is kept beside it, never cut: an off-mesh GapDrop walk-off to the ground, which the route
    /// prices by the fall's damage, so the way onto the ledge is the cheaper. Every rest past the fall is counted, kept at depth
    /// or beside a way within it.
    #[test]
    fn a_walk_off_that_overshoots_a_ledge_past_the_harmless_fall_is_not_re_aimed_there() {
        let high = 5.6f32;
        let mut tris = Vec::new();
        tris.extend(quad(0.0, 5.0, 0.0, 10.0, high));
        tris.extend(quad(5.0, 5.6, 0.0, 10.0, high - 1.0));
        tris.extend(quad(5.6, 12.0, 0.0, 10.0, 0.0));
        let seeds = [Vec3::new(2.0, high, 5.0)];
        let body = super::super::Body { drop_check: true, drop_lands: true, held_floors: true, ..super::super::Body::client() };
        assert!(high > body.max_drop && 1.0 > body.step);
        let (grid, stats) = WalkGrid::build_kept_with_stats(&tris, [0.0, 0.0, 12.0, 10.0], 0.25, body, &seeds, &[]);
        let case = format!("{:?}", stats.drops_walked);
        assert!(stats.drops_walked.past_harmless_fall > 0, "a walk off the deck rests past the harmless fall: {case}");
        let (mut onto_ledge, mut to_ground) = (0, 0);
        for a in (0..grid.len() as u32).filter(|&a| {
            let p = grid.position(a);
            (p.y - high).abs() < 0.05 && (1.0..=9.0).contains(&p.z)
        }) {
            let downs: Vec<(u32, LinkKind, f32)> =
                grid.links(a).map(|(b, kind)| (b, kind, high - grid.position(b).y)).filter(|&(_, _, f)| f > body.step).collect();
            let within = downs.iter().filter(|&&(_, _, f)| f <= body.max_drop).map(|&(b, kind, _)| grid.link_cost(a, b, kind));
            let within = within.fold(f32::INFINITY, f32::min);
            for &(b, kind, f) in downs.iter().filter(|&&(_, _, f)| f > body.max_drop) {
                assert_eq!(kind, LinkKind::GapDrop, "{case}: {:?} -> {:?} {f:.2} m", grid.position(a), grid.position(b));
                assert!(within < grid.link_cost(a, b, kind), "{case}: {:?}: the deep fall is not the dearer way down", grid.position(a));
                to_ground += 1;
            }
            onto_ledge += usize::from(within.is_finite());
        }
        assert!(onto_ledge > 0, "the deck's edge goes down onto the ledge: {case}");
        assert!(to_ground > 0, "the deep rest is kept beside the ledge: {case}");
        let walked = stats.drops_walked;
        assert_eq!(walked.past_harmless_fall_kept + walked.past_harmless_fall_also, walked.past_harmless_fall, "{case}");
        assert_eq!(stats.drops_past_harmless_cut.len(), walked.past_harmless_fall_kept, "{case}");
        assert!(!stats.drops_past_harmless_also.is_empty() && stats.drops_past_harmless_also.len() <= walked.past_harmless_fall_also, "{case}");
    }
}

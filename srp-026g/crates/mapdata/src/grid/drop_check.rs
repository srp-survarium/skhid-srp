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

use super::{DIRS, LIP_DROP_M, LinkKind, NONE, SEED_M, SPAWN_M, Triangle, Vec3, WalkGrid, Wide};
use super::controlled::{STARTS, Stance, WalkStart, body_at, out_of_walls, parallel};
use super::gap::{GAP_PRESS_MAX_M, GAP_PRESS_MIN_M, GAP_PRESS_STEP_M};
use crate::controller::{jump, Collision};
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
/// when that is less.
fn left_take_off(from_y: f32, to_y: f32) -> f32 {
    from_y - LEAVE_M.min(0.5 * (from_y - to_y).max(0.0))
}

/// [feed 67] The step of [`walk_drop`], s: the client's 50 ms step, the recorded steps of the traced match being 50 ms apart
/// (docs/notes/exactness_rig.md, the chain replay: 8,993 steps in 50 chains).
const LAND_DT: f32 = 0.05;
/// [feed 67] The steps a body has to leave the take-off in (3 s), and then to come to stand on the landing (2 s): the bounds of
/// the exactness session's walk (FINDINGS-FEED-exactness 2026-11-12 09:56Z), our choice, not client values.
const LEAVE_STEPS: usize = 60;
const LAND_STEPS: usize = 40;
/// [feed 67] Within this of the to-point across, m, the body walks on along the link's line instead of steering at it (a body
/// stopped over a short drop's to-point stays on the rim: the exactness session's first walk, 1,877 of 6,000 on Chemical Plant,
/// FINDINGS-FEED-exactness 2026-11-12 09:56Z). The walk's own value, our choice: under a step at 3.5 m/s (0.175 m) the body
/// would stop short of the to-point instead.
const WALK_ON_WITHIN_M: f32 = 0.3;
/// [feed 67] A body has arrived standing within this of the to-point across, m, and [`ARRIVE_HEIGHT_M`] of its height: the
/// exactness walk's arrival (FINDINGS-FEED-exactness 2026-11-12 09:56Z), our choice; across, two cells (0.5 m) of the grid; in
/// height, the walk check's [`super::controlled`] `AT_HEIGHT_M`.
const ARRIVE_FLAT_M: f32 = 0.5;
const ARRIVE_HEIGHT_M: f32 = 0.2;
/// [A377] The steps of the client's stop blend: on the k-th step after the walk key is let go the walk is 1 - k/6 of the speed
/// it had (docs/notes/walk_blend.md: the 0.3 s blend, 0x921804, a linear weight; 82 clean stops counted, 1 - k/6).
const STOP_STEPS: usize = 6;

/// [feed 67] How the client's body walking off a Drop link ends ([`walk_drop`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DropWalk {
    /// Standing on the to-point's floor ([`ARRIVE_FLAT_M`] across, [`ARRIVE_HEIGHT_M`] in height) within [`LAND_STEPS`] of
    /// leaving the take-off's level.
    Arrives,
    /// Left the take-off's level and came to stand elsewhere: its feet there (a ledge partway, a floor past the landing).
    LandsAt(Vec3),
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
    for n in 1..=LEAVE_STEPS + LAND_STEPS {
        let feet = c.feet();
        let d = flat(feet, to);
        let walk = if d <= WALK_ON_WITHIN_M {
            line * (speed * LAND_DT)
        } else {
            Vec3::new((to.x - feet.x) / d, 0.0, (to.z - feet.z) / d) * (speed * LAND_DT).min(d)
        };
        c.tick(world, LAND_DT, s, TickInput {
            walk,
            air_control: Vec3::ZERO,
            crouch: CrouchInput::Tested { key, legs_broken: false },
            sprinting: false,
        });
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
                    return if standing { DropWalk::LandsAt(feet) } else { DropWalk::Unsettled };
                }
            }
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

/// [feed 67] [`WalkGrid::landing_drops`]'s links to cut (take-off, direction, rank 0), re-aims (take-off, direction, new
/// landing) and [A377] lips by where the body stops (take-off, landing).
type Cuts = Vec<(u32, usize, f32)>;
type Reaims = Vec<(u32, usize, u32)>;
type Lips = Vec<(u32, u32)>;

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
}

/// [feed 67] What [`super::Body::drop_lands`] does with one Drop link.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Verdict {
    /// A line arrives, or a line's body cannot move (it says nothing).
    #[default]
    Keep,
    /// The body lands on another floor of the grid, and walked at that floor it arrives there: the link goes there.
    Reaim(u32),
    /// [A389] No walk line arrives or re-aims as [`Self::Reaim`]; a client way down (a walk line's own landing, the crouched
    /// walk-off, a jump off the edge) comes to stand on this floor: the link goes there.
    Taught(u32),
    /// No client way down tried stands on a floor of the grid: the walk lines, the crouched walk-off, the short and sprint
    /// jumps at the gap leads.
    Cut,
}

impl WalkGrid {
    /// [feed 67] The Drop links out of a floor a body gets to from the seeds or a spawn, as (take-off, direction, from, to).
    fn reached_drops(&self, wide: &Wide, seeds: &[Vec3], spawns: &[Vec3]) -> Vec<(u32, usize, Vec3, Vec3)> {
        let n = self.height.len();
        let reached = self.reached_floors(wide, seeds, spawns);
        (0..n)
            .filter(|&a| reached[a])
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
    /// to cut it by; counted). Returns (walked, re-aimed, standing on no floor).
    pub(super) fn walk_off_links(&self, wide: &mut Wide, world: &Collision, seeds: &[Vec3], spawns: &[Vec3]) -> (usize, usize, usize) {
        let reached = self.reached_floors(wide, seeds, spawns);
        let todo: Vec<usize> = (0..wide.off.len())
            .filter(|&i| {
                let l = wide.off[i];
                l.press == super::gap::Takeoff::WalkOff && reached[l.from as usize]
            })
            .collect();
        if todo.is_empty() {
            return (0, 0, 0);
        }
        let s = self.body.controller().1;
        let start = self.body.walk_start();
        let verdicts = parallel(&todo, |&i| {
            let l = wide.off[i];
            self.drop_verdict(world, &s, start, l.from, self.position(l.from), self.position(l.to))
        });
        let (mut moved, mut none) = (0, 0);
        for (&i, v) in todo.iter().zip(verdicts) {
            match v {
                Verdict::Keep => {}
                Verdict::Reaim(b) | Verdict::Taught(b) => {
                    let from = wide.off[i].from;
                    if b != from && !wide.out(from as usize).any(|x| x == b) {
                        wide.off[i].to = b;
                        moved += 1;
                    }
                }
                Verdict::Cut => none += 1,
            }
        }
        if moved > 0 {
            wide.off.sort_by_key(|l| (l.from, l.to));
            wide.off.dedup_by_key(|l| (l.from, l.to));
        }
        (todo.len(), moved, none)
    }

    /// [feed 67] [`super::Body::drop_lands`]: every Drop link out of a reached floor walked by the client's body to its landing
    /// ([`walk_drop`], on the three [`STARTS`] lines across it). A link is kept when a line arrives (or a line's body cannot
    /// move). When none does and a line's body comes to stand on another floor lower than the take-off, and the body walked at
    /// that floor arrives there, the link is re-aimed at it; else it is cut. Returns the links to cut (take-off, direction,
    /// rank 0) and the re-aims (take-off, direction, new landing). On the machine's threads; in link order.
    ///
    /// [A377] And the links kept or re-aimed whose take-off is a lip by where the body stops ([`Self::stops_below`]), as
    /// (take-off, landing).
    pub(super) fn landing_drops(
        &self,
        wide: &Wide,
        world: &Collision,
        seeds: &[Vec3],
        spawns: &[Vec3],
    ) -> (Cuts, Reaims, Lips, DropStats) {
        let links = self.reached_drops(wide, seeds, spawns);
        let mut stats = DropStats { walked: links.len(), ..DropStats::default() };
        if links.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new(), stats);
        }
        let s = self.body.controller().1;
        let start = self.body.walk_start();
        let verdicts = parallel(&links, |&(a, _, from, to)| {
            let v = self.drop_verdict(world, &s, start, a, from, to);
            let lip = match v {
                Verdict::Keep => self.stops_below(world, &s, start, from, to),
                Verdict::Reaim(b) | Verdict::Taught(b) => self.stops_below(world, &s, start, from, self.position(b)),
                Verdict::Cut => false,
            };
            (v, lip)
        });
        let mut cut = Vec::new();
        let mut reaim = Vec::new();
        let mut lips = Vec::new();
        for (&(a, d, _, _), (v, lip)) in links.iter().zip(verdicts) {
            match v {
                Verdict::Keep if lip => lips.push((a, wide.links[a as usize][d])),
                Verdict::Keep => {}
                Verdict::Reaim(b) | Verdict::Taught(b) => {
                    stats.taught += usize::from(matches!(v, Verdict::Taught(_)));
                    reaim.push((a, d, b));
                    if lip {
                        lips.push((a, b));
                    }
                }
                Verdict::Cut => cut.push((a, d, 0.0)),
            }
        }
        stats.cut = cut.len();
        (cut, reaim, lips, stats)
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

    /// [feed 67] One link of [`Self::landing_drops`].
    fn drop_verdict(&self, world: &Collision, s: &Settings, start: WalkStart, a: u32, from: Vec3, to: Vec3) -> Verdict {
        let len = flat(from, to).max(1e-3);
        let side = Vec3::new(-(to.z - from.z) / len, 0.0, (to.x - from.x) / len);
        let walks = STARTS.map(|off| walk_drop(world, s, start, from + side * off, to + side * off));
        if walks.iter().any(|w| matches!(w, DropWalk::Arrives | DropWalk::NoMove)) {
            return Verdict::Keep;
        }
        for (w, off) in walks.iter().zip(STARTS) {
            let DropWalk::LandsAt(feet) = *w else { continue };
            let Some(b) = self.locate_within(feet, ARRIVE_FLAT_M, ARRIVE_HEIGHT_M) else { continue };
            let at = self.position(b);
            if b == a || (at.y - feet.y).abs() > ARRIVE_HEIGHT_M || at.y >= from.y - self.body.step {
                continue;
            }
            if walk_drop(world, s, start, from + side * off, at + side * off) == DropWalk::Arrives {
                return Verdict::Reaim(b);
            }
        }
        // [A389] No cut on the walk alone (a link is cut only where no client action takes the body down there): a walk line's own landing on a floor lower down, then the crouched walk-off, then the jumps off the edge.
        let lower = |feet: Vec3| {
            self.locate_within(feet, ARRIVE_FLAT_M, ARRIVE_HEIGHT_M).filter(|&b| {
                let y = self.position(b).y;
                b != a && (y - feet.y).abs() <= ARRIVE_HEIGHT_M && y < from.y - self.body.step
            })
        };
        if let Some(b) = walks.iter().find_map(|w| if let DropWalk::LandsAt(feet) = *w { lower(feet) } else { None }) {
            return Verdict::Taught(b);
        }
        for off in STARTS {
            match walk_drop_as(world, s, start, Stance::Crouched, CROUCH_MPS, from + side * off, to + side * off) {
                DropWalk::Arrives => return Verdict::Keep,
                DropWalk::LandsAt(feet) => {
                    if let Some(b) = lower(feet) {
                        return Verdict::Taught(b);
                    }
                }
                _ => {}
            }
        }
        match self.jumped_down(world, s, a, from, ((to.x - from.x) / len, (to.z - from.z) / len)) {
            Some(b) if (self.position(b).y - to.y).abs() <= ARRIVE_HEIGHT_M && flat(self.position(b), to) <= ARRIVE_FLAT_M => Verdict::Keep,
            Some(b) => Verdict::Taught(b),
            None => Verdict::Cut,
        }
    }

    /// [A389] The floor lower than the take-off `a` (at `from`) by more than a step that a jump off it along `way` lands on and
    /// stays on: the short jump at the body's walk cap, then the sprint jump (every variant on one floor), pressed at each of the
    /// gap links' leads before the take-off's centre (A385's presses, `jump::neighbour_flights` at the press itself, the keys
    /// let go at the landing, A385-FORMAT §1); the first that lands. None from a floor only a crouched body fits (no jump is
    /// pressed from a crouch, rule (e): 0x2FBDC0 -> 0x2FC230 -> `on_ground` 0x7EF90).
    fn jumped_down(&self, world: &Collision, s: &Settings, a: u32, from: Vec3, way: (f32, f32)) -> Option<u32> {
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
        for kind in [jump::Kind::Walk, jump::Kind::Sprint] {
            for i in 0..=n {
                let lead = GAP_PRESS_MIN_M + GAP_PRESS_STEP_M * i as f32;
                let Some(flights) = jump::neighbour_flights(world, s, from, way, kind, lead, own, kits) else { continue };
                let floors: Vec<Option<u32>> = flights
                    .iter()
                    .map(|st| jump::landing_and_rest(st, |f| below(f.feet).is_some()).and_then(|(_, rest)| below(rest.feet)))
                    .collect();
                // A sprint press lands only when every variant does (A385-FORMAT §1, rule (b)), here on one floor.
                if let Some(&Some(b)) = floors.first()
                    && floors.iter().all(|&f| f == Some(b))
                {
                    return Some(b);
                }
            }
        }
        None
    }

    /// [feed 67] Re-aims Drop links ([`Self::landing_drops`]): the link (take-off, direction) goes to the new landing, in its
    /// direction's slot when the landing is in that neighbour cell, else as an off-mesh walk-off ([`LinkKind::GapDrop`],
    /// [`super::gap::Takeoff::WalkOff`]) in place of the slot's link (none added where the take-off already has one there).
    /// Returns how many were re-aimed.
    pub(super) fn reaim_drops(&self, wide: &mut Wide, reaim: &[(u32, usize, u32)]) -> usize {
        let mut added = false;
        for &(a, d, b) in reaim {
            let old = wide.links[a as usize][d];
            if self.cell_of[old as usize] == self.cell_of[b as usize] {
                wide.links[a as usize][d] = b;
                continue;
            }
            wide.links[a as usize][d] = NONE;
            if !wide.out(a as usize).any(|x| x == b) {
                wide.off.push(super::gap::OffLink { from: a, to: b, kind: LinkKind::GapDrop, press: super::gap::Takeoff::WalkOff, share: super::gap::Share::UNFLOWN, walk: None, aim: super::gap::Aim::STRAIGHT });
                added = true;
            }
        }
        if added {
            wide.off.sort_by_key(|l| (l.from, l.to));
        }
        reaim.len()
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
        let DropWalk::LandsAt(feet) = walk_drop(&world, &s, start, from, to) else { panic!("lands elsewhere") };
        assert!((feet.y - 1.0).abs() < 0.1, "{feet:?}");
        assert_eq!(walk_drop(&world, &s, start, from, Vec3::new(0.25, 1.0, 0.0)), DropWalk::Arrives);

        let mut deck = tris.clone();
        deck.extend(quad(0.0, 5.0, -2.0, 2.0, 2.0));
        assert_eq!(walk_drop(&Collision::build(&deck), &s, start, from, to), DropWalk::Held);
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
}

//! Where a bot's body is now, though the server sees it only in the worlds
//! it is given: one every few seconds, of a tick a quarter of a second or
//! more behind the clock (`Game` asks two clients for one every
//! `refresh`, sooner while a bot needs one).
//!
//! The server knows every input the bot's body got, at its tick: it made
//! them. So from the last world that shows the body (a [`Fix`]) the body
//! is moved on tick by tick with those inputs by the movement model
//! ([`super::kin`]), kept on the walk grid where the map is known (a wall
//! stops it, a slope lifts it). A new world puts it back where the game
//! has it, and the inputs since are played again from there. How far the
//! model was off is kept ([`BodyTrack::last_error`]) for the logs and the
//! tuning.
//!
//! What the model does not know: other bodies in the way, being hit
//! (`stopping speed`), slides off steep ground, a jump's arc. Each is put
//! right by the next world. A jump it reckons only as the grid has it:
//! pressed, the body is up a jump link it walks into within
//! [`JUMP_AIR_MS`] ([`jumped_to`]), as a player who jumped takes himself to
//! be up the ledge until he sees he is not; a jump that did not get it up
//! is put right by the next world as the rest.
//!
//! The pitch is the base pitch (`m_look_pitch`), which a world saves; the
//! view is that plus the weapon's kick ([`super::recoil`]: PROVEN,
//! 0x2E0F60). For a bot the kick is the bot's own prediction, kept with
//! each tick, and its turns as decided already carry the part of a
//! downward turn the kick took (0x2E0FB0), so the base pitch is the
//! world's. A human's kick is not known: 0.

use std::collections::VecDeque;

use mapdata::{LinkKind, NodeId, Vec3, WalkGrid};
use survarium::v026g::match_msgs::TICK_MS;

use super::kin::{Blend, Kinematics, TICK_S, to_world, wrap};
use crate::v026g::drive::key;

/// A world's word on the bot's body.
#[derive(Clone, Debug, PartialEq)]
pub struct Fix {
    /// The world's tick.
    pub time: u32,
    pub alive: bool,
    /// When this body was spawned: another value is another life.
    pub inserted: u32,
    /// The feet, map frame (alive only).
    pub feet: Option<Vec3>,
    /// The game's yaw: never wrapped.
    pub yaw: f32,
    pub pitch: f32,
    /// The keys the world has the body hold (`actions_mask`).
    pub keys: u32,
    /// `m_linear_horizontal_speed`, if it was read.
    pub speed: Option<f32>,
}

/// The body as the bot reckons it at a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    pub time: u32,
    pub feet: Vec3,
    /// The ground velocity, map frame.
    pub velocity: [f32; 2],
    /// The velocity's blend, in the body's frame (`kin::Blend`).
    pub blend: Blend,
    /// In `(-π, π]`.
    pub yaw: f32,
    /// The base pitch (the world's).
    pub pitch: f32,
    /// The weapon's kick over it after the tick (a bot's own prediction).
    pub kick: f32,
    /// The keys held: they move the body from the next tick on (a world
    /// does not show its own tick's keys yet, `kin`).
    pub keys: u32,
    /// The grid floor it stands on, where the map is known.
    pub node: Option<NodeId>,
    /// Until when (ms) a jump pressed keeps it in the air: walking into a
    /// floor its floor jumps to, it gets up on it ([`JUMP_AIR_MS`]); 0 on
    /// the ground.
    pub air_until: u32,
}

impl Estimate {
    /// The view's pitch: the base pitch and the kick (0x2E0F60).
    pub fn view_pitch(&self) -> f32 {
        (self.pitch + self.kick).clamp(-MAX_PITCH, MAX_PITCH)
    }
}

/// How far off its reckoned place the body may be (m): the median of the bench is 0.17, the 90th percentile 0.33.
const RECKONING_M: f32 = 0.3;
/// A fall within this of a landing of the route's own drops (m) is planned.
const PLANNED_LANDING_M: f32 = 1.0;

/// The keys that move the body on the ground.
const MOVE_KEYS: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::SPRINT;

impl Estimate {
    /// The body as the view has it now (`me`), on the floor `node`, its velocity already blended in: what
    /// [`stays_up`] plays forward from.
    pub fn of_view(me: &super::view::PlaceView, node: Option<NodeId>) -> Self {
        let [fx, fz] = super::kin::forward(me.yaw);
        let local = [
            me.velocity[0] * fx + me.velocity[1] * fz,
            -me.velocity[0] * fz + me.velocity[1] * fx,
        ];
        Self {
            time: me.known_at,
            feet: me.feet,
            velocity: me.velocity,
            blend: Blend::at(local),
            yaw: wrap(me.yaw),
            pitch: me.pitch,
            kick: 0.0,
            keys: me.keys,
            node,
            air_until: 0,
        }
    }
}

/// What the stop check [`stays_up`] plays the body over: its movement model and speed level, the grid, the
/// landings of the route's own drops, and the ticks the keys count held.
#[derive(Clone, Copy)]
pub struct Stop<'a> {
    pub kin: &'a Kinematics,
    pub level: f32,
    pub grid: &'a WalkGrid,
    pub planned: &'a [Vec3],
    pub hold: u32,
}

/// Whether the body `at` that holds `keys` for the next tick (turning `turn` radians) and then lets go keeps up: no
/// step of its slide, over the blend to a stop, goes down a ledge of a lip's height ([`mapdata::grid::LIP_DROP_M`]) off
/// the floor it stands on. A body does not stop at once (the blend of [`Kinematics::blend_ticks`]): a key
/// pressed at a rim carries it a stride on, so the check plays the stop, not the step.
pub fn stays_up(at: &Estimate, keys: u32, turn: f32, stop: &Stop) -> bool {
    let Stop { kin, level, grid, planned, hold } = *stop;
    let top = at.feet.y;
    let mut at = *at;
    for k in 0..kin.blend_ticks + hold + 2 {
        // The keys count a tick more than the model's one-tick lag (the game may take them at once).
        let (held, turn) = if k < hold { (keys, if k == 0 { turn } else { 0.0 }) } else { (keys & !MOVE_KEYS, 0.0) };
        at = advance(at, Tick { time: at.time + TICK_MS, keys: held, turn: (turn, 0.0), kick: 0.0 }, kin, level, Some(grid));
        // A fall that lands where the route drops is the route's.
        let falls = |feet: Vec3| top - feet.y >= mapdata::grid::LIP_DROP_M && !planned.iter().any(|landing| landing.distance_xz(feet) <= PLANNED_LANDING_M);
        if falls(at.feet) {
            return false;
        }
        // The body is not exactly where it is reckoned (a world's lateness, a ping's jitter): the same stop
        // from [`RECKONING_M`] further on along its way must not fall either.
        let speed = at.velocity[0].hypot(at.velocity[1]);
        if speed > 0.3
            && let Some(node) = at.node
        {
            let to = Vec3::new(
                at.feet.x + at.velocity[0] / speed * RECKONING_M,
                at.feet.y,
                at.feet.z + at.velocity[1] / speed * RECKONING_M,
            );
            if falls(slide(grid, node, at.feet, to).1) {
                return false;
            }
        }
    }
    true
}

/// A decided tick: the keys held, the turn made to the base pitch, and the
/// kick after it.
#[derive(Clone, Copy, Debug)]
struct Tick {
    time: u32,
    keys: u32,
    turn: (f32, f32),
    kick: f32,
}

/// Ticks kept to be played again from a world: worlds come at most a few
/// seconds behind (a world older than this has no use).
const KEPT_MS: u32 = 20_000;
/// A floor this far from the feet is not the body's.
const LOCATE_M: f32 = 1.5;
/// A body is reckoned on this far past its last known tick at most, ms.
const PROJECT_MS: u32 = 3_000;
/// The pitch's limit, radians (LIKELY, `kin`).
const MAX_PITCH: f32 = 1.0;
/// A jump pressed keeps the reckoned body in the air this long, ms
/// ([`jumped_to`]). The worlds of the owner's matches of 2026-10-23 show a
/// body that pressed jump rising from about 200 ms after the press, at the
/// height of a 0.9-1 m ledge by 500 ms, and the jump over by 800 ms
/// (`player_bots_stuck.md` §8.9; worlds a second apart).
pub const JUMP_AIR_MS: u32 = 800;

#[derive(Clone, Debug, Default)]
pub struct BodyTrack {
    ticks: VecDeque<Tick>,
    /// The reckoned feet at each decided tick, to compare with a world.
    trail: VecDeque<(u32, Vec3)>,
    fix: Option<Fix>,
    estimate: Option<Estimate>,
    /// The distance between the reckoned feet and a world's, at the
    /// world's tick, for the last world that had both.
    last_error: Option<f32>,
    /// How far the reckoning moved the body on the ground from the world
    /// before to the last one (by the bot's own keys, on the walk grid):
    /// how far it meant to go while the worlds show how far it went.
    last_meant: Option<f32>,
    /// This life's speeds over the nominal ones (`Kinematics::level_of`).
    level: Option<f32>,
    /// The level its profile gives (`bot_skills::profile_movement_level`:
    /// skills, the weight carried), taken until a world shows this life's.
    prior_level: Option<f32>,
    /// The lives the worlds showed, the present one counted.
    lives: u32,
    /// Where the body was last reckoned alive, and which life that was:
    /// kept past its death, as a world shows a dead body without feet.
    last_alive: Option<(u32, Vec3)>,
}

impl BodyTrack {
    /// The body now (at the last decided tick), if it is alive and was seen.
    pub fn estimate(&self) -> Option<&Estimate> {
        self.estimate.as_ref()
    }

    pub fn fix(&self) -> Option<&Fix> {
        self.fix.as_ref()
    }

    /// The bench only: `estimate` as a human's body is seen, known `late`
    /// ms before `clock` (his ping) and reckoned on to the clock: the feet
    /// of its last decided tick by then and its speed between that tick and
    /// the one before, on in a line. With no trail that old, `estimate`.
    #[cfg(test)]
    pub fn seen_late(&self, estimate: Estimate, clock: u32, late: u32) -> Estimate {
        let known = clock.saturating_sub(late);
        let Some(k) = self.trail.iter().rposition(|&(time, _)| time <= known) else {
            return estimate;
        };
        let (time, feet) = self.trail[k];
        let velocity = match k.checked_sub(1).map(|k| self.trail[k]) {
            Some((before, from)) if time > before => {
                let s = (time - before) as f32 / 1000.0;
                [(feet.x - from.x) / s, (feet.z - from.z) / s]
            }
            _ => estimate.velocity,
        };
        let ahead = clock.saturating_sub(time) as f32 / 1000.0;
        Estimate {
            time: clock,
            feet: Vec3::new(feet.x + velocity[0] * ahead, feet.y, feet.z + velocity[1] * ahead),
            velocity,
            ..estimate
        }
    }

    pub fn last_error(&self) -> Option<f32> {
        self.last_error
    }

    /// The speed level its profile gives, until a world shows the life's
    /// own (see `prior_level`).
    pub fn set_prior_level(&mut self, level: Option<f32>) {
        self.prior_level = level.filter(|level| level.is_finite() && *level > 0.0);
    }

    /// The speed level the body is moved at: this life's from a world, else
    /// its profile's, else the nominal 1.
    pub fn level(&self) -> f32 {
        self.level.or(self.prior_level).unwrap_or(1.0)
    }

    /// How far the body was reckoned to move between the last two worlds
    /// of this life ([`Self::fixed`]); none at a life's first world or
    /// when the world's tick has no reckoning.
    pub fn last_meant(&self) -> Option<f32> {
        self.last_meant
    }

    /// The body reckoned on to `time` with the keys it holds (a human's,
    /// between two of its inputs); at most [`PROJECT_MS`] past the last
    /// tick known, as its inputs may have stopped coming.
    pub fn at(&self, time: u32, kin: &Kinematics, grid: Option<&WalkGrid>) -> Option<Estimate> {
        let estimate = *self.estimate.as_ref()?;
        let until = time.min(estimate.time + PROJECT_MS);
        let ticks = until.saturating_sub(estimate.time) / TICK_MS;
        if ticks == 0 {
            return Some(estimate);
        }
        let tick = Tick {
            time: estimate.time + ticks * TICK_MS,
            keys: estimate.keys,
            turn: (0.0, 0.0),
            kick: estimate.kick,
        };
        Some(advance(estimate, tick, kin, self.level(), grid))
    }

    /// Where the body is, or was last reckoned alive, and in which of its
    /// lives (counted from 1 as the worlds showed them): its feet now while
    /// it lives, else where its last life was reckoned when a world first
    /// showed it dead (that world has no feet for it: the kill feed's spot
    /// of a death the world already shows). None for a body never placed.
    pub fn last_alive(&self) -> Option<(u32, Vec3)> {
        match self.estimate.as_ref() {
            Some(estimate) => Some((self.lives, estimate.feet)),
            None => self.last_alive,
        }
    }

    /// Whether the last world showed the body dead.
    pub fn dead(&self) -> bool {
        self.fix.as_ref().is_some_and(|fix| !fix.alive)
    }

    /// The bot decided the tick at `time`: its keys, the turn as sent (the
    /// pitch's with the part the kick took) and the kick after it.
    pub fn decided(
        &mut self,
        time: u32,
        keys: u32,
        turn: (f32, f32),
        kick: f32,
        kin: &Kinematics,
        grid: Option<&WalkGrid>,
    ) {
        let tick = Tick {
            time,
            keys,
            turn,
            kick,
        };
        self.ticks.push_back(tick);
        while self
            .ticks
            .front()
            .is_some_and(|tick| tick.time + KEPT_MS < time)
        {
            self.ticks.pop_front();
        }
        let level = self.level();
        if let Some(estimate) = self.estimate.as_mut() {
            *estimate = advance(*estimate, tick, kin, level, grid);
            self.trail.push_back((time, estimate.feet));
            while self
                .trail
                .front()
                .is_some_and(|(at, _)| at + KEPT_MS < time)
            {
                self.trail.pop_front();
            }
        }
    }

    /// A world showed the body. An older world than the last is not used.
    /// Returns whether it starts a new life (or the first one).
    pub fn fixed(&mut self, fix: Fix, kin: &Kinematics, grid: Option<&WalkGrid>) -> bool {
        if self.fix.as_ref().is_some_and(|last| last.time >= fix.time) {
            return false;
        }
        let new_life = fix.alive
            && self
                .fix
                .as_ref()
                .is_none_or(|last| !last.alive || last.inserted != fix.inserted);
        let feet = match (fix.alive, fix.feet) {
            (true, Some(feet)) => feet,
            _ => {
                // Where it fell, as near as is known: the reckoning at the
                // world's tick (it was dead by then), else the last world's
                // feet, else the reckoning now.
                if let Some(estimate) = self.estimate.take() {
                    let feet = self
                        .trail
                        .iter()
                        .rev()
                        .find(|(at, _)| *at <= fix.time)
                        .map(|(_, feet)| *feet)
                        .or_else(|| self.fix.as_ref().and_then(|last| last.feet))
                        .unwrap_or(estimate.feet);
                    self.last_alive = Some((self.lives, feet));
                }
                self.trail.clear();
                self.level = None;
                self.fix = Some(fix);
                return false;
            }
        };
        if new_life {
            self.level = None;
            self.lives += 1;
        }
        if let Some(level) = fix.speed.and_then(|speed| kin.level_of(fix.keys, speed)) {
            self.level = Some(level);
        }
        let level = self.level();
        let reckoned = (!new_life)
            .then(|| {
                self.trail
                    .iter()
                    .find(|(at, _)| *at == fix.time)
                    .map(|(_, reckoned)| *reckoned)
            })
            .flatten();
        self.last_error = reckoned.map(|reckoned| reckoned.distance_xz(feet));
        // The way reckoned from the world before to this one's tick, step
        // by step (a turning body's way is longer than its displacement).
        let before = self
            .fix
            .as_ref()
            .filter(|last| last.alive)
            .and_then(|last| last.feet);
        self.last_meant = reckoned.zip(before).map(|(_, from)| {
            let mut at = from;
            let mut way = 0.0;
            for &(_, feet) in self.trail.iter().filter(|(time, _)| *time <= fix.time) {
                way += at.distance_xz(feet);
                at = feet;
            }
            way
        });
        let node = grid.and_then(|grid| grid.locate(feet, LOCATE_M));
        // The world has its own tick's keys (PROVEN, `kin`); its velocity
        // is its speed along the way the keys before were taking it.
        let held = fix.keys;
        let before = self
            .ticks
            .iter()
            .rev()
            .find(|tick| tick.time < fix.time)
            .map_or(fix.keys, |tick| tick.keys);
        let mut local = kin.target_local(before, level);
        if let Some(speed) = fix.speed.filter(|speed| speed.is_finite()) {
            let now = local[0].hypot(local[1]);
            local = if now > 1e-3 {
                [local[0] / now * speed, local[1] / now * speed]
            } else {
                [speed, 0.0]
            };
        }
        let mut estimate = Estimate {
            time: fix.time,
            feet,
            velocity: to_world(local, fix.yaw),
            blend: Blend::at(local),
            yaw: wrap(fix.yaw),
            pitch: fix.pitch,
            // The world has no kick; the bot's own for that tick.
            kick: self
                .ticks
                .iter()
                .find(|tick| tick.time == fix.time)
                .map_or(0.0, |tick| tick.kick),
            keys: held,
            node,
            air_until: self.air_at(fix.time),
        };
        self.trail.clear();
        // The ticks after the world's, again from where the world has it.
        for tick in self.ticks.iter().filter(|tick| tick.time > fix.time) {
            estimate = advance(estimate, *tick, kin, level, grid);
            self.trail.push_back((tick.time, estimate.feet));
        }
        // A world newer than every decided tick: the body is as it shows.
        self.estimate = Some(estimate);
        self.fix = Some(fix);
        new_life
    }
}

impl BodyTrack {
    /// Until when a jump pressed at or before `time` keeps the body in the
    /// air ([`JUMP_AIR_MS`]); 0 if none does.
    fn air_at(&self, time: u32) -> u32 {
        let mut held = false;
        let mut pressed = None;
        for tick in self.ticks.iter().take_while(|tick| tick.time <= time) {
            let jump = tick.keys & key::JUMP != 0;
            if jump && !held {
                pressed = Some(tick.time);
            }
            held = jump;
        }
        pressed
            .map(|at| at + JUMP_AIR_MS)
            .filter(|&until| until > time)
            .unwrap_or(0)
    }
}

/// The body at the next decided tick: moved by the keys it held (a tick
/// missed between two decided ones, a stall, held the same keys), then the
/// tick's turn and keys taken. In the air ([`Estimate::air_until`]), a step
/// a ledge stops that walks into a floor its floor jumps to gets up on it.
fn advance(
    mut at: Estimate,
    tick: Tick,
    kin: &Kinematics,
    level: f32,
    grid: Option<&WalkGrid>,
) -> Estimate {
    let steps = (tick.time.saturating_sub(at.time) / TICK_MS).max(1);
    let target = kin.target_local(at.keys, level);
    for step in 1..=steps {
        let local = at.blend.tick(target, kin.blend_ticks);
        at.velocity = to_world(local, at.yaw);
        let to = Vec3::new(
            at.feet.x + at.velocity[0] * TICK_S,
            at.feet.y,
            at.feet.z + at.velocity[1] * TICK_S,
        );
        match (grid, at.node) {
            (Some(grid), Some(node)) => {
                let (mut node, mut feet, mut stopped) = slide(grid, node, at.feet, to);
                if at.time + step * TICK_MS <= at.air_until
                    && feet.distance_xz(to) > 1e-3
                    && let Some(up) = jumped_to(grid, node, feet, to)
                {
                    node = up;
                    feet = grid.position(up);
                    stopped = false;
                    at.air_until = 0;
                }
                at.node = Some(node);
                at.feet = feet;
                if stopped {
                    at.velocity = [0.0, 0.0];
                }
            }
            _ => at.feet = to,
        }
    }
    if tick.keys & key::JUMP != 0 && at.keys & key::JUMP == 0 {
        at.air_until = tick.time + JUMP_AIR_MS;
    } else if tick.time > at.air_until {
        at.air_until = 0;
    }
    at.yaw = wrap(at.yaw + tick.turn.0);
    at.pitch = (at.pitch + tick.turn.1).clamp(-MAX_PITCH, MAX_PITCH);
    at.kick = tick.kick;
    at.keys = tick.keys;
    at.time = tick.time;
    at
}

/// A move from `from` (on `node`) toward `to` over the grid, in steps of
/// at most half a cell: each step goes to a floor linked to the one it is
/// on, or along one axis only (a body slides along a wall), or not at all.
/// Returns the floor, the feet, and whether the move was stopped before
/// it got anywhere.
pub fn slide(grid: &WalkGrid, node: NodeId, from: Vec3, to: Vec3) -> (NodeId, Vec3, bool) {
    slide_up_to(grid, node, from, to, f32::INFINITY)
}

/// [`slide`] with no step up a walk link higher than `max_rise`: the
/// tests' stand-in game walks a body at the client's step
/// (`Body::step`), not up the grid's "forced" stair links of up to
/// 0.75 m, which the client does not climb (owner-test analysis,
/// Mamayev Kurgan). The bots' reckoning keeps [`slide`].
#[cfg(test)]
pub fn slide_capped(
    grid: &WalkGrid,
    node: NodeId,
    from: Vec3,
    to: Vec3,
    max_rise: f32,
) -> (NodeId, Vec3, bool) {
    slide_up_to(grid, node, from, to, max_rise)
}

fn slide_up_to(
    grid: &WalkGrid,
    mut node: NodeId,
    from: Vec3,
    to: Vec3,
    max_rise: f32,
) -> (NodeId, Vec3, bool) {
    let half = grid.cell_size() * 0.5;
    let (dx, dz) = (to.x - from.x, to.z - from.z);
    let steps = ((dx.abs().max(dz.abs()) / half).ceil() as usize).max(1);
    let (sx, sz) = (dx / steps as f32, dz / steps as f32);
    let mut at = from;
    for _ in 0..steps {
        // The step, else along one axis (a body slides along a wall): each
        // to a floor walked to, else off a ledge to the floor below (its
        // drop skips the cell eroded at the ledge's foot: [`dropped_to`]).
        // A body walking into a ledge's edge at a slant goes over it, not
        // along it.
        let tries = [(sx, sz), (sx, 0.0), (0.0, sz)];
        let moved = tries.iter().find_map(|&(mx, mz)| {
            if mx == 0.0 && mz == 0.0 {
                return None;
            }
            let point = Vec3::new(at.x + mx, at.y, at.z + mz);
            floor_under(grid, node, point, max_rise)
                .map(|next| (next, Vec3::new(point.x, grid.position(next).y, point.z)))
                .or_else(|| {
                    dropped_to(grid, node, at, (mx, mz)).map(|next| (next, grid.position(next)))
                })
        });
        let Some((next, point)) = moved else {
            break;
        };
        node = next;
        at = point;
    }
    (node, at, at == from)
}

/// The floor below a ledge that a step `(sx, sz)` from `at` (on `node`)
/// walks off: a drop link of `node`'s ahead of the step whose line from
/// `node` passes within half a cell of where the step ends. The grid's
/// drop from the last floor of a sheer ledge lands two cells on, past the
/// cell eroded at the foot of its face, so no floor holds the step between:
/// a body walked to the edge and stood there, the server's reckoning of a
/// bot as well as the tests' stand-in (`player_bots_stuck.md` §8.4).
fn dropped_to(grid: &WalkGrid, node: NodeId, at: Vec3, (sx, sz): (f32, f32)) -> Option<NodeId> {
    let half = grid.cell_size() * 0.5 + 1e-4;
    let from = grid.position(node);
    let (px, pz) = (at.x + sx, at.z + sz);
    grid.links(node)
        .filter(|(_, kind)| *kind == LinkKind::Drop)
        .map(|(next, _)| next)
        .filter(|&next| {
            let to = grid.position(next);
            let (dx, dz) = (to.x - from.x, to.z - from.z);
            let len2 = dx * dx + dz * dz;
            if len2 <= 0.0 || dx * sx + dz * sz <= 0.0 {
                return false;
            }
            let t = (((px - from.x) * dx + (pz - from.z) * dz) / len2).clamp(0.0, 1.0);
            let (ex, ez) = (from.x + dx * t - px, from.z + dz * t - pz);
            ex * ex + ez * ez <= half * half
        })
        .min_by(|&a, &b| {
            let d = |n: NodeId| {
                let p = grid.position(n);
                (p.x - px).powi(2) + (p.z - pz).powi(2)
            };
            d(a).total_cmp(&d(b))
        })
}

/// The floor a body in the air at `feet` (on `node`) gets up on walking
/// toward `to`: a jump link of `node`'s ahead of the step whose line from
/// `node` passes within half a cell of `to`, the nearest to it. The
/// reckoning's ([`JUMP_AIR_MS`]) and the tests' stand-in game's.
pub fn jumped_to(grid: &WalkGrid, node: NodeId, feet: Vec3, to: Vec3) -> Option<NodeId> {
    let half = grid.cell_size() * 0.5 + 1e-4;
    let from = grid.position(node);
    let (sx, sz) = (to.x - feet.x, to.z - feet.z);
    grid.links(node)
        .filter(|(_, kind)| *kind == LinkKind::Jump)
        .map(|(next, _)| next)
        .filter(|&next| {
            let at = grid.position(next);
            let (dx, dz) = (at.x - from.x, at.z - from.z);
            let len2 = dx * dx + dz * dz;
            if len2 <= 0.0 || dx * sx + dz * sz <= 0.0 {
                return false;
            }
            let t = (((to.x - from.x) * dx + (to.z - from.z) * dz) / len2).clamp(0.0, 1.0);
            let (ex, ez) = (from.x + dx * t - to.x, from.z + dz * t - to.z);
            ex * ex + ez * ez <= half * half
        })
        .min_by(|&a, &b| {
            let d = |n: NodeId| grid.position(n).distance_xz(to);
            d(a).total_cmp(&d(b))
        })
}

/// The floor whose cell holds `point`: `node` itself or one it walks or
/// drops to.
fn floor_under(grid: &WalkGrid, node: NodeId, point: Vec3, max_rise: f32) -> Option<NodeId> {
    let half = grid.cell_size() * 0.5 + 1e-4;
    let base = grid.position(node).y;
    let holds = |n: NodeId| {
        let p = grid.position(n);
        (p.x - point.x).abs() <= half && (p.z - point.z).abs() <= half
    };
    if holds(node) {
        return Some(node);
    }
    grid.links(node)
        .filter(|(_, kind)| *kind != LinkKind::Jump)
        .map(|(next, _)| next)
        .find(|&next| holds(next) && grid.position(next).y - base <= max_rise)
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::v026g::drive::key;
    use mapdata::{Body, Triangle, WalkerBehaviour};

    /// A flat floor from (x0, z0) to (x1, z1) at height y (as the mapdata
    /// crate's tests make one).
    pub fn floor(x0: f32, z0: f32, x1: f32, z1: f32, y: f32) -> [Triangle; 2] {
        let walker = WalkerBehaviour::BySlope;
        let (a, b, c, d) = (
            Vec3::new(x0, y, z0),
            Vec3::new(x1, y, z0),
            Vec3::new(x1, y, z1),
            Vec3::new(x0, y, z1),
        );
        [
            Triangle {
                v: [a, b, c],
                walker,
            },
            Triangle {
                v: [a, c, d],
                walker,
            },
        ]
    }

    /// A wall across x at `x` from z0 to z1, 3 m high.
    pub fn wall_x(x: f32, z0: f32, z1: f32) -> [Triangle; 2] {
        let walker = WalkerBehaviour::BySlope;
        let (a, b, c, d) = (
            Vec3::new(x, 0.0, z0),
            Vec3::new(x, 0.0, z1),
            Vec3::new(x, 3.0, z1),
            Vec3::new(x, 3.0, z0),
        );
        [
            Triangle {
                v: [a, b, c],
                walker,
            },
            Triangle {
                v: [a, c, d],
                walker,
            },
        ]
    }

    /// A 40 x 40 m floor with a wall along x = 20 from z = 0 to z = 30:
    /// the way round is past z = 30.
    pub fn walled() -> WalkGrid {
        let mut triangles: Vec<Triangle> = floor(0.0, 0.0, 40.0, 40.0, 0.0).to_vec();
        triangles.extend(wall_x(20.0, 0.0, 30.0));
        WalkGrid::build(&triangles, [0.0, 0.0, 40.0, 40.0], 0.25, Body::default())
    }

    fn fix(time: u32, feet: Vec3, yaw: f32) -> Fix {
        Fix {
            time,
            alive: true,
            inserted: 100,
            feet: Some(feet),
            yaw,
            pitch: 0.0,
            keys: 0,
            speed: None,
        }
    }

    /// Between two worlds the keys meant a walk's length (the stuck
    /// watch compares it with how far the worlds show the body went), and
    /// nothing while it stood.
    #[test]
    fn the_way_the_keys_meant_between_two_worlds_is_kept() {
        let kin = Kinematics {
            blend_ticks: 1,
            ..Kinematics::default()
        };
        let mut track = BodyTrack::default();
        let at = Vec3::new(5.0, 0.0, 5.0);
        track.fixed(fix(1_000, at, 0.0), &kin, None);
        assert_eq!(track.last_meant(), None, "a life's first world");
        for time in (1_050..=3_000).step_by(50) {
            track.decided(time, key::FORWARD, (0.0, 0.0), 0.0, &kin, None);
        }
        // The world shows it where it was: caught.
        track.fixed(fix(2_500, at, 0.0), &kin, None);
        let meant = track.last_meant().unwrap();
        assert!((meant - kin.walk[0] * 1.45).abs() < 1e-3, "{meant}");
        for time in (3_050..=4_500).step_by(50) {
            track.decided(time, 0, (0.0, 0.0), 0.0, &kin, None);
        }
        track.fixed(fix(4_000, at, 0.0), &kin, None);
        let meant = track.last_meant().unwrap();
        // The forward ticks up to 3 000 only.
        assert!((meant - kin.walk[0] * 0.5).abs() < 1e-3, "{meant}");
        track.fixed(fix(4_500, at, 0.0), &kin, None);
        assert!(track.last_meant().unwrap() < 1e-3, "it stood");
    }

    #[test]
    fn the_body_is_moved_on_by_its_inputs_and_put_back_by_a_world() {
        // Speeds reached at once, to count the metres.
        let kin = Kinematics {
            blend_ticks: 1,
            ..Kinematics::default()
        };
        let mut track = BodyTrack::default();
        assert!(track.estimate().is_none());
        // Inputs before any world are kept for when one comes.
        for time in (1_050..=2_000).step_by(50) {
            track.decided(time, key::FORWARD, (0.0, 0.0), 0.0, &kin, None);
        }
        assert!(track.fixed(fix(1_000, Vec3::new(5.0, 0.0, 5.0), 0.0), &kin, None));
        let at = *track.estimate().unwrap();
        assert_eq!(at.time, 2_000);
        // Forward at yaw 0 is +z; the keys of a tick move the body from the
        // next one: 19 ticks of walking.
        assert!(
            (at.feet.z - (5.0 + kin.walk[0] * 0.95)).abs() < 1e-3,
            "{at:?}"
        );
        assert!((at.feet.x - 5.0).abs() < 1e-3);
        // A world of a later tick: the body is put where it shows, the
        // ticks after it played again, and how far off the reckoning was is
        // kept.
        track.decided(2_050, key::FORWARD, (0.0, 0.0), 0.0, &kin, None);
        // The world has its tick's keys: forward.
        let walking = Fix {
            keys: key::FORWARD,
            ..fix(1_500, Vec3::new(5.0, 0.0, 6.0), 0.0)
        };
        assert!(!track.fixed(walking, &kin, None));
        let error = track.last_error().unwrap();
        assert!(
            (error - (kin.walk[0] * 0.45 - 1.0).abs()).abs() < 1e-3,
            "{error}"
        );
        let at = *track.estimate().unwrap();
        assert!(
            (at.feet.z - (6.0 + kin.walk[0] * 0.55)).abs() < 1e-3,
            "{at:?}"
        );
        // An older world is not used.
        assert!(!track.fixed(fix(1_400, Vec3::new(0.0, 0.0, 0.0), 0.0), &kin, None));
        // A turn is applied at its tick: a quarter turn, then forward goes
        // to -x.
        track.decided(
            2_100,
            0,
            (std::f32::consts::FRAC_PI_2, 0.0),
            0.0,
            &kin,
            None,
        );
        let before = track.estimate().unwrap().feet;
        track.decided(2_150, key::FORWARD, (0.0, 0.0), 0.0, &kin, None);
        assert_eq!(
            track.estimate().unwrap().feet,
            before,
            "the keys move it a tick later"
        );
        track.decided(2_200, key::FORWARD, (0.0, 0.0), 0.0, &kin, None);
        let after = track.estimate().unwrap().feet;
        assert!(after.x < before.x - 0.1 && (after.z - before.z).abs() < 1e-3);
        // Dead: nothing reckoned; alive again with a new spawn time is a
        // new life.
        track.fixed(
            Fix {
                alive: false,
                feet: None,
                ..fix(2_000, Vec3::ZERO, 0.0)
            },
            &kin,
            None,
        );
        assert!(track.dead() && track.estimate().is_none());
        assert!(track.fixed(
            Fix {
                inserted: 3_000,
                ..fix(3_200, Vec3::new(1.0, 0.0, 1.0), 0.0)
            },
            &kin,
            None
        ));
    }

    /// A body moves at its profile's speed level (its skills, its weight)
    /// until a world shows the life's own; a new life starts from the
    /// profile's again.
    #[test]
    fn a_body_moves_at_its_profiles_level_until_a_world_shows_its_own() {
        // Speeds reached at once, to count the metres.
        let kin = Kinematics {
            blend_ticks: 1,
            ..Kinematics::default()
        };
        let walk = |track: &mut BodyTrack, from: u32, to: u32| {
            for time in (from..=to).step_by(50) {
                track.decided(time, key::FORWARD, (0.0, 0.0), 0.0, &kin, None);
            }
        };
        let mut track = BodyTrack::default();
        assert_eq!(track.level(), 1.0);
        track.set_prior_level(Some(0.9));
        assert_eq!(track.level(), 0.9);
        track.fixed(fix(0, Vec3::ZERO, 0.0), &kin, None);
        walk(&mut track, 50, 1_000);
        // 19 ticks of walking at 0.9 of the walk.
        let z = track.estimate().unwrap().feet.z;
        assert!((z - kin.walk[0] * 0.9 * 0.95).abs() < 1e-3, "{z}");
        // A world shows it walking at 1.1: that is the life's level.
        let walking = Fix {
            keys: key::FORWARD,
            speed: Some(kin.walk[0] * 1.1),
            ..fix(1_000, Vec3::ZERO, 0.0)
        };
        track.fixed(walking, &kin, None);
        assert!((track.level() - 1.1).abs() < 1e-5);
        // A new life: the profile's again.
        track.fixed(
            Fix {
                inserted: 5_000,
                ..fix(5_000, Vec3::ZERO, 0.0)
            },
            &kin,
            None,
        );
        assert_eq!(track.level(), 0.9);
        // Nonsense is not taken.
        track.set_prior_level(Some(f32::NAN));
        assert_eq!(track.level(), 1.0);
    }

    #[test]
    fn a_wall_stops_the_reckoned_body_and_it_slides_along() {
        let grid = walled();
        // Speeds reached at once, to count the metres.
        let kin = Kinematics {
            blend_ticks: 1,
            ..Kinematics::default()
        };
        let mut track = BodyTrack::default();
        // At x = 17, facing +x (yaw -π/2), walking into the wall for 2 s.
        let yaw = -std::f32::consts::FRAC_PI_2;
        assert!((crate::v026g::bots::kin::forward(yaw)[0] - 1.0).abs() < 1e-5);
        track.fixed(fix(0, Vec3::new(17.0, 0.0, 10.0), yaw), &kin, Some(&grid));
        assert!(track.estimate().unwrap().node.is_some());
        for time in (50..=2_000).step_by(50) {
            track.decided(time, key::FORWARD, (0.0, 0.0), 0.0, &kin, Some(&grid));
        }
        let at = track.estimate().unwrap();
        // The body's radius keeps it short of the wall.
        assert!(at.feet.x < 19.8 && at.feet.x > 19.0, "{at:?}");
        assert!((at.feet.z - 10.0).abs() < 0.2);
        // Diagonally into it: it slides along the wall in z.
        let yaw = -std::f32::consts::FRAC_PI_4;
        let mut track = BodyTrack::default();
        track.fixed(fix(0, Vec3::new(18.5, 0.0, 10.0), yaw), &kin, Some(&grid));
        for time in (50..=2_000).step_by(50) {
            track.decided(time, key::FORWARD, (0.0, 0.0), 0.0, &kin, Some(&grid));
        }
        let at = track.estimate().unwrap();
        assert!(at.feet.x < 19.8 && at.feet.z > 13.0, "{at:?}");
    }

    /// A body that walks off a ledge lands on the floor below, though the
    /// drop skips the cells eroded at the ledge's edge.
    #[test]
    fn a_body_walks_off_a_ledge_to_the_floor_below() {
        // A block 2 m high from x 0 to 10.1 (its top and its face) on the
        // ground: the ground at the face's foot is eroded, and the drop
        // from the top's last floor skips that cell.
        let mut triangles: Vec<Triangle> = floor(0.0, 0.0, 20.0, 10.0, 0.0).to_vec();
        triangles.extend(floor(0.0, 0.0, 10.1, 10.0, 2.0));
        triangles.extend(wall_x(10.1, 0.0, 10.0).map(|mut t| {
            t.v.iter_mut().for_each(|v| v.y = v.y.min(2.0));
            t
        }));
        let grid = WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, Body::default());
        let start = grid.locate(Vec3::new(8.0, 2.0, 5.0), 0.5).unwrap();
        let skips = (0..grid.len() as NodeId).any(|n| {
            grid.links(n).any(|(m, kind)| {
                kind == LinkKind::Drop && grid.position(n).distance_xz(grid.position(m)) > 0.4
            })
        });
        assert!(skips, "the drops here skip a cell");
        let (mut node, mut feet) = (start, grid.position(start));
        // At a slant to the edge, as a body steering rarely walks square
        // to it: it goes over, not along.
        for _ in 0..40 {
            let to = feet + Vec3::new(0.25, 0.0, 0.03);
            (node, feet, _) = slide(&grid, node, feet, to);
        }
        assert!(feet.x > 11.0 && feet.y.abs() < 0.1, "{feet:?}");
        assert_eq!(grid.locate(feet, 0.5), Some(node));
    }

    /// The reckoned body walking into a 0.9 m ledge stops at its foot;
    /// with jump pressed, it gets up it within [`JUMP_AIR_MS`], also when
    /// a world of a tick after the press puts it back at the foot (the
    /// ticks since played again). A world after the jump showing it still
    /// below keeps it there.
    #[test]
    fn a_jump_pressed_takes_the_reckoned_body_up_a_ledge() {
        let mut triangles: Vec<Triangle> = floor(0.0, 0.0, 20.0, 10.0, 0.0).to_vec();
        triangles.extend(floor(10.0, 0.0, 20.0, 10.0, 0.9));
        triangles.extend(wall_x(10.0, 0.0, 10.0).map(|mut t| {
            t.v.iter_mut().for_each(|v| v.y = v.y.min(0.9));
            t
        }));
        let grid = WalkGrid::build(&triangles, [0.0, 0.0, 20.0, 10.0], 0.25, Body::default());
        let kin = Kinematics {
            blend_ticks: 1,
            ..Kinematics::default()
        };
        // Facing +x.
        let yaw = -std::f32::consts::FRAC_PI_2;
        let start = |track: &mut BodyTrack| {
            track.fixed(fix(0, Vec3::new(8.0, 0.0, 5.0), yaw), &kin, Some(&grid));
        };
        let walk = |track: &mut BodyTrack, from: u32, to: u32, jump_at: Option<u32>| {
            for time in (from..=to).step_by(50) {
                let jump = if jump_at == Some(time) { key::JUMP } else { 0 };
                track.decided(
                    time,
                    key::FORWARD | jump,
                    (0.0, 0.0),
                    0.0,
                    &kin,
                    Some(&grid),
                );
            }
        };
        let mut track = BodyTrack::default();
        start(&mut track);
        walk(&mut track, 50, 2_000, None);
        let at = track.estimate().unwrap();
        assert!(at.feet.x < 10.0 && at.feet.y.abs() < 0.1, "no jump: {at:?}");
        let mut track = BodyTrack::default();
        start(&mut track);
        walk(&mut track, 50, 2_000, Some(450));
        let at = *track.estimate().unwrap();
        assert!(
            at.feet.x > 11.0 && (at.feet.y - 0.9).abs() < 0.1,
            "jumped: {at:?}"
        );
        assert_eq!(at.air_until, 0);
        // A world after the jump: it did not get up.
        let below = Fix {
            keys: key::FORWARD,
            ..fix(2_000, Vec3::new(9.6, 0.0, 5.0), yaw)
        };
        track.fixed(below, &kin, Some(&grid));
        walk(&mut track, 2_050, 3_000, None);
        let at = track.estimate().unwrap();
        assert!(
            at.feet.x < 10.0 && at.feet.y.abs() < 0.1,
            "shown below: {at:?}"
        );
        // A world in the jump's air, at the foot: up again.
        let mut track = BodyTrack::default();
        start(&mut track);
        walk(&mut track, 50, 1_000, Some(450));
        let foot = Fix {
            keys: key::FORWARD,
            ..fix(600, Vec3::new(9.4, 0.0, 5.0), yaw)
        };
        track.fixed(foot, &kin, Some(&grid));
        let at = track.estimate().unwrap();
        assert!((at.feet.y - 0.9).abs() < 0.1, "in the air: {at:?}");
    }

    #[test]
    fn the_view_is_the_base_pitch_and_the_kick_of_the_last_tick() {
        let kin = Kinematics::default();
        let mut track = BodyTrack::default();
        track.fixed(fix(0, Vec3::ZERO, 0.0), &kin, None);
        // Up 0.02 with no kick; then the kick at 0.03 and a pull of 0.01
        // all taken off it (the base turn 0).
        track.decided(50, 0, (0.0, 0.02), 0.0, &kin, None);
        track.decided(100, key::FIRE, (0.0, 0.0), 0.03, &kin, None);
        track.decided(150, key::FIRE, (0.0, 0.0), 0.02, &kin, None);
        let at = track.estimate().unwrap();
        assert!((at.pitch - 0.02).abs() < 1e-6 && (at.view_pitch() - 0.04).abs() < 1e-6);
        // A world of tick 100: its base pitch, the bot's kick of that tick,
        // the ticks after played again.
        let world = Fix {
            pitch: 0.025,
            ..fix(100, Vec3::ZERO, 0.0)
        };
        track.fixed(world, &kin, None);
        let at = track.estimate().unwrap();
        assert!((at.view_pitch() - 0.045).abs() < 1e-6, "{at:?}");
        // The view is clamped as the client clamps it.
        track.decided(200, 0, (0.0, 0.97), 0.1, &kin, None);
        assert_eq!(track.estimate().unwrap().view_pitch(), 1.0);
    }
}

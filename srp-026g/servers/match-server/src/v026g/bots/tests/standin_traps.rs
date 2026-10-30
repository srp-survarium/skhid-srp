//! The stand-in game's bear traps (**model**, `SRP_BENCH_TRAPS_MODEL`, default on): without them a
//! bot that puts a trap down places nothing the bench can see, a trap nobody can step on or
//! shoot, and the bots' answers to traps (`trap_react`) have nothing to meet. With no body
//! carrying a trap the model changes nothing, so a run is decision-identical to one without it.
//!
//! What the client does (`docs/notes/bear_traps.md`, PROVEN unless marked):
//! - a trap is placed on the **release** of its quick-use key: a ray from the head along the
//!   view, [`PLACE_REACH_M`], the static walk geometry, flat ground only (normal.y at least
//!   cos 30 degrees), not on another trap's sensor; armed at once, for ever;
//! - the sensor: a body whose foot point is within [`SENSOR_M`] of it, and not in the air above
//!   [`SENSOR_TOP_M`], sets it off: any body, the placer too. It stands fired for [`GONE_MS`]
//!   and is gone. The catch takes both legs to 0 (no jump, no sprint, speed [`SLOW`]) for
//!   [`LEGS_MS`], and 0.3 pain a foot ([`PAIN`] together); no root, no health;
//! - a ranged hit on its cylinder (radius [`HIT_R`], half-height [`HIT_HALF`], [`HIT_UP`] above
//!   the floor point) disarms it, whoever shot (the design's section 8 says so; the same
//!   section also says a hit by the user's own team is refused: **GUESS**, the model takes any
//!   shooter); a use held [`DEFUSE_MS`] on it by its placer or a body of the other team does too
//!   (`can_defuse`); disarmed it stands [`GONE_MS`] and is gone.
//!
//! What it has of it:
//! - the worlds show it as the live game's objects section does (`match_msgs::traps`, state
//!   1 / 2 / 3, 77 bytes a trap), told to the bots with [`tell`];
//! - **not modelled**: the catch's snap sound (the bots' hearing has no kind for it yet), the
//!   recycling of the oldest trap when a pocket is out (a body places at most its pocket's count a
//!   life), a skill that stops a same-team catch (`friendly_trap`, no bot carries it).
//! - GUESS: the use ray's reach and tolerance ([`USE_REACH_M`], [`USE_TOL_M`]); the live use range
//!   is not read.

use std::sync::OnceLock;

use super::*;
use crate::v026g::bots::traps::TrapKit;

thread_local! {
    /// A test's word on the model, for its own thread.
    static FORCED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// A test puts the model on or off for its own thread (`None`: as the run says).
pub(crate) fn force_model(on: Option<bool>) {
    FORCED.with(|f| f.set(on));
}

/// Whether the stand-in has traps (`SRP_BENCH_TRAPS_MODEL=off` says no; on when unset).
pub(crate) fn model_on() -> bool {
    if let Some(on) = FORCED.with(std::cell::Cell::get) {
        return on;
    }
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| bench_settings().var("SRP_BENCH_TRAPS_MODEL").as_deref() != Ok("off"))
}

/// The placing ray from the head, metres.
pub(crate) const PLACE_REACH_M: f32 = 2.0;
/// The steepest floor a trap lies on: the floor normal's y at least this (cos 30 degrees).
pub(crate) const FLAT_NORMAL_Y: f32 = 0.866;
/// A body whose foot point is this near sets a trap off (on the ground).
pub(crate) const SENSOR_M: f32 = 0.6;
/// … unless its feet are higher than this over the trap's floor point.
pub(crate) const SENSOR_TOP_M: f32 = 0.22;
/// … or lower than this under it (another floor).
pub(crate) const SENSOR_BELOW_M: f32 = 0.5;
/// The trap's cylinder for a bullet.
pub(crate) const HIT_R: f32 = 0.125;
pub(crate) const HIT_HALF: f32 = 0.05;
pub(crate) const HIT_UP: f32 = 0.12;
/// A hand defuse, ms.
pub(crate) const DEFUSE_MS: u32 = 5_000;
/// A fired or disarmed trap stands this long, ms.
pub(crate) const GONE_MS: u32 = 8_000;
/// The catch: legs at 0 this long, ms; the walk at this share of its speed; the pain taken.
pub(crate) const LEGS_MS: u32 = 8_000;
pub(crate) const SLOW: f32 = 0.6;
pub(crate) const PAIN: f32 = 0.6;
/// The use ray's reach (the client's `usable_objects_detection_distance`) and how near the trap's
/// middle it must pass: GUESS.
pub(crate) const USE_REACH_M: f32 = 2.0;
pub(crate) const USE_TOL_M: f32 = 0.3;

/// What a trap is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Phase {
    Armed,
    Fired { at: u32 },
    Disarmed { at: u32 },
}

/// A placed trap.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Trap {
    /// The floor point it lies on.
    pub(crate) at: Vec3,
    pub(crate) placer: Place,
    pub(crate) team: u8,
    /// The tick it was placed.
    pub(crate) placed: u32,
    /// The placer held crouch when it let go of the key, and how far its feet were from the point, metres
    /// (ground): the pose and the distance at the press, for the catch lines.
    pub(crate) placer_crouched: bool,
    pub(crate) press_m: f32,
    /// The eye's height over the feet and the view's pitch (down positive) at the release, for the catch lines.
    pub(crate) eye_m: f32,
    pub(crate) pitch_down: f32,
    /// The placer's speed at the release, m/s.
    pub(crate) placer_speed: f32,
    pub(crate) phase: Phase,
    /// A hand defuse under way: who, since when.
    defusing: Option<(Place, u32)>,
}

impl Trap {
    /// The middle of what a bullet or a use meets.
    fn middle(&self) -> Vec3 {
        self.at + Vec3::new(0.0, HIT_UP, 0.0)
    }
}

/// What befell, for the bench's line and the tests.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Event {
    Placed { tick: u32, place: Place, team: u8, press_m: f32, placer_crouched: bool, placer_speed: f32 },
    /// `victim` set `placer`'s trap off: whose body that is, as a kill's kinds say.
    Caught { tick: u32, placer: Place, victim: Place, kind: Kill, placed: u32, placer_crouched: bool, press_m: f32, eye_m: f32, pitch_down: f32, placer_team: u8, victim_team: u8, placer_speed: f32 },
    /// `placed` is the tick the trap was put down, which names it with its placer.
    ShotOff { tick: u32, shooter: Place, placer: Place, placed: u32 },
    DefusedBy { tick: u32, by: Place, placer: Place, placed: u32 },
}

/// A body's part of the model: what it carries and how a catch slows it.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Rig {
    /// The key that puts one down (0: the body carries none), and how many are left this life.
    key: u32,
    stock: u8,
    /// The spawn the stock was filled for.
    life: u32,
    /// Until when a catch holds the legs ([`SLOW`], no sprint).
    pub(crate) slow_until: u32,
}

impl Rig {
    /// The kit the body's bot says it carries, filled once a life.
    fn learn(&mut self, kit: TrapKit, spawned: u32) {
        if self.life != spawned || self.key == 0 {
            *self = Self { key: kit.key, stock: kit.count, life: spawned, slow_until: self.slow_until };
        }
    }

    /// Whether a catch still holds the legs at `tick`.
    pub(crate) fn slowed(&self, tick: u32) -> bool {
        tick < self.slow_until
    }
}

/// The traps of a run.
#[derive(Default)]
pub(crate) struct Traps {
    pub(crate) list: Vec<Trap>,
    pub(crate) events: Vec<Event>,
    /// The objects section of each world of the last two seconds.
    worlds: VecDeque<(u32, Vec<u8>)>,
}

impl Traps {
    /// The counts for the bench's line: placed, caught (enemy, mate, own), shot off, defused.
    pub(crate) fn counts(&self) -> [u32; 6] {
        let mut c = [0; 6];
        for event in &self.events {
            match event {
                Event::Placed { .. } => c[0] += 1,
                Event::Caught { kind: Kill::Enemy, .. } => c[1] += 1,
                Event::Caught { kind: Kill::Mate, .. } => c[2] += 1,
                Event::Caught { kind: Kill::Own, .. } => c[3] += 1,
                Event::ShotOff { .. } => c[4] += 1,
                Event::DefusedBy { .. } => c[5] += 1,
            }
        }
        c
    }

    /// One line for each catch: whose body it was, the placer's pose when it let go, how long after placing, and how
    /// far the point was from its feet then (`traps.press_landing_m`: at least 1 m; the trigger reaches 0.6 m).
    pub(crate) fn catch_lines(&self) -> Vec<String> {
        self.events
            .iter()
            .filter_map(|event| match event {
                Event::Caught { tick, placer, victim, kind, placed, placer_crouched, press_m, eye_m, pitch_down, placer_team, .. } => Some(format!(
                    "  trap catch: {} (placer {placer} of team {placer_team}, victim {victim}) placer {} {:.1} s after placing, {:.2} m from its feet at the press (eye {:.2} m, look {:.2} rad down)",
                    match kind {
                        Kill::Enemy => "enemy",
                        Kill::Mate => "mate",
                        Kill::Own => "own",
                    },
                    if *placer_crouched { "crouched" } else { "standing" },
                    tick.saturating_sub(*placed) as f32 / 1000.0,
                    press_m,
                    eye_m,
                    pitch_down,
                )),
                _ => None,
            })
            .collect()
    }

    /// The catches by kind and the placer's pose, and the nearest and median press distance of the placed: one
    /// line for the log (the players' catches are 85 % enemies, 11 % allies, 4 % the placer: WP-T).
    pub(crate) fn catch_summary(&self) -> String {
        // By the placer's team: [team 0, team 1] for enemy, mate and own catches, and how many traps each team placed.
        let mut by_team = [[0u32; 3]; 2];
        let mut placed = [0u32; 2];
        for trap in self.events.iter().filter_map(|event| match event {
            Event::Caught { kind, placer_team, .. } => Some((*kind as usize, usize::from(*placer_team).min(1))),
            _ => None,
        }) {
            by_team[trap.1][trap.0] += 1;
        }
        for event in &self.events {
            if let Event::Placed { team, .. } = event {
                placed[usize::from(*team).min(1)] += 1;
            }
        }
        // The placer's own catches by cause: how long after placing (a walk-on in the first second, a turn or a return
        // within 10 s, a later route or stand), and whether it was moving at the release.
        let mut own = [[0u32; 2]; 3];
        for event in &self.events {
            if let Event::Caught { kind: Kill::Own, tick, placed, placer_speed, .. } = event {
                let after = tick.saturating_sub(*placed);
                own[if after < 1_000 { 0 } else if after < 10_000 { 1 } else { 2 }][usize::from(*placer_speed >= 0.5)] += 1;
            }
        }
        let own_text = format!(
            "  own catches by cause [still/moving at the release]: within 1 s {}/{}, 1-10 s {}/{}, over 10 s {}/{}",
            own[0][0], own[0][1], own[1][0], own[1][1], own[2][0], own[2][1]
        );
        let by_team_text = format!("{own_text}
") + &format!(
            "  trap catches by the placer's team [t0, t1]: placed {placed:?}, enemy [{}, {}] mate [{}, {}] own [{}, {}]",
            by_team[0][0], by_team[1][0], by_team[0][1], by_team[1][1], by_team[0][2], by_team[1][2]
        );
        let mut by = [[0u32; 2]; 3];
        for event in &self.events {
            if let Event::Caught { kind, placer_crouched, .. } = event {
                by[*kind as usize][usize::from(*placer_crouched)] += 1;
            }
        }
        // The press distance over ALL placed traps (not only the caught): feet to the trap at the release.
        let mut press: Vec<f32> = self
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Placed { press_m, .. } => Some(*press_m),
                _ => None,
            })
            .collect();
        press.sort_by(f32::total_cmp);
        let median = press.get(press.len() / 2).copied().unwrap_or(f32::NAN);
        format!(
            "{by_team_text}
  trap catches [standing/crouched placer]: enemy {}/{} mate {}/{} own {}/{}; press distance of all {} placed: nearest {:.2} m, median {median:.2} m, under 1.0 m {} ({:.1} %)",
            by[0][0],
            by[0][1],
            by[1][0],
            by[1][1],
            by[2][0],
            by[2][1],
            press.len(),
            press.first().copied().unwrap_or(f32::NAN),
            press.iter().filter(|&&m| m < 1.0).count(),
            100.0 * press.iter().filter(|&&m| m < 1.0).count() as f32 / press.len().max(1) as f32,
        )
    }

    /// The objects section a world shows: the live record of each trap, one after another
    /// (`match_msgs::traps`), among other bytes.
    fn section(&self) -> Vec<u8> {
        let mut out = vec![0u8; 3];
        for trap in &self.list {
            let state: u8 = match trap.phase {
                Phase::Armed => 1,
                Phase::Fired { .. } => 2,
                Phase::Disarmed { .. } => 3,
            };
            out.push(state);
            out.extend_from_slice(&u32::MAX.to_le_bytes());
            let rows: [f32; 16] = [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, trap.at.x, trap.at.y, trap.at.z, 1.0,
            ];
            for f in rows {
                out.extend_from_slice(&f.to_le_bytes());
            }
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&trap.placed.to_le_bytes());
            out.extend_from_slice(&[0u8; 3]);
        }
        out
    }
}

/// The distance along the ray `origin + t dir` (`dir` unit, `t` up to `max`) at which it enters the
/// upright cylinder of radius `r` and half-height `half` about `center`, if it does.
pub(crate) fn ray_cylinder(origin: Vec3, dir: Vec3, max: f32, center: Vec3, r: f32, half: f32) -> Option<f32> {
    let (ox, oz) = (origin.x - center.x, origin.z - center.z);
    // The stretch of the ray inside the circle in the ground plane.
    let a = dir.x * dir.x + dir.z * dir.z;
    let (mut t0, mut t1) = (0.0_f32, max);
    if a < 1e-9 {
        if ox * ox + oz * oz > r * r {
            return None;
        }
    } else {
        let b = ox * dir.x + oz * dir.z;
        let c = ox * ox + oz * oz - r * r;
        let disc = b * b - a * c;
        if disc < 0.0 {
            return None;
        }
        let root = disc.sqrt();
        t0 = t0.max((-b - root) / a);
        t1 = t1.min((-b + root) / a);
    }
    // … and between the cylinder's two caps.
    let (lo, hi) = (center.y - half, center.y + half);
    if dir.y.abs() < 1e-9 {
        if origin.y < lo || origin.y > hi {
            return None;
        }
    } else {
        let (s0, s1) = ((lo - origin.y) / dir.y, (hi - origin.y) / dir.y);
        t0 = t0.max(s0.min(s1));
        t1 = t1.min(s0.max(s1));
    }
    (t0 <= t1).then_some(t0)
}

impl Game {
    /// The mesh the client's trap ray meets (the walk geometry), built once.
    fn walker(&mut self) -> Arc<mapdata::RayTree> {
        let world = &self.world;
        self.walls
            .get_or_insert_with(|| {
                let walker = world.map.walker_triangles();
                if walker.is_empty() {
                    world.sight.clone()
                } else {
                    Arc::new(mapdata::RayTree::build(&walker))
                }
            })
            .clone()
    }

    /// The traps put down at this tick: `released` are the keys each body let go of. A body with
    /// a trap left whose key it released looks along its view for the floor.
    pub(crate) fn trap_place(&mut self, released: &[u32], tick: u32) {
        if !model_on() || released.iter().all(|&keys| keys == 0) {
            return;
        }
        for place in 0..self.bodies.len() {
            let body = &self.bodies[place];
            let rig = body.trap;
            if !body.alive || rig.stock == 0 || rig.key == 0 || released[place] & rig.key == 0 {
                continue;
            }
            let eye = Self::eye(body);
            let [fx, fz] = forward(body.yaw);
            let pitch = body.view_elevation();
            let dir = Vec3::new(fx * pitch.cos(), pitch.sin(), fz * pitch.cos());
            let Some(at) = self.trap_land(eye, dir) else {
                continue;
            };
            self.bodies[place].trap.stock -= 1;
            self.trap_lay(place, at, -pitch, tick);
        }
    }

    /// Where a trap let go of from `eye` along `dir` lies: the walk geometry within [`PLACE_REACH_M`], on flat ground, not on an armed
    /// trap's sensor.
    pub(crate) fn trap_land(&mut self, eye: Vec3, dir: Vec3) -> Option<Vec3> {
        let walker = self.walker();
        let hit = walker.ray_hit(eye, dir, PLACE_REACH_M)?;
        if hit.normal.y < FLAT_NORMAL_Y {
            return None;
        }
        let at = eye + dir * hit.t;
        let on_a_sensor = self.traps.list.iter().any(|t| {
            t.phase == Phase::Armed && t.at.distance_xz(at) <= SENSOR_M && (at.y - t.at.y).abs() <= SENSOR_TOP_M
        });
        (!on_a_sensor).then_some(at)
    }

    /// Puts `place`'s trap down at `at` (the floor point [`Game::trap_land`] found) with the view `pitch_down` radians down.
    pub(crate) fn trap_lay(&mut self, place: Place, at: Vec3, pitch_down: f32, tick: u32) {
        let body = &self.bodies[place];
        let eye = Self::eye(body);
        let team = body.team;
        let (placer_crouched, press_m) = (body.keys & key::CROUCH != 0, body.feet.distance_xz(at));
        let eye_m = eye.y - body.feet.y;
        let placer_speed = body.step_m / TICK_S;
        self.traps.list.push(Trap {
            at,
            placer: place,
            team,
            placed: tick,
            placer_crouched,
            press_m,
            eye_m,
            pitch_down,
            placer_speed,
            phase: Phase::Armed,
            defusing: None,
        });
        self.traps.events.push(Event::Placed { tick, place, team, press_m, placer_crouched, placer_speed });
    }

    /// The eye and the unit direction to the floor `ahead_m` from `place` (metres on the ground, `turn` radians from its heading, to
    /// the left), for a stand-in that lets a trap go where it stands: the same eye the key's release would use.
    pub(crate) fn trap_aim_ahead(&self, place: Place, ahead_m: f32, turn: f32) -> (Vec3, Vec3) {
        let body = &self.bodies[place];
        let eye = Self::eye(body);
        // No farther than the ray from the eye reaches the floor (0.1 m of slack under the reach).
        let eye_m = eye.y - body.feet.y;
        let ahead_m = ahead_m.min(((PLACE_REACH_M - 0.1).powi(2) - eye_m.powi(2)).max(0.01).sqrt());
        let [fx, fz] = forward(body.yaw + turn);
        let target = Vec3::new(body.feet.x + fx * ahead_m, body.feet.y, body.feet.z + fz * ahead_m);
        let to = target - eye;
        (eye, to * (1.0 / to.length().max(1e-3)))
    }

    /// The traps' tick: the spent ones go, a body on a sensor is caught, a use held on one runs on.
    pub(crate) fn trap_tick(&mut self, tick: u32) {
        if !model_on() || self.traps.list.is_empty() {
            return;
        }
        self.traps.list.retain(|t| match t.phase {
            Phase::Armed => true,
            Phase::Fired { at } | Phase::Disarmed { at } => tick < at + GONE_MS,
        });
        for t in 0..self.traps.list.len() {
            if self.traps.list[t].phase != Phase::Armed {
                continue;
            }
            let trap = self.traps.list[t];
            // The body nearest the point, among those on the sensor.
            let caught = (0..self.bodies.len())
                .filter(|&p| {
                    let body = &self.bodies[p];
                    let up = body.feet.y - trap.at.y;
                    body.alive
                        && body.feet.distance_xz(trap.at) <= SENSOR_M
                        && (-SENSOR_BELOW_M..=SENSOR_TOP_M).contains(&up)
                })
                .min_by(|&a, &b| {
                    let (da, db) = (
                        self.bodies[a].feet.distance_xz(trap.at),
                        self.bodies[b].feet.distance_xz(trap.at),
                    );
                    da.total_cmp(&db)
                });
            if let Some(victim) = caught {
                self.traps.list[t].phase = Phase::Fired { at: tick };
                let kind = if victim == trap.placer {
                    Kill::Own
                } else if self.bodies[victim].team == trap.team {
                    Kill::Mate
                } else {
                    Kill::Enemy
                };
                self.traps.events.push(Event::Caught { tick, placer: trap.placer, victim, kind, placed: trap.placed, placer_crouched: trap.placer_crouched, press_m: trap.press_m, eye_m: trap.eye_m, pitch_down: trap.pitch_down, placer_team: trap.team, victim_team: self.bodies[victim].team, placer_speed: trap.placer_speed });
                let body = &mut self.bodies[victim];
                body.legs = [0.0; 2];
                body.leg_broken_until = body.leg_broken_until.max(tick + LEGS_MS);
                body.trap.slow_until = tick + LEGS_MS;
                body.pools.hurt(standin_damage::Pool::Pain, PAIN, tick);
                continue;
            }
            self.trap_defuse(t, tick);
        }
    }

    /// A hand defuse of trap `t`: who holds USE with the eye ray on it, for how long.
    fn trap_defuse(&mut self, t: usize, tick: u32) {
        let trap = self.traps.list[t];
        let user = (0..self.bodies.len()).find(|&p| {
            let body = &self.bodies[p];
            if !body.alive || body.keys & key::USE == 0 || (p != trap.placer && body.team == trap.team) {
                return false;
            }
            let eye = Self::eye(body);
            let [fx, fz] = forward(body.yaw);
            let pitch = body.view_elevation();
            let dir = Vec3::new(fx * pitch.cos(), pitch.sin(), fz * pitch.cos());
            let to = trap.middle() - eye;
            let along = to.dot(dir);
            along > 0.0
                && along <= USE_REACH_M
                && (to - dir * along).length() <= USE_TOL_M
                && self.world.sight.visible(eye, trap.middle())
        });
        let trap = &mut self.traps.list[t];
        match (user, trap.defusing) {
            (None, _) => trap.defusing = None,
            (Some(p), Some((q, _))) if p == q => {}
            (Some(p), _) => trap.defusing = Some((p, tick)),
        }
        if let Some((by, since)) = trap.defusing
            && tick >= since + DEFUSE_MS
        {
            trap.phase = Phase::Disarmed { at: tick };
            trap.defusing = None;
            let (placer, placed) = (trap.placer, trap.placed);
            self.traps.events.push(Event::DefusedBy { tick, by, placer, placed });
        }
    }

    /// A bullet from `eye` along `dir` (unit) that would hit a body `body_t` away, if any: the
    /// armed trap it meets first, if any, is disarmed. Whoever shot.
    pub(crate) fn trap_shot(&mut self, shooter: Place, eye: Vec3, dir: Vec3, body_t: Option<f32>, tick: u32) {
        if !model_on() || self.traps.list.is_empty() {
            return;
        }
        let mut first: Option<(usize, f32)> = None;
        for (t, trap) in self.traps.list.iter().enumerate() {
            if trap.phase != Phase::Armed {
                continue;
            }
            let Some(along) = ray_cylinder(eye, dir, 100.0, trap.middle(), HIT_R, HIT_HALF) else {
                continue;
            };
            if first.is_none_or(|(_, d)| along < d) {
                first = Some((t, along));
            }
        }
        let Some((t, along)) = first else {
            return;
        };
        if body_t.is_some_and(|b| b < along) || self.world.sight.ray_cast(eye, dir, along).is_some() {
            return;
        }
        let trap = &mut self.traps.list[t];
        trap.phase = Phase::Disarmed { at: tick };
        trap.defusing = None;
        let (placer, placed) = (trap.placer, trap.placed);
        self.traps.events.push(Event::ShotOff { tick, shooter, placer, placed });
    }

    /// The world of this tick keeps the objects section it shows.
    pub(crate) fn trap_record(&mut self) {
        if !model_on() {
            return;
        }
        let section = self.traps.section();
        self.traps.worlds.push_back((self.tick, section));
        while self.traps.worlds.len() > 40 {
            self.traps.worlds.pop_front();
        }
    }
}

/// Tells the bots the world of `tick` as the game shows its traps; and the stand-in learns the
/// kit each bot carries (a body's stock is filled once a life).
pub(crate) fn tell(game: &mut Game, bots: &mut Bots, tick: u32) {
    if !model_on() {
        return;
    }
    for (place, body) in game.bodies.iter_mut().enumerate() {
        if let Some(Some(bot)) = bots.bots.get(place)
            && let Some(kit) = bot.features.traps.habit.kit()
        {
            body.trap.learn(kit, body.spawned);
        }
    }
    let Some((_, section)) = game.traps.worlds.iter().find(|(t, _)| *t == tick) else {
        return;
    };
    bots.traps_from_world(section, tick);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::traps::Habit;
    use survarium::v026g::match_msgs::traps::{TrapState, find};

    const KEY: u32 = key::QUICK[1];

    /// Two bodies of two teams on the soak field, `a` and `b` in place: bodies 0 and 1.
    fn field(a: Vec3, b: Vec3) -> Game {
        let world = soak_world();
        let teams = [0u8, 1];
        let mut game = Game::new(world.clone(), &teams, 5, Some(30), None);
        for (place, at) in [(0, a), (1, b)] {
            game.bodies[place].node = world.grid.locate(at, 1.0);
            game.bodies[place].feet = at;
            game.bodies[place].alive = true;
        }
        game
    }

    fn start() -> Game {
        field(Vec3::new(14.0, 0.0, 12.0), Vec3::new(70.0, 0.0, 12.0))
    }

    /// `keys` for `place` at the next tick, and the tick played.
    fn feed(game: &mut Game, place: Place, keys: u32) {
        let at = game.tick + TICK_MS;
        let gun = Arms::fallback().primary.0;
        game.inputs[place].push_back((at, keys, (0.0, 0.0), None, gun));
        game.step();
    }

    /// The first body carries two traps on `KEY`, looks down at its feet and lets go of the key.
    fn placed(game: &mut Game) {
        let spawned = game.bodies[0].spawned;
        game.bodies[0].trap.learn(TrapKit { count: 2, key: KEY }, spawned);
        game.bodies[0].pitch = -1.0;
        feed(game, 0, KEY);
        feed(game, 0, 0);
    }

    #[test]
    fn a_ray_meets_the_traps_cylinder_where_it_is() {
        let middle = Vec3::new(10.0, 0.12, 5.0);
        let level = |dy: f32| {
            ray_cylinder(Vec3::new(10.0, 0.12 + dy, 0.0), Vec3::new(0.0, 0.0, 1.0), 20.0, middle, HIT_R, HIT_HALF)
        };
        // Along the ground plane at the middle's height: in 5 - 0.125 m.
        assert!((level(0.0).unwrap() - 4.875).abs() < 1e-4);
        // Within the half-height, in; over it, past.
        assert!(level(0.049).is_some());
        assert!(level(0.06).is_none());
        // From above, down the axis: the top cap, 0.05 over the middle.
        let down = ray_cylinder(Vec3::new(10.0, 1.12, 5.0), Vec3::new(0.0, -1.0, 0.0), 5.0, middle, HIT_R, HIT_HALF);
        assert!((down.unwrap() - 0.95).abs() < 1e-4);
        // Beside it, out; too short, out.
        let side = ray_cylinder(Vec3::new(10.2, 0.12, 0.0), Vec3::new(0.0, 0.0, 1.0), 20.0, middle, HIT_R, HIT_HALF);
        assert!(side.is_none());
        let short = ray_cylinder(Vec3::new(10.0, 0.12, 0.0), Vec3::new(0.0, 0.0, 1.0), 4.0, middle, HIT_R, HIT_HALF);
        assert!(short.is_none());
    }

    /// A body that carries traps looks down and lets go of the key: a trap lies where its view meets the floor,
    /// armed, one fewer in the pocket; the same spot again, and a body with no traps, put nothing down.
    #[test]
    fn a_released_key_puts_a_trap_where_the_view_meets_the_floor() {
        let mut game = start();
        placed(&mut game);
        assert_eq!(game.traps.list.len(), 1, "{:?}", game.traps.events);
        let trap = game.traps.list[0];
        assert_eq!((trap.phase, trap.placer, trap.team), (Phase::Armed, 0, 0));
        assert!(trap.at.y.abs() < 0.05, "on the floor: {:?}", trap.at);
        let ahead = trap.at.distance_xz(game.bodies[0].feet);
        assert!((0.5..=PLACE_REACH_M).contains(&ahead), "{ahead} m ahead");
        assert_eq!(game.bodies[0].trap.stock, 1);
        // The key held does nothing until it is let go; on the same spot a second one is refused.
        feed(&mut game, 0, KEY);
        assert_eq!(game.traps.list.len(), 1);
        feed(&mut game, 0, 0);
        assert_eq!(game.traps.list.len(), 1, "on the first one's sensor");
        assert_eq!(game.bodies[0].trap.stock, 1);
        // A body that carries none puts none.
        feed(&mut game, 1, KEY);
        feed(&mut game, 1, 0);
        assert_eq!(game.traps.list.len(), 1);
        // Looking along the ground, the ray (2 m) reaches no floor: none.
        let mut level = start();
        let spawned = level.bodies[0].spawned;
        level.bodies[0].trap.learn(TrapKit { count: 1, key: KEY }, spawned);
        level.bodies[0].pitch = 0.0;
        feed(&mut level, 0, KEY);
        feed(&mut level, 0, 0);
        assert!(level.traps.list.is_empty());
    }

    /// A body on the sensor is caught, any team's, the placer too: the trap is fired, both legs are 0 for 8 s
    /// (no jump, no sprint, the walk at 0.6), the pain is taken, and the trap is gone after 8 s. A body in the
    /// air above 0.22 m, or on another floor, is not caught.
    #[test]
    fn a_body_on_the_sensor_is_caught_and_its_legs_are_taken() {
        let mut game = start();
        placed(&mut game);
        let at = game.traps.list[0].at;
        // A jump over it: feet 0.3 m above its floor point.
        game.bodies[1].feet = Vec3::new(at.x + 0.2, at.y + 0.3, at.z);
        game.trap_tick(game.tick);
        assert_eq!(game.traps.list[0].phase, Phase::Armed);
        // Another floor, 3 m up.
        game.bodies[1].feet = Vec3::new(at.x, at.y + 3.0, at.z);
        game.trap_tick(game.tick);
        assert_eq!(game.traps.list[0].phase, Phase::Armed);
        // The foot point 0.5 m off, on the floor.
        game.bodies[1].feet = Vec3::new(at.x + 0.5, at.y, at.z);
        let tick = game.tick;
        let pain = game.bodies[1].pools.pain;
        game.trap_tick(tick);
        assert_eq!(game.traps.list[0].phase, Phase::Fired { at: tick });
        let body = &game.bodies[1];
        assert_eq!(body.legs, [0.0; 2]);
        assert!(body.leg_broken_until >= tick + LEGS_MS);
        assert!(body.trap.slowed(tick + LEGS_MS - TICK_MS) && !body.trap.slowed(tick + LEGS_MS));
        assert!((pain - body.pools.pain - PAIN).abs() < 1e-4, "{pain} -> {}", body.pools.pain);
        assert_eq!(game.traps.counts(), [1, 1, 0, 0, 0, 0]);
        // The spent trap stays 8 s, then is gone.
        game.trap_tick(tick + GONE_MS - TICK_MS);
        assert_eq!(game.traps.list.len(), 1);
        game.trap_tick(tick + GONE_MS);
        assert!(game.traps.list.is_empty());
        // The placer's own foot, and a mate's, are caught as well.
        for (stepper, kind) in [(0, Kill::Own), (1, Kill::Mate)] {
            let mut game = start();
            placed(&mut game);
            let at = game.traps.list[0].at;
            if stepper == 1 {
                game.bodies[1].team = 0;
            }
            game.bodies[0].feet = Vec3::new(0.0, 0.0, 0.0);
            game.bodies[stepper].feet = Vec3::new(at.x + 0.1, at.y, at.z);
            game.trap_tick(game.tick);
            assert_eq!(game.traps.counts()[1..4].iter().sum::<u32>(), 1);
            assert!(matches!(game.traps.events.last(), Some(Event::Caught { kind: k, .. }) if *k == kind));
            // The catch line: whose body, the placer's pose, how long after, how far from its feet at the press.
            let lines = game.traps.catch_lines();
            assert_eq!(lines.len(), 1, "{lines:?}");
            let word = if kind == Kill::Own { "own" } else { "mate" };
            assert!(
                lines[0].contains(&format!("trap catch: {word} (placer 0 of team 0, victim {stepper}) placer standing"))
                    && lines[0].contains("s after placing, ")
                    && lines[0].contains(" m from its feet at the press (eye "),
                "{lines:?}"
            );
            assert!(game.traps.catch_summary().contains("press distance of all 1 placed: nearest "));
        }
    }

    /// The catch holds the legs in the game's step: a sprinting body moves at 0.6 of its walk, not its run.
    #[test]
    fn a_caught_body_walks_slow_and_does_not_sprint() {
        let walked = |caught: bool| {
            let mut game = start();
            game.bodies[0].yaw = 0.0;
            if caught {
                game.bodies[0].trap.slow_until = game.tick + LEGS_MS;
            }
            let from = game.bodies[0].feet;
            for _ in 0..40 {
                feed(&mut game, 0, key::FORWARD | key::SPRINT);
            }
            game.bodies[0].feet.distance_xz(from)
        };
        let (free, held) = (walked(false), walked(true));
        eprintln!("free {free:.2} m, caught {held:.2} m");
        assert!(held < free * 0.7, "{held} against {free}");
        assert!(held > 0.0);
    }

    /// A bullet that meets the cylinder disarms the trap, whoever shot; one above it, beside it, or behind a body
    /// does not.
    #[test]
    fn a_bullet_on_the_cylinder_disarms_the_trap() {
        let mut game = start();
        placed(&mut game);
        let trap = game.traps.list[0];
        let eye = Vec3::new(trap.at.x, trap.at.y + 1.5, trap.at.z - 8.0);
        let toward = |to: Vec3| (to - eye) * (1.0 / (to - eye).length());
        let tick = game.tick;
        // Too high by 0.2 m, then too far to the side, then with a body nearer on the line.
        game.trap_shot(1, eye, toward(trap.middle() + Vec3::new(0.0, 0.2, 0.0)), None, tick);
        game.trap_shot(1, eye, toward(trap.middle() + Vec3::new(0.3, 0.0, 0.0)), None, tick);
        game.trap_shot(1, eye, toward(trap.middle()), Some(3.0), tick);
        assert_eq!(game.traps.list[0].phase, Phase::Armed);
        // On it: the enemy's bullet.
        game.trap_shot(1, eye, toward(trap.middle()), Some(30.0), tick);
        assert_eq!(game.traps.list[0].phase, Phase::Disarmed { at: tick });
        assert_eq!(game.traps.events.last(), Some(&Event::ShotOff { tick, shooter: 1, placer: 0, placed: game.traps.list[0].placed }));
        // A disarmed trap is no target, and is gone after 8 s.
        game.trap_shot(0, eye, toward(trap.middle()), None, tick);
        assert_eq!(game.traps.counts()[4], 1);
        game.trap_tick(tick + GONE_MS);
        assert!(game.traps.list.is_empty());
    }

    /// A use held 5 s with the eye ray on the trap, by its placer or a body of the other team, disarms it; a mate of the
    /// placer cannot, and letting go of the key starts it over.
    #[test]
    fn a_use_held_five_seconds_disarms_the_trap() {
        // The user stands 1 m south of the trap, facing north and looking down at it.
        let setup = |game: &mut Game, user: Place, at: Vec3| {
            let b = &mut game.bodies[user];
            b.feet = Vec3::new(at.x, at.y, at.z - 1.0);
            b.yaw = 0.0;
            let to = Vec3::new(at.x, at.y + HIT_UP, at.z) - Game::eye(b);
            b.pitch = (to.y / to.length()).asin();
            b.recoil.reset();
        };
        let hold = |game: &mut Game, user: Place, ms: u32| {
            for _ in 0..ms / TICK_MS {
                game.tick += TICK_MS;
                game.bodies[user].keys = key::USE;
                game.trap_tick(game.tick);
            }
        };
        let mut game = start();
        placed(&mut game);
        let at = game.traps.list[0].at;
        setup(&mut game, 1, at);
        // A pause of one tick restarts the five seconds.
        hold(&mut game, 1, 3_000);
        game.bodies[1].keys = 0;
        game.tick += TICK_MS;
        game.trap_tick(game.tick);
        hold(&mut game, 1, 3_000);
        assert_eq!(game.traps.list[0].phase, Phase::Armed, "6 s in all, but not 5 in a row");
        hold(&mut game, 1, 2_050);
        assert!(matches!(game.traps.list[0].phase, Phase::Disarmed { .. }), "{:?}", game.traps.events);
        assert!(matches!(game.traps.events.last(), Some(Event::DefusedBy { by: 1, placer: 0, .. })));
        // A mate of the placer: nothing. The placer: it does.
        for (user, team, disarmed) in [(1, 0, false), (0, 0, true)] {
            let mut game = start();
            placed(&mut game);
            assert_eq!(game.traps.list[0].at, at);
            game.bodies[1].team = team;
            setup(&mut game, user, at);
            hold(&mut game, user, 5_100);
            assert_eq!(matches!(game.traps.list[0].phase, Phase::Disarmed { .. }), disarmed, "user {user}");
        }
    }

    /// The objects section the worlds keep reads back through the decoder the server has: the state, the place and
    /// the placing time of each trap.
    #[test]
    fn the_worlds_show_the_traps_as_the_live_game_does() {
        let mut game = start();
        placed(&mut game);
        let tick = game.tick;
        let at = game.traps.list[0].at;
        game.trap_record();
        let found = find(&game.traps.worlds.back().unwrap().1, tick);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].state, TrapState::Armed);
        assert!((found[0].position[0] - at.x).abs() < 1e-5 && (found[0].position[2] - at.z).abs() < 1e-5);
        assert_eq!(found[0].placed_ms, game.traps.list[0].placed);
        // Fired and disarmed read as theirs.
        game.traps.list[0].phase = Phase::Fired { at: tick };
        assert_eq!(find(&game.traps.section(), tick)[0].state, TrapState::Fired);
        game.traps.list[0].phase = Phase::Disarmed { at: tick };
        assert_eq!(find(&game.traps.section(), tick)[0].state, TrapState::Disarmed);
        // And with two, both are found.
        let mut second = game.traps.list[0];
        second.at = Vec3::new(at.x + 3.0, at.y, at.z);
        second.phase = Phase::Armed;
        game.traps.list.push(second);
        assert_eq!(find(&game.traps.section(), tick).len(), 2);
    }

    /// The model switched off puts no trap down and shows no world: the run is the one without it.
    #[test]
    fn the_model_off_is_the_game_without_traps() {
        force_model(Some(false));
        let mut game = start();
        placed(&mut game);
        let nothing = game.traps.list.is_empty() && game.traps.worlds.is_empty();
        force_model(None);
        assert!(nothing);
    }

    /// The habit of a bot that carries traps, in the stand-in: it puts one down (the key released on a floor in
    /// reach), the world shows it, the team's list is confirmed by that world, and an enemy that steps on it is caught.
    #[test]
    fn a_bots_trap_is_placed_shown_confirmed_and_caught_in_the_stand_in() {
        use crate::v026g::bots::trap_know::Phase as Known;
        let world = soak_world();
        let teams = [0u8, 1];
        let mut bots = Bots::new(
            0x7A95,
            survarium::v026g::maps::mode::TEAM_DEATHMATCH,
            0,
            &teams,
            &[true, false],
            &[None, None],
            Arc::new(Tuning::all_off()),
            Trace::none(0),
        );
        bots.map_ready(world.clone());
        let mut game = Game::new(world.clone(), &teams, 5, Some(30), None);
        for (place, at) in [(0, Vec3::new(14.0, 0.0, 12.0)), (1, Vec3::new(70.0, 0.0, 12.0))] {
            game.bodies[place].node = world.grid.locate(at, 1.0);
            game.bodies[place].feet = at;
        }
        bots.entered(0, true, 0);
        bots.bots[0].as_mut().unwrap().features.traps.habit = Habit::carry(TrapKit { count: 2, key: KEY });
        // The team has known enemy humans all over this half of the field for a while.
        let mut who = 100;
        for x in (6..=42).step_by(6) {
            for z in (0..=36).step_by(6) {
                who += 1;
                for k in 0..8 {
                    bots.teams[0].knowledge.humans.knew(who, Vec3::new(x as f32, 0.0, z as f32), k * 1_000);
                }
            }
        }
        let (mut clock, mut next_world) = (0, 500);
        while clock < 40_000 {
            clock += 25;
            while game.tick + TICK_MS <= clock {
                game.step();
            }
            if clock >= next_world {
                next_world += 500;
                let world_tick = clock / TICK_MS * TICK_MS - 300;
                standin_traps::tell(&mut game, &mut bots, world_tick);
                if let Some((_, fixes, health, facts)) = game.world_at(world_tick) {
                    for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                        bots.fixed(place, fix, Some(health));
                    }
                    bots.facts(facts);
                }
            }
            let (made, _) = bots.poll(Clock { clock, time_left_s: 600.0, total_s: 900.0, playing: true });
            for made in made {
                let r = made.input.rotation;
                let turn = (
                    half_value(u16::from_le_bytes([r[0], r[1]])),
                    half_value(u16::from_le_bytes([r[2], r[3]])),
                );
                let gun = Arms::fallback().primary.0;
                // The body stands still once the habit has begun: it plans its spot from where the bot stands, and a
                // walk of 2 m during the look (3 m/s for 0.6 s) puts the trap past the 1.5 m the team's list takes
                // as the same (`trap_know::MATCH_M`): a finding for the habit, not for this model.
                let placing = !matches!(bots.bots[0].as_ref().unwrap().features.traps.habit.state(), crate::v026g::bots::traps::State::Idle);
                let walk = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::SPRINT;
                let actions = if placing && made.place == 0 { made.input.actions & !walk } else { made.input.actions };
                game.inputs[made.place].push_back((made.time, actions, turn, None, gun));
            }
        }
        assert_eq!(game.traps.counts()[0], 1, "{:?}", game.traps.events);
        let known = bots.teams[0].knowledge.traps.all();
        assert_eq!(known.len(), 1);
        assert_eq!(known[0].phase, Known::Armed, "the world confirmed it");
        let at = game.traps.list[0].at;
        assert!(known[0].at.distance_xz(at) < 0.01, "the exact place: {:?} against {at:?}", known[0].at);
        // An enemy walks onto it.
        game.bodies[1].feet = Vec3::new(at.x + 0.3, at.y, at.z);
        game.bodies[1].node = world.grid.locate(game.bodies[1].feet, 1.0);
        game.trap_tick(game.tick);
        assert_eq!(game.traps.counts()[1], 1, "{:?}", game.traps.events);
    }
}

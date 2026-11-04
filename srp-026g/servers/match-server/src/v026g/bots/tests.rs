//! Soak runs (`player_bots_research.md` B8 point 5): whole matches of bots
//! against a stand-in for the game. Only a real client simulates a match,
//! so these runs stand in for it with a small game of their own:
//! - bodies move by the bots' inputs at their ticks with the movement
//!   model (`kin`), kept on the walk grid;
//! - a shot hits the first enemy whose chest lies within 0.45 m of the
//!   view's ray, with a clear line on the bullet mesh; five hits kill;
//! - the weapon each bot holds fires as the client fires it: an automatic
//!   one at its rate while the trigger is held, any other once a press;
//!   and kicks as the client kicks ([`super::recoil`]): the shots lift the
//!   view itself (the base pitch and the kick), a downward turn is taken
//!   off the kick first, the kick comes back by the weapon's controller;
//!   each shot's kick drawn in the config's range (the bots predict the
//!   middle);
//! - the dead respawn at their side's spawns after 6.4 s (the radio);
//! - a use takes a battery, puts it in at one's own container, or steals
//!   from the other's, as the client's use does: only what the view's ray
//!   from the eye meets within 2 m (`base_player::detect_usable_object`):
//!   within 0.2 m of a battery's middle, 0.6 m of a use box's; a drop
//!   drops it;
//! - Team Deathmatch: each death costs the team a ticket; a team out of
//!   tickets no longer respawns, and loses when its last body dies;
//! - a world of every body, the statistics and the batteries comes every
//!   second, 300 ms behind the clock, as the match gives the bots.
//!
//! The stand-in is much simpler than the game (no spread, no side kick, no
//! armour, no stamina, no jumps; the rounds are not counted, the bots count
//! their own): the runs check that bots of two teams find each
//! other, fight, die, come back, take and bring batteries and end a
//! match, with no input out of the rules; not how well.
//! `SRP_BOT_NOTES=1` prints what each bot did.

use std::collections::VecDeque;
use std::sync::Arc;

use mapdata::{BatterySpawner, Container, NodeId, Vec3, World};
use survarium::v026g::match_msgs::TICK_MS;

use super::body::{Fix, slide};
use super::guns::{Arms, Gun};
use super::kin::{Blend, Kinematics, TICK_S, forward, to_world};
use super::recoil::{Cadence, Recoil, kick_of};
use super::rng::Rng;
use super::trace::Trace;
use super::tuning::Tuning;
use super::view::{BatteryState, Clock, WorldFacts};
use super::*;
use crate::v026g::drive::{half_value, key};

/// The client's use: a ray from the eye this long (`usable_objects_detection_distance`).
const USE_REACH_M: f32 = 2.0;
/// A lying battery's middle over its position, and how near the ray must
/// pass it (its box is about 0.5 by 0.25 m).
const BATTERY_MIDDLE: Vec3 = Vec3::new(0.0, 0.12, 0.0);
const BATTERY_M: f32 = 0.2;
/// A container's use box: its middle over the use point
/// (`Container::use_point`), and how near the ray must pass it.
const USE_BOX_UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const USE_BOX_M: f32 = 0.6;

/// A body of the stand-in game.
#[derive(Clone, Debug)]
struct Body {
    /// The weapon in hand (the bot's, as its last input was made), its
    /// kick and its trigger.
    gun: &'static Gun,
    recoil: Recoil,
    cadence: Cadence,
    team: u8,
    feet: Vec3,
    node: Option<NodeId>,
    yaw: f32,
    /// The base pitch (`m_look_pitch`, as the worlds give it); the view is
    /// it and the kick.
    pitch: f32,
    blend: Blend,
    keys: u32,
    health: f32,
    alive: bool,
    dead_at: u32,
    spawned: u32,
    carrying: Option<usize>,
    moved: f32,
    /// The bot's target as its last input was made (the reflex's).
    target: Option<Place>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Lying {
    /// Where, and the HUD mask as the client keeps it
    /// (`TeamKnowledge::hud_update`): 0 when it appears, the carrier's
    /// team's bit alone when he drops it or dies with it.
    Free(Vec3, u32),
    Carried(Place),
    Home(u8),
}

type Recorded = (u32, Vec<Fix>, Vec<f32>, WorldFacts);
/// An input for a tick: the tick, the keys, the turn, the bot's target
/// and weapon in hand.
type Queued = (u32, u32, (f32, f32), Option<Place>, &'static Gun);

/// A stolen battery: the team it was stolen from, and when (ms) and how
/// (alive) it fell since, if it did.
type StolenMark = (u8, Option<(u32, bool)>);

/// The stand-in game.
struct Game {
    world: Arc<World>,
    kin: Kinematics,
    bodies: Vec<Body>,
    batteries: Vec<Lying>,
    containers: Vec<(u8, Vec3)>,
    stats: Vec<(u32, u32, u32)>,
    points: [u32; 2],
    /// Team Deathmatch's tickets.
    tickets: Option<[u32; 2]>,
    /// Timed batteries: the next one's time, and every how long one
    /// appears by a station (round the stations in turn).
    timed: Option<(u32, u32)>,
    inputs: Vec<VecDeque<Queued>>,
    /// The worlds of the last two seconds.
    history: VecDeque<Recorded>,
    tick: u32,
    rng: Rng,
    shots: u32,
    hits: u32,
    /// Shots at the bot's target whose ray met the world short of him, and
    /// those with nothing of him showing.
    walled: u32,
    hidden: u32,
    /// Rays the counts above cast (not the bots').
    metric_rays: u64,
    delivered: [u32; 2],
    stolen: u32,
    /// The places that took a battery out of the enemy's container, in
    /// order (the bench reads them by the thief's play).
    thieves: Vec<Place>,
    /// Carriers killed with a battery: (the shooter, the carrier), in
    /// order (the bench reads them by the shooter's play).
    carrier_kills: Vec<(Place, Place)>,
    /// By battery: stolen from this team's container, and when (ms) and
    /// how (alive: dropped by a live carrier) it fell since, if it did.
    stolen_marks: Vec<Option<StolenMark>>,
    /// A stolen battery that fell, until it was home again: its owners'
    /// container (true) or the thief's side's, ms from the fall, alive.
    stolen_drops: Vec<(bool, u32, bool)>,
    /// What befell stolen batteries, for the bench's log.
    stolen_log: Vec<String>,
    /// Lying batteries taken.
    taken: u32,
    /// Whether a use's ray stops at the bullet mesh short of a lying
    /// battery (`SRP_BENCH_USE_LINE=1` in the real-map runs). LIKELY as
    /// the client: in the owner's match of 2026-10-16 a bot aimed right
    /// at a battery 9 times from where the mesh blocks the line, and took
    /// it at once from a side with a clear one.
    use_line: bool,
}

/// The soak map: the 80 x 40 m field (a wall across its middle up to
/// z = 25), side 0 spawning at x = 5, side 1 at x = 75, their containers
/// near their spawns and three batteries between.
fn soak_world() -> Arc<World> {
    soak_world_with(&[])
}

/// [`soak_world`] with `solids` (more triangles) in its geometry.
fn soak_world_with(solids: &[mapdata::Triangle]) -> Arc<World> {
    super::tactics::tests::field_with_solids(solids, |map| {
        let container = |team: i32, id: i32, x: f32, z: f32| Container {
            position: Vec3::new(x, 0.0, z),
            team,
            id,
            use_shapes: Vec::new(),
        };
        map.containers = vec![container(0, 0, 12.0, 30.0), container(1, 1, 68.0, 30.0)];
        map.stations = vec![Vec3::new(40.0, 0.0, 33.0), Vec3::new(30.0, 0.0, 5.0)];
        map.battery_spawners = [(40.0, 33.0), (25.0, 12.0), (55.0, 12.0)]
            .iter()
            .map(|&(x, z)| BatterySpawner {
                position: Vec3::new(x, 0.0, z),
                item: Vec3::new(x, 0.3, z),
                layer: "default".into(),
            })
            .collect();
    })
}

impl Game {
    fn new(
        world: Arc<World>,
        teams: &[u8],
        seed: u64,
        tickets: Option<u32>,
        timed: Option<u32>,
    ) -> Self {
        let containers = world
            .map
            .containers
            .iter()
            .map(|c| (c.team as u8, c.use_point()))
            .collect();
        // Timed batteries start with none lying about.
        let batteries = if timed.is_some() {
            Vec::new()
        } else {
            world
                .map
                .battery_spawners
                .iter()
                .map(|s| Lying::Free(s.item, 0))
                .collect()
        };
        let gun = Arms::fallback().primary.0;
        let body = |team| Body {
            gun,
            recoil: Recoil::new(kick_of(gun.item)),
            cadence: Cadence::default(),
            team,
            feet: Vec3::ZERO,
            node: None,
            yaw: 0.0,
            pitch: 0.0,
            blend: Blend::default(),
            keys: 0,
            health: 1.0,
            alive: false,
            dead_at: 0,
            spawned: 0,
            carrying: None,
            moved: 0.0,
            target: None,
        };
        let mut game = Self {
            kin: Kinematics::default(),
            bodies: teams.iter().map(|&team| body(team)).collect(),
            batteries,
            containers,
            stats: vec![(0, 0, 0); teams.len()],
            points: [0, 0],
            tickets: tickets.map(|t| [t, t]),
            timed: timed.map(|every| (every, every)),
            inputs: vec![VecDeque::new(); teams.len()],
            history: VecDeque::new(),
            tick: 0,
            rng: Rng::new(seed),
            shots: 0,
            hits: 0,
            walled: 0,
            hidden: 0,
            metric_rays: 0,
            delivered: [0, 0],
            stolen: 0,
            thieves: Vec::new(),
            carrier_kills: Vec::new(),
            stolen_marks: Vec::new(),
            stolen_drops: Vec::new(),
            stolen_log: Vec::new(),
            taken: 0,
            use_line: false,
            world,
        };
        for place in 0..teams.len() {
            game.spawn(place);
        }
        game
    }

    fn spawn(&mut self, place: Place) {
        let side = i32::from(self.bodies[place].team);
        let spawns: Vec<Vec3> = self
            .world
            .map
            .spawns
            .iter()
            .filter(|s| s.team == side)
            .map(|s| s.position)
            .collect();
        let at = spawns[self.rng.below(spawns.len())]
            + Vec3::new(self.rng.range(-2.0, 2.0), 0.0, self.rng.range(-2.0, 2.0));
        let node = self.world.grid.locate(at, 3.0);
        let feet = node.map_or(at, |n| self.world.grid.position(n));
        let tick = self.tick;
        let body = &mut self.bodies[place];
        body.feet = feet;
        body.node = node;
        // Facing the other side.
        body.yaw = if side == 0 {
            -std::f32::consts::FRAC_PI_2
        } else {
            std::f32::consts::FRAC_PI_2
        };
        body.pitch = 0.0;
        body.blend = Blend::default();
        body.health = 1.0;
        body.alive = true;
        body.spawned = tick;
        body.carrying = None;
        // Weapon 1 in hand afresh.
        body.gun = Arms::fallback().primary.0;
        body.recoil = Recoil::new(kick_of(body.gun.item));
        body.cadence = Cadence::default();
    }

    fn eye(body: &Body) -> Vec3 {
        let up = if body.keys & key::CROUCH != 0 {
            0.95
        } else {
            1.6
        };
        body.feet + Vec3::new(0.0, up, 0.0)
    }

    fn chest(body: &Body) -> Vec3 {
        let up = if body.keys & key::CROUCH != 0 {
            0.8
        } else {
            1.3
        };
        body.feet + Vec3::new(0.0, up, 0.0)
    }

    /// One tick: each weapon's kick moves on, each body takes its input
    /// (the world of the tick shows it, before the move; a downward turn
    /// is taken off the kick first), then moves; shots, uses, drops,
    /// deaths, respawns.
    fn step(&mut self) {
        self.tick += TICK_MS;
        let tick = self.tick;
        let n = self.bodies.len();
        let mut pressed = vec![0u32; n];
        for body in &mut self.bodies {
            body.recoil.update(tick);
        }
        for (place, queue) in self.inputs.iter_mut().enumerate() {
            while let Some(&(at, keys, turn, target, gun)) = queue.front() {
                if at > tick {
                    break;
                }
                assert_eq!(at, tick, "an input for a tick already played");
                queue.pop_front();
                let body = &mut self.bodies[place];
                pressed[place] = keys & !body.keys;
                body.keys = keys;
                body.target = target;
                // Another weapon taken, or a reload: no kick.
                if gun.item != body.gun.item {
                    body.gun = gun;
                    body.recoil = Recoil::new(kick_of(gun.item));
                    body.recoil.update(tick);
                    body.cadence = Cadence::default();
                } else if pressed[place] & key::RELOAD != 0 {
                    body.recoil.reset();
                }
                body.yaw += turn.0;
                let taken = body.recoil.turn(turn.1);
                body.pitch = (body.pitch + turn.1 + taken).clamp(-1.0, 1.0);
            }
        }
        self.record();
        for place in 0..n {
            if !self.bodies[place].alive {
                let team = usize::from(self.bodies[place].team);
                let may = self.tickets.is_none_or(|tickets| tickets[team] > 0);
                if may && tick >= self.bodies[place].dead_at + 6_400 {
                    self.spawn(place);
                }
                continue;
            }
            let body = &mut self.bodies[place];
            let level = if body.carrying.is_some() { 0.95 } else { 1.0 };
            let local = body.blend.tick(
                self.kin.target_local(body.keys, level),
                self.kin.blend_ticks,
            );
            let v = to_world(local, body.yaw);
            let to = Vec3::new(
                body.feet.x + v[0] * TICK_S,
                body.feet.y,
                body.feet.z + v[1] * TICK_S,
            );
            if let Some(node) = body.node {
                let (node, feet, _) = slide(&self.world.grid, node, body.feet, to);
                body.moved += feet.distance_xz(body.feet);
                body.node = Some(node);
                body.feet = feet;
            }
        }
        self.shoot(tick);
        self.use_and_drop(&pressed);
        // Timed batteries: one appears by the next station in turn.
        if let Some((next, every)) = self.timed
            && tick >= next
        {
            let stations = &self.world.map.stations;
            let at = stations[self.batteries.len() % stations.len()];
            self.batteries
                .push(Lying::Free(at + Vec3::new(1.5, 0.3, 0.0), 0));
            self.timed = Some((next + every, every));
        }
    }

    /// Shots, each along the view (the base pitch and the kick) and each
    /// kicking it: a hit on the nearest enemy chest within 0.45 m of the
    /// ray.
    fn shoot(&mut self, tick: u32) {
        for shooter in 0..self.bodies.len() {
            let body = &mut self.bodies[shooter];
            let held = body.alive && body.keys & key::FIRE != 0 && body.carrying.is_none();
            let (auto, interval_ms) = (body.gun.auto, body.gun.shot_ms());
            let mut times = Vec::new();
            body.cadence
                .tick(tick, held, auto, interval_ms, |t| times.push(t));
            for at in times {
                self.shot(shooter, at);
            }
        }
    }

    /// One shot of `shooter` at `at` ms (within the tick).
    fn shot(&mut self, shooter: Place, at: f32) {
        self.shots += 1;
        let body = &self.bodies[shooter];
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let pitch = body.recoil.view(body.pitch);
        let (c, s) = (pitch.cos(), pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        let kick = body.recoil.kick();
        let top = kick.top(self.rng.unit());
        self.bodies[shooter].recoil.shot(at, top, 0.0);
        let tick = self.tick;
        let body = &self.bodies[shooter];
        // A shot at the bot's target with nothing of him showing
        // (`aimpoint`'s parts), or whose ray meets the world short of
        // him.
        if let Some(target) = body.target.map(|t| &self.bodies[t])
            && target.alive
        {
            let before = super::aimpoint::rays_cast();
            let crouched = target.keys & key::CROUCH != 0;
            if !super::aimpoint::shows(eye, target.feet, crouched, &self.world.sight, || true) {
                self.hidden += 1;
            }
            let reach = (Self::chest(target) - eye).length() - 0.5;
            if self.world.sight.ray_cast(eye, dir, reach).is_some() {
                self.walled += 1;
            }
            self.metric_rays += super::aimpoint::rays_cast() - before;
        }
        let mut best: Option<(Place, f32)> = None;
        for (target, other) in self.bodies.iter().enumerate() {
            if !other.alive || other.team == body.team {
                continue;
            }
            let to = Self::chest(other) - eye;
            let along = to.dot(dir);
            if along <= 0.0 || along > 100.0 {
                continue;
            }
            let miss = (to - dir * along).length();
            if miss < 0.45
                && best.is_none_or(|(_, d)| along < d)
                && self.world.sight.visible(eye, Self::chest(other))
            {
                best = Some((target, along));
            }
        }
        let Some((target, _)) = best else {
            return;
        };
        self.hits += 1;
        let other = &mut self.bodies[target];
        other.health -= 0.2;
        if other.health <= 1e-3 {
            other.alive = false;
            other.dead_at = tick;
            if let Some(battery) = other.carrying.take() {
                self.batteries[battery] = Lying::Free(other.feet, 1 << other.team);
                self.carrier_kills.push((shooter, target));
                if let Some(Some((_, fell @ None))) = self.stolen_marks.get_mut(battery) {
                    *fell = Some((tick, false));
                    self.stolen_log.push(format!(
                        "{tick} battery {battery} fell at its carrier's death"
                    ));
                }
            }
            self.stats[target].1 += 1;
            self.stats[target].2 = 0;
            if let Some(tickets) = self.tickets.as_mut() {
                let team = usize::from(other.team);
                tickets[team] = tickets[team].saturating_sub(1);
            }
            self.stats[shooter].0 += 1;
            self.stats[shooter].2 += 1;
        }
    }

    /// Whether the view's ray from `body`'s eye (its base pitch) comes
    /// within `radius` of `point` no farther than the use's 2 m.
    fn uses(body: &Body, point: Vec3, radius: f32) -> bool {
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let (c, s) = (body.pitch.cos(), body.pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        let to = point - eye;
        let along = to.x * dir.x + to.y * dir.y + to.z * dir.z;
        if !(0.0..=USE_REACH_M).contains(&along) {
            return false;
        }
        (to - dir * along).length() <= radius
    }

    /// [`Self::uses`] a lying battery, the ray clear of the bullet mesh up
    /// to it when [`Game::use_line`].
    fn uses_battery(&self, body: &Body, at: Vec3) -> bool {
        let point = at + BATTERY_MIDDLE;
        if !Self::uses(body, point, BATTERY_M) {
            return false;
        }
        if !self.use_line {
            return true;
        }
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let (c, s) = (body.pitch.cos(), body.pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        let to = point - eye;
        let along = to.x * dir.x + to.y * dir.y + to.z * dir.z;
        along <= 0.15 || self.world.sight.ray_cast(eye, dir, along - 0.15).is_none()
    }

    fn use_and_drop(&mut self, pressed: &[u32]) {
        for place in 0..self.bodies.len() {
            let body = self.bodies[place].clone();
            if !body.alive {
                continue;
            }
            if pressed[place] & key::DROP != 0
                && let Some(battery) = body.carrying
            {
                self.batteries[battery] = Lying::Free(body.feet, 1 << body.team);
                self.bodies[place].carrying = None;
                self.stolen_fell(battery, self.tick, true);
            }
            if pressed[place] & key::USE == 0 {
                continue;
            }
            let team = body.team;
            match body.carrying {
                Some(battery) => {
                    let home = self.containers.iter().position(|(t, _)| *t == team);
                    if let Some(id) = home
                        && Self::uses(&body, self.containers[id].1 + USE_BOX_UP, USE_BOX_M)
                    {
                        self.batteries[battery] = Lying::Home(id as u8);
                        self.bodies[place].carrying = None;
                        self.stolen_home(battery, team);
                        self.points[usize::from(team)] += 1;
                        self.delivered[usize::from(team)] += 1;
                    }
                }
                None => {
                    let free = self.batteries.iter().position(
                        |b| matches!(b, Lying::Free(at, _) if self.uses_battery(&body, *at)),
                    );
                    let theirs = self.containers.iter().position(|(t, at)| {
                        *t != team && Self::uses(&body, *at + USE_BOX_UP, USE_BOX_M)
                    });
                    if let Some(battery) = free {
                        self.batteries[battery] = Lying::Carried(place);
                        self.bodies[place].carrying = Some(battery);
                        self.taken += 1;
                    } else if let Some(id) = theirs
                        && let Some(battery) = self
                            .batteries
                            .iter()
                            .position(|b| *b == Lying::Home(id as u8))
                    {
                        self.batteries[battery] = Lying::Carried(place);
                        self.bodies[place].carrying = Some(battery);
                        self.points[usize::from(1 - team)] -= 1;
                        self.stolen += 1;
                        self.thieves.push(place);
                        if self.stolen_marks.len() <= battery {
                            self.stolen_marks.resize(battery + 1, None);
                        }
                        self.stolen_marks[battery] = Some((1 - team, None));
                    }
                }
            }
        }
    }

    /// A stolen battery fell (its first fall since the theft counts).
    fn stolen_fell(&mut self, battery: usize, tick: u32, alive: bool) {
        if let Some(Some((_, fell @ None))) = self.stolen_marks.get_mut(battery) {
            *fell = Some((tick, alive));
            self.stolen_log.push(format!(
                "{tick} battery {battery} fell {}",
                if alive { "alive" } else { "at death" }
            ));
        }
    }

    /// A battery was put into team `team`'s container.
    fn stolen_home(&mut self, battery: usize, team: u8) {
        if let Some(mark) = self.stolen_marks.get_mut(battery).and_then(Option::take)
            && let (owner, Some((fell, alive))) = mark
        {
            self.stolen_drops
                .push((owner == team, self.tick.saturating_sub(fell), alive));
            self.stolen_log.push(format!(
                "{} battery {battery} home with {}, {} ms after it fell",
                self.tick,
                if owner == team {
                    "its owners"
                } else {
                    "the thieves"
                },
                self.tick.saturating_sub(fell)
            ));
        }
    }

    fn record(&mut self) {
        let fixes = self
            .bodies
            .iter()
            .map(|body| Fix {
                time: self.tick,
                alive: body.alive,
                inserted: body.spawned,
                feet: body.alive.then_some(body.feet),
                yaw: body.yaw,
                pitch: body.pitch,
                keys: body.keys,
                speed: Some({
                    let [a, l] = body.blend.now(self.kin.blend_ticks);
                    a.hypot(l)
                }),
            })
            .collect();
        let health = self.bodies.iter().map(|b| b.health).collect();
        let facts = WorldFacts {
            time: self.tick,
            stats: self.stats.clone(),
            points: Some(self.tickets.unwrap_or(self.points)),
            can_respawn: Some(self.tickets.map_or([true, true], |t| [t[0] > 0, t[1] > 0])),
            next_battery: self.timed.map(|(next, _)| next),
            batteries: self
                .batteries
                .iter()
                .map(|b| match b {
                    Lying::Free(at, _) => BatteryState::Free(*at),
                    Lying::Carried(_) => BatteryState::Carried(None),
                    Lying::Home(id) => BatteryState::Home(*id),
                })
                .collect(),
            marks: self
                .batteries
                .iter()
                .map(|b| match b {
                    Lying::Free(_, mask) => *mask,
                    Lying::Carried(_) | Lying::Home(_) => 0,
                })
                .collect(),
            container_counts: (0..self.containers.len())
                .map(|id| {
                    self.batteries
                        .iter()
                        .filter(|b| **b == Lying::Home(id as u8))
                        .count()
                })
                .collect(),
        };
        self.history.push_back((self.tick, fixes, health, facts));
        while self.history.len() > 40 {
            self.history.pop_front();
        }
    }

    fn world_at(&self, tick: u32) -> Option<Recorded> {
        self.history.iter().find(|(t, ..)| *t == tick).cloned()
    }
}

/// Every input the bots made and every note they wrote, as one number:
/// two runs that decide the same give the same digest. The proof that a
/// change for speed or memory left the bots as they were
/// (`docs/notes/player_bots_mapdata.md` §6).
#[derive(Clone, Copy, Debug, Default)]
struct Decisions {
    digest: mapdata::digest::Digest,
    inputs: u64,
    notes: u64,
}

impl Decisions {
    fn made(&mut self, made: &Made) {
        self.text(&format!("{} {} {:?}", made.place, made.time, made.input));
        self.inputs += 1;
    }

    fn note(&mut self, note: &Note) {
        self.text(&format!("{} {} {}", note.place, note.time, note.text));
        self.notes += 1;
    }

    /// With `SRP_TRACE_DECISIONS=<file>`, each line is also appended
    /// there: two runs' files show where they part.
    fn text(&mut self, text: &str) {
        if let Some(path) = std::env::var_os("SRP_TRACE_DECISIONS") {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            writeln!(file, "{text}").unwrap();
        }
        self.digest.len(text.len());
        for chunk in text.as_bytes().chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.digest.u64(u64::from_le_bytes(word));
        }
    }

    fn line(&self) -> String {
        format!(
            "decisions {:016x} ({} inputs, {} notes)",
            self.digest.finish(),
            self.inputs,
            self.notes
        )
    }
}

/// How a soak run went.
#[derive(Debug, Default)]
struct Outcome {
    decisions: Decisions,
    winner: Option<u8>,
    end_s: f32,
    kills: [u32; 2],
    delivered: [u32; 2],
    stolen: u32,
    shots: u32,
    hits: u32,
    walled: u32,
    hidden: u32,
    /// Sight and aim rays the bots cast (`aimpoint`), per bot and second
    /// of play.
    rays_per_bot_s: f32,
    least_moved_m: f32,
    stuck_notes: usize,
    #[allow(dead_code)]
    notes: Vec<Note>,
}

/// A match of `per_team` bots a side to `win` batteries (Team Deathmatch
/// when `win` is 0: the most kills when time is up), for at most
/// `limit_s`.
fn soak(per_team: usize, win: u32, tdm_tickets: u32, limit_s: u32, seed: u64) -> Outcome {
    soak_mode(per_team, win, tdm_tickets, None, limit_s, seed)
}

/// [`soak`], with timed batteries (one every `timed_s`) when given.
fn soak_mode(
    per_team: usize,
    win: u32,
    tdm_tickets: u32,
    timed_s: Option<u32>,
    limit_s: u32,
    seed: u64,
) -> Outcome {
    soak_tuned(
        per_team,
        win,
        tdm_tickets,
        timed_s,
        limit_s,
        seed,
        bench_tuning(),
    )
}

/// [`soak_mode`] with the bots' `tuning`.
fn soak_tuned(
    per_team: usize,
    win: u32,
    tdm_tickets: u32,
    timed_s: Option<u32>,
    limit_s: u32,
    seed: u64,
    tuning: Tuning,
) -> Outcome {
    let world = soak_world();
    let teams: Vec<u8> = (0..2 * per_team).map(|p| u8::from(p >= per_team)).collect();
    let driven = vec![true; teams.len()];
    let roles: Vec<Option<&str>> = vec![None; teams.len()];
    let mode = if timed_s.is_some() {
        survarium::v026g::maps::mode::TIMED_BATTERIES
    } else if win > 0 {
        survarium::v026g::maps::mode::BATTERIES
    } else {
        survarium::v026g::maps::mode::TEAM_DEATHMATCH
    };
    let mut bots = Bots::new(
        0x50AC + seed as u32,
        mode,
        0,
        &teams,
        &driven,
        &roles,
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let tickets = (win == 0).then_some(tdm_tickets);
    let mut game = Game::new(world, &teams, seed, tickets, timed_s.map(|s| s * 1000));
    for place in 0..teams.len() {
        bots.entered(place, true, 0);
    }
    let mut out = Outcome::default();
    let rays_before = super::aimpoint::rays_cast();
    let limit_ms = limit_s * 1000;
    let (mut clock, mut next_world) = (0, 1_000);
    while clock < limit_ms {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    let notes = bots.fixed(place, fix, Some(health));
                    notes.iter().for_each(|note| out.decisions.note(note));
                    out.notes.extend(notes);
                }
                let notes = bots.facts(facts);
                notes.iter().for_each(|note| out.decisions.note(note));
                out.notes.extend(notes);
            }
        }
        let (made, notes) = bots.poll(Clock {
            clock,
            time_left_s: (limit_ms - clock) as f32 / 1000.0,
            total_s: limit_s as f32,
            playing: true,
        });
        notes.iter().for_each(|note| out.decisions.note(note));
        out.notes.extend(notes);
        for made in made {
            out.decisions.made(&made);
            // Every input keeps the rules: on the grid, ahead, in order,
            // no forbidden bit, a turn under π.
            assert!(made.time > clock && made.time <= clock + 100);
            assert_eq!(made.input.actions & key::NEVER, 0);
            assert!(made.input.rotation_is_sane());
            let queue = &mut game.inputs[made.place];
            assert!(queue.back().is_none_or(|last| last.0 < made.time));
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            assert!(turn.0.abs() < std::f32::consts::PI && turn.1.abs() < std::f32::consts::PI);
            let bot = bots.bots[made.place].as_ref();
            let target = bot.and_then(|bot| bot.reflex.target());
            let gun = bot.map_or(Arms::fallback().primary.0, |bot| bot.reflex.held_gun());
            queue.push_back((made.time, made.input.actions, turn, target, gun));
        }
        if win > 0
            && let Some(team) = (0..2u8).find(|&t| game.points[usize::from(t)] >= win)
        {
            out.winner = Some(team);
            break;
        }
        // Team Deathmatch: a team out of tickets with no one alive loses.
        if let Some(tickets) = game.tickets
            && let Some(loser) = (0..2u8).find(|&t| {
                tickets[usize::from(t)] == 0 && !game.bodies.iter().any(|b| b.team == t && b.alive)
            })
        {
            out.winner = Some(1 - loser);
            break;
        }
    }
    out.end_s = clock as f32 / 1000.0;
    for (place, &team) in teams.iter().enumerate() {
        out.kills[usize::from(team)] += game.stats[place].0;
    }
    // Time up: the mode's leader wins (batteries home, else kills).
    if out.winner.is_none() {
        let (a, b) = if win > 0 {
            (game.points[0], game.points[1])
        } else {
            (out.kills[0], out.kills[1])
        };
        out.winner = match a.cmp(&b) {
            std::cmp::Ordering::Greater => Some(0),
            std::cmp::Ordering::Less => Some(1),
            std::cmp::Ordering::Equal => None,
        };
    }
    out.delivered = game.delivered;
    out.stolen = game.stolen;
    out.shots = game.shots;
    out.hits = game.hits;
    out.walled = game.walled;
    out.hidden = game.hidden;
    out.rays_per_bot_s = (super::aimpoint::rays_cast() - rays_before - game.metric_rays) as f32
        / teams.len() as f32
        / out.end_s.max(1.0);
    out.least_moved_m = game
        .bodies
        .iter()
        .map(|b| b.moved)
        .fold(f32::INFINITY, f32::min);
    out.stuck_notes = out
        .notes
        .iter()
        .filter(|n| n.text.contains("stuck"))
        .count();
    if std::env::var_os("SRP_BOT_NOTES").is_some() {
        for note in &out.notes {
            eprintln!(
                "{:>7} ms  bot {:>2}: {}",
                note.time, note.place as i64, note.text
            );
        }
        eprintln!(
            "winner {:?} at {} s; kills {:?}; delivered {:?}; stolen {}; hits {} of {} shots, {} into a wall short of the target, {} at a hidden target; {:.1} sight and aim rays per bot a second; least moved {:.0} m; stuck notes {}",
            out.winner,
            out.end_s,
            out.kills,
            out.delivered,
            out.stolen,
            out.hits,
            out.shots,
            out.walled,
            out.hidden,
            out.rays_per_bot_s,
            out.least_moved_m,
            out.stuck_notes
        );
    }
    out
}

/// A short run every build: two minutes of 4 against 4, fights and no
/// input out of the rules.
#[test]
fn a_short_soak_has_fights_and_keeps_the_input_rules() {
    let out = soak(4, 3, 0, 120, 3);
    assert!(
        out.kills[0] + out.kills[1] >= 2,
        "{:?} ({} hits of {} shots)",
        out.kills,
        out.hits,
        out.shots
    );
    assert!(out.least_moved_m > 20.0, "{}", out.least_moved_m);
    // Never through the world (`aimpoint`): before it, 48 of 416 shots
    // went into a wall short of the target or at one of whom nothing
    // showed; the few left are a tick's turn or step.
    assert!(
        out.walled + out.hidden <= out.shots / 33,
        "{} into a wall, {} at a hidden target, of {} shots",
        out.walled,
        out.hidden,
        out.shots
    );
}

/// The short run with a tiny search budget: nearly every route search goes
/// on over several polls ([`Searching`]), its bot walking the way so far,
/// and the bots still move, fight and keep the input rules.
#[test]
fn a_short_soak_with_its_searches_in_slices_still_plays() {
    let before = super::path::searches();
    let mut tuning = Tuning::default();
    tuning.think.search_floors_per_poll = 300;
    let out = soak_tuned(4, 3, 0, None, 120, 3, tuning);
    let after = super::path::searches();
    assert!(
        after.sliced - before.sliced >= 50,
        "{} of {} searches sliced",
        after.sliced - before.sliced,
        after.searches - before.searches
    );
    assert!(
        out.kills[0] + out.kills[1] >= 2,
        "{:?} ({} hits of {} shots)",
        out.kills,
        out.hits,
        out.shots
    );
    assert!(out.least_moved_m > 20.0, "{}", out.least_moved_m);
    // Its bots are gone, and with them this thread's search arrays.
    assert_eq!(mapdata::search_memory_bytes(), 0);
}

/// Through the start countdown the game holds the players where they
/// stand: the bots rest (their inputs keep the clock going, with no key
/// and no turn) and go only when play begins.
#[test]
fn the_bots_rest_through_the_start_countdown() {
    let world = soak_world();
    let teams: Vec<u8> = (0..4).map(|p| u8::from(p >= 2)).collect();
    let mut bots = Bots::new(
        0x50AC,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true; 4],
        &[None; 4],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world, &teams, 1, None, None);
    (0..teams.len()).for_each(|place| bots.entered(place, true, 0));
    let start: Vec<Vec3> = game.bodies.iter().map(|body| body.feet).collect();
    let play_from = 15_000;
    let (mut clock, mut next_world) = (0, 1_000);
    let mut going = 0;
    while clock < play_from + 10_000 {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let playing = clock >= play_from;
        if clock == play_from {
            for (body, start) in game.bodies.iter().zip(&start) {
                assert!(
                    body.feet.distance_xz(*start) < 0.1,
                    "{:?} from {start:?}",
                    body.feet
                );
            }
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 600.0,
            playing,
        });
        for made in made {
            let moves =
                made.input.actions & (key::FORWARD | key::BACK | key::LEFT | key::RIGHT) != 0;
            if playing {
                going += usize::from(moves);
            } else {
                assert_eq!(made.input.actions, 0, "{made:?}");
                assert!(made.input.rotation.iter().all(|&b| b == 0), "{made:?}");
            }
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            game.inputs[made.place].push_back((
                made.time,
                made.input.actions,
                turn,
                None,
                Arms::fallback().primary.0,
            ));
        }
    }
    assert!(going > 100, "{going} inputs that move after the start");
}

/// The whole match (minutes of play: `cargo test --release -- --ignored`).
#[test]
#[ignore = "a whole match; run in release"]
fn four_bots_a_side_fight_and_bring_batteries_to_a_win() {
    let out = soak(4, 3, 0, 900, 1);
    eprintln!("{}", out.decisions.line());
    let total = out.kills[0] + out.kills[1];
    assert!(
        total >= 4,
        "they fought: {:?} kills, {} hits of {} shots",
        out.kills,
        out.hits,
        out.shots
    );
    assert!(
        out.delivered[0] + out.delivered[1] >= 1,
        "batteries brought: {:?}",
        out.delivered
    );
    assert!(
        out.least_moved_m > 30.0,
        "every bot moved: {}",
        out.least_moved_m
    );
    assert!(out.winner.is_some(), "a winner in {} s: {out:?}", out.end_s);
}

#[test]
#[ignore = "a whole match; run in release"]
fn a_team_deathmatch_of_bots_is_fought_to_the_last_ticket() {
    // 25 tickets a side: the endgame (no respawns, the hunt) comes.
    let out = soak(4, 0, 25, 900, 2);
    eprintln!("{}", out.decisions.line());
    assert!(
        out.winner.is_some(),
        "a winner in {} s: {:?}",
        out.end_s,
        out.kills
    );
    assert!(out.end_s < 899.0, "ended by the tickets: {} s", out.end_s);
    assert!(
        out.kills[0] > 0 && out.kills[1] > 0,
        "{:?} ({} hits, {} shots)",
        out.kills,
        out.hits,
        out.shots
    );
    assert!(out.least_moved_m > 20.0, "{}", out.least_moved_m);
}

/// Timed batteries: they come by the stations over time; bots wait there
/// and bring them home.
#[test]
#[ignore = "a whole match; run in release"]
fn timed_batteries_are_waited_for_at_the_stations_and_brought_home() {
    let out = soak_mode(4, 4, 0, Some(20), 900, 4);
    eprintln!("{}", out.decisions.line());
    assert!(
        out.delivered[0] + out.delivered[1] >= 2,
        "{:?}",
        out.delivered
    );
    assert!(
        out.notes.iter().any(|n| n.text.contains("takes Station")),
        "no station waits"
    );
    assert!(out.winner.is_some());
}

/// With the battery cheat off: the bots look for the batteries at the
/// spawners, find them and bring them home.
#[test]
fn a_short_soak_without_the_battery_cheat_finds_and_brings_batteries() {
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = false;
    let out = soak_tuned(4, 3, 0, None, 150, 3, tuning);
    assert!(
        out.notes
            .iter()
            .any(|n| n.text.contains("takes Collect Spawner")),
        "no spawner looked at"
    );
    assert!(
        out.notes
            .iter()
            .any(|n| n.text.contains("takes Collect Battery")),
        "no battery fetched"
    );
    assert!(
        out.delivered[0] + out.delivered[1] >= 1,
        "batteries brought: {:?}",
        out.delivered
    );
}

/// [`four_bots_a_side_fight_and_bring_batteries_to_a_win`] and
/// [`timed_batteries_are_waited_for_at_the_stations_and_brought_home`]
/// with the battery cheat on (off by default).
#[test]
#[ignore = "whole matches; run in release"]
fn with_the_battery_cheat_whole_matches_are_won_by_batteries() {
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = true;
    let out = soak_tuned(4, 3, 0, None, 900, 1, tuning.clone());
    eprintln!("{}", out.decisions.line());
    assert!(
        out.delivered[0] + out.delivered[1] >= 1,
        "batteries brought: {:?}",
        out.delivered
    );
    assert!(out.winner.is_some(), "a winner in {} s: {out:?}", out.end_s);
    let out = soak_tuned(4, 4, 0, Some(20), 900, 4, tuning);
    eprintln!("{}", out.decisions.line());
    assert!(
        out.delivered[0] + out.delivered[1] >= 2,
        "{:?}",
        out.delivered
    );
    assert!(out.winner.is_some());
}

/// A crouched body right over a battery cannot use it: the view's pitch
/// stops at 1 rad, and no ray of the use passes within its 0.2 m of the
/// battery's middle closer than about 0.3 m (on the floor; the soak map's
/// lie 0.3 m up). From where the take stands off it (`collect`), looking
/// at it, it can. "Could not take it" 14 times in the owner's match of
/// 2026-10-16, twice with the bot 0.1-0.2 m from the battery.
#[test]
fn a_body_right_over_a_battery_cannot_use_it_and_one_a_step_off_can() {
    use super::plays::collect::{BATTERY_AIM_M, TAKE_PITCH_MAX_RAD, TAKE_PITCH_RAD};
    let battery = Vec3::new(25.0, 0.0, 12.0);
    let mut body = Game::new(soak_world(), &[0], 1, None, None).bodies[0].clone();
    body.keys = key::CROUCH;
    let any_look = |body: &mut Body| {
        (0..64).any(|y| {
            (-100..=100).any(|p| {
                body.yaw = y as f32 * std::f32::consts::TAU / 64.0;
                body.pitch = p as f32 / 100.0;
                Game::uses(body, battery + BATTERY_MIDDLE, BATTERY_M)
            })
        })
    };
    for off in [0.0, 0.1, 0.2] {
        body.feet = battery + Vec3::new(off, 0.0, 0.0);
        assert!(!any_look(&mut body), "{off} m off");
    }
    // The take's stand: the look down at TAKE_PITCH_RAD, aimed as the
    // motor aims (`Look::Use`); and as near as a take is tried without a
    // step back (TAKE_PITCH_MAX_RAD), some look still uses it.
    let aim = battery + Vec3::new(0.0, BATTERY_AIM_M, 0.0);
    let below = view::EYE_CROUCHED - BATTERY_AIM_M;
    body.feet = battery + Vec3::new(below / TAKE_PITCH_MAX_RAD.tan() + 0.02, 0.0, 0.0);
    assert!(any_look(&mut body), "at the steepest take");
    body.feet = battery + Vec3::new(below / TAKE_PITCH_RAD.tan(), 0.0, 0.0);
    let eye = Game::eye(&body);
    body.yaw = kin::yaw_of([aim.x - eye.x, aim.z - eye.z]);
    body.pitch = (aim.y - eye.y).atan2(eye.distance_xz(aim));
    assert!(body.pitch.abs() < 1.0);
    assert!(Game::uses(&body, battery + BATTERY_MIDDLE, BATTERY_M));
}

/// What one bot's search proved no way to, its team leaves for
/// `TEAM_NO_WAY_KEEP_MS` from the proof (in the owner's match of
/// 2026-10-16 six bots learnt one battery each on its own, 183 takes); a
/// second proof keeps it longer; one goal near another is the same.
#[test]
fn a_goal_one_bot_found_no_way_to_is_left_by_its_team() {
    let goal = Vec3::new(30.0, 0.0, 10.0);
    let mut team = Vec::new();
    // Proved at 10 s: the bot's own memory until 30 s.
    learn_no_way(&mut team, &[(goal, 10_000 + NO_WAY_KEEP_MS)], 12_000);
    assert_eq!(team, vec![(goal, 10_000 + TEAM_NO_WAY_KEEP_MS)]);
    // Learnt again at the next tick: the same.
    learn_no_way(&mut team, &[(goal, 10_000 + NO_WAY_KEEP_MS)], 12_500);
    assert_eq!(team.len(), 1);
    // Another bot proves it again at 40 s, a metre off.
    let near = goal + Vec3::new(1.0, 0.0, 0.0);
    learn_no_way(&mut team, &[(near, 40_000 + NO_WAY_KEEP_MS)], 41_000);
    assert_eq!(team, vec![(goal, 40_000 + TEAM_NO_WAY_KEEP_MS)]);
    const { assert!(TEAM_NO_WAY_KEEP_MS >= 3 * NO_WAY_KEEP_MS) };
}

/// A bot standing right on a battery steps off it and takes it (it
/// pressed use there four times, failed, and took the battery again).
#[test]
fn a_bot_standing_on_a_battery_steps_off_and_takes_it() {
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = true;
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x7A6E,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    // On battery 1 (25, 0.3, 12), the other battery 25 m off; the enemy
    // is no bot and stands at his spawn.
    let on = Vec3::new(25.0, 0.0, 12.0);
    game.bodies[0].node = world.grid.locate(on, 1.0);
    game.bodies[0].feet = on;
    bots.entered(0, true, 0);
    let (mut clock, mut next_world, mut took) = (0, 1_000, None);
    while clock < 12_000 && took.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        });
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        if game.bodies[0].carrying.is_some() {
            took = Some(clock);
        }
    }
    let took = took.unwrap_or_else(|| {
        panic!(
            "not taken: at {:?}, batteries {:?}",
            game.bodies[0].feet, game.batteries
        )
    });
    // Its first move is at 2 s; a step off and a use or two.
    assert!(took <= 7_000, "taken at {took} ms");
}

/// A wall 3 m high between the bot and battery 1 (25, 0.3, 12), where it
/// stands 1.3 m east of it, and the stand-in's use stopped by the bullet
/// mesh (as the client's, LIKELY: b9 of 0x7db2c16a, 9 uses aimed right at
/// it from where the mesh blocks the line, taken at once from a side with
/// a clear one). The bot takes it from round the wall (before: four uses
/// there, failed, the play taken again, and so on).
#[test]
fn a_bot_takes_a_battery_from_round_a_wall_its_use_cannot_pass() {
    let world = soak_world_with(&super::body::tests::wall_x(25.6, 11.5, 12.5));
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = true;
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x7A6F,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    game.use_line = true;
    let east = Vec3::new(26.3, 0.0, 12.0);
    game.bodies[0].node = world.grid.locate(east, 1.0);
    game.bodies[0].feet = east;
    // From there the use's ray is stopped, however it looks.
    let mut body = game.bodies[0].clone();
    body.keys = key::CROUCH;
    let battery = Vec3::new(25.0, 0.3, 12.0);
    let blocked = (0..64).all(|y| {
        (-100..=100).all(|p| {
            body.yaw = y as f32 * std::f32::consts::TAU / 64.0;
            body.pitch = p as f32 / 100.0;
            !game.uses_battery(&body, battery)
        })
    });
    assert!(blocked);
    bots.entered(0, true, 0);
    let (mut clock, mut next_world, mut took) = (0, 1_000, None);
    let mut notes_seen = Vec::new();
    while clock < 20_000 && took.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let (made, notes) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        });
        notes_seen.extend(notes.into_iter().map(|n| n.text));
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        if game.bodies[0].carrying == Some(1) {
            took = Some(clock);
        }
    }
    let took = took.unwrap_or_else(|| {
        panic!(
            "not taken: at {:?}, batteries {:?}, notes {notes_seen:?}",
            game.bodies[0].feet, game.batteries
        )
    });
    // Its first move is at 2 s; round the wall, a use or two.
    assert!(took <= 10_000, "taken at {took} ms: {notes_seen:?}");
    assert!(
        !notes_seen.iter().any(|n| n.contains("could not take it")),
        "{notes_seen:?}"
    );
}

/// How a lost-battery run went (`lost_battery_run`): when the thief died,
/// when the team got a look spot, when its bot first went to look, when
/// it took the battery (match ms).
#[derive(Debug, Default)]
struct LostBattery {
    died: Option<u32>,
    look: Option<u32>,
    sent: Option<u32>,
    took: Option<u32>,
}

/// Match 0x15aa5b99 in small: an enemy (place 1, no bot) steals battery 0
/// from team 0's container and crouches 6 m off it carrying it; bot 0 of
/// team 0 sees him and kills him; the battery falls where he died, in a
/// "nook": no ray of the battery sight sees it (`battery_sight_m` 0), only
/// a bot within `battery_near_m`. The thief cheat is off, so the team knows
/// where he was only by its own senses. Runs to `limit` ms.
fn lost_battery_run(look_weight: Option<f32>, limit: u32) -> LostBattery {
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = false;
    tuning.cheats.thief_every_ms = 0;
    tuning.sense.battery_sight_m = 0.0;
    if let Some(weight) = look_weight {
        tuning
            .plays
            .entry(super::plays::PlayKind::Collect)
            .or_insert_with(|| super::plays::default_tuning(super::plays::PlayKind::Collect))
            .numbers
            .insert("look_weight".into(), weight);
    }
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x15AA,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 3, None, None);
    // Battery 0 is in team 0's container (id 0, at (12, 0, 30)); the
    // thief stands by it.
    game.batteries[0] = Lying::Home(0);
    game.points[0] = 1;
    let by_container = Vec3::new(13.0, 0.0, 31.5);
    game.bodies[1].node = world.grid.locate(by_container, 1.0);
    game.bodies[1].feet = by_container;
    // The bot a little way off, facing the container.
    let bot_at = Vec3::new(14.0, 0.0, 12.0);
    game.bodies[0].node = world.grid.locate(bot_at, 1.0);
    game.bodies[0].feet = bot_at;
    game.bodies[0].yaw = kin::yaw_of([by_container.x - bot_at.x, by_container.z - bot_at.z]);
    bots.entered(0, true, 0);
    bots.entered(1, true, 0);
    let mut out = LostBattery::default();
    let (mut clock, mut next_world) = (0, 1_000);
    while clock < limit && out.took.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        // At 3 s he takes it out; at 5 s he crouches in the nook 6 m off.
        if clock == 3_000 {
            game.batteries[0] = Lying::Carried(1);
            game.bodies[1].carrying = Some(0);
            game.points[0] = 0;
        }
        if clock == 5_000 {
            let nook = Vec3::new(17.0, 0.0, 34.0);
            game.bodies[1].node = world.grid.locate(nook, 1.0);
            game.bodies[1].feet = nook;
            game.bodies[1].keys = key::CROUCH;
        }
        // He comes back at his spawn and stands there.
        if out.died.is_none() && !game.bodies[1].alive {
            out.died = Some(clock);
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        if out.look.is_none() && !bots.teams[0].knowledge.looks.is_empty() {
            out.look = Some(clock);
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        });
        if out.sent.is_none()
            && bots.bots[0].as_ref().is_some_and(|b| {
                b.assignment.as_ref().is_some_and(|a| {
                    a.kind == super::plays::PlayKind::Collect
                        && matches!(a.target, super::plays::TargetRef::Point(_))
                })
            })
        {
            out.sent = Some(clock);
        }
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        if game.bodies[0].carrying == Some(0) {
            out.took = Some(clock);
        }
    }
    out
}

/// F4 of the analysis of matches 0x7db2c16a/0x15aa5b99: the thief the bot
/// killed dropped our battery in a nook its sight misses; the team looks
/// where it saw him die within a few seconds, and the bot finds and takes
/// the battery. Without the look (its weight 0) the battery is not found
/// in the same time (126 s in the owner's match).
#[test]
fn a_battery_a_killed_thief_dropped_in_a_nook_is_looked_for_and_taken() {
    let run = lost_battery_run(None, 60_000);
    let died = run
        .died
        .unwrap_or_else(|| panic!("the thief lives: {run:?}"));
    let look = run.look.unwrap_or_else(|| panic!("no look: {run:?}"));
    // The world showing the death comes 300 ms behind, a second apart.
    assert!(look >= died && look <= died + 2_500, "{run:?}");
    let sent = run.sent.unwrap_or_else(|| panic!("nobody sent: {run:?}"));
    assert!(sent <= look + 5_000, "{run:?}");
    let took = run.took.unwrap_or_else(|| panic!("not taken: {run:?}"));
    assert!(took <= died + 30_000, "{run:?}");
    let without = lost_battery_run(Some(0.0), took + 10_000);
    eprintln!("with the look {run:?}; without {without:?}");
    assert_eq!(without.died, run.died, "the same fight");
    assert!(without.took.is_none(), "{without:?}");
}

/// How a dropped-theft run went (`theft_drop_run`), match ms: when the
/// thief dropped the battery, when the team got its looks, when its bot
/// first went to look, when the team knew where the battery lies, when
/// the bot took it.
#[derive(Debug, Default)]
struct TheftDrop {
    dropped: Option<u32>,
    look: Option<u32>,
    sent: Option<u32>,
    seen: Option<u32>,
    took: Option<u32>,
}

/// F2 of the analysis of the owner's matches 0x652d/0x1a09 of 2026-10-16
/// in small: an enemy (place 1, no bot) takes battery 0 out of team 0's
/// container at 3 s and drops it at `drop_at` at `drop_ms` (4 s: a world
/// shows him carrying it; 3.5 s: none does, the next shows it lying), then
/// is gone (at his spawn, behind the wall). Bot 0 of team 0 stands 18 m off the container
/// facing away from it. The thief cheat and the battery cheat are off: the
/// team has the HUD's theft and its own sight. `weight` is Collect's
/// `theft_look_weight` (0: no look is offered). Runs to `limit` ms.
fn theft_drop_run(weight: Option<f32>, drop_at: Vec3, drop_ms: u32, limit: u32) -> TheftDrop {
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = false;
    tuning.cheats.thief_every_ms = 0;
    if let Some(weight) = weight {
        tuning
            .plays
            .entry(super::plays::PlayKind::Collect)
            .or_insert_with(|| super::plays::default_tuning(super::plays::PlayKind::Collect))
            .numbers
            .insert("theft_look_weight".into(), weight);
    }
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0xF2,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 3, None, None);
    // Battery 0 in team 0's container (id 0, at (12, 0, 30)); the thief
    // stands by it. Battery 1 lies at its spawner for the first worlds
    // (the bots tell the batteries' frame by it), then is in team 1's
    // container.
    game.batteries.truncate(2);
    game.batteries[0] = Lying::Home(0);
    game.points[0] = 1;
    let by_container = Vec3::new(13.0, 0.0, 31.5);
    game.bodies[1].node = world.grid.locate(by_container, 1.0);
    game.bodies[1].feet = by_container;
    // The bot a little way off, its back to the container.
    let bot_at = Vec3::new(14.0, 0.0, 12.0);
    game.bodies[0].node = world.grid.locate(bot_at, 1.0);
    game.bodies[0].feet = bot_at;
    game.bodies[0].yaw = kin::yaw_of([bot_at.x - by_container.x, bot_at.z - by_container.z]);
    bots.entered(0, true, 0);
    bots.entered(1, true, 0);
    let mut out = TheftDrop::default();
    let (mut clock, mut next_world) = (0, 1_000);
    while clock < limit && out.took.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock == 2_500 {
            game.batteries[1] = Lying::Home(1);
            game.points[1] = 1;
        }
        if clock == 3_000 {
            game.batteries[0] = Lying::Carried(1);
            game.bodies[1].carrying = Some(0);
            game.points[0] = 0;
        }
        if clock == drop_ms {
            game.batteries[0] = Lying::Free(drop_at, 1 << game.bodies[1].team);
            game.bodies[1].carrying = None;
            out.dropped = Some(clock);
        }
        if clock == 4_500 {
            let away = Vec3::new(75.0, 0.0, 5.0);
            game.bodies[1].node = world.grid.locate(away, 1.0);
            game.bodies[1].feet = away;
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        if out.look.is_none() && bots.teams[0].knowledge.looks.iter().any(|l| l.theft) {
            out.look = Some(clock);
        }
        if out.seen.is_none()
            && out.dropped.is_some()
            && bots.teams[0].knowledge.battery(0).is_some()
        {
            out.seen = Some(clock);
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        });
        if out.sent.is_none()
            && bots.bots[0].as_ref().is_some_and(|b| {
                b.assignment.as_ref().is_some_and(|a| {
                    a.kind == super::plays::PlayKind::Collect
                        && matches!(a.target, super::plays::TargetRef::Point(_))
                })
            })
        {
            out.sent = Some(clock);
        }
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        if game.bodies[0].carrying == Some(0) {
            out.took = Some(clock);
        }
    }
    out
}

/// F2 of the analysis of the owner's matches 0x652d/0x1a09 of 2026-10-16:
/// a thief takes our battery out and drops it 2-5 m from our container
/// (on the side away from the bot, or beside it), seen carrying it or
/// not (taken and dropped between two worlds). The team's looks come
/// with the world showing the drop, the bot is sent within 5 s, sees the
/// battery and takes it. Without the looks (their weight 0) it is found
/// later or not at all.
#[test]
fn a_battery_a_thief_dropped_by_our_container_is_looked_for_and_taken() {
    for (drop_at, drop_ms) in [
        (Vec3::new(12.0, 0.0, 33.0), 4_000),
        (Vec3::new(8.0, 0.0, 31.5), 4_000),
        (Vec3::new(15.5, 0.0, 34.0), 4_000),
        (Vec3::new(12.0, 0.0, 33.0), 3_500),
    ] {
        let run = theft_drop_run(None, drop_at, drop_ms, 60_000);
        let dropped = run.dropped.expect("dropped");
        let look = run.look.unwrap_or_else(|| panic!("no look: {run:?}"));
        // The world showing the drop comes 300 ms behind, a second apart.
        assert!(
            look >= dropped && look <= dropped + 2_500,
            "{drop_at:?} {run:?}"
        );
        let sent = run
            .sent
            .unwrap_or_else(|| panic!("nobody sent: {drop_at:?} {run:?}"));
        assert!(sent <= look + 5_000, "{drop_at:?} {run:?}");
        let seen = run
            .seen
            .unwrap_or_else(|| panic!("not seen: {drop_at:?} {run:?}"));
        assert!(seen <= dropped + 15_000, "{drop_at:?} {run:?}");
        let took = run
            .took
            .unwrap_or_else(|| panic!("not taken: {drop_at:?} {run:?}"));
        let without = theft_drop_run(Some(0.0), drop_at, drop_ms, took + 30_000);
        eprintln!("{drop_at:?} at {drop_ms}: with the looks {run:?}; without {without:?}");
        assert!(without.look.is_some(), "{without:?}");
        assert!(without.sent.is_none(), "{without:?}");
        assert!(
            without.seen.is_none_or(|s| s > seen),
            "{drop_at:?}: with {run:?}; without {without:?}"
        );
    }
}

/// Where the time of the bots' polls goes, seconds.
#[derive(Default)]
struct Times {
    polls: u64,
    view: f64,
    commander: f64,
    think: f64,
    ticks: f64,
    /// Each poll's whole time, µs.
    each: Vec<f32>,
    /// The longest of each part in one poll, µs.
    worst: [f32; 4],
}

/// The default tuning, with `SRP_BENCH_SEARCH_FLOORS=<n>` (`max`: no bound)
/// as the floors a poll's searches may expand: the whole matches and the
/// real-map runs at other budgets.
/// The process's resident memory now and its peak since
/// [`reset_peak_memory`], in MB: on Linux only (`/proc/self/status`).
fn memory_mb() -> Option<(f64, f64)> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb = |key: &str| {
        status
            .lines()
            .find(|line| line.starts_with(key))?
            .split_whitespace()
            .nth(1)?
            .parse::<f64>()
            .ok()
    };
    Some((kb("VmRSS:")? / 1024.0, kb("VmHWM:")? / 1024.0))
}

/// Starts the peak of [`memory_mb`] again from the memory now (Linux).
fn reset_peak_memory() {
    let _ = std::fs::write("/proc/self/clear_refs", "5");
}

fn bench_tuning() -> Tuning {
    let mut tuning = Tuning::default();
    // The battery cheat instead of the fair battery knowledge.
    if std::env::var_os("SRP_BENCH_CHEAT").is_some() {
        tuning.cheats.batteries = true;
    }
    if let Some(m) = std::env::var("SRP_BENCH_BATTERY_SIGHT_M")
        .ok()
        .and_then(|m| m.parse().ok())
    {
        tuning.sense.battery_sight_m = m;
    }
    // Collect's and Intercept's numbers: `name=value,...`.
    for (var, kind) in [
        ("SRP_BENCH_COLLECT", super::plays::PlayKind::Collect),
        ("SRP_BENCH_INTERCEPT", super::plays::PlayKind::Intercept),
    ] {
        let Ok(numbers) = std::env::var(var) else {
            continue;
        };
        let play = tuning
            .plays
            .entry(kind)
            .or_insert_with(|| super::plays::default_tuning(kind));
        for pair in numbers.split(',').filter(|p| !p.is_empty()) {
            let (name, value) = pair
                .split_once('=')
                .unwrap_or_else(|| panic!("{var}: name=value"));
            play.numbers
                .insert(name.to_owned(), value.parse().expect("a number"));
        }
    }
    // The reflex's numbers (`CombatTuning`): `name=json,...` (`;` between
    // them when a value has a comma).
    if let Ok(numbers) = std::env::var("SRP_BENCH_COMBAT") {
        let mut combat = serde_json::to_value(&tuning.aim.combat).unwrap();
        let sep = if numbers.contains(';') { ';' } else { ',' };
        for pair in numbers.split(sep).filter(|p| !p.is_empty()) {
            let (name, value) = pair.split_once('=').expect("SRP_BENCH_COMBAT: name=value");
            combat[name] = serde_json::from_str(value).expect("SRP_BENCH_COMBAT: a JSON value");
        }
        tuning.aim.combat = serde_json::from_value(combat).expect("SRP_BENCH_COMBAT: known names");
    }
    if let Ok(floors) = std::env::var("SRP_BENCH_SEARCH_FLOORS") {
        tuning.think.search_floors_per_poll = if floors == "max" {
            u32::MAX
        } else {
            floors
                .parse()
                .expect("SRP_BENCH_SEARCH_FLOORS: a number or max")
        };
    }
    tuning
}

/// [`Bots::poll`] with each part timed; the same steps in the same order.
fn timed_poll(bots: &mut Bots, clock: Clock, times: &mut Times) -> (Vec<Made>, Vec<Note>) {
    use std::time::Instant;
    let started = Instant::now();
    let (mut made, mut notes) = (Vec::new(), Vec::new());
    let now = clock.clock;
    let world = bots.tactics.as_ref().map(|t| t.world.clone());
    bots.tracker.held_sounds(now);
    if let Some(tactics) = bots.tactics.as_mut() {
        tactics.hazards.advance(now);
    }
    let view = bots
        .tracker
        .view(&clock, &bots.tuning.kin, world.as_deref());
    let viewed = Instant::now();
    if bots.tactics.is_some() {
        for team in 0..2u8 {
            if now >= bots.teams[usize::from(team)].next_commander {
                bots.commander(team, &view, &mut notes);
            }
        }
    }
    let commanded = Instant::now();
    let (mut think, mut ticks) = (0.0, 0.0);
    let mut rays = RayBudget(bots.tuning.think.rays_per_poll);
    bots.searching.left = SEARCHES_PER_POLL;
    bots.searching.floors = bots.tuning.think.search_floors_per_poll as usize;
    let a = Instant::now();
    bots.go_on_searching();
    ticks += a.elapsed().as_secs_f64();
    for index in 0..bots.bots.len() {
        if bots.bots[index].is_none() {
            continue;
        }
        let a = Instant::now();
        bots.think(index, &view, &mut rays, &mut notes);
        let b = Instant::now();
        bots.ticks(index, &clock, &view, &mut made);
        think += (b - a).as_secs_f64();
        ticks += b.elapsed().as_secs_f64();
    }
    let parts = [
        (viewed - started).as_secs_f64(),
        (commanded - viewed).as_secs_f64(),
        think,
        ticks,
    ];
    times.polls += 1;
    times.view += parts[0];
    times.commander += parts[1];
    times.think += parts[2];
    times.ticks += parts[3];
    for (worst, part) in times.worst.iter_mut().zip(parts) {
        *worst = worst.max(part as f32 * 1e6);
    }
    times.each.push(started.elapsed().as_secs_f32() * 1e6);
    (made, notes)
}

/// The cost of the bots on the real maps, only with
/// `SRP_MAPDATA_DIR=<mapdata-dump files>` (and `SRP_BOT_ROUTES` for the
/// human routes): the stand-in game on a real map, polled every 5 ms as
/// the match server's loop does with nobody sending, each part of the poll
/// timed (the stand-in's own time not counted).
///
/// ```text
/// SRP_MAPDATA_DIR=<dir> SRP_BOT_ROUTES=<json> cargo test -p match-server --release real_bot_cost -- --ignored --nocapture
/// ```
#[test]
#[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
fn real_bot_cost() {
    use std::time::Instant;
    let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    let table = std::env::var_os("SRP_BOT_ROUTES")
        .map(|path| super::routes::RouteTable::load(std::path::Path::new(&path)).unwrap());
    let limit_s: u32 = std::env::var("SRP_BENCH_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(240);
    let sizes: Vec<usize> = std::env::var("SRP_BENCH_SIDES")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.parse().ok()).collect())
        .unwrap_or(vec![4, 8, 10]);
    // Another match of the same map and size (0: the digests' runs).
    let seed: u32 = std::env::var("SRP_BENCH_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    use survarium::v026g::maps::mode;
    let maps: Vec<(u8, u8)> = std::env::var("SRP_BENCH_MAPS")
        .ok()
        .map(|s| {
            s.split(',')
                .filter_map(|m| {
                    let (level, mode) = m.split_once('/')?;
                    Some((level.parse().ok()?, mode.parse().ok()?))
                })
                .collect()
        })
        .unwrap_or(vec![
            (6, mode::BATTERIES),
            (1, mode::TEAM_DEATHMATCH),
            (3, mode::TIMED_BATTERIES),
        ]);
    for (level, mode) in maps {
        let project = format!("level_{level:02}/day_{mode}");
        let started = Instant::now();
        let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name(&project))).unwrap();
        let world = Arc::new(World::build(map, mapdata::CELL, mapdata::Body::default()));
        let built_s = started.elapsed().as_secs_f32();
        // What the build freed handed back (glibc keeps it otherwise): the
        // memory of the world as the server holds it.
        mapdata::free_search_memory();
        let world_mb = memory_mb().map(|(now, _)| now);
        for &per_team in &sizes {
            let teams: Vec<u8> = (0..2 * per_team).map(|p| u8::from(p >= per_team)).collect();
            let driven = vec![true; teams.len()];
            let roles: Vec<Option<&str>> = vec![None; teams.len()];
            let mut bots = Bots::new(
                0xBE7C + per_team as u32 + seed * 1000,
                mode,
                level,
                &teams,
                &driven,
                &roles,
                Arc::new(bench_tuning()),
                Trace::none(0),
            );
            if let Some(table) = &table {
                bots.set_routes(table);
            }
            // Every place's weapon known as a live match knows it (the
            // weapon 1 of its role): play 11's matchup against the enemy's.
            if std::env::var_os("SRP_BENCH_WEAPONS").is_some() {
                let weapons = bots
                    .bots
                    .iter()
                    .map(|bot| {
                        let bot = bot.as_ref()?;
                        super::reflex::WeaponKind::of_role(&bot.personality(&bots.tuning).name)
                    })
                    .collect();
                bots.set_weapons(weapons);
            }
            let started = Instant::now();
            let summary = bots.map_ready(world.clone());
            let ready_ms = started.elapsed().as_secs_f32() * 1e3;
            let ready_mb = memory_mb().map(|(now, _)| now);
            reset_peak_memory();
            let (tickets, timed) = match mode {
                mode::TEAM_DEATHMATCH => (Some(60), None),
                mode::TIMED_BATTERIES => (None, Some(20_000)),
                _ => (None, None),
            };
            let mut game = Game::new(
                world.clone(),
                &teams,
                per_team as u64 + u64::from(seed) * 1000,
                tickets,
                timed,
            );
            game.use_line = std::env::var_os("SRP_BENCH_USE_LINE").is_some();
            // As the game does (`gather_victory_items_rule::select_spawners`):
            // this many batteries at spawners drawn at random (the mode's
            // count is 10), not one at every spawner.
            if let Some(count) = std::env::var("SRP_BENCH_BATTERIES")
                .ok()
                .and_then(|n| n.parse::<usize>().ok())
                && timed.is_none()
                && mode != mode::TEAM_DEATHMATCH
            {
                let mut items: Vec<Vec3> =
                    world.map.battery_spawners.iter().map(|s| s.item).collect();
                let mut draw = Rng::new(0x5EED + per_team as u64 + u64::from(seed) * 1000);
                for i in (1..items.len()).rev() {
                    let j = draw.below(i + 1);
                    items.swap(i, j);
                }
                items.truncate(count);
                game.batteries = items.into_iter().map(|at| Lying::Free(at, 0)).collect();
            }
            // One side behind from the start (F0): `SRP_BENCH_START=a,b`
            // puts `a` of the lying batteries in team 0's container and
            // `b` in team 1's, with the points, the rest left lying.
            let start: [u32; 2] = std::env::var("SRP_BENCH_START")
                .ok()
                .and_then(|s| {
                    let (a, b) = s.split_once(',')?;
                    Some([a.trim().parse().ok()?, b.trim().parse().ok()?])
                })
                .filter(|_| mode != mode::TEAM_DEATHMATCH)
                .unwrap_or([0, 0]);
            let mut lying = 0..game.batteries.len();
            for team in 0..2u8 {
                let Some(id) = game.containers.iter().position(|(t, _)| *t == team) else {
                    continue;
                };
                for _ in 0..start[usize::from(team)] {
                    let Some(battery) = lying.next() else {
                        break;
                    };
                    game.batteries[battery] = Lying::Home(id as u8);
                    game.points[usize::from(team)] += 1;
                }
            }
            let start = game.points;
            for place in 0..teams.len() {
                bots.entered(place, true, 0);
            }
            let mut times = Times::default();
            let mut decisions = Decisions::default();
            let mut plays: std::collections::BTreeMap<String, u32> = Default::default();
            // Takes: plays ended "could not take it", and stands given up.
            let (mut take_ends, mut stands_given_up) = (0u32, 0u32);
            let mut theft_looks = 0u32;
            let mut stolen_known: Vec<(usize, u32)> = Vec::new();
            // World-seconds a stolen battery that fell lay unknown to its
            // owners (fell alive, at death).
            let mut lying_unknown = [0u32; 2];
            super::path::reset_worst();
            let searches_before = super::path::searches();
            let rays_before = super::aimpoint::rays_cast();
            let limit_ms = limit_s * 1000;
            let (mut clock, mut next_world) = (0, 1_000);
            // Living bots' ticks with a goal to walk to, and those of them
            // with no route: a bot standing where it means to walk.
            let (mut goal_ticks, mut stood_ticks) = (0u64, 0u64);
            let mut stood_by: std::collections::BTreeMap<String, u64> = Default::default();
            // By the bot's play: takes (and those scoring under 0.05),
            // kills, deaths, steals (the objective plays' yield, F2).
            let mut by_play: std::collections::BTreeMap<String, [u32; 5]> = Default::default();
            // By team, the attacks on the enemy's container (F0): takes,
            // living bot ticks on one, those with a teammate on one too,
            // kills and deaths on one; and all steals; attacks replaced
            // within 10 s of their take (the churn PR 123 cut).
            let mut by_team = [[0u64; 7]; 2];
            let mut last_take: Vec<Option<(bool, u32)>> = vec![None; teams.len()];
            // Living bots' ticks looking at a spawner's spot (Collect at a
            // `TargetRef::Spawner`), and their deaths while at it.
            let (mut search_ticks, mut search_deaths) = (0u64, 0u64);
            let mut was_alive: Vec<bool> = game.bodies.iter().map(|b| b.alive).collect();
            // Carriers killed with a battery, by the shooter's play and by
            // the carrier's team; and every play taken (F3: churn).
            let mut carrier_kills: std::collections::BTreeMap<String, u32> = Default::default();
            let mut carriers_killed = [0u32; 2];
            let mut takes_all = 0u32;
            while clock < limit_ms {
                clock += 5;
                while game.tick + TICK_MS <= clock {
                    let stats_before = game.stats.clone();
                    game.step();
                    let play_of = |place: Place| {
                        bots.bots[place]
                            .as_ref()
                            .and_then(|b| b.assignment.as_ref())
                            .map_or("none".to_owned(), |a| format!("{:?}", a.kind))
                    };
                    let attacking = |place: Place| {
                        bots.bots[place]
                            .as_ref()
                            .and_then(|b| b.assignment.as_ref())
                            .is_some_and(|a| super::team::attack(a.kind))
                    };
                    for (place, (now, before)) in game.stats.iter().zip(&stats_before).enumerate() {
                        let side = &mut by_team[usize::from(teams[place])];
                        if now.0 > before.0 {
                            by_play.entry(play_of(place)).or_default()[2] += now.0 - before.0;
                            if attacking(place) {
                                side[3] += u64::from(now.0 - before.0);
                            }
                        }
                        if now.1 > before.1 {
                            by_play.entry(play_of(place)).or_default()[3] += now.1 - before.1;
                            if attacking(place) {
                                side[4] += u64::from(now.1 - before.1);
                            }
                        }
                    }
                    for place in std::mem::take(&mut game.thieves) {
                        by_play.entry(play_of(place)).or_default()[4] += 1;
                        by_team[usize::from(teams[place])][5] += 1;
                    }
                    let mut on_attack = [0u64; 2];
                    for bot in bots.bots.iter().flatten() {
                        if game.bodies[bot.place].alive && attacking(bot.place) {
                            on_attack[usize::from(bot.team)] += 1;
                        }
                    }
                    for (side, n) in by_team.iter_mut().zip(on_attack) {
                        side[1] += n;
                        if n >= 2 {
                            side[2] += n;
                        }
                    }
                    for (shooter, carrier) in std::mem::take(&mut game.carrier_kills) {
                        *carrier_kills.entry(play_of(shooter)).or_default() += 1;
                        carriers_killed[usize::from(game.bodies[carrier].team)] += 1;
                    }
                    for bot in bots.bots.iter().flatten() {
                        let body = &game.bodies[bot.place];
                        let searching = bot.assignment.as_ref().is_some_and(|a| {
                            a.kind == super::plays::PlayKind::Collect
                                && matches!(a.target, super::plays::TargetRef::Spawner(_))
                        });
                        if searching && body.alive {
                            search_ticks += 1;
                        }
                        if searching && was_alive[bot.place] && !body.alive {
                            search_deaths += 1;
                        }
                        was_alive[bot.place] = body.alive;
                        if let Go::To { at, within_m } = bot.intent.go
                            && body.alive
                            && body.feet.distance_xz(at) > within_m
                        {
                            goal_ticks += 1;
                            // A step walked straight has no route.
                            if bot.route.is_none() && !bot.intent.straight {
                                stood_ticks += 1;
                                let play = bot
                                    .assignment
                                    .as_ref()
                                    .map_or("none".into(), |a| format!("{:?}", a.kind));
                                *stood_by.entry(play).or_default() += 1;
                            }
                        }
                    }
                }
                if clock >= next_world {
                    next_world += 1_000;
                    if let Some((_, fixes, health, facts)) =
                        game.world_at(clock / TICK_MS * TICK_MS - 300)
                    {
                        for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                            for note in bots.fixed(place, fix, Some(health)) {
                                decisions.note(&note);
                            }
                        }
                        for note in bots.facts(facts) {
                            decisions.note(&note);
                            if note.text.contains("its thief no longer carrying it") {
                                theft_looks += 1;
                                game.stolen_log.push(format!("{} {}", note.time, note.text));
                            }
                        }
                        // When a stolen battery that fell is known to its
                        // owners again (seen, or carried by one of them).
                        for (battery, mark) in game.stolen_marks.iter().enumerate() {
                            if let Some((owner, Some((_, alive)))) = *mark
                                && matches!(game.batteries.get(battery), Some(Lying::Free(..)))
                                && bots.teams[usize::from(owner)]
                                    .knowledge
                                    .battery(battery)
                                    .is_none()
                            {
                                lying_unknown[usize::from(!alive)] += 1;
                            }
                            if let Some((owner, Some((fell, _)))) = *mark
                                && !stolen_known.contains(&(battery, fell))
                                && (bots.teams[usize::from(owner)]
                                    .knowledge
                                    .battery(battery)
                                    .is_some()
                                    || game.batteries.get(battery).is_some_and(|b| {
                                        matches!(b, Lying::Carried(p)
                                            if game.bodies[*p].team == owner)
                                    }))
                            {
                                stolen_known.push((battery, fell));
                                game.stolen_log.push(format!(
                                    "{clock} battery {battery} known to its owners, {} ms after it fell",
                                    clock.saturating_sub(fell)
                                ));
                            }
                        }
                    }
                }
                let clock_now = Clock {
                    clock,
                    time_left_s: (limit_ms - clock) as f32 / 1000.0,
                    total_s: limit_s as f32,
                    playing: true,
                };
                let (made, notes) = timed_poll(&mut bots, clock_now, &mut times);
                for note in notes {
                    decisions.note(&note);
                    if let Some(rest) = note.text.strip_prefix("takes ") {
                        takes_all += 1;
                        let kind = rest.split(' ').next().unwrap_or("").to_owned();
                        let score: f32 = rest
                            .split("(score ")
                            .nth(1)
                            .and_then(|s| s.split(':').next())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1.0);
                        let is_attack = matches!(kind.as_str(), "Raid" | "Sneak" | "LateDenial");
                        let side = &mut by_team[usize::from(teams[note.place])];
                        if is_attack {
                            side[0] += 1;
                        }
                        if let Some((true, at)) = last_take[note.place]
                            && note.time < at + 10_000
                        {
                            side[6] += 1;
                        }
                        last_take[note.place] = Some((is_attack, note.time));
                        let row = by_play.entry(kind).or_default();
                        row[0] += 1;
                        row[1] += u32::from(score < 0.05);
                    }
                    if note.text.contains("its thief no longer carrying it") {
                        theft_looks += 1;
                    }
                    if note.text.starts_with("ends Collect: could not take it") {
                        take_ends += 1;
                    } else if note.text.starts_with("took no battery") {
                        stands_given_up += 1;
                    }
                    let key: String = note
                        .text
                        .split(|c: char| c == '(' || c == ':' || c.is_ascii_digit())
                        .next()
                        .unwrap_or("")
                        .to_owned();
                    *plays.entry(key).or_default() += 1;
                }
                for made in made {
                    decisions.made(&made);
                    let r = made.input.rotation;
                    let turn = (
                        half_value(u16::from_le_bytes([r[0], r[1]])),
                        half_value(u16::from_le_bytes([r[2], r[3]])),
                    );
                    let bot = bots.bots[made.place].as_ref();
                    let target = bot.and_then(|bot| bot.reflex.target());
                    let gun = bot.map_or(Arms::fallback().primary.0, |bot| bot.reflex.held_gun());
                    game.inputs[made.place].push_back((
                        made.time,
                        made.input.actions,
                        turn,
                        target,
                        gun,
                    ));
                }
            }
            let rays = super::aimpoint::rays_cast() - rays_before - game.metric_rays;
            let searches = super::path::searches();
            let (searched, failed, search_us) = (
                searches.searches - searches_before.searches,
                searches.failed - searches_before.failed,
                searches.us - searches_before.us,
            );
            let stages: Vec<(u64, f64)> = (0..4)
                .map(|k| {
                    let n = searches.stages[k] - searches_before.stages[k];
                    let us = searches.stage_us[k] - searches_before.stage_us[k];
                    (n, us / 1e3 / n.max(1) as f64)
                })
                .collect();
            eprintln!(
                "  by stage (no way, corridor, widened, whole): (searches, ms each) {stages:?}; worst search {:.1} ms",
                searches.worst_us / 1e3
            );
            let by_expanded: Vec<u64> = (0..8)
                .map(|k| searches.by_expanded[k] - searches_before.by_expanded[k])
                .collect();
            eprintln!(
                "  floors expanded: {} in all, worst search {}; searches by floors expanded (up to {:?}, more): {by_expanded:?}; sliced {} in {} slices",
                searches.expanded - searches_before.expanded,
                searches.worst_expanded,
                super::path::EXPANDED_BUCKETS,
                searches.sliced - searches_before.sliced,
                searches.slices - searches_before.slices,
            );
            eprintln!(
                "  pulled straight: {} paths of {} floors, {} straight-line checks, {:.1} ms",
                searches.pulls - searches_before.pulls,
                searches.pull_floors - searches_before.pull_floors,
                searches.straight_checks - searches_before.straight_checks,
                (searches.pull_us - searches_before.pull_us) / 1e3,
            );
            eprintln!(
                "  grid searches {searched} ({:.1} a bot a second), {failed} failed; {:.2} ms each on average; {:.0} % of the bots' time",
                searched as f64 / teams.len() as f64 / limit_s as f64,
                search_us / 1e3 / searched.max(1) as f64,
                search_us / 1e4 / (times.view + times.commander + times.think + times.ticks)
            );
            if std::env::var_os("SRP_BENCH_WHY").is_some() {
                let tactics = bots.tactics.as_ref().unwrap();
                let grid = &tactics.world.grid;
                for bot in bots.bots.iter().flatten() {
                    let me = game.bodies[bot.place].feet;
                    let goal = match bot.intent.go {
                        Go::To { at, .. } => Some(at),
                        _ => None,
                    };
                    let from = grid.locate(me, 1.5);
                    let to = goal.and_then(|g| grid.locate(g, 3.0));
                    let open = from.zip(to).map(|(a, b)| path::plan(grid, a, b).is_some());
                    let allowed = from.zip(to).map(|(a, b)| {
                        path::plan_where(grid, a, b, |n| tactics.allowed(n, clock, bot.side, false))
                            .is_some()
                    });
                    let objective = from.zip(to).map(|(a, b)| {
                        path::plan_where(grid, a, b, |n| tactics.allowed(n, clock, bot.side, true))
                            .is_some()
                    });
                    eprintln!(
                        "  bot {} team {} side {:?} at {me:?} goal {goal:?} objective {} from {from:?} to {to:?}: path open {open:?}, allowed {allowed:?}, objective-allowed {objective:?}; play {:?}",
                        bot.place,
                        bot.team,
                        bot.side,
                        bot.intent.objective,
                        bot.assignment.as_ref().map(|a| (a.kind, a.target)),
                    );
                }
            }
            let mut top: Vec<_> = plays.into_iter().collect();
            top.sort_by_key(|entry| std::cmp::Reverse(entry.1));
            eprintln!("  notes: {:?}", &top[..top.len().min(24)]);
            eprintln!(
                "  stood with a goal and no route: {stood_ticks} of {goal_ticks} bot ticks ({:.1} %); delivered {:?}; points {:?}, stolen {}",
                100.0 * stood_ticks as f64 / goal_ticks.max(1) as f64,
                game.delivered,
                game.points,
                game.stolen
            );
            eprintln!(
                "  takes: {} taken, {take_ends} could-not-take ends, {stands_given_up} stands given up",
                game.taken
            );
            // F2: stolen batteries that fell, alive (dropped) or at the
            // carrier's death: (home again with their owners, ms each),
            // home with the thief's side, still out at the end.
            for alive in [true, false] {
                let home: Vec<u32> = game
                    .stolen_drops
                    .iter()
                    .filter(|d| d.0 && d.2 == alive)
                    .map(|d| d.1)
                    .collect();
                let theirs = game
                    .stolen_drops
                    .iter()
                    .filter(|d| !d.0 && d.2 == alive)
                    .count();
                let out = game
                    .stolen_marks
                    .iter()
                    .flatten()
                    .filter(|m| m.1.is_some_and(|(_, a)| a == alive))
                    .count();
                eprintln!(
                    "  stolen fallen {}: home again ms {home:?}, home with the thieves {theirs}, still out {out}; lying unknown to its owners {} s; theft looks {theft_looks}",
                    if alive { "alive" } else { "at death" },
                    lying_unknown[usize::from(!alive)],
                );
            }
            for line in &game.stolen_log {
                eprintln!("  stolen log: {line}");
            }
            let mut by: Vec<_> = stood_by.into_iter().collect();
            by.sort_by_key(|entry| std::cmp::Reverse(entry.1));
            eprintln!("  stood by play: {:?}", &by[..by.len().min(8)]);
            let rows: Vec<String> = by_play
                .iter()
                .map(|(k, r)| format!("{k} {} {} {} {} {}", r[0], r[1], r[2], r[3], r[4]))
                .collect();
            let tick_s = f64::from(TICK_MS) / 1000.0;
            let sides: Vec<String> = by_team
                .iter()
                .map(|r| {
                    format!(
                        "[{}, {:.0}, {:.0}, {}, {}, {}, {}]",
                        r[0],
                        r[1] as f64 * tick_s,
                        r[2] as f64 * tick_s,
                        r[3],
                        r[4],
                        r[5],
                        r[6]
                    )
                })
                .collect();
            eprintln!(
                "  by team (attack takes, attack bot-s, of them with a teammate attacking, attack kills, attack deaths, steals, attacks replaced within 10 s): {}; started {:?}, points {:?}",
                sides.join(" "),
                start,
                game.points
            );
            eprintln!(
                "  by play (takes, under 0.05, kills, deaths, steals): {}",
                rows.join("; ")
            );
            eprintln!(
                "  carriers: killed carrying {:?} (by team carrying), by the shooter's play {:?}; delivered {:?}; picked up {} lying + {} stolen; plays taken {takes_all} = {:.2} a bot-minute",
                carriers_killed,
                carrier_kills,
                game.delivered,
                game.taken,
                game.stolen,
                f64::from(takes_all) / teams.len() as f64 / (f64::from(limit_s) / 60.0)
            );
            eprintln!(
                "  search: {:.0} bot-s at spawners' spots, {search_deaths} deaths in it",
                search_ticks as f64 * f64::from(TICK_MS) / 1000.0
            );
            let line = format!("{project} {per_team}v{per_team}: {}", decisions.line());
            eprintln!("  {line}");
            if let Some(out) = std::env::var_os("SRP_DECISIONS_OUT") {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(out)
                    .unwrap();
                writeln!(file, "{line}").unwrap();
            }
            if let (Some(world_mb), Some(ready_mb), Some((_, peak))) =
                (world_mb, ready_mb, memory_mb())
            {
                eprintln!(
                    "  memory MB: {world_mb:.0} with the world, {ready_mb:.0} with the bots' \
                     tactics, peak {peak:.0} in the match"
                );
            }
            let total = times.view + times.commander + times.think + times.ticks;
            let mut each = std::mem::take(&mut times.each);
            each.sort_by(f32::total_cmp);
            let at = |q: f64| each[((each.len() - 1) as f64 * q) as usize];
            let kills: u32 = game.stats.iter().map(|s| s.0).sum();
            eprintln!(
                "{project} {per_team}v{per_team}: world built in {built_s:.2} s, map_ready {ready_ms:.1} ms ({summary})\n  \
                 {limit_s} s of play: bots {:.1} ms a second = {:.2} % of a core; per bot {:.3} %\n  \
                 poll µs p50 {:.0}, p99 {:.0}, p99.9 {:.0}, max {:.0}; {} polls\n  \
                 parts: view {:.0} %, commander {:.0} %, think {:.0} %, ticks {:.0} %; worst µs view {:.0}, commander {:.0}, think {:.0}, ticks {:.0}\n  \
                 rays {:.1} per bot a second; kills {kills}, shots {}, hits {}",
                total * 1e3 / limit_s as f64,
                total * 100.0 / limit_s as f64,
                total * 100.0 / limit_s as f64 / teams.len() as f64,
                at(0.5),
                at(0.99),
                at(0.999),
                each[each.len() - 1],
                times.polls,
                100.0 * times.view / total,
                100.0 * times.commander / total,
                100.0 * times.think / total,
                100.0 * times.ticks / total,
                times.worst[0],
                times.worst[1],
                times.worst[2],
                times.worst[3],
                rays as f64 / teams.len() as f64 / limit_s as f64,
                game.shots,
                game.hits,
            );
        }
    }
}

/// A body at `x` on the line z = 0, alive or not, in a world at `time`.
fn standing(time: u32, x: f32, alive: bool) -> Fix {
    Fix {
        time,
        alive,
        inserted: 100,
        feet: alive.then(|| Vec3::new(x, 0.0, 0.0)),
        yaw: 0.0,
        pitch: 0.0,
        keys: 0,
        speed: Some(0.0),
    }
}

/// A bot that left is forgotten by the others.
#[test]
fn a_bot_that_left_is_forgotten() {
    use survarium::v026g::maps::mode;
    // A human of team 0 at x = 0; team 1's bots at 5, 40 and 30 m.
    let teams = [0, 1, 1, 1];
    let driven = [false, true, true, true];
    let mut bots = Bots::new(
        0x6A7E,
        mode::BATTERIES,
        0,
        &teams,
        &driven,
        &[None; 4],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    for place in 0..4 {
        bots.entered(place, true, 100);
    }
    for (place, x) in [0.0, 5.0, 40.0, 30.0].into_iter().enumerate() {
        bots.fixed(place, standing(1_000, x, true), Some(1.0));
    }
    // Bot 3 leaves: it is forgotten, and no partner of anyone.
    bots.bots[1].as_mut().unwrap().partner = Some(3);
    bots.entered(3, false, 6_000);
    assert!(!bots.is_bot(3) && bots.is_bot(1));
    assert_eq!(bots.bots[1].as_ref().unwrap().partner, None);
    assert!(
        !bots
            .tracker()
            .view(
                &Clock {
                    clock: 6_000,
                    time_left_s: 0.0,
                    total_s: 0.0,
                    playing: true,
                },
                &bots.tuning.kin,
                None
            )
            .places[3]
            .present
    );
    let (made, _) = bots.poll(Clock {
        clock: 9_000,
        time_left_s: 600.0,
        total_s: 900.0,
        playing: true,
    });
    assert!(made.iter().all(|made| made.place != 3));
}

/// The world's HUD mask reaches the teams' knowledge (`WorldFacts::marks`
/// through the tracker to `TeamKnowledge::hud_update`): team 1's dropped
/// battery that falls through level_01's floor and is put back 2 m away
/// stays known to team 1 (matches 0x1a09…/0x7298…), never to team 0.
#[test]
fn a_dropped_battery_put_back_after_a_fall_stays_on_its_teams_hud() {
    let teams = [0, 1];
    let mut bots = Bots::new(
        0xF1,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, true],
        &[None; 2],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    assert!(!bots.tuning.cheats.batteries);
    for place in 0..2 {
        bots.entered(place, true, 100);
    }
    let lying = |time: u32, at: Vec3, mask: u32| WorldFacts {
        time,
        batteries: vec![BatteryState::Free(at)],
        marks: vec![mask],
        ..WorldFacts::default()
    };
    let known = |bots: &Bots, team: usize| bots.teams[team].knowledge.battery(0).copied();
    let dropped = Vec3::new(-43.58, 19.55, 3.80);
    let reset = Vec3::new(-41.6, 21.6, 3.5);
    bots.facts(lying(1_000, Vec3::new(-14.0, 21.7, -61.4), 0));
    assert!(known(&bots, 0).is_none() && known(&bots, 1).is_none());
    bots.facts(WorldFacts {
        time: 2_000,
        batteries: vec![BatteryState::Carried(None)],
        marks: vec![0],
        ..WorldFacts::default()
    });
    bots.facts(lying(3_000, dropped, 2));
    assert!(known(&bots, 1).is_some_and(|k| k.hud));
    bots.facts(lying(4_000, Vec3::new(dropped.x, -86.0, dropped.z), 2));
    bots.facts(lying(5_000, reset, 2));
    let kept = known(&bots, 1).expect("still on team 1's HUD");
    assert!(kept.hud && kept.time == 5_000);
    // In the bots' frame (without a map, the mirrored one).
    let reset_frame = Vec3::new(reset.x, reset.y, -reset.z);
    assert!((kept.at - reset_frame).length() < 1e-3, "{kept:?}");
    assert!(known(&bots, 0).is_none());
}

/// A place's ammo lasts the match: a bot's kit keeps what it has left from
/// life to life, and when it runs low the bot picks its second kit (its
/// backup's weapon and another ammo) for its next life, once.
#[test]
fn a_bot_spent_of_its_first_kit_picks_its_second_for_the_next_life() {
    let mut bots = Bots::new(
        0x417,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &[0, 1],
        &[true, true],
        &[Some("flanker"), Some("rusher")],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    let bot = bots.bots[0].as_mut().unwrap();
    let (uzi, ak) = (bot.kits[0].0, bot.kits[1].0);
    assert_eq!(uzi.primary.0.item, "uzi");
    assert_eq!(ak.primary.0.item, "ak_74u");
    assert_eq!((bot.kits[0].1, bot.kits[1].1), (600, 600));
    // Full: no choice. A life that spends all but 40 rounds: still a
    // magazine left, and it keeps the kit.
    assert_eq!(bot.pick_kit(), None);
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (32, 8);
    assert_eq!(bot.pick_kit(), None);
    // The next life in the same kit has those 40, not a full kit.
    bot.new_life(0.5, None, Vec3::ZERO);
    assert_eq!((bot.kit, bot.reflex.left()), (0, 40));
    assert_eq!(bot.reflex.hands().primary.rounds, 32);
    // Down to 10: kit 2 for its next life, chosen once.
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (10, 0);
    assert_eq!(bot.pick_kit(), Some((1, 10, 600)));
    assert_eq!(bot.pick_kit(), None);
    // Its next life is in kit 2, with the AK-74U and its 600 rounds; kit
    // 1 keeps its 10.
    bot.new_life(0.5, None, Vec3::ZERO);
    assert_eq!(bot.kit, 1);
    assert_eq!(bot.reflex.held_gun().item, "ak_74u");
    assert_eq!(bot.reflex.left(), 600);
    assert_eq!(bot.kits[0].1, 10);
    // The rusher's second kit is the Uzi.
    let rusher = bots.bots[1].as_ref().unwrap();
    assert_eq!(rusher.kits[1].0.primary.0.item, "uzi");
}

/// Match 0x5402a730 (2026-10-16): the client's world has a dead body
/// without feet, and `game::driven` hands the bots its bodies before its
/// statistics, so the death the statistics show came with no spot: no
/// kill feed entry, no danger spot, play 21 never a candidate. Now the
/// death is where the body was last reckoned alive, and a team's third
/// bot has a partner too (one way: it trades, it follows no one).
#[test]
fn a_partners_death_the_world_shows_without_feet_is_traded_by_both_teammates() {
    let world = soak_world();
    let teams = [0, 0, 0, 1, 1, 1];
    let mut bots = Bots::new(
        0x5402,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true; 6],
        &[None; 6],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    bots.map_ready(world);
    for place in 0..6 {
        bots.entered(place, true, 0);
    }
    let spots = [
        Vec3::new(10.0, 0.0, 30.0),
        Vec3::new(14.0, 0.0, 30.0),
        Vec3::new(15.0, 0.0, 34.0),
        Vec3::new(70.0, 0.0, 30.0),
        Vec3::new(72.0, 0.0, 34.0),
        Vec3::new(70.0, 0.0, 38.0),
    ];
    let body = |time: u32, place: usize, alive: bool| Fix {
        time,
        alive,
        inserted: 100,
        feet: alive.then_some(spots[place]),
        yaw: 0.0,
        pitch: 0.0,
        keys: 0,
        speed: Some(0.0),
    };
    let clock = |clock: u32| Clock {
        clock,
        time_left_s: 600.0,
        total_s: 900.0,
        playing: true,
    };
    for place in 0..6 {
        bots.fixed(place, body(1_000, place, true), Some(1.0));
    }
    bots.facts(WorldFacts {
        time: 1_000,
        stats: vec![(0, 0, 0); 6],
        ..WorldFacts::default()
    });
    bots.poll(clock(1_100));
    let partner = |bots: &Bots, place: usize| {
        let bot = bots.bots[place].as_ref().unwrap();
        (bot.partner, bot.paired)
    };
    assert_eq!(partner(&bots, 0), (Some(1), true));
    assert_eq!(partner(&bots, 1), (Some(0), true));
    // The third: its nearest living teammate, one way.
    assert_eq!(partner(&bots, 2), (Some(1), false));
    assert_eq!(partner(&bots, 5), (Some(4), false));

    // Bot 1 dies: the world shows it dead, without feet, then the
    // statistics.
    for place in 0..6 {
        bots.fixed(place, body(2_000, place, place != 1), Some(1.0));
    }
    let mut stats = vec![(0, 0, 0); 6];
    stats[1].1 = 1;
    stats[3].0 = 1;
    bots.facts(WorldFacts {
        time: 2_000,
        stats,
        ..WorldFacts::default()
    });
    let view = bots.tracker().view(&clock(2_000), &bots.tuning.kin, None);
    assert_eq!(view.deaths.len(), 1, "{:?}", view.deaths);
    assert_eq!(view.deaths[0].place, 1);
    assert!(
        view.deaths[0].at.distance_xz(spots[1]) < 1.0,
        "{:?}",
        view.deaths
    );
    let danger = &bots.teams[0].knowledge.danger;
    assert_eq!(danger.len(), 1, "{danger:?}");
    assert_eq!(danger[0].victim, 1);
    assert!(danger[0].at.distance_xz(spots[1]) < 1.0, "{danger:?}");
    // Both its teammates trade it: its pair and the third.
    for now in (2_100..=3_000).step_by(100) {
        bots.poll(clock(now));
    }
    // The third keeps its partner while he is down.
    assert_eq!(partner(&bots, 2), (Some(1), false));
    for place in [0, 2] {
        let bot = bots.bots[place].as_ref().unwrap();
        assert_eq!(
            bot.assignment.as_ref().map(|a| a.kind),
            Some(plays::PlayKind::Trade),
            "bot {place}"
        );
    }
}

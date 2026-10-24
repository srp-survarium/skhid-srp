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
//! armour, no stamina, a jump only up to the client's height
//! ([`STAND_IN_AIR_MS`], [`stand_in_jump`]), none with a broken leg, a
//! hit's stagger as one slow ([`STAND_IN_LEG_SHARE`], [`STAND_IN_STAGGER`]);
//! the rounds are not counted, the bots count their own): the runs check
//! that bots of two teams find each other, fight, die, come back, take
//! and bring batteries and end a match, with no input out of the rules;
//! not how well.
//! `SRP_BOT_NOTES=1` prints what each bot did.

use std::collections::VecDeque;
use std::sync::Arc;

use mapdata::{BatterySpawner, Container, NodeId, Vec3, World};
use survarium::v026g::match_msgs::TICK_MS;

use super::body::{Fix, jumped_to, slide};
use super::guns::{Arms, Gun};
use super::kin::{Blend, Kinematics, TICK_S, forward, to_world};
use super::recoil::{Cadence, Recoil, kick_of};
use super::rng::Rng;
use super::trace::Trace;
use super::tuning::Tuning;
use super::view::{BatteryState, Clock, WorldFacts};
use super::*;
use crate::v026g::drive::{half_value, key};

#[path = "fight_tally.rs"]
mod fight_tally;
#[path = "bench_spread.rs"]
mod bench_spread;

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
/// How long a put-in takes, ms: the battery's put state lasts
/// `states[2].put_time`, 0.6 s (`gameplay/victory_items/default.options`,
/// `docs/notes/battery_pickup.md`), and the battery goes in at its end
/// (LIKELY: in the owner's Cologne match of 2026-10-24 every put-in a bot
/// judged failed went in 0.3-3.3 s later). `SRP_BENCH_PUT_MS=<ms>` sets
/// another (0: in at the press, as before).
const PUT_MS: u32 = 600;

/// A carrier's use at his container in the stand-in: when, from where, how
/// far its ray passed the use box's middle, whether it began a put, and
/// whether one was going on already.
#[derive(Clone, Copy, Debug)]
struct PutPress {
    place: usize,
    tick: u32,
    feet: Vec3,
    miss: f32,
    began: bool,
    putting: bool,
}

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
    /// Until when a jump pressed keeps the body in the air, ms
    /// ([`STAND_IN_AIR_MS`]).
    air_until: u32,
    /// The leg pools (0.5 full), until when an empty one is broken, and
    /// the last hit's tick (0: none this life), [`STAND_IN_LEG_SHARE`].
    legs: [f32; 2],
    leg_broken_until: u32,
    last_hit: u32,
    /// The bot's target as its last input was made (the reflex's).
    target: Option<Place>,
    /// Its shots' spread ([`bench_spread`]).
    spread: bench_spread::Spread,
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

/// Every this long one lying battery is moved (`Game::nudge`), ms.
const NUDGE_EVERY_MS: u32 = 20_000;

/// A carrier killed this soon after his take counts as killed taking it
/// (`Game::taken_killed`), ms.
const TAKEN_KILLED_MS: u32 = 3_000;

/// How long a jump keeps a stand-in body in the air, ms: walking into a
/// floor its floor jumps to ([`jumped_to`]) meanwhile, it gets up on it.
/// GUESS: a bot jumps up its route's ledge 0.8 m short of it
/// (`motor.rs` `jump_key`), 0.25 s at a run; the game's jump is up for
/// about half a second. The stand-in had no jumps: a route up a jump link
/// stood at its foot, and 100 of the 118 held-keys stuck notes of a bench
/// were within a metre of a jump floor (`player_bots_stuck.md` §8.6).
const STAND_IN_AIR_MS: u32 = 600;

/// A stand-in body's legs: one hit in this many lands on a leg [GUESS: the
/// hip and foot zones], taking [`STAND_IN_LEG_HIT`] of its pool (0.5 full;
/// one or two rifle hits empty one, LIKELY, sprint-jump RE `FINDINGS.md`
/// §1.8). An empty pool breaks the leg: no jump at all for
/// [`STAND_IN_LEG_BROKEN_MS`] (PROVEN), then it is whole (the game mends
/// it slowly).
const STAND_IN_LEG_SHARE: u32 = 4;
const STAND_IN_LEG_HIT: f32 = 0.25;
const STAND_IN_LEG_BROKEN_MS: u32 = 8_000;
/// After a hit the stand-in walks at this share of its speed for
/// [`STAND_IN_STAGGER_MS`] (the game's stagger: up to half, a rifle hit
/// 15-35 %, PROVEN; the share a GUESS between).
const STAND_IN_STAGGER: f32 = 0.75;
const STAND_IN_STAGGER_MS: u32 = 500;

/// The leg a hit of `shooter` on `target` at `tick` lands on, if any
/// ([`STAND_IN_LEG_SHARE`]): a hash, so the game's draws are not moved.
fn stand_in_leg_hit(tick: u32, shooter: Place, target: Place) -> Option<usize> {
    let from = (shooter as u32).wrapping_mul(0x85EB_CA6B);
    let to = (target as u32).wrapping_mul(0x9E37_79B9);
    let h = (tick ^ from ^ to).wrapping_mul(0xC2B2_AE35);
    let h = h ^ (h >> 16);
    let leg = ((h >> 8) & 1) as usize;
    h.is_multiple_of(STAND_IN_LEG_SHARE).then_some(leg)
}

/// A jump of the stand-in: how high the feet get over the floor jumped
/// from, and how far ahead a jump with no grid link lands, m.
#[derive(Clone, Copy, Debug)]
struct StandInJump {
    rise_m: f32,
    ahead_m: f32,
}

/// The client's jump standing or walking: the `on_site*` clips' root tops
/// at 0.998 m (PROVEN, the jump study). How far it carries was never
/// measured: GUESS 0.6 m ahead, kept short on purpose.
const STANDING_JUMP: StandInJump = StandInJump {
    rise_m: 0.998,
    ahead_m: 0.6,
};

/// The client's sprint jump: 1.282-1.350 m by clip (PROVEN, the jump
/// study), played at a press with forward and sprint held at walk speed or
/// more, any weapon, no battery carried (PROVEN,
/// `player_logic_jump_state::initialize` 0x337040). No bot's jump picks it
/// yet. The distance is [`STANDING_JUMP`]'s (GUESS).
const SPRINT_JUMP: StandInJump = StandInJump {
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
fn stand_in_jump(
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
fn ledge_grid(rise: f32, body: mapdata::Body) -> mapdata::WalkGrid {
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
fn walk_into_ledge(grid: &mapdata::WalkGrid, jump: Option<StandInJump>) -> Vec3 {
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
fn a_stand_in_body_in_the_air_gets_up_a_ledge() {
    let grid = ledge_grid(0.9, mapdata::Body::default());
    let stopped = walk_into_ledge(&grid, None);
    assert!(stopped.x < 10.0 && stopped.y.abs() < 0.1, "{stopped:?}");
    let up = walk_into_ledge(&grid, Some(STANDING_JUMP));
    assert!(up.x > 12.0 && (up.y - 0.9).abs() < 0.1, "{up:?}");
}

/// A grid jump link higher than the client's jump (1.03 m, linked under
/// the grid's 1.05 m) is not made: the body stays at the ledge's foot.
#[test]
fn a_stand_in_jump_does_not_make_a_link_over_the_clients_height() {
    let grid = ledge_grid(1.03, mapdata::Body::default());
    let stayed = walk_into_ledge(&grid, Some(STANDING_JUMP));
    assert!(stayed.x < 10.0 && stayed.y.abs() < 0.1, "{stayed:?}");
}

/// With no grid link (a grid built for a 0.5 m jump), the jump still gets
/// up a 0.9 m ledge; not up a 1.2 m one, which only a sprint jump clears.
#[test]
fn a_stand_in_jump_gets_up_a_ledge_with_no_link_within_its_height() {
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
    /// The bodies' hurts in those worlds ([`Game::hurts_at`]).
    hurts: VecDeque<(u32, Vec<Hurt>)>,
    tick: u32,
    rng: Rng,
    shots: u32,
    hits: u32,
    /// By place: shots, and hits on an enemy.
    shots_hits: Vec<(u32, u32)>,
    /// The places that put a battery in at the last step.
    put_in: Vec<Place>,
    /// The team hits of the last step: shooter, victim, the shot's eye
    /// and direction.
    team_hit_shots: Vec<(Place, Place, Vec3, Vec3, f32, Vec3)>,
    /// Hits on a teammate (the game has friendly fire), and the teammates
    /// killed so.
    team_hits: u32,
    team_kills: u32,
    /// Jumps pressed, and of them those a broken leg refused.
    jumps_pressed: u32,
    jumps_refused: u32,
    /// Shots at the bot's target whose ray met the world short of him, and
    /// those with nothing of him showing.
    walled: u32,
    hidden: u32,
    /// [PR C] The same by the shooter's team.
    walled_by: [u32; 2],
    hidden_by: [u32; 2],
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
    /// Every kill: (tick, the shooter, the killed), for the bench's
    /// events (`SRP_BENCH_EVENTS`).
    kill_log: Vec<(u32, Place, Place)>,
    /// Every hit on an enemy: (tick, the shooter, the one hit), for the
    /// bench's hit tally ([`HitTally`]).
    hit_log: Vec<(u32, Place, Place)>,
    /// By battery: stolen from this team's container, and when (ms) and
    /// how (alive: dropped by a live carrier) it fell since, if it did.
    stolen_marks: Vec<Option<StolenMark>>,
    /// A stolen battery that fell, until it was home again: its owners'
    /// container (true) or the thief's side's, ms from the fall, alive.
    stolen_drops: Vec<(bool, u32, bool)>,
    /// What befell stolen batteries, for the bench's log.
    stolen_log: Vec<String>,
    /// Lying batteries taken; by place, when its battery was taken; the
    /// carriers killed within [`TAKEN_KILLED_MS`] of their take.
    taken: u32,
    took: Vec<Option<u32>>,
    taken_killed: u32,
    /// Whether a use's ray stops at the bullet mesh short of a lying
    /// battery (`SRP_BENCH_USE_LINE=1` in the real-map runs). LIKELY as
    /// the client: in the owner's match of 2026-10-16 a bot aimed right
    /// at a battery 9 times from where the mesh blocks the line, and took
    /// it at once from a side with a clear one.
    use_line: bool,
    /// How near a battery's middle a take's ray must pass, metres
    /// ([`BATTERY_M`]; `SRP_BENCH_TAKE_M` in the real-map runs makes takes
    /// fail as some do in the owner's matches, audit #7).
    take_m: f32,
    /// Batteries no use takes off the floor (`SRP_BENCH_UNTAKEABLE=<n>`:
    /// the first n; as the sunk ones of audit #7, which a use never took).
    untakeable: usize,
    /// How long a put-in takes ([`PUT_MS`]), and each place's put under
    /// way: the battery, the container, and when it began. A use pressed
    /// while one is under way does nothing (GUESS); the put ends unmade if
    /// the body died or no longer carries that battery.
    put_ms: u32,
    putting: Vec<Option<(usize, usize, u32)>>,
    /// Each use a carrier pressed within [`ARRIVED_M`] of his container.
    put_presses: Vec<PutPress>,
    /// Every put-in of the run: who, and the tick it went in (`put_in`
    /// holds only the last step's).
    put_in_log: Vec<(Place, u32)>,
    /// A camping carrier (audit #6, `SRP_BENCH_CAMP=<m>`): the first of
    /// team 0's carriers to come within this of his container stops there,
    /// moves no more and puts nothing in until he dies or no longer
    /// carries (as the owner's match 0x4da3b51d's bot 13, 8 m from his
    /// container for 273 s). The camper and when he began, once.
    camp_m: Option<f32>,
    camper: Option<(Place, u32)>,
    camp_done: bool,
    /// How the camping ended: (began, ended, he died).
    camp_ended: Option<(u32, u32, bool)>,
    /// Batteries moved unseen (audit #7, `SRP_BENCH_NUDGE=<m>`): every
    /// [`NUDGE_EVERY_MS`] one lying battery, in turn, moves this far on its
    /// floor, as one an enemy took and dropped dead a step on (the owner's
    /// match 0x4da3b51d, battery 3: 0.7 m); the next nudge's time, and how
    /// many moved.
    nudge: Option<(f32, u32)>,
    nudged: u32,
    /// Every shot, for the bench's fight tally ([`fight_tally`]).
    shot_facts: Vec<fight_tally::ShotFact>,
    /// Shots spread as the game's do ([`bench_spread`], `SRP_BENCH_SPREAD`).
    spread: bool,
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
            air_until: 0,
            legs: [0.5; 2],
            leg_broken_until: 0,
            last_hit: 0,
            target: None,
            spread: bench_spread::Spread::default(),
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
            hurts: VecDeque::new(),
            tick: 0,
            rng: Rng::new(seed),
            shots: 0,
            hits: 0,
            shots_hits: vec![(0, 0); teams.len()],
            put_in: Vec::new(),
            team_hit_shots: Vec::new(),
            team_hits: 0,
            jumps_pressed: 0,
            jumps_refused: 0,
            team_kills: 0,
            walled: 0,
            walled_by: [0; 2],
            hidden_by: [0; 2],
            hidden: 0,
            metric_rays: 0,
            delivered: [0, 0],
            stolen: 0,
            thieves: Vec::new(),
            carrier_kills: Vec::new(),
            kill_log: Vec::new(),
            hit_log: Vec::new(),
            stolen_marks: Vec::new(),
            stolen_drops: Vec::new(),
            stolen_log: Vec::new(),
            taken: 0,
            took: vec![None; teams.len()],
            taken_killed: 0,
            use_line: false,
            take_m: BATTERY_M,
            untakeable: 0,
            put_ms: std::env::var("SRP_BENCH_PUT_MS")
                .ok()
                .and_then(|ms| ms.parse().ok())
                .unwrap_or(PUT_MS),
            putting: vec![None; teams.len()],
            put_presses: Vec::new(),
            put_in_log: Vec::new(),
            camp_m: None,
            camper: None,
            camp_done: false,
            camp_ended: None,
            nudge: None,
            nudged: 0,
            shot_facts: Vec::new(),
            spread: false,
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
        body.legs = [0.5; 2];
        body.leg_broken_until = 0;
        body.last_hit = 0;
        body.alive = true;
        body.spawned = tick;
        body.carrying = None;
        // Weapon 1 in hand afresh.
        body.gun = Arms::fallback().primary.0;
        body.recoil = Recoil::new(kick_of(body.gun.item));
        body.cadence = Cadence::default();
        body.spread.reset();
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
        self.put_in.clear();
        self.team_hit_shots.clear();
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
                if pressed[place] & key::JUMP != 0 {
                    self.jumps_pressed += 1;
                    if body.legs.iter().all(|&pool| pool > 0.0) {
                        body.air_until = tick + STAND_IN_AIR_MS;
                    } else {
                        self.jumps_refused += 1;
                    }
                }
                body.keys = keys;
                body.target = target;
                // Another weapon taken, or a reload: no kick.
                if gun.item != body.gun.item {
                    body.gun = gun;
                    body.recoil = Recoil::new(kick_of(gun.item));
                    body.recoil.update(tick);
                    body.cadence = Cadence::default();
                    body.spread.reset();
                } else if pressed[place] & key::RELOAD != 0 {
                    body.recoil.reset();
                }
                body.yaw += turn.0;
                let taken = body.recoil.turn(turn.1);
                body.pitch = (body.pitch + turn.1 + taken).clamp(-1.0, 1.0);
            }
        }
        for (body, &pressed) in self.bodies.iter_mut().zip(&pressed) {
            if body.alive {
                body.spread.tick(body.gun, body.keys, pressed, tick, TICK_S);
            }
        }
        self.record();
        self.camp(tick, &mut pressed);
        for place in 0..n {
            if self.camper.is_some_and(|(p, _)| p == place) {
                continue;
            }
            if !self.bodies[place].alive {
                let team = usize::from(self.bodies[place].team);
                let may = self.tickets.is_none_or(|tickets| tickets[team] > 0);
                if may && tick >= self.bodies[place].dead_at + 6_400 {
                    self.spawn(place);
                }
                continue;
            }
            let body = &mut self.bodies[place];
            if body.leg_broken_until != 0 && tick >= body.leg_broken_until {
                body.legs = body.legs.map(|pool| if pool > 0.0 { pool } else { 0.5 });
                body.leg_broken_until = 0;
            }
            let mut level = if body.carrying.is_some() { 0.95 } else { 1.0 };
            if body.last_hit != 0 && tick < body.last_hit + STAND_IN_STAGGER_MS {
                level *= STAND_IN_STAGGER;
            }
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
                let grid = &self.world.grid;
                let (mut node, mut feet, _) = slide(grid, node, body.feet, to);
                // In the air, walking into a floor the jump clears: up on
                // it (once a jump).
                if tick < body.air_until
                    && feet.distance_xz(to) > 1e-3
                    && let Some(up) = stand_in_jump(grid, node, feet, to, STANDING_JUMP)
                {
                    node = up;
                    feet = grid.position(up);
                    body.air_until = 0;
                }
                body.moved += feet.distance_xz(body.feet);
                body.node = Some(node);
                body.feet = feet;
            }
        }
        self.shoot(tick);
        self.use_and_drop(&pressed);
        if let Some((m, next)) = self.nudge
            && tick >= next
        {
            self.nudge = Some((m, next + NUDGE_EVERY_MS));
            self.nudge_one(m, tick);
        }
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

    /// A lying battery, the next in turn, moved `m` on its floor (`nudge`).
    fn nudge_one(&mut self, m: f32, tick: u32) {
        let lying: Vec<usize> = (0..self.batteries.len())
            .filter(|&b| matches!(self.batteries[b], Lying::Free(..)))
            .collect();
        if lying.is_empty() {
            return;
        }
        let k = (tick / NUDGE_EVERY_MS) as usize;
        let battery = lying[k % lying.len()];
        let Lying::Free(at, mask) = self.batteries[battery] else {
            return;
        };
        let grid = &self.world.grid;
        let a = k as f32 * 2.4;
        let to = Vec3::new(at.x + m * a.cos(), at.y, at.z + m * a.sin());
        let (Some(from_floor), Some(to_floor)) = (grid.locate(at, 0.6), grid.locate(to, 0.6)) else {
            return;
        };
        let (from_floor, to_floor) = (grid.position(from_floor), grid.position(to_floor));
        if to_floor.distance_xz(to) > 0.3 || (to_floor.y - from_floor.y).abs() > 0.5 {
            return;
        }
        let up = at.y - from_floor.y;
        self.batteries[battery] = Lying::Free(Vec3::new(to.x, to_floor.y + up, to.z), mask);
        self.nudged += 1;
    }

    /// The camping carrier (`camp_m`): the first of team 0's carriers
    /// within `camp_m` of his container becomes him; he presses no use
    /// (and `step` does not move him) until he dies or no longer carries.
    fn camp(&mut self, tick: u32, pressed: &mut [u32]) {
        let Some(camp_m) = self.camp_m else {
            return;
        };
        if let Some((place, began)) = self.camper {
            let body = &self.bodies[place];
            if body.alive && body.carrying.is_some() {
                pressed[place] &= !key::USE;
                return;
            }
            self.camp_ended = Some((began, tick, !body.alive));
            self.camper = None;
            self.camp_done = true;
        }
        if self.camp_done {
            return;
        }
        let Some(&(_, home)) = self.containers.iter().find(|(t, _)| *t == 0) else {
            return;
        };
        if let Some(place) = (0..self.bodies.len()).find(|&p| {
            let body = &self.bodies[p];
            body.alive
                && body.team == 0
                && body.carrying.is_some()
                && body.feet.distance_xz(home) <= camp_m
        }) {
            self.camper = Some((place, tick));
            pressed[place] &= !key::USE;
        }
    }

    /// Shots, each along the view (the base pitch and the kick) and each
    /// kicking it: a hit on the nearest chest within 0.45 m of the ray, a
    /// teammate's too (the game has friendly fire).
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
        self.shots_hits[shooter].0 += 1;
        let body = &self.bodies[shooter];
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let pitch = body.recoil.view(body.pitch);
        let (c, s) = (pitch.cos(), pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        let kick = body.recoil.kick();
        let top = kick.top(self.rng.unit());
        let (gun, keys) = (body.gun, body.keys);
        self.bodies[shooter].recoil.shot(at, top, 0.0);
        let (aim_state, cone) = self.bodies[shooter].spread.shot(gun, keys, at);
        let view_dir = dir;
        let dir = if self.spread {
            bench_spread::deflect(dir, cone, &mut self.rng)
        } else {
            dir
        };
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
            let team = usize::from(body.team).min(1);
            if !super::aimpoint::shows(eye, target.feet, crouched, &self.world.sight, || true) {
                self.hidden += 1;
                self.hidden_by[team] += 1;
            }
            let reach = (Self::chest(target) - eye).length() - 0.5;
            if self.world.sight.ray_cast(eye, dir, reach).is_some() {
                self.walled += 1;
                self.walled_by[team] += 1;
            }
            self.metric_rays += super::aimpoint::rays_cast() - before;
        }
        let mut best: Option<(Place, f32)> = None;
        for (target, other) in self.bodies.iter().enumerate() {
            if !other.alive || target == shooter {
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
        let shooter_body = &self.bodies[shooter];
        let (moving, aimed, crouched) = fight_tally::stance(shooter_body.keys);
        let aimed_at = shooter_body
            .target
            .filter(|&t| self.bodies[t].alive)
            .map(|t| {
                let (d, off) = fight_tally::aim_off(eye, dir, Self::chest(&self.bodies[t]));
                (t, d, off)
            });
        self.shot_facts.push(fight_tally::ShotFact {
            shooter,
            tick,
            hit: best
                .map(|b| b.0)
                .filter(|&t| self.bodies[t].team != shooter_body.team),
            target: aimed_at,
            moving,
            aimed,
            crouched,
            aim_state,
            cone,
        });
        let Some((target, _)) = best else {
            return;
        };
        let team_hit = self.bodies[target].team == body.team;
        if team_hit {
            self.team_hits += 1;
            self.team_hit_shots.push((shooter, target, eye, dir, cone, view_dir));
        } else {
            self.hits += 1;
            self.shots_hits[shooter].1 += 1;
            self.hit_log.push((tick, shooter, target));
        }
        let other = &mut self.bodies[target];
        other.health -= 0.2;
        other.last_hit = tick;
        if let Some(leg) = stand_in_leg_hit(tick, shooter, target) {
            other.legs[leg] = (other.legs[leg] - STAND_IN_LEG_HIT).max(0.0);
            if other.legs[leg] <= 0.0 && other.leg_broken_until == 0 {
                other.leg_broken_until = tick + STAND_IN_LEG_BROKEN_MS;
            }
        }
        if other.health <= 1e-3 {
            other.alive = false;
            other.dead_at = tick;
            if let Some(battery) = other.carrying.take() {
                if self.took[target].is_some_and(|at| tick <= at + TAKEN_KILLED_MS) {
                    self.taken_killed += 1;
                }
                self.batteries[battery] = Lying::Free(other.feet, 1 << other.team);
                self.carrier_kills.push((shooter, target));
                if let Some(Some((_, fell @ None))) = self.stolen_marks.get_mut(battery) {
                    *fell = Some((tick, false));
                    self.stolen_log.push(format!(
                        "{tick} battery {battery} fell at its carrier's death"
                    ));
                }
            }
            self.kill_log.push((tick, shooter, target));
            self.stats[target].1 += 1;
            self.stats[target].2 = 0;
            if let Some(tickets) = self.tickets.as_mut() {
                let team = usize::from(other.team);
                tickets[team] = tickets[team].saturating_sub(1);
            }
            if team_hit {
                self.team_kills += 1;
            } else {
                self.stats[shooter].0 += 1;
                self.stats[shooter].2 += 1;
            }
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

    /// How far `body`'s use ray (from the eye, [`USE_REACH_M`] long)
    /// passes `point`.
    fn ray_miss(body: &Body, point: Vec3) -> f32 {
        let eye = Self::eye(body);
        let [fx, fz] = forward(body.yaw);
        let (c, s) = (body.pitch.cos(), body.pitch.sin());
        let dir = Vec3::new(fx * c, s, fz * c);
        let along = (point - eye).dot(dir).clamp(0.0, USE_REACH_M);
        (eye + dir * along - point).length()
    }

    /// [`Self::uses`] a lying battery, the ray clear of the bullet mesh up
    /// to it when [`Game::use_line`].
    fn uses_battery(&self, body: &Body, at: Vec3) -> bool {
        let point = at + BATTERY_MIDDLE;
        if !Self::uses(body, point, self.take_m) {
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
        // The puts whose time is up go in.
        for place in 0..self.bodies.len() {
            let Some((battery, id, began)) = self.putting[place] else {
                continue;
            };
            let body = &self.bodies[place];
            if !body.alive || body.carrying != Some(battery) {
                self.putting[place] = None;
            } else if self.tick >= began + self.put_ms {
                self.putting[place] = None;
                self.put_into(place, battery, id);
            }
        }
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
                    let was_putting = self.putting[place].is_some();
                    let began = !was_putting
                        && home.is_some_and(|id| {
                            Self::uses(&body, self.containers[id].1 + USE_BOX_UP, USE_BOX_M)
                        });
                    if began && let Some(id) = home {
                        if self.put_ms == 0 {
                            self.put_into(place, battery, id);
                        } else {
                            self.putting[place] = Some((battery, id, self.tick));
                        }
                    }
                    if let Some(id) = home
                        && body.feet.distance_xz(self.containers[id].1) <= ARRIVED_M
                    {
                        self.put_presses.push(PutPress {
                            place,
                            tick: self.tick,
                            feet: body.feet,
                            miss: Self::ray_miss(&body, self.containers[id].1 + USE_BOX_UP),
                            began,
                            putting: was_putting,
                        });
                    }
                }
                None => {
                    let free = self.batteries.iter().enumerate().position(|(i, b)| {
                        i >= self.untakeable
                            && matches!(b, Lying::Free(at, _) if self.uses_battery(&body, *at))
                    });
                    let theirs = self.containers.iter().position(|(t, at)| {
                        *t != team && Self::uses(&body, *at + USE_BOX_UP, USE_BOX_M)
                    });
                    if let Some(battery) = free {
                        self.batteries[battery] = Lying::Carried(place);
                        self.bodies[place].carrying = Some(battery);
                        self.taken += 1;
                        self.took[place] = Some(self.tick);
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

    /// `place`'s battery goes into container `id`.
    fn put_into(&mut self, place: Place, battery: usize, id: usize) {
        let team = self.bodies[place].team;
        self.batteries[battery] = Lying::Home(id as u8);
        self.bodies[place].carrying = None;
        self.stolen_home(battery, team);
        self.points[usize::from(team)] += 1;
        self.delivered[usize::from(team)] += 1;
        self.put_in.push(place);
        self.put_in_log.push((place, self.tick));
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
        let tick = self.tick;
        let hurts = self
            .bodies
            .iter()
            .map(|b| Hurt::of(b.legs.map(Some), b.last_hit, tick))
            .collect();
        self.hurts.push_back((tick, hurts));
        while self.hurts.len() > 40 {
            self.hurts.pop_front();
        }
    }

    /// The bodies' hurts in the world of `tick` ([`Self::world_at`]).
    fn hurts_at(&self, tick: u32) -> Option<Vec<Hurt>> {
        let (_, hurts) = self.hurts.iter().find(|(t, _)| *t == tick)?;
        Some(hurts.clone())
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
    taken: u32,
    taken_killed: u32,
    shots: u32,
    hits: u32,
    team_hits: u32,
    team_kills: u32,
    walled: u32,
    hidden: u32,
    /// Sight and aim rays the bots cast (`aimpoint`), per bot and second
    /// of play.
    rays_per_bot_s: f32,
    least_moved_m: f32,
    stuck_notes: usize,
    #[allow(dead_code)]
    notes: Vec<Note>,
    /// Every raid taken (play 24), as it went.
    raids: Vec<RaidRun>,
    carry: CarryTally,
}

/// The batteries' carries as the bots saw them (the audit of the owner's
/// matches of 2026-10-23, #2, #6, #7): the carrier the view believed
/// against the stand-in's, world by world; the put-ins and takes that
/// failed; the intercepts and the interceptors killed. With
/// `SRP_BENCH_NOTES=<file>` every note and death is also appended there
/// (`<run> <place> <ms> <text>`), for the churn counts.
#[derive(Clone, Debug, Default)]
struct CarryTally {
    run: String,
    /// World-seconds of a carried battery: its carrier believed right,
    /// believed someone else, not known.
    right: u32,
    wrong: u32,
    unknown: u32,
    put_in_failed: u32,
    take_failed: u32,
    stands_failed: u32,
    collect_takes: u32,
    intercept_takes: u32,
    intercept_deaths: u32,
    /// Beliefs the view dropped or moved (its notes).
    beliefs_moved: u32,
    /// Each carrier's first world within [`ARRIVED_M`] of his container on
    /// this carry, and the put-ins seen; the put-ins' waits from there, ms.
    arrived: Vec<Option<u32>>,
    put_ins_seen: usize,
    put_in_waits: Vec<u32>,
    /// The uses at the containers ([`Game::put_presses`]) counted so far;
    /// of them: all, those that missed the box (no put began, none going
    /// on), those whose ray passed it by over [`FAR_MISS_M`] (the bot
    /// judges those missed at once), of these the ones that began a put all
    /// the same (a put the bot cut short) and the ones whose carrier's next
    /// use from the same stand began a put; and the waits from the
    /// arrival to the carry's first use there, ms.
    presses_seen: usize,
    presses: u32,
    presses_missed: u32,
    presses_far: u32,
    far_began: u32,
    far_then_in: u32,
    pressed: Vec<bool>,
    first_seen: usize,
    first_press_waits: Vec<u32>,
}

/// A carrier is at his container within this, metres (xz).
const ARRIVED_M: f32 = 3.0;
/// The bot's rule for a use that surely missed the box (`collect`'s
/// `PUT_MISS_M`), metres; the next use "from the same stand" is within
/// [`SAME_STAND_M`] and [`NEXT_PRESS_MS`] of it.
const FAR_MISS_M: f32 = 1.0;
const SAME_STAND_M: f32 = 0.3;
const NEXT_PRESS_MS: u32 = 3_000;

impl CarryTally {
    fn new(run: String) -> Self {
        Self {
            run,
            ..Self::default()
        }
    }

    /// A world: each carried battery's believed carrier against the truth;
    /// the carriers at their containers, and how long the put-ins took
    /// from there.
    fn world(&mut self, game: &Game, bots: &Bots) {
        self.arrived.resize(game.bodies.len(), None);
        self.pressed.resize(game.bodies.len(), false);
        // The uses old enough that the next one from the same stand is in.
        let presses = &game.put_presses;
        while let Some(press) = presses.get(self.presses_seen)
            && press.tick + NEXT_PRESS_MS <= game.tick
        {
            self.presses += 1;
            self.presses_missed += u32::from(!press.began && !press.putting);
            if press.miss > FAR_MISS_M {
                self.presses_far += 1;
                self.far_began += u32::from(press.began);
                let next = presses[self.presses_seen + 1..].iter().find(|p| {
                    p.place == press.place
                        && p.tick <= press.tick + NEXT_PRESS_MS
                        && p.feet.distance_xz(press.feet) <= SAME_STAND_M
                });
                self.far_then_in += u32::from(next.is_some_and(|p| p.began));
            }
            self.presses_seen += 1;
        }
        // The carry's first use there, from its arrival.
        for press in &presses[self.first_seen.min(presses.len())..] {
            if let Some(at) = self.arrived.get(press.place).copied().flatten()
                && !self.pressed[press.place]
                && press.tick >= at
            {
                self.pressed[press.place] = true;
                self.first_press_waits.push(press.tick - at);
            }
        }
        self.first_seen = presses.len();
        // The put-ins since, after the uses: a carry's first use and its
        // put-in may fall between the same two worlds.
        let log = &game.put_in_log;
        for &(place, tick) in &log[self.put_ins_seen.min(log.len())..] {
            if let Some(at) = self.arrived[place].take() {
                self.put_in_waits.push(tick.saturating_sub(at));
            }
        }
        self.put_ins_seen = log.len();
        for (place, body) in game.bodies.iter().enumerate() {
            let home = game.containers.iter().find(|(t, _)| *t == body.team);
            if !body.alive || body.carrying.is_none() {
                self.arrived[place] = None;
                self.pressed[place] = false;
            } else if self.arrived[place].is_none()
                && home.is_some_and(|(_, at)| at.distance_xz(body.feet) <= ARRIVED_M)
            {
                self.arrived[place] = Some(game.tick);
            }
        }
        for (battery, lying) in game.batteries.iter().enumerate() {
            if let Lying::Carried(place) = *lying {
                match bots.tracker().carrier(battery) {
                    Some(p) if p == place => self.right += 1,
                    Some(_) => self.wrong += 1,
                    None => self.unknown += 1,
                }
            }
        }
    }

    /// The deaths of a step (the statistics `before` it), by the play.
    fn deaths(&mut self, before: &[(u32, u32, u32)], game: &Game, bots: &Bots) {
        for (place, (now, was)) in game.stats.iter().zip(before).enumerate() {
            if now.1 <= was.1 {
                continue;
            }
            let play = bots.bots[place]
                .as_ref()
                .and_then(|b| b.assignment.as_ref())
                .map(|a| a.kind);
            if play == Some(super::plays::PlayKind::Intercept) {
                self.intercept_deaths += 1;
            }
            self.dump(place, game.tick, &format!("died (on {play:?})"));
        }
    }

    fn note(&mut self, note: &Note) {
        let text = note.text.as_str();
        if text.starts_with("ends Collect: could not put it in") {
            self.put_in_failed += 1;
        } else if text.starts_with("ends Collect: could not take it") {
            self.take_failed += 1;
        } else if text.starts_with("took no battery") {
            self.stands_failed += 1;
        } else if text.starts_with("takes Collect Battery") {
            self.collect_takes += 1;
        } else if text.starts_with("takes Intercept") {
            self.intercept_takes += 1;
        } else if text.contains("believed to carry battery") {
            self.beliefs_moved += 1;
        }
        self.dump(note.place, note.time, text);
    }

    fn dump(&self, place: usize, time: u32, text: &str) {
        if let Some(path) = std::env::var_os("SRP_BENCH_NOTES") {
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            writeln!(file, "{} {} {time} {text}", self.run, place as i64).unwrap();
        }
    }

    fn line(&self, taken: u32) -> String {
        format!(
            "carrier believed right {} wrong {} unknown {} world-s; put-ins failed {}; takes failed {}, stands failed {}; collect takes {} for {taken} pickups; intercepts {}, interceptors killed {}; beliefs moved {}; put-ins from arrival {} took {} ms; uses at home {}, missed {}, far {} (began a put {}), far then in {}; first uses from arrival {} took {} ms",
            self.right,
            self.wrong,
            self.unknown,
            self.put_in_failed,
            self.take_failed,
            self.stands_failed,
            self.collect_takes,
            self.intercept_takes,
            self.intercept_deaths,
            self.beliefs_moved,
            self.put_in_waits.len(),
            self.put_in_waits
                .iter()
                .map(|&ms| u64::from(ms))
                .sum::<u64>(),
            self.presses,
            self.presses_missed,
            self.presses_far,
            self.far_began,
            self.far_then_in,
            self.first_press_waits.len(),
            self.first_press_waits
                .iter()
                .map(|&ms| u64::from(ms))
                .sum::<u64>(),
        )
    }
}

/// One raid of a soak, from its take to the bot's next play.
#[derive(Clone, Debug, Default)]
struct RaidRun {
    /// The raider.
    place: Place,
    /// Its score at the take.
    score: f32,
    /// How long it ran, s.
    ran_s: f32,
    /// How near the eye came to their container's use box, m.
    nearest_m: f32,
    /// Uses pressed meanwhile.
    uses: u32,
    stole: bool,
    died: bool,
    /// What came next: the next play, or how it ended.
    next: String,
}

/// The raids of a run as they go ([`RaidRun`]), each bot's from its take
/// to its next play.
#[derive(Default)]
struct RaidTally {
    /// Each bot's raid under way: its start, and the run so far.
    raiding: Vec<Option<(u32, RaidRun)>>,
    runs: Vec<RaidRun>,
}

impl RaidTally {
    /// After the game's steps up to `clock`: the raiders' bodies, and the
    /// `thieves` of those steps.
    fn bodies(&mut self, game: &Game, thieves: &[Place], clock: u32) {
        self.raiding.resize(game.bodies.len(), None);
        for (place, raid) in self.raiding.iter_mut().enumerate() {
            let Some((since, run)) = raid else {
                continue;
            };
            let body = &game.bodies[place];
            if let Some(&(_, at)) = game.containers.iter().find(|(t, _)| *t != body.team) {
                let d = (at + USE_BOX_UP - Game::eye(body)).length();
                run.nearest_m = run.nearest_m.min(d);
            }
            run.stole |= thieves.contains(&place);
            run.died |= !body.alive;
            run.ran_s = clock.saturating_sub(*since) as f32 / 1000.0;
        }
    }

    /// After a poll: its notes, the plays now and the inputs made.
    fn polled(&mut self, bots: &Bots, notes: &[Note], made: &[Made]) {
        self.raiding.resize(bots.bots.len(), None);
        for note in notes {
            if let Some(Some((_, run))) = self.raiding.get_mut(note.place)
                && note.text.starts_with("ends Raid")
            {
                run.next = note.text.clone();
            }
        }
        for (place, raid) in self.raiding.iter_mut().enumerate() {
            let assignment = bots.bots[place]
                .as_ref()
                .and_then(|bot| bot.assignment.as_ref());
            let on = assignment.filter(|a| a.kind == super::plays::PlayKind::Raid);
            if raid.as_ref().map(|r| r.0) != on.map(|a| a.since) {
                if let Some((_, mut run)) = raid.take() {
                    if run.next.is_empty() {
                        run.next = assignment.map_or("none".into(), |a| format!("{:?}", a.kind));
                    }
                    self.runs.push(run);
                }
                *raid = on.map(|a| {
                    let run = RaidRun {
                        place,
                        score: a.score,
                        nearest_m: f32::INFINITY,
                        ..Default::default()
                    };
                    (a.since, run)
                });
            }
        }
        for made in made {
            if made.input.actions & key::USE != 0
                && let Some(Some((_, run))) = self.raiding.get_mut(made.place)
            {
                run.uses += 1;
            }
        }
    }

    /// The runs, those under way at the end too.
    fn finish(mut self) -> Vec<RaidRun> {
        for (_, mut run) in self.raiding.into_iter().flatten() {
            run.next = "the match's end".into();
            self.runs.push(run);
        }
        self.runs
    }
}

/// One line on `runs`: how many, how long, how near, the uses, the steals,
/// the deaths; and what came after them.
fn raid_line(runs: &[RaidRun]) -> String {
    let n = runs.len().max(1);
    let mut ran: Vec<f32> = runs.iter().map(|r| r.ran_s).collect();
    ran.sort_by(f32::total_cmp);
    let mut scores: Vec<f32> = runs.iter().map(|r| r.score).collect();
    scores.sort_by(f32::total_cmp);
    let mut next: std::collections::BTreeMap<&str, u32> = Default::default();
    for run in runs {
        let what = run.next.split(';').next().unwrap_or_default();
        *next.entry(what).or_default() += 1;
    }
    let mut next: Vec<(&str, u32)> = next.into_iter().collect();
    next.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    next.truncate(8);
    format!(
        "{} raids; score median {:.2}; ran median {:.1} s, over 10 s {}, over 30 s {}; came within 5 m {}, 2 m {}; used {}; stole {}; died on one {}; next {next:?}",
        runs.len(),
        scores.get(n / 2).copied().unwrap_or(0.0),
        ran.get(n / 2).copied().unwrap_or(0.0),
        runs.iter().filter(|r| r.ran_s > 10.0).count(),
        runs.iter().filter(|r| r.ran_s > 30.0).count(),
        runs.iter().filter(|r| r.nearest_m <= 5.0).count(),
        runs.iter().filter(|r| r.nearest_m <= 2.0).count(),
        runs.iter().filter(|r| r.uses > 0).count(),
        runs.iter().filter(|r| r.stole).count(),
        runs.iter().filter(|r| r.died).count(),
    )
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
    // `SRP_SOAK_TRACE=<dir>`: the decision trace of each run there.
    let trace = std::env::var_os("SRP_SOAK_TRACE").map_or(Trace::none(0), |dir| {
        Trace::open(std::path::Path::new(&dir), seed as u32, 0, u64::MAX)
    });
    let cheat = if tuning.cheats.batteries { "-cheat" } else { "" };
    let mut bots = Bots::new(
        0x50AC + seed as u32,
        mode,
        0,
        &teams,
        &driven,
        &roles,
        Arc::new(tuning),
        trace,
    );
    bots.map_ready(world.clone());
    let tickets = (win == 0).then_some(tdm_tickets);
    let mut game = Game::new(world, &teams, seed, tickets, timed_s.map(|s| s * 1000));
    for place in 0..teams.len() {
        bots.entered(place, true, 0);
    }
    let mut out = Outcome {
        carry: CarryTally::new(format!("soak{per_team}-s{seed}{cheat}")),
        ..Outcome::default()
    };
    let rays_before = super::aimpoint::rays_cast();
    let limit_ms = limit_s * 1000;
    let mut raids = RaidTally::default();
    let (mut clock, mut next_world) = (0, 1_000);
    while clock < limit_ms {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            let before = game.stats.clone();
            game.step();
            out.carry.deaths(&before, &game, &bots);
        }
        let thieves = std::mem::take(&mut game.thieves);
        raids.bodies(&game, &thieves, clock);
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    let notes = bots.fixed(place, fix, Some(health));
                    notes.iter().for_each(|note| out.decisions.note(note));
                    notes.iter().for_each(|note| out.carry.note(note));
                    out.notes.extend(notes);
                }
                let notes = bots.facts(facts);
                notes.iter().for_each(|note| out.decisions.note(note));
                notes.iter().for_each(|note| out.carry.note(note));
                out.notes.extend(notes);
                out.carry.world(&game, &bots);
            }
        }
        let (made, notes) = bots.poll(Clock {
            clock,
            time_left_s: (limit_ms - clock) as f32 / 1000.0,
            total_s: limit_s as f32,
            playing: true,
        });
        notes.iter().for_each(|note| out.decisions.note(note));
        notes.iter().for_each(|note| out.carry.note(note));
        raids.polled(&bots, &notes, &made);
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
    out.raids = raids.finish();
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
    out.taken = game.taken;
    out.taken_killed = game.taken_killed;
    out.shots = game.shots;
    out.hits = game.hits;
    out.team_hits = game.team_hits;
    out.team_kills = game.team_kills;
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

/// The stand-in's put-in lasts [`PUT_MS`]: the battery goes in at its end,
/// a use pressed meanwhile does nothing, and a carrier who died first puts
/// nothing in.
#[test]
fn a_stand_in_put_in_goes_in_at_the_end_of_the_put() {
    let mut game = Game::new(soak_world(), &[0, 1], 1, None, None);
    let (_, home) = game.containers[0];
    let feet = home + Vec3::new(0.0, 0.0, 1.0);
    let body = &mut game.bodies[0];
    body.alive = true;
    body.feet = feet;
    // Facing the use box (-z), a metre over the container's point.
    body.yaw = std::f32::consts::PI;
    body.pitch = (1.0 - 1.6f32).atan2(1.0);
    body.carrying = Some(0);
    game.batteries[0] = Lying::Carried(0);
    game.put_ms = PUT_MS;
    game.use_and_drop(&[key::USE, 0]);
    assert_eq!(game.batteries[0], Lying::Carried(0), "in the put");
    game.tick += PUT_MS - 50;
    game.use_and_drop(&[key::USE, 0]);
    assert_eq!(game.batteries[0], Lying::Carried(0), "a press meanwhile");
    game.tick += 50;
    game.use_and_drop(&[0, 0]);
    assert_eq!(game.batteries[0], Lying::Home(0));
    assert_eq!(game.delivered[0], 1);
    // Dead before the put ends: nothing goes in.
    game.bodies[0].carrying = Some(1);
    game.batteries[1] = Lying::Carried(0);
    game.use_and_drop(&[key::USE, 0]);
    game.bodies[0].alive = false;
    game.tick += PUT_MS;
    game.use_and_drop(&[0, 0]);
    assert_eq!(game.batteries[1], Lying::Carried(0));
    assert_eq!(game.delivered[0], 1);
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

/// No shot through a teammate (`friends`): Team Deathmatch matches of 6 a
/// side with the check off and on, the same seeds. Off, a bot fires
/// through a teammate as it did live (2026-10-20..23: 7,250 team kills to
/// 27,088 kills); on, far fewer team hits, and the enemy still dies about
/// as often.
/// A take waits for the fight while an enemy sees the taker, unless he
/// reloads or a teammate covers ([`Ctx::exposed_to`]): the battery soaks
/// (4 a side to 3, seeds 1-8, the battery cheat off and on) with the wait
/// off (`exposed_m` 0) and on. Prints the batteries taken and brought,
/// the carriers killed within [`TAKEN_KILLED_MS`] of their take, and the
/// wins; batteries are still brought with it on.
///
/// [`Ctx::exposed_to`]: super::plays::Ctx::exposed_to
#[test]
#[ignore = "whole matches; run in release"]
fn a_take_waits_for_the_fight_unless_covered() {
    let mut totals = Vec::new();
    for on in [false, true] {
        let (mut taken, mut killed, mut brought, mut won, mut runs) = (0, 0, 0, 0, 0);
        let mut draws = Vec::new();
        for cheat in [false, true] {
            for seed in 1..=16 {
                let mut tuning = Tuning::default();
                tuning.cheats.batteries = cheat;
                if !on {
                    let mut play = super::plays::default_tuning(PlayKind::Collect);
                    play.numbers.insert("exposed_m".into(), 0.0);
                    tuning.plays.insert(PlayKind::Collect, play);
                }
                let out = soak_tuned(4, 3, 0, None, 900, seed, tuning);
                taken += out.taken;
                killed += out.taken_killed;
                brought += out.delivered[0] + out.delivered[1];
                won += u32::from(out.delivered.iter().any(|&d| d >= 3));
                if out.winner.is_none() {
                    draws.push((cheat, seed));
                }
                runs += 1;
            }
        }
        eprintln!(
            "take waits {on}: taken {taken}, killed within 3 s of a take {killed}, brought {brought}, won by batteries {won} of {runs}; draws (cheat, seed) {draws:?}"
        );
        totals.push((taken, killed, brought));
    }
    let (_, _, brought_on) = totals[1];
    let (_, _, brought_off) = totals[0];
    assert!(
        brought_on * 10 >= brought_off * 8,
        "batteries still brought: {brought_on} against {brought_off}"
    );
}

#[test]
#[ignore = "whole matches; run in release"]
fn bots_hold_fire_while_a_teammate_is_in_the_line_of_fire() {
    let mut totals = Vec::new();
    for on in [false, true] {
        let mut tuning = bench_tuning();
        tuning.aim.friends.on = on;
        let (mut kills, mut team_kills, mut hits, mut team_hits, mut shots) = (0, 0, 0, 0, 0);
        for seed in 1..=6 {
            let out = soak_tuned(6, 0, 60, None, 600, seed, tuning.clone());
            kills += out.kills[0] + out.kills[1];
            team_kills += out.team_kills;
            hits += out.hits;
            team_hits += out.team_hits;
            shots += out.shots;
        }
        eprintln!(
            "friends.on {on}: kills {kills}, team kills {team_kills}; hits {hits}, team hits {team_hits} of {shots} shots"
        );
        totals.push((kills, team_kills, hits, team_hits));
    }
    let [
        (kills_off, _, _, team_hits_off),
        (kills_on, _, _, team_hits_on),
    ] = totals[..]
    else {
        unreachable!()
    };
    assert!(
        team_hits_off >= 20,
        "the harness sees friendly fire with the check off: {team_hits_off}"
    );
    assert!(
        team_hits_on * 4 <= team_hits_off,
        "a quarter of the team hits at most: {team_hits_on} against {team_hits_off}"
    );
    assert!(
        kills_on * 10 >= kills_off * 8,
        "the enemy dies about as often: {kills_on} kills against {kills_off}"
    );
}

/// The raids (play 24) of whole battery matches, the battery cheat off
/// and on, `SRP_RAID_SEEDS` seeds each (default 8): how many are taken,
/// how long they run, how near their container they come, the uses, the
/// steals, and the matches' deliveries and winners. A measurement: it
/// asserts nothing.
#[test]
#[ignore = "whole matches; run in release"]
fn raids_in_whole_battery_matches() {
    let seeds: u64 = std::env::var("SRP_RAID_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    for cheat in [false, true] {
        let mut tuning = bench_tuning();
        tuning.cheats.batteries = cheat;
        let mut runs: Vec<RaidRun> = Vec::new();
        let (mut stolen, mut delivered, mut wins, mut decided) = (0, 0, [0u32; 2], 0);
        for seed in 1..=seeds {
            let out = soak_tuned(4, 3, 0, None, 900, seed, tuning.clone());
            stolen += out.stolen;
            delivered += out.delivered[0] + out.delivered[1];
            if let Some(team) = out.winner {
                wins[usize::from(team)] += 1;
                decided += u32::from(out.end_s < 899.0);
            }
            eprintln!(
                "  cheat {cheat} seed {seed}: winner {:?} at {:.0} s, delivered {:?}, stolen {}, raids {}, raid steals {}; kills {:?}; {}",
                out.winner,
                out.end_s,
                out.delivered,
                out.stolen,
                out.raids.len(),
                out.raids.iter().filter(|r| r.stole).count(),
                out.kills,
                out.carry.line(out.taken)
            );
            runs.extend(out.raids);
        }
        eprintln!(
            "cheat {cheat}: {}; all steals {stolen}, delivered {delivered}, wins {wins:?} ({decided} by the points)",
            raid_line(&runs)
        );
    }
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
    // Seed 2: a match of these may end level (1 of 32 with the rule of
    // `a_take_waits_for_the_fight_unless_covered` and without it); seed 1
    // did with it, 1-1 after 900 s.
    let out = soak_tuned(4, 3, 0, None, 900, 2, tuning.clone());
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

/// Who keeps the enemy by the battery busy in [`fight_by_a_battery`].
#[derive(Clone, Copy, PartialEq)]
enum Busy {
    /// Nobody: the bot alone in his sight.
    Nobody,
    /// A teammate (place 2, no bot) with a clear line to him.
    Mate,
    /// He reloads (a reload pressed every second).
    Reloading,
}

/// A bot walks in to battery 1 with an enemy in plain sight past it
/// (place 1, no bot, who does not die: the fight never ends), for 15 s:
/// when it took the battery, if it did, and whether it fought him.
fn fight_by_a_battery(busy: Busy) -> (Option<u32>, bool, u32) {
    use survarium::v026g::match_msgs::{PlayerInput, WireTime};
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = true;
    let teams = [0u8, 1, 0];
    let mut bots = Bots::new(
        0x7A70,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false, false],
        &[None, None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    // 8 m east of battery 1 (25, 0.3, 12), the enemy 10 m past it, on the
    // same side of the wall: the bot sees him as it walks in. The
    // teammate 13 m off him, out of the bot's line; or far off, behind
    // the wall, when he does not cover.
    let mate = if busy == Busy::Mate {
        Vec3::new(26.0, 0.0, 4.0)
    } else {
        Vec3::new(70.0, 0.0, 35.0)
    };
    for (place, at) in [
        (0, Vec3::new(33.0, 0.0, 12.0)),
        (1, Vec3::new(15.0, 0.0, 12.0)),
        (2, mate),
    ] {
        game.bodies[place].node = world.grid.locate(at, 1.0);
        game.bodies[place].feet = at;
    }
    for place in 0..3 {
        bots.entered(place, true, 0);
    }
    let (mut clock, mut next_world, mut took) = (0, 1_000, None);
    let mut fought = false;
    while clock < 15_000 && took.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
            game.bodies[1].health = 1.0;
            if busy == Busy::Reloading && game.tick.is_multiple_of(TICK_MS) {
                let reload = game.tick.is_multiple_of(1_000);
                let input = PlayerInput {
                    time: WireTime::tick(game.tick),
                    rotation: [0; 4],
                    actions: if reload { key::RELOAD } else { 0 },
                };
                bots.input(1, game.tick, &input);
            }
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
        let target = bots.bots[0].as_ref().and_then(|bot| bot.reflex.target());
        fought |= target == Some(1);
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, target, gun));
        }
        if game.bodies[0].carrying == Some(1) {
            took = Some(clock);
        }
    }
    assert!(game.bodies[1].alive);
    (took, fought, game.shots)
}

/// A bot by a battery with an enemy in plain sight whom nobody keeps busy
/// fights him and does not take it: a take turns the view from him and a
/// carrier cannot shoot ([`Ctx::exposed_to`]).
///
/// [`Ctx::exposed_to`]: super::plays::Ctx::exposed_to
#[test]
fn a_bot_in_an_enemys_sight_fights_before_it_takes_a_battery() {
    let (took, fought, shots) = fight_by_a_battery(Busy::Nobody);
    assert!(fought && shots > 0, "no fight: {shots} shots");
    assert_eq!(took, None, "taken in his sight");
}

/// The same bot takes the battery when a teammate has a clear line to the
/// enemy, or while the enemy reloads: within the use's reach the take's
/// look wins over the reflex's aim (`USE_LOOK_M`). The client's use is one
/// ray along the view; a bot whose reflex kept its view on the enemy
/// pressed use at him, and in the whole-match soaks the battery where the
/// teams meet lay there all match.
#[test]
fn a_covered_bot_takes_a_battery_in_an_enemys_sight() {
    for busy in [Busy::Mate, Busy::Reloading] {
        let (took, fought, shots) = fight_by_a_battery(busy);
        assert!(fought && shots > 0, "no fight: {shots} shots");
        let took = took.unwrap_or_else(|| panic!("not taken ({shots} shots)"));
        // Its first move is at 2 s; 8 m in, a turn to the battery and a
        // use or two (without the use's look: still not taken at 15 s, 66
        // shots).
        assert!(took <= 9_000, "taken at {took} ms");
    }
}

/// A raid that gets there steals (play 24): bot 0 of team 0, 0-3 down,
/// 25 m from the enemy's container with all three batteries in it, their
/// one body dead for the run, 5 min into the match. It walks in, crouches
/// the last metres, steps in to the use's reach and takes one out; the
/// raid ends with it carried and the carry home is Collect's. Before the
/// step in, a raid that stopped 2 m off (`reach_m`) pressed use at the
/// box's middle from 2.0 m and more, past the 2 m ray.
#[test]
fn a_raid_that_gets_there_steals() {
    let world = soak_world();
    let mut tuning = bench_tuning();
    tuning.cheats.batteries = true;
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x2A1D,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 5, None, None);
    for battery in 0..3 {
        game.batteries[battery] = Lying::Home(1);
    }
    game.points = [0, 3];
    let start = Vec3::new(44.0, 0.0, 34.0);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    // Dead, and not back within the run.
    game.bodies[1].alive = false;
    game.bodies[1].dead_at = 1_000_000;
    bots.entered(0, true, 0);
    bots.entered(1, true, 0);
    // A sneak (play 4) would take the same window: just ran.
    if let Some(bot) = bots.bots[0].as_mut() {
        bot.last_ran.push((super::plays::PlayKind::Sneak, 0));
    }
    let (mut clock, mut next_world) = (0, 1_000);
    let mut stole: Option<(u32, Option<super::plays::PlayKind>)> = None;
    while clock < 40_000 && stole.is_none() {
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
        if game.thieves.contains(&0) {
            let play = bots.bots[0]
                .as_ref()
                .and_then(|bot| bot.assignment.as_ref())
                .map(|a| a.kind);
            stole = Some((clock, play));
        }
    }
    let (at, play) = stole.unwrap_or_else(|| {
        panic!(
            "no steal: at {:?}, {:.1} m from their container, batteries {:?}",
            game.bodies[0].feet,
            game.bodies[0].feet.distance_xz(game.containers[1].1),
            game.batteries
        )
    });
    assert_eq!(play, Some(super::plays::PlayKind::Raid), "stolen at {at} ms");
    assert_eq!(game.points, [0, 2]);
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
    // The scenario needs the thief to take the battery before he dies: the
    // fixed bursts of before the bursts by distance (2026-10-23), with
    // which the fight goes that way; with the table's taps the bot's
    // random sequence differs and it kills him at 2.65 s, by the container.
    tuning.aim.combat.fire.rifle = vec![];
    // Likewise the shots before the sights are up (`aim_in_first` off):
    // with the aim-in first it kills him at 2.95 s, before the take.
    tuning.aim.combat.aim_in_first = false;
    // And the aim led to the input's tick (2026-10-24): with it the bot
    // kills him at 2.6 s, before the take.
    tuning.aim.combat.lead_target = false;
    tuning.aim.combat.lead_own = false;
    tuning.aim.combat.turn_feed = false;
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

/// The real-map bench's events (`SRP_BENCH_EVENTS=<dir>`): the run's notes,
/// its kills (shooter, killed) and each bot's place and play once a world,
/// one JSON line each, in `<dir>/<level>-<mode>-<n>v<n>.jsonl`. What the
/// plays' churn, the idle bots with a goal and the trades are counted from
/// (`player_bots_behaviour.md` §3 "Play choice and trades").
struct BenchEvents(Option<std::io::BufWriter<std::fs::File>>);

impl BenchEvents {
    fn open(level: u8, mode: u8, per_team: usize) -> Self {
        Self(std::env::var_os("SRP_BENCH_EVENTS").map(|dir| {
            let path = std::path::Path::new(&dir)
                .join(format!("level_{level:02}-{mode}-{per_team}v{per_team}.jsonl"));
            std::io::BufWriter::new(std::fs::File::create(path).unwrap())
        }))
    }

    fn line(&mut self, value: serde_json::Value) {
        use std::io::Write;
        if let Some(file) = self.0.as_mut() {
            writeln!(file, "{value}").unwrap();
        }
    }

    fn note(&mut self, note: &Note) {
        if self.0.is_some() {
            self.line(serde_json::json!({"t": note.time, "bot": note.place, "note": note.text}));
        }
    }

    /// The kills since the last call, and each bot as it is now: [place,
    /// team, alive, x, z, yaw, play, aiming at someone, walking to a goal].
    fn world(&mut self, clock: u32, game: &mut Game, bots: &Bots) {
        let kills = std::mem::take(&mut game.kill_log);
        if self.0.is_none() {
            return;
        }
        for (tick, shooter, killed) in kills {
            self.line(serde_json::json!({"t": tick, "kill": [shooter, killed]}));
        }
        let each: Vec<serde_json::Value> = bots
            .bots
            .iter()
            .flatten()
            .map(|bot| {
                let body = &game.bodies[bot.place];
                let goal = matches!(bot.intent.go, Go::To { at, within_m } if body.feet.distance_xz(at) > within_m);
                serde_json::json!([
                    bot.place,
                    bot.team,
                    body.alive,
                    (body.feet.x * 10.0).round() / 10.0,
                    (body.feet.z * 10.0).round() / 10.0,
                    (body.yaw * 100.0).round() / 100.0,
                    bot.assignment.as_ref().map(|a| format!("{:?}", a.kind)),
                    bot.reflex.target().is_some(),
                    goal,
                ])
            })
            .collect();
        self.line(serde_json::json!({"t": clock, "w": each}));
    }
}

/// A spell of a bot holding fire for a teammate in its line
/// ([`super::friends::Held`]), from its first tick to its last.
#[derive(Clone, Debug)]
struct Spell {
    t0: u32,
    who: Place,
    target: Option<Place>,
    ticks: u32,
    /// Ticks it ran, it was ready to fire but for him, and it and he
    /// moved across the line the same way and opposite ways (both over
    /// [`BlockTally::SIDE_MPS`]).
    run: u32,
    ready: u32,
    same: u32,
    against: u32,
    /// Ticks it held a side key, and he moved across the line (over
    /// [`BlockTally::SIDE_MPS`]).
    stepping: u32,
    his: u32,
    /// At its start: how far along the way to the target he stood, %
    /// (100 at the target).
    along: u32,
    /// At its start: its play, his play, and whether he had a target.
    plays: (String, String, bool),
    /// Its target killed meanwhile: by itself, by a teammate.
    target_killed: Option<bool>,
}

/// One ended spell: ms, running ms, ready ms, same-way ms, its end.
type SpellDone = (u32, u32, u32, u32, &'static str);

/// The real-map bench's blocked spells: every bot's, from the fire step's
/// [`super::friends::Held`], a game tick at a time; each spell as it ends
/// goes to the events (`"spell": [bot, blocker, ms, running ms, ready ms,
/// same-way ms, opposite-way ms, end, side-key ms, blocker-across ms,
/// along %, shooter's play, blocker's play, blocker aiming]`). And what
/// the blockers' own work came to meanwhile, so that a way made for the
/// shooter is seen to cost them nothing (`player_bots_behaviour.md` §2.6):
/// their shots and hits while they block, the time from a
/// new target to the first shot at it, their headway to their goal, and
/// the carriers' times from the take to the put-in (the events'
/// `"mission"` line at the run's end).
#[derive(Default)]
struct BlockTally {
    spells: Vec<Option<Spell>>,
    feet: Vec<Option<Vec3>>,
    kills_seen: usize,
    done: Vec<SpellDone>,
    /// Bot ticks alive.
    alive_ticks: u64,
    /// By place: the game's shots and hits so far; the target and since
    /// when it had it with no shot yet (and whether it blocked someone
    /// meanwhile); the goal's distance a tick ago; the carry's start.
    shots_hits: Vec<(u32, u32)>,
    aiming: Vec<Option<(Place, u32, bool)>>,
    goal_m: Vec<Option<f32>>,
    carry_from: Vec<Option<u32>>,
    /// [not blocking, blocking]: shots, hits; times from a new target to
    /// the first shot, ms; ticks with a goal, and the headway made, m.
    shots: [u32; 2],
    hits: [u32; 2],
    to_fire: [Vec<u32>; 2],
    goal_ticks: [u64; 2],
    headway_m: [f32; 2],
    /// Carries put in: from the take, ms.
    carries: Vec<u32>,
    /// Team hits: in all; the teammate check on the true bodies along the
    /// shot would have held it; on the shooter's reckoning of both. Each
    /// goes to the events (`"teamhit": [shooter, victim, held on the true
    /// bodies, held on the reckoned ones, the shooter's and the victim's
    /// reckoning off (m), the victim's world's age (ms), apart (m), off the
    /// shooter's facing (deg), the target's distance (m), shooter and
    /// victim crouched, the gun's category, held on the view's line before
    /// the spread, the cone (deg), the spread's turn (deg), aim held,
    /// moving]`). The stand-in game has no body collisions: most of its
    /// team hits are bodies standing in one another (under 0.3 m apart),
    /// which the game does not have.
    team_hits: [u32; 3],
    /// [measure] By place: the view's episode on a visible enemy.
    onview: Vec<Option<OnView>>,
    /// [measure] The on-view ticks with no shot by the fire gates.
    gates: fight_tally::GateTally,
}

/// [measure] A bot free to fire with its view within 4 deg of a visible
/// enemy within 40 m (the Cologne report's episodes, per tick).
#[derive(Clone, Copy)]
struct OnView {
    enemy: Place,
    start: u32,
    last: u32,
    ticks: u32,
    fired: Option<u32>,
    held: u32,
    false_held: u32,
    new_held: u32,
    near_line: u32,
    distance: f32,
}

fn measure_place_view(game: &Game, place: Place) -> PlaceView {
    let body = &game.bodies[place];
    PlaceView {
        place,
        team: body.team,
        bot: true,
        present: true,
        alive: true,
        feet: body.feet,
        yaw: body.yaw,
        pitch: body.pitch,
        velocity: [0.0, 0.0],
        keys: body.keys,
        health: body.health,
        spawned: body.spawned,
        kills: 0,
        deaths: 0,
        streak: 0,
        carrying: None,
        seen_at: 0,
        known_at: 0,
    }
}

fn measure_view(game: &Game, places: Vec<PlaceView>) -> MatchView {
    MatchView {
        clock: game.tick,
        mode: super::view::Mode::TeamDeathmatch,
        time_left_s: 600.0,
        total_s: 900.0,
        points: [0, 0],
        places,
        batteries: Vec::new(),
        marks: Vec::new(),
        containers: Vec::new(),
        sounds: Vec::new(),
        deaths: Vec::new(),
        world_time: 0,
        can_respawn: [true, true],
        next_battery: None,
    }
}

impl BlockTally {
    /// [measure] With `SRP_BENCH_GATES` set, every fire gate is tested each
    /// tick (`Reflex::gate_tally`: `clear_shot`'s rays though an earlier
    /// gate failed; no shot changes), for the gate tally's "every" counts.
    fn gate_tally(bots: &mut Bots) {
        static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *ON.get_or_init(|| std::env::var_os("SRP_BENCH_GATES").is_some()) {
            for bot in bots.bots.iter_mut().flatten() {
                bot.reflex.gate_tally = true;
            }
        }
    }

    /// [measure] The view-on-enemy episodes, a tick at a time.
    fn onview_tick(&mut self, game: &Game, bots: &Bots, bot: &Bot, fired: bool, events: &mut BenchEvents) {
        let place = bot.place;
        self.onview.resize(game.bodies.len(), None);
        let me = &game.bodies[place];
        let eye = Game::eye(me);
        let [fx, fz] = forward(me.yaw);
        let pitch = me.recoil.view(me.pitch);
        let view_dir = Vec3::new(fx * pitch.cos(), pitch.sin(), fz * pitch.cos());
        let free = me.alive && me.carrying.is_none() && bot.intent.fire;
        let seen = if free {
            game.bodies
                .iter()
                .enumerate()
                .filter(|(e, b)| {
                    *e != place && b.alive && b.team != me.team && b.feet.distance_xz(me.feet) <= 40.0
                })
                .filter_map(|(e, b)| {
                    let to = Game::chest(b) - eye;
                    let off = (to.dot(view_dir) / to.length().max(0.01)).clamp(-1.0, 1.0).acos();
                    (off <= 4f32.to_radians()).then_some((e, off))
                })
                .filter(|&(e, _)| {
                    let b = &game.bodies[e];
                    super::aimpoint::shows(eye, b.feet, b.keys & key::CROUCH != 0, &game.world.sight, || true)
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(e, _)| e)
        } else {
            None
        };
        let now = game.tick;
        let mut ep = self.onview[place].take();
        if let Some(e) = ep
            && (!me.alive || (seen != Some(e.enemy) && now > e.last + 2 * TICK_MS))
        {
            Self::end_onview(place, e, events);
            ep = None;
        }
        if let Some(enemy) = seen {
            let e = ep.get_or_insert(OnView {
                enemy,
                start: now,
                last: now,
                ticks: 0,
                fired: None,
                held: 0,
                false_held: 0,
                new_held: 0,
                near_line: 0,
                distance: game.bodies[enemy].feet.distance_xz(me.feet),
            });
            if e.enemy == enemy {
                e.last = now;
            }
        }
        if let Some(e) = ep.as_mut() {
            e.ticks += 1;
            if !fired {
                self.gates.tick(bot.reflex.gates(), e.enemy, now);
            }
            if fired && e.fired.is_none() {
                e.fired = Some(now - e.start);
            }
            let target = &game.bodies[e.enemy];
            let to = Game::chest(target) - eye;
            let distance = target.feet.distance_xz(me.feet);
            if let Some(held) = bot.reflex.held_for() {
                e.held += 1;
                let t = bots.tuning.aim.friends;
                let base = super::friends::FriendlyFireTuning {
                    touch_m: 0.0,
                    past_target_m: 2.0,
                    ..t
                };
                let dirs = [view_dir, to * (1.0 / to.length().max(0.01))];
                let view = measure_view(
                    game,
                    vec![measure_place_view(game, place), measure_place_view(game, held.who)],
                );
                let on = |t: &super::friends::FriendlyFireTuning| {
                    super::friends::blocking(
                        &measure_place_view(game, place),
                        &view,
                        &dirs,
                        distance,
                        t.spread(me.gun),
                        0.0,
                        t,
                    )
                    .is_some_and(|b| b.who == held.who)
                };
                let truly = on(&t);
                e.false_held += u32::from(!truly);
                e.new_held += u32::from(truly && !on(&base));
            }
            // A teammate within 1.2 m of the line in plan, between the eye
            // and the enemy (the report's measure).
            let (lx, lz) = (to.x, to.z);
            let len = lx.hypot(lz).max(0.01);
            let near = game.bodies.iter().enumerate().any(|(p, b)| {
                if p == place || !b.alive || b.team != me.team {
                    return false;
                }
                let (dx, dz) = (b.feet.x - eye.x, b.feet.z - eye.z);
                let s = (dx * lx + dz * lz) / len;
                s > 0.0 && s < len && ((dx * lz - dz * lx) / len).abs() <= 1.2
            });
            e.near_line += u32::from(near);
        }
        self.onview[place] = ep;
    }

    fn end_onview(place: Place, e: OnView, events: &mut BenchEvents) {
        if e.last < e.start + 9 * TICK_MS {
            return;
        }
        events.line(serde_json::json!({
            "t": e.start,
            "onview": [place, e.enemy, e.last - e.start + TICK_MS, e.fired, e.ticks, e.held,
                e.false_held, e.new_held, e.near_line, e.distance],
        }));
    }

    /// Sideways speed counted as moving across the line, m/s.
    const SIDE_MPS: f32 = 0.5;

    /// After a game step.
    fn tick(&mut self, game: &Game, bots: &Bots, events: &mut BenchEvents) {
        self.team_hit_tick(game, bots, events);
        let n = game.bodies.len();
        self.spells.resize(n, None);
        self.shots_hits.resize(n, (0, 0));
        self.aiming.resize(n, None);
        self.goal_m.resize(n, None);
        self.carry_from.resize(n, None);
        let moved: Vec<Option<Vec3>> = (0..n)
            .map(|p| {
                let body = &game.bodies[p];
                let before = self.feet.get(p).copied().flatten();
                before.filter(|_| body.alive).map(|b| body.feet - b)
            })
            .collect();
        self.feet = game
            .bodies
            .iter()
            .map(|b| b.alive.then_some(b.feet))
            .collect();
        if game.kill_log.len() < self.kills_seen {
            self.kills_seen = 0;
        }
        for &(_, shooter, victim) in &game.kill_log[self.kills_seen..] {
            for (place, spell) in self.spells.iter_mut().enumerate() {
                if let Some(spell) = spell.as_mut()
                    && spell.target == Some(victim)
                {
                    spell.target_killed = Some(shooter == place);
                }
            }
        }
        self.kills_seen = game.kill_log.len();
        // The blockers now: teammates some bot holds fire for.
        let mut blocking = vec![false; n];
        for spell in self.spells.iter().flatten() {
            blocking[spell.who] = true;
        }
        for bot in bots.bots.iter().flatten() {
            let place = bot.place;
            let body = &game.bodies[place];
            let k = usize::from(blocking[place]);
            let (s, h) = game.shots_hits[place];
            let (s0, h0) = std::mem::replace(&mut self.shots_hits[place], (s, h));
            self.shots[k] += s - s0;
            self.hits[k] += h - h0;
            self.onview_tick(game, bots, bot, s > s0, events);
            // From a new target to the first shot at it.
            let target = bot.reflex.target().filter(|_| body.alive);
            self.aiming[place] = match (target, self.aiming[place]) {
                (Some(t), Some((was, since, blocked))) if t == was => {
                    let blocked = blocked || k == 1;
                    if s > s0 {
                        self.to_fire[usize::from(blocked)].push(game.tick - since);
                        None
                    } else {
                        Some((was, since, blocked))
                    }
                }
                // A new target with no shot this tick (a target taken while
                // firing is not timed).
                (Some(t), None) if s == s0 => Some((t, game.tick, k == 1)),
                (Some(t), Some(_)) if s == s0 => Some((t, game.tick, k == 1)),
                _ => None,
            };
            // Headway to the goal.
            let goal = match bot.intent.go {
                Go::To { at, within_m } if body.alive && body.feet.distance_xz(at) > within_m => {
                    Some(body.feet.distance_xz(at))
                }
                _ => None,
            };
            if let (Some(now), Some(before)) = (goal, self.goal_m[place]) {
                self.goal_ticks[k] += 1;
                self.headway_m[k] += (before - now).clamp(-1.0, 1.0);
            }
            self.goal_m[place] = goal;
            if body.alive {
                self.alive_ticks += 1;
            }
            self.spell_tick(game, bots, bot, &moved, events);
        }
        // Carries: from the take to the put-in.
        for (place, body) in game.bodies.iter().enumerate() {
            match (body.carrying.is_some(), self.carry_from[place]) {
                (true, None) => self.carry_from[place] = Some(game.tick),
                (false, Some(from)) => {
                    if game.put_in.contains(&place) {
                        self.carries.push(game.tick - from);
                    }
                    self.carry_from[place] = None;
                }
                _ => {}
            }
        }
    }

    /// Each team hit of the step: would the teammate check have held the
    /// shot on the true bodies (the shot's own eye and line), and on the
    /// bodies as the shooter reckoned them; how far the reckoning was off.
    fn team_hit_tick(&mut self, game: &Game, bots: &Bots, events: &mut BenchEvents) {
        let t = super::friends::FriendlyFireTuning::default();
        let place_view = |place: Place, feet: Vec3, yaw: f32, pitch: f32| {
            let body = &game.bodies[place];
            PlaceView {
                place,
                team: body.team,
                bot: true,
                present: true,
                alive: true,
                feet,
                yaw,
                pitch,
                velocity: [0.0, 0.0],
                keys: body.keys,
                health: body.health,
                spawned: body.spawned,
                kills: 0,
                deaths: 0,
                streak: 0,
                carrying: None,
                seen_at: 0,
                known_at: 0,
            }
        };
        let view = |places: Vec<PlaceView>| MatchView {
            clock: game.tick,
            mode: super::view::Mode::TeamDeathmatch,
            time_left_s: 600.0,
            total_s: 900.0,
            points: [0, 0],
            places,
            batteries: Vec::new(),
            marks: Vec::new(),
            containers: Vec::new(),
            sounds: Vec::new(),
            deaths: Vec::new(),
            world_time: 0,
            can_respawn: [true, true],
            next_battery: None,
        };
        for &(shooter, victim, _eye, dir, cone, view_dir) in &game.team_hit_shots {
            let (a, b) = (&game.bodies[shooter], &game.bodies[victim]);
            let distance = a
                .target
                .map_or(60.0, |target| game.bodies[target].feet.distance_xz(a.feet));
            let spread = t.spread(a.gun);
            let held_on = |me: PlaceView, him: PlaceView, dir: Vec3| {
                super::friends::blocking(&me, &view(vec![me.clone(), him]), &[dir], distance, spread, 0.0, &t)
                    .is_some_and(|blocked| blocked.who == victim)
            };
            let held = |me: PlaceView, him: PlaceView| held_on(me, him, dir);
            let on_view = held_on(
                place_view(shooter, a.feet, a.yaw, a.pitch),
                place_view(victim, b.feet, b.yaw, b.pitch),
                view_dir,
            );
            let deflected = view_dir.dot(dir).clamp(-1.0, 1.0).acos().to_degrees();
            let truly = held(
                place_view(shooter, a.feet, a.yaw, a.pitch),
                place_view(victim, b.feet, b.yaw, b.pitch),
            );
            let estimate = |place: Place| bots.tracker.body(place).and_then(|body| body.estimate()).copied();
            let (ea, eb) = (estimate(shooter), estimate(victim));
            let reckoned = match (ea, eb) {
                (Some(ea), Some(eb)) => held(
                    place_view(shooter, ea.feet, ea.yaw, ea.pitch),
                    place_view(victim, eb.feet, eb.yaw, eb.pitch),
                ),
                _ => false,
            };
            let off = |e: Option<super::body::Estimate>, body: &Body| e.map_or(-1.0, |e| e.feet.distance_xz(body.feet));
            let fix_age = bots
                .tracker
                .body(victim)
                .and_then(|body| body.fix())
                .map_or(0, |fix| game.tick.saturating_sub(fix.time));
            let to = b.feet - a.feet;
            let [fx, fz] = forward(a.yaw);
            let off_deg = (fx * to.z - fz * to.x).atan2(fx * to.x + fz * to.z).abs().to_degrees();
            self.team_hits[0] += 1;
            self.team_hits[1] += u32::from(truly);
            self.team_hits[2] += u32::from(reckoned);
            events.line(serde_json::json!({
                "t": game.tick,
                "teamhit": [shooter, victim, truly, reckoned, off(ea, a), off(eb, b), fix_age,
                    a.feet.distance_xz(b.feet), off_deg, distance,
                    a.keys & key::CROUCH != 0, b.keys & key::CROUCH != 0, a.gun.category,
                    on_view, cone, deflected, a.keys & key::AIM != 0, bench_spread::moving(a.keys)],
            }));
        }
    }

    fn spell_tick(
        &mut self,
        game: &Game,
        bots: &Bots,
        bot: &Bot,
        moved: &[Option<Vec3>],
        events: &mut BenchEvents,
    ) {
        let place = bot.place;
        let body = &game.bodies[place];
        let Some(held) = bot.reflex.held_for().filter(|_| body.alive) else {
            if let Some(spell) = self.spells[place].take() {
                let end = if !body.alive {
                    "died"
                } else {
                    match spell.target_killed {
                        Some(true) => "target killed by it",
                        Some(false) => "target killed by another",
                        None if bot.reflex.target() != spell.target => "target lost",
                        None => "cleared",
                    }
                };
                self.end(place, spell, end, events);
            }
            return;
        };
        let spell = self.spells[place].get_or_insert_with(|| Spell {
            t0: game.tick,
            who: held.who,
            target: bot.reflex.target(),
            ticks: 0,
            run: 0,
            ready: 0,
            same: 0,
            against: 0,
            stepping: 0,
            his: 0,
            along: bot.reflex.target().map_or(0, |t| {
                let to = game.bodies[t].feet.distance_xz(body.feet).max(0.1);
                (100.0 * game.bodies[held.who].feet.distance_xz(body.feet) / to) as u32
            }),
            plays: {
                let play = |p: Place| {
                    bots.bots[p]
                        .as_ref()
                        .and_then(|b| b.assignment.as_ref())
                        .map_or("none".to_owned(), |a| format!("{:?}", a.kind))
                };
                let aiming = bots.bots[held.who]
                    .as_ref()
                    .is_some_and(|b| b.reflex.target().is_some());
                (play(place), play(held.who), aiming)
            },
            target_killed: None,
        });
        spell.who = held.who;
        spell.ticks += 1;
        spell.run += u32::from(held.running);
        spell.ready += u32::from(held.ready);
        spell.stepping += u32::from(body.keys & (key::LEFT | key::RIGHT) != 0);
        let across = game.bodies[held.who].feet - body.feet;
        let side = Vec3::new(-across.z, 0.0, across.x);
        let len = side.length();
        if let (Some(a), Some(b)) = (moved[place], moved[held.who])
            && len > 0.01
        {
            let dt = TICK_MS as f32 / 1000.0;
            let (va, vb) = (a.dot(side) / len / dt, b.dot(side) / len / dt);
            spell.his += u32::from(vb.abs() > Self::SIDE_MPS);
            if va.abs() > Self::SIDE_MPS && vb.abs() > Self::SIDE_MPS {
                if va.signum() == vb.signum() {
                    spell.same += 1;
                } else {
                    spell.against += 1;
                }
            }
        }
    }

    fn end(&mut self, place: Place, spell: Spell, end: &'static str, events: &mut BenchEvents) {
        let ms = |ticks: u32| ticks * TICK_MS;
        events.line(serde_json::json!({
            "t": spell.t0,
            "spell": [place, spell.who, ms(spell.ticks), ms(spell.run), ms(spell.ready),
                ms(spell.same), ms(spell.against), end, ms(spell.stepping), ms(spell.his), spell.along,
                spell.plays.0, spell.plays.1, spell.plays.2],
        }));
        self.done.push((
            ms(spell.ticks),
            ms(spell.run),
            ms(spell.ready),
            ms(spell.same),
            end,
        ));
    }

    /// The spells under way at the end, and one line on all of them.
    fn finish(&mut self, events: &mut BenchEvents) -> String {
        for place in 0..self.onview.len() {
            if let Some(e) = self.onview[place].take() {
                Self::end_onview(place, e, events);
            }
        }
        for place in 0..self.spells.len() {
            if let Some(spell) = self.spells[place].take() {
                self.end(place, spell, "the run's end", events);
            }
        }
        events.line(serde_json::json!({
            "mission": {
                "shots": self.shots, "hits": self.hits,
                "to_fire_ms": [&self.to_fire[0], &self.to_fire[1]],
                "goal_ticks": self.goal_ticks, "headway_m": self.headway_m,
                "carries_ms": &self.carries,
                "team_hits": self.team_hits,
            }
        }));
        let mut ms: Vec<u32> = self.done.iter().map(|d| d.0).collect();
        ms.sort_unstable();
        let at = |q: f32| {
            ms.get(((ms.len() as f32 * q) as usize).min(ms.len().saturating_sub(1)))
                .copied()
                .unwrap_or(0)
        };
        let total: u64 = ms.iter().map(|&m| u64::from(m)).sum();
        let share = |f: fn(&SpellDone) -> u32| {
            let part: u64 = self.done.iter().map(|d| u64::from(f(d))).sum();
            100.0 * part as f32 / total.max(1) as f32
        };
        let mut ends: std::collections::BTreeMap<&str, u32> = Default::default();
        for d in &self.done {
            *ends.entry(d.4).or_default() += 1;
        }
        let bot_min = self.alive_ticks as f32 * TICK_MS as f32 / 60_000.0;
        format!(
            "blocked: {} spells, {:.1} s in all ({:.2} s a bot-minute alive), over 1 s {}; median {} ms, 90th {} ms, longest {} ms; running {:.0} %, ready to fire {:.0} %, moving the same way as the blocker {:.0} %; ends {ends:?}; blockers hit {} of {} shots",
            ms.len(),
            total as f32 / 1000.0,
            total as f32 / 1000.0 / bot_min.max(1e-3),
            ms.iter().filter(|&&m| m > 1000).count(),
            at(0.5),
            at(0.9),
            ms.last().copied().unwrap_or(0),
            share(|d| d.1),
            share(|d| d.2),
            share(|d| d.3),
            self.hits[1],
            self.shots[1],
        )
    }
}

/// A bot hit by an enemy, followed for [`HitTally::FOLLOW_MS`].
#[derive(Clone, Debug)]
struct HitFollow {
    t0: u32,
    shooter: Place,
    /// It knew of him at the hit: its target, or a track of him sensed
    /// within [`HitTally::SENSED_MS`].
    sensed: bool,
    /// Its view off the way to him at the hit, degrees, and his distance.
    off_deg: f32,
    distance: f32,
    /// Ms from the hit: its view within [`HitTally::FACED_DEG`] of him,
    /// him its target, its first hit on him.
    faced: Option<u32>,
    targeted: Option<u32>,
    hit_back: Option<u32>,
    died: bool,
    killed_him: bool,
}

/// The real-map bench's hits taken (audit issue 8, `hit_bearing`): each
/// hit on a living bot that it had no follow of already starts one, and
/// what it did in the next [`HitTally::FOLLOW_MS`]: turned to the
/// shooter, took him as its target, hit him back, killed him, died. Split
/// by whether the bot sensed him at the hit. Each follow goes to the
/// events as a `"hit"` line: [bot, shooter, sensed, off degrees,
/// distance, faced ms, targeted ms, hit back ms, died, killed him].
#[derive(Default)]
struct HitTally {
    follows: Vec<Option<HitFollow>>,
    /// [not sensed, sensed]: follows; faced within 1 s; targeted within
    /// 1.5 s; hit back; died; killed him.
    counts: [[u32; 6]; 2],
}

impl HitTally {
    const FOLLOW_MS: u32 = 3_000;
    const SENSED_MS: u32 = 1_500;
    const FACED_DEG: f32 = 20.0;

    /// How far a view at `yaw` from `from` is off the way to `to`, degrees.
    fn off(yaw: f32, from: Vec3, to: Vec3) -> f32 {
        use super::kin::{wrap, yaw_of};
        wrap(yaw - yaw_of([to.x - from.x, to.z - from.z])).abs().to_degrees()
    }

    /// After a game step: takes the game's hits.
    fn tick(&mut self, game: &mut Game, bots: &Bots, events: &mut BenchEvents) {
        let tick = game.tick;
        let n = game.bodies.len();
        self.follows.resize(n, None);
        let hits = std::mem::take(&mut game.hit_log);
        for &(at, shooter, victim) in &hits {
            // Its hit back (and the kill, a hit that left him dead).
            if let Some(follow) = self.follows[shooter].as_mut()
                && follow.shooter == victim
            {
                follow.hit_back.get_or_insert(at - follow.t0);
                let him = &game.bodies[victim];
                if !him.alive && him.dead_at == at {
                    follow.killed_him = true;
                }
            }
            let body = &game.bodies[victim];
            if !body.alive && body.dead_at == at {
                Self::mates(game, bots, at, shooter, victim, events);
            }
            let Some(bot) = bots.bots[victim].as_ref() else {
                continue;
            };
            if !body.alive || self.follows[victim].is_some() {
                continue;
            }
            let sensed = bot.reflex.target() == Some(shooter)
                || bot
                    .memory
                    .tracks()
                    .iter()
                    .any(|t| t.who == shooter && t.time + Self::SENSED_MS >= at);
            let to = game.bodies[shooter].feet;
            self.follows[victim] = Some(HitFollow {
                t0: at,
                shooter,
                sensed,
                off_deg: Self::off(body.yaw, body.feet, to),
                distance: body.feet.distance_xz(to),
                faced: None,
                targeted: None,
                hit_back: None,
                died: false,
                killed_him: false,
            });
        }
        for place in 0..n {
            let Some(follow) = self.follows[place].as_mut() else {
                continue;
            };
            let body = &game.bodies[place];
            let since = tick.saturating_sub(follow.t0);
            if body.alive {
                let to = game.bodies[follow.shooter].feet;
                if follow.faced.is_none()
                    && Self::off(body.yaw, body.feet, to) <= Self::FACED_DEG
                {
                    follow.faced = Some(since);
                }
                if follow.targeted.is_none()
                    && bots.bots[place]
                        .as_ref()
                        .is_some_and(|b| b.reflex.target() == Some(follow.shooter))
                {
                    follow.targeted = Some(since);
                }
            } else {
                follow.died = true;
            }
            if !body.alive || since >= Self::FOLLOW_MS || follow.killed_him {
                let follow = self.follows[place].take().unwrap();
                self.end(place, follow, events);
            }
        }
    }

    /// At a kill: the victim's living teammates within 30 m, as a
    /// `"mates"` line: [victim, killer, [[mate, metres from the victim,
    /// metres from the killer, a line from its eye to his chest, it knew of
    /// him (a track within [`Self::SENSED_MS`]), him its target], ...]].
    fn mates(
        game: &Game,
        bots: &Bots,
        at: u32,
        killer: Place,
        victim: Place,
        events: &mut BenchEvents,
    ) {
        if events.0.is_none() {
            return;
        }
        let dead = &game.bodies[victim];
        let him = &game.bodies[killer];
        let mates: Vec<serde_json::Value> = bots
            .bots
            .iter()
            .flatten()
            .filter(|b| b.place != victim && b.team == dead.team)
            .filter(|b| game.bodies[b.place].alive)
            .filter(|b| game.bodies[b.place].feet.distance_xz(dead.feet) <= 30.0)
            .map(|b| {
                let body = &game.bodies[b.place];
                let line = game
                    .world
                    .sight
                    .visible(Game::eye(body), Game::chest(him));
                let knew = b
                    .memory
                    .tracks()
                    .iter()
                    .any(|t| t.who == killer && t.time + Self::SENSED_MS >= at);
                serde_json::json!([
                    b.place,
                    (body.feet.distance_xz(dead.feet) * 10.0).round() / 10.0,
                    (body.feet.distance_xz(him.feet) * 10.0).round() / 10.0,
                    line,
                    knew,
                    b.reflex.target() == Some(killer),
                ])
            })
            .collect();
        events.line(serde_json::json!({"t": at, "mates": [victim, killer, mates]}));
    }

    fn end(&mut self, place: Place, f: HitFollow, events: &mut BenchEvents) {
        events.line(serde_json::json!({
            "t": f.t0,
            "hit": [place, f.shooter, f.sensed, f.off_deg.round(), (f.distance * 10.0).round() / 10.0,
                f.faced, f.targeted, f.hit_back, f.died, f.killed_him],
        }));
        let row = &mut self.counts[usize::from(f.sensed)];
        row[0] += 1;
        row[1] += u32::from(f.faced.is_some_and(|ms| ms <= 1_000));
        row[2] += u32::from(f.targeted.is_some_and(|ms| ms <= 1_500));
        row[3] += u32::from(f.hit_back.is_some());
        row[4] += u32::from(f.died);
        row[5] += u32::from(f.killed_him);
    }

    /// The follows under way at the end, and one line on all of them.
    fn finish(&mut self, events: &mut BenchEvents) -> String {
        for place in 0..self.follows.len() {
            if let Some(follow) = self.follows[place].take() {
                self.end(place, follow, events);
            }
        }
        let line = |row: &[u32; 6]| {
            let pc = |k: usize| 100.0 * row[k] as f32 / row[0].max(1) as f32;
            format!(
                "{} hits; faced him within 1 s {:.0} %, took him as target within 1.5 s {:.0} %, hit him back {:.0} %, died {:.0} %, killed him {:.0} %",
                row[0],
                pc(1),
                pc(2),
                pc(3),
                pc(4),
                pc(5)
            )
        };
        format!(
            "hits taken: shooter not sensed: {}; sensed: {}",
            line(&self.counts[0]),
            line(&self.counts[1])
        )
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
    bench_tuning_of(false)
}

/// `SRP_BENCH_PING=<lo>-<hi>` (or one number), ms: a ping for each of
/// `places`, drawn evenly between the two by `seed`.
fn bench_pings(places: usize, seed: u32) -> Option<Vec<u32>> {
    let text = std::env::var("SRP_BENCH_PING").ok().filter(|t| !t.is_empty())?;
    let (lo, hi) = text.split_once('-').unwrap_or((&text, &text));
    let (lo, hi): (f32, f32) = (
        lo.trim().parse().expect("SRP_BENCH_PING: ms"),
        hi.trim().parse().expect("SRP_BENCH_PING: ms"),
    );
    let mut rng = Rng::new(0x9146 + u64::from(seed));
    Some((0..places).map(|_| rng.range(lo, hi.max(lo)) as u32).collect())
}

/// [`bench_tuning`]; for `side1`, the plays', the reflex's and the senses'
/// numbers from `SRP_BENCH_SIDE1_<name>` instead of `SRP_BENCH_<name>`
/// (the one-sided runs, `SRP_BENCH_ONE_SIDE`: team 1 plays by it).
fn bench_tuning_of(side1: bool) -> Tuning {
    let var = |name: &str| {
        let prefix = if side1 { "SRP_BENCH_SIDE1_" } else { "SRP_BENCH_" };
        std::env::var(format!("{prefix}{name}"))
    };
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
    if let Some(m) = std::env::var("SRP_BENCH_FRIEND_NEAR_M")
        .ok()
        .and_then(|m| m.parse().ok())
    {
        tuning.aim.friends.near_m = m;
    }
    // The teammates led to the tick decided (`FriendlyFireTuning::lead`):
    // 0 off, 1 on; and the lead's extra, ms.
    if let Ok(v) = std::env::var("SRP_BENCH_FF_LEAD") {
        tuning.aim.friends.lead = v != "0";
    }
    // Teammates this far past the target count
    // (`FriendlyFireTuning::past_target_m`).
    if let Some(m) = std::env::var("SRP_BENCH_FF_PAST_M")
        .ok()
        .and_then(|m| m.parse().ok())
    {
        tuning.aim.friends.past_target_m = m;
    }
    // A teammate this near in front blocks whatever his angle
    // (`FriendlyFireTuning::touch_m`; 0 off).
    if let Some(m) = std::env::var("SRP_BENCH_FF_TOUCH_M")
        .ok()
        .and_then(|m| m.parse().ok())
    {
        tuning.aim.friends.touch_m = m;
    }
    if let Some(ms) = std::env::var("SRP_BENCH_FF_LEAD_MS")
        .ok()
        .and_then(|m| m.parse().ok())
    {
        tuning.aim.friends.lead_extra_ms = ms;
    }
    // Collect's and Intercept's numbers: `name=value,...`.
    for (name, kind) in [
        ("COLLECT", super::plays::PlayKind::Collect),
        ("INTERCEPT", super::plays::PlayKind::Intercept),
        ("RAID", super::plays::PlayKind::Raid),
        ("BUDDY", super::plays::PlayKind::Buddy),
        ("TRADE", super::plays::PlayKind::Trade),
        ("ROAM", super::plays::PlayKind::Roam),
        ("CUT", super::plays::PlayKind::Cut),
        ("HOLDEXIT", super::plays::PlayKind::HoldExit),
        ("CROSSFIRE", super::plays::PlayKind::Crossfire),
        ("PAIRUP", super::plays::PlayKind::PairUp),
    ] {
        let Ok(numbers) = var(name) else {
            continue;
        };
        let play = tuning
            .plays
            .entry(kind)
            .or_insert_with(|| super::plays::default_tuning(kind));
        for pair in numbers.split(',').filter(|p| !p.is_empty()) {
            let (name, value) = pair
                .split_once('=')
                .unwrap_or_else(|| panic!("SRP_BENCH_{name}: name=value"));
            play.numbers
                .insert(name.to_owned(), value.parse().expect("a number"));
        }
    }
    // The reflex's numbers (`CombatTuning`): `name=json,...` (`;` between
    // them when a value has a comma).
    if let Ok(numbers) = var("COMBAT") {
        let mut combat = serde_json::to_value(&tuning.aim.combat).unwrap();
        let sep = if numbers.contains(';') { ';' } else { ',' };
        for pair in numbers.split(sep).filter(|p| !p.is_empty()) {
            let (name, value) = pair.split_once('=').expect("SRP_BENCH_COMBAT: name=value");
            combat[name] = serde_json::from_str(value).expect("SRP_BENCH_COMBAT: a JSON value");
        }
        tuning.aim.combat = serde_json::from_value(combat).expect("SRP_BENCH_COMBAT: known names");
    }
    // The aim's own numbers (`AimTuning`: `range_m`, `fire_within_deg`,
    // `pause_ms`, ...), as `SRP_BENCH_COMBAT`'s.
    if let Ok(numbers) = var("AIM") {
        let mut aim = serde_json::to_value(&tuning.aim).unwrap();
        let sep = if numbers.contains(';') { ';' } else { ',' };
        for pair in numbers.split(sep).filter(|p| !p.is_empty()) {
            let (name, value) = pair.split_once('=').expect("SRP_BENCH_AIM: name=value");
            aim[name] = serde_json::from_str(value).expect("SRP_BENCH_AIM: a JSON value");
        }
        tuning.aim = serde_json::from_value(aim).expect("SRP_BENCH_AIM: known names");
    }
    // The senses' numbers (`SenseTuning`), as `SRP_BENCH_COMBAT`'s.
    if let Ok(numbers) = var("SENSE") {
        let mut sense = serde_json::to_value(&tuning.sense).unwrap();
        for pair in numbers.split(',').filter(|p| !p.is_empty()) {
            let (name, value) = pair.split_once('=').expect("SRP_BENCH_SENSE: name=value");
            sense[name] = serde_json::from_str(value).expect("SRP_BENCH_SENSE: a JSON value");
        }
        tuning.sense = serde_json::from_value(sense).expect("SRP_BENCH_SENSE: known names");
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
            // Every bot of one tier (`SRP_BENCH_TIER=4-5|9-10|13-15`): its
            // kits and its skill.
            if let Ok(tier) = std::env::var("SRP_BENCH_TIER")
                && !tier.is_empty()
            {
                let tier: survarium::v026g::loadouts::BotTier =
                    serde_json::from_value(serde_json::Value::String(tier))
                        .expect("SRP_BENCH_TIER: 4-5, 9-10 or 13-15");
                bots = bots.with_tiers(&vec![Some(tier); teams.len()]);
            }
            // One-sided (`SRP_BENCH_ONE_SIDE`): team 1 by the
            // `SRP_BENCH_SIDE1_*` numbers, so a change given to team 0
            // alone shows in its K/D.
            if std::env::var_os("SRP_BENCH_ONE_SIDE").is_some() {
                bots.side1_tuning = Some(Arc::new(bench_tuning_of(true)));
            }
            // Every body seen as a live human is (`SRP_BENCH_PING=<lo>-<hi>`,
            // ms): known to the clock less a ping drawn for each place, and
            // reckoned on from there (`Tracker::seen_late`). Off, the
            // bench's bots know each other to their decided ticks, up to
            // 100 ms past the clock, which no live human gives them.
            if let Some(pings) = bench_pings(teams.len(), seed) {
                bots.tracker.seen_late = Some(pings);
            }
            let mut fight = fight_tally::FightTally::default();
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
            if let Some(m) = std::env::var("SRP_BENCH_TAKE_M")
                .ok()
                .and_then(|m| m.parse().ok())
            {
                game.take_m = m;
            }
            if let Some(n) = std::env::var("SRP_BENCH_UNTAKEABLE")
                .ok()
                .and_then(|n| n.parse().ok())
            {
                game.untakeable = n;
            }
            game.camp_m = std::env::var("SRP_BENCH_CAMP")
                .ok()
                .and_then(|m| m.parse().ok());
            game.nudge = std::env::var("SRP_BENCH_NUDGE")
                .ok()
                .and_then(|m| m.parse().ok())
                .map(|m| (m, NUDGE_EVERY_MS));
            game.spread = std::env::var_os("SRP_BENCH_SPREAD").is_some();
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
            let (mut take_ends, mut stands_given_up, mut stands_stale) = (0u32, 0u32, 0u32);
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
            // with no route: a bot standing where it means to walk. By team.
            let (mut goal_ticks, mut stood_ticks) = ([0u64; 2], [0u64; 2]);
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
            // Living bots' ticks and deaths by their play and team
            // (`Kind/team`).
            let mut play_ticks: std::collections::BTreeMap<String, u64> = Default::default();
            let mut deaths_by: std::collections::BTreeMap<String, u32> = Default::default();
            let mut carriers_killed = [0u32; 2];
            let mut takes_all = 0u32;
            let mut raids = RaidTally::default();
            let mut events = BenchEvents::open(level, mode, per_team);
            let mut carry = CarryTally::new(format!("{project}-{per_team}v{per_team}"));
            let mut routes = RouteTally::start(game.bodies.len());
            let mut push = PushTally::start(&world, game.bodies.len());
            let mut human = HumanTally::start();
            bots.bench_human = human.as_ref().map(|h| h.place);
            let mut blocks = BlockTally::default();
            let mut hits = HitTally::default();
            while clock < limit_ms {
                clock += 5;
                while game.tick + TICK_MS <= clock {
                    let stats_before = game.stats.clone();
                    game.step();
                    carry.deaths(&stats_before, &game, &bots);
                    BlockTally::gate_tally(&mut bots);
                    blocks.tick(&game, &bots, &mut events);
                    hits.tick(&mut game, &bots, &mut events);
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
                        push.stats(place, teams[place], now.0 - before.0, now.1 - before.1);
                        let side = &mut by_team[usize::from(teams[place])];
                        if now.0 > before.0 {
                            by_play.entry(play_of(place)).or_default()[2] += now.0 - before.0;
                            if attacking(place) {
                                side[3] += u64::from(now.0 - before.0);
                            }
                        }
                        if now.1 > before.1 {
                            by_play.entry(play_of(place)).or_default()[3] += now.1 - before.1;
                            *deaths_by
                                .entry(format!("{}/{}", play_of(place), teams[place]))
                                .or_default() += now.1 - before.1;
                            if attacking(place) {
                                side[4] += u64::from(now.1 - before.1);
                            }
                        }
                    }
                    for fact in std::mem::take(&mut game.shot_facts) {
                        fight.shot(fact);
                    }
                    fight.sample(&game, game.tick);
                    let thieves = std::mem::take(&mut game.thieves);
                    raids.bodies(&game, &thieves, game.tick);
                    routes.tick(&game, &bots, game.tick);
                    push.tick(&bots);
                    if let Some(human) = &mut human {
                        human.tick(&game, &thieves);
                    }
                    for place in thieves {
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
                        if body.alive {
                            let kind = bot
                                .assignment
                                .as_ref()
                                .map_or("none".to_owned(), |a| format!("{:?}", a.kind));
                            *play_ticks.entry(format!("{kind}/{}", bot.team)).or_default() += 1;
                        }
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
                            let team = usize::from(body.team);
                            goal_ticks[team] += 1;
                            // A step walked straight has no route.
                            if bot.route.is_none() && !bot.intent.straight {
                                stood_ticks[team] += 1;
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
                    events.world(clock, &mut game, &bots);
                    let world_tick = clock / TICK_MS * TICK_MS - 300;
                    let hurts = game.hurts_at(world_tick).unwrap_or_default();
                    for (place, hurt) in hurts.into_iter().enumerate() {
                        bots.hurt(place, hurt);
                    }
                    if let Some((_, fixes, health, facts)) = game.world_at(world_tick) {
                        for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                            for note in bots.fixed(place, fix, Some(health)) {
                                routes.note(&note);
                                decisions.note(&note);
                                events.note(&note);
                                carry.note(&note);
                            }
                        }
                        routes.world(&game, &bots);
                        push.world(&game, &bots);
                        for note in bots.facts(facts) {
                            decisions.note(&note);
                            carry.note(&note);
                            if note.text.contains("its thief no longer carrying it") {
                                theft_looks += 1;
                                game.stolen_log.push(format!("{} {}", note.time, note.text));
                            }
                        }
                        carry.world(&game, &bots);
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
                raids.polled(&bots, &notes, &made);
                for note in notes {
                    decisions.note(&note);
                    fight.note(&note);
                    events.note(&note);
                    carry.note(&note);
                    push.note(&note, &bots);
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
                        // Aimed at a belief the battery had left (it lies
                        // over 0.2 m from the spot the note gives).
                        let believed = note
                            .text
                            .split("believed at (")
                            .nth(1)
                            .and_then(|s| s.split(')').next())
                            .map(|s| s.split(", ").filter_map(|v| v.parse::<f32>().ok()).collect::<Vec<_>>());
                        let battery = note
                            .text
                            .strip_prefix("took no battery ")
                            .and_then(|s| s.split(' ').next())
                            .and_then(|s| s.parse::<usize>().ok());
                        if let (Some([x, y, z]), Some(battery)) = (believed.as_deref(), battery)
                            && let Some(Lying::Free(at, _)) = game.batteries.get(battery)
                            && (*at - Vec3::new(*x, *y, *z)).length() > 0.2
                        {
                            stands_stale += 1;
                            if std::env::var("SRP_BENCH_SHOW_STALE").is_ok() {
                                println!("  stale stand at {} ms: lies at {at:?}; {}", note.time, note.text);
                            }
                        }
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
                "  stood with a goal and no route: {} of {} bot ticks ({:.1} %), by team {stood_ticks:?} of {goal_ticks:?}; delivered {:?}; points {:?}, stolen {}",
                stood_ticks.iter().sum::<u64>(),
                goal_ticks.iter().sum::<u64>(),
                100.0 * stood_ticks.iter().sum::<u64>() as f64 / goal_ticks.iter().sum::<u64>().max(1) as f64,
                game.delivered,
                game.points,
                game.stolen
            );
            eprintln!(
                "  takes: {} taken, {take_ends} could-not-take ends, {stands_given_up} stands given up ({stands_stale} at a belief the battery had left)",
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
            let raid_runs = raids.finish();
            eprintln!("  raids: {}", raid_line(&raid_runs));
            // A side's raids: [runs, bot-s, came within 5 m, stole, died].
            let mut raid_sums = [[0f32; 5]; 2];
            for run in &raid_runs {
                let side = &mut raid_sums[usize::from(teams[run.place] & 1)];
                side[0] += 1.0;
                side[1] += run.ran_s;
                side[2] += f32::from(u8::from(run.nearest_m <= 5.0));
                side[3] += f32::from(u8::from(run.stole));
                side[4] += f32::from(u8::from(run.died));
                if std::env::var("SRP_BENCH_SHOW_RAIDS").is_ok() {
                    eprintln!(
                        "  raid {} {:.3} {:.1} {:.1} {} {} {}",
                        teams[run.place] & 1,
                        run.score,
                        run.ran_s,
                        run.nearest_m,
                        u8::from(run.stole),
                        u8::from(run.died),
                        run.next.split(';').next().unwrap_or_default().replace(' ', "_")
                    );
                }
            }
            eprintln!("  {}", blocks.finish(&mut events));
            eprintln!("  {}", blocks.gates.line());
            eprintln!("  {}", hits.finish(&mut events));
            eprintln!("  {}", fight.line(&game));
            eprintln!("  routes: {}", routes.line());
            eprintln!(
                "  jumps: pressed {} refused {}",
                game.jumps_pressed, game.jumps_refused
            );
            eprintln!("  push: {}", push.line(&bots));
            if let Some(human) = &human {
                eprintln!("  human: {}", human.line());
            }
            eprintln!(
                "  carriers: killed carrying {:?} (by team carrying), by the shooter's play {:?}; delivered {:?}; picked up {} lying + {} stolen; plays taken {takes_all} = {:.2} a bot-minute",
                carriers_killed,
                carrier_kills,
                game.delivered,
                game.taken,
                game.stolen,
                f64::from(takes_all) / teams.len() as f64 / (f64::from(limit_s) / 60.0)
            );
            eprintln!("  carry: {}", carry.line(game.taken));
            // One line of sums for comparing runs paired by seed.
            let mut kd = [[0u32; 2]; 2];
            for (place, stat) in game.stats.iter().enumerate() {
                let side = &mut kd[usize::from(teams[place])];
                side[0] += stat.0;
                side[1] += stat.1;
            }
            let play_s: std::collections::BTreeMap<&String, f64> = play_ticks
                .iter()
                .map(|(k, n)| (k, *n as f64 * f64::from(TICK_MS) / 1000.0))
                .collect();
            let play_rows: std::collections::BTreeMap<&String, [u32; 5]> =
                by_play.iter().map(|(k, r)| (k, *r)).collect();
            eprintln!(
                "  sums: {}",
                serde_json::json!({
                    "map": format!("{project}-{per_team}v{per_team}"),
                    "seed": seed,
                    "kills": [kd[0][0], kd[1][0]],
                    "deaths": [kd[0][1], kd[1][1]],
                    "delivered": game.delivered,
                    "points": game.points,
                    "taken": game.taken,
                    "take_ends": take_ends,
                    "stands_given_up": stands_given_up,
                    "stands_stale": stands_stale,
                    "carriers_killed": carriers_killed,
                    "play_s": play_s,
                    "by_play": play_rows,
                    "deaths_by": deaths_by,
                    "camper": game.camper.map(|(p, t)| (p, t, game.bodies[p].alive)),
                    "camp_ended": game.camp_ended,
                    "nudged": game.nudged,
                    "raids": raid_sums,
                    "stolen": game.stolen,
                })
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
                 rays {:.1} per bot a second; kills {kills}, shots {}, hits {}, team hits {}, team kills {}",
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
                game.team_hits,
                game.team_kills,
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
    // Where the world has it: the worlds are in the map's frame.
    assert!((kept.at - reset).length() < 1e-3, "{kept:?}");
    assert!(known(&bots, 0).is_none());
}

/// A kit's ammo is its storage for the match: each life starts with the
/// slot's rounds while the storage has them, only the rounds fired are
/// gone, and when the present kit has less than a magazine left the bot
/// picks, once, the kit with the most rounds for its next life.
#[test]
fn a_bot_spent_of_its_first_kit_picks_its_second_for_the_next_life() {
    use survarium::v026g::loadouts::BOT_AMMO_LIVES;
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
    let items: Vec<&str> = bot
        .kits
        .iter()
        .map(|(arms, _)| arms.primary.0.item)
        .collect();
    assert_eq!(items, ["vityaz", "ak_74u", "mp_153"]);
    // Each kit full: a life's rounds (the bags' most) times the lives.
    let life: Vec<u32> = bot.kits.iter().map(|(arms, _)| arms.primary.1).collect();
    assert_eq!(life, [200, 120, 60]);
    let left: Vec<u32> = bot.kits.iter().map(|&(_, left)| left).collect();
    assert_eq!(
        left,
        life.iter().map(|n| n * BOT_AMMO_LIVES).collect::<Vec<_>>()
    );
    assert_eq!(bot.reflex.left(), 200);
    // Full: no choice. A life that fires 160 of its 200: the kit has
    // 1040 left, and it keeps the kit.
    assert_eq!(bot.pick_kit(), None);
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (32, 8);
    assert_eq!(bot.pick_kit(), None);
    // The next life in the same kit starts full again from its storage.
    bot.new_life(0.5, None, Vec3::ZERO);
    assert_eq!((bot.kit, bot.kits[0].1, bot.reflex.left()), (0, 1040, 200));
    // Its storage down to 210, and this life fires 190 of them: 20 left,
    // under a magazine. The fullest other kit for the next life (the
    // AK-74U's 720 before the MP-153's 360), chosen once.
    bot.kits[0].1 = 210;
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (10, 0);
    assert_eq!(bot.pick_kit(), Some((1, 20, 720)));
    assert_eq!(bot.pick_kit(), None);
    // Its next life is in kit 2 with a life's 120 of the AK-74U; kit 1
    // keeps its 20.
    bot.new_life(0.5, None, Vec3::ZERO);
    assert_eq!(bot.kit, 1);
    assert_eq!(bot.reflex.held_gun().item, "ak_74u");
    assert_eq!(bot.reflex.left(), 120);
    assert_eq!(bot.kits[0].1, 20);
    // The rusher's kits: the MP-153, the Vityaz, the AK-74U.
    let rusher = bots.bots[1].as_ref().unwrap();
    let items: Vec<&str> = rusher
        .kits
        .iter()
        .map(|(arms, _)| arms.primary.0.item)
        .collect();
    assert_eq!(items, ["mp_153", "vityaz", "ak_74u"]);
}

/// Three kits, the first empty: the bot picks the fullest of the others,
/// whichever its place.
#[test]
fn a_bot_with_its_first_kit_empty_picks_the_fullest() {
    let mut bots = Bots::new(
        0x418,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &[0],
        &[true],
        &[Some("flanker")],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    let bot = bots.bots[0].as_mut().unwrap();
    assert_eq!(bot.kits.len(), 3);
    // This life's 200 were the kit's last, all fired.
    bot.kits[0].1 = 200;
    bot.kits[1].1 = 100;
    bot.kits[2].1 = 300;
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (0, 0);
    assert!(bot.reflex.dry());
    assert_eq!(bot.pick_kit(), Some((2, 0, 300)));
    assert_eq!(bot.pick_kit(), None);
    bot.new_life(0.5, None, Vec3::ZERO);
    assert_eq!((bot.kit, bot.reflex.held_gun().item), (2, "mp_153"));
    assert_eq!(bot.reflex.left(), 60);
    assert_eq!(bot.kits[0].1, 0);
}

/// Every kit under a magazine: another only when it has more rounds than
/// the present one, chosen once, and the choice does not go back and
/// forth while the present kit runs down.
#[test]
fn every_kit_low_the_bot_picks_the_fullest_once() {
    let mut bots = Bots::new(
        0x419,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &[0],
        &[true],
        &[Some("flanker")],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    let bot = bots.bots[0].as_mut().unwrap();
    // The Vityaz's storage 200, this life fired 180: 20 left (under a
    // magazine). The AK-74U's kit 10 (of 30), the MP-153's 1.
    bot.kits[0].1 = 200;
    bot.kits[1].1 = 10;
    bot.kits[2].1 = 1;
    let hands = bot.reflex.hands();
    (hands.primary.rounds, hands.primary.spare) = (20, 0);
    // Nothing fuller than the present kit: it stays.
    assert_eq!(bot.pick_kit(), None);
    // Down to 5: the AK-74U's 10, once.
    bot.reflex.hands().primary.rounds = 5;
    assert_eq!(bot.pick_kit(), Some((1, 5, 10)));
    for rounds in [4, 2, 0] {
        bot.reflex.hands().primary.rounds = rounds;
        assert_eq!(bot.pick_kit(), None, "{rounds}");
    }
    bot.new_life(0.5, None, Vec3::ZERO);
    assert_eq!((bot.kit, bot.reflex.left()), (1, 10));
    // In the AK-74U's kit with its 10: the Vityaz's has nothing, the
    // MP-153's 1. No kit with more: it stays, and never goes back to the empty one.
    assert_eq!(bot.pick_kit(), None);
    bot.reflex.hands().primary.rounds = 0;
    bot.reflex.hands().primary.spare = 0;
    assert_eq!(bot.pick_kit(), Some((2, 0, 1)));
    assert_eq!(bot.pick_kit(), None);
}

/// A bot's kits are its tier's (`Bots::with_tiers`, read from its
/// profile): three kits, each with a life's rounds of its own ammo.
#[test]
fn a_bot_of_a_higher_tier_carries_that_tiers_kits() {
    use survarium::v026g::loadouts::{BotRole, BotTier};
    let bots = Bots::new(
        0x41A,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &[0, 1, 1],
        &[true, true, true],
        &[Some("support"), Some("camper"), None],
        Arc::new(Tuning::default()),
        Trace::none(0),
    )
    .with_tiers(&[Some(BotTier::High), Some(BotTier::Mid), Some(BotTier::High)]);
    let items = |place: usize| -> Vec<&str> {
        bots.bots[place]
            .as_ref()
            .unwrap()
            .kits
            .iter()
            .map(|(arms, _)| arms.primary.0.item)
            .collect()
    };
    assert_eq!(items(0), ["ak_12", "sr3m", "vsk_94"]);
    assert_eq!(items(1), ["sv_98", "akmn", "pp2000"]);
    // A bot of no role keeps none.
    assert!(items(2).is_empty());
    let support = bots.bots[0].as_ref().unwrap();
    assert_eq!(support.reflex.held_gun().item, "ak_12");
    assert_eq!(support.reflex.left(), support.kits[0].0.primary.1);
    assert_eq!(support.role, Some(BotRole::Support));
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

/// The side step weaves a walk to or from the target into a diagonal; a
/// walk to a side already, or none, is left as it is.
#[test]
fn the_side_step_weaves_a_walk_along_the_view() {
    assert_eq!(weave(key::FORWARD, key::LEFT), Some(key::FORWARD | key::LEFT));
    assert_eq!(weave(key::BACK, key::RIGHT), Some(key::BACK | key::RIGHT));
    assert_eq!(
        weave(key::FORWARD | key::CROUCH | key::JUMP, key::RIGHT),
        Some(key::FORWARD | key::CROUCH | key::JUMP | key::RIGHT)
    );
    assert_eq!(weave(key::FORWARD | key::LEFT, key::RIGHT), None);
    assert_eq!(weave(key::LEFT, key::RIGHT), None);
    assert_eq!(weave(0, key::LEFT), None);
    assert_eq!(weave(key::FORWARD, 0), None);
}

/// The commander moving a bot's Crossfire to another choke leaves the
/// play's cooldown alone (the new run is still a candidate at the next
/// poll); a switch to another play starts it.
#[test]
fn a_switch_to_another_target_of_the_same_play_keeps_its_cooldown() {
    use super::plays::{PlayKind, Running, crossfire};
    let tuning = Tuning::default();
    let mut bot = Bot::new(1, 0, 0, Some("support"), &tuning);
    bot.last_ran.push((PlayKind::Crossfire, 1_000));
    bot.running = Some(Running::Crossfire(crossfire::State::NoSpot));
    bot.give_way(PlayKind::Crossfire, 60_000);
    assert!(bot.running.is_none());
    assert_eq!(bot.last_ran, vec![(PlayKind::Crossfire, 1_000)]);
    bot.running = Some(Running::Crossfire(crossfire::State::NoSpot));
    bot.give_way(PlayKind::Roam, 61_000);
    assert_eq!(bot.last_ran, vec![(PlayKind::Crossfire, 61_000)]);
    // With nothing running, nothing is stamped.
    bot.give_way(PlayKind::Cut, 62_000);
    assert_eq!(bot.last_ran, vec![(PlayKind::Crossfire, 61_000)]);
}

/// A jump while the trigger is down: held [`JUMP_HOLD_MS`] with the trigger
/// let go (the weapon takes no jump while it fires, 0x2E3A70), then the
/// trigger again; a jump with the trigger not down lately is the one tick
/// it was. Each jump is traced, and what followed it.
#[test]
fn a_jump_lets_the_trigger_go_and_is_traced() {
    let tuning = Tuning::default();
    let at = Vec3::new(1.0, 0.0, 2.0);
    let walk = key::FORWARD | key::FIRE;
    let mut bot = Bot::new(1, 0, 0, Some("support"), &tuning);
    bot.tracing = true;
    assert_eq!(jumping(&mut bot, walk, None, at, 1_000), walk);
    let hold = (JUMP_HOLD_MS / TICK_MS) as usize;
    let mut sent = vec![jumping(&mut bot, walk | key::JUMP, Some("hop"), at, 1_050)];
    for k in 1..hold + 2 {
        sent.push(jumping(&mut bot, walk, None, at, 1_050 + TICK_MS * k as u32));
    }
    let held = key::FORWARD | key::JUMP;
    assert!(sent[..hold].iter().all(|&k| k == held), "{sent:?}");
    assert!(sent[hold..].iter().all(|&k| k == walk), "{sent:?}");
    assert_eq!(bot.traced.len(), 1);
    assert_eq!(bot.traced[0]["jump"], "hop");
    assert_eq!(bot.traced[0]["firing"], true);
    bot.jump_followed(4_050, Some(at + Vec3::new(2.0, 0.5, 0.0)), "went on");
    assert_eq!(bot.traced[1]["jumped"], "hop");
    assert_eq!(bot.traced[1]["then"], "went on");
    assert_eq!(bot.traced[1]["after_ms"], 3_000);
    assert!((bot.traced[1]["moved_m"].as_f64().unwrap() - 2.0).abs() < 1e-6);
    assert!(bot.jump_watch.is_none());

    // Not firing: one tick, as before.
    let mut bot = Bot::new(1, 0, 0, Some("support"), &tuning);
    let walk = key::FORWARD;
    assert_eq!(
        jumping(&mut bot, walk | key::JUMP, Some("route"), at, 1_000),
        walk | key::JUMP
    );
    assert_eq!(jumping(&mut bot, walk, None, at, 1_050), walk);
    // Not tracing: watched, no line.
    assert!(bot.traced.is_empty() && bot.jump_watch.is_some());
}

/// The near margin of the line of fire (`friends::FriendlyFireTuning::
/// near_m`) off and on, `SRP_NEAR_SEEDS` seeds (default 8): whole TDM
/// (6 against 6) and battery (4 against 4) matches, kills and team hits
/// per seed. A measurement: it asserts nothing (friends.rs has the numbers).
#[test]
#[ignore = "whole matches; run in release"]
fn friends_near_margin_off_and_on() {
    let seeds: u64 = std::env::var("SRP_NEAR_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);
    for seed in 1..=seeds {
        for near in [0.0, bench_tuning().aim.friends.near_m] {
            let mut tuning = bench_tuning();
            tuning.aim.friends.near_m = near;
            let tdm = soak_tuned(6, 0, 60, None, 600, seed, tuning.clone());
            let batteries = soak_tuned(4, 3, 0, None, 600, seed, tuning);
            for (mode, out) in [("tdm", tdm), ("batteries", batteries)] {
                eprintln!(
                    "{mode} seed {seed} near_m {near}: kills {}, team kills {}; hits {}, team hits {} of {} shots; winner {:?}",
                    out.kills[0] + out.kills[1],
                    out.team_kills,
                    out.hits,
                    out.team_hits,
                    out.shots,
                    out.winner
                );
            }
        }
    }
}

/// The field with a walkway 3 m up along z 31-34 from x 4 to x 30, a ramp
/// down from its east end to the ground at x 38, team 0's container on it
/// at x 7, and a wall under it across x = 14 (z 28-34): the shape of
/// 0x4da3b51d's stuck carrier (`player_bots_stuck.md` §8).
fn walkway_world() -> Arc<World> {
    use super::body::tests::{floor, wall_x};
    let walker = mapdata::WalkerBehaviour::BySlope;
    let tri = |a: Vec3, b: Vec3, c: Vec3| mapdata::Triangle {
        v: [a, b, c],
        walker,
    };
    let mut solids = floor(4.0, 31.0, 30.0, 34.0, 3.0).to_vec();
    let (a, b) = (Vec3::new(30.0, 3.0, 31.0), Vec3::new(38.0, 0.0, 31.0));
    let (c, d) = (Vec3::new(38.0, 0.0, 34.0), Vec3::new(30.0, 3.0, 34.0));
    solids.push(tri(a, b, c));
    solids.push(tri(a, c, d));
    // The ramp solid under (no way beneath it).
    solids.extend(wall_x(30.0, 31.0, 34.0));
    for z in [31.0, 34.0] {
        solids.push(tri(
            Vec3::new(30.0, 0.0, z),
            Vec3::new(38.0, 0.0, z),
            Vec3::new(30.0, 3.0, z),
        ));
    }
    solids.extend(wall_x(14.0, 28.0, 34.0));
    super::tactics::tests::field_with_solids(&solids, |map| {
        let container = |team: i32, id: i32, at: Vec3| Container {
            position: at,
            team,
            id,
            use_shapes: Vec::new(),
        };
        map.containers = vec![
            container(0, 0, Vec3::new(7.0, 3.0, 32.5)),
            container(1, 1, Vec3::new(68.0, 0.0, 30.0)),
        ];
        map.battery_spawners = vec![BatterySpawner {
            position: Vec3::new(15.0, 0.0, 32.5),
            item: Vec3::new(15.0, 0.3, 32.5),
            layer: "default".into(),
        }];
    })
}

/// Issue 1 of the audit (0x4da3b51d bot 13, 273 s): a carrier fallen off a
/// walkway stands under it against a wall, its route above it. Its route
/// was found on the way (the ground under a route is on it across) and its
/// reckoning was as blocked as it (no stuck sign): it pushed into the wall
/// until the end. It must see it is off its route (the floors' heights),
/// or stuck by its walk keys held with its body still, and go round by the
/// ramp: home within 45 s of play, the take included (before: never).
#[test]
fn a_carrier_fallen_under_its_walkway_route_gets_home() {
    let world = walkway_world();
    let teams = [0u8, 1];
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = true;
    let mut bots = Bots::new(
        0x57C4,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 11, None, None);
    // Under the walkway, east of the wall, by the battery.
    let under = Vec3::new(16.0, 0.0, 32.5);
    game.bodies[0].node = world.grid.locate(under, 0.5);
    game.bodies[0].feet = under;
    let grid = &world.grid;
    assert!(grid.position(game.bodies[0].node.unwrap()).y < 0.5);
    bots.entered(0, true, 0);
    let (mut clock, mut next_world, mut planted) = (0, 1_000, false);
    while clock < 60_000 && game.delivered[0] == 0 {
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
        // Once it took it and heads home: the route it had on the walkway,
        // from above where it stands on to its container (as it had before
        // it fell).
        if !planted
            && let Some(bot) = bots.bots[0].as_mut()
            && let Some((_, goal)) = bot.route
            && goal.distance_xz(Vec3::new(7.0, 0.0, 32.5)) < 3.0
        {
            let from = grid.locate(Vec3::new(21.0, 3.0, 32.5), 0.5).unwrap();
            let to = grid.locate(goal, 3.0).unwrap();
            assert!(grid.position(to).y > 2.5, "home is on the walkway: {goal:?}");
            bot.route = Some((path::plan(grid, from, to).unwrap(), goal));
            planted = true;
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
    }
    assert!(planted, "never headed home");
    assert!(
        game.delivered[0] == 1 && clock <= 45_000,
        "not home by {clock} ms: at {:?}",
        game.bodies[0].feet
    );
}

/// Bot 0 of [`soak_world`] (no play: its goal set each poll) walked about
/// by the stand-in, with `floors` floors searched a poll; `goal` gives its
/// goal at each clock. Calls `each` after every poll with the clock, the
/// bot, its body and the notes; runs to `until` ms.
fn walk_to_goals(
    floors: u32,
    until: u32,
    goal: impl Fn(u32) -> Vec3,
    mut each: impl FnMut(u32, &Bot, &Body, &[Note]),
) {
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.think.search_floors_per_poll = floors;
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x6E07,
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
    let start = Vec3::new(5.0, 0.0, 10.0);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    bots.entered(0, true, 0);
    let (mut clock, mut next_world) = (0, 1_000);
    while clock < until {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        let mut notes = Vec::new();
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    notes.extend(bots.fixed(place, fix, Some(health)));
                }
                bots.facts(facts);
            }
        }
        // `Bots::poll` with the play's step left out: the goal is given.
        let clock_now = Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        let view = bots
            .tracker
            .view(&clock_now, &bots.tuning.kin, Some(&world));
        bots.searching.left = SEARCHES_PER_POLL;
        bots.searching.floors = bots.tuning.think.search_floors_per_poll as usize;
        bots.go_on_searching();
        if let Some(bot) = bots.bots[0].as_mut() {
            bot.intent = Intent::to(goal(clock), Pace::Run).within(1.0);
        }
        let mut made = Vec::new();
        bots.ticks(0, &clock_now, &view, &mut made);
        if let Some(bot) = bots.bots[0].as_mut()
            && let Some(text) = bot.dropped.take()
        {
            bot.note(&mut notes, clock, text);
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
        each(clock, bots.bots[0].as_ref().unwrap(), &game.bodies[0], &notes);
    }
}

/// Issue 5 of the audit: a bot whose goal moves on keeps walking its
/// route while the new search goes on over several polls, never standing
/// for it; and one whose goal moves on every poll (its searches dropped
/// before they end) keeps walking too, and notes the dropped searches at
/// most once in [`path::DROPPED_NOTE_MS`].
#[test]
fn a_bot_keeps_walking_its_route_while_its_next_search_goes_on() {
    // Round the wall's end (x 40, z 0-25) to x 48 (out of the other
    // side's spawn area), then 4.5 m on; 150 floors a poll: the search to
    // the new goal takes several.
    let (first, moved) = (Vec3::new(48.0, 0.0, 36.0), Vec3::new(48.0, 0.0, 31.5));
    let changed = 6_000;
    let sliced = path::searches().sliced;
    let (mut had, mut routeless, mut to_moved) = (false, 0, None);
    let mut walked = (None, 0.0f32);
    walk_to_goals(
        150,
        12_000,
        |clock| if clock < changed { first } else { moved },
        |clock, bot, body, _| {
            had |= bot.route.is_some();
            if had && bot.route.is_none() {
                routeless += 1;
            }
            if to_moved.is_none()
                && clock > changed
                && bot.route.as_ref().is_some_and(|(_, goal)| goal.distance_xz(moved) < 0.5)
            {
                to_moved = Some(clock);
            }
            if clock == changed {
                walked.0 = Some(body.feet);
            }
            if clock == changed + 3_000 {
                walked.1 = walked.0.map_or(0.0, |from| from.distance_xz(body.feet));
            }
        },
    );
    // Both searches went on over several polls (their first poll's way so
    // far walked meanwhile).
    assert!(path::searches().sliced >= sliced + 2);
    assert!(to_moved.is_some_and(|at| at <= changed + 100), "{to_moved:?}");
    assert_eq!(routeless, 0, "stood {routeless} polls with no route");
    assert!(walked.1 > 8.0, "walked {} m in the 3 s after", walked.1);
    // Every poll a new goal 3 m on (its search never ends): still a route
    // at every poll once it had one, and a note of the dropped searches
    // at most every 5 s.
    let (mut had, mut routeless, mut notes) = (false, 0, Vec::new());
    let mut moved_m = (None, 0.0f32);
    walk_to_goals(
        150,
        16_000,
        |clock| Vec3::new(48.0, 0.0, 10.0 + 3.0 * ((clock / 25) % 8) as f32),
        |clock, bot, body, polled| {
            had |= bot.route.is_some();
            if had && bot.route.is_none() {
                routeless += 1;
            }
            for note in polled {
                if note.text.starts_with("dropped its route search") {
                    notes.push(clock);
                }
            }
            if clock == 4_000 {
                moved_m.0 = Some(body.feet);
            }
            if clock == 16_000 {
                moved_m.1 = moved_m.0.map_or(0.0, |from| from.distance_xz(body.feet));
            }
        },
    );
    assert!(had);
    assert_eq!(routeless, 0, "stood {routeless} polls with no route");
    assert!(!notes.is_empty() && notes.len() <= 4, "notes at {notes:?}");
    assert!(notes.windows(2).all(|w| w[1] >= w[0] + path::DROPPED_NOTE_MS), "{notes:?}");
    assert!(moved_m.1 > 10.0, "walked {} m in 12 s", moved_m.1);
}

/// Issue 5's stand (0x5d506a22 bot 14, 116-131 s): a goal computed at
/// another body's height (Buddy's, short of a partner on a walkway) hangs
/// 4 m over the floor under it, where `WalkGrid::locate` finds none. The
/// bot walks to the floor under it ([`path::goal_floor`]) instead of
/// standing with no way.
#[test]
fn a_goal_in_the_air_is_walked_to_the_floor_under_it() {
    let goal = Vec3::new(20.0, 4.0, 20.0);
    let (mut had, mut end) = (false, Vec3::ZERO);
    walk_to_goals(
        10_000,
        12_000,
        |_| goal,
        |_, bot, body, _| {
            had |= bot.route.is_some();
            end = body.feet;
        },
    );
    assert!(had, "never had a route");
    assert!(end.distance_xz(goal) < 2.0, "at {end:?}");
}

/// The bench's count of how the bots got about (`player_bots_stuck.md` §8),
/// as the audit's `wallpush.py` and `movement.py` count it from a match's
/// worlds: sampled once a world (1 s), and the carriers each tick.
#[derive(Default)]
struct RouteTally {
    /// Each body at the last world: feet, alive; and its runs so far
    /// (worlds holding a walk key without moving, worlds still, worlds of
    /// them in a fight); its last move of 1.5 m or more.
    last: Vec<Option<(Vec3, bool)>>,
    push_run: Vec<u32>,
    idle_run: Vec<(u32, u32)>,
    last_move: Vec<Option<(f32, f32)>>,
    /// Runs of 5 worlds or more holding a walk key moving under 0.3 m a
    /// world, and their worlds.
    pushes: (u32, u32),
    /// Stretches of 5 s or more moving under 0.5 m a world, and their
    /// seconds: with no target, and in a fight.
    idle: (u32, u32),
    idle_fighting: u32,
    /// Moves of 1.5 m or more turning back over 135° from the one before,
    /// with no target.
    reversals: u32,
    /// Living bot-worlds.
    worlds: u32,
    /// Each body's pickup time, and the carries delivered (ms each) and
    /// lost.
    carry_from: Vec<Option<(u32, usize)>>,
    carried: Vec<u32>,
    lost: u32,
    /// "is stuck" notes, and those of them by the walk keys alone.
    stuck: (u32, u32),
    no_route: [u64; 5],
    /// Each body's feet at the last tick, and its falls of a metre or more
    /// in a tick: off its route (no drop of it there), and taken by it.
    feet: Vec<Option<Vec3>>,
    falls: (u32, u32),
    /// Each body's team, where its bots got stuck or fell off their route
    /// (team, place, where), and the stucks and falls off at a spot a
    /// teammate had already (`stuck::TeamSpots`).
    teams: Vec<Option<u8>>,
    marks: Vec<(u8, usize, Vec3)>,
    repeats: (u32, u32),
    /// Each body's zone set at the last world (0: none) and the set it
    /// last left, when; living bot-worlds on a zone's floors while it is
    /// on; entries onto zones' floors, and of them back onto the set left
    /// within `stuck::ZONE_AGAIN_MS`; the escapes' count at the start
    /// (`tactics::escapes`).
    zone_in: Vec<u16>,
    zone_left: Vec<Option<(u16, u32)>>,
    zone_on: u32,
    zone_entries: (u32, u32),
    escapes: [u64; 3],
}

impl RouteTally {
    fn start(bodies: usize) -> Self {
        RouteTally {
            last: vec![None; bodies],
            push_run: vec![0; bodies],
            idle_run: vec![(0, 0); bodies],
            last_move: vec![None; bodies],
            carry_from: vec![None; bodies],
            feet: vec![None; bodies],
            teams: vec![None; bodies],
            no_route: super::path::no_routes(),
            zone_in: vec![0; bodies],
            zone_left: vec![None; bodies],
            escapes: super::tactics::escapes::counts(),
            ..Default::default()
        }
    }

    /// A tick: the carries begun and ended, and the falls.
    fn tick(&mut self, game: &Game, bots: &Bots, time: u32) {
        for (place, body) in game.bodies.iter().enumerate() {
            if let Some(bot) = bots.bots[place].as_ref() {
                self.teams[place] = Some(bot.team);
            }
            let before = std::mem::replace(&mut self.feet[place], body.alive.then_some(body.feet));
            if let Some(from) = before
                && body.alive
                && from.y - body.feet.y >= 1.0
            {
                // Its route drops there: a corner over a metre under the
                // one before, within a metre of where it landed.
                let planned = bots.bots[place]
                    .as_ref()
                    .and_then(|bot| bot.route.as_ref())
                    .is_some_and(|(route, _)| {
                        route.points().windows(2).any(|w| {
                            w[0].at.y - w[1].at.y >= 1.0 && w[1].at.distance_xz(body.feet) <= 1.0
                        })
                    });
                if planned {
                    self.falls.1 += 1;
                } else {
                    self.falls.0 += 1;
                    self.repeats.1 += u32::from(self.mark(place, from));
                }
            }
            match (self.carry_from[place], body.carrying) {
                (None, Some(battery)) => self.carry_from[place] = Some((time, battery)),
                (Some((from, battery)), None) => {
                    self.carry_from[place] = None;
                    if matches!(game.batteries.get(battery), Some(Lying::Home(_))) {
                        self.carried.push(time - from);
                    } else {
                        self.lost += 1;
                    }
                }
                _ => {}
            }
        }
    }

    /// A world: the runs of pushing and standing, the reversals, and the
    /// bots on zones' floors.
    fn world(&mut self, game: &Game, bots: &Bots) {
        const WALK: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT;
        for (place, body) in game.bodies.iter().enumerate() {
            if bots.bots[place].is_some() {
                self.zones(place, body, bots, game.tick);
            }
            let now = (body.feet, body.alive);
            let before = self.last[place].replace(now);
            let Some((from, true)) = before.filter(|_| body.alive) else {
                self.end_runs(place);
                self.last_move[place] = None;
                continue;
            };
            self.worlds += 1;
            let moved = from.distance_xz(body.feet);
            let fight = body.target.is_some();
            if body.keys & WALK != 0 && moved < 0.3 {
                self.push_run[place] += 1;
            } else {
                self.end_push(place);
            }
            if moved < 0.5 {
                let run = &mut self.idle_run[place];
                run.0 += 1;
                run.1 += u32::from(fight);
            } else {
                self.end_idle(place);
            }
            let step = (body.feet.x - from.x, body.feet.z - from.z);
            if moved >= 1.5 && !fight {
                if let Some(back) = self.last_move[place] {
                    let cos = (step.0 * back.0 + step.1 * back.1) / (moved * back.0.hypot(back.1));
                    self.reversals += u32::from(cos < (135f32).to_radians().cos());
                }
                self.last_move[place] = Some(step);
            } else if moved >= 1.5 {
                self.last_move[place] = None;
            }
        }
    }

    /// `place`'s bot's zone at `time`: on its floors while on, entered
    /// (again).
    fn zones(&mut self, place: usize, body: &Body, bots: &Bots, time: u32) {
        let Some(tactics) = bots.tactics.as_ref() else {
            return;
        };
        let hazards = &tactics.hazards;
        let node = body
            .alive
            .then(|| tactics.world.grid.locate(body.feet, 1.0))
            .flatten();
        let set = node.map_or(0, |node| hazards.set_of(node));
        let on = node
            .and_then(|node| hazards.zone_of(node))
            .is_some_and(|zone| zone.activity.active(time));
        self.zone_on += u32::from(on);
        let was = std::mem::replace(&mut self.zone_in[place], set);
        if was == 0 && set != 0 {
            self.zone_entries.0 += 1;
            let again = self.zone_left[place].is_some_and(|(left, at)| {
                left == set && time.saturating_sub(at) <= super::stuck::ZONE_AGAIN_MS
            });
            self.zone_entries.1 += u32::from(again);
        } else if was != 0 && set == 0 && body.alive {
            self.zone_left[place] = Some((was, time));
        }
        if !body.alive {
            self.zone_left[place] = None;
        }
    }

    fn end_push(&mut self, place: usize) {
        let run = std::mem::take(&mut self.push_run[place]);
        if run >= 5 {
            self.pushes.0 += 1;
            self.pushes.1 += run;
        }
    }

    fn end_idle(&mut self, place: usize) {
        let (run, fighting) = std::mem::take(&mut self.idle_run[place]);
        if run >= 5 {
            if fighting * 2 >= run {
                self.idle_fighting += run;
            } else {
                self.idle.0 += 1;
                self.idle.1 += run;
            }
        }
    }

    fn end_runs(&mut self, place: usize) {
        self.end_push(place);
        self.end_idle(place);
    }

    fn note(&mut self, note: &Note) {
        if note.text.starts_with("is stuck at") {
            if let Some(feet) = self.feet.get(note.place).copied().flatten() {
                self.repeats.0 += u32::from(self.mark(note.place, feet));
            }
            self.stuck.0 += 1;
            self.stuck.1 += u32::from(note.text.contains("(walk keys held"));
        }
    }

    /// A stuck or a fall off its route of `place`'s bot at `at`: whether a
    /// teammate had one there already.
    fn mark(&mut self, place: usize, at: Vec3) -> bool {
        let Some(team) = self.teams.get(place).copied().flatten() else {
            return false;
        };
        let again = self
            .marks
            .iter()
            .any(|&(t, p, spot)| t == team && p != place && super::stuck::near(spot, at));
        self.marks.push((team, place, at));
        again
    }

    /// The line the bench prints (one `routes:` line, `key value` pairs).
    fn line(mut self) -> String {
        for place in 0..self.last.len() {
            self.end_runs(place);
        }
        let now = super::path::no_routes();
        let no_route: Vec<u64> = (0..5).map(|k| now[k] - self.no_route[k]).collect();
        let escapes = super::tactics::escapes::counts();
        let escaped: Vec<u64> = (0..3).map(|k| escapes[k] - self.escapes[k]).collect();
        let mean = |v: &[u32]| v.iter().map(|&x| f64::from(x)).sum::<f64>() / v.len().max(1) as f64;
        format!(
            "worlds {} pushes {} push_s {} idle {} idle_s {} idle_fight_s {} reversals {} stuck {} stuck_keys {} carried {} carry_ms {:.0} lost {} no_way {} searching {} starved {} searched {} ended {} falls_off {} falls_planned {} stuck_again {} falls_again {} zone_on_s {} zone_entries {} zone_again {} escapes_back {} escapes_on {} escapes_none {}",
            self.worlds,
            self.pushes.0,
            self.pushes.1,
            self.idle.0,
            self.idle.1,
            self.idle_fighting,
            self.reversals,
            self.stuck.0,
            self.stuck.1,
            self.carried.len(),
            mean(&self.carried),
            self.lost,
            no_route[0],
            no_route[1],
            no_route[2],
            no_route[3],
            no_route[4],
            self.falls.0,
            self.falls.1,
            self.repeats.0,
            self.repeats.1,
            self.zone_on,
            self.zone_entries.0,
            self.zone_entries.1,
            escaped[0],
            escaped[1],
            escaped[2],
        )
    }
}

/// How far forward the bots go, as the #343 check's `advance.py` counts it
/// from a match's worlds: each living bot's progress along the line from
/// its side's spawns (0) to the enemy's (1), once a world; kills and deaths
/// by where the bot stood; and the spot plays' runs (HoldExit, Crossfire,
/// Cut) that reached their spot, read each tick.
struct PushTally {
    /// Each map side's spawns' middle.
    spawns: Vec<(i32, Vec3)>,
    /// Per team: living bot-worlds, of them past half way, past 70 %.
    worlds: [[u32; 3]; 2],
    /// Per team: kills and deaths, all and past half way.
    kills: [[u32; 2]; 2],
    deaths: [[u32; 2]; 2],
    /// Each place's progress at the last world.
    last: Vec<Option<f32>>,
    /// Each bot's spot-play run: (kind, since, reached its spot).
    runs: Vec<Option<(usize, u32, bool)>>,
    /// Per spot play (HoldExit, Crossfire, Cut) and team: runs, of them
    /// reached.
    spot: [[[u32; 2]; 2]; 3],
    /// Roam runs ended "the route turned bad".
    turned_bad: u32,
    /// Each place's last play taken ("Kind Target"), while it runs it.
    last_take: Vec<Option<String>>,
    /// A slot handed from one teammate to another: a bot takes the play
    /// and target a teammate was running, and the teammate no longer runs
    /// it.
    handoffs: u32,
}

impl PushTally {
    fn start(world: &World, bodies: usize) -> Self {
        let mut spawns: Vec<(i32, Vec3, u32)> = Vec::new();
        for s in &world.map.spawns {
            match spawns.iter_mut().find(|e| e.0 == s.team) {
                Some(e) => {
                    e.1 = e.1 + s.position;
                    e.2 += 1;
                }
                None => spawns.push((s.team, s.position, 1)),
            }
        }
        PushTally {
            spawns: spawns
                .into_iter()
                .map(|(team, sum, n)| (team, sum * (1.0 / n as f32)))
                .collect(),
            worlds: [[0; 3]; 2],
            kills: [[0; 2]; 2],
            deaths: [[0; 2]; 2],
            last: vec![None; bodies],
            runs: vec![None; bodies],
            spot: [[[0; 2]; 2]; 3],
            turned_bad: 0,
            last_take: vec![None; bodies],
            handoffs: 0,
        }
    }

    /// The commander's notes, after its poll: Roam's "the route turned
    /// bad" ends, and slot hand-offs.
    fn note(&mut self, note: &Note, bots: &Bots) {
        if note.text.starts_with("ends ") {
            self.turned_bad += u32::from(note.text == "ends Roam: the route turned bad");
            if let Some(last) = self.last_take.get_mut(note.place) {
                *last = None;
            }
            return;
        }
        let Some(rest) = note.text.strip_prefix("takes ") else {
            return;
        };
        let what = rest.split(" (score").next().unwrap_or(rest).to_owned();
        let Some(Some(me)) = bots.bots.get(note.place) else {
            return;
        };
        // Plays with no target take no slot.
        if !what.ends_with(" None") {
            for other in bots.bots.iter().flatten() {
                let running = |a: &Assignment| format!("{:?} {:?}", a.kind, a.target) == what;
                if other.place != note.place
                    && other.team == me.team
                    && self.last_take[other.place].as_deref() == Some(what.as_str())
                    && !other.assignment.as_ref().is_some_and(running)
                {
                    self.handoffs += 1;
                    self.last_take[other.place] = None;
                    break;
                }
            }
        }
        self.last_take[note.place] = Some(what);
    }

    /// A bot's progress from its side's spawns to the enemy's.
    fn progress(&self, side: Option<i32>, at: Vec3) -> Option<f32> {
        let side = side?;
        let own = self.spawns.iter().find(|s| s.0 == side)?.1;
        let enemy = self.spawns.iter().find(|s| s.0 != side)?.1;
        let (ax, az) = (enemy.x - own.x, enemy.z - own.z);
        let len2 = ax * ax + az * az;
        (len2 > 1.0).then(|| ((at.x - own.x) * ax + (at.z - own.z) * az) / len2)
    }

    fn world(&mut self, game: &Game, bots: &Bots) {
        for bot in bots.bots.iter().flatten() {
            let body = &game.bodies[bot.place];
            let p = body
                .alive
                .then(|| self.progress(bot.side, body.feet))
                .flatten();
            self.last[bot.place] = p;
            if let Some(p) = p {
                let w = &mut self.worlds[usize::from(bot.team & 1)];
                w[0] += 1;
                w[1] += u32::from(p > 0.5);
                w[2] += u32::from(p > 0.7);
            }
        }
    }

    /// A place's kills and deaths this tick.
    fn stats(&mut self, place: usize, team: u8, kills: u32, deaths: u32) {
        let forward = u32::from(self.last[place].is_some_and(|p| p > 0.5));
        let t = usize::from(team & 1);
        self.kills[t][0] += kills;
        self.kills[t][1] += kills * forward;
        self.deaths[t][0] += deaths;
        self.deaths[t][1] += deaths * forward;
    }

    fn tick(&mut self, bots: &Bots) {
        use super::plays::{PlayKind, Running, crossfire, cut, hold_exit};
        for bot in bots.bots.iter().flatten() {
            let run = bot.assignment.as_ref().and_then(|a| {
                let kind = match a.kind {
                    PlayKind::HoldExit => 0,
                    PlayKind::Crossfire => 1,
                    PlayKind::Cut => 2,
                    _ => return None,
                };
                Some((kind, a.since))
            });
            let held = matches!(
                bot.running,
                Some(Running::HoldExit(hold_exit::State::Hold { .. }))
                    | Some(Running::Crossfire(crossfire::State::Hold { .. }))
                    | Some(Running::Cut(cut::State::Hold { .. }))
            );
            let t = usize::from(bot.team & 1);
            match (&mut self.runs[bot.place], run) {
                (Some((kind, since, reached)), Some((k, s))) if *kind == k && *since == s => {
                    *reached |= held;
                }
                (slot, run) => {
                    if let Some((kind, _, reached)) = slot.take() {
                        self.spot[kind][t][0] += 1;
                        self.spot[kind][t][1] += u32::from(reached);
                    }
                    *slot = run.map(|(k, s)| (k, s, held));
                }
            }
        }
    }

    /// The line the bench prints (one `push:` line, `key value` pairs, team
    /// 0's then team 1's).
    fn line(mut self, bots: &Bots) -> String {
        for bot in bots.bots.iter().flatten() {
            if let Some((kind, _, reached)) = self.runs[bot.place].take() {
                let t = usize::from(bot.team & 1);
                self.spot[kind][t][0] += 1;
                self.spot[kind][t][1] += u32::from(reached);
            }
        }
        let mut out = String::new();
        for t in 0..2 {
            let w = self.worlds[t];
            let s = |k: usize| self.spot[k][t];
            out += &format!(
                "worlds{t} {} half{t} {} past70_{t} {} kills{t} {} kills_fwd{t} {} deaths{t} {} deaths_fwd{t} {} hold_exit{t} {} hold_exit_at{t} {} crossfire{t} {} crossfire_at{t} {} cut{t} {} cut_at{t} {} ",
                w[0],
                w[1],
                w[2],
                self.kills[t][0],
                self.kills[t][1],
                self.deaths[t][0],
                self.deaths[t][1],
                s(0)[0],
                s(0)[1],
                s(1)[0],
                s(1)[1],
                s(2)[0],
                s(2)[1],
            );
        }
        out += &format!("turned_bad {} handoffs {}", self.turned_bad, self.handoffs);
        out
    }
}

/// An enemy within this of the human stand-in, with a line to him, meets
/// him, m.
const MEET_M: f32 = 25.0;
/// A take met: an enemy met him this long before it, ms.
const MET_BEFORE_MS: u32 = 10_000;
/// A teammate this near at a take supports him, m.
const SUPPORT_M: f32 = 15.0;
/// His death traded: his killer dies this soon after, ms.
const TRADED_MS: u32 = 5_000;

/// The human stand-in (`SRP_BENCH_HUMAN=<place>`, CG-12): a bot the plays
/// take for a human, whose commander gives it Raid on the enemy's container
/// whenever it is a candidate. Read each tick: his takes from their
/// container and from the floor, how many an enemy met (a line to him
/// within [`MEET_M`] in the [`MET_BEFORE_MS`] before), the time from his
/// first theft to the next meeting, his deliveries, his takes with a
/// teammate within [`SUPPORT_M`], and his deaths traded.
struct HumanTally {
    place: Place,
    /// The tick an enemy last met him.
    met_at: Option<u32>,
    steals: u32,
    steals_met: u32,
    takes: u32,
    takes_met: u32,
    supported: u32,
    first_steal: Option<u32>,
    /// From his first theft to the next meeting, ms.
    first_meet_ms: Option<u32>,
    delivered: u32,
    deaths: u32,
    traded: u32,
    /// His deaths not yet traded: (tick, killer).
    open_deaths: Vec<(u32, Place)>,
    kills_seen: usize,
    last_kill: Option<(u32, Place, Place)>,
    carrying: Option<usize>,
}

impl HumanTally {
    fn start() -> Option<Self> {
        let place = std::env::var("SRP_BENCH_HUMAN").ok()?.parse().ok()?;
        Some(HumanTally {
            place,
            met_at: None,
            steals: 0,
            steals_met: 0,
            takes: 0,
            takes_met: 0,
            supported: 0,
            first_steal: None,
            first_meet_ms: None,
            delivered: 0,
            deaths: 0,
            traded: 0,
            open_deaths: Vec::new(),
            kills_seen: 0,
            last_kill: None,
            carrying: None,
        })
    }

    /// After the tick's step; `thieves` are the tick's takes from a
    /// container.
    fn tick(&mut self, game: &Game, thieves: &[Place]) {
        let tick = game.tick;
        let Some(me) = game.bodies.get(self.place) else {
            return;
        };
        if me.alive {
            let crouched = me.keys & key::CROUCH != 0;
            let met = game.bodies.iter().any(|b| {
                b.alive
                    && b.team != me.team
                    && b.feet.distance_xz(me.feet) <= MEET_M
                    && super::aimpoint::shows(Game::eye(b), me.feet, crouched, &game.world.sight, || {
                        true
                    })
            });
            if met {
                if let Some(first) = self.first_steal
                    && self.first_meet_ms.is_none()
                    && tick > first
                {
                    self.first_meet_ms = Some(tick - first);
                }
                self.met_at = Some(tick);
            }
        }
        let met_lately = self
            .met_at
            .is_some_and(|at| tick.saturating_sub(at) <= MET_BEFORE_MS);
        let supported = game.bodies.iter().enumerate().any(|(p, b)| {
            p != self.place && b.alive && b.team == me.team && b.feet.distance_xz(me.feet) <= SUPPORT_M
        });
        let stole = thieves.contains(&self.place);
        let took = game.took[self.place] == Some(tick) && !stole;
        if stole {
            self.steals += 1;
            self.steals_met += u32::from(met_lately);
            self.first_steal.get_or_insert(tick);
        }
        if took {
            self.takes += 1;
            self.takes_met += u32::from(met_lately);
        }
        if stole || took {
            self.supported += u32::from(supported);
        }
        self.delivered += u32::from(
            self.carrying.is_some() && me.carrying.is_none() && game.put_in.contains(&self.place),
        );
        self.carrying = me.carrying;
        // The events' world takes the log: what was seen is gone if its
        // last entry is not where it was.
        if self.kills_seen > 0 && game.kill_log.get(self.kills_seen - 1) != self.last_kill.as_ref() {
            self.kills_seen = 0;
        }
        for &(at, shooter, victim) in &game.kill_log[self.kills_seen..] {
            if victim == self.place {
                self.deaths += 1;
                self.open_deaths.push((at, shooter));
            } else if let Some(i) = self
                .open_deaths
                .iter()
                .position(|&(died, killer)| killer == victim && at <= died + TRADED_MS)
            {
                self.traded += 1;
                self.open_deaths.remove(i);
            }
        }
        self.kills_seen = game.kill_log.len();
        self.last_kill = game.kill_log.last().copied();
        self.open_deaths.retain(|&(died, _)| tick <= died + TRADED_MS);
    }

    /// The bench's `human:` line.
    fn line(&self) -> String {
        format!(
            "place {} steals {} steals_met {} takes {} takes_met {} supported {} first_meet_ms {} delivered {} deaths {} traded {}",
            self.place,
            self.steals,
            self.steals_met,
            self.takes,
            self.takes_met,
            self.supported,
            self.first_meet_ms.map_or(-1, i64::from),
            self.delivered,
            self.deaths,
            self.traded,
        )
    }
}

/// Match 0x6da44bbb at 662-677 s: two bots of the two teams 0.8-1 m apart,
/// each facing away from the other, knew each other only as heard (no
/// target, no shot for 15 s). Here two bots stand 1 m apart back to back
/// on the flat floor; with `SenseTuning::touch_m` each takes the other as
/// its target within a think and the notice of the first world that shows
/// them; with it off neither does in 2.5 s (the stand-in's bodies do not
/// block each other, so they are not stuck as there).
#[test]
fn back_to_back_at_arms_length_each_takes_the_other_as_target() {
    let run = |touch_m: f32| -> [Option<u32>; 2] {
        let world = soak_world();
        let mut tuning = Tuning::default();
        tuning.sense.touch_m = touch_m;
        tuning.sense.step_m = 0.0;
        let teams = [0u8, 1];
        let mut bots = Bots::new(
            0x6DA4,
            survarium::v026g::maps::mode::TEAM_DEATHMATCH,
            0,
            &teams,
            &[true, true],
            &[None, None],
            Arc::new(tuning),
            Trace::none(0),
        );
        bots.map_ready(world.clone());
        let mut game = Game::new(world.clone(), &teams, 5, Some(30), None);
        // Back to back: 0 faces +z, 1 a metre behind it faces -z.
        for (place, at, yaw) in [
            (0, Vec3::new(14.0, 0.0, 12.0), 0.0),
            (1, Vec3::new(14.0, 0.0, 11.0), std::f32::consts::PI),
        ] {
            game.bodies[place].node = world.grid.locate(at, 1.0);
            game.bodies[place].feet = at;
            game.bodies[place].yaw = yaw;
        }
        bots.entered(0, true, 0);
        bots.entered(1, true, 0);
        let mut taken = [None, None];
        let (mut clock, mut next_world) = (0, 500);
        while clock < 2_500 {
            clock += 25;
            while game.tick + TICK_MS <= clock {
                game.step();
            }
            // A world every half second, 300 ms behind.
            if clock >= next_world {
                next_world += 500;
                if let Some((_, fixes, health, facts)) =
                    game.world_at(clock / TICK_MS * TICK_MS - 300)
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
            for (place, other) in [(0, 1), (1, 0)] {
                let target = bots.bots[place].as_ref().and_then(|b| b.reflex.target());
                if taken[place].is_none() && target == Some(other) {
                    taken[place] = Some(clock);
                }
            }
        }
        taken
    };
    let (with, without) = (run(2.0), run(0.0));
    eprintln!("touch 2 m: targets at {with:?}; off: {without:?}");
    // The first world shows them at 500 ms: a think and the notice later.
    assert!(with.iter().all(|t| t.is_some_and(|t| t <= 1_200)), "{with:?}");
    assert_eq!(without, [None, None], "off: back to back unseen");
}

/// Bots by damage zones: the Cologne anomaly bench's runs, one bot of side 0
/// on an 80 x 40 m field with walls across x = 40, its goal given every
/// poll (no play), a zone's timing set by hand.
mod zone_runs {
    use mapdata::{MapData, RayTree, Rotation, Shape, ShapeKind, Spawn, WalkGrid, Zone, ZoneKind};

    use super::super::anomalies::Control;
    use super::super::hazards::Activity;
    use super::*;
    use crate::v026g::bots::body::tests::{floor, wall_x};

    fn zone(name: &str, center: Vec3, half: Vec3) -> Zone {
        Zone {
            kind: ZoneKind::Damage,
            name: name.into(),
            team: -1,
            position: center,
            damage_type: "electric".into(),
            damage: None,
            shapes: vec![Shape {
                kind: ShapeKind::Box,
                center,
                rotation: Rotation::IDENTITY,
                half,
            }],
        }
    }

    /// The field, walls across x = 40 over the `walls` z ranges, the zones.
    fn field(walls: &[(f32, f32)], zones: Vec<Zone>) -> Arc<World> {
        let mut triangles = floor(0.0, 0.0, 80.0, 40.0, 0.0).to_vec();
        for &(z0, z1) in walls {
            triangles.extend(wall_x(40.0, z0, z1));
        }
        let grid = WalkGrid::build(
            &triangles,
            [0.0, 0.0, 80.0, 40.0],
            0.25,
            mapdata::Body::default(),
        );
        let spawn = |team: i32, x: f32, z: f32| Spawn {
            position: Vec3::new(x, 0.0, z),
            rotation: Vec3::ZERO,
            team,
            priority: 0,
            point_id: 0,
            layer: "default".into(),
        };
        let map = MapData {
            spawns: vec![spawn(0, 5.0, 10.0), spawn(1, 75.0, 10.0)],
            zones,
            ..MapData::default()
        };
        let sight = RayTree::build(&triangles);
        Arc::new(World::assemble(map, grid.into(), sight.into()))
    }

    /// The wall's only gap (z 25-30; with `detour` also z 0-3), a 4 x 5 m
    /// zone in it.
    fn gap(detour: bool) -> Arc<World> {
        let walls: &[(f32, f32)] = if detour {
            &[(3.0, 25.0), (30.0, 40.0)]
        } else {
            &[(0.0, 25.0), (30.0, 40.0)]
        };
        let zone = zone("gap", Vec3::new(40.0, 1.0, 27.5), Vec3::new(2.0, 1.0, 2.5));
        field(walls, vec![zone])
    }

    /// On 6 s, off 6 s, from `first_on` (level_03's electric pairs).
    fn timer(first_on: u32) -> Activity {
        Activity::Spans(
            (0..20)
                .map(|k| (first_on + k * 12_000, first_on + k * 12_000 + 6_000))
                .collect(),
        )
    }

    struct Run {
        world: Arc<World>,
        start: Vec3,
        goal: Option<Vec3>,
        /// Zone 0's timing, put back every poll; none: always on.
        activity: Option<Activity>,
        /// The body put here at this time (pushed in).
        push: Option<(u32, Vec3)>,
        /// Its route dropped this often, ms (a chase: the goal moves on).
        replan_every: Option<u32>,
        until: u32,
    }

    fn run(world: Arc<World>, activity: Option<Activity>) -> Run {
        Run {
            world,
            start: Vec3::new(12.0, 0.0, 27.5),
            goal: Some(Vec3::new(60.0, 0.0, 27.5)),
            activity,
            push: None,
            replan_every: None,
            until: 60_000,
        }
    }

    #[derive(Debug, Default)]
    struct Seen {
        /// Ms on a zone's floors while it is on, and the longest stretch.
        on_ms: u32,
        longest_on_ms: u32,
        /// Steps off the zone's floors back west, the goal east of it.
        back_outs: u32,
        /// When it got within 1.5 m of its goal.
        arrived_ms: Option<u32>,
    }

    fn drive(run: Run) -> Seen {
        let world = run.world.clone();
        let teams = [0u8];
        let mut bots = Bots::new(
            0xA70A,
            survarium::v026g::maps::mode::BATTERIES,
            0,
            &teams,
            &[true],
            &[None],
            Arc::new(Tuning::default()),
            Trace::none(0),
        );
        bots.map_ready(world.clone());
        let mut game = Game::new(world.clone(), &teams, 7, None, None);
        game.bodies[0].node = world.grid.locate(run.start, 1.0);
        game.bodies[0].feet = run.start;
        bots.entered(0, true, 0);
        let middle = world.map.zones[0].shapes[0].center.x;
        let mut seen = Seen::default();
        let (mut clock, mut next_world) = (0u32, 1_000u32);
        let (mut pushed, mut on_run, mut was_in) = (false, 0u32, false);
        while clock < run.until {
            clock += 25;
            while game.tick + TICK_MS <= clock {
                game.step();
            }
            if let Some((at, to)) = run.push
                && !pushed
                && clock >= at
            {
                pushed = true;
                game.bodies[0].node = world.grid.locate(to, 1.0);
                game.bodies[0].feet = to;
            }
            if clock >= next_world {
                next_world += 1_000;
                if let Some((_, fixes, health, facts)) =
                    game.world_at(clock / TICK_MS * TICK_MS - 300)
                {
                    for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                        bots.fixed(place, fix, Some(health));
                    }
                    bots.facts(facts);
                }
            }
            if let (Some(activity), Some(tactics)) = (&run.activity, bots.tactics.as_mut()) {
                tactics.hazards.zones[0].control = Control::Timer;
                tactics.hazards.zones[0].activity = activity.clone();
            }
            let now = Clock {
                clock,
                time_left_s: 600.0,
                total_s: 900.0,
                playing: true,
            };
            let view = bots.tracker.view(&now, &bots.tuning.kin, Some(&world));
            bots.searching.left = SEARCHES_PER_POLL;
            bots.searching.floors = bots.tuning.think.search_floors_per_poll as usize;
            bots.go_on_searching();
            if let Some(bot) = bots.bots[0].as_mut() {
                bot.intent = match run.goal {
                    Some(at) => Intent::to(at, Pace::Run).within(1.0),
                    None => Intent::stay(),
                };
                if run.replan_every.is_some_and(|every| clock % every == 0) {
                    bot.route = None;
                }
            }
            let mut made = Vec::new();
            bots.ticks(0, &now, &view, &mut made);
            for made in made {
                let r = made.input.rotation;
                let turn = (
                    half_value(u16::from_le_bytes([r[0], r[1]])),
                    half_value(u16::from_le_bytes([r[2], r[3]])),
                );
                let gun = Arms::fallback().primary.0;
                game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
            }
            if clock % 100 != 0 {
                continue;
            }
            let hazards = &bots.tactics.as_ref().unwrap().hazards;
            let feet = game.bodies[0].feet;
            let node = world.grid.locate(feet, 1.0);
            let marked = node.is_some_and(|node| hazards.zone_of(node).is_some());
            if node.is_some_and(|node| hazards.on_at(node, clock)) {
                seen.on_ms += 100;
                on_run += 100;
                seen.longest_on_ms = seen.longest_on_ms.max(on_run);
            } else {
                on_run = 0;
            }
            if was_in && !marked && run.goal.is_some_and(|goal| goal.x > middle) && feet.x < middle
            {
                seen.back_outs += 1;
            }
            was_in = marked;
            if let Some(goal) = run.goal
                && seen.arrived_ms.is_none()
                && feet.distance_xz(goal) <= 1.5
            {
                seen.arrived_ms = Some(clock);
            }
        }
        eprintln!("{seen:?}");
        seen
    }

    /// Runs 1 and 3: a route through the only gap (or one with a detour) is
    /// not walked onto the zone's floors while it is on (from 6.5 s): it
    /// crosses in an off span, or goes round. Stock: 1.2 s on them, both.
    #[test]
    fn a_bot_on_its_way_keeps_off_a_zone_while_it_is_on() {
        for detour in [false, true] {
            let seen = drive(run(gap(detour), Some(timer(6_500))));
            assert_eq!(seen.on_ms, 0, "detour {detour}: {seen:?}");
            assert!(seen.arrived_ms.is_some(), "detour {detour}: {seen:?}");
        }
    }

    /// Run 8: the only way, its route planned again every 1.5 s (a chase):
    /// it crosses forward, never backing out the way it came. Stock: 2
    /// back-outs, the goal at 41.7 s.
    #[test]
    fn a_chasing_bot_crosses_a_switching_zone_without_backing_out() {
        let mut chase = run(gap(false), Some(timer(6_500)));
        chase.replan_every = Some(1_500);
        let seen = drive(chase);
        assert_eq!(seen.back_outs, 0, "{seen:?}");
        assert!(seen.arrived_ms.is_some_and(|at| at <= 35_000), "{seen:?}");
    }

    /// Runs 4 and 4b: with no goal (holding), or standing at its goal, in
    /// the gap when the zone comes on: out as it does. Stock: 5.9 s on its
    /// floors each on span.
    #[test]
    fn a_bot_with_no_goal_walks_out_of_a_zone_coming_on() {
        for goal in [None, Some(Vec3::new(40.3, 0.0, 27.5))] {
            let mut hold = run(gap(false), Some(timer(6_500)));
            hold.start = Vec3::new(40.0, 0.0, 27.5);
            hold.goal = goal;
            hold.until = 30_000;
            let seen = drive(hold);
            assert!(seen.on_ms <= 1_000, "goal {goal:?}: {seen:?}");
        }
    }

    /// Run 6b: with no goal, pushed into a small zone always on: out at
    /// once (within its next fixes: the stand-in sends them each second).
    /// Stock: 27 s, to the run's end.
    #[test]
    fn a_bot_with_no_goal_pushed_into_a_zone_walks_out() {
        let small = zone(
            "small",
            Vec3::new(30.0, 1.0, 20.0),
            Vec3::new(2.0, 1.0, 2.0),
        );
        let mut pushed = run(field(&[], vec![small]), None);
        pushed.start = Vec3::new(20.0, 0.0, 20.0);
        pushed.goal = None;
        pushed.push = Some((3_000, Vec3::new(30.0, 0.0, 20.0)));
        pushed.until = 30_000;
        let seen = drive(pushed);
        assert!(seen.on_ms <= 3_000, "{seen:?}");
    }

    /// Run 5: pushed 25 m into a big zone always on, deeper than the way
    /// out's search reaches: out by the exit field, then on to its goal.
    /// Stock: its escapes found nothing, it stood 37 s.
    #[test]
    fn a_bot_deep_in_a_big_zone_walks_out() {
        let big = zone(
            "big",
            Vec3::new(25.0, 1.0, 20.0),
            Vec3::new(25.0, 1.0, 20.0),
        );
        let mut deep = run(field(&[], vec![big]), None);
        deep.start = Vec3::new(60.0, 0.0, 20.0);
        deep.goal = Some(Vec3::new(70.0, 0.0, 30.0));
        deep.push = Some((3_000, Vec3::new(25.0, 0.0, 20.0)));
        deep.until = 40_000;
        let seen = drive(deep);
        assert!(seen.on_ms <= 15_000, "{seen:?}");
        assert!(seen.arrived_ms.is_some(), "{seen:?}");
    }
}

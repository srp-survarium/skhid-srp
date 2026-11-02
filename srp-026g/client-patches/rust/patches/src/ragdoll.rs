//! Patch 76: a killed player falls as a ragdoll, drawn only
//! (`docs/notes/ragdoll.md`). The solver and the corpses' machine are the
//! crate `ragdoll` (srp-026g/crates/ragdoll, pure Rust, tested there); this
//! is the game's side, its hooks `ragdoll.toml`:
//!
//! - `on_step` (game_world::tick, after the simulation's move): the corpses'
//!   machine, the ragdolls stepped, the level read by ray_test;
//! - `on_cull` (mt_draw's cull): a corpse of ours is always drawn;
//! - `on_bones` (mt_draw, the bones made, the "Weapon" bone not yet read):
//!   the clip captured, a ragdoll's bones written;
//! - `on_eye` (update_camera, the eye given to the view): the local
//!   player's own ragdoll's eye;
//! - `on_ray` (bullet::check_collision, the bullet's ray traced): the
//!   segment tested against the ragdolls; a hit corpse's part is pushed at
//!   the next step (once a bullet) and the game's body-hit particle and
//!   sound play at the hit; a living player's hit is kept for its death's
//!   push (once a bullet: its first segment through the body);
//! - `on_held_item` (mt_draw, before the held item is read): a corpse of
//!   ours holding nothing (a battery carrier's once the battery drops: the
//!   game then updates no bone) is posed from its ragdoll alone.
//! - `on_post_bones`: the content's extra render bone follows Head while
//!   worn, and the cosmetic headwear body after death;
//! - `on_render_bones`: ordinary 69-bone commands initialize the extra
//!   render bone for the lobby, baking and other renderer callers.
//!
//! The world is read once a frame into the crate's types (the places seen
//! alive or dead with their death's key, the bones' matrices as world
//! poses); written back are only the drawn bones of a ragdoll (mt_draw's
//! stack array) and the camera's eye (a matrix of its own). Nothing the
//! simulation keeps, saves or hashes is written.

#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicBool, Ordering};

use ::ragdoll::corpse::{Corpse, CorpseHit, Corpses, Death, DeathKey, Drawn, Frame, MAX_PLACES, Seen, SlotId};
use ::ragdoll::math::{Pose, Quat, Vec3, abs};
use ::ragdoll::ragdoll::{Cause, GunShape, HeadwearShape, Knock, Level, Skeleton, StartReport, Sweep};
use ::ragdoll::shot::{Hit, Recent, Shot, ShotKey, ShotLog, impulse};
use ::ragdoll::skeleton::{BONE_COUNT, BONES, PARTS, Part};

use crate::vostok::bullet::BulletRay;
use crate::vostok::draw::{self, DrawnBones};
use crate::vostok::trace::{self, TraceFile};
use crate::vostok::world::{BodyId, GameWorld, Held, NetworkClient, PhysicsWorld, Player, PlayerId, SkeletonId};
use crate::vostok::{Context, Flow, Lock, NoExit, RagdollOnHeldItemExit, RagdollOnPostBonesExit, Thread, camera, clock, headwear};

/// ray_test's filter (an object is hit when its mask has a bit of the
/// group and its group a bit of the mask): the foot IK's level (group 24h,
/// mask 2: the walker collision, cgroup 2 / cmask 4), the level's collision
/// that bullets hit (group 10h, mask 8: cgroup 8 / cmask 16, as the lobby's
/// camera ray), and the batteries (group 0A18h, mask 418h, put in by
/// victory_item_core 0x301CB0, lying or just dropped: bits 400h and 200h).
/// Nothing else: players 824h/3, hit models 40h/11h, grenades 818h/8 (their
/// group has 8h, their mask none of 434h), the lobby's 800h/800h and 20h/20h,
/// the client objects' 80h/100h and 1/20h. The walker mesh alone is coarse
/// and lies under the drawn ground on some maps (Rudnya: a median 4 cm, 10
/// cm at the 90th percentile, deeper than the gun's radius); the collision
/// follows the drawn ground, and the nearer of the two is the one met. The
/// ray only reads the world: a body rests on a battery, the battery is
/// never moved.
const LEVEL_GROUP: u32 = 0x24 | 0x10 | 0x400;
const LEVEL_MASK: u32 = 0x02 | 0x08 | 0x200;
// ray_test takes them as u16
const _: () = assert!(LEVEL_GROUP <= 0xFFFF && LEVEL_MASK <= 0xFFFF);
/// The level's rays a frame, for all ragdolls (a ray is a few microseconds).
const RAYS_A_FRAME: u32 = 160;
const UP: Vec3 = Vec3::Y;
/// The living hits kept between a frame's bullets and its step.
const LIVING_HITS: usize = 32;
/// The pushes kept between a frame's bullets and its step.
const PUSHES: usize = 32;
/// A living hit's key's place (no corpse's: a place is below MAX_PLACES).
const LIVING: u8 = 0xFE;
/// The trace's spikes (464f): a push past `SPIKE_PUSH` N s (one round's is
/// 27 at most), a death's whole-body knock past `SPIKE_KNOCK` m/s (one
/// full-power round's is 0.83) or capped, a part past `SPIKE_SPEED` m/s
/// after a hit (one hit's is at most its cap, 6, and the knock).
const SPIKE_PUSH: f32 = 30.0;
const SPIKE_KNOCK: f32 = 1.0;
const SPIKE_SPEED: f32 = 7.0;

/// A row-vector affine matrix (vostok's float4x4: rows i, j, k the axes,
/// row c the place) as a rigid pose; `None` when it is not one (a scale, a
/// mirror, NaN): such a skeleton is left as the game draws it.
pub(crate) fn pose_of(m: &[f32; 16]) -> Option<Pose> {
    let i = Vec3::new(m[0], m[1], m[2]);
    let j = Vec3::new(m[4], m[5], m[6]);
    let k = Vec3::new(m[8], m[9], m[10]);
    let c = Vec3::new(m[12], m[13], m[14]);
    let unit = |v: Vec3| abs(v.length() - 1.0) < 0.02;
    let rigid = unit(i) && unit(j) && unit(k) && abs(i.dot(j)) < 0.02 && abs(j.dot(k)) < 0.02
        && abs(i.dot(k)) < 0.02 && i.cross(j).dot(k) > 0.9 && c.is_finite();
    rigid.then(|| Pose::new(Quat::from_basis(i, j, k), c))
}

/// The pose as a row-vector affine matrix.
pub(crate) fn matrix_of(p: Pose) -> [f32; 16] {
    let i = p.rotation.rotate(Vec3::X);
    let j = p.rotation.rotate(Vec3::Y);
    let k = p.rotation.rotate(Vec3::Z);
    let c = p.position;
    [i.x, i.y, i.z, 0.0, j.x, j.y, j.z, 0.0, k.x, k.y, k.z, 0.0, c.x, c.y, c.z, 1.0]
}

fn vec3([x, y, z]: [f32; 3]) -> Vec3 {
    Vec3::new(x, y, z)
}

/// Where each of our bones is in a skeleton's matrix array (the game's bone
/// index - 1: object_movement, the root, has no matrix).
type BoneIndex = [u8; BONE_COUNT];

/// What a skeleton was found to be: the one human skeleton (ours, by name
/// and parent), or another (no ragdoll for it).
#[derive(Clone, Copy)]
enum Mapping {
    Empty,
    Ours { skeleton: SkeletonId, index: BoneIndex },
    Other { skeleton: SkeletonId },
}

/// The skeleton read once: every bone of ours found by name, each one's
/// parent the game's parent; `None` when it is not our skeleton.
fn read_skeleton(skeleton: &crate::vostok::world::Skeleton<'_>) -> Option<BoneIndex> {
    let total = skeleton.bone_count();
    if !(2..=80).contains(&total) {
        return None;
    }
    let mut index = [0u8; BONE_COUNT];
    let mut found = [false; BONE_COUNT];
    for g in 1..total {
        if !skeleton.bone_has_name(g) {
            return None;
        }
        if let Some(ours) = BONES.iter().position(|(n, _)| skeleton.bone_named(g, n)) {
            if found[ours] {
                return None;
            }
            found[ours] = true;
            index[ours] = (g - 1) as u8;
        }
    }
    if found.iter().any(|f| !f) {
        return None;
    }
    // the parents: the game's of each bone is ours (the root's is the root
    // motion's, bone 0)
    for (ours, (_, parent)) in BONES.iter().enumerate() {
        let expected = match parent {
            Some(p) => u32::from(index[*p as usize]) + 1,
            None => 0,
        };
        if skeleton.bone_parent(u32::from(index[ours]) + 1) != Some(expected) {
            return None;
        }
    }
    Some(index)
}

/// What the draws of a frame need of a place, made by the step.
#[derive(Clone, Copy)]
enum Draw {
    Off,
    On { player: PlayerId, key: DeathKey, slot: SlotId, index: BoneIndex },
}

/// The camera's eye for the local player, made by the step.
#[derive(Clone, Copy)]
enum EyeView {
    Off,
    On { player: PlayerId, matrix: [f32; 16] },
}

/// The frame clock (QueryPerformanceCounter).
#[derive(Clone, Copy)]
enum Clock {
    Unstarted,
    Running { ticks: u32, frequency: u32 },
}

/// A bullet's round as the trace says it: the ammo's mass and muzzle speed
/// (its identity), its speed at the hit, its segment, its id and shooter.
#[derive(Clone, Copy)]
struct Round {
    mass: f32,
    muzzle: f32,
    speed: f32,
    segment: u32,
    bullet: u32,
    shooter: u8,
}

/// A shot's push on a corpse, for the next step.
#[derive(Clone, Copy)]
struct Push {
    place: usize,
    death: DeathKey,
    part: Part,
    point: Vec3,
    impulse: Vec3,
    round: Round,
}

/// A bullet in a living player's hit model: the body, the hit, the round
/// (the step finds the place by the body).
#[derive(Clone, Copy)]
struct Living {
    body: BodyId,
    hit: Hit,
    round: Round,
}

/// The shots of the frame's bullets: the log of those applied, the pushes
/// to apply, the living hits to keep.
struct Shots {
    log: ShotLog,
    pushes: [Option<Push>; PUSHES],
    living: [Option<Living>; LIVING_HITS],
}

/// A place's motion while its player is alive: its transform's place and
/// velocity, the last frame's (the death carries it).
#[derive(Clone, Copy)]
struct Moving {
    at: Vec3,
    velocity: Vec3,
    known: bool,
}

/// The trace's file: not opened yet, open, or failed (no more tries).
#[derive(Clone, Copy)]
enum TraceState {
    Closed,
    Open(TraceFile),
    Failed,
}

struct State {
    corpses: Corpses,
    draws: [Draw; MAX_PLACES],
    eye: EyeView,
    mappings: [Mapping; 4],
    clock: Clock,
    /// The step's time, s (the frames' real times summed).
    time: f32,
    /// Each place's hits lately (the next death's push).
    recent: [Recent; MAX_PLACES],
    /// Each place's motion while alive (the next death's carried velocity).
    moving: [Moving; MAX_PLACES],
    /// Each place's last living hit's round (the trace's spike of its death).
    rounds: [Option<Round>; MAX_PLACES],
    /// Native killing-hit classification, scoped to each place's current life.
    deaths: [Option<(DeathKey, headwear::FatalHit)>; MAX_PLACES],
    trace: TraceState,
    shots: Shots,
}

/// The game has one world at a time; its logic thread runs the step, the
/// draws (one a place, maybe at once on several threads) and the bullets
/// (256 a task) take the lock in turn.
static STATE: Lock<State> = Lock::new(State {
    corpses: Corpses::new(),
    draws: [Draw::Off; MAX_PLACES],
    eye: EyeView::Off,
    mappings: [Mapping::Empty; 4],
    clock: Clock::Unstarted,
    time: 0.0,
    recent: [Recent::new(); MAX_PLACES],
    moving: [Moving { at: Vec3::ZERO, velocity: Vec3::ZERO, known: false }; MAX_PLACES],
    rounds: [None; MAX_PLACES],
    deaths: [None; MAX_PLACES],
    trace: TraceState::Closed,
    shots: Shots { log: ShotLog::new(), pushes: [None; PUSHES], living: [None; LIVING_HITS] },
});

/// Set by the step when some place has a ragdoll a bullet could meet: the
/// bullets' hook returns at once otherwise, the lock not taken.
static ANY_BODIES: AtomicBool = AtomicBool::new(false);

/// The real time since the last frame, s (1/60 the first time).
fn frame_time(clock: &mut Clock, thread: &Thread) -> f32 {
    let ticks = clock::counter(thread) as u32;
    match *clock {
        Clock::Running { ticks: last, frequency } => {
            *clock = Clock::Running { ticks, frequency };
            ticks.wrapping_sub(last) as f32 / frequency as f32
        }
        Clock::Unstarted => {
            let frequency = clock::frequency(thread);
            if frequency > 0 && frequency <= u64::from(u32::MAX) {
                *clock = Clock::Running { ticks, frequency: frequency as u32 };
            }
            1.0 / 60.0
        }
    }
}

/// The level as the ragdolls read it: ray_test on the simulation's physics
/// world, the level's group alone, a budget of rays a frame.
struct GameLevel<'t> {
    world: Option<PhysicsWorld<'t>>,
    rays: u32,
}

impl Level for GameLevel<'_> {
    fn sweep(&mut self, from: Vec3, to: Vec3, radius: f32) -> Sweep {
        let Some(world) = &self.world else { return Sweep::Deferred };
        if self.rays == 0 {
            return Sweep::Deferred;
        }
        self.rays -= 1;
        let along = to - from;
        let Some(direction) = along.direction() else { return Sweep::Clear };
        let origin = [from.x, from.y, from.z];
        let direction = [direction.x, direction.y, direction.z];
        match world.ray_test(origin, direction, along.length() + radius, LEVEL_GROUP, LEVEL_MASK) {
            None => Sweep::Clear,
            Some(hit) => Sweep::Hit { point: vec3(hit.point), normal: vec3(hit.normal) },
        }
    }
}

/// The bones of ours of a skeleton, from the cache or read now.
fn mapping(mappings: &mut [Mapping; 4], skeleton: &crate::vostok::world::Skeleton<'_>) -> Option<BoneIndex> {
    let id = skeleton.id();
    for m in mappings.iter() {
        match *m {
            Mapping::Ours { skeleton: s, index } if s == id => return Some(index),
            Mapping::Other { skeleton: s } if s == id => return None,
            _ => {}
        }
    }
    let read = read_skeleton(skeleton);
    mappings.rotate_right(1);
    mappings[0] = match read {
        Some(index) => Mapping::Ours { skeleton: id, index },
        None => Mapping::Other { skeleton: id },
    };
    read
}

/// What the player wears and carries, kg on each part: each inventory
/// slot's weight (a piece's times a stack's count) on the parts it sits on
/// (the helmet and the mask on the head, the torso's armour on the chest,
/// the arms and the pelvis, the back's on the chest, the legs' on the pelvis
/// and the legs, the gloves on the forearms, the boots on the shins, the
/// weapons and the rest half on the chest, half on the pelvis).
fn load_of(player: &Player<'_>) -> [f32; PARTS] {
    use Part::*;
    let mut load = [0.0f32; PARTS];
    let shares: [&[(Part, f32)]; 8] = [
        &[(Head, 1.0)],
        &[(Head, 1.0)],
        &[(Torso, 0.7), (LeftUpperArm, 0.1), (RightUpperArm, 0.1), (Pelvis, 0.1)],
        &[(Torso, 1.0)],
        &[(Pelvis, 0.3), (LeftThigh, 0.25), (RightThigh, 0.25), (LeftShin, 0.1), (RightShin, 0.1)],
        &[(LeftForearm, 0.5), (RightForearm, 0.5)],
        &[(LeftShin, 0.5), (RightShin, 0.5)],
        &[(Torso, 0.5), (Pelvis, 0.5)],
    ];
    player.slot_weights(|slot, weight| {
        let Some(weight) = weight else { return };
        if !weight.is_finite() || !(0.0..=50.0).contains(&weight) {
            return;
        }
        for &(part, share) in shares[slot.min(7)] {
            load[part.index()] += weight * share;
        }
    });
    load
}

/// What the world shows of a player: alive, or dead with its death (and
/// the skeleton's bones of ours). A player not inserted, or with another
/// skeleton, is `Alive` here: nothing of ours is drawn.
fn seen(
    player: &Player<'_>,
    thread: &Thread,
    mappings: &mut [Mapping; 4],
    recent: &Recent,
    moving: &Moving,
    fatal: Option<(DeathKey, headwear::FatalHit)>,
    now: f32,
) -> (Seen, Option<BoneIndex>) {
    if !player.inserted() || player.alive() {
        return (Seen::Alive, None);
    }
    let Some(skeleton) = player.skeleton() else { return (Seen::Alive, None) };
    let Some(index) = mapping(mappings, &skeleton) else { return (Seen::Alive, None) };
    let stood_on = pose_of(&player.transform()).map(|p| (p.position, UP));
    // a gun drops only from the hands that held it: not a battery (the
    // simulation drops the battery itself, 0x300E50), nor a carrier's
    // lowered gun (the artefact in hand drops instead)
    let gun = (player.held() == Held::Item && crate::artefact_carry::gun_in_hands_at_death(player, thread))
        .then_some(GunShape::RIFLE);
    let mut death = Death::new(DeathKey(player.spawned()), Cause::Unknown, gun, stood_on);
    if fatal == Some((death.key, headwear::FatalHit::Headshot)) {
        death.headwear = headwear::of_player(player).map(|_| HeadwearShape);
    }
    (death.hits, death.hit_count) = recent.of_death(now);
    death.load = load_of(player);
    death.velocity = if moving.known { moving.velocity } else { Vec3::ZERO };
    (Seen::Dead(death), Some(index))
}

/// Record the native death event without changing simulation state.
pub fn on_killed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some((place, life, hit)) = headwear::killed(ctx) {
        STATE.with(ctx.thread(), |state| {
            if let Some(death) = state.deaths.get_mut(usize::from(place)) {
                *death = Some((life, hit));
            }
        });
    }
    Flow::Continue
}

/// The step: once a frame on the logic thread, after the simulation's move.
pub fn on_step(ctx: &mut Context<'_>) -> Flow<NoExit> {
    // patch 85's frame step (one detour at this address)
    crate::weapon_pickup::after_move(ctx);
    let thread = ctx.thread();
    let client = GameWorld::at_after_move(ctx).and_then(|world| world.network_client());
    let lone = STATE.with(thread, |state| {
        step(state, client.as_ref(), thread);
        // the guns bodies left behind (their players respawned), for patch
        // 85's drops drawn alone
        core::array::from_fn(|place| state.corpses.lone_gun(place)
            .map(|(death, pose)| crate::weapon_pickup::LoneGun { life: death.0, transform: matrix_of(pose) }))
    });
    crate::weapon_pickup::draw_alone(ctx, &lone);
    Flow::Continue
}

fn step(state: &mut State, client: Option<&NetworkClient<'_>>, thread: &Thread) {
    let dt = frame_time(&mut state.clock, thread);
    state.time += dt;
    let living = core::mem::replace(&mut state.shots.living, [None; LIVING_HITS]);
    // the pushes of the shots since the last step (the bullets' threads
    // are done: the simulation's move has returned)
    for push in state.shots.pushes.iter_mut() {
        if let Some(p) = push.take()
            && let Some(knock) = state.corpses.push(p.place, p.death, p.part, p.point, p.impulse)
            && (knock.push > SPIKE_PUSH || knock.speed > SPIKE_SPEED)
        {
            spike_of_push(&mut state.trace, thread, state.time, p.place, &p.round, &knock);
        }
    }
    let mut seen_places = [Seen::Alive; MAX_PLACES];
    let mut players: [Option<(Player<'_>, Option<BoneIndex>)>; MAX_PLACES] = [None; MAX_PLACES];
    let mut count = 0;
    let mut local = None;
    let mut physics = None;
    if let Some(client) = client {
        count = (client.places() as usize).min(MAX_PLACES);
        let local_place = client.local_place().map(usize::from);
        for place in 0..count {
            let Some(player) = client.player(place as u32) else {
                state.deaths[place] = None;
                continue;
            };
            if !player.inserted() || player.alive() {
                state.deaths[place] = None;
            }
            physics = physics.or(player.physics_world());
            // this frame's bullets in this player (its hit model's body)
            let body = player.hit_body();
            for l in living.iter().flatten() {
                if body == Some(l.body) {
                    state.recent[place].record(state.time, l.hit);
                    state.rounds[place] = Some(l.round);
                    let r = &l.round;
                    trace_line(&mut state.trace, thread, |w| {
                        w.text("hit,");
                        w.fields(&[(state.time, 3), (place as f32, 0), (r.mass, 4), (r.speed, 1),
                                   (l.hit.impulse.length(), 2), (r.muzzle, 0), (r.segment as f32, 0),
                                   (r.bullet as f32, 0), (f32::from(r.shooter), 0)]);
                    });
                }
            }
            // its motion while alive (its transform's place)
            if player.inserted() && player.alive() {
                let at = player.transform();
                let at = Vec3::new(at[12], at[13], at[14]);
                let m = &mut state.moving[place];
                let step = at - m.at;
                m.velocity = if m.known && dt > 1e-4 && step.length() < 2.0 && at.is_finite() {
                    (m.velocity + step * (1.0 / dt)) * 0.5
                } else {
                    Vec3::ZERO
                };
                m.at = at;
                m.known = at.is_finite();
            }
            let (s, index) = seen(&player, thread, &mut state.mappings, &state.recent[place], &state.moving[place],
                                  state.deaths[place], state.time);
            seen_places[place] = s;
            players[place] = Some((player, index));
            if local_place == Some(place) {
                local = Some((place, player));
            }
        }
    }
    state.deaths[count..].fill(None);
    let frame = Frame { dt, up: UP, places: &seen_places[..count], local: local.map(|(p, _)| p) };
    let mut level = GameLevel { world: physics, rays: RAYS_A_FRAME };
    state.corpses.step(&frame, &mut level);
    for (place, r) in state.corpses.started() {
        trace_line(&mut state.trace, thread, |w| {
            w.text("death,");
            w.fields(&[
                (state.time, 3),
                (place as f32, 0),
                (r.cause as f32, 0),
                (r.hits as f32, 0),
                (r.push, 2),
                (r.knock, 2),
                (r.mass, 1),
                (r.speed, 2),
                (r.strengths[0], 2),
                (r.strengths[1], 2),
                (r.strengths[2], 2),
                (r.strengths[3], 2),
            ]);
        });
        if r.capped || r.knock > SPIKE_KNOCK || r.push > SPIKE_PUSH || r.peak_speed > SPIKE_SPEED {
            spike_of_death(&mut state.trace, thread, state.time, place, state.rounds[place].as_ref(), &r);
        }
    }
    for (place, player) in players.iter().enumerate() {
        state.draws[place] = match (state.corpses.place(place), *player) {
            (Corpse::Body { death, slot }, Some((player, Some(index)))) => {
                Draw::On { player: player.id(), key: death, slot, index }
            }
            _ => Draw::Off,
        };
    }
    state.eye = match (state.corpses.eye(), local) {
        (Some(eye), Some((_, player))) => EyeView::On { player: player.id(), matrix: matrix_of(eye) },
        _ => EyeView::Off,
    };
    ANY_BODIES.store(state.corpses.any_bodies(), Ordering::Release);
}

/// A death's spike line: its killing round (the place's last living hit),
/// its hits summed, its fastest part, its kit, its push, that part's speed,
/// whether the body's knock was capped.
fn spike_of_death(file: &mut TraceState, thread: &Thread, time: f32, place: usize, round: Option<&Round>,
                  r: &StartReport) {
    let body: f32 = Part::ALL.iter().map(|p| p.mass()).sum();
    let none = Round { mass: f32::NAN, muzzle: f32::NAN, speed: f32::NAN, segment: 0, bullet: 0, shooter: 0 };
    let round = round.unwrap_or(&none);
    trace_line(file, thread, |w| {
        w.text("spike,");
        w.fields(&[(time, 3), (place as f32, 0)]);
        w.text(",death,");
        w.fields(&[(round.mass, 4), (round.speed, 1), (round.muzzle, 0), (round.segment as f32, 0),
                   (f32::from(r.hits), 0), (r.peak.index() as f32, 0), (r.mass - body, 1), (r.push, 2),
                   (r.peak_speed, 2), (u8::from(r.capped) as f32, 0)]);
    });
}

/// A corpse push's spike line: its round, the part, the part's mass (its
/// kit in), the push, the part's speed after, whether its speed was capped.
fn spike_of_push(file: &mut TraceState, thread: &Thread, time: f32, place: usize, round: &Round, k: &Knock) {
    trace_line(file, thread, |w| {
        w.text("spike,");
        w.fields(&[(time, 3), (place as f32, 0)]);
        w.text(",push,");
        w.fields(&[(round.mass, 4), (round.speed, 1), (round.muzzle, 0), (round.segment as f32, 0), (1.0, 0),
                   (k.part.index() as f32, 0), (k.mass, 1), (k.push, 2), (k.speed, 2),
                   (u8::from(k.capped) as f32, 0)]);
    });
}

/// The trace's file name, in the user data folder.
const TRACE_FILE: &str = "srp_ragdoll.csv";

/// A line of the opt-in trace (nothing unless the build turned it on, a
/// local one: vostok::trace): the file opened at the first line (its column
/// names first), the line written at once.
fn trace_line(file: &mut TraceState, thread: &Thread, line: impl FnOnce(&mut Line)) {
    if !trace::enabled() {
        return;
    }
    if let TraceState::Closed = file {
        *file = match TraceFile::open(thread, TRACE_FILE) {
            None => TraceState::Failed,
            Some(handle) => {
                let mut head = Line::new();
                head.text(
                    "# srp ragdoll trace: hit,time s,place,mass kg,speed m/s,push N s,muzzle m/s,segment,bullet,\
                     shooter | death,time s,place,cause (0 none 1 body 2 head 3 blast),hits,push N s,knock m/s,mass \
                     kg,speed m/s,legs,waist,neck,arms (the muscles' strength at the start) | spike,time s,place,\
                     death or push,mass kg,speed m/s,muzzle m/s,segment,hits,part,kit kg (a push: the part's mass),\
                     push N s,part speed m/s,capped\r\n",
                );
                handle.write(thread, head.bytes());
                TraceState::Open(handle)
            }
        };
    }
    if let TraceState::Open(handle) = *file {
        let mut text = Line::new();
        line(&mut text);
        text.text("\r\n");
        handle.write(thread, text.bytes());
    }
}

/// A line of the trace, at most 512 bytes.
struct Line {
    bytes: [u8; 512],
    used: usize,
}

impl Line {
    fn new() -> Line {
        Line { bytes: [0; 512], used: 0 }
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes[..self.used]
    }

    fn push(&mut self, b: u8) {
        if self.used < self.bytes.len() {
            self.bytes[self.used] = b;
            self.used += 1;
        }
    }

    fn text(&mut self, text: &str) {
        for &b in text.as_bytes() {
            self.push(b);
        }
    }

    /// A number with `decimals` places (core's float formatting needs a
    /// 64-bit division the arena has not; a value past 10^6 is shown so).
    fn number(&mut self, value: f32, decimals: u32) {
        if !value.is_finite() {
            self.text("nan");
            return;
        }
        let scale = [1.0, 10.0, 100.0, 1000.0, 10000.0][decimals.min(4) as usize];
        let v = if value < 0.0 {
            self.push(b'-');
            -value
        } else {
            value
        };
        let v = if v > 1.0e6 { 1.0e6 } else { v };
        let scaled = (v * scale + 0.5) as u32;
        let (whole, part) = (scaled / scale as u32, scaled % scale as u32);
        self.int(whole);
        if decimals > 0 {
            self.push(b'.');
            let mut digits = [b'0'; 4];
            let mut p = part;
            for d in (0..decimals.min(4) as usize).rev() {
                digits[d] = b'0' + (p % 10) as u8;
                p /= 10;
            }
            for &d in &digits[..decimals.min(4) as usize] {
                self.push(d);
            }
        }
    }

    fn int(&mut self, mut n: u32) {
        let mut digits = [0u8; 10];
        let mut k = 0;
        loop {
            digits[k] = b'0' + (n % 10) as u8;
            k += 1;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        for i in (0..k).rev() {
            self.push(digits[i]);
        }
    }

    /// Comma-separated numbers.
    fn fields(&mut self, values: &[(f32, u32)]) {
        for (i, &(v, d)) in values.iter().enumerate() {
            if i > 0 {
                self.push(b',');
            }
            self.number(v, d);
        }
    }
}

/// A bullet's ray traced (its result not yet read): the segment, cut at
/// what the game's ray hit, is tested against the ragdolls; a hit is pushed
/// at the next step, once a shot, and shows the game's body-hit effects.
/// Reads the game's memory only (and calls the bullet manager's own effect
/// queues), so the bullet goes on exactly as before.
pub fn on_ray(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Some(ray) = BulletRay::at(ctx) {
        bullet(&ray, thread);
    }
    Flow::Continue
}

fn bullet(ray: &BulletRay<'_>, thread: &Thread) {
    let from = vec3(ray.origin());
    let direction = vec3(ray.direction());
    let shooter = ray.shooter_place();
    // one push a bullet a victim, whatever its segment (shot.rs ShotKey)
    let key = |place: u8, death: u32| ShotKey { shooter: shooter.unwrap_or(u8::MAX), bullet: ray.id(), place, death };
    // the bullet's round: its ammo's mass (kg) and its speed now (m/s, its
    // segment's)
    let round = Round {
        mass: ray.ammo_mass(),
        muzzle: ray.muzzle_speed(),
        speed: vec3(ray.velocity()).length(),
        segment: ray.segment(),
        bullet: ray.id(),
        shooter: shooter.unwrap_or(u8::MAX),
    };
    let push = |direction: Vec3| impulse(direction, round.mass, round.speed);
    // a living player hit: kept for its death's push, once a bullet
    if let Some(body) = ray.struck_body()
        && let Some(point) = ray.struck()
        && let Some(unit) = direction.direction()
    {
        let living = Living { body, hit: Hit { point: vec3(point), impulse: push(unit) }, round };
        STATE.with(thread, |state| take_living(&mut state.shots, key(LIVING, body.key()), living));
    }
    if !ANY_BODIES.load(Ordering::Acquire) {
        return;
    }
    // the game's ray stops at what it hit: the living, a wall
    let reach = match ray.struck() {
        None => ray.length(),
        Some(point) => (vec3(point) - from).length(),
    };
    let Some(shot) = Shot::new(from, direction, reach) else { return };
    let taken = STATE.with(thread, |state| {
        let hit = state.corpses.trace(&shot)?;
        take_shot(&mut state.shots, key(hit.place as u8, hit.death.0), &hit, push(shot.direction), round).then_some(hit)
    });
    if let Some(hit) = taken
        && shooter.is_some()
    {
        let v = |v: Vec3| [v.x, v.y, v.z];
        ray.play_body_hit(v(hit.hit.point), v(shot.direction), v(hit.hit.normal));
    }
}

/// A living player's hit kept for the step, if it is the shot's first trace.
fn take_living(shots: &mut Shots, key: ShotKey, living: Living) {
    if let Some(free) = shots.living.iter_mut().find(|h| h.is_none())
        && shots.log.first(key)
    {
        *free = Some(living);
    }
}

/// The shot's push queued for the next step, if it is the shot's first
/// trace (a rollback's re-simulated tick traces it again) and a push is
/// free. True if taken.
fn take_shot(shots: &mut Shots, key: ShotKey, hit: &CorpseHit, j: Vec3, round: Round) -> bool {
    match shots.pushes.iter_mut().find(|p| p.is_none()) {
        Some(free) if shots.log.first(key) => {
            *free = Some(Push {
                place: hit.place,
                death: hit.death,
                part: hit.hit.part,
                point: hit.hit.point,
                impulse: j,
                round,
            });
            true
        }
        _ => false,
    }
}

/// The draw of this player, if a corpse of ours: the step's entry, the
/// player still dead in the same life.
fn draw_of(state: &State, player: &Player<'_>) -> Option<(DeathKey, SlotId, BoneIndex)> {
    state.draws.iter().find_map(|d| match *d {
        Draw::On { player: p, key, slot, index }
            if p == player.id() && player.inserted() && !player.alive() && player.spawned() == key.0 =>
        {
            Some((key, slot, index))
        }
        _ => None,
    })
}

/// mt_draw's cull: a corpse of ours is drawn even when its model was not
/// seen the last frame (its bones move).
pub fn on_cull(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Flow::Skip = crate::artefact_carry::on_cull(ctx) { return Flow::Skip; }
    let ours = match draw::culled_player(ctx) {
        Some(player) => STATE.with(ctx.thread(), |state| draw_of(state, &player).is_some()),
        None => false,
    };
    if ours { draw::draw_anyway(ctx) } else { Flow::Continue }
}

/// mt_draw's bones: the model-space matrices of the skeleton. The clip is
/// captured; a ragdoll's bones are written over it.
pub fn on_bones(ctx: &mut Context<'_>) -> Flow<NoExit> {
    crate::artefact_carry::on_bones(ctx);
    if let Some(bones) = DrawnBones::at(ctx) {
        STATE.with(ctx.thread(), |state| apply(state, &bones));
    }
    Flow::Continue
}

/// The rendered accessory pose of the same death, in the player's model space.
fn headwear_pose(state: &State, player: &Player<'_>) -> Option<[f32; 16]> {
    let (key, slot, _) = draw_of(state, player)?;
    if *state.deaths.get(usize::from(player.place()))? != Some((key, headwear::FatalHit::Headshot)) {
        return None;
    }
    let pose = state.corpses.slot(slot).body()?.headwear_pose()?;
    Some(matrix_of(pose_of(&player.transform())?.inverse().then(pose)))
}

/// mt_draw's render command, after its weapon draw and before any arguments
/// have been pushed. Alive/canned bodies use Head; a ragdoll uses its own pose.
pub fn on_post_bones(ctx: &mut Context<'_>) -> Flow<RagdollOnPostBonesExit> {
    let posted = DrawnBones::at_post_bones(ctx).is_some_and(|bones| {
        let extra = STATE.with(ctx.thread(), |state| headwear_pose(state, &bones.player()));
        bones.post_headwear(extra)
    });
    if posted { Flow::Exit(RagdollOnPostBonesExit::Posted) } else { Flow::Continue }
}

pub fn on_render_bones(ctx: &mut Context<'_>) -> Flow<NoExit> {
    headwear::finish_update(ctx);
    Flow::Continue
}

fn apply(state: &mut State, bones: &DrawnBones<'_>) {
    let player = bones.player();
    let Some((_, slot, index)) = draw_of(state, &player) else { return };
    if index.iter().any(|&i| u32::from(i) >= bones.count()) {
        return;
    }
    let Some(transform) = pose_of(&player.transform()) else { return };
    let mut skeleton: Skeleton = [Pose::IDENTITY; BONE_COUNT];
    for (b, &i) in index.iter().enumerate() {
        let Some(model) = bones.get(u32::from(i)).as_ref().and_then(pose_of) else { return };
        skeleton[b] = transform.then(model);
    }
    if state.corpses.slot_mut(slot).draw(&mut skeleton) == Drawn::Ragdoll {
        let back = transform.inverse();
        for (b, &i) in index.iter().enumerate() {
            bones.set(u32::from(i), matrix_of(back.then(skeleton[b])));
        }
    }
}

/// mt_draw before it reads the held item: a corpse of ours holding nothing
/// (mt_draw would update no bone: the `je` after the span) is posed from its
/// ragdoll alone and given to the renderer as mt_draw gives its own; then
/// the draw goes on where that `je` goes.
pub fn on_held_item(ctx: &mut Context<'_>) -> Flow<RagdollOnHeldItemExit> {
    // the bones made under the lock, given to the renderer after it
    let posed = draw::unheld_player(ctx).is_some_and(|player| {
        STATE.with(ctx.thread(), |state| {
            pose_unheld(state, &player).map(|b| (b, headwear_pose(state, &player)))
        }).is_some_and(|(b, extra)| {
            draw::post_headwear(&player, &b, extra) || draw::post_bones(&player, &b)
        })
    });
    if posed {
        draw::no_held_item(ctx);
        Flow::Exit(RagdollOnHeldItemExit::NoBones)
    } else {
        Flow::Continue
    }
}

/// The ragdoll's bones of a corpse of ours that mt_draw made none for, in
/// the game's order (model space); None when it is not one (a dying body:
/// its clip not captured; a skeleton with bones not ours).
fn pose_unheld(state: &State, player: &Player<'_>) -> Option<[[f32; 16]; BONE_COUNT]> {
    let (_, slot, index) = draw_of(state, player)?;
    let transform = pose_of(&player.transform())?;
    // every matrix of the game's is one of ours (index: the game's bone - 1)
    let count = player.skeleton().map_or(0, |s| s.bone_count() as usize).saturating_sub(1);
    if count != BONE_COUNT || count > draw::MAX_BONES {
        return None;
    }
    let mut skeleton: Skeleton = [Pose::IDENTITY; BONE_COUNT];
    if state.corpses.slot(slot).draw_alone(&mut skeleton) != Drawn::Ragdoll {
        return None;
    }
    let back = transform.inverse();
    let mut bones = [[0.0f32; 16]; BONE_COUNT];
    for (b, &i) in index.iter().enumerate() {
        bones[usize::from(i)] = matrix_of(back.then(skeleton[b]));
    }
    Some(bones)
}

/// update_camera's eye: the local player's ragdoll's eye in the first
/// person, while it has one.
pub fn on_eye(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let eye = match camera::eye_player(ctx) {
        Some((player, true)) => STATE.with(ctx.thread(), |state| match state.eye {
            EyeView::On { player: p, matrix } if p == player.id() && draw_of(state, &player).is_some() => Some(matrix),
            _ => None,
        }),
        _ => None,
    };
    if let Some(matrix) = eye {
        camera::look_from(ctx, matrix);
    }
    Flow::Continue
}

const _: () = assert!(BONE_COUNT < u8::MAX as usize);

#[cfg(test)]
mod tests {
    use super::*;

    /// Bullet's filter as the game's needsCollision (0x75BD0) applies it.
    fn meets(group: u32, mask: u32) -> bool {
        group & LEVEL_MASK != 0 && mask & LEVEL_GROUP != 0
    }

    #[test]
    fn the_level_filter_meets_the_level_and_the_batteries_alone() {
        // a project's collision_objects (cgroup, cmask): the walker
        // collision, the collision bullets hit, one used for both
        assert!(meets(2, 4) && meets(8, 16) && meets(10, 20));
        // the batteries (victory_item_core 0x301CB0)
        assert!(meets(0xA18, 0x418));
        // players' controllers, hit models, grenades, the lobby's, the
        // client objects' geometries
        for (group, mask) in [(0x824, 3), (0x40, 0x11), (0x818, 8), (0x800, 0x800), (0x20, 0x20), (0x80, 0x100), (1, 0x20)] {
            assert!(!meets(group, mask), "{group:#x}/{mask:#x}");
        }
    }
}

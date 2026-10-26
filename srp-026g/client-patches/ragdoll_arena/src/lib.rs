//! Patch 76: the death ragdoll's client side (`docs/notes/ragdoll.md`).
//!
//! `ragdoll_026g.asm` has the four hooks' entries; they align the stack and
//! call the functions here. This file is the only code that knows the
//! game's raw fields: it reads them once a frame into the crate `ragdoll`'s
//! types (the places seen alive or dead with their death's key, the bones'
//! matrices as world poses) and writes back only the drawn bones of a
//! ragdoll (mt_draw's stack array) and the camera's eye (a matrix of its
//! own, given to the view in place of player+44Ch). Nothing the simulation
//! keeps, saves or hashes is written.
//!
//! - `ragdoll_step` (game_world::tick, after the simulation's move): the
//!   corpses' machine, the ragdolls stepped, the level read by ray_test;
//! - `ragdoll_draws` (mt_draw's cull): a corpse of ours is always drawn;
//! - `ragdoll_apply` (mt_draw, the bones made, the "Weapon" bone not yet
//!   read): the clip captured, the ragdoll's bones written;
//! - `ragdoll_eye` (update_camera, the eye given to the view): the local
//!   player's own ragdoll's eye, followed smoothly;
//! - `ragdoll_bullet` (bullet::check_collision, the bullet's ray traced):
//!   the segment tested against the ragdolls; a hit corpse's part is pushed
//!   at the next step (once a shot) and the game's body-hit particle and
//!   sound play at the hit (`shot_effects`).

#![no_std]

use core::ffi::c_void;
use core::ptr::{null, read_volatile, write_volatile};
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use ragdoll::corpse::{Corpse, CorpseHit, Corpses, Death, DeathKey, Drawn, Frame, MAX_PLACES, Seen, SlotId};
use ragdoll::math::{Pose, Quat, Vec3, abs};
use ragdoll::ragdoll::{Cause, GunShape, Level, Skeleton, Sweep};
use ragdoll::shot::{Hit, Recent, Shot, ShotKey, ShotLog, impulse};
use ragdoll::skeleton::{BONE_COUNT, BONES, PARTS, Part};

mod engine {
    include!("../../engine_026g.rs");
}

// The game's fields (ragdoll.md section 8; the re-kit rows of patch 76).
/// game_world: the game.
const GAME: usize = 0x9C;
/// game: the network client.
const CLIENT: usize = 0x53AC;
/// network client: the local player (patch 46's LOCAL).
const LOCAL_PLAYER: usize = 0x4;
/// network client's vtable (0x9FFB14): match_options 0x219990 (thiscall,
/// no arguments) and get_active_player 0x27A9C0 (thiscall, out, place; a
/// reference added).
const MATCH_OPTIONS_SLOT: usize = 0x34;
const ACTIVE_PLAYER_SLOT: usize = 0x50;
/// match options: the places' count (u8).
const PLACES: usize = 0x577B5;
/// player
const REFERENCES: usize = 0xD0;
const PLACE: usize = 0x114;
const HELD_ITEM: usize = 0x128;
const SPAWNED: usize = 0x328; // m_inserted_time_in_ms: the life's spawn time
const ALIVE: usize = 0x33C; // m_is_alive
const INSERTED: usize = 0x33F; // m_has_been_inserted
const PHYSICS_WORLD: usize = 0x350;
const TRANSFORM: usize = 0x40C; // the simulated transform (4x4)
const EYE: usize = 0x44C; // the camera's transform: bone "Item" x +40Ch
const MODEL: usize = 0xF290;
/// model: the skeleton.
const SKELETON: usize = 0x10C;
/// skeleton: the bones' count, their array (1Ch each: +0 name, +4 parent).
const BONE_TOTAL: usize = 0x108;
const BONE_ARRAY: usize = 0x110;
const BONE_SIZE: usize = 0x1C;
/// The imports' slots: QueryPerformanceCounter, QueryPerformanceFrequency.
const QUERY_COUNTER: usize = 0x870158;
const QUERY_FREQUENCY: usize = 0x870150;
/// ray_test's filter (an object is hit when its mask has a bit of the
/// group and its group a bit of the mask): the foot IK's level (group 24h,
/// mask 2: walkable geometry, no players 824h/3, hit models 40h/11h or
/// grenades 818h/8), and the batteries (group 0A18h, mask 418h, put in by
/// victory_item_core 0x301CB0, lying or just dropped): bits 400h and 200h,
/// which no other object of the world has. The ray only reads the world:
/// a body rests on a battery, the battery is never moved.
const LEVEL_GROUP: u32 = 0x24 | 0x400;
const LEVEL_MASK: u32 = 0x02 | 0x200;
/// ray_test's result: the hit point, the normal (toward the ray's start
/// when the last argument is 0), the object (0: no hit).
const HIT_POINT: usize = 0x0;
const HIT_NORMAL: usize = 0x10;
const HIT_OBJECT: usize = 0x1C;
/// The level's rays a frame, for all ragdolls (a ray is a few microseconds).
const RAYS_A_FRAME: u32 = 160;
const UP: Vec3 = Vec3::Y;

// A bullet (`ragdoll_bullet`; the re-kit rows of check_collision 0x32D4E0).
const BULLET_ID: usize = 0x4;
/// The shooter (0: none; the game then plays no hit effects): +4 its place.
const BULLET_SHOOTER: usize = 0x8;
const SHOOTER_PLACE: usize = 0x4;
/// The bullet's ammo data (the ammo config's `data`, loaded by 0x332440):
/// +14h its `mass` (kg, a gameplay scale), +3Ch its `muzzle_speed`.
const BULLET_AMMO: usize = 0x40;
const AMMO_MASS: usize = 0x14;
/// The bullet's velocity, its segment's start (PROVEN: the ctor 0x32BA60
/// writes the fire velocity to +1Ch and +34h, change_trajectory 0x32EE20 a
/// pierce's or a ricochet's new one to both; check_ragdoll_026g.py runs
/// change_trajectory); +34h is what compute_trajectory_velocity reads.
const BULLET_VELOCITY: usize = 0x1C;
const BULLET_MANAGER: usize = 0x44;
const BULLET_MATERIAL: usize = 0x48;
/// The game's own condition for a hit's sound (process_front_face_piercing
/// 0x32E6CC): +80h clear and at most one segment before.
const BULLET_QUIET: usize = 0x80;
const BULLET_SEGMENT: usize = 0x84;
/// bullet manager: the renderer (0: no effects), the game materials.
const MANAGER_RENDERER: usize = 0x90;
const MANAGER_MATERIALS: usize = 0x94;
/// game material: its id.
const MATERIAL_ID: usize = 0x64;
/// The material a living player's hit model is to bullets (bt_animated_rigid_body
/// 0x70E70 sets every hit body's +38h to 10, `actor`): the pair bullet x actor
/// plays fx/material_hits/blod and body_hits.
const ACTOR: u32 = 10;
/// The material pair: the sound's slot, the particles' slots (begin, end),
/// their parameter (process_front_face_piercing's, a body's way).
const PAIR_SOUND: usize = 0x18;
const PAIR_PARTICLES: usize = 0x24;
const PAIR_PARTICLES_END: usize = 0x28;
const PAIR_PARAMETER: usize = 0x74;
/// The ray's result (ray_test's): the object, 0 when nothing was hit.
const RESULT_OBJECT: usize = 0x1C;
/// A living player's hit model in the physics world: a
/// bt_animated_rigid_body (vtable 0x9E2AF4) whose material (+38h, u16) is
/// actor, 10 (its ctor 0x70E70); the player's is [[player+498h]+12Ch]
/// (activate_physics 0x2D9930 adds that to the world).
const ANIMATED_BODY_VTABLE: u32 = 0x9E2AF4;
const BODY_MATERIAL: usize = 0x38;
const HIT_MODEL: usize = 0x498;
const HIT_MODEL_BODY: usize = 0x12C;
/// player: its inventory; inventory: its 23 slots (item pointers, profile
/// slots 0..6 helmet, mask, torso, back, legs, gloves, boots, then the
/// weapons and the rest); an item: its weight a piece (kg, f32), its
/// dictionary entry (+22Ah set: a stack, its count u16 +260h): what
/// inventory::update_weight 0x2FC6E0 sums into m_carried_weight (+180h).
const INVENTORY: usize = 0x10C;
const INVENTORY_SLOTS: usize = 0x10C;
const SLOTS: usize = 23;
const ITEM_WEIGHT: usize = 0x218;
const ITEM_ENTRY: usize = 0x25C;
const ENTRY_STACK: usize = 0x22A;
const ITEM_COUNT: usize = 0x260;
/// The living hits kept between a frame's bullets and its step.
const LIVING_HITS: usize = 32;
/// The pushes kept between a frame's bullets and its step.
const PUSHES: usize = 32;

/// A game address (the exe is never rebased; the arena is 32-bit).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Addr(u32);

impl Addr {
    fn at(self, offset: usize) -> usize {
        self.0 as usize + offset
    }

    unsafe fn u8(self, offset: usize) -> u8 {
        unsafe { read_volatile(self.at(offset) as *const u8) }
    }

    unsafe fn u32(self, offset: usize) -> u32 {
        unsafe { read_volatile(self.at(offset) as *const u32) }
    }

    unsafe fn pointer(self, offset: usize) -> Option<Addr> {
        match unsafe { self.u32(offset) } {
            0 => None,
            a => Some(Addr(a)),
        }
    }

    unsafe fn matrix(self, offset: usize) -> [f32; 16] {
        unsafe { read_volatile(self.at(offset) as *const [f32; 16]) }
    }
}

/// A row-vector affine matrix (vostok's float4x4: rows i, j, k the axes,
/// row c the place) as a rigid pose; `None` when it is not one (a scale, a
/// mirror, NaN): such a skeleton is left as the game draws it.
fn pose_of(m: &[f32; 16]) -> Option<Pose> {
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
fn matrix_of(p: Pose) -> [f32; 16] {
    let i = p.rotation.rotate(Vec3::X);
    let j = p.rotation.rotate(Vec3::Y);
    let k = p.rotation.rotate(Vec3::Z);
    let c = p.position;
    [i.x, i.y, i.z, 0.0, j.x, j.y, j.z, 0.0, k.x, k.y, k.z, 0.0, c.x, c.y, c.z, 1.0]
}

/// Where each of our bones is in a skeleton's matrix array (the game's bone
/// index - 1: object_movement, the root, has no matrix).
type BoneIndex = [u8; BONE_COUNT];

/// What a skeleton was found to be: the one human skeleton (ours, by name
/// and parent), or another (no ragdoll for it).
#[derive(Clone, Copy)]
enum Mapping {
    Empty,
    Ours { skeleton: Addr, index: BoneIndex },
    Other { skeleton: Addr },
}

/// A C string of the game's against a name of ours.
unsafe fn same_name(at: Addr, name: &str) -> bool {
    let bytes = name.as_bytes();
    for (n, &b) in bytes.iter().enumerate() {
        if unsafe { read_volatile((at.0 as usize + n) as *const u8) } != b {
            return false;
        }
    }
    unsafe { read_volatile((at.0 as usize + bytes.len()) as *const u8) == 0 }
}

/// The skeleton read once: every bone of ours found by name, each one's
/// parent the game's parent; `None` when it is not our skeleton.
unsafe fn read_skeleton(skeleton: Addr) -> Option<BoneIndex> {
    let total = unsafe { skeleton.u32(BONE_TOTAL) } as usize;
    if !(2..=80).contains(&total) {
        return None;
    }
    let base = skeleton.at(BONE_ARRAY);
    let mut index = [0u8; BONE_COUNT];
    let mut found = [false; BONE_COUNT];
    for g in 1..total {
        let bone = Addr((base + g * BONE_SIZE) as u32);
        let name = Addr(unsafe { bone.u32(0) });
        if name.0 == 0 {
            return None;
        }
        if let Some(ours) = BONES.iter().position(|(n, _)| unsafe { same_name(name, n) }) {
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
        let bone = Addr((base + (index[ours] as usize + 1) * BONE_SIZE) as u32);
        let game_parent = unsafe { bone.u32(4) } as usize;
        let expected = match parent {
            Some(p) => base + (index[*p as usize] as usize + 1) * BONE_SIZE,
            None => base,
        };
        if game_parent != expected {
            return None;
        }
    }
    Some(index)
}

/// What the draws of a frame need of a place, made by the step.
#[derive(Clone, Copy)]
enum Draw {
    Off,
    On { player: Addr, key: DeathKey, slot: SlotId, index: BoneIndex },
}

/// The camera's eye for the local player, made by the step.
#[derive(Clone, Copy)]
enum EyeView {
    Off,
    On { player: Addr, matrix: [f32; 16] },
}

/// The frame clock (QueryPerformanceCounter).
#[derive(Clone, Copy)]
enum Clock {
    Unstarted,
    Running { ticks: u32, frequency: u32 },
}

/// A shot's push on a corpse, for the next step.
#[derive(Clone, Copy)]
struct Push {
    place: usize,
    death: DeathKey,
    part: Part,
    point: Vec3,
    impulse: Vec3,
}

/// The shots of the frame's bullets: the log of those applied, the pushes
/// to apply. The bullets may be ticked on several threads at once
/// (bullet_manager::tick, 256 a task), so `SHOTS_LOCK` guards these.
struct Shots {
    log: ShotLog,
    pushes: [Option<Push>; PUSHES],
    /// Bullets in living players' hit models: the hit model's body, the
    /// hit (the step finds the place by the body).
    living: [Option<(u32, Hit, f32, f32)>; LIVING_HITS],
}

/// A place's motion while its player is alive: its transform's place and
/// velocity, the last frame's (the death carries it).
#[derive(Clone, Copy)]
struct Moving {
    at: Vec3,
    velocity: Vec3,
    known: bool,
}

/// The opt-in diagnostics (README patch 76): 1 in a build made with
/// `--ragdoll-trace` (patch_026g.py writes it), never in a release; then
/// every living hit and every death's start goes as a line to
/// `<user data folder>\srp_ragdoll.csv`.
#[unsafe(no_mangle)]
pub static mut RAGDOLL_TRACE: u32 = TRACE_OFF;
/// (not 0: a zero static would go in no block the arena has)
const TRACE_OFF: u32 = 0x4F46_4600;
/// The imports: CreateFileA, WriteFile.
const CREATE_FILE: usize = 0x870048;
const WRITE_FILE: usize = 0x870108;
type CreateFile = unsafe extern "stdcall" fn(*const u8, u32, u32, *mut c_void, u32, u32, *mut c_void) -> isize;
type WriteFile = unsafe extern "stdcall" fn(isize, *const u8, u32, *mut u32, *mut c_void) -> i32;

/// The trace's file: not opened yet, open, or failed (no more tries).
#[derive(Clone, Copy)]
enum TraceFile {
    Closed,
    Open(isize),
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
    trace: TraceFile,
    shots: Shots,
    /// Set by the step when some place has a ragdoll a bullet could meet:
    /// the bullets' trace returns at once otherwise.
    any_bodies: AtomicBool,
    /// Guards `shots` between the bullets' threads.
    shots_lock: AtomicBool,
}

/// The game has one world at a time; its logic thread runs the step, and
/// the draws (one a place, maybe at once on several threads, after the
/// step) read `draws` and use their own slot alone.
static mut STATE: State = State {
    corpses: Corpses::new(),
    draws: [Draw::Off; MAX_PLACES],
    eye: EyeView::Off,
    mappings: [Mapping::Empty; 4],
    clock: Clock::Unstarted,
    time: 0.0,
    recent: [Recent::new(); MAX_PLACES],
    moving: [Moving { at: Vec3::ZERO, velocity: Vec3::ZERO, known: false }; MAX_PLACES],
    trace: TraceFile::Closed,
    shots: Shots {
        log: ShotLog::new(),
        pushes: [None; PUSHES],
        living: [None; LIVING_HITS],
    },
    any_bodies: AtomicBool::new(false),
    shots_lock: AtomicBool::new(false),
};

/// A Windows import (stdcall: it pops its argument; `extern "system"` is cdecl
/// on the i686 Linux target the crate is built for).
type QueryPerformance = unsafe extern "stdcall" fn(*mut i64) -> i32;

/// The real time since the last frame, s (1/60 the first time).
unsafe fn frame_time(clock: &mut Clock) -> f32 {
    let mut now = 0i64;
    unsafe {
        let counter: QueryPerformance = core::mem::transmute(read_volatile(QUERY_COUNTER as *const usize));
        counter(&mut now);
    }
    let ticks = now as u32;
    match *clock {
        Clock::Running { ticks: last, frequency } => {
            *clock = Clock::Running { ticks, frequency };
            ticks.wrapping_sub(last) as f32 / frequency as f32
        }
        Clock::Unstarted => {
            let mut frequency = 0i64;
            unsafe {
                let query: QueryPerformance = core::mem::transmute(read_volatile(QUERY_FREQUENCY as *const usize));
                query(&mut frequency);
            }
            if frequency > 0 && frequency <= u32::MAX as i64 {
                *clock = Clock::Running { ticks, frequency: frequency as u32 };
            }
            1.0 / 60.0
        }
    }
}

/// The level as the ragdolls read it: ray_test on the simulation's physics
/// world, the level's group alone, a budget of rays a frame.
struct GameLevel {
    world: Option<Addr>,
    rays: u32,
}

impl Level for GameLevel {
    fn sweep(&mut self, from: Vec3, to: Vec3, radius: f32) -> Sweep {
        let Some(world) = self.world else { return Sweep::Deferred };
        if self.rays == 0 {
            return Sweep::Deferred;
        }
        self.rays -= 1;
        let along = to - from;
        let Some(direction) = along.direction() else { return Sweep::Clear };
        let origin = [from.x, from.y, from.z];
        let direction = [direction.x, direction.y, direction.z];
        let mut result = [0u32; 10];
        unsafe {
            engine::engine_ray_test(
                world.0 as *mut c_void,
                result.as_mut_ptr() as *mut c_void,
                origin.as_ptr(),
                direction.as_ptr(),
                along.length() + radius,
                LEVEL_GROUP,
                LEVEL_MASK,
                null(),
                0,
            );
        }
        if result[HIT_OBJECT / 4] == 0 {
            return Sweep::Clear;
        }
        let f = |at: usize| f32::from_bits(result[at / 4]);
        Sweep::Hit {
            point: Vec3::new(f(HIT_POINT), f(HIT_POINT + 4), f(HIT_POINT + 8)),
            normal: Vec3::new(f(HIT_NORMAL), f(HIT_NORMAL + 4), f(HIT_NORMAL + 8)),
        }
    }
}

type MatchOptions = unsafe extern "thiscall" fn(*mut c_void) -> *mut c_void;
type ActivePlayer = unsafe extern "thiscall" fn(*mut c_void, *mut u32, u32) -> *mut u32;

/// The place's current player, its reference given back at once (the place
/// holds its own).
unsafe fn place_player(client: Addr, place: u32) -> Option<Addr> {
    unsafe {
        let vtable = Addr(client.u32(0));
        let active: ActivePlayer = core::mem::transmute(vtable.u32(ACTIVE_PLAYER_SLOT) as usize);
        let mut out = 0u32;
        active(client.0 as *mut c_void, &mut out, place);
        let player = Addr(out);
        if out == 0 {
            return None;
        }
        let references = AtomicU32::from_ptr(player.at(REFERENCES) as *mut u32);
        if references.fetch_sub(1, Ordering::AcqRel) == 1 {
            engine::engine_release_object(player.0 as *mut c_void, player.at(REFERENCES) as *mut u32);
            return None;
        }
        Some(player)
    }
}

/// The bones of ours of a skeleton, from the cache or read now.
unsafe fn mapping(mappings: &mut [Mapping; 4], skeleton: Addr) -> Option<BoneIndex> {
    for m in mappings.iter() {
        match *m {
            Mapping::Ours { skeleton: s, index } if s == skeleton => return Some(index),
            Mapping::Other { skeleton: s } if s == skeleton => return None,
            _ => {}
        }
    }
    let read = unsafe { read_skeleton(skeleton) };
    mappings.rotate_right(1);
    mappings[0] = match read {
        Some(index) => Mapping::Ours { skeleton, index },
        None => Mapping::Other { skeleton },
    };
    read
}

/// What the player wears and carries, kg on each part: each inventory
/// slot's weight (a piece's times a stack's count) on the parts it sits on
/// (the helmet and the mask on the head, the torso's armour on the chest,
/// the arms and the pelvis, the back's on the chest, the legs' on the pelvis
/// and the legs, the gloves on the forearms, the boots on the shins, the
/// weapons and the rest half on the chest, half on the pelvis).
unsafe fn load_of(player: Addr) -> [f32; PARTS] {
    use Part::*;
    let mut load = [0.0f32; PARTS];
    let Some(inventory) = (unsafe { player.pointer(INVENTORY) }) else { return load };
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
    for slot in 0..SLOTS {
        let Some(item) = (unsafe { inventory.pointer(INVENTORY_SLOTS + 4 * slot) }) else { continue };
        let piece = f32::from_bits(unsafe { item.u32(ITEM_WEIGHT) });
        let stack = unsafe { item.pointer(ITEM_ENTRY) }.is_some_and(|e| unsafe { e.u8(ENTRY_STACK) } != 0);
        let count = if stack { (unsafe { item.u32(ITEM_COUNT) } & 0xFFFF) as f32 } else { 1.0 };
        let weight = piece * count;
        if !weight.is_finite() || !(0.0..=50.0).contains(&weight) {
            continue;
        }
        for &(part, share) in shares[slot.min(7)] {
            load[part.index()] += weight * share;
        }
    }
    load
}

/// What the world shows of a player: alive, or dead with its death (and
/// the skeleton's bones of ours). A player not inserted, or with another
/// skeleton, is `Alive` here: nothing of ours is drawn.
unsafe fn seen(
    player: Addr,
    mappings: &mut [Mapping; 4],
    recent: &Recent,
    moving: &Moving,
    now: f32,
) -> (Seen, Option<BoneIndex>) {
    unsafe {
        if player.u8(INSERTED) == 0 || player.u8(ALIVE) != 0 {
            return (Seen::Alive, None);
        }
        let Some(model) = player.pointer(MODEL) else { return (Seen::Alive, None) };
        let Some(skeleton) = model.pointer(SKELETON) else { return (Seen::Alive, None) };
        let Some(index) = mapping(mappings, skeleton) else { return (Seen::Alive, None) };
        let transform = player.matrix(TRANSFORM);
        let stood_on = pose_of(&transform).map(|p| (p.position, UP));
        let gun = (player.u32(HELD_ITEM) != 0).then_some(GunShape::RIFLE);
        let mut death = Death::new(DeathKey(player.u32(SPAWNED)), Cause::Unknown, gun, stood_on);
        (death.hits, death.hit_count) = recent.of_death(now);
        death.load = load_of(player);
        death.velocity = if moving.known { moving.velocity } else { Vec3::ZERO };
        (Seen::Dead(death), Some(index))
    }
}

/// The step: once a frame on the logic thread, after the simulation's move
/// (game_world::tick 0x1F7C3C).
///
/// # Safety
/// `world` is the game world, on its logic thread, outside the draws.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ragdoll_step(world: *mut u8) {
    unsafe {
        let state = &mut *(&raw mut STATE);
        let dt = frame_time(&mut state.clock);
        state.time += dt;
        let living = core::mem::replace(&mut state.shots.living, [None; LIVING_HITS]);
        // the pushes of the shots since the last step (the bullets' threads
        // are done: the simulation's move has returned)
        for push in state.shots.pushes.iter_mut() {
            if let Some(p) = push.take() {
                state.corpses.push(p.place, p.death, p.part, p.point, p.impulse);
            }
        }
        let world = Addr(world as u32);
        let client = world.pointer(GAME).and_then(|game| game.pointer(CLIENT));
        let mut seen_places = [Seen::Alive; MAX_PLACES];
        let mut players: [Option<(Addr, Option<BoneIndex>)>; MAX_PLACES] = [None; MAX_PLACES];
        let mut count = 0;
        let mut local = None;
        let mut physics = None;
        if let Some(client) = client {
            let vtable = Addr(client.u32(0));
            let options: MatchOptions = core::mem::transmute(vtable.u32(MATCH_OPTIONS_SLOT) as usize);
            let options = options(client.0 as *mut c_void);
            if !options.is_null() {
                count = (Addr(options as u32).u8(PLACES) as usize).min(MAX_PLACES);
            }
            let local_place = client.pointer(LOCAL_PLAYER).map(|p| p.u8(PLACE) as usize);
            for place in 0..count {
                let Some(player) = place_player(client, place as u32) else { continue };
                physics = physics.or(player.pointer(PHYSICS_WORLD));
                // this frame's bullets in this player (its hit model's body)
                let body = player.pointer(HIT_MODEL).and_then(|m| m.pointer(HIT_MODEL_BODY));
                for (object, hit, mass, speed) in living.iter().flatten() {
                    if body == Some(Addr(*object)) {
                        state.recent[place].record(state.time, *hit);
                        trace_line(&mut state.trace, |w| {
                            w.text("hit,");
                            w.fields(&[(state.time, 3), (place as f32, 0), (*mass, 4), (*speed, 1), (hit.impulse.length(), 2)]);
                        });
                    }
                }
                // its motion while alive (its transform's place)
                if player.u8(INSERTED) != 0 && player.u8(ALIVE) != 0 {
                    let at = player.matrix(TRANSFORM);
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
                let (s, index) = seen(
                    player,
                    &mut state.mappings,
                    &state.recent[place],
                    &state.moving[place],
                    state.time,
                );
                seen_places[place] = s;
                players[place] = Some((player, index));
                if local_place == Some(place) {
                    local = Some((place, player));
                }
            }
        }
        let frame = Frame { dt, up: UP, places: &seen_places[..count], local: local.map(|(p, _)| p) };
        let mut level = GameLevel { world: physics, rays: RAYS_A_FRAME };
        state.corpses.step(&frame, &mut level);
        for (place, r) in state.corpses.started() {
            trace_line(&mut state.trace, |w| {
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
        }
        for place in 0..MAX_PLACES {
            state.draws[place] = match (state.corpses.place(place), players[place]) {
                (Corpse::Body { death, slot }, Some((player, Some(index)))) => {
                    Draw::On { player, key: death, slot, index }
                }
                _ => Draw::Off,
            };
        }
        state.eye = match (state.corpses.eye(), local) {
            (Some(eye), Some((_, player))) => EyeView::On { player, matrix: matrix_of(eye) },
            _ => EyeView::Off,
        };
        state.any_bodies.store(state.corpses.any_bodies(), Ordering::Release);
    }
}

/// A line of the opt-in trace (nothing unless the build turned it on): the
/// file opened at the first line (its column names first), the line
/// written at once.
fn trace_line(file: &mut TraceFile, line: impl FnOnce(&mut Line)) {
    if unsafe { read_volatile(&raw const RAGDOLL_TRACE) } != 1 {
        return;
    }
    unsafe {
        if let TraceFile::Closed = file {
            let mut path = Line::new();
            let folder = engine::engine_user_data_directory() as *const u8;
            let mut n = 0;
            while !folder.is_null() && n < 260 && read_volatile(folder.add(n)) != 0 {
                path.push(read_volatile(folder.add(n)));
                n += 1;
            }
            for &b in b"\\srp_ragdoll.csv\0" {
                path.push(b);
            }
            let create: CreateFile = core::mem::transmute(read_volatile(CREATE_FILE as *const usize));
            // FILE_APPEND_DATA, shared for reading and writing, OPEN_ALWAYS
            let handle = create(path.bytes.as_ptr(), 4, 3, core::ptr::null_mut(), 4, 0x80, core::ptr::null_mut());
            *file = if folder.is_null() || handle == -1 || handle == 0 {
                TraceFile::Failed
            } else {
                let mut head = Line::new();
                head.text(
                    "# srp ragdoll trace: hit,time s,place,mass kg,speed m/s,push N s | death,time s,place,cause \
                     (0 none 1 body 2 head 3 blast),hits,push N s,knock m/s,mass kg,speed m/s,legs,waist,neck,arms (the \
                     muscles' strength at the start; the death clip is the game's, captured)\r\n",
                );
                put(handle, &head);
                TraceFile::Open(handle)
            };
        }
        if let TraceFile::Open(handle) = *file {
            let mut text = Line::new();
            line(&mut text);
            text.text("\r\n");
            put(handle, &text);
        }
    }
}

unsafe fn put(handle: isize, line: &Line) {
    unsafe {
        let write: WriteFile = core::mem::transmute(read_volatile(WRITE_FILE as *const usize));
        let mut written = 0u32;
        write(handle, line.bytes.as_ptr(), line.used as u32, &mut written, core::ptr::null_mut());
    }
}

/// A line of text (an ASCII path or a trace line), at most 400 bytes.
struct Line {
    bytes: [u8; 400],
    used: usize,
}

impl Line {
    fn new() -> Line {
        Line { bytes: [0; 400], used: 0 }
    }

    fn push(&mut self, b: u8) {
        if self.used < self.bytes.len() {
            self.bytes[self.used] = b;
            self.used += 1;
        }
    }
}

impl Line {
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
        let v = if value < 0.0 { self.push(b'-'); -value } else { value };
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

/// A float3 of the game's.
unsafe fn vec3(at: *const [f32; 3]) -> Vec3 {
    let [x, y, z] = unsafe { read_volatile(at) };
    Vec3::new(x, y, z)
}

/// bullet::check_collision's ray (0x32D627, the ray traced, its result not
/// yet read): `bullet` the bullet, `from` + `direction` (unit) x `length`
/// the ray, `result` its result. The segment, cut at what the game's ray
/// hit, is tested against the ragdolls; a hit is pushed at the next step,
/// once a shot, and shows the game's body-hit effects. Reads the game's
/// memory only (and calls the bullet manager's own effect queues), so the
/// bullet goes on exactly as before.
///
/// # Safety
/// The game's bullet, its ray and result, inside the simulation's move.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ragdoll_bullet(
    bullet: *const u8,
    from: *const [f32; 3],
    direction: *const [f32; 3],
    length: f32,
    result: *const u8,
) {
    unsafe {
        let bullet = Addr(bullet as u32);
        let from = vec3(from);
        let direction = vec3(direction);
        let result = Addr(result as u32);
        let shooter = bullet.pointer(BULLET_SHOOTER);
        let key = |place: u8, death: u32| ShotKey {
            shooter: shooter.map_or(u8::MAX, |s| s.u8(SHOOTER_PLACE)),
            bullet: bullet.u32(BULLET_ID),
            segment: bullet.u32(BULLET_SEGMENT),
            place,
            death,
        };
        let push = |direction: Vec3| {
            let (mass, speed) = round(bullet);
            impulse(direction, mass, speed)
        };
        // a living player hit: kept for its death's push, once a shot
        if let Some(object) = result.pointer(RESULT_OBJECT)
            && object.u32(0) == ANIMATED_BODY_VTABLE
            && object.u32(BODY_MATERIAL) & 0xFFFF == ACTOR
            && let Some(unit) = direction.direction()
        {
            let hit = Hit { point: vec3(result.0 as *const [f32; 3]), impulse: push(unit) };
            let (mass, speed) = round(bullet);
            take_living(key(LIVING, object.0), object.0, hit, mass, speed);
        }
        // (the atomics alone are shared: the step's `&mut` is not alive now)
        if !(&*(&raw const STATE.any_bodies)).load(Ordering::Acquire) {
            return;
        }
        // the game's ray stops at what it hit: the living, a wall
        let reach = match result.u32(RESULT_OBJECT) {
            0 => length,
            _ => (vec3(result.0 as *const [f32; 3]) - from).length(),
        };
        let Some(shot) = Shot::new(from, direction, reach) else { return };
        // the step changes the corpses only after the move: a read here
        let corpses = &*(&raw const STATE.corpses);
        let Some(hit) = corpses.trace(&shot) else { return };
        if !take_shot(key(hit.place as u8, hit.death.0), &hit, push(shot.direction)) {
            return;
        }
        if shooter.is_some() {
            shot_effects(bullet, &shot, &hit);
        }
    }
}

/// A living hit's key's place (no corpse's: a place is below MAX_PLACES).
const LIVING: u8 = 0xFE;

/// The shots' lock taken for `f`.
fn with_shots<R>(f: impl FnOnce(&mut Shots) -> R) -> R {
    let lock = unsafe { &*(&raw const STATE.shots_lock) };
    while lock
        .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
    // the lock is held: the only user of the shots
    let out = f(unsafe { &mut *(&raw mut STATE.shots) });
    lock.store(false, Ordering::Release);
    out
}

/// A living player's hit kept for the step, if it is the shot's first trace.
fn take_living(key: ShotKey, object: u32, hit: Hit, mass: f32, speed: f32) {
    with_shots(|shots| {
        if let Some(free) = shots.living.iter_mut().find(|h| h.is_none())
            && shots.log.first(key)
        {
            *free = Some((object, hit, mass, speed));
        }
    })
}

/// The bullet's round: its ammo's mass (kg) and its speed now (m/s, its
/// segment's: the velocity +1Ch that the bullet's ctor 0x32BA8E and
/// change_trajectory 0x32EE55 write, as +34h).
unsafe fn round(bullet: Addr) -> (f32, f32) {
    unsafe {
        let mass = bullet.pointer(BULLET_AMMO).map_or(f32::NAN, |a| f32::from_bits(a.u32(AMMO_MASS)));
        let speed = vec3(bullet.at(BULLET_VELOCITY) as *const [f32; 3]).length();
        (mass, speed)
    }
}

/// The shot's push queued for the next step, if it is the shot's first
/// trace (a rollback's re-simulated tick traces it again) and a push is
/// free. True if taken.
fn take_shot(key: ShotKey, hit: &CorpseHit, j: Vec3) -> bool {
    with_shots(|shots| match shots.pushes.iter_mut().find(|p| p.is_none()) {
        Some(free) if shots.log.first(key) => {
            *free = Some(Push {
                place: hit.place,
                death: hit.death,
                part: hit.hit.part,
                point: hit.hit.point,
                impulse: j,
            });
            true
        }
        _ => false,
    })
}

/// The game's own effects of a bullet in a living body, at the corpse's
/// hit: the material pair bullet x actor's first particle and its sound, as
/// process_front_face_piercing (0x32E4F0) plays them for a pierced player.
/// The game's queues take them (the bullet manager runs them after the
/// tick). No wound decal: the game's own goes nowhere on a body (a
/// player's hittable gives add_decal no model, 0x1F1780).
unsafe fn shot_effects(bullet: Addr, shot: &Shot, hit: &CorpseHit) {
    unsafe {
        let Some(manager) = bullet.pointer(BULLET_MANAGER) else { return };
        let (Some(materials), Some(material)) = (manager.pointer(MANAGER_MATERIALS), bullet.pointer(BULLET_MATERIAL))
        else {
            return;
        };
        if manager.u32(MANAGER_RENDERER) == 0 {
            return;
        }
        let first = material.u32(MATERIAL_ID) & 0xFFFF;
        if first >= 128 {
            return;
        }
        let pair = engine::engine_material_pair(materials.0 as *const c_void, first, ACTOR);
        if pair.is_null() {
            return;
        }
        let pair = Addr(pair as u32);
        let point = [hit.hit.point.x, hit.hit.point.y, hit.hit.point.z];
        let direction = [shot.direction.x, shot.direction.y, shot.direction.z];
        let normal = [hit.hit.normal.x, hit.hit.normal.y, hit.hit.normal.z];
        let particles = pair.u32(PAIR_PARTICLES);
        if particles != 0 && particles != pair.u32(PAIR_PARTICLES_END) {
            engine::engine_play_particle(
                point.as_ptr(),
                manager.0 as *mut c_void,
                particles as *const c_void,
                direction.as_ptr(),
                normal.as_ptr(),
                pair.u32(PAIR_PARAMETER),
            );
        }
        if pair.u32(PAIR_SOUND) != 0 && bullet.u8(BULLET_QUIET) == 0 && bullet.u32(BULLET_SEGMENT) <= 1 {
            engine::engine_play_sound(
                manager.0 as *mut c_void,
                pair.at(PAIR_SOUND) as *const c_void,
                point.as_ptr(),
            );
        }
    }
}

/// The draw of this player, if a corpse of ours: the step's entry, the
/// player still dead in the same life.
unsafe fn draw_of(player: Addr) -> Option<(DeathKey, SlotId, BoneIndex)> {
    unsafe {
        let draws = &*(&raw const STATE.draws);
        draws.iter().find_map(|d| match *d {
            Draw::On { player: p, key, slot, index }
                if p == player
                    && player.u8(INSERTED) != 0
                    && player.u8(ALIVE) == 0
                    && player.u32(SPAWNED) == key.0 =>
            {
                Some((key, slot, index))
            }
            _ => None,
        })
    }
}

/// mt_draw's cull (0x24757C): 1 when a corpse of ours is this player (it is
/// drawn even when its model was not seen the last frame: its bones move).
///
/// # Safety
/// `player` is a player being drawn.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ragdoll_draws(player: *mut u8) -> u32 {
    unsafe { draw_of(Addr(player as u32)).is_some() as u32 }
}

/// mt_draw's bones (0x247716): `bones` the model-space matrices of the
/// skeleton (`count` of them, the game's order). The clip is captured; a
/// ragdoll's bones are written over it.
///
/// # Safety
/// `player` is the player being drawn, `bones` its array of `count`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ragdoll_apply(player: *mut u8, bones: *mut [f32; 16], count: u32) {
    unsafe {
        let player = Addr(player as u32);
        let Some((_, slot, index)) = draw_of(player) else { return };
        if index.iter().any(|&i| i as u32 >= count) {
            return;
        }
        let Some(transform) = pose_of(&player.matrix(TRANSFORM)) else { return };
        let mut skeleton: Skeleton = [Pose::IDENTITY; BONE_COUNT];
        for (b, &i) in index.iter().enumerate() {
            let Some(model) = pose_of(&read_volatile(bones.add(i as usize))) else { return };
            skeleton[b] = transform.then(model);
        }
        let slot = Corpses::slot_raw(&raw mut STATE.corpses, slot);
        if (*slot).draw(&mut skeleton) == Drawn::Ragdoll {
            let back = transform.inverse();
            for (b, &i) in index.iter().enumerate() {
                write_volatile(bones.add(i as usize), matrix_of(back.then(skeleton[b])));
            }
        }
    }
}

/// update_camera's eye (0x1F3643): `given` the transform the view is given
/// (&player+44Ch in the first person); the local player's ragdoll's eye
/// instead, while it has one.
///
/// # Safety
/// `player` is the camera's player.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ragdoll_eye(player: *mut u8, given: *const f32) -> *const f32 {
    unsafe {
        let player = Addr(player as u32);
        match &*(&raw const STATE.eye) {
            EyeView::On { player: p, matrix }
                if *p == player && given as usize == player.at(EYE) && draw_of(player).is_some() =>
            {
                matrix.as_ptr()
            }
            _ => given,
        }
    }
}

const _: () = assert!(BONE_COUNT < u8::MAX as usize);

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    // A loud crash at a known address, never a wrong world.
    loop {
        unsafe { core::arch::asm!("ud2", options(noreturn)) }
    }
}

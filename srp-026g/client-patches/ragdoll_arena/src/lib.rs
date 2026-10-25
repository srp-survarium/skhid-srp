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
//!   player's own ragdoll's eye, followed smoothly.

#![no_std]

use core::ffi::c_void;
use core::ptr::{null, read_volatile, write_volatile};
use core::sync::atomic::{AtomicU32, Ordering};

use ragdoll::corpse::{Corpse, Corpses, Death, DeathKey, Drawn, Frame, MAX_PLACES, Seen, SlotId};
use ragdoll::math::{Pose, Quat, Vec3, abs};
use ragdoll::ragdoll::{Cause, GunShape, Level, Skeleton, Sweep};
use ragdoll::skeleton::{BONE_COUNT, BONES};

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
/// ray_test's filter: the foot IK's group and the walkers' mask, the level
/// alone (no players, hit models, grenades, batteries or items).
const LEVEL_GROUP: u32 = 0x24;
const LEVEL_MASK: u32 = 0x02;
/// ray_test's result: the hit point, the normal (toward the ray's start
/// when the last argument is 0), the object (0: no hit).
const HIT_POINT: usize = 0x0;
const HIT_NORMAL: usize = 0x10;
const HIT_OBJECT: usize = 0x1C;
/// The level's rays a frame, for all ragdolls (a ray is a few microseconds).
const RAYS_A_FRAME: u32 = 160;
const UP: Vec3 = Vec3::Y;

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

struct State {
    corpses: Corpses,
    draws: [Draw; MAX_PLACES],
    eye: EyeView,
    mappings: [Mapping; 4],
    clock: Clock,
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

/// What the world shows of a player: alive, or dead with its death (and
/// the skeleton's bones of ours). A player not inserted, or with another
/// skeleton, is `Alive` here: nothing of ours is drawn.
unsafe fn seen(player: Addr, mappings: &mut [Mapping; 4]) -> (Seen, Option<BoneIndex>) {
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
        let death = Death { key: DeathKey(player.u32(SPAWNED)), cause: Cause::Unknown, gun, stood_on };
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
                let (s, index) = seen(player, &mut state.mappings);
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

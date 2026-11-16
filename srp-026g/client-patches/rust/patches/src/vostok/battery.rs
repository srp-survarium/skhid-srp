//! A carried battery's strikes (patch 143, docs/notes/battery-hit.md): the
//! states of the battery's state machine past its own three, and the
//! gun-stock bullet a strike hits with.
//!
//! The cook's callback (victory_item_core_cook::on_animations_loaded
//! 0x2FA400, the client's batteries and the core's alike) makes a battery's
//! states with initialize_logic (0x301920): take, carry and put, in that
//! order in the machine's list (item +168h); it has loaded every `states[i]`
//! of the config, the content's strikes' clips (3 to 8) too. At
//! `battery.logic` each [`Strike`]'s swing and recovery are appended: take
//! states (0x338940) over their authored clips, of the carry's kind (every
//! native "is carrying" test holds through them), their vtable the patch's
//! copy of the take's (finalize and may-be-put-away the patch's); and, for
//! each strike but the first, a carry state of its own (0x338BE0, the
//! carry's clips) before them: the carry a carrier stands in says which
//! strike the melee key starts. The list: take, carry, put; the ram's swing
//! and recovery; the shove's carry, swing and recovery; the slam's. The
//! transitions: a carry -> its swing (the patch's predicate, after carry ->
//! put), a swing -> put and its recovery, a recovery -> put and the next
//! strike's carry, the last's the first carry again (copies of carry ->
//! put's and take -> carry's), each tried once the state's clip ended. So a
//! carrier's strikes go round, and which comes next is the machine's current
//! state, which the game's own fsm runs, saves (its index) and loads, in
//! every game alike: no field of the patch's.

use core::ffi::c_void;

use super::context::Context;
use super::hook::{CallbackEntry, Handle, Thread};
use super::input::ActionMask;
use super::project::NativeVtable;
use super::world::read;
use crate::raw;

/// A battery state's kind (+20h, their constructors 0x338968, 0x338C09,
/// 0x338F28); the swing is of the carry's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Take = 1,
    Put = 2,
    Carry = 3,
}

impl Kind {
    fn parse(raw: u32) -> Option<Kind> {
        match raw {
            1 => Some(Kind::Take),
            2 => Some(Kind::Put),
            3 => Some(Kind::Carry),
            _ => None,
        }
    }
}

/// A posture's motion (+24h) of a jump (player_logic_stand_jump_state,
/// 0x336B87): in the air, where weapon_core::melee_pred 0x2E4100 refuses
/// the gun-stock hit.
const JUMP: u32 = 4;

/// The machine's states as initialize_logic leaves them.
const MADE_STATES: u32 = 3;

/// A kind of strike, in the order a carrier's strikes take them: the ram
/// first, after the slam the ram again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strike {
    /// Both arms push the battery out.
    Ram,
    /// The right arm shoves its right end forward about the left hand.
    Shove,
    /// Both arms lift it and throw it down.
    Slam,
}

impl Strike {
    pub const ALL: [Strike; 3] = [Strike::Ram, Strike::Shove, Strike::Slam];

    /// The config's state of its swing's clips; its recovery's is the next
    /// (battery-hit-1: 3 and 4 the ram's, 5 and 6 the shove's, 7 and 8 the
    /// slam's).
    const fn swing_clips(self) -> usize {
        3 + 2 * self as usize
    }

    /// Where its swing stands in the machine's list (the saved state's
    /// index): 3 the ram's, after take, carry and put; then each strike's
    /// carry, swing and recovery (5, 6, 7 the shove's; 8, 9, 10 the slam's).
    const fn swing_index(self) -> usize {
        match self {
            Strike::Ram => 3,
            Strike::Shove => 6,
            Strike::Slam => 9,
        }
    }

    /// The strike's grid on the plane square to the look [`STRIKE_FACE`]
    /// ahead, (right, up) m: 5 x 3 points over where its battery lands,
    /// nearest the look first, left before right: a fixed order, so every
    /// game picks the same ray. Where the battery is on that plane, posed as
    /// the game mixes it (scripts/battery-hit/preview.py):
    /// - the ram's, at the end of its push: 0.38 m left to 0.27 m right,
    ///   0.06 to 0.35 m below the look: 0.6 m wide around the look, from it
    ///   down to the battery's lower edge;
    /// - the shove's right end, which lands: 0.39 to 0.49 m right of the
    ///   look, its left end the pivot at the carrier's hip: the ram's grid
    ///   one column to the right;
    /// - the slam's: 0.20 to 0.50 m below the look at its strike, to 0.60 m
    ///   after: the ram's columns, the rows from the look down to 0.5 m.
    const fn grid(self) -> [(f32, f32); 15] {
        match self {
            Strike::Ram => [
                (0.0, 0.0),
                (-0.15, 0.0), (0.15, 0.0),
                (0.0, -0.175),
                (-0.15, -0.175), (0.15, -0.175),
                (-0.3, 0.0), (0.3, 0.0),
                (-0.3, -0.175), (0.3, -0.175),
                (0.0, -0.35),
                (-0.15, -0.35), (0.15, -0.35),
                (-0.3, -0.35), (0.3, -0.35),
            ],
            Strike::Shove => [
                (0.0, 0.0),
                (-0.15, 0.0), (0.15, 0.0),
                (0.0, -0.175),
                (-0.15, -0.175), (0.15, -0.175),
                (0.3, 0.0),
                (0.3, -0.175),
                (0.0, -0.35),
                (-0.15, -0.35), (0.15, -0.35),
                (0.45, 0.0),
                (0.3, -0.35),
                (0.45, -0.175),
                (0.45, -0.35),
            ],
            Strike::Slam => [
                (0.0, 0.0),
                (-0.15, 0.0), (0.15, 0.0),
                (0.0, -0.25),
                (-0.15, -0.25), (0.15, -0.25),
                (-0.3, 0.0), (0.3, 0.0),
                (-0.3, -0.25), (0.3, -0.25),
                (0.0, -0.5),
                (-0.15, -0.5), (0.15, -0.5),
                (-0.3, -0.5), (0.3, -0.5),
            ],
        }
    }

    /// Its rays from the eye whose rows are `right`, `up` and `forward`
    /// (unit): through each point of its grid, in its order, the first the
    /// look itself. Each is cast the bullet's own length (the gun's melee
    /// distance, 1.7 m: 1.0 m past the plane on the look), so the bullet
    /// reaches what its ray found.
    pub fn rays(self, right: [f32; 3], up: [f32; 3], forward: [f32; 3]) -> impl Iterator<Item = [f32; 3]> {
        self.grid().into_iter().map(move |(x, y)| {
            if (x, y) == (0.0, 0.0) {
                return forward;
            }
            let ray = [0, 1, 2].map(|i| forward[i] * STRIKE_FACE + right[i] * x + up[i] * y);
            let length = core::f32::math::sqrt(ray[0] * ray[0] + ray[1] * ray[1] + ray[2] * ray[2]);
            ray.map(|v| v / length)
        })
    }
}

/// How long a strike's two states last, s: each plays its clip in that time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Times {
    /// The swing: the gather and the drive, its end the strike.
    pub swing: f32,
    /// The recovery: the follow-through and the way back to the carry.
    pub recover: f32,
}

/// The inventory slots of the kit's guns, the first first (7 and 10).
const GUN_SLOTS: [usize; 2] = [7, 10];

/// Why a battery has no strikes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoSwing {
    /// Its machine is not the three states initialize_logic makes (one
    /// added already, or another layout): left as it is.
    NotAsMade,
    /// The cook loaded no states 3 to 8 (the content's battery-hit-1: a
    /// game without it would play otherwise than the others).
    NoSwingClips,
    /// The game's allocator gave nothing.
    OutOfMemory,
}

/// The first clip of a state `i`'s six among the cook's results: the
/// config first, then 2 + 6i (on_animations_loaded reads the take's from
/// result 2, 0x2FA52D, the carry's from 8, the put's from 14).
const fn first_result(state: usize) -> usize {
    2 + 6 * state
}

/// The carry's state in the config: its clips are every strike's carry's.
const CARRY: usize = 1;

/// A battery at `battery.logic`: its three states made, the cook's results
/// in hand.
pub struct MadeBattery<'t> {
    battery: raw::VictoryItemCore,
    queries: raw::QueriesResult,
    _thread: &'t Thread,
}

impl<'t> MadeBattery<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<MadeBattery<'t>> {
        use raw::site::battery_logic as site;
        // SAFETY: the site's ESI, the battery initialize_logic (0x2FA7E4)
        // has just made the states of, and its EBP frame, whose +8 is the
        // queries_result the callback was handed (alive through it).
        unsafe {
            let battery = site::battery(ctx)?;
            let queries = site::args(ctx)?.queries()?;
            Some(MadeBattery { battery, queries, _thread: ctx.thread() })
        }
    }

    /// The six clips of config state `state`, as the cook loaded them.
    ///
    /// # Safety
    /// The results are alive (the callback's).
    unsafe fn clips(&self, state: usize, room: &mut raw::BatteryClipsStorage) -> Result<raw::BatteryClips, NoSwing> {
        unsafe {
            if (read(self.queries.count()) as usize) < first_result(state) + 6 {
                return Err(NoSwing::NoSwingClips);
            }
            let clips = room.handle();
            for i in 0..6 {
                let clip = read(self.queries.loaded().at(first_result(state) + i).resource());
                if clip == 0 {
                    return Err(NoSwing::NoSwingClips);
                }
                *clips.clips().at(i).resource() = clip;
            }
            Ok(clips)
        }
    }

    /// Appends every [`Strike`]'s states (the module's account): its swing
    /// and its recovery, take states of the carry's kind over its clips,
    /// lasting `times` of it, `methods` their vtable; a carry of its own
    /// for each strike but the first, whose carry is the machine's; `wants`
    /// a carry -> its swing's predicate.
    pub fn add_strikes(&self, times: impl Fn(Strike) -> Times, methods: NativeVtable<raw::TakeStateMethods>,
                       wants: CallbackEntry<super::OrigFsmAlways>) -> Result<(), NoSwing> {
        let battery = self.battery;
        // SAFETY: the battery and its states, alive (its cook's callback, on
        // the thread that loads it); each read as initialize_logic wrote it
        // and checked, every clip found and every state made, before
        // anything of the machine is written; the clips the cook's.
        unsafe {
            let (take, carry, put) = made_states(battery).ok_or(NoSwing::NotAsMade)?;
            let take_carry = take.transitions_head().filter(|t| t.destination() == Some(carry))
                .ok_or(NoSwing::NotAsMade)?;
            let carry_put = carry.transitions_head().filter(|t| t.destination() == Some(put))
                .ok_or(NoSwing::NotAsMade)?;
            let mut room = raw::BatteryClipsStorage::zeroed();
            for strike in Strike::ALL {
                self.clips(strike.swing_clips(), &mut room)?;
                self.clips(strike.swing_clips() + 1, &mut room)?;
            }
            // A strike's carry, swing and recovery.
            let mut made = |strike: Strike| -> Result<[raw::BatteryState; 3], NoSwing> {
                let from = match strike {
                    Strike::Ram => carry,
                    Strike::Shove | Strike::Slam => carry_again(battery, self.clips(CARRY, &mut room)?)?,
                };
                let swing = carry_like(battery, self.clips(strike.swing_clips(), &mut room)?, times(strike).swing, methods)?;
                let recover = carry_like(battery, self.clips(strike.swing_clips() + 1, &mut room)?, times(strike).recover,
                                         methods)?;
                Ok([from, swing, recover])
            };
            let states = [made(Strike::Ram)?, made(Strike::Shove)?, made(Strike::Slam)?];
            // Appended as initialize_logic appends (0x301A28..0x301A4A).
            let mut tail = put;
            for state in states.into_iter().flatten().filter(|state| *state != carry) {
                tail.set_next(Some(state));
                tail = state;
                *battery.machine_states() += 1;
            }
            tail.set_next(None);
            battery.set_machine_tail(Some(tail));
            let mut predicate = raw::FsmPredicateStorage::zeroed();
            let wanted = predicate.handle();
            // A function-pointer functor (initialize_logic 0x301A7A: its
            // vtable tagged 1, copied as bytes), the entry its function, the
            // battery the buffer's second word.
            *wanted.manager() = raw::image::FSM_FUNCTION_PREDICATE | 1;
            *wanted.buffer().function() = wants.entry() as usize as u32;
            wanted.buffer().set_battery(Some(battery));
            for (i, [from, swing, recover]) in states.into_iter().enumerate() {
                if from != carry {
                    raw::engine_fsm_append_transition(Some(from), Some(put), Some(carry_put.predicate()));
                }
                raw::engine_fsm_append_transition(Some(from), Some(swing), Some(wanted));
                raw::engine_fsm_append_transition(Some(swing), Some(put), Some(carry_put.predicate()));
                raw::engine_fsm_append_transition(Some(swing), Some(recover), Some(take_carry.predicate()));
                raw::engine_fsm_append_transition(Some(recover), Some(put), Some(carry_put.predicate()));
                let [next, ..] = states[(i + 1) % states.len()];
                raw::engine_fsm_append_transition(Some(recover), Some(next), Some(take_carry.predicate()));
            }
        }
        Ok(())
    }
}

/// A carry state (initialize_logic's allocation and constructor, 0x301969)
/// over the carry's `clips`: a strike's own carry.
///
/// # Safety
/// `battery` and `clips` are alive, on the loading thread.
unsafe fn carry_again(battery: raw::VictoryItemCore, clips: raw::BatteryClips) -> Result<raw::BatteryState, NoSwing> {
    unsafe {
        let allocator = raw::image::game_allocator().allocator()
            .map_or(core::ptr::null_mut(), |a| a.address() as usize as *mut c_void);
        if allocator.is_null() {
            return Err(NoSwing::OutOfMemory);
        }
        let name = raw::engine_allocation_name(raw::image::CARRY_STATE_NAME as usize as *const c_void);
        let room = raw::engine_malloc(allocator, raw::BatteryCarryState::SIZE as u32, name);
        if room.is_null() {
            return Err(NoSwing::OutOfMemory);
        }
        raw::engine_battery_carry_state(Some(battery), room, Some(clips)).ok_or(NoSwing::OutOfMemory)
    }
}

/// A take state (initialize_logic's allocation and constructor, 0x30192D)
/// over `clips` played in `time` s, of the carry's kind, `methods` its vtable.
///
/// # Safety
/// `battery` and `clips` are alive, on the loading thread.
unsafe fn carry_like(battery: raw::VictoryItemCore, clips: raw::BatteryClips, time: f32,
                     methods: NativeVtable<raw::TakeStateMethods>) -> Result<raw::BatteryState, NoSwing> {
    unsafe {
        let allocator = raw::image::game_allocator().allocator()
            .map_or(core::ptr::null_mut(), |a| a.address() as usize as *mut c_void);
        if allocator.is_null() {
            return Err(NoSwing::OutOfMemory);
        }
        let name = raw::engine_allocation_name(raw::image::TAKE_STATE_NAME as usize as *const c_void);
        let room = raw::engine_malloc(allocator, raw::BatteryState::SIZE as u32, name);
        if room.is_null() {
            return Err(NoSwing::OutOfMemory);
        }
        let state = raw::engine_battery_take_state(room, Some(battery), Some(clips), time).ok_or(NoSwing::OutOfMemory)?;
        *state.kind() = Kind::Carry as u32;
        *state.vtable() = methods.address();
        Ok(state)
    }
}

/// The take, carry and put of a machine initialize_logic made, none for any
/// other.
///
/// # Safety
/// `battery` is alive, its machine made.
unsafe fn made_states(battery: raw::VictoryItemCore)
    -> Option<(raw::BatteryState, raw::BatteryState, raw::BatteryState)> {
    unsafe {
        if read(battery.machine_states()) != MADE_STATES {
            return None;
        }
        let take = battery.machine_head()?;
        let carry = take.next()?;
        let put = carry.next()?;
        let kinds = [take, carry, put].map(|s| Kind::parse(read(s.kind())));
        (kinds == [Some(Kind::Take), Some(Kind::Carry), Some(Kind::Put)] && put.next().is_none() && battery.machine_tail() == Some(put)
            && read(take.vtable()) == raw::image::take_state_methods().address())
            .then_some((take, carry, put))
    }
}

/// The swing as its finalize gets it (the patch's vtable slot): fsm::tick
/// leaving it (0x38CAA5), or the battery put down.
pub struct BatteryState<'t> {
    native: raw::BatteryState,
    thread: &'t Thread,
}

impl<'t> Handle<'t> for BatteryState<'t> {
    type Raw = raw::BatteryState;

    unsafe fn from_raw(thread: &'t Thread, native: Option<raw::BatteryState>) -> Option<Self> {
        Some(BatteryState { native: native?, thread })
    }
}

impl<'t> BatteryState<'t> {
    pub(in crate::vostok) fn raw(&self) -> raw::BatteryState {
        self.native
    }

    /// Its clip ended (m_animation_has_been_ended, +28h): the swing went
    /// through; false when it is left before (the carrier's death).
    pub fn ended(&self) -> bool {
        // SAFETY: the live state its finalize is called with, on its thread.
        unsafe { read(self.native.ready()) != 0 }
    }

    /// Its battery's carrier, alive.
    pub fn carrier(&self) -> Option<Carrier<'t>> {
        // SAFETY: as above; its battery the constructor's (+1Ch).
        unsafe { Carrier::of(self.thread, self.native.battery()?) }
    }

    /// The strike it is the swing of, by its place in its machine's list;
    /// none for a recovery, which shares the swings' vtable.
    pub fn swing(&self) -> Option<Strike> {
        // SAFETY: as above; the machine's list as add_strikes linked it.
        unsafe {
            let mut state = self.native.battery()?.machine_head();
            let mut index = 0;
            while let Some(at) = state {
                if at == self.native {
                    return Strike::ALL.into_iter().find(|strike| strike.swing_index() == index);
                }
                state = at.next();
                index += 1;
            }
            None
        }
    }
}

/// Carry -> swing's predicate's buffer, as its functor's invoker (0x303450)
/// enters the predicate with it: its second word is the battery.
pub struct PredicateBuffer<'t> {
    native: Option<raw::PredicateBuffer>,
    thread: &'t Thread,
}

impl<'t> PredicateBuffer<'t> {
    /// # Safety
    /// `native` is the buffer of a functor [`MadeBattery::add_strikes`] made
    /// (the fsm's copy), alive while the predicate runs, on `thread`.
    pub(in crate::vostok) unsafe fn from_raw(thread: &'t Thread, native: Option<raw::PredicateBuffer>) -> PredicateBuffer<'t> {
        PredicateBuffer { native, thread }
    }

    /// The carrier of the predicate's battery, alive.
    pub fn carrier(&self) -> Option<Carrier<'t>> {
        // SAFETY: the buffer of the functor add_strikes made, copied into the
        // carry's transition; its battery alive while its machine ticks.
        unsafe { Carrier::of(self.thread, self.native?.battery()?) }
    }
}

/// A battery's carrier (its +8, alive), and the battery.
pub struct Carrier<'t> {
    player: raw::Player,
    battery: raw::VictoryItemCore,
    _thread: &'t Thread,
}

impl<'t> Carrier<'t> {
    /// # Safety
    /// `battery` is alive, on `thread`.
    unsafe fn of(thread: &'t Thread, battery: raw::VictoryItemCore) -> Option<Carrier<'t>> {
        unsafe {
            let player = battery.user()?;
            (read(player.alive()) != 0).then_some(Carrier { player, battery, _thread: thread })
        }
    }

    /// The melee key in its input (bit 27, as weapon_core::melee_pred 0x2E4100 reads it).
    pub fn melee_down(&self) -> bool {
        // SAFETY: the live carrier's current input (set_input 0x2D876C).
        let actions = ActionMask::from_bits(unsafe { read(self.player.actions()) });
        actions.intersection(ActionMask::MELEE) == ActionMask::MELEE
    }

    /// In the air: its posture now (the battery's clip set's, +1ACh -> +20h)
    /// a jump's (+24h 4), as weapon_core::melee_pred 0x2E4100 reads a held
    /// gun's. The gun's other rule, not aimed (+5E8h), has no carrier's
    /// counterpart: a carrier does not aim.
    pub fn airborne(&self) -> bool {
        // SAFETY: the live battery's clip set, ticked with it.
        unsafe {
            self.battery.user_animations().and_then(|a| a.posture())
                .is_some_and(|posture| read(posture.motion()) == JUMP)
        }
    }

    /// The gun-stock hit's bullet (0x33AB50's emit_bullet) of its first gun
    /// (slot 7, else 10): from the eye's place, the gun's melee data and
    /// distance, now; along the first of `strike`'s rays ([`Strike::rays`],
    /// over where its battery lands) whose first hit is a living player's
    /// body, else along the look as the gun-stock hit. One bullet, so one
    /// victim a strike.
    ///
    /// The gun is deactivated while the battery is held (weapon_core's
    /// deactivate 0x2E2EE0: no user, its clip set no posture). Where the
    /// bullet reads them through the gun, all its life (its stance at the
    /// constructor, its melee damage each step), [`substitute_player`] and
    /// [`substitute_posture`] give it the gun's player and that player's
    /// posture instead; nothing of the gun is written.
    pub fn strike(&self, strike: Strike) -> Result<(), NoStrike> {
        // SAFETY: the live carrier's inventory and eye; a slot's item read
        // as a gun only after its active object's vtable says it is one; the
        // arguments as 0x33AB50 gives them, EDI the gun.
        unsafe {
            let inventory = self.player.inventory().ok_or(NoStrike::NoInventory)?;
            let gun = GUN_SLOTS.iter().find_map(|&slot| {
                let gun = inventory.slots().at(slot).item()?.active_object();
                [raw::image::WEAPON_VTABLE, raw::image::WEAPON_CORE_VTABLE].contains(&read(gun.vtable()))
                    .then(|| gun.weapon())
            }).ok_or(NoStrike::NoGun)?;
            let manager = gun.bullet_manager().ok_or(NoStrike::NoBulletManager)?;
            let physics = manager.physics().ok_or(NoStrike::NoPhysics)?;
            let eye = self.player.eye();
            let row = |r: usize| [0, 1, 2].map(|i| read(eye.add(4 * r + i)));
            let (right, up, forward, place) = (row(0), row(1), row(2), row(3));
            let distance = read(gun.melee_distance());
            let requester = RayRequester { _unread: [0; 2], shooter: self.player.bullet_owner() };
            let direction = strike.rays(right, up, forward)
                .find(|direction| first_hits_body(physics, &requester, place, *direction, distance))
                .unwrap_or(forward);
            let velocity = direction.map(|v| v * distance);
            raw::engine_emit_bullet(Some(gun), Some(manager), eye.add(12), velocity.as_ptr(),
                                    gun.melee_data().cast(), read(self.player.time()),
                                    Some(self.player.bullet_owner()), self.player.bullet_subject().cast(), 0);
        }
        Ok(())
    }
}

/// How far ahead of the eye the strikes' grids lie, m: the ram's battery's
/// front at the end of its push, 0.045 s after its strike (at the strike
/// itself, the push's fastest moment, it is 5 cm short of that and 7 cm
/// lower); the other strikes' batteries reach 0.6 m ahead.
const STRIKE_FACE: f32 = 0.7;

/// check_collision's ray (0x32D5EE..0x32D5F5): group 10h, mask 48h (the
/// hittable level and the hit models), the unflipped normal.
const BULLET_GROUP: u32 = 0x10;
const BULLET_MASK: u32 = 0x48;

/// What a ray test hands a hit model's delegate as check_collision's bullet
/// (needsCollision 0x75BD0 passes its requester to it, 0x75BFB):
/// recompute_damage_collision_bones 0x2DA5B0 reads only its +8, the
/// shooter, whose place (+4) it skips (0x2DA5E3), so the carrier's own body
/// is not met.
#[repr(C)]
struct RayRequester {
    _unread: [u32; 2],
    shooter: raw::BulletOwner,
}

/// Whether the ray's first hit is a living player's body: the game's own
/// ray test as the bullet's check_collision makes it, so a wall in front
/// stops it.
///
/// # Safety
/// `physics` is alive, on the simulation's thread.
unsafe fn first_hits_body(physics: raw::BulletPhysicsWorld, requester: &RayRequester, origin: [f32; 3],
                          direction: [f32; 3], length: f32) -> bool {
    let mut room = raw::RayResultStorage::zeroed();
    let result = room.handle();
    // SAFETY: our result, float3s and requester, alive for the call.
    unsafe {
        raw::engine_ray_test(Some(physics), Some(result), origin.as_ptr(), direction.as_ptr(), length, BULLET_GROUP,
                             BULLET_MASK, (requester as *const RayRequester).cast(), 1);
        result.object().is_some_and(|object| super::ray_test::living_body(object))
    }
}

/// Why a carrier could not strike. None happens in a match: a living
/// player has an inventory, a kit without guns in slots 7 and 10 crashes
/// every game at the spawn, and every roster gun has its bullet manager
/// (base_player::set_bullet_manager 0x2D7570), every manager the match's
/// physics world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoStrike {
    NoInventory,
    NoGun,
    NoBulletManager,
    NoPhysics,
}

/// The player a gun belongs to: the holder of its inventory (its item's
/// +250h, the inventory's +174h, the player's +108h interface, 0x1F17A0 for
/// both player classes), which every game builds with the kit, a late
/// joiner's and a replay's too.
///
/// # Safety
/// `gun` is alive, on this thread.
unsafe fn player_of(gun: raw::Weapon) -> Option<raw::Player> {
    unsafe { Some(gun.item().inventory()?.holder()?.player()) }
}

/// The posture of `player`'s held battery (its clip set's, +1ACh -> +20h),
/// none when it holds no battery.
///
/// # Safety
/// `player` is alive, on this thread.
unsafe fn battery_posture(player: raw::Player) -> Option<raw::PostureState> {
    unsafe {
        let held = player.held_item()?;
        [raw::image::VICTORY_ITEM_VTABLE, raw::image::VICTORY_ITEM_CORE_VTABLE].contains(&read(held.vtable()))
            .then(|| held.battery())?
            .user_animations()?.posture()
    }
}

/// The kill feed's weapon icon of a strike's kill: the hud movie's
/// CombatLogBatteryIcon (CombatLogItem.BATTERY_ICON), past every weapon's
/// `combat_log_icon` (0 to 99).
pub const KILL_FEED_ICON: u32 = 200;

/// `kill_feed.weapon_icon`: generate_killed_event about to write the kill
/// event's weapon icon (EDX). A kill by a strike's bullet shows the battery's
/// icon instead of the gun stock's: its data the gun's melee data (the
/// function's own melee test, 0x2DA2BC..0x2DA2D4) and the gun's player
/// holding a battery. The hud's handler alone reads the icon: what a match
/// computes is the same.
pub fn kill_feed_icon(ctx: &mut Context<'_>) {
    // SAFETY: the site's EBP, the killing bullet, alive while the function
    // runs, on its thread.
    let strike = unsafe { raw::site::kill_feed_weapon_icon::bullet(ctx).is_some_and(|bullet| struck_by_battery(bullet)) };
    if strike {
        ctx.set_reg(super::Reg::Edx, KILL_FEED_ICON);
    }
}

/// Whether `bullet` is a melee bullet whose gun's player holds a battery: a
/// strike's (a gun-stock hit needs the gun held). The gun is the bullet's
/// item's active object (+0Ch less 10h), as the constructor and
/// generate_killed_event take it; read as a gun only once the bullet's data
/// is that gun's melee data. The kill feed's icon and the ragdoll's push
/// (`BulletRay::melee`) both tell a strike by it.
///
/// # Safety
/// `bullet` is alive, on this thread.
pub(in crate::vostok) unsafe fn struck_by_battery(bullet: raw::Bullet) -> bool {
    unsafe {
        let (Some(item), Some(data)) = (bullet.weapon_item(), bullet.ammo()) else { return false };
        let gun = item.active_object().weapon();
        if data.address() != gun.melee_data() as usize as u32 {
            return false;
        }
        player_of(gun).and_then(|player| player.held_item()).is_some_and(|held| {
            [raw::image::VICTORY_ITEM_VTABLE, raw::image::VICTORY_ITEM_CORE_VTABLE].contains(&read(held.vtable()))
        })
    }
}

/// The sites where a bullet reads its gun's user (`gun.stance_user`,
/// `gun.melee_damage_user`, `gun.melee_falloff_user`), their entries
/// filtered to EAX 0: a deactivated gun, whose bullets are only the swing's
/// strike (no gun shoots natively while it is not held). EAX becomes the
/// gun's player, so the game reads that player's alive byte, kit and
/// profile, as it reads a held gun's user's. A gun with no player is the
/// patch's crash (named), never a skip.
pub fn substitute_player(ctx: &mut Context<'_>, site: UserSite) {
    // SAFETY: each site's ECX, the gun whose user (EAX) it just read, alive
    // while the game reads it, on its thread.
    let player = unsafe {
        let gun = match site {
            UserSite::Stance => raw::site::gun_stance_user::gun(ctx),
            UserSite::MeleeDamage => raw::site::gun_melee_damage_user::gun(ctx),
            UserSite::MeleeFalloff => raw::site::gun_melee_falloff_user::gun(ctx),
        };
        gun.and_then(|gun| player_of(gun))
    };
    let player = player.expect("patch 143: a deactivated gun read by a bullet has a player (its inventory's holder)");
    ctx.set_reg(super::Reg::Eax, player.address());
}

/// A site reading a gun's user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserSite {
    Stance,
    MeleeDamage,
    MeleeFalloff,
}

/// `gun.stance_posture`, filtered to EAX 0 (a deactivated gun's clip set has
/// no posture): EAX becomes the posture of the gun's player's held battery,
/// the carrier's posture now, so the strike's stance is the one a held gun
/// gives in it. None (no battery held) is the patch's crash (named).
pub fn substitute_posture(ctx: &mut Context<'_>) {
    // SAFETY: the site's ECX, the gun whose posture (EAX) it just read,
    // alive, on its thread.
    let posture = unsafe {
        raw::site::gun_stance_posture::gun(ctx).and_then(|gun| battery_posture(player_of(gun)?))
    };
    let posture = posture.expect("patch 143: a deactivated gun's player holds a battery with a posture");
    ctx.set_reg(super::Reg::Eax, posture.address());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_strikes_rays_start_on_the_look_and_cover_its_grid() {
        let (right, up, forward) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
        for strike in Strike::ALL {
            let rays: Vec<[f32; 3]> = strike.rays(right, up, forward).collect();
            assert_eq!(rays.len(), 15);
            assert_eq!(rays[0], forward, "the look first, as the gun-stock hit");
            for ray in &rays {
                assert!((ray[0] * ray[0] + ray[1] * ray[1] + ray[2] * ray[2] - 1.0).abs() < 1e-6);
            }
            // each crosses the plane at its grid point
            for (ray, (x, y)) in rays.iter().zip(strike.grid()) {
                assert!((ray[0] / ray[2] * STRIKE_FACE - x).abs() < 1e-6 && (ray[1] / ray[2] * STRIKE_FACE - y).abs() < 1e-6);
            }
            assert!(rays.iter().all(|ray| ray[1] <= 0.0), "from the look down");
        }
    }

    #[test]
    fn a_strikes_grid_is_five_by_three_nearest_the_look_first() {
        let columns = |strike| match strike {
            Strike::Ram | Strike::Slam => [-0.3, -0.15, 0.0, 0.15, 0.3],
            Strike::Shove => [-0.15, 0.0, 0.15, 0.3, 0.45],
        };
        let rows = |strike| match strike {
            Strike::Ram | Strike::Shove => [0.0, -0.175, -0.35],
            Strike::Slam => [0.0, -0.25, -0.5],
        };
        for strike in Strike::ALL {
            let grid = strike.grid();
            for x in columns(strike) {
                for y in rows(strike) {
                    assert_eq!(grid.iter().filter(|point| **point == (x, y)).count(), 1, "{strike:?} ({x}, {y})");
                }
            }
            let far = |(x, y): (f32, f32)| x * x + y * y;
            for pair in grid.windows(2) {
                // nearer first; as near, left before right (f32: a hair of tolerance)
                assert!(far(pair[0]) < far(pair[1]) + 1e-6, "{strike:?} {pair:?}");
                if (far(pair[0]) - far(pair[1])).abs() < 1e-6 {
                    assert!(pair[0].0 < pair[1].0, "{strike:?} {pair:?}");
                }
            }
        }
    }

    #[test]
    fn the_strikes_take_the_configs_states_and_the_machines_places_in_turn() {
        assert_eq!(Strike::ALL.map(Strike::swing_clips), [3, 5, 7]);
        assert_eq!(Strike::ALL.map(Strike::swing_index), [3, 6, 9]);
    }
}

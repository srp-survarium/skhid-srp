//! A carried battery's swing (patch 143, docs/notes/battery-hit.md): the
//! fourth state of the battery's state machine, and the gun-stock bullet it
//! strikes with.
//!
//! The cook's callback (victory_item_core_cook::on_animations_loaded
//! 0x2FA400, the client's batteries and the core's alike) makes a battery's
//! states with initialize_logic (0x301920): take, carry and put, in that
//! order in the machine's list (item +168h). At `battery.logic` the swing is
//! appended to them: a take state (0x338940) over the put's six clips, its
//! kind the carry's (every native "is carrying" test holds through it), its
//! vtable the patch's copy of the take's (its finalize the patch's), and three
//! transitions: carry -> swing (the patch's predicate, after carry -> put),
//! swing -> put (a copy of carry -> put's) and swing -> carry (a copy of
//! take -> carry's), tried once its clip ended. The game's own fsm then
//! runs, saves (its index, 3) and loads it, in every game alike.

use core::ffi::c_void;

use super::context::Context;
use super::hook::{CallbackEntry, Handle, Thread};
use super::input::ActionMask;
use super::project::NativeVtable;
use super::world::read;
use crate::raw;

/// A state's kind (+20h): take, carry and put (their constructors 0x338968,
/// 0x338C09, 0x338F28).
const TAKE: u32 = 1;
const CARRY: u32 = 3;
const PUT: u32 = 2;

/// The machine's states as initialize_logic leaves them.
const MADE_STATES: u32 = 3;

/// The inventory slots of the kit's guns, the first first (7 and 10).
const GUN_SLOTS: [usize; 2] = [7, 10];

/// Why a battery has no swing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoSwing {
    /// Its machine is not the three states initialize_logic makes (one
    /// added already, or another layout): left as it is.
    NotAsMade,
    /// The game's allocator gave nothing.
    OutOfMemory,
}

/// A battery at `battery.logic`: its three states made, the put's clips in
/// the callback's frame.
pub struct MadeBattery<'t> {
    battery: raw::VictoryItemCore,
    put_clips: raw::BatteryClips,
    _thread: &'t Thread,
}

impl<'t> MadeBattery<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<MadeBattery<'t>> {
        use raw::site::battery_logic as site;
        // SAFETY: the site's ESI, the battery initialize_logic (0x2FA7E4)
        // has just made the states of, and its ESP, the callback's frame
        // whose +28h is the put's six clips (released after the site).
        unsafe {
            let battery = site::battery(ctx)?;
            let frame = site::frame(ctx)?;
            Some(MadeBattery { battery, put_clips: frame.put_clips(), _thread: ctx.thread() })
        }
    }

    /// Appends the swing: the put's clips played as a take's in `time` s,
    /// `methods` its vtable, `wants` carry -> swing's predicate.
    pub fn add_swing(&self, time: f32, methods: NativeVtable<raw::TakeStateMethods>,
                     wants: CallbackEntry<super::OrigFsmAlways>) -> Result<(), NoSwing> {
        let battery = self.battery;
        // SAFETY: the battery and its states, alive (its cook's callback, on
        // the thread that loads it); each read as initialize_logic wrote it
        // and checked before anything is written.
        unsafe {
            let (take, carry, put) = made_states(battery).ok_or(NoSwing::NotAsMade)?;
            let take_carry = take.transitions_head().filter(|t| t.destination() == Some(carry))
                .ok_or(NoSwing::NotAsMade)?;
            let carry_put = carry.transitions_head().filter(|t| t.destination() == Some(put))
                .ok_or(NoSwing::NotAsMade)?;
            // initialize_logic's allocation of a take state: the game's
            // allocator, the take's type descriptor its name (0x30192D).
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
            let swing = raw::engine_battery_take_state(room, Some(battery), Some(self.put_clips), time)
                .ok_or(NoSwing::OutOfMemory)?;
            *swing.kind() = CARRY;
            *swing.vtable() = methods.address();
            // Appended as initialize_logic appends (0x301A28..0x301A4A).
            swing.set_next(None);
            put.set_next(Some(swing));
            battery.set_machine_tail(Some(swing));
            *battery.machine_states() += 1;
            let mut predicate = raw::FsmPredicateStorage::zeroed();
            let wanted = predicate.handle();
            // A function-pointer functor (initialize_logic 0x301A7A: its
            // vtable tagged 1, copied as bytes), the entry its function, the
            // battery the buffer's second word.
            *wanted.manager() = raw::image::FSM_FUNCTION_PREDICATE | 1;
            *wanted.buffer().function() = wants.entry() as usize as u32;
            wanted.buffer().set_battery(Some(battery));
            raw::engine_fsm_append_transition(Some(carry), Some(swing), Some(wanted));
            raw::engine_fsm_append_transition(Some(swing), Some(put), Some(carry_put.predicate()));
            raw::engine_fsm_append_transition(Some(swing), Some(carry), Some(take_carry.predicate()));
        }
        Ok(())
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
        let kinds = [take, carry, put].map(|s| read(s.kind()));
        (kinds == [TAKE, CARRY, PUT] && put.next().is_none() && battery.machine_tail() == Some(put)
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
}

/// Carry -> swing's predicate's buffer, as its functor's invoker (0x303450)
/// enters the predicate with it: its second word is the battery.
pub struct PredicateBuffer<'t> {
    native: Option<raw::PredicateBuffer>,
    thread: &'t Thread,
}

impl<'t> PredicateBuffer<'t> {
    /// # Safety
    /// `native` is the buffer of a functor [`MadeBattery::add_swing`] made
    /// (the fsm's copy), alive while the predicate runs, on `thread`.
    pub(in crate::vostok) unsafe fn from_raw(thread: &'t Thread, native: Option<raw::PredicateBuffer>) -> PredicateBuffer<'t> {
        PredicateBuffer { native, thread }
    }

    /// The carrier of the predicate's battery, alive.
    pub fn carrier(&self) -> Option<Carrier<'t>> {
        // SAFETY: the buffer of the functor add_swing made, copied into the
        // carry's transition; its battery alive while its machine ticks.
        unsafe { Carrier::of(self.thread, self.native?.battery()?) }
    }
}

/// A battery's carrier (its +8, alive).
pub struct Carrier<'t> {
    player: raw::Player,
    _thread: &'t Thread,
}

impl<'t> Carrier<'t> {
    /// # Safety
    /// `battery` is alive, on `thread`.
    unsafe fn of(thread: &'t Thread, battery: raw::VictoryItemCore) -> Option<Carrier<'t>> {
        unsafe {
            let player = battery.user()?;
            (read(player.alive()) != 0).then_some(Carrier { player, _thread: thread })
        }
    }

    /// The melee key in its input (bit 27, as weapon_core::melee_pred 0x2E4100 reads it).
    pub fn melee_down(&self) -> bool {
        // SAFETY: the live carrier's current input (set_input 0x2D876C).
        let actions = ActionMask::from_bits(unsafe { read(self.player.actions()) });
        actions.intersection(ActionMask::MELEE) == ActionMask::MELEE
    }

    /// The gun-stock hit's bullet (0x33AB50's emit_bullet) of its first gun
    /// (slot 7, else 10): from the eye's place along its forward row times
    /// the gun's melee distance, the gun's melee data, now.
    pub fn strike(&self) -> Result<(), NoGun> {
        // SAFETY: the live carrier's inventory and eye; a slot's item read
        // as a gun only after its active object's vtable says it is one; the
        // arguments as 0x33AB50 gives them, EDI the gun.
        unsafe {
            let inventory = self.player.inventory().ok_or(NoGun)?;
            let gun = GUN_SLOTS.iter().find_map(|&slot| {
                let gun = inventory.slots().at(slot).item()?.active_object();
                [raw::image::WEAPON_VTABLE, raw::image::WEAPON_CORE_VTABLE].contains(&read(gun.vtable()))
                    .then(|| gun.weapon())
            }).ok_or(NoGun)?;
            let manager = gun.bullet_manager().ok_or(NoGun)?;
            let eye = self.player.eye();
            let distance = read(gun.melee_distance());
            let velocity = [8, 9, 10].map(|i| read(eye.add(i)) * distance);
            raw::engine_emit_bullet(Some(gun), Some(manager), eye.add(12), velocity.as_ptr(),
                                    gun.melee_data().cast(), read(self.player.time()),
                                    Some(self.player.bullet_owner()), self.player.bullet_subject().cast(), 0);
        }
        Ok(())
    }
}

/// A carrier with no gun in slot 7 or 10 (never in a match: a kit without
/// both crashes every game at the spawn).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoGun;

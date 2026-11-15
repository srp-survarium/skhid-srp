//! A carried battery's swing (patch 143, docs/notes/battery-hit.md): the
//! fourth and fifth states of the battery's state machine, and the gun-stock
//! bullet it strikes with.
//!
//! The cook's callback (victory_item_core_cook::on_animations_loaded
//! 0x2FA400, the client's batteries and the core's alike) makes a battery's
//! states with initialize_logic (0x301920): take, carry and put, in that
//! order in the machine's list (item +168h); it has loaded every `states[i]`
//! of the config, the content's swing (3) and recovery (4) too. At
//! `battery.logic` both are appended: take states (0x338940) over their
//! authored clips, of the carry's kind (every native "is carrying" test holds
//! through them), their vtable the patch's copy of the take's (finalize and
//! may-be-put-away the patch's); transitions carry -> swing (the patch's
//! predicate, after carry -> put), swing -> put and swing -> recovery, then
//! recovery -> put and recovery -> carry (copies of carry -> put's and
//! take -> carry's), each tried once the state's clip ended. The game's own
//! fsm runs, saves (indices 3, 4) and loads them, in every game alike.

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

/// The inventory slots of the kit's guns, the first first (7 and 10).
const GUN_SLOTS: [usize; 2] = [7, 10];

/// Why a battery has no swing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoSwing {
    /// Its machine is not the three states initialize_logic makes (one
    /// added already, or another layout): left as it is.
    NotAsMade,
    /// The cook loaded no states 3 and 4 (the content's battery-hit-1:
    /// a game without it would play otherwise than the others).
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

/// The swing's and the recovery's states in the config (battery-hit-1).
const SWING: usize = 3;
const RECOVER: usize = 4;

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

    /// Appends the swing (config state 3: wind-up and strike, `swing_s`
    /// long, its end the impact) and the recovery (state 4: follow-through
    /// back to the carry, `recover_s`), each a take state of the carry's kind
    /// with `methods` its vtable; `wants` carry -> swing's predicate.
    pub fn add_swing(&self, swing_s: f32, recover_s: f32, methods: NativeVtable<raw::TakeStateMethods>,
                     wants: CallbackEntry<super::OrigFsmAlways>) -> Result<(), NoSwing> {
        let battery = self.battery;
        // SAFETY: the battery and its states, alive (its cook's callback, on
        // the thread that loads it); each read as initialize_logic wrote it
        // and checked before anything is written; the clips the cook's.
        unsafe {
            let (take, carry, put) = made_states(battery).ok_or(NoSwing::NotAsMade)?;
            let take_carry = take.transitions_head().filter(|t| t.destination() == Some(carry))
                .ok_or(NoSwing::NotAsMade)?;
            let carry_put = carry.transitions_head().filter(|t| t.destination() == Some(put))
                .ok_or(NoSwing::NotAsMade)?;
            let (mut swing_room, mut recover_room) = (raw::BatteryClipsStorage::zeroed(), raw::BatteryClipsStorage::zeroed());
            let swing_clips = self.clips(SWING, &mut swing_room)?;
            let recover_clips = self.clips(RECOVER, &mut recover_room)?;
            let swing = carry_like(battery, swing_clips, swing_s, methods)?;
            let recover = carry_like(battery, recover_clips, recover_s, methods)?;
            // Appended as initialize_logic appends (0x301A28..0x301A4A).
            put.set_next(Some(swing));
            swing.set_next(Some(recover));
            recover.set_next(None);
            battery.set_machine_tail(Some(recover));
            *battery.machine_states() += 2;
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
            raw::engine_fsm_append_transition(Some(swing), Some(recover), Some(take_carry.predicate()));
            raw::engine_fsm_append_transition(Some(recover), Some(put), Some(carry_put.predicate()));
            raw::engine_fsm_append_transition(Some(recover), Some(carry), Some(take_carry.predicate()));
        }
        Ok(())
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

    /// The swing (its machine's fourth state), not the recovery (the fifth),
    /// which shares its vtable.
    pub fn is_swing(&self) -> bool {
        // SAFETY: as above; the machine's list as add_swing linked it.
        unsafe {
            let fourth = self.native.battery().and_then(|b| b.machine_head())
                .and_then(|s| s.next()).and_then(|s| s.next()).and_then(|s| s.next());
            fourth == Some(self.native)
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
    /// (slot 7, else 10): from the eye's place along its forward row times
    /// the gun's melee distance, the gun's melee data, now.
    ///
    /// The gun is deactivated while the battery is held (weapon_core's
    /// deactivate 0x2E2EE0: no user, its clip set no posture). Where the
    /// bullet reads them through the gun, all its life (its stance at the
    /// constructor, its melee damage each step), [`substitute_player`] and
    /// [`substitute_posture`] give it the gun's player and that player's
    /// posture instead; nothing of the gun is written.
    pub fn strike(&self) -> Result<(), NoStrike> {
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

/// Why a carrier could not strike. None happens in a match: a living
/// player has an inventory, a kit without guns in slots 7 and 10 crashes
/// every game at the spawn, and every roster gun has its bullet manager
/// (base_player::set_bullet_manager 0x2D7570).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoStrike {
    NoInventory,
    NoGun,
    NoBulletManager,
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

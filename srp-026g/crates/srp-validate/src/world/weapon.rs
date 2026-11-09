//! A weapon item's state (`weapon_core::deserialize` 0x2E3490, writer
//! `weapon_core::serialize` 0x2E3130) and the portable object a weapon and a
//! carried battery end with (`portable_interactive_object_core::
//! deserialize` 0x2FF970, writer 0x2FF930), read into zero-copy views
//! ([`WeaponState`], [`Portable`]).
//!
//! The names are the 0.26g writers' own: each serialize passes the member's
//! C++ expression to its assert (e.g. `m_random.seed()` 0xA13530), checked
//! against the 0.21d PDB's members at the 0.21d reader's stores. PROVEN
//! unless marked. A reader turns every field with a limited set of values
//! into its type and refuses the world when it cannot.
//!
//! FIELD MAP (weapon+offset: the 0.26g member the reader stores)
//!
//! ```text
//! u32  m_random_generator          +0x5BC  minstd_rand state, 1..=2^31-2
//! u32  m_last_tick_time_in_ms      +0x5A4  0xFFFFFFFF: not ticked since spawn
//! bool m_load_ammo_on_next_activate +0x5F4
//! u8   m_ammo_in_magazine          +0x5E5  (<= m_magazine_capacity LIKELY, not known here)
//! u8   m_fire_queue_type           +0x5E6  (< the config's queue types, not known here)
//! u8   m_current_ammo_id           +0x5B1  < m_ammunition_slots_count (+0x5B0)
//! bool m_is_round_chambered        +0x5F1  a gun with a chamber state only
//! -- the rest only with interactive_object::m_user (+0x8) set ([`InHand`]):
//! u32  m_ammo_switch_time_in_ms    +0x5A8  a list of two or more only; 0xFFFFFFFF none
//! u8   m_next_ammo_id              +0x5B2  a switch only; < the slot count
//! recoil_calculator                +0x468  u32 m_random seed, u32 m_time_of_last_shot_in_ms
//!                                          (0xFFFFFFFF none); after a shot float3 m_recoil,
//!                                          m_recoil_speed and its pid controller's m_error,
//!                                          m_integral
//! dispersion_calculator            +0x3F8  transitions m_character_dispersion,
//!                                          m_weapon_dispersion (LIKELY names), f32
//!                                          m_skill_influence
//! bool m_aimed                     +0x5E8
//! transition m_aim_progress        +0x5FC  (LIKELY name, 0.21d +0x4D0)
//! breath_vibration_calculator      +0x288  aimed only: f32 m_phase_time, float2
//!                                          m_vibration, f32 m_amplitude,
//!                                          m_breath_holding_reserve, m_speed_factor,
//!                                          m_penalty_factor, m_not_holding_breath_delay;
//!                                          its m_logic state < 3
//! u8   m_sprint_animation_to_wait_for +0x5EC {0, 2, 4} or 6 none (every store read)
//! bool m_need_to_auto_reload       +0x5F5
//! u8   m_logic state id            +0x320  < the gun's states; that state's bytes
//! the portable object              +0x278  [`Portable`]
//! ```
//!
//! A transition (`transition_helper` 0x32FF70): f32 m_current_value,
//! m_start_value, m_target_value, m_transition_time, u32
//! m_start_transition_time_in_ms (0xFFFFFFFF settled).
//!
//! The weapon states (state_type, class: the bytes of the main stream; each
//! `weapon_sound_events_handler_state` writes its `m_sounds_counter` to the
//! second packet only, which an uploaded world has not):
//!
//! ```text
//! 302 310 313 320  idle, double-barrel idle, pistol idle, preview:
//!                  bool m_animation_has_been_ended (weapon_core_base_state +0x12D)
//! 300 301 303 304  show, hide, reload, chamber; 311 314 the double-barrel's and
//! 311 314          pistol's reload: + u8 m_index_of_animation_to_wait (+0x134, 0xFF none)
//! 305              fire: + u8 m_bullets_in_queue (+0x13C), bool m_can_reset_fire_queue
//! 312 315          double-barrel and pistol fire: + u8 m_weapon_animation_index (+0x16C)
//! 321              melee: + bool m_melee_animation_ended (+0x158)
//! 306              shotgun reload: the flag, its sub-state (start, one round, finish;
//!                  order LIKELY) and that sub-state's flag and index
//! 316              throw grenade: the flag, its sub-state (pull pin, idle, throw;
//!                  order LIKELY) with its flag and index, u8 the grenade set's
//!                  profile_slot_id() (< 23 LIKELY; 23 none: exe patch 59, below),
//!                  u8 m_grenade_index (+0x180, 0xFF none)
//! ```

use super::inventory::{
    Chamber, Fault, Gun, GunState, Shape, SlotId, ammo_id, flag, float, floats, state_id,
    u8, u32,
};
use crate::reader::Reader;
use crate::{Finite, Reason};

/// A u32 that is 0xFFFFFFFF for none.
pub(super) fn optional_u32(reader: &mut Reader) -> Result<Option<u32>, Fault> {
    Ok(match u32(reader)? {
        u32::MAX => None,
        value => Some(value),
    })
}

/// A u8 that is 0xFF for none.
pub(super) fn optional_u8(reader: &mut Reader) -> Result<Option<u8>, Fault> {
    Ok(match u8(reader)? {
        0xFF => None,
        value => Some(value),
    })
}

/// A field outside its domain.
fn outside(at: usize, field: &'static str, value: u32) -> Fault {
    (at, Reason::TailField { at, field, value })
}

/// A transition helper (`transition_helper::deserialize` 0x32FF70).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    current_value: Finite,
    start_value: Finite,
    target_value: Finite,
    transition_time: Finite,
    start_transition_time_ms: Option<u32>,
}

impl Transition {
    /// `m_current_value`.
    pub fn current_value(&self) -> Finite {
        self.current_value
    }

    /// `m_start_value`.
    pub fn start_value(&self) -> Finite {
        self.start_value
    }

    /// `m_target_value`.
    pub fn target_value(&self) -> Finite {
        self.target_value
    }

    /// `m_transition_time`.
    pub fn transition_time(&self) -> Finite {
        self.transition_time
    }

    /// `m_start_transition_time_in_ms`, `None` when settled.
    pub fn start_transition_time_ms(&self) -> Option<u32> {
        self.start_transition_time_ms
    }

    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        Ok(Self {
            current_value: float(reader)?,
            start_value: float(reader)?,
            target_value: float(reader)?,
            transition_time: float(reader)?,
            start_transition_time_ms: optional_u32(reader)?,
        })
    }
}

/// The largest state a minstd_rand generator has (2^31 - 2): its state is
/// `1..=MAX` (the constructor stores 1, the generator 0x2AB6E0 multiplies
/// by 48271 modulo 2^31 - 1).
const MINSTD_MAX: u32 = 0x7FFF_FFFE;

/// A weapon's state, borrowed from the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponState {
    random_generator: u32,
    last_tick_time_ms: Option<u32>,
    load_ammo_on_next_activate: bool,
    ammo_in_magazine: u8,
    fire_queue_type: u8,
    current_ammo_id: u8,
    round_chambered: Option<bool>,
    in_hand: Option<InHand>,
}

impl WeaponState {
    /// `m_random_generator`: the state of its minstd_rand, `1..=2^31-2`.
    pub fn random_generator(&self) -> u32 {
        self.random_generator
    }

    /// `m_last_tick_time_in_ms`, `None` when not ticked since its spawn.
    pub fn last_tick_time_ms(&self) -> Option<u32> {
        self.last_tick_time_ms
    }

    /// `m_load_ammo_on_next_activate`.
    pub fn load_ammo_on_next_activate(&self) -> bool {
        self.load_ammo_on_next_activate
    }

    /// `m_ammo_in_magazine`.
    pub fn ammo_in_magazine(&self) -> u8 {
        self.ammo_in_magazine
    }

    /// `m_fire_queue_type`.
    pub fn fire_queue_type(&self) -> u8 {
        self.fire_queue_type
    }

    /// `m_current_ammo_id`: an index of its ammo list (0 for a gun with
    /// none, LIKELY; any byte is taken then).
    pub fn current_ammo_id(&self) -> u8 {
        self.current_ammo_id
    }

    /// `m_is_round_chambered`, saved by a gun with a chamber state only.
    pub fn round_chambered(&self) -> Option<bool> {
        self.round_chambered
    }

    /// What a gun with a user saves (one put in a hand since its cook).
    pub fn in_hand(&self) -> Option<InHand> {
        self.in_hand
    }

    pub(super) fn read(reader: &mut Reader, gun: Gun, shape: Shape) -> Result<Self, Fault> {
        let at = reader.at();
        let random_generator = u32(reader)?;
        if !(1..=MINSTD_MAX).contains(&random_generator) {
            return Err(outside(at, "a weapon's m_random_generator", random_generator));
        }
        let last_tick_time_ms = optional_u32(reader)?;
        let load_ammo_on_next_activate = flag(reader)?;
        let ammo_in_magazine = u8(reader)?;
        let fire_queue_type = u8(reader)?;
        // A picked gun's list is the picker's: its ids bounded by the most a
        // list holds.
        let bound = super::inventory::ammo_bound(gun.ammo);
        let current_ammo_id = ammo_id(reader, bound)?;
        let round_chambered = match gun.chamber {
            Chamber::Saved => Some(flag(reader)?),
            Chamber::NotSaved => None,
        };
        let in_hand = match shape {
            Shape::User { list } => Some(InHand::read(reader, gun, list, bound)?),
            Shape::NoUser => None,
        };
        Ok(Self {
            random_generator,
            last_tick_time_ms,
            load_ammo_on_next_activate,
            ammo_in_magazine,
            fire_queue_type,
            current_ammo_id,
            round_chambered,
            in_hand,
        })
    }
}

/// A weapon's ammo switch, saved by a gun whose list has two or more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmmoSwitch {
    /// A list under two: nothing saved.
    NotSaved,
    /// No switch (`m_ammo_switch_time_in_ms` 0xFFFFFFFF).
    Idle,
    /// A switch at `time_ms` (a tick time + 1000) to `next_ammo_id`.
    To { time_ms: u32, next_ammo_id: u8 },
}

/// A shot's recoil (`recoil_calculator`, after `m_time_of_last_shot_in_ms`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shot {
    time_ms: u32,
    recoil: [Finite; 3],
    recoil_speed: [Finite; 3],
    error: [Finite; 3],
    integral: [Finite; 3],
}

impl Shot {
    /// `m_time_of_last_shot_in_ms`.
    pub fn time_ms(&self) -> u32 {
        self.time_ms
    }

    /// `m_recoil`.
    pub fn recoil(&self) -> [Finite; 3] {
        self.recoil
    }

    /// `m_recoil_speed`.
    pub fn recoil_speed(&self) -> [Finite; 3] {
        self.recoil_speed
    }

    /// Its `pid_controller<float3>`'s `m_error` (the recoil member that
    /// holds the controller, recoil+0x24, is not named anywhere).
    pub fn error(&self) -> [Finite; 3] {
        self.error
    }

    /// The pid controller's `m_integral`.
    pub fn integral(&self) -> [Finite; 3] {
        self.integral
    }
}

/// The recoil (`recoil_calculator::serialize` 0x331FF0, weapon+0x468).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recoil {
    random_seed: u32,
    last_shot: Option<Shot>,
}

impl Recoil {
    /// `m_random.seed()` (seeded from the user at activation; any u32).
    pub fn random_seed(&self) -> u32 {
        self.random_seed
    }

    /// The last shot, `None` for `m_time_of_last_shot_in_ms` 0xFFFFFFFF.
    pub fn last_shot(&self) -> Option<Shot> {
        self.last_shot
    }
}

/// The breath's state machine's states (`breath_vibration_calculator::
/// m_logic`), which save nothing (0x31AD50).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreathState {
    Normal,
    Holding,
    ShortBreathing,
}

/// The breath while aimed (`breath_vibration_calculator::serialize`
/// 0x31DA30, weapon+0x288).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Breath {
    phase_time: Finite,
    vibration: [Finite; 2],
    amplitude: Finite,
    breath_holding_reserve: Finite,
    speed_factor: Finite,
    penalty_factor: Finite,
    not_holding_breath_delay: Finite,
    state: BreathState,
}

impl Breath {
    /// `m_phase_time`.
    pub fn phase_time(&self) -> Finite {
        self.phase_time
    }

    /// `m_vibration`.
    pub fn vibration(&self) -> [Finite; 2] {
        self.vibration
    }

    /// `m_amplitude`.
    pub fn amplitude(&self) -> Finite {
        self.amplitude
    }

    /// `m_breath_holding_reserve`.
    pub fn breath_holding_reserve(&self) -> Finite {
        self.breath_holding_reserve
    }

    /// `m_speed_factor`.
    pub fn speed_factor(&self) -> Finite {
        self.speed_factor
    }

    /// `m_penalty_factor`.
    pub fn penalty_factor(&self) -> Finite {
        self.penalty_factor
    }

    /// `m_not_holding_breath_delay`.
    pub fn not_holding_breath_delay(&self) -> Finite {
        self.not_holding_breath_delay
    }

    /// Its state machine's state (order LIKELY).
    pub fn state(&self) -> BreathState {
        self.state
    }

    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        let phase_time = float(reader)?;
        let vibration = floats(reader)?;
        let [
            amplitude,
            breath_holding_reserve,
            speed_factor,
            penalty_factor,
            not_holding_breath_delay,
        ] = floats(reader)?;
        let state = match state_id(reader, 3)? {
            0 => BreathState::Normal,
            1 => BreathState::Holding,
            _ => BreathState::ShortBreathing,
        };
        Ok(Self {
            phase_time,
            vibration,
            amplitude,
            breath_holding_reserve,
            speed_factor,
            penalty_factor,
            not_holding_breath_delay,
            state,
        })
    }
}

/// The animation a sprint waits for (`m_sprint_animation_to_wait_for`):
/// 0, 2 or 4; 6 is none (every store, 0x2DEFAA indexes the user's
/// animation tables with it unbounded).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SprintAnimation(u8);

impl SprintAnimation {
    pub fn get(self) -> u8 {
        self.0
    }
}

/// What a gun with a user saves after the always-saved fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InHand {
    ammo_switch: AmmoSwitch,
    recoil: Recoil,
    character_dispersion: Transition,
    weapon_dispersion: Transition,
    skill_influence: Finite,
    aim_progress: Transition,
    breath: Option<Breath>,
    sprint_animation_to_wait_for: Option<SprintAnimation>,
    need_to_auto_reload: bool,
    state_index: u8,
    state: StateData,
    portable: Portable,
}

impl InHand {
    pub fn ammo_switch(&self) -> AmmoSwitch {
        self.ammo_switch
    }

    pub fn recoil(&self) -> Recoil {
        self.recoil
    }

    /// The dispersion calculator's `m_character_dispersion`.
    pub fn character_dispersion(&self) -> Transition {
        self.character_dispersion
    }

    /// The dispersion calculator's `m_weapon_dispersion`.
    pub fn weapon_dispersion(&self) -> Transition {
        self.weapon_dispersion
    }

    /// The dispersion calculator's `m_skill_influence`.
    pub fn skill_influence(&self) -> Finite {
        self.skill_influence
    }

    /// `m_aimed`: the breath is saved for an aimed gun only.
    pub fn aimed(&self) -> bool {
        self.breath.is_some()
    }

    /// `m_aim_progress`.
    pub fn aim_progress(&self) -> Transition {
        self.aim_progress
    }

    pub fn breath(&self) -> Option<Breath> {
        self.breath
    }

    /// `m_sprint_animation_to_wait_for`, `None` for 6.
    pub fn sprint_animation_to_wait_for(&self) -> Option<SprintAnimation> {
        self.sprint_animation_to_wait_for
    }

    /// `m_need_to_auto_reload`.
    pub fn need_to_auto_reload(&self) -> bool {
        self.need_to_auto_reload
    }

    /// The state machine's current state: its index among the gun's states.
    pub fn state_index(&self) -> u8 {
        self.state_index
    }

    /// That state's own bytes, by its class.
    pub fn state(&self) -> StateData {
        self.state
    }

    pub fn portable(&self) -> Portable {
        self.portable
    }

    fn read(reader: &mut Reader, gun: Gun, list: u8, bound: u8) -> Result<Self, Fault> {
        let ammo_switch = if list >= 2 {
            match optional_u32(reader)? {
                None => AmmoSwitch::Idle,
                Some(time_ms) => AmmoSwitch::To {
                    time_ms,
                    next_ammo_id: ammo_id(reader, bound)?,
                },
            }
        } else {
            AmmoSwitch::NotSaved
        };
        let random_seed = u32(reader)?;
        let last_shot = match optional_u32(reader)? {
            None => None,
            Some(time_ms) => Some(Shot {
                time_ms,
                recoil: floats(reader)?,
                recoil_speed: floats(reader)?,
                error: floats(reader)?,
                integral: floats(reader)?,
            }),
        };
        let character_dispersion = Transition::read(reader)?;
        let weapon_dispersion = Transition::read(reader)?;
        let skill_influence = float(reader)?;
        let aimed = flag(reader)?;
        let aim_progress = Transition::read(reader)?;
        let breath = if aimed {
            Some(Breath::read(reader)?)
        } else {
            None
        };
        let at = reader.at();
        let sprint_animation_to_wait_for = match u8(reader)? {
            6 => None,
            value @ (0 | 2 | 4) => Some(SprintAnimation(value)),
            value => {
                return Err(outside(
                    at,
                    "a weapon's m_sprint_animation_to_wait_for",
                    value.into(),
                ));
            }
        };
        let need_to_auto_reload = flag(reader)?;
        let at = reader.at();
        let state_index = u8(reader)?;
        let Some(&class) = gun.states.get(usize::from(state_index)) else {
            return Err((
                at,
                Reason::TailGunState {
                    state: state_index,
                    states: gun.states.len(),
                },
            ));
        };
        let state = StateData::read(reader, class)?;
        let portable = Portable::read(reader)?;
        Ok(Self {
            ammo_switch,
            recoil: Recoil {
                random_seed,
                last_shot,
            },
            character_dispersion,
            weapon_dispersion,
            skill_influence,
            aim_progress,
            breath,
            sprint_animation_to_wait_for,
            need_to_auto_reload,
            state_index,
            state,
            portable,
        })
    }
}

/// What every animation-end-aware state saves
/// (`weapon_core_animation_end_aware_state`): its base's
/// `m_animation_has_been_ended` and `m_index_of_animation_to_wait` (an
/// index of the state's own animations, LIKELY even; 0xFF none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnimationWait {
    animation_has_been_ended: bool,
    index_of_animation_to_wait: Option<u8>,
}

impl AnimationWait {
    /// `m_animation_has_been_ended`.
    pub fn animation_has_been_ended(&self) -> bool {
        self.animation_has_been_ended
    }

    /// `m_index_of_animation_to_wait`, `None` for 0xFF.
    pub fn index_of_animation_to_wait(&self) -> Option<u8> {
        self.index_of_animation_to_wait
    }

    pub(super) fn read(reader: &mut Reader) -> Result<Self, Fault> {
        Ok(Self {
            animation_has_been_ended: flag(reader)?,
            index_of_animation_to_wait: optional_u8(reader)?,
        })
    }
}

/// What a fire state saves (`weapon_core_fire_state_base`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fire {
    wait: AnimationWait,
    bullets_in_queue: u8,
    can_reset_fire_queue: bool,
}

impl Fire {
    pub fn wait(&self) -> AnimationWait {
        self.wait
    }

    /// `m_bullets_in_queue`.
    pub fn bullets_in_queue(&self) -> u8 {
        self.bullets_in_queue
    }

    /// `m_can_reset_fire_queue`.
    pub fn can_reset_fire_queue(&self) -> bool {
        self.can_reset_fire_queue
    }

    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        Ok(Self {
            wait: AnimationWait::read(reader)?,
            bullets_in_queue: u8(reader)?,
            can_reset_fire_queue: flag(reader)?,
        })
    }
}

/// A shotgun reload's sub-states (`weapon_core_shotgun_reload_state::
/// m_logic`; order LIKELY).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReloadStep {
    Start,
    OneRound,
    Finish,
}

/// A throw state's grenade slot for no grenade held (exe patch 59's
/// `NO_SLOT`, the inventory's own none: [`StateData::ThrowGrenade`]).
const NO_GRENADE: u8 = super::inventory::SLOTS;

/// A throw's sub-states (`weapon_core_throw_grenade_state::m_logic`; names
/// and order LIKELY).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThrowStep {
    PullPin,
    Idle,
    Throw,
}

/// A weapon state's own bytes, by its class ([`GunState`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateData {
    /// Idle, preview: `m_animation_has_been_ended`.
    Idle { animation_has_been_ended: bool },
    /// Show, hide, reload, chamber.
    AnimationEnd(AnimationWait),
    Fire(Fire),
    /// A double-barrel's or a pistol's fire: + `m_weapon_animation_index`
    /// (0 or 1 in every world; the constructor's 0xFF LIKELY never saved).
    FireTwice { fire: Fire, weapon_animation_index: u8 },
    Melee {
        wait: AnimationWait,
        melee_animation_ended: bool,
    },
    ShotgunReload {
        animation_has_been_ended: bool,
        step: ReloadStep,
        step_wait: AnimationWait,
    },
    ThrowGrenade {
        animation_has_been_ended: bool,
        step: ThrowStep,
        step_wait: AnimationWait,
        /// The grenade's `profile_slot_id()` (`[+0x17C]+0x254`, 0x33A51B), an
        /// index into the slots its load reads with no bound in 0.26g
        /// (0x33A5AB..0x33A5BD); `None` for 23 (0x17), which exe patch 59
        /// writes for no grenade held (a Sponge let go: `sponge_save` at
        /// 0x33A51B) and loads as none (`sponge_load`, any slot of 23 or
        /// more). Stock 0.26g never writes it (it reads `[0+0x254]`).
        grenade_slot: Option<SlotId>,
        /// `m_grenade_index`, `None` for 0xFF.
        grenade_index: Option<u8>,
    },
}

impl StateData {
    /// The class it is of.
    pub fn class(&self) -> GunState {
        match self {
            StateData::Idle { .. } => GunState::Flag,
            StateData::AnimationEnd(_) => GunState::FlagByte,
            StateData::Fire(_) => GunState::Fire,
            StateData::FireTwice { .. } => GunState::FireTwice,
            StateData::Melee { .. } => GunState::Melee,
            StateData::ShotgunReload { .. } => GunState::ShotgunReload,
            StateData::ThrowGrenade { .. } => GunState::ThrowGrenade,
        }
    }

    fn read(reader: &mut Reader, class: GunState) -> Result<Self, Fault> {
        Ok(match class {
            GunState::Flag => StateData::Idle {
                animation_has_been_ended: flag(reader)?,
            },
            GunState::FlagByte => StateData::AnimationEnd(AnimationWait::read(reader)?),
            GunState::Fire => StateData::Fire(Fire::read(reader)?),
            GunState::FireTwice => StateData::FireTwice {
                fire: Fire::read(reader)?,
                weapon_animation_index: u8(reader)?,
            },
            GunState::Melee => StateData::Melee {
                wait: AnimationWait::read(reader)?,
                melee_animation_ended: flag(reader)?,
            },
            GunState::ShotgunReload => StateData::ShotgunReload {
                animation_has_been_ended: flag(reader)?,
                step: match state_id(reader, 3)? {
                    0 => ReloadStep::Start,
                    1 => ReloadStep::OneRound,
                    _ => ReloadStep::Finish,
                },
                step_wait: AnimationWait::read(reader)?,
            },
            GunState::ThrowGrenade => {
                let animation_has_been_ended = flag(reader)?;
                let step = match state_id(reader, 3)? {
                    0 => ThrowStep::PullPin,
                    1 => ThrowStep::Idle,
                    _ => ThrowStep::Throw,
                };
                let step_wait = AnimationWait::read(reader)?;
                let at = reader.at();
                let slot = u8(reader)?;
                let grenade_slot = match slot {
                    NO_GRENADE => None,
                    slot => Some(SlotId::new(slot).ok_or((at, Reason::TailGrenadeSlot { slot }))?),
                };
                StateData::ThrowGrenade {
                    animation_has_been_ended,
                    step,
                    step_wait,
                    grenade_slot,
                    grenade_index: optional_u8(reader)?,
                }
            }
        })
    }
}

/// `jump_type_enum` (0.21d PDB), as `m_jump_type` saves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpType {
    NoJump,
    OnSite,
    SprintFwd,
    SprintFwdRight,
    SprintFwdLeft,
    FromSiteFwd,
    FromSiteFwdRight,
    FromSiteRight,
    FromSiteBackRight,
    FromSiteBack,
    FromSiteBackLeft,
    FromSiteLeft,
    FromSiteFwdLeft,
}

impl JumpType {
    /// The type of a byte, if the enum has it (0..=12).
    pub const fn of(byte: u8) -> Option<JumpType> {
        Some(match byte {
            0 => JumpType::NoJump,
            1 => JumpType::OnSite,
            2 => JumpType::SprintFwd,
            3 => JumpType::SprintFwdRight,
            4 => JumpType::SprintFwdLeft,
            5 => JumpType::FromSiteFwd,
            6 => JumpType::FromSiteFwdRight,
            7 => JumpType::FromSiteRight,
            8 => JumpType::FromSiteBackRight,
            9 => JumpType::FromSiteBack,
            10 => JumpType::FromSiteBackLeft,
            11 => JumpType::FromSiteLeft,
            12 => JumpType::FromSiteFwdLeft,
            _ => return None,
        })
    }

    /// A sprint jump (`jump_logic`'s prepare, start, landing); the others
    /// are short jumps (0x337040).
    const fn sprint(self) -> bool {
        matches!(
            self,
            JumpType::SprintFwd | JumpType::SprintFwdRight | JumpType::SprintFwdLeft
        )
    }

    /// A jump on the spot or a sprint jump: its on-site weight is exactly
    /// 1.0 (0x337302).
    const fn full_on_site_weight(self) -> bool {
        matches!(self, JumpType::OnSite) || self.sprint()
    }
}

pub(super) fn jump_type(reader: &mut Reader, field: &'static str) -> Result<JumpType, Fault> {
    let at = reader.at();
    let byte = u8(reader)?;
    JumpType::of(byte).ok_or(outside(at, field, byte.into()))
}

/// What every jump sub-state saves first: `m_interval_id_to_wait_for`
/// (0xFF none) and `m_is_jump_finished`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JumpWait {
    interval_id_to_wait_for: Option<u8>,
    is_jump_finished: bool,
}

impl JumpWait {
    /// `m_interval_id_to_wait_for`, `None` for 0xFF.
    pub fn interval_id_to_wait_for(&self) -> Option<u8> {
        self.interval_id_to_wait_for
    }

    /// `m_is_jump_finished`.
    pub fn is_jump_finished(&self) -> bool {
        self.is_jump_finished
    }

    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        Ok(Self {
            interval_id_to_wait_for: optional_u8(reader)?,
            is_jump_finished: flag(reader)?,
        })
    }
}

/// The stand-jump state's sub-states (0x336E60), with what each saves after
/// its [`JumpWait`]. A sprint jump goes prepare, start, landing; any other
/// a short start and a short landing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JumpStep {
    /// `jump_logic_state_prepare`.
    Prepare {
        wait: JumpWait,
        time_scale: Finite,
        prepare_interval_ended: bool,
    },
    /// `jump_logic_state_start`.
    Start {
        wait: JumpWait,
        preface_interval_ended: bool,
        jump_interval_ended: bool,
    },
    /// `jump_logic_state_landing`.
    Landing {
        wait: JumpWait,
        landing_interval_ended: bool,
    },
    /// `short_jump_start_state`.
    ShortStart {
        wait: JumpWait,
        jump_animation_ended: bool,
    },
    /// `short_jump_landing_state`.
    ShortLanding { wait: JumpWait },
}

/// The selector's jump (`player_logic_stand_jump_state` 0x337510).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Jump {
    jump_type: JumpType,
    from_right_leg: bool,
    weight_for_on_site_animation: Finite,
    step: JumpStep,
}

impl Jump {
    /// `m_jump_type`, never [`JumpType::NoJump`] (its only writer,
    /// initialize 0x337040).
    pub fn jump_type(&self) -> JumpType {
        self.jump_type
    }

    /// `m_from_right_leg`.
    pub fn from_right_leg(&self) -> bool {
        self.from_right_leg
    }

    /// `m_weight_for_on_site_animation`: 1.0 for a jump on the spot or a
    /// sprint jump.
    pub fn weight_for_on_site_animation(&self) -> Finite {
        self.weight_for_on_site_animation
    }

    pub fn step(&self) -> JumpStep {
        self.step
    }

    fn read(reader: &mut Reader) -> Result<Self, Fault> {
        let type_at = reader.at();
        let jump_type = jump_type(reader, "a jump's m_jump_type")?;
        if jump_type == JumpType::NoJump {
            return Err(outside(type_at, "a jump's m_jump_type", 0));
        }
        let from_right_leg = flag(reader)?;
        let weight_at = reader.at();
        let weight_for_on_site_animation = float(reader)?;
        if jump_type.full_on_site_weight() && weight_for_on_site_animation.bits() != 1f32.to_bits() {
            return Err(outside(
                weight_at,
                "a jump's m_weight_for_on_site_animation",
                weight_for_on_site_animation.bits(),
            ));
        }
        let step_at = reader.at();
        let step = state_id(reader, 5)?;
        // A sprint jump's steps are the first three, any other's the last
        // two (0x337040).
        if jump_type.sprint() != (step < 3) {
            return Err(outside(step_at, "a jump's sub-state for its type", step.into()));
        }
        let wait = JumpWait::read(reader)?;
        let step = match step {
            0 => JumpStep::Prepare {
                wait,
                time_scale: float(reader)?,
                prepare_interval_ended: flag(reader)?,
            },
            1 => JumpStep::Start {
                wait,
                preface_interval_ended: flag(reader)?,
                jump_interval_ended: flag(reader)?,
            },
            2 => JumpStep::Landing {
                wait,
                landing_interval_ended: flag(reader)?,
            },
            3 => JumpStep::ShortStart {
                wait,
                jump_animation_ended: flag(reader)?,
            },
            _ => JumpStep::ShortLanding { wait },
        };
        Ok(Self {
            jump_type,
            from_right_leg,
            weight_for_on_site_animation,
            step,
        })
    }
}

/// The animation selector's state (`weapon_user_animations_selector`'s
/// machine, constructor 0x2FAB70), by its byte 0..6.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorState {
    /// `player_logic_stand_on_site_state`.
    StandOnSite,
    /// `player_logic_stand_run_state`.
    StandRun,
    /// `player_logic_stand_sprint_state`.
    StandSprint,
    /// `player_logic_stand_jump_state`.
    StandJump(Jump),
    /// `player_logic_crouch_on_site_state`.
    CrouchOnSite,
    /// `player_logic_crouch_walk_state`.
    CrouchWalk,
    /// `player_logic_dead_state` (0x338430): `m_is_ready_to_be_deactivated`
    /// and `m_animation_index` (under the user's death animations, a count
    /// of the game data: not bounded here).
    Dead {
        is_ready_to_be_deactivated: bool,
        animation_index: u8,
    },
}

/// A body part of the hit animations selector
/// (`hit_animations_selector::serialize` 0x3309B0, its `hit_body_part`s
/// inlined).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HitBodyPart {
    last_hit_time_ms: u32,
    current_hit_amount: u32,
    target_hit_amount: u32,
}

impl HitBodyPart {
    /// `last_hit_time` (0 until the first hit).
    pub fn last_hit_time_ms(&self) -> u32 {
        self.last_hit_time_ms
    }

    /// `current_hit_amount`.
    pub fn current_hit_amount(&self) -> u32 {
        self.current_hit_amount
    }

    /// `target_hit_amount`.
    pub fn target_hit_amount(&self) -> u32 {
        self.target_hit_amount
    }
}

/// The hit animations selector's body parts, in their order (the name
/// table 0x91FF08).
pub const HIT_BODY_PARTS: [&str; 8] = [
    "head",
    "back",
    "left_arm",
    "right_arm",
    "left_chest",
    "right_chest",
    "left_leg",
    "right_leg",
];

/// A portable object (`portable_interactive_object_core::deserialize`
/// 0x2FF970, a weapon's and a battery's; the selector's deserialize is
/// inlined): the animation selector's `m_random` seed, its
/// `m_right_leg_is_supporting`, its state machine's state, the hit
/// animations' body parts. Its legs' IK (`m_legs_ik_solver`) only with the
/// byte at 0x1F1D9EF set, which no game sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Portable {
    random_seed: u32,
    right_leg_is_supporting: bool,
    selector: SelectorState,
    hit_body_parts: [HitBodyPart; 8],
}

impl Portable {
    /// The selector's `m_random.seed()` (an LCG of full period: any u32).
    pub fn random_seed(&self) -> u32 {
        self.random_seed
    }

    /// `m_right_leg_is_supporting`.
    pub fn right_leg_is_supporting(&self) -> bool {
        self.right_leg_is_supporting
    }

    pub fn selector(&self) -> SelectorState {
        self.selector
    }

    /// By [`HIT_BODY_PARTS`].
    pub fn hit_body_parts(&self) -> [HitBodyPart; 8] {
        self.hit_body_parts
    }

    pub(super) fn read(reader: &mut Reader) -> Result<Self, Fault> {
        let random_seed = u32(reader)?;
        let right_leg_is_supporting = flag(reader)?;
        let selector = match state_id(reader, 7)? {
            0 => SelectorState::StandOnSite,
            1 => SelectorState::StandRun,
            2 => SelectorState::StandSprint,
            3 => SelectorState::StandJump(Jump::read(reader)?),
            4 => SelectorState::CrouchOnSite,
            5 => SelectorState::CrouchWalk,
            _ => SelectorState::Dead {
                is_ready_to_be_deactivated: flag(reader)?,
                animation_index: u8(reader)?,
            },
        };
        let mut hit_body_parts = [HitBodyPart {
            last_hit_time_ms: 0,
            current_hit_amount: 0,
            target_hit_amount: 0,
        }; 8];
        for part in &mut hit_body_parts {
            *part = HitBodyPart {
                last_hit_time_ms: u32(reader)?,
                current_hit_amount: u32(reader)?,
                target_hit_amount: u32(reader)?,
            };
        }
        Ok(Self {
            random_seed,
            right_leg_is_supporting,
            selector,
            hit_body_parts,
        })
    }
}


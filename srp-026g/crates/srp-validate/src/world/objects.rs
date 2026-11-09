//! The game-objects section (world apply 0x2D2770: the walk 0x2D2BB0..
//! 0x2D2BE6 over the list at core+0x18914, `node->vtable[1]`), read as
//! typed views ([`GameObjects`]).
//!
//! The section is a bare concatenation of the objects of one list, with
//! **no count, no id and no length per object**: each object reads its own
//! bytes. So the section can be read only when the list is known
//! ([`Context::objects`]); without it (a tool reading kept worlds) the
//! section is not checked. The list is (PROVEN, client_patch.md 3.14 and
//! the four functions that append to it):
//!
//! 1. The places' grenades and traps ([`Held`]), the newest bound place
//!    first (`late_join_026g.asm` `bind_slot_reserved`): a place's kits
//!    that have a player object (0x321820), each kit's 23 inventory slots
//!    (0x2FC7D0), one object for each grenade of a grenade stack (0x309EC0
//!    -> 0x303820) and for each trap of a trap stack (0x3067A0).
//! 2. The map's objects (`simple_game_project::insert` 0x285750), from its
//!    `client_objects` in config order within each kind ([`Map`]): the
//!    damage zones no anomaly drives, the resupply zones, the effect zones
//!    (which save nothing) and the anomalies. A map's artefact containers
//!    are read by their anomaly, its battery containers by the gather rule
//!    (the rules section).
//!
//! What each object saves here (what it reads from the effects stream is
//! that section's). The field names are the ones each `serialize` hands
//! the packet writer with the field (its debug name, a string beside the
//! call); PROVEN for the layouts (each `serialize` and its `deserialize`,
//! addresses below) unless a line says LIKELY:
//!
//! ```text
//! grenade_core           serialize 0x303DA0, deserialize 0x303F80
//!   u8   current_state   bit 0: in the physics world (+0x138 set),
//!                        bit 1: thrown (+0x13C m_explode_time_ms not -1),
//!                        bit 2: +0x144 (meaning unknown); no other bit
//!   bit 1:  u32 m_explode_time_ms, u32 m_id
//!   bit 0:  f32 x16 m_transform, f32 x3 linear_velocity,
//!           f32 x3 angular_velocity
//!
//! booby_trap_core        serialize 0x3052E0, deserialize 0x3053C0
//!   u8   m_trap_state    0 removed, 1 armed, 2 fired, 3 disarmed
//!                        (switch_to_state 0x305210's jump table of 4;
//!                        the names are 0.10b's booby_trap_state)
//!   not removed: u32 m_action_time_in_ms (delayed_action), f32 x16
//!           m_transform, u32 m_state_timer, u32 m_deploy_time_in_ms
//!   armed:  the usable object
//!
//! usable_object          serialize 0x2F8460, deserialize 0x2F84C0
//!   u8   m_usable_object_users.size()   at most 20 (an inline buffer of
//!                        20, 0xF3090, that a 21st overflows)
//!   u8 x N  owner.id     a place, dereferenced in the place table of 20
//!                        with no bound (0x2F8510)
//!
//! damage_zone_core       serialize 0x3193F0, deserialize 0x319470
//!   u32  m_last_hit_time_ms
//!   u8   m_hitting       a bool: the writer 0x80FA0 writes 0x00 or 0xFF
//!
//! resupply zone          its usable object only
//!
//! generic_anomaly_core   serialize 0x31AF20, deserialize 0x31B0A0
//!   artefacts_enabled (+0xC):
//!     u8  m_need_to_spawn_artefacts      a bool, 0x00 or 0xFF
//!     set: u32 m_next_artefact_spawn_time_ms
//!   f32  m_energy_current
//!   u8   current_state_index   0xFF none, else an index into m_states
//!                        (+0x68, sorted by state_prio 0x33C400), read
//!                        with no bound
//!   the current state (anomaly_state, serialize 0x31B1D0, deserialize
//!   0x31B230): u32 m_finish_time_ms, then each of its zone groups
//!   (zone_group, serialize 0x31B2A0, deserialize 0x31B390):
//!     not keep_all_zones_active (+0x1): u32 m_next_recharge_time, a
//!       current_byte_mask byte for each 8 zones (bit z: zone z is on;
//!       the writer sets no bit past the zones)
//!     each zone that is on: its damage_zone_core
//!   u8   events_mask     bit 0 m_was_zone_trigger_event (+0x78), bit 1
//!                        m_was_shoot_trigger_event (+0x79): select_state
//!                        0x31BA90 pairs them with a state's
//!                        zone_activity_trigger and shoot_trigger
//!                        (0x31BAD7, 0x31BAED); no other bit
//!   u32  m_random.seed()
//!   its artefact containers (artefact_container_core, serialize
//!   0x31C530, deserialize 0x31C5B0): the usable object; u8 the
//!   artefact's id (0xFF none, else an **unbounded** index into the
//!   world's artefacts, [core+0x18940], which are the map's anomalies'
//!   own, register_artefacts 0x31B8E0); u32 m_next_artefact_spawn_time_ms
//!   its own artefacts
//!
//! artefact               inventory_item::serialize 0x2FFDF0 /
//!                        deserialize 0x2FFF10, then artefact_base
//!                        serialize 0x313670 / deserialize 0x313760
//!   u16  m_amount        (an f32 m_condition for an item whose params'
//!                        +0x22A is clear; an artefact's is set: every
//!                        world reads so)
//!   u8   player_id       0xFF none, else a place, then
//!   u8   player_profile_id   its kit: [[core+0x18458] + place*0x30 +
//!                        kit*4] with no bound, the place table of 20,
//!                        three kits with a player object
//!   when that kit has a player: u8 m_slot_id (its profile slot)
//!   u8   m_state         0 free, 1 spawning in a container, 2 ready in
//!                        it, 3 taken, 4 active, 5 (below); a Sponge's also
//!                        6 armed and 7 exploded (exe patch 59); no other
//!   1, 2: u8 m_container_id; 1: u32 m_time_left_to_spawn
//!   else: u32 m_time_left_to_cool
//!   6: patch 78's carry record, 20 bytes (`carry.ground_save` 0x313735,
//!      before the next field's write; read back by `carry.ground_load`
//!      0x3137EC, before 0x3137F7): every artefact in state 6, a Sponge's
//!      0xFF x 20 (no release: the carry never drops a Sponge)
//!   u32  m_last_activation_time_ms
//!   4: what its core saves of its own, by its item's data.type (the
//!   artefact cook's switch 0x2EE95F):
//!     3 lifebone (0x312470 / 0x3124B0): u32 m_time_left_to_deactivate
//!     6 spring (0x3119A0 / 0x3119E0): u32 m_time_left_to_deactivate
//!     7 onyx (0x312A80 / 0x312AF0): u32 m_time_left_to_deactivate,
//!       f32 m_damage_to_absorb
//!   12 the Sponge (exe patch 59, `sp_serialize` / `sp_deserialize` after
//!   artefact_base's; never 4: its enable_active is none): in every state
//!   its own fields, as `sponge::record` reads them (m_explode_time_ms,
//!   m_is_pin_pulled, thrown() and in the air its motion, m_contact,
//!   m_zone_owner, m_throw_state_holds); exploded with its cloud on, then
//!   the zone's damage_zone_core and f32 x16, the explosion's transform
//! ```
//!
//! The artefact states (PROVEN where they are set): on_removed 0x313490
//! sets 0; spawning in a container (0x3134C0, from spawn_artefact) sets 1
//! with the container's id ([container+0x64], its constructor's argument
//! 0x31C44E) and the spawn time; tick 0x313200 turns 1 into 2 when the
//! time runs out; on_taken 0x313470 sets 3; end_active 0x313530 sets 3
//! after 4; tick sets 5 when its holder's player has [+0x33C] clear and
//! the stack is not empty, after its vtable +0x84 (LIKELY: its passive
//! effects off while the holder is down).
//!
//! The readers hold every field to what the game writes: a byte that
//! selects a state is one of its states, a bool 0x00 or 0xFF, a flags byte
//! has only its bits, a float is a number ([`Finite`]), an index is within
//! the table the reader has (a state of the anomaly, a place of the table
//! of 20, a kit of three, a zone of the group), a count within its buffer.
//! Any other value refuses the world, naming the field ([`ObjectFault`]).
//! What the match decides is checked over the views in [`objects`]: a user
//! past the roster or on two objects, an artefact id past the map's, an
//! owner whose kit has no player.
//!
//! Left open (any value the field holds is taken): the u32 times, ids and
//! seeds; an artefact's amount, its slot and its container id (the writer
//! copies them from the live objects, and nothing here shows the set it
//! keeps them in).

use super::{Context, Profile, Section};
use crate::reader::{Reader, Short};
use crate::{Finite, KITS, MAX_PLAYERS, Part, Place, Reason, Refusal};

/// The list of objects a world's section holds, in its order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Objects<'a> {
    /// The places' grenades and traps, in the list's order.
    pub held: &'a [Held],
    pub map: &'a Map<'a>,
}

/// A grenade or a trap a place's kit holds: one object of the list each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Grenade,
    Trap,
}

/// The map's objects in the list (its project's `client_objects`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Map<'a> {
    /// The damage zones no anomaly's group names (an anomaly's zones are
    /// read by their anomaly: `generic_anomaly_core::resolve_links`
    /// 0x33C590 marks them and insert skips them).
    pub damage_zones: usize,
    pub resupply_zones: usize,
    pub anomalies: &'a [Anomaly<'a>],
}

impl Map<'_> {
    /// The world's artefacts table: every anomaly's own artefacts, in list
    /// order (`register_artefacts` 0x31B8E0), which a container's id names.
    pub fn artefacts(&self) -> usize {
        self.anomalies
            .iter()
            .filter_map(|anomaly| anomaly.artefacts)
            .map(|artefacts| artefacts.artefacts.len())
            .fold(0, usize::saturating_add)
    }
}

/// A map's `generic_anomaly_core`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anomaly<'a> {
    /// With `artefacts_enabled`: its containers and artefacts; without, it
    /// has neither and saves no `m_need_to_spawn_artefacts`.
    pub artefacts: Option<Artefacts<'a>>,
    /// Its `states`, in the game's order (sorted by `state_prio`).
    pub states: &'a [State<'a>],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Artefacts<'a> {
    /// `artefact_containers`.
    pub containers: usize,
    /// In the world's order: each of `artefacts[]` its `count` times.
    pub artefacts: &'a [Artefact],
}

/// An artefact's core, by its item's `data.type` (the artefact cook's
/// switch 0x2EE95F): what an active one saves of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Artefact {
    /// 3 (`artefact_lifebone_core`).
    Lifebone,
    /// 6 (`artefact_spring_core`).
    Spring,
    /// 7 (`artefact_onyx_core`).
    Onyx,
    /// 12 (exe patch 59's Sponge, items_dict 758): saves its own fields in
    /// every state, and has the states 6 and 7 ([`ArtefactState`]).
    Sponge,
}

impl Artefact {
    /// What an untouched artefact of it (free, never activated) saves after
    /// artefact_base's record: the Sponge's fields unarmed.
    pub fn untouched_own(self) -> &'static [u8] {
        match self {
            Artefact::Lifebone | Artefact::Spring | Artefact::Onyx => &[],
            Artefact::Sponge => &SPONGE_UNARMED,
        }
    }

    /// The core an active one has: none for the Sponge, which is never
    /// active.
    fn activated(self) -> Option<Activated> {
        match self {
            Artefact::Lifebone => Some(Activated::Lifebone),
            Artefact::Spring => Some(Activated::Spring),
            Artefact::Onyx => Some(Activated::Onyx),
            Artefact::Sponge => None,
        }
    }
}

/// The kinds of artefact that are ever active (state 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Activated {
    Lifebone,
    Spring,
    Onyx,
}

/// A Sponge's own fields unarmed (`sponge::record::write` of
/// `Saved::Held`): no fuse, no pin, not thrown, no contact, nobody, no hold.
const SPONGE_UNARMED: [u8; 9] = [0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0xFF, 0];

/// An anomaly's state: its zone groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct State<'a> {
    pub groups: &'a [Group],
}

/// A zone group of a state, with its zone count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// `keep_all_zones_active`: every zone is on while the state is.
    KeepAll { zones: usize },
    /// Its zones turn on and off: a recharge time and a bit for each zone
    /// are saved.
    Switched { zones: usize },
}

/// Why bytes are no object of the list: where (in the message) and what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectFault {
    Short(Short),
    /// A field (a byte, or a Sponge's u32) is none of the values it takes.
    Value {
        at: usize,
        field: &'static str,
        value: u32,
    },
    /// A float is no number.
    NotNumber {
        at: usize,
        field: &'static str,
    },
}

impl ObjectFault {
    /// Where in the message it is.
    pub fn at(self) -> usize {
        match self {
            ObjectFault::Short(short) => short.at,
            ObjectFault::Value { at, .. } | ObjectFault::NotNumber { at, .. } => at,
        }
    }
}

impl From<Short> for ObjectFault {
    fn from(short: Short) -> Self {
        ObjectFault::Short(short)
    }
}

impl core::fmt::Display for ObjectFault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            ObjectFault::Short(short) => write!(f, "cut short at {}", short.at),
            ObjectFault::Value { at, field, value } => {
                write!(
                    f,
                    "its {field} at {at} is {value:#04x}, which the game never writes"
                )
            }
            ObjectFault::NotNumber { at, field } => write!(f, "its {field} at {at} is no number"),
        }
    }
}

/// The next byte, as `parse` makes it its field's value, or a fault
/// naming the field.
fn byte<T>(
    reader: &mut Reader,
    field: &'static str,
    parse: impl FnOnce(u8) -> Option<T>,
) -> Result<T, ObjectFault> {
    let at = reader.at();
    let value = reader.u8()?;
    parse(value).ok_or(ObjectFault::Value {
        at,
        field,
        value: value.into(),
    })
}

/// A bool as the game's writer puts it (0x80FA0): 0x00 or 0xFF.
fn flag(reader: &mut Reader, field: &'static str) -> Result<bool, ObjectFault> {
    byte(reader, field, |value| match value {
        0x00 => Some(false),
        0xFF => Some(true),
        _ => None,
    })
}

/// A byte of flags that has no bit but `known`'s.
fn bits(reader: &mut Reader, field: &'static str, known: u8) -> Result<u8, ObjectFault> {
    byte(reader, field, |value| {
        (value & !known == 0).then_some(value)
    })
}

fn number(reader: &mut Reader, field: &'static str) -> Result<Finite, ObjectFault> {
    let at = reader.at();
    Finite::of_bits(reader.f32_bits()?).ok_or(ObjectFault::NotNumber { at, field })
}

fn numbers<const N: usize>(
    reader: &mut Reader,
    field: &'static str,
) -> Result<[Finite; N], ObjectFault> {
    let mut floats = [Finite::ZERO; N];
    for float in &mut floats {
        *float = number(reader, field)?;
    }
    Ok(floats)
}

/// A place as an object names it (a player's `id`): one of the game's
/// place table of 20.
fn read_place(reader: &mut Reader, field: &'static str) -> Result<Place, ObjectFault> {
    byte(reader, field, Place::new)
}

/// A kit of a place (`player_profile_id`): one of its three player
/// objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kit(u8);

impl Kit {
    pub fn get(self) -> u8 {
        self.0
    }
}

/// A user of a usable object: its place and where it is in the message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct User {
    at: usize,
    place: Place,
}

impl User {
    pub fn at(self) -> usize {
        self.at
    }

    pub fn place(self) -> Place {
        self.place
    }
}

/// A `usable_object`'s users (`m_usable_object_users`), at most 20.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsableObject {
    /// Where the first user is.
    at: usize,
    count: u8,
    users: [Place; MAX_PLAYERS],
}

impl UsableObject {
    fn read(reader: &mut Reader) -> Result<Self, ObjectFault> {
        let count = byte(reader, "m_usable_object_users.size()", |count| {
            (usize::from(count) <= MAX_PLAYERS).then_some(count)
        })?;
        let at = reader.at();
        let mut users = [Place::FIRST; MAX_PLAYERS];
        for user in users.iter_mut().take(usize::from(count)) {
            *user = read_place(reader, "owner.id")?;
        }
        Ok(Self { at, count, users })
    }

    pub fn users(&self) -> impl Iterator<Item = User> + '_ {
        let at = self.at;
        self.users
            .iter()
            .take(usize::from(self.count))
            .zip(at..)
            .map(|(&place, at)| User { at, place })
    }
}

/// A grenade's `grenade_core`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrenadeCore {
    thrown: Option<Thrown>,
    motion: Option<Motion>,
    flag_144: bool,
}

/// What a thrown grenade saves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Thrown {
    explode_time_ms: u32,
    id: u32,
}

impl Thrown {
    /// `m_explode_time_ms`: when it goes off.
    pub fn explode_time_ms(&self) -> u32 {
        self.explode_time_ms
    }

    /// `m_id` (the writer's name; what it identifies is unknown).
    pub fn id(&self) -> u32 {
        self.id
    }
}

/// Where a body in the physics world is and how it moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Motion {
    transform: [Finite; 16],
    linear_velocity: [Finite; 3],
    angular_velocity: [Finite; 3],
}

impl Motion {
    fn read(reader: &mut Reader) -> Result<Self, ObjectFault> {
        Ok(Self {
            transform: numbers(reader, "m_transform")?,
            linear_velocity: numbers(reader, "linear_velocity")?,
            angular_velocity: numbers(reader, "angular_velocity")?,
        })
    }

    /// `m_transform`, a 4x4 matrix by rows.
    pub fn transform(&self) -> [Finite; 16] {
        self.transform
    }

    pub fn linear_velocity(&self) -> [Finite; 3] {
        self.linear_velocity
    }

    pub fn angular_velocity(&self) -> [Finite; 3] {
        self.angular_velocity
    }
}

impl GrenadeCore {
    const IN_PHYSICS: u8 = 1;
    const THROWN: u8 = 2;
    const FLAG_144: u8 = 4;

    fn read(reader: &mut Reader) -> Result<Self, ObjectFault> {
        let state = bits(
            reader,
            "grenade current_state",
            Self::IN_PHYSICS | Self::THROWN | Self::FLAG_144,
        )?;
        let thrown = if state & Self::THROWN != 0 {
            Some(Thrown {
                explode_time_ms: reader.u32()?,
                id: reader.u32()?,
            })
        } else {
            None
        };
        let motion = if state & Self::IN_PHYSICS != 0 {
            Some(Motion::read(reader)?)
        } else {
            None
        };
        Ok(Self {
            thrown,
            motion,
            flag_144: state & Self::FLAG_144 != 0,
        })
    }

    /// Set when it was thrown (bit 1 of `current_state`).
    pub fn thrown(&self) -> Option<Thrown> {
        self.thrown
    }

    /// Set while it is in the physics world (bit 0).
    pub fn motion(&self) -> Option<Motion> {
        self.motion
    }

    /// Bit 2: the grenade core's bool at +0x144 (meaning unknown; no code
    /// of `grenade_core` but its serializers reads it).
    pub fn flag_144(&self) -> bool {
        self.flag_144
    }
}

/// A trap's `m_trap_state` (0.10b's `booby_trap_state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrapState {
    Removed,
    Armed,
    Fired,
    Disarmed,
}

impl TrapState {
    fn of(byte: u8) -> Option<Self> {
        Some(match byte {
            0 => TrapState::Removed,
            1 => TrapState::Armed,
            2 => TrapState::Fired,
            3 => TrapState::Disarmed,
            _ => return None,
        })
    }
}

/// What a trap that is not removed saves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacedTrap {
    action_time_ms: u32,
    transform: [Finite; 16],
    state_timer: u32,
    deploy_time_ms: u32,
}

impl PlacedTrap {
    fn read(reader: &mut Reader) -> Result<Self, ObjectFault> {
        Ok(Self {
            action_time_ms: reader.u32()?,
            transform: numbers(reader, "m_transform")?,
            state_timer: reader.u32()?,
            deploy_time_ms: reader.u32()?,
        })
    }

    /// Its `delayed_action`'s `m_action_time_in_ms`.
    pub fn action_time_ms(&self) -> u32 {
        self.action_time_ms
    }

    /// `m_transform`, a 4x4 matrix by rows.
    pub fn transform(&self) -> [Finite; 16] {
        self.transform
    }

    /// `m_state_timer`.
    pub fn state_timer(&self) -> u32 {
        self.state_timer
    }

    /// `m_deploy_time_in_ms`.
    pub fn deploy_time_ms(&self) -> u32 {
        self.deploy_time_ms
    }
}

/// A trap's `booby_trap_core`, by its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoobyTrapCore {
    Removed,
    /// Set and waiting: the players disarming it are its users.
    Armed {
        placed: PlacedTrap,
        usable: UsableObject,
    },
    Fired(PlacedTrap),
    Disarmed(PlacedTrap),
}

impl BoobyTrapCore {
    fn read(reader: &mut Reader) -> Result<Self, ObjectFault> {
        Ok(match byte(reader, "m_trap_state", TrapState::of)? {
            TrapState::Removed => BoobyTrapCore::Removed,
            TrapState::Armed => BoobyTrapCore::Armed {
                placed: PlacedTrap::read(reader)?,
                usable: UsableObject::read(reader)?,
            },
            TrapState::Fired => BoobyTrapCore::Fired(PlacedTrap::read(reader)?),
            TrapState::Disarmed => BoobyTrapCore::Disarmed(PlacedTrap::read(reader)?),
        })
    }

    pub fn state(&self) -> TrapState {
        match self {
            BoobyTrapCore::Removed => TrapState::Removed,
            BoobyTrapCore::Armed { .. } => TrapState::Armed,
            BoobyTrapCore::Fired(_) => TrapState::Fired,
            BoobyTrapCore::Disarmed(_) => TrapState::Disarmed,
        }
    }

    pub fn placed(&self) -> Option<PlacedTrap> {
        match *self {
            BoobyTrapCore::Removed => None,
            BoobyTrapCore::Armed { placed, .. }
            | BoobyTrapCore::Fired(placed)
            | BoobyTrapCore::Disarmed(placed) => Some(placed),
        }
    }
}

/// A `damage_zone_core`: a map's lone one, or a zone of an anomaly's group
/// that is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DamageZoneCore {
    last_hit_time_ms: u32,
    hitting: bool,
}

impl DamageZoneCore {
    fn read(reader: &mut Reader) -> Result<Self, ObjectFault> {
        Ok(Self {
            last_hit_time_ms: reader.u32()?,
            hitting: flag(reader, "m_hitting")?,
        })
    }

    /// `m_last_hit_time_ms`.
    pub fn last_hit_time_ms(&self) -> u32 {
        self.last_hit_time_ms
    }

    /// `m_hitting`.
    pub fn hitting(&self) -> bool {
        self.hitting
    }
}

/// A resupply zone: its usable object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResupplyZoneCore {
    usable: UsableObject,
}

impl ResupplyZoneCore {
    pub fn usable(&self) -> &UsableObject {
        &self.usable
    }
}

/// What a run of records of one kind is read by, one record each: a count
/// (each record alike) or the list's entries (one each).
trait Layouts: Copy {
    type Layout;
    fn next(&mut self) -> Option<Self::Layout>;
}

impl Layouts for usize {
    type Layout = ();
    fn next(&mut self) -> Option<()> {
        *self = self.checked_sub(1)?;
        Some(())
    }
}

impl<T: Copy> Layouts for &[T] {
    type Layout = T;
    fn next(&mut self) -> Option<T> {
        let (first, rest) = self.split_first()?;
        *self = rest;
        Some(*first)
    }
}

/// A run of records, each read and checked once by its reader and read
/// again, by the same reader, as it is handed out (so no list is kept).
#[derive(Clone, Copy, Debug)]
struct Run<'a, L> {
    reader: Reader<'a>,
    layouts: L,
}

type ReadOne<'a, T, L> = fn(&mut Reader<'a>, <L as Layouts>::Layout) -> Result<T, ObjectFault>;

impl<'a, L: Layouts> Run<'a, L> {
    fn read<T>(
        reader: &mut Reader<'a>,
        layouts: L,
        read: ReadOne<'a, T, L>,
    ) -> Result<Self, ObjectFault> {
        let start = *reader;
        let mut each = layouts;
        while let Some(layout) = each.next() {
            read(reader, layout)?;
        }
        Ok(Self {
            reader: start,
            layouts,
        })
    }

    /// The next record: the reader read it before, so it reads again.
    fn next<T>(&mut self, read: ReadOne<'a, T, L>) -> Option<T> {
        let layout = self.layouts.next()?;
        read(&mut self.reader, layout).ok()
    }
}

/// Damage zones that are on, in order.
#[derive(Clone, Copy, Debug)]
pub struct DamageZoneCores<'a>(Run<'a, usize>);

impl Iterator for DamageZoneCores<'_> {
    type Item = DamageZoneCore;
    fn next(&mut self) -> Option<DamageZoneCore> {
        self.0.next(|reader, ()| DamageZoneCore::read(reader))
    }
}

/// A switched group's `current_byte_mask` bytes: bit z of byte z/8 is zone
/// z, and no bit past the group's zones is set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZoneMask<'a> {
    bytes: &'a [u8],
    zones: usize,
}

impl<'a> ZoneMask<'a> {
    fn read(reader: &mut Reader<'a>, zones: usize) -> Result<Self, ObjectFault> {
        let at = reader.at();
        let bytes = reader.take(zones.div_ceil(8))?;
        for ((index, &value), at) in bytes.iter().enumerate().zip(at..) {
            let held = zones.saturating_sub(index.saturating_mul(8)).min(8);
            let mask = 1u16
                .checked_shl(held as u32)
                .map_or(0xFF, |bit| bit.wrapping_sub(1));
            if u16::from(value) & !mask != 0 {
                return Err(ObjectFault::Value {
                    at,
                    field: "current_byte_mask",
                    value: value.into(),
                });
            }
        }
        Ok(Self { bytes, zones })
    }

    /// The group's zone count.
    pub fn zones(&self) -> usize {
        self.zones
    }

    /// Whether zone `zone` is on.
    pub fn is_on(&self, zone: usize) -> bool {
        self.bytes
            .get(zone / 8)
            .is_some_and(|&bits| bits & (1 << (zone % 8)) != 0)
    }

    /// How many zones are on.
    pub fn on(&self) -> usize {
        self.bytes
            .iter()
            .map(|bits| bits.count_ones() as usize)
            .fold(0, usize::saturating_add)
    }
}

/// A `zone_group` of the anomaly's current state.
#[derive(Clone, Copy, Debug)]
pub enum ZoneGroup<'a> {
    /// `keep_all_zones_active`: every zone, each on.
    KeepAll { zones: DamageZoneCores<'a> },
    Switched {
        next_recharge_time: u32,
        mask: ZoneMask<'a>,
        /// The zones that are on, in zone order.
        zones: DamageZoneCores<'a>,
    },
}

impl<'a> ZoneGroup<'a> {
    fn read(reader: &mut Reader<'a>, group: Group) -> Result<Self, ObjectFault> {
        let zones = |reader: &mut Reader<'a>, on| {
            Run::read(reader, on, |reader, ()| DamageZoneCore::read(reader)).map(DamageZoneCores)
        };
        Ok(match group {
            Group::KeepAll { zones: count } => ZoneGroup::KeepAll {
                zones: zones(reader, count)?,
            },
            Group::Switched { zones: count } => {
                let next_recharge_time = reader.u32()?;
                let mask = ZoneMask::read(reader, count)?;
                ZoneGroup::Switched {
                    next_recharge_time,
                    mask,
                    zones: zones(reader, mask.on())?,
                }
            }
        })
    }

    /// The zones that are on, in zone order.
    pub fn zones(&self) -> DamageZoneCores<'a> {
        match *self {
            ZoneGroup::KeepAll { zones } | ZoneGroup::Switched { zones, .. } => zones,
        }
    }
}

/// The groups of the anomaly's current state, in its order.
#[derive(Clone, Copy, Debug)]
pub struct ZoneGroups<'a>(Run<'a, &'a [Group]>);

impl<'a> Iterator for ZoneGroups<'a> {
    type Item = ZoneGroup<'a>;
    fn next(&mut self) -> Option<ZoneGroup<'a>> {
        self.0.next(ZoneGroup::read)
    }
}

/// An anomaly's current `anomaly_state`.
#[derive(Clone, Copy, Debug)]
pub struct AnomalyState<'a> {
    index: u8,
    finish_time_ms: u32,
    groups: ZoneGroups<'a>,
}

impl<'a> AnomalyState<'a> {
    /// Its index in the anomaly's states (within them).
    pub fn index(&self) -> u8 {
        self.index
    }

    /// `m_finish_time_ms`.
    pub fn finish_time_ms(&self) -> u32 {
        self.finish_time_ms
    }

    pub fn groups(&self) -> ZoneGroups<'a> {
        self.groups
    }
}

/// The artefact id a container holds: an index into the world's
/// artefacts, which the match bounds ([`objects`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtefactId {
    at: usize,
    id: u8,
}

impl ArtefactId {
    pub fn at(self) -> usize {
        self.at
    }

    pub fn get(self) -> u8 {
        self.id
    }
}

/// An anomaly's `artefact_container_core`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtefactContainerCore {
    usable: UsableObject,
    artefact: Option<ArtefactId>,
    next_artefact_spawn_time_ms: u32,
}

impl ArtefactContainerCore {
    fn read(reader: &mut Reader) -> Result<Self, ObjectFault> {
        let usable = UsableObject::read(reader)?;
        let at = reader.at();
        let artefact = match reader.u8()? {
            0xFF => None,
            id => Some(ArtefactId { at, id }),
        };
        Ok(Self {
            usable,
            artefact,
            next_artefact_spawn_time_ms: reader.u32()?,
        })
    }

    pub fn usable(&self) -> &UsableObject {
        &self.usable
    }

    /// `m_artefact`'s id, if it holds one.
    pub fn artefact(&self) -> Option<ArtefactId> {
        self.artefact
    }

    /// `m_next_artefact_spawn_time_ms`.
    pub fn next_artefact_spawn_time_ms(&self) -> u32 {
        self.next_artefact_spawn_time_ms
    }
}

/// An anomaly's containers, in its order.
#[derive(Clone, Copy, Debug)]
pub struct ArtefactContainerCores<'a>(Run<'a, usize>);

impl Iterator for ArtefactContainerCores<'_> {
    type Item = ArtefactContainerCore;
    fn next(&mut self) -> Option<ArtefactContainerCore> {
        self.0
            .next(|reader, ()| ArtefactContainerCore::read(reader))
    }
}

/// The kit an artefact is in: a place's kit, and the slot it is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Owner {
    at: usize,
    place: Place,
    kit: Kit,
    slot: u8,
}

impl Owner {
    /// Where the owner's place is in the message.
    pub fn at(self) -> usize {
        self.at
    }

    /// `player_id`.
    pub fn place(self) -> Place {
        self.place
    }

    /// `player_profile_id`.
    pub fn kit(self) -> Kit {
        self.kit
    }

    /// `m_slot_id`, the profile slot it is in (left open).
    pub fn slot(self) -> u8 {
        self.slot
    }
}

/// What an active artefact's core saves of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveCore {
    Lifebone {
        time_left_to_deactivate: u32,
    },
    Spring {
        time_left_to_deactivate: u32,
    },
    Onyx {
        time_left_to_deactivate: u32,
        damage_to_absorb: Finite,
    },
}

impl ActiveCore {
    fn read(reader: &mut Reader, core: Activated) -> Result<Self, ObjectFault> {
        let time_left_to_deactivate = reader.u32()?;
        Ok(match core {
            Activated::Lifebone => ActiveCore::Lifebone {
                time_left_to_deactivate,
            },
            Activated::Spring => ActiveCore::Spring {
                time_left_to_deactivate,
            },
            Activated::Onyx => ActiveCore::Onyx {
                time_left_to_deactivate,
                damage_to_absorb: number(reader, "m_damage_to_absorb")?,
            },
        })
    }

    /// `m_time_left_to_deactivate`.
    pub fn time_left_to_deactivate(&self) -> u32 {
        match *self {
            ActiveCore::Lifebone {
                time_left_to_deactivate,
            }
            | ActiveCore::Spring {
                time_left_to_deactivate,
            }
            | ActiveCore::Onyx {
                time_left_to_deactivate,
                ..
            } => time_left_to_deactivate,
        }
    }
}

/// An artefact's `m_state`, with what each saves (the module doc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArtefactState {
    /// 0: neither in a container nor in a kit.
    Free { time_left_to_cool: u32 },
    /// 1: growing in a container.
    Spawning {
        container_id: u8,
        time_left_to_spawn: u32,
    },
    /// 2: grown, in its container.
    Ready { container_id: u8 },
    /// 3: in a kit.
    Taken { time_left_to_cool: u32 },
    /// 4: in a kit, its active effect on (never a Sponge).
    Active {
        time_left_to_cool: u32,
        core: ActiveCore,
    },
    /// 5: its holder's player down (LIKELY, the module doc).
    Disabled { time_left_to_cool: u32 },
    /// 6, a Sponge's only (exe patch 59): armed, its pin pulled. Patch 78's
    /// carry record (no release) was read with it.
    Armed {
        time_left_to_cool: u32,
        sponge: SpongeArmed,
    },
    /// 7, a Sponge's only: exploded.
    Exploded {
        time_left_to_cool: u32,
        sponge: SpongeExploded,
    },
}

/// An armed Sponge: in its holder's hand, or thrown (its body in the
/// physics world).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpongeArmed {
    InHand(sponge::machine::Armed),
    InAir(sponge::machine::Armed, sponge::record::Motion),
}

/// An exploded Sponge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpongeExploded {
    /// Its damage zone on: the zone's record and the explosion's transform
    /// follow its fields.
    Cloud {
        cloud: sponge::machine::Cloud,
        zone: DamageZoneCore,
        transform: [f32; 16],
    },
    /// Its cloud over while a throw state still holds it.
    Spent,
}

/// The `m_state` byte of an artefact of its kind, before what its state
/// saves.
#[derive(Clone, Copy)]
enum StateByte {
    Free,
    Spawning,
    Ready,
    Taken,
    Active(Activated),
    Disabled,
    Armed,
    Exploded,
}

impl StateByte {
    /// The state `byte` names for an artefact of `kind`: 6 and 7 are the
    /// Sponge's only, and it is never 4.
    fn of(byte: u8, kind: Artefact) -> Option<Self> {
        let sponge = kind == Artefact::Sponge;
        Some(match byte {
            0 => StateByte::Free,
            1 => StateByte::Spawning,
            2 => StateByte::Ready,
            3 => StateByte::Taken,
            4 => StateByte::Active(kind.activated()?),
            5 => StateByte::Disabled,
            sponge::record::STATE_ARMED if sponge => StateByte::Armed,
            sponge::record::STATE_EXPLODED if sponge => StateByte::Exploded,
            _ => return None,
        })
    }
}

/// Patch 78's carry record of an artefact in state 6 (`carry.ground_save`,
/// `Drop::snapshot`, 20 bytes): a Sponge's has no release, all 0xFF
/// (`NO_RELEASE`; its load restores none of it, `Drop::restore`).
const CARRY_NO_RELEASE: [u8; 20] = [0xFF; 20];

fn carry_no_release(reader: &mut Reader) -> Result<(), ObjectFault> {
    let at = reader.at();
    let record = reader.take(CARRY_NO_RELEASE.len())?;
    match record.iter().zip(at..).find(|&(&value, _)| value != 0xFF) {
        None => Ok(()),
        Some((&value, at)) => Err(ObjectFault::Value {
            at,
            field: "carry release",
            value: value.into(),
        }),
    }
}

/// A `sponge::record` fault at `at` in the message.
fn sponge_fault(at: usize, fault: sponge::record::Fault) -> ObjectFault {
    use sponge::record::Fault;
    match fault {
        Fault::Short { at: short } => ObjectFault::Short(Short {
            at: at.saturating_add(short),
            wanted: 1,
        }),
        Fault::Value {
            at: value_at,
            field,
            value,
        } => ObjectFault::Value {
            at: at.saturating_add(value_at),
            field,
            value,
        },
        Fault::NotNumber { at: number_at, field } => ObjectFault::NotNumber {
            at: at.saturating_add(number_at),
            field,
        },
    }
}

/// A Sponge's own fields after artefact_base's record of state `base`
/// (`sponge::record::parse`, the format's one definition).
fn sponge_own(reader: &mut Reader, base: u8) -> Result<sponge::record::Saved, ObjectFault> {
    let at = reader.at();
    let (saved, len) =
        sponge::record::parse(base, reader.rest()).map_err(|fault| sponge_fault(at, fault))?;
    reader.skip(len)?;
    Ok(saved)
}

/// The explosion's transform after a cloud's zone record.
fn cloud_transform(reader: &mut Reader) -> Result<[f32; 16], ObjectFault> {
    let at = reader.at();
    let (transform, len) = sponge::record::parse_cloud_transform(reader.rest())
        .map_err(|fault| sponge_fault(at, fault))?;
    reader.skip(len)?;
    Ok(transform)
}

/// `sponge::record::parse` answered a sponge of another state than the
/// base's `m_state` (it never does): the state byte is refused.
fn other_state(at: usize, base: u8) -> ObjectFault {
    ObjectFault::Value {
        at,
        field: "artefact m_state",
        value: base.into(),
    }
}

/// An anomaly's artefact: its `inventory_item`, its `artefact_base` and,
/// active, its core's own; a Sponge's own fields in its state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArtefactBase {
    amount: u16,
    owner: Option<Owner>,
    state: ArtefactState,
    last_activation_time_ms: u32,
}

impl ArtefactBase {
    fn read(reader: &mut Reader, kind: Artefact) -> Result<Self, ObjectFault> {
        let amount = reader.u16()?;
        let at = reader.at();
        let place = byte(reader, "player_id", |place| match place {
            0xFF => Some(None),
            place => Place::new(place).map(Some),
        })?;
        let owner = match place {
            None => None,
            Some(place) => {
                let kit = byte(reader, "player_profile_id", |kit| {
                    (usize::from(kit) < KITS).then_some(Kit(kit))
                })?;
                // The slot follows when the kit has a player: a world names
                // no other owner ([`objects`] holds it to one).
                let slot = reader.u8()?;
                Some(Owner {
                    at,
                    place,
                    kit,
                    slot,
                })
            }
        };
        // A state that is not in a container saves its cooldown; the
        // activation time follows every state, an active one's core after,
        // a Sponge's own fields after that.
        let cooling = |reader: &mut Reader| -> Result<(u32, u32), ObjectFault> {
            Ok((reader.u32()?, reader.u32()?))
        };
        let state_at = reader.at();
        let base = reader.u8()?;
        let state_byte = StateByte::of(base, kind).ok_or(ObjectFault::Value {
            at: state_at,
            field: "artefact m_state",
            value: base.into(),
        })?;
        let (state, last_activation_time_ms) = match state_byte {
            StateByte::Free => {
                let (time_left_to_cool, last) = cooling(reader)?;
                (ArtefactState::Free { time_left_to_cool }, last)
            }
            StateByte::Spawning => {
                let state = ArtefactState::Spawning {
                    container_id: reader.u8()?,
                    time_left_to_spawn: reader.u32()?,
                };
                (state, reader.u32()?)
            }
            StateByte::Ready => {
                let state = ArtefactState::Ready {
                    container_id: reader.u8()?,
                };
                (state, reader.u32()?)
            }
            StateByte::Taken => {
                let (time_left_to_cool, last) = cooling(reader)?;
                (ArtefactState::Taken { time_left_to_cool }, last)
            }
            StateByte::Active(core) => {
                let (time_left_to_cool, last) = cooling(reader)?;
                let core = ActiveCore::read(reader, core)?;
                let state = ArtefactState::Active {
                    time_left_to_cool,
                    core,
                };
                (state, last)
            }
            StateByte::Disabled => {
                let (time_left_to_cool, last) = cooling(reader)?;
                (ArtefactState::Disabled { time_left_to_cool }, last)
            }
            StateByte::Armed => {
                let time_left_to_cool = reader.u32()?;
                carry_no_release(reader)?;
                let last = reader.u32()?;
                use sponge::record::Saved;
                let sponge = match sponge_own(reader, base)? {
                    Saved::InHand(armed) => SpongeArmed::InHand(armed),
                    Saved::InAir(armed, motion) => SpongeArmed::InAir(armed, motion),
                    Saved::Held { .. } | Saved::Cloud(_) | Saved::Spent => {
                        return Err(other_state(state_at, base));
                    }
                };
                let state = ArtefactState::Armed {
                    time_left_to_cool,
                    sponge,
                };
                (state, last)
            }
            StateByte::Exploded => {
                let (time_left_to_cool, last) = cooling(reader)?;
                use sponge::record::Saved;
                let sponge = match sponge_own(reader, base)? {
                    Saved::Cloud(cloud) => SpongeExploded::Cloud {
                        cloud,
                        zone: DamageZoneCore::read(reader)?,
                        transform: cloud_transform(reader)?,
                    },
                    Saved::Spent => SpongeExploded::Spent,
                    Saved::Held { .. } | Saved::InHand(_) | Saved::InAir(..) => {
                        return Err(other_state(state_at, base));
                    }
                };
                let state = ArtefactState::Exploded {
                    time_left_to_cool,
                    sponge,
                };
                (state, last)
            }
        };
        // Held (0..5): a Sponge's own fields say it is unarmed.
        let held = !matches!(state_byte, StateByte::Armed | StateByte::Exploded);
        if held && kind == Artefact::Sponge {
            match sponge_own(reader, base)? {
                sponge::record::Saved::Held { .. } => {}
                _ => return Err(other_state(state_at, base)),
            }
        }
        Ok(Self {
            amount,
            owner,
            state,
            last_activation_time_ms,
        })
    }

    /// `m_amount`.
    pub fn amount(&self) -> u16 {
        self.amount
    }

    /// The kit it is in.
    pub fn owner(&self) -> Option<Owner> {
        self.owner
    }

    pub fn state(&self) -> ArtefactState {
        self.state
    }

    /// `m_last_activation_time_ms`.
    pub fn last_activation_time_ms(&self) -> u32 {
        self.last_activation_time_ms
    }
}

/// An anomaly's artefacts, in the world's order.
#[derive(Clone, Copy, Debug)]
pub struct ArtefactBases<'a>(Run<'a, &'a [Artefact]>);

impl Iterator for ArtefactBases<'_> {
    type Item = ArtefactBase;
    fn next(&mut self) -> Option<ArtefactBase> {
        self.0.next(ArtefactBase::read)
    }
}

/// What an anomaly with `artefacts_enabled` saves of its artefacts.
#[derive(Clone, Copy, Debug)]
pub struct AnomalyArtefacts<'a> {
    next_spawn_time_ms: Option<u32>,
    containers: ArtefactContainerCores<'a>,
    artefacts: ArtefactBases<'a>,
}

impl<'a> AnomalyArtefacts<'a> {
    /// `m_next_artefact_spawn_time_ms`, saved when
    /// `m_need_to_spawn_artefacts` is set.
    pub fn next_spawn_time_ms(&self) -> Option<u32> {
        self.next_spawn_time_ms
    }

    pub fn containers(&self) -> ArtefactContainerCores<'a> {
        self.containers
    }

    pub fn artefacts(&self) -> ArtefactBases<'a> {
        self.artefacts
    }
}

/// A map's `generic_anomaly_core`.
#[derive(Clone, Copy, Debug)]
pub struct GenericAnomalyCore<'a> {
    artefacts: Option<AnomalyArtefacts<'a>>,
    energy_current: Finite,
    state: Option<AnomalyState<'a>>,
    zone_trigger_event: bool,
    shoot_trigger_event: bool,
    random_seed: u32,
}

impl<'a> GenericAnomalyCore<'a> {
    const ZONE_TRIGGER: u8 = 1;
    const SHOOT_TRIGGER: u8 = 2;

    fn read(reader: &mut Reader<'a>, anomaly: &'a Anomaly<'a>) -> Result<Self, ObjectFault> {
        let next_spawn_time_ms = match anomaly.artefacts {
            Some(_) if flag(reader, "m_need_to_spawn_artefacts")? => Some(Some(reader.u32()?)),
            Some(_) => Some(None),
            None => None,
        };
        let energy_current = number(reader, "m_energy_current")?;
        let at = reader.at();
        let state = match reader.u8()? {
            0xFF => None,
            index => {
                let Some(state) = anomaly.states.get(usize::from(index)) else {
                    return Err(ObjectFault::Value {
                        at,
                        field: "current_state_index",
                        value: index.into(),
                    });
                };
                let finish_time_ms = reader.u32()?;
                let groups = Run::read(reader, state.groups, ZoneGroup::read)?;
                Some(AnomalyState {
                    index,
                    finish_time_ms,
                    groups: ZoneGroups(groups),
                })
            }
        };
        let events = bits(
            reader,
            "events_mask",
            Self::ZONE_TRIGGER | Self::SHOOT_TRIGGER,
        )?;
        let random_seed = reader.u32()?;
        let artefacts = match (anomaly.artefacts, next_spawn_time_ms) {
            (Some(artefacts), Some(next_spawn_time_ms)) => {
                let containers = Run::read(reader, artefacts.containers, |reader, ()| {
                    ArtefactContainerCore::read(reader)
                })?;
                let own = Run::read(reader, artefacts.artefacts, ArtefactBase::read)?;
                Some(AnomalyArtefacts {
                    next_spawn_time_ms,
                    containers: ArtefactContainerCores(containers),
                    artefacts: ArtefactBases(own),
                })
            }
            _ => None,
        };
        Ok(Self {
            artefacts,
            energy_current,
            state,
            zone_trigger_event: events & Self::ZONE_TRIGGER != 0,
            shoot_trigger_event: events & Self::SHOOT_TRIGGER != 0,
            random_seed,
        })
    }

    /// With `artefacts_enabled`: its spawn time, containers and artefacts.
    pub fn artefacts(&self) -> Option<AnomalyArtefacts<'a>> {
        self.artefacts
    }

    /// `m_energy_current`.
    pub fn energy_current(&self) -> Finite {
        self.energy_current
    }

    /// Its current state (`m_current_state`), if it has one.
    pub fn state(&self) -> Option<AnomalyState<'a>> {
        self.state
    }

    /// `m_was_zone_trigger_event` (bit 0 of `events_mask`).
    pub fn zone_trigger_event(&self) -> bool {
        self.zone_trigger_event
    }

    /// `m_was_shoot_trigger_event` (bit 1).
    pub fn shoot_trigger_event(&self) -> bool {
        self.shoot_trigger_event
    }

    /// `m_random.seed()`.
    pub fn random_seed(&self) -> u32 {
        self.random_seed
    }
}

/// One object of the section.
#[derive(Clone, Copy, Debug)]
pub enum GameObject<'a> {
    Grenade(GrenadeCore),
    BoobyTrap(BoobyTrapCore),
    DamageZone(DamageZoneCore),
    ResupplyZone(ResupplyZoneCore),
    Anomaly(GenericAnomalyCore<'a>),
}

/// What the list says an object of the section is.
#[derive(Clone, Copy)]
enum Kind<'a> {
    Held(Held),
    DamageZone,
    ResupplyZone,
    Anomaly(&'a Anomaly<'a>),
}

impl<'a> Objects<'a> {
    /// The list's object `index`, in the section's order.
    fn kind(&self, index: usize) -> Option<Kind<'a>> {
        if let Some(&held) = self.held.get(index) {
            return Some(Kind::Held(held));
        }
        let index = index.checked_sub(self.held.len())?;
        let Some(index) = index.checked_sub(self.map.damage_zones) else {
            return Some(Kind::DamageZone);
        };
        let Some(index) = index.checked_sub(self.map.resupply_zones) else {
            return Some(Kind::ResupplyZone);
        };
        self.map.anomalies.get(index).map(Kind::Anomaly)
    }
}

impl<'a> GameObject<'a> {
    fn read(reader: &mut Reader<'a>, kind: Kind<'a>) -> Result<Self, ObjectFault> {
        Ok(match kind {
            Kind::Held(Held::Grenade) => GameObject::Grenade(GrenadeCore::read(reader)?),
            Kind::Held(Held::Trap) => GameObject::BoobyTrap(BoobyTrapCore::read(reader)?),
            Kind::DamageZone => GameObject::DamageZone(DamageZoneCore::read(reader)?),
            Kind::ResupplyZone => GameObject::ResupplyZone(ResupplyZoneCore {
                usable: UsableObject::read(reader)?,
            }),
            Kind::Anomaly(anomaly) => {
                GameObject::Anomaly(GenericAnomalyCore::read(reader, anomaly)?)
            }
        })
    }
}

/// The objects of the list, read in its order.
#[derive(Clone, Copy, Debug)]
pub struct GameObjectIter<'a> {
    reader: Reader<'a>,
    list: &'a Objects<'a>,
    next: usize,
}

impl<'a> GameObjectIter<'a> {
    fn read(&mut self) -> Option<Result<GameObject<'a>, ObjectFault>> {
        let kind = self.list.kind(self.next)?;
        self.next = self.next.saturating_add(1);
        Some(GameObject::read(&mut self.reader, kind))
    }
}

impl<'a> Iterator for GameObjectIter<'a> {
    type Item = GameObject<'a>;
    fn next(&mut self) -> Option<GameObject<'a>> {
        self.read()?.ok()
    }
}

/// The game-objects section, borrowed from the world: every object of the
/// list read to the section's exact end, its shape checked.
#[derive(Clone, Copy, Debug)]
pub struct GameObjects<'a> {
    start: GameObjectIter<'a>,
}

impl<'a> GameObjects<'a> {
    /// The section, read as `list`'s objects to its end, or why not.
    pub fn read(section: &Section<'a>, list: &'a Objects<'a>) -> Result<Self, Refusal> {
        let start = GameObjectIter {
            reader: Reader::new(section.bytes, section.at),
            list,
            next: 0,
        };
        let mut walk = start;
        let unread = |at, objects| {
            Refusal::new(
                Part::Objects,
                at,
                Reason::ObjectsUnread {
                    len: section.bytes.len(),
                    objects,
                },
            )
        };
        let mut object = 0usize;
        while let Some(read) = walk.read() {
            match read {
                Ok(_) => object = object.saturating_add(1),
                Err(ObjectFault::Short(short)) => return Err(unread(short.at, object)),
                Err(fault) => {
                    return Err(Refusal::new(
                        Part::Objects,
                        fault.at(),
                        Reason::ObjectsField { object, fault },
                    ));
                }
            }
        }
        if !walk.reader.is_done() {
            return Err(unread(walk.reader.at(), object));
        }
        Ok(Self { start })
    }

    /// Its objects, in the list's order.
    pub fn objects(&self) -> GameObjectIter<'a> {
        self.start
    }
}

/// The section reads as the list's objects to its exact end, and what they
/// name is of the match: a user is a place of the roster that uses no
/// other object, a container's artefact one of the map's, an artefact's
/// owner a kit with a player. The list unknown: not checked.
pub fn objects(section: &Section, context: &Context) -> Result<(), Refusal> {
    let Some(list) = context.objects else {
        return Ok(());
    };
    let read = GameObjects::read(section, list)?;
    let mut rules = Rules {
        places: context.places.min(MAX_PLAYERS),
        owners: context.profiles,
        artefacts: list.map.artefacts(),
        using: 0,
    };
    for (object, read) in read.objects().enumerate() {
        rules
            .object(object, &read)
            .map_err(|(at, reason)| Refusal::new(Part::Objects, at, reason))?;
    }
    Ok(())
}

/// The match's rules over the objects' indexes, in the section's order.
struct Rules<'c> {
    places: usize,
    /// By place, whose kits have a player (an artefact's owner).
    owners: Option<&'c [Profile]>,
    artefacts: usize,
    /// Bit p: place p uses an object already.
    using: u32,
}

impl Rules<'_> {
    fn object(&mut self, object: usize, read: &GameObject) -> Result<(), (usize, Reason)> {
        match read {
            GameObject::BoobyTrap(BoobyTrapCore::Armed { usable, .. }) => {
                self.users(object, usable)
            }
            GameObject::ResupplyZone(zone) => self.users(object, zone.usable()),
            GameObject::Anomaly(anomaly) => {
                let Some(artefacts) = anomaly.artefacts() else {
                    return Ok(());
                };
                for container in artefacts.containers() {
                    self.users(object, container.usable())?;
                    if let Some(artefact) = container.artefact()
                        && usize::from(artefact.get()) >= self.artefacts
                    {
                        let reason = Reason::ObjectsArtefactPast {
                            object,
                            artefact: artefact.get(),
                            artefacts: self.artefacts,
                        };
                        return Err((artefact.at(), reason));
                    }
                }
                for artefact in artefacts.artefacts() {
                    if let Some(owner) = artefact.owner()
                        && !self.has_player(owner)
                    {
                        let reason = Reason::ObjectsOwner {
                            object,
                            place: owner.place().get(),
                            kit: owner.kit().get(),
                        };
                        return Err((owner.at(), reason));
                    }
                }
                Ok(())
            }
            GameObject::Grenade(_) | GameObject::BoobyTrap(_) | GameObject::DamageZone(_) => Ok(()),
        }
    }

    /// Each user a place of the roster that uses no other object (a
    /// player has one link node).
    fn users(&mut self, object: usize, usable: &UsableObject) -> Result<(), (usize, Reason)> {
        for user in usable.users() {
            let place = user.place().get();
            if user.place().index() >= self.places {
                let reason = Reason::ObjectsUserPast {
                    object,
                    user: place,
                    places: self.places,
                };
                return Err((user.at(), reason));
            }
            let bit = 1u32.checked_shl(u32::from(place)).unwrap_or(0);
            if self.using & bit != 0 {
                return Err((
                    user.at(),
                    Reason::ObjectsUserTwice {
                        object,
                        user: place,
                    },
                ));
            }
            self.using |= bit;
        }
        Ok(())
    }

    /// Whether the owner's kit has a player: the owner lookup finds one
    /// and the slot byte follows. A world names no other owner. The
    /// profiles unknown: any kit of a roster place.
    fn has_player(&self, owner: Owner) -> bool {
        owner.place().index() < self.places
            && self.owners.is_none_or(|profiles| {
                profiles
                    .get(owner.place().index())
                    .is_some_and(|profile| profile.usable(owner.kit().get()))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn context<'a>(objects: Option<&'a Objects<'a>>) -> Context<'a> {
        Context {
            time: 0,
            places: 4,
            game_mode: crate::mode::BATTERIES,
            batteries: 3,
            respawn_points: None,
            profiles: None,
            objects,
            artefacts: None,
            inventory: None,
        }
    }

    fn section(bytes: &[u8]) -> Section<'_> {
        Section { at: 0x100, bytes }
    }

    fn reason(objects: &Objects, bytes: &[u8]) -> Option<Reason> {
        super::objects(&section(bytes), &context(Some(objects)))
            .err()
            .map(|refusal| refusal.reason)
    }

    /// The field a shape fault names.
    fn field(objects: &Objects, bytes: &[u8]) -> Option<&'static str> {
        match reason(objects, bytes) {
            Some(Reason::ObjectsField {
                fault: ObjectFault::Value { field, .. } | ObjectFault::NotNumber { field, .. },
                ..
            }) => Some(field),
            _ => None,
        }
    }

    const GROUPS: [Group; 2] = [Group::Switched { zones: 10 }, Group::KeepAll { zones: 1 }];
    const STATES: [State; 2] = [State { groups: &GROUPS }, State { groups: &[] }];
    const ANOMALIES: [Anomaly; 1] = [Anomaly {
        artefacts: Some(Artefacts {
            containers: 2,
            artefacts: &[Artefact::Onyx, Artefact::Spring],
        }),
        states: &STATES,
    }];
    const MAP: Map = Map {
        damage_zones: 1,
        resupply_zones: 1,
        anomalies: &ANOMALIES,
    };
    const LIST: Objects = Objects {
        held: &[Held::Grenade, Held::Trap],
        map: &MAP,
    };
    /// An artefact owned by place 2's kit 0, in slot 4, spawning (state 1).
    const OWNED: [u8; 6] = [1, 0, 2, 0, 4, 1];
    /// A container used by place 1, holding artefact 1.
    const USED: [u8; 7] = [1, 1, 1, 0, 0, 0, 0];

    /// A section that reads to its end for [`LIST`].
    fn good() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0); // an untouched grenade
        bytes.push(0); // a removed trap
        bytes.extend([1, 2, 3, 4, 0xFF]); // the damage zone, hitting
        bytes.extend([2, 0, 3]); // the resupply zone: places 0 and 3
        // The anomaly: to spawn with its time, energy, state 0 with its
        // time; group 0: a u32 and zones 0, 2 and 9 on, their 15 bytes;
        // group 1: its one zone's 5; events, a seed.
        bytes.extend([0xFF, 9, 9, 9, 9]);
        bytes.extend(1.0f32.to_le_bytes());
        bytes.extend([0, 7, 7, 7, 7]);
        bytes.extend([0, 0, 0, 0, 0b101, 0b10]);
        bytes.extend([0; 15]);
        bytes.extend([0; 5]);
        bytes.extend([3, 0, 0, 0, 0]);
        // Its containers: one used, one empty.
        bytes.extend(USED);
        bytes.extend([0, 0xFF, 0, 0, 0, 0]);
        // Its artefacts: one owned, one free (state 0).
        bytes.extend(OWNED);
        bytes.extend([0; 9]);
        bytes.extend([1, 0, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        bytes
    }

    fn find(bytes: &[u8], what: &[u8]) -> usize {
        bytes
            .windows(what.len())
            .position(|window| window == what)
            .expect("in the section")
    }

    #[test]
    fn an_unknown_list_is_not_checked() {
        assert_eq!(objects(&section(&[1, 2, 3]), &context(None)), Ok(()));
    }

    #[test]
    fn a_section_that_reads_to_its_end_passes() {
        assert_eq!(reason(&LIST, &good()), None);
    }

    #[test]
    fn every_field_reads_by_name() {
        let bytes = good();
        let read = GameObjects::read(&section(&bytes), &LIST).unwrap();
        let objects: Vec<GameObject> = read.objects().collect();
        assert_eq!(objects.len(), 5);
        let GameObject::Grenade(grenade) = objects[0] else {
            panic!("a grenade")
        };
        assert_eq!((grenade.thrown(), grenade.motion()), (None, None));
        assert!(!grenade.flag_144());
        let GameObject::BoobyTrap(trap) = objects[1] else {
            panic!("a trap")
        };
        assert_eq!((trap.state(), trap.placed()), (TrapState::Removed, None));
        let GameObject::DamageZone(zone) = objects[2] else {
            panic!("a damage zone")
        };
        assert_eq!(zone.last_hit_time_ms(), 0x0403_0201);
        assert!(zone.hitting());
        let GameObject::ResupplyZone(resupply) = objects[3] else {
            panic!("a resupply zone")
        };
        let users: Vec<(usize, u8)> = resupply
            .usable()
            .users()
            .map(|user| (user.at(), user.place().get()))
            .collect();
        assert_eq!(users, [(0x108, 0), (0x109, 3)]);
        let GameObject::Anomaly(anomaly) = objects[4] else {
            panic!("an anomaly")
        };
        assert_eq!(anomaly.energy_current().get(), 1.0);
        assert!(anomaly.zone_trigger_event() && anomaly.shoot_trigger_event());
        assert_eq!(anomaly.random_seed(), 0);
        let state = anomaly.state().unwrap();
        assert_eq!((state.index(), state.finish_time_ms()), (0, 0x0707_0707));
        let groups: Vec<ZoneGroup> = state.groups().collect();
        let ZoneGroup::Switched {
            next_recharge_time,
            mask,
            zones,
        } = groups[0]
        else {
            panic!("a switched group")
        };
        assert_eq!(next_recharge_time, 0);
        assert_eq!((mask.zones(), mask.on()), (10, 3));
        assert!(mask.is_on(0) && mask.is_on(2) && mask.is_on(9) && !mask.is_on(1));
        assert_eq!(zones.count(), 3);
        assert_eq!(groups[1].zones().count(), 1);
        let artefacts = anomaly.artefacts().unwrap();
        assert_eq!(artefacts.next_spawn_time_ms(), Some(0x0909_0909));
        let containers: Vec<ArtefactContainerCore> = artefacts.containers().collect();
        assert_eq!(containers[0].artefact().map(ArtefactId::get), Some(1));
        assert_eq!(containers[1].artefact(), None);
        assert_eq!(containers[1].usable().users().count(), 0);
        let own: Vec<ArtefactBase> = artefacts.artefacts().collect();
        let owner = own[0].owner().unwrap();
        assert_eq!(
            (
                own[0].amount(),
                owner.place().get(),
                owner.kit().get(),
                owner.slot()
            ),
            (1, 2, 0, 4)
        );
        assert_eq!(
            own[0].state(),
            ArtefactState::Spawning {
                container_id: 0,
                time_left_to_spawn: 0
            }
        );
        assert_eq!(own[1].owner(), None);
        assert_eq!(
            own[1].state(),
            ArtefactState::Free {
                time_left_to_cool: 0
            }
        );
    }

    #[test]
    fn a_short_or_long_section_is_refused() {
        let mut bytes = good();
        bytes.pop();
        assert!(matches!(
            reason(&LIST, &bytes),
            Some(Reason::ObjectsUnread { .. })
        ));
        let mut bytes = good();
        bytes.push(0);
        assert!(matches!(
            reason(&LIST, &bytes),
            Some(Reason::ObjectsUnread { .. })
        ));
    }

    #[test]
    fn a_thrown_grenade_and_an_armed_trap_read_their_bytes() {
        let empty = Map {
            damage_zones: 0,
            resupply_zones: 0,
            anomalies: &[],
        };
        let list = Objects {
            held: &[Held::Grenade, Held::Trap],
            map: &empty,
        };
        let mut bytes = std::vec![GrenadeCore::IN_PHYSICS | GrenadeCore::THROWN];
        bytes.extend(7u32.to_le_bytes());
        bytes.extend([0; 4]);
        bytes.extend([0; 22 * 4]);
        bytes.extend([1, 0, 0, 0, 0]);
        bytes.extend([0; 16 * 4]);
        bytes.extend([0; 8]);
        bytes.extend([1, 2]);
        assert_eq!(reason(&list, &bytes), None);
        let read = GameObjects::read(&section(&bytes), &list).unwrap();
        let objects: Vec<GameObject> = read.objects().collect();
        let GameObject::Grenade(grenade) = objects[0] else {
            panic!("a grenade")
        };
        assert_eq!(
            grenade.thrown().map(|thrown| thrown.explode_time_ms()),
            Some(7)
        );
        assert!(grenade.motion().is_some());
        let GameObject::BoobyTrap(BoobyTrapCore::Armed { usable, .. }) = objects[1] else {
            panic!("an armed trap")
        };
        assert_eq!(usable.users().count(), 1);
        // A NaN in the grenade's transform; an unknown flag; a trap state
        // the game has not.
        let mut nan = bytes.clone();
        nan[9..13].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(field(&list, &nan), Some("m_transform"));
        let mut flag = bytes.clone();
        flag[0] |= 8;
        assert_eq!(field(&list, &flag), Some("grenade current_state"));
        let mut trap = bytes.clone();
        trap[1 + 8 + 88] = 4;
        assert_eq!(field(&list, &trap), Some("m_trap_state"));
    }

    #[test]
    fn an_active_artefact_reads_what_its_core_saves() {
        // The anomaly's artefacts active (state 4): the onyx's u32 and f32,
        // the spring's u32, after the base's two u32.
        let active = |own: usize| {
            let mut bytes = std::vec![1, 0, 0xFF, 4];
            bytes.extend([0; 4 + 4]);
            bytes.extend(std::iter::repeat_n(0, own));
            bytes
        };
        let mut bytes = good();
        let artefacts = bytes.len() - OWNED.len() - 9 - 12;
        bytes.truncate(artefacts);
        bytes.extend(active(8));
        bytes.extend(active(4));
        assert_eq!(reason(&LIST, &bytes), None);
        let read = GameObjects::read(&section(&bytes), &LIST).unwrap();
        let Some(GameObject::Anomaly(anomaly)) = read.objects().last() else {
            panic!("an anomaly")
        };
        let cores: Vec<ArtefactState> = anomaly
            .artefacts()
            .unwrap()
            .artefacts()
            .map(|artefact| artefact.state())
            .collect();
        assert!(matches!(
            cores[0],
            ArtefactState::Active {
                core: ActiveCore::Onyx { .. },
                ..
            }
        ));
        assert!(matches!(
            cores[1],
            ArtefactState::Active {
                core: ActiveCore::Spring { .. },
                ..
            }
        ));
        // An onyx read as a spring's.
        bytes.truncate(artefacts);
        bytes.extend(active(4));
        bytes.extend(active(4));
        assert!(matches!(
            reason(&LIST, &bytes),
            Some(Reason::ObjectsUnread { .. })
        ));
    }

    #[test]
    fn a_byte_outside_its_fields_values_is_refused() {
        let good = good();
        let mutate = |offset: usize, byte: u8| {
            let mut bytes = good.clone();
            bytes[offset] = byte;
            bytes
        };
        let state = 2 + 5 + 3 + 5 + 4;
        let artefact = find(&good, &OWNED);
        for (offset, byte, name) in [
            (6, 1, "m_hitting"),
            (7, 21, "m_usable_object_users.size()"),
            (8, 20, "owner.id"),
            (10, 1, "m_need_to_spawn_artefacts"),
            (state, 2, "current_state_index"),
            (state + 5 + 4 + 1, 0b110, "current_byte_mask"),
            (state + 5 + 4 + 2 + 15 + 5, 4, "events_mask"),
            (artefact + 2, 20, "player_id"),
            (artefact + 3, 3, "player_profile_id"),
            (artefact + 5, 6, "artefact m_state"),
        ] {
            assert_eq!(field(&LIST, &mutate(offset, byte)), Some(name), "{name}");
        }
        let mut nan = good.clone();
        nan[15..19].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(field(&LIST, &nan), Some("m_energy_current"));
    }

    #[test]
    fn an_index_past_the_match_is_refused() {
        let good = good();
        let mutate = |offset: usize, byte: u8| {
            let mut bytes = good.clone();
            bytes[offset] = byte;
            reason(&LIST, &bytes)
        };
        // The resupply zone's user past the roster of 4.
        assert!(matches!(
            mutate(8, 4),
            Some(Reason::ObjectsUserPast { user: 4, .. })
        ));
        // A container's user who uses the resupply zone too; its artefact
        // past the map's two.
        let container = find(&good, &USED);
        assert!(matches!(
            mutate(container + 1, 3),
            Some(Reason::ObjectsUserTwice { user: 3, .. })
        ));
        assert!(matches!(
            mutate(container + 2, 2),
            Some(Reason::ObjectsArtefactPast { artefact: 2, .. })
        ));
        // An artefact owned by a place past the roster.
        let artefact = find(&good, &OWNED);
        assert!(matches!(
            mutate(artefact + 2, 5),
            Some(Reason::ObjectsOwner { place: 5, .. })
        ));
    }

    #[test]
    fn an_owner_whose_kit_has_no_player_is_refused() {
        let profiles = [Profile {
            usable_kits: 0b001,
            ..Default::default()
        }; 4];
        let mut context = context(Some(&LIST));
        context.profiles = Some(&profiles);
        assert_eq!(objects(&section(&good()), &context), Ok(()));
        let mut bytes = good();
        let artefact = find(&bytes, &OWNED);
        bytes[artefact + 3] = 1;
        assert!(matches!(
            objects(&section(&bytes), &context).map_err(|refusal| refusal.reason),
            Err(Reason::ObjectsOwner {
                place: 2,
                kit: 1,
                ..
            })
        ));
    }

    // The Sponge (exe patch 59): an anomaly with an onyx and a Sponge.

    const SPONGE_ANOMALIES: [Anomaly; 1] = [Anomaly {
        artefacts: Some(Artefacts {
            containers: 1,
            artefacts: &[Artefact::Onyx, Artefact::Sponge],
        }),
        states: &[],
    }];
    const SPONGE_MAP: Map = Map {
        damage_zones: 0,
        resupply_zones: 0,
        anomalies: &SPONGE_ANOMALIES,
    };
    const SPONGE_LIST: Objects = Objects {
        held: &[],
        map: &SPONGE_MAP,
    };

    /// The anomaly up to its Sponge's record: nothing pending, no state,
    /// its container empty, its onyx free.
    fn before_sponge() -> Vec<u8> {
        let mut bytes = std::vec![0x00];
        bytes.extend(0f32.to_le_bytes());
        bytes.extend([0xFF, 0, 0, 0, 0, 0]);
        bytes.extend([0, 0xFF, 0, 0, 0, 0]);
        bytes.extend([1, 0, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        bytes
    }

    /// The Sponge's own fields as the patch writes them.
    fn own(saved: &sponge::record::Saved) -> Vec<u8> {
        use sponge::record::Field;
        let mut out = Vec::new();
        sponge::record::write(saved, |field| match field {
            Field::U32 { value, .. } => out.extend(value.to_le_bytes()),
            Field::U8 { value, .. } => out.push(value),
            Field::Matrix { value, .. } => value.iter().for_each(|v| out.extend(v.to_le_bytes())),
            Field::Float3 { value, .. } => value.iter().for_each(|v| out.extend(v.to_le_bytes())),
        });
        out
    }

    const CLOUD_ZONE: [u8; 5] = [1, 2, 3, 4, 0xFF];

    /// The section with the Sponge `saved`, its base `m_state` `base`, held
    /// by place 1's kit 0 in slot 13.
    fn with_sponge(saved: &sponge::record::Saved, base: u8) -> Vec<u8> {
        use sponge::record::Saved;
        let mut bytes = before_sponge();
        bytes.extend([1, 0, 1, 0, 13, base]);
        match base {
            1 => bytes.extend([0, 0, 0, 0, 0]),
            2 => bytes.push(0),
            _ => bytes.extend(9u32.to_le_bytes()),
        }
        if base == 6 {
            bytes.extend([0xFF; 20]);
        }
        bytes.extend(5u32.to_le_bytes());
        bytes.extend(own(saved));
        if let Saved::Cloud(_) = saved {
            bytes.extend(CLOUD_ZONE);
            (0..16).for_each(|n| bytes.extend((n as f32).to_le_bytes()));
        }
        bytes
    }

    fn sponge_state(bytes: &[u8]) -> ArtefactBase {
        let read = GameObjects::read(&section(bytes), &SPONGE_LIST).unwrap();
        let Some(GameObject::Anomaly(anomaly)) = read.objects().next() else {
            panic!("an anomaly")
        };
        anomaly.artefacts().unwrap().artefacts().nth(1).unwrap()
    }

    fn armed(contact: sponge::machine::Contact) -> sponge::machine::Armed {
        use sponge::machine::{Hold, Thrower};
        sponge::machine::Armed {
            explode_at: sponge::GameTime(103_000),
            thrower: Thrower::Place(sponge::Place::new(1).unwrap()),
            contact,
            hold: Hold::ThrowState,
        }
    }

    fn motion() -> sponge::record::Motion {
        sponge::record::Motion {
            transform: [0.5; 16],
            linear: [1.0, -2.0, 3.0],
            angular: [0.0; 3],
        }
    }

    fn cloud() -> sponge::machine::Cloud {
        use sponge::machine::{Hold, Thrower};
        sponge::machine::Cloud {
            exploded_at: sponge::GameTime(104_000),
            thrower: Thrower::Nobody,
            hold: Hold::Free,
        }
    }

    #[test]
    fn a_sponges_unarmed_record_is_the_patchs() {
        use sponge::record::Saved;
        assert_eq!(own(&Saved::Held { base: 0 }), SPONGE_UNARMED);
        assert_eq!(Artefact::Sponge.untouched_own(), SPONGE_UNARMED);
        assert_eq!(Artefact::Onyx.untouched_own(), [0u8; 0]);
    }

    #[test]
    fn a_sponge_reads_in_every_state_the_game_writes() {
        use sponge::machine::Contact;
        use sponge::record::Saved;
        // Held in any of artefact_base's states but active.
        for base in [0, 1, 2, 3, 5] {
            let bytes = with_sponge(&Saved::Held { base }, base);
            assert_eq!(reason(&SPONGE_LIST, &bytes), None, "{base}");
            let artefact = sponge_state(&bytes);
            assert!(!matches!(
                artefact.state(),
                ArtefactState::Armed { .. } | ArtefactState::Exploded { .. }
            ));
            assert_eq!(artefact.last_activation_time_ms(), 5);
        }
        // Armed in the hand: patch 78's 20 bytes before the activation time.
        let hand = armed(Contact::Waiting);
        let bytes = with_sponge(&Saved::InHand(hand), 6);
        let artefact = sponge_state(&bytes);
        assert_eq!(
            artefact.state(),
            ArtefactState::Armed {
                time_left_to_cool: 9,
                sponge: SpongeArmed::InHand(hand)
            }
        );
        assert_eq!(artefact.last_activation_time_ms(), 5);
        // In the air, with each first contact.
        for contact in [Contact::Waiting, Contact::Seen, Contact::Spent] {
            let bytes = with_sponge(&Saved::InAir(armed(contact), motion()), 6);
            assert_eq!(
                sponge_state(&bytes).state(),
                ArtefactState::Armed {
                    time_left_to_cool: 9,
                    sponge: SpongeArmed::InAir(armed(contact), motion())
                }
            );
        }
        // Its cloud on: the zone's record and the explosion's transform.
        let bytes = with_sponge(&Saved::Cloud(cloud()), 7);
        let ArtefactState::Exploded {
            time_left_to_cool: 9,
            sponge: SpongeExploded::Cloud {
                cloud: read,
                zone,
                transform,
            },
        } = sponge_state(&bytes).state()
        else {
            panic!("a cloud")
        };
        assert_eq!(read, cloud());
        assert_eq!((zone.last_hit_time_ms(), zone.hitting()), (0x0403_0201, true));
        assert_eq!(transform[15], 15.0);
        // Spent.
        let bytes = with_sponge(&Saved::Spent, 7);
        assert_eq!(
            sponge_state(&bytes).state(),
            ArtefactState::Exploded {
                time_left_to_cool: 9,
                sponge: SpongeExploded::Spent
            }
        );
    }

    #[test]
    fn a_sponge_the_game_never_writes_is_refused() {
        use sponge::machine::Contact;
        use sponge::record::Saved;
        let sponge_at = before_sponge().len();
        let state_at = sponge_at + 5;
        // Never active; 6 and 7 are no other artefact's.
        let mut active = with_sponge(&Saved::Held { base: 0 }, 0);
        active[state_at] = 4;
        assert_eq!(field(&SPONGE_LIST, &active), Some("artefact m_state"));
        let mut onyx = before_sponge();
        onyx.truncate(onyx.len() - 12);
        onyx.extend([1, 0, 0xFF, 6, 0, 0, 0, 0]);
        onyx.extend([0xFF; 20]);
        onyx.extend([0; 4]);
        onyx.extend(with_sponge(&Saved::Held { base: 0 }, 0).split_off(sponge_at));
        assert_eq!(field(&SPONGE_LIST, &onyx), Some("artefact m_state"));
        onyx[before_sponge().len() - 12 + 3] = 7;
        assert_eq!(field(&SPONGE_LIST, &onyx), Some("artefact m_state"));
        // An armed Sponge's carry record names no release.
        let mut carry = with_sponge(&Saved::InHand(armed(Contact::Waiting)), 6);
        carry[state_at + 1 + 4 + 7] = 0;
        assert!(matches!(
            reason(&SPONGE_LIST, &carry),
            Some(Reason::ObjectsField {
                fault: ObjectFault::Value {
                    field: "carry release",
                    value: 0,
                    at,
                },
                ..
            }) if at == 0x100 + state_at + 1 + 4 + 7
        ));
        // Its own fields, at their place in the message: a fuse while
        // unarmed, a pin not pulled while armed, a thrower past the 20, a
        // hold neither 0 nor 1.
        let own_at = |base: u8| state_at + 1 + 8 + if base == 6 { 20 } else { 0 };
        for (base, saved, offset, value, name) in [
            (0, Saved::Held { base: 0 }, 0, 0, "m_explode_time_ms"),
            (6, Saved::InHand(armed(Contact::Waiting)), 4, 0, "m_is_pin_pulled"),
            (6, Saved::InHand(armed(Contact::Waiting)), 7, 20, "m_zone_owner"),
            (7, Saved::Cloud(cloud()), 8, 2, "m_throw_state_holds"),
            (7, Saved::Spent, 8, 0, "m_throw_state_holds"),
        ] {
            let mut bytes = with_sponge(&saved, base);
            let at = own_at(base) + offset;
            bytes[at] = value;
            assert!(
                matches!(
                    reason(&SPONGE_LIST, &bytes),
                    Some(Reason::ObjectsField {
                        fault: ObjectFault::Value { field, at: fault_at, .. },
                        ..
                    }) if field == name && fault_at == 0x100 + at
                ),
                "{name}: {:?}",
                reason(&SPONGE_LIST, &bytes)
            );
        }
        // A thrown Sponge's motion and a cloud's transform are numbers; the
        // zone's m_hitting a bool.
        let mut air = with_sponge(&Saved::InAir(armed(Contact::Seen), motion()), 6);
        let motion_at = own_at(6) + 6;
        air[motion_at..motion_at + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(field(&SPONGE_LIST, &air), Some("m_transform"));
        let mut cloud_bytes = with_sponge(&Saved::Cloud(cloud()), 7);
        let zone_at = own_at(7) + 9;
        cloud_bytes[zone_at + 4] = 1;
        assert_eq!(field(&SPONGE_LIST, &cloud_bytes), Some("m_hitting"));
        let mut cloud_bytes = with_sponge(&Saved::Cloud(cloud()), 7);
        let end = cloud_bytes.len();
        cloud_bytes[end - 4..].copy_from_slice(&f32::INFINITY.to_le_bytes());
        assert_eq!(
            field(&SPONGE_LIST, &cloud_bytes),
            Some("m_damage_zone->get_collision_geometry(0)->transform()")
        );
        // Cut short anywhere: the section does not read to its end.
        for saved in [Saved::Held { base: 0 }, Saved::Spent] {
            let base = if saved == Saved::Spent { 7 } else { 0 };
            let mut bytes = with_sponge(&saved, base);
            bytes.pop();
            assert!(matches!(
                reason(&SPONGE_LIST, &bytes),
                Some(Reason::ObjectsUnread { .. })
            ));
        }
        let mut cloud_bytes = with_sponge(&Saved::Cloud(cloud()), 7);
        cloud_bytes.truncate(cloud_bytes.len() - 64);
        assert!(matches!(
            reason(&SPONGE_LIST, &cloud_bytes),
            Some(Reason::ObjectsUnread { .. })
        ));
    }
}

//! The game-objects section (world apply 0x2D2770: the walk 0x2D2BB0..
//! 0x2D2BE6 over the list at core+0x18914, `node->vtable[1]`).
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
//! What each object reads from this section (PROVEN; what an object reads
//! from the effects stream is that section's):
//!
//! - grenade (`grenade_core::deserialize` 0x303F80): a flags byte (1 in the
//!   physics world, 2 thrown, 4 spent), 8 bytes when thrown, 88 when in the
//!   physics world (a transform and two velocities: a grenade read out of
//!   step took a NaN transform and crashed the game, replay_sur.md);
//! - trap (0x3053C0): a state byte, and when it is not 0 a u32, a 64-byte
//!   transform, two u32, and when it is 1 the usable base;
//! - the usable base (`usable_object::deserialize` 0x2F84C0): a u8 N and N
//!   user places, into an inline buffer of 20 that a 21st overflows
//!   (0xF3090), each dereferenced as a place's player with no bound
//!   (0x2F8510); a player has one link node, so it uses one object at most;
//! - damage zone (0x319470): a u32 and a u8;
//! - anomaly (0x31B0A0): with `artefacts_enabled`, a pending byte and a u32
//!   when it is set; an f32 energy; a state index (0xFF none, else an
//!   **unbounded** index into the states, sorted by `state_prio`
//!   0x33C400); a state's u32 end time and its zone groups (0x31B230: a
//!   group that does not keep all its zones on has a u32 and a bit for each
//!   zone, 0x31B390; then 5 bytes for each zone that is on); a flags byte;
//!   a u32; then its artefact containers and its own artefacts;
//! - artefact container (0x31C5B0): the usable base, an artefact id (0xFF
//!   none, else an **unbounded** index into the world's artefacts, which
//!   are the map's anomalies' own, 0x31B8E0) and a u32;
//! - artefact (`inventory_item::deserialize` 0x2FFF10 then
//!   `artefact_base::deserialize` 0x313760): a u16 amount, an owner (0xFF
//!   none, else a place and a kit, looked up with no bound, then the slot
//!   byte when the owner has a player), a state byte (0 to 5) with its 1
//!   to 5 bytes, and a u32; an active one (state 4) then what its core
//!   saves of its own ([`Artefact`]).
//!
//! The check reads the list to the section's exact end and holds every
//! index and float within what the game needs; anything else refuses the
//! world.

use super::{Context, Profile, Section};
use crate::reader::{Reader, Short};
use crate::{KITS, MAX_PLAYERS, Part, Reason, Refusal, finite};

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
    /// has neither and saves no pending byte.
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
/// switch 0x2EE95F): what an active one saves after the base's u32.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Artefact {
    /// 3 (`artefact_lifebone_core::deserialize` 0x3124B0): a u32.
    Lifebone,
    /// 6 (`artefact_spring_core::deserialize` 0x3119E0): a u32.
    Spring,
    /// 7 (`artefact_onyx_core::deserialize` 0x312AF0): a u32 and an f32.
    Onyx,
}

impl Artefact {
    const fn active_bytes(self) -> usize {
        match self {
            Artefact::Lifebone | Artefact::Spring => 4,
            Artefact::Onyx => 4 + 4,
        }
    }
}

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
    /// Its zones turn on and off: a u32 and a bit for each zone are saved.
    Switched { zones: usize },
}

/// A grenade's flag bits (`grenade_core::serialize` 0x303DA0).
const IN_PHYSICS: u8 = 1;
const THROWN: u8 = 2;
const SPENT: u8 = 4;
/// What a thrown grenade saves more, and one in the physics world: a
/// 4x4 transform and two velocities, all floats.
const THROWN_BYTES: usize = 8;
const PHYSICS_FLOATS: usize = 16 + 3 + 3;
/// An armed trap's transform, between a u32 and two u32; state 1 is set
/// up, with its users.
const TRAP_FLOATS: usize = 16;
const TRAP_SET_UP: u8 = 1;
/// The bits an anomaly's flags byte has.
const ANOMALY_FLAGS: u8 = 0b11;
/// An artefact's active state, after which its core saves its own.
const ACTIVE: u8 = 4;

/// The objects read to the section's exact end, every index and float
/// within what the game needs. The list unknown: not checked.
pub fn objects(section: &Section, context: &Context) -> Result<(), Refusal> {
    let Some(list) = context.objects else {
        return Ok(());
    };
    let refuse = |offset, reason| Refusal::new(Part::Objects, offset, reason);
    let mut reader = Reader::new(section.bytes, section.at);
    let mut walk = Walk {
        reader: &mut reader,
        places: context.places.min(MAX_PLAYERS),
        owners: context.profiles,
        artefacts: list.map.artefacts(),
        using: 0,
        object: 0,
    };
    let walked = walk.list(list);
    let object = walk.object;
    let unread = |at| {
        refuse(
            at,
            Reason::ObjectsUnread {
                len: section.bytes.len(),
                objects: object,
            },
        )
    };
    match walked {
        Err(Fault::Short(short)) => Err(unread(short.at)),
        Err(Fault::At(at, reason)) => Err(refuse(at, reason)),
        Ok(()) if reader.is_done() => Ok(()),
        Ok(()) => Err(unread(reader.at())),
    }
}

enum Fault {
    Short(Short),
    At(usize, Reason),
}

impl From<Short> for Fault {
    fn from(short: Short) -> Self {
        Fault::Short(short)
    }
}

struct Walk<'r, 'b, 'c> {
    reader: &'r mut Reader<'b>,
    places: usize,
    /// By place, whose kits have a player (an artefact's owner).
    owners: Option<&'c [Profile]>,
    artefacts: usize,
    /// Bit p: place p uses an object already.
    using: u32,
    /// The object being read, for a refusal.
    object: usize,
}

impl Walk<'_, '_, '_> {
    fn list(&mut self, list: &Objects) -> Result<(), Fault> {
        for held in list.held {
            match held {
                Held::Grenade => self.grenade()?,
                Held::Trap => self.trap()?,
            }
            self.object = self.object.saturating_add(1);
        }
        for _ in 0..list.map.damage_zones {
            // A u32 (+0x9C) and a u8 (+0xA1), neither an index.
            self.reader.skip(5)?;
            self.object = self.object.saturating_add(1);
        }
        for _ in 0..list.map.resupply_zones {
            self.usable()?;
            self.object = self.object.saturating_add(1);
        }
        for anomaly in list.map.anomalies {
            self.anomaly(anomaly)?;
            self.object = self.object.saturating_add(1);
        }
        Ok(())
    }

    fn refuse(&self, at: usize, reason: Reason) -> Fault {
        Fault::At(at, reason)
    }

    fn floats(&mut self, count: usize) -> Result<(), Fault> {
        for _ in 0..count {
            let at = self.reader.at();
            if !finite(self.reader.f32_bits()?) {
                let object = self.object;
                return Err(self.refuse(at, Reason::ObjectsNotNumber { object }));
            }
        }
        Ok(())
    }

    fn flags(&mut self, known: u8) -> Result<u8, Fault> {
        let at = self.reader.at();
        let flags = self.reader.u8()?;
        if flags & !known != 0 {
            let object = self.object;
            return Err(self.refuse(at, Reason::ObjectsFlags { object, flags }));
        }
        Ok(flags)
    }

    fn grenade(&mut self) -> Result<(), Fault> {
        let flags = self.flags(IN_PHYSICS | THROWN | SPENT)?;
        if flags & THROWN != 0 {
            self.reader.skip(THROWN_BYTES)?;
        }
        if flags & IN_PHYSICS != 0 {
            self.floats(PHYSICS_FLOATS)?;
        }
        Ok(())
    }

    fn trap(&mut self) -> Result<(), Fault> {
        let state = self.reader.u8()?;
        if state == 0 {
            return Ok(());
        }
        self.reader.skip(4)?;
        self.floats(TRAP_FLOATS)?;
        self.reader.skip(4 + 4)?;
        if state == TRAP_SET_UP {
            self.usable()?;
        }
        Ok(())
    }

    /// The usable base: at most 20 users, each a place of the roster that
    /// uses no other object.
    fn usable(&mut self) -> Result<(), Fault> {
        let object = self.object;
        let at = self.reader.at();
        let count = self.reader.u8()?;
        if usize::from(count) > MAX_PLAYERS {
            return Err(self.refuse(at, Reason::ObjectsUsers { object, count }));
        }
        for _ in 0..count {
            let at = self.reader.at();
            let user = self.reader.u8()?;
            if usize::from(user) >= self.places {
                let places = self.places;
                return Err(self.refuse(
                    at,
                    Reason::ObjectsUserPast {
                        object,
                        user,
                        places,
                    },
                ));
            }
            let bit = 1u32.checked_shl(u32::from(user)).unwrap_or(0);
            if self.using & bit != 0 {
                return Err(self.refuse(at, Reason::ObjectsUserTwice { object, user }));
            }
            self.using |= bit;
        }
        Ok(())
    }

    fn anomaly(&mut self, anomaly: &Anomaly) -> Result<(), Fault> {
        let object = self.object;
        if anomaly.artefacts.is_some() && self.reader.u8()? != 0 {
            self.reader.skip(4)?;
        }
        self.floats(1)?;
        let at = self.reader.at();
        let state = self.reader.u8()?;
        if state != 0xFF {
            let Some(chosen) = anomaly.states.get(usize::from(state)) else {
                let states = anomaly.states.len();
                return Err(self.refuse(
                    at,
                    Reason::ObjectsStatePast {
                        object,
                        state,
                        states,
                    },
                ));
            };
            // Its end time, then its groups.
            self.reader.skip(4)?;
            for group in chosen.groups {
                let on = match *group {
                    Group::KeepAll { zones } => zones,
                    Group::Switched { zones } => self.switched(zones)?,
                };
                self.reader.skip(on.saturating_mul(5))?;
            }
        }
        self.flags(ANOMALY_FLAGS)?;
        self.reader.skip(4)?;
        if let Some(artefacts) = anomaly.artefacts {
            for _ in 0..artefacts.containers {
                self.container()?;
            }
            for &artefact in artefacts.artefacts {
                self.artefact(artefact)?;
            }
        }
        Ok(())
    }

    /// A switched group's u32 and its zones' bits: how many are on. A bit
    /// past the zones is no game's.
    fn switched(&mut self, zones: usize) -> Result<usize, Fault> {
        self.reader.skip(4)?;
        let mut on = 0usize;
        for byte in 0..zones.div_ceil(8) {
            let at = self.reader.at();
            let bits = self.reader.u8()?;
            let held = zones.saturating_sub(byte.saturating_mul(8)).min(8);
            let mask = 1u16
                .checked_shl(held as u32)
                .map_or(0xFF, |bit| bit.wrapping_sub(1));
            if u16::from(bits) & !mask != 0 {
                let object = self.object;
                return Err(self.refuse(at, Reason::ObjectsZoneBits { object, bits }));
            }
            on = on.saturating_add(bits.count_ones() as usize);
        }
        Ok(on)
    }

    fn container(&mut self) -> Result<(), Fault> {
        self.usable()?;
        let at = self.reader.at();
        let artefact = self.reader.u8()?;
        if artefact != 0xFF && usize::from(artefact) >= self.artefacts {
            let (object, artefacts) = (self.object, self.artefacts);
            return Err(self.refuse(
                at,
                Reason::ObjectsArtefactPast {
                    object,
                    artefact,
                    artefacts,
                },
            ));
        }
        self.reader.skip(4)?;
        Ok(())
    }

    /// An anomaly's artefact: its amount, its owner, its state.
    fn artefact(&mut self, artefact: Artefact) -> Result<(), Fault> {
        let object = self.object;
        self.reader.skip(2)?;
        let at = self.reader.at();
        let place = self.reader.u8()?;
        if place != 0xFF {
            let kit = self.reader.u8()?;
            if !self.has_player(place, kit) {
                return Err(self.refuse(at, Reason::ObjectsOwner { object, place, kit }));
            }
            // The slot it is in.
            self.reader.skip(1)?;
        }
        let at = self.reader.at();
        // 0 free, 1 maturing in a container, 2 ready there, 3 held or
        // cooling down, 4 active, 5 (written at 0x3132F1); the game writes
        // no other.
        let state = self.reader.u8()?;
        match state {
            0 | 3..=5 => self.reader.skip(4)?,
            1 => self.reader.skip(1 + 4)?,
            2 => self.reader.skip(1)?,
            state => {
                return Err(self.refuse(at, Reason::ObjectsArtefactState { object, state }));
            }
        }
        self.reader.skip(4)?;
        if state == ACTIVE {
            self.reader.skip(artefact.active_bytes())?;
        }
        Ok(())
    }

    /// Whether `place`'s kit `kit` has a player: the owner lookup
    /// `[core+0x18458]+(place*12+kit)*4` finds one and the slot byte
    /// follows. A world names no other owner. The profiles unknown: any kit
    /// of the table of a roster place.
    fn has_player(&self, place: u8, kit: u8) -> bool {
        if usize::from(place) >= self.places || usize::from(kit) >= KITS {
            return false;
        }
        self.owners.is_none_or(|profiles| {
            profiles
                .get(usize::from(place))
                .is_some_and(|profile| profile.usable(kit))
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
    /// An artefact owned by place 2's kit 0, in slot 4, set (state 1).
    const OWNED: [u8; 6] = [1, 0, 2, 0, 4, 1];
    /// A container used by place 1, holding artefact 1.
    const USED: [u8; 7] = [1, 1, 1, 0, 0, 0, 0];

    /// A section that reads to its end for [`LIST`].
    fn good() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.push(0); // an untouched grenade
        bytes.push(0); // an untouched trap
        bytes.extend([1, 2, 3, 4, 5]); // the damage zone
        bytes.extend([2, 0, 3]); // the resupply zone: places 0 and 3
        // The anomaly: pending with its time, energy, state 0 with its
        // time; group 0: a u32 and zones 0, 2 and 9 on, their 15 bytes;
        // group 1: its one zone's 5; flags, a u32.
        bytes.extend([1, 9, 9, 9, 9]);
        bytes.extend(1.0f32.to_le_bytes());
        bytes.extend([0, 7, 7, 7, 7]);
        bytes.extend([0, 0, 0, 0, 0b101, 0b10]);
        bytes.extend([0; 15]);
        bytes.extend([0; 5]);
        bytes.extend([3, 0, 0, 0, 0]);
        // Its containers: one used, one empty.
        bytes.extend(USED);
        bytes.extend([0, 0xFF, 0, 0, 0, 0]);
        // Its artefacts: one owned, one lying (state 0).
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
        let mut bytes = std::vec![IN_PHYSICS | THROWN];
        bytes.extend([0; THROWN_BYTES]);
        bytes.extend([0; PHYSICS_FLOATS * 4]);
        bytes.extend([TRAP_SET_UP, 0, 0, 0, 0]);
        bytes.extend([0; TRAP_FLOATS * 4]);
        bytes.extend([0; 8]);
        bytes.extend([1, 2]);
        assert_eq!(reason(&list, &bytes), None);
        // A NaN in the grenade's transform; an unknown flag.
        let mut nan = bytes.clone();
        let float = 1 + THROWN_BYTES;
        nan[float..float + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(matches!(
            reason(&list, &nan),
            Some(Reason::ObjectsNotNumber { object: 0 })
        ));
        let mut flag = bytes.clone();
        flag[0] |= 8;
        assert!(matches!(
            reason(&list, &flag),
            Some(Reason::ObjectsFlags { object: 0, .. })
        ));
    }

    #[test]
    fn an_active_artefact_reads_what_its_core_saves() {
        // The anomaly's artefacts active (state 4): the onyx's u32 and f32,
        // the spring's u32, after the base's two u32.
        let active = |own: usize| {
            let mut bytes = std::vec![1, 0, 0xFF, ACTIVE];
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
    fn an_index_past_its_bound_is_refused() {
        let good = good();
        let mutate = |offset: usize, byte: u8| {
            let mut bytes = good.clone();
            bytes[offset] = byte;
            reason(&LIST, &bytes)
        };
        // The resupply zone's user past the roster; its users over 20.
        assert!(matches!(
            mutate(8, 4),
            Some(Reason::ObjectsUserPast { user: 4, .. })
        ));
        assert!(matches!(
            mutate(7, 21),
            Some(Reason::ObjectsUsers { count: 21, .. })
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
        // The anomaly's state past its two; a zone bit past the group's ten.
        let state = 2 + 5 + 3 + 5 + 4;
        assert!(matches!(
            mutate(state, 2),
            Some(Reason::ObjectsStatePast { state: 2, .. })
        ));
        assert!(matches!(
            mutate(state + 5 + 4 + 1, 0b110),
            Some(Reason::ObjectsZoneBits { bits: 0b110, .. })
        ));
        // An artefact owned by a kit past the table; its state unknown.
        let artefact = find(&good, &OWNED);
        assert!(matches!(
            mutate(artefact + 3, 3),
            Some(Reason::ObjectsOwner { kit: 3, .. })
        ));
        assert!(matches!(
            mutate(artefact + 5, 6),
            Some(Reason::ObjectsArtefactState { state: 6, .. })
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
}

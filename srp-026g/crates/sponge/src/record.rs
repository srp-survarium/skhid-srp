//! A sponge in a world: its fields after artefact_base's record (0.28d0's
//! artefact_sponge_core::serialize 0x513A70 / deserialize 0x513C80, their
//! names and lines kept, three of ours after them), parsed once into a
//! [`Saved`] and written from one. Little-endian, no padding:
//!
//! ```text
//! u32  m_explode_time_ms      FFFFFFFFh unarmed; the fuse's end armed; the cloud's start exploded
//! u8   m_is_pin_pulled        1 armed (game state 6), else 0
//! u8   thrown()               1 armed in the air, else 0
//! thrown: f32 x 16 m_transform, f32 x 3 linear_velocity, f32 x 3 angular_velocity
//! u8   m_contact              armed: 1 waiting, 3 seen, 0 spent; else 0
//! u8   m_zone_owner           the thrower's place (< 20), FFh nobody
//! u8   m_throw_state_holds    0 or 1
//! exploded with its cloud on: the damage zone's own record (damage_zone_core's
//! serializable +130h, 0x3193F0), then f32 x 16, the explosion's transform
//! ```
//!
//! The game state (artefact_base's m_state, read before) says which: 0..5
//! [`Saved::Held`], 6 armed, 7 exploded ([`Saved::Cloud`]) or spent. Any other
//! combination is no sponge the game writes and is refused, naming the field.

use crate::machine::{Armed, Cloud, Contact, Hold, State, Thrower};
use crate::{GameTime, Place};

/// m_explode_time_ms of an unarmed sponge (0.28's).
pub const NOT_ARMED: u32 = 0xFFFF_FFFF;
/// m_zone_owner of no thrower.
pub const NOBODY: u8 = 0xFF;
/// The game states (artefact_base +280h) the Sponge adds.
pub const STATE_ARMED: u8 = 6;
pub const STATE_EXPLODED: u8 = 7;
/// The last of artefact_base's own states (5, its holder dead).
pub const STATE_LAST_BASE: u8 = 5;

/// Where an armed sponge is in the air: its body's transform and velocities.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub transform: [f32; 16],
    pub linear: [f32; 3],
    pub angular: [f32; 3],
}

/// What a world says of a sponge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Saved {
    /// An artefact (game state `base`, 0..5).
    Held { base: u8 },
    InHand(Armed),
    InAir(Armed, Motion),
    /// The zone's record and the explosion's transform follow.
    Cloud(Cloud),
    Spent,
}

impl Saved {
    /// What `state` writes; `motion` is asked only in the air.
    pub fn of(state: &State, base: u8, motion: impl FnOnce() -> Motion) -> Saved {
        match *state {
            State::Held => Saved::Held { base },
            State::InHand(armed) => Saved::InHand(armed),
            State::InAir(air) => Saved::InAir(air.armed, motion()),
            State::Cloud(cloud) => Saved::Cloud(cloud),
            State::Spent => Saved::Spent,
        }
    }
}

/// A field as the game's writers take it: its 0.28 name and line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Field {
    U32 { name: &'static str, line: u32, value: u32 },
    U8 { name: &'static str, line: u32, value: u8 },
    Matrix { name: &'static str, line: u32, value: [f32; 16] },
    Float3 { name: &'static str, line: u32, value: [f32; 3] },
}

/// The explosion's transform after the zone's record (0.28 wrote its
/// geometry's, which is the explosion's).
pub fn cloud_transform(value: [f32; 16]) -> Field {
    Field::Matrix { name: "m_damage_zone->get_collision_geometry(0)->transform()", line: 0x4F, value }
}

fn wire_contact(contact: Contact) -> u8 {
    match contact {
        Contact::Waiting => 1,
        Contact::Seen => 3,
        Contact::Spent => 0,
    }
}

fn wire_thrower(thrower: Thrower) -> u8 {
    match thrower {
        Thrower::Nobody => NOBODY,
        Thrower::Place(place) => place.get(),
    }
}

fn wire_hold(hold: Hold) -> u8 {
    match hold {
        Hold::Free => 0,
        Hold::ThrowState => 1,
    }
}

/// The sponge's fields of `saved`, in order (the zone's record and
/// [`cloud_transform`] of a cloud are the caller's after them).
pub fn write(saved: &Saved, mut put: impl FnMut(Field)) {
    let (explode, pin, armed) = match saved {
        Saved::Held { .. } | Saved::Spent => (NOT_ARMED, 0, None),
        Saved::InHand(armed) | Saved::InAir(armed, _) => (armed.explode_at.0, 1, Some(armed)),
        Saved::Cloud(cloud) => (cloud.exploded_at.0, 0, None),
    };
    put(Field::U32 { name: "m_explode_time_ms", line: 0x3B, value: explode });
    put(Field::U8 { name: "m_is_pin_pulled", line: 0x3C, value: pin });
    let motion = match saved {
        Saved::InAir(_, motion) => Some(motion),
        _ => None,
    };
    put(Field::U8 { name: "thrown()", line: 0x3D, value: u8::from(motion.is_some()) });
    if let Some(motion) = motion {
        put(Field::Matrix { name: "m_transform", line: 0x41, value: motion.transform });
        put(Field::Float3 { name: "linear_velocity", line: 0x44, value: motion.linear });
        put(Field::Float3 { name: "angular_velocity", line: 0x47, value: motion.angular });
    }
    let (thrower, hold) = match saved {
        Saved::Held { .. } => (Thrower::Nobody, Hold::Free),
        Saved::Spent => (Thrower::Nobody, Hold::ThrowState),
        Saved::InHand(armed) | Saved::InAir(armed, _) => (armed.thrower, armed.hold),
        Saved::Cloud(cloud) => (cloud.thrower, cloud.hold),
    };
    put(Field::U8 { name: "m_contact", line: 0x48, value: armed.map_or(0, |a| wire_contact(a.contact)) });
    put(Field::U8 { name: "m_zone_owner", line: 0x49, value: wire_thrower(thrower) });
    put(Field::U8 { name: "m_throw_state_holds", line: 0x4A, value: wire_hold(hold) });
}

/// Why bytes are no sponge's: where (from the start of the slice) and what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// Cut short: `at` needed more bytes.
    Short { at: usize },
    /// A field holds a value the game never writes there (in that state).
    Value { at: usize, field: &'static str, value: u32 },
    /// A float is not a number.
    NotNumber { at: usize, field: &'static str },
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], Fault> {
        let end = self.at.checked_add(N).filter(|&end| end <= self.bytes.len()).ok_or(Fault::Short { at: self.at })?;
        let out = self.bytes[self.at..end].try_into().map_err(|_| Fault::Short { at: self.at })?;
        self.at = end;
        Ok(out)
    }

    fn u32(&mut self) -> Result<u32, Fault> {
        self.take::<4>().map(u32::from_le_bytes)
    }

    fn u8(&mut self) -> Result<(usize, u8), Fault> {
        let at = self.at;
        self.take::<1>().map(|[b]| (at, b))
    }

    fn floats<const N: usize>(&mut self, field: &'static str) -> Result<[f32; N], Fault> {
        let at = self.at;
        let mut out = [0.0; N];
        for value in &mut out {
            *value = f32::from_le_bytes(self.take::<4>()?);
            if !value.is_finite() {
                return Err(Fault::NotNumber { at, field });
            }
        }
        Ok(out)
    }
}

fn refuse<T>(at: usize, field: &'static str, value: u32) -> Result<T, Fault> {
    Err(Fault::Value { at, field, value })
}

/// The sponge's fields at the start of `bytes`, its game state `base` read
/// before them: what they say and how many bytes they took. A cloud's
/// zone record and transform follow them ([`parse_cloud_transform`]).
pub fn parse(base: u8, bytes: &[u8]) -> Result<(Saved, usize), Fault> {
    let mut r = Reader { bytes, at: 0 };
    let explode_at = r.at;
    let explode = r.u32()?;
    let (pin_at, pin) = r.u8()?;
    let (thrown_at, thrown) = r.u8()?;
    let armed = match (base, pin) {
        (STATE_ARMED, 1) => true,
        (STATE_ARMED, _) | (0..=STATE_LAST_BASE | STATE_EXPLODED, 1..) => return refuse(pin_at, "m_is_pin_pulled", pin.into()),
        (0..=STATE_LAST_BASE | STATE_EXPLODED, 0) => false,
        (_, _) => return refuse(0, "artefact m_state", base.into()),
    };
    let motion = match thrown {
        0 => None,
        1 if armed => {
            let transform = r.floats::<16>("m_transform")?;
            let linear = r.floats::<3>("linear_velocity")?;
            let angular = r.floats::<3>("angular_velocity")?;
            Some(Motion { transform, linear, angular })
        }
        _ => return refuse(thrown_at, "thrown()", thrown.into()),
    };
    let (contact_at, contact) = r.u8()?;
    // seen only in the air: in the hand its body is in no physics world
    let contact = match (armed, contact) {
        (true, 1) => Contact::Waiting,
        (true, 3) if motion.is_some() => Contact::Seen,
        (_, 0) => Contact::Spent,
        _ => return refuse(contact_at, "m_contact", contact.into()),
    };
    let (owner_at, owner) = r.u8()?;
    let thrower = match (owner, Place::new(owner)) {
        (NOBODY, _) => Thrower::Nobody,
        (_, Some(place)) => Thrower::Place(place),
        (_, None) => return refuse(owner_at, "m_zone_owner", owner.into()),
    };
    let (hold_at, hold) = r.u8()?;
    let hold = match hold {
        0 => Hold::Free,
        1 => Hold::ThrowState,
        _ => return refuse(hold_at, "m_throw_state_holds", hold.into()),
    };
    let saved = if armed {
        let armed = Armed { explode_at: GameTime(explode), thrower, contact, hold };
        match motion {
            Some(motion) => Saved::InAir(armed, motion),
            None => Saved::InHand(armed),
        }
    } else if base == STATE_EXPLODED && explode != NOT_ARMED {
        Saved::Cloud(Cloud { exploded_at: GameTime(explode), thrower, hold })
    } else {
        // unarmed: no fuse, no thrower; spent is the exploded state's with a throw state holding it
        if explode != NOT_ARMED {
            return refuse(explode_at, "m_explode_time_ms", explode);
        }
        if thrower != Thrower::Nobody {
            return refuse(owner_at, "m_zone_owner", owner.into());
        }
        match (base, hold) {
            (STATE_EXPLODED, Hold::ThrowState) => Saved::Spent,
            // never 4 (active): its action slot is the Sponge's, artefact_base's 0x313339 never runs
            (0..=3 | STATE_LAST_BASE, Hold::Free) => Saved::Held { base },
            _ => return refuse(hold_at, "m_throw_state_holds", wire_hold(hold).into()),
        }
    };
    Ok((saved, r.at))
}

/// The explosion's transform after a cloud's zone record: 64 bytes.
pub fn parse_cloud_transform(bytes: &[u8]) -> Result<([f32; 16], usize), Fault> {
    let mut r = Reader { bytes, at: 0 };
    let transform = r.floats::<16>("m_damage_zone->get_collision_geometry(0)->transform()")?;
    Ok((transform, r.at))
}

#[cfg(test)]
mod tests {
    use super::*;

    extern crate std;
    use std::vec::Vec;

    fn bytes(saved: &Saved) -> Vec<u8> {
        let mut out = Vec::new();
        write(saved, |field| match field {
            Field::U32 { value, .. } => out.extend_from_slice(&value.to_le_bytes()),
            Field::U8 { value, .. } => out.push(value),
            Field::Matrix { value, .. } => value.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes())),
            Field::Float3 { value, .. } => value.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes())),
        });
        out
    }

    fn base_of(saved: &Saved) -> u8 {
        match saved {
            Saved::Held { base } => *base,
            Saved::InHand(_) | Saved::InAir(..) => STATE_ARMED,
            Saved::Cloud(_) | Saved::Spent => STATE_EXPLODED,
        }
    }

    #[test]
    fn every_state_round_trips() {
        let thrower = Thrower::Place(Place::new(7).unwrap());
        let armed = Armed { explode_at: GameTime(103000), thrower, contact: Contact::Seen, hold: Hold::ThrowState };
        let motion = Motion { transform: [0.5; 16], linear: [1.0, -2.0, 3.0], angular: [0.0; 3] };
        let all = [
            Saved::Held { base: 0 },
            Saved::Held { base: 3 },
            Saved::InHand(Armed { contact: Contact::Waiting, ..armed }),
            Saved::InHand(Armed { contact: Contact::Spent, ..armed }),
            Saved::InAir(armed, motion),
            Saved::InAir(Armed { thrower: Thrower::Nobody, contact: Contact::Spent, hold: Hold::Free, ..armed }, motion),
            Saved::Cloud(Cloud { exploded_at: GameTime(103000), thrower, hold: Hold::Free }),
            Saved::Spent,
        ];
        for saved in all {
            let b = bytes(&saved);
            assert_eq!(parse(base_of(&saved), &b), Ok((saved, b.len())), "{saved:?}");
        }
        assert_eq!(bytes(&Saved::Held { base: 0 }).len(), 9);
        assert_eq!(bytes(&Saved::InAir(armed, motion)).len(), 9 + 64 + 24);
    }

    #[test]
    fn refuses_what_the_game_never_writes() {
        let held = bytes(&Saved::Held { base: 0 });
        assert_eq!(parse(8, &held), Err(Fault::Value { at: 0, field: "artefact m_state", value: 8 }));
        assert!(matches!(parse(STATE_ARMED, &held), Err(Fault::Value { field: "m_is_pin_pulled", .. })));
        assert!(matches!(parse(STATE_EXPLODED, &held), Err(Fault::Value { field: "m_throw_state_holds", .. })));
        assert!(matches!(parse(4, &held), Err(Fault::Value { field: "m_throw_state_holds", .. })));
        let seen = Armed { explode_at: GameTime(1), thrower: Thrower::Nobody, contact: Contact::Seen, hold: Hold::Free };
        assert!(matches!(parse(STATE_ARMED, &bytes(&Saved::InHand(seen))), Err(Fault::Value { field: "m_contact", .. })));
        assert_eq!(parse(0, &held[..8]), Err(Fault::Short { at: 8 }));
        let mut bad = held.clone();
        bad[7] = 20;
        assert!(matches!(parse(0, &bad), Err(Fault::Value { field: "m_zone_owner", .. })));
        let mut bad = held.clone();
        bad[6] = 1;
        assert!(matches!(parse(0, &bad), Err(Fault::Value { field: "m_contact", .. })));
        let mut bad = held;
        bad[0] = 0;
        assert!(matches!(parse(0, &bad), Err(Fault::Value { field: "m_explode_time_ms", .. })));
        let armed = Armed { explode_at: GameTime(1), thrower: Thrower::Nobody, contact: Contact::Waiting, hold: Hold::Free };
        let mut air = bytes(&Saved::InAir(armed, Motion { transform: [0.0; 16], linear: [0.0; 3], angular: [0.0; 3] }));
        air[6..10].copy_from_slice(&f32::NAN.to_le_bytes());
        assert_eq!(parse(STATE_ARMED, &air), Err(Fault::NotNumber { at: 6, field: "m_transform" }));
    }
}

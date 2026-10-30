//! Dropped-weapon pickup's shared rules (client patch 83,
//! `docs/notes/weapon_pickup.md`): pure Rust, no engine pointers, used by the
//! client patch and the match server alike.
//!
//! - the pickup command an input carries ([`command`], [`encode`]): patch
//!   78's marker bit 29 with a pattern of two or more quick-use bits, which
//!   patch 78 already reads as no carry command and keeps from the game;
//! - the server's check of it and the client's twin ([`allowed`],
//!   [`sane`]): the client never sends what the server would drop;
//! - a drop's window ([`WINDOW_MS`]) and the one synced record a player
//!   carries in its state ([`Record`]): its own drop (open or taken) and the
//!   gun it borrows, if any.
#![cfg_attr(not(test), no_std)]

/// Patch 78's marker bit (unused by the game; bits 30, 31 stay clear).
pub const CHORD: u32 = 1 << 29;
/// The six quick-use bits, 14..19.
pub const QUICK: u32 = 63 << 14;
/// Use (bit 21).
pub const USE: u32 = 1 << 21;
/// How long a dead player's gun can be taken, simulation milliseconds.
pub const WINDOW_MS: u32 = 5_000;
/// A match's places (core's buffer holds 20).
pub const PLACES: u8 = 20;

/// A place of the match, 0..19.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Place(u8);

impl Place {
    pub const fn new(place: u8) -> Option<Place> {
        if place < PLACES { Some(Place(place)) } else { None }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// A profile's kit, 0..2.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Kit(u8);

impl Kit {
    pub const fn new(kit: u8) -> Option<Kit> {
        if kit < 3 { Some(Kit(kit)) } else { None }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

/// One of the two weapon slots of an inventory (7 and 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WeaponSlot {
    First,
    Second,
}

impl WeaponSlot {
    pub const fn from_slot(slot: u8) -> Option<WeaponSlot> {
        match slot {
            7 => Some(WeaponSlot::First),
            10 => Some(WeaponSlot::Second),
            _ => None,
        }
    }

    /// Its inventory slot.
    pub const fn slot(self) -> u8 {
        match self {
            WeaponSlot::First => 7,
            WeaponSlot::Second => 10,
        }
    }

    /// The inventory slots of its ammunition: two beside it and two of the
    /// four extra ammo slots (19..22).
    pub const fn ammo_slots(self) -> [u8; 4] {
        match self {
            WeaponSlot::First => [8, 9, 19, 20],
            WeaponSlot::Second => [11, 12, 21, 22],
        }
    }

    const fn bit(self) -> u8 {
        match self {
            WeaponSlot::First => 0,
            WeaponSlot::Second => 1,
        }
    }

    const fn from_bit(bit: u8) -> WeaponSlot {
        if bit == 0 { WeaponSlot::First } else { WeaponSlot::Second }
    }
}

/// Simulation milliseconds (player+324h), wrapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ms(pub u32);

impl Ms {
    /// Milliseconds from `earlier` to `self` (wrapping: never negative).
    pub const fn since(self, earlier: Ms) -> u32 {
        self.0.wrapping_sub(earlier.0)
    }
}

/// The 20 six-bit patterns with two or more bits set, in increasing order:
/// place n's pickup command carries the nth.
const PATTERNS: [u8; PLACES as usize] = {
    let mut out = [0u8; PLACES as usize];
    let (mut n, mut value) = (0usize, 0u8);
    while n < PLACES as usize {
        if value.count_ones() >= 2 {
            out[n] = value;
            n += 1;
        }
        value += 1;
    }
    out
};

/// The place an input's action mask asks to take the gun of, if any: the
/// marker with one of the 20 patterns. A single quick bit is patch 78's
/// carry command; any other pattern (two inputs OR-merged into one node)
/// asks for nothing.
pub fn command(actions: u32) -> Option<Place> {
    if actions & CHORD == 0 {
        return None;
    }
    let quick = ((actions & QUICK) >> 14) as u8;
    PATTERNS.iter().position(|&p| p == quick).map(|n| Place(n as u8))
}

/// The input builder's mask asking for `target`'s gun: the marker and its
/// pattern; Use and the quick bits of the press are consumed.
pub fn encode(actions: u32, target: Place) -> u32 {
    (actions & !(QUICK | USE)) | CHORD | (u32::from(PATTERNS[usize::from(target.0)]) << 14)
}

/// The server's check, which the client applies before it sends: a pickup
/// names a place of the match other than the sender's own.
pub fn allowed(sender: Place, target: Place, places: u8) -> bool {
    target != sender && target.0 < places
}

/// Whether the server relays an input with these actions from `sender`: no
/// pickup, or an [`allowed`] one. The server drops any other as it drops an
/// input with an impossible rotation (its owner told to drop it too), and the
/// client never makes one.
pub fn sane(actions: u32, sender: Place, places: u8) -> bool {
    command(actions).is_none_or(|target| allowed(sender, target, places))
}

/// A player's own drop: the gun it held at its death, while it can be taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drop {
    /// No drop: never died holding a gun of its two.
    None,
    /// Died holding `slot`'s gun of `kit` at `died`.
    Open { kit: Kit, slot: WeaponSlot, died: Ms },
    /// That gun was taken (once only).
    Taken,
}

impl Drop {
    /// The kit and slot that can be taken at `now`: open, within the window,
    /// and the kit's spare free (nobody holds it).
    pub fn available(self, now: Ms, spare_free: bool) -> Option<(Kit, WeaponSlot)> {
        match self {
            Drop::Open { kit, slot, died } if now.since(died) < WINDOW_MS && spare_free => Some((kit, slot)),
            _ => None,
        }
    }
}

/// The gun a living player holds instead of its own `own` slot's: the
/// spare of `donor`'s `kit`, its `slot`'s gun.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Borrow {
    pub donor: Place,
    pub kit: Kit,
    pub slot: WeaponSlot,
    pub own: WeaponSlot,
}

/// The one record a player's state carries: its drop and its borrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub drop: Drop,
    pub borrow: Option<Borrow>,
}

impl Record {
    pub const EMPTY: Record = Record { drop: Drop::None, borrow: None };
    /// Its bytes in a player's state.
    pub const BYTES: usize = 12;
    const VERSION: u8 = 1;

    pub fn encode(self) -> [u8; Self::BYTES] {
        let mut out = [0u8; Self::BYTES];
        out[0] = Self::VERSION;
        match self.drop {
            Drop::None => {}
            Drop::Open { kit, slot, died } => {
                out[1] = 1;
                out[2] = kit.0;
                out[3] = slot.bit();
                out[4..8].copy_from_slice(&died.0.to_le_bytes());
            }
            Drop::Taken => out[1] = 2,
        }
        if let Some(b) = self.borrow {
            out[8] = 1;
            out[9] = b.donor.0;
            out[10] = b.kit.0 | b.slot.bit() << 4;
            out[11] = b.own.bit();
        }
        out
    }

    /// Parses the bytes [`encode`](Self::encode) writes; anything else is
    /// refused (None): an unknown version, tag or value, or non-zero padding.
    pub fn decode(bytes: [u8; Self::BYTES]) -> Option<Record> {
        if bytes[0] != Self::VERSION {
            return None;
        }
        let bit = |b: u8| (b <= 1).then(|| WeaponSlot::from_bit(b));
        let drop = match bytes[1] {
            0 if bytes[2..8] == [0; 6] => Drop::None,
            1 => Drop::Open {
                kit: Kit::new(bytes[2])?,
                slot: bit(bytes[3])?,
                died: Ms(u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]])),
            },
            2 if bytes[2..8] == [0; 6] => Drop::Taken,
            _ => return None,
        };
        let borrow = match bytes[8] {
            0 if bytes[9..12] == [0; 3] => None,
            1 if bytes[10] & !0x13 == 0 => Some(Borrow {
                donor: Place::new(bytes[9])?,
                kit: Kit::new(bytes[10] & 0x0F)?,
                slot: bit(bytes[10] >> 4)?,
                own: bit(bytes[11])?,
            }),
            _ => return None,
        };
        Some(Record { drop, borrow })
    }
}

/// What the picker is at the tick its command is played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picker {
    pub place: Place,
    pub alive: bool,
    /// The weapon slot whose gun is in its hands, if one of its two is.
    pub held: Option<WeaponSlot>,
    pub borrow: Option<Borrow>,
}

/// Why a pickup command does nothing (every client refuses it alike).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Dead,
    /// It already holds a borrowed gun.
    Borrowing,
    /// No gun of its two in its hands.
    NoGunInHands,
    /// Its own place, or a drop not available (none, taken, past 5 s, or the
    /// kit's spare held by someone).
    NotAvailable,
    /// The spare is not of the held gun's kind (a rifle for a pistol).
    OtherKind,
}

/// A pickup every client plays: `picker`'s held slot takes `target`'s spare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Take {
    pub borrow: Borrow,
}

/// The rule of a pickup command (the owner's, `weapon_pickup.md`): a living
/// picker with one of its guns in hands and no borrowed one, an available
/// drop of another place, and a spare of the held gun's kind (both two-handed
/// or both not).
pub fn decide(picker: Picker, target: Place, drop: Drop, now: Ms, spare_free: bool,
              same_kind: impl FnOnce(Kit, WeaponSlot) -> bool) -> Result<Take, Refusal> {
    if !picker.alive {
        return Err(Refusal::Dead);
    }
    if picker.borrow.is_some() {
        return Err(Refusal::Borrowing);
    }
    let own = picker.held.ok_or(Refusal::NoGunInHands)?;
    if target == picker.place {
        return Err(Refusal::NotAvailable);
    }
    let (kit, slot) = drop.available(now, spare_free).ok_or(Refusal::NotAvailable)?;
    if !same_kind(kit, slot) {
        return Err(Refusal::OtherKind);
    }
    Ok(Take { borrow: Borrow { donor: target, kit, slot, own } })
}

/// The borrowed gun's ammunition: the magazine as the donor left it (the
/// chamber's round too), one full magazine in reserve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ammo {
    pub loaded: u8,
    pub chamber: bool,
    pub reserve: u16,
}

impl Ammo {
    pub fn grant(capacity: u8, loaded: u8, chamber: bool) -> Ammo {
        Ammo { loaded: loaded.min(capacity), chamber, reserve: u16::from(capacity) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_round_trip_and_stay_apart_from_carry() {
        for p in 0..PLACES {
            let place = Place::new(p).unwrap();
            let mask = encode(USE | 0x4000 | 0x21, place);
            assert_eq!(command(mask), Some(place));
            assert_eq!(mask & USE, 0);
            assert_eq!(mask & 0x21, 0x21);
            // Patch 78's decoder: exactly one quick bit is a carry command.
            assert!((((mask & QUICK) >> 14) as u8).count_ones() >= 2);
        }
        for slot in 0..6 {
            assert_eq!(command(CHORD | 1 << (14 + slot)), None, "a carry command is no pickup");
        }
        assert_eq!(command(QUICK & 0x3 << 14), None, "no marker");
        assert_eq!(command(CHORD), None);
        assert_eq!(command(CHORD | QUICK), None, "past the 20 patterns");
    }

    #[test]
    fn the_server_and_the_client_check_alike() {
        let (me, other) = (Place::new(3).unwrap(), Place::new(5).unwrap());
        assert!(allowed(me, other, 10));
        assert!(!allowed(me, me, 10));
        assert!(!allowed(me, other, 5));
        assert!(sane(encode(0x1, other), me, 10));
        assert!(!sane(encode(0x1, other), me, 5));
        assert!(!sane(encode(0x1, me), me, 10));
        assert!(sane(CHORD | 0x4000, me, 10), "a carry command");
        assert!(sane(0x1, me, 10));
    }

    #[test]
    fn a_drop_is_open_for_five_seconds_while_its_spare_is_free() {
        let drop = Drop::Open { kit: Kit::new(1).unwrap(), slot: WeaponSlot::Second, died: Ms(u32::MAX - 1000) };
        let at = |ms: u32| Ms((u32::MAX - 1000).wrapping_add(ms));
        assert_eq!(drop.available(at(0), true), Some((Kit::new(1).unwrap(), WeaponSlot::Second)));
        assert!(drop.available(at(4_999), true).is_some());
        assert_eq!(drop.available(at(5_000), true), None);
        assert_eq!(drop.available(at(10), false), None);
        assert_eq!(Drop::Taken.available(at(0), true), None);
    }

    #[test]
    fn records_round_trip_and_refuse_anything_else() {
        let borrow = Borrow { donor: Place::new(19).unwrap(), kit: Kit::new(2).unwrap(), slot: WeaponSlot::Second,
                              own: WeaponSlot::First };
        let cases = [
            Record::EMPTY,
            Record { drop: Drop::Taken, borrow: Some(borrow) },
            Record { drop: Drop::Open { kit: Kit::new(0).unwrap(), slot: WeaponSlot::First, died: Ms(123_456) },
                     borrow: None },
        ];
        for record in cases {
            assert_eq!(Record::decode(record.encode()), Some(record));
        }
        let good = cases[1].encode();
        for (at, value) in [(0, 2), (1, 3), (8, 2), (9, 20), (10, 3), (10, 0x22), (11, 2), (2, 1)] {
            let mut bad = good;
            bad[at] = value;
            assert_eq!(Record::decode(bad), None, "byte {at} = {value}");
        }
    }

    #[test]
    fn a_pickup_takes_an_available_drop_of_the_same_kind_once() {
        let me = Place::new(1).unwrap();
        let picker = Picker { place: me, alive: true, held: Some(WeaponSlot::Second), borrow: None };
        let donor = Place::new(4).unwrap();
        let kit = Kit::new(2).unwrap();
        let drop = Drop::Open { kit, slot: WeaponSlot::First, died: Ms(1_000) };
        let take = decide(picker, donor, drop, Ms(3_000), true, |_, _| true).unwrap();
        assert_eq!(take.borrow, Borrow { donor, kit, slot: WeaponSlot::First, own: WeaponSlot::Second });
        assert_eq!(decide(picker, donor, drop, Ms(6_000), true, |_, _| true), Err(Refusal::NotAvailable));
        assert_eq!(decide(picker, donor, drop, Ms(3_000), false, |_, _| true), Err(Refusal::NotAvailable));
        assert_eq!(decide(picker, donor, Drop::Taken, Ms(3_000), true, |_, _| true), Err(Refusal::NotAvailable));
        assert_eq!(decide(picker, me, drop, Ms(3_000), true, |_, _| true), Err(Refusal::NotAvailable));
        assert_eq!(decide(picker, donor, drop, Ms(3_000), true, |_, _| false), Err(Refusal::OtherKind));
        assert_eq!(decide(Picker { alive: false, ..picker }, donor, drop, Ms(3_000), true, |_, _| true),
                   Err(Refusal::Dead));
        assert_eq!(decide(Picker { held: None, ..picker }, donor, drop, Ms(3_000), true, |_, _| true),
                   Err(Refusal::NoGunInHands));
        assert_eq!(decide(Picker { borrow: Some(take.borrow), ..picker }, donor, drop, Ms(3_000), true, |_, _| true),
                   Err(Refusal::Borrowing));
        assert_eq!(Ammo::grant(30, 12, true), Ammo { loaded: 12, chamber: true, reserve: 30 });
        assert_eq!(Ammo::grant(8, 9, false).loaded, 8);
    }
}

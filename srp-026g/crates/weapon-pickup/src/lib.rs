//! Dropped-weapon pickup's shared rules (client patch 85,
//! `docs/notes/weapon_pickup.md`): pure Rust, no engine pointers, used by the
//! client patch and the match server alike.
//!
//! - the pickup command an input carries ([`command`], [`encode`]): patch
//!   78's marker bit 29 with a pattern of two or more quick-use bits, which
//!   patch 78 already reads as no carry command and keeps from the game;
//! - the server's check of it and the client's twin ([`allowed`],
//!   [`sane`]): the client never sends what the server would drop;
//! - a drop, open until its gun leaves the world with its body (the place's
//!   player removed: its respawn or its departure), and the one synced
//!   record a player carries in its state ([`Record`]): its own drop (open or
//!   taken) and the gun it borrows, if any;
//! - the rule of a pickup ([`decide`]): the held gun replaced by any gun, as
//!   the carry rules after the skill modifiers allow ([`CarryRule`]).
#![cfg_attr(not(test), no_std)]

/// Patch 78's marker bit (unused by the game; bits 30, 31 stay clear).
pub const CHORD: u32 = 1 << 29;
/// The six quick-use bits, 14..19.
pub const QUICK: u32 = 63 << 14;
/// Use (bit 21).
pub const USE: u32 = 1 << 21;
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

    /// The other weapon slot.
    pub const fn other(self) -> WeaponSlot {
        match self {
            WeaponSlot::First => WeaponSlot::Second,
            WeaponSlot::Second => WeaponSlot::First,
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

/// A player's own drop: the gun it held at its death, while its body (and
/// the gun on it) is in the world. The place's player taken out of the world
/// (base_player::remove(1): its respawn, its departure) ends it: the drop is
/// [`Drop::None`] again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drop {
    /// No drop: not dead holding a gun of its two, or its body gone.
    None,
    /// Died holding `slot`'s gun of `kit`.
    Open { kit: Kit, slot: WeaponSlot },
    /// That gun was taken (once only).
    Taken,
}

impl Drop {
    /// The kit and slot that can be taken: an open drop's (each death has
    /// its own gun).
    pub fn available(self) -> Option<(Kit, WeaponSlot)> {
        match self {
            Drop::Open { kit, slot } => Some((kit, slot)),
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
            Drop::Open { kit, slot } => {
                out[1] = 1;
                out[2] = kit.0;
                out[3] = slot.bit();
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
            1 if bytes[4..8] == [0; 4] => Drop::Open { kit: Kit::new(bytes[2])?, slot: bit(bytes[3])? },
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

/// A gun's handedness: its item's `double_handed` property (weapon+5E9h,
/// from its config, 0x30AB8D..0x30ABC0), not a guess from its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handedness {
    One,
    Two,
}

impl Handedness {
    pub const fn from_double_handed(double_handed: bool) -> Handedness {
        if double_handed { Handedness::Two } else { Handedness::One }
    }
}

/// What a kit may carry in its two weapon slots, after its skill modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarryRule {
    /// `disable_second_weapon` (modifier 64): only slot 7 holds a gun.
    OneWeapon,
    /// The normal rule: at most one double-handed gun.
    OneDoubleHanded,
    /// `use_two_double_handed_weapons` (modifier 25): both may be.
    TwoDoubleHanded,
}

impl CarryRule {
    /// The rule from the kit's two modifier sums (both modifiers additive,
    /// the list at 0x927E08: apply_modifier of 0 with weight 1 is the sum);
    /// a sum above 0 has the skill, and `disable_second_weapon` wins.
    pub fn from_modifiers(disable_second_weapon: f32, use_two_double_handed_weapons: f32) -> CarryRule {
        if disable_second_weapon > 0.0 {
            CarryRule::OneWeapon
        } else if use_two_double_handed_weapons > 0.0 {
            CarryRule::TwoDoubleHanded
        } else {
            CarryRule::OneDoubleHanded
        }
    }
}

/// What the picker is at the tick its command is played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picker {
    pub place: Place,
    pub alive: bool,
    /// The weapon slot whose gun is in its hands, if one of its two is.
    pub held: Option<WeaponSlot>,
    /// The gun in its other weapon slot, if any.
    pub other: Option<Handedness>,
    pub rule: CarryRule,
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
    /// Its own place, or a drop not available (none, taken, its body gone).
    NotAvailable,
    /// `disable_second_weapon`: a gun held in slot 10, or one beside it.
    SecondWeaponDisabled,
    /// A double-handed gun beside another, without
    /// `use_two_double_handed_weapons`.
    TwoDoubleHanded,
}

/// A pickup every client plays: `picker`'s held slot takes `target`'s spare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Take {
    pub borrow: Borrow,
}

/// The rule of a pickup command (the owner's, `weapon_pickup.md`): a living
/// picker with one of its guns in hands and no borrowed one, an available
/// drop of another place; the held gun is replaced (never the other slot's)
/// by any gun the carry rule allows beside the other slot's: with
/// [`CarryRule::OneWeapon`] only a gun held in slot 7 with slot 10 empty,
/// with [`CarryRule::OneDoubleHanded`] no double-handed gun beside another.
/// `incoming` gives the drop's gun's handedness (asked only when the other
/// slot holds a double-handed gun).
pub fn decide(picker: Picker, target: Place, drop: Drop,
              incoming: impl FnOnce(Kit, WeaponSlot) -> Handedness) -> Result<Take, Refusal> {
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
    let (kit, slot) = drop.available().ok_or(Refusal::NotAvailable)?;
    match picker.rule {
        CarryRule::OneWeapon if own == WeaponSlot::Second || picker.other.is_some() => {
            return Err(Refusal::SecondWeaponDisabled);
        }
        CarryRule::OneDoubleHanded
            if picker.other == Some(Handedness::Two) && incoming(kit, slot) == Handedness::Two => {
            return Err(Refusal::TwoDoubleHanded);
        }
        _ => {}
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

/// An ammunition's config as the game counts it in ammo bags (its
/// `parameters/clip_size` and `parameters/required_bags`, both above 0):
/// a clip of `clip_size` rounds takes `required_bags` bags (the lobby's
/// clips a bag is `1 / required_bags`, ItemFactory). Most ammo takes one bag
/// a clip; the machine guns' belts two (the PKP's 7N13: 100 rounds, 2 bags).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmmoBags {
    clip_size: u32,
    required_bags: u32,
}

impl AmmoBags {
    /// None for a config the bag count cannot use (no clip, no bags).
    pub const fn parse(clip_size: u32, required_bags: u32) -> Option<AmmoBags> {
        if clip_size == 0 || required_bags == 0 {
            return None;
        }
        Some(AmmoBags { clip_size, required_bags })
    }

    pub const fn clip_size(self) -> u32 {
        self.clip_size
    }

    pub const fn required_bags(self) -> u32 {
        self.required_bags
    }

    /// The bags `rounds` of it take: the lobby's count of a slot
    /// (AmmoShareElement.getCount, `ceil(rounds / clip_size / clips_in_bag)`).
    pub const fn bags(self, rounds: u16) -> u32 {
        (rounds as u32 * self.required_bags).div_ceil(self.clip_size)
    }

    /// The most rounds of it `bags` bags carry: whole clips, `bags div
    /// required_bags` of them (the lobby's fill of an ammo slot, 0x28A035:
    /// div required_bags, imul clip_size; AmmoShareElement's max
    /// `int(int(bags * clips_in_bag) * clip_size)`).
    pub const fn rounds(self, bags: u32) -> u32 {
        bags / self.required_bags * self.clip_size
    }
}

/// What a crate's use fills a borrowed stack to, with its inputs (logged).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refill {
    /// The bags the picker's kit gives the slot (0: the kit names no ammo
    /// with an allowance there, and the native refill skips the slot).
    pub bags: u32,
    /// The rounds of the borrowed ammunition those bags carry, its gun's
    /// magazine and chamber included.
    pub total: u16,
    /// The stack's count after the refill.
    pub target: u16,
}

/// The crate's refill of a borrowed stack, for free (the owner: the game's
/// own refill rule). The native refill (resupply_from_profile 0x2FDC10)
/// fills each ammunition slot to its kit's allowance (the profile's rounds
/// for the slot, which the lobby set as whole clips of the bags it gave the
/// slot), counting the magazine and chamber of the gun whose selected stack
/// it is. A borrowed stack is in a slot whose allowance is rounds of the
/// picker's own ammunition (`own`): it gets the rounds of the borrowed
/// ammunition the same bags carry, its gun's `magazine` (and chamber, when
/// it is the gun's selected stack) inside them; never below what it has.
/// `slot` is None when the kit names no ammunition with an allowance there.
pub fn crate_refill(slot: Option<(u16, AmmoBags)>, borrowed: AmmoBags, stack: u16,
                    magazine: Option<(u8, bool)>) -> Refill {
    let bags = slot.map_or(0, |(allowance, own)| own.bags(allowance));
    let total = u16::try_from(borrowed.rounds(bags)).unwrap_or(u16::MAX);
    let held = magazine.map_or(0, |(loaded, chamber)| u16::from(loaded) + u16::from(chamber));
    Refill { bags, total, target: stack.max(total.saturating_sub(held)) }
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
    fn a_drop_is_open_until_it_ends() {
        let drop = Drop::Open { kit: Kit::new(1).unwrap(), slot: WeaponSlot::Second };
        assert_eq!(drop.available(), Some((Kit::new(1).unwrap(), WeaponSlot::Second)));
        assert_eq!(Drop::Taken.available(), None);
        assert_eq!(Drop::None.available(), None);
        assert_eq!(Ms(5).since(Ms(u32::MAX - 4)), 10, "time wraps");
    }

    #[test]
    fn records_round_trip_and_refuse_anything_else() {
        let borrow = Borrow { donor: Place::new(19).unwrap(), kit: Kit::new(2).unwrap(), slot: WeaponSlot::Second,
                              own: WeaponSlot::First };
        let cases = [
            Record::EMPTY,
            Record { drop: Drop::Taken, borrow: Some(borrow) },
            Record { drop: Drop::Open { kit: Kit::new(0).unwrap(), slot: WeaponSlot::First }, borrow: None },
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
        let mut padded = cases[2].encode();
        padded[5] = 1;
        assert_eq!(Record::decode(padded), None, "an open drop's padding");
    }

    #[test]
    fn a_pickup_takes_an_available_drop_once() {
        let me = Place::new(1).unwrap();
        let picker = Picker { place: me, alive: true, held: Some(WeaponSlot::Second), other: None,
                              rule: CarryRule::OneDoubleHanded, borrow: None };
        let donor = Place::new(4).unwrap();
        let kit = Kit::new(2).unwrap();
        let drop = Drop::Open { kit, slot: WeaponSlot::First };
        let rifle = |_, _| Handedness::Two;
        let take = decide(picker, donor, drop, rifle).unwrap();
        assert_eq!(take.borrow, Borrow { donor, kit, slot: WeaponSlot::First, own: WeaponSlot::Second });
        assert_eq!(decide(picker, donor, Drop::Taken, rifle), Err(Refusal::NotAvailable));
        assert_eq!(decide(picker, donor, Drop::None, rifle), Err(Refusal::NotAvailable), "its body gone");
        assert_eq!(decide(picker, me, drop, rifle), Err(Refusal::NotAvailable));
        assert_eq!(decide(Picker { alive: false, ..picker }, donor, drop, rifle), Err(Refusal::Dead));
        assert_eq!(decide(Picker { held: None, ..picker }, donor, drop, rifle), Err(Refusal::NoGunInHands));
        assert_eq!(decide(Picker { borrow: Some(take.borrow), ..picker }, donor, drop, rifle),
                   Err(Refusal::Borrowing));
    }

    #[test]
    fn the_held_gun_is_replaced_as_the_carry_rules_allow() {
        let picker = Picker { place: Place::new(1).unwrap(), alive: true, held: Some(WeaponSlot::Second),
                              other: Some(Handedness::One), rule: CarryRule::OneDoubleHanded, borrow: None };
        let (donor, kit) = (Place::new(4).unwrap(), Kit::new(0).unwrap());
        let drop = Drop::Open { kit, slot: WeaponSlot::First };
        let (rifle, pistol) = (|_, _| Handedness::Two, |_, _| Handedness::One);
        // Any gun for any gun: a rifle for the held pistol beside a pistol.
        assert_eq!(decide(picker, donor, drop, rifle).unwrap().borrow.own, WeaponSlot::Second);
        // Beside a rifle: a pistol yes, a rifle no, unless modifier 25.
        let beside = Picker { other: Some(Handedness::Two), ..picker };
        assert!(decide(beside, donor, drop, pistol).is_ok());
        assert_eq!(decide(beside, donor, drop, rifle), Err(Refusal::TwoDoubleHanded));
        assert!(decide(Picker { rule: CarryRule::TwoDoubleHanded, ..beside }, donor, drop, rifle).is_ok());
        // Modifier 64: slot 7 alone.
        let one = Picker { rule: CarryRule::OneWeapon, other: None, ..picker };
        assert_eq!(decide(one, donor, drop, pistol), Err(Refusal::SecondWeaponDisabled));
        assert!(decide(Picker { held: Some(WeaponSlot::First), ..one }, donor, drop, rifle).is_ok());
        assert_eq!(decide(Picker { held: Some(WeaponSlot::First), other: Some(Handedness::One), ..one }, donor,
                          drop, pistol), Err(Refusal::SecondWeaponDisabled));
        assert_eq!(CarryRule::from_modifiers(0.0, 0.0), CarryRule::OneDoubleHanded);
        assert_eq!(CarryRule::from_modifiers(0.0, 1.0), CarryRule::TwoDoubleHanded);
        assert_eq!(CarryRule::from_modifiers(1.0, 1.0), CarryRule::OneWeapon);
        assert_eq!(CarryRule::from_modifiers(f32::NAN, f32::NAN), CarryRule::OneDoubleHanded);
    }

    #[test]
    fn the_ammunition_granted_and_refilled() {
        assert_eq!(Ammo::grant(30, 12, true), Ammo { loaded: 12, chamber: true, reserve: 30 });
        assert_eq!(Ammo::grant(8, 9, false).loaded, 8);
    }

    #[test]
    fn the_crate_refills_a_borrowed_stack_as_the_game_fills_its_bags() {
        // The configs of 0.26g and the rev 40 content: 5.45x39 7N10 (30 a
        // clip, a bag), 7.62x54R (10, a bag), 9x18 (50, a bag), the PKP's
        // 7.62x54 7N13 (100, two bags).
        let (ak, mosin, pm) = (AmmoBags::parse(30, 1).unwrap(), AmmoBags::parse(10, 1).unwrap(),
                               AmmoBags::parse(50, 1).unwrap());
        let pkp = AmmoBags::parse(100, 2).unwrap();
        assert_eq!((AmmoBags::parse(0, 1), AmmoBags::parse(30, 0)), (None, None));
        assert_eq!((ak.bags(90), ak.bags(100), ak.bags(0), pkp.bags(100), pkp.bags(101)), (3, 4, 0, 2, 3));
        assert_eq!((pkp.rounds(3), pkp.rounds(4), pkp.rounds(1), ak.rounds(3)), (100, 200, 0, 90));
        // An AK's three bags (90 rounds) for a PKP: one belt, its 40 in the
        // magazine inside it.
        assert_eq!(crate_refill(Some((90, ak)), pkp, 0, Some((40, false))),
                   Refill { bags: 3, total: 100, target: 60 });
        // The same ammunition: the own gun's own refill (90 less 25 and the chamber).
        assert_eq!(crate_refill(Some((90, ak)), ak, 10, Some((25, true))),
                   Refill { bags: 3, total: 90, target: 64 });
        assert_eq!(crate_refill(Some((40, mosin)), pkp, 30, None), Refill { bags: 4, total: 200, target: 200 },
                   "a stack not the selected one: no magazine inside");
        assert_eq!(crate_refill(Some((50, pm)), pkp, 7, Some((0, false))), Refill { bags: 1, total: 0, target: 7 },
                   "one bag carries no belt");
        assert_eq!(crate_refill(Some((90, ak)), ak, 95, Some((30, false))).target, 95, "never below what it has");
        assert_eq!(crate_refill(None, ak, 5, None), Refill { bags: 0, total: 0, target: 5 }, "no allowance");
    }
}

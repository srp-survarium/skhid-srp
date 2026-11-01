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
//!   taken) and, for each of its two weapon slots, the gun picked into it
//!   (its own gun then hidden);
//! - the rule of a pickup ([`decide`]): the gun in hands replaced by any
//!   gun, as the carry rules after the skill modifiers allow ([`CarryRule`]).
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
/// [`Drop::None`] again. Its gun is the one in that weapon slot
/// ([`Record::drop_recipe`]): the gun picked into it if one was, else the
/// slot's own gun of `kit`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drop {
    /// No drop: not dead holding a gun of its two, or its body gone.
    None,
    /// Died with `kit` holding `slot`'s gun.
    Open { kit: Kit, slot: WeaponSlot },
    /// That gun was taken (once only); a cancelled pickup opens it again
    /// as it was.
    Taken { kit: Kit, slot: WeaponSlot },
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

    /// An open drop taken; anything else as it is.
    pub fn taken(self) -> Drop {
        match self {
            Drop::Open { kit, slot } => Drop::Taken { kit, slot },
            other => other,
        }
    }

    /// A taken drop open again as it was; anything else as it is.
    pub fn reopened(self) -> Drop {
        match self {
            Drop::Taken { kit, slot } => Drop::Open { kit, slot },
            other => other,
        }
    }
}

/// A picked gun's config: the profile kit slot its gun was cooked from.
/// Read only to cook it anew (a client without it: a late join, a resync),
/// never to find a gun.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Recipe {
    pub place: Place,
    pub kit: Kit,
    pub slot: WeaponSlot,
}

/// The gun picked into a weapon slot (the slot's own gun hidden meanwhile):
/// its pickup's tick, which tells two pickups apart, and its recipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pick {
    pub taken: Ms,
    pub recipe: Recipe,
}

/// A pickup waiting for its weapon switch: the gun of `pick` is the
/// picker's pending object (its own registry id), the gun in hands of
/// `slot` holstering; the game's own switch puts it into `slot`. `from` is
/// the place whose drop it was taken from (a picked gun's drop is its last
/// holder's, not its recipe's place): a cancel opens that drop again. The
/// donor's magazine and chamber it gets then (None: a full magazine, the
/// donor's gun not there at the command) are fixed at the command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    pub slot: WeaponSlot,
    pub from: Place,
    pub pick: Pick,
    pub magazine: Option<(u8, bool)>,
}

/// A weapon's random state (weapon+5BCh, the spread's minstd_rand) as the
/// spawn hook sets a spawned kit's (patch_026g.py `weapon_random_hook`,
/// client_patch.md 3.18): of the place, the kit, the inventory slot and the
/// time, in 1..2^31-2. A picked gun takes it at its switch (the switch's
/// time), so a re-used copy and a fresh one start alike in every game.
pub const fn spawn_seed(place: u8, kit: u8, slot: u8, time: u32) -> u32 {
    let key = ((place as u32) << 16 | (kit as u32) << 8 | slot as u32).wrapping_mul(0x85EB_CA6B);
    let h = key ^ time.wrapping_mul(0x9E37_79B1);
    (h ^ h >> 15) % 0x7FFF_FFFE + 1
}

/// The ammunition slots a weapon binds (cook 0x2C7855..0x2C79E5): where an
/// own stack can be parked from.
pub const AMMO_SLOTS: [u8; 8] = [8, 9, 11, 12, 19, 20, 21, 22];

/// The own gun's wear a pick hides (its condition, 0..1, as the f32 the
/// gun keeps at weapon+270h): the world carried it until the swap, so
/// every game read the same at the swap; the pick's end gives it back, and
/// remove(1)'s unload adds it to the profile item (0x2FD375).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wear(u32);

impl Wear {
    pub fn of(condition: f32) -> Wear {
        Wear(condition.to_bits())
    }

    pub fn condition(self) -> f32 {
        f32::from_bits(self.0)
    }
}

/// A weapon slot's pick: the gun picked into it and the own gun's wear it
/// hides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held {
    pub pick: Pick,
    pub own: Wear,
}

/// The one record a player's state carries: its drop, what is picked
/// into each of its weapon slots (7, 10) with the own gun's wear it hides,
/// and a pickup waiting for its switch. Nothing else of the own kit a pick
/// sets aside: its ammunition is neither charged nor refunded (the owner,
/// 2026-11-02), as the kit gives it in every game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub drop: Drop,
    pub picks: [Option<Held>; 2],
    pub pending: Option<Pending>,
}

impl Record {
    pub const EMPTY: Record = Record { drop: Drop::None, picks: [None; 2], pending: None };
    /// Its bytes in a player's state: the version, the drop (its tag, kit
    /// and slot in one byte, a taken drop's too), then for slot 7 and
    /// slot 10 a byte (0 none, else the recipe's place + 1, its kit << 5,
    /// its slot << 7) and the tick (u32); then the pending pickup: its
    /// weapon slot (0 none, 1 slot 7, 2 slot 10) with the place it was taken
    /// from << 2, its pick as a slot's, the magazine and a byte (bit 0 the
    /// donor's magazine known, bit 1 its chamber); then for slot 7 and slot
    /// 10 the own gun's wear its pick hides (the f32's bits, 0 without a
    /// pick). Version 7 (6 had no kit and slot in a taken drop nor a pending
    /// pickup's place; 5 had no wear; 4 had the own kit's counts, 28 bytes
    /// more).
    pub const BYTES: usize = 28;
    const VERSION: u8 = 7;

    pub fn pick(&self, slot: WeaponSlot) -> Option<Pick> {
        self.held(slot).map(|h| h.pick)
    }

    pub fn held(&self, slot: WeaponSlot) -> Option<Held> {
        self.picks[usize::from(slot.bit())]
    }

    pub fn set_pick(&mut self, slot: WeaponSlot, held: Option<Held>) {
        self.picks[usize::from(slot.bit())] = held;
    }

    /// The gun `place`'s open drop is (None: no open drop): the gun picked
    /// into the drop's weapon slot if one was (the owner, 2026-11-02: a
    /// player who dies holding a picked gun drops that gun; the record keeps
    /// the pick until base_player::remove(1) ends the drop), else that
    /// slot's own gun of the drop's kit.
    pub fn drop_recipe(&self, place: Place) -> Option<Recipe> {
        let (kit, slot) = self.drop.available()?;
        Some(self.pick(slot).map_or(Recipe { place, kit, slot }, |pick| pick.recipe))
    }

    fn encode_pick(out: &mut [u8], pick: Pick) {
        let r = pick.recipe;
        out[0] = (r.place.0 + 1) | r.kit.0 << 5 | r.slot.bit() << 7;
        out[1..5].copy_from_slice(&pick.taken.0.to_le_bytes());
    }

    /// A pick's 5 bytes ([`encode_pick`](Self::encode_pick)): None for a
    /// zero byte with a zero tick, refused (outer None) for anything else
    /// out of range.
    fn decode_pick(bytes: &[u8]) -> Option<Option<Pick>> {
        let taken = Ms(u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]));
        match bytes[0] {
            0 => (taken.0 == 0).then_some(None),
            b => Some(Some(Pick { taken, recipe: Recipe {
                place: Place::new((b & 0x1F).checked_sub(1)?)?,
                kit: Kit::new(b >> 5 & 3)?,
                slot: WeaponSlot::from_bit(b >> 7),
            } })),
        }
    }

    pub fn encode(self) -> [u8; Self::BYTES] {
        let mut out = [0u8; Self::BYTES];
        out[0] = Self::VERSION;
        out[1] = match self.drop {
            Drop::None => 0,
            Drop::Open { kit, slot } => 1 | kit.0 << 2 | slot.bit() << 4,
            Drop::Taken { kit, slot } => 2 | kit.0 << 2 | slot.bit() << 4,
        };
        for ((at, wear), held) in [2, 7].into_iter().zip([20, 24]).zip(self.picks) {
            if let Some(h) = held {
                Self::encode_pick(&mut out[at..at + 5], h.pick);
                out[wear..wear + 4].copy_from_slice(&h.own.0.to_le_bytes());
            }
        }
        if let Some(p) = self.pending {
            out[12] = (1 + p.slot.bit()) | p.from.0 << 2;
            Self::encode_pick(&mut out[13..18], p.pick);
            if let Some((loaded, chamber)) = p.magazine {
                out[18] = loaded;
                out[19] = 1 | u8::from(chamber) << 1;
            }
        }
        out
    }

    /// Parses the bytes [`encode`](Self::encode) writes; anything else is
    /// refused (None): an unknown version or tag, a value out of range, or
    /// a tick without a pick.
    pub fn decode(bytes: [u8; Self::BYTES]) -> Option<Record> {
        if bytes[0] != Self::VERSION {
            return None;
        }
        let drop = match (bytes[1] & 3, bytes[1] >> 2) {
            (0, 0) => Drop::None,
            (1, rest) if rest < 8 => Drop::Open { kit: Kit::new(rest & 3)?, slot: WeaponSlot::from_bit(rest >> 2) },
            (2, rest) if rest < 8 => Drop::Taken { kit: Kit::new(rest & 3)?, slot: WeaponSlot::from_bit(rest >> 2) },
            _ => return None,
        };
        let pending = match (bytes[12] & 3, bytes[12] >> 2, Self::decode_pick(&bytes[13..18])?, bytes[18], bytes[19]) {
            (0, 0, None, 0, 0) => None,
            (tag @ 1..=2, from, Some(pick), loaded, flags) => Some(Pending {
                slot: WeaponSlot::from_bit(tag - 1),
                from: Place::new(from)?,
                pick,
                magazine: match flags {
                    0 if loaded == 0 => None,
                    1 | 3 => Some((loaded, flags == 3)),
                    _ => return None,
                },
            }),
            _ => return None,
        };
        let held = |at: usize, wear: usize| -> Option<Option<Held>> {
            let own = u32::from_le_bytes([bytes[wear], bytes[wear + 1], bytes[wear + 2], bytes[wear + 3]]);
            match Self::decode_pick(&bytes[at..at + 5])? {
                Some(pick) => Some(Some(Held { pick, own: Wear(own) })),
                None => (own == 0).then_some(None),
            }
        };
        let picks = [held(2, 20)?, held(7, 24)?];
        Some(Record { drop, picks, pending })
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

/// What a picker has in its hands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hands {
    /// The gun of this weapon slot, settled.
    Gun(WeaponSlot),
    /// A switch under way (the held object is not the one to come).
    Switching,
    /// Nothing, or no gun of its two (a battery, an artefact, a grenade, a medkit).
    NoGun,
}

/// What the picker is at the tick its command is played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picker {
    pub place: Place,
    pub alive: bool,
    pub hands: Hands,
    /// The gun in its other weapon slot, if any.
    pub other: Option<Handedness>,
    pub rule: CarryRule,
}

/// Why a pickup command does nothing (every client refuses it alike).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Dead,
    /// No gun of its two in its hands.
    NoGunInHands,
    /// A weapon switch under way.
    Switching,
    /// Its own place, or a drop not available (none, taken, its body gone).
    NotAvailable,
    /// `disable_second_weapon`: a gun held in slot 10, or one beside it.
    SecondWeaponDisabled,
    /// A double-handed gun beside another, without
    /// `use_two_double_handed_weapons`.
    TwoDoubleHanded,
}

/// The rule of a pickup command (the owner's, `weapon_pickup.md`): a living
/// picker with a gun of its two weapon slots in hands (its own or a picked
/// one), an available drop of another place; the slot in hands takes the
/// drop's gun (never the other slot), which the result names, as the carry
/// rule allows beside the other slot's gun: with
/// [`CarryRule::OneWeapon`] only a gun held in slot 7 with slot 10 empty,
/// with [`CarryRule::OneDoubleHanded`] no double-handed gun beside another.
/// `incoming` gives the drop's gun's handedness (asked only when the other
/// slot holds a double-handed gun).
pub fn decide(picker: Picker, target: Place, drop: Drop,
              incoming: impl FnOnce(Kit, WeaponSlot) -> Handedness) -> Result<WeaponSlot, Refusal> {
    if !picker.alive {
        return Err(Refusal::Dead);
    }
    let own = match picker.hands {
        Hands::Gun(slot) => slot,
        Hands::Switching => return Err(Refusal::Switching),
        Hands::NoGun => return Err(Refusal::NoGunInHands),
    };
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
    Ok(own)
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

    /// The bags whose whole clips `rounds` of it hold, rounded down: the
    /// lobby makes a slot's rounds whole clips of the bags it gives it
    /// ([`rounds`](Self::rounds)), so this is those bags, and a stray round
    /// above them gives no bag more.
    pub const fn bags(self, rounds: u16) -> u32 {
        rounds as u32 * self.required_bags / self.clip_size
    }

    /// The most rounds of it `bags` bags carry: whole clips, `bags div
    /// required_bags` of them (the lobby's fill of an ammo slot, 0x28A035:
    /// div required_bags, imul clip_size; AmmoShareElement's max
    /// `int(int(bags * clips_in_bag) * clip_size)`).
    pub const fn rounds(self, bags: u32) -> u32 {
        bags / self.required_bags * self.clip_size
    }
}

/// What a crate's use fills a borrowed gun's loaded stack to, with its
/// inputs (logged).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refill {
    /// The bags the picker's kit gives the weapon slot.
    pub bags: u32,
    /// The rounds of the borrowed ammunition those bags carry, its gun's
    /// magazine and chamber included.
    pub total: u16,
    /// The stack's count after the refill.
    pub target: u16,
}

/// The crate's refill of a borrowed gun, for free, by the game's own rule
/// (the owner, 2026-11-01; no minimum: the extra magazine is the pickup's
/// alone). The lobby gives a weapon its ammunition as whole clips of the
/// ammo bags it gives that weapon (process_default_relocate_item 0x288D60:
/// `(bags div required_bags) x clip_size` of one ammunition item, into one
/// slot; the kit's bags `ammo_bags_count` and modifier 16, split 70/30 between
/// slot 7's and slot 10's weapon when both have ammunition, or the player's
/// own split in the ammo window), and the crate (resupply_from_profile
/// 0x2FDC10) fills that slot to it, the magazine and chamber of the weapon
/// inside. The borrowed gun gets the same: `bags`, the bags of its weapon
/// slot, in whole clips of its loaded ammunition (`loaded`), its
/// `magazine` and chamber inside them; its loaded stack (`stack` rounds) is
/// filled to that, never lowered. Its other stacks are not refilled.
pub fn crate_refill(bags: u32, loaded: AmmoBags, stack: u16, magazine: (u8, bool)) -> Refill {
    let total = u16::try_from(loaded.rounds(bags)).unwrap_or(u16::MAX);
    let held = u16::from(magazine.0) + u16::from(magazine.1);
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
    fn the_seed_is_the_spawn_hooks() {
        // late_join_026g.asm's weapon_random_hook: place 3, kit 0, slot 10 at 14 350 ms.
        let key = ((3u32 << 16) | 10).wrapping_mul(0x85EB_CA6B);
        let h = key ^ 14_350u32.wrapping_mul(0x9E37_79B1);
        assert_eq!(spawn_seed(3, 0, 10, 14_350), (h ^ (h >> 15)) % 0x7FFF_FFFE + 1);
        for time in [0, 50, u32::MAX] {
            let seed = spawn_seed(19, 2, 7, time);
            assert!((1..=0x7FFF_FFFE).contains(&seed), "never 0 nor 2^31-1");
        }
    }

    #[test]
    fn a_drop_is_open_until_it_ends() {
        let drop = Drop::Open { kit: Kit::new(1).unwrap(), slot: WeaponSlot::Second };
        assert_eq!(drop.available(), Some((Kit::new(1).unwrap(), WeaponSlot::Second)));
        assert_eq!(drop.taken(), Drop::Taken { kit: Kit::new(1).unwrap(), slot: WeaponSlot::Second });
        assert_eq!(drop.taken().available(), None);
        assert_eq!(drop.taken().reopened(), drop, "a cancelled pickup opens it as it was");
        assert_eq!(Drop::None.taken(), Drop::None);
        assert_eq!(Drop::None.reopened(), Drop::None);
        assert_eq!(Drop::None.available(), None);
        assert_eq!(Ms(5).since(Ms(u32::MAX - 4)), 10, "time wraps");
    }

    #[test]
    fn a_dead_players_drop_is_the_gun_in_its_hands() {
        let (me, donor) = (Place::new(3).unwrap(), Place::new(5).unwrap());
        let (own_kit, donor_kit) = (Kit::new(0).unwrap(), Kit::new(1).unwrap());
        let picked = Recipe { place: donor, kit: donor_kit, slot: WeaponSlot::Second };
        let open = |slot| Record { drop: Drop::Open { kit: own_kit, slot },
                                   picks: [None, Some(Held { pick: Pick { taken: Ms(9), recipe: picked }, own: Wear::of(1.0) })],
                                   pending: None };
        assert_eq!(open(WeaponSlot::Second).drop_recipe(me), Some(picked), "the gun picked into the slot held");
        assert_eq!(open(WeaponSlot::First).drop_recipe(me), Some(Recipe { place: me, kit: own_kit, slot: WeaponSlot::First }),
                   "the other slot's own gun");
        let picked_into_7 = Record { picks: [Some(Held { pick: Pick { taken: Ms(9), recipe: picked }, own: Wear::of(1.0) }), None],
                                     ..open(WeaponSlot::First) };
        assert_eq!(picked_into_7.drop_recipe(me), Some(picked), "a slot-10 gun picked into slot 7: cooked in slot 10");
        let taken = Record { drop: open(WeaponSlot::Second).drop.taken(), ..open(WeaponSlot::Second) };
        assert_eq!(taken.drop_recipe(me), None);
        assert_eq!(Record { drop: taken.drop.reopened(), ..taken }.drop_recipe(me), Some(picked));
        assert_eq!(Record::EMPTY.drop_recipe(me), None);
    }

    #[test]
    fn records_round_trip_and_refuse_anything_else() {
        let recipe = Recipe { place: Place::new(19).unwrap(), kit: Kit::new(2).unwrap(), slot: WeaponSlot::Second };
        let pick = Pick { taken: Ms(78_112), recipe };
        let waiting = |slot, magazine| Some(Pending { slot, from: Place::new(7).unwrap(), pick: Pick { taken: Ms(90_001), recipe },
                                                      magazine });
        let cases = [
            Record::EMPTY,
            Record { drop: Drop::Taken { kit: Kit::new(1).unwrap(), slot: WeaponSlot::Second }, picks: [Some(Held { pick, own: Wear::of(0.9) }), None], pending: None },
            Record { drop: Drop::Open { kit: Kit::new(2).unwrap(), slot: WeaponSlot::Second }, picks: [None, Some(Held { pick, own: Wear::of(1.0) })],
                     pending: None },
            Record { drop: Drop::None, picks: [Some(Held { pick, own: Wear::of(0.5) }), Some(Held { pick: Pick { taken: Ms(1), recipe }, own: Wear::of(0.0) })], pending: None },
            Record { drop: Drop::None, picks: [None, Some(Held { pick, own: Wear::of(1.0) })], pending: waiting(WeaponSlot::Second, Some((17, true))) },
            Record { drop: Drop::Taken { kit: Kit::new(0).unwrap(), slot: WeaponSlot::First }, picks: [None; 2], pending: waiting(WeaponSlot::First, None) },
            Record { drop: Drop::Taken { kit: Kit::new(2).unwrap(), slot: WeaponSlot::First }, picks: [None; 2], pending: waiting(WeaponSlot::First, Some((0, false))) },
        ];
        for record in cases {
            assert_eq!(Record::decode(record.encode()), Some(record));
        }
        assert_eq!(cases[1].encode()[1], 2 | 1 << 2 | 1 << 4, "taken, its kit and slot");
        assert_eq!(cases[1].encode()[2], 20 | 2 << 5 | 1 << 7, "place + 1, kit, slot");
        assert_eq!(cases[4].encode()[12..20], [2 | 7 << 2, 20 | 2 << 5 | 1 << 7, 0x91, 0x5F, 1, 0, 17, 3],
                   "slot 10 from place 7, the pick, 17 + chamber");
        assert_eq!(Record::BYTES, 28);
        assert_eq!(cases[1].encode()[20..28], [0x66, 0x66, 0x66, 0x3F, 0, 0, 0, 0], "slot 7's own wear 0.9, slot 10 none");
        let mut old = cases[1].encode();
        old[0] = 6;
        assert_eq!(Record::decode(old), None, "version 6 (no kit and slot in a taken drop) is not read");
        let mut stray = cases[1].encode();
        stray[24] = 1;
        assert_eq!(Record::decode(stray), None, "a wear without its pick");
        let good = cases[1].encode();
        for (at, value) in [(0, 4), (1, 3), (1, 2 | 8 << 2), (1, 1 | 3 << 2), (1, 2 | 3 << 2), (2, 0x60), (2, 21), (7, 0x60), (8, 1),
                            (12, 1), (13, 1), (18, 5), (19, 1)] {
            let mut bad = good;
            bad[at] = value;
            assert_eq!(Record::decode(bad), None, "byte {at} = {value}");
        }
        let pending = cases[4].encode();
        for (at, value) in [(12, 0), (12, 3), (12, 2 | 20 << 2), (12, 7 << 2), (13, 0), (19, 2), (19, 4)] {
            let mut bad = pending;
            bad[at] = value;
            assert_eq!(Record::decode(bad), None, "pending byte {at} = {value}");
        }
    }

    #[test]
    fn a_pickup_takes_an_available_drop_once() {
        let me = Place::new(1).unwrap();
        let picker = Picker { place: me, alive: true, hands: Hands::Gun(WeaponSlot::Second), other: None,
                              rule: CarryRule::OneDoubleHanded };
        let donor = Place::new(4).unwrap();
        let kit = Kit::new(2).unwrap();
        let drop = Drop::Open { kit, slot: WeaponSlot::First };
        let rifle = |_, _| Handedness::Two;
        assert_eq!(decide(picker, donor, drop, rifle), Ok(WeaponSlot::Second), "the slot in hands");
        assert_eq!(decide(picker, donor, drop.taken(), rifle), Err(Refusal::NotAvailable));
        assert_eq!(decide(picker, donor, Drop::None, rifle), Err(Refusal::NotAvailable), "its body gone");
        assert_eq!(decide(picker, me, drop, rifle), Err(Refusal::NotAvailable));
        assert_eq!(decide(Picker { alive: false, ..picker }, donor, drop, rifle), Err(Refusal::Dead));
        assert_eq!(decide(Picker { hands: Hands::NoGun, ..picker }, donor, drop, rifle), Err(Refusal::NoGunInHands));
        assert_eq!(decide(Picker { hands: Hands::Switching, ..picker }, donor, drop, rifle), Err(Refusal::Switching));
    }

    #[test]
    fn the_held_gun_is_replaced_as_the_carry_rules_allow() {
        let picker = Picker { place: Place::new(1).unwrap(), alive: true, hands: Hands::Gun(WeaponSlot::Second),
                              other: Some(Handedness::One), rule: CarryRule::OneDoubleHanded };
        let (donor, kit) = (Place::new(4).unwrap(), Kit::new(0).unwrap());
        let drop = Drop::Open { kit, slot: WeaponSlot::First };
        let (rifle, pistol) = (|_, _| Handedness::Two, |_, _| Handedness::One);
        // Any gun for any gun: a rifle for the held pistol beside a pistol.
        assert_eq!(decide(picker, donor, drop, rifle), Ok(WeaponSlot::Second));
        // Beside a rifle: a pistol yes, a rifle no, unless modifier 25.
        let beside = Picker { other: Some(Handedness::Two), ..picker };
        assert!(decide(beside, donor, drop, pistol).is_ok());
        assert_eq!(decide(beside, donor, drop, rifle), Err(Refusal::TwoDoubleHanded));
        assert!(decide(Picker { rule: CarryRule::TwoDoubleHanded, ..beside }, donor, drop, rifle).is_ok());
        // Modifier 64: slot 7 alone.
        let one = Picker { rule: CarryRule::OneWeapon, other: None, ..picker };
        assert_eq!(decide(one, donor, drop, pistol), Err(Refusal::SecondWeaponDisabled));
        assert!(decide(Picker { hands: Hands::Gun(WeaponSlot::First), ..one }, donor, drop, rifle).is_ok());
        assert_eq!(decide(Picker { hands: Hands::Gun(WeaponSlot::First), other: Some(Handedness::One), ..one }, donor,
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
        // The configs of 0.26g: 5.45x39 7N10 (30 a clip, a bag), the PKP's
        // 7.62x54 7N13 (100, two bags).
        let ak = AmmoBags::parse(30, 1).unwrap();
        let pkp = AmmoBags::parse(100, 2).unwrap();
        assert_eq!((AmmoBags::parse(0, 1), AmmoBags::parse(30, 0)), (None, None));
        assert_eq!((ak.bags(90), ak.bags(91), ak.bags(100), ak.bags(0), pkp.bags(100), pkp.bags(101)),
                   (3, 3, 3, 0, 2, 2), "rounded down: a stray round gives no bag more");
        assert_eq!((pkp.rounds(3), pkp.rounds(4), pkp.rounds(1), ak.rounds(3)), (100, 200, 0, 90));
        // An AK's three bags (90 rounds) for a PKP: one belt, its 40 in the
        // magazine inside it.
        assert_eq!(crate_refill(3, pkp, 0, (40, false)), Refill { bags: 3, total: 100, target: 60 });
        // The same ammunition: the own gun's own refill (90 less 25 and the chamber).
        assert_eq!(crate_refill(3, ak, 10, (25, true)), Refill { bags: 3, total: 90, target: 64 });
        assert_eq!(crate_refill(4, pkp, 30, (0, false)), Refill { bags: 4, total: 200, target: 200 });
        assert_eq!(crate_refill(1, pkp, 7, (0, false)), Refill { bags: 1, total: 0, target: 7 },
                   "one bag carries no belt: no minimum");
        assert_eq!(crate_refill(3, ak, 95, (30, false)).target, 95, "never below what it has");
        // The owner's test: a weapon slot of 2 bags (100 rounds of a 50-round
        // ammunition), a borrowed gun of 30-round clips with 13 in it: 60, 47.
        let ammo_70 = AmmoBags::parse(50, 1).unwrap();
        assert_eq!(ammo_70.bags(100), 2);
        assert_eq!(crate_refill(ammo_70.bags(100), ak, 0, (13, false)), Refill { bags: 2, total: 60, target: 47 });
    }
}

//! Shared rules for temporary, in-match weapon pickup.
//!
//! No engine pointers. Only the selecting client uses `targeting` to locate
//! its cosmetic weapon. Every simulator checks the resulting loadout and
//! applies the same ammunition grant. Native ownership and snapshot adapters
//! are responsible for applying the accepted transaction.

#![no_std]
#![forbid(unsafe_code)]

use core::num::NonZeroU8;

pub mod history;
pub mod input;
pub mod input_history;
mod reserves;
pub mod snapshot;
pub mod state;
pub mod targeting;
pub use reserves::Reserves;
mod original;
pub use original::Original;

/// Both native weapon slots accept either weapon class. Neither means pistol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponSlot {
    First,
    Second,
}

impl WeaponSlot {
    pub fn from_native(slot: u32) -> Option<Self> {
        match slot {
            7 => Some(Self::First),
            10 => Some(Self::Second),
            _ => None,
        }
    }

    pub const fn native(self) -> u32 {
        match self {
            Self::First => 7,
            Self::Second => 10,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::First => 0,
            Self::Second => 1,
        }
    }

    const fn other(self) -> Self {
        match self {
            Self::First => Self::Second,
            Self::Second => Self::First,
        }
    }
}

/// Taken from the item's double_handed property, not a category-name guess.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handedness {
    One,
    Two,
}

/// Effective carrying restrictions, after the native skill modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarryRule {
    /// disable_second_weapon: only native slot 7 may be occupied.
    OneWeapon,
    /// Normal rule: at most one double-handed weapon.
    OneDoubleHanded,
    /// use_two_double_handed_weapons.
    TwoDoubleHanded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    NoHeldWeapon,
    SecondWeaponDisabled,
    TwoDoubleHandedWeapons,
}

/// A current native inventory read, separate from the saved respawn loadout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Loadout {
    slots: [Option<Handedness>; 2],
    held: Option<WeaponSlot>,
    rule: CarryRule,
}

impl Loadout {
    pub const fn new(
        slots: [Option<Handedness>; 2],
        held: Option<WeaponSlot>,
        rule: CarryRule,
    ) -> Self {
        Self { slots, held, rule }
    }

    /// Always replace the held weapon. A refused pickup never chooses a
    /// different slot on the player's behalf, nor bypasses a carrying skill.
    pub fn replacement(self, incoming: Handedness) -> Result<WeaponSlot, Refused> {
        let held = self.held.ok_or(Refused::NoHeldWeapon)?;
        if self.slots[held.index()].is_none() {
            return Err(Refused::NoHeldWeapon);
        }
        let other = self.slots[held.other().index()];
        match self.rule {
            CarryRule::OneWeapon if held == WeaponSlot::Second || other.is_some() => {
                Err(Refused::SecondWeaponDisabled)
            }
            CarryRule::OneDoubleHanded
                if incoming == Handedness::Two && other == Some(Handedness::Two) =>
            {
                Err(Refused::TwoDoubleHandedWeapons)
            }
            _ => Ok(held),
        }
    }
}

/// Ammunition captured at death, before respawn setup can change the weapon.
/// The native engine stores the chamber separately from the magazine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DroppedAmmo {
    capacity: NonZeroU8,
    magazine: u8,
    chamber: Chamber,
    kind: AmmoKind,
}

/// Ordinal in this weapon template's compatible-ammo list. The native kit
/// has eight ammo slots in total. The adapter additionally resolves this
/// ordinal against the template's actual list before touching an inventory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AmmoKind(u8);

impl AmmoKind {
    pub fn parse(raw: u8) -> Option<Self> {
        (raw < 8).then_some(Self(raw))
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chamber {
    None,
    Empty,
    Loaded,
}

impl DroppedAmmo {
    pub fn parse(capacity: u8, magazine: u8, chamber: Chamber, kind: AmmoKind) -> Option<Self> {
        let capacity = NonZeroU8::new(capacity)?;
        (magazine <= capacity.get()).then_some(Self {
            capacity,
            magazine,
            chamber,
            kind,
        })
    }

    pub const fn capacity(self) -> u8 {
        self.capacity.get()
    }

    pub const fn magazine(self) -> u8 {
        self.magazine
    }

    pub const fn chamber(self) -> Chamber {
        self.chamber
    }

    pub const fn kind(self) -> AmmoKind {
        self.kind
    }

    pub const fn loaded_rounds(self) -> u16 {
        self.magazine as u16
            + if matches!(self.chamber, Chamber::Loaded) {
                1
            } else {
                0
            }
    }

    /// The single spare magazine is reserve ammunition. Do not run the
    /// native spawn loader: doing so fills an empty dropped magazine and
    /// silently changes what the dead player left in it.
    pub const fn pickup_reserve(self) -> u16 {
        self.capacity.get() as u16
    }
}

/// A checked replacement with its ammo initialization. Constructing this
/// value performs no inventory mutation; refused requests leave it untouched.
#[derive(Debug, PartialEq, Eq)]
pub struct PickupGrant {
    slot: WeaponSlot,
    ammo: DroppedAmmo,
}

impl PickupGrant {
    pub fn check(
        loadout: Loadout,
        incoming: Handedness,
        ammo: DroppedAmmo,
    ) -> Result<Self, Refused> {
        Ok(Self {
            slot: loadout.replacement(incoming)?,
            ammo,
        })
    }

    pub const fn slot(&self) -> WeaponSlot {
        self.slot
    }

    pub const fn ammo(&self) -> DroppedAmmo {
        self.ammo
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use CarryRule::*;
    use Handedness::{One, Two};
    use WeaponSlot::{First, Second};

    #[test]
    fn a_pistol_is_not_assumed_to_be_in_the_second_slot() {
        for (slots, pistol, rifle) in [
            ([Some(One), Some(Two)], First, Second),
            ([Some(Two), Some(One)], Second, First),
        ] {
            let holding_pistol = Loadout::new(slots, Some(pistol), OneDoubleHanded);
            assert_eq!(
                holding_pistol.replacement(Two),
                Err(Refused::TwoDoubleHandedWeapons)
            );
            assert_eq!(holding_pistol.replacement(One), Ok(pistol));
            let holding_rifle = Loadout::new(slots, Some(rifle), OneDoubleHanded);
            assert_eq!(holding_rifle.replacement(Two), Ok(rifle));
        }
    }

    #[test]
    fn dual_primary_replaces_whichever_weapon_is_held() {
        for held in [First, Second] {
            let loadout = Loadout::new([Some(Two), Some(Two)], Some(held), TwoDoubleHanded);
            assert_eq!(loadout.replacement(Two), Ok(held));
            assert_eq!(loadout.replacement(One), Ok(held));
        }
        let pistol = Loadout::new([Some(Two), Some(One)], Some(Second), TwoDoubleHanded);
        assert_eq!(pistol.replacement(Two), Ok(Second));
    }

    #[test]
    fn disabled_second_slot_cannot_be_enabled_by_pickup() {
        let one = Loadout::new([Some(One), None], Some(First), OneWeapon);
        assert_eq!(one.replacement(Two), Ok(First));
        let second = Loadout::new([None, Some(One)], Some(Second), OneWeapon);
        assert_eq!(second.replacement(Two), Err(Refused::SecondWeaponDisabled));
        let stale = Loadout::new([Some(One), Some(One)], Some(First), OneWeapon);
        assert_eq!(stale.replacement(Two), Err(Refused::SecondWeaponDisabled));
    }

    #[test]
    fn holding_a_battery_or_grenade_does_not_select_an_arbitrary_weapon() {
        let loadout = Loadout::new([Some(Two), Some(One)], None, OneDoubleHanded);
        assert_eq!(loadout.replacement(Two), Err(Refused::NoHeldWeapon));
        assert_eq!(WeaponSlot::from_native(13), None);
        let empty = Loadout::new([Some(Two), None], Some(Second), TwoDoubleHanded);
        assert_eq!(empty.replacement(Two), Err(Refused::NoHeldWeapon));
    }

    #[test]
    fn pickup_preserves_empty_partial_and_full_magazines_and_the_chamber() {
        let loadout = Loadout::new([Some(Two), Some(One)], Some(First), OneDoubleHanded);
        for capacity in [1, 8, 30, 100, 255] {
            for magazine in [0, capacity / 2, capacity] {
                for chamber in [Chamber::None, Chamber::Empty, Chamber::Loaded] {
                    let ammo = DroppedAmmo::parse(
                        capacity,
                        magazine,
                        chamber,
                        AmmoKind::parse(1).unwrap(),
                    )
                    .unwrap();
                    let grant = PickupGrant::check(loadout, Two, ammo).unwrap();
                    assert_eq!(grant.ammo().magazine(), magazine);
                    assert_eq!(grant.ammo().chamber(), chamber);
                    assert_eq!(grant.ammo().kind().get(), 1);
                    assert_eq!(grant.ammo().pickup_reserve(), u16::from(capacity));
                    let chamber_round = u16::from(chamber == Chamber::Loaded);
                    assert_eq!(
                        grant.ammo().loaded_rounds(),
                        u16::from(magazine) + chamber_round
                    );
                }
            }
        }
        assert!(DroppedAmmo::parse(0, 0, Chamber::None, AmmoKind::parse(0).unwrap()).is_none());
        assert!(DroppedAmmo::parse(30, 31, Chamber::Loaded, AmmoKind::parse(0).unwrap()).is_none());
    }
}

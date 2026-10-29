//! The client's bullet penetration (0.26g), for the bots and the bench: what a surface's material
//! stops of a bullet and what the bullet keeps after it.
//!
//! The rule is read from the exe (`bullet::try_pierce` 0x32D960; the material loader 0x323A7B
//! stores the key `armor` of `game.materials` at material+0x4C, which the bullet reads as the
//! resistance). PROVEN. For a bullet of pierce `P` meeting a front face of armor `R`:
//!
//! ```text
//! f = clamp((P - R) / R, 0, 1)        (armor 0 is free: f = 1)
//! k = 1 - (1 - f)^2                   (0 when P <= R: the bullet stops)
//! damage = damage * k,  P = P * k     and it flies on
//! ```
//!
//! Only front faces count. A bullet's pierce is its weapon's `bullet_pierce` times the ammo's
//! `k_arp` (`weapon_ammunition.cpp`: `m_pierce = k_arp`) at the muzzle. In flight both the bullet's
//! damage (+0x70) and its pierce (+0x74) are set to `min(current, curve(speed))` at every step
//! (`update_bullet_position` 0x32F044..0x32F174, the same factor as `bullet_at`), and `try_pierce`
//! reads +0x74: so a wall meets the pierce at its own distance, and what a crossing took off stays
//! off (`min`, not a product). PROVEN.

/// What a material resists a bullet with: `physic.armor` of `game.materials`. Never negative, NaN
/// or infinite. Glass, cloth, plastic, chain-link and foliage are [`Self::FREE`]; stone is 0.7, the
/// ground 20.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Armor(f32);

impl Armor {
    /// Armor 0: every bullet goes through at full damage.
    pub const FREE: Self = Self(0.0);

    /// `None` for a value that is not an armor (negative, NaN or infinite).
    pub fn new(value: f32) -> Option<Self> {
        (value.is_finite() && value >= 0.0).then_some(Self(value))
    }

    pub fn value(self) -> f32 {
        self.0
    }

    /// What a bullet of `pierce` has after crossing one front face of this armor
    /// ([`survarium::v026g::damage::wall_share`]: the one formula for bullets and blasts).
    pub fn crossed_by(self, pierce: f32) -> Crossing {
        let k = survarium::v026g::damage::wall_share(pierce, self.0);
        if k > 0.0 {
            Crossing::Through { k }
        } else {
            Crossing::Stopped
        }
    }
}

/// The one bit pattern a file or a tree stores for [`FaceArmor::Unknown`] (the quiet NaN); any other NaN is damage.
const UNKNOWN_WIRE: u32 = 0x7fc0_0000;

/// What a bullet-mesh face is made of, as far as a bullet is concerned.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum FaceArmor {
    /// The face's material is not known (no settings entry, no `armor`, or a file before format 5): it stops every
    /// bullet from either side, as every face of the mesh did before there were materials.
    #[default]
    Unknown,
    /// A material: stops or lets through by its armor, on its front face only.
    Material(Armor),
}

impl FaceArmor {
    pub const FREE: Self = Self::Material(Armor::FREE);

    /// The face as a file or a tree stores it: one `f32`, NaN for [`Self::Unknown`]. `None` for a number that
    /// is no armor (negative or infinite).
    pub fn from_wire(value: f32) -> Option<Self> {
        if value.to_bits() == UNKNOWN_WIRE {
            Some(Self::Unknown)
        } else {
            Armor::new(value).map(Self::Material)
        }
    }

    /// The `f32` a file or a tree stores ([`Self::from_wire`]); the one NaN there is.
    pub fn to_wire(self) -> f32 {
        match self {
            Self::Unknown => f32::from_bits(UNKNOWN_WIRE),
            Self::Material(armor) => armor.value(),
        }
    }
}

/// A bullet's pierce (`bullet_pierce` times the ammo's `k_arp`): 0.275 (SR-2M) to 0.40 (AK-12)
/// for the rifles and SMGs bots meet, 0.02 for buckshot. Positive and finite.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Pierce(f32);

impl Pierce {
    /// `None` for a value that is not a pierce (zero, negative, NaN or infinite).
    pub fn new(value: f32) -> Option<Self> {
        Self::new_const(value)
    }

    /// [`Self::new`] for a constant (a default that does not compile when it is no pierce).
    pub const fn new_const(value: f32) -> Option<Self> {
        if value.is_finite() && value > 0.0 { Some(Self(value)) } else { None }
    }

    pub fn value(self) -> f32 {
        self.0
    }
}

/// What a bullet carries at a distance: its damage and its pierce, as they stand there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Load {
    pub damage: f32,
    pub pierce: f32,
}

/// One front face crossed ([`Armor::crossed_by`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Crossing {
    /// The bullet stops here.
    Stopped,
    /// It goes on with `k` of its damage and of its pierce, `0 < k <= 1`.
    Through { k: f32 },
}

/// The share of a bullet's damage that is left after the surfaces on a line: 1 for a clear line,
/// 0 for a bullet stopped. In `[0, 1]`.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct DamageShare(f32);

impl DamageShare {
    /// Nothing in the way.
    pub const ALL: Self = Self(1.0);
    /// The bullet stops on the line.
    pub const NONE: Self = Self(0.0);

    pub fn value(self) -> f32 {
        self.0
    }

    /// The bullet does not reach the far end.
    pub fn is_stopped(self) -> bool {
        self.0 <= 0.0
    }

    /// The share of `kept` in `whole`, `1` when there was none to keep.
    pub(crate) fn of(kept: f32, whole: f32) -> Self {
        if whole > 0.0 { Self((kept / whole).clamp(0.0, 1.0)) } else { Self::ALL }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn share(armor: f32, pierce: f32) -> f32 {
        match Armor::new(armor).unwrap().crossed_by(pierce) {
            Crossing::Stopped => 0.0,
            Crossing::Through { k } => k,
        }
    }

    /// The table of `an-sight.md` §2.2 (live game data, rev 34): the damage share through one front
    /// face. Wood 0.1 and a car 0.08; brick and thick metal 0.2; concrete 0.23.
    #[test]
    fn matches_the_clients_table() {
        // (gun and ammo, P, wood, brick or thick metal, concrete)
        let table = [
            ("AK-12 6.5x39", 0.40, 1.00, 1.00, 0.93),
            ("SR-3M SP-6", 0.36, 1.00, 0.96, 0.81),
            ("VSK-94 SP-5", 0.33, 1.00, 0.88, 0.68),
            ("P90 5.7", 0.30, 1.00, 0.75, 0.52),
            ("SR-2M 9x21", 0.275, 1.00, 0.61, 0.35),
        ];
        for (gun, p, wood, brick, concrete) in table {
            for (armor, want) in [(0.1, wood), (0.08, wood), (0.2, brick), (0.23, concrete)] {
                let got = share(armor, p);
                assert!(
                    (got - want).abs() < 0.006,
                    "{gun} through armor {armor}: {got} vs {want}"
                );
            }
        }
        // Buckshot stops at wood, brick and concrete alike.
        for armor in [0.1, 0.08, 0.2, 0.23] {
            assert_eq!(share(armor, 0.02), 0.0, "armor {armor}");
        }
    }

    #[test]
    fn free_armor_costs_nothing_and_stone_and_ground_stop() {
        assert_eq!(share(0.0, 0.01), 1.0);
        // Stone 0.7, ground 20, default 2: no rifle goes through.
        for armor in [0.7, 2.0, 20.0] {
            assert_eq!(share(armor, 0.40), 0.0, "armor {armor}");
        }
        // Exactly the pierce: the bullet stops (k = 0).
        assert_eq!(share(0.4, 0.4), 0.0);
    }

    #[test]
    fn a_value_that_is_no_armor_or_pierce_is_refused() {
        for bad in [-0.1, f32::NAN, f32::INFINITY] {
            assert_eq!(Armor::new(bad), None);
            assert_eq!(Pierce::new(bad), None);
        }
        assert_eq!(Pierce::new(0.0), None);
        assert_eq!(Armor::new(0.0), Some(Armor::FREE));
    }

    /// The one `f32` of a file: a number is a material, NaN is not known, the rest is no armor.
    #[test]
    fn a_face_is_parsed_from_its_wire_value() {
        assert_eq!(FaceArmor::from_wire(0.23), Some(FaceArmor::Material(Armor::new(0.23).unwrap())));
        assert_eq!(FaceArmor::from_wire(0.0), Some(FaceArmor::FREE));
        assert_eq!(FaceArmor::from_wire(f32::NAN), Some(FaceArmor::Unknown));
        // Another NaN is a damaged file, not a face of unknown material.
        assert_eq!(FaceArmor::from_wire(f32::from_bits(0x7fc0_0001)), None);
        assert_eq!(FaceArmor::from_wire(-1.0), None);
        assert_eq!(FaceArmor::from_wire(f32::INFINITY), None);
        for face in [FaceArmor::Unknown, FaceArmor::FREE, FaceArmor::Material(Armor::new(20.0).unwrap())] {
            assert_eq!(FaceArmor::from_wire(face.to_wire()), Some(face));
        }
    }
}

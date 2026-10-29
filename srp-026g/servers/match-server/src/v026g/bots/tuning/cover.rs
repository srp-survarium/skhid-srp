//! What counts as cover from a bullet (`mapdata::RayTree::damage_share`).

use super::*;

/// A share of a bullet's damage, 0 up to but not including 1: the share over which a line is open to fire. (At 1 no
/// line would be open, a clear one included.)
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Share(f32);

impl Share {
    pub const fn of(share: f32) -> Self {
        assert!(share >= 0.0 && share < 1.0, "a share outside 0 up to 1");
        Self(share)
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for Share {
    type Error = String;

    fn try_from(share: f32) -> Result<Self, String> {
        if (0.0..1.0).contains(&share) {
            Ok(Self(share))
        } else {
            Err(format!("{share} is not a share under which a line is shut (0 up to, not including, 1)"))
        }
    }
}

impl From<Share> for f32 {
    fn from(share: Share) -> Self {
        share.0
    }
}

/// A bullet's pierce as a number of a file: positive and finite, or the file is refused with the reason.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct BulletPierce(mapdata::Pierce);

impl BulletPierce {
    /// For a default: a value out of range does not compile.
    pub const fn of(value: f32) -> Self {
        match mapdata::Pierce::new_const(value) {
            Some(pierce) => Self(pierce),
            None => panic!("a pierce is positive and finite"),
        }
    }
}

impl TryFrom<f32> for BulletPierce {
    type Error = String;

    fn try_from(value: f32) -> Result<Self, String> {
        mapdata::Pierce::new(value)
            .map(Self)
            .ok_or_else(|| format!("{value} is not a pierce (positive and finite)"))
    }
}

impl From<BulletPierce> for f32 {
    fn from(pierce: BulletPierce) -> Self {
        pierce.0.value()
    }
}

/// Cover that stops bullets, by the game's own rule: a bullet of this `pierce` goes through crates, cars,
/// sandbags, thin metal and most building walls at full or high damage (`bullet::try_pierce`, an-sight 2.2), so
/// a spot behind them is in the line of fire.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PierceCover {
    /// The pierce of the bullets the bot hides from: `bullet_pierce` times the ammunition's `k_arp`, 0.275 (SR-2M)
    /// to 0.40 (AK-12) for the rifles and SMGs of the kits. The middle of them: 0.3125 (between the P90 0.30 and the
    /// VSK-94 0.33; brick keeps 0.81 of the damage, concrete 0.59).
    pub pierce: BulletPierce,
    /// A line is open to fire when more than this share of the damage reaches its end: the 0.5 of brick (0.61 to 1.0
    /// by the gun) and concrete (0.35 to 0.93) is open to the rifles that go through; stone, the ground and the
    /// default material stop everything. GUESS: a share a bot would not sit behind.
    pub max_share: Share,
}

impl Default for PierceCover {
    fn default() -> Self {
        Self { pierce: BulletPierce::of(0.3125), max_share: Share::of(0.5) }
    }
}

/// What a bot's cover tests go by, parsed once from [`CoverTuning`]: the bullet and the share over which a line is open.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverRule {
    pub pierce: mapdata::Pierce,
    pub max_share: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CoverTuning {
    /// What the bots' danger tests of cover go by (a spot out of an enemy's fire, an exposed climb, the step to
    /// heal cover, "am I exposed to a known enemy"). `Some`: a line is open to fire when the bullet that reaches its
    /// end keeps more than `max_share` (cover is what stops bullets). `None` (off): every triangle of the bullet mesh is
    /// cover, as it was (the mesh had no materials). Tests of sight (will he fire at me, is this spot out of his
    /// sight) do not read it. On by default.
    pub pierce: Option<PierceCover>,
}

impl Default for CoverTuning {
    fn default() -> Self {
        Self { pierce: Some(PierceCover::default()) }
    }
}

impl CoverTuning {
    /// The rule the cover tests go by, or none (cover is whatever the mesh shows).
    pub fn rule(&self) -> Option<CoverRule> {
        self.pierce.map(|cover| CoverRule { pierce: cover.pierce.0, max_share: cover.max_share.get() })
    }
}

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

/// What a bot pays, in spot score, for each whole share of an enemy's bullet that reaches a spot it could hide at
/// (`SpotQuery::out_of_fire_of`): positive and finite, or the file is refused with the reason. The spot score is
/// metres-scale (`-0.01` a metre of walk), so 0.5 means a spot a third of a bullet reaches costs as much as 17 m of walk.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct SpotCost(f32);

impl SpotCost {
    /// For a default: a value out of range does not compile.
    pub const fn of(value: f32) -> Self {
        assert!(value > 0.0 && value < f32::INFINITY, "a spot cost is positive and finite");
        Self(value)
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for SpotCost {
    type Error = String;

    fn try_from(value: f32) -> Result<Self, String> {
        if value > 0.0 && value.is_finite() {
            Ok(Self(value))
        } else {
            Err(format!("{value} is not a spot cost (positive and finite)"))
        }
    }
}

impl From<SpotCost> for f32 {
    fn from(cost: SpotCost) -> Self {
        cost.0
    }
}

/// What a hold spot pays, in spot score, for each bearing (30-degree sector) the enemy's way can see it from
/// ([`Exposure`]): positive and finite, or the file is refused with the reason. The spot score is metres-scale (`0.5` a
/// metre of rise, `-0.01` a metre of walk), so 2.0 means a spot seen from one more bearing costs as much as 4 m of rise.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct ExposureWeight(f32);

impl ExposureWeight {
    /// For a default: a value out of range does not compile.
    pub const fn of(value: f32) -> Self {
        assert!(value > 0.0 && value < f32::INFINITY, "an exposure weight is positive and finite");
        Self(value)
    }

    pub const fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for ExposureWeight {
    type Error = String;

    fn try_from(value: f32) -> Result<Self, String> {
        if value > 0.0 && value.is_finite() {
            Ok(Self(value))
        } else {
            Err(format!("{value} is not an exposure weight (positive and finite)"))
        }
    }
}

impl From<ExposureWeight> for f32 {
    fn from(weight: ExposureWeight) -> Self {
        weight.0
    }
}

/// [A195] A hold spot is scored down by the bearings the enemy's human routes can see it from, besides the one it is held
/// for: the holders' killers come down the same line (53-57 % within 30 degrees of the entrant's bearing), but the height
/// preference alone makes a spot see 0.4 more of the 12 sectors than a spot with the same sight and none. The default is
/// what the by-side bench measured ([`Exposure::measured`]).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Exposure {
    /// Score taken off for each sector (30 degrees) the spot is seen from.
    pub weight: ExposureWeight,
    /// The enemy's route points that count are this far from the spot ...
    pub near_m: f32,
    /// ... to this far, metres.
    pub far_m: f32,
    /// Sectors whose centre is within this of the bearing to the query's anchor are the line it is held for: not counted.
    pub anchor_deg: f32,
}

impl Exposure {
    /// The numbers the bench read (smoke, 12 seeds, sides swapped): 2.0 a bearing, the enemy's route points 20 to 60 m
    /// off, the 45 degrees round the held line not counted.
    pub const fn measured() -> Self {
        Self { weight: ExposureWeight::of(2.0), near_m: 20.0, far_m: 60.0, anchor_deg: 45.0 }
    }
}

impl Default for Exposure {
    fn default() -> Self {
        Self::measured()
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
    /// A hiding spot's score falls by this much for each whole share of the bullet that reaches it (A142: the
    /// spots under `max_share` are not all as good); none: any spot under `max_share` is as good as another.
    pub spot_cost: Option<SpotCost>,
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
    /// Hiding spots (`SpotQuery::out_of_fire_of`: the outranged hide, the heal cover) are weighed, not only filtered
    /// (Killzone's cover evaluation weighs partial cover by degree): of the spots under `pierce.max_share`, the one
    /// that a smaller share of the enemy's bullet reaches scores higher, by this much for a whole share. `None`
    /// (off): every spot under `max_share` is as good as the next, the nearest first. Reads `pierce`: with that off
    /// there is no share to weigh. On by default.
    pub spot_cost: Option<SpotCost>,
    /// The spots HoldExit and Crossfire hold are weighed by how many bearings the enemy's human routes can see them
    /// from (A195, `SpotQuery::exposure`). `None` (off): the order is by height and nearness, as it was. On by default (#908: score +2.9 %
    /// z +2.4, kills +3.2 %, deaths -3.0 % in the by-side bench).
    pub hold_exposure: Option<Exposure>,
}

impl Default for CoverTuning {
    fn default() -> Self {
        Self { pierce: Some(PierceCover::default()), spot_cost: Some(SpotCost::of(0.5)), hold_exposure: Some(Exposure::measured()) }
    }
}

impl CoverTuning {
    /// The rule the cover tests go by, or none (cover is whatever the mesh shows).
    pub fn rule(&self) -> Option<CoverRule> {
        self.pierce.map(|cover| CoverRule {
            pierce: cover.pierce.0,
            max_share: cover.max_share.get(),
            spot_cost: self.spot_cost,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [A195] The exposure term is on by default with the measured numbers; a file that names it with no numbers gets the same, a weight that
    /// is not positive and finite, or a name it does not know, is refused, and `null` turns it off.
    #[test]
    fn the_hold_exposure_is_on_by_default_and_a_file_gets_the_measured_numbers() {
        assert_eq!(CoverTuning::default().hold_exposure, Some(Exposure::measured()));
        let on: CoverTuning = serde_json::from_str(r#"{"hold_exposure": {}}"#).unwrap();
        assert_eq!(on.hold_exposure, Some(Exposure::measured()));
        let tuned: CoverTuning = serde_json::from_str(r#"{"hold_exposure": {"weight": 1.0}}"#).unwrap();
        assert_eq!(tuned.hold_exposure.map(|e| e.weight.get()), Some(1.0));
        for bad in [r#"{"hold_exposure": {"weight": 0.0}}"#, r#"{"hold_exposure": {"weight": -1.0}}"#, r#"{"hold_exposure": {"wait": 1.0}}"#] {
            assert!(serde_json::from_str::<CoverTuning>(bad).is_err(), "{bad}");
        }
        let off: CoverTuning = serde_json::from_str(r#"{"hold_exposure": null}"#).unwrap();
        assert!(off.hold_exposure.is_none());
    }
}

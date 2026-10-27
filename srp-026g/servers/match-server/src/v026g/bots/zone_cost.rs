//! What walking through a damage zone costs a body, in the pools the zone's hits really take
//! from (batteries F2).
//!
//! The client routes a zone's hit once to each part named in the zone's `hit_parts_filter`,
//! through that part's armour against the zone's damage type (0x319AF0 -> `base_player::hit`
//! 0x2D9AE0 -> `body_part_parameters::hit_by_type` 0x329FF0: the damage times
//! `1 - clamp(armour - piercing, 0, 1)`), and the body table sends the result to pools by the
//! part and the type: toxic and radiation to **infection** (no regeneration, death at 0), fire
//! and shock to pain and the part's own pool, blunt to pain and the body
//! (`gameplay/hit_params/human_hit_params.options`, PROVEN). The bots measured every zone against
//! the body pool (part 0) and counted the raw hit: a toxic cloud they could cross in the books
//! killed them through a pool they never looked at, and a mask did nothing for them.
//!
//! [`Cost`] is a walk's loss in the pools that kill at 0 (body, head, infection and pain: the
//! table's thresholds), [`Reserve`] what a
//! body has left of them, and [`Budget`] the pair a decision needs: a reserve and the armour the
//! cost is worked out with. With no armour ([`Budget::raw`], the switch off) a zone costs its raw
//! hit in the body pool and the other pools are never short: the numbers the code always had.

use std::ops::{Add, Div, Mul};
use std::sync::Arc;

use survarium::v026g::damage::{self, ArmourParams};

use super::hazards::Hazard;
use super::pools::Pools;

/// A walk's loss in each pool that can kill.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cost {
    pub body: f32,
    pub head: f32,
    pub infection: f32,
    pub pain: f32,
}

impl Cost {
    pub const ZERO: Self = Self {
        body: 0.0,
        head: 0.0,
        infection: 0.0,
        pain: 0.0,
    };

    /// A loss of the body pool alone (the raw hit).
    pub fn body(amount: f32) -> Self {
        Self {
            body: amount,
            ..Self::ZERO
        }
    }

    /// The loss of a named pool of the body table; the pools that do not kill count for nothing.
    fn of_pool(pool: &str, amount: f32) -> Self {
        match pool {
            "body" => Self::body(amount),
            "head" => Self {
                head: amount,
                ..Self::ZERO
            },
            "infection" => Self {
                infection: amount,
                ..Self::ZERO
            },
            "pain" => Self {
                pain: amount,
                ..Self::ZERO
            },
            _ => Self::ZERO,
        }
    }

    /// Whether every pool's loss is finite.
    pub fn is_finite(&self) -> bool {
        self.body.is_finite()
            && self.head.is_finite()
            && self.infection.is_finite()
            && self.pain.is_finite()
    }

    /// The largest loss of any pool.
    pub fn most(&self) -> f32 {
        self.body.max(self.head).max(self.infection).max(self.pain)
    }
}

impl Add for Cost {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            body: self.body + other.body,
            head: self.head + other.head,
            infection: self.infection + other.infection,
            pain: self.pain + other.pain,
        }
    }
}

impl Mul<f32> for Cost {
    type Output = Self;

    fn mul(self, k: f32) -> Self {
        Self {
            body: self.body * k,
            head: self.head * k,
            infection: self.infection * k,
            pain: self.pain * k,
        }
    }
}

impl Div<f32> for Cost {
    type Output = Self;

    fn div(self, k: f32) -> Self {
        Self {
            body: self.body / k,
            head: self.head / k,
            infection: self.infection / k,
            pain: self.pain / k,
        }
    }
}

impl std::iter::Sum for Cost {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::ZERO, Add::add)
    }
}

/// What a body has of the pools that kill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reserve {
    pub body: f32,
    pub head: f32,
    pub infection: f32,
    pub pain: f32,
}

impl Reserve {
    /// The body pool alone known; the others are never short.
    pub fn body(health: f32) -> Self {
        Self {
            body: health,
            head: f32::INFINITY,
            infection: f32::INFINITY,
            pain: f32::INFINITY,
        }
    }

    /// The pools of a bot's last world.
    pub fn of(pools: &Pools) -> Self {
        Self {
            body: pools.body,
            head: pools.head,
            infection: pools.infection,
            pain: pools.pain,
        }
    }

    /// Whether the body bears `cost` in every pool it takes from: at most `share` of the pool and
    /// leaves `left` of it. The body pool is always asked (what the code always did); the others
    /// only when the walk takes from them.
    pub fn bears(&self, cost: Cost, share: f32, left: f32) -> bool {
        let pool = |value: f32, cost: f32| cost <= share * value && value - cost >= left;
        pool(self.body, cost.body)
            && (cost.head <= 0.0 || pool(self.head, cost.head))
            && (cost.infection <= 0.0 || pool(self.infection, cost.infection))
            && (cost.pain <= 0.0 || pool(self.pain, cost.pain))
    }

    /// The largest share of a pool `cost` takes (the body pool's with `max(1e-3)` under it, as the
    /// old `cost / health`): 0 for a pool it does not touch.
    pub fn share(&self, cost: Cost) -> f32 {
        let of = |value: f32, cost: f32| if cost > 0.0 { cost / value.max(1e-3) } else { 0.0 };
        (cost.body / self.body.max(1e-3))
            .max(of(self.head, cost.head))
            .max(of(self.infection, cost.infection))
            .max(of(self.pain, cost.pain))
    }

    /// Whether `cost` leaves at least `left` in every pool it takes from (the body pool always).
    pub fn leaves(&self, cost: Cost, left: f32) -> bool {
        self.body - cost.body >= left
            && (cost.head <= 0.0 || self.head - cost.head >= left)
            && (cost.infection <= 0.0 || self.infection - cost.infection >= left)
            && (cost.pain <= 0.0 || self.pain - cost.pain >= left)
    }
}

/// The armour a body wears against a zone's hits: the largest of its items against each hit type
/// on each part (the client sums the items' `hit_params`; the repo's damage rule takes the largest
/// ([`damage`]'s module doc), the same as for bullets).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ZoneArmour {
    items: Vec<&'static ArmourParams>,
}

impl ZoneArmour {
    /// The armour among the items with the dictionary ids `dict_ids` (the items of a kit).
    pub fn of_items(dict_ids: impl IntoIterator<Item = u16>) -> Self {
        let mut items: Vec<&'static ArmourParams> = dict_ids
            .into_iter()
            .filter_map(|id| damage::armour().iter().find(|a| a.dict_id == id))
            .collect();
        items.sort_by_key(|a| a.dict_id);
        items.dedup_by_key(|a| a.dict_id);
        Self { items }
    }

    /// The armour of the part `part` against hits of `hit_type`.
    pub fn against(&self, part: &str, hit_type: &str) -> f32 {
        self.items
            .iter()
            .map(|item| item.of_type(part, hit_type))
            .fold(0.0, f32::max)
    }

    /// A number that is equal for equal armour: the key of what depends on it.
    pub fn key(&self) -> u64 {
        self.items
            .iter()
            .fold(0xcbf2_9ce4_8422_2325, |h, a| (h ^ u64::from(a.dict_id)).wrapping_mul(0x100_0000_01b3))
    }
}

/// How a battery's crossing was weighed with the pools routed (the bench's tally): asked, borne, refused.
/// Raw asks are not counted.
#[cfg(test)]
pub mod asks {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 3] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];

    pub fn count(borne: bool) {
        COUNTS[0].fetch_add(1, Ordering::Relaxed);
        COUNTS[if borne { 1 } else { 2 }].fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> [u64; 3] {
        [0, 1, 2].map(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

/// The reserve a decision weighs a cost against, and the armour the cost is worked out with
/// (`None`: the raw hit in the body pool, the model off).
#[derive(Clone, Debug, PartialEq)]
pub struct Budget {
    pub reserve: Reserve,
    pub armour: Option<Arc<ZoneArmour>>,
}

impl Budget {
    /// The raw model: `health` of the body pool, zones counted by their raw hit.
    pub fn raw(health: f32) -> Self {
        Self {
            reserve: Reserve::body(health),
            armour: None,
        }
    }

    /// The pool-aware model: the pools of a bot and its armour.
    pub fn routed(pools: &Pools, armour: Arc<ZoneArmour>) -> Self {
        Self {
            reserve: Reserve::of(pools),
            armour: Some(armour),
        }
    }

    /// The body pool the old rules read.
    pub fn health(&self) -> f32 {
        self.reserve.body
    }

    /// What equal armour makes equal, for keys of what is worked out once for it (0: raw).
    pub fn armour_key(&self) -> u64 {
        self.armour.as_ref().map_or(0, |a| a.key())
    }
}

impl Hazard {
    /// What standing in this zone costs a second at full intensity under `armour`, in the pools
    /// that kill. A zone the map file gives no hit for is lethal (infinite, as
    /// [`Hazard::hit_per_s`]); one whose file names no parts (written before format 4) is its raw
    /// hit in the body pool, as before.
    pub fn cost_per_s(&self, armour: &ZoneArmour) -> Cost {
        let Some(damage) = &self.damage else {
            return Cost::body(f32::INFINITY);
        };
        if damage.part_names.is_empty() {
            return Cost::body(self.hit_per_s());
        }
        // One part's hit a second: a continuous zone's per `interval`, the others' per hit.
        let per_part = if damage.continuous {
            damage.max_hit
        } else {
            damage.max_hit * 1000.0 / damage.interval_ms.max(1) as f32
        };
        damage::human()
            .zone_hit(
                &damage.part_names,
                &self.damage_type,
                per_part,
                damage.armor_piercing,
                |part, hit_type| armour.against(part, hit_type),
            )
            .into_iter()
            .map(|(pool, amount)| Cost::of_pool(&pool, amount))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::anomalies::Control;
    use crate::v026g::bots::hazards::Activity;
    use mapdata::{Damage, Vec3};

    fn zone(damage_type: &str, parts: &[&str], max_hit: f32, piercing: f32) -> Hazard {
        Hazard {
            name: "zone".into(),
            zone: 0,
            anomaly: None,
            damage_type: damage_type.into(),
            damage: Some(Damage {
                max_hit,
                armor_piercing: piercing,
                interval_ms: 1000,
                continuous: true,
                parts: parts.len() as u8,
                part_names: parts.iter().map(|p| (*p).to_owned()).collect(),
                ..Damage::default()
            }),
            centre: Vec3::ZERO,
            control: Control::Static,
            activity: Activity::Always,
        }
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    /// Acid on the face costs the infection pool, not the body; a mask lowers it.
    #[test]
    fn a_toxic_zone_costs_the_infection_pool_and_a_mask_lowers_it() {
        let acid = zone("toxic", &["face"], 0.25, 0.0);
        let bare = acid.cost_per_s(&ZoneArmour::default());
        assert!(near(bare.infection, 0.25) && bare.body == 0.0 && bare.head == 0.0, "{bare:?}");
        // A mask that covers the face against toxic lowers it by its armour.
        if let Some((mask, _, armour)) = damage::armour().iter().find_map(|a| {
            a.typed
                .iter()
                .find(|(hit_box, ty, v)| hit_box == "face" && ty == "toxic" && *v > 0.0)
                .map(|(_, _, v)| (a, (), *v))
        }) {
            let worn = ZoneArmour::of_items([mask.dict_id]);
            assert!(near(worn.against("face", "toxic"), armour));
            let masked = acid.cost_per_s(&worn);
            assert!(near(masked.infection, 0.25 * (1.0 - armour.clamp(0.0, 1.0))), "{masked:?}");
            assert!(masked.infection < bare.infection);
            assert_eq!(worn.key(), ZoneArmour::of_items([mask.dict_id, mask.dict_id]).key());
            assert_ne!(worn.key(), ZoneArmour::default().key());
        }
        // Old files: no names, the raw hit in the body pool.
        let mut old = zone("toxic", &["face"], 0.25, 0.0);
        old.damage.as_mut().unwrap().part_names.clear();
        let raw = old.cost_per_s(&ZoneArmour::default());
        assert_eq!(raw, Cost::body(0.25 * 1.0));
    }

    /// Fire on the feet takes the pain pool (it kills at 0, and regenerates) and the legs' own
    /// pools, which do not kill: the cost is pain alone, 0.8 of the hit a foot.
    #[test]
    fn fire_on_the_feet_costs_the_pain_pool() {
        let fire = zone("fire", &["left_foot", "right_foot"], 0.1, 0.0);
        let cost = fire.cost_per_s(&ZoneArmour::default());
        assert!(near(cost.pain, 0.16) && cost.body == 0.0 && cost.infection == 0.0, "{cost:?}");
        // Fire on the torso takes the body pool too.
        let torso = zone("fire", &["front"], 0.1, 0.0);
        assert!(near(torso.cost_per_s(&ZoneArmour::default()).body, 0.1));
    }

    /// A zone with no hit in the file is lethal.
    #[test]
    fn a_zone_with_no_hit_is_lethal() {
        let mut none = zone("toxic", &["face"], 0.25, 0.0);
        none.damage = None;
        assert!(!none.cost_per_s(&ZoneArmour::default()).is_finite());
    }

    #[test]
    fn a_reserve_bears_the_pools_the_walk_takes_from() {
        let full = Reserve {
            body: 1.0,
            head: 0.4,
            infection: 1.0,
            pain: 1.0,
        };
        // Share 0.5 and left 0.4 of a pool, as `collect::bears`.
        assert!(full.bears(Cost::body(0.4), 0.5, 0.4));
        assert!(!full.bears(Cost::body(0.6), 0.5, 0.4));
        // Infection: 0.45 of 1.0 is borne; 0.3 of a 0.5 pool is not (over half).
        assert!(full.bears(Cost { infection: 0.45, ..Cost::ZERO }, 0.5, 0.4));
        let sick = Reserve { infection: 0.5, ..full };
        assert!(!sick.bears(Cost { infection: 0.3, ..Cost::ZERO }, 0.5, 0.4));
        // A pool the walk does not take from is not asked (head 0.4 is under `left`).
        assert!(full.bears(Cost::body(0.1), 0.5, 0.4));
        assert!(full.leaves(Cost { infection: 0.5, ..Cost::ZERO }, 0.4));
        assert!(!sick.leaves(Cost { infection: 0.3, ..Cost::ZERO }, 0.4));
    }

    /// The raw budget is the old rule: the body pool only, whatever the other pools.
    #[test]
    fn the_raw_budget_is_the_old_rule() {
        let budget = Budget::raw(0.8);
        assert_eq!(budget.armour_key(), 0);
        assert!(budget.reserve.bears(Cost::body(0.3), 0.5, 0.4));
        assert!(!budget.reserve.bears(Cost::body(0.5), 0.5, 0.4));
        // Even a zero cost needs 0.4 left in the body pool, as the old `bears(0, h)` did.
        assert!(!Reserve::body(0.3).bears(Cost::ZERO, 0.5, 0.4));
    }

    /// The pools that kill are the ones the table says kill.
    #[test]
    fn body_head_infection_and_pain_kill_and_the_limbs_do_not() {
        let human = damage::human();
        for name in ["body", "head", "infection", "pain"] {
            assert!(human.part(name).unwrap().lethal, "{name}");
        }
        for name in ["right_leg", "left_hand"] {
            assert!(!human.part(name).unwrap().lethal, "{name}");
        }
    }
}

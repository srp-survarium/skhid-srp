//! What a stand-in hit does: the damage of the weapon in the shooter's hand
//! against the armour the target's kit wears, as the 0.26g client computes a
//! hit (`survarium::v026g::damage`, whose module doc has the rule with each
//! step marked PROVEN, LIKELY or GUESS and its exe address), in place of the
//! flat fifth of the health every hit took before. `SRP_BENCH_DAMAGE=flat`
//! gives the old flat hit back (a sensitivity row, and the "before" of a
//! comparison).
//!
//! What a hit is made of:
//! - the weapon, its ammo and the distance: the bullet's damage and armour
//!   piercing ([`damage::bullet_at`]). The weapon is the shooter's held gun;
//!   its ammo is the one the tier's kit of that weapon carries
//!   (`loadouts::TierGear`), a shotgun's shot its `buck_shot` pellets, as
//!   many as [`pellets_hitting`] lets land;
//! - the kit worn: the armour of the tier's kit that has the target's held
//!   gun (a weapon is in one kit of a tier), the largest `ranged` armour of
//!   its items on the box hit (LIKELY, `damage` module doc);
//! - the part hit, drawn by [`HIT_SHARES`] of the shooter's weapon class and
//!   then a box of the part by chance;
//! - the pools: the box's `bdb_coeff` of the damage after armour taken from
//!   `body` (the body's `health`), `head` and `pain` ([`Pools`]); a pool at 0
//!   kills. The leg and hand pools do not kill and the stand-in's own leg
//!   model ([`stand_in_leg_hit`]) keeps handling the legs.
//!
//! Not modelled: the regeneration of `head` and `pain` (the body's `health`
//! has none either, in the stand-in), a bullet that crosses two boxes, the
//! killer's spawn protection, skills and modifications (none changes a hit
//! in 0.26g's data).

use survarium::v026g::damage::{self, AmmoParams, ArmourParams, BodyParams};
use survarium::v026g::loadouts::{BotTier, RoleGear, RusherKit, TierGear};
use survarium::v026g::weapons::{self, WeaponParams};

use crate::v026g::bots::guns::Gun;
use crate::v026g::bots::rng::Rng;

use super::{Body, bench_rusher_kits, bench_settings, bench_tier};

/// The parts a hit lands on, as the live matches show it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Part {
    Torso,
    Head,
    Arm,
    Leg,
}

impl Part {
    /// The hit boxes of the part (`human_hit_params`), each as likely as
    /// another (GUESS: the recordings do not say which box).
    pub(crate) fn boxes(self) -> &'static [&'static str] {
        match self {
            Part::Torso => &["front", "back"],
            Part::Head => &["brain", "face"],
            Part::Arm => &["right_arm", "left_arm", "right_forearm", "left_forearm"],
            Part::Leg => &["right_hip", "left_hip", "right_foot", "left_foot"],
        }
    }
}

/// A weapon's class, as the live recordings' tables split their kills.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Rifle,
    Smg,
    Lmg,
    Pistol,
    Shotgun,
    Dmr,
    Bolt,
}

impl Class {
    /// By the weapon's `parameters.type` and its dictionary category (8
    /// rifles, 13 sniper rifles, 14 shotguns, 15 submachine guns, 16
    /// machine guns, 17 pistols): a bolt is a bolt, any other sniper rifle
    /// a marksman's, an `wpn_smg` an SMG whatever its category.
    pub(crate) fn of(weapon: &WeaponParams) -> Self {
        match (weapon.kind, weapon.category) {
            (_, 14) => Class::Shotgun,
            (_, 17) => Class::Pistol,
            (_, 16) => Class::Lmg,
            ("wpn_bolt", _) => Class::Bolt,
            (_, 13) => Class::Dmr,
            ("wpn_smg", _) | (_, 15) => Class::Smg,
            _ => Class::Rifle,
        }
    }
}

/// Where the hits of each class of weapon land: shares of a class's hits on
/// the torso, the head, an arm and a leg, **a fixed table from the live
/// matches with 12 or more human players** (119 of the recorded 177,
/// 2026-10-14 to 17, 29 000 gun kills), made by
/// `private/pvb-343-check/damage7b/pools.py`:
/// - the recordings' snapshots of each player carry the health of every body
///   pool (`wplayers.parts`); the pools a victim's life lost, summed between
///   consecutive snapshots up to the killing shot, by the killer's weapon
///   class, give the damage that went to the body, the hands and the legs
///   (the head's is mostly lost in the one shot that kills, which no
///   snapshot sees: PROVEN for the numbers, LIKELY for what they say);
/// - each pool's damage is turned into hits by the factor the median worn
///   armour leaves of a hit of the class's median `bullet_pierce` (torso 0.6,
///   upper arm 0.2, forearm 0.5, legs 0.5: `REAL-ARMOUR-CHECK.md`);
/// - the head's share is the one for which 2 hits a kill (the live mean)
///   give the class's share of headshot kills (`kills.csv` `hs`: 14 to 28 %).
///
/// So the table is LIKELY for the torso against limbs, GUESS for the head.
/// Rows in [`Part`]'s order; each sums to 1.
pub(crate) const HIT_SHARES: [(Class, [f32; 4]); 7] = [
    (Class::Rifle, [0.417, 0.115, 0.226, 0.242]),
    (Class::Smg, [0.442, 0.083, 0.218, 0.257]),
    (Class::Lmg, [0.370, 0.150, 0.191, 0.289]),
    (Class::Pistol, [0.450, 0.100, 0.215, 0.235]),
    (Class::Shotgun, [0.441, 0.074, 0.215, 0.270]),
    (Class::Dmr, [0.457, 0.091, 0.240, 0.212]),
    (Class::Bolt, [0.446, 0.093, 0.245, 0.216]),
];

/// The part of a hit by `class` for the draw `r` in [0, 1).
pub(crate) fn part_for(class: Class, mut r: f32) -> Part {
    let (_, shares) = HIT_SHARES
        .iter()
        .find(|(c, _)| *c == class)
        .expect("every class has its shares");
    for (part, share) in [Part::Torso, Part::Head, Part::Arm, Part::Leg].into_iter().zip(shares) {
        if r < *share {
            return part;
        }
        r -= share;
    }
    Part::Leg
}

/// The share of a shotgun's pellets that land at `distance` metres: all up to
/// 10 m, 0.6 at 25 m, 0.3 at 50 m, in a line between and beyond. GUESS: the
/// cone of `buck_dispersion` is not worked out (`DAMAGE-FORMULA.md` §3).
pub(crate) fn pellets_hitting(buck_shot: u32, distance: f32) -> u32 {
    if buck_shot <= 1 {
        return 1;
    }
    let share = if distance <= 10.0 {
        1.0
    } else if distance <= 25.0 {
        1.0 - 0.4 * (distance - 10.0) / 15.0
    } else {
        (0.6 - 0.3 * (distance - 25.0) / 25.0).max(0.3)
    };
    ((buck_shot as f32 * share).round() as u32).max(1)
}

/// The pools of a body besides `Body::health` (the `body` pool).
///
/// **Regeneration** ([`regenerate`]). The live client regenerates every pool
/// by itself: `regeneration_timeout` (8 s) after its last hit it gains
/// `regeneration_speed` of its health a second up to `regeneration_threshold`
/// of it, body and head 0.03 a second up to half, pain 0.10 up to full
/// (`human_hit_params`; `body_part::regenerate` 0x32A4B0: PROVEN as code, the
/// data's reading LIKELY). Not modelled: `health_regeneration_speed_modifier`
/// (no item of the tiers' kits sets it: GUESS), the Larkspur and the other
/// drugs. Healing items act through [`Pools::restore`] (and `Body::health`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pools {
    pub(crate) head: f32,
    pub(crate) pain: f32,
    /// The body's `health` as the last [`regenerate`] saw it: a fall in it
    /// since (a bullet, a zone) is a hit on the body pool.
    body_seen: f32,
    /// When each pool (body, head, pain) was last hit, ms; `None`: not this
    /// life.
    hit_at: [Option<u32>; 3],
}

/// A pool of [`Pools`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pool {
    Head,
    Pain,
}

impl Pools {
    /// `pool` loses `amount` at `now` ms: its regeneration counts from here.
    pub(crate) fn hurt(&mut self, pool: Pool, amount: f32, now: u32) {
        let (health, hit_at) = match pool {
            Pool::Head => (&mut self.head, 1),
            Pool::Pain => (&mut self.pain, 2),
        };
        *health -= amount;
        self.hit_at[hit_at] = Some(now);
    }

    /// A lethal pool of these is empty.
    pub(crate) fn dead(&self) -> bool {
        self.head <= 1e-3 || self.pain <= 1e-3
    }

    /// `pool` gains `amount`, up to its full health (the body's data).
    pub(crate) fn restore(&mut self, pool: Pool, amount: f32) {
        let full = Self::default();
        match pool {
            Pool::Head => self.head = (self.head + amount).min(full.head),
            Pool::Pain => self.pain = (self.pain + amount).min(full.pain),
        }
    }
}

impl Default for Pools {
    /// Full, by the body's data.
    fn default() -> Self {
        let body = damage::human();
        let health = |pool: &str| body.part(pool).map_or(1.0, |part| part.health);
        Self {
            head: health("head"),
            pain: health("pain"),
            body_seen: health("body"),
            hit_at: [None; 3],
        }
    }
}

/// One pool's regeneration over `dt_s` after `part`'s timeout since `hit`
/// (the pool's last hit), at `now` ms.
fn regenerated(part: &damage::BodyPart, health: f32, hit: Option<u32>, now: u32, dt_s: f32) -> f32 {
    let Some(hit) = hit else {
        return health;
    };
    let cap = part.health * part.regeneration_threshold;
    if (now.saturating_sub(hit) as f32) < part.regeneration_timeout * 1000.0 || health >= cap {
        return health;
    }
    (health + part.regeneration_speed * dt_s).min(cap)
}

/// Every living body's pools regenerate by one tick, `now` ms: the client's
/// rule for each ([`Pools`]). The timeout counts from the pool's last hit,
/// told by a fall of its health since the last call (a bullet hit, or a
/// zone's on the body pool). None with `SRP_BENCH_DAMAGE=flat`, the old
/// stand-in's.
pub(crate) fn regenerate(model: &Model, bodies: &mut [Body], now: u32) {
    if model.flat {
        return;
    }
    let human = damage::human();
    let dt_s = crate::v026g::bots::kin::TICK_S;
    for body in bodies.iter_mut().filter(|body| body.alive) {
        let pools = &mut body.pools;
        if body.health < pools.body_seen {
            pools.hit_at[0] = Some(now);
        }
        let pool_part = |name: &str| human.part(name).expect("the body has the pool");
        body.health = regenerated(pool_part("body"), body.health, pools.hit_at[0], now, dt_s);
        pools.body_seen = body.health;
        pools.head = regenerated(pool_part("head"), pools.head, pools.hit_at[1], now, dt_s);
        pools.pain = regenerated(pool_part("pain"), pools.pain, pools.hit_at[2], now, dt_s);
    }
}

/// How the stand-in's hits are worked out: the bots' tier (their kits) and
/// the kit its rusher wears (the rusher's ammo and armour).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Model {
    pub(crate) tier: BotTier,
    pub(crate) rusher: RusherKit,
    /// `SRP_BENCH_DAMAGE=flat`: every hit takes a fifth of the health.
    pub(crate) flat: bool,
    /// `SRP_BENCH_GRENADE_DAMAGE=off`: a grenade's blast hurts nobody (the
    /// stand-in before [`super::standin_grenade`]; its counts only).
    pub(crate) grenades: bool,
}

impl Model {
    /// The client's blast hurts the bodies.
    pub(crate) fn blasts_hurt(&self) -> bool {
        !self.flat && self.grenades
    }
}

impl Default for Model {
    fn default() -> Self {
        Self {
            tier: bench_tier(),
            rusher: bench_rusher_kits().worn(bench_tier()),
            flat: bench_settings().var("SRP_BENCH_DAMAGE").is_ok_and(|v| v == "flat"),
            grenades: !bench_settings().var("SRP_BENCH_GRENADE_DAMAGE").is_ok_and(|v| v == "off"),
        }
    }
}

/// What the tiers' flat hit used to take, of the health.
pub(crate) const FLAT_HIT: f32 = 0.2;

/// The kit of `tier` that has `weapon` when its rusher wears `rusher`
/// (every kit of a tier has another weapon), else of any tier: the
/// stand-in's first gun is the lowest tier's support's until the bot's first
/// input names its own.
fn gear_with<'a>(tier: &'a TierGear, rusher: RusherKit, weapon: &str) -> Option<&'a RoleGear> {
    use survarium::v026g::loadouts::BotRole;
    BotRole::ALL
        .into_iter()
        .map(|role| tier.of_kit(role, rusher))
        .find(|gear| gear.weapon == weapon)
}

fn kit_of(tier: BotTier, rusher: RusherKit, weapon: &str) -> Option<&'static RoleGear> {
    gear_with(tier.gear(), rusher, weapon).or_else(|| {
        BotTier::ALL
            .into_iter()
            .find_map(|other| gear_with(other.gear(), rusher, weapon))
    })
}

/// What the kit of `tier` and `rusher` that has `weapon` wears against a zone's hits (the largest of its
/// items on each part and type, `zone_cost::ZoneArmour`); nothing if no kit has the weapon.
pub(crate) fn zone_armour_of(tier: BotTier, rusher: RusherKit, weapon: &str) -> crate::v026g::bots::zone_cost::ZoneArmour {
    crate::v026g::bots::zone_cost::ZoneArmour::of_items(
        kit_of(tier, rusher, weapon)
            .map_or(&[][..], |kit| kit.armour)
            .iter()
            .filter_map(|(_, name)| damage::armour_named(name))
            .map(|item| item.dict_id),
    )
}

/// The largest `ranged` armour on `hit_box` among `kit`'s items.
pub(crate) fn armour_on(kit: Option<&RoleGear>, hit_box: &str) -> f32 {
    kit.map_or(0.0, |kit| {
        kit.armour
            .iter()
            .filter_map(|(_, name)| damage::armour_named(name))
            .map(|item: &ArmourParams| item.of(hit_box))
            .fold(0.0, f32::max)
    })
}

/// What one pellet or bullet of the hit does to `pools`: `(body, head,
/// pain)` taken, from the box's feeds after the box's armour.
fn pools_taken(body: &BodyParams, hit_box: &str, amount: f32, arp: f32, armor: f32) -> [f32; 3] {
    let delta = damage::after_armour(amount, arp, armor);
    let mut taken = [0.0; 3];
    if let Some(part) = body.part(hit_box) {
        for (pool, c) in &part.feeds {
            let slot = match pool.as_str() {
                "body" => 0,
                "head" => 1,
                "pain" => 2,
                _ => continue,
            };
            taken[slot] += c * delta;
        }
    }
    taken
}

/// One hit of `shooter_gun` at `distance` metres on `target`: takes the
/// damage off its pools and, when a lethal pool is empty, its `health` to 0
/// (the stand-in's death rule reads it). Draws the part and the box from
/// `rng`. Gives the stamina the hit takes from the target's bar: the
/// weapon's `stamina_damage` for each pellet or bullet that lands (none for
/// a flat hit).
pub(crate) fn hit(
    model: &Model,
    target: &mut Body,
    shooter_gun: &Gun,
    distance: f32,
    rng: &mut Rng,
    now: u32,
) -> f32 {
    if model.flat {
        target.health -= FLAT_HIT;
        return 0.0;
    }
    let Some(weapon) = weapons::weapon_named(shooter_gun.item) else {
        target.health -= FLAT_HIT;
        return 0.0;
    };
    let ammo = kit_of(model.tier, model.rusher, shooter_gun.item)
        .and_then(|kit| damage::ammo_named(kit.ammo))
        .unwrap_or(&PLAIN_AMMO);
    let worn = kit_of(model.tier, model.rusher, target.gun.item);
    let bullet = damage::bullet_at(weapon, ammo, distance);
    let class = Class::of(weapon);
    let body = damage::human();
    let pellets = pellets_hitting(ammo.buck_shot, distance);
    for _ in 0..pellets {
        let part = part_for(class, rng.unit());
        let boxes = part.boxes();
        let hit_box = boxes[rng.below(boxes.len())];
        let [on_body, on_head, on_pain] = pools_taken(
            body,
            hit_box,
            bullet.damage,
            bullet.arp,
            armour_on(worn, hit_box),
        );
        target.health -= on_body;
        for (pool, taken) in [(Pool::Head, on_head), (Pool::Pain, on_pain)] {
            if taken > 0.0 {
                target.pools.hurt(pool, taken, now);
            }
        }
    }
    if target.pools.dead() {
        target.health = target.health.min(0.0);
    }
    weapon.stamina_damage * pellets as f32
}

/// An ammunition that is not in the data: no multiplier, one bullet.
const PLAIN_AMMO: AmmoParams = AmmoParams {
    dict_id: 0,
    name: "",
    k_damage: 1.0,
    k_arp: 1.0,
    k_min_damage_amount: 1.0,
    k_max_damage_distance: 1.0,
    k_min_damage_distance: 1.0,
    muzzle_speed: 800.0,
    air_resistance: 1.0,
    distance: 1000.0,
    buck_shot: 1,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::guns::{Arms, gun};

    const MODEL: Model = Model {
        tier: BotTier::Low,
        rusher: RusherKit::Control,
        flat: false,
        grenades: true,
    };

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 2e-3
    }

    fn game_body() -> Body {
        let mut game = super::super::Game::new(super::super::soak_world(), &[0, 1], 1, None, None);
        game.bodies.remove(0)
    }

    /// A body of the stand-in holding `item`, shot once by `shooter` at
    /// `distance` with the part `part` and the box `hit_box` forced: the
    /// pools after.
    fn shot_on(tier: BotTier, shooter: &str, target_gun: &str, hit_box: &str, distance: f32) -> (f32, Pools) {
        let mut body = game_body();
        body.gun = gun(target_gun).expect("a weapon");
        let weapon = weapons::weapon_named(shooter).unwrap();
        let ammo = kit_of(tier, RusherKit::Control, shooter)
            .and_then(|kit| damage::ammo_named(kit.ammo))
            .unwrap();
        let bullet = damage::bullet_at(weapon, ammo, distance);
        let taken = pools_taken(
            damage::human(),
            hit_box,
            bullet.damage,
            bullet.arp,
            armour_on(kit_of(tier, RusherKit::Control, target_gun), hit_box),
        );
        body.health -= taken[0];
        body.pools.head -= taken[1];
        body.pools.pain -= taken[2];
        (body.health, body.pools)
    }

    #[test]
    fn the_hit_shares_sum_to_one_and_cover_every_class() {
        for (class, shares) in HIT_SHARES {
            assert!(near(shares.iter().sum::<f32>(), 1.0), "{class:?}");
        }
        for r in [0.0, 0.3, 0.45, 0.5, 0.6, 0.7, 0.9, 0.999] {
            for class in [Class::Rifle, Class::Smg, Class::Lmg, Class::Pistol, Class::Shotgun, Class::Dmr, Class::Bolt] {
                part_for(class, r);
            }
        }
        assert_eq!(part_for(Class::Rifle, 0.0), Part::Torso);
        assert_eq!(part_for(Class::Rifle, 0.43), Part::Head);
        assert_eq!(part_for(Class::Rifle, 0.999), Part::Leg);
    }

    #[test]
    fn a_weapon_is_in_the_class_the_live_tables_put_it_in() {
        let class = |name: &str| Class::of(weapons::weapon_named(name).unwrap());
        assert_eq!(class("ak_74u"), Class::Smg);
        assert_eq!(class("uzi"), Class::Smg);
        assert_eq!(class("spas_12"), Class::Shotgun);
        assert_eq!(class("toz_34"), Class::Shotgun);
        assert_eq!(class("mosin_nagant"), Class::Bolt);
        assert_eq!(class("svd"), Class::Dmr);
        assert_eq!(class("tt_33"), Class::Pistol);
        assert_eq!(class("rpd"), Class::Lmg);
        assert_eq!(class("a_545"), Class::Rifle);
    }

    #[test]
    fn pellets_land_by_distance() {
        assert_eq!(pellets_hitting(1, 5.0), 1);
        assert_eq!(pellets_hitting(8, 5.0), 8);
        assert_eq!(pellets_hitting(8, 25.0), 5);
        assert_eq!(pellets_hitting(8, 50.0), 2);
        assert_eq!(pellets_hitting(8, 90.0), 2);
        assert!(pellets_hitting(8, 17.5) < 8 && pellets_hitting(8, 17.5) > 5);
    }

    #[test]
    fn a_torso_hit_takes_the_body_and_the_pain_alike() {
        // Unarmoured (the starter pistol is in no tier's kit): the whole
        // bullet.
        let weapon = weapons::weapon_named("ak_74u").unwrap();
        let ammo = kit_of(BotTier::Low, RusherKit::Control, "ak_74u")
            .and_then(|k| damage::ammo_named(k.ammo))
            .unwrap();
        let full = damage::bullet_at(weapon, ammo, 5.0).damage;
        let (health, pools) = shot_on(BotTier::Low, "ak_74u", "tt_33", "front", 5.0);
        assert!(near(1.0 - health, full), "{health} {full}");
        assert!(near(1.0 - pools.pain, full));
        assert_eq!(pools.head, Pools::default().head);
    }

    #[test]
    fn a_head_hit_takes_the_head_pool_and_the_pain() {
        let (health, pools) = shot_on(BotTier::Low, "ak_74u", "tt_33", "brain", 5.0);
        assert_eq!(health, 1.0);
        assert!(pools.head < 0.4 && near(0.4 - pools.head, 1.0 - pools.pain));
    }

    /// A limb hit never touches the body or the head: it takes pain only, at
    /// its own share (arm 0.8, forearm 1, hip 0.9, foot 0.8).
    #[test]
    fn a_limb_hit_takes_pain_only() {
        let pain = |hit_box: &str| {
            let (health, pools) = shot_on(BotTier::Low, "ak_74u", "tt_33", hit_box, 5.0);
            assert_eq!((health, pools.head), (1.0, Pools::default().head), "{hit_box}");
            1.0 - pools.pain
        };
        let forearm = pain("right_forearm");
        assert!(near(pain("right_arm") / forearm, 0.8));
        assert!(near(pain("left_hip") / forearm, 0.9));
        assert!(near(pain("left_foot") / forearm, 0.8));
    }

    /// The armour of the kit the target holds: against the AK-12's piercing
    /// the tier 13-15 torso takes what its armour keeps above the piercing,
    /// as `DAMAGE-FORMULA.md` has it (each 0.1 of armour above the piercing
    /// removes a tenth).
    #[test]
    fn the_target_s_kit_armours_it() {
        let tier = BotTier::High;
        let weapon = weapons::weapon_named("ak_12").unwrap();
        let ammo = damage::ammo_named("ammo_6.5x39").unwrap();
        let bullet = damage::bullet_at(weapon, ammo, 10.0);
        let torso = armour_on(kit_of(tier, RusherKit::Control, "sr3m"), "front");
        assert!(torso > 0.5, "the tier's torso {torso}");
        let (health, _) = shot_on(tier, "ak_12", "sr3m", "front", 10.0);
        let want = bullet.damage * (1.0 - (torso - bullet.arp).clamp(0.0, 1.0));
        assert!(near(1.0 - health, want), "{health} {want}");
        // The unarmoured hit of the same bullet is more.
        let (bare, _) = shot_on(tier, "ak_12", "tt_33", "front", 10.0);
        assert!(bare < health);
    }

    /// The hits a kill takes: an unarmoured target shot in the torso falls
    /// to the same hit as many times as its damage goes into the body's
    /// pool of 1 (the AK-74U at 10 m), through [`hit`] with the part forced by
    /// a class whose every hit is a torso hit.
    #[test]
    fn a_kill_takes_as_many_hits_as_the_damage_says() {
        let weapon = weapons::weapon_named("ak_74u").unwrap();
        let ammo = kit_of(BotTier::Low, RusherKit::Control, "ak_74u")
            .and_then(|k| damage::ammo_named(k.ammo))
            .unwrap();
        let delta = damage::bullet_at(weapon, ammo, 10.0).damage;
        let mut body = game_body();
        body.gun = gun("tt_33").unwrap();
        let mut hits = 0;
        while body.health > 1e-3 && hits < 100 {
            let taken = pools_taken(damage::human(), "front", delta, 0.0, 0.0);
            body.health -= taken[0];
            hits += 1;
        }
        assert_eq!(hits, (1.0 / delta).ceil() as i32, "{delta}");
    }

    /// Regeneration by the client's rule: nothing for 8 s after a hit, then
    /// the pain pool 0.10 a second up to full, the body 0.03 up to half,
    /// and a pool above its threshold none.
    #[test]
    fn the_pools_regenerate_after_their_timeout() {
        let mut bodies = vec![game_body()];
        bodies[0].alive = true;
        // A hit at 1 s: body 0.3 (a fall of its health), pain 0.4.
        bodies[0].health = 0.3;
        bodies[0].pools.pain = 0.4;
        bodies[0].pools.hit_at[2] = Some(1_000);
        regenerate(&MODEL, &mut bodies, 1_000);
        assert_eq!((bodies[0].health, bodies[0].pools.pain), (0.3, 0.4));
        let dt = crate::v026g::bots::kin::TICK_S;
        // Inside the timeout (8 s from 1 s): nothing.
        regenerate(&MODEL, &mut bodies, 8_900);
        assert_eq!((bodies[0].health, bodies[0].pools.pain), (0.3, 0.4));
        // After it: a tick's worth.
        regenerate(&MODEL, &mut bodies, 9_050);
        assert!(near(bodies[0].health, 0.3 + 0.03 * dt));
        assert!(near(bodies[0].pools.pain, 0.4 + 0.10 * dt));
        // Long after: the pain full, the body at half and no more.
        for t in 0..400 {
            regenerate(&MODEL, &mut bodies, 9_100 + t * 50);
        }
        assert!(near(bodies[0].health, 0.5) && near(bodies[0].pools.pain, 1.0));
        // A pool above its threshold stays.
        bodies[0].health = 0.8;
        bodies[0].pools.hit_at[0] = Some(0);
        regenerate(&MODEL, &mut bodies, 100_000);
        assert_eq!(bodies[0].health, 0.8);
        // The dead do not regenerate.
        bodies[0].alive = false;
        bodies[0].pools.pain = 0.1;
        regenerate(&MODEL, &mut bodies, 100_050);
        assert_eq!(bodies[0].pools.pain, 0.1);
    }

    #[test]
    fn restoring_a_pool_stops_at_its_full_health() {
        let full = Pools::default();
        let mut pools = Pools {
            head: 0.1,
            pain: 0.2,
            ..full
        };
        pools.restore(Pool::Head, 0.15);
        pools.restore(Pool::Pain, 5.0);
        assert!(near(pools.head, 0.25) && pools.pain == full.pain);
        pools.restore(Pool::Head, 5.0);
        assert_eq!(pools, full);
    }

    #[test]
    fn a_flat_hit_is_the_old_fifth() {
        let mut body = game_body();
        let model = Model {
            tier: BotTier::Low,
            rusher: RusherKit::Control,
            flat: true,
            grenades: true,
        };
        let mut rng = Rng::new(1);
        hit(&model, &mut body, Arms::fallback().primary.0, 10.0, &mut rng, 1_000);
        assert!(near(body.health, 0.8));
    }

    /// An emptied pool kills: a head pool at 0 takes the health to 0, which
    /// the stand-in's death rule reads.
    #[test]
    fn an_empty_head_pool_kills() {
        let mut body = game_body();
        body.pools.head = 0.0;
        body.health = 0.9;
        let model = Model {
            tier: BotTier::Low,
            rusher: RusherKit::Control,
            flat: false,
            grenades: true,
        };
        let mut rng = Rng::new(1);
        hit(&model, &mut body, Arms::fallback().primary.0, 10.0, &mut rng, 1_000);
        assert!(body.health <= 1e-3);
    }

    /// The hits follow the kit the rusher wears: the stand-in finds its
    /// ammo and its armour by the weapon in hand and the kit worn, so the
    /// slug's and the SMG's ammo are theirs and the helmet kits' helmet
    /// stops more than the old kit's (a helmet-only arm shows in the bench).
    #[test]
    fn the_rushers_ammo_and_armour_follow_its_kit() {
        for tier in [BotTier::Mid, BotTier::High] {
            for rusher in RusherKit::ALL {
                if rusher.worn_in(tier) != rusher {
                    continue;
                }
                let gear = tier.gear().rusher_gear(rusher);
                let found = kit_of(tier, rusher, gear.weapon).unwrap();
                assert_eq!(found, gear, "{tier} {}", rusher.name());
            }
            let head = |rusher| armour_on(kit_of(tier, rusher, "spas_12"), "brain");
            assert!(head(RusherKit::Helmet) > head(RusherKit::Control), "{tier}");
            assert_eq!(head(RusherKit::Slug), head(RusherKit::Helmet), "{tier}");
            let ammo = |rusher| kit_of(tier, rusher, "spas_12").unwrap().ammo;
            assert_eq!(ammo(RusherKit::Slug), "ammo_12mm_slug");
            assert_eq!(ammo(RusherKit::Helmet), "ammo_12mm_buck");
        }
    }

    /// On the live game data (`SRP_GAME_DATA`): the same for the P90 and the
    /// Kriss, whose ammo and armour the test data lacks. Run alone:
    /// `SRP_GAME_DATA=<server-game-data.db> cargo test -p match-server --lib
    /// -- --ignored on_the_live_data_the_rushers_ammo`
    #[test]
    #[ignore = "needs SRP_GAME_DATA, the server's game data"]
    fn on_the_live_data_the_rushers_ammo_and_armour_follow_its_kit() {
        survarium::v026g::game_data::init().expect("SRP_GAME_DATA reads and passes srp's checks");
        the_rushers_ammo_and_armour_follow_its_kit();
        for (tier, rusher, ammo) in [
            (BotTier::High, RusherKit::Smg, "ammo_5.7x28"),
            (BotTier::High, RusherKit::Alt, "ammo_45acp"),
            (BotTier::Mid, RusherKit::Smg, "ammo_9x21"),
            (BotTier::Mid, RusherKit::Alt, "ammo_45acp"),
        ] {
            let weapon = tier.gear().rusher_gear(rusher).weapon;
            let found = kit_of(tier, rusher, weapon).unwrap();
            assert!(damage::ammo_named(found.ammo).is_some(), "{ammo}");
            assert_eq!(found.ammo, ammo);
            assert!(weapons::weapon_named(weapon).is_some(), "{weapon}");
        }
    }

    /// F2: an acid cloud on the face takes the infection pool, which does not mend, and kills when it runs
    /// out; the body pool is never touched. Raw, the same cloud takes the body pool.
    #[test]
    fn a_routed_acid_zone_takes_the_infection_pool_and_kills_by_it() {
        use crate::v026g::bots::hazards::Hazards;
        use mapdata::{Damage, Rotation, Shape, ShapeKind, Zone, ZoneKind};
        let at = mapdata::Vec3::new(30.0, 0.0, 20.0);
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            map.zones.push(Zone {
                kind: ZoneKind::Damage,
                name: "acid".into(),
                team: -1,
                position: at,
                damage_type: "toxic".into(),
                damage: Some(Damage {
                    max_hit: 0.25,
                    interval_ms: 1000,
                    continuous: true,
                    parts: 1,
                    part_names: vec!["face".into()],
                    ..Damage::default()
                }),
                shapes: vec![Shape {
                    kind: ShapeKind::Box,
                    center: at + mapdata::Vec3::new(0.0, 1.0, 0.0),
                    rotation: Rotation::IDENTITY,
                    half: mapdata::Vec3::new(5.0, 2.0, 5.0),
                }],
            });
        });
        let inside = Hazards::build(&world, 0.0);
        let run = |routed: bool| {
            let mut game = super::super::Game::new(world.clone(), &[0, 1], 1, None, None);
            game.zone_routed = routed;
            game.bodies[0].feet = at;
            for _ in 0..(60.0 / super::super::TICK_S) as usize {
                game.tick += super::super::TICK_MS;
                game.zone_tick(&inside, &inside, true);
                if !game.bodies[0].alive {
                    break;
                }
            }
            game
        };
        let routed = run(true);
        assert!(!routed.bodies[0].alive, "the infection ran out");
        assert_eq!(routed.zone_pool_deaths, [0, 0, 1, 0], "{:?}", routed.zone_pool_lost);
        assert!(routed.zone_pool_lost[2] > 0.5 && routed.zone_pool_lost[0] == 0.0, "{:?}", routed.zone_pool_lost);
        assert!((routed.bodies[0].health - 1.0).abs() < 1e-6, "the body pool is not touched");
        // Raw: the body pool pays.
        let raw = run(false);
        assert!(!raw.bodies[0].alive);
        assert_eq!(raw.zone_pool_deaths, [0, 0, 0, 0]);
        assert!(raw.bodies[0].health <= 1e-3);
    }
}

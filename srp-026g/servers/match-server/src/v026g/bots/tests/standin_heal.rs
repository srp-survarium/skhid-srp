//! The stand-in game's drugs (**model**,
//! `SRP_BENCH_HEAL_MODEL`, default on): without them a bot that presses a
//! medkit in a run changes nothing the bench counts, and a stock that is
//! wrong shows in no number.
//!
//! What the client does (PROVEN, `fact-health-recovery.md` §1-§2; the
//! record's §8.8):
//! - a drug is used by its quick-use key, from the slot of the kit in use;
//!   a use takes one unit out of the slot at once, and a slot whose drug is
//!   active ignores a second press (`medkit::action` 0x33D700);
//! - nothing heals for the drug's delay; then for its active time each pool
//!   it treats gains its rate a second, clamped to the pool's maximum
//!   (`medkit::tick` 0x33D790): the medkit 2 s then 3 s at 1.0 a second on
//!   the head (0.4), the body (1.0), the hands and legs (0.5), no pain; the
//!   bandages of revision 32 (`origin/main:srp-026g/scripts/content-edit/
//!   bandages-1.json`) 1 s then 1 s at 1.0 on hands and legs, 0.35 on the
//!   body, 0.14 on the head, 0.35 on the pain; the antitoxin 1 s then 10 s
//!   at 0.5 on the infection, and a toxic hit is halved while it is active
//!   (LIKELY: the hit path was not read);
//! - a use cannot be cancelled; only a death ends it;
//! - a spawn fills a slot from the item's storage (the slot's count, at
//!   most what the storage has) and the storage loses it; a death puts the
//!   slot back (`inventory::setup_from_profile` 0x2FCB70).
//!
//! What the stand-in game has of it (`standin_damage.rs` is the damage model and keeps the
//! regeneration of the body, the head and the pain: 0.03 a second after 8 s without a hit, up to
//! half, the pain 0.1 to full; this file does not model it again):
//! - the drugs, their stock, their active time and the pools they act on, the effects
//!   applied only through [`Restore`] (`value`, `restore`), as the client restores a pool through
//!   `increase_health`: the body pool is `Body::health`, the head and the pain are
//!   `standin_damage::Pools::restore`, the legs are the stand-in's own leg pools, and the hands
//!   and the infection (which nothing in the stand-in damages) are [`Rest`];
//! - the legs keep the stand-in's own 8 s mending (`STAND_IN_LEG_BROKEN_MS`); a drug adds to it
//!   and ends the broken leg's effects (`leg_broken_until`) once both legs are above 0
//!   (`remove_affects`, 0x33DA50);
//! - the damage is `standin_damage`'s (the weapon against the armour, on the body, the head and
//!   the pain; a zone's hit its `hit_per_s`, every one taken as toxic); the hands and the
//!   infection lose nothing, so an antitoxin finds nothing to cure in the bench (its
//!   arithmetic is tested);
//! - each body has the three kits of a bot's profile, two medkits in each (`BOT_POCKETS`); the
//!   kit a spawn uses is the one the bench says the bot picked ([`tell`]).

use std::sync::OnceLock;

use survarium::v026g::loadouts::{BOT_POCKETS, QUICK_USE_FIRST_BIT};

use super::*;
use crate::v026g::bots::stock::{Drug, Reading, Readings};

/// Whether the stand-in has the drugs (`SRP_BENCH_HEAL_MODEL`,
/// anything but `off`).
pub(crate) fn model_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        !bench_settings()
            .var("SRP_BENCH_HEAL_MODEL")
            .is_ok_and(|v| v == "off")
    })
}

pub(crate) const BODY_MAX: f32 = 1.0;
pub(crate) const HEAD_MAX: f32 = 0.4;
pub(crate) const LIMB_MAX: f32 = 0.5;
pub(crate) const PAIN_MAX: f32 = 1.0;
pub(crate) const INFECTION_MAX: f32 = 1.0;

/// A pool of the body, as the world's `part_health` has them (a leg and a
/// hand are two).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Pool {
    Body,
    Head,
    Leg(usize),
    Hand(usize),
    Pain,
    Infection,
}

impl Pool {
    pub(crate) fn max(self) -> f32 {
        match self {
            Self::Body => BODY_MAX,
            Self::Head => HEAD_MAX,
            Self::Leg(_) | Self::Hand(_) => LIMB_MAX,
            Self::Pain => PAIN_MAX,
            Self::Infection => INFECTION_MAX,
        }
    }
}

/// What the drugs act on: the pools the client restores, reached through
/// two calls and no field. [`BodyPools`] is the stand-in body's: the damage
/// model (`standin_damage`) keeps the head and the pain and restores them
/// itself.
pub(crate) trait Restore {
    fn value(&self, pool: Pool) -> f32;
    /// Adds `amount` to the pool, clamped to 0 and its maximum
    /// (`body_part::increase_health` 0x32A420).
    fn restore(&mut self, pool: Pool, amount: f32);
}

/// The pools nothing else of the stand-in has: the hands and the infection (nothing damages
/// them here).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rest {
    pub(crate) hands: [f32; 2],
    pub(crate) infection: f32,
}

impl Rest {
    pub(crate) const FULL: Self = Self {
        hands: [LIMB_MAX; 2],
        infection: INFECTION_MAX,
    };
}

/// A stand-in body's pools: its `health` (the body pool), its leg pools, the
/// damage model's [`standin_damage::Pools`] (head, pain) and the [`Rest`]. The
/// head and the pain are restored through the damage model's own call.
pub(crate) struct BodyPools<'a> {
    pub(crate) health: &'a mut f32,
    pub(crate) legs: &'a mut [f32; 2],
    pub(crate) pools: &'a mut super::standin_damage::Pools,
    pub(crate) rest: &'a mut Rest,
}

impl Restore for BodyPools<'_> {
    fn value(&self, pool: Pool) -> f32 {
        match pool {
            Pool::Body => *self.health,
            Pool::Head => self.pools.head,
            Pool::Leg(side) => self.legs[side],
            Pool::Hand(side) => self.rest.hands[side],
            Pool::Pain => self.pools.pain,
            Pool::Infection => self.rest.infection,
        }
    }

    fn restore(&mut self, pool: Pool, amount: f32) {
        use super::standin_damage::Pool as Damage;
        let up = |value: &mut f32| *value = (*value + amount).clamp(0.0, pool.max());
        match pool {
            Pool::Body => up(self.health),
            Pool::Head => self.pools.restore(Damage::Head, amount),
            Pool::Leg(side) => up(&mut self.legs[side]),
            Pool::Hand(side) => up(&mut self.rest.hands[side]),
            Pool::Pain => self.pools.restore(Damage::Pain, amount),
            Pool::Infection => up(&mut self.rest.infection),
        }
    }
}

/// The groups of pools a drug treats.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Group {
    Body,
    Head,
    Legs,
    Hands,
    Pain,
    Infection,
}

impl Group {
    fn pools(self) -> &'static [Pool] {
        match self {
            Self::Body => &[Pool::Body],
            Self::Head => &[Pool::Head],
            Self::Legs => &[Pool::Leg(0), Pool::Leg(1)],
            Self::Hands => &[Pool::Hand(0), Pool::Hand(1)],
            Self::Pain => &[Pool::Pain],
            Self::Infection => &[Pool::Infection],
        }
    }
}

/// What a drug does: the client's `.options` (PROVEN) and the revision 32
/// bandages.
#[derive(Debug)]
pub(crate) struct Model {
    pub(crate) drug: Drug,
    pub(crate) delay_s: f32,
    pub(crate) active_s: f32,
    rates: &'static [(Group, f32)],
    /// A toxic hit is this share of itself while the drug is active.
    toxic: f32,
}

pub(crate) const MODELS: [Model; 3] = [
    Model {
        drug: Drug::Medkit,
        delay_s: 2.0,
        active_s: 3.0,
        rates: &[
            (Group::Body, 1.0),
            (Group::Head, 1.0),
            (Group::Hands, 1.0),
            (Group::Legs, 1.0),
        ],
        toxic: 1.0,
    },
    Model {
        drug: Drug::Bandages,
        delay_s: 1.0,
        active_s: 1.0,
        rates: &[
            (Group::Hands, 1.0),
            (Group::Legs, 1.0),
            (Group::Body, 0.35),
            (Group::Head, 0.14),
            (Group::Pain, 0.35),
        ],
        toxic: 1.0,
    },
    Model {
        drug: Drug::Antitoxin,
        delay_s: 1.0,
        active_s: 10.0,
        rates: &[(Group::Infection, 0.5)],
        toxic: 0.5,
    },
];

impl Model {
    /// The rate a second the drug restores `pool` at.
    pub(crate) fn rate(&self, pool: Pool) -> f32 {
        self.rates
            .iter()
            .filter(|(group, _)| group.pools().contains(&pool))
            .map(|&(_, rate)| rate)
            .sum()
    }
}

pub(crate) fn model_of(drug: Drug) -> &'static Model {
    MODELS
        .iter()
        .find(|model| model.drug == drug)
        .expect("a model for every drug")
}

/// `model`'s rates over `dt_s` seconds on `target`.
fn heal(target: &mut impl Restore, model: &Model, dt_s: f32) {
    for &(group, rate) in model.rates {
        for &pool in group.pools() {
            target.restore(pool, rate * dt_s);
        }
    }
}

/// A drug under way.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Use {
    drug: Drug,
    quick: usize,
    at: u32,
}

/// A slot of a kit, the game's way: `hand` the stack in the slot, `store`
/// the item's storage.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Slot {
    drug: Drug,
    quick: usize,
    count: u16,
    store: u16,
    hand: u16,
}

/// What a press of a quick-use key did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Pressed {
    /// A unit went out of the slot.
    Used(Drug),
    /// The slot had none (or the key has no drug): nothing happened.
    Empty,
    /// The slot's drug is active: ignored.
    Active(Drug),
}

/// A body's drugs.
#[derive(Clone, Debug)]
pub(crate) struct Heal {
    uses: Vec<Use>,
    kits: Vec<Vec<Slot>>,
    kit: usize,
    /// The kit the next spawn uses ([`tell`]).
    pub(crate) kit_next: usize,
}

/// The drugs of every bot kit ([`BOT_POCKETS`]), the game's: all in store.
fn standard_kit() -> Vec<Slot> {
    BOT_POCKETS
        .iter()
        .filter_map(|item| {
            let drug = Drug::of_name(item.name)?;
            Some(Slot {
                drug,
                quick: usize::from(item.input_bit? - QUICK_USE_FIRST_BIT),
                count: item.count as u16,
                store: item.count as u16,
                hand: 0,
            })
        })
        .collect()
}

impl Heal {
    pub(crate) fn new() -> Self {
        Self {
            uses: Vec::new(),
            kits: vec![standard_kit(); 3],
            kit: 0,
            kit_next: 0,
        }
    }

    /// A spawn: nothing active, the last life's slots back in store and
    /// the new kit's filled from it.
    pub(crate) fn spawn(&mut self) {
        self.uses.clear();
        for slot in &mut self.kits[self.kit] {
            slot.store += slot.hand;
            slot.hand = 0;
        }
        self.kit = self.kit_next.min(self.kits.len() - 1);
        for slot in &mut self.kits[self.kit] {
            slot.hand = slot.count.min(slot.store);
            slot.store -= slot.hand;
        }
    }

    /// The key `quick` (0 on) pressed at `tick`.
    pub(crate) fn press(&mut self, quick: usize, tick: u32) -> Pressed {
        let Some(slot) = self.kits[self.kit]
            .iter_mut()
            .find(|slot| slot.quick == quick)
        else {
            return Pressed::Empty;
        };
        if slot.hand == 0 {
            return Pressed::Empty;
        }
        if self.uses.iter().any(|u| u.quick == quick) {
            return Pressed::Active(slot.drug);
        }
        slot.hand -= 1;
        self.uses.push(Use {
            drug: slot.drug,
            quick,
            at: tick,
        });
        Pressed::Used(slot.drug)
    }

    /// The share of a toxic hit the body takes at `tick`.
    pub(crate) fn toxic_factor(&self, tick: u32) -> f32 {
        self.uses
            .iter()
            .filter(|u| active(u, tick))
            .map(|u| model_of(u.drug).toxic)
            .fold(1.0, f32::min)
    }

    /// One tick (`dt_s` long, ending at `tick`) on `pools`: the drugs under way. Returns
    /// whether a drug acted, so the broken legs' effects end.
    fn tick(&mut self, pools: &mut impl Restore, tick: u32, dt_s: f32) -> bool {
        let mut acted = false;
        for u in &self.uses {
            if active(u, tick) {
                heal(pools, model_of(u.drug), dt_s);
                acted = true;
            }
        }
        self.uses.retain(|u| !over(u, tick));
        acted
    }

    /// What the world shows of the drugs.
    fn readings(&self, alive: bool) -> Readings {
        let mut drugs = Vec::new();
        for (kit, slots) in self.kits.iter().enumerate() {
            for slot in slots {
                let in_use = kit == self.kit;
                drugs.push(Reading {
                    kit,
                    drug: slot.drug,
                    quick: slot.quick,
                    slot: slot.count,
                    in_use,
                    in_hand: (in_use && alive).then_some(slot.hand),
                    // A dead body's slot is back in store.
                    stored: u32::from(slot.store + if in_use && !alive { slot.hand } else { 0 }),
                });
            }
        }
        Readings {
            kit: self.kit,
            kits: self.kits.len(),
            alive,
            drugs,
        }
    }
}

/// In its active time at `tick`, the ticks `(at + delay, at + delay +
/// active]`.
fn active(u: &Use, tick: u32) -> bool {
    let model = model_of(u.drug);
    let from = u.at + (model.delay_s * 1000.0) as u32;
    tick > from && tick <= from + (model.active_s * 1000.0) as u32
}

fn over(u: &Use, tick: u32) -> bool {
    let model = model_of(u.drug);
    tick >= u.at + ((model.delay_s + model.active_s) * 1000.0) as u32
}

/// A living body's drugs for the tick ending at `tick`
/// (`TICK_S` long).
pub(crate) fn advance(body: &mut Body, tick: u32) {
    if !model_on() {
        return;
    }
    let mut pools = BodyPools {
        health: &mut body.health,
        legs: &mut body.legs,
        pools: &mut body.pools,
        rest: &mut body.rest,
    };
    let acted = body.heal.tick(&mut pools, tick, TICK_S);
    // The broken leg's effects end with the drug's first tick.
    if acted && body.legs.iter().all(|&pool| pool > 0.0) {
        body.leg_broken_until = 0;
    }
}

/// What the bots are told of a world: the pools, and the drugs.
#[derive(Clone, Debug)]
pub(crate) struct Seen {
    pub(crate) pools: crate::v026g::bots::pools::Pools,
    pub(crate) readings: Readings,
}

/// A press of a quick-use key: the tick, the place, what it did, and the
/// body pool before it.
pub(crate) type DrugPress = (u32, Place, Pressed, f32);

/// The quick-use keys among `pressed` (the keys that went down this tick)
/// pressed by the living `body` of `place` at `tick`.
pub(crate) fn press_keys(
    body: &mut Body,
    place: Place,
    pressed: u32,
    tick: u32,
    log: &mut Vec<DrugPress>,
) {
    if !model_on() || !body.alive {
        return;
    }
    for (quick, bit) in key::QUICK.into_iter().enumerate() {
        if pressed & bit != 0 {
            let what = body.heal.press(quick, tick);
            log.push((tick, place, what, body.health));
        }
    }
}

impl Game {
    /// What a world made at `tick` shows of every body's pools and drugs
    /// ([`Game::record`] keeps them with the world).
    pub(crate) fn seen_at(&self, tick: u32) -> Option<Vec<Seen>> {
        let (_, seen) = self.seen.iter().find(|(t, _)| *t == tick)?;
        Some(seen.clone())
    }
}

/// Tells the bots the world of `tick` as the game shows it: each bot's
/// pools and drugs; and the stand-in learns the kit each picked for its next life.
pub(crate) fn tell(game: &mut Game, bots: &mut Bots, tick: u32) {
    if !model_on() {
        return;
    }
    for (place, body) in game.bodies.iter_mut().enumerate() {
        if let Some(Some(bot)) = bots.bots.get(place) {
            body.heal.kit_next = bot.kit_chosen;
        }
    }
    let Some(seen) = game.seen_at(tick) else {
        return;
    };
    for (place, seen) in seen.iter().enumerate() {
        bots.pools(place, seen.pools);
        bots.stock(place, tick, &seen.readings);
    }
}

/// The snapshot of a body's pools and drugs a world keeps.
pub(crate) fn seen(body: &Body) -> Seen {
    let (mut health, mut legs, mut damage, mut rest) =
        (body.health, body.legs, body.pools, body.rest);
    let pools = BodyPools {
        health: &mut health,
        legs: &mut legs,
        pools: &mut damage,
        rest: &mut rest,
    };
    Seen {
        pools: crate::v026g::bots::pools::Pools {
            body: pools.value(Pool::Body),
            head: pools.value(Pool::Head),
            legs: [Pool::Leg(0), Pool::Leg(1)].map(|pool| pools.value(pool)),
            hands: [Pool::Hand(0), Pool::Hand(1)].map(|pool| pools.value(pool)),
            pain: pools.value(Pool::Pain),
            infection: pools.value(Pool::Infection),
        },
        readings: body.heal.readings(body.alive),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pools as plain values, for the arithmetic.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Pools {
        body: f32,
        head: f32,
        legs: [f32; 2],
        hands: [f32; 2],
        pain: f32,
        infection: f32,
    }

    impl Restore for Pools {
        fn value(&self, pool: Pool) -> f32 {
            match pool {
                Pool::Body => self.body,
                Pool::Head => self.head,
                Pool::Leg(side) => self.legs[side],
                Pool::Hand(side) => self.hands[side],
                Pool::Pain => self.pain,
                Pool::Infection => self.infection,
            }
        }

        fn restore(&mut self, pool: Pool, amount: f32) {
            let slot = match pool {
                Pool::Body => &mut self.body,
                Pool::Head => &mut self.head,
                Pool::Leg(side) => &mut self.legs[side],
                Pool::Hand(side) => &mut self.hands[side],
                Pool::Pain => &mut self.pain,
                Pool::Infection => &mut self.infection,
            };
            *slot = (*slot + amount).clamp(0.0, pool.max());
        }
    }

    fn run(heal: &mut Heal, pools: &mut Pools, from: u32, ms: u32) {
        let ticks = ms / TICK_MS;
        for k in 1..=ticks {
            heal.tick(pools, from + k * TICK_MS, TICK_S);
        }
    }

    fn hurt() -> Pools {
        Pools {
            body: 0.4,
            head: 0.1,
            legs: [0.0, 0.3],
            hands: [0.2, 0.5],
            pain: 0.5,
            infection: 0.3,
        }
    }

    fn spawned() -> Heal {
        let mut heal = Heal::new();
        heal.spawn();
        heal
    }

    #[test]
    fn a_medkit_heals_nothing_for_two_seconds_then_one_a_second_for_three_clamped() {
        let mut heal = spawned();
        let mut pools = hurt();
        assert_eq!(heal.press(0, 10_000), Pressed::Used(Drug::Medkit));
        run(&mut heal, &mut pools, 10_000, 2_000);
        assert_eq!(pools, hurt(), "the delay");
        run(&mut heal, &mut pools, 12_000, 500);
        assert!((pools.body - 0.9).abs() < 1e-3, "{}", pools.body);
        assert!((pools.head - 0.4).abs() < 1e-3, "head clamped at 0.4");
        assert!((pools.legs[0] - 0.5).abs() < 1e-3 && (pools.hands[0] - 0.5).abs() < 1e-3);
        run(&mut heal, &mut pools, 12_500, 3_000);
        assert_eq!(pools.body, 1.0);
        // No pain, no infection.
        assert_eq!((pools.pain, pools.infection), (0.5, 0.3));
        assert!(heal.uses.is_empty(), "over after 5 s");
    }

    #[test]
    fn bandages_and_the_antitoxin_have_their_own_rates_and_times() {
        let mut heal = spawned();
        heal.kits[0].push(Slot {
            drug: Drug::Bandages,
            quick: 1,
            count: 1,
            store: 0,
            hand: 1,
        });
        heal.kits[0].push(Slot {
            drug: Drug::Antitoxin,
            quick: 2,
            count: 1,
            store: 0,
            hand: 1,
        });
        let mut pools = hurt();
        assert_eq!(heal.press(1, 0), Pressed::Used(Drug::Bandages));
        run(&mut heal, &mut pools, 0, 1_000);
        assert_eq!(pools, hurt(), "1 s delay");
        run(&mut heal, &mut pools, 1_000, 1_000);
        assert!((pools.body - 0.75).abs() < 1e-3, "{}", pools.body);
        assert!((pools.head - 0.24).abs() < 1e-3, "{}", pools.head);
        assert!((pools.legs[0] - 0.5).abs() < 1e-3);
        assert!((pools.pain - 0.85).abs() < 1e-3, "{}", pools.pain);
        assert_eq!(pools.infection, 0.3);
        let mut pools = hurt();
        let mut heal2 = spawned();
        heal2.kits[0].push(Slot {
            drug: Drug::Antitoxin,
            quick: 2,
            count: 1,
            store: 0,
            hand: 1,
        });
        assert_eq!(heal2.press(2, 0), Pressed::Used(Drug::Antitoxin));
        assert_eq!(heal2.toxic_factor(500), 1.0, "not yet");
        assert_eq!(heal2.toxic_factor(5_000), 0.5, "halved while active");
        assert_eq!(heal2.toxic_factor(11_500), 1.0, "over after 11 s");
        run(&mut heal2, &mut pools, 0, 4_000);
        assert!(
            (pools.infection - 1.0).abs() < 1e-3,
            "full in 2 s from 0.3 (1.5 s)"
        );
        assert_eq!(pools.body, 0.4);
    }

    #[test]
    fn the_slot_is_the_games_a_press_takes_one_and_a_second_while_active_is_ignored() {
        let mut heal = spawned();
        assert_eq!(heal.kits[0][0].hand, 2, "the spawn fills the slot");
        assert_eq!(heal.kits[0][0].store, 0);
        assert_eq!(heal.press(0, 1_000), Pressed::Used(Drug::Medkit));
        assert_eq!(heal.press(0, 1_050), Pressed::Active(Drug::Medkit));
        assert_eq!(heal.kits[0][0].hand, 1, "an ignored press takes none");
        // Another key has no drug.
        assert_eq!(heal.press(3, 1_100), Pressed::Empty);
        // After the 5 s the second goes; the third finds the slot empty.
        heal.uses.clear();
        assert_eq!(heal.press(0, 9_000), Pressed::Used(Drug::Medkit));
        heal.uses.clear();
        assert_eq!(heal.press(0, 20_000), Pressed::Empty);
        // A death and a spawn give the kit nothing back; another kit has
        // its own two.
        heal.spawn();
        assert_eq!(heal.kits[0][0].hand, 0);
        heal.kit_next = 1;
        heal.spawn();
        assert_eq!(heal.kits[1][0].hand, 2);
        assert_eq!(heal.kit, 1);
    }

    #[test]
    fn the_dead_slot_goes_back_to_store_and_the_world_shows_it_so() {
        let mut heal = spawned();
        heal.press(0, 0);
        let alive = heal.readings(true);
        let r = &alive.drugs[0];
        assert_eq!((r.in_use, r.in_hand, r.stored), (true, Some(1), 0));
        let dead = heal.readings(false);
        let r = &dead.drugs[0];
        assert_eq!((r.in_hand, r.stored), (None, 1));
        // The other kits are whole in store.
        assert_eq!(
            (
                dead.drugs[1].in_use,
                dead.drugs[1].in_hand,
                dead.drugs[1].stored
            ),
            (false, None, 2)
        );
    }

    /// The game around it: a press in an input heals the body in the
    /// game's own time, the slot's third press does nothing, and the world
    /// shows the slot as the press left it.
    #[test]
    fn a_pressed_medkit_heals_a_hurt_body_in_the_stand_in_game_and_the_third_press_does_nothing() {
        let mut game = Game::new(soak_world(), &[0, 1], 1, None, None);
        let gun = game.bodies[0].gun;
        game.bodies[0].health = 0.4;
        let press = |game: &mut Game, tick: u32, keys: u32| {
            game.inputs[0].push_back((tick, keys, (0.0, 0.0), None, gun));
        };
        // Presses at 1 s, 8 s and 15 s, each let go a tick later.
        for at in [1_000, 8_000, 15_000] {
            press(&mut game, at, key::QUICK[0]);
            press(&mut game, at + TICK_MS, 0);
        }
        let mut hurt_at = 0;
        while game.tick < 20_000 {
            game.step();
            if game.tick == 3_000 {
                // 2 s after the first press: nothing yet.
                assert_eq!(game.bodies[0].health, 0.4);
                assert_eq!(
                    game.seen_at(3_000).unwrap()[0].readings.drugs[0].in_hand,
                    Some(1)
                );
            }
            if game.tick == 7_000 {
                assert!(
                    (game.bodies[0].health - 1.0).abs() < 1e-3,
                    "{}",
                    game.bodies[0].health
                );
                game.bodies[0].health = 0.5;
                hurt_at = game.tick;
            }
        }
        assert!(hurt_at > 0);
        let presses: Vec<_> = game.drug_log.iter().map(|p| p.2).collect();
        assert_eq!(
            presses,
            [
                Pressed::Used(Drug::Medkit),
                Pressed::Used(Drug::Medkit),
                Pressed::Empty
            ]
        );
        // The second healed it again, the third nothing: it regrew only to
        // the line (0.5) before, and the medkit took it to full.
        assert!((game.bodies[0].health - 1.0).abs() < 1e-3);
        // The world shows the empty slot and the store.
        let drug = &game.seen_at(19_950).unwrap()[0].readings.drugs[0];
        assert_eq!((drug.in_hand, drug.stored), (Some(0), 0));
    }

    /// The bot's table of what a drug treats is its own knowledge
    /// (`plays/heal.rs`); the stand-in is the game: a number changed on
    /// one side only shows here.
    #[test]
    fn the_model_has_the_records_numbers_and_the_bots_table_the_same() {
        use crate::v026g::bots::plays::heal::ITEMS;
        use crate::v026g::bots::pools::Pool as Group;
        let medkit = model_of(Drug::Medkit);
        assert_eq!((medkit.delay_s, medkit.active_s), (2.0, 3.0));
        let bandages = model_of(Drug::Bandages);
        assert_eq!((bandages.delay_s, bandages.active_s), (1.0, 1.0));
        let antitoxin = model_of(Drug::Antitoxin);
        assert_eq!((antitoxin.delay_s, antitoxin.active_s), (1.0, 10.0));
        for item in &ITEMS {
            let model = model_of(item.drug);
            assert_eq!(
                (item.delay_s, item.active_s),
                (model.delay_s, model.active_s)
            );
            let pairs = [
                (Group::Body, Pool::Body),
                (Group::Head, Pool::Head),
                (Group::Legs, Pool::Leg(0)),
                (Group::Legs, Pool::Leg(1)),
                (Group::Hands, Pool::Hand(0)),
                (Group::Hands, Pool::Hand(1)),
                (Group::Pain, Pool::Pain),
                (Group::Infection, Pool::Infection),
            ];
            for (mine, theirs) in pairs {
                let rate = item
                    .treats
                    .iter()
                    .find(|(pool, _)| *pool == mine)
                    .map_or(0.0, |&(_, rate)| rate);
                assert_eq!(rate, model.rate(theirs), "{:?} {mine:?}", item.drug);
            }
        }
    }

    /// Two minutes of 4 against 4 in the stand-in game, the bots told what
    /// each world shows: they hurt one another, press the medkit and are
    /// healed by it, and no press goes into an empty slot or into an
    /// active medkit (the stock is the game's), in every life of the run.
    #[test]
    fn a_short_run_heals_its_bots_and_presses_no_empty_slot() {
        let world = soak_world();
        let teams: Vec<u8> = (0..8).map(|p| u8::from(p >= 4)).collect();
        let mut bots = Bots::new(
            0x50AC,
            survarium::v026g::maps::mode::BATTERIES,
            0,
            &teams,
            &[true; 8],
            &[None; 8],
            Arc::new(Tuning::default()),
            Trace::none(0),
        );
        bots.map_ready(world.clone());
        let mut game = Game::new(world, &teams, 3, None, None);
        (0..teams.len()).for_each(|place| bots.entered(place, true, 0));
        let (mut clock, mut next_world) = (0, 1_000);
        while clock < 120_000 {
            clock += 25;
            while game.tick + TICK_MS <= clock {
                game.step();
            }
            if clock >= next_world {
                next_world += 1_000;
                let tick = clock / TICK_MS * TICK_MS - 300;
                if let Some((_, fixes, health, facts)) = game.world_at(tick) {
                    for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                        bots.fixed(place, fix, Some(health));
                    }
                    tell(&mut game, &mut bots, tick);
                    bots.facts(facts);
                }
            }
            let (made, _) = bots.poll(Clock {
                clock,
                time_left_s: 600.0,
                total_s: 600.0,
                playing: true,
            });
            for made in made {
                let r = made.input.rotation;
                let turn = (
                    half_value(u16::from_le_bytes([r[0], r[1]])),
                    half_value(u16::from_le_bytes([r[2], r[3]])),
                );
                game.inputs[made.place].push_back((
                    made.time,
                    made.input.actions,
                    turn,
                    None,
                    Arms::fallback().primary.0,
                ));
            }
        }
        let used = game
            .drug_log
            .iter()
            .filter(|p| matches!(p.2, Pressed::Used(_)))
            .count();
        let wasted: Vec<_> = game
            .drug_log
            .iter()
            .filter(|p| !matches!(p.2, Pressed::Used(_)))
            .collect();
        assert!(
            used >= 2,
            "{used} presses in 2 minutes of fights ({} hits)",
            game.hits
        );
        assert!(wasted.is_empty(), "{wasted:?}");
    }
}

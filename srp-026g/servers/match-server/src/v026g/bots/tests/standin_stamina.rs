//! The stamina of a stand-in body, worked as the 0.26g client works it (grid's
//! `S` specification §7, from `player_stamina::tick` 0x329790, `regenerate`
//! 0x3298C0, `spend` 0x329A20, `decrease` 0x3296C0, `sprint` 0x329A70 and the
//! jump's gate `is_trying_to_jump` 0x2FC230; PROVEN unless marked), in place
//! of a body that sprints and jumps for as long as its keys ask:
//! - the bar is 0-100; the sprint keys (SPRINT and FORWARD, none of BACK,
//!   FIRE, CROUCH, AIM: [`Kinematics::sprints`]) spend it at
//!   `5 × (1 + 1.5 t) × (1 + skill 201)` a second, whether the body can
//!   move at sprint or not (LIKELY: the hands item says "sprinting" from the
//!   keys). A carrier spends nothing (GUESS: its hands item is the battery);
//! - not sprinting, the bar regenerates at `10 × (1 + skill 204)` a second
//!   when it is empty or more than 1 s after the last spending (a sprint, a
//!   jump or a hit), and the regeneration runs before the low flag is
//!   judged, so a bar a hit emptied does not set the flag;
//! - the low flag is set when the bar is at 0 after a tick, and cleared
//!   when it is over 20 (a lockout let go at 0 takes about 3 s to clear: the
//!   first tick regenerates because the bar is empty, then the wait holds it
//!   a second, then 2 s of regeneration). While it is set the body does not sprint (it walks
//!   at its walk speed), whatever its keys; an overburdened body (the kit's
//!   weight at its capacity, no skill 213) never sprints;
//! - a jump needs the flag clear and the bar at its cost, `10 × (1 + 1.5 t)
//!   × (1 + skill 201)`, which it spends at the press (the take-off is about
//!   0.2 s after it in the game: LIKELY); a refused press spends nothing;
//! - a bullet that hits takes the weapon's `stamina_damage` (the shotgun's
//!   per pellet that lands) from the victim's bar, and restarts the wait.
//!   Grenades' damage to it is not modelled (LIKELY the same path, not
//!   proven).
//!
//! `t` is the weight's cost, `clamp((w - 20) / (capacity - 20), 0, 1)`, of
//! the kit the body's weapon belongs to in the bench's tier; the skills are
//! that tier's ([`bot_skills`]). A weapon of no kit gets a light kit's
//! (`t = 0`, no skills).
//!
//! `SRP_BENCH_STAMINA=on` turns the model on; off (the default) is no stamina (the stand-in before this
//! file), and the counts stay at zero.

use survarium::v026g::bot_skills;
use survarium::v026g::loadouts::BotEquipment;
use survarium::v026g::loadouts::BotRole;
use survarium::v026g::loadouts::BotTier;
use survarium::v026g::loadouts::RusherKits;
use survarium::v026g::loadouts::bot_kits;
use survarium::v026g::skills::modifier_index;

use crate::v026g::bots::guns::Arms;

use super::bench_settings;

/// `SRP_BENCH_STAMINA=on` turns the model on (off by default: the stand-in's bodies then sprint and jump as before).
fn enabled() -> bool {
    bench_settings().var("SRP_BENCH_STAMINA").is_ok_and(|v| v == "on")
}

/// The bar's top, and what the stamina does a second (`default.player`'s
/// `stamina_params`).
pub(crate) const MAX: f32 = 100.0;

pub(crate) const SPRINT_SPEND_PER_S: f32 = 5.0;

pub(crate) const REGEN_PER_S: f32 = 10.0;

/// The jump's cost before the weight and skills (`amount_to_jump`).
pub(crate) const JUMP_COST: f32 = 10.0;

/// Regeneration waits this long after the last spending, ms.
pub(crate) const WAIT_MS: u32 = 1_000;

/// The low flag is set at a bar of 0 and cleared over this.
pub(crate) const CLEAR_ABOVE: f32 = 20.0;

/// What a body's kit makes of the rates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rates {
    /// Spent a second of sprint keys.
    pub(crate) spend: f32,
    /// Regained a second.
    pub(crate) regen: f32,
    /// Spent by a jump, and the least the bar needs for one.
    pub(crate) jump: f32,
    /// Not over its capacity (or skill 213).
    pub(crate) can_sprint: bool,
    /// The weight's cost `t` of the kit (0 for a light one).
    pub(crate) weight_cost: f32,
}

impl Rates {
    /// A kit of no weight cost and no skills.
    pub(crate) const LIGHT: Self = Self {
        spend: SPRINT_SPEND_PER_S,
        regen: REGEN_PER_S,
        jump: JUMP_COST,
        can_sprint: true,
        weight_cost: 0.0,
    };

    /// The rates of `item`'s kit in `tier` (its first kit with that weapon,
    /// the camper's first), else [`Rates::LIGHT`].
    pub(crate) fn of(tier: BotTier, item: &str) -> Self {
        let roles = [BotRole::Camper, BotRole::Flanker, BotRole::Support, BotRole::Rusher];
        let equipment = BotEquipment::of(tier, RusherKits::default());
        for role in roles {
            let kits = bot_kits(equipment, role);
            for (k, kit) in kits.iter().enumerate() {
                if Arms::of_kit(equipment, role, k).is_none_or(|arms| arms.primary.0.item != item) {
                    continue;
                }
                let modifiers = bot_skills::bot_modifiers(&kits);
                let capacity = bot_skills::carry_capacity(kit, &modifiers);
                let weight = kit.weight();
                let t = bot_skills::weight_cost(weight, capacity);
                let index = |name| modifier_index(name).expect("a modifier of the 68");
                let carried = 1.0 + survarium::v026g::item_params::player()
                    .max_carried_weight_spending_speed_modifier
                    * t;
                let spending = 1.0 + modifiers[index("stamina_spending_speed_skill")];
                return Self {
                    spend: SPRINT_SPEND_PER_S * carried * spending,
                    regen: REGEN_PER_S * (1.0 + modifiers[index("stamina_regeneration_speed")]),
                    jump: JUMP_COST * carried * spending,
                    can_sprint: weight < capacity || modifiers[index("can_sprint")] > 0.0,
                    weight_cost: t,
                };
            }
        }
        Self::LIGHT
    }
}

/// The flag the bar sets and clears.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Flag {
    Fine,
    Low,
}

/// What the stamina counted, for the bench.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Counts {
    /// Times the low flag was set.
    pub(crate) lockouts: u32,
    /// Ticks a living body had the flag set.
    pub(crate) low_ticks: u32,
    /// Of those, the ticks its sprint keys were held (a lockout lasts as
    /// long as the key does).
    pub(crate) sprint_keys_low: u32,
    /// Jump presses refused for the bar or the flag.
    pub(crate) jump_low: u32,
    /// Sprint keys held by a body that cannot sprint (low or overburdened),
    /// ticks.
    pub(crate) sprint_refused: u32,
}

/// A body's stamina.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stamina {
    value: f32,
    flag: Flag,
    /// The previous tick's time at the last spending, ms; none since a spawn.
    last_sprint: Option<u32>,
    last_tick: u32,
    rates: Rates,
    /// The weapon the rates are of.
    item: &'static str,
    /// The model works ([`enabled`]).
    on: bool,
    pub(crate) counts: Counts,
}

impl Stamina {
    /// A full bar at `now` ms for `item` in `tier`.
    pub(crate) fn new(now: u32, tier: BotTier, item: &'static str) -> Self {
        Self::working(enabled(), now, tier, item)
    }

    /// The same, the model working or not (a test turns it on whatever the environment says).
    pub(crate) fn working(on: bool, now: u32, tier: BotTier, item: &'static str) -> Self {
        Self {
            value: MAX,
            flag: Flag::Fine,
            last_sprint: None,
            last_tick: now,
            rates: if on { Rates::of(tier, item) } else { Rates::LIGHT },
            item,
            on,
            counts: Counts::default(),
        }
    }

    /// A new life: a full bar, the rates of its weapon's kit.
    pub(crate) fn spawn(&mut self, now: u32, tier: BotTier, item: &'static str) {
        *self = Self {
            counts: self.counts,
            ..Self::working(self.on, now, tier, item)
        };
    }

    /// The body took another weapon (another kit): its rates, the bar as it is.
    pub(crate) fn switch(&mut self, tier: BotTier, item: &'static str) {
        if self.on && self.item != item {
            self.item = item;
            self.rates = Rates::of(tier, item);
        }
    }

    pub(crate) fn value(&self) -> f32 {
        self.value
    }

    pub(crate) fn low(&self) -> bool {
        self.flag == Flag::Low
    }

    /// The weight's cost `t` of the kit the rates are of.
    pub(crate) fn weight_cost(&self) -> f32 {
        self.rates.weight_cost
    }

    /// The kit is not over its capacity (or has skill 213): whatever the bar, the body may sprint by its weight.
    pub(crate) fn can_sprint_by_weight(&self) -> bool {
        self.rates.can_sprint
    }

    /// The body may move at sprint: the flag clear and not overburdened.
    pub(crate) fn may_sprint(&self) -> bool {
        !self.on || self.flag == Flag::Fine && self.rates.can_sprint
    }

    /// The bar is spent by `amount`: the wait restarts.
    fn spend(&mut self, amount: f32) {
        self.value = (self.value - amount).max(0.0);
        self.last_sprint = Some(self.last_tick);
    }

    /// One tick at `now` ms. `spending`: the sprint keys of a body that
    /// spends (they are held and it carries nothing); `keys`: the sprint
    /// keys are held.
    pub(crate) fn tick(&mut self, now: u32, spending: bool, keys: bool) {
        if !self.on {
            return;
        }
        let dt = now.saturating_sub(self.last_tick) as f32 / 1_000.0;
        if spending {
            self.spend(self.rates.spend * dt);
        } else if self.value < MAX
            && (self.value == 0.0 || self.last_sprint.is_none_or(|at| now > at + WAIT_MS))
        {
            self.value = (self.value + self.rates.regen * dt).min(MAX);
        }
        self.last_tick = now;
        match self.flag {
            Flag::Fine if self.value <= 0.0 => {
                self.flag = Flag::Low;
                self.counts.lockouts += 1;
            }
            Flag::Low if self.value > CLEAR_ABOVE => self.flag = Flag::Fine,
            _ => {}
        }
        if self.flag == Flag::Low {
            self.counts.low_ticks += 1;
            self.counts.sprint_keys_low += u32::from(keys);
        }
        self.counts.sprint_refused += u32::from(keys && !self.may_sprint());
    }

    /// A jump is pressed: it goes, and its cost is spent, when the flag is
    /// clear and the bar has the cost; else nothing is spent.
    pub(crate) fn jump(&mut self) -> bool {
        if !self.on {
            return true;
        }
        if self.flag == Flag::Low || self.value < self.rates.jump {
            self.counts.jump_low += 1;
            return false;
        }
        self.spend(self.rates.jump);
        true
    }

    /// A bullet or pellet takes `loss` from the bar.
    pub(crate) fn hit(&mut self, loss: f32) {
        if self.on && loss > 0.0 {
            self.spend(loss);
        }
    }
}

/// The counts of every body, summed.
pub(crate) fn counts_of<'a>(bodies: impl Iterator<Item = &'a Stamina>) -> Counts {
    bodies.fold(Counts::default(), |mut sum, s| {
        sum.lockouts += s.counts.lockouts;
        sum.low_ticks += s.counts.low_ticks;
        sum.sprint_keys_low += s.counts.sprint_keys_low;
        sum.jump_low += s.counts.jump_low;
        sum.sprint_refused += s.counts.sprint_refused;
        sum
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamina(rates: Rates) -> Stamina {
        Stamina {
            value: MAX,
            flag: Flag::Fine,
            last_sprint: None,
            last_tick: 0,
            rates,
            item: "",
            on: true,
            counts: Counts::default(),
        }
    }

    /// Runs `ticks` of 50 ms with the sprint keys as given; the time of the
    /// last.
    fn run(s: &mut Stamina, from: u32, ticks: u32, sprint: bool) -> u32 {
        let mut now = from;
        for _ in 0..ticks {
            now += 50;
            s.tick(now, sprint, sprint);
        }
        now
    }

    fn rates(t: f32) -> Rates {
        Rates {
            spend: SPRINT_SPEND_PER_S * (1.0 + 1.5 * t),
            regen: REGEN_PER_S,
            jump: JUMP_COST * (1.0 + 1.5 * t),
            can_sprint: true,
            weight_cost: t,
        }
    }

    /// A light kit sprints 20 s from a full bar and the flag is set at the
    /// tick that empties it; a heavy one (t = 1) in 8 s, t = 0.5 in 11.4 s.
    #[test]
    fn a_bar_empties_in_twenty_seconds_light_and_eight_heavy() {
        for (t, ticks) in [(0.0, 400), (0.5, 229), (1.0, 160)] {
            // To a tick either side (the bar is a float).
            let mut s = stamina(rates(t));
            let now = run(&mut s, 0, ticks - 2, true);
            assert!(!s.low(), "t {t} at {} ticks", ticks - 2);
            run(&mut s, now, 3, true);
            assert!(s.low(), "t {t} at {} ticks", ticks + 1);
            assert_eq!(s.counts.lockouts, 1);
        }
    }

    /// Held through the lockout the bar stays 0, the flag stays and nothing
    /// regenerates; let go at 0, the first tick regenerates (the bar is
    /// empty), and then the wait of a second after the last spending holds
    /// it (the same law as a hit's 0.6 left for 400 ms), after which the flag
    /// clears over 20: about 3 s in all, not the 2.05 s of grid's
    /// specification (7.0), which reads the zero test as holding all along.
    #[test]
    fn a_held_key_keeps_the_lockout_and_a_released_one_ends_it_in_two_seconds() {
        let mut s = stamina(Rates::LIGHT);
        let now = run(&mut s, 0, 400, true);
        assert!(s.low());
        let now = run(&mut s, now, 200, true);
        assert!(s.low() && s.value() == 0.0, "{}", s.value());
        assert!((199..=201).contains(&s.counts.sprint_keys_low), "{}", s.counts.sprint_keys_low);
        let mut ticks = 0;
        let mut at = now;
        while s.low() {
            at = run(&mut s, at, 1, false);
            ticks += 1;
            assert!(ticks < 100);
        }
        assert!((58..=61).contains(&ticks), "1 s wait and 2 s of regeneration: {ticks} ticks, {}", s.value());
    }

    /// Regeneration waits a second after the last spending, then runs at 10
    /// a second; the spending's time is the previous tick's.
    #[test]
    fn regeneration_waits_a_second_after_the_last_spending() {
        let mut s = stamina(Rates::LIGHT);
        let now = run(&mut s, 0, 100, true);
        assert!((s.value() - 75.0).abs() < 1e-3, "{}", s.value());
        // The last spending is the previous tick's time, 4,950 ms.
        let waited = run(&mut s, now, 19, false);
        assert!((s.value() - 75.0).abs() < 1e-3, "no gain inside the wait: {}", s.value());
        run(&mut s, waited, 1, false);
        assert!(s.value() > 75.0, "gain after it: {}", s.value());
    }

    /// A jump costs 10 (17.5 at t = 0.5) from a bar that has it and no flag;
    /// a refused one spends nothing, a spent one restarts the wait.
    #[test]
    fn a_jump_costs_its_amount_and_is_refused_under_it() {
        let mut s = stamina(Rates::LIGHT);
        s.value = 30.0;
        assert!(s.jump());
        assert!((s.value() - 20.0).abs() < 1e-3);
        assert!(s.last_sprint.is_some());
        let mut heavy = stamina(rates(0.5));
        heavy.value = 30.0;
        assert!(heavy.jump());
        assert!((heavy.value() - 12.5).abs() < 1e-3);
        let mut low = stamina(Rates::LIGHT);
        low.value = 9.9;
        assert!(!low.jump());
        assert_eq!((low.value(), low.last_sprint, low.counts.jump_low), (9.9, None, 1));
        let mut flagged = stamina(Rates::LIGHT);
        flagged.flag = Flag::Low;
        flagged.value = 50.0;
        assert!(!flagged.jump());
        assert_eq!(flagged.value(), 50.0);
    }

    /// A hit that empties the bar does not set the flag (the regeneration
    /// runs first), and the wait restarts.
    #[test]
    fn a_hit_that_empties_the_bar_does_not_set_the_flag() {
        let mut s = stamina(Rates::LIGHT);
        run(&mut s, 0, 2, false);
        s.hit(100.0);
        assert_eq!(s.value(), 0.0);
        run(&mut s, 100, 1, false);
        assert!(!s.low(), "{}", s.value());
        assert!(s.value() > 0.0 && s.value() < 1.0, "{}", s.value());
        // Then the wait of a second holds it (a shotgun hit left 0.6 for 400 ms).
        let at = run(&mut s, 150, 5, false);
        assert!(s.value() < 1.0, "held by the wait: {}", s.value());
        run(&mut s, at, 25, false);
        assert!(s.value() > 1.0, "regenerating after it: {}", s.value());
    }

    /// An overburdened body does not move at sprint.
    #[test]
    fn an_overburdened_body_may_not_sprint() {
        let s = stamina(Rates { can_sprint: false, ..Rates::LIGHT });
        assert!(!s.may_sprint());
        assert!(stamina(Rates::LIGHT).may_sprint());
    }

    /// The bots' kits: every weapon of every tier's kits has rates in the
    /// range the weight and the skills give, and none is overburdened.
    #[test]
    fn the_bots_kits_have_rates_and_none_is_overburdened() {
        for tier in BotTier::ALL {
            for role in BotRole::ALL {
                for arms in Arms::kits_of(BotEquipment::of(tier, RusherKits::default()), role) {
                    let rates = Rates::of(tier, arms.primary.0.item);
                    assert!(rates.can_sprint, "{tier:?} {role:?}");
                    // 5 a second at the lightest, 2.5 times at the capacity, less the skills.
                    assert!((4.4..=12.6).contains(&rates.spend), "{tier:?} {role:?}: {}", rates.spend);
                    assert!((rates.jump / rates.spend - 2.0).abs() < 1e-3);
                    assert!((9.9..=11.1).contains(&rates.regen), "{}", rates.regen);
                }
            }
        }
        assert_eq!(Rates::of(BotTier::Low, "no_such_weapon"), Rates::LIGHT);
    }

    /// The bot's own reckoning at its kit's rates (`think.stamina_rates` kit, `bots::stamina::Rates::of_kit`) works
    /// the body's rates: for each weapon, the first kit that has it (the kit these rates are of).
    #[test]
    fn the_bots_reckoning_at_its_kits_rates_is_the_bodys() {
        use crate::v026g::bots::stamina;

        let roles = [BotRole::Camper, BotRole::Flanker, BotRole::Support, BotRole::Rusher];
        for tier in BotTier::ALL {
            let equipment = BotEquipment::of(tier, RusherKits::default());
            let mut seen = Vec::new();
            for role in roles {
                let kits = bot_kits(equipment, role);
                let modifiers = bot_skills::bot_modifiers(&kits);
                for (k, kit) in kits.iter().enumerate() {
                    let Some(arms) = Arms::of_kit(equipment, role, k) else {
                        continue;
                    };
                    let item = arms.primary.0.item;
                    if seen.contains(&item) {
                        continue;
                    }
                    seen.push(item);
                    let body = Rates::of(tier, item);
                    let bot = stamina::Rates::of_kit(kit, &modifiers);
                    assert_eq!((bot.spend, bot.regen, bot.jump), (body.spend, body.regen, body.jump), "{tier:?} {role:?} {k} {item}");
                }
            }
        }
    }

    /// In the game: a body that holds SPRINT and FORWARD runs at the sprint
    /// pace until its bar is empty, then walks with the keys held, and a
    /// jump pressed while the flag is set is refused.
    #[test]
    fn a_game_body_runs_until_the_bar_is_empty_then_walks() {
        use super::super::{Game, soak_world};
        use crate::v026g::bots::guns::Arms;
        use crate::v026g::drive::key;
        use survarium::v026g::match_msgs::TICK_MS;

        let mut game = Game::new(soak_world(), &[0], 1, None, None);
        let gun = Arms::fallback().primary.0;
        game.bodies[0].stamina = Stamina::working(true, 0, super::super::bench_tier(), gun.item);
        let (feet, node) = (game.bodies[0].feet, game.bodies[0].node);
        let keys = key::FORWARD | key::SPRINT;
        let mut paces = Vec::new();
        for _ in 0..500 {
            let at = game.tick + TICK_MS;
            game.inputs[0].push_back((at, keys, (0.0, 0.0), None, gun));
            game.step();
            let body = &mut game.bodies[0];
            paces.push(body.step_m / super::super::TICK_S);
            // Back to the start so the field does not run out.
            body.feet = feet;
            body.node = node;
        }
        let rate = game.bodies[0].stamina.value();
        assert!(game.bodies[0].stamina.low() || rate < 100.0, "spent");
        let run = paces[20..60].iter().sum::<f32>() / 40.0;
        let walk = paces[450..].iter().sum::<f32>() / 50.0;
        assert!(run > 4.5 && walk < 4.0 && walk > 3.0, "run {run:.2} m/s, walk {walk:.2} m/s");
        assert!(game.bodies[0].stamina.low(), "locked out at 25 s of sprint keys");
        // A jump pressed in the lockout is refused (and counted).
        let before = (game.jumps_refused, game.bodies[0].stamina.counts.jump_low);
        let at = game.tick + TICK_MS;
        game.inputs[0].push_back((at, keys | key::JUMP, (0.0, 0.0), None, gun));
        game.step();
        assert_eq!(game.jumps_refused, before.0 + 1);
        assert_eq!(game.bodies[0].stamina.counts.jump_low, before.1 + 1);
    }

    /// Off: no spending, no flag, every jump goes.
    #[test]
    fn the_off_switch_makes_the_stamina_nothing() {
        let mut s = stamina(Rates::LIGHT);
        s.on = false;
        run(&mut s, 0, 1_000, true);
        assert!(s.may_sprint() && !s.low() && s.value() == MAX);
        s.value = 0.0;
        assert!(s.jump());
    }
}

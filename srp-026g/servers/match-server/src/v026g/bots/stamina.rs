//! The bot's own reckoning of its stamina bar (B9 S7): the world carries no stamina, so the bot works the client's
//! law from the keys it sent, as the stand-in's body does in the bench (`tests/standin_stamina.rs`, from #557;
//! `player_stamina::tick` 0x329790, `regenerate` 0x3298C0, `spend` 0x329A20, `sprint` 0x329A70, the jump's gate
//! 0x2FC230; PROVEN unless marked):
//! - the bar is 0-100; it is spent at [`SPRINT_SPEND_PER_S`] a second only while the body IS in the sprint state
//!   (`player_stamina::tick` 0x329790 asks the item's `is_sprinting`, PROVEN: `docs/notes/sprint_state.md`, A364 P10): the sprint
//!   keys (SPRINT and FORWARD, none of BACK, FIRE, CROUCH, AIM) and `can_sprint` 0x2DBF40 letting it (the low flag clear, not a
//!   carrier, not in a reload), and not at a jump's press (the state ends at the press). A refused sprint costs nothing;
//! - not sprinting, the bar regenerates at [`REGEN_PER_S`] when it is empty or more than [`WAIT_MS`] after the last
//!   spending (a sprint, a jump or a hit);
//! - the low flag is set when the bar is at 0 after a tick and cleared over [`CLEAR_ABOVE`]: a lockout takes about 3 s to
//!   clear (the first tick regenerates because the bar is empty, the wait holds it a second, then 2 s of regeneration), the
//!   sprint keys held or not (the flag refuses the state, so nothing is spent): the game's heaviest body (32.9 kg) sprints,
//!   walks about 3 s, sprints again (A364, `private/grid/rig/sprint-census.txt`);
//! - a jump needs the flag clear and the bar at [`JUMP_COST`], spent at the press; a refused press spends nothing.
//!
//! The rates are the light kit's (no weight cost, no skills, [`Rates::LIGHT`]) unless `think.stamina_rates` is `kit`:
//! then a life's are its kit's ([`Rates::of_kit`]), the client's law of the weight's cost `t` and the skills: spending and
//! the jump's cost `x (1 + 1.5 t) x (1 + skill 201)`, regeneration `x (1 + skill 204)`, 4.4-12.6 a second across the bots'
//! kits (A207: the light rates let a heavy kit's real bar reach 0 with the reckoning still over the reserve). A hit's own
//! stamina damage is not reckoned (the world's hit time only restarts the wait). Counted whether the switch
//! (`think.stamina`) is on or off ([`counts`]); the motor acts on it only when on.

use survarium::v026g::bot_skills;
use survarium::v026g::loadouts::Loadout;
use survarium::v026g::skills::{MODIFIERS, modifier_index};

use crate::v026g::drive::key;

use super::kin::Kinematics;

pub const MAX: f32 = 100.0;
pub const SPRINT_SPEND_PER_S: f32 = 5.0;
pub const REGEN_PER_S: f32 = 10.0;
pub const JUMP_COST: f32 = 10.0;
/// Regeneration waits this long after the last spending, ms.
pub const WAIT_MS: u32 = 1_000;
/// The low flag is set at a bar of 0 and cleared over this.
pub const CLEAR_ABOVE: f32 = 20.0;
/// A sprint covers this much ground a second, m/s (`kin::Kinematics::default().sprint`, F5: the body's on traced steps).
pub const SPRINT_MPS: f32 = 4.9587;
/// What a sprint spends a metre.
pub const SPEND_PER_M: f32 = SPRINT_SPEND_PER_S / SPRINT_MPS;
/// The bar a sprint jump wants at its take-off over what its run-up spends: the jump's cost and a margin, so a
/// reckoning that is a little off still has it.
pub const JUMP_MARGIN: f32 = 5.0;

/// What a kit makes of the bar's law: spent a second of sprint keys, regained a second, and a jump's cost (also the
/// least the bar needs for one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rates {
    pub spend: f32,
    pub regen: f32,
    pub jump: f32,
}

impl Rates {
    /// A kit of no weight cost and no skills.
    pub const LIGHT: Self = Self { spend: SPRINT_SPEND_PER_S, regen: REGEN_PER_S, jump: JUMP_COST };

    /// The rates of `kit` worn with `modifiers` (the bot's skills, `bot_skills::bot_modifiers` of its kits): the weight's
    /// cost `t` of the kit against its capacity, as the bench's stand-in body works it (`tests/standin_stamina.rs`).
    pub fn of_kit(kit: &Loadout, modifiers: &[f32; MODIFIERS]) -> Self {
        let index = |name| modifier_index(name).expect("a modifier of the 68");
        let t = bot_skills::weight_cost(kit.weight(), bot_skills::carry_capacity(kit, modifiers));
        let carried = 1.0 + survarium::v026g::item_params::player().max_carried_weight_spending_speed_modifier * t;
        let spending = 1.0 + modifiers[index("stamina_spending_speed_skill")];
        Self {
            spend: SPRINT_SPEND_PER_S * carried * spending,
            regen: REGEN_PER_S * (1.0 + modifiers[index("stamina_regeneration_speed")]),
            jump: JUMP_COST * carried * spending,
        }
    }

    /// What a sprint spends a metre.
    pub fn spend_per_m(&self) -> f32 {
        self.spend / SPRINT_MPS
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Flag {
    #[default]
    Fine,
    Low,
}

/// One bot's reckoned bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Reckoning {
    value: f32,
    flag: Flag,
    /// The time of the last spending (a sprint tick, a jump, a hit), ms; none since a spawn.
    last_spend: Option<u32>,
    last_tick: u32,
    /// The keys of the last tick, for the jump's edge.
    keys: u32,
    /// The life's kit's ([`Reckoning::set_rates`]), else the light kit's.
    rates: Rates,
    /// [F1] The ticks since the oldest world that may still come, with the bar after each: a world's saved record
    /// ([`Reckoning::saved`]) is the bar at its time, and the ticks after it are played again from there.
    steps: std::collections::VecDeque<Step>,
}

/// [F1] One tick as [`Reckoning::tick`] took it, and the bar after it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Step {
    now: u32,
    keys: u32,
    carrying: bool,
    refused: bool,
    value: f32,
}

/// [F1] Ticks kept for a late world: a world reaches the bots at most its interval (1,000 ms: worlds come once a second) plus
/// the bots' 300 ms lead after its time, 26 ticks of 50 ms; 64 keep 3.2 s, more than twice that.
const STEPS_KEPT: usize = 64;

impl Default for Reckoning {
    fn default() -> Self {
        Self {
            value: MAX,
            flag: Flag::Fine,
            last_spend: None,
            last_tick: 0,
            keys: 0,
            rates: Rates::LIGHT,
            steps: std::collections::VecDeque::new(),
        }
    }
}

/// [F1] The bar as the bot's OWN saved record has it in a world (`player_stamina::serialize` 0x329310, read by
/// `srp_validate::world::Stamina`), parsed once at the boundary: its value, its low flag and its last spending moved onto
/// the world's clock. The client's bar is this; a hit's stamina damage, the take-off's spending and the weight's cost are in
/// it, which the reckoning between worlds does not know (exactness S5: the reckoned bar was right in 85.6 % of worlds before
/// a hit, 62 % after; 20 or more off in 23,553 of 112,013).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Saved {
    /// The world's time, ms.
    pub at: u32,
    pub value: f32,
    pub low: bool,
    /// The last spending on the world's clock; none when the record has none (0).
    pub last_spend: Option<u32>,
}

impl Saved {
    /// The record of a world of `at`: `value` (`m_value`, held to 0..=[`MAX`]), the low flag, and the record's own clock
    /// (`m_current_time_in_ms`) and last spending (`m_last_spending_time_in_ms`, 0 for none), the spending put on the
    /// world's clock by its age on the record's.
    pub fn of(at: u32, value: f32, low: bool, current_ms: u32, last_spending_ms: u32) -> Self {
        let last_spend = (last_spending_ms != 0).then(|| at.saturating_sub(current_ms.saturating_sub(last_spending_ms)));
        Self { at, value: value.clamp(0.0, MAX), low, last_spend }
    }
}

/// What the motor is told of the bar, when the switch is on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gauge {
    pub value: f32,
    pub low: bool,
    pub rates: Rates,
}

impl Reckoning {
    /// A new life: a full bar, at the light kit's rates until [`Reckoning::set_rates`].
    pub fn spawn(&mut self, now: u32) {
        *self = Self { last_tick: now, ..Self::default() };
    }

    /// [F1] A world's saved record of the bar: the bar at its time is the record's, and the ticks the bot took since are
    /// played again on it. A record older than every kept tick, or of a time before the life, sets the bar as it is now.
    pub fn saved(&mut self, saved: Saved) {
        let reckoned = self.steps.iter().rev().find(|step| step.now <= saved.at).map(|step| step.value);
        if let Some(reckoned) = reckoned {
            counts::saved_world((reckoned - saved.value).abs());
        }
        let keys = self.steps.iter().rev().find(|step| step.now <= saved.at).map_or(self.keys, |step| step.keys);
        let after: Vec<Step> = self.steps.iter().filter(|step| step.now > saved.at).copied().collect();
        if saved.at < self.last_tick && reckoned.is_none() {
            return;
        }
        self.value = saved.value;
        self.flag = if saved.low { Flag::Low } else { Flag::Fine };
        self.last_spend = saved.last_spend;
        self.last_tick = saved.at;
        self.keys = keys;
        self.steps.retain(|step| step.now <= saved.at);
        for step in after {
            self.tick(step.now, step.keys, step.carrying, step.refused);
        }
    }

    /// The life's rates (its kit's, `think.stamina_rates`).
    pub fn set_rates(&mut self, rates: Rates) {
        self.rates = rates;
    }

    pub fn value(&self) -> f32 {
        self.value
    }

    pub fn low(&self) -> bool {
        self.flag == Flag::Low
    }

    pub fn of(gauge: Gauge) -> Self {
        Self { value: gauge.value, flag: if gauge.low { Flag::Low } else { Flag::Fine }, rates: gauge.rates, ..Self::default() }
    }

    pub fn gauge(&self) -> Gauge {
        Gauge { value: self.value, low: self.low(), rates: self.rates }
    }

    /// [A364 P10] Whether the body is in the client's sprint state with `keys`: the sprint keys, `can_sprint` letting it (the low
    /// flag clear, no carrier, nothing `refused`) and no jump's press in them (the state ends at the press, PROVEN).
    pub fn sprinting(&self, keys: u32, carrying: bool, refused: bool) -> bool {
        let press = keys & key::JUMP != 0 && self.keys & key::JUMP == 0;
        Kinematics::sprints(keys) && !carrying && !refused && self.flag == Flag::Fine && !press
    }

    /// The jump goes: the flag clear and the bar at its cost.
    pub fn may_jump(&self) -> bool {
        self.flag == Flag::Fine && self.value >= self.rates.jump
    }

    /// One tick at `now` ms, with the keys the bot sent in the tick before (`keys`), whether it carries and whether its own
    /// `can_sprint` refuses (`refused`: a reload). A jump pressed in them (a new press: not held in the tick before that) spends
    /// its cost if it went. The bar is spent only in the sprint state ([`Reckoning::sprinting`]).
    pub fn tick(&mut self, now: u32, keys: u32, carrying: bool, refused: bool) {
        let dt = now.saturating_sub(self.last_tick) as f32 / 1_000.0;
        let held = Kinematics::sprints(keys);
        if self.sprinting(keys, carrying, refused) {
            self.value = (self.value - self.rates.spend * dt).max(0.0);
            self.last_spend = Some(self.last_tick);
        } else if self.value < MAX && (self.value == 0.0 || self.last_spend.is_none_or(|at| now > at + WAIT_MS)) {
            self.value = (self.value + self.rates.regen * dt).min(MAX);
        }
        if keys & key::JUMP != 0 && self.keys & key::JUMP == 0 {
            if self.may_jump() {
                self.value -= self.rates.jump;
                self.last_spend = Some(self.last_tick);
            } else {
                counts::jump_refused();
            }
        }
        self.keys = keys;
        self.last_tick = now;
        self.steps.push_back(Step { now, keys, carrying, refused, value: self.value });
        while self.steps.len() > STEPS_KEPT {
            self.steps.pop_front();
        }
        match self.flag {
            Flag::Fine if self.value <= 0.0 => {
                self.flag = Flag::Low;
                counts::lockout();
            }
            Flag::Low if self.value > CLEAR_ABOVE => self.flag = Flag::Fine,
            _ => {}
        }
        if self.flag == Flag::Low {
            counts::low_tick(held);
        }
    }

    /// Whether a sprint may go on with a sprint-needing segment `ahead_m` metres on along the route (a sprint jump:
    /// its run-up of `run_up_m` is sprinted too): the bar, less what the sprint to the segment spends, still pays the
    /// jump, its run-up and the margin, at the life's rates. Walking (not spending) the bar comes back after the wait,
    /// so a sprint dropped early arrives with more.
    pub fn covers(&self, ahead_m: f32, run_up_m: f32) -> bool {
        self.value - self.rates.spend_per_m() * (ahead_m + run_up_m) >= self.rates.jump + JUMP_MARGIN
    }
}

/// The bench's counts (all matches): lockouts begun, ticks a body was locked out, of those with the sprint keys
/// held (the client walks them: A364 P10), jump presses the bar refused, sprints let go at the empty
/// bar, sprint segments that were short of the bar when first seen, the ticks a tired body was kept walking into a
/// sprint jump, the sprint gap segments short only by the gap's run, and the ms of sprint saved for them.
pub mod counts {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];

    pub fn lockout() {
        COUNTS[0].fetch_add(1, Ordering::Relaxed);
    }

    pub fn low_tick(sprint_keys: bool) {
        COUNTS[1].fetch_add(1, Ordering::Relaxed);
        COUNTS[2].fetch_add(u64::from(sprint_keys), Ordering::Relaxed);
    }

    pub fn jump_refused() {
        COUNTS[3].fetch_add(1, Ordering::Relaxed);
    }

    pub fn released_low() {
        COUNTS[4].fetch_add(1, Ordering::Relaxed);
    }

    pub fn short_segment() {
        COUNTS[5].fetch_add(1, Ordering::Relaxed);
    }

    /// A tick a tired body, walking into a sprint jump, was kept walking (the sprint is not taken up at the bar's
    /// empty or low).
    pub fn sprint_jump_kept_walking() {
        COUNTS[6].fetch_add(1, Ordering::Relaxed);
    }

    /// A sprint gap segment seen short of the bar only because the gap's own run is priced ([`super::Reckoning::covers`]).
    pub fn gap_run_short() {
        COUNTS[7].fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> [u64; 8] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }

    static SAVED_MS: AtomicU64 = AtomicU64::new(0);

    pub fn saved(ms: u32) {
        SAVED_MS.fetch_add(u64::from(ms), Ordering::Relaxed);
    }

    pub fn saved_ms() -> u64 {
        SAVED_MS.load(Ordering::Relaxed)
    }

    /// [F1] Worlds whose saved bar set the reckoning, and of them those the reckoning was 5 and 20 or more off.
    static SAVED: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];

    pub fn saved_world(off: f32) {
        SAVED[0].fetch_add(1, Ordering::Relaxed);
        SAVED[1].fetch_add(u64::from(off >= 5.0), Ordering::Relaxed);
        SAVED[2].fetch_add(u64::from(off >= 20.0), Ordering::Relaxed);
    }

    pub fn saved_worlds() -> [u64; 3] {
        std::array::from_fn(|k| SAVED[k].load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(r: &mut Reckoning, from: u32, ms: u32, keys: u32) -> u32 {
        let mut now = from;
        while now < from + ms {
            now += 50;
            r.tick(now, keys, false, false);
        }
        now
    }

    /// [F1] A world's saved record sets the bar at its time and the ticks since are played again on it: a sprint of 1 s after
    /// a world of 1,000 ms that saved 30 (a hit's drain the reckoning did not know) leaves 25, not the reckoned 95; the record's
    /// last spending is put on the world's clock by its age on its own.
    #[test]
    fn a_saved_record_sets_the_bar_and_the_ticks_since_are_played_again() {
        let sprint = key::FORWARD | key::SPRINT;
        let mut r = Reckoning::default();
        r.spawn(0);
        let now = run(&mut r, 0, 1_000, 0);
        let now = run(&mut r, now, 1_000, sprint);
        assert!((r.value() - 95.0).abs() < 1e-3, "reckoned {}", r.value());
        r.saved(Saved::of(1_000, 30.0, false, 51_000, 50_900));
        assert!((r.value() - 25.0).abs() < 1e-3, "from the record: {}", r.value());
        assert_eq!(now, 2_000);
        assert_eq!(Saved::of(1_000, 30.0, false, 51_000, 50_900).last_spend, Some(900));
        assert_eq!(Saved::of(1_000, 130.0, true, 51_000, 0), Saved { at: 1_000, value: MAX, low: true, last_spend: None });
        // A record's low flag holds the sprint off until it clears.
        let mut low = Reckoning::default();
        low.spawn(0);
        run(&mut low, 0, 500, 0);
        low.saved(Saved::of(500, 0.0, true, 500, 450));
        assert!(low.low() && !low.sprinting(sprint, false, false));
    }

    /// The law: a sprint of 20 s empties the bar and sets the flag; the flag clears in about 3 s (one regenerating tick, the
    /// 1 s wait, then 2 s), the sprint keys held or not (A364 P10: the flag refuses the sprint state, nothing is spent); a jump
    /// needs the flag clear and 10 of the bar.
    #[test]
    fn the_bar_is_spent_by_a_sprint_and_a_lockout_clears_in_about_three_seconds() {
        let sprint = key::FORWARD | key::SPRINT;
        let mut r = Reckoning::default();
        let now = run(&mut r, 0, 19_000, sprint);
        assert!(r.value() > 0.0 && !r.low(), "{} {}", r.value(), r.low());
        let now = run(&mut r, now, 1_000, sprint);
        assert!(r.low() && r.value() <= 1e-3, "{} {}", r.value(), r.low());
        // Held on: it clears all the same, in about 3 s, and the sprint spends again.
        let mut held = r.clone();
        let (mut t, mut seconds) = (now, 0.0);
        while held.low() && seconds < 10.0 {
            t += 50;
            seconds += 0.05;
            held.tick(t, sprint, false, false);
        }
        assert!((2.8..3.4).contains(&seconds), "held: {seconds}");
        let before = held.value();
        held.tick(t + 50, sprint, false, false);
        assert!(held.value() < before, "sprinting again spends");
        // Let go: clear after about 3 s.
        let mut t = now;
        let mut seconds = 0.0;
        while r.low() && seconds < 10.0 {
            t += 50;
            seconds += 0.05;
            r.tick(t, 0, false, false);
        }
        assert!((2.8..3.4).contains(&seconds), "{seconds}");
        // A jump press spends 10; one at a bar under 10 is refused and spent nothing.
        let mut full = Reckoning::default();
        full.tick(50, key::JUMP, false, false);
        assert!((full.value() - (MAX - JUMP_COST)).abs() < 1e-3, "{}", full.value());
        let mut empty = Reckoning { value: 4.0, ..Reckoning::default() };
        empty.tick(50, key::JUMP, false, false);
        assert!(empty.value() >= 4.0);
    }

    /// A sprint jump `ahead_m` away wants the bar to pay the sprint there, its run-up, the jump and the margin.
    #[test]
    fn a_segment_is_covered_when_the_bar_pays_the_sprint_to_it_and_the_jump() {
        let r = Reckoning::default();
        assert!(r.covers(30.0, 2.0));
        let low = Reckoning { value: 40.0, ..Reckoning::default() };
        assert!(low.covers(10.0, 2.0));
        assert!(!low.covers(25.0, 2.0));
    }

    /// At a heavy kit's rates a held sprint empties the bar sooner, a jump costs more, and a segment is covered later.
    #[test]
    fn a_heavy_kits_bar_empties_sooner() {
        let sprint = key::FORWARD | key::SPRINT;
        let heavy = Rates { spend: 12.5, regen: 10.0, jump: 25.0 };
        let mut light = Reckoning::default();
        let mut kit = Reckoning::default();
        kit.set_rates(heavy);
        run(&mut light, 0, 8_000, sprint);
        run(&mut kit, 0, 8_000, sprint);
        assert!(!light.low() && (light.value() - 60.0).abs() < 1.0, "{}", light.value());
        assert!(kit.low() && kit.value() <= 1e-3, "{}", kit.value());
        let mut jump = Reckoning::default();
        jump.set_rates(heavy);
        jump.tick(50, key::JUMP, false, false);
        assert!((jump.value() - (MAX - 25.0)).abs() < 1e-3, "{}", jump.value());
        let half = Reckoning { value: 50.0, rates: heavy, ..Reckoning::default() };
        assert!(Reckoning { value: 50.0, ..Reckoning::default() }.covers(20.0, 2.0));
        assert!(!half.covers(20.0, 2.0));
        assert_eq!(Reckoning::of(half.gauge()).rates, heavy);
    }
}

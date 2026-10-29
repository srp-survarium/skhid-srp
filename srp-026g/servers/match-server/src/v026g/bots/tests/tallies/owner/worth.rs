//! The owner counts of what the bots do with a seen enemy's worth (`bots/worth.rs`; the fights analysis of
//! 2026-10-28, R1 and R2): a carrier who cannot shoot back, an enemy hurt to pain, a battery dropped beside a
//! hurt enemy, and how the fights go for the one who drew blood first.
//!
//! - **carrier in clear sight**: bot-ticks in which an enemy bot within [`SEE_M`] has a living carrier in a clear
//!   line (eye to chest), and those of them in which that bot's target was another (or none). The Cologne steal:
//!   14 clear bot-seconds in a 22 s carry, half of them on his escorts.
//! - **hurt, lived 15 s more**: a body whose pain fell under [`HURT_PAIN`] for the first time in its life with an
//!   enemy bot within [`HURT_NEAR_M`], and whether it was alive [`LIVE_MS`] later. Lower is better for the bots.
//! - **drops by a hurt carrier**: a battery let go alive by a body whose pain is under [`DROP_HURT_PAIN`]: how
//!   long until a body of the other team (its owners) carries it again, or until the thief's side does.
//! - **switches off a hurter**: target switches while an enemy who hurt the bot in the last [`super::fight::HURT_MS`] is in
//!   its line and is not the new target (the fights topic's count), split by whether the switch was a target taken for its
//!   worth that tick (`Reflex::worth_stats` rose) or any other: the guard "switches off the hurter" of the smoke compare.
//! - **blood first**: bot-vs-bot exchanges (hits between a pair, [`EXCHANGE_GAP_MS`] apart at most), by who hit
//!   first: how often the one hit died and how often the one who hit died, up to [`EXCHANGE_END_MS`] after the last
//!   hit (an-fights `engage.py`: 66 % of those hit first died, bot against bot, on the owner's matches).

use super::*;
use crate::v026g::bots::worth::WorthStats;

/// A bot sees a carrier from this far, m.
pub(crate) const SEE_M: f32 = 40.0;

/// Pain under this is hurt, as a share of the pool.
pub(crate) const HURT_PAIN: f32 = 0.3;

/// An enemy bot this near knew about a hurt body, m.
pub(crate) const HURT_NEAR_M: f32 = 30.0;

/// The body is counted as living on if it is alive this long after, ms.
pub(crate) const LIVE_MS: u32 = 15_000;

/// A carrier who lets go of a battery alive with pain under this is hurt.
pub(crate) const DROP_HURT_PAIN: f32 = 0.5;

/// A dropped battery not carried again in this is counted as left, ms.
pub(crate) const RETAKE_MS: u32 = 20_000;

/// A target switch within this of the reflex taking a target for its worth is the worth's, ms (the game's target follows the
/// reflex's by a tick or two).
pub(crate) const WORTH_TO_TARGET_MS: u32 = 600;

/// Hits of a pair further apart than this are two exchanges, ms.
pub(crate) const EXCHANGE_GAP_MS: u32 = 8_000;

/// A death this long after an exchange's last hit is its, ms.
pub(crate) const EXCHANGE_END_MS: u32 = 2_000;

#[derive(Clone, Copy)]
struct HurtOne {
    who: Place,
    spawned: u32,
    at: u32,
}

#[derive(Clone, Copy)]
struct Dropped {
    battery: usize,
    at: u32,
    team: u8,
}

#[derive(Default)]
pub(super) struct WorthCounts {
    /// By place: the target at the last tick, the worth takings (threat + actor + hurt) at the last reading and the tick
    /// they last rose (the target a tick or two after the reflex took it), and the enemies who hurt it lately
    /// (shooter, tick).
    last_target: Vec<Option<Place>>,
    taken: Vec<u32>,
    taken_at: Vec<Option<u32>>,
    hurt_by: Vec<Vec<(Place, u32)>>,
    /// Target switches [for the worth, otherwise], and those of them with a hurter in the line who is not the new target.
    switches: [u32; 2],
    switches_off_hurter: [u32; 2],
    /// Bot-ticks with a target and a hurter in the line who is not that target, and those of them in which the bot has the
    /// hurter in sight (its memory's live sights): a hurter it sees is one it could keep, one it does not see is one it
    /// does not know of.
    chances: u32,
    chances_seen: u32,
    switches_off_seen_hurter: [u32; 2],
    /// ... by the age of the hurter's last hit at the switch (the world reaches the bot late): under 1 s, 1 to 2 s, over 2 s; for
    /// the worth and other.
    off_seen_hurter_age: [[u32; 3]; 2],
    /// By place: the enemies in the bot's live sights at the last reading.
    sighted: Vec<Vec<Place>>,
    /// Bot-ticks with a living carrier in a clear line of an enemy bot, and those with another target.
    carrier_clear: [u32; 2],
    /// By place: the life already counted as hurt (`Body::spawned`).
    marked: Vec<Option<u32>>,
    hurt: Vec<HurtOne>,
    hurt_n: u32,
    hurt_lived: u32,
    /// By place: the battery it carried at the last tick.
    carried: Vec<Option<usize>>,
    dropped: Vec<Dropped>,
    drops_n: u32,
    /// Ms until the battery was carried again by the owners' team / by the thief's team; left.
    retaken_ms: Vec<u32>,
    thief_ms: Vec<u32>,
    drops_left: u32,
    /// The switches' own counts (`Reflex::worth_stats`, one reflex a life): the lives that ended, and by place the
    /// life under way.
    banked: WorthStats,
    now: Vec<Option<WorthStats>>,
}

impl Topic for WorthCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, tick: u32) {
        if self.now.len() <= place {
            self.now.resize(place + 1, None);
        }
        let Some(bot) = bot else {
            return;
        };
        let stats = bot.reflex.worth_stats();
        if self.sighted.len() <= place {
            self.sighted.resize(place + 1, Vec::new());
        }
        self.sighted[place] = bot.memory.live_sights().map(|(who, _)| who).collect();
        if self.taken.len() <= place {
            self.taken.resize(place + 1, 0);
            self.taken_at.resize(place + 1, None);
        }
        let taken = stats.threat + stats.actor + stats.hurt;
        if taken > self.taken[place] {
            self.taken_at[place] = Some(tick);
        }
        self.taken[place] = taken;
        // A reflex younger than the last reading is a new life: the last one is banked.
        if let Some(last) = self.now[place]
            && stats.total() < last.total()
        {
            self.banked.add(&last);
        }
        self.now[place] = Some(stats);
    }

    fn begin(&mut self, _run: &Run, game: &mut Game) {
        self.hurt_by.resize(game.bodies.len(), Vec::new());
        for &(t, shooter, victim) in &game.hit_log {
            if let Some(hurt) = self.hurt_by.get_mut(victim) {
                hurt.retain(|&(who, _)| who != shooter);
                hurt.push((shooter, t));
            }
        }
        let tick = game.tick;
        for hurt in &mut self.hurt_by {
            hurt.retain(|&(_, t)| tick <= t + super::fight::HURT_MS);
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, sight: &mapdata::RayTree, rays: &mut u64) {
        let tick = game.tick;
        let n = game.bodies.len();
        // Target switches, with a hurter in the line who is not the new target.
        self.last_target.resize(n, None);
        self.taken_at.resize(n, None);
        self.hurt_by.resize(n, Vec::new());
        for (p, body) in game.bodies.iter().enumerate() {
            let target = body.target.filter(|_| body.alive && run.is_bot.get(p).copied().unwrap_or(false));
            // A hurter in the line who is not the target: the chance a switch has to be one off a hurter.
            self.sighted.resize(n, Vec::new());
            let in_line: Vec<(Place, u32)> = target
                .map(|new| {
                    self.hurt_by[p]
                        .iter()
                        .filter(|&&(who, _)| {
                            who != new
                                && super::fight::enemy_of(game, p, body.team, who)
                                    .is_some_and(|him| super::fight::in_sight(sight, him, body, f32::INFINITY, rays))
                        })
                        .map(|&(who, at)| (who, tick.saturating_sub(at)))
                        .collect()
                })
                .unwrap_or_default();
            let hurter_in_line = !in_line.is_empty();
            let seen_age = in_line.iter().filter(|(who, _)| self.sighted[p].contains(who)).map(|&(_, age)| age).min();
            let hurter_seen = seen_age.is_some();
            self.chances += u32::from(hurter_in_line);
            self.chances_seen += u32::from(hurter_seen);
            if let (Some(was), Some(new)) = (self.last_target[p], target)
                && was != new
            {
                let for_worth = usize::from(!self.taken_at[p].is_some_and(|at| tick.saturating_sub(at) <= WORTH_TO_TARGET_MS));
                self.switches[for_worth] += 1;
                self.switches_off_hurter[for_worth] += u32::from(hurter_in_line);
                self.switches_off_seen_hurter[for_worth] += u32::from(hurter_seen);
                if let Some(age) = seen_age {
                    self.off_seen_hurter_age[for_worth][usize::from(age >= 1_000) + usize::from(age >= 2_000)] += 1;
                }
            }
            self.last_target[p] = target;
        }
        self.marked.resize(n, None);
        self.carried.resize(n, None);
        let bot = |p: Place| run.is_bot.get(p).copied().unwrap_or(false);
        // A carrier in a clear line of an enemy bot.
        for (c, carrier) in game.bodies.iter().enumerate() {
            if !carrier.alive || carrier.carrying.is_none() {
                continue;
            }
            for (e, enemy) in game.bodies.iter().enumerate() {
                if !bot(e)
                    || !enemy.alive
                    || enemy.team == carrier.team
                    || enemy.feet.distance_xz(carrier.feet) > SEE_M
                {
                    continue;
                }
                *rays += 1;
                if sight.visible(Game::eye(enemy), Game::chest(carrier)) {
                    self.carrier_clear[0] += 1;
                    self.carrier_clear[1] += u32::from(enemy.target != Some(c));
                }
            }
        }
        // A body hurt to pain, with an enemy bot near: alive 15 s later?
        for (p, body) in game.bodies.iter().enumerate() {
            if body.alive
                && body.pools.pain < HURT_PAIN
                && self.marked[p] != Some(body.spawned)
            {
                self.marked[p] = Some(body.spawned);
                let near = game.bodies.iter().enumerate().any(|(e, enemy)| {
                    bot(e)
                        && enemy.alive
                        && enemy.team != body.team
                        && enemy.feet.distance_xz(body.feet) <= HURT_NEAR_M
                });
                if near {
                    self.hurt.push(HurtOne { who: p, spawned: body.spawned, at: tick });
                }
            }
        }
        let bodies = &game.bodies;
        let (mut n_done, mut lived) = (0, 0);
        self.hurt.retain(|h| {
            let body = &bodies[h.who];
            let same_life = body.alive && body.spawned == h.spawned;
            if same_life && tick < h.at + LIVE_MS {
                return true;
            }
            n_done += 1;
            lived += u32::from(same_life);
            false
        });
        self.hurt_n += n_done;
        self.hurt_lived += lived;
        // A battery let go alive by a hurt carrier: who has it next.
        for (p, body) in game.bodies.iter().enumerate() {
            if let Some(battery) = self.carried[p]
                && body.carrying.is_none()
                && body.alive
                && body.pools.pain < DROP_HURT_PAIN
                && matches!(game.batteries.get(battery), Some(crate::v026g::bots::tests::Lying::Free(..)))
            {
                self.drops_n += 1;
                self.dropped.push(Dropped { battery, at: tick, team: body.team });
            }
            self.carried[p] = body.carrying;
        }
        let (mut retaken, mut thief, mut left) = (Vec::new(), Vec::new(), 0);
        self.dropped.retain(|d| {
            let holder = game
                .bodies
                .iter()
                .find(|b| b.alive && b.carrying == Some(d.battery));
            match holder {
                Some(h) if h.team == d.team => thief.push(tick - d.at),
                Some(_) => retaken.push(tick - d.at),
                None if tick >= d.at + RETAKE_MS => left += 1,
                None => return true,
            }
            false
        });
        self.retaken_ms.extend(retaken);
        self.thief_ms.extend(thief);
        self.drops_left += left;
    }

    fn parts(&mut self, run: &Run, _game: &Game, scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("worth", self.part(run, scale))]
    }
}

/// An exchange between two bots: who hit first, and the first hit's and the last hit's ticks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Exchange {
    first: Place,
    other: Place,
    start: u32,
    end: u32,
}

/// The exchanges of the hits `(tick, shooter, victim)` between bots, in the order of their starts.
fn exchanges(hits: &[(u32, Place, Place)], bot: impl Fn(Place) -> bool) -> Vec<Exchange> {
    let mut by_pair: BTreeMap<(Place, Place), Vec<(u32, Place)>> = BTreeMap::new();
    for &(t, shooter, victim) in hits {
        if bot(shooter) && bot(victim) {
            by_pair.entry((shooter.min(victim), shooter.max(victim))).or_default().push((t, shooter));
        }
    }
    let mut out = Vec::new();
    for ((a, b), hits) in by_pair {
        let mut open: Option<Exchange> = None;
        for (t, shooter) in hits {
            match open.as_mut() {
                Some(x) if t <= x.end + EXCHANGE_GAP_MS => x.end = t,
                _ => {
                    out.extend(open.take());
                    open = Some(Exchange { first: shooter, other: a + b - shooter, start: t, end: t });
                }
            }
        }
        out.extend(open);
    }
    out.sort_by_key(|x| x.start);
    out
}

impl WorthCounts {
    fn part(&self, run: &Run, scale: &Scale) -> Part {
        let bot = |p: Place| run.is_bot.get(p).copied().unwrap_or(false);
        let seconds = |ticks: u32| (10.0 * f64::from(ticks) * scale.tick_s).round() / 10.0;
        let mean_s = |ms: &[u32]| {
            if ms.is_empty() { 0.0 } else { (10.0 * ms.iter().map(|&m| f64::from(m)).sum::<f64>() / ms.len() as f64 / 1000.0).round() / 10.0 }
        };
        // Blood first.
        let (mut n, mut other_died, mut first_died) = (0u32, 0u32, 0u32);
        for x in exchanges(&run.hits, bot) {
            let died = |who: Place| run.kills.iter().any(|&(t, _, victim)| victim == who && t >= x.start && t <= x.end + EXCHANGE_END_MS);
            n += 1;
            other_died += u32::from(died(x.other));
            first_died += u32::from(died(x.first));
        }
        let pct = Scale::pct;
        let mut w = self.banked;
        for last in self.now.iter().flatten() {
            w.add(last);
        }
        let json = serde_json::json!({
            "worth_target_objective_threat": w.threat,
            "worth_target_objective_actor": w.actor,
            "worth_target_hurt": w.hurt,
            "worth_target_switched": w.switched,
            "worth_pushes": w.pushes,
            "worth_push_expired": w.push_expired,
            "worth_push_seen_again": w.push_seen_again,
            "worth_push_other_target": w.push_other_target,
            "worth_glances": w.glances,
            "switches_for_worth": self.switches[0],
            "switches_for_worth_off_hurter": self.switches_off_hurter[0],
            "switches_other": self.switches[1],
            "switches_other_off_hurter": self.switches_off_hurter[1],
            "hurter_in_line_bot_ticks": self.chances,
            "hurter_seen_in_line_bot_ticks": self.chances_seen,
            "switches_for_worth_off_seen_hurter": self.switches_off_seen_hurter[0],
            "switches_other_off_seen_hurter": self.switches_off_seen_hurter[1],
            "off_seen_hurter_age_worth": self.off_seen_hurter_age[0],
            "off_seen_hurter_age_other": self.off_seen_hurter_age[1],
            "carrier_clear_s": seconds(self.carrier_clear[0]),
            "carrier_clear_other_target_s": seconds(self.carrier_clear[1]),
            "carrier_clear_other_target_pct": pct(f64::from(self.carrier_clear[1]), f64::from(self.carrier_clear[0])),
            "hurt_n": self.hurt_n,
            "hurt_lived_n": self.hurt_lived,
            "hurt_lived_pct": pct(f64::from(self.hurt_lived), f64::from(self.hurt_n)),
            "hurt_drops_n": self.drops_n,
            "hurt_drops_owners_n": self.retaken_ms.len(),
            "hurt_drops_owners_mean_s": mean_s(&self.retaken_ms),
            "hurt_drops_thief_side_n": self.thief_ms.len(),
            "hurt_drops_thief_side_mean_s": mean_s(&self.thief_ms),
            "hurt_drops_left_n": self.drops_left,
            "blood_first_n": n,
            "blood_first_other_died_pct": pct(f64::from(other_died), f64::from(n)),
            "blood_first_first_died_pct": pct(f64::from(first_died), f64::from(n)),
        });
        let text = format!(
            "owner-test worth: carrier in a clear line of an enemy bot {:.1} bot-s, of them on another target {:.1} s ({:.1} %); hurt to pain {} lived {} ({:.1} %); \
             dropped by a hurt carrier {}: owners carry it again {} (mean {:.1} s), thief's side {} (mean {:.1} s), left {}; \
             blood first, {} exchanges: the one hit died {:.1} %, the one who hit died {:.1} %; \
             targets taken for their worth: objective threat {}, objective actor {}, hurt {} ({} from another target); \
             pushes {} (ended: time {}, seen again {}, another target {}); flank glances {}; \
             target switches for the worth {} ({} with a hurter in the line), other {} ({}); bot-ticks with a hurter in the line who is not the target {} (in the bot's sight {}), switches off a hurter in its sight: for the worth {} (by the age of his last hit: <1 s {}, 1-2 s {}, >2 s {}), other {} (<1 s {}, 1-2 s {}, >2 s {})",
            seconds(self.carrier_clear[0]),
            seconds(self.carrier_clear[1]),
            pct(f64::from(self.carrier_clear[1]), f64::from(self.carrier_clear[0])),
            self.hurt_n,
            self.hurt_lived,
            pct(f64::from(self.hurt_lived), f64::from(self.hurt_n)),
            self.drops_n,
            self.retaken_ms.len(),
            mean_s(&self.retaken_ms),
            self.thief_ms.len(),
            mean_s(&self.thief_ms),
            self.drops_left,
            n,
            pct(f64::from(other_died), f64::from(n)),
            pct(f64::from(first_died), f64::from(n)),
            w.threat,
            w.actor,
            w.hurt,
            w.switched,
            w.pushes,
            w.push_expired,
            w.push_seen_again,
            w.push_other_target,
            w.glances,
            self.switches[0],
            self.switches_off_hurter[0],
            self.switches[1],
            self.switches_off_hurter[1],
            self.chances,
            self.chances_seen,
            self.switches_off_seen_hurter[0],
            self.off_seen_hurter_age[0][0],
            self.off_seen_hurter_age[0][1],
            self.off_seen_hurter_age[0][2],
            self.switches_off_seen_hurter[1],
            self.off_seen_hurter_age[1][0],
            self.off_seen_hurter_age[1][1],
            self.off_seen_hurter_age[1][2],
        );
        Part::new(json, text)
    }
}

#[test]
fn the_hits_of_a_pair_are_one_exchange_until_a_gap_and_the_first_hitter_is_named() {
    let bot = |_: Place| true;
    let hits = [
        (1_000, 3, 5),
        (1_500, 5, 3),
        (2_000, 3, 5),
        (20_000, 5, 3),
        (1_200, 1, 2),
    ];
    let mut hits = hits.to_vec();
    hits.sort_by_key(|h| h.0);
    let got = exchanges(&hits, bot);
    assert_eq!(
        got,
        vec![
            Exchange { first: 3, other: 5, start: 1_000, end: 2_000 },
            Exchange { first: 1, other: 2, start: 1_200, end: 1_200 },
            Exchange { first: 5, other: 3, start: 20_000, end: 20_000 },
        ]
    );
    assert!(exchanges(&hits, |p| p != 1).iter().all(|x| x.first != 1 && x.other != 1));
}

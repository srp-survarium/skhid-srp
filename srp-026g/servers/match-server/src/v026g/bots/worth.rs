//! What a seen enemy is worth shooting first, beyond how near he is (the fights analysis of the owner's
//! matches, 2026-10-28, R1 and R2), and what the team saw land on him.
//!
//! - **Worth** ([`Worth::of`]): metres of nearness an enemy counts for in `Reflex::choose`. An objective actor
//!   (in the battery modes a carrier: his hands hold the battery and cannot shoot back, `process_quick_slots`
//!   0x2D8DA0) and an enemy acting against the team's objective (a thief of its battery, on its HUD) are asked
//!   of [`super::objective_actor`], never of a mode; a hurt enemy is one hit from dying. The Cologne steal: a
//!   carrier walked past ten bots for 22 s and the first shot at him came 11 s after the pickup.
//! - **Hurt, as seen** ([`Wounds`]): an estimate from the bullets the team saw land, not a read of an enemy's
//!   pools. A bot that fired at an enemy sees its own bullet hit him (a hit marker, a puff of blood) at once; its
//!   teammates know after a callout's delay (`told_ms`). Each hit takes the weapon's nominal damage
//!   (`bullet_damage` in the game data, a share of a body's pool, before armour and the part hit); a fall, a zone,
//!   bleeding or a teammate human's bullet are never counted. The estimate mends as the game's pools do, after
//!   their 8 s timeout (`body_part::regenerate` 0x32A4B0, PROVEN; `standin_damage.rs`).

use super::objective_actor;
use super::view::{MatchView, Place};
use serde::{Deserialize, Serialize};

use super::know::TeamKnowledge;

/// The game's regeneration: a pool mends this long after its last hit, ms (the pools'
/// `regeneration_timeout`, 8 s for all).
const REGEN_TIMEOUT_MS: u32 = 8_000;
/// Body mends 0.03 a second, up to half of the pool.
const BODY_REGEN_PER_S: f32 = 0.03;
const BODY_REGEN_CAP: f32 = 0.5;
/// The hits of a wound kept, at most.
const HITS_KEPT: usize = 32;

/// Where an enemy's "hurt" is read from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HurtFrom {
    /// The enemy's true body pool (the tip's rule: a fact no player sees, and blind to pain and limbs).
    Body,
    /// What the bot and its team saw land ([`Wounds`]).
    #[default]
    Seen,
}

/// The numbers of the target's worth and of the finishing push (`CombatTuning::worth`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorthTuning {
    /// Where "hurt" comes from. `CombatTuning::wounded_bonus_m` is the worth of a fully hurt enemy. `Body` is the
    /// tip's rule (the enemy's true body pool, which no player sees); `Seen` is the estimate from the bullets the team
    /// saw land ([`Wounds`]): the switch of the tip's own read as well as of mine.
    pub hurt_from: HurtFrom,
    /// An enemy who is an objective actor now (a carrier, in the battery modes: [`objective_actor::is_actor`]) counts
    /// this much nearer, metres; 0: no worth for it.
    pub objective_actor_m: f32,
    /// ... an enemy acting against the team's objective (a thief of its battery, while the theft notice holds:
    /// [`objective_actor::acts_against`]), in place of `objective_actor_m`.
    pub objective_threat_m: f32,
    /// Another seen enemy takes the target from the kept one when he is worth this much more, metres of
    /// nearness (an objective actor 25 over a plain enemy takes it, a hurt one over a whole one does not until he is
    /// fully hurt: `wounded_bonus_m` is 20); 0: worth never takes a target.
    pub switch_m: f32,
    /// A bullet hit counts as the team's when one of its bots fired at the enemy this long before it, ms (the world
    /// that shows the hit reaches the bots up to 1.5 s late).
    pub credit_ms: u32,
    /// A teammate bot's hit is known to the others this long after it, ms (a callout; `avoid.told_ms` is 200-400).
    pub told_ms: u32,
    /// The enemy who hit the bot last stays its target this long, ms, whatever the worth of the others (the hurter
    /// is not given up for a carrier mid-duel); 0: the tip's one think.
    pub hurter_keep_ms: u32,
    /// The push on a hurt enemy who broke sight: `push_m` is how far his last place may be, metres; 0 is off.
    pub push_m: f32,
    /// ... he must be this hurt (`[0, 1]`), and the push goes on this long at most from the hit, ms (the pain
    /// refills from 8 s after it).
    pub push_hurt: f32,
    pub push_ms: u32,
    /// ... at most this many bots of a team push on one enemy.
    pub push_bots: u8,
    /// A teammate's callout of an enemy turns a bot that holds a target to look toward him for a moment (a
    /// glance to the flank), when he is within this, metres and outside the view; 0 is off, the default: no source found
    /// for it and no effect of its own in the bench (40 m was benched).
    pub flank_glance_m: f32,
    /// ... and only when the target is farther than this, metres (a close duel is no time to look away).
    pub flank_glance_min_m: f32,
    /// ... and only in a pause of the trigger (between bursts, before the next press) of at least this long, ms: the look
    /// lasts the pause, at most `flank_glance_ms`. A glance in a burst costs shots in a duel (the head-to-head of the first
    /// version lost 11 % of its K/D to 2779 glances); 0: at any time.
    pub flank_glance_min_pause_ms: u32,
    /// ... at most once in this long, ms, and for this long.
    pub flank_glance_again_ms: u32,
    pub flank_glance_ms: u32,
}

impl Default for WorthTuning {
    fn default() -> Self {
        Self {
            hurt_from: HurtFrom::Seen,
            objective_actor_m: 0.0,
            objective_threat_m: 0.0,
            switch_m: 0.0,
            credit_ms: 2_000,
            told_ms: 400,
            hurter_keep_ms: 0,
            push_m: 0.0,
            push_hurt: 0.5,
            push_ms: 6_000,
            push_bots: 2,
            flank_glance_m: 0.0,
            flank_glance_min_m: 12.0,
            flank_glance_min_pause_ms: 250,
            flank_glance_again_ms: 2_500,
            flank_glance_ms: 500,
        }
    }
}

impl WorthTuning {
    /// The numbers the worth, the keep of the enemy who hit the bot and the push were benched with (OFF by default: no gain
    /// on the tip `1ed94752`, 192 pairs score -2.0 %, z -1.4, carriers killed level).
    pub fn parts_on() -> Self {
        Self {
            objective_actor_m: 25.0,
            objective_threat_m: 45.0,
            switch_m: 20.0,
            hurter_keep_ms: 3_000,
            push_m: 30.0,
            ..Self::default()
        }
    }

    /// Whether the numbers make sense: metres finite and not negative, the hurt a share, a credit window.
    pub fn valid(&self) -> bool {
        let metres = [self.objective_actor_m, self.objective_threat_m, self.switch_m, self.push_m, self.flank_glance_m];
        metres.iter().all(|m| m.is_finite() && *m >= 0.0)
            && (0.0..=1.0).contains(&self.push_hurt)
            && self.credit_ms > 0
    }
}

/// What a pool has mended `since_ms` after its last hit, by the game's rule: nothing for the timeout, then
/// `per_s` a second down to `floor`.
fn mended(missing: f32, since_ms: u32, per_s: f32, floor: f32) -> f32 {
    let mending_s = since_ms.saturating_sub(REGEN_TIMEOUT_MS) as f32 / 1000.0;
    (missing - per_s * mending_s).max(floor.min(missing))
}

/// One bullet hit on an enemy that a bot of the team saw land: whose bullet, the share of a body's pool the weapon's
/// nominal damage is, and the world's time of the hit.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Hit {
    by: Place,
    share: f32,
    at: u32,
}

/// The hits the team saw land on one enemy in one life.
#[derive(Clone, Debug, PartialEq)]
struct Wound {
    who: Place,
    /// His life (`PlaceView::spawned`): a new life is a whole body.
    life: u32,
    hits: Vec<Hit>,
}

impl Wound {
    /// The hits `asker` knows at `now`: its own at once, a teammate's `told_ms` after it.
    fn known<'a>(&'a self, now: u32, asker: Place, told_ms: u32) -> impl Iterator<Item = &'a Hit> + 'a {
        self.hits
            .iter()
            .filter(move |h| h.by == asker || now >= h.at.saturating_add(told_ms))
    }

    /// How hurt he is at `now` as `asker` knows it, `[0, 1]`: the shares of the hits it knows, each mended as the game
    /// mends a body pool since the hit before it.
    fn hurt(&self, now: u32, asker: Place, told_ms: u32) -> f32 {
        let mut missing = 0.0_f32;
        let mut last: Option<u32> = None;
        for h in self.known(now, asker, told_ms) {
            if let Some(last) = last {
                missing = mended(missing, h.at.saturating_sub(last), BODY_REGEN_PER_S, 1.0 - BODY_REGEN_CAP);
            }
            missing = (missing + h.share).min(1.0);
            last = Some(h.at);
        }
        last.map_or(0.0, |last| {
            mended(missing, now.saturating_sub(last), BODY_REGEN_PER_S, 1.0 - BODY_REGEN_CAP).clamp(0.0, 1.0)
        })
    }
}

/// A bot of the team going after a hurt enemy who broke sight.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pusher {
    victim: Place,
    by: Place,
    until: u32,
}

/// What a team saw land on its enemies, and who is going after them.
#[derive(Clone, Debug, Default)]
pub struct Wounds {
    wounds: Vec<Wound>,
    pushers: Vec<Pusher>,
}

impl Wounds {
    /// A bullet hit by `by` on `who` (in his life `life`) at the world's time `at`, taking `share` of a body's pool at
    /// the weapon's nominal damage.
    pub fn landed(&mut self, who: Place, life: u32, share: f32, by: Place, at: u32) {
        let hit = Hit { by, share, at };
        match self.wounds.iter_mut().find(|w| w.who == who) {
            Some(w) if w.life == life => {
                w.hits.push(hit);
                w.hits.sort_by_key(|h| h.at);
                if w.hits.len() > HITS_KEPT {
                    w.hits.remove(0);
                }
            }
            Some(w) => *w = Wound { who, life, hits: vec![hit] },
            None => self.wounds.push(Wound { who, life, hits: vec![hit] }),
        }
    }

    /// How hurt `asker` knows `who` got in his life `life`, `[0, 1]`; 0 if it knows of no hit.
    pub fn hurt(&self, who: Place, life: u32, now: u32, asker: Place, told_ms: u32) -> f32 {
        self.wounds
            .iter()
            .find(|w| w.who == who && w.life == life)
            .map_or(0.0, |w| w.hurt(now, asker, told_ms))
    }

    /// When the last hit on `who` that `asker` knows landed, if it knows one in his life `life`.
    pub fn last_hit(&self, who: Place, life: u32, now: u32, asker: Place, told_ms: u32) -> Option<u32> {
        self.wounds
            .iter()
            .find(|w| w.who == who && w.life == life)
            .and_then(|w| w.known(now, asker, told_ms).map(|h| h.at).max())
    }

    /// A bot of the team goes after `victim` until `until`.
    pub fn push_claim(&mut self, victim: Place, by: Place, until: u32) {
        self.pushers.retain(|p| p.by != by);
        self.pushers.push(Pusher { victim, by, until });
    }

    /// Whether `by` may go after `victim` at `now`: fewer than `max` other bots of the team do.
    pub fn may_push(&self, victim: Place, by: Place, now: u32, max: u8) -> bool {
        self.pushers
            .iter()
            .filter(|p| p.victim == victim && p.by != by && now < p.until)
            .count()
            < usize::from(max)
    }

    /// `by` goes after nobody now (its push ended, or it never began): its claim goes.
    pub fn push_release(&mut self, by: Place) {
        self.pushers.retain(|p| p.by != by);
    }

    /// The enemy at `who` died or left: nothing of him stays.
    pub fn forget(&mut self, who: Place) {
        self.wounds.retain(|w| w.who != who);
        self.pushers.retain(|p| p.victim != who);
    }

    /// Old claims and hits go.
    pub fn forget_old(&mut self, now: u32) {
        self.pushers.retain(|p| now < p.until);
        self.wounds.retain_mut(|w| {
            w.hits.retain(|h| now < h.at.saturating_add(60_000));
            !w.hits.is_empty()
        });
    }
}

/// What a think read from the team about one enemy, for the choices between thinks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Known {
    pub who: Place,
    /// He acts against the team's objective now ([`objective_actor::acts_against`]).
    pub threat: bool,
    /// How hurt the asking bot knows him ([`Wounds::hurt`]), `[0, 1]`.
    pub hurt: f32,
}

impl Known {
    /// Every enemy of `mine` as the team's knowledge has him at `now`, for the bot `asker`.
    pub fn of_enemies(
        view: &MatchView,
        team: Option<&TeamKnowledge>,
        (mine, asker): (u8, Place),
        now: u32,
        told_ms: u32,
    ) -> Vec<Self> {
        view.enemies_of(mine)
            .filter(|enemy| enemy.alive)
            .map(|enemy| Self {
                who: enemy.place,
                threat: objective_actor::acts_against(enemy, team, now),
                hurt: team.map_or(0.0, |team| {
                    team.wounds.hurt(enemy.place, enemy.spawned, now, asker, told_ms)
                }),
            })
            .collect()
    }
}

/// What the choice of a target reads: the view as of the choice, what the last think knew of the team, and
/// the numbers.
#[derive(Clone, Copy)]
pub struct Worth<'a> {
    pub view: &'a MatchView,
    pub known: &'a [Known],
    pub tuning: &'a WorthTuning,
    /// Worth of a fully hurt enemy, metres (`CombatTuning::wounded_bonus_m`).
    pub hurt_m: f32,
}

/// What made the worth of a chosen enemy, the first of these that applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    Threat,
    Actor,
    Hurt,
}

impl Why {
    pub fn name(self) -> &'static str {
        match self {
            Self::Threat => "objective threat (thief of the team's battery)",
            Self::Actor => "objective actor (carrier)",
            Self::Hurt => "hurt as the team saw it",
        }
    }
}

/// [measure] What the worth, the push and the flank glance did in one life of a reflex (the bench's owner
/// counts add the lives up; nothing decides by them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorthStats {
    /// A target taken (fresh or a switch) that the nearest enemy would not have been, by why.
    pub threat: u32,
    pub actor: u32,
    pub hurt: u32,
    /// ... of them, taken from another target held.
    pub switched: u32,
    /// Pushes on a hurt enemy begun, and how each ended: its time ran out, the enemy was the target again,
    /// another enemy became the target.
    pub pushes: u32,
    pub push_expired: u32,
    pub push_seen_again: u32,
    pub push_other_target: u32,
    /// Flank glances at a callout.
    pub glances: u32,
}

impl WorthStats {
    pub fn total(&self) -> u32 {
        self.threat + self.actor + self.hurt + self.pushes + self.glances
    }

    pub fn count(&mut self, why: Why, switched: bool) {
        match why {
            Why::Threat => self.threat += 1,
            Why::Actor => self.actor += 1,
            Why::Hurt => self.hurt += 1,
        }
        self.switched += u32::from(switched);
    }

    pub fn add(&mut self, other: &Self) {
        self.threat += other.threat;
        self.actor += other.actor;
        self.hurt += other.hurt;
        self.switched += other.switched;
        self.pushes += other.pushes;
        self.push_expired += other.push_expired;
        self.push_seen_again += other.push_seen_again;
        self.push_other_target += other.push_other_target;
        self.glances += other.glances;
    }
}

impl Worth<'_> {
    /// Why `who` is worth more than his distance: the first of a thief, a carrier and a hurt enemy (as the team saw
    /// him get hurt) that applies and has a number.
    pub fn why(&self, who: Place) -> Option<Why> {
        let place = self.view.place(who)?;
        let known = self.known.iter().find(|k| k.who == who);
        if objective_actor::is_actor(place) {
            if known.is_some_and(|k| k.threat) && self.tuning.objective_threat_m > 0.0 {
                return Some(Why::Threat);
            }
            if self.tuning.objective_actor_m > 0.0 {
                return Some(Why::Actor);
            }
        }
        // The tip's own wounded-enemy rule (the body pool) is no reason of this module's: no count, no note, so the switches
        // off leave the tip's logs as they were.
        (self.tuning.hurt_from == HurtFrom::Seen && self.hurt_m > 0.0 && self.of(who) - self.acting(who) > 0.0)
            .then_some(Why::Hurt)
    }

    /// The part of the worth that being an objective actor gives.
    fn acting(&self, who: Place) -> f32 {
        let known = self.known.iter().find(|k| k.who == who);
        match self.view.place(who) {
            Some(place) if objective_actor::is_actor(place) => {
                if known.is_some_and(|k| k.threat) {
                    self.tuning.objective_threat_m
                } else {
                    self.tuning.objective_actor_m
                }
            }
            _ => 0.0,
        }
    }

    /// Metres of nearness `who` counts for: his hurt, and an objective actor's (an enemy acting against the team's own
    /// objective over a plain actor's).
    pub fn of(&self, who: Place) -> f32 {
        let Some(place) = self.view.place(who) else {
            return 0.0;
        };
        let known = self.known.iter().find(|k| k.who == who);
        let hurt = match self.tuning.hurt_from {
            HurtFrom::Body => 1.0 - place.health.clamp(0.0, 1.0),
            HurtFrom::Seen => known.map_or(0.0, |k| k.hurt),
        };
        self.acting(who) + self.hurt_m * hurt
    }
}

/// No decision on an enemy the team has not seen or heard within this, ms (the fairness bound, [`super::fairness::UNSEEN_MS`]).
pub const SENSED_WITHIN_MS: u32 = super::fairness::UNSEEN_MS;

#[cfg(test)]
mod tests {
    use super::*;

    const ME: Place = 0;
    const TOLD: u32 = 400;

    fn hurt(w: &Wounds, now: u32) -> f32 {
        w.hurt(3, 100, now, ME, TOLD)
    }

    #[test]
    fn a_hit_of_ones_own_is_known_at_once_a_mates_after_the_callout() {
        let mut w = Wounds::default();
        w.landed(3, 100, 0.4, ME, 10_000);
        w.landed(3, 100, 0.3, 5, 10_100);
        assert!((hurt(&w, 10_100) - 0.4).abs() < 1e-5, "the mate's hit is not known yet");
        assert!((hurt(&w, 10_500) - 0.7).abs() < 1e-5, "told 400 ms after");
        assert!((w.hurt(3, 100, 10_100, 6, TOLD)).abs() < 1e-5, "a third bot knows only its mate's, after the delay");
        assert!((w.hurt(3, 100, 10_400, 6, TOLD) - 0.4).abs() < 1e-5);
        assert_eq!(w.last_hit(3, 100, 10_100, ME, TOLD), Some(10_000));
        assert_eq!(w.last_hit(3, 100, 10_500, ME, TOLD), Some(10_100));
    }

    #[test]
    fn hurt_stays_for_the_timeout_then_mends_as_a_body_pool_does() {
        let mut w = Wounds::default();
        w.landed(3, 100, 0.9, ME, 10_000);
        assert!((hurt(&w, 10_000) - 0.9).abs() < 1e-5);
        assert!((hurt(&w, 18_000) - 0.9).abs() < 1e-5, "the 8 s timeout");
        assert!((hurt(&w, 20_000) - 0.84).abs() < 1e-4, "0.03 a second after it");
        assert!((hurt(&w, 100_000) - 0.5).abs() < 1e-4, "a body pool mends only to half");
        assert_eq!(w.hurt(3, 101, 10_000, ME, TOLD), 0.0, "another life is a whole body");
        assert_eq!(w.hurt(4, 100, 10_000, ME, TOLD), 0.0, "nothing seen of another");
    }

    #[test]
    fn hits_add_up_on_what_is_left_of_the_last() {
        let mut w = Wounds::default();
        w.landed(3, 100, 0.4, ME, 0);
        w.landed(3, 100, 0.4, ME, 1_000);
        assert!((hurt(&w, 1_000) - 0.8).abs() < 1e-5);
        w.landed(3, 100, 0.5, ME, 2_000);
        assert!((hurt(&w, 2_000) - 1.0).abs() < 1e-5, "capped at the whole pool");
    }

    #[test]
    fn the_push_is_capped_per_enemy_and_released_when_it_ends() {
        let mut w = Wounds::default();
        assert!(w.may_push(7, 1, 0, 2));
        w.push_claim(7, 1, 6_000);
        w.push_claim(7, 2, 6_000);
        assert!(!w.may_push(7, 3, 1_000, 2));
        assert!(w.may_push(7, 1, 1_000, 2), "a claim is its own bot's");
        assert!(w.may_push(8, 3, 1_000, 2), "another enemy");
        assert!(w.may_push(7, 3, 6_000, 2), "claims run out");
        w.push_release(1);
        assert!(w.may_push(7, 3, 1_000, 2), "a push that ended frees its place at once");
    }

    #[test]
    fn the_default_numbers_are_valid() {
        assert!(WorthTuning::default().valid());
    }
}

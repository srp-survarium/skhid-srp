//! Adjusting the bots while a match runs (`player_bots_research.md` B2):
//! three tiers.
//! 1. [`MatchSignals`], per team each commander tick: the score gap, the
//!    time left, who is alive on each side, thefts, the bots' K/D against
//!    humans.
//! 2. [`Mods`], per team: `aggression`, `risk`, `objective_urgency`, each a
//!    curve of signals (tuning), clamped and smoothed so one goal does not
//!    flip the team in a tick. Considerations read them
//!    (`curve::Input::Aggression`, ...).
//! 3. [`Skill`]: self-balancing (play 23). Every `every_s` the bots' K/D
//!    against humans over `window_s`; outside the dead band, one damped
//!    step (Unreal Tournament's `AdjustSkill`). A bot takes the new skill
//!    at its next respawn; it changes only the aim and the noticing delay,
//!    never what bots know.

use std::collections::VecDeque;

use super::curve::Input;
use super::tuning::{BotTier, ModTuning, SkillTuning};
use super::view::{MatchView, Mode};

/// What the commander of `team` reads of the match.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MatchSignals {
    pub team: u8,
    pub mode: Option<Mode>,
    pub score_gap: f32,
    pub time_left_s: f32,
    /// `[0, 1]` of the play time gone.
    pub progress: f32,
    pub alive_ours: usize,
    pub alive_theirs: usize,
    /// The share of the enemy team dead (waiting to respawn).
    pub enemies_dead: f32,
    /// Seconds since a battery of ours was taken (large if none).
    pub since_theft_s: f32,
    pub humans: usize,
}

impl MatchSignals {
    pub fn read(view: &MatchView, team: u8, since_theft_s: f32) -> Self {
        let count = |ours: bool, alive: bool| {
            view.places
                .iter()
                .filter(|p| p.present && (p.team == team) == ours && (!alive || p.alive))
                .count()
        };
        let theirs_all = count(false, false).max(1);
        Self {
            team,
            mode: Some(view.mode),
            score_gap: view.score_gap(team),
            time_left_s: view.time_left_s,
            progress: if view.total_s > 0.0 {
                (1.0 - view.time_left_s / view.total_s).clamp(0.0, 1.0)
            } else {
                0.0
            },
            alive_ours: count(true, true),
            alive_theirs: count(false, true),
            enemies_dead: 1.0 - count(false, true) as f32 / theirs_all as f32,
            since_theft_s,
            humans: view.places.iter().filter(|p| p.present && !p.bot).count(),
        }
    }

    /// A signal as a consideration's input.
    pub fn input(&self, input: Input) -> Option<f32> {
        Some(match input {
            Input::TimeLeft => self.time_left_s,
            Input::ScoreGap => self.score_gap,
            Input::EnemiesDead => self.enemies_dead,
            Input::SinceTheft => self.since_theft_s,
            Input::BehindLate => self.behind_late(),
            Input::TimeLeftBehind => self.time_left_behind(),
            _ => return None,
        })
    }

    /// Behind on points, `[0, 1]`: 1 from [`BEHIND_FULL`] points down.
    pub fn behind(&self) -> f32 {
        (-self.score_gap / BEHIND_FULL).clamp(0.0, 1.0)
    }

    /// Behind on points, late in the match, `[0, 1]`: [`Self::behind`]
    /// times the play time gone, full from [`LATE_FULL`] of it. A team
    /// far behind with little time left gains nothing by holding a spot
    /// (in the owner's match of 2026-10-16 the bots 1-3 to 1-5 behind
    /// spent 26 % of their time in hold plays).
    pub fn behind_late(&self) -> f32 {
        self.behind() * (self.progress / LATE_FULL).clamp(0.0, 1.0)
    }

    /// Seconds left, fewer the farther behind: down to half at
    /// [`BEHIND_FULL`] points behind, so a team far behind starts the
    /// endgame (play 16, late denial) earlier.
    pub fn time_left_behind(&self) -> f32 {
        self.time_left_s * (1.0 - 0.5 * self.behind())
    }
}

/// [`MatchSignals::behind`] is full this many points behind.
pub const BEHIND_FULL: f32 = 3.0;
/// [`MatchSignals::behind_late`] is full from this share of the play time
/// gone.
pub const LATE_FULL: f32 = 0.75;

/// A team's modifiers, `[min, max]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Mods {
    pub aggression: f32,
    pub risk: f32,
    pub objective_urgency: f32,
}

impl Default for Mods {
    fn default() -> Self {
        Self {
            aggression: 0.5,
            risk: 0.5,
            objective_urgency: 0.3,
        }
    }
}

impl Mods {
    /// One commander tick: each modifier moves part of the way to its
    /// target.
    pub fn update(&mut self, signals: &MatchSignals, tuning: &ModTuning) {
        let value = |input: Input| signals.input(input).unwrap_or(0.0);
        let aggression = tuning
            .aggression_gap
            .eval(value(tuning.aggression_gap.input))
            .get()
            * (0.5
                + 0.5
                    * tuning
                        .aggression_time
                        .eval(value(tuning.aggression_time.input))
                        .get());
        let risk = tuning.risk_alive.eval(value(tuning.risk_alive.input)).get();
        let urgency = tuning
            .urgency_time
            .eval(value(tuning.urgency_time.input))
            .get();
        let step = |m: &mut f32, target: f32| {
            *m += (target.clamp(tuning.min, tuning.max) - *m) * tuning.smoothing;
        };
        step(&mut self.aggression, aggression);
        step(&mut self.risk, risk);
        step(&mut self.objective_urgency, urgency);
    }

    pub fn input(&self, input: Input) -> Option<f32> {
        Some(match input {
            Input::Aggression => self.aggression,
            Input::Risk => self.risk,
            Input::ObjectiveUrgency => self.objective_urgency,
            _ => return None,
        })
    }
}

/// A kill between a bot and a human.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Duel {
    pub time: u32,
    /// The bot killed the human (else the human killed the bot).
    pub bot_won: bool,
}

/// A team's skill and its self-balancing (play 23).
#[derive(Clone, Debug)]
pub struct Skill {
    pub skill: f32,
    steps_down: u32,
    steps_up: u32,
    duels: VecDeque<Duel>,
    last_look: u32,
    /// The floor of its tier ([`SkillTuning::of_tier`]).
    floor: f32,
}

impl Skill {
    /// A team of bots of `tier`: its tier's start and floor.
    pub fn new(tuning: &SkillTuning, tier: BotTier) -> Self {
        let row = tuning.of_tier(tier);
        Self {
            skill: row.start,
            floor: row.min,
            steps_down: 0,
            steps_up: 0,
            duels: VecDeque::new(),
            last_look: 0,
        }
    }

    pub fn duel(&mut self, duel: Duel) {
        self.duels.push_back(duel);
    }

    /// The bots' K/D against humans over the window, and how many duels.
    pub fn kd(&self, now: u32, window_ms: u32) -> (f32, u32) {
        let recent = self.duels.iter().filter(|d| d.time + window_ms >= now);
        let (won, lost) = recent.fold((0u32, 0u32), |(w, l), d| {
            if d.bot_won { (w + 1, l) } else { (w, l + 1) }
        });
        (won as f32 / lost.max(1) as f32, won + lost)
    }

    /// Every `every_s`: a step outside the dead band. Returns the change
    /// made, if one was (for the log).
    pub fn balance(&mut self, now: u32, tuning: &SkillTuning) -> Option<(f32, f32, f32)> {
        if now < self.last_look + tuning.every_s * 1000 {
            return None;
        }
        self.last_look = now;
        let window = tuning.window_s * 1000;
        self.duels.retain(|d| d.time + window >= now);
        let (kd, events) = self.kd(now, window);
        if events < tuning.min_events {
            return None;
        }
        let before = self.skill;
        if kd > tuning.kd_high {
            self.steps_down += 1;
            self.skill -= tuning.step / self.steps_down.min(10) as f32;
        } else if kd < tuning.kd_low {
            self.steps_up += 1;
            self.skill += tuning.step / self.steps_up.min(10) as f32;
        } else {
            return None;
        }
        self.skill = self.skill.clamp(self.floor, tuning.max);
        Some((before, self.skill, kd))
    }
}

/// `a` to `b` by `t` in `[0, 1]`.
pub fn mix(range: (f32, f32), t: f32) -> f32 {
    range.0 + (range.1 - range.0) * t.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_team_behind_late_grows_aggressive_by_degrees() {
        let tuning = ModTuning::default();
        let mut mods = Mods::default();
        let behind = MatchSignals {
            score_gap: -2.0,
            time_left_s: 60.0,
            ..MatchSignals::default()
        };
        let before = mods.aggression;
        mods.update(&behind, &tuning);
        let once = mods.aggression;
        assert!(once > before && once < 0.95, "smoothed: {once}");
        for _ in 0..30 {
            mods.update(&behind, &tuning);
        }
        assert!(mods.aggression > 0.7, "{}", mods.aggression);
        let mut ahead = Mods::default();
        for _ in 0..30 {
            ahead.update(
                &MatchSignals {
                    score_gap: 3.0,
                    time_left_s: 800.0,
                    ..MatchSignals::default()
                },
                &tuning,
            );
        }
        assert!(ahead.aggression < 0.2, "{}", ahead.aggression);
    }

    #[test]
    fn skill_steps_outside_the_dead_band_in_damped_steps() {
        let tuning = SkillTuning::default();
        let mut skill = Skill::new(&tuning, BotTier::default());
        // Bots win 8 of 10: too strong, a step down.
        for k in 0..10 {
            skill.duel(Duel {
                time: 1_000 + k * 1_000,
                bot_won: k < 8,
            });
        }
        let (before, after, kd) = skill.balance(60_000, &tuning).unwrap();
        assert!(after < before && kd > tuning.kd_high);
        // Not again before `every_s` (30 s); in the band nothing changes.
        assert!(skill.balance(75_000, &tuning).is_none());
        let mut even = Skill::new(&tuning, BotTier::default());
        for k in 0..10 {
            even.duel(Duel {
                time: k * 1_000,
                bot_won: k % 2 == 0,
            });
        }
        assert!(even.balance(60_000, &tuning).is_none());
        // Steps shrink: the second is half the first.
        let first = before - after;
        for k in 0..10 {
            skill.duel(Duel {
                time: 61_000 + k * 1_000,
                bot_won: true,
            });
        }
        let (b2, a2, _) = skill.balance(120_000, &tuning).unwrap();
        assert!(((b2 - a2) - first / 2.0).abs() < 1e-4);
    }

    #[test]
    fn each_tier_starts_at_its_skill_and_never_falls_below_its_floor() {
        let tuning = SkillTuning::default();
        for (tier, start, floor) in [
            (BotTier::Tier4To5, 0.6, 0.3),
            (BotTier::Tier9To10, 0.75, 0.5),
            (BotTier::Tier13To15, 0.85, 0.65),
        ] {
            let mut skill = Skill::new(&tuning, tier);
            assert_eq!(skill.skill, start, "{tier:?}");
            // The bots win every duel for ten minutes: down to the floor,
            // never under it.
            for minute in 0..10u32 {
                for k in 0..10 {
                    skill.duel(Duel {
                        time: minute * 60_000 + k * 1_000,
                        bot_won: true,
                    });
                }
                skill.balance(minute * 60_000 + 30_000, &tuning);
            }
            assert!(
                skill.skill >= floor - 1e-6 && skill.skill < start,
                "{tier:?} {}",
                skill.skill
            );
            // They lose every duel for an hour: up, to 1 at most, every
            // tier.
            for minute in 10..70u32 {
                for k in 0..10 {
                    skill.duel(Duel {
                        time: minute * 60_000 + k * 1_000,
                        bot_won: false,
                    });
                }
                skill.balance(minute * 60_000 + 30_000, &tuning);
            }
            assert!(
                skill.skill <= 1.0 && skill.skill > start,
                "{tier:?} {}",
                skill.skill
            );
        }
        // A tier with no row: the old `start` and `min`.
        let old = SkillTuning {
            tiers: Default::default(),
            ..SkillTuning::default()
        };
        assert_eq!(Skill::new(&old, BotTier::Tier13To15).skill, old.start);
        // The JSON names a tier by its band.
        let json = r#"{"tiers":{"9-10":{"start":0.8,"min":0.4}}}"#;
        let file: SkillTuning = serde_json::from_str(json).unwrap();
        assert_eq!(file.of_tier(BotTier::Tier9To10).start, 0.8);
        assert_eq!(file.of_tier(BotTier::Tier4To5).start, file.start);
        assert_eq!(BotTier::of_level(10), BotTier::Tier9To10);
        assert_eq!(BotTier::of_level(14), BotTier::Tier13To15);
        assert_eq!(BotTier::of_level(5), BotTier::Tier4To5);
    }
}

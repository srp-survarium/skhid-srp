//! Every number the bots go by (`docs/notes/player_bots_research.md` B7):
//! typed structs, a default for each, and an optional JSON file
//! (`SRP_BOT_TUNING`) read when a match is made, so a new file is taken up
//! by the next match and never in the middle of one. Unknown fields are
//! refused and the values checked ([`Tuning::validate`]); each trace line
//! names the tuning's hash ([`Tuning::hash`]), so a match can be replayed
//! with what it ran on.
//!
//! The file names only what it changes: a section given replaces that
//! section's defaults field by field (`#[serde(default)]`); a play given
//! replaces that play's tuning whole.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::curve::Consideration;
use super::kin::Kinematics;
use super::motor::Feel;
use super::plays::{self, PlayKind, Rank};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tuning {
    pub kin: Kinematics,
    pub feel: Feel,
    pub sense: SenseTuning,
    pub cheats: CheatTuning,
    pub aim: AimTuning,
    pub skill: SkillTuning,
    pub mods: ModTuning,
    pub think: ThinkTuning,
    /// Metres kept from a damage zone (`hazards`).
    pub hazard_margin: f32,
    /// By play; a play missing here runs on its defaults
    /// (`plays::default_tuning`).
    pub plays: BTreeMap<PlayKind, PlayTuning>,
    /// The personalities and their share of the bots (play 14).
    pub personalities: Vec<Personality>,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            kin: Kinematics::default(),
            feel: Feel::default(),
            sense: SenseTuning::default(),
            cheats: CheatTuning::default(),
            aim: AimTuning::default(),
            skill: SkillTuning::default(),
            mods: ModTuning::default(),
            think: ThinkTuning::default(),
            hazard_margin: 1.0,
            plays: PlayKind::ALL
                .iter()
                .map(|&kind| (kind, plays::default_tuning(kind)))
                .collect(),
            personalities: Personality::defaults(),
        }
    }
}

/// What a bot sees and hears (`player_bots_behaviour.md` §2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SenseTuning {
    /// The view's full width, degrees.
    pub view_deg: f32,
    /// Farther than this nothing is seen, metres.
    pub sight_m: f32,
    /// The noticing delay, ms: near and far (it grows with distance up to
    /// `notice_far_m`), and the share of it left for an enemy who moves or
    /// fires.
    pub notice_near_ms: u32,
    pub notice_far_ms: u32,
    pub notice_far_m: f32,
    pub notice_moving: f32,
    /// Hearing ranges, metres.
    pub shot_m: f32,
    pub sprint_m: f32,
    pub reload_m: f32,
    pub use_m: f32,
    /// Footsteps: an enemy walking upright (faster than `step_speed`, m/s,
    /// not crouched) is heard within this, metres (0: never; the view has
    /// no walking sound, only a sprint's). Audit issue 4: in match
    /// 0x4da3b51d a bot stood 1-4 m from an enemy for 8 s and knew him only
    /// by an old sound. GUESS: half the sprint's range; the client's
    /// footstep sound ranges are not read. Bench, tiers 9-10: enemies
    /// within reach engaged within 1 s rose 8.6 ± 2.5 points a seed.
    pub step_m: f32,
    pub step_speed: f32,
    /// A heard position's error per metre of distance.
    pub heard_error: f32,
    /// What one bot senses its teammates know this much later, ms.
    pub callout_ms: u32,
    /// An enemy not sensed for this long is a fading last known position.
    pub forget_ms: u32,
    /// A last known position is dropped after this, ms.
    pub drop_ms: u32,
    /// With the battery cheat off (`CheatTuning::batteries`): a battery,
    /// or a battery spawner's spot, is seen within this, metres, in the
    /// view and in a clear line; within `battery_near_m` (on the ground)
    /// at once. GUESS: a lying battery is small but lit.
    pub battery_sight_m: f32,
    pub battery_near_m: f32,
    /// Rays a think may cast at batteries and spawners (nearest first).
    pub battery_rays: u32,
    /// Timed batteries: a station's spawner seen this long ago is worth a
    /// look again, ms.
    pub spawner_recheck_ms: u32,
}

impl Default for SenseTuning {
    fn default() -> Self {
        Self {
            view_deg: 90.0,
            sight_m: 150.0,
            notice_near_ms: 200,
            notice_far_ms: 500,
            notice_far_m: 60.0,
            notice_moving: 0.7,
            shot_m: 60.0,
            sprint_m: 15.0,
            reload_m: 10.0,
            use_m: 8.0,
            step_m: 8.0,
            step_speed: 1.5,
            heard_error: 0.15,
            callout_ms: 1_000,
            forget_ms: 3_000,
            drop_ms: 20_000,
            battery_sight_m: 35.0,
            battery_near_m: 3.0,
            battery_rays: 3,
            spawner_recheck_ms: 45_000,
        }
    }
}

/// The small cheats (`player_bots_behaviour.md` §2: "a bit, not too
/// much"). Never adaptive; never in the aim.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CheatTuning {
    /// Every battery's state and place, not only the HUD's. Off by
    /// default (the owner's word, 2026-10-16: the all-knowing bots ran
    /// straight to the batteries): the fair battery knowledge instead
    /// (`player_bots_behaviour.md` §2.1).
    pub batteries: bool,
    /// A thief's place with this error, metres, this often, ms (0: off).
    pub thief_error_m: f32,
    pub thief_every_ms: u32,
    /// The nearest enemy with this error, this often, while none is
    /// sensed (0: off).
    pub radar_error_m: f32,
    pub radar_every_ms: u32,
    /// Hearing ranges times this.
    pub hearing: f32,
    /// The forgetting time instead of the sense's.
    pub forget_ms: u32,
}

impl Default for CheatTuning {
    fn default() -> Self {
        Self {
            batteries: false,
            thief_error_m: 10.0,
            thief_every_ms: 3_000,
            radar_error_m: 15.0,
            radar_every_ms: 5_000,
            hearing: 1.25,
            forget_ms: 5_000,
        }
    }
}

/// The aim at skill 0 (weakest) and 1 (best); `skill` mixes them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AimTuning {
    /// The aim's error when it settles, degrees: a floor drawn every tick.
    pub error_deg: (f32, f32),
    /// The aim's slow wander about a tracked target: its spread (standard
    /// deviation, yaw; the pitch's is half), degrees, and how fast it
    /// comes back, seconds (a mean-reverting walk).
    pub wander_deg: (f32, f32),
    pub wander_s: f32,
    /// The time to bring the view onto a target, seconds (the motor's
    /// turn time while aiming).
    pub settle_s: (f32, f32),
    /// The noticing delay times this.
    pub notice: (f32, f32),
    /// Fire only within this of the target's direction, degrees.
    pub fire_within_deg: f32,
    /// Bursts of an automatic weapon whose class has no rows in
    /// `combat.fire` (`FireTuning::smg`, `rifle`): held, ms; and the pause
    /// after any burst, ms (the best players, 6+ matches of 2026-10-16..22:
    /// 150 / 300-350 / 600-800 ms, `player_bots_weapons.md` §3).
    pub burst_ms: (u32, u32),
    pub pause_ms: (u32, u32),
    /// Farther than this a bot does not shoot, metres; with a bolt action
    /// `combat.fire.bolt_range_m`.
    pub range_m: f32,
    /// From noticing an enemy to the first shot, ms (drawn ±15 % per
    /// target).
    pub reaction_ms: (f32, f32),
    /// The reflex's other numbers (plays 5, 7, 11, 19, 20, 21, reloads),
    /// the same at every skill.
    pub combat: super::reflex::CombatTuning,
    /// No shot through a teammate ([`super::friends`]).
    pub friends: super::friends::FriendlyFireTuning,
}

impl Default for AimTuning {
    fn default() -> Self {
        Self {
            // Skill 1 is the best quarter of the top players of
            // `TOP-PLAYERS.md` (the owner, 2026-10-23; it was their
            // median): a tighter floor and wander, a quicker settle,
            // reaction and noticing. The chain from a sighting to the first
            // shot at skill 1 is then ~400-450 ms (their q1 400 ms).
            error_deg: (6.0, 0.9),
            wander_deg: (3.0, 0.8),
            wander_s: 0.4,
            settle_s: (0.35, 0.10),
            notice: (1.5, 0.6),
            fire_within_deg: 4.0,
            burst_ms: (500, 1_000),
            pause_ms: (150, 700),
            range_m: 80.0,
            reaction_ms: (450.0, 120.0),
            combat: super::reflex::CombatTuning::default(),
            friends: super::friends::FriendlyFireTuning::default(),
        }
    }
}

/// Self-balancing (play 23): the bots' K/D against humans keeps them fun.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SkillTuning {
    /// The start and the floor of a tier with no row in `tiers`.
    pub start: f32,
    pub min: f32,
    /// The most, every tier (the owner's word, 2026-10-23: no tier has its
    /// own most).
    pub max: f32,
    /// By the bots' tier ([`BotTier`]): where a team's skill starts and
    /// the floor the self-balancing keeps it above (the owner, 2026-10-23:
    /// 4-5 from 0.6, floor 0.3; 9-10 from 0.75, floor 0.5; 13-15 from
    /// 0.85, floor 0.65).
    pub tiers: BTreeMap<BotTier, TierSkill>,
    /// One step's size before damping (`step / min(steps, 10)`).
    pub step: f32,
    /// The dead band of K/D against humans.
    pub kd_low: f32,
    pub kd_high: f32,
    /// How often it is looked at, and over how long, seconds.
    pub every_s: u32,
    pub window_s: u32,
    /// Fewer kills and deaths than this in the window say nothing.
    pub min_events: u32,
}

impl Default for SkillTuning {
    fn default() -> Self {
        Self {
            start: 0.7,
            min: 0.0,
            max: 1.0,
            tiers: [
                (BotTier::Low, (0.6, 0.3)),
                (BotTier::Mid, (0.75, 0.5)),
                (BotTier::High, (0.85, 0.65)),
            ]
            .into_iter()
            .map(|(tier, (start, min))| (tier, TierSkill { start, min }))
            .collect(),
            step: 0.15,
            kd_low: 0.67,
            kd_high: 1.5,
            every_s: 30,
            window_s: 300,
            min_events: 4,
        }
    }
}

impl SkillTuning {
    /// Where a team of bots of `tier` starts, and its floor: the tier's
    /// row, else `start` and `min`; the start within the floor and `max`.
    pub fn of_tier(&self, tier: BotTier) -> TierSkill {
        let row = self.tiers.get(&tier).copied().unwrap_or(TierSkill {
            start: self.start,
            min: self.min,
        });
        let min = row.min.min(self.max);
        TierSkill {
            start: row.start.clamp(min, self.max),
            min,
        }
    }
}

/// A tier's start and floor of the skill.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TierSkill {
    pub start: f32,
    pub min: f32,
}

/// The bots' tier (`survarium::v026g::loadouts::BotTier`, from the
/// lobby's profiles: `loadouts::bot_of`).
pub use survarium::v026g::loadouts::BotTier;

/// The team modifiers (`signals::Mods`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModTuning {
    /// Aggression from the score gap (behind is high) and the match's
    /// progress.
    pub aggression_gap: Consideration,
    pub aggression_time: Consideration,
    /// Risk from the share of the team alive against the enemy's.
    pub risk_alive: Consideration,
    /// Objective urgency from the time left.
    pub urgency_time: Consideration,
    /// How far a modifier moves toward its target each commander tick.
    pub smoothing: f32,
    pub min: f32,
    pub max: f32,
}

impl Default for ModTuning {
    fn default() -> Self {
        use super::curve::{Curve, Input};
        Self {
            aggression_gap: Consideration::new(
                Input::ScoreGap,
                -3.0,
                3.0,
                Curve::Logistic { k: 8.0, mid: 0.4 },
            )
            .inverted(),
            aggression_time: Consideration::new(
                Input::TimeLeft,
                0.0,
                900.0,
                Curve::Poly { k: 0.5 },
            )
            .inverted(),
            risk_alive: Consideration::new(Input::EnemiesDead, 0.0, 1.0, Curve::Linear),
            urgency_time: Consideration::new(Input::TimeLeft, 0.0, 600.0, Curve::Poly { k: 2.0 })
                .inverted(),
            smoothing: 0.2,
            min: 0.1,
            max: 1.0,
        }
    }
}

/// [`ThinkTuning::search_floors_per_poll`] by default: a floor expanded
/// costs about 0.2 µs on a fast desktop core and 0.5 µs on the host's, so
/// a poll searches for at most about 2 ms here and 5 ms there. Measured on
/// six maps, ten seeds each (`docs/notes/player_bots_mapdata.md` §6): the
/// bots play as with no bound, and the worst poll fell from up to 81 ms to
/// 3-10 ms.
pub const SEARCH_FLOORS_PER_POLL: u32 = 10_000;

/// How often what runs (research B5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThinkTuning {
    /// A bot's think (senses, play step), ms: drawn per think.
    pub think_ms: (u32, u32),
    /// The commander's tick, ms.
    pub commander_ms: u32,
    /// Sight rays a poll may cast, all bots together.
    pub rays_per_poll: u32,
    /// Floors a poll's route searches may expand, all bots together, as
    /// Detour's crowd shares its path queue's iterations: a search that
    /// runs past them goes on at the next polls (`Bots::poll`).
    pub search_floors_per_poll: u32,
    /// A trace line at least this often per bot, ms.
    pub trace_every_ms: u32,
}

impl Default for ThinkTuning {
    fn default() -> Self {
        Self {
            think_ms: (200, 400),
            commander_ms: 500,
            rays_per_poll: 400,
            search_floors_per_poll: SEARCH_FLOORS_PER_POLL,
            trace_every_ms: 5_000,
        }
    }
}

/// One play's tuning: its rank band, its considerations, and its numbers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayTuning {
    pub rank: Rank,
    pub considerations: Vec<Consideration>,
    /// The play's own numbers by name (`plays::<play>::NUMBERS` lists the
    /// names and defaults; another name is refused).
    #[serde(default)]
    pub numbers: BTreeMap<String, f32>,
}

/// A personality (play 14): weights per play family and motor habits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Personality {
    pub name: String,
    /// Its share of the bots.
    pub share: f32,
    /// Weights, about 1 (`curve::Input::Personality` reads them).
    pub hold: f32,
    pub push: f32,
    pub sneak: f32,
    pub support: f32,
    /// How readily it sprints, `[0, 1]`.
    pub sprint: f32,
}

impl Default for Personality {
    fn default() -> Self {
        Self {
            name: "steady".into(),
            share: 1.0,
            hold: 1.0,
            push: 1.0,
            sneak: 1.0,
            support: 1.0,
            sprint: 0.5,
        }
    }
}

impl Personality {
    /// The mix of how humans play (saved worlds, per life): about 55 %
    /// steady, 30 % mostly sprinting, 10-15 % holding still; so mostly
    /// flankers and support, a third rushers, few campers.
    pub fn defaults() -> Vec<Self> {
        let p = |name: &str, share, hold, push, sneak, support, sprint| Self {
            name: name.into(),
            share,
            hold,
            push,
            sneak,
            support,
            sprint,
        };
        vec![
            p("flanker", 0.3, 0.8, 1.0, 1.6, 0.9, 0.5),
            p("support", 0.25, 0.9, 0.8, 0.7, 1.6, 0.4),
            p("rusher", 0.3, 0.5, 1.6, 0.9, 0.8, 0.9),
            p("camper", 0.15, 1.8, 0.6, 0.8, 1.0, 0.2),
        ]
    }
}

impl Tuning {
    /// The defaults with the file's changes.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut tuning: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        // A `plays` given names only the plays it changes.
        let mut plays = Self::default().plays;
        plays.extend(std::mem::take(&mut tuning.plays));
        tuning.plays = plays;
        tuning.validate()?;
        Ok(tuning)
    }

    /// The values' sanity: ranges in order, shares summing to about 1, each
    /// play's numbers known to it.
    pub fn validate(&self) -> Result<(), String> {
        let check = |ok: bool, what: &str| if ok { Ok(()) } else { Err(what.to_owned()) };
        check(
            self.sense.view_deg > 0.0 && self.sense.view_deg <= 360.0,
            "sense.view_deg",
        )?;
        check(
            self.sense.notice_near_ms <= self.sense.notice_far_ms,
            "sense.notice_*_ms",
        )?;
        check(
            self.think.think_ms.0 > 0 && self.think.think_ms.0 <= self.think.think_ms.1,
            "think.think_ms",
        )?;
        check(self.think.commander_ms >= 50, "think.commander_ms")?;
        check(
            self.feel.hop_rise_m.0 <= self.feel.hop_rise_m.1 && self.feel.hop_every_s > 0.0,
            "feel.hop_*",
        )?;
        check(
            self.skill.min <= self.skill.start && self.skill.start <= self.skill.max,
            "skill.start",
        )?;
        for (tier, row) in &self.skill.tiers {
            check(
                (0.0..=row.start).contains(&row.min) && row.start <= self.skill.max,
                &format!("skill.tiers.{tier:?}"),
            )?;
        }
        check(self.skill.kd_low < self.skill.kd_high, "skill.kd_*")?;
        check(self.aim.burst_ms.0 <= self.aim.burst_ms.1, "aim.burst_ms")?;
        check(self.aim.pause_ms.0 <= self.aim.pause_ms.1, "aim.pause_ms")?;
        check(self.aim.wander_s > 0.0, "aim.wander_s")?;
        let c = &self.aim.combat;
        check(
            c.strafe_run_ms.0 > 0 && c.strafe_run_ms.0 <= c.strafe_run_ms.1,
            "aim.combat.strafe_run_ms",
        )?;
        check(
            c.strafe_still_ms.0 <= c.strafe_still_ms.1,
            "aim.combat.strafe_still_ms",
        )?;
        check(
            (0.0..=1.0).contains(&c.recoil_pull)
                && (0.0..=c.recoil_pull).contains(&c.recoil_pull_low),
            "aim.combat.recoil_pull",
        )?;
        check(self.hazard_margin >= 0.0, "hazard_margin")?;
        let shares: f32 = self.personalities.iter().map(|p| p.share).sum();
        check(
            !self.personalities.is_empty() && (shares - 1.0).abs() < 0.05,
            "personalities: shares must sum to 1",
        )?;
        for (kind, play) in &self.plays {
            for name in play.numbers.keys() {
                check(
                    plays::numbers(*kind).iter().any(|(known, _)| known == name),
                    &format!("plays.{kind:?}.numbers.{name}: unknown"),
                )?;
            }
        }
        Ok(())
    }

    /// The hash of the tuning as it runs (its JSON), for the traces.
    pub fn hash(&self) -> u32 {
        crc32fast::hash(serde_json::to_string(self).unwrap_or_default().as_bytes())
    }

    /// A play's number: the file's, else the play's default.
    pub fn number(&self, kind: PlayKind, name: &str) -> f32 {
        self.plays
            .get(&kind)
            .and_then(|play| play.numbers.get(name).copied())
            .or_else(|| {
                plays::numbers(kind)
                    .iter()
                    .find(|(known, _)| *known == name)
                    .map(|(_, value)| *value)
            })
            .unwrap_or_else(|| {
                debug_assert!(false, "{kind:?} has no number {name}");
                0.0
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_valid_and_a_file_changes_only_what_it_names() {
        let defaults = Tuning::default();
        defaults.validate().unwrap();
        let dir = std::env::temp_dir().join(format!("srp-tuning-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bots.json");
        std::fs::write(&path, r#"{"sense":{"shot_m":70},"hazard_margin":1.5}"#).unwrap();
        let tuning = Tuning::load(&path).unwrap();
        assert_eq!(tuning.sense.shot_m, 70.0);
        assert_eq!(tuning.sense.sprint_m, defaults.sense.sprint_m);
        assert_eq!(tuning.hazard_margin, 1.5);
        assert_eq!(tuning.plays, defaults.plays);
        assert_ne!(tuning.hash(), defaults.hash());
        // Unknown fields and bad values are refused.
        std::fs::write(&path, r#"{"sense":{"shot_range":70}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        std::fs::write(&path, r#"{"skill":{"kd_low":2,"kd_high":1}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        std::fs::write(&path, r#"{"aim":{"wander_s":0}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        std::fs::write(&path, r#"{"aim":{"combat":{"strafe_run_ms":[0,100]}}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        // A section given in part keeps the rest of its defaults.
        std::fs::write(&path, r#"{"aim":{"combat":{"ads_near_m":12}}}"#).unwrap();
        let tuning = Tuning::load(&path).unwrap();
        assert_eq!(tuning.aim.combat.ads_near_m, 12.0);
        assert_eq!(
            tuning.aim.combat.strafe_run_ms,
            defaults.aim.combat.strafe_run_ms
        );
        assert_eq!(tuning.aim.burst_ms, defaults.aim.burst_ms);
    }
}

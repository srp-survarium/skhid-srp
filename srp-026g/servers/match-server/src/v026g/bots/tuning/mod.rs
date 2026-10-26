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
    /// A carrier's put-in stand is one clear of a teammate's claim when
    /// one is (`Ctx::nearest_use_stand`; the bench's switch,
    /// `SRP_BENCH_STAND_CLAIM=0` turns it off).
    pub stand_claim: bool,
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
            stand_claim: true,
            plays: PlayKind::ALL
                .iter()
                .map(|&kind| (kind, plays::default_tuning(kind)))
                .collect(),
            personalities: Personality::defaults(),
        }
    }
}

/// The bots' tier (`survarium::v026g::loadouts::BotTier`, from the
/// lobby's profiles: `loadouts::bot_of`).
pub use survarium::v026g::loadouts::BotTier;

mod aim;
mod motion;
mod play;
mod sense;
mod team;

pub use aim::{AimTuning, SkillTuning, TierSkill};
pub use play::PlayTuning;
pub use sense::{CheatTuning, SenseTuning};
pub use team::{ModTuning, Personality, ThinkTuning, SEARCH_FLOORS_PER_POLL};

/// `Ok` if `ok`, else the name of what is wrong.
fn check(ok: bool, what: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(what.to_owned()) }
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
        sense::validate(&self.sense)?;
        team::validate_think(&self.think)?;
        motion::validate_feel(&self.feel)?;
        aim::validate_skill(&self.skill)?;
        aim::validate_aim(&self.aim)?;
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

    #[test]
    fn validate_names_the_first_wrong_value_in_the_order_of_the_sections() {
        let wrong = |change: fn(&mut Tuning)| {
            let mut tuning = Tuning::default();
            change(&mut tuning);
            tuning.validate().unwrap_err()
        };
        assert_eq!(wrong(|t| t.sense.view_deg = 0.0), "sense.view_deg");
        assert_eq!(wrong(|t| t.sense.notice_near_ms = t.sense.notice_far_ms + 1), "sense.notice_*_ms");
        assert_eq!(wrong(|t| t.think.think_ms = (0, 5)), "think.think_ms");
        assert_eq!(wrong(|t| t.think.commander_ms = 10), "think.commander_ms");
        assert_eq!(wrong(|t| t.feel.hop_every_s = 0.0), "feel.hop_*");
        assert_eq!(wrong(|t| t.skill.start = t.skill.max + 1.0), "skill.start");
        assert_eq!(wrong(|t| t.skill.kd_low = 9.0), "skill.kd_*");
        assert_eq!(wrong(|t| t.aim.wander_s = 0.0), "aim.wander_s");
        assert_eq!(wrong(|t| t.aim.combat.strafe_run_ms = (0, 5)), "aim.combat.strafe_run_ms");
        assert_eq!(wrong(|t| t.aim.combat.recoil_pull = 2.0), "aim.combat.recoil_pull");
        assert_eq!(wrong(|t| t.hazard_margin = -1.0), "hazard_margin");
        assert_eq!(wrong(|t| t.personalities.clear()), "personalities: shares must sum to 1");
        // Several wrong at once: the first section in the order says so.
        assert_eq!(
            wrong(|t| {
                t.hazard_margin = -1.0;
                t.aim.wander_s = 0.0;
                t.sense.view_deg = 0.0;
            }),
            "sense.view_deg"
        );
        assert_eq!(
            wrong(|t| {
                t.aim.combat.recoil_pull = 2.0;
                t.aim.burst_ms = (9, 1);
                t.skill.kd_low = 9.0;
            }),
            "skill.kd_*"
        );
    }
}

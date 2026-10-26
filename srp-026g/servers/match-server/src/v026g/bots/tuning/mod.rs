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
//!
//! A feature owns its section: the struct, its `Default` and its limits are in its
//! area file, and `Tuning` only names it. Limits are parsed where a type can hold them
//! (`limits`: `Metres`, `Positive`, `Span`), so a bad file fails when it is read; the
//! older sections keep a [`Check`] each, listed in [`Tuning::validate`] (sorted).
//! The keys of a field that moved into a section are in `legacy`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

use super::curve::Consideration;
use super::kin::Kinematics;
use super::motor::Feel;
use super::plays;
use super::plays::PlayKind;
use super::plays::Rank;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
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
    pub unstick: UnstickTuning,
    /// The bear-trap habit (`bots/traps.rs`).
    pub traps: TrapTuning,
    pub heal: HealTuning,
    pub threat: ThreatTuning,
    pub commit: CommitTuning,
    /// Stepping out of live grenades (`grenade_threat.rs`).
    pub avoid: AvoidTuning,
    pub grid: GridTuning,
    pub hazard: HazardTuning,
    pub personalities: Personalities,
    pub plays: PlaysTuning,
}

/// The bots' tier (`survarium::v026g::loadouts::BotTier`, from the
/// lobby's profiles: `loadouts::bot_of`).
pub use survarium::v026g::loadouts::BotTier;

mod aim;
mod grid;
mod hazard;
mod heal;
mod legacy;
mod limits;
mod motion;
mod play;
mod sense;
mod team;
mod traps;
mod unstick;

pub use aim::AimTuning;
pub use aim::SkillTuning;
pub use aim::TierSkill;
pub use grid::GridTuning;
pub use hazard::EscapeRule;
pub use hazard::HazardTuning;
pub use heal::HealTuning;
pub use heal::NeedWeights;
pub use limits::Metres;
pub use limits::Positive;
pub use limits::Span;
pub use play::PlayTuning;
pub use play::PlaysTuning;
pub use sense::CheatTuning;
pub use sense::HearingRanges;
pub use sense::SenseTuning;
pub use sense::ShotRanges;
pub use team::AvoidTuning;
pub use team::CommitTuning;
pub use team::ModTuning;
pub use team::Personalities;
pub use team::Personality;
pub use team::SEARCH_FLOORS_PER_POLL;
pub use team::ThinkTuning;
pub use team::ThreatTuning;
pub use traps::ReactTuning;
pub use traps::TrapTuning;
pub use unstick::SUICIDE_AFTER_OWNER_MS;
pub use unstick::UnstickTuning;

/// `Ok` if `ok`, else the name of what is wrong.
fn check(ok: bool, what: &str) -> Result<(), String> {
    if ok { Ok(()) } else { Err(what.to_owned()) }
}

/// A section's own limits, for what its types cannot hold: the first wrong value, by its name. A section
/// implements it in its area file and is listed once in [`Tuning::validate`].
pub trait Check {
    fn check(&self) -> Result<(), String>;
}

impl Tuning {
    /// The off-mesh links of the maps' grids ([`GridTuning::gap_links`]).
    pub fn off_mesh_links(&self) -> mapdata::OffMeshLinks {
        self.grid.off_mesh_links()
    }

    /// The body of the maps' grids ([`GridTuning::client_numbers`]).
    pub fn grid_body(&self) -> mapdata::Body {
        self.grid.body()
    }

    /// The defaults with the file's changes.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_value(value).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// A patch's or a file's JSON with the old key of a moved field at its new place (`legacy`).
    pub fn upgrade_keys(value: &mut serde_json::Value) -> Result<(), String> {
        legacy::upgrade(value)
    }

    /// The tuning a file's JSON says (its old keys read as their new ones), checked.
    pub fn from_value(mut value: serde_json::Value) -> Result<Self, String> {
        legacy::upgrade(&mut value)?;
        let mut tuning: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        // A `plays` given names only the plays it changes.
        let mut plays = PlaysTuning::default();
        plays.extend(std::mem::take(&mut tuning.plays.0));
        tuning.plays = plays;
        tuning.validate()?;
        Ok(tuning)
    }

    /// The values' sanity, for what the types do not hold: the sections' [`Check`]s, sorted by section.
    pub fn validate(&self) -> Result<(), String> {
        self.aim.check()?;
        self.avoid.check()?;
        self.feel.check()?;
        self.heal.check()?;
        self.personalities.check()?;
        self.plays.check()?;
        self.sense.check()?;
        self.skill.check()?;
        self.think.check()?;
        self.traps.check()?;
        self.unstick.check()?;
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
        std::fs::write(&path, r#"{"sense":{"shot_m":70},"hazard":{"margin_m":1.5}}"#).unwrap();
        let tuning = Tuning::load(&path).unwrap();
        assert_eq!(tuning.sense.shot_m, 70.0);
        assert_eq!(tuning.sense.sprint_m, defaults.sense.sprint_m);
        assert_eq!(tuning.hazard.margin_m.get(), 1.5);
        assert_eq!(tuning.plays, defaults.plays);
        assert_ne!(tuning.hash(), defaults.hash());
        // The old key of a moved field still reads.
        std::fs::write(&path, r#"{"hazard_margin":1.5}"#).unwrap();
        assert_eq!(Tuning::load(&path).unwrap().hazard.margin_m.get(), 1.5);
        // Unknown fields and bad values are refused.
        std::fs::write(&path, r#"{"sense":{"shot_range":70}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        std::fs::write(&path, r#"{"skill":{"kd_low":2,"kd_high":1}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        std::fs::write(&path, r#"{"aim":{"wander_s":0}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        std::fs::write(&path, r#"{"aim":{"combat":{"strafe_run_ms":[0,100]}}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        // A value a type cannot hold is refused when it is read, whichever key names it.
        for bad in [
            r#"{"hazard":{"margin_m":-1}}"#,
            r#"{"hazard_margin":-1}"#,
            r#"{"feel":{"hop_every_s":0}}"#,
            r#"{"feel":{"hop_rise_m":[1,0.5]}}"#,
            r#"{"hazard":{"escape":"sideways"}}"#,
        ] {
            std::fs::write(&path, bad).unwrap();
            assert!(Tuning::load(&path).is_err(), "{bad}");
        }
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
        assert_eq!(wrong(|t| t.sense.sprint_body_mps = Some(0.0)), "sense.sprint_body_mps");
        assert_eq!(
            wrong(|t| t.sense.shots = ShotRanges::ByWeapon { unknown_m: 0.0 }),
            "sense.shots.by_weapon.unknown_m"
        );
        assert_eq!(
            wrong(|t| t.sense.ranges = HearingRanges::Game { crouch_step_m: t.sense.step_m + 1.0 }),
            "sense.ranges.game.crouch_step_m"
        );
        assert_eq!(wrong(|t| t.think.think_ms = (0, 5)), "think.think_ms");
        assert_eq!(wrong(|t| t.think.commander_ms = 10), "think.commander_ms");
        assert_eq!(wrong(|t| t.feel.off_walk_below_m = 0.1), "feel.off_walk_below_m");
        assert_eq!(wrong(|t| t.skill.start = t.skill.max + 1.0), "skill.start");
        assert_eq!(wrong(|t| t.skill.kd_low = 9.0), "skill.kd_*");
        assert_eq!(wrong(|t| t.aim.wander_s = 0.0), "aim.wander_s");
        assert_eq!(wrong(|t| t.aim.combat.strafe_run_ms = (0, 5)), "aim.combat.strafe_run_ms");
        assert_eq!(wrong(|t| t.aim.combat.recoil_pull = 2.0), "aim.combat.recoil_pull");
        assert_eq!(wrong(|t| t.personalities.clear()), "personalities: shares must sum to 1");
        // Several wrong at once: the first section in the sorted list says so.
        assert_eq!(
            wrong(|t| {
                t.aim.wander_s = 0.0;
                t.sense.view_deg = 0.0;
                t.personalities.clear();
            }),
            "aim.wander_s"
        );
        assert_eq!(
            wrong(|t| {
                t.aim.combat.recoil_pull = 2.0;
                t.aim.burst_ms = (9, 1);
                t.skill.kd_low = 9.0;
            }),
            "aim.burst_ms"
        );
    }
}

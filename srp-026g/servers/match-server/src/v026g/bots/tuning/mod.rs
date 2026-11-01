//! Every number the bots go by (`docs/notes/player_bots_research.md` B7):
//! typed structs, a default for each, and an optional JSON file
//! (`SRP_BOT_TUNING`) read when the server starts ([`LoadedTuning`]): a
//! file the server cannot use stops it at the start, with the reason, and
//! never falls back to the defaults. Before each match the file is read again,
//! with the admin page's changes over it ([`LiveTuning`]): one refused then keeps
//! the last good tuning. Unknown fields are
//! refused and the values checked ([`Tuning::validate`]); each trace line
//! names the tuning's hash ([`Tuning::hash`]), so a match can be replayed
//! with what it ran on.
//!
//! The defaults are the bots as the owner wants them played: every option that shipped is on in
//! `Tuning::default()` (the switch report names what is not, [`SwitchReport`]). The file is a JSON
//! merge patch (RFC 7386) over the defaults ([`Tuning::patched`]): it names only what it changes,
//! objects merge key by key, a list or a number replaces, so a play given with its numbers only keeps
//! its rank and considerations. A whole tuning dumped from the defaults is a patch that changes
//! nothing. `null` sets an optional field to none (the RFC would take the key out, which gives the
//! default back: not an off switch now that the defaults are on) and is refused for any other field.
//!
//! A feature owns its section: the struct, its `Default` and its limits are in its
//! area file, and `Tuning` only names it. Limits are parsed where a type can hold them
//! (`limits`: `Metres`, `Positive`, `Span`), so a bad file fails when it is read; the
//! older sections keep a [`Check`] each, listed in [`Tuning::validate`] (sorted).
//! The keys of a field that moved into a section are in `legacy`.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

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
    pub perch: PerchTuning,
    /// Sprint on the calm stretches (`bots/pace.rs`).
    pub pace: PaceTuning,
    pub grid: GridTuning,
    /// What counts as cover from a bullet (`mapdata`'s materials).
    pub cover: CoverTuning,
    /// What gathers the team's bots (`plays/buddy.rs`, `plays/crossfire.rs`, the callouts).
    pub spacing: SpacingTuning,
    pub hazard: HazardTuning,
    /// What smoke hides from the bots (`bots/smoke.rs`).
    pub smoke: SmokeTuning,
    /// What the bots' kits are built with (`kit.rs`).
    pub kit: KitTuning,
    pub personalities: Personalities,
    pub plays: PlaysTuning,
}

/// The bots' tier (`survarium::v026g::loadouts::BotTier`, from the
/// lobby's profiles: `loadouts::bot_of`).
pub use survarium::v026g::loadouts::BotTier;

mod aim;
mod cover;
mod grid;
mod hazard;
mod heal;
mod kit;
mod legacy;
mod limits;
mod live;
mod motion;
mod pace;
mod play;
mod report;
mod sense;
mod smoke;
mod spacing;
mod team;
mod teams;
mod traps;
mod unstick;

pub use aim::AimTuning;
pub use aim::SkillTuning;
pub use aim::TierSkill;
pub use cover::BulletPierce;
pub use cover::CoverRule;
pub use cover::CoverTuning;
pub use cover::Exposure;
pub use cover::ExposureWeight;
pub use cover::PierceCover;
pub use cover::Share;
pub use cover::SpotCost;
pub use grid::GridTuning;
pub use hazard::EscapeRule;
pub use hazard::HazardTuning;
pub use hazard::RouteCrossing;
pub use hazard::ZonePools;
pub use heal::DrugStops;
pub use heal::HealTuning;
pub use heal::NeedWeights;
pub use kit::KitTuning;
pub use limits::Metres;
pub use limits::Positive;
pub use limits::Span;
pub use live::LiveTuning;
pub use pace::PaceTuning;
pub use pace::Reserve;
pub use play::PlayTuning;
pub use play::PlaysTuning;
pub use report::SwitchReport;
pub use sense::CheatTuning;
pub use sense::HearingRanges;
pub use sense::SenseTuning;
pub use sense::ShotRanges;
pub use smoke::CloudWaiters;
pub use smoke::SmokeTuning;
pub use smoke::WaitForCloud;
pub use spacing::Callouts;
pub use spacing::SpacingTuning;
pub use team::AvoidTuning;
pub use team::CommitTuning;
pub use team::Concede;
pub use team::GangRule;
pub use team::GangUp;
pub use team::LonePeek;
pub use team::ModTuning;
pub use team::PairTarget;
pub use team::PerchTuning;
pub use team::Personalities;
pub use team::Personality;
pub use team::PinFromCover;
pub use team::PinRule;
pub use team::SEARCH_FLOORS_PER_POLL;
pub use team::SearchQueue;
pub use team::SniperMemory;
pub use team::StaminaRates;
pub use team::ThinkTuning;
pub use team::ThreatTuning;
pub use team::WalkWatch;
pub use team::WatchKnown;
pub use teams::TeamTunings;
pub use traps::DefeatedTuning;
pub use traps::PassTuning;
pub use traps::ReactTuning;
pub use traps::TrapTuning;
pub use unstick::DropShut;
pub use unstick::SUICIDE_AFTER_OWNER_MS;
pub use unstick::UnstickTuning;

/// `patch` merged into `target` as RFC 7386 merges, but for its `null`: an object merges key by key, a `null` is
/// put in as the key's value (the field's none), anything else replaces what is there. RFC 7386 takes a `null`'s
/// key out, and an absent key reads as the field's default: under a section that is none in the defaults, the
/// patch's object starts empty, so `{"threat":{"concede":{"lone_peek":null}}}` took `lone_peek` out and read its
/// default `Some` back (board 2026-11-01T04:15:20Z).
pub(crate) fn merge_patch(target: &mut serde_json::Value, patch: serde_json::Value) {
    let serde_json::Value::Object(patch) = patch else {
        *target = patch;
        return;
    };
    if !target.is_object() {
        *target = serde_json::Value::Object(serde_json::Map::new());
    }
    let target = target.as_object_mut().expect("made an object above");
    for (key, value) in patch {
        merge_patch(target.entry(key).or_insert(serde_json::Value::Null), value);
    }
}

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

    /// The tuning with every option that the defaults have on put back off (`all_off.json`): the bots as they were
    /// before the owner's set shipped, except where the old value is wrong by the client or the game (the pitch unit, the
    /// use ray, the zones' pools: A223) or a cheat, information no player has (sight through smoke, the true health: A234),
    /// which keeps the default. The switch report is against it
    /// ([`Tuning::switch_report`]), the tests of what an option changes start from it, and its JSON is the bench's OFF arm.
    pub fn all_off() -> Self {
        Self::parse(include_str!("all_off.json")).expect("all_off.json is a valid patch")
    }

    /// The defaults with the changes of the patch in the file (see [`Tuning::patched`]).
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The defaults with the changes of the patch in `text` (see [`Tuning::patched`]).
    pub fn parse(text: &str) -> Result<Self, String> {
        let patch = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
        Self::patched(patch)
    }

    /// The defaults with the changes of a JSON merge patch (RFC 7386, [`Tuning::apply_patch`]), checked.
    pub fn patched(patch: serde_json::Value) -> Result<Self, String> {
        let mut value = serde_json::to_value(Self::default()).map_err(|e| e.to_string())?;
        Self::apply_patch(&mut value, patch)?;
        Self::from_value(value)
    }

    /// `patch` merged into a tuning's JSON `value`: the old keys of a moved field read as their new ones, an object
    /// merges key by key, a list or a scalar replaces what is there, and a `null` sets the field to none, at any
    /// depth, under a section that is none in `value` too ([`merge_patch`]). The defaults are on, so a patch that
    /// switches an optional field off says `null`. A field that cannot be none, or a name the tuning does not
    /// have, refuses the `null` when the whole is read.
    pub fn apply_patch(value: &mut serde_json::Value, mut patch: serde_json::Value) -> Result<(), String> {
        legacy::upgrade(&mut patch)?;
        merge_patch(value, patch);
        Ok(())
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
        self.grid.check()?;
        self.heal.check()?;
        self.personalities.check()?;
        self.plays.check()?;
        self.sense.check()?;
        self.skill.check()?;
        self.smoke.check()?;
        self.think.check()?;
        self.traps.check()?;
        self.unstick.check()?;
        sense::validate_sight(&self.sense, &self.cheats, &self.think)?;
        Ok(())
    }

    /// Whether the maps' grids need their table of wall clearances (`mapdata::WalkGrid::wall_clearance`) for this tuning: the one
    /// place that says it, so a map loader derives it from the tuning in force and nothing can be set to disagree. The table costs
    /// 0.5-1.7 s of CPU a map, which a tuning without `think.wall_clear_corners` does not spend.
    pub fn needs_wall_clearance(&self) -> bool {
        self.think.wall_clear_corners
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

/// Where the server's tuning comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Source {
    /// `SRP_BOT_TUNING` is not set: [`Tuning::default`].
    Defaults,
    /// The patch file `SRP_BOT_TUNING` names, with the digest of the bytes that were read.
    File { path: PathBuf, sha: String },
}

/// The tuning a bot match runs on, and where it comes from. The server's is parsed when it starts: a file that is
/// there but cannot be used (unreadable, not JSON, a name or a value refused) is an error at the start, never the
/// defaults. Each match takes it from [`LiveTuning`], which reads the file again and adds the admin page's changes.
#[derive(Clone, Debug)]
pub struct LoadedTuning {
    source: Source,
    /// The digest of the admin page's changes over the source ([`LiveTuning`]), when there are any.
    changes: Option<String>,
    tuning: Arc<Tuning>,
}

impl Default for LoadedTuning {
    /// The defaults, as a server with no file runs.
    fn default() -> Self {
        Self { source: Source::Defaults, changes: None, tuning: Arc::new(Tuning::default()) }
    }
}

impl PartialEq for LoadedTuning {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source && self.changes == other.changes && self.tuning.hash() == other.tuning.hash()
    }
}

impl Eq for LoadedTuning {}

impl LoadedTuning {
    /// The file `SRP_BOT_TUNING` names (unset or empty: the defaults).
    pub fn from_env() -> Result<Self, String> {
        match std::env::var_os("SRP_BOT_TUNING").filter(|path| !path.is_empty()) {
            Some(path) => Self::from_file(path.as_ref()),
            None => Ok(Self::default()),
        }
    }

    /// The defaults with the changes of the patch file at `path`; the error says which file and what is wrong with it.
    pub fn from_file(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("SRP_BOT_TUNING {}: {e}", path.display()))?;
        Self::from_bytes(path, &bytes)
    }

    /// The defaults with the changes of the patch `bytes`, read from the file at `path`.
    fn from_bytes(path: &Path, bytes: &[u8]) -> Result<Self, String> {
        let text = std::str::from_utf8(bytes).map_err(|e| format!("SRP_BOT_TUNING {}: not UTF-8: {e}", path.display()))?;
        let tuning = Tuning::parse(text).map_err(|e| format!("SRP_BOT_TUNING {}: {e}", path.display()))?;
        Ok(Self {
            source: Source::File { path: path.to_owned(), sha: super::switch_log::bytes_sha(bytes) },
            changes: None,
            tuning: Arc::new(tuning),
        })
    }

    pub fn tuning(&self) -> &Arc<Tuning> {
        &self.tuning
    }

    /// The file it was read from, and the digest of its bytes.
    fn file(&self) -> Option<(&Path, &str)> {
        match &self.source {
            Source::Defaults => None,
            Source::File { path, sha } => Some((path, sha)),
        }
    }

    /// The same source with the admin page's changes (their digest) made into `tuning`.
    fn changed(&self, changes: String, tuning: Tuning) -> Self {
        Self { source: self.source.clone(), changes: Some(changes), tuning: Arc::new(tuning) }
    }

    /// The tuning part of a match's data line: the file (with its digest) or `tuning=defaults`, and the digest of the
    /// admin page's changes when there are any.
    pub fn data_line(&self) -> String {
        let source = match &self.source {
            Source::Defaults => "tuning=defaults".to_owned(),
            Source::File { path, sha } => format!("tuning={} {sha}", path.display()),
        };
        match &self.changes {
            Some(sha) => format!("{source} admin={sha}"),
            None => source,
        }
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

    /// A tuning file (the live server loads one) cannot make a sight outlast
    /// its test: nothing is seen with no test longer than the longest think
    /// gap and a tick (450 ms with the defaults), a hold is within that, and
    /// a grace is a human's (150 to 400 ms).
    #[test]
    fn the_sight_windows_are_capped_by_the_think_cadence() {
        let wrong = |change: fn(&mut Tuning)| {
            let mut tuning = Tuning::default();
            change(&mut tuning);
            tuning.validate()
        };
        // The defaults are at the caps, and a longer think gap moves them.
        let at_the_cap = Tuning::default();
        assert_eq!(at_the_cap.sense.sight_untested_ms, at_the_cap.think.think_ms.1 + 50);
        // The refresh and the hold are on by default, the hold at the cap too (and so is the alias's).
        assert!(at_the_cap.sense.sight_refresh);
        assert_eq!(at_the_cap.cheats.sight_hold_ms, at_the_cap.sense.sight_untested_ms);
        assert_eq!(crate::v026g::bots::know::SIGHT_HOLD_MS, at_the_cap.sense.sight_untested_ms);
        assert!(wrong(|t| t.sense.sight_untested_ms = 450).is_ok());
        assert!(wrong(|t| t.sense.sight_untested_ms = 451).is_err());
        assert!(
            wrong(|t| {
                t.think.think_ms = (200, 600);
                t.sense.sight_untested_ms = 650;
            })
            .is_ok()
        );
        let named = |why: Result<(), String>, name: &str| {
            assert!(why.unwrap_err().starts_with(name));
        };
        named(wrong(|t| t.sense.sight_untested_ms = 5_000), "sense.sight_untested_ms");
        named(wrong(|t| t.cheats.sight_hold_ms = 451), "cheats.sight_hold_ms");
        assert!(wrong(|t| t.cheats.sight_hold_ms = 0).is_ok());
        assert!(wrong(|t| t.cheats.sight_hold_ms = 450).is_ok());
        assert!(wrong(|t| t.sense.sight_grace_ms = 400).is_ok());
        named(wrong(|t| t.sense.sight_grace_ms = 401), "sense.sight_grace_ms");
        assert!(wrong(|t| t.sense.sight_grace_ms = 149).is_err());
        // From a file, as the live server loads it.
        let dir = std::env::temp_dir().join(format!("srp-sight-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bots.json");
        std::fs::write(&path, r#"{"sense":{"sight_untested_ms":5000}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        std::fs::write(&path, r#"{"cheats":{"sight_hold_ms":600}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        let fine = r#"{"sense":{"sight_grace_ms":150},"cheats":{"sight_hold_ms":250}}"#;
        std::fs::write(&path, fine).unwrap();
        assert!(Tuning::load(&path).is_ok());
    }

    /// RFC 7386's examples of the merge, but for a `null`: it stays as the key's value (the field's none), at any
    /// depth, where the RFC takes the key out.
    #[test]
    fn the_merge_is_rfc_7386_but_a_null_is_none() {
        let cases = [
            (r#"{"a":"b"}"#, r#"{"a":"c"}"#, r#"{"a":"c"}"#),
            (r#"{"a":"b"}"#, r#"{"b":"c"}"#, r#"{"a":"b","b":"c"}"#),
            (r#"{"a":"b"}"#, r#"{"a":null}"#, r#"{"a":null}"#),
            (r#"{"a":"b","b":"c"}"#, r#"{"a":null}"#, r#"{"a":null,"b":"c"}"#),
            (r#"{"a":["b"]}"#, r#"{"a":"c"}"#, r#"{"a":"c"}"#),
            (r#"{"a":"c"}"#, r#"{"a":["b"]}"#, r#"{"a":["b"]}"#),
            (r#"{"a":{"b":"c"}}"#, r#"{"a":{"b":"d","c":null}}"#, r#"{"a":{"b":"d","c":null}}"#),
            (r#"{"a":[{"b":"c"}]}"#, r#"{"a":[1]}"#, r#"{"a":[1]}"#),
            (r#"["a","b"]"#, r#"["c","d"]"#, r#"["c","d"]"#),
            (r#"{"a":"b"}"#, r#"["c"]"#, r#"["c"]"#),
            (r#"{"a":"foo"}"#, "null", "null"),
            (r#"{"a":"foo"}"#, r#""bar""#, r#""bar""#),
            (r#"{"e":null}"#, r#"{"a":1}"#, r#"{"e":null,"a":1}"#),
            (r#"[1,2]"#, r#"{"a":"b","c":null}"#, r#"{"a":"b","c":null}"#),
            ("{}", r#"{"a":{"bb":{"ccc":null}}}"#, r#"{"a":{"bb":{"ccc":null}}}"#),
            (r#"{"a":null}"#, r#"{"a":{"b":null,"c":1}}"#, r#"{"a":{"b":null,"c":1}}"#),
        ];
        for (target, patch, want) in cases {
            let mut value: serde_json::Value = serde_json::from_str(target).unwrap();
            merge_patch(&mut value, serde_json::from_str(patch).unwrap());
            assert_eq!(value, serde_json::from_str::<serde_json::Value>(want).unwrap(), "{target} + {patch}");
        }
    }

    /// The owner's options are the defaults: a server with no file runs on the all-ON set, and the
    /// numbers a play names are the ones its table says.
    #[test]
    fn the_defaults_are_the_options_the_owner_turned_on() {
        let on = Tuning::default();
        assert!(on.heal.on && on.heal.bandages);
        assert!(on.traps.react.on && on.traps.react.shot && on.traps.react.pass);
        assert!(on.traps.react.jump && on.traps.react.hear && on.traps.react.defuse);
        assert_eq!((on.traps.per_life, on.traps.cap_team), (1, 4));
        assert!(on.grid.gap_links && on.grid.floor_paths && on.grid.client_numbers);
        assert!(on.grid.exact_erosion && on.grid.honest_drops);
        assert_eq!(on.kin.jump_snap, crate::v026g::bots::kin::JumpSnap::ByKeys);
        assert_eq!(on.feel.drop_truth, crate::v026g::bots::path::DropTruth::Links);
        assert_eq!(on.hazard.zone_pools, ZonePools::Routed);
        assert_eq!(on.cheats.sight_hold_ms, 450);
        assert_eq!(on.unstick.progress_ms, Some(6_000));
        assert_eq!(on.unstick.suicide_after_ms, Some(SUICIDE_AFTER_OWNER_MS));
        // The switches that stay off, each for its recorded reason.
        assert!(!on.cheats.batteries);
        assert!(!on.aim.combat.grenade_push_tracks);
        assert!(!on.aim.combat.grenade_volley);
        // A number a default tuning names is the table's own.
        let roam = &on.plays[&PlayKind::Roam].numbers;
        assert_eq!(roam.len(), 3);
        assert_eq!(on.number(PlayKind::Roam, "fresh_claims"), 1.0);
        assert_eq!(on.number(PlayKind::Buddy, "respawn_rule"), 1.0);
        for kind in PlayKind::ALL.iter() {
            for (name, value) in &on.plays[kind].numbers {
                let table = plays::numbers(*kind).iter().find(|(known, _)| known == name);
                assert_eq!(table.map(|(_, table)| table), Some(value), "{kind:?}.{name}");
            }
        }
    }

    /// The file is a merge patch: a play given with its numbers only keeps its rank and considerations, a whole
    /// tuning dumped from the defaults changes nothing, and `null` switches an optional field off.
    #[test]
    fn the_file_is_a_merge_patch_over_the_defaults() {
        let defaults = Tuning::default();
        let kept = Tuning::parse(r#"{"plays":{"roam":{"numbers":{"join_ahead":0}}}}"#).unwrap();
        assert_eq!(kept.number(PlayKind::Roam, "join_ahead"), 0.0);
        assert_eq!(kept.number(PlayKind::Roam, "fresh_claims"), 1.0);
        assert_eq!(kept.plays[&PlayKind::Roam].rank, defaults.plays[&PlayKind::Roam].rank);
        assert_eq!(kept.plays[&PlayKind::Roam].considerations, defaults.plays[&PlayKind::Roam].considerations);
        assert_eq!(kept.plays[&PlayKind::Buddy], defaults.plays[&PlayKind::Buddy]);
        // The whole tuning as a patch, and an empty one, are the defaults.
        let dump = serde_json::to_string(&defaults).unwrap();
        assert_eq!(Tuning::parse(&dump).unwrap().hash(), defaults.hash());
        assert_eq!(Tuning::parse("{}").unwrap().hash(), defaults.hash());
        // `null` for an optional field that has a value switches it off, and the rest is the defaults.
        let off = Tuning::parse(
            r#"{"unstick":{"progress_ms":null,"blind_stop_ms":null,"suicide_after_ms":null},"feel":{"carrier_jump_m":null},"sense":{"sprint_body_mps":null}}"#,
        )
        .unwrap();
        assert_eq!(off.unstick.progress_ms, None);
        assert_eq!(off.unstick.blind_stop_ms, None);
        assert_eq!(off.unstick.suicide_after_ms, None);
        assert_eq!(off.feel.carrier_jump_m, None);
        assert_eq!(off.sense.sprint_body_mps, None);
        assert_eq!(off.unstick.unknown_meant_needs_keys, defaults.unstick.unknown_meant_needs_keys);
        // Under a section that is none in the defaults (`threat.concede`): a `null` inside it is none too, a field
        // left out is the section's own default, and a `null` for the section is none. (`keep_after_death`: on in the
        // section's default, so its `null` is the case.)
        assert_eq!(defaults.threat.concede, None);
        let keep_off = Tuning::parse(r#"{"threat":{"concede":{"keep_after_death":null}}}"#).unwrap();
        assert_eq!(keep_off.threat.concede, Some(Concede { keep_after_death: None, ..Concede::default() }));
        let fails = Tuning::parse(r#"{"threat":{"concede":{"fails":3}}}"#).unwrap();
        assert_eq!(fails.threat.concede, Some(Concede { fails: 3, ..Concede::default() }));
        assert!(Concede::default().keep_after_death.is_some());
        let none = Tuning::parse(r#"{"threat":{"concede":null}}"#).unwrap();
        assert_eq!(none.hash(), defaults.hash());
        // The same through `apply_patch` over another base (the bench's way): over all-off too.
        let mut value = serde_json::to_value(Tuning::all_off()).unwrap();
        Tuning::apply_patch(&mut value, serde_json::json!({"threat": {"concede": {"keep_after_death": null}}})).unwrap();
        let read = Tuning::from_value(value).unwrap();
        assert_eq!(read.threat.concede, Some(Concede { keep_after_death: None, ..Concede::default() }));
        // A `null` for a field that cannot be none, or for a name the tuning does not have, is refused, whichever way
        // it is said.
        for bad in [
            r#"{"heal":{"on":null}}"#,
            r#"{"sense":null}"#,
            r#"{"plays":{"roam":null}}"#,
            r#"{"threat":{"concede":{"fails":null}}}"#,
            r#"{"threat":{"concede":{"no_such":null}}}"#,
            r#"{"smoke":{"no_such":null}}"#,
            r#"{"sense":{"shot_range":70}}"#,
            r#"[1]"#,
            "{not json",
        ] {
            assert!(Tuning::parse(bad).is_err(), "{bad}");
        }
    }

    /// A server never runs on settings other than the file it was told: a file that cannot be used is an error at
    /// the start with the file and the reason, and the defaults are for no file at all.
    #[test]
    fn a_tuning_file_the_server_cannot_use_is_an_error_not_the_defaults() {
        let dir = std::env::temp_dir().join(format!("srp-loaded-tuning-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bots.json");
        let loaded = |text: &str| {
            std::fs::write(&path, text).unwrap();
            LoadedTuning::from_file(&path)
        };
        let ok = loaded(r#"{"plays":{"roam":{"numbers":{"join_ahead":0}}}}"#).unwrap();
        assert_eq!(ok.tuning().number(PlayKind::Roam, "join_ahead"), 0.0);
        assert!(ok.data_line().contains("bots.json sha256:"), "{}", ok.data_line());
        assert_ne!(ok, LoadedTuning::default());
        assert_eq!(loaded("{}").unwrap().tuning().hash(), LoadedTuning::default().tuning().hash());
        assert_eq!(LoadedTuning::default().data_line(), "tuning=defaults");
        // The first form of the old failure: a play with numbers only, in a file that is a full tuning but for that.
        assert!(loaded(r#"{"plays":{"roam":{"numbers":{"join_ahead":1}},"buddy":{"numbers":{"respawn_rule":1}}}}"#).is_ok());
        for bad in ["", "{not json", r#"{"sense":{"no_such":1}}"#, r#"{"plays":{"roam":{"numbers":{"no_such":1}}}}"#, r#"{"sense":{"view_deg":0}}"#] {
            let why = loaded(bad).unwrap_err();
            assert!(why.starts_with("SRP_BOT_TUNING ") && why.contains("bots.json"), "{bad}: {why}");
        }
        let missing = LoadedTuning::from_file(&dir.join("absent.json")).unwrap_err();
        assert!(missing.contains("absent.json"), "{missing}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `all_off.json` is the patch that puts every option the defaults have on back off (the bench's OFF arm,
    /// `SRP_BENCH_TUNING`): each of its values differs from the default it patches, so none is stale, the tuning it
    /// makes is a valid one with the options off, and it never brings back a value wrong by the game (`KEPT_RIGHT`) or a
    /// cheat (`NO_CHEATS`).
    #[test]
    fn the_all_off_patch_switches_off_what_the_defaults_have_on() {
        let patch: serde_json::Value = serde_json::from_str(include_str!("all_off.json")).unwrap();
        let defaults = serde_json::to_value(Tuning::default()).unwrap();
        fn leaves<'a>(value: &'a serde_json::Value, at: String, out: &mut Vec<(String, &'a serde_json::Value)>) {
            match value {
                serde_json::Value::Object(map) => {
                    for (key, value) in map {
                        leaves(value, format!("{at}/{key}"), out);
                    }
                }
                other => out.push((at, other)),
            }
        }
        let mut changes = Vec::new();
        leaves(&patch, String::new(), &mut changes);
        assert!(changes.len() > 60, "{}", changes.len());
        // Switches whose old value is wrong by the client or the game, read in the exe or the data or measured (A223, the
        // owner: "All that is wrong for sure shouldnt be returned even if we testing all-off"): the patch never names them, so
        // the all-off arm keeps the default. Each field's doc gives its proof. An old value that is only weaker (a habit, a
        // guard, less information) is no bug and stays in the patch.
        const KEPT_RIGHT: &[&str] = &[
            "/aim/combat/grenade_kit_cap",
            "/aim/combat/hit_cover_reach",
            "/aim/combat/live_eye",
            "/aim/combat/mirror_gated",
            "/aim/friends/spread_source",
            "/cover/pierce",
            "/grid/client_numbers",
            "/grid/drop_room",
            "/hazard/zone_intensity",
            "/hazard/zone_pools",
            "/heal/drug_stops",
            "/kin/fix_floor",
            "/kin/jump_snap",
            "/kin/pitch_scale",
            "/sense/ranges",
            "/sense/shots",
            "/sense/sprint_body_mps",
            "/sense/use_ray_m",
            "/think/stamina_rates",
        ];
        // Switches whose old value is a cheat, information no player has (A234, the owner: "Yes, drop cheats"; the rule
        // "smarter, never cheaters"): the patch never names them either, so the all-off arm keeps the default.
        const NO_CHEATS: &[&str] = &[
            "/aim/combat/aim_last_known",
            "/aim/combat/band_carrier_seen_ms",
            "/aim/combat/worth/hurt_from",
            "/sense/bush_notice",
            "/sense/grass_notice",
            "/smoke/sight",
        ];
        for pointer in KEPT_RIGHT {
            assert!(defaults.pointer(pointer).is_some(), "{pointer} is no switch of the tuning");
            assert!(patch.pointer(pointer).is_none(), "{pointer}'s old value is wrong by the game: the all-off arm keeps the default");
        }
        for pointer in NO_CHEATS {
            assert!(defaults.pointer(pointer).is_some(), "{pointer} is no switch of the tuning");
            assert!(patch.pointer(pointer).is_none(), "{pointer}'s old value is information no player has: the all-off arm keeps the default");
        }
        // Switches that are off by default and already named in the patch, so that the change of their default to on cannot
        // leave them on in the all-off arm (A195 `cover.hold_exposure`). Once a default is on the entry is a real one: take it off here.
        const OFF_BY_DEFAULT: &[&str] = &["/kin/margin_edge", "/feel/past_drop", "/feel/sprint_turns", "/spacing/apart_tie", "/spacing/lane"];
        for (pointer, off) in changes {
            if OFF_BY_DEFAULT.contains(&pointer.as_str()) {
                assert_eq!(defaults.pointer(&pointer), Some(off), "{pointer} is on by default now: take it off OFF_BY_DEFAULT");
                continue;
            }
            // A play's number that is not in the default tuning is read at its table's value, which is on.
            let on = defaults.pointer(&pointer).cloned().or_else(|| {
                let (play, name) = pointer.strip_prefix("/plays/")?.split_once("/numbers/")?;
                let kind: PlayKind = serde_json::from_value(play.into()).ok()?;
                plays::numbers(kind).iter().find(|(known, _)| *known == name).map(|(_, value)| (*value).into())
            });
            assert!(on.as_ref().is_some_and(|on| on != off), "{pointer} is {on:?} by default, the patch says {off}");
        }
        let off = Tuning::patched(patch).unwrap();
        assert!(!off.heal.on && !off.traps.react.on && !off.grid.gap_links && !off.think.stamina);
        assert_eq!(off.number(PlayKind::Roam, "fresh_claims"), 0.0);
        assert_eq!(off.number(PlayKind::Buddy, "respawn_rule"), 0.0);
        assert_eq!(off.unstick.progress_ms, None);
        assert_eq!(off.feel.carrier_jump_m, None);
        assert!(off.aim.combat.grenade_push_tracks);
        let off = serde_json::to_value(off).unwrap();
        for pointer in KEPT_RIGHT.iter().chain(NO_CHEATS) {
            assert_eq!(off.pointer(pointer), defaults.pointer(pointer), "{pointer}");
        }
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
        assert_eq!(wrong(|t| t.sense.use_ray_m = 0.0), "sense.use_ray_m");
        assert_eq!(wrong(|t| t.sense.use_ray_m = 5.5), "sense.use_ray_m");
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
        assert_eq!(wrong(|t| t.aim.combat.band_cross_deg = Some(95.0)), "aim.combat.band_cross_deg");
        assert_eq!(wrong(|t| t.aim.combat.band_cross_deg = Some(-5.0)), "aim.combat.band_cross_deg");
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

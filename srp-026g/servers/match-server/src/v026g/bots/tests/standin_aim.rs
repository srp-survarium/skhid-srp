//! The stand-in camper's aim (A193): the human he stands in for, with a reflex of his own, not the bots' defaults.
//!
//! The camper's reflex, noticing and shot played by `Tuning::default()` until now, so every change of the bots' aim moved the
//! "human" that every bench meets as well (the far aim of A182 made his bolt hit more, and a bench of the next release would have
//! met a camper of another build than the one it was compared with). His aim is now the file `camper_human_aim.json`: a whole
//! `aim` section ([`AimTuning`]: the error, the wander, the settling, the reaction, the fire rules and the reflex's `combat`),
//! read once, which no default reaches. A test names every key of the current default and fails when one is missing from the
//! file, so a new aim switch is a decision for the camper too (his value, written in the file), not a silent follow of the bots'.
//! The rest of his tuning (senses, motion, the decisions the stand-in's drive bypasses) is the build's defaults as before.

use super::*;
use crate::v026g::bots::tuning::AimTuning;

/// The camper's `aim` section, as written.
const HUMAN_AIM: &str = include_str!("camper_human_aim.json");

/// The aim of the stand-in camper: [`HUMAN_AIM`].
pub(crate) fn human_aim() -> AimTuning {
    serde_json::from_str(HUMAN_AIM).expect("camper_human_aim.json is a whole aim section")
}

/// The tuning the stand-in camper plays by: the build's defaults, with [`human_aim`] as his aim.
pub(crate) fn human_tuning() -> Tuning {
    Tuning { aim: human_aim(), ..Tuning::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every path of an object's keys down to its leaves, `/`-joined.
    fn key_paths(value: &serde_json::Value, prefix: &str, into: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, inner) in map {
                    key_paths(inner, &format!("{prefix}/{key}"), into);
                }
            }
            _ => into.push(prefix.to_string()),
        }
    }

    #[test]
    fn the_camper_file_names_every_key_of_the_aim() {
        let mut default = Vec::new();
        key_paths(&serde_json::to_value(AimTuning::default()).expect("JSON"), "", &mut default);
        let mut pinned = Vec::new();
        key_paths(&serde_json::from_str(HUMAN_AIM).expect("JSON"), "", &mut pinned);
        default.sort();
        pinned.sort();
        let missing: Vec<_> = default.iter().filter(|key| !pinned.contains(key)).collect();
        let extra: Vec<_> = pinned.iter().filter(|key| !default.contains(key)).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "tests/camper_human_aim.json: write the camper's value for {missing:?}, drop {extra:?}"
        );
    }

    #[test]
    fn the_camper_plays_by_a_tuning_that_validates() {
        human_tuning().validate().expect("the camper's tuning is a valid one");
    }

    /// `AIM_DUMP=<file> cargo test dump_the_default_aim -- --ignored`: the file's text for the defaults of this build.
    #[test]
    #[ignore]
    fn dump_the_default_aim() {
        let path = std::env::var("AIM_DUMP").expect("AIM_DUMP=<file>");
        std::fs::write(path, serde_json::to_string_pretty(&AimTuning::default()).expect("JSON") + "\n").expect("written");
    }
}

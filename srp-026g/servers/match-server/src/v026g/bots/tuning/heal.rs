use serde::{Deserialize, Serialize};

use super::check;

/// The consumable play (`plays/heal.rs`, play 22): one rule for the medkit,
/// the bandages and the antitoxin, from the best players' numbers
/// (`fighting-record.md` §8.8): they press 2-3 s after the first loss, at
/// any depth, 79 % of their hurt presses with the body under 0.8, moving and
/// firing, with an enemy within 30 m in 57 % of them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HealTuning {
    /// The graded rule. Off: the old one, a medkit under `below` out of
    /// every known enemy's line of sight, then still and crouched for
    /// `wait_s` (`wait_s.0` to `wait_s.1`), fire held.
    pub graded: bool,
    /// A need under this scores nothing: a body at 0.85 is not hurt enough
    /// for a unit of the kit (the humans' 79 % under 0.8 is need 0.2).
    pub start_need: f32,
    /// A need from this takes the alarm band: the play beats any other
    /// play. 0.2 is a body under 0.8: the bot knows its health a world late,
    /// so in a fight a lower line presses at a true 0.4-0.55, the best
    /// players' median being 0.65 (`pr-heal.md`, the timing section).
    pub urgent_need: f32,
    /// How much of a pool's missing share counts in a need: what kills
    /// counts fully, what slows less (a broken leg 0.38; an infection at 0.8
    /// reaches `start_need`).
    pub weights: NeedWeights,
    /// The heal is a habit of the reflex (like the reload): it presses the
    /// item's key at once whatever the bot is doing, no play is taken, no
    /// running play is displaced (the take, the route end and the commander's
    /// re-choice of each press are gone). Off: the play (`plays/heal.rs`),
    /// with its cover step first, as in #520's first look.
    pub interrupt: bool,
    /// The cover walk during the item's delay goes this far a second, m
    /// (the walk pace, 3.5 m/s in `kin.rs`, PROVEN from the client's speeds):
    /// a medkit's 2 s delay (PROVEN, `fact-health-recovery.md`) is a 7 m
    /// walk, a bandage's 1 s 3.5 m. The reflex's own cover search
    /// (`break_sight`, 48 samples, from 2 m) finds the spot within it.
    pub walk_mps: f32,
    /// No cover walk, and the old play's cover step is not taken, with an
    /// enemy it knows of within this, m: GUESS (the humans press within 10 m
    /// in 10 % of their presses; whether they walk off is not in the data).
    pub enemy_m: f32,
    /// The old play's cover step (`interrupt` off): taken first only if a
    /// cover spot is within `cover_m`, m, and lasting at most
    /// `cover_step_ms`. Otherwise it presses at once.
    pub cover_m: f32,
    pub cover_step_ms: u32,
    /// The old rule's numbers (`graded` off).
    pub below: f32,
    pub wait_s: (f32, f32),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NeedWeights {
    pub body: f32,
    pub head: f32,
    pub legs: f32,
    pub hands: f32,
    pub pain: f32,
    pub infection: f32,
}

impl Default for NeedWeights {
    fn default() -> Self {
        Self {
            body: 1.0,
            head: 1.0,
            legs: 0.38,
            hands: 0.3,
            pain: 0.1,
            infection: 0.75,
        }
    }
}

impl Default for HealTuning {
    fn default() -> Self {
        Self {
            graded: true,
            start_need: 0.15,
            urgent_need: 0.2,
            weights: NeedWeights::default(),
            interrupt: true,
            walk_mps: 3.5,
            enemy_m: 10.0,
            cover_m: 4.0,
            cover_step_ms: 1_500,
            below: 0.6,
            wait_s: (1.5, 3.0),
        }
    }
}

pub(super) fn validate(heal: &HealTuning) -> Result<(), String> {
    let w = &heal.weights;
    check(
        (0.0..1.0).contains(&heal.start_need) && heal.start_need <= heal.urgent_need,
        "heal.start_need",
    )?;
    check(heal.urgent_need <= 1.0, "heal.urgent_need")?;
    check(
        [w.body, w.head, w.legs, w.hands, w.pain, w.infection]
            .iter()
            .all(|w| (0.0..=1.0).contains(w)),
        "heal.weights",
    )?;
    check(
        heal.cover_m >= 0.0 && heal.enemy_m >= 0.0 && heal.cover_step_ms <= 5_000,
        "heal.cover_*",
    )?;
    check((0.0..=10.0).contains(&heal.walk_mps), "heal.walk_mps")?;
    check(heal.below > 0.0 && heal.below <= 1.0, "heal.below")?;
    check(
        0.0 < heal.wait_s.0 && heal.wait_s.0 <= heal.wait_s.1,
        "heal.wait_s",
    )
}

#[cfg(test)]
mod tests {
    use super::super::Tuning;

    #[test]
    fn the_rule_is_on_by_default_and_a_file_changes_only_what_it_names() {
        let defaults = Tuning::default();
        assert!(defaults.heal.graded);
        let dir = std::env::temp_dir().join(format!("srp-heal-tuning-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bots.json");
        std::fs::write(&path, r#"{"heal":{"graded":false,"weights":{"legs":0.2}}}"#).unwrap();
        let tuning = Tuning::load(&path).unwrap();
        assert!(!tuning.heal.graded);
        assert_eq!(tuning.heal.weights.legs, 0.2);
        assert_eq!(tuning.heal.weights.body, defaults.heal.weights.body);
        assert_eq!(tuning.heal.start_need, defaults.heal.start_need);
        std::fs::write(&path, r#"{"heal":{"start_need":0.5,"urgent_need":0.3}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        std::fs::write(&path, r#"{"heal":{"medkits":3}}"#).unwrap();
        assert!(Tuning::load(&path).is_err(), "unknown fields are refused");
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}

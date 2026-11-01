use serde::{Deserialize, Serialize};

use super::{check, Check};

/// The consumable play (`plays/heal.rs`, play 22): one rule for the medkit,
/// the bandages and the antitoxin, from the best players' numbers
/// (`fighting-record.md` §8.8): they press 2-3 s after the first loss, at
/// any depth, 79 % of their hurt presses with the body under 0.8, moving and
/// firing, with an enemy within 30 m in 57 % of them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HealTuning {
    /// The consumable rework as a whole (#520), **on by default; with it off the bots are the old tip's** (the old
    /// Heal play: a medkit under the play's `below` number out of every known enemy's line of sight, then still
    /// and crouched, fire held; Retreat's medkit count per life). On: the graded need rule over the pools, the
    /// drugs of the kit counted for the match from the worlds (`stock.rs`), and, with `interrupt`, the reflex's
    /// habit. The bench's stand-in has the drugs only with it on (`standin_heal::model_on`).
    pub on: bool,
    /// The bandage habit, **on by default** (needs `on`): the pain pool counts as a need of its own for the
    /// kit's bandages, so a bot presses them after a medkit when its pain pool is low, or alone when the body
    /// is full (the owner, 2026-10-26: "bots that carry bandages use them"). The medkit gives no pain
    /// (`resources.db` rev 32: the bandages give pain +0.35, body +0.35, head +0.14, full limbs; 1 s delay,
    /// 1 s active), and in the pools run the team kills of the graded rule came on a pain pool under 0.6 (20
    /// of 33 lethal hits). Off, the pain weighs `weights.pain` (0.1: never alone a need).
    pub bandages: bool,
    /// With `bandages` on, the pain pool's weight in a need, in place of `weights.pain`: 0.4 makes a pain
    /// pool under about 0.62 a need of `start_need` (0.15). The owner's "pain under 0.6" is the line; the
    /// weight is a GUESS that reaches it.
    pub bandage_pain_weight: f32,
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
    /// [A217] What the bot makes of a bullet that hits it while a drug works ([`DrugStops`]; patch 84, live since
    /// revision 40).
    pub drug_stops: DrugStops,
}

/// [A217] Patch 84 (live since revision 40; `origin/main:srp-026g/docs/notes/drug-interrupt.md`, PROVEN in the
/// exe): a ranged hit that took health (`damage_model::hit_body_part` 0x2E5080 said 1) stops every healing drug the
/// victim has in use. In its delay nothing is healed; in its active time what it healed stays. The unit stays used,
/// and another can be pressed at once. The player sees it: the drug's quick slot shows red for a second, and the
/// health bar stops. The bot sees the hit as the world's `m_last_hit_time_ms`, which the same function writes only
/// for a hit that took health (0x2E5134, PROVEN), whatever its type ([`crate::v026g::bots::stock::Press`]).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrugStops {
    /// As the bots were before patch 84: a drug pressed works to its end whatever hits the bot, so a medkit shot
    /// through in its delay is counted as healing and blocks the next press for its whole window. Wrong since
    /// revision 40; kept to copy the old live bots.
    Unknown,
    /// A drug that a hit stopped is known stopped: it heals no more in the reckoning, and the item may be pressed
    /// again at once. The press itself is as before (at once, the cover walk in its delay).
    Known,
    /// [`Self::Known`], and the press waits while hits land (the default): no drug is pressed within `quiet_ms` of
    /// the last hit that took health, and the bot walks to cover meanwhile, as it does in a press's delay.
    /// "Break the line of sight, then heal": a press under fire is a unit a hit in the next 2 s throws away.
    Wait {
        /// GUESS: 1.5 s, under the medkit's 2 s delay; the bot learns of a hit a world late.
        quiet_ms: u32,
    },
}

impl DrugStops {
    /// Whether a drug a hit stopped is known stopped.
    pub fn known(self) -> bool {
        !matches!(self, Self::Unknown)
    }

    /// How long after the last hit that took health a press waits, ms (none: it does not wait).
    pub fn quiet_ms(self) -> Option<u32> {
        match self {
            Self::Wait { quiet_ms } => Some(quiet_ms),
            Self::Unknown | Self::Known => None,
        }
    }
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

impl HealTuning {
    /// The weights a need is reckoned with: the tuning's, the pain's own with the bandage habit on.
    pub fn weights_in_use(&self) -> NeedWeights {
        NeedWeights {
            pain: if self.bandages { self.bandage_pain_weight } else { self.weights.pain },
            ..self.weights
        }
    }
}

impl Default for HealTuning {
    fn default() -> Self {
        Self {
            on: true,
            bandages: true,
            bandage_pain_weight: 0.4,
            start_need: 0.15,
            urgent_need: 0.2,
            weights: NeedWeights::default(),
            interrupt: true,
            walk_mps: 3.5,
            enemy_m: 10.0,
            cover_m: 4.0,
            cover_step_ms: 1_500,
            drug_stops: DrugStops::Wait { quiet_ms: 1_500 },
        }
    }
}

impl Check for HealTuning {
    fn check(&self) -> Result<(), String> {
        let w = &self.weights;
        check(
            (0.0..1.0).contains(&self.start_need) && self.start_need <= self.urgent_need,
            "heal.start_need",
        )?;
        check(self.urgent_need <= 1.0, "heal.urgent_need")?;
        check(
            [w.body, w.head, w.legs, w.hands, w.pain, w.infection]
                .iter()
                .all(|w| (0.0..=1.0).contains(w)),
            "heal.weights",
        )?;
        check(
            self.cover_m >= 0.0 && self.enemy_m >= 0.0 && self.cover_step_ms <= 5_000,
            "heal.cover_*",
        )?;
        check((0.0..=1.0).contains(&self.bandage_pain_weight), "heal.bandage_pain_weight")?;
        check(self.drug_stops.quiet_ms().is_none_or(|ms| ms <= 10_000), "heal.drug_stops.wait.quiet_ms")?;
        check((0.0..=10.0).contains(&self.walk_mps), "heal.walk_mps")
    }
}

#[cfg(test)]
mod tests {
    use super::super::Tuning;

    #[test]
    fn the_rework_is_on_by_default_and_a_file_changes_only_what_it_names() {
        let defaults = Tuning::default();
        assert!(defaults.heal.on && defaults.heal.bandages, "the owner's set: the rework and the bandages are on");
        assert!(defaults.heal.interrupt, "the habit is on once the rework is");
        let dir = std::env::temp_dir().join(format!("srp-heal-tuning-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bots.json");
        std::fs::write(&path, r#"{"heal":{"weights":{"legs":0.2}}}"#).unwrap();
        let tuning = Tuning::load(&path).unwrap();
        assert!(tuning.heal.on);
        assert_eq!(tuning.heal.weights.legs, 0.2);
        assert_eq!(tuning.heal.weights.body, defaults.heal.weights.body);
        assert_eq!(tuning.heal.start_need, defaults.heal.start_need);
        std::fs::write(&path, r#"{"heal":{"on":false}}"#).unwrap();
        assert!(!Tuning::load(&path).unwrap().heal.on, "the bots as they were");
        std::fs::write(&path, r#"{"heal":{"start_need":0.5,"urgent_need":0.3}}"#).unwrap();
        assert!(Tuning::load(&path).is_err());
        std::fs::write(&path, r#"{"heal":{"medkits":3}}"#).unwrap();
        assert!(Tuning::load(&path).is_err(), "unknown fields are refused");
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
}

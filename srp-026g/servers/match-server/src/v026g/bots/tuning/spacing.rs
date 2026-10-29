//! How much the team pulls its bots together: the plays and the callouts that gather them (A151, the bots keep 3.5
//! teammates within 30 m against 1.65 for the live players). Today's behaviour is the default of each; the other
//! settings are the bench's diagnosis arms (which of them brings the clump), read per team.

use super::*;

/// What the team's callouts tell, and how old a place may be and still steer a play.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Callouts {
    /// Every first-hand sight and sound is told to the teammates after `sense.callout_ms`, and a play aimed at a place
    /// follows its track whatever its age (the tracks live `sense.drop_ms`): today.
    #[default]
    SeenAndHeard,
    /// Only what a bot saw is told (a sound is its own), and a play aimed at a place follows a track no older than
    /// `steer_age_ms`.
    SeenOnly { steer_age_ms: u32 },
}

impl Callouts {
    /// Whether a sound a bot heard is told to its teammates.
    pub fn tells_heard(self) -> bool {
        matches!(self, Self::SeenAndHeard)
    }

    /// The oldest track (ms) a play aimed at a place steers by, or none (any).
    pub fn steer_age_ms(self) -> Option<u32> {
        match self {
            Self::SeenAndHeard => None,
            Self::SeenOnly { steer_age_ms } => Some(steer_age_ms),
        }
    }
}

/// The commander's piling consideration (A151 step 2b): a play that puts a bot where teammates' plays already go is scored
/// lower when the commander hands the plays out, so the third bot takes its next best. The trade partner never counts and
/// the first `free` mates are free (mutual support stays; the next ones pile).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Piling {
    /// Two targets are one place when their points are within this, m (ground distance); targets with no point only
    /// when they are the same target.
    pub radius_m: Metres,
    /// This many teammates (the partner not counted) on a place are free. GUESS.
    pub free: u32,
    /// The play's score falls by this share for each teammate past `free`. GUESS.
    pub step: Share,
    /// ... but to no less than this share of its score. GUESS.
    pub floor: Share,
}

impl Default for Piling {
    fn default() -> Self {
        Self { radius_m: Metres::of(15.0), free: 1, step: Share::of(0.375), floor: Share::of(0.25) }
    }
}

/// [`Piling`] parsed once for the commander.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilingRule {
    pub radius_m: f32,
    pub free: u32,
    pub step: f32,
    pub floor: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpacingTuning {
    /// The Buddy play (a follower within trading reach of its leader) is offered. Today: yes.
    pub buddy: bool,
    /// What the callouts tell and how old a place steers a play.
    pub callouts: Callouts,
    /// The commander's piling consideration. `None` (off): the plays are handed out as before. On by default.
    pub piling: Option<Piling>,
}

impl Default for SpacingTuning {
    fn default() -> Self {
        Self { buddy: true, callouts: Callouts::SeenAndHeard, piling: Some(Piling::default()) }
    }
}

impl SpacingTuning {
    /// The rule the commander goes by, or none.
    pub fn piling_rule(&self) -> Option<PilingRule> {
        self.piling.map(|p| PilingRule { radius_m: p.radius_m.get(), free: p.free, step: p.step.get(), floor: p.floor.get() })
    }
}

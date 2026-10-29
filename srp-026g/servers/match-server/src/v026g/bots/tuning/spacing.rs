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

/// How a respawned bot leaves its spawn (`spawn_exit`, PR C).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnExits {
    /// By its play's shortest way, whatever its teammates took (before PR C).
    Off,
    /// By one of the base's exits: its way to the goal through it times `1 + claim_cost` for each teammate that claimed it
    /// in the last 20 s, among the exits whose way is at most `slack_m` longer than the shortest. GUESS both: 20 m is a few
    /// seconds' walk; 1.0 makes a second claimer double an exit's cost.
    Spread { slack_m: f32, claim_cost: f32 },
}

impl Default for SpawnExits {
    fn default() -> Self {
        Self::Spread { slack_m: 20.0, claim_cost: 1.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpacingTuning {
    /// The Buddy play (a follower within trading reach of its leader) is offered. Today: yes.
    pub buddy: bool,
    /// What the callouts tell and how old a place steers a play.
    pub callouts: Callouts,
    /// How a respawned bot leaves its spawn (PR C: the first 20 s of a life hold about half the clump's excess).
    pub spawn_exits: SpawnExits,
}

impl Default for SpacingTuning {
    fn default() -> Self {
        Self { buddy: true, callouts: Callouts::SeenAndHeard, spawn_exits: SpawnExits::default() }
    }
}

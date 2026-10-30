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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpacingTuning {
    /// The Buddy play (a follower within trading reach of its leader) is offered. Today: yes.
    pub buddy: bool,
    /// What the callouts tell and how old a place steers a play.
    pub callouts: Callouts,
    /// A175 (the owner 2026-10-30: they should NOT stack, "too close to each other or maybe even inside each other, as there is no collision for teammates"; 0.87 % of the
    /// bench's living teammate pair-seconds are within 1 m against the live players' 0.09 %, two thirds of it two bodies walking the same way in file): a bot with a living teammate
    /// inside `apart_m` steps aside, away from him, where the ground that side is clear for a stride (`bots/apart.rs`, the fight's own side-step test). Off: the bots go through each other.
    pub apart: bool,
    /// The distance inside which a teammate is in the way, metres on the ground (and less than 2 m apart in height). The live players are within 1 m of a teammate in 0.38 % of the world samples.
    pub apart_m: Metres,
    /// The follow plays (Escort, Buddy, PairUp, CoverHuman) hold a teammate at a short distance on purpose: with this off they do not step aside (the carrier's escort stays at its post).
    pub apart_follow: bool,
}

impl Default for SpacingTuning {
    fn default() -> Self {
        Self { buddy: true, callouts: Callouts::SeenAndHeard, apart: true, apart_m: Metres::of(1.1), apart_follow: true }
    }
}

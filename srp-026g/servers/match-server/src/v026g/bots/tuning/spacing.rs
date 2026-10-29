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

/// What a spot costs for the teammates already near it (A151 step 2a): a bot choosing a place to hold, hide or wait at
/// prefers one with fewer teammates round it, as a spot the enemy's bullet reaches less is preferred (`cover.spot_cost`).
/// Mutual support is kept: the first `free_mates` near are free (a trade partner), each one more costs `cost`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpotCrowd {
    /// A teammate within this of the spot is near it, m (ground distance).
    pub radius_m: Metres,
    /// This many teammates near the spot are free.
    pub free_mates: u32,
    /// The spot's score falls by this for each teammate near it past `free_mates`. The spot score is metres-scale
    /// (-0.01 a metre of walk), so 0.125 is 12.5 m of walk a teammate. GUESS.
    pub cost: Positive,
}

impl Default for SpotCrowd {
    fn default() -> Self {
        Self { radius_m: Metres::of(15.0), free_mates: 1, cost: Positive::of(0.125) }
    }
}

/// [`SpotCrowd`] parsed once for the spot search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrowdRule {
    pub radius_m: f32,
    pub free_mates: u32,
    pub cost: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpacingTuning {
    /// The Buddy play (a follower within trading reach of its leader) is offered. Today: yes.
    pub buddy: bool,
    /// What the callouts tell and how old a place steers a play.
    pub callouts: Callouts,
    /// The friend-crowding cost of a spot (`SpotQuery` searches of the plays that choose where to hold, wait or flank, and
    /// the picks from the lists the hold, crossfire and cut plays keep). `None` (off): a spot is chosen as before.
    /// On by default.
    pub spot_crowd: Option<SpotCrowd>,
}

impl Default for SpacingTuning {
    fn default() -> Self {
        Self { buddy: true, callouts: Callouts::SeenAndHeard, spot_crowd: Some(SpotCrowd::default()) }
    }
}

impl SpacingTuning {
    /// The rule the spot search goes by, or none.
    pub fn crowd_rule(&self) -> Option<CrowdRule> {
        self.spot_crowd.map(|c| CrowdRule { radius_m: c.radius_m.get(), free_mates: c.free_mates, cost: c.cost.get() })
    }
}

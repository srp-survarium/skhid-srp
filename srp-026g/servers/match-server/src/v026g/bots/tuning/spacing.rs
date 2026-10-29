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

/// The first-leg corridor cost (A151, Killzone 2's "squad corridors"): a body that came alive lately plans its way
/// with a region its mates were in lately costing more, so the second and third bot out of an exit take the next
/// corridor when it is not much longer. Live players who were together 10 s after a respawn are 36 % apart by 20 s;
/// the bots 21 % (fx-cover's respawn-wave check: they fork later, the same direction at 10 s as the humans).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FirstLeg {
    /// A body this many seconds after it came alive, or less, plans by the cost. GUESS (the clump is at 10-20 s).
    pub young_s: u32,
    /// The cost counts the mates that were in a region in the last this many seconds. GUESS.
    pub trail_s: u32,
    /// A region costs this much more of its length for each mate on it (up to `max_mates`). GUESS.
    pub step: Share,
    /// The mates that count in a region. GUESS.
    pub max_mates: u32,
    /// The way the cost finds is kept when it is at most this share longer than the plain way. GUESS (about 15 %; an
    /// exact fraction, 1/8; Killzone's corridors have no such cap, ours must not cost delivery).
    pub detour: Share,
    /// The plain way is looked at only when this many metres of its first 60 m run through regions mates were in. GUESS.
    pub shared_m: u32,
    /// Floors the costed search may expand, at most. GUESS (the K1 whole-route search runs out at about 1,900).
    pub floors: u32,
}

impl Default for FirstLeg {
    fn default() -> Self {
        Self {
            young_s: 25,
            trail_s: 15,
            step: Share::of(0.5),
            max_mates: 3,
            detour: Share::of(0.125),
            shared_m: 12,
            floors: 1_500,
        }
    }
}

/// [`FirstLeg`] parsed once, in the units the code uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FirstLegRule {
    pub young_ms: u32,
    pub trail_ms: u32,
    pub step: f32,
    pub max_mates: u32,
    pub detour: f32,
    pub shared_m: f32,
    pub floors: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpacingTuning {
    /// The Buddy play (a follower within trading reach of its leader) is offered. Today: yes.
    pub buddy: bool,
    /// What the callouts tell and how old a place steers a play.
    pub callouts: Callouts,
    /// The first-leg corridor cost for the bodies that came alive lately; none (off): the plain way.
    pub first_leg: Option<FirstLeg>,
}

impl Default for SpacingTuning {
    fn default() -> Self {
        Self { buddy: true, callouts: Callouts::SeenAndHeard, first_leg: Some(FirstLeg::default()) }
    }
}

impl SpacingTuning {
    /// The first-leg corridor cost's numbers, if it is on.
    pub fn first_leg_rule(&self) -> Option<FirstLegRule> {
        self.first_leg.map(|f| FirstLegRule {
            young_ms: f.young_s * 1000,
            trail_ms: f.trail_s * 1000,
            step: f.step.get(),
            max_mates: f.max_mates,
            detour: f.detour.get(),
            shared_m: f.shared_m as f32,
            floors: f.floors as usize,
        })
    }
}

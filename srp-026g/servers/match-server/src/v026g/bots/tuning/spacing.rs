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
    /// Where the step aside is refused (no clear ground either side: a corridor, a lip) and a teammate inside `apart_m` is ahead and moving the same way, the bot behind lets go of
    /// forward for the tick: the one ahead pulls away and the file opens (the live players' nearest mate is a median 21 m away). Off: it walks on his heels.
    pub apart_ease: bool,
    /// A teammate exactly on the bot's line (within 0.15 m across) is stepped away from to the side the lower place takes; where that side is refused and the other is clear, the bot
    /// steps to the other (29 % of the 1 m pair-seconds the step aside leaves are refused with the other side clear). Off: the refused step is dropped.
    pub apart_tie: bool,
    /// A175 (the A175 note, 2026-10-31): bots of one team that go the same way walk one line at one speed, with nothing between them (no collision for teammates): of the pairs
    /// moving alike within 16 m 26.6 % are on the same line (under 0.5 m across) against the live players' 12.4 %, and the step aside (one key for one tick) is pulled back by the
    /// route steering. With a lane a bot with a teammate in file on its line walks a parallel line `offset_m` to the side away from him (`bots/apart.rs` [`Lane`]), by shifting the
    /// point its walk steers at; off: every bot steers at the route's own line.
    pub lane: Option<LaneTuning>,
    /// A212 (the owner: bots stand inside each other, "before release"): at a match's opening a team's bodies leave adjacent spawn points together and close up on the way out,
    /// 83 % of the overlap in a life's first 20 s; in 55 % of it the bot behind walks on into a teammate ahead of it, and the side step is refused in 57 % (no clear ground that side;
    /// `traces-20261028/fx-zones-a212.md`). With a queue a bot with a teammate just ahead on its way gives way (`bots/apart.rs` [`apart::queue`]): it drops its sprint inside
    /// `gap_m` and lets go of forward inside `stop_m` (Reynolds' "queuing at a doorway"). Off: nothing keeps a gap along the path.
    pub queue: Option<QueueTuning>,
}

/// The queue's settings (A212): how near a teammate ahead is given way to.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct QueueTuning {
    /// A teammate this near ahead on the bot's way, or nearer, is given way to by dropping the sprint, m (live players walking the same way keep 1 m or more across or
    /// 2 m or more along in 92 % of the pairs within 16 m, `A175/human_file.txt`).
    pub gap_m: Metres,
    /// … and this near, by letting go of forward for the tick, m (the step apart's reach, `apart_m`).
    pub stop_m: Metres,
    /// Ahead: within this of the bot's way, degrees.
    pub cone_deg: Positive,
}

impl Default for QueueTuning {
    fn default() -> Self {
        Self { gap_m: Metres::of(2.0), stop_m: Metres::of(1.1), cone_deg: Positive::of(60.0) }
    }
}

/// The lane's settings (A175): when a teammate is in file and how far the bot moves over.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LaneTuning {
    /// How far to the side the steering point moves, m (a body is 0.8 m wide; the live players' lateral offset to a teammate walking their way is 1 m or more in 78 % of the pairs).
    pub offset_m: Metres,
    /// A teammate walking the same way within this far along the bot's heading, ahead or behind, is in file, m.
    pub look_m: Metres,
    /// … and within this across it, m (a teammate farther to the side is already on a line of his own).
    pub line_m: Metres,
    /// The lane is kept this long after the teammate is gone, ms (so a pair does not weave).
    pub hold_ms: u32,
}

impl Default for LaneTuning {
    fn default() -> Self {
        Self { offset_m: Metres::of(0.9), look_m: Metres::of(8.0), line_m: Metres::of(0.7), hold_ms: 2_000 }
    }
}

impl Default for SpacingTuning {
    fn default() -> Self {
        Self {
            buddy: true,
            callouts: Callouts::SeenAndHeard,
            apart: true,
            apart_m: Metres::of(1.1),
            apart_follow: true,
            apart_ease: false,
            apart_tie: false,
            lane: None,
            queue: None,
        }
    }
}

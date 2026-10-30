//! Sprint on the calm stretches (N1, the owner's one sprint rule: "sprint on calm stretches"). Bots walked 83 % of the
//! ticks of calm walking (a sprint on 17-25 % of them against the live players' 59-60 %) because the plays' pace rule
//! (`Ctx::pace_toward`) walked whenever a track or a death spot was within 30 m of the bot or of the point it was
//! handed. With `calm_sprint` the plays hand `Pace::Cruise` and the bot's own gate (`bots/pace.rs`) decides it, along
//! the path it is about to walk, with a hysteresis on the ring and a bar reserve for the sprint (`reserve`).

use super::*;

/// The bar a sprint is given up at and taken up again over, drawn for each life: no two bots breathe together, and one
/// bot's cycle is not one number.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reserve {
    /// The sprint is let go of at a bar this low (units of 100), drawn from this span.
    pub stop: Span,
    /// It is taken up again over the stop plus this much, drawn from this span.
    pub gap: Span,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PaceTuning {
    /// The plays hand `Pace::Cruise` and the gate (`bots/pace.rs`) walks it for a known danger ahead on the path. Off: the plays'
    /// own rule (a track or a death spot within the ring of the bot or of the play's goal).
    pub calm_sprint: bool,
    /// A known danger within this of the bot or of a point of the path ahead walks it, metres.
    pub ring_m: Metres,
    /// The walk entered at the ring is left only when nothing is within this, metres: a track that moves a metre does not
    /// toggle the sprint key every think.
    pub leave_m: Metres,
    /// How far ahead along the path the ring is looked at, seconds of a sprint at the kinematics' sprint speed.
    pub look_s: Positive,
    /// A walk entered at the ring lasts at least this long, ms (two thinks).
    pub hold_ms: u32,
    /// A play that goes to a destination (`Pace::Arrive`) walks the last `goal_walk_m` metres to it when a known danger is within the ring of the destination (the plays' own
    /// rule's goal clause, kept for the last stretch only). Off: the ring of the feet and of the path ahead only.
    pub goal_clause: bool,
    /// ... this many metres, from the destination.
    pub goal_walk_m: Metres,
    /// A track older than this does not walk the bot, ms (the plays' rule read tracks of any age up to the memory's `sense.drop_ms`: 20 s). A
    /// human who last saw an enemy 10 s ago does not walk for him.
    pub track_age_ms: u32,
    /// The radar cheat's tracks (`CheatTuning`, owner-approved) count for the ring as they did for the plays' rule.
    pub ring_radar: bool,
    /// The team's death spots (a teammate's last place on the minimap) count for the ring.
    pub ring_death_spots: bool,
    /// Plays that keep the plays' own pace rule (the ring of the feet and of the point handed) with `calm_sprint` on. Default: Collect. Its approach to a battery is
    /// walked whenever a known danger is near the battery, and sprinting it (the gate sees 12 m ahead) cost the batteries modes 5-7 % of the score and 8-10 % of the
    /// deliveries in the head-to-head (N1, `bench` arms A and B/C: the play alone carries the swing). Empty: every play that may run hands `Cruise`.
    pub old_rule_plays: Vec<PlayKind>,
    /// The bar reserve: a sprint given up and taken up again at a bar drawn for each life. None: the lockout's own 3 and 20.
    pub reserve: Option<Reserve>,
}

impl Default for PaceTuning {
    fn default() -> Self {
        Self {
            calm_sprint: true,
            ring_m: Metres::of(30.0),
            leave_m: Metres::of(34.0),
            look_s: Positive::of(2.4),
            hold_ms: 600,
            track_age_ms: 20_000,
            goal_clause: false,
            goal_walk_m: Metres::of(30.0),
            ring_radar: true,
            ring_death_spots: true,
            old_rule_plays: vec![PlayKind::Collect],
            reserve: Some(Reserve { stop: Span::of(10.0, 25.0), gap: Span::of(15.0, 30.0) }),
        }
    }
}

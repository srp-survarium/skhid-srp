use super::*;

/// The team modifiers (`signals::Mods`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ModTuning {
    /// Aggression from the score gap (behind is high) and the match's
    /// progress.
    pub aggression_gap: Consideration,
    pub aggression_time: Consideration,
    /// Risk from the share of the team alive against the enemy's.
    pub risk_alive: Consideration,
    /// Objective urgency from the time left.
    pub urgency_time: Consideration,
    /// How far a modifier moves toward its target each commander tick.
    pub smoothing: f32,
    pub min: f32,
    pub max: f32,
}

impl Default for ModTuning {
    fn default() -> Self {
        use super::super::curve::{Curve, Input};
        Self {
            aggression_gap: Consideration::new(
                Input::ScoreGap,
                -3.0,
                3.0,
                Curve::Logistic { k: 8.0, mid: 0.4 },
            )
            .inverted(),
            aggression_time: Consideration::new(
                Input::TimeLeft,
                0.0,
                900.0,
                Curve::Poly { k: 0.5 },
            )
            .inverted(),
            risk_alive: Consideration::new(Input::EnemiesDead, 0.0, 1.0, Curve::Linear),
            urgency_time: Consideration::new(Input::TimeLeft, 0.0, 600.0, Curve::Poly { k: 2.0 })
                .inverted(),
            smoothing: 0.2,
            min: 0.1,
            max: 1.0,
        }
    }
}

/// #25b, the owner's remembered sniper (2026-10-30 quiz: "Yes, 2 minutes"): a sniper the team senses (seen, heard or told)
/// is a threat spot with lanes all round him (`threat::ThreatSpots::sniper_sensed`), held `memory_s` after the last sense
/// by the spots' decay, at the place he was last sensed (never his live one; `threat::area_m` says how far he may be from
/// it by then), let go at his death or a teammate's clear look. The routes keep out of his lanes like any held spot's, and
/// the objective plays (Collect, Escort, Intercept, not a carrier going home) weigh a way through them: a cost, never a
/// block.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SniperMemory {
    /// How long a sensed sniper's spot is held after the last sense, s: the human snipers' area stays as dangerous for
    /// 120 s and more after a kill (kill share per crossing within 20 m of an earlier kill spot 7-10 % in every bin up to
    /// 120 s, `human25/reveal_decay.txt`, 145 live recordings of 12+ players). LIKELY.
    pub memory_s: f32,
    /// An objective play's score falls by this share for each 10 m of the straight way to its target that lies in the
    /// sniper spots' lanes. GUESS.
    pub way_cut_per_10m: Share,
    /// ... but to no less than this share of it (a cost, never a block: humans do not avoid a revealed sniper's lane,
    /// `human25`). GUESS.
    pub way_floor: Share,
    /// The routes (the detours, A3's replans) keep out of the sensed snipers' lanes, as of a held spot of kills
    /// (`threat::Covered`). Off: they keep out of kill spots alone.
    pub routes: bool,
    /// K1: a walking bot in a sensed sniper's lanes runs (`threat.sprint`), as in a kill spot's. Off: kill spots alone.
    pub sprint: bool,
    /// A smoke across a sensed sniper's lanes (`smoke.cross_lane`), as across a kill spot's. Off: kill spots alone.
    pub smoke: bool,
}

impl Default for SniperMemory {
    fn default() -> Self {
        Self { memory_s: 120.0, way_cut_per_10m: Share::of(0.1), way_floor: Share::of(0.5), routes: true, sprint: true, smoke: true }
    }
}

impl SniperMemory {
    /// An objective play's share of its score for `exposed_m` of its way in the lanes.
    pub fn way_weight(&self, exposed_m: f32) -> f32 {
        (1.0 - self.way_cut_per_10m.get() * exposed_m.max(0.0) / 10.0).max(self.way_floor.get())
    }
}

/// design-p1c §3 "no feeding" (`concede.rs`): the team's tries on a shooter's area are remembered per area, and after
/// `fails` failed ones the area is conceded for `concede_s` (no job or Breach on it, the plays aimed into it refused or,
/// the objective's, weighed down; the routes keep their cost). A retry must differ from the last try by kind, by
/// `retry_deg` of approach, or by `retry_s`. The numbers are from the live recordings (`fx-threat-obj-human-note.md`):
/// a human team's next man into a camper's lane comes a median 12 s after a death (p75 32 s), its next death to him a
/// median 29 s after it; the old bots went in after 4 s and died again after 5.5 s.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Concede {
    /// Counted failures (a role holder down, a death at the spot nobody traded) that concede the area. design-p1c §3.
    pub fails: u8,
    /// How long a conceded area stays conceded, s: past the p90 of a camper's stay after a kill against the old bots
    /// (51-62 s); 86 % of humans' re-entries come by 60 s. LIKELY.
    pub concede_s: f32,
    /// A retry this many degrees or more off the last try's approach differs from it: out of the old +-20 deg lane. GUESS.
    pub retry_deg: f32,
    /// A retry this long after the last try differs from it, s: humans' re-entry p75 32 s, next death median 29 s. LIKELY.
    pub retry_s: f32,
    /// An objective play aimed into a conceded area: its score times this, so another objective goes first; never 0
    /// (design-p1c item 13: a conceded spot over the only battery does not stall the objective). GUESS.
    pub objective_weight: f32,
    /// [item 15] A lone bot hit from a held spot's lane makes no second peek into it for a while ([`LonePeek`]). Off by its
    /// by-side bench (battery camper, 128 pairs, concede on both teams: second-peek deaths 32 vs 30, z +0.82; camper kills
    /// 373 vs 367): it refused 1,982 candidate plays against the like-last rule's 756,335 ahead of it in the same check,
    /// since a death sets its lane's last try for `retry_s` and a hit then a peek mostly fall in a lane already tried.
    pub lone_peek: Option<LonePeek>,
    /// A shooter's death keeps the tries on his areas for a while ([`KeepAfterDeath`]); none: his death opens them.
    pub keep_after_death: Option<KeepAfterDeath>,
}

impl Default for Concede {
    fn default() -> Self {
        Self {
            fails: 2,
            concede_s: 60.0,
            retry_deg: 45.0,
            retry_s: 30.0,
            objective_weight: 0.5,
            lone_peek: None,
            keep_after_death: Some(KeepAfterDeath::default()),
        }
    }
}

/// No feeding, look 2 (`concede::SpotState::Down`): a camper killed at his spot comes back to it. Live, within 180 s of
/// his death he is back within 10 m in 69 % of the PvB runs (median 41 s, p75 66 s; humans' matches 59 %, 48 s, p75 89 s)
/// and kills from it again in 53 % (`traces-20261028/fx-threat-obj-same-spot-note.md`). His death keeps the team's tries
/// on his areas for `keep_s`: anything is offered while he is down, and they are taken up when the team senses him back in
/// one or a try fails on it again.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeepAfterDeath {
    /// How long, s: Valve's nav-area danger, "one kill == 1.0, which we will forget about in two minutes"; covers the p75
    /// of his return. LIKELY.
    pub keep_s: f32,
}

impl Default for KeepAfterDeath {
    fn default() -> Self {
        Self { keep_s: 120.0 }
    }
}

impl KeepAfterDeath {
    /// [`Self::keep_s`] in ms.
    pub fn keep_ms(&self) -> u32 {
        (self.keep_s.max(0.0) * 1000.0) as u32
    }
}

/// design-p1c item 15: a bot hit by a shooter it sensed, his spot a held line, is refused (or, objective, weighed) a play back
/// into that lane from the bearing the hit came from for `wait_s`, while no mate stands within `mate_m` of it (with a mate
/// it goes: two peeking together, not one after another). The second peek on the bench: 16 of 144 kills of the static
/// stand-in, 6 of 92 of the one by a battery, a median 11 s after the first hit (`fx-threat-obj-nofeed-bench.md`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LonePeek {
    /// How long after the hit, s: a human team's median re-entry into a camper's lane (`fx-threat-obj-human-note.md`).
    /// LIKELY.
    pub wait_s: f32,
    /// A mate this near makes the bot not alone, m: a pair that can trade (design-p1c §1, 3-5 m apart when they go). GUESS.
    pub mate_m: f32,
}

impl Default for LonePeek {
    fn default() -> Self {
        Self { wait_s: 12.0, mate_m: 10.0 }
    }
}

impl LonePeek {
    /// [`Self::wait_s`] in ms.
    pub fn wait_ms(&self) -> u32 {
        (self.wait_s.max(0.0) * 1000.0) as u32
    }
}

impl Concede {
    /// The rules, parsed once (`concede::ConcedeRules`).
    pub fn rules(&self) -> crate::v026g::bots::concede::ConcedeRules {
        crate::v026g::bots::concede::ConcedeRules {
            fails: self.fails.max(1),
            concede_ms: (self.concede_s.max(0.0) * 1000.0) as u32,
            retry_deg: self.retry_deg.clamp(0.0, 180.0),
            retry_ms: (self.retry_s.max(0.0) * 1000.0) as u32,
            keep_ms: self.keep_after_death.map(|k| k.keep_ms()),
        }
    }

    /// [`Self::objective_weight`] kept in (0, 1]: an objective play is weighed, never refused.
    pub fn objective_weight(&self) -> f32 {
        self.objective_weight.clamp(0.01, 1.0)
    }
}

/// design-p1c §1, items 13, 14, 16, 18 (`breach.rs`): when a held spot's Flank failed or found no flank spot, two crossers
/// gather out of his lanes and out of his sight, go on one tick to two cover points that see his spot from two bearings out
/// of his lanes, and the second trades the first; a pinner holds him meanwhile when one is free. The numbers are the live
/// humans' (`fx-gear-breach-human-note.md`: 478 campers ended by the victims' team in 402 recordings of 12+ players; counts
/// PROVEN, timings LIKELY to the worlds' 5 s).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Breach {
    /// The crossers wait for each other this long at most after the longer walk to the gather points at the walk cap, s:
    /// the camper's median life after his last kill (8 s); a group not there by then fails (`GatherTimedOut`), it does not
    /// trickle.
    pub gather_s: f32,
    /// A crosser walks this far to its gather point at most, m; a farther mate is no crosser. Humans' comers stood within
    /// 35 m of his spot at his death and made their last 8 m inside the 30 s before (the note's comer rule); the first
    /// bench's crossers walked 13-127 m (median ~55) and half the Breaches lost one on the way. GUESS from that window.
    pub walk_max_m: f32,
    /// The walk from the gather points to the cover points, s at most (half of the humans' 2+ comers go within 5 s of
    /// each other).
    pub go_s: f32,
    /// At the cover points the job holds this long for him, s, then ends (`Time`). GUESS.
    pub hold_s: f32,
    /// A crosser's death is traded when he dies within this, s (humans' median 4 s, the stat tools' 5 s window).
    pub trade_s: f32,
    /// The two cover points lie this far apart, m at least and at most (humans' nearest pair at his death: P25 7 m,
    /// median 12 m).
    pub apart_min_m: f32,
    pub apart_max_m: f32,
    /// Seen from his spot, the cover points' bearings differ by this at least, degrees (84 % of humans' pushes came from
    /// two or more 45-degree sectors, median spread 68 degrees).
    pub bearing_min_deg: f32,
    /// The cover points lie this far from his spot, m (the killer of a camper stood P25 9.6 / median 18.7 m off).
    pub cover_min_m: f32,
    pub cover_max_m: f32,
    /// A gather point lies this far from its cover point, m, out of his lanes and out of his sight. GUESS.
    pub gather_min_m: f32,
    pub gather_max_m: f32,
}

impl Default for Breach {
    fn default() -> Self {
        Self {
            gather_s: 8.0,
            walk_max_m: 40.0,
            go_s: 6.0,
            hold_s: 10.0,
            trade_s: 5.0,
            apart_min_m: 6.0,
            apart_max_m: 12.0,
            bearing_min_deg: 30.0,
            cover_min_m: 8.0,
            cover_max_m: 22.0,
            gather_min_m: 5.0,
            gather_max_m: 15.0,
        }
    }
}

impl Breach {
    /// The rules, parsed once (`breach::BreachRules`).
    pub fn rules(&self) -> crate::v026g::bots::breach::BreachRules {
        let ms = |s: f32| (s.max(0.0) * 1000.0) as u32;
        let band = |lo: f32, hi: f32| (lo.max(0.0), hi.max(lo.max(0.0)));
        crate::v026g::bots::breach::BreachRules {
            gather_ms: ms(self.gather_s),
            walk_max_m: self.walk_max_m.max(0.0),
            go_ms: ms(self.go_s),
            hold_ms: ms(self.hold_s),
            trade_ms: ms(self.trade_s),
            apart_m: band(self.apart_min_m, self.apart_max_m),
            bearing_min_deg: self.bearing_min_deg.clamp(0.0, 180.0),
            cover_m: band(self.cover_min_m, self.cover_max_m),
            gather_m: band(self.gather_min_m, self.gather_max_m),
        }
    }
}

/// The gang-up cap (PR C item 20, `team::gang_factor`): a play aimed at an enemy that teammates' plays are already
/// aimed at is scored lower when the commander hands the plays out, so the third bot takes its next best. The trade
/// partner never counts and the first `free` mates are free (a pair on one enemy stays); the next ones pile.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GangUp {
    /// This many teammates (the partner not counted) aimed at an enemy are free. GUESS.
    pub free: u32,
    /// The play's score falls by this share for each teammate past `free`. GUESS.
    pub step: Share,
    /// ... but to no less than this share of its score. GUESS.
    pub floor: Share,
}

impl Default for GangUp {
    fn default() -> Self {
        Self { free: 1, step: Share::of(0.5), floor: Share::of(0.25) }
    }
}

/// [`GangUp`] parsed once for the commander.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GangRule {
    pub free: u32,
    pub step: f32,
    pub floor: f32,
}

/// A job's pin from cover inside the held spot's own lanes (Breach part 1, `plays/crossfire.rs`): with no pin spot out
/// of the lanes, a peek spot in his own lanes (hidden from his eye crouched, seeing him standing), held crouched with a
/// few short peeks at his spot.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PinFromCover {
    /// The peek spots' ring is sampled this many times as densely as the play's own: a peek spot is a narrow lip, and
    /// the play's samples found one in 13 % of the polls, 4x in 36 % (`breach-plan.md`). PROVEN by the read.
    pub samples_x: u32,
    /// A peek stands this long, s: humans holding a spot stand up from a crouch for a median 1.15 s, p75 3.2 s
    /// (`human-peeks.md`: 10 live matches of 6-10 players, 592 holds, PROVEN).
    pub peek_s: f32,
    /// Hidden between peeks from this to `hide_max_s`, s (the cycle then matches the humans' rate below).
    pub hide_min_s: f32,
    pub hide_max_s: f32,
    /// At most this many peeks a play, then it ends (no peek pattern to read). GUESS.
    pub peeks: u32,
    /// At most this many peeks a minute: a cycle (peek and hide) is at least 60 / this long. Humans holding a spot
    /// stand up 2.4 times a hold minute, 3.2 in holds they fired from (a hold's median 1.8, p75 5.7, p90 7.8; PROVEN,
    /// `human-peeks.md`): 5 is about their p75, never their top.
    pub per_minute: u32,
    /// A peek spot is offered only if its walk fits the job it serves: its way (`Ctx::path_dist`) at `walk_mps` takes
    /// at most this long, s. The jobs last a median 6.9 s (p75 13.3 s); with no cap 82 % of the walks needed longer
    /// (median 27.6 s for 55.5 m) and only 11 of 104 arrived (`breach-plan.md`, gate 2); capped, 8 of 24 (gate 3).
    /// PROVEN by the reads; 8 s GUESS.
    pub peek_reach_s: f32,
    /// The pinners' pace to a peek spot along their way, m/s: gate 3's capped walks went 1.1 m/s (14.1 m in a median
    /// 12.6 s; they sneak near the spot). PROVEN by the read.
    pub walk_mps: f32,
}

impl Default for PinFromCover {
    fn default() -> Self {
        Self { samples_x: 4, peek_s: 1.5, hide_min_s: 10.0, hide_max_s: 14.0, peeks: 4, per_minute: 5, peek_reach_s: 8.0, walk_mps: 1.1 }
    }
}

/// [`PinFromCover`] parsed once for the play: the times in ms, the hide stretched so that no cycle beats the cap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PinRule {
    pub samples_x: u32,
    pub peek_ms: u32,
    pub hide_ms: (u32, u32),
    pub peeks: u32,
    /// The longest way to a peek spot, m (`peek_reach_s` at `walk_mps`).
    pub reach_m: f32,
}

impl PinFromCover {
    pub fn rule(&self) -> PinRule {
        let ms = |s: f32| (s.max(0.0) * 1000.0) as u32;
        let peek_ms = ms(self.peek_s).max(1);
        let cycle_ms = 60_000 / self.per_minute.max(1);
        let lo = ms(self.hide_min_s).max(cycle_ms.saturating_sub(peek_ms));
        PinRule {
            samples_x: self.samples_x.max(1),
            peek_ms,
            hide_ms: (lo, ms(self.hide_max_s).max(lo)),
            peeks: self.peeks.max(1),
            reach_m: self.peek_reach_s.max(0.0) * self.walk_mps.max(0.0),
        }
    }
}

/// The team's threat spots (K1, `threat.rs`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThreatTuning {
    /// The team remembers where a shooter kills from and its routes keep
    /// out of his line (a soft preference); off, none of it runs.
    pub spots: bool,
    /// A bot walking to a goal in a held shooter's lane runs: it crosses
    /// fast when there is no good way round, and keeps its task.
    pub sprint: bool,
    /// A route through a lane gets a local detour round it where there is a
    /// short one (the search is capped, `search_floors`).
    pub detour: bool,
    /// Floors of a poll's search budget one search round the covered
    /// regions may take (the poll's is `ThinkTuning::search_floors_per_poll`).
    pub search_floors: u32,
    /// A2 (`an-anticamper.md` §3b): the way round a lane is planned on the
    /// whole route, the regions' search costing a covered region's walk
    /// `lane_cost` times more, and the floors searched inside that corridor
    /// (off: a local detour round the first lane with the lane shut, which
    /// ran out of floors in 79-81 % of exposed routes in the bench).
    pub whole_route: bool,
    /// What a metre walked in a covered region costs more than a plain one
    /// (the whole-route search): 3 makes a lane metre cost 4. GUESS, the order
    /// of Killzone's links under fire and Valve's danger cost; the length cap
    /// (`stuck::worth_round`) bounds what any detour may add.
    pub lane_cost: f32,
    /// Floors the whole-route search round the lanes may expand, its own
    /// (not what the poll's other searches left; taken off the poll's after).
    /// The local detour's `search_floors` (3,000) was sized for a stretch of
    /// 30-40 m; the whole route is 100-200 m on a 0.25 m grid. Default a
    /// poll's budget (`SEARCH_FLOORS_PER_POLL`): it runs only on an exposed
    /// route, 1-5 % of the routes planned; the bench's poll tail decides.
    pub whole_route_floors: u32,
    /// A2's fine pass: inside the costed corridor's room the floors are
    /// searched with the same cost (a covered floor's link costs
    /// `1 + lane_cost` times its length, the route's ends free; Detour's area
    /// costs), not with the covered floors shut (off: shut, the room's
    /// uncovered regions only, `RouteFilter::keeping_to`).
    pub lane_weighted: bool,
    /// A3: a route walked is planned again when the covered regions change
    /// and more than `threat::EXPOSED_M` of its way ahead is in them (off: a
    /// route kept to its end whatever was held after it was planned).
    /// OFF by default: its head-to-head (96 pairs, p1m 2026-10-29) cost
    /// +17.5 % floors a match-second for the team (z +6.9) and the package
    /// with it scored -2.0 % (z -1.4), while the package without it scored
    /// +0.7 % (z +0.5) at the floors of OFF: CPU for no score.
    pub replan: bool,
    /// A4: a held spot casts its lanes again when a kill comes from a
    /// bearing outside every lane cast (or the reach grows, or its centre
    /// moves), keeping the old cover until the new cast ends (off: cast
    /// again only when the reach grows, covering nothing meanwhile).
    pub recast: bool,
    /// §8.4: a spot is the shooter's area, kills within `threat::AREA_M`
    /// merged and its centre their middle, and he is on it while sensed in
    /// it (off: the first kill's point and `threat::MERGE_M`).
    pub area: bool,
    /// K2: a held spot starts a team job: a flanker (Flank from the side
    /// off his death lanes) and a pinner (Crossfire at the edge of his
    /// reach, out of his lanes); the crouch-and-aim gate stays a second
    /// trigger of Flank.
    pub job: bool,
    /// The job's plays compete in the objective band (off: the fight band,
    /// where any objective cuts them, RC3).
    pub job_objective: bool,
    /// A job play's score: the team gave the role, so it is scored by the
    /// role (this, times the play's health gate), not by the bot's
    /// personality, push or cooldown, which kept 80 of 129 roles from being
    /// taken (p1-bench, 2026-10-29). GUESS: 0.7, about a battery take next
    /// to the bot (Collect 0.54-0.66 by Vector Laboratory's battery 0).
    pub job_score: f32,
    /// A job opens only on a spot with this many kills or more (K2): a hold.
    /// One kill and a sighting already hold a spot for the routes (K1), and
    /// a job on each of those sent bots to every fight: a job began a median
    /// 0.6 s after its spot was made and ended 10 s later, unheld (1,273 of
    /// 1,457 jobs, 30 a match), with flips +38 %, U-turns +22 % and floors
    /// +27 % (p1-bench 5504b7a4). PROVEN: a hold is a run of 2+ kills
    /// within 10 m in one life (`an-anticamper.md` §8.4, 8,474 live holds).
    pub job_kills: u32,
    /// K1: a held spot's lanes are cast by the team's bullets (#705's cover
    /// rule, `threat::FireEyes`): a crate, a car or a thin wall the bullets
    /// pierce does not hide a lane (off: by sight, as it was).
    pub lanes_pierce: bool,
    /// K3: Trade does not go to a partner's death in a held spot's lanes.
    pub trade_out_of_lines: bool,
    /// K3: PairUp does not join a teammate standing in a held spot's lanes,
    /// and its join and wait spots are out of the top player's sight and the
    /// held lanes (F3).
    pub pair_up_hidden: bool,
    /// K3's other half (`an-anticamper.md` §4.1): a Crossfire spot is never
    /// in a held spot's lanes.
    pub crossfire_out_of_lines: bool,
    /// K1: the floors the whole-route search spends past its grant
    /// (`whole_route_floors` is its own) are booked to the poll, so the
    /// searches after it wait and the poll's cap holds (off: taken off the
    /// grant only, the rest unbooked; verify-703 should-fix 1).
    pub whole_route_booked: bool,
    /// K2: a job opens only when a role of it can be given at once, so a
    /// spot no free bot can take keeps no slot nor its shooter's one job
    /// (off: opened, then dropped at the tick's end, holding the slot that
    /// tick, every tick; verify-703 should-fix 2).
    pub job_open_staffed: bool,
    /// K2: a role goes to a bot out of every held line (off: out of the
    /// holds' only, `job_kills`; verify-703 should-fix 3). OFF by default:
    /// alone against the four follow-ups off (96 pairs by side, 2026-10-29)
    /// the team scored -1.4 % (z -1.4; -2.7 %, z -2.2 at 48) while its own
    /// jobs did better (their shooters killed 247 vs 226, holders down 240
    /// vs 273): not settled, cost seen, and no cause in the event files.
    pub job_out_of_all_lines: bool,
    /// K2: a Crossfire finds its job's held line by the spot's id, its
    /// `TargetRef::ThreatSpot` (off: by the choke's point, so a plain
    /// Crossfire on an enemy's last place that is a held spot's centre was
    /// taken for the job's pin; verify-703 should-fix 5).
    pub job_line_by_id: bool,
    /// Breach slice 0 cause 1: a job's flank and pin spot rings also take the floors of the storeys under and over
    /// his height, within [`crate::v026g::bots::threat::JOB_LEVELS_M`] (the flank ring took only his own height, the
    /// pin's only upward): a camper on a roof, an upper storey or a slope left the ring with no floor (26 % of each
    /// role's unplayed roles). It fires as designed (js-mech-41ee0edb, v5: its class 150 -> 31 flanker, 148 -> 33
    /// pinner unplayed roles, the roles unplayed -19 % / -17 %; bots' CPU 57.8 vs 58.3 ms/s). Off by default: on the
    /// tip e9d6ad90 its own arm loses (js3-h2h-058fd2c7, 96 pairs: score -2.5 %, z -2.7; deaths +2.5 %, z +2.5;
    /// 812a85d9 had it flat, -0.1 %). Named cause (event files): the roles it gets played die, and not to the camper:
    /// deaths in job Flank +19 and job Crossfire +17, holders down to another enemy +16 (to the shooter 29 vs 31), the
    /// Trades after them +32. A spot on another storey is a longer way across the map to it (LIKELY).
    pub job_spot_levels: bool,
    /// PR C item 20: the gang-up cap on the plays aimed at one enemy (the commander's assignment). `None` (off): the
    /// plays are handed out as before. On by default.
    pub gang_up: Option<GangUp>,
    /// Breach part 1 (`breach-plan.md`): a job's pinner with no pin spot out of the lanes pins from cover inside his
    /// own lanes ([`PinFromCover`]): 36 % of the pinners' unplayed roles had every pin spot in a held line, 91 % of
    /// those in his own lanes. `None` (off): that role goes unplayed as before.
    ///
    /// OFF by default: the play needs a job that outlives the walk to its spot, and in the bench the jobs end in the
    /// fight itself, a median 6.9-9.0 s (gates 2 and 3, 96 matches each: its shooter died 32 %, its Flanker went down
    /// 24-27 %, its Pinner 13-16 %, the spot cleared 13 %, ran its time 0.3-0.6 %). The walks went 1.1 m/s: with no cap
    /// 11 of 104 arrived; capped to a walk that fits the job, 8 of 24 (0.08 a match), and 0 kills or deaths at a peek
    /// in 96 matches. A longer job would not help (they end in the fight, not on a timer); nearer spots cannot be
    /// reached in time. Kept for a mode or an enemy whose spots outlive a walk (a sniper held for minutes).
    pub pin_from_cover: Option<PinFromCover>,
    /// Play 28 (the owner's X3 part 2, 2026-10-26): a free teammate whose weapon reaches a sensed enemy sniper (a rifle, a
    /// sniper rifle) takes a spot that sees his last sensed place within its own reach, from a way hidden from him and out of
    /// his lanes, and answers him from there. Off: no bot is offered the play. OFF by default, not settled (96 pairs by side,
    /// 2026-10-30): score +1.5 % (z +0.9) on seeds 1-4, -1.5 % (z -1.0) on seeds 5-8; it fires (4.6 starts a match, 0.2 sniper
    /// kills by answerers) but the snipers killed and the deaths to them do not move, and it scores plays +6 %. A210
    /// (2026-11-01): its runs lasted 5 s (a quarter taken mid-walk by a play of its band, a third lapsing with the bot's own
    /// 10 s sense of him); now its walk is a hold walking to its spot and it answers the sniper the team remembers.
    pub answer_sniper: bool,
    /// X3 (the owner, 2026-10-26: "keep to the task by a route out of the sniper's sight"): a sniper the team senses is a
    /// threat spot at once, with lanes all round him (the ground he sees within his weapon's reach), so the routes of the team
    /// keep out of his sight as they keep out of a shooter who has killed (`ThreatTuning::sprint` crosses where they cannot).
    /// Off: a sniper is kept out of only after he has killed. OFF by default, not settled (96 pairs by side, 2026-10-30):
    /// score +1.7 % (z +1.1) on seeds 1-4, -3.0 % (z -1.8) on seeds 5-8, deaths to snipers within 10 s of a sighting -16 % then
    /// +10 %; it costs +9 % of the rays cast a second. A210 (2026-11-01): with `sniper_memory` on (the default) the sensed
    /// snipers are spots whatever this says (its one read, `commander.rs`: `sniper_lanes || sniper_memory.is_some()`), so it
    /// acts only with the memory off; it stays OFF.
    pub sniper_lanes: bool,
    /// #25b: a sensed sniper's spot remembered for about two minutes as his area ([`SniperMemory`]); it makes the spots
    /// whether or not `sniper_lanes` is on. Off: `sniper_lanes`' spots as #836 made them (held 15 s after a sense), and no
    /// objective play weighs them. ON by default (the coordinator's rule, 2026-10-31: TDM's deaths to snipers settled, no
    /// cost in batteries or by the stand-in sniper; by side, memory with all its consumers vs off): TDM deaths to snipers
    /// -6 % (120 pairs, z -2.4; score +3.0 % z +2.0 on the 84 fresh), batteries score +0.2 % (120 pairs, z +0.2), the
    /// stand-in cell -0.4 % (384 pairs over both camper placements and two seed sets). An earlier -1.6 % on one seed set
    /// did not replicate. Its detours (+46-59 %) pass known enemies no more than the ways they replace, and die no more.
    pub sniper_memory: Option<SniperMemory>,
    /// design-p1c §3 "no feeding" and items 11, 15 ([`Concede`], `concede.rs`): a team's failed tries on a shooter's area
    /// counted per area; after enough, the area is conceded for a while; a retry must differ from the last. OFF by
    /// default: not built into the jobs yet (the memory and its rules only).
    pub concede: Option<Concede>,
    /// design-p1c §1, items 13, 14, 16, 18 ([`Breach`], `breach.rs`): the group push on a held spot whose Flank failed or
    /// found no flank spot. OFF by default until its by-side bench.
    pub breach: Option<Breach>,
    /// design-p1c §6, two campers covering one lane ([`PairTarget`]): of two holds whose lanes cover common ground, a job
    /// opens only on the one with more kills. `None` (off): each hold gets its own job, up to `threat::JOBS_MAX`. OFF by
    /// default: the bench has one stand-in camper, so no by-side bench of a pair yet.
    pub pair_target: Option<PairTarget>,
    /// A229 ([`WalkWatch`], `plays/watch.rs`): a play's walk with a known threat ahead looks at it (the spot of a held lane
    /// the bot stands in; with `known`, an enemy's last known place) instead of along the route. OFF by default until its
    /// by-side bench.
    pub walk_watch: Option<WalkWatch>,
}

/// A229 (`traces-20261028/fx-threat-obj-a229-a231-note.md`): a walk looks along its route, so a bot walks into a lane its
/// team knows with the view where the route goes (bench, tip c31448aa: in a held lane of the team the view was on its spot
/// 27 % of the walking ticks in batteries, 33 % in TDM, the spot behind 25 % / 15 %). A human clears the known angle as he
/// walks toward it; Counter-Strike's bot looks at its path's encounter spots.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WalkWatch {
    /// A threat this far off the walk's heading or less is ahead, degrees: a lane walked across, not toward its spot,
    /// keeps the route's view and the sprint. GUESS.
    pub ahead_deg: f32,
    /// Also an enemy's last known place ([`WatchKnown`]); `None`: only the held lanes' spots.
    pub known: Option<WatchKnown>,
}

impl Default for WalkWatch {
    fn default() -> Self {
        Self { ahead_deg: 60.0, known: Some(WatchKnown::default()) }
    }
}

/// [`WalkWatch::known`]: the last place the bot's or its team's own senses gave an enemy (`Track::own`), ahead and near.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WatchKnown {
    /// A place given within this is known, ms (as the detour count's `KNOWN_MS`). GUESS.
    pub ms: u32,
    /// At most this far, m: a rifle's fight range (A220: kills at about 15 m, A221 bands to 65 m). GUESS.
    pub max_m: f32,
}

impl Default for WatchKnown {
    fn default() -> Self {
        Self { ms: 10_000, max_m: 60.0 }
    }
}

/// design-p1c §6 (`traces-20261028/fx-threat-obj-two-campers-note.md`): a team that beats two campers covering one lane takes
/// them one at a time (live, 373 pairs: the first falls a median 33 s after the pair's first kill, the second 43 s after him),
/// and the one taken first is the one who had killed more when the pair formed (161 of 255, 63 %; the nearer 55 %, the one
/// who began first 55 %). So of two holds whose lanes cover common ground, the job (K2) opens on the one with more kills
/// (ties: sensed latest, then the lower id); the other has none while both hold, unless one runs on it already. Their lanes
/// stay costed (K1), and a role's spot stays out of every held line, so nobody crosses ground both see.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PairTarget {
    /// The lanes are walked in steps this long for a point the other covers, m. GUESS.
    pub step_m: f32,
}

impl Default for PairTarget {
    fn default() -> Self {
        Self { step_m: 5.0 }
    }
}

/// How a bot commits to a play it has started (the commander's keep rule,
/// `team.rs`). Every switch is off until a bench turns it on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CommitTuning {
    /// A hold (Crossfire, HoldExit, Cut) still walking to its spot is kept
    /// against any play of its own band, whatever the score; a higher band
    /// (an objective, an alarm) still takes the bot, and so does the play's
    /// own end.
    pub hold_to_arrival: bool,
    /// A running Cut is scored without the crowd at its cut point
    /// (`Input::FriendsNearTarget` taken as 0): the crowd is a reason not to
    /// start a Cut where friends already are, not to leave one the bot is
    /// walking to.
    pub cut_given_crowd: bool,
    /// A Crossfire on an enemy's last known place keeps its choke where it
    /// was when it began, after the bot's memory has forgotten the enemy,
    /// for `crossfire_point_s` at most (the choke is a position; it ended
    /// with the memory's timer).
    pub crossfire_point: bool,
    /// How long that frozen choke stays valid from the play's start, s: the
    /// walk (about 15 s) plus how long players keep covering a spot after
    /// the enemy left their line (live 12+ matches, 144 of them: of the
    /// episodes that cover it at all, median 5 s, P75 9-10 s, P90 15-20 s;
    /// `push-record/scripts/human_cover_lastseen.py`).
    pub crossfire_point_s: f32,
    /// [A192] A Crossfire's other half is a teammate running Crossfire on
    /// the same choke (the commander's, `TeamKnowledge::crossfiring`), at
    /// the point it means to be at; and a running one checks it is apart
    /// only within `sneak_m` of its spot. Off: every teammate within
    /// `partner_m` of the choke, where it stands, every `repick_every_s`
    /// (the bench: 92 of 789 takes ended "no spot apart" on a mate passing by).
    /// And a new Crossfire on a known enemy's last place only when the team's
    /// own senses gave it within 15 s (`fairness::UNSEEN_MS`, the owner's
    /// rule): with spots no longer blocked by passers-by, more offers were
    /// scored on older places (the bench's stale reads +13-15 %, their share
    /// unchanged); a running one keeps its choke (`crossfire_point`).
    /// On with `held_in_pass` (`fresh-a192-20261031T0312`, by side, 96 pairs):
    /// Crossfire arrivals 385 vs 206, kills while holding 67 vs 33, "no spot
    /// apart" ends 9 vs 341, stale enemy reads -62 %, score +3.0 %, delivered
    /// +4.3 %; the drop-one arms put the gain on this switch.
    pub crossfire_apart_mates: bool,
    /// [A192] The commander's slots held by the bots kept on them are
    /// rebuilt as each bot of the pass is given up: a slot freed earlier in
    /// the same pass is free for the rest of it. Off: the held slots are
    /// found once before the pass (a bot that lost its Collect slot took a
    /// filler for one pass, then the Collect back: 35 flips within 3 s).
    /// On with `crossfire_apart_mates` (its bench): those flips 21 vs 38.
    pub held_in_pass: bool,
    /// [A192] A Crossfire offer's score is taken times the chance it is
    /// still wanted at arrival, `exp(-walk_s / tau)`, the walk at a run's
    /// pace to the choke (the bench: of the takes that ended before the
    /// spot, the walk's median time to the end was about 13 s). None: off,
    /// the default: with 13 s the drop-one arm without it held more (arrivals
    /// 409 vs 388, kills while holding 78 vs 56, takes a match 14.4 vs 12.7)
    /// for deaths before arrival a take 0.268 vs 0.285 (`drop-a192-20261031T0043`).
    pub crossfire_arrival_tau_s: Option<f32>,
}

impl ThreatTuning {
    /// The gang-up cap's rule, or none (off).
    pub fn gang_rule(&self) -> Option<GangRule> {
        self.gang_up.map(|g| GangRule { free: g.free, step: g.step.get(), floor: g.floor.get() })
    }
}

impl Default for CommitTuning {
    fn default() -> Self {
        Self {
            hold_to_arrival: true,
            cut_given_crowd: true,
            crossfire_point: true,
            crossfire_point_s: 30.0,
            crossfire_apart_mates: true,
            held_in_pass: true,
            crossfire_arrival_tau_s: None,
        }
    }
}

impl Default for ThreatTuning {
    fn default() -> Self {
        Self {
            spots: true,
            sprint: true,
            detour: true,
            search_floors: 3_000,
            whole_route: true,
            lane_cost: 3.0,
            whole_route_floors: SEARCH_FLOORS_PER_POLL,
            lane_weighted: true,
            replan: false,
            recast: true,
            area: true,
            job: true,
            job_objective: true,
            job_score: 0.7,
            job_kills: 2,
            lanes_pierce: true,
            trade_out_of_lines: true,
            pair_up_hidden: true,
            crossfire_out_of_lines: true,
            whole_route_booked: true,
            job_open_staffed: true,
            job_out_of_all_lines: false,
            job_line_by_id: true,
            job_spot_levels: false,
            gang_up: Some(GangUp::default()),
            pin_from_cover: None,
            answer_sniper: false,
            sniper_lanes: false,
            sniper_memory: Some(SniperMemory::default()),
            concede: None,
            breach: None,
            pair_target: None,
            walk_watch: None,
        }
    }
}

/// How a poll's route searches are shared out among the match's bots
/// (`search::Searching`): the searches and floors of a poll
/// (`SEARCHES_PER_POLL`, [`ThinkTuning::search_floors_per_poll`]) and the
/// searches left under way (the match's one, or one a team).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchQueue {
    /// The poll's searches stay the match's, the bots going in turn by team
    /// (the team that did not get the last search first, each team round
    /// robin from the bot after its last served one); the floors are each
    /// team's by its living bots, a done team's rest lent to the other; each
    /// team has its own search under way, which goes on again at the poll's
    /// end on what both left. One team's long searches cannot starve the
    /// other's bots, and no place comes first.
    #[default]
    Fair,
    /// As it was: one pool and one search under way for the match, by place
    /// order (the first places, team 0 on the bench and the first to join
    /// live, claim first each poll).
    FirstCome,
}

/// The humans' perches in the hold plays (V-S2b, `plays::goals::perch_first`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PerchTuning {
    /// A hold's spot pick may take a perch of the routes table; off, none.
    pub on: bool,
    /// The holds' share of a bot's time, GUESS from the live play mix
    /// (27-63 %): the humans' share of time on perches over this is the
    /// share of the holds' picks that take one.
    pub hold_share: f32,
    /// A multiplier on that share, for the bench to sweep (1: as designed).
    pub p_scale: f32,
    /// The most that share of picks may be.
    pub p_max: f32,
    /// A perch is taken at least this far from the anchor it holds, m, and at
    /// most this far.
    pub min_m: f32,
    pub max_m: f32,
}

impl Default for PerchTuning {
    fn default() -> Self {
        Self { on: true, hold_share: 0.4, p_scale: 1.0, p_max: 0.5, min_m: 10.0, max_m: 45.0 }
    }
}

/// [`ThinkTuning::search_floors_per_poll`] by default: a floor expanded
/// costs about 0.2 µs on a fast desktop core and 0.5 µs on the host's, so
/// a poll searches for at most about 2 ms here and 5 ms there. Measured on
/// six maps, ten seeds each (`docs/notes/player_bots_mapdata.md` §6): the
/// bots play as with no bound, and the worst poll fell from up to 81 ms to
/// 3-10 ms.
pub const SEARCH_FLOORS_PER_POLL: u32 = 10_000;

/// Stepping out of a live grenade (`grenade_threat.rs`, `grenades-record/avoid-design.md`).
///
/// Every number but the switch is a GUESS and a sweep arm: the live matches show no human reaction to a
/// landed grenade at all (mates and enemies leave the radius as often as they do 20 s earlier or later,
/// `docs/notes/grenade_visibility.md`), so no delay can be copied from them; the defaults are the
/// published reaction times (Rabin, Game AI Pro 2 ch. 5: 0.16-0.22 s simple, 0.38 s recognition).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AvoidTuning {
    /// A bot steps out of the blast of every grenade it knows of (below). Off: no bot knows of one, the
    /// list is not read.
    pub grenade_avoid: bool,
    /// The server's live list always reads a server bot's pockets from its profile, as a human's; this decides whether a team
    /// dodges a grenade the list holds from a bot's throw key with its pockets read and empty (the client throws nothing).
    /// Off: the team dodges it as a frag, so mates stepped out of blasts that never came (155 of 204 bot throws in the owner's
    /// matches of 2026-10-28 had empty pockets). A key of a bot whose pockets were not read (the bench's) may throw a frag:
    /// every team dodges it.
    pub bot_pockets: bool,
    /// The server's live list takes a player's pockets from the kit a world says he is in (`players::Supplies::kit`),
    /// not always the profile's first kit: a throw from kit 2 or 3 is then listed. Off: kit 1 whatever a world says,
    /// as before. Read where the profiles are read (`game/driven.rs`), once a match, and **not per team** on purpose: it
    /// decides what the server's one list of grenades holds for every throw of a player (which kit's pockets a human's
    /// key acts on), not what a team's bots do with the list, so two teams would see different lists for the same human
    /// throw, which has no meaning in the game. The match-wide tuning's value is the one used (a one-sided head-to-head
    /// bench leaves it the same on both sides; `bot_pockets` and `grenade_avoid` are the per-team switches).
    pub kit_in_use: bool,
    /// The thrower's teammates know the landing from the launch (the owner's rule, a justified cheat: a
    /// human team can call it, few do); they step out after this long, ms, drawn.
    pub told_ms: (u32, u32),
    /// The other team's bots know of a grenade once it lies on the ground and they hear or see it; they
    /// step out after this long, ms, drawn.
    pub notice_ms: (u32, u32),
    /// An enemy sees a grenade on the ground within this, metres, in its view cone, with a line to it.
    /// The client's draw distance for the 0.05 x 0.10 m model is not known.
    pub see_m: f32,
    /// The view cone of that, degrees (the whole angle).
    pub cone_deg: f32,
    /// An enemy within this of a landing hears the first bounce, metres (the live bounce sound is silent
    /// from 20 m; at 8 m it is 0.07 of full).
    pub hear_m: f32,
    /// A bot is out of the blast this far past its radius, metres.
    pub margin_m: f32,
    /// A bot whose walk would take it into the blast within this, ms, counts as in it.
    pub ahead_ms: u32,
    /// How often a bot that is not stepping looks again at the grenades, ms.
    pub look_ms: u32,
    /// A step out of a blast is walked as a person walks out of one (register #30): the floor is the nearest outside the radius whose
    /// planned route fits what the legs can count on over the fuse that is left (`hazards::escape_reach_m`, not the sprint over
    /// all of it), the walk is `Pace::Escape(EscapeWhy::Grenade)` (the sprint on any route, the sights, crouch, side steps and
    /// trigger let go). Off: the nearest floor by link distance at 5 m/s, `Pace::Run` with the sights up.
    pub escape: bool,
}

impl Default for AvoidTuning {
    fn default() -> Self {
        Self {
            grenade_avoid: true,
            bot_pockets: true,
            kit_in_use: true,
            told_ms: (200, 400),
            notice_ms: (400, 900),
            see_m: 30.0,
            cone_deg: 100.0,
            hear_m: 8.0,
            margin_m: 0.5,
            ahead_ms: 1_000,
            look_ms: 200,
            escape: false,
        }
    }
}

/// How often what runs (research B5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ThinkTuning {
    /// A bot's think (senses, play step), ms: drawn per think.
    pub think_ms: (u32, u32),
    /// The commander's tick, ms.
    pub commander_ms: u32,
    /// Sight rays a poll may cast, all bots together.
    pub rays_per_poll: u32,
    /// Floors a poll's route searches may expand, all bots together, as
    /// Detour's crowd shares its path queue's iterations: a search that
    /// runs past them goes on at the next polls (`Bots::poll`).
    pub search_floors_per_poll: u32,
    /// How the poll's route searches are shared out ([`SearchQueue`]):
    /// fairly by team (the default), or first come by place, as it was.
    pub search_queue: SearchQueue,
    /// A trace line at least this often per bot, ms.
    pub trace_every_ms: u32,
    /// A bot that gave up the tries to leave its pocket (`escape::Trapped::gave_up`)
    /// asks no search and takes no goal outside the pocket (`Ctx::reachable`,
    /// `drive`): it is the escape's, and `stuck::SelfKill`'s at the end. Off: it asks again for every goal the plays give it, every
    /// 0.5 s, as it did.
    pub shut_in_gate: bool,
    /// A rest point (the goal of a walk) on a lip or near one is moved inward to the nearest floor off the edge
    /// ([`super::super::path::off_lip`]): the owner's rule, a body at rest keeps its centre on the floor (the sniper that
    /// stood with its feet half over a drop). Off: the goal as the play gave it.
    pub rest_off_lip: bool,
    /// A body that stands on no floor of the grid (on the top of an object, 2.5 m or more above the
    /// floors) plans from the nearest floor it drops to, up to 5 m below ([`super::super::path::stranded_floor`]):
    /// the first leg of its route is the drop off the edge. Off: it has
    /// no start floor, so no route, and it stands (`no_way`).
    pub stranded_start: bool,
    /// A walked leg of a bot's route that crosses a doorway gets a corner at the middle of the opening
    /// ([`super::super::doors::centre_of`], `Route::door_centred`) and the walk steers at it before the corner after it
    /// (`Route::gate_within`): the walk's look-ahead cuts the corner before a doorway and the body arrives off the route,
    /// 0.32 m from the jamb on floors the grid keeps 0.3175 m from a wall, where the client's body holds 0.41 m (A66,
    /// Chemical Plant's 717 door). Off: the route as the planner pulled it.
    pub door_centres: bool,
    /// A pulled route's corners are kept where the capsule fits (`Route::wall_cleared`, A191b): a corner on a floor under 0.41 m
    /// from a wall moves to a floor within 0.5 m with room, and a leg that passes a wall under the capsule's 0.38 m gets a corner
    /// at a floor with room beside the dip. The grid keeps its floors 0.3175 m from walls, where a live capsule does not fit, and
    /// the stuck notes under the capsule stand-in are 13 % at a waypoint with no room, 8 % steering into a wall, 3 % in a gap
    /// narrower than the body. Off: the route as the planner pulled it (and after the door gates, if those are on).
    pub wall_clear_corners: bool,
    /// The bot's reckoned stamina bar (`stamina::Reckoning`) steers its legs: it lets go of the sprint at an empty bar
    /// (a lockout lasts as long as the sprint keys are held), stops sprinting early enough to arrive at a jump with the
    /// bar to pay it (the sprint to it, the run-up and the jump), and presses no jump the bar cannot pay. Off: the sprint and the jumps as before; the reckoning is counted.
    pub stamina: bool,
    /// The rates the bar is reckoned at ([`StaminaRates`]).
    pub stamina_rates: StaminaRates,
    /// A body that landed from a fall of 6 m or more cannot move for about 8 s (the J0 worlds, `stuck::HARD_FALL_M`):
    /// the stuck watch keeps quiet that long, as it cannot tell it from a wall. Off: the watch backs the body off and sidesteps, and cuts its play ("stuck twice there").
    pub fall_lock: bool,
}

impl Default for ThinkTuning {
    fn default() -> Self {
        Self {
            think_ms: (200, 400),
            commander_ms: 500,
            rays_per_poll: 400,
            search_floors_per_poll: SEARCH_FLOORS_PER_POLL,
            search_queue: SearchQueue::Fair,
            trace_every_ms: 5_000,
            shut_in_gate: true,
            rest_off_lip: true,
            stranded_start: true,
            door_centres: false,
            wall_clear_corners: false,
            stamina: true,
            stamina_rates: StaminaRates::Kit,
            fall_lock: true,
        }
    }
}

/// The rates the bot reckons its stamina bar at (`stamina::Rates`): a heavy kit's bar empties up to 2.5 times as fast
/// as the light kit's, so at the light rates a sprint that is long for the kit runs its real bar to 0 with the reckoning
/// still over the reserve (A207: the stand-in's lockouts rose 41 to 198 with the sprint on short legs). The bar is the
/// player's HUD, so the bot knowing its kit's is no cheat.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaminaRates {
    /// As it was: every kit at the light kit's (spend 5 a second, regain 10, a jump 10).
    Light,
    /// Each life at its kit's: the weight's cost against the kit's capacity and the skills (`stamina::Rates::of_kit`). The
    /// default (A207 ON's by-side bench, 288 matches, `feel.sprint_turns` off: lockouts 2 vs 26, z -4.5, falls off the route
    /// 839 vs 897, outcomes level).
    #[default]
    Kit,
}

/// A personality (play 14): weights per play family and motor habits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Personality {
    pub name: String,
    /// Its share of the bots.
    pub share: f32,
    /// Weights, about 1 (`curve::Input::Personality` reads them).
    pub hold: f32,
    pub push: f32,
    pub sneak: f32,
    pub support: f32,
    /// How readily it sprints, `[0, 1]`.
    pub sprint: f32,
}

impl Default for Personality {
    fn default() -> Self {
        Self {
            name: "steady".into(),
            share: 1.0,
            hold: 1.0,
            push: 1.0,
            sneak: 1.0,
            support: 1.0,
            sprint: 0.5,
        }
    }
}

impl Personality {
    /// The mix of how humans play (saved worlds, per life): about 55 %
    /// steady, 30 % mostly sprinting, 10-15 % holding still; so mostly
    /// flankers and support, a third rushers, few campers.
    pub fn defaults() -> Vec<Self> {
        let p = |name: &str, share, hold, push, sneak, support, sprint| Self {
            name: name.into(),
            share,
            hold,
            push,
            sneak,
            support,
            sprint,
        };
        vec![
            p("flanker", 0.3, 0.8, 1.0, 1.6, 0.9, 0.5),
            p("support", 0.25, 0.9, 0.8, 0.7, 1.6, 0.4),
            p("rusher", 0.3, 0.5, 1.6, 0.9, 0.8, 0.9),
            p("camper", 0.15, 1.8, 0.6, 0.8, 1.0, 0.2),
        ]
    }
}

/// The personalities and their share of the bots (play 14). In a file, a list of them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Personalities(pub Vec<Personality>);

impl Default for Personalities {
    fn default() -> Self {
        Self(Personality::defaults())
    }
}

impl std::ops::Deref for Personalities {
    type Target = Vec<Personality>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Personalities {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Check for Personalities {
    /// At least one, their shares summing to about 1.
    fn check(&self) -> Result<(), String> {
        let shares: f32 = self.0.iter().map(|p| p.share).sum();
        check(
            !self.0.is_empty() && (shares - 1.0).abs() < 0.05,
            "personalities: shares must sum to 1",
        )
    }
}

impl Check for AvoidTuning {
    /// The avoidance's ranges in order.
    fn check(&self) -> Result<(), String> {
        check(self.told_ms.0 <= self.told_ms.1, "avoid.told_ms")?;
        check(self.notice_ms.0 <= self.notice_ms.1, "avoid.notice_ms")?;
        check(self.see_m >= 0.0 && self.hear_m >= 0.0, "avoid.see_m, hear_m")?;
        check((0.0..=360.0).contains(&self.cone_deg), "avoid.cone_deg")?;
        check(self.margin_m >= 0.0, "avoid.margin_m")?;
        check(self.look_ms >= 20, "avoid.look_ms")?;
        Ok(())
    }
}

impl Check for ThinkTuning {
    /// The think rates' ranges in order.
    fn check(&self) -> Result<(), String> {
        check(
            self.think_ms.0 > 0 && self.think_ms.0 <= self.think_ms.1,
            "think.think_ms",
        )?;
        check(self.commander_ms >= 50, "think.commander_ms")?;
        Ok(())
    }
}

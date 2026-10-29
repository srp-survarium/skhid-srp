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
}

impl Default for CommitTuning {
    fn default() -> Self {
        Self {
            hold_to_arrival: true,
            cut_given_crowd: true,
            crossfire_point: true,
            crossfire_point_s: 30.0,
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
            replan: true,
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
    /// The bot's reckoned stamina bar (`stamina::Reckoning`) steers its legs: it lets go of the sprint at an empty bar
    /// (a lockout lasts as long as the sprint keys are held), stops sprinting early enough to arrive at a jump with the
    /// bar to pay it (the sprint to it, the run-up and the jump), and presses no jump the bar cannot pay. Off: the sprint and the jumps as before; the reckoning is counted.
    pub stamina: bool,
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
            stamina: true,
            fall_lock: true,
        }
    }
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

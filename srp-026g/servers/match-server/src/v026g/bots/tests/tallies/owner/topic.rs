//! A topic of the owner counts: a type that owns its counts, reads what it counts and gives its part of
//! the `owner` object and of the owner-test text at the end. A new topic is its file, its `mod` and `use`
//! lines and one line of [`topics`]: the tally calls every topic at the same points, in the order of the list.
//!
//! What a topic reads: the record of every body ([`Run`]); the game and its bodies; where only the bot knows
//! it, the bot itself ([`Topic::read`], a shared reference: the topic cannot change it), with a method on
//! `Bot` in the topic's file for a derived value.
//!
//! The list is a static list of constructors, not a registry of the `inventory` or `linkme` kind:
//! those register by code run before `main` or by link order and promise no order of iteration, and the topics'
//! hooks run in a fixed order on one tick (campers' kills are read once the tick's hits are in, for one).

use std::any::Any;

use super::*;

/// What a topic is given for one body at one tick, once the body is recorded.
pub(super) struct BodyAt<'a> {
    pub(super) game: &'a Game,
    pub(super) place: Place,
    pub(super) tick: u32,
    /// The record just made and the one before it.
    pub(super) rec: &'a Rec,
    pub(super) prev: &'a Rec,
    pub(super) sight: &'a mapdata::RayTree,
}

pub(super) trait Topic: Any {
    /// A topic as `Any`, for a test that reaches into one.
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// What only the bot knows, for each place each tick (none: not a bot): the topic keeps what it needs.
    fn read(&mut self, _place: Place, _bot: Option<&Bot>, _tick: u32) {}

    /// The same from a test's [`BotNow`].
    #[cfg(test)]
    fn inject(&mut self, _place: Place, _now: Option<&BotNow>) {}

    /// A bot's note (takes, stuck watches, grenades thrown, ...).
    fn note(&mut self, _run: &Run, _note: &Note, _kind: &NoteKind<'_>, _game: &Game) {}

    /// Before the bodies of a tick: what the game logged this tick (hits, landings).
    fn begin(&mut self, _run: &Run, _game: &mut Game) {}

    /// One body at one tick, after it was recorded. `rays` counts the rays cast.
    fn body(&mut self, _run: &Run, _at: &BodyAt<'_>, _rays: &mut u64) {}

    /// After the bodies of a tick.
    fn end(&mut self, _run: &Run, _game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {}

    /// What only the match-level state of the bots knows (the tracker's lists, the tactics' caches), once a
    /// tick before the readings. A test with no `Bots` never calls it.
    fn bots(&mut self, _bots: &Bots, _game: &Game) {}

    /// The counts this topic gives to the ones after it in [`topics`] (a count that another topic prints in its
    /// own line): by name, read in their `parts` as `shared`. Rare: a topic keeps its own counts.
    fn publish(&self) -> Vec<(&'static str, serde_json::Value)> {
        Vec::new()
    }

    /// The topic's parts at the end, each under its name (the golden counts keep one file a name).
    fn parts(&mut self, run: &Run, game: &Game, scale: &Scale, shared: &Shared) -> Vec<(&'static str, Part)>;
}

/// The counts the topics before one gave it ([`Topic::publish`]).
pub(super) type Shared = BTreeMap<&'static str, serde_json::Value>;

/// The topics (`n` places), in the order of the `owner-test` lines of the bench log (the first line, `owner-test:`, is
/// the fight's: the log tools read it) and of the hooks' calls: the campers' kills are read once the tick's
/// hits are in (their topic is after the fight's, which takes the hits); the outcomes are last, for they read what the topics
/// before them publish.
pub(super) fn topics(n: usize) -> Vec<Box<dyn Topic>> {
    vec![
        Box::new(FightCounts::new(n)),
        Box::new(JumpCounts::default()),
        Box::new(MoveCounts::new(n)),
        Box::new(GrenadeCounts::default()),
        Box::new(TeamKillCounts),
        Box::new(StockCounts),
        Box::new(StaminaCounts),
        Box::new(CamperCounts::default()),
        Box::new(ResupplyCounts::new(n)),
        Box::new(HealCounts::new(n)),
        Box::new(SightCounts::new(n)),
        Box::new(ShotOffsets::default()),
        Box::new(PitchBands::default()),
        Box::new(ShotBands::default()),
        Box::new(FireGate::new(n)),
        Box::new(SkillHits::new(n)),
        Box::new(ShotHeights::default()),
        Box::new(KickGap::default()),
        Box::new(HitShapes::default()),
        Box::new(DamageZoneFixes::default()),
        Box::new(ZoneDeaths::new(n)),
        Box::new(SmokeCounts::new()),
        Box::new(Scoreboard::default()),
        Box::new(Metrics::default()),
        Box::new(Feel::default()),
        Box::new(OutcomeCounts::default()),
        Box::new(PierceCounts::default()),
        Box::new(PocketCounts::default()),
        Box::new(SpacingCounts::new(n)),
        Box::new(GangUpCounts::default()),
        Box::new(PinCoverCounts::default()),
        Box::new(LiveListCounts::default()),
        Box::new(SearchQueueCounts::default()),
        Box::new(ThreatAnswerCounts::default()),
        Box::new(HitMarkCounts::default()),
        Box::new(BreachCounts::default()),
        Box::new(WorthCounts::default()),
        Box::new(GridCounts::new(n)),
        Box::new(ThroughCounts::default()),
        Box::new(OutrangedCounts::default()),
        Box::new(AnswerCounts::default()),
        Box::new(PaceCounts::new(n)),
        Box::new(SenseCounts::default()),
        Box::new(SniperCounts::new(n)),
        Box::new(GunShots::new(n)),
        Box::new(StealCounts::new(n)),
        Box::new(CrouchCounts::new(n)),
        Box::new(RangeLegCounts::new(n)),
        Box::new(SprintTables::default()),
        Box::new(LookCounts::new(n)),
        Box::new(CollectAtCounts::new(n)),
    ]
}

/// What a test fabricates of a bot at a tick ([`OwnerTally::inject`]): the topics' [`Topic::inject`] take what
/// they read of it.
#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub(super) struct BotNow {
    pub(super) dry: bool,
    pub(super) reload: Option<(u32, u32)>,
    pub(super) jump: Option<(&'static str, Option<(Vec3, Vec3)>)>,
    pub(super) play: String,
    pub(super) step: Option<String>,
    pub(super) goal_m: Option<f32>,
    pub(super) range_held: Option<u32>,
    /// The goal it has no route to and why it stands short of it ([`StoodRead`]).
    pub(super) stood: Option<StoodRead>,
    pub(super) grenades: GrenadeStats,
    /// Its view is held for a throw now, and how long ago, ms, it let go of its last grenade.
    pub(super) throw_held: bool,
    pub(super) thrown_ago_ms: Option<u32>,
    /// What its avoidance of live grenades has counted this match (`grenade_threat::AvoidStats`).
    pub(super) avoid: crate::v026g::bots::grenade_threat::AvoidStats,
    /// The resupply holds it has made (`Bot::resupplies`), and how near dry it is ([`Nearly`]).
    pub(super) resupplies: u32,
    pub(super) nearly: Nearly,
    /// Its weapon 1's rounds and the kit's count for a life.
    pub(super) ammo: crate::v026g::bots::guns::Ammo,
    /// The reflex has a target; a heal cover walk begun in the last 1.6 s; the walks begun so far in the match.
    pub(super) target: bool,
    pub(super) heal_walking: bool,
    pub(super) heal_walks: u32,
    /// Why the last heal press's cover walk was or was not made.
    pub(super) heal_note: Option<crate::v026g::bots::reflex::HealNote>,
    /// The fire gates of its last tick (`Reflex::gates`).
    pub(super) gates: crate::v026g::bots::reflex::Gates,
    /// It has a live sight of an enemy.
    pub(super) sighted: bool,
    /// Its sprint gate's last answer, the cause of a sprint's end, and its bar (`PaceCounts`).
    pub(super) pace: Option<PaceNow>,
    /// Its assignment, and its tracks (who, source, time).
    pub(super) aim: Option<(crate::v026g::bots::plays::PlayKind, crate::v026g::bots::plays::TargetRef)>,
    pub(super) tracks: Vec<(Place, crate::v026g::bots::know::Source, u32)>,
    /// Why the reflex has its legs ([`Bot::bench_legs_why`]).
    pub(super) legs: Option<crate::v026g::bots::reflex::LegsWhy>,
}

#[cfg(test)]
impl BotNow {
    pub(super) fn doing(&self) -> Doing {
        Doing {
            play: self.play.clone(),
            step: self.step.clone(),
            goal_m: self.goal_m,
            jump: self.jump,
            cover: "",
            after_landing: None,
            walk: 0,
            why: 0,
            has_route: false,
            motor_keys: 0,
            stepped: 0,
            apart: 0,
            guard: 0,
            watch: LipWatchKind::Watching,
            guard_why: 0,
        }
    }
}

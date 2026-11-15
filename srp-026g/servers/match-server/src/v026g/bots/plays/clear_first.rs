//! [take.clear_first] Every play's walk at a loose battery a man holds: no walk in on him, a Breach on him instead.
//!
//! The live recordings of 2026-11-13: 13 bots killed by a man standing by a loose battery, all 13 knowing of him (heard 8,
//! told 3, seen 2), none aiming at him. Only 6 were on Collect: the others walked to the same place on Intercept,
//! GuardLead, Escort, Raid, Trade, Flank and Roam. So the rule is the walk's, read in one place for every play
//! ([`clear_first`], on each play's intent in `think.rs` beside `walk_watch`), not a copy per play.
//!
//! The rule. A loose battery within the take's `exposed_m` is held by a man the bot knows by his spot alone
//! ([`Ctx::held_battery`] with [`ThreatBy::Spot`]: heard, told of or lost from sight lately, covered by nobody). A man seen
//! is the fight's: the reflex has him. A play's walk that would bring the bot nearer him:
//! - does not walk in. The bot tells the team of him (`know::BatteryHold`); the commander makes his spot a held line of no
//!   lanes (`threat::HeldLine::at_battery`) due a Breach at once (`breach.rs`, `threat.breach`): two crossers gather out
//!   of his sight, go together to two cover points that see him, one throws first, the reflex fights him when he shows;
//! - waits out of his sight, sights up, looking at him: where it stands if he cannot see it there, else at the nearest of
//!   the map's spots out of his sight on a ring round him no nearer than it is (`Tactics::spots` with `unseen_by`);
//! - is bounded by the team's jobs' own times, with no constant of its own. While a team job on him runs (a Breach whose
//!   shooter he is, or a Flank job on him) it waits: their gather, go, hold, trade and give-up times end them; with none it
//!   waits at most `breach::DUE_MS`, the time his spot stays due a Breach that may not open (no crossers, one at a time).
//!   Then the play walks on (the objective is not given up), and the same man is not waited for again while the team
//!   holds him at that battery;
//! - ends when he is dead (the kill feed), reloading, seen (the fight takes over), sensed elsewhere, or his spot no longer
//!   lives (`Ctx::spot_lives`: his track's life, `sense.drop_ms`, unless the team saw that spot empty for `threat::CLEAR_MS`).
//!
//! The play keeps its target throughout. A carrier (its way home is its own), a heal, a retreat and a Breach's own bots
//! are left alone.

use mapdata::Vec3;

use super::{Ctx, Go, HeldBattery, Intent, Look, PlayKind, ThreatBy};
use crate::v026g::bots::breach;
use crate::v026g::bots::know::{BatteryHold, HoldLook, Source};
use crate::v026g::bots::rng::Rng;
use crate::v026g::bots::tactics::SpotQuery;
use crate::v026g::bots::threat::BATTERY_LINE_ID;
use crate::v026g::bots::view::Place;
use crate::v026g::drive::key;

/// The bot's state at a held battery ([`clear_first`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Clearing {
    /// Its walk held off from him since `since`: waiting at `stand` for a Breach on him, or for his end.
    Waiting { battery: usize, who: Place, stand: Stand, since: u32, wait: Wait },
    /// Its wait ran out (the bound): its play walks on while he holds that battery.
    Released { battery: usize, who: Place },
}

impl Clearing {
    fn of(&self) -> (usize, Place) {
        match *self {
            Clearing::Waiting { battery, who, .. } | Clearing::Released { battery, who } => (battery, who),
        }
    }
}

/// What bounds a wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    /// No team job on him (yet): at most `breach::DUE_MS` from the wait's start.
    Alone,
    /// A team job on him runs (a Breach whose shooter he is, on his battery's line or his kill spot, or a Flank job on
    /// him): its own times end it.
    Job,
}

/// Where a waiting bot stands.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stand {
    /// Where it stands: he cannot see it there, or the map offered no spot out of his sight.
    Here,
    /// Walking to this spot out of his sight.
    To(Vec3),
}

/// Why a wait ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Dead (the kill feed).
    Dead,
    /// Heard reloading.
    Reloading,
    /// Seen: the fight's.
    Seen,
    /// His spot faded.
    Faded,
    /// The play's walk no longer goes toward him.
    WalkedOff,
    /// The team's job on him ended with him still holding it.
    JobEnded,
}

/// The held line id of a man's battery hold (`threat::HeldLine::at_battery`).
pub fn line_id(who: Place) -> u32 {
    BATTERY_LINE_ID + who as u32
}

/// The man a held battery's threat is had by, known by his spot alone: his spot and its sensing. None for a man seen.
fn known_spot(held: &HeldBattery) -> Option<(Vec3, u32)> {
    match held.threat.by {
        ThreatBy::Spot { at, sensed } => Some((at, sensed)),
        ThreatBy::Sight => None,
    }
}

/// Why `who`'s hold ended for the bot: his death, a reload, a sight, else his spot faded.
fn end_of(cx: &Ctx, who: Place) -> End {
    let reload_ms = (cx.num(PlayKind::Collect, "reload_cover_s") * 1000.0) as u32;
    if cx.view.place(who).is_none_or(|p| !p.alive) {
        End::Dead
    } else if cx.reloading(who, reload_ms) {
        End::Reloading
    } else if cx
        .memory
        .tracks()
        .iter()
        .any(|t| t.who == who && t.source == Source::Seen && cx.now <= t.time + super::SEEN_NOW_MS)
    {
        End::Seen
    } else {
        End::Faded
    }
}

/// The play `kind`'s `intent` at a held battery (see the module), with `memory` the bot's state across thinks; and the hold to
/// tell the team of, if the bot knows a man by his spot at a loose battery. The intent as it was with `take.clear_first` off,
/// no battery held so, a walk that does not bring the bot nearer him, a carrier, a heal, a retreat or a Breach's bot.
pub fn clear_first(
    cx: &Ctx,
    kind: PlayKind,
    memory: &mut Option<Clearing>,
    rng: &mut Rng,
    intent: Intent,
) -> (Intent, Option<BatteryHold>) {
    let held = if cx.tuning.take.clear_first {
        cx.held_battery().and_then(|h| known_spot(&h).map(|spot| (h, spot)))
    } else {
        None
    };
    let hold = held.map(|(h, (at, sensed))| BatteryHold::new(h.threat.who, at, sensed, h.battery, h.at));
    let gated = cx.me.carrying.is_none()
        && !matches!(kind, PlayKind::Heal | PlayKind::Retreat | PlayKind::Breach)
        && breach::role_in(&cx.team.breaches, cx.me.place).is_none();
    let walking_in = |spot: Vec3| match intent.go {
        Go::To { at, .. } => at.distance_xz(spot) < cx.me.feet.distance_xz(spot),
        _ => false,
    };
    // The same man at the same battery: by the bot's own threat, or the team's hold of him (kept while his track lives, not
    // cleared), so a release outlives a moment the bot's own sense of him lapses.
    let still = |c: &Clearing| {
        let (battery, who) = c.of();
        held.is_some_and(|(h, _)| (h.battery, h.threat.who) == (battery, who))
            || cx.team.battery_holds.iter().any(|h| h.who == who && h.battery == battery && h.look != HoldLook::Cleared)
    };
    let Some((held, (spot, _))) = held.filter(|(_, (spot, _))| gated && walking_in(*spot)) else {
        match *memory {
            Some(c @ Clearing::Waiting { battery, who, .. }) => {
                let end = if still(&c) { End::WalkedOff } else { end_of(cx, who) };
                tally::count(kind, tally::Event::Ended(end));
                log::debug!("clear first: {kind:?} at battery {battery} ends its wait for player {who}: {end:?}");
                *memory = None;
            }
            Some(c @ Clearing::Released { .. }) if !still(&c) => *memory = None,
            _ => {}
        }
        return (intent, hold);
    };
    let key = (held.battery, held.threat.who);
    if memory.is_none_or(|c| c.of() != key) {
        let stand = stand_for(cx, spot, rng);
        tally::count(kind, tally::Event::Gated);
        cx.lines.push(
            crate::v026g::bots::nav_log::Kind::Take,
            Some(crate::v026g::bots::nav_log::key(("clear first", key.0, key.1, cx.now))),
            crate::v026g::bots::nav_log::clear_line(kind, key.0, held.at, key.1, spot, stand),
        );
        *memory = Some(Clearing::Waiting { battery: key.0, who: key.1, stand, since: cx.now, wait: Wait::Alone });
    }
    // A team job on him: a Breach whose shooter he is (on his battery's line, or on his kill spot, which keeps his battery's
    // Breach from opening: one job a shooter), or a Flank job on him.
    let job_on = cx.team.breaches.iter().any(|b| b.shooter == key.1 && !b.phase.ended())
        || cx.team.threat.jobs.iter().any(|j| j.shooter == key.1);
    let Some(Clearing::Waiting { stand, since, wait, .. }) = memory.as_mut() else {
        // Released: the play walks on.
        return (intent, hold);
    };
    let released = match (*wait, job_on) {
        (Wait::Alone, true) => {
            *wait = Wait::Job;
            tally::count(kind, tally::Event::JobOn);
            None
        }
        (Wait::Job, false) => Some((tally::Event::Ended(End::JobEnded), "the team's job on him ended")),
        (Wait::Alone, false) if cx.now >= since.saturating_add(breach::DUE_MS) => {
            Some((tally::Event::TimedOut, "no team job on him within DUE_MS"))
        }
        _ => None,
    };
    if let Some((event, why)) = released {
        tally::count(kind, event);
        log::debug!("clear first: {kind:?} at battery {} walks on past player {}: {why}", key.0, key.1);
        *memory = Some(Clearing::Released { battery: key.0, who: key.1 });
        return (intent, hold);
    }
    tally::count(kind, tally::Event::Step);
    // At the stand by the walk's own arrival (`Intent::to`'s `within_m`): it waits there.
    if let Stand::To(at) = *stand
        && let Go::To { within_m, .. } = Intent::to(at, intent.pace).go
        && cx.me.feet.distance_xz(at) <= within_m
    {
        *stand = Stand::Here;
    }
    let aimed = |i: Intent| Intent { hold: i.hold | key::AIM, ..i.looking(Look::At(spot)) };
    let waiting = match *stand {
        Stand::Here => aimed(Intent::stay()),
        Stand::To(at) => aimed(Intent::to(at, intent.pace)),
    };
    (waiting, hold)
}

/// Where to wait out of the sight of a man at `spot`: here if he cannot see the bot's eye from there, else the nearest of the
/// map's spots out of his sight on a ring round him from the bot's distance out to the take's `exposed_m` whose straight walk
/// never comes nearer him than the bot is now (all the ring's samples are weighed, not the spots' own best: theirs favour
/// the ground near him); here if the map offers none.
fn stand_for(cx: &Ctx, spot: Vec3, rng: &mut Rng) -> Stand {
    if !cx.tactics.sees(Ctx::standing_eye(spot), cx.me.eye()) {
        return Stand::Here;
    }
    let num = |name| cx.num(PlayKind::Collect, name);
    let (near_m, exposed_m) = (num("hold_near_m"), num("exposed_m"));
    let from = cx.me.feet.distance_xz(spot).max(near_m);
    let query = SpotQuery {
        unseen_by: Some(spot),
        side: cx.bot.side,
        ..SpotQuery::ring(spot, from, exposed_m.max(from + near_m))
    };
    let me = cx.me.feet;
    let now_m = me.distance_xz(spot);
    cx.tactics
        .spots(&query, cx.now, query.samples, rng)
        .into_iter()
        .map(|s| s.at)
        // The walk there (the straight line) never nearer him than now: not round past him.
        .filter(|&at| nearest_on(me, at, spot) >= now_m)
        .min_by(|a, b| me.distance_xz(*a).total_cmp(&me.distance_xz(*b)))
        .map_or(Stand::Here, Stand::To)
}

/// The least ground distance from `p` to the segment `a`-`b`, m.
fn nearest_on(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    let (dx, dz) = (b.x - a.x, b.z - a.z);
    let len = dx * dx + dz * dz;
    let t = if len > 0.0 { (((p.x - a.x) * dx + (p.z - a.z) * dz) / len).clamp(0.0, 1.0) } else { 0.0 };
    Vec3::new(a.x + dx * t, p.y, a.z + dz * t).distance_xz(p)
}

/// [take.clear_first] The clearings by play (test builds count; the bench's route tally prints them at the match's end).
pub mod tally {
    use super::{End, PlayKind};

    /// What a clearing did.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Event {
        /// Its walk held off from a man at a loose battery.
        Gated,
        /// A team job on him (a Breach or a Flank) ran while it waited.
        JobOn,
        /// Its wait ran out with no Breach on him (`breach::DUE_MS`): the play walks on.
        TimedOut,
        /// A think spent waiting.
        Step,
        /// Ended, and why.
        Ended(End),
    }

    pub const EVENTS: usize = 10;
    pub const NAMES: [&str; EVENTS] = [
        "gated",
        "job_on",
        "timed_out",
        "steps",
        "end_dead",
        "end_reloading",
        "end_seen",
        "end_faded",
        "end_walked_off",
        "end_job_ended",
    ];

    #[cfg_attr(not(test), allow(dead_code))]
    fn index(event: Event) -> usize {
        match event {
            Event::Gated => 0,
            Event::JobOn => 1,
            Event::TimedOut => 2,
            Event::Step => 3,
            Event::Ended(End::Dead) => 4,
            Event::Ended(End::Reloading) => 5,
            Event::Ended(End::Seen) => 6,
            Event::Ended(End::Faded) => 7,
            Event::Ended(End::WalkedOff) => 8,
            Event::Ended(End::JobEnded) => 9,
        }
    }

    #[cfg(test)]
    thread_local! {
        static COUNTS: std::cell::Cell<[[u64; EVENTS]; PlayKind::ALL.len()]> =
            const { std::cell::Cell::new([[0; EVENTS]; PlayKind::ALL.len()]) };
    }

    #[cfg_attr(not(test), allow(unused_variables))]
    pub fn count(kind: PlayKind, event: Event) {
        #[cfg(test)]
        COUNTS.with(|c| {
            let mut counts = c.get();
            if let Some(k) = PlayKind::ALL.iter().position(|&p| p == kind) {
                counts[k][index(event)] += 1;
            }
            c.set(counts);
        });
    }

    /// The counts so far on this thread, by play (`PlayKind::ALL`'s order) and event ([`NAMES`]).
    #[cfg(test)]
    pub fn counts() -> [[u64; EVENTS]; PlayKind::ALL.len()] {
        COUNTS.with(std::cell::Cell::get)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::breach::{Breach, Crosser, Phase};
    use crate::v026g::bots::know::TeamKnowledge;
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::plays::{Pace, TargetRef};
    use crate::v026g::bots::tactics::Tactics;
    use crate::v026g::bots::threat::{HeldLine, Holder};
    use crate::v026g::bots::view::{BatteryState, Mode};

    const BATTERY: Vec3 = Vec3::new(25.0, 0.0, 12.0);
    const HIM: Vec3 = Vec3::new(27.0, 0.0, 12.0);
    const ME: Vec3 = Vec3::new(8.0, 0.0, 12.0);

    /// The field with `solids`, a loose battery, the bot of team 0 on Intercept at `ME`, and a man of team 1 heard at
    /// `HIM` (2 m from the battery); both switches on.
    fn scene(solids: &[mapdata::Triangle]) -> Scenario {
        let mut s = Scenario::field(Mode::Batteries);
        s.tactics = Tactics::new(crate::v026g::bots::tactics::tests::field_with_solids(solids, |_| {}), 1.0);
        s.battery(BatteryState::Free(BATTERY));
        s.bot(0, 0, ME);
        s.current = Some((PlayKind::Intercept, TargetRef::Place(1), s.view.clock));
        s.enemy(1, 1, HIM, Some(Source::Heard));
        s.tuning.take.known_threats = true;
        s.tuning.take.clear_first = true;
        s
    }

    fn run(s: &Scenario, kind: PlayKind, memory: &mut Option<Clearing>, intent: Intent) -> (Intent, Option<BatteryHold>) {
        clear_first(&s.cx(0), kind, memory, &mut Rng::new(1), intent)
    }

    fn waits_on_him(intent: &Intent) -> bool {
        matches!(intent.look, Look::At(at) if at == HIM) && intent.hold & key::AIM != 0
    }

    fn walk_to(at: Vec3) -> Intent {
        Intent::to(at, Pace::Run)
    }

    /// A walk in on a man heard by a loose battery does not walk in, whatever the play: it waits, sights up on him, and tells
    /// the team of him (his spot, its sensing, the battery); dead, the play walks on. A walk away, a retreat and a carrier
    /// walk on; the switch off tells nobody.
    #[test]
    fn a_walk_in_on_a_man_heard_by_a_battery_waits_and_tells_the_team() {
        let walk_in = walk_to(BATTERY);
        for kind in [PlayKind::Intercept, PlayKind::GuardLead, PlayKind::Collect, PlayKind::Escort, PlayKind::Roam] {
            let s = scene(&[]);
            let mut memory = None;
            let (waiting, hold) = run(&s, kind, &mut memory, walk_in);
            assert!(matches!(waiting.go, Go::Stay) && waits_on_him(&waiting), "{kind:?}: {waiting:?}");
            assert_eq!(hold, Some(BatteryHold::new(1, HIM, s.view.clock, 0, BATTERY)));
            assert!(matches!(memory, Some(Clearing::Waiting { battery: 0, who: 1, wait: Wait::Alone, .. })), "{memory:?}");
        }
        let mut s = scene(&[]);
        let mut memory = None;
        run(&s, PlayKind::Intercept, &mut memory, walk_in);
        s.view.places[1].alive = false;
        assert_eq!(run(&s, PlayKind::Intercept, &mut memory, walk_in).0, walk_in, "dead: walks on");
        assert_eq!(memory, None);
        let s = scene(&[]);
        let away = walk_to(Vec3::new(2.0, 0.0, 12.0));
        assert_eq!(run(&s, PlayKind::Intercept, &mut None, away).0, away, "a walk away");
        assert_eq!(run(&s, PlayKind::Retreat, &mut None, walk_in).0, walk_in, "a retreat");
        let mut s = scene(&[]);
        s.view.places[0].carrying = Some(0);
        assert_eq!(run(&s, PlayKind::Collect, &mut None, walk_in).0, walk_in, "a carrier");
        let mut s = scene(&[]);
        s.tuning.take.clear_first = false;
        assert_eq!(run(&s, PlayKind::Intercept, &mut None, walk_in), (walk_in, None), "off");
    }

    /// The bound, Breach's own: with no Breach on him a wait lasts `breach::DUE_MS`, then the play walks on and does not wait
    /// for the same man again; while a Breach runs on him it waits past that, until the Breach ends.
    #[test]
    fn a_wait_lasts_due_ms_alone_and_a_breach_on_him_while_it_runs() {
        let walk_in = walk_to(BATTERY);
        let mut s = scene(&[]);
        let mut memory = None;
        run(&s, PlayKind::GuardLead, &mut memory, walk_in);
        s.view.clock += breach::DUE_MS - 100;
        s.enemy(1, 1, HIM, Some(Source::Heard));
        assert!(waits_on_him(&run(&s, PlayKind::GuardLead, &mut memory, walk_in).0), "within DUE_MS");
        s.view.clock += 200;
        s.enemy(1, 1, HIM, Some(Source::Heard));
        assert_eq!(run(&s, PlayKind::GuardLead, &mut memory, walk_in).0, walk_in, "past DUE_MS: walks on");
        assert_eq!(memory, Some(Clearing::Released { battery: 0, who: 1 }));
        assert_eq!(run(&s, PlayKind::GuardLead, &mut memory, walk_in).0, walk_in, "not again for the same man");
        // A Breach on him: the wait goes past DUE_MS, and ends with it.
        let mut s = scene(&[]);
        let mut memory = None;
        run(&s, PlayKind::GuardLead, &mut memory, walk_in);
        let now = s.view.clock;
        let crosser =
            |place, x| Crosser { holder: Holder { place, since: now }, gather: Vec3::new(x, 0.0, 30.0), cover: Vec3::new(x, 0.0, 20.0) };
        let job = Breach {
            spot: line_id(1),
            shooter: 1,
            at: HIM,
            objective: true,
            since: now,
            pinner: None,
            crossers: [crosser(2, 20.0), crosser(4, 30.0)],
            points_at_open: None,
            phase: Phase::Gather { until: now + 30_000 },
        };
        s.team.breaches = std::sync::Arc::new(vec![job]);
        assert!(waits_on_him(&run(&s, PlayKind::GuardLead, &mut memory, walk_in).0));
        assert!(matches!(memory, Some(Clearing::Waiting { wait: Wait::Job, .. })), "{memory:?}");
        s.view.clock += breach::DUE_MS + 1_000;
        s.enemy(1, 1, HIM, Some(Source::Heard));
        assert!(waits_on_him(&run(&s, PlayKind::GuardLead, &mut memory, walk_in).0), "the Breach runs: still waiting");
        s.team.breaches = std::sync::Arc::default();
        assert_eq!(run(&s, PlayKind::GuardLead, &mut memory, walk_in).0, walk_in, "the Breach ended: walks on");
        assert_eq!(memory, Some(Clearing::Released { battery: 0, who: 1 }));
        // A Flank job on him (his kill spot): the wait goes past DUE_MS while it runs.
        let mut s = scene(&[]);
        let mut memory = None;
        run(&s, PlayKind::GuardLead, &mut memory, walk_in);
        let flank = crate::v026g::bots::threat::Job::for_test(4, 1, now, Some(Holder { place: 2, since: now }), None);
        s.team.threat = std::sync::Arc::new(crate::v026g::bots::threat::Picture { jobs: vec![flank], ..Default::default() });
        s.view.clock += breach::DUE_MS + 1_000;
        s.enemy(1, 1, HIM, Some(Source::Heard));
        assert!(waits_on_him(&run(&s, PlayKind::GuardLead, &mut memory, walk_in).0), "a Flank on him runs: still waiting");
        assert!(matches!(memory, Some(Clearing::Waiting { wait: Wait::Job, .. })), "{memory:?}");
    }

    /// A release outlives a lapse of the bot's own sense of him while the team holds him at that battery: no new wait.
    #[test]
    fn a_release_outlives_a_lapse_while_the_team_holds_him() {
        let walk_in = walk_to(BATTERY);
        let mut s = scene(&[]);
        s.team.hold_battery(BatteryHold::new(1, HIM, s.view.clock, 0, BATTERY));
        let mut memory = Some(Clearing::Released { battery: 0, who: 1 });
        // The bot's own sense of him lapses (no threat this think): the release is kept by the team's hold.
        s.view.places[1].alive = true;
        let mut lapsed = s.memory.clone();
        lapsed.forget(1);
        s.memory = lapsed;
        assert_eq!(run(&s, PlayKind::GuardLead, &mut memory, walk_in).0, walk_in);
        assert_eq!(memory, Some(Clearing::Released { battery: 0, who: 1 }));
        // Heard again: still released, the play walks on.
        s.enemy(1, 1, HIM, Some(Source::Heard));
        assert_eq!(run(&s, PlayKind::GuardLead, &mut memory, walk_in).0, walk_in);
    }

    /// In his sight, the bot waits at the nearest of the map's spots out of his sight, no nearer him; there it stays.
    #[test]
    fn in_his_sight_it_waits_at_a_spot_out_of_it_no_nearer_him() {
        let walk_in = walk_to(BATTERY);
        let wall = crate::v026g::bots::body::tests::wall_x(14.0, 16.0, 30.0);
        let mut s = scene(&wall);
        assert!(s.tactics.sees(Ctx::standing_eye(HIM), s.cx(0).me.eye()), "he sees the bot where it stands");
        let mut memory = None;
        let (going, _) = run(&s, PlayKind::Trade, &mut memory, walk_in);
        let Some(Clearing::Waiting { stand: Stand::To(stand), .. }) = memory else { panic!("no stand: {memory:?}") };
        assert!(matches!(going.go, Go::To { at, .. } if at == stand) && waits_on_him(&going), "{going:?}");
        assert!(stand.distance_xz(HIM) >= ME.distance_xz(HIM) - 0.01, "no nearer him: {stand:?}");
        s.bot(0, 0, stand);
        assert!(!s.tactics.sees(Ctx::standing_eye(HIM), s.cx(0).me.eye()), "out of his sight there");
        let (there, _) = run(&s, PlayKind::Trade, &mut memory, walk_in);
        assert!(matches!(there.go, Go::Stay) && waits_on_him(&there), "{there:?}");
    }

    /// A man silent by the battery past `sense.forget_ms` still holds it while his track lives (`sense.drop_ms`): the gate
    /// waits on. Once the team saw his spot empty (the hold cleared) that spot of him is no threat, and the play walks on; a
    /// newer sensing of him counts again.
    #[test]
    fn a_silent_man_holds_while_his_track_lives_and_a_spot_seen_empty_does_not() {
        use crate::v026g::bots::know::HoldLook;
        let walk_in = walk_to(BATTERY);
        let mut s = scene(&[]);
        let sensed = s.view.clock;
        s.view.clock += s.tuning.sense.forget_ms + 2_000;
        assert!(s.tuning.sense.forget_ms + 2_000 < s.tuning.sense.drop_ms);
        let mut memory = None;
        assert!(waits_on_him(&run(&s, PlayKind::GuardLead, &mut memory, walk_in).0), "silent past forget_ms: held");
        let mut cleared = BatteryHold::new(1, HIM, sensed, 0, BATTERY);
        cleared.look = HoldLook::Cleared;
        s.team.hold_battery(cleared);
        assert_eq!(run(&s, PlayKind::GuardLead, &mut memory, walk_in).0, walk_in, "seen empty: walks on");
        s.enemy(1, 1, HIM, Some(Source::Heard));
        assert!(waits_on_him(&run(&s, PlayKind::GuardLead, &mut None, walk_in).0), "heard again: held again");
    }

    /// A man carrying the battery is closed on (a carrier cannot fire); 2-4 s after he puts it down by him, the next think
    /// holds off.
    #[test]
    fn a_carrier_is_closed_on_and_the_think_after_he_puts_it_down_holds_off() {
        let walk_in = walk_to(HIM);
        let mut s = scene(&[]);
        s.view.batteries[0] = BatteryState::Carried(Some(1));
        s.view.places[1].carrying = Some(0);
        let mut memory = None;
        assert_eq!(run(&s, PlayKind::Intercept, &mut memory, walk_in).0, walk_in, "carrying: closed on");
        for after in [2_000, 4_000] {
            let mut s = scene(&[]);
            s.view.clock += after;
            s.enemy(1, 1, HIM, Some(Source::Heard));
            let mut memory = None;
            assert!(waits_on_him(&run(&s, PlayKind::Intercept, &mut memory, walk_in).0), "{after} ms after the drop: held off");
        }
    }

    /// The commander's verdict on a hold: kept while his track lives, moved to a newer sensing by the battery, dropped when
    /// he is dead or sensed elsewhere; seen empty for `CLEAR_MS` it is cleared.
    #[test]
    fn a_hold_is_kept_moved_dropped_and_cleared_by_the_rules() {
        use crate::v026g::bots::know::{HoldLook, keep_hold};
        use crate::v026g::bots::threat::CLEAR_MS;
        let (drop_ms, exposed_m) = (20_000, 25.0);
        let mut h = BatteryHold::new(1, HIM, 1_000, 0, BATTERY);
        assert!(keep_hold(&mut h, true, None, false, 15_000, drop_ms, exposed_m), "silent: his track lives");
        assert!(!keep_hold(&mut h.clone(), true, None, false, 21_001, drop_ms, exposed_m), "his track gone");
        assert!(!keep_hold(&mut h.clone(), false, None, false, 2_000, drop_ms, exposed_m), "dead");
        assert!(!keep_hold(&mut h.clone(), true, Some((Vec3::new(80.0, 0.0, 12.0), 2_000)), false, 2_000, drop_ms, exposed_m), "elsewhere");
        let near = Vec3::new(28.0, 0.0, 14.0);
        assert!(keep_hold(&mut h, true, Some((near, 3_000)), false, 3_000, drop_ms, exposed_m));
        assert_eq!((h.at, h.sensed, h.look), (near, 3_000, HoldLook::Open), "moved");
        assert!(keep_hold(&mut h, true, None, true, 4_000, drop_ms, exposed_m));
        assert_eq!(h.look, HoldLook::EmptySince(4_000));
        assert!(keep_hold(&mut h, true, None, true, 4_000 + CLEAR_MS, drop_ms, exposed_m));
        assert_eq!(h.look, HoldLook::Cleared);
        assert!(keep_hold(&mut h, true, None, false, 8_000, drop_ms, exposed_m));
        assert_eq!(h.look, HoldLook::Cleared, "cleared stays until he is sensed again");
        assert!(keep_hold(&mut h, true, Some((near, 9_000)), false, 9_000, drop_ms, exposed_m));
        assert_eq!(h.look, HoldLook::Open, "sensed again: open");
    }

    /// A Breach refused on a battery's line rests it: not made due again while it stands (within DUE_MS, his spot on it, no
    /// more bots free than then); a bot freed, his spot moved or DUE_MS passed lets it be tried again.
    #[test]
    fn a_refused_battery_line_rests_until_what_refused_it_changes() {
        use crate::v026g::bots::threat::BatteryRefused;
        let r = BatteryRefused { spot: line_id(1), at: HIM, free: 1, until: 1_000 + breach::DUE_MS };
        assert!(r.stands(HIM, 1, 5_000, 2.0), "nothing changed");
        assert!(!r.stands(HIM, 2, 5_000, 2.0), "a bot freed");
        assert!(!r.stands(HIM + Vec3::new(3.0, 0.0, 0.0), 1, 5_000, 2.0), "his spot moved");
        assert!(!r.stands(HIM, 1, 1_000 + breach::DUE_MS, 2.0), "DUE_MS passed");
    }

    /// The team keeps one hold a man, the latest; his held line covers no ground.
    #[test]
    fn a_battery_hold_is_one_a_man_and_its_line_covers_nothing() {
        let mut team = TeamKnowledge::default();
        let hold = |sensed| BatteryHold::new(1, HIM, sensed, 0, BATTERY);
        team.hold_battery(hold(1_000));
        team.hold_battery(hold(900));
        team.hold_battery(hold(1_200));
        assert_eq!(team.battery_holds, vec![hold(1_200)]);
        let line = HeldLine::at_battery(&hold(1_200));
        assert_eq!((line.id, line.shooter, line.battery, line.seen), (line_id(1), 1, Some(0), Some(1_200)));
        assert!(!line.covers(HIM) && !line.covers(BATTERY) && !line.covers(ME));
    }
}

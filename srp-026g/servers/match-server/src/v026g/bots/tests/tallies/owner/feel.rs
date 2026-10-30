//! The feel of a match for one player (`traces-20261028/an-top-player.md` §5, `an-gang-up.md`): the counters that tell whether the
//! bots treat a star, or a human, as a live crowd does. Each is worked out the way the live census worked it out, so that a bench
//! number can be set beside the live one. By team for the team's top bot, for the stand-in human (`SRP_BENCH_HUMAN`) and, for the
//! rest of the bots, as the baseline they are compared with.
//!
//! - **top:** a team's top is whoever the bots' own rule names (`plays::pair_up::TopRule`, the one PairUp calls, with its default
//!   numbers): kills + 2 x streak at least 8 and 3 over the team's next, read at the last tick (so a death does not hide that he was
//!   top). The stand-in human and camper are ranked like the bots; when one leads he is also counted in `top of team N` (and always in
//!   his own group), which is the case the tripwire of `an-top-player.md` §5 watches.
//! - **deaths a minute alive** as top and not (live 1.19 and 0.81).
//! - **gang-up share:** deaths with 3 or more distinct enemies within 30 m (ground) of the dead at some sample of the 10 s before (live
//!   tops 41 to 45 %, non-tops 27 to 29 %), and the mean number of enemies within 30 m at the death, and distinct in the 10 s (live
//!   1.1 for a top's whole life). **Differences from the live census, with their effect:** the live worlds are 1.25 s apart and the
//!   window is sampled the same way here (every 25th tick), so a pass between samples is missed in both; the window starts at the life's
//!   spawn (not in the previous life); the live groups are by match size (18-20 players: 45 % top, 29 % not), the bench's matches are
//!   6 to 20 players of bots, 180 s, from the first second (the first minute is the busiest, so it raises the bench's share).
//! - **spawn to first contact:** seconds from a life's start to the first tick an enemy is within 30 m, by the group the life began
//!   in (live median 14 to 15 s for a top's lives, 17 to 18 s for the others); lives that ended with no contact are counted apart.
//! - **the gang-up counters** (`an-gang-up.md` §5), for the stand-in (human or camper), the tops and, for deaths, the others: for a
//!   subject, the plays aimed at him (enemy bots whose assignment targets him) a tick on average, and the share of his alive time with
//!   3 or more enemies within 30 m; for each **arrival** (an enemy bot's first tick within 30 m of him) the arriving bot's assignment
//!   kind and what it is aimed at (him, an exit, an objective, another place, none), the source of its track of him (Seen, Heard,
//!   Told, Hud, Radar, Thief, none) and, exactly (the last tick each source was the track's, kept per bot and victim), which sources
//!   it had of him in the 10 s before; the PairUp arrivals aimed at a top per top death.
//! - **density:** mates and enemies within 30 m of a living bot, the mean over its alive ticks, by team: over the whole match and after
//!   the first minute (the live figures, 1.65 and 0.65, are from alive samples of whole matches; an-gang-up's bench comparison is
//!   "after the first minute", the first minute being denser).
//! - **shooters per victim:** distinct enemies who hit the dead in the 5 s before the death, mean and the share of 3 or more (does
//!   the reflex's target choice need a dog-pile term?).
//! - Distances are on the ground (XZ), as the gang-up analysis's bench numbers are.

use super::*;
use crate::v026g::bots::know::{CheatKind, Source};
use crate::v026g::bots::plays::{PlayKind, TargetRef, pair_up};

/// Within this of the dead, m, an enemy is near.
const NEAR_M: f32 = 30.0;
/// The window before a death, in ticks (10 s).
const WINDOW_TICKS: usize = (10_000 / TICK_MS) as usize;
/// Enemies near in the window that make a death crowded.
const CROWD: u32 = 3;
/// The shooters of a death are those who hit in this window, ms.
const SHOOTERS_MS: u32 = 5_000;
/// A source used by the track this recently before an arrival counts as held at it, ms.
const RECENT_MS: u32 = 10_000;
/// The density's "after the first minute", ms.
const FIRST_MINUTE_MS: u32 = 60_000;
/// The crowded window is sampled this often, in ticks (the live worlds are 1.25 s apart).
const SAMPLE_TICKS: usize = 25;

/// How a bot's track of a subject came to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Via {
    Seen,
    Heard,
    Told,
    Hud,
    Radar,
    Thief,
}

impl Via {
    const ALL: [Via; 6] = [Via::Seen, Via::Heard, Via::Told, Via::Hud, Via::Radar, Via::Thief];

    fn of(source: Source) -> Self {
        match source {
            Source::Seen => Via::Seen,
            Source::Heard => Via::Heard,
            Source::Told { .. } => Via::Told,
            Source::Hud => Via::Hud,
            Source::Cheat(CheatKind::Radar) => Via::Radar,
            Source::Cheat(CheatKind::Thief) => Via::Thief,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Via::Seen => "Seen",
            Via::Heard => "Heard",
            Via::Told => "Told",
            Via::Hud => "Hud",
            Via::Radar => "Radar",
            Via::Thief => "Thief",
        }
    }
}

/// What an arriving bot's assignment is aimed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Aim {
    /// The subject himself.
    Him,
    Exit,
    Objective,
    AnotherPlace,
    Nothing,
}

impl Aim {
    const ALL: [Aim; 5] = [Aim::Him, Aim::Exit, Aim::Objective, Aim::AnotherPlace, Aim::Nothing];

    fn name(self) -> &'static str {
        match self {
            Aim::Him => "him",
            Aim::Exit => "exit",
            Aim::Objective => "objective",
            Aim::AnotherPlace => "another place",
            Aim::Nothing => "none",
        }
    }

    /// What an assignment's target is, for a subject at `him`.
    fn of(target: Option<TargetRef>, him: Place) -> Self {
        match target {
            Some(TargetRef::Place(place)) if place == him => Aim::Him,
            Some(TargetRef::Exit(_)) => Aim::Exit,
            Some(TargetRef::Battery(_) | TargetRef::Container(_) | TargetRef::Spawner(_)) => Aim::Objective,
            // A threat spot (K2's job plays): a held spot's point, not the subject's place.
            Some(TargetRef::Place(_) | TargetRef::Point(_) | TargetRef::ThreatSpot(_)) => Aim::AnotherPlace,
            Some(TargetRef::None) | None => Aim::Nothing,
        }
    }
}

/// What a player is for these counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cat {
    /// The top bot of team `n`.
    Top(u8),
    /// Any other bot of team `n`.
    Rest(u8),
    /// The stand-in human.
    Human,
    /// The stand-in camper.
    Camper,
}

impl Cat {
    const ALL: [Cat; 6] = [Cat::Top(0), Cat::Top(1), Cat::Rest(0), Cat::Rest(1), Cat::Human, Cat::Camper];

    fn index(self) -> usize {
        match self {
            Cat::Top(team) => usize::from(team & 1),
            Cat::Rest(team) => 2 + usize::from(team & 1),
            Cat::Human => 4,
            Cat::Camper => 5,
        }
    }

    fn name(self) -> String {
        match self {
            Cat::Top(team) => format!("top of team {team}"),
            Cat::Rest(team) => format!("other bots of team {team}"),
            Cat::Human => "the stand-in human".to_owned(),
            Cat::Camper => "the stand-in camper".to_owned(),
        }
    }
}

/// The groups a player is counted in: his own (the human, the camper, the team's top, else another bot), and the team's top too when
/// the human or the camper leads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Members {
    own: Cat,
    also_top: Option<Cat>,
}

impl Members {
    fn iter(self) -> impl Iterator<Item = Cat> {
        std::iter::once(self.own).chain(self.also_top)
    }

    fn has(self, cat: Cat) -> bool {
        self.iter().any(|c| c == cat)
    }
}

/// A player's life, for the first contact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Life {
    #[default]
    Dead,
    /// Alive since `spawned`, in the groups `began` at the start, no enemy near yet.
    Alive { spawned: u32, began: Members },
    /// An enemy has come near.
    Met,
}

/// What is counted for a group of players.
#[derive(Default)]
struct Group {
    /// Ticks alive.
    alive_ticks: u64,
    deaths: u32,
    crowded: u32,
    /// Enemies within 30 m at the death, summed; distinct in the 10 s before, summed.
    near_at_death: u64,
    near_in_window: u64,
    /// Lives that began in this group: ms to the first contact, and lives that ended with none.
    contact_ms: Vec<u32>,
    no_contact: u32,
    /// Subjects only: plays aimed at him, summed over his alive ticks; ticks with 3 or more enemies near.
    aimed_sum: u64,
    crowd_ticks: u64,
    /// Distinct enemies who hit the dead in the 5 s before the death, summed; deaths with 3 or more.
    shooters_sum: u64,
    shooters_3: u32,
    arrivals: Arrivals,
}

/// The arrivals of enemy bots within 30 m of a subject.
#[derive(Default)]
struct Arrivals {
    total: u32,
    /// By the play kind of the arriving bot's assignment.
    by_kind: BTreeMap<String, u32>,
    /// By [`Aim::ALL`].
    by_aim: [u32; 5],
    /// By [`Via::ALL`], the source of its track of him at the entry; and arrivals with no track of him.
    by_source: [u32; 6],
    no_track: u32,
    /// By [`Via::ALL`]: arrivals that had the source as the track's within [`RECENT_MS`] before the entry.
    held_10s: [u32; 6],
    /// PairUp arrivals aimed at him.
    pair_up: u32,
}

/// What the tally knows of a bot, read each tick (a bot's own memory and assignment).
#[derive(Default, Clone)]
struct Mind {
    /// Its assignment's play and target.
    aim: Option<(PlayKind, TargetRef)>,
    /// Its tracks: who, source, and when sensed.
    tracks: Vec<(Place, Source, u32)>,
    /// By enemy: the last tick each of [`SOURCES`] was the track's source.
    last: Vec<(Place, [Option<u32>; 6])>,
}

#[derive(Default)]
pub(super) struct Feel {
    human: Option<Place>,
    camper: Option<Place>,
    groups: [Group; 6],
    life: Vec<Life>,
    /// The top of each team at the last tick.
    tops: [Option<Place>; 2],
    kills_seen: usize,
    /// By place: what the bot knows (none: not a bot).
    minds: Vec<Mind>,
    /// By place: the enemies within 30 m of a subject at the last tick (a set of places).
    inside: Vec<u64>,
    /// By team: mates and enemies within 30 m of a living bot, summed over its alive ticks, and the ticks: the whole match, and after
    /// its first minute.
    density: [[(u64, u64, u64); 2]; 2],
    /// The first tick the topic saw, and by place the tick the life began.
    first_tick: Option<u32>,
    spawned: Vec<u32>,
}

impl Feel {
    pub(super) fn set_human(&mut self, human: Option<Place>, camper: Option<Place>) {
        self.human = human;
        self.camper = camper;
    }

    fn grow(&mut self, places: usize) {
        if self.life.len() < places {
            self.life.resize(places, Life::Dead);
            self.minds.resize(places, Mind::default());
            self.inside.resize(places, 0);
            self.spawned.resize(places, 0);
        }
    }

    /// The groups a player is counted in.
    fn members(&self, place: Place, team: u8) -> Members {
        let top = (self.tops[usize::from(team & 1)] == Some(place)).then_some(Cat::Top(team));
        if self.human == Some(place) {
            Members { own: Cat::Human, also_top: top }
        } else if self.camper == Some(place) {
            Members { own: Cat::Camper, also_top: top }
        } else {
            Members { own: top.unwrap_or(Cat::Rest(team)), also_top: None }
        }
    }

    /// The team's top by PairUp's own rule (`TopRule`, its default numbers) from the game's kills and streaks, every body of the team
    /// ranked, the stand-ins too.
    pub(super) fn top_of(game: &Game, team: u8) -> Option<Place> {
        let players = game.bodies.iter().enumerate().filter(|(_, body)| body.team == team).map(|(place, _)| {
            let (kills, _, streak) = game.stats.get(place).copied().unwrap_or_default();
            (place, kills, streak)
        });
        pair_up::TopRule::default_numbers().top(players)
    }

    /// The enemies of `team` near `at` at record `idx` of the run, as a set of places.
    fn near(run: &Run, game: &Game, team: u8, at: Vec3, idx: usize) -> u64 {
        let mut set = 0u64;
        for (place, body) in game.bodies.iter().enumerate().take(64) {
            if body.team == team {
                continue;
            }
            if let Some(rec) = run.recs.get(place).and_then(|recs| recs.get(idx))
                && rec.alive
                && rec.feet.distance_xz(at) <= NEAR_M
            {
                set |= 1 << place;
            }
        }
        set
    }

    /// [the smoke topic] Whether the death of `victim` at its last record is crowded by [`Self::deaths_tick`]'s rule (3 or more
    /// distinct enemies within 30 m at some sample of the 10 s before, within the life), and how many are near at the death.
    pub(super) fn crowded_at_death(run: &Run, game: &Game, victim: Place) -> Option<(bool, u32)> {
        let team = game.bodies.get(victim)?.team;
        let recs = run.recs.get(victim)?;
        let last = recs.len().checked_sub(1)?;
        let now = Self::near(run, game, team, recs[last].feet, last);
        let mut window = 0u64;
        for back in (0..=WINDOW_TICKS.min(last)).step_by(SAMPLE_TICKS) {
            let idx = last - back;
            if back > 0 && !recs[idx].alive {
                break;
            }
            window |= Self::near(run, game, team, recs[idx].feet, idx);
        }
        Some((window.count_ones() >= CROWD, now.count_ones()))
    }

    /// The tick's deaths: crowded or not, and a note for a top's or the human's.
    fn deaths_tick(&mut self, run: &Run, game: &Game) {
        let new: Vec<(u32, Place, Place)> = run.kills[self.kills_seen.min(run.kills.len())..].to_vec();
        self.kills_seen = run.kills.len();
        for (t, _killer, victim) in new {
            let Some(body) = game.bodies.get(victim) else {
                continue;
            };
            let team = body.team;
            let Some(recs) = run.recs.get(victim) else {
                continue;
            };
            let Some(last) = recs.len().checked_sub(1) else {
                continue;
            };
            let members = self.members(victim, team);
            let at = recs[last].feet;
            let now = Self::near(run, game, team, at, last);
            // The window is sampled as the live worlds are (every 1.25 s back from the death) and starts at the life's spawn.
            let life_ticks = (t.saturating_sub(self.spawned[victim]) / TICK_MS) as usize;
            let mut window = 0u64;
            for back in (0..=WINDOW_TICKS.min(life_ticks).min(last)).step_by(SAMPLE_TICKS) {
                let idx = last - back;
                window |= Self::near(run, game, team, recs[idx].feet, idx);
            }
            let crowded = window.count_ones() >= CROWD;
            let mut shooters = 0u64;
            for &(hit_t, shooter, target) in run.hits.iter().rev().take_while(|&&(hit_t, ..)| t.saturating_sub(hit_t) <= SHOOTERS_MS) {
                if target == victim && hit_t <= t && game.bodies.get(shooter).is_some_and(|b| b.team != team) {
                    shooters |= 1 << shooter.min(63);
                }
            }
            for cat in members.iter() {
                let group = &mut self.groups[cat.index()];
                group.deaths += 1;
                group.crowded += u32::from(crowded);
                group.near_at_death += u64::from(now.count_ones());
                group.near_in_window += u64::from(window.count_ones());
                group.shooters_sum += u64::from(shooters.count_ones());
                group.shooters_3 += u32::from(shooters.count_ones() >= CROWD);
            }
        }
    }

    /// Each body's life: its first contact, once.
    fn lives_tick(&mut self, run: &Run, game: &Game, tick: u32) {
        for place in 0..game.bodies.len() {
            let body = &game.bodies[place];
            let Some(rec) = run.recs.get(place).and_then(|recs| recs.last()) else {
                continue;
            };
            let idx = run.recs[place].len() - 1;
            let members = self.members(place, body.team);
            let state = match (self.life[place], rec.alive) {
                (Life::Dead, true) => {
                    self.spawned[place] = tick;
                    Life::Alive { spawned: tick, began: members }
                }
                (Life::Alive { began, .. }, false) => {
                    // The life ended with no enemy near.
                    for cat in began.iter() {
                        self.groups[cat.index()].no_contact += 1;
                    }
                    Life::Dead
                }
                (Life::Met, false) => Life::Dead,
                (state, _) => state,
            };
            self.life[place] = state;
            if rec.alive {
                for cat in members.iter() {
                    self.groups[cat.index()].alive_ticks += 1;
                }
                if let Life::Alive { spawned, began } = state
                    && Self::near(run, game, body.team, rec.feet, idx) != 0
                {
                    for cat in began.iter() {
                        self.groups[cat.index()].contact_ms.push(tick.saturating_sub(spawned));
                    }
                    self.life[place] = Life::Met;
                }
            }
        }
    }
}

impl Mind {
    /// The bot's assignment and tracks now, and the tick each track's source was last seen at.
    fn read(&mut self, bot: Option<&Bot>, tick: u32) {
        let Some(bot) = bot else {
            *self = Mind::default();
            return;
        };
        self.aim = bot.assignment.as_ref().map(|a| (a.kind, a.target));
        self.tracks.clear();
        self.tracks.extend(bot.memory.tracks().iter().map(|track| (track.who, track.source, track.time)));
        for i in 0..self.tracks.len() {
            let (who, source, _) = self.tracks[i];
            self.stamp(who, source, tick);
        }
    }

    /// `source` was `who`'s track's source at `tick`.
    fn stamp(&mut self, who: Place, source: Source, tick: u32) {
        let at = match self.last.iter().position(|(w, _)| *w == who) {
            Some(at) => at,
            None => {
                self.last.push((who, [None; 6]));
                self.last.len() - 1
            }
        };
        self.last[at].1[Via::of(source) as usize] = Some(tick);
    }
}

impl Feel {
    /// Each tick: the subjects' aimed plays and crowded time, the arrivals within 30 m, and every living bot's density.
    fn subjects_tick(&mut self, run: &Run, game: &Game, tick: u32) {
        let alive: Vec<(Place, u8, Vec3)> = (0..game.bodies.len())
            .filter_map(|place| {
                let rec = run.recs.get(place).and_then(|recs| recs.last())?;
                rec.alive.then_some((place, game.bodies[place].team, rec.feet))
            })
            .collect();
        for &(place, team, feet) in &alive {
            let (mates, enemies) = alive.iter().filter(|o| o.0 != place && o.2.distance_xz(feet) <= NEAR_M).fold((0, 0), |(m, e), o| {
                if o.1 == team { (m + 1, e) } else { (m, e + 1) }
            });
            let first = *self.first_tick.get_or_insert(tick);
            for (k, after) in [(0, true), (1, tick.saturating_sub(first) >= FIRST_MINUTE_MS)] {
                if after {
                    let density = &mut self.density[k][usize::from(team & 1)];
                    *density = (density.0 + mates, density.1 + enemies, density.2 + 1);
                }
            }
        }
        for place in 0..game.bodies.len() {
            let Some(&(_, team, feet)) = alive.iter().find(|o| o.0 == place) else {
                self.inside[place] = 0;
                continue;
            };
            let members = self.members(place, team);
            if matches!(members.own, Cat::Rest(_)) {
                continue;
            }
            let idx = run.recs[place].len() - 1;
            let near = Self::near(run, game, team, feet, idx);
            let aimed = alive
                .iter()
                .filter(|o| o.1 != team && self.minds[o.0].aim.is_some_and(|(_, t)| t == TargetRef::Place(place)))
                .count();
            for cat in members.iter() {
                let group = &mut self.groups[cat.index()];
                group.aimed_sum += aimed as u64;
                group.crowd_ticks += u64::from(near.count_ones() >= CROWD);
            }
            let before = self.inside[place];
            for enemy in (0..game.bodies.len().min(64)).filter(|e| near & (1 << e) != 0 && before & (1 << e) == 0) {
                self.arrive(members, place, enemy, tick);
            }
            self.inside[place] = near;
        }
    }

    /// `enemy` came within 30 m of the subject `him` at `tick`.
    fn arrive(&mut self, members: Members, him: Place, enemy: Place, tick: u32) {
        let mind = &self.minds[enemy];
        let aim = Aim::of(mind.aim.map(|(_, target)| target), him);
        let kind = mind.aim.map_or_else(|| "none".to_owned(), |(kind, _)| format!("{kind:?}"));
        let via = mind.tracks.iter().find(|t| t.0 == him).map(|t| Via::of(t.1));
        let held: Vec<Via> = mind
            .last
            .iter()
            .find(|(who, _)| *who == him)
            .map(|(_, ticks)| Via::ALL.into_iter().filter(|&v| ticks[v as usize].is_some_and(|at| tick.saturating_sub(at) <= RECENT_MS)).collect())
            .unwrap_or_default();
        let pair_up = matches!(mind.aim, Some((PlayKind::PairUp, _))) && aim == Aim::Him;
        for cat in members.iter() {
            let arrivals = &mut self.groups[cat.index()].arrivals;
            arrivals.total += 1;
            *arrivals.by_kind.entry(kind.clone()).or_default() += 1;
            arrivals.by_aim[aim as usize] += 1;
            match via {
                Some(via) => arrivals.by_source[via as usize] += 1,
                None => arrivals.no_track += 1,
            }
            for &v in &held {
                arrivals.held_10s[v as usize] += 1;
            }
            arrivals.pair_up += u32::from(pair_up);
        }
    }
}

impl Topic for Feel {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn read(&mut self, place: Place, bot: Option<&Bot>, tick: u32) {
        self.grow(place + 1);
        self.minds[place].read(bot, tick);
    }

    #[cfg(test)]
    fn inject(&mut self, place: Place, now: Option<&BotNow>) {
        self.grow(place + 1);
        let mind = &mut self.minds[place];
        match now {
            Some(now) => {
                mind.aim = now.aim;
                mind.tracks = now.tracks.clone();
                for &(who, source, time) in &now.tracks {
                    mind.stamp(who, source, time);
                }
            }
            None => *mind = Mind::default(),
        }
    }

    fn end(&mut self, run: &Run, game: &mut Game, _sight: &mapdata::RayTree, _rays: &mut u64) {
        self.grow(game.bodies.len());
        // The categories of this tick are those of the last: a death keeps the standing he had.
        self.lives_tick(run, game, game.tick);
        self.subjects_tick(run, game, game.tick);
        self.deaths_tick(run, game);
        for team in 0..2u8 {
            self.tops[usize::from(team)] = Self::top_of(game, team);
        }
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        let mut json = serde_json::Map::new();
        let mut lines = vec!["owner-test feel (the live census's definitions; live in brackets)".to_owned()];
        for cat in Cat::ALL {
            let g = &self.groups[cat.index()];
            let subject = !matches!(cat, Cat::Rest(_));
            let minutes = g.alive_ticks as f64 * f64::from(TICK_MS) / 60_000.0;
            let per_min = if minutes > 0.0 { round3(f64::from(g.deaths) / minutes) } else { 0.0 };
            let share = if g.deaths > 0 { round1(100.0 * f64::from(g.crowded) / f64::from(g.deaths)) } else { 0.0 };
            let mean = |sum: u64| if g.deaths > 0 { round3(sum as f64 / f64::from(g.deaths)) } else { 0.0 };
            let ticks = g.alive_ticks as f64;
            let aimed_mean = if subject && ticks > 0.0 { round3(g.aimed_sum as f64 / ticks) } else { 0.0 };
            let alive_3plus = if subject && ticks > 0.0 { round1(100.0 * g.crowd_ticks as f64 / ticks) } else { 0.0 };
            let shooters_3 = if g.deaths > 0 { round1(100.0 * f64::from(g.shooters_3) / f64::from(g.deaths)) } else { 0.0 };
            let contact: Vec<Option<f64>> =
                [0.25, 0.5, 0.75].map(|p| quantile(&g.contact_ms, p).map(|ms| round1(f64::from(ms) / 1000.0))).to_vec();
            let a = &g.arrivals;
            let pair_up_per_death = if g.deaths > 0 { round3(f64::from(a.pair_up) / f64::from(g.deaths)) } else { 0.0 };
            let by_aim: BTreeMap<&str, u32> = Aim::ALL.iter().map(|aim| aim.name()).zip(a.by_aim).collect();
            let mut by_source: BTreeMap<&str, u32> = Via::ALL.iter().map(|via| via.name()).zip(a.by_source).collect();
            by_source.insert("none", a.no_track);
            let held: BTreeMap<&str, u32> = Via::ALL.iter().map(|via| via.name()).zip(a.held_10s).collect();
            lines.push(format!(
                "{}: alive {minutes:.1} min, deaths {} = {per_min} a minute (live 1.19 top, 0.81 not), crowded {} = {share} % (live top 41-45, not 27-29), enemies within 30 m at death {} / distinct in 10 s {} (live 1.1), shooters in 5 s {} (3+ {shooters_3} %), spawn to first contact p25/median/p75 s {contact:?} over {} lives, {} lives with none (live 14-15 top, 17-18 not)",
                cat.name(),
                g.deaths,
                g.crowded,
                mean(g.near_at_death),
                mean(g.near_in_window),
                mean(g.shooters_sum),
                g.contact_ms.len(),
                g.no_contact,
            ));
            if subject {
                lines.push(format!(
                    "  gangup {}: plays aimed at him {aimed_mean} a tick, 3+ enemies near {alive_3plus} % of his alive time; {} arrivals: aimed at {by_aim:?}, track of him {by_source:?}, source held in the 10 s before {held:?}, by play {:?}; PairUp aimed at him {} = {pair_up_per_death} a death (guard <= 1)",
                    cat.name(),
                    a.total,
                    a.by_kind,
                    a.pair_up,
                ));
            }
            json.insert(
                cat.name(),
                serde_json::json!({
                    "alive_min": round3(minutes),
                    "deaths": g.deaths,
                    "deaths_per_min": per_min,
                    "crowded": g.crowded,
                    "crowded_pct": share,
                    "enemies_30m_at_death": mean(g.near_at_death),
                    "enemies_30m_in_10s": mean(g.near_in_window),
                    "shooters_5s_mean": mean(g.shooters_sum),
                    "shooters_5s_3plus_pct": shooters_3,
                    "contact_s_p25_median_p75": contact,
                    "contact_lives": g.contact_ms.len(),
                    "no_contact_lives": g.no_contact,
                    "aimed_plays_mean": aimed_mean,
                    "alive_3plus_near_pct": alive_3plus,
                    "arrivals": { "total": a.total, "aimed_at": by_aim, "track_source": by_source, "held_10s": held, "by_play": a.by_kind, "pair_up_on_him": a.pair_up, "pair_up_per_death": pair_up_per_death },
                }),
            );
        }
        let density = |k: usize, team: usize, mates: bool| {
            let (m, e, n) = self.density[k][team];
            if n > 0 { round3((if mates { m } else { e }) as f64 / n as f64) } else { 0.0 }
        };
        let (mates, enemies) = ([density(0, 0, true), density(0, 1, true)], [density(0, 0, false), density(0, 1, false)]);
        let (mates_late, enemies_late) = ([density(1, 0, true), density(1, 1, true)], [density(1, 0, false), density(1, 1, false)]);
        lines.push(format!(
            "density within 30 m of a living bot, by team: mates {mates:?} (live 1.65), enemies {enemies:?} (live 0.65); after the first minute: mates {mates_late:?}, enemies {enemies_late:?}"
        ));
        json.insert(
            "density".to_owned(),
            serde_json::json!({ "mates_30m": mates, "enemies_30m": enemies, "mates_30m_after_1min": mates_late, "enemies_30m_after_1min": enemies_late }),
        );
        vec![("feel", Part::new(serde_json::json!({ "feel": json }), lines.join("\n  ")))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body of team 0 with kills and a streak that stand out is the team's top; two close ones are none.
    #[test]
    fn a_top_stands_out_by_kills_and_streak_from_the_next() {
        let (mut game, _) = made_up(&[0, 0, 0, 1]);
        game.stats[0] = (6, 0, 2); // 6 + 4 = 10
        game.stats[1] = (6, 0, 0); // 6: lead 4
        assert_eq!(Feel::top_of(&game, 0), Some(0));
        game.stats[1] = (8, 0, 0); // lead 2: no top
        assert_eq!(Feel::top_of(&game, 0), None);
        game.stats[0] = (3, 0, 2); // 7: under the minimum
        game.stats[1] = (0, 0, 0);
        assert_eq!(Feel::top_of(&game, 0), None);
    }

    /// Three enemies within 30 m of the dead in the 10 s before make a death crowded; two do not; the first contact is timed from
    /// the spawn.
    #[test]
    fn a_death_with_three_enemies_near_is_crowded_and_a_contact_is_timed() {
        let (mut game, mut tally) = made_up(&[0, 1, 1, 1, 1]);
        // Body 0 (team 0) at the origin; enemies 1-3 come within 30 m at 2 s, body 4 stays far.
        for k in 0..60u32 {
            game.tick = (k + 1) * TICK_MS;
            put(&mut game, 0, 10.0, 10.0, 0);
            for place in 1..=3 {
                let near = k >= 40;
                put(&mut game, place, if near { 20.0 } else { 70.0 }, 10.0 + place as f32, 0);
            }
            put(&mut game, 4, 75.0, 30.0, 0);
            tally.step_with(&mut game, &[None, None, None, None, None]);
        }
        game.tick += TICK_MS;
        game.bodies[0].alive = false;
        game.kill_log.push((game.tick, 1, 0));
        tally.step_with(&mut game, &[None, None, None, None, None]);
        let (text, json) = tally.finish(&game);
        let rest = &json["feel"]["other bots of team 0"];
        assert_eq!((rest["deaths"].as_u64(), rest["crowded"].as_u64()), (Some(1), Some(1)), "{text}");
        // Body 0 met an enemy 2 s after it spawned (the tick of the first record).
        let contact = rest["contact_s_p25_median_p75"][1].as_f64().unwrap_or(f64::NAN);
        assert!((contact - 2.0).abs() < 0.2, "{contact}");
    }

    /// An enemy bot's first tick within 30 m of the stand-in is an arrival, with its play, what it was aimed at and the sources its
    /// track of him had; plays aimed at him are counted every tick.
    #[test]
    fn an_arrival_is_counted_with_its_aim_and_the_sources_it_held() {
        let (mut game, mut tally) = made_up(&[0, 1, 1]);
        tally.topic::<Feel>().set_human(Some(0), None);
        for k in 0..40u32 {
            game.tick = (k + 1) * TICK_MS;
            put(&mut game, 0, 10.0, 10.0, 0);
            put(&mut game, 1, if k >= 20 { 20.0 } else { 70.0 }, 10.0, 0);
            put(&mut game, 2, 75.0, 30.0, 0);
            let bot = BotNow {
                aim: Some((PlayKind::PairUp, TargetRef::Place(0))),
                tracks: vec![(0, Source::Told { by: 2 }, game.tick)],
                ..BotNow::default()
            };
            tally.step_with(&mut game, &[None, Some(bot), None]);
        }
        let (_, json) = tally.finish(&game);
        let human = &json["feel"]["the stand-in human"];
        let arrivals = &human["arrivals"];
        assert_eq!(arrivals["total"], 1, "{human}");
        assert_eq!((arrivals["aimed_at"]["him"].as_u64(), arrivals["track_source"]["Told"].as_u64()), (Some(1), Some(1)));
        assert_eq!((arrivals["held_10s"]["Told"].as_u64(), arrivals["held_10s"]["Radar"].as_u64()), (Some(1), Some(0)));
        assert_eq!((arrivals["by_play"]["PairUp"].as_u64(), arrivals["pair_up_on_him"].as_u64()), (Some(1), Some(1)));
        assert_eq!(human["aimed_plays_mean"], 1.0);
    }

    /// The stand-in human is ranked like a bot: when he leads he is the team's top and is counted in both groups; the camper the same.
    #[test]
    fn the_stand_in_human_can_be_the_top() {
        let (mut game, _) = made_up(&[0, 0, 1]);
        game.stats[0] = (9, 0, 0);
        assert_eq!(Feel::top_of(&game, 0), Some(0), "the rule names him whoever he is");
        let mut feel = Feel::default();
        feel.set_human(Some(0), None);
        feel.tops[0] = Some(0);
        let members = feel.members(0, 0);
        assert_eq!((members.own, members.also_top), (Cat::Human, Some(Cat::Top(0))));
        assert!(members.has(Cat::Top(0)) && members.has(Cat::Human));
        assert_eq!(feel.members(1, 0).own, Cat::Rest(0));
    }
}

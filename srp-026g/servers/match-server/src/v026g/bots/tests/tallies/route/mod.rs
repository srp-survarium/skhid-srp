//! The bench's count of how the bots got about (`player_bots_stuck.md` §8), as the audit's `wallpush.py` and
//! `movement.py` count it from a match's worlds: sampled once a world (1 s), and the carriers each tick.
//!
//! The tally is a list of topics ([`RouteTopic`], [`topics`]), each a type in its own file that owns its counts and
//! gives its part of the `routes:` line: a new count is a file, its `mod` line and one line of the list. What every
//! topic reads of the bodies is read once a tick into [`RouteRun`] and [`TickAt`] (a fall, a carry). The line is the
//! topics' pieces in the order of the list, in the three places the line has always had: the main run of keys, the
//! route endings after it, and the tail (a new count goes to the tail, so the key it adds is the last on the line).

use super::*;

mod carries;
mod counters;
mod exact;
mod falls;
mod fields;
mod pockets;
mod probe;
mod reversal_roles;
mod stuck;
mod walking;
mod zones;

use counters::*;

/// What the topics share of the run: each body's feet at the last tick and its team.
pub(super) struct RouteRun {
    pub(super) feet: Vec<Option<Vec3>>,
    pub(super) teams: Vec<Option<u8>>,
    /// Each body's walk at the last tick to an exact spot (`Motor::exact_end`).
    exact_was: Vec<bool>,
    /// Each body's pickup (the tick, the battery) while it carries.
    carry_from: Vec<Option<(u32, usize)>>,
}

/// A fall of a metre or more in a tick: where it began, and whether the bot's route drops there (a corner over
/// a metre under the one before, within a metre of where it landed) and was walked to an exact spot.
#[derive(Clone, Copy, Debug)]
pub(super) struct Fall {
    pub(super) from: Vec3,
    pub(super) planned: bool,
    pub(super) exact: bool,
}

/// What a body's carry did this tick: began, or ended (`home`: delivered; else lost) after `ms`.
#[derive(Clone, Copy, Debug)]
pub(super) enum CarryEvent {
    Began,
    Ended { ms: u32, home: bool },
}

/// One body at one tick, for the topics' [`RouteTopic::tick`].
pub(super) struct TickAt<'a> {
    pub(super) bots: &'a Bots,
    pub(super) time: u32,
    pub(super) place: usize,
    pub(super) body: &'a Body,
    /// The body's feet at the tick before, if it was alive.
    pub(super) before: Option<Vec3>,
    pub(super) fall: Option<Fall>,
    pub(super) carry: Option<CarryEvent>,
    /// The anomaly field on the body's floor, and whether it hurts (`standin_fields`).
    pub(super) field: Option<(crate::v026g::bots::live_zones::FieldId, bool)>,
}

/// The `routes:` line in its three places ([`RouteTally::line`]); a piece is `key value` pairs.
#[derive(Default)]
pub(super) struct Line {
    pub(super) main: Vec<String>,
    pub(super) ends: Vec<String>,
    pub(super) tail: Vec<String>,
}

/// A topic of the route tally: its counts, what it reads each tick, each world and each note, and its pieces of
/// the line.
pub(super) trait RouteTopic {
    /// One body at one tick.
    fn tick(&mut self, _run: &RouteRun, _at: &TickAt<'_>) {}

    /// The worlds of the match, once a second: every body.
    fn world(&mut self, _run: &RouteRun, _game: &Game, _bots: &Bots) {}

    /// A bot's note.
    fn note(&mut self, _run: &RouteRun, _note: &Note) {}

    /// The match is over: what the topic reads of the game's end.
    fn finish(&mut self, _game: &Game) {}

    /// The topic's pieces of the line at the end, and any lines of its own it prints.
    fn line(&mut self, run: &RouteRun, line: &mut Line);
}

/// The topics, in the order of the line's keys (a new one goes where its keys should come; most go last).
fn topics(bodies: usize) -> Vec<Box<dyn RouteTopic>> {
    vec![
        Box::new(walking::Walking::new(bodies)),
        Box::new(stuck::Stuck::default()),
        Box::new(carries::Carries::default()),
        no_route(),
        Box::new(falls::Falls::default()),
        Box::new(zones::Zones::new(bodies)),
        escapes(),
        Box::new(pockets::Pockets::new(bodies)),
        Box::new(exact::ExactFalls::new(bodies)),
        motor(),
        side_slides(),
        request_gaps(),
        shots_heard(),
        holder_stand_sight(),
        Box::new(reversal_roles::ReversalRoles::new(bodies)),
        below_walk(),
        rests(),
        rest_stops(),
        sprint_jumps(),
        landings(),
        stranded(),
        foliage(),
        self_kills(),
        fall_locks(),
        asks(),
        route_ends(),
        Box::new(probe::Probe::new(bodies)),
        Box::new(fields::Fields::new(bodies)),
    ]
}

/// The tally of the bench's run.
pub(crate) struct RouteTally {
    run: RouteRun,
    topics: Vec<Box<dyn RouteTopic>>,
}

impl RouteTally {
    pub(crate) fn start(bodies: usize) -> Self {
        Self {
            run: RouteRun {
                feet: vec![None; bodies],
                teams: vec![None; bodies],
                exact_was: vec![false; bodies],
                carry_from: vec![None; bodies],
            },
            topics: topics(bodies),
        }
    }

    /// A tick: the falls, the carries begun and ended.
    pub(crate) fn tick(&mut self, game: &Game, bots: &Bots, time: u32) {
        for (place, body) in game.bodies.iter().enumerate() {
            let run = &mut self.run;
            let bot = bots.bots[place].as_ref();
            if let Some(bot) = bot {
                run.teams[place] = Some(bot.team);
            }
            let before = std::mem::replace(&mut run.feet[place], body.alive.then_some(body.feet));
            let exact_now = bot.is_some_and(|bot| bot.motor.exact_end.is_some());
            let exact = std::mem::replace(&mut run.exact_was[place], exact_now) || exact_now;
            let fall = before.filter(|from| body.alive && from.y - body.feet.y >= 1.0).map(|from| {
                // Its route drops there: a corner over a metre under the one before, within a metre of where
                // it landed.
                let planned = bot.and_then(|bot| bot.route.as_ref()).is_some_and(|(route, _)| {
                    route.points().windows(2).any(|w| {
                        w[0].at.y - w[1].at.y >= 1.0 && w[1].at.distance_xz(body.feet) <= 1.0
                    })
                });
                Fall { from, planned, exact }
            });
            let carry = match (run.carry_from[place], body.carrying) {
                (None, Some(battery)) => {
                    run.carry_from[place] = Some((time, battery));
                    Some(CarryEvent::Began)
                }
                (Some((from, battery)), None) => {
                    run.carry_from[place] = None;
                    Some(CarryEvent::Ended {
                        ms: time - from,
                        home: matches!(game.batteries.get(battery), Some(Lying::Home(_))),
                    })
                }
                _ => None,
            };
            let field = (game.fields.on() && body.alive)
                .then(|| game.world.grid.locate(body.feet, 1.0))
                .flatten()
                .and_then(|node| game.fields.here(node));
            let at = TickAt { bots, time, place, body, before, fall, carry, field };
            for topic in &mut self.topics {
                topic.tick(&self.run, &at);
            }
        }
    }

    /// A world: the runs of pushing and standing, the reversals, and the bots on zones' floors.
    pub(crate) fn world(&mut self, game: &Game, bots: &Bots) {
        for topic in &mut self.topics {
            topic.world(&self.run, game, bots);
        }
    }

    pub(crate) fn note(&mut self, note: &Note) {
        for topic in &mut self.topics {
            topic.note(&self.run, note);
        }
    }

    /// The match is over.
    pub(crate) fn finish(&mut self, game: &Game) {
        for topic in &mut self.topics {
            topic.finish(game);
        }
    }

    /// The line the bench prints (one `routes:` line, `key value` pairs).
    pub(crate) fn line(mut self) -> String {
        let mut line = Line::default();
        for topic in &mut self.topics {
            topic.line(&self.run, &mut line);
        }
        let mut out = line.main.join(" ");
        for piece in line.ends.iter().chain(&line.tail) {
            out.push(' ');
            out += piece;
        }
        out
    }
}

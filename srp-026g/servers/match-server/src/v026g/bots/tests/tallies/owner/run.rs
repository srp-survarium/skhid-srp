//! What every topic of the owner counts reads and none owns: the record of every body at every tick
//! ([`Rec`]), the bots' plays, and the events the bots and the game gave that more than one topic reads
//! (the jump presses, the stuck notes). The tally writes it ([`Run::push`], [`OwnerTally::note`]); a
//! topic ([`Topic`]) reads it and keeps its own counts.

use super::*;

/// What a bot was doing at a tick, as the record needs it: its play and step (the step once a second), how far
/// its goal is, and the jump it pressed (why, and the route's take-off and landing for a route jump).
#[derive(Clone, Debug, Default)]
pub(super) struct Doing {
    pub(super) play: String,
    pub(super) step: Option<String>,
    pub(super) goal_m: Option<f32>,
    pub(super) jump: Option<(&'static str, Option<(Vec3, Vec3)>)>,
    /// Where the reflex's hide from an outranging enemy is ([`crate::v026g::bots::reflex::Reflex::cover_state`]).
    pub(super) cover: &'static str,
    /// The route's length left past the jump's landing, and the route goal's distance from that landing (the FWDREL probe).
    pub(super) after_landing: Option<(f32, f32)>,
    pub(super) walk: u8,
    pub(super) why: u16,
    pub(super) has_route: bool,
}

impl Doing {
    /// What `bot` (at `place`) is doing: its play, its step (when `sample`), how far its goal is from its body,
    /// and the jump it is watching.
    pub(super) fn of(bot: &Bot, game: &Game, place: Place, sample: bool) -> Self {
        let tick = game.tick;
        let feet = game.bodies[place].feet;
        let watch = bot.jump_watch.filter(|w| w.at.abs_diff(tick) <= TICK_MS * 2);
        Self {
            play: bot
                .assignment
                .as_ref()
                .map_or("none".to_owned(), |a| format!("{:?}", a.kind)),
            step: sample.then(|| bot.running.as_ref().map_or_else(String::new, |r| r.step_name())),
            goal_m: match bot.intent.go {
                Go::To { at, .. } => Some(feet.distance_xz(at)),
                Go::Stay => None,
            },
            jump: watch.map(|w| (w.by, w.route.map(|r| (r.take_off, r.landing)))),
            cover: bot.reflex.cover_state(),
            walk: bot.bench_walk,
            why: bot.bench_why,
            has_route: bot.route.is_some(),
            after_landing: watch.and_then(|w| w.route).and_then(|jump| {
                bot.route.as_ref().map(|(route, goal)| {
                    let mut r = route.clone();
                    let (walked, _) = r.progress(jump.landing);
                    ((r.length() - walked).max(0.0), goal.distance_xz(jump.landing))
                })
            }),
        }
    }
}

/// What a bot's note says, read once (the tally's boundary) for the topics: a take and its play and target,
/// a stuck watch, a play ended for no way to its goal, a grenade thrown (its aim and blast time) or held back
/// (and why). The first that fits, in this order.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum NoteKind<'a> {
    Takes { play: &'a str, target: &'a str },
    Stuck,
    NoWay,
    Thrown { aim: Vec3, blast: u32 },
    HeldBack(&'a str),
    Other,
}

impl<'a> NoteKind<'a> {
    pub(super) fn of(text: &'a str) -> Self {
        if let Some(rest) = text.strip_prefix("takes ") {
            let mut words = rest.split_whitespace();
            Self::Takes { play: words.next().unwrap_or(""), target: words.next().unwrap_or("") }
        } else if text.starts_with("is stuck at") {
            Self::Stuck
        } else if text.contains("no way to its goal") {
            Self::NoWay
        } else if let Some((aim, blast)) = thrown_at(text) {
            Self::Thrown { aim, blast }
        } else if let Some(why) = held_back(text) {
            Self::HeldBack(why)
        } else {
            Self::Other
        }
    }
}

/// The run so far: see the module.
#[derive(Default)]
pub(super) struct Run {
    pub(super) first_tick: u32,
    pub(super) recs: Vec<Vec<Rec>>,
    pub(super) is_bot: Vec<bool>,
    pub(super) plays: Vec<String>,
    pub(super) play_index: BTreeMap<String, u16>,
    /// By place: in a designed hold, in a step with a goal (the last step read).
    pub(super) status: Vec<(bool, bool)>,
    /// The bots' jump presses (`jumps.rs` reads why they were pressed, `moving.rs` which freed a body).
    pub(super) presses: Vec<Press>,
    /// By place: the times of the bots' "is stuck at" notes (`moving.rs` and `jumps.rs`).
    pub(super) stuck_notes: Vec<Vec<u32>>,
    /// The bench's stand-in camper (`SRP_BENCH_CAMPER`): its counts are also given apart.
    pub(super) camper: Option<Place>,
    /// Every hit on an enemy and every kill of the match, as the game logged them (the bench drains the game's
    /// logs): (tick, shooter, victim).
    pub(super) hits: Vec<(u32, Place, Place)>,
    pub(super) kills: Vec<(u32, Place, Place)>,
    kills_seen: usize,
    last_kill: Option<(u32, Place, Place)>,
}

impl Run {
    pub(super) fn new(n: usize) -> Self {
        Self {
            recs: vec![Vec::new(); n],
            is_bot: vec![false; n],
            status: vec![(false, false); n],
            stuck_notes: vec![Vec::new(); n],
            ..Self::default()
        }
    }

    /// The game's logs of this tick: its hits, and the kills made since the last read.
    pub(super) fn read_game(&mut self, game: &Game) {
        self.hits.extend(game.hit_log.iter().copied());
        if self.kills_seen > 0 && game.kill_log.get(self.kills_seen - 1) != self.last_kill.as_ref() {
            self.kills_seen = 0;
        }
        self.kills.extend(game.kill_log[self.kills_seen.min(game.kill_log.len())..].iter().copied());
        self.kills_seen = game.kill_log.len();
        self.last_kill = game.kill_log.last().copied();
    }

    fn play_id(&mut self, name: String) -> u16 {
        if let Some(&id) = self.play_index.get(&name) {
            return id;
        }
        let id = self.plays.len() as u16;
        self.plays.push(name.clone());
        self.play_index.insert(name, id);
        id
    }

    /// The record index of `tick`.
    pub(super) fn index(&self, tick: u32) -> usize {
        (tick.saturating_sub(self.first_tick) / TICK_MS) as usize
    }

    /// The tick of record index `k`, ms.
    pub(super) fn time_of(&self, k: usize) -> u32 {
        self.first_tick + k as u32 * TICK_MS
    }

    /// The bots' jump presses by place and time.
    pub(super) fn sorted_presses(&self) -> Vec<Press> {
        let mut presses = self.presses.clone();
        presses.sort_by_key(|p| (p.place, p.t));
        presses
    }

    /// The bots' minutes: the ticks bots were alive, in minutes.
    pub(super) fn bot_min(&self) -> f64 {
        let ticks: u64 = (0..self.recs.len())
            .filter(|&p| self.is_bot[p])
            .map(|p| self.recs[p].iter().filter(|r| r.alive).count() as u64)
            .sum();
        ticks as f64 * f64::from(TICK_S) / 60.0
    }

    /// `place`'s body at `tick` is recorded: the bot's `doing` makes its status, and a jump key pressed this tick
    /// is a press. Gives the record just made, and the one before.
    pub(super) fn push(&mut self, game: &Game, place: Place, doing: Option<&Doing>) -> (Rec, Rec) {
        let tick = game.tick;
        let body = &game.bodies[place];
        let prev = self.recs[place].last().copied().unwrap_or_default();
        if let Some(doing) = doing {
            self.is_bot[place] = true;
            if let Some(step) = &doing.step {
                self.status[place] = status_of(&doing.play, step, doing.goal_m);
            }
        }
        let enemy_near = game.bodies.iter().any(|b| {
            b.alive && b.team != body.team && b.feet.distance_xz(body.feet) <= CALM_ENEMY_M
        });
        let play = self.play_id(doing.map_or("human".to_owned(), |d| d.play.clone()));
        let (hold, travel) = self.status[place];
        let rec = Rec {
            alive: body.alive,
            keys: body.keys,
            feet: body.feet,
            yaw: body.yaw,
            pitch: body.view_elevation(),
            speed: body.step_m / TICK_S,
            calm: body.keys & (key::AIM | key::FIRE) == 0 && !enemy_near,
            carrying: body.carrying.is_some(),
            air: body.flight.is_some() || body.air_until > tick,
            play,
            hold,
            travel,
            past_band: target_past_band(game, place, body),
            walk: doing.map_or(0, |d| d.walk),
            why: doing.map_or(0, |d| d.why),
            has_route: doing.is_some_and(|d| d.has_route),
        };
        self.recs[place].push(rec);
        // A bot's jump pressed: why, from its watch of it.
        if let Some(doing) = doing
            && body.alive
            && body.keys & key::JUMP != 0
            && prev.keys & key::JUMP == 0
        {
            self.presses.push(Press {
                place,
                t: tick,
                feet: body.feet,
                by: doing.jump.map_or("other", |(by, _)| by),
                route: doing.jump.and_then(|(_, route)| route),
                carrying: rec.carrying,
                cover: doing.cover,
                after_landing: doing.after_landing,
            });
        }
        (rec, prev)
    }
}

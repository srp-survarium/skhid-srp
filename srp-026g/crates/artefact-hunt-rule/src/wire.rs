//! The state's bytes in a world (the rules blob, little-endian).
//!
//! | size | field | domain |
//! |---|---|---|
//! | u32 | `m_random` | any |
//! | u8 x2 | `m_score[team]` | 0..=goal |
//! | u32 | `m_round_starts` | the rounds started after the first: see below |
//! | u32 x2 | `m_sides[team]` | roster places (bits 0..19), no place in both |
//! | u8 | `m_stage` | 0 idle (the scores 0), 1 play, 2 over |
//! | | **over** | |
//! | u32 | `m_over.at` | a time |
//! | u8 | `m_over.reason` | 0 collected, 1 time out, 2 the other team wiped in sudden death |
//! | | **play** | |
//! | u32 | `m_start` | a time |
//! | u8 | `m_round` | 2 waiting, 3 spawned, 4 carried, 5 dropped, 6 brought, 7 fade, 8 countdown (0.46c2's state ids) |
//! | u8 | `m_clock` | not for 6: 0 regular, 1 sudden death |
//! | u32 | `m_sudden.from` | only for 1: a time |
//! | u32 | `m_sudden.fallen` | only for 1 and not for 7: places of the sides |
//! | u8 | `m_spawner` | only for 3: below the map's spawners |
//! | i32 x3 | `m_dropped` | only for 5: mm, each within 100 km |
//! | u8 | `m_carrier` | only for 4: a place of the sides, not fallen |
//! | u32 | `m_until` | not for 4: a time |
//! | u32 x2 | `m_way.left[team]` | only for 3, 4, 5: 0..=65536 |
//! | u32 x20 | `m_way.carried[place]` | only for 3, 4, 5: 0..=128 x 65536 |
//! | u8 | `m_size` | the bytes before it |
//!
//! A time is the match's start up to its horizon ([`Params`]': the match,
//! two round restarts, sudden death, an artefact's longest wait or life and
//! a minute). Across the fields: a regular clock outside 6 has both scores
//! below the goal; sudden death has the scores at most 1 apart, and with
//! equal scores nobody's team stands wiped (its point is given at once).
//! `m_round_starts` is the points less one in 6 and 7 (the delivery's round
//! is still to start) and less one more for sudden death's wiped team's
//! point (unequal scores in sudden death), 0 idle; over, the points less one
//! after the goal or a late lead (collected) and after the wiped team's point
//! (wiped), the points to two less at a time out (a hunt timed out, a late
//! delivery, sudden death's delivery, with a wiped point in its step at most).
//! The last byte, `m_size`, counts the bytes before it: the rules after this
//! one are read from the section's end, and it says where this one starts
//! ([`Rule::read_back`]). With it 20 bytes idle, 25 over, 29 to 138 in play.

use core::ffi::CStr;

use crate::geometry::Position;
use crate::params::Params;
use crate::roster::{PLAYERS, Place, Roster, Sides, Team};
use crate::rule::{Artefact, Clock, Over, Play, Reason, Rng, Round, Rule, Spot, Stage, Sudden};
use crate::time::Time;
use crate::way::Way;

/// The most bytes a state takes: an artefact dropped in sudden death.
pub const MAX_BYTES: usize = 15 + 4 + 4 + 1 + 9 + 12 + 4 + 8 + 4 * PLAYERS + 1;

/// A field's value as the game's writers take it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    U8(u8),
    U32(u32),
}

/// A field of the state, in the order of the wire: its name for the game's
/// hash-mismatch dump, and its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Field {
    pub name: &'static CStr,
    pub value: Value,
}

/// Why bytes are no state of this map's rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    Short,
    Score,
    /// The round starts are not what the scores and the stage give.
    RoundStarts,
    Sides,
    Stage,
    Reason,
    Round,
    Clock,
    /// A time before the match's start or past its horizon.
    Time,
    Spawner,
    Position,
    Carrier,
    Fallen,
    Way,
    /// A team with nobody left in sudden death and equal scores: its point
    /// is given at once, the state never stands.
    Wiped,
    /// The last byte is not the count of the bytes before it.
    Size,
}

const CARRIED: [&CStr; PLAYERS] = [
    c"m_way.carried[0]",
    c"m_way.carried[1]",
    c"m_way.carried[2]",
    c"m_way.carried[3]",
    c"m_way.carried[4]",
    c"m_way.carried[5]",
    c"m_way.carried[6]",
    c"m_way.carried[7]",
    c"m_way.carried[8]",
    c"m_way.carried[9]",
    c"m_way.carried[10]",
    c"m_way.carried[11]",
    c"m_way.carried[12]",
    c"m_way.carried[13]",
    c"m_way.carried[14]",
    c"m_way.carried[15]",
    c"m_way.carried[16]",
    c"m_way.carried[17]",
    c"m_way.carried[18]",
    c"m_way.carried[19]",
];

const IDLE: u8 = 0;
const PLAY: u8 = 1;
const OVER: u8 = 2;

const WAITING: u8 = 2;
const SPAWNED: u8 = 3;
const CARRIED_BY: u8 = 4;
const DROPPED: u8 = 5;
const BROUGHT: u8 = 6;
const FADE: u8 = 7;
const COUNTDOWN: u8 = 8;

const REGULAR: u8 = 0;
const SUDDEN: u8 = 1;

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn u8(&mut self) -> Result<u8, WireError> {
        let (&byte, rest) = self.0.split_first().ok_or(WireError::Short)?;
        self.0 = rest;
        Ok(byte)
    }

    fn u32(&mut self) -> Result<u32, WireError> {
        let (bytes, rest) = self.0.split_first_chunk::<4>().ok_or(WireError::Short)?;
        self.0 = rest;
        Ok(u32::from_le_bytes(*bytes))
    }

    fn time(&mut self) -> Result<Time, WireError> {
        self.u32().map(Time::from_ms)
    }

    fn roster(&mut self, error: WireError) -> Result<Roster, WireError> {
        Roster::from_bits(self.u32()?).ok_or(error)
    }

    fn way(&mut self) -> Result<Way, WireError> {
        let left = [self.u32()?, self.u32()?];
        let mut carried = [0; PLAYERS];
        for part in &mut carried {
            *part = self.u32()?;
        }
        Way::from_parts(left, carried).ok_or(WireError::Way)
    }
}

fn u8(put: &mut dyn FnMut(Field), name: &'static CStr, value: u8) {
    put(Field {
        name,
        value: Value::U8(value),
    });
}

fn u32(put: &mut dyn FnMut(Field), name: &'static CStr, value: u32) {
    put(Field {
        name,
        value: Value::U32(value),
    });
}

fn way_fields(put: &mut dyn FnMut(Field), way: &Way) {
    u32(put, c"m_way.left[0]", way.left[0]);
    u32(put, c"m_way.left[1]", way.left[1]);
    for (name, &part) in CARRIED.iter().zip(&way.carried) {
        u32(put, name, part);
    }
}

/// A clock with its fallen (the hunt, the countdown).
fn clock_fields(put: &mut dyn FnMut(Field), clock: &Clock) {
    match clock {
        Clock::Regular => u8(put, c"m_clock", REGULAR),
        Clock::Sudden(sudden) => {
            u8(put, c"m_clock", SUDDEN);
            u32(put, c"m_sudden.from", sudden.from.ms());
            u32(put, c"m_sudden.fallen", sudden.fallen.bits());
        }
    }
}

impl Rule {
    /// The state's fields in the wire's order: the one definition of the
    /// layout ([`Rule::write`] and the game's serialize both walk it).
    pub fn fields(&self, put: &mut dyn FnMut(Field)) {
        let mut size = 0u8;
        self.body(&mut |field| {
            size += match field.value {
                Value::U8(_) => 1,
                Value::U32(_) => 4,
            };
        });
        self.body(put);
        u8(put, c"m_size", size);
    }

    fn body(&self, put: &mut dyn FnMut(Field)) {
        u32(put, c"m_random", self.rng.state());
        u8(put, c"m_score[0]", self.score[0]);
        u8(put, c"m_score[1]", self.score[1]);
        u32(put, c"m_round_starts", u32::from(self.round_starts));
        u32(put, c"m_sides[0]", self.sides.of(Team::Zero).bits());
        u32(put, c"m_sides[1]", self.sides.of(Team::One).bits());
        let Play { start, round } = match self.stage {
            Stage::Idle => return u8(put, c"m_stage", IDLE),
            Stage::Over(over) => {
                u8(put, c"m_stage", OVER);
                u32(put, c"m_over.at", over.at.ms());
                return u8(put, c"m_over.reason", over.reason as u8);
            }
            Stage::Play(play) => play,
        };
        u8(put, c"m_stage", PLAY);
        u32(put, c"m_start", start.ms());
        match round {
            Round::Hunt { clock, artefact } => {
                let id = match artefact {
                    Artefact::Waiting { .. } => WAITING,
                    Artefact::Lying {
                        spot: Spot::Spawner(_),
                        ..
                    } => SPAWNED,
                    Artefact::Lying {
                        spot: Spot::Dropped(_),
                        ..
                    } => DROPPED,
                    Artefact::Carried { .. } => CARRIED_BY,
                };
                u8(put, c"m_round", id);
                clock_fields(put, &clock);
                match artefact {
                    Artefact::Waiting { until } => u32(put, c"m_until", until.ms()),
                    Artefact::Lying { spot, until, way } => {
                        match spot {
                            Spot::Spawner(index) => u8(put, c"m_spawner", index),
                            Spot::Dropped(at) => {
                                let [x, y, z] = at.mm();
                                u32(put, c"m_dropped.x", x as u32);
                                u32(put, c"m_dropped.y", y as u32);
                                u32(put, c"m_dropped.z", z as u32);
                            }
                        }
                        u32(put, c"m_until", until.ms());
                        way_fields(put, &way);
                    }
                    Artefact::Carried { by, way } => {
                        u8(put, c"m_carrier", by.get());
                        way_fields(put, &way);
                    }
                }
            }
            Round::Brought { until } => {
                u8(put, c"m_round", BROUGHT);
                u32(put, c"m_until", until.ms());
            }
            Round::Fade { until, sudden } => {
                u8(put, c"m_round", FADE);
                match sudden {
                    None => u8(put, c"m_clock", REGULAR),
                    Some(from) => {
                        u8(put, c"m_clock", SUDDEN);
                        u32(put, c"m_sudden.from", from.ms());
                    }
                }
                u32(put, c"m_until", until.ms());
            }
            Round::Countdown { until, clock } => {
                u8(put, c"m_round", COUNTDOWN);
                clock_fields(put, &clock);
                u32(put, c"m_until", until.ms());
            }
        }
    }

    /// The bytes [`Rule::fields`] names, written to `out`: their count
    /// (at most [`MAX_BYTES`]), `None` when `out` is too short.
    pub fn write(&self, out: &mut [u8]) -> Option<usize> {
        let mut at = 0;
        let mut fits = true;
        self.fields(&mut |field| {
            let (bytes, len) = match field.value {
                Value::U8(value) => ([value, 0, 0, 0], 1),
                Value::U32(value) => (value.to_le_bytes(), 4),
            };
            match out.get_mut(at..at + len) {
                Some(place) => place.copy_from_slice(&bytes[..len]),
                None => fits = false,
            }
            at += len;
        });
        fits.then_some(at)
    }

    /// Parses the bytes [`Rule::write`] wrote for this map's parameters: the
    /// state and the bytes read. Nothing of them is trusted: a value out of
    /// its domain, or a state the rule never stands in, is an error.
    pub fn read(params: &Params, bytes: &[u8]) -> Result<(Rule, usize), WireError> {
        let mut input = Reader(bytes);
        let rng = Rng::seeded(input.u32()?);
        let score = [input.u8()?, input.u8()?];
        if score[0] > params.goal || score[1] > params.goal {
            return Err(WireError::Score);
        }
        let round_starts = input.u32()?;
        let team_0 = input.roster(WireError::Sides)?;
        let team_1 = input.roster(WireError::Sides)?;
        let sides = Sides::new(team_0, team_1).ok_or(WireError::Sides)?;
        let stage = match input.u8()? {
            IDLE if score == [0, 0] => Stage::Idle,
            IDLE => return Err(WireError::Score),
            OVER => {
                let at = input.time()?;
                let reason = match input.u8()? {
                    0 => Reason::Collected,
                    1 => Reason::TimeOut,
                    2 => Reason::Wiped,
                    _ => return Err(WireError::Reason),
                };
                Stage::Over(Over { at, reason })
            }
            PLAY => Stage::Play(read_play(params, &mut input, score, sides)?),
            _ => return Err(WireError::Stage),
        };
        if !round_starts_fit(round_starts, score, &stage) {
            return Err(WireError::RoundStarts);
        }
        let rule = Rule {
            rng,
            score,
            round_starts: round_starts as u16,
            sides,
            stage,
        };
        let size = bytes.len().saturating_sub(input.0.len());
        if usize::from(input.u8()?) != size {
            return Err(WireError::Size);
        }
        Ok((rule, size + 1))
    }

    /// Parses the state that ends `bytes` (the rules blob up to the kd
    /// stats): the state and where its bytes start.
    pub fn read_back(params: &Params, bytes: &[u8]) -> Result<(Rule, usize), WireError> {
        let (&size, before) = bytes.split_last().ok_or(WireError::Short)?;
        let at = before
            .len()
            .checked_sub(usize::from(size))
            .ok_or(WireError::Short)?;
        let (rule, _) = Rule::read(params, bytes.get(at..).ok_or(WireError::Short)?)?;
        Ok((rule, at))
    }
}

/// Whether `count` round starts fit the scores and the stage: every
/// regular delivery but one being followed is a round start (its fade's
/// end), sudden death's wiped team's point is none.
fn round_starts_fit(count: u32, score: [u8; 2], stage: &Stage) -> bool {
    let points = u32::from(score[0]) + u32::from(score[1]);
    let expected = match stage {
        Stage::Idle => Some(0),
        Stage::Over(Over {
            reason: Reason::Collected,
            ..
        }) => points.checked_sub(1),
        // The wiped team's point started no round; every delivery did (the
        // late one that equalised too: its fade began sudden death).
        Stage::Over(Over {
            reason: Reason::Wiped,
            ..
        }) => points.checked_sub(1),
        // A delivery or the time ending a hunt, sudden death's wiped point
        // and a delivery in one step before it.
        Stage::Over(Over {
            reason: Reason::TimeOut,
            ..
        }) => {
            return count <= points && count.saturating_add(2) >= points;
        }
        Stage::Play(Play { round, .. }) => {
            let restarting = matches!(round, Round::Brought { .. } | Round::Fade { .. });
            let wiped = match round {
                Round::Hunt {
                    clock: Clock::Sudden(_),
                    ..
                }
                | Round::Countdown {
                    clock: Clock::Sudden(_),
                    ..
                }
                | Round::Fade {
                    sudden: Some(_), ..
                } => score[0] != score[1],
                _ => false,
            };
            points.checked_sub(u32::from(restarting) + u32::from(wiped))
        }
    };
    expected == Some(count)
}

/// The time domain of a match that started at `start`.
struct Times {
    start: Time,
    horizon: u32,
}

impl Times {
    fn read(&self, input: &mut Reader<'_>) -> Result<Time, WireError> {
        let time = input.time()?;
        match time.since(self.start) {
            Some(passed) if passed <= self.horizon => Ok(time),
            _ => Err(WireError::Time),
        }
    }
}

/// The clock byte and what it carries. `counts_fallen`: whether this round
/// counts them (not the fade, where everyone is about to respawn).
fn read_clock(
    input: &mut Reader<'_>,
    times: &Times,
    sides: Sides,
    counts_fallen: bool,
) -> Result<Option<Sudden>, WireError> {
    match input.u8()? {
        REGULAR => Ok(None),
        SUDDEN => {
            let from = times.read(input)?;
            let fallen = if counts_fallen {
                input.roster(WireError::Fallen)?
            } else {
                Roster::EMPTY
            };
            if !fallen.within(sides.all()) {
                return Err(WireError::Fallen);
            }
            Ok(Some(Sudden { from, fallen }))
        }
        _ => Err(WireError::Clock),
    }
}

fn read_play(
    params: &Params,
    input: &mut Reader<'_>,
    score: [u8; 2],
    sides: Sides,
) -> Result<Play, WireError> {
    let start = input.time()?;
    let times = Times {
        start,
        horizon: params.horizon(),
    };
    let id = input.u8()?;
    let sudden = match id {
        WAITING | SPAWNED | CARRIED_BY | DROPPED | COUNTDOWN => {
            read_clock(input, &times, sides, true)?
        }
        FADE => read_clock(input, &times, sides, false)?,
        BROUGHT => None,
        _ => return Err(WireError::Round),
    };
    // Across the fields: what the rule's transitions keep.
    let below = score[0] < params.goal && score[1] < params.goal;
    match sudden {
        None if id != BROUGHT && !below => return Err(WireError::Score),
        // One delivery: one team at the goal at most.
        None if score == [params.goal; 2] => return Err(WireError::Score),
        Some(_) if score[0].abs_diff(score[1]) > 1 => return Err(WireError::Score),
        Some(sudden) if score[0] == score[1] => {
            let wiped = Team::BOTH
                .into_iter()
                .any(|team| sides.of(team).minus(sudden.fallen).is_empty());
            if wiped {
                return Err(WireError::Wiped);
            }
        }
        _ => {}
    }
    let clock = sudden.map_or(Clock::Regular, Clock::Sudden);
    let fallen = sudden.map_or(Roster::EMPTY, |sudden| sudden.fallen);
    let round = match id {
        WAITING => Round::Hunt {
            clock,
            artefact: Artefact::Waiting {
                until: times.read(input)?,
            },
        },
        SPAWNED => {
            let index = input.u8()?;
            params.spawner(index).ok_or(WireError::Spawner)?;
            let until = times.read(input)?;
            Round::Hunt {
                clock,
                artefact: Artefact::Lying {
                    spot: Spot::Spawner(index),
                    until,
                    way: input.way()?,
                },
            }
        }
        DROPPED => {
            let mm = [
                input.u32()? as i32,
                input.u32()? as i32,
                input.u32()? as i32,
            ];
            let at = Position::from_mm(mm).ok_or(WireError::Position)?;
            let until = times.read(input)?;
            Round::Hunt {
                clock,
                artefact: Artefact::Lying {
                    spot: Spot::Dropped(at),
                    until,
                    way: input.way()?,
                },
            }
        }
        CARRIED_BY => {
            let by = Place::new(input.u8()?).ok_or(WireError::Carrier)?;
            if !sides.all().has(by) || fallen.has(by) {
                return Err(WireError::Carrier);
            }
            Round::Hunt {
                clock,
                artefact: Artefact::Carried {
                    by,
                    way: input.way()?,
                },
            }
        }
        BROUGHT => Round::Brought {
            until: times.read(input)?,
        },
        FADE => Round::Fade {
            until: times.read(input)?,
            sudden: sudden.map(|sudden| sudden.from),
        },
        _ => Round::Countdown {
            until: times.read(input)?,
            clock,
        },
    };
    Ok(Play { start, round })
}

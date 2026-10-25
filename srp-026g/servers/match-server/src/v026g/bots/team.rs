//! A team's commander (`player_bots_research.md` B1, B2; Killzone's
//! squads): every ~0.5 s it hands each bot of the team a play.
//!
//! - Each bot's candidates are sorted by rank band, then score. An
//!   objective the curves all but rule out (under [`Bands::floor`]) is no
//!   objective: it competes in the roam band by its score ([`Bands`]). So
//!   the bands still put a real objective first, whatever its score
//!   against a fight or a roam.
//! - **One bot does the job:** a second bot on a container a teammate
//!   already has (a second raider, a second guard of the lead) needs
//!   [`Bands::second`] to keep the objective band.
//! - **Behind on points** (the HUD's score), an attack on the enemy's
//!   container ([`attack`]) has [`Bands::behind_floor`] and
//!   [`Bands::behind_second`] instead: it keeps the objective band, and a
//!   second attacker may join it (F0 of the owner's matches 0x652d4e6e,
//!   0x1a0961e2, 0x7298fb57); one scoring under 0.06 still does not.
//! - **Commitment:** a bot keeps its play unless a candidate of a higher
//!   band comes, or one of the same band beats it by [`MARGIN`] (research
//!   B9: no flapping between near-equal plays).
//! - **Slots:** a candidate with a slot (an ambush on a battery, a battery
//!   to fetch, a guard post) goes to one bot only; bots are served in the
//!   order of their best score, so the best-placed bot gets the slot.
//!   A slot already held stays with its holder while its play is still a
//!   candidate, unless another bot's candidate for it beats the holder's
//!   by [`MARGIN`] (else two bots near one battery swap it every tick,
//!   neither going for it; seen in the soaks). A bot farther from the
//!   target than the holder needs [`MARGIN_FAR`], and a holder within
//!   [`COMMIT_M`] of it keeps it against any claim: each bot scores from
//!   its own memory, and the holder, nearest and in sight of the danger
//!   there, knew of more enemies by the battery than a teammate behind,
//!   so the slot went back and forth and nobody took it (Cologne
//!   0x60aeff4f, battery 2: six hand-offs in 20 s).
//!   A holder also keeps it against any claim of its rank or lower for
//!   its first [`HOLD_FRESH_MS`] of running the play: commitment by time,
//!   as [`COMMIT_M`] is by distance (Game AI Pro ch. 9 §9.7, inertia).
//!   Taken by a teammate's better score a moment later, the holder went
//!   back to its roam and took the play again within seconds (bench of the
//!   3D routes, 2026-10-25: Roam, Crossfire, Roam). A holder whose way to
//!   the play failed lately, or that carries a battery, has no such hold
//!   (`shaky` of [`assign_trapped`]); a dead one is not asking at all.
//! - The commander says why: each assignment carries the winner's score and
//!   the runners-up for the trace.

use super::curve::Input;
use super::plays::{Candidate, PlayKind, Rank, SlotKey, TargetRef};
use super::view::Place;

/// A candidate of the same band must beat the running play by this.
pub const MARGIN: f32 = 0.12;

/// A bot farther from a held slot's target than its holder must beat the
/// holder's score by this to take it.
pub const MARGIN_FAR: f32 = 0.30;

/// A holder this near its slot's target (its way there, metres) keeps the
/// slot against any other bot's claim.
pub const COMMIT_M: f32 = 5.0;

/// A holder keeps its slot against any claim of its rank or lower for this
/// long after it took the play, ms (the module's doc).
pub const HOLD_FRESH_MS: u32 = 3_000;

/// The candidate's way to its target, metres, when it weighs one.
fn path_m(c: &Candidate) -> Option<f32> {
    c.why
        .parts
        .iter()
        .find(|(input, _, _)| *input == Input::PathDist)
        .map(|&(_, value, _)| value)
}

/// A slot held by a running play still asked for.
struct Held {
    slot: SlotKey,
    holder: Place,
    /// The holder's candidate for it: its score and rank.
    score: f32,
    rank: Rank,
    /// The holder's way to the target, metres.
    holder_m: Option<f32>,
    /// Within [`HOLD_FRESH_MS`] of taking the play, and not `shaky`.
    fresh: bool,
}

/// Whether `c` (another bot's) may take the slot `held`.
fn takes_held(c: &Candidate, held: &Held) -> bool {
    if held.fresh && c.rank <= held.rank {
        return false;
    }
    let (score, holder_m) = (held.score, held.holder_m);
    let margin = match (holder_m, path_m(c)) {
        (Some(held), _) if held <= COMMIT_M => return false,
        (Some(held), Some(mine)) if mine > held => MARGIN_FAR,
        _ => MARGIN,
    };
    c.score.get() >= score + margin
}

/// How the commander reads a candidate's band (F2 of the owner's matches
/// of 2026-10-16, `player_bots_behaviour.md` §3 "The commander's bands").
/// In those two matches 192 of 226 raids were taken at a score under 0.05,
/// over a roam at 1.00, and gave no steal; half were replaced within
/// 10 s; the two raids that scored were taken at 0.21 and 0.24.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bands {
    /// An objective candidate scoring under this competes in
    /// [`Bands::demote_to`] by its score.
    pub floor: f32,
    /// The band an objective under the floor competes in: roam (in the
    /// fight band a raid at 0.02 still beat every roam; bench, 600 s).
    pub demote_to: Rank,
    /// The bot's running play keeps its band down to this share of the
    /// bars (no flapping across the floor).
    pub keep: f32,
    /// A second bot on the same container (a raid's or a guard's second
    /// slot, a sneak or a late denial a teammate already has) needs this
    /// score to keep the objective band.
    pub second: f32,
    /// While the team is behind on points, an attack on the enemy's
    /// container ([`attack`]) is held to this floor instead of
    /// [`Bands::floor`]: their scores are products of many parts
    /// (enemies dead, near the target, the way, aggression), so most sit
    /// under 0.10. With the floor the team behind took none of those in
    /// three real 7v7 matches (Hold, Group and Roam instead, 2-5 points
    /// down, every battery home); before it, 25-43 % of them, and it came
    /// back from -3 to 0 in one. Not 0: since a raid once taken holds
    /// (`raid::raid_candidate`), the raids under it ran on for nothing.
    /// In the owner's PvB matches 0x6da44bbb and 0x1b8ee975 the team
    /// behind took 47 and 57 raids at 0.00-0.16 and stole none (14 raid
    /// deaths in one); 0.06 would have left 11 and 11. Bench (600 s,
    /// `SRP_BENCH_START=3,7` and `7,3`, 8 maps, 4 v 4 and 7 v 7, 6 seeds):
    /// the side behind raided 299 -> 166 times a seed, 6,097 -> 2,846
    /// bot-s, its raid deaths 29.5 -> 18.8, its raid steals 8.3 -> 9.2 and
    /// the points won back held (10.8 -> 10.3); its kills rose 421 -> 434.
    /// At 0.10 the points won back fell (-6.3 ± 2.9).
    pub behind_floor: f32,
    /// ... and a second attacker on the same container to this bar
    /// instead of [`Bands::second`] (attacks in pairs, not one by one: a
    /// late denial alone died 2.0-2.7 times a minute).
    pub behind_second: f32,
}

impl Default for Bands {
    fn default() -> Self {
        Self {
            floor: 0.10,
            demote_to: Rank::Roam,
            keep: 0.5,
            second: 0.20,
            behind_floor: 0.06,
            behind_second: 0.0,
        }
    }
}

/// An attack on the enemy's container: what a team behind on points
/// needs to win points back (the batteries home are taken only so).
pub fn attack(kind: PlayKind) -> bool {
    matches!(
        kind,
        PlayKind::Raid | PlayKind::Sneak | PlayKind::LateDenial
    )
}

impl Bands {
    /// The bands the commander uses: the defaults; in the bench (tests
    /// only) changed by `SRP_BENCH_BANDS=name=value,...` (`floor`, `keep`,
    /// `second`, `behind_floor`, `behind_second`).
    pub fn used() -> Self {
        #[cfg(test)]
        {
            static BANDS: std::sync::OnceLock<Bands> = std::sync::OnceLock::new();
            *BANDS.get_or_init(|| {
                let mut bands = Self::default();
                let given = std::env::var("SRP_BENCH_BANDS").unwrap_or_default();
                for pair in given.split(',').filter(|p| !p.is_empty()) {
                    let (name, value) = pair.split_once('=').expect("SRP_BENCH_BANDS: name=value");
                    let value: f32 = value.parse().expect("SRP_BENCH_BANDS: a number");
                    match name {
                        "floor" => bands.floor = value,
                        "keep" => bands.keep = value,
                        "second" => bands.second = value,
                        "behind_floor" => bands.behind_floor = value,
                        "behind_second" => bands.behind_second = value,
                        _ => panic!("SRP_BENCH_BANDS: no band {name}"),
                    }
                }
                bands
            })
        }
        #[cfg(not(test))]
        Self::default()
    }

    /// The band `c` competes in: its rank, or [`Bands::demote_to`] for an
    /// objective under the floor, or under [`Bands::second`] when a
    /// teammate already has its container (`second`); an attack of a
    /// team `behind` on points has [`Bands::behind_floor`] and
    /// [`Bands::behind_second`] instead. The bot's own running play
    /// (`running`) is held to [`Bands::keep`] of the bar.
    pub fn band(&self, c: &Candidate, running: bool, second: bool, behind: bool) -> Rank {
        if c.rank != Rank::Objective {
            return c.rank;
        }
        let (floor, second_bar) = if behind && attack(c.kind) {
            (self.behind_floor, self.behind_second)
        } else {
            (self.floor, self.second)
        };
        let bar = if second { floor.max(second_bar) } else { floor };
        let bar = if running { bar * self.keep } else { bar };
        if c.score.get() < bar {
            self.demote_to.min(c.rank)
        } else {
            c.rank
        }
    }

    /// The order of a bot's candidates: band, then score; the best first.
    pub fn better(&self, a: &Candidate, b: &Candidate, behind: bool) -> std::cmp::Ordering {
        self.band(b, false, false, behind)
            .cmp(&self.band(a, false, false, behind))
            .then(b.score.get().total_cmp(&a.score.get()))
    }
}

/// A bot's play from the commander.
#[derive(Clone, Debug, PartialEq)]
pub struct Assignment {
    pub kind: PlayKind,
    pub target: TargetRef,
    pub slot: Option<SlotKey>,
    pub score: f32,
    /// Since when (match ms).
    pub since: u32,
    pub reason: &'static str,
}

/// One bot's input to the commander.
pub struct Asking<'a> {
    pub place: Place,
    pub candidates: Vec<Candidate>,
    pub current: Option<&'a Assignment>,
}

/// The assignments for one commander tick: `(place, assignment, changed)`,
/// the score level. A bot with no candidate at all gets none.
pub fn assign(asking: Vec<Asking>, now: u32) -> Vec<(Place, Assignment, bool)> {
    assign_at(asking, now, 0.0)
}

/// [`assign`] for a team at score gap `gap` (the HUD's points, ours less
/// theirs).
pub fn assign_at(asking: Vec<Asking>, now: u32, gap: f32) -> Vec<(Place, Assignment, bool)> {
    assign_with(asking, now, &Bands::used(), gap)
}

/// Whether a teammate's play already goes to this container (one bot
/// there does the job; a second needs [`Bands::second`]).
fn on_container(c: &Candidate, given: &[(PlayKind, TargetRef)]) -> bool {
    matches!(c.target, TargetRef::Container(_)) && given.contains(&(c.kind, c.target))
}

/// [`assign_at`] for a team whose bots of `given_up` gave up in a pocket
/// (`escape.rs`): a slot one holds goes to a teammate as a free one, even
/// within [`COMMIT_M`] of its target, and none takes a slotted play. The
/// bots of `shaky` (a way to their play failed lately, a carrier) have no
/// [`HOLD_FRESH_MS`] hold on their slots.
pub fn assign_trapped(
    asking: Vec<Asking>,
    now: u32,
    gap: f32,
    given_up: &[Place],
    shaky: &[Place],
) -> Vec<(Place, Assignment, bool)> {
    assign_given_up(asking, now, &Bands::used(), gap, given_up, shaky)
}

/// [`assign`] with the bands given.
pub fn assign_with(
    asking: Vec<Asking>,
    now: u32,
    bands: &Bands,
    gap: f32,
) -> Vec<(Place, Assignment, bool)> {
    assign_given_up(asking, now, bands, gap, &[], &[])
}

fn assign_given_up(
    mut asking: Vec<Asking>,
    now: u32,
    bands: &Bands,
    gap: f32,
    given_up: &[Place],
    shaky: &[Place],
) -> Vec<(Place, Assignment, bool)> {
    let behind = gap < 0.0;
    for bot in &mut asking {
        bot.candidates.sort_by(|a, b| bands.better(a, b, behind));
    }
    // The best-placed bots first.
    asking.sort_by(|a, b| {
        let best = |x: &Asking| {
            x.candidates.first().map_or((Rank::Roam, 0.0), |c| {
                (bands.band(c, false, false, behind), c.score.get())
            })
        };
        let (ra, sa) = best(a);
        let (rb, sb) = best(b);
        rb.cmp(&ra).then(sb.total_cmp(&sa))
    });
    // Slots held by running plays still asked for.
    let held: Vec<Held> = asking
        .iter()
        .filter(|bot| !given_up.contains(&bot.place))
        .filter_map(|bot| {
            let current = bot.current?;
            let slot = current.slot?;
            bot.candidates
                .iter()
                .find(|c| {
                    c.slot == Some(slot) && c.kind == current.kind && c.target == current.target
                })
                .map(|c| Held {
                    slot,
                    holder: bot.place,
                    score: c.score.get(),
                    rank: c.rank,
                    holder_m: path_m(c),
                    fresh: !shaky.contains(&bot.place)
                        && now < current.since.saturating_add(HOLD_FRESH_MS),
                })
        })
        .collect();
    let mut taken: Vec<SlotKey> = Vec::new();
    // The plays given this tick, by kind and target.
    let mut given: Vec<(PlayKind, TargetRef)> = Vec::new();
    let mut out = Vec::new();
    for bot in asking {
        let trapped = given_up.contains(&bot.place);
        let free = |c: &&Candidate| {
            c.slot.is_none_or(|slot| {
                !trapped
                    && !taken.contains(&slot)
                    && held
                        .iter()
                        .all(|h| h.slot != slot || h.holder == bot.place || takes_held(c, h))
            })
        };
        let is_running = |c: &Candidate| {
            bot.current
                .is_some_and(|current| c.kind == current.kind && c.target == current.target)
        };
        let band = |c: &Candidate| bands.band(c, is_running(c), on_container(c, &given), behind);
        // The best free candidate, its band read with the plays given so
        // far (the order above is the bands alone).
        let Some(best) = bot
            .candidates
            .iter()
            .filter(free)
            .filter(|c| !is_running(c))
            .fold(None, |best: Option<&Candidate>, c| match best {
                Some(b) if (band(b), b.score.get()) >= (band(c), c.score.get()) => Some(b),
                _ => Some(c),
            })
            .or_else(|| bot.candidates.iter().find(free))
        else {
            continue;
        };
        // The running play, if it is still a candidate and its slot free.
        let running = bot.current.and_then(|current| {
            bot.candidates
                .iter()
                .filter(free)
                .find(|c| c.kind == current.kind && c.target == current.target)
                .map(|c| (current, c))
        });
        let (choice, changed, reason) = match running {
            Some((current, c))
                if band(c) > band(best)
                    || (band(c) == band(best) && best.score.get() < c.score.get() + MARGIN) =>
            {
                (c, false, current.reason)
            }
            Some((_, c)) if band(c) == band(best) => (best, true, "a better play by the margin"),
            _ if bot.current.is_some() => (best, true, "the play ran out or a higher band came"),
            _ => (best, true, "the best play"),
        };
        if let Some(slot) = choice.slot {
            taken.push(slot);
        }
        given.push((choice.kind, choice.target));
        let since = match (changed, bot.current) {
            (false, Some(current)) => current.since,
            _ => now,
        };
        out.push((
            bot.place,
            Assignment {
                kind: choice.kind,
                target: choice.target,
                slot: choice.slot,
                score: choice.score.get(),
                since,
                reason,
            },
            changed,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::curve::{Explain, Score};
    use crate::v026g::bots::plays::Rank;

    fn c(kind: PlayKind, rank: Rank, score: f32, slot: Option<u32>, target: usize) -> Candidate {
        Candidate {
            kind,
            target: TargetRef::Battery(target),
            rank,
            score: Score::new(score),
            why: Explain::default(),
            slot: slot.map(|key| SlotKey { kind, key }),
        }
    }

    #[test]
    fn a_slot_goes_to_the_best_placed_bot_and_the_other_takes_its_next_play() {
        let asking = vec![
            Asking {
                place: 1,
                candidates: vec![
                    c(PlayKind::Collect, Rank::Objective, 0.5, Some(0), 0),
                    c(PlayKind::Roam, Rank::Roam, 0.9, None, 9),
                ],
                current: None,
            },
            Asking {
                place: 2,
                candidates: vec![c(PlayKind::Collect, Rank::Objective, 0.8, Some(0), 0)],
                current: None,
            },
        ];
        let out = assign(asking, 1_000);
        let of = |place| out.iter().find(|(p, _, _)| *p == place).unwrap().1.kind;
        assert_eq!(of(2), PlayKind::Collect);
        assert_eq!(
            of(1),
            PlayKind::Roam,
            "the band below once the slot is taken"
        );
    }

    #[test]
    fn a_held_slot_stays_with_its_holder_unless_beaten_by_the_margin() {
        let current = Assignment {
            kind: PlayKind::Collect,
            target: TargetRef::Battery(0),
            slot: Some(SlotKey {
                kind: PlayKind::Collect,
                key: 0,
            }),
            score: 0.5,
            since: 100,
            reason: "the best play",
        };
        let run = |other: f32| {
            let asking = vec![
                Asking {
                    place: 1,
                    candidates: vec![
                        c(PlayKind::Collect, Rank::Objective, 0.5, Some(0), 0),
                        c(PlayKind::Roam, Rank::Roam, 0.9, None, 9),
                    ],
                    current: Some(&current),
                },
                Asking {
                    place: 2,
                    candidates: vec![
                        c(PlayKind::Collect, Rank::Objective, other, Some(0), 0),
                        c(PlayKind::Roam, Rank::Roam, 0.9, None, 8),
                    ],
                    current: None,
                },
            ];
            let out = assign(asking, 1_000 + HOLD_FRESH_MS);
            let of = |place| out.iter().find(|(p, _, _)| *p == place).unwrap().1.kind;
            (of(1), of(2))
        };
        // A little better placed: the holder keeps it.
        assert_eq!(run(0.55), (PlayKind::Collect, PlayKind::Roam));
        // Much better placed: it goes over.
        assert_eq!(run(0.8), (PlayKind::Roam, PlayKind::Collect));
    }

    /// Cologne's battery 2: the holder by the battery knew more enemies
    /// there and scored less than a teammate behind it.
    #[test]
    fn a_held_slot_goes_to_a_farther_bot_by_the_far_margin_and_never_from_by_its_target() {
        let at = |score: f32, path_m: f32| Candidate {
            why: Explain {
                parts: vec![(Input::PathDist, path_m, 0.9)],
            },
            ..c(PlayKind::Collect, Rank::Objective, score, Some(0), 0)
        };
        let current = Assignment {
            kind: PlayKind::Collect,
            target: TargetRef::Battery(0),
            slot: Some(SlotKey {
                kind: PlayKind::Collect,
                key: 0,
            }),
            score: 0.6,
            since: 100,
            reason: "the best play",
        };
        let run = |holder: Candidate, other: Candidate, alarm: bool| {
            let mut mine = vec![holder, c(PlayKind::Roam, Rank::Roam, 0.9, None, 9)];
            if alarm {
                mine.push(c(PlayKind::Trade, Rank::Alarm, 0.7, None, 7));
            }
            let asking = vec![
                Asking {
                    place: 1,
                    candidates: mine,
                    current: (!alarm).then_some(&current),
                },
                Asking {
                    place: 2,
                    candidates: vec![other, c(PlayKind::Roam, Rank::Roam, 0.9, None, 8)],
                    current: None,
                },
            ];
            let out = assign(asking, 1_000 + HOLD_FRESH_MS);
            let of = |place| out.iter().find(|(p, _, _)| *p == place).unwrap().1.kind;
            (of(1), of(2))
        };
        let (collect, roam) = (PlayKind::Collect, PlayKind::Roam);
        // By the battery: kept against a far better score.
        assert_eq!(run(at(0.36, 3.0), at(0.61, 12.0), false), (collect, roam));
        assert_eq!(run(at(0.36, 3.0), at(0.95, 2.0), false), (collect, roam));
        // Farther off: a farther bot needs the far margin.
        assert_eq!(run(at(0.36, 12.0), at(0.61, 20.0), false), (collect, roam));
        assert_eq!(run(at(0.36, 12.0), at(0.70, 20.0), false), (roam, collect));
        // A nearer bot, the usual margin.
        assert_eq!(run(at(0.36, 12.0), at(0.50, 8.0), false), (roam, collect));
        // An alarm takes the holder (the poll after, it runs the alarm),
        // and the slot goes.
        assert_eq!(
            run(at(0.36, 3.0), at(0.61, 12.0), true),
            (PlayKind::Trade, collect)
        );
    }

    /// A holder keeps its slot against a better teammate for its first
    /// `HOLD_FRESH_MS` of running the play, but not against a claim of a
    /// higher rank, nor when its way failed lately or it carries
    /// (`shaky`), nor when it is dead (not asking); past the hold, the
    /// margin rules again.
    #[test]
    fn a_fresh_holder_keeps_its_slot_unless_shaky_dead_or_outranked() {
        let current = Assignment {
            kind: PlayKind::Collect,
            target: TargetRef::Battery(0),
            slot: Some(SlotKey {
                kind: PlayKind::Collect,
                key: 0,
            }),
            score: 0.5,
            since: 1_000,
            reason: "the best play",
        };
        let run = |now: u32, rank: Rank, shaky: &[Place], holder_asks: bool| {
            let mut asking = vec![Asking {
                place: 2,
                candidates: vec![
                    c(PlayKind::Collect, rank, 0.9, Some(0), 0),
                    c(PlayKind::Roam, Rank::Roam, 0.9, None, 8),
                ],
                current: None,
            }];
            if holder_asks {
                asking.push(Asking {
                    place: 1,
                    candidates: vec![
                        c(PlayKind::Collect, Rank::Objective, 0.5, Some(0), 0),
                        c(PlayKind::Roam, Rank::Roam, 0.9, None, 9),
                    ],
                    current: Some(&current),
                });
            }
            let out = assign_trapped(asking, now, 0.0, &[], shaky);
            out.iter().find(|(p, _, _)| *p == 2).unwrap().1.kind
        };
        let (collect, roam) = (PlayKind::Collect, PlayKind::Roam);
        let fresh = 1_000 + HOLD_FRESH_MS - 1;
        // The plain hold: a far better teammate does not take it.
        assert_eq!(run(fresh, Rank::Objective, &[], true), roam);
        // Past the hold: the margin rules, and it goes over.
        assert_eq!(
            run(1_000 + HOLD_FRESH_MS, Rank::Objective, &[], true),
            collect
        );
        // A claim of a higher rank takes it within the hold.
        assert_eq!(run(fresh, Rank::Alarm, &[], true), collect);
        // A holder whose way failed lately, or that carries: no hold.
        assert_eq!(run(fresh, Rank::Objective, &[1], true), collect);
        // A dead holder is not asking: the slot is free.
        assert_eq!(run(fresh, Rank::Objective, &[], false), collect);
    }

    /// A holder by its target that gave up in a pocket (`escape.rs`) lets
    /// the slot go to a teammate, and takes no slotted play itself.
    #[test]
    fn a_holder_given_up_in_a_pocket_lets_its_slot_go() {
        let at = |score: f32, path_m: f32| Candidate {
            why: Explain {
                parts: vec![(Input::PathDist, path_m, 0.9)],
            },
            ..c(PlayKind::Collect, Rank::Objective, score, Some(0), 0)
        };
        let current = Assignment {
            kind: PlayKind::Collect,
            target: TargetRef::Battery(0),
            slot: Some(SlotKey {
                kind: PlayKind::Collect,
                key: 0,
            }),
            score: 0.6,
            since: 100,
            reason: "the best play",
        };
        let run = |given_up: &[Place]| {
            let asking = vec![
                Asking {
                    place: 1,
                    candidates: vec![at(0.9, 3.0), c(PlayKind::Roam, Rank::Roam, 0.9, None, 9)],
                    current: Some(&current),
                },
                Asking {
                    place: 2,
                    candidates: vec![at(0.5, 12.0), c(PlayKind::Roam, Rank::Roam, 0.9, None, 8)],
                    current: None,
                },
            ];
            let out = assign_trapped(asking, 1_000, 0.0, given_up, &[]);
            let of = |place| out.iter().find(|(p, _, _)| *p == place).unwrap().1.kind;
            (of(1), of(2))
        };
        let (collect, roam) = (PlayKind::Collect, PlayKind::Roam);
        assert_eq!(run(&[]), (collect, roam));
        assert_eq!(run(&[1]), (roam, collect));
    }

    #[test]
    fn a_running_play_is_kept_unless_beaten_by_the_margin_or_a_higher_band() {
        let current = Assignment {
            kind: PlayKind::Roam,
            target: TargetRef::Battery(1),
            slot: None,
            score: 0.5,
            since: 100,
            reason: "the best play",
        };
        let ask = |candidates| {
            assign(
                vec![Asking {
                    place: 0,
                    candidates,
                    current: Some(&current),
                }],
                2_000,
            )
        };
        // A slightly better roam: kept, with its start time.
        let out = ask(vec![
            c(PlayKind::Roam, Rank::Roam, 0.5, None, 1),
            c(PlayKind::Roam, Rank::Roam, 0.6, None, 2),
        ]);
        assert!(!out[0].2 && out[0].1.since == 100);
        // Much better: changed.
        let out = ask(vec![
            c(PlayKind::Roam, Rank::Roam, 0.3, None, 1),
            c(PlayKind::Roam, Rank::Roam, 0.6, None, 2),
        ]);
        assert!(out[0].2 && out[0].1.target == TargetRef::Battery(2));
        // A higher band, however low its score: changed.
        let out = ask(vec![
            c(PlayKind::Roam, Rank::Roam, 0.9, None, 1),
            c(PlayKind::Collect, Rank::Objective, 0.1, None, 3),
        ]);
        assert_eq!(out[0].1.kind, PlayKind::Collect);
    }

    fn on(kind: PlayKind, score: f32, slot: u32) -> Candidate {
        Candidate {
            target: TargetRef::Container(0),
            ..c(kind, Rank::Objective, score, Some(slot), 0)
        }
    }

    const BANDS: Bands = Bands {
        floor: 0.10,
        demote_to: Rank::Roam,
        keep: 0.5,
        second: 0.2,
        behind_floor: 0.06,
        behind_second: 0.0,
    };

    #[test]
    fn the_default_bands_are_the_measured_ones() {
        assert_eq!(Bands::default(), BANDS);
    }

    /// One bot's play: (kind, changed).
    fn one(
        bands: &Bands,
        candidates: Vec<Candidate>,
        current: Option<&Assignment>,
    ) -> (PlayKind, bool) {
        one_at(bands, candidates, current, 0.0)
    }

    /// [`one`] at a score gap.
    fn one_at(
        bands: &Bands,
        candidates: Vec<Candidate>,
        current: Option<&Assignment>,
        gap: f32,
    ) -> (PlayKind, bool) {
        let out = assign_with(
            vec![Asking {
                place: 0,
                candidates,
                current,
            }],
            2_000,
            bands,
            gap,
        );
        (out[0].1.kind, out[0].2)
    }

    #[test]
    fn an_objective_the_curves_all_but_rule_out_is_no_objective() {
        // The owner's match, 15a 593.4: a raid at 0.02 over a roam at 1.00.
        let low = || {
            vec![
                on(PlayKind::Raid, 0.02, 0),
                c(PlayKind::Cut, Rank::Fight, 0.08, None, 1),
                c(PlayKind::Roam, Rank::Roam, 1.0, None, 2),
            ]
        };
        assert_eq!(one(&BANDS, low(), None).0, PlayKind::Cut);
        // Under the floor it is a roam-band play by its score.
        let mut no_fight = low();
        no_fight.remove(1);
        assert_eq!(one(&BANDS, no_fight.clone(), None).0, PlayKind::Roam);
        // In the fight band (tried) it still beat every roam.
        let to_fight = Bands {
            demote_to: Rank::Fight,
            ..BANDS
        };
        assert_eq!(one(&to_fight, no_fight, None).0, PlayKind::Raid);
        // A raid that means it (the two that scored were at 0.21, 0.24).
        let real = vec![
            on(PlayKind::Raid, 0.21, 0),
            c(PlayKind::Cut, Rank::Fight, 0.9, None, 1),
            c(PlayKind::Roam, Rank::Roam, 1.0, None, 2),
        ];
        assert_eq!(one(&BANDS, real, None).0, PlayKind::Raid);
    }

    #[test]
    fn meaningful_objectives_and_alarms_still_win() {
        let fights = || {
            vec![
                c(PlayKind::Cut, Rank::Fight, 0.9, None, 1),
                c(PlayKind::Roam, Rank::Roam, 1.0, None, 2),
            ]
        };
        for (play, score) in [
            (
                c(PlayKind::Collect, Rank::Objective, 0.15, Some(3), 3),
                0.15,
            ),
            (c(PlayKind::Escort, Rank::Objective, 0.3, Some(3), 3), 0.3),
            (
                c(PlayKind::Intercept, Rank::Objective, 0.12, Some(3), 3),
                0.12,
            ),
            (on(PlayKind::LateDenial, 0.12, 0), 0.12),
            // An alarm is never demoted, however low.
            (c(PlayKind::Intercept, Rank::Alarm, 0.01, Some(3), 3), 0.01),
            (c(PlayKind::Collect, Rank::Alarm, 0.02, Some(3), 3), 0.02),
        ] {
            let kind = play.kind;
            let mut all = fights();
            all.push(play);
            assert_eq!(one(&BANDS, all, None).0, kind, "{kind:?} at {score}");
        }
    }

    #[test]
    fn a_running_objective_keeps_its_band_down_to_the_keep_share_of_the_floor() {
        let current = Assignment {
            kind: PlayKind::Raid,
            target: TargetRef::Container(0),
            slot: Some(SlotKey {
                kind: PlayKind::Raid,
                key: 0,
            }),
            score: 0.2,
            since: 100,
            reason: "the best play",
        };
        let with = |raid: f32| {
            vec![
                on(PlayKind::Raid, raid, 0),
                c(PlayKind::Cut, Rank::Fight, 0.5, None, 1),
            ]
        };
        // 0.07: under the floor for a new take, over half of it: kept.
        assert_eq!(
            one(&BANDS, with(0.07), Some(&current)),
            (PlayKind::Raid, false)
        );
        assert_eq!(one(&BANDS, with(0.07), None).0, PlayKind::Cut);
        // 0.03: under both: the fight wins.
        assert_eq!(
            one(&BANDS, with(0.03), Some(&current)),
            (PlayKind::Cut, true)
        );
    }

    #[test]
    fn a_second_bot_on_one_container_needs_the_second_bar() {
        let run = |second: f32| {
            let asking = vec![
                Asking {
                    place: 1,
                    candidates: vec![
                        on(PlayKind::Raid, 0.4, 0),
                        c(PlayKind::Cut, Rank::Fight, 0.3, None, 1),
                    ],
                    current: None,
                },
                Asking {
                    place: 2,
                    candidates: vec![
                        on(PlayKind::Raid, second, 0x100),
                        c(PlayKind::Cut, Rank::Fight, 0.3, None, 2),
                    ],
                    current: None,
                },
            ];
            let out = assign_with(asking, 1_000, &BANDS, 0.0);
            let of = |place| out.iter().find(|(p, _, _)| *p == place).unwrap().1.kind;
            (of(1), of(2))
        };
        assert_eq!(run(0.15), (PlayKind::Raid, PlayKind::Cut));
        assert_eq!(run(0.25), (PlayKind::Raid, PlayKind::Raid));
        // Two guards of the lead the same way.
        let guards = |score: f32| {
            let asking = vec![
                Asking {
                    place: 1,
                    candidates: vec![on(PlayKind::GuardLead, 0.6, 0)],
                    current: None,
                },
                Asking {
                    place: 2,
                    candidates: vec![
                        on(PlayKind::GuardLead, score, 1),
                        c(PlayKind::HoldExit, Rank::Fight, 0.3, None, 2),
                    ],
                    current: None,
                },
            ];
            let out = assign_with(asking, 1_000, &BANDS, 0.0);
            out.iter().find(|(p, _, _)| *p == 2).unwrap().1.kind
        };
        assert_eq!(guards(0.12), PlayKind::HoldExit);
        assert_eq!(guards(0.5), PlayKind::GuardLead);
    }

    #[test]
    fn a_team_behind_attacks_the_enemy_container_under_the_floor() {
        // 7298 600.9 s, bot 12 at 0-5: LateDenial 0.102 lost to Roam 1.0;
        // most attacks score under 0.10.
        for attack in [PlayKind::Raid, PlayKind::Sneak, PlayKind::LateDenial] {
            let all_at = |score| {
                vec![
                    on(attack, score, 0),
                    c(PlayKind::Crossfire, Rank::Fight, 0.85, None, 1),
                    c(PlayKind::Roam, Rank::Roam, 1.0, None, 2),
                ]
            };
            let all = || all_at(0.07);
            assert_eq!(one_at(&BANDS, all(), None, -2.0).0, attack, "{attack:?}");
            // Level or ahead the floor stands (PR 123's anti-churn).
            assert_eq!(one_at(&BANDS, all(), None, 0.0).0, PlayKind::Crossfire);
            assert_eq!(one_at(&BANDS, all(), None, 1.0).0, PlayKind::Crossfire);
            // Behind, one under 0.06 is no attack (0x6da44bbb: 47 raids
            // at 0.00-0.16, none stole).
            assert_eq!(one_at(&BANDS, all_at(0.03), None, -2.0).0, PlayKind::Crossfire);
        }
        // Other objectives keep the floor behind too: a hold (an ambush, a
        // guard) gains a team behind nothing.
        let ambush = vec![
            c(PlayKind::Ambush, Rank::Objective, 0.03, Some(4), 4),
            c(PlayKind::Roam, Rank::Roam, 1.0, None, 2),
        ];
        assert_eq!(one_at(&BANDS, ambush, None, -2.0).0, PlayKind::Roam);
    }

    #[test]
    fn a_team_behind_sends_a_second_attacker() {
        let run = |gap: f32| {
            let asking = vec![
                Asking {
                    place: 1,
                    candidates: vec![
                        on(PlayKind::Raid, 0.4, 0),
                        c(PlayKind::Cut, Rank::Fight, 0.3, None, 1),
                    ],
                    current: None,
                },
                Asking {
                    place: 2,
                    candidates: vec![
                        on(PlayKind::Raid, 0.07, 0x100),
                        c(PlayKind::Cut, Rank::Fight, 0.3, None, 2),
                    ],
                    current: None,
                },
            ];
            let out = assign_with(asking, 1_000, &BANDS, gap);
            let of = |place| out.iter().find(|(p, _, _)| *p == place).unwrap().1.kind;
            (of(1), of(2))
        };
        assert_eq!(run(-1.0), (PlayKind::Raid, PlayKind::Raid));
        assert_eq!(run(0.0), (PlayKind::Raid, PlayKind::Cut));
    }

    #[test]
    fn a_floor_of_zero_is_the_old_lexicographic_order() {
        let off = Bands {
            floor: 0.0,
            second: 0.0,
            ..BANDS
        };
        let all = vec![
            on(PlayKind::Raid, 0.01, 0),
            c(PlayKind::Cut, Rank::Fight, 0.9, None, 1),
            c(PlayKind::Roam, Rank::Roam, 1.0, None, 2),
        ];
        assert_eq!(one(&off, all, None).0, PlayKind::Raid);
    }
}

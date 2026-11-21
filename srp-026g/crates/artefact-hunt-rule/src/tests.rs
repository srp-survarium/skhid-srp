//! The rules of docs/notes/artefact_hunt.md, case by case, with 0.46c2's
//! shipped numbers.

extern crate std;

use std::vec::Vec;

use super::*;

/// 0.46c2's `mode.parameters`.
const RAW: RawParams = RawParams {
    match_time: 900,
    time_needed_to_continue_match: 90,
    draw_additional_time: 180,
    time_before_new_round: 3,
    round_countdown_time: 5,
    score: 5,
    respawn_time: 15,
    time_needed_to_respawn: 7,
    vi_spawn_time: 60,
    vi_spawn_variation: 10,
    vi_not_picked_ttl: 120,
    vi_dropped_ttl: 60,
    drop_lift: 0.13,
    usable_radius: 0.3,
};

/// A base's container: a box 1 m a side over its position, and one a
/// metre higher (the shapes are the config's, in world space).
const fn crate_at(x: f32) -> [RawShape; 1] {
    [RawShape {
        kind: 1,
        position: [x, 0.5, 0.0],
        axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        half: [0.5, 0.5, 0.5],
    }]
}
const CRATE_0: [RawShape; 1] = crate_at(0.0);
const CRATE_1: [RawShape; 1] = crate_at(100.0);

fn spawners() -> [Spawner; 4] {
    [10.0, 40.0, 60.0, 90.0].map(|x| Spawner {
        position: [x, 0.0, 0.0],
    })
}

/// Team 1's base first in the config: the bases are by team.
const BASES: [BaseDef; 2] = [
    BaseDef {
        team: 1,
        position: [100.0, 0.0, 0.0],
        shapes: &CRATE_1,
    },
    BaseDef {
        team: 0,
        position: [0.0, 0.0, 0.0],
        shapes: &CRATE_0,
    },
];

/// Team 0's base at x = 0, team 1's at x = 100 m; four spawners on the
/// line between them.
fn params() -> Params {
    Params::parse(&RAW, &spawners(), &BASES).unwrap()
}

fn place(raw: u8) -> Place {
    Place::new(raw).unwrap()
}

fn at(x: f32) -> Position {
    Position::from_metres([x, 0.0, 0.0]).unwrap()
}

fn t(ms: u32) -> Time {
    Time::from_ms(ms)
}

/// The match's start in the tests: not 0, so that a time is never taken for
/// a duration.
const START: u32 = 20_000;

/// The simulation's step.
const TICK: u32 = 50;

/// A match with its rule: the events of a test go through `send`, which
/// also checks that every state is one the wire carries and takes back.
struct Match {
    params: Params,
    rule: Rule,
    now: u32,
    log: Vec<Action>,
}

impl Match {
    /// Places 0..3 of team 0 and 10..13 of team 1 entered; the match started.
    fn started(seed: u32) -> Match {
        Match::started_with(seed, params())
    }

    fn started_with(seed: u32, params: Params) -> Match {
        let mut game = Match {
            params,
            rule: Rule::new(seed),
            now: 0,
            log: Vec::new(),
        };
        for raw in 0..4 {
            game.send(Event::Entered {
                place: place(raw),
                team: Team::Zero,
                now: t(0),
            });
            game.send(Event::Entered {
                place: place(10 + raw),
                team: Team::One,
                now: t(0),
            });
        }
        game.now = START;
        game.send(Event::Start { now: t(START) });
        game
    }

    fn send(&mut self, event: Event) -> Vec<Action> {
        let (rule, actions) = self.rule.step(&self.params, event);
        self.rule = rule;
        let mut bytes = [0u8; MAX_BYTES];
        let len = rule.write(&mut bytes).expect("a state fits MAX_BYTES");
        assert_eq!(
            Rule::read(&self.params, &bytes[..len]),
            Ok((rule, len)),
            "after {event:?}"
        );
        let actions: Vec<Action> = actions.iter().collect();
        self.log.extend(&actions);
        actions
    }

    /// One step: what the respawn rule was told before the rule's tick.
    fn step(&mut self) -> Spawning {
        self.now += TICK;
        let spawning = self.rule.spawning(t(self.now));
        self.send(Event::Tick { now: t(self.now) });
        spawning
    }

    /// Ticks of 50 ms up to and with the match time `ms` (from its start).
    fn run_to(&mut self, ms: u32) {
        while self.now < START + ms {
            self.step();
        }
    }

    fn run_for(&mut self, ms: u32) {
        self.run_to(self.now - START + ms);
    }

    fn play(&self) -> Play {
        match self.rule.stage() {
            Stage::Play(play) => *play,
            other => panic!("not in play: {other:?}"),
        }
    }

    fn round(&self) -> Round {
        *self.play().round()
    }

    fn artefact(&self) -> Artefact {
        match self.round() {
            Round::Hunt { artefact, .. } => artefact,
            other => panic!("no hunt: {other:?}"),
        }
    }

    fn clock(&self) -> Clock {
        match self.round() {
            Round::Hunt { clock, .. } | Round::Countdown { clock, .. } => clock,
            other => panic!("no clock: {other:?}"),
        }
    }

    /// The artefact's time, from the match's start.
    fn until(&self) -> u32 {
        match self.artefact() {
            Artefact::Waiting { until } | Artefact::Lying { until, .. } => until.ms() - START,
            Artefact::Carried { .. } => panic!("carried: no time"),
        }
    }

    /// Runs until the artefact lies in the world; where.
    fn spawned(&mut self) -> Position {
        assert!(
            matches!(self.artefact(), Artefact::Waiting { .. }),
            "{:?}",
            self.artefact()
        );
        // At the first tick at or after its time (one tick when it is due).
        self.run_to(self.until().max(self.now - START + 1));
        match self.rule.artefact(&self.params) {
            ArtefactView::Lying { at, .. } => at,
            other => panic!("not lying: {other:?}"),
        }
    }

    fn take(&mut self, raw: u8) {
        self.send(Event::Take {
            place: place(raw),
            now: t(self.now),
        });
    }

    fn drop_at(&mut self, raw: u8, x: f32) {
        self.send(Event::Drop {
            place: place(raw),
            now: t(self.now),
            ground: at(x),
        });
    }

    fn deliver(&mut self, raw: u8) -> Vec<Action> {
        let team = self.rule.sides().team_of(place(raw)).unwrap();
        let ground = self.params.base(team);
        self.send(Event::Deliver {
            place: place(raw),
            now: t(self.now),
            ground,
        })
    }

    /// When somebody dead now would come back.
    fn respawn(&self) -> Option<Time> {
        self.rule.respawn_at(&self.params, t(self.now), Shift::NONE)
    }

    fn die(&mut self, raw: u8, x: f32) -> Vec<Action> {
        self.send(Event::Died {
            place: place(raw),
            now: t(self.now),
            ground: at(x),
        })
    }

    fn leave(&mut self, raw: u8) {
        self.send(Event::Left {
            place: place(raw),
            now: t(self.now),
            ground: None,
        });
    }

    /// The artefact appears, `raw` takes it and puts it in.
    fn point_for(&mut self, raw: u8) -> Vec<Action> {
        self.spawned();
        self.take(raw);
        self.deliver(raw)
    }

    /// Runs through a delivery's 3 s, the fade and the countdown, to the
    /// next round's hunt; the step the round started at.
    fn next_round(&mut self) -> u32 {
        let Round::Brought { .. } = self.round() else {
            panic!("{:?}", self.round())
        };
        while matches!(self.round(), Round::Brought { .. } | Round::Fade { .. }) {
            self.step();
        }
        let started = self.now;
        while matches!(self.round(), Round::Countdown { .. }) {
            self.step();
        }
        started
    }

    /// The match at `ms` of its time, an artefact due at the next tick,
    /// with these scores.
    fn at_time(seed: u32, ms: u32, score: [u8; 2]) -> Match {
        Match::at_time_with(seed, params(), ms, score)
    }

    fn at_time_with(seed: u32, params: Params, ms: u32, score: [u8; 2]) -> Match {
        let mut game = Match::started_with(seed, params);
        game.now = START + ms;
        game.rule = Rule {
            score,
            // every delivery before was followed by its round
            round_starts: u16::from(score[0]) + u16::from(score[1]),
            stage: Stage::Play(Play {
                start: t(START),
                round: Round::Hunt {
                    clock: Clock::Regular,
                    artefact: Artefact::Waiting {
                        until: t(START + ms),
                    },
                },
            }),
            ..game.rule
        };
        game
    }

    fn over(&self) -> (Outcome, Over) {
        self.rule.result().expect("the match is over")
    }
}

#[test]
fn the_rng_is_046c2s() {
    // s = s x 0x08088405 + 1, the draw the high half of s x range.
    let (rng, draw) = Rng::seeded(0).below(20_000);
    assert_eq!((rng.state(), draw), (1, 0));
    let (rng, draw) = rng.below(20_000);
    assert_eq!((rng.state(), draw), (0x0808_8406, 627));
    let (rng, draw) = rng.below(4);
    assert_eq!((rng.state(), draw), (0xDC6D_AC1F, 3));
    let (rng, draw) = Rng::seeded(0xFFFF_FFFF).below(1000);
    assert_eq!((rng.state(), draw), (0xF7F7_7BFC, 968));
    assert_eq!(Rng::seeded(123).below(0).1, 0);
}

#[test]
fn the_square_root_is_whole_and_rounded_down() {
    for value in [
        0u64,
        1,
        2,
        3,
        4,
        15,
        16,
        17,
        99,
        100,
        1 << 32,
        (1 << 32) - 1,
        u64::from(u32::MAX) * u64::from(u32::MAX),
    ] {
        let root = u128::from(crate::geometry::root_for_tests(value));
        assert!(
            root * root <= u128::from(value) && (root + 1) * (root + 1) > u128::from(value),
            "{value}"
        );
    }
    assert_eq!(
        at(3.0).distance(Position::from_metres([0.0, 4.0, 0.0]).unwrap()),
        5000
    );
}

#[test]
fn a_position_is_a_point_of_a_map() {
    assert_eq!(
        Position::from_metres([1.5, -2.25, 0.0]).unwrap().mm(),
        [1500, -2250, 0]
    );
    assert_eq!(Position::from_metres([f32::NAN, 0.0, 0.0]), None);
    assert_eq!(Position::from_metres([0.0, f32::INFINITY, 0.0]), None);
    assert_eq!(Position::from_metres([0.0, 0.0, 100_001.0]), None);
    assert_eq!(Position::from_mm([0, 0, Position::LIMIT + 1]), None);
    assert_eq!(at(1.0).raised(130).mm(), [1000, 130, 0]);
}

#[test]
fn a_config_is_parsed_or_names_its_fault() {
    let spawners = [Spawner {
        position: [1.0, 2.0, 3.0],
    }];
    let bases = [
        BaseDef {
            team: 0,
            position: [0.0; 3],
            shapes: &CRATE_0,
        },
        BaseDef {
            team: 1,
            position: [50.0, 0.0, 0.0],
            shapes: &CRATE_1,
        },
    ];
    let parse = |raw: RawParams| Params::parse(&raw, &spawners, &bases);
    assert!(parse(RAW).is_ok());
    for (raw, error) in [
        (
            RawParams {
                match_time: 0,
                ..RAW
            },
            ParamError::MatchTime,
        ),
        (
            RawParams {
                time_needed_to_continue_match: 901,
                ..RAW
            },
            ParamError::TimeNeededToContinueMatch,
        ),
        (
            RawParams {
                time_before_new_round: 601,
                ..RAW
            },
            ParamError::TimeBeforeNewRound,
        ),
        (
            RawParams {
                round_countdown_time: 601,
                ..RAW
            },
            ParamError::RoundCountdownTime,
        ),
        (RawParams { score: 0, ..RAW }, ParamError::Score),
        (RawParams { score: 256, ..RAW }, ParamError::Score),
        (
            RawParams {
                respawn_time: 0,
                ..RAW
            },
            ParamError::RespawnTime,
        ),
        (
            RawParams {
                vi_dropped_ttl: 100_000,
                ..RAW
            },
            ParamError::ViDroppedTtl,
        ),
        (
            RawParams {
                drop_lift: f32::NAN,
                ..RAW
            },
            ParamError::DropLift,
        ),
    ] {
        assert_eq!(parse(raw), Err(error));
    }
    assert_eq!(Params::parse(&RAW, &[], &bases), Err(ParamError::Spawners));
    assert_eq!(
        Params::parse(
            &RAW,
            &[Spawner {
                position: [f32::NAN; 3]
            }],
            &bases
        ),
        Err(ParamError::Spawners)
    );
    assert_eq!(
        Params::parse(&RAW, &[spawners[0]; MAX_SPAWNERS + 1], &bases),
        Err(ParamError::Spawners)
    );
    assert_eq!(
        Params::parse(&RAW, &spawners, &bases[..1]),
        Err(ParamError::Bases)
    );
    let same_team = [
        bases[0],
        BaseDef {
            team: 0,
            ..bases[1]
        },
    ];
    assert_eq!(
        Params::parse(&RAW, &spawners, &same_team),
        Err(ParamError::Bases)
    );
    let same_place = [
        bases[0],
        BaseDef {
            team: 1,
            ..bases[0]
        },
    ];
    assert_eq!(
        Params::parse(&RAW, &spawners, &same_place),
        Err(ParamError::Bases)
    );
    let no_shapes = [
        bases[0],
        BaseDef {
            shapes: &[],
            ..bases[1]
        },
    ];
    assert_eq!(
        Params::parse(&RAW, &spawners, &no_shapes),
        Err(ParamError::Bases)
    );
    let bad = [RawShape {
        kind: 3,
        ..CRATE_0[0]
    }];
    let bad_kind = [
        bases[0],
        BaseDef {
            shapes: &bad,
            ..bases[1]
        },
    ];
    assert_eq!(
        Params::parse(&RAW, &spawners, &bad_kind),
        Err(ParamError::Bases)
    );
    let nan = [RawShape {
        half: [f32::NAN; 3],
        ..CRATE_0[0]
    }];
    let bad_half = [
        bases[0],
        BaseDef {
            shapes: &nan,
            ..bases[1]
        },
    ];
    assert_eq!(
        Params::parse(&RAW, &spawners, &bad_half),
        Err(ParamError::Bases)
    );
    let many = [CRATE_0[0]; MAX_SHAPES + 1];
    let too_many = [
        bases[0],
        BaseDef {
            shapes: &many,
            ..bases[1]
        },
    ];
    assert_eq!(
        Params::parse(&RAW, &spawners, &too_many),
        Err(ParamError::Bases)
    );
    for radius in [0.0, -1.0, f32::NAN, 11.0] {
        assert_eq!(
            parse(RawParams {
                usable_radius: radius,
                ..RAW
            }),
            Err(ParamError::UsableRadius)
        );
    }
    // The bases are by team, whatever their order in the config.
    assert_eq!(params().base(Team::One).mm(), [100_000, 0, 0]);
}

#[test]
fn the_artefact_appears_50_to_70_s_after_the_start_at_a_spawner() {
    let mut seen = [false; 4];
    for seed in 0..200 {
        let mut game = Match::started(seed);
        let wait = game.until();
        assert!((50_000..70_000).contains(&wait), "seed {seed}: {wait}");
        // Nothing lies in the world before; it lies there at the first tick at or after.
        game.run_to((wait - 1) / TICK * TICK);
        if !wait.is_multiple_of(TICK) {
            assert_eq!(game.rule.artefact(&game.params), ArtefactView::Nowhere);
        }
        game.run_to(wait);
        let Artefact::Lying {
            spot: Spot::Spawner(index),
            until,
            ..
        } = game.artefact()
        else {
            panic!("{:?}", game.artefact())
        };
        seen[usize::from(index)] = true;
        // vi_not_picked_ttl from the tick it appeared at.
        assert_eq!(until.ms(), game.now + 120_000);
    }
    assert_eq!(seen, [true; 4], "every spawner is drawn");
}

#[test]
fn the_draws_are_the_rngs_in_order() {
    let mut game = Match::started(7);
    let (rng, wait) = Rng::seeded(7).below(20_000);
    assert_eq!(game.until(), 50_000 + wait);
    game.spawned();
    let (rng, index) = rng.below(4);
    assert!(
        matches!(game.artefact(), Artefact::Lying { spot: Spot::Spawner(i), .. } if u32::from(i) == index)
    );
    assert_eq!(game.rule.rng(), rng);
}

#[test]
fn an_artefact_nobody_takes_is_removed_and_another_waited_for() {
    let mut game = Match::started(3);
    game.spawned();
    let appeared = game.now;
    game.run_for(120_000 - TICK);
    assert!(matches!(game.artefact(), Artefact::Lying { .. }));
    game.run_for(TICK);
    let Artefact::Waiting { until } = game.artefact() else {
        panic!("{:?}", game.artefact())
    };
    assert_eq!(game.now, appeared + 120_000);
    assert!((50_000..70_000).contains(&(until.ms() - game.now)));
    // No round ends: removed, it is waited for again in the same round.
    assert_eq!(game.clock(), Clock::Regular);
    // The same spawner may come again: nothing remembers the last.
    game.spawned();
}

#[test]
fn anybody_takes_it_and_the_carrier_is_the_rules() {
    let mut game = Match::started(1);
    game.take(0);
    assert_eq!(
        game.rule.carrier(),
        None,
        "nothing to take while it is waited for"
    );
    game.spawned();
    game.take(10);
    assert_eq!(game.rule.carrier(), Some(place(10)));
    assert_eq!(
        game.rule.artefact(&game.params),
        ArtefactView::Carried {
            by: place(10),
            team: Team::One
        }
    );
    // One at a time: the second press of the same tick takes nothing.
    game.take(0);
    assert_eq!(game.rule.carrier(), Some(place(10)));
    // A place that is not in the match takes nothing.
    let mut game = Match::started(1);
    game.spawned();
    game.take(19);
    assert_eq!(game.rule.carrier(), None);
}

#[test]
fn a_dropped_artefact_lies_a_little_over_the_ground_for_60_s_each_time() {
    let mut game = Match::started(1);
    game.spawned();
    game.take(0);
    // Somebody else's Drop key drops nothing.
    game.drop_at(1, 30.0);
    assert_eq!(game.rule.carrier(), Some(place(0)));
    game.run_for(10_000);
    game.drop_at(0, 30.0);
    let Artefact::Lying {
        spot: Spot::Dropped(lies),
        until,
        ..
    } = game.artefact()
    else {
        panic!("{:?}", game.artefact())
    };
    assert_eq!(lies.mm(), [30_000, 130, 0]);
    assert_eq!(until.ms(), game.now + 60_000);
    // Anyone takes it the same way; the whole time again at each drop.
    game.run_for(59_000);
    game.take(11);
    game.run_for(5_000);
    game.drop_at(11, 35.0);
    assert_eq!(game.until(), game.now - START + 60_000);
    game.run_for(60_000);
    assert!(
        matches!(game.artefact(), Artefact::Waiting { .. }),
        "removed: a new wait"
    );
}

#[test]
fn a_carriers_death_and_leaving_drop_it() {
    let mut game = Match::started(1);
    game.spawned();
    game.take(2);
    game.die(2, 44.0);
    assert!(
        matches!(game.artefact(), Artefact::Lying { spot: Spot::Dropped(p), .. } if p.mm() == [44_000, 130, 0])
    );
    game.take(12);
    game.send(Event::Left {
        place: place(12),
        now: t(game.now),
        ground: Some(at(45.0)),
    });
    assert!(
        matches!(game.artefact(), Artefact::Lying { spot: Spot::Dropped(p), .. } if p.mm() == [45_000, 130, 0])
    );
    assert_eq!(game.rule.sides().team_of(place(12)), None);
    // A carrier who leaves from nowhere takes it out of the world.
    game.take(13);
    game.leave(13);
    assert!(matches!(game.artefact(), Artefact::Waiting { .. }));
}

#[test]
fn a_delivery_ends_the_round_3_s_then_a_fade_then_a_frozen_countdown() {
    let mut game = Match::started(1);
    game.spawned();
    game.take(0);
    // Only the carrier delivers.
    assert!(game.deliver(1).is_empty());
    assert_eq!(game.rule.score(), [0, 0]);
    let actions = game.deliver(0);
    let delivered = game.now;
    assert_eq!(game.rule.score(), [1, 0]);
    assert_eq!(actions[0], Action::Put { place: place(0) });
    assert!(
        actions[1..]
            .iter()
            .all(|action| matches!(action, Action::Bring { .. }))
    );
    // 1. 3 s: play goes on, nobody respawns, "Round restarts in".
    assert_eq!(
        game.round(),
        Round::Brought {
            until: t(delivered + 3000)
        }
    );
    assert_eq!(game.rule.artefact(&game.params), ArtefactView::Nowhere);
    let hud = game.rule.hud(&game.params, t(delivered + 1000));
    assert_eq!(
        (hud.phase, hud.phase_ms, hud.phase_total_ms, hud.after),
        (Phase::Brought, 2000, 3000, Some(After::NewRound))
    );
    assert!(game.rule.respawns() && !game.rule.frozen());
    let mut held = 0;
    while game.now < delivered + 3000 {
        assert_eq!(game.step(), Spawning::Held);
        held += 1;
    }
    assert_eq!(held, 60);
    // 3. the 1 s fade: still held; at its end everyone respawns.
    assert_eq!(
        game.round(),
        Round::Fade {
            until: t(delivered + 4000),
            sudden: None
        }
    );
    assert_eq!(game.rule.hud(&game.params, t(game.now)).phase, Phase::Fade);
    while game.now < delivered + 3950 {
        assert_eq!(game.step(), Spawning::Held);
    }
    assert!(!game.rule.frozen());
    assert_eq!(game.step(), Spawning::Everyone);
    // 4. the 5 s countdown, everyone frozen, the respawns their own again.
    assert_eq!(
        game.round(),
        Round::Countdown {
            until: t(delivered + 9000),
            clock: Clock::Regular
        }
    );
    let hud = game.rule.hud(&game.params, t(game.now + 1000));
    assert_eq!(
        (hud.phase, hud.phase_ms, hud.phase_total_ms),
        (Phase::Countdown, 4000, 5000)
    );
    while game.now < delivered + 8950 {
        assert!(game.rule.frozen());
        assert_eq!(game.step(), Spawning::Native);
    }
    assert!(game.rule.frozen());
    game.step();
    // 5. the next round: the 50 to 70 s wait from its start.
    assert!(!game.rule.frozen());
    assert!((50_000..70_000).contains(&(game.until() - (delivered + 9000 - START))));
    // The scores and the match time carry over.
    let hud = game.rule.hud(&game.params, t(game.now));
    assert_eq!(
        (hud.score, hud.clock_ms),
        ([1, 0], 900_000 - (game.now - START))
    );
    // A round with no delivery spawns nobody of the rule's.
    game.log.clear();
    game.spawned();
    assert!(game.log.is_empty());
}

#[test]
fn respawns_are_held_from_the_delivery_to_the_round_start() {
    let mut game = Match::started(1);
    game.spawned();
    game.take(0);
    // A death's time is the wave's in every round phase: a time that falls
    // in the restart is held there and given at the round's start.
    game.die(11, 1.0);
    let before = game.respawn().unwrap();
    game.take(0);
    game.deliver(0);
    assert_eq!(game.respawn(), Some(before));
    assert!(game.rule.respawns());
    let started = game.next_round();
    assert_eq!(game.rule.spawning(t(started + TICK)), Spawning::Native);
    // Spawning only answers the step it is asked for: a fade not yet over
    // holds, one over gives everyone.
    let mut game = Match::started(1);
    game.point_for(0);
    while matches!(game.round(), Round::Brought { .. }) {
        game.step();
    }
    let Round::Fade { until, .. } = game.round() else {
        panic!()
    };
    assert_eq!(game.rule.spawning(t(until.ms() - 1)), Spawning::Held);
    assert_eq!(game.rule.spawning(until), Spawning::Everyone);
    assert_eq!(game.rule.spawning(t(until.ms() + 1)), Spawning::Everyone);
}

#[test]
fn the_first_to_five_wins_3_s_after_the_fifth_delivery() {
    let mut game = Match::at_time(11, 0, [0, 4]);
    game.point_for(10);
    assert_eq!(game.rule.score(), [0, 5]);
    let hud = game.rule.hud(&game.params, t(game.now + 1000));
    assert_eq!(
        (hud.phase, hud.phase_ms, hud.after, hud.sudden_death),
        (Phase::Brought, 2000, Some(After::End), SuddenShown::Off)
    );
    // Nobody respawns in the 3 s, and none follows.
    for _ in 0..59 {
        assert_eq!(game.step(), Spawning::Held);
    }
    assert_eq!(game.rule.result(), None);
    game.step();
    let (outcome, over) = game.over();
    assert_eq!(
        (outcome, over.reason),
        (Outcome::Winner(Team::One), Reason::Collected)
    );
    assert!(game.rule.completed(t(game.now)));
    assert_eq!(game.rule.spawning(t(game.now + TICK)), Spawning::Native);
    // Four points are no end.
    let mut game = Match::at_time(11, 0, [3, 0]);
    game.point_for(0);
    assert_eq!(
        game.rule.hud(&game.params, t(game.now)).after,
        Some(After::NewRound)
    );
}

#[test]
fn at_900_s_the_higher_score_wins_at_once() {
    let mut game = Match::at_time(1, 899_900, [2, 1]);
    game.spawned();
    game.take(0);
    game.run_to(899_950);
    assert_eq!(game.rule.result(), None);
    game.run_to(900_000);
    let (outcome, over) = game.over();
    assert_eq!(
        (outcome, over.reason),
        (Outcome::Winner(Team::Zero), Reason::TimeOut)
    );
}

#[test]
fn at_900_s_equal_scores_go_to_sudden_death_for_180_s_then_a_draw() {
    let mut game = Match::at_time(1, 899_900, [2, 2]);
    game.run_to(900_000);
    let Clock::Sudden(sudden) = game.clock() else {
        panic!("sudden death")
    };
    assert_eq!(sudden.from(), t(START + 900_000));
    let hud = game.rule.hud(&game.params, t(game.now + 30_000));
    assert_eq!(
        (hud.sudden_death, hud.clock_ms, hud.clock_total_ms),
        (SuddenShown::On, 150_000, 180_000)
    );
    assert_eq!((hud.alive, hud.total), ([4, 4], [4, 4]));
    // The artefact goes on as it was.
    assert!(matches!(game.artefact(), Artefact::Lying { .. }));
    game.run_to(900_000 + 179_950);
    assert_eq!(game.rule.result(), None);
    game.run_to(900_000 + 180_000);
    let (outcome, over) = game.over();
    assert_eq!((outcome, over.reason), (Outcome::Draw, Reason::TimeOut));
    assert_eq!(game.rule.score(), [2, 2]);
}

#[test]
fn in_sudden_death_nobody_respawns_and_a_wiped_team_gives_a_point() {
    let mut game = Match::at_time(1, 899_000, [1, 1]);
    // A death before it is on is given its wave, and still comes.
    assert!(game.rule.respawns() && game.respawn().is_some());
    game.die(3, 5.0);
    game.run_to(900_000);
    assert_eq!(
        game.rule.standing(Team::Zero).len(),
        4,
        "dead before it began: still in"
    );
    assert!(!game.rule.respawns() && game.respawn().is_none());
    for raw in 10..13 {
        game.die(raw, 50.0);
    }
    assert_eq!(game.rule.standing(Team::One).len(), 1);
    assert_eq!(game.rule.score(), [1, 1]);
    game.die(13, 50.0);
    assert_eq!(game.rule.score(), [2, 1], "team 1 has nobody left");
    assert_eq!(game.rule.result(), None);
    game.step();
    assert_eq!(
        (game.over().0, game.over().1.reason()),
        (Outcome::Winner(Team::Zero), Reason::Wiped),
        "its end says the other team was wiped"
    );
}

#[test]
fn in_sudden_death_a_leaver_counts_as_a_death_and_a_joiner_is_in() {
    let mut game = Match::at_time(1, 900_000, [0, 0]);
    game.step();
    for raw in 0..3 {
        game.leave(raw);
    }
    assert_eq!(game.rule.standing(Team::Zero).len(), 1);
    game.send(Event::Entered {
        place: place(5),
        team: Team::Zero,
        now: t(game.now),
    });
    game.die(3, 1.0);
    assert_eq!(game.rule.score(), [0, 0], "the joiner stands");
    game.leave(5);
    assert_eq!(game.rule.score(), [0, 1]);
}

#[test]
fn sudden_death_that_starts_with_a_team_gone_ends() {
    let mut game = Match::at_time(1, 899_000, [0, 0]);
    for raw in 10..14 {
        game.leave(raw);
    }
    assert_eq!(
        game.rule.score(),
        [0, 0],
        "outside sudden death an empty team costs nothing"
    );
    game.run_to(900_000);
    assert_eq!(game.rule.score(), [1, 0]);
    game.step();
    assert_eq!(game.over().0, Outcome::Winner(Team::Zero));
}

#[test]
fn in_sudden_death_a_delivery_wins_at_once() {
    let mut game = Match::at_time(1, 899_950, [3, 3]);
    game.run_to(900_000);
    game.take(12);
    let actions = game.deliver(12);
    assert!(actions.contains(&Action::Put { place: place(12) }));
    let (outcome, over) = game.over();
    assert_eq!(
        (outcome, over.reason, game.rule.score()),
        (Outcome::Winner(Team::One), Reason::TimeOut, [3, 4])
    );
}

#[test]
fn a_delivery_with_less_than_90_s_left_starts_no_new_round() {
    // One test, at the delivery: 90 s left exactly still plays a round.
    let mut game = Match::at_time(1, 809_950, [1, 0]);
    game.point_for(0);
    assert_eq!(game.now, START + 810_000);
    assert_eq!(
        game.rule.hud(&game.params, t(game.now)).after,
        Some(After::NewRound)
    );

    // Scores differ: the match ends 3 s later.
    let mut game = Match::at_time(1, 810_000, [1, 0]);
    game.point_for(0);
    assert_eq!(game.now, START + 810_050);
    assert_eq!(
        game.rule.hud(&game.params, t(game.now)).after,
        Some(After::End)
    );
    // Nobody respawns in the 3 s: they end the match.
    for _ in 0..59 {
        assert_eq!(game.step(), Spawning::Held);
    }
    assert_eq!(game.rule.result(), None);
    game.step();
    let (outcome, over) = game.over();
    assert_eq!(
        (outcome, over.reason),
        (Outcome::Winner(Team::Zero), Reason::Collected)
    );
}

#[test]
fn a_late_delivery_that_equalises_plays_a_round_in_sudden_death() {
    // 0.46c2's edge (a draw without sudden death at 90 to 93 s left) is not kept.
    for ms in [810_050, 811_500, 850_000, 899_950] {
        let mut game = Match::at_time(1, ms - 50, [1, 0]);
        game.point_for(10);
        let delivered = game.now;
        assert_eq!(game.rule.score(), [1, 1]);
        let from = t(delivered + 3000 + 5000);
        let hud = game.rule.hud(&game.params, t(game.now));
        assert_eq!(
            (hud.sudden_death, hud.after, hud.phase_ms),
            (SuddenShown::Armed, Some(After::SuddenDeath { from }), 3000),
            "at {ms}"
        );
        // Armed is not on: the 3 s are of the match time.
        assert!(game.rule.respawns());
        while matches!(game.round(), Round::Brought { .. }) {
            game.step();
        }
        // On from the fade: deaths get no time, its fallen are not counted.
        assert_eq!(
            game.round(),
            Round::Fade {
                until: t(delivered + 4000),
                sudden: Some(from)
            }
        );
        assert!(!game.rule.respawns() && game.respawn().is_none());
        game.die(0, 1.0);
        assert_eq!(game.rule.standing(Team::Zero).len(), 4);
        // Everyone respawns at the fade's end; the countdown counts the fallen.
        while matches!(game.round(), Round::Fade { .. }) {
            assert_ne!(game.step(), Spawning::Native);
        }
        assert!(game.rule.frozen());
        game.die(1, 1.0);
        assert_eq!(game.rule.standing(Team::Zero).len(), 3);
        while game.rule.frozen() {
            game.step();
        }
        assert!(matches!(
            game.clock(),
            Clock::Sudden(sudden) if sudden.from() == from && sudden.fallen().len() == 1
        ));
        // Its 180 s from 8 s after the delivery.
        let hud = game.rule.hud(&game.params, t(delivered + 9000));
        assert_eq!((hud.clock_ms, hud.alive), (179_000, [3, 4]));
        game.run_to(from.ms() - START + 179_950);
        assert_eq!(game.rule.result(), None, "at {ms}");
        game.step();
        assert_eq!(game.over().0, Outcome::Draw);
    }
}

#[test]
fn the_match_time_ends_in_a_hunt_only() {
    // With no time needed to go on, a round can start after 900 s: the
    // match time's test is the hunt's (0.46c2's states 2 to 5), at its
    // first step.
    let params = Params::parse(
        &RawParams {
            time_needed_to_continue_match: 0,
            ..RAW
        },
        &spawners(),
        &BASES,
    )
    .unwrap();
    for (before, ends) in [([0, 0], true), ([0, 1], false)] {
        let mut game = Match::at_time_with(1, params, 899_000, before);
        game.point_for(0);
        assert_eq!(
            game.rule.hud(&game.params, t(game.now)).after,
            Some(After::NewRound)
        );
        let started = game.next_round();
        assert!(started - START > 900_000);
        assert_eq!(game.clock(), Clock::Regular, "the countdown has no test");
        game.step();
        if ends {
            assert_eq!(game.over().1.reason(), Reason::TimeOut);
        } else {
            assert!(matches!(game.clock(), Clock::Sudden(_)));
        }
    }
}

#[test]
fn respawns_come_in_waves_of_15_s_never_sooner_than_7_s() {
    let game = Match::started(1);
    let respawn = |death: u32, shift: i32| {
        let at = game
            .rule
            .respawn_at(&game.params, t(START + death), Shift::new(shift));
        at.expect("a death in play respawns").ms() - START
    };
    // The first wave at or after the death + 7 s.
    assert_eq!(respawn(0, 0), 15_000);
    assert_eq!(respawn(8_000, 0), 15_000);
    assert_eq!(respawn(8_001, 0), 30_000);
    assert_eq!(respawn(23_000, 0), 30_000);
    assert_eq!(respawn(100_000, 0), 120_000);
    // The modifier's part of 15 s (-20 %: 3 s) moves the wave, never under the 7 s.
    assert_eq!(respawn(0, -3_000), 12_000);
    assert_eq!(respawn(7_000, -3_000), 14_000);
    assert_eq!(
        respawn(8_000, -3_000),
        15_000,
        "wave 15 s - 3 s is sooner than death + 7 s"
    );
    assert_eq!(respawn(0, 7_500), 22_500);
    assert_eq!(respawn(0, -15_000), 7_000);
    assert_eq!(respawn(1_010, i32::MIN), 8_010);
    // A death a step before the start (an event's time is the step before).
    assert_eq!(
        game.rule
            .respawn_at(&game.params, t(START - 50), Shift::NONE),
        Some(t(START + 15_000))
    );
    // Before the start and after the end the respawn rule's own time stands.
    assert_eq!(
        Rule::new(1).respawn_at(&game.params, t(5), Shift::NONE),
        None
    );
    let mut over = Match::at_time(1, 899_950, [1, 0]);
    over.step();
    assert_eq!(over.respawn(), None);
}

#[test]
fn the_bring_award_is_shared_by_each_carriers_part_of_the_way() {
    let mut game = Match::started(0);
    // Wherever it appears, the carriers below walk the rest of the way to
    // team 1's base at 100 m.
    let from = game.spawned().mm()[0] as f32 / 1000.0;
    let quarter = (100.0 - from) / 4.0;
    game.take(10);
    game.drop_at(10, from + quarter);
    game.take(11);
    game.drop_at(11, from + 4.0 * quarter - 0.001);
    game.take(12);
    let actions = game.deliver(12);
    let shares: Vec<(u8, u16)> = actions
        .iter()
        .filter_map(|action| match action {
            Action::Bring { place, hundredths } => Some((place.get(), *hundredths)),
            _ => None,
        })
        .collect();
    // A quarter, three quarters, and the last step's nothing.
    assert_eq!(shares, [(10, 25), (11, 75)]);
    assert!(actions.contains(&Action::Put { place: place(12) }));
}

#[test]
fn the_way_counts_from_where_the_other_team_left_it() {
    let params = params();
    // It appears in the middle; team 0 carries it 30 m home, team 1 takes
    // it back 20 m, team 0 carries it home from there.
    let mut way = Way::begin(&params, at(50.0));
    assert_eq!(way.left(), [WHOLE / 2, WHOLE / 2]);
    way = way.touched(&params, place(0), Team::Zero, at(50.0));
    way = way.touched(&params, place(0), Team::Zero, at(20.0));
    way = way.touched(&params, place(10), Team::One, at(20.0));
    way = way.touched(&params, place(10), Team::One, at(40.0));
    way = way.touched(&params, place(1), Team::Zero, at(40.0));
    way = way.touched(&params, place(1), Team::Zero, at(0.0));
    // In thousandths of the way between the bases.
    let part = |raw: u8| (way.carried()[usize::from(raw)] * 1000 + WHOLE / 2) / WHOLE;
    // Place 0: 50 -> 20 m; place 10: 20 -> 40 m; place 1: 40 -> 0 m.
    assert_eq!((part(0), part(10), part(1)), (300, 200, 400));
    let sides = Sides::new(
        Roster::from_bits(0b11).unwrap(),
        Roster::from_bits(1 << 10).unwrap(),
    )
    .unwrap();
    let shares: Vec<(u8, u16)> = way
        .shares(&sides, Team::Zero)
        .map(|(place, share)| (place.get(), share))
        .collect();
    assert_eq!(shares, [(0, 43), (1, 57)]);
    // A step away from one's base is no part of the way.
    let back = Way::begin(&params, at(50.0)).touched(&params, place(0), Team::Zero, at(80.0));
    assert_eq!(back.carried()[0], 0);
    // One who left has no share; the others share the whole.
    let alone: Vec<(u8, u16)> = way
        .forgetting(place(0))
        .shares(&sides, Team::Zero)
        .map(|(place, share)| (place.get(), share))
        .collect();
    assert_eq!(alone, [(1, 100)]);
}

#[test]
fn the_hud_shows_the_rule() {
    let mut game = Match::started(1);
    let params = game.params;
    let hud = Rule::new(1).hud(&params, t(5));
    assert_eq!(
        (hud.phase, hud.clock_ms, hud.score, hud.goal),
        (Phase::Pregame, 0, [0, 0], 5)
    );
    let hud = game.rule.hud(&params, t(START + 1500));
    assert_eq!(
        (
            hud.phase,
            hud.clock_ms,
            hud.clock_total_ms,
            hud.sudden_death
        ),
        (Phase::Waiting, 898_500, 900_000, SuddenShown::Off)
    );
    assert_eq!(
        (hud.phase_ms, hud.after, hud.alive, hud.total),
        (0, None, [0, 0], [4, 4])
    );
    game.spawned();
    let hud = game.rule.hud(&params, t(game.now + 20_000));
    assert_eq!(
        (hud.phase, hud.phase_ms, hud.phase_total_ms),
        (Phase::Spawned, 100_000, 120_000)
    );
    assert!(matches!(
        hud.artefact,
        ArtefactView::Lying {
            spot: Spot::Spawner(_),
            ..
        }
    ));
    game.take(1);
    assert_eq!(game.rule.hud(&params, t(game.now)).phase, Phase::Carried);
    game.drop_at(1, 7.0);
    let hud = game.rule.hud(&params, t(game.now + 1000));
    assert_eq!(
        (hud.phase, hud.phase_ms, hud.phase_total_ms),
        (Phase::Dropped, 59_000, 60_000)
    );
    game.take(1);
    game.deliver(1);
    let hud = game.rule.hud(&params, t(game.now));
    assert_eq!((hud.score, hud.artefact), ([1, 0], ArtefactView::Nowhere));
    game.next_round();
    game.run_for(1_000_000);
    assert_eq!(game.rule.hud(&params, t(game.now)).phase, Phase::Finished);
}

#[test]
fn the_viewer_is_told_what_happened_once() {
    let mut game = Match::started(1);
    let own = Viewer {
        place: Some(place(0)),
        team: Some(Team::Zero),
    };
    let enemy = Viewer {
        place: Some(place(10)),
        team: Some(Team::One),
    };
    let nobody = Viewer {
        place: None,
        team: None,
    };
    let mut seen = game.rule;
    let mut told = |game: &Match| {
        let cues = Cue::between(&seen, &game.rule);
        seen = game.rule;
        cues
    };
    assert_eq!(told(&game), [None, None]);
    game.spawned();
    let [Some(cue), None] = told(&game) else {
        panic!()
    };
    assert_eq!(
        (cue, cue.line(own), cue.message(own)),
        (Cue::Spawned, Some(Line::Spawned), Some(Message::Spawned))
    );
    assert_eq!(
        cue.line(nobody),
        None,
        "no line for a viewer without a team"
    );
    assert_eq!(told(&game), [None, None], "nothing twice");
    game.take(0);
    let [Some(cue), None] = told(&game) else {
        panic!()
    };
    assert_eq!(
        (cue.line(own), cue.message(own)),
        (Some(Line::PickedUp), Some(Message::YouPicked))
    );
    assert_eq!(
        (cue.line(enemy), cue.message(enemy)),
        (Some(Line::PickedUpByEnemy), None)
    );
    assert_eq!(
        cue.message(Viewer {
            place: Some(place(1)),
            ..own
        }),
        None,
        "the picker alone"
    );
    game.drop_at(0, 20.0);
    let [Some(cue), None] = told(&game) else {
        panic!()
    };
    assert_eq!(
        (cue.line(own), cue.line(enemy), cue.message(own)),
        (Some(Line::Dropped), Some(Line::DroppedByEnemy), None)
    );
    game.run_for(60_000);
    let [Some(cue), None] = told(&game) else {
        panic!()
    };
    assert_eq!(
        (cue, cue.line(own), cue.message(nobody)),
        (Cue::Destroyed, None, Some(Message::Destroyed))
    );
    game.spawned();
    told(&game);
    game.take(11);
    told(&game);
    game.deliver(11);
    let [Some(cue), None] = told(&game) else {
        panic!()
    };
    assert_eq!(
        (cue.line(own), cue.message(own)),
        (Some(Line::BroughtByEnemy), Some(Message::EnemyDelivered))
    );
    assert_eq!(
        (cue.line(enemy), cue.message(enemy)),
        (Some(Line::Brought), Some(Message::TeamDelivered))
    );
    // The round's restart says nothing of the artefact.
    game.next_round();
    assert_eq!(told(&game), [None, None]);

    // A delivery that ends the match in sudden death tells both.
    let mut game = Match::at_time(1, 900_000, [0, 0]);
    game.step();
    game.take(0);
    let seen = game.rule;
    game.deliver(0);
    let [Some(delivered), Some(ended)] = Cue::between(&seen, &game.rule) else {
        panic!()
    };
    assert_eq!(delivered, Cue::Delivered { team: Team::Zero });
    assert_eq!(ended.message(own), Some(Message::Won(Reason::TimeOut)));
    assert_eq!(ended.message(enemy), Some(Message::Lost(Reason::TimeOut)));
    assert_eq!(ended.line(own), None);
    // A wiped team's point is no delivery, whoever carries.
    let mut game = Match::at_time(1, 900_000, [0, 0]);
    game.step();
    game.take(0);
    let seen = game.rule;
    for raw in 10..14 {
        game.die(raw, 1.0);
    }
    assert_eq!(game.rule.score(), [1, 0]);
    assert_eq!(Cue::between(&seen, &game.rule), [None, None]);
    // A game that joins late compares the world it got with itself.
    assert_eq!(Cue::between(&game.rule, &game.rule), [None, None]);
}

#[test]
fn before_the_start_and_after_the_end_nothing_moves() {
    let params = params();
    let idle = Rule::new(9);
    for event in [
        Event::Tick { now: t(100) },
        Event::Take {
            place: place(0),
            now: t(100),
        },
        Event::Drop {
            place: place(0),
            now: t(100),
            ground: at(0.0),
        },
        Event::Deliver {
            place: place(0),
            now: t(100),
            ground: at(0.0),
        },
        Event::Died {
            place: place(0),
            now: t(100),
            ground: at(0.0),
        },
        Event::Left {
            place: place(0),
            now: t(100),
            ground: None,
        },
    ] {
        let (rule, actions) = idle.step(&params, event);
        assert_eq!(rule, idle, "{event:?}");
        assert!(actions.is_empty(), "{event:?}");
    }
    assert_eq!(idle.spawning(t(100)), Spawning::Native);
    assert!(!idle.frozen());
    // The match starts once (a world's load sends the start again).
    let mut game = Match::started(9);
    let started = game.rule;
    game.send(Event::Start {
        now: t(START + 500),
    });
    assert_eq!(game.rule, started);
    // Over: nothing but who is in the match changes, and no death is the rule's.
    let mut game = Match::at_time(1, 899_950, [1, 0]);
    game.step();
    let over = game.rule;
    assert!(over.result().is_some());
    game.run_for(10_000);
    game.take(0);
    assert!(game.die(0, 1.0).is_empty());
    assert_eq!(game.rule, over);
    assert!(!over.completed(t(game.now - 20_000)) && over.completed(t(game.now)));
}

#[test]
fn nothing_of_the_artefact_happens_in_a_restart() {
    let mut game = Match::started(4);
    game.point_for(0);
    let restart = game.rule;
    for event in [
        Event::Take {
            place: place(1),
            now: t(game.now),
        },
        Event::Drop {
            place: place(0),
            now: t(game.now),
            ground: at(1.0),
        },
        Event::Deliver {
            place: place(0),
            now: t(game.now),
            ground: at(0.0),
        },
        Event::Died {
            place: place(2),
            now: t(game.now),
            ground: at(0.0),
        },
    ] {
        assert!(game.send(event).is_empty());
    }
    assert_eq!(game.rule, restart);
}

#[test]
fn an_event_may_be_a_step_older_than_the_last_tick() {
    // The game gives a death the time of the step before (its dispatcher
    // 0x2D1EA0): the rule's times are each event's own.
    let mut game = Match::started(1);
    game.spawned();
    game.take(0);
    game.run_for(1000);
    game.send(Event::Died {
        place: place(0),
        now: t(game.now - TICK),
        ground: at(9.0),
    });
    assert_eq!(game.until(), game.now - TICK + 60_000 - START);
    game.take(0);
    game.send(Event::Deliver {
        place: place(0),
        now: t(game.now - TICK),
        ground: at(0.0),
    });
    assert_eq!(
        game.round(),
        Round::Brought {
            until: t(game.now - TICK + 3000)
        }
    );
}

#[test]
fn the_wire_is_parsed_and_never_trusted() {
    let mut game = Match::started(5);
    let params = game.params;
    let bytes_of = |rule: &Rule| {
        let mut bytes = [0u8; MAX_BYTES];
        let len = rule.write(&mut bytes).unwrap();
        bytes[..len].to_vec()
    };
    assert_eq!(bytes_of(&Rule::new(5)).len(), 19 + 1);
    // Play, waiting: the head, the start, the round, the clock, its time.
    let waiting = bytes_of(&game.rule);
    assert_eq!(waiting.len(), 19 + 4 + 1 + 1 + 4 + 1);
    assert_eq!(waiting[29], 29, "the size of what is before it");
    assert_eq!(waiting[..4], game.rule.rng().state().to_le_bytes());
    assert_eq!(waiting[10..18], [0x0F, 0, 0, 0, 0, 0x3C, 0, 0], "the sides");
    assert_eq!((waiting[18], waiting[23], waiting[24]), (1, 2, 0));
    assert_eq!(
        Rule::read(&params, &waiting[..waiting.len() - 1]),
        Err(WireError::Short)
    );
    let broken = |at: usize, value: u8| {
        let mut bytes = waiting.clone();
        bytes[at] = value;
        Rule::read(&params, &bytes).map(|_| ())
    };
    assert_eq!(broken(4, 6), Err(WireError::Score), "over the goal");
    assert_eq!(broken(6, 1), Err(WireError::RoundStarts), "no delivery yet");
    assert_eq!(
        broken(4, 5),
        Err(WireError::Score),
        "the goal while the match time runs"
    );
    assert_eq!(broken(12, 0x10), Err(WireError::Sides), "place 20");
    assert_eq!(
        broken(14, 1),
        Err(WireError::Sides),
        "a place of both teams"
    );
    assert_eq!(broken(18, 3), Err(WireError::Stage));
    assert_eq!(broken(23, 9), Err(WireError::Round));
    assert_eq!(broken(23, 1), Err(WireError::Round));
    assert_eq!(broken(24, 2), Err(WireError::Clock));
    assert_eq!(
        broken(28, 0x80),
        Err(WireError::Time),
        "a time before the start"
    );
    assert_eq!(
        broken(28, 0x7F),
        Err(WireError::Time),
        "a time past the horizon"
    );
    assert_eq!(broken(29, 28), Err(WireError::Size));
    // In a longer buffer the rule's bytes are counted; before the kd stats
    // it is found from its end.
    let mut longer = waiting.clone();
    longer.extend([0xAA; 7]);
    assert_eq!(Rule::read(&params, &longer), Ok((game.rule, waiting.len())));
    let mut after = std::vec![0x55u8; 37];
    after.extend(&waiting);
    assert_eq!(Rule::read_back(&params, &after), Ok((game.rule, 37)));
    assert_eq!(
        Rule::read_back(&params, &waiting[1..]),
        Err(WireError::Short)
    );
    assert_eq!(Rule::read_back(&params, &[]), Err(WireError::Short));

    // Lying at a spawner: its index is one of the map's.
    game.spawned();
    let mut lying = bytes_of(&game.rule);
    assert_eq!(lying.len(), 30 + 1 + 88);
    assert_eq!((lying[23], lying[24]), (3, 0));
    lying[25] = 4;
    assert_eq!(
        Rule::read(&params, &lying).map(|_| ()),
        Err(WireError::Spawner)
    );
    // Carried: by a place of the match.
    game.take(2);
    let mut carried = bytes_of(&game.rule);
    assert_eq!(
        (carried.len(), carried[23], carried[25]),
        (26 + 88 + 1, 4, 2)
    );
    carried[25] = 9;
    assert_eq!(
        Rule::read(&params, &carried).map(|_| ()),
        Err(WireError::Carrier)
    );
    carried[25] = 2;
    carried[26..30].copy_from_slice(&(WHOLE + 1).to_le_bytes());
    assert_eq!(
        Rule::read(&params, &carried).map(|_| ()),
        Err(WireError::Way)
    );
    // Dropped: a point of a map.
    game.drop_at(2, 12.5);
    let mut dropped = bytes_of(&game.rule);
    assert_eq!((dropped.len(), dropped[23]), (25 + 12 + 4 + 88 + 1, 5));
    assert_eq!(
        dropped[25..37],
        [0xD4, 0x30, 0, 0, 130, 0, 0, 0, 0, 0, 0, 0]
    );
    dropped[28] = 0x7F;
    assert_eq!(
        Rule::read(&params, &dropped).map(|_| ()),
        Err(WireError::Position)
    );

    // The 3 s after a delivery: no clock, a time.
    game.take(2);
    game.deliver(2);
    let brought = bytes_of(&game.rule);
    assert_eq!((brought.len(), brought[23]), (19 + 4 + 1 + 4 + 1, 6));
    assert_eq!(brought[24..28], (game.now + 3000).to_le_bytes());
    // The goal is a state of these 3 s only, of one team.
    let mut goal = brought.clone();
    goal[4] = 5;
    goal[6] = 4; // the round starts four earlier deliveries give
    assert!(Rule::read(&params, &goal).is_ok());
    goal[5] = 5;
    assert_eq!(
        Rule::read(&params, &goal).map(|_| ()),
        Err(WireError::Score)
    );
    // The fade and the countdown: a clock and a time.
    while matches!(game.round(), Round::Brought { .. }) {
        game.step();
    }
    let fade = bytes_of(&game.rule);
    assert_eq!((fade.len(), fade[23], fade[24]), (30, 7, 0));
    assert_eq!(
        (game.rule.round_starts(), fade[6]),
        (0, 0),
        "its round still to start"
    );
    while matches!(game.round(), Round::Fade { .. }) {
        game.step();
    }
    let countdown = bytes_of(&game.rule);
    assert_eq!((countdown.len(), countdown[23], countdown[24]), (30, 8, 0));
    assert_eq!(
        (game.rule.round_starts(), countdown[6]),
        (1, 1),
        "the fade's end started it"
    );
    let mut early = countdown.clone();
    early[6] = 0;
    assert_eq!(
        Rule::read(&params, &early).map(|_| ()),
        Err(WireError::RoundStarts)
    );

    // Sudden death: the fallen are of the match, and no team stands wiped
    // with equal scores.
    let mut game = Match::at_time(1, 900_000, [0, 0]);
    game.step();
    game.die(0, 1.0);
    let sudden = bytes_of(&game.rule);
    assert_eq!((sudden[24], &sudden[29..33]), (1, &[1u8, 0, 0, 0][..]));
    let fallen = |bits: u32| {
        let mut bytes = sudden.clone();
        bytes[29..33].copy_from_slice(&bits.to_le_bytes());
        Rule::read(&params, &bytes).map(|_| ())
    };
    assert_eq!(fallen(0b0111), Ok(()));
    assert_eq!(fallen(0b1111), Err(WireError::Wiped));
    assert_eq!(fallen(1 << 4), Err(WireError::Fallen), "not in the match");
    assert_eq!(fallen(1 << 20), Err(WireError::Fallen));
    let mut apart = sudden.clone();
    apart[4] = 2;
    assert_eq!(
        Rule::read(&params, &apart).map(|_| ()),
        Err(WireError::Score),
        "a wipe makes the scores 1 apart, no more"
    );
    // The carrier is not one of the fallen.
    game.take(1);
    let mut carried = bytes_of(&game.rule);
    assert_eq!((carried[23], carried[33]), (4, 1));
    carried[33] = 0;
    assert_eq!(
        Rule::read(&params, &carried).map(|_| ()),
        Err(WireError::Carrier)
    );
    // A fade in sudden death has its start and no fallen.
    let mut game = Match::at_time(1, 850_000, [0, 1]);
    game.point_for(0);
    while matches!(game.round(), Round::Brought { .. }) {
        game.step();
    }
    let fade = bytes_of(&game.rule);
    assert_eq!(
        (fade.len(), fade[23], fade[24]),
        (19 + 4 + 1 + 5 + 4 + 1, 7, 1)
    );

    // Over.
    let mut game = Match::at_time(1, 899_950, [1, 0]);
    game.step();
    let over = bytes_of(&game.rule);
    assert_eq!((over.len(), over[18], over[23]), (25, 2, 1));
    let mut bad = over.clone();
    bad[23] = 3;
    assert_eq!(
        Rule::read(&params, &bad).map(|_| ()),
        Err(WireError::Reason)
    );
}

#[test]
fn the_fields_are_the_bytes_in_order() {
    let mut game = Match::at_time(5, 900_000, [0, 0]);
    game.step();
    game.take(0);
    game.drop_at(0, 3.0);
    let mut bytes = [0u8; MAX_BYTES];
    let len = game.rule.write(&mut bytes).unwrap();
    assert_eq!(len, MAX_BYTES, "dropped in sudden death: the longest state");
    let mut walked = Vec::new();
    let mut names = Vec::new();
    game.rule.fields(&mut |field| {
        names.push(field.name.to_str().unwrap());
        match field.value {
            Value::U8(value) => walked.push(value),
            Value::U32(value) => walked.extend(value.to_le_bytes()),
        }
    });
    assert_eq!(walked, bytes[..len]);
    assert_eq!(
        names[..13],
        [
            "m_random",
            "m_score[0]",
            "m_score[1]",
            "m_round_starts",
            "m_sides[0]",
            "m_sides[1]",
            "m_stage",
            "m_start",
            "m_round",
            "m_clock",
            "m_sudden.from",
            "m_sudden.fallen",
            "m_dropped.x",
        ]
    );
    assert_eq!(names[names.len() - 2..], ["m_way.carried[19]", "m_size"]);
    assert_eq!(game.rule.write(&mut bytes[..len - 1]), None);
}

/// Matches played by a generator: every state they pass through is one the
/// wire carries and takes back (checked in `send`), a late joiner that
/// loads any of them computes the same from there, and the invariants hold.
#[test]
fn random_matches_keep_the_invariants() {
    let mut longest = 0;
    let mut rounds = 0;
    for seed in 0..80u32 {
        let mut dice = Rng::seeded(seed ^ 0x5EED);
        let mut roll = |range: u32| {
            let (next, value) = dice.below(range);
            dice = next;
            value
        };
        // Start late in the match so that its end, the last deliveries and
        // sudden death are all played.
        let late = 600_000 + roll(280_000) / TICK * TICK;
        let mut game = Match::at_time(seed, late, [roll(5) as u8, roll(5) as u8]);
        let mut joiner: Option<Rule> = None;
        let first_starts = game.rule.round_starts();
        let mut started_here = 0;
        let mut replay: Vec<Event> = Vec::new();
        for _ in 0..60_000 {
            if game.rule.result().is_some() {
                break;
            }
            let who = place(roll(20) as u8);
            // An event's time is the step's or the one before.
            let now = t(game.now - roll(2) * TICK);
            let ground = at(roll(100) as f32);
            let event = match roll(100) {
                0..=1 => Event::Entered {
                    place: who,
                    team: if roll(2) == 0 { Team::Zero } else { Team::One },
                    now,
                },
                2 => Event::Left {
                    place: who,
                    now,
                    ground: (roll(4) != 0).then_some(ground),
                },
                3..=4 => Event::Died {
                    place: who,
                    now,
                    ground,
                },
                5..=24 => Event::Take { place: who, now },
                25..=26 => Event::Drop {
                    place: game.rule.carrier().unwrap_or(who),
                    now,
                    ground,
                },
                27..=31 => Event::Deliver {
                    place: game.rule.carrier().unwrap_or(who),
                    now,
                    ground,
                },
                _ => {
                    game.now += TICK;
                    Event::Tick { now: t(game.now) }
                }
            };
            let frozen = game.rule.frozen();
            let spawning = match event {
                Event::Tick { now } => Some(game.rule.spawning(now)),
                _ => None,
            };
            game.send(event);
            // The respawn rule's "everyone" is the step the countdown starts.
            if let Some(spawning) = spawning {
                let started = !frozen && game.rule.frozen();
                assert_eq!(spawning == Spawning::Everyone, started, "seed {seed}");
                rounds += usize::from(started);
                started_here += u16::from(started);
            }
            // The round starts are counted exactly: each everyone's respawn.
            assert_eq!(
                game.rule.round_starts(),
                first_starts + started_here,
                "seed {seed}"
            );
            if joiner.is_some() {
                replay.push(event);
            } else if roll(500) == 0 {
                joiner = Some(game.rule);
            }
            let rule = &game.rule;
            let goal = game.params.goal();
            assert!(rule.score()[0] <= goal && rule.score()[1] <= goal);
            if let Some(by) = rule.carrier() {
                let team = rule
                    .sides()
                    .team_of(by)
                    .expect("the carrier is in the match");
                assert!(rule.standing(team).has(by));
            }
            if rule.sudden_on() && rule.score()[0] == rule.score()[1] {
                assert!(
                    !rule.standing(Team::Zero).is_empty() && !rule.standing(Team::One).is_empty()
                );
            }
            assert_eq!(rule.respawns(), !rule.sudden_on());
            longest = longest.max(game.now - START);
        }
        let (outcome, _) = game.rule.result().expect("every match ends");
        assert_eq!(outcome, game.rule.leader());
        // The joiner: the bytes of the world it got, then the same events.
        if let Some(rule) = joiner {
            let mut bytes = [0u8; MAX_BYTES];
            let len = rule.write(&mut bytes).unwrap();
            let (mut loaded, _) = Rule::read(&game.params, &bytes[..len]).unwrap();
            for event in replay {
                loaded = loaded.step(&game.params, event).0;
            }
            assert_eq!(loaded, game.rule, "seed {seed}");
        }
    }
    assert!(rounds > 10, "rounds were played: {rounds}");
    // No match outlives its time, a late delivery's 8 s and sudden death.
    assert!(longest <= 900_000 + 8_000 + 180_000 + TICK, "{longest}");
}

#[test]
fn the_use_key_takes_within_its_reach_and_before_the_level() {
    let params = params();
    let lying = at(10.0).raised(130);
    // Standing at 8.5 m, the eye 1.5 m up, looking down at it.
    let eye = [8.5, 1.5, 0.0];
    let to = [1.5f32, -1.37, 0.0];
    let length = (to[0] * to[0] + to[1] * to[1]).sqrt();
    let view = View {
        eye,
        forward: [to[0] / length, to[1] / length, 0.0],
    };
    assert!(params.takes(&view, lying, None));
    // A wall a metre along the view stands between.
    assert!(!params.takes(&view, lying, Some(1.0)));
    // The level just past the sphere's near side does not.
    assert!(params.takes(&view, lying, Some(length)));
    // Out of the 2 m reach.
    let far = View {
        eye: [7.0, 1.5, 0.0],
        forward: [1.0, 0.0, 0.0],
    };
    assert!(!params.takes(&far, at(10.0).raised(1500), None));
    // Looking away.
    let away = View {
        forward: [-1.0, 0.0, 0.0],
        ..view
    };
    assert!(!params.takes(&away, lying, None));
}

#[test]
fn a_delivery_is_the_view_on_ones_own_base() {
    let params = params();
    // Team 0's crate is at x = 0 (0..1 m up); team 1's at x = 100.
    let at_0 = View {
        eye: [1.6, 1.5, 0.0],
        forward: [-0.8, -0.6, 0.0],
    };
    assert!(params.delivers(&at_0, Team::Zero, None));
    assert!(!params.delivers(&at_0, Team::One, None));
    // The crate's own collision at its surface is no wall.
    assert!(params.delivers(&at_0, Team::Zero, Some(1.38)));
    // A wall half a metre before it is.
    assert!(!params.delivers(&at_0, Team::Zero, Some(0.5)));
    let back = View {
        eye: [3.0, 1.5, 0.0],
        forward: [-1.0, 0.0, 0.0],
    };
    assert!(!params.delivers(&back, Team::Zero, None));
}

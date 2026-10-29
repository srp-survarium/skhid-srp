use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use mapdata::Vec3;

use super::*;
use crate::v026g::bots::control::Driver;
use crate::v026g::bots::reflex::{Gates, Legs, LegsWhy, Override};
use crate::v026g::bots::view::Place;
use crate::v026g::drive::key;

const A: Spot = Spot::at(10.0, 0.0, 10.0);
const HOLD: Spot = Spot::at(30.0, 0.0, 10.0);
const LANE: &[Spot] = &[Spot::at(0.0, 0.0, 0.0), Spot::at(0.0, 0.0, 20.0), Spot::at(20.0, 0.0, 20.0)];

fn dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("srp-client-test-{tag}-{}-{n}", std::process::id()))
}

fn body(place: Place, team: u8, at: Vec3) -> Body {
    Body {
        place,
        team,
        alive: true,
        feet: at,
        health: 1.0,
        kills: 0,
        deaths: 0,
        crouched: false,
        keys: 0,
        yaw: 0.0,
        pitch: 0.0,
        speed: 0.0,
    }
}

fn bot_view(place: Place, team: u8, at: Vec3) -> BotView {
    BotView {
        body: body(place, team, at),
        mind: Mind {
            target: None,
            gates: Gates::default(),
            legs: None,
        },
    }
}

/// The match as a made-up owner (place 0, team 0) and bots see it: the mate is place 1, the enemies 2 and 3.
struct Sim {
    clock: u32,
    owner: Body,
    bots: Vec<BotView>,
}

impl Sim {
    fn new() -> Self {
        Self {
            clock: 0,
            owner: body(0, 0, Vec3::new(0.0, 0.0, 0.0)),
            bots: vec![
                bot_view(1, 0, Vec3::new(5.0, 0.0, 0.0)),
                bot_view(2, 1, Vec3::new(50.0, 0.0, 50.0)),
                bot_view(3, 1, Vec3::new(60.0, 0.0, 50.0)),
            ],
        }
    }

    fn walk(body: &mut Body, to: Vec3, mps: f32) {
        let (dx, dz) = (to.x - body.feet.x, to.z - body.feet.z);
        let d = dx.hypot(dz);
        if d > 0.05 {
            let step = (mps * 0.05).min(d);
            body.feet = Vec3::new(body.feet.x + dx / d * step, 0.0, body.feet.z + dz / d * step);
        }
    }

    /// 50 ms pass: the bots walk where the runner scripts them, the frame is the match then.
    fn frame(&mut self, link: &Link) -> Frame {
        self.clock += 50;
        for bot in &mut self.bots {
            if let Some(Drive {
                act: Act::Go { at, within_m, .. },
                ..
            }) = link.drive(bot.body.place)
                && bot.body.feet.distance_xz(at) > within_m * 0.5
            {
                Self::walk(&mut bot.body, at, 4.0);
            }
        }
        Frame {
            clock: At(self.clock),
            playing: true,
            owner: Some(Owner {
                body: self.owner.clone(),
                pools: None,
            }),
            bots: self.bots.clone(),
            batteries: Vec::new(),
            heard: Vec::new(),
            sight: None,
        }
    }

    fn tick(&mut self, link: &mut Link) {
        let frame = self.frame(link);
        link.tick(&frame);
    }
}

fn teams(per_team: usize) -> Vec<u8> {
    std::iter::repeat_n(0, per_team).chain(std::iter::repeat_n(1, per_team)).collect()
}

fn provenance() -> Provenance {
    Provenance {
        sha: "abc1234".into(),
        tuning: 0xDEAD_BEEF,
        tuning_source: "tuning=defaults".into(),
    }
}

fn link_for(tag: &str, steps: Vec<Step>, next: Option<Group>) -> (Link, PathBuf) {
    let dir = dir(tag);
    let output = Arc::new(Output::new(&dir, "unit").unwrap());
    let played = group_of(2, BATTERIES, &teams(2));
    let todo = steps.into_iter().map(Todo::new).collect();
    (Link::new(Runner::new(0xBEEF, provenance(), played, todo, next), output), dir)
}

fn results_of(link: &Link) -> String {
    std::fs::read_to_string(link.output().results_path()).unwrap_or_default()
}

fn group() -> Group {
    Group::new(2, BATTERIES)
}

fn steps() -> Vec<Step> {
    vec![
        Step::new("t-hold", group(), "Go to the spot and kill two.")
            .setup(
                Who::Nth(Side::Enemies, 0),
                Script::Hold {
                    at: HOLD,
                    within_m: 1.0,
                    crouch: false,
                },
            )
            .start(&[Cond::OwnerNear { at: A, r_m: 3.0 }, Cond::BotsInPlace])
            .end(&[End::OwnerKills(2)])
            .measure(Measure::OwnerKills)
            .measure(Measure::FirstKillMs)
            .measure(Measure::NearestBotM(Side::Enemies))
            .bar("owner_kills", Test::AtLeast(2.0))
            .budget_s(60),
        Step::new("t-walk", group(), "Stand and watch the mate walk.")
            .setup(
                Who::Nth(Side::Mates, 0),
                Script::Walk {
                    lane: LANE,
                    gait: Gait::Walk,
                    looped: false,
                },
            )
            .end(&[End::OwnerDied])
            .measure(Measure::TravelM)
            .bar("travel_m", Test::AtLeast(5.0))
            .budget_s(10)
            .tries(2),
    ]
}

#[test]
fn two_steps_run_one_after_another_with_a_made_up_owner() {
    let (mut link, dir) = link_for("two", steps(), Some(Group::new(7, TEAM_DEATHMATCH)));
    let mut sim = Sim::new();
    let mut says = Vec::new();
    let (mut seen_hold, mut released, mut second_leg) = (false, false, false);
    for _ in 0..8_000 {
        if link.output().used("t-hold") == 0 {
            // The owner walks to A and makes two kills there, at 12 s and 16 s.
            Sim::walk(&mut sim.owner, A.vec(), 5.0);
            seen_hold |= matches!(link.drive(2), Some(Drive { act: Act::Go { .. }, .. }));
            let there = sim.owner.feet.distance_xz(A.vec()) < 1.0;
            if there && sim.clock.is_multiple_of(4_000) && sim.clock > 10_000 {
                sim.owner.kills += 1;
            }
        } else {
            // Then he strolls on.
            sim.owner.feet = Vec3::new(sim.owner.feet.x + 0.05, 0.0, sim.owner.feet.z);
            released |= link.drive(2).is_none();
            second_leg |= matches!(link.drive(1), Some(Drive { act: Act::Go { at, .. }, .. }) if at == LANE[1].vec());
        }
        sim.tick(&mut link);
        says.extend(link.take_says());
        if link.output().used("t-walk") == 2 {
            break;
        }
    }
    assert!(seen_hold, "the enemy bot was scripted to hold");
    assert!(released, "the bot is its own again when the step ends");
    assert!(second_leg, "the mate walks its lane");
    assert_eq!(says[0], "[t-hold/1] Go to the spot and kill two.");
    assert!(says.iter().any(|s| s.starts_with("[t-hold] done: PASS")), "{says:?}");
    assert!(says.iter().any(|s| s.starts_with("[t-walk/1] ")), "{says:?}");
    assert!(says.iter().any(|s| s.starts_with("[t-walk/2] ")), "{says:?}");

    // Nothing is left: the owner is told the next match.
    sim.clock += 20_000;
    sim.tick(&mut link);
    let last = link.take_says();
    assert!(last.iter().any(|s| s.contains("Vector Laboratory, team deathmatch")), "{last:?}");

    let results = results_of(&link);
    let rows: Vec<&str> = results.lines().collect();
    assert_eq!(rows[0], report::HEADER);
    assert_eq!(rows.len(), 1 + 3, "one row per try: {results}");
    assert!(rows[1].contains("\tt-hold\t") && rows[1].contains("\tPASS\towner_kills\t"), "{}", rows[1]);
    assert!(rows[1].contains("owner_kills=2"), "{}", rows[1]);
    assert!(
        rows[1].starts_with("0xbeef\tabc1234\tdeadbeef\ttuning=defaults\t"),
        "the commit and the tuning are in every row: {}",
        rows[1]
    );
    assert!(rows[2].contains("\tt-walk\t") && rows[2].contains("\t1\t2\t") && rows[2].contains("\tbudget\t"), "{}", rows[2]);
    assert!(rows[3].contains("\t2\t2\t"), "{}", rows[3]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_step_the_owner_never_starts_is_void_and_used_up() {
    let step = Step::new("t-never", group(), "Go far away.")
        .start(&[Cond::OwnerNear {
            at: Spot::at(500.0, 0.0, 500.0),
            r_m: 3.0,
        }])
        .end(&[End::OwnerDied])
        .arm_s(10);
    let (mut link, dir) = link_for("never", vec![step], None);
    let mut sim = Sim::new();
    for _ in 0..400 {
        sim.tick(&mut link);
    }
    let results = results_of(&link);
    assert!(results.contains("\tVOID(never_started)\tarm_time\t"), "{results}");
    assert_eq!(link.output().used("t-never"), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_bot_voids_the_try() {
    let step = Step::new("t-missing", group(), "x")
        .setup(Who::Nth(Side::Enemies, 5), Script::Idle)
        .end(&[End::OwnerDied]);
    let (mut link, dir) = link_for("missing", vec![step], None);
    Sim::new().tick(&mut link);
    let results = results_of(&link);
    assert!(results.contains("VOID(missing_bots(enemy_bot_5))"), "{results}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_match_that_ends_mid_step_voids_the_try_without_using_it_up() {
    let step = Step::new("t-cut", group(), "x").end(&[End::OwnerDied]).budget_s(120);
    let (mut link, dir) = link_for("cut", vec![step], None);
    let mut sim = Sim::new();
    for _ in 0..200 {
        sim.tick(&mut link);
    }
    let results_path = link.output().results_path().to_path_buf();
    let output = link.output.clone();
    let last = sim.clock;
    drop(link);
    let results = std::fs::read_to_string(results_path).unwrap();
    assert!(results.contains("VOID(match_ended)"), "{results}");
    assert!(
        results.contains(&format!("\t{last}\t")),
        "the row ends when the match did, not when the try began: {results}"
    );
    assert_eq!(output.used("t-cut"), 0, "a cut try is run again");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A step ends with the owner's death; the next must not start (and end at once) while he is dead: it waits for his respawn.
#[test]
fn a_dead_owner_starts_no_step_and_the_next_one_waits_for_his_respawn() {
    let first = Step::new("t-die", group(), "Die.").end(&[End::OwnerDied]).budget_s(60);
    let second = Step::new("t-after", group(), "Live 5 s.")
        .end(&[End::OwnerDied])
        .measure(Measure::ElapsedMs)
        .budget_s(5);
    let (mut link, dir) = link_for("dead", vec![first, second], None);
    let mut sim = Sim::new();
    // Eight seconds dead: longer than the rest and the settle together.
    let (dies, respawns) = (5_000, 13_000);
    for _ in 0..4_000 {
        if sim.clock == dies {
            sim.owner.alive = false;
            sim.owner.deaths += 1;
        }
        if sim.clock == respawns {
            sim.owner.alive = true;
        }
        sim.tick(&mut link);
        if link.output().used("t-after") == 1 {
            break;
        }
    }
    let results = results_of(&link);
    let rows: Vec<&str> = results.lines().skip(1).collect();
    assert_eq!(rows.len(), 2, "{results}");
    assert!(rows[0].contains("\towner_died\t"), "{}", rows[0]);
    assert!(
        rows[1].contains("\tt-after\t") && rows[1].contains("\tbudget\t") && rows[1].contains("elapsed_ms=5000"),
        "the second ran its 5 s alive: {}",
        rows[1]
    );
    let start: u32 = rows[1].split('\t').nth(10).unwrap().parse().unwrap();
    assert!(start >= respawns, "it started at {start}, after the respawn at {respawns}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A throw winds from the START, not from the arming; before it the bot stands.
#[test]
fn a_throw_winds_from_the_start() {
    let never = Grip {
        fight: Fight::Never,
        react: React::None,
    };
    let step = Step::new("t-throw", group(), "Watch.")
        .setup_with(
            Who::Nth(Side::Enemies, 0),
            Script::Throw {
                at: Spot::at(40.0, 0.0, 40.0),
                wind: Span::ms(600),
            },
            never,
        )
        .start(&[Cond::After(Span::s(3))])
        .end(&[End::OwnerDied])
        .budget_s(4);
    let (mut link, dir) = link_for("throw", vec![step], None);
    let mut sim = Sim::new();
    let (mut held_before_start, mut held_after, mut let_go) = (false, false, false);
    for _ in 0..200 {
        sim.tick(&mut link);
        match link.drive(2) {
            Some(Drive {
                act: Act::Throw { held: true, .. },
                ..
            }) => {
                if sim.clock < 3_000 {
                    held_before_start = true;
                } else {
                    held_after = true;
                }
            }
            Some(Drive {
                act: Act::Throw { held: false, .. },
                grip,
            }) => {
                assert_eq!(grip.fight, Fight::Never);
                let_go = true;
            }
            _ => {}
        }
    }
    assert!(!held_before_start && held_after && let_go, "{held_before_start} {held_after} {let_go}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A package's own condition, end and measure with several values.
#[test]
fn a_package_can_bring_its_own_condition_end_and_measures() {
    fn owner_east(frame: &Frame, _waited: Span) -> bool {
        frame.owner.as_ref().is_some_and(|o| o.body.feet.x > 20.0)
    }
    fn owner_far(frame: &Frame, progress: &Progress) -> bool {
        progress.since_start >= Span::s(2) && frame.owner.as_ref().is_some_and(|o| o.body.feet.x > 100.0)
    }
    /// The furthest east the owner got and how many polls it took.
    #[derive(Debug, Default)]
    struct East {
        polls: u32,
        furthest: Option<f32>,
    }
    impl CustomMeter for East {
        fn sample(&mut self, frame: &Frame) {
            self.polls += 1;
            if let Some(o) = frame.owner.as_ref() {
                self.furthest = Some(self.furthest.map_or(o.body.feet.x, |f| f.max(o.body.feet.x)));
            }
        }

        fn value(&self, n: usize) -> Option<f64> {
            match n {
                0 => Some(f64::from(self.polls)),
                1 => self.furthest.map(f64::from),
                _ => None,
            }
        }
    }
    let step = Step::new("t-own", group(), "Go east.")
        .start(&[Cond::Custom(WaitFor {
            describe: "you are east of x 20",
            holds: owner_east,
        })])
        .end(&[End::Custom(EndWhen {
            reason: "far_east",
            describe: "you are east of x 100",
            holds: owner_far,
        })])
        .measure(Measure::Custom {
            names: &["polls", "furthest_x"],
            make: |_| Box::new(East::default()),
        })
        .bar("furthest_x", Test::AtLeast(100.0))
        .budget_s(60);
    step.check().unwrap();
    let (mut link, dir) = link_for("own", vec![step], None);
    let mut sim = Sim::new();
    for _ in 0..1_200 {
        sim.owner.feet = Vec3::new(sim.owner.feet.x + 0.25, 0.0, 0.0);
        sim.tick(&mut link);
    }
    let results = results_of(&link);
    assert!(results.contains("\tPASS\tfar_east\t"), "{results}");
    assert!(results.contains("polls=") && results.contains("furthest_x="), "{results}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_plan_groups_steps_by_map_and_mode() {
    let g = |level, mode| Group::new(level, mode);
    let make = |id: &'static str, group| Step::new(id, group, "x").end(&[End::OwnerDied]);
    let all = vec![
        make("a-1", g(7, TEAM_DEATHMATCH)),
        make("b-1", g(2, BATTERIES)),
        make("a-2", g(2, BATTERIES)),
        make("b-2", g(2, BATTERIES).of_size(8)),
        make("a-3", g(7, TEAM_DEATHMATCH)),
    ];
    let plan = Plan::parse("all", all.clone()).unwrap();
    let order: Vec<&str> = plan.steps().iter().map(|s| s.id).collect();
    assert_eq!(
        order,
        ["check-message", "b-1", "a-2", "b-2", "a-1", "a-3"],
        "the message check first, then by map, mode and size; registration order within"
    );
    assert_eq!(
        plan.groups(),
        [g(2, BATTERIES).of_size(8), g(7, TEAM_DEATHMATCH)],
        "steps of any size ride with the first sized match of their map and mode"
    );
    assert_eq!(plan.matches()[0].1.len(), 4, "the check, b-1 and a-2 ride with b-2");

    let output = Output::new(&dir("plan"), "p").unwrap();
    let small = group_of(2, BATTERIES, &teams(4));
    let todo: Vec<&str> = plan.todo_for(small, &output).iter().map(|t| t.step.id).collect();
    assert_eq!(todo, ["check-message", "b-1", "a-2"], "any-size steps fit a 4-a-side match, the 8-a-side one does not");
    let eight = group_of(2, BATTERIES, &teams(8));
    let todo: Vec<&str> = plan.todo_for(eight, &output).iter().map(|t| t.step.id).collect();
    assert_eq!(todo, ["check-message", "b-1", "a-2", "b-2"]);
    assert_eq!(plan.next_group(small, &output), Some(g(2, BATTERIES).of_size(8)));
    assert_eq!(plan.next_group(eight, &output), Some(g(7, TEAM_DEATHMATCH)));

    assert!(Plan::parse("a-1,nope", all.clone()).unwrap_err().contains("nope"));
    let by_id = Plan::parse("a-3, b-1", all).unwrap();
    assert_eq!(by_id.steps().iter().map(|s| s.id).collect::<Vec<_>>(), ["check-message", "b-1", "a-3"]);
}

#[test]
fn a_plan_refuses_a_step_that_cannot_be_run() {
    let bad = Step::new("x-bad", Group::new(2, 3), "Mode 3 is not offered in Player vs Bots.").end(&[End::OwnerDied]);
    let err = Plan::parse("all", vec![bad]).unwrap_err();
    assert!(err.contains("x-bad") && err.contains("batteries (2) and team deathmatch (4)"), "{err}");
}

#[test]
fn the_message_check_is_a_step_that_passes_on_two_crouches() {
    let step = check::message_check(Group::new(2, BATTERIES).of_size(6));
    step.check().unwrap();
    assert_eq!(step.group.size, Size::Any);
    let (mut link, dir) = link_for("check", vec![step], None);
    let mut sim = Sim::new();
    for _ in 0..800 {
        // C held half a second at 5 s and at 7 s: four changes.
        sim.owner.crouched = matches!(sim.clock, 5_000..=5_500 | 7_000..=7_500);
        sim.tick(&mut link);
    }
    let results = results_of(&link);
    assert!(
        results.contains("\tcheck-message\tcheck\t") && results.contains("\tPASS\t") && results.contains("crouch_changes=4"),
        "{results}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn it_is_refused_on_a_live_configuration() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| pairs.iter().find(|(n, _)| *n == name).map(|(_, v)| (*v).to_string())
    };
    assert_eq!(live_reason(&env(&[])), None, "a local server sets neither");
    assert_eq!(live_reason(&env(&[("SRP_PUBLIC_HOST", "127.0.0.1")])), None);
    assert_eq!(live_reason(&env(&[("SRP_PUBLIC_HOST", "192.168.1.20")])), None);
    assert_eq!(live_reason(&env(&[("SRP_PUBLIC_HOST", "localhost")])), None);
    assert!(live_reason(&env(&[("SRP_PUBLIC_HOST", "203.0.113.9")])).is_some());
    assert!(live_reason(&env(&[("SRP_PUBLIC_HOST", "play.example.org")])).is_some());
    assert!(
        live_reason(&env(&[("SRP_PUBLIC_HOST", "?")])).is_some(),
        "a value that is not text is no local address"
    );
    assert!(live_reason(&env(&[("SRP_LOBBY_PUBLIC_HOST", "198.51.100.4")])).is_some());
    assert!(live_reason(&env(&[("SRP_ADMIN_TOKEN", "x")])).is_some());
    assert_eq!(live_reason(&env(&[("SRP_ADMIN_TOKEN", " ")])), None);
}

#[test]
fn every_registered_step_can_be_run() {
    let all = registry();
    assert!(!all.is_empty());
    for step in &all {
        step.check().unwrap();
        assert!(step.id.starts_with(step.package), "{} lives in {}", step.id, step.package);
    }
    let mut ids: Vec<&str> = all.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(before, ids.len(), "step ids are unique across packages");
}

#[test]
fn a_bad_step_is_caught() {
    let ok = || Step::new("x-y", group(), "Do it.").end(&[End::OwnerDied]);
    ok().check().unwrap();
    assert!(ok().budget_s(181).check().is_err());
    assert!(Step::new("Bad Id", group(), "Do it.").check().is_err());
    assert!(Step::new("x-y", Group::new(9, BATTERIES), "Do it.").check().is_err());
    assert!(Step::new("x-y", Group::new(2, 5), "Do it.").check().is_err());
    assert!(Step::new("x-y", Group::new(2, 3), "Do it.").check().is_err(), "mode 3 is not offered in Player vs Bots");
    assert!(Step::new("x-y", group().of_size(7), "Do it.").check().is_err(), "Player vs Bots offers 6, 8 and 10");
    Step::new("x-y", group().of_size(10), "Do it.").check().unwrap();
    assert!(Step::new("x-y", group(), "two\nlines").check().is_err());
    assert!(ok().bar("nothing", Test::AtLeast(1.0)).check().is_err(), "a bar on an undeclared measure");
    assert!(ok().start(&[Cond::BotsInPlace]).check().is_err(), "BotsInPlace needs a Hold");
    let throw = Script::Throw {
        at: Spot::at(0.0, 0.0, 0.0),
        wind: Span::ms(500),
    };
    // A throw and an idle dummy fire nothing by default; asking the reflex to fight for them is refused.
    for script in [throw, Script::Idle] {
        let step = ok().setup(Who::All(Side::Enemies), script);
        assert_eq!(step.setup[0].grip.fight, Fight::Never, "{script:?}");
        step.check().unwrap();
        assert!(ok().setup_with(Who::All(Side::Enemies), script, Grip::DEFAULT).check().is_err(), "{script:?} with a free fight");
    }
    assert_eq!(Grip::for_script(Script::Stand { crouch: false }), Grip::DEFAULT);
}

/// A bot that is to face a spot stands looking at it, and the custom conditions read keys and view of the bodies.
#[test]
fn a_bot_can_be_told_to_face_a_spot() {
    let at = Spot::at(40.0, 0.0, 10.0);
    let step = Step::new("t-face", group(), "Look at the dummy.")
        .setup(Who::Nth(Side::Enemies, 0), Script::Face { at, crouch: true })
        .end(&[End::OwnerDied])
        .budget_s(5);
    step.check().unwrap();
    let (mut link, dir) = link_for("face", vec![step], None);
    let mut sim = Sim::new();
    for _ in 0..80 {
        sim.tick(&mut link);
    }
    assert_eq!(link.drive(2).expect("scripted").act, Act::Face { look: at.vec(), crouch: true });
    let _ = std::fs::remove_dir_all(&dir);
}

/// An idle dummy stands and is handed to the bot with a grip that does not let the reflex fire.
#[test]
fn an_idle_dummy_does_not_fire_back() {
    let step = Step::new("t-idle", group(), "Shoot the dummy.").setup(Who::Nth(Side::Enemies, 0), Script::Idle).end(&[End::OwnerDied]).budget_s(5);
    let (mut link, dir) = link_for("idle", vec![step], None);
    let mut sim = Sim::new();
    for _ in 0..80 {
        sim.tick(&mut link);
    }
    let drive = link.drive(2).expect("scripted");
    assert_eq!(drive.act, Act::Stand { crouch: false });
    assert_eq!(drive.grip.fight, Fight::Never);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The grip decides what of the reflex's wish stands: a free fight keeps the aim and the trigger and loses the legs; a never
/// keeps the reload only. Own and the bench camper's control leave the wish as it is.
#[test]
fn a_grip_leaves_the_reflex_what_the_step_lets_it() {
    let wish = Override {
        look_at: Some(Vec3::new(1.0, 2.0, 3.0)),
        fire: true,
        keys: key::GRENADE | key::RELOAD | key::AIM,
        legs: Some(Legs {
            at: Vec3::new(5.0, 0.0, 5.0),
            within_m: 1.0,
            run: true,
            crouch: false,
            why: LegsWhy::Punish,
        }),
        strafe: key::LEFT,
        ..Override::default()
    };
    let free = Driver::Scripted(Grip {
        fight: Fight::Free,
        react: React::None,
    })
    .filter(wish);
    assert!(free.look_at.is_some() && free.fire && free.keys == wish.keys, "aim, trigger and keys stand");
    assert!(free.legs.is_none() && free.strafe == 0, "the legs are the script's");
    let never = Driver::Scripted(Grip {
        fight: Fight::Never,
        react: React::Normal,
    })
    .filter(wish);
    assert!(never.look_at.is_none() && !never.fire && never.legs.is_none());
    assert_eq!(never.keys, key::RELOAD, "only the reload is left");
    // A full fight leaves the reflex its legs and side step too.
    let full = Driver::Scripted(Grip {
        fight: Fight::Full,
        react: React::None,
    });
    assert_eq!(full.filter(wish), wish);
    assert!(!full.holds_legs());
    assert_eq!(Driver::Own.filter(wish), wish);
    assert_eq!(Driver::BenchCamper.filter(wish), wish);
    assert!(!Driver::Own.holds_legs() && Driver::BenchCamper.holds_legs());
    assert!(Driver::Scripted(Grip::DEFAULT).holds_legs());
    assert!(!Driver::Scripted(Grip::DEFAULT).reacts(), "a holder holds when a grenade lands by it");
    assert!(
        Driver::Scripted(Grip {
            fight: Fight::Free,
            react: React::Normal
        })
        .reacts()
    );
    assert!(Driver::Own.reacts() && Driver::BenchCamper.reacts());
}

/// A step can ask for a role of a bot; two steps of one match that ask for two roles of one seat make the plan refuse; the roles
/// are put in the lobby's by seat.
#[test]
fn a_step_can_ask_for_the_role_of_a_bot() {
    use survarium::v026g::loadouts::BotRole::{Camper, Flanker, Rusher, Support};
    let step = |id: &'static str, role| Step::new(id, group(), "x").seat(Side::Enemies, 1, role).end(&[End::OwnerDied]);
    step("a-1", Camper).check().unwrap();
    let plan = Plan::parse("all", vec![step("a-1", Camper), Step::new("a-2", group(), "x").end(&[End::OwnerDied])]).unwrap();
    let roster = plan.roster_for(group_of(2, BATTERIES, &teams(2)));
    assert_eq!(roster, [Seat { side: Side::Enemies, nth: 1, role: Camper }]);
    assert!(plan.roster_for(group_of(7, TEAM_DEATHMATCH, &teams(2))).is_empty(), "another match has no seats");
    let clash = Plan::parse("all", vec![step("a-1", Camper), step("a-2", Rusher)]).unwrap_err();
    assert!(clash.contains("a-1") && clash.contains("a-2"), "{clash}");
    // The same role twice is no clash.
    Plan::parse("all", vec![step("a-1", Camper), step("a-2", Camper)]).unwrap();
    // The owner and 2 mates on team 0 would be 3 a side: the bots by place are 2 mates and 3 enemies.
    let bot_teams = [0, 0, 1, 1, 1];
    let defaults = vec![Flanker, Support, Rusher, Flanker, Support];
    // The enemy bot 1 is the second bot of team 1: place 3.
    assert_eq!(roles_for(&roster, &bot_teams, defaults.clone()), vec![Flanker, Support, Rusher, Camper, Support]);
    let missing = [Seat { side: Side::Enemies, nth: 9, role: Camper }];
    assert_eq!(roles_for(&missing, &bot_teams, defaults.clone()), defaults, "a seat with no bot changes nothing");
    assert_eq!(bot_roles(2, BATTERIES, 3, &bot_teams, defaults.clone()), defaults, "no plan set: the lobby's roles");
}

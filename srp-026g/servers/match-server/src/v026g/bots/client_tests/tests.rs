use std::sync::atomic::{AtomicU32, Ordering};

use mapdata::Vec3;

use super::*;

const A: Spot = Spot::at(10.0, 0.0, 10.0);
const HOLD: Spot = Spot::at(30.0, 0.0, 10.0);
const LANE: &[Spot] = &[Spot::at(0.0, 0.0, 0.0), Spot::at(0.0, 0.0, 20.0), Spot::at(20.0, 0.0, 20.0)];

fn dir(tag: &str) -> std::path::PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("srp-client-test-{tag}-{}-{n}", std::process::id()))
}

fn body(place: usize, team: u8, at: Vec3) -> Body {
    Body {
        place,
        team,
        alive: true,
        feet: at,
        health: 1.0,
        kills: 0,
        deaths: 0,
        crouched: false,
    }
}

/// The match as a made-up owner (place 0, team 0) and bots see it: the mate is place 1, the enemies 2 and 3.
struct Sim {
    clock: u32,
    owner: Body,
    bots: Vec<Body>,
    heard: Vec<Heard>,
}

impl Sim {
    fn new() -> Self {
        Self {
            clock: 0,
            owner: body(0, 0, Vec3::new(0.0, 0.0, 0.0)),
            bots: vec![
                body(1, 0, Vec3::new(5.0, 0.0, 0.0)),
                body(2, 1, Vec3::new(50.0, 0.0, 50.0)),
                body(3, 1, Vec3::new(60.0, 0.0, 50.0)),
            ],
            heard: Vec::new(),
        }
    }

    fn frame(&mut self) -> Frame {
        Frame {
            clock: self.clock,
            playing: true,
            owner: Some(self.owner.clone()),
            bots: self.bots.clone(),
            heard: std::mem::take(&mut self.heard),
        }
    }

    fn walk(body: &mut Body, to: Vec3, mps: f32, dt: f32) {
        let (dx, dz) = (to.x - body.feet.x, to.z - body.feet.z);
        let d = dx.hypot(dz);
        if d > 0.05 {
            let step = (mps * dt).min(d);
            body.feet = Vec3::new(body.feet.x + dx / d * step, 0.0, body.feet.z + dz / d * step);
        }
    }

    /// Bots walk where the runner scripts them; 50 ms.
    fn advance(&mut self, link: &Link) {
        self.clock += 50;
        for bot in &mut self.bots {
            if let Some(Drive::Go { at, within_m, .. }) = link.drive(bot.place)
                && bot.feet.distance_xz(at) > within_m * 0.5
            {
                Self::walk(bot, at, 4.0, 0.05);
            }
        }
    }
}

fn steps() -> Vec<Step> {
    let group = Group::new(2, BATTERIES);
    vec![
        Step::new("t-hold", group, "Go to the spot and kill two.")
            .setup(Who::Nth(Side::Enemies, 0), Script::Hold { at: HOLD, within_m: 1.0, crouch: false })
            .start(&[Cond::OwnerNear { at: A, r_m: 3.0 }, Cond::BotsInPlace])
            .end(&[End::OwnerKills(2)])
            .measure(Measure::OwnerKills)
            .measure(Measure::FirstKillMs)
            .measure(Measure::NearestBotM(Side::Enemies))
            .bar("owner_kills", Test::AtLeast(2.0))
            .budget_s(60),
        Step::new("t-walk", group, "Stand and watch the mate walk.")
            .setup(Who::Nth(Side::Mates, 0), Script::Walk { lane: LANE, gait: Gait::Walk, looped: false })
            .end(&[End::OwnerDied])
            .measure(Measure::TravelM)
            .measure(Measure::Notes { name: "stuck_notes", contains: "stuck" })
            .bar("travel_m", Test::AtLeast(5.0))
            .budget_s(10)
            .tries(2),
    ]
}

fn link_for(tag: &str, steps: Vec<Step>, next: Option<Group>) -> (Link, std::path::PathBuf) {
    let dir = dir(tag);
    let output = Arc::new(Output::new(&dir, "unit").unwrap());
    let played = group_of(2, BATTERIES, &[0, 0, 1, 1]);
    let runner = Runner::new(0xBEEF, played, steps.into_iter().map(Todo::new).collect(), next);
    (Link::new(runner, output), dir)
}

#[test]
fn two_steps_run_one_after_another_with_a_made_up_owner() {
    let (mut link, dir) = link_for("two", steps(), Some(Group::new(7, TEAM_DEATHMATCH)));
    let mut sim = Sim::new();
    let mut says = Vec::new();
    let mut seen_hold = false;
    let mut walked_to_second_leg = false;
    let mut hold_gone_after_end = false;
    for _ in 0..8_000 {
        // The owner: walks to A, stays, makes two kills there at 12 s and 16 s of the first step's run; then walks on.
        let first_done = link.output().used("t-hold") > 0;
        if !first_done {
            Sim::walk(&mut sim.owner, A.vec(), 5.0, 0.05);
            if let Some(Drive::Go { .. }) = link.drive(2) {
                seen_hold = true;
            }
            if sim.owner.feet.distance_xz(A.vec()) < 1.0 && sim.clock.is_multiple_of(4_000) && sim.owner.kills < 2 && sim.clock > 10_000 {
                sim.owner.kills += 1;
            }
        } else {
            sim.owner.feet = Vec3::new(sim.owner.feet.x + 0.05, 0.0, sim.owner.feet.z);
            hold_gone_after_end |= link.drive(2).is_none();
            if matches!(link.drive(1), Some(Drive::Go { at, .. }) if at == LANE[1].vec()) {
                walked_to_second_leg = true;
            }
        }
        sim.advance(&link);
        let frame = sim.frame();
        link.tick(&frame);
        says.extend(link.take_says());
        if link.output().used("t-walk") == 2 {
            break;
        }
    }
    assert!(seen_hold, "the enemy bot was scripted to hold");
    assert!(hold_gone_after_end, "the bot is released when the step ends");
    assert!(walked_to_second_leg, "the mate walks its lane");
    assert_eq!(says[0], "[t-hold/1] Go to the spot and kill two.");
    assert!(says.iter().any(|s| s.starts_with("[t-hold] done: PASS")), "{says:?}");
    assert!(says.iter().any(|s| s.starts_with("[t-walk/1] ")) && says.iter().any(|s| s.starts_with("[t-walk/2] ")), "{says:?}");
    // Nothing is left: the owner is told the next match.
    sim.clock += 20_000;
    let frame = sim.frame();
    link.tick(&frame);
    let last = link.take_says();
    assert!(last.iter().any(|s| s.contains("Vector Laboratory, team deathmatch")), "{last:?}");

    let results = std::fs::read_to_string(link.output().results_path()).unwrap();
    let rows: Vec<&str> = results.lines().collect();
    assert_eq!(rows[0], report::HEADER);
    assert_eq!(rows.len(), 1 + 3, "one row per try: {results}");
    assert!(rows[1].contains("\tt-hold\t") && rows[1].contains("\tPASS\towner_kills\t"), "{}", rows[1]);
    assert!(rows[1].contains("owner_kills=2"), "{}", rows[1]);
    assert!(rows[2].contains("\tt-walk\t") && rows[2].contains("\t1\t2\t"), "{}", rows[2]);
    assert!(rows[3].contains("\t2\t2\t"), "{}", rows[3]);
    assert!(rows[2].contains("\tbudget\t"), "{}", rows[2]);
    let card = std::fs::read_to_string(link.output().card_path());
    assert!(card.is_err(), "the card is the init's, not the link's");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_step_the_owner_never_starts_is_void_and_used_up() {
    let group = Group::new(2, BATTERIES);
    let step = Step::new("t-never", group, "Go far away.")
        .start(&[Cond::OwnerNear { at: Spot::at(500.0, 0.0, 500.0), r_m: 3.0 }])
        .end(&[End::OwnerDied])
        .arm_s(10);
    let (mut link, dir) = link_for("never", vec![step], None);
    let mut sim = Sim::new();
    for _ in 0..400 {
        sim.advance(&link);
        let frame = sim.frame();
        link.tick(&frame);
    }
    let results = std::fs::read_to_string(link.output().results_path()).unwrap();
    assert!(results.contains("\tVOID(never_started)\tarm_time\t"), "{results}");
    assert_eq!(link.output().used("t-never"), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_bot_voids_the_try() {
    let step = Step::new("t-missing", Group::new(2, BATTERIES), "x")
        .setup(Who::Nth(Side::Enemies, 5), Script::Idle)
        .end(&[End::OwnerDied]);
    let (mut link, dir) = link_for("missing", vec![step], None);
    let mut sim = Sim::new();
    sim.advance(&link);
    let frame = sim.frame();
    link.tick(&frame);
    let results = std::fs::read_to_string(link.output().results_path()).unwrap();
    assert!(results.contains("VOID(missing_bots(enemy_bot_5))"), "{results}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_match_that_ends_mid_step_voids_the_try_without_using_it_up() {
    let step = Step::new("t-cut", Group::new(2, BATTERIES), "x").end(&[End::OwnerDied]).budget_s(120);
    let (mut link, dir) = link_for("cut", vec![step], None);
    let mut sim = Sim::new();
    for _ in 0..200 {
        sim.advance(&link);
        let frame = sim.frame();
        link.tick(&frame);
    }
    let results_path = link.output().results_path().to_path_buf();
    let output = link.output.clone();
    drop(link);
    let results = std::fs::read_to_string(results_path).unwrap();
    assert!(results.contains("VOID(match_ended)"), "{results}");
    assert_eq!(output.used("t-cut"), 0, "a cut try is run again");
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
    assert_eq!(order, ["b-1", "a-2", "b-2", "a-1", "a-3"], "by map, then mode, then size; registration order within");
    assert_eq!(plan.groups().len(), 3);
    let output = Output::new(&dir("plan"), "p").unwrap();
    let played = group_of(2, BATTERIES, &[0; 4].iter().chain(&[1; 4]).copied().collect::<Vec<u8>>());
    let todo: Vec<&str> = plan.todo_for(played, &output).iter().map(|t| t.step.id).collect();
    assert_eq!(todo, ["b-1", "a-2"], "any-size steps fit a 4-a-side match, the 8-a-side one does not");
    let eight = group_of(2, BATTERIES, &[0; 8].iter().chain(&[1; 8]).copied().collect::<Vec<u8>>());
    let todo: Vec<&str> = plan.todo_for(eight, &output).iter().map(|t| t.step.id).collect();
    assert_eq!(todo, ["b-1", "a-2", "b-2"]);
    assert_eq!(plan.next_group(played, &output), Some(g(2, BATTERIES).of_size(8)), "the same map first, then the next map");
    assert_eq!(plan.next_group(eight, &output), Some(g(7, TEAM_DEATHMATCH)));
    assert!(Plan::parse("a-1,nope", all.clone()).unwrap_err().contains("nope"));
    let by_id = Plan::parse("a-3, b-1", all).unwrap();
    assert_eq!(by_id.steps().iter().map(|s| s.id).collect::<Vec<_>>(), ["b-1", "a-3"]);
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
    assert!(live_reason(&env(&[("SRP_LOBBY_PUBLIC_HOST", "198.51.100.4")])).is_some());
    assert!(live_reason(&env(&[("SRP_ADMIN_TOKEN", "x")])).is_some());
    assert_eq!(live_reason(&env(&[("SRP_ADMIN_TOKEN", " ")])), None);
}

#[test]
fn every_registered_step_can_be_run() {
    let all = registry();
    assert!(!all.is_empty());
    let mut ids: Vec<&str> = all.iter().map(|s| s.id).collect();
    for step in &all {
        step.check().unwrap();
        assert!(step.id.starts_with(step.package), "{} lives in {}", step.id, step.package);
    }
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(before, ids.len(), "step ids are unique across packages");
}

#[test]
fn a_bad_step_is_caught() {
    let ok = || Step::new("x-y", Group::new(2, BATTERIES), "Do it.").end(&[End::OwnerDied]);
    ok().check().unwrap();
    assert!(ok().budget_s(181).check().is_err());
    assert!(Step::new("Bad Id", Group::new(2, BATTERIES), "Do it.").check().is_err());
    assert!(Step::new("x-y", Group::new(9, BATTERIES), "Do it.").check().is_err());
    assert!(Step::new("x-y", Group::new(2, 5), "Do it.").check().is_err());
    assert!(Step::new("x-y", Group::new(2, BATTERIES), "two\nlines").check().is_err());
    assert!(ok().bar("nothing", Test::AtLeast(1.0)).check().is_err(), "a bar on an undeclared measure");
    assert!(ok().start(&[Cond::BotsInPlace]).check().is_err(), "BotsInPlace needs a Hold");
}

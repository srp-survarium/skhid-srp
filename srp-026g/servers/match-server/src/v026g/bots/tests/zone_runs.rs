use mapdata::{MapData, RayTree, Rotation, Shape, ShapeKind, Spawn, WalkGrid, Zone, ZoneKind};

use super::super::anomalies::Control;
use super::super::hazards::Activity;
use super::*;
use crate::v026g::bots::body::tests::{floor, wall_x};

fn zone(name: &str, center: Vec3, half: Vec3) -> Zone {
    Zone {
        kind: ZoneKind::Damage,
        name: name.into(),
        team: -1,
        position: center,
        damage_type: "electric".into(),
        damage: None,
        shapes: vec![Shape {
            kind: ShapeKind::Box,
            center,
            rotation: Rotation::IDENTITY,
            half,
        }],
    }
}

/// The field, walls across x = 40 over the `walls` z ranges, the zones.
fn field(walls: &[(f32, f32)], zones: Vec<Zone>) -> Arc<World> {
    let mut triangles = floor(0.0, 0.0, 80.0, 40.0, 0.0).to_vec();
    for &(z0, z1) in walls {
        triangles.extend(wall_x(40.0, z0, z1));
    }
    let grid = WalkGrid::build(
        &triangles,
        [0.0, 0.0, 80.0, 40.0],
        0.25,
        mapdata::Body::default(),
    );
    let spawn = |team: i32, x: f32, z: f32| Spawn {
        position: Vec3::new(x, 0.0, z),
        rotation: Vec3::ZERO,
        team,
        priority: 0,
        point_id: 0,
        layer: "default".into(),
    };
    let map = MapData {
        spawns: vec![spawn(0, 5.0, 10.0), spawn(1, 75.0, 10.0)],
        zones,
        ..MapData::default()
    };
    let sight = RayTree::build(&triangles);
    Arc::new(World::assemble(map, grid.into(), sight.into()))
}

/// The wall's only gap (z 25-30; with `detour` also z 0-3), a 4 x 5 m
/// zone in it.
fn gap(detour: bool) -> Arc<World> {
    let walls: &[(f32, f32)] = if detour {
        &[(3.0, 25.0), (30.0, 40.0)]
    } else {
        &[(0.0, 25.0), (30.0, 40.0)]
    };
    let zone = zone("gap", Vec3::new(40.0, 1.0, 27.5), Vec3::new(2.0, 1.0, 2.5));
    field(walls, vec![zone])
}

/// On 6 s, off 6 s, from `first_on` (level_03's electric pairs).
fn timer(first_on: u32) -> Activity {
    Activity::Spans(
        (0..20)
            .map(|k| (first_on + k * 12_000, first_on + k * 12_000 + 6_000))
            .collect(),
    )
}

struct Run {
    world: Arc<World>,
    start: Vec3,
    goal: Option<Vec3>,
    /// Zone 0's timing, put back every poll; none: always on.
    activity: Option<Activity>,
    /// The body put here at this time (pushed in).
    push: Option<(u32, Vec3)>,
    /// Its route dropped this often, ms (a chase: the goal moves on).
    replan_every: Option<u32>,
    /// Its intent says the zone's floor is its choice
    /// (`Intent::zone_ok`, a battery's crossing) until this time, ms.
    zone_ok_until: u32,
    until: u32,
}

fn run(world: Arc<World>, activity: Option<Activity>) -> Run {
    Run {
        world,
        start: Vec3::new(12.0, 0.0, 27.5),
        goal: Some(Vec3::new(60.0, 0.0, 27.5)),
        activity,
        push: None,
        replan_every: None,
        zone_ok_until: 0,
        until: 60_000,
    }
}

#[derive(Debug, Default)]
struct Seen {
    /// Ms on a zone's floors while it is on, and the longest stretch.
    on_ms: u32,
    longest_on_ms: u32,
    /// Steps off the zone's floors back west, the goal east of it.
    back_outs: u32,
    /// When it got within 1.5 m of its goal.
    arrived_ms: Option<u32>,
}

fn drive(run: Run) -> Seen {
    let world = run.world.clone();
    let teams = [0u8];
    let mut bots = Bots::new(
        0xA70A,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true],
        &[None],
        Arc::new(zone_tuning()),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    game.bodies[0].node = world.grid.locate(run.start, 1.0);
    game.bodies[0].feet = run.start;
    bots.entered(0, true, 0);
    let middle = world.map.zones[0].shapes[0].center.x;
    let mut seen = Seen::default();
    let (mut clock, mut next_world) = (0u32, 1_000u32);
    let (mut pushed, mut on_run, mut was_in) = (false, 0u32, false);
    while clock < run.until {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if let Some((at, to)) = run.push
            && !pushed
            && clock >= at
        {
            pushed = true;
            game.bodies[0].node = world.grid.locate(to, 1.0);
            game.bodies[0].feet = to;
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) =
                game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        if let (Some(activity), Some(tactics)) = (&run.activity, bots.tactics.as_mut()) {
            tactics.hazards.zones[0].control = Control::Timer;
            tactics.hazards.zones[0].activity = activity.clone();
        }
        let now = Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        let view = bots.tracker.view(&now, &bots.tuning.kin, Some(&world));
        bots.features.searching.left = SEARCHES_PER_POLL;
        bots.features.searching.floors = bots.tuning.think.search_floors_per_poll as usize;
        bots.go_on_searching();
        if let Some(bot) = bots.bots[0].as_mut() {
            bot.intent = match run.goal {
                Some(at) => Intent::to(at, Pace::Run).within(1.0),
                None => Intent::stay(),
            };
            if clock < run.zone_ok_until {
                bot.intent = bot.intent.zone_ok();
            }
            if run.replan_every.is_some_and(|every| clock % every == 0) {
                bot.route = None;
            }
        }
        let mut made = Vec::new();
        bots.ticks(0, &now, &view, &mut made);
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        if clock % 100 != 0 {
            continue;
        }
        let hazards = &bots.tactics.as_ref().unwrap().hazards;
        let feet = game.bodies[0].feet;
        let node = world.grid.locate(feet, 1.0);
        let marked = node.is_some_and(|node| hazards.zone_of(node).is_some());
        if node.is_some_and(|node| hazards.on_at(node, clock)) {
            seen.on_ms += 100;
            on_run += 100;
            seen.longest_on_ms = seen.longest_on_ms.max(on_run);
        } else {
            on_run = 0;
        }
        if was_in && !marked && run.goal.is_some_and(|goal| goal.x > middle) && feet.x < middle
        {
            seen.back_outs += 1;
        }
        was_in = marked;
        if let Some(goal) = run.goal
            && seen.arrived_ms.is_none()
            && feet.distance_xz(goal) <= 1.5
        {
            seen.arrived_ms = Some(clock);
        }
    }
    eprintln!("{seen:?}");
    seen
}

/// Runs 1 and 3: a route through the only gap (or one with a detour) is
/// not walked onto the zone's floors while it is on (from 6.5 s): it
/// crosses in an off span, or goes round. Stock: 1.2 s on them, both.
#[test]
fn a_bot_on_its_way_keeps_off_a_zone_while_it_is_on() {
    for detour in [false, true] {
        let seen = drive(run(gap(detour), Some(timer(6_500))));
        assert_eq!(seen.on_ms, 0, "detour {detour}: {seen:?}");
        assert!(seen.arrived_ms.is_some(), "detour {detour}: {seen:?}");
    }
}

/// Run 8: the only way, its route planned again every 1.5 s (a chase):
/// it crosses forward, never backing out the way it came. Stock: 2
/// back-outs, the goal at 41.7 s.
#[test]
fn a_chasing_bot_crosses_a_switching_zone_without_backing_out() {
    let mut chase = run(gap(false), Some(timer(6_500)));
    chase.replan_every = Some(1_500);
    let seen = drive(chase);
    assert_eq!(seen.back_outs, 0, "{seen:?}");
    assert!(seen.arrived_ms.is_some_and(|at| at <= 35_000), "{seen:?}");
}

/// Runs 4 and 4b: with no goal (holding), or standing at its goal, in
/// the gap when the zone comes on: out as it does. Stock: 5.9 s on its
/// floors each on span.
#[test]
fn a_bot_with_no_goal_walks_out_of_a_zone_coming_on() {
    for goal in [None, Some(Vec3::new(40.3, 0.0, 27.5))] {
        let mut hold = run(gap(false), Some(timer(6_500)));
        hold.start = Vec3::new(40.0, 0.0, 27.5);
        hold.goal = goal;
        hold.until = 30_000;
        let seen = drive(hold);
        assert!(seen.on_ms <= 1_000, "goal {goal:?}: {seen:?}");
    }
}

/// Run 6b: with no goal, pushed into a small zone always on: out at
/// once (within its next fixes: the stand-in sends them each second).
/// Stock: 27 s, to the run's end.
#[test]
fn a_bot_with_no_goal_pushed_into_a_zone_walks_out() {
    let small = zone(
        "small",
        Vec3::new(30.0, 1.0, 20.0),
        Vec3::new(2.0, 1.0, 2.0),
    );
    let mut pushed = run(field(&[], vec![small]), None);
    pushed.start = Vec3::new(20.0, 0.0, 20.0);
    pushed.goal = None;
    pushed.push = Some((3_000, Vec3::new(30.0, 0.0, 20.0)));
    pushed.until = 30_000;
    let seen = drive(pushed);
    assert!(seen.on_ms <= 3_000, "{seen:?}");
}

/// A battery's crossing (`collect`): with no goal in a zone always on,
/// a bot whose intent says the zone is its choice (`Intent::zone_ok`)
/// is not walked out; once the intent drops it (the crossing's re-check
/// of its health failed, the take given up), out as any. Without it from
/// the start, out at once (the run above).
#[test]
fn a_bot_in_a_zone_by_choice_stays_and_walks_out_once_it_is_not() {
    let small = zone(
        "small",
        Vec3::new(30.0, 1.0, 20.0),
        Vec3::new(2.0, 1.0, 2.0),
    );
    let world = field(&[], vec![small]);
    let pushed = |zone_ok_until: u32| {
        let mut pushed = run(world.clone(), None);
        pushed.start = Vec3::new(20.0, 0.0, 20.0);
        pushed.goal = None;
        pushed.push = Some((3_000, Vec3::new(30.0, 0.0, 20.0)));
        pushed.zone_ok_until = zone_ok_until;
        pushed.until = 30_000;
        drive(pushed)
    };
    let by_choice = pushed(30_000);
    assert!(by_choice.on_ms >= 25_000, "{by_choice:?}");
    let dropped = pushed(12_000);
    assert!((8_000..=12_000).contains(&dropped.on_ms), "{dropped:?}");
    let never = pushed(0);
    assert!(never.on_ms <= 3_000, "{never:?}");
}

/// Run 5: pushed 25 m into a big zone always on, deeper than the way
/// out's search reaches: out by the exit field, then on to its goal.
/// Stock: its escapes found nothing, it stood 37 s.
#[test]
fn a_bot_deep_in_a_big_zone_walks_out() {
    let big = zone(
        "big",
        Vec3::new(25.0, 1.0, 20.0),
        Vec3::new(25.0, 1.0, 20.0),
    );
    let mut deep = run(field(&[], vec![big]), None);
    deep.start = Vec3::new(60.0, 0.0, 20.0);
    deep.goal = Some(Vec3::new(70.0, 0.0, 30.0));
    deep.push = Some((3_000, Vec3::new(25.0, 0.0, 20.0)));
    deep.until = 40_000;
    let seen = drive(deep);
    assert!(seen.on_ms <= 15_000, "{seen:?}");
    assert!(seen.arrived_ms.is_some(), "{seen:?}");
}

/// The owner's set (the defaults) but for `feel.unclear_aim`: with it on, the four runs that walk a bot out of a zone
/// (no goal, pushed in, in the gap when the zone comes on, deep in a big one) leave it standing in the zone. Found by
/// A128 when the defaults went on: each passes with only that one option off (the options were not run together before).
fn zone_tuning() -> Tuning {
    let mut tuning = Tuning::default();
    tuning.feel.unclear_aim = false;
    tuning
}

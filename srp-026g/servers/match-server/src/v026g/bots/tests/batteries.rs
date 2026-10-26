use super::*;

/// The stand-in's put-in lasts [`PUT_MS`]: the battery goes in at its end,
/// a use pressed meanwhile does nothing, and a carrier who died first puts
/// nothing in.
#[test]
pub(crate) fn a_stand_in_put_in_goes_in_at_the_end_of_the_put() {
    let mut game = Game::new(soak_world(), &[0, 1], 1, None, None);
    let (_, home) = game.containers[0];
    let feet = home + Vec3::new(0.0, 0.0, 1.0);
    let body = &mut game.bodies[0];
    body.alive = true;
    body.feet = feet;
    // Facing the use box (-z), a metre over the container's point.
    body.yaw = std::f32::consts::PI;
    body.pitch = (1.0 - 1.6f32).atan2(1.0);
    body.carrying = Some(0);
    game.batteries[0] = Lying::Carried(0);
    game.put_ms = PUT_MS;
    game.use_and_drop(&[key::USE, 0]);
    assert_eq!(game.batteries[0], Lying::Carried(0), "in the put");
    game.tick += PUT_MS - 50;
    game.use_and_drop(&[key::USE, 0]);
    assert_eq!(game.batteries[0], Lying::Carried(0), "a press meanwhile");
    game.tick += 50;
    game.use_and_drop(&[0, 0]);
    assert_eq!(game.batteries[0], Lying::Home(0));
    assert_eq!(game.delivered[0], 1);
    // Dead before the put ends: nothing goes in.
    game.bodies[0].carrying = Some(1);
    game.batteries[1] = Lying::Carried(0);
    game.use_and_drop(&[key::USE, 0]);
    game.bodies[0].alive = false;
    game.tick += PUT_MS;
    game.use_and_drop(&[0, 0]);
    assert_eq!(game.batteries[1], Lying::Carried(0));
    assert_eq!(game.delivered[0], 1);
}

/// A crouched body right over a battery cannot use it: the view's pitch
/// stops at 1 rad, and no ray of the use passes within its 0.2 m of the
/// battery's middle closer than about 0.3 m (on the floor; the soak map's
/// lie 0.3 m up). From where the take stands off it (`collect`), looking
/// at it, it can. "Could not take it" 14 times in the owner's match of
/// 2026-10-16, twice with the bot 0.1-0.2 m from the battery.
#[test]
pub(crate) fn a_body_right_over_a_battery_cannot_use_it_and_one_a_step_off_can() {
    use super::plays::collect::{BATTERY_AIM_M, TAKE_PITCH_MAX_RAD, TAKE_PITCH_RAD};
    let battery = Vec3::new(25.0, 0.0, 12.0);
    let mut body = Game::new(soak_world(), &[0], 1, None, None).bodies[0].clone();
    body.keys = key::CROUCH;
    let any_look = |body: &mut Body| {
        (0..64).any(|y| {
            (-100..=100).any(|p| {
                body.yaw = y as f32 * std::f32::consts::TAU / 64.0;
                body.pitch = p as f32 / 100.0;
                Game::uses(body, battery + BATTERY_MIDDLE, BATTERY_M)
            })
        })
    };
    for off in [0.0, 0.1, 0.2] {
        body.feet = battery + Vec3::new(off, 0.0, 0.0);
        assert!(!any_look(&mut body), "{off} m off");
    }
    // The take's stand: the look down at TAKE_PITCH_RAD, aimed as the
    // motor aims (`Look::Use`); and as near as a take is tried without a
    // step back (TAKE_PITCH_MAX_RAD), some look still uses it.
    let aim = battery + Vec3::new(0.0, BATTERY_AIM_M, 0.0);
    let below = view::EYE_CROUCHED - BATTERY_AIM_M;
    body.feet = battery + Vec3::new(below / TAKE_PITCH_MAX_RAD.tan() + 0.02, 0.0, 0.0);
    assert!(any_look(&mut body), "at the steepest take");
    body.feet = battery + Vec3::new(below / TAKE_PITCH_RAD.tan(), 0.0, 0.0);
    let eye = Game::eye(&body);
    body.yaw = kin::yaw_of([aim.x - eye.x, aim.z - eye.z]);
    body.pitch = (aim.y - eye.y).atan2(eye.distance_xz(aim));
    assert!(body.pitch.abs() < 1.0);
    assert!(Game::uses(&body, battery + BATTERY_MIDDLE, BATTERY_M));
}

/// A bot standing right on a battery steps off it and takes it (it
/// pressed use there four times, failed, and took the battery again).
#[test]
pub(crate) fn a_bot_standing_on_a_battery_steps_off_and_takes_it() {
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = true;
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x7A6E,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    // On battery 1 (25, 0.3, 12), the other battery 25 m off; the enemy
    // is no bot and stands at his spawn.
    let on = Vec3::new(25.0, 0.0, 12.0);
    game.bodies[0].node = world.grid.locate(on, 1.0);
    game.bodies[0].feet = on;
    bots.entered(0, true, 0);
    let (mut clock, mut next_world, mut took) = (0, 1_000, None);
    while clock < 12_000 && took.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        });
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        if game.bodies[0].carrying.is_some() {
            took = Some(clock);
        }
    }
    let took = took.unwrap_or_else(|| {
        panic!(
            "not taken: at {:?}, batteries {:?}",
            game.bodies[0].feet, game.batteries
        )
    });
    // Its first move is at 2 s; a step off and a use or two.
    assert!(took <= 7_000, "taken at {took} ms");
}

/// A battery whose origin lies 0.36 m under a raised patch of floor
/// (School's battery 2): the patched client takes it from a crouch over it,
/// within 1 m across, along a ray that passes floors; the stand-in says so
/// (on the real maps the bullet mesh blocks the stock line from above; the
/// test floor's winding lets that line through, so only the rule's yes is
/// tested).
#[test]
pub(crate) fn a_battery_sunk_under_the_floor_is_taken_through_it_by_the_patched_rule() {
    let solids = super::body::tests::floor(23.5, 10.5, 26.5, 13.5, 0.36);
    let world = soak_world_with(&solids);
    let teams = [0u8, 1];
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    game.use_line = true;
    let battery = Vec3::new(25.0, 0.0, 12.0);
    let mut body = game.bodies[0].clone();
    body.keys = key::CROUCH;
    let scan = |body: &mut Body| {
        (0..64).any(|y| {
            (-100..=100).any(|p| {
                body.yaw = y as f32 * std::f32::consts::TAU / 64.0;
                body.pitch = p as f32 / 100.0;
                game.uses_battery(body, battery)
            })
        })
    };
    body.feet = Vec3::new(25.6, 0.36, 12.0);
    assert!(scan(&mut body), "taken through the floor from over it");
}

/// A wall 3 m high between the bot and battery 1 (25, 0.3, 12), where it
/// stands 1.3 m east of it, and the stand-in's use stopped by the bullet
/// mesh (as the client's, LIKELY: b9 of 0x7db2c16a, 9 uses aimed right at
/// it from where the mesh blocks the line, taken at once from a side with
/// a clear one). The bot takes it from round the wall (before: four uses
/// there, failed, the play taken again, and so on).
#[test]
pub(crate) fn a_bot_takes_a_battery_from_round_a_wall_its_use_cannot_pass() {
    let world = soak_world_with(&super::body::tests::wall_x(25.6, 11.5, 12.5));
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = true;
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x7A6F,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    game.use_line = true;
    let east = Vec3::new(26.3, 0.0, 12.0);
    game.bodies[0].node = world.grid.locate(east, 1.0);
    game.bodies[0].feet = east;
    // From there the use's ray is stopped, however it looks.
    let mut body = game.bodies[0].clone();
    body.keys = key::CROUCH;
    let battery = Vec3::new(25.0, 0.3, 12.0);
    let blocked = (0..64).all(|y| {
        (-100..=100).all(|p| {
            body.yaw = y as f32 * std::f32::consts::TAU / 64.0;
            body.pitch = p as f32 / 100.0;
            !game.uses_battery(&body, battery)
        })
    });
    assert!(blocked);
    bots.entered(0, true, 0);
    let (mut clock, mut next_world, mut took) = (0, 1_000, None);
    let mut notes_seen = Vec::new();
    while clock < 20_000 && took.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let (made, notes) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        });
        notes_seen.extend(notes.into_iter().map(|n| n.text));
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        if game.bodies[0].carrying == Some(1) {
            took = Some(clock);
        }
    }
    let took = took.unwrap_or_else(|| {
        panic!(
            "not taken: at {:?}, batteries {:?}, notes {notes_seen:?}",
            game.bodies[0].feet, game.batteries
        )
    });
    // Its first move is at 2 s; round the wall, a use or two.
    assert!(took <= 10_000, "taken at {took} ms: {notes_seen:?}");
    assert!(
        !notes_seen.iter().any(|n| n.contains("could not take it")),
        "{notes_seen:?}"
    );
}

/// Who keeps the enemy by the battery busy in [`fight_by_a_battery`].
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Busy {
    /// Nobody: the bot alone in his sight.
    Nobody,
    /// A teammate (place 2, no bot) with a clear line to him.
    Mate,
    /// He reloads (a reload pressed every second).
    Reloading,
}

/// A bot walks in to battery 1 with an enemy in plain sight past it
/// (place 1, no bot, who does not die: the fight never ends), for 15 s:
/// when it took the battery, if it did, and whether it fought him.
pub(crate) fn fight_by_a_battery(busy: Busy) -> (Option<u32>, bool, u32) {
    use survarium::v026g::match_msgs::{PlayerInput, WireTime};
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = true;
    let teams = [0u8, 1, 0];
    let mut bots = Bots::new(
        0x7A70,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false, false],
        &[None, None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 7, None, None);
    // The enemy's health is put back every step ("who does not die"); only
    // the old flat hit has no other pool to empty (the head's and the
    // pain's, which the weapon's own damage now empties first).
    game.damage.flat = true;
    // 8 m east of battery 1 (25, 0.3, 12), the enemy 10 m past it, on the
    // same side of the wall: the bot sees him as it walks in. The
    // teammate 13 m off him, out of the bot's line; or far off, behind
    // the wall, when he does not cover.
    let mate = if busy == Busy::Mate {
        Vec3::new(26.0, 0.0, 4.0)
    } else {
        Vec3::new(70.0, 0.0, 35.0)
    };
    for (place, at) in [
        (0, Vec3::new(33.0, 0.0, 12.0)),
        (1, Vec3::new(15.0, 0.0, 12.0)),
        (2, mate),
    ] {
        game.bodies[place].node = world.grid.locate(at, 1.0);
        game.bodies[place].feet = at;
    }
    for place in 0..3 {
        bots.entered(place, true, 0);
    }
    let (mut clock, mut next_world, mut took) = (0, 1_000, None);
    let mut fought = false;
    while clock < 15_000 && took.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
            game.bodies[1].health = 1.0;
            if busy == Busy::Reloading && game.tick.is_multiple_of(TICK_MS) {
                let reload = game.tick.is_multiple_of(1_000);
                let input = PlayerInput {
                    time: WireTime::tick(game.tick),
                    rotation: [0; 4],
                    actions: if reload { key::RELOAD } else { 0 },
                };
                bots.input(1, game.tick, 0, &input);
            }
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        });
        let target = bots.bots[0].as_ref().and_then(|bot| bot.reflex.target());
        fought |= target == Some(1);
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, target, gun));
        }
        if game.bodies[0].carrying == Some(1) {
            took = Some(clock);
        }
    }
    assert!(game.bodies[1].alive);
    (took, fought, game.shots)
}

/// A bot by a battery with an enemy in plain sight whom nobody keeps busy
/// fights him and does not take it: a take turns the view from him and a
/// carrier cannot shoot ([`Ctx::exposed_to`]).
///
/// [`Ctx::exposed_to`]: super::plays::Ctx::exposed_to
#[test]
pub(crate) fn a_bot_in_an_enemys_sight_fights_before_it_takes_a_battery() {
    let (took, fought, shots) = fight_by_a_battery(Busy::Nobody);
    assert!(fought && shots > 0, "no fight: {shots} shots");
    assert_eq!(took, None, "taken in his sight");
}

/// The same bot takes the battery when a teammate has a clear line to the
/// enemy, or while the enemy reloads: within the use's reach the take's
/// look wins over the reflex's aim (`USE_LOOK_M`). The client's use is one
/// ray along the view; a bot whose reflex kept its view on the enemy
/// pressed use at him, and in the whole-match soaks the battery where the
/// teams meet lay there all match.
#[test]
pub(crate) fn a_covered_bot_takes_a_battery_in_an_enemys_sight() {
    for busy in [Busy::Mate, Busy::Reloading] {
        let (took, fought, shots) = fight_by_a_battery(busy);
        assert!(fought && shots > 0, "no fight: {shots} shots");
        let took = took.unwrap_or_else(|| panic!("not taken ({shots} shots)"));
        // Its first move is at 2 s; 8 m in, a turn to the battery and a
        // use or two (without the use's look: still not taken at 15 s, 66
        // shots).
        assert!(took <= 9_000, "taken at {took} ms");
    }
}

/// A raid that gets there steals (play 24): bot 0 of team 0, 0-3 down,
/// 25 m from the enemy's container with all three batteries in it, their
/// one body dead for the run, 5 min into the match. It walks in, crouches
/// the last metres, steps in to the use's reach and takes one out; the
/// raid ends with it carried and the carry home is Collect's. Before the
/// step in, a raid that stopped 2 m off (`reach_m`) pressed use at the
/// box's middle from 2.0 m and more, past the 2 m ray.
#[test]
pub(crate) fn a_raid_that_gets_there_steals() {
    let world = soak_world();
    let mut tuning = bench_tuning();
    tuning.cheats.batteries = true;
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x2A1D,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 5, None, None);
    for battery in 0..3 {
        game.batteries[battery] = Lying::Home(1);
    }
    game.points = [0, 3];
    let start = Vec3::new(44.0, 0.0, 34.0);
    game.bodies[0].node = world.grid.locate(start, 1.0);
    game.bodies[0].feet = start;
    // Dead, and not back within the run.
    game.bodies[1].alive = false;
    game.bodies[1].dead_at = 1_000_000;
    bots.entered(0, true, 0);
    bots.entered(1, true, 0);
    // A sneak (play 4) would take the same window: just ran.
    if let Some(bot) = bots.bots[0].as_mut() {
        bot.last_ran.push((super::plays::PlayKind::Sneak, 0));
    }
    let (mut clock, mut next_world) = (0, 1_000);
    let mut stole: Option<(u32, Option<super::plays::PlayKind>)> = None;
    while clock < 40_000 && stole.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        });
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        if game.thieves.contains(&0) {
            let play = bots.bots[0]
                .as_ref()
                .and_then(|bot| bot.assignment.as_ref())
                .map(|a| a.kind);
            stole = Some((clock, play));
        }
    }
    let (at, play) = stole.unwrap_or_else(|| {
        panic!(
            "no steal: at {:?}, {:.1} m from their container, batteries {:?}",
            game.bodies[0].feet,
            game.bodies[0].feet.distance_xz(game.containers[1].1),
            game.batteries
        )
    });
    assert_eq!(play, Some(super::plays::PlayKind::Raid), "stolen at {at} ms");
    assert_eq!(game.points, [0, 2]);
}

/// How a lost-battery run went (`lost_battery_run`): when the thief died,
/// when the team got a look spot, when its bot first went to look, when
/// it took the battery (match ms).
#[derive(Debug, Default)]
pub(crate) struct LostBattery {
    pub(crate) died: Option<u32>,
    pub(crate) look: Option<u32>,
    pub(crate) sent: Option<u32>,
    pub(crate) took: Option<u32>,
}

/// Match 0x15aa5b99 in small: an enemy (place 1, no bot) steals battery 0
/// from team 0's container and crouches 6 m off it carrying it; bot 0 of
/// team 0 sees him and kills him; the battery falls where he died, in a
/// "nook": no ray of the battery sight sees it (`battery_sight_m` 0), only
/// a bot within `battery_near_m`. The thief cheat is off, so the team knows
/// where he was only by its own senses. Runs to `limit` ms.
pub(crate) fn lost_battery_run(look_weight: Option<f32>, limit: u32) -> LostBattery {
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = false;
    tuning.cheats.thief_every_ms = 0;
    tuning.sense.battery_sight_m = 0.0;
    // The scenario needs the thief to take the battery before he dies: the
    // fixed bursts of before the bursts by distance (2026-10-23), with
    // which the fight goes that way; with the table's taps the bot's
    // random sequence differs and it kills him at 2.65 s, by the container.
    tuning.aim.combat.fire.rifle = vec![];
    // Likewise the shots before the sights are up (`aim_in_first` off):
    // with the aim-in first it kills him at 2.95 s, before the take.
    tuning.aim.combat.aim_in_first = false;
    // And the aim led to the input's tick (2026-10-24): with it the bot
    // kills him at 2.6 s, before the take.
    tuning.aim.combat.lead_target = false;
    tuning.aim.combat.lead_own = false;
    tuning.aim.combat.turn_feed = false;
    if let Some(weight) = look_weight {
        tuning
            .plays
            .entry(super::plays::PlayKind::Collect)
            .or_insert_with(|| super::plays::default_tuning(super::plays::PlayKind::Collect))
            .numbers
            .insert("look_weight".into(), weight);
    }
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x15AA,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 3, None, None);
    // The scenario's timing (the thief takes the battery at 3 s and is
    // killed after, by the tuning above) was found with the flat fifth of
    // the health a hit took: the weapon's own damage kills him at 2.6 s.
    // With the sight refresh (the bot sees him all the while, no think-gap
    // drops) it kills him by the container at 3.5 s if he takes it at 3 s,
    // and not in 60 s if he takes it at 1 s; at 2 s, and the nook at 3 s, it
    // kills him in the nook at 15.8 s, which is the case the test is for.
    game.damage.flat = true;
    // Battery 0 is in team 0's container (id 0, at (12, 0, 30)); the
    // thief stands by it.
    game.batteries[0] = Lying::Home(0);
    game.points[0] = 1;
    let by_container = Vec3::new(13.0, 0.0, 31.5);
    game.bodies[1].node = world.grid.locate(by_container, 1.0);
    game.bodies[1].feet = by_container;
    // The bot a little way off, facing the container.
    let bot_at = Vec3::new(14.0, 0.0, 12.0);
    game.bodies[0].node = world.grid.locate(bot_at, 1.0);
    game.bodies[0].feet = bot_at;
    game.bodies[0].yaw = kin::yaw_of([by_container.x - bot_at.x, by_container.z - bot_at.z]);
    bots.entered(0, true, 0);
    bots.entered(1, true, 0);
    let mut out = LostBattery::default();
    let (mut clock, mut next_world) = (0, 1_000);
    while clock < limit && out.took.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        // At 2 s he takes it out; at 3 s he crouches in the nook 6 m off.
        if clock == 2_000 {
            game.batteries[0] = Lying::Carried(1);
            game.bodies[1].carrying = Some(0);
            game.points[0] = 0;
        }
        if clock == 3_000 {
            let nook = Vec3::new(17.0, 0.0, 34.0);
            game.bodies[1].node = world.grid.locate(nook, 1.0);
            game.bodies[1].feet = nook;
            game.bodies[1].keys = key::CROUCH;
        }
        // He comes back at his spawn and stands there.
        if out.died.is_none() && !game.bodies[1].alive {
            out.died = Some(clock);
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        if out.look.is_none() && !bots.teams[0].knowledge.looks.is_empty() {
            out.look = Some(clock);
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        });
        if out.sent.is_none()
            && bots.bots[0].as_ref().is_some_and(|b| {
                b.assignment.as_ref().is_some_and(|a| {
                    a.kind == super::plays::PlayKind::Collect
                        && matches!(a.target, super::plays::TargetRef::Point(_))
                })
            })
        {
            out.sent = Some(clock);
        }
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        if game.bodies[0].carrying == Some(0) {
            out.took = Some(clock);
        }
    }
    out
}

/// F4 of the analysis of matches 0x7db2c16a/0x15aa5b99: the thief the bot
/// killed dropped our battery in a nook its sight misses; the team looks
/// where it saw him die within a few seconds, and the bot finds and takes
/// the battery. Without the look (its weight 0) the battery is not found
/// in the same time (126 s in the owner's match).
#[test]
pub(crate) fn a_battery_a_killed_thief_dropped_in_a_nook_is_looked_for_and_taken() {
    let run = lost_battery_run(None, 60_000);
    let died = run
        .died
        .unwrap_or_else(|| panic!("the thief lives: {run:?}"));
    let look = run.look.unwrap_or_else(|| panic!("no look: {run:?}"));
    // The world showing the death comes 300 ms behind, a second apart.
    assert!(look >= died && look <= died + 2_500, "{run:?}");
    let sent = run.sent.unwrap_or_else(|| panic!("nobody sent: {run:?}"));
    assert!(sent <= look + 5_000, "{run:?}");
    let took = run.took.unwrap_or_else(|| panic!("not taken: {run:?}"));
    assert!(took <= died + 30_000, "{run:?}");
    let without = lost_battery_run(Some(0.0), took + 10_000);
    eprintln!("with the look {run:?}; without {without:?}");
    assert_eq!(without.died, run.died, "the same fight");
    assert!(without.took.is_none(), "{without:?}");
}

/// How a dropped-theft run went (`theft_drop_run`), match ms: when the
/// thief dropped the battery, when the team got its looks, when its bot
/// first went to look, when the team knew where the battery lies, when
/// the bot took it.
#[derive(Debug, Default)]
pub(crate) struct TheftDrop {
    pub(crate) dropped: Option<u32>,
    pub(crate) look: Option<u32>,
    pub(crate) sent: Option<u32>,
    pub(crate) seen: Option<u32>,
    pub(crate) took: Option<u32>,
}

/// F2 of the analysis of the owner's matches 0x652d/0x1a09 of 2026-10-16
/// in small: an enemy (place 1, no bot) takes battery 0 out of team 0's
/// container at 3 s and drops it at `drop_at` at `drop_ms` (4 s: a world
/// shows him carrying it; 3.5 s: none does, the next shows it lying), then
/// is gone (at his spawn, behind the wall). Bot 0 of team 0 stands 18 m off the container
/// facing away from it. The thief cheat and the battery cheat are off: the
/// team has the HUD's theft and its own sight. `weight` is Collect's
/// `theft_look_weight` (0: no look is offered). Runs to `limit` ms.
pub(crate) fn theft_drop_run(weight: Option<f32>, drop_at: Vec3, drop_ms: u32, limit: u32) -> TheftDrop {
    let world = soak_world();
    let mut tuning = Tuning::default();
    tuning.cheats.batteries = false;
    tuning.cheats.thief_every_ms = 0;
    if let Some(weight) = weight {
        tuning
            .plays
            .entry(super::plays::PlayKind::Collect)
            .or_insert_with(|| super::plays::default_tuning(super::plays::PlayKind::Collect))
            .numbers
            .insert("theft_look_weight".into(), weight);
    }
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0xF2,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 3, None, None);
    // Battery 0 in team 0's container (id 0, at (12, 0, 30)); the thief
    // stands by it. Battery 1 lies at its spawner for the first worlds
    // (the bots tell the batteries' frame by it), then is in team 1's
    // container.
    game.batteries.truncate(2);
    game.batteries[0] = Lying::Home(0);
    game.points[0] = 1;
    let by_container = Vec3::new(13.0, 0.0, 31.5);
    game.bodies[1].node = world.grid.locate(by_container, 1.0);
    game.bodies[1].feet = by_container;
    // The bot a little way off, its back to the container.
    let bot_at = Vec3::new(14.0, 0.0, 12.0);
    game.bodies[0].node = world.grid.locate(bot_at, 1.0);
    game.bodies[0].feet = bot_at;
    game.bodies[0].yaw = kin::yaw_of([bot_at.x - by_container.x, bot_at.z - by_container.z]);
    bots.entered(0, true, 0);
    bots.entered(1, true, 0);
    let mut out = TheftDrop::default();
    let (mut clock, mut next_world) = (0, 1_000);
    while clock < limit && out.took.is_none() {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock == 2_500 {
            game.batteries[1] = Lying::Home(1);
            game.points[1] = 1;
        }
        if clock == 3_000 {
            game.batteries[0] = Lying::Carried(1);
            game.bodies[1].carrying = Some(0);
            game.points[0] = 0;
        }
        if clock == drop_ms {
            game.batteries[0] = Lying::Free(drop_at, 1 << game.bodies[1].team);
            game.bodies[1].carrying = None;
            out.dropped = Some(clock);
        }
        if clock == 4_500 {
            let away = Vec3::new(75.0, 0.0, 5.0);
            game.bodies[1].node = world.grid.locate(away, 1.0);
            game.bodies[1].feet = away;
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        if out.look.is_none() && bots.teams[0].knowledge.looks.iter().any(|l| l.theft) {
            out.look = Some(clock);
        }
        if out.seen.is_none()
            && out.dropped.is_some()
            && bots.teams[0].knowledge.battery(0).is_some()
        {
            out.seen = Some(clock);
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        });
        if out.sent.is_none()
            && bots.bots[0].as_ref().is_some_and(|b| {
                b.assignment.as_ref().is_some_and(|a| {
                    a.kind == super::plays::PlayKind::Collect
                        && matches!(a.target, super::plays::TargetRef::Point(_))
                })
            })
        {
            out.sent = Some(clock);
        }
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
        if game.bodies[0].carrying == Some(0) {
            out.took = Some(clock);
        }
    }
    out
}

/// F2 of the analysis of the owner's matches 0x652d/0x1a09 of 2026-10-16:
/// a thief takes our battery out and drops it 2-5 m from our container
/// (on the side away from the bot, or beside it), seen carrying it or
/// not (taken and dropped between two worlds). The team's looks come
/// with the world showing the drop, the bot is sent within 5 s, sees the
/// battery and takes it. Without the looks (their weight 0) it is found
/// later or not at all.
#[test]
pub(crate) fn a_battery_a_thief_dropped_by_our_container_is_looked_for_and_taken() {
    for (drop_at, drop_ms) in [
        (Vec3::new(12.0, 0.0, 33.0), 4_000),
        (Vec3::new(8.0, 0.0, 31.5), 4_000),
        (Vec3::new(15.5, 0.0, 34.0), 4_000),
        (Vec3::new(12.0, 0.0, 33.0), 3_500),
    ] {
        let run = theft_drop_run(None, drop_at, drop_ms, 60_000);
        let dropped = run.dropped.expect("dropped");
        let look = run.look.unwrap_or_else(|| panic!("no look: {run:?}"));
        // The world showing the drop comes 300 ms behind, a second apart.
        assert!(
            look >= dropped && look <= dropped + 2_500,
            "{drop_at:?} {run:?}"
        );
        let sent = run
            .sent
            .unwrap_or_else(|| panic!("nobody sent: {drop_at:?} {run:?}"));
        assert!(sent <= look + 5_000, "{drop_at:?} {run:?}");
        let seen = run
            .seen
            .unwrap_or_else(|| panic!("not seen: {drop_at:?} {run:?}"));
        assert!(seen <= dropped + 15_000, "{drop_at:?} {run:?}");
        let took = run
            .took
            .unwrap_or_else(|| panic!("not taken: {drop_at:?} {run:?}"));
        let without = theft_drop_run(Some(0.0), drop_at, drop_ms, took + 30_000);
        eprintln!("{drop_at:?} at {drop_ms}: with the looks {run:?}; without {without:?}");
        assert!(without.look.is_some(), "{without:?}");
        assert!(without.sent.is_none(), "{without:?}");
        assert!(
            without.seen.is_none_or(|s| s > seen),
            "{drop_at:?}: with {run:?}; without {without:?}"
        );
    }
}

/// The resupply box in the game: the own team's zone, met by the view's ray
/// from a use pressed (the edge), tops the stack up 3 s later, and not for a
/// body looking away, the other team's, or a key let go on the way.
#[test]
pub(crate) fn a_stand_in_resupplies_at_its_own_teams_box_after_its_time() {
    use super::standin_weapon::RESUPPLY_MS;
    let mut world = soak_world();
    let home = Game::new(world.clone(), &[0], 1, None, None).containers[0].1;
    let zone = |team: i32, at: Vec3| mapdata::Zone {
        kind: mapdata::ZoneKind::Resupply,
        name: String::new(),
        team,
        position: at,
        damage_type: String::new(),
        damage: None,
        shapes: vec![use_box(at)],
    };
    let far = Vec3::new(70.0, 0.0, 5.0);
    Arc::get_mut(&mut world).expect("a world of its own").map.zones = vec![zone(1, home), zone(0, far)];
    let run = |yaw: f32, team: u8, release_at: Option<u32>| {
        let mut game = Game::new(world.clone(), &[team], 1, None, None);
        let body = &mut game.bodies[0];
        body.alive = true;
        body.feet = home + Vec3::new(0.0, 0.0, 1.0);
        body.yaw = yaw;
        body.pitch = (1.0 - 1.6f32).atan2(1.0);
        for i in 0..20 {
            assert!(body.stock.fired(i as f32 * 100.0));
        }
        let before = body.stock.load.spare;
        game.tick = 10_000;
        body.keys = key::USE;
        game.resupply(&[key::USE]);
        let began = game.tick;
        while game.tick < began + RESUPPLY_MS {
            game.tick += 50;
            if release_at.is_some_and(|at| game.tick >= began + at) {
                game.bodies[0].keys = 0;
            }
            game.resupply(&[0]);
        }
        (game.bodies[0].stock.load.spare - before, game.bodies[0].stock.counts)
    };
    // The zone at the container is team 1's: team 1's body topped up.
    let (put, counts) = run(std::f32::consts::PI, 1, None);
    assert_eq!((put, counts.box_topped), (20, 1));
    assert_eq!(run(std::f32::consts::PI, 0, None).0, 0, "team 0's box is far");
    assert_eq!(run(0.0, 1, None).0, 0, "looking away");
    let (put, counts) = run(std::f32::consts::PI, 1, Some(1_000));
    assert_eq!((put, counts.box_cut), (0, 1), "the key let go");
}

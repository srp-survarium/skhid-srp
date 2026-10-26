use super::*;

use crate::v026g::bots::traps::{Habit, TrapKit};

/// One bot of team 0 on the stand-in field, with a human of team 1 far off,
/// for `seconds`. `kit`: the traps it carries. Returns the bot's tick-by-tick
/// actions, the pitch its body had at each, and the bots.
fn run(kit: Option<TrapKit>, tuning: Tuning, seconds: u32) -> (Vec<(u32, u32, f32)>, Bots) {
    run_with(kit, tuning, seconds, None)
}

/// [`run`], with an armed trap at `armed` in every world's objects section.
pub(super) fn run_with(
    kit: Option<TrapKit>,
    tuning: Tuning,
    seconds: u32,
    armed: Option<Vec3>,
) -> (Vec<(u32, u32, f32)>, Bots) {
    let world = soak_world();
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x7A95,
        survarium::v026g::maps::mode::TEAM_DEATHMATCH,
        0,
        &teams,
        &[true, false],
        &[None, None],
        Arc::new(tuning),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    let mut game = Game::new(world.clone(), &teams, 5, Some(30), None);
    for (place, at) in [
        (0, Vec3::new(14.0, 0.0, 12.0)),
        (1, Vec3::new(70.0, 0.0, 12.0)),
    ] {
        game.bodies[place].node = world.grid.locate(at, 1.0);
        game.bodies[place].feet = at;
    }
    bots.entered(0, true, 0);
    if let Some(kit) = kit {
        bots.bots[0].as_mut().unwrap().traps = Habit::carry(kit);
    }
    // The team has known enemy humans all over this half of the field for a
    // while (one human a lane cell, so none is throttled).
    let mut who = 100;
    for x in (6..=42).step_by(6) {
        for z in (0..=36).step_by(6) {
            who += 1;
            for k in 0..8 {
                bots.teams[0].knowledge.humans.knew(
                    who,
                    Vec3::new(x as f32, 0.0, z as f32),
                    k * 1_000,
                );
            }
        }
    }
    let mut sent = Vec::new();
    let (mut clock, mut next_world) = (0, 500);
    while clock < seconds * 1_000 {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
        }
        if clock >= next_world {
            next_world += 500;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300)
            {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
            if let Some(at) = armed {
                bots.traps_from_world(&objects_with(at, 1, 1_000), clock);
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
            if made.place == 0 {
                sent.push((made.time, made.input.actions, game.bodies[0].pitch));
            }
            let gun = Arms::fallback().primary.0;
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, None, gun));
        }
    }
    (sent, bots)
}

/// A bot that carries traps, on a field the team knows an enemy human walks,
/// looks down and puts one down: the trap's key is down for one input and up
/// the next, the view steep enough for the head ray, one a life; the team
/// counts it. A bot with none, or with the habit off, never presses it.
#[test]
pub(crate) fn a_bot_that_carries_traps_puts_one_down_with_one_pulse_of_the_key() {
    let key = key::QUICK[1];
    let kit = TrapKit { count: 2, key };
    let (sent, bots) = run(Some(kit), Tuning::default(), 40);
    let down: Vec<usize> = (0..sent.len()).filter(|&i| sent[i].1 & key != 0).collect();
    eprintln!("key down at inputs {down:?}");
    assert_eq!(down.len(), 1, "one pulse: {down:?}");
    let i = down[0];
    assert!(
        i + 1 < sent.len() && sent[i + 1].1 & key == 0,
        "let go the next input"
    );
    // The view was steep near the press (the body's pitch; a look down is
    // negative, and the stand-in's body is a few inputs behind the bot's).
    let steepest = sent[i.saturating_sub(12)..=(i + 4).min(sent.len() - 1)]
        .iter()
        .map(|s| s.2)
        .fold(0.0_f32, f32::min);
    assert!(steepest <= -0.9, "pitch {steepest}");
    let bot = bots.bots[0].as_ref().unwrap();
    assert_eq!(bot.traps.placed_this_life(), 1);
    assert_eq!(bots.teams[0].knowledge.traps.all().len(), 1);
    // None without the kit, and none with the habit switched off.
    let (sent, _) = run(None, Tuning::default(), 40);
    assert!(sent.iter().all(|s| s.1 & key == 0));
    let mut off = Tuning::default();
    off.traps.enabled = false;
    let (sent, _) = run(Some(kit), off, 40);
    assert!(sent.iter().all(|s| s.1 & key == 0));
}

/// A world's objects section with one placed trap in it (the record
/// `survarium::v026g::match_msgs::traps` reads) among other bytes.
pub(super) fn objects_with(at: Vec3, state: u8, placed_ms: u32) -> Vec<u8> {
    let mut b = vec![0u8; 12];
    b.push(state);
    b.extend_from_slice(&u32::MAX.to_le_bytes());
    let rows: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, at.x, at.y, at.z, 1.0,
    ];
    for f in rows {
        b.extend_from_slice(&f.to_le_bytes());
    }
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&placed_ms.to_le_bytes());
    b.extend_from_slice(&[0u8; 9]);
    b
}

/// The trap a bot promised is confirmed by the world that shows it, its
/// exact place replaces the bot's, the team's field shuts the floors round
/// it, and a world without it takes it away.
#[test]
pub(crate) fn a_world_confirms_the_promise_and_a_world_without_it_takes_it_away() {
    use crate::v026g::bots::trap_know::Phase;
    let kit = TrapKit {
        count: 2,
        key: key::QUICK[1],
    };
    let (_, mut bots) = run(Some(kit), Tuning::default(), 40);
    let promised = bots.teams[0].knowledge.traps.all()[0];
    let Phase::Promised { released } = promised.phase else {
        panic!("{promised:?}");
    };
    // A world a second later shows it a hand's breadth off.
    let exact = promised.at + Vec3::new(0.1, 0.0, 0.05);
    let shown = bots.traps_from_world(&objects_with(exact, 1, released + 200), 41_000);
    assert_eq!(shown, 1);
    let trap = bots.teams[0].knowledge.traps.all()[0];
    assert_eq!((trap.phase, trap.at), (Phase::Armed, exact));
    assert_eq!(bots.teams[0].knowledge.traps.confirmed, 1);
    assert!(!bots.teams[0].trap_field.is_empty());
    // The other team's list stays empty: it is not their trap.
    assert!(bots.teams[1].knowledge.traps.all().is_empty());
    // Fired: spent, the floors are open again.
    bots.traps_from_world(&objects_with(exact, 2, released + 200), 45_000);
    assert!(bots.teams[0].trap_field.is_empty());
    // A world without it: gone.
    bots.traps_from_world(&[0u8; 20], 60_000);
    assert!(bots.teams[0].knowledge.traps.all().is_empty());
}

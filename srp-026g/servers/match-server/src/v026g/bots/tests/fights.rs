use super::*;

/// Match 0x6da44bbb at 662-677 s: two bots of the two teams 0.8-1 m apart,
/// each facing away from the other, knew each other only as heard (no
/// target, no shot for 15 s). Here two bots stand 1 m apart back to back
/// on the flat floor; with `SenseTuning::touch_m` each takes the other as
/// its target within a think and the notice of the first world that shows
/// them; with it off neither does in 2.5 s (the stand-in's bodies do not
/// block each other, so they are not stuck as there).
#[test]
pub(crate) fn back_to_back_at_arms_length_each_takes_the_other_as_target() {
    let run = |touch_m: f32| -> [Option<u32>; 2] {
        let world = soak_world();
        let mut tuning = Tuning::default();
        tuning.sense.touch_m = touch_m;
        tuning.sense.step_m = 0.0;
        let teams = [0u8, 1];
        let mut bots = Bots::new(
            0x6DA4,
            survarium::v026g::maps::mode::TEAM_DEATHMATCH,
            0,
            &teams,
            &[true, true],
            &[None, None],
            Arc::new(tuning),
            Trace::none(0),
        );
        bots.map_ready(world.clone());
        let mut game = Game::new(world.clone(), &teams, 5, Some(30), None);
        // Back to back: 0 faces +z, 1 a metre behind it faces -z.
        for (place, at, yaw) in [
            (0, Vec3::new(14.0, 0.0, 12.0), 0.0),
            (1, Vec3::new(14.0, 0.0, 11.0), std::f32::consts::PI),
        ] {
            game.bodies[place].node = world.grid.locate(at, 1.0);
            game.bodies[place].feet = at;
            game.bodies[place].yaw = yaw;
        }
        bots.entered(0, true, 0);
        bots.entered(1, true, 0);
        let mut taken = [None, None];
        let (mut clock, mut next_world) = (0, 500);
        while clock < 2_500 {
            clock += 25;
            while game.tick + TICK_MS <= clock {
                game.step();
            }
            // A world every half second, 300 ms behind.
            if clock >= next_world {
                next_world += 500;
                if let Some((_, fixes, health, facts)) =
                    game.world_at(clock / TICK_MS * TICK_MS - 300)
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
            for (place, other) in [(0, 1), (1, 0)] {
                let target = bots.bots[place].as_ref().and_then(|b| b.reflex.target());
                if taken[place].is_none() && target == Some(other) {
                    taken[place] = Some(clock);
                }
            }
        }
        taken
    };
    let (with, without) = (run(2.0), run(0.0));
    eprintln!("touch 2 m: targets at {with:?}; off: {without:?}");
    // The first world shows them at 500 ms: a think and the notice later.
    assert!(with.iter().all(|t| t.is_some_and(|t| t <= 1_200)), "{with:?}");
    assert_eq!(without, [None, None], "off: back to back unseen");
}

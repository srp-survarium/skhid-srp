//! The stand-in's enemy block (`Game::body_block_m`, `SRP_BENCH_BODY_BLOCK_M`): enemy bodies stop at the two radii, friends and the
//! default pass through.

use super::*;

/// Two bodies of `teams` walk at each other along z on the flat floor for 3 s; the distance between them at the end, metres.
fn closest_after_a_head_on_walk(teams: [u8; 2], block: Option<f32>) -> f32 {
    let world = soak_world();
    let mut game = Game::new(world.clone(), &teams, 5, Some(30), None);
    game.body_block_m = block;
    for (place, at, yaw) in [(0, Vec3::new(14.0, 0.0, 10.0), 0.0), (1, Vec3::new(14.0, 0.0, 14.0), std::f32::consts::PI)] {
        game.bodies[place].node = world.grid.locate(at, 1.0);
        game.bodies[place].feet = at;
        game.bodies[place].yaw = yaw;
    }
    let gun = Arms::fallback().primary.0;
    let ticks = 3_000 / TICK_MS;
    let start = game.tick;
    for k in 1..=ticks {
        for place in 0..2 {
            game.inputs[place].push_back((start + k * TICK_MS, key::FORWARD, (0.0, 0.0), None, gun));
        }
    }
    let mut closest = f32::MAX;
    for _ in 0..ticks {
        game.step();
        closest = closest.min(game.bodies[0].feet.distance_xz(game.bodies[1].feet));
    }
    closest
}

#[test]
fn enemy_bodies_stop_at_the_two_radii_and_friends_and_the_default_pass_through() {
    let blocked = closest_after_a_head_on_walk([0, 1], Some(0.76));
    assert!((0.7..0.9).contains(&blocked), "{blocked}");
    let off = closest_after_a_head_on_walk([0, 1], None);
    assert!(off < 0.4, "{off}");
    let friends = closest_after_a_head_on_walk([0, 0], Some(0.76));
    assert!(friends < 0.4, "{friends}");
}

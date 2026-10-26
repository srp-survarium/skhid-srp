//! The bots' eye is where the game's bullet starts ([`view::eye_height`]): its heights by stance,
//! pitch and pace; the users that take it (the stand ranges, a head in a sight test); and the
//! stand-in, which shoots from the same eye and from the position the world shows the body at.

use mapdata::{Triangle, WalkerBehaviour};

use super::view::{PlaceView, eye_height, still_eye};
use super::*;

/// A wall across x = `x` from z = 0 to 40, `height` m high.
fn low_wall(x: f32, height: f32) -> [Triangle; 2] {
    let walker = WalkerBehaviour::BySlope;
    let (a, b) = (Vec3::new(x, 0.0, 0.0), Vec3::new(x, 0.0, 40.0));
    let (c, d) = (Vec3::new(x, height, 40.0), Vec3::new(x, height, 0.0));
    [
        Triangle {
            v: [a, b, c],
            walker,
        },
        Triangle {
            v: [a, c, d],
            walker,
        },
    ]
}

fn place(feet: Vec3, keys: u32, velocity: [f32; 2], pitch: f32) -> PlaceView {
    PlaceView {
        place: 0,
        team: 0,
        bot: true,
        present: true,
        alive: true,
        feet,
        yaw: 0.0,
        pitch,
        velocity,
        keys,
        health: 1.0,
        spawned: 0,
        kills: 0,
        deaths: 0,
        streak: 0,
        carrying: None,
        seen_at: 0,
        known_at: 0,
    }
}

/// The heights of the live bullets (23 recorded matches, 2,591 bullets over the shooter's feet,
/// `bullets_height.py`) by stance and pitch, against the function: standing 1.470 level and
/// 1.418 / 1.503 at a quarter radian down / up, crouched 1.038 still and 1.146 slow.
#[test]
pub(crate) fn the_eye_height_is_the_live_bullets_by_stance_pitch_and_pace() {
    let rows: [(&str, bool, f32, f32, f32, f32); 7] = [
        // name, crouched, speed, pitch, live median, tolerance
        ("standing, level", false, 3.0, 0.0, 1.470, 0.012),
        ("standing, still", false, 0.0, 0.0, 1.475, 0.017),
        ("standing, 0.27 down", false, 3.0, -0.27, 1.418, 0.012),
        ("standing, 0.27 up", false, 3.0, 0.27, 1.503, 0.012),
        ("crouched, still", true, 0.0, 0.0, 1.038, 0.012),
        ("crouched, slow", true, 1.0, 0.0, 1.146, 0.012),
        ("crouched, walking", true, 3.0, 0.0, 1.186, 0.05),
    ];
    for (name, crouched, speed, pitch, live, tolerance) in rows {
        let got = eye_height(crouched, speed, pitch);
        assert!(
            (got - live).abs() <= tolerance,
            "{name}: {got} against {live}"
        );
    }
    // Up is higher, in both stances; standing is higher than crouched.
    for crouched in [false, true] {
        assert!(eye_height(crouched, 2.0, 0.5) > eye_height(crouched, 2.0, 0.0));
        assert!(eye_height(crouched, 2.0, -0.5) < eye_height(crouched, 2.0, 0.0));
    }
    assert!(eye_height(false, 0.0, 0.0) > eye_height(true, 2.0, 0.0) + 0.3);
}

/// A crouched body at rest has its head lower than one crouch-walking (1.04 against 1.14 m), and
/// not the old 0.95; the standing one is 1.46 m, not the old 1.6.
#[test]
pub(crate) fn a_crouched_body_still_has_its_head_lower_than_one_walking() {
    let still = eye_height(true, 0.0, 0.0);
    let walking = eye_height(true, 1.5, 0.0);
    assert!(
        (still - 1.04).abs() < 1e-4 && (walking - 1.14).abs() < 1e-4,
        "{still} {walking}"
    );
    // The class edge: under 0.1 m/s still.
    assert_eq!(eye_height(true, 0.09, 0.3), eye_height(true, 0.0, 0.3));
    assert_eq!(eye_height(true, 0.1, 0.3), eye_height(true, 5.0, 0.3));
    assert!((eye_height(false, 0.0, 0.0) - 1.46).abs() < 1e-4);
    // The standing eye does not depend on the pace.
    assert_eq!(eye_height(false, 0.0, 0.2), eye_height(false, 4.0, 0.2));
}

/// A body's eye is its feet and the function of its pose: the keys' crouch, its ground speed and
/// the view's pitch; a place the bot knows only by its track is the standing (or crouched) one at
/// rest, looking level.
#[test]
pub(crate) fn a_places_eye_follows_its_pose() {
    let feet = Vec3::new(3.0, 2.0, -1.0);
    let cases = [
        (0, [0.0, 0.0], 0.0),
        (0, [3.0, 4.0], 0.4),
        (key::CROUCH, [0.0, 0.0], -0.3),
        (key::CROUCH, [0.6, 0.8], 0.3),
    ];
    for (keys, velocity, pitch) in cases {
        let body = place(feet, keys, velocity, pitch);
        let speed = velocity[0].hypot(velocity[1]);
        let want = eye_height(keys & key::CROUCH != 0, speed, pitch);
        assert_eq!(
            body.eye(),
            feet + Vec3::new(0.0, want, 0.0),
            "{keys} {velocity:?} {pitch}"
        );
    }
    assert_eq!(still_eye(feet, false), feet + Vec3::new(0.0, 1.46, 0.0));
    assert_eq!(still_eye(feet, true), feet + Vec3::new(0.0, 1.04, 0.0));
}

/// The stand finder tests a use ray at two heights a pose: they hold the eye of every pose the
/// bots have, the pitch within a quarter radian (a use is near level).
#[test]
pub(crate) fn the_stand_ranges_hold_the_eye_of_every_pose() {
    use mapdata::stands::{EYE_CROUCHED, EYE_STANDING};
    for crouched in [false, true] {
        let range = if crouched { EYE_CROUCHED } else { EYE_STANDING };
        for speed in [0.0, 0.05, 0.5, 3.0] {
            for k in -5..=5 {
                let pitch = 0.05 * k as f32;
                let h = eye_height(crouched, speed, pitch);
                assert!(
                    (range[0]..=range[1]).contains(&h),
                    "crouched {crouched} speed {speed} pitch {pitch}: {h} outside {range:?}"
                );
            }
        }
    }
}

/// A wall 1.5 m high hides a body from a standing one across it: the game's bullet leaves 1.46 m
/// up, so the line level with both heads meets the wall (the 1.6 m of before passed over it, and
/// no spot was found behind a wall of this height).
#[test]
pub(crate) fn a_spot_behind_a_wall_of_1_5_m_is_hidden_from_an_enemys_head() {
    let world = super::tactics::tests::field_with_solids(&low_wall(20.0, 1.5), |_| {});
    let tactics = super::tactics::Tactics::new(world, 1.0);
    let target = Vec3::new(30.0, 0.0, 10.0);
    let from = Vec3::new(5.0, 0.0, 20.0);
    let query = super::tactics::SpotQuery {
        sight_to: Some(target),
        hidden_from: Some(from),
        samples: 200,
        ..super::tactics::SpotQuery::ring(target, 6.0, 20.0)
    };
    let spots = tactics.spots(&query, 0, 5, &mut Rng::new(5));
    assert!(!spots.is_empty(), "spots behind the wall");
    for spot in &spots {
        assert!(spot.at.x > 20.0, "{spot:?}");
        assert!(!tactics.sees(still_eye(from, false), still_eye(spot.at, false)));
    }
}

/// A shot leaves from where the world of its tick shows the shooter, before that tick's move (PROVEN
/// live: the bullet starts at the world's position of its own tick), at the game's eye height for his
/// pace and view; the move comes after.
#[test]
pub(crate) fn a_stand_in_shot_leaves_from_the_position_before_its_ticks_move() {
    let world = soak_world();
    let mut game = Game::new(world.clone(), &[0, 0], 1, None, None);
    let gun = Arms::fallback().primary.0;
    let start = Vec3::new(20.0, 0.0, 10.0);
    // A teammate dead ahead, 14 m: a shot at him is a team hit, which keeps the shot's eye.
    for (place, feet) in [(0, start), (1, start + Vec3::new(0.0, 0.0, 14.0))] {
        let body = &mut game.bodies[place];
        body.node = world.grid.locate(feet, 1.0);
        body.feet = feet;
        body.yaw = 0.0;
        body.alive = true;
    }
    // Walking for 1.5 s, then walking and firing.
    for tick in (TICK_MS..=1_500).step_by(TICK_MS as usize) {
        game.inputs[0].push_back((tick, key::FORWARD, (0.0, 0.0), None, gun));
        game.inputs[1].push_back((tick, 0, (0.0, 0.0), None, gun));
    }
    while game.tick < 1_500 {
        game.step();
    }
    assert!(
        game.bodies[0].step_m > 0.05,
        "walking: {}",
        game.bodies[0].step_m
    );
    let tick = game.tick + TICK_MS;
    game.inputs[0].push_back((tick, key::FORWARD | key::FIRE, (0.0, 0.0), None, gun));
    game.inputs[1].push_back((tick, 0, (0.0, 0.0), None, gun));
    let before = game.bodies[0].feet;
    let (pace, view) = (
        game.bodies[0].step_m / TICK_S,
        game.bodies[0].recoil.view(0.0),
    );
    game.step();
    let after = game.bodies[0].feet;
    assert!(
        after.distance_xz(before) > 0.05,
        "the body moved in the tick"
    );
    let &(shooter, victim, eye, _, _, _) = game.team_hit_shots.first().expect("a team hit at 14 m");
    assert_eq!((shooter, victim), (0, 1));
    let want = before + Vec3::new(0.0, eye_height(false, pace, view), 0.0);
    assert!(
        (eye - want).length() < 1e-4,
        "the shot left from {eye:?}, before the move {want:?}, after {after:?}"
    );
}

/// The stand-in's eye is the bots' function at its pace and view.
#[test]
pub(crate) fn the_stand_ins_eye_is_the_bots_eye_function() {
    let world = soak_world();
    let mut game = Game::new(world, &[0], 1, None, None);
    let body = &mut game.bodies[0];
    body.feet = Vec3::new(1.0, 5.0, 2.0);
    body.pitch = 0.4;
    for (keys, step_m) in [(0, 0.0), (0, 0.2), (key::CROUCH, 0.0), (key::CROUCH, 0.1)] {
        body.keys = keys;
        body.step_m = step_m;
        let want = eye_height(
            keys & key::CROUCH != 0,
            step_m / TICK_S,
            body.recoil.view(0.4),
        );
        assert_eq!(Game::eye(body), body.feet + Vec3::new(0.0, want, 0.0));
    }
}

/// Two runs of one seed give the same match, with the eye reading the pace, the pitch and the
/// keys: the same shots, hits, kills and the same shots into a wall or at a hidden target.
#[test]
pub(crate) fn a_soak_with_the_games_eye_is_the_same_twice() {
    let run = || {
        let out = soak(4, 3, 0, 40, 7);
        (
            out.shots,
            out.hits,
            out.kills,
            out.walled,
            out.hidden,
            out.delivered,
        )
    };
    let (a, b) = (run(), run());
    assert_eq!(a, b);
    assert!(a.0 > 0, "shots fired: {a:?}");
}

/// The off switch of this thread's eye, put back on when the test ends.
struct EyeOff;

impl EyeOff {
    fn new() -> Self {
        super::view::set_live_eye(false);
        Self
    }
}

impl Drop for EyeOff {
    fn drop(&mut self) {
        super::view::set_live_eye(true);
    }
}

/// `CombatTuning::live_eye` off is the old eye: 1.6 m standing and 0.95 m crouched, whatever the
/// pace and the pitch.
#[test]
pub(crate) fn with_the_switch_off_the_eye_is_the_old_constants() {
    let _off = EyeOff::new();
    for (speed, pitch) in [(0.0, 0.0), (3.0, 0.5), (0.0, -0.5)] {
        assert_eq!(eye_height(false, speed, pitch), 1.6);
        assert_eq!(eye_height(true, speed, pitch), 0.95);
    }
    assert_eq!(still_eye(Vec3::ZERO, false).y, 1.6);
}

/// A bot set built with `live_eye` off reads the eye its tuning says at each call, and the default
/// is on.
#[test]
pub(crate) fn the_bots_tuning_sets_the_eye_of_its_thread() {
    assert!(Tuning::default().aim.combat.live_eye);
    for on in [false, true, false] {
        let mut tuning = Tuning::default();
        tuning.aim.combat.live_eye = on;
        let mut bots = Bots::new(
            1,
            survarium::v026g::maps::mode::TEAM_DEATHMATCH,
            0,
            &[0, 1],
            &[true, true],
            &[None, None],
            Arc::new(tuning),
            Trace::none(0),
        );
        bots.poll(Clock {
            clock: 100,
            time_left_s: 100.0,
            total_s: 100.0,
            playing: true,
        });
        assert_eq!(super::view::live_eye(), on);
    }
    super::view::set_live_eye(true);
}

/// The tuning the pin below was taken with: the bench's defaults with every behaviour switch that is newer
/// than the pin (and ON by default) turned OFF. **The single list: a behaviour PR whose default is ON adds
/// ONE line here and never touches the test; a switch merged OFF needs none** (the flip PR adds it). Keep it
/// sorted by the switch's path, each line its own block with the PR named, so two PRs add at different places
/// and git merges them without a conflict.
///
/// To add yours, one block in its sorted place (by `section.field`), the switch at its OFF value:
///
/// ```text
/// // <PR>: <what the pinned counts predate>.
/// tuning.<section>.<switch> = <the value that is the old behaviour>;
/// ```
fn pre_eye_pin_tuning() -> Tuning {
    let mut tuning = bench_tuning();

    // #534: the eye is where the game's bullet starts.
    tuning.aim.combat.live_eye = false;

    // X1 (#552): the trigger's range by weapon class; the pin has the old 80 m.
    tuning.aim.combat.range_by_weapon = false;

    // K1: the threat spots; the pinned counts predate them.
    tuning.threat.spots = false;

    tuning
}

/// With the switch off (`SRP_BENCH_COMBAT=live_eye=false`) the soak is the base's to the shot: the
/// counts of seeds 3, 7 and 12 of the 120 s 4 against 4 soak at 2174f9e6, before the fix (shots, hits,
/// kills of each team, shots into a wall, at a hidden target, batteries delivered, sight and aim rays
/// a bot second). The old heights, the old stand ranges, the shots after the move and a look every 100 ms
/// alone.
#[test]
pub(crate) fn with_the_switch_off_a_soak_is_the_old_one() {
    let _off = EyeOff::new();
    let base = [
        (3, (365, 69, [16, 14], 2, 0, [1, 1]), 48.04),
        (7, (505, 82, [14, 13], 2, 0, [1, 2]), 49.71),
        (12, (769, 126, [17, 20], 2, 2, [1, 1]), 44.16),
    ];
    for (seed, want, rays) in base {
        let out = soak_tuned(4, 3, 0, None, 120, seed, pre_eye_pin_tuning());
        let got = (
            out.shots,
            out.hits,
            out.kills,
            out.walled,
            out.hidden,
            out.delivered,
        );
        assert_eq!(got, want, "seed {seed}");
        assert!(
            (out.rays_per_bot_s - rays).abs() < 0.01,
            "seed {seed}: {}",
            out.rays_per_bot_s
        );
    }
}

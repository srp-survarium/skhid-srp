use super::*;

/// The stand-in camper (`SRP_BENCH_CAMPER=<place>`, `campers.md` K1 §4):
/// that place's bot goes to [`camper_spot`] and holds it all match,
/// standing, back to it after each respawn; its reflex fires on sight. Its
/// spot: within this of it, m; the route points it is chosen by: kept this
/// far apart, m, and seen within this, m.
pub(crate) const CAMPER_WITHIN_M: f32 = 0.7;

/// The camper's spot and how many route points it sees, if any.
pub(crate) type CamperSpot = Option<(Vec3, u32)>;

pub(crate) const CAMPER_THIN_M: f32 = 2.0;

pub(crate) const CAMPER_SEES_M: f32 = 50.0;

/// The stand-in camper's spot: of `side`'s human route points, the one
/// whose standing eye sees the most of the other side's route points
/// within [`CAMPER_SEES_M`] (any body part of a standing body there, the
/// bots' own sight test); ties to the higher. Worked out once a map, from
/// the map's routes only (no spot table), with the count it sees; none
/// when it sees none.
pub(crate) fn camper_spot(tactics: &super::tactics::Tactics, side: i32) -> CamperSpot {
    let points = |side: i32| {
        let mut kept: Vec<Vec3> = Vec::new();
        for p in tactics.routes_of(side).flat_map(|r| r.points.iter().copied()) {
            if p.is_finite() && !kept.iter().any(|k| k.distance_xz(p) < CAMPER_THIN_M && (k.y - p.y).abs() < 1.0) {
                kept.push(p);
            }
        }
        kept
    };
    let (mine, theirs) = (points(side), points(1 - side));
    let sight = &tactics.world.sight;
    mine.iter()
        .map(|&p| {
            let eye = p + Vec3::new(0.0, 1.6, 0.0);
            let seen = theirs
                .iter()
                .filter(|&&q| {
                    q.distance_xz(p) <= CAMPER_SEES_M && super::aimpoint::shows(eye, q, false, sight, || true)
                })
                .count() as u32;
            (p, seen)
        })
        .filter(|&(_, seen)| seen > 0)
        .max_by(|a, b| a.1.cmp(&b.1).then(a.0.y.total_cmp(&b.0.y)))
}

/// Human routes on the soak field: side 0's north lane past the wall's
/// end and its south lane behind the wall; side 1's the same at x 60-70.
pub(crate) fn camper_routes() -> Vec<super::routes::HumanRoute> {
    let route = |team: i32, points: &[(f32, f32)]| super::routes::HumanRoute {
        team,
        lives: 10,
        points: points.iter().map(|&(x, z)| Vec3::new(x, 0.0, z)).collect(),
        ..Default::default()
    };
    vec![
        route(0, &[(10.0, 33.0), (35.0, 33.0)]),
        route(0, &[(20.0, 8.0), (30.0, 5.0)]),
        route(1, &[(60.0, 33.0), (70.0, 33.0)]),
        route(1, &[(60.0, 5.0), (70.0, 5.0)]),
    ]
}

/// The camper's spot is the route point that sees the most of the other
/// side's: on the north lane at the wall's end, not behind the wall (which
/// sees none of them), nor farther back.
#[test]
pub(crate) fn the_stand_in_campers_spot_sees_most_of_the_other_sides_routes() {
    let mut tactics = super::tactics::Tactics::new(super::tactics::tests::field(), 1.0);
    tactics.set_routes(camper_routes());
    let (at, seen) = camper_spot(&tactics, 0).expect("a spot");
    assert!(at.distance_xz(Vec3::new(35.0, 0.0, 33.0)) < 0.5, "{at:?} {seen}");
    assert_eq!(seen, 4);
    // Behind the wall alone: no spot.
    let mut walled = super::tactics::Tactics::new(super::tactics::tests::field(), 1.0);
    walled.set_routes(camper_routes().into_iter().filter(|r| r.team == 1 || r.points[0].z < 20.0).collect());
    assert_eq!(camper_spot(&walled, 0), None);
}

/// In a Team Deathmatch on the soak field, the stand-in camper (bot 0)
/// goes to its spot and holds it; its teammate (the near miss) roams.
#[test]
pub(crate) fn the_stand_in_camper_holds_its_spot_and_a_teammate_does_not() {
    let world = soak_world();
    let teams = [0u8, 0, 1, 1];
    let mut bots = Bots::new(
        0x50AC,
        survarium::v026g::maps::mode::TEAM_DEATHMATCH,
        0,
        &teams,
        &[true; 4],
        &[None; 4],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    bots.map_ready(world.clone());
    if let Some(tactics) = bots.tactics.as_mut() {
        tactics.set_routes(camper_routes());
    }
    bots.bench_camper = Some((0, None));
    let mut game = Game::new(world, &teams, 1, Some(1_000), None);
    for place in 0..teams.len() {
        bots.entered(place, true, 0);
    }
    let spot = Vec3::new(35.0, 0.0, 33.0);
    // By bot: in this life at the spot (within 1 m) yet; living ticks
    // since, and of them those within 2 m of it; living ticks in all.
    let mut there = [false; 2];
    let mut held = [(0u32, 0u32, 0u32); 2];
    let (limit_ms, mut clock, mut next_world) = (120_000, 0, 1_000);
    while clock < limit_ms {
        clock += 25;
        while game.tick + TICK_MS <= clock {
            game.step();
            for (place, held) in held.iter_mut().enumerate() {
                let body = &game.bodies[place];
                let off = body.feet.distance_xz(spot);
                there[place] = body.alive && (there[place] || off <= 1.0);
                held.2 += u32::from(body.alive);
                if there[place] {
                    held.0 += 1;
                    held.1 += u32::from(off <= 2.0);
                }
            }
        }
        if clock >= next_world {
            next_world += 1_000;
            if let Some((_, fixes, health, facts)) = game.world_at(clock / TICK_MS * TICK_MS - 300) {
                for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                    bots.fixed(place, fix, Some(health));
                }
                bots.facts(facts);
            }
        }
        let (made, _) = bots.poll(Clock {
            clock,
            time_left_s: (limit_ms - clock) as f32 / 1000.0,
            total_s: (limit_ms / 1000) as f32,
            playing: true,
        });
        for made in made {
            let r = made.input.rotation;
            let turn = (
                half_value(u16::from_le_bytes([r[0], r[1]])),
                half_value(u16::from_le_bytes([r[2], r[3]])),
            );
            let bot = bots.bots[made.place].as_ref();
            let target = bot.and_then(|bot| bot.reflex.target());
            let gun = bot.map_or(Arms::fallback().primary.0, |bot| bot.reflex.held_gun());
            game.inputs[made.place].push_back((made.time, made.input.actions, turn, target, gun));
        }
    }
    let found = bots.bench_camper.and_then(|(_, spot)| spot.flatten());
    assert!(found.is_some_and(|(at, _)| at.distance_xz(spot) < 0.5), "{found:?}");
    // The camper got there and held it while alive (the reflex never moves
    // it off); its mate spent little of its life there.
    let [camper, mate] = held;
    // Its lives end by kills, so the ticks it spends at the spot move with
    // any change to the other bodies of the game (a stuck recovery among
    // them: 117 and 67 in two runs of this soak that differ in one back-off's
    // keys). The floor is "got there for a few seconds"; the property is
    // that it holds the spot all the while it is there.
    assert!(camper.0 >= 50 && camper.1 == camper.0, "camper {camper:?}, mate {mate:?}");
    assert!(f64::from(mate.1) <= 0.2 * f64::from(mate.2), "camper {camper:?}, mate {mate:?}");
}

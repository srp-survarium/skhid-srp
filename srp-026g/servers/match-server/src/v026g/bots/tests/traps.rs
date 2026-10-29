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
    run_on(kit, tuning, seconds, |_| armed)
}

/// [`run_with`], the trap's place asked of the bots at each world (a trap
/// put on the bot's own route is found once it has one).
pub(super) fn run_on(
    kit: Option<TrapKit>,
    tuning: Tuning,
    seconds: u32,
    mut armed: impl FnMut(&Bots) -> Option<Vec3>,
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
        bots.bots[0].as_mut().unwrap().features.traps.habit = Habit::carry(kit);
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
            if let Some(at) = armed(&bots) {
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
    let (sent, bots) = run(Some(kit), Tuning::all_off(), 40);
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
    assert_eq!(bot.features.traps.habit.placed_this_life(), 1);
    assert_eq!(bots.place_counts().placed, 1);
    assert_eq!(bots.place_counts().died_placing, 0);
    assert!(bots.react_counts().line().starts_with("trap react: rolls "));
    assert_eq!(bots.teams[0].knowledge.traps.all().len(), 1);
    // None without the kit, and none with the habit switched off.
    let (sent, _) = run(None, Tuning::all_off(), 40);
    assert!(sent.iter().all(|s| s.1 & key == 0));
    let mut off = Tuning::all_off();
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
    let (_, mut bots) = run(Some(kit), Tuning::all_off(), 40);
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
    assert!(!bots.teams[0].features.traps.field.is_empty());
    // The other team's list stays empty: it is not their trap.
    assert!(bots.teams[1].knowledge.traps.all().is_empty());
    // Fired: spent, the floors are open again; the team's trap went off,
    // and nobody with a broken leg is near it.
    bots.traps_from_world(&objects_with(exact, 2, released + 200), 45_000);
    assert!(bots.teams[0].features.traps.field.is_empty());
    assert_eq!(bots.own_trap_counts().fired, 1);
    assert_eq!(bots.own_trap_counts().own_team_caught, 0);
    // A world without it: gone.
    bots.traps_from_world(&[0u8; 20], 60_000);
    assert!(bots.teams[0].knowledge.traps.all().is_empty());
}

/// A trap of the team that goes off with a broken-legged body of the same
/// team near it is an own-team catch; with the legs whole, or the body far,
/// it is not.
#[test]
pub(crate) fn an_own_trap_that_goes_off_beside_a_broken_legged_mate_is_an_own_team_catch() {
    let (_, mut bots) = run(None, Tuning::all_off(), 4);
    let clock = Clock {
        clock: 5_000,
        time_left_s: 0.0,
        total_s: 0.0,
        playing: true,
    };
    let world = bots.tactics.as_ref().map(|t| t.world.clone());
    let view = bots.tracker.view(&clock, &bots.match_wide_tuning().kin, world.as_deref());
    let at = view.place(0).unwrap().feet + Vec3::new(1.0, 0.0, 0.0);
    // Legs whole: a trap went off, nobody caught.
    bots.count_own_traps(5_000, &[(0, at)]);
    assert_eq!((bots.own_trap_counts().fired, bots.own_trap_counts().own_team_caught), (1, 0));
    // A broken leg beside it: caught. The other team's trap is not ours.
    bots.bots[0].as_mut().unwrap().hurt.leg_broken = true;
    bots.count_own_traps(5_000, &[(0, at)]);
    assert_eq!((bots.own_trap_counts().fired, bots.own_trap_counts().own_team_caught), (2, 1));
    bots.count_own_traps(5_000, &[(1, at)]);
    assert_eq!((bots.own_trap_counts().fired, bots.own_trap_counts().own_team_caught), (3, 1));
    // Far from it: not caught.
    bots.count_own_traps(5_000, &[(0, at + Vec3::new(30.0, 0.0, 0.0))]);
    assert_eq!((bots.own_trap_counts().fired, bots.own_trap_counts().own_team_caught), (4, 1));
}

/// A trap lands where the head ray of the release frame meets the floor, not
/// where the habit planned it: the bot goes on walking while it looks down.
#[test]
pub(crate) fn a_trap_lands_where_the_head_ray_meets_the_floor() {
    use crate::v026g::bots::traps::{PLACE_RAY_M, landing};
    let eye = Vec3::new(10.0, 1.6, 4.0);
    let yaw = crate::v026g::bots::kin::yaw_of([1.0, 0.0]);
    // At the press pitch the ray meets the floor 1.12 m out along the yaw.
    let at = landing(eye, yaw, -0.96, 0.0).expect("lands");
    assert!((at.x - (eye.x + 1.6 / 0.96_f32.tan())).abs() < 1e-3, "{at:?}");
    assert!((at.z - eye.z).abs() < 1e-3 && at.y == 0.0);
    assert!((at - Vec3::new(eye.x, 0.0, eye.z)).length() < PLACE_RAY_M);
    // Walking on 1.8 m before the release moves it 1.8 m: the planned spot
    // (1 m from where the bot stood) is no longer the place.
    let walked = landing(eye + Vec3::new(1.8, 0.0, 0.0), yaw, -0.96, 0.0).unwrap();
    assert!((walked.x - at.x - 1.8).abs() < 1e-3);
    // Too shallow a look: the ray ends in the air (the pitch of 1.6 / sin
    // = 2.0 m is the limit), a look up or level, a floor above the eye.
    assert!(landing(eye, yaw, -0.80, 0.0).is_none());
    assert!(landing(eye, yaw, 0.2, 0.0).is_none());
    assert!(landing(eye, yaw, 0.0, 0.0).is_none());
    assert!(landing(eye, yaw, -1.0, 1.7).is_none());
    assert!(landing(eye, yaw, -1.0, 0.0).is_some());
}

/// A bot is told its traps at its spawn from its kit (`Bot::arm`): the flankers and rushers carry the stack of the
/// third pocket with the quick-use key of rank 3, the others none; with the owner's kit off (`SRP_BOT_KIT`, the
/// default) the old pockets hold none and no habit is armed. Run it with `SRP_BOT_KIT=1` to see the kit on.
#[test]
pub(crate) fn a_bots_habit_is_armed_from_the_traps_in_its_kit() {
    use survarium::v026g::loadouts::{BotEquipment, BotRole, BotTier, RusherKits, new_kit};
    let roles = [Some("flanker"), Some("rusher"), Some("support"), Some("camper")];
    let equipment = BotEquipment::of(BotTier::Mid, RusherKits::default());
    let bots = Bots::new(
        0x41C,
        survarium::v026g::maps::mode::BATTERIES,
        0,
        &[0, 0, 1, 1],
        &[true; 4],
        &roles,
        Arc::new(Tuning::all_off()),
        Trace::none(0),
    )
    .with_tiers(&[Some(equipment); 4]);
    for (place, role) in roles.iter().enumerate() {
        let role = BotRole::ALL.into_iter().find(|each| Some(each.name()) == *role).unwrap();
        let kit = bots.bots[place].as_ref().unwrap().features.traps.habit.kit();
        let wanted = (new_kit() && role.carries_traps()).then_some(TrapKit { count: 2, key: key::QUICK[2] });
        assert_eq!(kit, wanted, "{role:?} (owner's kit on: {})", new_kit());
    }
}

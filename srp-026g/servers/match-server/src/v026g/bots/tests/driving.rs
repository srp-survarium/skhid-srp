use super::*;

/// The side step weaves a walk to or from the target into a diagonal; a
/// walk to a side already, or none, is left as it is.
#[test]
pub(crate) fn the_side_step_weaves_a_walk_along_the_view() {
    assert_eq!(weave(key::FORWARD, key::LEFT), Some(key::FORWARD | key::LEFT));
    assert_eq!(weave(key::BACK, key::RIGHT), Some(key::BACK | key::RIGHT));
    assert_eq!(
        weave(key::FORWARD | key::CROUCH | key::JUMP, key::RIGHT),
        Some(key::FORWARD | key::CROUCH | key::JUMP | key::RIGHT)
    );
    assert_eq!(weave(key::FORWARD | key::LEFT, key::RIGHT), None);
    assert_eq!(weave(key::LEFT, key::RIGHT), None);
    assert_eq!(weave(0, key::LEFT), None);
    assert_eq!(weave(key::FORWARD, 0), None);
}

/// A carrier's line way out yields to its motor's crossing only while the
/// motor walks it: a crossing left with no route (the route dropped by the
/// unstick's back-off, or a search that found none) holds nothing back, so
/// a carrier standing in the zone takes its line out (`collect.rs`).
#[test]
pub(crate) fn a_crossing_with_no_route_left_does_not_hold_a_carriers_way_out() {
    use super::path::{Route, Waypoint};
    let tuning = Tuning::default();
    let mut bot = Bot::new(1, 0, 0, Some("support"), &tuning);
    let home = Vec3::new(10.0, 0.0, 10.0);
    bot.features.crossing.goal = Some(home);
    assert_eq!(walks_crossing(&bot), None);
    let point = |x: f32| Waypoint {
        at: Vec3::new(x, 0.0, 0.0),
        node: 0,
        jump: None,
        low: false,
    };
    bot.route = Some((Route::new(vec![point(0.0), point(5.0)]), home));
    assert_eq!(walks_crossing(&bot), Some(home));
    bot.features.crossing.goal = None;
    assert_eq!(walks_crossing(&bot), None);
}

/// The commander moving a bot's Crossfire to another choke leaves the
/// play's cooldown alone (the new run is still a candidate at the next
/// poll); a switch to another play starts it.
#[test]
pub(crate) fn a_switch_to_another_target_of_the_same_play_keeps_its_cooldown() {
    use super::plays::{PlayKind, Running, crossfire};
    let tuning = Tuning::default();
    let mut bot = Bot::new(1, 0, 0, Some("support"), &tuning);
    bot.last_ran.push((PlayKind::Crossfire, 1_000));
    bot.running = Some(Running::Crossfire(crossfire::State::NoSpot));
    bot.give_way(PlayKind::Crossfire, 60_000);
    assert!(bot.running.is_none());
    assert_eq!(bot.last_ran, vec![(PlayKind::Crossfire, 1_000)]);
    bot.running = Some(Running::Crossfire(crossfire::State::NoSpot));
    bot.give_way(PlayKind::Roam, 61_000);
    assert_eq!(bot.last_ran, vec![(PlayKind::Crossfire, 61_000)]);
    // With nothing running, nothing is stamped.
    bot.give_way(PlayKind::Cut, 62_000);
    assert_eq!(bot.last_ran, vec![(PlayKind::Crossfire, 61_000)]);
}

/// A jump while the trigger is down: held [`JUMP_HOLD_MS`] with the trigger
/// let go (the weapon takes no jump while it fires, 0x2E3A70), then the
/// trigger again; a jump with the trigger not down lately is the one tick
/// it was. Each jump is traced, and what followed it.
#[test]
pub(crate) fn a_jump_lets_the_trigger_go_and_is_traced() {
    let tuning = Tuning::default();
    let at = Vec3::new(1.0, 0.0, 2.0);
    let walk = key::FORWARD | key::FIRE;
    let mut bot = Bot::new(1, 0, 0, Some("support"), &tuning);
    bot.tracing = true;
    assert_eq!(jumping(&mut bot, walk, None, at, 1_000), walk);
    let hold = (JUMP_HOLD_MS / TICK_MS) as usize;
    let mut sent = vec![jumping(&mut bot, walk | key::JUMP, Some("hop"), at, 1_050)];
    for k in 1..hold + 2 {
        sent.push(jumping(&mut bot, walk, None, at, 1_050 + TICK_MS * k as u32));
    }
    let held = key::FORWARD | key::JUMP;
    assert!(sent[..hold].iter().all(|&k| k == held), "{sent:?}");
    assert!(sent[hold..].iter().all(|&k| k == walk), "{sent:?}");
    assert_eq!(bot.traced.len(), 1);
    assert_eq!(bot.traced[0]["jump"], "hop");
    assert_eq!(bot.traced[0]["firing"], true);
    bot.jump_followed(4_050, Some(at + Vec3::new(2.0, 0.5, 0.0)), "went on");
    assert_eq!(bot.traced[1]["jumped"], "hop");
    assert_eq!(bot.traced[1]["then"], "went on");
    assert_eq!(bot.traced[1]["after_ms"], 3_000);
    assert!((bot.traced[1]["moved_m"].as_f64().unwrap() - 2.0).abs() < 1e-6);
    assert!(bot.jump_watch.is_none());

    // Not firing: one tick, as before.
    let mut bot = Bot::new(1, 0, 0, Some("support"), &tuning);
    let walk = key::FORWARD;
    assert_eq!(
        jumping(&mut bot, walk | key::JUMP, Some("route"), at, 1_000),
        walk | key::JUMP
    );
    assert_eq!(jumping(&mut bot, walk, None, at, 1_050), walk);
    // Not tracing: watched, no line.
    assert!(bot.traced.is_empty() && bot.jump_watch.is_some());
}

/// [body physics] A route jump's watch is up when the bot's own reckoned body stands up the link on the ground, though no world (2 s
/// apart on the bench) showed it there: the log's line says "up by the reckoning only". The body over the landing in the air is not
/// up; with neither, the jump is short.
#[test]
pub(crate) fn a_route_jump_up_by_the_reckoning_is_up_in_the_log() {
    use super::motor::{PressState, RoutePress};
    let tuning = Tuning::default();
    let (take_off, landing) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.5, 1.08, 0.0));
    let route = RoutePress {
        take_off,
        landing,
        tries: 1,
        before: 0.1,
        facing: 0.0,
        sprint_gate: None,
        aim: super::path::PressAim::STRAIGHT,
        state: PressState { lead_m: 0.1, flown_lead_m: None, side_m: 0.0, turn_rad: 0.0, above_m: 0.0, along_ms: 0.0, window: None },
    };
    let watched = |bot: &mut Bot| {
        bot.jump_watch = Some(JumpWatch { by: "route", at: 1_000, from: take_off, route: Some(route), up: None, up_reckoned: None, landed: None });
    };
    let mut bot = Bot::new(1, 0, 0, Some("support"), &tuning);
    watched(&mut bot);
    bot.jump_reckoned(1_500, Vec3::new(0.6, 1.3, 0.0), false);
    assert_eq!(bot.jump_watch.and_then(|w| w.up_reckoned), None, "over the landing in the air is not up");
    bot.jump_reckoned(1_900, Vec3::new(0.7, 1.08, 0.0), true);
    bot.jump_reckoned(2_000, Vec3::new(0.9, 1.08, 0.0), true);
    assert_eq!(bot.jump_watch.and_then(|w| w.up_reckoned), Some(1_900), "the first tick up on the ground");
    bot.jump_followed(4_000, Some(Vec3::new(3.0, 0.0, 0.0)), "went on");
    let lines = bot.nav_log.take();
    assert!(lines.iter().any(|(_, l)| l.starts_with("nav: route jump up:") && l.contains("up by the reckoning only, 900 ms after the press")), "{lines:?}");

    let mut bot = Bot::new(1, 0, 0, Some("support"), &tuning);
    watched(&mut bot);
    bot.jump_reckoned(1_900, Vec3::new(0.1, 0.0, 0.0), true);
    bot.jump_followed(4_000, Some(Vec3::new(0.1, 0.0, 0.0)), "went on");
    let lines = bot.nav_log.take();
    assert!(lines.iter().any(|(_, l)| l.starts_with("nav: route jump short:") && !l.contains("by the reckoning")), "{lines:?}");
}

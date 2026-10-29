use super::*;

/// The tuning with the whole sight refresh on (what `SRP_BENCH_SIGHT_REFRESH=1` makes): it is off by default.
pub(crate) fn refreshed_tuning() -> Tuning {
    let mut tuning = Tuning::all_off();
    tuning.sense.sight_refresh = true;
    tuning.cheats.sight_hold_ms = crate::v026g::bots::know::SIGHT_HOLD_MS;
    tuning
}

/// The ray budget of a poll is the batteries' with the refresh (an enemy in view is tested outside it, whole),
/// and is shared by the thinks due only when it binds (`sense::fair_share`): a match whose budget does not bind
/// is decided the same whatever the budget (400 rays a poll or ten times that), shot for shot. Without the
/// refresh the enemies in view take the poll's rays, and a match whose budget does not bind is the same.
#[test]
pub(crate) fn a_budget_that_does_not_bind_changes_no_decision() {
    for refresh in [false, true] {
        let run = |rays_per_poll: u32| {
            let mut tuning = if refresh { refreshed_tuning() } else { Tuning::all_off() };
            tuning.think.rays_per_poll = rays_per_poll;
            let out = soak_tuned(4, 3, 0, None, 60, 4, tuning);
            (out.kills, out.shots, out.hits, out.walled, out.hidden)
        };
        let usual = run(Tuning::all_off().think.rays_per_poll);
        assert_eq!(usual, run(4_000), "refresh {refresh}: {usual:?}");
        // A budget with nothing in it does bind (the batteries are not looked
        // at, and without the refresh nor are the enemies), and it is felt.
        assert_ne!(usual, run(0), "refresh {refresh}: {usual:?}");
    }
}

/// The switch is a switch: off is the old bots (not the default now) and is not the refresh (the bots decide otherwise with it on),
/// and each state is the same match twice.
#[test]
pub(crate) fn the_sight_refresh_off_is_the_old_bots_and_changes_the_soak_when_on() {
    assert!(!Tuning::all_off().sense.sight_refresh);
    assert_eq!(Tuning::all_off().cheats.sight_hold_ms, 0);
    let run = |tuning: Tuning| {
        let out = soak_tuned(4, 3, 0, None, 60, 4, tuning);
        (out.kills, out.shots, out.hits, out.walled, out.hidden)
    };
    let off = run(Tuning::all_off());
    assert_eq!(off, run(Tuning::all_off()));
    let on = run(refreshed_tuning());
    assert_eq!(on, run(refreshed_tuning()));
    assert_ne!(off, on, "{off:?}");
}

/// With the refresh on a short soak has fights, keeps the input rules and does not fire into the world: at
/// the default grace of 250 ms seed 3 gives 2 of 500 shots into a wall or at a hidden target (16 of 460 at a
/// grace of 150 ms; the shots left are the stand-in's stale view, `fact-soak-shots-blocked.md`).
#[test]
pub(crate) fn with_the_sight_refresh_a_short_soak_has_fights_and_keeps_the_input_rules() {
    let out = soak_tuned(4, 3, 0, None, 120, 3, refreshed_tuning());
    assert!(out.kills[0] + out.kills[1] >= 2, "{:?}", out.kills);
    assert!(out.least_moved_m > 20.0, "{}", out.least_moved_m);
    assert!(
        out.walled + out.hidden <= out.shots / 33,
        "{} into a wall, {} at a hidden target, of {} shots",
        out.walled,
        out.hidden,
        out.shots
    );
}

/// The thinks due are counted before any ray is cast: the bots that have
/// entered, are alive, and whose next think has come, once the map is
/// ready.
#[test]
pub(crate) fn the_thinks_due_are_counted_from_the_bots_that_have_entered_and_are_alive() {
    let world = soak_world();
    let teams = [0u8, 0, 1, 1];
    let mut bots = Bots::new(
        0x51647,
        survarium::v026g::maps::mode::TEAM_DEATHMATCH,
        0,
        &teams,
        &[true; 4],
        &[None; 4],
        Arc::new(Tuning::all_off()),
        Trace::none(0),
    );
    let mut view = bots.tracker.view(
        &Clock {
            clock: 5_000,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        },
        &bots.match_wide_tuning().kin,
        Some(&*world),
    );
    for place in &mut view.places {
        place.alive = true;
    }
    for place in 0..4 {
        bots.entered(place, true, 0);
    }
    assert_eq!(bots.thinks_due(&view), 0, "the map is not ready");
    bots.map_ready(world);
    assert_eq!(bots.thinks_due(&view), 4);
    view.places[1].alive = false;
    assert_eq!(bots.thinks_due(&view), 3, "a dead bot does not think");
    bots.bots[2].as_mut().unwrap().next_think = 5_001;
    assert_eq!(bots.thinks_due(&view), 2, "its time has not come");
    bots.bots[3].as_mut().unwrap().entered = None;
    assert_eq!(bots.thinks_due(&view), 1, "it has not entered");
}

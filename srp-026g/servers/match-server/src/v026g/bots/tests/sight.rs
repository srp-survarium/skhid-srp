use super::*;

/// The ray budget of a poll is now the batteries' (an enemy in view is
/// tested outside it, whole), and is shared by the thinks due only when it
/// binds (`sense::fair_share`): a match whose budget does not bind is
/// decided the same whatever the budget (400 rays a poll or ten times that),
/// shot for shot.
#[test]
pub(crate) fn a_budget_that_does_not_bind_changes_no_decision() {
    let run = |rays_per_poll: u32| {
        let mut tuning = Tuning::default();
        tuning.think.rays_per_poll = rays_per_poll;
        let out = soak_tuned(4, 3, 0, None, 60, 4, tuning);
        (out.kills, out.shots, out.hits, out.walled, out.hidden)
    };
    let usual = run(Tuning::default().think.rays_per_poll);
    assert_eq!(usual, run(4_000), "{usual:?}");
    // A budget with nothing in it does bind (the batteries are not looked
    // at), and it is felt.
    assert_ne!(usual, run(0), "{usual:?}");
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
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    let mut view = bots.tracker.view(
        &Clock {
            clock: 5_000,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        },
        &bots.tuning.kin,
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

//! The mode notices (`docs/notes/mode_introductions.md`): a mode's notice
//! is shown in each account's first three matches of the mode, counted for
//! each mode apart, a late joiner's match too. The lobby says so in each
//! place's profile of the options it makes (`MatchProfile::notice`).

use survarium::v026g::lobby::rooms::{ROOM_JOIN_BASE, ROOM_LEAVE};
use survarium::v026g::maps::{GameMode, MapId};
use survarium::v026g::match_msgs::static_info::Notice;

use super::rooms_tests::{create, rows, spec};
use super::*;

/// The map of `mode` the test data has (Blackout: Rudnya's twin of its
/// battery map, `blackout-mode-1.json`).
fn map_of(mode: GameMode) -> MapId {
    maps::playable()
        .find(|map| map.mode == mode)
        .unwrap_or_else(|| panic!("a {mode:?} map"))
        .id
}

/// The notice of the client's place in the match `ticket`, as the lobby
/// made it (a late joiner's: as it was taken in).
fn notice(world: &World, ticket: u32, player: usize) -> Notice {
    let made = world.host.made.locked();
    let (_, setup) = made.iter().find(|(id, _)| *id == ticket).expect("a match made");
    setup
        .players
        .iter()
        .rev()
        .find(|place| place.account_id == world.ids[player].0)
        .expect("a place in it")
        .profile
        .notice
}

/// Alice and Bob play `count` matches of `mode` from a room Play makes on
/// its one map: each match's notice for both.
fn matches_of(world: &mut World, mode: GameMode, count: usize, players: [usize; 2]) -> Vec<[Notice; 2]> {
    world.core.settings.maps = vec![map_of(mode)];
    let mut seen = Vec::new();
    for _ in 0..count {
        let (ticket, team) = world.match_of(players[0], players[1]);
        let made = world.host.made.locked().last().unwrap().1.clone();
        assert_eq!(made.played.mode(), mode);
        seen.push(players.map(|player| notice(world, ticket, player)));
        world.end_match(ticket, team);
        world.advance(Duration::from_secs(3));
        for player in players {
            world.take(player);
        }
    }
    // Out of the room, so the next Play makes one of the next mode.
    for player in players {
        rows(world, player, ROOM_LEAVE, 1);
    }
    seen
}

#[test]
fn a_modes_notice_is_shown_in_each_accounts_first_three_matches_of_it() {
    let mut settings = Settings::local("127.0.0.1:0");
    settings.maps = vec![map_of(GameMode::Blackout)];
    let mut world = World::with_settings(settings);
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let shown = [Notice::Shown; 2];
    let skipped = [Notice::Skipped; 2];

    // Blackout: the first three show it, the fourth does not.
    assert_eq!(
        matches_of(&mut world, GameMode::Blackout, 4, [alice, bob]),
        [shown, shown, shown, skipped]
    );
    // Freeze Tag counts apart: its own three. (Its maps come with a pack:
    // test data without one has the account's word alone.)
    if maps::playable().any(|map| map.mode == GameMode::FreezeTag) {
        assert_eq!(
            matches_of(&mut world, GameMode::FreezeTag, 4, [alice, bob]),
            [shown, shown, shown, skipped]
        );
    } else {
        assert_eq!(
            world.account(alice).mode_introductions.notice(GameMode::FreezeTag),
            Notice::Shown
        );
    }
    // A mode without a notice shows none and counts nothing.
    assert_eq!(
        matches_of(&mut world, GameMode::TeamDeathmatch, 1, [alice, bob]),
        [skipped]
    );
    let counted: Vec<_> = world
        .account(alice)
        .mode_introductions
        .rows()
        .filter(|&(mode, _)| mode == maps::ModeNotice::Blackout)
        .collect();
    assert_eq!(counted, [(maps::ModeNotice::Blackout, 3)]);
    world.assert_clean();
}

/// A late joiner's match counts as one it started: Erin joins the room's
/// Blackout matches late, the notice shown in the first three, none in the
/// fourth.
#[test]
fn a_late_joiners_match_counts_too() {
    let mut settings = Settings::local("127.0.0.1:0");
    settings.maps = vec![map_of(GameMode::Blackout)];
    let mut world = World::with_settings(settings);
    let players = ["Alice", "Bob", "Carol", "Dave"].map(|name| world.connect(name));
    let erin = world.connect("Erin");
    let key = create(&mut world, players[0], spec(6), None);
    for &player in &players[1..] {
        rows(&mut world, player, ROOM_JOIN_BASE + key, 1);
    }
    let late = |world: &mut World| -> Notice {
        world.advance(Duration::from_secs(40));
        let (ticket, _) = sent_to_match(&world.take(players[0])).expect("the room's match");
        for &player in &players[1..] {
            world.take(player);
        }
        world.host.admitting.locked().insert(ticket);
        rows(world, erin, ROOM_JOIN_BASE + key, 1);
        world.take(erin);
        world.advance(Duration::from_secs(16));
        assert_eq!(sent_to_match(&world.take(erin)).map(|(id, _)| id), Some(ticket), "late");
        let notice = notice(world, ticket, erin);
        world.end_match(ticket, 0);
        world.advance(Duration::from_secs(3));
        for player in players.into_iter().chain([erin]) {
            world.take(player);
        }
        rows(world, erin, ROOM_LEAVE, 1);
        notice
    };
    assert_eq!(late(&mut world), Notice::Shown);
    assert_eq!(late(&mut world), Notice::Shown);
    assert_eq!(
        world.account(erin).mode_introductions.notice(GameMode::Blackout),
        Notice::Shown,
        "two of three"
    );
    assert_eq!(late(&mut world), Notice::Shown);
    assert_eq!(
        world.account(erin).mode_introductions.notice(GameMode::Blackout),
        Notice::Skipped,
        "her three late matches counted"
    );
    assert_eq!(late(&mut world), Notice::Skipped);
}

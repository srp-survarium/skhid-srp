//! The Live window (`live`): the list of the matches being played, a
//! match's roster, and watching one as it is played, behind it.

use survarium::v026g::lobby::rooms::ROOM_JOIN_BASE;
use survarium::v026g::lobby::live_watch::LIVE_WATCH_BASE;
use survarium::v026g::lobby::{
    LIVE_ROSTER_ASK, LIVE_ROSTER_START, LIVE_START, LiveWatch, REPLAY_START, ReplayWatchAsk,
};

use super::rooms_tests::{befriend, create, rows, spec};
use super::*;
use crate::v026g::{LivePhase, LiveView};

/// From two players to their room's match: the vote, the wait, a little
/// more.
const ROOM_TO_MATCH: Duration = Duration::from_secs(32);

/// Alice and Bob in a room's match, which the match server has in play:
/// its ticket, the world's indexes `[alice, bob]`.
fn a_match_in_play(world: &mut World) -> (u32, [usize; 2]) {
    let alice = world.connect("Alice");
    let bob = world.connect("Bob");
    let key = create(world, alice, spec(6), None);
    rows(world, bob, ROOM_JOIN_BASE + key, 1);
    world.advance(ROOM_TO_MATCH);
    let (ticket, _) = sent_to_match(&world.take(alice)).expect("Alice plays");
    let mut kills_deaths = [(0, 0); session::MAX_PLAYERS_PER_MATCH];
    kills_deaths[0] = (3, 1);
    kills_deaths[1] = (1, 3);
    world.host.live.locked().insert(
        ticket,
        LiveView {
            present: 0b11,
            points: Some([2, 1]),
            played: Some(Duration::from_secs(75)),
            phase: LivePhase::Playing,
            kills_deaths,
            ..LiveView::default()
        },
    );
    (ticket, [alice, bob])
}

/// The movie's ask to watch `match_id` (a replay session's): its `R1` row.
fn ask_to_watch(world: &mut World, index: usize, match_id: u32) -> String {
    world.take(index);
    let ask = ReplayWatchAsk {
        seconds: 0,
        place: None,
    };
    world.ask(index, request::player_details(ask.player_id(), match_id));
    world
        .take(index)
        .iter()
        .filter_map(|message| match message {
            Decoded::EloList(rows) => rows
                .iter()
                .find(|(name, place, _, _)| *place == REPLAY_START && name.starts_with("R1;"))
                .map(|(name, ..)| name.clone()),
            _ => None,
        })
        .next()
        .expect("an R1 row")
}

/// A Live spectator whose game never reaches the match server, or which
/// signs in anew, gives its place back when its lobby link or session goes:
/// not only when the match ends.
#[test]
fn a_live_watcher_gives_its_place_back_when_its_link_goes() {
    let mut world = World::new();
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    let session = world.ids[carol].1;
    let matchmaking = |world: &World| world.store.matchmaking().watched(session);
    assert!(matchmaking(&world).is_some());
    // Its game crashed while loading: nothing more comes from it.
    drop(std::mem::replace(&mut world.clients[carol].1, TestClient::new()));
    world.advance(world.core.settings.match_link_timeout + Duration::from_secs(1));
    assert_eq!(matchmaking(&world), None);
}

/// The session's end is enough on its own, whichever way it comes.
#[test]
fn a_live_watcher_watches_no_more_when_its_session_ends() {
    let mut world = World::new();
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    let session = world.ids[carol].1;
    assert!(world.store.end_session(session));
    assert_eq!(world.store.matchmaking().watched(session), None);
}

/// Past the limit of lookups of unknown matches, an ask says nothing of a
/// match being played either: asking was how to tell a running private
/// match from no match at all.
#[test]
fn past_the_guessing_limit_a_running_match_is_not_told_apart() {
    use session::v026g::replay_watch::{MISSES_ALLOWED, WatchSettings};
    use session::v026g::replays::Replays;
    let desk: &'static Replays =
        Box::leak(Box::new(Replays::new(None).with_watch(WatchSettings::default())));
    let mut world = World::new();
    world.core.replays = Some(desk);
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    let account = world.ids[carol].0;
    for n in 0..MISSES_ALLOWED {
        desk.watches().missed(account, 0x2000 + n, world.now);
    }
    assert_eq!(
        ask_to_watch(&mut world, carol, ticket),
        format!("R1;busy;{ticket:08x};0")
    );
    assert_eq!(world.store.matchmaking().watched(world.ids[carol].1), None);
}

#[test]
fn the_live_window_lists_a_match_its_roster_and_watches_it_behind() {
    let mut world = World::new();
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    let list = rows(&mut world, carol, LIVE_START, 20);
    assert_eq!(list.len(), 2, "{list:?}");
    assert_eq!(list[0], "L0;1;30");
    let fields: Vec<&str> = list[1].split(';').collect();
    assert_eq!(fields[0], "L1");
    assert_eq!(fields[1], format!("{ticket:08x}"));
    // In play, one a side, 2 to 1, 75 s, nobody watching, may be watched.
    assert_eq!(&fields[4..], ["2", "1", "1", "2", "1", "75", "0", "1"]);
    assert!(list.iter().all(|row| row.len() <= 63));

    world.ask(carol, request::player_details(LIVE_ROSTER_ASK, ticket));
    let roster = rows(&mut world, carol, LIVE_ROSTER_START, 24);
    assert_eq!(roster.len(), 2, "{roster:?}");
    let alice: Vec<&str> = roster[0].split(';').collect();
    assert_eq!(alice[..2], ["L2", format!("{ticket:08x}").as_str()]);
    assert_eq!(alice[2], "0");
    assert_eq!(alice[4..], ["3", "1", "Alice"]);
    assert!(roster[1].ends_with(";1;3;Bob"), "{}", roster[1]);

    // Watching: sent to the match as a spectator, 30 s behind, Bob (place
    // 1) in view first.
    world.take(carol);
    let ask = ReplayWatchAsk {
        seconds: 0,
        place: Some(1),
    };
    world.ask(carol, request::player_details(ask.player_id(), ticket));
    world.advance(Duration::from_secs(1));
    let decoded = world.take(carol);
    assert_eq!(sent_to_match(&decoded), Some((ticket, 0)));
    assert!(decoded.iter().any(|message| matches!(
        message,
        Decoded::EloList(rows) if rows[0].0 == format!("R1;watching;{ticket:08x};0")
    )));
    let session = world.ids[carol].1;
    let watcher = world.store.matchmaking().watched(session).expect("watches");
    assert_eq!(
        watcher.kind,
        session::WatchKind::Live {
            delay: session::LiveDelay::DEFAULT,
            view: Some(1),
        }
    );
    // No room's queue keeps it, and none ends it.
    world.advance(Duration::from_secs(5));
    assert!(world.store.matchmaking().watched(session).is_some());
    let list = rows(&mut world, carol, LIVE_START, 20);
    assert!(list[1].ends_with(";1;1"), "{}", list[1]);
}

/// The asker's friends in a listed match, after the `L1` rows: those
/// present in it, by place and team.
#[test]
fn the_live_list_names_the_askers_friends_in_each_match() {
    let mut world = World::new();
    let (ticket, [alice, bob]) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    befriend(&mut world, carol, bob);
    let list = rows(&mut world, carol, LIVE_START, 20);
    assert_eq!(list.len(), 3, "{list:?}");
    assert!(list[1].starts_with("L1;"));
    assert_eq!(list[2], format!("F1;{ticket:08x};1;1;Bob"));
    // Alice too; Bob gone from the match: Alice alone.
    befriend(&mut world, carol, alice);
    world.host.live.locked().get_mut(&ticket).unwrap().present = 0b01;
    let list = rows(&mut world, carol, LIVE_START, 20);
    assert_eq!(list[2..], [format!("F1;{ticket:08x};0;0;Alice")]);
    // Nobody's friend: no F1 row.
    let dave = world.connect("Dave");
    assert_eq!(rows(&mut world, dave, LIVE_START, 20).len(), 2);
}

/// Caps: one Live spectator a match here; past it, `busy` and the list
/// says full. A match not begun is not watched yet.
#[test]
fn a_full_match_is_busy_and_one_not_begun_is_not_watched() {
    let mut settings = Settings::local("127.0.0.1:0");
    settings.live.per_match = 1;
    let mut world = World::with_settings(settings);
    let (ticket, _) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    let dave = world.connect("Dave");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    assert_eq!(
        ask_to_watch(&mut world, dave, ticket),
        format!("R1;busy;{ticket:08x};0")
    );
    let list = rows(&mut world, dave, LIVE_START, 20);
    assert!(list[1].ends_with(";1;0"), "{}", list[1]);
    world.host.live.locked().get_mut(&ticket).unwrap().phase = LivePhase::Gathering;
    let erin = world.connect("Erin");
    assert_eq!(
        ask_to_watch(&mut world, erin, ticket),
        format!("R1;busy;{ticket:08x};0")
    );
}

/// The Settings' box: `3B` at the setting's start (the exe's).
fn set_live_watch(world: &mut World, index: usize, state: LiveWatch) {
    world.ask(index, request::ranking(state.start(), 1));
}

/// The account's setting as the server keeps it.
fn live_watch_of(world: &World, index: usize) -> LiveWatch {
    world.accounts.get(world.ids[index].0).unwrap().locked().extras.live_watch
}

/// The list's `L1` rows for this asker (a fresh look).
fn listed_matches(world: &mut World, index: usize) -> Vec<String> {
    rows(world, index, LIVE_START, 20).into_iter().filter(|row| row.starts_with("L1;")).collect()
}

/// The asker's chosen roster: any `L2` row the server sends for it.
fn roster_rows(world: &mut World, index: usize) -> Vec<String> {
    world.take(index);
    world.ask(index, request::ranking(LIVE_ROSTER_START, 24));
    world
        .take(index)
        .iter()
        .filter_map(|message| match message {
            Decoded::EloList(rows) => Some(rows.clone()),
            _ => None,
        })
        .flatten()
        .map(|(name, ..)| name)
        .filter(|name| name.starts_with("L2;"))
        .collect()
}

#[test]
fn an_account_may_be_watched_until_it_says_otherwise() {
    let mut world = World::new();
    let (ticket, [alice, _]) = a_match_in_play(&mut world);
    assert_eq!(live_watch_of(&world, alice), LiveWatch::Allowed);
    let carol = world.connect("Carol");
    assert_eq!(listed_matches(&mut world, carol).len(), 1);
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
}

/// Off for either one of the match's players: the match is not listed, has
/// no roster, a direct ask is refused as a private room's, and no friend's
/// row shows it. On again: back. Nothing names the player.
#[test]
fn off_for_one_player_hides_the_match_from_everyone_until_on_again() {
    for hiding in [0, 1] {
        let mut world = World::new();
        let (ticket, players) = a_match_in_play(&mut world);
        let carol = world.connect("Carol");
        befriend(&mut world, carol, players[0]);
        befriend(&mut world, carol, players[1]);
        assert_eq!(rows(&mut world, carol, LIVE_START, 20).len(), 4, "listed, two friends");

        set_live_watch(&mut world, players[hiding], LiveWatch::Hidden);
        assert_eq!(live_watch_of(&world, players[hiding]), LiveWatch::Hidden);
        let list = rows(&mut world, carol, LIVE_START, 20);
        assert_eq!(list, ["L0;0;30"], "not listed, no F1 row");

        world.ask(carol, request::player_details(LIVE_ROSTER_ASK, ticket));
        let roster = roster_rows(&mut world, carol);
        assert!(roster.is_empty(), "{roster:?}");

        let session = world.ids[carol].1;
        assert_eq!(
            ask_to_watch(&mut world, carol, ticket),
            format!("R1;not_yours;{ticket:08x};0")
        );
        assert!(world.store.matchmaking().watched(session).is_none());

        set_live_watch(&mut world, players[hiding], LiveWatch::Allowed);
        assert_eq!(rows(&mut world, carol, LIVE_START, 20).len(), 4, "listed again");
        assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    }
}

/// A spectator already there goes on; the next ask is refused.
#[test]
fn a_change_applies_from_then_on() {
    let mut world = World::new();
    let (ticket, [alice, _]) = a_match_in_play(&mut world);
    let carol = world.connect("Carol");
    assert!(ask_to_watch(&mut world, carol, ticket).starts_with("R1;watching;"));
    let session = world.ids[carol].1;
    set_live_watch(&mut world, alice, LiveWatch::Hidden);
    world.advance(Duration::from_secs(5));
    assert!(world.store.matchmaking().watched(session).is_some(), "not kicked");
    let dave = world.connect("Dave");
    assert_eq!(
        ask_to_watch(&mut world, dave, ticket),
        format!("R1;not_yours;{ticket:08x};0")
    );
}

/// A start in the setting's range that is none of its two is refused: no
/// change, no answer.
#[test]
fn an_invalid_setting_is_refused() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    set_live_watch(&mut world, alice, LiveWatch::Hidden);
    world.take(alice);
    for start in [LIVE_WATCH_BASE, LIVE_WATCH_BASE + 3, LIVE_WATCH_BASE + 1000] {
        world.ask(alice, request::ranking(start, 1));
        assert_eq!(live_watch_of(&world, alice), LiveWatch::Hidden);
        assert!(!world.take(alice).iter().any(|message| matches!(message, Decoded::SquadInfo { .. })));
    }
}

/// `0xA1` tells the game the state: a change is shown back, nothing else
/// resends it.
#[test]
fn the_squad_info_shows_the_setting_back() {
    let mut world = World::new();
    let alice = world.connect("Alice");
    let live_watch = |decoded: &[Decoded]| {
        decoded.iter().rev().find_map(|message| match message {
            Decoded::SquadInfo { live_watch, .. } => Some(*live_watch),
            _ => None,
        })
    };
    assert_eq!(live_watch(&world.take(alice)), Some(1), "the sign-in's");
    set_live_watch(&mut world, alice, LiveWatch::Hidden);
    assert_eq!(live_watch(&world.take(alice)), Some(2));
    set_live_watch(&mut world, alice, LiveWatch::Hidden);
    assert_eq!(live_watch(&world.take(alice)), None, "unchanged: not sent");
    set_live_watch(&mut world, alice, LiveWatch::Allowed);
    assert_eq!(live_watch(&world.take(alice)), Some(1));
}

/// The setting reaches the account's save (a repository's pass writes it),
/// and a restart reads it back.
#[test]
fn a_change_of_the_setting_is_saved() {
    use session::v026g::persist::SqliteGameRepository;
    let dir = std::env::temp_dir().join(format!(
        "srp-live-watch-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let open = || GameAccounts::with_repository(Box::new(SqliteGameRepository::open_in(&dir).unwrap())).unwrap();
    let accounts = open();
    accounts.set_starter_kit(false);
    let mut world = World::with_accounts(Settings::local("127.0.0.1:0"), accounts);
    let alice = world.connect("Alice");
    let account_id = world.ids[alice].0;
    world.accounts.flush();
    set_live_watch(&mut world, alice, LiveWatch::Hidden);
    assert!(world.accounts.is_dirty(account_id));
    world.accounts.flush();
    assert!(!world.accounts.is_dirty(account_id));
    let reopened = open();
    assert_eq!(reopened.get(account_id).unwrap().locked().extras.live_watch, LiveWatch::Hidden);
    drop(reopened);
    let _ = std::fs::remove_dir_all(&dir);
}

//! The player pop-up's Matches tab (`player_matches`): `3B` at
//! `PLAYER_MATCHES_START`, about the account of the asker's last `93`, or
//! at `OWN_MATCHES_START`, about the asker, answered from srp.db's history
//! with its public matches only; and the Watch hub's Recent tab
//! (`recent_matches`) at `RECENT_MATCHES_START` and the next.

use session::v026g::persist::history::{
    Counters, Eligibility, MatchInfo, MatchKind, MatchRecord, PlayerLine, PlayerRecord, Presence,
    Winner,
};
use session::v026g::persist::{GameRepository, SqliteGameRepository};
use session::v026g::replays::{ReplayId, Replays};
use survarium::v026g::lobby::GEAR_BY_PLACE;
use survarium::v026g::lobby::player_matches::{
    OWN_MATCHES_START, PLAYER_MATCHES_ASK, PLAYER_MATCHES_START,
};
use survarium::v026g::lobby::recent_matches::RECENT_MATCHES_START;
use survarium::v026g::maps::GameMode;

use super::*;

const BOB: u32 = 900_001;

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "srp-lobby-matches-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Bob's (account `bob`) match `n` (match id `0x100 + n`), ended `n` hours
/// after the epoch's first day: he is on team `n % 2`, team 0 wins.
fn bobs_match(bob: u32, n: u32, kind: MatchKind) -> MatchRecord {
    let ended_at = 86_400 + u64::from(n) * 3_600;
    MatchRecord {
        replay_id: ReplayId::new(
            0x100 + n,
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(ended_at),
        ),
        kind,
        info: MatchInfo {
            ended_at,
            duration_ms: 600_000,
            map_id: 3,
            game_mode: GameMode::Batteries,
            match_level: 5,
            winner: Winner::FirstTeam,
            team_points: [5, 2],
        },
        players: vec![PlayerRecord {
            account_id: bob,
            clan_id: None,
            line: PlayerLine {
                team: (n % 2) as u8,
                place: 2,
                // an even match is of before seats were kept
                seat: (n % 2 == 1).then_some(4),
                presence: Presence::Finished,
                rating: 100,
                score: 100,
                kills: u64::from(n),
                deaths: 3,
                counters: Counters::default(),
            },
            kits: Vec::new(),
        }],
        events: Vec::new(),
    }
}

/// A world on srp.db in `dir` with Bob's (account `bob`) 12 public matches
/// (0 to 11), then a private one and one against bots, newer than all.
fn world(dir: &std::path::Path, bob: u32) -> World {
    {
        let mut repository = SqliteGameRepository::open_in(dir).unwrap();
        for n in 0..12 {
            let record = bobs_match(bob, n, MatchKind::Public(Eligibility::Counted));
            repository.record_match(&record).unwrap();
        }
        repository
            .record_match(&bobs_match(bob, 12, MatchKind::Private))
            .unwrap();
        repository
            .record_match(&bobs_match(bob, 13, MatchKind::VsBots))
            .unwrap();
    }
    let repository = SqliteGameRepository::open_in(dir).unwrap();
    let accounts = GameAccounts::with_repository(Box::new(repository)).unwrap();
    accounts.set_starter_kit(false);
    World::with_accounts(Settings::local("127.0.0.1:0"), accounts)
}

/// The rows of the answer to the tab's ask for `count`.
fn matches(world: &mut World, index: usize, count: u8) -> Vec<String> {
    matches_at(world, index, PLAYER_MATCHES_START, count)
}

/// The rows of the answer to the ask at `start` for `count`.
fn matches_at(world: &mut World, index: usize, start: u32, count: u8) -> Vec<String> {
    rows_at(world, index, start, count, PLAYER_MATCHES_START)
}

/// The rows of the answer to the ask at `start` for `count`, at places
/// from `first` on.
fn rows_at(world: &mut World, index: usize, start: u32, count: u8, first: u32) -> Vec<String> {
    world.take(index);
    world.ask(index, request::ranking(start, count));
    let decoded = world.take(index);
    let [Decoded::EloList(rows)] = &decoded[..] else {
        panic!("not one 91: {decoded:?}");
    };
    rows.iter()
        .enumerate()
        .map(|(i, (name, place, rating, change))| {
            assert_eq!((*place, *rating, *change), (first + i as u32, 0, 0));
            assert!(name.len() <= 63, "{name}");
            name.clone()
        })
        .collect()
}

#[test]
fn the_tab_lists_the_shown_players_public_matches_newest_first() {
    let dir = temp("public");
    let mut world = world(&dir, BOB);
    let (bob, _) = world.accounts.sign_up(BOB, "Bob");
    bob.locked().rating = 5000;
    let alice = world.connect("Alice");

    // No `93` yet: nobody's matches.
    assert_eq!(matches(&mut world, alice, 10), ["P1;0;"]);

    // Bob's kits from the Rankings (place 1): the tab is about Bob.
    world.ask(alice, request::ranking(0, 100));
    world.take(alice);
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 1));
    let rows = matches(&mut world, alice, 10);
    assert_eq!(rows.len(), 11);
    assert_eq!(rows[0], "P1;10;Bob");
    // Newest public first: match 11 (team 1, a defeat), then 10 (team 0, a
    // victory); the private one and the one against bots never.
    let now = session::v026g::quests::now();
    let minutes = |n: u64| (now - (86_400 + n * 3_600)) / 60;
    assert_eq!(
        rows[1],
        format!("P2;0000010b;4;3;2;0;11;3;{};0", minutes(11))
    );
    assert_eq!(
        rows[2],
        format!("P2;0000010a;255;3;2;1;10;3;{};0", minutes(10))
    );
    assert!(rows.last().unwrap().starts_with("P2;00000102;"));
    // At most what is asked for, and ten.
    assert_eq!(matches(&mut world, alice, 3).len(), 4);
    assert_eq!(matches(&mut world, alice, 200).len(), 11);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_own_ask_lists_the_askers_matches() {
    let dir = temp("own");
    // Bob signs in first: account 1.
    let mut world = world(&dir, 1);
    let bob = world.connect("Bob");
    assert_eq!(world.ids[bob].0, 1);
    world.accounts.get(1).unwrap().locked().rating = 5000;
    let alice = world.connect("Alice");

    // Whoever Bob was shown last, its own ask is about Bob. Its own list has its match against
    // bots and its private room's too, the newest first, each marked in a last field; a public
    // match's row has no such field.
    let own = matches_at(&mut world, bob, OWN_MATCHES_START, 10);
    assert_eq!(own.len(), 11);
    assert_eq!(own[0], "P1;10;Bob");
    assert!(own[1].starts_with("P2;0000010d;") && own[1].ends_with(";2"), "{}", own[1]);
    assert!(own[2].starts_with("P2;0000010c;") && own[2].ends_with(";1"), "{}", own[2]);
    assert!(own[3].starts_with("P2;0000010b;"));
    assert_eq!(own[3].split(';').count(), 10);
    // Alice is shown Bob's public matches only: the same rows, without those two.
    world.ask(alice, request::ranking(0, 100));
    world.take(alice);
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 1));
    let shown = matches(&mut world, alice, 10);
    assert_eq!(shown[1..9], own[3..11]);
    assert!(shown.iter().all(|row| !row.starts_with("P2;0000010c;") && !row.starts_with("P2;0000010d;")));
    // Alice played none.
    assert_eq!(
        matches_at(&mut world, alice, OWN_MATCHES_START, 10),
        ["P1;0;Alice"]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_search_chooses_whose_matches_are_listed() {
    let dir = temp("search");
    let mut world = world(&dir, BOB);
    let _ = world.accounts.sign_up(BOB, "Bob");
    let alice = world.connect("Alice");

    // The Watch window's search found Bob (find_players): his matches.
    world.ask(alice, request::player_details(PLAYER_MATCHES_ASK, BOB));
    let rows = matches(&mut world, alice, 10);
    assert_eq!(rows.len(), 11);
    assert_eq!(rows[0], "P1;10;Bob");
    // An account there is not chooses nobody new: still Bob's.
    world.ask(alice, request::player_details(PLAYER_MATCHES_ASK, 777_777));
    assert_eq!(matches(&mut world, alice, 10), rows);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_row_says_its_replays_state_for_the_askers_game() {
    let dir = temp("replay");
    let mut world = world(&dir.join("db"), BOB);
    let recordings = dir.join("recordings");
    std::fs::create_dir_all(&recordings).unwrap();
    let replays: &'static Replays = Box::leak(Box::new(Replays::new(Some(recordings.clone()))));
    // Match 11's recording is kept, match 10's still being written, match
    // 9's deleted by the cap, match 8's never kept. Who may watch hides no
    // row: Alice played none of them.
    for name in [
        "rec026g_19700102-100000_0000010b.srprec",
        "rec026g_19700102-100000_0000010a.srprec.part",
    ] {
        std::fs::write(recordings.join(name), b"SRPREC01").unwrap();
    }
    replays.recording_deleted(0x109);
    world.core.replays = Some(replays);
    let (bob, _) = world.accounts.sign_up(BOB, "Bob");
    bob.locked().rating = 5000;
    let alice = world.connect("Alice");
    world.ask(alice, request::ranking(0, 100));
    world.take(alice);
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 1));
    let rows = matches(&mut world, alice, 4);
    let replay: Vec<&str> = rows[1..]
        .iter()
        .map(|row| row.rsplit(';').next().unwrap())
        .collect();
    // Kept, kept, gone, not recorded.
    assert_eq!(replay, ["1", "1", "3", "0"]);
    // The Recent tab's rows say the same.
    let recent = rows_at(
        &mut world,
        alice,
        RECENT_MATCHES_START,
        4,
        RECENT_MATCHES_START,
    );
    let replay: Vec<&str> = recent[1..]
        .iter()
        .map(|row| row.rsplit(';').next().unwrap())
        .collect();
    assert_eq!(replay, ["1", "1", "3", "0"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_recent_tab_lists_public_matches_all_or_the_friends() {
    let dir = temp("recent");
    let mut world = world(&dir, BOB);
    let recent = |world: &mut World, index, f: u32, count| {
        rows_at(
            world,
            index,
            RECENT_MATCHES_START + f,
            count,
            RECENT_MATCHES_START,
        )
    };
    let alice = world.connect("Alice");
    let rows = recent(&mut world, alice, 0, 20);
    // Bob's 12 public matches, newest first; the private one and the one
    // against bots never.
    assert_eq!(rows.len(), 13);
    assert_eq!(rows[0], "M0;12");
    let now = session::v026g::quests::now();
    let minutes = |n: u64| (now - (86_400 + n * 3_600)) / 60;
    assert_eq!(
        rows[1],
        format!("M1;0000010b;3;2;5;2;1;600;{};0", minutes(11))
    );
    assert!(rows[12].starts_with("M1;00000100;"));
    assert_eq!(recent(&mut world, alice, 0, 3).len(), 4);

    // A match no trusted world gave a standing has no score.
    let mut unknown = bobs_match(BOB, 14, MatchKind::Public(Eligibility::Counted));
    unknown.info.winner = Winner::Unknown;
    unknown.info.team_points = [0, 0];
    SqliteGameRepository::open_in(&dir)
        .unwrap()
        .record_match(&unknown)
        .unwrap();
    assert!(recent(&mut world, alice, 0, 1)[1].starts_with("M1;0000010e;3;2;-;-;1;600;"));

    // Friends only: none until Bob is Alice's friend.
    assert_eq!(recent(&mut world, alice, 1, 20), ["M0;0"]);
    let alice_id = world.ids[alice].0;
    world
        .store
        .account(alice_id)
        .unwrap()
        .locked()
        .friends
        .push(BOB);
    let _ = world.accounts.sign_up(BOB, "Bob");
    let friends = recent(&mut world, alice, 1, 20);
    assert_eq!(friends[0], "M0;13");
    assert!(friends[1].starts_with("M1;0000010e;"));
    // After the M1 rows, Bob in each (of the history: no roster place),
    // in the matches' order.
    assert_eq!(friends.len(), 1 + 13 + 13, "{friends:?}");
    assert_eq!(friends[14], "F1;0000010e;255;0;Bob");
    assert_eq!(friends[15], "F1;0000010b;255;1;Bob");
    // The whole list names him too; one who is no friend's has none.
    assert_eq!(recent(&mut world, alice, 0, 20).len(), 1 + 13 + 13);
    let carol = world.connect("Carol");
    assert_eq!(recent(&mut world, carol, 0, 20).len(), 1 + 13);
    let _ = std::fs::remove_dir_all(&dir);
}

//! The player pop-up's Matches tab (`player_matches`): `3B` at
//! `PLAYER_MATCHES_START`, about the account of the asker's last `93`,
//! answered from srp.db's history with its public matches only.

use session::v026g::persist::history::{
    Counters, Eligibility, MatchInfo, MatchKind, MatchRecord, PlayerLine, PlayerRecord, Presence,
    Winner,
};
use session::v026g::persist::{GameRepository, SqliteGameRepository};
use session::v026g::replays::{ReplayId, ReplaySettings, Replays, Sidecar};
use survarium::v026g::lobby::GEAR_BY_PLACE;
use survarium::v026g::lobby::player_matches::PLAYER_MATCHES_START;
use survarium::v026g::maps::GameMode;

use super::*;

const BOB: u32 = 900_001;

fn temp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "srp-lobby-matches-{name}-{}-{}",
        std::process::id(),
        Replays::now_unix()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Bob's match `n` (match id `0x100 + n`), ended `n` hours after the
/// epoch's first day: he is on team `n % 2`, team 0 wins.
fn bobs_match(n: u32, kind: MatchKind) -> MatchRecord {
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
            account_id: BOB,
            clan_id: None,
            line: PlayerLine {
                team: (n % 2) as u8,
                place: 2,
                presence: Presence::Finished,
                rating: Some(100),
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

/// A world on srp.db in `dir` with Bob's 12 public matches (0 to 11), then
/// a private one and one against bots, newer than all.
fn world(dir: &std::path::Path) -> World {
    {
        let mut repository = SqliteGameRepository::open_in(dir).unwrap();
        for n in 0..12 {
            let record = bobs_match(n, MatchKind::Public(Eligibility::Counted));
            assert!(repository.record_match(&record).unwrap());
        }
        repository
            .record_match(&bobs_match(12, MatchKind::Private))
            .unwrap();
        repository
            .record_match(&bobs_match(13, MatchKind::VsBots))
            .unwrap();
    }
    let repository = SqliteGameRepository::open_in(dir).unwrap();
    let accounts = GameAccounts::with_repository(Box::new(repository)).unwrap();
    accounts.set_starter_kit(false);
    World::with_accounts(Settings::local("127.0.0.1:0"), accounts)
}

/// The rows of the answer to the tab's ask for `count`.
fn matches(world: &mut World, index: usize, count: u8) -> Vec<String> {
    world.take(index);
    world.ask(index, request::ranking(PLAYER_MATCHES_START, count));
    let decoded = world.take(index);
    let [Decoded::EloList(rows)] = &decoded[..] else {
        panic!("not one 91: {decoded:?}");
    };
    rows.iter()
        .enumerate()
        .map(|(i, (name, place, rating, change))| {
            assert_eq!(
                (*place, *rating, *change),
                (PLAYER_MATCHES_START + i as u32, 0, 0)
            );
            assert!(name.len() <= 63, "{name}");
            name.clone()
        })
        .collect()
}

#[test]
fn the_tab_lists_the_shown_players_public_matches_newest_first() {
    let dir = temp("public");
    let mut world = world(&dir);
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
        format!("P2;0000010b;2;3;2;0;11;3;{};0", minutes(11))
    );
    assert_eq!(
        rows[2],
        format!("P2;0000010a;2;3;2;1;10;3;{};0", minutes(10))
    );
    assert!(rows.last().unwrap().starts_with("P2;00000102;"));
    // At most what is asked for, and ten.
    assert_eq!(matches(&mut world, alice, 3).len(), 4);
    assert_eq!(matches(&mut world, alice, 200).len(), 11);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_match_whose_replay_is_kept_says_so() {
    let dir = temp("replay");
    let mut world = world(&dir.join("db"));
    let settings = ReplaySettings {
        replay_dir: dir.join("replays"),
        record_dir: dir.join("recordings"),
        modes_dir: dir.join("modes"),
        max_recording_bytes: 1 << 20,
        max_replay_bytes: 1 << 20,
    }
    .checked()
    .unwrap();
    let replays: &'static Replays = Box::leak(Box::new(Replays::new(Some(settings), 6)));
    // Match 11's replay is made; match 10's recording is kept.
    std::fs::write(replays.sur_path(0x10B).unwrap(), b"0123456789ab").unwrap();
    let sidecar = Sidecar {
        match_id: 0x10B,
        size: 12,
        sha256: "cd".repeat(32),
        revisions: vec![None],
        account_ids: vec![BOB],
        made_at: 0,
    };
    std::fs::write(replays.json_path(0x10B).unwrap(), sidecar.to_json()).unwrap();
    std::fs::write(
        dir.join("recordings")
            .join("rec026g_19700102-100000_0000010a.srprec"),
        b"SRPREC01",
    )
    .unwrap();
    world.core.replays = Some(replays);
    let (bob, _) = world.accounts.sign_up(BOB, "Bob");
    bob.locked().rating = 5000;
    let alice = world.connect("Alice");
    world.ask(alice, request::ranking(0, 100));
    world.take(alice);
    world.ask(alice, request::player_details(GEAR_BY_PLACE, 1));
    let rows = matches(&mut world, alice, 3);
    let replay: Vec<&str> = rows[1..]
        .iter()
        .map(|row| row.rsplit(';').next().unwrap())
        .collect();
    assert_eq!(replay, ["1", "1", "0"]);
    let _ = std::fs::remove_dir_all(&dir);
}

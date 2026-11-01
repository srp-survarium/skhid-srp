//! The API against a history kept in a temporary `srp.db`: each
//! endpoint's shape (`docs/notes/match_history.md`), what is never served,
//! and the refusals.

use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;
use session::v026g::account::Account;
use session::v026g::clans::{Clan, Member, Rank, status};
use session::v026g::persist::history::{
    Counters, MatchInfo, MatchKind, MatchRecord, PlayerLine, PlayerRecord, Presence, Winner,
};
use session::v026g::{GameAccounts, GameRepository, SqliteGameRepository};
use session::{Ban, SessionStore};

use super::*;
use crate::{LiveMatch, LobbyLink, MatchCounter};

const NOW: u64 = 1_793_500_000;

struct Nothing;

impl LobbyLink for Nothing {
    fn account_changed(&self, _account_id: u32) {}
    fn set_admin_text(&self, _first: &str, _second: &str) {}
    fn clear_admin_text(&self) {}
    fn admin_text(&self) -> Option<(String, String)> {
        None
    }
}

impl MatchCounter for Nothing {
    fn active_matches(&self) -> usize {
        0
    }
    fn live_matches(&self) -> Vec<LiveMatch> {
        Vec::new()
    }
}

struct World {
    dir: std::path::PathBuf,
    store: SessionStore,
    game: Game,
    /// Alice's and Bob's account ids.
    alice: u32,
    bob: u32,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn line(team: u8, score: u64, kills: u64, deaths: u64) -> PlayerLine {
    PlayerLine {
        team,
        place: 1,
        presence: Presence::Finished,
        rating: 120,
        score,
        kills,
        deaths,
        counters: Counters {
            assists: 4,
            headshots: 3,
            grenade_kills: 1,
            melee_kills: 0,
            batteries: 2,
            capture_hundredths: 0,
        },
    }
}

fn record(kind: MatchKind, ended_at: u64, alice: u32, bob: u32) -> MatchRecord {
    MatchRecord {
        game_match_id: 0x1234,
        kind,
        info: MatchInfo {
            ended_at,
            duration_ms: 844_500,
            map_id: 3,
            game_mode: 2,
            match_level: 12,
            winner: Winner::FirstTeam,
            team_points: [5, 3],
        },
        players: vec![
            PlayerRecord {
                account_id: alice,
                clan_id: Some(7),
                line: line(0, 310, 14, 6),
            },
            PlayerRecord {
                account_id: bob,
                clan_id: None,
                line: PlayerLine {
                    presence: Presence::Left,
                    ..line(1, 100, 6, 14)
                },
            },
        ],
    }
}

/// Alice (of clan SKH) and Bob, signed up with login names other than
/// their in-game names; three public matches, a private one and one
/// against bots.
fn world(name: &str) -> World {
    let dir = std::env::temp_dir().join(format!(
        "srp-web-history-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = SessionStore::new();
    let ip = "127.0.0.1".parse().unwrap();
    let digest = b"da39a3ee5e6b4b0d3255bfef95601890afd80709";
    let alice = store
        .sign_in_with_digest_from("alice_login", digest, ip, Instant::now())
        .unwrap()
        .account_id();
    let bob = store
        .sign_in_with_digest_from("bob_login", digest, ip, Instant::now())
        .unwrap()
        .account_id();
    let mut repository = SqliteGameRepository::open_in(&dir).unwrap();
    repository.save(alice, &Account::new("Alice")).unwrap();
    repository.save(bob, &Account::new("Bob")).unwrap();
    repository
        .save_clan(&Clan {
            id: 7,
            name: "Skhid".to_owned(),
            tag: "SKH".to_owned(),
            registered: true,
            leader: alice,
            ranks: vec![Rank {
                name: "st_clan_rank_commander".to_owned(),
                permissions: u8::MAX,
                is_default: true,
            }],
            members: vec![Member {
                account_id: alice,
                rank: 0,
                status: status::ACCEPTED,
            }],
            daily_message: String::new(),
            experience: 0,
            money: 0,
        })
        .unwrap();
    for (kind, at) in [
        (MatchKind::Public, NOW - 3 * 86_400),
        (MatchKind::Public, NOW - 5_000),
        (MatchKind::Private, NOW - 2_000),
        (MatchKind::VsBots, NOW - 1_500),
        (MatchKind::Public, NOW - 1_000),
    ] {
        repository
            .record_match(&record(kind, at, alice, bob))
            .unwrap();
    }
    let accounts = Arc::new(GameAccounts::with_repository(Box::new(repository)).unwrap());
    let game = Game {
        accounts,
        lobby: Arc::new(Nothing),
        matches: Arc::new(Nothing),
        downloads: Arc::new(crate::Downloads::new(Vec::new(), None, None)),
        min_patch_revision: 0,
        replays: None,
    };
    World {
        dir,
        store,
        game,
        alice,
        bob,
    }
}

impl World {
    fn get(&self, target: &str) -> (&'static str, Value) {
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let response = response(path, query, Lang::En, &self.store, &self.game, NOW);
        assert!(matches!(response.content_type, ContentType::Json));
        (
            response.status,
            serde_json::from_str(&response.body).unwrap(),
        )
    }

    fn ok(&self, target: &str) -> Value {
        let (status, body) = self.get(target);
        assert_eq!(status, "200 OK", "{target}: {body}");
        body
    }
}

fn keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    keys
}

const TOTALS: [&str; 15] = [
    "assists",
    "batteries",
    "captures",
    "deaths",
    "grenade_kills",
    "headshots",
    "kd",
    "kills",
    "losses",
    "matches",
    "melee_kills",
    "score",
    "score_avg",
    "win_rate",
    "wins",
];

const SUMMARY: [&str; 10] = [
    "duration_s",
    "ended_at",
    "id",
    "level",
    "map",
    "map_id",
    "mode",
    "players",
    "team_points",
    "winner",
];

const LINE: [&str; 15] = [
    "assists",
    "batteries",
    "captures",
    "deaths",
    "grenade_kills",
    "headshots",
    "kd",
    "kills",
    "left",
    "melee_kills",
    "place",
    "rating",
    "score",
    "team",
    "won",
];

fn sorted(lists: &[&[&'static str]]) -> Vec<&'static str> {
    let mut all: Vec<&str> = lists.iter().flat_map(|list| list.iter().copied()).collect();
    all.sort_unstable();
    all
}

#[test]
fn a_player_is_its_card_and_totals_of_public_matches() {
    let world = world("player");
    let body = world.ok("/api/history/player?name=alice");
    assert_eq!(
        keys(&body),
        ["experience", "level", "player", "rating", "totals"]
    );
    assert_eq!(body["player"]["id"], world.alice);
    assert_eq!(body["player"]["name"], "Alice");
    assert_eq!(
        body["player"]["clan"],
        serde_json::json!({ "id": 7, "tag": "SKH" })
    );
    assert_eq!(keys(&body["totals"]), TOTALS);
    let totals = &body["totals"];
    assert_eq!(
        (totals["matches"].as_u64(), totals["wins"].as_u64()),
        (Some(3), Some(3))
    );
    assert_eq!(totals["kills"], 42);
    assert_eq!(totals["kd"], 2.33);
    assert_eq!(totals["win_rate"], 1.0);
    assert_eq!(totals["score_avg"], 310.0);
    let by_id = world.ok(&format!("/api/history/player?id={}", world.bob));
    assert_eq!(by_id["player"]["name"], "Bob");
    assert_eq!(by_id["player"]["clan"], Value::Null);
    assert_eq!(by_id["totals"]["losses"], 3);
    // A login name is no player's name.
    assert_eq!(
        world.get("/api/history/player?name=alice_login").0,
        "404 Not Found"
    );
    assert_eq!(world.get("/api/history/player").0, "400 Bad Request");
    assert_eq!(world.get("/api/history/player?id=x").0, "400 Bad Request");
}

#[test]
fn a_players_matches_are_pages_of_its_public_ones() {
    let world = world("player-matches");
    let first = world.ok(&format!(
        "/api/history/player/matches?id={}&limit=2",
        world.alice
    ));
    assert_eq!(keys(&first), ["matches", "next"]);
    let matches = first["matches"].as_array().unwrap();
    let ids: Vec<u64> = matches.iter().map(|m| m["id"].as_u64().unwrap()).collect();
    assert_eq!(
        ids,
        [5, 2],
        "the private and the bots' matches are not served"
    );
    assert_eq!(keys(&matches[0]), sorted(&[&SUMMARY, &LINE]));
    assert_eq!(matches[0]["winner"], 0);
    assert_eq!(matches[0]["won"], true);
    assert_eq!(matches[0]["duration_s"], 845);
    assert_eq!(matches[0]["team_points"], serde_json::json!([5, 3]));
    assert_eq!(first["next"], 2);
    let last = world.ok(&format!(
        "/api/history/player/matches?id={}&limit=2&before=2",
        world.alice
    ));
    assert_eq!(last["matches"].as_array().unwrap().len(), 1);
    assert_eq!(last["next"], Value::Null);
    let bob = world.ok(&format!("/api/history/player/matches?id={}", world.bob));
    assert_eq!(bob["matches"][0]["left"], true);
    assert_eq!(bob["matches"][0]["won"], false);
    assert_eq!(
        world.get("/api/history/player/matches?id=1&limit=51").0,
        "400 Bad Request"
    );
}

#[test]
fn a_players_days_add_up_by_day() {
    let world = world("days");
    let body = world.ok(&format!("/api/history/player/days?id={}", world.alice));
    let days = body["days"].as_array().unwrap();
    assert_eq!(days.len(), 2);
    assert_eq!(keys(&days[0]), sorted(&[&TOTALS, &["day"]]));
    assert_eq!(days[1]["matches"], 2);
    let today = world.ok(&format!(
        "/api/history/player/days?id={}&days=1",
        world.alice
    ));
    assert_eq!(today["days"].as_array().unwrap().len(), 1);
    assert_eq!(
        world.get("/api/history/player/days?id=1&days=91").0,
        "400 Bad Request"
    );
}

#[test]
fn matches_are_listed_and_shown_public_only() {
    let world = world("matches");
    let list = world.ok("/api/history/matches");
    let ids: Vec<u64> = list["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_u64().unwrap())
        .collect();
    assert_eq!(ids, [5, 2, 1]);
    assert_eq!(keys(&list["matches"][0]), SUMMARY);
    assert_eq!(list["next"], Value::Null);
    let clan = world.ok("/api/history/matches?clan=7&limit=1");
    assert_eq!(clan["matches"][0]["id"], 5);
    assert_eq!(clan["next"], 5);

    let shown = world.ok("/api/history/match?id=5");
    assert_eq!(keys(&shown), ["match", "players"]);
    assert_eq!(keys(&shown["match"]), SUMMARY);
    let players = shown["players"].as_array().unwrap();
    assert_eq!(players.len(), 2);
    assert_eq!(keys(&players[0]), sorted(&[&LINE, &["level", "player"]]));
    assert_eq!(players[0]["player"]["name"], "Alice");
    assert_eq!(players[1]["left"], true);
    for hidden in [3, 4, 99] {
        assert_eq!(
            world.get(&format!("/api/history/match?id={hidden}")).0,
            "404 Not Found"
        );
    }
}

#[test]
fn the_top_lists_leave_out_the_banned() {
    let world = world("top");
    let by_kills = world.ok("/api/history/top?by=kills");
    assert_eq!(by_kills["by"], "kills");
    let players = by_kills["players"].as_array().unwrap();
    assert_eq!(players.len(), 2);
    assert_eq!(keys(&players[0]), ["level", "player", "rating", "totals"]);
    assert_eq!(players[0]["player"]["name"], "Alice");
    assert_eq!(world.ok("/api/history/top")["by"], "rating");

    world.store.ban_account(
        world.alice,
        Ban {
            until: None,
            reason: "secret reason".to_owned(),
        },
    );
    let by_kills = world.ok("/api/history/top?by=kills");
    let players = by_kills["players"].as_array().unwrap();
    assert_eq!(players.len(), 1);
    assert_eq!(players[0]["player"]["name"], "Bob");
    assert_eq!(
        world.get("/api/history/top?by=balance").0,
        "400 Bad Request"
    );
}

#[test]
fn the_best_scores_are_of_the_period() {
    let world = world("best");
    let day = world.ok("/api/history/best");
    assert_eq!(day["period"], "day");
    let rows = day["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 4, "two public matches today, two players each");
    assert_eq!(
        keys(&rows[0]),
        [
            "deaths", "ended_at", "kills", "level", "map", "match", "mode", "player", "score"
        ]
    );
    assert_eq!(rows[0]["score"], 310);
    let hour = world.ok("/api/history/best?period=hour");
    assert_eq!(hour["rows"].as_array().unwrap().len(), 2);
    assert_eq!(
        world.get("/api/history/best?period=week").0,
        "400 Bad Request"
    );
}

#[test]
fn clans_have_totals_and_members() {
    let world = world("clans");
    let list = world.ok("/api/history/clans");
    assert_eq!(list["by"], "score");
    let clans = list["clans"].as_array().unwrap();
    assert_eq!(clans.len(), 1);
    assert_eq!(
        keys(&clans[0]),
        ["id", "level", "members", "name", "tag", "totals"]
    );
    assert_eq!(clans[0]["totals"]["matches"], 3);

    let card = world.ok("/api/history/clan?tag=SKH");
    assert_eq!(keys(&card), ["clan", "members", "totals"]);
    assert_eq!(
        keys(&card["clan"]),
        ["id", "leader", "level", "members", "name", "tag"]
    );
    assert_eq!(card["clan"]["leader"]["name"], "Alice");
    let members = card["members"].as_array().unwrap();
    assert_eq!(
        keys(&members[0]),
        ["commander", "level", "player", "rating", "totals"]
    );
    assert_eq!(members[0]["commander"], true);
    assert_eq!(world.ok("/api/history/clan?id=7")["clan"]["tag"], "SKH");
    assert_eq!(world.get("/api/history/clan?id=8").0, "404 Not Found");
}

#[test]
fn nothing_private_is_ever_in_an_answer() {
    let world = world("private");
    world.store.ban_account(
        world.bob,
        Ban {
            until: None,
            reason: "secret reason".to_owned(),
        },
    );
    for target in [
        "/api/history/player?name=Bob".to_owned(),
        format!("/api/history/player/matches?id={}", world.bob),
        "/api/history/match?id=5".to_owned(),
        "/api/history/matches".to_owned(),
        "/api/history/best".to_owned(),
        "/api/history/clan?id=7".to_owned(),
    ] {
        let body = world.ok(&target).to_string();
        for secret in [
            "_login",
            "127.0.0.1",
            "secret reason",
            "balance",
            "password",
        ] {
            assert!(!body.contains(secret), "{target}: {body}");
        }
    }
}

#[test]
fn an_unknown_endpoint_is_not_found_and_no_database_no_history() {
    let world = world("unknown");
    assert_eq!(world.get("/api/history/nothing").0, "404 Not Found");
    let game = Game {
        accounts: Arc::new(GameAccounts::new()),
        ..world.game.clone()
    };
    let response = response(
        "/api/history/matches",
        "",
        Lang::En,
        &world.store,
        &game,
        NOW,
    );
    assert_eq!(response.status, "503 Service Unavailable");
}

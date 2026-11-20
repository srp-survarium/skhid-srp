//! The API against a history kept in a temporary `srp.db`: each
//! endpoint's shape (`docs/notes/match_history.md`), what is never served,
//! and the refusals.

use std::sync::Arc;
use std::time::Instant;

use serde_json::{Value, json};
use session::v026g::account::Account;
use session::v026g::clans::{Clan, Member, Rank, status};
use session::v026g::persist::history::{
    Counters, Eligibility, GearSlot, KitItem, MatchInfo, MatchKind, MatchRecord, PlayerLine,
    Outcome, PlayerRecord, Presence, Winner,
};
use session::v026g::replays::{ReplayId, Replays};
use session::v026g::{GameAccounts, GameRepository, SqliteGameRepository};
use session::{Ban, SessionStore};
use survarium::v026g::maps::{GameMode, MapId};
use survarium::v026g::twist::LobbyMode;

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
    /// Alice's, Bob's and Carol's account ids.
    alice: u32,
    bob: u32,
    carol: u32,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A line of a match team 0 won.
fn line(team: u8, score: u64, kills: u64, deaths: u64) -> PlayerLine {
    PlayerLine {
        team,
        place: 1,
        seat: team,
        presence: Presence::Finished,
        outcome: if team == 0 {
            Outcome::Won
        } else {
            Outcome::Lost
        },
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
            artefacts: 1,
            shots: 400,
            hits: 100,
        },
        played_ms: 840_000,
    }
}

fn dict_id(name: &str) -> u16 {
    survarium::v026g::dictionary::Dictionary::shared()
        .item_named(name)
        .unwrap()
        .dict_id
}

/// A kit at `place` holding `held` (`(slot, item name)`), 14 kills of 400
/// shots, 100 of which hit.
fn kit(place: u8, held: &[(usize, &str)]) -> KitLine {
    KitLine {
        kit: place,
        counts: KitCounts {
            kills: 14,
            headshots: 3,
            shots: 400,
            hits: 100,
        },
        items: held
            .iter()
            .map(|&(slot, name)| KitItem {
                slot: GearSlot::of(slot).unwrap(),
                dict_id: dict_id(name),
            })
            .collect(),
    }
}

fn record(kind: MatchKind, ended_at: u64, alice: u32, bob: u32) -> MatchRecord {
    MatchRecord {
        replay_id: ReplayId::new(
            0x1234,
            std::time::UNIX_EPOCH + std::time::Duration::from_secs(ended_at),
        ),
        kind,
        info: MatchInfo {
            ended_at,
            duration_ms: 844_500,
            map_id: MapId::new(3),
            mode: LobbyMode::Plain(GameMode::Batteries),
            match_level: 12,
            winner: Winner::FirstTeam,
            team_points: [5, 3],
        },
        players: vec![
            PlayerRecord {
                account_id: alice,
                clan_id: Some(7),
                line: line(0, 310, 14, 6),
                kits: vec![kit(
                    0,
                    &[(0, "bm_helmet_1"), (2, "bm_torso_4"), (7, "ak_74u")],
                )],
            },
            PlayerRecord {
                account_id: bob,
                clan_id: None,
                // 600 shots in three public matches: too few for the
                // accuracy list.
                line: PlayerLine {
                    presence: Presence::Left,
                    counters: Counters {
                        shots: 200,
                        hits: 50,
                        ..line(1, 100, 6, 14).counters
                    },
                    ..line(1, 100, 6, 14)
                },
                kits: vec![kit(1, &[(7, "uzi")])],
            },
        ],
    }
}

/// Alice (of clan SKH) and Bob, signed up with login names other than
/// their in-game names; three public matches, a private one and one
/// against bots. Carol, also of SKH, has never played and is still named
/// by her login.
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
    let carol = store
        .sign_in_with_digest_from("carol_login", digest, ip, Instant::now())
        .unwrap()
        .account_id();
    let mut repository = SqliteGameRepository::open_in(&dir).unwrap();
    repository.save(alice, &Account::new("Alice")).unwrap();
    repository.save(bob, &Account::new("Bob")).unwrap();
    repository
        .save(carol, &Account::new("carol_login"))
        .unwrap();
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
            members: [alice, carol]
                .map(|account_id| Member {
                    account_id,
                    rank: 0,
                    status: status::ACCEPTED,
                })
                .to_vec(),
            daily_message: String::new(),
            experience: 0,
            money: 0,
        })
        .unwrap();
    for (kind, at) in [
        (MatchKind::Public(Eligibility::Counted), NOW - 3 * 86_400),
        (MatchKind::Public(Eligibility::Counted), NOW - 5_000),
        (MatchKind::Private, NOW - 2_000),
        (MatchKind::VsBots, NOW - 1_500),
        (MatchKind::Public(Eligibility::Counted), NOW - 1_000),
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
        results: Arc::new(session::v026g::rewards::Results::new()),
    };
    World {
        dir,
        store,
        game,
        alice,
        bob,
        carol,
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

const TOTALS: [&str; 20] = [
    "accuracy",
    "artefacts",
    "assists",
    "batteries",
    "captures",
    "deaths",
    "grenade_kills",
    "headshots",
    "hits",
    "kd",
    "kills",
    "losses",
    "matches",
    "melee_kills",
    "played_s",
    "score",
    "score_avg",
    "shots",
    "win_rate",
    "wins",
];

const SUMMARY: [&str; 15] = [
    "duration_s",
    "eligibility",
    "ended_at",
    "id",
    "level",
    "map",
    "map_icon",
    "map_id",
    "map_key",
    "mode",
    "players",
    "replay_id",
    "rules",
    "team_points",
    "winner",
];

const LINE: [&str; 20] = [
    "accuracy",
    "artefacts",
    "assists",
    "batteries",
    "captures",
    "deaths",
    "grenade_kills",
    "headshots",
    "hits",
    "kd",
    "kills",
    "left",
    "melee_kills",
    "outcome",
    "place",
    "played_s",
    "rating",
    "score",
    "shots",
    "team",
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
    assert_eq!(totals["artefacts"], 3);
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
fn a_player_without_a_public_match_is_not_found() {
    let world = world("never-played");
    // Carol's name is her login: answered as if there were no Carol.
    let unknown = world.get("/api/history/player?name=nobody");
    assert_eq!(unknown.0, "404 Not Found");
    for target in [
        format!("/api/history/player?id={}", world.carol),
        "/api/history/player?name=carol_login".to_owned(),
        "/api/history/player?name=CAROL_LOGIN".to_owned(),
    ] {
        assert_eq!(world.get(&target), unknown, "{target}");
    }
    // Her lists are empty, as an unknown player's.
    for target in [
        format!("/api/history/player/matches?id={}", world.carol),
        format!("/api/history/player/days?id={}", world.carol),
        format!("/api/history/player/equipment?id={}", world.carol),
    ] {
        let body = world.ok(&target).to_string();
        assert!(!body.contains("carol"), "{target}: {body}");
    }
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
    assert_eq!(keys(&matches[0]), sorted(&[&SUMMARY, &LINE, &["kits"]]));
    assert_eq!(matches[0]["winner"], 0);
    assert_eq!(matches[0]["outcome"], "won");
    assert_eq!(matches[0]["duration_s"], 845);
    assert_eq!(matches[0]["team_points"], serde_json::json!([5, 3]));
    assert_eq!(matches[0]["eligibility"], "counted");
    assert_eq!(first["next"], 2);
    let last = world.ok(&format!(
        "/api/history/player/matches?id={}&limit=2&before=2",
        world.alice
    ));
    assert_eq!(last["matches"].as_array().unwrap().len(), 1);
    assert_eq!(last["next"], Value::Null);
    let bob = world.ok(&format!("/api/history/player/matches?id={}", world.bob));
    assert_eq!(bob["matches"][0]["left"], true);
    assert_eq!(bob["matches"][0]["outcome"], "lost");
    assert_eq!(
        world.get("/api/history/player/matches?id=1&limit=51").0,
        "400 Bad Request"
    );
}

#[test]
fn a_match_row_has_the_kits_used_and_their_items() {
    let world = world("kits");
    let body = world.ok(&format!(
        "/api/history/player/matches?id={}&limit=1",
        world.alice
    ));
    let row = &body["matches"][0];
    assert_eq!(
        (row["shots"].as_u64(), row["hits"].as_u64()),
        (Some(400), Some(100))
    );
    assert_eq!(row["accuracy"], 0.25);
    let kits = row["kits"].as_array().unwrap();
    assert_eq!(kits.len(), 1);
    assert_eq!(
        keys(&kits[0]),
        [
            "accuracy",
            "armour",
            "headshots",
            "hits",
            "kills",
            "kit",
            "shots",
            "weapons"
        ]
    );
    assert_eq!(kits[0]["kit"], 0);
    assert_eq!(kits[0]["kills"], 14);
    let weapons = kits[0]["weapons"].as_array().unwrap();
    assert_eq!(weapons.len(), 1);
    assert_eq!(keys(&weapons[0]), ["icon", "id", "key", "name", "slot"]);
    assert_eq!(weapons[0]["slot"], 7);
    assert_eq!(weapons[0]["id"], dict_id("ak_74u"));
    assert!(
        weapons[0]["name"]
            .as_str()
            .is_some_and(|name| !name.is_empty())
    );
    let armour: Vec<u64> = kits[0]["armour"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["slot"].as_u64().unwrap())
        .collect();
    assert_eq!(armour, [0, 2]);

    let shown = world.ok("/api/history/match?id=5");
    assert_eq!(shown["players"][1]["kits"][0]["kit"], 1);
    assert_eq!(
        shown["players"][1]["kits"][0]["weapons"][0]["id"],
        dict_id("uzi")
    );
}

#[test]
fn a_players_equipment_is_its_most_used_items() {
    let world = world("equipment");
    let body = world.ok(&format!("/api/history/player/equipment?id={}", world.alice));
    assert_eq!(keys(&body), ["armour", "weapons"]);
    let weapons = body["weapons"].as_array().unwrap();
    assert_eq!(weapons.len(), 1);
    // No kills or shots of an item: the game counts those by kit.
    assert_eq!(keys(&weapons[0]), ["icon", "id", "key", "matches", "name"]);
    assert_eq!(weapons[0]["key"], "ak_74u");
    assert_eq!(weapons[0]["id"], dict_id("ak_74u"));
    // Its three public matches: the private one and the bots' are not
    // counted.
    assert_eq!(weapons[0]["matches"], 3);
    assert_eq!(body["armour"].as_array().unwrap().len(), 2);
    let one = world.ok(&format!(
        "/api/history/player/equipment?id={}&limit=1",
        world.alice
    ));
    assert_eq!(one["armour"].as_array().unwrap().len(), 1);
    assert_eq!(
        world.get("/api/history/player/equipment?id=1&limit=51").0,
        "400 Bad Request"
    );
    assert_eq!(
        world.get("/api/history/player/equipment").0,
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
    assert_eq!(keys(&shown), ["match", "players", "replay"]);
    assert_eq!(keys(&shown["match"]), SUMMARY);
    let players = shown["players"].as_array().unwrap();
    assert_eq!(players.len(), 2);
    assert_eq!(
        keys(&players[0]),
        sorted(&[&LINE, &["kits", "level", "player"]])
    );
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
    assert_eq!(by_kills["order"], "desc");
    // From the other end.
    let fewest = world.ok("/api/history/top?by=kills&order=asc");
    assert_eq!(fewest["order"], "asc");
    assert_eq!(fewest["players"][0]["player"]["name"], "Bob");
    assert_eq!(
        world.get("/api/history/top?by=kills&order=up").0,
        "400 Bad Request"
    );
    assert_eq!(world.ok("/api/history/clans?order=asc")["order"], "asc");
    // Too few matches yet for the average score's list.
    let by_average = world.ok("/api/history/top?by=score_avg");
    assert_eq!(by_average["by"], "score_avg");
    assert!(by_average["players"].as_array().unwrap().is_empty());
    for key in ["win_rate", "kd", "headshot_rate"] {
        let by_ratio = world.ok(&format!("/api/history/top?by={key}"));
        assert_eq!(by_ratio["by"], key);
        assert!(by_ratio["players"].as_array().unwrap().is_empty());
    }
    assert_eq!(world.get("/api/history/top?by=artefacts").0, "400 Bad Request");
    let accurate = world.ok("/api/history/top?by=accuracy");
    assert_eq!(accurate["by"], "accuracy");
    let players = accurate["players"].as_array().unwrap();
    assert_eq!(players.len(), 1, "Bob has too few shots");
    assert_eq!(players[0]["player"]["name"], "Alice");
    assert_eq!(players[0]["totals"]["shots"], 1200);
    assert_eq!(players[0]["totals"]["accuracy"], 0.25);

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
fn the_days_leaders_and_the_all_time_totals() {
    let world = world("leaders");
    // Two of the three public matches ended today (UTC).
    let today = world.ok("/api/history/today");
    assert_eq!(keys(&today), ["leaders", "since"]);
    assert_eq!(today["since"], NOW - NOW % 86_400);
    assert_eq!(
        keys(&today["leaders"]),
        ["assists", "kills", "played_s", "score", "wins"]
    );
    let kills = today["leaders"]["kills"].as_array().unwrap();
    assert_eq!(kills.len(), 2);
    assert_eq!(keys(&kills[0]), ["player", "value"]);
    assert_eq!((&kills[0]["player"]["name"], &kills[0]["value"]), (&json!("Alice"), &json!(28)));
    assert_eq!(kills[1]["value"], 12);
    // As many of each list as asked, five without a word.
    let first = world.ok("/api/history/today?limit=1");
    assert_eq!(first["leaders"]["kills"], json!([kills[0]]));
    assert_eq!(world.ok("/api/history/today?limit=50")["leaders"], today["leaders"]);
    assert_eq!(world.get("/api/history/today?limit=51").0, "400 Bad Request");
    // Bob won nothing today: not on that list.
    assert_eq!(today["leaders"]["wins"].as_array().unwrap().len(), 1);
    world.store.ban_account(
        world.alice,
        Ban {
            until: None,
            reason: "secret reason".to_owned(),
        },
    );
    let kills = &world.ok("/api/history/today")["leaders"]["kills"];
    assert_eq!(kills.as_array().unwrap().len(), 1, "a banned account leads nothing");
    assert_eq!(kills[0]["player"]["name"], "Bob");

    let all = world.ok("/api/history/totals");
    assert_eq!(
        keys(&all),
        ["accuracy", "assists", "hits", "kills", "matches", "played_s", "players", "shots"]
    );
    assert_eq!((&all["players"], &all["matches"]), (&json!(2), &json!(3)));
    assert_eq!(all["kills"], 3 * (14 + 6));
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
            "deaths", "ended_at", "kills", "level", "map", "map_icon", "map_key", "match", "mode",
            "player", "rules",
            "score"
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
fn the_best_scores_leave_out_the_banned() {
    let world = world("best-banned");
    world.store.ban_account(
        world.alice,
        Ban {
            until: None,
            reason: "secret reason".to_owned(),
        },
    );
    let day = world.ok("/api/history/best");
    let rows = day["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "Bob's two lines of today");
    for row in rows {
        assert_eq!(row["player"]["name"], "Bob");
    }
}

#[test]
fn clans_have_totals_and_members() {
    let world = world("clans");
    let list = world.ok("/api/history/clans");
    assert_eq!(list["by"], "matches");
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
    // Carol, who never played, is counted but not listed.
    assert_eq!(card["clan"]["members"], 2);
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["player"]["name"], "Alice");
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
        format!("/api/history/player/equipment?id={}", world.bob),
        "/api/history/match?id=5".to_owned(),
        "/api/history/matches".to_owned(),
        "/api/history/best".to_owned(),
        "/api/history/clan?id=7".to_owned(),
        "/api/history/top".to_owned(),
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

/// A made-up history for the local preview (`serve_wiki_preview`) and the
/// page's browser check (`scripts/wiki/check_history.mjs`), in a new `srp.db`
/// under `dir`: twelve players and Nova, who never played, two clans (RAD's
/// commander is Nova), and 23 public matches ending at `now`, written as srp
/// writes them: one a free-for-all, one a draw, one too short to count;
/// Yasen in the newest 15 only, with too few shots for the accuracy list;
/// Dub leaves every fifth; one weapon is gone from the game data.
pub(crate) fn preview(dir: &std::path::Path, now: u64) -> GameAccounts {
    const NAMES: [&str; 13] = [
        "Sheep", "Volk", "Kvitka", "Lys", "Bars", "Grim", "Sova", "Taras", "Zhuk", "Orel", "Dub",
        "Yasen", "Nova",
    ];
    const FIRST: u32 = 1042;
    let mut repository = SqliteGameRepository::open_in(dir).unwrap();
    for (index, name) in (0u16..).zip(NAMES) {
        let mut account = Account::new(name);
        account.rating = 1180 - 20 * index;
        if name != "Nova" {
            account.exp_total = 1_234_567 - 90_000 * u32::from(index);
        }
        repository.save(FIRST + u32::from(index), &account).unwrap();
    }
    let clans = [
        (7, "Skhid", "SKH", vec![0, 1, 4]),
        (9, "Radianty", "RAD", vec![12, 3, 5]),
    ];
    for (id, name, tag, members) in &clans {
        repository
            .save_clan(&Clan {
                id: *id,
                name: (*name).to_owned(),
                tag: (*tag).to_owned(),
                registered: true,
                leader: FIRST + members[0],
                ranks: vec![Rank {
                    name: "st_clan_rank_commander".to_owned(),
                    permissions: u8::MAX,
                    is_default: true,
                }],
                members: members
                    .iter()
                    .map(|&index| Member {
                        account_id: FIRST + index,
                        rank: 0,
                        status: status::ACCEPTED,
                    })
                    .collect(),
                daily_message: String::new(),
                experience: 60_000,
                money: 0,
            })
            .unwrap();
    }
    let clan_of = |index: u32| {
        clans
            .iter()
            .find(|(_, _, _, members)| members.contains(&index))
            .map(|(id, ..)| *id)
    };
    let guns = ["ak_74u", "uzi"].map(dict_id);
    let armour = ["bm_helmet_1", "bm_torso_4"].map(dict_id);
    // No item of the game data.
    const GONE: u16 = 65_000;
    let maps: Vec<_> = survarium::v026g::maps::playable().collect();
    let map_of = |mode: GameMode, nth: usize| {
        let of_mode: Vec<_> = maps.iter().filter(|map| map.mode == mode).collect();
        of_mode[nth % of_mode.len()].id
    };
    let mut seed: u64 = 26;
    let mut random = |below: u64| {
        seed = (seed * 1_103_515_245 + 12_345) % 2_147_483_648;
        (seed / 65_536) % below
    };
    for i in 0..23u32 {
        let free_for_all = i == 2;
        let winner = match i {
            2 => Winner::MostKills,
            4 => Winner::Draw,
            _ if random(2) == 0 => Winner::FirstTeam,
            _ => Winner::SecondTeam,
        };
        let mode = if free_for_all {
            GameMode::FreeForAll
        } else {
            GameMode::Batteries
        };
        // The free-for-all was a Racket.
        let called = if free_for_all {
            LobbyMode::Racket
        } else {
            LobbyMode::Plain(mode)
        };
        let mut players: Vec<PlayerRecord> = (0..12u32)
            .filter(|&k| k != 11 || i >= 8)
            .map(|k| {
                let team = if free_for_all {
                    survarium::v026g::maps::free_for_all_team(k as usize)
                } else {
                    (k % 2) as u8
                };
                let (kills, deaths) = (random(18), 1 + random(12));
                let shots = if k == 11 {
                    3 * kills + 5
                } else {
                    22 * kills + 15 * random(9) + 20
                };
                let hits = shots * (18 + 3 * random(7)) / 100;
                let headshots = random(kills.max(1));
                let mut kits = vec![KitLine {
                    kit: 0,
                    counts: KitCounts {
                        kills,
                        headshots,
                        shots,
                        hits,
                    },
                    items: [
                        (0, armour[0]),
                        (2, armour[1]),
                        (7, if (i + k) % 9 == 0 { GONE } else { guns[(k % 2) as usize] }),
                    ]
                    .into_iter()
                    .chain((k % 3 == 0).then_some((10, guns[1])))
                    .map(|(slot, dict_id)| KitItem {
                        slot: GearSlot::of(slot).unwrap(),
                        dict_id,
                    })
                    .collect(),
                }];
                if (i + k) % 4 == 0 {
                    // A second kit took half of it.
                    let first = &mut kits[0].counts;
                    let half = KitCounts {
                        kills: first.kills / 2,
                        headshots: first.headshots / 2,
                        shots: first.shots / 4,
                        hits: first.hits / 4,
                    };
                    first.kills -= half.kills;
                    first.headshots -= half.headshots;
                    first.shots -= half.shots;
                    first.hits -= half.hits;
                    kits.push(KitLine {
                        kit: 2,
                        counts: half,
                        items: vec![KitItem {
                            slot: GearSlot::of(7).unwrap(),
                            dict_id: guns[((k + 1) % 2) as usize],
                        }],
                    });
                }
                PlayerRecord {
                    account_id: FIRST + k,
                    clan_id: clan_of(k),
                    line: PlayerLine {
                        team,
                        place: 0,
                        seat: k as u8,
                        presence: if k == 10 && i % 5 == 0 {
                            Presence::Left
                        } else {
                            Presence::Finished
                        },
                        outcome: Outcome::Neither,
                        rating: 1180 - 20 * k as u16,
                        score: 60 + 25 * kills + random(90),
                        kills,
                        deaths,
                        counters: Counters {
                            assists: random(6),
                            headshots,
                            grenade_kills: random(2),
                            melee_kills: u64::from(random(10) == 0),
                            batteries: if free_for_all { 0 } else { random(4) },
                            capture_hundredths: 0,
                            artefacts: u64::from((7 * i + 3 * k) % 4),
                            shots,
                            hits,
                        },
                        played_ms: if k == 10 && i % 5 == 0 { 300_000 } else { 780_000 },
                    },
                    kits,
                }
            })
            .collect();
        // Places and outcomes as `rewards` says them.
        let top_kills = players.iter().map(|player| player.line.kills).max();
        let lines: Vec<PlayerLine> = players.iter().map(|player| player.line).collect();
        for player in &mut players {
            let me = player.line;
            let rank = |line: &PlayerLine| {
                if free_for_all {
                    (line.kills, line.score)
                } else {
                    (line.score, 0)
                }
            };
            let ahead = lines.iter().filter(|them| {
                (free_for_all || them.team == me.team)
                    && (rank(them), me.seat) > (rank(&me), them.seat)
            });
            player.line.place = ahead.count() as u8 + 1;
            let won = me.presence == Presence::Finished
                && match winner {
                    Winner::FirstTeam => me.team == 0,
                    Winner::SecondTeam => me.team == 1,
                    Winner::MostKills => Some(me.kills) == top_kills,
                    Winner::Draw | Winner::Unknown => false,
                };
            player.line.outcome = match winner {
                _ if won => Outcome::Won,
                Winner::Draw | Winner::Unknown => Outcome::Neither,
                _ => Outcome::Lost,
            };
        }
        let ended_at = now - u64::from(22 - i) * 5_400;
        let record = MatchRecord {
            replay_id: ReplayId::new(
                0x5E1A_0000 + i,
                std::time::UNIX_EPOCH + std::time::Duration::from_secs(ended_at - 900),
            ),
            kind: MatchKind::Public(if i == 6 {
                Eligibility::Short
            } else {
                Eligibility::Counted
            }),
            info: MatchInfo {
                ended_at,
                duration_ms: 600_000 + random(600) as u32 * 1_000,
                map_id: map_of(mode, i as usize),
                mode: called,
                match_level: 12,
                winner,
                team_points: match winner {
                    Winner::FirstTeam => [5, 2 + random(3) as u16],
                    Winner::SecondTeam => [1 + random(4) as u16, 5],
                    Winner::Draw => [3, 3],
                    Winner::MostKills | Winner::Unknown => [0, 0],
                },
            },
            players,
        };
        repository.record_match(&record).unwrap();
    }
    GameAccounts::with_repository(Box::new(repository)).unwrap()
}

/// The preview's replays: under `dir`, the newest match's made (a file of a
/// few bytes, no game's), the one before it a kept recording nobody
/// converts (asked for, it stays `preparing`), the others none.
pub(crate) fn preview_replays(dir: &std::path::Path) -> Replays {
    use session::v026g::replays::{ReplaySettings, Sidecar, oldest_replayable_here};
    let settings = ReplaySettings {
        record_dir: dir.join("recordings"),
        replay_dir: dir.join("replays"),
        max_recording_bytes: 1 << 20,
        max_replay_bytes: 1 << 20,
    };
    std::fs::create_dir_all(&settings.record_dir).unwrap();
    std::fs::create_dir_all(&settings.replay_dir).unwrap();
    let replays = Replays::new(Some(settings), 6);
    let (made, kept) = (0x5E1A_0000 + 22, 0x5E1A_0000 + 21);
    let bytes = b"a made-up replay of the preview";
    std::fs::write(replays.sur_path(made).unwrap(), bytes).unwrap();
    let sidecar = Sidecar {
        match_id: made,
        size: bytes.len() as u64,
        sha256: "0".repeat(64),
        revisions: vec![Some(oldest_replayable_here(0))],
        account_ids: Vec::new(),
        made_at: 0,
    };
    std::fs::write(replays.json_path(made).unwrap(), sidecar.to_json()).unwrap();
    let recording = replays.settings().unwrap().record_dir.join(format!("rec026g_20261120-120000_{kept:08x}.srprec"));
    std::fs::write(recording, b"x").unwrap();
    replays.recording_kept(kept);
    replays
}

/// A match says whether it has a replay to ask for; the ask gives the
/// site's token of a made one, queues a kept recording, and knows no other.
#[test]
fn a_matchs_replay_is_asked_for_by_anyone() {
    let dir = std::env::temp_dir().join(format!("srp-web-history-replay-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let accounts = preview(&dir, NOW);
    let replays = preview_replays(&dir);
    let ask = |target: &str, replays: Option<&Replays>| {
        let (path, query) = target.split_once('?').unwrap();
        let endpoint = Endpoint::parse(path, query).unwrap();
        answer(&endpoint, accounts.history().unwrap(), replays, &|_| false, Lang::En, NOW)
    };
    let ok = |target: &str| ask(target, Some(&replays)).unwrap();
    assert_eq!(ok("/api/history/match?id=23")["replay"], true);
    assert_eq!(ok("/api/history/match?id=22")["replay"], true);
    assert_eq!(ok("/api/history/match?id=21")["replay"], false);
    let ready = ok("/api/history/replay?id=23");
    assert_eq!(keys(&ready), ["sha256", "size", "status", "url"]);
    assert_eq!(ready["status"], "ready");
    let url = ready["url"].as_str().unwrap();
    assert!(url.starts_with("/replays/") && url.len() == "/replays/".len() + 32);
    assert_eq!(ok("/api/history/replay?id=23")["url"], url, "the site's one token of the match");
    assert_eq!(ok("/api/history/replay?id=22"), json!({ "status": "preparing", "place": 1 }));
    assert_eq!(ok("/api/history/replay?id=21"), json!({ "status": "gone" }));
    assert!(matches!(ask("/api/history/replay?id=99", Some(&replays)), Err(Refusal::NotFound)));
    // A server without replays: none offered, and the ask says so.
    assert_eq!(ask("/api/history/match?id=23", None).unwrap()["replay"], false);
    assert_eq!(ask("/api/history/replay?id=23", None).unwrap(), json!({ "status": "off" }));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The preview's history is one the API serves whole.
#[test]
fn the_previews_history_is_served() {
    let dir = std::env::temp_dir().join(format!(
        "srp-web-history-preview-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let accounts = preview(&dir, NOW);
    let history = accounts.history().unwrap();
    let ask = |target: &str| {
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        let endpoint = Endpoint::parse(path, query).unwrap();
        answer(&endpoint, history, None, &|_| false, Lang::En, NOW).unwrap()
    };
    assert_eq!(ask("/api/history/top")["players"].as_array().unwrap().len(), 12);
    assert_eq!(ask("/api/history/matches?limit=50")["matches"].as_array().unwrap().len(), 23);
    let free = ask("/api/history/match?id=3");
    assert_eq!(free["match"]["winner"], "free_for_all");
    let places: Vec<u64> = free["players"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["place"].as_u64().unwrap())
        .collect();
    let mut sorted = places.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, (1..=11).collect::<Vec<u64>>(), "placed among everyone");
    assert_eq!(ask("/api/history/match?id=7")["match"]["eligibility"], "short");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The match page paints its teams as the Maps page does: the same two
/// colours, held together here.
#[test]
fn the_teams_colours_are_the_maps_pages() {
    let colour = |text: &'static str, name: &str| -> &'static str {
        let from = text.find(&format!("{name}:")).unwrap_or_else(|| panic!("{name}"));
        let value = text[from + name.len() + 1..].trim_start();
        &value[..7]
    };
    let maps = include_str!("../../maps/maps.html");
    for team in ["--team0", "--team1"] {
        assert_eq!(colour(STYLE, team), colour(maps, team), "{team}");
        assert!(colour(STYLE, team).starts_with('#'));
    }
}

/// Every map a match can be played on has the rooms' picture of it, and the
/// history gives its address.
#[test]
fn a_match_has_its_maps_picture_as_the_rooms_draw_it() {
    for map in survarium::v026g::maps::playable() {
        let key = format!("{}_{}", map.level, map.weather);
        let url = icons::url(IconSet::Map, &key).unwrap_or_else(|| panic!("no picture {key}"));
        let path = url.split_once("?v=").unwrap().0;
        assert!(icons::file(path).is_some_and(|(_, bytes)| !bytes.is_empty()), "{path}");
    }
    let dir = std::env::temp_dir().join(format!("srp-web-history-map-picture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let accounts = preview(&dir, NOW);
    let endpoint = Endpoint::parse("/api/history/matches", "limit=50").unwrap();
    let answer = answer(&endpoint, accounts.history().unwrap(), None, &|_| false, Lang::En, NOW).unwrap();
    for played in answer["matches"].as_array().unwrap() {
        assert!(played["map_icon"].as_str().unwrap().starts_with("/wiki/icons/maps/"));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

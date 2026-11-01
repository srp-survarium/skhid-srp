//! The 0.26g page against a store and 0.26g accounts in memory, with a
//! lobby link that records what it is told.

use std::sync::atomic::AtomicU32;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use session::v026g::GameAccounts;
use session::{LockExt, Net, SessionStore, SignInError};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lobby::FULL_CONDITION;

use super::{Game, LobbyLink, level_start};
use crate::{Lang, Request, Web};

/// SHA-1 as the 0.26g client sends a password: 40 lower-case hex
/// characters (what the login server keeps the argon2id hash of).
fn sha1_hex(bytes: &[u8]) -> Vec<u8> {
    let digest: String = openssl::sha::sha1(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert!(session::password::is_sha1_digest(digest.as_bytes()));
    digest.into_bytes()
}

#[derive(Default)]
struct Told {
    changed: Mutex<Vec<u32>>,
    text: Mutex<Option<(String, String)>>,
    cleared: AtomicU32,
    matches: AtomicU32,
    live: Mutex<Vec<super::LiveMatch>>,
}

impl super::MatchCounter for Told {
    fn active_matches(&self) -> usize {
        self.matches.load(std::sync::atomic::Ordering::Relaxed) as usize
    }
    fn live_matches(&self) -> Vec<super::LiveMatch> {
        self.live.locked().clone()
    }
}

impl LobbyLink for Told {
    fn account_changed(&self, account_id: u32) {
        self.changed.locked().push(account_id);
    }
    fn set_admin_text(&self, first: &str, second: &str) {
        *self.text.locked() = Some((first.to_owned(), second.to_owned()));
    }
    fn clear_admin_text(&self) {
        *self.text.locked() = None;
        self.cleared
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn admin_text(&self) -> Option<(String, String)> {
        self.text.locked().clone()
    }
}

struct World {
    web: Web,
    accounts: Arc<GameAccounts>,
    told: Arc<Told>,
}

fn world() -> World {
    world_with(GameAccounts::new())
}

/// The world with these 0.26g accounts (one kept in a repository: what is
/// written, and the audit trail).
fn world_with(accounts: GameAccounts) -> World {
    let accounts = Arc::new(accounts);
    let told = Arc::new(Told::default());
    let web = Web {
        store: Arc::new(SessionStore::new()),
        token: Some("tok".into()),
        admin_path: Some("/hidden".into()),
        admin_ips: vec![Net::parse("127.0.0.1").unwrap()],
        trust_proxy: false,
        token_failures: Mutex::new((0, None)),
        connections: AtomicU32::new(0),
        downloads: AtomicU32::new(0),
        resource_downloads: AtomicU32::new(0),
        replay_downloads: AtomicU32::new(0),
        replay_failures: Mutex::new(std::collections::HashMap::new()),
        plans: Default::default(),
        game: Some(Game {
            accounts: accounts.clone(),
            lobby: told.clone(),
            matches: told.clone(),
            downloads: Arc::new(crate::Downloads::new(Vec::new(), None, None)),
            min_patch_revision: 0,
            replays: None,
        }),
    };
    World {
        web,
        accounts,
        told,
    }
}

impl World {
    /// An account as the 0.26g login server registers it (the digest of
    /// `password`), with its 0.26g state as the lobby makes it. Returns its
    /// id.
    fn player(&self, name: &str, password: &str) -> u32 {
        let id = self
            .web
            .store
            .sign_in_with_digest_from(
                name,
                &sha1_hex(password.as_bytes()),
                "127.0.0.1".parse().unwrap(),
                Instant::now(),
            )
            .unwrap()
            .account_id();
        self.accounts.get_or_create(id, name);
        id
    }

    fn balances(&self, id: u32) -> (u32, u32) {
        let account = self.accounts.get(id).unwrap();
        let account = account.locked();
        (account.generic_money, account.premium_money)
    }

    fn admin(&self, fields: &[(&str, &str)]) -> String {
        let mut all = vec![("token", "tok")];
        all.extend_from_slice(fields);
        self.web.route(&post("/hidden", &all), "t").body
    }
}

fn post(path: &str, form: &[(&str, &str)]) -> Request {
    Request {
        method: "POST".into(),
        path: path.into(),
        target: path.into(),
        lang: Lang::En,
        forwarded: String::new(),
        range: String::new(),
        client: Some("127.0.0.1".parse().unwrap()),
        form: form
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
    }
}

fn get(path: &str) -> Request {
    Request {
        method: "GET".into(),
        ..post(path, &[])
    }
}

#[test]
fn the_exchange_is_in_the_game_now() {
    let world = world();
    let id = world.player("fighter", "secret");
    let (silver, gold) = world.balances(id);
    // The launcher's SHOP button: `/exchange?lang=<its code>`.
    let shop = |code: &str| {
        let mut request = get("/exchange");
        request.target = format!("/exchange?lang={code}");
        request.lang = Lang::pick(code, "");
        world.web.route(&request, "t")
    };
    for (code, lang, text) in [
        ("en", "en", "Silver and gold are exchanged in the game now"),
        ("ua", "uk", "Срібло й золото тепер обмінюються в грі"),
        ("ru", "ru", "Серебро и золото теперь обмениваются в игре"),
        (
            "de",
            "de",
            "Silber und Gold werden jetzt im Spiel getauscht",
        ),
        ("pl", "pl", "Srebro i złoto wymienia się teraz w grze"),
    ] {
        let answer = shop(code);
        assert_eq!(answer.status, "200 OK", "{code}");
        assert!(
            answer
                .body
                .contains(&format!("<section class=card lang={lang}>")),
            "{code}"
        );
        assert!(answer.body.contains(text), "{code}: {}", answer.body);
        assert!(answer.body.contains("SURVARIUM 0.26G") && !answer.body.contains("<form"));
    }
    // Without a code: the page's own language.
    let mut browser = get("/exchange");
    browser.lang = Lang::Uk;
    assert!(world.web.route(&browser, "t").body.contains("Тепер у грі"));
    // A form sent as the old page sent it is a page that is not there, and
    // changes nothing.
    let sent = world.web.route(
        &post(
            "/exchange",
            &[("user", "fighter"), ("password", "secret"), ("gold", "1")],
        ),
        "t",
    );
    assert_eq!(sent.status, "404 Not Found");
    assert_eq!(world.balances(id), (silver, gold));
    assert!(world.told.changed.locked().is_empty());
}

#[test]
fn the_admin_changes_money_and_the_account() {
    let world = world();
    let id = world.player("tester", "secret");
    let (silver, gold) = world.balances(id);
    let refused = world.web.route(
        &post(
            "/hidden",
            &[("token", "bad"), ("user", "tester"), ("gold", "+9")],
        ),
        "t",
    );
    assert!(refused.body.contains("Wrong token"));
    assert_eq!(world.balances(id), (silver, gold));

    // Money is moderation's: a preview, then the confirm (`moderation`).
    let asked = [
        ("user", "tester"),
        ("gold", "+500"),
        ("silver", "-999999999"),
        ("money_reason", "test"),
    ];
    let preview = world.admin(&asked);
    assert_eq!(world.balances(id), (silver, gold));
    let plan = hidden(&preview, "money_plan");
    let mut confirm = asked.to_vec();
    confirm.extend([("money_plan", plan.as_str()), ("confirm", "1")]);
    let done = world.admin(&confirm);
    assert!(done.contains("tester: silver 2 000 → 0 (-2 000)"), "{done}");
    assert_eq!(world.balances(id), (0, gold + 500));
    assert_eq!(world.told.changed.locked()[..], [id]);

    // Premium, a level with experience on top, reputation.
    let done = world.admin(&[
        ("tab", "account"),
        ("acc_user", "tester"),
        ("premium_hours", "+24"),
        ("level", "3"),
        ("experience", "+10"),
        ("faction", "2"),
        ("reputation", "+150"),
    ]);
    assert!(done.contains("level 3"), "{done}");
    assert!(done.contains("24 h of premium"), "{done}");
    assert!(done.contains("Reputation with faction 2: 150"), "{done}");
    assert!(done.contains("value=account id=tab-account checked"));
    {
        let account = world.accounts.get(id).unwrap();
        let account = account.locked();
        assert_eq!(account.level(), 3);
        assert_eq!(account.exp_total, level_start(3) + 10);
        assert_eq!(account.premium_hours(crate::unix_now()), Some(24));
        assert_eq!(account.reputation(2), 150);
    }
    // Taken back; the premium ends.
    let done = world.admin(&[
        ("tab", "account"),
        ("acc_user", "tester"),
        ("premium_hours", "-48"),
        ("faction", "2"),
        ("reputation", "-1000"),
    ]);
    assert!(done.contains("0 h of premium"), "{done}");
    {
        let account = world.accounts.get(id).unwrap();
        let account = account.locked();
        assert_eq!(account.premium_until, 0);
        assert_eq!(account.reputation(2), 0);
        assert_eq!(account.level(), 3);
    }
    // Wrong fields change nothing.
    for fields in [
        // One past the highest level, 100.
        [("level", "101"), ("faction", "")],
        [("level", ""), ("faction", "9")],
        [("experience", "x"), ("faction", "")],
        [("reputation", "+5"), ("faction", "")],
    ] {
        let mut all = vec![("tab", "account"), ("acc_user", "tester")];
        all.extend_from_slice(&fields);
        assert!(world.admin(&all).contains("whole numbers"), "{fields:?}");
    }
    assert!(
        world
            .admin(&[("tab", "account"), ("acc_user", "ghost"), ("level", "1")])
            .contains("No such user")
    );
    assert_eq!(world.told.changed.locked().len(), 3);
}

#[test]
fn a_level_under_the_spent_skill_points_is_refused() {
    let world = world();
    let id = world.player("skilled", "secret");
    {
        let account = world.accounts.get(id).unwrap();
        let mut account = account.locked();
        account.exp_total = level_start(5);
        account.skills = vec![(1, 4)];
    }
    let refused = world.admin(&[("tab", "account"), ("acc_user", "skilled"), ("level", "3")]);
    assert!(refused.contains("skill points"), "{refused}");
    assert_eq!(world.accounts.get(id).unwrap().locked().level(), 5);
    assert!(world.told.changed.locked().is_empty());
    let done = world.admin(&[("tab", "account"), ("acc_user", "skilled"), ("level", "4")]);
    assert!(done.contains("level 4"), "{done}");
}

#[test]
fn the_admin_gives_items_of_the_dictionary_only() {
    let world = world();
    let id = world.player("tester", "secret");
    let dictionary = Dictionary::shared();
    let owned = |dict_id: u16| -> Vec<u32> {
        let account = world.accounts.get(id).unwrap();
        let account = account.locked();
        account
            .inventory
            .iter()
            .filter(|item| item.dict_id == dict_id)
            .map(|item| item.condition_or_stack)
            .collect()
    };
    // The starter kit's pistol and its ammo are in the dictionary: one that
    // is no stack, one that is.
    let (weapon, ammo) = {
        let account = world.accounts.get(id).unwrap();
        let account = account.locked();
        let pick = |stack: bool| {
            account
                .inventory
                .iter()
                .map(|item| item.dict_id)
                .find(|dict_id| {
                    dictionary
                        .item(*dict_id)
                        .is_some_and(|item| item.is_stack == stack)
                })
                .expect("the starter loadout has a weapon and its ammo")
        };
        (pick(false), pick(true))
    };
    let had = (owned(weapon).len(), owned(ammo)[0]);

    let by_id = world.admin(&[
        ("tab", "item"),
        ("item_user", "tester"),
        ("item", &weapon.to_string()),
        ("count", "2"),
    ]);
    assert!(by_id.contains("given 2 of"), "{by_id}");
    let weapons = owned(weapon);
    assert_eq!(weapons.len(), had.0 + 2);
    assert_eq!(weapons[weapons.len() - 1], FULL_CONDITION);

    // By name; a stack grows, it is not a second instance.
    let name = &dictionary.item(ammo).unwrap().name;
    let by_name = world.admin(&[
        ("tab", "item"),
        ("item_user", "tester"),
        ("item", name),
        ("count", "30"),
    ]);
    assert!(by_name.contains("given 30 of"), "{by_name}");
    assert_eq!(owned(ammo), [had.1 + 30]);
    assert_eq!(world.told.changed.locked().len(), 2);

    // An id the dictionary has not (the client would crash on it), and
    // counts out of range.
    let unknown = (1..u16::MAX)
        .find(|id| dictionary.item(*id).is_none())
        .unwrap();
    for (item, count, answer) in [
        (unknown.to_string(), "1", "No such item"),
        ("no_such_item".to_owned(), "1", "No such item"),
        (weapon.to_string(), "9", "count for this item is 1 to 8"),
        (weapon.to_string(), "0", "count for this item"),
        (
            ammo.to_string(),
            "100001",
            "count for this item is 1 to 100000",
        ),
    ] {
        let body = world.admin(&[
            ("tab", "item"),
            ("item_user", "tester"),
            ("item", &item),
            ("count", count),
        ]);
        assert!(body.contains(answer), "{item} x{count}: {body}");
    }
    assert_eq!(world.told.changed.locked().len(), 2);
    // Every id is still one the dictionary has, and the account is whole.
    let account = world.accounts.get(id).unwrap();
    let mut account = account.locked();
    assert!(
        account
            .inventory
            .iter()
            .all(|item| dictionary.item(item.dict_id).is_some())
    );
    assert!(!account.repair());
}

#[test]
fn the_admin_sets_and_clears_the_text_for_everyone() {
    let world = world();
    let shown = world.admin(&[
        ("tab", "text"),
        ("line1", "Restart at 20:00"),
        ("line2", "<b>"),
    ]);
    assert!(shown.contains("shown to everyone"), "{shown}");
    assert_eq!(
        world.told.admin_text(),
        Some(("Restart at 20:00".to_owned(), "<b>".to_owned()))
    );
    // The page shows what stands, escaped.
    assert!(shown.contains("Shown now:") && shown.contains("&lt;b&gt;"));
    // Too long for the client's buffers (a Cyrillic letter is 2 bytes).
    let long = "я".repeat(32);
    let refused = world.admin(&[("tab", "text"), ("line1", &long)]);
    assert!(refused.contains("at most 63 bytes"), "{refused}");
    assert_eq!(world.told.admin_text().unwrap().0, "Restart at 20:00");
    let cleared = world.admin(&[("tab", "text"), ("clear_text", "1")]);
    assert!(cleared.contains("The text is cleared"), "{cleared}");
    assert_eq!(world.told.admin_text(), None);
    // Without the token nothing is set.
    let refused = world.web.route(
        &post(
            "/hidden",
            &[("token", "bad"), ("tab", "text"), ("line1", "x")],
        ),
        "t",
    );
    assert!(refused.body.contains("Wrong token"));
    assert_eq!(world.told.admin_text(), None);
}

#[test]
fn bans_maintenance_and_registration_are_the_stores() {
    let world = world();
    let id = world.player("cheater", "secret");
    let session = world.web.store.create_session(id);
    let banned = world.admin(&[
        ("tab", "ban"),
        ("ban_user", "cheater"),
        ("hours", "2"),
        ("reason", "test"),
    ]);
    assert!(
        banned.contains("cheater banned for 2 h; 1 session(s) ended"),
        "{banned}"
    );
    // The session the 0.26g lobby holds has ended (its link is signed out
    // on the lobby's next pass), and the 0.26g sign-in is refused.
    assert!(world.web.store.has_ended(session));
    let again = world.web.store.sign_in_with_digest_from(
        "cheater",
        &sha1_hex(b"secret"),
        "127.0.0.1".parse().unwrap(),
        Instant::now(),
    );
    assert_eq!(again, Err(SignInError::Banned));
    world.admin(&[("tab", "unban"), ("unban_user", "cheater")]);
    assert!(world.web.store.account_ban(id).is_none());

    world.admin(&[("tab", "maint"), ("minutes", "45")]);
    assert_eq!(world.web.store.minutes_to_maintenance(), Some(45));
    world.admin(&[("tab", "signups"), ("signups", "closed")]);
    let refused = world.web.store.sign_in_with_digest_from(
        "late",
        &sha1_hex(b"secret"),
        "127.0.0.1".parse().unwrap(),
        Instant::now(),
    );
    assert_eq!(refused, Err(SignInError::RegistrationsClosed));
}

#[test]
fn the_pages_are_the_026g_ones() {
    let world = world();
    let home = world.web.route(&get("/"), "t").body;
    // The exe has the server and memory built in and its language in Settings.
    assert!(!home.contains("-localization") && !home.contains("-mt_allocator"));
    assert!(!home.contains("-client") && !home.contains("0.21d") && !home.contains("0.21D"));
    assert!(home.contains("Survarium 0.26g client"));
    assert!(!home.contains("/exchange"));
    let admin = world.web.route(&get("/hidden"), "t");
    assert_eq!(admin.status, "200 OK");
    for tab in [
        "money", "ban", "unban", "bans", "maint", "signups", "account", "item", "items", "text",
    ] {
        assert!(admin.body.contains(&format!("id=tab-{tab}")), "{tab}");
    }
    let missing = world.web.route(&get("/nothing"), "t");
    assert_eq!(missing.status, "404 Not Found");
    assert!(missing.body.contains("0.26G"));
    let mut elsewhere = get("/hidden");
    elsewhere.client = "203.0.113.5".parse().ok();
    assert_eq!(world.web.route(&elsewhere, "t").status, "404 Not Found");
}

#[test]
fn public_stats_count_players_rooms_and_hosted_matches() {
    let mut world = world();
    // The endpoint is public even with the admin disabled.
    world.web.token = None;
    world.web.admin_path = None;
    let stats = || world.web.route(&get("/api/stats"), "t");
    assert_eq!(
        stats().body,
        r#"{"online":0,"registered":0,"matches":0,"rooms":0,"in_rooms":0,"queued":0,"last_match_ends_in":0}"#
    );
    let account = world.player("player-a", "test-password");
    let session = world.web.store.create_session(account);
    let hold = world.web.store.hold(session).unwrap();
    let second_session = world.web.store.create_session(account);
    let second_hold = world.web.store.hold(second_session).unwrap();
    // Multiple connections for one account count once.
    assert_eq!(
        stats().body,
        r#"{"online":1,"registered":1,"matches":0,"rooms":0,"in_rooms":0,"queued":0,"last_match_ends_in":0}"#
    );
    // The rooms as the lobby last counted them.
    world.web.store.set_room_counts(session::RoomCounts {
        rooms: 2,
        in_rooms: 3,
        queued: 1,
    });
    world
        .told
        .matches
        .store(2, std::sync::atomic::Ordering::Relaxed);
    let response = stats();
    assert_eq!(response.status, "200 OK");
    assert_eq!(response.content_type.as_str(), "application/json");
    assert_eq!(
        response.body,
        r#"{"online":1,"registered":1,"matches":2,"rooms":2,"in_rooms":3,"queued":1,"last_match_ends_in":0}"#
    );
    assert_eq!(world.web.store.match_count(), 0); // Use the host, not formed rosters.
    drop(hold);
    drop(second_hold);
    assert_eq!(
        stats().body,
        r#"{"online":0,"registered":1,"matches":2,"rooms":2,"in_rooms":3,"queued":1,"last_match_ends_in":0}"#
    );
    assert_eq!(
        world.web.route(&post("/api/stats", &[]), "t").status,
        "404 Not Found"
    );
}

#[test]
fn public_stats_say_when_the_last_match_ends_at_the_latest() {
    use std::time::Duration;
    let world = world();
    let stats = || world.web.route(&get("/api/stats"), "t").body;
    let live = |phase, time_left: Option<Duration>| crate::LiveMatch {
        map_id: 117,
        phase,
        players: [4, 3],
        played: None,
        time_left,
        standing: None,
    };
    world
        .told
        .matches
        .store(3, std::sync::atomic::Ordering::Relaxed);
    // The longest time left of those in play, in whole seconds rounded up:
    // the launcher's "Time left".
    *world.told.live.locked() = vec![
        live(crate::LivePhase::Playing, Some(Duration::from_secs(120))),
        live(
            crate::LivePhase::Playing,
            Some(Duration::from_millis(299_200)),
        ),
        live(crate::LivePhase::Countdown, Some(Duration::from_secs(200))),
    ];
    assert_eq!(
        stats(),
        r#"{"online":0,"registered":0,"matches":3,"rooms":0,"in_rooms":0,"queued":0,"last_match_ends_in":300}"#
    );
    // One that has its result is about to go; one still loading has its
    // whole wait, countdown and match time ahead (5 + 20 + 900 s).
    *world.told.live.locked() = vec![
        live(crate::LivePhase::Over, Some(Duration::ZERO)),
        live(crate::LivePhase::Playing, Some(Duration::from_secs(60))),
    ];
    assert!(stats().ends_with(r#""last_match_ends_in":60}"#));
    world
        .told
        .live
        .locked()
        .push(live(crate::LivePhase::Gathering, None));
    assert!(stats().ends_with(r#""last_match_ends_in":925}"#));
    // A map the client does not have: the time every playable mode has.
    *world.told.live.locked() = vec![crate::LiveMatch {
        map_id: 0,
        ..live(crate::LivePhase::Gathering, None)
    }];
    assert!(stats().ends_with(r#""last_match_ends_in":925}"#));
    // Only an ended one: nothing to wait for.
    *world.told.live.locked() = vec![live(crate::LivePhase::Over, None)];
    assert_eq!(
        stats(),
        r#"{"online":0,"registered":0,"matches":3,"rooms":0,"in_rooms":0,"queued":0,"last_match_ends_in":0}"#
    );
}

#[test]
fn public_stats_http_has_json_headers_and_ignores_language() {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;
    let world = world();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        world.web.serve(stream);
    });
    let mut client = TcpStream::connect(address).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client
        .write_all(b"GET /api/stats?lang=uk HTTP/1.0\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    server.join().unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(headers.starts_with("HTTP/1.0 200 OK\r\n"));
    assert!(headers.contains("\r\nContent-Type: application/json\r\n"));
    assert!(headers.contains("\r\nCache-Control: no-store\r\n"));
    assert!(headers.contains(&format!("\r\nContent-Length: {}\r\n", body.len())));
    assert_eq!(
        body,
        r#"{"online":0,"registered":0,"matches":0,"rooms":0,"in_rooms":0,"queued":0,"last_match_ends_in":0}"#
    );
}

#[test]
fn the_launchers_page_and_its_fresh_numbers_come_with_the_launchers_policy() {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;
    let world = world();
    world
        .told
        .matches
        .store(3, std::sync::atomic::Ordering::Relaxed);
    // A Team Deathmatch on Rudnya in play.
    *world.told.live.locked() = vec![crate::LiveMatch {
        map_id: 117,
        phase: crate::LivePhase::Playing,
        players: [4, 3],
        played: Some(Duration::from_secs(130)),
        time_left: Some(Duration::from_secs(770)),
        standing: Some(crate::LiveStanding {
            points: [96, 91],
            leader: Some(0),
            age: Duration::from_secs(3),
        }),
    }];
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let targets = [
        "/launcher/v2/survarium/?lang=ua",
        "/launcher/places/uk",
        "/launcher/places/en",
        "/",
        "/launcher/fresh/uk",
    ];
    let server = std::thread::spawn(move || {
        for _ in targets {
            let (stream, _) = listener.accept().unwrap();
            world.web.serve(stream);
        }
    });
    let answers: Vec<(String, String)> = targets
        .iter()
        .map(|target| {
            let mut client = TcpStream::connect(address).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            client
                .write_all(format!("GET {target} HTTP/1.0\r\n\r\n").as_bytes())
                .unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap();
            let (headers, body) = response.split_once("\r\n\r\n").unwrap();
            assert!(headers.starts_with("HTTP/1.0 200 OK\r\n"), "{target}");
            assert!(
                headers.contains("\r\nCache-Control: no-store\r\n"),
                "{target}"
            );
            (headers.to_owned(), body.to_owned())
        })
        .collect();
    server.join().unwrap();
    let launchers = format!(
        "\r\nContent-Security-Policy: {}\r\n",
        crate::launcher::policy()
    );
    let (page, fresh_uk, fresh_en, home) = (&answers[0], &answers[1], &answers[2], &answers[3]);
    assert!(page.0.contains(&launchers));
    assert!(page.1.contains("<script type=\"text/javascript\">"));
    assert!(page.1.contains("<b>3</b><span>матчів зараз</span>"));
    let score = "<td class=e>&le; 13 хв</td><td class=c><b>96</b> : 91</td>";
    assert!(page.1.contains(score));
    // The fresh numbers are the four places of that page, each behind its
    // name, and nothing more.
    for (fresh, matches) in [(fresh_uk, "матчів зараз"), (fresh_en, "matches now")] {
        assert!(fresh.0.contains(&launchers));
        assert!(
            fresh
                .0
                .contains("\r\nContent-Type: text/html; charset=utf-8\r\n")
        );
        let parts: Vec<&str> = fresh.1.split("<hr>").collect();
        assert_eq!(parts.len(), 8);
        assert_eq!(
            [parts[0], parts[2], parts[4], parts[6]],
            ["board", "live", "now", "totals"]
        );
        assert!(parts[5].contains(&format!("<b>3</b><span>{matches}</span>")));
        for banned in ["<script", "<style", "<html", "<body"] {
            assert!(!fresh.1.contains(banned), "{banned}");
        }
    }
    let parts: Vec<&str> = fresh_uk.1.split("<hr>").collect();
    assert!(parts[3].contains(score));
    for part in &parts {
        assert!(page.1.contains(part));
    }
    // A page opened before the matches' tab asks where it always did, and
    // is given its three places in its order.
    let before = &answers[4];
    assert!(before.0.contains(&launchers));
    assert_eq!(
        before.1.split("<hr>").collect::<Vec<_>>(),
        [parts[1], parts[5], parts[7]]
    );
    // Every other page keeps the site's policy: no script at all.
    assert!(home.0.contains(&format!(
        "\r\nContent-Security-Policy: {}\r\n",
        crate::POLICY
    )));
    assert!(!home.0.contains("script-src"));
}

fn offered(n: u32, sha: char, urls: &[&str]) -> crate::OfferedPatch {
    crate::OfferedPatch {
        name: format!("patch_{n}"),
        sha256: sha.to_string().repeat(64),
        size: 1,
        urls: urls.iter().map(|url| (*url).to_owned()).collect(),
    }
}

/// `GET /patches` over a socket, as the players' updater asks it: the
/// head and the body.
fn patches_over_http(web: Web, request: &'static [u8]) -> (String, String) {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        web.serve(stream);
    });
    let mut client = TcpStream::connect(address).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    client.write_all(request).unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    server.join().unwrap();
    let (head, body) = response.split_once("\r\n\r\n").unwrap();
    (head.to_owned(), body.to_owned())
}

#[test]
fn the_patches_are_listed_for_the_updater_as_json() {
    // None: an empty list, which the updater takes (a 404 would keep PLAY
    // off for everyone).
    let empty = world();
    assert_eq!(empty.web.route(&get("/patches"), "t").body, "{}");
    let (head, body) = patches_over_http(empty.web, b"GET /patches HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(head.starts_with("HTTP/1.0 200 OK\r\n"), "{head}");
    assert!(
        head.contains("\r\nContent-Type: application/json\r\n"),
        "{head}"
    );
    assert!(head.contains("\r\nCache-Control: no-store\r\n"), "{head}");
    assert_eq!(body, "{}");

    // Those of the game data, in its order, each with its sha256 and its
    // URLs in their order (`urls` always, one or several). Public: no
    // token, no admin page, any language.
    let mut world = world();
    world.web.token = None;
    world.web.admin_path = None;
    world.web.game.as_mut().unwrap().downloads = Arc::new(crate::Downloads::new(
        vec![
            offered(
                14,
                'a',
                &[
                    "http://203.0.113.5/patches/resources.db.patch_14",
                    "https://drive.usercontent.google.com/download?id=abc&export=download&confirm=t",
                ],
            ),
            offered(16, '0', &["https://files.example/db/resources.db.patch_16"]),
        ],
        None,
        None,
    ));
    let expected = format!(
        "{{\"patch_14\":{{\"sha256\":\"{}\",\"urls\":[\"http://203.0.113.5/patches/resources.db.patch_14\",\
         \"https://drive.usercontent.google.com/download?id=abc&export=download&confirm=t\"]}},\
         \"patch_16\":{{\"sha256\":\"{}\",\"urls\":[\"https://files.example/db/resources.db.patch_16\"]}}}}",
        "a".repeat(64),
        "0".repeat(64)
    );
    assert_eq!(world.web.route(&get("/patches"), "t").body, expected);
    assert_eq!(
        world.web.route(&post("/patches", &[]), "t").status,
        "404 Not Found"
    );
    let (head, body) = patches_over_http(world.web, b"GET /patches?lang=uk HTTP/1.0\r\n\r\n");
    assert!(
        head.contains("\r\nContent-Type: application/json\r\n"),
        "{head}"
    );
    assert!(head.contains(&format!("\r\nContent-Length: {}\r\n", body.len())));
    assert_eq!(body, expected);
}

/// The game's archive at `/game`, apart from the patches: a 404 when the
/// data names none (the updater then uses the addresses built into it).
#[test]
fn the_games_archive_is_offered_apart_from_the_patches() {
    let mut world = world();
    assert_eq!(world.web.route(&get("/game"), "t").status, "404 Not Found");
    let archive = crate::OfferedPatch {
        name: "survarium-026g.7z".into(),
        sha256: "7".repeat(64),
        size: 2_906_544_103,
        urls: vec![
            "http://203.0.113.5:8090/survarium-026g.7z".into(),
            "https://archive.org/download/x/survarium_full_026g0.7z".into(),
        ],
    };
    world.web.game.as_mut().unwrap().downloads = Arc::new(crate::Downloads::new(
        vec![offered(14, 'a', &["http://203.0.113.5/p14"])],
        Some(archive),
        None,
    ));
    let game = world.web.route(&get("/game"), "t");
    assert_eq!(game.status, "200 OK");
    assert_eq!(
        game.body,
        format!(
            "{{\"sha256\":\"{}\",\"size\":2906544103,\"urls\":[\"http://203.0.113.5:8090/survarium-026g.7z\",\
             \"https://archive.org/download/x/survarium_full_026g0.7z\"]}}",
            "7".repeat(64)
        )
    );
    // Not among the patches: every member there is one.
    assert_eq!(
        world.web.route(&get("/patches"), "t").body,
        format!(
            "{{\"patch_14\":{{\"sha256\":\"{}\",\"urls\":[\"http://203.0.113.5/p14\"]}}}}",
            "a".repeat(64)
        )
    );
    let (head, body) = patches_over_http(world.web, b"GET /game HTTP/1.0\r\n\r\n");
    assert!(
        head.contains("\r\nContent-Type: application/json\r\n"),
        "{head}"
    );
    assert!(body.contains("\"size\":2906544103"));
}

#[test]
fn a_patch_list_string_is_always_escaped() {
    // srp-game-data writes nothing that needs it; were it ever there, the
    // list would still be JSON.
    let odd = crate::OfferedPatch {
        name: "patch_1".into(),
        sha256: "a\"b\\c\n".into(),
        size: 1,
        urls: vec!["http://x/y".into(), "http://x/\"".into()],
    };
    assert_eq!(
        crate::patches::json(&[odd]),
        r#"{"patch_1":{"sha256":"a\"b\\c\u000a","urls":["http://x/y","http://x/\""]}}"#
    );
}

#[test]
fn the_damage_calculator_is_a_026g_page_under_the_sites_header() {
    let world = world();
    let home = world.web.route(&get("/"), "t").body;
    assert!(home.contains(
        "<nav><a href=\"/?lang=en\" aria-current=page>Home</a><a href=\"/wiki?lang=en\">Wiki</a>"
    ));
    let page = world.web.route(&get(crate::damage::PATH), "t");
    assert_eq!(page.status, "200 OK");
    assert!(
        page.body
            .starts_with("<!doctype html>\n<html lang=\"en\">\n")
    );
    assert!(
        page.body
            .contains("<a class=brand href=\"/?lang=en\">СХІД</a>")
    );
    assert!(
        page.body
            .contains("<a href=\"/wiki?lang=en\" aria-current=page>Wiki</a>")
    );
    assert!(page.body.contains("<nav><a href=\"/?lang=en\">Home</a>"));
    assert!(
        page.body
            .contains("<a class=lang href=\"?lang=uk\">Українська</a>")
    );
    assert!(!page.body.contains("{{LANG}}") && !page.body.contains("<!--SITE-HEADER-->"));
    // Nothing inline runs and nothing is fetched from elsewhere.
    assert!(!page.body.contains("<script>") && !page.body.contains("https://"));
    let mut uk = get(crate::damage::PATH);
    uk.lang = Lang::Uk;
    let uk = world.web.route(&uk, "t").body;
    assert!(uk.starts_with("<!doctype html>\n<html lang=\"uk\">\n"));
    assert!(uk.contains(" aria-current=page>Вікі</a>"));
    assert!(uk.contains(
        "<p class=breadcrumb><a href=\"/wiki?lang=uk\">Вікі</a> / Калькулятор шкоди</p>"
    ));
}

#[test]
fn every_file_the_calculator_names_is_served_to_be_kept() {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::time::Duration;
    let world = world();
    // What the page and its script ask for: /damage/... before a ?v=.
    fn named(text: &str) -> Vec<String> {
        text.match_indices("/damage/")
            .filter_map(|(at, _)| text[at..].split_once("?v="))
            .map(|(path, _)| path.to_owned())
            .filter(|path| !path.contains(['"', ' ', '<', '\n']))
            .collect()
    }
    let page = world.web.route(&get(crate::damage::PATH), "t").body;
    let mut wanted = named(&page);
    let (script_type, script) = crate::damage::file("/damage/calc.js").unwrap();
    assert_eq!(script_type, "text/javascript; charset=utf-8");
    wanted.extend(named(std::str::from_utf8(script).unwrap()));
    for (ending, count) in [(".js", 1), (".woff2", 7), (".webp", 13)] {
        assert_eq!(
            wanted.iter().filter(|path| path.ends_with(ending)).count(),
            count,
            "{ending}"
        );
    }
    for path in &wanted {
        let (content_type, bytes) = crate::damage::file(path).unwrap_or_else(|| panic!("{path}"));
        assert!(!bytes.is_empty(), "{path}");
        let expected = match path.rsplit('.').next() {
            Some("js") => "text/javascript; charset=utf-8",
            Some("woff2") => "font/woff2",
            _ => "image/webp",
        };
        assert_eq!(content_type, expected, "{path}");
    }

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let asks = [
        "GET /damage/calc.js?v=0 HTTP/1.0",
        "HEAD /damage/maps/level_01.webp HTTP/1.0",
        "GET /wiki/calculator?lang=uk HTTP/1.0",
        "GET /damage/missing.js HTTP/1.0",
    ];
    let server = std::thread::spawn(move || {
        for _ in asks {
            let (stream, _) = listener.accept().unwrap();
            world.web.serve(stream);
        }
    });
    let answers: Vec<Vec<u8>> = asks
        .iter()
        .map(|ask| {
            let mut client = TcpStream::connect(address).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            client
                .write_all(format!("{ask}\r\n\r\n").as_bytes())
                .unwrap();
            let mut got = Vec::new();
            client.read_to_end(&mut got).unwrap();
            got
        })
        .collect();
    server.join().unwrap();
    let head = |answer: &[u8]| {
        let at = answer.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        (
            String::from_utf8(answer[..at].to_vec()).unwrap(),
            answer.len() - at,
        )
    };
    let (script_head, script_len) = head(&answers[0]);
    assert!(
        script_head
            .starts_with("HTTP/1.0 200 OK\r\nContent-Type: text/javascript; charset=utf-8\r\n")
    );
    assert!(script_head.contains("\r\nCache-Control: public, max-age=31536000, immutable\r\n"));
    assert!(script_head.contains(&format!("\r\nContent-Length: {}\r\n", script.len())));
    assert_eq!(script_len, script.len());
    let (map_head, map_len) = head(&answers[1]);
    assert!(map_head.contains("\r\nContent-Type: image/webp\r\n"));
    assert_eq!(map_len, 0, "a HEAD gets no body");
    let (page_head, _) = head(&answers[2]);
    assert!(page_head.contains("\r\nCache-Control: no-store\r\n"));
    assert!(page_head.contains(&format!(
        "\r\nContent-Security-Policy: {}\r\n",
        crate::damage::POLICY
    )));
    assert!(String::from_utf8_lossy(&answers[2]).contains("<html lang=\"uk\">"));
    assert!(
        head(&answers[3])
            .0
            .starts_with("HTTP/1.0 404 Not Found\r\n")
    );
}

#[test]
fn the_maps_are_a_026g_page_under_the_sites_header() {
    let world = world();
    let page = world.web.route(&get(crate::maps::PATH), "t");
    assert_eq!(page.status, "200 OK");
    assert!(
        page.body
            .starts_with("<!doctype html>\n<html lang=\"en\">\n")
    );
    assert!(
        page.body
            .contains("<a href=\"/wiki?lang=en\" aria-current=page>Wiki</a>")
    );
    assert!(
        page.body
            .contains("<p class=breadcrumb><a href=\"/wiki?lang=en\">Wiki</a> / Maps</p>")
    );
    assert!(!page.body.contains("{{LANG}}") && !page.body.contains("<!--SITE-HEADER-->"));
    assert!(!page.body.contains("<script>") && !page.body.contains("https://"));
    let mut uk = get(crate::maps::PATH);
    uk.lang = Lang::Uk;
    let uk = world.web.route(&uk, "t").body;
    assert!(uk.starts_with("<!doctype html>\n<html lang=\"uk\">\n"));
    assert!(uk.contains(" aria-current=page>Вікі</a>"));
}

#[test]
fn wiki_routes_use_the_sites_navigation_and_language() {
    let world = world();
    for path in [
        "/wiki",
        "/wiki/weapons",
        "/wiki/modifiers",
        "/wiki/equipment",
        "/wiki/calculator",
        "/wiki/maps",
    ] {
        for lang in [Lang::En, Lang::Uk] {
            let mut request = get(path);
            request.lang = lang;
            let response = world.web.route(&request, "t");
            assert_eq!(response.status, "200 OK");
            assert!(
                response
                    .body
                    .contains(&format!("<html lang=\"{}\">", lang.code()))
            );
            assert!(response.body.contains(&format!(
                "href=\"/wiki?lang={}\" aria-current=page",
                lang.code()
            )));
        }
    }
    // The index leads to every section.
    let index = world.web.route(&get("/wiki"), "t").body;
    for section in [
        "/wiki/weapons",
        "/wiki/modifiers",
        "/wiki/equipment",
        "/wiki/calculator",
        "/wiki/maps",
    ] {
        assert!(
            index.contains(&format!("<a class=wiki-link href=\"{section}?lang=en\">")),
            "{section}"
        );
    }
    assert_eq!(
        world.web.route(&get("/wiki/unknown"), "t").status,
        "404 Not Found"
    );
}

#[test]
fn the_calculators_and_the_maps_old_addresses_lead_into_the_wiki() {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    let world = world();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let asks = [
        (
            "GET /damage?lang=uk HTTP/1.0",
            Some("/wiki/calculator?lang=uk"),
        ),
        ("HEAD /maps HTTP/1.0", Some("/wiki/maps?lang=en")),
        // Their files keep their addresses.
        ("GET /maps/maps.js HTTP/1.0", None),
    ];
    let server = std::thread::spawn(move || {
        for _ in asks {
            let (stream, _) = listener.accept().unwrap();
            world.web.serve(stream);
        }
    });
    for (ask, location) in asks {
        let mut client = TcpStream::connect(address).unwrap();
        client
            .write_all(format!("{ask}\r\n\r\n").as_bytes())
            .unwrap();
        let mut got = String::new();
        client.read_to_string(&mut got).unwrap();
        match location {
            Some(location) => {
                assert!(
                    got.starts_with("HTTP/1.0 301 Moved Permanently\r\n"),
                    "{ask}"
                );
                assert!(
                    got.contains(&format!("\r\nLocation: {location}\r\n")),
                    "{ask}"
                );
                assert!(got.ends_with("\r\n\r\n"), "{ask}: no body");
            }
            None => assert!(got.starts_with("HTTP/1.0 200 OK\r\n"), "{ask}"),
        }
    }
    server.join().unwrap();
}

/// A standalone local review of the real HTTP routes, with in-memory test
/// accounts. Does not start or connect to login, lobby or match servers.
#[test]
#[ignore = "local browser preview; serves until stopped"]
fn serve_wiki_preview() {
    // SRP_GAME_DATA: the pages from that server data (a release's, as srp runs
    // on), not the tests' archive; it must be read before anything else is.
    if std::env::var_os("SRP_GAME_DATA").is_some() {
        survarium::v026g::game_data::init().expect("SRP_GAME_DATA reads as srp's game data");
    }
    let world = world();
    let listener = std::net::TcpListener::bind("127.0.0.1:24583").unwrap();
    eprintln!("Wiki preview: http://127.0.0.1:24583/wiki");
    for stream in listener.incoming() {
        world.web.serve(stream.unwrap());
    }
}

#[test]
fn every_file_the_maps_name_is_served() {
    // What the page and its script ask for, before a ?v=: its own files
    // and the calculator's renders and fonts.
    fn named(text: &str) -> Vec<String> {
        ["/maps/", "/damage/"]
            .iter()
            .flat_map(|prefix| text.match_indices(prefix))
            // "/damage/maps/..." holds a "/maps/" too.
            .filter(|&(at, _)| !text[..at].ends_with("/damage"))
            .filter_map(|(at, _)| text[at..].split_once("?v="))
            .map(|(path, _)| path.to_owned())
            .filter(|path| !path.contains(['"', ' ', '<', '\n']))
            .collect()
    }
    let world = world();
    let page = world.web.route(&get(crate::maps::PATH), "t").body;
    let mut wanted = named(&page);
    let (script_type, script) = crate::maps::file("/maps/maps.js").unwrap();
    assert_eq!(script_type, "text/javascript; charset=utf-8");
    wanted.extend(named(std::str::from_utf8(script).unwrap()));
    assert!(wanted.iter().any(|path| path.starts_with("/maps/minimap/")));
    assert!(wanted.iter().any(|path| path.starts_with("/maps/icons/")));
    assert!(wanted.iter().any(|path| path.starts_with("/damage/maps/")));
    for path in &wanted {
        let (_, bytes) = crate::maps::file(path)
            .or_else(|| crate::damage::file(path))
            .unwrap_or_else(|| panic!("{path}"));
        assert!(!bytes.is_empty(), "{path}");
    }
}

#[test]
fn the_maps_and_the_calculator_are_built_from_the_offered_patches() {
    // A live release rebuilds both pages from the patches it offers and its
    // record commits them with release/patches.tsv (release/build-site); this
    // fails a PR that changes one without the other.
    let rows = |text: &str| -> Vec<(String, String)> {
        text.lines()
            .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
            .map(|line| {
                let mut fields = line.split('\t');
                (
                    fields.next().unwrap().to_owned(),
                    fields.next().unwrap().to_owned(),
                )
            })
            .collect()
    };
    let offered = rows(include_str!("../../../../../release/patches.tsv"));
    let maps = rows(include_str!("../../maps/built-from.tsv"));
    assert_eq!(
        maps, offered,
        "the Maps tab is built from other patches than release/patches.tsv: rebuild it with build.py (srp-026g/scripts/maps-tab/README.md)"
    );
    let calculator = rows(include_str!("../../damage/built-from.tsv"));
    assert_eq!(
        calculator, offered,
        "the calculator is built from other patches than release/patches.tsv: rebuild it with build_site.py (srp-026g/scripts/damage-calc/README.md)"
    );
}

/// The value of the hidden field `name` the page shows (a plan's one-time
/// token).
fn hidden(body: &str, name: &str) -> String {
    let start = format!("name={name} value=");
    let at = body
        .find(&start)
        .unwrap_or_else(|| panic!("no {name} in {body}"))
        + start.len();
    body[at..]
        .chars()
        .take_while(char::is_ascii_hexdigit)
        .collect()
}

/// A world whose 0.26g accounts are kept in a repository in memory.
fn kept_world() -> (World, session::v026g::MemoryGameRepository) {
    let repository = session::v026g::MemoryGameRepository::default();
    let accounts = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
    (world_with(accounts), repository)
}

/// Every field sent, then the confirm with the plan the preview showed.
fn confirmed<'a>(
    fields: &[(&'a str, &'a str)],
    plan_field: &'a str,
    plan: &'a str,
) -> Vec<(&'a str, &'a str)> {
    let mut all = fields.to_vec();
    all.extend([(plan_field, plan), ("confirm", "1")]);
    all
}

#[test]
fn money_is_taken_after_a_preview_and_never_below_zero() {
    let (world, repository) = kept_world();
    let id = world.player("pain", "secret");
    let other = world.player("bystander", "secret");
    world.accounts.get(id).unwrap().locked().generic_money = 480_000;
    world.accounts.get(id).unwrap().locked().premium_money = 7;
    world.accounts.flush();
    let others = world.balances(other);
    let asked = [
        ("user", "pain"),
        ("silver", "-500000"),
        ("money_reason", "farmed silver"),
    ];

    // The preview: before and after, what is really taken; nothing changed.
    let preview = world.admin(&asked);
    assert!(preview.contains("Check the change below"), "{preview}");
    assert!(
        preview.contains("480 000 → 0 (-480 000); -500 000 asked, never below 0"),
        "{preview}"
    );
    assert!(preview.contains("7 → 7 (0)"), "{preview}");
    assert!(preview.contains("no: it sees the change at its next sign-in"));
    assert!(
        preview.contains("name=confirm value=1>Apply to pain"),
        "{preview}"
    );
    // Enter in a field presses the form's first button: the preview's.
    let first = preview.find("<button").unwrap();
    assert!(preview[first..].starts_with("<button>"), "{preview}");
    assert_eq!(world.balances(id), (480_000, 7));
    let plan = hidden(&preview, "money_plan");

    // The confirm without the plan, or with another, is a preview again.
    for wrong in ["", "00"] {
        let body = world.admin(&confirmed(&asked, "money_plan", wrong));
        assert!(body.contains("nothing was changed"), "{body}");
        assert_eq!(world.balances(id), (480_000, 7));
    }
    // Without a reason nothing is applied.
    let no_reason = [("user", "pain"), ("silver", "-500000")];
    let body = world.admin(&confirmed(&no_reason, "money_plan", &plan));
    assert!(body.contains("A reason is required"), "{body}");
    assert!(!body.contains("name=confirm value=1"));
    assert_eq!(world.balances(id), (480_000, 7));
    assert!(world.told.changed.locked().is_empty());

    // A confirm uses its token up, applied or not: a new preview's.
    let used = world.admin(&confirmed(&asked, "money_plan", &plan));
    assert!(used.contains("used already or is older"), "{used}");
    assert_eq!(world.balances(id), (480_000, 7));
    let plan = hidden(&used, "money_plan");
    let done = world.admin(&confirmed(&asked, "money_plan", &plan));
    assert!(
        done.contains("pain: silver 480 000 → 0 (-480 000), gold 7 → 7 (0)."),
        "{done}"
    );
    assert!(!done.contains("name=confirm value=1"));
    assert_eq!(world.balances(id), (0, 7));
    // Written at once, with its audit row; the open game is told.
    assert_eq!(repository.get(id).unwrap().generic_money, 0);
    let actions = repository.all_admin_actions();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].account_id, id);
    assert_eq!(actions[0].kind, "money");
    assert_eq!(actions[0].reason, "farmed silver");
    assert_eq!(actions[0].change, "silver -480000 (asked -500000), gold +0");
    assert_eq!(actions[0].before, "480000 silver, 7 gold");
    assert_eq!(actions[0].after, "0 silver, 7 gold");
    assert_eq!(world.told.changed.locked()[..], [id]);

    // A reload of the confirm changes nothing more.
    let reload = world.admin(&confirmed(&asked, "money_plan", &plan));
    assert!(reload.contains("nothing was changed") || reload.contains("Nothing would change"));
    assert_eq!(world.balances(id), (0, 7));
    assert_eq!(repository.all_admin_actions().len(), 1);
    // The audit trail is shown with the account.
    assert!(
        reload.contains("Earlier moderation of this account"),
        "{reload}"
    );
    assert!(reload.contains("farmed silver"));

    // Gold by id, the other account untouched.
    let gold = [
        ("user", &*format!("#{id}")),
        ("gold", "-10"),
        ("money_reason", "farmed gold"),
    ];
    let preview = world.admin(&gold);
    let plan = hidden(&preview, "money_plan");
    let done = world.admin(&confirmed(&gold, "money_plan", &plan));
    assert!(done.contains("gold 7 → 0 (-7)"), "{done}");
    assert_eq!(world.balances(id), (0, 0));
    assert_eq!(world.balances(other), others);
    assert_eq!(repository.all_admin_actions().len(), 2);
}

#[test]
fn money_of_an_online_account_reaches_its_game() {
    let world = world();
    let id = world.player("Mandalorienc", "secret");
    let session = world.web.store.create_session(id);
    let _held = world.web.store.hold(session).unwrap();
    let asked = [
        ("user", "mandalorienc"),
        ("gold", "-100"),
        ("money_reason", "farmed gold"),
    ];
    world.accounts.get(id).unwrap().locked().premium_money = 250;
    let preview = world.admin(&asked);
    assert!(
        preview.contains("yes: its game is sent the change at once"),
        "{preview}"
    );
    let plan = hidden(&preview, "money_plan");
    // The game spends gold between the preview and the confirm: the plan
    // no longer fits, and nothing is taken.
    world.accounts.get(id).unwrap().locked().premium_money = 240;
    let late = world.admin(&confirmed(&asked, "money_plan", &plan));
    assert!(
        late.contains("The account changed since the preview"),
        "{late}"
    );
    assert_eq!(world.balances(id).1, 240);
    let plan = hidden(&late, "money_plan");
    world.admin(&confirmed(&asked, "money_plan", &plan));
    assert_eq!(world.balances(id).1, 140);
    assert_eq!(world.told.changed.locked()[..], [id]);
}

#[test]
fn moderation_needs_the_token_the_address_and_a_known_account() {
    let world = world();
    let id = world.player("tester", "secret");
    let before = world.balances(id);
    let asked = [("user", "tester"), ("silver", "-5"), ("money_reason", "x")];
    let plan = hidden(&world.admin(&asked), "money_plan");
    let mut fields = vec![("token", "bad")];
    fields.extend(confirmed(&asked, "money_plan", &plan));
    let refused = world.web.route(&post("/hidden", &fields), "t");
    assert!(refused.body.contains("Wrong token"));
    assert!(!refused.body.contains("money_plan"));
    let mut fields = vec![("token", "tok")];
    fields.extend(confirmed(&asked, "money_plan", &plan));
    let mut elsewhere = post("/hidden", &fields);
    elsewhere.client = "203.0.113.5".parse().ok();
    assert_eq!(world.web.route(&elsewhere, "t").status, "404 Not Found");
    // Items alike.
    let item = world.accounts.get(id).unwrap().locked().inventory[0].item_id;
    let take = item.to_string();
    let mut fields = vec![
        ("token", "bad"),
        ("tab", "items"),
        ("items_user", "tester"),
        ("take", &take),
        ("items_reason", "x"),
        ("confirm", "1"),
    ];
    assert!(
        world
            .web
            .route(&post("/hidden", &fields), "t")
            .body
            .contains("Wrong token")
    );
    fields[0] = ("token", "tok");
    let mut elsewhere = post("/hidden", &fields);
    elsewhere.client = "203.0.113.5".parse().ok();
    assert_eq!(world.web.route(&elsewhere, "t").status, "404 Not Found");
    assert_eq!(world.balances(id), before);
    assert!(
        world
            .accounts
            .get(id)
            .unwrap()
            .locked()
            .item(item)
            .is_some()
    );
    assert!(world.told.changed.locked().is_empty());

    // Unknown accounts.
    for user in ["ghost", "#999999", "999999"] {
        let body = world.admin(&[("user", user), ("silver", "-5"), ("money_reason", "x")]);
        assert!(
            body.contains("No 0.26g account has that id") || body.contains("No such user"),
            "{user}: {body}"
        );
        assert!(!body.contains("money_plan"));
        let body = world.admin(&[("tab", "items"), ("items_user", user)]);
        assert!(!body.contains("name=take"), "{user}");
    }
}

#[test]
fn items_are_listed_previewed_and_taken_off_every_kit() {
    let (world, repository) = kept_world();
    let id = world.player("Fasojib", "secret");
    let other = world.player("bystander", "secret");
    let (pistol, ammo, units, kept) = {
        let account = world.accounts.get(id).unwrap();
        let mut account = account.locked();
        let pistol = account.inventory[0].item_id;
        let ammo = account.inventory[1].clone();
        // The pistol is worn by every kit.
        let slot = account.kits[0].slot_of(pistol).unwrap();
        let worn = account.kits[0].slots[slot];
        for kit in &mut account.kits {
            kit.slots[slot] = worn;
        }
        let kept = account.inventory[2].item_id;
        (pistol, ammo.item_id, ammo.condition_or_stack, kept)
    };
    let others = world.accounts.get(other).unwrap().locked().clone();
    world.accounts.flush();

    // The listing.
    let list = world.admin(&[("tab", "items"), ("items_user", "fasojib")]);
    assert!(list.contains("Tick the items to take"), "{list}");
    assert!(
        list.contains(&format!("name=take value={pistol} ")),
        "{list}"
    );
    assert!(list.contains(&format!("name=u{ammo} ")), "{list}");
    assert!(list.contains("kit 1*, slot"), "{list}");
    assert!(list.contains("kit 3, slot"), "{list}");
    assert!(!list.contains("items_plan"));

    // The pistol whole, and all but 1 unit of the ammo.
    let pistol_text = pistol.to_string();
    let ammo_text = ammo.to_string();
    let ammo_units = format!("u{ammo}");
    let leave_one = (units - 1).to_string();
    let asked = [
        ("tab", "items"),
        ("items_user", "Fasojib"),
        ("take", pistol_text.as_str()),
        ("take", ammo_text.as_str()),
        (ammo_units.as_str(), leave_one.as_str()),
        ("items_reason", "duped"),
    ];
    let preview = world.admin(&asked);
    assert!(preview.contains("Would be taken"), "{preview}");
    assert!(preview.contains("taken off its kits"), "{preview}");
    assert!(preview.contains("(1 left)"), "{preview}");
    assert!(preview.contains(&format!("name=take value={pistol} checked")));
    assert!(preview.contains("name=confirm value=1>Apply: take them from Fasojib"));
    let first = preview.find("<button").unwrap();
    assert!(preview[first..].starts_with("<button>"));
    let plan = hidden(&preview, "items_plan");
    assert!(
        world
            .accounts
            .get(id)
            .unwrap()
            .locked()
            .item(pistol)
            .is_some()
    );

    // Another choice than the preview's: nothing is taken.
    let fewer = [
        ("tab", "items"),
        ("items_user", "Fasojib"),
        ("take", pistol_text.as_str()),
        ("items_reason", "duped"),
    ];
    let body = world.admin(&confirmed(&fewer, "items_plan", &plan));
    assert!(body.contains("nothing was"), "{body}");
    assert!(
        world
            .accounts
            .get(id)
            .unwrap()
            .locked()
            .item(pistol)
            .is_some()
    );
    assert!(world.told.changed.locked().is_empty());

    // That confirm used the token up: a new preview's.
    let plan = hidden(&world.admin(&asked), "items_plan");
    let done = world.admin(&confirmed(&asked, "items_plan", &plan));
    assert!(done.contains("Fasojib: 2 item(s) taken."), "{done}");
    assert!(!done.contains("name=confirm value=1"));
    {
        let account = world.accounts.get(id).unwrap();
        let account = account.locked();
        assert!(account.item(pistol).is_none());
        assert_eq!(account.item(ammo).unwrap().condition_or_stack, 1);
        assert!(account.item(kept).is_some());
        assert!(account.kits.iter().all(|kit| kit.slot_of(pistol).is_none()));
        let worn = account.kits[0].slot_of(ammo).unwrap();
        assert_eq!(account.kits[0].slots[worn].unwrap().count, 1);
        // The kits are valid: a repair finds nothing to change.
        assert!(!account.clone().repair());
        // Written at once.
        assert_eq!(repository.get(id).unwrap(), *account);
    }
    assert_eq!(world.told.changed.locked()[..], [id]);
    assert_eq!(*world.accounts.get(other).unwrap().locked(), others);
    let actions = repository.all_admin_actions();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].kind, "items");
    assert_eq!(actions[0].reason, "duped");
    assert!(
        actions[0].change.contains(&format!("item {pistol})")),
        "{:?}",
        actions[0]
    );

    // A reload: the pistol is gone, nothing more is taken.
    let reload = world.admin(&confirmed(&asked, "items_plan", &plan));
    assert!(
        reload.contains(&format!("no item {pistol} now")),
        "{reload}"
    );
    assert_eq!(
        world
            .accounts
            .get(id)
            .unwrap()
            .locked()
            .item(ammo)
            .unwrap()
            .condition_or_stack,
        1
    );
    assert_eq!(repository.all_admin_actions().len(), 1);
    assert_eq!(world.told.changed.locked().len(), 1);
}

#[test]
fn a_moderation_the_database_refused_says_so() {
    let (world, repository) = kept_world();
    let id = world.player("pain", "secret");
    world.accounts.flush();
    repository.fail_saves(true);
    let asked = [
        ("user", "pain"),
        ("silver", "-100"),
        ("money_reason", "farmed"),
    ];
    let plan = hidden(&world.admin(&asked), "money_plan");
    let done = world.admin(&confirmed(&asked, "money_plan", &plan));
    assert!(
        done.contains("Error: the change stands in memory only"),
        "{done}"
    );
    assert_eq!(world.balances(id).0, 1_900);
    // The saver writes it once the database takes it again.
    repository.fail_saves(false);
    world.accounts.flush();
    assert_eq!(repository.get(id).unwrap().generic_money, 1_900);
    assert_eq!(repository.all_admin_actions().len(), 1);
}

#[test]
fn a_used_confirm_changes_nothing_even_once_the_account_is_back() {
    let (world, repository) = kept_world();
    let id = world.player("pain", "secret");
    world.accounts.get(id).unwrap().locked().generic_money = 500;
    let asked = [
        ("user", "pain"),
        ("silver", "-100"),
        ("money_reason", "farmed"),
    ];
    let plan = hidden(&world.admin(&asked), "money_plan");
    let confirm = confirmed(&asked, "money_plan", &plan);
    world.admin(&confirm);
    assert_eq!(world.balances(id).0, 400);
    // The game gives the 100 back: the account is as at the preview.
    world.accounts.get(id).unwrap().locked().generic_money = 500;
    let again = world.admin(&confirm);
    assert!(again.contains("used already or is older"), "{again}");
    assert_eq!(world.balances(id).0, 500);
    assert_eq!(repository.all_admin_actions().len(), 1);

    // Items alike: part of a stack, the stack made whole again.
    let (ammo, units) = {
        let account = world.accounts.get(id).unwrap();
        let account = account.locked();
        let ammo = &account.inventory[1];
        assert!(Dictionary::shared().is_stack(ammo.dict_id));
        (ammo.item_id, ammo.condition_or_stack)
    };
    let ammo_text = ammo.to_string();
    let ammo_units = format!("u{ammo}");
    let asked = [
        ("tab", "items"),
        ("items_user", "pain"),
        ("take", ammo_text.as_str()),
        (ammo_units.as_str(), "1"),
        ("items_reason", "duped"),
    ];
    let plan = hidden(&world.admin(&asked), "items_plan");
    let confirm = confirmed(&asked, "items_plan", &plan);
    let stack = || {
        world
            .accounts
            .get(id)
            .unwrap()
            .locked()
            .item(ammo)
            .unwrap()
            .condition_or_stack
    };
    assert!(world.admin(&confirm).contains("1 item(s) taken"));
    assert_eq!(stack(), units - 1);
    world
        .accounts
        .get(id)
        .unwrap()
        .locked()
        .inventory
        .iter_mut()
        .find(|item| item.item_id == ammo)
        .unwrap()
        .condition_or_stack = units;
    let again = world.admin(&confirm);
    assert!(again.contains("used already or is older"), "{again}");
    assert_eq!(stack(), units);
    assert_eq!(repository.all_admin_actions().len(), 2);

    // A preview older than its life is refused, money and items.
    let money = [
        ("user", "pain"),
        ("silver", "-100"),
        ("money_reason", "farmed"),
    ];
    let plan = hidden(&world.admin(&money), "money_plan");
    let items_plan = hidden(&world.admin(&asked), "items_plan");
    world.web.plans.expire_all();
    let late = world.admin(&confirmed(&money, "money_plan", &plan));
    assert!(late.contains("older than 10 minutes"), "{late}");
    let late = world.admin(&confirmed(&asked, "items_plan", &items_plan));
    assert!(late.contains("older than 10 minutes"), "{late}");
    assert_eq!(world.balances(id).0, 500);
    assert_eq!(stack(), units);
    assert_eq!(repository.all_admin_actions().len(), 2);
}

#[test]
fn bare_digits_are_a_name_and_only_hash_digits_an_id() {
    let world = world();
    world.accounts.get_or_create(1337, "Bystander");
    world.accounts.get_or_create(4242, "Other");
    for login in ["first", "second"] {
        let id = world.player(login, "secret");
        world.accounts.get(id).unwrap().locked().name = "1337".into();
    }
    // A user name in digits that has not signed in to 0.26g yet.
    world
        .web
        .store
        .sign_in_with_digest_from(
            "4242",
            &sha1_hex(b"secret"),
            "127.0.0.1".parse().unwrap(),
            Instant::now(),
        )
        .unwrap();
    let money =
        |user: &str| world.admin(&[("user", user), ("silver", "-5"), ("money_reason", "x")]);
    let items = |user: &str| world.admin(&[("tab", "items"), ("items_user", user)]);
    for body in [money("1337"), items("1337")] {
        assert!(
            body.contains("More than one account has that character name"),
            "{body}"
        );
        assert!(
            !body.contains("Bystander") && !body.contains("0x539"),
            "{body}"
        );
        assert!(!body.contains("money_plan") && !body.contains("name=take"));
    }
    for body in [money("4242"), items("4242")] {
        assert!(
            body.contains("has not signed in to the 0.26g lobby yet"),
            "{body}"
        );
        assert!(
            !body.contains("Other") && !body.contains("0x1092"),
            "{body}"
        );
        assert!(!body.contains("money_plan") && !body.contains("name=take"));
    }
    for body in [money("#1337"), items("#1337")] {
        assert!(
            body.contains("Bystander") && body.contains("1337 (0x539)"),
            "{body}"
        );
    }
}

#[test]
fn a_take_value_that_is_no_item_id_says_so() {
    let world = world();
    let id = world.player("tester", "secret");
    let before = world.accounts.get(id).unwrap().locked().clone();
    let body = world.admin(&[
        ("tab", "items"),
        ("items_user", "tester"),
        ("take", "12abc"),
        ("items_reason", "x"),
    ]);
    assert!(
        body.contains("&quot;12abc&quot; is not an item id"),
        "{body}"
    );
    assert!(!body.contains("Units are a whole number"), "{body}");
    assert_eq!(*world.accounts.get(id).unwrap().locked(), before);
}

#[test]
fn a_moderation_waits_for_a_busy_saver_once_only() {
    let (world, repository) = kept_world();
    let id = world.player("pain", "secret");
    let other = world.player("bystander", "secret");
    world.accounts.flush();
    let asked = [
        ("user", "pain"),
        ("silver", "-100"),
        ("money_reason", "farmed"),
    ];
    let plan = hidden(&world.admin(&asked), "money_plan");
    // The saver is in a long pass: it holds the database.
    let held = repository.hold_saves();
    world.accounts.get(other).unwrap().locked().generic_money = 1;
    world.accounts.mark_dirty(other);
    std::thread::scope(|scope| {
        let saver = scope.spawn(|| world.accounts.flush());
        while repository.saves_held() == 0 {
            std::thread::yield_now();
        }
        let started = Instant::now();
        let done = world.admin(&confirmed(&asked, "money_plan", &plan));
        let waited = started.elapsed();
        assert!(done.contains("The database was busy"), "{done}");
        assert!(
            done.contains("the database is busy; shown next time"),
            "{done}"
        );
        assert!(
            waited < session::RESET_WRITE_WAIT + session::RESET_WRITE_WAIT / 2,
            "waited {waited:?}"
        );
        drop(held);
        saver.join().unwrap();
    });
    assert_eq!(world.balances(id).0, 1_900);
    world.accounts.flush();
    assert_eq!(repository.get(id).unwrap().generic_money, 1_900);
    assert_eq!(repository.all_admin_actions().len(), 1);
}

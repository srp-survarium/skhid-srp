//! The web page over the 0.26g stack, for real: srp's servers
//! (`srp::v026g::Stack`) on free localhost ports, the web server on one
//! more, a scripted 0.26g client signed in through TLS and holding its
//! lobby link, and plain HTTP requests as a browser makes them.
//!
//! What it shows: what the admin changes reaches the open game (through
//! the strict client model: nothing a real client would drop or misread);
//! the old exchange form changes nothing (it is in the game now); the
//! operator's text reaches every client and who signs in later; a ban from
//! the admin page ends the 0.26g lobby link and refuses the next sign-in.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lobby_server::v026g::test_client::TestClient;
use session::v026g::GameAccounts;
use session::{LockExt, SessionStore};
use srp::v026g::{Options, Stack};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::lobby::FULL_CONDITION;
use survarium::v026g::lobby::client_model::Decoded;

const SOON: Duration = Duration::from_secs(10);
const TOKEN: &str = "test-admin-token";
const ADMIN: &str = "/hidden-admin";

fn temp_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "srp-web-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The stack over a database in `dir` and the 0.26g web page beside it, as
/// `main` joins them. Returns the stack and the page's address.
fn start(name: &str) -> (Stack, SocketAddr) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    // SAFETY: before any server thread, and every test comes through here.
    ONCE.call_once(|| unsafe { std::env::set_var("SRP_SNAPSHOT_DIR", "off") });
    let dir = temp_dir(name);
    let store = SessionStore::with_repository(
        Duration::from_secs(60),
        Box::new(session::persist::open_in(&dir).expect("the database")),
    )
    .expect("the accounts");
    let accounts = GameAccounts::with_repository(Box::new(
        session::v026g::SqliteGameRepository::open_in(&dir).expect("the 0.26g tables"),
    ))
    .expect("the 0.26g accounts");
    let certs = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../certs");
    let stack = Stack::start(Arc::new(store), Arc::new(accounts), Options::local(&certs))
        .expect("the servers");
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let web = listener.local_addr().unwrap();
    let (store, game) = (stack.store.clone(), stack.web_game());
    std::thread::spawn(move || {
        let admin = web_server::Admin {
            token: Some(TOKEN.to_owned()),
            path: Some(ADMIN.to_owned()),
            ips: vec![session::Net::parse("127.0.0.1").unwrap()],
            trust_proxy: false,
        };
        let _ = web_server::serve_on(&listener, store, admin, Some(game));
    });
    (stack, web)
}

/// One form sent as a browser does; the page that comes back.
fn post(web: SocketAddr, path: &str, form: &[(&str, &str)]) -> String {
    let encode = |text: &str| -> String {
        text.bytes()
            .map(|byte| match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                    (byte as char).to_string()
                }
                byte => format!("%{byte:02X}"),
            })
            .collect()
    };
    let body = form
        .iter()
        .map(|(key, value)| format!("{key}={}", encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    let mut stream = TcpStream::connect(web).expect("the web server");
    stream.set_read_timeout(Some(SOON)).unwrap();
    write!(
        stream,
        "POST {path} HTTP/1.0\r\nContent-Type: application/x-www-form-urlencoded\r\n\
         Accept-Language: en\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer);
    answer
}

fn admin(web: SocketAddr, form: &[(&str, &str)]) -> String {
    let mut all = vec![("token", TOKEN)];
    all.extend_from_slice(form);
    post(web, ADMIN, &all)
}

fn sha1_hex(bytes: &[u8]) -> Vec<u8> {
    openssl::sha::sha1(bytes)
        .iter()
        .flat_map(|byte| format!("{byte:02x}").into_bytes())
        .collect()
}

/// The 0.26g sign-in with a typed password: the reply past the TLS
/// handshake (`08` + session + lobby address, or one status byte).
fn sign_in(login: SocketAddr, user_name: &str, password: &str) -> Vec<u8> {
    use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
    let mut stream = TcpStream::connect(login).expect("the login server");
    stream.set_read_timeout(Some(SOON)).unwrap();
    stream.set_write_timeout(Some(SOON)).unwrap();
    let mut message = vec![0x01, user_name.len() as u8];
    message.extend(user_name.as_bytes());
    let mut version = [0_u8; 12];
    version[..6].copy_from_slice(b"0.26g0");
    message.extend(version);
    message.push(0);
    message.extend(u32::MAX.to_le_bytes());
    stream.write_all(&message).unwrap();
    let mut answer = [0];
    stream.read_exact(&mut answer).unwrap();
    assert_eq!(answer[0], 0x0B, "the user name was not taken");
    let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
    connector.set_security_level(0);
    connector.set_verify(SslVerifyMode::NONE);
    let mut tls = connector
        .build()
        .configure()
        .unwrap()
        .verify_hostname(false)
        .connect("localhost", stream)
        .expect("the TLS handshake");
    let mut message = vec![0x03];
    message.extend(sha1_hex(password.as_bytes()));
    message.push(password.len() as u8);
    message.extend(password.as_bytes());
    tls.write_all(&message).unwrap();
    let mut reply = Vec::new();
    let _ = tls.read_to_end(&mut reply);
    reply
}

/// A game in the lobby.
struct Game {
    name: &'static str,
    lobby_addr: SocketAddr,
    socket: UdpSocket,
    lobby: TestClient,
}

impl Game {
    fn signed_in(stack: &Stack, name: &'static str, password: &str) -> Self {
        let reply = sign_in(stack.login_addr, name, password);
        assert_eq!(reply.len(), 11, "not signed in: {reply:02X?}");
        assert_eq!(reply[0], 0x08);
        let session = u32::from_le_bytes(reply[1..5].try_into().unwrap());
        let ip = Ipv4Addr::from(u32::from_le_bytes(reply[5..9].try_into().unwrap()));
        let port = u16::from_le_bytes(reply[9..11].try_into().unwrap());
        let socket = UdpSocket::bind("127.0.0.1:0").expect("a free port");
        socket.set_nonblocking(true).unwrap();
        let mut lobby = TestClient::new();
        lobby.sign_in(session, true);
        let mut game = Self {
            name,
            lobby_addr: SocketAddr::from((ip, port)),
            socket,
            lobby,
        };
        // The whole sign-in push: the squad info is its last message.
        game.until("the sign-in push", |game| {
            let ids = &game.lobby.ids;
            ids.contains(&0x85)
                && ids.contains(&0xA1)
                && ids.iter().filter(|id| **id == 0xA3).count() == 2
        });
        game
    }

    fn pump(&mut self) {
        let mut buffer = [0_u8; 2048];
        for datagram in self.lobby.poll(Instant::now()) {
            let _ = self.socket.send_to(&datagram, self.lobby_addr);
        }
        while let Ok((len, from)) = self.socket.recv_from(&mut buffer) {
            if from == self.lobby_addr {
                self.lobby.receive(&buffer[..len]);
            }
        }
    }

    fn until(&mut self, what: &str, done: impl Fn(&Game) -> bool) {
        let deadline = Instant::now() + SOON;
        loop {
            self.pump();
            if done(self) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "{}: timed out waiting for {what}; faults {:?}, last {:02X?}",
                self.name,
                self.lobby.faults,
                self.lobby.ids.iter().rev().take(12).collect::<Vec<_>>()
            );
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    fn has(&self, pick: impl Fn(&Decoded) -> bool) -> bool {
        self.lobby.decoded.iter().any(pick)
    }

    fn forget(&mut self) {
        self.lobby.ids.clear();
        self.lobby.decoded.clear();
    }

    fn assert_clean(&self) {
        assert_eq!(self.lobby.faults, Vec::<String>::new(), "{}", self.name);
        assert_eq!(self.lobby.waiting_in_order(), 0, "{}", self.name);
        assert_eq!(self.lobby.order_drops, 0, "{}", self.name);
    }
}

#[test]
fn the_page_changes_an_account_and_its_open_game_is_told() {
    let (stack, web) = start("account");
    let password = "web-password-1";
    let mut alice = Game::signed_in(&stack, "WebAlice", password);
    let id = stack.store.account_id_by_name("WebAlice").unwrap();
    let (silver, gold) = {
        let account = stack.accounts.get(id).unwrap();
        let account = account.locked();
        (account.generic_money, account.premium_money)
    };
    alice.forget();

    // The exchange is in the game now: the form the page had, with the
    // password typed in the game, is a page that is not there and changes
    // nothing.
    let sent = post(
        web,
        "/exchange",
        &[("user", "WebAlice"), ("password", password), ("gold", "3")],
    );
    assert!(sent.starts_with("HTTP/1.0 404 Not Found\r\n"), "{sent}");
    {
        let account = stack.accounts.get(id).unwrap();
        let account = account.locked();
        assert_eq!(
            (account.generic_money, account.premium_money),
            (silver, gold)
        );
    }

    // The admin: premium time, a level, reputation. The game gets its
    // account again and its services (the premium hours).
    let changed = admin(
        web,
        &[
            ("tab", "account"),
            ("acc_user", "WebAlice"),
            ("premium_hours", "+24"),
            ("level", "2"),
            ("faction", "1"),
            ("reputation", "+40"),
        ],
    );
    assert!(changed.contains("level 2"), "{changed}");
    alice.until("the account again", |game| {
        game.has(|message| matches!(message, Decoded::Skills { level: 2, .. }))
            && game.has(|message| {
                // A new account's 3000 with the Scavengers and the 40.
                matches!(message, Decoded::Reputations { reputations, .. }
                    if reputations.contains(&(1, 3_040)))
            })
            && game.has(|message| {
                matches!(
                    message,
                    // A new account's month (720 hours) and the 24.
                    Decoded::Services {
                        premium_hours: Some(744)
                    }
                )
            })
    });
    alice.forget();

    // An item of the dictionary, by its id: in the inventory the game
    // holds, new and whole.
    let dictionary = Dictionary::shared();
    let weapon = {
        let account = stack.accounts.get(id).unwrap();
        let account = account.locked();
        account
            .inventory
            .iter()
            .map(|item| item.dict_id)
            .find(|dict_id| dictionary.item(*dict_id).is_some_and(|item| !item.is_stack))
            .expect("the starter weapon")
    };
    let owned = |game: &Game| {
        game.lobby
            .model
            .inventory()
            .iter()
            .filter(|item| item.dict_id == weapon && item.condition_or_stack == FULL_CONDITION)
            .count()
    };
    let before = owned(&alice);
    let given = admin(
        web,
        &[
            ("tab", "item"),
            ("item_user", "WebAlice"),
            ("item", &weapon.to_string()),
            ("count", "2"),
        ],
    );
    assert!(given.contains("given 2 of"), "{given}");
    alice.until("the items", |game| owned(game) == before + 2);

    // Written: the database has what the game was shown.
    stack.accounts.flush();
    assert!(!stack.accounts.is_dirty(id));
    alice.assert_clean();
    stack.stop();
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

/// `1 234 567`, as the page writes money.
fn group(value: u32) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(digit);
    }
    out
}

/// The admin's moderation of an account whose game is open: silver taken
/// (never below 0) and a worn item taken away, each after its preview. The
/// game is shown the new balance and an inventory and kits without the
/// item; both are in the database at once, with their audit rows.
#[test]
fn moderation_reaches_the_open_game_and_the_database() {
    let (stack, web) = start("moderation");
    let mut alice = Game::signed_in(&stack, "WebFarmer", "farmer-password-1");
    let id = stack.store.account_id_by_name("WebFarmer").unwrap();
    let silver = stack.accounts.get(id).unwrap().locked().generic_money;
    assert!(silver > 0);
    alice.forget();

    let asked = [
        ("user", "WebFarmer"),
        ("silver", "-99999999"),
        ("money_reason", "farmed"),
    ];
    let preview = admin(web, &asked);
    assert!(
        preview.contains("yes: its game is sent the change at once"),
        "{preview}"
    );
    let plan = hidden(&preview, "money_plan");
    let mut confirm = asked.to_vec();
    confirm.extend([("money_plan", plan.as_str()), ("confirm", "1")]);
    let done = admin(web, &confirm);
    assert!(done.contains(&format!("(-{})", group(silver))), "{done}");
    alice.until("the new balance", |game| {
        game.has(|message| matches!(message, Decoded::Money { generic: 0, .. }))
    });
    alice.forget();

    // The starter kit's jacket, worn (`session::v026g::tutorial`).
    let jacket = stack.accounts.get(id).unwrap().locked().kits[0].slots[2]
        .expect("the starter jacket")
        .item_id;
    let take = jacket.to_string();
    let asked = [
        ("tab", "items"),
        ("items_user", "WebFarmer"),
        ("take", take.as_str()),
        ("items_reason", "duped"),
    ];
    let plan = hidden(&admin(web, &asked), "items_plan");
    let mut confirm = asked.to_vec();
    confirm.extend([("items_plan", plan.as_str()), ("confirm", "1")]);
    let done = admin(web, &confirm);
    assert!(done.contains("WebFarmer: 1 item(s) taken."), "{done}");
    alice.until("the inventory without it", |game| {
        game.has(|message| matches!(message, Decoded::Inventory { .. }))
            && game
                .lobby
                .model
                .inventory()
                .iter()
                .all(|item| item.item_id != jacket)
    });
    // The audit trail (in the database) has both, the newest first.
    let actions = stack.accounts.admin_actions(id, 10, true).unwrap();
    let kinds: Vec<&str> = actions.iter().map(|action| action.kind.as_str()).collect();
    assert_eq!(kinds, ["items", "money"]);
    assert!(
        stack.accounts.get(id).unwrap().locked().kits[0]
            .slot_of(jacket)
            .is_none()
    );
    alice.assert_clean();
    stack.stop();
}

#[test]
fn the_operators_text_and_a_ban_reach_the_026g_lobby() {
    let (stack, web) = start("ban");
    let mut alice = Game::signed_in(&stack, "WebCheater", "cheater-password-1");
    let mut bob = Game::signed_in(&stack, "WebBob", "bob-password-1");
    alice.forget();
    bob.forget();

    let shown = admin(
        web,
        &[
            ("tab", "text"),
            ("line1", "Restart at 20:00"),
            ("line2", "Thank you"),
        ],
    );
    assert!(shown.contains("shown to everyone"), "{shown}");
    for game in [&mut alice, &mut bob] {
        game.until("the operator's text", |game| {
            game.has(|message| {
                matches!(message, Decoded::AdminText(first, second)
                    if first == "Restart at 20:00" && second == "Thank you")
            })
        });
    }
    // Who signs in while it stands is shown it too.
    let carol = Game::signed_in(&stack, "WebCarol", "carol-password-1");
    assert!(carol.has(|message| matches!(message, Decoded::AdminText(..))));
    let cleared = admin(web, &[("tab", "text"), ("clear_text", "1")]);
    assert!(cleared.contains("The text is cleared"), "{cleared}");
    bob.until("the text cleared", |game| {
        game.has(|message| matches!(message, Decoded::AdminTextClear))
    });

    // The ban: the cheater's lobby link is ended (0xAA, then the link is
    // closed), nobody else's; its next sign-in is refused with the game's
    // ban status (0x1C: "banned by an administrator").
    let banned = admin(
        web,
        &[
            ("tab", "ban"),
            ("ban_user", "WebCheater"),
            ("hours", "1"),
            ("reason", "test"),
        ],
    );
    assert!(
        banned.contains("WebCheater banned for 1 h; 1 session(s) ended"),
        "{banned}"
    );
    alice.until("the sign-out", |game| {
        game.has(|message| matches!(message, Decoded::SignedOut)) && game.lobby.disconnected
    });
    let refused = sign_in(stack.login_addr, "WebCheater", "cheater-password-1");
    assert_eq!(refused, [0x1C], "{refused:02X?}");
    // Bob plays on.
    let counts = bob.lobby.online.len();
    bob.until("the online counts", |game| game.lobby.online.len() > counts);
    assert!(!bob.lobby.disconnected);

    // Lifted: it signs in again.
    let lifted = admin(web, &[("tab", "unban"), ("unban_user", "WebCheater")]);
    assert!(lifted.contains("WebCheater unbanned"), "{lifted}");
    let again = Game::signed_in(&stack, "WebCheater", "cheater-password-1");

    alice.assert_clean();
    bob.assert_clean();
    carol.assert_clean();
    again.assert_clean();
    stack.stop();
}

/// The admin's password reset while srp runs: from the page (after its
/// confirm step) and from a script's one POST. The game signed in with the
/// account is signed out, nobody else's; the next sign-in, with any
/// password, sets the new one, and the old one is refused from then on.
#[test]
fn a_password_reset_signs_the_game_out_and_the_next_sign_in_sets_it() {
    let (stack, web) = start("reset");
    let mut alice = Game::signed_in(&stack, "WebForgetful", "old-password-1");
    let mut bob = Game::signed_in(&stack, "WebBob", "bob-password-1");
    alice.forget();
    bob.forget();
    let alice_id = stack.store.account_id_by_name("webforgetful").unwrap();

    let shown = admin(web, &[("tab", "password"), ("reset_user", "WEBFORGETFUL")]);
    assert!(
        shown.contains("Reset the password of webforgetful"),
        "{shown}"
    );
    assert!(shown.contains("yes: its game is signed out"), "{shown}");
    let done = admin(
        web,
        &[
            ("tab", "password"),
            ("reset_user", "WebForgetful"),
            ("reset_id", &alice_id.to_string()),
        ],
    );
    assert!(
        done.contains("The password of webforgetful is cleared; 1 session(s) ended"),
        "{done}"
    );
    alice.until("the sign-out", |game| {
        game.has(|message| matches!(message, Decoded::SignedOut)) && game.lobby.disconnected
    });
    let counts = bob.lobby.online.len();
    bob.until("the online counts", |game| game.lobby.online.len() > counts);
    assert!(!bob.lobby.disconnected);

    // Any password is taken, then it alone.
    let again = Game::signed_in(&stack, "WebForgetful", "new-password-2");
    let refused = sign_in(stack.login_addr, "WebForgetful", "old-password-1");
    let wrong = sign_in(stack.login_addr, "WebBob", "not-bobs-password");
    assert_eq!(refused.len(), 1, "{refused:02X?}");
    assert_eq!(refused, wrong);

    // A script: one POST, one line back.
    let answer = admin(web, &[("tab", "reset_password"), ("login", "webbob")]);
    let bob_id = stack.store.account_id_by_name("webbob").unwrap();
    assert!(answer.starts_with("HTTP/1.0 200 OK\r\n"), "{answer}");
    assert!(answer.contains("Content-Type: text/plain"), "{answer}");
    assert!(
        answer.ends_with(&format!("\r\n\r\nok webbob {bob_id}\n")),
        "{answer}"
    );
    bob.until("the sign-out", |game| {
        game.has(|message| matches!(message, Decoded::SignedOut)) && game.lobby.disconnected
    });
    let unknown = admin(web, &[("tab", "reset_password"), ("login", "nobody")]);
    assert!(
        unknown.starts_with("HTTP/1.0 404 Not Found\r\n"),
        "{unknown}"
    );
    assert!(unknown.ends_with("\r\n\r\nno such login\n"), "{unknown}");
    let refused = post(
        web,
        ADMIN,
        &[
            ("token", "wrong"),
            ("tab", "reset_password"),
            ("login", "webforgetful"),
        ],
    );
    assert!(
        refused.starts_with("HTTP/1.0 403 Forbidden\r\n"),
        "{refused}"
    );
    let bob_again = Game::signed_in(&stack, "WebBob", "bob-password-2");

    alice.assert_clean();
    bob.assert_clean();
    again.assert_clean();
    bob_again.assert_clean();
    stack.stop();
}

/// What the players' updater asks after the Skhid files: `GET
/// /patches`, with no token, from the same page. It lists the patches the
/// server's game data was made of (the test data: 0.26g's archive with
/// patches 14, 15 and 16), in the game's order.
#[test]
fn the_updater_is_told_the_patches_the_server_runs_on() {
    let (stack, web) = start("patches");
    let mut stream = TcpStream::connect(web).expect("the web server");
    stream.set_read_timeout(Some(SOON)).unwrap();
    stream
        .write_all(b"GET /patches HTTP/1.1\r\nHost: skhid\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer);
    let (head, body) = answer.split_once("\r\n\r\n").expect("an HTTP answer");
    assert!(head.starts_with("HTTP/1.0 200 OK\r\n"), "{head}");
    assert!(
        head.contains("\r\nContent-Type: application/json\r\n"),
        "{head}"
    );
    let provenance = &survarium::v026g::game_data::shared().provenance;
    let expected: Vec<String> = provenance
        .offered()
        .map(|(name, sha256, urls)| {
            let [url] = urls else {
                panic!("the test data's patches have one URL each: {urls:?}")
            };
            format!("\"{name}\":{{\"sha256\":\"{sha256}\",\"urls\":[\"{url}\"]}}")
        })
        .collect();
    assert_eq!(expected.len(), 3);
    assert_eq!(body, format!("{{{}}}", expected.join(",")));
    assert!(body.starts_with(
        "{\"patch_14\":{\"sha256\":\"7535d0d0c287cf1c3c338936fdef54edb7f36e1334d7e872801a27d505d4c6c3\",\
         \"urls\":[\"http://203.0.113.5/patches/resources.db.patch_14\"]},\"patch_15\":"
    ));
    stack.stop();
}

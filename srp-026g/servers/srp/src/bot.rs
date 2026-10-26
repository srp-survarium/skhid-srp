//! A scripted 0.26g client that takes a place in a match: the test bot.
//! **For testing only.**
//!
//! A match needs two players, and one person with one PC has one game.
//! The bot is the second player: it does on the wire what a client does
//! from the login screen to the results screen, again and again, so that
//! a real client gets a real match (level load, world upload, countdown,
//! play, end, results). `srp-bot` (`src/bin/srp-bot.rs`) runs it; so does
//! the end-to-end test (`tests/v026g_bot.rs`).
//!
//! What it does, one printed line a step:
//! 1. signs in at the login server (TCP, then TLS), as a client with a
//!    typed password does; a new name registers itself there;
//! 2. signs in at the lobby (UDP, `2E`) and takes the whole account;
//! 3. presses Play with the kit the account plays (a new account's
//!    starter kit), which puts it into a room (the lobby's `rooms`), and
//!    waits there for the room's match;
//! 4. told where its match is (0x82), connects to the match server with
//!    its session (0xD8), answers the port probe, takes the static info,
//!    and says at once that it has loaded (0xDA): it has no level to load;
//! 5. takes the time (0xE5, answered with 0xDB) and the world (0xE8),
//!    acks everything, answers the pings, sends a keep-alive a second, and
//!    stays until it is told the match is over (0xEA with its own place)
//!    or the link is closed. It sends **no inputs**: its player stands
//!    where the real client's own respawn rule puts it; with `drive` it
//!    runs a fixed script of inputs instead (`srp::drive`): it walks,
//!    turns, fires;
//! 6. back in the lobby (`31 <order> FF`) takes the results screen, and
//!    presses Play again (or stops, with `once`).
//!
//! What it cannot do: simulate the game. It has no world, so it sends no
//! hash report (0xDF) and no world (0xE0 chunks), and makes none up. The
//! server must know that, or it would wait for a second witness for ever:
//! name the bot's account in `SRP_BOT_ACCOUNTS` (`session::v026g::bots`;
//! what the match server then does is at the top of
//! `servers/match-server/src/v026g/game.rs`).
//!
//! The clients it is made of are the scripted clients of the lobby's and
//! the match server's tests (`lobby_server::v026g::scripted`,
//! `match_server::v026g::scripted`): every message it takes goes through
//! the strict client models, and what a real client would have dropped or
//! choked on is printed as a warning.

use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::drive::{self, Drive};
use lobby_server::v026g::scripted::{CLIENT_IDLE, TestClient, request};
use match_server::v026g::scripted::Client as MatchClient;
use survarium::v026g::lobby::ClientState;
use survarium::v026g::lobby::client_model::Decoded;
use survarium::v026g::lobby::requests::id as lobby_id;
use survarium::v026g::lobby::rooms::{
    PIN_MAX, ROOM_JOIN_BASE, ROOM_KEY_MAX, ROOM_PIN_BASE, ROOM_STATE,
};
use survarium::v026g::maps;
use survarium::v026g::match_msgs::ServerMessage;

/// The version a 0.26g client signs in with.
const VERSION: &[u8] = b"0.26g0";
/// The login server's answers.
const TLS_NEXT: u8 = 0x0B;
const SIGNED_IN: u8 = 0x08;
const INVALID: u8 = 0x0A;

/// How long a TCP step of the sign-in may take.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(10);
/// A sign-in that failed for a reason that may pass (srp is not running,
/// or is being restarted) is tried again this often.
const RETRY: Duration = Duration::from_secs(5);
/// The lobby's sign-in pushes come within this, or the link is given up.
const LOBBY_SIGN_IN: Duration = Duration::from_secs(20);
/// A link the server has sent nothing on for this long is dead. (The
/// lobby sends its online counts every few seconds, the match server its
/// pings every second.)
const SILENT: Duration = Duration::from_secs(20);
/// After a match, the results screen is waited for this long.
const RESULTS_WAIT: Duration = Duration::from_secs(15);
/// The pause between the results and the next Play.
const PLAY_AGAIN: Duration = Duration::from_secs(3);
/// A Play the lobby refused is tried again after this.
const PLAY_REFUSED: Duration = Duration::from_secs(30);
/// How often a bot that waits for its room's match asks who is in it.
const ROOM_POLL: Duration = Duration::from_secs(2);
/// How often a bot in a match says that it is still there.
const STATUS_EVERY: Duration = Duration::from_secs(30);
/// How long a pass of the loop sleeps.
const PASS: Duration = Duration::from_millis(5);

/// Where a bot's lines go.
pub type Print = Arc<dyn Fn(&str) + Send + Sync>;

/// An existing room, optionally protected by a PIN. Used by local spawn inspection.
#[derive(Clone, Copy, Debug)]
pub struct RoomTarget {
    key: u32,
    pin: Option<u16>,
}

impl RoomTarget {
    pub fn new(key: u32, pin: Option<u16>) -> Option<Self> {
        ((1..=ROOM_KEY_MAX).contains(&key) && pin.is_none_or(|pin| pin <= PIN_MAX))
            .then_some(Self { key, pin })
    }
}

/// One bot.
#[derive(Clone, Debug)]
pub struct Config {
    /// The login server (`host:port`), as the client's `-server=`.
    pub server: String,
    /// The account's user name; a new one registers itself.
    pub name: String,
    /// Its password. Never printed.
    pub password: String,
    /// Stop after one match.
    pub once: bool,
    /// The kit to press Play with; `None` for the one the account plays.
    pub kit: Option<u32>,
    /// In a match, send the inputs of the stage 0 script (`srp::drive`).
    pub drive: bool,
    /// The patch revision its sign-in says: one that speaks the late join
    /// (5 and later; only such a game plays in rooms).
    pub revision: u8,
    /// Join this room instead of pressing Play.
    pub room: Option<RoomTarget>,
}

/// SHA-1 as the client prints it: 40 lower-case hex characters.
fn sha1_hex(bytes: &[u8]) -> Vec<u8> {
    openssl::sha::sha1(bytes)
        .iter()
        .flat_map(|byte| format!("{byte:02x}").into_bytes())
        .collect()
}

/// What the login server said: the plain answer byte, and past the TLS
/// handshake its reply to the password.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignIn {
    pub answer: u8,
    pub reply: Vec<u8>,
}

impl SignIn {
    /// The session and the lobby's address of a `08` reply; `None` when
    /// the sign-in was refused.
    pub fn accepted(&self) -> Option<(u32, SocketAddr)> {
        if self.answer != TLS_NEXT || self.reply.len() != 11 || self.reply[0] != SIGNED_IN {
            return None;
        }
        let session = u32::from_le_bytes(self.reply[1..5].try_into().ok()?);
        // The IP as a little-endian number (the client passes it through
        // `htonl`), the port as it is.
        let ip = Ipv4Addr::from(u32::from_le_bytes(self.reply[5..9].try_into().ok()?));
        let port = u16::from_le_bytes(self.reply[9..11].try_into().ok()?);
        Some((session, SocketAddr::from((ip, port))))
    }

    /// Why it was refused, in a few words.
    pub fn refusal(&self) -> String {
        match (self.answer, self.reply.first()) {
            (0x22, _) => "the server takes another version of the game".to_owned(),
            (INVALID, _) => "the server does not take this user name".to_owned(),
            (TLS_NEXT, Some(&INVALID)) => "invalid user name or password".to_owned(),
            (TLS_NEXT, Some(code)) => format!("the login server answered {code:#04x}"),
            (TLS_NEXT, None) => "the login server closed the link after the password".to_owned(),
            (answer, _) => format!("the login server answered {answer:#04x}"),
        }
    }
}

/// Sign in as the 0.26g client does with a typed password: the sign-in in
/// the clear, the TLS handshake, the digest and the password. An error is
/// the link's (nobody listening, a timeout); a refusal is a [`SignIn`]
/// that is not [`SignIn::accepted`].
pub fn sign_in(login: SocketAddr, user_name: &str, password: &str) -> io::Result<SignIn> {
    sign_in_with_revision(login, user_name, password, 0)
}

/// Sign in with an explicit client dialect for integration clients.
pub fn sign_in_with_revision(
    login: SocketAddr,
    user_name: &str,
    password: &str,
    revision: u8,
) -> io::Result<SignIn> {
    use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
    let too_long = |what: &str| io::Error::other(format!("the {what} is longer than 255 bytes"));
    let name_len = u8::try_from(user_name.len()).map_err(|_| too_long("user name"))?;
    let password_len = u8::try_from(password.len()).map_err(|_| too_long("password"))?;
    let mut stream = TcpStream::connect_timeout(&login, LOGIN_TIMEOUT)?;
    stream.set_read_timeout(Some(LOGIN_TIMEOUT))?;
    stream.set_write_timeout(Some(LOGIN_TIMEOUT))?;
    let mut message = vec![0x01, name_len];
    message.extend(user_name.as_bytes());
    let mut version = [0_u8; 12];
    version[..VERSION.len()].copy_from_slice(VERSION);
    version[7] = revision;
    message.extend(version);
    message.push(0); // not from the EULA window
    message.extend(u32::MAX.to_le_bytes()); // no online queue order
    stream.write_all(&message)?;
    let mut answer = [0];
    stream.read_exact(&mut answer)?;
    if answer[0] != TLS_NEXT {
        return Ok(SignIn {
            answer: answer[0],
            reply: Vec::new(),
        });
    }
    let mut connector = SslConnector::builder(SslMethod::tls()).map_err(io::Error::other)?;
    // The game's own certificate and key are of 2013.
    connector.set_security_level(0);
    connector.set_verify(SslVerifyMode::NONE);
    let mut tls = connector
        .build()
        .configure()
        .map_err(io::Error::other)?
        .verify_hostname(false)
        .connect("localhost", stream)
        .map_err(|error| io::Error::other(format!("the TLS handshake: {error}")))?;
    let mut message = vec![0x03];
    message.extend(sha1_hex(password.as_bytes()));
    message.push(password_len);
    message.extend(password.as_bytes());
    tls.write_all(&message)?;
    let mut reply = Vec::new();
    // The server closes the link after its reply.
    let _ = tls.read_to_end(&mut reply);
    Ok(SignIn {
        answer: answer[0],
        reply,
    })
}

fn socket() -> io::Result<UdpSocket> {
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_nonblocking(true)?;
    Ok(socket)
}

/// The bot's link to its match.
struct MatchLink {
    addr: SocketAddr,
    socket: UdpSocket,
    client: MatchClient,
    match_id: u32,
    started: Instant,
    heard: Instant,
    ready: bool,
    synced: bool,
    faults_told: usize,
    relays: u64,
    status_at: Instant,
    /// The input script, with `drive`.
    drive: Option<Drive>,
    /// Its inputs the server dropped (0xF1) or moved (0xEF).
    dropped: u32,
    moved: u32,
}

/// Where the bot is.
enum Stage {
    /// Not signed in; the next try is at this time.
    SignedOut(Instant),
    /// The lobby's `2E` is out; its pushes are awaited.
    SigningIn(Instant),
    /// In the lobby; Play is pressed at this time.
    Idle(Instant),
    /// A private-room PIN is sent; wait for its acknowledgement before joining.
    GivingPin { room: RoomTarget, since: Instant },
    /// Play is out: in a room, waiting for its match.
    Waiting,
    InMatch(Box<MatchLink>),
    /// Back from a match: the results screen is awaited since then.
    Results(Instant),
    /// It stopped for good.
    Done,
}

/// The lobby link, from the sign-in on.
struct Lobby {
    /// The login session: the lobby sign-in's. The match takes the `82`'s.
    #[allow(dead_code)]
    session: u32,
    addr: SocketAddr,
    socket: UdpSocket,
    client: TestClient,
    heard: Instant,
    faults_told: usize,
    /// The kit the account plays, and what it was told it has.
    kit: Option<u32>,
    money: Option<(u32, u32)>,
    /// The room it is in and its members, as last told, so that only
    /// changes are printed; and when it last asked.
    room: Option<(u32, u32)>,
    room_asked: Option<Instant>,
    order_id: u32,
    in_lobby: bool,
    /// The results screen (0x97) that came since the last match began, in
    /// a line. (The lobby may send it before the match link hears the
    /// end.)
    results: Option<String>,
}

/// One bot: [`Bot::step`] is one pass of its loop.
pub struct Bot {
    config: Config,
    print: Print,
    stage: Stage,
    lobby: Option<Lobby>,
    /// Others than this bot are in its room (the lobby's last word).
    pub others_in_room: bool,
    /// Matches it was in to their end.
    pub matches: u32,
}

impl Bot {
    pub fn new(config: Config, print: Print) -> Self {
        Self {
            config,
            print,
            stage: Stage::SignedOut(Instant::now()),
            lobby: None,
            others_in_room: false,
            matches: 0,
        }
    }

    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// It stopped for good: after its one match, or refused at the login.
    pub fn is_done(&self) -> bool {
        matches!(self.stage, Stage::Done)
    }

    /// It waits for its room's match.
    pub fn is_waiting(&self) -> bool {
        matches!(self.stage, Stage::Waiting)
    }

    fn say(&self, text: &str) {
        (self.print)(&format!("{}: {text}", self.config.name));
    }

    fn resolve(server: &str) -> io::Result<SocketAddr> {
        server
            .to_socket_addrs()?
            .find(SocketAddr::is_ipv4)
            .ok_or_else(|| io::Error::other(format!("`{server}` has no IPv4 address")))
    }

    /// The whole sign-in: the login server, then the lobby's `2E`.
    fn start(&mut self, now: Instant) {
        self.lobby = None;
        self.others_in_room = false;
        let login = match Self::resolve(&self.config.server) {
            Ok(login) => login,
            Err(error) => {
                self.say(&format!(
                    "the login server {}: {error}; again in {} s",
                    self.config.server,
                    RETRY.as_secs()
                ));
                self.stage = Stage::SignedOut(now + RETRY);
                return;
            }
        };
        let answer = match sign_in_with_revision(
            login,
            &self.config.name,
            &self.config.password,
            self.config.revision,
        ) {
            Ok(answer) => answer,
            Err(error) => {
                self.say(&format!(
                    "no sign-in at {login} ({error}); is srp running? Again in {} s",
                    RETRY.as_secs()
                ));
                self.stage = Stage::SignedOut(now + RETRY);
                return;
            }
        };
        let Some((session, lobby_addr)) = answer.accepted() else {
            // Not tried again: another wrong password would lock the name.
            self.say(&format!(
                "the sign-in at {login} was refused: {}. Stopped",
                answer.refusal()
            ));
            self.stage = Stage::Done;
            return;
        };
        // The login server names the lobby by its public host; a bot that
        // reached srp on this machine stays on the address it used.
        let lobby_addr = if login.ip().is_loopback() {
            SocketAddr::new(login.ip(), lobby_addr.port())
        } else {
            lobby_addr
        };
        let socket = match socket() {
            Ok(socket) => socket,
            Err(error) => {
                self.say(&format!("no UDP socket: {error}"));
                self.stage = Stage::SignedOut(now + RETRY);
                return;
            }
        };
        self.say(&format!(
            "signed in at the login server {login} (session {session:#x}); the lobby is at \
             {lobby_addr}"
        ));
        let mut client = TestClient::new();
        client.idle = CLIENT_IDLE;
        client.sign_in(session, true);
        self.lobby = Some(Lobby {
            session,
            addr: lobby_addr,
            socket,
            client,
            heard: now,
            faults_told: 0,
            kit: None,
            money: None,
            room: None,
            room_asked: None,
            order_id: 1,
            in_lobby: false,
            results: None,
        });
        self.stage = Stage::SigningIn(now);
    }

    /// The lobby link: send what is due, take what came. What it took is
    /// returned, and forgotten by the scripted client (it stays for hours).
    fn pump_lobby(&mut self, now: Instant) -> Vec<Decoded> {
        let Some(lobby) = &mut self.lobby else {
            return Vec::new();
        };
        let mut buffer = [0_u8; 2048];
        for datagram in lobby.client.poll(now) {
            let _ = lobby.socket.send_to(&datagram, lobby.addr);
        }
        while let Ok((len, from)) = lobby.socket.recv_from(&mut buffer) {
            if from == lobby.addr {
                lobby.heard = now;
                lobby.client.receive(&buffer[..len]);
            }
        }
        lobby.client.ids.clear();
        lobby.client.online.clear();
        let decoded = std::mem::take(&mut lobby.client.decoded);
        let faults: Vec<String> = lobby.client.faults[lobby.faults_told..].to_vec();
        lobby.faults_told = lobby.client.faults.len();
        for message in &decoded {
            match message {
                Decoded::Account(own) => {
                    let kits = &own.body.kits;
                    lobby.kit = kits
                        .iter()
                        .find(|kit| kit.id == own.active_kit)
                        .or(kits.first())
                        .map(|kit| kit.id);
                }
                Decoded::Money { generic, premium } => lobby.money = Some((*generic, *premium)),
                Decoded::ClientState { state, .. } => {
                    lobby.in_lobby = *state == ClientState::Lobby;
                    if let ClientState::InQueue { order_id, .. }
                    | ClientState::InMatch { order_id, .. } = state
                    {
                        lobby.order_id = *order_id;
                    }
                }
                Decoded::MatchStats { stats, own } => {
                    let row = stats.rows.iter().find(|row| row.player_id == own.player_id);
                    let winner = match stats.winner_team {
                        team @ (0 | 1) => format!("team {team} won"),
                        _ => "nobody won".to_owned(),
                    };
                    lobby.results = Some(format!(
                        "the results of match {:#x}: {winner}, {} players on the screen{}",
                        stats.match_id,
                        stats.rows.len(),
                        row.map_or(String::new(), |row| format!(
                            "; its own row: team {}, score {}, {} kills, {} deaths",
                            row.team, row.score, row.kills, row.deaths
                        ))
                    ));
                }
                _ => {}
            }
        }
        for fault in faults {
            self.say(&format!(
                "WARNING: the lobby sent what a client would not take: {fault}"
            ));
        }
        decoded
    }

    /// One pass. `may_play` holds a bot in the lobby back from Play (the
    /// extra bots of a run wait for somebody to play with).
    pub fn step(&mut self, may_play: bool) {
        let now = Instant::now();
        if let Stage::SignedOut(at) = self.stage {
            if now >= at {
                self.start(now);
            }
            return;
        }
        if matches!(self.stage, Stage::Done) {
            return;
        }
        let decoded = self.pump_lobby(now);
        let Some(lobby) = &self.lobby else {
            self.stage = Stage::SignedOut(now + RETRY);
            return;
        };
        // The lobby link is the session: without it everything starts
        // again from the login server.
        let lost = if lobby.client.disconnected {
            Some("the lobby closed the link")
        } else if now.saturating_duration_since(lobby.heard) >= SILENT {
            Some("the lobby is silent")
        } else {
            None
        };
        if let Some(why) = lost {
            self.say(&format!(
                "{why} (was srp stopped?); signing in again in {} s",
                RETRY.as_secs()
            ));
            self.lobby = None;
            self.others_in_room = false;
            self.stage = Stage::SignedOut(now + RETRY);
            return;
        }
        let (kit, money, in_lobby) = (lobby.kit, lobby.money, lobby.in_lobby);

        match &mut self.stage {
            Stage::SignedOut(_) | Stage::Done => {}
            Stage::SigningIn(since) => {
                if in_lobby && kit.is_some() {
                    let money = money.map_or(String::new(), |(silver, gold)| {
                        format!(", {silver} silver and {gold} gold")
                    });
                    self.say(&format!(
                        "in the lobby: the account has kit {}{money}",
                        kit.unwrap_or(0)
                    ));
                    self.stage = Stage::Idle(now);
                } else if now.saturating_duration_since(*since) >= LOBBY_SIGN_IN {
                    self.say("the lobby did not send the account; signing in again");
                    self.lobby = None;
                    self.stage = Stage::SignedOut(now + RETRY);
                }
            }
            Stage::Idle(at) => {
                if now >= *at && may_play {
                    let kit = self.config.kit.or(kit).unwrap_or(1);
                    if let Some(lobby) = &mut self.lobby {
                        if let Some(room) = self.config.room {
                            if let Some(pin) = room.pin {
                                let (id, body) =
                                    request::ranking(ROOM_PIN_BASE + u32::from(pin), 1);
                                lobby.client.send(id, &body);
                            }
                            if room.pin.is_none() {
                                let (id, body) = request::ranking(ROOM_JOIN_BASE + room.key, 1);
                                lobby.client.send(id, &body);
                            }
                        } else {
                            let (id, body) = request::play(kit);
                            lobby.client.send(id, &body);
                        }
                        lobby.room = None;
                        lobby.room_asked = None;
                    }
                    if let Some(room) = self.config.room {
                        self.say(&format!(
                            "joining room {} (stationary unless --drive)",
                            room.key
                        ));
                    } else {
                        self.say(&format!("pressed Play with kit {kit}"));
                    }
                    self.stage = match self.config.room {
                        Some(room) if room.pin.is_some() => Stage::GivingPin { room, since: now },
                        _ => Stage::Waiting,
                    };
                }
            }
            Stage::GivingPin { room, since } => {
                if decoded.iter().any(|message| matches!(message, Decoded::EloList(rows)
                    if rows.first().is_some_and(|row| row.1 == ROOM_PIN_BASE && row.0 == "S1;K;1"))) {
                    if let Some(lobby) = &mut self.lobby {
                        let (id, body) = request::ranking(ROOM_JOIN_BASE + room.key, 1);
                        lobby.client.send(id, &body);
                    }
                    self.stage = Stage::Waiting;
                } else if now.saturating_duration_since(*since) >= LOBBY_SIGN_IN {
                    self.say("room PIN acknowledgement timed out; stopped");
                    self.stage = Stage::Done;
                }
            }
            Stage::Waiting => self.waiting(&decoded, now),
            Stage::InMatch(_) => self.in_match(now),
            Stage::Results(since) => {
                let since = *since;
                let screen = self.lobby.as_mut().and_then(|lobby| lobby.results.take());
                if let Some(screen) = screen {
                    self.say(&screen);
                    self.after_results(now);
                } else if now.saturating_duration_since(since) >= RESULTS_WAIT {
                    self.say("no results screen came (a match nobody played has none)");
                    self.after_results(now);
                }
            }
        }
    }

    fn after_results(&mut self, now: Instant) {
        self.matches += 1;
        if self.config.once {
            if let Some(lobby) = &mut self.lobby {
                lobby.client.leave();
                for datagram in lobby.client.poll(now) {
                    let _ = lobby.socket.send_to(&datagram, lobby.addr);
                }
            }
            self.say("one match done; signed out");
            self.lobby = None;
            self.stage = Stage::Done;
        } else {
            self.stage = Stage::Idle(now + PLAY_AGAIN);
        }
    }

    /// Waiting: who is in its room (asked every [`ROOM_POLL`]), then
    /// "match found" and the match's address (0x82).
    fn waiting(&mut self, decoded: &[Decoded], now: Instant) {
        if let Some(lobby) = &mut self.lobby
            && lobby
                .room_asked
                .is_none_or(|asked| now.saturating_duration_since(asked) >= ROOM_POLL)
        {
            let (id, body) = request::ranking(ROOM_STATE, 1);
            lobby.client.send(id, &body);
            lobby.room_asked = Some(now);
        }
        for message in decoded {
            match message {
                Decoded::EloList(rows) if rows.first().is_some_and(|row| row.1 == ROOM_STATE) => {
                    // `S1;R;<key>;..;<members>;..` (`lobby::rooms::room_state`).
                    let fields: Vec<&str> = rows[0].0.split(';').collect();
                    if self.config.room.is_some() && fields.get(1) == Some(&"N")
                        && fields.get(2).is_some_and(|code| *code != "0") {
                        let reason = match fields.get(2).copied() {
                            Some("2") => "room does not exist",
                            Some("3") => "wrong room PIN",
                            Some("4") => "room is full",
                            _ => "room rejected the join",
                        };
                        self.say(&format!("{reason}; stopped (create a 10v10 private room and check its code/PIN)"));
                        self.stage = Stage::Done;
                        return;
                    }
                    let room = (fields.get(1) == Some(&"R"))
                        .then(|| {
                            Some((fields.get(2)?.parse().ok()?, fields.get(11)?.parse().ok()?))
                        })
                        .flatten();
                    self.others_in_room = room.is_some_and(|(_, members)| members >= 2);
                    let last = self.lobby.as_ref().and_then(|lobby| lobby.room);
                    if room.is_some() && last != room {
                        if let Some(lobby) = &mut self.lobby {
                            lobby.room = room;
                        }
                        let (key, members) = room.unwrap_or_default();
                        self.say(&format!(
                            "in room {key}: {members} in it (a match needs two players)"
                        ));
                    }
                }
                Decoded::ClientState {
                    state: ClientState::InQueue { place: 0, .. },
                    ..
                } => self.say("a match is found"),
                Decoded::Denied { op, text } if *op == lobby_id::PLAY => {
                    self.say(&format!(
                        "the lobby refused Play ({text}); again in {} s",
                        PLAY_REFUSED.as_secs()
                    ));
                    self.others_in_room = false;
                    self.stage = Stage::Idle(now + PLAY_REFUSED);
                    return;
                }
                Decoded::ConnectToMatch {
                    host,
                    port,
                    match_id,
                    team,
                    match_session,
                } => {
                    // The game sends the `82`'s session in 0xD8 and 0xD9,
                    // never its login one (lobby.md §2.2).
                    let session = *match_session;
                    self.others_in_room = false;
                    if let Some(lobby) = &mut self.lobby {
                        lobby.results = None;
                    }
                    let lobby_ip = self.lobby.as_ref().map(|lobby| lobby.addr.ip());
                    let ip = host.parse::<IpAddr>().ok().or(lobby_ip);
                    let drive = self.config.drive;
                    let link = ip.zip(socket().ok()).map(|(ip, socket)| {
                        let mut client = if survarium::v026g::match_msgs::late_join::spoken_by(
                            self.config.revision,
                        ) {
                            MatchClient::idle_growing(session)
                        } else {
                            MatchClient::idle(session)
                        };
                        client.connect();
                        MatchLink {
                            addr: SocketAddr::new(ip, *port),
                            socket,
                            client,
                            match_id: *match_id,
                            started: now,
                            heard: now,
                            ready: false,
                            synced: false,
                            faults_told: 0,
                            relays: 0,
                            status_at: now + STATUS_EVERY,
                            drive: drive.then(|| Drive::new(drive::stage_0())),
                            dropped: 0,
                            moved: 0,
                        }
                    });
                    match link {
                        Some(link) => {
                            self.say(&format!(
                                "goes to match {match_id:#x} on team {team} at {}; connecting \
                                 (0xD8)",
                                link.addr
                            ));
                            self.stage = Stage::InMatch(Box::new(link));
                        }
                        None => {
                            self.say(&format!("the match address {host}:{port} is no address"));
                            self.leave_match(now, "it could not connect");
                        }
                    }
                    return;
                }
                _ => {}
            }
        }
    }

    /// Back to the lobby from a match: `31 <order> FF`.
    fn leave_match(&mut self, now: Instant, why: &str) {
        if let Some(lobby) = &mut self.lobby {
            let (id, body) = request::leave(lobby.order_id, true);
            lobby.client.send(id, &body);
        }
        self.say(&format!(
            "{why}; back to the lobby, waiting for the results"
        ));
        self.stage = Stage::Results(now);
    }

    /// The match link: one pass.
    fn in_match(&mut self, now: Instant) {
        let name = self.config.name.clone();
        let print = self.print.clone();
        let say = |text: &str| print(&format!("{name}: {text}"));
        let Stage::InMatch(link) = &mut self.stage else {
            return;
        };
        let ms = now.saturating_duration_since(link.started).as_millis() as u64;
        let mut buffer = [0_u8; 2048];
        for datagram in link.client.poll(ms) {
            let _ = link.socket.send_to(&datagram, link.addr);
        }
        while let Ok((len, from)) = link.socket.recv_from(&mut buffer) {
            if from == link.addr {
                link.heard = now;
                link.client.receive(&buffer[..len]);
            }
        }
        let client = &mut link.client;
        for fault in &client.faults[link.faults_told..] {
            say(&format!(
                "WARNING: the match server sent what a client would not take: {fault}"
            ));
        }
        link.faults_told = client.faults.len();
        let own = client.model.local_index;
        // What it took since the last pass; nothing is kept (a match is
        // long).
        let mut entered = Vec::new();
        let mut left = Vec::new();
        let taken = std::mem::take(&mut client.taken);
        client.skipped.clear();
        let player = |place: u8| -> String {
            client
                .model
                .options
                .as_ref()
                .and_then(|options| options.players.get(usize::from(place)))
                .map_or(format!("player {place}"), |profile| {
                    format!("player {place} ({})", profile.name)
                })
        };
        for message in taken {
            match message {
                ServerMessage::PlayerEntered { player, .. } if !entered.contains(&player) => {
                    entered.push(player);
                }
                ServerMessage::PlayerLeft { player, .. }
                    if Some(player) != own && !left.contains(&player) =>
                {
                    left.push(player);
                }
                ServerMessage::InputRelay { .. } | ServerMessage::ProfileInputRelay { .. } => {
                    link.relays += 1;
                }
                ServerMessage::DiscardInput { time } if link.drive.is_some() => {
                    link.dropped += 1;
                    say(&format!(
                        "WARNING: the server dropped its input of tick {} (0xF1); its clock at {} ms",
                        time.tick,
                        client.clock()
                    ));
                }
                ServerMessage::MoveInput { from, to } if link.drive.is_some() => {
                    link.moved += 1;
                    say(&format!(
                        "WARNING: the server moved its input of tick {} to tick {} (0xEF); its \
                         clock at {} ms",
                        from.tick,
                        to.tick,
                        client.clock()
                    ));
                }
                ServerMessage::Snapshot {
                    generation,
                    time,
                    body,
                    packed,
                } => say(&format!(
                    "took the world (0xE8): generation {generation}, the world at {time} ms, {} \
                     bytes{}",
                    body.len(),
                    if packed { ", sent compressed" } else { "" }
                )),
                _ => {}
            }
        }
        for place in entered {
            if Some(place) == own {
                say("is in the match (0xE9 for its own place): the others see its player");
                if let Some(drive) = link.drive.as_mut().filter(|drive| !drive.started()) {
                    drive.entered(client.clock());
                    say(&format!(
                        "drives: the script starts 2 s after its entry at {} ms",
                        client.clock()
                    ));
                }
            } else {
                say(&format!("{} entered the match", player(place)));
            }
        }
        for place in left {
            say(&format!("{} left the match", player(place)));
        }
        let loaded = client.model.options.as_ref().map(|options| {
            let map = maps::map_mode(options.map_id).map_or(String::new(), |map| {
                format!(" ({}, mode {})", map.name, map.mode)
            });
            format!(
                "took the static info (0xE4): match {:#x}, map {}{map}, {} players, its place \
                 {}; says it has loaded (0xDA)",
                options.match_id,
                options.map_id,
                options.players.len(),
                own.map_or("?".to_owned(), |place| place.to_string())
            )
        });
        if !link.ready
            && let Some(loaded) = loaded
        {
            say(&loaded);
            client.ready();
            link.ready = true;
            say("waits for the time and the world: a real (patched) client must give the first");
        }
        if !link.synced && client.model.is_synced() {
            link.synced = true;
            say(&format!(
                "has the time (0xE5, answered) and the world: its clock is at {} ms. It stays, \
                 sends {}no hashes and no world, to the match's end",
                client.clock(),
                if link.drive.is_some() {
                    "its script's inputs, "
                } else {
                    "no inputs, "
                }
            ));
        }
        if link.synced
            && let Some(drive) = link.drive.as_mut()
        {
            let clock = client.clock();
            for sent in drive.due(clock) {
                client.send_input(sent.input);
                say(&format!(
                    "input for {} ms (tick {}, clock {clock} ms): keys {:#x}{}{}",
                    sent.time,
                    sent.input.time.tick,
                    sent.input.actions,
                    if sent.turn == (0.0, 0.0) {
                        String::new()
                    } else {
                        format!(
                            ", turn yaw {:+.7} pitch {:+.7} (halves {:02x?}), yaw sum {:+.7}",
                            sent.turn.0, sent.turn.1, sent.input.rotation, sent.yaw_sum
                        )
                    },
                    sent.what.map_or(String::new(), |what| format!(": {what}"))
                ));
            }
        }
        if link.synced && now >= link.status_at {
            link.status_at = now + STATUS_EVERY;
            say(&format!(
                "still in match {:#x}: its clock at {} s, {} inputs of the others taken{}",
                link.match_id,
                client.clock() / 1000,
                link.relays,
                if link.drive.is_some() {
                    format!(
                        "; of its own {} dropped, {} moved",
                        link.dropped, link.moved
                    )
                } else {
                    String::new()
                }
            ));
        }
        let over = if client.told_to_leave {
            Some("the match is over (0xEA with its own place)")
        } else if client.disconnected {
            Some("the match server closed the link")
        } else if now.saturating_duration_since(link.heard) >= SILENT {
            Some("the match server is silent")
        } else {
            None
        };
        if let Some(why) = over {
            // The last acks, so that the server need not send the end
            // again.
            for datagram in client.poll(ms + 1) {
                let _ = link.socket.send_to(&datagram, link.addr);
            }
            self.leave_match(now, why);
        }
    }
}

/// Run the bots until `stop` is set or each has stopped by itself (after
/// its one match, or refused at the login server). The first bot presses
/// Play as soon as it is in the lobby and waits in its room for a real
/// player; the others press Play once somebody else is in a waiting bot's
/// room, so that the bots do not make matches among themselves.
pub fn run(configs: Vec<Config>, print: Print, stop: &AtomicBool) {
    let mut bots: Vec<Bot> = configs
        .into_iter()
        .map(|config| Bot::new(config, print.clone()))
        .collect();
    while !stop.load(Ordering::Relaxed) && !bots.iter().all(Bot::is_done) {
        let somebody_waits = bots
            .iter()
            .any(|bot| bot.is_waiting() && bot.others_in_room);
        // The first bot that is not done leads.
        let leader = bots.iter().position(|bot| !bot.is_done());
        for (index, bot) in bots.iter_mut().enumerate() {
            bot.step(bot.config.room.is_some() || Some(index) == leader || somebody_waits);
        }
        std::thread::sleep(PASS);
    }
}

/// The names of `count` bots after `first`: a name that ends in a number
/// counts on from it (`bot-1`, `bot-2`, ...), any other gets `-2`, `-3`.
pub fn names(first: &str, count: usize) -> Vec<String> {
    let digits = first.chars().rev().take_while(char::is_ascii_digit).count();
    let (stem, number) = first.split_at(first.len() - digits);
    match number.parse::<u64>() {
        Ok(number) => (0..count as u64)
            .map(|index| format!("{stem}{}", number + index))
            .collect(),
        Err(_) => (0..count)
            .map(|index| match index {
                0 => first.to_owned(),
                _ => format!("{first}-{}", index + 1),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_targets_require_valid_keys_and_pins() {
        assert!(RoomTarget::new(1, Some(0)).is_some());
        assert!(RoomTarget::new(99_999, Some(9999)).is_some());
        assert!(RoomTarget::new(0, None).is_none());
        assert!(RoomTarget::new(100_000, None).is_none());
        assert!(RoomTarget::new(1, Some(10_000)).is_none());
    }

    #[test]
    fn the_bots_of_a_run_are_numbered_on() {
        assert_eq!(names("bot-1", 3), ["bot-1", "bot-2", "bot-3"]);
        assert_eq!(names("bot-9", 2), ["bot-9", "bot-10"]);
        assert_eq!(names("tester", 2), ["tester", "tester-2"]);
        assert_eq!(names("bot-1", 1), ["bot-1"]);
    }

    #[test]
    fn a_sign_in_reply_is_read_as_the_client_reads_it() {
        let mut reply = vec![SIGNED_IN];
        reply.extend(0x1234_5678_u32.to_le_bytes());
        reply.extend(u32::from(Ipv4Addr::new(127, 0, 0, 1)).to_le_bytes());
        reply.extend(1235_u16.to_le_bytes());
        let accepted = SignIn {
            answer: TLS_NEXT,
            reply,
        };
        assert_eq!(
            accepted.accepted(),
            Some((0x1234_5678, "127.0.0.1:1235".parse().unwrap()))
        );
        let refused = SignIn {
            answer: TLS_NEXT,
            reply: vec![INVALID],
        };
        assert_eq!(refused.accepted(), None);
        assert_eq!(refused.refusal(), "invalid user name or password");
        let version = SignIn {
            answer: 0x22,
            reply: Vec::new(),
        };
        assert_eq!(version.accepted(), None);
    }
}

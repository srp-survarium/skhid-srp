//! The login server of the 0.26g client (build 2777, January 2015), beside
//! 0.21d's in `lib.rs`. What differs (`docs/notes/login_chat.md`
//! §4): the version, the password message (a SHA-1 digest, with the password
//! itself only when it was typed), the answer (the session and the lobby's
//! UDP address, no browser server) and the numbers of the refusals. The
//! client neither pings the login server nor signs out: a session ends when
//! its lobby connection does.

use openssl::ssl::SslContext;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Instant;

use session::{SESSION_IDLE_LIMIT, SessionStore, SignInError};
use vostok::config::{self, ServerConfig};

use super::{
    CERT_PATH, COLLECT_INTERVAL, GONE_CHECK_INTERVAL, PKEY_PATH, accept_tls, parse_sign_in,
    read_message, sign_in_length, tls_context, tls_context_from,
};

/// The version field of a 0.26g sign-in (`sign_in_on_connected` 0x3807D0
/// copies it from 0x9DAE98 into 12 zeroed bytes).
const VERSION: &str = "0.26g0";

/// `network_client::on_connected_to_login` 0x21A6C0 turns each into a login
/// screen status; the names are 0.21d's where the role is the same.
#[repr(u8)]
#[rustfmt::skip]
#[expect(non_camel_case_types)]
#[expect(dead_code)]
#[derive(Clone, Copy)]
enum login_server_message_types_enum {
    servers_connection_info_message_type           = 0x08, // + | signed in: connects to the lobby
    password_request_message_type                  = 0x09, // connection error
    invalid_user_name_or_password_message_type     = 0x0A, // +
    valid_user_name_message_type                   = 0x0B, // + | starts the TLS handshake
    sign_in_attempt_interval_violated_message_type = 0x0C, // +
    // 0x0D..0x11: connection error (0x11 was 0.21d's "user banned").
    ban_reason_bad_language                        = 0x12, // + | "user banned" and the reason
    ban_reason_insult                              = 0x13, // +
    ban_reason_faschism                            = 0x14, // +
    ban_reason_politics                            = 0x15, // +
    ban_reason_flood                               = 0x16, // +
    ban_reason_advertising                         = 0x17, // +
    ban_reason_discuss_admins                      = 0x18, // +
    ban_reason_reallife_treating                   = 0x19, // +
    ban_reason_account_trading                     = 0x1A, // +
    ban_reason_incorrect_info                      = 0x1B, // +
    ban_reason_administrator                       = 0x1C, // +
    ban_reason_nickname                            = 0x1D, // +
    ban_reason_cheating                            = 0x1E, // +
    ban_reason_teamkilling                         = 0x1F, // +
    user_restricted_by_access_level_message_type   = 0x20, // +
    sign_in_user_already_signed_in                 = 0x21, // +
    sign_in_invalid_version                        = 0x22, // +
    sign_in_user_unconfirmed_registration          = 0x23, // +
    sign_in_eula_check_failed                      = 0x24, // shows the EULA window
    sign_in_added_to_online_queue                  = 0x25, // "in online queue"; has a body
    // 0x26..0x2C: the client knows none of them.
    login_server_invalid_message_type              = 0x2D, // the client's own failure code
}

#[repr(u8)]
#[rustfmt::skip]
#[expect(non_camel_case_types)]
#[expect(dead_code)]
enum login_client_message_types_enum {
    sign_in_message_type    = 0x1,
    ping_queue_message_type = 0x3, // online-queue ping (UDP); srp queues nobody
}

/// The password message's kinds (`on_sign_in_handshaked` 0x380160).
#[repr(u8)]
#[rustfmt::skip]
#[expect(non_camel_case_types)]
enum password_message_kind_enum {
    remembered_account = 0x2, // the digest only
    typed_password     = 0x3, // the digest, then the password
}

const SIGN_IN: u8 = login_client_message_types_enum::sign_in_message_type as u8;
const REMEMBERED: u8 = password_message_kind_enum::remembered_account as u8;
const TYPED: u8 = password_message_kind_enum::typed_password as u8;

const CONNECTION_INFO: u8 =
    login_server_message_types_enum::servers_connection_info_message_type as u8;
const VALID_USER_NAME: u8 = login_server_message_types_enum::valid_user_name_message_type as u8;

/// A wrong password, a password message that can't be one, or a user name no
/// account can have: "Invalid user name or password".
const INVALID_USER_NAME_OR_PASSWORD: u8 =
    login_server_message_types_enum::invalid_user_name_or_password_message_type as u8;

/// "Sign-in attempt interval is violated": an address that asked too often,
/// or registered as many accounts as it may for now.
const TOO_MANY_ATTEMPTS: u8 =
    login_server_message_types_enum::sign_in_attempt_interval_violated_message_type as u8;

/// New accounts are not taken: the client's "access level restriction".
const REGISTRATION_CLOSED: u8 =
    login_server_message_types_enum::user_restricted_by_access_level_message_type as u8;

/// Not this server's client, or a Skhid game older than the server takes:
/// the game's "You're using an outdated version of the game.".
const INVALID_VERSION: u8 = login_server_message_types_enum::sign_in_invalid_version as u8;

/// Games older than a patch revision are not let in: srp's
/// `SRP_MIN_PATCH_REVISION`. They
/// are answered before the password, so they get no session and nothing of
/// the lobby, whose sign-in names items an older game may not have.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RevisionGate {
    /// The least patch revision let in; 0, the default, lets every game in.
    pub min: u8,
    /// The user names never refused: srp's test bots (`SRP_BOT_ACCOUNTS`),
    /// whose sign-in says no revision.
    pub exempt: Vec<String>,
}

impl RevisionGate {
    fn refuses(&self, revision: u8, user_name: &str) -> bool {
        revision < self.min
            && !self
                .exempt
                .iter()
                .any(|name| name.eq_ignore_ascii_case(user_name))
    }
}

/// A ban with no reason the client has a text for: "banned by the
/// administration".
const BANNED: u8 = login_server_message_types_enum::ban_reason_administrator as u8;

/// The client's ban reasons, each with the words an admin's reason
/// (`Ban::reason`, free text) is taken for it by. The first row with a word
/// in the reason wins; none: [`BANNED`].
#[rustfmt::skip]
const BAN_REASONS: &[(login_server_message_types_enum, &[&str])] = &[
    (login_server_message_types_enum::ban_reason_cheating,          &["cheat", "hack", "aimbot", "wallhack", "exploit"]),
    (login_server_message_types_enum::ban_reason_teamkilling,       &["teamkill", "team kill", "team-kill"]),
    (login_server_message_types_enum::ban_reason_account_trading,   &["trading", "account sale", "selling"]),
    (login_server_message_types_enum::ban_reason_reallife_treating, &["threat", "treating", "real life", "reallife"]),
    (login_server_message_types_enum::ban_reason_discuss_admins,    &["discuss"]),
    (login_server_message_types_enum::ban_reason_incorrect_info,    &["incorrect", "false info", "misinform"]),
    (login_server_message_types_enum::ban_reason_faschism,          &["fascism", "faschism", "nazi"]),
    (login_server_message_types_enum::ban_reason_politics,          &["politic"]),
    (login_server_message_types_enum::ban_reason_advertising,       &["advert"]),
    (login_server_message_types_enum::ban_reason_flood,             &["flood", "spam"]),
    (login_server_message_types_enum::ban_reason_nickname,          &["nickname", "nick name", "user name", "username"]),
    (login_server_message_types_enum::ban_reason_insult,            &["insult", "abus", "harass"]),
    (login_server_message_types_enum::ban_reason_bad_language,      &["language", "swear", "profan", "obscen"]),
];

/// What a banned player is told: the client's reason closest to the admin's.
fn ban_code(reason: &str) -> u8 {
    let reason = reason.to_lowercase();
    BAN_REASONS
        .iter()
        .find(|(_, words)| words.iter().any(|word| reason.contains(word)))
        .map_or(BANNED, |(code, _)| *code as u8)
}

/// Where a signed-in client is sent: the lobby, which 0.26g reaches over UDP
/// at the address in the answer itself (there is no browser server).
#[derive(Clone, Copy)]
struct Lobby {
    login: &'static ServerConfig,
    lobby: &'static ServerConfig,
    /// The lobby's public host as an address: the answer has no room for a
    /// name. Looked up once, at start.
    public_ip: Ipv4Addr,
}

impl Lobby {
    /// Fails when the lobby's public host is neither an IPv4 address nor a
    /// name with one: no client could be told where the lobby is.
    fn new(login: &'static ServerConfig, lobby: &'static ServerConfig) -> io::Result<Self> {
        let public_ip = resolve_ipv4(&lobby.public_host, lobby.port).ok_or_else(|| {
            io::Error::other(format!(
                "the lobby's public host `{}` (SRP_LOBBY_PUBLIC_HOST or SRP_PUBLIC_HOST) is not \
                 an IPv4 address and no IPv4 address was found for it; the 0.26g client is told \
                 the lobby's address as 4 bytes",
                lobby.public_host
            ))
        })?;
        Ok(Self {
            login,
            lobby,
            public_ip,
        })
    }

    /// The lobby's address for a client at `peer` that reached the login
    /// server at `reached`: see `ServerConfig::host_for`.
    fn address_for(&self, peer: IpAddr, reached: Option<IpAddr>) -> (Ipv4Addr, u16) {
        let ip = reached
            .map(|reached| self.lobby.host_for(self.login, peer, reached))
            .and_then(|host| host.parse().ok())
            .unwrap_or(self.public_ip);
        (ip, self.lobby.port)
    }
}

/// `host` as an IPv4 address: itself, or the first one its name has.
fn resolve_ipv4(host: &str, port: u16) -> Option<Ipv4Addr> {
    host.parse().ok().or_else(|| {
        (host, port)
            .to_socket_addrs()
            .ok()?
            .find_map(|address| match address {
                SocketAddr::V4(address) => Some(*address.ip()),
                SocketAddr::V6(_) => None,
            })
    })
}

/// The 0.26g login server: sign-ins on the login server's TCP address, each
/// answered with a session and the lobby's UDP address.
pub fn run(store: Arc<SessionStore>) -> io::Result<()> {
    // Once, so a missing or bad certificate stops the server here rather
    // than failing every sign-in.
    let tls = Arc::new(tls_context().map_err(|error| {
        io::Error::other(format!(
            "cannot load {CERT_PATH} and {PKEY_PATH} (run srp from the repo root): {error}"
        ))
    })?);
    let config = config::get();
    let lobby = Lobby::new(&config.login_server, &config.lobby_server)?;
    let listener = TcpListener::bind(config.login_server.bind_addr())?;
    accept(&listener, store, tls, lobby, RevisionGate::default())
}

/// Where a login server started with [`serve`] sends its clients, and the
/// certificate it shows them: what [`run`] takes from `vostok::config` and
/// `./certs`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    /// The login server's own public host: a client on this machine's
    /// network is told the address it reached srp at for the lobby too
    /// when both have one public host (`ServerConfig::host_for`).
    pub public_host: String,
    /// The lobby's public host (an IPv4 address or a name with one) and
    /// UDP port.
    pub lobby_host: String,
    pub lobby_port: u16,
    pub local_peers: bool,
    /// The login server's private key and certificate (PEM).
    pub key: std::path::PathBuf,
    pub certificate: std::path::PathBuf,
    /// The games refused for their patch revision (the default: none).
    pub revision_gate: RevisionGate,
}

/// [`run`] on a listener already bound (a caller that picks the address
/// itself: `servers/srp`, and its tests on a free port).
pub fn serve(
    listener: &TcpListener,
    store: Arc<SessionStore>,
    endpoint: &Endpoint,
) -> io::Result<()> {
    let tls = tls_context_from(
        &endpoint.key.to_string_lossy(),
        &endpoint.certificate.to_string_lossy(),
    )
    .map_err(|error| {
        io::Error::other(format!(
            "cannot load {} and {}: {error}",
            endpoint.certificate.display(),
            endpoint.key.display()
        ))
    })?;
    let port = listener.local_addr()?.port();
    // The two addresses live as long as the process, as the configured
    // ones do: every connection's thread reads them.
    let server = |public_host: &str, port: u16| -> &'static ServerConfig {
        Box::leak(Box::new(ServerConfig {
            bind_host: String::new(),
            public_host: public_host.to_owned(),
            port,
            local_peers: endpoint.local_peers,
        }))
    };
    let lobby = Lobby::new(
        server(&endpoint.public_host, port),
        server(&endpoint.lobby_host, endpoint.lobby_port),
    )?;
    accept(
        listener,
        store,
        Arc::new(tls),
        lobby,
        endpoint.revision_gate.clone(),
    )
}

/// Every connection of `listener`, each on a thread of its own.
fn accept(
    listener: &TcpListener,
    store: Arc<SessionStore>,
    tls: Arc<SslContext>,
    lobby: Lobby,
    gate: RevisionGate,
) -> io::Result<()> {
    log::info!(
        "0.26g sign-ins on TCP {}; the lobby is at UDP {}:{}",
        listener.local_addr()?,
        lobby.public_ip,
        lobby.lobby.port
    );
    if gate.min > 0 {
        log::info!(
            "Sign-ins of games older than patch revision {} are refused",
            gate.min
        );
    }
    let gate = Arc::new(gate);

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            // A failed `accept` shouldn't take down the whole server.
            Err(error) => {
                log::error!("Failed to accept connection: {error}");
                continue;
            }
        };
        let (store, tls, gate) = (store.clone(), tls.clone(), gate.clone());
        std::thread::spawn(move || {
            if std::panic::catch_unwind(|| handle_client(&store, &tls, lobby, &gate, stream))
                .is_err()
            {
                log::error!("Connection handler panicked; dropped the connection");
            }
        });
    }
    Ok(())
}

/// End the sessions nothing was seen of for too long, as 0.21d's
/// `run_presence` does between its pings. The 0.26g client sends the login
/// server nothing once signed in, so a session is seen only through its
/// lobby and match connections (`SessionStore::hold`, `touch`,
/// `seen_in_match`); one whose client never reached the lobby ends here.
pub fn run_collector(store: Arc<SessionStore>) {
    let mut next_collect = Instant::now() + COLLECT_INTERVAL;
    loop {
        std::thread::sleep(GONE_CHECK_INTERVAL);
        let gone = store.drop_gone_players(Instant::now());
        if !gone.is_empty() {
            log::info!(
                "{} game(s) gone without signing out (nothing from them for {} s): {gone:#x?}",
                gone.len(),
                session::GONE_AFTER.as_secs()
            );
        }
        if Instant::now() >= next_collect {
            next_collect = Instant::now() + COLLECT_INTERVAL;
            let ended = store.end_idle_sessions(Instant::now(), SESSION_IDLE_LIMIT);
            if !ended.is_empty() {
                log::info!(
                    "Ended {} session(s) unseen for {} s: {ended:#x?}",
                    ended.len(),
                    SESSION_IDLE_LIMIT.as_secs()
                );
            }
        }
    }
}

fn handle_client(
    store: &SessionStore,
    tls: &SslContext,
    lobby: Lobby,
    gate: &RevisionGate,
    mut stream: TcpStream,
) {
    let mut request_type = [0_u8];
    if let Err(error) = io::Read::read_exact(&mut stream, &mut request_type) {
        log::info!("Login connection closed before its request: {error}");
        return;
    }
    // The 0.26g client connects only to sign in: it signs out without a word
    // (`login_client_impl::sign_out` 0x385860 just closes).
    if request_type[0] == SIGN_IN {
        handle_sign_in(store, tls, lobby, gate, stream);
    } else {
        log::warn!(
            "Unknown login request type {}; dropped the connection",
            request_type[0]
        );
    }
}

/// The password message, after the TLS handshake: its kind, the digest, and
/// behind kind 3 the password with its u8 length. A kind the client has not
/// is one byte long: `parse_password` refuses it.
fn password_length(buffer: &[u8]) -> Option<usize> {
    match *buffer.first()? {
        REMEMBERED => Some(1 + DIGEST_LEN),
        TYPED => Some(1 + DIGEST_LEN + 1 + usize::from(*buffer.get(1 + DIGEST_LEN)?)),
        _ => Some(1),
    }
}

/// The digest: SHA-1 as 40 hex characters (`session::password::is_sha1_digest`).
const DIGEST_LEN: usize = 40;
/// The longest password the client sends: its buffer is 48 bytes,
/// zero-terminated (`login_client_impl::sign_in` 0x3809DC).
const MAX_PASSWORD_LEN: usize = 47;

/// What is wrong with a password message.
#[derive(Debug, PartialEq)]
enum PasswordError {
    /// No kind the client sends.
    UnknownKind(u8),
    /// Not 40 lower-case hex characters.
    MalformedDigest,
    /// Longer than the client's buffer holds.
    PasswordTooLong,
    /// The digest is not the SHA-1 of the password sent with it.
    DigestMismatch,
}

/// The digest of a whole password message, as `password_length` measured it.
/// The digest is all the client has of a remembered account, so it alone is
/// the account's secret; a typed password comes with it, and must be the one
/// the digest is of (`login_client::sign_in` 0x376957: standard SHA-1 of the
/// password's bytes, the five words printed `%08x` by 0x3761F0).
fn parse_password(message: &[u8]) -> Result<&[u8], PasswordError> {
    let (&kind, rest) = message.split_first().unwrap_or((&0, &[]));
    if kind != REMEMBERED && kind != TYPED {
        return Err(PasswordError::UnknownKind(kind));
    }
    let (digest, rest) = rest
        .split_at_checked(DIGEST_LEN)
        .ok_or(PasswordError::MalformedDigest)?;
    if !session::password::is_sha1_digest(digest) {
        return Err(PasswordError::MalformedDigest);
    }
    if kind == TYPED {
        let password = rest.get(1..).unwrap_or_default();
        if password.len() > MAX_PASSWORD_LEN {
            return Err(PasswordError::PasswordTooLong);
        }
        if sha1_hex(password) != digest {
            return Err(PasswordError::DigestMismatch);
        }
    }
    Ok(digest)
}

/// SHA-1 as the client prints it: 40 lower-case hex characters.
fn sha1_hex(bytes: &[u8]) -> [u8; DIGEST_LEN] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hex = [0; DIGEST_LEN];
    for (pair, byte) in hex.chunks_mut(2).zip(openssl::sha::sha1(bytes)) {
        pair[0] = HEX[usize::from(byte >> 4)];
        pair[1] = HEX[usize::from(byte & 0xF)];
    }
    hex
}

/// The answer to a right password (`on_sign_in_answer_received` 0x37FD26):
/// the session, then where the lobby is, `[u32 ip][u16 port]`. The client
/// reads the IP as a number and passes it through `htonl` (WS2_32 ordinal 8,
/// the import at 0xAEFFF4) into the address, so on the wire it is the IP as
/// a little-endian number: 127.0.0.1 is `01 00 00 7F`. The port is used as
/// read.
fn connection_info(session_id: u32, ip: Ipv4Addr, port: u16) -> [u8; 11] {
    let mut reply = [CONNECTION_INFO; 11];
    reply[1..5].copy_from_slice(&session_id.to_le_bytes());
    reply[5..9].copy_from_slice(&u32::from(ip).to_le_bytes());
    reply[9..].copy_from_slice(&port.to_le_bytes());
    reply
}

/// What the client is told of a refused sign-in. A wrong password, a name
/// locked out for too many of them and a name no account can have get the
/// same answer, so a guesser learns nothing more.
fn refusal_code(store: &SessionStore, user_name: &str, error: SignInError) -> u8 {
    match error {
        SignInError::WrongPassword
        | SignInError::TooManyAttempts
        | SignInError::InvalidUserName => INVALID_USER_NAME_OR_PASSWORD,
        SignInError::TooManyFromAddress | SignInError::RegistrationLimit => TOO_MANY_ATTEMPTS,
        SignInError::RegistrationsClosed => REGISTRATION_CLOSED,
        // A ban lifted this very moment has no reason left to tell.
        SignInError::Banned => store
            .account_id_by_name(user_name)
            .and_then(|account_id| store.account_ban(account_id))
            .map_or(BANNED, |ban| ban_code(&ban.reason)),
    }
}

fn handle_sign_in(
    store: &SessionStore,
    tls: &SslContext,
    lobby: Lobby,
    gate: &RevisionGate,
    mut stream: TcpStream,
) {
    let mut buffer = [0; 1024];
    let peer = match stream.peer_addr() {
        Ok(peer) => peer,
        Err(error) => {
            log::info!("Sign-in dropped: {error}");
            return;
        }
    };
    let reached = stream.local_addr().ok().map(|reached| reached.ip());
    let len = match read_message(&mut stream, &mut buffer, sign_in_length) {
        Ok(len) => len,
        Err(error) => {
            log::warn!("Unreadable sign-in from {peer}: {error}");
            return;
        }
    };
    let sign_in = parse_sign_in(&buffer[..len]);
    let user_name = sign_in.user_name;
    log::info!("User name: {user_name}");
    log::info!(
        "Client version: {}, patch revision {}, cheat flags {:#04x} (from the EULA window: {}, \
         queue order: {:#x})",
        sign_in.version,
        sign_in.patch_revision,
        sign_in.cheat_flags,
        sign_in.from_eula_window,
        sign_in.queue_order_id
    );
    // The cheat check of client_patch.md section 3.25: revision 13 and later
    // report in byte 8 whether they saw a stale shell_32.dll in %TEMP%, a
    // file one known cheat drops and never removes. Only logged.
    if sign_in.cheat_flags & 0x01 != 0 {
        log::warn!("Cheat check: {user_name} from {peer} has a stale shell_32.dll in %TEMP%");
    }

    // Another build's client would misread everything after this.
    if sign_in.version != VERSION {
        log::warn!(
            "Sign-in of {user_name} from {peer} refused: client version {:?}, not {VERSION}",
            sign_in.version
        );
        let _ = stream.write_all(&[INVALID_VERSION]);
        return;
    }
    // A Skhid game older than the server takes. The player is sent back to
    // the login screen with the game's "outdated version", restarts through
    // the launcher, which updates the game, and signs in again.
    if gate.refuses(sign_in.patch_revision, &user_name) {
        log::info!(
            "Sign-in of {user_name} from {peer} refused: patch revision {}, the server takes {} \
             and later",
            sign_in.patch_revision,
            gate.min
        );
        let _ = stream.write_all(&[INVALID_VERSION]);
        return;
    }
    // Nobody signs in, or registers, from a banned address: the game's own
    // "user banned" answer, before the password.
    if let Some(ban) = store.ip_ban(peer.ip()) {
        log::warn!(
            "Sign-in of {user_name} from banned address {peer} ({}) refused",
            ban.net
        );
        let _ = stream.write_all(&[ban_code(&ban.ban.reason)]);
        return;
    }
    // A name no account can have is refused before the password, as the
    // original server answered an unknown user name.
    if session::persist::user_name_key(&user_name).is_none() {
        log::info!("Sign-in from {peer} refused: empty or over-long user name");
        let _ = stream.write_all(&[INVALID_USER_NAME_OR_PASSWORD]);
        return;
    }
    if let Err(error) = stream.write_all(&[VALID_USER_NAME]) {
        log::warn!("Sign-in of {user_name} dropped: {error}");
        return;
    }

    let Some(mut ssl_stream) = accept_tls(tls, stream) else {
        return;
    };
    // Neither the password nor its digest is ever logged; only a hash of
    // the digest is kept.
    let mut buffer = [0; 1 + DIGEST_LEN + 1 + 255];
    let len = match read_message(&mut ssl_stream, &mut buffer, password_length) {
        Ok(len) => len,
        Err(error) => {
            log::warn!("Sign-in of {user_name} dropped before the password: {error}");
            return;
        }
    };
    let digest = match parse_password(&buffer[..len]) {
        Ok(digest) => digest,
        Err(error) => {
            log::warn!("Sign-in of {user_name} from {peer} refused: {error:?}");
            let _ = ssl_stream.write_all(&[INVALID_USER_NAME_OR_PASSWORD]);
            return;
        }
    };
    // The account (made here on a first sign-in) and the session come only
    // once the digest is right.
    let signed_in =
        match store.sign_in_with_digest_from(&user_name, digest, peer.ip(), Instant::now()) {
            Ok(signed_in) => signed_in,
            Err(error) => {
                match error {
                    SignInError::WrongPassword => log::info!("Wrong password for {user_name}"),
                    SignInError::TooManyAttempts => log::info!(
                        "Wrong password for {user_name}: too many in a row, refused for a while"
                    ),
                    SignInError::InvalidUserName => log::info!("Invalid user name {user_name}"),
                    SignInError::TooManyFromAddress | SignInError::RegistrationLimit => {
                        log::warn!("Sign-in of {user_name} from {peer} refused: {error:?}");
                    }
                    SignInError::RegistrationsClosed => log::info!(
                        "Sign-in of new user name {user_name} from {peer}: registration is closed"
                    ),
                    SignInError::Banned => {
                        log::warn!("Sign-in of banned account {user_name} from {peer} refused");
                    }
                }
                let _ = ssl_stream.write_all(&[refusal_code(store, &user_name, error)]);
                return;
            }
        };
    let account_id = signed_in.account_id();
    // None: the admin reset the password while it was checked. Only a
    // sign-in that got its session is where the account last signed in.
    let Some(session_id) = store.create_signed_in_session(signed_in, sign_in.patch_revision) else {
        log::warn!("Sign-in of {user_name} refused: its password was reset meanwhile");
        let _ = ssl_stream.write_all(&[INVALID_USER_NAME_OR_PASSWORD]);
        return;
    };
    store.signed_in_from(account_id, peer.ip());
    let (lobby_ip, lobby_port) = lobby.address_for(peer.ip(), reached);
    log::info!(
        "Sign-in: account={account_id:#x} session={session_id:#x}, \
         sent to the lobby at UDP {lobby_ip}:{lobby_port}"
    );

    // The client reads the reply with one read of at most 82 bytes.
    let reply = connection_info(session_id, lobby_ip, lobby_port);
    if let Err(error) = ssl_stream.write_all(&reply) {
        log::warn!("Sign-in for session {session_id:#x} dropped before the reply: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::super::tls_context_from;
    use super::*;
    use std::io::Read;

    /// SHA-1 of `test-password-1`, and a digest of no password used here.
    const DIGEST_1: &[u8; 40] = b"f5358b98af5ac658c4a219f518bd0fa5ed2ffd65";
    const DIGEST_2: &[u8; 40] = b"0000000000000000000000000000000000000000";

    /// A 0.26g sign-in after its type byte, as `sign_in_on_connected` writes it.
    fn sign_in_message(email: &[u8], version: &[u8], eula: u8, queue: u32) -> Vec<u8> {
        let mut sign_in = vec![email.len() as u8];
        sign_in.extend(email);
        let mut field = [0; 12];
        field[..version.len()].copy_from_slice(version);
        sign_in.extend(field);
        sign_in.push(eula);
        sign_in.extend(queue.to_le_bytes());
        sign_in
    }

    /// The password message of a remembered account.
    fn remembered(digest: &[u8]) -> Vec<u8> {
        let mut message = vec![0x02];
        message.extend(digest);
        message
    }

    /// The password message of a typed password, with the digest the client
    /// makes of it.
    fn typed(password: &[u8]) -> Vec<u8> {
        typed_with(&sha1_hex(password), password)
    }

    fn typed_with(digest: &[u8], password: &[u8]) -> Vec<u8> {
        let mut message = vec![0x03];
        message.extend(digest);
        message.push(password.len() as u8);
        message.extend(password);
        message
    }

    fn server(public_host: &str, port: u16, local_peers: bool) -> &'static ServerConfig {
        Box::leak(Box::new(ServerConfig {
            bind_host: "127.0.0.1".into(),
            public_host: public_host.into(),
            port,
            local_peers,
        }))
    }

    /// The lobby of a default srp: on this machine, UDP 1235.
    fn local_lobby() -> Lobby {
        Lobby::new(
            server("127.0.0.1", 1234, true),
            server("127.0.0.1", 1235, true),
        )
        .unwrap()
    }

    /// A 0.26g login server on a free local port, serving `store`.
    fn start(store: Arc<SessionStore>, lobby: Lobby) -> SocketAddr {
        start_gated(store, lobby, RevisionGate::default())
    }

    /// [`start`], refusing the games `gate` refuses.
    fn start_gated(store: Arc<SessionStore>, lobby: Lobby, gate: RevisionGate) -> SocketAddr {
        let certs = concat!(env!("CARGO_MANIFEST_DIR"), "/../../certs");
        let tls = tls_context_from(
            &format!("{certs}/survarium_login_server.key"),
            &format!("{certs}/survarium_login_server.crt"),
        )
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                handle_client(&store, &tls, lobby, &gate, stream);
            }
        });
        addr
    }

    /// Sign in the way the 0.26g client does; returns the plain answer and,
    /// past the TLS handshake, the reply to the password message.
    fn sign_in_as(
        addr: SocketAddr,
        version: &[u8],
        user_name: &[u8],
        password_message: &[u8],
    ) -> (u8, Vec<u8>) {
        use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
        let mut stream = TcpStream::connect(addr).unwrap();
        let mut message = vec![0x01];
        message.extend(sign_in_message(user_name, version, 0, u32::MAX));
        stream.write_all(&message).unwrap();
        let mut answer = [0];
        stream.read_exact(&mut answer).unwrap();
        if answer[0] != 0x0B {
            // Nothing follows a refusal.
            let mut rest = Vec::new();
            let _ = stream.read_to_end(&mut rest);
            assert!(rest.is_empty());
            return (answer[0], Vec::new());
        }
        let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
        connector.set_security_level(0);
        connector.set_verify(SslVerifyMode::NONE);
        let mut tls = connector
            .build()
            .configure()
            .unwrap()
            .verify_hostname(false)
            .connect("localhost", stream)
            .unwrap();
        tls.write_all(password_message).unwrap();
        let mut reply = Vec::new();
        let _ = tls.read_to_end(&mut reply);
        (answer[0], reply)
    }

    fn sign_in(addr: SocketAddr, user_name: &[u8], password_message: &[u8]) -> (u8, Vec<u8>) {
        sign_in_as(addr, b"0.26g0", user_name, password_message)
    }

    /// The whole answer to a right password: session `session`, the lobby at
    /// `ip` (as on the wire) and UDP 1235.
    fn connection_info_bytes(session: u32, ip: [u8; 4]) -> Vec<u8> {
        let mut reply = vec![0x08];
        reply.extend(session.to_le_bytes());
        reply.extend(ip);
        reply.extend([0xD3, 0x04]);
        reply
    }

    /// The session of a 0x08 answer.
    fn session_of(reply: &[u8]) -> u32 {
        assert_eq!(reply.len(), 11, "{reply:02X?}");
        assert_eq!(reply[0], 0x08);
        u32::from_le_bytes(reply[1..5].try_into().unwrap())
    }

    #[test]
    fn late_join_revision_is_retained_for_the_lobby() {
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone(), local_lobby());
        let (_, reply) = sign_in_as(
            addr,
            b"0.26g0\0\x80",
            b"revision-user",
            &typed(b"test-password-1"),
        );
        assert_eq!(store.client_revision(session_of(&reply)), Some(0x80));
        let (_, reply) = sign_in(addr, b"stock-revision-user", &typed(b"test-password-1"));
        assert_eq!(store.client_revision(session_of(&reply)), Some(0));
    }

    #[test]
    fn the_message_numbers_are_the_0_26g_clients() {
        use login_server_message_types_enum as server;
        assert_eq!(server::ban_reason_bad_language as u8, 0x12);
        assert_eq!(server::ban_reason_administrator as u8, 0x1C);
        assert_eq!(server::ban_reason_teamkilling as u8, 0x1F);
        assert_eq!(
            [
                CONNECTION_INFO,
                INVALID_USER_NAME_OR_PASSWORD,
                VALID_USER_NAME,
                TOO_MANY_ATTEMPTS,
                BANNED,
                REGISTRATION_CLOSED,
                server::sign_in_user_already_signed_in as u8,
                INVALID_VERSION,
                server::sign_in_eula_check_failed as u8,
            ],
            [0x08, 0x0A, 0x0B, 0x0C, 0x1C, 0x20, 0x21, 0x22, 0x24]
        );
    }

    #[test]
    fn a_digest_is_the_sha1_of_the_password_in_lower_case_hex() {
        assert_eq!(&sha1_hex(b""), b"da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(
            &sha1_hex(b"abc"),
            b"a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(&sha1_hex(b"test-password-1"), DIGEST_1);
    }

    #[test]
    fn a_password_message_is_read_whole_and_checked() {
        // Kind 2 is 41 bytes, kind 3 says how long it is, anything else is
        // cut at its kind.
        assert_eq!(password_length(&[]), None);
        assert_eq!(password_length(&[0x02]), Some(41));
        assert_eq!(password_length(&typed(b"test-password-1")[..41]), None);
        assert_eq!(
            password_length(&typed(b"test-password-1")[..42]),
            Some(42 + 15)
        );
        assert_eq!(password_length(&[0x01, 0x02]), Some(1));

        assert_eq!(parse_password(&remembered(DIGEST_1)), Ok(&DIGEST_1[..]));
        assert_eq!(
            parse_password(&typed(b"test-password-1")),
            Ok(&DIGEST_1[..])
        );
        // The longest password the client sends, and one byte more.
        assert!(parse_password(&typed(&[b'a'; 47])).is_ok());
        assert_eq!(
            parse_password(&typed(&[b'a'; 48])),
            Err(PasswordError::PasswordTooLong)
        );
        assert_eq!(
            parse_password(&typed_with(DIGEST_1, b"test-password-2")),
            Err(PasswordError::DigestMismatch)
        );
        // 0.21d's message (a length, then the password) is no 0.26g one.
        assert_eq!(parse_password(&[4]), Err(PasswordError::UnknownKind(4)));
        for digest in [
            &b"F5358B98AF5AC658C4A219F518BD0FA5ED2FFD65"[..],
            b"f5358b98af5ac658c4a219f518bd0fa5ed2ffd6g",
            b"f5358b98af5ac658c4a219f518bd0fa5ed2ffd6\0",
        ] {
            assert_eq!(
                parse_password(&remembered(digest)),
                Err(PasswordError::MalformedDigest)
            );
        }
        assert_eq!(
            parse_password(&remembered(&DIGEST_1[..39])),
            Err(PasswordError::MalformedDigest)
        );
    }

    #[test]
    fn the_reply_says_the_session_and_where_the_lobby_is_as_the_client_reads_it() {
        assert_eq!(
            connection_info(0x1234_5678, Ipv4Addr::LOCALHOST, 1235),
            [
                0x08, 0x78, 0x56, 0x34, 0x12, 0x01, 0x00, 0x00, 0x7F, 0xD3, 0x04
            ]
        );
        assert_eq!(
            connection_info(0x7FFF_FFFF, Ipv4Addr::new(203, 0, 113, 67), 0xABCD),
            [0x08, 0xFF, 0xFF, 0xFF, 0x7F, 67, 113, 0, 203, 0xCD, 0xAB]
        );
    }

    #[test]
    fn the_lobby_address_is_the_one_the_client_can_reach() {
        let ip = |text: &str| text.parse::<IpAddr>().unwrap();
        let lobby = Lobby::new(
            server("203.0.113.67", 1234, true),
            server("203.0.113.67", 1235, true),
        )
        .unwrap();
        // srp's own machine and network: the address the client reached.
        assert_eq!(
            lobby.address_for(ip("127.0.0.1"), Some(ip("127.0.0.1"))),
            (Ipv4Addr::LOCALHOST, 1235)
        );
        assert_eq!(
            lobby.address_for(ip("192.168.50.20"), Some(ip("192.168.50.134"))),
            (Ipv4Addr::new(192, 168, 50, 134), 1235)
        );
        // Through the internet, or reached nobody knows where: the public one.
        let public = (Ipv4Addr::new(203, 0, 113, 67), 1235);
        assert_eq!(
            lobby.address_for(ip("198.51.100.47"), Some(ip("192.168.50.134"))),
            public
        );
        assert_eq!(lobby.address_for(ip("127.0.0.1"), None), public);

        // A name is looked up once, at start; one without an address stops
        // the server there.
        let named = Lobby::new(
            server("localhost", 1234, true),
            server("localhost", 1235, true),
        );
        assert_eq!(named.unwrap().public_ip, Ipv4Addr::LOCALHOST);
        let error = Lobby::new(
            server("127.0.0.1", 1234, true),
            server("no-such-host.invalid", 1235, true),
        )
        .err()
        .unwrap();
        assert!(
            error.to_string().contains("no-such-host.invalid"),
            "{error}"
        );
    }

    #[test]
    fn a_ban_reason_becomes_the_clients_closest_one() {
        for (reason, code) in [
            ("", 0x1C),
            ("test", 0x1C),
            ("<aimbot>", 0x1E),
            ("Cheating in ranked matches", 0x1E),
            ("teamkilling", 0x1F),
            ("bad language", 0x12),
            ("insults in chat", 0x13),
            ("nazi symbols", 0x14),
            ("politics", 0x15),
            ("chat flood", 0x16),
            ("advertising", 0x17),
            ("discussing the admins", 0x18),
            ("real life threats", 0x19),
            ("account trading", 0x1A),
            ("incorrect info", 0x1B),
            ("offensive nickname", 0x1D),
        ] {
            assert_eq!(ban_code(reason), code, "{reason}");
        }
        // Every reason is one the client has a text for.
        for (code, _) in BAN_REASONS {
            assert!((0x12..=0x1F).contains(&(*code as u8)));
        }
    }

    #[test]
    fn each_refusal_gets_its_0_26g_number() {
        let store = SessionStore::new();
        let code = |error| refusal_code(&store, "nobody", error);
        assert_eq!(code(SignInError::WrongPassword), 0x0A);
        assert_eq!(code(SignInError::TooManyAttempts), 0x0A);
        assert_eq!(code(SignInError::InvalidUserName), 0x0A);
        assert_eq!(code(SignInError::TooManyFromAddress), 0x0C);
        assert_eq!(code(SignInError::RegistrationLimit), 0x0C);
        assert_eq!(code(SignInError::RegistrationsClosed), 0x20);
        // No ban to read a reason from: the plain one.
        assert_eq!(code(SignInError::Banned), 0x1C);

        // An address that registered its fill (loopback never has: the limit
        // is asked as the login server asks it, from elsewhere).
        let from = "203.0.113.7".parse::<IpAddr>().unwrap();
        let now = Instant::now();
        let mut last = Err(SignInError::InvalidUserName);
        for name in ["one", "two", "three", "four"] {
            last = store.sign_in_with_digest_from(name, DIGEST_1, from, now);
        }
        assert_eq!(last, Err(SignInError::RegistrationLimit));
        assert_eq!(refusal_code(&store, "four", last.unwrap_err()), 0x0C);
    }

    #[test]
    fn a_first_sign_in_registers_and_either_kind_signs_in_again() {
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone(), local_lobby());

        // Registered by a typed password, then remembered: the digest alone.
        let (answer, reply) = sign_in(addr, b"e2e-user", &typed(b"test-password-1"));
        assert_eq!(answer, 0x0B);
        let first = session_of(&reply);
        assert_eq!(
            reply,
            connection_info_bytes(first, [0x01, 0x00, 0x00, 0x7F])
        );
        let account = store.account_id_by_name("e2e-user").unwrap();
        assert_eq!(
            store.account_for_session(first).unwrap().lock().unwrap().id,
            account
        );

        let (answer, reply) = sign_in(addr, b"E2E-User", &remembered(DIGEST_1));
        assert_eq!(answer, 0x0B);
        let second = session_of(&reply);
        assert_ne!(first, second);
        assert_eq!(
            reply,
            connection_info_bytes(second, [0x01, 0x00, 0x00, 0x7F])
        );
        assert_eq!(store.session_count(), 2);

        // Registered by a remembered account, then typed.
        let (_, reply) = sign_in(addr, b"other@e2e.invalid", &remembered(DIGEST_1));
        session_of(&reply);
        let (_, reply) = sign_in(addr, b"other@e2e.invalid", &typed(b"test-password-1"));
        session_of(&reply);
        assert_ne!(store.account_id_by_name("other@e2e.invalid"), Some(account));
        assert_eq!(store.session_count(), 4);

        // Only an argon2id hash of the digest is kept.
        let hash = store
            .account(account)
            .unwrap()
            .lock()
            .unwrap()
            .password_hash
            .clone()
            .unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(!hash.contains(std::str::from_utf8(DIGEST_1).unwrap()));
    }

    #[test]
    fn a_wrong_password_or_a_bad_password_message_gets_no_session() {
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone(), local_lobby());
        let (_, reply) = sign_in(addr, b"e2e-user", &typed(b"test-password-1"));
        session_of(&reply);

        // A wrong password, typed or remembered.
        let (answer, reply) = sign_in(addr, b"e2e-user", &typed(b"test-password-2"));
        assert_eq!((answer, reply), (0x0B, vec![0x0A]));
        let (answer, reply) = sign_in(addr, b"e2e-user", &remembered(DIGEST_2));
        assert_eq!((answer, reply), (0x0B, vec![0x0A]));
        // The right digest with another password: not what the client sends.
        let message = typed_with(DIGEST_1, b"test-password-2");
        assert_eq!(sign_in(addr, b"e2e-user", &message), (0x0B, vec![0x0A]));
        // A digest that is none, for an account and for a new name; 0.21d's
        // password message.
        let upper = remembered(b"F5358B98AF5AC658C4A219F518BD0FA5ED2FFD65");
        assert_eq!(sign_in(addr, b"e2e-user", &upper), (0x0B, vec![0x0A]));
        let not_hex = remembered(&[b'z'; 40]);
        assert_eq!(sign_in(addr, b"newcomer", &not_hex), (0x0B, vec![0x0A]));
        let old = [&[15][..], b"test-password-1"].concat();
        assert_eq!(sign_in(addr, b"newcomer", &old), (0x0B, vec![0x0A]));
        assert_eq!(store.account_id_by_name("newcomer"), None);
        assert_eq!(store.session_count(), 1);

        // No user name: refused before the password.
        let (answer, reply) = sign_in(addr, b"", &remembered(DIGEST_1));
        assert_eq!((answer, reply), (0x0A, Vec::new()));
        assert_eq!(store.session_count(), 1);
    }

    #[test]
    fn a_client_that_says_a_patch_revision_signs_in_as_any_other() {
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone(), local_lobby());
        // Byte 7 of the version field, past the text's zero (client_patch.md §6).
        for (revision, user_name) in [(1, &b"e2e-user-1"[..]), (255, b"e2e-user-2")] {
            let (answer, reply) = sign_in_as(
                addr,
                &patched(revision),
                user_name,
                &typed(b"test-password-1"),
            );
            assert_eq!(answer, 0x0B, "revision {revision}");
            assert_eq!(
                reply,
                connection_info_bytes(session_of(&reply), [0x01, 0x00, 0x00, 0x7F])
            );
        }
        assert_eq!(store.session_count(), 2);
    }

    /// The version field of a game of patch `revision` (client_patch.md §6).
    fn patched(revision: u8) -> [u8; 8] {
        [b'0', b'.', b'2', b'6', b'g', b'0', 0, revision]
    }

    #[test]
    fn a_game_older_than_the_server_takes_is_told_its_version_is_outdated() {
        let store = Arc::new(SessionStore::new());
        let gate = RevisionGate {
            min: 11,
            exempt: vec!["bot-1".to_owned()],
        };
        let addr = start_gated(store.clone(), local_lobby(), gate);
        // Older ones, the unpatched exe (0) too: refused before the
        // password, with no account made and no session.
        for revision in [0, 4, 10] {
            let (answer, reply) = sign_in_as(
                addr,
                &patched(revision),
                b"old-game",
                &typed(b"test-password-1"),
            );
            assert_eq!((answer, reply), (0x22, Vec::new()), "revision {revision}");
        }
        assert_eq!(store.account_id_by_name("old-game"), None);
        assert_eq!(store.session_count(), 0);
        // The least revision and newer ones sign in.
        for (revision, user_name) in [(11, &b"current-game"[..]), (0x80, b"newer-game")] {
            let (answer, reply) = sign_in_as(
                addr,
                &patched(revision),
                user_name,
                &typed(b"test-password-1"),
            );
            assert_eq!(answer, 0x0B, "revision {revision}");
            assert_eq!(store.client_revision(session_of(&reply)), Some(revision));
        }
        // A test bot says no revision and is let in, whatever its case.
        let (answer, reply) = sign_in(addr, b"Bot-1", &typed(b"test-password-1"));
        assert_eq!(answer, 0x0B);
        assert_eq!(store.client_revision(session_of(&reply)), Some(0));
        assert_eq!(store.session_count(), 3);
    }

    #[test]
    fn another_builds_client_is_told_its_version_is_wrong() {
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone(), local_lobby());
        for version in [&b"0.21d"[..], b"0.26g", b"0.26g1", b"0.26g00", b""] {
            let (answer, reply) = sign_in_as(addr, version, b"e2e-user", &remembered(DIGEST_1));
            assert_eq!((answer, reply), (0x22, Vec::new()), "{version:?}");
        }
        // 0.100b's shorter sign-in is read whole too.
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.write_all(b"\x01\x05a@b.c0.100b\0\0").unwrap();
        let mut answer = Vec::new();
        stream.read_to_end(&mut answer).unwrap();
        assert_eq!(answer, [0x22]);
        assert_eq!(store.account_id_by_name("e2e-user"), None);
        assert_eq!(store.session_count(), 0);
    }

    #[test]
    fn a_banned_account_and_a_banned_address_are_told_why() {
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone(), local_lobby());
        let (_, reply) = sign_in(addr, b"cheater", &remembered(DIGEST_1));
        let session = session_of(&reply);
        let id = store.account_id_by_name("cheater").unwrap();
        let ban = |reason: &str| session::Ban {
            until: None,
            reason: reason.into(),
        };
        store.ban_account(id, ban("<aimbot>"));
        assert!(store.has_ended(session));
        // Told only once the password is right, over TLS.
        let (answer, reply) = sign_in(addr, b"cheater", &remembered(DIGEST_1));
        assert_eq!((answer, reply), (0x0B, vec![0x1E]));
        let (_, reply) = sign_in(addr, b"cheater", &remembered(DIGEST_2));
        assert_eq!(reply, [0x0A]);
        // Each reason the client has, and none.
        for (reason, code) in [("teamkilling", 0x1F), ("chat flood", 0x16), ("", 0x1C)] {
            store.ban_account(id, ban(reason));
            let (_, reply) = sign_in(addr, b"cheater", &typed(b"test-password-1"));
            assert_eq!(reply, [code], "{reason}");
        }
        assert_eq!(store.session_count(), 0);

        // A banned address: before the password, and nobody registers.
        store.ban_net(session::Net::parse("127.0.0.0/8").unwrap(), ban("insults"));
        let (answer, reply) = sign_in(addr, b"newcomer", &remembered(DIGEST_1));
        assert_eq!((answer, reply), (0x13, Vec::new()));
        assert_eq!(store.account_id_by_name("newcomer"), None);
    }

    #[test]
    fn closed_registration_refuses_new_names_only() {
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone(), local_lobby());
        let (_, reply) = sign_in(addr, b"veteran", &remembered(DIGEST_1));
        session_of(&reply);
        store.set_registrations_open(false);
        let (answer, reply) = sign_in(addr, b"newcomer", &typed(b"test-password-1"));
        assert_eq!((answer, reply), (0x0B, vec![0x20]));
        assert_eq!(store.account_id_by_name("newcomer"), None);
        let (_, reply) = sign_in(addr, b"veteran", &typed(b"test-password-1"));
        session_of(&reply);
        store.set_registrations_open(true);
        let (_, reply) = sign_in(addr, b"newcomer", &typed(b"test-password-1"));
        session_of(&reply);
    }

    #[test]
    fn five_wrong_passwords_lock_the_name_out_even_for_the_right_one() {
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone(), local_lobby());
        let (_, reply) = sign_in(addr, b"guarded", &remembered(DIGEST_1));
        session_of(&reply);
        for _ in 0..session::MAX_FAILED_SIGN_INS {
            let (_, reply) = sign_in(addr, b"guarded", &remembered(DIGEST_2));
            assert_eq!(reply, [0x0A]);
        }
        // Locked out: the same answer as a wrong password, and no session.
        let (answer, reply) = sign_in(addr, b"Guarded", &remembered(DIGEST_1));
        assert_eq!((answer, reply), (0x0B, vec![0x0A]));
        assert_eq!(store.session_count(), 1);
        // Another name is not held back.
        let (_, reply) = sign_in(addr, b"other", &remembered(DIGEST_1));
        session_of(&reply);
    }

    #[test]
    fn a_client_from_elsewhere_is_sent_to_the_lobbys_public_address() {
        // `SRP_LOCAL_PEERS=off` with a public host: everyone gets that one.
        let lobby = Lobby::new(
            server("203.0.113.67", 1234, false),
            server("203.0.113.67", 1235, false),
        )
        .unwrap();
        let store = Arc::new(SessionStore::new());
        let addr = start(store, lobby);
        let (answer, reply) = sign_in(addr, b"e2e-user", &typed(b"test-password-1"));
        assert_eq!(answer, 0x0B);
        assert_eq!(
            reply,
            connection_info_bytes(session_of(&reply), [67, 113, 0, 203])
        );
    }

    #[test]
    fn only_a_sign_in_is_heard() {
        let store = Arc::new(SessionStore::new());
        let addr = start(store, local_lobby());
        // 0.21d's sign-out, and a cut sign-in: closed with no answer.
        for request in [&[0x02, 1, 2, 3, 4][..], &[0x01, 200, b'a']] {
            let mut stream = TcpStream::connect(addr).unwrap();
            stream.write_all(request).unwrap();
            // The server may have closed it already (not connected).
            let _ = stream.shutdown(std::net::Shutdown::Write);
            let mut answer = Vec::new();
            let _ = stream.read_to_end(&mut answer);
            assert!(answer.is_empty());
        }
    }
}

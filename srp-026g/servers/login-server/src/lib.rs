use openssl::ssl::{Ssl, SslContext, SslFiletype, SslMethod, SslStream};
use std::collections::HashSet;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant};

use session::{LockExt, SESSION_IDLE_LIMIT, SessionStore, SignInError};
use vostok::config;

/// The 0.26g client's login server; the rest of this file is 0.21d's.
pub mod v026g;

const PKEY_PATH: &str = "./certs/survarium_login_server.key";
const CERT_PATH: &str = "./certs/survarium_login_server.crt";

/// If no prefix is provided the client will construct an incorrect URI like so:
///     "127.0.0.1&type=2&.."
/// Notice that there is no `/` and that the first arguments starts with `&` instead of `?`.
/// As a temporary (?) fix provide an unused argument to the client, since it accepts url prefix
/// as a part of the message.
const URL_PREFIX: &[u8] = b"/hello?unused=1";

#[repr(u8)]
#[rustfmt::skip]
#[expect(non_camel_case_types)]
#[expect(dead_code)]
enum login_server_message_types_enum {
    servers_connection_info_message_type           = 0x8,  // + | logged in | client doesn't start required servers
    password_request_message_type                  = 0x9,  // connection error
    invalid_user_name_or_password_message_type     = 0xA,  // +
    valid_user_name_message_type                   = 0xB,  // + | establishes ssh connection
    sign_in_attempt_interval_violated_message_type = 0xC,  // +
    sign_out_successful                            = 0xD,  // connection error
    occupied_user_name_message_type                = 0xE,  // connection error
    send_sign_up_info_message_type                 = 0xF,  // connection error
    sign_up_successful_message_type                = 0x10, // connection error
    user_banned_message_type                       = 0x11, // +
    user_restricted_by_access_level_message_type   = 0x12, // +
    sign_in_user_already_signed_in                 = 0x13, // + | got stuck on connecting
    sign_in_invalid_version                        = 0x14, // +
    sign_in_eula_check_failed                      = 0x15, // 0.21d: shows the EULA window
    sign_in_added_to_online_queue                  = 0x16, // 0.21d: "in online queue"
    login_server_invalid_message_type              = 0x1F, // connection error
}

#[repr(u8)]
#[rustfmt::skip]
#[expect(non_camel_case_types)]
#[expect(dead_code)]
enum login_client_message_types_enum {
    sign_up_message_type              = 0x0, // not used by the client
    sign_in_message_type              = 0x1,
    sign_out_message_type             = 0x2,
    ping_online_message_type          = 0x3, // 0.21d presence ping (UDP)
    ping_queue_message_type           = 0x4, // 0.21d online-queue ping (UDP)
    login_client_invalid_message_type = 0x7, // not used by the client
}

/// How often sessions unseen for [`SESSION_IDLE_LIMIT`] are ended: the
/// original login server's `collect_interval`
/// (`login_server/sources/entry_point.cpp`).
const COLLECT_INTERVAL: Duration = Duration::from_secs(60);

/// How often the games gone without signing out are looked for
/// (`SessionStore::drop_gone_players`).
const GONE_CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// @NOTE: Real solution should:
/// * Accept the config for addresses of other servers
/// * Use `async` instead of threads
/// * Have a database for different users and their session ids
/// * ...many more things
pub fn run(store: Arc<SessionStore>) -> std::io::Result<()> {
    // Once, so a missing or bad certificate stops the server here rather
    // than failing every sign-in.
    let tls = Arc::new(tls_context().map_err(|error| {
        io::Error::other(format!(
            "cannot load {CERT_PATH} and {PKEY_PATH} (run srp from the repo root): {error}"
        ))
    })?);
    let listener = TcpListener::bind(config::get().login_server.bind_addr())?;

    for stream in listener.incoming() {
        let stream = match stream {
            Ok(stream) => stream,
            // A failed `accept` shouldn't take down the whole server.
            Err(error) => {
                log::error!("Failed to accept connection: {error}");
                continue;
            }
        };
        let store = store.clone();
        let tls = tls.clone();
        std::thread::spawn(move || {
            if std::panic::catch_unwind(|| handle_client(store, &tls, stream)).is_err() {
                log::error!("Connection handler panicked; dropped the connection");
            }
        });
    }
    Ok(())
}

/// Hear presence pings, and end sessions unseen for too long.
///
/// Once signed in, the 0.21d client pings the login server's IP on UDP, at
/// the port number it signed in at (the login TCP port, `m_host_port`), every
/// 10 seconds: `[0x03][u32 session id]` (`login_client_impl::ping` 0x245DA3,
/// `schedule_ping` 0x245D3D). It never waits for an answer and never stops.
/// While in the online queue it sends `[0x04][u32 queue id]` instead, which
/// srp never puts it in. The 0.100b client pinged port 25100 every second with
/// the bare 4-byte session id; that form is still heard. The original login
/// server kept the time of each
/// session's last ping and every minute signed out those unseen for ten
/// (`login_server.cpp`, `query_processor.cpp`); so does this. Lobby, chat
/// and match connections count as sightings too (see `SessionStore`), so a
/// client without pings (a script) keeps its session while it plays.
pub fn run_presence(store: Arc<SessionStore>) -> io::Result<()> {
    let address = config::get().login_pings.bind_addr();
    let mut socket = None;
    let mut unknown = HashSet::new();
    let mut next_collect = Instant::now() + COLLECT_INTERVAL;
    let mut next_gone_check = Instant::now() + GONE_CHECK_INTERVAL;
    loop {
        if socket.is_none() {
            socket = match UdpSocket::bind(&address) {
                Ok(bound) => {
                    log::info!("Hearing presence pings on UDP {address}");
                    Some(bound)
                }
                Err(error) => {
                    log::error!(
                        "Cannot hear presence pings on UDP {address} ({error}); \
                         retrying in {} s",
                        COLLECT_INTERVAL.as_secs()
                    );
                    None
                }
            };
        }
        let wait = next_collect
            .min(next_gone_check)
            .saturating_duration_since(Instant::now());
        match &socket {
            Some(socket) if !wait.is_zero() => {
                socket.set_read_timeout(Some(wait))?;
                let mut buffer = [0; 16];
                match socket.recv_from(&mut buffer) {
                    Ok((len, from)) => hear_ping(&store, &buffer[..len], from, &mut unknown),
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) => {}
                    // On Windows a datagram socket also reports other hosts'
                    // errors (an oversized datagram, a reset); none ends it.
                    Err(error) => log::debug!("Presence ping unreadable: {error}"),
                }
            }
            Some(_) => {}
            None => std::thread::sleep(wait),
        }
        if Instant::now() >= next_gone_check {
            next_gone_check = Instant::now() + GONE_CHECK_INTERVAL;
            let gone = store.drop_gone_players(Instant::now());
            if !gone.is_empty() {
                log::info!(
                    "{} game(s) gone without signing out (nothing from them for {} s): {gone:#x?}",
                    gone.len(),
                    session::GONE_AFTER.as_secs()
                );
            }
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

/// One presence ping: `[0x03][u32 session id]` (0.21d) or the bare session
/// id (0.100b). A ping for an unknown or ended session is logged once per
/// session (the client keeps pinging).
fn hear_ping(store: &SessionStore, datagram: &[u8], from: SocketAddr, unknown: &mut HashSet<u32>) {
    const PING_ONLINE: u8 = login_client_message_types_enum::ping_online_message_type as u8;
    let session_id = match *datagram {
        [PING_ONLINE, a, b, c, d] | [a, b, c, d] => Some(u32::from_le_bytes([a, b, c, d])),
        _ => None,
    };
    let Some(session_id) = session_id else {
        log::debug!(
            "Ignored a {}-byte presence datagram from {from}",
            datagram.len()
        );
        return;
    };
    if store.touch(session_id) {
        log::trace!("Presence ping of session {session_id:#x} from {from}");
        return;
    }
    if unknown.len() >= 1024 {
        unknown.clear();
    }
    if unknown.insert(session_id) {
        log::debug!("Presence ping from {from} for unknown or ended session {session_id:#x}");
    }
}

fn handle_client(store: Arc<SessionStore>, tls: &SslContext, mut stream: TcpStream) {
    let mut request_type = [0_u8];
    if let Err(error) = stream.read_exact(&mut request_type) {
        log::info!("Login connection closed before its request: {error}");
        return;
    }

    let request_type = request_type[0];
    log::debug!("Received message of type: {request_type}");

    match () {
        _ if request_type == login_client_message_types_enum::sign_in_message_type as u8 => {
            handle_sign_in(store, tls, stream);
        }
        _ if request_type == login_client_message_types_enum::sign_out_message_type as u8 => {
            handle_sign_out(&store, tls, stream);
        }
        _ => log::warn!("Unknown login request type {request_type}; dropped the connection"),
    }
}

/// Read until `buffer` holds a whole message, as `complete` measures it from
/// what has arrived (`None`: can't tell yet), and return its length. Whatever
/// else came in the same reads is read too and ignored.
fn read_message(
    stream: &mut impl Read,
    buffer: &mut [u8],
    complete: impl Fn(&[u8]) -> Option<usize>,
) -> io::Result<usize> {
    let mut filled = 0;
    loop {
        if let Some(len) = complete(&buffer[..filled])
            && len <= filled
        {
            return Ok(len);
        }
        if filled == buffer.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "message too long",
            ));
        }
        match stream.read(&mut buffer[filled..])? {
            0 => return Err(io::ErrorKind::UnexpectedEof.into()),
            read => filled += read,
        }
    }
}

/// The 0.100b client's version field: `"0.100b"` and two zeros.
const VERSION_0100B: &[u8] = b"0.100b";

/// A sign-in, after its type byte: the user name (u8 length), then the game
/// version. 0.21d (`sign_in_on_connected` 0x23E759) sends a 12-byte version
/// (`"0.21d"`, zero-padded), a `connect_from_eula_wnd` byte and a u32
/// `online_queue_order_id`; 0.100b sent an 8-byte version and nothing more.
/// Reading only part of it would leave the rest to be taken for the start of
/// the TLS handshake.
fn sign_in_length(buffer: &[u8]) -> Option<usize> {
    let user_name = 1 + usize::from(*buffer.first()?);
    let version = buffer.get(user_name..user_name + VERSION_0100B.len())?;
    Some(
        user_name
            + if version == VERSION_0100B {
                8
            } else {
                12 + 1 + 4
            },
    )
}

/// What a sign-in says besides the password.
#[derive(Debug, PartialEq)]
struct SignIn {
    /// What the player typed as the account (`m_account_email`): any user
    /// name, an email or not.
    user_name: String,
    version: String,
    /// 0.26g: what a patched client says of its patches, 0 from an exe
    /// that says nothing (`docs/notes/client_patch.md` §6).
    /// The version field is 12 bytes, the text and zeros; the patch puts
    /// this into byte 7, past the zero that ends a text of six letters.
    patch_revision: u8,
    /// 0.26g: byte 8 of the version field (`client_patch.md` §3.25). A game
    /// of patch revision 13 or later reports here what it saw of one known
    /// cheat; 0 from any older exe. Bit 0 (1): a stale shell_32.dll in the
    /// temporary folder. Bit 7 (0x80): the check ran. Only logged.
    cheat_flags: u8,
    /// 0.21d: 1 when the player signs in from the EULA window.
    from_eula_window: bool,
    /// 0.21d: the place in the online queue it rejoins, `u32::MAX` for none.
    queue_order_id: u32,
}

/// Read a whole sign-in, as `sign_in_length` measured it.
fn parse_sign_in(message: &[u8]) -> SignIn {
    let name_end = 1 + usize::from(message[0]);
    let (version, rest) = message[name_end..].split_at((message.len() - name_end).min(12));
    let field = version;
    let version = field.split(|&byte| byte == 0).next().unwrap_or_default();
    // Only past the text's own zero, and only in a whole field.
    let (patch_revision, cheat_flags) = if field.len() == 12 && version.len() < 7 {
        (field[7], field[8])
    } else {
        (0, 0)
    };
    let (from_eula_window, queue_order_id) = match *rest {
        [eula, a, b, c, d] => (eula != 0, u32::from_le_bytes([a, b, c, d])),
        _ => (false, u32::MAX),
    };
    SignIn {
        user_name: String::from_utf8_lossy(&message[1..name_end]).into_owned(),
        version: String::from_utf8_lossy(version).into_owned(),
        patch_revision,
        cheat_flags,
        from_eula_window,
        queue_order_id,
    }
}

/// The 0x08 reply's tail in 0.21d: where the lobby and messaging servers
/// are, `[u32 ip][u16 port]` each (`on_sign_in_answer_received` 0x23DF25).
/// The client reverses the 4 bytes into the address, so they are the IP as a
/// number, little-endian: 127.0.0.1 is `01 00 00 7F`. It uses them only when
/// started without `-client`; srp's chat shares the lobby's port. A host
/// that is not an IPv4 address is looked up; failing that the IP is 0.
fn lobby_addresses(host: &str, port: u16) -> [u8; 12] {
    let ip = host.parse::<Ipv4Addr>().ok().or_else(|| {
        (host, port)
            .to_socket_addrs()
            .ok()?
            .find_map(|address| match address {
                SocketAddr::V4(address) => Some(*address.ip()),
                SocketAddr::V6(_) => None,
            })
    });
    let ip = ip.map_or(0, u32::from).to_le_bytes();
    let port = port.to_le_bytes();
    let mut tail = [0; 12];
    for half in tail.chunks_mut(6) {
        half[..4].copy_from_slice(&ip);
        half[4..].copy_from_slice(&port);
    }
    tail
}

/// The client's answer to a wrong password, or a user name it can't have:
/// "Invalid user name or password", back to the login screen.
const INVALID_USER_NAME_OR_PASSWORD: u8 =
    login_server_message_types_enum::invalid_user_name_or_password_message_type as u8;

/// New accounts are not taken: the client's "restricted by access level".
const REGISTRATION_CLOSED: u8 =
    login_server_message_types_enum::user_restricted_by_access_level_message_type as u8;

/// "Too many sign-in attempts": an address that asked too often, or
/// registered as many accounts as it may for now.
const TOO_MANY_ATTEMPTS: u8 =
    login_server_message_types_enum::sign_in_attempt_interval_violated_message_type as u8;

/// "This account is banned": the client shows it and stays at the login
/// screen.
const USER_BANNED: u8 = login_server_message_types_enum::user_banned_message_type as u8;

/// The password, behind its u8 length (the client's buffer takes 47 bytes).
fn password_length(buffer: &[u8]) -> Option<usize> {
    Some(1 + usize::from(*buffer.first()?))
}

fn handle_sign_in(store: Arc<SessionStore>, tls: &SslContext, mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    let peer = match stream.peer_addr() {
        Ok(peer) => peer,
        Err(error) => {
            log::info!("Sign-in dropped: {error}");
            return;
        }
    };
    // Where the client finds the browser server: see `ServerConfig::host_for`.
    let config = config::get();
    let (browser_host, lobby_host) = match stream.local_addr() {
        Ok(reached) => (
            config
                .browser_server
                .host_for(&config.login_server, peer.ip(), reached.ip()),
            config
                .lobby_server
                .host_for(&config.login_server, peer.ip(), reached.ip()),
        ),
        Err(_) => (
            config.browser_server.public_host.clone(),
            config.lobby_server.public_host.clone(),
        ),
    };
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
        "Client version: {} (from the EULA window: {}, queue order: {:#x})",
        sign_in.version,
        sign_in.from_eula_window,
        sign_in.queue_order_id
    );

    // Nobody signs in, or registers, from a banned address: the game's own
    // "user banned" answer, before the password.
    if let Some(ban) = store.ip_ban(peer.ip()) {
        log::warn!(
            "Sign-in of {user_name} from banned address {peer} ({}) refused",
            ban.net
        );
        let _ = stream.write_all(&[USER_BANNED]);
        return;
    }
    // A name no account can have is refused before the password, as the
    // original server answered an unknown user name.
    if session::persist::user_name_key(&user_name).is_none() {
        log::info!("Sign-in from {peer} refused: empty or over-long user name");
        let _ = stream.write_all(&[INVALID_USER_NAME_OR_PASSWORD]);
        return;
    }
    let message_type = login_server_message_types_enum::valid_user_name_message_type as u8;
    if let Err(error) = stream.write_all(&[message_type]) {
        log::warn!("Sign-in of {user_name} dropped: {error}");
        return;
    }

    let Some(mut ssl_stream) = accept_tls(tls, stream) else {
        return;
    };
    // The password is never logged, and kept only as a hash.
    let mut buffer = [0; 1 + 255];
    let len = match read_message(&mut ssl_stream, &mut buffer, password_length) {
        Ok(len) => len,
        Err(error) => {
            log::warn!("Sign-in of {user_name} dropped before the password: {error}");
            return;
        }
    };
    // The account (made here on a first sign-in) and the session come only
    // once the password is right.
    let signed_in = match store.sign_in_from(&user_name, &buffer[1..len], peer.ip(), Instant::now())
    {
        Ok(signed_in) => signed_in,
        Err(error) => {
            match error {
                SignInError::WrongPassword => log::info!("Wrong password for {user_name}"),
                SignInError::TooManyAttempts => log::info!(
                    "Wrong password for {user_name}: too many in a row, refused for a while"
                ),
                SignInError::InvalidUserName => log::info!("Invalid user name {user_name}"),
                // Only the 0.26g sign-in takes nicknames.
                SignInError::InvalidNickname | SignInError::NicknameTaken => {
                    log::info!("Sign-in of {user_name} refused: {error:?}");
                }
                SignInError::TooManyFromAddress | SignInError::RegistrationLimit => {
                    log::warn!("Sign-in of {user_name} from {peer} refused: {error:?}");
                    let _ = ssl_stream.write_all(&[TOO_MANY_ATTEMPTS]);
                    return;
                }
                SignInError::RegistrationsClosed => {
                    log::info!(
                        "Sign-in of new user name {user_name} from {peer}: registration is closed"
                    );
                    let _ = ssl_stream.write_all(&[REGISTRATION_CLOSED]);
                    return;
                }
                SignInError::Banned => {
                    log::warn!("Sign-in of banned account {user_name} from {peer} refused");
                    let _ = ssl_stream.write_all(&[USER_BANNED]);
                    return;
                }
            }
            // The same answer for all, so a guesser learns nothing more.
            let _ = ssl_stream.write_all(&[INVALID_USER_NAME_OR_PASSWORD]);
            return;
        }
    };
    let account_id = signed_in.account_id();
    // None: the admin reset the password while it was checked. Only a
    // sign-in that got its session is where the account last signed in.
    let Some(session_id) = store.create_signed_in_session(signed_in, 0) else {
        log::warn!("Sign-in of {user_name} refused: its password was reset meanwhile");
        let _ = ssl_stream.write_all(&[INVALID_USER_NAME_OR_PASSWORD]);
        return;
    };
    store.signed_in_from(account_id, peer.ip());
    log::info!("Sign-in: account={account_id:#x} session={session_id:#x}");

    let mut buffer = vec![];
    buffer.push(login_server_message_types_enum::servers_connection_info_message_type as u8);

    // Host only: the client talks HTTP to the browser server on the
    // implicit port 80 and appends `URL_PREFIX` itself.
    log::debug!("Session {session_id:#x} goes to the browser server at {browser_host}");
    buffer.push(browser_host.len() as u8);
    buffer.extend(browser_host.as_bytes());
    buffer.push(URL_PREFIX.len() as u8);
    buffer.extend(URL_PREFIX);

    buffer.extend(session_id.to_le_bytes());
    buffer.extend(lobby_addresses(&lobby_host, config.lobby_server.port));
    // The 0.21d client reads the reply with one read of at most 82 bytes.
    if buffer.len() > 82 {
        log::warn!(
            "The sign-in reply is {} bytes, more than the client reads (82): \
             shorten the browser server's public host",
            buffer.len()
        );
    }

    if let Err(error) = ssl_stream.write_all(&buffer) {
        log::warn!("Sign-in for session {session_id:#x} dropped before the reply: {error}");
    }
}

/// A sign-out: the session id in the clear, then the password over TLS, and
/// no answer (`login_client_impl::sign_out`, 0x7A1CDC). The client sends one
/// when it quits, and when it gives up reconnecting to the lobby. Once the
/// password has come, the session ends, as the original login server signed
/// it out (`client_session_sign_out.cpp`), once the password is the
/// account's.
fn handle_sign_out(store: &SessionStore, tls: &SslContext, mut stream: TcpStream) {
    // Exactly the id: the client starts its TLS handshake right behind it,
    // with no answer to wait for, so a longer read would take the start of
    // the handshake too.
    let mut session_id = [0; 4];
    let session_id = match stream.read_exact(&mut session_id) {
        Ok(()) => u32::from_le_bytes(session_id),
        Err(error) => {
            log::info!("Unreadable sign-out: {error}");
            return;
        }
    };
    log::debug!("Sign-out of session {session_id:#x}");

    let Some(mut ssl_stream) = accept_tls(tls, stream) else {
        return;
    };
    let mut buffer = [0; 1024];
    let len = match read_message(&mut ssl_stream, &mut buffer, password_length) {
        Ok(len) => len,
        Err(error) => {
            log::info!("Sign-out of session {session_id:#x} dropped before the password: {error}");
            return;
        }
    };
    // Only the session's own player signs it out: the password must be its
    // account's, or anyone who learned the id could end it.
    let hash = store
        .account_for_session(session_id)
        .and_then(|account| account.locked().password_hash.clone());
    if let Some(hash) = hash
        && !session::password::verify(&hash, &buffer[1..len])
    {
        log::warn!("Sign-out of session {session_id:#x} with a wrong password; ignored");
        return;
    }
    if store.end_session(session_id) {
        log::info!("Session {session_id:#x} signed out");
    } else {
        log::info!("Sign-out of unknown or ended session {session_id:#x}");
    }
}

/// The login server's TLS setup, with the game's own key and certificate.
fn tls_context() -> Result<SslContext, openssl::error::ErrorStack> {
    tls_context_from(PKEY_PATH, CERT_PATH)
}

fn tls_context_from(key: &str, cert: &str) -> Result<SslContext, openssl::error::ErrorStack> {
    let mut builder = SslContext::builder(SslMethod::tls())?;
    builder.set_security_level(0);
    builder.set_private_key_file(key, SslFiletype::PEM)?;
    builder.set_certificate_chain_file(cert)?;
    Ok(builder.build())
}

/// Upgrade a login connection to TLS. `None` when the handshake fails, which is
/// how a client that rejects the certificate shows up here: it just closes the
/// connection.
fn accept_tls(tls: &SslContext, stream: TcpStream) -> Option<SslStream<TcpStream>> {
    let peer = stream.peer_addr().ok();
    let ssl = match Ssl::new(tls) {
        Ok(ssl) => ssl,
        Err(error) => {
            log::error!("TLS setup for {peer:?} failed: {error}");
            return None;
        }
    };

    match ssl.accept(stream) {
        Ok(ssl_stream) => Some(ssl_stream),
        Err(error) => {
            // The client checks the certificate against its own
            // `resources/ssl/survarium_login_server.crt` on its first attempt
            // only. The game's original certificate expired in 2014, so that
            // file has to be replaced with `CERT_PATH` (see WORK.md).
            log::warn!(
                "TLS handshake with {peer:?} failed ({error}); if the client logs \
                 \"certificate verify failed\", copy {CERT_PATH} over its \
                 resources/ssl/survarium_login_server.crt"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hands out its bytes a few at a time, as a socket may.
    struct Trickle<'a>(&'a [u8]);

    impl Read for Trickle<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let n = self.0.len().min(buffer.len()).min(3);
            buffer[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }

    /// A 0.21d sign-in after its type byte, as `sign_in_on_connected` writes it.
    fn sign_in_0_21d(email: &[u8], eula: u8, queue: u32) -> Vec<u8> {
        let mut sign_in = vec![email.len() as u8];
        sign_in.extend(email);
        sign_in.extend(b"0.21d\0\0\0\0\0\0\0");
        sign_in.push(eula);
        sign_in.extend(queue.to_le_bytes());
        sign_in
    }

    #[test]
    fn a_sign_in_is_read_whole_however_it_arrives() {
        let mut sign_in = vec![5];
        sign_in.extend(b"a@b.c0.100b\0\0");
        let mut buffer = [0; 64];
        let len = read_message(&mut Trickle(&sign_in), &mut buffer, sign_in_length).unwrap();
        assert_eq!(&buffer[..len], sign_in.as_slice());

        // The TLS handshake follows: nothing of it may be read with the sign-in.
        let mut sign_in = sign_in_0_21d(b"a@b.c", 0, u32::MAX);
        let len_0_21d = sign_in.len();
        sign_in.extend([0x16, 3, 1]);
        let len = read_message(&mut Trickle(&sign_in), &mut buffer, sign_in_length).unwrap();
        assert_eq!(len, len_0_21d);
        assert_eq!(len, 1 + 5 + 17);
    }

    #[test]
    fn a_sign_in_says_its_email_version_and_queue_place() {
        let sign_in = sign_in_0_21d(b"player@e2e.invalid", 1, 7);
        assert_eq!(
            parse_sign_in(&sign_in),
            SignIn {
                user_name: "player@e2e.invalid".into(),
                version: "0.21d".into(),
                patch_revision: 0,
                cheat_flags: 0,
                from_eula_window: true,
                queue_order_id: 7,
            }
        );
        let mut old = vec![3];
        old.extend(b"a@b0.100b\0\0");
        assert_eq!(
            parse_sign_in(&old),
            SignIn {
                user_name: "a@b".into(),
                version: "0.100b".into(),
                patch_revision: 0,
                cheat_flags: 0,
                from_eula_window: false,
                queue_order_id: u32::MAX,
            }
        );
    }

    #[test]
    fn a_patched_0_26g_client_says_its_patch_revision() {
        let read = |field: &[u8; 12]| {
            let mut sign_in = vec![3];
            sign_in.extend(b"a@b");
            sign_in.extend(field);
            sign_in.push(0);
            sign_in.extend(u32::MAX.to_le_bytes());
            let sign_in = parse_sign_in(&sign_in);
            (sign_in.version, sign_in.patch_revision, sign_in.cheat_flags)
        };
        // The stock field, and what the patched `sign_in_on_connected`
        // 0x3807D0 makes of it: the text is the same.
        assert_eq!(read(b"0.26g0\0\0\0\0\0\0"), ("0.26g0".into(), 0, 0));
        assert_eq!(read(b"0.26g0\0\x01\0\0\0\0"), ("0.26g0".into(), 1, 0));
        assert_eq!(read(b"0.26g0\0\x02\0\0\0\0"), ("0.26g0".into(), 2, 0));
        // Revision 13 also sets byte 8 (the cheat check, §3.25): 0x80 ran,
        // 0x81 ran and found the stale shell_32.dll.
        assert_eq!(read(b"0.26g0\0\x0d\x80\0\0\0"), ("0.26g0".into(), 13, 0x80));
        assert_eq!(read(b"0.26g0\0\x0d\x81\0\0\0"), ("0.26g0".into(), 13, 0x81));
        // A text that reaches byte 7 keeps it: no revision, no flags.
        assert_eq!(read(b"0.26g001\0\0\0\0"), ("0.26g001".into(), 0, 0));
        assert_eq!(read(b"0.26g00\0\x01\0\0\0"), ("0.26g00".into(), 0, 0));
        // 0.100b's field is shorter than 12 bytes.
        let mut old = vec![3];
        old.extend(b"a@b0.100b\0\0");
        assert_eq!(parse_sign_in(&old).patch_revision, 0);
        assert_eq!(parse_sign_in(&old).cheat_flags, 0);
    }

    #[test]
    fn the_reply_says_where_the_lobby_is_as_the_client_reads_it() {
        assert_eq!(
            lobby_addresses("127.0.0.1", 1235),
            [0x01, 0, 0, 0x7F, 0xD3, 0x04, 0x01, 0, 0, 0x7F, 0xD3, 0x04]
        );
        assert_eq!(lobby_addresses("203.0.113.67", 1235)[..4], [67, 113, 0, 203],);
        assert_eq!(lobby_addresses("no-such-host.invalid", 1235)[..4], [0; 4]);
    }

    #[test]
    fn a_presence_ping_keeps_its_session_and_nothing_else_does() {
        let store = SessionStore::new();
        let account = store.get_or_create_account([127, 0, 0, 1].into(), "pinger");
        let session = store.create_session(account);
        let from = SocketAddr::from(([127, 0, 0, 1], 50000));
        let mut unknown = HashSet::new();

        let limit = Duration::from_millis(50);
        // 0.21d's `[0x03][u32]` and 0.100b's bare id.
        let mut online = vec![0x03];
        online.extend(session.to_le_bytes());
        for ping in [online.as_slice(), &session.to_le_bytes()] {
            std::thread::sleep(Duration::from_millis(60));
            hear_ping(&store, ping, from, &mut unknown);
            assert!(store.end_idle_sessions(Instant::now(), limit).is_empty());
        }

        // Too long, too short, someone else's: no sighting.
        std::thread::sleep(Duration::from_millis(60));
        let mut long = session.to_le_bytes().to_vec();
        long.push(0);
        hear_ping(&store, &long, from, &mut unknown);
        hear_ping(&store, &session.to_le_bytes()[..3], from, &mut unknown);
        let mut queue = vec![0x04];
        queue.extend(session.to_le_bytes());
        hear_ping(&store, &queue, from, &mut unknown);
        hear_ping(&store, &0xBAD_u32.to_le_bytes(), from, &mut unknown);
        assert_eq!(unknown, HashSet::from([0xBAD]));
        assert_eq!(store.end_idle_sessions(Instant::now(), limit), [session]);
    }

    #[test]
    fn a_cut_or_oversized_message_is_an_error_not_a_panic() {
        let mut buffer = [0; 64];
        assert!(read_message(&mut Trickle(&[200, 1, 2]), &mut buffer, sign_in_length).is_err());
        let mut small = [0; 4];
        assert!(read_message(&mut Trickle(&[9; 16]), &mut small, password_length).is_err());
    }

    /// A login server on a free local port, serving `store`.
    fn start(store: Arc<SessionStore>) -> SocketAddr {
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
                handle_client(store.clone(), &tls, stream);
            }
        });
        addr
    }

    /// Sign in the way the 0.21d client does; returns the plain answer and,
    /// past the TLS handshake, the reply to the password.
    fn sign_in(addr: SocketAddr, user_name: &[u8], password: &[u8]) -> (u8, Vec<u8>) {
        use openssl::ssl::{SslConnector, SslVerifyMode};
        let mut stream = TcpStream::connect(addr).unwrap();
        let mut message = vec![login_client_message_types_enum::sign_in_message_type as u8];
        message.extend(sign_in_0_21d(user_name, 0, u32::MAX));
        stream.write_all(&message).unwrap();
        let mut answer = [0];
        stream.read_exact(&mut answer).unwrap();
        if answer[0] != login_server_message_types_enum::valid_user_name_message_type as u8 {
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
        let mut message = vec![password.len() as u8];
        message.extend(password);
        tls.write_all(&message).unwrap();
        let mut reply = Vec::new();
        let _ = tls.read_to_end(&mut reply);
        (answer[0], reply)
    }

    #[test]
    fn a_banned_account_and_a_banned_address_are_told_so() {
        const VALID: u8 = login_server_message_types_enum::valid_user_name_message_type as u8;
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone());
        let id = store.sign_in("cheater", b"pw").unwrap();
        let ban = session::Ban {
            until: None,
            reason: String::new(),
        };
        store.ban_account(id, ban.clone());
        // Told only once the password is right, over TLS.
        let (answer, reply) = sign_in(addr, b"cheater", b"pw");
        assert_eq!((answer, reply), (VALID, vec![USER_BANNED]));
        let (_, reply) = sign_in(addr, b"cheater", b"wrong");
        assert_eq!(reply, [INVALID_USER_NAME_OR_PASSWORD]);
        assert_eq!(store.session_count(), 0);

        // A banned address: before the password, and nobody registers.
        store.ban_net(session::Net::parse("127.0.0.0/8").unwrap(), ban);
        let (answer, reply) = sign_in(addr, b"newcomer", b"pw");
        assert_eq!((answer, reply), (USER_BANNED, Vec::new()));
        assert_eq!(store.account_id_by_name("newcomer"), None);
    }

    #[test]
    fn a_first_sign_in_registers_and_a_wrong_password_gets_no_session() {
        const VALID: u8 = login_server_message_types_enum::valid_user_name_message_type as u8;
        const CONNECTION_INFO: u8 =
            login_server_message_types_enum::servers_connection_info_message_type as u8;
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone());

        // Registered by its first sign-in, then signed in again.
        for user_name in [&b"e2e-user"[..], b"E2E-User"] {
            let (answer, reply) = sign_in(addr, user_name, b"test-password-1");
            assert_eq!(answer, VALID);
            assert_eq!(reply.first(), Some(&CONNECTION_INFO));
        }
        assert_eq!(store.session_count(), 2);

        // A wrong password: told so, and no session.
        let (answer, reply) = sign_in(addr, b"e2e-user", b"test-password-2");
        assert_eq!(answer, VALID);
        assert_eq!(reply, [INVALID_USER_NAME_OR_PASSWORD]);
        assert_eq!(store.session_count(), 2);

        // No user name: refused before the password.
        let (answer, reply) = sign_in(addr, b"", b"test-password-1");
        assert_eq!(answer, INVALID_USER_NAME_OR_PASSWORD);
        assert!(reply.is_empty());
        assert_eq!(store.session_count(), 2);
    }
}

mod lobby_server;
mod messaging_server;
// The 0.26g lobby (UDP, with the chat in it), beside the 0.21d one.
pub mod v026g;

use session::SessionStore;
use vostok::config;
use vostok::network_client::TcpClient;

use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

const _: () = {
    // The messages are serialized and deserialized as plain bytes (`vostok`, `survarium`),
    // and all of them are expected to use little endian.
    assert!(
        cfg!(target_endian = "little"),
        "This code only supports little-endian systems"
    );
};

/// What the lobby and the chat do with a session the store doesn't know, or
/// one that has ended (signed out, or unseen for too long).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownSessions {
    /// Refuse it, as a session the login server didn't open.
    Refuse,
    /// Serve it as a default account: a lobby run without a login server.
    UseDefaultAccount,
}

/// The lobby and chat server of srp, beside its login server.
pub fn run(store: Arc<SessionStore>) -> std::io::Result<()> {
    serve(store, UnknownSessions::Refuse)
}

/// A lobby and chat server on its own, with no login server to open
/// sessions: any session gets a default account.
pub fn run_standalone(store: Arc<SessionStore>) -> std::io::Result<()> {
    serve(store, UnknownSessions::UseDefaultAccount)
}

fn serve(store: Arc<SessionStore>, unknown: UnknownSessions) -> std::io::Result<()> {
    let chat = Arc::new(messaging_server::ChatRoom::default());

    let listener = TcpListener::bind(config::get().lobby_server.bind_addr())?;

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
        let chat = chat.clone();
        std::thread::spawn(move || {
            if std::panic::catch_unwind(|| handle_connection(store, &chat, stream, unknown))
                .is_err()
            {
                log::error!("Connection handler panicked; dropped the connection");
            }
        });
    }

    Ok(())
}

/// A new connection must send its first message within this long: one that
/// sends nothing is not a client, and holds a thread.
#[cfg(not(test))]
const FIRST_MESSAGE_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(test)]
const FIRST_MESSAGE_TIMEOUT: Duration = Duration::from_millis(300);

pub fn handle_connection(
    store: Arc<SessionStore>,
    chat: &messaging_server::ChatRoom,
    stream: TcpStream,
    unknown: UnknownSessions,
) {
    // The lobby and chat ("messaging") protocols share this port; the first
    // message tells them apart: the lobby's is `0x26`, the chat's `0xC3`
    // (spec 1.1). The chat writes through `stream` while `tcp_client` reads
    // from its clone.
    let reader = match stream.try_clone() {
        Ok(reader) => reader,
        Err(error) => {
            log::error!("Lobby connection unusable: {error}");
            return;
        }
    };
    // A client that goes away without a word is noticed within about a
    // minute, and one that stops reading can't hold a writer for long.
    if let Err(error) = vostok::network_client::tune_connection(&stream) {
        log::debug!("Lobby connection not tuned: {error}");
    }
    let mut tcp_client = TcpClient::new(reader);
    let _ = tcp_client.set_read_timeout(Some(FIRST_MESSAGE_TIMEOUT));
    match tcp_client.peek::<lobby_server::frame::Frame>() {
        Ok(lobby_server::frame::Frame(payload))
            if payload.first() == Some(&lobby_server::SIGN_IN) =>
        {
            lobby_server::run(store, chat, tcp_client, unknown)
        }
        Ok(_) => {
            // The chat waits for its client as long as it takes.
            let _ = tcp_client.set_read_timeout(None);
            messaging_server::run(&store, chat, tcp_client, stream, unknown)
        }
        Err(error) if error.is_timeout() => {
            log::info!("Lobby connection sent nothing in {FIRST_MESSAGE_TIMEOUT:?}; closed it");
        }
        Err(error) if error.is_disconnect() => {
            log::info!("Lobby connection closed before its first message");
        }
        Err(error) => {
            log::error!("Unreadable first lobby message ({error}); dropped the connection");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::time::{Duration, Instant};

    /// The lobby and chat on a free local port.
    fn start(store: Arc<SessionStore>, unknown: UnknownSessions) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let chat = Arc::new(messaging_server::ChatRoom::default());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let (store, chat) = (store.clone(), chat.clone());
                std::thread::spawn(move || {
                    handle_connection(store, &chat, stream.unwrap(), unknown)
                });
            }
        });
        addr
    }

    #[test]
    fn a_connection_that_sends_nothing_is_closed() {
        let addr = start(Arc::new(SessionStore::new()), UnknownSessions::Refuse);
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let started = std::time::Instant::now();
        // The server closes it: end of stream, well before our own timeout.
        assert_eq!(stream.read(&mut [0; 1]).unwrap(), 0);
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    fn connect(addr: SocketAddr, first: &[u8]) -> TcpStream {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut bytes = vec![first.len() as u8];
        bytes.extend(first);
        stream.write_all(&bytes).unwrap();
        stream
    }

    /// The 0.21d sign-in: session, not back from a match, no match stats
    /// pending, shop items wanted.
    fn lobby_sign_in(addr: SocketAddr, session_id: u32) -> TcpStream {
        let mut sign_in = vec![0x26];
        sign_in.extend(session_id.to_le_bytes());
        sign_in.push(0);
        sign_in.extend(u32::MAX.to_le_bytes());
        sign_in.push(0xFF);
        connect(addr, &sign_in)
    }

    fn chat_sign_in(addr: SocketAddr, session_id: u32) -> TcpStream {
        let mut sign_in = vec![0xC3];
        sign_in.extend(session_id.to_le_bytes());
        sign_in.push(5);
        connect(addr, &sign_in)
    }

    /// The next frame, or `None` once the server has closed the connection.
    fn frame(stream: &mut TcpStream) -> Option<Vec<u8>> {
        let mut len = [0_u8];
        if stream.read(&mut len).unwrap() == 0 {
            return None;
        }
        let len = if len[0] == 0 {
            let mut long = [0_u8; 2];
            stream.read_exact(&mut long).unwrap();
            u16::from_le_bytes(long)
        } else {
            len[0].into()
        };
        let mut frame = vec![0; usize::from(len)];
        stream.read_exact(&mut frame).unwrap();
        Some(frame)
    }

    /// The sign-in push, through its last message (0x38).
    fn push(stream: &mut TcpStream) -> Vec<Vec<u8>> {
        let mut frames = Vec::new();
        while let Some(frame) = frame(stream) {
            let last = frame[0] == 0x38;
            frames.push(frame);
            if last {
                break;
            }
        }
        frames
    }

    fn signed_in(store: &SessionStore, email: &str) -> u32 {
        let account_id = store.get_or_create_account(IpAddr::V4(Ipv4Addr::LOCALHOST), email);
        store.create_session(account_id)
    }

    #[test]
    fn an_unknown_or_ended_session_is_refused_before_sign_in() {
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone(), UnknownSessions::Refuse);

        let mut lobby = lobby_sign_in(addr, 0xBAD);
        assert_eq!(frame(&mut lobby), None);
        let mut chat = chat_sign_in(addr, 0xBAD);
        assert_eq!(frame(&mut chat), None);

        let session = signed_in(&store, "player");
        let mut lobby = lobby_sign_in(addr, session);
        assert_eq!(frame(&mut lobby), Some(vec![0x30, 0x00]));
        let mut chat = chat_sign_in(addr, session);
        assert_eq!(frame(&mut chat), Some(b"\xCA\x06player".to_vec()));

        // Signed out: the open connections go on, new ones are refused.
        assert!(store.end_session(session));
        let mut again = lobby_sign_in(addr, session);
        assert_eq!(frame(&mut again), None);
        let mut chat_again = chat_sign_in(addr, session);
        assert_eq!(frame(&mut chat_again), None);
    }

    #[test]
    fn a_standalone_lobby_serves_an_unknown_session() {
        let addr = start(
            Arc::new(SessionStore::new()),
            UnknownSessions::UseDefaultAccount,
        );
        let mut lobby = lobby_sign_in(addr, 0xBAD);
        let push = push(&mut lobby);
        assert_eq!(push[0], [0x30, 0x00]);
        // The whole account, ending with the online stats.
        assert!(push.iter().any(|frame| frame[..2] == [0x36, 0x03]));
        assert!(push.iter().any(|frame| frame[..2] == [0x36, 0x01]));
        assert_eq!(push.last().unwrap()[0], 0x38);
        // Then the client asks for its profile.
        lobby.write_all(&[6, 0x21, 0x02, 1, 0, 0, 0]).unwrap();
        assert_eq!(frame(&mut lobby).unwrap()[..2], [0x36, 0x02]);
        let mut chat = chat_sign_in(addr, 0xBAD);
        assert_eq!(frame(&mut chat), Some(b"\xCA\x0Adummy_name".to_vec()));
    }

    #[test]
    fn an_open_connection_keeps_its_session_from_idling_out() {
        let store = Arc::new(SessionStore::new());
        let addr = start(store.clone(), UnknownSessions::Refuse);
        let in_lobby = signed_in(&store, "lobby");
        let in_chat = signed_in(&store, "chat");
        let mut lobby = lobby_sign_in(addr, in_lobby);
        assert_eq!(push(&mut lobby).last().unwrap()[0], 0x38);
        let mut chat = chat_sign_in(addr, in_chat);
        assert!(frame(&mut chat).is_some());

        let later = Instant::now() + Duration::from_secs(3600);
        assert!(
            store
                .end_idle_sessions(later, session::SESSION_IDLE_LIMIT)
                .is_empty()
        );

        // Its end is the last sighting.
        drop(lobby);
        let deadline = Instant::now() + Duration::from_secs(2);
        while store.end_idle_sessions(later, session::SESSION_IDLE_LIMIT) != [in_lobby] {
            assert!(
                Instant::now() < deadline,
                "the lobby never let its session go"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(store.account_for_session(in_chat).is_some());
    }
}

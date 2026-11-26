//! login-server (0.69d): TCP framing, sign-in 01/02 and answers 0B..0E,
//! RSA-1024 PKCS#1 v1.5, and the Steam-mode sign-in through the client's Import panel (`10 04` / `04`, see
//! [`handshake`]; the password digest in [`digest`]). Spec: `docs/notes/login.md`.
//!
//! * [`frame`]: the framing, [`handshake`]: the exchange as a state machine without IO,
//!   [`keys`]: the RSA key (path from the srp config, never in the repository).
//! * [`serve`] / [`handle_connection`]: the TCP loop. The caller supplies an [`Authenticator`]; it
//!   sees the account name of the request and decides (grant -> session id + lobby address).

pub mod digest;
pub mod frame;
pub mod handshake;
pub mod keys;

pub use handshake::{
    AcceptAll, Attempt, Authenticator, Granted, Handshake, ImportKind, ImportRequest, LoginError, Outcome, Refusal,
    Reply, SignIn,
};
pub use keys::{KeyError, LoginKey};

use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

/// How long a connection may sit silent.
pub const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a connection may sit silent while the Steam Import panel waits for the player to type a name and password
/// ([`Handshake::read_timeout`]). Choice: 5 minutes. The client keeps the connection for the panel (`10 04` changes
/// neither its login state nor its connection, and `04` goes out on the same connection without a reconnect), so the
/// server must not drop it after 30 s.
pub const IMPORT_READ_TIMEOUT: Duration = Duration::from_secs(300);

/// The `password_type` asked in `0D`: 1 = SHA-1 hex digest of the typed password (0 would send it in clear, other
/// values no digest). A remembered password always sends type 2 + the same stored digest, so both paths carry one
/// digest and the password never travels in clear. PROVEN: 0x20011E320, 0x20011E5F1, 0x200122BD0.
pub const DEFAULT_PASSWORD_TYPE: u8 = 1;

/// Run the exchange on one connection. Returns how it ended (`Ok(None)`: the client left first).
pub fn handle_connection(
    mut stream: TcpStream,
    key: &LoginKey,
    password_type: u8,
    auth: &dyn Authenticator,
) -> io::Result<Option<Outcome>> {
    stream.set_read_timeout(Some(READ_TIMEOUT))?;
    let mut hs = Handshake::new(key, password_type, rand::random());
    hs.peer = stream.peer_addr().ok().map(|a| a.ip());
    let mut reader = frame::FrameReader::default();
    let mut buf = [0u8; 4096];
    loop {
        let n = stream.read(&mut buf)?;
        if n == 0 {
            return Ok(None);
        }
        reader.push(&buf[..n]);
        while let Some(payload) = reader
            .next_frame()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        {
            let reply = hs
                .on_frame(&payload, auth)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            for p in &reply.payloads {
                stream.write_all(&frame::frame(p))?;
            }
            if reply.outcome != Outcome::Continue {
                stream.flush()?;
                return Ok(Some(reply.outcome));
            }
            // longer only while the Steam Import panel waits (no change on the normal path)
            stream.set_read_timeout(Some(hs.read_timeout()))?;
        }
    }
}

/// Accept connections forever, one thread each. `on_outcome` is called with every finished sign-in
/// (account name included).
pub fn serve(
    listener: TcpListener,
    key: Arc<LoginKey>,
    password_type: u8,
    auth: Arc<dyn Authenticator>,
    on_outcome: Arc<dyn Fn(&Outcome) + Send + Sync>,
) -> io::Result<()> {
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                log::warn!("accept failed: {e}");
                continue;
            }
        };
        let (key, auth, on_outcome) = (key.clone(), auth.clone(), on_outcome.clone());
        std::thread::spawn(move || match handle_connection(stream, &key, password_type, auth.as_ref()) {
            Ok(Some(outcome)) => on_outcome(&outcome),
            Ok(None) => log::debug!("login connection closed by the client"),
            Err(e) => log::debug!("login connection ended: {e}"),
        });
    }
    Ok(())
}

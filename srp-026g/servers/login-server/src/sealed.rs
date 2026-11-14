//! A 0.26g sign-in's connection past the user name: the sign-in cipher
//! (`crates/sign-in-cipher`) over the TCP stream, where the game's own
//! protocol had TLS. Read and written as the TLS stream was: a message the
//! game writes at once is one record, and so is each answer.

pub use sign_in_cipher::SKHID;
use sign_in_cipher::{Channel, HEADER, HELLO, Header, MAX_BODY, MAX_MESSAGE, Random};
pub use sign_in_cipher::{ServerKey, ServerPublicKey};
use std::io::{self, Read, Write};
use std::net::TcpStream;

/// The setting that holds the server's lasting key: 64 hex characters.
pub const KEY_SETTING: &str = "SRP_SIGN_IN_KEY";

/// A key's 32 bytes from 64 hex characters.
fn hex32(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    if text.len() != 64 || !text.is_ascii() {
        return None;
    }
    let mut bytes = [0; 32];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(bytes)
}

/// The server's lasting key from its setting's text.
pub fn parse_key(text: &str) -> Result<ServerKey, String> {
    hex32(text)
        .map(ServerKey::from_bytes)
        .ok_or_else(|| format!("{KEY_SETTING} is not 64 hex characters"))
}

/// A server's public key from 64 hex characters (what a server that is not
/// the released game's logs at its start).
pub fn parse_public_key(text: &str) -> Option<ServerPublicKey> {
    hex32(text).map(ServerPublicKey)
}

/// The server's lasting key, from the environment ([`KEY_SETTING`]). The
/// key is no file of the repository: whoever holds it reads the passwords
/// of the games built with its public half.
pub fn key_from_env() -> Result<ServerKey, String> {
    let text = std::env::var(KEY_SETTING).map_err(|_| {
        format!(
            "{KEY_SETTING} is not set: the login server's secret key of the sign-in cipher \
             (docs/notes/sign-in-cipher.md)"
        )
    })?;
    parse_key(&text)
}

/// A key for the tests and their bots: no game is built with it.
pub fn test_key() -> ServerKey {
    ServerKey::from_bytes(*b"skhid sign-in cipher's test key!")
}

/// Says which games the key serves: the released ones, or only games
/// built with this key.
pub fn log_key(key: &ServerKey) {
    let public = key.public();
    if public == sign_in_cipher::SKHID {
        log::info!("Sign-in cipher: the released game's key");
    } else {
        let hex: String = public.0.iter().map(|byte| format!("{byte:02x}")).collect();
        log::warn!(
            "Sign-in cipher: not the released game's key; only a game built with the public key {hex} can sign in"
        );
    }
}

pub struct Sealed {
    stream: TcpStream,
    channel: Channel,
    /// The last record's message, and what of it is not read yet.
    message: [u8; MAX_BODY],
    unread: std::ops::Range<usize>,
}

impl Sealed {
    /// The hellos: the game's read, the server's sent.
    pub fn accept(key: &ServerKey, mut stream: TcpStream) -> io::Result<Self> {
        let mut hello = [0; HELLO];
        stream.read_exact(&mut hello)?;
        let mut random = [0; 32];
        openssl::rand::rand_bytes(&mut random).map_err(io::Error::other)?;
        let (answer, channel) = sign_in_cipher::accept(key, Random::from_generator(random), &hello)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "a hello no game sends"))?;
        stream.write_all(&answer)?;
        Ok(Self {
            stream,
            channel,
            message: [0; MAX_BODY],
            unread: 0..0,
        })
    }
}

impl Read for Sealed {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.unread.is_empty() {
            let mut header = [0; HEADER];
            self.stream.read_exact(&mut header)?;
            let header = Header::parse(header).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "a record length no record has")
            })?;
            let body = &mut self.message[..header.body()];
            self.stream.read_exact(body)?;
            let message = self.channel.open(header, body).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "a record the game did not seal")
            })?;
            self.unread = 0..message.len();
        }
        let count = buffer.len().min(self.unread.len());
        buffer[..count]
            .copy_from_slice(&self.message[self.unread.start..self.unread.start + count]);
        self.unread.start += count;
        Ok(count)
    }
}

impl Write for Sealed {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for message in bytes.chunks(MAX_MESSAGE) {
            // A chunk is at most MAX_MESSAGE: never too long.
            let record = self
                .channel
                .seal(message)
                .map_err(|_| io::ErrorKind::InvalidInput)?;
            self.stream.write_all(record.bytes())?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

/// The game's side, for srp's own clients (the bots, the tests): the hellos
/// on `stream`, `message` sent, and the server's whole reply, read until it
/// closes the connection.
pub fn ask(mut stream: TcpStream, server: ServerPublicKey, message: &[u8]) -> io::Result<Vec<u8>> {
    let mut random = [0; 32];
    openssl::rand::rand_bytes(&mut random).map_err(io::Error::other)?;
    let hello = sign_in_cipher::ClientHello::new(Random::from_generator(random), server);
    stream.write_all(&hello.message())?;
    let mut answer = [0; HELLO];
    stream.read_exact(&mut answer)?;
    let invalid = |what: &'static str| io::Error::new(io::ErrorKind::InvalidData, what);
    let mut channel = hello
        .answered(&answer)
        .map_err(|_| invalid("a hello no server sends"))?;
    for part in message.chunks(MAX_MESSAGE) {
        let record = channel
            .seal(part)
            .map_err(|_| io::ErrorKind::InvalidInput)?;
        stream.write_all(record.bytes())?;
    }
    let mut reply = Vec::new();
    loop {
        let mut header = [0; HEADER];
        // The server closes the link after its reply.
        if stream.read_exact(&mut header).is_err() {
            return Ok(reply);
        }
        let header = Header::parse(header).map_err(|_| invalid("a record length no record has"))?;
        let mut body = vec![0; header.body()];
        stream.read_exact(&mut body)?;
        reply.extend(
            channel
                .open(header, &mut body)
                .map_err(|_| invalid("a record the server did not seal"))?,
        );
    }
}

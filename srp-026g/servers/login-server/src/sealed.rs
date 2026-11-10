//! A 0.26g sign-in's connection past the user name: the sign-in cipher
//! (`crates/sign-in-cipher`) over the TCP stream, where the game's own
//! protocol had TLS. Read and written as the TLS stream was: a message the
//! game writes at once is one record, and so is each answer.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::Path;

use sign_in_cipher::{Channel, HEADER, HELLO, Header, MAX_BODY, MAX_MESSAGE, Random, ServerKey};

/// The server's lasting key, from its file: 64 hex characters. Refused
/// unless it is the key the game is built with (`sign_in_cipher::SKHID`):
/// with any other, every sign-in would fail at the password.
pub fn load_key(path: &Path) -> io::Result<ServerKey> {
    let invalid = |what: &str| io::Error::other(format!("{}: {what}", path.display()));
    let text = std::fs::read_to_string(path).map_err(|error| invalid(&error.to_string()))?;
    let text = text.trim();
    if text.len() != 64 || !text.is_ascii() {
        return Err(invalid("not 64 hex characters"));
    }
    let mut bytes = [0; 32];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let pair = std::str::from_utf8(pair).map_err(|_| invalid("not hex"))?;
        *byte = u8::from_str_radix(pair, 16).map_err(|_| invalid("not hex"))?;
    }
    let key = ServerKey::from_bytes(bytes);
    if key.public() != sign_in_cipher::SKHID {
        return Err(invalid(
            "not the key the game is built with (sign_in_cipher::SKHID)",
        ));
    }
    Ok(key)
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
pub fn ask(mut stream: TcpStream, message: &[u8]) -> io::Result<Vec<u8>> {
    let mut random = [0; 32];
    openssl::rand::rand_bytes(&mut random).map_err(io::Error::other)?;
    let hello =
        sign_in_cipher::ClientHello::new(Random::from_generator(random), sign_in_cipher::SKHID);
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

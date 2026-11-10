//! The cipher of a 0.26g sign-in: what srp's login server and the game
//! (client patch 139) speak where the game's own protocol had its TLS
//! handshake (docs/notes/sign-in-cipher.md).
//!
//! After the server's "user name valid" byte:
//!
//! ```text
//! game   -> server   e_c: the game's X25519 public key, new for this sign-in   (32 bytes)
//! server -> game     e_s: the server's, new for this sign-in                   (32 bytes)
//! both               keys = HKDF-SHA-256(salt = SHA-256(NAME | S | e_c | e_s),
//!                                        secret = DH(e_c, S) | DH(e_c, e_s))
//! then, either way   records: u16 LE length | ChaCha20-Poly1305(message) | tag
//! ```
//!
//! S is the server's lasting public key, which the game carries: only the
//! holder of its secret half derives the keys, so only srp reads the
//! password, and the keys of a sign-in recorded off the wire open nothing
//! sent again later (e_s is new each time). This is Noise's NK pattern.
//!
//! Pure Rust and no allocation: the game links it as it is.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

mod aead;
pub mod game;
mod sha256;
mod x25519;

/// Mixed into the keys: another protocol's messages derive other keys.
const NAME: &[u8] = b"skhid sign-in cipher 1";

/// A hello: one X25519 public key.
pub const HELLO: usize = 32;
/// The longest message of one record. The game's password message is at
/// most 89 bytes and it reads at most 82 of an answer.
pub const MAX_MESSAGE: usize = 255;
/// A record's length field.
pub const HEADER: usize = 2;
/// The longest record body: a message and its tag.
pub const MAX_BODY: usize = MAX_MESSAGE + aead::TAG;

/// 32 bytes of a cryptographic random generator, used for nothing else.
pub struct Random([u8; 32]);

impl Random {
    /// `bytes` must come from the system's (or OpenSSL's) generator.
    pub const fn from_generator(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// The server's lasting secret key.
pub struct ServerKey([u8; 32]);

/// The server's lasting public key: what the game is built with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServerPublicKey(pub [u8; 32]);

/// The Skhid login server's public key: the public half of
/// `srp-026g/certs/sign_in_cipher.key`, which srp refuses to start without.
pub const SKHID: ServerPublicKey = ServerPublicKey([
    0x85, 0x8d, 0xf2, 0xb7, 0xad, 0x42, 0xc2, 0xa5, 0xae, 0x04, 0x7e, 0x9e, 0x9c, 0xae, 0x5a, 0x7b,
    0xaa, 0xd2, 0xdb, 0xa2, 0x12, 0xa2, 0x42, 0xd2, 0x51, 0x2a, 0xab, 0x56, 0x05, 0xca, 0xb4, 0x4b,
]);

impl ServerKey {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn generate(random: Random) -> Self {
        Self(random.0)
    }

    pub const fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    pub fn public(&self) -> ServerPublicKey {
        ServerPublicKey(x25519::scalar_mult(&self.0, &x25519::BASE))
    }
}

/// A hello no honest side sends: a point whose every product is zero.
#[derive(Debug, PartialEq, Eq)]
pub struct BadHello;

/// A record that this channel's other end did not seal, or not as the next.
#[derive(Debug, PartialEq, Eq)]
pub struct Forged;

/// A record length no record has.
#[derive(Debug, PartialEq, Eq)]
pub struct BadLength;

/// A message longer than [`MAX_MESSAGE`].
#[derive(Debug, PartialEq, Eq)]
pub struct TooLong;

/// The game's side between its hello and the server's.
pub struct ClientHello {
    secret: [u8; 32],
    public: [u8; HELLO],
    server: ServerPublicKey,
}

impl ClientHello {
    pub fn new(random: Random, server: ServerPublicKey) -> Self {
        Self {
            secret: random.0,
            public: x25519::scalar_mult(&random.0, &x25519::BASE),
            server,
        }
    }

    /// What the game sends.
    pub const fn message(&self) -> [u8; HELLO] {
        self.public
    }

    /// The channel, once the server's hello is here.
    pub fn answered(self, server_hello: &[u8; HELLO]) -> Result<Channel, BadHello> {
        let lasting = x25519::scalar_mult(&self.secret, &self.server.0);
        let fresh = x25519::scalar_mult(&self.secret, server_hello);
        let [to_server, to_client] =
            keys(&self.server, &self.public, server_hello, &lasting, &fresh)?;
        Ok(Channel {
            send: Way::new(to_server),
            receive: Way::new(to_client),
        })
    }
}

/// The server's side: its hello and the channel, from the game's hello.
pub fn accept(
    key: &ServerKey,
    random: Random,
    client_hello: &[u8; HELLO],
) -> Result<([u8; HELLO], Channel), BadHello> {
    let hello = x25519::scalar_mult(&random.0, &x25519::BASE);
    let lasting = x25519::scalar_mult(&key.0, client_hello);
    let fresh = x25519::scalar_mult(&random.0, client_hello);
    let [to_server, to_client] = keys(&key.public(), client_hello, &hello, &lasting, &fresh)?;
    Ok((
        hello,
        Channel {
            send: Way::new(to_client),
            receive: Way::new(to_server),
        },
    ))
}

/// The two ways' keys: the game's to the server, the server's to the game.
fn keys(
    server: &ServerPublicKey,
    client_hello: &[u8; HELLO],
    server_hello: &[u8; HELLO],
    lasting: &[u8; 32],
    fresh: &[u8; 32],
) -> Result<[[u8; aead::KEY]; 2], BadHello> {
    // A point of small order makes the product zero whatever the secret.
    if *lasting == [0; 32] || *fresh == [0; 32] {
        return Err(BadHello);
    }
    let mut transcript = sha256::Sha256::new();
    for part in [NAME, &server.0, client_hello, server_hello] {
        transcript.update(part);
    }
    let mut secret = [0; 64];
    secret[..32].copy_from_slice(lasting);
    secret[32..].copy_from_slice(fresh);
    let material = sha256::hkdf64(&transcript.finish(), &secret, b"keys");
    let mut keys = [[0; aead::KEY]; 2];
    keys[0].copy_from_slice(&material[..32]);
    keys[1].copy_from_slice(&material[32..]);
    Ok(keys)
}

/// One direction: its key and the number of its next record.
struct Way {
    key: [u8; aead::KEY],
    next: u64,
}

impl Way {
    const fn new(key: [u8; aead::KEY]) -> Self {
        Self { key, next: 0 }
    }

    fn nonce(&self) -> [u8; aead::NONCE] {
        let mut nonce = [0; aead::NONCE];
        nonce[4..].copy_from_slice(&self.next.to_le_bytes());
        nonce
    }
}

/// A sealed record, as it goes on the wire.
pub struct Record {
    bytes: [u8; HEADER + MAX_BODY],
    length: usize,
}

impl Record {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

/// A record's header, read: how long its body is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    bytes: [u8; HEADER],
    body: usize,
}

impl Header {
    pub fn parse(bytes: [u8; HEADER]) -> Result<Self, BadLength> {
        let body = usize::from(u16::from_le_bytes(bytes));
        if (aead::TAG..=MAX_BODY).contains(&body) {
            Ok(Self { bytes, body })
        } else {
            Err(BadLength)
        }
    }

    /// The bytes after the header: the sealed message and its tag.
    pub const fn body(self) -> usize {
        self.body
    }
}

/// Both directions of a sign-in, after the hellos.
pub struct Channel {
    send: Way,
    receive: Way,
}

impl Channel {
    pub fn seal(&mut self, message: &[u8]) -> Result<Record, TooLong> {
        if message.len() > MAX_MESSAGE {
            return Err(TooLong);
        }
        let body = message.len() + aead::TAG;
        let mut bytes = [0; HEADER + MAX_BODY];
        // At most MAX_BODY, which a u16 holds.
        let header = (body as u16).to_le_bytes();
        bytes[..HEADER].copy_from_slice(&header);
        let sealed = &mut bytes[HEADER..HEADER + message.len()];
        sealed.copy_from_slice(message);
        let tag = aead::seal(&self.send.key, &self.send.nonce(), &header, sealed);
        bytes[HEADER + message.len()..HEADER + body].copy_from_slice(&tag);
        self.send.next += 1;
        Ok(Record {
            bytes,
            length: HEADER + body,
        })
    }

    /// The message of the next record, decrypted in place in `body` (which
    /// must be the header's `body()` bytes; any other length is forged).
    pub fn open<'body>(
        &mut self,
        header: Header,
        body: &'body mut [u8],
    ) -> Result<&'body [u8], Forged> {
        if body.len() != header.body {
            return Err(Forged);
        }
        let (sealed, tag) = body.split_at_mut(header.body - aead::TAG);
        let tag: &[u8; aead::TAG] = (&*tag).try_into().map_err(|_| Forged)?;
        if !aead::open(
            &self.receive.key,
            &self.receive.nonce(),
            &header.bytes,
            sealed,
            tag,
        ) {
            return Err(Forged);
        }
        self.receive.next += 1;
        Ok(sealed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    pub fn hex32(text: &str) -> [u8; 32] {
        hex(text).try_into().unwrap()
    }

    fn random(seed: u8) -> Random {
        Random::from_generator(core::array::from_fn(|i| {
            seed.wrapping_mul(31).wrapping_add(i as u8)
        }))
    }

    /// A record taken apart as a reader of the wire does.
    fn parts(record: &Record) -> (Header, Vec<u8>) {
        let bytes = record.bytes();
        let header = Header::parse([bytes[0], bytes[1]]).unwrap();
        assert_eq!(header.body(), bytes.len() - HEADER);
        (header, bytes[HEADER..].to_vec())
    }

    fn connected(server_random: u8) -> (ClientHello, [u8; HELLO], Channel, Channel) {
        let key = ServerKey::generate(random(1));
        let hello = ClientHello::new(random(2), key.public());
        let message = hello.message();
        let (answer, server) = accept(&key, random(server_random), &message).unwrap();
        let client = ClientHello::new(random(2), key.public())
            .answered(&answer)
            .unwrap();
        (hello, message, client, server)
    }

    #[test]
    fn messages_pass_both_ways_in_order() {
        let (_, _, mut client, mut server) = connected(3);
        for message in [&b"\x02digest"[..], b"", &[0xAB; MAX_MESSAGE]] {
            let (header, mut body) = parts(&client.seal(message).unwrap());
            assert_eq!(server.open(header, &mut body).unwrap(), message);
            let (header, mut body) = parts(&server.seal(message).unwrap());
            assert_eq!(client.open(header, &mut body).unwrap(), message);
        }
        assert_eq!(client.seal(&[0; MAX_MESSAGE + 1]).err(), Some(TooLong));
    }

    #[test]
    fn a_record_is_not_its_plain_text_and_differs_each_time() {
        let (_, _, mut client, _) = connected(3);
        let first = client.seal(b"the same password").unwrap();
        let second = client.seal(b"the same password").unwrap();
        assert_ne!(first.bytes(), second.bytes());
        assert!(!first.bytes().windows(8).any(|window| window == b"password"));
    }

    #[test]
    fn a_record_sent_again_or_out_of_order_is_refused() {
        let (_, _, mut client, mut server) = connected(3);
        let (first_header, first) = parts(&client.seal(b"one").unwrap());
        let (second_header, second) = parts(&client.seal(b"two").unwrap());
        assert_eq!(server.open(second_header, &mut second.clone()), Err(Forged));
        assert_eq!(
            server.open(first_header, &mut first.clone()).unwrap(),
            b"one"
        );
        assert_eq!(server.open(first_header, &mut first.clone()), Err(Forged));
        assert_eq!(
            server.open(second_header, &mut second.clone()).unwrap(),
            b"two"
        );
    }

    /// What was recorded of one sign-in opens nothing at the next: the
    /// server's hello is new, so the keys are.
    #[test]
    fn a_recorded_sign_in_replayed_is_refused() {
        let (_, recorded_hello, mut client, _) = connected(3);
        let (header, body) = parts(&client.seal(b"\x02digest").unwrap());
        let key = ServerKey::generate(random(1));
        let (_, mut later) = accept(&key, random(4), &recorded_hello).unwrap();
        assert_eq!(later.open(header, &mut body.clone()), Err(Forged));
    }

    /// Without the server's secret key its place cannot be taken: whoever
    /// answers the hello cannot open the password's record.
    #[test]
    fn another_key_cannot_read_the_game() {
        let real = ServerKey::generate(random(1));
        let impostor = ServerKey::generate(random(9));
        let hello = ClientHello::new(random(2), real.public());
        let (answer, mut impostor_channel) =
            accept(&impostor, random(3), &hello.message()).unwrap();
        let mut client = hello.answered(&answer).unwrap();
        let (header, mut body) = parts(&client.seal(b"\x02digest").unwrap());
        assert_eq!(impostor_channel.open(header, &mut body), Err(Forged));
    }

    #[test]
    fn a_changed_length_or_byte_is_refused() {
        let (_, _, mut client, mut server) = connected(3);
        let record = client.seal(b"message").unwrap();
        let (header, body) = parts(&record);
        // A header that says one byte less, with the body cut to match.
        let shorter = Header::parse(((header.body() - 1) as u16).to_le_bytes()).unwrap();
        assert_eq!(
            server.open(shorter, &mut body[..body.len() - 1].to_vec()),
            Err(Forged)
        );
        assert_eq!(server.open(header, &mut body[1..].to_vec()), Err(Forged));
        let mut changed = body.clone();
        changed[0] ^= 1;
        assert_eq!(server.open(header, &mut changed), Err(Forged));
        assert_eq!(server.open(header, &mut body.clone()).unwrap(), b"message");
    }

    #[test]
    fn lengths_no_record_has_are_refused() {
        assert_eq!(Header::parse([15, 0]), Err(BadLength));
        assert_eq!(
            Header::parse(((MAX_BODY + 1) as u16).to_le_bytes()),
            Err(BadLength)
        );
        assert_eq!(Header::parse([16, 0]).unwrap().body(), 16);
        assert_eq!(
            Header::parse((MAX_BODY as u16).to_le_bytes())
                .unwrap()
                .body(),
            MAX_BODY
        );
    }

    #[test]
    fn a_hello_of_small_order_is_refused() {
        let key = ServerKey::generate(random(1));
        assert!(accept(&key, random(3), &[0; HELLO]).is_err());
        let mut one = [0; HELLO];
        one[0] = 1;
        assert!(accept(&key, random(3), &one).is_err());
        assert!(
            ClientHello::new(random(2), key.public())
                .answered(&[0; HELLO])
                .is_err()
        );
    }
}

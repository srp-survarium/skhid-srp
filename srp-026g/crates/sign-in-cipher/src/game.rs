//! The game's side as the game drives it. The 0.26g client reaches its TLS
//! through three OpenSSL calls (`SSL_connect`, `SSL_write`, `SSL_read`, from
//! boost.asio's engine) and moves the bytes itself, through a pair of
//! buffers; patch 139 answers those calls with this machine instead. A call
//! that needs bytes the server has not sent yet says so, and the game calls
//! again once more have come, as it did with OpenSSL.

use crate::{
    BadHello, Channel, ClientHello, Forged, HEADER, HELLO, Header, MAX_BODY, MAX_MESSAGE, Random,
    ServerPublicKey,
};

/// The game's buffers and its random generator, as the machine needs them.
pub trait Wire {
    /// The server's bytes that are here, at most `into.len()` of them; 0
    /// when none are.
    fn receive(&mut self, into: &mut [u8]) -> usize;
    /// Queues bytes for the server; false when the game cannot take them.
    fn send(&mut self, bytes: &[u8]) -> bool;
    /// 32 bytes of the generator; `None` when it has none to give.
    fn random(&mut self) -> Option<Random>;
}

/// What the game asks.
pub enum Call<'bytes> {
    /// `SSL_connect`: the hellos.
    Connect,
    /// `SSL_write`: a message to seal.
    Write(&'bytes [u8]),
    /// `SSL_read`: room for the server's message.
    Read(&'bytes mut [u8]),
}

/// What the game is told.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    /// Connected (1), or this many bytes written or read.
    Done(usize),
    /// Not yet: call again when more of the server's bytes are here.
    WantRead,
    /// The sign-in cannot go on: the connection is to be dropped.
    Failed,
}

/// Bytes of the server gathered until they are `N`, or a whole record.
struct Gathered<const N: usize> {
    bytes: [u8; N],
    filled: usize,
}

impl<const N: usize> Gathered<N> {
    const fn new() -> Self {
        Self {
            bytes: [0; N],
            filled: 0,
        }
    }

    /// Receives up to `needed` bytes in all; true once they are here.
    fn fill(&mut self, needed: usize, wire: &mut impl Wire) -> bool {
        while self.filled < needed {
            match wire.receive(&mut self.bytes[self.filled..needed]) {
                0 => return false,
                count => self.filled += count,
            }
        }
        true
    }
}

/// A record's message, opened, and what of it the game has not read yet.
struct Message {
    bytes: [u8; MAX_MESSAGE],
    unread: core::ops::Range<usize>,
}

/// A sign-in's cipher on the game's side.
pub struct Session(State);

impl Session {
    /// No sign-in under way.
    pub const fn new() -> Self {
        Self(State::Idle)
    }

    /// The server took the user name: a sign-in's cipher starts here,
    /// whatever was left of an earlier one.
    pub fn begin(&mut self) {
        self.0 = State::Begun;
    }

    /// Answers one call of the game, for a game built with `server`'s key.
    pub fn call(
        &mut self,
        call: Call<'_>,
        server: ServerPublicKey,
        wire: &mut impl Wire,
    ) -> Answer {
        let (state, answer) = core::mem::replace(&mut self.0, State::Idle).step(call, server, wire);
        self.0 = state;
        answer
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

enum State {
    /// No sign-in under way.
    Idle,
    /// The user name was taken; nothing of the cipher is sent yet.
    Begun,
    /// The game's hello is sent; the server's is gathered.
    Hello(ClientHello, Gathered<HELLO>),
    /// Records, both ways.
    Open(Channel, Gathered<{ HEADER + MAX_BODY }>, Message),
}

impl State {
    /// The one transition: the state after `call`, and the game's answer.
    /// A failure leaves no sign-in under way, until the next begins.
    fn step(self, call: Call<'_>, server: ServerPublicKey, wire: &mut impl Wire) -> (Self, Answer) {
        match (self, call) {
            (Self::Begun, Call::Connect) => {
                // Whatever an earlier, abandoned sign-in left unread.
                let mut left = [0; 64];
                while wire.receive(&mut left) != 0 {}
                let Some(random) = wire.random() else {
                    return (Self::Idle, Answer::Failed);
                };
                let hello = ClientHello::new(random, server);
                if !wire.send(&hello.message()) {
                    return (Self::Idle, Answer::Failed);
                }
                (Self::Hello(hello, Gathered::new()), Answer::WantRead)
            }
            (Self::Hello(hello, mut gathered), Call::Connect) => {
                if !gathered.fill(HELLO, wire) {
                    return (Self::Hello(hello, gathered), Answer::WantRead);
                }
                match hello.answered(&gathered.bytes) {
                    Ok(channel) => {
                        let message = Message {
                            bytes: [0; MAX_MESSAGE],
                            unread: 0..0,
                        };
                        (
                            Self::Open(channel, Gathered::new(), message),
                            Answer::Done(1),
                        )
                    }
                    Err(BadHello) => (Self::Idle, Answer::Failed),
                }
            }
            (open @ Self::Open(..), Call::Connect) => (open, Answer::Done(1)),

            (Self::Open(mut channel, gathered, message), Call::Write(bytes)) => {
                for part in bytes.chunks(MAX_MESSAGE) {
                    // A chunk is at most MAX_MESSAGE: never too long.
                    let Ok(record) = channel.seal(part) else {
                        return (Self::Idle, Answer::Failed);
                    };
                    if !wire.send(record.bytes()) {
                        return (Self::Idle, Answer::Failed);
                    }
                }
                (
                    Self::Open(channel, gathered, message),
                    Answer::Done(bytes.len()),
                )
            }

            (Self::Open(mut channel, mut gathered, mut message), Call::Read(into)) => {
                if message.unread.is_empty() {
                    if !gathered.fill(HEADER, wire) {
                        return (Self::Open(channel, gathered, message), Answer::WantRead);
                    }
                    let Ok(header) = Header::parse([gathered.bytes[0], gathered.bytes[1]]) else {
                        return (Self::Idle, Answer::Failed);
                    };
                    if !gathered.fill(HEADER + header.body(), wire) {
                        return (Self::Open(channel, gathered, message), Answer::WantRead);
                    }
                    match channel.open(header, &mut gathered.bytes[HEADER..HEADER + header.body()])
                    {
                        Ok(opened) => {
                            message.bytes[..opened.len()].copy_from_slice(opened);
                            message.unread = 0..opened.len();
                        }
                        Err(Forged) => return (Self::Idle, Answer::Failed),
                    }
                    gathered.filled = 0;
                }
                let count = into.len().min(message.unread.len());
                into[..count].copy_from_slice(
                    &message.bytes[message.unread.start..message.unread.start + count],
                );
                message.unread.start += count;
                (Self::Open(channel, gathered, message), Answer::Done(count))
            }

            // A call out of its turn: OpenSSL's own would fail it too.
            (Self::Idle | Self::Begun | Self::Hello(..), Call::Write(_) | Call::Read(_))
            | (Self::Idle, Call::Connect) => (Self::Idle, Answer::Failed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ServerKey, accept};
    use std::collections::VecDeque;

    /// The game's two buffers: what the server sent and what the game queued.
    #[derive(Default)]
    struct Buffers {
        from_server: VecDeque<u8>,
        to_server: Vec<u8>,
        randoms: u8,
        full: bool,
    }

    impl Wire for Buffers {
        fn receive(&mut self, into: &mut [u8]) -> usize {
            let count = into.len().min(self.from_server.len());
            for byte in &mut into[..count] {
                *byte = self.from_server.pop_front().unwrap();
            }
            count
        }

        fn send(&mut self, bytes: &[u8]) -> bool {
            self.to_server.extend(bytes);
            !self.full
        }

        fn random(&mut self) -> Option<Random> {
            self.randoms += 1;
            Some(Random::from_generator([self.randoms.wrapping_mul(77); 32]))
        }
    }

    fn key() -> ServerKey {
        ServerKey::from_bytes([5; 32])
    }

    /// One call, as the patch makes it; the state after it and the answer.
    fn step(mut session: Session, call: Call<'_>, wire: &mut Buffers) -> (Session, Answer) {
        let answer = session.call(call, key().public(), wire);
        (session, answer)
    }

    fn begun(mut session: Session) -> Session {
        session.begin();
        session
    }

    /// The game's side through the hellos; the server's channel.
    fn connected(wire: &mut Buffers) -> (Session, Channel) {
        wire.to_server.clear();
        let (session, _) = (begun(Session::new()), Answer::Done(0));
        let (session, answer) = step(session, Call::Connect, wire);
        assert_eq!(answer, Answer::WantRead);
        // The game sends the hello, calls again, then waits for the server.
        let hello: [u8; HELLO] = core::mem::take(&mut wire.to_server).try_into().unwrap();
        let (session, answer) = step(session, Call::Connect, wire);
        assert_eq!(answer, Answer::WantRead);
        let (reply, channel) = accept(&key(), Random::from_generator([9; 32]), &hello).unwrap();
        wire.from_server.extend(reply);
        let (session, answer) = step(session, Call::Connect, wire);
        assert_eq!(answer, Answer::Done(1));
        (session, channel)
    }

    /// The server's side: the next record the game queued, opened.
    fn sent(wire: &mut Buffers, server: &mut Channel) -> Vec<u8> {
        let header = Header::parse([wire.to_server[0], wire.to_server[1]]).unwrap();
        let mut body: Vec<u8> = wire
            .to_server
            .drain(..HEADER + header.body())
            .skip(HEADER)
            .collect();
        server.open(header, &mut body).unwrap().to_vec()
    }

    #[test]
    fn a_sign_in_as_the_game_makes_it() {
        let mut wire = Buffers::default();
        let (session, mut server) = connected(&mut wire);
        let (session, answer) = step(session, Call::Write(b"\x02digest"), &mut wire);
        assert_eq!(answer, Answer::Done(7));
        assert_eq!(sent(&mut wire, &mut server), b"\x02digest");
        assert!(wire.to_server.is_empty());

        // The answer is not here yet, then here.
        let mut room = [0; 0x52];
        let (session, answer) = step(session, Call::Read(&mut room), &mut wire);
        assert_eq!(answer, Answer::WantRead);
        wire.from_server
            .extend(server.seal(b"\x08session").unwrap().bytes());
        let (_, answer) = step(session, Call::Read(&mut room), &mut wire);
        assert_eq!(answer, Answer::Done(8));
        assert_eq!(&room[..8], b"\x08session");
    }

    #[test]
    fn the_servers_bytes_may_come_one_at_a_time() {
        let mut wire = Buffers::default();
        let (mut session, _) = (begun(Session::new()), Answer::Done(0));
        (session, _) = step(session, Call::Connect, &mut wire);
        let hello: [u8; HELLO] = core::mem::take(&mut wire.to_server).try_into().unwrap();
        let (reply, mut server) = accept(&key(), Random::from_generator([9; 32]), &hello).unwrap();
        let mut answer = Answer::WantRead;
        for byte in reply {
            assert_eq!(answer, Answer::WantRead);
            wire.from_server.push_back(byte);
            (session, answer) = step(session, Call::Connect, &mut wire);
        }
        assert_eq!(answer, Answer::Done(1));

        let record = server.seal(b"answer").unwrap();
        let mut room = [0; 4];
        let mut answer = Answer::WantRead;
        for byte in record.bytes() {
            assert_eq!(answer, Answer::WantRead);
            wire.from_server.push_back(*byte);
            (session, answer) = step(session, Call::Read(&mut room), &mut wire);
        }
        // A message longer than the room is read in parts.
        assert_eq!((answer, &room), (Answer::Done(4), b"answ"));
        let (_, answer) = step(session, Call::Read(&mut room), &mut wire);
        assert_eq!((answer, &room[..2]), (Answer::Done(2), &b"er"[..]));
    }

    #[test]
    fn a_long_write_is_several_records() {
        let mut wire = Buffers::default();
        let (session, mut server) = connected(&mut wire);
        let long = [0x5A; MAX_MESSAGE + 10];
        let (_, answer) = step(session, Call::Write(&long), &mut wire);
        assert_eq!(answer, Answer::Done(long.len()));
        assert_eq!(sent(&mut wire, &mut server), long[..MAX_MESSAGE]);
        assert_eq!(sent(&mut wire, &mut server), long[MAX_MESSAGE..]);
    }

    /// A sign-in dropped at any point leaves nothing for the next: a new
    /// Begin sends a new hello and ignores what the old one left unread.
    #[test]
    fn a_new_sign_in_starts_over_from_any_state() {
        let mut wire = Buffers::default();
        let (session, _) = (begun(Session::new()), Answer::Done(0));
        let (session, _) = step(session, Call::Connect, &mut wire);
        let first_hello = core::mem::take(&mut wire.to_server);
        wire.from_server.extend([1, 2, 3]);
        let (session, _) = step(session, Call::Connect, &mut wire);

        let (session, _) = (begun(session), Answer::Done(0));
        wire.from_server.extend([4, 5]);
        let (session, answer) = step(session, Call::Connect, &mut wire);
        assert_eq!(answer, Answer::WantRead);
        assert!(wire.from_server.is_empty());
        assert_ne!(wire.to_server, first_hello);
        assert!(matches!(session.0, State::Hello(..)));

        let (session, _) = connected(&mut wire);
        let (session, _) = (begun(session), Answer::Done(0));
        assert!(matches!(session.0, State::Begun));
    }

    #[test]
    fn calls_out_of_turn_and_forgeries_fail_and_end_the_sign_in() {
        let mut wire = Buffers::default();
        let mut room = [0; 8];
        for (session, call) in [
            (Session::new(), Call::Connect),
            (Session::new(), Call::Write(b"x")),
            (Session(State::Begun), Call::Write(b"x")),
            (Session(State::Begun), Call::Read(&mut room)),
        ] {
            let (after, answer) = step(session, call, &mut wire);
            assert_eq!(answer, Answer::Failed);
            assert!(matches!(after.0, State::Idle));
        }

        // A hello of small order.
        let (session, _) = (begun(Session::new()), Answer::Done(0));
        let (session, _) = step(session, Call::Connect, &mut wire);
        wire.from_server.extend([0; HELLO]);
        let (session, answer) = step(session, Call::Connect, &mut wire);
        assert_eq!(answer, Answer::Failed);
        assert!(matches!(session.0, State::Idle));

        // A record changed on the way, and one with a length no record has.
        let (session, mut server) = connected(&mut wire);
        let mut record = server.seal(b"answer").unwrap().bytes().to_vec();
        record[5] ^= 1;
        wire.from_server.extend(record);
        let (session, answer) = step(session, Call::Read(&mut room), &mut wire);
        assert_eq!(answer, Answer::Failed);
        assert!(matches!(session.0, State::Idle));
        wire.from_server.clear();
        let (session, _) = connected(&mut wire);
        wire.from_server.extend([3, 0]);
        let (_, answer) = step(session, Call::Read(&mut room), &mut wire);
        assert_eq!(answer, Answer::Failed);

        // The game's buffer refusing the hello.
        let mut wire = Buffers {
            full: true,
            ..Buffers::default()
        };
        let (session, _) = (begun(Session::new()), Answer::Done(0));
        let (_, answer) = step(session, Call::Connect, &mut wire);
        assert_eq!(answer, Answer::Failed);
    }
}

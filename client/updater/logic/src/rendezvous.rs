//! Finding the newest release without our site
//! (`srp-026g/docs/notes/torrent-downloads.md`, "The rendezvous"): every
//! updater that allows torrent is a peer of one fixed swarm, whose info hash
//! is made of the release's public key ([`info_hash`]), on the public
//! trackers. Peers there have no pieces; they tell each other the revision
//! of the newest signed torrent they have (BEP 10's extension handshake,
//! `skhid_revision`) and send it, whole, to a peer that has an older one or
//! none (the `skhid_torrent` extension message). What comes is taken only
//! when its signature is the release's (`crate::signed`): a peer can lie
//! about nothing but wasting a connection.
//!
//! One connection is one [`Exchange`]: its state and its one transition,
//! [`Exchange::received`].

use alloc::vec::Vec;

use crate::bencode::{self, Value};
use crate::sha1;
use crate::wire::{HANDSHAKE_LEN, PROTOCOL};

/// What the swarm's info hash is made of, with the public key.
pub const DOMAIN: &[u8] = b"skhid-rendezvous 1\n";
/// The name of the extension message that carries a torrent.
pub const EXTENSION: &[u8] = b"skhid_torrent";
/// The id this end asks peers to send it a torrent with.
const OUR_ID: u8 = 1;
/// The largest torrent sent or taken (the release's are a few hundred KB).
pub const MAX_TORRENT: usize = 4 << 20;
/// The largest extension handshake taken.
const MAX_HANDSHAKE: usize = 16 * 1024;

pub fn info_hash(public_key: &[u8; 32]) -> [u8; 20] {
    let mut seed = Vec::with_capacity(DOMAIN.len() + 32);
    seed.extend_from_slice(DOMAIN);
    seed.extend_from_slice(public_key);
    sha1::digest(&seed)
}

/// The handshake of this swarm: the extension protocol's bit (BEP 10)
/// set, the swarm's info hash.
pub fn handshake(info_hash: &[u8; 20], peer_id: &[u8; 20]) -> [u8; HANDSHAKE_LEN] {
    let mut out = crate::wire::handshake(info_hash, peer_id);
    out[25] |= 0x10;
    out
}

fn extended(id: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + payload.len());
    out.extend_from_slice(&(payload.len() as u32 + 2).to_be_bytes());
    out.push(20);
    out.push(id);
    out.extend_from_slice(payload);
    out
}

/// This end's extension handshake: its id for [`EXTENSION`], and the
/// revision of the torrent it has.
pub fn extension_handshake(revision: Option<u64>) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(b"d1:md13:skhid_torrenti1ee");
    if let Some(revision) = revision {
        payload.extend_from_slice(alloc::format!("14:skhid_revisioni{revision}e").as_bytes());
    }
    payload.push(b'e');
    extended(0, &payload)
}

/// The newest torrent this end has: its revision and bytes, and whether
/// it may be sent on this connection (the program limits what it gives
/// away: so many an address, within the upload limit).
#[derive(Debug, Clone, Copy)]
pub struct Ours<'a> {
    pub revision: u64,
    pub torrent: &'a [u8],
    pub give: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Stage {
    /// Their handshake is awaited.
    Handshake,
    /// Their extension handshake is awaited.
    Extensions,
    /// They said a newer revision than ours: their torrent is awaited (and
    /// ours, older, was not sent).
    Waiting,
}

/// One connection of the rendezvous swarm.
#[derive(Debug)]
pub struct Exchange {
    stage: Stage,
    buffer: Vec<u8>,
    /// Where the next message starts in `buffer`.
    at: usize,
}

/// What a connection's bytes lead to.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Bytes to send; the exchange goes on.
    Going(Vec<u8>),
    /// Done: bytes to send, then the connection ends; `torrent`: the one
    /// the peer sent, to be checked by the caller; `gave`: this end's
    /// torrent is in `send`.
    Done {
        send: Vec<u8>,
        torrent: Option<Vec<u8>>,
        gave: bool,
    },
    /// The peer broke the rules: the connection ends.
    Refused,
}

impl Exchange {
    /// A connection just made or taken: what this end sends first (its
    /// handshake and extension handshake) goes with it.
    pub fn start(info_hash: &[u8; 20], peer_id: &[u8; 20], ours: Option<Ours>) -> (Self, Vec<u8>) {
        let mut hello = handshake(info_hash, peer_id).to_vec();
        hello.extend(extension_handshake(ours.map(|ours| ours.revision)));
        (
            Self {
                stage: Stage::Handshake,
                buffer: Vec::new(),
                at: 0,
            },
            hello,
        )
    }

    pub fn received(
        &mut self,
        bytes: &[u8],
        info_hash: &[u8; 20],
        ours: Option<Ours>,
    ) -> Outcome {
        // What was read goes once a push; messages are cut at a cursor.
        self.buffer.drain(..self.at);
        self.at = 0;
        self.buffer.extend_from_slice(bytes);
        let mut send = Vec::new();
        loop {
            let rest = &self.buffer[self.at..];
            if self.stage == Stage::Handshake {
                let Some(hello) = rest.get(..HANDSHAKE_LEN) else {
                    return Outcome::Going(send);
                };
                if hello[0] != 19
                    || &hello[1..20] != PROTOCOL
                    || hello[25] & 0x10 == 0
                    || &hello[28..48] != info_hash
                {
                    return Outcome::Refused;
                }
                self.at += HANDSHAKE_LEN;
                self.stage = Stage::Extensions;
                continue;
            }
            let Some(head) = rest.get(..4) else {
                return Outcome::Going(send);
            };
            let length = u32::from_be_bytes(head.try_into().expect("4 bytes")) as usize;
            let limit = match self.stage {
                Stage::Waiting => MAX_TORRENT + 2,
                _ => MAX_HANDSHAKE + 2,
            };
            if length > limit {
                return Outcome::Refused;
            }
            let Some(body) = rest.get(4..4 + length) else {
                return Outcome::Going(send);
            };
            self.at += 4 + length;
            if body.is_empty() {
                continue; // a keep-alive
            }
            if body.len() < 2 || body[0] != 20 {
                return Outcome::Refused;
            }
            match (&self.stage, body[1]) {
                (Stage::Extensions, 0) => {
                    let Some(theirs) = bencode::parse(&body[2..]) else {
                        return Outcome::Refused;
                    };
                    let their_id = theirs
                        .get(b"m")
                        .and_then(|m| m.get(EXTENSION))
                        .and_then(Value::int)
                        .and_then(|id| u8::try_from(id).ok())
                        .filter(|&id| id != 0);
                    let their_revision = theirs
                        .get(b"skhid_revision")
                        .and_then(Value::int)
                        .and_then(|revision| u64::try_from(revision).ok());
                    let Some(their_id) = their_id else {
                        return Outcome::Refused;
                    };
                    let our_revision = ours.map(|ours| ours.revision);
                    let mut gave = false;
                    if let Some(ours) = ours
                        && ours.give
                        && their_revision.is_none_or(|theirs| theirs < ours.revision)
                    {
                        send.extend(extended(their_id, ours.torrent));
                        gave = true;
                    }
                    if their_revision.is_some_and(|theirs| our_revision.is_none_or(|ours| theirs > ours)) {
                        self.stage = Stage::Waiting;
                        continue;
                    }
                    return Outcome::Done {
                        send,
                        torrent: None,
                        gave,
                    };
                }
                (Stage::Waiting, OUR_ID) => {
                    return Outcome::Done {
                        send,
                        torrent: Some(body[2..].to_vec()),
                        gave: false,
                    };
                }
                _ => return Outcome::Refused,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: [u8; 20] = [5; 20];

    /// Two ends that talk until both are done: what each got.
    fn talk(a: Option<Ours>, b: Option<Ours>) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
        let (mut left, mut to_right) = Exchange::start(&HASH, &[1; 20], a);
        let (mut right, mut to_left) = Exchange::start(&HASH, &[2; 20], b);
        let (mut got_left, mut got_right) = (None, None);
        let (mut left_done, mut right_done) = (false, false);
        for _ in 0..10 {
            if !right_done {
                match right.received(&core::mem::take(&mut to_right), &HASH, b) {
                    Outcome::Going(send) => to_left.extend(send),
                    Outcome::Done { send, torrent, .. } => {
                        to_left.extend(send);
                        got_right = torrent;
                        right_done = true;
                    }
                    Outcome::Refused => panic!("refused"),
                }
            }
            if !left_done {
                match left.received(&core::mem::take(&mut to_left), &HASH, a) {
                    Outcome::Going(send) => to_right.extend(send),
                    Outcome::Done { send, torrent, .. } => {
                        to_right.extend(send);
                        got_left = torrent;
                        left_done = true;
                    }
                    Outcome::Refused => panic!("refused"),
                }
            }
        }
        assert!(left_done && right_done);
        (got_left, got_right)
    }

    #[test]
    fn the_newer_torrent_goes_to_the_older_end_only() {
        let new = Ours { revision: 61, torrent: b"new torrent", give: true };
        let old = Ours { revision: 60, torrent: b"old torrent", give: true };
        assert_eq!(talk(Some(new), Some(old)), (None, Some(b"new torrent".to_vec())));
        assert_eq!(talk(Some(old), Some(new)), (Some(b"new torrent".to_vec()), None));
        assert_eq!(talk(None, Some(old)), (Some(b"old torrent".to_vec()), None));
        assert_eq!(talk(Some(new), Some(new)), (None, None));
        assert_eq!(talk(None, None), (None, None));
        // Not to be given on this connection: it says its revision only
        // (the other end waits, until the program ends the connection).
        let kept = Some(Ours { give: false, ..new });
        let (mut left, _) = Exchange::start(&HASH, &[1; 20], kept);
        let (_, from_right) = Exchange::start(&HASH, &[2; 20], Some(old));
        assert_eq!(
            left.received(&from_right, &HASH, kept),
            Outcome::Done { send: Vec::new(), torrent: None, gave: false }
        );
    }

    #[test]
    fn keep_alives_come_in_their_own_bytes() {
        // A megabyte of keep-alives, a byte a push: each costs its bytes.
        let (mut exchange, _) = Exchange::start(&HASH, &[1; 20], None);
        let hello = handshake(&HASH, &[2; 20]);
        assert_eq!(exchange.received(&hello, &HASH, None), Outcome::Going(Vec::new()));
        let alive = alloc::vec![0_u8; 1 << 20];
        assert_eq!(exchange.received(&alive, &HASH, None), Outcome::Going(Vec::new()));
        for _ in 0..1000 {
            assert_eq!(exchange.received(&[0, 0, 0, 0], &HASH, None), Outcome::Going(Vec::new()));
        }
        assert!(exchange.buffer.len() < 8);
        let theirs = extension_handshake(None);
        assert!(matches!(exchange.received(&theirs, &HASH, None), Outcome::Done { torrent: None, gave: false, .. }));
    }

    #[test]
    fn what_a_stranger_may_not_do() {
        let ours = Some(Ours { revision: 60, torrent: b"t", give: true });
        let refused = |bytes: &[u8]| {
            let (mut exchange, _) = Exchange::start(&HASH, &[1; 20], ours);
            exchange.received(bytes, &HASH, ours)
        };
        // Another swarm, no extension bit.
        assert_eq!(refused(&handshake(&[6; 20], &[2; 20])), Outcome::Refused);
        assert_eq!(refused(&crate::wire::handshake(&HASH, &[2; 20])), Outcome::Refused);
        let hello = handshake(&HASH, &[2; 20]);
        // A torrent nobody asked for (they said no newer revision).
        let mut unasked = hello.to_vec();
        unasked.extend(extension_handshake(Some(60)));
        unasked.extend(extended(OUR_ID, b"junk"));
        let (mut exchange, _) = Exchange::start(&HASH, &[1; 20], ours);
        assert!(matches!(exchange.received(&unasked, &HASH, ours), Outcome::Done { torrent: None, .. }));
        // A BEP 3 message here, or one too long.
        let mut bitfield = hello.to_vec();
        bitfield.extend([0, 0, 0, 2, 5, 0]);
        assert_eq!(refused(&bitfield), Outcome::Refused);
        let mut long = hello.to_vec();
        long.extend(&((MAX_HANDSHAKE + 3) as u32).to_be_bytes());
        assert_eq!(refused(&long), Outcome::Refused);
        assert_ne!(info_hash(&[1; 32]), info_hash(&[2; 32]));
    }
}

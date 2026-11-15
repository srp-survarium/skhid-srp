//! The download and the sharing of one torrent, as one typed state and one
//! transition: [`Swarm::step`] takes an [`Event`] (a connection's bytes, a
//! piece read from the disk, the clock) and gives the [`Action`]s the
//! program does (bytes to send, a connection to close, a checked piece to
//! write, a piece to read). No sockets or files here.
//!
//! The rules (`srp-026g/docs/notes/torrent-downloads.md`):
//! - a piece counts only once its SHA-256 is checked (`Torrent::piece_ok`);
//!   a piece that fails names its sender when one peer sent all of it: that
//!   peer is closed and banned for the run; one that several peers sent is
//!   fetched again from one peer alone, so a liar cannot get honest peers
//!   banned;
//! - only checked pieces are uploaded: a piece on disk is offered, and
//!   checked when it is read, before any byte of it is sent;
//! - fixed limits: peers, requests a peer may queue, requests in flight,
//!   pieces being fetched (their bytes are held until checked), the time a
//!   peer may take, the messages that change nothing; past them the
//!   connection ends. A piece being fetched that no peer can go on with
//!   goes back to the missing ones;
//! - uploads within the run's one allowance (`crate::upload`, shared by
//!   every swarm), to [`UPLOAD_SLOTS`] peers at a time.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use crate::torrent::{BLOCK, Torrent};
use crate::upload::Allowance;
use crate::wire::{self, Block, Message, Reader, Shape};

/// A connection's number, given by the program; never used twice in a run.
pub type PeerKey = u32;

/// The most connections at a time.
pub const MAX_PEERS: usize = 40;
/// Connections with one address at a time (players behind one NAT share
/// theirs).
pub const PER_ADDRESS: usize = 4;
/// Peers uploaded to at a time.
pub const UPLOAD_SLOTS: usize = 4;
/// Requests a peer may have queued with this end; past that it is closed.
const MAX_QUEUE: usize = 256;
/// Requests in flight to one peer.
const PIPELINE: usize = 16;
/// The most bytes of pieces being fetched, held until checked.
const FETCH_BYTES: u64 = 32 << 20;
const MAX_FETCHING: usize = 16;
/// Pieces read from the disk kept for the next requests of them.
const CACHE: usize = 4;
pub const HANDSHAKE_MS: u64 = 15_000;
const SILENT_MS: u64 = 150_000;
const KEEP_ALIVE_MS: u64 = 90_000;
/// A request not answered in this time: the peer is closed.
const REQUEST_MS: u64 = 60_000;
const ROTATE_MS: u64 = 30_000;
/// Messages that change nothing (keep-alives, blocks not asked for, a have
/// of a piece it said it has...) a peer may send in [`ROTATE_MS`]; past
/// that it is closed.
const MAX_USELESS: u32 = 2048;

/// What a piece is at the start, as the program found the disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// Not here, not wanted.
    Absent,
    /// Wanted and not here.
    Missing,
    /// On disk, not checked yet: offered, checked when read.
    Offered,
    /// Wanted, on disk and checked.
    Checked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ask {
    Open,
    Asked { peer: PeerKey, at: u64 },
    Got,
}

/// Who may send a missing piece's blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sourcing {
    /// Any peers, a block each.
    Any,
    /// It failed with blocks of several peers: one peer sends all of it, so
    /// that a wrong one names its sender.
    One,
}

/// Who sends a piece being fetched.
#[derive(Debug)]
enum Senders {
    /// Peers that sent a block of it, with their addresses (kept here: a
    /// peer that left is still banned by its address).
    Any(Vec<(PeerKey, [u8; 4])>),
    /// The one peer asked for all of it.
    One(PeerKey, [u8; 4]),
}

impl Senders {
    fn sourcing(&self) -> Sourcing {
        match self {
            Senders::Any(_) => Sourcing::Any,
            Senders::One(..) => Sourcing::One,
        }
    }

    /// Whether `peer` may be asked a block.
    fn may_ask(&self, peer: PeerKey) -> bool {
        match self {
            Senders::Any(_) => true,
            Senders::One(one, _) => *one == peer,
        }
    }
}

#[derive(Debug)]
struct Fetch {
    data: Vec<u8>,
    blocks: Vec<Ask>,
    senders: Senders,
    /// When a block of it was last asked for or came.
    touched: u64,
}

#[derive(Debug)]
enum Piece {
    Absent,
    Missing(Sourcing),
    Fetching(Fetch),
    /// Of a file shared, on disk: offered, checked when read.
    Offered,
    /// Of a file shared, read and checked.
    Shared,
    /// Wanted and here, checked (fetched, or found in its part).
    Checked,
}

impl Piece {
    fn wanted(&self) -> bool {
        matches!(self, Piece::Missing(_) | Piece::Fetching(_))
    }

    fn here(&self) -> bool {
        matches!(self, Piece::Offered | Piece::Shared | Piece::Checked)
    }
}

/// What this end told a peer about wanting its pieces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Interest {
    No,
    Yes,
}

/// The peer's choke of this end, and the requests in flight to it.
#[derive(Debug)]
enum Download {
    Choked { told: Interest },
    Unchoked { told: Interest, asked: Vec<Block> },
}

impl Download {
    fn told(&self) -> Interest {
        match self {
            Download::Choked { told } | Download::Unchoked { told, .. } => *told,
        }
    }
}

/// This end's choke of the peer.
#[derive(Debug)]
enum Upload {
    /// Not interested (and choked).
    Idle,
    /// Interested, choked: waits for a slot since then.
    Waiting { since: u64 },
    /// Interested, unchoked: its requests.
    Serving { since: u64, queue: VecDeque<Block> },
}

#[derive(Debug)]
enum Has {
    /// Neither a bitfield nor a have yet: a bitfield may come.
    Nothing,
    Pieces(Vec<u8>),
}

#[derive(Debug)]
struct Open {
    has: Has,
    download: Download,
    upload: Upload,
}

#[derive(Debug)]
enum Link {
    Handshake { since: u64 },
    Open(Open),
}

#[derive(Debug)]
struct Peer {
    ip: [u8; 4],
    reader: Reader,
    link: Link,
    heard: u64,
    sent: u64,
    /// Messages that changed nothing since the last rotation.
    useless: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Send(PeerKey, Vec<u8>),
    Close(PeerKey),
    /// A checked piece, to be written where it belongs.
    Write(u32, Vec<u8>),
    /// A piece to be read from the disk and given back as [`Event::Loaded`].
    Load(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event<'a> {
    /// A connection is up (made or taken); this end's handshake goes first.
    Connected { peer: PeerKey, ip: [u8; 4] },
    Received(PeerKey, &'a [u8]),
    Closed(PeerKey),
    /// A piece read from the disk; `None`: it could not be.
    Loaded(u32, Option<Vec<u8>>),
    /// The clock, in milliseconds.
    Tick(u64),
}

/// What a message of a peer did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    /// Nothing changed: counted against [`MAX_USELESS`].
    Useless,
    /// Something changed; nothing more to ask of the peer.
    Noted,
    /// What this end asks of the peer may change.
    Ask,
}

pub struct Swarm {
    torrent: Torrent,
    shape: Shape,
    peer_id: [u8; 20],
    pieces: Vec<Piece>,
    peers: BTreeMap<PeerKey, Peer>,
    banned: Vec<[u8; 4]>,
    now: u64,
    loading: Option<u32>,
    cache: VecDeque<(u32, Vec<u8>)>,
    rotated: u64,
    pub uploaded: u64,
    pub downloaded: u64,
    /// The wanted bytes, and how many of them are checked.
    wanted: u64,
    done: u64,
    /// When a piece last came, checked.
    pub progressed: u64,
}

fn bit(bits: &[u8], piece: u32) -> bool {
    bits.get(piece as usize / 8)
        .is_some_and(|byte| byte & (0x80 >> (piece % 8)) != 0)
}

impl Swarm {
    /// `banned`: the addresses banned so far this run (by other swarms).
    pub fn new(torrent: Torrent, peer_id: [u8; 20], start: &[Start], banned: Vec<[u8; 4]>, now: u64) -> Self {
        let count = torrent.piece_count();
        let shape = Shape {
            pieces: count,
            piece_length: torrent.piece_length,
            last_piece: torrent.piece_size(count - 1),
        };
        let mut wanted = 0;
        let pieces = (0..count)
            .map(|piece| match start.get(piece as usize).copied().unwrap_or(Start::Absent) {
                Start::Absent => Piece::Absent,
                Start::Missing => {
                    wanted += u64::from(shape.piece_size(piece));
                    Piece::Missing(Sourcing::Any)
                }
                Start::Offered => Piece::Offered,
                Start::Checked => Piece::Checked,
            })
            .collect();
        Self {
            torrent,
            shape,
            peer_id,
            pieces,
            peers: BTreeMap::new(),
            banned,
            now,
            loading: None,
            cache: VecDeque::new(),
            rotated: now,
            uploaded: 0,
            downloaded: 0,
            wanted,
            done: 0,
            progressed: now,
        }
    }

    pub fn torrent(&self) -> &Torrent {
        &self.torrent
    }

    /// The wanted bytes checked, of all wanted.
    pub fn progress(&self) -> (u64, u64) {
        (self.done, self.wanted)
    }

    pub fn complete(&self) -> bool {
        !self.pieces.iter().any(Piece::wanted)
    }

    /// Whether anything is here to share.
    pub fn sharing(&self) -> bool {
        self.pieces.iter().any(Piece::here)
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    /// Peers past their handshake that have a piece this end wants.
    pub fn useful_peers(&self) -> usize {
        self.peers
            .values()
            .filter(|peer| matches!(&peer.link, Link::Open(open) if open.download.told() == Interest::Yes))
            .count()
    }

    /// The addresses banned this run, by this swarm or given at its start.
    pub fn banned(&self) -> &[[u8; 4]] {
        &self.banned
    }

    /// Whether a connection with `ip` may be made or taken: room, not
    /// banned, not [`PER_ADDRESS`] connections with it already.
    pub fn may_connect(&self, ip: [u8; 4]) -> bool {
        self.peers.len() < MAX_PEERS
            && !self.banned.contains(&ip)
            && self.peers.values().filter(|peer| peer.ip == ip).count() < PER_ADDRESS
    }

    /// One event, and what it brings. What is uploaded comes out of
    /// `allowance`, the run's one for every swarm (the program fills it by
    /// the clock).
    pub fn step(&mut self, event: Event, allowance: &mut Allowance) -> Vec<Action> {
        let mut out = Vec::new();
        match event {
            Event::Connected { peer, ip } => {
                if !self.may_connect(ip) || self.peers.contains_key(&peer) {
                    out.push(Action::Close(peer));
                } else {
                    self.peers.insert(
                        peer,
                        Peer {
                            ip,
                            reader: Reader::default(),
                            link: Link::Handshake { since: self.now },
                            heard: self.now,
                            sent: self.now,
                            useless: 0,
                        },
                    );
                    let hello = wire::handshake(&self.torrent.v1, &self.peer_id);
                    out.push(Action::Send(peer, hello.into()));
                }
            }
            Event::Received(peer, bytes) => self.received(peer, bytes, &mut out),
            Event::Closed(peer) => self.forget(peer),
            Event::Loaded(piece, data) => self.loaded(piece, data),
            Event::Tick(now) => self.tick(now, &mut out),
        }
        self.serve(allowance, &mut out);
        out
    }

    fn close(&mut self, peer: PeerKey, out: &mut Vec<Action>) {
        if self.peers.contains_key(&peer) {
            self.forget(peer);
            out.push(Action::Close(peer));
        }
    }

    /// A peer gone: its requests in flight go back to be asked of others.
    fn forget(&mut self, peer: PeerKey) {
        self.peers.remove(&peer);
        self.release(peer);
    }

    fn send(&mut self, peer: PeerKey, message: &Message, out: &mut Vec<Action>) {
        if let Some(state) = self.peers.get_mut(&peer) {
            state.sent = self.now;
            out.push(Action::Send(peer, wire::frame(message)));
        }
    }

    fn received(&mut self, key: PeerKey, bytes: &[u8], out: &mut Vec<Action>) {
        let Some(peer) = self.peers.get_mut(&key) else {
            return;
        };
        peer.heard = self.now;
        peer.reader.push(bytes);
        if let Link::Handshake { .. } = peer.link {
            let Some(hello) = peer.reader.handshake() else {
                return;
            };
            match wire::their_handshake(&hello, &self.torrent.v1) {
                Some(id) if id != self.peer_id => {}
                _ => return self.close(key, out),
            }
            peer.link = Link::Open(Open {
                has: Has::Nothing,
                download: Download::Choked { told: Interest::No },
                upload: Upload::Idle,
            });
            let bits = self.bitfield();
            if bits.iter().any(|&byte| byte != 0) {
                self.send(key, &Message::Bitfield(bits), out);
            }
        }
        loop {
            let Some(peer) = self.peers.get_mut(&key) else {
                return;
            };
            match peer.reader.message(&self.shape) {
                Ok(Some(message)) => match self.message(key, message, out) {
                    Ok(Effect::Useless) => {
                        let Some(peer) = self.peers.get_mut(&key) else { return };
                        peer.useless += 1;
                        if peer.useless > MAX_USELESS {
                            return self.close(key, out);
                        }
                    }
                    Ok(Effect::Noted) => {}
                    Ok(Effect::Ask) => self.request(key, out),
                    Err(wire::Refused) => return self.close(key, out),
                },
                Ok(None) => return,
                Err(wire::Refused) => return self.close(key, out),
            }
        }
    }

    fn bitfield(&self) -> Vec<u8> {
        let mut bits = alloc::vec![0_u8; self.shape.bitfield_len()];
        for (piece, state) in self.pieces.iter().enumerate() {
            if state.here() {
                bits[piece / 8] |= 0x80 >> (piece % 8);
            }
        }
        bits
    }

    fn open(&mut self, key: PeerKey) -> Option<&mut Open> {
        match &mut self.peers.get_mut(&key)?.link {
            Link::Open(open) => Some(open),
            Link::Handshake { .. } => None,
        }
    }

    /// One message of an open connection: what it did. `Err`: refused.
    fn message(&mut self, key: PeerKey, message: Message, out: &mut Vec<Action>) -> Result<Effect, wire::Refused> {
        let now = self.now;
        let len = self.shape.bitfield_len();
        Ok(match message {
            Message::KeepAlive => Effect::Useless,
            Message::Bitfield(bits) => {
                let open = self.open(key).ok_or(wire::Refused)?;
                match open.has {
                    Has::Nothing => open.has = Has::Pieces(bits),
                    Has::Pieces(_) => return Err(wire::Refused),
                }
                self.interest(key, out);
                Effect::Ask
            }
            Message::Have(piece) => {
                let wanted = self.pieces[piece as usize].wanted();
                let open = self.open(key).ok_or(wire::Refused)?;
                let bits = match &mut open.has {
                    Has::Pieces(bits) => bits,
                    has @ Has::Nothing => {
                        *has = Has::Pieces(alloc::vec![0; len]);
                        let Has::Pieces(bits) = has else { unreachable!() };
                        bits
                    }
                };
                if bit(bits, piece) {
                    return Ok(Effect::Useless);
                }
                bits[piece as usize / 8] |= 0x80 >> (piece % 8);
                // A have only ever makes this end interested.
                if wanted && open.download.told() == Interest::No {
                    self.interest(key, out);
                }
                Effect::Ask
            }
            Message::Choke => {
                let open = self.open(key).ok_or(wire::Refused)?;
                let Download::Unchoked { told, .. } = open.download else {
                    return Ok(Effect::Useless);
                };
                // What was asked of it is asked of others.
                open.download = Download::Choked { told };
                self.release(key);
                Effect::Noted
            }
            Message::Unchoke => {
                let open = self.open(key).ok_or(wire::Refused)?;
                let Download::Choked { told } = open.download else {
                    return Ok(Effect::Useless);
                };
                open.download = Download::Unchoked {
                    told,
                    asked: Vec::new(),
                };
                Effect::Ask
            }
            Message::Interested => {
                let open = self.open(key).ok_or(wire::Refused)?;
                let Upload::Idle = open.upload else {
                    return Ok(Effect::Useless);
                };
                open.upload = Upload::Waiting { since: now };
                Effect::Noted
            }
            Message::NotInterested => {
                let open = self.open(key).ok_or(wire::Refused)?;
                let serving = match open.upload {
                    Upload::Idle => return Ok(Effect::Useless),
                    Upload::Waiting { .. } => false,
                    Upload::Serving { .. } => true,
                };
                open.upload = Upload::Idle;
                if serving {
                    self.send(key, &Message::Choke, out);
                }
                Effect::Noted
            }
            Message::Request(block) => {
                // A request of a choked peer is dropped (BEP 3), and so is
                // one of a piece not here.
                let here = self.pieces[block.piece as usize].here();
                let open = self.open(key).ok_or(wire::Refused)?;
                match &mut open.upload {
                    Upload::Serving { queue, .. } if here && !queue.contains(&block) => {
                        if queue.len() >= MAX_QUEUE {
                            return Err(wire::Refused);
                        }
                        queue.push_back(block);
                        Effect::Noted
                    }
                    _ => Effect::Useless,
                }
            }
            Message::Cancel(block) => {
                let open = self.open(key).ok_or(wire::Refused)?;
                match &mut open.upload {
                    Upload::Serving { queue, .. } if queue.contains(&block) => {
                        queue.retain(|queued| *queued != block);
                        Effect::Noted
                    }
                    _ => Effect::Useless,
                }
            }
            Message::Piece { block, data } => {
                let open = self.open(key).ok_or(wire::Refused)?;
                let Download::Unchoked { asked, .. } = &mut open.download else {
                    return Ok(Effect::Useless);
                };
                let Some(at) = asked.iter().position(|a| *a == block) else {
                    return Ok(Effect::Useless);
                };
                asked.swap_remove(at);
                self.block(key, block, &data, out);
                Effect::Ask
            }
        })
    }

    /// The requests in flight to `key` go back to be asked of others; a
    /// piece only it was to send goes back to the missing ones.
    fn release(&mut self, key: PeerKey) {
        for piece in &mut self.pieces {
            let Piece::Fetching(fetch) = piece else { continue };
            if let Senders::One(one, _) = fetch.senders
                && one == key
            {
                *piece = Piece::Missing(Sourcing::One);
                continue;
            }
            for ask in &mut fetch.blocks {
                if matches!(ask, Ask::Asked { peer, .. } if *peer == key) {
                    *ask = Ask::Open;
                }
            }
        }
    }

    fn block(&mut self, key: PeerKey, block: Block, data: &[u8], out: &mut Vec<Action>) {
        let Some(ip) = self.peers.get(&key).map(|peer| peer.ip) else {
            return;
        };
        let Piece::Fetching(fetch) = &mut self.pieces[block.piece as usize] else {
            return;
        };
        let index = (block.begin / BLOCK) as usize;
        // Only what was asked of this peer, at that size.
        let size = (self.shape.piece_size(block.piece) - block.begin).min(BLOCK);
        if !matches!(fetch.blocks.get(index), Some(Ask::Asked { peer, .. }) if *peer == key)
            || block.length != size
        {
            return;
        }
        let at = block.begin as usize;
        fetch.data[at..at + data.len()].copy_from_slice(data);
        fetch.blocks[index] = Ask::Got;
        fetch.touched = self.now;
        if let Senders::Any(from) = &mut fetch.senders
            && !from.iter().any(|(peer, _)| *peer == key)
        {
            from.push((key, ip));
        }
        if fetch.blocks.iter().all(|ask| *ask == Ask::Got) {
            let Piece::Fetching(fetch) =
                core::mem::replace(&mut self.pieces[block.piece as usize], Piece::Missing(Sourcing::Any))
            else {
                unreachable!()
            };
            self.fetched(block.piece, fetch, out);
        }
    }

    fn fetched(&mut self, piece: u32, fetch: Fetch, out: &mut Vec<Action>) {
        if !self.torrent.piece_ok(piece, &fetch.data) {
            // A wrong byte. One peer sent it all: that one goes, and does
            // not come back this run (by its address, even if it left).
            // Several did: none is known to be the liar; the piece comes
            // again from one peer alone.
            let guilty = match fetch.senders {
                Senders::One(peer, ip) => Some((peer, ip)),
                Senders::Any(from) if from.len() == 1 => from.first().copied(),
                Senders::Any(_) => None,
            };
            match guilty {
                Some((peer, ip)) => {
                    if !self.banned.contains(&ip) {
                        self.banned.push(ip);
                    }
                    let same: Vec<PeerKey> = self
                        .peers
                        .iter()
                        .filter(|(_, state)| state.ip == ip)
                        .map(|(key, _)| *key)
                        .chain([peer])
                        .collect();
                    for key in same {
                        self.close(key, out);
                    }
                    self.pieces[piece as usize] = Piece::Missing(Sourcing::Any);
                }
                None => self.pieces[piece as usize] = Piece::Missing(Sourcing::One),
            }
            return;
        }
        let size = u64::from(self.shape.piece_size(piece));
        self.downloaded += size;
        self.done += size;
        self.progressed = self.now;
        self.pieces[piece as usize] = Piece::Checked;
        out.push(Action::Write(piece, fetch.data));
        let keys: Vec<PeerKey> = self.peers.keys().copied().collect();
        for key in keys {
            if self.open(key).is_some() {
                self.send(key, &Message::Have(piece), out);
                self.interest(key, out);
            }
        }
    }

    /// Whether the peer has a piece this end wants.
    fn wants_from(&self, has: &Has) -> bool {
        let Has::Pieces(bits) = has else {
            return false;
        };
        self.pieces
            .iter()
            .enumerate()
            .any(|(piece, state)| state.wanted() && bit(bits, piece as u32))
    }

    fn interest(&mut self, key: PeerKey, out: &mut Vec<Action>) {
        let Some(Link::Open(open)) = self.peers.get(&key).map(|peer| &peer.link) else {
            return;
        };
        let want = if self.wants_from(&open.has) {
            Interest::Yes
        } else {
            Interest::No
        };
        let Some(open) = self.open(key) else { return };
        let told = match &mut open.download {
            Download::Choked { told } | Download::Unchoked { told, .. } => told,
        };
        if *told != want {
            *told = want;
            let message = match want {
                Interest::Yes => Message::Interested,
                Interest::No => Message::NotInterested,
            };
            self.send(key, &message, out);
        }
    }

    /// Asks an unchoked peer for blocks it has: of a piece being fetched
    /// first, else of a new piece, the lowest it has.
    fn request(&mut self, key: PeerKey, out: &mut Vec<Action>) {
        let now = self.now;
        let max_fetching = ((FETCH_BYTES / u64::from(self.shape.piece_length)) as usize)
            .clamp(1, MAX_FETCHING);
        let shape = self.shape;
        let Some(Peer {
            link: Link::Open(open),
            sent,
            ip,
            ..
        }) = self.peers.get_mut(&key)
        else {
            return;
        };
        let ip = *ip;
        let (Has::Pieces(bits), Download::Unchoked { told: Interest::Yes, asked }) =
            (&open.has, &mut open.download)
        else {
            return;
        };
        while asked.len() < PIPELINE {
            let mut fetching = 0;
            let mut found = None;
            for (piece, state) in self.pieces.iter_mut().enumerate() {
                if let Piece::Fetching(fetch) = state {
                    fetching += 1;
                    if bit(bits, piece as u32)
                        && fetch.senders.may_ask(key)
                        && let Some(index) = fetch.blocks.iter().position(|a| *a == Ask::Open)
                    {
                        fetch.blocks[index] = Ask::Asked { peer: key, at: now };
                        fetch.touched = now;
                        found = Some((piece as u32, index as u32));
                        break;
                    }
                }
            }
            if found.is_none() && fetching < max_fetching {
                let new = (0..shape.pieces).find_map(|piece| match self.pieces[piece as usize] {
                    Piece::Missing(sourcing) if bit(bits, piece) => Some((piece, sourcing)),
                    _ => None,
                });
                if let Some((piece, sourcing)) = new {
                    let size = shape.piece_size(piece);
                    let mut blocks = alloc::vec![Ask::Open; size.div_ceil(BLOCK) as usize];
                    blocks[0] = Ask::Asked { peer: key, at: now };
                    self.pieces[piece as usize] = Piece::Fetching(Fetch {
                        data: alloc::vec![0; size as usize],
                        blocks,
                        senders: match sourcing {
                            Sourcing::Any => Senders::Any(Vec::new()),
                            Sourcing::One => Senders::One(key, ip),
                        },
                        touched: now,
                    });
                    found = Some((piece, 0));
                }
            }
            let Some((piece, index)) = found else { break };
            let begin = index * BLOCK;
            let block = Block {
                piece,
                begin,
                length: (shape.piece_size(piece) - begin).min(BLOCK),
            };
            asked.push(block);
            *sent = now;
            out.push(Action::Send(key, wire::frame(&Message::Request(block))));
        }
    }

    /// A piece being fetched that nothing is asked of, untouched for
    /// [`REQUEST_MS`], and that no unchoked peer has: back to the missing
    /// ones (its bytes dropped), so it holds no room of the pieces fetched.
    fn idle_fetches(&mut self) {
        let now = self.now;
        for index in 0..self.pieces.len() {
            let Piece::Fetching(fetch) = &self.pieces[index] else { continue };
            if now.saturating_sub(fetch.touched) <= REQUEST_MS
                || fetch.blocks.iter().any(|ask| matches!(ask, Ask::Asked { .. }))
            {
                continue;
            }
            let servable = self.peers.iter().any(|(key, peer)| {
                matches!(&peer.link, Link::Open(Open {
                    has: Has::Pieces(bits),
                    download: Download::Unchoked { .. },
                    ..
                }) if bit(bits, index as u32) && fetch.senders.may_ask(*key))
            });
            if !servable {
                let sourcing = fetch.senders.sourcing();
                self.pieces[index] = Piece::Missing(sourcing);
            }
        }
    }

    fn tick(&mut self, now: u64, out: &mut Vec<Action>) {
        self.now = now;
        let keys: Vec<PeerKey> = self.peers.keys().copied().collect();
        for key in keys {
            let Some(peer) = self.peers.get(&key) else { continue };
            let late = match &peer.link {
                Link::Handshake { since } => now.saturating_sub(*since) > HANDSHAKE_MS,
                Link::Open(_) => {
                    now.saturating_sub(peer.heard) > SILENT_MS
                        || self.pieces.iter().any(|piece| {
                            matches!(piece, Piece::Fetching(fetch) if fetch.blocks.iter().any(|ask|
                                matches!(ask, Ask::Asked { peer, at } if *peer == key
                                    && now.saturating_sub(*at) > REQUEST_MS)))
                        })
                }
            };
            if late {
                self.close(key, out);
            } else if now.saturating_sub(peer.sent) > KEEP_ALIVE_MS {
                self.send(key, &Message::KeepAlive, out);
            }
        }
        if now.saturating_sub(self.rotated) >= ROTATE_MS {
            self.rotated = now;
            for peer in self.peers.values_mut() {
                peer.useless = 0;
            }
            self.rotate(out);
        }
        self.idle_fetches();
        let keys: Vec<PeerKey> = self.peers.keys().copied().collect();
        for key in keys {
            self.request(key, out);
        }
    }

    /// One serving peer of the longest makes room for the one waiting
    /// longest.
    fn rotate(&mut self, out: &mut Vec<Action>) {
        let waiting = self
            .peers
            .values()
            .any(|peer| matches!(peer.link, Link::Open(Open { upload: Upload::Waiting { .. }, .. })));
        let oldest = self
            .peers
            .iter()
            .filter_map(|(key, peer)| match &peer.link {
                Link::Open(Open {
                    upload: Upload::Serving { since, .. },
                    ..
                }) => Some((*since, *key)),
                _ => None,
            })
            .min();
        if let (true, Some((_, key))) = (waiting, oldest)
            && self.serving() >= UPLOAD_SLOTS
        {
            self.send(key, &Message::Choke, out);
            let now = self.now;
            if let Some(open) = self.open(key) {
                open.upload = Upload::Waiting { since: now };
            }
        }
    }

    fn serving(&self) -> usize {
        self.peers
            .values()
            .filter(|peer| matches!(peer.link, Link::Open(Open { upload: Upload::Serving { .. }, .. })))
            .count()
    }

    /// Fills the upload slots, then sends what the allowance lets of the
    /// requests of pieces in the cache, and asks for the next piece to read.
    fn serve(&mut self, allowance: &mut Allowance, out: &mut Vec<Action>) {
        while self.serving() < UPLOAD_SLOTS {
            let next = self
                .peers
                .iter()
                .filter_map(|(key, peer)| match &peer.link {
                    Link::Open(Open {
                        upload: Upload::Waiting { since },
                        ..
                    }) => Some((*since, *key)),
                    _ => None,
                })
                .min();
            let Some((_, key)) = next else { break };
            let now = self.now;
            if let Some(open) = self.open(key) {
                open.upload = Upload::Serving {
                    since: now,
                    queue: VecDeque::new(),
                };
            }
            self.send(key, &Message::Unchoke, out);
        }
        let keys: Vec<PeerKey> = self.peers.keys().copied().collect();
        for key in keys {
            loop {
                if !allowance.room(BLOCK) {
                    return;
                }
                let queue = match self.peers.get_mut(&key).map(|peer| &mut peer.link) {
                    Some(Link::Open(Open {
                        upload: Upload::Serving { queue, .. },
                        ..
                    })) => queue,
                    _ => break,
                };
                let Some(&block) = queue.front() else { break };
                let Some((_, data)) = self.cache.iter().find(|(piece, _)| *piece == block.piece)
                else {
                    if self.loading.is_none() {
                        self.loading = Some(block.piece);
                        out.push(Action::Load(block.piece));
                    }
                    break;
                };
                let at = block.begin as usize;
                let data = data[at..at + block.length as usize].to_vec();
                queue.pop_front();
                allowance.spend(u64::from(block.length));
                self.uploaded += u64::from(block.length);
                self.send(key, &Message::Piece { block, data }, out);
            }
        }
    }

    fn loaded(&mut self, piece: u32, data: Option<Vec<u8>>) {
        if self.loading != Some(piece) {
            return;
        }
        self.loading = None;
        match data.filter(|data| self.torrent.piece_ok(piece, data)) {
            Some(data) => {
                if let state @ Piece::Offered = &mut self.pieces[piece as usize] {
                    *state = Piece::Shared;
                }
                if self.cache.len() >= CACHE {
                    self.cache.pop_front();
                }
                self.cache.push_back((piece, data));
            }
            None => {
                // Not the piece: it is not offered again, and no request of
                // it is answered (the peers ask others). A piece of a file
                // being fetched is wanted again: the fetch is not complete
                // without it.
                let state = &mut self.pieces[piece as usize];
                *state = match state {
                    Piece::Checked => {
                        self.wanted += u64::from(self.shape.piece_size(piece));
                        Piece::Missing(Sourcing::Any)
                    }
                    _ => Piece::Absent,
                };
                for peer in self.peers.values_mut() {
                    if let Link::Open(Open {
                        upload: Upload::Serving { queue, .. },
                        ..
                    }) = &mut peer.link
                    {
                        queue.retain(|block| block.piece != piece);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::torrent::{make, parse};
    use crate::upload::CAP;

    impl Swarm {
        /// A step with an allowance of its own, full: for what is not about
        /// the cap.
        fn on(&mut self, event: Event) -> Vec<Action> {
            self.step(event, &mut Allowance::new(self.now))
        }
    }

    const PIECE: u32 = 32 * 1024;

    fn data() -> (Vec<u8>, Vec<u8>) {
        let big: Vec<u8> = (0..150_000_u32).map(|i| (i % 251) as u8).collect();
        (big, b"tail".to_vec())
    }

    fn torrent() -> Torrent {
        let (big, small) = data();
        let bytes = make(&[("game/big", &big), ("game/small", &small)], PIECE, &[]);
        parse(&bytes).unwrap()
    }

    /// The piece's bytes on the wire.
    fn piece(torrent: &Torrent, index: u32) -> Vec<u8> {
        let (big, small) = data();
        let (file, of_file) = torrent.piece_file(index);
        let start = (u64::from(index) * u64::from(PIECE) - torrent.files[file].offset) as usize;
        let source = if file == 0 { &big } else { &small };
        let mut bytes = source[start..start + of_file as usize].to_vec();
        bytes.resize(torrent.piece_size(index) as usize, 0);
        bytes
    }

    /// Runs two swarms connected as peer 1 of each other until nothing is
    /// said: the disk of each is `disk`, written by its `Write`s.
    struct Pair {
        a: Swarm,
        b: Swarm,
        disk_b: BTreeMap<u32, Vec<u8>>,
        closed: bool,
        /// a's disk has other bytes than the torrent's.
        junk_a: bool,
    }

    impl Pair {
        fn run(&mut self, to_a: Vec<Action>, to_b: Vec<Action>) {
            self.run_within(&mut Allowance::new(0), to_a, to_b);
        }

        /// [`Pair::run`], a uploading within `up`.
        fn run_within(&mut self, up: &mut Allowance, mut to_a: Vec<Action>, mut to_b: Vec<Action>) {
            for _ in 0..10_000 {
                if to_a.is_empty() && to_b.is_empty() {
                    return;
                }
                let mut next_a = Vec::new();
                let mut next_b = Vec::new();
                // `to_a`: what a did; its sends reach b.
                for action in core::mem::take(&mut to_a) {
                    match action {
                        Action::Send(_, bytes) => next_b.extend(self.b.on(Event::Received(1, &bytes))),
                        Action::Close(_) => self.closed = true,
                        Action::Load(index) => {
                            let mut bytes = piece(self.a.torrent(), index);
                            if self.junk_a {
                                bytes.fill(9);
                            }
                            next_a.extend(self.a.step(Event::Loaded(index, Some(bytes)), up));
                        }
                        Action::Write(..) => panic!("the seed writes nothing"),
                    }
                }
                for action in core::mem::take(&mut to_b) {
                    match action {
                        Action::Send(_, bytes) => next_a.extend(self.a.step(Event::Received(1, &bytes), up)),
                        Action::Close(_) => self.closed = true,
                        Action::Load(index) => {
                            let bytes = self.disk_b.get(&index).cloned();
                            next_b.extend(self.b.on(Event::Loaded(index, bytes)));
                        }
                        Action::Write(index, bytes) => {
                            self.disk_b.insert(index, bytes);
                        }
                    }
                }
                to_a = next_a;
                to_b = next_b;
            }
            panic!("no end");
        }
    }

    fn pair(seed: Start, leech: &[Start]) -> (Pair, Vec<Action>, Vec<Action>) {
        let torrent = torrent();
        let count = torrent.piece_count() as usize;
        let a = Swarm::new(torrent.clone(), [1; 20], &alloc::vec![seed; count], Vec::new(), 0);
        let b = Swarm::new(torrent, [2; 20], leech, Vec::new(), 0);
        let mut pair = Pair { a, b, disk_b: BTreeMap::new(), closed: false, junk_a: false };
        let to_a = pair.a.on(Event::Connected { peer: 1, ip: [10, 0, 0, 2] });
        let to_b = pair.b.on(Event::Connected { peer: 1, ip: [10, 0, 0, 1] });
        (pair, to_a, to_b)
    }

    #[test]
    fn a_leech_gets_every_piece_checked_from_a_seed() {
        let count = torrent().piece_count() as usize;
        let (mut pair, to_a, to_b) = pair(Start::Offered, &alloc::vec![Start::Missing; count]);
        pair.run(to_a, to_b);
        assert!(!pair.closed);
        assert!(pair.b.complete());
        assert_eq!(pair.b.progress(), (pair.b.wanted, pair.b.wanted));
        for index in 0..count as u32 {
            assert_eq!(pair.disk_b[&index], piece(pair.b.torrent(), index));
        }
        assert_eq!(pair.a.uploaded, pair.b.downloaded);
    }

    #[test]
    fn only_what_is_wanted_is_fetched_and_what_was_fetched_is_shared() {
        let count = torrent().piece_count() as usize;
        let mut leech = alloc::vec![Start::Absent; count];
        leech[count - 1] = Start::Missing;
        let (mut pair, to_a, to_b) = pair(Start::Offered, &leech);
        pair.run(to_a, to_b);
        assert!(pair.b.complete());
        assert_eq!(pair.disk_b.keys().copied().collect::<Vec<_>>(), [count as u32 - 1]);
        assert_eq!(pair.b.bitfield().iter().map(|b| b.count_ones()).sum::<u32>(), 1);
    }

    #[test]
    fn a_seed_whose_disk_is_wrong_sends_nothing_of_it() {
        let count = torrent().piece_count() as usize;
        let (mut pair, to_a, to_b) = pair(Start::Offered, &alloc::vec![Start::Missing; count]);
        pair.junk_a = true;
        pair.run(to_a, to_b);
        assert_eq!(pair.a.uploaded, 0);
        assert!(pair.disk_b.is_empty());
        assert!(!pair.a.sharing());
    }

    #[test]
    fn a_peer_that_sends_a_wrong_block_is_closed_and_banned() {
        let torrent = torrent();
        let count = torrent.piece_count() as usize;
        let mut leech = Swarm::new(torrent.clone(), [2; 20], &alloc::vec![Start::Missing; count], Vec::new(), 0);
        leech.on(Event::Connected { peer: 3, ip: [6, 6, 6, 6] });
        let mut bytes = wire::handshake(&torrent.v1, &[9; 20]).to_vec();
        bytes.extend(wire::frame(&Message::Bitfield(alloc::vec![0xFC])));
        bytes.extend(wire::frame(&Message::Unchoke));
        let actions = leech.on(Event::Received(3, &bytes));
        let requests: Vec<Block> = actions
            .iter()
            .filter_map(|action| match action {
                Action::Send(3, bytes) => match wire::parse(&bytes[4..], &leech.shape) {
                    Ok(Message::Request(block)) => Some(block),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        // Piece 0's two blocks, of junk.
        let mut actions = Vec::new();
        for block in requests.iter().filter(|block| block.piece == 0) {
            let message = Message::Piece { block: *block, data: alloc::vec![0xAA; block.length as usize] };
            actions.extend(leech.on(Event::Received(3, &wire::frame(&message))));
        }
        assert!(actions.contains(&Action::Close(3)));
        assert!(!actions.iter().any(|action| matches!(action, Action::Write(..))));
        assert!(!leech.may_connect([6, 6, 6, 6]));
        assert!(leech.may_connect([6, 6, 6, 7]));
        assert_eq!(leech.peer_count(), 0);
    }

    #[test]
    fn strangers_that_break_the_rules_are_closed() {
        let torrent = torrent();
        let count = torrent.piece_count() as usize;
        let hello = wire::handshake(&torrent.v1, &[9; 20]).to_vec();
        let other = wire::handshake(&[0; 20], &[9; 20]).to_vec();
        let mine = wire::handshake(&torrent.v1, &[2; 20]).to_vec();
        let mut twice = hello.clone();
        twice.extend(wire::frame(&Message::Bitfield(alloc::vec![0])));
        twice.extend(wire::frame(&Message::Bitfield(alloc::vec![0])));
        let mut huge = hello.clone();
        huge.extend(&100_000_u32.to_be_bytes());
        for bytes in [other, mine, twice, huge] {
            let mut swarm = Swarm::new(torrent.clone(), [2; 20], &alloc::vec![Start::Missing; count], Vec::new(), 0);
            swarm.on(Event::Connected { peer: 1, ip: [5, 5, 5, 5] });
            assert!(swarm.on(Event::Received(1, &bytes)).contains(&Action::Close(1)));
        }
        // Silence, and a handshake that does not come.
        let mut swarm = Swarm::new(torrent.clone(), [2; 20], &alloc::vec![Start::Missing; count], Vec::new(), 0);
        swarm.on(Event::Connected { peer: 1, ip: [5, 5, 5, 5] });
        assert!(swarm.on(Event::Tick(HANDSHAKE_MS + 1)).contains(&Action::Close(1)));
        // Room for MAX_PEERS, PER_ADDRESS of each address.
        let mut swarm = Swarm::new(torrent, [2; 20], &alloc::vec![Start::Missing; count], Vec::new(), 0);
        for peer in 0..MAX_PEERS as u32 {
            let ip = [1, 0, 0, (peer / PER_ADDRESS as u32) as u8];
            assert!(!swarm.on(Event::Connected { peer, ip }).contains(&Action::Close(peer)));
        }
        assert!(swarm.on(Event::Connected { peer: 99, ip: [2, 0, 0, 1] }).contains(&Action::Close(99)));
        swarm.on(Event::Closed(5));
        assert!(swarm.on(Event::Connected { peer: 100, ip: [1, 0, 0, 0] }).contains(&Action::Close(100)));
        assert!(!swarm.on(Event::Connected { peer: 101, ip: [1, 0, 0, 1] }).contains(&Action::Close(101)));
    }

    #[test]
    fn two_swarms_upload_within_one_allowance() {
        let count = torrent().piece_count() as usize;
        let (mut one, to_a1, to_b1) = pair(Start::Offered, &alloc::vec![Start::Missing; count]);
        let (mut two, to_a2, to_b2) = pair(Start::Offered, &alloc::vec![Start::Missing; count]);
        // Lowered to a block a second (the launcher's 1E): its burst is four.
        let mut up = Allowance::new(0);
        up.lower(Some(u64::from(BLOCK)));
        one.run_within(&mut up, to_a1, to_b1);
        two.run_within(&mut up, to_a2, to_b2);
        let burst = u64::from(BLOCK) * 4;
        assert!(one.a.uploaded + two.a.uploaded <= burst, "the second swarm has what the first left");
        for second in 1..=3 {
            up.tick(second * 1000);
            let to_a = one.a.step(Event::Tick(second * 1000), &mut up);
            one.run_within(&mut up, to_a, Vec::new());
            let to_a = two.a.step(Event::Tick(second * 1000), &mut up);
            two.run_within(&mut up, to_a, Vec::new());
        }
        let both = one.a.uploaded + two.a.uploaded;
        assert_eq!(both, up.sent());
        assert!(both <= burst + 3 * u64::from(BLOCK), "{both}");
        assert!(both > burst);
        // The cap itself is the most: a raise is no raise.
        up.lower(Some(CAP * 4));
        assert_eq!(up.rate(), CAP);
    }

    /// A peer `key` at `ip` past its handshake with `swarm`, saying it has
    /// the pieces of `bits` and unchoking it: what the swarm did.
    fn open_peer(swarm: &mut Swarm, key: PeerKey, ip: [u8; 4], bits: u8) -> Vec<Action> {
        let mut actions = swarm.on(Event::Connected { peer: key, ip });
        let mut bytes = wire::handshake(&swarm.torrent.v1, &[key as u8; 20]).to_vec();
        bytes.extend(wire::frame(&Message::Bitfield(alloc::vec![bits])));
        bytes.extend(wire::frame(&Message::Unchoke));
        actions.extend(swarm.on(Event::Received(key, &bytes)));
        actions
    }

    /// The blocks asked of `key` in `actions`.
    fn asked(swarm: &Swarm, actions: &[Action], key: PeerKey) -> Vec<Block> {
        actions
            .iter()
            .filter_map(|action| match action {
                Action::Send(to, bytes) if *to == key => match wire::parse(&bytes[4..], &swarm.shape) {
                    Ok(Message::Request(block)) => Some(block),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    /// `block` as a peer sends it: right, or of junk.
    fn sent(swarm: &mut Swarm, key: PeerKey, block: Block, right: bool) -> Vec<Action> {
        let mut data = piece(&swarm.torrent, block.piece)[block.begin as usize..][..block.length as usize].to_vec();
        if !right {
            data.fill(0xAA);
        }
        swarm.on(Event::Received(key, &wire::frame(&Message::Piece { block, data })))
    }

    #[test]
    fn a_wrong_block_among_others_bans_no_one_and_the_piece_comes_from_one_peer() {
        let torrent = torrent();
        let count = torrent.piece_count() as usize;
        let mut leech = Swarm::new(torrent, [2; 20], &alloc::vec![Start::Missing; count], Vec::new(), 0);
        // The liar has piece 0 only: both its blocks are asked of it; it
        // sends the first, of junk, and leaves.
        let actions = open_peer(&mut leech, 3, [6, 6, 6, 6], 0x80);
        let of_liar = asked(&leech, &actions, 3);
        assert_eq!(of_liar.iter().map(|b| b.piece).collect::<Vec<_>>(), [0, 0]);
        sent(&mut leech, 3, of_liar[0], false);
        leech.on(Event::Closed(3));
        // An honest peer sends the second: the piece fails, of two senders.
        let actions = open_peer(&mut leech, 4, [7, 7, 7, 7], 0xFC);
        let of_honest = asked(&leech, &actions, 4);
        assert_eq!(of_honest[0], of_liar[1]);
        let actions = sent(&mut leech, 4, of_honest[0], true);
        assert!(!actions.iter().any(|a| matches!(a, Action::Close(_) | Action::Write(0, _))));
        assert!(leech.may_connect([6, 6, 6, 6]) && leech.may_connect([7, 7, 7, 7]));
        // Asked again at once, all of it of one peer: the honest one.
        assert!(matches!(&leech.pieces[0], Piece::Fetching(Fetch { senders: Senders::One(4, _), .. })));
        let again: Vec<Block> = asked(&leech, &actions, 4).into_iter().filter(|b| b.piece == 0).collect();
        assert_eq!(again.len(), 2);
        let mut actions = Vec::new();
        for block in again {
            actions.extend(sent(&mut leech, 4, block, true));
        }
        assert!(actions.iter().any(|a| matches!(a, Action::Write(0, _))));
    }

    #[test]
    fn a_fetch_no_peer_goes_on_with_goes_back_to_the_missing_pieces() {
        let torrent = torrent();
        let count = torrent.piece_count() as usize;
        let mut leech = Swarm::new(torrent, [2; 20], &alloc::vec![Start::Missing; count], Vec::new(), 0);
        // A peer that says it has piece 0 and answers nothing.
        let actions = open_peer(&mut leech, 3, [6, 6, 6, 6], 0x80);
        assert_eq!(asked(&leech, &actions, 3).len(), 2);
        assert!(leech.on(Event::Tick(REQUEST_MS + 1)).contains(&Action::Close(3)));
        assert!(matches!(leech.pieces[0], Piece::Missing(Sourcing::Any)));
        // The next peer that has it is asked for it.
        let actions = open_peer(&mut leech, 4, [7, 7, 7, 7], 0xFC);
        assert!(asked(&leech, &actions, 4).iter().any(|b| b.piece == 0));
    }

    #[test]
    fn a_wanted_piece_that_reads_wrong_is_wanted_again() {
        let torrent = torrent();
        let count = torrent.piece_count() as usize;
        let mut swarm = Swarm::new(torrent, [2; 20], &alloc::vec![Start::Checked; count], Vec::new(), 0);
        assert!(swarm.complete());
        swarm.on(Event::Connected { peer: 3, ip: [6, 6, 6, 6] });
        let mut bytes = wire::handshake(&swarm.torrent.v1, &[3; 20]).to_vec();
        bytes.extend(wire::frame(&Message::Interested));
        swarm.on(Event::Received(3, &bytes));
        let block = Block { piece: 0, begin: 0, length: BLOCK };
        let actions = swarm.on(Event::Received(3, &wire::frame(&Message::Request(block))));
        assert!(actions.contains(&Action::Load(0)));
        swarm.on(Event::Loaded(0, Some(alloc::vec![9; PIECE as usize])));
        assert!(!swarm.complete());
        assert_eq!(swarm.progress(), (0, u64::from(PIECE)));
    }

    #[test]
    fn a_peer_that_sends_only_what_changes_nothing_is_closed() {
        let torrent = torrent();
        let count = torrent.piece_count() as usize;
        let mut swarm = Swarm::new(torrent, [2; 20], &alloc::vec![Start::Missing; count], Vec::new(), 0);
        swarm.on(Event::Connected { peer: 3, ip: [6, 6, 6, 6] });
        let mut bytes = wire::handshake(&swarm.torrent.v1, &[3; 20]).to_vec();
        for _ in 0..MAX_USELESS {
            bytes.extend(wire::frame(&Message::KeepAlive));
        }
        assert!(!swarm.on(Event::Received(3, &bytes)).contains(&Action::Close(3)));
        // A new window: as many again are fine; one more is not.
        swarm.on(Event::Tick(ROTATE_MS));
        let alive = wire::frame(&Message::KeepAlive).repeat(MAX_USELESS as usize);
        assert!(!swarm.on(Event::Received(3, &alive)).contains(&Action::Close(3)));
        assert!(swarm.on(Event::Received(3, &wire::frame(&Message::KeepAlive))).contains(&Action::Close(3)));
    }

    #[test]
    fn bans_given_at_the_start_hold() {
        let torrent = torrent();
        let count = torrent.piece_count() as usize;
        let mut swarm = Swarm::new(torrent, [2; 20], &alloc::vec![Start::Missing; count], alloc::vec![[6, 6, 6, 6]], 0);
        assert!(swarm.on(Event::Connected { peer: 3, ip: [6, 6, 6, 6] }).contains(&Action::Close(3)));
        assert_eq!(swarm.banned(), [[6, 6, 6, 6]]);
    }
}

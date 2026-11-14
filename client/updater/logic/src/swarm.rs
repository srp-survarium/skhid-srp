//! The download and the sharing of one torrent, as one typed state and one
//! transition: [`Swarm::step`] takes an [`Event`] (a connection's bytes, a
//! piece read from the disk, the clock) and gives the [`Action`]s the
//! program does (bytes to send, a connection to close, a checked piece to
//! write, a piece to read). No sockets or files here.
//!
//! The rules (`srp-026g/docs/notes/torrent-downloads.md`):
//! - a piece counts only once its SHA-256 is checked (`Torrent::piece_ok`);
//!   a piece that fails closes, and bans for the run, every peer that sent
//!   a block of it;
//! - only checked pieces are uploaded: a piece on disk is offered, and
//!   checked when it is read, before any byte of it is sent;
//! - fixed limits: peers, requests a peer may queue, requests in flight,
//!   pieces being fetched (their bytes are held until checked), the time a
//!   peer may take; past them the connection ends;
//! - uploads within the upload limit, to [`UPLOAD_SLOTS`] peers at a time.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use crate::torrent::{BLOCK, Torrent};
use crate::wire::{self, Block, Message, Reader, Shape};

/// A connection's number, given by the program; never used twice in a run.
pub type PeerKey = u32;

/// The most connections at a time.
pub const MAX_PEERS: usize = 40;
/// Connections with one address at a time (players behind one NAT share
/// theirs).
const PER_ADDRESS: usize = 4;
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
const HANDSHAKE_MS: u64 = 15_000;
const SILENT_MS: u64 = 150_000;
const KEEP_ALIVE_MS: u64 = 90_000;
/// A request not answered in this time: the peer is closed.
const REQUEST_MS: u64 = 60_000;
const ROTATE_MS: u64 = 30_000;

/// What a piece is at the start, as the program found the disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    /// Not here, not wanted.
    Absent,
    /// Wanted and not here.
    Missing,
    /// On disk, not checked yet: offered, checked when read.
    Offered,
    /// On disk and checked.
    Checked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ask {
    Open,
    Asked { peer: PeerKey, at: u64 },
    Got,
}

#[derive(Debug)]
struct Fetch {
    data: Vec<u8>,
    blocks: Vec<Ask>,
    /// Who sent its blocks: closed and banned if it fails.
    from: Vec<PeerKey>,
}

#[derive(Debug)]
enum Piece {
    Absent,
    Missing,
    Fetching(Fetch),
    Offered,
    Checked,
}

impl Piece {
    fn wanted(&self) -> bool {
        matches!(self, Piece::Missing | Piece::Fetching(_))
    }

    fn here(&self) -> bool {
        matches!(self, Piece::Offered | Piece::Checked)
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
    /// Bytes a second this end may upload; `None`: no limit.
    UploadLimit(Option<u64>),
}

pub struct Swarm {
    torrent: Torrent,
    shape: Shape,
    peer_id: [u8; 20],
    pieces: Vec<Piece>,
    peers: BTreeMap<PeerKey, Peer>,
    banned: Vec<[u8; 4]>,
    now: u64,
    limit: Option<u64>,
    tokens: u64,
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
    pub fn new(torrent: Torrent, peer_id: [u8; 20], start: &[Start], now: u64) -> Self {
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
                    Piece::Missing
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
            banned: Vec::new(),
            now,
            limit: None,
            tokens: 0,
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

    /// Whether a connection with `ip` may be made or taken: room, not
    /// banned, not [`PER_ADDRESS`] connections with it already.
    pub fn may_connect(&self, ip: [u8; 4]) -> bool {
        self.peers.len() < MAX_PEERS
            && !self.banned.contains(&ip)
            && self.peers.values().filter(|peer| peer.ip == ip).count() < PER_ADDRESS
    }

    pub fn step(&mut self, event: Event) -> Vec<Action> {
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
            Event::UploadLimit(limit) => {
                self.limit = limit;
                self.tokens = 0;
            }
        }
        self.serve(&mut out);
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
                Ok(Some(message)) => {
                    if self.message(key, message, out).is_err() {
                        return self.close(key, out);
                    }
                }
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

    /// One message of an open connection. `Err`: refused.
    fn message(
        &mut self,
        key: PeerKey,
        message: Message,
        out: &mut Vec<Action>,
    ) -> Result<(), wire::Refused> {
        let now = self.now;
        let len = self.shape.bitfield_len();
        match message {
            Message::KeepAlive => {}
            Message::Bitfield(bits) => {
                let open = self.open(key).ok_or(wire::Refused)?;
                match open.has {
                    Has::Nothing => open.has = Has::Pieces(bits),
                    Has::Pieces(_) => return Err(wire::Refused),
                }
            }
            Message::Have(piece) => {
                let open = self.open(key).ok_or(wire::Refused)?;
                if let Has::Nothing = open.has {
                    open.has = Has::Pieces(alloc::vec![0; len]);
                }
                if let Has::Pieces(bits) = &mut open.has {
                    bits[piece as usize / 8] |= 0x80 >> (piece % 8);
                }
            }
            Message::Choke => {
                let open = self.open(key).ok_or(wire::Refused)?;
                let told = open.download.told();
                // What was asked of it is asked of others.
                open.download = Download::Choked { told };
                self.release(key);
            }
            Message::Unchoke => {
                let open = self.open(key).ok_or(wire::Refused)?;
                if let Download::Choked { told } = open.download {
                    open.download = Download::Unchoked {
                        told,
                        asked: Vec::new(),
                    };
                }
            }
            Message::Interested => {
                let open = self.open(key).ok_or(wire::Refused)?;
                if let Upload::Idle = open.upload {
                    open.upload = Upload::Waiting { since: now };
                }
            }
            Message::NotInterested => {
                let open = self.open(key).ok_or(wire::Refused)?;
                let serving = matches!(open.upload, Upload::Serving { .. });
                open.upload = Upload::Idle;
                if serving {
                    self.send(key, &Message::Choke, out);
                }
            }
            Message::Request(block) => {
                // A request of a choked peer is dropped (BEP 3), and so is
                // one of a piece not here.
                let here = self.pieces[block.piece as usize].here();
                let open = self.open(key).ok_or(wire::Refused)?;
                if let Upload::Serving { queue, .. } = &mut open.upload
                    && here
                    && !queue.contains(&block)
                {
                    if queue.len() >= MAX_QUEUE {
                        return Err(wire::Refused);
                    }
                    queue.push_back(block);
                }
            }
            Message::Cancel(block) => {
                let open = self.open(key).ok_or(wire::Refused)?;
                if let Upload::Serving { queue, .. } = &mut open.upload {
                    queue.retain(|queued| *queued != block);
                }
            }
            Message::Piece { block, data } => {
                let open = self.open(key).ok_or(wire::Refused)?;
                if let Download::Unchoked { asked, .. } = &mut open.download {
                    asked.retain(|a| *a != block);
                }
                self.block(key, block, &data, out);
            }
        }
        self.interest(key, out);
        self.request(key, out);
        Ok(())
    }

    /// The requests in flight to `key` go back to be asked of others.
    fn release(&mut self, key: PeerKey) {
        for piece in &mut self.pieces {
            if let Piece::Fetching(fetch) = piece {
                for ask in &mut fetch.blocks {
                    if matches!(ask, Ask::Asked { peer, .. } if *peer == key) {
                        *ask = Ask::Open;
                    }
                }
            }
        }
    }

    fn block(&mut self, key: PeerKey, block: Block, data: &[u8], out: &mut Vec<Action>) {
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
        if !fetch.from.contains(&key) {
            fetch.from.push(key);
        }
        if fetch.blocks.iter().all(|ask| *ask == Ask::Got) {
            let Piece::Fetching(fetch) =
                core::mem::replace(&mut self.pieces[block.piece as usize], Piece::Missing)
            else {
                unreachable!()
            };
            self.fetched(block.piece, fetch, out);
        }
    }

    fn fetched(&mut self, piece: u32, fetch: Fetch, out: &mut Vec<Action>) {
        if !self.torrent.piece_ok(piece, &fetch.data) {
            // Somebody sent a wrong byte: every peer that sent a block of it
            // goes, and does not come back this run.
            for peer in fetch.from {
                if let Some(state) = self.peers.get(&peer) {
                    let ip = state.ip;
                    if !self.banned.contains(&ip) {
                        self.banned.push(ip);
                    }
                }
                self.close(peer, out);
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
            ..
        }) = self.peers.get_mut(&key)
        else {
            return;
        };
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
                        && let Some(index) = fetch.blocks.iter().position(|a| *a == Ask::Open)
                    {
                        fetch.blocks[index] = Ask::Asked { peer: key, at: now };
                        found = Some((piece as u32, index as u32));
                        break;
                    }
                }
            }
            if found.is_none() && fetching < max_fetching {
                let new = (0..shape.pieces).find(|&piece| {
                    matches!(self.pieces[piece as usize], Piece::Missing) && bit(bits, piece)
                });
                if let Some(piece) = new {
                    let size = shape.piece_size(piece);
                    let mut blocks = alloc::vec![Ask::Open; size.div_ceil(BLOCK) as usize];
                    blocks[0] = Ask::Asked { peer: key, at: now };
                    self.pieces[piece as usize] = Piece::Fetching(Fetch {
                        data: alloc::vec![0; size as usize],
                        blocks,
                        from: Vec::new(),
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

    fn tick(&mut self, now: u64, out: &mut Vec<Action>) {
        let elapsed = now.saturating_sub(self.now);
        self.now = now;
        if let Some(limit) = self.limit {
            let burst = limit.max(u64::from(BLOCK) * 4);
            self.tokens = (self.tokens + limit * elapsed / 1000).min(burst);
        }
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
            self.rotate(out);
        }
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

    /// Fills the upload slots, then sends what the limit lets of the
    /// requests of pieces in the cache, and asks for the next piece to read.
    fn serve(&mut self, out: &mut Vec<Action>) {
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
                if self.limit.is_some() && self.tokens < u64::from(BLOCK) {
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
                if self.limit.is_some() {
                    self.tokens -= u64::from(block.length);
                }
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
                    *state = Piece::Checked;
                }
                if self.cache.len() >= CACHE {
                    self.cache.pop_front();
                }
                self.cache.push_back((piece, data));
            }
            None => {
                // Not the piece: it is not offered again, and no request of
                // it is answered (the peers ask others).
                self.pieces[piece as usize] = Piece::Absent;
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
        fn run(&mut self, mut to_a: Vec<Action>, mut to_b: Vec<Action>) {
            for _ in 0..10_000 {
                if to_a.is_empty() && to_b.is_empty() {
                    return;
                }
                let mut next_a = Vec::new();
                let mut next_b = Vec::new();
                // `to_a`: what a did; its sends reach b.
                for action in core::mem::take(&mut to_a) {
                    match action {
                        Action::Send(_, bytes) => next_b.extend(self.b.step(Event::Received(1, &bytes))),
                        Action::Close(_) => self.closed = true,
                        Action::Load(index) => {
                            let mut bytes = piece(self.a.torrent(), index);
                            if self.junk_a {
                                bytes.fill(9);
                            }
                            next_a.extend(self.a.step(Event::Loaded(index, Some(bytes))));
                        }
                        Action::Write(..) => panic!("the seed writes nothing"),
                    }
                }
                for action in core::mem::take(&mut to_b) {
                    match action {
                        Action::Send(_, bytes) => next_a.extend(self.a.step(Event::Received(1, &bytes))),
                        Action::Close(_) => self.closed = true,
                        Action::Load(index) => {
                            let bytes = self.disk_b.get(&index).cloned();
                            next_b.extend(self.b.step(Event::Loaded(index, bytes)));
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
        let a = Swarm::new(torrent.clone(), [1; 20], &alloc::vec![seed; count], 0);
        let b = Swarm::new(torrent, [2; 20], leech, 0);
        let mut pair = Pair { a, b, disk_b: BTreeMap::new(), closed: false, junk_a: false };
        let to_a = pair.a.step(Event::Connected { peer: 1, ip: [10, 0, 0, 2] });
        let to_b = pair.b.step(Event::Connected { peer: 1, ip: [10, 0, 0, 1] });
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
        let mut leech = Swarm::new(torrent.clone(), [2; 20], &alloc::vec![Start::Missing; count], 0);
        leech.step(Event::Connected { peer: 3, ip: [6, 6, 6, 6] });
        let mut bytes = wire::handshake(&torrent.v1, &[9; 20]).to_vec();
        bytes.extend(wire::frame(&Message::Bitfield(alloc::vec![0xFC])));
        bytes.extend(wire::frame(&Message::Unchoke));
        let actions = leech.step(Event::Received(3, &bytes));
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
            actions.extend(leech.step(Event::Received(3, &wire::frame(&message))));
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
            let mut swarm = Swarm::new(torrent.clone(), [2; 20], &alloc::vec![Start::Missing; count], 0);
            swarm.step(Event::Connected { peer: 1, ip: [5, 5, 5, 5] });
            assert!(swarm.step(Event::Received(1, &bytes)).contains(&Action::Close(1)));
        }
        // Silence, and a handshake that does not come.
        let mut swarm = Swarm::new(torrent.clone(), [2; 20], &alloc::vec![Start::Missing; count], 0);
        swarm.step(Event::Connected { peer: 1, ip: [5, 5, 5, 5] });
        assert!(swarm.step(Event::Tick(HANDSHAKE_MS + 1)).contains(&Action::Close(1)));
        // Room for MAX_PEERS, PER_ADDRESS of each address.
        let mut swarm = Swarm::new(torrent, [2; 20], &alloc::vec![Start::Missing; count], 0);
        for peer in 0..MAX_PEERS as u32 {
            let ip = [1, 0, 0, (peer / PER_ADDRESS as u32) as u8];
            assert!(!swarm.step(Event::Connected { peer, ip }).contains(&Action::Close(peer)));
        }
        assert!(swarm.step(Event::Connected { peer: 99, ip: [2, 0, 0, 1] }).contains(&Action::Close(99)));
        swarm.step(Event::Closed(5));
        assert!(swarm.step(Event::Connected { peer: 100, ip: [1, 0, 0, 0] }).contains(&Action::Close(100)));
        assert!(!swarm.step(Event::Connected { peer: 101, ip: [1, 0, 0, 1] }).contains(&Action::Close(101)));
    }

    #[test]
    fn uploads_keep_to_the_limit() {
        let count = torrent().piece_count() as usize;
        let (mut pair, to_a, to_b) = pair(Start::Offered, &alloc::vec![Start::Missing; count]);
        pair.a.step(Event::UploadLimit(Some(u64::from(BLOCK))));
        pair.run(to_a, to_b);
        assert_eq!(pair.a.uploaded, 0, "no token before a second passed");
        for second in 1..=3 {
            let to_a = pair.a.step(Event::Tick(second * 1000));
            pair.run(to_a, Vec::new());
        }
        // The burst (4 blocks) and one a second.
        assert!(pair.a.uploaded <= u64::from(BLOCK) * 4, "{}", pair.a.uploaded);
        assert!(pair.a.uploaded >= u64::from(BLOCK));
    }
}

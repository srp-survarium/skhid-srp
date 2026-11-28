//! The wire between the 0.69d relay (`servers/match-server`) and the world host daemon (`srp-world-host`).
//!
//! **Transport.** One unix stream socket per match (EOF = that match's host is gone). Every message is one frame:
//!
//! ```text
//! u32 len   (little-endian; the bytes that follow it: 1 type byte + the payload; 1 ..= MAX_FRAME)
//! u8  type
//! payload
//! ```
//!
//! **Encoding of the payloads.** Little-endian integers. `str` = `u16 len` + that many UTF-8 bytes (at most
//! [`MAX_TEXT`]). `blob` = `u32 len` + bytes. Counts of players are a `u8` (at most [`MAX_PLAYERS`]). A payload must be
//! consumed exactly: trailing bytes are an error. Every frame from the other side is untrusted: the decoders bound every
//! count and length before allocating.
//!
//! **Relay -> host** ([`ToHost`]): `Create` 0x01, `Df` 0x02, `WantDe` 0x03 (phase 2), `Close` 0x04.
//! **Project list** (its own connection, outside the match frames): `ListProjects` 0x05 (no payload) is answered with
//! one `Projects` 0x8B ([`projects_payload`] / [`decode_projects`]) and the connection is closed: the lobby asks the
//! daemon which level projects it serves instead of reading the daemon's config itself.
//! **Room status** (its own connection too): `ListStatus` 0x06 (no payload) is answered with one `Status` 0x8C
//! ([`status::HostStatus`]): the loaded maps with their matches and room, and whether a new map can load.
//! **Host -> relay** ([`FromHost`]): `Ready` 0x81, `Tick` 0x82 (phase 2; phase 1 writes them to a file per match),
//! `Outcome` 0x83, `Stats` 0x84, `De` 0x85 (phase 2), `Final` 0x86, `Diverged` 0x87, `Kills` 0x89 (version 3),
//! `Used` 0x8A (version 6), `Fault` 0x8F.
//!
//! Encoding choices (both sides use this crate, so they agree):
//! * `len` counts the type byte and the payload (not itself);
//! * `str` is `u16 len` + UTF-8;
//! * `Stats` carries `u32 t` (the host's world time of the read-out) before the per-player part, and the two team
//!   scores after it, like `Outcome`;
//! * `Final` has exactly the `Outcome` layout (read at `Close.t_last`).

use std::fmt;
use std::io::{self, Read, Write};

/// Version 5: the host's per-tick bot entries ([`FromHost::Bots`]).
pub mod bots;
/// The room status query ([`status::HostStatus`]).
pub mod status;

/// Version carried by `Create`. A host that does not speak it answers `Fault`.
/// 2: `PlayerStats` has `keys_spent` / `gold_spent` (PvE paid respawns).
/// 3: `Outcome` / `Final` have the per-player detail trailer ([`PlayerDetail`]: the 74 event rows, kill streaks, the
/// inactive flag, the badge counts), and there is the `Kills` frame ([`KillRecord`]) (one version for both).
/// 4: the host builds the start world: `Create.world` may be empty, and then `Ready` carries the start item
/// ([`Ready::start`]); every version-3 frame is unchanged, and a host answers a version-3 `Create` with a `Ready`
/// without the start item.
/// 5: host bots: `Create` has `u8 drive_bots` at its end, and the host answers each stepped DF with a [`bots::Bots`]
/// frame (0x88) when it is set; versions 3 and 4 decode and behave as in version 4.
/// 6: the `Used` frame (0x8A, [`Used`]): what each human used in the match (the profiles he spawned with, their weapons,
/// his ammo / pocket stacks' use), sent before `Final` only when the `Create` says 6; every other frame is unchanged.
/// 7: `Create` has `u8 ai_ignore_players` after `drive_bots` (a custom room's "bots ignore players": the host sets the
/// exe's own harmless-bots byte before its AI runs); versions 3 to 6 decode with the flag off.
/// 8: optional trailers, sent only when the `Create` says 8 (a version-7 frame decodes unchanged): `Kills` ends with
/// `n x u32 assists` ([`KillRecord::assists`]), `Final` ends with `nd x Vitals` ([`PlayerDetail::vitals`]).
pub const PROTO_VERSION: u16 = 8;
/// The oldest `Create.version` a host still serves (the frames of version 3 are a subset of version 4).
pub const MIN_PROTO_VERSION: u16 = 3;
/// Largest `len` of a frame (type byte + payload): 4 MiB (the relay accepts options blobs up to
/// 0x39C0C8 bytes; real ones are about 18 KB, a DE item about 36 KB).
pub const MAX_FRAME: usize = 4 * 1024 * 1024;
/// Players of a match (the client's limit, records of the options).
pub const MAX_PLAYERS: usize = 20;
/// Longest text (project name, fault reason), bytes.
pub const MAX_TEXT: usize = 256;
/// Events of the statistics matrix (the event-name table 0x200986E10: kill 0, assist 1, death 2, ..., research_progress 73).
pub const EVENTS: usize = 74;
/// Badge slots of a player block (`B+0x534`, 27 x 12 bytes; slot t = badge type t of `badges.options`).
pub const BADGES: usize = 27;
/// Most kill records in one `Kills` frame.
pub const MAX_KILLS: usize = 2048;

/// Frame types.
pub mod ty {
    pub const CREATE: u8 = 0x01;
    pub const DF: u8 = 0x02;
    pub const WANT_DE: u8 = 0x03;
    pub const CLOSE: u8 = 0x04;
    /// the project-list query (its own connection, no payload)
    pub const LIST_PROJECTS: u8 = 0x05;
    /// the room status query (its own connection, no payload)
    pub const LIST_STATUS: u8 = 0x06;
    pub const READY: u8 = 0x81;
    pub const TICK: u8 = 0x82;
    pub const OUTCOME: u8 = 0x83;
    pub const STATS: u8 = 0x84;
    pub const DE: u8 = 0x85;
    pub const FINAL: u8 = 0x86;
    pub const DIVERGED: u8 = 0x87;
    pub const BOTS: u8 = 0x88;
    pub const KILLS: u8 = 0x89;
    pub const USED: u8 = 0x8A;
    /// the answer to `LIST_PROJECTS`: `u16 n`, n x `str` (the level project names)
    pub const PROJECTS: u8 = 0x8B;
    /// the answer to `LIST_STATUS` ([`crate::status::HostStatus`])
    pub const STATUS: u8 = 0x8C;
    pub const FAULT: u8 = 0x8F;
}

/// `Create.kind`.
pub const KIND_PVP: u8 = 0;
pub const KIND_PVE: u8 = 1;

/// World end states the host reports (`W+0x180AE0`, docs/notes/online.md 3.1): 3 = running, 4 / 5 = ended.
pub const STATE_RUNNING: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The payload ended before a field.
    Short,
    /// Bytes left after the last field.
    Trailing(usize),
    /// A frame length of 0 or above [`MAX_FRAME`].
    FrameLen(u32),
    /// A frame type this side does not take.
    Type(u8),
    /// A count or length over its bound.
    Limit(&'static str),
    /// A text that is not UTF-8.
    Utf8,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Short => write!(f, "payload too short"),
            Error::Trailing(n) => write!(f, "{n} trailing byte(s)"),
            Error::FrameLen(n) => write!(f, "frame length {n} outside 1..={MAX_FRAME}"),
            Error::Type(t) => write!(f, "unexpected frame type {t:#04x}"),
            Error::Limit(what) => write!(f, "limit exceeded: {what}"),
            Error::Utf8 => write!(f, "text is not UTF-8"),
        }
    }
}

impl std::error::Error for Error {}

impl From<Error> for io::Error {
    fn from(e: Error) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, e)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// byte writer / reader

#[derive(Default)]
struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.0.extend_from_slice(b);
        self
    }
    fn blob(&mut self, b: &[u8]) -> &mut Self {
        self.u32(b.len() as u32).bytes(b)
    }
    fn str(&mut self, s: &str) -> &mut Self {
        self.u16(s.len() as u16).bytes(s.as_bytes())
    }
}

struct R<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> R<'a> {
    fn new(b: &'a [u8]) -> Self {
        R { b, p: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.p.checked_add(n).ok_or(Error::Short)?;
        let s = self.b.get(self.p..end).ok_or(Error::Short)?;
        self.p = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, Error> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Result<u32, Error> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    /// `u32 len` + bytes, at most `max`.
    fn blob(&mut self, max: usize, what: &'static str) -> Result<Vec<u8>, Error> {
        let n = self.u32()? as usize;
        if n > max {
            return Err(Error::Limit(what));
        }
        Ok(self.take(n)?.to_vec())
    }
    fn str(&mut self, what: &'static str) -> Result<String, Error> {
        let n = usize::from(self.u16()?);
        if n > MAX_TEXT {
            return Err(Error::Limit(what));
        }
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| Error::Utf8)
    }
    fn count(&mut self, what: &'static str) -> Result<usize, Error> {
        let n = usize::from(self.u8()?);
        if n > MAX_PLAYERS {
            return Err(Error::Limit(what));
        }
        Ok(n)
    }
    fn end(&self) -> Result<(), Error> {
        match self.b.len() - self.p {
            0 => Ok(()),
            n => Err(Error::Trailing(n)),
        }
    }
}

fn check_text(s: &str, what: &'static str) -> Result<(), Error> {
    if s.len() > MAX_TEXT { Err(Error::Limit(what)) } else { Ok(()) }
}

fn check_players(n: usize, what: &'static str) -> Result<(), Error> {
    if n > MAX_PLAYERS { Err(Error::Limit(what)) } else { Ok(()) }
}

// ---------------------------------------------------------------------------------------------------------------------
// frames

/// One frame: its type and payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub ty: u8,
    pub payload: Vec<u8>,
}

/// The bytes of one frame (`u32 len, u8 type, payload`). Fails when the frame would exceed [`MAX_FRAME`].
pub fn frame_bytes(ty: u8, payload: &[u8]) -> Result<Vec<u8>, Error> {
    let len = payload.len() + 1;
    if len > MAX_FRAME {
        return Err(Error::FrameLen(len.min(u32::MAX as usize) as u32));
    }
    let mut v = Vec::with_capacity(4 + len);
    v.extend_from_slice(&(len as u32).to_le_bytes());
    v.push(ty);
    v.extend_from_slice(payload);
    Ok(v)
}

/// Write one frame.
pub fn write_frame<Wr: Write + ?Sized>(w: &mut Wr, ty: u8, payload: &[u8]) -> io::Result<()> {
    w.write_all(&frame_bytes(ty, payload)?)
}

/// Read one frame (blocking). `Ok(None)` = a clean EOF at a frame boundary; an EOF inside a frame is an error.
pub fn read_frame<Rd: Read + ?Sized>(r: &mut Rd) -> io::Result<Option<Frame>> {
    let mut head = [0u8; 4];
    let mut got = 0;
    while got < 4 {
        match r.read(&mut head[got..]) {
            Ok(0) if got == 0 => return Ok(None),
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    let len = u32::from_le_bytes(head);
    if len == 0 || len as usize > MAX_FRAME {
        return Err(Error::FrameLen(len).into());
    }
    let mut body = vec![0u8; len as usize];
    r.read_exact(&mut body)?;
    let ty = body[0];
    body.remove(0);
    Ok(Some(Frame { ty, payload: body }))
}

/// Incremental frame splitter for non-blocking readers: feed bytes with [`push`](Self::push), take frames with
/// [`next_frame`](Self::next_frame).
#[derive(Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }

    /// Bytes held that do not make a whole frame yet.
    pub fn pending(&self) -> usize {
        self.buf.len()
    }

    /// The next whole frame, `Ok(None)` when more bytes are needed. A bad length is an error (the stream is then lost).
    pub fn next_frame(&mut self) -> Result<Option<Frame>, Error> {
        if self.buf.len() < 4 {
            return Ok(None);
        }
        let len = u32::from_le_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]);
        if len == 0 || len as usize > MAX_FRAME {
            return Err(Error::FrameLen(len));
        }
        let total = 4 + len as usize;
        if self.buf.len() < total {
            return Ok(None);
        }
        let ty = self.buf[4];
        let payload = self.buf[5..total].to_vec();
        self.buf.drain(..total);
        Ok(Some(Frame { ty, payload }))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// relay -> host

/// One options record of the match (records order = the options' player order: humans, then the script bots).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record {
    pub bot: bool,
    pub team: u8,
}

/// `Create` (0x01): the match the host must build. The same head as the relay's `.srprec` recording (`Recorder::open`),
/// the host's proven input: `u16 version, u32 match_id, u8 kind, str project, u32 map, u32 mode, u8 tick_ms, u8 n,
/// n x {u8 bot, u8 team}, u32 len + options (raw DD blob), u32 len + DE item (decoded, roster applied)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Create {
    pub version: u16,
    pub match_id: u32,
    /// [`KIND_PVP`] or [`KIND_PVE`]
    pub kind: u8,
    /// level project (`pve_01_v1_maze2`, ...); empty when the relay has none
    pub project: String,
    pub map: u32,
    pub mode: u32,
    pub tick_ms: u8,
    pub records: Vec<Record>,
    /// the raw (uncompressed) `match_options` blob the clients get in DD
    pub options: Vec<u8>,
    /// the decoded world item of the first DE (roster applied); version 4: empty = the host builds the start world from
    /// the options and the level and sends it in `Ready` ([`Ready::start`])
    pub world: Vec<u8>,
    /// version 5: the host runs the exe's AI for every options record with `ai_id != -1` and sends its entries in a
    /// `Bots` frame per stepped DF (`u8` 0 / 1 after `world`; absent below version 5 = false)
    pub drive_bots: bool,
    /// version 7: the AI ignores the human players (a custom room's "bots ignore players", the 98 / `62 1E` byte): the
    /// host sets the exe's global byte 0x20F43FE53 before its AI runs; the AI's vision target filter (0x20032CA70) and
    /// sound filter (0x200368B50) then skip every record with `ai_id == -1`. Absent below version 7 = false.
    pub ai_ignore_players: bool,
}

/// `Df` (0x02): `u32 t, u32 len, DF bytes`: the relay's per-tick DF bytes before the per-recipient filtering, no
/// confirmations (what the recorder writes). The fin DFs (every recipient's leave bit) are never sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Df {
    pub t: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToHost {
    Create(Create),
    Df(Df),
    /// `WantDe` (0x03, phase 2): `u32 request_id`
    WantDe { request_id: u32 },
    /// `Close` (0x04): `u32 t_last`, the last normal DF
    Close { t_last: u32 },
}

impl ToHost {
    pub fn ty(&self) -> u8 {
        match self {
            ToHost::Create(_) => ty::CREATE,
            ToHost::Df(_) => ty::DF,
            ToHost::WantDe { .. } => ty::WANT_DE,
            ToHost::Close { .. } => ty::CLOSE,
        }
    }

    pub fn payload(&self) -> Result<Vec<u8>, Error> {
        let mut w = W::default();
        match self {
            ToHost::Create(c) => {
                check_text(&c.project, "Create project")?;
                check_players(c.records.len(), "Create records")?;
                w.u16(c.version).u32(c.match_id).u8(c.kind).str(&c.project).u32(c.map).u32(c.mode).u8(c.tick_ms);
                w.u8(c.records.len() as u8);
                for r in &c.records {
                    w.u8(u8::from(r.bot)).u8(r.team);
                }
                w.blob(&c.options).blob(&c.world);
                if c.version >= 5 {
                    w.u8(u8::from(c.drive_bots));
                }
                if c.version >= 7 {
                    w.u8(u8::from(c.ai_ignore_players));
                }
            }
            ToHost::Df(d) => {
                w.u32(d.t).blob(&d.bytes);
            }
            ToHost::WantDe { request_id } => {
                w.u32(*request_id);
            }
            ToHost::Close { t_last } => {
                w.u32(*t_last);
            }
        }
        if w.0.len() + 1 > MAX_FRAME {
            return Err(Error::Limit("frame over MAX_FRAME"));
        }
        Ok(w.0)
    }

    /// The whole frame.
    pub fn frame(&self) -> Result<Vec<u8>, Error> {
        frame_bytes(self.ty(), &self.payload()?)
    }

    pub fn decode(f: &Frame) -> Result<ToHost, Error> {
        let mut r = R::new(&f.payload);
        let m = match f.ty {
            ty::CREATE => {
                let version = r.u16()?;
                let match_id = r.u32()?;
                let kind = r.u8()?;
                let project = r.str("Create project")?;
                let map = r.u32()?;
                let mode = r.u32()?;
                let tick_ms = r.u8()?;
                let n = r.count("Create records")?;
                let mut records = Vec::with_capacity(n);
                for _ in 0..n {
                    let bot = r.u8()? != 0;
                    records.push(Record { bot, team: r.u8()? });
                }
                let options = r.blob(MAX_FRAME, "Create options")?;
                let world = r.blob(MAX_FRAME, "Create world")?;
                let drive_bots = if version >= 5 { r.u8()? != 0 } else { false };
                let ai_ignore_players = if version >= 7 { r.u8()? != 0 } else { false };
                ToHost::Create(Create { version, match_id, kind, project, map, mode, tick_ms, records, options, world, drive_bots, ai_ignore_players })
            }
            ty::DF => {
                let t = r.u32()?;
                ToHost::Df(Df { t, bytes: r.blob(MAX_FRAME, "Df bytes")? })
            }
            ty::WANT_DE => ToHost::WantDe { request_id: r.u32()? },
            ty::CLOSE => ToHost::Close { t_last: r.u32()? },
            t => return Err(Error::Type(t)),
        };
        r.end()?;
        Ok(m)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// host -> relay

/// `Ready` (0x81): the match exists in the host. `u32 match_id, u32 world_time, u32 collision_checksum, u32 create_ms`,
/// then (version 4, only when the `Create` had an empty world) `u32 len + start item`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ready {
    pub match_id: u32,
    /// the time of the host's present: the start item's time when the host built it
    pub world_time: u32,
    /// `[[core+0x210]+0x380]`, the value the clients compare with DD's level checksum
    pub collision_checksum: u32,
    pub create_ms: u32,
    /// version 4: the start world the host built (the clients' first DE, decoded, full = 1); None = the relay gave the
    /// world (the frame then has the version-3 layout)
    pub start: Option<Vec<u8>>,
}

/// The per-player part of `Outcome` / `Stats` / `Final`: `u8 slot, u16 kills, u16 deaths, u16 score, u16 chests,
/// u16 keys_spent, u32 gold_spent`. kills / deaths = event rows 0 / 2 of the player's statistics block, score = block
/// `+0xA1C` (u16), chests = event row 42 `pve_open_chest`; keys_spent / gold_spent = the PvE paid respawns the world
/// charged this player: world entry `[W+0x17C2A0] + slot*0xB8` `+0x54` (keys) / `+0x58` (gold), the counters
/// 0x2003CF230 raises against the options budget `+0x2D0` / `+0x2D4` (version 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlayerStats {
    pub slot: u8,
    pub kills: u16,
    pub deaths: u16,
    pub score: u16,
    pub chests: u16,
    /// PvE access keys spent on paid respawns (entry `+0x54`)
    pub keys_spent: u16,
    /// PvE gold spent on paid respawns (entry `+0x58`)
    pub gold_spent: u32,
}

/// One player's detail row of the `Outcome` / `Final` trailer (version 3), from his statistics block
/// `B = S + 0x30 + slot*0xA24`:
///
/// ```text
/// u8 slot, u16 best_streak, u16 cur_streak, u8 inactive, u32 last_active_ms, u32 inactive_ext_ms, u16 events[74],
/// u32 badge_mask (bit t = slot t has a non-zero count; bits 27..31 = 0), per set bit ascending: u8 c1, c2, c3
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerDetail {
    pub slot: u8,
    /// best kill streak of the match `B+0xA1E`
    pub best_streak: u16,
    /// current kill streak `B+0xA08`
    pub cur_streak: u16,
    /// the world's inactive flag `B+0x9F0` (set only with options +0x107D = 1 or 3)
    pub inactive: bool,
    /// last activity stamp `B+0x9E8` (world time, ms)
    pub last_active_ms: u32,
    /// time not counted for inactivity `B+0x9EC` (search and destroy, ms)
    pub inactive_ext_ms: u32,
    /// per event e: the sum over the 9 profiles of `u16 [B + e*0x12 + p*2]`, saturating at 0xFFFF
    pub events: [u16; EVENTS],
    /// per badge slot t: how many times state 1 / 2 / 3 was earned (`B+0x534+12t+1..+3`); final only in `Final` (after
    /// the host's match-end call `S.vt[0x110]`); `Outcome` carries zeros
    pub badges: [[u8; 3]; BADGES],
    /// version 8, `Final` only: the host's per-step health sample of the player ([`Vitals`]); None = not sent
    pub vitals: Option<Vitals>,
}

/// What the host saw of a player's health over the match (version 8, `Final` trailer `u16 regen_best, u8 artefact_hp_min`):
/// sampled after every stepped DF from the "health" parameter (the value 0x200344E70 reads: current / max) and the
/// alive flag (+0x748) of his character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Vitals {
    /// the most health (percent of max, summed over the steps where it rose while he was alive) he got back within one
    /// life (achievement 402)
    pub regen_best: u16,
    /// his lowest health (percent, truncated as 0x200344E70) at a step where his `artefact` event row (5) rose; 0xFF =
    /// no such step (achievement 414)
    pub artefact_hp_min: u8,
}

impl Default for PlayerDetail {
    fn default() -> Self {
        PlayerDetail { slot: 0, best_streak: 0, cur_streak: 0, inactive: false, last_active_ms: 0, inactive_ext_ms: 0, events: [0; EVENTS], badges: [[0; 3]; BADGES], vitals: None }
    }
}

impl PlayerDetail {
    /// bit t = badge slot t has a non-zero count
    pub fn badge_mask(&self) -> u32 {
        self.badges.iter().enumerate().filter(|(_, c)| c.iter().any(|&x| x != 0)).fold(0, |m, (t, _)| m | 1 << t)
    }
}

/// One death as the statistics handler's vtable slot 3 (0x2001FB490) received it from the victim's death handler
/// 0x2003046B0; version 3, `Kills` frame. The argument bytes go on the wire raw (PROVEN meanings: t, killer, victim,
/// profiles, weapon dict, weapon category, head zone). "Argument n" below counts the handler's `this` as 0.
/// 29 bytes: `u32 t, u8 killer, u8 victim, u8 killer_profile, u8 victim_profile, u16 weapon_dict, u8 weapon_category,
/// u8 killer_hp, u32 damage_type, u32 item_uid, u16 victim_weapon_dict, u8 arg80, u16 flags, u32 damagers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KillRecord {
    /// world time of the death (argument 1)
    pub t: u32,
    /// killer slot, 0xFF = none (argument 2)
    pub killer: u8,
    /// victim slot (argument 11)
    pub victim: u8,
    pub killer_profile: u8,
    pub victim_profile: u8,
    /// dict id of the killing item, 0 = none (argument 3)
    pub weapon_dict: u16,
    /// `category_id` of the killing item (dictionary entry +0x307; argument 5)
    pub weapon_category: u8,
    /// argument 9: the killer's health in percent (0x200344E70: the "health" parameter, current / max x 100,
    /// truncated; 100 without a killer)
    pub killer_hp: u8,
    /// argument 10: the hit's inflictor handle `u16 id << 8 | u8 +0xA` (-1 without one, e.g. a bleeding death). For a
    /// bullet: its shot id (`killer << 11 | per-player counter`, 0x2003BDAE0) and pellet index
    pub damage_type: u32,
    /// argument 4 (LIKELY the killing item's uid, 0xFFFFFFFF none)
    pub item_uid: u32,
    /// argument 27: the victim's active item dict (`+0x4F8 -> +0x304`)
    pub victim_weapon_dict: u16,
    /// bit 0: argument 16 != 0, the melee byte (hit +0xAC: a bullet of the weapon's own melee ammunition);
    /// bit 1: the death handler's ricochet byte (hit +0xAD && +0xAE: the kill feed's `ricochet_icon`);
    /// bit 2: its penetration byte (hit +0xAD && !+0xAE: `penetration_icon`). Hit +0xAD / +0xAE = the bullet's
    /// path-segment / ricochet counts (+0x68 / +0x69) when it hit (0x20039A17F); a host without bits 1 / 2 sends 0 / 1
    pub arg80: u8,
    /// bit = that byte argument is non-zero ([`kill_flag`])
    pub flags: u16,
    /// bit j = the victim's damage row from player j (`B(victim)+0x678`) is non-zero at the call
    pub damagers: u32,
    /// version 8: bit j = player j gets the assist for this death by the core's rule (0x20033E660, computed by the host at
    /// the call from the same rows): a teammate of the killer whose damage with one profile is >= 10 % of the killer
    /// team's damage to the victim; None = not sent (an older host)
    pub assists: Option<u32>,
}

/// Bits of [`KillRecord::flags`]: the byte arguments of slot 3 ("argument n" counts the handler's `this` as 0).
pub mod kill_flag {
    /// argument 15: hit zone "brain" / "face" (headshot, PROVEN)
    pub const HEAD_ZONE: u16 = 1 << 0;
    /// argument 17: an inflictor object was present
    pub const INFLICTOR: u16 = 1 << 1;
    /// argument 18: hit zone "health": a bleeding death (the core's `bleeding_kill`, 0x20033E1B8)
    pub const HEALTH_ZONE: u16 = 1 << 2;
    /// argument 19: inflictor +0x69 > 0: for a bullet its ricochet count (raised by the ricochet 0x20039A9C1) when the
    /// death is handled, which counts ricochets after the hit too; the kill's own ricochet is [`KillRecord::arg80`] bit 1
    pub const INFLICTOR_69: u16 = 1 << 3;
    /// argument 20
    pub const ARG_A0: u16 = 1 << 4;
    /// argument 21
    pub const ARG_A8: u16 = 1 << 5;
    /// argument 22 (0x200304F30: positions / angle)
    pub const ARG_B0: u16 = 1 << 6;
    /// argument 23: killer +0x748 (alive)
    pub const KILLER_ALIVE: u16 = 1 << 7;
    /// argument 24: the victim's weapon +0x44C4: aimed in (set / cleared by 0x2006D60C0 / 0x2006D5F70)
    pub const VICTIM_C0: u16 = 1 << 8;
    /// argument 25: the victim's weapon is reloading (its state id is 4, 0x2006D6A20)
    pub const VICTIM_C8: u16 = 1 << 9;
    /// argument 26: the victim's use of the object at +0x290 is in progress (its vt[0x30]); which object:
    /// [`VICTIM_ANOMALY`]
    pub const VICTIM_D0: u16 = 1 << 10;
    /// argument 7: killer +0x6D8
    pub const KILLER_6D8: u16 = 1 << 11;
    /// argument 13: victim +0x6D8
    pub const VICTIM_6D8: u16 = 1 << 12;
    /// worked out by the host at the same moment (not a slot 3 argument): a firearm kill whose weapon was not aimed in,
    /// the kill feed's `hipfire_icon` rule (0x200304AE3: aim-in progress 0x2006CC8A0 of the weapon below 0.5)
    pub const HIPFIRE: u16 = 1 << 13;
    /// worked out by the host: a firearm kill by the killer's latest shot with the weapon's magazine (+0x44BA) and
    /// chamber (+0x44CD) empty, i.e. the shot that fired the last round (the exe's own last-round flag, 0x2002FFF48)
    pub const LAST_ROUND: u16 = 1 << 14;
    /// worked out by the host: the object the victim was using (`victim+0x290`, whose vt[0x30] gives argument 26) is
    /// an anomaly (RTTI class name contains `anomaly`)
    pub const VICTIM_ANOMALY: u16 = 1 << 15;
}

/// `Kills` (0x89, version 3): `u32 t_upto, u16 n, n x KillRecord`. Every kill with `t <= t_upto` not sent before is in
/// this frame or an earlier one; sent before `Final`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kills {
    pub t_upto: u32,
    pub records: Vec<KillRecord>,
}

/// One ammo / pocket stack's use (`Used`, version 6): `u32 uid, u16 dict, u32 used`. `used` = the options storage at
/// the start - the storage at the end - the units the player still carries (magazines included) at the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UsedStack {
    pub uid: u32,
    pub dict: u16,
    pub used: u32,
}

/// One human's row of `Used` (version 6): `u8 slot, u16 profiles (bit p = he spawned with options profile p), u8 nw,
/// nw x u16 weapon dict (slots 7 / 10 of those profiles, each dict once), u8 ns, ns x UsedStack (only used > 0)`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlayerUse {
    pub slot: u8,
    pub profiles: u16,
    pub weapons: Vec<u16>,
    pub stacks: Vec<UsedStack>,
}

/// `Used` (0x8A, version 6): `u32 t, u8 n, n x PlayerUse`, read at `Close.t_last`, sent before `Final`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Used {
    pub t: u32,
    pub players: Vec<PlayerUse>,
}

/// Most weapons in one [`PlayerUse`] row (9 profiles x 2 weapon slots).
pub const MAX_USE_WEAPONS: usize = 18;

/// `Outcome` (0x83, latched once at the first final tick with state 4 / 5) and `Final` (0x86, at `Close.t_last`):
/// `u32 t_end, u8 state, u8 winner_team, u8 pve_success, u32 mission_ms, u8 n, n x PlayerStats, u32 team_score[2]`,
/// then (version 3) `u8 nd, nd x PlayerDetail`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// the DF time where the state changed (Final: `Close.t_last`)
    pub t_end: u32,
    /// `W+0x180AE0`: 4 / 5 = ended; `Final` may carry 3 when the relay ended the match
    pub state: u8,
    pub winner_team: u8,
    /// PvE: winner team 0 (the extraction `exit`, the only success)
    pub pve_success: bool,
    /// `t_end` - the enter DF time
    pub mission_ms: u32,
    pub players: Vec<PlayerStats>,
    pub team_score: [u32; 2],
    /// version 3: one row per record slot (`Outcome`: badges zero; `Final`: after the match-end call)
    pub details: Vec<PlayerDetail>,
}

/// `Stats` (0x84): every 5 s of world time; doubles as the heartbeat. `u32 t, u8 n, n x PlayerStats,
/// u32 team_score[2]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stats {
    pub t: u32,
    pub players: Vec<PlayerStats>,
    pub team_score: [u32; 2],
}

/// `Tick` (0x82, phase 2): `u32 t, u8 mask[3], u8 hash per set mask bit, u8 g0, g1, g2` (the E0 body with the full time).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tick {
    pub t: u32,
    pub mask: [u8; 3],
    /// one per set bit of `mask`, in bit order
    pub hashes: Vec<u8>,
    pub g: [u8; 3],
}

/// `De` (0x85, phase 2): `u32 request_id, u32 time, u32 len, item`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct De {
    pub request_id: u32,
    pub time: u32,
    pub item: Vec<u8>,
}

/// `Diverged` (0x87): `u32 t, u32 retimed, u32 merged`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Diverged {
    pub t: u32,
    pub retimed: u32,
    pub merged: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FromHost {
    Ready(Ready),
    Tick(Tick),
    Outcome(Outcome),
    Stats(Stats),
    De(De),
    Final(Outcome),
    Diverged(Diverged),
    Kills(Kills),
    /// version 6: what each human used (sent before `Final`)
    Used(Used),
    /// version 5: the bot entries of one stepped DF, for DF `t_for` ([`bots::Bots`])
    Bots(bots::Bots),
    /// `Fault` (0x8F): `str reason (<= 256)`, then the host exits
    Fault { reason: String },
}

fn put_players(w: &mut W, p: &[PlayerStats], what: &'static str) -> Result<(), Error> {
    check_players(p.len(), what)?;
    w.u8(p.len() as u8);
    for s in p {
        w.u8(s.slot).u16(s.kills).u16(s.deaths).u16(s.score).u16(s.chests).u16(s.keys_spent).u32(s.gold_spent);
    }
    Ok(())
}

fn get_players(r: &mut R, what: &'static str) -> Result<Vec<PlayerStats>, Error> {
    let n = r.count(what)?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(PlayerStats {
            slot: r.u8()?,
            kills: r.u16()?,
            deaths: r.u16()?,
            score: r.u16()?,
            chests: r.u16()?,
            keys_spent: r.u16()?,
            gold_spent: r.u32()?,
        });
    }
    Ok(v)
}

/// Slots of a detail list: each < 20, each once.
fn check_detail_slots(d: &[PlayerDetail]) -> Result<(), Error> {
    let mut seen = 0u32;
    for x in d {
        if usize::from(x.slot) >= MAX_PLAYERS || seen & (1 << x.slot) != 0 {
            return Err(Error::Limit("Outcome detail slot"));
        }
        seen |= 1 << x.slot;
    }
    Ok(())
}

fn put_details(w: &mut W, d: &[PlayerDetail]) -> Result<(), Error> {
    check_players(d.len(), "Outcome details")?;
    check_detail_slots(d)?;
    w.u8(d.len() as u8);
    for x in d {
        w.u8(x.slot).u16(x.best_streak).u16(x.cur_streak).u8(u8::from(x.inactive)).u32(x.last_active_ms).u32(x.inactive_ext_ms);
        for e in x.events {
            w.u16(e);
        }
        let mask = x.badge_mask();
        w.u32(mask);
        for (t, c) in x.badges.iter().enumerate() {
            if mask & (1 << t) != 0 {
                w.bytes(c);
            }
        }
    }
    Ok(())
}

fn get_details(r: &mut R) -> Result<Vec<PlayerDetail>, Error> {
    let n = r.count("Outcome details")?;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let mut x = PlayerDetail { slot: r.u8()?, ..Default::default() };
        x.best_streak = r.u16()?;
        x.cur_streak = r.u16()?;
        x.inactive = r.u8()? != 0;
        x.last_active_ms = r.u32()?;
        x.inactive_ext_ms = r.u32()?;
        for e in x.events.iter_mut() {
            *e = r.u16()?;
        }
        let mask = r.u32()?;
        if mask >> BADGES != 0 {
            return Err(Error::Limit("Outcome badge mask"));
        }
        for (t, b) in x.badges.iter_mut().enumerate() {
            if mask & (1 << t) != 0 {
                let c = r.take(3)?;
                *b = [c[0], c[1], c[2]];
                if *b == [0; 3] {
                    return Err(Error::Limit("Outcome badge bit without a count"));
                }
            }
        }
        v.push(x);
    }
    check_detail_slots(&v)?;
    Ok(v)
}

fn put_outcome(w: &mut W, o: &Outcome) -> Result<(), Error> {
    w.u32(o.t_end).u8(o.state).u8(o.winner_team).u8(u8::from(o.pve_success)).u32(o.mission_ms);
    put_players(w, &o.players, "Outcome players")?;
    w.u32(o.team_score[0]).u32(o.team_score[1]);
    put_details(w, &o.details)?;
    // version 8: the vitals trailer, only when every row has them
    if !o.details.is_empty() && o.details.iter().all(|d| d.vitals.is_some()) {
        for d in &o.details {
            let v = d.vitals.unwrap_or_default();
            w.u16(v.regen_best).u8(v.artefact_hp_min);
        }
    }
    Ok(())
}

fn put_kill(w: &mut W, k: &KillRecord) {
    w.u32(k.t).u8(k.killer).u8(k.victim).u8(k.killer_profile).u8(k.victim_profile).u16(k.weapon_dict).u8(k.weapon_category).u8(k.killer_hp);
    w.u32(k.damage_type).u32(k.item_uid).u16(k.victim_weapon_dict).u8(k.arg80).u16(k.flags).u32(k.damagers);
}

fn get_kill(r: &mut R) -> Result<KillRecord, Error> {
    Ok(KillRecord {
        t: r.u32()?,
        killer: r.u8()?,
        victim: r.u8()?,
        killer_profile: r.u8()?,
        victim_profile: r.u8()?,
        weapon_dict: r.u16()?,
        weapon_category: r.u8()?,
        killer_hp: r.u8()?,
        damage_type: r.u32()?,
        item_uid: r.u32()?,
        victim_weapon_dict: r.u16()?,
        arg80: r.u8()?,
        flags: r.u16()?,
        damagers: r.u32()?,
        assists: None,
    })
}

fn get_outcome(r: &mut R) -> Result<Outcome, Error> {
    let t_end = r.u32()?;
    let state = r.u8()?;
    let winner_team = r.u8()?;
    let pve_success = r.u8()? != 0;
    let mission_ms = r.u32()?;
    let players = get_players(r, "Outcome players")?;
    let team_score = [r.u32()?, r.u32()?];
    let mut details = get_details(r)?;
    if r.p < r.b.len() && !details.is_empty() {
        for d in details.iter_mut() {
            d.vitals = Some(Vitals { regen_best: r.u16()?, artefact_hp_min: r.u8()? });
        }
    }
    Ok(Outcome { t_end, state, winner_team, pve_success, mission_ms, players, team_score, details })
}

impl FromHost {
    pub fn ty(&self) -> u8 {
        match self {
            FromHost::Ready(_) => ty::READY,
            FromHost::Tick(_) => ty::TICK,
            FromHost::Outcome(_) => ty::OUTCOME,
            FromHost::Stats(_) => ty::STATS,
            FromHost::De(_) => ty::DE,
            FromHost::Final(_) => ty::FINAL,
            FromHost::Diverged(_) => ty::DIVERGED,
            FromHost::Kills(_) => ty::KILLS,
            FromHost::Used(_) => ty::USED,
            FromHost::Bots(_) => ty::BOTS,
            FromHost::Fault { .. } => ty::FAULT,
        }
    }

    pub fn payload(&self) -> Result<Vec<u8>, Error> {
        let mut w = W::default();
        match self {
            FromHost::Ready(x) => {
                w.u32(x.match_id).u32(x.world_time).u32(x.collision_checksum).u32(x.create_ms);
                if let Some(item) = &x.start {
                    w.blob(item);
                }
            }
            FromHost::Tick(x) => {
                let bits: u32 = x.mask.iter().map(|b| b.count_ones()).sum();
                if bits as usize != x.hashes.len() {
                    return Err(Error::Limit("Tick hashes != set mask bits"));
                }
                if mask_over_players(x.mask) {
                    return Err(Error::Limit("Tick mask beyond 20 players"));
                }
                w.u32(x.t).bytes(&x.mask).bytes(&x.hashes).bytes(&x.g);
            }
            FromHost::Outcome(o) | FromHost::Final(o) => put_outcome(&mut w, o)?,
            FromHost::Stats(s) => {
                w.u32(s.t);
                put_players(&mut w, &s.players, "Stats players")?;
                w.u32(s.team_score[0]).u32(s.team_score[1]);
            }
            FromHost::De(d) => {
                w.u32(d.request_id).u32(d.time).blob(&d.item);
            }
            FromHost::Diverged(d) => {
                w.u32(d.t).u32(d.retimed).u32(d.merged);
            }
            FromHost::Kills(k) => {
                if k.records.len() > MAX_KILLS {
                    return Err(Error::Limit("Kills records"));
                }
                w.u32(k.t_upto).u16(k.records.len() as u16);
                for x in &k.records {
                    put_kill(&mut w, x);
                }
                // version 8: the assists trailer, only when every record has them
                if !k.records.is_empty() && k.records.iter().all(|x| x.assists.is_some()) {
                    for x in &k.records {
                        w.u32(x.assists.unwrap_or(0));
                    }
                }
            }
            FromHost::Used(u) => {
                check_players(u.players.len(), "Used players")?;
                w.u32(u.t).u8(u.players.len() as u8);
                for p in &u.players {
                    if p.weapons.len() > MAX_USE_WEAPONS {
                        return Err(Error::Limit("Used weapons"));
                    }
                    if p.stacks.len() > usize::from(u8::MAX) {
                        return Err(Error::Limit("Used stacks"));
                    }
                    w.u8(p.slot).u16(p.profiles).u8(p.weapons.len() as u8);
                    for d in &p.weapons {
                        w.u16(*d);
                    }
                    w.u8(p.stacks.len() as u8);
                    for x in &p.stacks {
                        w.u32(x.uid).u16(x.dict).u32(x.used);
                    }
                }
            }
            FromHost::Bots(b) => b.put(&mut w)?,
            FromHost::Fault { reason } => {
                check_text(reason, "Fault reason")?;
                w.str(reason);
            }
        }
        if w.0.len() + 1 > MAX_FRAME {
            return Err(Error::Limit("frame over MAX_FRAME"));
        }
        Ok(w.0)
    }

    /// The whole frame.
    pub fn frame(&self) -> Result<Vec<u8>, Error> {
        frame_bytes(self.ty(), &self.payload()?)
    }

    pub fn decode(f: &Frame) -> Result<FromHost, Error> {
        let mut r = R::new(&f.payload);
        let m = match f.ty {
            ty::READY => {
                let (match_id, world_time, collision_checksum, create_ms) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?);
                let start = if r.p < r.b.len() { Some(r.blob(MAX_FRAME, "Ready start item")?) } else { None };
                FromHost::Ready(Ready { match_id, world_time, collision_checksum, create_ms, start })
            }
            ty::TICK => {
                let t = r.u32()?;
                let m = r.take(3)?;
                let mask = [m[0], m[1], m[2]];
                if mask_over_players(mask) {
                    return Err(Error::Limit("Tick mask beyond 20 players"));
                }
                let n: u32 = mask.iter().map(|b| b.count_ones()).sum();
                let hashes = r.take(n as usize)?.to_vec();
                let g = r.take(3)?;
                FromHost::Tick(Tick { t, mask, hashes, g: [g[0], g[1], g[2]] })
            }
            ty::OUTCOME => FromHost::Outcome(get_outcome(&mut r)?),
            ty::FINAL => FromHost::Final(get_outcome(&mut r)?),
            ty::STATS => {
                let t = r.u32()?;
                let players = get_players(&mut r, "Stats players")?;
                let team_score = [r.u32()?, r.u32()?];
                FromHost::Stats(Stats { t, players, team_score })
            }
            ty::DE => {
                let request_id = r.u32()?;
                let time = r.u32()?;
                FromHost::De(De { request_id, time, item: r.blob(MAX_FRAME, "De item")? })
            }
            ty::DIVERGED => FromHost::Diverged(Diverged { t: r.u32()?, retimed: r.u32()?, merged: r.u32()? }),
            ty::KILLS => {
                let t_upto = r.u32()?;
                let n = usize::from(r.u16()?);
                if n > MAX_KILLS {
                    return Err(Error::Limit("Kills records"));
                }
                let mut records = Vec::with_capacity(n);
                for _ in 0..n {
                    records.push(get_kill(&mut r)?);
                }
                if r.p < r.b.len() && n > 0 {
                    for x in records.iter_mut() {
                        x.assists = Some(r.u32()?);
                    }
                }
                FromHost::Kills(Kills { t_upto, records })
            }
            ty::USED => {
                let t = r.u32()?;
                let n = r.count("Used players")?;
                let mut players = Vec::with_capacity(n);
                for _ in 0..n {
                    let slot = r.u8()?;
                    let profiles = r.u16()?;
                    let nw = usize::from(r.u8()?);
                    if nw > MAX_USE_WEAPONS {
                        return Err(Error::Limit("Used weapons"));
                    }
                    let mut weapons = Vec::with_capacity(nw);
                    for _ in 0..nw {
                        weapons.push(r.u16()?);
                    }
                    let ns = usize::from(r.u8()?);
                    let mut stacks = Vec::with_capacity(ns);
                    for _ in 0..ns {
                        stacks.push(UsedStack { uid: r.u32()?, dict: r.u16()?, used: r.u32()? });
                    }
                    players.push(PlayerUse { slot, profiles, weapons, stacks });
                }
                FromHost::Used(Used { t, players })
            }
            ty::BOTS => FromHost::Bots(bots::Bots::get(&mut r)?),
            ty::FAULT => FromHost::Fault { reason: r.str("Fault reason")? },
            t => return Err(Error::Type(t)),
        };
        r.end()?;
        Ok(m)
    }
}

/// Most project names in one `Projects` frame.
pub const MAX_PROJECTS: usize = 4096;

/// The `Projects` payload (0x8B): `u16 n`, n x `str`.
pub fn projects_payload(names: &[String]) -> Result<Vec<u8>, Error> {
    if names.len() > MAX_PROJECTS {
        return Err(Error::Limit("Projects count"));
    }
    let mut w = W::default();
    w.u16(names.len() as u16);
    for n in names {
        check_text(n, "Projects name")?;
        w.str(n);
    }
    Ok(w.0)
}

/// The project names of a `Projects` frame (0x8B); any other type is [`Error::Type`].
pub fn decode_projects(f: &Frame) -> Result<Vec<String>, Error> {
    if f.ty != ty::PROJECTS {
        return Err(Error::Type(f.ty));
    }
    let mut r = R::new(&f.payload);
    let n = usize::from(r.u16()?);
    if n > MAX_PROJECTS {
        return Err(Error::Limit("Projects count"));
    }
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(r.str("Projects name")?);
    }
    r.end()?;
    Ok(v)
}

/// A mask bit at player index 20 or above (byte 0 = players 0..7, LIKELY order of E0, match_msgs.rs).
fn mask_over_players(mask: [u8; 3]) -> bool {
    let v = u32::from(mask[0]) | u32::from(mask[1]) << 8 | u32::from(mask[2]) << 16;
    v >> MAX_PLAYERS != 0
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_project_list_round_trips_and_is_bounded() {
        use super::{decode_projects, projects_payload, ty, Error, Frame};
        let names = vec!["level_01_day_tdm".to_string(), "pve_01_v1_maze2".to_string()];
        let f = Frame { ty: ty::PROJECTS, payload: projects_payload(&names).unwrap() };
        assert_eq!(decode_projects(&f).unwrap(), names);
        assert_eq!(decode_projects(&Frame { ty: ty::PROJECTS, payload: vec![] }), Err(Error::Short));
        assert_eq!(decode_projects(&Frame { ty: ty::PROJECTS, payload: vec![0, 0, 9] }), Err(Error::Trailing(1)));
        assert_eq!(decode_projects(&Frame { ty: ty::FAULT, payload: vec![0, 0] }), Err(Error::Type(ty::FAULT)));
    }

    use super::*;

    fn stats(n: u8) -> Vec<PlayerStats> {
        (0..n).map(|i| PlayerStats { slot: i, kills: 40 + u16::from(i), deaths: 1, score: 590, chests: 2, keys_spent: u16::from(i), gold_spent: 50 * u32::from(i) }).collect()
    }

    fn to_host_samples() -> Vec<ToHost> {
        vec![
            ToHost::Create(Create {
                version: PROTO_VERSION,
                match_id: 0x6AFC_2E2C,
                kind: KIND_PVE,
                project: "pve_01_v1_maze2".into(),
                map: 15,
                mode: 7,
                tick_ms: 50,
                records: (0..18).map(|i| Record { bot: i > 0, team: u8::from(i > 0) }).collect(),
                options: (0..18_000u32).map(|i| i as u8).collect(),
                world: vec![0xAB; 36_200],
                drive_bots: true,
                ai_ignore_players: true,
            }),
            ToHost::Df(Df { t: 523_950, bytes: vec![1, 2, 3, 4] }),
            ToHost::Df(Df { t: 0, bytes: vec![] }),
            ToHost::WantDe { request_id: 7 },
            ToHost::Close { t_last: 586_700 },
        ]
    }

    fn details(n: u8) -> Vec<PlayerDetail> {
        (0..n)
            .map(|i| {
                let mut d = PlayerDetail { slot: i, best_streak: 3 + u16::from(i), cur_streak: 1, inactive: i == 2, last_active_ms: 90_000, inactive_ext_ms: 0, ..Default::default() };
                d.events[0] = 40;
                d.events[15] = 7;
                d.events[73] = 0xFFFF;
                d.badges[5] = [0, 1, 0];
                d.badges[23] = [1, 0, 0];
                d.badges[26] = [u8::from(i), 0, 0];
                d
            })
            .collect()
    }

    fn kill(t: u32) -> KillRecord {
        KillRecord {
            t,
            killer: 1,
            victim: 4,
            killer_profile: 0,
            victim_profile: 2,
            weapon_dict: 0x0304,
            weapon_category: 13,
            killer_hp: 100,
            damage_type: 0x18,
            item_uid: 0xFFFF_FFFF,
            victim_weapon_dict: 77,
            arg80: 0,
            flags: kill_flag::HEAD_ZONE | kill_flag::KILLER_ALIVE,
            damagers: 0b10010,
            assists: None,
        }
    }

    fn from_host_samples() -> Vec<FromHost> {
        let o = Outcome { t_end: 523_950, state: 5, winner_team: 1, pve_success: false, mission_ms: 500_000, players: stats(18), team_score: [3, 9], details: details(18) };
        vec![
            FromHost::Ready(Ready { match_id: 1, world_time: 2, collision_checksum: 3, create_ms: 4, start: None }),
            FromHost::Ready(Ready { match_id: 1, world_time: 850, collision_checksum: 3, create_ms: 4, start: Some(vec![7; 9357]) }),
            FromHost::Ready(Ready { match_id: 1, world_time: 800, collision_checksum: 3, create_ms: 4, start: Some(vec![]) }),
            FromHost::Tick(Tick { t: 1000, mask: [0b101, 0, 0b1000], hashes: vec![9, 8, 7], g: [1, 2, 3] }),
            FromHost::Tick(Tick { t: 1050, mask: [0; 3], hashes: vec![], g: [4, 5, 6] }),
            FromHost::Outcome(o.clone()),
            FromHost::Final(Outcome { state: 3, players: vec![], details: vec![], ..o.clone() }),
            FromHost::Final(Outcome { details: details(20), ..o }),
            FromHost::Kills(Kills { t_upto: 9000, records: (0..300).map(|i| kill(50 * i)).collect() }),
            FromHost::Kills(Kills { t_upto: 0, records: vec![] }),
            FromHost::Used(Used {
                t: 856_000,
                players: vec![
                    PlayerUse { slot: 0, profiles: 0b11, weapons: vec![55, 107, 1178], stacks: vec![UsedStack { uid: 100_028, dict: 760, used: 4 }, UsedStack { uid: 100_014, dict: 81, used: 412 }] },
                    PlayerUse { slot: 3, profiles: 0, weapons: vec![], stacks: vec![] },
                ],
            }),
            FromHost::Used(Used { t: 0, players: vec![] }),
            FromHost::Stats(Stats { t: 5000, players: stats(2), team_score: [0, 0] }),
            FromHost::De(De { request_id: 7, time: 9000, item: vec![5; 5800] }),
            FromHost::Diverged(Diverged { t: 17_000, retimed: 1, merged: 0 }),
            FromHost::Fault { reason: "create refused: max_matches".into() },
        ]
    }

    #[test]
    fn every_relay_message_round_trips() {
        for m in to_host_samples() {
            let bytes = m.frame().unwrap();
            let f = read_frame(&mut &bytes[..]).unwrap().unwrap();
            assert_eq!(f.ty, m.ty());
            assert_eq!(ToHost::decode(&f).unwrap(), m);
        }
    }

    #[test]
    fn every_host_message_round_trips() {
        for m in from_host_samples() {
            let bytes = m.frame().unwrap();
            let f = read_frame(&mut &bytes[..]).unwrap().unwrap();
            assert_eq!(f.ty, m.ty());
            assert_eq!(FromHost::decode(&f).unwrap(), m);
        }
    }

    /// Version 4 keeps the version-3 `Ready` layout when the relay gave the world: 16 bytes, no start item.
    #[test]
    fn ready_without_a_start_item_is_the_version_3_layout() {
        let r = FromHost::Ready(Ready { match_id: 0x10002774, world_time: 800, collision_checksum: 0x255f73e4, create_ms: 9, start: None });
        let p = r.payload().unwrap();
        assert_eq!(p.len(), 16);
        assert_eq!(&p[..8], &[0x74, 0x27, 0x00, 0x10, 0x20, 0x03, 0, 0]);
        let with = FromHost::Ready(Ready { match_id: 1, world_time: 850, collision_checksum: 2, create_ms: 3, start: Some(vec![1, 2, 3]) });
        let p = with.payload().unwrap();
        assert_eq!(p.len(), 16 + 4 + 3);
        assert_eq!(&p[16..], &[3, 0, 0, 0, 1, 2, 3]);
        // a cut start item is an error, not a version-3 Ready
        let f = Frame { ty: ty::READY, payload: p[..21].to_vec() };
        assert!(FromHost::decode(&f).is_err());
    }

    #[test]
    fn frame_layout_is_len_type_payload() {
        let b = ToHost::Close { t_last: 0x0102_0304 }.frame().unwrap();
        assert_eq!(b, [5, 0, 0, 0, ty::CLOSE, 4, 3, 2, 1]);
        let b = ToHost::Df(Df { t: 50, bytes: vec![0xEE] }).frame().unwrap();
        assert_eq!(b, [10, 0, 0, 0, ty::DF, 50, 0, 0, 0, 1, 0, 0, 0, 0xEE]);
        let b = FromHost::Fault { reason: "x".into() }.frame().unwrap();
        assert_eq!(b, [4, 0, 0, 0, ty::FAULT, 1, 0, b'x']);
        // version 2 player row: slot, kills, deaths, score, chests, keys_spent (u16), gold_spent (u32)
        let p = PlayerStats { slot: 2, kills: 3, deaths: 4, score: 5, chests: 6, keys_spent: 7, gold_spent: 0x0102_0304 };
        let b = FromHost::Stats(Stats { t: 1, players: vec![p], team_score: [8, 9] }).frame().unwrap();
        assert_eq!(b, [29, 0, 0, 0, ty::STATS, 1, 0, 0, 0, 1, 2, 3, 0, 4, 0, 5, 0, 6, 0, 7, 0, 4, 3, 2, 1, 8, 0, 0, 0, 9, 0, 0, 0]);
    }

    #[test]
    fn version_3_detail_row_and_kill_record_layout() {
        // one Outcome with no players and one detail row: slot 1, kill row 2 (event 0) and headshot 1 (event 15), badge
        // slot 5 state 2 once and slot 23 (best player) once
        let mut d = PlayerDetail { slot: 1, best_streak: 0x0102, cur_streak: 3, inactive: true, last_active_ms: 0x0A0B_0C0D, inactive_ext_ms: 5, ..Default::default() };
        d.events[0] = 2;
        d.events[15] = 1;
        d.badges[5] = [0, 1, 0];
        d.badges[23] = [1, 0, 0];
        let o = Outcome { t_end: 0, state: 4, winner_team: 0, pve_success: false, mission_ms: 0, players: vec![], team_score: [0; 2], details: vec![d] };
        let p = FromHost::Final(o).payload().unwrap();
        // head: t_end 4, state 1, winner 1, pve 1, mission 4, n 1, team scores 8 = 20 bytes; then nd = 1
        assert_eq!(p[20], 1);
        let row = &p[21..];
        assert_eq!(&row[..14], &[1, 2, 1, 3, 0, 1, 0x0D, 0x0C, 0x0B, 0x0A, 5, 0, 0, 0]);
        let ev = &row[14..14 + 2 * EVENTS];
        assert_eq!(&ev[..2], &[2, 0]);
        assert_eq!(&ev[30..32], &[1, 0]);
        assert!(ev[2..30].iter().chain(&ev[32..]).all(|&b| b == 0));
        assert_eq!(&row[14 + 2 * EVENTS..], &[0x20, 0, 0x80, 0, 0, 1, 0, 1, 0, 0]);
        assert_eq!(row.len(), 166 + 6);
        // one kill record = 29 bytes after u32 t_upto, u16 n
        let k = KillRecord {
            t: 0x0102_0304,
            killer: 5,
            victim: 6,
            killer_profile: 7,
            victim_profile: 8,
            weapon_dict: 0x090A,
            weapon_category: 11,
            killer_hp: 12,
            damage_type: 0x0D0E_0F10,
            item_uid: 0x1112_1314,
            victim_weapon_dict: 0x1516,
            arg80: 0x17,
            flags: 0x1819,
            damagers: 0x1A1B_1C1D,
            assists: None,
        };
        let p = FromHost::Kills(Kills { t_upto: 9, records: vec![k] }).payload().unwrap();
        assert_eq!(
            p,
            [9, 0, 0, 0, 1, 0, 4, 3, 2, 1, 5, 6, 7, 8, 0x0A, 9, 11, 12, 0x10, 0x0F, 0x0E, 0x0D, 0x14, 0x13, 0x12, 0x11, 0x16, 0x15, 0x17, 0x19, 0x18, 0x1D, 0x1C, 0x1B, 0x1A]
        );
    }

    /// Version 6 `Used`: `u32 t, u8 n`, per row `u8 slot, u16 profiles, u8 nw, u16 x nw, u8 ns, {u32, u16, u32} x ns`.
    /// Version 8: the assists and vitals trailers ride at the end of `Kills` / `Final` only when every record / row has
    /// them; without them the frames are the version-6 bytes and decode to None.
    #[test]
    fn version_8_trailers_are_optional() {
        let k = kill(5);
        let old = FromHost::Kills(Kills { t_upto: 9, records: vec![k] }).payload().unwrap();
        let k7 = KillRecord { assists: Some(0b1010), ..k };
        let new = FromHost::Kills(Kills { t_upto: 9, records: vec![k7, KillRecord { assists: Some(0), ..k }] }).payload().unwrap();
        assert_eq!(new.len(), 6 + 2 * 29 + 2 * 4);
        assert_eq!(FromHost::decode(&Frame { ty: ty::KILLS, payload: old }).unwrap(), FromHost::Kills(Kills { t_upto: 9, records: vec![k] }), "no trailer: None");
        let FromHost::Kills(d) = FromHost::decode(&Frame { ty: ty::KILLS, payload: new.clone() }).unwrap() else { panic!() };
        assert_eq!(d.records.iter().map(|r| r.assists).collect::<Vec<_>>(), vec![Some(0b1010), Some(0)]);
        // a mixed log is sent without the trailer; a cut trailer is an error
        let mixed = FromHost::Kills(Kills { t_upto: 9, records: vec![k7, k] }).payload().unwrap();
        assert_eq!(mixed.len(), 6 + 2 * 29);
        assert_eq!(FromHost::decode(&Frame { ty: ty::KILLS, payload: new[..new.len() - 1].to_vec() }), Err(Error::Short));
        let mut o = Outcome { t_end: 1, state: 5, winner_team: 0, pve_success: false, mission_ms: 1, players: stats(2), team_score: [0, 0], details: details(2) };
        let plain = FromHost::Final(o.clone()).payload().unwrap();
        for (i, d) in o.details.iter_mut().enumerate() {
            d.vitals = Some(Vitals { regen_best: 120 + i as u16, artefact_hp_min: 0xFF });
        }
        let with = FromHost::Final(o.clone()).payload().unwrap();
        assert_eq!(with.len(), plain.len() + 2 * 3);
        assert_eq!(FromHost::decode(&Frame { ty: ty::FINAL, payload: with }).unwrap(), FromHost::Final(o.clone()));
        let FromHost::Final(p) = FromHost::decode(&Frame { ty: ty::FINAL, payload: plain }).unwrap() else { panic!() };
        assert!(p.details.iter().all(|d| d.vitals.is_none()));
    }

    #[test]
    fn version_6_used_layout_and_bounds() {
        let u = Used { t: 0x0102_0304, players: vec![PlayerUse { slot: 2, profiles: 0x0105, weapons: vec![0x0A0B], stacks: vec![UsedStack { uid: 0x1112_1314, dict: 0x2122, used: 0x3132_3334 }] }] };
        let p = FromHost::Used(u.clone()).payload().unwrap();
        assert_eq!(p, [4, 3, 2, 1, 1, 2, 5, 1, 1, 0x0B, 0x0A, 1, 0x14, 0x13, 0x12, 0x11, 0x22, 0x21, 0x34, 0x33, 0x32, 0x31]);
        assert_eq!(FromHost::decode(&Frame { ty: ty::USED, payload: p.clone() }).unwrap(), FromHost::Used(u.clone()));
        // a cut row, trailing bytes, too many weapons / players
        assert_eq!(FromHost::decode(&Frame { ty: ty::USED, payload: p[..p.len() - 1].to_vec() }), Err(Error::Short));
        let mut more = p.clone();
        more.push(0);
        assert_eq!(FromHost::decode(&Frame { ty: ty::USED, payload: more }), Err(Error::Trailing(1)));
        let mut w = u.clone();
        w.players[0].weapons = vec![1; MAX_USE_WEAPONS + 1];
        assert_eq!(FromHost::Used(w).payload(), Err(Error::Limit("Used weapons")));
        let mut bad = p;
        bad[8] = (MAX_USE_WEAPONS + 1) as u8;
        assert_eq!(FromHost::decode(&Frame { ty: ty::USED, payload: bad }), Err(Error::Limit("Used weapons")));
        let many = Used { t: 0, players: vec![PlayerUse::default(); MAX_PLAYERS + 1] };
        assert_eq!(FromHost::Used(many).payload(), Err(Error::Limit("Used players")));
    }

    #[test]
    fn version_3_trailer_is_bounded() {
        let o = Outcome { t_end: 0, state: 5, winner_team: 0, pve_success: false, mission_ms: 0, players: vec![], team_score: [0; 2], details: details(20) };
        assert!(FromHost::Final(o.clone()).frame().is_ok());
        let mut more = o.clone();
        more.details.push(PlayerDetail::default());
        assert!(FromHost::Final(more).frame().is_err(), "21 rows");
        let mut dup = o.clone();
        dup.details[3].slot = 2;
        assert_eq!(FromHost::Final(dup).payload(), Err(Error::Limit("Outcome detail slot")));
        let mut far = o.clone();
        far.details.truncate(1);
        far.details[0].slot = 20;
        assert_eq!(FromHost::Final(far).payload(), Err(Error::Limit("Outcome detail slot")));
        // a wire mask with bit 27, or a set bit with three zero counts, is refused
        // details(1): slot 0 has badges 5 and 23 (badge 26 count is 0): the mask at 20 + 1 + 14 + 148 = 183, two x 3 bytes
        let one = Outcome { details: details(1), ..o.clone() };
        let mut p = FromHost::Final(one).payload().unwrap();
        assert_eq!(p.len(), 183 + 4 + 6);
        assert_eq!(&p[183..187], &[0x20, 0, 0x80, 0]);
        p[186] |= 0x08;
        assert_eq!(FromHost::decode(&Frame { ty: ty::FINAL, payload: p.clone() }), Err(Error::Limit("Outcome badge mask")));
        p[186] &= !0x08;
        p[187..190].copy_from_slice(&[0, 0, 0]);
        assert_eq!(FromHost::decode(&Frame { ty: ty::FINAL, payload: p }), Err(Error::Limit("Outcome badge bit without a count")));
        // a version-2 Outcome (no trailer) is too short for version 3
        let mut v2 = FromHost::Final(Outcome { details: vec![], ..o }).payload().unwrap();
        v2.pop();
        assert_eq!(FromHost::decode(&Frame { ty: ty::FINAL, payload: v2 }), Err(Error::Short));
        // too many kills
        let k = Kills { t_upto: 0, records: vec![KillRecord::default(); MAX_KILLS + 1] };
        assert_eq!(FromHost::Kills(k).payload(), Err(Error::Limit("Kills records")));
        let mut p = vec![0, 0, 0, 0];
        p.extend_from_slice(&((MAX_KILLS + 1) as u16).to_le_bytes());
        assert_eq!(FromHost::decode(&Frame { ty: ty::KILLS, payload: p }), Err(Error::Limit("Kills records")));
        // a cut record
        let mut p = FromHost::Kills(Kills { t_upto: 1, records: vec![KillRecord::default()] }).payload().unwrap();
        p.pop();
        assert_eq!(FromHost::decode(&Frame { ty: ty::KILLS, payload: p }), Err(Error::Short));
    }

    #[test]
    fn a_stream_of_frames_splits_in_any_chunks() {
        let msgs = from_host_samples();
        let mut all = Vec::new();
        for m in &msgs {
            all.extend(m.frame().unwrap());
        }
        for chunk in [1usize, 3, 7, 4096] {
            let mut d = FrameDecoder::new();
            let mut got = Vec::new();
            for c in all.chunks(chunk) {
                d.push(c);
                while let Some(f) = d.next_frame().unwrap() {
                    got.push(FromHost::decode(&f).unwrap());
                }
            }
            assert_eq!(got, msgs, "chunk {chunk}");
            assert_eq!(d.pending(), 0);
        }
        // the blocking reader: frames back to back, then a clean EOF
        let mut r = &all[..];
        let mut n = 0;
        while let Some(f) = read_frame(&mut r).unwrap() {
            assert_eq!(FromHost::decode(&f).unwrap(), msgs[n]);
            n += 1;
        }
        assert_eq!(n, msgs.len());
    }

    #[test]
    fn untrusted_frames_are_bounded() {
        // length 0 and above 4 MiB
        assert!(read_frame(&mut &[0u8, 0, 0, 0][..]).is_err());
        let big = ((MAX_FRAME + 1) as u32).to_le_bytes();
        assert!(read_frame(&mut &big[..]).is_err());
        let mut d = FrameDecoder::new();
        d.push(&big);
        assert_eq!(d.next_frame(), Err(Error::FrameLen(MAX_FRAME as u32 + 1)));
        // EOF inside a frame
        assert!(read_frame(&mut &[5u8, 0, 0, 0, ty::CLOSE, 1][..]).is_err());
        // too many players
        let mut o = Outcome { t_end: 0, state: 5, winner_team: 0, pve_success: true, mission_ms: 0, players: stats(20), team_score: [0; 2], details: vec![] };
        assert!(FromHost::Outcome(o.clone()).frame().is_ok());
        o.players.push(PlayerStats::default());
        assert_eq!(FromHost::Outcome(o).frame(), Err(Error::Limit("Outcome players")));
        let f = Frame { ty: ty::STATS, payload: vec![0, 0, 0, 0, 21] };
        assert_eq!(FromHost::decode(&f), Err(Error::Limit("Stats players")));
        // a long text
        let f = FromHost::Fault { reason: "y".repeat(MAX_TEXT + 1) };
        assert!(f.frame().is_err());
        let mut p = vec![];
        p.extend_from_slice(&((MAX_TEXT + 1) as u16).to_le_bytes());
        p.extend(std::iter::repeat_n(b'y', MAX_TEXT + 1));
        assert_eq!(FromHost::decode(&Frame { ty: ty::FAULT, payload: p }), Err(Error::Limit("Fault reason")));
        // a blob longer than the frame
        let f = Frame { ty: ty::DF, payload: vec![0, 0, 0, 0, 0xFF, 0xFF, 0, 0] };
        assert_eq!(ToHost::decode(&f), Err(Error::Short));
        // trailing bytes, short payload, unknown type, a type of the other direction
        assert_eq!(ToHost::decode(&Frame { ty: ty::CLOSE, payload: vec![0, 0, 0, 0, 9] }), Err(Error::Trailing(1)));
        assert_eq!(ToHost::decode(&Frame { ty: ty::CLOSE, payload: vec![0, 0] }), Err(Error::Short));
        assert_eq!(FromHost::decode(&Frame { ty: 0x99, payload: vec![] }), Err(Error::Type(0x99)));
        assert_eq!(FromHost::decode(&Frame { ty: ty::CLOSE, payload: vec![0; 4] }), Err(Error::Type(ty::CLOSE)));
        assert_eq!(ToHost::decode(&Frame { ty: ty::READY, payload: vec![0; 16] }), Err(Error::Type(ty::READY)));
        // a Tick whose hashes do not match its mask, or a mask bit beyond 20 players
        let t = Tick { t: 0, mask: [1, 0, 0], hashes: vec![], g: [0; 3] };
        assert!(FromHost::Tick(t).frame().is_err());
        let f = Frame { ty: ty::TICK, payload: vec![0, 0, 0, 0, 0, 0, 0x10, 1, 0, 0, 0] };
        assert_eq!(FromHost::decode(&f), Err(Error::Limit("Tick mask beyond 20 players")));
        // bad UTF-8
        assert_eq!(FromHost::decode(&Frame { ty: ty::FAULT, payload: vec![1, 0, 0xFF] }), Err(Error::Utf8));
    }
}

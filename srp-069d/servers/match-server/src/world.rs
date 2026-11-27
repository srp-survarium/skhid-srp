//! Cached initial worlds, one file per (map, mode, project), and the collision checksum.
//!
//! Without a world host the relay has no game simulation, so the world sent as the first DE to every client comes from
//! a file: a capture of a local-match start of the stock client (the item writer 0x2002FC5A0 with full = 1, procedure
//! in docs/notes/online.md 2.2) packed by `srpde-pack`. With a world host the host builds the start world instead.
//!
//! # `.srpde` file format (little endian)
//!
//! ```text
//!  0  8  magic "SRPDE005"
//!  8  u32 map id            lobby map id (queue / `5B` map)
//! 12  u32 mode id           lobby mode id
//! 16  u32 time              match clock of the captured item = DE time = first u32 of the serialized item
//! 20  u32 match_id          seed the world was captured with; a match using this world MUST use this match_id
//!                           (match_id is the random seed unless single_player; LIKELY inside G1..G4)
//! 24  u32 level_checksum    DD checksum: the client's own CRC-32 of the collision world, copied from its log line
//!                           "Checksum on %s: Client = %x" (a mismatch is a fatal assertion in the client)
//! 28  u8  tick_ms           tick of the captured match (options +0x1083), must not be 0
//! 29  u8  body_kind         0 = the serialized item as the writer produced it (the server packs it per DE),
//!                           1 = the item already as raw LZMA2 (sent as it is, compressed = 1)
//! 30  u16 send_period_ms    options +0x1078 of the captured match
//! 32  u32 roster_objects    objects of the capture's own roster at the head of G1 (see [`RosterObjects`])
//! 36  u32 roster_bytes      their bytes, right after G1's u32 count
//! 40  u32 records_at        G3 offset of the kd_stats per-record block (FFFFFFFF = not located)
//! 44  u32 records           options records of the capture (the block holds 5 bytes per record)
//! 48  u32 teams_from_end    distance of the timelimit rule's two team counts from G3's end (FFFFFFFF = not located)
//! 52  u32 flags             bit 0 = fixed roster: G3 holds per-record blocks besides kd_stats that are not located, so
//!                           the world serves only a roster of the capture's record count (every PvE world)
//! 56  64  project           the level project, NUL padded ("" = the (map, mode) row's; PvE: `pve_01_v1_maze1` ..., the
//!                           six projects share map 15 / mode 7 and differ in level checksum and seed)
//! 120 u32 body_len
//! 124 body
//! ```
//!
//! `SRPDE004` files (the same fields up to 48, then `u32 body_len` at 52 and the body at 56) still load: project "",
//! flags 0.
//!
//! # Per-player objects
//!
//! Section G1 of the item (`u16 len` at item offset 22, then `u32 count` and the objects) starts with the objects the
//! roster's kits put in the world (grenades, medkits: [`RosterObjects`]), then the level's own objects. The client builds
//! its own list from the options of DD, so a world sent to a match must carry exactly that match's roster objects:
//! [`World::with_roster`] swaps the capture's part for the match's (an 88-object capture against the 106 objects of a
//! two-player roster misparses in both clients).
//!
//! # Per-record block in G3
//!
//! `kd_stats_rule` (rule type 3) saves 5 bytes per options record (u16 kills, u16 deaths, u8 flag; saver 0x20030EDF0)
//! and takes the count from the client's own options (`opts+0x1080`), not from the stream. At t = 50 the block is all
//! zero. Its G3 offset is 42 in TDM / CTP / slaughter (head 26 + mode rule 16); battle royale, artefact hunt and PvE are
//! not split yet, so their worlds only serve a roster with the capture's record count. A 1-record capture against 2
//! online records shifts every later G3 byte by 5 and both clients crash saving their next item.
//!
//! PvE: besides kd_stats (5 bytes per record at G3 offset 26) the scripting rule saves four blocks whose length follows
//! the record count. Their offsets in G3 are not located, so a PvE world is captured with the match's full roster (the humans and the 17 script bots)
//! and is [fixed](World::fixed_roster): it serves only a roster of the capture's record count; another count is refused
//! with an error (OPEN: a splitter that replays the scene saver).
//!
//! The timelimit rule saves the two team counts `n_players >> 1` last (`+0x273`, `+0x274`; saver 0x200365A25) and the
//! loader overwrites the client's own values with them; in TDM / CTP / slaughter they are 16 and 15 bytes before G3's
//! end (timelimit 20 + statistics 14). [`World::with_roster`] writes `records >> 1` (0 / 0 from a one-record capture
//! misjudges the short team after a leave). The decoded item must be at most 0x44800 bytes. UNVERIFIED: that a world
//! captured with C = 0 (no players) loads in an online client; the fallback is a capture after `enter_match`.

use crate::match_msgs::{Packing, WorldMsg, DE_MAX_BODY};
use protocol::{lzma2, Encode};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

pub const MAGIC: &[u8; 8] = b"SRPDE005";
/// The previous format (no flags, no project): still loaded.
pub const MAGIC_004: &[u8; 8] = b"SRPDE004";
const HEADER_LEN: usize = 124;
const HEADER_LEN_004: usize = 56;
/// Bytes of the project field (NUL padded; at most 63 used).
pub const PROJECT_LEN: usize = 64;
/// `flags` bit: the world serves only its capture's record count.
pub const FLAG_FIXED_ROSTER: u32 = 1;

/// A level project whose G3 is roster-fixed: the PvE mission projects `pve_01_*`.
pub fn is_fixed_roster_project(project: &str) -> bool {
    project.starts_with("pve_")
}
/// Bytes per options record in the kd_stats block of G3.
pub const KD_PER_RECORD: usize = 5;
/// `records_at` of a world whose kd_stats block is not located.
pub const RECORDS_NOT_LOCATED: u32 = u32::MAX;
/// Item offset of G1's u16 length (after the 22-byte item header).
const G1_AT: usize = 22;

/// One per-player object of the starting world, as the t = 50 item saves it (savers PROVEN). Every object is one node
/// of the level-object list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartObject {
    /// `placeable_medkit` (data.type 16; saver 0x2003DB5F0): state 0, not deployed, owner byte +0x60C, +0x588 = 0:
    /// `00 00 XX 00`. XX is uninitialised memory in the client (the ctor 0x2003D9F60 never writes it, any value loads);
    /// we write 0. Spec letter `M`.
    Medkit,
    /// `grenade` (data.type 9, and the one grenade of an artefact_sponge; saver 0x2003DC4C0): kind 0, no owner, the
    /// inactive state writes nothing: `00 FF`. Spec letter `G`.
    Grenade,
    /// `booby_trap` (data.type 2; base placeable saver 0x20039CCB0): state 0: `00`. Spec letter `T`.
    BoobyTrap,
    /// `explosive_trap` (data.type 3: mines, C4): `00 FF FF FF FF FF FF XX`, XX uninitialised (+0x610); we write 0.
    /// Spec letter `E`.
    ExplosiveTrap,
    /// `stationary_scanner` (data.type 13; saver 0x2003D5F90): `00 00`. Spec letter `S`.
    Scanner,
    /// `healing_totem` (data.type 14; saver 0x2003D75B0): `00 00`. Spec letter `H`.
    Totem,
    /// `placeable_resupply_crate` (data.type 15; saver 0x2003D9A80): `00 00` + 20 x `FF FF FF FF`. Spec letter `C`.
    Crate,
}

const CRATE: [u8; 82] = {
    let mut b = [0xFFu8; 82];
    b[0] = 0;
    b[1] = 0;
    b
};

impl StartObject {
    pub fn t50_bytes(self) -> &'static [u8] {
        match self {
            StartObject::Medkit => &[0, 0, 0, 0],
            StartObject::Grenade => &[0, 0xFF],
            StartObject::BoobyTrap => &[0],
            StartObject::ExplosiveTrap => &[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0],
            StartObject::Scanner | StartObject::Totem => &[0, 0],
            StartObject::Crate => &CRATE,
        }
    }

    fn letter(c: u8) -> Option<Self> {
        Some(match c {
            b'M' => StartObject::Medkit,
            b'G' => StartObject::Grenade,
            b'T' => StartObject::BoobyTrap,
            b'E' => StartObject::ExplosiveTrap,
            b'S' => StartObject::Scanner,
            b'H' => StartObject::Totem,
            b'C' => StartObject::Crate,
            _ => return None,
        })
    }

    /// Does `b` hold this object as a capture saved it (the uninitialised byte of a medkit / explosive trap is free)?
    fn matches(self, b: &[u8]) -> bool {
        let want = self.t50_bytes();
        let free = match self {
            StartObject::Medkit => Some(2),
            StartObject::ExplosiveTrap => Some(7),
            _ => None,
        };
        b.len() == want.len() && b.iter().zip(want).enumerate().all(|(i, (x, y))| x == y || Some(i) == free)
    }
}

/// The per-player objects of a roster, from its spec (`local_match::start_objects`): records separated by `|`, the
/// state-0 profiles by `,`, each profile its objects in slot order as a letter ([`StartObject`]) and a count (none = 1):
/// `MG5,MG5,MG5` = three profiles of one medkit and five grenades.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RosterObjects {
    pub objects: Vec<StartObject>,
    /// Options records of the roster (the spec's `|`-separated parts).
    pub records: u32,
}

impl RosterObjects {
    pub fn from_spec(spec: &str) -> Result<Self, WorldError> {
        let mut objects = Vec::new();
        let b = spec.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'|' || b[i] == b',' {
                i += 1;
                continue;
            }
            let o = StartObject::letter(b[i]).ok_or(WorldError::Format("roster spec: unknown object letter"))?;
            let j = b[i + 1..].iter().position(|c| !c.is_ascii_digit()).map_or(b.len(), |k| i + 1 + k);
            let n: usize = if j == i + 1 { 1 } else { spec[i + 1..j].parse().map_err(|_| WorldError::Format("roster spec: bad count"))? };
            if n > 0x200 {
                return Err(WorldError::Format("roster spec: count over 512"));
            }
            objects.extend(std::iter::repeat(o).take(n));
            i = j;
        }
        Ok(Self { objects, records: spec.split('|').count() as u32 })
    }

    pub fn count(&self) -> u32 {
        self.objects.len() as u32
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.objects.iter().flat_map(|o| o.t50_bytes().iter().copied()).collect()
    }

    /// The length of this roster's objects at the head of `g` (G1 after its count), if `g` starts with them.
    fn prefix_len(&self, g: &[u8]) -> Option<usize> {
        let mut p = 0;
        for o in &self.objects {
            let n = o.t50_bytes().len();
            if !o.matches(g.get(p..p + n)?) {
                return None;
            }
            p += n;
        }
        Some(p)
    }
}

/// (start, length) of every section of the item: G1, G2, G3, G4 follow the 22-byte header, each `u16 len` + bytes.
fn sections(item: &[u8]) -> Result<Vec<(usize, usize)>, WorldError> {
    let mut p = G1_AT;
    let mut out = Vec::new();
    while p + 2 <= item.len() {
        let len = u16::from_le_bytes([item[p], item[p + 1]]) as usize;
        if p + 2 + len > item.len() {
            return Err(WorldError::Format("a section runs past the item"));
        }
        out.push((p + 2, len));
        p += 2 + len;
    }
    if p != item.len() || out.len() < 3 {
        return Err(WorldError::Format("the item is not header + sections G1..G4"));
    }
    Ok(out)
}

/// (G3 start, G3 length) of a later history item, which carries the players' input lists between the 22-byte header and
/// G1 (t = 100 items: about 3.2 KB): G1 is the first offset whose four sections end exactly at the item end and whose
/// object count is `g1_count` (the same level's objects).
fn sections_after_prefix(item: &[u8], g1_count: u32) -> Result<(usize, usize), WorldError> {
    for o in G1_AT..item.len().saturating_sub(8) {
        let mut p = o;
        let mut lens = [0usize; 4];
        let mut ok = true;
        for l in lens.iter_mut() {
            let Some(b) = item.get(p..p + 2) else {
                ok = false;
                break;
            };
            *l = u16::from_le_bytes([b[0], b[1]]) as usize;
            p += 2 + *l;
        }
        if ok && p == item.len() && lens[0] >= 4 && item.get(o + 2..o + 6) == Some(&g1_count.to_le_bytes()[..]) {
            let g3 = o + 2 + lens[0] + 2 + lens[1] + 2;
            return Ok((g3, lens[2]));
        }
    }
    Err(WorldError::Format("later item: no G1..G4 with this world's G1 object count"))
}

/// (G1 start in the item, G1 length): G1 = `u16 len` at [`G1_AT`], then the section.
fn g1_span(item: &[u8]) -> Result<(usize, usize), WorldError> {
    let len = item.get(G1_AT..G1_AT + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or(WorldError::Format("item shorter than G1's length"))?;
    if len < 4 || item.len() < G1_AT + 2 + len {
        return Err(WorldError::Format("G1 shorter than its count or past the item"));
    }
    Ok((G1_AT + 2, len))
}
/// The largest DE payload the client can receive from us: 255 parts (u8 count, 0x2007568EF) of our part size.
pub const DE_MAX_PAYLOAD: usize = 255 * udp_transport::wire::PART;

#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error("world file: {0}")]
    Format(&'static str),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("lzma2: {0}")]
    Lzma(String),
    #[error("{0}")]
    Roster(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct World {
    pub map: u32,
    pub mode: u32,
    pub time: u32,
    pub match_id: u32,
    pub level_checksum: u32,
    pub tick_ms: u8,
    pub send_period_ms: u16,
    /// Objects of the capture's own roster at the head of G1 and their bytes (0, 0 = none).
    pub roster_objects: u32,
    pub roster_bytes: u32,
    /// G3 offset of the kd_stats per-record block ([`RECORDS_NOT_LOCATED`] = unknown) and the capture's record count.
    pub records_at: u32,
    pub records: u32,
    /// Distance of the two team-count bytes from G3's end ([`RECORDS_NOT_LOCATED`] = unknown).
    pub teams_from_end: u32,
    /// The level project ("" = the (map, mode) row's own); a PvE world names one of the six mission projects.
    pub project: String,
    /// G3 holds per-record blocks that are not located: only a roster of `records` records may use this world.
    pub fixed_roster: bool,
    /// Serialized item (decoded; `compressed = false`), or its raw LZMA2 (`already_lzma2`).
    pub body: Vec<u8>,
    pub already_lzma2: bool,
}

impl World {
    /// A world from the serialized item `item` (its first u32 is the time).
    pub fn from_item(map: u32, mode: u32, match_id: u32, level_checksum: u32, tick_ms: u8, send_period_ms: u16, item: Vec<u8>) -> Result<Self, WorldError> {
        if item.len() < 4 {
            return Err(WorldError::Format("item shorter than its time field"));
        }
        let time = u32::from_le_bytes([item[0], item[1], item[2], item[3]]);
        let w = Self {
            map,
            mode,
            time,
            match_id,
            level_checksum,
            tick_ms,
            send_period_ms,
            roster_objects: 0,
            roster_bytes: 0,
            records_at: RECORDS_NOT_LOCATED,
            records: 0,
            teams_from_end: RECORDS_NOT_LOCATED,
            project: String::new(),
            fixed_roster: false,
            body: item,
            already_lzma2: false,
        };
        w.validate()?;
        Ok(w)
    }

    fn validate(&self) -> Result<(), WorldError> {
        if self.tick_ms == 0 {
            return Err(WorldError::Format("tick_ms is 0 (the client divides by it)"));
        }
        if self.time % u32::from(self.tick_ms) != 0 {
            // PROVEN: the client asserts `CD.time % tick_ms == 0` (fatal, 0x20032F1A7), and its CD
            // times follow the DE time, so a world off the tick grid would crash every client
            return Err(WorldError::Format("world time is not a multiple of tick_ms"));
        }
        let item = self.item()?;
        if item.len() > DE_MAX_BODY {
            return Err(WorldError::Format("item over 0x44800 bytes"));
        }
        if item.len() < 22 {
            return Err(WorldError::Format("item shorter than its 22 byte header"));
        }
        if u32::from_le_bytes([item[0], item[1], item[2], item[3]]) != self.time {
            return Err(WorldError::Format("item time differs from the header time (client asserts they are equal)"));
        }
        // the client reads the part count as a u8 (0x2007568EF): a DE needs at most 255 parts of our part size, else
        // `send_de` fails at match start and nobody gets a world
        for packing in [Packing::Raw, Packing::StoredLzma2] {
            if self.de_payload(packing)?.len() > DE_MAX_PAYLOAD {
                return Err(WorldError::Format("DE payload over 255 transport parts (store the item as raw LZMA2, body_kind 1)"));
            }
        }
        Ok(())
    }

    /// The decoded serialized item.
    pub fn item(&self) -> Result<Vec<u8>, WorldError> {
        if self.already_lzma2 {
            lzma2::decompress(&self.body).map_err(|e| WorldError::Lzma(e.to_string()))
        } else {
            Ok(self.body.clone())
        }
    }

    /// The DE message payload (`u32 time, u8 compressed, body`).
    pub fn de_payload(&self, packing: Packing) -> Result<Vec<u8>, WorldError> {
        let msg = if self.already_lzma2 {
            WorldMsg { time: self.time, compressed: 1, body: self.body.clone() }
        } else {
            WorldMsg::pack(self.time, &self.body, packing).map_err(|_| WorldError::Format("item too large"))?
        };
        msg.to_bytes().map_err(|_| WorldError::Format("DE encode"))
    }

    pub fn to_file_bytes(&self) -> Vec<u8> {
        let mut o = Vec::with_capacity(HEADER_LEN + self.body.len());
        o.extend_from_slice(MAGIC);
        for v in [self.map, self.mode, self.time, self.match_id, self.level_checksum] {
            o.extend_from_slice(&v.to_le_bytes());
        }
        o.push(self.tick_ms);
        o.push(u8::from(self.already_lzma2));
        o.extend_from_slice(&self.send_period_ms.to_le_bytes());
        o.extend_from_slice(&self.roster_objects.to_le_bytes());
        o.extend_from_slice(&self.roster_bytes.to_le_bytes());
        o.extend_from_slice(&self.records_at.to_le_bytes());
        o.extend_from_slice(&self.records.to_le_bytes());
        o.extend_from_slice(&self.teams_from_end.to_le_bytes());
        o.extend_from_slice(&(if self.fixed_roster { FLAG_FIXED_ROSTER } else { 0 }).to_le_bytes());
        let mut p = [0u8; PROJECT_LEN];
        p[..self.project.len().min(PROJECT_LEN - 1)].copy_from_slice(&self.project.as_bytes()[..self.project.len().min(PROJECT_LEN - 1)]);
        o.extend_from_slice(&p);
        o.extend_from_slice(&(self.body.len() as u32).to_le_bytes());
        o.extend_from_slice(&self.body);
        o
    }

    pub fn from_file_bytes(b: &[u8]) -> Result<Self, WorldError> {
        let (header, old) = match b.get(..8) {
            Some(m) if m == MAGIC => (HEADER_LEN, false),
            Some(m) if m == MAGIC_004 => (HEADER_LEN_004, true),
            _ => return Err(WorldError::Format("bad magic or short header")),
        };
        if b.len() < header {
            return Err(WorldError::Format("bad magic or short header"));
        }
        let u32_at = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let body_len = u32_at(header - 4) as usize;
        let body = b.get(header..header + body_len).ok_or(WorldError::Format("body shorter than body_len"))?;
        let kind = b[29];
        if kind > 1 {
            return Err(WorldError::Format("unknown body_kind"));
        }
        let (flags, project) = if old {
            (0, String::new())
        } else {
            let raw = &b[56..56 + PROJECT_LEN];
            let n = raw.iter().position(|&c| c == 0).unwrap_or(PROJECT_LEN);
            (u32_at(52), std::str::from_utf8(&raw[..n]).map_err(|_| WorldError::Format("project is not UTF-8"))?.to_string())
        };
        let w = Self {
            map: u32_at(8),
            mode: u32_at(12),
            time: u32_at(16),
            match_id: u32_at(20),
            level_checksum: u32_at(24),
            tick_ms: b[28],
            send_period_ms: u16::from_le_bytes([b[30], b[31]]),
            roster_objects: u32_at(32),
            roster_bytes: u32_at(36),
            records_at: u32_at(40),
            records: u32_at(44),
            teams_from_end: u32_at(48),
            project,
            fixed_roster: flags & FLAG_FIXED_ROSTER != 0,
            body: body.to_vec(),
            already_lzma2: kind == 1,
        };
        w.validate()?;
        Ok(w)
    }

    pub fn load(path: &Path) -> Result<Self, WorldError> {
        Self::from_file_bytes(&std::fs::read(path)?)
    }

    /// Mark the head of G1 as the capture's own roster objects (`srpde-pack --capture-roster`): G1 must start with
    /// exactly those objects in their t = 50 form, else the spec is not the capture's roster.
    pub fn with_capture_roster(mut self, r: &RosterObjects) -> Result<Self, WorldError> {
        let item = self.item()?;
        let (at, len) = g1_span(&item)?;
        let g = &item[at..at + len];
        let count = u32::from_le_bytes([g[0], g[1], g[2], g[3]]);
        if count < r.count() {
            return Err(WorldError::Format("G1 has fewer objects than the capture roster"));
        }
        let n = r.prefix_len(&g[4..]).ok_or(WorldError::Format("G1 does not start with the capture roster's objects"))?;
        self.roster_objects = r.count();
        self.roster_bytes = n as u32;
        self.records = r.records;
        Ok(self)
    }

    /// Name the world's level project (`srpde-pack --project`): the six PvE projects share (map, mode) and are told apart
    /// by it. A PvE project makes the world [fixed](Self::fixed_roster) (its G3 has per-record blocks that are not
    /// located).
    pub fn with_project(mut self, project: &str) -> Result<Self, WorldError> {
        if project.len() >= PROJECT_LEN || project.bytes().any(|c| c == 0 || !c.is_ascii_graphic()) {
            return Err(WorldError::Format("project: 1..63 printable ASCII characters"));
        }
        self.project = project.to_string();
        self.fixed_roster = is_fixed_roster_project(project);
        Ok(self)
    }

    /// Locate G3's kd_stats block at `at` (`srpde-pack --kd-at`): the capture's records x 5 bytes there must be zero
    /// (t = 50) and inside G3. Call after [`with_capture_roster`](Self::with_capture_roster) (it sets the record count).
    pub fn with_record_block(mut self, at: u32) -> Result<Self, WorldError> {
        if self.fixed_roster {
            return Err(WorldError::Format("a fixed-roster (PvE) world: its G3 has more per-record blocks than kd_stats"));
        }
        let item = self.item()?;
        let s = sections(&item)?;
        let (g3, len) = s[2];
        let n = KD_PER_RECORD * self.records as usize;
        let block = item.get(g3 + at as usize..g3 + at as usize + n).filter(|_| at as usize + n <= len).ok_or(WorldError::Format("kd_stats block past G3"))?;
        if block.iter().any(|&b| b != 0) {
            return Err(WorldError::Format("kd_stats block is not zero (wrong offset or not a t = 50 item)"));
        }
        self.records_at = at;
        Ok(self)
    }

    /// Locate the timelimit rule's team counts `from_end` bytes before G3's end (`srpde-pack --teams-from-end`): the two
    /// bytes must hold the capture's `records >> 1`.
    pub fn with_team_counts(mut self, from_end: u32) -> Result<Self, WorldError> {
        let item = self.item()?;
        let (g3, len) = sections(&item)?[2];
        let at = len.checked_sub(from_end as usize).filter(|a| a + 2 <= len).ok_or(WorldError::Format("team counts outside G3"))?;
        let want = (self.records >> 1) as u8;
        if item[g3 + at] != want || item[g3 + at + 1] != want {
            return Err(WorldError::Format("team counts are not the capture's records / 2 (wrong offset?)"));
        }
        self.teams_from_end = from_end;
        Ok(self)
    }

    /// Give a PvE capture the scripting rule's per-slot environment list of the same capture's next item `later` (the
    /// t = 100 item, after the client's first world step; `srpde-pack --scene-env-from`). Why: an online client runs the
    /// script's first step after the DE (it sets an environment name per player slot), then rolls back to the DE item;
    /// loading a slot the item has as -1 while the client holds a name crashes it (0x2002AA1B3). The original server
    /// sent a world past the first step. Only the 80 list bytes are taken from `later`; every other byte stays.
    /// Layout (saver 0x2003FDC17): 20 x u32 string offsets (-1 = none), then four u8-counted u32 lists, then the later
    /// rules' bytes, whose length is the same in both items. In this item the list must be all -1 and the four lists
    /// empty.
    pub fn with_scene_env_from(mut self, later: &[u8]) -> Result<Self, WorldError> {
        const LIST: usize = 20 * 4;
        let mut item = self.item()?;
        let (g3, len) = sections(&item)?[2];
        let x = &item[g3..g3 + len];
        let mut pat = vec![0xFFu8; LIST];
        pat.extend_from_slice(&[0; 4]);
        let hits: Vec<usize> = x.windows(pat.len()).enumerate().filter(|(_, w)| *w == &pat[..]).map(|(i, _)| i).collect();
        let &[at] = hits.as_slice() else {
            return Err(WorldError::Format("scene env: this item's empty list + empty lists not found exactly once in G3"));
        };
        let tail = len - (at + LIST + 4);
        let g1 = g1_span(&item)?.0;
        let g1_count = u32::from_le_bytes([item[g1], item[g1 + 1], item[g1 + 2], item[g1 + 3]]);
        let (lg3, llen) = sections_after_prefix(later, g1_count)?;
        let y = &later[lg3..lg3 + llen];
        let end = llen.checked_sub(tail).ok_or(WorldError::Format("scene env: later G3 shorter than the tail"))?;
        let mut found = Vec::new();
        for s in (LIST..end.saturating_sub(3)).rev() {
            let mut p = s;
            for _ in 0..4 {
                if p >= end {
                    break;
                }
                p += 1 + 4 * y[p] as usize;
            }
            let slots = y[s - LIST..s].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]));
            if p == end && slots.clone().all(|v| v == u32::MAX || v < 0x40_0000) {
                found.push(s - LIST);
            }
        }
        let &[from] = found.as_slice() else {
            return Err(WorldError::Format("scene env: the later item's list not found exactly once"));
        };
        item[g3 + at..g3 + at + LIST].copy_from_slice(&y[from..from + LIST]);
        self.body = if self.already_lzma2 { lzma2::compress(&item) } else { item };
        Ok(self)
    }

    /// This world for a match whose roster puts `r` in the world: the capture's roster objects are replaced by `r` and
    /// the G1 count and length follow; the level's objects and every other byte stay. A [fixed](Self::fixed_roster) world
    /// refuses a roster of another record count.
    pub fn with_roster(&self, r: &RosterObjects) -> Result<Self, WorldError> {
        if self.fixed_roster && r.records != self.records {
            return Err(WorldError::Roster(format!(
                "the world of project {} serves only its capture's roster of {} records, the match has {} (its G3 per-record scripting blocks are not located: OPEN)",
                self.project, self.records, r.records
            )));
        }
        let item = self.item()?;
        let (at, len) = g1_span(&item)?;
        let g = &item[at..at + len];
        let count = u32::from_le_bytes([g[0], g[1], g[2], g[3]]);
        let cut = 4 + self.roster_bytes as usize;
        if count < self.roster_objects || cut > len {
            return Err(WorldError::Format("G1 shorter than the capture roster"));
        }
        let mut g1 = (count - self.roster_objects + r.count()).to_le_bytes().to_vec();
        g1.extend_from_slice(&r.bytes());
        g1.extend_from_slice(&g[cut..]);
        let new_len = u16::try_from(g1.len()).map_err(|_| WorldError::Format("G1 over 65535 bytes"))?;
        let mut out = item[..G1_AT].to_vec();
        out.extend_from_slice(&new_len.to_le_bytes());
        out.extend_from_slice(&g1);
        out.extend_from_slice(&item[at + len..]);
        if r.records != self.records {
            out = self.g3_for_records(&out, r.records)?;
        }
        let mut w = self.clone();
        w.roster_objects = r.count();
        w.roster_bytes = r.bytes().len() as u32;
        w.records = r.records;
        w.body = if self.already_lzma2 { lzma2::compress(&out) } else { out };
        w.validate()?;
        Ok(w)
    }

    /// `item` with G3's kd_stats block sized for `records` (5 zero bytes each, the t = 50 form).
    fn g3_for_records(&self, item: &[u8], records: u32) -> Result<Vec<u8>, WorldError> {
        if self.records_at == RECORDS_NOT_LOCATED || self.teams_from_end == RECORDS_NOT_LOCATED {
            return Err(WorldError::Format("the kd_stats block or the team counts of this world are not located (srpde-pack --kd-at, --teams-from-end): it serves only the capture's record count"));
        }
        let s = sections(item)?;
        let (g3, len) = s[2];
        let at = self.records_at as usize;
        let old = KD_PER_RECORD * self.records as usize;
        if at + old > len {
            return Err(WorldError::Format("kd_stats block past G3"));
        }
        let mut new_g3 = item[g3..g3 + at].to_vec();
        new_g3.extend(std::iter::repeat(0u8).take(KD_PER_RECORD * records as usize));
        new_g3.extend_from_slice(&item[g3 + at + old..g3 + len]);
        let t = new_g3.len().checked_sub(self.teams_from_end as usize).filter(|t| t + 2 <= new_g3.len()).ok_or(WorldError::Format("team counts outside G3"))?;
        new_g3[t] = (records >> 1) as u8;
        new_g3[t + 1] = (records >> 1) as u8;
        let new_len = u16::try_from(new_g3.len()).map_err(|_| WorldError::Format("G3 over 65535 bytes"))?;
        let mut out = item[..g3 - 2].to_vec();
        out.extend_from_slice(&new_len.to_le_bytes());
        out.extend_from_slice(&new_g3);
        out.extend_from_slice(&item[g3 + len..]);
        Ok(out)
    }

    /// The world as the clients may get it: an item earlier than [`min_first_time`] is relabelled at that time (the item's
    /// first u32 and the header time; nothing else changes, so every client still loads the same bytes). A capture is
    /// the t = 50 item of a local start (docs/notes/online.md 2.2), but the client's history backfill after the first DE
    /// (`deserialize` 0x2002F19C3) walks down from T - tick to T - horizon - tick with horizon = options +0x107A. When
    /// T < horizon the walk wraps below 0 and writes through a null item: every client crashes. The original server sent
    /// the world it had been running for seconds, never t = 50.
    pub fn for_clients(mut self) -> Result<Self, WorldError> {
        let min = min_first_time(self.tick_ms);
        if self.time >= min {
            return Ok(self);
        }
        let mut item = self.item()?;
        item[..4].copy_from_slice(&min.to_le_bytes());
        self.time = min;
        self.body = if self.already_lzma2 { lzma2::compress(&item) } else { item };
        self.validate()?;
        Ok(self)
    }
}

/// The earliest world time a first DE may carry: 16 ticks. The lobby keeps the options' maximum remote delay at most 15
/// ticks (`local_match::options::max_delay_ms`), so T >= horizon + tick and the client's backfill never wraps below 0.
pub fn min_first_time(tick_ms: u8) -> u32 {
    16 * u32::from(tick_ms)
}

/// Worlds by (map, mode, project).
#[derive(Default)]
pub struct WorldStore {
    worlds: BTreeMap<(u32, u32, String), Arc<World>>,
}

impl WorldStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, w: World) {
        self.worlds.insert((w.map, w.mode, w.project.clone()), Arc::new(w));
    }

    /// A world of (map, mode): the one without a project name, else the first project in name order.
    pub fn get(&self, map: u32, mode: u32) -> Option<Arc<World>> {
        self.worlds.range((map, mode, String::new())..).next().filter(|(k, _)| k.0 == map && k.1 == mode).map(|(_, w)| w.clone())
    }

    /// The world of exactly this project of (map, mode).
    pub fn get_project(&self, map: u32, mode: u32, project: &str) -> Option<Arc<World>> {
        self.worlds.get(&(map, mode, project.to_string())).cloned()
    }

    /// The projects of (map, mode) that have a world.
    pub fn projects(&self, map: u32, mode: u32) -> Vec<String> {
        self.worlds.keys().filter(|k| k.0 == map && k.1 == mode).map(|k| k.2.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.worlds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.worlds.is_empty()
    }

    /// Load every `*.srpde` file of `dir` (e.g. `data/worlds`); files that fail are logged and skipped.
    pub fn load_dir(dir: &Path) -> Result<Self, WorldError> {
        let mut s = Self::new();
        for e in std::fs::read_dir(dir)? {
            let p = e?.path();
            if p.extension().is_some_and(|x| x == "srpde") {
                match World::load(&p).and_then(|w| {
                    let t0 = w.time;
                    w.for_clients().map(|w| (t0, w))
                }) {
                    Ok((t0, w)) => {
                        if t0 != w.time {
                            log::info!("{}: world time {t0} relabelled {} for the clients (history horizon)", p.display(), w.time);
                        }
                        s.insert(w)
                    }
                    Err(e) => log::error!("{}: {e}", p.display()),
                }
            }
        }
        Ok(s)
    }
}

/// CRC-32 as the client computes the DD level checksum (PROVEN 0x2005EC0C0): reflected 0xEDB88320,
/// init 0xFFFFFFFF, final NOT (= the usual CRC-32), over, for every collision object in list order, the shape data
/// (0x2005EBEC0 on `[obj+0xD0]`) followed by the 64 bytes at `obj+0x10..0x4F` (transform). The server cannot compute
/// it without the level's collision data: the capture tool reads `Client = %x` from the client log; this helper is for
/// a tool that has the objects (or to cross-check a log value).
pub fn collision_checksum<'a>(objects: impl IntoIterator<Item = (&'a [u8], &'a [u8; 64])>) -> u32 {
    let mut h = crc32fast::Hasher::new();
    for (shape, transform) in objects {
        h.update(shape);
        h.update(transform);
    }
    h.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::Decode;

    pub fn sample_item(time: u32, len: usize) -> Vec<u8> {
        let mut v = time.to_le_bytes().to_vec();
        v.extend((0..len.max(18)).map(|i| (i * 31 % 251) as u8));
        v
    }

    /// Header + G1 (count 2 + 6 bytes) + G2 empty + `g3` + G4 empty, with `prefix` bytes between the header and G1.
    fn item_with_g3(time: u32, prefix: &[u8], g3: &[u8]) -> Vec<u8> {
        let mut v = time.to_le_bytes().to_vec();
        v.extend([0u8; 18]);
        v.extend_from_slice(prefix);
        let g1 = [2u8, 0, 0, 0, 0, 0xFF, 0, 0xFF, 7, 7];
        for s in [&g1[..], &[], g3, &[]] {
            v.extend((s.len() as u16).to_le_bytes());
            v.extend_from_slice(s);
        }
        v
    }

    /// The PvE scene environment list of the t = 100 item replaces the t = 50 item's empty one; nothing else changes
    /// (layout as in the PvE captures: list, four u8-counted lists, later rules' tail whose values may differ).
    #[test]
    fn the_scene_env_list_comes_from_the_item_after_the_first_step() {
        let head = [9u8, 8, 7, 6, 5];
        let tail50 = [0u8, 0, 0, 0, 3, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0];
        let tail100 = [0u8, 0, 0, 0, 3, 0xFF, 0, 0, 0, 0, 0xFF, 0xFF, 3, 0];
        let mut g50 = head.to_vec();
        g50.extend([0xFF; 80]);
        g50.extend([0, 0, 0, 0]);
        g50.extend(tail50);
        let mut g100 = head.to_vec();
        g100.extend([0xCF, 0x2B, 0x10, 0x00]);
        g100.extend([0xFF; 76]);
        g100.extend([0, 1, 0xCF, 0x2C, 0x10, 0, 2, 0xE9, 0x2B, 0x10, 0, 0x17, 0x2C, 0x10, 0, 0]);
        g100.extend(tail100);
        let t50 = item_with_g3(50, &[], &g50);
        let t100 = item_with_g3(100, &[3, 0xFF, 0xFF, 3, 0, 0xFF, 0xC4, 0, 0, 0, 6], &g100);
        let w = World::from_item(15, 7, 1, 2, 50, 100, t50.clone()).unwrap().with_scene_env_from(&t100).unwrap();
        let got = w.item().unwrap();
        let at = t50.len() - 2 - tail50.len() - 4 - 80;
        assert_eq!(&got[at..at + 4], &[0xCF, 0x2B, 0x10, 0x00]);
        assert_eq!(&got[at + 4..at + 80], &[0xFF; 76][..]);
        assert_eq!(&got[..at], &t50[..at]);
        assert_eq!(&got[at + 80..], &t50[at + 80..], "the four lists and the tail stay the t = 50 ones");
        // a capture whose list is not empty (not a t = 50 item) is refused
        assert!(World::from_item(15, 7, 1, 2, 50, 100, got).unwrap().with_scene_env_from(&t100).is_err());
    }

    /// A captured t = 50 item goes out at 16 ticks; only the time field changes; a later world is kept as it is.
    #[test]
    fn early_worlds_are_relabelled_past_the_history_horizon() {
        let item = sample_item(50, 300);
        let w = World::from_item(1, 1, 7, 9, 50, 100, item.clone()).unwrap().for_clients().unwrap();
        assert_eq!(w.time, 800);
        let got = w.item().unwrap();
        assert_eq!(&got[..4], &800u32.to_le_bytes());
        assert_eq!(&got[4..], &item[4..]);
        let late = World::from_item(1, 1, 7, 9, 50, 100, sample_item(5000, 300)).unwrap();
        assert_eq!(late.clone().for_clients().unwrap(), late);
        // the LZMA2-stored form too
        let mut z = World::from_item(1, 1, 7, 9, 50, 100, item.clone()).unwrap();
        z.body = lzma2::compress(&item);
        z.already_lzma2 = true;
        let z = z.for_clients().unwrap();
        assert_eq!((z.time, &z.item().unwrap()[..4]), (800, &800u32.to_le_bytes()[..]));
    }

    /// An item like a TDM capture: header, G1 = count + `blocks` + 5 level bytes (2 level objects), G2 empty, G3 = one
    /// head byte, `records` x 5 zero bytes (kd_stats at G3 offset 1), two tail bytes, G4 empty.
    fn item_with(blocks: &[u8], roster: u32, records: usize) -> Vec<u8> {
        let mut v = item_with_g1_only(blocks, roster);
        v.extend([0, 0]);
        let half = (records >> 1) as u8;
        let g3: Vec<u8> = [9u8].into_iter().chain(std::iter::repeat(0).take(5 * records)).chain([1, 2, half, half, 7]).collect();
        v.extend((g3.len() as u16).to_le_bytes());
        v.extend(g3);
        v.extend([0, 0]);
        v
    }

    fn item_with_g1(blocks: &[u8], roster: u32) -> Vec<u8> {
        item_with(blocks, roster, 1)
    }

    fn item_with_g1_only(blocks: &[u8], roster: u32) -> Vec<u8> {
        let mut v = 50u32.to_le_bytes().to_vec();
        v.extend([0xAB; 18]);
        let level = [0xFF, 0x00, 0xFF, 0x11, 0x22];
        let mut g1 = (roster + 2).to_le_bytes().to_vec();
        g1.extend_from_slice(blocks);
        g1.extend_from_slice(&level);
        v.extend((g1.len() as u16).to_le_bytes());
        v.extend(g1);
        v
    }

    #[test]
    fn roster_spec_parses() {
        let r = RosterObjects::from_spec("MG5,MG5|M,,G6").unwrap();
        assert_eq!(r.count(), 1 + 5 + 1 + 5 + 1 + 6);
        assert_eq!(&r.bytes()[..6], &[0, 0, 0, 0, 0, 0xFF]);
        assert_eq!(RosterObjects::from_spec("").unwrap().count(), 0);
        assert!(RosterObjects::from_spec("MX").is_err());
        assert_eq!(RosterObjects::from_spec("G").unwrap().count(), 1, "no count = 1");
        let all = RosterObjects::from_spec("M2T3E1S1H1C1").unwrap();
        assert_eq!(all.count(), 9);
        assert_eq!(all.bytes().len(), 2 * 4 + 3 + 8 + 2 + 2 + 82);
        // the uninitialised bytes of a capture are accepted
        let cap = [0u8, 0, 0x37, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x91];
        assert_eq!(RosterObjects::from_spec("ME").unwrap().prefix_len(&cap), Some(12));
        assert_eq!(RosterObjects::from_spec("EM").unwrap().prefix_len(&cap), None);
    }

    /// The capture's roster (one human `MG5`, the medkit owner byte uninitialised) is swapped for a two-player roster:
    /// count and length follow, the level part and everything after G1 stay byte for byte.
    #[test]
    fn the_capture_roster_is_swapped_for_the_matchs() {
        let cap = [0, 0, 0x61, 0, 0, 0xFF, 0, 0xFF, 0, 0xFF, 0, 0xFF, 0, 0xFF];
        let item = item_with_g1(&cap, 6);
        let w = World::from_item(1, 1, 7, 9, 50, 100, item.clone()).unwrap();
        assert!(w.clone().with_capture_roster(&RosterObjects::from_spec("MG6").unwrap()).is_err(), "not the capture's roster");
        assert!(w.clone().with_capture_roster(&RosterObjects::from_spec("G5M").unwrap()).is_err(), "wrong order");
        let w = w.with_capture_roster(&RosterObjects::from_spec("MG5").unwrap()).unwrap();
        assert_eq!((w.roster_objects, w.roster_bytes, w.records), (6, 14, 1));
        assert!(w.clone().with_record_block(0).is_err(), "the head byte is not zero");
        let w = w.with_record_block(1).unwrap();
        assert!(w.clone().with_team_counts(2).is_err(), "7 is not a team count");
        let w = w.with_team_counts(3).unwrap();
        let again = World::from_file_bytes(&w.to_file_bytes()).unwrap();
        assert_eq!(again, w);

        let two = RosterObjects::from_spec("MG5|MG5").unwrap();
        let m = w.with_roster(&two).unwrap();
        let mut blocks = two.bytes();
        assert_eq!(blocks.len(), 28);
        let want = item_with(&blocks, 12, 2); // team counts 1 / 1
        assert_eq!(m.item().unwrap(), want, "G1 for two players and G3 with two kd_stats records");
        assert_eq!(m.records, 2);
        assert_eq!((m.roster_objects, m.roster_bytes), (12, 28));
        // and back, from the stored LZMA2 form as well
        let mut z = m.clone();
        z.body = lzma2::compress(&m.item().unwrap());
        z.already_lzma2 = true;
        let one = z.with_roster(&RosterObjects::from_spec("MG5").unwrap()).unwrap();
        blocks.truncate(14);
        assert_eq!(one.item().unwrap(), item_with_g1(&blocks, 6));
        // an empty roster leaves only the level
        assert_eq!(w.with_roster(&RosterObjects::from_spec("").unwrap()).unwrap().item().unwrap(), item_with_g1(&[], 0));
        // a world whose block is not located serves only the capture's record count
        let unlocated = World::from_item(1, 1, 7, 9, 50, 100, item.clone()).unwrap().with_capture_roster(&RosterObjects::from_spec("MG5").unwrap()).unwrap();
        assert!(unlocated.with_roster(&RosterObjects::from_spec("MG5").unwrap()).is_ok());
        assert!(unlocated.with_roster(&two).is_err());
    }

    #[test]
    fn file_round_trip() {
        let w = World::from_item(3, 1, 0x1234, 0xCAFEF00D, 50, 100, sample_item(5000, 200)).unwrap();
        let b = w.to_file_bytes();
        assert_eq!(&b[..8], b"SRPDE005");
        assert_eq!(World::from_file_bytes(&b).unwrap(), w);
        // with a project: name and the fixed-roster flag survive
        let pv = w.clone().with_project("pve_01_v2_maze1").unwrap();
        assert!(pv.fixed_roster);
        assert_eq!(World::from_file_bytes(&pv.to_file_bytes()).unwrap(), pv);
        assert!(w.clone().with_project("").is_ok_and(|x| !x.fixed_roster));
        assert!(w.clone().with_project(&"x".repeat(64)).is_err());
        // an SRPDE004 file (no flags, no project) still loads
        let mut old = b"SRPDE004".to_vec();
        old.extend_from_slice(&b[8..52]);
        old.extend_from_slice(&b[120..]);
        let o = World::from_file_bytes(&old).unwrap();
        assert_eq!((o.project.as_str(), o.fixed_roster), ("", false));
        assert_eq!(o, w);
        // DE payload: time, compressed, body
        let de = w.de_payload(Packing::Raw).unwrap();
        assert_eq!(&de[..5], &[0x88, 0x13, 0, 0, 0]);
        assert_eq!(&de[5..], &w.body[..]);
        let de = w.de_payload(Packing::StoredLzma2).unwrap();
        assert_eq!(de[4], 1);
        assert_eq!(WorldMsg::from_bytes(&de).unwrap().unpack().unwrap(), w.body);
    }

    #[test]
    fn already_packed_body() {
        let item = sample_item(100, 64);
        let mut w = World::from_item(1, 1, 7, 9, 50, 100, item.clone()).unwrap();
        w.body = lzma2::compress(&item);
        w.already_lzma2 = true;
        let again = World::from_file_bytes(&w.to_file_bytes()).unwrap();
        assert_eq!(again.item().unwrap(), item);
        // sent as it is
        assert_eq!(again.de_payload(Packing::Raw).unwrap()[4], 1);
    }

    #[test]
    fn rejects_bad_files() {
        assert!(World::from_file_bytes(b"short").is_err());
        let w = World::from_item(1, 1, 7, 9, 50, 100, sample_item(100, 64)).unwrap();
        let mut b = w.to_file_bytes();
        b[28] = 0; // tick_ms 0
        assert!(World::from_file_bytes(&b).is_err());
        let mut b = w.to_file_bytes();
        b[16] = 0x55; // header time != item time
        assert!(World::from_file_bytes(&b).is_err());
        let mut b = w.to_file_bytes();
        b.truncate(b.len() - 1);
        assert!(World::from_file_bytes(&b).is_err());
        assert!(World::from_item(1, 1, 7, 9, 50, 100, sample_item(1, DE_MAX_BODY + 10)).is_err());
    }

    #[test]
    fn store_lookup() {
        let mut s = WorldStore::new();
        s.insert(World::from_item(3, 1, 1, 2, 50, 100, sample_item(50, 30)).unwrap());
        assert!(s.get(3, 1).is_some());
        assert!(s.get(3, 2).is_none());
        // the six PvE projects share (15, 7): one world each, by project
        for (k, p) in ["pve_01_v1_maze1", "pve_01_v1_maze2"].iter().enumerate() {
            s.insert(World::from_item(15, 7, 10 + k as u32, 2, 50, 100, sample_item(50, 30)).unwrap().with_project(p).unwrap());
        }
        assert_eq!(s.get_project(15, 7, "pve_01_v1_maze2").unwrap().match_id, 11);
        assert!(s.get_project(15, 7, "pve_01_v2_maze1").is_none());
        assert_eq!(s.get(15, 7).unwrap().match_id, 10, "any project of (map, mode)");
        assert_eq!(s.projects(15, 7), ["pve_01_v1_maze1", "pve_01_v1_maze2"]);
        assert_eq!(s.len(), 3);
    }

    /// A PvE world serves only its capture's record count: the human's objects may change, another count is refused.
    #[test]
    fn a_fixed_roster_world_refuses_another_record_count() {
        let cap = [0, 0, 0, 0, 0, 0xFF, 0, 0xFF, 0, 0xFF, 0, 0xFF, 0, 0xFF];
        let w = World::from_item(15, 7, 7, 9, 50, 100, item_with_g1(&cap, 6)).unwrap().with_capture_roster(&RosterObjects::from_spec("MG5").unwrap()).unwrap();
        let w = w.with_project("pve_01_v1_maze1").unwrap();
        assert!(w.clone().with_record_block(1).is_err(), "no kd_stats block for a PvE world");
        assert!(w.with_roster(&RosterObjects::from_spec("M2G5").unwrap()).is_ok(), "same record count, other objects");
        let e = w.with_roster(&RosterObjects::from_spec("MG5|MG5").unwrap()).unwrap_err().to_string();
        assert!(e.contains("pve_01_v1_maze1") && e.contains("1 records") && e.contains("has 2"), "{e}");
    }

    #[test]
    fn checksum_is_plain_crc32() {
        // CRC-32 of "123456789" is 0xCBF43926; the shape bytes and the 64 transform bytes are just concatenated
        let t = [0u8; 64];
        let one = collision_checksum([(&b"123456789"[..], &t)]);
        let mut h = crc32fast::Hasher::new();
        h.update(b"123456789");
        h.update(&t);
        assert_eq!(one, h.finalize());
        assert_eq!(crc32fast::hash(b"123456789"), 0xCBF43926);
    }
}

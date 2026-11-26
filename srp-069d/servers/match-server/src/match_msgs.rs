//! Payload codecs of the 0.69d MATCH connection (ids C9..D4 C->S, DC..E5 S->C). `crates/protocol` only has the
//! lobby messages; these are the match ones. Each struct is the bytes AFTER the `id [x]` header (the transport,
//! [`crate::mtransport`], adds that), with `Encode` / `Decode` of `protocol`.
//!
//! Evidence tags: PROVEN = layout read in the client code; LIKELY = layout proven but the meaning or a unit is
//! inferred; UNVERIFIED = no evidence beyond analogy. The unit tests check every codec against byte examples derived
//! from the client's reader and writer and against itself.

use protocol::{lzma2, Decode, Encode, Error, Reader, Result, Writer};

/// Players per match (client limit, `n_players` max 20).
pub const MAX_PLAYERS: usize = 20;

fn bit_count(m: u32) -> usize {
    m.count_ones() as usize
}

// ---------------------------------------------------------------------------------------------------------------
// C->S

/// C9, the first datagram: `u32 order_id, u32 session, u8 debug, u8 0` (10 bytes; plain, reliable).
/// PROVEN (built at 0x2001F06E6). `order_id` = last u32 of the lobby's 0x82 (our per-player token);
/// `session` = login session (`nc+0x1138`, first u32 of login 0x0B); `debug` 00, or FF with debug option 0x200AB69D8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connect {
    pub order_id: u32,
    pub session: u32,
    pub debug: u8,
    pub reserved: u8,
}

impl Encode for Connect {
    fn encode(&self, w: &mut Writer) {
        w.u32(self.order_id).u32(self.session).u8(self.debug).u8(self.reserved);
    }
}
impl Decode for Connect {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(Self { order_id: r.u32()?, session: r.u32()?, debug: r.u8()?, reserved: r.u8()? })
    }
}

/// CA, port probe: `u32 order_id` (plain, reliable). PROVEN id and argument; the client sends one per port of the
/// DC range (LIKELY, = 0.26g).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortProbe {
    pub order_id: u32,
}
impl Encode for PortProbe {
    fn encode(&self, w: &mut Writer) {
        w.u32(self.order_id);
    }
}
impl Decode for PortProbe {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(Self { order_id: r.u32()? })
    }
}

/// `profile_input` (writer 0x2003278E0, reader 0x200327B30): `u8 a, u8 b, u8 c, u8 d, u8 n (<= 12), n x {u32, u8, u8, u8}`
/// = 5 + 7n bytes. PROVEN layout. Meaning LIKELY: kit choice + per-item choices. In CB bytes 0 and 1 are 0, byte 2 =
/// global 0x20F43FC6A, byte 3 = global 0x200A9CA4C; the server only relays it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ProfileInput {
    pub head: [u8; 4],
    pub entries: Vec<(u32, u8, u8, u8)>,
}

impl Encode for ProfileInput {
    fn encode(&self, w: &mut Writer) {
        if self.entries.len() > 12 {
            w.fail(Error::Limit("profile_input entries (max 12)"));
        }
        w.bytes(&self.head).u8(self.entries.len() as u8);
        for (a, b, c, d) in &self.entries {
            w.u32(*a).u8(*b).u8(*c).u8(*d);
        }
    }
}
impl Decode for ProfileInput {
    fn decode(r: &mut Reader) -> Result<Self> {
        let head = r.array::<4>()?;
        let n = usize::from(r.u8()?);
        if n > 12 {
            return Err(Error::Limit("profile_input entries (max 12)"));
        }
        let mut entries = Vec::with_capacity(n);
        for _ in 0..n {
            entries.push((r.u32()?, r.u8()?, r.u8()?, r.u8()?));
        }
        Ok(Self { head, entries })
    }
}

/// `player_input` (reader 0x2003276E0), 11 bytes: `u16, u16` (rotation), then 7 bytes big-endian = 56-bit action mask.
/// PROVEN sizes and the big-endian mask; little-endian u16 is LIKELY (the plain reader of the client is LE).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct PlayerInput {
    pub rot: [u16; 2],
    /// Only the low 56 bits are on the wire.
    pub actions: u64,
}

impl PlayerInput {
    pub const LEN: usize = 11;
}
impl Encode for PlayerInput {
    fn encode(&self, w: &mut Writer) {
        w.u16(self.rot[0]).u16(self.rot[1]);
        w.bytes(&self.actions.to_be_bytes()[1..8]);
    }
}
impl Decode for PlayerInput {
    fn decode(r: &mut Reader) -> Result<Self> {
        let rot = [r.u16()?, r.u16()?];
        let b = r.array::<7>()?;
        let mut full = [0u8; 8];
        full[1..].copy_from_slice(&b);
        Ok(Self { rot, actions: u64::from_be_bytes(full) })
    }
}

/// CB, ready after the level load and the first profile input: `u8 flag (00, FF with option 0x200AE6560), profile_input`.
/// PROVEN (0x2001F1360).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Ready {
    pub flag: u8,
    pub profile: ProfileInput,
}
impl Encode for Ready {
    fn encode(&self, w: &mut Writer) {
        w.u8(self.flag);
        self.profile.encode(w);
    }
}
impl Decode for Ready {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(Self { flag: r.u8()?, profile: ProfileInput::decode(r)? })
    }
}

/// CC, `send_query_dynamic_match_info` = resync request (plain, send once, 6 bytes):
/// `u8 tick (time / tick_ms), u8[3] mask of players that differ, u8 global differs (00/FF), u8 reason`.
/// PROVEN layout (0x2001AE770). The reason is ONE value, not bit flags (CC sender 0x2001AE98F, PROVEN):
/// 4 = a mismatch with every DF present and equal presence masks, 2 = a mismatch beyond the contiguous DFs
/// (`nc+0x7A6990` set), 8 = a mismatch while the presence masks differ (always the case with a mask-0 E0 while players
/// are present), 1 = forced (a DE that did not land). `global` 0xFF = g0 or g1 differ. Every value but 1 is a real
/// mismatch ([`QueryWorld::reason_name`]).
/// The mask byte order (byte 0 = players 0..7) is LIKELY. Only exists after the server sent an E0 that differs (or
/// the forced flag after a DE), so with E0 off it should never arrive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryWorld {
    pub tick: u8,
    pub players: u32,
    pub global: u8,
    pub reason: u8,
}
impl QueryWorld {
    /// The reason value's meaning (one value, see the type's docs).
    pub fn reason_name(reason: u8) -> &'static str {
        match reason {
            1 => "forced: a DE did not land",
            2 => "mismatch beyond the contiguous DFs",
            4 => "mismatch, every DF present, masks equal",
            8 => "mismatch, presence masks differ (always with a mask-0 E0)",
            _ => "unknown reason value",
        }
    }
}

impl Encode for QueryWorld {
    fn encode(&self, w: &mut Writer) {
        w.u8(self.tick).bytes(&self.players.to_le_bytes()[..3]).u8(self.global).u8(self.reason);
    }
}
impl Decode for QueryWorld {
    fn decode(r: &mut Reader) -> Result<Self> {
        let tick = r.u8()?;
        let m = r.array::<3>()?;
        Ok(Self {
            tick,
            players: u32::from_le_bytes([m[0], m[1], m[2], 0]),
            global: r.u8()?,
            reason: r.u8()?,
        })
    }
}

/// CD, client game update (plain, cfg lifetime; writer 0x20032EFD0), one per tick with something to send. PROVEN:
/// `u32 time, u8 n_late, u8 n_now, u8 n_profile, u8 n_ping, n_late x {u8 dt, player_input}, n_now x player_input,
/// n_profile x profile_input, n_ping x u16`. `time` is the tick-aligned client clock. LIKELY: the unit of `dt`
/// is ticks (an input changed after it was sent, valid at `time - dt`); a client has one player so n_now is 0 or 1.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ClientUpdate {
    pub time: u32,
    pub late: Vec<(u8, PlayerInput)>,
    pub now: Vec<PlayerInput>,
    pub profile: Vec<ProfileInput>,
    pub ping: Vec<u16>,
}
impl Encode for ClientUpdate {
    fn encode(&self, w: &mut Writer) {
        w.u32(self.time)
            .count_u8(self.late.len(), "CD late")
            .count_u8(self.now.len(), "CD now")
            .count_u8(self.profile.len(), "CD profile")
            .count_u8(self.ping.len(), "CD ping");
        for (dt, i) in &self.late {
            w.u8(*dt);
            i.encode(w);
        }
        for i in &self.now {
            i.encode(w);
        }
        for p in &self.profile {
            p.encode(w);
        }
        for p in &self.ping {
            w.u16(*p);
        }
    }
}
impl Decode for ClientUpdate {
    fn decode(r: &mut Reader) -> Result<Self> {
        let time = r.u32()?;
        let (nl, nn, np, ng) = (r.u8()?, r.u8()?, r.u8()?, r.u8()?);
        let mut late = Vec::new();
        for _ in 0..nl {
            late.push((r.u8()?, PlayerInput::decode(r)?));
        }
        let mut now = Vec::new();
        for _ in 0..nn {
            now.push(PlayerInput::decode(r)?);
        }
        let mut profile = Vec::new();
        for _ in 0..np {
            profile.push(ProfileInput::decode(r)?);
        }
        let mut ping = Vec::new();
        for _ in 0..ng {
            ping.push(r.u16()?);
        }
        Ok(Self { time, late, now, profile, ping })
    }
}

/// D4, integrity flags (plain, once): `u32` flags, sent every 10 s when non-zero. PROVEN. The server ignores it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Integrity {
    pub flags: u32,
}
impl Encode for Integrity {
    fn encode(&self, w: &mut Writer) {
        w.u32(self.flags);
    }
}
impl Decode for Integrity {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(Self { flags: r.u32()? })
    }
}

// CE (leave) has an empty payload, CF/D0/D2/D3 are ignored by this server.

// ---------------------------------------------------------------------------------------------------------------
// S->C

/// DC, ports (plain, reliable, 4 bytes): `u16 min_port, u16 max_port`; the client probes every port of the range with
/// CA and uses a random one (LIKELY), so send `min = max = server port`. First S->C message of a connection, PROVEN
/// (anything else first = "connection forbidden" + "game: invalid session id"). Example: `DC D4 04 D4 04` = port 1236.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ports {
    pub min: u16,
    pub max: u16,
}
impl Encode for Ports {
    fn encode(&self, w: &mut Writer) {
        w.u16(self.min).u16(self.max);
    }
}
impl Decode for Ports {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(Self { min: r.u16()?, max: r.u16()? })
    }
}

/// DD, static match info (unique; taken only in client state 1 = after DC): `u8 local_player_index, u32 level_checksum`,
/// then raw LZMA2 (to the end of the message) of `match_options`. PROVEN (0x2001F3791). The checksum must
/// equal the client's own CRC-32 of the loaded collision world (fatal assertion otherwise, PROVEN check / LIKELY fatal in
/// release): take `Client = %x` from the client log per map. The options blob is the one of the lobby's `83 5B`
/// (layout: local-match `options.rs`); this server treats it as opaque bytes built by the lobby.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticInfo {
    pub local_player_index: u8,
    pub level_checksum: u32,
    /// Raw LZMA2 stream (stored chunks are valid, LIKELY).
    pub options_lzma2: Vec<u8>,
}
impl StaticInfo {
    pub fn new(local_player_index: u8, level_checksum: u32, raw_options: &[u8]) -> Self {
        Self { local_player_index, level_checksum, options_lzma2: lzma2::compress(raw_options) }
    }
    pub fn options(&self) -> Result<Vec<u8>> {
        lzma2::decompress(&self.options_lzma2)
    }
}
impl Encode for StaticInfo {
    fn encode(&self, w: &mut Writer) {
        w.u8(self.local_player_index).u32(self.level_checksum).bytes(&self.options_lzma2);
    }
}
impl Decode for StaticInfo {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(Self { local_player_index: r.u8()?, level_checksum: r.u32()?, options_lzma2: r.rest().to_vec() })
    }
}

/// How the DE body is packed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packing {
    /// `compressed = 0`, the serialized item as it is. UNVERIFIED (the client's raw branch was not traced).
    Raw,
    /// `compressed = 1`, raw LZMA2 made of stored chunks (valid LZMA2; the 0.26g way). LIKELY.
    StoredLzma2,
}

/// Decoded size limit of a DE body (client buffer, 0x2001AFBE4/0x2001AFBF9).
pub const DE_MAX_BODY: usize = 0x44800;

/// DE, dynamic match info = world snapshot (unique, lifetime 1900 ms / resend 100 ms): `u32 time, u8 compressed, body`.
/// PROVEN (0x2001AF985). `time` must equal the `u32 time` at the start of the serialized item (checked at
/// 0x2001AFC99). `body` = one serialized history item as written by the item writer 0x2002FC5A0 with full = 1
/// raw LZMA2 when `compressed != 0`, decoded size <= [`DE_MAX_BODY`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldMsg {
    pub time: u32,
    pub compressed: u8,
    pub body: Vec<u8>,
}
impl WorldMsg {
    /// Build from the serialized item `raw` (its first 4 bytes are the item time).
    pub fn pack(time: u32, raw: &[u8], packing: Packing) -> Result<Self> {
        if raw.len() > DE_MAX_BODY {
            return Err(Error::Limit("DE body over 0x44800 bytes"));
        }
        Ok(match packing {
            Packing::Raw => Self { time, compressed: 0, body: raw.to_vec() },
            Packing::StoredLzma2 => Self { time, compressed: 1, body: lzma2::compress(raw) },
        })
    }
    /// The serialized item.
    pub fn unpack(&self) -> Result<Vec<u8>> {
        if self.compressed == 0 { Ok(self.body.clone()) } else { lzma2::decompress(&self.body) }
    }
}
impl Encode for WorldMsg {
    fn encode(&self, w: &mut Writer) {
        w.u32(self.time).u8(self.compressed).bytes(&self.body);
    }
}
impl Decode for WorldMsg {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(Self { time: r.u32()?, compressed: r.u8()?, body: r.rest().to_vec() })
    }
}

/// DF, game update = the inputs of ONE tick (plain, cfg lifetime; reader 0x20032E790, handler 0x2001AFE30). PROVEN layout:
///
/// ```text
/// u32 time
/// u8  code                 0xFF normally (read when the local player is in leave_mask; LIKELY leave reason; 2 skips UI clean-up)
/// u32 enter_mask           bit i: enter_match(i, time)
/// u32 leave_mask           bit i: leave_match(i, time); the local player's bit closes the match (close_current_match(3))
/// u8 n_late, u8 n_profile, u8 n_ping, u8 n_ai, u8 n_conf
/// n_conf x {u32 a, u32 b}  server_input_confirmation (LIKELY: a = the client's CD stamp, b = server time; b = a rule LIKELY)
/// n_late x {u8 dt, u8 player}   an input of `player` at time - dt  (PROVEN 0x20032ED00: time = T - dt, dt in milliseconds, no scaling)
/// u32 input_mask           bit i: player i has an input at `time`
/// inputs: n_late then popcount(input_mask) x player_input (11 B each)   (bit-packed form when opt.packed_inputs: NOT supported)
/// n_profile x {u8 player, profile_input}
/// n_ping x {u8 player, u16}
/// n_ai x {u8 player, u16, u16}
/// ```
/// Smallest update (22 bytes): `<u32 time> FF 00000000 00000000 00 00 00 00 00 00000000` (PROVEN, from the layout).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameUpdate {
    pub time: u32,
    pub code: u8,
    pub enter_mask: u32,
    pub leave_mask: u32,
    pub conf: Vec<(u32, u32)>,
    pub late: Vec<(u8, u8)>,
    pub input_mask: u32,
    /// `late.len()` late inputs first, then one per set bit of `input_mask` (lowest bit first). LIKELY order.
    pub inputs: Vec<PlayerInput>,
    pub profile: Vec<(u8, ProfileInput)>,
    pub ping: Vec<(u8, u16)>,
    pub ai: Vec<(u8, u16, u16)>,
}

impl GameUpdate {
    /// An empty update for `time`.
    pub fn empty(time: u32) -> Self {
        Self {
            time,
            code: 0xFF,
            enter_mask: 0,
            leave_mask: 0,
            conf: vec![],
            late: vec![],
            input_mask: 0,
            inputs: vec![],
            profile: vec![],
            ping: vec![],
            ai: vec![],
        }
    }
}
impl Encode for GameUpdate {
    fn encode(&self, w: &mut Writer) {
        if self.inputs.len() != self.late.len() + bit_count(self.input_mask) {
            w.fail(Error::Invalid("DF inputs != late + popcount(input_mask)"));
        }
        w.u32(self.time).u8(self.code).u32(self.enter_mask).u32(self.leave_mask);
        w.count_u8(self.late.len(), "DF late")
            .count_u8(self.profile.len(), "DF profile")
            .count_u8(self.ping.len(), "DF ping")
            .count_u8(self.ai.len(), "DF ai")
            .count_u8(self.conf.len(), "DF conf");
        for (a, b) in &self.conf {
            w.u32(*a).u32(*b);
        }
        for (dt, p) in &self.late {
            w.u8(*dt).u8(*p);
        }
        w.u32(self.input_mask);
        for i in &self.inputs {
            i.encode(w);
        }
        for (p, pi) in &self.profile {
            w.u8(*p);
            pi.encode(w);
        }
        for (p, v) in &self.ping {
            w.u8(*p).u16(*v);
        }
        for (p, a, b) in &self.ai {
            w.u8(*p).u16(*a).u16(*b);
        }
    }
}
impl Decode for GameUpdate {
    fn decode(r: &mut Reader) -> Result<Self> {
        let time = r.u32()?;
        let code = r.u8()?;
        let enter_mask = r.u32()?;
        let leave_mask = r.u32()?;
        let (nl, np, ng, na, nc) = (r.u8()?, r.u8()?, r.u8()?, r.u8()?, r.u8()?);
        let mut conf = Vec::new();
        for _ in 0..nc {
            conf.push((r.u32()?, r.u32()?));
        }
        let mut late = Vec::new();
        for _ in 0..nl {
            late.push((r.u8()?, r.u8()?));
        }
        let input_mask = r.u32()?;
        let mut inputs = Vec::new();
        for _ in 0..usize::from(nl) + bit_count(input_mask) {
            inputs.push(PlayerInput::decode(r)?);
        }
        let mut profile = Vec::new();
        for _ in 0..np {
            profile.push((r.u8()?, ProfileInput::decode(r)?));
        }
        let mut ping = Vec::new();
        for _ in 0..ng {
            ping.push((r.u8()?, r.u16()?));
        }
        let mut ai = Vec::new();
        for _ in 0..na {
            ai.push((r.u8()?, r.u16()?, r.u16()?));
        }
        Ok(Self { time, code, enter_mask, leave_mask, conf, late, input_mask, inputs, profile, ping, ai })
    }
}

/// E0, server hash of one tick (plain, lifetime 40 ms): `u8 tick, u8[3] player mask, one u8 per set bit (player hash),
/// u8, u8, u8 (global state)`. PROVEN layout (0x2001AE040); the hashes are CRC-16/ARC low bytes of the client's own
/// serialization. Sent only from a world host's Tick frames (relay `on_host_tick`);
/// a wrong E0 causes CC storms. Mask byte order little endian: PROVEN, the reader tests byte `+0x14 + (i >> 3)`,
/// bit `i & 7` for player i (0x2001AE0F0).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hash {
    pub tick: u8,
    pub players: u32,
    pub hashes: Vec<u8>,
    pub globals: [u8; 3],
}
impl Encode for Hash {
    fn encode(&self, w: &mut Writer) {
        if self.hashes.len() != bit_count(self.players & 0x00FF_FFFF) {
            w.fail(Error::Invalid("E0 hashes != popcount(mask)"));
        }
        w.u8(self.tick).bytes(&self.players.to_le_bytes()[..3]).bytes(&self.hashes).bytes(&self.globals);
    }
}
impl Decode for Hash {
    fn decode(r: &mut Reader) -> Result<Self> {
        let tick = r.u8()?;
        let m = r.array::<3>()?;
        let players = u32::from_le_bytes([m[0], m[1], m[2], 0]);
        let hashes = r.bytes(bit_count(players))?.to_vec();
        Ok(Self { tick, players, hashes, globals: r.array::<3>()? })
    }
}

/// E2, 8 bytes stored at `nc+0x18` (ordered + unique, fixed 8 bytes). PROVEN handler 0x2001EE3AF. It is the scoreboard's
/// per-player activity mask, 2 bits per player (`(nc+0x18 >> 2i) & 3`, 0 = shows the place, 1/2/3 = other states,
/// 0x2001A3684), not a time. The client never initialises it: the relay sends 0 after the DE (else a player can show
/// as "disconnected").
#[derive(Clone, Debug, PartialEq)]
pub struct ServerTime {
    pub value: f64,
}
impl Encode for ServerTime {
    fn encode(&self, w: &mut Writer) {
        w.f64(self.value);
    }
}
impl Decode for ServerTime {
    fn decode(r: &mut Reader) -> Result<Self> {
        Ok(Self { value: r.f64()? })
    }
}

/// E4, kick with text (unique, length by content): `u8 kind (<= 5), u8 n, n bytes`. PROVEN (`process_anti_cheat_kick`
/// 0x2001F2FF0). Meaning of the kinds and the text encoding UNVERIFIED.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kick {
    pub kind: u8,
    pub text: Vec<u8>,
}
impl Encode for Kick {
    fn encode(&self, w: &mut Writer) {
        if self.kind > 5 {
            w.fail(Error::Invalid("E4 kind > 5"));
        }
        w.u8(self.kind).count_u8(self.text.len(), "E4 text").bytes(&self.text);
    }
}
impl Decode for Kick {
    fn decode(r: &mut Reader) -> Result<Self> {
        let kind = r.u8()?;
        let n = usize::from(r.u8()?);
        Ok(Self { kind, text: r.bytes(n)?.to_vec() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.split_whitespace().collect();
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    fn round<T: Encode + Decode + PartialEq + std::fmt::Debug>(v: T, bytes: &str) {
        let b = hex(bytes);
        assert_eq!(v.to_bytes().unwrap(), b, "encode");
        assert_eq!(T::from_bytes(&b).unwrap(), v, "decode");
    }

    #[test]
    fn connect_c9() {
        // C9 <u32 order_id> <u32 session> <u8 dbg: 00, or FF> 00
        round(Connect { order_id: 0x12345678, session: 0xDEADBEEF, debug: 0, reserved: 0 }, "78563412 efbeadde 00 00");
        round(Connect { order_id: 1, session: 2, debug: 0xFF, reserved: 0 }, "01000000 02000000 ff 00");
    }

    #[test]
    fn ports_dc_example() {
        // `DC D4 04 D4 04` for port 1236
        round(Ports { min: 1236, max: 1236 }, "d404 d404");
    }

    #[test]
    fn probe_ca() {
        round(PortProbe { order_id: 0xAABBCCDD }, "ddccbbaa");
    }

    #[test]
    fn smallest_df_is_22_bytes() {
        // the smallest DF: `<u32 time> FF 00000000 00000000 00 00 00 00 00 00000000`
        let df = GameUpdate::empty(1000);
        let b = df.to_bytes().unwrap();
        assert_eq!(b.len(), 22);
        assert_eq!(b, hex("e8030000 ff 00000000 00000000 00 00 00 00 00 00000000"));
        assert_eq!(GameUpdate::from_bytes(&b).unwrap(), df);
    }

    #[test]
    fn df_with_everything_round_trips() {
        let i = |a: u64| PlayerInput { rot: [0x1234, 0x5678], actions: a };
        let df = GameUpdate {
            time: 5000,
            code: 0xFF,
            enter_mask: 0b11,
            leave_mask: 0b100,
            conf: vec![(4950, 4950)],
            late: vec![(2, 1)],
            input_mask: 0b101,
            inputs: vec![i(1), i(2), i(3)],
            profile: vec![(0, ProfileInput { head: [0, 0, 1, 2], entries: vec![(7, 1, 2, 3)] })],
            ping: vec![(1, 80)],
            ai: vec![(2, 1, 2)],
        };
        let b = df.to_bytes().unwrap();
        // 22 + conf 8 + late 2 + 3 inputs 33 + profile (1 + 5 + 7) + ping 3 + ai 5
        assert_eq!(b.len(), 22 + 8 + 2 + 33 + 13 + 3 + 5);
        assert_eq!(GameUpdate::from_bytes(&b).unwrap(), df);
        // inputs count must match
        let mut bad = df.clone();
        bad.inputs.pop();
        assert!(bad.to_bytes().is_err());
    }

    #[test]
    fn player_input_layout() {
        // u16 u16 LE, 7 bytes big endian action mask
        round(PlayerInput { rot: [0x0102, 0x0304], actions: 0x0A0B0C0D0E0F10 }, "0201 0403 0a0b0c0d0e0f10");
        // only 56 bits go out
        let p = PlayerInput { rot: [0, 0], actions: 0xFF00_0000_0000_0001 };
        assert_eq!(PlayerInput::from_bytes(&p.to_bytes().unwrap()).unwrap().actions, 1);
    }

    #[test]
    fn profile_input_is_5_plus_7n() {
        round(ProfileInput { head: [0, 0, 3, 4], entries: vec![] }, "00000304 00");
        let p = ProfileInput { head: [0, 0, 3, 4], entries: vec![(0x11223344, 1, 2, 3), (5, 6, 7, 8)] };
        assert_eq!(p.to_bytes().unwrap().len(), 5 + 14);
        assert_eq!(ProfileInput::from_bytes(&p.to_bytes().unwrap()).unwrap(), p);
        assert!(ProfileInput { head: [0; 4], entries: vec![(0, 0, 0, 0); 13] }.to_bytes().is_err());
        assert!(ProfileInput::from_bytes(&hex("00000000 0d")).is_err());
    }

    #[test]
    fn ready_cb() {
        round(Ready { flag: 0, profile: ProfileInput { head: [0, 0, 1, 2], entries: vec![] } }, "00 00000102 00");
    }

    #[test]
    fn query_cc_is_6_bytes() {
        // u8 tick, u8[3] mask, u8 global, u8 reason
        round(QueryWorld { tick: 7, players: 0b101, global: 0xFF, reason: 8 }, "07 050000 ff 08");
    }

    #[test]
    fn cc_reason_is_one_value() {
        // 0x2001AE98F: one value; 8 and 4 are mismatches, 1 forced; no bit decoding (12 is not "4 and 8")
        assert!(QueryWorld::reason_name(8).starts_with("mismatch"));
        assert!(QueryWorld::reason_name(1).starts_with("forced"));
        assert_eq!(QueryWorld::reason_name(12), "unknown reason value");
    }

    #[test]
    fn cd_round_trip_and_layout() {
        let i = PlayerInput { rot: [1, 2], actions: 3 };
        let cd = ClientUpdate { time: 100, late: vec![(1, i)], now: vec![i], profile: vec![], ping: vec![55] };
        let b = cd.to_bytes().unwrap();
        // u32 + 4 counts + (1 + 11) + 11 + 2
        assert_eq!(b.len(), 4 + 4 + 12 + 11 + 2);
        assert_eq!(&b[..8], &hex("64000000 01 01 00 01")[..]);
        assert_eq!(ClientUpdate::from_bytes(&b).unwrap(), cd);
        // the idle CD of a client without changes: time + four zero counts
        assert_eq!(ClientUpdate { time: 50, ..Default::default() }.to_bytes().unwrap(), hex("32000000 00000000"));
    }

    #[test]
    fn static_info_dd() {
        let opts = vec![1u8, 2, 3, 4, 5];
        let dd = StaticInfo::new(1, 0xCAFEBABE, &opts);
        let b = dd.to_bytes().unwrap();
        assert_eq!(&b[..5], &hex("01 bebafeca")[..]);
        let back = StaticInfo::from_bytes(&b).unwrap();
        assert_eq!(back.local_player_index, 1);
        assert_eq!(back.level_checksum, 0xCAFEBABE);
        assert_eq!(back.options().unwrap(), opts);
    }

    #[test]
    fn world_de_pack_both_ways() {
        let item: Vec<u8> = (0..3000u32).map(|i| (i % 251) as u8).collect();
        for packing in [Packing::Raw, Packing::StoredLzma2] {
            let de = WorldMsg::pack(1234, &item, packing).unwrap();
            let b = de.to_bytes().unwrap();
            assert_eq!(&b[..4], &1234u32.to_le_bytes());
            assert_eq!(b[4], u8::from(packing == Packing::StoredLzma2));
            assert_eq!(WorldMsg::from_bytes(&b).unwrap().unpack().unwrap(), item);
        }
        assert!(WorldMsg::pack(0, &vec![0; DE_MAX_BODY + 1], Packing::Raw).is_err());
    }

    #[test]
    fn e0_hash_layout() {
        let h = Hash { tick: 3, players: 0b1010, hashes: vec![0xAA, 0xBB], globals: [1, 2, 3] };
        round(h, "03 0a0000 aabb 010203");
        assert!(Hash { tick: 0, players: 1, hashes: vec![], globals: [0; 3] }.to_bytes().is_err());
    }

    #[test]
    fn e2_and_e4() {
        round(ServerTime { value: 1.0 }, "000000000000f03f");
        round(Kick { kind: 1, text: b"ab".to_vec() }, "01 02 6162");
        assert!(Kick { kind: 6, text: vec![] }.to_bytes().is_err());
    }
}

//! The 0.26g match messages and the pure match logic around them
//! (`docs/notes/match.md`, `transport.md` §4). No sockets and no
//! server loop: the match server feeds these with what the transport
//! delivers and sends what they write.
//!
//! A message here is its **payload**: the bytes after the id and, for the
//! unique ids (0xE4..=0xE8, 0xF6), after the transport's unique-id byte. The
//! transport adds the id, that byte, a length prefix where the id is
//! variable, and splits what does not fit a datagram.
//!
//! - this file: ids, per-id lengths, the 1-byte tick times, every
//!   client→server parser and server→client writer, the sync generation and
//!   the time-sync arithmetic;
//! - [`static_info`]: the match options of 0xE4;
//! - [`snapshot`]: the world snapshot's sections, its CRC, the statistics
//!   and rules blobs;
//! - [`upload`]: the patched client's snapshot upload (0xE0 chunks, 0xF6);
//! - [`rules`]: when a match starts, ends and who wins;
//! - [`clock`]: how fast a game's clock takes a correction, by its build;
//! - [`client_model`]: what a client accepts, for the server's tests.
//!
//! Everything is PROVEN in the notes (read in the 0.26g exe) unless a
//! comment says LIKELY or GUESS. Nothing was checked against a running
//! 0.26g match yet.

pub mod client_model;
pub mod clock;
pub mod late_join;
pub mod motion;
pub mod players;
pub mod rules;
pub mod snapshot;
pub mod static_info;
pub mod upload;

use client_model::COMPRESSED_SNAPSHOT_BUFFER;
use upload::{Chunk, ChunkError};

use super::lzma2;

/// The simulation's tick: every time on the wire but 0xE5's is a tick.
pub const TICK_MS: u32 = 50;
/// `player_profile[20]` in the match options.
pub const MAX_PLAYERS: usize = 20;

/// Message ids (`match.md` §1).
pub mod id {
    // Client to server, 0xD8..=0xE1.
    pub const CONNECT: u8 = 0xD8;
    pub const PORT_PROBE: u8 = 0xD9;
    pub const READY: u8 = 0xDA;
    pub const TIME_REPLY: u8 = 0xDB;
    pub const INPUT: u8 = 0xDC;
    pub const INPUT_PRECISE: u8 = 0xDD;
    pub const PROFILE_INPUT: u8 = 0xDE;
    pub const HASH: u8 = 0xDF;
    /// Empty: the client leaves. With a payload: a patched client's
    /// snapshot chunk ([`super::upload`]).
    pub const LEAVE: u8 = 0xE0;
    pub const KEEP_ALIVE: u8 = 0xE1;

    // Server to client, 0xE3..=0xF6. 0xE2 is no message: never send it.
    pub const PORTS: u8 = 0xE3;
    pub const STATIC_INFO: u8 = 0xE4;
    pub const SET_TIME: u8 = 0xE5;
    pub const PING: u8 = 0xE6;
    pub const CLOCK_OFFSET: u8 = 0xE7;
    pub const SNAPSHOT: u8 = 0xE8;
    pub const PLAYER_ENTERED: u8 = 0xE9;
    pub const PLAYER_LEFT: u8 = 0xEA;
    pub const INPUT_RELAY: u8 = 0xEB;
    pub const INPUT_RELAY_PRECISE: u8 = 0xEC;
    pub const INPUTS_RELAY: u8 = 0xED;
    pub const INPUTS_RELAY_PRECISE: u8 = 0xEE;
    pub const MOVE_INPUT: u8 = 0xEF;
    pub const MOVE_INPUT_PRECISE: u8 = 0xF0;
    pub const DISCARD_INPUT: u8 = 0xF1;
    pub const DISCARD_INPUT_PRECISE: u8 = 0xF2;
    pub const PROFILE_INPUT_RELAY: u8 = 0xF3;
    pub const MOVE_PROFILE_INPUT: u8 = 0xF4;
    pub const DISCARD_PROFILE_INPUT: u8 = 0xF5;
    /// The stock client's debug "hash mismatch", which it skips; the patched
    /// client takes it as "upload your world" ([`super::upload`]).
    pub const UPLOAD_REQUEST: u8 = 0xF6;

    pub const CLIENT_FIRST: u8 = CONNECT;
    pub const CLIENT_LAST: u8 = KEEP_ALIVE;
    pub const SERVER_FIRST: u8 = PORTS;
    pub const SERVER_LAST: u8 = UPLOAD_REQUEST;
}

/// The payload length of a client→server id, as the client's table has it
/// (0x279670): all ten are fixed, so in a datagram of several messages they
/// follow each other with no length bytes. `None`: not a client id.
///
/// 0xE0 is 0 here; a chunk (0xE0 with a payload) only ever comes alone in
/// its datagram, where the message is the rest of the datagram.
pub fn client_length(id: u8) -> Option<usize> {
    Some(match id {
        id::CONNECT => 5,
        id::PORT_PROBE => 4,
        id::READY => 0,
        id::TIME_REPLY => 6,
        id::INPUT => 9,
        id::INPUT_PRECISE => 10,
        id::PROFILE_INPUT => 2,
        id::HASH => 5,
        id::LEAVE => 0,
        id::KEEP_ALIVE => 0,
        _ => return None,
    })
}

/// How long a server→client message is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Length {
    Fixed(usize),
    /// Length-prefixed by the transport (0xE4, 0xE8, 0xF6).
    Variable,
    /// 0xED / 0xEE: `head` time bytes, the player mask, 8 bytes for each
    /// player in it ([`player_inputs_len`]).
    PlayerInputs {
        head: usize,
    },
}

/// The length of a server→client id (the client's table, 0x279670).
/// `None`: the client has no such message (0xE2 and everything outside
/// 0xE3..=0xF6); sending one makes it drop the datagram or worse.
pub fn server_length(id: u8) -> Option<Length> {
    Some(match id {
        id::PORTS => Length::Fixed(4),
        id::STATIC_INFO | id::SNAPSHOT | id::UPLOAD_REQUEST => Length::Variable,
        id::SET_TIME => Length::Fixed(7),
        id::PING => Length::Fixed(4),
        id::CLOCK_OFFSET => Length::Fixed(2),
        id::PLAYER_ENTERED | id::PLAYER_LEFT => Length::Fixed(2),
        id::INPUT_RELAY => Length::Fixed(10),
        id::INPUT_RELAY_PRECISE => Length::Fixed(11),
        id::INPUTS_RELAY => Length::PlayerInputs { head: 1 },
        id::INPUTS_RELAY_PRECISE => Length::PlayerInputs { head: 2 },
        id::MOVE_INPUT => Length::Fixed(2),
        id::MOVE_INPUT_PRECISE => Length::Fixed(4),
        id::DISCARD_INPUT => Length::Fixed(1),
        id::DISCARD_INPUT_PRECISE => Length::Fixed(2),
        id::PROFILE_INPUT_RELAY => Length::Fixed(3),
        id::MOVE_PROFILE_INPUT => Length::Fixed(2),
        id::DISCARD_PROFILE_INPUT => Length::Fixed(1),
        _ => return None,
    })
}

/// The bytes of a player mask for a roster of `roster` places:
/// `(roster - 1) / 8 + 1` (0x279610).
pub fn mask_bytes(roster: u8) -> usize {
    (usize::from(roster).max(1) - 1) / 8 + 1
}

/// The length of a 0xED / 0xEE payload that starts `bytes`, as the client's
/// length getter works it out (0x27975B): it wants `head + mask + 16` bytes
/// there (two players' inputs) before it looks, then takes
/// `head + mask + 8 × popcount(mask)`. `None`: the client would drop the
/// whole datagram.
pub fn player_inputs_len(head: usize, roster: u8, bytes: &[u8]) -> Option<usize> {
    let mask_len = mask_bytes(roster);
    if bytes.len() < head + mask_len + 16 {
        return None;
    }
    let players: u32 = bytes[head..head + mask_len]
        .iter()
        .map(|byte| byte.count_ones())
        .sum();
    let len = head + mask_len + 8 * players as usize;
    (len <= bytes.len()).then_some(len)
}

/// The signed distance from `b` to `a` in 8-bit sequence arithmetic
/// (0x20FF90): within −128..=127.
pub fn sdiff8(a: u8, b: u8) -> i32 {
    i32::from(a.wrapping_sub(b) as i8)
}

/// The tick byte of a time: `time / 50`, low 8 bits
/// (`serialize_local_fixed_time` 0x3185F0).
pub fn tick_byte(time_ms: u32) -> u8 {
    (time_ms / TICK_MS) as u8
}

/// The time a tick byte means to a receiver whose clock is at `clock_ms`
/// (reader 0x318416): the tick within ±127 of the clock's own. A sender
/// more than 6.35 s away from the receiver's clock is read as another tick,
/// with no error. `None`: before time 0 (the client's unsigned arithmetic
/// would wrap there).
pub fn tick_time(byte: u8, clock_ms: u32) -> Option<u32> {
    let current = i64::from(clock_ms / TICK_MS);
    let tick = current + i64::from(sdiff8(byte, current as u8));
    u32::try_from(tick * i64::from(TICK_MS)).ok()
}

/// A time as a message carries it: the tick, and for the "precise" ids the
/// milliseconds into it (`serialize_local_time` 0x318620).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WireTime {
    pub tick: u8,
    /// 0..=49, on the ids that carry it.
    pub ms: Option<u8>,
}

impl WireTime {
    /// A time on the 50 ms grid (the part of a tick is dropped).
    pub fn tick(time_ms: u32) -> Self {
        Self {
            tick: tick_byte(time_ms),
            ms: None,
        }
    }

    /// A time with its milliseconds.
    pub fn precise(time_ms: u32) -> Self {
        Self {
            tick: tick_byte(time_ms),
            ms: Some((time_ms % TICK_MS) as u8),
        }
    }

    /// The time this means at a receiver whose clock is at `clock_ms`.
    pub fn resolve(self, clock_ms: u32) -> Option<u32> {
        Some(tick_time(self.tick, clock_ms)? + u32::from(self.ms.unwrap_or(0)))
    }

    fn write(self, precise: bool, out: &mut Vec<u8>) {
        out.push(self.tick);
        if precise {
            out.push(self.ms.unwrap_or(0));
        }
    }
}

/// A player's input (`survarium::player_input::serialize` 0x3184A0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerInput {
    pub time: WireTime,
    /// `math::half2(rotation)`: two IEEE half floats, as they came. They
    /// are relayed byte for byte; decoding and re-encoding one would change
    /// the simulation.
    pub rotation: [u8; 4],
    /// `actions_mask`; bit 31 is a per-input flag the client splits off.
    pub actions: u32,
}

/// A half float's exponent field.
const HALF_EXPONENT: u16 = 0x7C00;
/// The exponent of 1024.0: a rotation of 163 turns in one input.
const HALF_EXPONENT_1024: u16 = 25 << 10;

impl PlayerInput {
    /// Whether the rotation is one a client's mouse can make: both halves
    /// numbers, and each under 1024 radians.
    ///
    /// The client checks nothing of a relayed input (the reader 0x318400
    /// stores the four bytes as they came), and every client then adds the
    /// two to that player's angles (`base_player`, 0x2D89A4 → 0x2D8AD0:
    /// `yaw += dx`, the rotation built from it handed to the physics
    /// world; `dy` to the item in its hands). An infinity or a NaN stays
    /// in that player's state from then on, in every world of the match
    /// and in every snapshot made of it. PROVEN (code); what the physics
    /// world does with it was not read.
    ///
    /// The limit keeps a sum from reaching infinity too: the client adds
    /// the rotations of two inputs it merges into one node, as halves
    /// (0x2C5B60 → 0x2C6640 → 0xF67B0), and a half ends at 65 504.
    pub fn rotation_is_sane(&self) -> bool {
        self.rotation
            .chunks_exact(2)
            .all(|half| u16::from_le_bytes([half[0], half[1]]) & HALF_EXPONENT < HALF_EXPONENT_1024)
    }
}

/// Why a client's message was not taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Not a client→server id.
    UnknownId(u8),
    /// Not the id's fixed length.
    Length { id: u8, expected: usize, got: usize },
    /// A precise time's milliseconds are not within a tick.
    Milliseconds(u8),
    /// 0xE0 with a payload that is no snapshot chunk.
    Chunk(ChunkError),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownId(id) => write!(f, "{id:#04x} is no client message"),
            Self::Length { id, expected, got } => {
                write!(f, "message {id:#04x} has {got} bytes, not {expected}")
            }
            Self::Milliseconds(ms) => write!(f, "{ms} ms is not within a tick"),
            Self::Chunk(error) => write!(f, "snapshot chunk: {error}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// What a client sends (`match.md` §1.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientMessage<'a> {
    /// 0xD8, the first message. `debug` is the client's
    /// `-debug_hash_mismatch` flag: 0x00 or 0xFF.
    Connect { session: u32, debug: u8 },
    /// 0xD9, one for each port of 0xE3's range.
    PortProbe { session: u32 },
    /// 0xDA: the level is loaded. From here the client takes sync
    /// generation 0 as new ([`Generation`]).
    Ready,
    /// 0xDB, the answer to 0xE5 and to 0xE6 ([`TimeReply`]).
    TimeReply(TimeReply),
    /// 0xDC, or 0xDD with the milliseconds.
    Input(PlayerInput),
    /// 0xDE: LIKELY the kit the player picks for the next respawn.
    ProfileInput { tick: u8, value: u8 },
    /// 0xDF: the CRC of the client's event-horizon world, whose time is
    /// `tick` ([`snapshot::crc`]). [`FORCED_MISMATCH`] after the client's
    /// force-mismatch hotkey.
    Hash { tick: u8, crc: u32 },
    /// 0xE0, empty.
    Leave,
    /// 0xE0 with a payload: a patched client's snapshot chunk.
    Chunk(Chunk<'a>),
    /// 0xE1, every second once the world runs.
    KeepAlive,
}

/// The CRC a client reports after its force-mismatch hotkey.
pub const FORCED_MISMATCH: u32 = 0xFFFF_FFFF;

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn milliseconds(ms: u8) -> Result<u8, ParseError> {
    if u32::from(ms) < TICK_MS {
        Ok(ms)
    } else {
        Err(ParseError::Milliseconds(ms))
    }
}

/// Reads one client message. `payload` is what the transport delivered
/// after the id: the id's fixed length ([`client_length`]), or for a 0xE0
/// alone in its datagram the rest of the datagram.
pub fn parse_client(id: u8, payload: &[u8]) -> Result<ClientMessage<'_>, ParseError> {
    let expected = client_length(id).ok_or(ParseError::UnknownId(id))?;
    if id == id::LEAVE && !payload.is_empty() {
        return Chunk::parse(payload)
            .map(ClientMessage::Chunk)
            .map_err(ParseError::Chunk);
    }
    if payload.len() != expected {
        return Err(ParseError::Length {
            id,
            expected,
            got: payload.len(),
        });
    }
    Ok(match id {
        id::CONNECT => ClientMessage::Connect {
            session: u32_at(payload, 0),
            debug: payload[4],
        },
        id::PORT_PROBE => ClientMessage::PortProbe {
            session: u32_at(payload, 0),
        },
        id::READY => ClientMessage::Ready,
        id::TIME_REPLY => ClientMessage::TimeReply(TimeReply {
            first: u16_at(payload, 0),
            stamp: u16_at(payload, 2),
            client_ms: u16_at(payload, 4),
        }),
        id::INPUT => ClientMessage::Input(PlayerInput {
            time: WireTime {
                tick: payload[0],
                ms: None,
            },
            rotation: [payload[1], payload[2], payload[3], payload[4]],
            actions: u32_at(payload, 5),
        }),
        id::INPUT_PRECISE => ClientMessage::Input(PlayerInput {
            time: WireTime {
                tick: payload[0],
                ms: Some(milliseconds(payload[1])?),
            },
            rotation: [payload[2], payload[3], payload[4], payload[5]],
            actions: u32_at(payload, 6),
        }),
        id::PROFILE_INPUT => ClientMessage::ProfileInput {
            tick: payload[0],
            value: payload[1],
        },
        id::HASH => ClientMessage::Hash {
            tick: payload[0],
            crc: u32_at(payload, 1),
        },
        id::LEAVE => ClientMessage::Leave,
        _ => ClientMessage::KeepAlive,
    })
}

/// One player's input in a 0xED / 0xEE batch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BatchInput {
    pub player: u8,
    pub rotation: [u8; 4],
    pub actions: u32,
}

/// What the server sends (`match.md` §1.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerMessage {
    /// 0xE3: the client probes each port with 0xD9 and then sends to a
    /// random one of them. Give the server's one port twice.
    Ports { min: u16, max: u16 },
    /// 0xE4: the client's own roster place, then the match options as a
    /// raw LZMA2 stream ([`static_info::StaticInfo::payload`]).
    StaticInfo { local_index: u8, lzma2: Vec<u8> },
    /// 0xE5: sets the client's game time and clears its history. Taken only
    /// when `generation` is newer than the client's ([`Generation`]); the
    /// client answers 0xDB.
    SetTime {
        generation: u8,
        echo: u16,
        match_time: u32,
    },
    /// 0xE6: the client answers 0xDB on its next send pass
    /// ([`TimeReply::to_ping`]).
    Ping { a: u16, b: u16 },
    /// 0xE7: shifts the client's clock by so many ms.
    ClockOffset(i16),
    /// 0xE8: the world at `time`. `body` is what
    /// `game_state_history_item::serialize` wrote ([`snapshot`]). `packed`:
    /// it goes as LZMA2 (the message's `compressed` byte set) where that
    /// is smaller and the client's buffer holds the state; else as it is.
    Snapshot {
        generation: u8,
        time: u32,
        body: Vec<u8>,
        packed: bool,
    },
    /// 0xE9: a roster place becomes active at a tick.
    PlayerEntered { tick: u8, player: u8 },
    /// 0xEA: a roster place leaves. Sent to the leaver itself it
    /// disconnects it.
    PlayerLeft { tick: u8, player: u8 },
    /// 0xEB, or 0xEC when the input's time has milliseconds.
    InputRelay { player: u8, input: PlayerInput },
    /// 0xED / 0xEE: the inputs of two or more players at one time
    /// ([`ServerMessage::inputs_relay`]).
    InputsRelay {
        time: WireTime,
        /// The roster size of the static info: it sizes the player mask.
        roster: u8,
        /// By player, ascending.
        inputs: Vec<BatchInput>,
    },
    /// 0xEF / 0xF0: the receiver's own input moves from one time to
    /// another. LIKELY how the original server kept a late input.
    MoveInput { from: WireTime, to: WireTime },
    /// 0xF1 / 0xF2: the receiver's own input at a time is dropped.
    DiscardInput { time: WireTime },
    /// 0xF3: a player's 0xDE.
    ProfileInputRelay { tick: u8, player: u8, value: u8 },
    /// 0xF4.
    MoveProfileInput { from_tick: u8, to_tick: u8 },
    /// 0xF5.
    DiscardProfileInput { tick: u8 },
    /// 0xF6 with its one payload byte: "upload your event-horizon world".
    /// Only for patched clients ([`upload`]).
    UploadRequest,
}

impl ServerMessage {
    /// A 0xED / 0xEE batch: the inputs sorted by player. `None` unless
    /// there are at least two (the client's length check asks for two),
    /// each of another player below `roster`.
    pub fn inputs_relay(time: WireTime, roster: u8, mut inputs: Vec<BatchInput>) -> Option<Self> {
        inputs.sort_by_key(|input| input.player);
        let distinct = inputs
            .windows(2)
            .all(|pair| pair[0].player != pair[1].player);
        let in_roster = inputs.iter().all(|input| input.player < roster);
        (inputs.len() >= 2 && distinct && in_roster && usize::from(roster) <= MAX_PLAYERS)
            .then_some(Self::InputsRelay {
                time,
                roster,
                inputs,
            })
    }

    /// A snapshot for the client's sync `generation`, sent as it is.
    pub fn snapshot(generation: u8, time: u32, body: &[u8]) -> Self {
        Self::Snapshot {
            generation,
            time,
            body: body.to_vec(),
            packed: false,
        }
    }

    /// The same, sent compressed where that serves (see
    /// [`ServerMessage::Snapshot`]).
    pub fn packed_snapshot(generation: u8, time: u32, body: &[u8]) -> Self {
        Self::Snapshot {
            generation,
            time,
            body: body.to_vec(),
            packed: true,
        }
    }

    pub fn id(&self) -> u8 {
        match self {
            Self::Ports { .. } => id::PORTS,
            Self::StaticInfo { .. } => id::STATIC_INFO,
            Self::SetTime { .. } => id::SET_TIME,
            Self::Ping { .. } => id::PING,
            Self::ClockOffset(_) => id::CLOCK_OFFSET,
            Self::Snapshot { .. } => id::SNAPSHOT,
            Self::PlayerEntered { .. } => id::PLAYER_ENTERED,
            Self::PlayerLeft { .. } => id::PLAYER_LEFT,
            Self::InputRelay { input, .. } => match input.time.ms {
                None => id::INPUT_RELAY,
                Some(_) => id::INPUT_RELAY_PRECISE,
            },
            Self::InputsRelay { time, .. } => match time.ms {
                None => id::INPUTS_RELAY,
                Some(_) => id::INPUTS_RELAY_PRECISE,
            },
            Self::MoveInput { from, to } => match (from.ms, to.ms) {
                (None, None) => id::MOVE_INPUT,
                _ => id::MOVE_INPUT_PRECISE,
            },
            Self::DiscardInput { time } => match time.ms {
                None => id::DISCARD_INPUT,
                Some(_) => id::DISCARD_INPUT_PRECISE,
            },
            Self::ProfileInputRelay { .. } => id::PROFILE_INPUT_RELAY,
            Self::MoveProfileInput { .. } => id::MOVE_PROFILE_INPUT,
            Self::DiscardProfileInput { .. } => id::DISCARD_PROFILE_INPUT,
            Self::UploadRequest => id::UPLOAD_REQUEST,
        }
    }

    /// The bytes after the id (and after the unique-id byte of 0xE4..=0xE8
    /// and 0xF6, which the transport writes).
    pub fn payload(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Self::Ports { min, max } => {
                out.extend(min.to_le_bytes());
                out.extend(max.to_le_bytes());
            }
            Self::StaticInfo { local_index, lzma2 } => {
                out.reserve(1 + lzma2.len());
                out.push(*local_index);
                out.extend_from_slice(lzma2);
            }
            Self::SetTime {
                generation,
                echo,
                match_time,
            } => {
                out.push(*generation);
                out.extend(echo.to_le_bytes());
                out.extend(match_time.to_le_bytes());
            }
            Self::Ping { a, b } => {
                out.extend(a.to_le_bytes());
                out.extend(b.to_le_bytes());
            }
            Self::ClockOffset(offset) => out.extend(offset.to_le_bytes()),
            Self::Snapshot {
                generation,
                time,
                body,
                packed,
            } => {
                out.reserve(6 + body.len());
                out.push(*generation);
                // The client unpacks into 0x8000 bytes on its stack and
                // does not check the result (0x27C059): a state over that
                // goes as it is, and so does one that would not shrink.
                let stream = (*packed && 4 + body.len() <= COMPRESSED_SNAPSHOT_BUFFER)
                    .then(|| {
                        let mut state = time.to_le_bytes().to_vec();
                        state.extend_from_slice(body);
                        lzma2::packed(&state)
                    })
                    .filter(|stream| stream.len() < 4 + body.len());
                match stream {
                    Some(stream) => {
                        out.push(1);
                        out.extend(stream);
                    }
                    None => {
                        out.push(0);
                        out.extend(time.to_le_bytes());
                        out.extend_from_slice(body);
                    }
                }
            }
            Self::PlayerEntered { tick, player } | Self::PlayerLeft { tick, player } => {
                out.extend([*tick, *player]);
            }
            Self::InputRelay { player, input } => {
                input.time.write(input.time.ms.is_some(), &mut out);
                out.push(*player);
                out.extend(input.rotation);
                out.extend(input.actions.to_le_bytes());
            }
            Self::InputsRelay {
                time,
                roster,
                inputs,
            } => {
                time.write(time.ms.is_some(), &mut out);
                let mut mask = vec![0u8; mask_bytes(*roster)];
                for input in inputs {
                    if let Some(byte) = mask.get_mut(usize::from(input.player / 8)) {
                        *byte |= 1 << (input.player % 8);
                    }
                }
                out.extend(mask);
                // Every action mask, then every rotation.x, then every
                // rotation.y (handler 0x27C4EC..0x27C565).
                for input in inputs {
                    out.extend(input.actions.to_le_bytes());
                }
                for input in inputs {
                    out.extend(&input.rotation[..2]);
                }
                for input in inputs {
                    out.extend(&input.rotation[2..]);
                }
            }
            Self::MoveInput { from, to } => {
                let precise = from.ms.is_some() || to.ms.is_some();
                from.write(precise, &mut out);
                to.write(precise, &mut out);
            }
            Self::DiscardInput { time } => time.write(time.ms.is_some(), &mut out),
            Self::ProfileInputRelay {
                tick,
                player,
                value,
            } => out.extend([*tick, *player, *value]),
            Self::MoveProfileInput { from_tick, to_tick } => out.extend([*from_tick, *to_tick]),
            Self::DiscardProfileInput { tick } => out.push(*tick),
            // The client's length getter refuses an empty 0xF6 and drops
            // the datagram; the patch ignores the byte.
            Self::UploadRequest => out.push(0),
        }
        out
    }
}

/// The client's sync generation (`match.md` §2.2), one byte in 8-bit
/// sequence arithmetic.
///
/// The client sets its own to 0xFF when it sends 0xDA. It takes a 0xE5 only
/// when that one's generation is **newer** than its own (and does not store
/// it), and a 0xE8 when it is **equal or newer** (and stores it). So every
/// sync, the first and each resync, is 0xE5(g) then 0xE8(g) with a g one
/// past the last: 0 for the first. A 0xE8 of an older sync that arrives
/// late is skipped whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Generation(pub u8);

impl Generation {
    /// What the client holds once it has sent 0xDA: the first sync is 0.
    pub const AT_READY: Self = Self(0xFF);

    /// The generation of the next sync.
    #[must_use]
    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// Whether `self` is newer than `other`, as the client compares
    /// (0x27B5A9..0x27B5CD): ahead by 1..=127, or behind by 128..=255.
    pub fn is_newer_than(self, other: Self) -> bool {
        if self.0 > other.0 {
            self.0 - other.0 < 0x80
        } else {
            other.0 - self.0 >= 0x80
        }
    }

    /// Whether a client at `self` takes a 0xE5 of `generation`.
    pub fn takes_set_time(self, generation: u8) -> bool {
        Self(generation).is_newer_than(self)
    }

    /// Whether a client at `self` takes a 0xE8 of `generation` (it then
    /// holds that generation).
    pub fn takes_snapshot(self, generation: u8) -> bool {
        generation == self.0 || Self(generation).is_newer_than(self)
    }
}

/// The client's 0xDB (`match.md` §4): three u16.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeReply {
    /// After 0xE5: `|low16(match_time) − echo|`, so 0 when the server put
    /// the time's low 16 bits in `echo`. After 0xE6: its `b`.
    pub first: u16,
    /// The server's u16 from the request, plus how long the client held it:
    /// 0xE5's `echo` as it was, 0xE6's `a` plus the ms until the send pass.
    /// So `now16 − stamp` is the round trip for both [LIKELY the intent].
    pub stamp: u16,
    /// The client's game clock in ms, low 16 bits [LIKELY].
    pub client_ms: u16,
}

impl TimeReply {
    /// What the client answers a 0xE5 with (0x27B5D3..0x27B6D8), its game
    /// clock being `client_ms` when it sends.
    pub fn to_set_time(echo: u16, match_time: u32, client_ms: u32) -> Self {
        let gap = i32::from((match_time as u16).wrapping_sub(echo) as i16);
        Self {
            first: gap.unsigned_abs() as u16,
            stamp: echo,
            client_ms: client_ms as u16,
        }
    }

    /// What the client answers a 0xE6 with on its next send pass, `held_ms`
    /// after the ping came (0x27CE5E..0x27CF26).
    pub fn to_ping(a: u16, b: u16, held_ms: u32, client_ms: u32) -> Self {
        Self {
            first: b,
            stamp: a.wrapping_add(held_ms as u16),
            client_ms: client_ms as u16,
        }
    }

    /// The round trip in ms, the server's clock being `now_ms` when the
    /// reply came and the request's u16 having been that clock's low 16
    /// bits. Good up to 65 s.
    pub fn round_trip(&self, now_ms: u32) -> u16 {
        (now_ms as u16).wrapping_sub(self.stamp)
    }

    /// The client's clock, whole: the value nearest `near_ms` (the server's
    /// own match clock) with the reply's low 16 bits.
    pub fn client_clock(&self, near_ms: u32) -> u32 {
        unwrap16(self.client_ms, near_ms)
    }
}

/// The u32 nearest `near` whose low 16 bits are `low`: a 16-bit clock made
/// whole against one's own. Right while the two are within 32.7 s.
pub fn unwrap16(low: u16, near: u32) -> u32 {
    let diff = i64::from(low.wrapping_sub(near as u16) as i16);
    (i64::from(near) + diff).clamp(0, i64::from(u32::MAX)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROTATION: [u8; 4] = [0x00, 0x3C, 0x00, 0xC0]; // half 1.0, half -2.0

    #[test]
    fn the_client_table_has_ten_fixed_lengths() {
        // transport.md §4, C→S: D8..E1.
        let lengths: Vec<usize> = (0xD8..=0xE1).map(|id| client_length(id).unwrap()).collect();
        assert_eq!(lengths, [5, 4, 0, 6, 9, 10, 2, 5, 0, 0]);
        assert_eq!(client_length(0xD7), None);
        assert_eq!(client_length(0xE2), None);
    }

    #[test]
    fn the_server_table_is_the_clients() {
        // transport.md §4, S→C: E3..F6.
        use Length::{Fixed, PlayerInputs, Variable};
        let expected = [
            Fixed(4),
            Variable,
            Fixed(7),
            Fixed(4),
            Fixed(2),
            Variable,
            Fixed(2),
            Fixed(2),
            Fixed(10),
            Fixed(11),
            PlayerInputs { head: 1 },
            PlayerInputs { head: 2 },
            Fixed(2),
            Fixed(4),
            Fixed(1),
            Fixed(2),
            Fixed(3),
            Fixed(2),
            Fixed(1),
            Variable,
        ];
        for (id, length) in (0xE3..=0xF6).zip(expected) {
            assert_eq!(server_length(id), Some(length), "{id:#x}");
        }
        assert_eq!(server_length(0xE2), None);
        assert_eq!(server_length(0xF7), None);
    }

    #[test]
    fn every_fixed_server_message_is_as_long_as_the_table_says() {
        let time = WireTime { tick: 7, ms: None };
        let precise = WireTime {
            tick: 7,
            ms: Some(3),
        };
        let input = |time| PlayerInput {
            time,
            rotation: ROTATION,
            actions: 1,
        };
        let messages = [
            ServerMessage::Ports { min: 1, max: 1 },
            ServerMessage::SetTime {
                generation: 0,
                echo: 0,
                match_time: 0,
            },
            ServerMessage::Ping { a: 0, b: 0 },
            ServerMessage::ClockOffset(0),
            ServerMessage::PlayerEntered { tick: 0, player: 0 },
            ServerMessage::PlayerLeft { tick: 0, player: 0 },
            ServerMessage::InputRelay {
                player: 0,
                input: input(time),
            },
            ServerMessage::InputRelay {
                player: 0,
                input: input(precise),
            },
            ServerMessage::MoveInput {
                from: time,
                to: time,
            },
            ServerMessage::MoveInput {
                from: time,
                to: precise,
            },
            ServerMessage::DiscardInput { time },
            ServerMessage::DiscardInput { time: precise },
            ServerMessage::ProfileInputRelay {
                tick: 0,
                player: 0,
                value: 0,
            },
            ServerMessage::MoveProfileInput {
                from_tick: 0,
                to_tick: 0,
            },
            ServerMessage::DiscardProfileInput { tick: 0 },
        ];
        let mut ids = Vec::new();
        for message in messages {
            let Some(Length::Fixed(length)) = server_length(message.id()) else {
                panic!("{:#x} is not fixed", message.id());
            };
            assert_eq!(message.payload().len(), length, "{:#x}", message.id());
            ids.push(message.id());
        }
        assert_eq!(
            ids,
            [
                0xE3, 0xE5, 0xE6, 0xE7, 0xE9, 0xEA, 0xEB, 0xEC, 0xEF, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4,
                0xF5
            ]
        );
    }

    #[test]
    fn times_are_ticks_within_127_of_the_receivers_clock() {
        // match.md §1.3: byte = time / 50, read against the clock.
        assert_eq!(tick_byte(0), 0);
        assert_eq!(tick_byte(49), 0);
        assert_eq!(tick_byte(12_750), 255);
        assert_eq!(tick_byte(12_800), 0);
        assert_eq!(tick_byte(539_650), (10_793 % 256) as u8);
        // The clock at tick 10 793 (0x2A29): byte 0x29 is that tick.
        assert_eq!(tick_time(0x29, 539_650), Some(539_650));
        assert_eq!(tick_time(0x29, 539_699), Some(539_650));
        // Five ticks ahead and behind.
        assert_eq!(tick_time(0x2E, 539_650), Some(539_900));
        assert_eq!(tick_time(0x24, 539_650), Some(539_400));
        // Across the byte's wrap: clock at tick 255, byte 2 is tick 258.
        assert_eq!(tick_time(2, 12_750), Some(12_900));
        assert_eq!(tick_time(254, 12_800), Some(12_700));
        // The edges: +127 and −128 ticks.
        assert_eq!(tick_time(127, 0), Some(6_350));
        assert_eq!(tick_time(128, 12_800), Some(6_400));
        // Before time 0.
        assert_eq!(tick_time(255, 0), None);
        assert_eq!(sdiff8(0, 255), 1);
        assert_eq!(sdiff8(255, 0), -1);
        assert_eq!(sdiff8(128, 0), -128);
        assert_eq!(sdiff8(127, 0), 127);
    }

    #[test]
    fn a_precise_time_adds_its_milliseconds() {
        let time = WireTime::precise(539_673);
        assert_eq!(
            time,
            WireTime {
                tick: 0x29,
                ms: Some(23)
            }
        );
        assert_eq!(time.resolve(539_800), Some(539_673));
        assert_eq!(WireTime::tick(539_673).resolve(539_800), Some(539_650));
    }

    #[test]
    fn connect_ports_and_ready() {
        // match.md §1.1: D8 `u32 session, u8 debug`; D9 `u32 session`.
        assert_eq!(
            parse_client(0xD8, &[0x78, 0x56, 0x34, 0x12, 0x00]),
            Ok(ClientMessage::Connect {
                session: 0x1234_5678,
                debug: 0
            })
        );
        assert_eq!(
            parse_client(0xD8, &[1, 0, 0, 0, 0xFF]),
            Ok(ClientMessage::Connect {
                session: 1,
                debug: 0xFF
            })
        );
        assert_eq!(
            parse_client(0xD9, &[0x78, 0x56, 0x34, 0x12]),
            Ok(ClientMessage::PortProbe {
                session: 0x1234_5678
            })
        );
        assert_eq!(parse_client(0xDA, &[]), Ok(ClientMessage::Ready));
        assert_eq!(parse_client(0xE1, &[]), Ok(ClientMessage::KeepAlive));
        assert_eq!(parse_client(0xE0, &[]), Ok(ClientMessage::Leave));
        // transport.md §9 (b): E3 {1236, 1236}.
        assert_eq!(
            ServerMessage::Ports {
                min: 1236,
                max: 1236
            }
            .payload(),
            [0xD4, 0x04, 0xD4, 0x04]
        );
    }

    #[test]
    fn wrong_lengths_and_ids_are_refused() {
        assert_eq!(
            parse_client(0xD8, &[0; 4]),
            Err(ParseError::Length {
                id: 0xD8,
                expected: 5,
                got: 4
            })
        );
        assert_eq!(
            parse_client(0xDA, &[0]),
            Err(ParseError::Length {
                id: 0xDA,
                expected: 0,
                got: 1
            })
        );
        assert_eq!(parse_client(0xE2, &[]), Err(ParseError::UnknownId(0xE2)));
        assert_eq!(
            parse_client(0xE3, &[0; 4]),
            Err(ParseError::UnknownId(0xE3))
        );
        // A precise input 50 ms into its tick is no time.
        assert_eq!(
            parse_client(0xDD, &[1, 50, 0, 0, 0, 0, 0, 0, 0, 0]),
            Err(ParseError::Milliseconds(50))
        );
        // 0xE0 with a payload that is no chunk.
        assert!(matches!(
            parse_client(0xE0, &[1, 2, 3]),
            Err(ParseError::Chunk(_))
        ));
    }

    /// `PlayerInput::rotation_is_sane`: the halves a server hands on.
    #[test]
    fn a_rotation_is_a_number_under_1024() {
        let sane = |x: u16, y: u16| {
            let (x, y) = (x.to_le_bytes(), y.to_le_bytes());
            PlayerInput {
                time: WireTime::tick(1_000),
                rotation: [x[0], x[1], y[0], y[1]],
                actions: 0,
            }
            .rotation_is_sane()
        };
        // 0, -0, the smallest subnormal, 1, -1, pi, and 1023.5 either way.
        for half in [
            0x0000, 0x8000, 0x0001, 0x3C00, 0xBC00, 0x4248, 0x63FF, 0xE3FF,
        ] {
            assert!(sane(half, 0), "{half:#06x}");
            assert!(sane(0, half), "{half:#06x}");
        }
        // 1024, -1024, 65 504 (the largest half), the infinities, NaNs
        // quiet and signalling, of either sign.
        for half in [
            0x6400, 0xE400, 0x7BFF, 0xFBFF, 0x7C00, 0xFC00, 0x7E00, 0x7C01, 0xFE00, 0xFFFF,
        ] {
            assert!(!sane(half, 0), "{half:#06x}");
            assert!(!sane(0, half), "{half:#06x}");
            assert!(!sane(half, half), "{half:#06x}");
        }
    }

    #[test]
    fn inputs_keep_their_half_floats_byte_for_byte() {
        // DC: `u8 tick, half2 rotation, u32 actions_mask`.
        let wire = [0x29, 0x00, 0x3C, 0x00, 0xC0, 0x21, 0x00, 0x00, 0x80];
        let ClientMessage::Input(input) = parse_client(0xDC, &wire).unwrap() else {
            panic!();
        };
        assert_eq!(
            input,
            PlayerInput {
                time: WireTime {
                    tick: 0x29,
                    ms: None
                },
                rotation: ROTATION,
                actions: 0x8000_0021,
            }
        );
        // EB: `u8 tick, u8 player, half2, u32`.
        let relay = ServerMessage::InputRelay { player: 3, input };
        assert_eq!(relay.id(), 0xEB);
        assert_eq!(
            relay.payload(),
            [0x29, 0x03, 0x00, 0x3C, 0x00, 0xC0, 0x21, 0x00, 0x00, 0x80]
        );

        // DD: `u8 tick, u8 ms, half2, u32`; EC: `tick, ms, player, half2, u32`.
        let wire = [0x29, 0x17, 0x00, 0x3C, 0x00, 0xC0, 0x20, 0x00, 0x00, 0x00];
        let ClientMessage::Input(input) = parse_client(0xDD, &wire).unwrap() else {
            panic!();
        };
        assert_eq!(input.time.ms, Some(23));
        let relay = ServerMessage::InputRelay { player: 19, input };
        assert_eq!(relay.id(), 0xEC);
        assert_eq!(
            relay.payload(),
            [
                0x29, 0x17, 0x13, 0x00, 0x3C, 0x00, 0xC0, 0x20, 0x00, 0x00, 0x00
            ]
        );
    }

    #[test]
    fn a_batch_lists_masks_then_each_rotation_half() {
        // ED: `u8 tick`, ceil(N/8) mask bytes, k × u32 actions, k × u16
        // rotation.x, k × u16 rotation.y. Players 1 and 9 of a roster of 10.
        let inputs = vec![
            BatchInput {
                player: 9,
                rotation: [0xAA, 0xBB, 0xCC, 0xDD],
                actions: 0x0000_0002,
            },
            BatchInput {
                player: 1,
                rotation: [0x11, 0x22, 0x33, 0x44],
                actions: 0x8000_0001,
            },
        ];
        let time = WireTime {
            tick: 0x40,
            ms: None,
        };
        let batch = ServerMessage::inputs_relay(time, 10, inputs.clone()).unwrap();
        assert_eq!(batch.id(), 0xED);
        let expected = [
            0x40, // tick
            0x02, 0x02, // players 1 and 9
            0x01, 0x00, 0x00, 0x80, 0x02, 0x00, 0x00, 0x00, // actions
            0x11, 0x22, 0xAA, 0xBB, // rotation.x
            0x33, 0x44, 0xCC, 0xDD, // rotation.y
        ];
        assert_eq!(batch.payload(), expected);
        assert_eq!(player_inputs_len(1, 10, &expected), Some(expected.len()));

        // EE: `u8 tick, u8 ms` first. A roster of 8 has one mask byte.
        let time = WireTime {
            tick: 0x40,
            ms: Some(49),
        };
        let mut inputs = inputs;
        inputs[0].player = 7;
        let batch = ServerMessage::inputs_relay(time, 8, inputs).unwrap();
        assert_eq!(batch.id(), 0xEE);
        let payload = batch.payload();
        assert_eq!(payload[..3], [0x40, 0x31, 0x82]);
        assert_eq!(payload.len(), 2 + 1 + 16);
        assert_eq!(player_inputs_len(2, 8, &payload), Some(19));
    }

    #[test]
    fn a_batch_needs_two_distinct_players_of_the_roster() {
        let time = WireTime { tick: 0, ms: None };
        let input = |player| BatchInput {
            player,
            rotation: [0; 4],
            actions: 0,
        };
        assert_eq!(ServerMessage::inputs_relay(time, 4, vec![input(0)]), None);
        assert_eq!(
            ServerMessage::inputs_relay(time, 4, vec![input(1), input(1)]),
            None
        );
        assert_eq!(
            ServerMessage::inputs_relay(time, 4, vec![input(1), input(4)]),
            None
        );
        assert!(ServerMessage::inputs_relay(time, 4, vec![input(3), input(0)]).is_some());
        // The client's length check wants two inputs' bytes before it
        // counts the mask.
        assert_eq!(
            player_inputs_len(1, 4, &[0, 0b1, 0, 0, 0, 0, 0, 0, 0, 0]),
            None
        );
        // A mask that promises more than there is.
        let mut short = vec![0, 0b111];
        short.extend([0; 16]);
        assert_eq!(player_inputs_len(1, 4, &short), None);
    }

    #[test]
    fn mask_bytes_follow_the_roster() {
        assert_eq!(mask_bytes(1), 1);
        assert_eq!(mask_bytes(8), 1);
        assert_eq!(mask_bytes(9), 2);
        assert_eq!(mask_bytes(16), 2);
        assert_eq!(mask_bytes(17), 3);
        assert_eq!(mask_bytes(20), 3);
    }

    #[test]
    fn roster_changes_and_input_corrections() {
        // E9 / EA: `u8 tick, u8 player`.
        assert_eq!(
            ServerMessage::PlayerEntered {
                tick: 0x10,
                player: 2
            }
            .payload(),
            [0x10, 0x02]
        );
        assert_eq!(
            ServerMessage::PlayerLeft {
                tick: 0x11,
                player: 5
            }
            .payload(),
            [0x11, 0x05]
        );
        // EF `from, to`; F0 `tick, ms` twice.
        let from = WireTime {
            tick: 0x20,
            ms: None,
        };
        let to = WireTime {
            tick: 0x22,
            ms: None,
        };
        let moved = ServerMessage::MoveInput { from, to };
        assert_eq!((moved.id(), moved.payload()), (0xEF, vec![0x20, 0x22]));
        let from = WireTime {
            tick: 0x20,
            ms: Some(7),
        };
        let moved = ServerMessage::MoveInput { from, to };
        assert_eq!(
            (moved.id(), moved.payload()),
            (0xF0, vec![0x20, 0x07, 0x22, 0x00])
        );
        // F1 `tick`; F2 `tick, ms`.
        let discard = ServerMessage::DiscardInput { time: to };
        assert_eq!((discard.id(), discard.payload()), (0xF1, vec![0x22]));
        let discard = ServerMessage::DiscardInput { time: from };
        assert_eq!((discard.id(), discard.payload()), (0xF2, vec![0x20, 0x07]));
    }

    #[test]
    fn profile_inputs() {
        // DE `u8 tick, u8 value`; F3 `tick, player, value`; F4; F5.
        assert_eq!(
            parse_client(0xDE, &[0x30, 0x02]),
            Ok(ClientMessage::ProfileInput {
                tick: 0x30,
                value: 2
            })
        );
        assert_eq!(
            ServerMessage::ProfileInputRelay {
                tick: 0x30,
                player: 4,
                value: 2
            }
            .payload(),
            [0x30, 0x04, 0x02]
        );
        assert_eq!(
            ServerMessage::MoveProfileInput {
                from_tick: 0x30,
                to_tick: 0x33
            }
            .payload(),
            [0x30, 0x33]
        );
        assert_eq!(
            ServerMessage::DiscardProfileInput { tick: 0x30 }.payload(),
            [0x30]
        );
    }

    #[test]
    fn hash_reports() {
        // DF: `u8 tick, u32 crc`.
        assert_eq!(
            parse_client(0xDF, &[0x29, 0xEF, 0xBE, 0xAD, 0xDE]),
            Ok(ClientMessage::Hash {
                tick: 0x29,
                crc: 0xDEAD_BEEF
            })
        );
        assert_eq!(
            parse_client(0xDF, &[0x29, 0xFF, 0xFF, 0xFF, 0xFF]),
            Ok(ClientMessage::Hash {
                tick: 0x29,
                crc: FORCED_MISMATCH
            })
        );
    }

    #[test]
    fn time_sync_messages() {
        // E5: `u8 generation, u16 echo, u32 match_time_ms`.
        let match_time = 0x0001_E240; // 123 456 ms
        let set = ServerMessage::SetTime {
            generation: 3,
            echo: 0xE240,
            match_time,
        };
        assert_eq!(set.payload(), [0x03, 0x40, 0xE2, 0x40, 0xE2, 0x01, 0x00]);
        // E6 `u16 a, u16 b`; E7 `i16`.
        assert_eq!(
            ServerMessage::Ping {
                a: 0x1234,
                b: 0x0056
            }
            .payload(),
            [0x34, 0x12, 0x56, 0x00]
        );
        assert_eq!(ServerMessage::ClockOffset(-2).payload(), [0xFE, 0xFF]);
        // DB: three u16.
        assert_eq!(
            parse_client(0xDB, &[0x00, 0x00, 0x40, 0xE2, 0x39, 0x30]),
            Ok(ClientMessage::TimeReply(TimeReply {
                first: 0,
                stamp: 0xE240,
                client_ms: 0x3039
            }))
        );
    }

    #[test]
    fn the_clients_time_replies() {
        // To 0xE5 with the time's low bits as the echo: gap 0.
        let reply = TimeReply::to_set_time(0xE240, 0x0001_E240, 0x0001_E240);
        assert_eq!(
            reply,
            TimeReply {
                first: 0,
                stamp: 0xE240,
                client_ms: 0xE240
            }
        );
        // The echo 10 ms off either way: the gap's size.
        assert_eq!(TimeReply::to_set_time(0xE24A, 0x0001_E240, 0).first, 10);
        assert_eq!(TimeReply::to_set_time(0xE236, 0x0001_E240, 0).first, 10);
        // To 0xE6: `b`, `a` plus the 16 ms it was held, the clock.
        let reply = TimeReply::to_ping(0xFFF8, 0x0056, 16, 70_000);
        assert_eq!(
            reply,
            TimeReply {
                first: 0x0056,
                stamp: 0x0008,
                client_ms: 0x1170
            }
        );
        // The server stamped its clock's low bits (…FFF8) and gets the
        // reply 60 ms later: 44 ms of it were the round trip.
        assert_eq!(reply.round_trip(0x0002_FFF8 + 60), 44);
        // The 16-bit clock made whole against the server's.
        assert_eq!(reply.client_clock(70_100), 70_000);
        assert_eq!(unwrap16(0xFFFF, 0x0002_0001), 0x0001_FFFF);
        assert_eq!(unwrap16(0x0001, 0x0001_FFFF), 0x0002_0001);
        assert_eq!(unwrap16(0xFFFF, 3), 0);
    }

    #[test]
    fn sync_generations_are_compared_as_the_client_does() {
        let ready = Generation::AT_READY;
        assert_eq!(ready.next(), Generation(0));
        // The first sync is generation 0.
        assert!(ready.takes_set_time(0));
        assert!(ready.takes_snapshot(0));
        assert!(!ready.takes_set_time(0xFF), "its own is not newer");
        assert!(
            ready.takes_snapshot(0xFF),
            "but a snapshot of its own is taken"
        );
        // A resync must step: at 0, 0xE5(0) is dropped, 0xE5(1) taken.
        let synced = Generation(0);
        assert!(!synced.takes_set_time(0));
        assert!(synced.takes_set_time(1));
        assert!(synced.takes_snapshot(1));
        // A late snapshot of an older sync is skipped.
        assert!(!Generation(5).takes_snapshot(4));
        assert!(!Generation(5).takes_set_time(4));
        // Half the range ahead is not newer; one less is.
        assert!(Generation(127).is_newer_than(Generation(0)));
        assert!(!Generation(128).is_newer_than(Generation(0)));
        assert!(Generation(0).is_newer_than(Generation(128)));
        assert!(Generation(0).is_newer_than(Generation(200)));
        assert!(!Generation(200).is_newer_than(Generation(0)));
    }

    #[test]
    fn a_snapshot_goes_out_uncompressed_with_its_generation_and_time() {
        // E8: `u8 generation, u8 compressed, u32 time, body`.
        let snapshot = ServerMessage::snapshot(2, 539_650, &[0xAA, 0xBB]);
        assert_eq!(snapshot.id(), 0xE8);
        assert_eq!(
            snapshot.payload(),
            [0x02, 0x00, 0x02, 0x3C, 0x08, 0x00, 0xAA, 0xBB]
        );
        // Asked for packed, a state that would not shrink goes the same.
        let packed = ServerMessage::packed_snapshot(2, 539_650, &[0xAA, 0xBB]);
        assert_eq!(packed.payload(), snapshot.payload());
    }

    #[test]
    fn a_packed_snapshot_is_lzma2_of_its_time_and_state() {
        // E8: `u8 generation, u8 1, LZMA2(u32 time, body)`.
        let body: Vec<u8> = (0..9_000u32).map(|i| (i % 41) as u8).collect();
        let payload = ServerMessage::packed_snapshot(7, 539_650, &body).payload();
        assert_eq!(payload[..2], [0x07, 0x01]);
        assert!(payload.len() < body.len() / 2, "{} bytes", payload.len());
        let mut state = 539_650u32.to_le_bytes().to_vec();
        state.extend(&body);
        assert_eq!(lzma2::unpack(&payload[2..], 0x8000), Some(state));
        // The client unpacks into 0x8000 bytes: the largest state that
        // fits is packed, one byte more goes as it is.
        let fits = vec![0; COMPRESSED_SNAPSHOT_BUFFER - 4];
        assert_eq!(ServerMessage::packed_snapshot(7, 50, &fits).payload()[1], 1);
        let over = vec![0; COMPRESSED_SNAPSHOT_BUFFER - 3];
        let payload = ServerMessage::packed_snapshot(7, 50, &over).payload();
        assert_eq!(payload[1], 0);
        assert_eq!(payload.len(), 2 + 4 + over.len());
    }

    #[test]
    fn static_info_starts_with_the_local_index() {
        let info = ServerMessage::StaticInfo {
            local_index: 1,
            lzma2: vec![0x01, 0x00, 0x00, 0x55, 0x00],
        };
        assert_eq!(info.id(), 0xE4);
        assert_eq!(info.payload(), [0x01, 0x01, 0x00, 0x00, 0x55, 0x00]);
    }

    #[test]
    fn the_upload_request_has_one_byte() {
        // client_patch.md §3.4: `F6 <unique> 00`.
        assert_eq!(ServerMessage::UploadRequest.id(), 0xF6);
        assert_eq!(ServerMessage::UploadRequest.payload(), [0x00]);
    }
}

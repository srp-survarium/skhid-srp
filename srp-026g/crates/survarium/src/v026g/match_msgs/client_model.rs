//! A model of the 0.26g client's side of a match, for tests of what a
//! server sends: a strict reader of every server→client message, and the
//! order and generation rules of `docs/notes/match.md` §1.2 and
//! §2.2.
//!
//! The real client checks almost nothing: a wrong length desynchronises
//! its parse of the datagram, a wrong index reads other memory. The model
//! turns each of those into an error, and reports what the client would
//! silently skip, so a server test can assert that everything it sent was
//! taken. It is no simulation: it has no world, only the rules that decide
//! whether a message is taken.

use super::super::dictionary::Dictionary;
use super::super::lzma2;
use super::snapshot::Sections;
use super::static_info::{self, MatchOptions, StaticInfoError};
use super::{
    BatchInput, Generation, Grid, Length, MAX_PLAYERS, PlayerInput, ServerMessage, TICK_MS,
    TimeReply, Update, WireError, WireTime, id, mask_bytes, player_inputs_len, server_length,
};

/// A compressed snapshot is decoded into a buffer of this size.
pub const COMPRESSED_SNAPSHOT_BUFFER: usize = 0x8000;

/// What a real client would choke on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    /// No such server message: the client drops the datagram (0xE2: jumps
    /// to address 0).
    UnknownId(u8),
    /// Not the id's length: the client's parse of the datagram goes wrong.
    Length { id: u8, got: usize },
    /// A precise time's milliseconds are not within a tick.
    Milliseconds(u8),
    /// A player index outside the roster.
    Player { player: u8, roster: u8 },
    /// The static info would harm the client.
    StaticInfo(StaticInfoError),
    /// A 0xE5 before the client sent 0xDA: it has no match core yet and
    /// dereferences it.
    SetTimeBeforeReady,
    /// A 0xE8 with no 0xE5 taken before it: the client's clock was never
    /// set.
    SnapshotWithoutTime,
    /// A 0xE8 whose state is not a time, a mask, player blocks and six
    /// sections; or a compressed one that is no LZMA2 stream or decodes to
    /// more than 0x8000 bytes.
    SnapshotState,
    /// A message with a tick, before any 0xE5 set the clock it is read
    /// against.
    NoClock,
    /// A 0xF7 that is no update of the match's grid, or one to a game
    /// that was never told the per-tick protocol (it has no such id).
    Update(Option<WireError>),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownId(id) => write!(f, "{id:#04x} is no server message"),
            Self::Length { id, got } => write!(f, "message {id:#04x} with {got} bytes"),
            Self::Milliseconds(ms) => write!(f, "{ms} ms is not within a tick"),
            Self::Player { player, roster } => {
                write!(f, "player {player} in a roster of {roster}")
            }
            Self::StaticInfo(error) => write!(f, "static info: {error}"),
            Self::SetTimeBeforeReady => write!(f, "0xE5 before the client is ready"),
            Self::SnapshotWithoutTime => write!(f, "0xE8 with no 0xE5 before it"),
            Self::SnapshotState => write!(f, "0xE8's state is no world"),
            Self::NoClock => write!(f, "a tick before the clock was set"),
            Self::Update(Some(error)) => write!(f, "0xF7: {error:?}"),
            Self::Update(None) => write!(f, "0xF7 to a game of the stock protocol"),
        }
    }
}

impl std::error::Error for ModelError {}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn rotation(bytes: &[u8], at: usize) -> [u8; 4] {
    [bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]
}

fn precise(tick: u8, ms: u8) -> Result<WireTime, ModelError> {
    if u32::from(ms) >= TICK_MS {
        return Err(ModelError::Milliseconds(ms));
    }
    Ok(WireTime { tick, ms: Some(ms) })
}

/// The time a game's world core has before any 0xE5 (0x2D0765).
const INITIAL_CORE_MS: u32 = 50;

fn on_grid(tick: u8) -> WireTime {
    WireTime { tick, ms: None }
}

/// Reads a server message's payload by the client's rules: the exact
/// length of its table, players inside the roster of `roster` places.
/// What [`ServerMessage::payload`] wrote reads back equal.
pub fn decode_server(id: u8, payload: &[u8], roster: u8) -> Result<ServerMessage, ModelError> {
    let wrong_length = ModelError::Length {
        id,
        got: payload.len(),
    };
    let in_roster = |player: u8| {
        if player < roster {
            Ok(player)
        } else {
            Err(ModelError::Player { player, roster })
        }
    };
    match server_length(id).ok_or(ModelError::UnknownId(id))? {
        Length::Fixed(length) if payload.len() != length => return Err(wrong_length),
        Length::PlayerInputs { head }
            if player_inputs_len(head, roster, payload) != Some(payload.len()) =>
        {
            return Err(wrong_length);
        }
        _ => {}
    }
    Ok(match id {
        id::PORTS => ServerMessage::Ports {
            min: u16_at(payload, 0),
            max: u16_at(payload, 2),
        },
        id::STATIC_INFO => {
            let (payload, ticks) =
                static_info::split_ticks_trailer(payload).map_err(ModelError::StaticInfo)?;
            let (&local_index, lzma2) = payload.split_first().ok_or(wrong_length)?;
            ServerMessage::StaticInfo {
                local_index,
                lzma2: lzma2.to_vec(),
                ticks,
            }
        }
        id::SET_TIME => ServerMessage::SetTime {
            generation: payload[0],
            echo: u16_at(payload, 1),
            match_time: u32_at(payload, 3),
        },
        id::PING => ServerMessage::Ping {
            a: u16_at(payload, 0),
            b: u16_at(payload, 2),
        },
        id::CLOCK_OFFSET => ServerMessage::ClockOffset(u16_at(payload, 0) as i16),
        id::SNAPSHOT => {
            // The length getter wants more than two bytes.
            if payload.len() <= 2 {
                return Err(wrong_length);
            }
            let packed = payload[1] != 0;
            let state = if packed {
                lzma2::unpack(&payload[2..], COMPRESSED_SNAPSHOT_BUFFER)
                    .ok_or(ModelError::SnapshotState)?
            } else {
                payload[2..].to_vec()
            };
            if state.len() < 4 {
                return Err(ModelError::SnapshotState);
            }
            ServerMessage::Snapshot {
                generation: payload[0],
                time: u32_at(&state, 0),
                body: state[4..].to_vec(),
                packed,
            }
        }
        id::PLAYER_ENTERED => ServerMessage::PlayerEntered {
            tick: payload[0],
            player: in_roster(payload[1])?,
        },
        id::PLAYER_LEFT => ServerMessage::PlayerLeft {
            tick: payload[0],
            player: in_roster(payload[1])?,
        },
        id::INPUT_RELAY => ServerMessage::InputRelay {
            player: in_roster(payload[1])?,
            input: PlayerInput {
                time: on_grid(payload[0]),
                rotation: rotation(payload, 2),
                actions: u32_at(payload, 6),
            },
        },
        id::INPUT_RELAY_PRECISE => ServerMessage::InputRelay {
            player: in_roster(payload[2])?,
            input: PlayerInput {
                time: precise(payload[0], payload[1])?,
                rotation: rotation(payload, 3),
                actions: u32_at(payload, 7),
            },
        },
        id::INPUTS_RELAY | id::INPUTS_RELAY_PRECISE => {
            let (time, head) = if id == id::INPUTS_RELAY {
                (on_grid(payload[0]), 1)
            } else {
                (precise(payload[0], payload[1])?, 2)
            };
            let mask_len = mask_bytes(roster);
            let mut players = Vec::new();
            for (index, byte) in payload[head..head + mask_len].iter().enumerate() {
                for bit in 0..8 {
                    if byte & (1 << bit) != 0 {
                        players.push(in_roster((index * 8 + bit) as u8)?);
                    }
                }
            }
            let count = players.len();
            let actions_at = head + mask_len;
            let x_at = actions_at + 4 * count;
            let y_at = x_at + 2 * count;
            let inputs = players
                .into_iter()
                .enumerate()
                .map(|(index, player)| BatchInput {
                    player,
                    rotation: [
                        payload[x_at + 2 * index],
                        payload[x_at + 2 * index + 1],
                        payload[y_at + 2 * index],
                        payload[y_at + 2 * index + 1],
                    ],
                    actions: u32_at(payload, actions_at + 4 * index),
                })
                .collect();
            ServerMessage::InputsRelay {
                time,
                roster,
                inputs,
            }
        }
        id::MOVE_INPUT => ServerMessage::MoveInput {
            from: on_grid(payload[0]),
            to: on_grid(payload[1]),
        },
        id::MOVE_INPUT_PRECISE => ServerMessage::MoveInput {
            from: precise(payload[0], payload[1])?,
            to: precise(payload[2], payload[3])?,
        },
        id::DISCARD_INPUT => ServerMessage::DiscardInput {
            time: on_grid(payload[0]),
        },
        id::DISCARD_INPUT_PRECISE => ServerMessage::DiscardInput {
            time: precise(payload[0], payload[1])?,
        },
        id::PROFILE_INPUT_RELAY => ServerMessage::ProfileInputRelay {
            tick: payload[0],
            player: in_roster(payload[1])?,
            value: payload[2],
        },
        id::MOVE_PROFILE_INPUT => ServerMessage::MoveProfileInput {
            from_tick: payload[0],
            to_tick: payload[1],
        },
        id::DISCARD_PROFILE_INPUT => ServerMessage::DiscardProfileInput { tick: payload[0] },
        // 0xF6: the length getter refuses an empty one.
        _ => {
            if payload.is_empty() {
                return Err(wrong_length);
            }
            ServerMessage::UploadRequest
        }
    })
}

/// Why the client skips a message without harm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    /// Not the connection state the message belongs to: 0xE3 only while
    /// connecting, 0xE4 only once the ports are known, the rest only after
    /// the static info.
    WrongState,
    /// The level is not loaded yet (no 0xDA sent): there is no match core.
    NoCore,
    /// A 0xE5 whose generation is not newer than the client's, or a 0xE8
    /// of an older one.
    OldGeneration,
    /// An input, roster change or correction at or before the last
    /// snapshot's time: "older than last resync".
    OlderThanResync,
    /// A roster entry (0xE9) for a tick its game has played past by the
    /// event horizon, once the world of its sync is loaded: "REJECTED,
    /// reason: too old" (`enter_match` 0x2D15C2..0x2D15E8). A client never
    /// moves it to a later tick, so every copy is refused the same way.
    TooOld,
    /// 0xF6 on a stock client: skipped with a log line.
    NotPatched,
}

/// A message the client took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    pub message: ServerMessage,
    /// The time the message's tick means on the client's clock.
    pub time: Option<u32>,
    /// The 0xDB the client answers with (to 0xE5 at once, to 0xE6 on its
    /// next send pass).
    pub reply: Option<TimeReply>,
}

/// What a message did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Taken {
    Applied(Applied),
    Skipped(Skip),
}

/// The connection states (`network_client`+0x1F1C64).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Connecting,
    PortsKnown,
    StaticInfoReceived,
}

/// One client of a match, as far as the rules of taking messages go.
#[derive(Clone, Debug)]
pub struct ClientModel {
    /// Whether it has the srp upload patch (it acts on 0xF6).
    pub patched: bool,
    /// Experimental fixed-capacity wire dialect; independent of upload support.
    pub growing_roster: bool,
    pub state: State,
    /// The options it loaded, and its own roster place.
    pub options: Option<MatchOptions>,
    pub local_index: Option<u8>,
    /// Whether it has sent 0xDA.
    pub ready: bool,
    pub generation: Generation,
    /// Its game clock, once a 0xE5 set it.
    pub clock: Option<u32>,
    /// How its clock takes a correction (0xE7). `None`: all of it at once,
    /// which no game does and most tests want (they are not about the
    /// clock). `Some(factor)`: as the game, at that share of the time that
    /// passes ([`super::clock`]).
    pub clock_factor: Option<f32>,
    /// What is left of the correction it is taking (ms, either way), and
    /// the part of a ms made up and not yet on the clock.
    correction_left: f64,
    correction_carried: f64,
    /// The time of the last snapshot it loaded; `None` while its world is
    /// frozen (before "THE VERY FIRST SYNCHRONIZATION").
    pub last_resync: Option<u32>,
    /// Whether the world of its latest 0xE5 is loaded: from then on its
    /// "present" is its clock, and a roster entry older than the event
    /// horizon is refused (`Skip::TooOld`). Before it (the 0xE5 cleared
    /// the present to 50 ms) every entry is taken, and the event list
    /// outlives the load (0xE8 loads in mode 1, 0x27C107).
    pub world_loaded: bool,
    /// Snapshots loaded: 1 is the first sync, each more a "FULL RESYNC".
    pub syncs: u32,
    /// The per-tick protocol, from its 0xE4's trailer: the match's grid.
    /// `None`: the stock protocol (it has no 0xF7).
    pub ticks: Option<Grid>,
}

/// The client's event horizon (`gwc+0x18FB4`): `max_ping_time_in_ms` +
/// 250, 500 ms on a client.
pub const EVENT_HORIZON_MS: u32 = 500;

impl ClientModel {
    pub fn new(patched: bool) -> Self {
        Self {
            patched,
            growing_roster: false,
            state: State::Connecting,
            options: None,
            local_index: None,
            ready: false,
            generation: Generation::AT_READY,
            clock: None,
            clock_factor: None,
            correction_left: 0.0,
            correction_carried: 0.0,
            last_resync: None,
            world_loaded: false,
            syncs: 0,
            ticks: None,
        }
    }

    /// 0xF7, the per-tick protocol's update: taken in the state of the
    /// other match messages, once the game has its core. What of it its
    /// world takes (nothing at or before the last world's time) is the
    /// patch's to sort out, by the same rules as the stock handlers'.
    fn receive_update(&mut self, payload: &[u8]) -> Result<Taken, ModelError> {
        let grid = self.ticks.ok_or(ModelError::Update(None))?;
        let update = Update::parse(payload, grid).map_err(|error| ModelError::Update(Some(error)))?;
        if self.state != State::StaticInfoReceived {
            return Ok(Taken::Skipped(Skip::WrongState));
        }
        if !self.ready {
            return Ok(Taken::Skipped(Skip::NoCore));
        }
        let time = Some(update.tick.0);
        Ok(Taken::Applied(Applied {
            message: ServerMessage::Update(update),
            time,
            reply: None,
        }))
    }

    /// The roster size, once the static info came.
    pub fn roster(&self) -> u8 {
        self.options
            .as_ref()
            .map_or(0, |options| options.players.len() as u8)
    }

    /// The client finished loading and sends 0xDA: its generation becomes
    /// 0xFF. `false`: it has no static info to load from.
    pub fn send_ready(&mut self) -> bool {
        if self.state != State::StaticInfoReceived {
            return false;
        }
        self.ready = true;
        self.generation = Generation::AT_READY;
        true
    }

    /// Time passes on the client's clock. While it takes a correction the
    /// clock goes by `1 + factor` of that time, or `1 - factor` (the
    /// timer's 0x147110).
    pub fn advance(&mut self, ms: u32) {
        let Some(clock) = &mut self.clock else {
            return;
        };
        *clock = clock.saturating_add(ms);
        let Some(factor) = self.clock_factor else {
            return;
        };
        let step = (f64::from(ms) * f64::from(factor)).min(self.correction_left.abs());
        let step = step.copysign(self.correction_left);
        self.correction_left -= step;
        self.correction_carried += step;
        let whole = self.correction_carried.trunc();
        self.correction_carried -= whole;
        *clock = clock.saturating_add_signed(whole as i32);
    }

    /// What is left of the correction its clock is taking, in ms.
    pub fn correction_left(&self) -> f64 {
        self.correction_left
    }

    /// Whether its simulation runs: it has loaded a snapshot.
    pub fn is_synced(&self) -> bool {
        self.last_resync.is_some()
    }

    fn resolve(&self, time: WireTime) -> Result<u32, ModelError> {
        // Before time 0 the client's arithmetic wraps; the server never
        // means such a tick.
        time.resolve(self.clock.ok_or(ModelError::NoClock)?)
            .ok_or(ModelError::NoClock)
    }

    /// The time a 0xEA names. A game that was never given a time has its
    /// core's own: `pvp_match_core`'s init clears the world core to 50 ms
    /// (0x2D57F0 → 0x2D0550(0, 1): +0x18FC0 = +0x18FC4 = 50, last resync
    /// +0x18FD4 = 0). PROVEN. So the tick byte 0 is 0 ms there, which
    /// `leave_match` refuses as "older than last resync" (0x2D18B9,
    /// `time <= [+0x18FD4]`) before it looks at anything else.
    fn resolve_leave(&self, tick: u8) -> Result<u32, ModelError> {
        on_grid(tick)
            .resolve(self.clock.unwrap_or(INITIAL_CORE_MS))
            .ok_or(ModelError::NoClock)
    }

    /// Takes one message as the client would (`on_match_packet_received`
    /// 0x27A300). `dictionary` is the client's own item dictionary.
    pub fn receive(
        &mut self,
        id: u8,
        payload: &[u8],
        dictionary: &Dictionary,
    ) -> Result<Taken, ModelError> {
        // Before the static info the roster is not known; 0xED / 0xEE's
        // length is then read with a mask of no bytes, which the model
        // does not follow: they are skipped by state anyway.
        let roster = if self.growing_roster {
            MAX_PLAYERS as u8
        } else {
            self.roster().max(1).min(MAX_PLAYERS as u8)
        };
        if id == id::SERVER_UPDATE {
            return self.receive_update(payload);
        }
        let message = decode_server(id, payload, roster)?;
        let wanted = match id {
            id::PORTS => State::Connecting,
            id::STATIC_INFO => State::PortsKnown,
            _ => State::StaticInfoReceived,
        };
        if self.state != wanted {
            return Ok(Taken::Skipped(Skip::WrongState));
        }
        let mut time = None;
        let mut reply = None;
        match &message {
            ServerMessage::Ports { .. } => self.state = State::PortsKnown,
            ServerMessage::StaticInfo { ticks, .. } => {
                let (index, options) = static_info::parse_payload(payload, dictionary)
                    .map_err(ModelError::StaticInfo)?;
                self.local_index = Some(index);
                self.options = Some(options);
                self.ticks = *ticks;
                self.state = State::StaticInfoReceived;
            }
            ServerMessage::SetTime {
                generation,
                echo,
                match_time,
            } => {
                if !self.ready {
                    return Err(ModelError::SetTimeBeforeReady);
                }
                if !self.generation.takes_set_time(*generation) {
                    return Ok(Taken::Skipped(Skip::OldGeneration));
                }
                self.clock = Some(*match_time);
                self.world_loaded = false;
                reply = Some(TimeReply::to_set_time(*echo, *match_time, *match_time));
            }
            ServerMessage::Ping { a, b } => {
                reply = Some(TimeReply::to_ping(*a, *b, 0, self.clock.unwrap_or(0)));
            }
            ServerMessage::ClockOffset(offset) => {
                if self.clock_factor.is_some() {
                    // In place of the one it was taking: what was left of
                    // that is not made up (`set_offset` 0x1EE9E0 puts a
                    // new end, from now, for the new offset alone).
                    self.correction_left = f64::from(*offset);
                } else if let Some(clock) = &mut self.clock {
                    *clock = clock.saturating_add_signed((*offset).into());
                }
            }
            ServerMessage::Snapshot {
                generation,
                time: snapshot_time,
                body,
                ..
            } => {
                if !self.ready {
                    return Ok(Taken::Skipped(Skip::NoCore));
                }
                if !self.generation.takes_snapshot(*generation) {
                    return Ok(Taken::Skipped(Skip::OldGeneration));
                }
                if self.clock.is_none() {
                    return Err(ModelError::SnapshotWithoutTime);
                }
                if Sections::candidates(body).is_empty() {
                    return Err(ModelError::SnapshotState);
                }
                self.generation = Generation(*generation);
                self.last_resync = Some(*snapshot_time);
                self.world_loaded = true;
                self.syncs += 1;
                time = Some(*snapshot_time);
            }
            ServerMessage::UploadRequest => {
                if !self.patched {
                    return Ok(Taken::Skipped(Skip::NotPatched));
                }
                if !self.ready {
                    return Ok(Taken::Skipped(Skip::NoCore));
                }
            }
            // The one message at a tick a game with no clock is ever
            // sent: its own leaving, which ends the match for it.
            ServerMessage::PlayerLeft { tick, .. } => {
                if !self.ready {
                    return Ok(Taken::Skipped(Skip::NoCore));
                }
                let resolved = self.resolve_leave(*tick)?;
                if resolved <= self.last_resync.unwrap_or(0) {
                    return Ok(Taken::Skipped(Skip::OlderThanResync));
                }
                if self.clock.is_none() {
                    return Err(ModelError::NoClock);
                }
                time = Some(resolved);
            }
            // Everything else goes to the match core at a tick.
            other => {
                if !self.ready {
                    return Ok(Taken::Skipped(Skip::NoCore));
                }
                let at = match other {
                    ServerMessage::PlayerEntered { tick, .. }
                    | ServerMessage::ProfileInputRelay { tick, .. }
                    | ServerMessage::DiscardProfileInput { tick } => on_grid(*tick),
                    ServerMessage::MoveProfileInput { from_tick, .. } => on_grid(*from_tick),
                    ServerMessage::InputRelay { input, .. } => input.time,
                    ServerMessage::InputsRelay { time, .. }
                    | ServerMessage::DiscardInput { time } => *time,
                    ServerMessage::MoveInput { from, .. } => *from,
                    _ => unreachable!("handled above"),
                };
                let resolved = self.resolve(at)?;
                if self.last_resync.is_some_and(|resync| resolved <= resync) {
                    return Ok(Taken::Skipped(Skip::OlderThanResync));
                }
                // `enter_match` 0x2D1470: an entry older than the horizon
                // from its present, once its world is loaded.
                if matches!(other, ServerMessage::PlayerEntered { .. })
                    && self.world_loaded
                    && resolved + EVENT_HORIZON_MS <= self.clock.unwrap_or(0)
                {
                    return Ok(Taken::Skipped(Skip::TooOld));
                }
                time = Some(resolved);
            }
        }
        Ok(Taken::Applied(Applied {
            message,
            time,
            reply,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::super::static_info::{
        MatchItem, MatchKit, MatchProfile, NO_PROTECTION, SCORE_WEIGHTS, StaticInfo,
    };
    use super::*;

    fn dictionary() -> &'static Dictionary {
        Dictionary::shared()
    }

    fn player(name: &str, team: u8) -> MatchProfile {
        let mut kit = MatchKit::empty(1, "");
        kit.slots[7] = Some((0, 1));
        MatchProfile {
            team,
            name: name.into(),
            clan_tag: String::new(),
            protection: NO_PROTECTION,
            kits: vec![kit],
            items: vec![MatchItem {
                instance_id: 1,
                dict_id: dictionary().item_named("tt_33").unwrap().dict_id,
                condition_or_stack: 1_000_000,
                ..MatchItem::default()
            }],
            squad: 0,
        }
    }

    fn static_info(players: usize) -> StaticInfo {
        let options = MatchOptions {
            players: (0..players)
                .map(|index| player(&format!("p{index}"), (index % 2) as u8))
                .collect(),
            map_id: 61,
            match_id: 7,
            match_level: 0,
            score_weights: SCORE_WEIGHTS,
        };
        StaticInfo::new(&options, dictionary()).unwrap()
    }

    /// A world with no players: the mask and six empty sections.
    const EMPTY_WORLD: [u8; 16] = [0; 16];

    fn send(client: &mut ClientModel, message: &ServerMessage) -> Result<Taken, ModelError> {
        client.receive(message.id(), &message.payload(), dictionary())
    }

    fn applied(client: &mut ClientModel, message: &ServerMessage) -> Applied {
        match send(client, message) {
            Ok(Taken::Applied(applied)) => applied,
            other => panic!("{message:?} was not taken: {other:?}"),
        }
    }

    fn skipped(client: &mut ClientModel, message: &ServerMessage) -> Skip {
        match send(client, message) {
            Ok(Taken::Skipped(skip)) => skip,
            other => panic!("{message:?} was not skipped: {other:?}"),
        }
    }

    /// A client through the handshake, synced at `time`.
    fn synced(players: usize, time: u32) -> ClientModel {
        let mut client = ClientModel::new(true);
        applied(&mut client, &ServerMessage::Ports { min: 1, max: 1 });
        applied(&mut client, &static_info(players).message(0).unwrap());
        assert!(client.send_ready());
        applied(
            &mut client,
            &ServerMessage::SetTime {
                generation: 0,
                echo: time as u16,
                match_time: time + 350,
            },
        );
        applied(&mut client, &ServerMessage::snapshot(0, time, &EMPTY_WORLD));
        client
    }

    #[test]
    fn every_message_reads_back_as_it_was_written() {
        let time = WireTime { tick: 9, ms: None };
        let fine = WireTime {
            tick: 9,
            ms: Some(49),
        };
        let input = |time| PlayerInput {
            time,
            rotation: [1, 2, 3, 4],
            actions: 0x8000_0020,
        };
        let batch = |player| BatchInput {
            player,
            rotation: [player, 2, 3, 4],
            actions: u32::from(player) << 8,
        };
        let messages = [
            ServerMessage::Ports {
                min: 1236,
                max: 1240,
            },
            ServerMessage::StaticInfo {
                local_index: 3,
                lzma2: vec![1, 0, 0, 9, 0],
                ticks: None,
            },
            ServerMessage::StaticInfo {
                local_index: 3,
                lzma2: vec![1, 0, 0, 9, 0],
                ticks: Some(Grid::STOCK),
            },
            ServerMessage::SetTime {
                generation: 200,
                echo: 0xBEEF,
                match_time: 0x0102_0304,
            },
            ServerMessage::Ping { a: 1, b: 2 },
            ServerMessage::ClockOffset(-300),
            ServerMessage::snapshot(4, 12_345, &[1, 2, 3]),
            ServerMessage::PlayerEntered {
                tick: 1,
                player: 19,
            },
            ServerMessage::PlayerLeft { tick: 2, player: 0 },
            ServerMessage::InputRelay {
                player: 5,
                input: input(time),
            },
            ServerMessage::InputRelay {
                player: 5,
                input: input(fine),
            },
            ServerMessage::inputs_relay(time, 20, vec![batch(0), batch(8), batch(19)]).unwrap(),
            ServerMessage::inputs_relay(fine, 20, vec![batch(7), batch(16)]).unwrap(),
            ServerMessage::MoveInput {
                from: time,
                to: time,
            },
            ServerMessage::MoveInput {
                from: fine,
                to: fine,
            },
            ServerMessage::DiscardInput { time },
            ServerMessage::DiscardInput { time: fine },
            ServerMessage::ProfileInputRelay {
                tick: 3,
                player: 2,
                value: 1,
            },
            ServerMessage::MoveProfileInput {
                from_tick: 3,
                to_tick: 4,
            },
            ServerMessage::DiscardProfileInput { tick: 3 },
            ServerMessage::UploadRequest,
        ];
        let mut ids = Vec::new();
        for message in messages {
            let read = decode_server(message.id(), &message.payload(), 20);
            assert_eq!(read.as_ref(), Ok(&message));
            ids.push(message.id());
        }
        // Every server id was covered.
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids, (0xE3..=0xF6).collect::<Vec<u8>>());
    }

    #[test]
    fn wrong_lengths_ids_and_players_are_errors() {
        assert_eq!(
            decode_server(0xE2, &[], 2),
            Err(ModelError::UnknownId(0xE2))
        );
        assert_eq!(
            decode_server(0xDC, &[0; 9], 2),
            Err(ModelError::UnknownId(0xDC))
        );
        assert_eq!(
            decode_server(0xE5, &[0; 6], 2),
            Err(ModelError::Length { id: 0xE5, got: 6 })
        );
        assert_eq!(
            decode_server(0xEB, &[0; 11], 2),
            Err(ModelError::Length { id: 0xEB, got: 11 })
        );
        // Empty variable messages.
        assert!(decode_server(0xE4, &[], 2).is_err());
        assert!(decode_server(0xE8, &[0, 0], 2).is_err());
        assert!(decode_server(0xF6, &[], 2).is_err());
        // A state shorter than its time.
        assert_eq!(
            decode_server(0xE8, &[0, 0, 1, 2, 3], 2),
            Err(ModelError::SnapshotState)
        );
        // Player 2 in a roster of 2.
        assert_eq!(
            decode_server(0xE9, &[0, 2], 2),
            Err(ModelError::Player {
                player: 2,
                roster: 2
            })
        );
        // A batch of one player.
        let mut one = vec![0, 0b01];
        one.extend([0; 8]);
        assert_eq!(
            decode_server(0xED, &one, 2),
            Err(ModelError::Length { id: 0xED, got: 10 })
        );
        // A batch written for a roster of 8 read in one of 9: the mask is
        // a byte longer there.
        let batch = |player| BatchInput {
            player,
            rotation: [0; 4],
            actions: 0,
        };
        let time = WireTime { tick: 0, ms: None };
        let payload = ServerMessage::inputs_relay(time, 8, vec![batch(0), batch(1)])
            .unwrap()
            .payload();
        assert!(decode_server(0xED, &payload, 8).is_ok());
        assert!(decode_server(0xED, &payload, 9).is_err());
        // Milliseconds outside a tick.
        assert_eq!(
            decode_server(0xF2, &[0, 50], 2),
            Err(ModelError::Milliseconds(50))
        );
    }

    #[test]
    fn a_compressed_snapshot_is_unpacked() {
        let mut state = 500u32.to_le_bytes().to_vec();
        state.extend(EMPTY_WORLD);
        let mut payload = vec![3, 1];
        payload.extend(lzma2::stored(&state));
        assert_eq!(
            decode_server(0xE8, &payload, 2),
            Ok(ServerMessage::packed_snapshot(3, 500, &EMPTY_WORLD))
        );
        // Really compressed, as the server sends a world that shrinks.
        let world: Vec<u8> = EMPTY_WORLD.iter().copied().cycle().take(6_000).collect();
        let sent = ServerMessage::packed_snapshot(3, 500, &world);
        assert_eq!(sent.payload()[1], 1);
        assert_eq!(decode_server(0xE8, &sent.payload(), 2), Ok(sent));
        // Over the client's 0x8000-byte buffer.
        let mut payload = vec![3, 1];
        payload.extend(lzma2::stored(&vec![0; COMPRESSED_SNAPSHOT_BUFFER + 1]));
        assert_eq!(
            decode_server(0xE8, &payload, 2),
            Err(ModelError::SnapshotState)
        );
    }

    #[test]
    fn the_handshake_in_order() {
        let mut client = ClientModel::new(false);
        let info = static_info(2);
        // Out of order: static info before the ports, a relay before both.
        assert_eq!(
            skipped(&mut client, &info.message(0).unwrap()),
            Skip::WrongState
        );
        let entered = ServerMessage::PlayerEntered { tick: 0, player: 0 };
        assert_eq!(skipped(&mut client, &entered), Skip::WrongState);
        assert!(!client.send_ready());

        applied(&mut client, &ServerMessage::Ports { min: 1, max: 1 });
        assert_eq!(client.state, State::PortsKnown);
        assert_eq!(
            skipped(&mut client, &ServerMessage::Ports { min: 1, max: 1 }),
            Skip::WrongState
        );
        applied(&mut client, &info.message(1).unwrap());
        assert_eq!(client.state, State::StaticInfoReceived);
        assert_eq!(client.local_index, Some(1));
        assert_eq!(client.roster(), 2);

        // Before 0xDA: 0xE5 would crash it; the rest is skipped.
        let set_time = ServerMessage::SetTime {
            generation: 0,
            echo: 400,
            match_time: 400,
        };
        assert_eq!(
            send(&mut client, &set_time),
            Err(ModelError::SetTimeBeforeReady)
        );
        assert_eq!(skipped(&mut client, &entered), Skip::NoCore);
        assert!(client.send_ready());

        // A snapshot before any 0xE5.
        let world = ServerMessage::snapshot(0, 50, &EMPTY_WORLD);
        assert_eq!(
            send(&mut client, &world),
            Err(ModelError::SnapshotWithoutTime)
        );
        // 0xE5, answered by 0xDB, then 0xE8: the world runs.
        let reply = applied(&mut client, &set_time).reply.unwrap();
        assert_eq!(
            reply,
            TimeReply {
                first: 0,
                stamp: 400,
                client_ms: 400
            }
        );
        assert!(!client.is_synced());
        assert_eq!(applied(&mut client, &world).time, Some(50));
        assert!(client.is_synced());
        assert_eq!(client.generation, Generation(0));
        assert_eq!(client.syncs, 1);
        // A stock client skips the upload request.
        assert_eq!(
            skipped(&mut client, &ServerMessage::UploadRequest),
            Skip::NotPatched
        );
    }

    #[test]
    fn a_resync_needs_the_next_generation() {
        let mut client = synced(2, 50);
        client.advance(10_000);
        // The same generation again: the time is dropped, the snapshot
        // would be loaded onto the old clock. The server must step.
        let stale = ServerMessage::SetTime {
            generation: 0,
            echo: 0,
            match_time: 10_400,
        };
        assert_eq!(skipped(&mut client, &stale), Skip::OldGeneration);
        // Generation 1: 0xE5 then 0xE8.
        applied(
            &mut client,
            &ServerMessage::SetTime {
                generation: 1,
                echo: 0,
                match_time: 10_400,
            },
        );
        assert_eq!(client.clock, Some(10_400));
        applied(
            &mut client,
            &ServerMessage::snapshot(1, 10_050, &EMPTY_WORLD),
        );
        assert_eq!(client.generation, Generation(1));
        assert_eq!(client.syncs, 2);
        assert_eq!(client.last_resync, Some(10_050));
        // The first sync's snapshot, resent late, is skipped whole.
        let late = ServerMessage::snapshot(0, 50, &EMPTY_WORLD);
        assert_eq!(skipped(&mut client, &late), Skip::OldGeneration);
        assert_eq!(client.last_resync, Some(10_050));
    }

    #[test]
    fn ticks_are_read_on_the_clients_clock_and_old_ones_dropped() {
        // Synced at 10 000 ms with the clock at 10 350.
        let mut client = synced(4, 10_000);
        let relay = |time_ms: u32| ServerMessage::InputRelay {
            player: 1,
            input: PlayerInput {
                time: WireTime::tick(time_ms),
                rotation: [0; 4],
                actions: 1,
            },
        };
        assert_eq!(applied(&mut client, &relay(10_050)).time, Some(10_050));
        assert_eq!(applied(&mut client, &relay(10_400)).time, Some(10_400));
        // At or before the snapshot: "older than last resync".
        assert_eq!(skipped(&mut client, &relay(10_000)), Skip::OlderThanResync);
        assert_eq!(skipped(&mut client, &relay(9_950)), Skip::OlderThanResync);
        let entered = ServerMessage::PlayerEntered {
            tick: super::super::tick_byte(10_100),
            player: 3,
        };
        assert_eq!(applied(&mut client, &entered).time, Some(10_100));
        // A precise time keeps its milliseconds.
        let fine = ServerMessage::DiscardInput {
            time: WireTime::precise(10_123),
        };
        assert_eq!(applied(&mut client, &fine).time, Some(10_123));
        // More than 127 ticks from the clock, a tick is read as another:
        // 7 s ahead comes out 5.8 s behind, with no error.
        assert_eq!(skipped(&mut client, &relay(17_350)), Skip::OlderThanResync);
        // A player outside the roster is an error.
        let outsider = ServerMessage::PlayerLeft { tick: 0, player: 4 };
        assert!(matches!(
            send(&mut client, &outsider),
            Err(ModelError::Player { .. })
        ));
    }

    #[test]
    fn pings_and_clock_offsets() {
        let mut client = synced(2, 50);
        assert_eq!(client.clock, Some(400));
        let reply = applied(&mut client, &ServerMessage::Ping { a: 1_000, b: 42 })
            .reply
            .unwrap();
        assert_eq!(
            reply,
            TimeReply {
                first: 42,
                stamp: 1_000,
                client_ms: 400
            }
        );
        applied(&mut client, &ServerMessage::ClockOffset(-30));
        assert_eq!(client.clock, Some(370));
        // The patched client takes the upload request.
        applied(&mut client, &ServerMessage::UploadRequest);
    }

    #[test]
    fn a_clock_with_a_factor_takes_a_correction_over_time() {
        use super::super::clock::{FAST_FACTOR, STOCK_FACTOR, correction_ms};
        for factor in [STOCK_FACTOR, FAST_FACTOR] {
            let mut client = synced(2, 50);
            client.clock_factor = Some(factor);
            assert_eq!(client.clock, Some(400));
            applied(&mut client, &ServerMessage::ClockOffset(300));
            // Nothing at once; then the clock's own time and the factor's
            // share of it.
            assert_eq!(client.clock, Some(400));
            let takes = correction_ms(300, factor) as u32;
            let mut passed = 0;
            while passed + 50 <= takes / 2 {
                client.advance(50);
                passed += 50;
            }
            let made_up = i64::from(client.clock.unwrap()) - 400 - i64::from(passed);
            let expected = (f64::from(passed) * f64::from(factor)) as i64;
            assert!(
                (made_up - expected).abs() <= 1,
                "{factor}: {made_up} of {expected}"
            );
            client.advance(takes - passed);
            assert_eq!(client.clock, Some(400 + takes + 300), "{factor}");
            // Done: the clock goes on as time does.
            client.advance(1_000);
            assert_eq!(client.clock, Some(400 + takes + 300 + 1_000));
            assert_eq!(client.correction_left(), 0.0);
        }
        // Backwards the clock is slowed, and a correction that comes while
        // one runs takes its place.
        let mut client = synced(2, 50);
        client.clock_factor = Some(FAST_FACTOR);
        applied(&mut client, &ServerMessage::ClockOffset(-300));
        client.advance(500);
        assert_eq!(client.clock, Some(400 + 500 - 150));
        applied(&mut client, &ServerMessage::ClockOffset(30));
        client.advance(1_000);
        // (To a ms: a part of one was made up of the first and not yet on
        // the clock.)
        let clock = i64::from(client.clock.unwrap());
        assert!(
            (clock - (400 + 500 - 150 + 1_000 + 30)).abs() <= 1,
            "{clock}"
        );
        assert_eq!(client.correction_left(), 0.0);
    }

    #[test]
    fn static_info_that_would_harm_the_client_is_an_error() {
        let mut client = ClientModel::new(false);
        applied(&mut client, &ServerMessage::Ports { min: 1, max: 1 });
        // A local index outside the roster.
        let mut payload = static_info(2).payload(0).unwrap();
        payload[0] = 2;
        assert!(matches!(
            client.receive(0xE4, &payload, dictionary()),
            Err(ModelError::StaticInfo(StaticInfoError::LocalIndex { .. }))
        ));
        // A stream that is no LZMA2.
        assert!(matches!(
            client.receive(0xE4, &[0, 0xFF, 0xFF], dictionary()),
            Err(ModelError::StaticInfo(StaticInfoError::Lzma2))
        ));
        assert_eq!(client.state, State::PortsKnown);
    }

    #[test]
    fn a_snapshot_that_is_no_world_is_an_error() {
        let mut client = synced(2, 50);
        applied(
            &mut client,
            &ServerMessage::SetTime {
                generation: 1,
                echo: 0,
                match_time: 5_000,
            },
        );
        let broken = ServerMessage::snapshot(1, 4_650, &[1, 0, 0, 0, 9, 9, 9]);
        assert_eq!(send(&mut client, &broken), Err(ModelError::SnapshotState));
        assert_eq!(client.syncs, 1);
    }
}

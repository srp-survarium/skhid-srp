//! The 0.26g client's own match replay file, `<name>.sur` (`docs/notes/
//! replay_sur.md`): written here, read by the game with
//! `-match_replay=<name>`.
//!
//! Everything below was read from the client's reader (`replay_match_reader`,
//! exe VAs in the note); no file made here has been played yet. The layout:
//!
//! ```text
//! file  = chunk*                      any order, every id 0..=4 once
//! chunk = u32 id, u32 size (top 2 bits: flags, ignored), size bytes
//!
//! 0  raw      cstr (skipped), cstr "0.26g0", u32 8
//! 1  LZMA2    the match options: a 0xE4 payload without its first byte
//! 2  LZMA2    the body: records, below
//! 3  u16 count, LZMA2 of `count` key frames: u16 len, u32 body_offset, data
//! 4  LZMA2    u32 len, len bytes: the mode's `.options` file
//!
//! record = u8 type, u32 time_ms, then by type
//!   0        end of this tick's reading (the time is the reader's cursor)
//!   4        a time marker
//!   5..=20   f = type - 5: [f&8: u32 mask, a u8 per set bit]   kit choices
//!                          [f&4: u32 mask]                     entered
//!                          [f&2: u32 mask]                     left
//!                          [f&1: u32 digits, a u32/u64 per non-zero digit]
//! ```
//!
//! The digits are 20 base-3 numerals, player 0 the most significant; 0 no
//! input, 1 a u32 (the rotation only), 2 a u64 (rotation, then the actions
//! mask): [`Writer`] writes only u64s.
//!
//! The game applies a record the moment it reads it, as soon as the reader's
//! cursor (the last record's time) is behind the end of the 50 ms tick being
//! played; so a record must never be read before its own tick. [`Writer`]
//! puts a type 0 record, timed at the start of the next tick that has
//! records, after every tick's records: it stops the reading there.
//!
//! Limits the client does not check (past them it writes over memory):
//! at most [`MAX_KEY_FRAMES`] key frames, and the decoded chunks together
//! under [`MAX_DECODED`] bytes.

use survarium::v026g::lzma2;

/// `replay_match_reader`'s check of chunk 0 (0x1F0647, the string at
/// 0x9DAE98).
pub const ENGINE_VERSION: &str = "0.26g0";
/// The format version the reader demands of chunk 0 and hands to the
/// world's `deserialize` for every key frame.
pub const FORMAT_VERSION: u32 = 8;
/// The reader's key frame vector holds 128.
pub const MAX_KEY_FRAMES: usize = 128;
/// The decoded chunks together go into one 8 MiB area.
pub const MAX_DECODED: usize = 8 << 20;
/// A key frame's data length is a u16.
pub const MAX_KEY_FRAME_DATA: usize = 0xFFFF;
/// A tick of the game's clock.
pub const TICK_MS: u32 = 50;
/// Players a record can name.
pub const MAX_PLAYERS: usize = 20;

/// The chunk ids.
pub mod chunk {
    pub const VERSION: u32 = 0;
    pub const OPTIONS: u32 = 1;
    pub const BODY: u32 = 2;
    pub const KEY_FRAMES: u32 = 3;
    pub const CONFIG: u32 = 4;
}

/// Record types.
pub mod record_type {
    pub const END_OF_TICK: u8 = 0;
    pub const MARKER: u8 = 4;
    /// Type of the record with the flag set `f`: `FIRST_EVENT + f`.
    pub const FIRST_EVENT: u8 = 5;
    pub const PROFILE: u8 = 8;
    pub const ENTER: u8 = 4;
    pub const LEAVE: u8 = 2;
    pub const INPUTS: u8 = 1;
}

/// Why a file cannot be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SurError {
    TooManyKeyFrames(usize),
    KeyFrameTooLong(usize),
    TooLarge(usize),
    Player(u8),
}

impl std::fmt::Display for SurError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooManyKeyFrames(n) => write!(f, "{n} key frames, the client holds 128"),
            Self::KeyFrameTooLong(n) => write!(f, "a key frame of {n} bytes, a u16 holds 65535"),
            Self::TooLarge(n) => write!(f, "{n} decoded bytes, the client has 8 MiB"),
            Self::Player(p) => write!(f, "player {p}, a record names 0..=19"),
        }
    }
}

impl std::error::Error for SurError {}

/// One player's input in a record: what 0xEB relays as `half2 rotation,
/// u32 actions_mask`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputValue {
    pub rotation: [u8; 4],
    pub actions: u32,
}

/// Everything that happens at one time.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Event {
    pub time: u32,
    /// Players that become active.
    pub enter: u32,
    /// Players that leave.
    pub leave: u32,
    /// `(player, value)` kit choices (0xF3).
    pub profiles: Vec<(u8, u8)>,
    /// `(player, input)`.
    pub inputs: Vec<(u8, InputValue)>,
}

impl Event {
    pub fn at(time: u32) -> Self {
        Self {
            time,
            ..Self::default()
        }
    }

    fn flags(&self) -> u8 {
        let mut f = 0;
        if !self.profiles.is_empty() {
            f |= record_type::PROFILE;
        }
        if self.enter != 0 {
            f |= record_type::ENTER;
        }
        if self.leave != 0 {
            f |= record_type::LEAVE;
        }
        if !self.inputs.is_empty() {
            f |= record_type::INPUTS;
        }
        f
    }
}

/// The start of the tick a time is in.
pub fn tick_start(time: u32) -> u32 {
    time / TICK_MS * TICK_MS
}

/// The body: records, in time order.
#[derive(Default)]
pub struct Writer {
    bytes: Vec<u8>,
    /// Where the last record written starts, and its time.
    last_time: Option<u32>,
    /// Whether the last record was the end-of-tick marker of the previous
    /// events.
    open_tick: Option<u32>,
}

impl Writer {
    /// A body that starts at `start`: the first record is a marker with
    /// that time, which is all the game takes from it.
    pub fn new(start: u32) -> Self {
        let mut writer = Self::default();
        writer.bytes.push(record_type::MARKER);
        writer.bytes.extend(start.to_le_bytes());
        writer.last_time = Some(start);
        writer
    }

    /// Where the next record will start.
    pub fn offset(&self) -> usize {
        self.bytes.len()
    }

    /// Appends the records of one time. Times must not go back. A tick's
    /// records are closed by [`Writer::finish`] or by the first event of a
    /// later tick (a type 0 record timed at that tick's start).
    pub fn push(&mut self, event: &Event) -> Result<(), SurError> {
        debug_assert!(self.last_time.is_none_or(|t| event.time >= t));
        let tick = tick_start(event.time);
        if self.open_tick.is_some_and(|open| open != tick) {
            self.close(tick);
        }
        self.open_tick = Some(tick);
        self.last_time = Some(event.time);
        let f = event.flags();
        self.bytes.push(record_type::FIRST_EVENT + f);
        self.bytes.extend(event.time.to_le_bytes());
        if f & record_type::PROFILE != 0 {
            let mut profiles = event.profiles.clone();
            profiles.sort_by_key(|&(player, _)| player);
            profiles.dedup_by_key(|&mut (player, _)| player);
            let mut mask = 0u32;
            for &(player, _) in &profiles {
                if usize::from(player) >= MAX_PLAYERS {
                    return Err(SurError::Player(player));
                }
                mask |= 1 << player;
            }
            self.bytes.extend(mask.to_le_bytes());
            self.bytes.extend(profiles.iter().map(|&(_, value)| value));
        }
        if f & record_type::ENTER != 0 {
            self.bytes.extend(event.enter.to_le_bytes());
        }
        if f & record_type::LEAVE != 0 {
            self.bytes.extend(event.leave.to_le_bytes());
        }
        if f & record_type::INPUTS != 0 {
            let mut inputs = event.inputs.clone();
            inputs.sort_by_key(|&(player, _)| player);
            inputs.dedup_by_key(|&mut (player, _)| player);
            let mut digits = [0u8; MAX_PLAYERS];
            for &(player, _) in &inputs {
                if usize::from(player) >= MAX_PLAYERS {
                    return Err(SurError::Player(player));
                }
                digits[usize::from(player)] = 2;
            }
            // Player 0 is the most significant digit.
            let number = digits
                .iter()
                .fold(0u32, |number, &digit| number * 3 + u32::from(digit));
            self.bytes.extend(number.to_le_bytes());
            for (_, value) in &inputs {
                self.bytes.extend(value.rotation);
                self.bytes.extend(value.actions.to_le_bytes());
            }
        }
        Ok(())
    }

    /// Stops the reading of the tick being played: a type 0 record with the
    /// time `next_start` (the start of the next tick with records, or the
    /// end of the replay).
    fn close(&mut self, next_start: u32) {
        self.bytes.push(record_type::END_OF_TICK);
        self.bytes.extend(next_start.to_le_bytes());
        self.last_time = Some(next_start);
        self.open_tick = None;
    }

    /// Ends the body at `end` (raised to the end of the tick being played):
    /// a last type 0 record with that time, which the game takes as the
    /// replay's end time.
    pub fn finish(mut self, end: u32) -> Vec<u8> {
        let floor = self
            .open_tick
            .map_or(0, |tick| tick + TICK_MS)
            .max(self.last_time.unwrap_or(0));
        self.close(end.max(floor));
        self.bytes
    }
}

/// A record read back (for tests and for checking a file's own body).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadRecord {
    pub offset: usize,
    pub kind: u8,
    pub event: Event,
}

/// Reads a body as the client's `0x1F0BE0` does. `None` if it is cut or has
/// a type 1..=3.
pub fn read_body(body: &[u8]) -> Option<Vec<ReadRecord>> {
    let mut records = Vec::new();
    let mut at = 0usize;
    let u32_at = |at: &mut usize| -> Option<u32> {
        let bytes = body.get(*at..*at + 4)?;
        *at += 4;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    };
    while at < body.len() {
        let offset = at;
        let kind = body[at];
        at += 1;
        let time = u32_at(&mut at)?;
        let mut event = Event::at(time);
        if kind == record_type::END_OF_TICK || kind == record_type::MARKER {
            records.push(ReadRecord {
                offset,
                kind,
                event,
            });
            continue;
        }
        if !(record_type::FIRST_EVENT..record_type::FIRST_EVENT + 16).contains(&kind) {
            return None;
        }
        let f = kind - record_type::FIRST_EVENT;
        if f & record_type::PROFILE != 0 {
            let mask = u32_at(&mut at)?;
            for player in 0..MAX_PLAYERS as u8 {
                if mask & (1 << player) != 0 {
                    event.profiles.push((player, *body.get(at)?));
                    at += 1;
                }
            }
        }
        if f & record_type::ENTER != 0 {
            event.enter = u32_at(&mut at)?;
        }
        if f & record_type::LEAVE != 0 {
            event.leave = u32_at(&mut at)?;
        }
        if f & record_type::INPUTS != 0 {
            let number = u32_at(&mut at)?;
            // The client reverses the digits, then takes them from the
            // least significant: player 0 is the last digit it extracted.
            let mut digits = [0u8; MAX_PLAYERS];
            let mut rest = number;
            for slot in digits.iter_mut().rev() {
                *slot = (rest % 3) as u8;
                rest /= 3;
            }
            for (player, &digit) in digits.iter().enumerate() {
                let value = match digit {
                    0 => continue,
                    1 => {
                        let rotation: [u8; 4] = body.get(at..at + 4)?.try_into().ok()?;
                        at += 4;
                        InputValue {
                            rotation,
                            actions: 0,
                        }
                    }
                    _ => {
                        let rotation: [u8; 4] = body.get(at..at + 4)?.try_into().ok()?;
                        let actions =
                            u32::from_le_bytes(body.get(at + 4..at + 8)?.try_into().ok()?);
                        at += 8;
                        InputValue { rotation, actions }
                    }
                };
                event.inputs.push((player as u8, value));
            }
        }
        records.push(ReadRecord {
            offset,
            kind,
            event,
        });
    }
    Some(records)
}

/// A key frame: the world at `data[..4]` (u32 ms), and where in the body
/// the reading goes on after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyFrame {
    pub body_offset: u32,
    /// `u32 time_ms` and the 0xE8 world body (an upload's bytes).
    pub data: Vec<u8>,
}

impl KeyFrame {
    pub fn new(time: u32, body: &[u8], body_offset: usize) -> Self {
        let mut data = time.to_le_bytes().to_vec();
        data.extend_from_slice(body);
        Self {
            body_offset: body_offset as u32,
            data,
        }
    }

    pub fn time(&self) -> u32 {
        u32::from_le_bytes([self.data[0], self.data[1], self.data[2], self.data[3]])
    }
}

/// How the LZMA2 chunks are made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coding {
    /// Stored chunks: no encoder; what every other LZMA2 payload of the
    /// server went as until 2026-10-13.
    Stored,
    /// Compressed chunks, as the worlds of 0xE8 go to the clients.
    Packed,
}

impl Coding {
    fn encode(self, data: &[u8]) -> Vec<u8> {
        match self {
            Self::Stored => lzma2::stored(data),
            Self::Packed => lzma2::packed(data),
        }
    }
}

fn push_chunk(out: &mut Vec<u8>, id: u32, bytes: &[u8]) {
    out.extend(id.to_le_bytes());
    out.extend((bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// The pieces of a file.
pub struct Parts<'a> {
    /// A 0xE4 payload without its first byte: a raw LZMA2 stream.
    pub options_lzma2: &'a [u8],
    /// The body, [`Writer::finish`]ed.
    pub body: &'a [u8],
    /// In time order, at most [`MAX_KEY_FRAMES`].
    pub key_frames: &'a [KeyFrame],
    /// The mode's `.options` file as it is in the game's resources.
    pub config: &'a [u8],
}

/// The decoded size of everything the reader unpacks, checked against what
/// it holds.
pub fn decoded_size(options: usize, parts: &Parts<'_>) -> usize {
    let frames: usize = parts.key_frames.iter().map(|k| 6 + k.data.len()).sum();
    options + parts.body.len() + frames + 4 + parts.config.len()
}

/// Makes the file. `options_decoded` is the length of the options once
/// unpacked (only for the 8 MiB check).
pub fn write(
    parts: &Parts<'_>,
    options_decoded: usize,
    coding: Coding,
) -> Result<Vec<u8>, SurError> {
    if parts.key_frames.len() > MAX_KEY_FRAMES {
        return Err(SurError::TooManyKeyFrames(parts.key_frames.len()));
    }
    if let Some(frame) = parts
        .key_frames
        .iter()
        .find(|frame| frame.data.len() > MAX_KEY_FRAME_DATA)
    {
        return Err(SurError::KeyFrameTooLong(frame.data.len()));
    }
    let decoded = decoded_size(options_decoded, parts);
    if decoded >= MAX_DECODED {
        return Err(SurError::TooLarge(decoded));
    }
    let mut out = Vec::new();

    let mut version = vec![0u8]; // the first string, empty
    version.extend(ENGINE_VERSION.as_bytes());
    version.push(0);
    version.extend(FORMAT_VERSION.to_le_bytes());
    push_chunk(&mut out, chunk::VERSION, &version);

    push_chunk(&mut out, chunk::OPTIONS, parts.options_lzma2);
    push_chunk(&mut out, chunk::BODY, &coding.encode(parts.body));

    let mut table = Vec::new();
    for frame in parts.key_frames {
        table.extend((frame.data.len() as u16).to_le_bytes());
        table.extend(frame.body_offset.to_le_bytes());
        table.extend_from_slice(&frame.data);
    }
    let mut frames = (parts.key_frames.len() as u16).to_le_bytes().to_vec();
    frames.extend(coding.encode(&table));
    push_chunk(&mut out, chunk::KEY_FRAMES, &frames);

    let mut config = (parts.config.len() as u32).to_le_bytes().to_vec();
    config.extend_from_slice(parts.config);
    push_chunk(&mut out, chunk::CONFIG, &coding.encode(&config));
    Ok(out)
}

/// A file read back into its chunks (tests, and a check of a made file).
pub fn chunks(file: &[u8]) -> Option<Vec<(u32, &[u8])>> {
    let mut at = 0usize;
    let mut out = Vec::new();
    while at < file.len() {
        let id = u32::from_le_bytes(file.get(at..at + 4)?.try_into().ok()?);
        let word = u32::from_le_bytes(file.get(at + 4..at + 8)?.try_into().ok()?);
        let size = (word & 0x3FFF_FFFF) as usize;
        out.push((id, file.get(at + 8..at + 8 + size)?));
        at += 8 + size;
    }
    Some(out)
}

/// What [`verify`] found in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verified {
    pub records: usize,
    pub start_ms: u32,
    pub end_ms: u32,
    pub key_frames: Vec<(u32, u32)>,
    pub decoded_bytes: usize,
}

/// Reads a file the way the client's reader does and checks what the reader
/// does not: every chunk once, the version chunk, a body that parses with a
/// marker first and an end record last, key frames in time order that point
/// at record boundaries, the decoded total under 8 MiB.
pub fn verify(file: &[u8]) -> Result<Verified, String> {
    let all = chunks(file).ok_or("a chunk runs past the end")?;
    let find = |id: u32| -> Result<&[u8], String> {
        let mut found = all.iter().filter(|(c, _)| *c == id);
        let one = found.next().ok_or(format!("no chunk {id}"))?;
        if found.next().is_some() {
            return Err(format!("chunk {id} twice"));
        }
        Ok(one.1)
    };
    if all.len() != 5 {
        return Err(format!("{} chunks", all.len()));
    }
    let mut version = vec![0u8];
    version.extend(ENGINE_VERSION.as_bytes());
    version.push(0);
    version.extend(FORMAT_VERSION.to_le_bytes());
    if find(chunk::VERSION)? != version {
        return Err("chunk 0 is not the version chunk".into());
    }
    let unpack = |bytes: &[u8]| lzma2::unpack(bytes, MAX_DECODED).ok_or("a chunk does not unpack");
    let options = unpack(find(chunk::OPTIONS)?)?;
    let body = unpack(find(chunk::BODY)?)?;
    let config = unpack(find(chunk::CONFIG)?)?;
    let frames_chunk = find(chunk::KEY_FRAMES)?;
    let count = usize::from(u16::from_le_bytes([
        *frames_chunk.first().ok_or("short chunk 3")?,
        *frames_chunk.get(1).ok_or("short chunk 3")?,
    ]));
    let table = unpack(&frames_chunk[2..])?;
    if config.len() < 4
        || u32::from_le_bytes(config[..4].try_into().unwrap()) as usize != config.len() - 4
    {
        return Err("chunk 4's length is wrong".into());
    }
    let decoded = options.len() + body.len() + table.len() + config.len();
    if decoded >= MAX_DECODED {
        return Err(format!("{decoded} decoded bytes"));
    }
    if count > MAX_KEY_FRAMES {
        return Err(format!("{count} key frames"));
    }
    let records = read_body(&body).ok_or("the body does not parse")?;
    let first = records.first().ok_or("an empty body")?;
    if first.kind != record_type::MARKER {
        return Err("the body does not start with a marker".into());
    }
    let last = records.last().expect("one");
    if last.kind != record_type::END_OF_TICK {
        return Err("the body does not end with a type 0 record".into());
    }
    let boundaries: std::collections::HashSet<usize> = records
        .iter()
        .map(|r| r.offset)
        .chain([body.len()])
        .collect();
    let mut frames = Vec::new();
    let mut at = 0usize;
    for _ in 0..count {
        let len = usize::from(u16::from_le_bytes(
            table
                .get(at..at + 2)
                .ok_or("cut table")?
                .try_into()
                .unwrap(),
        ));
        let offset = u32::from_le_bytes(
            table
                .get(at + 2..at + 6)
                .ok_or("cut table")?
                .try_into()
                .unwrap(),
        );
        let data = table.get(at + 6..at + 6 + len).ok_or("cut key frame")?;
        at += 6 + len;
        if len < 4 {
            return Err("a key frame without a time".into());
        }
        if !boundaries.contains(&(offset as usize)) {
            return Err(format!("a key frame points into a record ({offset})"));
        }
        let time = u32::from_le_bytes(data[..4].try_into().unwrap());
        if frames.last().is_some_and(|&(before, _)| time <= before) {
            return Err("key frames out of order".into());
        }
        // Nothing after the key frame's time may come before its offset.
        if records.iter().any(|r| {
            r.offset < offset as usize && r.kind != record_type::MARKER && r.event.time > time
        }) {
            return Err(format!("a key frame at {time} skips a later record"));
        }
        frames.push((time, offset));
    }
    if at != table.len() {
        return Err("bytes after the key frames".into());
    }
    Ok(Verified {
        records: records.len(),
        start_ms: first.event.time,
        end_ms: last.event.time,
        key_frames: frames,
        decoded_bytes: decoded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(seed: u8) -> InputValue {
        InputValue {
            rotation: [seed, seed + 1, seed + 2, seed + 3],
            actions: 0x1000_0000 | u32::from(seed),
        }
    }

    #[test]
    fn a_body_reads_back() {
        let mut writer = Writer::new(50);
        let mut one = Event::at(100);
        one.enter = 0b101;
        let mut two = Event::at(120);
        two.inputs = vec![(7, input(7)), (0, input(0)), (19, input(19))];
        two.profiles = vec![(3, 1), (1, 2)];
        let mut three = Event::at(300);
        three.leave = 1 << 5;
        for event in [&one, &two, &three] {
            writer.push(event).unwrap();
        }
        let body = writer.finish(400);
        let records = read_body(&body).unwrap();
        let kinds: Vec<_> = records.iter().map(|r| (r.kind, r.event.time)).collect();
        assert_eq!(
            kinds,
            [
                (4, 50),
                (5 + 4, 100),
                (5 + 8 + 1, 120),
                // closes the tick of 100 and 120 at the next tick with records
                (0, 300),
                (5 + 2, 300),
                (0, 400),
            ]
        );
        assert_eq!(records[1].event.enter, 0b101);
        assert_eq!(records[2].event.profiles, [(1, 2), (3, 1)]);
        let players: Vec<u8> = records[2].event.inputs.iter().map(|i| i.0).collect();
        assert_eq!(players, [0, 7, 19]);
        assert_eq!(records[2].event.inputs[1].1, input(7));
        assert_eq!(records[4].event.leave, 1 << 5);
    }

    #[test]
    fn the_byte_layout_of_a_record() {
        let mut writer = Writer::new(0);
        let mut event = Event::at(0x0102_0304);
        event.inputs = vec![(0, input(1)), (2, input(9))];
        writer.push(&event).unwrap();
        let body = writer.finish(0x0102_0304);
        // marker: 04, time 0
        assert_eq!(&body[..5], [4, 0, 0, 0, 0]);
        // type 5 + 1, time, digits, two u64
        assert_eq!(body[5], 6);
        assert_eq!(&body[6..10], 0x0102_0304u32.to_le_bytes());
        // player 0 and 2 have digit 2: 2 * 3^19 + 2 * 3^17
        let digits = 2 * 3u32.pow(19) + 2 * 3u32.pow(17);
        assert_eq!(&body[10..14], digits.to_le_bytes());
        assert_eq!(&body[14..18], [1, 2, 3, 4]);
        assert_eq!(&body[18..22], 0x1000_0001u32.to_le_bytes());
        assert_eq!(&body[22..26], [9, 10, 11, 12]);
        assert_eq!(&body[26..30], 0x1000_0009u32.to_le_bytes());
        assert_eq!(body.len(), 5 + 25 + 5);
    }

    #[test]
    fn the_number_of_digits_matches_the_clients_loop() {
        // The client's two loops (0x1F0C95..0x1F0D0D): reverse 20 base-3
        // digits, then take them least significant first, player 0 first.
        fn client(number: u32) -> Vec<(usize, u32)> {
            let (mut reversed, mut rest) = (0u32, number);
            for _ in 0..20 {
                reversed = rest % 3 + reversed * 3;
                rest /= 3;
            }
            let mut out = Vec::new();
            let (mut rest, mut player) = (reversed, 0);
            while rest != 0 {
                if rest % 3 != 0 {
                    out.push((player, rest % 3));
                }
                rest /= 3;
                player += 1;
            }
            out
        }
        let mut writer = Writer::new(0);
        let mut event = Event::at(10);
        event.inputs = vec![(1, input(1)), (4, input(4)), (19, input(19))];
        writer.push(&event).unwrap();
        let body = writer.finish(10);
        let number = u32::from_le_bytes(body[10..14].try_into().unwrap());
        assert_eq!(client(number), [(1, 2), (4, 2), (19, 2)]);
    }

    #[test]
    fn a_tick_is_closed_before_the_next_ticks_records() {
        let mut writer = Writer::new(0);
        writer.push(&Event::at(10)).unwrap();
        writer.push(&Event::at(40)).unwrap();
        writer.push(&Event::at(45)).unwrap();
        writer.push(&Event::at(1000)).unwrap();
        let body = writer.finish(1000);
        let kinds: Vec<_> = read_body(&body)
            .unwrap()
            .iter()
            .map(|r| (r.kind, r.event.time))
            .collect();
        assert_eq!(
            kinds,
            [
                (4, 0),
                (5, 10),
                (5, 40),
                (5, 45),
                (0, 1000),
                (5, 1000),
                (0, 1050)
            ]
        );
    }

    #[test]
    fn a_file_has_its_five_chunks() {
        let options = lzma2::stored(b"options");
        let mut writer = Writer::new(0);
        writer.push(&Event::at(50)).unwrap();
        let body = writer.finish(100);
        let frames = [KeyFrame::new(50, b"world", 5)];
        let parts = Parts {
            options_lzma2: &options,
            body: &body,
            key_frames: &frames,
            config: b"config",
        };
        for coding in [Coding::Stored, Coding::Packed] {
            let file = write(&parts, 7, coding).unwrap();
            let chunks = chunks(&file).unwrap();
            let ids: Vec<u32> = chunks.iter().map(|c| c.0).collect();
            assert_eq!(ids, [0, 1, 2, 3, 4]);
            // chunk 0: "", "0.26g0", 8
            assert_eq!(
                chunks[0].1,
                b"\0"
                    .iter()
                    .chain(b"0.26g0\0")
                    .chain(&8u32.to_le_bytes())
                    .copied()
                    .collect::<Vec<u8>>()
            );
            assert_eq!(chunks[1].1, options);
            assert_eq!(lzma2::unpack(chunks[2].1, 1 << 20).unwrap(), body);
            // chunk 3: u16 count, then the table
            assert_eq!(&chunks[3].1[..2], 1u16.to_le_bytes());
            let table = lzma2::unpack(&chunks[3].1[2..], 1 << 20).unwrap();
            assert_eq!(
                u16::from_le_bytes([table[0], table[1]]) as usize,
                frames[0].data.len()
            );
            assert_eq!(u32::from_le_bytes(table[2..6].try_into().unwrap()), 5);
            assert_eq!(&table[6..], frames[0].data);
            assert_eq!(&table[6..10], 50u32.to_le_bytes());
            let config = lzma2::unpack(chunks[4].1, 1 << 20).unwrap();
            assert_eq!(&config[..4], 6u32.to_le_bytes());
            assert_eq!(&config[4..], b"config");
        }
    }

    #[test]
    fn the_clients_limits_are_kept() {
        let options = lzma2::stored(b"o");
        let frames: Vec<_> = (0..129).map(|i| KeyFrame::new(i, b"w", 0)).collect();
        let parts = Parts {
            options_lzma2: &options,
            body: &[],
            key_frames: &frames,
            config: b"c",
        };
        assert_eq!(
            write(&parts, 1, Coding::Stored),
            Err(SurError::TooManyKeyFrames(129))
        );
        let big = [KeyFrame::new(0, &vec![0; 0x10000], 0)];
        let parts = Parts {
            key_frames: &big,
            ..parts
        };
        assert!(matches!(
            write(&parts, 1, Coding::Stored),
            Err(SurError::KeyFrameTooLong(_))
        ));
    }

    #[test]
    fn the_largest_upload_is_one_key_frame() {
        // Patch 60: a world of up to 0xFF00 bytes; with its u32 time it is
        // one key frame (a u16 length), and the transport takes its chunks.
        use survarium::v026g::match_msgs::upload::MAX_TOTAL;
        const { assert!(4 + MAX_TOTAL <= MAX_KEY_FRAME_DATA) };
        assert_eq!(udp_transport::wire::UPLOAD_MAX_TOTAL, MAX_TOTAL);
        assert_eq!(KeyFrame::new(50, &vec![7; MAX_TOTAL], 0).data.len(), 0xFF04);
    }
}

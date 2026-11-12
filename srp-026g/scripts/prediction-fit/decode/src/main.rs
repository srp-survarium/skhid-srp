//! Match recordings (`.srprec`, `match_server::v026g::record`) made into
//! per-player input timelines and world positions, for fitting the
//! prediction of other players' inputs (`docs/notes/prediction_fit.md`).
//!
//! ```text
//! prediction-decode <out dir> <file.srprec>...
//! ```
//!
//! For each recording `<stem>`, three files in `<out dir>/<stem>/`:
//! - `inputs.bin`: every input a client sent (0xDC, 0xDD), the first copy
//!   of each (place, game ms), sorted by place and time; records of
//!   [`INPUT_BYTES`]: u8 place, u8 precise (0xDD), u16 0, u32 game ms,
//!   i32 arrival on the match clock (ms), f32 yaw turn, f32 pitch turn (the
//!   two halves, radians), u32 actions (bit 31, the reader's flag, cleared);
//! - `worlds.bin`: each player of each uploaded world (0xE0 chunks, the
//!   upload most clients agree on per world time); records of
//!   [`WORLD_BYTES`]: u32 world ms, u8 place, u8 alive, u8 flags (bit 0
//!   crouched capsule, 1 sprinting, 2 jumping, 3 body read), u8 uploads
//!   agreeing, u32 inserted ms, f32 x, y, z (feet, map frame), f32 yaw,
//!   f32 pitch, u32 actions, f32 horizontal speed (NaN: not read);
//! - `meta.json`: the map, the mode, each place's team, whether it is a
//!   server bot, its round trip (acks), and the counts.
//!
//! No account or name is written: places only.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Mutex;

use match_server::v026g::record::{self, IN, NOTE, OUT};
use serde_json::{Value, json};
use survarium::v026g::match_msgs::motion::Motion;
use survarium::v026g::match_msgs::upload::{Chunk, Pushed, Reassembler};
use survarium::v026g::match_msgs::{self, ClientMessage, players};
use udp_transport::table::{Direction, Table};
use udp_transport::wire;

const INPUT_BYTES: usize = 24;
const WORLD_BYTES: usize = 44;
/// Bit 31 of an actions mask is a flag the client's reader splits off.
const INPUT_FLAG: u32 = 1 << 31;
/// An ack this long after the datagram is a seq number come round again.
const ACK_REACH_MS: u32 = 2_000;

/// An IEEE half float's value.
fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exponent = i32::from((bits >> 10) & 0x1F);
    let mantissa = f32::from(bits & 0x3FF);
    match exponent {
        0 => sign * mantissa * 2f32.powi(-24),
        31 if mantissa == 0.0 => sign * f32::INFINITY,
        31 => f32::NAN,
        _ => sign * (1.0 + mantissa / 1024.0) * 2f32.powi(exponent - 15),
    }
}

/// One input as the server first got it.
struct Input {
    precise: bool,
    arrival: i64,
    yaw: f32,
    pitch: f32,
    actions: u32,
}

#[derive(Default)]
struct Place {
    /// The seqs of the datagrams sent to it, when.
    sent: HashMap<u8, u32>,
    round_trips: Vec<u32>,
    uploads: Reassembler,
    datagrams: u64,
    revision: Option<u64>,
}

#[derive(Default)]
struct Recording {
    header: Option<Value>,
    /// Recording ms less match ms, from the clock note.
    offset: Option<i64>,
    /// Datagrams from the clients before the clock note.
    early: Vec<(u8, u32, Vec<u8>)>,
    places: BTreeMap<u8, Place>,
    inputs: BTreeMap<(u8, u32), Input>,
    input_copies: u64,
    input_conflicts: u64,
    unparsed: u64,
    worlds: BTreeMap<u32, Vec<Vec<u8>>>,
    notes: Vec<Value>,
}

impl Recording {
    fn record(&mut self, kind: u8, place: u8, at_ms: u32, bytes: &[u8]) {
        match kind {
            NOTE => {
                let Ok(note) = serde_json::from_slice::<Value>(bytes) else {
                    return;
                };
                match note["note"].as_str() {
                    Some("header") => self.header = Some(note),
                    Some("clock") if self.offset.is_none() => {
                        if let Some(ms) = note["match_ms"].as_i64() {
                            self.offset = Some(i64::from(at_ms) - ms);
                            for (place, at, bytes) in std::mem::take(&mut self.early) {
                                self.incoming(place, at, &bytes);
                            }
                        }
                    }
                    Some("revision") => {
                        if let (Some(p), Some(r)) =
                            (note["place"].as_u64(), note["revision"].as_u64())
                        {
                            self.places.entry(p as u8).or_default().revision = Some(r);
                        }
                    }
                    Some("entered" | "left" | "places" | "result") => {
                        let mut kept = json!({"note": note["note"], "at_ms": at_ms});
                        for key in ["place", "match_ms"] {
                            if let Some(v) = note.get(key) {
                                kept[key] = v.clone();
                            }
                        }
                        self.notes.push(kept);
                    }
                    _ => {}
                }
            }
            OUT => {
                if let Some(&seq) = bytes.first() {
                    self.places
                        .entry(place)
                        .or_default()
                        .sent
                        .insert(seq, at_ms);
                }
            }
            IN => {
                if self.offset.is_none() {
                    self.early.push((place, at_ms, bytes.to_vec()));
                } else {
                    self.incoming(place, at_ms, bytes);
                }
            }
            _ => {}
        }
    }

    fn incoming(&mut self, place: u8, at_ms: u32, bytes: &[u8]) {
        let offset = self.offset.unwrap_or(0);
        let Ok(datagram) = wire::parse(Table::Match, Direction::ToServer, bytes, 0) else {
            self.unparsed += 1;
            return;
        };
        let state = self.places.entry(place).or_default();
        state.datagrams += 1;
        for k in 0..datagram.acks.count.min(128) {
            if datagram.acks.bits >> (127 - k) & 1 == 0 {
                continue;
            }
            let seq = datagram.rseq.wrapping_sub(k as u8);
            if let Some(sent) = state.sent.remove(&seq)
                && let Some(trip) = at_ms.checked_sub(sent).filter(|&t| t < ACK_REACH_MS)
            {
                state.round_trips.push(trip);
            }
        }
        let now = i64::from(at_ms) - offset;
        let Ok(clock) = u32::try_from(now) else {
            return;
        };
        for message in &datagram.messages {
            if message.id == wire::UPLOAD_CARRIER
                && message.part.is_none()
                && !message.payload.is_empty()
            {
                if let Ok(chunk) = Chunk::parse(message.payload)
                    && let Pushed::Complete(upload) = state.uploads.push(&chunk)
                {
                    self.worlds
                        .entry(upload.item_time)
                        .or_default()
                        .push(upload.body);
                }
                continue;
            }
            let Ok(ClientMessage::Input(input)) =
                match_msgs::parse_client(message.id, message.payload)
            else {
                continue;
            };
            let Some(time) = input.time.resolve(clock) else {
                continue;
            };
            let r = input.rotation;
            let value = Input {
                precise: input.time.ms.is_some(),
                arrival: now,
                yaw: half(u16::from_le_bytes([r[0], r[1]])),
                pitch: half(u16::from_le_bytes([r[2], r[3]])),
                actions: input.actions & !INPUT_FLAG,
            };
            match self.inputs.get(&(place, time)) {
                None => {
                    self.inputs.insert((place, time), value);
                }
                Some(known)
                    if known.actions == value.actions
                        && known.yaw.to_bits() == value.yaw.to_bits()
                        && known.pitch.to_bits() == value.pitch.to_bits() =>
                {
                    self.input_copies += 1
                }
                Some(_) => self.input_conflicts += 1,
            }
        }
    }
}

fn percentile(sorted: &[u32], p: f64) -> Option<u32> {
    (!sorted.is_empty()).then(|| sorted[((sorted.len() - 1) as f64 * p).round() as usize])
}

fn decode(path: &Path, out: &Path) -> io::Result<Value> {
    let mut recording = Recording::default();
    record::read_each(path, |r| {
        recording.record(r.kind, r.place, r.at_ms, r.bytes)
    })?;

    let mut inputs = Vec::with_capacity(recording.inputs.len() * INPUT_BYTES);
    for (&(place, time), input) in &recording.inputs {
        inputs.push(place);
        inputs.push(u8::from(input.precise));
        inputs.extend_from_slice(&[0, 0]);
        inputs.extend_from_slice(&time.to_le_bytes());
        inputs.extend_from_slice(
            &(input
                .arrival
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
                .to_le_bytes(),
        );
        inputs.extend_from_slice(&input.yaw.to_le_bytes());
        inputs.extend_from_slice(&input.pitch.to_le_bytes());
        inputs.extend_from_slice(&input.actions.to_le_bytes());
    }

    let players_seen: usize = recording.worlds.len() * 16;
    let mut worlds = Vec::with_capacity(players_seen * WORLD_BYTES);
    let mut world_count = 0u64;
    let mut unread_players = 0u64;
    for (&time, bodies) in &recording.worlds {
        let mut counts: Vec<(usize, u8)> = Vec::new();
        for (i, body) in bodies.iter().enumerate() {
            match counts.iter_mut().find(|(first, _)| bodies[*first] == *body) {
                Some((_, n)) => *n = n.saturating_add(1),
                None => counts.push((i, 1)),
            }
        }
        let Some(&(best, agree)) = counts.iter().max_by_key(|(_, n)| *n) else {
            continue;
        };
        let Some((blocks, _)) = players::walk(&bodies[best]) else {
            continue;
        };
        world_count += 1;
        for block in blocks {
            let Some((_, motion)) = Motion::find(block.state, time) else {
                unread_players += 1;
                continue;
            };
            let mut flags = 0u8;
            let mut feet = [f32::NAN; 3];
            if let Some(body) = &motion.body {
                flags |= u8::from(body.capsule_crouch)
                    | u8::from(body.sprinting) << 1
                    | u8::from(body.jumping) << 2
                    | 8;
                feet = body.feet();
            }
            let speed = motion
                .damage
                .as_ref()
                .map_or(f32::NAN, |d| d.linear_horizontal_speed);
            worlds.extend_from_slice(&time.to_le_bytes());
            worlds.extend_from_slice(&[block.place, u8::from(motion.alive), flags, agree]);
            worlds.extend_from_slice(&motion.inserted_time_ms.to_le_bytes());
            for v in feet.iter().chain(&[motion.rotation_y, motion.look_pitch]) {
                worlds.extend_from_slice(&v.to_le_bytes());
            }
            worlds.extend_from_slice(&motion.actions_mask.to_le_bytes());
            worlds.extend_from_slice(&speed.to_le_bytes());
        }
    }

    let header = recording.header.unwrap_or(Value::Null);
    let mut places = Vec::new();
    for entry in header["places"].as_array().into_iter().flatten() {
        let Some(place) = entry["place"].as_u64() else {
            continue;
        };
        let state = recording.places.get_mut(&(place as u8));
        let (trips, datagrams, revision) = match state {
            Some(s) => {
                s.round_trips.sort_unstable();
                (std::mem::take(&mut s.round_trips), s.datagrams, s.revision)
            }
            None => (Vec::new(), 0, None),
        };
        let count = recording
            .inputs
            .keys()
            .filter(|(p, _)| u64::from(*p) == place)
            .count();
        places.push(json!({
            "place": place,
            "team": entry["team"],
            "squad": entry["squad"],
            "bot": entry["bot"],
            "revision": revision,
            "datagrams_in": datagrams,
            "inputs": count,
            "rtt_samples": trips.len(),
            "rtt_p10": percentile(&trips, 0.1),
            "rtt_p50": percentile(&trips, 0.5),
            "rtt_p90": percentile(&trips, 0.9),
        }));
    }
    let meta = json!({
        "recording": path.file_name().and_then(|n| n.to_str()),
        "map": header["map"],
        "map_id": header["map_id"],
        "mode": header["mode"],
        "match_level": header["match_level"],
        "growing_roster": header["growing_roster"],
        "clock_offset_ms": recording.offset,
        "places": places,
        "inputs": recording.inputs.len(),
        "input_copies": recording.input_copies,
        "input_conflicts": recording.input_conflicts,
        "unparsed_datagrams": recording.unparsed,
        "worlds": world_count,
        "world_players_unread": unread_players,
        "notes": recording.notes,
    });
    fs::create_dir_all(out)?;
    fs::write(out.join("inputs.bin"), inputs)?;
    fs::write(out.join("worlds.bin"), worlds)?;
    fs::write(out.join("meta.json"), serde_json::to_vec_pretty(&meta)?)?;
    Ok(meta)
}

fn main() -> ExitCode {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let Some((out, files)) = args.split_first() else {
        eprintln!("usage: prediction-decode <out dir> <file.srprec>...");
        return ExitCode::FAILURE;
    };
    let todo: Mutex<Vec<&PathBuf>> = Mutex::new(files.iter().rev().collect());
    let failed = Mutex::new(0usize);
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(16);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let Some(file) = todo.lock().expect("lock").pop() else {
                        break;
                    };
                    let stem = file.file_stem().unwrap_or_default();
                    let dir = out.join(stem);
                    if dir.join("meta.json").exists() {
                        continue;
                    }
                    match decode(file, &dir) {
                        Ok(meta) => {
                            let _ = writeln!(
                                io::stdout().lock(),
                                "{}: {} inputs, {} worlds",
                                stem.to_string_lossy(),
                                meta["inputs"],
                                meta["worlds"]
                            );
                        }
                        Err(error) => {
                            *failed.lock().expect("lock") += 1;
                            eprintln!("{}: {error}", file.display());
                        }
                    }
                }
            });
        }
    });
    let failed = *failed.lock().expect("lock");
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halves_read_as_the_game_wrote_them() {
        assert_eq!(half(0x3C00), 1.0);
        assert_eq!(half(0xC000), -2.0);
        assert_eq!(half(0x3555), 0.333_251_95);
        assert_eq!(half(0x0001), 2f32.powi(-24));
        assert!(half(0x7C00).is_infinite());
        assert!(half(0x7E00).is_nan());
    }

    #[test]
    fn percentiles_of_sorted_samples() {
        assert_eq!(percentile(&[], 0.5), None);
        assert_eq!(percentile(&[10, 20, 30, 40, 50], 0.5), Some(30));
        assert_eq!(percentile(&[10, 20, 30, 40, 50], 0.9), Some(50));
    }
}

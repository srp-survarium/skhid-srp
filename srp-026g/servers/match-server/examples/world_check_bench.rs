//! What the world checks cost on real worlds with their real match: the
//! match server's whole check of an upload (`world_validation::check`, its
//! late-joiner retries included, as `match_server::v026g::check_bench`
//! runs it), and the game's check of the same world at its load (patch 87,
//! `client-patches/rust/patches/src/world_check.rs`: `srp_validate` with the
//! game's context, 20 places and the match's mode, batteries and respawn
//! points), timed on the host.
//!
//! ```text
//! cargo run --release -p match-server --example world_check_bench -- \
//!     --data SERVER_GAME_DATA [--recordings PATH]... \
//!     [--world-list LIST --root DIR --context PATH...] [--tsv OUT] \
//!     [--dump DIR] [--flips] [--quick]
//! ```
//!
//! - `--data`: the server game data the recordings' matches ran with.
//! - `--recordings PATH`: a match recording (`SRP_RECORD_DIR`'s `.srprec`),
//!   or a folder of them: every complete upload (0xE0) checked against the
//!   roster the server had when it came (the header's `initial_players`,
//!   then the places of each `places` note, `sync_roster`; the profiles are
//!   the largest 0xE4's).
//! - `--world-list LIST --root DIR`: kept worlds (`SRP_SNAPSHOT_DIR`'s
//!   `match026g_<id>_<ms>_p<place>_...bin`), one path a line relative to
//!   DIR, each checked against the roster its match's recording (from
//!   `--recordings` or `--context PATH`, which only gives matches) had at
//!   the world's time. A world of no given recording is counted and left.
//! - `--tsv OUT`: one row a world: the verdicts and the times.
//! - `--dump DIR`: each world's body as `DIR/<n>.body`, and `DIR/worlds.tsv`
//!   with a row a world as `drift_dump`'s (its time, the game's context and
//!   the game's verdict of it and of its 16 copies with one byte flipped):
//!   what `check_world_check_026g.py --drift` runs through the arena under
//!   Unicorn, and what a 32-bit build of the game's check can be timed on.
//! - `--flips`: the TSV also has the server's verdicts of those 16 copies,
//!   so that two builds' TSVs show whether a change moved any refusal.
//! - `--quick`: 5 calls a batch, not 20.
//!
//! A world's time is the median of five batches of calls (ns a call), and
//! its first call's, timed alone after the world before it. A match's total
//! is the sum of its uploads' times.

use std::collections::{BTreeMap, HashMap};
use std::hint::black_box;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use match_server::v026g::check_bench::Match;
use match_server::v026g::record::{self, IN, NOTE, OUT};
use srp_validate::MAX_PLAYERS;
use srp_validate::world::{self, Context};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::match_msgs::mask_bytes;
use survarium::v026g::match_msgs::static_info::{self, MatchOptions};
use survarium::v026g::match_msgs::upload::{Chunk, Pushed, Reassembler};
use udp_transport::table::{Direction, Table};
use udp_transport::wire;

struct Options {
    data: PathBuf,
    recordings: Vec<PathBuf>,
    context: Vec<PathBuf>,
    world_list: Option<PathBuf>,
    root: Option<PathBuf>,
    tsv: Option<PathBuf>,
    dump: Option<PathBuf>,
    flips: bool,
    calls: u32,
}

fn main() {
    let mut options = Options {
        data: PathBuf::new(),
        recordings: Vec::new(),
        context: Vec::new(),
        world_list: None,
        root: None,
        tsv: None,
        dump: None,
        flips: false,
        calls: 20,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value =
            |what: &str| PathBuf::from(args.next().unwrap_or_else(|| panic!("{arg} {what}")));
        match arg.as_str() {
            "--data" => options.data = value("DB"),
            "--recordings" => options.recordings.push(value("PATH")),
            "--context" => options.context.push(value("PATH")),
            "--world-list" => options.world_list = Some(value("LIST")),
            "--root" => options.root = Some(value("DIR")),
            "--tsv" => options.tsv = Some(value("OUT")),
            "--dump" => options.dump = Some(value("DIR")),
            "--flips" => options.flips = true,
            "--quick" => options.calls = 5,
            other => panic!("unknown argument {other:?}"),
        }
    }
    assert!(
        !options.data.as_os_str().is_empty()
            && (!options.recordings.is_empty() || options.world_list.is_some()),
        "usage: world_check_bench --data DB [--recordings PATH]... \
         [--world-list LIST --root DIR --context PATH...] [--tsv OUT] [--dump DIR] [--flips] [--quick]"
    );
    survarium::v026g::game_data::init_from(&options.data).expect("the server game data");
    // Made once, before any timing: the server has them from its start.
    let _ = (Dictionary::shared(), survarium::v026g::item_params::all());

    let mut out = Out::new(options.tsv.as_deref(), options.dump.as_deref());

    let mut uploads = Vec::new();
    let mut matches = Vec::new();
    for path in files(&options.recordings) {
        let Some(recording) = Recording::read(&path) else {
            continue;
        };
        let timed: Vec<Timed> = recording
            .uploads
            .iter()
            .map(|upload| {
                let (roster, check) = recording.roster_at(upload.at_ms);
                let (place, time, body) = (upload.place, upload.time, &upload.body[..]);
                let timed = Timed::of(&recording, check, place, time, body, roster, options.calls)
                    .with_flips(options.flips, check, body);
                out.world(&recording.name, &timed, body);
                timed
            })
            .collect();
        println!(
            "{}: {} uploads, rosters {:?} (first {}), {} s",
            recording.name,
            timed.len(),
            recording
                .rosters
                .iter()
                .map(|(_, n, _)| n)
                .collect::<Vec<_>>(),
            recording.initial,
            recording.span_ms / 1000
        );
        matches.push((
            timed.iter().map(|t| t.server_ns).sum::<u64>(),
            recording.span_ms,
        ));
        uploads.extend(timed);
    }
    let mut kept = Vec::new();
    if let Some(list) = &options.world_list {
        let root = options.root.clone().unwrap_or_default();
        let mut by_id: HashMap<u32, Recording> = HashMap::new();
        for path in files(options.recordings.iter().chain(&options.context)) {
            if let Some(recording) = Recording::read_matches(&path) {
                by_id.insert(recording.id, recording);
            }
        }
        let mut unmatched = 0;
        let text = std::fs::read_to_string(list).expect("the world list");
        for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let path = root.join(line);
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let Some((id, time, place)) = kept_name(&name) else {
                continue;
            };
            let Some(recording) = by_id.get(&id) else {
                unmatched += 1;
                continue;
            };
            let body = std::fs::read(&path).expect("a kept world");
            let clock = recording.clock_ms(time);
            let (roster, check) = recording.roster_at(clock);
            let timed = Timed::of(recording, check, place, time, &body, roster, options.calls)
                .with_flips(options.flips, check, &body);
            out.world(&name, &timed, &body);
            kept.push(timed);
        }
        println!(
            "kept worlds: {} of a given recording's match, {unmatched} of none",
            kept.len()
        );
    }

    println!();
    if !uploads.is_empty() {
        summary("uploads of the recordings", &uploads);
        let mut totals: Vec<u64> = matches.iter().map(|(ns, _)| *ns).collect();
        totals.sort_unstable();
        let per_minute: Vec<f64> = matches
            .iter()
            .filter(|(_, span)| *span > 0)
            .map(|(ns, span)| *ns as f64 / 1e3 / (f64::from(*span) / 60_000.0))
            .collect();
        println!(
            "  a match's uploads in all: median {:.2} ms, max {:.2} ms over {} matches; \
             at most {:.0} us a minute of match",
            ms(median(&totals)),
            ms(totals.last().copied().unwrap_or(0)),
            totals.len(),
            per_minute.iter().copied().fold(0.0, f64::max)
        );
    }
    if !kept.is_empty() {
        summary("kept worlds", &kept);
    }
}

/// One world's verdicts and times.
struct Timed {
    place: u8,
    time: u32,
    bytes: usize,
    roster: usize,
    initial: usize,
    places: usize,
    server_first_ns: u64,
    server_ns: u64,
    game_ns: u64,
    server: Result<(), String>,
    game: Result<(), String>,
    /// With `--flips`: the server's verdicts of the world's [`flipped`]
    /// copies, `ok` or the refusal each.
    flips: Option<String>,
    /// What the game checks the world against.
    context: Context<'static>,
}

impl Timed {
    fn of(
        recording: &Recording,
        check: &Match,
        place: u8,
        time: u32,
        body: &[u8],
        roster: usize,
        calls: u32,
    ) -> Self {
        let places = if recording.growing {
            MAX_PLAYERS
        } else {
            roster
        };
        let start = Instant::now();
        let server = check.check(black_box(body), time, places);
        let server_first_ns = start.elapsed().as_nanos() as u64;
        let server_ns = batches(calls, || check.check(black_box(body), time, places).is_ok());
        // Patch 87's `Match::context`: the game's own rule objects give the
        // mode, batteries and respawn points; nothing of the profiles.
        let map = recording.options.map.map();
        let context = Context {
            time,
            places: MAX_PLAYERS,
            game_mode: map.mode.rules().number(),
            batteries: map.victory_items_count as usize,
            respawn_points: Some(map.respawn_points),
            profiles: None,
            artefacts: None,
            objects: None,
            inventory: None,
        };
        let game = world::validate(body, &context).map_err(|refusal| refusal.to_string());
        let game_ns = batches(calls, || world::validate(black_box(body), &context).is_ok());
        Self {
            place,
            time,
            bytes: body.len(),
            roster,
            initial: recording.initial,
            places,
            server_first_ns,
            server_ns,
            game_ns,
            server,
            game,
            flips: None,
            context,
        }
    }

    /// Adds the server's verdicts of the world's flipped copies.
    fn with_flips(mut self, flips: bool, check: &Match, body: &[u8]) -> Self {
        if flips {
            let verdicts: Vec<String> = flipped(body)
                .map(|copy| match check.check(&copy, self.time, self.places) {
                    Ok(()) => "ok".to_string(),
                    Err(why) => why,
                })
                .collect();
            self.flips = Some(verdicts.join(" | "));
        }
        self
    }
}

/// Where each world's row goes: the TSV, and the dumped bodies.
struct Out {
    tsv: Option<std::fs::File>,
    dump: Option<(PathBuf, std::fs::File, usize)>,
}

impl Out {
    fn new(tsv: Option<&Path>, dump: Option<&Path>) -> Self {
        let tsv = tsv.map(|path| {
            let mut file = std::fs::File::create(path).expect("the TSV");
            writeln!(
                file,
                "source\tplace\ttime\tbytes\troster\tinitial\tplaces\tserver_first_ns\tserver_ns\t\
                 game_ns\tserver\tgame\tserver_flips"
            )
            .unwrap();
            file
        });
        let dump = dump.map(|dir| {
            std::fs::create_dir_all(dir).expect("the dump folder");
            let rows = std::fs::File::create(dir.join("worlds.tsv")).expect("worlds.tsv");
            (dir.to_path_buf(), rows, 0)
        });
        Self { tsv, dump }
    }

    fn world(&mut self, source: &str, timed: &Timed, body: &[u8]) {
        if let Some(file) = &mut self.tsv {
            let verdict = |result: &Result<(), String>| match result {
                Ok(()) => "ok".to_string(),
                Err(why) => why.replace(['\t', '\n'], " "),
            };
            writeln!(
                file,
                "{source}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                timed.place,
                timed.time,
                timed.bytes,
                timed.roster,
                timed.initial,
                timed.places,
                timed.server_first_ns,
                timed.server_ns,
                timed.game_ns,
                verdict(&timed.server),
                verdict(&timed.game),
                timed
                    .flips
                    .as_deref()
                    .unwrap_or("")
                    .replace(['\t', '\n'], " ")
            )
            .unwrap();
        }
        if let Some((dir, rows, n)) = &mut self.dump {
            let file = dir.join(format!("{n:06}.body"));
            *n += 1;
            std::fs::write(&file, body).expect("a dumped world");
            // As drift_dump: the world, then 16 copies each with one byte
            // flipped, `1` for each the game refuses.
            let wanted: String = std::iter::once(body.to_vec())
                .chain(flipped(body))
                .map(|copy| {
                    if world::validate(&copy, &timed.context).is_ok() {
                        '0'
                    } else {
                        '1'
                    }
                })
                .collect();
            let context = &timed.context;
            writeln!(
                rows,
                "{}\t{}\t{}\t{}\t{}\t{wanted}",
                file.display(),
                context.time,
                context.game_mode,
                context.batteries,
                context.respawn_points.unwrap_or(0)
            )
            .unwrap();
        }
    }
}

/// 16 copies of `body`, copy k with its byte at `len * k / 16` flipped
/// (`check_world_check_026g.py`'s mutated copies).
fn flipped(body: &[u8]) -> impl Iterator<Item = Vec<u8>> + '_ {
    (0..16).filter(|_| !body.is_empty()).map(|k| {
        let mut copy = body.to_vec();
        copy[body.len() * k / 16] ^= 0xFF;
        copy
    })
}

/// The median of five batches of `calls` calls, in ns a call.
fn batches(calls: u32, mut call: impl FnMut() -> bool) -> u64 {
    let mut each: Vec<u64> = (0..5)
        .map(|_| {
            let start = Instant::now();
            for _ in 0..calls {
                black_box(call());
            }
            start.elapsed().as_nanos() as u64 / u64::from(calls)
        })
        .collect();
    each.sort_unstable();
    each[2]
}

fn summary(what: &str, timed: &[Timed]) {
    let sorted = |get: fn(&Timed) -> u64| {
        let mut values: Vec<u64> = timed.iter().map(get).collect();
        values.sort_unstable();
        values
    };
    let line = |name: &str, values: &[u64]| {
        println!(
            "  {name:28} median {:8.2} us  p99 {:8.2} us  max {:8.2} us",
            us(median(values)),
            us(percentile(values, 99)),
            us(values.last().copied().unwrap_or(0))
        );
    };
    let refused =
        |get: fn(&Timed) -> &Result<(), String>| timed.iter().filter(|t| get(t).is_err()).count();
    println!(
        "{what}: {} worlds; the server refuses {}, the game {}",
        timed.len(),
        refused(|t| &t.server),
        refused(|t| &t.game)
    );
    line("server check", &sorted(|t| t.server_ns));
    line("server check, first call", &sorted(|t| t.server_first_ns));
    line("game check (host)", &sorted(|t| t.game_ns));
}

fn median(sorted: &[u64]) -> u64 {
    sorted.get(sorted.len() / 2).copied().unwrap_or(0)
}

/// The nearest-rank percentile.
fn percentile(sorted: &[u64], p: usize) -> u64 {
    let rank = (sorted.len() * p).div_ceil(100).max(1);
    sorted.get(rank - 1).copied().unwrap_or(0)
}

fn us(ns: u64) -> f64 {
    ns as f64 / 1e3
}

fn ms(ns: u64) -> f64 {
    ns as f64 / 1e6
}

/// The `.srprec` files of `paths`, each a file or a folder, in name order.
fn files<'a>(paths: impl IntoIterator<Item = &'a PathBuf>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if path.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(path)
                .expect("a recordings folder")
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "srprec"))
                .collect();
            found.sort();
            out.extend(found);
        } else {
            out.push(path.clone());
        }
    }
    out
}

/// `match026g_<id hex>_<ms>_p<place>_...`: the match, the time, the place.
fn kept_name(name: &str) -> Option<(u32, u32, u8)> {
    let mut parts = name.strip_prefix("match026g_")?.split('_');
    let id = u32::from_str_radix(parts.next()?, 16).ok()?;
    let time = parts.next()?.parse().ok()?;
    let place = parts.next()?.strip_prefix('p')?.parse().ok()?;
    Some((id, time, place))
}

/// A 0xE4 being joined: its part count, the parts by index.
type Parts = (u8, BTreeMap<u8, Vec<u8>>);

/// A complete upload: its place, when it came (the recording's clock), its
/// item time and its body.
struct Upload {
    place: u8,
    at_ms: u32,
    time: u32,
    body: Vec<u8>,
}

/// A recording's match: its rosters as the server had them, and its uploads.
struct Recording {
    name: String,
    /// The match id of its file name (`rec026g_<date>_<id hex>`).
    id: u32,
    growing: bool,
    /// The options of the largest roster a 0xE4 gave: a roster's profiles
    /// are this one's first.
    options: MatchOptions,
    /// From when (the recording's clock) the server had each roster, and its
    /// check of the match with it, oldest first: the header's initial
    /// places from the start, then each `places` note's (`sync_roster`).
    rosters: Vec<(u32, usize, Match)>,
    initial: usize,
    /// The recording's clock less the match's (its notes' `match_ms`).
    offset_ms: i64,
    uploads: Vec<Upload>,
    span_ms: u32,
}

impl Recording {
    fn read(path: &Path) -> Option<Self> {
        Self::read_with(path, true)
    }

    /// The match only, not its uploads.
    fn read_matches(path: &Path) -> Option<Self> {
        Self::read_with(path, false)
    }

    fn read_with(path: &Path, with_uploads: bool) -> Option<Self> {
        let name = path.file_name()?.to_string_lossy().into_owned();
        let id = name
            .trim_end_matches(".srprec")
            .rsplit('_')
            .next()
            .and_then(|id| u32::from_str_radix(id, 16).ok())?;
        let dictionary = Dictionary::shared();
        let mut growing = false;
        let mut offsets = Vec::new();
        let mut roster_of: HashMap<u8, u8> = HashMap::new();
        let mut parts: HashMap<(u8, u8), Parts> = HashMap::new();
        let mut reassemblers: HashMap<u8, Reassembler> = HashMap::new();
        let mut largest: Option<MatchOptions> = None;
        let mut first_roster = None;
        let mut header_initial = None;
        let mut joins: Vec<(u32, usize)> = Vec::new();
        let mut uploads = Vec::new();
        let mut first_ms = None;
        let mut span_ms = 0;
        let read = record::read_each(path, |r| {
            let first = *first_ms.get_or_insert(r.at_ms);
            span_ms = r.at_ms.saturating_sub(first);
            match r.kind {
                NOTE => {
                    let Ok(note) = serde_json::from_slice::<serde_json::Value>(r.bytes) else {
                        return;
                    };
                    if let Some(g) = note["growing_roster"].as_bool() {
                        growing = g;
                    }
                    if let Some(n) = note["initial_players"].as_u64() {
                        header_initial = Some(n as usize);
                    }
                    // The server took late joiners' places (`sync_roster`).
                    if note["note"] == "places"
                        && let Some(last) = note["places"].as_array().and_then(|places| {
                            places.iter().filter_map(|p| p["place"].as_u64()).max()
                        })
                    {
                        joins.push((r.at_ms, last as usize + 1));
                    }
                    if let Some(ms) = note["match_ms"].as_u64() {
                        offsets.push(i64::from(r.at_ms) - ms as i64);
                    }
                }
                IN if with_uploads => {
                    let Ok(datagram) = wire::parse(Table::Match, Direction::ToServer, r.bytes, 0)
                    else {
                        return;
                    };
                    for message in &datagram.messages {
                        if message.id != wire::UPLOAD_CARRIER
                            || message.part.is_some()
                            || message.payload.is_empty()
                        {
                            continue;
                        }
                        let Ok(chunk) = Chunk::parse(message.payload) else {
                            continue;
                        };
                        if let Pushed::Complete(upload) =
                            reassemblers.entry(r.place).or_default().push(&chunk)
                        {
                            uploads.push(Upload {
                                place: r.place,
                                at_ms: r.at_ms,
                                time: upload.item_time,
                                body: upload.body,
                            });
                        }
                    }
                }
                OUT => {
                    // As `sur_convert`: a match that lets players in late
                    // sends masks for 20 from the start.
                    let roster = roster_of.get(&r.place).copied().unwrap_or(0);
                    let mask_len = match (growing, roster) {
                        (true, _) => mask_bytes(MAX_PLAYERS as u8),
                        (false, 0) => 0,
                        (false, roster) => mask_bytes(roster),
                    };
                    let Ok(datagram) =
                        wire::parse(Table::Match, Direction::ToClient, r.bytes, mask_len)
                    else {
                        return;
                    };
                    for message in datagram.messages.iter().filter(|m| m.id == 0xE4) {
                        let Some(x) = message.x else { continue };
                        let (count, got) =
                            parts.entry((r.place, x)).or_insert((1, BTreeMap::new()));
                        match message.part {
                            Some(part) => {
                                if let Some(n) = part.count {
                                    *count = n;
                                }
                                got.insert(part.index, message.payload.to_vec());
                                if got.len() < usize::from(*count) {
                                    continue;
                                }
                            }
                            None => {
                                got.insert(0, message.payload.to_vec());
                            }
                        }
                        let whole: Vec<u8> = got.values().flatten().copied().collect();
                        parts.remove(&(r.place, x));
                        if let Ok((_, parsed)) = static_info::parse_payload(&whole, dictionary) {
                            let n = parsed.players.len();
                            roster_of.insert(r.place, n as u8);
                            first_roster.get_or_insert(n);
                            if largest.as_ref().is_none_or(|o| o.players.len() < n) {
                                largest = Some(parsed);
                            }
                        }
                    }
                }
                _ => {}
            }
        });
        if let Err(error) = read {
            println!("{name}: read to {span_ms} ms only: {error}");
        }
        let (Some(options), Some(first)) = (largest, first_roster) else {
            println!("{name}: no 0xE4, {} uploads left out", uploads.len());
            return None;
        };
        offsets.sort_unstable();
        let initial = header_initial.unwrap_or(first).min(options.players.len());
        let mut timeline = vec![(0, initial)];
        for (at, n) in joins {
            let n = n.min(options.players.len());
            if timeline.last().is_some_and(|&(_, last)| n > last) {
                timeline.push((at, n));
            }
        }
        let rosters = timeline
            .into_iter()
            .map(|(at, n)| {
                let roster = MatchOptions {
                    players: options.players[..n].to_vec(),
                    ..options.clone()
                };
                let check = Match::new(&roster, initial).expect("a roster the server sent");
                (at, n, check)
            })
            .collect();
        Some(Self {
            name,
            id,
            growing,
            options,
            rosters,
            initial,
            offset_ms: offsets.get(offsets.len() / 2).copied().unwrap_or(0),
            uploads,
            span_ms,
        })
    }

    /// The roster the server had at `at_ms` (the recording's clock).
    fn roster_at(&self, at_ms: u32) -> (usize, &Match) {
        let (_, n, check) = self
            .rosters
            .iter()
            .rev()
            .find(|(from, _, _)| *from <= at_ms)
            .unwrap_or(&self.rosters[0]);
        (*n, check)
    }

    /// The recording's clock at the match's `time`.
    fn clock_ms(&self, time: u32) -> u32 {
        (i64::from(time) + self.offset_ms).clamp(0, i64::from(u32::MAX)) as u32
    }
}

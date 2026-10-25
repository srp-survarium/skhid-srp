//! What the client check costs on real traffic (`client_world_check.md`
//! section 14): the host build of [`messages::check`] on every match message
//! the server sent in packet logs, and of [`world::validate`] on every kept
//! world and every world (0xE8) those logs carry, with the context the
//! game's payload reads (20 places, the match's mode, batteries and respawn
//! points; no profiles).
//!
//! ```text
//! cargo run --release -p srp-validate --example bench -- \
//!     [--worlds DIR]... [--packets DIR]... [--tsv OUT] [--quick]
//! ```
//!
//! - `--worlds DIR`: every `match026g_*.bin` under DIR (the match server's
//!   `SRP_SNAPSHOT_DIR` dumps: a world body, as 0x2D0B70 reads it).
//! - `--packets DIR`: every `match-*.log` under DIR (the match server's
//!   packet logs): the messages it sent (`OUT`), parts joined, each timed
//!   with `messages::check`; each 0xE8's world unpacked and timed as a
//!   world.
//! - `--tsv OUT`: one row a world (source, bytes, players, the context,
//!   the verdict, ns min/median, and each part's ns).
//! - `--quick`: fewer repetitions.
//!
//! The context of a world is the first that lets it pass, the payload's 20
//! places first: the same search as `drift_dump`, so a world is timed on its
//! whole path when any match takes it, and on its refusal's path otherwise.
//! Times are the median of five batches, each repeated to about 2 ms
//! (`--quick`: 0.2 ms), in ns a call.

use std::collections::{BTreeMap, HashSet};
use std::hint::black_box;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use srp_validate::world::{self, Blocks, Context};
use srp_validate::{MAX_PLAYERS, messages, mode};

const MODES: [u8; 4] = [
    mode::BATTERIES,
    mode::TIMED_BATTERIES,
    mode::TEAM_DEATHMATCH,
    mode::RESEARCH,
];
const MAX_BATTERIES: usize = 16;
const MAX_RESPAWN: usize = 127;
/// The client's buffer for an unpacked world (0x27C05E).
const WORLD_BUFFER: usize = 0x8000;

struct Options {
    worlds: Vec<PathBuf>,
    packets: Vec<PathBuf>,
    tsv: Option<PathBuf>,
    dump: Option<PathBuf>,
    budget_ns: u128,
}

fn main() {
    let mut options = Options {
        worlds: Vec::new(),
        packets: Vec::new(),
        tsv: None,
        dump: None,
        budget_ns: 2_000_000,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--worlds" => options
                .worlds
                .push(args.next().expect("--worlds DIR").into()),
            "--packets" => options
                .packets
                .push(args.next().expect("--packets DIR").into()),
            "--tsv" => options.tsv = Some(args.next().expect("--tsv OUT").into()),
            "--quick" => options.budget_ns = 200_000,
            "--dump-refused" => {
                options.dump = Some(args.next().expect("--dump-refused DIR").into())
            }
            other => panic!("unknown argument {other:?}"),
        }
    }
    assert!(
        !options.worlds.is_empty() || !options.packets.is_empty(),
        "usage: bench [--worlds DIR]... [--packets DIR]... [--tsv OUT] [--quick]"
    );

    // Each world as the game's reader has it at 0x2D0B70: the time (u32),
    // then the body (`world::validate`'s, from the mask on).
    let mut worlds: Vec<(String, Vec<u8>)> = Vec::new();
    for dir in &options.worlds {
        let mut files = Vec::new();
        collect(
            dir,
            &|name| name.starts_with("match026g_") && name.ends_with(".bin"),
            &mut files,
        );
        files.sort();
        for path in files {
            let body = std::fs::read(&path).expect("a world file reads");
            // `match026g_<match>_<time>_p<place>_...`: the dump is the body
            // after the time.
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let Some(time) = name.split('_').nth(2).and_then(|t| t.parse::<u32>().ok()) else {
                continue;
            };
            let mut state = time.to_le_bytes().to_vec();
            state.extend(body);
            worlds.push((path.display().to_string(), state));
        }
    }
    let kept = worlds.len();

    if !options.packets.is_empty() {
        let mut logs = Vec::new();
        for dir in &options.packets {
            collect(
                dir,
                &|name| name.starts_with("match-") && name.ends_with(".log"),
                &mut logs,
            );
        }
        logs.sort();
        let messages = sent_messages(&logs);
        bench_messages(&messages, options.budget_ns);
        let mut refused: BTreeMap<String, Vec<&Path>> = BTreeMap::new();
        for message in &messages {
            if let Err(refusal) = messages::check(message.id, &message.payload) {
                refused
                    .entry(format!("{}: {}", refusal.part, refusal.reason))
                    .or_default()
                    .push(&logs[message.log]);
            }
        }
        for (reason, logs) in &refused {
            println!(
                "  refused: {:5}  {reason}  (e.g. {})",
                logs.len(),
                logs[0].display()
            );
        }
        let (unpacked, refused_streams, unpack_ns) = unpack_worlds(&messages, &logs);
        println!(
            "\n0xE8 worlds in the logs: {} unpacked ({} distinct), {} not unpacked; \
             LZMA2 unpack (host, lzma-rust2) median {} ns",
            unpacked.len(),
            unpacked
                .iter()
                .map(|(_, body)| body)
                .collect::<HashSet<_>>()
                .len(),
            refused_streams,
            median(unpack_ns)
        );
        let mut seen = HashSet::new();
        for (name, body) in unpacked {
            if seen.insert(body.clone()) {
                worlds.push((name, body));
            }
        }
    }

    if !worlds.is_empty() {
        bench_worlds(&worlds, kept, &options);
        bench_worst(&worlds, options.budget_ns, options.dump.as_deref());
    }
}

// ---- the worst cases ------------------------------------------------------

/// Worlds made from a kept one of a single player to the caps the check
/// allows: 20 players, the bullets section full of records that read at
/// every pellet offset, the rules section at its cap. Each timed as the
/// payload checks it.
fn bench_worst(worlds: &[(String, Vec<u8>)], budget_ns: u128, dump: Option<&Path>) {
    // The biggest passing world of one player, mode 2 (the gather search).
    let Some((state, m)) = worlds
        .iter()
        .filter_map(|(_, state)| {
            let body = body_of(state);
            let one = body
                .first_chunk::<4>()
                .map(|b| u32::from_le_bytes(*b).count_ones())
                == Some(1);
            let m = choose(state)?;
            (one && m.places == MAX_PLAYERS && m.game_mode == mode::BATTERIES).then_some((state, m))
        })
        .max_by_key(|(state, _)| state.len())
    else {
        println!("\nWorst cases: no one-player mode 2 world to make them from");
        return;
    };
    let body = body_of(state);
    let frame = world::frame(body, MAX_PLAYERS).expect("it passed");
    let block_end = frame.bullets().at - 2;
    let block = &body[4..block_end];
    let sections = |replace: &dyn Fn(usize, &[u8]) -> Vec<u8>| -> Vec<u8> {
        let mut out = Vec::new();
        for (i, section) in frame.sections.iter().enumerate() {
            let bytes = replace(i, section.bytes);
            out.extend((bytes.len() as u16).to_le_bytes());
            out.extend(bytes);
        }
        out
    };
    let make = |players: u32, replace: &dyn Fn(usize, &[u8]) -> Vec<u8>| -> Vec<u8> {
        let mut out = state[..4].to_vec();
        out.extend(((1u64 << players) - 1).to_le_bytes()[..4].iter());
        for _ in 0..players {
            out.extend(block);
        }
        out.extend(sections(replace));
        out
    };
    let keep = |_: usize, bytes: &[u8]| bytes.to_vec();
    // 135 bullet records (the section's cap), every byte 7: owner 7, second
    // 7, slot 7, ammo 7, floats 0x07070707, so every record reads at every
    // pellet offset and the framing search keeps all of them.
    let bullets = |i: usize, bytes: &[u8]| {
        if i != 0 {
            return bytes.to_vec();
        }
        let mut out = vec![0u8; 12];
        out.extend(135u32.to_le_bytes());
        out.extend(std::iter::repeat_n(7u8, 135 * 68));
        out
    };
    // The rules at their cap (0x800): the timelimit and the kd stats kept,
    // the middle a respawn rule with no one waiting and zeros, which the
    // gather search tries at every even offset.
    let rules = |i: usize, bytes: &[u8]| {
        if i != 4 {
            return bytes.to_vec();
        }
        let kd = &bytes[bytes.len() - (5 * MAX_PLAYERS + 2)..];
        let mut out = bytes[..13].to_vec();
        out.resize(0x800 - kd.len(), 0);
        out.extend(kd);
        out
    };
    let cases: [(&str, Vec<u8>); 4] = [
        ("the kept world (1 player)", make(1, &keep)),
        ("20 copies of its player", make(20, &keep)),
        (
            "20 players, 135 bullets at every offset",
            make(20, &bullets),
        ),
        ("20 players, rules at the cap", make(20, &rules)),
    ];
    println!(
        "\nWorst cases, made from a kept one-player mode 2 world ({} bytes):",
        body.len()
    );
    let mut rows = String::new();
    for (n, (name, made)) in cases.into_iter().enumerate() {
        let ctx = context(&made, m);
        let body = body_of(&made);
        let verdict = match world::validate(body, &ctx) {
            Ok(()) => "OK".to_string(),
            Err(refusal) => format!("{}: {}", refusal.part, refusal.reason),
        };
        let (median, _) = time(budget_ns, || world::validate(black_box(body), &ctx).is_ok());
        println!(
            "  {name:40} {:6} bytes  {:8.2} us  {verdict}",
            body.len(),
            median as f64 / 1e3
        );
        if let Some(dir) = dump {
            // As drift_dump's rows: the body after the time, the match, the
            // host's verdict (check_client_verify_026g.py --drift).
            std::fs::create_dir_all(dir).expect("the dump folder");
            let file = dir.join(format!("worst_{n}.body"));
            std::fs::write(&file, body).expect("a dump");
            rows += &format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                file.display(),
                ctx.time,
                m.game_mode,
                m.batteries,
                m.respawn,
                u8::from(verdict != "OK")
            );
        }
    }
    if let Some(dir) = dump {
        std::fs::write(dir.join("worst.tsv"), rows).expect("the worst cases' rows");
    }
}

// ---- packet logs ---------------------------------------------------------

/// A message the server sent: its log, its id, its payload after the id.
struct Sent {
    log: usize,
    id: u8,
    payload: Vec<u8>,
}

/// Every message of the `OUT` datagrams of `logs`, parts joined.
fn sent_messages(logs: &[PathBuf]) -> Vec<Sent> {
    let mut out = Vec::new();
    for (log, path) in logs.iter().enumerate() {
        let text = std::fs::read_to_string(path).expect("a packet log reads");
        let mut datagram: Option<Vec<u8>> = None;
        // Parts by x: (parts in all, the payload so far, parts seen).
        let mut parts: BTreeMap<u32, (usize, Vec<u8>, usize)> = BTreeMap::new();
        for line in text.lines() {
            if let Some(rest) = line.split_once(" OUT ").map(|(_, rest)| rest) {
                datagram = rest.split_once(" len=").and_then(|(_, rest)| {
                    let mut words = rest.split(' ');
                    words.next()?;
                    words.map(|hex| u8::from_str_radix(hex, 16).ok()).collect()
                });
                continue;
            }
            if !line.starts_with("20") && !line.starts_with("    msg id=") {
                continue;
            }
            if line.starts_with("20") {
                datagram = None;
                continue;
            }
            let Some(bytes) = datagram.as_ref() else {
                continue;
            };
            let field = |key: &str| {
                line.split(' ')
                    .find_map(|word| word.strip_prefix(key))
                    .map(str::to_owned)
            };
            let Some(id) = field("id=").and_then(|id| u8::from_str_radix(&id, 16).ok()) else {
                continue;
            };
            if !(0xE2..=0xF6).contains(&id) {
                continue;
            }
            let (Some(len), Some(at)) = (
                field("len=").and_then(|v| v.parse::<usize>().ok()),
                field("at=").and_then(|v| v.parse::<usize>().ok()),
            ) else {
                continue;
            };
            let Some(piece) = bytes.get(at..at + len) else {
                continue;
            };
            match field("part=") {
                None => out.push(Sent {
                    log,
                    id,
                    payload: piece.to_vec(),
                }),
                Some(part) => {
                    let x = field("x=").and_then(|v| v.parse().ok()).unwrap_or(0);
                    if let Some((first, count)) = part.split_once('/') {
                        if first == "0" {
                            let count = count.parse().unwrap_or(1);
                            parts.insert(x, (count, piece.to_vec(), 1));
                        }
                    } else if let Some(entry) = parts.get_mut(&x) {
                        entry.1.extend_from_slice(piece);
                        entry.2 += 1;
                    }
                    if let Some(&(count, _, seen)) = parts.get(&x)
                        && seen == count
                    {
                        let (_, payload, _) = parts.remove(&x).expect("just seen");
                        out.push(Sent { log, id, payload });
                    }
                }
            }
        }
    }
    out
}

fn bench_messages(sent: &[Sent], budget_ns: u128) {
    let logs = sent.iter().map(|m| m.log).collect::<HashSet<_>>().len();
    println!(
        "Match messages the server sent: {} in {} logs (host build of messages::check, ns a call)",
        sent.len(),
        logs
    );
    let mut by_id: BTreeMap<u8, (Vec<u64>, usize, usize)> = BTreeMap::new();
    let reps = (budget_ns / 4_000).clamp(16, 512) as u32;
    for message in sent {
        let start = Instant::now();
        let mut refused = 0;
        for _ in 0..reps {
            refused += usize::from(
                messages::check(black_box(message.id), black_box(&message.payload)).is_err(),
            );
        }
        let ns = (start.elapsed().as_nanos() / u128::from(reps)) as u64;
        let entry = by_id.entry(message.id).or_default();
        entry.0.push(ns);
        entry.1 += usize::from(refused != 0);
        entry.2 = entry.2.max(message.payload.len());
    }
    println!("  id    count  refused  max-bytes    min  median    p99    max");
    let mut all = Vec::new();
    for (id, (mut ns, refused, bytes)) in by_id {
        ns.sort_unstable();
        println!(
            "  {id:02X} {:8} {:8} {:10} {:6} {:7} {:6} {:6}",
            ns.len(),
            refused,
            bytes,
            ns[0],
            pct(&ns, 50.0),
            pct(&ns, 99.0),
            ns[ns.len() - 1]
        );
        all.extend(ns);
    }
    all.sort_unstable();
    if !all.is_empty() {
        println!(
            "  all {:8}                     {:6} {:7} {:6} {:6}",
            all.len(),
            all[0],
            pct(&all, 50.0),
            pct(&all, 99.0),
            all[all.len() - 1]
        );
    }
}

/// Each 0xE8's world body: unpacked when compressed (srp sends LZMA2), with
/// the cap of the client's buffer. (bodies, streams not unpacked, unpack ns).
#[allow(clippy::type_complexity)]
fn unpack_worlds(sent: &[Sent], logs: &[PathBuf]) -> (Vec<(String, Vec<u8>)>, usize, Vec<u64>) {
    let mut out = Vec::new();
    let mut failed = 0;
    let mut times = Vec::new();
    for (n, message) in sent.iter().enumerate() {
        if message.id != messages::id::SNAPSHOT {
            continue;
        }
        let name = format!("{}:msg{}", logs[message.log].display(), n);
        match message.payload.get(1) {
            Some(0) => out.push((name, message.payload[2..].to_vec())),
            Some(1) => {
                let stream = &message.payload[2..];
                let start = Instant::now();
                let body = survarium::v026g::lzma2::unpack(stream, WORLD_BUFFER);
                times.push(start.elapsed().as_nanos() as u64);
                match body {
                    Some(body) => out.push((name, body)),
                    None => failed += 1,
                }
            }
            _ => failed += 1,
        }
    }
    (out, failed, times)
}

// ---- worlds ---------------------------------------------------------------

/// The match a world is checked against: the payload's fields.
#[derive(Clone, Copy)]
struct Match {
    places: usize,
    game_mode: u8,
    batteries: usize,
    respawn: usize,
}

/// The body after a state's time.
fn body_of(state: &[u8]) -> &[u8] {
    state.get(4..).unwrap_or_default()
}

/// The context of a state (its time, then the body).
fn context(state: &[u8], m: Match) -> Context<'static> {
    Context {
        time: state
            .first_chunk::<4>()
            .map_or(0, |b| u32::from_le_bytes(*b)),
        places: m.places,
        game_mode: m.game_mode,
        batteries: m.batteries,
        respawn_points: Some(m.respawn),
        profiles: None,
        artefacts: None,
        objects: None,
        inventory: None,
    }
}

/// The first match that lets `body` pass, the payload's 20 places first;
/// `None` when none does.
fn choose(state: &[u8]) -> Option<Match> {
    let body = body_of(state);
    let frame = world::frame(body, MAX_PLAYERS).ok()?;
    let places = std::iter::once(MAX_PLAYERS).chain(1..MAX_PLAYERS);
    for places in places {
        for game_mode in MODES {
            let batteries: Vec<usize> = if mode::WITH_BATTERIES.contains(&game_mode) {
                (0..=MAX_BATTERIES).collect()
            } else {
                vec![0]
            };
            for batteries in batteries {
                let m = Match {
                    places,
                    game_mode,
                    batteries,
                    respawn: 0,
                };
                if world::statistics(&frame.statistics(), &context(state, m)).is_err() {
                    continue;
                }
                for respawn in 0..=MAX_RESPAWN {
                    let m = Match { respawn, ..m };
                    if world::rules(&frame.rules(), &context(state, m)).is_ok()
                        && world::validate(body, &context(state, m)).is_ok()
                    {
                        return Some(m);
                    }
                }
            }
        }
    }
    None
}

/// ns a call of `f`: the median and the least of five batches, each about
/// `budget_ns` long.
fn time<F: FnMut() -> bool>(budget_ns: u128, mut f: F) -> (u64, u64) {
    let start = Instant::now();
    black_box(f());
    let once = start.elapsed().as_nanos().max(20);
    let reps = (budget_ns / once).clamp(3, 100_000) as u32;
    let mut batches = Vec::with_capacity(5);
    for _ in 0..5 {
        let start = Instant::now();
        for _ in 0..reps {
            black_box(f());
        }
        batches.push((start.elapsed().as_nanos() / u128::from(reps)) as u64);
    }
    batches.sort_unstable();
    (batches[2], batches[0])
}

struct Row {
    source: String,
    bytes: usize,
    players: u32,
    m: Option<Match>,
    verdict: String,
    min: u64,
    median: u64,
    /// frame, players (heads and tails), bullets, statistics, objects,
    /// rules, physics, effects.
    parts: [u64; 8],
}

fn bench_worlds(worlds: &[(String, Vec<u8>)], kept: usize, options: &Options) {
    let budget = options.budget_ns;
    let mut rows = Vec::with_capacity(worlds.len());
    for (source, state) in worlds {
        let m = choose(state);
        // A world no match takes is timed with the payload's 20 places and
        // mode 2: the refusal's path.
        let used = m.unwrap_or(Match {
            places: MAX_PLAYERS,
            game_mode: mode::BATTERIES,
            batteries: 0,
            respawn: 0,
        });
        let ctx = context(state, used);
        let body = body_of(state);
        let verdict = match world::validate(body, &ctx) {
            Ok(()) => "OK".to_string(),
            Err(refusal) => format!("{}: {}", refusal.part, refusal.reason),
        };
        let (median, min) = time(budget, || world::validate(black_box(body), &ctx).is_ok());
        let mut parts = [0u64; 8];
        if let Ok(frame) = world::frame(body, ctx.places) {
            let part_budget = budget / 4;
            parts[0] = time(part_budget, || {
                world::frame(black_box(body), ctx.places).is_ok()
            })
            .0;
            parts[1] = time(part_budget, || {
                Blocks::of(black_box(body)).all(|block| {
                    world::player_head(&block, &ctx).is_ok()
                        && world::player_tail(&block, &ctx).is_ok()
                })
            })
            .0;
            parts[2] = time(part_budget, || {
                world::bullets(&frame, black_box(body), &ctx).is_ok()
            })
            .0;
            parts[3] = time(part_budget, || {
                world::statistics(&frame.statistics(), &ctx).is_ok()
            })
            .0;
            parts[4] = time(part_budget, || {
                world::objects(&frame.objects(), &ctx).is_ok()
            })
            .0;
            parts[5] = time(part_budget, || world::rules(&frame.rules(), &ctx).is_ok()).0;
            parts[6] = time(part_budget, || world::physics(&frame.physics()).is_ok()).0;
            parts[7] = time(part_budget, || world::effects(&frame.effects()).is_ok()).0;
        }
        let players = body
            .first_chunk::<4>()
            .map_or(0, |b| u32::from_le_bytes(*b).count_ones());
        rows.push(Row {
            source: source.clone(),
            bytes: body.len(),
            players,
            m,
            verdict,
            min,
            median,
            parts,
        });
    }

    if let Some(dir) = &options.dump {
        // Each world no match takes, as the game reads it (time, body).
        std::fs::create_dir_all(dir).expect("the dump folder");
        for (n, ((_, state), row)) in worlds.iter().zip(&rows).enumerate() {
            if row.m.is_none() {
                std::fs::write(dir.join(format!("refused_{n:05}.state")), state).expect("a dump");
            }
        }
    }
    let ok = rows.iter().filter(|r| r.verdict == "OK").count();
    println!(
        "\nWorlds: {} ({} kept, {} from the logs); {} pass with some match, {} at the payload's 20 places",
        rows.len(),
        kept,
        rows.len() - kept,
        ok,
        rows.iter()
            .filter(|r| r.m.is_some_and(|m| m.places == MAX_PLAYERS))
            .count()
    );
    let mut refusals: BTreeMap<String, usize> = BTreeMap::new();
    for row in rows.iter().filter(|r| r.verdict != "OK") {
        let key = row.verdict.split(" at ").next().unwrap_or(&row.verdict);
        let key: String = key.chars().take(60).collect();
        *refusals.entry(key).or_default() += 1;
    }
    for (reason, count) in &refusals {
        println!("  refused by every match: {count:5}  {reason}");
    }

    let summary = |label: &str, rows: &[&Row]| {
        if rows.is_empty() {
            return;
        }
        let mut ns: Vec<u64> = rows.iter().map(|r| r.median).collect();
        ns.sort_unstable();
        let mut bytes: Vec<u64> = rows.iter().map(|r| r.bytes as u64).collect();
        bytes.sort_unstable();
        println!(
            "  {label:28} n={:5}  us: min {:7.2} median {:7.2} p99 {:7.2} max {:7.2}   bytes median {} max {}",
            ns.len(),
            ns[0] as f64 / 1e3,
            pct(&ns, 50.0) as f64 / 1e3,
            pct(&ns, 99.0) as f64 / 1e3,
            ns[ns.len() - 1] as f64 / 1e3,
            pct(&bytes, 50.0),
            bytes[bytes.len() - 1]
        );
    };
    println!("\nworld::validate, host build, us a call (median of 5 batches):");
    let all: Vec<&Row> = rows.iter().collect();
    summary("all", &all);
    summary(
        "passing (whole path)",
        &rows
            .iter()
            .filter(|r| r.verdict == "OK")
            .collect::<Vec<_>>(),
    );
    summary(
        "refused",
        &rows
            .iter()
            .filter(|r| r.verdict != "OK")
            .collect::<Vec<_>>(),
    );
    for game_mode in MODES {
        summary(
            &format!("passing, mode {game_mode}"),
            &rows
                .iter()
                .filter(|r| r.m.is_some_and(|m| m.game_mode == game_mode))
                .collect::<Vec<_>>(),
        );
    }
    for (lo, hi) in [(0, 2), (3, 6), (7, 12), (13, 20)] {
        summary(
            &format!("passing, {lo}..{hi} players"),
            &rows
                .iter()
                .filter(|r| r.verdict == "OK" && (lo..=hi).contains(&r.players))
                .collect::<Vec<_>>(),
        );
    }

    let names = [
        "frame",
        "players",
        "bullets",
        "statistics",
        "objects",
        "rules",
        "physics",
        "effects",
    ];
    println!("\nBy part, passing worlds (ns, median / max):");
    for (i, name) in names.iter().enumerate() {
        let mut ns: Vec<u64> = rows
            .iter()
            .filter(|r| r.verdict == "OK")
            .map(|r| r.parts[i])
            .collect();
        ns.sort_unstable();
        if !ns.is_empty() {
            println!("  {name:11} {:8} {:8}", pct(&ns, 50.0), ns[ns.len() - 1]);
        }
    }

    let mut worst: Vec<&Row> = rows.iter().collect();
    worst.sort_by_key(|r| std::cmp::Reverse(r.median));
    println!("\nThe ten slowest:");
    for row in worst.iter().take(10) {
        println!(
            "  {:8.2} us  {:6} bytes  {:2} players  {}  {}  {}",
            row.median as f64 / 1e3,
            row.bytes,
            row.players,
            describe(row.m),
            row.verdict,
            row.source
        );
    }

    if let Some(path) = &options.tsv {
        let mut out = std::fs::File::create(path).expect("the tsv opens");
        writeln!(
            out,
            "source\tbytes\tplayers\tplaces\tmode\tbatteries\trespawn\tverdict\tns_min\tns_median\t{}",
            names.join("\t")
        )
        .unwrap();
        for row in &rows {
            let m = row.m.map_or("-\t-\t-\t-".to_string(), |m| {
                format!(
                    "{}\t{}\t{}\t{}",
                    m.places, m.game_mode, m.batteries, m.respawn
                )
            });
            writeln!(
                out,
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                row.source,
                row.bytes,
                row.players,
                m,
                row.verdict,
                row.min,
                row.median,
                row.parts.map(|ns| ns.to_string()).join("\t")
            )
            .unwrap();
        }
    }
}

fn describe(m: Option<Match>) -> String {
    m.map_or("no match".to_string(), |m| {
        format!(
            "places {} mode {} batteries {} respawn {}",
            m.places, m.game_mode, m.batteries, m.respawn
        )
    })
}

fn pct(sorted: &[u64], p: f64) -> u64 {
    let at = ((sorted.len() as f64 - 1.0) * p / 100.0).round() as usize;
    sorted[at.min(sorted.len() - 1)]
}

fn median(mut values: Vec<u64>) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    pct(&values, 50.0)
}

/// Every file under `dir` whose name `wanted` takes, following no symbolic
/// link (a Wine prefix's `dosdevices/z:` is `/`).
fn collect(dir: &Path, wanted: &dyn Fn(&str) -> bool, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect(&path, wanted, files);
        } else if path
            .file_name()
            .is_some_and(|name| wanted(&name.to_string_lossy()))
        {
            files.push(path);
        }
    }
}

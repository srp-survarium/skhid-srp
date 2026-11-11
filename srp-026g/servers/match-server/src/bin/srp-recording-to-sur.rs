//! Makes the client's own replay file (`.sur`, `docs/notes/replay_sur.md`)
//! from a match recording (`SRP_RECORD_DIR`, `match_server::v026g::record`).
//!
//! ```text
//! srp-recording-to-sur <file.srprec> [--out <dir>] [--stored] [--every <ms>]
//! ```
//!
//! The mode's config is the game data's (`SRP_GAME_DATA`): the
//! `gameplay/game_modes/` file the recording's map id names in `maps_dict`,
//! as the server's own conversion takes it. The result is `<out>/<name>.sur`,
//! `<name>` the recording's own file name; a summary is printed.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use match_server::v026g::record;
use match_server::v026g::sur::Coding;
use match_server::v026g::sur_convert::{self, Settings};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |name: &str| {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|at| args.get(at + 1))
            .cloned()
    };
    let Some(input) = args.iter().find(|arg| {
        !arg.starts_with("--") && Path::new(arg).extension().is_some_and(|e| e == "srprec")
    }) else {
        eprintln!(
            "usage: srp-recording-to-sur <file.srprec> [--out <dir>] [--stored] [--every <ms>]"
        );
        return ExitCode::from(2);
    };
    // The players' dictionary of the recording's revision: without it the
    // 0xE4 of a match whose kits hold items newer than the test archive's
    // (every live match since the ported weapons) does not parse.
    if let Err(error) = survarium::v026g::game_data::init() {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    let mut settings = Settings::default();
    if args.iter().any(|arg| arg == "--stored") {
        settings.coding = Coding::Stored;
    }
    if let Some(every) = value("--every").and_then(|v| v.parse().ok()) {
        settings.key_frame_every_ms = every;
    }
    settings.first_key_frame_only = args.iter().any(|arg| arg == "--first-key-frame-only");
    if let Some(list) = value("--keep-key-frames") {
        settings.keep_key_frames = Some(
            list.split(',')
                .filter_map(|t| t.trim().parse().ok())
                .collect(),
        );
    }
    if let Some(list) = value("--drop-key-frames") {
        settings.drop_key_frames = list
            .split(',')
            .filter_map(|t| t.trim().parse().ok())
            .collect();
    }
    // The notes first, then the datagrams as they are read: a long match
    // is never all in memory (`sur_convert::convert_file`).
    let notes = match record::read_notes(Path::new(input)) {
        Ok(notes) => notes,
        Err(error) => {
            eprintln!("{input}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let Some(config_name) = sur_convert::config_name(&notes) else {
        eprintln!("{input}: no header note");
        return ExitCode::FAILURE;
    };
    let Some(config) = sur_convert::config(&config_name) else {
        eprintln!("{input}: the game data has no mode config {config_name}");
        return ExitCode::FAILURE;
    };
    let converted = sur_convert::convert_file(Path::new(input), &notes, config, settings);
    let (file, report) = match converted {
        Ok(done) => done,
        Err(error) => {
            eprintln!("{input}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let out_dir = PathBuf::from(value("--out").unwrap_or_else(|| ".".into()));
    let name = Path::new(input)
        .file_stem()
        .map_or_else(|| "replay".into(), |s| s.to_string_lossy().into_owned());
    let out = out_dir.join(format!("{name}.sur"));
    if let Err(error) = std::fs::create_dir_all(&out_dir).and_then(|()| std::fs::write(&out, &file))
    {
        eprintln!("{}: {error}", out.display());
        return ExitCode::FAILURE;
    }
    println!(
        "match {} map {} mode {}, {} places at the start, roster {}",
        report.match_id, report.map, report.mode, report.places, report.roster
    );
    println!(
        "recording: {} datagrams out, {} in ({} not parsed); clock offset {} ms",
        report.datagrams_out, report.datagrams_in, report.unparsed, report.clock_offset_ms
    );
    println!(
        "events: {} inputs ({} copies dropped, {} conflicts, {} dropped as future, {} flagged), {} kit choices, {} entered, {} left",
        report.inputs,
        report.input_copies,
        report.input_conflicts,
        report.dropped_future,
        report.flagged_inputs,
        report.profiles,
        report.entered,
        report.left
    );
    println!(
        "relay copies dropped from places whose link was backed up: {:?} (place: copies); messages with no copy kept: {}; masks of {}",
        report.lagging_copies,
        report.messages_lost,
        if report.growing_roster {
            "20 (growing roster)"
        } else {
            "the roster"
        }
    );
    println!(
        "worlds: {} uploads complete, {} item times, {} times where clients disagreed; first {:?}",
        report.worlds_complete,
        report.world_times,
        report.worlds_disagreeing,
        report.first_world_ms
    );
    println!(
        "replay: {} ms to {} ms, {} key frames ({:?} ...), body {} bytes, decoded {} bytes",
        report.start_ms,
        report.end_ms,
        report.key_frames.len(),
        &report.key_frames[..report.key_frames.len().min(4)],
        report.body_bytes,
        report.decoded_bytes
    );
    println!("key frame times (ms): {:?}", report.key_frames);
    let roster: Vec<String> = report
        .roster_events
        .iter()
        .map(|(t, k, p)| format!("{k}{p}@{t}"))
        .collect();
    println!(
        "late joiners: {}; installs (place, world ms, objects): {:?}, {} exact; not sure: {:?}; key frames padded: {}, left out: {}",
        report.late_joiners,
        report.installs,
        report.installs_exact,
        report.installs_unsure,
        report.padded_key_frames,
        report.dropped_unsure_key_frames
    );
    println!("roster events: {}", roster.join(" "));
    println!("inputs per minute: {:?}", report.inputs_per_minute);
    if !report.unparsed_why.is_empty() {
        println!("not parsed: {:?}", report.unparsed_why);
    }
    if args.iter().any(|arg| arg == "--sections") {
        for (time, parts) in &report.key_frame_parts {
            println!(
                "key frame {time}: players, bullets, statistics, objects, physics, rules, effects = {parts:?}"
            );
        }
    }
    for note in &report.notes {
        println!("note: {note}");
    }
    println!(
        "wrote {} ({} bytes; recording {} bytes)",
        out.display(),
        file.len(),
        std::fs::metadata(input).map_or(0, |m| m.len())
    );
    ExitCode::SUCCESS
}

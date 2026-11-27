//! `srpde-pack`: turn a captured serialized world item into a `.srpde` file the match server loads (docs/notes/online.md
//! section 2, capture procedure). The item is the raw bytes the stock client's item writer 0x2002FC5A0 (full = 1) produced
//! at the end of a local-mode `on_match_ready`; the tool checks what can be checked offline (time field, size cap, header
//! shape) and writes the file. Nothing here invents a world.
//!
//! ```text
//! srpde-pack <item.bin> <out.srpde> --map N --mode N --match-id N --checksum HEX --tick N --send-period N --capture-roster SPEC
//! ```
//! * `--checksum`: the client's own `Client = %x` of the log line "Checksum on <map>: Client = ..., Server = ..."
//! * `--tick` / `--send-period`: `tick_ms` (+0x1083) and send period (+0x1078) of the options the client ran with.
//! * `--lzma2`: `item.bin` is already a raw LZMA2 stream (as it is sent in DE with compressed = 1).
//! * `--capture-roster`: the objects the capture's own roster put in the world (`local_match::roster_spec`, logged by the
//!   capture as "roster objects"; e.g. `MG5,MG5,MG5`): checked against the head of G1 and stored, so the match server can
//!   swap them for each match's roster (`World::with_roster`).
//! * `--kd-at`: G3 offset of the kd_stats per-record block (42 in TDM / CTP / slaughter);
//!   checked to be zero. Without it the world only serves a roster with the capture's record count.
//! * `--teams-from-end`: distance of the timelimit rule's two team counts from G3's end (16 in TDM / CTP / slaughter);
//!   checked = the capture's records / 2. Needed with `--kd-at` for rosters of another size.
//! * `--project NAME`: the level project of the capture (a PvE world: `pve_01_v{1..3}_maze{1,2}`, logged by the capture
//!   as "project"); the six PvE projects share map 15 / mode 7 and are told apart by it. A PvE project makes the world
//!   fixed-roster: it serves only the capture's record count (the human + the 17 bots), `--kd-at` is refused.
//! * `--scene-env-from ITEM`: a PvE capture's next item (t = 100, after the client's first world step; same capture):
//!   its scripting-rule environment list replaces the t = 50 item's empty one (`World::with_scene_env_from`; an online
//!   client rolls back across its own first step and crashes on an empty list).
//! * `--emit SPEC OUT`: also write the item the match server would send to a roster SPEC (a check against references).

use match_server::world::{RosterObjects, World};
use protocol::lzma2;
use std::process::ExitCode;

fn num(v: &str) -> Result<u32, String> {
    let (t, radix) = match v.strip_prefix("0x") {
        Some(h) => (h, 16),
        None => (v, 10),
    };
    u32::from_str_radix(t, radix).map_err(|e| format!("{v}: {e}"))
}

fn run() -> Result<String, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("usage: srpde-pack <item.bin> <out.srpde> --map N --mode N --match-id N --checksum HEX --tick N --send-period N --capture-roster SPEC [--lzma2]".into());
    }
    let (input, output) = (&args[0], &args[1]);
    let (mut map, mut mode, mut match_id, mut checksum, mut tick, mut period) = (None, None, None, None, None, None);
    let mut packed = false;
    let mut roster = None;
    let mut kd_at = None;
    let mut teams = None;
    let mut emit: Option<(String, String)> = None;
    let mut project: Option<String> = None;
    let mut scene_env: Option<String> = None;
    let mut i = 2;
    while i < args.len() {
        let key = args[i].as_str();
        if key == "--lzma2" {
            packed = true;
            i += 1;
            continue;
        }
        let val = args.get(i + 1).ok_or_else(|| format!("{key} needs a value"))?;
        match key {
            "--map" => map = Some(num(val)?),
            "--mode" => mode = Some(num(val)?),
            "--match-id" => match_id = Some(num(val)?),
            "--checksum" => checksum = Some(u32::from_str_radix(val.trim_start_matches("0x"), 16).map_err(|e| format!("{val}: {e}"))?),
            "--tick" => tick = Some(num(val)?),
            "--send-period" => period = Some(num(val)?),
            "--kd-at" => kd_at = Some(num(val)?),
            "--teams-from-end" => teams = Some(num(val)?),
            "--project" => project = Some(val.clone()),
            "--scene-env-from" => scene_env = Some(val.clone()),
            "--emit" => {
                let out = args.get(i + 2).ok_or("--emit needs SPEC and OUT")?;
                emit = Some((val.clone(), out.clone()));
                i += 1;
            }
            "--capture-roster" => roster = Some(RosterObjects::from_spec(val).map_err(|e| e.to_string())?),
            other => return Err(format!("unknown option {other}")),
        }
        i += 2;
    }
    let need = |v: Option<u32>, n: &str| v.ok_or_else(|| format!("--{n} is required"));
    let (map, mode, match_id) = (need(map, "map")?, need(mode, "mode")?, need(match_id, "match-id")?);
    let (checksum, tick, period) = (need(checksum, "checksum")?, need(tick, "tick")?, need(period, "send-period")?);
    if tick == 0 || tick > 255 || period == 0 || period > 0xFFFF {
        return Err("--tick must be 1..255 and --send-period 1..65535".into());
    }
    let bytes = std::fs::read(input).map_err(|e| format!("{input}: {e}"))?;
    let item = if packed { lzma2::decompress(&bytes).map_err(|e| format!("lzma2: {e}"))? } else { bytes };
    let roster = roster.ok_or("--capture-roster is required")?;
    let mut w = World::from_item(map, mode, match_id, checksum, tick as u8, period as u16, item).map_err(|e| e.to_string())?.with_capture_roster(&roster).map_err(|e| e.to_string())?;
    if let Some(p) = &project {
        w = w.with_project(p).map_err(|e| e.to_string())?;
    }
    if let Some(path) = &scene_env {
        let later = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        w = w.with_scene_env_from(&later).map_err(|e| e.to_string())?;
    }
    if let Some(at) = kd_at {
        w = w.with_record_block(at).map_err(|e| e.to_string())?;
    }
    if let Some(t) = teams {
        w = w.with_team_counts(t).map_err(|e| e.to_string())?;
    }
    if packed {
        w.body = lzma2::compress(&w.body);
        w.already_lzma2 = true;
    }
    std::fs::write(output, w.to_file_bytes()).map_err(|e| format!("{output}: {e}"))?;
    if let Some((spec, out)) = &emit {
        let r = RosterObjects::from_spec(spec).map_err(|e| e.to_string())?;
        let item = w.with_roster(&r).and_then(|m| m.item()).map_err(|e| e.to_string())?;
        std::fs::write(out, item).map_err(|e| format!("{out}: {e}"))?;
    }
    Ok(format!(
        "{output}: map {map} mode {mode} project {:?} match_id {match_id:#x} time {} item {} bytes, capture roster {} objects ({} bytes), {} record(s){}, kd_stats at {}",
        w.project,
        w.time,
        w.item().map(|i| i.len()).unwrap_or(0),
        w.roster_objects,
        w.roster_bytes,
        w.records,
        if w.fixed_roster { " (fixed: only this record count)" } else { "" },
        if w.records_at == match_server::world::RECORDS_NOT_LOCATED { "not located".to_string() } else { format!("G3+{}", w.records_at) }
    ))
}

fn main() -> ExitCode {
    match run() {
        Ok(line) => {
            println!("{line}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("srpde-pack: {e}");
            ExitCode::FAILURE
        }
    }
}

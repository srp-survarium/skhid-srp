//! Probe a running `world-host daemon`: for each level project named, send the `Create` the relay sends for a PvP match
//! whose start world the host builds (protocol 4: empty world; the options of one default test human on that data row, the
//! production writer `local_match::online_options_row`) and wait for its `Ready` (the start world) or `Fault`. Prints one
//! line per project. The connection is closed afterwards (the match child exits on EOF).
//!
//! Ignored by default (it needs a running daemon whose config lists the projects):
//!   SRP_PROBE_SOCK=<socket> SRP_PROBE_PROJECTS=level_06_day_tdm,level_03_rain_slr_vn [SRP_PROBE_TIMEOUT_SECS=180]
//!   cargo test -p world-host --test start_world_probe -- --ignored --nocapture
#![cfg(unix)]

use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};
use world_host_proto::{read_frame, Create, FromHost, Record, ToHost, KIND_PVP, PROTO_VERSION};

fn probe(gd: &game_data::GameData, sock: &str, project: &str, timeout: Duration, match_id: u32) -> Result<String, String> {
    let row = gd.matches.modes().iter().find(|r| r.project == project).ok_or("no data row with this project")?;
    let tick = local_match::options::TICK_MS as u8;
    let p = local_match::OnlineParams {
        map: row.map,
        mode: row.mode,
        weather: row.variation,
        match_id,
        tick_ms: tick,
        send_period_ms: local_match::options::SEND_PERIOD_MS,
        match_type: local_match::options::MATCH_TYPE_PLAIN,
        max_delay_ms: local_match::options::max_delay_ms(tick),
    };
    let human = local_match::OnlinePlayer { team: 0, human: local_match::HumanLoadout::default_loadout(), hero: None, pve_budget: (0, 0) };
    let options = local_match::online_options_row(gd, &p, row, &[human], &[]).map_err(|e| format!("options: {e}"))?;
    let create = Create {
        version: PROTO_VERSION,
        match_id,
        kind: KIND_PVP,
        project: project.to_string(),
        map: row.map,
        mode: row.mode,
        tick_ms: tick,
        records: vec![Record { bot: false, team: 0 }],
        options,
        world: Vec::new(),
        drive_bots: false,
        ai_ignore_players: false,
    };
    let mut s = UnixStream::connect(sock).map_err(|e| format!("connect: {e}"))?;
    s.set_read_timeout(Some(timeout)).ok();
    use std::io::Write;
    s.write_all(&ToHost::Create(create).frame().map_err(|e| format!("frame: {e}"))?).map_err(|e| format!("write: {e}"))?;
    let t0 = Instant::now();
    loop {
        let f = read_frame(&mut s).map_err(|e| format!("read after {:?}: {e}", t0.elapsed()))?.ok_or("EOF before Ready")?;
        match FromHost::decode(&f).map_err(|e| format!("decode: {e}"))? {
            FromHost::Ready(r) => {
                let _ = s.shutdown(std::net::Shutdown::Both);
                return match r.start {
                    Some(w) if !w.is_empty() => Ok(format!(
                        "Ready after {:.1} s: start world {} bytes, world time {}, collision checksum {:08x}, create {} ms",
                        t0.elapsed().as_secs_f32(),
                        w.len(),
                        r.world_time,
                        r.collision_checksum,
                        r.create_ms
                    )),
                    _ => Err("Ready without a start world".into()),
                };
            }
            FromHost::Fault { reason } => return Err(format!("Fault after {:.1} s: {reason}", t0.elapsed().as_secs_f32())),
            _ => {}
        }
    }
}

#[test]
#[ignore]
fn the_daemon_builds_the_start_world_of_each_project() {
    game_data::require_tables!();
    let sock = std::env::var("SRP_PROBE_SOCK").expect("SRP_PROBE_SOCK");
    let projects = std::env::var("SRP_PROBE_PROJECTS").expect("SRP_PROBE_PROJECTS");
    let timeout = Duration::from_secs(std::env::var("SRP_PROBE_TIMEOUT_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(180));
    let gd = game_data::testing::tables();
    let mut failed = 0;
    for (i, p) in projects.split(',').map(str::trim).filter(|p| !p.is_empty()).enumerate() {
        match probe(&gd, &sock, p, timeout, 0x5EED_0100 + i as u32) {
            Ok(m) => println!("PROBE OK   {p}: {m}"),
            Err(e) => {
                failed += 1;
                println!("PROBE FAIL {p}: {e}");
            }
        }
    }
    assert_eq!(failed, 0, "{failed} project(s) failed");
}

//! Tables of a match recording (local analysis tool, A187 reader validation; not for a PR):
//!
//! ```text
//! srp-rec-table <file.srprec> <out dir>
//! ```
//!
//! Writes, read the way the replay converter reads them (`sur_convert::tables_of_file`):
//!
//! - `rec_inputs.csv`: `match,time,place,actions,rot_a,rot_b`: every input relayed (0xEB/0xEC/0xED/0xEE) or accepted, at its
//!   match time; `rot_a`, `rot_b` the two halves of the relayed `half2 rotation`, as floats;
//! - `rec_worlds.csv`: `match,time,upload,uploads,same,place,alive,cx,cy,cz,fx,fy,fz,yaw,pitch,keys,speed,inserted`: every
//!   player block of every world a client uploaded (upload `k` of `uploads` at that time, `same` 1 when all the uploads of
//!   the time are byte-equal), the body's centre (`m_last_pos`) and feet in the map's frame;
//! - `rec_head.txt`: the header's map, mode and place count, the note kinds, the report's counts. No account is written.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use match_server::v026g::record::{self, NOTE};
use match_server::v026g::sur_convert;
use survarium::v026g::match_msgs::motion::Motion;
use survarium::v026g::match_msgs::players;

fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from((bits >> 10) & 0x1F);
    let frac = f32::from(bits & 0x3FF);
    match exp {
        0 => sign * frac * 2f32.powi(-24),
        31 => f32::NAN,
        _ => sign * (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, out] = args.as_slice() else {
        eprintln!("usage: srp-rec-table <file.srprec> <out dir>");
        return ExitCode::from(2);
    };
    match run(Path::new(input), Path::new(out)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{input}: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(input: &Path, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(out)?;
    let notes = record::read_notes(input)?;
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut head = String::new();
    for (k, note) in notes.iter().filter(|r| r.kind == NOTE).enumerate() {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&note.bytes) else {
            *kinds.entry("unparsed".into()).or_default() += 1;
            continue;
        };
        if k == 0 {
            head = format!(
                "header: match {} map {} mode {} places {}",
                value["match_id"],
                value["map"],
                value["mode"],
                value["places"].as_array().map_or(0, Vec::len)
            );
        }
        let kind = value
            .as_object()
            .and_then(|o| o.get("note").or_else(|| o.get("kind")).or_else(|| o.get("event")))
            .and_then(|v| v.as_str())
            .map_or_else(|| value.as_object().map_or("?".into(), |o| o.keys().cloned().collect::<Vec<_>>().join("+")), str::to_owned);
        *kinds.entry(kind).or_default() += 1;
    }
    let tables = sur_convert::tables_of_file(input, &notes)?;
    let id = tables.report.match_id.trim_start_matches("0x").to_owned();
    let mut inputs = std::io::BufWriter::new(std::fs::File::create(out.join("rec_inputs.csv"))?);
    writeln!(inputs, "match,time,place,actions,rot_a,rot_b")?;
    for (&(time, place), value) in &tables.inputs {
        let r = value.rotation;
        writeln!(
            inputs,
            "{id},{time},{place},{:#x},{:.4},{:.4}",
            value.actions,
            half(u16::from_le_bytes([r[0], r[1]])),
            half(u16::from_le_bytes([r[2], r[3]]))
        )?;
    }
    inputs.flush()?;
    let mut worlds = std::io::BufWriter::new(std::fs::File::create(out.join("rec_worlds.csv"))?);
    writeln!(worlds, "match,time,upload,uploads,same,place,alive,cx,cy,cz,fx,fy,fz,yaw,pitch,keys,speed,inserted")?;
    let (mut read, mut unread) = (0usize, 0usize);
    for (&time, bodies) in &tables.worlds {
        let same = u8::from(bodies.windows(2).all(|w| w[0] == w[1]));
        for (k, body) in bodies.iter().enumerate() {
            for block in players::walk(body).map(|(blocks, _)| blocks).unwrap_or_default() {
                let Some((_, motion)) = Motion::find(block.state, time) else {
                    unread += 1;
                    continue;
                };
                read += 1;
                let (c, f) = motion.body.as_ref().map_or(([f32::NAN; 3], [f32::NAN; 3]), |b| (b.position, b.feet()));
                let speed = motion.damage.as_ref().map_or(f32::NAN, |d| d.linear_horizontal_speed);
                writeln!(
                    worlds,
                    "{id},{time},{k},{},{same},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.4},{:.4},{:#x},{speed:.3},{}",
                    bodies.len(),
                    block.place,
                    u8::from(motion.alive),
                    c[0],
                    c[1],
                    c[2],
                    f[0],
                    f[1],
                    f[2],
                    motion.rotation_y,
                    motion.look_pitch,
                    motion.actions_mask,
                    motion.inserted_time_ms
                )?;
            }
        }
    }
    worlds.flush()?;
    let r = &tables.report;
    let text = format!(
        "{head}\nnote kinds: {kinds:?}\nclock offset ms: {}\ndatagrams in {} out {} unparsed {} {:?}\nworlds complete {} world times {}\ninputs {} copies {} conflicts {} flagged {} dropped_future {}\nplayer blocks read {read} unread {unread}\n",
        r.clock_offset_ms,
        r.datagrams_in,
        r.datagrams_out,
        r.unparsed,
        r.unparsed_why,
        r.worlds_complete,
        r.world_times,
        r.inputs,
        r.input_copies,
        r.input_conflicts,
        r.flagged_inputs,
        r.dropped_future
    );
    std::fs::write(out.join("rec_head.txt"), text)?;
    Ok(())
}

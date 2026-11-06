//! Reads a match recording (`SRP_RECORD_DIR`, `match_server::v026g::record`).
//!
//! ```text
//! srp-recording <file.srprec>           the notes, and each place's datagrams
//! srp-recording <file.srprec> --dump    every record: ms, kind, place, length, hex
//! ```

use std::path::Path;
use std::process::ExitCode;

use match_server::v026g::record::{self, ACCEPTED_INPUT, ACCEPTED_PROFILE, IN, NOTE, OUT};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dump = args.iter().any(|arg| arg == "--dump");
    let Some(path) = args.iter().find(|arg| !arg.starts_with("--")) else {
        eprintln!("usage: srp-recording <file.srprec> [--dump]");
        return ExitCode::from(2);
    };
    let records = match record::read(Path::new(path)) {
        Ok(records) => records,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut places: Vec<[(u64, u64); 2]> = Vec::new();
    for record in &records {
        if dump {
            let kind = match record.kind {
                IN => "IN ",
                OUT => "OUT",
                NOTE => "NOTE",
                ACCEPTED_INPUT => "INPUT",
                ACCEPTED_PROFILE => "KIT",
                _ => "?",
            };
            if record.kind == NOTE {
                println!(
                    "{:>9} {kind} {}",
                    record.at_ms,
                    String::from_utf8_lossy(&record.bytes)
                );
            } else {
                let hex: Vec<String> = record.bytes.iter().map(|b| format!("{b:02x}")).collect();
                println!(
                    "{:>9} {kind} p{:<2} len={} {}",
                    record.at_ms,
                    record.place,
                    record.bytes.len(),
                    hex.join(" ")
                );
            }
            continue;
        }
        match record.kind {
            NOTE => println!(
                "{:>9} ms  {}",
                record.at_ms,
                String::from_utf8_lossy(&record.bytes)
            ),
            IN | OUT => {
                let place = usize::from(record.place);
                if places.len() <= place {
                    places.resize(place + 1, [(0, 0); 2]);
                }
                let slot = &mut places[place][usize::from(record.kind == OUT)];
                slot.0 += 1;
                slot.1 += record.bytes.len() as u64;
            }
            _ => {}
        }
    }
    if !dump {
        println!();
        println!("place  datagrams in (bytes)  datagrams out (bytes)");
        for (place, [inbound, outbound]) in places.iter().enumerate() {
            println!(
                "{place:>5}  {:>9} ({:>9})  {:>9} ({:>10})",
                inbound.0, inbound.1, outbound.0, outbound.1
            );
        }
        let last = records.last().map_or(0, |record| record.at_ms);
        println!(
            "{} records over {:.1} min",
            records.len(),
            f64::from(last) / 60_000.0
        );
    }
    ExitCode::SUCCESS
}

//! The items workbook (`/items.xlsx`) of a server's game data, written to a
//! file, as the site serves it:
//!
//!     cargo run -p web-server --example items-xlsx -- <server-game-data.db> <out.xlsx>
//!
//! The data is checked as `srp` checks it at start.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [data, out] = args.as_slice() else {
        eprintln!("usage: items-xlsx <server-game-data.db> <out.xlsx>");
        return ExitCode::from(2);
    };
    if let Err(cause) = survarium::v026g::game_data::init_from(data.as_ref()) {
        eprintln!("{data}: {cause}");
        return ExitCode::FAILURE;
    }
    match web_server::items_workbook() {
        Ok(bytes) => match std::fs::write(out, bytes) {
            Ok(()) => {
                println!("{out}: {} bytes", bytes.len());
                ExitCode::SUCCESS
            }
            Err(cause) => {
                eprintln!("{out}: {cause}");
                ExitCode::FAILURE
            }
        },
        Err(cause) => {
            eprintln!("the workbook: {cause}");
            ExitCode::FAILURE
        }
    }
}

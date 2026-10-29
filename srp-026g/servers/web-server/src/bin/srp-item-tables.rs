//! `srp-item-tables <server-game-data.db> --dir <dir>`: the per-release item
//! tables (`equipment.tsv`, `weapons.tsv`; [`web_server::item_tables`]) of
//! that game data, written into `<dir>` where they differ from what is
//! there. Says `unchanged` for a table that is the same, else what changed
//! (an item's column `old -> new`, an item new or gone). Exit status 1 when
//! the data does not read (as `srp` would refuse it) or a file cannot be
//! written.

use std::path::PathBuf;

use survarium::v026g::game_data;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [data, flag, dir] = args.as_slice() else {
        eprintln!("usage: srp-item-tables <server-game-data.db> --dir <dir>");
        std::process::exit(2);
    };
    if flag != "--dir" {
        eprintln!("usage: srp-item-tables <server-game-data.db> --dir <dir>");
        std::process::exit(2);
    }
    if let Err(error) = game_data::init_from(&PathBuf::from(data)) {
        eprintln!("{data}: srp would not start on it: {error}");
        std::process::exit(1);
    }
    match web_server::item_tables::update(&PathBuf::from(dir)) {
        Ok(report) => {
            for line in report {
                println!("{line}");
            }
        }
        Err(error) => {
            eprintln!("{dir}: {error}");
            std::process::exit(1);
        }
    }
}

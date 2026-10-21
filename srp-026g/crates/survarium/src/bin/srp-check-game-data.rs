//! `srp-check-game-data <server-game-data.db>`: checks the server's game
//! data as `srp` does at start (`survarium::v026g::game_data::init`: every
//! table, and the golden weapons' patch), so that `release/build-server-db`
//! refuses data `srp` would refuse after the swap. Exit status 1 with the
//! reason when it is refused.

use std::path::PathBuf;

use survarium::v026g::game_data;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [path] = args.as_slice() else {
        eprintln!("usage: srp-check-game-data <server-game-data.db>");
        std::process::exit(2);
    };
    match game_data::init_from(&PathBuf::from(path)) {
        Ok(data) => println!("{}: srp starts on it", data.origin),
        Err(error) => {
            eprintln!("{path}: srp would not start on it: {error}");
            std::process::exit(1);
        }
    }
}

//! Prints [`session::v026g::wiki::rules`] over the server's game data
//! (`SRP_GAME_DATA`), or with `--test-data` over the test data, for the
//! site's wiki (`srp-026g/scripts/wiki`, its README).
//!
//! ```text
//! SRP_GAME_DATA=<server-game-data.db> cargo run -p session --example wiki_rules > rules.json
//! cargo run -p session --example wiki_rules -- --test-data > rules-test.json
//! ```

use std::io::Write;

fn main() {
    if std::env::args().nth(1).as_deref() != Some("--test-data")
        && let Err(error) = survarium::v026g::game_data::init()
    {
        eprintln!("wiki_rules: {error}");
        std::process::exit(1);
    }
    let text = session::v026g::wiki::rules();
    std::io::stdout()
        .write_all(text.as_bytes())
        .expect("stdout");
}

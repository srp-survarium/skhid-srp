//! Why a world no match takes is refused: the match server's own decoder
//! (`survarium::v026g::match_msgs::snapshot::decode`) against the check's.
//!
//! ```text
//! cargo run --release -p srp-validate --example explain -- FILE...
//! ```
//!
//! Each FILE is a world as the game's reader has it (`bench --dump-refused`):
//! the time (u32), then the body. For each it prints the sections' lengths,
//! the first match (places, mode, batteries) under which the server reads
//! the statistics and the rules, and the check's verdict under it.

use srp_validate::world::{self, Context};
use srp_validate::{MAX_PLAYERS, mode};
use survarium::v026g::match_msgs::snapshot;

fn main() {
    for path in std::env::args().skip(1) {
        let state = std::fs::read(&path).expect("a state file");
        let time = u32::from_le_bytes(state[..4].try_into().unwrap());
        let body = &state[4..];
        println!("{path}: time {time}, {} bytes", body.len());
        match world::frame(body, MAX_PLAYERS) {
            Ok(frame) => println!(
                "  mask {:#x}, sections {:?}",
                frame.active_clients_mask,
                frame.sections.map(|s| s.bytes.len())
            ),
            Err(refusal) => println!("  frame: {} at {}", refusal.reason, refusal.offset),
        }
        let mut found = false;
        'search: for places in (1..=MAX_PLAYERS).rev() {
            for game_mode in [
                mode::BATTERIES,
                mode::TIMED_BATTERIES,
                mode::TEAM_DEATHMATCH,
                mode::RESEARCH,
            ] {
                for batteries in 0..=16 {
                    let Some(decoded) = snapshot::decode(time, body, places, game_mode, batteries)
                    else {
                        continue;
                    };
                    let (Some(statistics), Some(rules)) = (&decoded.statistics, &decoded.rules)
                    else {
                        continue;
                    };
                    // The whole rules: the respawn rule and the mode's.
                    if rules.respawn.is_none()
                        || !(rules.gather.is_some()
                            || rules.tdm.is_some()
                            || rules.research.is_some())
                    {
                        continue;
                    }
                    found = true;
                    println!(
                        "  server reads: places {places} mode {game_mode} batteries {batteries} \
                         ({} battery records; respawn {:?}, gather {}, tdm {}, research {})",
                        statistics.victory_items.len(),
                        rules.respawn.as_ref().map(|r| r.points.len()),
                        rules.gather.is_some(),
                        rules.tdm.is_some(),
                        rules.research.is_some()
                    );
                    let context = Context {
                        time,
                        places,
                        game_mode,
                        batteries: statistics.victory_items.len(),
                        respawn_points: rules.respawn.as_ref().map(|r| r.points.len()),
                        profiles: None,
                        artefacts: None,
                        objects: None,
                        inventory: None,
                    };
                    match world::validate(body, &context) {
                        Ok(()) => println!("  check: OK"),
                        Err(refusal) => println!(
                            "  check: {} at {}: {}",
                            refusal.part, refusal.offset, refusal.reason
                        ),
                    }
                    break 'search;
                }
            }
        }
        if !found {
            println!("  the server reads no statistics and rules under any match either");
        }
    }
}

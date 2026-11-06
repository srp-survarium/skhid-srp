//! Prints where the batteries of a battery match (modes 2 and 3) are in
//! each of its saved 0.26g world snapshots (the match server's
//! `SRP_SNAPSHOT_DIR` dumps, named `match026g_<id>_<time>_..`), in the
//! order of their times: the teams' points, each battery (carried, in a
//! container, lying at a position) and each container's users and items.
//!
//! ```text
//! cargo run -p survarium --example batteries -- <players> <mode> <dumps or a folder of them> ...
//! ```

use survarium::v026g::maps::RuleMode;
use survarium::v026g::match_msgs::snapshot::{self, Battery};

/// The match time in a dump's name.
fn time_of(path: &str) -> Option<u32> {
    let name = path.rsplit(['/', '\\']).next()?;
    name.split('_').nth(2)?.parse().ok()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let players: usize = args
        .next()
        .and_then(|arg| arg.parse().ok())
        .expect("players");
    let mode = args
        .next()
        .and_then(|arg| arg.parse().ok())
        .and_then(RuleMode::of_config)
        .expect("mode: 2 to 5");
    // A folder stands for the dumps in it.
    let mut paths: Vec<(u32, String)> = args
        .flat_map(|path| match std::fs::read_dir(&path) {
            Ok(entries) => entries
                .flatten()
                .map(|entry| entry.path().to_string_lossy().into_owned())
                .collect(),
            Err(_) => vec![path],
        })
        .filter_map(|path| Some((time_of(&path)?, path)))
        .collect();
    paths.sort();
    let mut last = String::new();
    for (time, path) in paths {
        let Ok(body) = std::fs::read(&path) else {
            println!("{time:>7}: cannot be read");
            continue;
        };
        let decoded = snapshot::decode(time, &body, players, mode, 10);
        let gather = decoded
            .as_ref()
            .and_then(|decoded| decoded.rules.as_ref())
            .and_then(|rules| rules.gather.clone());
        let Some(gather) = gather else {
            // What did read, to see how far the walk got.
            println!(
                "{time:>7}: the gather rule does not read (the body walks: {}, statistics: {}, \
                 rules: {}, {} bytes of rules)",
                decoded.is_some(),
                decoded
                    .as_ref()
                    .is_some_and(|decoded| decoded.statistics.is_some()),
                decoded
                    .as_ref()
                    .is_some_and(|decoded| decoded.rules.is_some()),
                decoded
                    .as_ref()
                    .map_or(0, |decoded| decoded.sections.rules.len()),
            );
            continue;
        };
        let batteries: Vec<String> = gather
            .batteries
            .iter()
            .map(|battery| match battery {
                Battery::Carried => "carried".to_owned(),
                Battery::InContainer(id) => format!("in{id}"),
                Battery::Free { position, .. } => {
                    format!(
                        "at({:.0},{:.0},{:.0})",
                        position[0], position[1], position[2]
                    )
                }
            })
            .collect();
        let containers: Vec<String> = gather
            .containers
            .iter()
            .map(|container| format!("users {:?} items {:?}", container.users, container.items))
            .collect();
        let line = format!(
            "points {:?} | {} | {}",
            gather.team_points,
            batteries.join(" "),
            containers.join(" ; ")
        );
        // Only what changed.
        if line != last {
            println!("{time:>7}: {line}");
            last = line;
        }
    }
}

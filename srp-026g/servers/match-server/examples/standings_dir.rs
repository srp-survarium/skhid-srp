//! The standing in each saved snapshot of a directory, one line a snapshot:
//! `match time points0 points1 tickets0 tickets1 kills:deaths:online,...`.
//! `standings_dir <directory> <matches.tsv>`; the second file names each
//! match's id (hex), roster places and map id, tab separated.
use std::collections::HashMap;

use survarium::v026g::maps;
use survarium::v026g::match_msgs::snapshot;

fn main() {
    let dir = std::env::args().nth(1).expect("directory");
    let matches = std::fs::read_to_string(std::env::args().nth(2).expect("matches")).unwrap();
    let matches: HashMap<String, (usize, u8)> = matches
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            Some((
                fields.next()?.to_owned(),
                (fields.next()?.parse().ok()?, fields.next()?.parse().ok()?),
            ))
        })
        .collect();
    let mut unread = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let mut parts = name.split('_');
        let (Some(id), Some(time)) = (parts.nth(1), parts.next()) else {
            continue;
        };
        let (Some(&(players, map_id)), Ok(time)) = (matches.get(id), time.parse::<u32>()) else {
            continue;
        };
        let Some(map) = maps::map_mode(map_id) else {
            continue;
        };
        let body = std::fs::read(format!("{dir}/{name}")).unwrap();
        let batteries = if map.mode.rules() == survarium::v026g::maps::RuleMode::TeamDeathmatch {
            0
        } else {
            map.victory_items_count as usize
        };
        let Some(decoded) = snapshot::decode(time, &body, players, map.mode.rules(), batteries) else {
            unread += 1;
            continue;
        };
        let tail = decoded
            .statistics
            .as_ref()
            .map(|statistics| statistics.tail);
        let points = tail.map_or([0; 2], |tail| tail.team_points);
        let Some(rules) = &decoded.rules else {
            unread += 1;
            continue;
        };
        let tickets = rules.tdm.map_or([0; 2], |tdm| tdm.tickets);
        let players: Vec<String> = rules
            .kills_deaths
            .iter()
            .map(|record| {
                format!(
                    "{}:{}:{}",
                    record.kills,
                    record.deaths,
                    u8::from(record.online)
                )
            })
            .collect();
        println!(
            "{id}\t{time}\t{}\t{}\t{}\t{}\t{}\t{}",
            map.mode.lobby_number(),
            points[0],
            points[1],
            tickets[0],
            tickets[1],
            players.join(",")
        );
    }
    eprintln!("{unread} snapshots could not be read");
}

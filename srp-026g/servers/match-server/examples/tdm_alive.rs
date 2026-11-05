//! The tdm rule of each saved Team Deathmatch snapshot of a directory, one
//! line a snapshot: `match time players tickets alive-mask active-mask
//! kills:deaths:online,...`. `tdm_alive <directory> <map id>`; the roster
//! size is the one with which the snapshot reads.
use survarium::v026g::maps;
use survarium::v026g::match_msgs::{players, snapshot};

fn main() {
    let dir = std::env::args().nth(1).expect("directory");
    let map_id: u8 = std::env::args().nth(2).expect("map id").parse().unwrap();
    let map = maps::map_mode(map_id).expect("map");
    // Optional: each roster place's team as digits, to test the elimination
    // check (a team with no tickets, no respawn flag, nobody queued and
    // every place of it in the alive mask dead by its block).
    let teams: Option<Vec<u8>> = std::env::args().nth(3).map(|digits| {
        digits
            .bytes()
            .map(|digit| digit.wrapping_sub(b'0'))
            .collect()
    });
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort_by_key(|name| {
        name.split('_')
            .nth(2)
            .and_then(|time| time.parse::<u32>().ok())
            .unwrap_or(0)
    });
    for name in names {
        let mut parts = name.split('_');
        let (Some(id), Some(time)) = (parts.nth(1), parts.next()) else {
            continue;
        };
        let Ok(time) = time.parse::<u32>() else {
            continue;
        };
        let body = std::fs::read(format!("{dir}/{name}")).unwrap();
        let read = (1..=20).find_map(|players| {
            let decoded = snapshot::decode(time, &body, players, map.mode.rules(), 0)?;
            let tdm = decoded.rules.as_ref()?.tdm?;
            Some((players, decoded, tdm))
        });
        let Some((players, decoded, tdm)) = read else {
            println!("{id}\t{time}\tunread");
            continue;
        };
        let records: Vec<String> = decoded
            .rules
            .as_ref()
            .unwrap()
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
        let out = teams.as_ref().map(|teams| {
            let rules = decoded.rules.as_ref().unwrap();
            let queued: Vec<u8> = rules.respawn.as_ref().map_or(Vec::new(), |respawn| {
                respawn.times.iter().map(|time| time.0).collect()
            });
            let blocks = players::blocks(&body).unwrap_or_default();
            let dead = |place: usize| {
                let Some(block) = blocks
                    .iter()
                    .find(|block| usize::from(block.place) == place)
                else {
                    return true;
                };
                let verdicts: Vec<bool> = (0..400)
                    .filter_map(|items| {
                        let head = players::PlayerState::read(block.state, items)?;
                        (head.look_pitch.is_finite()
                            && head.look_pitch.abs() < 4.0
                            && head.rotation_y.is_finite()
                            && head.rotation_y.abs() < 8.0
                            && head.effects < 40)
                            .then_some(head.alive)
                    })
                    .collect();
                !verdicts.is_empty() && verdicts.iter().all(|alive| !alive)
            };
            (0..2usize)
                .filter(|&team| {
                    tdm.tickets[team] == 0
                        && !rules.team_respawn[team]
                        && teams.iter().enumerate().all(|(place, &of)| {
                            usize::from(of) != team
                                || (!queued.contains(&(place as u8))
                                    && (tdm.alive_players_mask >> place & 1 == 0 || dead(place)))
                        })
                })
                .collect::<Vec<_>>()
        });
        let waiting = decoded
            .rules
            .as_ref()
            .and_then(|read| read.respawn.as_ref())
            .map_or(String::new(), |respawn| format!("{:?}", respawn.times));
        println!(
            "{id}\t{time}\t{players}\t{:?}\t{:020b}\t{:020b}\t{waiting}\t{:?}\tout {:?}\t{}",
            tdm.tickets,
            tdm.alive_players_mask,
            tdm.active_players_mask,
            decoded.rules.as_ref().unwrap().team_respawn,
            out,
            records.join(",")
        );
    }
}

//! Reads the worlds a 0.26g match server saved (`SRP_SNAPSHOT_DIR`) into
//! tables, for the player bots' research (`docs/notes/player_bots.md`).
//!
//! ```text
//! srp-worlds <worlds dir> <out dir>
//! ```
//!
//! For each match and world time one file is taken (a checked one first,
//! never a rejected one), and three CSV files are written:
//!
//! - `positions.csv`: every player block: alive, position in the map's
//!   frame, facing, keys, crouch and sprint, speed, health, recent hits;
//! - `statistics.csv`: each roster place's event totals and its score by
//!   the scoreboard's weights of modes 2-4 (`static_info::SCORE_WEIGHTS`);
//! - `batteries.csv`: each battery's owner team, finder, the distances
//!   the statistics keep, and how far each player has carried it;
//! - `hits.csv`: for each match, each pair of places where one hit the
//!   other (a receiver's damage from an inflictor, or an inflictor's last
//!   hit time on a receiver), with the number of worlds that show it: who
//!   fought whom, so the two sides. The damage is kept for a life only;
//! - `shapes.csv`: each match's roster places and battery count (none in
//!   Team Deathmatch).
//!
//! The roster size and the battery count are not in the file names: the
//! ones whose statistics fill their section exactly are taken.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use survarium::v026g::match_msgs::MAX_PLAYERS;
use survarium::v026g::match_msgs::motion::Motion;
use survarium::v026g::match_msgs::players;
use survarium::v026g::match_msgs::snapshot::{self, Sections, Statistics, event};
use survarium::v026g::match_msgs::static_info::SCORE_WEIGHTS;

/// What a file's name says.
struct Saved {
    path: PathBuf,
    time: u32,
    /// `checked` 0, `initial` 1, anything else 2 (`unchecked`).
    rank: u8,
}

/// `match026g_<id>_<time>_p<from>_<kind>_r<run>.bin`.
fn parse_name(name: &str) -> Option<(String, u32, &str)> {
    let rest = name.strip_prefix("match026g_")?.strip_suffix(".bin")?;
    let mut parts = rest.split('_');
    let id = parts.next()?.to_owned();
    let time = parts.next()?.parse().ok()?;
    parts.next()?.strip_prefix('p')?;
    let kind = parts.next()?;
    Some((id, time, kind))
}

/// The events written to `statistics.csv`, by name.
const EVENTS: [(&str, usize); 12] = [
    ("kills", event::KILL),
    ("assists", event::ASSIST),
    ("deaths", event::DEATH),
    ("headshots", event::HEADSHOT_KILL),
    ("bring", event::BRING_VICTORY_ITEM),
    ("steal", event::STEAL_VICTORY_ITEM),
    ("recover", event::RECOVER_VICTORY_ITEM),
    ("found", event::VICTORY_ITEM_FOUND),
    ("put_in", event::PUT_VICTORY_ITEM_IN_CONTAINER),
    ("shots", event::BULLET_FIRE),
    ("hits", event::BULLET_HIT),
    ("grenade_kills", event::GRENADE_KILL),
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [dir, out] = args.as_slice() else {
        eprintln!("usage: srp-worlds <worlds dir> <out dir>");
        return ExitCode::from(2);
    };
    match run(Path::new(dir), Path::new(out)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(dir: &Path, out: &Path) -> std::io::Result<()> {
    // match id -> time -> the best file.
    let mut matches: BTreeMap<String, BTreeMap<u32, Saved>> = BTreeMap::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((id, time, kind)) = parse_name(&name) else {
            continue;
        };
        let rank = match kind {
            "checked" => 0,
            "initial" => 1,
            "rejected" => continue,
            _ => 2,
        };
        let saved = Saved {
            path: entry.path(),
            time,
            rank,
        };
        let worlds = matches.entry(id).or_default();
        if worlds.get(&time).is_none_or(|held| held.rank > rank) {
            worlds.insert(time, saved);
        }
    }
    std::fs::create_dir_all(out)?;
    let mut positions = BufWriter::new(File::create(out.join("positions.csv"))?);
    writeln!(
        positions,
        "match,time,place,alive,x,y,z,yaw,pitch,keys,crouch,sprint,jumping,speed,health,hits,spawned"
    )?;
    let mut statistics = BufWriter::new(File::create(out.join("statistics.csv"))?);
    write!(statistics, "match,time,place,killstreak")?;
    for (name, _) in EVENTS {
        write!(statistics, ",{name}")?;
    }
    writeln!(statistics, ",score")?;
    let mut batteries = BufWriter::new(File::create(out.join("batteries.csv"))?);
    writeln!(
        batteries,
        "match,time,battery,owner_team,first_finder,nearest_0,nearest_1,carried"
    )?;

    let mut hits = BufWriter::new(File::create(out.join("hits.csv"))?);
    writeln!(hits, "match,inflictor,receiver,worlds")?;
    let mut shapes = BufWriter::new(File::create(out.join("shapes.csv"))?);
    writeln!(shapes, "match,places,batteries")?;

    let (mut worlds, mut unread, mut players_read, mut players_unread) = (0, 0, 0, 0);
    for (id, saved) in &matches {
        // (roster places, batteries) once found for the match.
        let mut shape: Option<(usize, usize)> = None;
        // (inflictor, receiver) -> worlds that show a hit.
        let mut pairs: BTreeMap<(usize, usize), u32> = BTreeMap::new();
        for world in saved.values() {
            worlds += 1;
            let body = std::fs::read(&world.path)?;
            let Some(sections) = Sections::walk(&body) else {
                unread += 1;
                continue;
            };
            for block in players::walk(&body)
                .map(|(blocks, _)| blocks)
                .unwrap_or_default()
            {
                let Some((_, motion)) = Motion::find(block.state, world.time) else {
                    players_unread += 1;
                    continue;
                };
                players_read += 1;
                write_position(&mut positions, id, world.time, block.place, &motion)?;
            }
            if sections.statistics.is_empty() {
                continue;
            }
            let lowest = 32 - sections.active_clients_mask.leading_zeros() as usize;
            let decoded = shape
                .and_then(|(players, batteries)| {
                    Statistics::decode(sections.statistics, players, batteries)
                })
                .or_else(|| {
                    (lowest.max(1)..=MAX_PLAYERS)
                        .flat_map(|players| (0..=64).map(move |batteries| (players, batteries)))
                        .find_map(|(players, batteries)| {
                            let decoded =
                                Statistics::decode(sections.statistics, players, batteries)?;
                            shape = Some((players, batteries));
                            Some(decoded)
                        })
                });
            let Some(decoded) = decoded else {
                continue;
            };
            for (place, player) in decoded.players.iter().enumerate() {
                write!(
                    statistics,
                    "{id},{},{place},{}",
                    world.time, player.killstreak
                )?;
                for (_, number) in EVENTS {
                    write!(statistics, ",{}", player.total(number))?;
                }
                writeln!(statistics, ",{}", snapshot::score(player, &SCORE_WEIGHTS))?;
            }
            for (index, battery) in decoded.victory_items.iter().enumerate() {
                // Each carrier's distance over the kits: `place:metres` with `;`.
                let carried: Vec<String> = (0..MAX_PLAYERS)
                    .filter_map(|place| {
                        let metres: f32 =
                            battery.carried_distances.iter().map(|kit| kit[place]).sum();
                        (metres > 0.0).then(|| format!("{place}:{metres:.2}"))
                    })
                    .collect();
                writeln!(
                    batteries,
                    "{id},{},{index},{},{},{:.2},{:.2},{}",
                    world.time,
                    battery.owner_team,
                    battery.first_finder,
                    battery.nearest_positions[0],
                    battery.nearest_positions[1],
                    carried.join(";")
                )?;
            }
            for (receiver, player) in decoded.players.iter().enumerate() {
                for inflictor in 0..MAX_PLAYERS {
                    if player
                        .received_damage
                        .iter()
                        .any(|kit| kit[inflictor] > 0.0)
                    {
                        *pairs.entry((inflictor, receiver)).or_default() += 1;
                    }
                    if inflictor != receiver && player.last_hit_ms[inflictor] > 0 {
                        // This player (receiver here) hit `inflictor`.
                        *pairs.entry((receiver, inflictor)).or_default() += 1;
                    }
                }
            }
        }
        for ((inflictor, receiver), count) in pairs {
            writeln!(hits, "{id},{inflictor},{receiver},{count}")?;
        }
        if let Some((places, batteries)) = shape {
            writeln!(shapes, "{id},{places},{batteries}")?;
        }
    }
    hits.flush()?;
    shapes.flush()?;
    positions.flush()?;
    statistics.flush()?;
    batteries.flush()?;
    eprintln!(
        "{} matches, {worlds} worlds ({unread} not walked); player blocks: {players_read} read, \
         {players_unread} not",
        matches.len()
    );
    Ok(())
}

fn write_position(
    out: &mut impl Write,
    id: &str,
    time: u32,
    place: u8,
    motion: &Motion,
) -> std::io::Result<()> {
    write!(out, "{id},{time},{place},{}", u8::from(motion.alive))?;
    match &motion.body {
        Some(body) => {
            let [x, y, z] = body.position;
            write!(out, ",{x:.2},{y:.2},{z:.2}")?;
        }
        None => write!(out, ",,,")?,
    }
    write!(
        out,
        ",{:.3},{:.3},{:#x}",
        motion.rotation_y, motion.look_pitch, motion.actions_mask
    )?;
    match &motion.body {
        Some(body) => write!(
            out,
            ",{},{},{}",
            u8::from(body.capsule_crouch),
            u8::from(body.sprinting),
            u8::from(body.jumping)
        )?,
        None => write!(out, ",,,")?,
    }
    match &motion.damage {
        Some(damage) => write!(
            out,
            ",{:.2},{:.3}",
            damage.linear_horizontal_speed,
            damage.part_health.first().copied().unwrap_or(f32::NAN)
        )?,
        None => write!(out, ",,")?,
    }
    writeln!(
        out,
        ",{},{}",
        motion.recent_hits.len(),
        motion.inserted_time_ms
    )
}

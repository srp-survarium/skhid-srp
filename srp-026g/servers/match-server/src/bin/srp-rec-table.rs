//! Tables of a match recording (local analysis tool, A187 reader validation; not for a PR):
//!
//! ```text
//! srp-rec-table <file.srprec> <out dir> [--revision <n>]
//! ```
//!
//! The states are read in the layout of the client patch revision that wrote them (`players::state_at_revision`, A305 F7). A
//! world's layout is its uploader's, not each block's place's (a client-test recording's one human uploads every bot's block),
//! and the tables do not say who uploaded: each block is read at each revision the recording's `revision` notes name (oldest
//! first), then `--revision`, then as it is (today's layout), and the first that reads is taken; a wrong layout does not read
//! (`PlayerState::read` refuses its pickup record). `rec_head.txt` counts the blocks read at each.
//!
//! Writes, read the way the replay converter reads them (`sur_convert::tables_of_file`):
//!
//! - `rec_inputs.csv`: `match,time,place,actions,rot_a,rot_b`: every input relayed (0xEB/0xEC/0xED/0xEE) or accepted, at its
//!   match time; `rot_a`, `rot_b` the two halves of the relayed `half2 rotation`, as floats;
//! - `rec_worlds.csv`: `match,time,upload,uploads,same,place,alive,cx,cy,cz,fx,fy,fz,yaw,pitch,keys,speed,inserted`: every
//!   player block of every world a client uploaded (upload `k` of `uploads` at that time, `same` 1 when all the uploads of
//!   the time are byte-equal), the body's centre (`m_last_pos`) and feet in the map's frame;
//! - `rec_motion.csv` (A304): `match,time,upload,place,alive,sprinting,jumping,colliding,can_stand,normal_y,fall_y,last_hit,hits,
//!   last_recent_hit,stagger,health0`: the same blocks' controller flags (`m_is_sprinting` and the others), the support normal's
//!   y and the fall velocity's y, the damage model's last hit time, the recent hits' count and latest time, the stagger float
//!   before the hits' count, and part 0's health; empty where the block has none;
//! - `rec_head.txt`: the header's map, mode and place count, the note kinds, the report's counts. No account is written.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use match_server::v026g::record::{self, NOTE};
use match_server::v026g::sur_convert;
use survarium::v026g::match_msgs::motion::Motion;
use survarium::v026g::match_msgs::players;

fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from((bits >> 10) & 0x1F);
    let frac = f32::from(bits & 0x3FF);
    match exp {
        0 => sign * frac * 2f32.powi(-24),
        31 => f32::NAN,
        _ => sign * (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (input, out, fallback) = match args.as_slice() {
        [input, out] => (input, out, None),
        [input, out, flag, n] if flag == "--revision" => match n.parse::<u16>() {
            Ok(n) => (input, out, Some(n)),
            Err(_) => {
                eprintln!("--revision takes a number");
                return ExitCode::from(2);
            }
        },
        _ => {
            eprintln!("usage: srp-rec-table <file.srprec> <out dir> [--revision <n>]");
            return ExitCode::from(2);
        }
    };
    match run(Path::new(input), Path::new(out), fallback) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{input}: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Client patch 78's carry snapshot at the end of a state (`artefact_carry::carry::Carry::SNAPSHOT_SIZE`): every state has one
/// once read at its revision.
const CARRY_SNAPSHOT: usize = 32;

fn run(input: &Path, out: &Path, fallback: Option<u16>) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(out)?;
    // A314 (fx-review, local): with SRP_GAME_DATA set, the server's own game data (its dictionary has the patches' items).
    if std::env::var_os(survarium::v026g::game_data::GAME_DATA_ENV).is_some()
        && let Err(e) = survarium::v026g::game_data::init()
    {
        eprintln!("a314 game data: {e}");
    }
    let notes = record::read_notes(input)?;
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut head = String::new();
    // A332: the mode and the place count, for the statistics' battery records.
    let (mut mode_num, mut places_n) = (0u8, 0usize);
    // Each place's client patch revision, from its `revision` note (game.rs).
    let mut revisions: BTreeMap<u8, u16> = BTreeMap::new();
    let mut notes_out: Vec<String> = Vec::new();
    for (k, note) in notes.iter().filter(|r| r.kind == NOTE).enumerate() {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&note.bytes) else {
            *kinds.entry("unparsed".into()).or_default() += 1;
            continue;
        };
        if k == 0 {
            mode_num = value["mode"].as_u64().unwrap_or(0) as u8;
            places_n = value["places"].as_array().map_or(0, Vec::len);
            head = format!(
                "header: match {} map {} mode {} places {}",
                value["match_id"],
                value["map"],
                value["mode"],
                value["places"].as_array().map_or(0, Vec::len)
            );
        }
        let kind = value
            .as_object()
            .and_then(|o| o.get("note").or_else(|| o.get("kind")).or_else(|| o.get("event")))
            .and_then(|v| v.as_str())
            .map_or_else(|| value.as_object().map_or("?".into(), |o| o.keys().cloned().collect::<Vec<_>>().join("+")), str::to_owned);
        if kind == "revision"
            && let (Some(place), Some(revision)) = (value["place"].as_u64(), value["revision"].as_u64())
        {
            revisions.insert(place as u8, revision as u16);
        }
        // A314 (fx-review, local): every note but the header (which names the places) to rec_notes.jsonl, one JSON a line.
        if k != 0 {
            notes_out.push(value.to_string());
        }
        *kinds.entry(kind).or_default() += 1;
    }
    std::fs::write(out.join("rec_notes.jsonl"), notes_out.join("\n") + "\n")?;
    let tables = sur_convert::tables_of_file(input, &notes)?;
    let id = tables.report.match_id.trim_start_matches("0x").to_owned();
    let mut inputs = std::io::BufWriter::new(std::fs::File::create(out.join("rec_inputs.csv"))?);
    writeln!(inputs, "match,time,place,actions,rot_a,rot_b")?;
    for (&(time, place), value) in &tables.inputs {
        let r = value.rotation;
        writeln!(
            inputs,
            "{id},{time},{place},{:#x},{:.4},{:.4}",
            value.actions,
            half(u16::from_le_bytes([r[0], r[1]])),
            half(u16::from_le_bytes([r[2], r[3]]))
        )?;
    }
    inputs.flush()?;
    // A314 (fx-review, local): every roster place's quick-use keys, as the client gives them (the n-th filled slot of
    // `QUICK_USE_ORDER`, keys 1..6), with the item in each: `rec_kits.csv`: `match,roster,place,team,kit,quick,slot,item,count`.
    let mut kits = std::io::BufWriter::new(std::fs::File::create(out.join("rec_kits.csv"))?);
    writeln!(kits, "match,roster,place,team,kit,quick,slot,item,count")?;
    let dictionary = survarium::v026g::dictionary::Dictionary::shared();
    for (&roster, (_, options)) in &tables.rosters {
        eprintln!(
            "a314 roster {roster}: {} players, kits {:?}",
            options.players.len(),
            options.players.iter().map(|p| (p.kits.len(), p.items.len())).collect::<Vec<_>>()
        );
        for (place, profile) in options.players.iter().enumerate() {
            for (k, kit) in profile.kits.iter().enumerate() {
                let filled = survarium::v026g::loadouts::QUICK_USE_ORDER
                    .iter()
                    .copied()
                    .filter(|&slot| kit.slots[usize::from(slot)].is_some_and(|(_, count)| count > 0));
                for (quick, slot) in filled.enumerate().take(6) {
                    let Some((index, count)) = kit.slots[usize::from(slot)] else { continue };
                    let name = profile
                        .items
                        .get(usize::from(index))
                        .and_then(|item| dictionary.item(item.dict_id))
                        .map_or("?".to_string(), |row| row.name.to_string());
                    writeln!(kits, "{id},{roster},{place},{},{k},{},{slot},{name},{count}", profile.team, quick + 1)?;
                }
            }
        }
    }
    kits.flush()?;
    // A332 (fx-review, local): each roster place's kits' speed level as the server reckons it
    // (`bot_skills::profile_movement_level_in`, no weapon term), the kit's weight, and every weapon slot's own
    // `movement_speed_modifier`: `rec_levels.csv`: `match,roster,place,team,kit,level_none,weight,weapons` (weapons:
    // `slot:modifier` joined by `;`).
    let mut levels = std::io::BufWriter::new(std::fs::File::create(out.join("rec_levels.csv"))?);
    writeln!(levels, "match,roster,place,team,kit,level_none,weight,weapons")?;
    for (&roster, (_, options)) in &tables.rosters {
        for (place, profile) in options.players.iter().enumerate() {
            for (k, kit) in profile.kits.iter().enumerate() {
                let Some(level) = survarium::v026g::bot_skills::profile_movement_level_in(profile, k, false) else { continue };
                let mut weight = 0.0f32;
                let mut weapons: Vec<String> = Vec::new();
                for (slot, entry) in kit.slots.iter().enumerate() {
                    let Some((index, count)) = entry else { continue };
                    let Some(item) = profile.items.get(usize::from(*index)) else { continue };
                    weight += survarium::v026g::bot_skills::item_weight(item.dict_id, u32::from(*count), &item.modifications);
                    if let Some(w) = survarium::v026g::weapons::weapon(item.dict_id) {
                        weapons.push(format!("{slot}:{:.3}", w.movement_speed_modifier));
                    }
                }
                writeln!(levels, "{id},{roster},{place},{},{k},{level:.4},{weight:.3},{}", profile.team, weapons.join(";"))?;
            }
        }
    }
    levels.flush()?;
    // A332: who carries a battery: a player whose `carried_distances[kit][player]` grew since the last world read
    // (statistics 0x334DC0, modes 2 and 3): `rec_carry.csv`: `match,time,battery,place,kit,distance`.
    let mut carry = std::io::BufWriter::new(std::fs::File::create(out.join("rec_carry.csv"))?);
    writeln!(carry, "match,time,battery,place,kit,distance")?;
    let mut carried_last: BTreeMap<(usize, usize, usize), f32> = BTreeMap::new();
    let (mut decoded_n, mut stats_n, mut items_n) = (0usize, 0usize, 0usize);
    let players_n = tables.rosters.values().map(|(_, o)| o.players.len()).max().unwrap_or(places_n);
    for (&time, bodies) in &tables.worlds {
        let Some(body) = bodies.first() else { continue };
        // The live server passes its own player count and the map's battery count (game.rs); here: the first that reads.
        let tries = [places_n, players_n, 20].into_iter().flat_map(|p| (0..=5usize).rev().map(move |b| (p, b)));
        let found = tries
            .filter_map(|(p, b)| survarium::v026g::match_msgs::snapshot::decode(time, body, p, mode_num, b))
            .find(|d| d.statistics.as_ref().is_some_and(|s| !s.victory_items.is_empty()));
        let Some(decoded) = found else { continue };
        decoded_n += 1;
        let Some(statistics) = decoded.statistics else { continue };
        stats_n += 1;
        items_n = items_n.max(statistics.victory_items.len());
        for (b, item) in statistics.victory_items.iter().enumerate() {
            for (kit, row) in item.carried_distances.iter().enumerate() {
                for (place, &d) in row.iter().enumerate() {
                    let before = carried_last.insert((b, kit, place), d).unwrap_or(0.0);
                    if d > before + 1e-3 {
                        writeln!(carry, "{id},{time},{b},{place},{kit},{d:.2}")?;
                    }
                }
            }
        }
    }
    carry.flush()?;
    eprintln!("a332 carry: mode {mode_num} places {places_n} players {players_n}: worlds decoded {decoded_n}, with statistics {stats_n}, victory items {items_n}");
    let mut worlds = std::io::BufWriter::new(std::fs::File::create(out.join("rec_worlds.csv"))?);
    writeln!(worlds, "match,time,upload,uploads,same,place,alive,cx,cy,cz,fx,fy,fz,yaw,pitch,keys,speed,inserted")?;
    let mut motions = std::io::BufWriter::new(std::fs::File::create(out.join("rec_motion.csv"))?);
    writeln!(motions, "match,time,upload,place,alive,sprinting,jumping,colliding,can_stand,normal_y,fall_y,last_hit,hits,last_recent_hit,stagger,health0,kit,head,legs,hands,pain,infection")?;
    let (mut read, mut unread) = (0usize, 0usize);
    // The layouts to try, oldest revision first, then the fallback, then today's.
    let mut candidates: Vec<Option<u16>> = revisions.values().copied().collect::<std::collections::BTreeSet<_>>().into_iter().map(Some).collect();
    if let Some(n) = fallback
        && !candidates.contains(&Some(n))
    {
        candidates.push(Some(n));
    }
    candidates.push(None);
    let mut read_at: BTreeMap<Option<u16>, usize> = BTreeMap::new();
    // A314 (fx-review, local): each human place's medicines in hand (`players::Supplies`, as the bots read a live block):
    // `rec_supplies.csv`: `match,time,place,kit,item,in_hand,stored`, upload 0 only. The profile is the largest roster's.
    let mut supplies = std::io::BufWriter::new(std::fs::File::create(out.join("rec_supplies.csv"))?);
    writeln!(supplies, "match,time,place,kit,item,in_hand,stored")?;
    let mut profiles: BTreeMap<u8, &survarium::v026g::match_msgs::static_info::MatchProfile> = BTreeMap::new();
    for (_, options) in tables.rosters.values() {
        for (place, profile) in options.players.iter().enumerate() {
            profiles.insert(place as u8, profile);
        }
    }
    const MEDS: [&str; 7] =
        ["medkit", "science_medkit", "blackmarket_medkit", "military_medkit", "bandages", "antitoxin", "painkiller"];
    for (&time, bodies) in &tables.worlds {
        let same = u8::from(bodies.windows(2).all(|w| w[0] == w[1]));
        for (k, body) in bodies.iter().enumerate() {
            for block in players::walk(body).map(|(blocks, _)| blocks).unwrap_or_default() {
                let found = candidates.iter().find_map(|&revision| {
                    let state = match revision {
                        Some(revision) => players::state_at_revision(block.state, revision)?,
                        None => std::borrow::Cow::Borrowed(block.state),
                    };
                    let (_, motion) = Motion::find(&state, time)?;
                    Some((revision, state, motion))
                });
                let Some((revision, state, motion)) = found else {
                    unread += 1;
                    continue;
                };
                *read_at.entry(revision).or_default() += 1;
                let state = &*state;
                read += 1;
                if k == 0
                    && revisions.contains_key(&block.place)
                    && let Some(profile) = profiles.get(&block.place)
                {
                    let read_block = players::PlayerBlock { place: block.place, state, animation: block.animation };
                    if let Some(s) = players::Supplies::read(&read_block, profile, dictionary) {
                        for item in &s.items {
                            let Some(row) = dictionary.item(item.dict_id) else { continue };
                            if !item.stack || !MEDS.contains(&row.name.as_str()) {
                                continue;
                            }
                            if let Some(n) = item.in_hand {
                                writeln!(supplies, "{id},{time},{},{},{},{n},{}", block.place, s.kit, row.name, item.stored)?;
                            }
                        }
                    }
                }
                let (c, f) = motion.body.as_ref().map_or(([f32::NAN; 3], [f32::NAN; 3]), |b| (b.position, b.feet()));
                let speed = motion.damage.as_ref().map_or(f32::NAN, |d| d.linear_horizontal_speed);
                // The stagger float: the 4 bytes before the recent hits' count, which ends the state but for the last bool.
                let native = state.len().saturating_sub(CARRY_SNAPSHOT);
                let count_at = native.checked_sub(5 + 8 * motion.recent_hits.len());
                let stagger = count_at
                    .and_then(|at| at.checked_sub(4))
                    .and_then(|at| state.get(at..at + 4))
                    .map_or(f32::NAN, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                let flag = |b: bool| u8::from(b).to_string();
                let (sprinting, jumping, colliding, can_stand, normal_y, fall_y) = motion.body.as_ref().map_or_else(
                    || Default::default(),
                    |b| {
                        (
                            flag(b.sprinting),
                            flag(b.jumping),
                            flag(b.colliding_with_player),
                            flag(b.can_stand),
                            format!("{:.3}", b.supporting_surface_normal[1]),
                            format!("{:.3}", b.fall_and_slide_velocity[1]),
                        )
                    },
                );
                let (last_hit, health0) = motion.damage.as_ref().map_or((String::new(), String::new()), |d| {
                    (d.last_hit_time_ms.to_string(), d.part_health.first().map_or(String::new(), |h| format!("{h:.3}")))
                });
                let last_recent = motion.recent_hits.last().map_or(String::new(), |h| h.0.to_string());
                // A314: the other pools (bots/pools.rs: head 3, legs 6-7, hands 16-17, pain 18), the lower limb of each pair.
                let pools = motion.damage.as_ref().filter(|d| d.part_health.len() >= 20).map_or(",,,,".to_string(), |d| {
                    let p = &d.part_health;
                    format!("{:.3},{:.3},{:.3},{:.3},{:.3}", p[3], p[6].min(p[7]), p[16].min(p[17]), p[18], p[19])
                });
                writeln!(
                    motions,
                    "{id},{time},{k},{},{},{sprinting},{jumping},{colliding},{can_stand},{normal_y},{fall_y},{last_hit},{},{last_recent},{stagger:.4},{health0},{},{pools}",
                    block.place,
                    u8::from(motion.alive),
                    motion.recent_hits.len(),
                    state.first().copied().unwrap_or(0)
                )?;
                writeln!(
                    worlds,
                    "{id},{time},{k},{},{same},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.4},{:.4},{:#x},{speed:.3},{}",
                    bodies.len(),
                    block.place,
                    u8::from(motion.alive),
                    c[0],
                    c[1],
                    c[2],
                    f[0],
                    f[1],
                    f[2],
                    motion.rotation_y,
                    motion.look_pitch,
                    motion.actions_mask,
                    motion.inserted_time_ms
                )?;
            }
        }
    }
    worlds.flush()?;
    motions.flush()?;
    supplies.flush()?;
    let r = &tables.report;
    let text = format!(
        "{head}\nnote kinds: {kinds:?}\nclock offset ms: {}\ndatagrams in {} out {} unparsed {} {:?}\nworlds complete {} world times {}\ninputs {} copies {} conflicts {} flagged {} dropped_future {}\nplayer blocks read {read} unread {unread}\nrevisions by place {revisions:?}, else {fallback:?}; blocks read at (None = as they are) {read_at:?}\n",
        r.clock_offset_ms,
        r.datagrams_in,
        r.datagrams_out,
        r.unparsed,
        r.unparsed_why,
        r.worlds_complete,
        r.world_times,
        r.inputs,
        r.input_copies,
        r.input_conflicts,
        r.flagged_inputs,
        r.dropped_future
    );
    std::fs::write(out.join("rec_head.txt"), text)?;
    Ok(())
}

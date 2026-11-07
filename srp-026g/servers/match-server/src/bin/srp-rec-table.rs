//! Tables of a match recording, for the bots' drift lab (`bots/tests/drift_replay.rs`) and its nav gate (a bench tool; the server never
//! runs it):
//!
//! ```text
//! SRP_GAME_DATA=<server game data db> srp-rec-table <file.srprec> <out dir>
//! ```
//!
//! A block is read in the layout of the client patch revision that wrote it (`players::state_at_revision`): each revision the
//! recording's `revision` notes name, oldest first, then today's; `rec_head.txt` counts the blocks read at each.
//!
//! Writes, read the way the replay converter reads them (`sur_convert::tables_of_file`):
//!
//! - `rec_inputs.csv`: `match,time,place,actions,rot_a,rot_b`: every input relayed (0xEB/0xEC/0xED/0xEE) or accepted, at its
//!   match time; `rot_a`, `rot_b` the two halves of the relayed `half2 rotation`, as floats;
//! - `rec_worlds.csv`: `match,time,upload,uploads,same,place,alive,cx,cy,cz,fx,fy,fz,yaw,pitch,keys,speed,inserted,kit,hands,
//!   carrying,speeds`: every player block of every world a client uploaded (upload `k` of `uploads` at that time, `same` 1 when
//!   all the uploads of the time are byte-equal), the body's centre (`m_last_pos`) and feet in the map's frame; [A334] the block's
//!   `Supplies` (kit in use, `hands_slot`, `carrying`; empty where unread) and the speed factors the server hands the reckoning
//!   for them (`speeds`, `;`-joined: `KitSpeeds::in_hands` with the hands known, else `KitSpeeds::candidates`);
//! - `rec_motion.csv` (A304): `match,time,upload,place,alive,sprinting,jumping,colliding,can_stand,normal_y,fall_y,last_hit,hits,
//!   last_recent_hit,stagger,health0`: the same blocks' controller flags (`m_is_sprinting` and the others), the support normal's
//!   y and the fall velocity's y, the damage model's last hit time, the recent hits' count and latest time, the stagger float
//!   before the hits' count, and part 0's health; [A325e] then `leg6,leg7,leg_damage_until,hand_damage_until,parts_speed`: the
//!   legs' health, the latest end time of a leg_damage affect (type 4) on parts 6/7 and of a hand_damage (type 3) on parts 16/17,
//!   and the sum of the 20 parts' `movement_speed_modifier` (`motion::BodyPart`); then `leg6_damage_until,leg7_damage_until`, each
//!   leg's own latest leg_damage end (empty: none); [A325] then the controller block's rest: `normal_x,normal_z` (the contact
//!   normal's other two), `fall_x,fall_z`, `ceil_x,ceil_y,ceil_z` (the +42 vector), `slide_x,slide_y,slide_z` (+54), the crouch
//!   flags `input_crouch,logic_crouch,capsule_crouch`; empty where the block has none;
//! - `rec_anim.csv` (A325): `match,time,upload,place,times,floats`: each block's animation state's times and floats tables
//!   (`players::animation_tables`, `;`-joined, in the state's order: the animation controller's buffer, 0x395AF0);
//! - `rec_levels.csv` (A332): `match,roster,place,team,kit,level_none,weight,weapons`: each roster place's kits' speed level as the
//!   server reckons it (`bot_skills::profile_movement_level_in`, no weapon term), the kit's weight (kg, no loaded rounds) and every
//!   weapon slot's `movement_speed_modifier` (`slot:modifier` joined by `;`); a life's level is `level_none` + its held weapon's
//!   modifier. Needs `SRP_GAME_DATA` (the patches' items);
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
    let [input, out] = args.as_slice() else {
        eprintln!("usage: srp-rec-table <file.srprec> <out dir>");
        return ExitCode::from(2);
    };
    // The players' dictionary of the recording's revision (`SRP_GAME_DATA`), as `srp-recording-to-sur` loads it: without it
    // `Dictionary::shared()` is the test archive's, and the 0xE4 of a match whose kits hold items newer than it (every live match
    // since the ported weapons) does not parse, so the rosters, and `rec_levels.csv`, come out empty.
    // `SRP_REC_TABLE_OLD_DATA=1`: a recording made on older data (the client tests' revision 34) is read with that data although
    // the server would refuse to serve it (its `golden` and `tables` checks run after `init` has set the data): only a file that
    // does not open stops the tool then; the refusal is printed.
    let old_data = std::env::var_os("SRP_REC_TABLE_OLD_DATA").is_some_and(|v| v == "1");
    if old_data && let Err(error) = survarium::v026g::game_data::from_env() {
        eprintln!("{error}");
        return ExitCode::from(2);
    }
    if let Err(error) = survarium::v026g::game_data::init() {
        if !old_data {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
        eprintln!("old data kept (SRP_REC_TABLE_OLD_DATA=1), the server would refuse it: {error}");
    }
    match run(Path::new(input), Path::new(out)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{input}: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(input: &Path, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(out)?;
    let notes = record::read_notes(input)?;
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut head = String::new();
    // Each place's client patch revision, from its `revision` note (`game.rs`), for the layout its blocks are read in.
    let mut revisions: BTreeMap<u8, u16> = BTreeMap::new();
    for (k, note) in notes.iter().filter(|r| r.kind == NOTE).enumerate() {
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&note.bytes) else {
            *kinds.entry("unparsed".into()).or_default() += 1;
            continue;
        };
        if k == 0 {
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
            && let (Ok(place), Ok(revision)) = (u8::try_from(place), u16::try_from(revision))
        {
            revisions.insert(place, revision);
        }
        *kinds.entry(kind).or_default() += 1;
    }
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
    // A332 (fx-review's probe bb5503dc, lifted): each roster place's kits' speed level as the server reckons it
    // (`bot_skills::profile_movement_level_in`, no weapon term), the kit's weight, and every weapon slot's own
    // `movement_speed_modifier`: `rec_levels.csv`: `match,roster,place,team,kit,level_none,weight,weapons` (weapons:
    // `slot:modifier` joined by `;`). Needs SRP_GAME_DATA (the patches' items).
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
    // [A334] Each place's profile (the largest roster that has it), for its blocks' `Supplies`: the kit in use, the weapon in hands
    // (`Supplies::hands_slot`, `m_active_slot`; PROVEN, fx-review A334c) and the speed factors the server hands the reckoning for
    // them (`game/driven.rs::observe_stock`: the in-hands F when the hands are known, else every weapon's).
    let profiles: BTreeMap<usize, &survarium::v026g::match_msgs::static_info::MatchProfile> =
        tables.rosters.values().flat_map(|(_, options)| options.players.iter().enumerate()).collect();
    let dictionary = survarium::v026g::dictionary::Dictionary::shared();
    let mut worlds = std::io::BufWriter::new(std::fs::File::create(out.join("rec_worlds.csv"))?);
    writeln!(worlds, "match,time,upload,uploads,same,place,alive,cx,cy,cz,fx,fy,fz,yaw,pitch,keys,speed,inserted,kit,hands,carrying,speeds")?;
    let mut motions = std::io::BufWriter::new(std::fs::File::create(out.join("rec_motion.csv"))?);
    writeln!(
        motions,
        "match,time,upload,place,alive,sprinting,jumping,colliding,can_stand,normal_y,fall_y,last_hit,hits,last_recent_hit,stagger,health0,\
         leg6,leg7,leg_damage_until,hand_damage_until,parts_speed,leg6_damage_until,leg7_damage_until,\
         normal_x,normal_z,fall_x,fall_z,ceil_x,ceil_y,ceil_z,slide_x,slide_y,slide_z,input_crouch,logic_crouch,capsule_crouch"
    )?;
    // [A325] Each block's animation state's two tables (`players::animation_tables`), for the hit selector's per-part values.
    let mut anims = std::io::BufWriter::new(std::fs::File::create(out.join("rec_anim.csv"))?);
    writeln!(anims, "match,time,upload,place,times,floats")?;
    let dump: Option<(u8, u32, u32)> = std::env::var("SRP_REC_TABLE_DUMP").ok().and_then(|v| {
        let mut parts = v.split(',').map(str::trim);
        Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
    });
    let mut dumps = std::io::BufWriter::new(std::fs::File::create(out.join("rec_dump.csv"))?);
    writeln!(dumps, "time,place,state")?;
    let (mut read, mut unread) = (0usize, 0usize);
    // The layouts a block is read in (`players::state_at_revision`, A305 F7, fx-review's bb5503dc): a world's layout is its
    // uploader's, not its block's place's (a client-test recording's one human uploads every bot's block), and the tables do not
    // say who uploaded, so each revision the `revision` notes name, oldest first, then as it is (today's); the first that reads is
    // taken (a wrong layout does not read: `PlayerState::read` refuses its pickup record).
    let layouts: Vec<Option<u16>> =
        revisions.values().copied().collect::<std::collections::BTreeSet<_>>().into_iter().map(Some).chain(std::iter::once(None)).collect();
    let mut read_at: BTreeMap<Option<u16>, usize> = BTreeMap::new();
    for (&time, bodies) in &tables.worlds {
        let same = u8::from(bodies.windows(2).all(|w| w[0] == w[1]));
        for (k, body) in bodies.iter().enumerate() {
            for raw in players::walk(body).map(|(blocks, _)| blocks).unwrap_or_default() {
                let found = layouts.iter().find_map(|&revision| {
                    let state = match revision {
                        Some(revision) => players::state_at_revision(raw.state, revision)?,
                        None => std::borrow::Cow::Borrowed(raw.state),
                    };
                    Motion::find(&state, time).map(|(_, motion)| (revision, state, motion))
                });
                let Some((revision, state, motion)) = found else {
                    unread += 1;
                    continue;
                };
                *read_at.entry(revision).or_default() += 1;
                let block = players::PlayerBlock { place: raw.place, state: &state, animation: raw.animation };
                read += 1;
                // `SRP_REC_TABLE_DUMP=<place>,<from>,<to>` (a probe): that place's state bytes, as read, in hex.
                if let Some((p, from, to)) = dump
                    && k == 0
                    && raw.place == p
                    && (from..=to).contains(&time)
                {
                    let hex: String = state.iter().map(|b| format!("{b:02x}")).collect();
                    writeln!(dumps, "{time},{},{hex}", raw.place)?;
                }
                if let Some(tables) = raw.animation.and_then(players::animation_tables) {
                    let join = |v: Vec<String>| v.join(";");
                    writeln!(
                        anims,
                        "{id},{time},{k},{},{},{}",
                        raw.place,
                        join(tables.times.iter().map(u32::to_string).collect()),
                        join(tables.floats.iter().map(|f| format!("{f:.4}")).collect())
                    )?;
                }
                let (c, f) = motion.body.as_ref().map_or(([f32::NAN; 3], [f32::NAN; 3]), |b| (b.position, b.feet()));
                let speed = motion.damage.as_ref().map_or(f32::NAN, |d| d.linear_horizontal_speed);
                // The stagger float: the 4 bytes before the recent hits' count, which ends the state but for the last bool.
                let count_at = block.state.len().checked_sub(5 + 8 * motion.recent_hits.len());
                let stagger = count_at
                    .and_then(|at| at.checked_sub(4))
                    .and_then(|at| block.state.get(at..at + 4))
                    .map_or(f32::NAN, |b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                let flag = |b: bool| u8::from(b).to_string();
                let (sprinting, jumping, colliding, can_stand, normal_y, fall_y) = motion.body.as_ref().map_or_else(
                    Default::default,
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
                // [A325e] The legs' health (parts 6, 7), the latest end of a leg_damage (type 4) on them and of a hand_damage (type 3)
                // on the hands (16, 17), and the sum of the 20 parts' own movement_speed_modifier.
                // [A325] The rest of the controller block (`bullet_character_controller::serialize` 0x7F7F0): the contact normal's x and
                // z, the fall velocity's x and z, the ceiling normal and slide vector (LIKELY names), the crouch flags.
                let controller_cols = motion.body.as_ref().map_or(",,,,,,,,,,,,".to_string(), |b| {
                    let [n, f, c, l] = [b.supporting_surface_normal, b.fall_and_slide_velocity, b.ceiling_surface_normal, b.slide_velocity];
                    format!(
                        "{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{}",
                        n[0], n[2], f[0], f[2], c[0], c[1], c[2], l[0], l[1], l[2],
                        u8::from(b.input_crouch), u8::from(b.logic_crouch), u8::from(b.capsule_crouch)
                    )
                });
                let parts_cols = motion.damage.as_ref().filter(|d| d.parts.len() >= 18).map_or(",,,,,,".to_string(), |d| {
                    let until = |ids: &[usize], kind: u8| {
                        ids.iter()
                            .flat_map(|&p| d.parts[p].affects.iter().filter(|a| a.0 == kind).map(|a| a.1))
                            .max()
                            .map_or(String::new(), |t| t.to_string())
                    };
                    format!(
                        "{:.3},{:.3},{},{},{:.3},{},{}",
                        d.parts[6].health,
                        d.parts[7].health,
                        until(&[6, 7], 4),
                        until(&[16, 17], 3),
                        d.parts.iter().map(|p| p.movement_speed_modifier).sum::<f32>(),
                        until(&[6], 4),
                        until(&[7], 4)
                    )
                });
                writeln!(
                    motions,
                    "{id},{time},{k},{},{},{sprinting},{jumping},{colliding},{can_stand},{normal_y},{fall_y},{last_hit},{},{last_recent},{stagger:.4},{health0},{parts_cols},{controller_cols}",
                    block.place,
                    u8::from(motion.alive),
                    motion.recent_hits.len()
                )?;
                let supplies = profiles
                    .get(&usize::from(block.place))
                    .and_then(|profile| survarium::v026g::match_msgs::players::Supplies::read(&block, profile, dictionary).map(|s| (profile, s)));
                let (kit, hands, carrying, speeds) = supplies.map_or_else(Default::default, |(profile, s)| {
                    let speeds = survarium::v026g::bot_skills::profile_kit_speeds(profile, usize::from(s.kit)).map_or(String::new(), |speeds| {
                        let values = match s.hands_slot {
                            Some(slot) => vec![speeds.in_hands(Some(slot))],
                            None => speeds.candidates(),
                        };
                        values.iter().map(|v| format!("{v:.4}")).collect::<Vec<_>>().join(";")
                    });
                    (
                        s.kit.to_string(),
                        s.hands_slot.map_or(String::new(), |h| h.to_string()),
                        s.carrying.map_or(String::new(), |c| u8::from(c).to_string()),
                        speeds,
                    )
                });
                writeln!(
                    worlds,
                    "{id},{time},{k},{},{same},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.4},{:.4},{:#x},{speed:.3},{},{kit},{hands},{carrying},{speeds}",
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
    anims.flush()?;
    dumps.flush()?;
    let r = &tables.report;
    let text = format!(
        "{head}\nnote kinds: {kinds:?}\nclock offset ms: {}\ndatagrams in {} out {} unparsed {} {:?}\nworlds complete {} world times {}\ninputs {} copies {} conflicts {} flagged {} dropped_future {}\nplayer blocks read {read} unread {unread}\nrevisions by place {revisions:?}; blocks read at (None = as they are) {read_at:?}\n",
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

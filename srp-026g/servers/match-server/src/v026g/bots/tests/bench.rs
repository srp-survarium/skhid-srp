use super::*;

/// The real-map bench's events (`SRP_BENCH_EVENTS=<dir>`): the run's notes,
/// its kills (shooter, killed) and each bot's place and play once a world,
/// one JSON line each, in `<dir>/<level>-<mode>-<n>v<n>.jsonl`. What the
/// plays' churn, the idle bots with a goal and the trades are counted from
/// (`player_bots_behaviour.md` §3 "Play choice and trades").
pub(crate) struct BenchEvents(pub(crate) Option<std::io::BufWriter<std::fs::File>>);

impl BenchEvents {
    pub(crate) fn open(level: u8, mode: u8, per_team: usize) -> Self {
        Self(bench_settings().var_os("SRP_BENCH_EVENTS").map(|dir| {
            let path = std::path::Path::new(&dir)
                .join(format!("level_{level:02}-{mode}-{per_team}v{per_team}.jsonl"));
            std::io::BufWriter::new(std::fs::File::create(path).unwrap())
        }))
    }

    pub(crate) fn line(&mut self, value: serde_json::Value) {
        use std::io::Write;
        if let Some(file) = self.0.as_mut() {
            writeln!(file, "{value}").unwrap();
        }
    }

    pub(crate) fn note(&mut self, note: &Note) {
        if self.0.is_some() {
            self.line(serde_json::json!({"t": note.time, "bot": note.place, "note": note.text}));
        }
    }

    /// The kills since the last call, and each bot as it is now: [place,
    /// team, alive, x, z, yaw, play, aiming at someone, walking to a goal].
    pub(crate) fn world(&mut self, clock: u32, game: &mut Game, bots: &Bots) {
        let kills = std::mem::take(&mut game.kill_log);
        if self.0.is_none() {
            return;
        }
        for (tick, shooter, killed) in kills {
            self.line(serde_json::json!({"t": tick, "kill": [shooter, killed]}));
        }
        let each: Vec<serde_json::Value> = bots
            .bots
            .iter()
            .flatten()
            .map(|bot| {
                let body = &game.bodies[bot.place];
                let goal = matches!(bot.intent.go, Go::To { at, within_m } if body.feet.distance_xz(at) > within_m);
                serde_json::json!([
                    bot.place,
                    bot.team,
                    body.alive,
                    (body.feet.x * 10.0).round() / 10.0,
                    (body.feet.z * 10.0).round() / 10.0,
                    (body.yaw * 100.0).round() / 100.0,
                    bot.assignment.as_ref().map(|a| format!("{:?}", a.kind)),
                    bot.reflex.target().is_some(),
                    goal,
                ])
            })
            .collect();
        self.line(serde_json::json!({"t": clock, "w": each}));
    }
}

/// Where the time of the bots' polls goes, seconds.
#[derive(Default)]
pub(crate) struct Times {
    pub(crate) polls: u64,
    pub(crate) view: f64,
    pub(crate) commander: f64,
    pub(crate) think: f64,
    pub(crate) ticks: f64,
    /// Each poll's whole time, µs.
    pub(crate) each: Vec<f32>,
    /// The longest of each part in one poll, µs.
    pub(crate) worst: [f32; 4],
}

/// The default tuning, with `SRP_BENCH_SEARCH_FLOORS=<n>` (`max`: no bound)
/// as the floors a poll's searches may expand: the whole matches and the
/// real-map runs at other budgets.
/// The process's resident memory now and its peak since
/// [`reset_peak_memory`], in MB: on Linux only (`/proc/self/status`).
pub(crate) fn memory_mb() -> Option<(f64, f64)> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kb = |key: &str| {
        status
            .lines()
            .find(|line| line.starts_with(key))?
            .split_whitespace()
            .nth(1)?
            .parse::<f64>()
            .ok()
    };
    Some((kb("VmRSS:")? / 1024.0, kb("VmHWM:")? / 1024.0))
}

/// Starts the peak of [`memory_mb`] again from the memory now (Linux).
pub(crate) fn reset_peak_memory() {
    let _ = std::fs::write("/proc/self/clear_refs", "5");
}

pub(crate) fn bench_tuning() -> Tuning {
    bench_tuning_of(false)
}

/// `SRP_BENCH_PING=<lo>-<hi>` (or one number), ms: a ping for each of
/// `places`, drawn evenly between the two by `seed`.
pub(crate) fn bench_pings(places: usize, seed: u32) -> Option<Vec<u32>> {
    let text = bench_settings().var("SRP_BENCH_PING").ok().filter(|t| !t.is_empty())?;
    let (lo, hi) = text.split_once('-').unwrap_or((&text, &text));
    let (lo, hi): (f32, f32) = (
        lo.trim().parse().expect("SRP_BENCH_PING: ms"),
        hi.trim().parse().expect("SRP_BENCH_PING: ms"),
    );
    let mut rng = Rng::new(0x9146 + u64::from(seed));
    Some((0..places).map(|_| rng.range(lo, hi.max(lo)) as u32).collect())
}

/// [`bench_tuning`]; for `side1`, the plays', the reflex's and the senses'
/// numbers from `SRP_BENCH_SIDE1_<name>` instead of `SRP_BENCH_<name>`
/// (the one-sided runs, `SRP_BENCH_ONE_SIDE`: team 1 plays by it).
pub(crate) fn bench_tuning_of(side1: bool) -> Tuning {
    let var = |name: &str| {
        let prefix = if side1 { "SRP_BENCH_SIDE1_" } else { "SRP_BENCH_" };
        bench_settings().var(&format!("{prefix}{name}"))
    };
    let mut tuning = Tuning::default();
    // The battery cheat instead of the fair battery knowledge.
    if bench_settings().flag("SRP_BENCH_CHEAT") {
        tuning.cheats.batteries = true;
    }
    if let Some(m) = bench_settings().number("SRP_BENCH_BATTERY_SIGHT_M")
    {
        tuning.sense.battery_sight_m = m;
    }
    if let Some(m) = bench_settings().number("SRP_BENCH_FRIEND_NEAR_M")
    {
        tuning.aim.friends.near_m = m;
    }
    // The teammates led to the tick decided (`FriendlyFireTuning::lead`):
    // 0 off, 1 on; and the lead's extra, ms.
    if let Ok(v) = bench_settings().var("SRP_BENCH_FF_LEAD") {
        tuning.aim.friends.lead = v != "0";
    }
    // Teammates this far past the target count
    // (`FriendlyFireTuning::past_target_m`).
    if let Some(m) = bench_settings().number("SRP_BENCH_FF_PAST_M")
    {
        tuning.aim.friends.past_target_m = m;
    }
    // A teammate this near in front blocks whatever his angle
    // (`FriendlyFireTuning::touch_m`; 0 off).
    if let Some(m) = bench_settings().number("SRP_BENCH_FF_TOUCH_M")
    {
        tuning.aim.friends.touch_m = m;
    }
    if let Some(ms) = bench_settings().number("SRP_BENCH_FF_LEAD_MS")
    {
        tuning.aim.friends.lead_extra_ms = ms;
    }
    // Collect's and Intercept's numbers: `name=value,...`.
    for (name, kind) in [
        ("COLLECT", super::plays::PlayKind::Collect),
        ("INTERCEPT", super::plays::PlayKind::Intercept),
        ("RAID", super::plays::PlayKind::Raid),
        ("BUDDY", super::plays::PlayKind::Buddy),
        ("TRADE", super::plays::PlayKind::Trade),
        ("ROAM", super::plays::PlayKind::Roam),
        ("CUT", super::plays::PlayKind::Cut),
        ("HOLDEXIT", super::plays::PlayKind::HoldExit),
        ("CROSSFIRE", super::plays::PlayKind::Crossfire),
        ("PAIRUP", super::plays::PlayKind::PairUp),
    ] {
        let Ok(numbers) = var(name) else {
            continue;
        };
        let play = tuning
            .plays
            .entry(kind)
            .or_insert_with(|| super::plays::default_tuning(kind));
        for pair in numbers.split(',').filter(|p| !p.is_empty()) {
            let (name, value) = pair
                .split_once('=')
                .unwrap_or_else(|| panic!("SRP_BENCH_{name}: name=value"));
            play.numbers
                .insert(name.to_owned(), value.parse().expect("a number"));
        }
    }
    // The reflex's numbers (`CombatTuning`): `name=json,...` (`;` between
    // them when a value has a comma).
    if let Ok(numbers) = var("COMBAT") {
        let mut combat = serde_json::to_value(&tuning.aim.combat).unwrap();
        let sep = if numbers.contains(';') { ';' } else { ',' };
        for pair in numbers.split(sep).filter(|p| !p.is_empty()) {
            let (name, value) = pair.split_once('=').expect("SRP_BENCH_COMBAT: name=value");
            combat[name] = serde_json::from_str(value).expect("SRP_BENCH_COMBAT: a JSON value");
        }
        tuning.aim.combat = serde_json::from_value(combat).expect("SRP_BENCH_COMBAT: known names");
    }
    // The aim's own numbers (`AimTuning`: `range_m`, `fire_within_deg`,
    // `pause_ms`, ...), as `SRP_BENCH_COMBAT`'s.
    if let Ok(numbers) = var("AIM") {
        let mut aim = serde_json::to_value(&tuning.aim).unwrap();
        let sep = if numbers.contains(';') { ';' } else { ',' };
        for pair in numbers.split(sep).filter(|p| !p.is_empty()) {
            let (name, value) = pair.split_once('=').expect("SRP_BENCH_AIM: name=value");
            aim[name] = serde_json::from_str(value).expect("SRP_BENCH_AIM: a JSON value");
        }
        tuning.aim = serde_json::from_value(aim).expect("SRP_BENCH_AIM: known names");
    }
    // The senses' numbers (`SenseTuning`), as `SRP_BENCH_COMBAT`'s.
    if let Ok(numbers) = var("SENSE") {
        let mut sense = serde_json::to_value(&tuning.sense).unwrap();
        for pair in numbers.split(',').filter(|p| !p.is_empty()) {
            let (name, value) = pair.split_once('=').expect("SRP_BENCH_SENSE: name=value");
            sense[name] = serde_json::from_str(value).expect("SRP_BENCH_SENSE: a JSON value");
        }
        tuning.sense = serde_json::from_value(sense).expect("SRP_BENCH_SENSE: known names");
    }
    // How far under a walked leg a body is still on its route
    // (`Feel::off_walk_below_m`): `1.5` the old rule.
    if let Some(m) = bench_settings().number("SRP_BENCH_OFF_WALK_BELOW_M") {
        tuning.feel.off_walk_below_m = m;
    }
    // A way out of a zone stays under its start's strength
    // (`Tuning::escape_ceiling`): `0` off.
    if let Ok(v) = bench_settings().var("SRP_BENCH_ESCAPE_CEILING") {
        tuning.escape_ceiling = v != "0";
    }
    if let Ok(floors) = bench_settings().var("SRP_BENCH_SEARCH_FLOORS") {
        tuning.think.search_floors_per_poll = if floors == "max" {
            u32::MAX
        } else {
            floors
                .parse()
                .expect("SRP_BENCH_SEARCH_FLOORS: a number or max")
        };
    }
    tuning
}

/// [`Bots::poll`] with each part timed; the same steps in the same order.
pub(crate) fn timed_poll(bots: &mut Bots, clock: Clock, times: &mut Times) -> (Vec<Made>, Vec<Note>) {
    use std::time::Instant;
    let started = Instant::now();
    let (mut made, mut notes) = (Vec::new(), Vec::new());
    let now = clock.clock;
    let world = bots.tactics.as_ref().map(|t| t.world.clone());
    bots.tracker.held_sounds(now);
    if let Some(tactics) = bots.tactics.as_mut() {
        tactics.hazards.advance(now);
    }
    let view = bots
        .tracker
        .view(&clock, &bots.tuning.kin, world.as_deref());
    let viewed = Instant::now();
    if bots.tactics.is_some() {
        for team in 0..2u8 {
            if now >= bots.teams[usize::from(team)].next_commander {
                bots.commander(team, &view, &mut notes);
            }
        }
    }
    let commanded = Instant::now();
    let (mut think, mut ticks) = (0.0, 0.0);
    let mut rays = RayBudget(bots.tuning.think.rays_per_poll);
    bots.searching.left = SEARCHES_PER_POLL;
    bots.searching.floors = bots.tuning.think.search_floors_per_poll as usize;
    let a = Instant::now();
    bots.go_on_searching();
    ticks += a.elapsed().as_secs_f64();
    for index in 0..bots.bots.len() {
        if bots.bots[index].is_none() {
            continue;
        }
        let a = Instant::now();
        bots.think(index, &view, &mut rays, &mut notes);
        let b = Instant::now();
        bots.ticks(index, &clock, &view, &mut made);
        think += (b - a).as_secs_f64();
        ticks += b.elapsed().as_secs_f64();
    }
    let parts = [
        (viewed - started).as_secs_f64(),
        (commanded - viewed).as_secs_f64(),
        think,
        ticks,
    ];
    times.polls += 1;
    times.view += parts[0];
    times.commander += parts[1];
    times.think += parts[2];
    times.ticks += parts[3];
    for (worst, part) in times.worst.iter_mut().zip(parts) {
        *worst = worst.max(part as f32 * 1e6);
    }
    times.each.push(started.elapsed().as_secs_f32() * 1e6);
    (made, notes)
}

/// The cost of the bots on the real maps, only with
/// `SRP_MAPDATA_DIR=<mapdata-dump files>` (and `SRP_BOT_ROUTES` for the
/// human routes): the stand-in game on a real map, polled every 5 ms as
/// the match server's loop does with nobody sending, each part of the poll
/// timed (the stand-in's own time not counted). `SRP_BENCH_SECONDS` sets
/// a match's length (240 s; 600 s is the owner's test, for the
/// `owner-test` counts of [`owner`]).
///
/// ```text
/// SRP_MAPDATA_DIR=<dir> SRP_BOT_ROUTES=<json> cargo test -p match-server --release real_bot_cost -- --ignored --nocapture
/// ```
pub(crate) fn real_bot_cost() {
    use std::time::Instant;
    let Some(dir) = bench_settings().var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
        eprintln!("skipping: SRP_MAPDATA_DIR is not set");
        return;
    };
    // The game data the run is on. `SRP_GAME_DATA` is the server's own file,
    // read as srp reads it (`game_data::init`, with every table check), before
    // any table is used: a file that fails refuses the run, and the bench never
    // falls back to the built-in archive without saying so. The built-in
    // archive is the stock game with test patches 14-16, not the live game's
    // items, weapons and modes (`resources/game-026g/README.md`).
    let live_data = match bench_settings().var_os("SRP_GAME_DATA") {
        Some(path) if !path.is_empty() => {
            let data = survarium::v026g::game_data::init().unwrap_or_else(|cause| {
                panic!("SRP_GAME_DATA {path:?}: {cause}: the bench does not run on other data")
            });
            eprintln!(
                "game data: {} ({} files; made of {})",
                data.origin,
                data.len(),
                data.made_of()
            );
            true
        }
        _ => {
            eprintln!(
                "game data: THE BUILT-IN TEST ARCHIVE, not the live game's (the stock 0.26g with test \
                 patches 14-16); set SRP_GAME_DATA to the server's file. Respawn times stay the stand-in's constants."
            );
            false
        }
    };
    let table = bench_settings().var_os("SRP_BOT_ROUTES")
        .map(|path| super::routes::RouteTable::load(std::path::Path::new(&path)).unwrap());
    let limit_s: u32 = bench_settings().number("SRP_BENCH_SECONDS")
        .unwrap_or(240);
    let sizes: Vec<usize> = bench_settings().var("SRP_BENCH_SIDES")
        .ok()
        .map(|s| s.split(',').filter_map(|n| n.parse().ok()).collect())
        .unwrap_or(vec![4, 8, 10]);
    // Another match of the same map and size (0: the digests' runs).
    let seed: u32 = bench_settings().number("SRP_BENCH_SEED")
        .unwrap_or(0);
    use survarium::v026g::maps::mode;
    let maps: Vec<(u8, u8)> = bench_settings().var("SRP_BENCH_MAPS")
        .ok()
        .map(|s| {
            s.split(',')
                .filter_map(|m| {
                    let (level, mode) = m.split_once('/')?;
                    Some((level.parse().ok()?, mode.parse().ok()?))
                })
                .collect()
        })
        .unwrap_or(vec![
            (6, mode::BATTERIES),
            (1, mode::TEAM_DEATHMATCH),
            (3, mode::TIMED_BATTERIES),
        ]);
    for (level, mode) in maps {
        let project = format!("level_{level:02}/day_{mode}");
        let started = Instant::now();
        let map = mapdata::MapData::read(&dir.join(mapdata::format::file_name(&project))).unwrap();
        let world = Arc::new(World::build(map, mapdata::CELL, mapdata::Body::default()));
        let built_s = started.elapsed().as_secs_f32();
        // What the build freed handed back (glibc keeps it otherwise): the
        // memory of the world as the server holds it.
        mapdata::free_search_memory();
        let world_mb = memory_mb().map(|(now, _)| now);
        for &per_team in &sizes {
            let teams: Vec<u8> = (0..2 * per_team).map(|p| u8::from(p >= per_team)).collect();
            let driven = vec![true; teams.len()];
            // The bots as the lobby gives them (`bench_roster`): each its
            // role and the tier's kits, read back from its profile as the
            // match server does.
            let roster = bench_roster(&teams, bench_tier());
            let roles = &roster.roles;
            let mut bots = Bots::new(
                0xBE7C + per_team as u32 + seed * 1000,
                mode,
                level,
                &teams,
                &driven,
                roles,
                Arc::new(bench_tuning()),
                Trace::none(0),
            );
            // Every bot of one tier (`SRP_BENCH_TIER=4-5|9-10|13-15`): its
            // kits and its skill.
            bots = bots.with_tiers(&roster.tiers);
            // One-sided (`SRP_BENCH_ONE_SIDE`): team 1 by the
            // `SRP_BENCH_SIDE1_*` numbers, so a change given to team 0
            // alone shows in its K/D.
            if bench_settings().flag("SRP_BENCH_ONE_SIDE") {
                bots.side1_tuning = Some(Arc::new(bench_tuning_of(true)));
            }
            // Every body seen as a live human is (`SRP_BENCH_PING=<lo>-<hi>`,
            // ms): known to the clock less a ping drawn for each place, and
            // reckoned on from there (`Tracker::seen_late`). Off, the
            // bench's bots know each other to their decided ticks, up to
            // 100 ms past the clock, which no live human gives them.
            if let Some(pings) = bench_pings(teams.len(), seed) {
                bots.tracker.seen_late = Some(pings);
            }
            let mut fight = fight_tally::FightTally::default();
            if let Some(table) = &table {
                bots.set_routes(table);
            }
            // The bushes a sight line crosses (`SRP_BOT_FOLIAGE`, else `bot-foliage.json` of the mapdata folder).
            if let super::foliage::Source::Set(path) | super::foliage::Source::Default(path) = super::foliage::Source::choose(
                bench_settings().var_os("SRP_BOT_FOLIAGE").as_deref(),
                Some(&dir),
            ) {
                bots.set_foliage(super::foliage::Foliage::load(&path, &project).unwrap());
            }
            // Every place's weapon known as a live match knows it (the
            // weapon 1 of its role): play 11's matchup against the enemy's.
            if bench_settings().flag("SRP_BENCH_WEAPONS") {
                let weapons = bots
                    .bots
                    .iter()
                    .map(|bot| {
                        let bot = bot.as_ref()?;
                        super::reflex::WeaponKind::of_role(&bot.personality(&bots.tuning).name)
                    })
                    .collect();
                bots.set_weapons(weapons);
            }
            let started = Instant::now();
            let summary = bots.map_ready(world.clone());
            let ready_ms = started.elapsed().as_secs_f32() * 1e3;
            let ready_mb = memory_mb().map(|(now, _)| now);
            reset_peak_memory();
            let (tickets, timed) = match mode {
                mode::TEAM_DEATHMATCH => (Some(60), None),
                mode::TIMED_BATTERIES => (None, Some(20_000)),
                _ => (None, None),
            };
            let mut game = Game::new(
                world.clone(),
                &teams,
                per_team as u64 + u64::from(seed) * 1000,
                tickets,
                timed,
            );
            game.use_line = bench_settings().flag("SRP_BENCH_USE_LINE");
            // The respawn delay from the live data's mode config and the radio.
            if live_data {
                game.respawn_ms =
                    respawn_ms_from_data(level, survarium::v026g::maps::weather::DAY, mode);
                eprintln!("  respawn from the game data: {:?} ms", game.respawn_ms);
            }
            if let Some(m) = bench_settings().number("SRP_BENCH_TAKE_M")
            {
                game.take_m = m;
            }
            if let Some(n) = bench_settings().number("SRP_BENCH_UNTAKEABLE")
            {
                game.untakeable = n;
            }
            game.camp_m = bench_settings().number("SRP_BENCH_CAMP");
            game.nudge = bench_settings().number("SRP_BENCH_NUDGE")
                .map(|m| (m, NUDGE_EVERY_MS));
            game.spread = bench_settings().flag("SRP_BENCH_SPREAD");
            // As the game does (`gather_victory_items_rule::select_spawners`):
            // this many batteries at spawners drawn at random (the mode's
            // count is 10), not one at every spawner.
            if let Some(count) = bench_settings().number::<usize>("SRP_BENCH_BATTERIES")
                && timed.is_none()
                && mode != mode::TEAM_DEATHMATCH
            {
                let mut items: Vec<Vec3> =
                    world.map.battery_spawners.iter().map(|s| s.item).collect();
                let mut draw = Rng::new(0x5EED + per_team as u64 + u64::from(seed) * 1000);
                for i in (1..items.len()).rev() {
                    let j = draw.below(i + 1);
                    items.swap(i, j);
                }
                items.truncate(count);
                game.batteries = items.into_iter().map(|at| Lying::Free(at, 0)).collect();
            }
            // One side behind from the start (F0): `SRP_BENCH_START=a,b`
            // puts `a` of the lying batteries in team 0's container and
            // `b` in team 1's, with the points, the rest left lying.
            let start: [u32; 2] = bench_settings().var("SRP_BENCH_START")
                .ok()
                .and_then(|s| {
                    let (a, b) = s.split_once(',')?;
                    Some([a.trim().parse().ok()?, b.trim().parse().ok()?])
                })
                .filter(|_| mode != mode::TEAM_DEATHMATCH)
                .unwrap_or([0, 0]);
            let mut lying = 0..game.batteries.len();
            for team in 0..2u8 {
                let Some(id) = game.containers.iter().position(|(t, _)| *t == team) else {
                    continue;
                };
                for _ in 0..start[usize::from(team)] {
                    let Some(battery) = lying.next() else {
                        break;
                    };
                    game.batteries[battery] = Lying::Home(id as u8);
                    game.points[usize::from(team)] += 1;
                }
            }
            let start = game.points;
            for place in 0..teams.len() {
                bots.entered(place, true, 0);
            }
            let mut times = Times::default();
            let mut decisions = Decisions::default();
            let mut plays: std::collections::BTreeMap<String, u32> = Default::default();
            // Takes: plays ended "could not take it", and stands given up.
            let (mut take_ends, mut stands_given_up, mut stands_stale) = (0u32, 0u32, 0u32);
            let mut theft_looks = 0u32;
            let mut stolen_known: Vec<(usize, u32)> = Vec::new();
            // World-seconds a stolen battery that fell lay unknown to its
            // owners (fell alive, at death).
            let mut lying_unknown = [0u32; 2];
            super::path::reset_worst();
            let searches_before = super::path::searches();
            let crossings_before = super::crossing::counts();
            let rays_before = super::aimpoint::rays_cast();
            let limit_ms = limit_s * 1000;
            let (mut clock, mut next_world) = (0, 1_000);
            // Living bots' ticks with a goal to walk to, and those of them
            // with no route: a bot standing where it means to walk. By team.
            let (mut goal_ticks, mut stood_ticks) = ([0u64; 2], [0u64; 2]);
            // Of those, the ticks a route was missing: not while the reflex
            // had the legs (a fight: it stands where the reflex wants it), not
            // on a play taken since its last motor tick (no tick yet to plan).
            let mut stood_routing = [0u64; 2];
            let mut stood_by: std::collections::BTreeMap<String, u64> = Default::default();
            // By the bot's play: its searches that found no way (each sets
            // `Bot::no_way` afresh) and its ticks stood at a goal one of them
            // was for, while it is not searched again (`NO_WAY_RETRY_MS`).
            let mut no_way_by: std::collections::BTreeMap<String, [u64; 2]> = Default::default();
            let mut no_way_seen: Vec<Option<u32>> = vec![None; teams.len()];
            // By the bot's play: takes (and those scoring under 0.05),
            // kills, deaths, steals (the objective plays' yield, F2).
            let mut by_play: std::collections::BTreeMap<String, [u32; 5]> = Default::default();
            // By team, the attacks on the enemy's container (F0): takes,
            // living bot ticks on one, those with a teammate on one too,
            // kills and deaths on one; and all steals; attacks replaced
            // within 10 s of their take (the churn PR 123 cut).
            let mut by_team = [[0u64; 7]; 2];
            let mut last_take: Vec<Option<(bool, u32)>> = vec![None; teams.len()];
            // Living bots' ticks looking at a spawner's spot (Collect at a
            // `TargetRef::Spawner`), and their deaths while at it.
            let (mut search_ticks, mut search_deaths) = (0u64, 0u64);
            let mut was_alive: Vec<bool> = game.bodies.iter().map(|b| b.alive).collect();
            // Carriers killed with a battery, by the shooter's play and by
            // the carrier's team; and every play taken (F3: churn).
            let mut carrier_kills: std::collections::BTreeMap<String, u32> = Default::default();
            // Living bots' ticks and deaths by their play and team
            // (`Kind/team`).
            let mut play_ticks: std::collections::BTreeMap<String, u64> = Default::default();
            let mut deaths_by: std::collections::BTreeMap<String, u32> = Default::default();
            let mut carriers_killed = [0u32; 2];
            // Deaths on a Collect of a lying battery with 4 or more living
            // enemies within 25 m of it, by the team: what
            // `collect::COLLECT_ENEMY_MARGIN` may cost.
            let mut collect_crowd_deaths = [0u32; 2];
            // Deaths on a Collect of a lying battery the bot itself knew 4 or
            // more enemies by (its remembered tracks within 25 m: the count
            // `COLLECT_ENEMY_MARGIN` holds it through), by the team; of
            // those, the ones with 4 or more living enemies there too.
            let mut collect_held_deaths = [0u32; 2];
            let mut collect_held_crowd_deaths = [0u32; 2];
            // Those held deaths by the bot's ground distance to the battery
            // (up to 3 m, 10 m, farther), and by whether it had an enemy in
            // sight then (seen, not): which narrower hold would have let it go.
            let mut collect_held_dist = [0u32; 3];
            let mut collect_held_seen = [0u32; 2];
            // Each crowded Collect death: the battery's spot (x, z, m), the
            // bot's distance to it (m), held (1) or not, an enemy in sight
            // (1) or not, and the team.
            let mut collect_crowd_spots: Vec<[i32; 6]> = Vec::new();
            // The container holder (`guard_lead::is_holder_slot`), by team:
            // living holder ticks, those within 22 m of the team's container
            // (on its post: guard spots are 12-20 m out), its deaths, thefts
            // from the team's container while it had a holder alive, and
            // ticks of a need with none (a theft this match, a battery home,
            // no holder alive).
            let mut holder_ticks = [0u64; 2];
            let mut holder_post_ticks = [0u64; 2];
            let mut holder_deaths = [0u32; 2];
            let mut holder_thefts = [0u32; 2];
            let mut unheld_ticks = [0u64; 2];
            let mut takes_all = 0u32;
            let mut raids = RaidTally::default();
            let mut events = BenchEvents::open(level, mode, per_team);
            let mut carry = CarryTally::new(format!("{project}-{per_team}v{per_team}"));
            let mut routes = RouteTally::start(game.bodies.len());
            let mut push = PushTally::start(&world, game.bodies.len());
            let mut human = HumanTally::start();
            bots.bench_human = human.as_ref().map(|h| h.place);
            let mut blocks = BlockTally::default();
            let mut hits = HitTally::default();
            let mut owner = owner::OwnerTally::new(game.bodies.len());
            bots.bench_camper = bench_settings().number("SRP_BENCH_CAMPER")
                .map(|place| (place, None));
            owner.camper = bots.bench_camper.map(|(place, _)| place);
            if let Some((place, _)) = bots.bench_camper
                && camper::CamperOptions::from_env().long_gun
            {
                bots.arm_camper_long(place, bench_tier());
            }
            // The zones' own shapes, for their hits on the bodies; batteries
            // taken from floors of zones on all match, and their spots.
            // The zones' damage to the stand-in's bodies, with
            // SRP_BENCH_ZONE_DAMAGE=1 only (a GUESS of the game's until the
            // owner's CG-20 test): off, the bench decides as before.
            let zone_damage = bench_settings().var("SRP_BENCH_ZONE_DAMAGE").is_ok_and(|v| v == "1");
            let inside = Hazards::build(&world, 0.0);
            let (mut zone_takes, mut zone_spots) = (0u32, Vec::<Vec3>::new());
            while clock < limit_ms {
                clock += 5;
                while game.tick + TICK_MS <= clock {
                    let stats_before = game.stats.clone();
                    for bot in bots.bots.iter().flatten() {
                        game.bodies[bot.place].why_now = bot.bench_why;
                    }
                    game.step();
                    if let Some(tactics) = bots.tactics.as_ref() {
                        if zone_damage {
                            game.zone_tick(&inside, &tactics.hazards);
                        }
                        for at in std::mem::take(&mut game.took_from) {
                            let fixed = tactics
                                .world
                                .grid
                                .locate(at, 1.5)
                                .is_some_and(|node| tactics.hazards.fixed_at(node));
                            if fixed {
                                zone_takes += 1;
                                if !zone_spots.iter().any(|s| (*s - at).length() < 1.0) {
                                    zone_spots.push(at);
                                }
                            }
                        }
                    }
                    carry.deaths(&stats_before, &game, &bots);
                    BlockTally::gate_tally(&mut bots);
                    blocks.tick(&game, &bots, &mut events);
                    owner.tick(&mut game, &bots);
                    hits.tick(&mut game, &bots, &mut events);
                    let play_of = |place: Place| {
                        bots.bots[place]
                            .as_ref()
                            .and_then(|b| b.assignment.as_ref())
                            .map_or("none".to_owned(), |a| format!("{:?}", a.kind))
                    };
                    let attacking = |place: Place| {
                        bots.bots[place]
                            .as_ref()
                            .and_then(|b| b.assignment.as_ref())
                            .is_some_and(|a| super::team::attack(a.kind))
                    };
                    let holder = |place: Place| {
                        bots.bots[place]
                            .as_ref()
                            .and_then(|b| b.assignment.as_ref())
                            .and_then(|a| a.slot)
                            .is_some_and(super::plays::guard_lead::is_holder_slot)
                    };
                    for (place, (now, before)) in game.stats.iter().zip(&stats_before).enumerate() {
                        push.stats(place, teams[place], now.0 - before.0, now.1 - before.1);
                        let side = &mut by_team[usize::from(teams[place])];
                        if now.0 > before.0 {
                            by_play.entry(play_of(place)).or_default()[2] += now.0 - before.0;
                            if attacking(place) {
                                side[3] += u64::from(now.0 - before.0);
                            }
                        }
                        if now.1 > before.1 {
                            by_play.entry(play_of(place)).or_default()[3] += now.1 - before.1;
                            *deaths_by
                                .entry(format!("{}/{}", play_of(place), teams[place]))
                                .or_default() += now.1 - before.1;
                            let lying = bots.bots[place]
                                .as_ref()
                                .and_then(|b| b.assignment.as_ref())
                                .and_then(|a| match (a.kind, a.target) {
                                    (
                                        super::plays::PlayKind::Collect,
                                        super::plays::TargetRef::Battery(battery),
                                    ) => match game.batteries.get(battery) {
                                        Some(&Lying::Free(at, _)) => Some(at),
                                        _ => None,
                                    },
                                    _ => None,
                                });
                            let crowded = lying.is_some_and(|at| {
                                let team = game.bodies[place].team;
                                game.bodies
                                    .iter()
                                    .filter(|b| {
                                        b.alive && b.team != team && b.feet.distance_xz(at) < 25.0
                                    })
                                    .count()
                                    >= 4
                            });
                            if crowded {
                                collect_crowd_deaths[usize::from(teams[place])] +=
                                    now.1 - before.1;
                            }
                            let held = lying.is_some_and(|at| {
                                bots.bots[place].as_ref().is_some_and(|b| {
                                    b.memory
                                        .tracks()
                                        .iter()
                                        .filter(|t| t.at.distance_xz(at) < 25.0)
                                        .count()
                                        >= 4
                                })
                            });
                            if held && let Some(at) = lying {
                                let team = usize::from(teams[place]);
                                let n = now.1 - before.1;
                                let off = game.bodies[place].feet.distance_xz(at);
                                collect_held_dist[usize::from(off > 3.0) + usize::from(off > 10.0)] +=
                                    n;
                                let seen = bots.bots[place].as_ref().is_some_and(|b| {
                                    b.memory.tracks().iter().any(|t| t.sight_since.is_some())
                                });
                                collect_held_seen[usize::from(!seen)] += n;
                                collect_held_deaths[team] += now.1 - before.1;
                                if crowded {
                                    collect_held_crowd_deaths[team] += now.1 - before.1;
                                }
                            }
                            if crowded && let Some(at) = lying {
                                let seen = bots.bots[place].as_ref().is_some_and(|b| {
                                    b.memory.tracks().iter().any(|t| t.sight_since.is_some())
                                });
                                let off = game.bodies[place].feet.distance_xz(at);
                                collect_crowd_spots.push([
                                    at.x.round() as i32,
                                    at.z.round() as i32,
                                    off.round() as i32,
                                    i32::from(held),
                                    i32::from(seen),
                                    i32::from(teams[place]),
                                ]);
                            }
                            if attacking(place) {
                                side[4] += u64::from(now.1 - before.1);
                            }
                            if holder(place) {
                                holder_deaths[usize::from(teams[place])] += now.1 - before.1;
                            }
                        }
                    }
                    for fact in std::mem::take(&mut game.shot_facts) {
                        fight.shot(fact);
                    }
                    fight.sample(&game, game.tick);
                    let thieves = std::mem::take(&mut game.thieves);
                    raids.bodies(&game, &thieves, game.tick);
                    routes.tick(&game, &bots, game.tick);
                    push.tick(&bots);
                    if let Some(human) = &mut human {
                        human.tick(&game, &thieves);
                    }
                    let mut holding = [false; 2];
                    for bot in bots.bots.iter().flatten() {
                        let body = &game.bodies[bot.place];
                        if !body.alive || !holder(bot.place) {
                            continue;
                        }
                        let team = usize::from(bot.team);
                        holding[team] = true;
                        holder_ticks[team] += 1;
                        let home = game.containers.iter().find(|c| usize::from(c.0) == team);
                        if home.is_some_and(|c| body.feet.distance_xz(c.1) <= 22.0) {
                            holder_post_ticks[team] += 1;
                        }
                    }
                    for &place in &thieves {
                        let robbed = 1 - usize::from(teams[place]);
                        holder_thefts[robbed] += u32::from(holding[robbed]);
                    }
                    for (team, &held) in holding.iter().enumerate() {
                        let home = game.batteries.iter().any(|b| {
                            matches!(*b, Lying::Home(id)
                                if game.containers.get(usize::from(id))
                                    .is_some_and(|c| usize::from(c.0) == team))
                        });
                        if !held && home && bots.teams[team].knowledge.thefts_match > 0 {
                            unheld_ticks[team] += 1;
                        }
                    }
                    for place in thieves {
                        by_play.entry(play_of(place)).or_default()[4] += 1;
                        by_team[usize::from(teams[place])][5] += 1;
                    }
                    let mut on_attack = [0u64; 2];
                    for bot in bots.bots.iter().flatten() {
                        if game.bodies[bot.place].alive && attacking(bot.place) {
                            on_attack[usize::from(bot.team)] += 1;
                        }
                    }
                    for (side, n) in by_team.iter_mut().zip(on_attack) {
                        side[1] += n;
                        if n >= 2 {
                            side[2] += n;
                        }
                    }
                    for (shooter, carrier) in std::mem::take(&mut game.carrier_kills) {
                        *carrier_kills.entry(play_of(shooter)).or_default() += 1;
                        carriers_killed[usize::from(game.bodies[carrier].team)] += 1;
                    }
                    for bot in bots.bots.iter().flatten() {
                        let body = &game.bodies[bot.place];
                        if body.alive {
                            let kind = bot
                                .assignment
                                .as_ref()
                                .map_or("none".to_owned(), |a| format!("{:?}", a.kind));
                            *play_ticks.entry(format!("{kind}/{}", bot.team)).or_default() += 1;
                        }
                        let searching = bot.assignment.as_ref().is_some_and(|a| {
                            a.kind == super::plays::PlayKind::Collect
                                && matches!(a.target, super::plays::TargetRef::Spawner(_))
                        });
                        if searching && body.alive {
                            search_ticks += 1;
                        }
                        if searching && was_alive[bot.place] && !body.alive {
                            search_deaths += 1;
                        }
                        was_alive[bot.place] = body.alive;
                        let play_now = || {
                            bot.assignment
                                .as_ref()
                                .map_or("none".to_owned(), |a| format!("{:?}", a.kind))
                        };
                        if let Some((_, until)) = bot.no_way
                            && no_way_seen[bot.place] != Some(until)
                        {
                            no_way_seen[bot.place] = Some(until);
                            no_way_by.entry(play_now()).or_default()[0] += 1;
                        }
                        if let Go::To { at, within_m } = bot.intent.go
                            && body.alive
                            && body.feet.distance_xz(at) > within_m
                        {
                            let team = usize::from(body.team);
                            goal_ticks[team] += 1;
                            // A step walked straight has no route.
                            if bot.route.is_none() && !bot.intent.straight {
                                stood_ticks[team] += 1;
                                let new_play = bot
                                    .assignment
                                    .as_ref()
                                    .is_some_and(|a| a.since > bot.bench_driven);
                                if !bot.bench_legs && !new_play {
                                    stood_routing[team] += 1;
                                }
                                let play = bot
                                    .assignment
                                    .as_ref()
                                    .map_or("none".into(), |a| format!("{:?}", a.kind));
                                *stood_by.entry(play).or_default() += 1;
                                if bot.no_way.is_some_and(|(goal, until)| {
                                    clock < until && goal.distance_xz(at) <= super::REPLAN_MOVED_M
                                }) {
                                    no_way_by.entry(play_now()).or_default()[1] += 1;
                                }
                            }
                        }
                    }
                }
                if clock >= next_world {
                    next_world += 1_000;
                    events.world(clock, &mut game, &bots);
                    let world_tick = clock / TICK_MS * TICK_MS - 300;
                    let hurts = game.hurts_at(world_tick).unwrap_or_default();
                    for (place, hurt) in hurts.into_iter().enumerate() {
                        bots.hurt(place, hurt);
                    }
                    if let Some((_, fixes, health, facts)) = game.world_at(world_tick) {
                        for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                            for note in bots.fixed(place, fix, Some(health)) {
                                routes.note(&note);
                                owner.note(&note, &game);
                                decisions.note(&note);
                                events.note(&note);
                                carry.note(&note);
                            }
                        }
                        routes.world(&game, &bots);
                        push.world(&game, &bots);
                        for note in bots.facts(facts) {
                            decisions.note(&note);
                            carry.note(&note);
                            if note.text.contains("its thief no longer carrying it") {
                                theft_looks += 1;
                                game.stolen_log.push(format!("{} {}", note.time, note.text));
                            }
                        }
                        carry.world(&game, &bots);
                        // When a stolen battery that fell is known to its
                        // owners again (seen, or carried by one of them).
                        for (battery, mark) in game.stolen_marks.iter().enumerate() {
                            if let Some((owner, Some((_, alive)))) = *mark
                                && matches!(game.batteries.get(battery), Some(Lying::Free(..)))
                                && bots.teams[usize::from(owner)]
                                    .knowledge
                                    .battery(battery)
                                    .is_none()
                            {
                                lying_unknown[usize::from(!alive)] += 1;
                            }
                            if let Some((owner, Some((fell, _)))) = *mark
                                && !stolen_known.contains(&(battery, fell))
                                && (bots.teams[usize::from(owner)]
                                    .knowledge
                                    .battery(battery)
                                    .is_some()
                                    || game.batteries.get(battery).is_some_and(|b| {
                                        matches!(b, Lying::Carried(p)
                                            if game.bodies[*p].team == owner)
                                    }))
                            {
                                stolen_known.push((battery, fell));
                                game.stolen_log.push(format!(
                                    "{clock} battery {battery} known to its owners, {} ms after it fell",
                                    clock.saturating_sub(fell)
                                ));
                            }
                        }
                    }
                }
                let clock_now = Clock {
                    clock,
                    time_left_s: (limit_ms - clock) as f32 / 1000.0,
                    total_s: limit_s as f32,
                    playing: true,
                };
                let (made, notes) = timed_poll(&mut bots, clock_now, &mut times);
                raids.polled(&bots, &notes, &made);
                for note in notes {
                    decisions.note(&note);
                    owner.note(&note, &game);
                    fight.note(&note);
                    events.note(&note);
                    carry.note(&note);
                    push.note(&note, &bots);
                    if let Some(rest) = note.text.strip_prefix("takes ") {
                        takes_all += 1;
                        let kind = rest.split(' ').next().unwrap_or("").to_owned();
                        let score: f32 = rest
                            .split("(score ")
                            .nth(1)
                            .and_then(|s| s.split(':').next())
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1.0);
                        let is_attack = matches!(kind.as_str(), "Raid" | "Sneak" | "LateDenial");
                        let side = &mut by_team[usize::from(teams[note.place])];
                        if is_attack {
                            side[0] += 1;
                        }
                        if let Some((true, at)) = last_take[note.place]
                            && note.time < at + 10_000
                        {
                            side[6] += 1;
                        }
                        last_take[note.place] = Some((is_attack, note.time));
                        let row = by_play.entry(kind).or_default();
                        row[0] += 1;
                        row[1] += u32::from(score < 0.05);
                    }
                    if note.text.contains("its thief no longer carrying it") {
                        theft_looks += 1;
                    }
                    if note.text.starts_with("ends Collect: could not take it") {
                        take_ends += 1;
                    } else if note.text.starts_with("took no battery") {
                        stands_given_up += 1;
                        // Aimed at a belief the battery had left (it lies
                        // over 0.2 m from the spot the note gives).
                        let believed = note
                            .text
                            .split("believed at (")
                            .nth(1)
                            .and_then(|s| s.split(')').next())
                            .map(|s| s.split(", ").filter_map(|v| v.parse::<f32>().ok()).collect::<Vec<_>>());
                        let battery = note
                            .text
                            .strip_prefix("took no battery ")
                            .and_then(|s| s.split(' ').next())
                            .and_then(|s| s.parse::<usize>().ok());
                        if let (Some([x, y, z]), Some(battery)) = (believed.as_deref(), battery)
                            && let Some(Lying::Free(at, _)) = game.batteries.get(battery)
                            && (*at - Vec3::new(*x, *y, *z)).length() > 0.2
                        {
                            stands_stale += 1;
                            if bench_settings().var("SRP_BENCH_SHOW_STALE").is_ok() {
                                println!("  stale stand at {} ms: lies at {at:?}; {}", note.time, note.text);
                            }
                        }
                    }
                    let key: String = note
                        .text
                        .split(|c: char| c == '(' || c == ':' || c.is_ascii_digit())
                        .next()
                        .unwrap_or("")
                        .to_owned();
                    *plays.entry(key).or_default() += 1;
                }
                for made in made {
                    decisions.made(&made);
                    let r = made.input.rotation;
                    let turn = (
                        half_value(u16::from_le_bytes([r[0], r[1]])),
                        half_value(u16::from_le_bytes([r[2], r[3]])),
                    );
                    let bot = bots.bots[made.place].as_ref();
                    let target = bot.and_then(|bot| bot.reflex.target());
                    let gun = bot.map_or(Arms::fallback().primary.0, |bot| bot.reflex.held_gun());
                    game.inputs[made.place].push_back((
                        made.time,
                        made.input.actions,
                        turn,
                        target,
                        gun,
                    ));
                }
            }
            let rays = super::aimpoint::rays_cast() - rays_before - game.metric_rays;
            let searches = super::path::searches();
            let (searched, failed, search_us) = (
                searches.searches - searches_before.searches,
                searches.failed - searches_before.failed,
                searches.us - searches_before.us,
            );
            let stages: Vec<(u64, f64)> = (0..4)
                .map(|k| {
                    let n = searches.stages[k] - searches_before.stages[k];
                    let us = searches.stage_us[k] - searches_before.stage_us[k];
                    (n, us / 1e3 / n.max(1) as f64)
                })
                .collect();
            eprintln!(
                "  by stage (no way, corridor, widened, whole): (searches, ms each) {stages:?}; worst search {:.1} ms",
                searches.worst_us / 1e3
            );
            let by_expanded: Vec<u64> = (0..8)
                .map(|k| searches.by_expanded[k] - searches_before.by_expanded[k])
                .collect();
            eprintln!(
                "  floors expanded: {} in all, worst search {}; searches by floors expanded (up to {:?}, more): {by_expanded:?}; sliced {} in {} slices",
                searches.expanded - searches_before.expanded,
                searches.worst_expanded,
                super::path::EXPANDED_BUCKETS,
                searches.sliced - searches_before.sliced,
                searches.slices - searches_before.slices,
            );
            eprintln!(
                "  pulled straight: {} paths of {} floors, {} straight-line checks, {:.1} ms",
                searches.pulls - searches_before.pulls,
                searches.pull_floors - searches_before.pull_floors,
                searches.straight_checks - searches_before.straight_checks,
                (searches.pull_us - searches_before.pull_us) / 1e3,
            );
            eprintln!(
                "  grid searches {searched} ({:.1} a bot a second), {failed} failed; {:.2} ms each on average; {:.0} % of the bots' time",
                searched as f64 / teams.len() as f64 / limit_s as f64,
                search_us / 1e3 / searched.max(1) as f64,
                search_us / 1e4 / (times.view + times.commander + times.think + times.ticks)
            );
            if bench_settings().flag("SRP_BENCH_WHY") {
                let tactics = bots.tactics.as_ref().unwrap();
                let grid = &tactics.world.grid;
                for bot in bots.bots.iter().flatten() {
                    let me = game.bodies[bot.place].feet;
                    let goal = match bot.intent.go {
                        Go::To { at, .. } => Some(at),
                        _ => None,
                    };
                    let from = grid.locate(me, 1.5);
                    let to = goal.and_then(|g| grid.locate(g, 3.0));
                    let open = from.zip(to).map(|(a, b)| path::plan(grid, a, b).is_some());
                    let allowed = from.zip(to).map(|(a, b)| {
                        path::plan_where(grid, a, b, |n| tactics.allowed(n, clock, bot.side, false))
                            .is_some()
                    });
                    let objective = from.zip(to).map(|(a, b)| {
                        path::plan_where(grid, a, b, |n| tactics.allowed(n, clock, bot.side, true))
                            .is_some()
                    });
                    eprintln!(
                        "  bot {} team {} side {:?} at {me:?} goal {goal:?} objective {} from {from:?} to {to:?}: path open {open:?}, allowed {allowed:?}, objective-allowed {objective:?}; play {:?}",
                        bot.place,
                        bot.team,
                        bot.side,
                        bot.intent.objective,
                        bot.assignment.as_ref().map(|a| (a.kind, a.target)),
                    );
                }
            }
            let mut top: Vec<_> = plays.into_iter().collect();
            top.sort_by_key(|entry| std::cmp::Reverse(entry.1));
            eprintln!("  notes: {:?}", &top[..top.len().min(24)]);
            eprintln!(
                "  stood with a goal and no route: {} of {} bot ticks ({:.1} %), by team {stood_ticks:?} of {goal_ticks:?}; delivered {:?}; points {:?}, stolen {}",
                stood_ticks.iter().sum::<u64>(),
                goal_ticks.iter().sum::<u64>(),
                100.0 * stood_ticks.iter().sum::<u64>() as f64 / goal_ticks.iter().sum::<u64>().max(1) as f64,
                game.delivered,
                game.points,
                game.stolen
            );
            eprintln!(
                "  stood with no route, not fighting nor a play just taken: {} of {} bot ticks ({:.2} %), by team {stood_routing:?}",
                stood_routing.iter().sum::<u64>(),
                goal_ticks.iter().sum::<u64>(),
                100.0 * stood_routing.iter().sum::<u64>() as f64
                    / goal_ticks.iter().sum::<u64>().max(1) as f64,
            );
            eprintln!(
                "  takes: {} taken, {take_ends} could-not-take ends, {stands_given_up} stands given up ({stands_stale} at a belief the battery had left)",
                game.taken
            );
            // F2: stolen batteries that fell, alive (dropped) or at the
            // carrier's death: (home again with their owners, ms each),
            // home with the thief's side, still out at the end.
            for alive in [true, false] {
                let home: Vec<u32> = game
                    .stolen_drops
                    .iter()
                    .filter(|d| d.0 && d.2 == alive)
                    .map(|d| d.1)
                    .collect();
                let theirs = game
                    .stolen_drops
                    .iter()
                    .filter(|d| !d.0 && d.2 == alive)
                    .count();
                let out = game
                    .stolen_marks
                    .iter()
                    .flatten()
                    .filter(|m| m.1.is_some_and(|(_, a)| a == alive))
                    .count();
                eprintln!(
                    "  stolen fallen {}: home again ms {home:?}, home with the thieves {theirs}, still out {out}; lying unknown to its owners {} s; theft looks {theft_looks}",
                    if alive { "alive" } else { "at death" },
                    lying_unknown[usize::from(!alive)],
                );
            }
            for line in &game.stolen_log {
                eprintln!("  stolen log: {line}");
            }
            let mut by: Vec<_> = stood_by.into_iter().collect();
            by.sort_by_key(|entry| std::cmp::Reverse(entry.1));
            eprintln!("  stood by play: {:?}", &by[..by.len().min(8)]);
            let rows: Vec<String> = no_way_by
                .iter()
                .map(|(k, r)| format!("{k} {} {}", r[0], r[1]))
                .collect();
            eprintln!(
                "  no way by play (searches failed, ticks stood at its goal): {}",
                rows.join("; ")
            );
            let rows: Vec<String> = by_play
                .iter()
                .map(|(k, r)| format!("{k} {} {} {} {} {}", r[0], r[1], r[2], r[3], r[4]))
                .collect();
            let tick_s = f64::from(TICK_MS) / 1000.0;
            let sides: Vec<String> = by_team
                .iter()
                .map(|r| {
                    format!(
                        "[{}, {:.0}, {:.0}, {}, {}, {}, {}]",
                        r[0],
                        r[1] as f64 * tick_s,
                        r[2] as f64 * tick_s,
                        r[3],
                        r[4],
                        r[5],
                        r[6]
                    )
                })
                .collect();
            eprintln!(
                "  by team (attack takes, attack bot-s, of them with a teammate attacking, attack kills, attack deaths, steals, attacks replaced within 10 s): {}; started {:?}, points {:?}",
                sides.join(" "),
                start,
                game.points
            );
            eprintln!(
                "  by play (takes, under 0.05, kills, deaths, steals): {}",
                rows.join("; ")
            );
            let raid_runs = raids.finish();
            eprintln!("  raids: {}", raid_line(&raid_runs));
            // A side's raids: [runs, bot-s, came within 5 m, stole, died].
            let mut raid_sums = [[0f32; 5]; 2];
            for run in &raid_runs {
                let side = &mut raid_sums[usize::from(teams[run.place] & 1)];
                side[0] += 1.0;
                side[1] += run.ran_s;
                side[2] += f32::from(u8::from(run.nearest_m <= 5.0));
                side[3] += f32::from(u8::from(run.stole));
                side[4] += f32::from(u8::from(run.died));
                if bench_settings().var("SRP_BENCH_SHOW_RAIDS").is_ok() {
                    eprintln!(
                        "  raid {} {:.3} {:.1} {:.1} {} {} {}",
                        teams[run.place] & 1,
                        run.score,
                        run.ran_s,
                        run.nearest_m,
                        u8::from(run.stole),
                        u8::from(run.died),
                        run.next.split(';').next().unwrap_or_default().replace(' ', "_")
                    );
                }
            }
            eprintln!("  {}", blocks.finish(&mut events));
            eprintln!("  {}", blocks.gates.line());
            eprintln!("  {}", blocks.reloads.line());
            eprintln!("  {}", hits.finish(&mut events));
            eprintln!("  {}", fight.line(&game));
            eprintln!("  routes: {}", routes.line());
            eprintln!(
                "  jumps: pressed {} refused {}; stand-in {}",
                game.jumps_pressed,
                game.jumps_refused,
                if game.honest { "honest" } else { "old" }
            );
            let (owner_line, owner_sums) = owner.finish(&game);
            eprintln!("  {owner_line}");
            if let Some((place, spot)) = bots.bench_camper {
                eprintln!("  camper: place {place} spot {spot:?}");
            }
            eprintln!("  push: {}", push.line(&bots));
            if let Some(human) = &human {
                eprintln!("  human: {}", human.line());
            }
            eprintln!(
                "  carriers: killed carrying {:?} (by team carrying), by the shooter's play {:?}; delivered {:?}; picked up {} lying + {} stolen; plays taken {takes_all} = {:.2} a bot-minute",
                carriers_killed,
                carrier_kills,
                game.delivered,
                game.taken,
                game.stolen,
                f64::from(takes_all) / teams.len() as f64 / (f64::from(limit_s) / 60.0)
            );
            eprintln!("  carry: {}", carry.line(game.taken));
            // One line of sums for comparing runs paired by seed.
            let mut kd = [[0u32; 2]; 2];
            for (place, stat) in game.stats.iter().enumerate() {
                let side = &mut kd[usize::from(teams[place])];
                side[0] += stat.0;
                side[1] += stat.1;
            }
            let secs = |ticks: u64| (ticks as f64 * f64::from(TICK_MS) / 100.0).round() / 10.0;
            let play_s: std::collections::BTreeMap<&String, f64> = play_ticks
                .iter()
                .map(|(k, n)| (k, *n as f64 * f64::from(TICK_MS) / 1000.0))
                .collect();
            let play_rows: std::collections::BTreeMap<&String, [u32; 5]> =
                by_play.iter().map(|(k, r)| (k, *r)).collect();
            let crossings: [u64; 7] = {
                let now = super::crossing::counts();
                std::array::from_fn(|k| now[k] - crossings_before[k])
            };
            // Zone spells (`ZoneSpell`): how many, the health they took in
            // all and at most, how many left the body under 0.6, and how
            // many took each 0.02 of health (the last bin, 0.98 or more).
            let zone_spells = {
                let mut spells = game.zone_spells.clone();
                spells.extend(
                    game.bodies
                        .iter()
                        .filter_map(|b| b.zone_spell.map(|z| z.ended(b))),
                );
                let mut bins = [0u32; 50];
                for [lost, ..] in &spells {
                    bins[((lost / 0.02) as usize).min(49)] += 1;
                }
                serde_json::json!({
                    "n": spells.len(),
                    "lost": spells.iter().map(|s| f64::from(s[0])).sum::<f64>(),
                    "worst": spells.iter().map(|s| s[0]).fold(0.0f32, f32::max),
                    "under_06": spells.iter().filter(|s| s[1] < 0.6).count(),
                    // Of those: begun under 0.7 health, in a fight, either.
                    "under_06_low": spells.iter().filter(|s| s[1] < 0.6 && s[2] < 0.7).count(),
                    "under_06_fight": spells.iter().filter(|s| s[1] < 0.6 && s[3] > 0.5).count(),
                    "under_06_either": spells
                        .iter()
                        .filter(|s| s[1] < 0.6 && (s[2] < 0.7 || s[3] > 0.5))
                        .count(),
                    "bins": bins.to_vec(),
                    // Stretches by their bot's motives (`Bot::bench_why`,
                    // a mask): all, and those that left it under 0.6.
                    "why": spells.iter().fold(std::collections::BTreeMap::<String, u32>::new(), |mut m, s| {
                        *m.entry((s[4] as u16).to_string()).or_default() += 1;
                        m
                    }),
                    "under_06_why": spells.iter().filter(|s| s[1] < 0.6).fold(
                        std::collections::BTreeMap::<String, u32>::new(),
                        |mut m, s| {
                            *m.entry((s[4] as u16).to_string()).or_default() += 1;
                            m
                        },
                    ),
                })
            };
            eprintln!(
                "  sums: {}",
                serde_json::json!({
                    "map": format!("{project}-{per_team}v{per_team}"),
                    "seed": seed,
                    "kills": [kd[0][0], kd[1][0]],
                    "deaths": [kd[0][1], kd[1][1]],
                    "delivered": game.delivered,
                    "points": game.points,
                    "taken": game.taken,
                    "take_ends": take_ends,
                    "stands_given_up": stands_given_up,
                    "stands_stale": stands_stale,
                    "carriers_killed": carriers_killed,
                    "collect_crowd_deaths": collect_crowd_deaths,
                    "collect_held_deaths": collect_held_deaths,
                    "collect_held_crowd_deaths": collect_held_crowd_deaths,
                    "collect_held_dist": collect_held_dist,
                    "collect_held_seen": collect_held_seen,
                    "collect_crowd_spots": collect_crowd_spots,
                    "holder_s": holder_ticks.map(secs),
                    "holder_post_s": holder_post_ticks.map(secs),
                    "holder_deaths": holder_deaths,
                    "holder_thefts": holder_thefts,
                    "unheld_s": unheld_ticks.map(secs),
                    "play_s": play_s,
                    "by_play": play_rows,
                    "deaths_by": deaths_by,
                    "camper": game.camper.map(|(p, t)| (p, t, game.bodies[p].alive)),
                    "camp_ended": game.camp_ended,
                    "nudged": game.nudged,
                    "raids": raid_sums,
                    "stolen": game.stolen,
                    "drops": game.drops,
                    "zone_deaths": game.zone_deaths,
                    "zone_hp": game.zone_hp,
                    "zone_spells": zone_spells,
                    // Crossings' searches (`crossing.rs`): searched, taken,
                    // ring walks, refused, floors expanded, wanted for a long
                    // route, stopped on the way.
                    "crossings": crossings,
                    "zone_takes": zone_takes,
                    "zone_spots": zone_spots.len(),
                    "carry_mps": if game.carry_ticks > 0 {
                        game.carry_m / (game.carry_ticks as f32 * TICK_S)
                    } else {
                        0.0
                    },
                    "stand_in": if game.honest { "honest" } else { "old" },
                    "owner": owner_sums,
                })
            );
            eprintln!(
                "  search: {:.0} bot-s at spawners' spots, {search_deaths} deaths in it",
                search_ticks as f64 * f64::from(TICK_MS) / 1000.0
            );
            let line = format!("{project} {per_team}v{per_team}: {}", decisions.line());
            eprintln!("  {line}");
            if let Some(out) = bench_settings().var_os("SRP_DECISIONS_OUT") {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(out)
                    .unwrap();
                writeln!(file, "{line}").unwrap();
            }
            if let (Some(world_mb), Some(ready_mb), Some((_, peak))) =
                (world_mb, ready_mb, memory_mb())
            {
                eprintln!(
                    "  memory MB: {world_mb:.0} with the world, {ready_mb:.0} with the bots' \
                     tactics, peak {peak:.0} in the match"
                );
            }
            let total = times.view + times.commander + times.think + times.ticks;
            let mut each = std::mem::take(&mut times.each);
            each.sort_by(f32::total_cmp);
            let at = |q: f64| each[((each.len() - 1) as f64 * q) as usize];
            let kills: u32 = game.stats.iter().map(|s| s.0).sum();
            eprintln!(
                "{project} {per_team}v{per_team}: world built in {built_s:.2} s, map_ready {ready_ms:.1} ms ({summary})\n  \
                 {limit_s} s of play: bots {:.1} ms a second = {:.2} % of a core; per bot {:.3} %\n  \
                 poll µs p50 {:.0}, p99 {:.0}, p99.9 {:.0}, max {:.0}; {} polls\n  \
                 parts: view {:.0} %, commander {:.0} %, think {:.0} %, ticks {:.0} %; worst µs view {:.0}, commander {:.0}, think {:.0}, ticks {:.0}\n  \
                 rays {:.1} per bot a second; kills {kills}, shots {}, hits {}, team hits {}, team kills {}",
                total * 1e3 / limit_s as f64,
                total * 100.0 / limit_s as f64,
                total * 100.0 / limit_s as f64 / teams.len() as f64,
                at(0.5),
                at(0.99),
                at(0.999),
                each[each.len() - 1],
                times.polls,
                100.0 * times.view / total,
                100.0 * times.commander / total,
                100.0 * times.think / total,
                100.0 * times.ticks / total,
                times.worst[0],
                times.worst[1],
                times.worst[2],
                times.worst[3],
                rays as f64 / teams.len() as f64 / limit_s as f64,
                game.shots,
                game.hits,
                game.team_hits,
                game.team_kills,
            );
        }
    }
}

/// The tier of the bench's bots (`SRP_BENCH_TIER`: 4-5, 9-10 or 13-15; the
/// lowest when unset), as the lobby gives a match's bots the equipment of its
/// players' gear level (`BotTier::of_gear`).
pub(crate) fn bench_tier() -> survarium::v026g::loadouts::BotTier {
    match bench_settings().var("SRP_BENCH_TIER") {
        Ok(tier) if !tier.is_empty() => serde_json::from_value(serde_json::Value::String(tier))
            .expect("SRP_BENCH_TIER: 4-5, 9-10 or 13-15"),
        _ => survarium::v026g::loadouts::BotTier::default(),
    }
}

/// The bench's bots on `teams` as the lobby and the match server have them:
/// the lobby's roles for the roster (`loadouts::bot_roles`, each team's k-th
/// bot `BotRole::nth(k)`), each bot's profile as the lobby builds it
/// (`loadouts::bot_profile`, the role's three kits of `tier`), and the role
/// and tier read back from the profile with `loadouts::bot_of`, as
/// `game/driven.rs` does for a match's `Bots::new` and `with_tiers`. One
/// home each, so the bench and a live match cannot drift: a bot carries what
/// the lobby gives it, not a lowest-tier kit in every tier.
pub(crate) struct BenchRoster {
    pub(crate) roles: Vec<Option<&'static str>>,
    pub(crate) tiers: Vec<Option<survarium::v026g::loadouts::BotTier>>,
}

pub(crate) fn bench_roster(teams: &[u8], tier: survarium::v026g::loadouts::BotTier) -> BenchRoster {
    use survarium::v026g::loadouts::{bot_of, bot_profile, bot_roles};
    let bots: Vec<_> = teams
        .iter()
        .zip(bot_roles(teams))
        .enumerate()
        .map(|(n, (&team, role))| bot_of(&bot_profile(n + 1, team, role, tier)))
        .collect();
    BenchRoster {
        roles: bots.iter().map(|bot| bot.map(|(_, role)| role.name())).collect(),
        tiers: bots.iter().map(|bot| bot.map(|(tier, _)| tier)).collect(),
    }
}

/// The bench's bots are the lobby's: for each team size and tier, each team's
/// k-th bot has the role `BotRole::nth(k)` and the tier, and carries the tier's
/// kits of its role (not the lowest tier's in every tier, as when the bench
/// gave no roles), the very `Arms` the match server's bots of that role and
/// tier carry.
#[test]
fn the_bench_bots_are_the_lobbys_by_role_and_tier() {
    use survarium::v026g::loadouts::{BotRole, BotTier, bot_profile};
    for per_team in [1usize, 4, 8, 10] {
        let teams: Vec<u8> = (0..2 * per_team).map(|p| u8::from(p >= per_team)).collect();
        for tier in BotTier::ALL {
            let roster = bench_roster(&teams, tier);
            let mut bots = Bots::new(
                0xBE7C,
                survarium::v026g::maps::mode::BATTERIES,
                6,
                &teams,
                &vec![true; teams.len()],
                &roster.roles,
                Arc::new(Tuning::default()),
                Trace::none(0),
            )
            .with_tiers(&roster.tiers);
            let mut on_team = [0usize; 2];
            for (place, &team) in teams.iter().enumerate() {
                let role = BotRole::nth(on_team[usize::from(team)]);
                on_team[usize::from(team)] += 1;
                assert_eq!(roster.roles[place], Some(role.name()), "{per_team} {tier} place {place}");
                assert_eq!(roster.tiers[place], Some(tier), "{per_team} {tier} place {place}");
                // The bot carries the tier's kits of its role: the very Arms.
                let want = Arms::kits_of(tier, role);
                let bot = bots.bots[place].as_mut().expect("a bot");
                assert_eq!(bot.kits.len(), want.len(), "{tier} {role}");
                for ((arms, _), want) in bot.kits.iter().zip(&want) {
                    assert!(std::ptr::eq(*arms, *want), "{tier} {role}");
                }
                // Its first kit's weapon is the lobby profile's weapon in slot 7.
                let profile = bot_profile(place + 1, team, role, tier);
                let slot = profile.kits[0].slots[7].expect("a weapon in slot 7");
                let weapon = survarium::v026g::dictionary::Dictionary::shared()
                    .item_named(bot.reflex.held_gun().item)
                    .expect("the weapon is an item")
                    .dict_id;
                assert_eq!(profile.items[usize::from(slot.0)].dict_id, weapon, "{tier} {role}");
            }
        }
    }
}

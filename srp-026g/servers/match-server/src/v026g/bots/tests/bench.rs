use super::*;
use super::team_kit::CarrierDeaths;

/// The real-map bench's events (`SRP_BENCH_EVENTS=<dir>`): the run's notes,
/// its kills (shooter, killed) and each bot's place and play once a world,
/// one JSON line each, in `<dir>/<level>-<mode>-<n>v<n>.jsonl`. What the
/// plays' churn, the idle bots with a goal and the trades are counted from
/// (`player_bots_behaviour.md` §3 "Play choice and trades").
pub(crate) struct BenchEvents(pub(crate) Option<std::io::BufWriter<std::fs::File>>, Vec<LookTrack>, usize);

/// What the events last saw of one bot's trap habit: whether it was looking, and its outcomes so far.
#[derive(Clone, Copy, Default)]
struct LookTrack {
    looking: bool,
    outcomes: crate::v026g::bots::traps::LookOutcomes,
}

impl BenchEvents {
    pub(crate) fn open(level: u8, mode: u8, per_team: usize) -> Self {
        Self(
            bench_settings().var_os("SRP_BENCH_EVENTS").map(|dir| {
                let path = std::path::Path::new(&dir)
                    .join(format!("level_{level:02}-{mode}-{per_team}v{per_team}.jsonl"));
                std::io::BufWriter::new(std::fs::File::create(path).unwrap())
            }),
            Vec::new(),
            0,
        )
    }

    /// The traps placed and set off since the last call, one line each, with the trap's floor point (the stand-in's list)
    /// and, for a catch, who set it off and whose body that is (`kind`: the placer's own, a mate's, an enemy's).
    pub(crate) fn traps(&mut self, game: &Game) {
        use super::standin_traps::Event;
        if self.0.is_none() {
            return;
        }
        let at = |placer: Place, placed: u32| {
            game.traps
                .list
                .iter()
                .find(|trap| trap.placer == placer && trap.placed == placed)
                .map(|trap| [(trap.at.x * 10.0).round() / 10.0, (trap.at.z * 10.0).round() / 10.0])
        };
        let from = self.2.min(game.traps.events.len());
        for event in &game.traps.events[from..] {
            match *event {
                Event::Placed { tick, place, team, .. } => {
                    self.line(serde_json::json!({"t": tick, "trap_placed": {"placer": place, "team": team, "at": at(place, tick)}}));
                }
                Event::Caught { tick, placer, victim, kind, placed, placer_team, victim_team, .. } => {
                    self.line(serde_json::json!({"t": tick, "trap_caught": {"placer": placer, "victim": victim, "kind": format!("{kind:?}"), "placer_team": placer_team, "victim_team": victim_team, "at": at(placer, placed)}}));
                }
                Event::ShotOff { .. } | Event::DefusedBy { .. } => {}
            }
        }
        self.2 = game.traps.events.len();
    }

    /// The containers once (team, x, z), for the thefts' distances.
    pub(crate) fn containers(&mut self, game: &Game) {
        let at = |v: Vec3| [(v.x * 10.0).round() / 10.0, (v.z * 10.0).round() / 10.0];
        let list: Vec<serde_json::Value> = game.containers.iter().map(|&(team, pos)| serde_json::json!([team, at(pos)[0], at(pos)[1]])).collect();
        self.line(serde_json::json!({"containers": list}));
    }

    /// A theft from `robbed`'s container by `thief` this tick, and whether a holder of the robbed team was alive.
    pub(crate) fn theft(&mut self, clock: u32, thief: Place, robbed: usize, holder: bool) {
        self.line(serde_json::json!({"t": clock, "theft": {"thief": thief, "robbed": robbed, "holder": holder}}));
    }

    /// Each look of the trap habit as it starts and as it ends (placed, called away, gave up and why, or lost to a death),
    /// one line each, with where the bot stands.
    pub(crate) fn looks(&mut self, clock: u32, game: &Game, bots: &Bots) {
        use crate::v026g::bots::traps::{State, Why};
        if self.0.is_none() {
            return;
        }
        for bot in bots.bots.iter().flatten() {
            if self.1.len() <= bot.place {
                self.1.resize(bot.place + 1, LookTrack::default());
            }
            let habit = &bot.features.traps.habit;
            let (outcomes, looking) = (habit.outcomes(), matches!(habit.state(), State::Looking { .. }));
            let before = self.1[bot.place];
            let body = &game.bodies[bot.place];
            let at = [(body.feet.x * 10.0).round() / 10.0, (body.feet.z * 10.0).round() / 10.0];
            let line = |kind: String| serde_json::json!({"t": clock, "bot": bot.place, "team": bot.team, "look": kind, "at": at});
            if looking && !before.looking {
                self.line(line("start".into()));
            }
            if outcomes != before.outcomes {
                let kind = if outcomes.placed > before.outcomes.placed {
                    "placed".to_owned()
                } else if outcomes.called_away > before.outcomes.called_away {
                    "called away".to_owned()
                } else if let Some(why) = Why::ALL.iter().find(|why| outcomes.gave_up[why.index()] > before.outcomes.gave_up[why.index()]) {
                    format!("gave up: {}", why.name())
                } else {
                    "gave up: unjudged".to_owned()
                };
                self.line(line(kind));
            } else if before.looking && !looking && !body.alive {
                self.line(line("lost: died".into()));
            }
            self.1[bot.place] = LookTrack { looking, outcomes };
        }
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
    /// team, alive, x, z, yaw, play, aiming at someone, walking to a goal,
    /// carrying a battery, the place of its target (null: none), y]. The
    /// feet's height comes last so that readers by index keep their columns;
    /// without it a fall under a slab read as a climb onto it (A199).
    pub(crate) fn world(&mut self, clock: u32, game: &mut Game, bots: &Bots) {
        let kills = std::mem::take(&mut game.kill_log);
        if self.0.is_none() {
            return;
        }
        for (tick, shooter, killed) in kills {
            self.line(serde_json::json!({"t": tick, "kill": [shooter, killed]}));
        }
        // One line for each line of fire the pierce opened where the solid mesh had a wall (`Tactics::shot_reaches`):
        // who would shoot from where at what, the share of the damage that reaches and the faces it crosses.
        if let Some(tactics) = bots.tactics.as_ref() {
            let at = |v: Vec3| [(v.x * 10.0).round() / 10.0, (v.y * 10.0).round() / 10.0, (v.z * 10.0).round() / 10.0];
            for event in tactics.cover_tally.drain_events() {
                self.line(serde_json::json!({
                    "t": clock,
                    "cover_open": {"from": at(event.from), "to": at(event.to), "share": (event.share * 100.0).round() / 100.0, "crossed": event.crossed},
                }));
            }
            // One line for each hiding spot the weighing changed (A142): the bot, the enemy, the spot the filter alone gives and
            // the one given, each with the share of the bullet that reaches it.
            for event in tactics.cover_tally.drain_spot_events() {
                let share = |s: f32| (s * 100.0).round() / 100.0;
                self.line(serde_json::json!({
                    "t": clock,
                    "spot_pick": {
                        "around": at(event.around),
                        "from": at(event.from),
                        "filter": [at(event.filter.0), share(event.filter.1)],
                        "weighed": [at(event.weighed.0), share(event.weighed.1)],
                    },
                }));
            }
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
                    body.carrying.is_some(),
                    bot.reflex.target(),
                    (body.feet.y * 100.0).round() / 100.0,
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

/// Prints `line` to stderr whole, for the lines the readers parse. `eprintln!` of a
/// `serde_json::Value` makes one write per token (stderr is unbuffered), and libtest's
/// "has been running for over 60 seconds" notice, printed to stdout by the harness's
/// thread into the same log, lands between two of them. Here the line goes in one
/// write, and stdout is held meanwhile, so the notice waits for the line's end.
pub(crate) fn say_whole(line: &str) {
    use std::io::Write;
    let _notice_waits = std::io::stdout().lock();
    let mut whole = String::with_capacity(line.len() + 1);
    whole.push_str(line);
    whole.push('\n');
    std::io::stderr().lock().write_all(whole.as_bytes()).expect("stderr");
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

/// Whether the bench's worlds need the grids' table of wall clearances: either team's tuning needs it
/// ([`Tuning::needs_wall_clearance`]; team 1's is its own only in a one-sided run).
fn bench_wall_clearance() -> bool {
    bench_tuning().needs_wall_clearance() || (bench_settings().flag("SRP_BENCH_ONE_SIDE") && bench_tuning_of(true).needs_wall_clearance())
}

/// [`tuning_from`] of the environment.
pub(crate) fn bench_tuning_of(side1: bool) -> Tuning {
    tuning_from(bench_settings(), side1)
}

/// The tuning a bench run goes by: the defaults, changed by the run's settings, parsed into the typed
/// [`Tuning`] once at the end (an unknown name or a value of the wrong kind refuses the run).
///
/// - `SRP_BENCH_TUNING` (for team 1 of a one-sided run, `SRP_BENCH_SIDE1_TUNING`) is a JSON merge patch
///   (RFC 7386) over the defaults' JSON, the form of the `SRP_BOT_TUNING` file: objects merge key by key,
///   anything else (a number, a list) replaces, `null` sets an optional field to none at any depth, under a
///   section that is none in the defaults too (`{"threat": {"concede": {"lone_peek": null}}}`), and is refused
///   for any other field or an unknown name ([`Tuning::apply_patch`]).
///   A new tuning switch needs no setting of its own: the run names it by its path, `{"aim": {"combat":
///   {"band_legs": false}}}`. The patch's result is checked by [`Tuning::validate`] as the file's is.
/// - The settings below it are older names for the same changes, kept so that the bench scripts and
///   BENCH-GUIDE go on working (`SRP_BENCH_CHEAT`, `_BATTERY_SIGHT_M`, `_FRIEND_NEAR_M`, `_FF_*`,
///   `_ESCAPE_CEILING`, `_SEARCH_FLOORS`, and the number lists `SRP_BENCH_<PLAY>`, `_COMBAT`, `_HEAL`, `_AIM`,
///   `_SENSE`). They are applied first, in the order they always were, and the patch last, so it wins.
///   For `side1` only the lists (`SRP_BENCH_SIDE1_<name>`) and the patch read team 1's names.
pub(crate) fn tuning_from(settings: &BenchSettings, side1: bool) -> Tuning {
    let prefix = if side1 { "SRP_BENCH_SIDE1_" } else { "SRP_BENCH_" };
    let var = |name: &str| settings.var(&format!("{prefix}{name}"));
    let mut tuning = serde_json::to_value(Tuning::default()).expect("the tuning is JSON");
    // The floor-aware path distance on (`1`) or off (`0`, the default): `GridTuning::floor_paths`.
    if let Ok(v) = settings.var("SRP_BENCH_FLOOR_PATHS") {
        set(&mut tuning, "/grid/floor_paths", (v != "0").into());
    }
    // The lip guard (2 ticks held) and the short look-ahead on lips (0.8 m) on: both are off by default
    // (`Feel::lip_guard_ticks`, `lip_look_ahead_m`).
    if settings.var("SRP_BENCH_LIP_GUARD").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/feel/lip_guard_ticks", 2.into());
        set(&mut tuning, "/feel/lip_look_ahead_m", 0.8.into());
    }
    // A walk with no point of the route it walks to in a line steers at a clear one back of it, or plans again
    // (`Feel::unclear_aim`; off by default: it steers at the unclear point).
    if settings.var("SRP_BENCH_UNCLEAR_AIM").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/feel/unclear_aim", true.into());
    }
    // The walk's tests of the ground at the route's floor heights, not the line between each leg's ends
    // (`Feel::floor_height_clear`; off by default).
    if settings.var("SRP_BENCH_FLOOR_HEIGHT").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/feel/floor_height_clear", true.into());
    }
    // Routes of a carrier over the jump links up to the standing jump only (`Feel::carrier_jump_m`; off by default:
    // every jump link of the grid).
    if settings.var("SRP_BENCH_CARRIER_JUMP").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/feel/carrier_jump_m", mapdata::grid::STANDING_JUMP_M.into());
    }
    // The maps' grids built with the numbers the clients run (`GridTuning::client_numbers`; off by default).
    if settings.var("SRP_BENCH_CLIENT_NUMBERS").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/grid/client_numbers", true.into());
    }
    // The bot's reckoned stamina steers its sprint and jumps (`ThinkTuning::stamina`; off by default).
    if settings.var("SRP_BENCH_BOT_STAMINA").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/think/stamina", true.into());
    }
    // The floors nearer a wall than the body's radius are taken exactly after the cell rounds (`GridTuning::exact_erosion`; off by default).
    if settings.var("SRP_BENCH_EXACT_EROSION").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/grid/exact_erosion", true.into());
    }
    // A Drop link is kept only where the body that walks off comes down within the harmless fall (`GridTuning::honest_drops`; off by default).
    if settings.var("SRP_BENCH_HONEST_DROPS").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/grid/honest_drops", true.into());
    }
    // What a route's drop is for all its readers (`Feel::drop_truth`; the height test by default).
    if settings.var("SRP_BENCH_DROP_TRUTH").is_ok_and(|v| v == "links") {
        set(&mut tuning, "/feel/drop_truth", "links".into());
    }
    // Which jump links the reckoned body takes up after a press (`Kinematics::jump_snap`; any by default).
    match settings.var("SRP_BENCH_RECKON_JUMP_CAP").as_deref() {
        Ok("on" | "1") => set(&mut tuning, "/kin/jump_snap", "by_keys".into()),
        Ok("never") => set(&mut tuning, "/kin/jump_snap", "never".into()),
        _ => {}
    }
    // A planned drop unsafe at a sprint is walked off at a walk, no sprint into the air (`Feel::drop_walk`; off by default).
    if settings.var("SRP_BENCH_DROP_WALK").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/feel/drop_walk", true.into());
    }
    // A sprint-band ledge jump is pressed only at speed with sprint held (`Feel::band_jump_gate`; off by default).
    if settings.var("SRP_BENCH_BAND_JUMP_GATE").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/feel/band_jump_gate", true.into());
    }
    // The sprint gap links are run up to at a sprint and pressed at speed (`Feel::sprint_gaps`; off by default).
    if settings.var("SRP_BENCH_SPRINT_GAPS").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/feel/sprint_gaps", true.into());
    }
    // A body on no floor plans from the floor it drops to, up to 5 m below (`ThinkTuning::stranded_start`; off by default).
    if settings.var("SRP_BENCH_STRANDED_START").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/think/stranded_start", true.into());
    }
    // A rest point on a lip moved inward (`ThinkTuning::rest_off_lip`; off by default).
    if settings.var("SRP_BENCH_REST_OFF_LIP").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/think/rest_off_lip", true.into());
    }
    // The stuck watch keeps quiet while a body that landed from 6 m or more cannot move (`ThinkTuning::fall_lock`; off by default).
    if settings.var("SRP_BENCH_FALL_LOCK").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/think/fall_lock", true.into());
    }
    // The battery cheat instead of the fair battery knowledge.
    if settings.flag("SRP_BENCH_CHEAT") {
        set(&mut tuning, "/cheats/batteries", true.into());
    }
    if let Some(m) = settings.number::<f32>("SRP_BENCH_BATTERY_SIGHT_M") {
        set(&mut tuning, "/sense/battery_sight_m", m.into());
    }
    if let Some(m) = settings.number::<f32>("SRP_BENCH_FRIEND_NEAR_M") {
        set(&mut tuning, "/aim/friends/near_m", m.into());
    }
    // A landing run against the way and the path, or into a wall, is braked (`Feel::landing_cancel_deg`; 0 off).
    if let Some(d) = settings.number::<f32>("SRP_BENCH_LANDING_CANCEL_DEG") {
        set(&mut tuning, "/feel/landing_cancel_deg", d.into());
    }
    // The landing cancel covers a planned drop too (`Feel::landing_cancel_drops`; off by default).
    if settings.var("SRP_BENCH_LANDING_CANCEL_DROPS").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/feel/landing_cancel_drops", true.into());
    }
    // The teammates led to the tick decided (`FriendlyFireTuning::lead`): 0 off, 1 on; and the lead's extra, ms.
    if let Ok(v) = settings.var("SRP_BENCH_FF_LEAD") {
        set(&mut tuning, "/aim/friends/lead", (v != "0").into());
    }
    // Teammates this far past the target count (`FriendlyFireTuning::past_target_m`).
    if let Some(m) = settings.number::<f32>("SRP_BENCH_FF_PAST_M") {
        set(&mut tuning, "/aim/friends/past_target_m", m.into());
    }
    // A teammate this near in front blocks whatever his angle (`FriendlyFireTuning::touch_m`; 0 off).
    if let Some(m) = settings.number::<f32>("SRP_BENCH_FF_TOUCH_M") {
        set(&mut tuning, "/aim/friends/touch_m", m.into());
    }
    if let Some(ms) = settings.number::<u32>("SRP_BENCH_FF_LEAD_MS") {
        set(&mut tuning, "/aim/friends/lead_extra_ms", ms.into());
    }
    // The plays' numbers: `name=value,...`.
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
        ("RESUPPLY_PLAY", super::plays::PlayKind::Resupply),
    ] {
        let Ok(numbers) = var(name) else {
            continue;
        };
        let play = serde_json::to_value(kind).expect("a play's name");
        let play = play.as_str().expect("a play's name is a string");
        for pair in numbers.split(',').filter(|p| !p.is_empty()) {
            let (number, value) = pair
                .split_once('=')
                .unwrap_or_else(|| panic!("SRP_BENCH_{name}: name=value"));
            let value: f32 = value.parse().expect("a number");
            set(&mut tuning, &format!("/plays/{play}/numbers/{number}"), value.into());
        }
    }
    // The reflex's numbers (`CombatTuning`): `name=json,...` (`;` between them when a value has a comma); the
    // aim's own (`AimTuning`: `range_m`, `fire_within_deg`, `pause_ms`, ...) and the senses' (`SenseTuning`) likewise.
    for (name, section, comma_or_semicolon) in [
        ("COMBAT", "/aim/combat", true),
        ("HEAL", "/heal", true),
        ("AIM", "/aim", true),
        ("SENSE", "/sense", false),
    ] {
        let Ok(numbers) = var(name) else {
            continue;
        };
        let sep = if comma_or_semicolon && numbers.contains(';') { ';' } else { ',' };
        for pair in numbers.split(sep).filter(|p| !p.is_empty()) {
            let (key, value) = pair
                .split_once('=')
                .unwrap_or_else(|| panic!("SRP_BENCH_{name}: name=value"));
            let value = serde_json::from_str(value)
                .unwrap_or_else(|_| panic!("SRP_BENCH_{name}: a JSON value"));
            tuning
                .pointer_mut(section)
                .expect("a section of the tuning")
                .as_object_mut()
                .expect("a section is an object")
                .insert(key.to_owned(), value);
        }
    }
    // The team's threat spots (K1): `0` off, else on; its parts, and the search's floors.
    if let Ok(v) = settings.var("SRP_BENCH_THREAT") {
        set(&mut tuning, "/threat/spots", (v != "0").into());
    }
    if let Ok(v) = settings.var("SRP_BENCH_THREAT_DETOUR") {
        set(&mut tuning, "/threat/detour", (v != "0").into());
    }
    if let Ok(v) = settings.var("SRP_BENCH_THREAT_SPRINT") {
        set(&mut tuning, "/threat/sprint", (v != "0").into());
    }
    if let Some(n) = settings.number::<u32>("SRP_BENCH_THREAT_FLOORS") {
        set(&mut tuning, "/threat/search_floors", n.into());
    }
    // A way out of a zone stays under its start's strength (`HazardTuning::escape`; the flag reads as the old key did): `0` off.
    if let Ok(v) = settings.var("SRP_BENCH_ESCAPE_CEILING") {
        set(&mut tuning, "/hazard/escape", (v != "0").into());
    }
    // The humans' perches in the hold plays (V-S2b): `0` off, else on; the share's multiplier.
    if let Ok(v) = settings.var("SRP_BENCH_PERCH") {
        set(&mut tuning, "/perch/on", (v != "0").into());
    }
    if let Some(x) = settings.number::<f32>("SRP_BENCH_PERCH_P") {
        set(&mut tuning, "/perch/p_scale", x.into());
    }
    // A battery's crossing of a damage zone weighed against the pools the zone's hits take from and the bot's armour
    // (`HazardTuning::zone_pools`): `routed`; else (the default) the body pool and the raw hit.
    if let Ok(v) = settings.var("SRP_BENCH_ZONE_POOLS") {
        set(&mut tuning, "/hazard/zone_pools", if v == "routed" { "routed" } else { "raw" }.into());
    }
    // A battery no straight walk crosses to is offered when the motor's own route over the zones' floors is borne
    // (`HazardTuning::route_crossing`): `1` on; else (the default) only the straight crossings.
    if let Ok(v) = settings.var("SRP_BENCH_ROUTE_CROSSING") {
        set(&mut tuning, "/hazard/route_crossing", (v != "0").into());
    }
    // The anomaly fields of the worlds are the bots' live zones (`Tuning::live_zones`): `0` off.
    if let Ok(v) = settings.var("SRP_BENCH_LIVE_ZONES") {
        set(&mut tuning, "/hazard/live_zones", (v != "0").into());
    }
    // The sight refresh, all of it (`SenseTuning::sight_refresh`, off by default) with the hold of the cheat
    // (`CheatTuning::sight_hold_ms`: 450 to the bot's next think); `SRP_BENCH_SIGHT_HOLD` then picks another
    // hold: `think` (that one), `0` (none) or a fixed number of ms (the A/B of 250 and 450).
    if settings.var("SRP_BENCH_SIGHT_REFRESH").is_ok_and(|v| v == "on" || v == "1") {
        set(&mut tuning, "/sense/sight_refresh", true.into());
        set(&mut tuning, "/cheats/sight_hold_ms", super::know::SIGHT_HOLD_MS.into());
    }
    match settings.var("SRP_BENCH_SIGHT_HOLD").as_deref() {
        Err(_) => {}
        Ok("think") => {
            set(&mut tuning, "/cheats/sight_hold_ms", super::know::SIGHT_HOLD_MS.into());
            set(&mut tuning, "/cheats/sight_hold_to_next_think", true.into());
        }
        Ok(ms) => {
            let ms: u32 = ms.parse().expect("SRP_BENCH_SIGHT_HOLD: think or ms");
            set(&mut tuning, "/cheats/sight_hold_ms", ms.into());
            set(&mut tuning, "/cheats/sight_hold_to_next_think", false.into());
        }
    }
    if let Ok(floors) = settings.var("SRP_BENCH_SEARCH_FLOORS") {
        let floors = if floors == "max" {
            u32::MAX
        } else {
            floors
                .parse()
                .expect("SRP_BENCH_SEARCH_FLOORS: a number or max")
        };
        set(&mut tuning, "/think/search_floors_per_poll", floors.into());
    }
    let patch = var("TUNING").ok().filter(|text| !text.trim().is_empty());
    if let Some(text) = &patch {
        let patch: serde_json::Value = serde_json::from_str(text)
            .unwrap_or_else(|cause| panic!("{prefix}TUNING is not JSON: {cause}"));
        // The patch of the `SRP_BOT_TUNING` file ([`Tuning::apply_patch`]): the old keys of a moved field still work.
        Tuning::apply_patch(&mut tuning, patch).unwrap_or_else(|cause| panic!("{prefix}TUNING: {cause}"));
    }
    let tuning: Tuning = serde_json::from_value(tuning)
        .unwrap_or_else(|cause| panic!("the bench's tuning: {cause}"));
    if patch.is_some() {
        tuning
            .validate()
            .unwrap_or_else(|wrong| panic!("{prefix}TUNING: {wrong}"));
    }
    tuning
}

/// `value` is put at the JSON pointer `path` of the tuning: its parent must be there (a section the tuning
/// does not have is a typo and refuses the run); a name the section does not have is refused when the
/// whole is parsed, except a play's number, which may be any.
fn set(tuning: &mut serde_json::Value, path: &str, value: serde_json::Value) {
    let (parent, name) = path.rsplit_once('/').expect("a pointer has a name");
    tuning
        .pointer_mut(parent)
        .and_then(serde_json::Value::as_object_mut)
        .unwrap_or_else(|| panic!("the bench's tuning has no {parent}"))
        .insert(name.to_owned(), value);
}

/// [`Bots::poll`] with each part timed; the same steps in the same order.
pub(crate) fn timed_poll(bots: &mut Bots, clock: Clock, times: &mut Times) -> (Vec<Made>, Vec<Note>) {
    use std::time::Instant;
    let started = Instant::now();
    super::bench_work::set(super::bench_work::Who::Shared);
    let (mut made, mut notes) = (Vec::new(), Vec::new());
    let now = clock.clock;
    let world = bots.tactics.as_ref().map(|t| t.world.clone());
    bots.tracker.held_sounds(now, bots.match_wide_tuning().sense.sprint_body_mps);
    // What each bot's kit can throw (its frags, its smoke's key), as `Bots::poll` does.
    bots.tell_kit();
    bots.tracker.launch_grenades(now, world.as_deref());
    if let Some(tactics) = bots.tactics.as_mut() {
        tactics.hazards.advance(now);
    }
    let view = bots
        .tracker
        .view(&clock, &bots.match_wide_tuning().kin, world.as_deref());
    let viewed = Instant::now();
    super::route_twins::phase(clock.playing, now);
    if bots.tactics.is_some() {
        for team in 0..2u8 {
            if bots.commander_due(team, &clock, &view) {
                super::bench_work::set(super::bench_work::Who::Team(team));
                bots.commander(team, &view, &mut notes);
                super::bench_work::set(super::bench_work::Who::Shared);
            }
        }
    }
    let commanded = Instant::now();
    let mut think = 0.0;
    let mut rays = RayBudget::new(bots.match_wide_tuning().think.rays_per_poll);
    rays.thinks_left = bots.thinks_due(&view);
    let claims_on = [0u8, 1].map(|team| bots.tuning_of(team).number(super::plays::PlayKind::Roam, "fresh_claims") > 0.5);
    // The poll's own loop (`Bots::each_bot`: the route-search queue's order, its poll and the searches under way),
    // each bot's think timed; the ticks are the rest of the loop, the searches under way with them (they are drive
    // work, as the drive's own searches are).
    let looped = Instant::now();
    bots.each_bot(&view, |bots, index| {
        let a = Instant::now();
        // Before play begins: only rest.
        if clock.playing {
            bots.think(index, &view, &mut rays, &mut notes);
        }
        bots.sync_route_claim(index, now, claims_on);
        think += a.elapsed().as_secs_f64();
        bots.ticks(index, &clock, &view, &mut made);
        bots.zone_walk_note(index, now, &mut notes);
    });
    let ticks = looped.elapsed().as_secs_f64() - think;
    super::bench_work::set(super::bench_work::Who::Idle);
    bots.polls.record(clock.clock, &rays);
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
    super::standin::start_bench_run();
    say_the_rusher_kit();
    say_the_switches();
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
        let file = dir.join(mapdata::format::file_name(&project));
        let version = mapdata::format::file_version(&file).unwrap_or_else(|e| panic!("{e}"));
        let allow_old = bench_settings().var("SRP_BENCH_MAPDATA_OLD").as_deref() == Ok("allow");
        match super::settings::mapdata_format_verdict(&file, version, allow_old) {
            Ok(Some(warning)) => eprintln!("{warning}"),
            Ok(None) => {}
            Err(refusal) => panic!("{refusal}"),
        }
        let map = mapdata::MapData::read(&file).unwrap();
        // The grid with the gap links when this run's tuning has them
        // (`GridTuning::gap_links`, `SRP_BENCH_TUNING`).
        let links = bench_tuning().off_mesh_links();
        let world = Arc::new(World::build_with(map, mapdata::CELL, bench_tuning().grid_body(), links, bench_wall_clearance()));
        // The baked-all alternative to the live zones, for what it costs (`SRP_BENCH_BAKE_FIELD_PLACES=<n>`).
        let world = match bench_settings().number::<usize>("SRP_BENCH_BAKE_FIELD_PLACES").filter(|&n| n > 0) {
            Some(n) => {
                let generator = standin_fields::FieldGen::new(&world, 0, 1, standin_fields::Taker::None, 0, 0.0);
                let radius = bench_settings().number("SRP_BENCH_FIELD_RADIUS_M").unwrap_or(standin_fields::RADIUS_M);
                let places = &generator.places()[..n.min(generator.places().len())];
                Arc::new(standin_fields::baked_places(&world, places, radius))
            }
            None => world,
        };
        let built_s = started.elapsed().as_secs_f32();
        // What the build freed handed back (glibc keeps it otherwise): the
        // memory of the world as the server holds it.
        mapdata::free_search_memory();
        let world_mb = memory_mb().map(|(now, _)| now);
        // The audit of the grid's erosion (`GridTuning::exact_erosion`): the floors with a wall nearer than the exact clearance
        // (none with the exact pass on) and the nearest such wall, metres.
        {
            let walker = world.map.walker_triangles();
            let clearance = world.grid.exact_clearance();
            let (near, nearest) = world.grid.floors_near_walls(&walker, clearance);
            eprintln!(
                "  exact_erosion audit: {} floors, {near} nearer to a wall than {clearance:.3} m (nearest {nearest:.3} m)",
                world.grid.len()
            );
        }
        for &per_team in &sizes {
            let teams: Vec<u8> = (0..2 * per_team).map(|p| u8::from(p >= per_team)).collect();
            let driven = vec![true; teams.len()];
            // One-sided (`SRP_BENCH_ONE_SIDE`): team 1 by the
            // `SRP_BENCH_SIDE1_*` numbers, so a change given to team 0
            // alone shows in its K/D.
            let mut tunings = super::tuning::TeamTunings::new(Arc::new(bench_tuning()));
            if bench_settings().flag("SRP_BENCH_ONE_SIDE") {
                tunings.give_team1(Arc::new(bench_tuning_of(true)));
            }
            // The bots as the lobby gives them (`bench_roster`): each its role and the tier's kits, read back from its
            // profile as the match server does, built with its team's pocket store.
            let roster = bench_roster(&teams, bench_equipment(), &tunings);
            let throws = bench_throws(&roster, &tunings);
            let roles = &roster.roles;
            let mut bots = Bots::new_with(
                0xBE7C + per_team as u32 + seed * 1000,
                crate::v026g::bots::objective::Mode::parse(mode).unwrap(),
                level,
                &teams,
                &driven,
                roles,
                tunings,
                Trace::none(0),
            );
            // Every bot of one tier (`SRP_BENCH_TIER=4-5|9-10|13-15`): its
            // kits and its skill.
            bots = bots.with_tiers(&roster.equipments);
            bots.set_pantries(roster.profiles.iter().map(super::stock::Pantry::of_profile).collect());
            // The tracker reads every body's pockets from its profile, as the live server does for its bots
            // (`game/driven.rs`): a throw key lists a frag only from a kit that holds one, and the bots' frag rolls go by
            // the frags their kit holds (`aim.combat.grenade_kit_cap`).
            bots.set_throws(throws);
            // Every body seen as a live human is (`SRP_BENCH_PING=<lo>-<hi>`,
            // ms): known to the clock less a ping drawn for each place, and
            // reckoned on from there (`Tracker::seen_late`). Off, the
            // bench's bots know each other to their decided ticks, up to
            // 100 ms past the clock, which no live human gives them.
            // `SRP_BENCH_PING_JITTER=<ms>`: each place's ping off by up to
            // that either way, anew every tick (a late input arriving in
            // bursts; with no `SRP_BENCH_PING`, around none).
            let jitter = bench_settings()
                .number::<u32>("SRP_BENCH_PING_JITTER")
                .unwrap_or(0);
            let pings = bench_pings(teams.len(), seed)
                .or_else(|| (jitter > 0).then(|| vec![0; teams.len()]));
            let ping_ms = pings.clone().unwrap_or_default();
            if let Some(pings) = pings {
                // The relay discards an input later than 425 ms less the
                // largest round trip of the other players (twice a ping
                // here, which is taken as the lateness itself:
                // `Game::late_relay_reach`); a body so late is shown with
                // no error, for the input never came.
                bots.tracker.seen_reach = (0..pings.len())
                    .map(|place| {
                        let others = pings.iter().enumerate().filter(|&(p, _)| p != place);
                        425u32.saturating_sub(2 * others.map(|(_, &ping)| ping).max().unwrap_or(0))
                    })
                    .collect();
                bots.tracker.seen_late = Some(pings);
                bots.tracker.seen_jitter = (jitter, seed);
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
            // The grass (`SRP_BOT_GRASS`, else `bot-grass.json` of the mapdata folder).
            if let super::grass::Source::Set(path) | super::grass::Source::Default(path) = super::grass::Source::choose_file(
                bench_settings().var_os("SRP_BOT_GRASS").as_deref(),
                Some(&dir),
                super::grass::DEFAULT_FILE,
            ) {
                bots.set_grass(super::grass::Grass::load(&path, &project).unwrap());
            }
            // Every place's weapon known as a live match knows it (the
            // weapon 1 of its role): play 11's matchup against the enemy's.
            if bench_settings().is_on("SRP_BENCH_WEAPONS") {
                let weapons = bots
                    .bots
                    .iter()
                    .map(|bot| {
                        let bot = bot.as_ref()?;
                        super::reflex::WeaponKind::of_role(&bot.personality(bots.tunings.of(bot.team)).name)
                    })
                    .collect();
                bots.set_weapons(weapons);
            }
            // Every bot's weapon 1 shot reach (H1c, `shot_reach`; read only
            // when `SenseTuning::shots` is by weapon).
            let reach = bots
                .bots
                .iter()
                .map(|bot| {
                    let bot = bot.as_ref()?;
                    let gun = bot.kits.get(bot.kit)?.0.primary.0;
                    super::shot_reach::reach_of(&format!("wpn_{}", gun.item))
                })
                .collect();
            bots.set_shot_reach(reach);
            // The armour each bot wears against a zone's hits, as the stand-in's body wears it (the kit of the
            // bench's tier and rusher kit that has the bot's gun): read only with `HazardTuning::zone_pools`
            // routed, which also makes the stand-in's zones hurt the pools their hits take from.
            {
                let model = super::standin_damage::Model::default();
                for place in 0..bots.bots.len() {
                    let gun = bots.bots[place]
                        .as_ref()
                        .and_then(|bot| bot.kits.get(bot.kit).map(|kit| kit.0.primary.0.item));
                    if let Some(gun) = gun {
                        bots.set_zone_armour(place, super::standin_damage::zone_armour_of(model.tier, model.rusher, model.mask, gun));
                    }
                }
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
                game.respawn_rule = respawn_rule_from_data(level, survarium::v026g::maps::weather::DAY, mode);
                eprintln!(
                    "  respawn from the game data: {:?} ms with the radio",
                    game.respawn_rule.map(|(time_s, modifier)| respawn_ms_of(time_s, modifier))
                );
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
            game.volume = HitVolume::bench();
            // The client's bullet flies (standin.rs `nearest_shape_flown`) in every hit model that tests shapes, unless `SRP_BENCH_HIT_FLIGHT=0`.
            game.flight = standin_hitbox::BulletPath::bench();
            // Bullets go through cover by the client's rule when either side's cover tests go by it (the stand-in's physics
            // is shared by both sides of a head-to-head); with `cover.pierce` off on both, the first surface stops them.
            game.pierce = [0u8, 1].iter().any(|&team| bots.tuning_of(team).cover.pierce.is_some());
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
            // [diag close] Once a second, for each living bot and each living enemy 4-12 m away: moments, no frag, in a fight (a target in sight), quiet and an enemy track within 3 m, quiet and none.
            let mut close_truth = [[0u64; 5]; 2];
            // By the bot's play: its searches that found no way (each sets
            // `Bot::no_way` afresh) and its ticks stood at a goal one of them
            // was for, while it is not searched again (`NO_WAY_RETRY_MS`).
            let mut no_way_by: std::collections::BTreeMap<String, [u64; 2]> = Default::default();
            let mut no_way_lines = vec![0u8; teams.len()];
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
            let mut carrier_deaths = CarrierDeaths::default();
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
            let mut pair_apart = 0u64;
            // The same ticks split by the pair's younger life: under `leader_settle_s` (20 s, `plays::buddy`) into it, which is where the
            // respawn rule (`buddy.respawn_rule`) leaves a follower forward on purpose, and later (the real separation).
            let mut pair_apart_settling = 0u64;
            let mut holder_post_ticks = [0u64; 2];
            let mut holder_deaths = [0u32; 2];
            let mut holder_thefts = [0u32; 2];
            let mut unheld_ticks = [0u64; 2];
            let mut takes_all = 0u32;
            let mut threat_changes = 0u32;
            let mut detour_deaths = 0u32;
            let (mut bot_deaths, mut bot_ticks, mut detour_ticks) = (0u32, 0u64, 0u64);
            threat::new_match();
            let threat_searches_before = threat::searches();
            let threat_checks_before = threat::checks();
            let threat_spent_before = threat::spents();
            let mut raids = RaidTally::default();
            let mut events = BenchEvents::open(level, mode, per_team);
            events.containers(&game);
            let mut carry = CarryTally::new(format!("{project}-{per_team}v{per_team}"));
            let mut routes = RouteTally::start(game.bodies.len());
            let mut push = PushTally::start(&world, game.bodies.len());
            let perch_before = super::plays::goals::perch_counts();
            let mut human = HumanTally::start();
            let mut smoke_habit = human.as_ref().and_then(|h| standin_smoke::SmokeHabit::start(h.place, seed, level, mode, per_team));
            let mut trap_habit = human.as_ref().and_then(|h| standin_trap_habit::TrapHabit::start(h.place, seed, level, mode, per_team));
            bots.bench_human = human.as_ref().map(|h| h.place);
            bots.bench_enemy_lanes = EnemyLanes::from_env();
            let mut blocks = BlockTally::default();
            // `SRP_BENCH_VIEW_ERROR=1`: every world fix the bots are given is moved by the live error (view_error.rs).
            let mut view_error = ViewError::from_env(game.bodies.len(), seed);
            let mut hits = HitTally::default();
            let mut owner = owner::OwnerTally::new(game.bodies.len());
            owner.set_pings(ping_ms);
            game.set_grenade_kits(&roster.grenades);
            game.set_heal_kits(&roster.profiles);
            game.set_radio_kits(&roster.profiles);
            // `SRP_BENCH_STAND_IN_LEVEL` (on by default): the bodies go at their kit's level and the bots' tracker starts there too, as the server hands it.
            if standin_level::enabled() {
                let levels = standin_level::levels(&roster.upgrades, bots.bench_human);
                game.set_speed_levels(&levels);
                bots.set_speed_levels(&levels);
            }
            // The stand-in camper plays by his own tuning (`human_tuning`: the build's defaults with the human aim of
            // `camper_human_aim.json`) whatever his team's tuning is, so that the two runs of a head-to-head bench face the same
            // opponent, and a change of the bots' aim does not move him.
            let camper_options = camper::CamperOptions::from_env();
            bots.bench_camper = bench_settings()
                .number_or_refuse::<Place>("SRP_BENCH_CAMPER")
                .map(|place| BenchCamper::new(camper_options.side.place(place, per_team, seed), camper_options, seed, Arc::new(human_tuning())));
            let camper_place = bots.bench_camper.as_ref().map(BenchCamper::place);
            owner.set_camper(camper_place);
            owner.set_scoreboard(format!("{project} {per_team}v{per_team} seed {seed}"), mode, bots.bench_human, camper_place);
            if let Some(place) = camper_place
                && camper_options.long_gun
            {
                bots.arm_camper_long(place, bench_tier());
            }
            // `SRP_BENCH_CAMPER_START=spot`: each of his lives starts on his spot (the one his own think takes, `camper_spot_at`), and
            // his map side is named for him: his first feet lie near the other team's spawns, which would name theirs (`Tactics::side_at`).
            let camper_start_spot = camper_place.is_some() && camper_options.start == CamperStart::Spot;
            if let Some(place) = camper_place
                && camper_options.start == CamperStart::Spot
            {
                let side = i32::from(game.bodies[place].team);
                if let Some((at, _)) = bots.tactics.as_ref().and_then(|tactics| camper_move::camper_spot_at(tactics, side, &camper_options)) {
                    game.placed_start = Some((place, at));
                    game.spawn(place);
                    if let Some(bot) = bots.bots[place].as_mut() {
                        bot.side = Some(side);
                    }
                }
            }
            // The zones' own shapes, for their hits on the bodies; batteries
            // taken from floors of zones on all match, and their spots.
            // The zones' damage to the stand-in's bodies, with
            // SRP_BENCH_ZONE_DAMAGE=1 only (a GUESS of the game's until the
            // owner's CG-20 test): off, the bench decides as before.
            let zone_damage = bench_settings().var("SRP_BENCH_ZONE_DAMAGE").is_ok_and(|v| v == "1");
            game.zone_routed = bots.match_wide_tuning().hazard.zone_pools == super::tuning::ZonePools::Routed;
            // The client's zone rule (a hit at the body's place in the zone, the interval included; bots in live lose exactly
            // that): on unless `SRP_BENCH_ZONE_INTENSITY=0` (the flat full hit the stand-in had).
            game.zone_intensity = bench_settings().var("SRP_BENCH_ZONE_INTENSITY").is_ok_and(|v| v != "0");
            let inside = Hazards::build(&world, 0.0);
            let (mut zone_takes, mut zone_spots) = (0u32, Vec::<Vec3>::new());
            while clock < limit_ms {
                clock += 5;
                while game.tick + TICK_MS <= clock {
                    let stats_before = game.stats.clone();
                    for bot in bots.bots.iter().flatten() {
                        game.bodies[bot.place].why_now = bot.bench_why;
                        // [counts only] The mirror of the kick the bot made for the tick about to be played (`kick_gap`).
                        let next = game.tick + TICK_MS;
                        game.bodies[bot.place].mirror = bot.bench_mirror.iter().find(|m| m.0 == next).map(|m| m.1);
                    }
                    game.step();
                    // [A239] A live stand-in camper (`SRP_BENCH_CAMPER_MOVE=live` with `SRP_BENCH_CAMPER_START=spot`) starts his next life
                    // on the spot he chose while dead (`BenchCamper::start_after_death`), not always his first.
                    if camper_start_spot
                        && let (Some(camper), Some(tactics)) = (bots.bench_camper.as_mut(), bots.tactics.as_ref())
                        && let Some(body) = game.bodies.get(camper.place()).filter(|b| !b.alive)
                        && let Some(side) = bots.bots.get(camper.place()).and_then(|b| b.as_ref()).and_then(|b| b.side)
                        && let Some(at) = camper.start_after_death(tactics, side, body.spawned)
                    {
                        game.placed_start = Some((camper.place(), at));
                    }
                    // The anomaly fields of the generator (`SRP_BENCH_LIVE_FIELDS`): their lives, and the world that lists them.
                    if game.fields.on() {
                        let world = game.world.clone();
                        game.fields.tick(game.tick, &world);
                        if let Some(listed) = game.fields.world_due(game.tick) {
                            bots.fields_given(&listed, game.tick);
                            game.fields.sample_shut(world.grid.len());
                        }
                    }
                    if let Some(tactics) = bots.tactics.as_ref() {
                        if zone_damage || game.fields.on() {
                            game.zone_tick(&inside, &tactics.hazards, zone_damage);
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
                    // The stand-in human's traps laid in the last step: with `SRP_BENCH_HUMAN_TRAPS_KNOWN=on` his team knows them from now on (off: it learns
                    // them as a live team learns a human mate's, once seen; A168).
                    if let Some(habit) = &mut trap_habit {
                        let known = bench_settings().var("SRP_BENCH_HUMAN_TRAPS_KNOWN").as_deref() == Ok("on");
                        for laid in std::mem::take(&mut habit.laid) {
                            if known {
                                bots.bench_team_knows_trap(laid.team, laid.place, laid.at, laid.tick);
                            }
                        }
                    }
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
                        let detour = threat::rounded_within(place, game.tick, 15_000);
                        if bots.bots[place].is_some() && game.bodies[place].alive {
                            bot_ticks += 1;
                            detour_ticks += u64::from(detour);
                        }
                        if now.1 > before.1 {
                            bot_deaths += now.1 - before.1;
                            if detour {
                                detour_deaths += now.1 - before.1;
                            }
                        }
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
                    super::route_twins::tick(game.tick, |place| game.bodies.get(place).map(|b| (b.feet, b.alive)));
                    // The harm count of the pair rules: ticks a pair's two bodies are alive more than 60 m apart.
                    for bot in bots.bots.iter().flatten() {
                        if let Some(mate) = bot.partner.filter(|&p| bot.place < p) {
                            let (a, b) = (&game.bodies[bot.place], &game.bodies[mate]);
                            let apart = a.alive && b.alive && a.feet.distance_xz(b.feet) > 60.0;
                            pair_apart += u64::from(apart);
                            pair_apart_settling += u64::from(apart && game.tick.saturating_sub(a.spawned.max(b.spawned)) < 20_000);
                        }
                    }
                    for bot in bots.bots.iter().flatten() {
                        let body = &game.bodies[bot.place];
                        super::plays::goals::perch_watch(
                            bot.place,
                            body.feet,
                            body.alive,
                            bot.assignment.as_ref().map(|a| a.kind),
                        );
                    }
                    if let Some(human) = &mut human {
                        human.tick(&game, &thieves);
                    }
                    if let Some(habit) = &mut smoke_habit {
                        let (place, now) = (habit.place(), game.tick);
                        let fresh: Vec<bool> = (0..game.bodies.len())
                            .map(|enemy| {
                                bots.bots[place].as_ref().is_some_and(|me| {
                                    me.memory.tracks().iter().any(|t| {
                                        t.who == enemy && super::fairness::Basis::of(t, now) == super::fairness::Basis::Fresh
                                    })
                                })
                            })
                            .collect();
                        habit.tick(&game, &mut bots.tracker, now, |enemy| fresh[enemy]);
                    }
                    if let Some(habit) = &mut trap_habit {
                        let now = game.tick;
                        habit.tick(&mut game, now);
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
                        events.theft(clock, place, robbed, holding[robbed]);
                    }
                    events.looks(clock, &game, &bots);
                    events.traps(&game);
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
                        carrier_deaths.add(game.bodies[shooter].team, &game.bodies[carrier]);
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
                        if clock % 1_000 == 0 && body.alive {
                            let (has_frag, fight) = (bot.reflex.frags_left() != Some(0), bot.reflex.target().is_some());
                            for (p, enemy) in game.bodies.iter().enumerate() {
                                let d = body.feet.distance_xz(enemy.feet);
                                if p == bot.place || !enemy.alive || enemy.team == body.team || !(4.0..=12.0).contains(&d) {
                                    continue;
                                }
                                let truth = &mut close_truth[usize::from(body.team)];
                                truth[0] += 1;
                                if !has_frag {
                                    truth[1] += 1;
                                } else if fight {
                                    truth[2] += 1;
                                } else if bot.memory.tracks().iter().any(|t| t.own.is_some_and(|(at, _)| at.distance_xz(enemy.feet) < 3.0)) {
                                    truth[3] += 1;
                                } else {
                                    truth[4] += 1;
                                }
                            }
                        }
                        let play_now = || {
                            bot.assignment
                                .as_ref()
                                .map_or("none".to_owned(), |a| format!("{:?}", a.kind))
                        };
                        if let Some((goal, until)) = bot.no_way
                            && no_way_seen[bot.place] != Some(until)
                        {
                            no_way_seen[bot.place] = Some(until);
                            no_way_by.entry(play_now()).or_default()[0] += 1;
                            // The first three failed searches of each bot: who, doing what, to where (the give-up of a goal with no way).
                            if no_way_lines[bot.place] < 3 {
                                no_way_lines[bot.place] += 1;
                                eprintln!(
                                    "  NOWAYGOAL t={} place={} team={} play={} target={:?} carrying={} feet=({:.1},{:.1},{:.1}) goal=({:.1},{:.1},{:.1})",
                                    game.tick,
                                    bot.place,
                                    body.team,
                                    play_now(),
                                    bot.assignment.as_ref().map(|a| &a.target),
                                    body.carrying.is_some(),
                                    body.feet.x,
                                    body.feet.y,
                                    body.feet.z,
                                    goal.x,
                                    goal.y,
                                    goal.z
                                );
                            }
                        }
                        if let Some(Go::To { at, within_m }) = bot.bench_go
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
                    standin_heal::tell(&mut game, &mut bots, world_tick);
                    standin_traps::tell(&mut game, &mut bots, world_tick);
                    if let Some((_, fixes, health, facts)) = game.world_at(world_tick) {
                        for (place, (fix, health)) in fixes.into_iter().zip(health).enumerate() {
                            let fix = match view_error.as_mut() {
                                Some(error) => error.perturb(place, fix, clock, 1_000),
                                None => fix,
                            };
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
                    if note.text.contains(": threat spots keep ") {
                        threat_changes += 1;
                    }
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
            let up = upgrades_summary(&roster.upgrades);
            eprintln!(
                "  upgrades: {} items on {} of {} bots, mean movement level of their first kits {}",
                up["items"], up["bots"], up["of"], up["level_mean"]
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
            eprintln!("  close truth (diag) by team, per bot-second and enemy 4-12 m away [moments, no frag, fight, quiet with a track, quiet and unknown]: {close_truth:?}");
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
            eprintln!(
                "  cover: shots walled {:?} (bullet got through {:?}), enemy hits by bullets that crossed cover {:?}, kills {:?}",
                game.walled_by, game.walled_through_by, game.cover_hits_by, game.cover_kills_by
            );
            routes.finish(&game);
            eprintln!("  routes: {}", routes.line());
            eprintln!("  {}", standin_fields::cost_line(&bots));
            eprintln!(
                "  jumps: pressed {} refused {}; stand-in {}",
                game.jumps_pressed,
                game.jumps_refused,
                if game.honest { "honest" } else { "old" }
            );
            let (owner_line, mut owner_sums) = owner.finish(&game);
            // What the bots' profiles carry of upgrades (the roster's, not the tally's: it reads no profile).
            owner_sums["upgrades"] = upgrades_summary(&roster.upgrades);
            owner_sums["carrier_deaths"] = carrier_deaths.json();
            eprintln!("  {owner_line}");
            // The refreshed sight's own counts: poll gaps, the ray budget's refusals, sights lost by
            // ping band (the match-end line; the code before the refresh has none of them).
            eprintln!("  sight polls: {}", bots.poll_report());
            eprintln!("  pockets: {}", bots.pocket_report());
            if let Some(error) = view_error.as_ref() {
                eprintln!("  {}", error.line());
            }
            if let Some(camper) = &bots.bench_camper {
                eprintln!("  {}", camper.line());
            }
            // The bots' bear traps: the habit's counts (activity: placed; harm: the team's own catches, lives
            // that ended while placing) and the response to the traps of others, one line each, always.
            {
                let habit = bots.place_counts();
                let own = bots.own_trap_counts();
                eprintln!(
                    "  trap habit: armed {} placed {} died_placing {} fired {} own_team_caught {}",
                    habit.armed, habit.placed, habit.died_placing, own.fired, own.own_team_caught
                );
                eprintln!("  trap gate: {}", crate::v026g::bots::traps::gates::line());
                eprintln!("  trap pass: {}", crate::v026g::bots::traps::onpass::line());
                eprintln!("  trap defeated: {}", crate::v026g::bots::traps::defeated::line());
                eprintln!("  trap passes: {}", crate::v026g::bots::traps::passes::line());
                let looks = bots.trap_look_outcomes();
                eprintln!("  trap looks, team 0: {}", looks[0].line());
                eprintln!("  trap looks, team 1: {}", looks[1].line());
                eprintln!("  {}", bots.react_counts().line());
            }
            // The stand-in's bear traps (none carried: no line, the log as it was).
            if !game.traps.events.is_empty() {
                let [placed, enemy, mate, own, shot, hand] = game.traps.counts();
                eprintln!(
                    "  traps (stand-in): placed {placed}, caught enemy {enemy} mate {mate} own {own}, shot off {shot}, defused by hand {hand}"
                );
                eprintln!("{}", game.traps.catch_summary());
                for line in game.traps.catch_lines() {
                    eprintln!("{line}");
                }
            }
            eprintln!("  push: {}", push.line(&bots));
            eprintln!(
                "  pairs: {} apart_ticks {pair_apart} apart_settling {pair_apart_settling} apart_later {}",
                super::pair_counts::line(),
                pair_apart - pair_apart_settling
            );
            eprintln!("  route twins: {}", super::route_twins::line());
            eprintln!("  {}", game.climbs.line());
            eprintln!("  {}", game.climbs.still.line());
            eprintln!("  {}", game.slides.line());
            for line in game.climbs.lines() {
                eprintln!("  {line}");
            }
            eprintln!(
                "  threat: changes {threat_changes} deaths {bot_deaths} deaths_after_detour {detour_deaths} bot_s {:.0} detour_bot_s {:.0} checked {} exposed {} round {} no_way {} too_long {} budget {} search_floors {} exposed_asked_m {:.1} exposed_kept_m {:.1}",
                bot_ticks as f64 * f64::from(TICK_MS) / 1000.0,
                detour_ticks as f64 * f64::from(TICK_MS) / 1000.0,
                threat::checks()[0] - threat_checks_before[0],
                threat::checks()[1] - threat_checks_before[1],
                threat::searches()[0] - threat_searches_before[0],
                threat::searches()[1] - threat_searches_before[1],
                threat::searches()[2] - threat_searches_before[2],
                threat::searches()[3] - threat_searches_before[3],
                threat::spents()[0] - threat_spent_before[0],
                (threat::spents()[1] - threat_spent_before[1]) as f64 / 10.0,
                (threat::spents()[2] - threat_spent_before[2]) as f64 / 10.0
            );
            let perched = super::plays::goals::perch_counts();
            eprintln!(
                "  perch: calls {} table {} in_range {} dropped_fine {} dropped_sight {} dropped_claim {} dropped_no_way {} dropped_climb {} with_candidate {} took {} spawn {} no_floor {} fall {} lip {} arrived {} on_perch_ticks {} repicked {} died_20s {} ticks_to_arrive {} died_late {} timed_out {} lost_2m {} lost_6m {} lost_15m {} lost_far {} left_play {} left_ticks {} ring_picks {} ring_arrived {} ring_ticks_to_arrive {} ring_left {} ring_left_ticks {} ring_died {}",
                perched[0] - perch_before[0], perched[1] - perch_before[1], perched[2] - perch_before[2],
                perched[3] - perch_before[3], perched[4] - perch_before[4], perched[5] - perch_before[5],
                perched[6] - perch_before[6], perched[7] - perch_before[7], perched[8] - perch_before[8],
                perched[9] - perch_before[9], perched[10] - perch_before[10], perched[11] - perch_before[11],
                perched[12] - perch_before[12], perched[13] - perch_before[13],
                perched[14] - perch_before[14], perched[15] - perch_before[15],
                perched[16] - perch_before[16], perched[17] - perch_before[17], perched[18] - perch_before[18],
                perched[19] - perch_before[19], perched[20] - perch_before[20],
                perched[21] - perch_before[21], perched[22] - perch_before[22], perched[23] - perch_before[23], perched[24] - perch_before[24], perched[25] - perch_before[25], perched[26] - perch_before[26],
                perched[27] - perch_before[27], perched[28] - perch_before[28], perched[29] - perch_before[29],
                perched[30] - perch_before[30], perched[31] - perch_before[31], perched[32] - perch_before[32]
            );
            eprintln!("  perch_left: {}", super::plays::goals::perch_left());
            eprintln!("  hold_switch: {}", super::hold_trace::line());
            if let Some(human) = &human {
                eprintln!("  human: {}", human.line());
            }
            if let Some(habit) = &smoke_habit {
                eprintln!("  {}", habit.line());
            }
            if let Some(habit) = &mut trap_habit {
                eprintln!("  {}", habit.line(&game));
                for line in habit.placement_lines(&game) {
                    eprintln!("{line}");
                }
            }
            for line in &game.carry_kill_lines {
                eprintln!("{line}");
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
            // The same by role (kills, deaths of both teams' bots of each):
            // the rusher's kit (`SRP_BENCH_RUSHER`) against the other three's.
            let mut kd_by_role: std::collections::BTreeMap<&str, [u32; 2]> = Default::default();
            for (place, stat) in game.stats.iter().enumerate() {
                let role = roster.roles.get(place).copied().flatten().unwrap_or("none");
                let row = kd_by_role.entry(role).or_default();
                row[0] += stat.0;
                row[1] += stat.1;
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
            // What the zones took by pool and the deaths by the pool that ran out (F2; zeros unless routed).
            eprintln!(
                "  zone_pools: {}",
                serde_json::json!({"zone_pool_lost": game.zone_pool_lost, "zone_pool_deaths": game.zone_pool_deaths})
            );
            let sums = serde_json::json!({
                "map": format!("{project}-{per_team}v{per_team}"),
                "seed": seed,
                "kills": [kd[0][0], kd[1][0]],
                "deaths": [kd[0][1], kd[1][1]],
                "kd_by_role": kd_by_role,
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
            });
            say_whole(&format!("  sums: {sums}"));
            // The same JSON in a file of its own (logs from before `say_whole` have the line cut by libtest's
            // "has been running for over 60 seconds" when a cell is slow).
            if let Some(dir) = bench_settings().var_os("SRP_BENCH_SUMS_DIR") {
                // `project` is `level_01/day_2`: the slash is a path separator.
                let file = std::path::Path::new(&dir).join(format!("{project}-{per_team}v{per_team}-s{seed}.json").replace('/', "-"));
                std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, sums.to_string()))
                    .unwrap_or_else(|e| panic!("SRP_BENCH_SUMS_DIR: {}: {e}", file.display()));
            }
            eprintln!(
                "  search: {:.0} bot-s at spawners' spots, {search_deaths} deaths in it",
                search_ticks as f64 * f64::from(TICK_MS) / 1000.0
            );
            if (0..2).any(|team| bots.tunings.of(team).aim.combat.fire.weapon_class_by_data) {
                // The switch's activity and harm (the held weapons whose class,
                // bursts' table, range floor or band the data's class changes).
                let c = bots.class_counts();
                eprintln!(
                    "  weapon class by data: {} holding, {} differ, {} burst table, {} range floor, {} band",
                    c.bots, c.differ, c.burst, c.floor, c.band
                );
            }
            let line = format!("{project} {per_team}v{per_team}: {}", decisions.line());
            say_whole(&format!("  {line}"));
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

/// The bots' rusher kit as the run has it, in a line (like the game data's):
/// the setting and what the data lets the tier's rusher wear.
/// Once per run, before the matches (`switches ON <section>:`, `switches OFF <section>:`, `switch values <section>:`, `changed numbers <section>:`, `data files:`,
/// `env switches:`, `bench models:`): what the bots of this run have on and off, the data they read and the stand-in's
/// models, so a switch left off, or a model nobody set, shows in the log at once. Team 1's block (`side1 ...`) follows when
/// `SRP_BENCH_ONE_SIDE` makes its tuning differ. Logging only.
fn say_the_switches() {
    let settings = bench_settings();
    for line in bench_tuning().switch_report().lines() {
        eprintln!("{line}");
    }
    if settings.flag("SRP_BENCH_ONE_SIDE") {
        // A match-wide setting is read from team 0's tuning (`TeamTunings::match_wide`): one the sides set apart
        // flips between the two runs of a head-to-head, so the bench says so loudly.
        let (zero, one) = (bench_tuning_of(false), bench_tuning_of(true));
        if zero.think.search_queue != one.think.search_queue {
            eprintln!(
                "WARNING: the sides differ in the match-wide think.search_queue ({:?} vs {:?}): the match runs team 0's",
                zero.think.search_queue, one.think.search_queue
            );
        }
        for line in bench_tuning_of(true).switch_report().lines() {
            eprintln!("side1 {line}");
        }
    }
    let patch = settings.var("SRP_BENCH_TUNING").unwrap_or_default();
    let tuning = if patch.trim().is_empty() {
        "tuning=defaults+lists".to_owned()
    } else {
        format!("tuning=SRP_BENCH_TUNING ({} bytes, {})", patch.len(), crate::v026g::bots::switch_log::bytes_sha(patch.as_bytes()))
    };
    let mapdata = settings.var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from);
    eprintln!(
        "{}",
        crate::v026g::bots::switch_log::data_files(
            &tuning,
            settings.var_os("SRP_BOT_ROUTES").as_deref(),
            settings.var_os("SRP_BOT_FOLIAGE").as_deref(),
            settings.var_os("SRP_BOT_GRASS").as_deref(),
            mapdata.as_deref(),
        )
    );
    eprintln!("{}", crate::v026g::bots::switch_log::env_switches());
    eprintln!("{}", super::settings::models_line(settings));
}

fn say_the_rusher_kit() {
    let tier = bench_tier();
    let asked = bench_rusher_kits().of(tier);
    let worn = bench_equipment().rusher;
    if tier == survarium::v026g::loadouts::BotTier::Low {
        eprintln!("rusher kit: tier {tier} has the one kit (mp_153)");
    } else if asked == worn {
        eprintln!("rusher kit: tier {tier} `{}`", worn.name());
    } else {
        eprintln!(
            "rusher kit: tier {tier} asked `{}`, THE GAME DATA LACKS ITS ITEMS: the old kit `{}`",
            asked.name(),
            worn.name()
        );
    }
}

/// The kit the rusher bots of the bench wear (`SRP_BENCH_RUSHER`, as the
/// lobby's `SRP_PVB_RUSHER_KIT`: one word or a kit a tier; the old table
/// when unset), and whether every bot wears the top players' respirator of its tier (`SRP_BENCH_MASKS=top`,
/// as the lobby's `SRP_PVB_MASKS`; none when unset).
pub(crate) fn bench_rusher_kits() -> survarium::v026g::loadouts::RusherKits {
    use survarium::v026g::loadouts::{Masks, RusherKits};
    let mut kits = match bench_settings().var("SRP_BENCH_RUSHER") {
        Ok(text) if !text.is_empty() => {
            RusherKits::parse(&text).unwrap_or_else(|why| panic!("SRP_BENCH_RUSHER: {why}"))
        }
        _ => RusherKits::default(),
    };
    if let Ok(text) = bench_settings().var("SRP_BENCH_MASKS") {
        kits.masks = Masks::parse(&text).unwrap_or_else(|why| panic!("SRP_BENCH_MASKS: {why}"));
    }
    kits
}

/// The equipment of the bench's bots: their tier (`SRP_BENCH_TIER`) and the
/// kit that tier's rusher wears (`SRP_BENCH_RUSHER`) in the game data the
/// run is on.
pub(crate) fn bench_equipment() -> survarium::v026g::loadouts::BotEquipment {
    survarium::v026g::loadouts::BotEquipment::of(bench_tier(), bench_rusher_kits())
}

/// The bench's bots on `teams` as the lobby and the match server have them:
/// the lobby's roles for the roster (`loadouts::bot_roles`, each team's k-th
/// bot `BotRole::nth(k)`), each bot's profile as the lobby builds it
/// (`loadouts::bot_profile`, the role's three kits of `equipment`), and the
/// role and equipment read back from the profile with
/// `loadouts::bot_equipment_of`, as `game/driven.rs` does for a match's
/// `Bots::new` and `with_tiers`. One home each, so the bench and a live
/// match cannot drift: a bot carries what the lobby gives it, not a
/// lowest-tier kit in every tier.
pub(crate) struct BenchRoster {
    pub(crate) roles: Vec<Option<&'static str>>,
    pub(crate) equipments: Vec<Option<survarium::v026g::loadouts::BotEquipment>>,
    /// What each bot's profile carries of upgrades ([`Upgrades`]).
    pub(crate) upgrades: Vec<Upgrades>,
    /// The frag grenades each of a bot's three kits holds and has in store (the profile's `count` and `stored` of its grenade pockets),
    /// by place; none for a place of no role.
    pub(crate) grenades: Vec<[standin_grenade::KitGrenades; 3]>,
    /// Each bot's profile, built with its team's pocket store (`kit.pocket_store` of its tuning).
    pub(crate) profiles: Vec<survarium::v026g::match_msgs::static_info::MatchProfile>,
}

/// What a bot's profile carries of upgrades (`loadouts::BOT_UPGRADES`, on with `SRP_BOT_UPGRADES`): the items of the profile
/// that wear a modification, and the speed level its first kit gives (`bot_skills::profile_movement_level`, the level the
/// match server hands its bots' tracker in `game/driven.rs`). This is what the profiles hold: the bench's stand-in does not
/// use it when `SRP_BENCH_STAND_IN_LEVEL` is on, its default (`standin_level.rs`; `off`: the bodies move at the nominal speed, and `Bots::set_speed_levels`
/// is not called), so a bot's moves do not change with it here. It says that an arm which turns the upgrades on has them on, and how
/// fast the profiles say they run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Upgrades {
    pub(crate) items: u32,
    pub(crate) level: Option<f32>,
}

impl Upgrades {
    pub(crate) fn of(profile: &survarium::v026g::match_msgs::static_info::MatchProfile) -> Self {
        Self {
            items: profile
                .items
                .iter()
                .filter(|item| item.modifications.iter().any(|&(_, defect)| !defect))
                .count() as u32,
            level: survarium::v026g::bot_skills::profile_movement_level(profile, 0),
        }
    }
}

/// The bots' upgrades of a match, for its `upgrades:` line and the `upgrades` key of its sums: the items that wear
/// upgrades, the bots that have one, and the mean speed level of the bots' first kits.
pub(crate) fn upgrades_summary(upgrades: &[Upgrades]) -> serde_json::Value {
    let levels: Vec<f32> = upgrades.iter().filter_map(|u| u.level).collect();
    let mean = if levels.is_empty() { 0.0 } else { levels.iter().sum::<f32>() / levels.len() as f32 };
    serde_json::json!({
        "items": upgrades.iter().map(|u| u.items).sum::<u32>(),
        "bots": upgrades.iter().filter(|u| u.items > 0).count(),
        "of": upgrades.len(),
        "level_mean": (f64::from(mean) * 1000.0).round() / 1000.0,
    })
}

/// The frag grenades a kit starts a life with and has in store: the units and the store of its pocket items that are a frag by the
/// game data (a smoke is no blast; the stand-in explodes frags only).
fn frag_pockets(kit: &survarium::v026g::loadouts::Loadout) -> standin_grenade::KitGrenades {
    let dictionary = survarium::v026g::dictionary::Dictionary::shared();
    kit.items
        .iter()
        .filter(|item| survarium::v026g::loadouts::POCKET_SLOTS.contains(&item.slot))
        .filter(|item| dictionary.item(item.dict_id).is_some_and(|i| super::arc::Grenade::named(&i.name).is_some()))
        .fold(standin_grenade::KitGrenades::default(), |sum, item| standin_grenade::KitGrenades {
            count: sum.count + item.count,
            stored: sum.stored + item.stored,
        })
}

/// What each bench body's throw keys throw: its profile's pockets, read as a server bot's
/// ([`super::live_grenades::Throws::of_profile`]), with the kit-in-use rule of the match-wide tuning (`avoid.kit_in_use`).
pub(crate) fn bench_throws(
    roster: &BenchRoster,
    tunings: &super::tuning::TeamTunings,
) -> Vec<super::live_grenades::Throws> {
    use super::live_grenades::{Player, Throws};
    let all_kits = tunings.match_wide().avoid.kit_in_use;
    roster
        .profiles
        .iter()
        .map(|profile| Throws::of_profile(profile, Player::Bot, all_kits))
        .collect()
}

pub(crate) fn bench_roster(
    teams: &[u8],
    equipment: survarium::v026g::loadouts::BotEquipment,
    tunings: &super::tuning::TeamTunings,
) -> BenchRoster {
    use survarium::v026g::loadouts::{bot_equipment_of, bot_profile_kit, bot_roles};
    let profiles: Vec<_> = teams
        .iter()
        .zip(bot_roles(teams))
        .enumerate()
        .map(|(n, (&team, role))| {
            let _kit = team_kit_scope(team);
            bot_profile_kit(n + 1, team, role, equipment, tunings.of(team).kit.pockets())
        })
        .collect();
    let bots: Vec<_> = profiles.iter().map(bot_equipment_of).collect();
    BenchRoster {
        grenades: bots
            .iter()
            .zip(teams)
            .map(|(bot, &team)| {
                let _kit = team_kit_scope(team);
                bot.map_or([standin_grenade::KitGrenades::default(); 3], |(equipment, role)| {
                    let kits = survarium::v026g::loadouts::bot_kits_kit(equipment, role, tunings.of(team).kit.pockets());
                    std::array::from_fn(|k| frag_pockets(kits[k]))
                })
            })
            .collect(),
        profiles: profiles.clone(),
        upgrades: profiles.iter().map(Upgrades::of).collect(),
        roles: bots
            .iter()
            .map(|bot| bot.map(|(_, role)| role.name()))
            .collect(),
        equipments: bots
            .iter()
            .map(|bot| bot.map(|(equipment, _)| equipment))
            .collect(),
    }
}

/// The bench's bots are the lobby's: for each team size and tier, each team's
/// k-th bot has the role `BotRole::nth(k)` and the tier, and carries the tier's
/// kits of its role (not the lowest tier's in every tier, as when the bench
/// gave no roles), the very `Arms` the match server's bots of that role and
/// tier carry.
#[test]
pub(crate) fn the_bench_bots_are_the_lobbys_by_role_and_tier() {
    use survarium::v026g::loadouts::{BotEquipment, BotRole, BotTier, RusherKits, bot_profile};
    for per_team in [1usize, 4, 8, 10] {
        let teams: Vec<u8> = (0..2 * per_team).map(|p| u8::from(p >= per_team)).collect();
        for tier in BotTier::ALL {
            let equipment = BotEquipment::of(tier, RusherKits::default());
            let roster = bench_roster(&teams, equipment, &super::tuning::TeamTunings::new(Arc::new(Tuning::default())));
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
            .with_tiers(&roster.equipments);
            let mut on_team = [0usize; 2];
            for (place, &team) in teams.iter().enumerate() {
                let role = BotRole::nth(on_team[usize::from(team)]);
                on_team[usize::from(team)] += 1;
                assert_eq!(roster.roles[place], Some(role.name()), "{per_team} {tier} place {place}");
                assert_eq!(
                    roster.equipments[place].map(|equipment| equipment.tier),
                    Some(tier),
                    "{per_team} {tier} place {place}"
                );
                // The bot carries the tier's kits of its role: the very Arms
                // (the rusher's kit is read back as the lobby gave it; the
                // flanker's, where another kit makes the same bot, may not
                // be).
                let read = roster.equipments[place].expect("a bot of an equipment");
                if role == BotRole::Rusher {
                    assert_eq!(read, equipment, "{per_team} {tier} place {place}");
                }
                let want = Arms::kits_of(read, role);
                let bot = bots.bots[place].as_mut().expect("a bot");
                assert_eq!(bot.kits.len(), want.len(), "{tier} {role}");
                for ((arms, _), want) in bot.kits.iter().zip(&want) {
                    assert!(std::ptr::eq(*arms, *want), "{tier} {role}");
                }
                // Its first kit's weapon is the lobby profile's weapon in slot 7.
                let profile = bot_profile(place + 1, team, role, equipment);
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


#[cfg(test)]
mod tuning_tests {
    use super::*;

    /// The bench's tuning before it was parsed once from JSON: the same switches, each set on the typed
    /// tuning. Kept to show that the one parse gives what it gave.
    fn reference(settings: &BenchSettings, side1: bool) -> Tuning {
        let var = |name: &str| {
            let prefix = if side1 { "SRP_BENCH_SIDE1_" } else { "SRP_BENCH_" };
            settings.var(&format!("{prefix}{name}"))
        };
        let mut tuning = Tuning::default();
        // The battery cheat instead of the fair battery knowledge.
        if settings.flag("SRP_BENCH_CHEAT") {
            tuning.cheats.batteries = true;
        }
        if let Some(m) = settings.number("SRP_BENCH_BATTERY_SIGHT_M")
        {
            tuning.sense.battery_sight_m = m;
        }
        if let Some(m) = settings.number("SRP_BENCH_FRIEND_NEAR_M")
        {
            tuning.aim.friends.near_m = m;
        }
        // The teammates led to the tick decided (`FriendlyFireTuning::lead`):
        // 0 off, 1 on; and the lead's extra, ms.
        if let Ok(v) = settings.var("SRP_BENCH_FF_LEAD") {
            tuning.aim.friends.lead = v != "0";
        }
        // Teammates this far past the target count
        // (`FriendlyFireTuning::past_target_m`).
        if let Some(m) = settings.number("SRP_BENCH_FF_PAST_M")
        {
            tuning.aim.friends.past_target_m = m;
        }
        // A teammate this near in front blocks whatever his angle
        // (`FriendlyFireTuning::touch_m`; 0 off).
        if let Some(m) = settings.number("SRP_BENCH_FF_TOUCH_M")
        {
            tuning.aim.friends.touch_m = m;
        }
        if let Some(ms) = settings.number("SRP_BENCH_FF_LEAD_MS")
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
        // A way out of a zone stays under its start's strength
        // (`HazardTuning::escape`): `0` off.
        if let Ok(v) = settings.var("SRP_BENCH_ESCAPE_CEILING") {
            tuning.hazard.escape =
                if v != "0" { super::tuning::EscapeRule::UnderStartZone } else { super::tuning::EscapeRule::Nearest };
        }
        if let Ok(floors) = settings.var("SRP_BENCH_SEARCH_FLOORS") {
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

    fn settings(pairs: &[(&str, &str)]) -> BenchSettings {
        BenchSettings::from_pairs(pairs)
    }

    /// The sight refresh is on with no setting (the defaults), `SRP_BENCH_SIGHT_REFRESH` on is the same, and the
    /// patch is what turns it off; `SRP_BENCH_SIGHT_HOLD` picks the hold.
    #[test]
    fn the_sight_refresh_setting_is_the_default_and_the_patch_turns_it_off() {
        let on = tuning_from(&settings(&[]), false);
        assert!(on.sense.sight_refresh);
        let hold = super::super::know::SIGHT_HOLD_MS;
        assert_eq!(on.cheats.sight_hold_ms, hold);
        assert_eq!(on, Tuning::default());
        for value in ["1", "on", "0"] {
            assert_eq!(tuning_from(&settings(&[("SRP_BENCH_SIGHT_REFRESH", value)]), false), on);
        }
        let mut off = Tuning::default();
        off.sense.sight_refresh = false;
        off.cheats.sight_hold_ms = 0;
        let patched = settings(&[("SRP_BENCH_TUNING", r#"{"sense":{"sight_refresh":false},"cheats":{"sight_hold_ms":0}}"#)]);
        assert_eq!(tuning_from(&patched, false), off);
        let with_hold = |hold: &str| {
            tuning_from(&settings(&[("SRP_BENCH_SIGHT_REFRESH", "1"), ("SRP_BENCH_SIGHT_HOLD", hold)]), false).cheats
        };
        assert_eq!((with_hold("250").sight_hold_ms, with_hold("250").sight_hold_to_next_think), (250, false));
        assert_eq!((with_hold("think").sight_hold_ms, with_hold("think").sight_hold_to_next_think), (hold, true));
        assert_eq!(with_hold("0").sight_hold_ms, 0);
    }

    /// Every older switch gives the tuning it gave, for team 0 and for team 1.
    #[test]
    fn every_older_switch_gives_the_tuning_it_gave() {
        let cases: Vec<Vec<(&str, &str)>> = vec![
            vec![],
            vec![("SRP_BENCH_CHEAT", "")],
            vec![("SRP_BENCH_BATTERY_SIGHT_M", "12.5"), ("SRP_BENCH_FRIEND_NEAR_M", "0.8")],
            vec![
                ("SRP_BENCH_FF_LEAD", "0"),
                ("SRP_BENCH_FF_PAST_M", "3"),
                ("SRP_BENCH_FF_TOUCH_M", "0"),
                ("SRP_BENCH_FF_LEAD_MS", "40"),
            ],
            vec![("SRP_BENCH_COLLECT", "look_weight=0.5,search_weight=0.3"), ("SRP_BENCH_CUT", "weight_tdm=0.4")],
            vec![("SRP_BENCH_COMBAT", "band_legs=false,range_floor_rifle_m=70")],
            vec![("SRP_BENCH_COMBAT", "hide_m=[1,2];band_legs=false")],
            vec![("SRP_BENCH_AIM", "wander_s=3;burst_ms=[10,20]")],
            vec![("SRP_BENCH_SENSE", "touch_m=1.5")],
            vec![("SRP_BENCH_ESCAPE_CEILING", "0"), ("SRP_BENCH_SEARCH_FLOORS", "max")],
            vec![("SRP_BENCH_SEARCH_FLOORS", "100")],
            vec![
                ("SRP_BENCH_SIDE1_COMBAT", "band_legs=false"),
                ("SRP_BENCH_SIDE1_CUT", "weight_tdm=0.4"),
                ("SRP_BENCH_COMBAT", "range_floor_rifle_m=70"),
            ],
        ];
        for pairs in &cases {
            let s = settings(pairs);
            for side1 in [false, true] {
                assert_eq!(tuning_from(&s, side1), reference(&s, side1), "{pairs:?} side1 {side1}");
            }
        }
        // And the defaults come back from their JSON as they are.
        assert_eq!(tuning_from(&settings(&[]), false), Tuning::default());
    }

    /// The patch does what the older switches do, for team 0 and (by its own name) for team 1, and wins
    /// over them when both name a thing.
    #[test]
    fn the_patch_is_the_older_switches_in_one_setting() {
        let patch = r#"{"escape_ceiling": false, "aim": {"combat": {"band_legs": false}},
                        "plays": {"cut": {"numbers": {"weight_tdm": 0.5}}}}"#;
        let by_patch = tuning_from(&settings(&[("SRP_BENCH_TUNING", patch)]), false);
        let by_switches = tuning_from(
            &settings(&[
                ("SRP_BENCH_ESCAPE_CEILING", "0"),
                ("SRP_BENCH_COMBAT", "band_legs=false"),
                ("SRP_BENCH_CUT", "weight_tdm=0.5"),
            ]),
            false,
        );
        assert_eq!(by_patch, by_switches);
        assert_ne!(by_patch, Tuning::default());
        // The keys of the fields that moved into a section (`tuning/legacy.rs`) still work in a patch, and are the new ones.
        let old = tuning_from(
            &settings(&[("SRP_BENCH_TUNING", r#"{"hazard_margin": 2.5, "escape_ceiling": false, "gap_links": true}"#)]),
            false,
        );
        let new = tuning_from(
            &settings(&[(
                "SRP_BENCH_TUNING",
                r#"{"hazard": {"margin_m": 2.5, "escape": "nearest"}, "grid": {"gap_links": true}}"#,
            )]),
            false,
        );
        assert_eq!(old, new);
        assert_eq!(old.hazard.margin_m.get(), 2.5);
        assert!(old.grid.gap_links);
        // Team 1 reads its own patch only.
        let side1 = tuning_from(&settings(&[("SRP_BENCH_SIDE1_TUNING", patch)]), true);
        assert_eq!(side1, by_patch);
        assert_eq!(tuning_from(&settings(&[("SRP_BENCH_SIDE1_TUNING", patch)]), false), Tuning::default());
        // The patch wins over an older switch naming the same thing.
        let both = tuning_from(
            &settings(&[("SRP_BENCH_COMBAT", "band_legs=true"), ("SRP_BENCH_TUNING", patch)]),
            false,
        );
        assert!(!both.aim.combat.band_legs);
        // A patch changes only what it names, and keeps what an older switch set elsewhere.
        let kept = tuning_from(
            &settings(&[
                ("SRP_BENCH_COMBAT", "range_floor_rifle_m=70"),
                ("SRP_BENCH_TUNING", r#"{"aim":{"combat":{"band_legs":false}}}"#),
            ]),
            false,
        );
        assert_eq!(kept.aim.combat.range_floor_rifle_m, 70.0);
        assert!(!kept.aim.combat.band_legs);
    }

    /// A name the tuning does not have, a value of the wrong kind, a value its limits refuse, and text
    /// that is not JSON each refuse the run.
    #[test]
    fn a_patch_that_is_wrong_refuses_the_run() {
        for bad in [
            r#"{"aim": {"combat": {"no_such_switch": true}}}"#,
            r#"{"escape_ceiling": "yes"}"#,
            r#"{"sense": {"view_deg": 0}}"#,
            "{not json",
        ] {
            let refused = std::panic::catch_unwind(|| tuning_from(&settings(&[("SRP_BENCH_TUNING", bad)]), false));
            assert!(refused.is_err(), "{bad}");
        }
        // An older switch with a name the tuning lacks is refused too.
        let refused = std::panic::catch_unwind(|| tuning_from(&settings(&[("SRP_BENCH_SENSE", "no_such=1")]), false));
        assert!(refused.is_err());
    }
}

/// A profile with the bots' upgrades on has items that wear modifications and a higher speed level than the plain
/// one; a plain profile has no item that does.
#[test]
fn a_profile_with_upgrades_counts_its_modified_items_and_its_speed_level() {
    use survarium::v026g::dictionary::Dictionary;
    use survarium::v026g::loadouts::{BotEquipment, BotRole, BotTier, RusherKits, bot_kits_in};
    use survarium::v026g::match_msgs::static_info::MatchProfile;
    let equipment = BotEquipment::of(BotTier::High, RusherKits::default());
    let profile = |upgrades: bool| {
        let kits = bot_kits_in(equipment, BotRole::Flanker, false, upgrades);
        let kits: Vec<&_> = kits.iter().collect();
        MatchProfile::from_loadouts(
            "BOT",
            0,
            &kits,
            survarium::v026g::bot_skills::bot_modifiers(&kits),
            Dictionary::shared(),
            1,
        )
    };
    let (plain, worn) = (Upgrades::of(&profile(false)), Upgrades::of(&profile(true)));
    assert_eq!(plain.items, 0);
    assert!(worn.items >= 1, "{worn:?}");
    assert!(worn.level > plain.level, "{worn:?} against {plain:?}");
    let summary = upgrades_summary(&[plain, worn, Upgrades { items: 0, level: None }]);
    assert_eq!(summary["items"], worn.items);
    assert_eq!((summary["bots"].as_u64(), summary["of"].as_u64()), (Some(1), Some(3)));
    let mean = (plain.level.unwrap() + worn.level.unwrap()) / 2.0;
    assert!((summary["level_mean"].as_f64().unwrap() - f64::from(mean)).abs() < 0.001, "{summary}");
    assert_eq!(upgrades_summary(&[])["level_mean"], 0.0);
}

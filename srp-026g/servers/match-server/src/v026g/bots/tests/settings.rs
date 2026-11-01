//! Every environment setting the real-map bench (`real_bot_cost`), its stand-in
//! game and its tallies read, in one place: the list below says what each is and
//! what it is when unset, [`BenchSettings`] reads the environment once, and the
//! bench's code asks it (`bench_settings().var("SRP_BENCH_SEED")`), never
//! `std::env`. A setting not in the list is a panic, so a new one is added here
//! first, with its default.
//!
//! Not the bench's, so still read where they are used: `SRP_SOAK_TRACE` and
//! `SRP_BOT_NOTES` (`soak.rs`), `SRP_RAID_SEEDS` and `SRP_NEAR_SEEDS` (`soaks.rs`),
//! `SRP_TRACE_DECISIONS` (`tallies/decisions.rs`).
//!
//! The accessors answer exactly what `std::env` did at the places they replace:
//! - [`BenchSettings::var`] is `std::env::var` (unset, or not UTF-8, is an `Err`),
//! - [`BenchSettings::var_os`] is `std::env::var_os`,
//! - [`BenchSettings::flag`] is `var_os(..).is_some()`: **set at all, even empty**
//!   (an empty `SRP_BENCH_X=` is a different cache key for `benchrun.sh`, and a
//!   different run here),
//! - [`BenchSettings::number`] is `var(..).ok().and_then(parse().ok())`: unset,
//!   empty or not a number is `None`.
//!
//! The game-model switches (they change what the stand-in game does, so what the
//! bots meet; `standin.rs` and `standin_jump.rs` read them) are marked **model**.

use std::collections::BTreeMap;
use std::env::VarError;
use std::ffi::OsString;
use std::sync::OnceLock;

/// One setting: its name, what it is when unset, and what it does.
pub(crate) struct Setting {
    pub(crate) name: &'static str,
    pub(crate) default: &'static str,
    pub(crate) what: &'static str,
}

const fn s(name: &'static str, default: &'static str, what: &'static str) -> Setting {
    Setting {
        name,
        default,
        what,
    }
}

/// The names of the plays', the reflex's and the senses' number lists
/// (`name=value,...`) and of the tuning's JSON patch (`TUNING`), read as `SRP_BENCH_<name>`, and for
/// team 1 of a one-sided run as `SRP_BENCH_SIDE1_<name>` (`tuning_from`).
pub(crate) const TUNING_NAMES: [&str; 16] = [
    "COLLECT",
    "INTERCEPT",
    "RAID",
    "BUDDY",
    "TRADE",
    "ROAM",
    "CUT",
    "HOLDEXIT",
    "CROSSFIRE",
    "PAIRUP",
    "RESUPPLY_PLAY",
    "COMBAT",
    "HEAL",
    "AIM",
    "SENSE",
    "TUNING",
];

pub(crate) const SETTINGS: &[Setting] = &[
    // What is run.
    s("SRP_MAPDATA_DIR", "unset: the bench is skipped", "the folder of the map dumps"),
    s("SRP_BOT_ROUTES", "unset: no human routes", "the routes table (JSON)"),
    s("SRP_BOT_FOLIAGE", "unset: bot-foliage.json of the mapdata folder, if any", "the bushes a sight line crosses (JSON, scripts/foliage-volumes); empty or `off`: none"),
    s("SRP_BOT_GRASS", "unset: bot-grass.json of the mapdata folder, if any", "the grass a sight line crosses (JSON, scripts/foliage-volumes --grass); empty or `off`: none"),
    s("SRP_GAME_DATA", "unset: the built-in test archive, said in a loud line", "the server's game data (`server-game-data.db`) the run is on, read and checked as srp reads it (`game_data::init`); a file that fails refuses the run"),
    s("SRP_BENCH_MAPS", "6/2, 1/4, 3/3 (level/mode)", "the maps: `level/mode,...`"),
    s("SRP_BENCH_SIDES", "4,8,10", "the sizes, bots a side: `n,...`"),
    s("SRP_BENCH_SECONDS", "240", "the length of a match, s"),
    s("SRP_BENCH_SEED", "0", "another match of the same map and size (0: the digests' runs)"),
    s("SRP_BENCH_TIER", "unset: the lowest tier", "every bot's tier, `4-5`, `9-10` or `13-15`: each bot is the lobby's (its role from `bot_roles`, the tier's kits of `bot_profile`), read back as the match server does (`bench_roster`)"),
    s("SRP_BENCH_MASKS", "off", "**model** `top`: every bot wears the respirator the top players of its tier wear (`loadouts::Masks`, `BotTier::top_mask`: `ren_mask_3` at 9-10, `edge_resp_3` at 13-15, none at 4-5), as the lobby's `SRP_PVB_MASKS`; changes the kits the bench's bots carry and so their armour against a damage zone"),
    s("SRP_BENCH_RUSHER", "control", "the kit the rusher bots of tiers 9-10 and 13-15 wear (`loadouts::RusherKits`, the lobby's `SRP_PVB_RUSHER_KIT`): `control` (the old kit: the SPAS-12 with buckshot; the default), `smg` (`sr2m` / `fn_p90`), `alt` (`ump45` / `kriss_vector`), `slug` or `helmet` (the SPAS-12 with slugs / buckshot and the SMG kit's helmet); one word for both tiers or `9-10=alt,13-15=smg`. A kit whose items the game data lacks is the old kit, said in a line (the built-in test archive has no `fn_p90` or `kriss_vector`: set SRP_GAME_DATA); the rusher's armour and ammo in the stand-in's hits follow it"),
    s("SRP_DECISIONS_OUT", "unset", "a file the run's decisions line is appended to"),
    // What is printed or written.
    s("SRP_BENCH_EVENTS", "unset", "a folder for the run's events, one JSON line each"),
    s("SRP_BENCH_SUMS_DIR", "unset", "a folder for each cell's `sums` JSON, one file a cell (`<map>-<n>v<n>-s<seed>.json`): logs of builds before `say_whole` (bench.rs) have the `sums:` line cut in two by libtest's \"has been running for over 60 seconds\" line when a cell is slow"),
    s("SRP_BENCH_NOTES", "unset", "a file the carry tally's notes are appended to"),
    s("SRP_BENCH_GATES", "off", "test every fire gate each tick, for the gate tally's counts (no shot changes)"),
    s("SRP_WRITE_GOLDEN", "off", "a path: the made-up match test writes its counts there instead of comparing (tallies/owner/golden.rs)"),
    s("SRP_BENCH_SHOW_STALE", "off", "print the stale-belief lines"),
    s("SRP_BENCH_SHOW_RAIDS", "off", "print the raid lines"),
    s("SRP_BENCH_WHY", "off", "print why each play ended"),
    s("SRP_BENCH_JUMP_ROWS", "off", "set: one `jumprow` line per route jump (the link's rise, sprint and speed the tick before the press, a battery carried, where the honest stand-in came down and with which clip); tallies/owner/jumps.rs"),
    s("SRP_BENCH_JUMP_TRACE", "unset", "`x,z;x,z;...` with `SRP_BENCH_JUMP_ROWS`: `jumptrace` lines of the presses at those take-offs (within 0.6 m), 1 s before to 2 s after, the first 6 failed and the first 6 up at each a run"),
    s("SRP_BENCH_CARRIER_JUMP", "off", "`on` (or `1`) plans a battery carrier's routes over the jump links up to the standing jump's 0.998 m only (`Feel::carrier_jump_m` Some); off by default: every jump link"),
    s("SRP_BENCH_REST_OFF_LIP", "off", "`on` (or `1`) moves a walk's goal that lies on a lip (or within the body's radius of one) inward to the nearest floor off the edge (`ThinkTuning::rest_off_lip`); off by default: the goal as the play gave it"),
    s("SRP_BENCH_EXACT_EROSION", "off", "`on` (or `1`) takes, after the cell rounds, the floors nearer to a wall than the body's radius, measured exactly (`mapdata::Erosion::Exact`), `GridTuning::exact_erosion`; off by default: the cells' erosion"),
    s("SRP_BENCH_CLIENT_NUMBERS", "off", "`on` (or `1`) builds the maps' grids with the numbers the 0.26g clients run (step 0.8, ground walked up to 55 degrees: `mapdata::Body::client`), `GridTuning::client_numbers`; off by default: the exe's 0.6 and 50 degrees"),
    s("SRP_BENCH_BOT_STAMINA", "off", "`on` (or `1`) lets the bots' reckoned stamina bar (`stamina::Reckoning`, the client's law from the keys they sent) steer them: no sprint at an empty bar, a sprint dropped early enough to pay a jump ahead (`ThinkTuning::stamina`); off by default: counted only"),
    s("SRP_BENCH_HONEST_DROPS", "off", "`on` (or `1`) keeps a Drop link of the maps' grids only where the body that walks off the edge at 3.5 m/s comes down on a floor within the harmless fall of 5 m (`GridTuning::honest_drops`, `mapdata::Body::drop_flight_mps`); off by default: the cell model, a chain of harmless steps is a chain of drops"),
    s("SRP_BENCH_RECKON_JUMP_CAP", "off", "`on` (or `1`) makes the reckoned body take a jump up a jump link only if the jump could make its rise: the plain jump's 1.09 m, or with forward and sprint held at the walk cap or more the sprint jump's 1.28 m; `never` keeps it on the take-off side until a world shows it on the landing floor (`Kinematics::jump_snap`); off by default: any jump link"),
    s("SRP_BENCH_DROP_WALK", "off", "`on` (or `1`) lets go of the sprint before the edge of a planned drop (a lip's height or more) that a sprint would carry past the harmless fall, so it is walked off, not carried into the air; a drop harmless at a sprint is sprinted off (`Feel::drop_walk`); off by default"),
    s("SRP_BENCH_BAND_JUMP_GATE", "off", "`on` (or `1`) presses a sprint-band ledge jump (a link the grid labels a sprint jump, `Feel::band_from_grid`) only with the sprint held and the speed at 4 m/s or more, also at the take-off; a slower arrival backs off for another run-up, and a link with no clear 1.5 m run is given up; the lining up of a sprint ledge or a sprint gap link keeps the sprint, which the driver let go of (`Feel::band_jump_gate`); off by default"),
    s("SRP_BENCH_SPRINT_GAPS", "off", "`on` (or `1`) runs up to a sprint gap link (`Leap::SprintGap`) at a sprint, the run turning onto the jump's way, and presses it at speed (`Feel::sprint_gaps`); off by default: a walking gap jump"),
    s("SRP_BENCH_STRANDED_START", "off", "`on` (or `1`) lets a body that stands on no floor of the grid (on the top of an object) plan from the floor it drops to, up to 5 m below (`ThinkTuning::stranded_start`); off by default: it has no route"),
    s("SRP_BENCH_UNCLEAR_AIM", "off", "`on` (or `1`): a walk whose look-ahead finds no point of the route it walks to in a line steers at a clear one back of its progress, or plans again (`Feel::unclear_aim`); off, it steers at the unclear point"),
    s("SRP_BENCH_FLOOR_HEIGHT", "off", "`on` (or `1`): the walk's tests of the ground ask at the route's floor heights, not at the line between each leg's ends (`Feel::floor_height_clear`)"),
    s("SRP_BENCH_DROP_TRUTH", "height", "`links` makes a route's drop the planner's Drop link for every reader of the route's drops (the lip guard's planned landings, the drop aim, the landing watch, the drop walk), from the client's step height on and with no stair or ramp; `height` (the default) is a waypoint a lip's height under the one before it (`Feel::drop_truth`)"),
    s("SRP_BENCH_LIP_GUARD", "off", "`on` (or `1`) turns the lip guard and the short look-ahead on lip floors on (`Feel::lip_guard_ticks` 2, `lip_look_ahead_m` 0.8); both are off by default"),
    s("SRP_BENCH_FALLS", "off", "print a line for every fall of a metre or more (FALL unplanned, PLANNED on the route), the bots' reckoning error and their distance off the route (`tallies/route.rs`)"),
    s("SRP_BENCH_STUCK_EPISODES", "off", "`on` (or `1`): print a `STUCKEP` line for every stuck episode of a bot: where, its team, how it ended, at a lip or not, and run-length strings of the lip guard's verdict, its watch and the walk over the 3 s before and the episode (`tallies/owner/moving.rs`)"),
    s("SRP_BENCH_FWD", "unset", "set: print a `FWDREL` line for every route jump pressed with forward held whose forward key goes up within 5 ticks: the play, the rise, what the route has left past the landing, the bot's walk state and turn over the next ticks (`tallies/owner/jumps.rs`)"),
    // The bots' side: tuning of the bench's bots.
    s("SRP_BENCH_FALL_LOCK", "off", "`on` (or `1`) keeps the stuck watch quiet for 8.5 s after a body landed from a fall of 6 m or more (`ThinkTuning::fall_lock`): such a body cannot move (the J0 worlds), and the watch took it for stuck"),
    s("SRP_BENCH_CHEAT", "off", "the battery cheat instead of the fair battery knowledge"),
    s("SRP_BENCH_BATTERY_SIGHT_M", "the tuning's", "SenseTuning::battery_sight_m"),
    s("SRP_BENCH_FRIEND_NEAR_M", "the tuning's", "FriendlyFireTuning::near_m"),
    s("SRP_BENCH_LANDING_CANCEL_DEG", "the feel's (0: off)", "Feel::landing_cancel_deg: a route jump's landing run more than this many degrees from both the route's next leg and the goal, or into a wall within its coast, is braked (60 is a guess); the landings are counted either way"),
    s("SRP_BENCH_LANDING_CANCEL_DROPS", "off", "`on` (or `1`) lets the landing cancel (`SRP_BENCH_LANDING_CANCEL_DEG`) brake the landing run of a planned drop too, watched from the take-off (`Feel::landing_cancel_drops`); the drop landings are counted either way; off by default"),
    s("SRP_BENCH_FF_LEAD", "the tuning's", "FriendlyFireTuning::lead: `0` off, else on"),
    s("SRP_BENCH_FLOOR_PATHS", "off", "Tuning::grid.floor_paths (off by default): any value but `0` (e.g. `1`) makes every play's path distance the regions' walk between floors a storey apart; off, the straight ground distance (Collect's batteries a storey off keep the walk)"),
    s("SRP_BENCH_FF_PAST_M", "the tuning's", "FriendlyFireTuning::past_target_m"),
    s("SRP_BENCH_FF_TOUCH_M", "the tuning's", "FriendlyFireTuning::touch_m"),
    s("SRP_BENCH_FF_LEAD_MS", "the tuning's", "FriendlyFireTuning::lead_extra_ms"),
    s("SRP_BENCH_THREAT", "the tuning's (on)", "ThreatTuning::spots: `0` off (no threat spots, nothing kept out of a shooter's line), else on"),
    s("SRP_BENCH_THREAT_DETOUR", "the tuning's (on)", "ThreatTuning::detour: `0` off (a route through a shooter's lane is walked as it is), else on"),
    s("SRP_BENCH_THREAT_FLOORS", "the tuning's (3000)", "ThreatTuning::search_floors: floors of a poll's budget one search round a shooter's lane may take"),
    s("SRP_BENCH_THREAT_SPRINT", "the tuning's (on)", "ThreatTuning::sprint: `0` off (a bot in a shooter's lane keeps the pace its play chose), else on"),
    s("SRP_BENCH_ESCAPE_CEILING", "on", "Tuning::hazard.escape: a way out of a zone stays under the strength of its start's; `0` the nearest way over any zone"),
    s("SRP_BENCH_PERCH", "the tuning's (on)", "PerchTuning::on: `0` off (the hold plays pick no perch of the routes table), else on"),
    s("SRP_BENCH_PERCH_P", "the tuning's (1)", "PerchTuning::p_scale: a multiplier on the share of hold picks that take a perch (the sweep: 1 and 2)"),
    s("SRP_BENCH_LIVE_ZONES", "off", "Tuning::hazard.live_zones: the anomaly fields of the worlds are the bots' live zones (kept off while a field hurts, got out of); `0` off"),
    s("SRP_BENCH_SEARCH_FLOORS", "the tuning's", "floors a poll's searches expand: a number or `max`"),
    s("SRP_BENCH_ONE_SIDE", "off", "team 1 plays by the SRP_BENCH_SIDE1_* numbers, so a change to team 0 alone shows"),
    s("SRP_BENCH_PING", "unset: none", "a ping for each place, `lo-hi` ms"),
    s("SRP_BENCH_SIGHT_REFRESH", "off", "`on` (or `1`) turns the whole sight refresh on (`SenseTuning::sight_refresh`, with the hold of the cheat, `CheatTuning::sight_hold_ms`, at 450 ms to the bot's next think); off by default, and the bots are then the tip's"),
    s("SRP_BENCH_SIGHT_HOLD", "unset: the hold the refresh setting gives (none without it)", "how long a sight that is not the target's goes on with no test (`CheatTuning`), with the refresh on: `think` (to the bot's next think and a tick, capped at the untested bound), `0` (none), or a number of ms (fixed, from the test; 250 and 450 are the A/B)"),
    s("SRP_BENCH_PING_JITTER", "unset: none", "each place's ping off by a bounded random walk within this many ms either way, a step of at most 14 ms a tick (the live p99 of the lateness's change), seeded by the seed; around none with no SRP_BENCH_PING. Not a live model: the ping is the lateness itself (one way, about twice as bad as a live round trip), and a lateness above the relay's reach (425 ms less twice the largest other ping) counts as discarded, no error"),
    s("SRP_BENCH_WEAPONS", "on (live)", "every place's weapon known as a live match knows it (play 11's matchup, 'outranged by a seen sniper'): the server sets it for every bot (`game/driven.rs`); `off` (or `0`) leaves the bots without it"),
    s("SRP_BENCH_HUMAN", "unset", "the place the plays take for a human (CG-12)"),
    s("SRP_BENCH_HUMAN_TRAPS", "live", "live: the stand-in human carries a trap in the live share of lives (2.6 %) and puts it down as the live players do (standin_trap_habit.rs); carrier: every life is a carrier's (more traps, the same life); off: none"),
    s("SRP_BENCH_HUMAN_TRAPS_KNOWN", "off", "off (live): a trap the stand-in human puts down is not in his team's trap list; his team learns it the way a live team learns a human mate's trap, once seen (within 8 m, in the cone, a clear line: fx-gear A168, LIKELY: the exe builds the trap model for every client of the match, no HUD or minimap marker tells of it); on: it is in the list from the tick it is laid (`Bots::bench_team_knows_trap`), more than a live team has, for a bench of how bots meet a known trap"),
    s("SRP_BENCH_UPGRADES_ONLY", "both", "**model** `0` or `1`: only that team keeps the process's `SRP_BOT_UPGRADES` (the armour modifications, and the weapon the client fires with the skills' reload and aim time); the other team is built as if they were off (its profiles, the arms of its kits, the stand-in's stamina rates and ammo); `both` (unset): both teams as the process. With it the `carrier_deaths` key of the sums splits the carriers' deaths by the killer's and the carrier's upgrade state (C14: own side or enemy side)"),
    s("SRP_BENCH_STAND_IN_LEVEL", "on (live)", "**model** `on` (or `1`, the default): every stand-in body moves at the movement level of its kit (`bot_skills::profile_movement_level`, 1 + 0.05 - 0.2 x weight cost: about 1.03-1.04 on average, 0.85-0.92 for the heavy kits), and the bots' tracker starts at the same level, as the live server hands it (`game/driven.rs`); the stand-in human goes at `HUMAN_LEVEL` (1.02, PROVEN: the median of 841 live bodies' sprint speed over the nominal); `off` (or `0`): every body at the nominal speed, as before the default changed (C14)"),
    s("SRP_BENCH_STAND_IN_LIFT", "on", "**model** `off`: the stand-in body does not lift (the client's sprint lift, `standin_jump.rs`): a sprint run into the face of a jump link up of 1.11 m or less, a top 0.35 m deep, held 0.3 s at the face, takes the body up with no jump; off, only a jump gets it up, as before the lift"),
    s("SRP_BENCH_SPAWN_POINTS", "free", "**model** `random`: a spawn goes to a random point of the team with replacement and 2 m of jitter (the old stand-in); free: onto a free respawn point of the team, exactly on it, as the game's respawn rule does (`select_respawn_points` 0x3171D0; live 0 of 790 fresh pairs within 1 m)"),
    s("SRP_BENCH_STAND_IN_EDGE", "on", "**model** `off`: the stand-in body falls at a ledge as the lattice has it, at the last floor cell's boundary; on (the client's capsule, `body::SUPPORT_M`): it is held until its feet are 0.435 m past that floor's centre (`bullet_character_controller::step_down` grounds a capsule while its hemisphere reaches the edge)"),
    s("SRP_BENCH_STAND_IN_FALL", "on", "**model** `off`: a stand-in body held past a ledge's edge moves on only toward a drop link of its floor or along a lip's floor (it can stand there pushing); on (with `SRP_BENCH_STAND_IN_EDGE`): it walks on while a floor of its level within `body::SUPPORT_M` holds it and falls straight down once none does, link or no link, as the client's capsule (`standin_jump::past_edge`)"),
    s("SRP_BENCH_STAND_IN_CORNER", "on", "**model** `off`: the stand-in body slides along every wall of the lattice at full speed; on (the client's capsule, `standin_capsule.rs`): a circle of the capsule's radius 0.38 m in the walker mesh's walls, moved by `step_forward_impl`'s wall slide with the live `cc_wall_full_slide_dot 0.50` and `cc_wall_slide_bouncing_factor 0.08`: held off a wall, a wall met within 60 degrees of its normal takes speed, a jamb met head on stops the body"),
    s("SRP_BENCH_STAND_IN_SLIDE", "off", "**model** `on`: a stand-in body on ground the client slides it off slides down the lattice, its lowest walk or drop neighbour at a time, gaining 9.8 x sin(slope) m/s a second, until its ground holds it (`standin_slide`, C14): by-slope ground over 55 degrees, over 45 degrees when it stands still (and while it slides), `dont_walk` ground always, `walk` ground never (the client's `need_slide_on_supporting_surface` 0x7bc20 and the down sweep's callback 0x84420); off: it stands where it stops. The `still_on_steep` line of the log counts the still ticks on such ground either way"),
    s("SRP_BENCH_STAND_IN_JUMP_WALL", "off", "**model** `on` (with `SRP_BENCH_STAND_IN_CORNER=on`): the walk a jump makes on the floor before it leaves it is held by the walls and the ledge as the walking body is (the capsule rule, then `slide_supported`); off: it walks as the lattice (`slide_capped`), through a wall the capsule holds and off a ledge the capsule stays on, so a body held at a wall that presses jump walks through it and drops (A199: Chemical Plant day 4's pipe-top ledge under the side-1 platform). The count of the walk ticks the capsule would hold is in the `climbs` line of the log either way"),
    s("SRP_BENCH_STAND_IN_AIR", "on", "**model** `off`: the stand-in's jump goes only the way of the keys at the press (the benches before #966: their jumps are not comparable with the later ones); on (the client's air control, `standin_jump::air_control`): each tick of a stand-in jump off the floor, the keys held then move the body 1.0 m/s their way (none with forward alone in a sprint jump), as `get_air_control_vector` 0x2D7990 does"),
    s("SRP_BENCH_HUMAN_SMOKE", "on", "off: the stand-in human throws no smoke (otherwise at the live rate, standin_smoke.rs)"),
    s("SRP_BENCH_SMOKE_FAIR_DUMP", "unset", "set: each sighting the smoke fairness counter counts is printed (`FAIR-SEEN`, tallies/owner/smoke.rs), with the clouds near it at the counter's tick and at the sensing's time"),
    s("SRP_BENCH_CAMPER", "unset", "the place of the stand-in camper"),
    s("SRP_BENCH_ENEMY_HUMAN_LANES", "on", "**model** `off` (or `0`): none. `on` (or `1`, `both`): every body of the other team counts as a human in the team's lanes of enemy humans (`know::HumanLanes`), for both teams; `team1` (or `team0`): for that team's bots only. Player vs Bots has its one human on team 0, so `team1` is that match (both teams placing traps counted them twice, an-gear-moves 3.3); only the lanes read it (the bear-trap habit's score needs them: a bench of bots alone has no human)"),
    s("SRP_BENCH_CAMPER_SIDE", "given", "**model** `mirror`: the camper stands on the other team, in the same role's place (place + team size, or back); `seed`: on the given place on an even seed, on its mirror on an odd one, so that a run's cells (and both arms of a head to head, which share seeds) split between the teams. Default: the team of the place named, which is team 0 for a place under the team size, so a by-side split of a camper cell is lopsided (only the pooled A+B cancels it)"),
    s("SRP_BENCH_CAMPER_RETURN", "always", "**model** where the standing camper (no `SRP_BENCH_CAMPER_MOVE`) goes after a death: `always` back to the very point at once, every life (the old stand-in, which a trap on the point catches each time); `live` as the live campers of 6+ player matches (A145, `tests/camper_move.rs` `LiveReturn`): back to the very point 21.5 % of the time and to a point of its area 3-8 m off 28.8 %, each after a wait drawn about the live median (46 s and 40 s from the death) during which he plays as any bot, else at once to another spot 25 m or more away"),
    s("SRP_BENCH_CAMPER_SPOT", "most", "**model** `hidden`: the camper's spot is one that sees many route points of the other side and is seen from under half of them (else the one that sees the most)"),
    s("SRP_BENCH_CAMPER_CROUCH", "unset", "**model** `1`: the camper crouches once at its spot (and picks it by the crouched eye and body)"),
    s("SRP_BENCH_CAMPER_MOVE", "unset", "**model** `1`: the camper works an area as a human does (an-anticamper §8.4, `tests/camper_move.rs`): the spot and up to three cover-side points 3-8 m from it, a peek out 0.4-0.45 s every 0.8-1.2 s while an enemy is known, a step to another point 2.5-3.5 m after a kill and 2-2.5 m after a hit, short crouch spells, and he leaves 3-5 s after a stay's second kill with no new one or after the dwell with fewer, to the next best spot 25 m or more away; off: he stands within 0.7 m of one point all match"),
    s("SRP_BENCH_VIEW_ERROR", "off", "**model** `1`: every world fix the bots are given is moved by the error the live picture has (an-sight §8.1, `tests/view_error.rs`): drawn from the measured quantiles by what the body was doing (still, walking, sprinting, crouched, a height change, a jump) and the fix's age, in a direction drawn evenly, and carried by the reckoning to the next fix. It makes the bots' picture of a body say hidden in about a fifth of the moments it is visible and visible in about a fifth of those it is hidden (the `view_flip` counts of the owner object). Off until those counts show it near the live 22 % / 20 %"),
    s("SRP_BENCH_VIEW_ERROR_SCALE", "0.7", "with `SRP_BENCH_VIEW_ERROR`: a multiplier on every error; 0.7 puts the flip counts at the live 22 % / 20 % with the bench's pings (the pings alone give 9 %)"),
    s("SRP_BENCH_CAMPER_AT", "unset: any route point of his side", "**model** where the camper's spot is chosen, among the route points of his side that see the other side: `battery` within 25 m of a battery lying on the map (the camper by an objective, an-anticamper RC3); `container` within 25 m of his own container; `exit` 20 to 45 m from the other team's nearest spawn (outside their spawn area, which no route enters; where the live spawn campers stood: median 35 m from the victim's spawn place), the one seen from in the narrowest sector (a spawn exit that cannot be flanked: every sight on him comes from one way); none near: no camper"),
    s("SRP_BENCH_CAMPER_START", "spawn", "**model** `spot`: each life of the stand-in camper starts on his spot (`SRP_BENCH_CAMPER_AT`), so a cell measures what his victims do (a spawn exit held, design-p1c §2), not his walk across the map; default: on a spawn point of his team, as every body"),
    s("SRP_BENCH_CAMPER_DWELL_S", "22.5", "with `SRP_BENCH_CAMPER_MOVE`: the seconds he stays at a spot with fewer than two kills (20-25 s in the live holds), drawn within 10 % of this"),
    s("SRP_BENCH_CAMPER_WATCH", "around", "**model** `lane`: the camper holds its view on the lane it watches (the stretch of the other side's route points its spot sees where most lie, 30 degree window), scoped, while at rest in its cover hold, as a live sniper does (A182, `tests/standin_watch.rs`); the live snipers already scoped and looking shoot at 44 % of the crossings, the first shot 1.65 s after the enemy shows; `around`: it stands about, unscoped"),
    s("SRP_BENCH_CAMPER_GUN", "unset", "**model** `long`: the camper carries the long gun (bolt, else marksman rifle) of the bench tier's kits"),
    // The game model (the stand-in's rules).
    s("SRP_BENCH_USE_LINE", "off", "**model** a use needs a clear line to the battery"),
    s("SRP_BENCH_TAKE_M", "BATTERY_M (0.2)", "**model** how near a battery's middle a take's ray must pass, m"),
    s("SRP_BENCH_UNTAKEABLE", "0", "**model** the first n batteries no use takes off the floor (audit #7)"),
    s("SRP_BENCH_CAMP", "unset", "**model** a camping carrier: the first of team 0's to come within this of his container, m, stops there (audit #6)"),
    s("SRP_BENCH_NUDGE", "unset", "**model** every NUDGE_EVERY_MS one lying battery moves this far, m, unseen (audit #7)"),
    s("SRP_BENCH_SPREAD", "off", "**model** shots spread (bench_spread.rs)"),
    s("SRP_BENCH_BATTERIES", "unset: one at every spawner", "**model** batteries lying at start, at spawners drawn at random (the mode's count is 10)"),
    s("SRP_BENCH_START", "0,0", "**model** batteries already in team 0's and team 1's containers: `a,b`"),
    s("SRP_BENCH_LIVE_FIELDS", "off", "**model** `<n>`: the generator of anomaly fields makes the release's rule, at most n alive at once (the release: 16): charging 20 s, damaging 4 s, then holding; they hurt the bodies whatever the bots know (`standin_fields.rs`)"),
    s("SRP_BENCH_FIELD_TAKER", "none", "**model** `scripted`: a field's artefact is taken 30 s into its holding (the field goes, its place rests 60 s); `none`: nobody takes one, as in a bot match"),
    s("SRP_BENCH_FIELD_WORLD_MS", "1000", "**model** how often a world lists the fields to the bots, ms (a lag arm: 5000)"),
    s("SRP_BENCH_FIELD_PLACES", "spread", "**model** `lanes`: the generator's places lie on the walks between the containers and the battery spawners, where the bots go"),
    s("SRP_BENCH_FIELD_PACE", "release", "**model** `fast`: the first try at 5 s, then every 4-8 s (a stress arm)"),
    s("SRP_BENCH_FIELD_RADIUS_M", "3", "**model** the fields' radius, m"),
    s("SRP_BENCH_BAKE_FIELD_PLACES", "off", "**cost only** `<n>`: n always-on damage zones baked at the generator's places (the alternative to the live board, for what it costs)"),
    s("SRP_BENCH_ZONE_POOLS", "raw", "Tuning::hazard.zone_pools: `routed` weighs a battery's crossing of a damage zone against the pools its hits take from (infection, pain, body, head) through the bot's armour on the parts hit, and the stand-in's zone damage goes to those pools; `raw` (the default), the body pool and the raw hit"),
    s("SRP_BENCH_ROUTE_CROSSING", "off", "Tuning::hazard.route_crossing: `1` offers a battery in zones on all match that no straight walk crosses to when the motor's own route over the zones' floors is borne (and the motor walks it); off (the default), only the straight crossings"),
    s("SRP_BENCH_ZONE_DAMAGE", "off", "**model** zones hurt the stand-in's bodies; `1` only (a GUESS of the game's)"),
    s("SRP_BENCH_ZONE_INTENSITY", "on", "**model** a zone's hit on the stand-in's bodies is the client's: `max_hit x intensity x 1000 / interval_ms` a second, the intensity the zone's hit curve at the body's place in the zone (bots in live lose exactly that, PROVEN; `mapdata::Damage::rate_at`); `0`: the zone's full hit wherever a body stands in it (the stand-in before)"),
    s("SRP_BENCH_HIT_FLIGHT", "on with the shapes", "**model** (A182) `0` turns it off; else, with `SRP_BENCH_HIT_MODEL` `on`, `boxes` or `scatter=<m>`, a bullet flies the client's path (`bullet::tick` 0x32C180: the muzzle speed, the drag, gravity; ammo_sp5 280 m/s takes 155 ms to 40 m and falls 0.5 m by 80 m) in 10 ms steps and finds each body where its last step carries it by then; `0`: the shot is an instant ray at where the bodies stood at the shot (the old hit model: 26 % of the bots' shots hit against the live 10.5 %) `still`: **diagnosis** the client's path (the flight's time and drop) against the bodies where they stood at the shot, to read what part of the flight's loss is the bodies moving"),
    s("SRP_BENCH_HIT_ARMS", "posed", "**diagnosis** (A182) `away`: with `SRP_BENCH_HIT_MODEL=on` the pose the shots are tested against has no forearm or hand shape (13 of the 19), to read how much of the bench's hit rate the arms in front of the chest make (52 % of its hits land on an arm against the live 23 %); `posed`: the table's pose"),
    s("SRP_BENCH_HIT_MODEL", "off", "**model** `on`: the stand-in's hit cut (`standin_hitbox.rs`), all together: a shot is a ray against the 19 hit shapes of the 0.26g player (the shape gives the hit box), a shotgun's ten pellets are rays in the ammunition's cone, and the line is moved across by the live position error (a Gaussian of 0.17 m per axis); `off` (unset): the nearest chest within 0.45 m of the line, a shotgun's centre ray, no scatter (the stand-in before); `boxes`: the shapes and the pellets without the scatter; `scatter=<m>`: another sigma"),
    s("SRP_BENCH_DAMAGE", "per weapon", "**model** `flat`: every stand-in hit takes a fifth of the health and nothing regenerates (the old stand-in); else the weapon's damage against the target's kit (`standin_damage.rs`)"),
    s("SRP_BENCH_GRENADE_DAMAGE", "on", "**model** `off`: a grenade's blast hurts nobody (only counted, as before `standin_grenade.rs`)"),
    s("SRP_BENCH_STOCK", "on", "**model** `off`: the stand-in's bodies fire whenever the fire key is down (no rounds, reloads or finite ammo: the stand-in before `standin_weapon.rs`); the counts stay"),
    s("SRP_BENCH_RESUPPLY", "on", "**model** `off`: the resupply box does nothing (the stand-in before `Stock::resupply`); the counts stay at zero"),
    s("SRP_BENCH_STAMINA", "off", "**model** `on`: the stand-in's bodies have the client's stamina bar (the sprint lockout, the jump's cost, a hit's stamina damage: `standin_stamina.rs`); off, they sprint and jump whenever their keys ask and the counts stay at zero"),
    s("SRP_BENCH_PITCH_SCALE", "client in a bench run (the game's), radian in a unit test", "**model** `radian`: the view's pitch unit is a radian in the stand-in (the stand-in before `pitch.rs`); `client`: 60.0 degrees up and 67.48 down, so a bot's shot and eye go where the Item bone does for the pitch it holds"),
    s("SRP_BENCH_PUT_MS", "PUT_MS (600)", "**model** a put-in lands this long after the press, ms"),
    s("SRP_BENCH_DRAW_TIME", "weapon (live)", "**model** `weapon`: a carrier's weapon fires again its own `show_time` after he drops a battery (0.5 s for eight weapons, 1.0 s for two) instead of 800 ms for every weapon, which any other value gives (`off`: the stand-in as it was; standin.rs `draw_ms`)"),
    s("SRP_BENCH_BODY_BLOCK_M", "0.76 (live)", "**model** enemy bodies block each other on the ground within this distance, m (0.76 in the game, PROVEN from the J0 matches' bodies files; friends never block); `off` (or `0`): bodies pass through each other"),
    s("SRP_BENCH_MAPDATA_OLD", "refuse", "`allow`: a run may read map data older than the format with the bullet armor (`mapdata::format::FIRST_WITH_ARMOR`, 5): every wall of such a file stops every bullet, so the pierce rule never opens a line; by default such a run is refused, naming the file. The format of the files is in the `data files:` line either way"),
    s("SRP_BENCH_JUMP_AHEAD_M", "STANDING_JUMP.ahead_m", "**model** how far ahead a standing jump reaches, m"),
    s("SRP_BENCH_STAGGER_MODEL", "`on`", "**model** `on`/`off`: a hit slows the stand-in by its weapon's `stopping_power` times the ammunition's `k_power`, the last 0.5 s summed and capped at 0.5, and stops its sprint (the game's rule, standin_stagger.rs); `off`: every hit slows it to 0.75 for 0.5 s (the stand-in before)"),
    s("SRP_BENCH_HEAL_MODEL", "follows `heal.on` of the run's tuning (off by default)", "**model** `on`/`off`: the stand-in has the drugs and their stock or not (standin_heal.rs)"),
    s("SRP_BENCH_DRUGS", "rev40 (live)", "**model** with the drugs on, `rev40`: a bullet that took health stops every drug in use (patch 84) and the Medikit takes 40 % of the pain (revision 40); `rev39`: neither, the drugs as they were before revision 40 (standin_heal.rs `rev40`)"),
    s("SRP_BENCH_TRAPS_MODEL", "on", "**model** `on`/`off`: the stand-in has the bear traps (placing, the sensor, a shot or a use that disarms) or not (standin_traps.rs); with no body carrying a trap it changes nothing"),
    s("SRP_BENCH_STAND_IN", "honest", "**model** `old`: the old lift up any floor in reach instead of the client's jump clips"),
];

/// Settings that shape the stand-in's world or the bots' roster though they are not tagged `**model**`: the line of the
/// bench's models names them too.
const WORLD_SETTINGS: [&str; 5] = ["SRP_BENCH_PING", "SRP_BENCH_WEAPONS", "SRP_BENCH_TIER", "SRP_BENCH_RUSHER", "SRP_BENCH_ONE_SIDE"];

/// The `bench models:` line: every `**model**` setting of [`SETTINGS`] (and [`WORLD_SETTINGS`]) with the value this run
/// has: the one in the environment, else its default as the table words it. Read from the table, so a setting added
/// there shows here with no second list.
pub(crate) fn models_line(settings: &BenchSettings) -> String {
    let mut parts = Vec::new();
    for setting in SETTINGS {
        if !setting.what.starts_with("**model**") && !WORLD_SETTINGS.contains(&setting.name) {
            continue;
        }
        let name = setting.name.strip_prefix("SRP_BENCH_").unwrap_or(setting.name);
        match settings.var_os(setting.name) {
            Some(value) => parts.push(format!("{name}={}", value.to_string_lossy())),
            None => parts.push(format!("{name}=({})", setting.default)),
        }
    }
    format!("bench models: {}", parts.join(" "))
}

/// The settings a run has the value of a live match for when it does not say otherwise (`off` or `0` turns one off). Each
/// was off by default before, so a run that did not set them knew nothing of the enemy's weapon, let bodies pass through
/// each other, drew every weapon in 800 ms, took no enemy for a human and moved every body at the nominal speed; the phase-end
/// sweep set the first four itself.
/// - `SRP_BENCH_WEAPONS`: PROVEN, the server gives every bot its weapon class (`game/driven.rs`, `set_weapons`).
/// - `SRP_BENCH_BODY_BLOCK_M`: PROVEN, 0.76 m from the J0 matches' bodies files (`standin.rs`).
/// - `SRP_BENCH_DRAW_TIME`: PROVEN, the weapons' own `show_time` in the configs (`standin.rs`).
/// - `SRP_BENCH_ENEMY_HUMAN_LANES`: LIKELY, not exact: in a live Player vs Bots match only team 1's bots meet a human
///   (`team1`, `player_bots.md` section 8); `on` gives both teams the lanes, as the sweep did (sides swap in a head to
///   head, so no team has them alone).
/// - `SRP_BENCH_STAND_IN_LEVEL`: PROVEN, the server hands every bot its kit's movement level (`game/driven.rs`); the stand-in
///   human goes at 1.02, the median of 841 live bodies (`standin_level.rs`).
const LIVE_DEFAULTS: [(&str, &str); 5] = [
    ("SRP_BENCH_WEAPONS", "1"),
    ("SRP_BENCH_BODY_BLOCK_M", "0.76"),
    ("SRP_BENCH_DRAW_TIME", "weapon"),
    ("SRP_BENCH_ENEMY_HUMAN_LANES", "on"),
    ("SRP_BENCH_STAND_IN_LEVEL", "on"),
];

/// The read environment: each setting of [`SETTINGS`] (and `SRP_BENCH_<name>` and
/// `SRP_BENCH_SIDE1_<name>` of [`TUNING_NAMES`]) as `std::env::var_os` gave it.
pub(crate) struct BenchSettings {
    raw: BTreeMap<String, OsString>,
}

fn known(name: &str) -> bool {
    SETTINGS.iter().any(|s| s.name == name)
        || TUNING_NAMES.iter().any(|t| {
            name.strip_prefix("SRP_BENCH_SIDE1_") == Some(*t)
                || name.strip_prefix("SRP_BENCH_") == Some(*t)
        })
}

impl BenchSettings {
    fn names() -> Vec<String> {
        let mut names: Vec<String> = SETTINGS.iter().map(|s| s.name.to_owned()).collect();
        for t in TUNING_NAMES {
            names.push(format!("SRP_BENCH_{t}"));
            names.push(format!("SRP_BENCH_SIDE1_{t}"));
        }
        names
    }

    /// The process's environment, read once for every listed name, with the live value for each of [`LIVE_DEFAULTS`]
    /// that it does not set.
    pub(crate) fn from_env() -> Self {
        Self::from_getter(|name| std::env::var_os(name)).with_live_defaults()
    }

    /// Each of [`LIVE_DEFAULTS`] that is not set, set to its live value. `off` (or `0`) is how a run turns one off.
    fn with_live_defaults(mut self) -> Self {
        for (name, value) in LIVE_DEFAULTS {
            self.raw.entry(name.to_owned()).or_insert_with(|| OsString::from(value));
        }
        self
    }

    fn from_getter(get: impl Fn(&str) -> Option<OsString>) -> Self {
        let raw = Self::names()
            .into_iter()
            .filter_map(|name| get(&name).map(|value| (name, value)))
            .collect();
        Self { raw }
    }

    /// Settings given by hand (the tests of this file).
    #[cfg(test)]
    pub(crate) fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        Self::from_getter(|name| {
            pairs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| OsString::from(*v))
        })
    }

    fn get(&self, name: &str) -> Option<&OsString> {
        assert!(known(name), "{name} is not in the bench's settings list (tests/settings.rs)");
        self.raw.get(name)
    }

    /// `std::env::var(name)`.
    pub(crate) fn var(&self, name: &str) -> Result<String, VarError> {
        match self.get(name) {
            None => Err(VarError::NotPresent),
            Some(value) => value
                .clone()
                .into_string()
                .map_err(VarError::NotUnicode),
        }
    }

    /// `std::env::var_os(name)`.
    pub(crate) fn var_os(&self, name: &str) -> Option<OsString> {
        self.get(name).cloned()
    }

    /// Set at all, even empty: `std::env::var_os(name).is_some()`.
    pub(crate) fn flag(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// [`Self::flag`] for a setting that is on by default ([`LIVE_DEFAULTS`]): set, and not `off` or `0`.
    pub(crate) fn is_on(&self, name: &str) -> bool {
        self.get(name).is_some_and(|value| value != "off" && value != "0")
    }

    /// `var(name).ok().and_then(|v| v.parse().ok())`.
    pub(crate) fn number<T: std::str::FromStr>(&self, name: &str) -> Option<T> {
        self.var(name).ok().and_then(|v| v.parse().ok())
    }

    /// [`Self::number`], but a value that is set and not a number refuses the run instead of reading as unset.
    pub(crate) fn number_or_refuse<T: std::str::FromStr>(&self, name: &str) -> Option<T> {
        parse_number(name, self.var(name).ok().as_deref()).unwrap_or_else(|e| panic!("{e}"))
    }
}

/// Whose lanes of enemy humans count the other team as human (`SRP_BENCH_ENEMY_HUMAN_LANES`, **model**).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum EnemyLanes {
    #[default]
    Off,
    /// Both teams: every enemy of any bot is a human (the old `on`).
    Both,
    /// The bots of this team only. In Player vs Bots the one human is on team 0, so team 1's bots meet a human and team 0's
    /// meet bots: `team1` is that match; both teams placing traps twice a match was the over-count of an-gear-moves 3.3.
    OnlyTeam(u8),
}

/// A setting that is a number: unset is none, a value that is not a number is an error.
pub(crate) fn parse_number<T: std::str::FromStr>(name: &str, value: Option<&str>) -> Result<Option<T>, String> {
    match value {
        None => Ok(None),
        Some(text) => text.trim().parse().map(Some).map_err(|_| format!("{name}={text:?} is not a number")),
    }
}

impl EnemyLanes {
    /// `off` or `0` or unset: none; `team0` / `team1`: that team's bots; `on`, `1`, `both` or empty: both teams (the old `on`);
    /// anything else is refused (a typo such as `team2` used to mean both).
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.map(str::trim) {
            None | Some("off" | "0") => Ok(Self::Off),
            Some("team0") => Ok(Self::OnlyTeam(0)),
            Some("team1") => Ok(Self::OnlyTeam(1)),
            Some("on" | "1" | "both" | "") => Ok(Self::Both),
            Some(other) => Err(format!("SRP_BENCH_ENEMY_HUMAN_LANES={other:?}: off, team0, team1 or on")),
        }
    }

    pub(crate) fn from_env() -> Self {
        Self::parse(bench_settings().var("SRP_BENCH_ENEMY_HUMAN_LANES").ok().as_deref()).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Whether the bots of `team` take every body of the other team for a human.
    pub(crate) fn applies_to(self, team: u8) -> bool {
        match self {
            Self::Off => false,
            Self::Both => true,
            Self::OnlyTeam(only) => only & 1 == team & 1,
        }
    }
}

/// What a run does with a map data file written in `version`: `Ok(None)` reads it, `Ok(Some(warning))` reads it though it has
/// no bullet armor (`SRP_BENCH_MAPDATA_OLD=allow`), `Err` refuses it. A file before the armor stops every bullet at every
/// wall, so the pierce rule never opens a line and a bench on it measures a world no live match has.
pub(crate) fn mapdata_format_verdict(path: &std::path::Path, version: u32, allow_old: bool) -> Result<Option<String>, String> {
    let first = mapdata::format::FIRST_WITH_ARMOR;
    if version >= first {
        return Ok(None);
    }
    let what = format!(
        "{}: map data format {version}, which has no bullet armor (format {first} has): every wall stops every bullet, so the pierce rule never opens a line",
        path.display()
    );
    if allow_old {
        Ok(Some(format!("WARNING: {what} (SRP_BENCH_MAPDATA_OLD=allow)")))
    } else {
        Err(format!("{what}. Dump the maps again (mapdata-dump) or set SRP_BENCH_MAPDATA_OLD=allow to run on it anyway."))
    }
}

/// The bench's settings, read from the environment the first time they are asked.
pub(crate) fn bench_settings() -> &'static BenchSettings {
    static SETTINGS_READ: OnceLock<BenchSettings> = OnceLock::new();
    SETTINGS_READ.get_or_init(BenchSettings::from_env)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run that says nothing has the live value of each of the five; `off` or `0` turns one off; a value of its own wins.
    #[test]
    fn the_live_defaults_hold_unless_a_run_says_otherwise() {
        let unset = BenchSettings::from_pairs(&[]).with_live_defaults();
        assert!(unset.is_on("SRP_BENCH_WEAPONS"));
        assert_eq!(unset.number::<f32>("SRP_BENCH_BODY_BLOCK_M"), Some(0.76));
        assert_eq!(unset.var("SRP_BENCH_DRAW_TIME"), Ok("weapon".to_owned()));
        assert_eq!(EnemyLanes::parse(unset.var("SRP_BENCH_ENEMY_HUMAN_LANES").ok().as_deref()), Ok(EnemyLanes::Both));
        assert!(unset.is_on("SRP_BENCH_STAND_IN_LEVEL"));
        // A reading that does not go through `from_env` (the other tests of this file) has none of them.
        assert!(!BenchSettings::from_pairs(&[]).is_on("SRP_BENCH_WEAPONS"));
        let off = BenchSettings::from_pairs(&[
            ("SRP_BENCH_WEAPONS", "off"),
            ("SRP_BENCH_BODY_BLOCK_M", "off"),
            ("SRP_BENCH_DRAW_TIME", "off"),
            ("SRP_BENCH_ENEMY_HUMAN_LANES", "off"),
            ("SRP_BENCH_STAND_IN_LEVEL", "off"),
        ])
        .with_live_defaults();
        assert!(!off.is_on("SRP_BENCH_WEAPONS"));
        assert!(!off.is_on("SRP_BENCH_STAND_IN_LEVEL"));
        assert_eq!(off.number::<f32>("SRP_BENCH_BODY_BLOCK_M"), None);
        assert_ne!(off.var("SRP_BENCH_DRAW_TIME"), Ok("weapon".to_owned()));
        assert_eq!(EnemyLanes::parse(off.var("SRP_BENCH_ENEMY_HUMAN_LANES").ok().as_deref()), Ok(EnemyLanes::Off));
        let zero = BenchSettings::from_pairs(&[("SRP_BENCH_WEAPONS", "0")]).with_live_defaults();
        assert!(!zero.is_on("SRP_BENCH_WEAPONS"));
        let own = BenchSettings::from_pairs(&[("SRP_BENCH_BODY_BLOCK_M", "0.5"), ("SRP_BENCH_ENEMY_HUMAN_LANES", "team1")]).with_live_defaults();
        assert_eq!(own.number::<f32>("SRP_BENCH_BODY_BLOCK_M"), Some(0.5));
        assert_eq!(EnemyLanes::parse(own.var("SRP_BENCH_ENEMY_HUMAN_LANES").ok().as_deref()), Ok(EnemyLanes::OnlyTeam(1)));
    }

    /// The table words each live default as the value the code gives, and the models line shows the value the run has.
    #[test]
    fn the_table_and_the_models_line_name_the_live_defaults() {
        for (name, _) in LIVE_DEFAULTS {
            let row = SETTINGS.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("{name} is not in the table"));
            assert!(row.default.starts_with("on") || row.default.contains("live"), "{name}: {}", row.default);
        }
        let line = models_line(&BenchSettings::from_pairs(&[]).with_live_defaults());
        for part in [" WEAPONS=1", " BODY_BLOCK_M=0.76", " DRAW_TIME=weapon", " ENEMY_HUMAN_LANES=on", " STAND_IN_LEVEL=on"] {
            assert!(line.contains(part), "{part} is not in `{line}`");
        }
    }

    /// A map data file before the bullet armor is refused by name, unless the run allows it and then it says so.
    #[test]
    fn a_map_data_file_without_bullet_armor_is_refused_unless_allowed() {
        let path = std::path::Path::new("level_01_day_2.mapdata");
        let first = mapdata::format::FIRST_WITH_ARMOR;
        assert_eq!(mapdata_format_verdict(path, first, false), Ok(None));
        assert_eq!(mapdata_format_verdict(path, first + 1, false), Ok(None));
        let refused = mapdata_format_verdict(path, first - 1, false).unwrap_err();
        assert!(refused.contains("level_01_day_2.mapdata") && refused.contains("no bullet armor"), "{refused}");
        let warned = mapdata_format_verdict(path, first - 1, true).unwrap().unwrap();
        assert!(warned.starts_with("WARNING:") && warned.contains("SRP_BENCH_MAPDATA_OLD=allow"), "{warned}");
    }

    /// An empty value is a value: a flag is on, a number is none, a text is empty.
    /// (The same answers `std::env` gave at the places this replaced.)
    #[test]
    fn an_empty_setting_is_set_for_a_flag_and_nothing_for_a_number() {
        let s = BenchSettings::from_pairs(&[
            ("SRP_BENCH_CHEAT", ""),
            ("SRP_BENCH_SECONDS", ""),
            ("SRP_BENCH_TIER", ""),
            ("SRP_BENCH_SIDES", ""),
            ("SRP_BENCH_ZONE_DAMAGE", ""),
        ]);
        assert!(s.flag("SRP_BENCH_CHEAT"));
        assert!(!s.flag("SRP_BENCH_ONE_SIDE"));
        assert_eq!(s.number::<u32>("SRP_BENCH_SECONDS"), None);
        assert_eq!(s.var("SRP_BENCH_TIER"), Ok(String::new()));
        assert_eq!(s.var("SRP_BENCH_SIDES"), Ok(String::new()));
        assert!(!s.var("SRP_BENCH_ZONE_DAMAGE").is_ok_and(|v| v == "1"));
        assert_eq!(s.var("SRP_BENCH_SEED"), Err(VarError::NotPresent));
        assert_eq!(s.var_os("SRP_BENCH_SEED"), None);
    }

    #[test]
    fn a_number_parses_as_before_and_the_side_one_names_are_listed() {
        let s = BenchSettings::from_pairs(&[
            ("SRP_BENCH_SEED", "7"),
            ("SRP_BENCH_TAKE_M", "0.5"),
            ("SRP_BENCH_SIDE1_COMBAT", "a=1"),
            ("SRP_BENCH_COMBAT", "b=2"),
        ]);
        assert_eq!(s.number::<u32>("SRP_BENCH_SEED"), Some(7));
        assert_eq!(s.number::<f32>("SRP_BENCH_TAKE_M"), Some(0.5));
        assert_eq!(s.number::<u32>("SRP_BENCH_TAKE_M"), None);
        assert_eq!(s.var("SRP_BENCH_SIDE1_COMBAT"), Ok("a=1".to_owned()));
        assert_eq!(s.var("SRP_BENCH_COMBAT"), Ok("b=2".to_owned()));
    }

    /// The list is the documentation: each setting says what it is and what it is unset,
    /// and none is listed twice.
    #[test]
    fn every_setting_says_what_it_is_and_what_it_is_unset() {
        let mut names = std::collections::BTreeSet::new();
        for setting in SETTINGS {
            assert!(!setting.what.is_empty() && !setting.default.is_empty(), "{}", setting.name);
            assert!(names.insert(setting.name), "{} is listed twice", setting.name);
        }
    }

    /// Every `SRP_BENCH_*` name written in the tests tree is in the list.
    /// The `bench models:` line names every `**model**` setting of the table (and the world settings), with the value the
    /// run has or the default the table words: a setting added to the table is in the line with no second list.
    #[test]
    fn the_models_line_covers_every_model_setting() {
        let line = models_line(&BenchSettings::from_pairs(&[]));
        assert!(line.starts_with("bench models: "), "{line}");
        let mut models = 0;
        for setting in SETTINGS {
            if setting.what.starts_with("**model**") || WORLD_SETTINGS.contains(&setting.name) {
                let name = setting.name.strip_prefix("SRP_BENCH_").unwrap();
                assert!(line.contains(&format!(" {name}=")), "{name} is not in `{line}`");
                models += 1;
            }
        }
        assert!(models >= 20, "the table has more than twenty models: {models}");
        for name in ["STAMINA", "HIT_MODEL", "BODY_BLOCK_M", "DRAW_TIME", "SPREAD", "USE_LINE", "LIVE_FIELDS", "PING", "WEAPONS"] {
            assert!(line.contains(&format!(" {name}=")), "{name} is not in `{line}`");
        }
        assert!(line.contains(" STAMINA=(off)"), "the default as the table words it: {line}");
        let set = models_line(&BenchSettings::from_pairs(&[("SRP_BENCH_STAMINA", "on"), ("SRP_BENCH_PING", "20-150")]));
        assert!(set.contains(" STAMINA=on") && set.contains(" PING=20-150"), "{set}");
    }

    #[test]
    fn every_bench_setting_the_tests_read_is_listed() {
        fn walk(dir: &std::path::Path, found: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, found);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    for (i, _) in text.match_indices("\"SRP_BENCH_") {
                        let name: String = text[i + 1..]
                            .chars()
                            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
                            .collect();
                        // A prefix (`"SRP_BENCH_"`, `"SRP_BENCH_SIDE1_"`) is built on.
                        if !name.ends_with('_') {
                            found.push(name);
                        }
                    }
                }
            }
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/v026g/bots/tests");
        let mut found = Vec::new();
        walk(&dir, &mut found);
        assert!(found.len() > 20, "{found:?}");
        let missing: Vec<_> = found.iter().filter(|n| !known(n)).collect();
        assert!(missing.is_empty(), "not in tests/settings.rs: {missing:?}");
    }

    /// A listed setting is never also a plays' (or reflex's, or senses') number list
    /// (`SRP_BENCH_<name>`, `SRP_BENCH_SIDE1_<name>` of [`TUNING_NAMES`]): the list is
    /// `name=value,...`, so a plain switch of the same name would panic in its parser.
    #[test]
    fn no_setting_is_named_like_a_tuning_list() {
        for setting in SETTINGS.iter() {
            for t in TUNING_NAMES {
                assert_ne!(setting.name, format!("SRP_BENCH_{t}"), "{t}");
                assert_ne!(setting.name, format!("SRP_BENCH_SIDE1_{t}"), "{t}");
            }
        }
        let mut names: Vec<_> = SETTINGS.iter().map(|s| s.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SETTINGS.len(), "a setting is listed twice");
    }
}

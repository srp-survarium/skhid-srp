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
    s("SRP_GAME_DATA", "unset: the built-in test archive, said in a loud line", "the server's game data (`server-game-data.db`) the run is on, read and checked as srp reads it (`game_data::init`); a file that fails refuses the run"),
    s("SRP_BENCH_MAPS", "6/2, 1/4, 3/3 (level/mode)", "the maps: `level/mode,...`"),
    s("SRP_BENCH_SIDES", "4,8,10", "the sizes, bots a side: `n,...`"),
    s("SRP_BENCH_SECONDS", "240", "the length of a match, s"),
    s("SRP_BENCH_SEED", "0", "another match of the same map and size (0: the digests' runs)"),
    s("SRP_BENCH_TIER", "unset: the lowest tier", "every bot's tier, `4-5`, `9-10` or `13-15`: each bot is the lobby's (its role from `bot_roles`, the tier's kits of `bot_profile`), read back as the match server does (`bench_roster`)"),
    s("SRP_BENCH_RUSHER", "control", "the kit the rusher bots of tiers 9-10 and 13-15 wear (`loadouts::RusherKits`, the lobby's `SRP_PVB_RUSHER_KIT`): `control` (the old kit: the SPAS-12 with buckshot; the default), `smg` (`sr2m` / `fn_p90`), `alt` (`ump45` / `kriss_vector`), `slug` or `helmet` (the SPAS-12 with slugs / buckshot and the SMG kit's helmet); one word for both tiers or `9-10=alt,13-15=smg`. A kit whose items the game data lacks is the old kit, said in a line (the built-in test archive has no `fn_p90` or `kriss_vector`: set SRP_GAME_DATA); the rusher's armour and ammo in the stand-in's hits follow it"),
    s("SRP_DECISIONS_OUT", "unset", "a file the run's decisions line is appended to"),
    // What is printed or written.
    s("SRP_BENCH_EVENTS", "unset", "a folder for the run's events, one JSON line each"),
    s("SRP_BENCH_NOTES", "unset", "a file the carry tally's notes are appended to"),
    s("SRP_BENCH_GATES", "off", "test every fire gate each tick, for the gate tally's counts (no shot changes)"),
    s("SRP_WRITE_GOLDEN", "off", "a path: the made-up match test writes its counts there instead of comparing (tallies/owner/golden.rs)"),
    s("SRP_BENCH_SHOW_STALE", "off", "print the stale-belief lines"),
    s("SRP_BENCH_SHOW_RAIDS", "off", "print the raid lines"),
    s("SRP_BENCH_WHY", "off", "print why each play ended"),
    s("SRP_BENCH_CARRIER_JUMP", "off", "`on` (or `1`) plans a battery carrier's routes over the jump links up to the standing jump's 0.998 m only (`Feel::carrier_jump_m` Some); off by default: every jump link"),
    s("SRP_BENCH_STRANDED_START", "off", "`on` (or `1`) lets a body that stands on no floor of the grid (on the top of an object) plan from the floor it drops to, up to 5 m below (`ThinkTuning::stranded_start`); off by default: it has no route"),
    s("SRP_BENCH_LIP_GUARD", "off", "`on` (or `1`) turns the lip guard and the short look-ahead on lip floors on (`Feel::lip_guard_ticks` 2, `lip_look_ahead_m` 0.8); both are off by default"),
    s("SRP_BENCH_FALLS", "off", "print a line for every fall of a metre or more (FALL unplanned, PLANNED on the route), the bots' reckoning error and their distance off the route (`tallies/route.rs`)"),
    // The bots' side: tuning of the bench's bots.
    s("SRP_BENCH_CHEAT", "off", "the battery cheat instead of the fair battery knowledge"),
    s("SRP_BENCH_BATTERY_SIGHT_M", "the tuning's", "SenseTuning::battery_sight_m"),
    s("SRP_BENCH_FRIEND_NEAR_M", "the tuning's", "FriendlyFireTuning::near_m"),
    s("SRP_BENCH_FF_LEAD", "the tuning's", "FriendlyFireTuning::lead: `0` off, else on"),
    s("SRP_BENCH_FLOOR_PATHS", "on", "Tuning::grid.floor_paths (off by default): `1` makes every play's path distance the regions' walk between floors a storey apart; off, the straight ground distance (Collect's batteries a storey off keep the walk)"),
    s("SRP_BENCH_FF_PAST_M", "the tuning's", "FriendlyFireTuning::past_target_m"),
    s("SRP_BENCH_FF_TOUCH_M", "the tuning's", "FriendlyFireTuning::touch_m"),
    s("SRP_BENCH_FF_LEAD_MS", "the tuning's", "FriendlyFireTuning::lead_extra_ms"),
    s("SRP_BENCH_THREAT", "the tuning's (on)", "ThreatTuning::spots: `0` off (no threat spots, nothing kept out of a shooter's line), else on"),
    s("SRP_BENCH_THREAT_DETOUR", "the tuning's (on)", "ThreatTuning::detour: `0` off (a route through a shooter's lane is walked as it is), else on"),
    s("SRP_BENCH_THREAT_FLOORS", "the tuning's (3000)", "ThreatTuning::search_floors: floors of a poll's budget one search round a shooter's lane may take"),
    s("SRP_BENCH_THREAT_SPRINT", "the tuning's (on)", "ThreatTuning::sprint: `0` off (a bot in a shooter's lane keeps the pace its play chose), else on"),
    s("SRP_BENCH_ESCAPE_CEILING", "on", "Tuning::hazard.escape: a way out of a zone stays under the strength of its start's; `0` the nearest way over any zone"),
    s("SRP_BENCH_SEARCH_FLOORS", "the tuning's", "floors a poll's searches expand: a number or `max`"),
    s("SRP_BENCH_ONE_SIDE", "off", "team 1 plays by the SRP_BENCH_SIDE1_* numbers, so a change to team 0 alone shows"),
    s("SRP_BENCH_PING", "unset: none", "a ping for each place, `lo-hi` ms"),
    s("SRP_BENCH_WEAPONS", "off", "every place's weapon known as a live match knows it (play 11's matchup)"),
    s("SRP_BENCH_HUMAN", "unset", "the place the plays take for a human (CG-12)"),
    s("SRP_BENCH_CAMPER", "unset", "the place of the stand-in camper"),
    s("SRP_BENCH_CAMPER_SPOT", "most", "**model** `hidden`: the camper's spot is one that sees many route points of the other side and is seen from under half of them (else the one that sees the most)"),
    s("SRP_BENCH_CAMPER_CROUCH", "unset", "**model** `1`: the camper crouches once at its spot (and picks it by the crouched eye and body)"),
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
    s("SRP_BENCH_ZONE_DAMAGE", "off", "**model** zones hurt the stand-in's bodies; `1` only (a GUESS of the game's)"),
    s("SRP_BENCH_DAMAGE", "per weapon", "**model** `flat`: every stand-in hit takes a fifth of the health and nothing regenerates (the old stand-in); else the weapon's damage against the target's kit (`standin_damage.rs`)"),
    s("SRP_BENCH_GRENADE_DAMAGE", "on", "**model** `off`: a grenade's blast hurts nobody (only counted, as before `standin_grenade.rs`)"),
    s("SRP_BENCH_STOCK", "on", "**model** `off`: the stand-in's bodies fire whenever the fire key is down (no rounds, reloads or finite ammo: the stand-in before `standin_weapon.rs`); the counts stay"),
    s("SRP_BENCH_RESUPPLY", "on", "**model** `off`: the resupply box does nothing (the stand-in before `Stock::resupply`); the counts stay at zero"),
    s("SRP_BENCH_PUT_MS", "PUT_MS (600)", "**model** a put-in lands this long after the press, ms"),
    s("SRP_BENCH_JUMP_AHEAD_M", "STANDING_JUMP.ahead_m", "**model** how far ahead a standing jump reaches, m"),
    s("SRP_BENCH_HEAL_MODEL", "follows `heal.on` of the run's tuning (off by default)", "**model** `on`/`off`: the stand-in has the drugs and their stock or not (standin_heal.rs)"),
    s("SRP_BENCH_STAND_IN", "honest", "**model** `old`: the old lift up any floor in reach instead of the client's jump clips"),
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

    /// The process's environment, read once for every listed name.
    pub(crate) fn from_env() -> Self {
        Self::from_getter(|name| std::env::var_os(name))
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

    /// `var(name).ok().and_then(|v| v.parse().ok())`.
    pub(crate) fn number<T: std::str::FromStr>(&self, name: &str) -> Option<T> {
        self.var(name).ok().and_then(|v| v.parse().ok())
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

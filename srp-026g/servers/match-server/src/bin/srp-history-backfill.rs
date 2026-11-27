//! Matches of before the history, made again from their recordings, as a
//! bundle a one-off script adds to srp.db (`docs/notes/match_history.md`,
//! "Matches of before the history"; `scripts/history-backfill/apply.py`).
//!
//! ```text
//! srp-history-backfill extract --data <server-game-data.db> --out <facts.jsonl> <recording>...
//! srp-history-backfill bundle --db <copy of srp.db> --data <server-game-data.db> --out <bundle.db>
//!     [--reports <reports.jsonl>] [--logs <srp log>...] [--bot-names <a,b>]
//!     [--twists-from <unix>] [--] <facts.jsonl>...
//! srp-history-backfill migrate --db <copy of srp.db>
//! ```
//!
//! `migrate` brings a copy of srp.db to this srp's schema, as srp's start
//! does: what `apply.py`'s dry run needs of a copy of the live file before
//! the release that ships migration 27.
//!
//! `extract` reads recordings with one server game data (the kits of the
//! match options are read with it: a period's recordings with its own data)
//! into one line of facts each, or why it gives none (`backfill::Dropped`).
//!
//! `bundle` takes every facts file (a recording read more than once keeps its
//! reading whose kits read), makes each match as srp's settlement had it and
//! writes it into the copy of srp.db by the code srp writes a match with
//! (`rewards::history_record`, `GameRepository::record_match`), marked
//! `recording`: the copy is migrated and changed, never the live file. Then
//! the bundle holds those matches with their lines, kits and items, and what
//! they added to the totals (by player, clan and item): what the one-off
//! script adds to the live file. Left out, each said with its count:
//! - a match of the history's time or after (the first match srp wrote),
//!   or one the copy has;
//! - a room with a PIN's (its report says so; a recording does not);
//! - a free-for-all from `--twists-from` on: its twist is not recorded;
//! - one whose map the game data no longer has.
//!
//! A line's rating after is its report's (`--reports`: the reports of the
//! srp.db copies, one JSON a line) or its settle line's in srp's logs
//! (`--logs`), else unknown; its experience is unknown (`None`): the pay of
//! then is not kept.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use match_server::v026g::backfill::{self, Extract, MatchFacts};
use rusqlite::{Connection, params};
use session::v026g::persist::history::{self, MatchKind, Paid, Source};
use session::v026g::persist::{GameRepository, SqliteGameRepository};
use session::v026g::rewards::{self, HistoryPeople, Judged};
use survarium::v026g::maps::GameMode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("extract") => extract(&args[1..]),
        Some("bundle") => bundle(&args[1..]),
        Some("migrate") => Options::parse(&args[1..], &["db"], &[]).and_then(|options| {
            SqliteGameRepository::open(Path::new(options.one("db")?))
                .map(|_| ())
                .map_err(|error| error.to_string())
        }),
        _ => Err(usage()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> String {
    "usage: srp-history-backfill extract --data <db> --out <facts.jsonl> <recording>...\n       \
     srp-history-backfill bundle --db <copy> --data <db> --out <bundle.db> [--reports <jsonl>] \
     [--logs <log>...] [--bot-names <a,b>] [--twists-from <unix>] [--] <facts.jsonl>...\n       \
     srp-history-backfill migrate --db <copy>"
        .to_owned()
}

/// The options of a command and the rest of its arguments.
struct Options {
    values: HashMap<String, Vec<String>>,
    rest: Vec<String>,
}

impl Options {
    fn parse(args: &[String], known: &[&str], many: &[&str]) -> Result<Self, String> {
        let mut values: HashMap<String, Vec<String>> = HashMap::new();
        let mut rest = Vec::new();
        let mut at = 0;
        while at < args.len() {
            let arg = &args[at];
            if arg == "--" {
                // The end of the options (after one that takes many).
                rest.extend(args[at + 1..].iter().cloned());
                break;
            }
            if let Some(name) = arg.strip_prefix("--") {
                if !known.contains(&name) {
                    return Err(format!("unknown option --{name}\n{}", usage()));
                }
                at += 1;
                if many.contains(&name) {
                    while at < args.len() && !args[at].starts_with("--") {
                        values
                            .entry(name.to_owned())
                            .or_default()
                            .push(args[at].clone());
                        at += 1;
                    }
                } else {
                    let value = args
                        .get(at)
                        .ok_or_else(|| format!("--{name} takes a value"))?;
                    values
                        .entry(name.to_owned())
                        .or_default()
                        .push(value.clone());
                    at += 1;
                }
            } else {
                rest.push(arg.clone());
                at += 1;
            }
        }
        Ok(Self { values, rest })
    }

    fn one(&self, name: &str) -> Result<&str, String> {
        self.values
            .get(name)
            .and_then(|values| values.first())
            .map(String::as_str)
            .ok_or_else(|| format!("--{name} is needed\n{}", usage()))
    }

    fn all(&self, name: &str) -> &[String] {
        self.values.get(name).map_or(&[], Vec::as_slice)
    }
}

fn init_data(path: &str) -> Result<(), String> {
    survarium::v026g::game_data::init_from(Path::new(path))
        .map(|_| ())
        .map_err(|error| format!("{path}: {error}"))
}

fn extract(args: &[String]) -> Result<(), String> {
    let options = Options::parse(args, &["data", "out"], &[])?;
    init_data(options.one("data")?)?;
    let paths: Vec<PathBuf> = options.rest.iter().map(PathBuf::from).collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = paths.len().div_ceil(threads).max(1);
    let read: Vec<Extract> = std::thread::scope(|scope| {
        let handles: Vec<_> = paths
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .map(|path| backfill::extract(path))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("a reading thread"))
            .collect()
    });
    let out = options.one("out")?;
    let mut file =
        BufWriter::new(std::fs::File::create(out).map_err(|error| format!("{out}: {error}"))?);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for one in &read {
        let key = match (&one.facts, &one.dropped) {
            (Some(facts), _) if facts.kits_read => "read".to_owned(),
            (Some(_), _) => "read, kits not read".to_owned(),
            (None, Some(dropped)) => format!("{dropped:?}")
                .split([' ', '{'])
                .next()
                .unwrap_or("")
                .to_owned(),
            (None, None) => "nothing".to_owned(),
        };
        *counts.entry(key).or_default() += 1;
        serde_json::to_writer(&mut file, one).map_err(|error| error.to_string())?;
        file.write_all(b"\n").map_err(|error| error.to_string())?;
    }
    for (what, count) in counts {
        println!("{count}\t{what}");
    }
    Ok(())
}

/// Why `bundle` left a match out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum LeftOut {
    Dropped(&'static str),
    OfTheHistory,
    InTheCopy,
    Private,
    TwistUnknown,
    UnknownMap,
}

/// A line's rating after, by match id and account (reports) or name (logs).
#[derive(Default)]
struct Ratings {
    by_account: HashMap<(u32, u32), u16>,
    by_name: HashMap<(u32, String), u16>,
    private: HashSet<u32>,
}

impl Ratings {
    fn read_reports(&mut self, path: &str) -> Result<(), String> {
        let file = std::fs::File::open(path).map_err(|error| format!("{path}: {error}"))?;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| error.to_string())?;
            let Ok(report) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            let Some(match_id) = report["match_id"].as_u64() else {
                continue;
            };
            let match_id = match_id as u32;
            if report["private"].as_bool() == Some(true) {
                self.private.insert(match_id);
            }
            for player in report["players"].as_array().into_iter().flatten() {
                if let (Some(account), Some(rating)) = (player["a"].as_u64(), player["ra"].as_u64())
                {
                    self.by_account
                        .insert((match_id, account as u32), rating as u16);
                }
            }
        }
        Ok(())
    }

    /// srp's settle lines: `Match 0x<id>: <name> (<standing>) gets ... rating <a> -> <b> (score <s>)`.
    fn read_log(&mut self, path: &str) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
        let text = String::from_utf8_lossy(&bytes);
        for line in text.lines() {
            let Some(at) = line.find("Match 0x") else {
                continue;
            };
            let line = &line[at + 8..];
            let Some((id, rest)) = line.split_once(": ") else {
                continue;
            };
            let Ok(match_id) = u32::from_str_radix(id, 16) else {
                continue;
            };
            let Some(gets) = rest.find(") gets ") else {
                continue;
            };
            let Some(open) = rest[..gets].rfind(" (") else {
                continue;
            };
            let name = &rest[..open];
            let Some(rating) = rest.rfind(" rating ") else {
                continue;
            };
            let Some((_, after)) = rest[rating + 8..].split_once(" -> ") else {
                continue;
            };
            let Some(after) = after
                .split(' ')
                .next()
                .and_then(|value| value.parse::<u16>().ok())
            else {
                continue;
            };
            self.by_name.insert((match_id, name.to_owned()), after);
        }
        Ok(())
    }

    fn of(&self, match_id: u32, account: u32, name: &str) -> Option<u16> {
        self.by_account
            .get(&(match_id, account))
            .or_else(|| self.by_name.get(&(match_id, name.to_owned())))
            .copied()
    }
}

/// The accounts of the copy, and the clans by tag of one match's places.
struct People<'a> {
    known: &'a HashSet<u32>,
    clans: HashMap<u32, u32>,
}

impl HistoryPeople for People<'_> {
    fn known(&self, account_id: u32) -> bool {
        self.known.contains(&account_id)
    }

    fn clan_of(&self, account_id: u32) -> Option<u32> {
        self.clans.get(&account_id).copied()
    }
}

fn sql(error: rusqlite::Error) -> String {
    error.to_string()
}

fn bundle(args: &[String]) -> Result<(), String> {
    let options = Options::parse(
        args,
        &[
            "db",
            "data",
            "out",
            "reports",
            "logs",
            "bot-names",
            "twists-from",
        ],
        &["logs"],
    )?;
    init_data(options.one("data")?)?;
    let copy = options.one("db")?;
    let out = options.one("out")?;
    if Path::new(out).exists() {
        return Err(format!("{out} exists: a bundle is written anew"));
    }
    let twists_from: Option<u64> = match options.values.get("twists-from") {
        Some(values) => Some(
            values[0]
                .parse()
                .map_err(|_| "--twists-from takes Unix seconds")?,
        ),
        None => None,
    };
    let bot_names: HashSet<String> = options
        .values
        .get("bot-names")
        .map(|values| {
            values[0]
                .split(',')
                .map(|name| name.trim().to_ascii_lowercase())
                .collect()
        })
        .unwrap_or_default();
    let mut ratings = Ratings::default();
    if let Ok(path) = options.one("reports") {
        ratings.read_reports(path)?;
    }
    for path in options.all("logs") {
        ratings.read_log(path)?;
    }

    // Every reading, one a recording: the one whose kits read.
    let mut readings: BTreeMap<String, Extract> = BTreeMap::new();
    for path in &options.rest {
        let file = std::fs::File::open(path).map_err(|error| format!("{path}: {error}"))?;
        for line in BufReader::new(file).lines() {
            let line = line.map_err(|error| error.to_string())?;
            let one: Extract =
                serde_json::from_str(&line).map_err(|error| format!("{path}: {error}"))?;
            let better = match (readings.get(&one.stem), &one.facts) {
                (None, _) => true,
                (Some(held), Some(facts)) => held
                    .facts
                    .as_ref()
                    .is_none_or(|held| !held.kits_read && facts.kits_read),
                (Some(_), None) => false,
            };
            if better {
                readings.insert(one.stem.clone(), one);
            }
        }
    }

    // The copy: migrated by its opening.
    let mut repository =
        SqliteGameRepository::open(Path::new(copy)).map_err(|error| error.to_string())?;
    let connection = Connection::open(copy).map_err(sql)?;
    let history_began: Option<i64> = connection
        .query_row(
            "SELECT MIN(ended_at) FROM g26_matches WHERE source = 'live'",
            [],
            |row| row.get(0),
        )
        .map_err(sql)?;
    let kept: HashSet<String> = connection
        .prepare("SELECT replay_id FROM g26_matches")
        .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
        .map_err(sql)?;
    let known: HashSet<u32> = connection
        .prepare("SELECT account_id FROM g26_accounts")
        .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
        .map_err(sql)?;
    let clans_by_tag: HashMap<String, u32> = connection
        .prepare("SELECT tag, clan_id FROM g26_clans WHERE registered != 0")
        .and_then(|mut statement| {
            statement
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get(1)?)))?
                .collect()
        })
        .map_err(sql)?;

    // What the totals were before: the bundle keeps what the matches added.
    connection
        .execute("ATTACH DATABASE ?1 AS b", [out])
        .map_err(sql)?;
    for table in TOTALS_TABLES {
        connection
            .execute_batch(&format!(
                "CREATE TABLE b.before_{table} AS SELECT * FROM main.{table};"
            ))
            .map_err(sql)?;
    }

    let mut left_out: BTreeMap<LeftOut, usize> = BTreeMap::new();
    let mut recorded = 0usize;
    let mut unrated = 0usize;
    let mut lines = 0usize;
    for (stem, one) in &readings {
        let Some(facts) = &one.facts else {
            let why = match &one.dropped {
                Some(backfill::Dropped::Unreadable { .. }) => "unreadable",
                Some(backfill::Dropped::NoResult) => "no result note",
                Some(backfill::Dropped::VsBots) => "against the server's bots",
                Some(backfill::Dropped::UnknownMap { .. }) => "map not in the game data",
                Some(backfill::Dropped::TrustedWorldUnreadable) => "trusted world unreadable",
                Some(backfill::Dropped::Disagrees { .. }) => {
                    "trusted world disagrees with the scoreboard"
                }
                None => "nothing",
            };
            *left_out.entry(LeftOut::Dropped(why)).or_default() += 1;
            continue;
        };
        if history_began.is_some_and(|began| facts.ended_at as i64 >= began) {
            *left_out.entry(LeftOut::OfTheHistory).or_default() += 1;
            continue;
        }
        if kept.contains(stem) {
            *left_out.entry(LeftOut::InTheCopy).or_default() += 1;
            continue;
        }
        let Some(result) = facts.result() else {
            *left_out.entry(LeftOut::UnknownMap).or_default() += 1;
            continue;
        };
        if ratings.private.contains(&result.match_id) {
            *left_out.entry(LeftOut::Private).or_default() += 1;
            continue;
        }
        if result.mode == GameMode::FreeForAll
            && twists_from.is_some_and(|from| facts.ended_at >= from)
        {
            *left_out.entry(LeftOut::TwistUnknown).or_default() += 1;
            continue;
        }
        let Some(record) = record_of(facts, &result, &ratings, &known, &clans_by_tag, &bot_names)
        else {
            *left_out
                .entry(LeftOut::Dropped("no account in it"))
                .or_default() += 1;
            continue;
        };
        lines += record.players.len();
        unrated += record
            .players
            .iter()
            .filter(|player| player.line.rating.is_none())
            .count();
        repository
            .record_match(&record)
            .map_err(|error| error.to_string())?;
        recorded += 1;
    }
    drop(repository);

    write_bundle(&connection, copy)?;
    println!(
        "{recorded}\tmatches made again from their recordings ({lines} lines, {unrated} with no rating known)"
    );
    for (why, count) in left_out {
        println!("{count}\tleft out: {why:?}");
    }
    Ok(())
}

/// The match's record, as srp's settlement writes one.
fn record_of(
    facts: &MatchFacts,
    result: &rewards::MatchResult,
    ratings: &Ratings,
    known: &HashSet<u32>,
    clans_by_tag: &HashMap<String, u32>,
    bot_names: &HashSet<String>,
) -> Option<history::MatchRecord> {
    let latest = result.latest_places();
    let judged = Judged::of(&latest);
    let bots: Vec<bool> = latest
        .players
        .iter()
        .map(|player| player.driven || bot_names.contains(&player.name.to_ascii_lowercase()))
        .collect();
    let paid: Vec<Paid> = latest
        .players
        .iter()
        .map(|player| Paid {
            rating: ratings.of(latest.match_id, player.account_id, &player.name),
            experience: None,
        })
        .collect();
    let kits = latest
        .players
        .iter()
        .map(|player| {
            judged.kit_lines(player, facts.kits_of(player.place), None, |index| {
                facts.kit_items(player.place, index)
            })
        })
        .collect();
    let clans = latest
        .players
        .iter()
        .filter_map(|player| {
            let tag = &facts
                .places
                .iter()
                .find(|place| place.place == player.place)?
                .clan_tag;
            Some((player.account_id, *clans_by_tag.get(tag)?))
        })
        .collect();
    let people = People { known, clans };
    let record = rewards::history_record(
        &judged,
        &bots,
        &paid,
        kits,
        &people,
        facts.ended_at,
        Source::Recording,
    )?;
    // A recording is kept only of a match of six players or more: the
    // public ones (a room with a PIN's is told by its report, above).
    matches!(record.kind, MatchKind::Public(_)).then_some(record)
}

/// The totals the history keeps as matches are written, and their keys.
const TOTALS_TABLES: [&str; 4] = [
    "g26_player_totals",
    "g26_clan_totals",
    "g26_player_items",
    "g26_player_weapons",
];

fn key_columns(table: &str) -> &'static [&'static str] {
    match table {
        "g26_player_totals" => &["account_id"],
        "g26_clan_totals" => &["clan_id"],
        _ => &["account_id", "dict_id"],
    }
}

/// The bundle: the matches marked `recording`, their lines, kits and
/// items, and each total's change (`delta_<table>`: a row a key the
/// matches touched, the new value less the old; a key new to the table is
/// a row of its own, `new` 1).
fn write_bundle(connection: &Connection, copy: &str) -> Result<(), String> {
    let batch = format!(
        "CREATE TABLE b.meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO b.meta VALUES ('format', '1'), ('made_from', {copy}),
             ('made_at', CAST(strftime('%s', 'now') AS TEXT));
         CREATE TABLE b.matches AS SELECT * FROM main.g26_matches WHERE source = 'recording';
         CREATE TABLE b.match_players AS SELECT p.* FROM main.g26_match_players p
             JOIN main.g26_matches m ON m.id = p.match_id WHERE m.source = 'recording';
         CREATE TABLE b.match_kits AS SELECT k.* FROM main.g26_match_kits k
             JOIN main.g26_matches m ON m.id = k.match_id WHERE m.source = 'recording';
         CREATE TABLE b.match_kit_items AS SELECT i.* FROM main.g26_match_kit_items i
             JOIN main.g26_matches m ON m.id = i.match_id WHERE m.source = 'recording';
         CREATE TABLE b.match_weapons AS SELECT w.* FROM main.g26_match_weapons w
             JOIN main.g26_matches m ON m.id = w.match_id WHERE m.source = 'recording';",
        copy = quote(copy)
    );
    connection.execute_batch(&batch).map_err(sql)?;
    for table in TOTALS_TABLES {
        let columns: Vec<String> = connection
            .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
            .map_err(sql)?;
        let keys = key_columns(table);
        let on = keys
            .iter()
            .map(|key| format!("o.{key} = n.{key}"))
            .collect::<Vec<_>>()
            .join(" AND ");
        let values: Vec<String> = columns
            .iter()
            .map(|column| {
                if keys.contains(&column.as_str()) {
                    format!("n.{column} AS {column}")
                } else if column == "gear" {
                    "n.gear AS gear".to_owned()
                } else if column == "longest_kill_dm" {
                    // The longest of them: kept as the new value.
                    format!("n.{column} AS {column}")
                } else {
                    format!("n.{column} - IFNULL(o.{column}, 0) AS {column}")
                }
            })
            .collect();
        let changed: Vec<String> = columns
            .iter()
            .filter(|column| !keys.contains(&column.as_str()) && column.as_str() != "gear")
            .map(|column| format!("n.{column} IS NOT o.{column}"))
            .collect();
        connection
            .execute_batch(&format!(
                "CREATE TABLE b.delta_{table} AS
                 SELECT {values}, (o.{first} IS NULL) AS new
                 FROM main.{table} n LEFT JOIN b.before_{table} o ON {on}
                 WHERE o.{first} IS NULL OR {changed};
                 DROP TABLE b.before_{table};",
                values = values.join(", "),
                first = keys[0],
                changed = if changed.is_empty() {
                    "0".to_owned()
                } else {
                    changed.join(" OR ")
                },
            ))
            .map_err(sql)?;
    }
    let matches: i64 = connection
        .query_row("SELECT COUNT(*) FROM b.matches", [], |row| row.get(0))
        .map_err(sql)?;
    connection
        .execute(
            "INSERT INTO b.meta VALUES ('matches', ?1)",
            params![matches.to_string()],
        )
        .map_err(sql)?;
    Ok(())
}

fn quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

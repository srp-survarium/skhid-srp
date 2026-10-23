//! The data file: `{ "updated", "notices": [...], "items": [...] }` (README.md has the schema).
//!
//! Parsing is two steps: serde reads the raw shape (types, unknown fields), then `validate`
//! checks every value against its allowed set and reports each problem with its field path.

use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum State {
    Waiting,
    InProgress,
    Review,
    Next,
    Requested,
    Done,
    Parked,
}

impl State {
    pub const ALL: [(&'static str, State); 7] = [
        ("waiting", State::Waiting),
        ("in-progress", State::InProgress),
        ("review", State::Review),
        ("next", State::Next),
        ("requested", State::Requested),
        ("done", State::Done),
        ("parked", State::Parked),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    Owner,
    Player,
    Dev,
}

impl Source {
    pub const ALL: [(&'static str, Source); 3] = [
        ("owner", Source::Owner),
        ("player", Source::Player),
        ("dev", Source::Dev),
    ];

    pub fn name(self) -> &'static str {
        name_of(&Self::ALL, self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Done,
}

impl Level {
    pub const ALL: [(&'static str, Level); 3] =
        [("info", Level::Info), ("warn", Level::Warn), ("done", Level::Done)];

    pub fn name(self) -> &'static str {
        name_of(&Self::ALL, self)
    }
}

pub const AREAS: [&str; 14] = [
    "launcher", "updater", "defender", "lobby", "match", "weapons", "economy", "maps", "server",
    "linux", "tooling", "release", "site", "other",
];

fn name_of<T: PartialEq + Copy>(all: &[(&'static str, T)], v: T) -> &'static str {
    all.iter().find(|(_, x)| *x == v).map(|(n, _)| *n).unwrap_or("?")
}

/// A calendar date (`YYYY-MM-DD`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    pub y: i32,
    pub m: u32,
    pub d: u32,
}

const MONTHS: [&str; 12] =
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

impl Date {
    pub fn parse(s: &str) -> Option<Date> {
        let b = s.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
            return None;
        }
        let num = |r: std::ops::Range<usize>| -> Option<u32> {
            let p = &s[r];
            p.bytes().all(|c| c.is_ascii_digit()).then(|| p.parse().ok())?
        };
        let (y, m, d) = (num(0..4)? as i32, num(5..7)?, num(8..10)?);
        let date = Date { y, m, d };
        // Round-tripping through the day count rejects 2026-02-30 and friends.
        ((1..=12).contains(&m) && Date::from_days(date.days()) == date).then_some(date)
    }

    /// Days since 1970-01-01 (H. Hinnant's `days_from_civil`).
    pub fn days(self) -> i64 {
        let y = i64::from(self.y) - i64::from(self.m <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = i64::from(self.m);
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(self.d) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }

    /// The inverse of `days`.
    pub fn from_days(z: i64) -> Date {
        let z = z + 719468;
        let era = z.div_euclid(146097);
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let y = (yoe + era * 400 + i64::from(m <= 2)) as i32;
        Date { y, m, d }
    }

    /// "23 Oct".
    pub fn short(self) -> String {
        format!("{} {}", self.d, MONTHS[(self.m - 1) as usize])
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }
}

// ---- raw shape (serde) ----

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBoard {
    updated: String,
    #[serde(default)]
    notices: Vec<RawNotice>,
    #[serde(default)]
    items: Vec<RawItem>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNotice {
    date: String,
    text: String,
    #[serde(default)]
    level: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawItem {
    n: u32,
    id: String,
    title: String,
    state: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    area: Option<String>,
    next: String,
    #[serde(default)]
    detail: Option<String>,
    #[serde(default)]
    requested_by: Vec<String>,
    #[serde(default)]
    prs: Vec<u32>,
    #[serde(default)]
    links: Vec<RawLink>,
    updated: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLink {
    label: String,
    url: String,
}

// ---- validated board ----

#[derive(Clone, Debug)]
pub struct Board {
    pub updated: String,
    pub notices: Vec<Notice>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub struct Notice {
    pub date: Date,
    pub text: String,
    pub level: Level,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub n: u32,
    pub id: String,
    pub title: String,
    pub state: State,
    pub source: Source,
    pub area: Option<String>,
    pub next: String,
    pub detail: Option<String>,
    pub requested_by: Vec<String>,
    pub prs: Vec<u32>,
    pub links: Vec<Link>,
    pub updated: Date,
}

#[derive(Clone, Debug)]
pub struct Link {
    pub label: String,
    pub url: String,
}

/// Parse and validate a data file. The error lists every problem found, one per line.
pub fn parse(text: &str) -> Result<Board, String> {
    let raw: RawBoard = serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;
    validate(raw)
}

fn pick<T: Copy>(all: &[(&'static str, T)], v: &str) -> Option<T> {
    all.iter().find(|(n, _)| *n == v).map(|(_, x)| *x)
}

fn expected<T>(all: &[(&'static str, T)]) -> String {
    all.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
}

fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

fn validate(raw: RawBoard) -> Result<Board, String> {
    let mut errs: Vec<String> = Vec::new();
    let mut need = |ok: bool, path: &str, msg: &str| {
        if !ok {
            errs.push(format!("{path}: {msg}"));
        }
    };

    need(!raw.updated.trim().is_empty(), "updated", "must not be empty");

    let mut notices = Vec::new();
    for (i, n) in raw.notices.into_iter().enumerate() {
        let p = format!("notices[{i}]");
        let date = Date::parse(&n.date);
        need(date.is_some(), &format!("{p}.date"), &format!("{:?} is not a YYYY-MM-DD date", n.date));
        need(!n.text.trim().is_empty(), &format!("{p}.text"), "must not be empty");
        let level = match n.level.as_deref() {
            None => Some(Level::Info),
            Some(l) => pick(&Level::ALL, l),
        };
        need(
            level.is_some(),
            &format!("{p}.level"),
            &format!("unknown level {:?}; expected one of {}", n.level.unwrap_or_default(), expected(&Level::ALL)),
        );
        if let (Some(date), Some(level)) = (date, level) {
            notices.push(Notice { date, text: n.text, level });
        }
    }

    let mut items = Vec::new();
    let mut numbers: HashMap<u32, usize> = HashMap::new();
    let mut ids: HashMap<String, usize> = HashMap::new();
    for (i, it) in raw.items.into_iter().enumerate() {
        let p = format!("items[{i}] (T{})", it.n);
        let f = |field: &str| format!("{p}.{field}");
        need(it.n >= 1, &f("n"), "must be 1 or more");
        if let Some(prev) = numbers.insert(it.n, i) {
            need(false, &f("n"), &format!("T{} is already used by items[{prev}]", it.n));
        }
        need(is_slug(&it.id), &f("id"), &format!("{:?} is not a slug (a-z, 0-9, -)", it.id));
        if let Some(prev) = ids.insert(it.id.clone(), i) {
            need(false, &f("id"), &format!("{:?} is already used by items[{prev}]", it.id));
        }
        need(!it.title.trim().is_empty(), &f("title"), "must not be empty");
        need(!it.next.trim().is_empty(), &f("next"), "must not be empty");
        let state = pick(&State::ALL, &it.state);
        need(
            state.is_some(),
            &f("state"),
            &format!("unknown state {:?}; expected one of {}", it.state, expected(&State::ALL)),
        );
        let source = match it.source.as_deref() {
            None => Some(Source::Dev),
            Some(s) => pick(&Source::ALL, s),
        };
        need(
            source.is_some(),
            &f("source"),
            &format!(
                "unknown source {:?}; expected one of {}",
                it.source.clone().unwrap_or_default(),
                expected(&Source::ALL)
            ),
        );
        if let Some(a) = &it.area {
            need(
                AREAS.contains(&a.as_str()),
                &f("area"),
                &format!("unknown area {a:?}; expected one of {}", AREAS.join(", ")),
            );
        }
        let updated = Date::parse(&it.updated);
        need(updated.is_some(), &f("updated"), &format!("{:?} is not a YYYY-MM-DD date", it.updated));
        for (k, pr) in it.prs.iter().enumerate() {
            need(*pr >= 1, &f(&format!("prs[{k}]")), "must be a PR number");
        }
        for (k, name) in it.requested_by.iter().enumerate() {
            need(!name.trim().is_empty(), &f(&format!("requested_by[{k}]")), "must not be empty");
        }
        let mut links = Vec::new();
        for (k, l) in it.links.into_iter().enumerate() {
            let lp = f(&format!("links[{k}]"));
            need(!l.label.trim().is_empty(), &format!("{lp}.label"), "must not be empty");
            need(
                l.url.starts_with("https://") || l.url.starts_with("http://"),
                &format!("{lp}.url"),
                "must start with https:// or http://",
            );
            links.push(Link { label: l.label, url: l.url });
        }
        if let (Some(state), Some(source), Some(updated)) = (state, source, updated) {
            items.push(Item {
                n: it.n,
                id: it.id,
                title: it.title,
                state,
                source,
                area: it.area,
                next: it.next,
                detail: it.detail.filter(|d| !d.trim().is_empty()),
                requested_by: it.requested_by,
                prs: it.prs,
                links,
                updated,
            });
        }
    }

    if errs.is_empty() {
        Ok(Board { updated: raw.updated, notices, items })
    } else {
        Err(errs.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../example.json");

    fn item(extra: &str) -> String {
        format!(
            r#"{{"updated":"x","items":[{{"n":1,"id":"a","title":"A","state":"next","next":"go","updated":"2026-10-23"{extra}}}]}}"#
        )
    }

    #[test]
    fn example_file_is_valid() {
        let b = parse(EXAMPLE).expect("example.json parses");
        assert!(!b.items.is_empty());
        assert!(!b.notices.is_empty());
    }

    #[test]
    fn minimal_item_gets_defaults() {
        let b = parse(&item("")).unwrap();
        assert_eq!(b.items[0].source, Source::Dev);
        assert_eq!(b.items[0].state, State::Next);
        assert!(b.items[0].area.is_none());
    }

    #[test]
    fn unknown_state_is_rejected_with_path() {
        let e = parse(&item("").replace("\"next\",\"next\"", "\"nope\",\"next\"")).unwrap_err();
        assert!(e.contains("items[0] (T1).state: unknown state \"nope\""), "{e}");
    }

    #[test]
    fn unknown_source_area_level_are_rejected() {
        let e = parse(&item(r#","source":"boss","area":"moon""#)).unwrap_err();
        assert!(e.contains("items[0] (T1).source"), "{e}");
        assert!(e.contains("items[0] (T1).area"), "{e}");
        let e = parse(r#"{"updated":"x","notices":[{"date":"2026-10-23","text":"t","level":"loud"}]}"#)
            .unwrap_err();
        assert!(e.contains("notices[0].level: unknown level \"loud\""), "{e}");
    }

    #[test]
    fn duplicate_numbers_and_ids_are_rejected() {
        let two = r#"{"updated":"x","items":[
            {"n":4,"id":"a","title":"A","state":"next","next":"go","updated":"2026-10-23"},
            {"n":4,"id":"a","title":"B","state":"next","next":"go","updated":"2026-10-23"}]}"#;
        let e = parse(two).unwrap_err();
        assert!(e.contains("items[1] (T4).n: T4 is already used by items[0]"), "{e}");
        assert!(e.contains("items[1] (T4).id"), "{e}");
    }

    #[test]
    fn unknown_fields_and_bad_values_are_rejected() {
        assert!(parse(&item(r#","impact":"high""#)).unwrap_err().contains("unknown field"));
        assert!(parse(&item(r#","links":[{"label":"x","url":"javascript:alert(1)"}]"#))
            .unwrap_err()
            .contains("links[0].url"));
        assert!(parse(&item("").replace("2026-10-23", "2026-02-30")).unwrap_err().contains(".updated"));
        assert!(parse(&item("").replace("\"a\"", "\"Not A Slug\"")).unwrap_err().contains(".id"));
        assert!(parse("{").unwrap_err().starts_with("invalid JSON"));
    }

    #[test]
    fn dates_round_trip() {
        let d = Date::parse("2026-10-23").unwrap();
        assert_eq!(d.to_string(), "2026-10-23");
        assert_eq!(Date::from_days(d.days()), d);
        assert_eq!(Date::from_days(0), Date { y: 1970, m: 1, d: 1 });
        assert_eq!(d.short(), "23 Oct");
        assert!(Date::parse("2024-02-29").is_some());
        assert!(Date::parse("2026-13-01").is_none());
        assert!(Date::parse("26-10-23").is_none());
    }
}

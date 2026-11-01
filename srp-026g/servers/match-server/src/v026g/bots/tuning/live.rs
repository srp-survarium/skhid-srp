//! The tuning a server's bot matches run on, taken again before each match (A246): the file `SRP_BOT_TUNING` read
//! again, with the admin page's changes over it ([`BotDesk`]), so that a change applies from the next match without a
//! restart. A match keeps the tuning it started with.
//!
//! - A file that cannot be used now (unreadable, refused, or changing what the server made at its start: the maps' grid
//!   and wall clearances, the bots' kits) keeps the last good tuning; the log says why once, and the page shows it.
//! - The page sets the knobs listed here ([`KNOBS`]), each within its limits; a tuning the values together break (the
//!   skill's order, `kd_low < kd_high`: [`Tuning::validate`]) is refused whole. A knob set to its default is no change
//!   kept, and "Reset to defaults" takes a group's changes, or all of them, back out.
//! - The changes are a JSON merge patch over the file, kept in `SRP_BOT_ADMIN_TUNING` (written at each change, read
//!   when the server starts; unset: in memory only). Each change is audited: the log, and a line of JSON in the file
//!   beside it (`<name>.audit.jsonl`: when, from which address, the knob, old and new), the latest shown on the page.
//! - A changes file that cannot be used at the start (not JSON, a knob not listed or out of its limits, not fitting the
//!   tuning) is left out and said: the matches run on the tuning file alone and the next change writes it again. It is
//!   the page's own state, not the operator's file, so it does not stop the server as a bad `SRP_BOT_TUNING` does.

use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use session::LockExt;
use session::v026g::bot_desk;
use session::v026g::bot_desk::BotDesk;
use session::v026g::bot_desk::Change;
use session::v026g::bot_desk::DeskView;
use session::v026g::bot_desk::KnobState;
use session::v026g::bot_desk::Kept;
use session::v026g::bot_desk::Refused;
use session::v026g::bot_desk::Text;

use super::BotTier;
use super::LoadedTuning;
use super::Tuning;
use crate::v026g::bots::switch_log::bytes_sha;

/// Where the admin page's changes are kept.
pub const CHANGES_ENV: &str = "SRP_BOT_ADMIN_TUNING";
/// The changes the page shows, the newest first.
const AUDIT_SHOWN: usize = 50;

/// A group of knobs on the page, with a reset of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Group {
    Brackets,
    Hits,
    Balance,
}

impl Group {
    const ALL: [Group; 3] = [Group::Brackets, Group::Hits, Group::Balance];

    fn key(self) -> &'static str {
        match self {
            Group::Brackets => "brackets",
            Group::Hits => "hits",
            Group::Balance => "balance",
        }
    }

    fn title(self) -> Text {
        match self {
            Group::Brackets => Text {
                en: "Skill by the players' equipment",
                uk: "Навичка за спорядженням гравців",
            },
            Group::Hits => Text {
                en: "Hits a shot at skill 1, by weapon class",
                uk: "Влучання за постріл на навичці 1, за класом зброї",
            },
            Group::Balance => Text {
                en: "Self-balancing (the bots' K/D against humans)",
                uk: "Самобалансування (K/D ботів проти людей)",
            },
        }
    }

    /// What its shown-only knobs, or its knobs to come, wait for.
    fn note(self) -> Option<Text> {
        match self {
            Group::Balance => None,
            Group::Brackets => Some(Text {
                en: "The ceiling is one for every equipment and shown only until each band gets its own (floor <= start <= ceiling).",
                uk: "Стеля одна для всього спорядження і лише показана, доки кожен рівень не матиме свою (мінімум <= старт <= стеля).",
            }),
            Group::Hits => Some(Text {
                en: "Comes with the aim's calibration by weapon class: each class's target at skill 1, with the measured rate beside it.",
                uk: "З'явиться з калібруванням прицілу за класами зброї: ціль кожного класу на навичці 1 і виміряне поруч.",
            }),
        }
    }
}

/// What a knob takes.
#[derive(Clone, Copy, Debug)]
enum Unit {
    Number { min: f64, max: f64 },
    Whole { min: u32, max: u32 },
}

/// A knob's value, within its limits.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Value {
    Number(f64),
    Whole(u32),
}

impl Unit {
    /// `x` if it is within the limits.
    fn value(self, x: f64) -> Result<Value, String> {
        match self {
            Unit::Number { min, max } if (min..=max).contains(&x) => Ok(Value::Number(x)),
            Unit::Whole { min, max } if x.fract() == 0.0 && (f64::from(min)..=f64::from(max)).contains(&x) => {
                Ok(Value::Whole(x as u32))
            }
            _ => Err(format!("{x} is not within {}", self.limits())),
        }
    }

    /// `text` read as a value.
    fn parse(self, text: &str) -> Result<Value, String> {
        let text = text.trim();
        let x: f64 = text.parse().map_err(|_| format!("`{text}` is not a number"))?;
        self.value(x)
    }

    fn limits(self) -> String {
        match self {
            Unit::Number { min, max } => format!("{min}–{max}"),
            Unit::Whole { min, max } => format!("{min}–{max}"),
        }
    }

    /// A tuning's number as the page shows it (an `f32` as it reads, a whole as a whole).
    fn shown(self, x: f64) -> String {
        match self {
            Unit::Number { .. } => format!("{}", x as f32),
            Unit::Whole { .. } => format!("{}", x as u32),
        }
    }
}

impl Value {
    fn json(self) -> serde_json::Value {
        match self {
            Value::Number(x) => serde_json::Value::from(x),
            Value::Whole(n) => serde_json::Value::from(n),
        }
    }

    fn x(self) -> f64 {
        match self {
            Value::Number(x) => x,
            Value::Whole(n) => f64::from(n),
        }
    }
}

/// Who sets a knob.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Set {
    /// The page.
    Page,
    /// Nobody yet: the page shows it (its group's note says why).
    Later,
}

/// A number of the tuning the page shows, and sets if it may.
struct Knob {
    /// Its key in the tuning (a JSON merge patch's path, `.` between the names).
    key: &'static str,
    group: Group,
    set: Set,
    label: Text,
    unit: Unit,
    read: fn(&Tuning) -> f64,
}

impl Knob {
    fn shown(&self, tuning: &Tuning) -> String {
        self.unit.shown((self.read)(tuning))
    }

    fn settable(&self) -> bool {
        self.set == Set::Page
    }
}

/// The skill's 0 to 1.
const SHARE: Unit = Unit::Number { min: 0.0, max: 1.0 };
/// A K/D.
const KD: Unit = Unit::Number { min: 0.1, max: 10.0 };

/// The knobs of the page. The brackets are the tiers of [`super::SkillTuning`] (players of equipment 0-5 meet bots of
/// tier 4-5, and so on: [`BotTier`]): each one's floor and start, which the page sets, and their one ceiling, `skill.max`,
/// shown only until each tier gets its own (A244).
const KNOBS: &[Knob] = &[
    Knob {
        key: "skill.tiers.4-5.min",
        group: Group::Brackets,
        set: Set::Page,
        label: Text { en: "Equipment 0-5: floor", uk: "Спорядження 0–5: мінімум" },
        unit: SHARE,
        read: |t| f64::from(t.skill.of_tier(BotTier::Low).min),
    },
    Knob {
        key: "skill.tiers.4-5.start",
        group: Group::Brackets,
        set: Set::Page,
        label: Text { en: "Equipment 0-5: start", uk: "Спорядження 0–5: старт" },
        unit: SHARE,
        read: |t| f64::from(t.skill.of_tier(BotTier::Low).start),
    },
    Knob {
        key: "skill.tiers.9-10.min",
        group: Group::Brackets,
        set: Set::Page,
        label: Text { en: "Equipment 6-10: floor", uk: "Спорядження 6–10: мінімум" },
        unit: SHARE,
        read: |t| f64::from(t.skill.of_tier(BotTier::Mid).min),
    },
    Knob {
        key: "skill.tiers.9-10.start",
        group: Group::Brackets,
        set: Set::Page,
        label: Text { en: "Equipment 6-10: start", uk: "Спорядження 6–10: старт" },
        unit: SHARE,
        read: |t| f64::from(t.skill.of_tier(BotTier::Mid).start),
    },
    Knob {
        key: "skill.tiers.13-15.min",
        group: Group::Brackets,
        set: Set::Page,
        label: Text { en: "Equipment 11-15: floor", uk: "Спорядження 11–15: мінімум" },
        unit: SHARE,
        read: |t| f64::from(t.skill.of_tier(BotTier::High).min),
    },
    Knob {
        key: "skill.tiers.13-15.start",
        group: Group::Brackets,
        set: Set::Page,
        label: Text { en: "Equipment 11-15: start", uk: "Спорядження 11–15: старт" },
        unit: SHARE,
        read: |t| f64::from(t.skill.of_tier(BotTier::High).start),
    },
    Knob {
        key: "skill.max",
        group: Group::Brackets,
        set: Set::Later,
        label: Text { en: "Ceiling (every equipment)", uk: "Стеля (будь-яке спорядження)" },
        unit: SHARE,
        read: |t| f64::from(t.skill.max),
    },
    Knob {
        key: "skill.step",
        group: Group::Balance,
        set: Set::Page,
        label: Text {
            en: "Skill step at a look (the n-th step one way in a row is step / n, n up to 10)",
            uk: "Крок навички за раз (n-й крок поспіль в один бік — крок / n, n до 10)",
        },
        unit: Unit::Number { min: 0.01, max: 0.5 },
        read: |t| f64::from(t.skill.step),
    },
    Knob {
        key: "skill.kd_low",
        group: Group::Balance,
        set: Set::Page,
        label: Text {
            en: "The bots get better while their K/D against humans is under",
            uk: "Боти кращають, поки їхній K/D проти людей нижче",
        },
        unit: KD,
        read: |t| f64::from(t.skill.kd_low),
    },
    Knob {
        key: "skill.kd_high",
        group: Group::Balance,
        set: Set::Page,
        label: Text {
            en: "The bots get worse while their K/D against humans is over",
            uk: "Боти слабшають, поки їхній K/D проти людей вище",
        },
        unit: KD,
        read: |t| f64::from(t.skill.kd_high),
    },
    Knob {
        key: "skill.every_s",
        group: Group::Balance,
        set: Set::Page,
        label: Text { en: "A look every (seconds)", uk: "Перевірка кожні (секунд)" },
        unit: Unit::Whole { min: 5, max: 600 },
        read: |t| f64::from(t.skill.every_s),
    },
    Knob {
        key: "skill.window_s",
        group: Group::Balance,
        set: Set::Page,
        label: Text { en: "Over the last (seconds)", uk: "За останні (секунд)" },
        unit: Unit::Whole { min: 30, max: 1800 },
        read: |t| f64::from(t.skill.window_s),
    },
    Knob {
        key: "skill.min_events",
        group: Group::Balance,
        set: Set::Page,
        label: Text {
            en: "Fewest kills and deaths in that time to act on",
            uk: "Найменше вбивств і смертей за цей час, щоб діяти",
        },
        unit: Unit::Whole { min: 1, max: 50 },
        read: |t| f64::from(t.skill.min_events),
    },
];

fn knob(key: &str) -> Option<&'static Knob> {
    KNOBS.iter().find(|knob| knob.key == key)
}

/// The page's changes: each knob set, by its key.
type Changes = BTreeMap<&'static str, Value>;

/// The changes as a JSON merge patch over the tuning.
fn patch_of(changes: &Changes) -> serde_json::Value {
    let mut patch = serde_json::Value::Object(serde_json::Map::new());
    for (key, value) in changes {
        let leaf = key.rsplit('.').fold(value.json(), |inner, name| {
            serde_json::Value::Object(serde_json::Map::from_iter([(name.to_owned(), inner)]))
        });
        super::merge_patch(&mut patch, leaf);
    }
    patch
}

/// The changes file's bytes: the patch, pretty, as an operator's file would be.
fn bytes_of(patch: &serde_json::Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(patch).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

/// The changes kept at `path` (none if there is no file): only knobs the page sets, each within its limits.
fn read_changes(path: &Path) -> Result<Changes, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Changes::new()),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let patch: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: not JSON: {e}", path.display()))?;
    let mut leaves = Vec::new();
    leaves_of(&patch, String::new(), &mut leaves);
    let mut changes = Changes::new();
    for (key, leaf) in leaves {
        let knob = knob(&key)
            .filter(|knob| knob.settable())
            .ok_or_else(|| format!("{}: `{key}` is not a knob the page sets", path.display()))?;
        let x = leaf.as_f64().ok_or_else(|| format!("{}: {key}: not a number", path.display()))?;
        let value = knob.unit.value(x).map_err(|e| format!("{}: {key}: {e}", path.display()))?;
        changes.insert(knob.key, value);
    }
    Ok(changes)
}

/// Each value of a patch that is not an object, with its key (`.` between the names).
fn leaves_of<'a>(value: &'a serde_json::Value, at: String, out: &mut Vec<(String, &'a serde_json::Value)>) {
    match value {
        serde_json::Value::Object(map) => {
            for (name, inner) in map {
                let key = if at.is_empty() { name.clone() } else { format!("{at}.{name}") };
                leaves_of(inner, key, out);
            }
        }
        leaf => out.push((at, leaf)),
    }
}

/// `changes` written to `path` whole: to a file beside it (the same file system), synced, then put in its place, so a
/// crash leaves the old file or the new one, never a part.
fn write_changes(path: &Path, changes: &Changes) -> Result<(), String> {
    write_whole(path, &bytes_of(&patch_of(changes))).map_err(|e| format!("{}: {e}", path.display()))
}

fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let next = path.with_extension("json.new");
    let mut file = std::fs::File::create(&next)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&next, path)?;
    // The rename made lasting too, where a folder opens as a file (Linux).
    if let Some(dir) = path.parent().and_then(|dir| std::fs::File::open(dir).ok()) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// The audit's file beside the changes file.
fn audit_path(changes: &Path) -> PathBuf {
    changes.with_extension("audit.jsonl")
}

/// The latest changes of the audit's file, the newest first (none if there is no file).
fn read_audit(path: &Path) -> VecDeque<Change> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return VecDeque::new();
    };
    let field = |line: &serde_json::Value, name: &str| line[name].as_str().unwrap_or_default().to_owned();
    text.lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .take(AUDIT_SHOWN)
        .map(|line| Change {
            at: line["at"].as_u64().unwrap_or_default(),
            from: field(&line, "from"),
            key: field(&line, "key"),
            old: field(&line, "old"),
            new: field(&line, "new"),
        })
        .collect()
}

/// `file` with the page's `changes`, checked.
fn combine(file: &LoadedTuning, changes: &Changes) -> Result<LoadedTuning, String> {
    if changes.is_empty() {
        return Ok(file.clone());
    }
    let patch = patch_of(changes);
    let mut value = serde_json::to_value(&**file.tuning()).map_err(|e| e.to_string())?;
    Tuning::apply_patch(&mut value, patch.clone())?;
    let tuning = Tuning::from_value(value).map_err(|e| format!("the admin page's changes: {e}"))?;
    Ok(file.changed(bytes_sha(&bytes_of(&patch)), tuning))
}

/// What the server made with the tuning it started on, which a match cannot change: the maps' grid and their wall
/// clearances ([`crate::v026g::bot_maps`]), and the bots' kits (the lobby's pockets).
fn fits(start: &Tuning, tuning: &Tuning) -> Result<(), String> {
    if tuning.grid != start.grid || tuning.needs_wall_clearance() != start.needs_wall_clearance() {
        return Err("it changes the maps' grid (`grid`, `think.wall_clear_corners`), which needs a restart".to_owned());
    }
    if tuning.kit != start.kit {
        return Err("it changes the bots' kits (`kit`), which needs a restart".to_owned());
    }
    Ok(())
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// The tuning of a server's bot matches: what it started on, and what the next match runs on.
pub struct LiveTuning {
    /// What the server started on: the maps and the kits were made with it.
    start: LoadedTuning,
    /// `SRP_BOT_ADMIN_TUNING`: where the page's changes are kept; `None`: in memory only.
    changes_file: Option<PathBuf>,
    state: Mutex<State>,
}

struct State {
    /// The file's tuning last read and taken (or the defaults).
    file: LoadedTuning,
    changes: Changes,
    /// What the next match runs on: `file` with `changes`.
    good: LoadedTuning,
    /// Why the files are not what the matches run on, said once in the log.
    refused: Option<Refused>,
    /// The latest changes, the newest first.
    audit: VecDeque<Change>,
}

impl State {
    /// `why` the matches keep the last good tuning, logged when it is new.
    fn refuse(&mut self, why: String) {
        if self.refused.as_ref().is_some_and(|refused| refused.why == why) {
            return;
        }
        log::warn!("Bots tuning: {why}; the matches keep tuning {:08x}", self.good.tuning().hash());
        self.refused = Some(Refused { at: unix_now(), why });
    }
}

impl Default for LiveTuning {
    /// The defaults, and changes in memory only.
    fn default() -> Self {
        Self::new(LoadedTuning::default(), None)
    }
}

impl std::fmt::Debug for LiveTuning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.locked();
        f.debug_struct("LiveTuning")
            .field("start", &self.start.data_line())
            .field("next", &state.good.data_line())
            .field("changes_file", &self.changes_file)
            .finish()
    }
}

/// A server's one store: equal to itself only (as `Maps`).
impl PartialEq for LiveTuning {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Eq for LiveTuning {}

impl LiveTuning {
    /// `SRP_BOT_TUNING` (one the server cannot use is the error, [`LoadedTuning::from_env`]) and the page's changes
    /// kept at `SRP_BOT_ADMIN_TUNING`.
    pub fn from_env() -> Result<Self, String> {
        let file = LoadedTuning::from_env()?;
        let changes_file = std::env::var_os(CHANGES_ENV).filter(|path| !path.is_empty()).map(PathBuf::from);
        Ok(Self::new(file, changes_file))
    }

    /// The tuning `file` with the changes kept at `changes_file`, if they can be used.
    pub fn new(file: LoadedTuning, changes_file: Option<PathBuf>) -> Self {
        let read = match &changes_file {
            Some(path) => read_changes(path).and_then(|changes| Ok((combine(&file, &changes)?, changes))),
            None => Ok((file.clone(), Changes::new())),
        };
        let (good, changes, refused) = match read {
            Ok((good, changes)) => (good, changes, None),
            Err(why) => {
                let why = format!("{why}; left out");
                log::error!("Bots tuning: the admin page's changes: {why}");
                (file.clone(), Changes::new(), Some(Refused { at: unix_now(), why }))
            }
        };
        if !changes.is_empty() {
            log::info!("Bots tuning: the admin page's changes {} over {}", patch_of(&changes), file.data_line());
        }
        let audit = changes_file.as_deref().map(|path| read_audit(&audit_path(path))).unwrap_or_default();
        Self {
            start: good.clone(),
            changes_file,
            state: Mutex::new(State { file, changes, good, refused, audit }),
        }
    }

    /// The tuning the server started on (the maps and the bots' kits are made with it).
    pub fn at_start(&self) -> &Arc<Tuning> {
        self.start.tuning()
    }

    /// The tuning for the match that starts now: the file read again (a change taken if it can be used, else the last
    /// good kept), with the page's changes.
    pub fn for_match(&self) -> LoadedTuning {
        let mut state = self.state.locked();
        if let Some((path, _)) = self.start.file() {
            match std::fs::read(path) {
                Err(error) => state.refuse(format!("SRP_BOT_TUNING {}: {error}", path.display())),
                Ok(bytes) => {
                    let sha = bytes_sha(&bytes);
                    if state.file.file().is_some_and(|(_, taken)| taken == sha) {
                        if state.refused.take().is_some() {
                            log::info!("Bots tuning: SRP_BOT_TUNING {} is back to {sha}", path.display());
                        }
                    } else {
                        let taken = LoadedTuning::from_bytes(path, &bytes).and_then(|file| {
                            let good = combine(&file, &state.changes)?;
                            fits(self.start.tuning(), good.tuning())
                                .map_err(|why| format!("SRP_BOT_TUNING {} {sha}: {why}", path.display()))?;
                            Ok((file, good))
                        });
                        match taken {
                            Ok((file, good)) => {
                                log::info!(
                                    "Bots tuning: SRP_BOT_TUNING {} is now {sha}: tuning {:08x} from this match",
                                    path.display(),
                                    good.tuning().hash()
                                );
                                (state.file, state.good, state.refused) = (file, good, None);
                            }
                            Err(why) => state.refuse(why),
                        }
                    }
                }
            }
        }
        state.good.clone()
    }

    /// `changes` made what the next match runs on, kept and audited; nothing changes if they cannot be used.
    fn commit(&self, state: &mut State, changes: Changes, from: &str) -> Result<Vec<Change>, String> {
        if changes == state.changes {
            return Ok(Vec::new());
        }
        let good = combine(&state.file, &changes)?;
        fits(self.start.tuning(), good.tuning())?;
        if let Some(path) = &self.changes_file {
            write_changes(path, &changes)?;
        }
        let at = unix_now();
        let made: Vec<Change> = KNOBS
            .iter()
            .filter_map(|knob| {
                let (old, new) = (knob.shown(state.good.tuning()), knob.shown(good.tuning()));
                (old != new).then(|| Change { at, from: from.to_owned(), key: knob.key.to_owned(), old, new })
            })
            .collect();
        for change in &made {
            log::info!(
                "Bots tuning: {} {} -> {} (the admin page, from {}): tuning {:08x} from the next match",
                change.key,
                change.old,
                change.new,
                change.from,
                good.tuning().hash()
            );
        }
        if let Some(path) = &self.changes_file {
            let lines: String = made
                .iter()
                .map(|c| {
                    let line = serde_json::json!({"at": c.at, "from": c.from, "key": c.key, "old": c.old, "new": c.new});
                    format!("{line}\n")
                })
                .collect();
            let audit = audit_path(path);
            let appended = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&audit)
                .and_then(|mut file| file.write_all(lines.as_bytes()));
            if let Err(error) = appended {
                log::error!("Bots tuning: the audit {}: {error} (the log above has the changes)", audit.display());
            }
        }
        for change in &made {
            state.audit.push_front(change.clone());
        }
        state.audit.truncate(AUDIT_SHOWN);
        (state.changes, state.good) = (changes, good);
        Ok(made)
    }
}

impl BotDesk for LiveTuning {
    fn view(&self) -> DeskView {
        let state = self.state.locked();
        let groups = Group::ALL
            .into_iter()
            .map(|group| bot_desk::Group {
                key: group.key(),
                title: group.title(),
                note: group.note(),
                knobs: KNOBS
                    .iter()
                    .filter(|knob| knob.group == group)
                    .map(|knob| bot_desk::Knob {
                        key: knob.key.to_owned(),
                        label: knob.label,
                        state: if knob.settable() { KnobState::Settable } else { KnobState::Shown },
                        value: knob.shown(state.good.tuning()),
                        default: knob.shown(state.file.tuning()),
                        limits: knob.unit.limits(),
                    })
                    .collect(),
            })
            .collect();
        DeskView {
            source: match state.file.file() {
                Some((path, sha)) => format!("SRP_BOT_TUNING {} ({sha})", path.display()),
                None => "the shipped defaults".to_owned(),
            },
            kept: match &self.changes_file {
                Some(path) => Kept::File(path.display().to_string()),
                None => Kept::Memory,
            },
            refused: state.refused.clone(),
            groups,
            audit: state.audit.iter().cloned().collect(),
        }
    }

    fn set(&self, asked: &[(String, String)], from: &str) -> Result<Vec<Change>, String> {
        let mut state = self.state.locked();
        let mut changes = state.changes.clone();
        for (key, text) in asked {
            let knob = knob(key).ok_or_else(|| format!("{key}: no such knob"))?;
            if !knob.settable() {
                return Err(format!("{key}: not settable yet"));
            }
            let value = knob.unit.parse(text).map_err(|why| format!("{key}: {why}"))?;
            if knob.unit.shown(value.x()) == knob.shown(state.file.tuning()) {
                changes.remove(knob.key);
            } else {
                changes.insert(knob.key, value);
            }
        }
        self.commit(&mut state, changes, from)
    }

    fn reset(&self, group: Option<&str>, from: &str) -> Result<Vec<Change>, String> {
        let group = match group {
            Some(key) => Some(Group::ALL.into_iter().find(|group| group.key() == key).ok_or_else(|| format!("{key}: no such group"))?),
            None => None,
        };
        let mut state = self.state.locked();
        let kept = state
            .changes
            .iter()
            .filter(|(key, _)| group.is_some_and(|group| knob(key).is_some_and(|knob| knob.group != group)))
            .map(|(key, value)| (*key, *value))
            .collect();
        self.commit(&mut state, kept, from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of its own for a test's files.
    fn dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("srp-live-tuning-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn set(desk: &LiveTuning, pairs: &[(&str, &str)]) -> Result<Vec<Change>, String> {
        let pairs: Vec<(String, String)> = pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        desk.set(&pairs, "192.0.2.1")
    }

    #[test]
    fn with_no_file_and_no_changes_every_match_runs_on_the_defaults() {
        let live = LiveTuning::default();
        let loaded = live.for_match();
        assert_eq!(loaded, LoadedTuning::default());
        assert_eq!(loaded.data_line(), "tuning=defaults");
        assert_eq!(loaded.tuning().hash(), Tuning::default().hash());
        assert_eq!(live.at_start().hash(), Tuning::default().hash());
        let view = live.view();
        assert_eq!((view.source.as_str(), &view.kept, &view.refused), ("the shipped defaults", &Kept::Memory, &None));
        // Every knob reads what the defaults say, and a reset with no change changes nothing.
        let knobs: Vec<_> = view.groups.iter().flat_map(|group| &group.knobs).collect();
        assert_eq!(knobs.len(), KNOBS.len());
        assert!(knobs.iter().all(|knob| knob.value == knob.default));
        assert_eq!(live.reset(None, "192.0.2.1"), Ok(Vec::new()));
        // A tuning made into JSON and read again is the same, so a change and its reset give back the same hash.
        let value = serde_json::to_value(Tuning::default()).unwrap();
        assert_eq!(Tuning::from_value(value).unwrap().hash(), Tuning::default().hash());
    }

    #[test]
    fn a_changed_file_applies_from_the_next_match_and_one_that_cannot_be_used_keeps_the_last_good() {
        let dir = dir("file");
        let path = dir.join("bots.json");
        std::fs::write(&path, r#"{"skill":{"step":0.2}}"#).unwrap();
        let live = LiveTuning::new(LoadedTuning::from_file(&path).unwrap(), None);
        assert_eq!(live.for_match().tuning().skill.step, 0.2);
        std::fs::write(&path, r#"{"skill":{"step":0.3}}"#).unwrap();
        let taken = live.for_match();
        assert_eq!(taken.tuning().skill.step, 0.3);
        assert_eq!(taken, LoadedTuning::from_file(&path).unwrap());
        // Not JSON, a value refused, a change of the maps' grid or the kits, no file: the last good, and the page says why.
        for (bad, why) in [
            ("{", "not JSON"),
            (r#"{"skill":{"kd_low":2,"kd_high":1}}"#, "skill.kd_*"),
            (r#"{"grid":{"gap_links":false}}"#, "needs a restart"),
            (r#"{"kit":{"pocket_store":false}}"#, "needs a restart"),
        ] {
            std::fs::write(&path, bad).unwrap();
            assert_eq!(live.for_match(), taken, "{bad}");
            let refused = live.view().refused.expect("said");
            assert!(refused.why.contains(why), "{bad}: {}", refused.why);
        }
        std::fs::remove_file(&path).unwrap();
        assert_eq!(live.for_match(), taken);
        // Back to the file taken: nothing is refused.
        std::fs::write(&path, r#"{"skill":{"step":0.3}}"#).unwrap();
        assert_eq!(live.for_match(), taken);
        assert_eq!(live.view().refused, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_page_sets_its_knobs_within_their_limits_all_or_none() {
        let live = LiveTuning::default();
        let made = set(&live, &[("skill.step", "0.2"), ("skill.every_s", " 60 ")]).unwrap();
        let said: Vec<_> = made.iter().map(|c| (c.key.as_str(), c.old.as_str(), c.new.as_str(), c.from.as_str())).collect();
        assert_eq!(said, [("skill.step", "0.15", "0.2", "192.0.2.1"), ("skill.every_s", "30", "60", "192.0.2.1")]);
        let next = live.for_match();
        assert_eq!((next.tuning().skill.step, next.tuning().skill.every_s), (0.2, 60));
        assert!(next.data_line().starts_with("tuning=defaults admin=sha256:"), "{}", next.data_line());
        // The same values again change nothing.
        assert_eq!(set(&live, &[("skill.step", "0.20")]), Ok(Vec::new()));
        // Refused whole: out of the limits, not a number, not whole, the K/D's order, a bracket's order (floor <= start <=
        // ceiling), a knob not settable yet (the ceiling) or none.
        for bad in [
            &[("skill.step", "0.25"), ("skill.window_s", "10")][..],
            &[("skill.step", "0.25"), ("skill.kd_low", "a lot")],
            &[("skill.every_s", "30.5")],
            &[("skill.kd_low", "2"), ("skill.kd_high", "1.9")],
            &[("skill.tiers.4-5.start", "1.2")],
            &[("skill.tiers.9-10.min", "0.8")],
            &[("skill.tiers.13-15.start", "0.6")],
            &[("skill.max", "0.9")],
            &[("aim.error_deg", "1")],
        ] {
            assert!(set(&live, bad).is_err(), "{bad:?}");
            assert_eq!(live.for_match(), next, "{bad:?}");
        }
        // A knob set back to its default is no change kept; the audit has every change, the newest first.
        set(&live, &[("skill.step", "0.15")]).unwrap();
        let view = live.view();
        let keys: Vec<_> = view.audit.iter().map(|c| (c.key.as_str(), c.new.as_str())).collect();
        assert_eq!(keys, [("skill.step", "0.15"), ("skill.every_s", "60"), ("skill.step", "0.2")]);
        assert_eq!(patch_of(&live.state.locked().changes), serde_json::json!({"skill": {"every_s": 60}}));
    }

    /// The brackets as the owner set them (2026-11-01T16:30:58Z: equipment 0-5 floor 0.0 start 0.3, 6-10 0.2 / 0.5, 11-15
    /// 0.3 / 0.6) can be put in from the page; a match's team then starts there.
    #[test]
    fn the_page_sets_the_brackets_floor_and_start_in_their_order() {
        let live = LiveTuning::default();
        let made = set(
            &live,
            &[
                ("skill.tiers.4-5.min", "0"),
                ("skill.tiers.4-5.start", "0.3"),
                ("skill.tiers.9-10.min", "0.2"),
                ("skill.tiers.9-10.start", "0.5"),
                ("skill.tiers.13-15.min", "0.3"),
                ("skill.tiers.13-15.start", "0.6"),
            ],
        )
        .unwrap();
        assert_eq!(made.len(), 6);
        let next = live.for_match();
        let rows: Vec<_> = BotTier::ALL.iter().map(|&tier| next.tuning().skill.of_tier(tier)).map(|row| (row.min, row.start)).collect();
        assert_eq!(rows, [(0.0, 0.3), (0.2, 0.5), (0.3, 0.6)]);
        // The start may go under its old floor when the floor goes with it, in one save.
        set(&live, &[("skill.tiers.4-5.min", "0.05"), ("skill.tiers.4-5.start", "0.1")]).unwrap();
        assert_eq!(live.for_match().tuning().skill.of_tier(BotTier::Low).start, 0.1);
        // The view: the floors and starts settable, the ceiling shown only, the group's note says why.
        let view = live.view();
        let brackets = view.groups.iter().find(|group| group.key == "brackets").unwrap();
        assert!(brackets.note.is_some());
        let shown: Vec<_> = brackets.knobs.iter().filter(|knob| knob.state == KnobState::Shown).map(|knob| knob.key.as_str()).collect();
        assert_eq!(shown, ["skill.max"]);
        // The group's reset puts the shipped brackets back.
        assert_eq!(live.reset(Some("brackets"), "192.0.2.3").unwrap().len(), 6);
        assert_eq!(live.for_match(), LoadedTuning::default());
    }

    #[test]
    fn the_changes_are_kept_in_their_file_audited_beside_it_and_reset_by_group() {
        let dir = dir("kept");
        let changes = dir.join("bot-admin-tuning.json");
        let live = LiveTuning::new(LoadedTuning::default(), Some(changes.clone()));
        set(&live, &[("skill.kd_low", "0.8"), ("skill.kd_high", "1.25")]).unwrap();
        let bytes = std::fs::read(&changes).unwrap();
        assert_eq!(serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(), serde_json::json!({"skill": {"kd_high": 1.25, "kd_low": 0.8}}));
        let next = live.for_match();
        assert_eq!(next.data_line(), format!("tuning=defaults admin={}", bytes_sha(&bytes)));
        // A server started again reads them, with the audit.
        let again = LiveTuning::new(LoadedTuning::default(), Some(changes.clone()));
        assert_eq!(again.for_match(), next);
        assert_eq!(again.at_start().hash(), next.tuning().hash());
        assert_eq!(again.view().audit, live.view().audit);
        assert_eq!(again.view().kept, Kept::File(changes.display().to_string()));
        // A group's reset takes its changes out, logged like any change; another group's reset changes nothing.
        assert_eq!(again.reset(Some("brackets"), "192.0.2.2"), Ok(Vec::new()));
        assert!(again.reset(Some("nope"), "192.0.2.2").is_err());
        let made = again.reset(Some("balance"), "192.0.2.2").unwrap();
        let said: Vec<_> = made.iter().map(|c| (c.key.as_str(), c.old.as_str(), c.new.as_str())).collect();
        assert_eq!(said, [("skill.kd_low", "0.8", "0.67"), ("skill.kd_high", "1.25", "1.5")]);
        assert_eq!(again.for_match(), LoadedTuning::default());
        assert_eq!(std::fs::read_to_string(&changes).unwrap(), "{}\n");
        let audit = std::fs::read_to_string(audit_path(&changes)).unwrap();
        assert_eq!(audit.lines().count(), 4);
        assert!(audit.lines().last().unwrap().contains(r#""from":"192.0.2.2""#), "{audit}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_changes_file_that_cannot_be_used_is_left_out_and_said() {
        let dir = dir("bad");
        let changes = dir.join("bot-admin-tuning.json");
        for (bad, why) in [
            ("[", "not JSON"),
            (r#"{"skill":{"step":0.9}}"#, "skill.step"),
            (r#"{"skill":{"step":0.2,"nope":1}}"#, "`skill.nope` is not a knob the page sets"),
            (r#"{"aim":{"error_deg":1}}"#, "`aim.error_deg` is not a knob the page sets"),
            (r#"{"skill":{"max":0.9}}"#, "`skill.max` is not a knob the page sets"),
            (r#"{"skill":{"every_s":"60"}}"#, "skill.every_s: not a number"),
            (r#"{"skill":{"kd_low":2}}"#, "skill.kd_*"),
        ] {
            std::fs::write(&changes, bad).unwrap();
            let live = LiveTuning::new(LoadedTuning::default(), Some(changes.clone()));
            assert_eq!(live.for_match(), LoadedTuning::default(), "{bad}");
            let refused = live.view().refused.expect("said");
            assert!(refused.why.contains(why), "{bad}: {}", refused.why);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

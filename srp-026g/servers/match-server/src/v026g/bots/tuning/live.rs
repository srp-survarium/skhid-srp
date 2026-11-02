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
use session::v026g::bot_desk::ClassLine;
use session::v026g::bot_desk::ClassRates;
use session::v026g::bot_desk::DeskView;
use session::v026g::bot_desk::Edit;
use session::v026g::bot_desk::KnobState;
use session::v026g::bot_desk::Kept;
use session::v026g::bot_desk::Preview;
use session::v026g::bot_desk::RatePoint;
use session::v026g::bot_desk::Rates;
use session::v026g::bot_desk::Refused;
use session::v026g::bot_desk::Text;

use super::BotTier;
use super::TypeCalibration;
use super::Check;
use crate::v026g::bots::guns::WeaponClass;
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
                en: "Each band's floor <= start <= ceiling, within the scale's top (shown only); a team starts at its band's start and the self-balancing keeps it between the floor and the ceiling.",
                uk: "Для кожного рівня мінімум <= старт <= стеля, у межах верху шкали (лише показано); команда починає зі старту свого рівня, а самобалансування тримає її між мінімумом і стелею.",
            }),
            Group::Hits => Some(Text {
                en: "Each class's hit rate a shot at skill 1, %, set as its skill-1 aim factor through the bench's calibration (its source below); the skill-0 and 0.5 factors stay, so the line keeps rising. Refused outside the bench's points, or under the lowest rate that keeps the line rising; the owner's bar is shown beside the limits, and a target may go over it. A type on the pool's numbers (the automatics') says so until its own are measured. Pistols have no bench points: their factors are shown only.",
                uk: "Влучання за постріл на навичці 1 для кожного класу, %, задається як фактор прицілу на навичці 1 за калібруванням стенду (джерело нижче); фактори навичок 0 і 0.5 лишаються, тож лінія росте. Відмова поза точками стенду або нижче найменшої ставки, з якою лінія росте; планку власника показано біля меж, і ціль може бути вище неї. Тип на спільних числах (автоматичної зброї) так і позначено, доки його власні не виміряно. Пістолети не мають точок стенду: їхні фактори лише показано.",
            }),
        }
    }
}

/// What a knob takes.
#[derive(Clone, Copy, Debug)]
enum Unit {
    Number { min: f64, max: f64 },
    Whole { min: u32, max: u32 },
    /// A class's hit rate a shot at skill 1, %, within the bench's points, set as its skill-1 factor through the bench's
    /// calibration ([`super::AimCalibration`], the `calibration` section, with the owner's bar).
    Hits { class: WeaponClass },
}

/// A knob's value, within its limits.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Value {
    Number(f64),
    Whole(u32),
    /// A class's factor at skill 1 (`aim.class_aim.<class>`'s last).
    Factor(f32),
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

    /// `text` read as a number (a rate may end in `%`).
    fn number(text: &str) -> Result<f64, String> {
        let text = text.trim();
        text.trim_end_matches('%').trim().parse().map_err(|_| format!("`{text}` is not a number"))
    }

    fn limits(self) -> String {
        match self {
            Unit::Number { min, max } => format!("{min}–{max}"),
            Unit::Whole { min, max } => format!("{min}–{max}"),
            Unit::Hits { .. } => "%".to_owned(),
        }
    }

    /// A tuning's number as the page shows it (an `f32` as it reads, a whole as a whole, a rate to a tenth; none: `—`).
    fn shown(self, x: f64) -> String {
        match self {
            _ if x.is_nan() => "—".to_owned(),
            Unit::Number { .. } => format!("{}", x as f32),
            Unit::Whole { .. } => format!("{}", x as u32),
            Unit::Hits { .. } => format!("{x:.1}"),
        }
    }
}

impl Value {
    fn x(self) -> f64 {
        match self {
            Value::Number(x) => x,
            Value::Whole(n) => f64::from(n),
            Value::Factor(f) => f64::from(f),
        }
    }
}

/// An `f32` in JSON as it reads (0.4, not 0.4000000059604645).
fn short(x: f32) -> serde_json::Value {
    format!("{x}").parse::<f64>().map_or(serde_json::Value::Null, serde_json::Value::from)
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
    /// Its number in a tuning (a rate: the calibration's at the class's skill-1 factor; NaN for none).
    read: fn(&Tuning) -> f64,
}

impl Knob {
    fn shown(&self, tuning: &Tuning) -> String {
        self.unit.shown((self.read)(tuning))
    }

    fn settable(&self) -> bool {
        self.set == Set::Page
    }

    /// The page's `text` as this knob's value over the tuning `file`: a number within the knob's limits; a rate within
    /// the class's usable span ([`Knob::usable`]), as the factor that gives it.
    fn value_of(&self, text: &str, file: &Tuning) -> Result<Value, String> {
        let x = Unit::number(text)?;
        let Unit::Hits { class } = self.unit else {
            return self.unit.value(x);
        };
        let calibration = file.calibration.of(class).ok_or("the class has no bench points")?;
        if x.is_nan() || x <= 0.0 {
            return Err(format!("{x} % is not over 0"));
        }
        if let Some((low, _)) = self.usable(file).filter(|&(low, _)| x < f64::from(low)) {
            let (_, middle, _) = file.aim.class_aim.of(class);
            return Err(format!(
                "{x} % is under {low:.1} %, the lowest that keeps the class's line rising with its skill-0.5 factor {middle}"
            ));
        }
        let factor = calibration.factor_for(x as f32).ok_or_else(|| {
            let (low, high) = calibration.rates().unwrap_or_default();
            format!("{x} % is outside the bench's points ({low:.1}–{high:.1} %)")
        })?;
        Ok(Value::Factor(factor))
    }

    /// `value` as the page shows it, over the tuning `file` (a factor as the rate it gives).
    fn shown_value(&self, value: Value, file: &Tuning) -> String {
        match (self.unit, value) {
            (Unit::Hits { class }, Value::Factor(factor)) => {
                let rate = file.calibration.of(class).and_then(|calibration| calibration.at_skill_1(factor));
                self.unit.shown(rate.map_or(f64::NAN, f64::from))
            }
            (unit, value) => unit.shown(value.x()),
        }
    }

    /// `value` in the changes' patch over the tuning `file`: a factor as its class's three, the file's at skill 0 and 0.5
    /// with it at skill 1, so the class's line keeps its lower points.
    fn json(&self, value: Value, file: &Tuning) -> serde_json::Value {
        match (self.unit, value) {
            (Unit::Hits { class }, Value::Factor(factor)) => {
                let (low, middle, _) = file.aim.class_aim.of(class);
                serde_json::Value::Array(vec![short(low), short(middle), short(factor)])
            }
            (_, Value::Number(x)) => serde_json::Value::from(x),
            (_, Value::Whole(n)) => serde_json::Value::from(n),
            (_, Value::Factor(f)) => short(f),
        }
    }

    /// A value of the changes file: a number, or a class's three factors (its skill-1 one kept).
    fn value_of_json(&self, leaf: &serde_json::Value) -> Result<Value, String> {
        if let Unit::Hits { .. } = self.unit {
            let three: Vec<f64> =
                leaf.as_array().map(|all| all.iter().filter_map(serde_json::Value::as_f64).collect()).unwrap_or_default();
            return match three[..] {
                [low, middle, high] if [low, middle, high].iter().all(|f| f.is_finite() && *f > 0.0) => {
                    Ok(Value::Factor(high as f32))
                }
                _ => Err("not three factors over 0".to_owned()),
            };
        }
        self.unit.value(leaf.as_f64().ok_or("not a number")?)
    }

    /// The values it takes, over the tuning `file` (a rate: the class's usable span, [`Knob::usable`], and the owner's bar).
    fn limits(&self, file: &Tuning) -> String {
        match self.unit {
            Unit::Hits { class } => match (self.usable(file), file.calibration.of(class)) {
                (Some((low, high)), Some(calibration)) => {
                    format!("{low:.1}–{high:.1} % (default bar {} %)", calibration.bar_text())
                }
                _ => "—".to_owned(),
            },
            unit => unit.limits(),
        }
    }

    /// The rates a class's row can be set to over the tuning `file`: from the rate at the widest skill-1 factor that
    /// keeps its line rising with the file's skill-0 and 0.5 factors ([`super::AimTuning`]'s check: within a half the
    /// factor may grow at most as fast as the wander shrinks; fx-gear, 2026-11-02) to the calibration's top (over the
    /// owner's bar too, A267). `None`: no points, or not even the calibration's narrowest factor keeps it rising.
    fn usable(&self, file: &Tuning) -> Option<(f32, f32)> {
        let Unit::Hits { class } = self.unit else {
            return None;
        };
        let calibration = file.calibration.of(class)?;
        let points = &calibration.points;
        let (narrowest, widest) = (points.0.first()?.0, points.0.last()?.0);
        let rises = |factor: f32| {
            let mut aim = file.aim.clone();
            aim.class_aim.of_mut(class).2 = factor;
            aim.check().is_ok()
        };
        if !rises(narrowest) {
            return None;
        }
        let widest = if rises(widest) {
            widest
        } else {
            let (mut good, mut bad) = (narrowest, widest);
            for _ in 0..32 {
                let middle = (good + bad) / 2.0;
                if rises(middle) { good = middle } else { bad = middle }
            }
            good
        };
        let (_, top) = calibration.rates()?;
        Some((calibration.at_skill_1(widest)?, top))
    }

    /// What the page shows beside the value: a class's factors in `tuning`.
    fn beside(&self, tuning: &Tuning) -> Option<String> {
        let Unit::Hits { class } = self.unit else {
            return None;
        };
        let (low, middle, high) = tuning.aim.class_aim.of(class);
        let factors = format!("factor at skill 1: {high} (skill 0 / 0.5: {low} / {middle})");
        let calibration = match tuning.calibration.type_of(class) {
            TypeCalibration::NotMeasured => {
                return Some(format!("factors {low} / {middle} / {high} (skill 0 / 0.5 / 1); no bench points"));
            }
            TypeCalibration::Own(numbers) => numbers,
            TypeCalibration::Pooled(numbers) => {
                let off = bar_note(numbers, high).map_or(String::new(), |note| format!("; {note}"));
                return Some(format!("{factors}; not measured on its own yet: partly the pool's numbers{off}"));
            }
        };
        Some(match bar_note(calibration, high) {
            Some(note) => format!("{factors}; {note}"),
            None => factors,
        })
    }
}

/// [A267] What the page says of a class's rate at skill 1 with the skill-1 factor `high` against its bar, when it is off it
/// (a default line may be: the page keeps and says it, never refuses it): `21.6 % at skill 1, above its bar 18–20 %`.
fn bar_note(calibration: &super::ClassCalibration, high: f32) -> Option<String> {
    let rate = calibration.at_skill_1(high)?;
    let side = calibration.against_bar(rate)?;
    Some(format!("{rate:.1} % at skill 1, {side} its bar {} %", calibration.bar_text()))
}

/// A class's rate at skill 1 in a tuning: its calibration's at its skill-1 factor (NaN for none).
fn hits(tuning: &Tuning, class: WeaponClass) -> f64 {
    let (_, _, factor) = tuning.aim.class_aim.of(class);
    tuning.calibration.of(class).and_then(|calibration| calibration.at_skill_1(factor)).map_or(f64::NAN, f64::from)
}

/// [A266] How the page's expected rates are had from the bench's numbers.
const METHOD: Text = Text {
    en: "Expected hits a shot, % (a shotgun: a pellet), on each class's line: the bench's ladder (A243, measured at skill 0, 0.2, 0.4, 0.6, 0.8 and 1 with the shipped factors), straight between its steps; a skill-1 rate set otherwise moves the upper half by the dial's own weight there, 2 × skill − 1 (none of it at 0.5 and under, all of it at 1). Estimates from the bench, weighed to the live humans' shots by distance and target speed; not live measurements.",
    uk: "Очікувані влучання за постріл, % (дробовик: за дробину), на лінії кожного класу: драбина стенду (A243, виміряна на навичці 0, 0.2, 0.4, 0.6, 0.8 і 1 з типовими факторами), пряма між її кроками; інша ставка на навичці 1 зсуває верхню половину з вагою самого регулятора, 2 × навичка − 1 (нічого на 0.5 і нижче, повністю на 1). Оцінки зі стенду, зважені за пострілами живих гравців за відстанню й швидкістю цілі; не живі виміри.",
};

/// A band of the players' equipment, as the page names it.
fn band(tier: BotTier) -> Text {
    match tier {
        BotTier::Low => Text { en: "Equipment 0-5", uk: "Спорядження 0–5" },
        BotTier::Mid => Text { en: "Equipment 6-10", uk: "Спорядження 6–10" },
        BotTier::High => Text { en: "Equipment 11-15", uk: "Спорядження 11–15" },
    }
}

/// [A266] The expected hit rates a shot in `tuning`, at skill 0, 0.5 and 1 and at each band's floor, start and ceiling,
/// by class (the hits' knobs, in their order), each on its line ([`super::ClassCalibration::line`]).
fn rates(tuning: &Tuning) -> Rates {
    let skill = Text { en: "Skill", uk: "Навичка" };
    let mut points: Vec<RatePoint> =
        [0.0, 0.5, 1.0].into_iter().map(|at| RatePoint { band: skill, what: None, skill: at }).collect();
    for tier in BotTier::ALL {
        let row = tuning.skill.of_tier(tier);
        points.extend(
            [
                (Text { en: "floor", uk: "мінімум" }, row.min),
                (Text { en: "start", uk: "старт" }, row.start),
                (Text { en: "ceiling", uk: "стеля" }, row.max),
            ]
            .map(|(what, at)| RatePoint { band: band(tier), what: Some(what), skill: at }),
        );
    }
    let classes = KNOBS
        .iter()
        .filter_map(|knob| match knob.unit {
            Unit::Hits { class } => Some(ClassRates {
                label: knob.label,
                line: line_of(tuning, class, &points),
                bar: tuning.calibration.of(class).map(|calibration| {
                    let note = bar_note(calibration, tuning.aim.class_aim.of(class).2);
                    note.unwrap_or_else(|| format!("bar at skill 1: {} %", calibration.bar_text()))
                }),
            }),
            _ => None,
        })
        .collect();
    Rates { method: METHOD, source: tuning.calibration.source.clone(), points, classes }
}

/// A class's expected rates at `points` in `tuning`.
fn line_of(tuning: &Tuning, class: WeaponClass, points: &[RatePoint]) -> ClassLine {
    let (calibration, pooled) = match tuning.calibration.type_of(class) {
        TypeCalibration::NotMeasured => {
            return ClassLine::Unknown(Text { en: "not measured (no bench kit)", uk: "не виміряно (немає набору на стенді)" });
        }
        TypeCalibration::Pooled(numbers) => (numbers, true),
        TypeCalibration::Own(numbers) => (numbers, false),
    };
    match calibration.line(tuning.aim.skill_curve, tuning.aim.class_aim.of(class)) {
        Some(line) => {
            let rates = points.iter().map(|point| line.at(point.skill)).collect();
            if pooled { ClassLine::Pooled(rates) } else { ClassLine::Own(rates) }
        }
        None => ClassLine::Unknown(Text {
            en: "not calibrated: the bench measured this class's line with the geometric curve and other skill-0 and 0.5 factors, or the skill-1 factor is outside its points",
            uk: "не калібровано: стенд виміряв лінію цього класу з геометричною кривою та іншими факторами навичок 0 і 0.5, або фактор навички 1 поза його точками",
        }),
    }
}

/// The knobs whose value as the page shows it differs from `old` to `new`.
fn edits(old: &Tuning, new: &Tuning) -> Vec<Edit> {
    KNOBS
        .iter()
        .filter_map(|knob| {
            let (old, new) = (knob.shown(old), knob.shown(new));
            (old != new).then(|| Edit { key: knob.key.to_owned(), old, new })
        })
        .collect()
}

/// The skill's 0 to 1.
const SHARE: Unit = Unit::Number { min: 0.0, max: 1.0 };
/// A K/D.
const KD: Unit = Unit::Number { min: 0.1, max: 10.0 };

/// The knobs of the page. The brackets are the tiers of [`super::SkillTuning`] (players of equipment 0-5 meet bots of
/// tier 4-5, and so on: [`BotTier`]): each one's floor, start and ceiling (A244), which the page sets, under the scale's
/// top, `skill.max`, shown only. The hits are each class's skill-1 factor (`aim.class_aim`, A243) as the rate it gives.
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
        key: "skill.tiers.4-5.max",
        group: Group::Brackets,
        set: Set::Page,
        label: Text { en: "Equipment 0-5: ceiling", uk: "Спорядження 0–5: стеля" },
        unit: SHARE,
        read: |t| f64::from(t.skill.of_tier(BotTier::Low).max),
    },
    Knob {
        key: "skill.tiers.9-10.max",
        group: Group::Brackets,
        set: Set::Page,
        label: Text { en: "Equipment 6-10: ceiling", uk: "Спорядження 6–10: стеля" },
        unit: SHARE,
        read: |t| f64::from(t.skill.of_tier(BotTier::Mid).max),
    },
    Knob {
        key: "skill.tiers.13-15.max",
        group: Group::Brackets,
        set: Set::Page,
        label: Text { en: "Equipment 11-15: ceiling", uk: "Спорядження 11–15: стеля" },
        unit: SHARE,
        read: |t| f64::from(t.skill.of_tier(BotTier::High).max),
    },
    Knob {
        key: "skill.max",
        group: Group::Brackets,
        set: Set::Later,
        label: Text { en: "Top of the scale (every equipment)", uk: "Верх шкали (будь-яке спорядження)" },
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
    Knob {
        key: "aim.class_aim.smg",
        group: Group::Hits,
        set: Set::Page,
        label: Text { en: "SMGs", uk: "Пістолети-кулемети" },
        unit: Unit::Hits { class: WeaponClass::Smg },
        read: |t| hits(t, WeaponClass::Smg),
    },
    Knob {
        key: "aim.class_aim.rifle",
        group: Group::Hits,
        set: Set::Page,
        label: Text { en: "Rifles (automatic)", uk: "Автоматичні гвинтівки" },
        unit: Unit::Hits { class: WeaponClass::Rifle },
        read: |t| hits(t, WeaponClass::Rifle),
    },
    Knob {
        key: "aim.class_aim.mg",
        group: Group::Hits,
        set: Set::Page,
        label: Text { en: "Machine guns", uk: "Кулемети" },
        unit: Unit::Hits { class: WeaponClass::MachineGun },
        read: |t| hits(t, WeaponClass::MachineGun),
    },
    Knob {
        key: "aim.class_aim.shotgun",
        group: Group::Hits,
        set: Set::Page,
        label: Text { en: "Shotguns (a pellet)", uk: "Дробовики (за дробину)" },
        unit: Unit::Hits { class: WeaponClass::Shotgun },
        read: |t| hits(t, WeaponClass::Shotgun),
    },
    Knob {
        key: "aim.class_aim.marksman",
        group: Group::Hits,
        set: Set::Page,
        label: Text { en: "DMRs", uk: "Марксманські гвинтівки" },
        unit: Unit::Hits { class: WeaponClass::Marksman },
        read: |t| hits(t, WeaponClass::Marksman),
    },
    Knob {
        key: "aim.class_aim.bolt",
        group: Group::Hits,
        set: Set::Page,
        label: Text { en: "Bolt actions", uk: "Гвинтівки з ковзним затвором" },
        unit: Unit::Hits { class: WeaponClass::Bolt },
        read: |t| hits(t, WeaponClass::Bolt),
    },
    Knob {
        key: "aim.class_aim.pistol",
        group: Group::Hits,
        set: Set::Later,
        label: Text { en: "Pistols", uk: "Пістолети" },
        unit: Unit::Hits { class: WeaponClass::Pistol },
        read: |t| hits(t, WeaponClass::Pistol),
    },
];

fn knob(key: &str) -> Option<&'static Knob> {
    KNOBS.iter().find(|knob| knob.key == key)
}

/// The page's changes: each knob set, by its key.
type Changes = BTreeMap<&'static str, Value>;

/// The changes as a JSON merge patch over the tuning.
fn patch_of(changes: &Changes, file: &Tuning) -> serde_json::Value {
    let mut patch = serde_json::Value::Object(serde_json::Map::new());
    for (key, value) in changes {
        let json = knob(key).map_or(serde_json::Value::Null, |knob| knob.json(*value, file));
        let leaf = key.rsplit('.').fold(json, |inner, name| {
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
        let value = knob.value_of_json(leaf).map_err(|e| format!("{}: {key}: {e}", path.display()))?;
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
fn write_changes(path: &Path, changes: &Changes, file: &Tuning) -> Result<(), String> {
    write_whole(path, &bytes_of(&patch_of(changes, file))).map_err(|e| format!("{}: {e}", path.display()))
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
    let patch = patch_of(changes, file.tuning());
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

    /// The page's `asked` values over the changes kept, each within its limits; a knob set to its default is no change.
    fn proposed(&self, asked: &[(String, String)]) -> Result<Changes, String> {
        let mut changes = self.changes.clone();
        for (key, text) in asked {
            let knob = knob(key).ok_or_else(|| format!("{key}: no such knob"))?;
            if !knob.settable() {
                return Err(format!("{key}: not settable yet"));
            }
            let value = knob.value_of(text, self.file.tuning()).map_err(|why| format!("{key}: {why}"))?;
            if knob.shown_value(value, self.file.tuning()) == knob.shown(self.file.tuning()) {
                changes.remove(knob.key);
            } else {
                changes.insert(knob.key, value);
            }
        }
        Ok(changes)
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
            log::info!("Bots tuning: the admin page's changes {} over {}", patch_of(&changes, file.tuning()), file.data_line());
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

    /// What the next match would run on with `changes` over the file, if a match can.
    fn checked(&self, state: &State, changes: &Changes) -> Result<LoadedTuning, String> {
        let good = combine(&state.file, changes)?;
        fits(self.start.tuning(), good.tuning())?;
        Ok(good)
    }

    /// `changes` made what the next match runs on, kept and audited; nothing changes if they cannot be used.
    fn commit(&self, state: &mut State, changes: Changes, from: &str) -> Result<Vec<Change>, String> {
        if changes == state.changes {
            return Ok(Vec::new());
        }
        let good = self.checked(state, &changes)?;
        if let Some(path) = &self.changes_file {
            write_changes(path, &changes, state.file.tuning())?;
        }
        let at = unix_now();
        let made: Vec<Change> = edits(state.good.tuning(), good.tuning())
            .into_iter()
            .map(|Edit { key, old, new }| Change { at, from: from.to_owned(), key, old, new })
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
                source: (group == Group::Hits).then(|| state.file.tuning().calibration.source.clone()),
                knobs: KNOBS
                    .iter()
                    .filter(|knob| knob.group == group)
                    .map(|knob| bot_desk::Knob {
                        key: knob.key.to_owned(),
                        label: knob.label,
                        state: if knob.settable() { KnobState::Settable } else { KnobState::Shown },
                        value: knob.shown(state.good.tuning()),
                        default: knob.shown(state.file.tuning()),
                        limits: knob.limits(state.file.tuning()),
                        beside: knob.beside(state.good.tuning()),
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
            rates: rates(state.good.tuning()),
            audit: state.audit.iter().cloned().collect(),
        }
    }

    fn set(&self, asked: &[(String, String)], from: &str) -> Result<Vec<Change>, String> {
        let mut state = self.state.locked();
        let changes = state.proposed(asked)?;
        self.commit(&mut state, changes, from)
    }

    fn preview(&self, asked: &[(String, String)]) -> Result<Preview, String> {
        let state = self.state.locked();
        let changes = state.proposed(asked)?;
        let next = if changes == state.changes { state.good.clone() } else { self.checked(&state, &changes)? };
        Ok(Preview {
            edits: edits(state.good.tuning(), next.tuning()),
            before: rates(state.good.tuning()),
            after: rates(next.tuning()),
        })
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
            &[("skill.tiers.13-15.start", "0.2")],
            &[("skill.tiers.13-15.start", "0.95")],
            &[("skill.tiers.9-10.max", "0.4")],
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
        assert_eq!(patch_of(&live.state.locked().changes, &Tuning::default()), serde_json::json!({"skill": {"every_s": 60}}));
    }

    /// The brackets ship as the owner set them (2026-11-01: equipment 0-5 0.0-0.6 from 0.3, 6-10 0.2-0.8 from 0.5, 11-15
    /// 0.3-0.9 from 0.6; A244); the page moves a band's floor and start inside its ceiling, and a match's team then
    /// starts there.
    #[test]
    fn the_page_sets_the_brackets_floor_and_start_in_their_order() {
        let live = LiveTuning::default();
        let shipped: Vec<_> = BotTier::ALL.iter().map(|&tier| live.for_match().tuning().skill.of_tier(tier)).map(|row| (row.min, row.start, row.max)).collect();
        assert_eq!(shipped, [(0.0, 0.3, 0.6), (0.2, 0.5, 0.8), (0.3, 0.6, 0.9)]);
        // The floors and starts before A244, each inside its band's ceiling.
        let made = set(
            &live,
            &[
                ("skill.tiers.4-5.min", "0.3"),
                ("skill.tiers.4-5.start", "0.6"),
                ("skill.tiers.9-10.min", "0.5"),
                ("skill.tiers.9-10.start", "0.75"),
                ("skill.tiers.13-15.min", "0.65"),
                ("skill.tiers.13-15.start", "0.85"),
            ],
        )
        .unwrap();
        assert_eq!(made.len(), 6);
        let next = live.for_match();
        let rows: Vec<_> = BotTier::ALL.iter().map(|&tier| next.tuning().skill.of_tier(tier)).map(|row| (row.min, row.start)).collect();
        assert_eq!(rows, [(0.3, 0.6), (0.5, 0.75), (0.65, 0.85)]);
        // The start may go under its old floor when the floor goes with it, in one save.
        set(&live, &[("skill.tiers.4-5.min", "0.05"), ("skill.tiers.4-5.start", "0.1")]).unwrap();
        assert_eq!(live.for_match().tuning().skill.of_tier(BotTier::Low).start, 0.1);
        // A band's ceiling moves too, above its start; the next match's dial keeps to it.
        set(&live, &[("skill.tiers.13-15.max", "0.95")]).unwrap();
        assert_eq!(live.for_match().tuning().skill.of_tier(BotTier::High).max, 0.95);
        // The view: the floors, starts and ceilings settable, the scale's top shown only, the group's note says why.
        let view = live.view();
        let brackets = view.groups.iter().find(|group| group.key == "brackets").unwrap();
        assert!(brackets.note.is_some());
        let shown: Vec<_> = brackets.knobs.iter().filter(|knob| knob.state == KnobState::Shown).map(|knob| knob.key.as_str()).collect();
        assert_eq!(shown, ["skill.max"]);
        // The group's reset puts the shipped brackets back.
        assert_eq!(live.reset(Some("brackets"), "192.0.2.3").unwrap().len(), 7);
        assert_eq!(live.for_match(), LoadedTuning::default());
    }

    /// [A246] A class's hit rate a shot at skill 1 is set as its skill-1 factor through the bench's calibration: its skill-0
    /// and 0.5 factors stay, the next match runs on it, and the changes file keeps the class's three factors. Outside the
    /// bench's points, the pistols' (no points) or a rate whose factor would make the line fall is refused whole; over the
    /// owner's bar is not (A267).
    #[test]
    fn the_page_sets_a_class_hit_rate_as_its_skill_1_factor() {
        let dir = dir("hits");
        let changes = dir.join("bot-admin-tuning.json");
        let live = LiveTuning::new(LoadedTuning::default(), Some(changes.clone()));
        let shipped = Tuning::default();
        let view = live.view();
        let group = view.groups.iter().find(|group| group.key == "hits").unwrap();
        assert_eq!(group.source.as_deref(), Some(shipped.calibration.source.as_str()));
        let rifle = group.knobs.iter().find(|knob| knob.key == "aim.class_aim.rifle").unwrap();
        assert_eq!((rifle.value.as_str(), rifle.default.as_str()), ("18.6", "18.6"));
        // Usable: from the rate at the widest factor that keeps the line rising (the automatics' 0.5 factor 0.77 allows up
        // to ~1.49, ~17.2 %) to the sweep's top (A267(a): 30.8 % at 0.40, on the line 30.6), the owner's bar beside.
        assert!(rifle.limits.starts_with("17.") && rifle.limits.ends_with("–30.6 % (default bar 18–20 %)"), "{}", rifle.limits);
        let span_low: f64 = rifle.limits.split('–').next().unwrap().parse().unwrap();
        assert_eq!(
            rifle.beside.as_deref(),
            Some("factor at skill 1: 1.32 (skill 0 / 0.5: 0.4 / 0.77); not measured on its own yet: partly the pool's numbers")
        );
        let shotgun = group.knobs.iter().find(|knob| knob.key == "aim.class_aim.shotgun").unwrap();
        assert!(shotgun.limits.ends_with("(default bar 20 %)") && !shotgun.beside.as_deref().unwrap().contains("pool"), "{shotgun:?}");
        // A default line off its bar is kept and said plainly (A267): the shotgun's 18.8 and the DMR's 28.9 under theirs.
        assert!(shotgun.beside.as_deref().unwrap().ends_with("; 18.8 % at skill 1, under its bar 20 %"), "{shotgun:?}");
        let dmr = group.knobs.iter().find(|knob| knob.key == "aim.class_aim.marksman").unwrap();
        assert!(dmr.beside.as_deref().unwrap().ends_with("; 28.9 % at skill 1, under its bar 30–32 %"), "{dmr:?}");
        assert_eq!(dmr.value, dmr.default);
        let pistol = group.knobs.iter().find(|knob| knob.key == "aim.class_aim.pistol").unwrap();
        assert_eq!((pistol.state, pistol.value.as_str()), (KnobState::Shown, "—"));
        // 19 % for the rifles: their skill-1 factor from the calibration, the lower two kept; the next match runs on it.
        let made = set(&live, &[("aim.class_aim.rifle", "19 %")]).unwrap();
        let said: Vec<_> = made.iter().map(|c| (c.key.as_str(), c.old.as_str(), c.new.as_str())).collect();
        assert_eq!(said, [("aim.class_aim.rifle", "18.6", "19.0")]);
        let factor = shipped.calibration.of(WeaponClass::Rifle).unwrap().factor_for(19.0).unwrap();
        let next = live.for_match();
        let (low, middle, high) = next.tuning().aim.class_aim.rifle;
        assert_eq!((low, middle), (0.40, 0.77));
        assert!((high - factor).abs() < 1e-4, "{high} vs {factor}");
        assert_eq!(next.tuning().aim.class_aim.smg, shipped.aim.class_aim.smg);
        // The changes file keeps the class's three factors, and a server started again runs on them.
        let kept: serde_json::Value = serde_json::from_slice(&std::fs::read(&changes).unwrap()).unwrap();
        assert_eq!(kept["aim"]["class_aim"]["rifle"].as_array().map(Vec::len), Some(3));
        assert_eq!(LiveTuning::new(LoadedTuning::default(), Some(changes.clone())).for_match(), next);
        // Refused whole: outside the points, the pistols, not a number, a line that would fall (under the usable span:
        // the rifles just under it, the bolt at 27 %, a factor near 4.5 over its skill-0.5 one).
        let under = format!("{:.1}", span_low - 0.2);
        for bad in [
            ("aim.class_aim.rifle", under.as_str()),
            ("aim.class_aim.rifle", "31"),
            ("aim.class_aim.rifle", "13"),
            ("aim.class_aim.rifle", "0"),
            ("aim.class_aim.rifle", "NaN"),
            ("aim.class_aim.marksman", "33"),
            ("aim.class_aim.pistol", "20"),
            ("aim.class_aim.rifle", "a lot"),
            ("aim.class_aim.bolt", "27"),
        ] {
            assert!(set(&live, &[bad]).is_err(), "{bad:?}");
            assert_eq!(live.for_match(), next, "{bad:?}");
        }
        let refused = set(&live, &[("aim.class_aim.bolt", "27")]).unwrap_err();
        assert!(refused.contains("the lowest that keeps the class's line rising"), "{refused}");
        // The rifles at the bottom of their span, the bolts at their bar and the DMRs inside their narrow span (on the
        // line, 28.4-29.7 %: the sweep's 29.25 % at the shipped 0.72 is the ladder's 28.9 at skill 1).
        let at_low = format!("{:.1}", span_low + 0.1);
        set(&live, &[("aim.class_aim.rifle", at_low.as_str())]).unwrap();
        set(&live, &[("aim.class_aim.bolt", "40"), ("aim.class_aim.marksman", "28.5")]).unwrap();
        let next = live.for_match();
        let factor_for = |class: WeaponClass, rate: f32| shipped.calibration.of(class).unwrap().factor_for(rate).unwrap();
        assert!((next.tuning().aim.class_aim.bolt.2 - factor_for(WeaponClass::Bolt, 40.0)).abs() < 1e-4);
        assert!((next.tuning().aim.class_aim.marksman.2 - factor_for(WeaponClass::Marksman, 28.5)).abs() < 1e-4);
        assert!(set(&live, &[("aim.class_aim.marksman", "29.8")]).unwrap_err().contains("outside the bench's points (28.4–29.7 %)"));
        // Over the owner's bar, within the points (A267): the SMGs at 21 % and the bolts at 42 %.
        set(&live, &[("aim.class_aim.smg", "21"), ("aim.class_aim.bolt", "42")]).unwrap();
        let over = live.for_match();
        assert!((over.tuning().aim.class_aim.smg.2 - factor_for(WeaponClass::Smg, 21.0)).abs() < 1e-4);
        assert!((over.tuning().aim.class_aim.bolt.2 - factor_for(WeaponClass::Bolt, 42.0)).abs() < 1e-4);
        // The group's reset puts the shipped factors back.
        assert_eq!(live.reset(Some("hits"), "192.0.2.4").unwrap().len(), 4);
        assert_eq!(live.for_match(), LoadedTuning::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A class's expected rates at the page's points (panics on a class with none).
    fn line(rates: &Rates, label: &str) -> Vec<f32> {
        match &rates.classes.iter().find(|class| class.label.en == label).unwrap().line {
            ClassLine::Own(at) | ClassLine::Pooled(at) => at.clone(),
            unknown => panic!("{label}: {unknown:?}"),
        }
    }

    /// [A266] The page's expected rates for the shipped tuning are A243's measured line (PR #1089's "after" rows,
    /// weighed; the two tiers' mean where both ran) within its rounding at each skill step it measured, straight between
    /// them; the class rows' skill-1 values are their lines' ends; the pistols are not measured.
    #[test]
    fn the_rates_shown_for_the_defaults_are_a243s_line() {
        let measured: [(&str, &[[f32; 6]]); 6] = [
            ("SMGs", &[[10.0, 11.9, 14.1, 15.7, 17.7, 18.4], [10.1, 12.0, 13.9, 15.8, 18.4, 18.8]]),
            ("Rifles (automatic)", &[[10.0, 11.9, 14.1, 15.7, 17.7, 18.4], [10.1, 12.0, 13.9, 15.8, 18.4, 18.8]]),
            ("Machine guns", &[[10.0, 11.9, 14.1, 15.7, 17.7, 18.4], [10.1, 12.0, 13.9, 15.8, 18.4, 18.8]]),
            ("Shotguns (a pellet)", &[[11.1, 12.9, 14.1, 16.6, 18.3, 19.2], [11.1, 13.0, 15.1, 17.1, 18.4, 18.4]]),
            ("DMRs", &[[18.7, 19.8, 22.3, 25.2, 28.4, 28.9]]),
            ("Bolt actions", &[[32.4, 29.2, 33.4, 35.2, 35.2, 39.1]]),
        ];
        let view = LiveTuning::default().view();
        let rates = &view.rates;
        assert_eq!(rates.source, Tuning::default().calibration.source);
        let skills: Vec<f32> = rates.points.iter().map(|point| point.skill).collect();
        assert_eq!(skills, [0.0, 0.5, 1.0, 0.0, 0.3, 0.6, 0.2, 0.5, 0.8, 0.3, 0.6, 0.9]);
        let hits = view.groups.iter().find(|group| group.key == "hits").unwrap();
        let mut steps = 0;
        for (label, rows) in measured {
            let shown = line(rates, label);
            let mean = |step: usize| rows.iter().map(|row| row[step]).sum::<f32>() / rows.len() as f32;
            for (point, rate) in rates.points.iter().zip(&shown) {
                let step = point.skill * 5.0;
                let expected = if (step - step.round()).abs() < 1e-4 {
                    steps += 1;
                    mean(step.round() as usize)
                } else {
                    let below = step.floor() as usize;
                    mean(below) + (mean(below + 1) - mean(below)) * (step - step.floor())
                };
                assert!((rate - expected).abs() <= 0.05 + 1e-4, "{label} at {}: {rate} vs {expected}", point.skill);
            }
            let knob = hits.knobs.iter().find(|knob| knob.label.en == label).unwrap();
            assert_eq!(knob.value, format!("{:.1}", shown[2]), "{label}");
        }
        assert_eq!(steps, 6 * 7, "0, 1 and the bands' 0.0, 0.6, 0.2, 0.8, 0.6: a class's points on the ladder's steps");
        let pistols = rates.classes.iter().find(|class| class.label.en == "Pistols").unwrap();
        assert!(matches!(pistols.line, ClassLine::Unknown(_)));
        // The automatics on the pool's numbers, said so; the others their own.
        let pooled: Vec<_> = rates.classes.iter().filter(|class| matches!(class.line, ClassLine::Pooled(_))).map(|class| class.label.en).collect();
        assert_eq!(pooled, ["SMGs", "Rifles (automatic)", "Machine guns"]);
        // On `all_off.json`'s factors (every class 1, the straight curve) the bench's line is not the class's: none shown.
        let mut off = Tuning::default();
        off.aim.class_aim = super::super::ClassAim::same();
        off.aim.skill_curve = super::super::SkillCurve::Linear;
        assert!(super::rates(&off).classes.iter().all(|class| matches!(class.line, ClassLine::Unknown(_))));
    }

    /// [A266] A preview says what a set would change and the expected rates before and after, and changes nothing: a
    /// class's skill-1 target moves its line's upper half by the dial's weight; a band's bracket moves its points, each
    /// class's rate read there. It refuses what a set refuses, with the same reason, and the set then makes what it said.
    #[test]
    fn a_preview_shows_the_change_and_its_rates_and_changes_nothing() {
        let live = LiveTuning::default();
        let asked = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs.iter().map(|(key, text)| ((*key).to_owned(), (*text).to_owned())).collect()
        };
        let edit = |key: &str, old: &str, new: &str| Edit { key: key.to_owned(), old: old.to_owned(), new: new.to_owned() };
        // A target: 19 % for the rifles at skill 1, 0.4 over their line's end; at each point its weight of it.
        let rifles = asked(&[("aim.class_aim.rifle", "19")]);
        let preview = live.preview(&rifles).unwrap();
        assert_eq!(preview.edits, [edit("aim.class_aim.rifle", "18.6", "19.0")]);
        assert_eq!(preview.before, live.view().rates);
        assert_eq!(preview.after.points, preview.before.points);
        let (before, after) = (line(&preview.before, "Rifles (automatic)"), line(&preview.after, "Rifles (automatic)"));
        let weights = [0.0, 0.0, 1.0, 0.0, 0.0, 0.2, 0.0, 0.0, 0.6, 0.0, 0.2, 0.8];
        for ((old, new), weight) in before.iter().zip(&after).zip(weights) {
            assert!((new - old - 0.4 * weight).abs() < 1e-3, "{before:?} -> {after:?}");
        }
        for other in ["SMGs", "Machine guns", "Shotguns (a pellet)", "DMRs", "Bolt actions"] {
            assert_eq!(line(&preview.after, other), line(&preview.before, other), "{other}");
        }
        // Nothing changed: the next match, the view, the audit.
        assert_eq!(live.for_match(), LoadedTuning::default());
        assert_eq!(live.view().rates, preview.before);
        assert!(live.view().audit.is_empty());
        // A bracket: the 6-10 band's ceiling 0.8 -> 0.9, each class read there (as at the 11-15 ceiling, 0.9).
        let preview = live.preview(&asked(&[("skill.tiers.9-10.max", "0.9")])).unwrap();
        assert_eq!(preview.edits, [edit("skill.tiers.9-10.max", "0.8", "0.9")]);
        let ceiling = 8;
        assert_eq!((preview.before.points[ceiling].skill, preview.after.points[ceiling].skill), (0.8, 0.9));
        for class in ["SMGs", "Shotguns (a pellet)", "DMRs", "Bolt actions"] {
            let (before, after) = (line(&preview.before, class), line(&preview.after, class));
            assert_eq!(after[ceiling], after[11], "{class}");
            assert_ne!(after[ceiling], before[ceiling], "{class}");
            let others = |at: &[f32]| -> Vec<f32> { at.iter().enumerate().filter(|(i, _)| *i != ceiling).map(|(_, x)| *x).collect() };
            assert_eq!(others(&after), others(&before), "{class}");
        }
        // Refused as a set refuses them, with the same reason; nothing changes.
        for bad in [
            &[("aim.class_aim.rifle", "31")][..],
            &[("aim.class_aim.rifle", "13")],
            &[("skill.tiers.9-10.max", "0.4")],
            &[("aim.class_aim.pistol", "20")],
            &[("skill.step", "a lot")],
            &[("skill.kd_low", "2"), ("skill.kd_high", "1.9")],
        ] {
            let bad = asked(bad);
            assert_eq!(live.preview(&bad).unwrap_err(), live.set(&bad, "192.0.2.5").unwrap_err(), "{bad:?}");
        }
        assert_eq!(live.for_match(), LoadedTuning::default());
        // The set makes what the preview said; the same values again preview no edit.
        let preview = live.preview(&rifles).unwrap();
        let made = live.set(&rifles, "192.0.2.5").unwrap();
        let said: Vec<Edit> = made.into_iter().map(|c| Edit { key: c.key, old: c.old, new: c.new }).collect();
        assert_eq!(said, preview.edits);
        assert_eq!(live.view().rates, preview.after);
        let again = live.preview(&rifles).unwrap();
        assert!(again.edits.is_empty() && again.before == again.after);
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
            (r#"{"aim":{"class_aim":{"pistol":[0.4,0.77,1.32]}}}"#, "`aim.class_aim.pistol` is not a knob the page sets"),
            (r#"{"aim":{"class_aim":{"rifle":[0.4,0.77]}}}"#, "aim.class_aim.rifle: not three factors"),
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

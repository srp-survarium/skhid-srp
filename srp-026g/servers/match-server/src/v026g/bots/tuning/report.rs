//! What a tuning has switched on and off, for the logs of the server and of the bench.
//!
//! The report is against [`Tuning::all_off`], the tuning before the owner's set of options went into the defaults,
//! so the lines read as they did when that set was a file: a server on the defaults says the whole set is ON and
//! lists the numbers it changes, a bench arm on the all-off patch says `none`.
//!
//! The report is built from the tuning serialised to JSON and the reference serialised the same way, so a field added to
//! any section shows in the logs with no list to keep: every boolean is ON or OFF; every `Option` is off (none) or set
//! (its value is shown); every enum written as text (or as an object where the default is text) shows its value (the
//! plays' ranks and the modifiers' inputs, which are not switches, only when changed); every
//! number that is not the default's shows as `name=value (default d)`; a list that is not the default's shows as
//! changed. A section is the first part of the name (`feel`, `sense`, `grid`...).
//!
//! The text has one line per kind and section, sections in alphabetical order, with fixed prefixes for the scripts that read
//! the logs (`switches ON <section>:`, `switches OFF <section>:`, `switch values <section>:`, `changed numbers <section>:`);
//! each line is `name name=value ...`. A kind with no entries is one line `switches ON: none` (and so on). A change to one
//! section's switches touches only that section's lines, so two changes to different sections never touch the same line of
//! the golden text.

use std::fmt;

use serde_json::Value;

use super::Tuning;

/// One kind of entry: `(section, entry text)` in the order of the walk.
type Entries = Vec<(String, String)>;

/// The switches of a tuning, sorted by what they are.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SwitchReport {
    /// Booleans that are true, and options that are set (`name=value`).
    pub on: Entries,
    /// Booleans that are false, and options that are none.
    pub off: Entries,
    /// Enums, written as text (`name=value`).
    pub values: Entries,
    /// Numbers and lists that differ from the defaults.
    pub changed: Entries,
}

impl Tuning {
    /// The state of every switch of this tuning, against the tuning with the owner's options off ([`Tuning::all_off`]).
    pub fn switch_report(&self) -> SwitchReport {
        let value = serde_json::to_value(self).expect("a tuning is JSON");
        let default = serde_json::to_value(Tuning::all_off()).expect("a tuning is JSON");
        let mut report = SwitchReport::default();
        walk(&mut Vec::new(), Some(&value), Some(&default), &mut report);
        report
    }
}

impl SwitchReport {
    /// The lines of the report: the ON lines, then OFF, values and changed numbers, one per section in each, each with its
    /// fixed prefix.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (prefix, entries) in
            [("switches ON", &self.on), ("switches OFF", &self.off), ("switch values", &self.values), ("changed numbers", &self.changed)]
        {
            lines.extend(section_lines(prefix, entries));
        }
        lines
    }
}

impl fmt::Display for SwitchReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.lines().join("\n"))
    }
}

/// `prefix section: a b=1` for each section of the entries, in the order of the sections; `prefix: none` when there are none.
fn section_lines(prefix: &str, entries: &Entries) -> Vec<String> {
    let mut sections: Vec<(&str, Vec<&str>)> = Vec::new();
    for (section, text) in entries {
        match sections.last_mut() {
            Some((open, texts)) if *open == section => texts.push(text),
            _ => sections.push((section, vec![text])),
        }
    }
    if sections.is_empty() {
        return vec![format!("{prefix}: none")];
    }
    sections.into_iter().map(|(section, texts)| format!("{prefix} {section}: {}", texts.join(" "))).collect()
}

/// A number as it reads: whole numbers as they are, others as the `f32` they were (0.8, not 0.800000011920929).
fn number(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        i.to_string()
    } else if let Some(u) = n.as_u64() {
        u.to_string()
    } else {
        let x = n.as_f64().unwrap_or(f64::NAN);
        let narrow = x as f32;
        if (f64::from(narrow) - x).abs() <= x.abs() * 1e-6 { narrow.to_string() } else { x.to_string() }
    }
}

/// A value as short text: numbers as [`number`], text as it is, anything else as compact JSON cut to 80 characters.
fn show(v: &Value) -> String {
    match v {
        Value::Number(n) => number(n),
        Value::String(s) => s.clone(),
        other => {
            let text = other.to_string();
            if text.chars().count() > 80 {
                let cut: String = text.chars().take(77).collect();
                format!("{cut}...")
            } else {
                text
            }
        }
    }
}

fn entry(path: &[String]) -> (String, String) {
    (path[0].clone(), path[1..].join("."))
}

fn walk(path: &mut Vec<String>, value: Option<&Value>, default: Option<&Value>, out: &mut SwitchReport) {
    let Some(value) = value else { return };
    // Both objects: their members (a variant with data is an object where the default may be text: that is a value).
    if let Value::Object(members) = value {
        let both = matches!(default, Some(Value::Object(_)) | None);
        if both {
            let defaults = default.and_then(Value::as_object);
            for (key, member) in members {
                path.push(key.clone());
                walk(path, Some(member), defaults.and_then(|d| d.get(key)), out);
                path.pop();
            }
            return;
        }
    }
    if path.len() < 2 {
        return; // a bare section is never a leaf
    }
    let (section, name) = entry(path);
    match value {
        Value::Bool(true) => out.on.push((section, name)),
        Value::Bool(false) => out.off.push((section, name)),
        Value::Null => out.off.push((section, name)),
        Value::String(s) => {
            // The plays' ranks and the modifiers' inputs and curves are text, not switches: shown only when changed.
            if !(matches!(section.as_str(), "plays" | "mods") && default == Some(value)) {
                out.values.push((section, format!("{name}={s}")));
            }
        }
        Value::Number(n) => match default {
            Some(Value::Null) => out.on.push((section, format!("{name}={}", number(n)))),
            Some(d) if d == value => {}
            Some(d) => out.changed.push((section, format!("{name}={} (default {})", number(n), show(d)))),
            None => out.changed.push((section, format!("{name}={} (new)", number(n)))),
        },
        Value::Object(_) => out.values.push((section, format!("{name}={}", show(value)))),
        Value::Array(_) => {
            if default != Some(value) {
                out.changed.push((section, format!("{name}={} (default {})", show(value), default.map_or_else(|| "none".to_owned(), show))));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every boolean of the reference tuning (found in its JSON, so a field added later is covered) is in the report,
    /// ON or OFF; an `Option` is OFF while none; nothing is ON that the tuning does not have on.
    #[test]
    fn the_report_lists_every_boolean_of_the_default_tuning() {
        fn bools(path: &mut Vec<String>, v: &Value, out: &mut Vec<(String, bool)>) {
            match v {
                Value::Object(members) => {
                    for (key, member) in members {
                        path.push(key.clone());
                        bools(path, member, out);
                        path.pop();
                    }
                }
                Value::Bool(b) if path.len() >= 2 => out.push((path.join("."), *b)),
                _ => {}
            }
        }
        let default = serde_json::to_value(Tuning::all_off()).unwrap();
        let mut all = Vec::new();
        bools(&mut Vec::new(), &default, &mut all);
        assert!(all.len() > 40, "the tuning has dozens of switches: {}", all.len());
        let report = Tuning::all_off().switch_report();
        let name = |(section, text): &(String, String)| format!("{section}.{text}");
        let on: Vec<String> = report.on.iter().map(name).collect();
        let off: Vec<String> = report.off.iter().map(name).collect();
        for (path, value) in all {
            let (listed, other) = if value { (&on, &off) } else { (&off, &on) };
            assert!(listed.contains(&path), "{path} ({value}) is not in the report's {}", if value { "ON" } else { "OFF" });
            assert!(!other.contains(&path), "{path} is on both sides");
        }
        assert!(off.contains(&"unstick.suicide_after_ms".to_owned()), "an option that is none is OFF");
        assert!(report.changed.is_empty(), "the reference changes nothing: {:?}", report.changed);
        // The defaults have the owner's set on: it is in the report as ON, with the options that are set shown.
        let report = Tuning::default().switch_report();
        let on: Vec<String> = report.on.iter().map(name).collect();
        assert!(on.contains(&"unstick.suicide_after_ms=60000".to_owned()), "{on:?}");
        assert!(on.contains(&"heal.on".to_owned()) && on.contains(&"grid.gap_links".to_owned()), "{on:?}");
        assert!(!report.changed.is_empty());
    }

    /// A switch turned on moves from OFF to ON, an option that is set shows its value, an enum shows its value and a
    /// number that is not the default's shows with the default.
    #[test]
    fn a_changed_tuning_shows_what_changed() {
        let mut value = serde_json::to_value(Tuning::all_off()).unwrap();
        Tuning::apply_patch(
            &mut value,
            serde_json::json!({
                "heal": {"on": true},
                "unstick": {"suicide_after_ms": 60000},
                "feel": {"lip_guard_ticks": 2},
                "hazard": {"route_crossing": "total"},
                "traps": {"on_the_pass": {"gain": 1.0, "route_ahead_m": 12.0}},
            }),
        )
        .unwrap();
        let tuning = Tuning::from_value(value).unwrap();
        let lines = tuning.switch_report().lines();
        let line = |prefix: &str| lines.iter().find(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("no `{prefix}` in {lines:?}"));
        assert!(line("switches ON heal:").contains("interrupt on"), "{lines:?}");
        assert!(line("switches ON unstick:").contains("suicide_after_ms=60000"), "{lines:?}");
        assert!(line("switches OFF heal:").contains("bandages"), "{lines:?}");
        assert!(line("switch values hazard:").contains("route_crossing=total"), "{lines:?}");
        assert!(line("switch values traps:").contains("on_the_pass={\"gain\""), "{lines:?}");
        assert!(line("changed numbers feel:").contains("lip_guard_ticks=2 (default 0)"), "{lines:?}");
    }

    /// The report is one line per kind and section, sections in alphabetical order, and a switch of one section moves
    /// only that section's lines: the text two changes to different sections edit never shares a line.
    #[test]
    fn one_line_per_section_and_a_change_moves_only_its_section() {
        let lines = Tuning::default().switch_report().lines();
        for prefix in ["switches ON ", "switches OFF ", "switch values ", "changed numbers "] {
            let sections: Vec<&str> =
                lines.iter().filter_map(|l| l.strip_prefix(prefix)).map(|rest| rest.split(':').next().unwrap()).collect();
            let mut sorted = sections.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sections, sorted, "{prefix}: one line per section, in order");
        }
        assert!(lines.iter().all(|l| l.contains(": ")), "{lines:?}");
        // A kind with no entries is one `none` line.
        assert_eq!(section_lines("switches ON", &Entries::new()), ["switches ON: none"]);
        // One switch flipped: the lines that differ are those of its own section.
        let mut value = serde_json::to_value(Tuning::all_off()).unwrap();
        Tuning::apply_patch(&mut value, serde_json::json!({"heal": {"on": true}})).unwrap();
        let flipped = Tuning::from_value(value).unwrap().switch_report().lines();
        let base = Tuning::all_off().switch_report().lines();
        let moved: Vec<&String> = flipped.iter().filter(|l| !base.contains(l)).collect();
        assert!(moved.iter().all(|l| l.contains(" heal:") || l.ends_with(": none")), "{moved:?}");
    }

    /// The default tuning's report lines, as the logs carry them (against the tuning with the owner's options off): a
    /// change of a default, or a new switch, changes this text on purpose. `SRP_WRITE_GOLDEN=1` prints the new text instead of comparing.
    #[test]
    fn the_default_tuning_report_is_what_it_was() {
        let text = format!("{}\n", Tuning::default().switch_report());
        if std::env::var_os("SRP_WRITE_GOLDEN").is_some() {
            println!("{text}");
            return;
        }
        let golden = include_str!("report_golden.txt").replace("\r\n", "\n");
        assert_eq!(text, golden, "the switch report of the default tuning changed (SRP_WRITE_GOLDEN=1 prints it)");
    }
}

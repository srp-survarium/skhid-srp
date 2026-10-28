//! What the runner leaves behind: the results file (one row per try), the printed test card, and the log lines.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::run::{Action, Row};
use super::step::{Cond, End, Group, Step};

/// The results file's columns.
pub const HEADER: &str = "match\tstep\tpackage\ttry\ttries\tresult\tended_by\tstart_ms\tend_ms\tvalues";

/// Where the plan's files are written, and the progress of its steps across matches.
#[derive(Debug)]
pub struct Output {
    results: PathBuf,
    card: PathBuf,
    /// Tries used up per step id.
    used: Mutex<Vec<(&'static str, u8)>>,
}

impl Output {
    /// Files named after the plan, in `dir` (made if it is not there).
    pub fn new(dir: &Path, plan: &str) -> std::io::Result<Self> {
        fs::create_dir_all(dir)?;
        let name = plan.replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "_");
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        Ok(Self {
            results: dir.join(format!("client-test-results-{name}-{stamp}.tsv")),
            card: dir.join(format!("client-test-card-{name}-{stamp}.md")),
            used: Mutex::new(Vec::new()),
        })
    }

    pub fn results_path(&self) -> &Path {
        &self.results
    }

    pub fn card_path(&self) -> &Path {
        &self.card
    }

    /// The steps' tries used up so far.
    pub fn used(&self, id: &str) -> u8 {
        self.used
            .lock()
            .map_or(0, |used| used.iter().find(|(step, _)| *step == id).map_or(0, |(_, n)| *n))
    }

    /// Writes the card of the whole plan.
    pub fn write_card(&self, text: &str) -> std::io::Result<()> {
        fs::write(&self.card, text)
    }

    /// Carries out what the runner asked for; the owner's words go back to the caller.
    pub fn apply(&self, actions: Vec<Action>) -> Vec<String> {
        let mut said = Vec::new();
        for action in actions {
            match action {
                Action::Log(line) => log::info!("{line}"),
                Action::Tell(text) => {
                    log::info!("CLIENT_TEST tell: {text}");
                    said.push(text);
                }
                Action::Row(row) => {
                    if let Err(error) = self.append(&row) {
                        log::warn!("CLIENT_TEST: the results file {} cannot be written: {error}", self.results.display());
                    }
                    if row.outcome.counts()
                        && let Ok(mut used) = self.used.lock()
                    {
                        match used.iter_mut().find(|(id, _)| *id == row.step) {
                            Some((_, n)) => *n += 1,
                            None => used.push((row.step, 1)),
                        }
                    }
                }
            }
        }
        said
    }

    fn append(&self, row: &Row) -> std::io::Result<()> {
        let fresh = !self.results.exists();
        let mut file = OpenOptions::new().create(true).append(true).open(&self.results)?;
        if fresh {
            writeln!(file, "{HEADER}")?;
        }
        writeln!(file, "{}", line_of(row))
    }
}

/// One results line.
pub fn line_of(row: &Row) -> String {
    let values: Vec<String> = row
        .values
        .iter()
        .map(|(name, value)| match value {
            Some(v) => format!("{name}={}", trim(*v)),
            None => format!("{name}=-"),
        })
        .collect();
    format!(
        "{:#x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        row.match_id,
        row.step,
        row.package,
        row.try_no,
        row.tries,
        row.outcome.text(),
        row.ended_by,
        row.start_ms,
        row.end_ms,
        values.join(";")
    )
}

fn trim(v: f64) -> String {
    let text = format!("{v:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The printed card: the steps in the order they run, by match, with what the owner does.
pub fn card(plan: &str, steps: &[Step]) -> String {
    let mut text = format!("# Client test card: {plan}\n\nThe steps run by themselves, one after another, in the match for their group.\n");
    let mut group: Option<Group> = None;
    let mut matches = 0;
    for step in steps {
        if group != Some(step.group) {
            group = Some(step.group);
            matches += 1;
            text.push_str(&format!("\n## Match {matches}: {}\n\nStart Player vs Bots on this map and mode.\n\n", step.group.describe()));
        }
        text.push_str(&format!("- **{}** ({}, {} s, {} {}): {}\n", step.id, step.package, step.budget_ms / 1_000, step.tries, if step.tries == 1 { "try" } else { "tries" }, step.instruction));
        text.push_str(&format!("  - starts when: {}\n", describe_start(&step.start)));
        if !step.end.is_empty() {
            text.push_str(&format!("  - ends at: {} or after {} s\n", describe_end(&step.end), step.budget_ms / 1_000));
        }
    }
    text
}

fn describe_start(conds: &[Cond]) -> String {
    let parts: Vec<String> = conds
        .iter()
        .map(|cond| match cond {
            Cond::OwnerNear { at, r_m } => format!("you are within {r_m} m of ({:.0}, {:.0}, {:.0})", at.0[0], at.0[1], at.0[2]),
            Cond::BotsInPlace => "the bots are in place".to_string(),
            Cond::After { ms } if *ms == 0 => "at once".to_string(),
            Cond::After { ms } => format!("{} s after the instruction", ms / 1_000),
        })
        .collect();
    parts.join(" and ")
}

fn describe_end(ends: &[End]) -> String {
    let parts: Vec<String> = ends
        .iter()
        .map(|end| match end {
            End::OwnerDied => "you die".to_string(),
            End::OwnerKills(n) => format!("you have {n} kills"),
            End::OwnerLeft { r_m, .. } => format!("you leave the spot by {r_m} m"),
            End::ScriptedDied(_) => "the scripted bot dies".to_string(),
        })
        .collect();
    parts.join(", ")
}

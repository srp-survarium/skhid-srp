//! The plan: which registered steps run (`SRP_CLIENT_TEST`), in what order, and what a match of a given group still has to do.

use super::check::message_check;
use super::report::Output;
use super::run::Todo;
use super::step::{Group, Seat, Size, Step};

/// Packages that only show the API: `all` leaves them out (2 x 90 s of the owner's time), naming one runs it.
const EXAMPLES: &[&str] = &["sample"];

/// The steps of a plan, grouped.
#[derive(Clone, Debug)]
pub struct Plan {
    pub name: String,
    /// Sorted by group (map, then mode, then size), the registration order kept within a group.
    steps: Vec<Step>,
}

impl Plan {
    /// The steps `spec` names out of `all` (in registration order):
    /// - `all`: every registered step;
    /// - `a,b,c`: step ids and package names;
    /// - `@file`: the same, one per line (`#` starts a comment).
    pub fn parse(spec: &str, all: Vec<Step>) -> Result<Self, String> {
        let spec = spec.trim();
        let tokens: Vec<String> = if let Some(path) = spec.strip_prefix('@') {
            let text = std::fs::read_to_string(path).map_err(|e| format!("SRP_CLIENT_TEST: cannot read {path}: {e}"))?;
            text.lines()
                .map(|l| l.split('#').next().unwrap_or("").trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        } else {
            spec.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
        };
        let everything = tokens.iter().any(|t| t == "all");
        // `all` is every step of every package but the ones that are only examples: a package named is run.
        for token in tokens.iter().filter(|t| *t != "all") {
            if !all.iter().any(|s| s.id == token || s.package == token) {
                let known: Vec<&str> = all.iter().map(|s| s.id).collect();
                return Err(format!("SRP_CLIENT_TEST: no step or package {token:?}; the steps are {}", known.join(", ")));
            }
        }
        let mut steps: Vec<Step> = all
            .into_iter()
            .filter(|s| (everything && !EXAMPLES.contains(&s.package)) || tokens.iter().any(|t| s.id == t || s.package == t))
            .collect();
        if steps.is_empty() {
            return Err("SRP_CLIENT_TEST: the plan has no step".to_string());
        }
        if let Some(bad) = steps.iter().find_map(|s| s.check().err()) {
            return Err(format!("SRP_CLIENT_TEST: {bad}"));
        }
        // One role a seat: two steps of one match that want two cannot both have their match.
        for (i, a) in steps.iter().enumerate() {
            for b in &steps[i + 1..] {
                if a.group.level != b.group.level || a.group.mode != b.group.mode {
                    continue;
                }
                if matches!((a.group.size, b.group.size), (Size::PerTeam(x), Size::PerTeam(y)) if x != y) {
                    continue;
                }
                for x in &a.roster {
                    if let Some(y) = b.roster.iter().find(|y| y.side == x.side && y.nth == x.nth && y.role != x.role) {
                        return Err(format!(
                            "SRP_CLIENT_TEST: steps {} and {} want different roles ({:?}, {:?}) for the {} bot {} of one match",
                            a.id,
                            b.id,
                            x.role,
                            y.role,
                            x.side.name(),
                            x.nth
                        ));
                    }
                }
            }
        }
        // A stable sort: the registration order stays within a group.
        steps.sort_by_key(|s| s.group);
        // The first thing the owner is asked: can he read what the server says (`check.rs`).
        let first = steps[0].group;
        steps.insert(0, message_check(first));
        Ok(Self {
            name: if everything { "all".to_string() } else { spec.trim_start_matches('@').rsplit(['/', '\\']).next().unwrap_or(spec).to_string() },
            steps,
        })
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    fn left(&self, step: &Step, output: &Output) -> bool {
        output.used(step.id) < step.tries
    }

    /// What a match of group `played` has to run: the steps that fit it and have tries left.
    pub fn todo_for(&self, played: Group, output: &Output) -> Vec<Todo> {
        self.steps
            .iter()
            .filter(|s| s.group.fits(played) && self.left(s, output))
            .map(|s| Todo {
                next_try: output.used(s.id) + 1,
                step: s.clone(),
            })
            .collect()
    }

    /// The first group, other than `played`'s steps, that still has steps to run.
    pub fn next_group(&self, played: Group, output: &Output) -> Option<Group> {
        self.steps
            .iter()
            .find(|s| !s.group.fits(played) && self.left(s, output))
            .map(|s| s.group)
    }

    /// The roles the steps of a match of group `played` ask for.
    pub fn roster_for(&self, played: Group) -> Vec<Seat> {
        let mut roster: Vec<Seat> = Vec::new();
        for seat in self.steps.iter().filter(|s| s.group.fits(played)).flat_map(|s| s.roster.iter()) {
            if !roster.iter().any(|r| r.side == seat.side && r.nth == seat.nth) {
                roster.push(*seat);
            }
        }
        roster
    }

    /// The matches the plan needs, in the order they run, each with the steps it runs. A step of any size runs in the first match of
    /// its map and mode, so it joins that one.
    pub fn matches(&self) -> Vec<(Group, Vec<&Step>)> {
        let mut out: Vec<(Group, Vec<&Step>)> = Vec::new();
        for step in &self.steps {
            let same_map = |g: &Group| g.level == step.group.level && g.mode == step.group.mode;
            match out.last_mut() {
                Some((g, list)) if *g == step.group => list.push(step),
                Some((g, list)) if g.size == Size::Any && same_map(g) => {
                    *g = step.group;
                    list.push(step);
                }
                _ => out.push((step.group, vec![step])),
            }
        }
        out
    }

    /// The groups of [`Self::matches`].
    pub fn groups(&self) -> Vec<Group> {
        self.matches().into_iter().map(|(group, _)| group).collect()
    }
}

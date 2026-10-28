//! The plan: which registered steps run (`SRP_CLIENT_TEST`), in what order, and what a match of a given group still has to do.

use super::report::Output;
use super::run::Todo;
use super::step::{Group, Step};

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
        for token in tokens.iter().filter(|t| *t != "all") {
            if !all.iter().any(|s| s.id == token || s.package == token) {
                let known: Vec<&str> = all.iter().map(|s| s.id).collect();
                return Err(format!("SRP_CLIENT_TEST: no step or package {token:?}; the steps are {}", known.join(", ")));
            }
        }
        let mut steps: Vec<Step> = all
            .into_iter()
            .filter(|s| everything || tokens.iter().any(|t| s.id == t || s.package == t))
            .collect();
        if steps.is_empty() {
            return Err("SRP_CLIENT_TEST: the plan has no step".to_string());
        }
        // A stable sort: the registration order stays within a group.
        steps.sort_by_key(|s| s.group);
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

    /// Distinct groups in the order they run, for the log and the card.
    pub fn groups(&self) -> Vec<Group> {
        let mut groups: Vec<Group> = Vec::new();
        for step in &self.steps {
            if groups.last() != Some(&step.group) {
                groups.push(step.group);
            }
        }
        groups
    }
}

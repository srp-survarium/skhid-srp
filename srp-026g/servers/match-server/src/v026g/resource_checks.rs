//! Resource checks (`srp_validate::checks`): a game that offers one tells
//! the `resources.db.patch_<n>` it mounted; the server asks it to hash the
//! files its match's project checks (the game data's `srp/checked`,
//! `resources_db::checked`: files of its map's project, and `startup.cfg`), and
//! logs what differs from the release's files. Only logged, in its own file
//! (target [`RESOURCES`], `logs/resources.log`, read by
//! `release/moderate resources`): nothing a player sees, nothing in the db.
//!
//! The lines:
//!
//! ```text
//! <name> (account <id>), match <id>, mounts 12 14 ... 73: as the release
//! <name> (account <id>), match <id>, mounts 12 14 ... 200: not as the release: also 200; not 73
//! <name> (account <id>), match <id>, level_01/day_2: 162 files as the release
//! <name> (account <id>), match <id>, level_01/day_2: 2 of 162 files not as the release: <path> size <n> sha256 <16 hex>, the release's size <n> sha256 <16 hex> | <path> missing | <path> unreadable
//! <name> (account <id>), match <id>, level_01/day_2: answered 3 files, 162 asked
//! ```

use std::collections::BTreeSet;
use std::fmt::Write as _;

use resources_db::checked::{Checked, Sum, hex};
use srp_validate::checks::{Answer, MAX_REQUEST, Mounts, PREFIX, Part, Request};

/// The log target of the resource checks (`logs/resources.log`).
pub const RESOURCES: &str = "resources";
/// The files of one line at most; more are counted.
const LISTED: usize = 16;

/// Whose check, for the log.
pub struct Whom<'a> {
    pub name: &'a str,
    pub account: u32,
    /// The match, as the log names it.
    pub match_id: u32,
    /// The match as its games know it (its seed, `registry::static_id`):
    /// the request's and the reports'.
    pub seed: u32,
}

impl Whom<'_> {
    fn line(&self, what: impl std::fmt::Display) -> String {
        format!(
            "{} (account {}), match {:#x}, {what}",
            self.name, self.account, self.match_id
        )
    }
}

/// What the release has, which a game's answers are compared with.
pub struct Release<'a> {
    pub checked: &'a Checked,
    /// The patches the release has players mount: their n, ascending.
    pub patches: &'a [u8],
    /// The match's project (`level_01/day_2`).
    pub project: &'a str,
}

/// What a step does besides its new state.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// The request, an 0xE4's payload.
    Send(Vec<u8>),
    Log(String),
}

/// One connection's check.
#[derive(Debug, Default)]
pub enum Check {
    /// Its game offered none (yet): an older game, or one still loading.
    #[default]
    Unoffered,
    /// Asked; what it answered so far.
    Asked(Asked),
    /// Logged: nothing more is taken.
    Logged,
}

#[derive(Debug)]
pub struct Asked {
    /// The request's paths, in its order.
    paths: Vec<String>,
    /// Its answer's part 0 came, saying it answers as many.
    counted: bool,
    answers: Vec<Option<Answer>>,
}

impl Check {
    /// `self` after the part `part` of one of the game's reports (each
    /// part once: the caller drops copies, `Carrier::key`).
    pub fn step(self, part: &Part, release: &Release, whom: &Whom) -> (Check, Vec<Action>) {
        match (self, part) {
            (Check::Unoffered, Part::Mounts { files: 0, mounts }) => offered(mounts, release, whom),
            // An answer to nothing asked (an older server's request, or
            // none): told once, then nothing more.
            (Check::Unoffered, _) => (
                Check::Logged,
                vec![Action::Log(whom.line(format_args!(
                    "{}: answered a request it was not sent",
                    release.project
                )))],
            ),
            (Check::Asked(asked), part) => asked.take(part, release, whom),
            (Check::Logged, _) => (Check::Logged, Vec::new()),
        }
    }
}

fn offered(mounts: &Mounts, release: &Release, whom: &Whom) -> (Check, Vec<Action>) {
    let mounted = mounts.numbers();
    let list = numbers(mounted);
    let mut actions = vec![Action::Log(if mounted == release.patches {
        whom.line(format_args!("mounts {list}: as the release"))
    } else {
        let also: Vec<u8> = mounted.iter().filter(|n| !release.patches.contains(n)).copied().collect();
        let not: Vec<u8> = release.patches.iter().filter(|n| !mounted.contains(n)).copied().collect();
        let mut why = Vec::new();
        if !also.is_empty() {
            why.push(format!("also {}", numbers(&also)));
        }
        if !not.is_empty() {
            why.push(format!("not {}", numbers(&not)));
        }
        whom.line(format_args!("mounts {list}: not as the release: {}", why.join("; ")))
    })];
    let paths: BTreeSet<&str> = release.checked.paths_of(release.project).collect();
    if paths.is_empty() {
        return (Check::Logged, actions);
    }
    let mut out = vec![0; MAX_REQUEST];
    let Some(len) = Request::encode(whom.seed, paths.iter().map(|path| path.as_bytes()), &mut out) else {
        actions.push(Action::Log(whom.line(format_args!(
            "{}: its {} files make no request (srp_validate::checks's limits)",
            release.project,
            paths.len()
        ))));
        return (Check::Logged, actions);
    };
    out.truncate(len);
    actions.push(Action::Send(out));
    let asked = Asked {
        answers: vec![None; paths.len()],
        paths: paths.into_iter().map(str::to_owned).collect(),
        counted: false,
    };
    (Check::Asked(asked), actions)
}

impl Asked {
    fn take(mut self, part: &Part, release: &Release, whom: &Whom) -> (Check, Vec<Action>) {
        let asked = self.paths.len();
        let unlike = |told: usize| {
            let line = whom.line(format_args!(
                "{}: answered {told} files, {asked} asked",
                release.project
            ));
            (Check::Logged, vec![Action::Log(line)])
        };
        match part {
            // A copy of the offer (a reconnection's is another connection's).
            Part::Mounts { files: 0, .. } => return (Check::Asked(self), Vec::new()),
            Part::Mounts { files, .. } => {
                if usize::from(*files) != self.paths.len() {
                    return unlike(usize::from(*files));
                }
                self.counted = true;
            }
            Part::Answers { first, .. } => {
                let first = usize::from(*first);
                let answers = part.answers();
                let Some(slots) = self.answers.get_mut(first..first + answers.len()) else {
                    return unlike(first + answers.len());
                };
                for (slot, answer) in slots.iter_mut().zip(answers) {
                    *slot = Some(*answer);
                }
            }
        }
        if !self.counted || self.answers.iter().any(Option::is_none) {
            return (Check::Asked(self), Vec::new());
        }
        (Check::Logged, vec![Action::Log(self.compared(release, whom))])
    }

    /// The line of a whole answer.
    fn compared(&self, release: &Release, whom: &Whom) -> String {
        let mut differ = Vec::new();
        for (path, answer) in self.paths.iter().zip(&self.answers) {
            let expected = release.checked.files.get(path);
            let what = match (answer, expected) {
                (Some(Answer::Hashed { size, sha256 }), Some(sum)) if same(*size, sha256, sum) => continue,
                (Some(Answer::Hashed { size, sha256 }), Some(sum)) => format!(
                    "{path} size {size} sha256 {}, the release's size {} sha256 {}",
                    hex(sha256),
                    sum.size,
                    hex(&sum.sha256[..PREFIX])
                ),
                (Some(Answer::Hashed { size, sha256 }), None) => {
                    format!("{path} size {size} sha256 {}, not the release's", hex(sha256))
                }
                (Some(Answer::Missing), _) => format!("{path} missing"),
                (Some(Answer::Unreadable), _) => format!("{path} unreadable"),
                (None, _) => format!("{path} unanswered"),
            };
            differ.push(what);
        }
        let count = self.paths.len();
        if differ.is_empty() {
            return whom.line(format_args!("{}: {count} files as the release", release.project));
        }
        let mut line = whom.line(format_args!(
            "{}: {} of {count} files not as the release: {}",
            release.project,
            differ.len(),
            differ.iter().take(LISTED).cloned().collect::<Vec<_>>().join(" | ")
        ));
        if differ.len() > LISTED {
            let _ = write!(line, " | and {} more", differ.len() - LISTED);
        }
        line
    }
}

fn same(size: u32, sha256: &[u8; PREFIX], sum: &Sum) -> bool {
    size == sum.size && sum.sha256.starts_with(sha256)
}

fn numbers(numbers: &[u8]) -> String {
    if numbers.is_empty() {
        return "none".into();
    }
    numbers.iter().map(u8::to_string).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn sum(n: u8) -> Sum {
        Sum { size: u32::from(n) * 100, sha256: [n; 32] }
    }

    fn checked() -> Checked {
        let files: BTreeMap<String, Sum> = [
            ("startup.cfg", 1),
            ("projects/level_01/day/client_project", 2),
            ("projects/level_01/vegetation/green.veg", 3),
            ("textures/flora/grass.dds", 4),
        ]
        .into_iter()
        .map(|(path, n)| (path.to_owned(), sum(n)))
        .collect();
        let own = ["projects/level_01/day/client_project", "projects/level_01/vegetation/green.veg", "textures/flora/grass.dds"];
        Checked {
            files,
            projects: [("level_01/day".to_owned(), own.into_iter().map(str::to_owned).collect())].into(),
            always: ["startup.cfg".to_owned()].into(),
        }
    }

    const WHOM: Whom = Whom { name: "player", account: 7, match_id: 0x1234, seed: 0x5eed };
    const PATCHES: [u8; 3] = [12, 14, 73];

    fn release(checked: &Checked) -> Release<'_> {
        Release { checked, patches: &PATCHES, project: "level_01/day" }
    }

    fn offer(numbers: &[u8]) -> Part {
        Part::Mounts { files: 0, mounts: Mounts::of(numbers).unwrap() }
    }

    fn lines(actions: &[Action]) -> Vec<&str> {
        actions
            .iter()
            .filter_map(|action| match action {
                Action::Log(line) => Some(line.as_str()),
                Action::Send(_) => None,
            })
            .collect()
    }

    /// The parts of a game's answer: `answers` in the request's order.
    fn answer(answers: &[Answer]) -> Vec<Part> {
        let mut report = srp_validate::checks::Report::new(Mounts::of(&PATCHES).unwrap());
        for answer in answers {
            report.add(*answer).unwrap();
        }
        let report = srp_validate::report::Report::Checks(report);
        (0..report.parts())
            .map(|part| {
                let (bytes, len) = report.part(WHOM.seed, part).unwrap();
                match srp_validate::report::Carrier::parse(&bytes[..len]).unwrap().body {
                    srp_validate::report::Body::Checks(part) => part,
                    body => panic!("{body:?}"),
                }
            })
            .collect()
    }

    fn hashed(n: u8) -> Answer {
        let sum = sum(n);
        Answer::Hashed { size: sum.size, sha256: sum.sha256[..PREFIX].try_into().unwrap() }
    }

    #[test]
    fn an_offer_logs_the_mounts_and_asks_the_projects_files_in_order() {
        let checked = checked();
        let (check, actions) = Check::Unoffered.step(&offer(&PATCHES), &release(&checked), &WHOM);
        assert!(matches!(check, Check::Asked(_)));
        assert_eq!(
            lines(&actions),
            ["player (account 7), match 0x1234, mounts 12 14 73: as the release"]
        );
        let Some(Action::Send(bytes)) = actions.last() else { panic!("{actions:?}") };
        let request = Request::parse(bytes).unwrap();
        assert_eq!(request.match_id, WHOM.seed);
        let mut paths = request.paths();
        let mut read = Vec::new();
        while let Some(path) = paths.next_path() {
            read.push(String::from_utf8(path.to_vec()).unwrap());
        }
        assert_eq!(
            read,
            [
                "projects/level_01/day/client_project",
                "projects/level_01/vegetation/green.veg",
                "startup.cfg",
                "textures/flora/grass.dds",
            ]
        );
    }

    #[test]
    fn mounts_unlike_the_releases_say_what_is_more_and_what_is_not_there() {
        let checked = checked();
        let (_, actions) = Check::Unoffered.step(&offer(&[12, 14, 200]), &release(&checked), &WHOM);
        assert_eq!(
            lines(&actions),
            ["player (account 7), match 0x1234, mounts 12 14 200: not as the release: also 200; not 73"]
        );
        let (_, actions) = Check::Unoffered.step(&offer(&[]), &release(&checked), &WHOM);
        assert_eq!(
            lines(&actions),
            ["player (account 7), match 0x1234, mounts none: not as the release: not 12 14 73"]
        );
    }

    fn asked(checked: &Checked) -> Check {
        Check::Unoffered.step(&offer(&PATCHES), &release(checked), &WHOM).0
    }

    /// The answers of the request's order: client_project, green.veg,
    /// startup.cfg, grass.dds.
    fn run(check: Check, parts: &[Part], checked: &Checked) -> (Check, Vec<String>) {
        let mut check = check;
        let mut all = Vec::new();
        for part in parts {
            let (next, actions) = check.step(part, &release(checked), &WHOM);
            all.extend(lines(&actions).into_iter().map(str::to_owned));
            check = next;
        }
        (check, all)
    }

    #[test]
    fn an_answer_as_the_release_is_one_line() {
        let checked = checked();
        let (check, lines) = run(asked(&checked), &answer(&[hashed(2), hashed(3), hashed(1), hashed(4)]), &checked);
        assert!(matches!(check, Check::Logged));
        assert_eq!(lines, ["player (account 7), match 0x1234, level_01/day: 4 files as the release"]);
    }

    #[test]
    fn files_unlike_the_release_are_each_named() {
        let checked = checked();
        let edited = Answer::Hashed { size: 400, sha256: [9; PREFIX] };
        let (_, lines) = run(asked(&checked), &answer(&[hashed(2), Answer::Missing, hashed(1), edited]), &checked);
        assert_eq!(
            lines,
            [
                "player (account 7), match 0x1234, level_01/day: 2 of 4 files not as the release: \
                 projects/level_01/vegetation/green.veg missing | textures/flora/grass.dds size 400 sha256 \
                 0909090909090909, the release's size 400 sha256 0404040404040404"
            ]
        );
    }

    #[test]
    fn the_answer_waits_for_every_part_in_any_order() {
        let checked = checked();
        let mut parts = answer(&[hashed(2), hashed(3), hashed(1), hashed(4)]);
        parts.reverse();
        let (check, lines) = run(asked(&checked), &parts[..1], &checked);
        assert!(matches!(check, Check::Asked(_)));
        assert!(lines.is_empty());
        let (check, lines) = run(check, &parts[1..], &checked);
        assert!(matches!(check, Check::Logged));
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn an_answer_of_another_count_or_to_nothing_asked_is_told_once() {
        let checked = checked();
        let (check, lines) = run(asked(&checked), &answer(&[hashed(2)]), &checked);
        assert!(matches!(check, Check::Logged));
        assert_eq!(lines, ["player (account 7), match 0x1234, level_01/day: answered 1 files, 4 asked"]);
        let (check, lines) = run(Check::Unoffered, &answer(&[hashed(2)]), &checked);
        assert!(matches!(check, Check::Logged));
        assert_eq!(
            lines,
            ["player (account 7), match 0x1234, level_01/day: answered a request it was not sent"]
        );
    }

    #[test]
    fn a_project_that_checks_nothing_asks_nothing() {
        let checked = Checked::default();
        let (check, actions) = Check::Unoffered.step(&offer(&PATCHES), &release(&checked), &WHOM);
        assert!(matches!(check, Check::Logged));
        assert_eq!(lines(&actions).len(), 1, "the mounts alone");
        assert!(!actions.iter().any(|action| matches!(action, Action::Send(_))));
    }

    #[test]
    fn a_long_list_is_cut_and_counted() {
        let mut checked = Checked::default();
        let paths: Vec<String> = (0..20).map(|n| format!("textures/f{n:02}.dds")).collect();
        for path in &paths {
            checked.files.insert(path.clone(), sum(1));
        }
        checked.projects.insert("level_01/day".into(), paths.iter().cloned().collect());
        let (_, lines) = run(asked(&checked), &answer(&[Answer::Unreadable; 20]), &checked);
        assert!(lines[0].contains("20 of 20 files not as the release"), "{}", lines[0]);
        assert_eq!(lines[0].matches(" unreadable").count(), LISTED);
        assert!(lines[0].ends_with(" | and 4 more"), "{}", lines[0]);
    }
}

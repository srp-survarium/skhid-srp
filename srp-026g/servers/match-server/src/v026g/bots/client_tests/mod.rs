//! The client-test runner: client tests of the bots, run one after another by the server while the owner plays.
//! TEST ONLY: compiled only with the cargo feature `client-test` (and in the unit tests), and then inert unless
//! `SRP_CLIENT_TEST=<plan>` is set. The server refuses to start with it set on a live configuration ([`live_reason`]).
//! `srp-026g/docs/notes/client_tests.md` is the owner's and the writers' short guide.
//!
//! # What it does
//!
//! `SRP_CLIENT_TEST` names the plan: `all`, a list of step ids and package names (`a,b`), or `@file` with one per
//! line. A step runs in a match of its [`Group`] (map, mode and, if the step wants it, size); the runner of a match
//! runs the steps that fit it, one after another, the ones with the same group together:
//!
//! 1. **Armed.** The runner picks the bots the step's setup names, scripts them ([`Script`]) and tells the owner the
//!    step's one-line instruction. The bots walk to their places meanwhile.
//! 2. **Running.** When every start condition ([`Cond`]) holds, `CLIENT_TEST step <id> START` is logged and the
//!    measures ([`Measure`]) are sampled from the server's own view of every poll.
//! 3. **End.** At the first end condition ([`End`]) or the step's budget (at most 3 min): `CLIENT_TEST step <id> END
//!    <PASS|FAIL(..)|INFO|VOID(..)>` is logged, a row is appended to the results file, the scripted bots go back to
//!    their ordinary play, and the next step is armed 4 s later.
//!
//! A try the owner did not start in time (`arm_ms`, 2 min) is `VOID(never_started)`; one the match or the owner cut
//! short is not used up and runs again, here or in the next match of the group.
//!
//! When no step is left for the match, the runner tells the owner the next group (map, mode, size), and the log says it.
//!
//! # Files
//!
//! In `SRP_CLIENT_TEST_DIR` (default `logs/client-test`):
//! - `client-test-results-<plan>-<time>.tsv`: one row per try: `match step package try tries result ended_by start_ms
//!   end_ms values` (`name=value;...`, `-` for no value);
//! - `client-test-card-<plan>-<time>.md`: the printed test card: matches in order, each step's instruction, start and
//!   end. It is written at the start; it is also the way to tell the owner when the game cannot (see below).
//!
//! # Writing steps
//!
//! One file per package, `bots/client_tests/<package>.rs`, with `pub fn steps() -> Vec<Step>`, registered by one
//! line in the `packages!` list below. A package never edits another's file or the list but for its own line.
//!
//! ```ignore
//! pub fn steps() -> Vec<Step> {
//!     const EXIT: Spot = Spot::at(47.0, 17.0, 116.0);
//!     vec![
//!         Step::new("sample-camper-exit", Group::new(2, BATTERIES), "Stand at the marked spot ...")
//!             .setup(Who::Nth(Side::Mates, 0), Script::Follow { within_m: 4.0, gait: Gait::Run })
//!             .start(&[Cond::OwnerNear { at: EXIT, r_m: 3.0 }])
//!             .end(&[End::OwnerDied, End::OwnerLeft { at: EXIT, r_m: 12.0 }])
//!             .measure(Measure::OwnerKills)
//!             .bar("owner_kills", Test::AtLeast(2.0))
//!             .budget_s(90)
//!             .tries(2),
//!     ]
//! }
//! ```
//!
//! - [`Step::new`]`(id, group, instruction)`: the id is `<package>-<what>`, unique across packages; the instruction is
//!   one line (at most 160 characters) that the owner reads in the game or on the card. `group`: [`Group::new`]`(level,
//!   mode)` for any size, `.of_size(n)` for `n` a side (Player vs Bots offers 6, 8 and 10; the owner is team 0).
//! - `.setup(who, script)`: [`Who`] is `Nth(Side, n)` (the bots of a side numbered by place from 0) or `All(Side)`;
//!   [`Script`] is `Idle`, `Stand`, `Hold`, `Walk`, `Follow`, `Throw`. A scripted bot's reflex still aims and fires on
//!   sight, except `Idle`'s, and it keeps its ground. A match that lacks a named bot voids the try (`missing_bots`).
//! - `.start(&[..])`: all [`Cond`]s must hold at once, after the owner has had 1.5 s to read: `OwnerNear`,
//!   `BotsInPlace`, `After`. The default is at once.
//! - `.end(&[..])`: the first [`End`] that holds, or the budget (`.budget_s(n)`, default 60, at most 180).
//! - `.measure(..)`, `.bar(name, test)`: every measure is a row value; a bar names one by [`Measure::name`] and the
//!   try is PASS when all bars hold, FAIL otherwise, INFO when the step has no bar. `Measure::Custom` takes two
//!   functions of the package's own, over [`Frame`] (the owner's and the bots' bodies, the bots' notes).
//! - `.tries(n)`: runs back to back, a row each. `.arm_s(n)`: the time to reach the start.
//!
//! What a measure can see is what the server sees: the world's positions, health, kills and deaths and the bots'
//! notes. Hits and damage are read from health and the statistics. Ask the owner only for what the server cannot see.
//!
//! The registry test checks every registered step ([`Step::check`]).
//!
//! # Telling the owner
//!
//! [`Link::take_says`] hands the owner's lines to the match server, which shows them if the game can show a message
//! (see `docs/notes/client_tests.md` for what the client allows). Every line is also logged as `CLIENT_TEST tell: ..`
//! and the card has all of them, in order.

mod frame;
mod hook;
mod plan;
mod report;
mod run;
mod step;

pub use frame::{Body, Frame, Heard, group_of};
pub use hook::script_bot;
pub use report::{Output, card, line_of};
pub use run::{Action, Drive, Outcome, Row, Runner, Todo, VoidWhy};
pub use step::*;

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use plan::Plan;

/// Registers the packages' step files: one line each, in this list. A package adds its own line and nothing else.
macro_rules! packages {
    ($($name:ident,)*) => {
        $(mod $name;)*

        /// Every registered step, in registration order, with the package set.
        pub fn registry() -> Vec<Step> {
            let mut all = Vec::new();
            $(
                for mut step in $name::steps() {
                    step.package = stringify!($name);
                    all.push(step);
                }
            )*
            all
        }
    };
}

packages! {
    sample,
}

/// What is set up by [`init`], once for the process.
struct Live {
    plan: Plan,
    output: Arc<Output>,
}

static LIVE: OnceLock<Live> = OnceLock::new();

/// The server's start: `SRP_CLIENT_TEST` read once. Unset or empty: no client test (Ok). Set on a live configuration, or
/// naming a step that does not exist, or with a results folder that cannot be made: an error that stops the server.
pub fn init() -> Result<(), String> {
    let Some(spec) = std::env::var("SRP_CLIENT_TEST").ok().filter(|s| !s.trim().is_empty()) else {
        return Ok(());
    };
    if LIVE.get().is_some() {
        return Ok(());
    }
    if let Some(why) = live_reason(&|name| std::env::var(name).ok()) {
        return Err(format!(
            "SRP_CLIENT_TEST is set, and this looks like the live server ({why}): the client tests are for a local server only"
        ));
    }
    let all = registry();
    let plan = Plan::parse(&spec, all)?;
    let dir = std::env::var_os("SRP_CLIENT_TEST_DIR")
        .filter(|d| !d.is_empty())
        .map_or_else(|| PathBuf::from("logs/client-test"), PathBuf::from);
    let output = Output::new(&dir, &plan.name).map_err(|e| format!("SRP_CLIENT_TEST: cannot make {}: {e}", dir.display()))?;
    output
        .write_card(&card(&plan.name, plan.steps()))
        .map_err(|e| format!("SRP_CLIENT_TEST: cannot write the card: {e}"))?;
    log::info!(
        "CLIENT_TEST plan {}: {} steps in {} matches; results {}, card {}",
        plan.name,
        plan.steps().len(),
        plan.groups().len(),
        output.results_path().display(),
        output.card_path().display()
    );
    for (n, group) in plan.groups().into_iter().enumerate() {
        log::info!("CLIENT_TEST match {}: {}", n + 1, group.describe());
    }
    let _ = LIVE.set(Live {
        plan,
        output: Arc::new(output),
    });
    Ok(())
}

/// The reason a configuration is the live server's, if it is: the admin page is on, or the public address is not a local
/// one. `get` reads the environment.
pub fn live_reason(get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let set = |name: &str| get(name).filter(|v| !v.trim().is_empty());
    if set("SRP_ADMIN_TOKEN").is_some() {
        return Some("SRP_ADMIN_TOKEN is set, the admin page is on".to_string());
    }
    ["SRP_PUBLIC_HOST", "SRP_LOBBY_PUBLIC_HOST"].into_iter().find_map(|name| {
        let host = set(name)?;
        (!local_host(host.trim())).then(|| format!("{name}={host} is not a local address"))
    })
}

fn local_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| match ip {
            IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
            IpAddr::V6(v6) => v6.is_loopback(),
        })
}

/// Where the owner's lines go: the process that runs the lobby too (`srp`) sets it, with the account's chat link
/// (`docs/notes/client_tests.md`). Not set: the lines are in the log and on the card only.
pub type Say = Box<dyn Fn(u32, &str) + Send + Sync>;

static SAY: OnceLock<Say> = OnceLock::new();

pub fn set_say(say: Say) {
    let _ = SAY.set(say);
}

/// A line for the account's client, if a way to it is set.
pub fn say(account_id: u32, text: &str) {
    if let Some(say) = SAY.get() {
        say(account_id, text);
    }
}

/// The runner of a match of group `played`, when the plan has steps left for it. When it has none, the log says which
/// match the plan wants next.
pub fn for_match(match_id: u32, played: Group) -> Option<Link> {
    let live = LIVE.get()?;
    let todo = live.plan.todo_for(played, &live.output);
    let next = live.plan.next_group(played, &live.output);
    if todo.is_empty() {
        log::info!(
            "CLIENT_TEST: match {match_id:#x} ({}) has no step left in the plan {}{}",
            played.describe(),
            live.plan.name,
            next.map_or(String::new(), |g| format!("; next: {}", g.describe()))
        );
        return None;
    }
    log::info!(
        "CLIENT_TEST: match {match_id:#x} ({}) runs {} steps: {}",
        played.describe(),
        todo.len(),
        todo.iter().map(|t| t.step.id).collect::<Vec<_>>().join(", ")
    );
    Some(Link::new(Runner::new(match_id, played, todo, next), live.output.clone()))
}

/// A match's runner and where its words and rows go.
pub struct Link {
    runner: Runner,
    output: Arc<Output>,
    says: Vec<String>,
}

impl Link {
    pub fn new(runner: Runner, output: Arc<Output>) -> Self {
        Self {
            runner,
            output,
            says: Vec::new(),
        }
    }

    pub fn tick(&mut self, frame: &Frame) {
        let actions = self.runner.tick(frame);
        self.says.extend(self.output.apply(actions));
    }

    pub fn drive(&self, place: crate::v026g::bots::view::Place) -> Option<Drive> {
        self.runner.drive(place)
    }

    /// The lines for the owner since the last call.
    pub fn take_says(&mut self) -> Vec<String> {
        std::mem::take(&mut self.says)
    }

    pub fn output(&self) -> &Output {
        &self.output
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        let actions = self.runner.finish();
        self.output.apply(actions);
    }
}

#[cfg(test)]
mod tests;

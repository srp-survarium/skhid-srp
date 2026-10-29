//! The client-test runner: client tests of the bots, run one after another by the server while the owner plays.
//! TEST ONLY: compiled only with the cargo feature `client-test` (and in the unit tests), and then inert unless
//! `SRP_CLIENT_TEST=<plan>` is set. The server refuses to start with it set on a live configuration ([`live_reason`]).
//! `srp-026g/docs/notes/client_tests.md` is the owner's and the writers' short guide.
//!
//! # What it does
//!
//! `SRP_CLIENT_TEST` names the plan: `all`, a list of step ids and package names (`a,b`), or `@file` with one per
//! line. A step runs in a match of its [`Group`] (map, mode and, if the step wants it, size); the runner of a match
//! runs the steps that fit it, one after another, the ones with the same group together. The plan's first step is
//! [`check::message_check`]: the owner holds C for a second, twice, if he can read the line the runner says (it counts his crouch
//! going down and coming up from the moment the line is told).
//!
//! 1. **Armed.** With the owner on his feet, the runner picks the bots the step's setup names, scripts them ([`Script`]) and
//!    tells the owner the step's one-line instruction (again every 6 s while it waits). The bots walk to their places meanwhile.
//! 2. **Running.** When every start condition ([`Cond`]) holds and the owner is alive, `CLIENT_TEST step <id> START` is
//!    logged and the measures ([`Measure`]) are sampled from the server's own view of every poll.
//! 3. **End.** At the first end condition ([`End`]) or the step's budget (at most 3 min): `CLIENT_TEST step <id> END
//!    <PASS|FAIL(..)|INFO|VOID(..)>` is logged, a row is appended to the results file, the scripted bots go back to
//!    their ordinary play, and the next step is armed 4 s later.
//!
//! A try the owner did not start in time (`arm`, 2 min) is `VOID(never_started)`; one the match or the owner cut
//! short is not used up and runs again, here or in the next match of the group.
//!
//! When no step is left for the match, the runner tells the owner the next group (map, mode, size), and the log says it.
//!
//! # Files
//!
//! In `SRP_CLIENT_TEST_DIR` (default `logs/client-test`):
//! - `client-test-results-<plan>-<time>.tsv`: one row per try: `match sha tuning tuning_source step package try tries result ended_by
//!   start_ms end_ms values` (`name=value;...`, `-` for no value). `sha` is the commit the server was built from
//!   (`SRP_BUILD_SHA`, in the environment or baked in at build time; the server does not start without it), `tuning` the hash
//!   of the bots' tuning and `tuning_source` where it came from (`tuning=defaults`, or the file and its digest, as a bench folder
//!   records it);
//! - `client-test-card-<plan>-<time>.md`: the printed test card: matches in order, each step's instruction, start and
//!   end, with the same sha and tuning. It is written at the start; it is also the way to tell the owner when the game
//!   cannot (see below).
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
//!   mode)` for any size, `.of_size(n)` for `n` a side (Player vs Bots offers 6, 8 and 10; the owner is team 0, with n-1
//!   bots, and the other team is n bots). Player vs Bots offers batteries (2) and team deathmatch (4) only.
//! - `.setup(who, script)`: [`Who`] is `Nth(Side, n)` (the bots of a side numbered by place from 0) or `All(Side)`;
//!   [`Script`] is `Idle`, `Stand`, `Face`, `Hold`, `Walk`, `Follow`, `Throw`. **A scripted bot is the script's:** its legs, look
//!   and keys; nothing else moves it. `.setup(who, script)` gives it the grip the script wants ([`Grip::for_script`]: `Idle`, a
//!   dummy, and `Throw` fire nothing; the others let the reflex fight). `.setup_with(who, script, Grip { fight, react })` says what
//!   is left to the bot: `Fight::Free` lets the reflex aim and fire on sight (but never take the legs), `Fight::Full` leaves it its
//!   legs and side step too (a holder that is to push), `Fight::Never` gives the script the look and the keys and the bot fires
//!   nothing; `React::None` (the default) holds back what else moves a body: the dodge of a grenade, the trap habit and its
//!   reaction to a trap and the jump over one, the step in off a lip, the zone way-out, the pocket escape, the sprint and the
//!   detour round a held lane, the unstick and the pause after it; `React::Normal` leaves them. A match that lacks a named bot
//!   voids the try (`missing_bots`). Bots cannot be placed: they walk, so a start that needs them says `Cond::BotsInPlace`.
//! - `.seat(side, n, role)`: the bot `n` of `side` is a `role` (`BotRole`: its kits and weapons are the role's) for the whole match: the
//!   lobby forms the match with it ([`bot_roles`], the hook `srp` registers with the lobby under the same feature). Two steps of
//!   one match may not ask for two roles of one seat. A bot's tier and its rusher kit are the match's.
//! - `.start(&[..])`: all [`Cond`]s must hold at once, after the owner has had 1.5 s to read, with him alive: `OwnerNear`,
//!   `BotsInPlace`, `After(Span)`, `Custom(WaitFor)`. The default is at once; `.settle(Span)` changes the 1.5 s to read.
//! - `.end(&[..])`: the first [`End`] that holds, or the budget (`.budget_s(n)`, default 60, at most 180): `OwnerDied`,
//!   `OwnerKills`, `OwnerLeft`, `ScriptedDied`, `Custom(EndWhen)`.
//! - `.measure(..)`, `.bar(name, test)`: every measure is row values; a bar names one by [`Measure::all_names`] and the
//!   try is PASS when all bars hold, FAIL otherwise, INFO when the step has no bar. `Measure::Custom` gives several names: its
//!   `make` builds a [`CustomMeter`] (a type of the package's own) that sees every [`Frame`] of the try and gives the value of
//!   each name by its index at the end.
//! - `.tries(n)`: runs back to back, a row each. `.arm_s(n)`: the time to reach the start.
//!
//! What a step can see is what the server sees ([`Frame`]): the world's positions, health, kills and deaths of every body; the
//! owner's health pools (body, head, legs, hands, pain, infection) when a world has shown them; each bot's reflex (its target, the
//! gates that held its fire, why it took its legs, as the reflex wished it); each body's keys, view and speed; where each battery is;
//! the map's line of sight ([`Frame::clear_line`]); and the bots' notes. Hits and damage are read from health and the statistics. Ask the owner only for what the server cannot see. Times are [`At`] (a moment on the match clock)
//! and [`Span`] (a length).
//!
//! [`Step::check`] checks a step; the registry test checks every registered step and a plan refuses one that fails.
//!
//! # Telling the owner
//!
//! [`Link::take_says`] hands the owner's lines to the match server, which sends them to his game's chat (`srp` registers the
//! way, [`set_say`]). Every line is also logged as `CLIENT_TEST tell: ..` and the card has all of them, in order. The line
//! reaching the screen is LIKELY, not seen: hence the first step.

mod check;
#[cfg(test)]
mod dry;
mod frame;
mod hook;
mod plan;
mod report;
mod run;
mod step;
mod time;

pub use frame::{Body, BotView, Frame, Heard, Mind, Owner, Sight, group_of};
pub use hook::script_bot;
pub use report::{Output, card, line_of};
pub use run::{Act, Action, Drive, Outcome, Provenance, Row, Runner, Todo, VoidWhy};
pub use step::*;

use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use plan::Plan;
use survarium::v026g::loadouts::BotRole;

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
    fights,
    sample,
    threat,
    zones,
}

/// What is set up by [`init`], once for the process.
struct Live {
    plan: Plan,
    output: Arc<Output>,
    provenance: Provenance,
}

static LIVE: OnceLock<Live> = OnceLock::new();

/// The server's start, once the bots' tuning is parsed (`tuning` is its hash): `SRP_CLIENT_TEST` read once. Unset or empty: no
/// client test (Ok). Set on a live configuration, or naming a step that does not exist or cannot be run, or with a results
/// folder that cannot be made: an error that stops the server.
pub fn init(tuning: u32, tuning_source: String) -> Result<(), String> {
    let Some(spec) = non_empty("SRP_CLIENT_TEST")? else {
        return Ok(());
    };
    if LIVE.get().is_some() {
        return Ok(());
    }
    if let Some(why) = live_reason(&|name| std::env::var_os(name).map(|v| v.into_string().unwrap_or_else(|_| "?".to_string()))) {
        return Err(format!(
            "SRP_CLIENT_TEST is set, and this looks like the live server ({why}): the client tests are for a local server only"
        ));
    }
    let plan = Plan::parse(&spec, registry())?;
    // The commit the server was built from: set when it is started (`SRP_BUILD_SHA`) or when it was built; a result that cannot say
    // which commit made it is not made.
    let sha = non_empty("SRP_BUILD_SHA")?.or_else(|| option_env!("SRP_BUILD_SHA").map(str::to_string)).ok_or_else(|| {
        "SRP_CLIENT_TEST needs SRP_BUILD_SHA (the commit this server was built from, in the environment now or at build time): every result row says it"
            .to_string()
    })?;
    let provenance = Provenance { sha, tuning, tuning_source };
    let dir = std::env::var_os("SRP_CLIENT_TEST_DIR")
        .filter(|d| !d.is_empty())
        .map_or_else(|| PathBuf::from("logs/client-test"), PathBuf::from);
    let output = Output::new(&dir, &plan.name).map_err(|e| format!("SRP_CLIENT_TEST: cannot make {}: {e}", dir.display()))?;
    output
        .write_card(&card(&plan.name, &provenance, &plan.matches()))
        .map_err(|e| format!("SRP_CLIENT_TEST: cannot write the card: {e}"))?;
    log::info!(
        "CLIENT_TEST plan {}: {} steps in {} matches, build {}, tuning {:08x} ({}); results {}, card {}",
        plan.name,
        plan.steps().len(),
        plan.groups().len(),
        provenance.sha,
        provenance.tuning,
        provenance.tuning_source,
        output.results_path().display(),
        output.card_path().display()
    );
    for (n, group) in plan.groups().into_iter().enumerate() {
        log::info!("CLIENT_TEST match {}: {}", n + 1, group.describe());
    }
    let _ = LIVE.set(Live {
        plan,
        output: Arc::new(output),
        provenance,
    });
    Ok(())
}

/// An environment variable that is set to something: `None` if unset or empty, an error if it is not text.
fn non_empty(name: &str) -> Result<Option<String>, String> {
    match std::env::var_os(name) {
        None => Ok(None),
        Some(value) => match value.into_string() {
            Ok(text) if text.trim().is_empty() => Ok(None),
            Ok(text) => Ok(Some(text.trim().to_string())),
            Err(_) => Err(format!("{name} is not text")),
        },
    }
}

/// The reason a configuration is the live server's, if it is: the admin page is on, or the public address is not a local
/// one. `get` reads the environment (a value that is not text reads as `?`, which is no local address).
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
    Some(Link::new(Runner::new(match_id, live.provenance.clone(), played, todo, next), live.output.clone()))
}

/// The roles of a match's server bots, as the plan wants them (TEST ONLY, called by the lobby when it forms a Player vs Bots match
/// through the hook `srp` registers): `defaults` are the roles the lobby would give the bots `bot_teams` (their teams, by place); a
/// plan that is not set, or has no seat for this match, gives them back unchanged. The owner is on team 0 with `per_team - 1` bots.
pub fn bot_roles(level: u8, mode: u8, per_team: u8, bot_teams: &[u8], defaults: Vec<BotRole>) -> Vec<BotRole> {
    let Some(live) = LIVE.get() else {
        return defaults;
    };
    roles_for(&live.plan.roster_for(Group::new(level, mode).of_size(per_team)), bot_teams, defaults)
}

/// `defaults` with the seats' roles in: the n-th bot of team 0 / team 1 by place. A seat with no such bot is logged and left out.
fn roles_for(roster: &[Seat], bot_teams: &[u8], mut roles: Vec<BotRole>) -> Vec<BotRole> {
    for seat in roster {
        let team = match seat.side {
            Side::Mates => 0,
            Side::Enemies => 1,
        };
        match bot_teams.iter().enumerate().filter(|(_, t)| **t == team).nth(usize::from(seat.nth)) {
            Some((place, _)) => roles[place] = seat.role,
            None => log::warn!("CLIENT_TEST: no {} bot {} in this match: its role {:?} is not given", seat.side.name(), seat.nth, seat.role),
        }
    }
    roles
}

/// A match's runner and where its words and rows go.
pub struct Link {
    runner: Runner,
    output: Arc<Output>,
    says: Vec<String>,
    /// The owner's health pools of the last world that showed them.
    owner_pools: Option<crate::v026g::bots::pools::Pools>,
}

impl Link {
    pub fn new(runner: Runner, output: Arc<Output>) -> Self {
        Self {
            runner,
            output,
            says: Vec::new(),
            owner_pools: None,
        }
    }

    pub fn tick(&mut self, frame: &Frame) {
        let actions = self.runner.tick(frame);
        self.says.extend(self.output.apply(actions));
    }

    pub fn drive(&self, place: crate::v026g::bots::view::Place) -> Option<Drive> {
        self.runner.drive(place)
    }

    /// A world's pools of a place that is no bot: the owner's.
    pub fn note_pools(&mut self, pools: crate::v026g::bots::pools::Pools) {
        self.owner_pools = Some(pools);
    }

    pub fn owner_pools(&self) -> Option<crate::v026g::bots::pools::Pools> {
        self.owner_pools
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

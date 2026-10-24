//! Getting out of pockets: floors a walk from a spawn gets to and no walk
//! gets back from (`WalkGrid::pocket_of`; Cologne CG-3, pit A).
//!
//! A bot on a pocket's floor with a goal outside it has no way by the grid's
//! links: its searches fail and, before this, it stood there aiming for the
//! rest of the match. It now tries the pocket's ways out, jumps up onto a
//! floor that returns (`WalkGrid::pocket_exits`):
//!
//! - each try is a route inside the pocket to the take-off with a jump leg
//!   onto the landing ([`Route::then_jump`]), walked by the motor like a
//!   route's jump (lined up, run up, [`motor::JUMP_TRIES`] presses);
//! - a try worked when the bot stands on a floor out of the pocket; it
//!   failed when the motor gave up on the jump, the route ended in the
//!   pocket, or [`TRY_MS`] went by;
//! - the team's exits that worked are tried first, then the grid's:
//!   standing jumps ([`ExitJump::Standing`], [`STANDING_TRIES`] each, the
//!   shortest across first), then
//!   at most [`SPRINT_EXITS`] sprint-only ones, the lowest first, with one
//!   standing try each (a rounded edge may let it through; no bot sprints
//!   yet); climbs onto steep ground are not tried;
//! - while it may not jump (a broken leg, a hit's stagger: `motor::Hurt`)
//!   the clocks of the try and of the giving up stand still: the motor
//!   presses no jump then, and the time is not the exit's fault;
//! - with every exit tried, or [`GIVE_UP_MS`] gone, it gives up
//!   ([`Trapped::gave_up`]): it stays and aims, and the plays leave it
//!   (`Ctx::trapped`, `Ctx::teammate_trapped`).
//!
//! Every try is traced (`{"escape": …}`, then `{"escaped": …}`), so the
//! owner's matches show which the client lands.

use std::collections::HashMap;
use std::sync::Mutex;

use mapdata::{ExitJump, NodeId, PocketExit, PocketId, Vec3, WalkGrid};

use super::path::{self, Route};
use super::view::Place;

/// A standing-jump exit is tried this many times (the motor presses up to
/// `motor::JUMP_TRIES` times in each). GUESS: the client's standing jump
/// tops at 0.998 m (PROVEN); a second try from a fresh run-up covers a bad
/// line-up.
pub const STANDING_TRIES: u8 = 2;
/// A sprint-only exit gets one standing try (no bot sprints yet) …
pub const SPRINT_TRIES: u8 = 1;
/// … at this many of them, the lowest first. GUESS: the owner got 0.96 m
/// up a rounded edge and then a step up; two looks at the lowest rims cover
/// that without walking the pocket's whole rim.
pub const SPRINT_EXITS: usize = 2;
/// Exits tried at most before giving up. GUESS.
pub const EXITS_MOST: usize = 8;
/// A try is given up after this, ms: the walk inside the pocket and the
/// motor's presses. GUESS: a pit's walk is under 10 m (2.5 s) and the
/// presses about 3 s.
pub const TRY_MS: u32 = 12_000;
/// Giving up on the pocket after this from the first try, ms. GUESS.
pub const GIVE_UP_MS: u32 = 40_000;

/// A bot in a pocket with its goal out of it (`Ctx::trapped`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Trapped {
    /// When it found itself trapped.
    pub since: u32,
    /// Every exit it may take was tried: it stays, aiming.
    pub gave_up: bool,
}

/// A bot's escape from its pocket (in `Stuck`).
#[derive(Clone, Debug)]
pub struct Escape {
    pocket: PocketId,
    since: u32,
    /// The exits to try, in order, and the tries left at each.
    exits: Vec<(PocketExit, u8)>,
    /// The exit being tried and when the try began.
    at: usize,
    trying: Option<u32>,
    gave_up: bool,
    /// The last tick seen, and the time spent unable to jump since the
    /// first (added to [`GIVE_UP_MS`]).
    last: u32,
    held: u32,
    /// For the bench: hurt (no jump) during the try under way, and since
    /// the first try.
    #[cfg(test)]
    hurt: (bool, bool),
}

impl Escape {
    pub fn trapped(&self) -> Trapped {
        Trapped {
            since: self.since,
            gave_up: self.gave_up,
        }
    }

    /// Whether a try is under way (its route is the escape's).
    pub fn trying(&self) -> bool {
        self.trying.is_some()
    }
}

/// What the escape did this tick, for the trace.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A try began: the exit, and its try (1, 2).
    Try(PocketExit, u8),
    /// The try in hand ended: it got out, or not.
    Worked(PocketExit),
    Failed(PocketExit),
    /// Every exit tried: it stays.
    GaveUp,
}

/// The team's escapes that worked and its trapped bots, shared through the
/// tactics (for the match).
#[derive(Debug, Default)]
pub struct Board {
    inner: Mutex<Shared>,
}

#[derive(Debug, Default)]
struct Shared {
    /// Exits that got a bot of the team out of a pocket.
    worked: Vec<(u8, PocketId, PocketExit)>,
    trapped: HashMap<Place, Trapped>,
    /// Whether a pocket has a standing exit, as asked.
    standing: HashMap<PocketId, bool>,
}

impl Board {
    fn shared(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The exits of `pocket` that got a bot of `team` out, latest first.
    pub fn worked(&self, team: u8, pocket: PocketId) -> Vec<PocketExit> {
        self.shared()
            .worked
            .iter()
            .rev()
            .filter(|&&(t, p, _)| t == team && p == pocket)
            .map(|&(_, _, exit)| exit)
            .collect()
    }

    fn learn(&self, team: u8, pocket: PocketId, exit: PocketExit) {
        let mut shared = self.shared();
        shared
            .worked
            .retain(|&(t, p, e)| !(t == team && p == pocket && e.to == exit.to));
        shared.worked.push((team, pocket, exit));
    }

    /// `place`'s bot trapped in a pocket, if it is.
    pub fn trapped(&self, place: Place) -> Option<Trapped> {
        self.shared().trapped.get(&place).copied()
    }

    fn set_trapped(&self, place: Place, trapped: Option<Trapped>) {
        let mut shared = self.shared();
        match trapped {
            Some(trapped) => shared.trapped.insert(place, trapped),
            None => shared.trapped.remove(&place),
        };
    }
}

/// Whether a way out of `pocket` is known that a bot takes now: a standing
/// exit, or one the team got out by. Sprint-only exits count for no one
/// yet (no bot sprints); `carrying`: never for a carrier (it cannot sprint,
/// PROVEN), so once sprint is in this argument keeps it out.
pub fn way_out(
    grid: &WalkGrid,
    board: &Board,
    team: u8,
    pocket: PocketId,
    _carrying: bool,
) -> bool {
    !board.worked(team, pocket).is_empty() || standing_exit(grid, board, pocket)
}

/// Whether `pocket` has a standing-jump exit (asked once a match).
pub fn standing_exit(grid: &WalkGrid, board: &Board, pocket: PocketId) -> bool {
    if let Some(&standing) = board.shared().standing.get(&pocket) {
        return standing;
    }
    let standing = grid
        .pocket_exits(pocket)
        .iter()
        .any(|exit| exit.jump == ExitJump::Standing);
    board.shared().standing.insert(pocket, standing);
    standing
}

/// The exits to try out of the pocket of `from`, in order, with their
/// tries: the team's that worked, the standing ones, the lowest
/// [`SPRINT_EXITS`] sprint-only ones; [`EXITS_MOST`] at most.
fn exits(
    grid: &WalkGrid,
    board: &Board,
    team: u8,
    pocket: PocketId,
    from: NodeId,
) -> Vec<(PocketExit, u8)> {
    let all = grid.pocket_exits(from);
    let mut sprint: Vec<PocketExit> = all
        .iter()
        .copied()
        .filter(|exit| exit.jump == ExitJump::Sprint)
        .collect();
    sprint.sort_by(|a, b| a.rise.total_cmp(&b.rise));
    let mut out: Vec<(PocketExit, u8)> = board
        .worked(team, pocket)
        .into_iter()
        .map(|exit| {
            let tries = if exit.jump == ExitJump::Standing {
                STANDING_TRIES
            } else {
                SPRINT_TRIES
            };
            (exit, tries)
        })
        .collect();
    // The shortest jumps first, then the lowest: how far across a jump
    // carries is not measured (the stand-in's 0.6 m is a GUESS), so the
    // surest are the shortest.
    let mut standing: Vec<PocketExit> = all
        .iter()
        .copied()
        .filter(|exit| exit.jump == ExitJump::Standing)
        .collect();
    standing.sort_by(|a, b| {
        (a.across, a.rise)
            .partial_cmp(&(b.across, b.rise))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let standing = standing.into_iter().map(|exit| (exit, STANDING_TRIES));
    let sprint = sprint
        .into_iter()
        .take(SPRINT_EXITS)
        .map(|exit| (exit, SPRINT_TRIES));
    for (exit, tries) in standing.chain(sprint) {
        if !out.iter().any(|(e, _)| e.to == exit.to) {
            out.push((exit, tries));
        }
    }
    out.truncate(EXITS_MOST);
    out
}

/// What the escape wants this tick.
pub struct Out {
    /// The goal to walk to in place of the play's (an exit's landing): the
    /// route for it is set.
    pub goal: Option<Vec3>,
    pub events: Vec<Event>,
}

/// The escape of the bot of `place` (team `team`) at `feet`, its goal
/// `goal`, at `time`: `escape` is its state, `route` its route and the goal
/// it was planned for. In a pocket its goal is out of, a try's route is
/// set in `route` and its landing given as the goal; out of one, the state
/// is dropped (a try that got it out is learned by the team). With
/// `may_jump` false the try's and the giving up's clocks stand still.
#[allow(clippy::too_many_arguments)]
pub fn tick(
    escape: &mut Option<Escape>,
    route: &mut Option<(Route, Vec3)>,
    grid: &WalkGrid,
    board: &Board,
    place: Place,
    team: u8,
    feet: Vec3,
    goal: Vec3,
    time: u32,
    may_jump: bool,
) -> Out {
    #[cfg(test)]
    if counts::OFF.load(std::sync::atomic::Ordering::Relaxed) {
        return Out {
            goal: None,
            events: Vec::new(),
        };
    }
    step(
        escape, route, grid, board, place, team, feet, goal, time, may_jump,
    )
    .counted()
}

#[allow(clippy::too_many_arguments)]
fn step(
    escape: &mut Option<Escape>,
    route: &mut Option<(Route, Vec3)>,
    grid: &WalkGrid,
    board: &Board,
    place: Place,
    team: u8,
    feet: Vec3,
    goal: Vec3,
    time: u32,
    may_jump: bool,
) -> Out {
    let mut events = Vec::new();
    let node = grid.locate(feet, 1.5);
    let pocket = node.and_then(|node| grid.pocket_of(node));
    // Its goal is in the same pocket: nothing to get out of.
    let goal_in =
        pocket.is_some() && path::goal_floor(grid, goal).and_then(|g| grid.pocket_of(g)) == pocket;
    let (Some(node), Some(pocket), false) = (node, pocket, goal_in) else {
        if let Some(old) = escape.take() {
            // Out of the pocket (or no longer bound out of it): a try under
            // way got it out when its feet are on a floor that returns.
            if let (Some(_), Some(node)) = (old.trying, node)
                && grid.returns(node)
                && let Some(&(exit, _)) = old.exits.get(old.at)
            {
                board.learn(team, old.pocket, exit);
                events.push(Event::Worked(exit));
            }
            board.set_trapped(place, None);
        }
        return Out { goal: None, events };
    };
    let state = match escape {
        Some(state) if state.pocket == pocket => state,
        _ => {
            board.set_trapped(place, None);
            escape.insert(Escape {
                pocket,
                since: time,
                exits: exits(grid, board, team, pocket, node),
                at: 0,
                trying: None,
                gave_up: false,
                last: time,
                held: 0,
                #[cfg(test)]
                hurt: (false, false),
            })
        }
    };
    #[cfg(test)]
    if !may_jump {
        state.hurt = (state.trying.is_some() || state.hurt.0, true);
    }
    if !may_jump {
        let still = time.saturating_sub(state.last);
        state.held += still;
        if let Some(began) = state.trying.as_mut() {
            *began += still;
        }
    }
    state.last = time;
    // The try under way: its route walked on, or ended in the pocket.
    if let Some(began) = state.trying {
        let landing = state
            .exits
            .get(state.at)
            .map(|(exit, _)| grid.position(exit.to));
        let walking = route
            .as_ref()
            .zip(landing)
            .is_some_and(|((_, planned), landing)| planned.distance_xz(landing) < 0.1);
        if walking && time < began + TRY_MS {
            board.set_trapped(place, Some(state.trapped()));
            return Out {
                goal: landing,
                events,
            };
        }
        state.trying = None;
        if walking {
            *route = None;
        }
        if let Some((exit, left)) = state.exits.get_mut(state.at) {
            events.push(Event::Failed(*exit));
            #[cfg(test)]
            if state.hurt.0 {
                counts::hurt(5);
            }
            *left = left.saturating_sub(1);
            if *left == 0 {
                state.at += 1;
            }
        }
    }
    if !state.gave_up {
        // The next try: a route to its take-off inside the pocket.
        let mut next = None;
        while time < state.since + state.held + GIVE_UP_MS
            && let Some(&(exit, left)) = state.exits.get(state.at)
        {
            let inside = |n: NodeId| grid.pocket_of(n) == Some(pocket);
            match path::plan_where(grid, node, exit.from, inside) {
                Some(walk) => {
                    next = Some((exit, left, walk));
                    break;
                }
                None => state.at += 1,
            }
        }
        match next {
            Some((exit, left, walk)) => {
                let landing = grid.position(exit.to);
                *route = Some((walk.then_jump(landing, exit.to), landing));
                state.trying = Some(time);
                #[cfg(test)]
                {
                    state.hurt.0 = false;
                }
                let tries = match exit.jump {
                    ExitJump::Standing => STANDING_TRIES,
                    _ => SPRINT_TRIES,
                };
                events.push(Event::Try(exit, tries - left + 1));
                board.set_trapped(place, Some(state.trapped()));
                return Out {
                    goal: Some(landing),
                    events,
                };
            }
            None => {
                state.gave_up = true;
                events.push(Event::GaveUp);
                #[cfg(test)]
                if state.hurt.1 {
                    counts::hurt(6);
                }
            }
        }
    }
    board.set_trapped(place, Some(state.trapped()));
    Out { goal: None, events }
}

impl Out {
    fn counted(self) -> Self {
        #[cfg(test)]
        self.events.iter().for_each(counts::count);
        self
    }
}

/// The trace line of an escape's event.
pub fn trace(time: u32, place: Place, grid: &WalkGrid, event: &Event) -> serde_json::Value {
    let exit_line = |exit: &PocketExit| {
        let (from, to) = (grid.position(exit.from), grid.position(exit.to));
        serde_json::json!({
            "from": [from.x, from.y, from.z], "to": [to.x, to.y, to.z],
            "rise": exit.rise, "across": exit.across, "jump": format!("{:?}", exit.jump),
        })
    };
    match event {
        Event::Try(exit, attempt) => serde_json::json!({
            "t": time, "bot": place, "escape": exit_line(exit), "try": attempt,
        }),
        Event::Worked(exit) => serde_json::json!({
            "t": time, "bot": place, "escaped": "worked", "exit": exit_line(exit),
        }),
        Event::Failed(exit) => serde_json::json!({
            "t": time, "bot": place, "escaped": "failed", "exit": exit_line(exit),
        }),
        Event::GaveUp => serde_json::json!({ "t": time, "bot": place, "escaped": "gave up" }),
    }
}

/// The escapes' events so far, for the bench's count: tries, worked,
/// failed, gave up, slots handed on by bots given up (`mod.rs`), and tries
/// failed and give-ups of bots that were hurt (no jump) in them.
#[cfg(test)]
pub mod counts {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTS: [AtomicU64; 7] = [const { AtomicU64::new(0) }; 7];

    /// The escape off, for the pocket sweep's before (`tests.rs`).
    pub static OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

    pub fn count(event: &super::Event) {
        let k = match event {
            super::Event::Try(..) => 0,
            super::Event::Worked(_) => 1,
            super::Event::Failed(_) => 2,
            super::Event::GaveUp => 3,
        };
        COUNTS[k].fetch_add(1, Ordering::Relaxed);
    }

    /// A bot given up in a pocket left a slotted play.
    pub fn handed() {
        COUNTS[4].fetch_add(1, Ordering::Relaxed);
    }

    /// A try failed (5) or a give-up (6) of a bot hurt in it.
    pub fn hurt(k: usize) {
        COUNTS[k].fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> [u64; 7] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

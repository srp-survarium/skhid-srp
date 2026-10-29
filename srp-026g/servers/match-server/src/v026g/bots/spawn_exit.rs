//! The way out of the spawn (PR C, design-p1c; fx-cover's clump split): a respawned bot whose play's goal lies outside its
//! side's spawn area leaves by one of its spawn's ways out (`Tactics::ways_out`: where the walks from the spawn to the map's
//! objectives and the enemy's spawn leave it, worked out once a map), chosen so the team's bots that came back together
//! do not all walk out of one: each exit costs its way to the goal through it times `1 + claim_cost` for each teammate
//! that claimed it lately, among the exits whose way is at most `slack_m` longer than the shortest one (A*mbush's cost
//! per agent on a node, Valve's squad route claims). The bots' clump is 1.4 times the live players' per available mate,
//! and about half of the excess is in the first 20 s of a life (10-20 s: 0.51 mates near per available mate, live 0.36).
//!
//! Only the goal of the walk changes: the exit first, then the play's own goal. Not steered: a bot that follows or joins
//! a teammate (Buddy, Escort, PairUp, Trade: being near him is their point), a carrier, a play that uses something
//! (`Intent::objective`), and a goal inside the spawn area. The reflex's legs, a grenade's way out and the rest still
//! override it in the drive, as they do any play's goal.

use super::*;
use crate::v026g::bots::know::TeamKnowledge;
use crate::v026g::bots::plays::{Go, Intent, PlayKind};
use crate::v026g::bots::tuning::SpawnExits;

/// A life is steered out of its spawn this long at most, ms (the clump's excess is in the first 20 s).
pub const WINDOW_MS: u32 = 20_000;
/// An exit is reached within this, m: the walk goes on to the play's goal.
pub const EXIT_REACH_M: f32 = 4.0;

/// A bot's way out of its spawn, for one life (by its `spawned` time).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SpawnExit {
    /// No life chose yet: the next steer of a new life chooses.
    #[default]
    Fresh,
    /// The life born at `life` walks out by exit `exit` (at `at`) first.
    Via { life: u32, exit: usize, at: Vec3 },
    /// The life born at `life` is out of its spawn's way (its exit reached, the area left, the window over, or a play
    /// that is not steered): steered no more.
    Out { life: u32 },
}

/// Why a life was not steered or stopped being steered (the bench's counts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It took an exit other than its shortest way's: its walk goes there first.
    Steered,
    /// The shortest way's exit was the cheapest: nothing changed but the claim.
    Natural,
    /// Its play follows or joins a teammate, uses something, or it carries.
    Exempt,
    /// Its exit reached.
    Reached,
    /// Out of the spawn area before its exit (another way out).
    LeftArea,
    /// The window ended first.
    Timeout,
}

/// What a steer reads: the bot's view of itself, its side, its play, the spawn's ways out and area (for the match's
/// `objective`), now.
pub struct Steer<'a> {
    pub tactics: &'a Tactics,
    pub objective: crate::v026g::bots::objective::ObjectiveMap,
    pub me: &'a PlaceView,
    pub side: Option<i32>,
    pub kind: PlayKind,
    pub now: u32,
}

/// The play's `intent` with the way out of the spawn put first, and the life's state moved on (`state`); the team's exit
/// claims (`knowledge`) updated when it chooses.
pub fn steer(state: &mut SpawnExit, rules: SpawnExits, at: Steer, knowledge: &mut TeamKnowledge, mut intent: Intent) -> Intent {
    let SpawnExits::Spread { slack_m, claim_cost } = rules else {
        return intent;
    };
    let Some(side) = at.side else {
        return intent;
    };
    let life = at.me.spawned;
    let current = match *state {
        SpawnExit::Via { life: l, .. } | SpawnExit::Out { life: l } if l == life => *state,
        _ => SpawnExit::Fresh,
    };
    let team = at.me.team;
    let out = |why| {
        count(team, why);
        SpawnExit::Out { life }
    };
    *state = match current {
        SpawnExit::Out { .. } => current,
        _ if at.now.saturating_sub(life) > WINDOW_MS => out(Outcome::Timeout),
        SpawnExit::Via { at: exit, .. } if at.me.feet.distance_xz(exit) <= EXIT_REACH_M => out(Outcome::Reached),
        SpawnExit::Via { .. } if !at.tactics.in_spawn_area(at.me.feet, Some(side)) => out(Outcome::LeftArea),
        SpawnExit::Via { .. } => current,
        SpawnExit::Fresh => match goal_out_of_spawn(&at, side, &intent) {
            // Nothing to steer yet (it stays, or its goal is in the spawn): it chooses later.
            None => SpawnExit::Fresh,
            Some(_) if exempt(&at, &intent) => out(Outcome::Exempt),
            Some(goal) => choose(&at, side, goal, slack_m, claim_cost, knowledge, life),
        },
    };
    if let SpawnExit::Via { at: exit, .. } = *state
        && goal_out_of_spawn(&at, side, &intent).is_some()
        && !exempt(&at, &intent)
    {
        intent.go = Go::To { at: exit, within_m: EXIT_REACH_M * 0.7 };
        intent.straight = false;
    }
    intent
}

/// The intent's goal, when it walks somewhere outside the side's spawn area.
fn goal_out_of_spawn(at: &Steer, side: i32, intent: &Intent) -> Option<Vec3> {
    match intent.go {
        Go::To { at: goal, within_m } if at.me.feet.distance_xz(goal) > within_m && !at.tactics.in_spawn_area(goal, Some(side)) => {
            Some(goal)
        }
        _ => None,
    }
}

/// Plays whose point is to be near a teammate, a use, a carrier: not steered.
fn exempt(at: &Steer, intent: &Intent) -> bool {
    matches!(at.kind, PlayKind::Buddy | PlayKind::Escort | PlayKind::PairUp | PlayKind::Trade)
        || intent.objective
        || at.me.carrying.is_some()
}

/// The exit this life leaves by (claimed for the team), or out when there is no choice.
fn choose(at: &Steer, side: i32, goal: Vec3, slack_m: f32, claim_cost: f32, knowledge: &mut TeamKnowledge, life: u32) -> SpawnExit {
    let exits = at.tactics.ways_out(side, at.objective);
    let way = |e: Vec3| at.me.feet.distance_xz(e) + e.distance_xz(goal);
    let Some((shortest, best_m)) = exits.iter().map(|&e| way(e)).enumerate().min_by(|a, b| a.1.total_cmp(&b.1)) else {
        return SpawnExit::Out { life };
    };
    let me = at.me.place;
    let (exit, _) = exits
        .iter()
        .enumerate()
        .filter(|&(_, &e)| way(e) <= best_m + slack_m)
        .map(|(i, &e)| (i, way(e) * (1.0 + claim_cost * knowledge.exit_claims_of_others(me, side, i, at.now, WINDOW_MS) as f32)))
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
        .unwrap_or((shortest, best_m));
    knowledge.claim_exit(me, side, exit, at.now);
    if exit == shortest {
        count(at.me.team, Outcome::Natural);
        return SpawnExit::Out { life };
    }
    count(at.me.team, Outcome::Steered);
    SpawnExit::Via { life, exit, at: exits[exit] }
}

#[cfg(test)]
thread_local! {
    static COUNTED: std::cell::Cell<[[u64; 6]; 2]> = const { std::cell::Cell::new([[0; 6]; 2]) };
}

/// The outcomes counted so far, by team and [`Outcome`] in order (the bench's).
#[cfg(test)]
pub fn counted() -> [[u64; 6]; 2] {
    COUNTED.with(std::cell::Cell::get)
}

/// The outcomes' names, by [`Outcome`] in order.
pub const NAMES: [&str; 6] = ["steered", "natural", "exempt", "reached", "left_area", "timeout"];

#[cfg(test)]
fn count(team: u8, why: Outcome) {
    COUNTED.with(|c| {
        let mut all = c.get();
        all[usize::from(team & 1)][why as usize] += 1;
        c.set(all);
    });
}

#[cfg(not(test))]
fn count(_: u8, _: Outcome) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::plays::Pace;
    use crate::v026g::bots::view::Mode;

    const SPREAD: SpawnExits = SpawnExits::Spread { slack_m: 200.0, claim_cost: 1.0 };

    /// Two bots of team 0 come back in side 0's spawn (x = 5) at 299 s, each with a goal at the far side (x = 75). A
    /// battery spawner at (35, 0, 3), south short of the wall, gives the spawn a second way out (the enemy's spawn and
    /// container are reached round the wall's north end).
    fn spawned() -> (Scenario, Vec3) {
        let mut s = Scenario::field(Mode::Batteries);
        let world = crate::v026g::bots::tactics::tests::field_with(|map| {
            let position = Vec3::new(35.0, 0.0, 3.0);
            map.battery_spawners.push(mapdata::BatterySpawner { position, item: position, layer: String::new() });
        });
        s.tactics = Tactics::new(world, 1.0);
        for place in [0, 2] {
            let p = s.bot(place, 0, Vec3::new(5.0, 0.0, 20.0));
            p.spawned = 299_000;
        }
        (s, Vec3::new(75.0, 0.0, 20.0))
    }

    fn to(goal: Vec3) -> Intent {
        Intent::to(goal, Pace::Run)
    }

    fn steer_of(s: &mut Scenario, place: Place, kind: PlayKind, state: &mut SpawnExit, rules: SpawnExits, intent: Intent) -> Intent {
        let me = s.view.places.iter().find(|p| p.place == place).unwrap().clone();
        let at = Steer { tactics: &s.tactics, objective: s.view.objective(), me: &me, side: Some(0), kind, now: 300_000 };
        steer(state, rules, at, &mut s.team, intent)
    }

    #[test]
    fn a_second_bot_back_with_a_teammate_leaves_by_another_exit() {
        let (mut s, goal) = spawned();
        let ways = s.tactics.ways_out(0, s.view.objective()).to_vec();
        assert!(ways.len() >= 2, "the field's spawn has two ways out or more: {ways:?}");
        assert!(s.tactics.in_spawn_area(Vec3::new(5.0, 0.0, 20.0), Some(0)), "the bots stand in their spawn");
        let (mut first, mut second) = (SpawnExit::Fresh, SpawnExit::Fresh);
        steer_of(&mut s, 0, PlayKind::Collect, &mut first, SPREAD, to(goal));
        // The first bot's claim: the one exit claimed by a bot other than place 2.
        let exits = ways.len();
        let first_exit = (0..exits)
            .find(|&e| s.team.exit_claims_of_others(2, 0, e, 300_000, WINDOW_MS) > 0)
            .expect("the first bot claimed its exit");
        let intent = steer_of(&mut s, 2, PlayKind::Collect, &mut second, SPREAD, to(goal));
        match second {
            SpawnExit::Via { exit, at, .. } => {
                assert_ne!(exit, first_exit, "{first:?} {second:?}");
                assert_eq!(intent.go, Go::To { at, within_m: EXIT_REACH_M * 0.7 }, "the walk goes to the exit first");
            }
            other => panic!("the second bot is spread: {other:?} (first {first:?})"),
        }
    }

    #[test]
    fn off_and_the_plays_near_a_teammate_change_nothing() {
        let (mut s, goal) = spawned();
        let mut state = SpawnExit::Fresh;
        assert_eq!(steer_of(&mut s, 0, PlayKind::Collect, &mut state, SpawnExits::Off, to(goal)), to(goal));
        assert_eq!(state, SpawnExit::Fresh);
        for kind in [PlayKind::Buddy, PlayKind::Escort, PlayKind::PairUp, PlayKind::Trade] {
            let mut state = SpawnExit::Fresh;
            assert_eq!(steer_of(&mut s, 0, kind, &mut state, SPREAD, to(goal)), to(goal), "{kind:?}");
            assert_eq!(state, SpawnExit::Out { life: 299_000 }, "{kind:?}");
        }
        // A goal inside the spawn: nothing yet, the life chooses later.
        let mut state = SpawnExit::Fresh;
        let near = to(Vec3::new(6.0, 0.0, 22.0));
        assert_eq!(steer_of(&mut s, 0, PlayKind::Collect, &mut state, SPREAD, near), near);
        assert_eq!(state, SpawnExit::Fresh);
    }

    #[test]
    fn an_exit_reached_and_a_new_life_move_the_state_on() {
        let (mut s, goal) = spawned();
        let exit = s.tactics.ways_out(0, s.view.objective())[0];
        let mut state = SpawnExit::Via { life: 299_000, exit: 0, at: exit };
        s.view.places.iter_mut().find(|p| p.place == 0).unwrap().feet = exit;
        assert_eq!(steer_of(&mut s, 0, PlayKind::Collect, &mut state, SPREAD, to(goal)), to(goal));
        assert_eq!(state, SpawnExit::Out { life: 299_000 });
        // A new life chooses again.
        s.view.places.iter_mut().find(|p| p.place == 0).unwrap().feet = Vec3::new(5.0, 0.0, 20.0);
        s.view.places.iter_mut().find(|p| p.place == 0).unwrap().spawned = 299_500;
        steer_of(&mut s, 0, PlayKind::Collect, &mut state, SPREAD, to(goal));
        assert!(!matches!(state, SpawnExit::Out { life: 299_000 } | SpawnExit::Fresh), "{state:?}");
    }
}

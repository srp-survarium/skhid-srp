//! What a team knows of its bear traps, and the floors its bots keep off
//! (`docs/notes/bear_traps.md`, `traps.rs`).
//!
//! The clients run every trap; the server learns of one only from a world's
//! objects section (`survarium::v026g::match_msgs::traps::find`), which gives
//! its place, state and placing time but not whose it is. So:
//!
//! - A bot that lets go of a trap **promises** it at once ([`TrapList::promise`]):
//!   a provisional entry at the point it looked at, so the team keeps off it
//!   and counts it from that tick.
//! - The next world that shows a trap within [`MATCH_M`] of the promise and
//!   placed within [`MATCH_MS`] of the release **confirms** it: the exact
//!   place, time and state replace the promise ([`TrapList::sync`]). A promise
//!   no world shows [`CONFIRM_MS`] after the release did not happen (a refused
//!   release: humans fail a third of theirs) and is dropped, counted.
//! - A confirmed trap is followed in every world: armed, fired (spent for 8 s
//!   then gone), disarmed (8 s then gone); one a world no longer shows is gone
//!   (fired out, or the oldest recycled by its owner's next).
//! - A trap a world shows that no bot of the team promised is not the team's
//!   to know: a human's, or another team's. It stays out of the list until a
//!   bot has seen it (a later change).
//!
//! The team's bots keep off the floors within [`FIELD_M`] of a standing trap
//! ([`TrapField`]): the route searches shut them (`RouteFilter::with_floors`)
//! and a route that runs over a disc is dropped to be searched again
//! (`traps::keep_off`). Sharing the list among the bots is the ledger's C-T1:
//! the placer could say it and the trap is a model on the floor.

use mapdata::{NodeId, Vec3, WalkGrid};
use survarium::v026g::match_msgs::traps::{PlacedTrap, TrapState};

use super::traps::{FIELD_M, off_route_m};
use super::view::Place;

/// A world's trap is the bot's promise if it lies this near, metres.
pub const MATCH_M: f32 = 1.5;
/// ... and was placed within this of the release, ms.
pub const MATCH_MS: u32 = 2_000;
/// A promise no world has shown this long after the release is dropped, ms.
pub const CONFIRM_MS: u32 = 20_000;
/// A trap a world keeps is the same one if it is this near, metres.
const SAME_M: f32 = 0.3;
/// A body of the team with a broken leg this near a trap of the team's that
/// went off was probably its victim, metres.
pub const CAUGHT_NEAR_M: f32 = 8.0;

/// How the teams' own traps went off: the bench counter "own trap caught an
/// own-team body".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OwnTrapCounts {
    /// Traps of the teams that went off (to *fired*).
    pub fired: u32,
    /// Of them, those with a broken-legged body of the same team near (a
    /// proxy: bots only).
    pub own_team_caught: u32,
}

/// Where a team's trap is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// The bot let go at `released`; no world has shown it yet.
    Promised {
        released: u32,
    },
    Armed,
    Fired,
    Disarmed,
}

/// A trap of the team.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnownTrap {
    pub at: Vec3,
    /// The match ms it was placed at (the world's), else the release.
    pub placed_ms: u32,
    pub by: Place,
    pub phase: Phase,
}

impl KnownTrap {
    /// It can still catch a body: promised or armed.
    pub fn standing(&self) -> bool {
        matches!(self.phase, Phase::Promised { .. } | Phase::Armed)
    }
}

/// The team's traps, and how its guesses fared.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrapList {
    traps: Vec<KnownTrap>,
    version: u64,
    /// Promises a world confirmed, and promises dropped unconfirmed.
    pub confirmed: u32,
    pub missed: u32,
}

impl TrapList {
    pub fn all(&self) -> &[KnownTrap] {
        &self.traps
    }

    /// Changes whenever the list does: the field is built again for it.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The traps that can still catch a body.
    pub fn standing(&self) -> impl Iterator<Item = &KnownTrap> {
        self.traps.iter().filter(|trap| trap.standing())
    }

    pub fn standing_count(&self) -> usize {
        self.standing().count()
    }

    /// `by` let go of a trap at `at` at `released`.
    pub fn promise(&mut self, by: Place, at: Vec3, released: u32) {
        self.traps.push(KnownTrap {
            at,
            placed_ms: released,
            by,
            phase: Phase::Promised { released },
        });
        self.version += 1;
    }

    /// A world of `world_ms` showed `found`. Returns where the team's traps
    /// went off since the last world (to *fired*).
    pub fn sync(&mut self, world_ms: u32, found: &[PlacedTrap]) -> Vec<Vec3> {
        let mut fired = Vec::new();
        let mut used = vec![false; found.len()];
        let mut changed = false;
        // The confirmed ones first: followed, or gone.
        self.traps.retain_mut(|trap| {
            if matches!(trap.phase, Phase::Promised { .. }) {
                return true;
            }
            let seen = found.iter().enumerate().position(|(i, f)| {
                !used[i]
                    && f.placed_ms == trap.placed_ms
                    && Vec3::from_array(f.position).distance_xz(trap.at) < SAME_M
            });
            let Some(i) = seen else {
                changed = true;
                return false;
            };
            used[i] = true;
            let phase = phase_of(found[i].state);
            if phase != trap.phase {
                if phase == Phase::Fired {
                    fired.push(trap.at);
                }
                trap.phase = phase;
                changed = true;
            }
            true
        });
        // The promises: confirmed by the nearest unused trap, else waited
        // for, else dropped.
        let mut missed = 0;
        let mut confirmed = 0;
        self.traps.retain_mut(|trap| {
            let Phase::Promised { released } = trap.phase else {
                return true;
            };
            let best = found
                .iter()
                .enumerate()
                .filter(|(i, f)| {
                    !used[*i]
                        && f.placed_ms.abs_diff(released) <= MATCH_MS
                        && Vec3::from_array(f.position).distance_xz(trap.at) <= MATCH_M
                })
                .min_by(|a, b| {
                    let d = |f: &PlacedTrap| Vec3::from_array(f.position).distance_xz(trap.at);
                    d(a.1).total_cmp(&d(b.1))
                });
            if let Some((i, f)) = best {
                used[i] = true;
                trap.at = Vec3::from_array(f.position);
                trap.placed_ms = f.placed_ms;
                trap.phase = phase_of(f.state);
                if trap.phase == Phase::Fired {
                    fired.push(trap.at);
                }
                confirmed += 1;
                changed = true;
                return true;
            }
            if world_ms > released + CONFIRM_MS {
                missed += 1;
                changed = true;
                return false;
            }
            true
        });
        self.confirmed += confirmed;
        self.missed += missed;
        if changed {
            self.version += 1;
        }
        fired
    }
}

fn phase_of(state: TrapState) -> Phase {
    match state {
        TrapState::Armed => Phase::Armed,
        TrapState::Fired => Phase::Fired,
        TrapState::Disarmed => Phase::Disarmed,
    }
}

/// The floors a team's bots keep off, and the discs they are round.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrapField {
    discs: Vec<Vec3>,
    /// Sorted.
    shut: Vec<NodeId>,
}

impl TrapField {
    /// The field of the standing traps in `list`: the floors whose centre is
    /// within [`FIELD_M`] of a trap, at about its height; a leg between
    /// two open floors is tested by [`Self::hits`] against the same.
    pub fn of(list: &TrapList, grid: &WalkGrid) -> Self {
        Self::round(list.standing().map(|trap| trap.at).collect(), grid)
    }

    /// The field of traps at `discs`.
    pub fn round(discs: Vec<Vec3>, grid: &WalkGrid) -> Self {
        let reach = FIELD_M;
        let mut shut: Vec<NodeId> = discs
            .iter()
            .flat_map(|disc| {
                let area = [
                    disc.x - reach,
                    disc.z - reach,
                    disc.x + reach,
                    disc.z + reach,
                ];
                grid.floors_in(area)
                    .filter(|&node| {
                        let p = grid.position(node);
                        p.distance_xz(*disc) <= reach && (p.y - disc.y).abs() < 1.0
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        shut.sort_unstable();
        shut.dedup();
        Self { discs, shut }
    }

    pub fn is_empty(&self) -> bool {
        self.discs.is_empty()
    }

    pub fn discs(&self) -> &[Vec3] {
        &self.discs
    }

    /// Whether a search may use `node`.
    pub fn open(&self, node: NodeId) -> bool {
        self.shut.binary_search(&node).is_err()
    }

    /// Whether the polyline through `points` runs over a disc.
    pub fn hits(&self, points: &[Vec3]) -> bool {
        self.discs
            .iter()
            .any(|disc| off_route_m(points, *disc) < FIELD_M - 0.05)
    }
}

/// A team's trap state, by feature ([`super::features`]).
#[derive(Default)]
pub struct TeamTraps {
    /// The floors of the traps its bots put down, and the list's version it was built for.
    pub field: std::sync::Arc<TrapField>,
    pub field_version: u64,
    /// The armed traps the last world showed, for the bots' notice (`trap_react.rs`); empty while the switch is off.
    pub world_traps: std::sync::Arc<Vec<survarium::v026g::match_msgs::traps::PlacedTrap>>,
    /// Bots that began shooting a trap lately: which, who, when (`trap_react.rs`, the count of double shooters).
    pub shooters: Vec<(super::trap_react::TrapKey, Place, u32)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::route_filter::RouteFilter;
    use crate::v026g::bots::view::Mode;
    use std::sync::Arc;

    fn world_trap(at: Vec3, state: TrapState, placed_ms: u32) -> PlacedTrap {
        PlacedTrap {
            state,
            position: at.to_array(),
            up: [0.0, 1.0, 0.0],
            placed_ms,
        }
    }

    #[test]
    fn a_promise_is_confirmed_by_the_world_and_followed_to_its_end() {
        let at = Vec3::new(30.0, 0.0, 10.0);
        let mut list = TrapList::default();
        list.promise(3, at, 100_000);
        assert_eq!(list.standing_count(), 1);
        let v = list.version();
        // A world of before the release shows nothing: still waited for.
        list.sync(99_000, &[]);
        assert_eq!(list.standing_count(), 1);
        // A world shows it, a hand's breadth off, placed 300 ms after the release.
        let exact = Vec3::new(30.1, 0.0, 10.05);
        list.sync(103_000, &[world_trap(exact, TrapState::Armed, 100_300)]);
        let trap = list.all()[0];
        assert_eq!(
            (trap.phase, trap.at, trap.placed_ms),
            (Phase::Armed, exact, 100_300)
        );
        assert_eq!((list.confirmed, list.missed), (1, 0));
        assert!(list.version() > v);
        // Fired: spent, no longer standing, still known.
        list.sync(110_000, &[world_trap(exact, TrapState::Fired, 100_300)]);
        assert_eq!(
            (list.all()[0].phase, list.standing_count()),
            (Phase::Fired, 0)
        );
        // Gone from the next world: gone.
        list.sync(120_000, &[]);
        assert!(list.all().is_empty());
    }

    #[test]
    fn a_promise_no_world_shows_is_dropped_and_counted_and_a_foreign_trap_is_not_taken() {
        let at = Vec3::new(30.0, 0.0, 10.0);
        let mut list = TrapList::default();
        list.promise(3, at, 100_000);
        // A trap elsewhere (a human's) does not confirm it, and is not kept.
        let foreign = world_trap(Vec3::new(50.0, 0.0, 10.0), TrapState::Armed, 100_100);
        list.sync(105_000, &[foreign]);
        assert_eq!((list.all().len(), list.confirmed), (1, 0));
        // One near but placed long before is not it either.
        let old = world_trap(at, TrapState::Armed, 60_000);
        list.sync(110_000, &[old]);
        assert_eq!(list.all().len(), 1);
        // Not shown within the confirming time: dropped.
        list.sync(100_000 + CONFIRM_MS + 1, &[]);
        assert!(list.all().is_empty());
        assert_eq!(list.missed, 1);
    }

    #[test]
    fn two_promises_take_their_own_nearest_trap() {
        let (a, b) = (Vec3::new(30.0, 0.0, 10.0), Vec3::new(33.0, 0.0, 10.0));
        let mut list = TrapList::default();
        list.promise(1, a, 100_000);
        list.promise(2, b, 100_200);
        let found = [
            world_trap(b + Vec3::new(0.2, 0.0, 0.0), TrapState::Armed, 100_300),
            world_trap(a + Vec3::new(0.2, 0.0, 0.0), TrapState::Armed, 100_100),
        ];
        list.sync(104_000, &found);
        assert_eq!(list.confirmed, 2);
        let by = |p: Place| list.all().iter().find(|t| t.by == p).unwrap().at;
        assert!(by(1).distance_xz(a) < 0.5 && by(2).distance_xz(b) < 0.5);
    }

    #[test]
    fn the_field_shuts_the_floors_round_a_trap_and_a_search_goes_round_them() {
        let s = Scenario::field(Mode::TeamDeathmatch);
        let grid = &s.tactics.world.grid;
        // The field is open ground between x = 10 and x = 38, z 0 to 40.
        let trap = Vec3::new(25.0, 0.0, 10.0);
        let mut list = TrapList::default();
        list.promise(0, trap, 1_000);
        let field = Arc::new(TrapField::of(&list, grid));
        assert!(!field.is_empty());
        let on = grid.locate(trap, 0.6).unwrap();
        let far = grid.locate(Vec3::new(25.0, 0.0, 14.0), 0.6).unwrap();
        assert!(!field.open(on) && field.open(far));
        // A search along z = 10 from x = 15 to 35 keeps off the trap.
        let from = grid.locate(Vec3::new(15.0, 0.0, 10.0), 1.0).unwrap();
        let to = grid.locate(Vec3::new(35.0, 0.0, 10.0), 1.0).unwrap();
        let rule = field.clone();
        let filter = RouteFilter::open().with_floors(move |node| rule.open(node));
        let plain = s
            .tactics
            .route_nodes(from, to, 0, Some(0), false, &RouteFilter::open())
            .expect("a route");
        let around = s
            .tactics
            .route_nodes(from, to, 0, Some(0), false, &filter)
            .expect("a route round it");
        let points = |r: &crate::v026g::bots::path::Route| {
            r.points().iter().map(|p| p.at).collect::<Vec<_>>()
        };
        assert!(
            field.hits(&points(&plain)),
            "the plain route runs over the trap"
        );
        assert!(
            !field.hits(&points(&around)),
            "the route round it does not: {:?}",
            points(&around)
        );
        assert!(off_route_m(&points(&around), trap) >= FIELD_M - 0.05);
    }

    #[test]
    fn the_field_is_the_triggers_reach_and_the_routes_tolerance_not_a_metre_and_more() {
        // 0.6 m (PROVEN) and 0.15 m: a corridor of 1.6 m keeps a lane.
        assert!((FIELD_M - 0.75).abs() < 1e-6);
        let s = Scenario::field(Mode::TeamDeathmatch);
        let grid = &s.tactics.world.grid;
        let trap = Vec3::new(25.0, 0.0, 10.0);
        let mut list = TrapList::default();
        list.promise(0, trap, 1_000);
        let field = TrapField::of(&list, grid);
        let near = |r0: f32, r1: f32| -> Vec<NodeId> {
            let area = [trap.x - r1, trap.z - r1, trap.x + r1, trap.z + r1];
            grid.floors_in(area)
                .filter(|&n| {
                    let d = grid.position(n).distance_xz(trap);
                    d > r0 && d <= r1 && (grid.position(n).y - trap.y).abs() < 1.0
                })
                .collect()
        };
        let inside = near(0.0, FIELD_M);
        let outside = near(FIELD_M, 1.2);
        assert!(!inside.is_empty() && !outside.is_empty());
        assert!(
            inside.iter().all(|&n| !field.open(n)),
            "every floor within 0.75 m is shut"
        );
        assert!(
            outside.iter().all(|&n| field.open(n)),
            "every floor beyond is open"
        );
        // A leg 0.7 m off the trap hits, one 0.8 m off does not.
        let leg = |z: f32| [Vec3::new(20.0, 0.0, z), Vec3::new(30.0, 0.0, z)];
        assert!(field.hits(&leg(10.0 + 0.6)));
        assert!(!field.hits(&leg(10.0 + 0.8)));
    }

    #[test]
    fn a_sync_says_which_of_the_teams_traps_went_off() {
        let at = Vec3::new(30.0, 0.0, 10.0);
        let mut list = TrapList::default();
        list.promise(3, at, 100_000);
        assert!(
            list.sync(103_000, &[world_trap(at, TrapState::Armed, 100_300)])
                .is_empty()
        );
        assert_eq!(
            list.sync(110_000, &[world_trap(at, TrapState::Fired, 100_300)]),
            vec![at]
        );
        // Once: the next world still showing it fired says nothing.
        assert!(
            list.sync(112_000, &[world_trap(at, TrapState::Fired, 100_300)])
                .is_empty()
        );
        // A promise first shown already fired is a trap that went off.
        let mut list = TrapList::default();
        list.promise(3, at, 100_000);
        assert_eq!(
            list.sync(105_000, &[world_trap(at, TrapState::Fired, 100_200)])
                .len(),
            1
        );
    }

    #[test]
    fn an_empty_field_hits_nothing_and_opens_everything() {
        let field = TrapField::default();
        assert!(field.is_empty() && field.open(7));
        assert!(!field.hits(&[Vec3::ZERO, Vec3::new(5.0, 0.0, 0.0)]));
    }
}

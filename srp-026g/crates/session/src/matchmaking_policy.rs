//! Where a late joiner of a match being played goes ([`place_joiner`]; the
//! owner's of 2026-11-05, after a match a late joiner made 15 apart): the
//! side that counts for less, or none (it waits). A room's match
//! (`lobby-server`'s `rooms`) takes a member who comes while it is played
//! so, until [`LOAD_MARGIN`] is left of it (the owner's of 2026-10-17: the
//! queue's late joins, their scoring across matches and their patience went
//! with the queue).

use std::time::Duration;

use crate::Strength;

/// A match takes late joiners until this is left of it (the owner's of
/// 2026-10-19); one who comes in its last seconds waits in the room for the
/// next. A joiner still loading when the match ends is safe: its loading
/// screen has set the map name the results screen reads (`results.md` §6,
/// hazard 10, is a game that never loaded a match).
pub const LOAD_MARGIN: Duration = Duration::from_secs(30);

/// Why a match being played takes no late joiner now, as the match server
/// knows it (`match.md` §32, §33). A room's waiting member is told it
/// (`rooms.md`, late joins).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Closed {
    /// A joiner is on its way in: its place is reserved and not entered
    /// yet, or a game of the match is still installing it. One at a time.
    Joining,
    /// A game of the match is loading (at its start, or one that came
    /// again), for 30 s at most (the match server's `LOADING_GRACE`), or
    /// the match has not begun its countdown.
    Loading,
    /// [`LOAD_MARGIN`] or less is left of it.
    LastSeconds,
    /// It is ending: its mode's rule has its end, or it has its result.
    Ending,
    /// Every roster place of it is taken.
    Full,
}

/// A side of a match being played, or a joiner: its players and what they
/// count for together beside being there (their strengths,
/// `crate::Strength`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Worth {
    pub players: usize,
    pub strength: Strength,
}

/// How a player is put into a match being played ([`place_joiner`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LateRule {
    /// The most players of a side.
    pub capacity: usize,
    /// The roster places the match has left.
    pub room: usize,
    /// What a player counts for by being there.
    pub player_value: Strength,
    /// What each side's lead counts for; nothing for the side that trails.
    pub lead: [Strength; 2],
    /// Each side's places whose players are not in the match now but keep
    /// them (a game that broke: Play takes it back there). They count
    /// against `capacity`, not as players of the side: a joiner never
    /// fills a side one of them comes back to.
    pub kept: [usize; 2],
}

/// Who goes into a match being played, and on which side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LatePlacement {
    /// Joiner index and team.
    pub assignments: Vec<(usize, u8)>,
    /// The sides with them.
    pub sides: [Worth; 2],
    /// What the sides count for apart, side 0 less side 1, before and with
    /// them: players, strengths and the lead together (for the log).
    pub apart_before: Strength,
    pub apart_after: Strength,
}

impl LatePlacement {
    /// The side it goes to.
    pub fn side(&self) -> u8 {
        self.assignments.first().map_or(0, |&(_, team)| team)
    }
}

/// Where a match being played, whose sides are `sides` now, takes `joiner`
/// (the owner's of 2026-11-05: "late joiners can't skew balance"): the side
/// that counts for less (its players, their strengths and its lead; of two
/// alike, side 0), while it has a place for all of the joiner; else none,
/// and the joiner waits until it has. Never the other side: one that went
/// to the side with fewer players, or back to the side it had played on,
/// made a match 15 apart (2026-11-05).
pub fn place_joiner(sides: [Worth; 2], joiner: Worth, rule: &LateRule) -> Option<LatePlacement> {
    let worth = |side: usize| {
        rule.player_value.times(sides[side].players) + sides[side].strength + rule.lead[side]
    };
    placed(sides, joiner, usize::from(worth(1) < worth(0)), rule)
}

/// Where a match being played takes `joiner` that has a side of its own
/// (a private room's member, on the team its owner put it on; the owner's
/// of 2026-10-26: "Start plays exactly that"): `side` while it has a place,
/// else the other one. The match's `room` stops it as in [`place_joiner`].
pub fn place_joiner_on(
    sides: [Worth; 2],
    joiner: Worth,
    side: u8,
    rule: &LateRule,
) -> Option<LatePlacement> {
    let side = usize::from(side.min(1));
    placed(sides, joiner, side, rule).or_else(|| placed(sides, joiner, 1 - side, rule))
}

/// `joiner` on `side`, unless that side is full with it or the match has
/// no place.
fn placed(sides: [Worth; 2], joiner: Worth, side: usize, rule: &LateRule) -> Option<LatePlacement> {
    let worth = |sides: &[Worth; 2], side: usize| {
        rule.player_value.times(sides[side].players) + sides[side].strength + rule.lead[side]
    };
    let mut after = sides;
    after[side].players += joiner.players;
    after[side].strength += joiner.strength;
    if joiner.players == 0
        || joiner.players > rule.room
        || after[side].players + rule.kept[side] > rule.capacity
    {
        return None;
    }
    Some(LatePlacement {
        assignments: vec![(0, side as u8)],
        sides: after,
        apart_before: worth(&sides, 0) - worth(&sides, 1),
        apart_after: worth(&after, 0) - worth(&after, 1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(players: usize, strength: i64) -> Worth {
        Worth {
            players,
            strength: Strength::from_milli(strength),
        }
    }

    fn rule() -> LateRule {
        LateRule {
            capacity: 10,
            room: 20,
            player_value: Strength::from_milli(1200),
            lead: [Strength::ZERO; 2],
            kept: [0, 0],
        }
    }

    /// A place kept for a player who may come back counts against its
    /// side's size: a full side by them takes nobody, and a squad needs
    /// both its places there.
    #[test]
    fn places_kept_for_players_who_may_come_back_count_against_a_side() {
        let one = side(1, 0);
        let pair = side(2, 0);
        let kept = LateRule {
            kept: [4, 0],
            ..rule()
        };
        // 6 in side 0, the weaker, and 4 kept for it: full, so it waits.
        assert!(place_joiner([side(6, 0), side(8, 0)], one, &kept).is_none());
        // A pair: one place on the weaker side is not two.
        let tight = LateRule {
            kept: [3, 0],
            ..rule()
        };
        assert!(place_joiner([side(6, 0), side(9, 0)], pair, &tight).is_none());
        assert!(place_joiner([side(6, 0), side(9, 0)], one, &tight).is_some());
        // Without kept places a pair takes the weaker side, with two.
        let placed = place_joiner([side(6, 0), side(9, 0)], pair, &rule()).unwrap();
        assert_eq!(placed.side(), 0);
    }

    /// A private room's member goes to its own team, fewer or not, while it
    /// has a place; a full one sends it to the other.
    #[test]
    fn a_joiner_with_its_own_side_goes_there_while_it_has_a_place() {
        let one = side(1, 0);
        let placed = place_joiner_on([side(1, 0), side(3, 0)], one, 1, &rule()).unwrap();
        assert_eq!(placed.assignments, [(0, 1)]);
        assert_eq!(placed.sides[1].players, 4);
        let placed = place_joiner_on([side(2, 0), side(10, 0)], one, 1, &rule()).unwrap();
        assert_eq!(placed.assignments, [(0, 0)]);
        assert!(place_joiner_on([side(10, 0), side(10, 0)], one, 0, &rule()).is_none());
    }

    /// The owner's of 2026-11-05: the weaker side, whatever the players
    /// apart; the stronger side never.
    #[test]
    fn a_joiner_goes_to_the_weaker_side_only() {
        let one = side(1, 300);
        // Four against three, the three far stronger and leading: the
        // four get the joiner (4.8 - 3 against 3.6 + 3 + 5).
        let led = LateRule {
            lead: [Strength::ZERO, Strength::from_milli(5000)],
            ..rule()
        };
        let placed = place_joiner([side(4, -3000), side(3, 3000)], one, &led).unwrap();
        assert_eq!(placed.side(), 0);
        assert_eq!(placed.sides, [side(5, -2700), side(3, 3000)]);
        assert_eq!(
            placed.apart_before,
            Strength::from_milli(4800 - 3000 - 3600 - 3000 - 5000)
        );
        assert_eq!(
            placed.apart_after,
            Strength::from_milli(6000 - 2700 - 3600 - 3000 - 5000)
        );
        // Three against three, side 1 weaker by value: there.
        let placed = place_joiner([side(3, 1000), side(3, 400)], one, &rule()).unwrap();
        assert_eq!(placed.side(), 1);
        // The lead counts: side 0 leads by more than side 1 is stronger.
        let led = LateRule {
            lead: [Strength::from_milli(1000), Strength::ZERO],
            ..rule()
        };
        let placed = place_joiner([side(3, 0), side(3, 500)], one, &led).unwrap();
        assert_eq!(placed.side(), 1);
        // Alike: side 0. An empty match: side 0.
        let placed = place_joiner([side(3, 0), side(3, 0)], one, &rule()).unwrap();
        assert_eq!(placed.side(), 0);
        let placed = place_joiner([side(0, 0), side(0, 0)], one, &rule()).unwrap();
        assert_eq!(placed.sides, [side(1, 300), side(0, 0)]);
    }

    #[test]
    fn a_full_weaker_side_or_no_room_takes_nobody() {
        let one = side(1, 0);
        let small = LateRule {
            capacity: 3,
            ..rule()
        };
        // The weaker side goes to three, its capacity.
        let placed = place_joiner([side(2, 0), side(3, 0)], one, &small).unwrap();
        assert_eq!(placed.side(), 0);
        // The weaker side full, the other not: nobody (it waits).
        assert_eq!(
            place_joiner([side(3, -9000), side(2, 0)], one, &small),
            None
        );
        // No roster place left.
        let no_room = LateRule { room: 0, ..rule() };
        assert_eq!(place_joiner([side(3, 0), side(2, 0)], one, &no_room), None);
        // Nobody to place.
        assert_eq!(
            place_joiner([side(3, 0), side(2, 0)], Worth::default(), &rule()),
            None
        );
    }
}

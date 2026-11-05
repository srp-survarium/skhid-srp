//! Where a late joiner of a match being played goes ([`place_joiner`]; the
//! owner's of 2026-10-19, after a 4 v 8): the side with fewer players; of
//! two alike in that, the side it played on in that match before; then the
//! one that counts for less; then side 0. A room's match (`lobby-server`'s
//! `rooms`) takes a member who comes while it is played so, until [`LOAD_MARGIN`]
//! is left of it (the owner's of 2026-10-17: the queue's late joins, their
//! scoring across matches and their patience went with the queue).

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
/// count for together beside being there (their K/Ds, `crate::Strength`).
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
    /// them: players, strengths and the lead together (for the log; the
    /// side is chosen by players first, [`place_joiner`]).
    pub apart_before: Strength,
    pub apart_after: Strength,
}

/// Where a match being played, whose sides are `sides` now, takes `joiner`
/// (the owner's of 2026-10-19: players, then the previous team, then the
/// balance):
/// 1. the side with fewer players in the match (a room's match went to
///    4 v 8, its four strong and leading);
/// 2. of two alike in that, `previous`: the side the joiner played on in
///    this match before (it left, or its game was gone for good);
/// 3. then the one that counts for less (players, strengths and the lead);
/// 4. then side 0.
///
/// Only that side being full (`capacity`) with it, and the match's `room`,
/// stop it.
pub fn place_joiner(
    sides: [Worth; 2],
    joiner: Worth,
    previous: Option<u8>,
    rule: &LateRule,
) -> Option<LatePlacement> {
    let worth = |sides: &[Worth; 2], side: usize| {
        rule.player_value.times(sides[side].players) + sides[side].strength + rule.lead[side]
    };
    let key = |side: usize| {
        (
            sides[side].players,
            previous != Some(side as u8),
            worth(&sides, side),
        )
    };
    let side = usize::from(key(1) < key(0));
    // That side full with the places kept on it: the other one.
    placed(sides, joiner, side, rule).or_else(|| placed(sides, joiner, 1 - side, rule))
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

/// Where a match being played takes `joiner` beside its squad mate (the
/// owner's of 2026-10-31: a squad plays on one side): `side` while it has
/// a place, else nowhere (it waits for the next match).
pub fn place_joiner_beside(
    sides: [Worth; 2],
    joiner: Worth,
    side: u8,
    rule: &LateRule,
) -> Option<LatePlacement> {
    placed(sides, joiner, usize::from(side.min(1)), rule)
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
    /// side's size: a full side by them sends the joiner to the other one,
    /// and a squad needs both its places on one side.
    #[test]
    fn places_kept_for_players_who_may_come_back_count_against_a_side() {
        let one = side(1, 0);
        let pair = side(2, 0);
        let kept = LateRule {
            kept: [4, 0],
            ..rule()
        };
        // 6 in side 0 and 4 kept for it: full. 8 in side 1: there.
        let placed = place_joiner([side(6, 0), side(8, 0)], one, None, &kept).unwrap();
        assert_eq!(placed.assignments, [(0, 1)]);
        // A pair: one place on each side is not two on one.
        let tight = LateRule {
            kept: [3, 0],
            ..rule()
        };
        assert!(place_joiner([side(6, 0), side(9, 0)], pair, None, &tight).is_none());
        assert!(place_joiner_beside([side(6, 0), side(9, 0)], one, 0, &tight).is_some());
        assert!(place_joiner_beside([side(6, 0), side(9, 0)], pair, 0, &tight).is_none());
        // Without kept places a pair takes the side with two.
        let placed = place_joiner([side(6, 0), side(9, 0)], pair, None, &rule()).unwrap();
        assert_eq!(placed.assignments, [(0, 0)]);
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

    #[test]
    fn a_joiner_goes_to_the_side_with_fewer_players_however_strong() {
        let one = side(1, 300);
        // Four against three, the three far stronger and leading: the three
        // get the joiner (live, 2026-10-19: a match went to 4 v 8 by value).
        let led = LateRule {
            lead: [Strength::ZERO, Strength::from_milli(5000)],
            ..rule()
        };
        let placed = place_joiner([side(4, -3000), side(3, 3000)], one, None, &led).unwrap();
        assert_eq!(placed.assignments, [(0, 1)]);
        assert_eq!(placed.sides, [side(4, -3000), side(4, 3300)]);
        assert_eq!(
            placed.apart_before,
            Strength::from_milli(1200 - 3000 - 3000 - 5000)
        );
        assert_eq!(
            placed.apart_after,
            Strength::from_milli(-3000 - 3300 - 5000)
        );
        let placed = place_joiner([side(3, 3000), side(4, -3000)], one, None, &rule()).unwrap();
        assert_eq!(placed.assignments, [(0, 0)]);
    }

    #[test]
    fn of_sides_alike_in_players_the_weaker_then_side_0() {
        let one = side(1, 0);
        // Three against three, side 1 weaker by value: there.
        let placed = place_joiner([side(3, 1000), side(3, 400)], one, None, &rule()).unwrap();
        assert_eq!(placed.assignments, [(0, 1)]);
        assert_eq!(placed.sides, [side(3, 1000), side(4, 400)]);
        // The lead counts: side 0 leads by more than side 1 is stronger.
        let led = LateRule {
            lead: [Strength::from_milli(1000), Strength::ZERO],
            ..rule()
        };
        let placed = place_joiner([side(3, 0), side(3, 500)], one, None, &led).unwrap();
        assert_eq!(placed.assignments, [(0, 1)]);
        // Alike in both: side 0.
        let placed = place_joiner([side(3, 0), side(3, 0)], one, None, &rule()).unwrap();
        assert_eq!(placed.assignments, [(0, 0)]);
        // An empty match: side 0.
        let placed = place_joiner([side(0, 0), side(0, 0)], one, None, &rule()).unwrap();
        assert_eq!(placed.sides, [side(1, 0), side(0, 0)]);
    }

    #[test]
    fn one_who_played_the_match_goes_back_to_its_side_when_the_players_are_alike() {
        let one = side(1, 0);
        // Alike in players: its side, though the other counts for less.
        let placed = place_joiner([side(3, 0), side(3, 2000)], one, Some(1), &rule()).unwrap();
        assert_eq!(placed.assignments, [(0, 1)]);
        let placed = place_joiner([side(3, 2000), side(3, 0)], one, Some(0), &rule()).unwrap();
        assert_eq!(placed.assignments, [(0, 0)]);
        // Fewer players come first: its side has more, so the other.
        let placed = place_joiner([side(3, 0), side(4, -2000)], one, Some(1), &rule()).unwrap();
        assert_eq!(placed.assignments, [(0, 0)]);
        // No previous side: the balance.
        let placed = place_joiner([side(3, 0), side(3, 2000)], one, None, &rule()).unwrap();
        assert_eq!(placed.assignments, [(0, 0)]);
    }

    #[test]
    fn a_full_side_or_no_room_takes_nobody() {
        let one = side(1, 0);
        let small = LateRule {
            capacity: 3,
            ..rule()
        };
        // The side with fewer goes to three, its capacity.
        let placed = place_joiner([side(2, 2000), side(3, 0)], one, None, &small).unwrap();
        assert_eq!(placed.assignments, [(0, 0)]);
        // Both full: nobody.
        assert_eq!(
            place_joiner([side(3, 0), side(3, 0)], one, None, &small),
            None
        );
        // No roster place left.
        let no_room = LateRule { room: 0, ..rule() };
        assert_eq!(
            place_joiner([side(3, 0), side(2, 0)], one, None, &no_room),
            None
        );
        // Nobody to place.
        assert_eq!(
            place_joiner([side(3, 0), side(2, 0)], Worth::default(), None, &rule()),
            None
        );
    }
}

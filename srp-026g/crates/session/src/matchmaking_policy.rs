//! Where a late joiner of a match being played goes ([`place_joiner`]; the
//! owner's of 2026-10-19, after a 4 v 8): the side with fewer players; of
//! two alike in that, the side it played on in that match before; then the
//! one that counts for less; then side 0. A room's match (`lobby-server`'s
//! `rooms`) takes a member who comes while it is played so, until [`LOAD_MARGIN`]
//! is left of it (the owner's of 2026-10-17: the queue's late joins, their
//! scoring across matches and their patience went with the queue).
//!
//! A room's server bots (the owner's of 2026-10-26: "up to 3 vs 3 bots,
//! which will be replaced with real players once those join"): the bots a
//! room's match starts with ([`start_bots`]), a joiner taking a bot's place
//! ([`place_joiner`] on a side with one), and what the match server does
//! with a team's bots as players come and go ([`bot_move`]). A place of a
//! match is never used twice, and every place counts toward its 20: a
//! joiner comes on a new place, the bot's own is left. So the match keeps
//! a place for each bot in it, for the player who takes that bot's place.

use std::time::Duration;

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
/// count for together beside being there (their strengths, in
/// `crate::matchmaking::STRENGTH_SCALE`s).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Worth {
    /// Everyone in it, the server's bots too: what the clients see.
    pub players: usize,
    pub strength: i64,
    /// Of [`Self::players`], the server's bots (a room's, `start_bots`):
    /// a joiner on the side takes one's place. A joiner has none.
    pub bots: usize,
}

/// How a player is put into a match being played ([`place_joiner`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LateRule {
    /// The most players of a side.
    pub capacity: usize,
    /// The roster places the match has left.
    pub room: usize,
    /// What a player counts for by being there.
    pub player_value: i64,
    /// What each side's lead counts for; nothing for the side that trails.
    pub lead: [i64; 2],
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
    pub apart_before: i64,
    pub apart_after: i64,
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
        rule.player_value * sides[side].players as i64 + sides[side].strength + rule.lead[side]
    };
    let key = |side: usize| {
        (
            sides[side].players,
            previous != Some(side as u8),
            worth(&sides, side),
        )
    };
    let side = usize::from(key(1) < key(0));
    // A side that cannot take it: the other in a bot's place, if it has
    // one (a player never waits while a bot holds a place).
    placed(sides, joiner, side, rule).or_else(|| {
        (sides[1 - side].bots > 0)
            .then(|| placed(sides, joiner, 1 - side, rule))
            .flatten()
    })
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
/// no place. On a side with the server's bots in the match, each player of
/// it takes a bot's place: the side keeps its size, and the place the
/// match kept for that bot is the joiner's. Elsewhere it takes a place the
/// match does not keep for a bot.
fn placed(sides: [Worth; 2], joiner: Worth, side: usize, rule: &LateRule) -> Option<LatePlacement> {
    let worth = |sides: &[Worth; 2], side: usize| {
        rule.player_value * sides[side].players as i64 + sides[side].strength + rule.lead[side]
    };
    let gives_way = sides[side].bots.min(joiner.players);
    // The places the match keeps for the bots that stay.
    let kept = if gives_way == joiner.players {
        0
    } else {
        sides[0].bots + sides[1].bots - gives_way
    };
    let mut after = sides;
    after[side].players += joiner.players - gives_way;
    after[side].bots -= gives_way;
    after[side].strength += joiner.strength;
    if joiner.players == 0
        || joiner.players + kept > rule.room
        || after[side].players > rule.capacity
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

/// The server's bots a room's match starts with on each team, its players
/// being `players` (the owner's of 2026-10-26: bots fill only places
/// players do not have): `bots` a team where its players leave that many
/// of the team's `size` places, fewer where they do not. The match keeps a
/// place for each bot, for the player who comes to take its place
/// ([`placed`]): `players + 2 × bots` is at most the 20 a match has, the
/// team with more bots giving one up first (team 1 of two alike).
pub fn start_bots(bots: usize, size: usize, players: [usize; 2]) -> [usize; 2] {
    let mut each = players.map(|team| bots.min(size.saturating_sub(team)));
    let seated = players[0] + players[1];
    while seated + 2 * (each[0] + each[1]) > crate::MAX_PLAYERS_PER_MATCH {
        let team = usize::from(each[1] >= each[0]);
        each[team] -= 1;
    }
    each
}

/// A team of a room's match as the match server has it now, for its bots
/// ([`bot_move`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TeamBots {
    /// The team's places when the match was made, its players' and its
    /// bots': the size its bots keep it at.
    pub fill: usize,
    /// Its bots' places (each bot has one place for the whole match: it
    /// leaves and comes back on it).
    pub places: usize,
    /// Its players in the match now (entered, not left).
    pub players: usize,
    /// Its bots in the match now.
    pub bots: usize,
}

/// What a team's bots do now ([`bot_move`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotMove {
    Stay,
    /// One leaves: a player came on the team, which is over its fill.
    GiveWay,
    /// One comes back: a player left the team, which is under its fill.
    ComeBack,
}

/// What a room's match does with `team`'s bots now, the match having
/// `places_left` places it has not used and `bots_in_match` bots in it:
/// the team keeps its fill with them (the owner's of 2026-10-26: "replaced
/// with real players once those join"; and a bot comes back when a player
/// leaves, the room keeping its teams filled up to its setting). A player
/// more than the fill takes a bot's place; a player fewer brings one of
/// the team's bots back, while the match keeps a place for it (a bot
/// comes back only with a place left beside the one of every bot in it).
pub fn bot_move(team: TeamBots, places_left: usize, bots_in_match: usize) -> BotMove {
    let wanted = team.places.min(team.fill.saturating_sub(team.players));
    if team.bots > wanted {
        BotMove::GiveWay
    } else if team.bots < wanted && bots_in_match < places_left {
        BotMove::ComeBack
    } else {
        BotMove::Stay
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(players: usize, strength: i64) -> Worth {
        Worth {
            players,
            strength,
            bots: 0,
        }
    }

    /// A side of `players` in the match, `bots` of them the server's.
    fn with_bots(players: usize, bots: usize) -> Worth {
        Worth {
            players,
            strength: 0,
            bots,
        }
    }

    /// A room of 3 a team: each team's players leave places for its bots,
    /// and the match keeps a place for each bot beside them.
    #[test]
    fn a_rooms_bots_fill_the_places_its_players_leave_and_keep_a_place_each() {
        // 1 v 1 in an 8 v 8: three bots a side.
        assert_eq!(start_bots(3, 8, [1, 1]), [3, 3]);
        // A team nearly full of players takes fewer.
        assert_eq!(start_bots(3, 8, [7, 2]), [1, 3]);
        assert_eq!(start_bots(3, 8, [8, 8]), [0, 0]);
        assert_eq!(start_bots(0, 8, [1, 1]), [0, 0]);
        // A private room's 1 v 10 (its size the 10 a team).
        assert_eq!(start_bots(3, 10, [1, 10]), [3, 0]);
        // 6 v 6 players and 3 a team would leave the match 2 places for 6
        // bots: 4 bots, 2 a team, each keeping a place (12 + 4 + 4 = 20).
        assert_eq!(start_bots(3, 8, [6, 6]), [2, 2]);
        // The team with more bots gives one up first.
        assert_eq!(start_bots(3, 10, [9, 6]), [1, 1]);
        for players in [[0, 0], [1, 0], [5, 5], [9, 10], [3, 7]] {
            for bots in 0..=3 {
                let each = start_bots(bots, 10, players);
                let seated = players[0] + players[1];
                assert!(seated + 2 * (each[0] + each[1]) <= 20, "{players:?} {bots}");
                assert!(each.iter().all(|&team| team <= bots));
            }
        }
    }

    /// A joiner on a side with a bot takes its place (the side keeps its
    /// size, also at its capacity) on the place the match kept for it;
    /// elsewhere it needs a place the match does not keep for a bot.
    #[test]
    fn a_joiner_takes_a_bots_place_and_the_kept_places_are_theirs() {
        let one = side(1, 0);
        let room = |room| LateRule {
            capacity: 8,
            room,
            ..rule()
        };
        // 1 + 3 bots against 1 + 3: alike, side 0; a bot of it gives way.
        let placed = place_joiner([with_bots(4, 3), with_bots(4, 3)], one, None, &room(12)).unwrap();
        assert_eq!(placed.assignments, [(0, 0)]);
        assert_eq!(placed.sides[0], with_bots(4, 2));
        // A side full of players and bots still takes one in a bot's place.
        let placed = place_joiner([with_bots(8, 1), side(8, 0)], one, None, &room(2)).unwrap();
        assert_eq!(placed.sides[0], with_bots(8, 0));
        // A side with no bot: not into the places kept for the bots; the
        // joiner takes a bot's place on the other side instead.
        let placed = place_joiner([side(2, 0), with_bots(4, 3)], one, None, &room(3)).unwrap();
        assert_eq!(placed.assignments, [(0, 1)]);
        assert_eq!(placed.sides, [side(2, 0), with_bots(4, 2)]);
        let placed = place_joiner([side(2, 0), with_bots(4, 3)], one, None, &room(4)).unwrap();
        assert_eq!(placed.sides[0], side(3, 0));
        // No bot anywhere and no place: nobody, as before.
        assert!(place_joiner([side(2, 0), side(3, 0)], one, None, &room(0)).is_none());
        // Its own side has none, the other has: the other's capacity counts.
        let placed = place_joiner_on([side(8, 0), with_bots(8, 2)], one, 0, &room(4)).unwrap();
        assert_eq!(placed.assignments, [(0, 1)]);
    }

    /// The team keeps its fill: a player over it takes a bot's place, a
    /// player under it brings a bot back while the match has a place for
    /// it beside the bots' kept ones.
    #[test]
    fn a_teams_bots_give_way_to_players_and_come_back_for_them() {
        let team = |players, bots| TeamBots {
            fill: 4,
            places: 3,
            players,
            bots,
        };
        assert_eq!(bot_move(team(1, 3), 12, 6), BotMove::Stay);
        assert_eq!(bot_move(team(2, 3), 11, 6), BotMove::GiveWay);
        assert_eq!(bot_move(team(4, 0), 8, 2), BotMove::Stay);
        assert_eq!(bot_move(team(5, 0), 8, 2), BotMove::Stay);
        // A player left: a bot back, if a place is left beside the bots'.
        assert_eq!(bot_move(team(1, 2), 9, 5), BotMove::ComeBack);
        assert_eq!(bot_move(team(1, 2), 5, 5), BotMove::Stay);
        // Never more than its own places, whoever left.
        assert_eq!(bot_move(team(0, 3), 12, 6), BotMove::Stay);
        // A team that started full of players has no bot to bring.
        let full = TeamBots {
            fill: 8,
            places: 0,
            players: 5,
            bots: 0,
        };
        assert_eq!(bot_move(full, 12, 0), BotMove::Stay);
    }

    fn rule() -> LateRule {
        LateRule {
            capacity: 10,
            room: 20,
            player_value: 1200,
            lead: [0, 0],
        }
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
            lead: [0, 5000],
            ..rule()
        };
        let placed = place_joiner([side(4, -3000), side(3, 3000)], one, None, &led).unwrap();
        assert_eq!(placed.assignments, [(0, 1)]);
        assert_eq!(placed.sides, [side(4, -3000), side(4, 3300)]);
        assert_eq!(placed.apart_before, 1200 - 3000 - 3000 - 5000);
        assert_eq!(placed.apart_after, -3000 - 3300 - 5000);
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
            lead: [1000, 0],
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

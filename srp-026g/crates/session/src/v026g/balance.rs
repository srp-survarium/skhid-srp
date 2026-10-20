//! What a player counts for when the sides of a match are made, and the
//! balance rating that is a part of it (`docs/notes/matchmaking.md` §8).
//!
//! All of it is srp's own [choice]: the client knows nothing of how the
//! original server made its sides. The numbers are fitted to the players'
//! server's matches of 2026-10-12 and 13 (88 with a winner), which is
//! little: they are settings ([`Weights`]) to be fitted again.
//!
//! - **A side counts** [`Weights::player`] for each of its players, and
//!   each player's [`Weights::strength`]: its gear level, its own level and
//!   its balance rating as far as that is trusted. A room makes its two
//!   sides count alike (`crate::v026g::rooms::sides`).
//! - **The balance rating** ([`Balance`]) is not the ranking's
//!   (`Account::rating`), which the client shows and which every player of
//!   a side moves by alike whatever the sides' sizes. This one is kept for
//!   the sides only. It starts at 0 (the average), a win raises it and a
//!   loss lowers it ([`changes`]), by more the less the result was to be
//!   expected: the side with a player more is expected to win
//!   ([`PLAYER`]), so winning with it moves little.
//! - **It is trusted by its matches** ([`Weights::trust`]): a rating of no
//!   match counts for nothing, of [`Weights::trust_matches`] for half.
//! - **A match of few players moves none** ([`RATED_PLAYERS`],
//!   [`players_in`]): one against one and two against one say nothing of
//!   how sides should be made.
//! - **A match that is being played** counts a third thing for the side
//!   that leads: its lead ([`Weights::lead`]), a battery or a ticket being
//!   a part of a player. A player who joins such a match goes to the side
//!   with fewer players; of two alike in that, to the side it played on in
//!   that match before, then to the one that counts for less
//!   (`crate::matchmaking_policy::place_joiner`).
//!   [`Weights::expected`] is the chance the whole count gives a side; the
//!   lobby writes it in its log each minute, to be fitted again.

use survarium::v026g::maps::mode;
use survarium::v026g::match_msgs::rules::DRAW;

use crate::matchmaking::STRENGTH_SCALE;

/// A whole point of the balance rating, as it is kept: thousandths.
pub const UNIT: i32 = 1000;

/// What one player more is worth to a side, in rating points: the odds of
/// the side with a player more were about nine to one (it won 35 of 38
/// matches; the fit's 2.2 to 2.3).
pub const PLAYER: f64 = 2.2;

/// How far a match moves the ratings of a side, all its players together,
/// when the result was a toss-up and went one way: half of this. Shared
/// among the players of the match, so a match of seven a side does not
/// move its sides seven times as far as a duel does (a step for each
/// player called a seven against seven at 99 to 1, wrongly).
pub const STEP: f64 = 0.8;

/// A new player's rating moves this many times further, on top of
/// [`STEP`], and less with each match: `STEP * (1 + NEW_PLAYER / (1 +
/// matches))`. Its first match moves it four times as far as its
/// hundredth.
pub const NEW_PLAYER: f64 = 3.0;

/// The fewest players a match must have had in it to move the balance
/// rating ([`players_in`]): one against one and two against one move none.
/// The owner's choice (2026-10-14), for three reasons. Two players can be
/// split one way only, so such a match says nothing of how sides should be
/// made. It is the easiest to play by agreement. And the shared step moves
/// each of its ratings the furthest: a duel seven times as far as a match
/// of fourteen (live, 2026-10-14: 0.75 and 0.81 of a point in one match,
/// against 0.09 in a match of seven).
pub const RATED_PLAYERS: u32 = 4;

/// The part of a match a player must have been in to be one of its
/// players for [`RATED_PLAYERS`]: half of the longest stay. So a match
/// that began one against one and grew is as large as it was for most of
/// its time.
pub const RATED_SHARE: f64 = 0.5;

/// `SRP_BALANCE_RATED_PLAYERS`: [`RATED_PLAYERS`] for this server, 1 to 20
/// (default 4). 1: every match moves the ratings, as before 2026-10-14.
/// Read once.
pub fn rated_players() -> u32 {
    static LEAST: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *LEAST.get_or_init(|| match std::env::var("SRP_BALANCE_RATED_PLAYERS") {
        Ok(value) => match value.trim().parse::<u32>() {
            Ok(least) if (1..=20).contains(&least) => least,
            _ => {
                log::error!(
                    "SRP_BALANCE_RATED_PLAYERS is `{value}`; it must be 1 to 20. Using {RATED_PLAYERS}"
                );
                RATED_PLAYERS
            }
        },
        Err(_) => RATED_PLAYERS,
    })
}

/// How many players a match had, for [`RATED_PLAYERS`]: those of
/// `players` (team, rating, share of the match as in [`changes_by_share`])
/// who were in it for [`RATED_SHARE`] of it or more. A player of no team
/// is not counted.
pub fn players_in(players: &[(u8, Balance, f64)]) -> u32 {
    players
        .iter()
        .filter(|player| player.0 < 2 && player.2.is_finite() && player.2 >= RATED_SHARE)
        .count() as u32
}

/// An account's balance rating: thousandths of a point ([`UNIT`]), 0 the
/// average, and the matches that moved it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Balance {
    pub rating: i32,
    pub matches: u32,
}

impl Balance {
    /// The rating moved by `change`, with one match more.
    pub fn moved(self, change: i32) -> Self {
        Self {
            rating: self.rating.saturating_add(change),
            matches: self.matches.saturating_add(1),
        }
    }
}

/// How a match moves each balance rating, in thousandths: `players` are
/// (team, rating) of those who were in the match, `winner` the winning
/// team (2 a draw). A match with one side, or with no known winner, moves
/// nothing; a player of no team is not moved.
pub fn changes(players: &[(u8, Balance)], winner: u8) -> Vec<i32> {
    let whole: Vec<(u8, Balance, f64)> = players
        .iter()
        .map(|&(team, balance)| (team, balance, 1.0))
        .collect();
    changes_by_share(&whole, winner)
}

/// [`changes`] of a match whose players were not all in it for all of it:
/// the third of each is the share of the match it was in, 0 to 1. A side
/// is what its players were for their shares (a player who came at half
/// time is half a player and half its rating), and a player is moved by
/// its share of what a whole match would move it. So a side that got a
/// player late and won is not taken for the side that won alone, and the
/// player who came into a lost match does not carry the loss.
pub fn changes_by_share(players: &[(u8, Balance, f64)], winner: u8) -> Vec<i32> {
    let share = |player: &(u8, Balance, f64)| {
        if player.2.is_finite() {
            player.2.clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    let count = |team: u8| -> f64 {
        players
            .iter()
            .filter(|player| player.0 == team)
            .map(share)
            .sum()
    };
    let (first, second) = (count(0), count(1));
    if first <= 0.0 || second <= 0.0 || winner > DRAW {
        return vec![0; players.len()];
    }
    let side = |team: u8| if team == 0 { 1.0 } else { -1.0 };
    let apart: f64 = players
        .iter()
        .filter(|player| player.0 < 2)
        .map(|player| side(player.0) * share(player) * f64::from(player.1.rating) / f64::from(UNIT))
        .sum::<f64>()
        + PLAYER * (first - second);
    let expected = 1.0 / (1.0 + (-apart).exp());
    let score = match winner {
        0 => 1.0,
        1 => 0.0,
        _ => 0.5,
    };
    let shared = 2.0 / (first + second);
    players
        .iter()
        .map(|player| {
            let (team, balance, _) = *player;
            if team > 1 {
                return 0;
            }
            let step = STEP * (1.0 + NEW_PLAYER / (1.0 + f64::from(balance.matches))) * shared;
            (step * share(player) * (score - expected) * side(team) * f64::from(UNIT)).round()
                as i32
        })
        .collect()
}

/// What the parts of a player count for when the sides are made, each in
/// gear levels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    /// A player, by being there. With an odd count the side with a player
    /// more has this much more, so the stronger players go to the other.
    /// 0: only the strengths are made even, as before 2026-10-13.
    pub player: f64,
    /// A level of the account (`Account::level`: its skill points).
    pub player_level: f64,
    /// A whole point of the balance rating, fully trusted.
    pub rating: f64,
    /// The matches after which a rating counts for half. 0: it counts in
    /// full from its first match.
    pub trust_matches: f64,
    /// A battery of lead in a match being played (modes 2 and 3), in
    /// players: what the side that leads counts for more.
    pub battery_lead: f64,
    /// The same for a ticket of lead in a Team Deathmatch.
    pub ticket_lead: f64,
}

impl Default for Weights {
    /// The fit of 2026-10-13 (player count, gear level, player level and
    /// the rating, trusted, against who won): a player 2.28, a gear level
    /// 0.15, a player level 0.18, a rating point 0.58. In gear levels that
    /// is 15, 1.2 and 3.9; without the rating the same fit said 12 and
    /// 1.5, and the splits were the same from 8 to 12.
    ///
    /// The lead is the fit of the same day on the worlds saved of 66 of
    /// those matches (who won against the players in the match and the
    /// lead, at five moments of each): a battery 0.57 of a player (51
    /// matches), ten tickets 2.2 players (15 matches, so taken lower). The
    /// side that first led by five batteries won 28 of 30 matches, by
    /// four 35 of 39; no lead of eleven tickets was lost (9).
    fn default() -> Self {
        Self {
            player: 12.0,
            player_level: 1.5,
            rating: 4.0,
            trust_matches: 5.0,
            battery_lead: 0.55,
            ticket_lead: 0.2,
        }
    }
}

impl Weights {
    /// [`Self::player`] as the sides of a match count it.
    pub fn player_value(&self) -> i32 {
        scaled(self.player)
    }

    /// How far a rating of `matches` matches is trusted, 0 to 1.
    pub fn trust(&self, matches: u32) -> f64 {
        let matches = f64::from(matches);
        if matches <= 0.0 {
            0.0
        } else {
            matches / (matches + self.trust_matches.max(0.0))
        }
    }

    /// What a player counts for on top of [`Self::player`] when the sides
    /// of a match are made: its gear level
    /// (`matchmaking::level`'s `min`), its own level and its rating.
    pub fn strength(&self, gear_level: u8, player_level: u8, balance: Balance) -> i32 {
        let rating = f64::from(balance.rating) / f64::from(UNIT);
        scaled(
            f64::from(gear_level)
                + self.player_level * f64::from(player_level)
                + self.rating * self.trust(balance.matches) * rating,
        )
    }

    /// What each side's lead counts for in a match of `game_mode` that
    /// stands at `points` (batteries home, or tickets left): nothing for the
    /// side that trails.
    pub fn lead(&self, game_mode: u8, points: [u16; 2]) -> [i32; 2] {
        let each = if game_mode == mode::TEAM_DEATHMATCH {
            self.ticket_lead
        } else {
            self.battery_lead
        };
        let ahead = |side: usize| {
            let lead = f64::from(points[side].saturating_sub(points[1 - side]));
            scaled(lead * each.max(0.0) * self.player.max(0.0))
        };
        [ahead(0), ahead(1)]
    }

    /// The chance of side 0 when it counts for `apart` more than side 1
    /// (players, strengths and lead together): a
    /// player more is [`PLAYER`] rating points, as in [`changes`].
    pub fn expected(&self, apart: i64) -> f64 {
        let player = f64::from(self.player_value());
        if player <= 0.0 {
            return 0.5;
        }
        1.0 / (1.0 + (-PLAYER * apart as f64 / player).exp())
    }
}

/// Gear levels as whole numbers ([`STRENGTH_SCALE`]).
fn scaled(levels: f64) -> i32 {
    (levels * f64::from(STRENGTH_SCALE))
        .round()
        .clamp(f64::from(i32::MIN / 64), f64::from(i32::MAX / 64)) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new() -> Balance {
        Balance::default()
    }

    fn rated(rating: i32, matches: u32) -> Balance {
        Balance { rating, matches }
    }

    #[test]
    fn equal_sides_move_by_as_much_each_way() {
        // Two new players: a toss-up, the step shared between two players
        // and four times as far for a first match.
        let moved = changes(&[(0, new()), (1, new())], 0);
        assert_eq!(moved, [1600, -1600]);
        assert_eq!(changes(&[(0, new()), (1, new())], 1), [-1600, 1600]);
        assert_eq!(changes(&[(0, new()), (1, new())], DRAW), [0, 0]);
    }

    #[test]
    fn a_match_moves_its_sides_as_far_whatever_their_size() {
        let duel: i32 = changes(&[(0, rated(0, 50)), (1, rated(0, 50))], 0)[0];
        let seven: Vec<(u8, Balance)> = (0..14).map(|i| (i % 2, rated(0, 50))).collect();
        let side: i32 = changes(&seven, 0)
            .iter()
            .zip(&seven)
            .filter(|(_, player)| player.0 == 0)
            .map(|(change, _)| *change)
            .sum();
        assert!((side - duel).abs() <= 7, "{side} against {duel}");
    }

    #[test]
    fn winning_with_a_player_more_moves_little_and_losing_with_it_much() {
        let players = [(0, rated(0, 20)), (0, rated(0, 20)), (1, rated(0, 20))];
        let won = changes(&players, 0);
        let lost = changes(&players, 1);
        assert!(won[0] > 0 && won[2] < 0);
        assert!(lost[0] < 0 && lost[2] > 0);
        assert!(lost[0].abs() > 5 * won[0].abs(), "{won:?} {lost:?}");
    }

    #[test]
    fn a_new_players_rating_moves_further() {
        let moved = changes(
            &[(0, new()), (0, rated(0, 30)), (1, new()), (1, rated(0, 30))],
            0,
        );
        assert!(moved[0] > 3 * moved[1]);
        assert!(moved[2] < 3 * moved[3]);
    }

    #[test]
    fn a_player_who_came_late_counts_for_its_share_of_the_match() {
        // Three against three, and a fourth for the last fifth of it.
        let whole = |team| (team, rated(0, 20), 1.0);
        let mut players = vec![whole(0), whole(0), whole(0), whole(1), whole(1), whole(1)];
        let level = changes_by_share(&players, 0);
        players.push((0, rated(0, 20), 0.2));
        let late = changes_by_share(&players, 0);
        // Its side was the likelier winner by that much: the win moves the
        // others less than the win of an even match, more than a win with
        // a player more from the start.
        let more: Vec<(u8, Balance)> = players.iter().map(|p| (p.0, p.1)).collect();
        let from_the_start = changes(&more, 0);
        assert!(
            late[0] < level[0] && late[0] > from_the_start[0],
            "{late:?}"
        );
        // And itself by a fifth of what it moves them.
        assert!((late[6] * 5 - late[0]).abs() <= 5, "{late:?}");
        // Shares of one are `changes`.
        let whole: Vec<(u8, Balance, f64)> = more.iter().map(|p| (p.0, p.1, 1.0)).collect();
        assert_eq!(changes_by_share(&whole, 0), from_the_start);
        // A player who was not in it is not moved and is nobody's side.
        let absent = [(0, new(), 1.0), (1, new(), 0.0)];
        assert_eq!(changes_by_share(&absent, 0), [0, 0]);
    }

    #[test]
    fn a_match_is_as_large_as_the_players_who_were_in_it_for_half_of_it() {
        let player = |team, share| (team, rated(0, 20), share);
        // One against one, and a third player for the last third of it.
        let duel = [player(0, 1.0), player(1, 1.0), player(0, 0.3)];
        assert_eq!(players_in(&duel), 2);
        // It began one against one and two more came early: four.
        let grown = [
            player(0, 1.0),
            player(1, 1.0),
            player(0, 0.8),
            player(1, 0.5),
        ];
        assert_eq!(players_in(&grown), 4);
        // Nobody's team, or no share to speak of: not a player of it.
        let odd = [
            player(0, 1.0),
            player(2, 1.0),
            player(1, f64::NAN),
            player(1, 0.0),
        ];
        assert_eq!(players_in(&odd), 1);
        assert_eq!(players_in(&[]), 0);
    }

    #[test]
    fn a_lead_counts_for_the_side_that_has_it() {
        let weights = Weights::default();
        // Batteries: 0.55 of a player each, a player being twelve levels.
        assert_eq!(weights.lead(mode::BATTERIES, [5, 3]), [1320, 0]);
        assert_eq!(weights.lead(mode::TIMED_BATTERIES, [0, 5]), [0, 3300]);
        assert_eq!(weights.lead(mode::BATTERIES, [4, 4]), [0, 0]);
        // Tickets: a fifth of a player.
        assert_eq!(weights.lead(mode::TEAM_DEATHMATCH, [80, 93]), [0, 3120]);
        // With no weight for a player there is no lead to count.
        let none = Weights {
            player: 0.0,
            ..weights
        };
        assert_eq!(none.lead(mode::BATTERIES, [9, 0]), [0, 0]);
        assert_eq!(none.expected(5000), 0.5);
    }

    #[test]
    fn the_chance_of_a_side_follows_what_it_counts_for_more() {
        let weights = Weights::default();
        assert_eq!(weights.expected(0), 0.5);
        // A player more: the nine to one of the rating.
        let player = i64::from(weights.player_value());
        assert!((weights.expected(player) - 0.9).abs() < 0.01);
        assert!((weights.expected(-player) - 0.1).abs() < 0.01);
    }

    #[test]
    fn one_side_or_no_winner_moves_nothing() {
        assert_eq!(changes(&[(0, new()), (0, new())], 0), [0, 0]);
        assert_eq!(changes(&[(0, new()), (1, new())], 3), [0, 0]);
        // A player of no team is not moved and not counted.
        assert_eq!(
            changes(&[(0, new()), (1, new()), (2, new())], 0),
            [1600, -1600, 0]
        );
    }

    #[test]
    fn the_expected_winner_gains_less() {
        let favourite = changes(&[(0, rated(2 * UNIT, 40)), (1, rated(0, 40))], 0);
        let upset = changes(&[(0, rated(2 * UNIT, 40)), (1, rated(0, 40))], 1);
        assert!(favourite[0] > 0 && favourite[0] < upset[1]);
    }

    #[test]
    fn a_rating_is_trusted_by_its_matches() {
        let weights = Weights::default();
        assert_eq!(weights.trust(0), 0.0);
        assert_eq!(weights.trust(5), 0.5);
        assert!(weights.trust(45) >= 0.9);
        // A new account is its gear and its level: the rating counts for
        // nothing, whatever it says.
        assert_eq!(weights.strength(3, 2, rated(5 * UNIT, 0)), 600);
        // After five matches a point counts for half of 4 gear levels.
        assert_eq!(weights.strength(3, 2, rated(UNIT, 5)), 800);
        assert_eq!(weights.strength(3, 2, rated(-UNIT, 5)), 400);
        assert_eq!(weights.player_value(), 1200);
    }

    #[test]
    fn weights_of_zero_leave_the_gear_level() {
        let weights = Weights {
            player: 0.0,
            player_level: 0.0,
            rating: 0.0,
            ..Weights::default()
        };
        assert_eq!(
            weights.strength(7, 9, rated(3 * UNIT, 100)),
            7 * STRENGTH_SCALE
        );
        assert_eq!(weights.player_value(), 0);
        // No trust setting: in full from the first match.
        let at_once = Weights {
            trust_matches: 0.0,
            ..Weights::default()
        };
        assert_eq!(at_once.trust(1), 1.0);
        assert_eq!(at_once.trust(0), 0.0);
    }
}

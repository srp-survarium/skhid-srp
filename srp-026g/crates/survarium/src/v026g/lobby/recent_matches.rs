//! The Watch hub's Recent tab in srp's changed lobby movie: the latest
//! public matches from the match history, on the ranking page's request
//! (`3B`) at a reserved start and its reply (`91`), the side channel of
//! [`super::player_matches`]. Only srp's changed movie asks.
//!
//! - The ask: `3B 00 <RECENT_MATCHES_START + f> <count>`, `f` 0 for all
//!   public matches, 1 for those a friend of the asker played in
//!   ([`RecentFilter`]). Private rooms' matches and matches against bots
//!   are never listed.
//! - The answer, one `91`. Row `i` is at place `RECENT_MATCHES_START + i`,
//!   rating and change 0, and its name is ASCII fields with `;` between them:
//!   - row 0: `M0;<n>;<f>`, `f` the ask's (the movie files the answer by it:
//!     a refresh of one list may come after the other is chosen);
//!   - then `n` rows, newest first:
//!     `M1;<match 8 hex>;<map id>;<mode>;<score a|->;<score b|->;<players>;<length s>;<minutes since end>;<replay>`.
//!     `mode` is the mode's lobby number. The scores are the teams' points,
//!     `-` when no trusted world said them. `players` counts the accounts
//!     the history has of it. `replay` is [`ReplayState`];
//!   - then the asker's friends in the listed matches, a row each
//!     ([`super::push::friend_row`], place 255: the history keeps no roster
//!     place).

use super::player_matches::ReplayState;
use super::push::{Ranked, elo_rows};
use super::{ServerMessage, WireError};

/// The `start` of the ask for all recent matches; `+ 1` for friends'.
pub const RECENT_MATCHES_START: u32 = 2_250_000_000;
/// The most matches one answer lists.
pub const RECENT_MATCHES_ROWS: usize = 20;

/// Which recent matches an ask is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecentFilter {
    /// Every public match.
    All,
    /// Public matches a friend of the asker played in.
    Friends,
}

impl RecentFilter {
    /// The ask at ranking start `start`, if it is one.
    pub fn of_start(start: u32) -> Option<Self> {
        let offset = start.checked_sub(RECENT_MATCHES_START)?;
        [Self::All, Self::Friends]
            .into_iter()
            .find(|filter| filter.offset() == offset)
    }

    /// Its offset from [`RECENT_MATCHES_START`]: the ask's `f`, which the
    /// answer repeats.
    pub fn offset(self) -> u32 {
        match self {
            Self::All => 0,
            Self::Friends => 1,
        }
    }
}

/// One recent match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecentMatchRow {
    /// The match server's id: what "Watch" asks by.
    pub match_id: u32,
    pub map_id: u8,
    /// The mode's lobby number.
    pub mode: u8,
    /// The teams' points; `None` when no trusted world said them.
    pub score: Option<[u16; 2]>,
    /// The accounts the history has of it (at most 20).
    pub players: u8,
    /// Its length, sent in whole seconds rounded up.
    pub length_ms: u32,
    pub minutes_since_end: u32,
    pub replay: ReplayState,
}

/// `91` with recent matches, at most [`RECENT_MATCHES_ROWS`], then the
/// `friends` rows ([`super::push::friend_row`]) that fit in the one `91`
/// (255 rows): a big friends' group in many matches loses its last rows,
/// never the answer. The answer names `filter`, the list it is.
pub fn recent_matches(
    filter: RecentFilter,
    matches: &[RecentMatchRow],
    friends: &[String],
) -> Result<ServerMessage, WireError> {
    let matches = &matches[..matches.len().min(RECENT_MATCHES_ROWS)];
    let mut names = vec![format!("M0;{};{}", matches.len(), filter.offset())];
    names.extend(matches.iter().map(|row| {
        let (a, b) = row.score.map_or_else(
            || ("-".to_owned(), "-".to_owned()),
            |[a, b]| (a.to_string(), b.to_string()),
        );
        format!(
            "M1;{:08x};{};{};{a};{b};{};{};{};{}",
            row.match_id,
            row.map_id,
            row.mode,
            row.players,
            row.length_ms.div_ceil(1000),
            row.minutes_since_end,
            row.replay as u8
        )
    }));
    let room = usize::from(u8::MAX) - names.len();
    names.extend(friends.iter().take(room).cloned());
    let rows: Vec<Ranked> = names
        .into_iter()
        .map(|name| Ranked {
            name,
            rating: 0,
            rating_before: 0,
        })
        .collect();
    let placed: Vec<(u32, &Ranked)> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (RECENT_MATCHES_START + index as u32, row))
        .collect();
    elo_rows(&placed)
}

#[cfg(test)]
mod tests {
    use super::super::limits;
    use super::*;

    #[test]
    fn the_longest_row_fits_and_twenty_matches_at_most_are_listed() {
        let row = RecentMatchRow {
            match_id: u32::MAX,
            map_id: u8::MAX,
            mode: u8::MAX,
            score: Some([u16::MAX; 2]),
            players: u8::MAX,
            length_ms: u32::MAX,
            minutes_since_end: u32::MAX,
            replay: ReplayState::OldVersion,
        };
        let longest = "M1;ffffffff;255;255;65535;65535;255;4294968;4294967295;4";
        assert!(longest.len() <= limits::NAME);
        let message = recent_matches(RecentFilter::All, &[row; 22], &[]).unwrap();
        assert_eq!(message.body()[0], 1 + RECENT_MATCHES_ROWS as u8);
        assert_eq!(
            RecentFilter::of_start(RECENT_MATCHES_START + 1),
            Some(RecentFilter::Friends)
        );
        assert_eq!(RecentFilter::of_start(RECENT_MATCHES_START + 2), None);
    }

    /// Twenty matches of a friends' group of thirteen: more rows than a `91`
    /// holds. The last friends' rows are left out; the answer goes.
    #[test]
    fn friends_rows_past_one_answer_are_left_out() {
        let row = RecentMatchRow {
            match_id: 1,
            map_id: 1,
            mode: 1,
            score: None,
            players: 20,
            length_ms: 1000,
            minutes_since_end: 1,
            replay: ReplayState::Kept,
        };
        let friends = vec!["F1;00000001;255;0;Friend".to_owned(); RECENT_MATCHES_ROWS * 13];
        let message =
            recent_matches(RecentFilter::Friends, &[row; RECENT_MATCHES_ROWS], &friends).unwrap();
        assert_eq!(message.body()[0], u8::MAX);
    }

    /// The head names the list the answer is: an answer to an ask made
    /// before the player chose the other list is filed as what it is.
    #[test]
    fn the_head_names_the_list() {
        let has = |message: &ServerMessage, text: &str| {
            message
                .body()
                .windows(text.len())
                .any(|bytes| bytes == text.as_bytes())
        };
        for filter in [RecentFilter::All, RecentFilter::Friends] {
            let message = recent_matches(filter, &[], &[]).unwrap();
            assert!(has(&message, &format!("M0;0;{}", filter.offset())));
            assert_eq!(
                RecentFilter::of_start(RECENT_MATCHES_START + filter.offset()),
                Some(filter)
            );
        }
    }
}

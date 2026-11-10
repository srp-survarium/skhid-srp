//! The player pop-up's Matches tab in srp's changed lobby movie: a player's
//! latest public matches from the match history, on the ranking page's
//! request (`3B`) at a reserved start and its reply (`91`), the same side
//! channel the rooms use ([`super::rooms`]). Only srp's changed movie asks;
//! a stock game never does.
//!
//! - The ask: `3B 00 <PLAYER_MATCHES_START> <count>`. It is about the
//!   player the asker was last shown the kits of (its last `93`): the
//!   pop-up has only the nickname, so the lobby keeps who that was. The
//!   Watch window's search by name chooses that player itself: it finds the
//!   account by the game's own player search (`find_players`), then sends
//!   `3D 00 <PLAYER_MATCHES_ASK> <account>`.
//!   `3B 00 <OWN_MATCHES_START> <count>` is about the asker itself.
//! - The answer, one `91`. Row `i` is at place `PLAYER_MATCHES_START + i`,
//!   rating and change 0, and its name is ASCII fields with `;` between them
//!   (the nickname as it is):
//!   - row 0: `P1;<n>;<nickname>`;
//!   - then `n` rows, newest first:
//!     `P2;<match id 8 hex>;<place>;<map id>;<mode>;<result>;<kills>;<deaths>;<minutes since its end>;<replay>`.
//!     `place` is the player's place in its team by score, from 1.
//!     `mode` is the mode's lobby number. `result` is [`Outcome`].
//!     `replay` is [`ReplayState`] for the asker's game; no row is left out
//!     by who may watch it (anyone may watch any replay).

use super::push::{Ranked, elo_rows};
use super::{ServerMessage, WireError, cut, limits};

/// The `start` of the ask about the player of the asker's last `93`.
pub const PLAYER_MATCHES_START: u32 = 2_230_000_000;
/// The `start` of the ask about the asker itself.
pub const OWN_MATCHES_START: u32 = 2_230_000_001;
/// The most matches one answer lists.
pub const PLAYER_MATCHES_ROWS: usize = 10;
/// The `player` of a `3D 00 <player> <account>` that chooses the account
/// the next [`PLAYER_MATCHES_START`] ask is about, as a `93` does. Its
/// public matches are anyone's to list (the pop-up's Matches lists them for
/// any player shown), so any account may be chosen.
pub const PLAYER_MATCHES_ASK: u32 = 0xFFFF_FF07;

/// Whose matches an ask is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Whose {
    /// The player the asker was last shown the kits of (its last `93`), or
    /// chose by [`PLAYER_MATCHES_ASK`].
    Shown,
    /// The asker.
    Own,
}

impl Whose {
    /// The ask at ranking start `start`, if it is one.
    pub fn of_start(start: u32) -> Option<Self> {
        match start {
            PLAYER_MATCHES_START => Some(Self::Shown),
            OWN_MATCHES_START => Some(Self::Own),
            _ => None,
        }
    }
}

/// What "Watch" on a listed match would get, for the asker's game: the
/// rows' `<replay>` (P2 here, M1 of `super::recent_matches`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayState {
    /// Too few players, or recordings were off.
    NotRecorded = 0,
    /// Its recording is kept: Watch plays it. (2, a replay file made for
    /// the download, is no longer sent.)
    Kept = 1,
    /// Its recording was deleted (the storage cap).
    Gone = 3,
    /// The asker's game is too old to watch replays.
    OldVersion = 4,
}

/// How a match ended for the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Defeat = 0,
    Victory = 1,
    /// A draw, or no trusted world said who won.
    Draw = 2,
}

/// One match of the player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerMatchRow {
    /// The match server's id: what "Watch replay" asks by.
    pub match_id: u32,
    /// The player's place in the match's roster, which "Watch replay" asks
    /// to see: 0xFF when not known (anyone's view).
    pub place: u8,
    pub map_id: u8,
    /// The mode's lobby number.
    pub mode: u8,
    pub outcome: Outcome,
    pub kills: u16,
    pub deaths: u16,
    pub minutes_since_end: u32,
    pub replay: ReplayState,
    pub shown: Shown,
}

/// What a line of the asker's own list is of. A public match's row has no
/// field for it (the row every movie reads); a private room's or one
/// against bots has it last, and a movie that does not know the field
/// leaves the row out, as before it was ever listed.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shown {
    #[default]
    Public = 0,
    /// A room with a PIN: listed for its own players only.
    Private = 1,
    /// Against the server's bots.
    VsBots = 2,
}

/// `91` with `nickname`'s matches: at most [`PLAYER_MATCHES_ROWS`].
pub fn player_matches(
    nickname: &str,
    matches: &[PlayerMatchRow],
) -> Result<ServerMessage, WireError> {
    let matches = &matches[..matches.len().min(PLAYER_MATCHES_ROWS)];
    let head = format!("P1;{};", matches.len());
    let mut names = vec![format!(
        "{head}{}",
        cut(nickname, limits::NAME - head.len())
    )];
    names.extend(matches.iter().map(|row| {
        let row_text = format!(
            "P2;{:08x};{};{};{};{};{};{};{};{}",
            row.match_id,
            row.place,
            row.map_id,
            row.mode,
            row.outcome as u8,
            row.kills,
            row.deaths,
            row.minutes_since_end,
            row.replay as u8
        );
        match row.shown {
            Shown::Public => row_text,
            shown => format!("{row_text};{}", shown as u8),
        }
    }));
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
        .map(|(index, row)| (PLAYER_MATCHES_START + index as u32, row))
        .collect();
    elo_rows(&placed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_longest_rows_fit_and_ten_matches_at_most_are_listed() {
        let row = PlayerMatchRow {
            match_id: u32::MAX,
            place: u8::MAX,
            map_id: u8::MAX,
            mode: u8::MAX,
            outcome: Outcome::Draw,
            kills: u16::MAX,
            deaths: u16::MAX,
            minutes_since_end: u32::MAX,
            replay: ReplayState::OldVersion,
            shown: Shown::VsBots,
        };
        let longest = "P2;ffffffff;255;255;255;2;65535;65535;4294967295;4;2";
        assert!(longest.len() <= limits::NAME);
        // A name past the row's room is cut, never the row refused.
        let message = player_matches(&"Ж".repeat(40), &[row; 12]).unwrap();
        assert_eq!(message.body()[0], 1 + PLAYER_MATCHES_ROWS as u8);
    }
}

//! The player pop-up's Matches tab (`survarium::v026g::lobby::
//! player_matches`): the latest public matches of the player whose kits
//! the asker was shown last, or of the asker itself, from the match
//! history. Private matches and matches against bots are never listed.

use std::time::Instant;

use session::LockExt;
use session::v026g::persist::history::{PlayerMatch, Winner};
use survarium::v026g::lobby::player_matches::{
    Outcome, PLAYER_MATCHES_ROWS, PlayerMatchRow, Whose, player_matches,
};

use super::core::{Core, Who};

impl Core<'_> {
    /// `3B 00 <PLAYER_MATCHES_START | OWN_MATCHES_START> <count>`: at most
    /// `count` (and [`PLAYER_MATCHES_ROWS`]) matches of the account of the
    /// asker's last `93`, or of the asker. Asked before any `93`, or for an
    /// account with no public match, the answer lists none.
    pub(super) fn player_matches(&mut self, who: &Who, whose: Whose, count: u8, now: Instant) {
        let subject = match whose {
            Whose::Shown => self
                .player(who.addr)
                .and_then(|player| player.shown_kits_of),
            Whose::Own => Some(who.account_id),
        };
        let nickname = subject
            .and_then(|account_id| self.accounts.get(account_id))
            .map(|account| account.locked().name.clone())
            .unwrap_or_default();
        let limit = usize::from(count).min(PLAYER_MATCHES_ROWS);
        let found = match (subject, self.accounts.history()) {
            (Some(account_id), Some(history)) if limit > 0 => {
                history.player_matches(account_id, None, limit)
            }
            _ => Ok(Vec::new()),
        };
        let matches = found.unwrap_or_else(|error| {
            log::warn!("{} asked for {nickname}'s matches: {error}", who.name);
            Vec::new()
        });
        let unix = session::v026g::quests::now();
        let rows: Vec<PlayerMatchRow> = matches
            .iter()
            .map(|played| {
                let match_id = played.summary.replay_id.match_id();
                row(
                    played,
                    unix,
                    self.replays.is_some_and(|r| r.kept(match_id, now)),
                )
            })
            .collect();
        self.send_made(who.addr, player_matches(&nickname, &rows));
    }
}

/// A match's row: `unix` is now, `replay` whether its replay can be asked
/// for.
fn row(played: &PlayerMatch, unix: u64, replay: bool) -> PlayerMatchRow {
    let info = &played.summary.info;
    let line = &played.line;
    let outcome = match info.winner {
        Winner::Draw | Winner::Unknown => Outcome::Draw,
        winner if winner.team() == Some(line.team) => Outcome::Victory,
        _ => Outcome::Defeat,
    };
    PlayerMatchRow {
        match_id: played.summary.replay_id.match_id(),
        place: line.place,
        map_id: info.map_id,
        mode: info.game_mode.lobby_number(),
        outcome,
        kills: line.kills.min(u64::from(u16::MAX)) as u16,
        deaths: line.deaths.min(u64::from(u16::MAX)) as u16,
        minutes_since_end: (unix.saturating_sub(info.ended_at) / 60).min(u64::from(u32::MAX))
            as u32,
        replay,
    }
}

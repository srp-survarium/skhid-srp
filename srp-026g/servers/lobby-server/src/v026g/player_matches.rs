//! The lists of matches from the match history: the player pop-up's
//! Matches tab (`survarium::v026g::lobby::player_matches`), the latest
//! public matches of the player whose kits the asker was shown last, or of
//! the asker itself; and the Watch hub's Recent tab
//! (`survarium::v026g::lobby::recent_matches`), the latest public matches,
//! or those of the asker's friends. Private rooms' matches and matches
//! against bots are never listed.

use std::time::Instant;

use session::LockExt;
use session::v026g::persist::history::{
    MatchSummary, OwnKind, PlayerLine, PlayerMatch, Presence, Winner,
};
use session::v026g::replays;
use survarium::v026g::lobby::friend_row;
use survarium::v026g::lobby::player_matches::{
    Outcome, PLAYER_MATCHES_ROWS, PlayerMatchRow, ReplayState, Shown, Whose, player_matches,
};
use survarium::v026g::lobby::recent_matches::{
    RECENT_MATCHES_ROWS, RecentFilter, RecentMatchRow, recent_matches,
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
        // The name the search shows (`name_of`), which the movie takes the
        // answer by: also an account with no 0.26g state.
        let nickname = subject
            .and_then(|account_id| self.name_of(account_id))
            .unwrap_or_default();
        let limit = usize::from(count).min(PLAYER_MATCHES_ROWS);
        // The asker's own list has its private rooms' matches and those
        // against bots too (the owner's of 2026-11-10: a player finds its
        // own matches there; only public ones were ever listed, a rule of
        // the site's that the game's lists took over). Another player's
        // list, the site and the totals stay public only.
        let found = match (subject, self.accounts.history()) {
            (Some(account_id), Some(history)) if limit > 0 => match whose {
                Whose::Own => history.own_matches(account_id, limit).map(|matches| {
                    matches
                        .into_iter()
                        .map(|(played, kind)| {
                            let shown = match kind {
                                OwnKind::Public => Shown::Public,
                                OwnKind::Private => Shown::Private,
                                OwnKind::VsBots => Shown::VsBots,
                            };
                            (played, shown)
                        })
                        .collect::<Vec<_>>()
                }),
                Whose::Shown => history.player_matches(account_id, None, limit).map(|matches| {
                    matches
                        .into_iter()
                        .map(|played| (played, Shown::Public))
                        .collect::<Vec<_>>()
                }),
            },
            _ => Ok(Vec::new()),
        };
        let matches = found.unwrap_or_else(|error| {
            log::warn!("{} asked for {nickname}'s matches: {error}", who.name);
            Vec::new()
        });
        let unix = session::v026g::quests::now();
        let rows: Vec<PlayerMatchRow> = matches
            .iter()
            .map(|(played, shown)| PlayerMatchRow {
                shown: *shown,
                ..player_row(played, unix, self.replay_state(who, &played.summary, now))
            })
            .collect();
        self.send_made(who.addr, player_matches(&nickname, &rows));
    }

    /// `3B 00 <RECENT_MATCHES_START + f> <count>`: at most `count` (and
    /// [`RECENT_MATCHES_ROWS`]) of the newest public matches, or of those a
    /// friend of the asker played in.
    pub(super) fn recent_matches(
        &mut self,
        who: &Who,
        filter: RecentFilter,
        count: u8,
        now: Instant,
    ) {
        let limit = usize::from(count).min(RECENT_MATCHES_ROWS);
        let friends = self
            .store
            .account(who.account_id)
            .map(|account| account.locked().friends.clone())
            .unwrap_or_default();
        let found = match (self.accounts.history(), filter) {
            (Some(history), RecentFilter::All) if limit > 0 => history.matches(None, limit, None),
            (Some(history), RecentFilter::Friends) if limit > 0 => {
                history.matches_with(&friends, limit)
            }
            _ => Ok(Vec::new()),
        };
        let matches = found.unwrap_or_else(|error| {
            log::warn!("{} asked for the recent matches: {error}", who.name);
            Vec::new()
        });
        let unix = session::v026g::quests::now();
        let rows: Vec<RecentMatchRow> = matches
            .iter()
            .map(|summary| recent_row(summary, unix, self.replay_state(who, summary, now)))
            .collect();
        let friend_rows = self.friends_in(who, &matches, &friends);
        self.send_made(who.addr, recent_matches(&rows, &friend_rows));
    }

    /// The `F1` rows of `friends` in `matches`, in the matches' order.
    fn friends_in(&self, who: &Who, matches: &[MatchSummary], friends: &[u32]) -> Vec<String> {
        let Some(history) = self.accounts.history() else {
            return Vec::new();
        };
        let ids: Vec<u64> = matches.iter().map(|summary| summary.id).collect();
        let found = history
            .players_among(&ids, friends)
            .unwrap_or_else(|error| {
                log::warn!(
                    "{} asked for the recent matches' friends: {error}",
                    who.name
                );
                Vec::new()
            });
        matches
            .iter()
            .flat_map(|summary| {
                found
                    .iter()
                    .filter(move |(id, ..)| *id == summary.id)
                    .filter_map(|&(_, account_id, team)| {
                        let name = self.accounts.get(account_id)?.locked().name.clone();
                        Some(friend_row(summary.replay_id.match_id(), None, team, &name))
                    })
            })
            .collect()
    }

    /// What "Watch" on `summary`'s match would get for `who`'s game
    /// (`Replays::state`).
    fn replay_state(&self, who: &Who, summary: &MatchSummary, now: Instant) -> ReplayState {
        let Some(held) = self.replays else {
            return ReplayState::NotRecorded;
        };
        let revision = self.store.client_revision(who.session_id);
        match held.state(summary.replay_id.match_id(), revision, now) {
            replays::ReplayState::NotRecorded => ReplayState::NotRecorded,
            replays::ReplayState::Kept => ReplayState::Kept,
            replays::ReplayState::Gone => ReplayState::Gone,
            replays::ReplayState::OldVersion => ReplayState::OldVersion,
        }
    }
}

/// Minutes from the end `ended_at` to `unix`.
fn minutes_since(ended_at: u64, unix: u64) -> u32 {
    (unix.saturating_sub(ended_at) / 60).min(u64::from(u32::MAX)) as u32
}

/// A recent match's row: `unix` is now.
fn recent_row(summary: &MatchSummary, unix: u64, replay: ReplayState) -> RecentMatchRow {
    let info = &summary.info;
    RecentMatchRow {
        match_id: summary.replay_id.match_id(),
        map_id: info.map_id,
        mode: info.game_mode.lobby_number(),
        // No trusted world said the standing: no points either; a free-for-all has no teams'.
        score: (!matches!(info.winner, Winner::Unknown | Winner::MostKills(_)))
            .then_some(info.team_points),
        players: u8::try_from(summary.players).unwrap_or(u8::MAX),
        length_ms: info.duration_ms,
        minutes_since_end: minutes_since(info.ended_at, unix),
        replay,
    }
}

/// What a player's row says of its match, as the results screen does (`rewards::won`): the screen
/// has only a victory and a defeat, so one who walked out of a match nobody won lost it there, as
/// in the rating; a draw is for those still in the match at its end.
fn outcome_of(winner: Winner, line: &PlayerLine) -> Outcome {
    match (winner, line.presence) {
        (winner, _) if winner.won(line) => Outcome::Victory,
        (Winner::Draw | Winner::Unknown, Presence::Finished) => Outcome::Draw,
        _ => Outcome::Defeat,
    }
}

/// A player's match's row: `unix` is now.
fn player_row(played: &PlayerMatch, unix: u64, replay: ReplayState) -> PlayerMatchRow {
    let info = &played.summary.info;
    let line = &played.line;
    let outcome = outcome_of(info.winner, line);
    PlayerMatchRow {
        match_id: played.summary.replay_id.match_id(),
        // the movie asks by it: 0xFF, none, is anyone's view
        place: line.seat.unwrap_or(u8::MAX),
        map_id: info.map_id,
        mode: info.game_mode.lobby_number(),
        outcome,
        kills: line.kills.min(u64::from(u16::MAX)) as u16,
        deaths: line.deaths.min(u64::from(u16::MAX)) as u16,
        minutes_since_end: minutes_since(info.ended_at, unix),
        replay,
        shown: Shown::Public,
    }
}

#[cfg(test)]
mod outcome_tests {
    use super::*;
    use session::v026g::persist::history::Counters;

    fn outcome(winner: Winner, team: u8, kills: u64, presence: Presence) -> Outcome {
        let line = PlayerLine {
            team,
            place: 1,
            seat: Some(0),
            presence,
            rating: 1000,
            score: 0,
            kills,
            deaths: 0,
            counters: Counters::default(),
        };
        outcome_of(winner, &line)
    }

    #[test]
    fn a_winners_team_won_even_for_one_who_left() {
        assert!(matches!(outcome(Winner::FirstTeam, 0, 0, Presence::Finished), Outcome::Victory));
        assert!(matches!(outcome(Winner::FirstTeam, 0, 0, Presence::Left), Outcome::Victory));
    }

    #[test]
    fn the_other_team_lost() {
        assert!(matches!(outcome(Winner::SecondTeam, 0, 0, Presence::Finished), Outcome::Defeat));
    }

    #[test]
    fn nobody_won_a_draw_for_those_who_stayed() {
        assert!(matches!(outcome(Winner::Draw, 0, 0, Presence::Finished), Outcome::Draw));
        assert!(matches!(outcome(Winner::Unknown, 1, 0, Presence::Finished), Outcome::Draw));
    }

    #[test]
    fn nobody_won_a_defeat_for_one_who_left_as_on_the_results_screen() {
        assert!(matches!(outcome(Winner::Draw, 0, 0, Presence::Left), Outcome::Defeat));
        assert!(matches!(outcome(Winner::Unknown, 1, 0, Presence::Left), Outcome::Defeat));
    }

    /// A free-for-all: whoever has the most kills won, whatever its team; the others lost.
    #[test]
    fn a_free_for_all_is_won_by_the_most_kills() {
        let ffa = Winner::MostKills(7);
        assert!(matches!(outcome(ffa, 1, 7, Presence::Finished), Outcome::Victory));
        assert!(matches!(outcome(ffa, 0, 7, Presence::Left), Outcome::Victory));
        assert!(matches!(outcome(ffa, 0, 6, Presence::Finished), Outcome::Defeat));
    }
}

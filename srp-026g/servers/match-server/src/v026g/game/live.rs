//! What a running match says of itself to those outside it
//! (`MatchRegistry::set_live`): how far it is, who is in it and the score.
//! Nothing here changes the match.

use super::*;
use crate::v026g::registry::{Live, LivePhase};
use survarium::v026g::match_msgs::MAX_PLAYERS;

/// The places' kills and deaths are told of a trusted world at least this
/// far behind the match clock: the longest Live delay, so that whatever
/// delay the lobby's Live window watches at, its roster's numbers never say
/// who died before its picture does (they may come later).
const KILLS_DEATHS_BEHIND_MS: u32 = session::LiveDelay::MAX_SECONDS as u32 * 1000;

/// Each place's kills and deaths.
type PlaceRecords = [(u16, u16); MAX_PLAYERS];

/// The places' kills and deaths of the trusted worlds, by world time,
/// oldest first: those of the last [`KILLS_DEATHS_BEHIND_MS`] and the
/// newest one before them.
#[derive(Default)]
pub(super) struct DelayedRecords(VecDeque<(u32, PlaceRecords)>);

impl DelayedRecords {
    /// A trusted world's records (its rules' `kills_deaths`), read at `time`.
    pub(super) fn observe(&mut self, time: u32, records: &[snapshot::KillsDeaths]) {
        if self.0.back().is_some_and(|&(last, _)| last >= time) {
            return;
        }
        let mut kept = [(0, 0); MAX_PLAYERS];
        for (record, kd) in records.iter().zip(&mut kept) {
            *kd = (record.kills, record.deaths);
        }
        self.0.push_back((time, kept));
        let cut = time.saturating_sub(KILLS_DEATHS_BEHIND_MS);
        while self.0.get(1).is_some_and(|&(time, _)| time <= cut) {
            self.0.pop_front();
        }
    }

    /// As told at the match clock `clock`: none before a world old enough.
    fn at(&self, clock: u32) -> PlaceRecords {
        let cut = clock.saturating_sub(KILLS_DEATHS_BEHIND_MS);
        self.0
            .iter()
            .rev()
            .find(|&&(time, _)| time <= cut)
            .map_or([(0, 0); MAX_PLAYERS], |&(_, records)| records)
    }
}

impl Game {
    /// How far the match is now.
    pub(crate) fn live_phase(&self) -> LivePhase {
        if self.result_made {
            LivePhase::Over
        } else {
            match self.rule.state {
                rules::state::IN_PROCESS => LivePhase::Playing,
                rules::state::COUNTDOWN => LivePhase::Countdown,
                _ => LivePhase::Gathering,
            }
        }
    }

    /// The match as it is at `now`.
    pub(crate) fn live_now(&self, now: Instant) -> Live {
        let phase = self.live_phase();
        let mut players = [0; 2];
        let mut present = 0_u32;
        for (index, (place, peer)) in self.formed.places.iter().zip(&self.peers).enumerate() {
            let here = match peer {
                Some(peer) => peer.stage != Stage::Gone,
                // The server's bot: to come until it is in, then in until
                // it gives its place up to a late joiner (`driven.rs`).
                None if place.driven => !self.result_made && !self.has_left(index),
                // Not connected yet: still to come until the countdown,
                // and one taken into the match being played until it is in.
                None => phase == LivePhase::Gathering || self.coming(index),
            };
            if here {
                players[usize::from(place.team.min(1))] += 1;
                present |= 1 << index;
            }
        }
        let clock = self.clock(now);
        let mut kills = [0_u16; 2];
        if let Some(rules) = self.state.as_ref().and_then(|state| state.rules.as_ref()) {
            for (place, record) in self.formed.places.iter().zip(&rules.kills_deaths) {
                let team = &mut kills[usize::from(place.team.min(1))];
                *team = team.saturating_add(record.kills);
            }
        }
        // By place: behind the match, as a Live window's picture is.
        let kills_deaths = self.live_records.at(clock);
        let ms = |ms: u32| Duration::from_millis(ms.into());
        Live {
            at: now,
            phase,
            players,
            present,
            kills,
            kills_deaths,
            played: self
                .rule
                .match_started_at()
                .map(|started| ms(clock.saturating_sub(started))),
            time_left: self
                .rule
                .time_limit_at()
                .map(|limit| ms(limit.saturating_sub(clock))),
            standing: self.state.as_ref().and_then(|state| {
                let standing = state.standing?;
                Some((standing, ms(clock.saturating_sub(state.time))))
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records(kills: u16, deaths: u16) -> Vec<snapshot::KillsDeaths> {
        vec![snapshot::KillsDeaths {
            kills,
            deaths,
            online: true,
        }]
    }

    /// Review of 2026-11-13: the roster's numbers came with each trusted
    /// world, telling a Live watcher who died before its picture did.
    #[test]
    fn the_places_kills_and_deaths_are_told_the_longest_delay_behind() {
        let mut delayed = DelayedRecords::default();
        delayed.observe(5_000, &records(1, 1));
        delayed.observe(10_000, &records(2, 1));
        assert_eq!(delayed.at(62_999)[0], (0, 0), "no world old enough");
        assert_eq!(delayed.at(63_000)[0], (1, 1));
        assert_eq!(delayed.at(68_000)[0], (2, 1));
        delayed.observe(70_000, &records(3, 1));
        assert_eq!(delayed.at(70_000)[0], (2, 1));
        assert_eq!(delayed.at(127_999)[0], (2, 1));
        assert_eq!(delayed.at(128_000)[0], (3, 1));
        delayed.observe(200_000, &records(4, 1));
        assert_eq!(delayed.0.len(), 2, "the newest old enough is kept");
        assert_eq!(delayed.at(200_000)[0], (3, 1));
    }
}

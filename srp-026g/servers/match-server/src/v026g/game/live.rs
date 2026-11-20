//! What a running match says of itself to those outside it
//! (`MatchRegistry::set_live`): how far it is, who is in it and the score.
//! Nothing here changes the match.

use super::*;
use crate::v026g::registry::{Live, LivePhase};

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
        let mut kills = [0_u16; 2];
        if let Some(rules) = self.state.as_ref().and_then(|state| state.rules.as_ref()) {
            for (place, record) in self.formed.places.iter().zip(&rules.kills_deaths) {
                let team = &mut kills[usize::from(place.team.min(1))];
                *team = team.saturating_add(record.kills);
            }
        }
        let clock = self.clock(now);
        let ms = |ms: u32| Duration::from_millis(ms.into());
        Live {
            at: now,
            phase,
            players,
            present,
            kills,
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

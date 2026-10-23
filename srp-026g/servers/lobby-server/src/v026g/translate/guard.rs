//! Protecting the API key (`docs/notes/chat-translation.md`, "Protecting
//! the API key"): what players type must not get the key's account
//! restricted.
//!
//! - **A player's throttle:** [`THROTTLE_NOS`] "no" verdicts on its lines
//!   (the word list or the gate) within [`THROTTLE_WINDOW`] and none of its
//!   lines goes to the backend for [`THROTTLE_COOLDOWN`].
//! - **The server's breaker:** when the backend's refusals pass
//!   [`REFUSAL_RATE`] of the lines sent within [`BREAKER_WINDOW`] (at least
//!   [`REFUSAL_MIN`] sent), or [`REFUSAL_BURST`] come within [`BURST`];
//!   or the "no" verdicts pass [`NO_RATE`] of the lines judged (at least
//!   [`NO_MIN`]), or [`NO_BURST`] come within [`BURST`]: nothing goes to the
//!   backend for [`BREAKER_OFF`], said loudly in the log.
//! - Counts only: no text, ever.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

pub const THROTTLE_NOS: usize = 3;
pub const THROTTLE_WINDOW: Duration = Duration::from_secs(10 * 60);
pub const THROTTLE_COOLDOWN: Duration = Duration::from_secs(60 * 60);
pub const BREAKER_WINDOW: Duration = Duration::from_secs(15 * 60);
pub const BURST: Duration = Duration::from_secs(60);
pub const REFUSAL_RATE: f32 = 0.05;
pub const REFUSAL_MIN: usize = 20;
pub const REFUSAL_BURST: usize = 3;
pub const NO_RATE: f32 = 0.15;
pub const NO_MIN: usize = 40;
pub const NO_BURST: usize = 15;
pub const BREAKER_OFF: Duration = Duration::from_secs(60 * 60);

/// What became of a line judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// Sent, and translated.
    Sent,
    /// A "no" (the word list or the gate).
    No,
    /// Sent, and the backend refused it.
    Refused,
}

/// Whether a player's line may go to the backend now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Allow {
    Yes,
    /// The player is in its cooldown.
    Player,
    /// The breaker is open: nothing goes.
    Server,
}

#[derive(Debug, Default)]
pub struct Guard {
    /// Per account: its recent "no" verdicts, and its cooldown's end.
    players: HashMap<u32, (VecDeque<Instant>, Option<Instant>)>,
    events: VecDeque<(Instant, Event)>,
    off_until: Option<Instant>,
    /// How often the breaker opened, and players were throttled.
    pub trips: u64,
    pub throttled: u64,
}

impl Guard {
    pub fn allows(&mut self, player: u32, now: Instant) -> Allow {
        if self.off_until.is_some_and(|until| now < until) {
            return Allow::Server;
        }
        match self.players.get(&player) {
            Some((_, Some(until))) if now < *until => Allow::Player,
            _ => Allow::Yes,
        }
    }

    /// What became of one of `player`'s lines.
    pub fn record(&mut self, player: u32, event: Event, now: Instant) {
        if event == Event::No {
            let (nos, until) = self.players.entry(player).or_default();
            nos.push_back(now);
            while nos
                .front()
                .is_some_and(|at| now.duration_since(*at) > THROTTLE_WINDOW)
            {
                nos.pop_front();
            }
            if nos.len() >= THROTTLE_NOS && until.is_none_or(|until| now >= until) {
                *until = Some(now + THROTTLE_COOLDOWN);
                nos.clear();
                self.throttled += 1;
                log::warn!(
                    "Chat translation: account {player} had {THROTTLE_NOS} lines refused within \
                     {} min; none of its lines is sent for {} min ({} players so far)",
                    THROTTLE_WINDOW.as_secs() / 60,
                    THROTTLE_COOLDOWN.as_secs() / 60,
                    self.throttled
                );
            }
        }
        self.events.push_back((now, event));
        while self
            .events
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) > BREAKER_WINDOW)
        {
            self.events.pop_front();
        }
        if self.off_until.is_some_and(|until| now < until) {
            return;
        }
        let count = |event: Event, within: Duration| {
            self.events
                .iter()
                .filter(|(at, kind)| *kind == event && now.duration_since(*at) <= within)
                .count()
        };
        let (sent, nos, refusals) = (
            count(Event::Sent, BREAKER_WINDOW),
            count(Event::No, BREAKER_WINDOW),
            count(Event::Refused, BREAKER_WINDOW),
        );
        let called = sent + refusals;
        let judged = called + nos;
        let why = if count(Event::Refused, BURST) >= REFUSAL_BURST {
            Some("a burst of refusals")
        } else if called >= REFUSAL_MIN && refusals as f32 > called as f32 * REFUSAL_RATE {
            Some("too many refusals")
        } else if count(Event::No, BURST) >= NO_BURST {
            Some("a burst of lines refused by the gate")
        } else if judged >= NO_MIN && nos as f32 > judged as f32 * NO_RATE {
            Some("too many lines refused by the gate")
        } else {
            None
        };
        if let Some(why) = why {
            self.off_until = Some(now + BREAKER_OFF);
            self.trips += 1;
            log::error!(
                "CHAT TRANSLATION OFF for {} min: {why} (in {} min: {called} sent, {refusals} \
                 refused by the model, {nos} refused by the gate; {} times so far)",
                BREAKER_OFF.as_secs() / 60,
                BREAKER_WINDOW.as_secs() / 60,
                self.trips
            );
            self.events.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_player_with_three_noes_in_ten_minutes_is_not_sent_for_an_hour() {
        let mut guard = Guard::default();
        let start = Instant::now();
        // Two, then a third after the window: no throttle.
        guard.record(7, Event::No, start);
        guard.record(7, Event::No, start + Duration::from_secs(60));
        guard.record(
            7,
            Event::No,
            start + THROTTLE_WINDOW + Duration::from_secs(120),
        );
        assert_eq!(
            guard.allows(7, start + THROTTLE_WINDOW + Duration::from_secs(121)),
            Allow::Yes
        );
        // Three within the window.
        guard.record(
            7,
            Event::No,
            start + THROTTLE_WINDOW + Duration::from_secs(150),
        );
        assert_eq!(
            guard.allows(7, start + THROTTLE_WINDOW + Duration::from_secs(151)),
            Allow::Yes
        );
        let at = start + THROTTLE_WINDOW + Duration::from_secs(180);
        guard.record(7, Event::No, at);
        assert_eq!(guard.allows(7, at), Allow::Player);
        assert_eq!(guard.allows(8, at), Allow::Yes);
        assert_eq!(guard.allows(7, at + THROTTLE_COOLDOWN), Allow::Yes);
        assert_eq!(guard.throttled, 1);
    }

    #[test]
    fn the_breaker_opens_on_refusals_or_noes_and_closes_after_an_hour() {
        // A burst of refusals.
        let mut guard = Guard::default();
        let start = Instant::now();
        for second in 0..REFUSAL_BURST as u64 {
            guard.record(
                second as u32,
                Event::Refused,
                start + Duration::from_secs(second),
            );
        }
        let at = start + Duration::from_secs(REFUSAL_BURST as u64);
        assert_eq!(guard.allows(99, at), Allow::Server);
        assert_eq!(guard.allows(99, at + BREAKER_OFF), Allow::Yes);
        assert_eq!(guard.trips, 1);

        // A rate: 2 refusals in 21 lines sent over 14 minutes (over 5 %),
        // spread out so that no burst opens it.
        let mut guard = Guard::default();
        for minute in 0..19u64 {
            guard.record(1, Event::Sent, start + Duration::from_secs(minute * 40));
        }
        guard.record(1, Event::Refused, start + Duration::from_secs(800));
        assert_eq!(
            guard.allows(1, start + Duration::from_secs(801)),
            Allow::Yes
        );
        guard.record(1, Event::Refused, start + Duration::from_secs(840));
        assert_eq!(
            guard.allows(1, start + Duration::from_secs(841)),
            Allow::Server
        );

        // Ordinary chat with a few noes leaves it closed.
        let mut guard = Guard::default();
        for line in 0..200u64 {
            let event = if line % 20 == 0 {
                Event::No
            } else {
                Event::Sent
            };
            guard.record(line as u32, event, start + Duration::from_secs(line * 4));
        }
        assert_eq!(
            guard.allows(1, start + Duration::from_secs(800)),
            Allow::Yes
        );
        assert_eq!(guard.trips, 0);
    }
}

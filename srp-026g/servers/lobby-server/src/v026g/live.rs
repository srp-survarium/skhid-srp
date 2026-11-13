//! The lobby's Live window (srp's changed lobby movie, the "Matches now" tab
//! revived): the matches being played, a match's roster, and watching one
//! as it is played (`docs/notes/replay-stream.md`, "Live").
//!
//! - The list: `3B 00 <LIVE_START> <count>` (the movie polls every 5 s), at
//!   most [`lobby::LIVE_ROWS`] matches: `L0;<n>;<delay s>` (how far behind
//!   a watch is), then a row a match, `L1;<match 8 hex>;<map id>;<mode>;
//!   <phase>;<players a>;<players b>;<score a|->;<score b|->;<played s>;
//!   <spectators>;<watch>`. `phase` 0 made, 1 countdown, 2 in play, 3 over;
//!   `watch` 1 may, 0 full, 2 not watchable (a private room's, or one of
//!   fixed rosters, which takes no spectator). A private room's match is
//!   not listed. After the `L1` rows, `F1;<match 8 hex>;<place>;<team>;
//!   <name>` for each friend of the asker in a listed match (the Watch
//!   hub's pinned cards and the friends list's Spectate).
//! - The roster: `3D 00 <LIVE_ROSTER_ASK> <match>` chooses the match, then
//!   `3B 00 <LIVE_ROSTER_START> <count>` gets a row a place in it, at most
//!   [`lobby::LIVE_ROSTER_ROWS`]: `L2;<match 8 hex>;<place>;<team>;<kills>;
//!   <deaths>;<name>`.
//! - Watching: the replay session's own ask (`replay`) for a match still
//!   being played sends the game to it as a spectator (#1285's watch), its
//!   delay behind ([`Config::delay`]: nobody calls a friend the positions);
//!   answered `R1;watching;<match>;0`, or `busy` when full ([`Config`]'s
//!   caps), `not_yours` when it is not watchable.
//!
//! Each row's place in the `91` is the request's start and its index.

use std::collections::HashSet;
use std::time::Instant;

use session::v026g::replays::Status;
use session::{LiveDelay, LockExt, QueuePool, WatchKind, Watcher};
use survarium::v026g::lobby;
use survarium::v026g::lobby::limits;

use super::core::{Core, Who};
use super::play::{LivePhase, LiveView, Running};

/// `SRP_LIVE_WATCH_DELAY_S`, `SRP_LIVE_WATCH_PER_MATCH`,
/// `SRP_LIVE_WATCH_MAX`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// How far behind the match a Live window's spectator watches.
    pub delay: LiveDelay,
    /// Spectators of one match at most, the Live window's and Queue and
    /// spectate's together (each costs about a player's traffic).
    pub per_match: usize,
    /// And of all matches.
    pub most: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            delay: LiveDelay::DEFAULT,
            per_match: 4,
            most: 8,
        }
    }
}

impl Config {
    pub fn from_env() -> Self {
        let number = |name: &str| {
            std::env::var(name)
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok())
        };
        let default = Self::default();
        Self {
            delay: delay_from(std::env::var("SRP_LIVE_WATCH_DELAY_S").ok().as_deref()),
            per_match: number("SRP_LIVE_WATCH_PER_MATCH").map_or(default.per_match, |n| n as usize),
            most: number("SRP_LIVE_WATCH_MAX").map_or(default.most, |n| n as usize),
        }
    }
}

/// `SRP_LIVE_WATCH_DELAY_S`, parsed: a value the match could not serve (no spectator would ever
/// get a world) is refused here, loudly, for the default.
fn delay_from(value: Option<&str>) -> LiveDelay {
    let Some(value) = value else { return LiveDelay::DEFAULT };
    value.trim().parse().ok().and_then(LiveDelay::from_secs).unwrap_or_else(|| {
        log::error!(
            "SRP_LIVE_WATCH_DELAY_S is `{value}`; it must be whole seconds from 0 to {}. Using {} s",
            LiveDelay::MAX_SECONDS,
            LiveDelay::DEFAULT.as_secs()
        );
        LiveDelay::DEFAULT
    })
}

/// Whether a Live window spectator may watch a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Watchable {
    May,
    Full,
    /// A private room's, of fixed rosters, or with a player who keeps its
    /// matches from being watched.
    Not,
}

impl Watchable {
    fn code(self) -> u8 {
        match self {
            Self::May => 1,
            Self::Full => 0,
            Self::Not => 2,
        }
    }
}

fn phase_code(phase: LivePhase) -> u8 {
    match phase {
        LivePhase::Gathering => 0,
        LivePhase::Countdown => 1,
        LivePhase::Playing => 2,
        LivePhase::Over => 3,
    }
}

impl Core<'_> {
    /// Whether the match (the lobby's id) takes no more spectators: its own,
    /// or all matches', at [`Config`]'s caps. Every spectator counts, the
    /// Live window's and Queue and spectate's (`rooms`' `room_watchers`).
    pub(super) fn watchers_full(&self, match_id: u32) -> bool {
        let matchmaking = self.store.matchmaking();
        let all: usize = self
            .matches
            .keys()
            .map(|&id| matchmaking.watchers_of(id).len())
            .sum();
        matchmaking.watchers_of(match_id).len() >= self.settings.live.per_match
            || all >= self.settings.live.most
    }

    fn watchable(&self, match_id: u32, running: &Running) -> Watchable {
        if running.private || running.pool != QueuePool::GrowingRoster {
            Watchable::Not
        } else if self.watchers_full(match_id) {
            Watchable::Full
        } else {
            Watchable::May
        }
    }

    /// The running matches the window shows, by the match server's id.
    fn listed(&self) -> Vec<(u32, &Running)> {
        let mut listed: Vec<(u32, &Running)> = self
            .matches
            .iter()
            .filter(|(_, running)| !running.private)
            .map(|(&id, running)| (id, running))
            .collect();
        listed.sort_by_key(|(_, running)| running.ticket.match_id);
        listed
    }

    /// `3B 00 <LIVE_START> <count>`: the matches being played.
    pub(super) fn live_list(&mut self, who: &Who, count: u8, now: Instant) {
        let count = usize::from(count.min(lobby::LIVE_ROWS));
        let friends: HashSet<u32> = self
            .store
            .account(who.account_id)
            .map(|account| account.locked().friends.iter().copied().collect())
            .unwrap_or_default();
        let mut rows: Vec<String> = Vec::new();
        let mut friend_rows: Vec<String> = Vec::new();
        for (id, running) in self.listed().into_iter().take(count) {
            let live = self
                .host
                .live(running.ticket.match_id, now)
                .unwrap_or_default();
            let mut players = [0u8; 2];
            for (place, &(_, team)) in running.seats.iter().enumerate() {
                if live.present & (1 << place) != 0 {
                    players[usize::from(team.min(1))] += 1;
                }
            }
            let score = |team: usize| {
                live.points
                    .map_or_else(|| "-".to_owned(), |points| points[team].to_string())
            };
            rows.push(format!(
                "L1;{:08x};{};{};{};{};{};{};{};{};{};{}",
                running.ticket.match_id,
                running.map.map().id,
                running.map.mode().lobby_number(),
                phase_code(live.phase),
                players[0],
                players[1],
                score(0),
                score(1),
                live.played.map_or(0, |played| played.as_secs()),
                self.store.matchmaking().watchers_of(id).len(),
                self.watchable(id, running).code(),
            ));
            if friends.is_empty() {
                continue;
            }
            for (place, (&(session, team), name)) in
                running.seats.iter().zip(&running.names).enumerate()
            {
                let friend = self
                    .store
                    .session_account(session)
                    .is_some_and(|account| friends.contains(&account.locked().id));
                if friend && live.present & (1 << place) != 0 {
                    friend_rows.push(lobby::friend_row(
                        running.ticket.match_id,
                        Some(place as u8),
                        team,
                        name,
                    ));
                }
            }
        }
        rows.insert(
            0,
            format!("L0;{};{}", rows.len(), self.settings.live.delay.as_secs()),
        );
        // As many friends' rows as one `91` holds (255 rows): a big friends'
        // group in many matches loses its last rows, never the answer.
        let room = usize::from(u8::MAX) - rows.len();
        rows.extend(friend_rows.into_iter().take(room));
        debug_assert!(rows.iter().all(|row| row.len() <= limits::NAME));
        self.send_made(who.addr, lobby::live_rows(lobby::LIVE_START, &rows));
    }

    /// `3D 00 <LIVE_ROSTER_ASK> <match>`: the match whose roster comes next.
    pub(super) fn live_roster_ask(&mut self, who: &Who, match_id: u32) {
        self.live_roster.insert(who.account_id, match_id);
    }

    /// `3B 00 <LIVE_ROSTER_START> <count>`: the chosen match's places, as
    /// the match server has them (kills and deaths of its newest trusted
    /// world); none when it is not being played (or not listed).
    pub(super) fn live_roster(&mut self, who: &Who, count: u8, now: Instant) {
        let count = usize::from(count.min(lobby::LIVE_ROSTER_ROWS));
        let chosen = self.live_roster.get(&who.account_id).copied();
        let running = chosen.and_then(|match_id| {
            self.listed()
                .into_iter()
                .find(|(_, running)| running.ticket.match_id == match_id)
                .map(|(_, running)| running)
        });
        let mut rows = Vec::new();
        if let Some(running) = running {
            let live: LiveView = self
                .host
                .live(running.ticket.match_id, now)
                .unwrap_or_default();
            for (place, (&(_, team), name)) in running.seats.iter().zip(&running.names).enumerate()
            {
                if rows.len() == count || live.present & (1 << place) == 0 {
                    continue;
                }
                let (kills, deaths) = live.kills_deaths.get(place).copied().unwrap_or_default();
                let head = format!(
                    "L2;{:08x};{place};{};{kills};{deaths};",
                    running.ticket.match_id,
                    team.min(1)
                );
                rows.push(lobby::named_row(head, name));
            }
        }
        self.send_made(who.addr, lobby::live_rows(lobby::LIVE_ROSTER_START, &rows));
    }

    /// The replay session's ask for `match_id` (the match server's id) when
    /// it is still being played: the game goes to watch it, the delay
    /// behind, `view` in view first (as a replay's ask has it). `None`: the
    /// match is not being played (the recording, then).
    pub(super) fn live_watch(
        &mut self,
        who: &Who,
        match_id: u32,
        view: Option<u8>,
        now: Instant,
    ) -> Option<Status> {
        let (id, running) = self
            .matches
            .iter()
            .find(|(_, running)| running.ticket.match_id == match_id)
            .map(|(&id, running)| (id, running))?;
        let phase = self.host.live(match_id, now).unwrap_or_default().phase;
        if !matches!(phase, LivePhase::Countdown | LivePhase::Playing) {
            // Made and not begun (no world to watch yet), or over: the
            // recording, when there is one.
            return (phase == LivePhase::Gathering).then_some(Status::Busy);
        }
        let status = match self.watchable(id, running) {
            Watchable::Not => Status::NotYours,
            Watchable::Full => Status::Busy,
            Watchable::May => {
                let watcher = Watcher {
                    match_id: id,
                    ticket: match_id,
                    account_id: who.account_id,
                    kind: WatchKind::Live {
                        delay: self.settings.live.delay,
                        view,
                    },
                };
                if self.store.matchmaking().watch(who.session_id, watcher) {
                    log::info!(
                        "{} watches match {match_id:#x} from the Live window, {} s behind",
                        who.name,
                        self.settings.live.delay.as_secs()
                    );
                    // `queue_status` sends its game (`send_to_watch`).
                    self.queue_status(who.addr, now);
                    Status::Watching
                } else {
                    Status::Busy
                }
            }
        };
        Some(status)
    }
}

#[cfg(test)]
mod delay_tests {
    use super::*;

    /// Compose passes its environment by name: a setting it does not list
    /// never reaches the server on the live host.
    #[test]
    fn compose_passes_the_watch_settings_on() {
        let compose = include_str!("../../../../docker-compose.yml");
        for name in [
            "SRP_REPLAY_WATCH",
            "SRP_REPLAY_MODERATORS",
            "SRP_REPLAY_WATCH_MAX",
            "SRP_LIVE_WATCH_DELAY_S",
            "SRP_LIVE_WATCH_PER_MATCH",
            "SRP_LIVE_WATCH_MAX",
        ] {
            let line = format!("- {name}");
            assert!(compose.lines().any(|l| l.trim() == line), "{name}");
        }
    }

    #[test]
    fn a_delay_up_to_the_longest_a_match_can_serve_is_taken() {
        for (value, seconds) in [("0", 0), ("30", 30), (" 45 ", 45), ("58", 58)] {
            assert_eq!(delay_from(Some(value)).as_secs(), seconds, "{value}");
        }
        assert_eq!(delay_from(None), LiveDelay::DEFAULT);
    }

    #[test]
    fn a_delay_a_match_cannot_serve_falls_back_to_the_default() {
        // 59 is the first that cannot start; the last two would wrap if they were not refused.
        for value in ["59", "60", "120", "65536", "18446744073709551615", "99999999999999999999999"] {
            assert_eq!(delay_from(Some(value)), LiveDelay::DEFAULT, "{value}");
        }
    }

    #[test]
    fn a_delay_that_is_no_number_falls_back_to_the_default() {
        for value in ["", "abc", "-1", "1.5", "30s"] {
            assert_eq!(delay_from(Some(value)), LiveDelay::DEFAULT, "{value}");
        }
    }

    #[test]
    fn the_type_holds_nothing_past_the_longest() {
        assert_eq!(LiveDelay::from_secs(58).map(LiveDelay::as_secs), Some(58));
        assert_eq!(LiveDelay::from_secs(59), None);
    }
}

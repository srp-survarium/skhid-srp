//! Anti-AFK (`docs/notes/anti-afk.md`): a player who gives no input for
//! three minutes of play is removed from the match, as a player who leaves
//! is (the owner's of 2026-11-01).
//!
//! The clock of a player runs only while it plays: alive, in a match that
//! is in play, its game in the world and heard from. Anything else pauses
//! it, and any input that does something starts it again from nothing. A
//! private room's match (a PIN) watches nobody, nor does a match with the
//! setting off: that is decided once, when the match is made ([`Watch`]).
//!
//! Only the rule is here; the match (`game/afk.rs`) says what a player is
//! doing at each poll and removes the players this names.

use std::time::{Duration, Instant};

use survarium::v026g::match_msgs::PlayerInput;

/// `SRP_ANTI_AFK` when unset: three minutes (the owner's).
pub(crate) const DEFAULT_LIMIT: Duration = Duration::from_secs(180);
/// The least limit the setting takes: a typo must not remove players who
/// stop to look at the map.
const LEAST_LIMIT: Duration = Duration::from_secs(60);
/// A game that has not shown a running simulation for this long (a hitch
/// of its link, a resync, a slow PC that froze: `Peer::simulating_at`) is
/// not heard, and its player's time does not count: a playing game shows
/// it every 50 or 100 ms (its hash reports).
pub(crate) const HEARD_WITHIN: Duration = Duration::from_secs(2);
/// The input's per-input flag (bit 31), which the client splits off: no
/// key of the player's.
const PER_INPUT_FLAG: u32 = 1 << 31;

/// `SRP_ANTI_AFK`: `off`, or the seconds without input after which a
/// player is removed (default 180, at least 60).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Setting {
    Off,
    After(Duration),
}

impl Default for Setting {
    fn default() -> Self {
        Self::After(DEFAULT_LIMIT)
    }
}

impl Setting {
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            None | Some("") => Self::default(),
            Some(off) if off.eq_ignore_ascii_case("off") => Self::Off,
            Some(seconds) => match seconds.parse::<u64>() {
                Ok(seconds) => Self::After(Duration::from_secs(seconds).max(LEAST_LIMIT)),
                Err(_) => {
                    log::error!(
                        "SRP_ANTI_AFK is `{seconds}`; it must be `off` or a number of seconds. \
                         Using {}",
                        DEFAULT_LIMIT.as_secs()
                    );
                    Self::default()
                }
            },
        }
    }
}

/// What a player is doing at a poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Presence {
    /// Alive in a match in play, its game in the world and heard from:
    /// its time counts.
    Playing,
    /// Anything else: loading, before play or after it, dead, being
    /// resynced, or its game not heard. Its time does not count.
    Away,
}

/// What happens to a player's watch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    /// The match looks at the player.
    Poll(Presence),
    /// The player did something ([`acts`]).
    Input,
}

/// What the match does about it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Keep,
    /// Out of the match, as a player who leaves.
    Remove,
}

/// A player's time without input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Idle {
    /// Its time does not count now; `idle` counted before.
    Paused { idle: Duration },
    /// Its time counts since `since`; `before` counted until then.
    Counting { before: Duration, since: Instant },
    /// Removed from the match.
    Removed,
}

impl Idle {
    pub const FRESH: Self = Self::Paused {
        idle: Duration::ZERO,
    };

    /// The one transition: the state after `event` at `now`, and what the
    /// match does, for a match that removes players after `limit`.
    pub fn step(self, event: Event, now: Instant, limit: Duration) -> (Self, Action) {
        let keep = |state| (state, Action::Keep);
        match (self, event) {
            (Self::Removed, _) => keep(Self::Removed),
            (Self::Paused { .. }, Event::Input) => keep(Self::FRESH),
            (Self::Counting { .. }, Event::Input) => keep(Self::Counting {
                before: Duration::ZERO,
                since: now,
            }),
            (Self::Paused { idle }, Event::Poll(Presence::Away)) => keep(Self::Paused { idle }),
            (Self::Paused { idle }, Event::Poll(Presence::Playing)) => keep(Self::Counting {
                before: idle,
                since: now,
            }),
            (Self::Counting { before, since }, Event::Poll(Presence::Away)) => keep(Self::Paused {
                idle: before + now.saturating_duration_since(since),
            }),
            (Self::Counting { before, since }, Event::Poll(Presence::Playing)) => {
                if before + now.saturating_duration_since(since) >= limit {
                    (Self::Removed, Action::Remove)
                } else {
                    keep(self)
                }
            }
        }
    }
}

/// Whether an input does something: the view turned (its rotation is the
/// mouse's move since the last input, two half floats), or a key or action
/// went down or up against the player's last input.
pub(crate) fn acts(input: &PlayerInput, last_actions: u32) -> bool {
    let turned = input
        .rotation
        .chunks_exact(2)
        // A half's sign bit alone is -0.
        .any(|half| u16::from_le_bytes([half[0], half[1]]) & 0x7FFF != 0);
    turned || (input.actions ^ last_actions) & !PER_INPUT_FLAG != 0
}

/// Whether a game that last showed a running simulation at `simulating_at`
/// is heard from at `now`.
pub(crate) fn heard(simulating_at: Option<Instant>, now: Instant) -> bool {
    simulating_at.is_some_and(|at| now.saturating_duration_since(at) <= HEARD_WITHIN)
}

/// One player's watch: its time and its last actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Player {
    idle: Idle,
    actions: u32,
}

impl Player {
    const FRESH: Self = Self {
        idle: Idle::FRESH,
        actions: 0,
    };
}

/// Whether a match removes idle players: decided once, when it is made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Watch {
    /// A private room's match (a PIN), or the setting off: nobody is
    /// removed.
    Exempt,
    /// By place, its player's watch (a place not seen yet is fresh).
    Watched {
        limit: Duration,
        players: Vec<Player>,
    },
}

impl Watch {
    /// The watch of a match of a private room or not, under the setting.
    pub fn new(setting: Setting, private: bool) -> Self {
        match (setting, private) {
            (Setting::Off, _) | (_, true) => Self::Exempt,
            (Setting::After(limit), false) => Self::Watched {
                limit,
                players: Vec::new(),
            },
        }
    }

    /// The player at `index` sent `input` (one the match took).
    pub fn input(&mut self, index: usize, input: &PlayerInput, now: Instant) {
        let Self::Watched { limit, players } = self else {
            return;
        };
        let player = player(players, index);
        if acts(input, player.actions) {
            player.idle = player.idle.step(Event::Input, now, *limit).0;
        }
        player.actions = input.actions;
    }

    /// The match looks at the player at `index`.
    pub fn poll(&mut self, index: usize, presence: Presence, now: Instant) -> Action {
        let Self::Watched { limit, players } = self else {
            return Action::Keep;
        };
        let player = player(players, index);
        let (idle, action) = player.idle.step(Event::Poll(presence), now, *limit);
        player.idle = idle;
        action
    }

    /// A game connected for the place at `index`: its player starts fresh.
    pub fn restart(&mut self, index: usize) {
        if let Self::Watched { players, .. } = self {
            *player(players, index) = Player::FRESH;
        }
    }

    /// The match removes nobody.
    pub fn is_exempt(&self) -> bool {
        matches!(self, Self::Exempt)
    }
}

fn player(players: &mut Vec<Player>, index: usize) -> &mut Player {
    if players.len() <= index {
        players.resize(index + 1, Player::FRESH);
    }
    &mut players[index]
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::match_msgs::WireTime;

    const LIMIT: Duration = DEFAULT_LIMIT;

    fn secs(start: Instant, at: u64) -> Instant {
        start + Duration::from_secs(at)
    }

    /// Polls every second from `from` to `to` (seconds), all `presence`:
    /// the first second a removal came, if one did.
    fn polls(
        idle: &mut Idle,
        start: Instant,
        from: u64,
        to: u64,
        presence: Presence,
    ) -> Option<u64> {
        for at in from..=to {
            let (next, action) = idle.step(Event::Poll(presence), secs(start, at), LIMIT);
            *idle = next;
            if action == Action::Remove {
                return Some(at);
            }
        }
        None
    }

    fn input(rotation: [u8; 4], actions: u32) -> PlayerInput {
        PlayerInput {
            time: WireTime::tick(0),
            rotation,
            actions,
        }
    }

    #[test]
    fn three_minutes_of_play_without_input_remove_the_player() {
        let start = Instant::now();
        let mut idle = Idle::FRESH;
        assert_eq!(polls(&mut idle, start, 0, 179, Presence::Playing), None);
        assert_eq!(
            polls(&mut idle, start, 180, 300, Presence::Playing),
            Some(180)
        );
        assert_eq!(idle, Idle::Removed);
        // Removed is the end: neither input nor play changes it.
        let (after, action) = idle.step(Event::Input, secs(start, 301), LIMIT);
        assert_eq!((after, action), (Idle::Removed, Action::Keep));
        assert_eq!(polls(&mut idle, start, 302, 600, Presence::Playing), None);
    }

    #[test]
    fn any_input_starts_the_time_again() {
        let start = Instant::now();
        let mut idle = Idle::FRESH;
        assert_eq!(polls(&mut idle, start, 0, 170, Presence::Playing), None);
        idle = idle.step(Event::Input, secs(start, 170), LIMIT).0;
        // 170 + 180: not before.
        assert_eq!(polls(&mut idle, start, 171, 349, Presence::Playing), None);
        assert_eq!(
            polls(&mut idle, start, 350, 400, Presence::Playing),
            Some(350)
        );
    }

    #[test]
    fn loading_and_the_time_before_play_do_not_count() {
        // A game that loads for 10 minutes, then the countdown: away all
        // that time; its three minutes begin with play.
        let start = Instant::now();
        let mut idle = Idle::FRESH;
        assert_eq!(polls(&mut idle, start, 0, 600, Presence::Away), None);
        assert_eq!(idle, Idle::FRESH);
        assert_eq!(polls(&mut idle, start, 601, 780, Presence::Playing), None);
        assert_eq!(
            polls(&mut idle, start, 781, 800, Presence::Playing),
            Some(781)
        );
    }

    #[test]
    fn dead_time_and_gaps_in_its_game_pause_the_time() {
        // 100 s of play, 300 s dead (or its game not heard: both are
        // away), then 79 s of play keep it; the 80th removes it.
        let start = Instant::now();
        let mut idle = Idle::FRESH;
        assert_eq!(polls(&mut idle, start, 0, 100, Presence::Playing), None);
        assert_eq!(polls(&mut idle, start, 101, 400, Presence::Away), None);
        assert_eq!(
            idle,
            Idle::Paused {
                idle: Duration::from_secs(101)
            }
        );
        assert_eq!(polls(&mut idle, start, 401, 479, Presence::Playing), None);
        assert_eq!(
            polls(&mut idle, start, 480, 500, Presence::Playing),
            Some(480)
        );
    }

    #[test]
    fn an_input_while_away_starts_fresh() {
        let start = Instant::now();
        let mut idle = Idle::FRESH;
        assert_eq!(polls(&mut idle, start, 0, 150, Presence::Playing), None);
        assert_eq!(polls(&mut idle, start, 151, 160, Presence::Away), None);
        idle = idle.step(Event::Input, secs(start, 160), LIMIT).0;
        assert_eq!(idle, Idle::FRESH);
        assert_eq!(polls(&mut idle, start, 161, 340, Presence::Playing), None);
        assert_eq!(
            polls(&mut idle, start, 341, 400, Presence::Playing),
            Some(341)
        );
    }

    #[test]
    fn an_input_acts_when_the_view_turns_or_a_key_changes() {
        let still = [0, 0, 0, 0];
        assert!(!acts(&input(still, 0x21), 0x21));
        // -0 is no turn.
        assert!(!acts(&input([0, 0x80, 0, 0x80], 0), 0));
        // The per-input flag is no key.
        assert!(!acts(&input(still, 0x21 | PER_INPUT_FLAG), 0x21));
        assert!(acts(&input(still, 0x20), 0x21));
        assert!(acts(&input(still, 0x21), 0x20));
        // A turn of 1.0 (half 0x3C00) either way.
        assert!(acts(&input([0x00, 0x3C, 0, 0], 0), 0));
        assert!(acts(&input([0, 0, 0x00, 0xBC], 0), 0));
    }

    #[test]
    fn a_game_is_heard_while_it_shows_a_running_simulation() {
        let start = Instant::now();
        assert!(!heard(None, start));
        assert!(heard(Some(start), secs(start, 2)));
        assert!(!heard(
            Some(start),
            start + HEARD_WITHIN + Duration::from_millis(1)
        ));
    }

    #[test]
    fn the_watch_counts_only_inputs_that_act() {
        let start = Instant::now();
        let mut watch = Watch::new(Setting::default(), false);
        for at in 0..=170 {
            assert_eq!(
                watch.poll(0, Presence::Playing, secs(start, at)),
                Action::Keep
            );
        }
        // Keys held as they were and a still mouse: nothing done.
        watch.input(0, &input([0; 4], 0), secs(start, 170));
        assert_eq!(
            watch.poll(0, Presence::Playing, secs(start, 180)),
            Action::Remove
        );

        let mut watch = Watch::new(Setting::default(), false);
        for at in 0..=170 {
            watch.poll(1, Presence::Playing, secs(start, at));
        }
        watch.input(1, &input([0; 4], 0x4), secs(start, 170));
        assert_eq!(
            watch.poll(1, Presence::Playing, secs(start, 180)),
            Action::Keep
        );
        // Its other places are their own.
        assert_eq!(
            watch.poll(3, Presence::Playing, secs(start, 180)),
            Action::Keep
        );
    }

    #[test]
    fn a_private_room_or_the_setting_off_removes_nobody() {
        let start = Instant::now();
        for mut watch in [
            Watch::new(Setting::default(), true),
            Watch::new(Setting::Off, false),
            Watch::new(Setting::Off, true),
        ] {
            assert!(watch.is_exempt());
            for at in 0..=3_600 {
                assert_eq!(
                    watch.poll(0, Presence::Playing, secs(start, at)),
                    Action::Keep
                );
            }
        }
        assert!(!Watch::new(Setting::default(), false).is_exempt());
    }

    #[test]
    fn a_game_connected_again_starts_fresh() {
        let start = Instant::now();
        let mut watch = Watch::new(Setting::default(), false);
        for at in 0..=170 {
            watch.poll(0, Presence::Playing, secs(start, at));
        }
        watch.restart(0);
        assert_eq!(
            watch.poll(0, Presence::Playing, secs(start, 171)),
            Action::Keep
        );
        assert_eq!(
            watch.poll(0, Presence::Playing, secs(start, 200)),
            Action::Keep
        );
        assert_eq!(
            watch.poll(0, Presence::Playing, secs(start, 351)),
            Action::Remove
        );
    }

    #[test]
    fn the_setting_reads_off_or_seconds() {
        assert_eq!(Setting::parse(None), Setting::After(DEFAULT_LIMIT));
        assert_eq!(Setting::parse(Some(" ")), Setting::After(DEFAULT_LIMIT));
        assert_eq!(Setting::parse(Some("OFF")), Setting::Off);
        assert_eq!(
            Setting::parse(Some("240")),
            Setting::After(Duration::from_secs(240))
        );
        assert_eq!(Setting::parse(Some("5")), Setting::After(LEAST_LIMIT));
        assert_eq!(Setting::parse(Some("soon")), Setting::After(DEFAULT_LIMIT));
    }
}

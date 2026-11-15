//! Sharing from the game (`srp-026g/docs/notes/torrent-downloads.md`,
//! "Sharing from the game"): the patched game, while its own
//! `enable_torrent_seeding` ("Allow torrent", a line of user.cfg) is on,
//! makes the pipe `\\.\pipe\skhid_seed-<its pid>` on the login screen and
//! in the lobby and starts `updater_skhid.exe seed <pipe>`; the updater
//! shares while the pipe is open. Entering a match, or the game's end,
//! closes it.
//!
//! On the pipe (the launcher's framing: a 16-bit length, the id, the
//! fields) the game sends its upload limit (`1E`, ignored: the cap is
//! fixed, `crate::upload`) and its stats switch (`22`, the console's
//! `lobby_torrent_stats`). It reads every message it gets as the stats'
//! tick, whatever its id, so the updater sends ticks (`18`) alone, and only
//! while the switch is on.

use alloc::string::String;

/// The game's pipe, before the game's process id.
pub const PIPE_PREFIX: &str = "\\\\.\\pipe\\skhid_seed-";

/// The pipe the game made: [`PIPE_PREFIX`] and 1 to 10 digits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GamePipe(String);

impl GamePipe {
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix(PIPE_PREFIX)?;
        (!digits.is_empty() && digits.len() <= 10 && digits.bytes().all(|c| c.is_ascii_digit()))
            .then(|| Self(text.into()))
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

/// `seed <pipe>`, as the game starts the updater: `None` for another
/// command; `Some(None)` for `seed` with anything but one pipe of the game.
pub fn command<S: AsRef<str>>(line: &[S]) -> Option<Option<GamePipe>> {
    match line {
        [_, action, rest @ ..] if action.as_ref() == "seed" => Some(match rest {
            [pipe] => GamePipe::parse(pipe.as_ref()),
            _ => None,
        }),
        _ => None,
    }
}

/// A message of the game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FromGame {
    /// `1E`: its upload limit (`change_seeding_upload_rate`): of no use.
    UploadLimit,
    /// `22`: the lobby's torrent stats shown (`FF`) or not (`00`).
    Stats(bool),
    Other(u8),
}

impl FromGame {
    /// One message's bytes (`crate::pipe::Reader::payload`: the id first).
    pub fn parse(payload: &[u8]) -> Option<Self> {
        Some(match payload {
            [] => return None,
            [0x1E, ..] => Self::UploadLimit,
            [0x22, on, ..] => Self::Stats(*on != 0),
            [id, ..] => Self::Other(*id),
        })
    }
}

/// What the game's stats row shows: this run's upload in bytes a second,
/// the cap, the peers connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    pub upload: u32,
    pub limit: u32,
    pub peers: u32,
}

/// `18`, as the game reads it (`torrent_client`'s pipe handler, 0x373BD0:
/// the bytes at +3, +4, +8 and +0Ch): seeding (`FF`), then the three.
pub fn frame(tick: Tick) -> [u8; 16] {
    let mut out = [0_u8; 16];
    out[..2].copy_from_slice(&14_u16.to_le_bytes());
    out[2] = 0x18;
    out[3] = 0xFF;
    out[4..8].copy_from_slice(&tick.upload.to_le_bytes());
    out[8..12].copy_from_slice(&tick.limit.to_le_bytes());
    out[12..].copy_from_slice(&tick.peers.to_le_bytes());
    out
}

/// A tick a second while the stats are shown.
pub const TICK_MS: u64 = 1000;

/// Whether the game shows the stats, and when the next tick is due.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stats {
    Hidden,
    Shown { due: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Game(FromGame),
    /// The clock, in milliseconds.
    Clock,
}

impl Stats {
    /// The one transition: the state after `event` at `now`, and whether a
    /// tick goes to the game now.
    pub fn step(self, now: u64, event: Event) -> (Self, bool) {
        match (self, event) {
            (Self::Hidden, Event::Game(FromGame::Stats(true))) => (Self::Shown { due: now + TICK_MS }, true),
            (_, Event::Game(FromGame::Stats(false))) => (Self::Hidden, false),
            (Self::Shown { due }, Event::Clock) if now >= due => (Self::Shown { due: now + TICK_MS }, true),
            (state, Event::Clock | Event::Game(_)) => (state, false),
        }
    }
}

/// The upload rate from the run's bytes sent, over the time since the last
/// measure.
#[derive(Debug, Clone, Copy)]
pub struct Meter {
    at: u64,
    sent: u64,
}

impl Meter {
    pub fn new(now: u64, sent: u64) -> Self {
        Self { at: now, sent }
    }

    /// Bytes a second since the last call (or the start).
    pub fn rate(&mut self, now: u64, sent: u64) -> u32 {
        let elapsed = now.saturating_sub(self.at).max(1);
        let bytes = sent.saturating_sub(self.sent);
        *self = Self { at: now, sent };
        u32::try_from(bytes.saturating_mul(1000) / elapsed).unwrap_or(u32::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_starts_it_with_its_pipe_only() {
        let pipe = r"\\.\pipe\skhid_seed-4242";
        assert_eq!(command(&["u.exe", "seed", pipe]), Some(GamePipe::parse(pipe)));
        assert_eq!(command(&["u.exe", "seed", pipe]).flatten().unwrap().name(), pipe);
        assert_eq!(command(&["u.exe", "check"]), None);
        for bad in [
            &["u.exe", "seed"][..],
            &["u.exe", "seed", pipe, "x"],
            &["u.exe", "seed", r"\\.\pipe\skhid_seed-"],
            &["u.exe", "seed", r"\\.\pipe\skhid_seed-12a"],
            &["u.exe", "seed", r"\\.\pipe\skhid_seed-12345678901"],
            &["u.exe", "seed", r"\\.\pipe\vostok_updater_pipe-Survarium"],
        ] {
            assert_eq!(command(bad), Some(None), "{bad:?}");
        }
    }

    #[test]
    fn the_games_messages() {
        assert_eq!(FromGame::parse(&[0x1E, 0, 0, 1, 0, 0, 0, 0, 0]), Some(FromGame::UploadLimit));
        assert_eq!(FromGame::parse(&[0x22, 0xFF]), Some(FromGame::Stats(true)));
        assert_eq!(FromGame::parse(&[0x22, 0]), Some(FromGame::Stats(false)));
        assert_eq!(FromGame::parse(&[0x22]), Some(FromGame::Other(0x22)));
        assert_eq!(FromGame::parse(&[7]), Some(FromGame::Other(7)));
        assert_eq!(FromGame::parse(&[]), None);
    }

    #[test]
    fn a_tick_is_what_the_game_copies() {
        let bytes = frame(Tick { upload: 0x0102_0304, limit: 1 << 20, peers: 3 });
        assert_eq!(&bytes[..4], [14, 0, 0x18, 0xFF]);
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 0x0102_0304);
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1 << 20);
        assert_eq!(u32::from_le_bytes(bytes[12..].try_into().unwrap()), 3);
    }

    #[test]
    fn ticks_only_while_the_stats_are_shown() {
        let state = Stats::Hidden;
        let (state, tick) = state.step(0, Event::Clock);
        assert!(!tick);
        let (state, tick) = state.step(0, Event::Game(FromGame::UploadLimit));
        assert!(!tick && state == Stats::Hidden);
        let (state, tick) = state.step(10, Event::Game(FromGame::Stats(true)));
        assert!(tick, "at once");
        let (state, tick) = state.step(500, Event::Clock);
        assert!(!tick);
        let (state, tick) = state.step(1010, Event::Clock);
        assert!(tick);
        // Shown again while shown: no extra tick.
        let (state, tick) = state.step(1020, Event::Game(FromGame::Stats(true)));
        assert!(!tick);
        let (state, tick) = state.step(1500, Event::Game(FromGame::Stats(false)));
        assert!(!tick && state == Stats::Hidden);
        let (_, tick) = state.step(5000, Event::Clock);
        assert!(!tick);
    }

    #[test]
    fn the_rate_is_of_the_last_interval() {
        let mut meter = Meter::new(0, 0);
        assert_eq!(meter.rate(1000, 500_000), 500_000);
        assert_eq!(meter.rate(3000, 1_500_000), 500_000);
        assert_eq!(meter.rate(3000, 1_500_000), 0);
    }
}

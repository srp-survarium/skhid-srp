//! Whether an account's matches may be watched live: the Settings window's
//! "Allow others to watch my matches live" (`docs/notes/replay-stream.md`,
//! "Live"). srp's own wire, as a lobby pick's (`lobby_pick`): the exe sends
//! `3B` at a reserved start when the box is applied and a stock game never
//! does. No answer: the state comes back in `0xA1`'s last srp byte
//! ([`super::social::squad_info`]), which is also how the exe learns it at
//! the sign-in.

/// `LIVE_WATCH_BASE + id` ([`LiveWatch::id`]): the asker's setting.
pub const LIVE_WATCH_BASE: u32 = 2_096_000_000;

/// An account's setting. Allowed is every account's until it says otherwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LiveWatch {
    /// Its matches are listed and may be watched live.
    #[default]
    Allowed,
    /// No match with it in it is listed, or may be watched live.
    Hidden,
}

impl LiveWatch {
    pub const ALL: [LiveWatch; 2] = [LiveWatch::Allowed, LiveWatch::Hidden];

    /// Its id on the wire and in the table (1 and 2; anything else is none).
    pub fn id(self) -> u8 {
        match self {
            LiveWatch::Allowed => 1,
            LiveWatch::Hidden => 2,
        }
    }

    /// The `3B` start that asks for it.
    pub fn start(self) -> u32 {
        LIVE_WATCH_BASE + u32::from(self.id())
    }

    /// The setting a reserved start asks for, if it is one.
    pub fn of_start(start: u32) -> Option<LiveWatch> {
        LiveWatch::ALL.into_iter().find(|state| start == state.start())
    }

    /// The setting of an id, none for any other.
    pub fn of_id(id: u8) -> Option<LiveWatch> {
        LiveWatch::ALL.into_iter().find(|state| state.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_is_its_start_and_its_id_and_nothing_else_is_one() {
        for state in LiveWatch::ALL {
            assert_eq!(LiveWatch::of_start(state.start()), Some(state));
            assert_eq!(LiveWatch::of_id(state.id()), Some(state));
        }
        for start in [LIVE_WATCH_BASE - 1, LIVE_WATCH_BASE, LIVE_WATCH_BASE + 3] {
            assert_eq!(LiveWatch::of_start(start), None);
        }
        for id in [0, 3, 255] {
            assert_eq!(LiveWatch::of_id(id), None);
        }
        assert_eq!(LiveWatch::default(), LiveWatch::Allowed);
        const { assert!(LIVE_WATCH_BASE > super::super::lobby_pick::LOBBY_PICK_BASE + 4) };
        const { assert!(LIVE_WATCH_BASE + 2 <= i32::MAX as u32) };
    }
}

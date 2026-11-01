//! The lobby scene a player picked (exe patch 81, `lobby-scenes.md`): srp's
//! own wire, on the ranking page's request (`3B`) at a reserved start, as
//! the presets are (`presets.rs`). The lobby movie's lobby window sends one
//! when a card is clicked, and the exe sends the saved pick at the sign-in;
//! a stock game never does. No answer: the pick comes back in `0xA1`'s srp
//! bytes ([`super::social::squad_info`]), the asker's own and its squad
//! commander's.

/// `LOBBY_PICK_BASE + id` ([`LobbyPick::id`]): the asker's pick.
pub const LOBBY_PICK_BASE: u32 = 2_095_000_000;

/// A lobby scene a player can pick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyPick {
    /// `lobby_scene/camp_069`, the default.
    Room,
    /// `lobby_scene/day`, the classic.
    Basement,
    /// `lobby_scene/stk2_bunker`.
    Bunker,
    /// `lobby_scene/hangar_046`.
    Hangar,
    /// `lobby_scene/stk2_house`.
    House,
    /// `lobby_scene/stk2_bar`.
    Range,
}

impl LobbyPick {
    pub const ALL: [LobbyPick; 6] = [
        LobbyPick::Room,
        LobbyPick::Basement,
        LobbyPick::Bunker,
        LobbyPick::Hangar,
        LobbyPick::House,
        LobbyPick::Range,
    ];

    /// Its id on the wire (1 to 6; 0 is none in `0xA1`).
    pub fn id(self) -> u8 {
        match self {
            LobbyPick::Room => 1,
            LobbyPick::Basement => 2,
            LobbyPick::Bunker => 3,
            LobbyPick::Hangar => 4,
            LobbyPick::House => 5,
            LobbyPick::Range => 6,
        }
    }

    /// The pick a reserved start asks for, if it is one.
    pub fn of_start(start: u32) -> Option<LobbyPick> {
        LobbyPick::ALL.into_iter().find(|pick| start == LOBBY_PICK_BASE + u32::from(pick.id()))
    }

    /// The pick of a wire id (none for 0 and any other).
    pub fn of_id(id: u8) -> Option<LobbyPick> {
        LobbyPick::ALL.into_iter().find(|pick| pick.id() == id)
    }

    /// `0xA1`'s byte of a pick or none.
    pub fn byte(pick: Option<LobbyPick>) -> u8 {
        pick.map_or(0, LobbyPick::id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pick_is_its_start_and_its_id() {
        for pick in LobbyPick::ALL {
            assert_eq!(LobbyPick::of_start(LOBBY_PICK_BASE + u32::from(pick.id())), Some(pick));
            assert_eq!(LobbyPick::of_id(pick.id()), Some(pick));
        }
        assert_eq!(LobbyPick::ALL.map(LobbyPick::id), [1, 2, 3, 4, 5, 6]);
        for start in [LOBBY_PICK_BASE, LOBBY_PICK_BASE + 7, LOBBY_PICK_BASE - 1] {
            assert_eq!(LobbyPick::of_start(start), None);
        }
        assert_eq!(LobbyPick::of_id(0), None);
        assert_eq!(LobbyPick::byte(None), 0);
        const { assert!(LOBBY_PICK_BASE > super::super::presets::PRESETS_LAST) };
    }
}

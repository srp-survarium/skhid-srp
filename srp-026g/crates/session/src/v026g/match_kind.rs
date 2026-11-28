//! Who a match is for: set once, where the lobby makes it, and carried with
//! it to the match server, its live view (`GET /api/stats`) and its result.

/// The kind of a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchKind {
    /// Players against players, paid as usual: Play's matchmaking (its
    /// public rooms too, which have no PIN) or a clan match. The only kind
    /// a release waits for with `release.sh --end-bot-and-private-matches`.
    Normal,
    /// Player vs Bots (the lobby's `pvb`): against the server's bots, paid as
    /// against bots; its result says so (`MatchResult::vs_bots`).
    VsBots,
    /// A room with a PIN, made without bots: it pays nothing
    /// (`MatchResult::private`).
    PrivateRoom,
    /// A room with a PIN made with bots (`rooms.md`): paid as against bots
    /// (the owner's of 2026-11-05: "this should still give bot rewards"),
    /// as [`MatchKind::VsBots`].
    PrivateRoomWithBots,
}

impl MatchKind {
    /// Whether its result is paid as against bots.
    pub fn vs_bots(self) -> bool {
        matches!(self, Self::VsBots | Self::PrivateRoomWithBots)
    }

    /// Whether its result pays nothing.
    pub fn private(self) -> bool {
        self == Self::PrivateRoom
    }

    /// Whether it is a room's with a PIN, with bots or without: its players'
    /// games may fill the collision view (`collision-view.md`; the static
    /// info's private room entry).
    pub fn pin_room(self) -> bool {
        matches!(self, Self::PrivateRoom | Self::PrivateRoomWithBots)
    }
}

#[cfg(test)]
mod tests {
    use super::MatchKind;

    #[test]
    fn paid_and_private_by_kind() {
        let of = |kind: MatchKind| (kind.vs_bots(), kind.private(), kind.pin_room());
        assert_eq!(of(MatchKind::Normal), (false, false, false));
        assert_eq!(of(MatchKind::VsBots), (true, false, false));
        assert_eq!(of(MatchKind::PrivateRoom), (false, true, true));
        assert_eq!(of(MatchKind::PrivateRoomWithBots), (true, false, true), "paid as against bots, a PIN room");
    }
}

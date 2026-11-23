//! Who a match is for: set once, where the lobby makes it, and carried with
//! it to the match server, its live view (`GET /api/stats`) and its result.

/// The kind of a match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchKind {
    /// Players against players, paid as usual: Play's matchmaking (its
    /// public rooms too, which have no PIN) or a clan match. The only kind
    /// a release waits for with `release.sh --end-bot-and-private-matches`.
    Normal,
    /// Against the server's bots: Player vs Bots (the lobby's `pvb`), or a
    /// room with a PIN made with bots (`rooms.md`). Paid as against bots;
    /// its result says so (`MatchResult::vs_bots`).
    VsBots,
    /// A room with a PIN, made without bots: it pays nothing
    /// (`MatchResult::private`).
    PrivateRoom,
}

impl MatchKind {
    /// Whether its result is paid as against bots.
    pub fn vs_bots(self) -> bool {
        self == Self::VsBots
    }

    /// Whether its result pays nothing.
    pub fn private(self) -> bool {
        self == Self::PrivateRoom
    }
}

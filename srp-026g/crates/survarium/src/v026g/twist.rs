//! A free-for-all's twists (`docs/notes/ffa.md`, "Twists"): a match of a free-for-all map played
//! with every kit's guns changed by the server, the map and its rules as they are. A twist is no
//! map of its own (the owner, 2026-11-14: "we will add more maps in the future": the match
//! options' map id is one byte, and a mode's own copies of the 33 free-for-all maps would take a
//! third of what is left); the rooms' vote draws one for its free-for-all cards
//! (`session::v026g::rooms`), and the lobby names it by its own mode number.

use super::dictionary::Dictionary;
use super::maps::{GameMode, MapMode, MatchMap};
use super::match_msgs::static_info::MatchProfile;

/// The shares of a free-for-all card's draw ([`Twist::draw`]): the plain mode's and each twist's,
/// in hundredths. Cowboy rare (the owner, 2026-11-14: "cowboy version should be rare with only
/// 10% chance"), Racket as often as the plain mode (my pick).
pub const SHARES: [(Option<Twist>, u32); 3] =
    [(None, 45), (Some(Twist::Racket), 45), (Some(Twist::Cowboy), 10)];

/// The sum of [`SHARES`]: what a draw's roll is below.
pub const SHARE_TOTAL: u32 = {
    let mut total = 0;
    let mut at = 0;
    while at < SHARES.len() {
        total += SHARES[at].1;
        at += 1;
    }
    total
};

/// What a twist does to the kits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Twist {
    /// Every sniper rifle and silenced gun an AK-12 ([`super::racket`]).
    Racket,
    /// Every kit a Colt Python alone ([`super::cowboy`]); "Showdown" in the lobby.
    Cowboy,
}

impl Twist {
    /// Every twist.
    pub const ALL: [Self; 2] = [Self::Racket, Self::Cowboy];

    /// Its number in the lobby: the rooms' cards, rows and banners, the matches' list. After every
    /// mode's ([`GameMode::lobby_number`]: 7 is Scarcity's, 8 Free for All's).
    pub const fn lobby_number(self) -> u8 {
        match self {
            Self::Racket => 9,
            Self::Cowboy => 10,
        }
    }

    /// What a free-for-all card of the rooms' vote plays, for a roll below [`SHARES`]' sum: each
    /// twist, and none, for its share.
    pub fn draw(roll: u32) -> Option<Self> {
        let mut roll = roll % SHARE_TOTAL;
        for (twist, share) in SHARES {
            if roll < share {
                return twist;
            }
            roll -= share;
        }
        unreachable!("a roll is below the shares' sum")
    }

    /// The twist of a lobby number.
    pub fn of_lobby_number(number: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|twist| twist.lobby_number() == number)
    }

    /// `profile`'s kits as the twist arms them.
    pub fn arm(self, profile: &mut MatchProfile, dictionary: &Dictionary) {
        match self {
            Self::Racket => super::racket::arm(profile, dictionary),
            Self::Cowboy => super::cowboy::arm(profile, &super::cowboy::Revolver::shared()),
        }
    }
}

/// What a match plays: a map, and on a free-for-all's a twist or none. Made only by
/// [`Self::plain`] and [`Self::twisted`], so a twist is never on another mode's map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Played {
    map: MatchMap,
    twist: Option<Twist>,
}

impl Played {
    /// `map` as it is.
    pub fn plain(map: MatchMap) -> Self {
        Self { map, twist: None }
    }

    /// `map` with `twist`, if it is a free-for-all's.
    pub fn twisted(map: MatchMap, twist: Twist) -> Option<Self> {
        (map.mode() == GameMode::FreeForAll).then_some(Self {
            map,
            twist: Some(twist),
        })
    }

    /// The map a match can be played on.
    pub fn match_map(self) -> MatchMap {
        self.map
    }

    /// The map played: what the client loads.
    pub fn map(self) -> &'static MapMode {
        self.map.map()
    }

    /// The map's mode: what its rules, its pay and its results are.
    pub fn mode(self) -> GameMode {
        self.map.mode()
    }

    pub fn twist(self) -> Option<Twist> {
        self.twist
    }

    /// Its number in the lobby: the twist's, else the mode's.
    pub fn lobby_number(self) -> u8 {
        self.twist
            .map_or_else(|| self.mode().lobby_number(), Twist::lobby_number)
    }

    /// `profile`'s kits for the match: the twist's arms, or as they are.
    pub fn arm(self, profile: &mut MatchProfile, dictionary: &Dictionary) {
        if let Some(twist) = self.twist {
            twist.arm(profile, dictionary);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::maps::{self, playable_in};
    use super::*;

    #[test]
    fn a_twist_is_a_free_for_alls_alone() {
        for map in maps::playable() {
            let played = MatchMap::of(map.id).unwrap();
            let twisted = Played::twisted(played, Twist::Racket);
            assert_eq!(twisted.is_some(), map.mode == GameMode::FreeForAll, "{}", map.id);
            assert_eq!(Played::plain(played).lobby_number(), map.mode.lobby_number());
        }
        let ffa = playable_in(GameMode::FreeForAll).next().unwrap();
        let racket = Played::twisted(MatchMap::of(ffa.id).unwrap(), Twist::Racket).unwrap();
        assert_eq!((racket.lobby_number(), racket.mode()), (9, GameMode::FreeForAll));
        // Every roll below the shares' sum: each its share.
        let drawn = |twist| (0..SHARE_TOTAL).filter(|&roll| Twist::draw(roll) == twist).count();
        for (twist, share) in SHARES {
            assert_eq!(drawn(twist), share as usize);
        }
        // No twist shares a number with a mode (nor with Scarcity's 7, not on main).
        for twist in Twist::ALL {
            assert_eq!(GameMode::of_lobby_number(twist.lobby_number()), None);
            assert_ne!(twist.lobby_number(), 7);
            assert_eq!(Twist::of_lobby_number(twist.lobby_number()), Some(twist));
        }
    }
}

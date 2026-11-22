//! A free-for-all's twists (`docs/notes/ffa.md`, "Twists"): a match of a free-for-all map played
//! with every kit's guns changed by the server, the map and its rules as they are. A twist is no
//! map of its own (the owner, 2026-11-14: "we will add more maps in the future": the match
//! options' map id is one byte, and a mode's own copies of the 33 free-for-all maps would take a
//! third of what is left); the rooms' vote draws one for its free-for-all cards
//! (`session::v026g::rooms`), and the lobby names it by its own mode number.

use super::cowboy::Variation;
use super::maps::{GameMode, MapMode, MatchMap};
use super::match_msgs::static_info::MatchProfile;
use super::racket::Gun;
use super::roulette::{Band, Hand};

/// What a free-for-all card of a server's room plays, as its share draws it ([`draw`]): the plain
/// mode or a twist, before it is armed for the room ([`deal`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pick {
    Plain,
    Racket,
    Cowboy(Variation),
    Roulette,
}

/// The shares of a free-for-all card's draw ([`draw`]), in hundredths. Roulette 10 from the plain
/// mode (the owner, 2026-11-22, approving my pick). Cowboy 10 in all since 2026-11-22 (20 from
/// 2026-11-15:
/// "let's make saloon 20% with two variations (no need to name them)"; was 10, "cowboy version
/// should be rare with only 10% chance", 2026-11-14), in two variations (the owner, 2026-11-14:
/// "saloon should have two variations with winchesters and colts or just with colts"), the
/// Winchester and Colt 5 (the owner, 2026-11-22: "winchester should become 5% again on FFA";
/// 15 from 2026-11-15) and the Colt alone 5; Racket 40. Racket's gun is its room's
/// (`racket`, `Gun::of_room`), Roulette's band one its room allows (`roulette::Band::draw`).
pub const SHARES: [(Pick, u32); 5] = [
    (Pick::Plain, 40),
    (Pick::Racket, 40),
    (Pick::Roulette, 10),
    (Pick::Cowboy(Variation::WinchesterAndColt), 5),
    (Pick::Cowboy(Variation::Colt), 5),
];

/// The sum of the [`SHARES`]: what a draw's roll is below.
pub const SHARE_TOTAL: u32 = {
    let mut total = 0;
    let mut at = 0;
    while at < SHARES.len() {
        total += SHARES[at].1;
        at += 1;
    }
    total
};

/// What a free-for-all card of a server's room plays, for a roll below [`SHARE_TOTAL`]. A private
/// room's never draws: its free-for-all is plain (`session::v026g::rooms`).
pub fn draw(roll: u32) -> Pick {
    let mut roll = roll % SHARE_TOTAL;
    for (pick, share) in SHARES {
        if roll < share {
            return pick;
        }
        roll -= share;
    }
    unreachable!("a roll is below the shares' sum")
}

/// The twist a free-for-all card of a room of `max_level` plays for `roll` ([`draw`]), armed:
/// Racket with the room's gun, Roulette with a band the room allows and a hand of it, dealt with
/// `below`'s rolls; none for the plain mode, or a Roulette in a room that allows no band.
pub fn deal(roll: u32, max_level: u8, below: &mut impl FnMut(usize) -> usize) -> Option<Armed> {
    match draw(roll) {
        Pick::Plain => None,
        Pick::Racket => Some(Armed::Racket(Gun::of_room(max_level))),
        Pick::Cowboy(variation) => Some(Armed::Cowboy(variation)),
        Pick::Roulette => {
            Band::draw(max_level, below).map(|band| Armed::Roulette(Hand::deal(band, below)))
        }
    }
}

/// The lives a twist's ammo lasts: its stack's store is a kit's rounds this many times. The game
/// takes each slot's count out of the store at every spawn, a kit change in the countdown too, and
/// a dead player keeps only what is left in it (`snapshot_rules.md` §7.3), so a kit's worth alone
/// would last one life. More than any match has: 15:00 with a 5 s respawn is 180.
pub const STORE_LIVES: u32 = 1_000;

/// What a twist does to the kits. What a match of it arms, and its mode in the lobby, is an
/// [`Armed`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Twist {
    /// Every sniper rifle and silenced gun an AK-12, or an AK-74N in a level-10 room
    /// ([`super::racket`]).
    Racket,
    /// Every kit a Winchester 1894 and a Colt Python, or a Colt Python alone
    /// ([`super::cowboy`]); "Saloon (Winchester)" and "Saloon" in the lobby.
    Cowboy,
    /// Every kit one of a hand of three, the same for every player, of a level band its room
    /// allows ([`super::roulette`]); "Roulette 0-5", "6-10" or "11-15" in the lobby.
    Roulette,
}

/// A twist as its match arms the kits: Cowboy's with its variation, Roulette's with its hand,
/// drawn with the card ([`deal`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Armed {
    /// [`Twist::Racket`] with its room's gun.
    Racket(Gun),
    /// [`Twist::Cowboy`] in one of its variations.
    Cowboy(Variation),
    /// [`Twist::Roulette`] with its hand.
    Roulette(Hand),
}

impl Armed {
    /// The twist it is of.
    pub const fn twist(self) -> Twist {
        match self {
            Self::Racket(_) => Twist::Racket,
            Self::Cowboy(_) => Twist::Cowboy,
            Self::Roulette(_) => Twist::Roulette,
        }
    }

    /// Its number in the lobby ([`LobbyMode::number`]).
    pub const fn lobby_number(self) -> u8 {
        LobbyMode::of(GameMode::FreeForAll, Some(self)).number()
    }

    /// The stacks it gives, which its match never charges (`session::v026g::autobuy`): nobody's,
    /// though an account may own the same kind. Roulette's: every stack of its hand.
    pub fn gives(self) -> Vec<u16> {
        match self {
            Self::Racket(gun) => vec![super::racket::Arms::shared(gun).ammo],
            Self::Cowboy(variation) => super::cowboy::Guns::shared().ammo(variation),
            Self::Roulette(hand) => hand.stacks(),
        }
    }

    /// `profile`'s kits as it arms them.
    pub fn arm(self, profile: &mut MatchProfile) {
        match self {
            Self::Racket(gun) => super::racket::arm(profile, &super::racket::Arms::shared(gun)),
            Self::Cowboy(variation) => {
                super::cowboy::arm(profile, &super::cowboy::Guns::shared(), variation)
            }
            Self::Roulette(hand) => super::roulette::arm(profile, &hand),
        }
    }
}

/// What the lobby calls the mode of a match: a mode's own, or a free-for-all's twist as it arms
/// (the rooms' cards, rows and banners, the matches' list, the history).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LobbyMode {
    Plain(GameMode),
    Racket,
    Cowboy(Variation),
    Roulette(Band),
}

impl LobbyMode {
    /// Every one.
    pub const ALL: [Self; 14] = [
        Self::Plain(GameMode::Batteries),
        Self::Plain(GameMode::TimedBatteries),
        Self::Plain(GameMode::TeamDeathmatch),
        Self::Plain(GameMode::Research),
        Self::Plain(GameMode::FreeForAll),
        Self::Plain(GameMode::Blackout),
        Self::Plain(GameMode::FreezeTag),
        Self::Plain(GameMode::ArtefactHunt),
        Self::Racket,
        Self::Cowboy(Variation::Colt),
        Self::Cowboy(Variation::WinchesterAndColt),
        Self::Roulette(Band::Low),
        Self::Roulette(Band::Mid),
        Self::Roulette(Band::High),
    ];

    /// A match of `mode` with `twist`.
    pub const fn of(mode: GameMode, twist: Option<Armed>) -> Self {
        match twist {
            None => Self::Plain(mode),
            Some(Armed::Racket(_)) => Self::Racket,
            Some(Armed::Cowboy(variation)) => Self::Cowboy(variation),
            Some(Armed::Roulette(hand)) => Self::Roulette(hand.band()),
        }
    }

    /// Its number in the lobby. A twist's is after every mode's ([`GameMode::lobby_number`]: 7 is
    /// Scarcity's, 8 Free for All's); Saloon's by its variation (the owner, 2026-11-15: "make it
    /// visible in names", "so people know what they get and can vote based on this"); Roulette's by
    /// its band, after Freeze Tag's 12 and Artifact Hunt's 13.
    pub const fn number(self) -> u8 {
        match self {
            Self::Plain(mode) => mode.lobby_number(),
            Self::Racket => 9,
            Self::Cowboy(Variation::Colt) => 10,
            Self::Cowboy(Variation::WinchesterAndColt) => 11,
            Self::Roulette(Band::Low) => 14,
            Self::Roulette(Band::Mid) => 15,
            Self::Roulette(Band::High) => 16,
        }
    }

    /// The one of a lobby number.
    pub fn of_number(number: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.number() == number)
    }

    /// The mode whose map and rules it plays: a twist's is the free-for-all.
    pub const fn mode(self) -> GameMode {
        match self {
            Self::Plain(mode) => mode,
            Self::Racket | Self::Cowboy(_) | Self::Roulette(_) => GameMode::FreeForAll,
        }
    }

    /// The key of its name in the game's texts: the one the rooms name it by
    /// (`RoomsText.as`, `MODES`; a test holds the two together).
    pub const fn name_key(self) -> &'static str {
        match self {
            Self::Plain(GameMode::Batteries) => "st_srp_mode_protective_device",
            Self::Plain(GameMode::TimedBatteries) => "st_srp_mode_chargers",
            Self::Plain(GameMode::TeamDeathmatch) => "st_srp_mode_team_deathmatch",
            Self::Plain(GameMode::Research) => "st_srp_mode_research",
            Self::Plain(GameMode::Blackout) => "st_srp_mode_blackout",
            Self::Plain(GameMode::FreeForAll) => "st_srp_mode_free_for_all",
            Self::Plain(GameMode::FreezeTag) => "st_srp_mode_freeze_tag",
            Self::Plain(GameMode::ArtefactHunt) => "st_srp_mode_artefact_hunt",
            Self::Racket => "st_srp_mode_racket",
            Self::Cowboy(Variation::Colt) => "st_srp_mode_cowboy",
            Self::Cowboy(Variation::WinchesterAndColt) => "st_srp_mode_cowboy_winchester",
            Self::Roulette(Band::Low) => "st_srp_mode_roulette_low",
            Self::Roulette(Band::Mid) => "st_srp_mode_roulette_mid",
            Self::Roulette(Band::High) => "st_srp_mode_roulette_high",
        }
    }
}

/// What a match plays: a map, and on a free-for-all's a twist or none. Made only by
/// [`Self::plain`] and [`Self::twisted`], so a twist is never on another mode's map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Played {
    map: MatchMap,
    twist: Option<Armed>,
}

impl Played {
    /// `map` as it is.
    pub fn plain(map: MatchMap) -> Self {
        Self { map, twist: None }
    }

    /// `map` with `twist`, if it is a free-for-all's.
    pub fn twisted(map: MatchMap, twist: Armed) -> Option<Self> {
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
        self.twist.map(Armed::twist)
    }

    /// The twist as it arms the kits: Cowboy's with its variation.
    pub fn armed(self) -> Option<Armed> {
        self.twist
    }

    /// Its number in the lobby: the twist's, else the mode's.
    pub fn lobby_number(self) -> u8 {
        self.twist
            .map_or_else(|| self.mode().lobby_number(), Armed::lobby_number)
    }

    /// `profile`'s kits for the match: the twist's arms, or as they are.
    pub fn arm(self, profile: &mut MatchProfile) {
        if let Some(twist) = self.twist {
            twist.arm(profile);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::maps::{self, playable_in};
    use super::*;

    /// The rooms name each mode by the same key (`RoomsText.as`, `MODES`): the site's names and
    /// the game's cannot part.
    #[test]
    fn the_lobby_modes_are_named_as_the_rooms_name_them() {
        let rooms = include_str!("../../../../flash/lobby_menu/src/survarium/rooms/RoomsText.as");
        let modes = rooms.split_once("MODES:Object = {").unwrap().1;
        let modes = modes.split_once("};").unwrap().0;
        let mut named: Vec<(u8, &str)> = modes
            .lines()
            .filter_map(|line| line.trim().strip_prefix('"'))
            .map(|line| {
                let (number, key) = line.split_once("\": \"").unwrap();
                (number.parse().unwrap(), key.trim_end_matches(',').trim_end_matches('"'))
            })
            .collect();
        named.sort_unstable();
        let mut own: Vec<(u8, &str)> = LobbyMode::ALL
            .iter()
            .map(|mode| (mode.number(), mode.name_key()))
            .collect();
        own.sort_unstable();
        assert_eq!(own, named);
        for mode in LobbyMode::ALL {
            assert_eq!(LobbyMode::of_number(mode.number()), Some(mode));
        }
        assert_eq!(LobbyMode::of_number(7), None);
    }

    #[test]
    fn a_twist_is_a_free_for_alls_alone() {
        for map in maps::playable() {
            let played = MatchMap::of(map.id).unwrap();
            let twisted = Played::twisted(played, Armed::Racket(Gun::Ak12));
            assert_eq!(twisted.is_some(), map.mode == GameMode::FreeForAll, "{}", map.id);
            assert_eq!(Played::plain(played).lobby_number(), map.mode.lobby_number());
        }
        let ffa = playable_in(GameMode::FreeForAll).next().unwrap();
        let racket = Armed::Racket(Gun::Ak12);
        let racket = Played::twisted(MatchMap::of(ffa.id).unwrap(), racket).unwrap();
        assert_eq!((racket.lobby_number(), racket.mode()), (9, GameMode::FreeForAll));
        // Every roll below the shares' sum: each its share, 40 / 40 / 10 / 5 / 5.
        let drawn = |pick| (0..SHARE_TOTAL).filter(|&roll| draw(roll) == pick).count();
        assert_eq!(SHARE_TOTAL, 100);
        for (pick, share) in SHARES {
            assert_eq!(drawn(pick), share as usize);
        }
        // Armed for its room: Racket with the room's gun, Roulette with a band the room allows,
        // none in a room under the low band's top.
        let mut zero = |_: usize| 0;
        let roll = |wanted: Pick| (0..SHARE_TOTAL).find(|&roll| draw(roll) == wanted).unwrap();
        let racket = deal(roll(Pick::Racket), 10, &mut zero);
        assert_eq!(racket, Some(Armed::Racket(Gun::Ak74n)));
        assert_eq!(deal(roll(Pick::Plain), 15, &mut zero), None);
        assert_eq!(deal(roll(Pick::Roulette), 4, &mut zero), None);
        let roulette = deal(roll(Pick::Roulette), 15, &mut zero).unwrap();
        assert_eq!(roulette.lobby_number(), 14);
        // Each twist as it arms its own number; none a mode's (nor Scarcity's 7, not on main).
        for number in [9, 10, 11, 14, 15, 16] {
            let mode = LobbyMode::of_number(number).unwrap();
            assert_eq!(mode.mode(), GameMode::FreeForAll, "{number}");
            assert_eq!(GameMode::of_lobby_number(number), None);
        }
    }
}

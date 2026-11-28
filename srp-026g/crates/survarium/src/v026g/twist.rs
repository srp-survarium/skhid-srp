//! A free-for-all's twists (`docs/notes/ffa.md`, "Twists"): a match of a free-for-all map played
//! with every kit's guns changed by the server, the map and its rules as they are. A twist is no
//! map of its own (the owner, 2026-11-14: "we will add more maps in the future": the match
//! options' map id is one byte, and a mode's own copies of the 33 free-for-all maps would take a
//! third of what is left); the rooms' vote draws one for its free-for-all cards
//! (`session::v026g::rooms`), and the lobby names it by its own mode number.

use binary_config_parser::tree::ConfigValue;

use super::cowboy::Variation;
use super::maps::{GameMode, MapMode, MatchMap};
use super::match_msgs::static_info::MatchProfile;
use super::pistols::{self, Pair};
use super::racket::Gun;
use super::roulette::{Band, Hand};

/// What a free-for-all card of a server's room plays, as its share draws it ([`draw`]): the plain
/// mode or a twist, armed for the room by [`draw`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pick {
    Plain,
    Racket,
    Cowboy(Variation),
    Roulette,
    Pistols,
}

impl Pick {
    /// The twist it plays, if any.
    const fn twist(self) -> Option<Twist> {
        match self {
            Self::Plain => None,
            Self::Racket => Some(Twist::Racket),
            Self::Cowboy(_) => Some(Twist::Cowboy),
            Self::Roulette => Some(Twist::Roulette),
            Self::Pistols => Some(Twist::Pistols),
        }
    }
}

/// What an open room's free-for-all card on a map plays ([`MapMode::open_play`], its config's
/// `mode.srp_twists_only`). A private room's plays the plain mode on every map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OpenPlay {
    /// The plain mode or a twist, by [`SHARES`] (no `srp_twists_only`, or `false`).
    Any,
    /// A twist alone, by their shares (`true`: Military Warehouses).
    Twisted,
    /// This twist alone (its name: Station's `"racket"`). Only a twist every room can play: one
    /// whose card a room might not deal (Roulette's and Pistols' band) would leave such a card
    /// nothing.
    Only(Twist),
}

impl OpenPlay {
    /// A config's `srp_twists_only`: a bool, or the name of the one twist an open room plays.
    pub fn of_config(value: &ConfigValue) -> Result<Self, String> {
        if let Some(only) = value.as_bool() {
            return Ok(if only { Self::Twisted } else { Self::Any });
        }
        match value.as_str() {
            Some("racket") => Ok(Self::Only(Twist::Racket)),
            Some("saloon") => Ok(Self::Only(Twist::Cowboy)),
            Some("roulette") => {
                Err("`srp_twists_only` \"roulette\": a room that allows no band has no Roulette".into())
            }
            Some("pistols") => {
                Err("`srp_twists_only` \"pistols\": a room that allows no band has no Pistols".into())
            }
            Some(other) => Err(format!("`srp_twists_only` {other:?} is no twist (racket, saloon)")),
            None => Err("`srp_twists_only` is neither a bool nor a twist's name".into()),
        }
    }

    /// Whether an open room's card on the map may play `pick`.
    const fn allows(self, pick: Pick) -> bool {
        match (self, pick.twist()) {
            (Self::Any, _) => true,
            (Self::Twisted | Self::Only(_), None) => false,
            (Self::Twisted, Some(_)) => true,
            (Self::Only(only), Some(twist)) => only as u8 == twist as u8,
        }
    }
}

/// The shares of a free-for-all card's draw ([`draw`]), in hundredths (the owner, 2026-11-27, of
/// Pistols: "it should be 10% of FFA cards", "take 5% from roulette and 5% from FFA normal"):
/// plain 25, Racket 30, Roulette 25, Pistols 10, Saloon (Winchester) 5, Saloon 5 (the Colt alone).
/// Before: plain 30, Racket 30, Roulette 30 (2026-11-22); plain 40, Racket 40, Roulette 10 (my
/// pick, the same day); plain 50 and Racket 40 without Roulette; the Winchester 15 from 2026-11-15
/// ("winchester should become 5% again on FFA"), Saloon 10 at first ("cowboy version should be rare
/// with only 10% chance"). Racket's gun is its room's (`racket`, `Gun::of_room`), Roulette's band
/// one its room allows (`roulette::Band::draw`), Pistols' 0-5 or 6-10 alone
/// (`pistols::Band::draw`).
pub const SHARES: [(Pick, u32); 6] = [
    (Pick::Plain, 25),
    (Pick::Racket, 30),
    (Pick::Roulette, 25),
    (Pick::Pistols, 10),
    (Pick::Cowboy(Variation::WinchesterAndColt), 5),
    (Pick::Cowboy(Variation::Colt), 5),
];

/// What a free-for-all card of a server's room of `max_level` plays on `map`, drawn after it: the
/// plain mode and each twist by its [`SHARES`], armed for the room (Racket with the room's gun,
/// `racket::Gun::of_room`; Roulette with a band the room allows and a hand of it, Pistols with a
/// band of its own two the room allows and a pair of it, dealt with the same rolls). On a map
/// played with a twist only ([`MapMode::open_play`]) the twists alone, by their shares (the plain
/// mode's taken out, so the map comes as often as any, always twisted), or its one twist; Roulette
/// and Pistols only in a room that allows one of their bands ([`Band::allowed`],
/// [`pistols::Band::allowed`]). `below(n)` is a roll below `n`.
/// A private room's never draws: its free-for-all is plain, and its pool has no such map
/// (`session::v026g::rooms`).
pub fn draw(map: MatchMap, max_level: u8, below: &mut impl FnMut(usize) -> usize) -> Played {
    let banded = Band::allowed(max_level).next().is_some();
    let open = map.map().open_play;
    let drawn = SHARES.iter().filter(|&&(pick, _)| {
        open.allows(pick)
            && match pick {
                Pick::Roulette => banded,
                Pick::Pistols => pistols::Band::allowed(max_level).next().is_some(),
                Pick::Plain | Pick::Racket | Pick::Cowboy(_) => true,
            }
    });
    let total: u32 = drawn.clone().map(|(_, share)| share).sum();
    let mut roll = below(total as usize) as u32 % total;
    let mut picked = Pick::Plain;
    for &(pick, share) in drawn {
        if roll < share {
            picked = pick;
            break;
        }
        roll -= share;
    }
    let armed = match picked {
        Pick::Plain => None,
        Pick::Racket => Some(Armed::Racket(Gun::of_room(max_level))),
        Pick::Cowboy(variation) => Some(Armed::Cowboy(variation)),
        Pick::Roulette => {
            Band::draw(max_level, below).map(|band| Armed::Roulette(Hand::deal(band, below)))
        }
        Pick::Pistols => {
            pistols::Band::draw(max_level, below).map(|band| Armed::Pistols(Pair::deal(band, below)))
        }
    };
    armed
        .and_then(|twist| Played::twisted(map, twist))
        .unwrap_or(Played::plain(map))
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
    /// allows ([`super::roulette`]); "FFA: Roulette" in the lobby, whatever the band.
    Roulette,
    /// Every kit one of a pair of two, each a pistol, the same for every player, of a level band
    /// (0-5 or 6-10) its room allows, the third kit not loaded ([`super::pistols`]); "FFA:
    /// Pistols" in the lobby.
    Pistols,
}

/// A twist as its match arms the kits: Cowboy's with its variation, Roulette's with its hand,
/// Pistols' with its pair, drawn with the card ([`draw`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Armed {
    /// [`Twist::Racket`] with its room's gun.
    Racket(Gun),
    /// [`Twist::Cowboy`] in one of its variations.
    Cowboy(Variation),
    /// [`Twist::Roulette`] with its hand.
    Roulette(Hand),
    /// [`Twist::Pistols`] with its pair.
    Pistols(Pair),
}

impl Armed {
    /// The twist it is of.
    pub const fn twist(self) -> Twist {
        match self {
            Self::Racket(_) => Twist::Racket,
            Self::Cowboy(_) => Twist::Cowboy,
            Self::Roulette(_) => Twist::Roulette,
            Self::Pistols(_) => Twist::Pistols,
        }
    }

    /// Whether the account's own items are in its match: Racket's and Saloon's kits are the
    /// account's with their guns swapped; Roulette's and Pistols' hold nothing of it (none of its
    /// stacks is used, its items pay no bonus: `session::v026g::autobuy`, `rewards`).
    pub const fn keeps_own_items(self) -> bool {
        match self {
            Self::Racket(_) | Self::Cowboy(_) => true,
            Self::Roulette(_) | Self::Pistols(_) => false,
        }
    }

    /// Its number in the lobby ([`LobbyMode::number`]).
    pub const fn lobby_number(self) -> u8 {
        LobbyMode::of(GameMode::FreeForAll, Some(self)).number()
    }

    /// The stacks it gives, which its match never charges (`session::v026g::autobuy`): nobody's,
    /// though an account may own the same kind. Roulette's: every stack of its hand; Pistols':
    /// of its pair.
    pub fn gives(self) -> Vec<u16> {
        match self {
            Self::Racket(gun) => vec![super::racket::Arms::shared(gun).ammo],
            Self::Cowboy(variation) => super::cowboy::Guns::shared().ammo(variation),
            Self::Roulette(hand) => hand.stacks(),
            Self::Pistols(pair) => pair.stacks(),
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
            Self::Pistols(pair) => super::pistols::arm(profile, &pair),
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
    Pistols,
}

impl LobbyMode {
    /// Every one.
    pub const ALL: [Self; 15] = [
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
        Self::Pistols,
    ];

    /// A match of `mode` with `twist`.
    pub const fn of(mode: GameMode, twist: Option<Armed>) -> Self {
        match twist {
            None => Self::Plain(mode),
            Some(Armed::Racket(_)) => Self::Racket,
            Some(Armed::Cowboy(variation)) => Self::Cowboy(variation),
            Some(Armed::Roulette(hand)) => Self::Roulette(hand.band()),
            Some(Armed::Pistols(_)) => Self::Pistols,
        }
    }

    /// Its number in the lobby. A twist's is after every mode's ([`GameMode::lobby_number`]: 7 is
    /// Scarcity's, 8 Free for All's); Saloon's by its variation (the owner, 2026-11-15: "make it
    /// visible in names", "so people know what they get and can vote based on this"); Roulette's by
    /// its band, after Freeze Tag's 12 and Artifact Hunt's 13; Pistols' one, whatever its band (as
    /// Roulette's name is one: the owner, 2026-11-22, "roulette shouldn't show its level"), 18: 17
    /// is the unmerged Instagib twist's (branch `feat/ffa-instagib`), kept apart so neither moves.
    pub const fn number(self) -> u8 {
        match self {
            Self::Plain(mode) => mode.lobby_number(),
            Self::Racket => 9,
            Self::Cowboy(Variation::Colt) => 10,
            Self::Cowboy(Variation::WinchesterAndColt) => 11,
            Self::Roulette(Band::Low) => 14,
            Self::Roulette(Band::Mid) => 15,
            Self::Roulette(Band::High) => 16,
            Self::Pistols => 18,
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
            Self::Racket | Self::Cowboy(_) | Self::Roulette(_) | Self::Pistols => GameMode::FreeForAll,
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
            Self::Pistols => "st_srp_mode_pistols",
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
        let ffa = playable_in(GameMode::FreeForAll).find(|map| map.open_play == OpenPlay::Any).unwrap();
        let racket = Armed::Racket(Gun::Ak12);
        let racket = Played::twisted(MatchMap::of(ffa.id).unwrap(), racket).unwrap();
        assert_eq!((racket.lobby_number(), racket.mode()), (9, GameMode::FreeForAll));
        // Every first roll below the shares' sum, the later ones (a Roulette's band and hand) 0:
        // what each card plays, by its lobby number.
        let rolls = |map: MatchMap, max_level: u8| {
            let mut total = 0;
            draw(map, max_level, &mut |below| {
                total = total.max(below);
                0
            });
            (0..total).map(move |roll| {
                let mut first = Some(roll);
                draw(map, max_level, &mut |_| first.take().unwrap_or(0)).lobby_number()
            })
        };
        let plain = MatchMap::of(ffa.id).unwrap();
        let count = |map, max_level, number| rolls(map, max_level).filter(|&n| n == number).count();
        // 25 / 30 / 25 / 10 / 5 / 5 (plain, Racket, Roulette, Pistols, the Saloons), Roulette's band
        // the room's first (`below` 0), Racket by the gun.
        assert_eq!(rolls(plain, 15).count(), 100);
        assert_eq!((count(plain, 15, 8), count(plain, 15, 9)), (25, 30));
        assert_eq!((count(plain, 15, 14), count(plain, 15, 18)), (25, 10));
        assert_eq!((count(plain, 15, 11), count(plain, 15, 10)), (5, 5));
        // A room that allows no band: no Roulette, no Pistols, the rest as before.
        assert_eq!(rolls(plain, 4).count(), 65);
        assert_eq!((count(plain, 4, 14), count(plain, 4, 18)), (0, 0));
        // Racket in a level-10 room: the AK-74N.
        let racket = draw(plain, 10, &mut |n| if n == 100 { 30 } else { 0 }).armed();
        assert_eq!(racket, Some(Armed::Racket(Gun::Ak74n)));
        // A map played with a twist only: the twists alone, Roulette's and Pistols' too.
        let twisted = |open: OpenPlay| {
            playable_in(GameMode::FreeForAll)
                .filter(move |map| map.open_play == open)
                .map(|map| MatchMap::of(map.id).unwrap())
        };
        assert_eq!(twisted(OpenPlay::Twisted).count(), 1);
        for map in twisted(OpenPlay::Twisted) {
            assert_eq!(rolls(map, 15).count(), 75);
            assert_eq!((count(map, 15, 8), count(map, 15, 14), count(map, 15, 18)), (0, 25, 10));
        }
        // A map played with Racket alone (Station): every card Racket, in a room with Roulette's and
        // Pistols' bands and in one without; Racket's gun by the room.
        assert_eq!(twisted(OpenPlay::Only(Twist::Racket)).count(), 1);
        for map in twisted(OpenPlay::Only(Twist::Racket)) {
            for max_level in [15, 4] {
                assert_eq!(rolls(map, max_level).count(), 30);
                assert_eq!(count(map, max_level, 9), 30);
                assert_eq!(count(map, max_level, 18), 0);
            }
            assert_eq!(draw(map, 10, &mut |_| 0).armed(), Some(Armed::Racket(Gun::Ak74n)));
        }
        // Each twist as it arms its own number; none a mode's (nor Scarcity's 7, not on main, nor
        // the unmerged Instagib's 17).
        assert_eq!(LobbyMode::of_number(17), None);
        for number in [9, 10, 11, 14, 15, 16, 18] {
            let mode = LobbyMode::of_number(number).unwrap();
            assert_eq!(mode.mode(), GameMode::FreeForAll, "{number}");
            assert_eq!(GameMode::of_lobby_number(number), None);
        }
    }

    /// Over many cards of a level-15 room on a plain free-for-all map, drawn as the rooms draw them
    /// (random rolls, every band and hand dealt), each version comes as often as its share:
    /// Pistols a tenth of the cards (the owner, 2026-11-27: "it should be 10% of FFA cards").
    #[test]
    fn many_cards_draw_each_version_by_its_share() {
        const CARDS: usize = 20_000;
        let ffa = playable_in(GameMode::FreeForAll).find(|map| map.open_play == OpenPlay::Any).unwrap();
        let map = MatchMap::of(ffa.id).unwrap();
        let mut state = 0x5EED_u64;
        let mut below = |n: usize| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((state >> 33) as usize) % n.max(1)
        };
        let mut seen = std::collections::BTreeMap::<u8, usize>::new();
        let mut bands = std::collections::BTreeMap::<u8, usize>::new();
        for _ in 0..CARDS {
            let played = draw(map, 15, &mut below);
            *seen.entry(played.lobby_number()).or_default() += 1;
            if let Some(Armed::Pistols(pair)) = played.armed() {
                *bands.entry(pair.band().top()).or_default() += 1;
            }
        }
        let share = |number: u8| 100.0 * *seen.get(&number).unwrap_or(&0) as f64 / CARDS as f64;
        let roulette = share(14) + share(15) + share(16);
        // Each within 1.5 points of its share: over 4.5 standard errors at 20,000 cards.
        for (what, got, want) in [
            ("plain", share(8), 25.0),
            ("Racket", share(9), 30.0),
            ("Roulette", roulette, 25.0),
            ("Pistols", share(18), 10.0),
            ("Saloon (Winchester)", share(11), 5.0),
            ("Saloon", share(10), 5.0),
        ] {
            assert!((got - want).abs() < 1.5, "{what}: {got:.2} % of {CARDS} cards, its share {want} %");
        }
        // Pistols' bands in a level-15 room: 0-5 or 6-10, half each, never 11-15.
        assert_eq!(bands.keys().copied().collect::<Vec<u8>>(), [5, 10]);
        let pistols = *seen.get(&18).unwrap() as f64;
        for (&top, &cards) in &bands {
            assert!((cards as f64 / pistols - 0.5).abs() < 0.06, "band to {top}: {cards} of {pistols}");
        }
    }
}

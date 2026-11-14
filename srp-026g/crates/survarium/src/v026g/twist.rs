//! A free-for-all's twists (`docs/notes/ffa.md`, "Twists"): a match of a free-for-all map played
//! with every kit's guns changed by the server, the map and its rules as they are. A twist is no
//! map of its own (the owner, 2026-11-14: "we will add more maps in the future": the match
//! options' map id is one byte, and a mode's own copies of the 33 free-for-all maps would take a
//! third of what is left); the rooms' vote draws one for its free-for-all cards
//! (`session::v026g::rooms`), and the lobby names it by its own mode number.

use super::dictionary::PROFILE_SLOTS;
use super::maps::{GameMode, MapMode, MatchMap};
use super::match_msgs::static_info::{
    FULL_CONDITION, KIT_USABLE, MatchItem, MatchKit, MatchProfile, NO_PROTECTION,
};
use super::skills::MODIFIERS;
use srp_validate::world::WEAPON_SLOTS;

/// The shares of a free-for-all card's draw ([`Plays::draw`]): the plain mode's and each twist's,
/// in hundredths. Cowboy rare (the owner, 2026-11-14: "cowboy version should be rare with only
/// 10% chance"), Racket as often as the plain mode (my pick).
pub const SHARES: [(Option<Twist>, u32); 3] =
    [(None, 45), (Some(Twist::Racket), 45), (Some(Twist::Cowboy), 10)];

/// What a free-for-all card of the rooms' vote may play: the plain mode, one twist, or more of
/// them, never none. A room's are the ones its pool holds on the card's map: the create window
/// ticks each twist as a mode of its own; the server's rooms have them all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plays {
    /// Bit `i`: [`SHARES`]' `i`th is in; never 0.
    bits: u8,
}

impl Plays {
    /// The plain mode and every twist: a server's room's draw (`SHARES` as they are).
    pub const ALL: Self = Self {
        bits: (1 << SHARES.len()) - 1,
    };

    /// The plays of `plays`; none of them: none.
    pub fn of(plays: impl IntoIterator<Item = Option<Twist>>) -> Option<Self> {
        let bits = plays.into_iter().fold(0, |bits, play| {
            bits | SHARES
                .iter()
                .position(|&(each, _)| each == play)
                .map_or(0, |at| 1 << at)
        });
        (bits != 0).then_some(Self { bits })
    }

    fn shares(self) -> impl Iterator<Item = (Option<Twist>, u32)> {
        SHARES
            .into_iter()
            .enumerate()
            .filter(move |&(at, _)| self.bits >> at & 1 == 1)
            .map(|(_, share)| share)
    }

    /// The sum of its shares: what a draw's roll is below.
    pub fn total(self) -> u32 {
        self.shares().map(|(_, share)| share).sum()
    }

    /// What a card plays, for a roll below [`Self::total`]: each of its plays for its share.
    pub fn draw(self, roll: u32) -> Option<Twist> {
        let mut roll = roll % self.total();
        for (play, share) in self.shares() {
            if roll < share {
                return play;
            }
            roll -= share;
        }
        unreachable!("a roll is below the shares' sum")
    }
}

/// The lives a twist's ammo lasts: its stack's store is a kit's rounds this many times. The game
/// takes each slot's count out of the store at every spawn, a kit change in the countdown too, and
/// a dead player keeps only what is left in it (`snapshot_rules.md` §7.3), so a kit's worth alone
/// would last one life. More than any match has: 15:00 with a 5 s respawn is 180.
pub const STORE_LIVES: u32 = 1_000;

/// What a twist does to the kits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Twist {
    /// Every sniper rifle and silenced gun an AK-12 ([`super::racket`]).
    Racket,
    /// Every kit a Colt Python alone ([`super::cowboy`]); "Saloon" in the lobby.
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

    /// The ammo the twist gives, which a match of it never charges (`session::v026g::autobuy`):
    /// nobody's, though an account may own the same kind.
    pub fn gives(self) -> u16 {
        match self {
            Self::Racket => super::racket::Arms::shared().ammo,
            Self::Cowboy => super::cowboy::Revolver::shared().ammo,
        }
    }

    /// The twist of a lobby number.
    pub fn of_lobby_number(number: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|twist| twist.lobby_number() == number)
    }

    /// `profile`'s kits as the twist arms them.
    pub fn arm(self, profile: &mut MatchProfile) {
        match self {
            Self::Racket => super::racket::arm(profile, &super::racket::Arms::shared()),
            Self::Cowboy => super::cowboy::arm(profile, &super::cowboy::Revolver::shared()),
        }
    }

    /// The guns of a kit whose slot `slot` holds `held(slot)` (items_dict ids) as the twist arms
    /// them ([`Self::arm`] on a profile of that kit alone): each weapon slot with the gun a match
    /// of the twist played there, for what its history keeps.
    pub fn armed_guns(self, held: impl Fn(usize) -> Option<u16>) -> [(usize, Option<u16>); 2] {
        let mut items = Vec::new();
        let mut slots = [None; PROFILE_SLOTS];
        for slot in WEAPON_SLOTS.map(usize::from) {
            if let Some(dict_id) = held(slot) {
                slots[slot] = Some((items.len() as u16, 1));
                items.push(MatchItem {
                    instance_id: items.len() as u32 + 1,
                    dict_id,
                    condition_or_stack: FULL_CONDITION,
                    ..MatchItem::default()
                });
            }
        }
        let mut profile = MatchProfile {
            team: 0,
            name: String::new(),
            clan_tag: String::new(),
            protection: NO_PROTECTION,
            kits: vec![MatchKit {
                state: KIT_USABLE,
                id: 0,
                name: String::new(),
                slots,
                modifiers: [0.0; MODIFIERS],
            }],
            items,
            squad: 0,
        };
        self.arm(&mut profile);
        let (kit, items) = (&profile.kits[0], &profile.items);
        WEAPON_SLOTS.map(usize::from).map(|slot| {
            let gun = kit.slots[slot].and_then(|(index, _)| items.get(usize::from(index)));
            (slot, gun.map(|item| item.dict_id))
        })
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

    /// A kit's guns as each twist armed them: Racket's AK-12 for the sniper rifle, its other gun
    /// kept; Cowboy's revolver alone; nothing held, nothing armed.
    #[test]
    fn a_kits_guns_are_the_twists() {
        let id = |name| super::super::dictionary::Dictionary::shared().item_named(name).unwrap().dict_id;
        let kit = |slot| match slot {
            7 => Some(id("svd")),
            10 => Some(id("ak_74u")),
            _ => None,
        };
        let racket = id(super::super::racket::GUN);
        assert_eq!(Twist::Racket.armed_guns(kit), [(7, Some(racket)), (10, Some(id("ak_74u")))]);
        let revolver = id(super::super::cowboy::GUN_NAME);
        assert_eq!(Twist::Cowboy.armed_guns(kit), [(7, Some(revolver)), (10, None)]);
        assert_eq!(Twist::Racket.armed_guns(|_| None), [(7, None), (10, None)]);
    }

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
        // Every roll below the shares' sum: each its share; of fewer plays, each of them its own.
        let drawn = |plays: Plays, twist| (0..plays.total()).filter(|&roll| plays.draw(roll) == twist).count();
        for (twist, share) in SHARES {
            assert_eq!(drawn(Plays::ALL, twist), share as usize);
        }
        let rare = Plays::of([Some(Twist::Racket), Some(Twist::Cowboy)]).unwrap();
        assert_eq!((rare.total(), drawn(rare, None), drawn(rare, Some(Twist::Cowboy))), (55, 0, 10));
        let plain = Plays::of([None, None]).unwrap();
        assert_eq!((plain.total(), drawn(plain, None)), (45, 45));
        assert_eq!(Plays::of([None, Some(Twist::Racket), Some(Twist::Cowboy)]), Some(Plays::ALL));
        assert_eq!(Plays::of([]), None);
        // No twist shares a number with a mode (nor with Scarcity's 7, not on main).
        for twist in Twist::ALL {
            assert_eq!(GameMode::of_lobby_number(twist.lobby_number()), None);
            assert_ne!(twist.lobby_number(), 7);
            assert_eq!(Twist::of_lobby_number(twist.lobby_number()), Some(twist));
        }
    }
}

//! Pistols, a free-for-all's twist (`docs/notes/ffa-pistols.md`; the owner, 2026-11-27: "it is just
//! like ffa: roulette, but everyone gets 2 slots instead of 3 and 2 pistols of the same level
//! band, also kashtan is not a pistol"): a card draws a level [`Band`] its room allows (0-5 or
//! 6-10: "as always 11-15 rooms just get 0-5 or 6-10 equips", the owner, 2026-11-27), and one
//! [`Pair`] of two kits from it, each a pistol of the band; every player of
//! the match plays those two kits ([`arm`]), nothing of his own, his third kit not loaded. The
//! rest is Roulette's ([`super::roulette`]): the armour, pockets, ammo, bonuses and attachments,
//! drawn alike, and the match server arms the profiles the static info sends, so every game, a
//! late joiner and a replay play the same kits.

use std::fmt;

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::match_msgs::static_info::MatchProfile;
use super::roulette::{self, Dealt, Pool};

/// The kits of a pair: two of a profile's three (the owner: "2 slots instead of 3").
pub const KITS: usize = 2;

/// A Pistols level band: Roulette's 0-5 and 6-10 ([`roulette::Band`]), not its 11-15, which has no
/// pistol of its own (none is above level 10): the whole pair, armour too, is of one of these (the
/// owner, 2026-11-27: "as always 11-15 rooms just get 0-5 or 6-10 equips").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Band {
    /// 0 to 5.
    Low,
    /// 6 to 10.
    Mid,
}

impl Band {
    pub const ALL: [Self; 2] = [Self::Low, Self::Mid];

    /// Roulette's band of the same levels: its pools.
    pub const fn levels_of(self) -> roulette::Band {
        match self {
            Self::Low => roulette::Band::Low,
            Self::Mid => roulette::Band::Mid,
        }
    }

    /// Its lowest and its highest level.
    pub const fn levels(self) -> (u8, u8) {
        self.levels_of().levels()
    }

    pub const fn top(self) -> u8 {
        self.levels().1
    }

    /// The bands a room that lets in equipment up to `max_level` (0: any) allows: each whose top
    /// is at most that, as Roulette's (`roulette::Band::allowed`), 11-15 never.
    pub fn allowed(max_level: u8) -> impl Iterator<Item = Self> {
        Self::ALL
            .into_iter()
            .filter(move |band| max_level == 0 || band.top() <= max_level)
    }

    /// One of the bands `max_level` allows, each as likely (a level-15 or level-10 room 0-5 or
    /// 6-10 half each); none for a room under level 5.
    pub fn draw(max_level: u8, below: &mut impl FnMut(usize) -> usize) -> Option<Self> {
        let allowed: Vec<Self> = Self::allowed(max_level).collect();
        (!allowed.is_empty()).then(|| allowed[roulette::pick(allowed.len(), below)])
    }
}

/// A Pistols match's equipment: two kits of one band, every player's.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pair {
    band: Band,
    kits: [Dealt; KITS],
}

impl Pair {
    /// A pair of `band`, each choice one of `below`'s rolls (`below(n)` is under `n`), drawn as a
    /// Roulette pistol hand's kits are: each kit a pistol of the band's
    /// (`roulette::BandPool::pistols`), the two different while the band has two, its armour,
    /// pockets and ammo of the band as Roulette's; then every gun its attachments and every gun
    /// and armour piece its bonuses.
    pub fn deal(band: Band, below: &mut impl FnMut(usize) -> usize) -> Self {
        let pool = Pool::shared().band(band.levels_of());
        let mut pistol = pool.pistol_draw();
        let mut kits = std::array::from_fn(|_| {
            let main = pistol.next(below);
            pool.kit(main, below)
        });
        pool.dress(&mut kits, below);
        Self { band, kits }
    }

    pub const fn band(&self) -> Band {
        self.band
    }

    pub fn kits(&self) -> &[Dealt; KITS] {
        &self.kits
    }

    /// Every stack it deals: the pistols' ammo and the pocket items.
    pub fn stacks(&self) -> Vec<u16> {
        roulette::stacks(&self.kits)
    }
}

/// Its band and its pistols, by name: what a log line says of it.
impl fmt::Debug for Pair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (low, top) = self.band.levels();
        write!(f, "Pistols {low}-{top}:")?;
        let dictionary = Dictionary::shared();
        for kit in &self.kits {
            match dictionary.item(kit.main.gun) {
                Some(item) => write!(f, " {}", item.name)?,
                None => write!(f, " {}", kit.main.gun)?,
            }
        }
        Ok(())
    }
}

/// The pistols `band` deals, by dict id, the lowest level first (then in dictionary order): what
/// the wiki lists.
pub fn pistols(band: Band) -> Vec<u16> {
    let mut pistols: Vec<u16> = Pool::shared().band(band.levels_of()).pistols().collect();
    pistols.sort_by_key(|&gun| {
        let level = super::item_params::item_params(gun).map(|params| params.match_making_level);
        (level, gun)
    });
    pistols
}

/// Every band of the game data has a pistol to deal (beside what [`roulette::check`] asks of
/// it); srp refuses to start otherwise. A band of one pistol deals it to both kits.
pub fn check(data: &GameData) -> Result<(), GameDataError> {
    roulette::check(data)?;
    match Band::ALL.into_iter().find(|&band| pistols(band).is_empty()) {
        Some(band) => {
            let (low, top) = band.levels();
            Err(GameDataError(format!(
                "{}: Pistols: levels {low}-{top} have no pistol to deal",
                data.origin
            )))
        }
        None => Ok(()),
    }
}

/// `profile` with `pair`'s kits as its own: kit 1 and 2 the pair's, held to the layout their
/// skills give, loaded, their ids, names and skills kept, their pockets packed; kit 3 not loaded
/// and empty (no player object: nobody spawns with it, `match.md` §23). The profile's items are
/// the pair's alone, as Roulette's ([`roulette::arm`]).
pub fn arm(profile: &mut MatchProfile, pair: &Pair) {
    roulette::arm_kits(profile, |n| pair.kits.get(n));
}

#[cfg(test)]
mod tests {
    use super::super::item_params;
    use super::super::loadouts::category;
    use super::super::match_msgs::static_info::{
        self, KIT_NOT_LOADED, KIT_USABLE, MatchKit,
    };
    use super::*;

    /// Rolls from a seed: the same seed, the same rolls.
    fn rolls(seed: u64) -> impl FnMut(usize) -> usize {
        let mut state = seed;
        move |n| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((state >> 33) as usize) % n.max(1)
        }
    }

    fn profile() -> MatchProfile {
        MatchProfile {
            team: 0,
            name: "pistols".to_owned(),
            clan_tag: String::new(),
            protection: static_info::NO_PROTECTION,
            kits: (1..=3).map(|id| MatchKit::empty(id, &format!("kit {id}"))).collect(),
            items: Vec::new(),
            squad: 0,
            face: crate::v026g::faces::Face::DEFAULT,
            notice: crate::v026g::match_msgs::static_info::Notice::Skipped,
        }
    }

    fn name(dict_id: u16) -> &'static str {
        &Dictionary::shared().item(dict_id).unwrap().name
    }

    /// Each band's pistols: the pistols' category, one hand, of the band's levels, never the
    /// Kashtan, at least two to deal.
    #[test]
    fn each_band_deals_its_own_pistols_never_the_kashtan() {
        check(super::super::game_data::shared()).unwrap();
        let dictionary = Dictionary::shared();
        let kashtan = dictionary.item_named("aek_919k").map(|item| item.dict_id);
        for band in Band::ALL {
            let pool = pistols(band);
            assert!(pool.len() >= KITS, "{band:?}: {pool:?}");
            let (low, top) = band.levels();
            for &gun in &pool {
                let item = dictionary.item(gun).unwrap();
                let params = item_params::item_params(gun).unwrap();
                assert_eq!(item.category, category::PISTOLS, "{}", item.name);
                assert!(!params.double_handed, "{}", item.name);
                assert!((low..=top).contains(&params.match_making_level), "{}", item.name);
                assert_eq!(item.like, None, "{}: a copy", item.name);
                assert_ne!(Some(gun), kashtan);
            }
        }
        let low: Vec<&str> = pistols(Band::Low).into_iter().map(name).collect();
        assert!(low.contains(&"tt_33") && low.contains(&"pm"), "{low:?}");
        // The Kashtan, where the data has it, is a submachine gun held in two hands.
        if let Some(kashtan) = kashtan {
            assert_eq!(dictionary.item(kashtan).unwrap().category, category::SMGS);
            assert!(item_params::item_params(kashtan).unwrap().double_handed);
        }
    }

    /// Every band deals: the same rolls the same pair, two kits of two different pistols of the
    /// band, every piece in the band, a profile a match is sent: kits 1 and 2 loaded with a pistol
    /// in weapon 1's slot alone, kit 3 not loaded and empty, its id and name kept.
    #[test]
    fn a_pair_is_two_kits_of_two_pistols_and_the_third_kit_is_not_loaded() {
        let dictionary = Dictionary::shared();
        for band in Band::ALL {
            let pool = pistols(band);
            for seed in 0..30 {
                let pair = Pair::deal(band, &mut rolls(seed));
                assert_eq!(pair, Pair::deal(band, &mut rolls(seed)));
                assert_eq!(pair.band(), band);
                let [first, second] = pair.kits().map(|kit| kit.main.gun);
                assert_ne!(first, second, "{pair:?}");
                assert!(pool.contains(&first) && pool.contains(&second), "{pair:?}");
                let mut armed = profile();
                arm(&mut armed, &pair);
                static_info::check_sent(0, &armed, dictionary).unwrap();
                for (n, kit) in armed.kits.iter().enumerate() {
                    assert_eq!((kit.id, kit.name.as_str()), (n as u32 + 1, format!("kit {}", n + 1).as_str()));
                    if n < KITS {
                        assert_eq!(kit.state, KIT_USABLE);
                        let (gun, _) = kit.slots[7].unwrap();
                        assert_eq!(armed.items[usize::from(gun)].dict_id, pair.kits()[n].main.gun);
                        assert_eq!((kit.slots[10], kit.slots[11], kit.slots[12]), (None, None, None));
                        assert!(kit.slots[8].is_some(), "{pair:?}: its ammo");
                        // The pistol and every armour piece of the drawn band.
                        let (low, top) = band.levels();
                        for (at, _) in (0..=7).filter_map(|slot| kit.slots[slot]) {
                            let level = item_params::item_params(armed.items[usize::from(at)].dict_id)
                                .unwrap()
                                .match_making_level;
                            assert!((low..=top).contains(&level), "{pair:?}: level {level}");
                        }
                    } else {
                        assert_eq!(kit.state, KIT_NOT_LOADED);
                        assert!(kit.slots.iter().all(Option::is_none));
                    }
                }
                // Every item a loaded kit holds; nothing of the account's.
                armed.drop_unheld();
                assert!(armed.items.iter().all(|item| item.dict_id != 0));
            }
        }
    }

    /// The bands a room draws: 0-5 or 6-10 in a level-15 room (the owner, 2026-11-27: "as always
    /// 11-15 rooms just get 0-5 or 6-10 equips") and a level-10 one, about half each; never
    /// 11-15; none under level 5.
    #[test]
    fn a_room_draws_the_low_or_the_mid_band_never_the_high() {
        let allowed = |max| Band::allowed(max).collect::<Vec<_>>();
        assert_eq!(allowed(15), Band::ALL);
        assert_eq!(allowed(10), Band::ALL);
        assert_eq!(allowed(0), Band::ALL);
        assert_eq!(allowed(5), [Band::Low]);
        assert!(allowed(4).is_empty());
        assert_eq!(Band::draw(4, &mut rolls(1)), None);
        let mut below = rolls(11);
        let draws = 4_000;
        let low = (0..draws).filter(|_| Band::draw(15, &mut below) == Some(Band::Low)).count();
        assert!((low as f64 / draws as f64 - 0.5).abs() < 0.04, "{low} of {draws} 0-5");
        assert_eq!(Band::Mid.levels_of(), roulette::Band::Mid);
    }

    /// The same pair for every player, whatever his own kits: a late joiner's profile is armed
    /// alike.
    #[test]
    fn every_profile_is_armed_alike() {
        let pair = Pair::deal(Band::Mid, &mut rolls(3));
        let mut a = profile();
        let mut b = profile();
        b.kits[2] = MatchKit::empty(9, "");
        arm(&mut a, &pair);
        arm(&mut b, &pair);
        assert_eq!(a.items, b.items);
        assert_eq!(a.kits[..KITS], b.kits[..KITS]);
        assert_eq!(pair.stacks(), roulette::stacks(pair.kits()));
    }
}

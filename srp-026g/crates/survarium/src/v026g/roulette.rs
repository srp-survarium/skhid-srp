//! Roulette, a free-for-all's twist (`docs/notes/ffa-roulette.md`; the owner, 2026-11-22: "where
//! everyone gets the same random equipment of the same level (3 kits are different)"): a card draws
//! a level [`Band`] its room allows and one [`Hand`] of three kits from it, and every player of
//! the match plays that hand ([`arm`]), nothing of his own. The match server arms the profiles the
//! static info sends, so every game, a late joiner and a replay play the same kits.

use std::fmt;

use weapon_pickup::CarryRule;

use super::dictionary::{Dictionary, Item, PROFILE_SLOTS};
use super::game_data::{GameData, GameDataError};
use super::item_params::{self, ItemClass};
use super::loadouts::{self, KitLayout, POCKET_SLOTS, bot_gear::category};
use super::match_msgs::static_info::{FULL_CONDITION, KIT_USABLE, MatchItem, MatchKit, MatchProfile};
use super::prices;
use super::skills::MODIFIERS;
use super::skills::slot_modifier::{DISABLE_SECOND_WEAPON, USE_TWO_DOUBLE_HANDED_WEAPONS};
use super::twist::STORE_LIVES;

/// The kits of a hand: a profile's three.
pub const KITS: usize = 3;

/// The main guns' classes, by dictionary category: a hand's three kits are of three of them.
const MAIN_CLASSES: [u8; 5] = [
    category::ASSAULT_RIFLES,
    category::MACHINE_GUNS,
    category::SHOTGUNS,
    category::SNIPER_RIFLES,
    category::SMGS,
];

/// The weapon slots and the ammo slot each one's rounds go into.
const MAIN: (u8, u8) = (7, 8);
const PISTOL: (u8, u8) = (10, 11);

/// The armour slots in the order a kit draws them (helmet 0, mask 1, torso 2, back 3, legs 4,
/// gloves 5, boots 6): the torso first, which may cover the legs, the helmet before the mask.
const ARMOUR_ORDER: [u8; 7] = [2, 0, 1, 4, 5, 6, 3];

/// The kinds of pocket item a kit is dealt: a medkit, a grenade, then four others.
const POCKET_KINDS: usize = POCKET_SLOTS.len();

/// The times a kit's armour is drawn again while the kit is over a player's carry limit.
const WEIGHT_DRAWS: usize = 16;

/// What a pocket is dealt: more than any pocket holds, so it is full ([`KitLayout::kept`]).
const POCKET_FILL: u32 = 100;

/// A level band: every gun and armour piece of a hand is of one (the owner, 2026-11-22: "it should
/// be either lvl1 to 5, lvl6 to 10, lvl11 to 15"; level 0 with the low one, his of the same day),
/// by the item's `match_making_level`, the rooms' measure of a kit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Band {
    /// 0 to 5.
    Low,
    /// 6 to 10.
    Mid,
    /// 11 to 15.
    High,
}

impl Band {
    pub const ALL: [Self; 3] = [Self::Low, Self::Mid, Self::High];

    /// Its lowest and its highest level.
    pub const fn levels(self) -> (u8, u8) {
        match self {
            Self::Low => (0, 5),
            Self::Mid => (6, 10),
            Self::High => (11, 15),
        }
    }

    pub const fn top(self) -> u8 {
        self.levels().1
    }

    fn holds(self, level: u8) -> bool {
        let (low, top) = self.levels();
        (low..=top).contains(&level)
    }

    /// The bands a room that lets in equipment up to `max_level` (0: any) allows: each whose top
    /// is at most that (the owner, 2026-11-22: "high lvl rooms can get low lvl bands").
    pub fn allowed(max_level: u8) -> impl Iterator<Item = Self> {
        Self::ALL
            .into_iter()
            .filter(move |band| max_level == 0 || band.top() <= max_level)
    }

    /// One of the bands `max_level` allows, each as likely; none for a room under the low band's
    /// top.
    pub fn draw(max_level: u8, below: &mut impl FnMut(usize) -> usize) -> Option<Self> {
        let allowed: Vec<Self> = Self::allowed(max_level).collect();
        (!allowed.is_empty()).then(|| allowed[below(allowed.len()) % allowed.len()])
    }
}

/// A gun of a hand and the ammunition it is dealt with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gun {
    pub gun: u16,
    pub ammo: u16,
    /// A clip of the ammo: its rounds and the ammo bags it takes.
    clip: u32,
    clip_bags: u32,
}

impl Gun {
    /// Its rounds for `bags` ammo bags: a clip for each clip's bags.
    fn rounds(self, bags: u32) -> u32 {
        bags / self.clip_bags * self.clip
    }
}

/// One kit of a hand: what every player's kit of its place holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Dealt {
    pub main: Gun,
    pub pistol: Gun,
    /// Armour slots 0 to 6; a slot another piece covers is empty.
    pub armour: [Option<u16>; 7],
    /// The pocket items in the order the pockets take them, each a type of its own.
    pub pockets: [Option<u16>; POCKET_KINDS],
}

impl Dealt {
    /// Its slots for a player whose kit has the skills' `modifiers`, as `(dict id, units)`: the
    /// armour, the main gun, the pistol unless the kit carries one weapon, a clip of each gun's ammo
    /// for every ammo bag of the kit (the main gun's the first and every other one, the pistol's
    /// the rest), the pocket items as many as the kit's pockets open, each pocket full.
    fn slots(&self, modifiers: &[f32; MODIFIERS]) -> [Option<(u16, u32)>; PROFILE_SLOTS] {
        let mut slots = [None; PROFILE_SLOTS];
        for (slot, piece) in self.armour.iter().enumerate() {
            slots[slot] = piece.map(|piece| (piece, 1));
        }
        let carry = CarryRule::from_modifiers(
            modifiers[DISABLE_SECOND_WEAPON],
            modifiers[USE_TWO_DOUBLE_HANDED_WEAPONS],
        );
        let guns = match carry {
            CarryRule::OneWeapon => vec![(MAIN, self.main)],
            CarryRule::OneDoubleHanded | CarryRule::TwoDoubleHanded => {
                vec![(MAIN, self.main), (PISTOL, self.pistol)]
            }
        };
        for &((gun_slot, _), gun) in &guns {
            slots[usize::from(gun_slot)] = Some((gun.gun, 1));
        }
        let worn: Vec<(u32, bool)> = slots
            .iter()
            .flatten()
            .flat_map(|&(dict_id, _)| loadouts::default_modifications(dict_id))
            .collect();
        let layout = KitLayout::new(&worn, modifiers);
        let bags = layout.ammo_bags();
        let shares = match guns.len() {
            1 => vec![bags],
            _ => vec![bags.div_ceil(2), bags / 2],
        };
        for (((_, ammo_slot), gun), bags) in guns.into_iter().zip(shares) {
            let rounds = gun.rounds(bags);
            if rounds > 0 {
                slots[usize::from(ammo_slot)] = Some((gun.ammo, rounds));
            }
        }
        for (pocket, kind) in POCKET_SLOTS.into_iter().zip(self.pockets) {
            slots[usize::from(pocket)] = kind.map(|kind| (kind, POCKET_FILL));
        }
        let mut kept = layout.kept(&slots);
        if kept[usize::from(PISTOL.0)].is_none() {
            kept[usize::from(PISTOL.1)] = None;
        }
        kept
    }

    /// What its slots weigh for a player of no skills.
    fn weight(&self) -> f32 {
        self.slots(&[0.0; MODIFIERS])
            .iter()
            .flatten()
            .map(|&(dict_id, units)| item_params::carried_weight(dict_id, units))
            .sum()
    }
}

/// A Roulette match's equipment: three kits of one band, every player's.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hand {
    band: Band,
    kits: [Dealt; KITS],
}

impl Hand {
    /// A hand of `band`, each choice one of `below`'s rolls (`below(n)` is under `n`):
    ///
    /// - three main-gun classes of those the band has, each as likely, and a gun of each, each as
    ///   likely: one a kit;
    /// - each kit a pistol, its armour slot by slot (a piece is drawn only if no slot it covers is
    ///   filled), a medkit, a grenade and four other pocket items in a drawn order; a kit over a
    ///   player's carry limit draws its armour again, [`WEIGHT_DRAWS`] times at most, then leaves
    ///   its heaviest pieces out until it fits.
    pub fn deal(band: Band, below: &mut impl FnMut(usize) -> usize) -> Self {
        let pool = &Pool::shared().bands[band as usize];
        let mut classes: Vec<&Vec<Gun>> = pool.classes.iter().collect();
        let kits = std::array::from_fn(|_| {
            let class = classes.remove(pick(classes.len(), below));
            let main = class[pick(class.len(), below)];
            pool.kit(main, below)
        });
        Self { band, kits }
    }

    pub const fn band(&self) -> Band {
        self.band
    }

    pub fn kits(&self) -> &[Dealt; KITS] {
        &self.kits
    }

    /// Every stack it deals: the guns' ammo and the pocket items.
    pub fn stacks(&self) -> Vec<u16> {
        let mut stacks: Vec<u16> = self
            .kits
            .iter()
            .flat_map(|kit| {
                [kit.main.ammo, kit.pistol.ammo]
                    .into_iter()
                    .chain(kit.pockets.into_iter().flatten())
            })
            .collect();
        stacks.sort_unstable();
        stacks.dedup();
        stacks
    }
}

/// Its band and its main guns, by name: what a log line says of it.
impl fmt::Debug for Hand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (low, top) = self.band.levels();
        write!(f, "Roulette {low}-{top}:")?;
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

/// A roll under `n` (`n` > 0).
fn pick(n: usize, below: &mut impl FnMut(usize) -> usize) -> usize {
    below(n) % n
}

/// An item's `match_making_level`.
fn level(dict_id: u16) -> Option<u8> {
    item_params::item_params(dict_id).map(|params| params.match_making_level)
}

/// What the game makes of an item in a kit.
fn class(dict_id: u16) -> Option<ItemClass> {
    item_params::item_params(dict_id).map(|params| params.class)
}

/// What a band deals from, read from the game data ([`Pool::of`]).
#[derive(Clone, Debug, PartialEq)]
struct BandPool {
    /// Each main-gun class the band has, its guns.
    classes: Vec<Vec<Gun>>,
    pistols: Vec<Gun>,
    /// By armour slot.
    armour: [Vec<u16>; 7],
    medkits: Vec<u16>,
    grenades: Vec<u16>,
    /// The other pocket items.
    others: Vec<u16>,
}

impl BandPool {
    /// A kit of `main`.
    fn kit(&self, main: Gun, below: &mut impl FnMut(usize) -> usize) -> Dealt {
        let pistol = self.pistols[pick(self.pistols.len(), below)];
        let mut pockets = [None; POCKET_KINDS];
        let mut others = self.others.clone();
        let medkit = self.medkits[pick(self.medkits.len(), below)];
        let grenade = self.grenades[pick(self.grenades.len(), below)];
        others.retain(|&kind| kind != medkit && kind != grenade);
        pockets[0] = Some(medkit);
        pockets[1] = Some(grenade);
        for pocket in pockets.iter_mut().skip(2) {
            if others.is_empty() {
                break;
            }
            *pocket = Some(others.remove(pick(others.len(), below)));
        }
        let mut kit = Dealt {
            main,
            pistol,
            armour: [None; 7],
            pockets,
        };
        let limit = item_params::player().max_carried_weight;
        for _ in 0..WEIGHT_DRAWS {
            kit.armour = self.armour(below);
            if kit.weight() <= limit {
                return kit;
            }
        }
        while kit.weight() > limit {
            let weight = |slot: usize| {
                kit.armour[slot].map_or(0.0, |piece| item_params::carried_weight(piece, 1))
            };
            let Some(heaviest) = (0..kit.armour.len())
                .filter(|&slot| kit.armour[slot].is_some())
                .max_by(|&a, &b| weight(a).total_cmp(&weight(b)))
            else {
                break;
            };
            kit.armour[heaviest] = None;
        }
        kit
    }

    /// A kit's armour, slot by slot in [`ARMOUR_ORDER`].
    fn armour(&self, below: &mut impl FnMut(usize) -> usize) -> [Option<u16>; 7] {
        let mut armour = [None; 7];
        let mut covered = [false; 7];
        for slot in ARMOUR_ORDER.map(usize::from) {
            if covered[slot] {
                continue;
            }
            let fits = |piece: &&u16| {
                item_params::covered_slots(**piece)
                    .iter()
                    .all(|&other| armour.get(usize::from(other)).is_none_or(Option::is_none))
            };
            let candidates: Vec<u16> = self.armour[slot].iter().filter(fits).copied().collect();
            if candidates.is_empty() {
                continue;
            }
            let piece = candidates[pick(candidates.len(), below)];
            armour[slot] = Some(piece);
            for &other in item_params::covered_slots(piece) {
                if let Some(flag) = covered.get_mut(usize::from(other)) {
                    *flag = true;
                }
            }
        }
        armour
    }
}

/// Every band's [`BandPool`].
#[derive(Clone, Debug, PartialEq)]
struct Pool {
    bands: [BandPool; 3],
}

impl Pool {
    /// The pools of `dictionary`'s items that are dealt: those the shop offers (sold, or a premium
    /// item, sold for silver as any), no golden or premium copy of another; or what a band lacks
    /// for a hand: three main-gun classes, a pistol, a medkit, a grenade.
    fn of(dictionary: &Dictionary) -> Result<Self, String> {
        let offered: Vec<&Item> = dictionary
            .items
            .iter()
            .filter(|item| item.like.is_none())
            .filter(|item| item.is_premium || prices::price(item.dict_id).is_some())
            .collect();
        // A gun of `category` fitting `slots` in `band`'s levels (at most its top for a pistol),
        // with the highest ammunition of at most its top it fires that its ammo slot takes.
        let guns = |category: u8, (gun_slot, ammo_slot): (u8, u8), levels: Band, top: u8| {
            offered
                .iter()
                .filter(|item| {
                    !item.is_stack
                        && item.category == category
                        && dictionary.fits(item.dict_id, gun_slot)
                        && item_params::item_params(item.dict_id).is_some_and(|p| p.gun.is_some())
                        && level(item.dict_id).is_some_and(|level| levels.holds(level))
                })
                .filter_map(|item| {
                    let ammo = offered
                        .iter()
                        .filter(|ammo| {
                            ammo.is_stack
                                && class(ammo.dict_id) == Some(ItemClass::Ammunition)
                                && dictionary.compatible(item.dict_id, ammo.dict_id)
                                && dictionary.fits(ammo.dict_id, ammo_slot)
                                && level(ammo.dict_id).is_some_and(|level| level <= top)
                        })
                        .filter_map(|ammo| {
                            let params = item_params::item_params(ammo.dict_id)?;
                            (params.clip_size > 0 && params.required_bags > 0)
                                .then_some((params.match_making_level, ammo, params))
                        })
                        .max_by_key(|&(level, ammo, _)| (level, std::cmp::Reverse(ammo.dict_id)))?;
                    let (_, ammo, params) = ammo;
                    Some(Gun {
                        gun: item.dict_id,
                        ammo: ammo.dict_id,
                        clip: params.clip_size,
                        clip_bags: params.required_bags,
                    })
                })
                .collect::<Vec<Gun>>()
        };
        let pocket = POCKET_SLOTS[0];
        let band = |band: Band| -> Result<BandPool, String> {
            let top = band.top();
            let (low, _) = band.levels();
            let classes: Vec<Vec<Gun>> = MAIN_CLASSES
                .into_iter()
                .map(|category| guns(category, MAIN, band, top))
                .filter(|guns| !guns.is_empty())
                .collect();
            // The band's pistols, else those of the highest band under it that has some.
            let pistols = Band::ALL
                .into_iter()
                .rev()
                .filter(|lower| lower.top() <= top)
                .map(|lower| guns(category::PISTOLS, PISTOL, lower, top))
                .find(|pistols| !pistols.is_empty())
                .unwrap_or_default();
            let armour = std::array::from_fn(|slot| {
                offered
                    .iter()
                    .filter(|item| {
                        !item.is_stack
                            && dictionary.fits(item.dict_id, slot as u8)
                            && level(item.dict_id).is_some_and(|level| band.holds(level))
                    })
                    .map(|item| item.dict_id)
                    .collect()
            });
            let pockets: Vec<&Item> = offered
                .iter()
                .copied()
                .filter(|item| {
                    item.is_stack
                        && dictionary.fits(item.dict_id, pocket)
                        && level(item.dict_id).is_some_and(|level| level <= top)
                })
                .collect();
            let medkit = |item: &Item| {
                matches!(class(item.dict_id), Some(ItemClass::Medkit { .. }))
                    && item.name.ends_with("medkit")
            };
            let grenade = |item: &Item| class(item.dict_id) == Some(ItemClass::GrenadeSet);
            let ids = |keep: &dyn Fn(&Item) -> bool| -> Vec<u16> {
                pockets
                    .iter()
                    .filter(|&&item| keep(item))
                    .map(|item| item.dict_id)
                    .collect()
            };
            let pool = BandPool {
                classes,
                pistols,
                armour,
                medkits: ids(&medkit),
                grenades: ids(&grenade),
                others: ids(&|item: &Item| !medkit(item) && !grenade(item)),
            };
            let lacks = [
                (pool.classes.len() < KITS, "three main-gun classes"),
                (pool.pistols.is_empty(), "a pistol"),
                (pool.medkits.is_empty(), "a medkit"),
                (pool.grenades.is_empty(), "a grenade"),
            ];
            match lacks.into_iter().find(|&(lacks, _)| lacks) {
                Some((_, what)) => Err(format!("levels {low}-{top} have no {what} to deal")),
                None => Ok(pool),
            }
        };
        Ok(Self {
            bands: [band(Band::Low)?, band(Band::Mid)?, band(Band::High)?],
        })
    }

    /// The pools of the server's game data ([`check`] refused data without them at start).
    fn shared() -> &'static Self {
        static SHARED: std::sync::OnceLock<Pool> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| {
            Pool::of(Dictionary::shared()).expect("every band has a hand to deal (roulette::check)")
        })
    }
}

/// Every band of the game data has a hand to deal ([`Pool::of`]); srp refuses to start otherwise.
pub fn check(data: &GameData) -> Result<(), GameDataError> {
    Pool::of(Dictionary::shared())
        .map(drop)
        .map_err(|why| GameDataError(format!("{}: Roulette: {why}", data.origin)))
}

/// `profile` with `hand`'s kits as its own: kit `n` the hand's `n`-th, held to the layout its skills
/// give ([`Dealt::slots`]), loaded, its id, name and skills kept, its pockets packed. The profile's
/// items are the hand's alone: each once, held by every kit that has it, whole, no modifications or
/// upgrades, ids from 1; a stack's store the largest kit's units of it [`STORE_LIVES`] times.
pub fn arm(profile: &mut MatchProfile, hand: &Hand) {
    let mut items: Vec<MatchItem> = Vec::new();
    let dictionary = Dictionary::shared();
    for (n, kit) in profile.kits.iter_mut().enumerate() {
        let dealt = &hand.kits[n % KITS];
        let slots = dealt.slots(&kit.modifiers).map(|held| {
            let (dict_id, units) = held?;
            let units = u16::try_from(units).unwrap_or(u16::MAX);
            let at = match items.iter().position(|item| item.dict_id == dict_id) {
                Some(at) => at,
                None => {
                    items.push(MatchItem {
                        instance_id: items.len() as u32 + 1,
                        dict_id,
                        condition_or_stack: if dictionary.is_stack(dict_id) {
                            0
                        } else {
                            FULL_CONDITION
                        },
                        ..MatchItem::default()
                    });
                    items.len() - 1
                }
            };
            if dictionary.is_stack(dict_id) {
                let store = &mut items[at].condition_or_stack;
                *store = (*store).max(u32::from(units).saturating_mul(STORE_LIVES));
            }
            Some((at as u16, units))
        });
        let name = std::mem::take(&mut kit.name);
        *kit = MatchKit::new(KIT_USABLE, kit.id, name, slots, kit.modifiers);
    }
    profile.items = items;
}

#[cfg(test)]
mod tests {
    use super::super::match_msgs::static_info;
    use super::*;

    /// Rolls from a seed: the same seed, the same rolls.
    fn rolls(seed: u64) -> impl FnMut(usize) -> usize {
        let mut state = seed;
        move |n| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((state >> 33) as usize) % n.max(1)
        }
    }

    #[test]
    fn a_room_allows_the_bands_under_its_max_level() {
        let allowed = |max| Band::allowed(max).collect::<Vec<_>>();
        assert_eq!(allowed(15), Band::ALL);
        assert_eq!(allowed(0), Band::ALL);
        assert_eq!(allowed(10), [Band::Low, Band::Mid]);
        assert!(allowed(4).is_empty());
        assert_eq!(Band::draw(4, &mut rolls(1)), None);
    }

    /// Every band of the test data deals: the same rolls the same hand, three main-gun classes,
    /// every kit's level in its band, a profile a match is sent.
    #[test]
    fn every_band_deals_a_hand_of_three_classes_in_its_levels() {
        check(super::super::game_data::shared()).unwrap();
        let dictionary = Dictionary::shared();
        for band in Band::ALL {
            for seed in 0..20 {
                let hand = Hand::deal(band, &mut rolls(seed));
                assert_eq!(hand, Hand::deal(band, &mut rolls(seed)));
                let mut classes: Vec<u8> = hand
                    .kits()
                    .iter()
                    .map(|kit| dictionary.item(kit.main.gun).unwrap().category)
                    .collect();
                classes.sort_unstable();
                classes.dedup();
                assert_eq!(classes.len(), KITS, "{hand:?}");
                let mut profile = MatchProfile {
                    team: 0,
                    name: "roulette".to_owned(),
                    clan_tag: String::new(),
                    protection: static_info::NO_PROTECTION,
                    kits: (1..=3).map(|id| MatchKit::empty(id, "")).collect(),
                    items: Vec::new(),
                    squad: 0,
                    face: crate::v026g::faces::Face::DEFAULT,
                };
                arm(&mut profile, &hand);
                static_info::check_sent(0, &profile, dictionary).unwrap();
                for kit in &profile.kits {
                    let held = kit.slots.iter().flatten().map(|&(at, units)| {
                        (0, profile.items[usize::from(at)].dict_id, u32::from(units))
                    });
                    let level = loadouts::gear_level(held);
                    assert!(level <= band.top(), "{hand:?}: level {level}");
                    assert!(kit.slots[7].is_some(), "{hand:?}");
                    assert_eq!(kit.state, KIT_USABLE);
                }
            }
        }
    }

    #[test]
    fn a_kit_of_one_weapon_has_no_pistol() {
        let hand = Hand::deal(Band::Mid, &mut rolls(7));
        let mut modifiers = [0.0; MODIFIERS];
        modifiers[DISABLE_SECOND_WEAPON] = 1.0;
        let slots = hand.kits()[0].slots(&modifiers);
        assert_eq!((slots[10], slots[11]), (None, None));
        assert!(slots[7].is_some());
    }
}

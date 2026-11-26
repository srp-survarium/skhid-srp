//! Roulette, a free-for-all's twist (`docs/notes/ffa-roulette.md`; the owner, 2026-11-22: "where
//! everyone gets the same random equipment of the same level (3 kits are different)"): a card draws
//! a level [`Band`] its room allows and one [`Hand`] of three kits from it, and every player of
//! the match plays that hand ([`arm`]), nothing of his own. The match server arms the profiles the
//! static info sends, so every game, a late joiner and a replay play the same kits.

use std::fmt;

use super::dictionary::{Dictionary, Item, PROFILE_SLOTS};
use super::drops::{Drops, Rng};
use super::game_data::{GameData, GameDataError};
use super::item_params::{self, ItemClass};
use super::loadouts::{self, KitLayout, POCKET_SLOTS, bot_gear::category};
use super::match_msgs::static_info::{
    FULL_CONDITION, KIT_USABLE, MatchItem, MatchKit, MatchProfile, Upgrade,
};
use super::modifications::{MAX_RANK, Modifications};
use super::prices;
use super::skills::MODIFIERS;
use super::twist::STORE_LIVES;
use super::weapon_modules::{self, Fitted, Kind, Modules};

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

/// The main gun's slot, weapon 1's, and the two ammo slots of its rounds. A kit has no other gun
/// (the owner, 2026-11-22: "it shouldn't try to slot in two weapons or pistols in second slot"):
/// weapon 2's slot stays empty, as Saloon's Colt alone leaves it.
const MAIN: (u8, [u8; 2]) = (7, [8, 9]);

/// The share, in percent, of the hands that are a pistol hand: each kit's one gun a pistol, at
/// random of the band's (the owner, 2026-11-22: "we can have random versions where everyone starts
/// with pistols", "it should still be random one", about 5 %).
pub const PISTOL_HANDS_PERCENT: usize = 5;

/// The armour slots in the order a kit draws them (helmet 0, mask 1, torso 2, back 3, legs 4,
/// gloves 5, boots 6): the torso first, which may cover the legs, the helmet before the mask.
const ARMOUR_ORDER: [u8; 7] = [2, 0, 1, 4, 5, 6, 3];

/// The kinds of pocket item a kit is dealt: a medkit, a grenade, then four others.
const POCKET_KINDS: usize = POCKET_SLOTS.len();

/// The times a kit's armour is drawn again while the kit is over a player's carry limit.
const WEIGHT_DRAWS: usize = 16;

/// What a pocket is dealt: more than any pocket holds, so it is full ([`KitLayout::kept`]).
const POCKET_FILL: u32 = 100;

/// Where the item ids of a profile's attachment records start ([`arm`]): over every item id of a
/// profile (its items' are 1 up), as an owned module's is no item of the profile.
const RECORD_IDS: u32 = 1_000_000;

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

/// An ammunition a gun is dealt with, and its clip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ammo {
    pub ammo: u16,
    /// A clip's rounds and the ammo bags it takes.
    clip: u32,
    clip_bags: u32,
}

impl Ammo {
    /// Its rounds for `bags` ammo bags: a clip for each clip's bags.
    fn rounds(self, bags: u32) -> u32 {
        bags / self.clip_bags * self.clip
    }
}

/// The modifications ("bonuses") an item of a hand comes with, rolled once with the hand
/// ([`Self::roll`]); in the order rolled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Bonuses([Option<u32>; MAX_RANK]);

impl Bonuses {
    /// A new item's of `dict_id`, as a drop's: a rarity of the after-match drop table (the item's
    /// own rarity levels), that many modifications of the types the item takes
    /// (`Modifications::roll_for_rarity`, drops' and upgrades' one pool).
    fn roll(dict_id: u16, rng: &mut Rng) -> Self {
        let Some(params) = item_params::item_params(dict_id) else {
            return Self::default();
        };
        let rarity = Drops::shared().generic.gear_rarity(params, rng);
        let mut bonuses = [None; MAX_RANK];
        let rolled = Modifications::shared().roll_for_rarity(dict_id, rarity, rng);
        for (bonus, id) in bonuses.iter_mut().zip(rolled) {
            *bonus = Some(id);
        }
        Self(bonuses)
    }

    /// As an item's own modifications, `(id, defect)`.
    pub fn modifications(&self) -> Vec<(u32, bool)> {
        self.0.iter().flatten().map(|&id| (id, false)).collect()
    }
}

/// A gun of a hand, its bonuses and the ammunition it is dealt with: one, the highest it fires (the
/// owner, 2026-11-22: "the ammo type should be highest available"); a shotgun's of each kind, its
/// buckshot and its slugs ("for shotguns it should be a mix").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gun {
    pub gun: u16,
    pub bonuses: Bonuses,
    pub attachments: Attachments,
    pub ammo: Ammo,
    pub second: Option<Ammo>,
}

impl Gun {
    fn ammo(&self) -> impl Iterator<Item = Ammo> {
        std::iter::once(self.ammo).chain(self.second)
    }
}

/// The kinds a roulette hand is fitted with, in this order: not the second sight (its side would
/// be one more draw; roulette hands had none before second sights were made).
const ROLLED: [Kind; 3] = [Kind::Scope, Kind::Muzzle, Kind::Grip];

/// The modules fitted to a gun of a hand, by kind in [`ROLLED`]'s order (the scope, the muzzle
/// device, the grip): the owner, 2026-11-22: "there should be random applicable attachments on
/// weapons" ([`Self::roll`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Attachments([Option<u16>; ROLLED.len()]);

impl Attachments {
    /// What `gun` is fitted with: for each kind it takes ([`Modules::fits`]), none half the time,
    /// else one of the modules of that kind that fit beside those drawn before it
    /// ([`Modules::check`]), each as likely. A sniper rifle takes no scope: its own stays (the one
    /// its config names).
    fn roll(gun: u16, gear: &[u16], below: &mut impl FnMut(usize) -> usize) -> Self {
        let modules = Modules::shared();
        let sniper = Dictionary::shared()
            .item(gun)
            .is_some_and(|item| item.category == category::SNIPER_RIFLES);
        let fits = modules.fits(gun);
        let mut fitted = Fitted::default();
        let mut attachments = [None; ROLLED.len()];
        for (at, kind) in ROLLED.into_iter().enumerate() {
            if sniper && kind == Kind::Scope {
                continue;
            }
            let candidates: Vec<u16> = fits
                .iter()
                .filter(|&&(module, of)| of == kind && gear.contains(&module))
                .map(|&(module, _)| module)
                .filter(|&module| modules.check(gun, fitted.with(kind, Some(module))).is_ok())
                .collect();
            if candidates.is_empty() || pick(2, below) == 0 {
                continue;
            }
            let module = candidates[pick(candidates.len(), below)];
            fitted = fitted.with(kind, Some(module));
            attachments[at] = Some(module);
        }
        Self(attachments)
    }

    /// The gun's records of them, as an owned module's reach a match (`weapon_kit::pack`, as
    /// Racket's sight): each its module's dict id in its kind's slot, the record's item id from
    /// `ids` (no item of the profile: an owned module is in no kit either).
    fn records(&self, ids: &mut impl FnMut() -> u32) -> Vec<Upgrade> {
        let held: Vec<super::lobby::Upgrade> = ROLLED
            .into_iter()
            .zip(self.0)
            .filter_map(|(kind, module)| {
                Some(super::lobby::Upgrade {
                    id: ids(),
                    a: u32::from(module?),
                    value: 0,
                    param: weapon_modules::slot_of(kind),
                })
            })
            .collect();
        super::weapon_kit::pack(&held)
            .into_iter()
            .map(|record| Upgrade {
                id: record.id,
                unknown_4: record.a,
                unknown_8: record.value,
                param: record.param,
            })
            .collect()
    }
}

/// An armour piece of a hand and its bonuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Piece {
    pub piece: u16,
    pub bonuses: Bonuses,
}

/// One kit of a hand: what every player's kit of its place holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Dealt {
    /// Its one gun, in weapon 1's slot.
    pub main: Gun,
    /// Armour slots 0 to 6; a slot another piece covers is empty.
    pub armour: [Option<Piece>; 7],
    /// The pocket items in the order the pockets take them, each a type of its own.
    pub pockets: [Option<u16>; POCKET_KINDS],
}

impl Dealt {
    /// The attachments of the gun in `slot` (none for any other slot).
    fn attachments_at(&self, slot: usize) -> Attachments {
        match slot == usize::from(MAIN.0) {
            true => self.main.attachments,
            false => Attachments::default(),
        }
    }

    /// The bonuses of what the kit wears in `slot`.
    fn bonuses_at(&self, slot: usize) -> Bonuses {
        match slot {
            slot if slot == usize::from(MAIN.0) => self.main.bonuses,
            _ => self
                .armour
                .get(slot)
                .copied()
                .flatten()
                .map_or_else(Bonuses::default, |piece| piece.bonuses),
        }
    }

    /// Its slots for a player whose kit has the skills' `modifiers`, as `(dict id, units)`: the
    /// armour, the main gun in weapon 1's slot (weapon 2's empty, whatever the kit's carry rule:
    /// one weapon every rule allows), a clip of its ammo for every ammo bag of the kit (a
    /// shotgun's buckshot the first and every other one, its slugs the rest), the pocket items as
    /// many as the kit's pockets open, each pocket full.
    fn slots(&self, modifiers: &[f32; MODIFIERS]) -> [Option<(u16, u32)>; PROFILE_SLOTS] {
        let mut slots = [None; PROFILE_SLOTS];
        for (slot, piece) in self.armour.iter().enumerate() {
            slots[slot] = piece.map(|piece| (piece.piece, 1));
        }
        let (gun_slot, ammo_slots) = MAIN;
        slots[usize::from(gun_slot)] = Some((self.main.gun, 1));
        let worn: Vec<(u32, bool)> = slots
            .iter()
            .enumerate()
            .filter_map(|(slot, held)| Some((slot, (*held)?.0)))
            .flat_map(|(slot, dict_id)| {
                loadouts::default_modifications(dict_id)
                    .into_iter()
                    .chain(self.bonuses_at(slot).modifications())
            })
            .collect();
        let oxygen_tank = slots
            .iter()
            .flatten()
            .any(|&(dict_id, _)| loadouts::is_oxygen_tank(dict_id));
        let layout = KitLayout::new(&worn, modifiers, oxygen_tank);
        let bags = layout.ammo_bags();
        let ammo: Vec<Ammo> = self.main.ammo().collect();
        let shares = match ammo.len() {
            1 => vec![bags],
            _ => vec![bags.div_ceil(2), bags / 2],
        };
        for ((slot, ammo), bags) in ammo_slots.into_iter().zip(ammo).zip(shares) {
            let rounds = ammo.rounds(bags);
            if rounds > 0 {
                slots[usize::from(slot)] = Some((ammo.ammo, rounds));
            }
        }
        for (pocket, kind) in POCKET_SLOTS.into_iter().zip(self.pockets) {
            slots[usize::from(pocket)] = kind.map(|kind| (kind, POCKET_FILL));
        }
        layout.kept(&slots)
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
    /// - a pistol hand [`PISTOL_HANDS_PERCENT`] times in 100 (a band with pistols): each kit a pistol
    ///   of the band's, three different ones while it has them; else three main-gun classes of
    ///   those the band has, each as likely, and a gun of each, each as likely: one a kit;
    /// - each kit its armour slot by slot (a piece is drawn only if no slot it covers is
    ///   filled), a medkit, a grenade and four other pocket items in a drawn order; a kit over a
    ///   player's carry limit draws its armour again, [`WEIGHT_DRAWS`] times at most, then leaves
    ///   its heaviest pieces out until it fits;
    /// - every gun its attachments ([`Attachments::roll`]);
    /// - every gun and armour piece its bonuses ([`Bonuses::roll`]; the owner, 2026-11-22:
    ///   "roulette should also give random bonuses on equipment/weapons"), from a generator
    ///   seeded by a roll.
    pub fn deal(band: Band, below: &mut impl FnMut(usize) -> usize) -> Self {
        let pool = &Pool::shared().bands[band as usize];
        let pistols = !pool.pistols.is_empty() && pick(100, below) < PISTOL_HANDS_PERCENT;
        let mut classes: Vec<&Vec<Gun>> = pool.classes.iter().collect();
        let mut left: Vec<Gun> = Vec::new();
        let mut kits = std::array::from_fn(|_| {
            let main = if pistols {
                // Three different pistols while the band has them, then again from all.
                if left.is_empty() {
                    left = pool.pistols.clone();
                }
                left.remove(pick(left.len(), below))
            } else {
                let class = classes.remove(pick(classes.len(), below));
                class[pick(class.len(), below)]
            };
            pool.kit(main, below)
        });
        for kit in &mut kits {
            kit.main.attachments = Attachments::roll(kit.main.gun, &pool.modules, below);
        }
        let seed = (pick(1 << 31, below) as u64) << 32 | pick(1 << 31, below) as u64;
        let mut rng = Rng::new(seed);
        for kit in &mut kits {
            kit.main.bonuses = Bonuses::roll(kit.main.gun, &mut rng);
            for piece in kit.armour.iter_mut().flatten() {
                piece.bonuses = Bonuses::roll(piece.piece, &mut rng);
            }
        }
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
                kit.main
                    .ammo()
                    .map(|ammo| ammo.ammo)
                    .chain(kit.pockets.into_iter().flatten())
                    .collect::<Vec<u16>>()
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
    /// Each main-gun class the band has, its guns (no bonuses yet).
    classes: Vec<Vec<Gun>>,
    /// The band's pistols, else those of the highest band under it that has some (none is above
    /// level 10): a pistol hand's guns; none, no pistol hand.
    pistols: Vec<Gun>,
    /// By armour slot.
    armour: [Vec<u16>; 7],
    medkits: Vec<u16>,
    grenades: Vec<u16>,
    /// The other pocket items.
    others: Vec<u16>,
    /// The modules a gun may be fitted with (they have no levels: every band's the same).
    modules: Vec<u16>,
}

impl BandPool {
    /// A kit of `main` (its bonuses are the hand's to roll).
    fn kit(&self, main: Gun, below: &mut impl FnMut(usize) -> usize) -> Dealt {
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
                kit.armour[slot].map_or(0.0, |piece| item_params::carried_weight(piece.piece, 1))
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
    fn armour(&self, below: &mut impl FnMut(usize) -> usize) -> [Option<Piece>; 7] {
        let mut armour: [Option<Piece>; 7] = [None; 7];
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
            armour[slot] = Some(Piece {
                piece,
                bonuses: Bonuses::default(),
            });
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
    /// The pools of `dictionary`'s items that are dealt, or what a band lacks for a hand: three
    /// main-gun classes, a medkit, a grenade. The guns, the armour and the modules are
    /// every such item of the dictionary that is no golden or premium copy of another (the owner,
    /// 2026-11-22: the guns no shop sells too, the VSS, the SR-3M, the SOCOM 16); the ammunition
    /// and the pocket items those the shop offers (sold, or a premium item, sold for silver as any).
    fn of(dictionary: &Dictionary) -> Result<Self, String> {
        let gear: Vec<&Item> = dictionary
            .items
            .iter()
            .filter(|item| item.like.is_none())
            .collect();
        let offered: Vec<&Item> = gear
            .iter()
            .copied()
            .filter(|item| item.is_premium || prices::price(item.dict_id).is_some())
            .collect();
        let modules: Vec<u16> = gear
            .iter()
            .filter(|item| Modules::shared().is_module(item.dict_id))
            .map(|item| item.dict_id)
            .collect();
        // The ammunitions the shop offers that `gun` fires and `slot` takes, with their clips.
        let ammo_of = |gun: u16, slot: u8| -> Vec<(u8, u32, Ammo)> {
            offered
                .iter()
                .filter(|ammo| {
                    ammo.is_stack
                        && class(ammo.dict_id) == Some(ItemClass::Ammunition)
                        && dictionary.compatible(gun, ammo.dict_id)
                        && dictionary.fits(ammo.dict_id, slot)
                })
                .filter_map(|ammo| {
                    let params = item_params::item_params(ammo.dict_id)?;
                    (params.clip_size > 0 && params.required_bags > 0).then_some((
                        params.match_making_level,
                        params.pellets,
                        Ammo {
                            ammo: ammo.dict_id,
                            clip: params.clip_size,
                            clip_bags: params.required_bags,
                        },
                    ))
                })
                .collect()
        };
        // The highest of `ammo` that `keep` keeps by its pellets (the lower dict id of two alike).
        let highest = |ammo: &[(u8, u32, Ammo)], keep: &dyn Fn(u32) -> bool| {
            ammo.iter()
                .filter(|&&(_, pellets, _)| keep(pellets))
                .max_by_key(|&&(level, _, ammo)| (level, std::cmp::Reverse(ammo.ammo)))
                .map(|&(_, _, ammo)| ammo)
        };
        // The guns of `category` fitting `slots` in `band`'s levels, each with its ammunition.
        let guns = |category: u8, (gun_slot, ammo_slots): (u8, [u8; 2]), levels: Band| {
            gear
                .iter()
                .filter(|item| {
                    !item.is_stack
                        && item.category == category
                        && dictionary.fits(item.dict_id, gun_slot)
                        && item_params::item_params(item.dict_id).is_some_and(|p| p.gun.is_some())
                        && level(item.dict_id).is_some_and(|level| levels.holds(level))
                })
                .filter_map(|item| {
                    let ammo = ammo_of(item.dict_id, ammo_slots[0]);
                    // A shotgun its highest buckshot and its highest slug, any other gun its
                    // highest round.
                    let (first, second) = if category == category::SHOTGUNS {
                        let buckshot = highest(&ammo, &|pellets| pellets > 1);
                        match (buckshot, highest(&ammo, &|pellets| pellets <= 1)) {
                            (Some(buckshot), slug) => (buckshot, slug),
                            (None, slug) => (slug?, None),
                        }
                    } else {
                        (highest(&ammo, &|_| true)?, None)
                    };
                    Some(Gun {
                        gun: item.dict_id,
                        bonuses: Bonuses::default(),
                        attachments: Attachments::default(),
                        ammo: first,
                        second,
                    })
                })
                .collect::<Vec<Gun>>()
        };
        let pocket = POCKET_SLOTS[0];
        let band = |band: Band| -> Result<BandPool, String> {
            let top = band.top();
            let (low, _) = band.levels();
            let pistols = Band::ALL
                .into_iter()
                .rev()
                .filter(|lower| lower.top() <= top)
                .map(|lower| guns(category::PISTOLS, MAIN, lower))
                .find(|pistols| !pistols.is_empty())
                .unwrap_or_default();
            let classes: Vec<Vec<Gun>> = MAIN_CLASSES
                .into_iter()
                .map(|category| guns(category, MAIN, band))
                .filter(|guns| !guns.is_empty())
                .collect();
            let armour = std::array::from_fn(|slot| {
                gear
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
                modules: modules.clone(),
            };
            let lacks = [
                (pool.classes.len() < KITS, "three main-gun classes"),
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
/// items are the hand's alone: each once (an item, its bonuses and its attachments), held by every
/// kit that has it, whole, its bonuses its own modifications, a gun's attachments its records
/// ([`Attachments::records`], their ids from [`RECORD_IDS`]), ids from 1; a stack's store the
/// largest kit's units of it [`STORE_LIVES`] times.
pub fn arm(profile: &mut MatchProfile, hand: &Hand) {
    let mut items: Vec<(MatchItem, Attachments)> = Vec::new();
    let mut next_record = RECORD_IDS;
    let mut record_id = || {
        next_record += 1;
        next_record
    };
    let dictionary = Dictionary::shared();
    for (n, kit) in profile.kits.iter_mut().enumerate() {
        let dealt = &hand.kits[n % KITS];
        let mut slots = [None; PROFILE_SLOTS];
        for (slot, held) in dealt.slots(&kit.modifiers).into_iter().enumerate() {
            let Some((dict_id, units)) = held else {
                continue;
            };
            let units = u16::try_from(units).unwrap_or(u16::MAX);
            let modifications = dealt.bonuses_at(slot).modifications();
            let attachments = dealt.attachments_at(slot);
            let same = |(item, fitted): &(MatchItem, Attachments)| {
                (item.dict_id, &item.modifications, *fitted) == (dict_id, &modifications, attachments)
            };
            let at = match items.iter().position(same) {
                Some(at) => at,
                None => {
                    let stack = dictionary.is_stack(dict_id);
                    let item = MatchItem {
                        instance_id: items.len() as u32 + 1,
                        dict_id,
                        condition_or_stack: if stack { 0 } else { FULL_CONDITION },
                        modifications,
                        upgrades: attachments.records(&mut record_id),
                        ..MatchItem::default()
                    };
                    items.push((item, attachments));
                    items.len() - 1
                }
            };
            if dictionary.is_stack(dict_id) {
                let store = &mut items[at].0.condition_or_stack;
                *store = (*store).max(u32::from(units).saturating_mul(STORE_LIVES));
            }
            slots[slot] = Some((at as u16, units));
        }
        let name = std::mem::take(&mut kit.name);
        *kit = MatchKit::new(KIT_USABLE, kit.id, name, slots, kit.modifiers);
    }
    profile.items = items.into_iter().map(|(item, _)| item).collect();
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
    /// every gun and armour piece in the band, a profile a match is sent.
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
                // A pistol hand's are pistols alone; any other's of three classes.
                match classes[..] {
                    [category::PISTOLS] => {}
                    _ => assert_eq!(classes.len(), KITS, "{hand:?}"),
                }
                let mut profile = MatchProfile {
                    team: 0,
                    name: "roulette".to_owned(),
                    clan_tag: String::new(),
                    protection: static_info::NO_PROTECTION,
                    kits: (1..=3).map(|id| MatchKit::empty(id, "")).collect(),
                    items: Vec::new(),
                    squad: 0,
                    face: crate::v026g::faces::Face::DEFAULT,
                    notice: crate::v026g::match_msgs::static_info::Notice::Skipped,
                };
                arm(&mut profile, &hand);
                static_info::check_sent(0, &profile, dictionary).unwrap();
                for kit in &profile.kits {
                    let worn = (0..=7).filter_map(|slot| kit.slots[slot]);
                    for (at, _) in worn {
                        let level = level(profile.items[usize::from(at)].dict_id).unwrap();
                        assert!(level <= band.top(), "{hand:?}: level {level}");
                    }
                    assert!(kit.slots[7].is_some(), "{hand:?}");
                    assert_eq!(kit.state, KIT_USABLE);
                }
            }
        }
    }

    /// One gun a kit, in weapon 1's slot, whatever the skills: weapon 2's slot and its ammo slots
    /// empty.
    #[test]
    fn a_kit_holds_its_main_gun_alone() {
        use super::super::skills::slot_modifier::USE_TWO_DOUBLE_HANDED_WEAPONS;
        let hand = Hand::deal(Band::Mid, &mut rolls(7));
        let mut two = [0.0; MODIFIERS];
        two[USE_TWO_DOUBLE_HANDED_WEAPONS] = 1.0;
        for modifiers in [[0.0; MODIFIERS], two] {
            let slots = hand.kits()[0].slots(&modifiers);
            assert_eq!((slots[10], slots[11], slots[12]), (None, None, None));
            assert!(slots[7].is_some());
        }
    }
}

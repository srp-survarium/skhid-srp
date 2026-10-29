//! The grenades in the match (`docs`: `grenades-record` §4): the server's own list of thrown fragmentation
//! grenades, the one thing the bots' view lacked. The match hands over no object states (the worlds' objects
//! section is 5 s apart and carries no body, `fact-grenade-arc.md`), but every throw begins with a key a place
//! presses, and the server sees every place's input, a human's as relayed and a bot's as made.
//!
//! - **The press.** The throw key ([`key::GRENADE`]: the first pocket that holds a grenade) or a quick-use
//!   key whose slot holds one ([`Pockets`], from the place's profile) goes down: the thrower, its team and
//!   the press time are known. The fuse (the grenade's `ttl_ms`) runs from the press, whichever key began the throw (PROVEN,
//!   `fact-grenade-arc.md`). A smoke grenade (`rdg-m`) is no danger and is left out, counted apart. A server
//!   bot's pockets are read from its profile like a human's (`AvoidTuning::bot_pockets`), so a throw key of a
//!   bot with no grenade left lists nothing, as the client throws nothing. The pockets are those of the kit in
//!   use ([`KitPockets`], the worlds say which).
//! - **The launch.** [`LAUNCH_MS`] after the press (LIKELY) the grenade leaves the hand along the thrower's
//!   view: the arc model of `arc.rs` over the map's bullet mesh gives the first contact and where it comes to
//!   rest ([`arc::Landing`]), once, from the body's reckoning at that time. A thrower that is dead by then
//!   threw nothing (counted).
//! - **The blast.** The fuse after the press; the entry is kept [`SPENT_MS`] after it.
//!
//! The list only records and predicts: nothing reads it unless a bot's tuning asks for it
//! (`grenade_avoid`), so a match plays out as before.

use mapdata::Vec3;
use survarium::v026g::loadouts::BotRole;
use survarium::v026g::match_msgs::static_info::MatchProfile;
use survarium::v026g::weapons::is_grenade_item;

use super::arc::{self, Grenade};
use super::pockets::{ProfilePockets, quick_use_order};
use super::view::Place;
use crate::v026g::drive::key;

/// The grenade leaves the hand this long after the press, ms (`arc::LAUNCH_DELAY_S`, LIKELY).
pub const LAUNCH_MS: u32 = (arc::LAUNCH_DELAY_S * 1000.0) as u32;
/// An entry is kept this long past its blast, ms: a poll or two, so a reader at the blast tick still sees it.
pub const SPENT_MS: u32 = 300;
/// A second throw key of one place this soon after the first is the same throw: a quick-use key acts when it
/// goes down and when it comes up, and a held throw key is pressed again (the recordings' rule, 1.25 s).
pub const SAME_THROW_MS: u32 = 1_250;

/// Every key that can begin a throw: the throw key and the six quick-use keys.
pub const THROW_KEYS: u32 = key::GRENADE
    | key::QUICK[0]
    | key::QUICK[1]
    | key::QUICK[2]
    | key::QUICK[3]
    | key::QUICK[4]
    | key::QUICK[5];

/// A grenade thrown: who, for which team, when the key went down, and how far it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiveGrenade {
    pub by: Place,
    pub team: u8,
    /// The match ms of the press.
    pub press: u32,
    pub phase: Phase,
    /// Its numbers (fuse, blast), from the game data: the bots' for a bot's, else its pocket's.
    pub grenade: Grenade,
    /// How the list knows it is a grenade.
    pub basis: Basis,
    /// Whose it is (the counts of the avoidance split by it).
    pub whose: Whose,
}

/// Why a grenade is on the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    /// The thrower's pockets (of the kit in use) held a grenade the key throws, as the client's would.
    Pockets,
    /// A server bot pressed the throw key with no grenade left (or its pockets unknown): the client throws nothing, the
    /// list keeps the entry as it did before the bots' pockets were read (`AvoidTuning::bot_pockets` off). A team with
    /// the switch on skips these in its avoidance (`grenade_threat`).
    AssumedFromBotKey,
}

/// Whose grenade it is, for the counts of the avoidance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Whose {
    /// A human's.
    Human,
    /// A server bot's, its pockets holding it.
    Bot,
    /// A server bot's key press with no grenade behind it ([`Basis::AssumedFromBotKey`]).
    Assumed,
}

impl Whose {
    pub const ALL: [Self; 3] = [Self::Human, Self::Bot, Self::Assumed];

    pub const fn index(self) -> usize {
        match self {
            Self::Human => 0,
            Self::Bot => 1,
            Self::Assumed => 2,
        }
    }
}

impl Whose {
    /// Whose a grenade is that is listed on `basis` from a thrower that is a server bot or not.
    pub fn of(basis: Basis, thrower_is_bot: bool) -> Self {
        match (basis, thrower_is_bot) {
            (Basis::AssumedFromBotKey, _) => Self::Assumed,
            (Basis::Pockets, true) => Self::Bot,
            (Basis::Pockets, false) => Self::Human,
        }
    }
}

/// Where a grenade is in its life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// The key is down, the pin out, the grenade still in the hand: the view may yet turn, so no landing.
    Pressed,
    /// Let go: where it first touches the ground, when, and where it comes to rest (the expected blast).
    Flying {
        launched: u32,
        lands: u32,
        rest: Vec3,
    },
}

impl LiveGrenade {
    /// When it goes off, match ms: the fuse from the press (PROVEN: 80 of 82 recorded blasts at press + 4000 ms
    /// for the RGD-5).
    pub fn blast_at(&self) -> u32 {
        self.press + self.grenade.fuse_ms()
    }

    /// Whether the entry still counts at `clock`: until [`SPENT_MS`] past the blast.
    pub fn live_at(&self, clock: u32) -> bool {
        clock <= self.blast_at() + SPENT_MS
    }
}

/// What a pocket of a profile holds, as far as the throw keys care.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pocketed {
    /// A fragmentation grenade: its blast hurts (the game data's `rgd-5`).
    Frag,
    /// Another grenade item (the game data's `rdg-m` smoke): no blast to keep out of.
    Smoke,
    /// Anything else the quick-use keys count (a medkit, a trap, a booster).
    Other,
}

impl Pocketed {
    pub fn is_grenade(self) -> bool {
        self != Self::Other
    }
}

/// One pocket (profile slot 13..=18): what it holds, and how many units, at the spawn and now; the numbers of the
/// grenade it holds (the bots' for a pocket made without them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pocket {
    pub item: Pocketed,
    /// The slot's stack at a spawn.
    pub full: u16,
    pub left: u16,
    /// Units of the item in storage behind the slot: a spawn fills the slot from it, `min(full, storage)`, and a
    /// death puts what is left back (`inventory::setup_from_profile` 0x2FCB70 and `unload_to_profile` 0x2FD2D0,
    /// PROVEN). A bot's kit has [`BOT_POCKET_STORE`](survarium::v026g::loadouts::BOT_POCKET_STORE) of it, so a life
    /// starts with the stack; with the old kit (storage at the count) the stack is for the match.
    pub store: u32,
    pub grenade: Grenade,
}

/// The storage of a pocket built without the profile's own (tests, and a kit with a store for every life): enough
/// for any number of lives.
const AMPLE_STORE: u32 = 1_000_000;

/// A player's pockets, in slot order: what the throw keys throw (`loadouts::POCKET_SLOTS`,
/// `QUICK_USE_ORDER`, PROVEN): the throw key the first pocket that holds a grenade with units left, quick use
/// `n` the `n`-th pocket that holds an item with units left (the pockets come first in the client's order),
/// and a grenade only if that item is one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pockets {
    slots: [Option<Pocket>; 6],
}

impl Pockets {
    /// From the profile's pocket slots 13..=18 in order: `(item, units)`, an empty or spent slot `None`; with storage
    /// for every life ([`Self::with_storage`] gives the profile's own).
    pub fn new(slots: [Option<(Pocketed, u16)>; 6]) -> Self {
        Self {
            slots: slots.map(|slot| {
                slot.filter(|&(_, units)| units > 0).map(|(item, units)| Pocket {
                    item,
                    full: units,
                    left: units,
                    store: AMPLE_STORE,
                    grenade: *Grenade::bots(),
                })
            }),
        }
    }

    /// The same pockets with the item's storage the profile gives (by slot, the whole stack before the first spawn):
    /// the slot starts filled from it, and a new life fills it from what is left ([`Self::refill`]).
    pub fn with_storage(mut self, stored: [Option<u32>; 6]) -> Self {
        for (slot, stored) in self.slots.iter_mut().zip(stored) {
            if let (Some(pocket), Some(stored)) = (slot.as_mut(), stored) {
                pocket.left = u32::from(pocket.full).min(stored) as u16;
                pocket.store = stored - u32::from(pocket.left);
            }
        }
        self
    }

    /// The same pockets holding the grenades `grenades` (by slot; a slot with none keeps the bots' numbers): a
    /// frag has its own fuse and blast in the game data.
    pub fn with_grenades(mut self, grenades: [Option<Grenade>; 6]) -> Self {
        for (slot, grenade) in self.slots.iter_mut().zip(grenades) {
            if let (Some(pocket), Some(grenade)) = (slot.as_mut(), grenade) {
                pocket.grenade = grenade;
            }
        }
        self
    }

    /// A new life: what is left in the slots goes back to the storage and the slots are filled from it, as the client
    /// does at a death and a spawn.
    pub fn refill(&mut self) {
        for pocket in self.slots.iter_mut().flatten() {
            let storage = pocket.store + u32::from(pocket.left);
            pocket.left = u32::from(pocket.full).min(storage) as u16;
            pocket.store = storage - u32::from(pocket.left);
        }
    }

    /// The fragmentation grenades the pockets hold now (units left in the slots).
    pub fn frags(&self) -> u16 {
        self.slots
            .iter()
            .flatten()
            .filter(|pocket| pocket.item == Pocketed::Frag)
            .map(|pocket| pocket.left)
            .sum()
    }

    /// The index of the pocket the keys `pressed` throw from, if they throw a grenade.
    fn throws_from(&self, pressed: u32) -> Option<usize> {
        let held = |slot: &Option<Pocket>| slot.is_some_and(|pocket| pocket.left > 0);
        if pressed & key::GRENADE != 0 {
            return self
                .slots
                .iter()
                .position(|slot| held(slot) && slot.is_some_and(|pocket| pocket.item.is_grenade()));
        }
        let n = key::QUICK.iter().position(|&bit| pressed & bit != 0)?;
        let i = quick_use_order(|i| held(&self.slots[i])).nth(n)?;
        self.slots[i]
            .is_some_and(|pocket| pocket.item.is_grenade())
            .then_some(i)
    }

    /// The grenade the keys `pressed` (just gone down) throw, if any: one unit is taken.
    pub fn throw(&mut self, pressed: u32) -> Option<Pocketed> {
        self.throw_grenade(pressed).map(|(item, _)| item)
    }

    /// [`Self::throw`] with the numbers of the grenade that was thrown.
    pub fn throw_grenade(&mut self, pressed: u32) -> Option<(Pocketed, Grenade)> {
        let i = self.throws_from(pressed)?;
        let pocket = self.slots[i].as_mut()?;
        pocket.left -= 1;
        Some((pocket.item, pocket.grenade))
    }
}

/// A kit of a profile, parsed from a world's raw index once ([`KitPockets::kit`]): one the profile has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KitIndex(usize);

impl KitIndex {
    pub fn get(self) -> usize {
        self.0
    }
}

/// Both the kit in use and the refill of a new life come from the first trusted world of a life (about 1.75 s after the
/// spawn): a throw before it is judged on the last life's kit and units.
///
/// A place's pockets over all the kits of its profile, and the kit in use: the throw keys act on the pockets of
/// the kit the place spawned in, and the worlds name it (`players::Supplies::kit`, `current_index`: the kit index,
/// LIKELY), not always the profile's first.
#[derive(Clone, Debug, PartialEq)]
pub struct KitPockets {
    /// One set of pockets for each kit, in the profile's order; never empty.
    kits: Vec<Pockets>,
    in_use: KitIndex,
}

impl KitPockets {
    /// The pockets of each kit of a profile, the first in use until a world says otherwise. `None` for a
    /// profile with no kit.
    pub fn new(kits: Vec<Pockets>) -> Option<Self> {
        (!kits.is_empty()).then_some(Self {
            kits,
            in_use: KitIndex(0),
        })
    }

    /// A world's raw kit index parsed against this profile's kits; `None` for a kit it does not have.
    pub fn kit(&self, raw: usize) -> Option<KitIndex> {
        (raw < self.kits.len()).then_some(KitIndex(raw))
    }

    /// The kit in use is `kit` (a world's word, parsed by [`Self::kit`]).
    pub fn use_kit(&mut self, kit: KitIndex) {
        self.in_use = kit;
    }

    /// A new life: every pocket of every kit full again.
    pub fn refill(&mut self) {
        for pockets in &mut self.kits {
            pockets.refill();
        }
    }

    /// The index of the kit in use.
    pub fn in_use(&self) -> KitIndex {
        self.in_use
    }

    /// [`Pockets::throw_grenade`] on the kit in use.
    pub fn throw_grenade(&mut self, pressed: u32) -> Option<(Pocketed, Grenade)> {
        self.kits[self.in_use.0].throw_grenade(pressed)
    }

    /// [`Pockets::frags`] of the kit in use.
    pub fn frags(&self) -> u16 {
        self.kits[self.in_use.0].frags()
    }
}

impl KitPockets {
    /// The pockets of a profile, the one reader of the live server and the bench's stand-in: for each kit (slots
    /// 13..=18) what each pocket holds, a frag (a grenade whose blast hurts, with its own fuse and radius: the game
    /// data's `rgd-5`), a smoke (another grenade item, `rdg-m`) or something else the quick-use keys count, the units
    /// of the slot, and the item's storage behind it (a stack's `condition_or_stack`: what a spawn fills the slot from,
    /// PROVEN, `inventory::setup_from_profile`). `all_kits` off reads the profile's first kit only. `None` for a
    /// profile with no kit.
    pub fn of_profile(profile: &MatchProfile, all_kits: bool) -> Option<Self> {
        let kits = ProfilePockets::of_profile(profile, all_kits)?
            .kits()
            .iter()
            .map(|kit| {
                let slots = kit.map(|pocket| {
                    let pocket = pocket?;
                    let held = match (Grenade::named(pocket.name), is_grenade_item(pocket.name)) {
                        (Some(_), _) => Pocketed::Frag,
                        (None, true) => Pocketed::Smoke,
                        (None, false) => Pocketed::Other,
                    };
                    Some((held, pocket.units))
                });
                let grenades = kit.map(|pocket| Grenade::named(pocket?.name).copied());
                let stored = kit.map(|pocket| pocket?.stored);
                Pockets::new(slots).with_grenades(grenades).with_storage(stored)
            })
            .collect();
        Self::new(kits)
    }
}

impl From<Pockets> for KitPockets {
    /// A profile with the one kit.
    fn from(pockets: Pockets) -> Self {
        Self {
            kits: vec![pockets],
            in_use: KitIndex(0),
        }
    }
}

/// What the tracker knows of a place's throw keys, decided once where the profile is read (`game/driven.rs`):
/// no flag changes what a missing value means.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Throws {
    /// The pockets of every kit: a key lists a grenade only if the kit in use holds one.
    Pockets(KitPockets),
    /// A server bot whose profile has no kit, or whose pockets were not read (the bench's, until it calls
    /// `set_throws`): its throw key is taken for a frag ([`Basis::AssumedFromBotKey`]), as before the pockets were read.
    GrenadeKeyIsFrag,
    /// A human whose profile did not read (or a late joiner): nothing is listed, and the throw keys are counted.
    #[default]
    Unknown,
}

impl Throws {
    /// The fragmentation grenades the place holds now in the kit in use, where its pockets are known; none where they are
    /// not (a bot the bench never gave pockets, a human unread): the grenade logic then goes by its own count, as before.
    pub fn frags_left(&self) -> Option<u16> {
        match self {
            Self::Pockets(pockets) => Some(pockets.frags()),
            Self::GrenadeKeyIsFrag | Self::Unknown => None,
        }
    }

    /// What a place's throw keys throw, decided from its profile with [`KitPockets::of_profile`]: the one reader of the live
    /// server (`game/driven.rs`) and of the bench (which gives the tracker its bots' pockets with `Bots::set_throws`). The
    /// kit in use is the one a world names (`Bots::kit_in_use`, from `players::Supplies::kit`) when `all_kits` is on
    /// (`AvoidTuning::kit_in_use`); with it off only the profile's first kit is read, as before. A server bot's pockets are
    /// read like a human's whatever the switch: `AvoidTuning::bot_pockets` decides only whether a team dodges the grenades
    /// listed from a bot's key alone. A profile with no kit is [`Self::unread`].
    pub fn of_profile(profile: &MatchProfile, all_kits: bool) -> Self {
        match KitPockets::of_profile(profile, all_kits) {
            Some(pockets) => Self::Pockets(pockets),
            None => Self::unread(survarium::v026g::loadouts::bot_role_of(profile).is_some()),
        }
    }

    /// What a place's throw keys mean before its profile is read: a bot's throw key a frag, a human's unknown.
    pub fn unread(bot: bool) -> Self {
        if bot { Self::GrenadeKeyIsFrag } else { Self::Unknown }
    }
}

/// What the tracker made of the throw keys it saw (a counter, for the logs and the benches).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThrowCounts {
    /// Fragmentation grenades listed from the thrower's pockets (a throw key that threw one, a bot's or a human's).
    pub frag: u32,
    /// Grenades listed from a bot's key alone ([`Basis::AssumedFromBotKey`]).
    pub assumed: u32,
    /// ... of them a human's.
    pub human_frag: u32,
    /// ... of a human's, thrown with a quick-use key (not the throw key), and from a kit other than the profile's first.
    pub human_quick: u32,
    pub human_other_kit: u32,
    /// A bot's throw key press with its pockets known and no grenade to throw: the client throws nothing. It is
    /// listed as an assumed grenade ([`Basis::AssumedFromBotKey`]) that a team with `bot_pockets` on does not dodge.
    /// Counts presses, not throws: a press within [`SAME_THROW_MS`] of the last listed one is not counted.
    pub bot_empty: u32,
    /// A human's throw key (or a quick-use key) that found no grenade in the kit in use, and one while the kit is
    /// not known to the profile.
    pub human_empty: u32,
    /// A world named a kit the profile does not have (`Supplies::kit` out of range): kept the kit it had.
    pub unknown_kit: u32,
    /// Smoke grenades: seen, left out.
    pub smoke: u32,
    /// A throw key of a place with no pockets known (a late joiner, or a profile that did not read): not listed.
    pub no_pockets: u32,
    /// Listed and let go, with a landing.
    pub launched: u32,
    /// Let go with no first contact within the flight's time (no ground, or off the map): dropped.
    pub unplaced: u32,
    /// The thrower was dead by the launch: dropped.
    pub died_throwing: u32,
}

impl super::Bots {
    /// A world says `place` is in its kit `kit`: its throw keys act on that kit's pockets
    /// ([`KitPockets::use_kit`]).
    pub fn kit_in_use(&mut self, place: Place, kit: usize) {
        self.tracker.kit_in_use(place, kit);
    }

    /// The match-end log line of the live list: what the throw keys came to, and what the avoidance did with
    /// it (`grenade_threat::AvoidStats` over the bots, by how they knew: told, saw, heard, and by whose grenade it was:
    /// a human's, a bot's from its pockets, one assumed from a bot's key alone). A local test reads from it why the
    /// avoidance acted or did not: no frag listed (a throw not seen: `no pockets`, or a human's key that threw nothing),
    /// none launched (`unplaced`, `died_throwing`), none noticed, or noticed and no way out.
    pub fn grenade_report(&self) -> String {
        let counts = self.tracker.throw_counts();
        let trio = |n: [u32; 3]| format!("{}/{}/{}", n[0], n[1], n[2]);
        let by_team = self.grenade_counts();
        let avoidance = by_team
            .iter()
            .enumerate()
            .map(|(team, by_whose)| {
                let line = Whose::ALL
                    .iter()
                    .map(|whose| {
                        let a = &by_whose[whose.index()];
                        format!(
                            "{whose:?}: noticed {} (a mate's {}) stepped {} already out {} out when it came to act {} no way out {} escaped {} caught {} refused {}",
                            trio(a.noticed),
                            a.noticed_mate,
                            trio(a.stepped),
                            trio(a.held),
                            trio(a.out_at_act),
                            trio(a.no_way),
                            trio(a.escaped),
                            trio(a.caught),
                            a.refused,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                format!("team {team} [{line}]")
            })
            .collect::<Vec<_>>()
            .join(" ");
        let refused = self
            .frag_rolls_refused()
            .iter()
            .enumerate()
            .map(|(team, by_role)| {
                let roles = BotRole::ALL
                    .iter()
                    .zip(by_role)
                    .filter(|&(_, &n)| n > 0)
                    .map(|(role, n)| format!("{} {n}", role.name()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("team {team} [{}]", if roles.is_empty() { "none" } else { &roles })
            })
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "{} frags listed from pockets ({} a human's: {} by a quick-use key, {} from another kit than the first), {} assumed from a bot's key with no grenade left, {} smokes, {} throws with no pockets known, {} human throw keys that found no grenade, {} kits named that the profile lacks, {} launched, {} unplaced, {} thrower dead; avoidance by the bot's team and the grenade's basis (told/saw/heard): {avoidance}; frag rolls refused, none in the kit: {refused}",
            counts.frag,
            counts.human_frag,
            counts.human_quick,
            counts.human_other_kit,
            counts.assumed,
            counts.smoke,
            counts.no_pockets,
            counts.human_empty,
            counts.unknown_kit,
            counts.launched,
            counts.unplaced,
            counts.died_throwing,
        )
    }

    /// Rolls for a grenade the bots refused because their kit held no frag (`CombatTuning::grenade_kit_cap`): `[team][role]`,
    /// by [`BotRole::ALL`], the lives over.
    pub fn frag_rolls_refused(&self) -> [[u32; BotRole::ALL.len()]; 2] {
        let mut out = [[0; BotRole::ALL.len()]; 2];
        for bot in self.bots.iter().flatten() {
            if let Some(role) = bot.role
                && let Some(at) = BotRole::ALL.iter().position(|&each| each == role)
            {
                out[usize::from(bot.team.min(1))][at] += bot.frag_rolls_refused + bot.reflex.refused_no_frag();
            }
        }
        out
    }

    /// What the avoidance of the bots of each team counted, by whose grenade it was ([`Whose::index`]).
    pub fn grenade_counts(&self) -> [[super::grenade_threat::AvoidStats; 3]; 2] {
        let mut by_team = [[super::grenade_threat::AvoidStats::default(); 3]; 2];
        for bot in self.bots.iter().flatten() {
            for (sum, each) in by_team[usize::from(bot.team.min(1))].iter_mut().zip(&bot.features.avoid.by_whose) {
                sum.add(each);
            }
        }
        by_team
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mapdata::World;

    use super::super::body::Fix;
    use super::super::kin::Kinematics;
    use super::super::view::{Clock, Tracker};
    use super::*;

    fn pockets() -> Pockets {
        // Pocket 13 a medkit, 14 two frags, 15 a smoke, 16.. empty.
        Pockets::new([
            Some((Pocketed::Other, 3)),
            Some((Pocketed::Frag, 2)),
            Some((Pocketed::Smoke, 1)),
            None,
            None,
            None,
        ])
    }

    #[test]
    fn the_throw_key_takes_the_first_pocket_with_a_grenade() {
        let mut p = pockets();
        assert_eq!(p.throw(key::GRENADE), Some(Pocketed::Frag));
        assert_eq!(p.throw(key::GRENADE), Some(Pocketed::Frag));
        // The frags are spent: the smoke is the first grenade left.
        assert_eq!(p.throw(key::GRENADE), Some(Pocketed::Smoke));
        assert_eq!(p.throw(key::GRENADE), None);
    }

    #[test]
    fn a_quick_key_throws_only_if_its_slot_is_a_grenade() {
        let mut p = pockets();
        // Quick 1 is the medkit's pocket, quick 2 the frags', quick 3 the smoke's, 4 nothing.
        assert_eq!(p.throw(key::QUICK[0]), None);
        assert_eq!(p.throw(key::QUICK[1]), Some(Pocketed::Frag));
        assert_eq!(p.throw(key::QUICK[2]), Some(Pocketed::Smoke));
        assert_eq!(p.throw(key::QUICK[3]), None);
        // The smoke is spent: it no longer counts, quick 3 is nothing now, and the frag is still quick 2.
        assert_eq!(p.throw(key::QUICK[2]), None);
        assert_eq!(p.throw(key::QUICK[1]), Some(Pocketed::Frag));
        // Both frags thrown: the next pocket moves up, quick 2 is nothing.
        assert_eq!(p.throw(key::QUICK[1]), None);
    }

    #[test]
    fn spent_pockets_close_the_gap_for_quick_use_and_a_new_life_fills_them() {
        let mut p = Pockets::new([
            Some((Pocketed::Frag, 1)),
            Some((Pocketed::Smoke, 1)),
            None,
            None,
            None,
            None,
        ]);
        assert_eq!(p.throw(key::QUICK[0]), Some(Pocketed::Frag));
        // The frag is gone: the smoke is now the first pocket held.
        assert_eq!(p.throw(key::QUICK[0]), Some(Pocketed::Smoke));
        assert_eq!(p.throw(key::GRENADE), None);
        p.refill();
        assert_eq!(p.throw(key::GRENADE), Some(Pocketed::Frag));
    }

    /// A floor at 0, 200 m a side: the map's bullet mesh for the launch.
    fn floor_world() -> Arc<World> {
        let (a, b) = (Vec3::new(-100.0, 0.0, -100.0), Vec3::new(100.0, 0.0, -100.0));
        let (c, d) = (Vec3::new(100.0, 0.0, 100.0), Vec3::new(-100.0, 0.0, 100.0));
        Arc::new(World {
            map: mapdata::MapData::default(),
            grid: Arc::new(mapdata::WalkGrid::build(
                &[],
                [0.0, 0.0, 1.0, 1.0],
                0.25,
                mapdata::Body::default(),
            )),
            sight: Arc::new(mapdata::RayTree::from_corners(vec![[a, b, c], [a, c, d]])),
        })
    }

    fn fix(time: u32, inserted: u32) -> Fix {
        Fix {
            time,
            alive: true,
            inserted,
            feet: Some(Vec3::ZERO),
            yaw: 0.0,
            pitch: 0.0,
            keys: 0,
            speed: Some(0.0),
        }
    }

    /// `(0, bot)` and `(1, human)` standing at the origin facing +z, placed by a world at 500 ms.
    fn stand(pockets: Option<Pockets>) -> Tracker {
        let kin = Kinematics::default();
        let mut tracker = Tracker::new(2, &[0, 1], &[true, false], vec![]);
        for place in 0..2 {
            tracker.entered(place, true);
            tracker.fixed(place, fix(500, 10), Some(1.0), &kin, None);
        }
        tracker.set_throws(vec![Throws::GrenadeKeyIsFrag, pockets.map_or(Throws::Unknown, |p| Throws::Pockets(p.into()))]);
        tracker
    }

    fn press(tracker: &mut Tracker, place: Place, time: u32, keys: u32) {
        tracker.input(place, time, keys, (0.0, 0.0), 0.0, &Kinematics::default(), None);
    }

    #[test]
    fn a_bots_throw_key_is_listed_and_lands_where_the_arc_puts_it_at_the_launch() {
        let world = floor_world();
        let mut tracker = stand(None);
        press(&mut tracker, 0, 1_000, key::GRENADE);
        press(&mut tracker, 0, 1_050, 0);
        let listed = tracker.live_grenades().to_vec();
        assert_eq!(listed.len(), 1);
        assert_eq!((listed[0].by, listed[0].team, listed[0].press), (0, 0, 1_000));
        assert_eq!(listed[0].phase, Phase::Pressed);
        assert_eq!(listed[0].blast_at(), 5_000);
        // Before the launch (press + 1100 ms): still in the hand.
        tracker.launch_grenades(2_099, Some(&*world));
        assert_eq!(tracker.live_grenades()[0].phase, Phase::Pressed);
        // At it: a level view from about 1.3 m over a floor comes down 8 to 9 m ahead, rolls 2 m on.
        tracker.launch_grenades(2_100, Some(&*world));
        let Phase::Flying { launched, lands, rest } = tracker.live_grenades()[0].phase else {
            panic!("launched: {:?}", tracker.live_grenades());
        };
        assert_eq!(launched, 2_100);
        assert!((2_300..2_900).contains(&lands), "{lands}");
        assert!(rest.x.abs() < 0.1 && (9.0..13.0).contains(&rest.z), "{rest:?}");
        assert_eq!(tracker.throw_counts().launched, 1);
        // The view carries it until a moment past the blast.
        let clock = Clock {
            clock: 3_000,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        assert_eq!(tracker.view(&clock, &Kinematics::default(), Some(&*world)).grenades.len(), 1);
        tracker.launch_grenades(5_301, Some(&*world));
        assert!(tracker.live_grenades().is_empty());
    }

    #[test]
    fn a_thrower_dead_by_the_launch_threw_nothing_and_no_map_leaves_it_in_the_hand() {
        let world = floor_world();
        let mut tracker = stand(None);
        press(&mut tracker, 0, 1_000, key::GRENADE);
        tracker.launch_grenades(2_200, None);
        assert_eq!(tracker.live_grenades().len(), 1, "no map: not placed, not dropped");
        let dead = Fix {
            alive: false,
            feet: None,
            speed: None,
            ..fix(1_500, 10)
        };
        tracker.fixed(0, dead, Some(0.0), &Kinematics::default(), None);
        tracker.launch_grenades(2_200, Some(&*world));
        assert!(tracker.live_grenades().is_empty());
        assert_eq!(tracker.throw_counts().died_throwing, 1);
    }

    #[test]
    fn a_humans_quick_keys_list_a_frag_once_a_throw_and_leave_smokes_and_medkits_out() {
        let pockets = Pockets::new([
            Some((Pocketed::Other, 1)),
            Some((Pocketed::Frag, 2)),
            Some((Pocketed::Smoke, 1)),
            None,
            None,
            None,
        ]);
        let mut tracker = stand(Some(pockets));
        // (time, keys): the medkit, a frag, the same key again at once (its release acts too), a frag later.
        for (time, keys) in [
            (1_000, key::QUICK[0]),
            (1_100, 0),
            (2_000, key::QUICK[1]),
            (2_100, 0),
            (2_500, key::QUICK[1]),
            (2_600, 0),
            (4_000, key::QUICK[1]),
            (4_100, 0),
        ] {
            press(&mut tracker, 1, time, keys);
        }
        let presses: Vec<u32> = tracker.live_grenades().iter().map(|g| g.press).collect();
        assert_eq!(presses, [2_000, 4_000]);
        assert!(tracker.live_grenades().iter().all(|g| g.by == 1 && g.team == 1));
        // The frags are spent: the smoke is the second pocket held, quick 2 now.
        press(&mut tracker, 1, 6_000, key::QUICK[1]);
        assert_eq!(tracker.live_grenades().len(), 2);
        let counts = tracker.throw_counts();
        assert_eq!((counts.frag, counts.smoke, counts.no_pockets), (2, 1, 0));
    }

    #[test]
    fn a_humans_smoke_key_becomes_a_cloud_where_the_arc_rests_it_and_the_view_carries_it() {
        use crate::v026g::bots::smoke::{MatchMs, SmokeGrenade};
        let world = floor_world();
        let pockets = Pockets::new([Some((Pocketed::Smoke, 1)), None, None, None, None, None]);
        let mut tracker = stand(Some(pockets));
        press(&mut tracker, 1, 1_000, key::GRENADE);
        press(&mut tracker, 1, 1_050, 0);
        assert!(tracker.live_grenades().is_empty(), "no frag listed");
        assert!(tracker.clouds().is_empty(), "in the hand");
        tracker.launch_grenades(2_100, Some(&*world));
        let clouds = tracker.clouds().to_vec();
        assert_eq!(clouds.len(), 1);
        let cloud = clouds[0];
        assert_eq!((cloud.by, cloud.team, cloud.thrown), (1, 1, MatchMs(1_000)));
        // The fuse from the press (the game data's, else the RDG-M's 4 s), the rest 9-13 m ahead as a frag's.
        let smoke = SmokeGrenade::of_game_data();
        assert_eq!(cloud.pours, MatchMs(1_000 + smoke.cloud.fuse_ms));
        assert!(cloud.centre.x.abs() < 0.1 && (9.0..13.0).contains(&cloud.centre.z), "{cloud:?}");
        let counts = tracker.smoke_counts();
        assert_eq!((counts.thrown, counts.clouds, counts.dropped), ([0, 1], [0, 1], [0, 0]));
        let clock = Clock {
            clock: 6_000,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        assert_eq!(tracker.view(&clock, &Kinematics::default(), Some(&*world)).smoke.all(), &clouds[..]);
        // Gone at its end.
        tracker.launch_grenades(cloud.ends().0, Some(&*world));
        assert!(tracker.clouds().is_empty());
    }

    #[test]
    fn a_world_that_shows_the_smoke_grenade_at_rest_puts_its_cloud_where_the_grenade_is() {
        use crate::v026g::bots::smoke::Placed;
        use survarium::v026g::match_msgs::grenades::{BlastMs, GrenadeBody, GrenadeId, Motion, Velocity, WorldPoint};
        let world = floor_world();
        let pockets = Pockets::new([Some((Pocketed::Smoke, 1)), None, None, None, None, None]);
        let mut tracker = stand(Some(pockets));
        press(&mut tracker, 1, 1_000, key::GRENADE);
        press(&mut tracker, 1, 1_050, 0);
        tracker.launch_grenades(2_100, Some(&*world));
        let predicted = tracker.clouds()[0];
        assert_eq!(predicted.placed, Placed::Arc);
        let body = |blast, id, at: [f32; 3], motion| GrenadeBody {
            blast: BlastMs(blast),
            id: GrenadeId(id),
            place: WorldPoint(at),
            motion,
        };
        // Another grenade's blast (2 s later): not this cloud.
        tracker.grenade_bodies(&[body(predicted.pours.0 + 2_000, 8, [3.0, 0.4, 20.0], Motion::Resting)], Some(&*world), 4_000);
        assert_eq!(tracker.clouds()[0], predicted);
        // Its own, still flying at 15 m/s: the arc's rest stands (a moving body is not where it stops).
        let flying = Motion::Moving(Velocity([15.0, 0.0, 0.0]));
        let seen = [predicted.centre.x + 6.0, 0.4, predicted.centre.z];
        tracker.grenade_bodies(&[body(predicted.pours.0 + 50, 7, seen, flying)], Some(&*world), 3_000);
        assert_eq!(tracker.clouds()[0], predicted);
        // At rest, on the 50 ms grid 50 ms after the press's fuse, 0.4 m over the floor 6 m off the arc's rest.
        tracker.grenade_bodies(&[body(predicted.pours.0 + 50, 7, seen, Motion::Resting)], Some(&*world), 4_000);
        let cloud = tracker.clouds()[0];
        assert_eq!(cloud.placed, Placed::Seen { world_ms: 4_000, id: GrenadeId(7) });
        assert!((cloud.centre.x - seen[0]).abs() < 1e-4 && cloud.centre.y.abs() < 1e-3, "{cloud:?}");
        let counts = tracker.smoke_counts();
        assert_eq!((counts.bodies_seen, counts.moved_dm), ([0, 1], [0, 60]));
        assert!(tracker.take_notes().iter().any(|n| n.contains("put on its grenade 7 at rest")));
        // A later sighting of the same body (it rolled on 1.5 m) follows it by its id; the arc's error is not counted again.
        let rolled = [seen[0] + 1.5, 0.4, seen[2]];
        tracker.grenade_bodies(&[body(predicted.pours.0 + 50, 7, rolled, Motion::Resting)], Some(&*world), 5_000);
        assert!((tracker.clouds()[0].centre.x - rolled[0]).abs() < 1e-4);
        assert_eq!(tracker.smoke_counts().moved_dm, [0, 60]);
    }

    #[test]
    fn a_frag_going_off_with_the_smoke_keeps_its_own_body_from_the_smokes_cloud() {
        use crate::v026g::bots::smoke::Placed;
        use survarium::v026g::match_msgs::grenades::{BlastMs, GrenadeBody, GrenadeId, Motion, WorldPoint};
        let world = floor_world();
        // The bot throws a frag and the human a smoke at once: both go off at press + 4 s, 50 ms apart.
        let pockets = Pockets::new([Some((Pocketed::Smoke, 1)), None, None, None, None, None]);
        let mut tracker = stand(Some(pockets));
        press(&mut tracker, 0, 1_000, key::GRENADE);
        press(&mut tracker, 1, 1_050, key::GRENADE);
        press(&mut tracker, 0, 1_100, 0);
        press(&mut tracker, 1, 1_150, 0);
        tracker.launch_grenades(2_200, Some(&*world));
        let cloud = tracker.clouds()[0];
        let Phase::Flying { rest: frag_rest, .. } = tracker.live_grenades()[0].phase else {
            panic!("{:?}", tracker.live_grenades());
        };
        // The frag's body, at rest beside the frag's predicted rest, its blast within 100 ms of the smoke's.
        let frag_body = GrenadeBody {
            blast: BlastMs(cloud.pours.0 - 50),
            id: GrenadeId(9),
            place: WorldPoint([frag_rest.x + 0.3, 0.2, frag_rest.z]),
            motion: Motion::Resting,
        };
        // Both stand at the origin facing +z: the two rests are near; move the smoke's prediction off by 4 m so the
        // frag's body is nearer the frag's rest.
        let mut moved = cloud;
        moved.centre.x += 4.0;
        tracker.clouds_mut_for_test()[0] = moved;
        tracker.grenade_bodies(&[frag_body], Some(&*world), 4_000);
        assert_eq!(tracker.clouds()[0].placed, Placed::Arc, "the frag's body moved the smoke");
    }

    #[test]
    fn a_new_life_fills_the_pockets_and_a_human_with_none_known_lists_nothing() {
        let kin = Kinematics::default();
        let one_frag = Pockets::new([Some((Pocketed::Frag, 1)), None, None, None, None, None]);
        let mut tracker = stand(Some(one_frag));
        press(&mut tracker, 1, 1_000, key::GRENADE);
        press(&mut tracker, 1, 1_050, 0);
        press(&mut tracker, 1, 3_000, key::GRENADE);
        press(&mut tracker, 1, 3_050, 0);
        assert_eq!(tracker.live_grenades().len(), 1, "one frag, one throw");
        tracker.fixed(1, fix(4_000, 11), Some(1.0), &kin, None);
        press(&mut tracker, 1, 6_000, key::GRENADE);
        assert_eq!(tracker.live_grenades().len(), 2, "a new life, a full pocket");
        let mut tracker = Tracker::new(2, &[0, 1], &[true, false], vec![]);
        tracker.fixed(1, fix(500, 10), Some(1.0), &kin, None);
        press(&mut tracker, 1, 1_000, key::GRENADE);
        assert!(tracker.live_grenades().is_empty());
        assert_eq!(tracker.throw_counts().no_pockets, 1);
    }

    #[test]
    fn an_entry_is_live_to_a_moment_past_its_blast() {
        let g = LiveGrenade {
            by: 0,
            team: 0,
            press: 1_000,
            phase: Phase::Pressed,
            grenade: *Grenade::bots(),
            basis: Basis::Pockets,
            whose: Whose::Human,
        };
        assert_eq!(g.blast_at(), 5_000);
        // A grenade with another fuse goes off by its own (a patched RGD-5, or another frag).
        let short = LiveGrenade {
            grenade: Grenade {
                fuse_s: 3.0,
                ..*Grenade::bots()
            },
            ..g
        };
        assert_eq!(short.blast_at(), 4_000);
        assert!(short.live_at(4_300) && !short.live_at(4_301));
        assert!(g.live_at(5_300));
        assert!(!g.live_at(5_301));
    }

    /// `(0, bot)` and `(1, human)` with the pockets given, a bot's now read like a human's.
    fn stand_pockets(bot: Option<Pockets>, human: Option<KitPockets>) -> Tracker {
        let kin = Kinematics::default();
        let mut tracker = Tracker::new(2, &[0, 1], &[true, false], vec![]);
        for place in 0..2 {
            tracker.entered(place, true);
            tracker.fixed(place, fix(500, 10), Some(1.0), &kin, None);
        }
        tracker.set_throws(vec![bot.map_or(Throws::GrenadeKeyIsFrag, |p| Throws::Pockets(p.into())), human.map_or(Throws::Unknown, Throws::Pockets)]);
        tracker
    }

    #[test]
    fn a_bot_lists_a_frag_from_its_pockets_and_an_empty_press_only_as_an_assumed_one() {
        let one_frag = Pockets::new([None, None, Some((Pocketed::Frag, 1)), None, None, None]);
        let mut tracker = stand_pockets(Some(one_frag), None);
        press(&mut tracker, 0, 1_000, key::GRENADE);
        press(&mut tracker, 0, 1_050, 0);
        assert_eq!(tracker.live_grenades().len(), 1);
        assert_eq!(tracker.live_grenades()[0].basis, Basis::Pockets);
        assert_eq!(tracker.live_grenades()[0].whose, Whose::Bot);
        // The one grenade is gone: the key throws nothing, as the client's. The list keeps the press as an
        // assumed grenade (a team with `bot_pockets` on does not dodge it), apart from the real one.
        press(&mut tracker, 0, 3_000, key::GRENADE);
        press(&mut tracker, 0, 3_050, 0);
        assert_eq!(tracker.live_grenades().len(), 2);
        assert_eq!(tracker.live_grenades()[1].basis, Basis::AssumedFromBotKey);
        assert_eq!(tracker.live_grenades()[1].whose, Whose::Assumed);
        let counts = tracker.throw_counts();
        assert_eq!((counts.frag, counts.human_frag, counts.bot_empty, counts.assumed), (1, 0, 1, 1));
        // A kit with no grenade (the rushers', flankers' and campers'): the throw key is an assumed one, a quick-use key
        // nothing.
        let medkit = Pockets::new([Some((Pocketed::Other, 2)), None, None, None, None, None]);
        let mut tracker = stand_pockets(Some(medkit), None);
        press(&mut tracker, 0, 1_000, key::GRENADE);
        press(&mut tracker, 0, 1_050, 0);
        press(&mut tracker, 0, 2_500, key::QUICK[0]);
        assert_eq!(tracker.live_grenades().len(), 1);
        assert_eq!(tracker.live_grenades()[0].basis, Basis::AssumedFromBotKey);
        assert_eq!(tracker.throw_counts().bot_empty, 1, "a quick-use key is no throw key press");
        // A bot whose profile has no kit: every throw key an assumed grenade.
        let mut tracker = stand_pockets(None, None);
        press(&mut tracker, 0, 1_000, key::GRENADE);
        assert_eq!(tracker.live_grenades()[0].basis, Basis::AssumedFromBotKey);
    }

    /// A place whose profile was never read (the bench's bots, which never call `set_throws`, a human before the lobby's
    /// profile): a bot's throw key is the old assumed frag, as at the tip; a human's is unknown and counted.
    #[test]
    fn a_place_nothing_was_told_of_has_its_throw_key_by_its_kind() {
        let kin = Kinematics::default();
        let mut tracker = Tracker::new(2, &[0, 1], &[true, false], vec![]);
        for place in 0..2 {
            tracker.entered(place, true);
            tracker.fixed(place, fix(500, 10), Some(1.0), &kin, None);
        }
        assert_eq!(Throws::unread(true), Throws::GrenadeKeyIsFrag);
        assert_eq!(Throws::unread(false), Throws::Unknown);
        press(&mut tracker, 0, 1_000, key::GRENADE);
        press(&mut tracker, 1, 1_000, key::GRENADE);
        assert_eq!(tracker.live_grenades().len(), 1, "the bot's key lists a grenade, the human's nothing");
        assert_eq!(tracker.live_grenades()[0].by, 0);
        assert_eq!(tracker.live_grenades()[0].basis, Basis::AssumedFromBotKey);
        let counts = tracker.throw_counts();
        assert_eq!((counts.assumed, counts.no_pockets), (1, 1));
    }

    /// The listing decisions are noted only while one of the list's switches is on, so a run with them off writes the notes
    /// (and the decisions digest) it always did.
    #[test]
    fn the_listing_is_noted_only_while_a_switch_of_the_list_is_on() {
        let one_frag = || Some(Pockets::new([None, None, Some((Pocketed::Frag, 2)), None, None, None]));
        let mut tracker = stand_pockets(one_frag(), None);
        press(&mut tracker, 0, 1_000, key::GRENADE);
        press(&mut tracker, 0, 1_050, 0);
        assert_eq!(tracker.live_grenades().len(), 1);
        assert!(tracker.take_notes().is_empty(), "off: nothing noted");
        tracker.note_listings(true);
        press(&mut tracker, 0, 3_000, key::GRENADE);
        let notes = tracker.take_notes();
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(notes[0].starts_with("grenade listed: place 0 team 0") && notes[0].contains("Pockets, Bot"), "{notes:?}");
        // A human's throw key that found no grenade is noted the same way.
        let mut tracker = stand_pockets(None, Some(Pockets::new([Some((Pocketed::Other, 2)), None, None, None, None, None]).into()));
        tracker.note_listings(true);
        press(&mut tracker, 1, 1_000, key::GRENADE);
        assert!(tracker.take_notes()[0].starts_with("throw key found no grenade: place 1"));
    }

    /// The grenade logic caps its rolls by the frags the kit holds now (`Throws::frags_left`): a frag is counted, a smoke and a
    /// medkit are not, a thrown one is gone, a new life brings the stack back, and a kit nobody read has none counted.
    #[test]
    fn the_frags_a_kit_holds_are_the_frags_left_in_the_kit_in_use() {
        let pockets = Pockets::new([Some((Pocketed::Other, 2)), Some((Pocketed::Frag, 2)), Some((Pocketed::Smoke, 1)), None, None, None]);
        let mut throws = Throws::Pockets(pockets.into());
        assert_eq!(throws.frags_left(), Some(2), "the smoke and the medkit are no frags");
        let Throws::Pockets(kit) = &mut throws else { unreachable!() };
        kit.throw_grenade(key::GRENADE);
        assert_eq!(throws.frags_left(), Some(1));
        let Throws::Pockets(kit) = &mut throws else { unreachable!() };
        kit.throw_grenade(key::GRENADE);
        assert_eq!(throws.frags_left(), Some(0));
        let medkits = Throws::Pockets(Pockets::new([Some((Pocketed::Other, 2)), None, None, None, None, None]).into());
        assert_eq!(medkits.frags_left(), Some(0), "a trap role's kit holds none");
        assert_eq!((Throws::GrenadeKeyIsFrag.frags_left(), Throws::Unknown.frags_left()), (None, None));
    }

    /// The client fills a stack slot from the item's storage at a spawn and takes what is left back at a death
    /// (PROVEN). A bot's kit with the storage at the count (the old kit) has its one grenade for the match; with the store
    /// of 100 every life has it.
    #[test]
    fn a_new_life_has_the_grenade_again_only_if_the_storage_has_it() {
        let kin = Kinematics::default();
        let frag = || Pockets::new([Some((Pocketed::Frag, 1)), None, None, None, None, None]);
        let lives = |stored: u32| {
            let pockets = frag().with_storage([Some(stored), None, None, None, None, None]);
            let mut tracker = stand_pockets(Some(pockets), None);
            let mut listed = Vec::new();
            for life in 0..3u32 {
                if life > 0 {
                    tracker.fixed(0, fix(4_000 * life, 10 + life), Some(1.0), &kin, None);
                }
                let before = tracker.throw_counts().frag;
                press(&mut tracker, 0, 1_000 + 10_000 * life, key::GRENADE);
                press(&mut tracker, 0, 1_050 + 10_000 * life, 0);
                listed.push(tracker.throw_counts().frag - before);
            }
            listed
        };
        assert_eq!(lives(1), [1, 0, 0], "the storage at the count: for the match");
        assert_eq!(lives(100), [1, 1, 1], "the store of 100: every life");
        assert_eq!(lives(2), [1, 1, 0], "two in all");
        // An unthrown grenade goes back to the storage at the death: it is not lost.
        let pockets = Pockets::new([Some((Pocketed::Frag, 2)), None, None, None, None, None])
            .with_storage([Some(3), None, None, None, None, None]);
        let mut p = pockets;
        assert_eq!(p.throw_grenade(key::GRENADE).map(|t| t.0), Some(Pocketed::Frag));
        p.refill();
        assert_eq!(p.throw_grenade(key::GRENADE).map(|t| t.0), Some(Pocketed::Frag));
        assert_eq!(p.throw_grenade(key::GRENADE).map(|t| t.0), Some(Pocketed::Frag));
        assert_eq!(p.throw_grenade(key::GRENADE), None, "3 units in all: 1 + 2");
    }

    #[test]
    fn a_humans_throw_acts_on_the_kit_the_world_says_he_is_in() {
        // Kit 0 holds a medkit, kit 1 two frags: a player who spawned in kit 1 throws; asked of kit 0 nothing is.
        let none = Pockets::new([Some((Pocketed::Other, 1)), None, None, None, None, None]);
        let frags = Pockets::new([Some((Pocketed::Frag, 2)), None, None, None, None, None]);
        let kits = KitPockets::new(vec![none, frags]).expect("two kits");
        let mut tracker = stand_pockets(None, Some(kits.clone()));
        press(&mut tracker, 1, 1_000, key::GRENADE);
        press(&mut tracker, 1, 1_050, 0);
        assert!(tracker.live_grenades().is_empty(), "the first kit holds no grenade");
        let mut tracker = stand_pockets(None, Some(kits));
        tracker.kit_in_use(1, 1);
        press(&mut tracker, 1, 1_000, key::GRENADE);
        press(&mut tracker, 1, 1_050, 0);
        assert_eq!(tracker.live_grenades().len(), 1);
        assert_eq!(tracker.throw_counts().human_frag, 1);
        // A kit the profile does not have changes nothing.
        tracker.kit_in_use(1, 7);
        assert_eq!(tracker.throw_counts().unknown_kit, 1);
        press(&mut tracker, 1, 3_000, key::GRENADE);
        press(&mut tracker, 1, 3_050, 0);
        assert_eq!(tracker.live_grenades().len(), 2);
        assert!(KitPockets::new(Vec::new()).is_none());
    }

    /// The owner throws with G and with a quick-use number, from any kit (2026-10-29): every throw is detected from the
    /// key the server sees (the client's throw is the key, PROVEN `loadouts::GRENADE_BIT`, `QUICK_USE_ORDER`) against the pockets of the kit in
    /// use. Kit 0 holds a medkit; kit 1 a medkit in pocket 13 and two frags in pocket 14, so its frags are quick use 2.
    #[test]
    fn a_quick_use_throw_from_a_second_kit_is_listed_and_counted_by_key_and_kit() {
        let first = Pockets::new([Some((Pocketed::Other, 1)), None, None, None, None, None]);
        let second = Pockets::new([
            Some((Pocketed::Other, 1)),
            Some((Pocketed::Frag, 2)),
            None,
            None,
            None,
            None,
        ]);
        let kits = KitPockets::new(vec![first, second]).expect("two kits");
        let mut tracker = stand_pockets(None, Some(kits));
        // Spawned in kit 1 (a world says so): quick use 2 is the frag, quick use 1 the medkit (no throw).
        tracker.kit_in_use(1, 1);
        press(&mut tracker, 1, 1_000, key::QUICK[0]);
        press(&mut tracker, 1, 1_050, 0);
        assert!(tracker.live_grenades().is_empty(), "the medkit's key is no throw");
        press(&mut tracker, 1, 2_000, key::QUICK[1]);
        press(&mut tracker, 1, 2_050, 0);
        assert_eq!(tracker.live_grenades().len(), 1);
        // G throws the second one from the same kit.
        press(&mut tracker, 1, 4_000, key::GRENADE);
        press(&mut tracker, 1, 4_050, 0);
        assert_eq!(tracker.live_grenades().len(), 2);
        let counts = tracker.throw_counts();
        assert_eq!((counts.frag, counts.human_frag, counts.human_quick, counts.human_other_kit), (2, 2, 1, 2));
        // Back in kit 0 after a death (a new life, the world says kit 0): neither key lists anything.
        tracker.fixed(1, fix(6_000, 11), Some(1.0), &Kinematics::default(), None);
        tracker.kit_in_use(1, 0);
        press(&mut tracker, 1, 7_000, key::GRENADE);
        press(&mut tracker, 1, 7_050, 0);
        press(&mut tracker, 1, 8_000, key::QUICK[1]);
        press(&mut tracker, 1, 8_050, 0);
        assert_eq!(tracker.live_grenades().len(), 2);
        assert_eq!(tracker.throw_counts().human_frag, 2);
    }
}

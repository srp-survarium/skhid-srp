//! What a bot has to use: the drugs of its kits, read as the HUD shows a
//! player (`player_bots_behaviour.md` §3 play 22; the record's §8.8).
//!
//! A kit's pocket holds a stack (the bots' kits: two medkits). The game
//! keeps it twice (`inventory::setup_from_profile` 0x2FCB70, PROVEN, see
//! `survarium::v026g::loadouts::BOT_AMMO_LIVES`): the stack in the slot
//! (`m_amount`, what a quick-use key takes from) and the item's storage;
//! a spawn fills the slot from the storage, a use takes one from the slot
//! at once, a death puts the slot back. So with the storage at the count
//! ([`PocketStore::PerMatch`]) a kit's drugs are a count for the match, not
//! for a life, and a bot that counted them per life pressed the third
//! medkit of a kit into an empty slot. With the store of
//! [`PocketStore::PerLife`] (the default since 2026-10-29) the storage holds
//! [`BOT_POCKET_STORE`] units, so every life starts with the stack and the
//! match's count is the store's; the worlds correct it either way.
//!
//! [`Pantry`] is that count, a bot's own. A press takes one out of it
//! ([`Pantry::pressed`]); a trusted world corrects it ([`Pantry::read`]),
//! from the player's block as `players::Supplies` reads it ([`readings`]):
//! the stack in the slot and the storage of each drug of each kit, and the
//! quick-use key the kit's slot order gives it. A world made before the
//! game could have applied a press says nothing of that press
//! ([`PRESS_LAG_MS`]), and one made before the bot's first world of a life
//! is no word on that life's slot ([`Pantry::new_life`] fills it from the
//! count).

use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::loadouts::{
    BOT_BANDAGES, BOT_MEDKIT, BOT_POCKETS, BotKitItem, QUICK_USE_FIRST_BIT, QUICK_USE_ORDER, new_kit,
    PocketStore,
};
use survarium::v026g::match_msgs::players::Supplies;
use survarium::v026g::match_msgs::static_info::MatchProfile;

use super::pockets::{ProfilePockets, quick_use_of};

/// A press is in a world made this long after it, ms: the input goes a
/// tick or two ahead of the match clock to the client, which applies it at
/// its tick (`game/driven.rs`, inputs 50 to 100 ms ahead).
pub const PRESS_LAG_MS: u32 = 500;

/// The quick-use keys there are (`key::QUICK`).
const QUICK_KEYS: usize = 6;

/// What a pocket's item does for the body (the client's `drugs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Drug {
    Medkit,
    Bandages,
    Antitoxin,
    /// The military medkit (`loadouts::KitMedkit::Military`).
    Military,
}

impl Drug {
    pub const ALL: [Self; 4] = [Self::Medkit, Self::Bandages, Self::Antitoxin, Self::Military];

    /// The dictionary's name of the item.
    pub fn name(self) -> &'static str {
        match self {
            Self::Medkit => "medkit",
            Self::Bandages => "bandages",
            Self::Antitoxin => "antitoxin",
            Self::Military => survarium::v026g::loadouts::BOT_MILITARY_MEDKIT_NAME,
        }
    }

    pub fn of_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|drug| drug.name() == name)
    }
}

/// [F65] The client's caps of modifier 43's three-bit rank fields, (first bit, most ranks): Painkiller, Fast Bandaging,
/// Metabolism, Medical Training, Adrenalin, Bones, Brothers (`medical_skills.rs` `Field::new(shift, bits, most)`; the test
/// `the_medical_numbers_are_the_client_patchs` reads them from that file).
pub const MEDICAL_CAPS: [(u32, u32); 7] = [(0, 3), (3, 2), (6, 5), (9, 3), (12, 3), (17, 5), (21, 2)];
/// [F65] Fast Bandaging's activation delay off a rank (`Skills::use_time`: native x (1 - rank x 0.05)).
pub const BANDAGING_PER_RANK: f32 = 0.05;
/// [F65] Metabolism's head start a rank, ms (`Skills::regeneration_delay_ms`: native - rank x 500).
pub const METABOLISM_PER_RANK_MS: u32 = 500;

/// [A341] What a bot's medical skills do to its drugs, read once from its kit's modifiers (the client's own reading of
/// them, so the bots' heal model times and sizes a drug as the game will):
/// - `delay`: a healing drug's activation delay times this: Fast Bandaging's rank (302, bits 3..5 of modifier 43
///   `armored_trap`, `client-patches/rust/patches/src/medical_skills.rs` `Skills::parse`) gives `1 - 0.05 x rank`
///   ([`BANDAGING_PER_RANK`]; the client's since the tree's rev 50, two ranks)
///   (`Skills::use_time`, applied at `medical.use_time` 0x33D486 to XMM0, the activation delay: `sites_026g.tsv`);
/// - `amount`: every amount times `max(0, 1 + drug_amount)` (modifier 57, Medical Training 312's
///   `drug_amount_modifier`; the getter's form `B x max(0, 1 + k S)`, 0x2D5AE0; k = 1 for it is LIKELY, the constructor
///   0x33CAC0's call of modifier 57 is the read that proves it: `docs/notes/bandages.md`);
/// - `regeneration_sooner_ms`: natural regeneration starts this much sooner, Metabolism (311, bits 6..8) x 500 ms
///   (`Skills::regeneration_delay_ms`).
///
/// A value modifier 43 cannot hold (not an exact integer of 24 bits, a rank over 5) is read as no skill, as the client
/// refuses it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrugFactors {
    pub delay: f32,
    pub amount: f32,
    pub regeneration_sooner_ms: u32,
}

impl DrugFactors {
    /// No medical skill.
    pub const NONE: Self = Self { delay: 1.0, amount: 1.0, regeneration_sooner_ms: 0 };

    /// The factors `modifiers` (a kit's 68) give: none when a rank is over the client's cap ([`MEDICAL_CAPS`]), as the client
    /// reads it (`Skills::parse` refuses the whole set, `vostok/medical.rs` takes no skill).
    pub fn of_modifiers(modifiers: &[f32; survarium::v026g::skills::MODIFIERS]) -> Self {
        let at = |name| survarium::v026g::skills::modifier_index(name).expect("a modifier of the 68");
        let packed = modifiers[at("armored_trap")];
        let ranks = (packed.is_finite() && (0.0..=16_777_215.0).contains(&packed) && packed == packed.trunc())
            .then_some(packed as u32)
            .filter(|bits| MEDICAL_CAPS.iter().all(|&(shift, most)| (bits >> shift) & 7 <= most));
        let rank = |shift: u32| ranks.map_or(0, |bits| (bits >> shift) & 7);
        Self {
            delay: 1.0 - rank(3) as f32 * BANDAGING_PER_RANK,
            amount: (1.0 + modifiers[at("drug_amount")]).max(0.0),
            regeneration_sooner_ms: rank(6) * METABOLISM_PER_RANK_MS,
        }
    }

    /// The factors of `profile`'s first kit (a bot's kits share its tier's skills); none for a profile with no kit.
    pub fn of_profile(profile: &MatchProfile) -> Self {
        profile.kits.first().map_or(Self::NONE, |kit| Self::of_modifiers(&kit.modifiers))
    }
}

impl Default for DrugFactors {
    fn default() -> Self {
        Self::NONE
    }
}

/// A drug of a kit.
#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    pub drug: Drug,
    /// Its quick-use key, 0 on (`key::QUICK[quick]`): the kit's filled
    /// slots in `QUICK_USE_ORDER`, this one's place among them.
    pub quick: usize,
    /// Units a life's slot starts with (the pocket's).
    pub slot: u16,
    /// Units left for the match, in the slot and in store.
    pub total: u16,
    /// Units in the slot now (this life).
    pub hand: u16,
    /// The last press of this life.
    pub pressed: Option<Press>,
}

/// A press of a drug's key, and what the worlds said of it since.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Press {
    /// When it went out, ms.
    pub at: u32,
    /// The first hit that took health after it, the world's ms (`m_last_hit_time_ms`, written only for a hit
    /// that took health: `damage_model::hit_body_part` 0x2E5134, PROVEN). Patch 84 stops the drug if it landed
    /// before the drug's end (`HealTuning::drug_stops`). The world keeps only the last hit, so a hit inside the
    /// drug's time followed by another after its end, both between two worlds, reads as the later one; and a hit
    /// that is no bullet (a zone, a fall, a blast) stops nothing in the game but is read as one (LIKELY rare
    /// within a drug's few seconds).
    pub hit: Option<u32>,
}

/// One drug of one kit in a world, as [`readings`] has it.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub kit: usize,
    pub drug: Drug,
    pub quick: usize,
    /// The pocket's count.
    pub slot: u16,
    /// The kit is the one in use.
    pub in_use: bool,
    /// The stack in the slot, when the kit is in use and the slot was
    /// found.
    pub in_hand: Option<u16>,
    /// The item's storage.
    pub stored: u32,
}

/// What a world says of a player's drugs.
#[derive(Clone, Debug, PartialEq)]
pub struct Readings {
    /// The kit in use, and how many kits the profile has.
    pub kit: usize,
    pub kits: usize,
    /// `m_is_alive`: a dead player's slots are back in store.
    pub alive: bool,
    pub drugs: Vec<Reading>,
}

/// The drugs each of a player's kits holds in `supplies`, with the keys of
/// the kit's own slot order. Only what the profile's kits have.
pub fn readings(supplies: &Supplies, profile: &MatchProfile, dictionary: &Dictionary) -> Readings {
    let mut out = Vec::new();
    for (k, kit) in profile.kits.iter().enumerate() {
        let filled: Vec<u8> = QUICK_USE_ORDER
            .iter()
            .copied()
            .filter(|&slot| kit.slots[usize::from(slot)].is_some_and(|(_, count)| count > 0))
            .collect();
        for (quick, &slot) in filled.iter().enumerate().take(QUICK_KEYS) {
            let Some((index, count)) = kit.slots[usize::from(slot)] else {
                continue;
            };
            let Some(item) = profile.items.get(usize::from(index)) else {
                continue;
            };
            let Some(drug) = dictionary
                .item(item.dict_id)
                .and_then(|row| Drug::of_name(&row.name))
            else {
                continue;
            };
            let Some(left) = supplies.items.get(usize::from(index)) else {
                continue;
            };
            let in_use = usize::from(supplies.kit) == k;
            out.push(Reading {
                kit: k,
                drug,
                quick,
                slot: count,
                in_use,
                in_hand: (in_use && supplies.alive && supplies.inventory_read)
                    .then_some(left.in_hand)
                    .flatten(),
                stored: left.stored,
            });
        }
    }
    Readings {
        kit: usize::from(supplies.kit),
        kits: profile.kits.len(),
        alive: supplies.alive,
        drugs: out,
    }
}

/// A bot's drugs, by kit.
#[derive(Clone, Debug, PartialEq)]
pub struct Pantry {
    kits: Vec<Vec<Held>>,
    /// The kit in use.
    kit: usize,
    /// What a kit the profile does not have is taken to hold, until a world says.
    unknown: Vec<Held>,
    counts: PantryCounts,
    /// What its medical skills do to its drugs (none but a bot's own profile's).
    factors: DrugFactors,
}

/// What a pantry did, for the logs and the bench (per bot; the bench sums them by team).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PantryCounts {
    /// Lives begun ([`Pantry::new_life`]).
    pub spawns: u32,
    /// Lives that began with a drug's slot below its pocket's units: the store held less than the stack, so the
    /// life started short. None with the store of [`PocketStore::PerLife`].
    pub short_spawns: u32,
    /// Units pressed, by drug ([`Drug::ALL`] order), and presses into an empty slot (the game does nothing then).
    pub pressed: [u32; Drug::ALL.len()],
    pub empty_presses: u32,
}

impl PantryCounts {
    /// The counts as a log line: `spawns 12 (short 0), medkits 5, bandages 3, antitoxins 0, military 0, empty presses 0`.
    pub fn line(&self) -> String {
        let [medkits, bandages, antitoxins, military] = self.pressed;
        format!(
            "spawns {} (short {}), medkits {medkits}, bandages {bandages}, antitoxins {antitoxins}, military {military}, empty presses {}",
            self.spawns, self.short_spawns, self.empty_presses
        )
    }
}

impl std::ops::AddAssign for PantryCounts {
    fn add_assign(&mut self, other: Self) {
        self.spawns += other.spawns;
        self.short_spawns += other.short_spawns;
        for (mine, theirs) in self.pressed.iter_mut().zip(other.pressed) {
            *mine += theirs;
        }
        self.empty_presses += other.empty_presses;
    }
}

impl Default for Pantry {
    fn default() -> Self {
        Self::standard()
    }
}

/// The pockets of a bot's kit before a world says what it holds: the owner's kit's drugs (the medkits, then the
/// bandages: [`BOT_BANDAGES`], quick use 2) when the bots carry it (`SRP_BOT_KIT`, [`new_kit`]), else the
/// tip's [`BOT_POCKETS`]. The bench's stand-in builds its bodies' kits from the same list.
pub(crate) fn standard_pockets() -> Vec<BotKitItem> {
    if new_kit() {
        vec![BOT_MEDKIT, BOT_BANDAGES]
    } else {
        BOT_POCKETS.to_vec()
    }
}

impl Pantry {
    /// What every server bot's kit holds ([`BOT_POCKETS`]: two medkits),
    /// until a world says what its kits hold.
    pub fn standard() -> Self {
        Self::standard_with(PocketStore::default())
    }

    /// [`Self::standard`] with the store the kits were built with.
    pub fn standard_with(store: PocketStore) -> Self {
        Self {
            kits: vec![Self::standard_kit(store)],
            kit: 0,
            unknown: Self::standard_kit(store),
            counts: PantryCounts::default(),
            factors: DrugFactors::NONE,
        }
    }

    /// [`Self::standard_with`] for the tip's [`BOT_POCKETS`] (two medkits), whatever the process's kit switch says: the tests of
    /// the medkit rules assume it.
    #[cfg(test)]
    pub(crate) fn medkits_with(store: PocketStore) -> Self {
        let kit = Self::kit_of(BOT_POCKETS.iter(), store);
        Self {
            kits: vec![kit.clone()],
            kit: 0,
            unknown: kit,
            counts: PantryCounts::default(),
            factors: DrugFactors::NONE,
        }
    }

    /// [`Self::medkits_with`] with the default store.
    #[cfg(test)]
    pub(crate) fn medkits() -> Self {
        Self::medkits_with(PocketStore::default())
    }

    /// The drugs of a bot's own profile: each kit's pockets (`pockets.rs`) with the stack the slot starts a life
    /// with and the storage behind it, so the count follows the store the profile was built with, a team's own
    /// (`kit.pocket_store`), not a process-wide one. A profile with no kit (a human's, a late joiner's) is the
    /// standard pantry.
    pub fn of_profile(profile: &MatchProfile) -> Self {
        let Some(pockets) = ProfilePockets::of_profile(profile, true) else {
            return Self::standard();
        };
        let kits: Vec<Vec<Held>> = pockets
            .kits()
            .iter()
            .map(|kit| {
                kit.iter()
                    .flatten()
                    .filter_map(|pocket| {
                        let drug = Drug::of_name(pocket.name)?;
                        let quick = quick_use_of(kit, pocket.slot)?;
                        let total = pocket.stored.unwrap_or(u32::from(pocket.units)).min(u32::from(u16::MAX)) as u16;
                        Some(Held {
                            drug,
                            quick,
                            slot: pocket.units,
                            total,
                            hand: pocket.units,
                            pressed: None,
                        })
                    })
                    .collect()
            })
            .collect();
        Self {
            unknown: kits.first().cloned().unwrap_or_default(),
            kits,
            kit: 0,
            counts: PantryCounts::default(),
            factors: DrugFactors::of_profile(profile),
        }
    }

    /// What its medical skills do to its drugs.
    pub fn factors(&self) -> DrugFactors {
        self.factors
    }

    /// What this pantry has done so far.
    pub fn counts(&self) -> PantryCounts {
        self.counts
    }

    fn standard_kit(store: PocketStore) -> Vec<Held> {
        Self::kit_of(standard_pockets().iter(), store)
    }

    fn kit_of<'a>(pockets: impl Iterator<Item = &'a BotKitItem>, store: PocketStore) -> Vec<Held> {
        pockets
            .filter_map(|item| {
                let drug = Drug::of_name(item.name)?;
                let quick = usize::from(item.input_bit? - QUICK_USE_FIRST_BIT);
                let count = item.count as u16;
                // What the kit has for the match until a world says: the slot and the storage behind it.
                let total = store.stored(item.count).min(u32::from(u16::MAX)) as u16;
                Some(Held {
                    drug,
                    quick,
                    slot: count,
                    total,
                    hand: count,
                    pressed: None,
                })
            })
            .collect()
    }

    fn kit_at(&mut self, kit: usize) -> &mut Vec<Held> {
        while self.kits.len() <= kit {
            self.kits.push(self.unknown.clone());
        }
        &mut self.kits[kit]
    }

    /// A new life in `kit`: the slot is filled from what the match has
    /// left of it (the client's spawn), and nothing is active.
    pub fn new_life(&mut self, kit: usize) {
        self.kit = kit;
        let mut short = false;
        for held in self.kit_at(kit) {
            held.hand = held.slot.min(held.total);
            held.pressed = None;
            short |= held.hand < held.slot;
        }
        self.counts.spawns += 1;
        self.counts.short_spawns += u32::from(short);
    }

    /// A trusted world says the last hit that took health was at `hit_ms` (the world's ms, 0: none): each press of
    /// the kit in use made before it and not hit yet takes it as its first hit ([`Press::hit`]).
    pub fn hit(&mut self, hit_ms: u32) {
        if hit_ms == 0 {
            return;
        }
        let kit = self.kit;
        for held in self.kit_at(kit) {
            if let Some(press) = held.pressed.as_mut()
                && press.hit.is_none()
                && hit_ms > press.at
            {
                press.hit = Some(hit_ms);
            }
        }
    }

    /// The drugs of the kit in use.
    pub fn held(&self) -> &[Held] {
        self.kits.get(self.kit).map_or(&[], Vec::as_slice)
    }

    /// Units of `drug` in the slots now.
    pub fn in_hand(&self, drug: Drug) -> u16 {
        self.held()
            .iter()
            .filter(|held| held.drug == drug)
            .map(|held| held.hand)
            .sum()
    }

    /// Units of `drug` left for the match in the kit in use.
    pub fn total(&self, drug: Drug) -> u16 {
        self.held()
            .iter()
            .filter(|held| held.drug == drug)
            .map(|held| held.total)
            .sum()
    }

    /// The drug a press of quick-use key `quick` (0 on) uses, if its slot
    /// holds one.
    pub fn at_key(&self, quick: usize) -> Option<&Held> {
        self.held()
            .iter()
            .find(|held| held.quick == quick && held.hand > 0)
    }

    /// The first held `drug` with a unit in its slot.
    pub fn first(&self, drug: Drug) -> Option<&Held> {
        self.held()
            .iter()
            .find(|held| held.drug == drug && held.hand > 0)
    }

    /// Quick-use key `quick` (0 on) was pressed at `now`: a unit leaves
    /// the slot and the match's count. False if the slot held none (the
    /// game does nothing then).
    pub fn pressed(&mut self, quick: usize, now: u32) -> bool {
        let kit = self.kit;
        let Some(held) = self
            .kit_at(kit)
            .iter_mut()
            .find(|held| held.quick == quick && held.hand > 0)
        else {
            self.counts.empty_presses += 1;
            return false;
        };
        held.hand -= 1;
        held.total = held.total.saturating_sub(1);
        held.pressed = Some(Press { at: now, hit: None });
        let drug = held.drug;
        if let Some(at) = Drug::ALL.iter().position(|&d| d == drug) {
            self.counts.pressed[at] += 1;
        }
        true
    }

    /// A trusted world made at `time` showed `readings`: the kits hold what
    /// it says, and the counts are its own unless a press of this life is
    /// too new for the world to show.
    pub fn read(&mut self, time: u32, readings: &Readings) {
        self.kit = readings.kit;
        for kit in 0..readings.kits.max(readings.kit + 1) {
            let held = self.kit_at(kit);
            held.retain(|h| {
                readings
                    .drugs
                    .iter()
                    .any(|r| r.kit == kit && r.drug == h.drug && r.quick == h.quick)
            });
            for r in readings.drugs.iter().filter(|r| r.kit == kit) {
                let at = held
                    .iter()
                    .position(|h| h.drug == r.drug && h.quick == r.quick)
                    .unwrap_or_else(|| {
                        held.push(Held {
                            drug: r.drug,
                            quick: r.quick,
                            slot: r.slot,
                            total: r.slot,
                            hand: r.slot,
                            pressed: None,
                        });
                        held.len() - 1
                    });
                let h = &mut held[at];
                h.slot = r.slot;
                if h.pressed.is_some_and(|p| time < p.at + PRESS_LAG_MS) {
                    continue;
                }
                let stored = r.stored.min(u32::from(u16::MAX)) as u16;
                match r.in_hand {
                    Some(n) => {
                        h.hand = n;
                        h.total = n.saturating_add(stored);
                    }
                    // In store: a kit not in use, or a body dead.
                    None if !(r.in_use && readings.alive) => h.total = stored,
                    None => {}
                }
            }
        }
    }
}

/// A live player's kit and player-state bytes, cut from a recording of a
/// match of 20 places (`tests/data/live_kit_20_players_2_medkits_2_bandages.txt`):
/// the profile's items and kit slots, and the state the client uploaded in a
/// world, as `players::Supplies` reads it. Masked: no name, no account; the
/// bytes of the state are the client's own. The kit in use holds a medkit
/// and bandages, 2 of each in the slots and the rest in store.
#[cfg(test)]
pub(crate) mod live_kit {
    use survarium::v026g::match_msgs::static_info::{
        MatchItem, MatchKit, MatchProfile, NO_PROTECTION,
    };

    pub(crate) struct LiveKit {
        pub(crate) place: u8,
        pub(crate) profile: MatchProfile,
        pub(crate) state: Vec<u8>,
    }

    pub(crate) fn live_kit() -> LiveKit {
        const TEXT: &str =
            include_str!("../../../tests/data/live_kit_20_players_2_medkits_2_bandages.txt");
        let (mut place, mut state) = (0u8, Vec::new());
        let mut items: Vec<MatchItem> = Vec::new();
        let mut kits: Vec<MatchKit> = Vec::new();
        for line in TEXT.lines().filter(|line| !line.starts_with('#')) {
            let words: Vec<&str> = line.split(' ').collect();
            let number = |at: usize| words[at].parse::<u32>().expect("a number");
            match words[0] {
                "place" => place = number(1) as u8,
                "item" => items.push(MatchItem {
                    dict_id: number(2) as u16,
                    condition_or_stack: number(3),
                    ..MatchItem::default()
                }),
                "slot" => {
                    let kit = number(1) as usize;
                    while kits.len() <= kit {
                        kits.push(MatchKit::empty(kits.len() as u32, ""));
                    }
                    kits[kit].slots[number(2) as usize] =
                        Some((number(3) as u16, number(4) as u16));
                }
                "state" => {
                    state = (0..words[1].len() / 2)
                        .map(|i| u8::from_str_radix(&words[1][2 * i..2 * i + 2], 16).expect("hex"))
                        .collect();
                }
                other => panic!("the fixture has a line `{other}`"),
            }
        }
        // Recorded 2026-10-26, before revision 38: no pickup record, no carry snapshot (A305: main's reader takes
        // today's layout, `state_at_revision`).
        let state = survarium::v026g::match_msgs::players::state_at_revision(&state, 37)
            .expect("the fixture's state")
            .into_owned();
        LiveKit {
            place,
            profile: MatchProfile {
                team: 0,
                name: String::new(),
                clan_tag: String::new(),
                protection: NO_PROTECTION,
                kits,
                items,
                squad: 0,
            },
            state,
        }
    }
}

/// A bot's consumables state, by feature ([`super::features`]): the medkits it used this life, the drugs of its
/// kits counted for the match, and its health pools as the last trusted world showed them.
#[derive(Default)]
pub struct BotState {
    /// Medkits used this life (the tip's count, which the Heal play and Retreat go by while `HealTuning::on` is off).
    pub medkits_used: u8,
    /// The drugs of its kits, counted for the match ([`Pantry`]).
    pub pantry: Pantry,
    /// Its health pools as the last trusted world showed them (`Bots::pools`); none before the first.
    pub pools: Option<super::pools::Pools>,
}

impl super::features::Feature for BotState {
    fn new_life(&mut self, life: &super::features::Life) {
        self.medkits_used = 0;
        self.pantry.new_life(life.kit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::loadouts::{BotEquipment, BotRole, BotTier, RusherKits, bot_profile, bot_profile_stored};
    use survarium::v026g::match_msgs::players::ItemLeft;

    /// A bot's block as the game shows it: each item of the profile in
    /// store as given, and the slots of kit `kit` holding `hand` of the
    /// medkit.
    fn supplies(profile: &MatchProfile, kit: u8, alive: bool, hand: u16, stored: u32) -> Supplies {
        let dictionary = Dictionary::shared();
        let items = profile
            .items
            .iter()
            .map(|item| {
                let medkit = dictionary
                    .item(item.dict_id)
                    .is_some_and(|row| row.name == "medkit");
                ItemLeft {
                    dict_id: item.dict_id,
                    stack: dictionary.is_stack(item.dict_id),
                    stored: if medkit { stored } else { 0 },
                    in_hand: (medkit && alive).then_some(hand),
                    condition: None,
                    magazine: None,
                    slot: None,
                }
            })
            .collect();
        Supplies {
            alive,
            kit,
            inventory_read: alive,
            items,
            hands_slot: None,
            carried_weight: None,
            carrying: None,
        }
    }

    fn profile() -> MatchProfile {
        bot_profile(
            1,
            0,
            BotRole::ALL[0],
            BotEquipment::of(BotTier::Low, RusherKits::default()),
        )
    }

    #[test]
    fn a_bots_kit_holds_two_medkits_on_quick_use_1_and_the_count_is_the_matchs() {
        let mut pantry = Pantry::standard_with(PocketStore::PerMatch);
        pantry.new_life(0);
        assert_eq!(pantry.in_hand(Drug::Medkit), 2);
        assert_eq!(pantry.first(Drug::Medkit).map(|h| h.quick), Some(0));
        assert!(pantry.pressed(0, 1_000));
        assert!(pantry.pressed(0, 9_000));
        assert_eq!(pantry.in_hand(Drug::Medkit), 0);
        // The third press of the kit finds an empty slot ...
        assert!(!pantry.pressed(0, 20_000));
        assert!(pantry.first(Drug::Medkit).is_none());
        // ... and a new life does not give the kit its medkits back: the
        // storage is empty (the old per-life count did).
        pantry.new_life(0);
        assert_eq!(pantry.in_hand(Drug::Medkit), 0);
        assert_eq!(pantry.total(Drug::Medkit), 0);
        assert!(!pantry.pressed(0, 40_000));
        // Another kit has its own two.
        pantry.new_life(1);
        assert_eq!(pantry.in_hand(Drug::Medkit), 2);
        assert!(pantry.pressed(0, 60_000));
        pantry.new_life(1);
        assert_eq!(pantry.in_hand(Drug::Medkit), 1, "one left for the match");
    }

    /// The store of 100 (the default): a kit's two medkits are there again at every life, the match's count is the
    /// store's, and a world's word on the storage still wins.
    #[test]
    fn with_the_pocket_store_every_life_starts_with_the_stack() {
        let mut pantry = Pantry::standard_with(PocketStore::PerLife);
        pantry.new_life(0);
        assert_eq!(pantry.in_hand(Drug::Medkit), 2);
        assert_eq!(pantry.total(Drug::Medkit), 100);
        for life in 0..3 {
            assert!(pantry.pressed(0, 1_000 + life * 20_000));
            assert!(pantry.pressed(0, 9_000 + life * 20_000));
            assert!(!pantry.pressed(0, 15_000 + life * 20_000), "the slot is empty in this life");
            pantry.new_life(0);
            assert_eq!(pantry.in_hand(Drug::Medkit), 2, "life {}", life + 2);
        }
        assert_eq!(pantry.total(Drug::Medkit), 94);
    }

    /// A pantry built from the profile follows the store the profile was built with, as the client's own spawn does, and counts
    /// what it did: the old store spends the stack for the match (a later life starts short), the store of 100 does not.
    #[test]
    fn a_pantry_of_a_profile_follows_the_store_the_profile_was_built_with() {
        for (store, total, short_spawns, pressed) in [(PocketStore::PerMatch, 0, 3, 2), (PocketStore::PerLife, 94, 0, 6)] {
            let profile = bot_profile_stored(
                1,
                0,
                BotRole::ALL[0],
                BotEquipment::of(BotTier::Low, RusherKits::default()),
                store,
            );
            let mut pantry = Pantry::of_profile(&profile);
            // The profile names the prior: 2 in the slot, 2 or 100 behind it, on quick use 1.
            assert_eq!(pantry.total(Drug::Medkit), if store == PocketStore::PerMatch { 2 } else { 100 });
            pantry.new_life(0);
            assert_eq!(pantry.first(Drug::Medkit).map(|h| h.quick), Some(0));
            for life in 0..3 {
                assert_eq!(pantry.in_hand(Drug::Medkit), if life == 0 || store == PocketStore::PerLife { 2 } else { 0 });
                while pantry.pressed(0, 1_000 + life * 20_000) {}
                pantry.new_life(0);
            }
            assert_eq!(pantry.total(Drug::Medkit), total, "{store:?}");
            let counts = pantry.counts();
            assert_eq!((counts.spawns, counts.short_spawns), (4, short_spawns), "{store:?}");
            assert_eq!((counts.pressed[0], counts.empty_presses), (pressed, 3), "{store:?}");
        }
        // A profile with no kit is the standard pantry.
        let mut none = profile();
        none.kits.clear();
        assert_eq!(Pantry::of_profile(&none), Pantry::standard());
    }

    #[test]
    fn the_readings_are_the_slot_and_the_store_with_the_kits_own_quick_key() {
        let profile = profile();
        let dictionary = Dictionary::shared();
        let read = readings(&supplies(&profile, 0, true, 2, 0), &profile, dictionary);
        // Three kits, each with its medkit on quick use 1.
        let medkits: Vec<_> = read
            .drugs
            .iter()
            .filter(|r| r.drug == Drug::Medkit)
            .collect();
        assert_eq!(medkits.len(), 3);
        for (k, r) in medkits.iter().enumerate() {
            assert_eq!((r.kit, r.quick, r.slot), (k, 0, 2));
            assert_eq!(r.in_use, k == 0);
        }
        assert_eq!(medkits[0].in_hand, Some(2));
        assert_eq!(medkits[1].in_hand, None, "a kit not in use has its store");
        // Dead: the slot is not read.
        let read = readings(&supplies(&profile, 0, false, 0, 2), &profile, dictionary);
        assert!(read.drugs.iter().all(|r| r.in_hand.is_none()));
        assert_eq!(read.drugs[0].stored, 2);
    }

    #[test]
    fn a_world_corrects_the_count_unless_a_press_is_too_new_for_it() {
        let profile = profile();
        let dictionary = Dictionary::shared();
        let mut pantry = Pantry::standard();
        pantry.new_life(0);
        assert!(pantry.pressed(0, 10_000));
        // A world just after the press: it cannot show it yet.
        let read = readings(&supplies(&profile, 0, true, 2, 0), &profile, dictionary);
        pantry.read(10_200, &read);
        assert_eq!(pantry.in_hand(Drug::Medkit), 1);
        // A world after the game applied it: the slot holds one.
        let read = readings(&supplies(&profile, 0, true, 1, 0), &profile, dictionary);
        pantry.read(11_000, &read);
        assert_eq!(
            (pantry.in_hand(Drug::Medkit), pantry.total(Drug::Medkit)),
            (1, 1)
        );
        // The game ignored a press (a medkit already active): the world
        // has the slot as it was, and the count follows it.
        assert!(pantry.pressed(0, 12_000));
        let read = readings(&supplies(&profile, 0, true, 1, 0), &profile, dictionary);
        pantry.read(13_000, &read);
        assert_eq!(pantry.in_hand(Drug::Medkit), 1);
        // Dead, the slot goes back to the store; the next life fills from it.
        let read = readings(&supplies(&profile, 0, false, 0, 1), &profile, dictionary);
        pantry.read(20_000, &read);
        pantry.new_life(0);
        assert_eq!(pantry.in_hand(Drug::Medkit), 1);
    }

    #[test]
    fn a_world_names_the_kit_in_use_and_what_every_kit_holds() {
        let profile = profile();
        let dictionary = Dictionary::shared();
        let mut pantry = Pantry::standard();
        pantry.new_life(0);
        // The game says kit 2 is in use, with one medkit left of its two.
        let read = readings(&supplies(&profile, 2, true, 1, 0), &profile, dictionary);
        pantry.read(5_000, &read);
        assert_eq!(pantry.in_hand(Drug::Medkit), 1);
        assert!(pantry.pressed(0, 6_000));
        assert!(!pantry.pressed(0, 7_000));
        // Kits with no medkit at all: the world's word, not the standard's.
        let mut bare = profile.clone();
        for kit in &mut bare.kits {
            kit.slots[13] = None;
        }
        let read = readings(&supplies(&bare, 0, true, 0, 0), &bare, dictionary);
        assert!(read.drugs.iter().all(|r| r.drug != Drug::Medkit));
        pantry.read(9_000, &read);
        pantry.new_life(0);
        assert_eq!(pantry.in_hand(Drug::Medkit), 0);
    }

    /// A block a live client uploaded in a match of 20 places: the kit in use
    /// (kit 1) holds a medkit on quick use 1 and bandages on quick use 2,
    /// two of each in the slots, 20 and 18 more in store.
    #[test]
    fn a_live_players_block_reads_two_medkits_and_two_bandages() {
        use super::live_kit::live_kit;
        use survarium::v026g::match_msgs::players::{PlayerBlock, PlayerState};
        let live = live_kit();
        let dictionary = Dictionary::shared();
        assert!(PlayerState::read(&live.state, live.profile.items.len()).is_some_and(|s| s.alive));
        let block = PlayerBlock {
            place: live.place,
            state: &live.state,
            animation: None,
        };
        let supplies = Supplies::read(&block, &live.profile, dictionary).unwrap();
        assert!(supplies.alive && supplies.inventory_read);
        assert_eq!(supplies.kit, 1);
        let of = |name: &str| {
            supplies
                .items
                .iter()
                .find(|item| {
                    dictionary
                        .item(item.dict_id)
                        .is_some_and(|row| row.name == name)
                })
                .unwrap()
        };
        let (medkit, bandages) = (of("medkit"), of("bandages"));
        assert_eq!((medkit.in_hand, medkit.stored), (Some(2), 20));
        assert_eq!((bandages.in_hand, bandages.stored), (Some(2), 18));
        // `Supplies::left`: the slots and the store.
        let left = supplies.left();
        assert_eq!(left[&medkit.dict_id], 22);
        assert_eq!(left[&bandages.dict_id], 20);
        // The readings, with the key of the kit's own slot order.
        let read = readings(&supplies, &live.profile, dictionary);
        assert_eq!((read.kit, read.kits, read.alive), (1, 3, true));
        let in_use: Vec<_> = read.drugs.iter().filter(|r| r.in_use).collect();
        assert_eq!(in_use.len(), 2);
        assert_eq!(
            (in_use[0].drug, in_use[0].quick, in_use[0].in_hand),
            (Drug::Medkit, 0, Some(2))
        );
        assert_eq!(
            (in_use[1].drug, in_use[1].quick, in_use[1].in_hand),
            (Drug::Bandages, 1, Some(2))
        );
        // The pantry follows it.
        let mut pantry = Pantry::standard();
        pantry.read(109_550, &read);
        assert_eq!(
            (pantry.in_hand(Drug::Medkit), pantry.total(Drug::Medkit)),
            (2, 22)
        );
        assert_eq!(
            (pantry.in_hand(Drug::Bandages), pantry.total(Drug::Bandages)),
            (2, 20)
        );
        assert_eq!(pantry.first(Drug::Bandages).map(|h| h.quick), Some(1));
        // Bandages used up: the third press has no slot to find.
        assert!(pantry.pressed(1, 110_000) && pantry.pressed(1, 111_000));
        assert!(pantry.first(Drug::Bandages).is_none());
        assert_eq!(
            pantry.total(Drug::Bandages),
            18,
            "the store is for the next life"
        );
    }

    /// [A217] A world's last hit that took health after a press is the press's first hit; one before it, a later
    /// one, or none changes nothing, and a new life's press starts clean.
    #[test]
    fn the_first_hit_after_a_press_is_kept_with_it() {
        let mut pantry = Pantry::standard();
        pantry.new_life(0);
        assert!(pantry.pressed(0, 10_000));
        let press = |pantry: &Pantry| pantry.at_key(0).and_then(|held| held.pressed);
        pantry.hit(0);
        pantry.hit(9_500);
        assert_eq!(press(&pantry), Some(Press { at: 10_000, hit: None }), "none, and a hit before the press");
        pantry.hit(11_200);
        pantry.hit(12_000);
        assert_eq!(press(&pantry), Some(Press { at: 10_000, hit: Some(11_200) }), "the first after it stays");
        pantry.new_life(0);
        assert_eq!(press(&pantry), None);
    }
    /// [A341, F65] The client's reading of a kit's medical modifiers: 302 at 2 (bits 3..5), 311 at 5 (bits 6..8), 312 at 3
    /// (bits 9..11, and `drug_amount` 0.24) give a delay x0.9, amounts x1.24 and regeneration 2.5 s sooner; a value the client
    /// refuses (not an exact integer, a rank over its field's cap: 302 at 3, 311 at 6) is no skill; no skill is the identity.
    #[test]
    fn the_drug_factors_are_the_clients_reading_of_the_medical_ranks() {
        use survarium::v026g::skills::{MODIFIERS, modifier_index};
        let at = |name| modifier_index(name).unwrap();
        let mut m = [0.0f32; MODIFIERS];
        assert_eq!(DrugFactors::of_modifiers(&m), DrugFactors::NONE);
        m[at("armored_trap")] = (1 + 2 * 8 + 5 * 64 + 3 * 512) as f32;
        m[at("drug_amount")] = 0.24;
        let f = DrugFactors::of_modifiers(&m);
        assert!((f.delay - 0.9).abs() < 1e-6 && (f.amount - 1.24).abs() < 1e-6, "{f:?}");
        assert_eq!(f.regeneration_sooner_ms, 2_500);
        for refused in [0.5, 7.0 * 8.0, 3.0 * 8.0 + 5.0 * 64.0, 6.0 * 64.0, -8.0, f32::NAN] {
            m[at("armored_trap")] = refused;
            let f = DrugFactors::of_modifiers(&m);
            assert_eq!((f.delay, f.regeneration_sooner_ms), (1.0, 0), "{refused}");
        }
    }
    /// [F65] The numbers the bots copy from the client patch are the patch's own: the caps of `medical_skills.rs`'s
    /// `Field::new` rows, Fast Bandaging's 0.05 and Metabolism's 500 ms a rank (read from the file as text: the patch crate is a
    /// static library and its file needs the patch's runtime), and every rank of the bots' block within its cap.
    #[test]
    fn the_medical_numbers_are_the_client_patchs() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../client-patches/rust/patches/src/medical_skills.rs");
        let source = std::fs::read_to_string(path).expect("the client patch's medical_skills.rs");
        let field = |name: &str| -> (u32, u32) {
            let line = source.lines().find(|l| l.starts_with(&format!("const {name}: Field = Field::new("))).expect(name);
            let args: Vec<u32> = line.split('(').nth(1).unwrap().trim_end_matches(");").split(',').map(|v| v.trim().parse().unwrap()).collect();
            assert_eq!(args[1], 3, "{name}: a three-bit field");
            (args[0], args[2])
        };
        let names = ["PAINKILLER", "BANDAGING", "METABOLISM", "TRAINING", "ADRENALIN", "BONES", "BROTHERS"];
        assert_eq!(names.map(field), MEDICAL_CAPS);
        assert!(source.contains(&format!("f32::from(self.bandaging) * {BANDAGING_PER_RANK}")), "Fast Bandaging's factor");
        assert!(source.contains(&format!("u32::from(self.metabolism) * {METABOLISM_PER_RANK_MS}")), "Metabolism's head start");
        medical_ranks_within_caps();
    }

    /// [F65, A372] The bots' medical block: each skill of its intent has its field (301 at bit 0, 302 at 3, 311 at 6, 312 at
    /// 9), and each rank the bots' builds resolve ([`survarium::v026g::bot_skills::BotBuilds`]) is within the field's cap. On
    /// the tests' data the tree has no medical tab and the builds hold none; the players' data: the ignored test below.
    fn medical_ranks_within_caps() {
        use survarium::v026g::bot_skills::{BOT_SKILL_TIERS, BotBuilds, MEDICAL};
        let shift = |id: u16| match id {
            301 => 0,
            302 => 3,
            311 => 6,
            312 => 9,
            _ => panic!("a medical skill of the block with no field here: {id}"),
        };
        let medical: Vec<u16> = MEDICAL.want.iter().chain(MEDICAL.open).copied().collect();
        for &id in &medical {
            shift(id);
        }
        let builds = BotBuilds::shared().expect("the bots' builds");
        for tier in &BOT_SKILL_TIERS {
            for pouch in [false, true] {
                for &(id, rank) in builds.of(tier, pouch).skills.iter().filter(|(id, _)| medical.contains(id)) {
                    let (_, most) = MEDICAL_CAPS.iter().find(|(s, _)| *s == shift(id)).copied().unwrap();
                    assert!(u32::from(rank) <= most, "skill {id} at {rank} over the client's cap {most}");
                }
            }
        }
    }

    /// [A341] On the players' data (`SRP_GAME_DATA`): a bot's own profile of every tier carries the medical ranks, so its pantry's
    /// factors are the skills' (delay x0.9, amounts x1.24, regeneration 2.5 s sooner).
    #[test]
    #[ignore = "needs SRP_GAME_DATA, the players' game data with the medical tab"]
    fn a_bots_profile_on_the_players_data_gives_its_pantry_the_medical_factors() {
        use survarium::v026g::loadouts::{BotEquipment, BotRole, BotTier, RusherKits, bot_profile};
        survarium::v026g::game_data::init().expect("SRP_GAME_DATA reads and passes srp's checks");
        medical_ranks_within_caps();
        for tier in BotTier::ALL {
            for role in BotRole::ALL {
                let profile = bot_profile(1, 0, role, BotEquipment::of(tier, RusherKits::default()));
                let f = Pantry::of_profile(&profile).factors();
                eprintln!("{tier:?} {role:?}: {f:?}");
                assert!((f.delay - 0.9).abs() < 1e-6 && (f.amount - 1.24).abs() < 1e-6, "{tier:?} {role:?} {f:?}");
                assert_eq!(f.regeneration_sooner_ms, 2_500);
            }
        }
    }
}

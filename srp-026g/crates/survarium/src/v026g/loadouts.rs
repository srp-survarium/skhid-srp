//! Starting loadouts: the game's own test loadouts,
//! `gameplay/test_player_profiles.options` of 0.26g's `resources.db`
//! (`docs/notes/data.md` §4).
//!
//! The file has ten profiles of thirteen item names, the name at index `i`
//! for profile slot `i` (as in 0.21d): helmet, mask, torso, back, legs,
//! gloves, boots, the first weapon and two ammo for it, the second weapon
//! and two ammo for it. Every name is a 0.26g item and fits its slot.
//!
//! **The carry limit.** 0.26g's player carries 30 kg
//! (`item_params::player().max_carried_weight`), and the lobby does not let a kit go
//! over it. With one clip of each ammo the ten profiles weigh 32.35, 32.3,
//! 38.4, 26.9, 19.3, 26.3, 22.3, 26.3, 26.3 and 26.3 kg: **the first three
//! are over**. (The gear alone is 30.5, 30.0 and 36.1 kg: profiles 0 and 2
//! are over even with no ammo.) They are the three with the heavy
//! Renaissance and Edge torsos. srp **drops those three** rather than trim
//! them [choice: which piece to take off a test profile is nobody's rule]:
//! [`Loadouts::usable`] gives the seven that fit. All ten stay readable in
//! [`Loadouts::profiles`].
//!
//! **What a new account has.** "One of the starting weapons available to
//! all players when they first enter the world, along with the TT-33 and
//! Mosin carbine" (the wiki's `PPSH-41` page of 2015-02-14, revision 9600,
//! and `Mosin carbine` of 2015-02-24, revision 10011; 0.26i). PROVEN (wiki
//! of the time). 0.26g's data agrees: the three weapons and their two
//! kinds of ammo (7.62x25 for the TT-33 and the PPSh-41, 7.62x54R for the
//! Mosin) are the only weapons and ammo with `base_cost` 0 and item
//! level 0. [`STARTER`] is what of it the first kit wears and
//! [`STARTER_STASH`] what lies in the inventory.
//!
//! [choice] Which of the three a new player wore is not known. srp's first
//! kit wears what it always did, the TT-33 and a clip in weapon slot 1, and
//! the Mosin, a clip for it and the PPSh-41 lie in the inventory for the
//! player to put on. A kit with one kind of ammo is also the one whose
//! ammo use the match counters prove (`session::v026g::autobuy`). The free
//! ammo is one clip each; more of it costs nothing
//! (`session::v026g::rules`).

use binary_config_parser::tree::{ConfigError, ConfigValue};

use super::dictionary::{Dictionary, PROFILE_SLOTS};
use super::item_params;
use super::match_msgs::static_info::MatchProfile;

/// Its path in the archive.
pub const PATH: &str = "gameplay/test_player_profiles.options";

/// What a new account's first kit wears, `(slot, item)`: the TT-33 in
/// weapon slot 1 and a clip of its free ammo in that weapon's first ammo
/// slot. See the module.
pub const STARTER: [(u8, &str); 2] = [(7, "tt_33"), (8, "ammo_7.62x25")];

/// What a new account owns besides what it wears ([`STARTER`]): the Mosin
/// carbine with a clip of its free ammo, and the PPSh-41, which fires the
/// TT-33's ammo.
pub const STARTER_STASH: [&str; 3] = ["mosin_nagant", "ammo_7.62x54r_old", "ppsh"];

/// What a slot of a loadout holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotItem {
    pub slot: u8,
    pub dict_id: u16,
    /// Units for a stack (one clip of ammo, one of anything else); 1 for an
    /// item that is no stack.
    pub count: u32,
}

/// One loadout: its items by slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loadout {
    pub items: Vec<SlotItem>,
}

impl Loadout {
    /// From `(slot, item name)`; an item that is unknown or does not fit its
    /// slot is an error.
    pub fn from_names<'a>(
        dictionary: &Dictionary,
        names: impl IntoIterator<Item = (u8, &'a str)>,
    ) -> Result<Self, ConfigError> {
        let items = names
            .into_iter()
            .map(|(slot, name)| {
                let item = dictionary
                    .item_named(name)
                    .ok_or_else(|| ConfigError(format!("no item `{name}`")))?;
                if usize::from(slot) >= PROFILE_SLOTS || !dictionary.fits(item.dict_id, slot) {
                    return Err(ConfigError(format!("`{name}` does not fit slot {slot}")));
                }
                let count = if item.is_stack {
                    item_params::shop_unit(item.dict_id)
                } else {
                    1
                };
                Ok(SlotItem {
                    slot,
                    dict_id: item.dict_id,
                    count,
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { items })
    }

    /// What the loadout weighs, as the lobby counts it.
    pub fn weight(&self) -> f32 {
        self.items
            .iter()
            .map(|item| item_params::carried_weight(item.dict_id, item.count))
            .sum()
    }

    /// Whether a player can carry it.
    pub fn fits_carry_limit(&self) -> bool {
        self.weight() <= item_params::player().max_carried_weight
    }

    pub fn slot(&self, slot: u8) -> Option<&SlotItem> {
        self.items.iter().find(|item| item.slot == slot)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loadouts {
    /// The file's profiles, in its order.
    pub profiles: Vec<Loadout>,
}

impl Loadouts {
    /// The players' test profiles ([`super::game_data`]), parsed once.
    pub fn shared() -> &'static Self {
        static SHARED: std::sync::OnceLock<Loadouts> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| {
            Self::parse(super::game_data::file(PATH), Dictionary::shared())
                .expect("the test profiles parse")
        })
    }

    pub fn parse(bytes: &[u8], dictionary: &Dictionary) -> Result<Self, ConfigError> {
        let profiles = ConfigValue::parse(bytes)?
            .req("profiles")?
            .children()
            .into_iter()
            .map(|profile| {
                let names = profile
                    .children()
                    .into_iter()
                    .map(|name| {
                        name.as_str()
                            .ok_or_else(|| ConfigError("a test profile item is not a name".into()))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Loadout::from_names(
                    dictionary,
                    names
                        .into_iter()
                        .enumerate()
                        .map(|(slot, name)| (slot as u8, name)),
                )
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { profiles })
    }

    /// The profiles a player can carry: all but the overweight ones.
    pub fn usable(&self) -> impl Iterator<Item = &Loadout> {
        self.profiles
            .iter()
            .filter(|loadout| loadout.fits_carry_limit())
    }
}

/// What a new account wears ([`STARTER`]).
pub fn starter() -> &'static Loadout {
    static SHARED: std::sync::OnceLock<Loadout> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        Loadout::from_names(Dictionary::shared(), STARTER).expect("the starter items exist")
    })
}

/// What a new account owns unworn ([`STARTER_STASH`]): `(dict id, units)`,
/// a clip of a stack, 1 of an item that is no stack.
pub fn starter_stash() -> &'static [(u16, u32)] {
    static SHARED: std::sync::OnceLock<Vec<(u16, u32)>> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        let dictionary = Dictionary::shared();
        STARTER_STASH
            .iter()
            .map(|name| {
                let item = dictionary
                    .item_named(name)
                    .expect("the starter items exist");
                let units = if item.is_stack {
                    item_params::shop_unit(item.dict_id)
                } else {
                    1
                };
                (item.dict_id, units)
            })
            .collect()
    })
}

/// The pockets: profile slots 13..=18, the only slots that take drugs,
/// boosters, traps and grenades (`slots_restrictions` of the dictionary:
/// categories 10, 11 and 24), and the six the client searches for a
/// grenade (its table at 0x925FC0). PROVEN.
pub const POCKET_SLOTS: [u8; 6] = [13, 14, 15, 16, 17, 18];

/// The order in which the client counts the slots for the quick-use keys
/// (its table at 0x925FD8, read by 0x2FE7F0): the pockets, the back, then
/// the other armour. PROVEN. Quick use `n` (1 on) is the `n`-th of these
/// that holds an item the client lets be used: every item class it was
/// read for (the base item, the grenade, the medkit, the oxygen tank)
/// passes the class test (`vtable+0x58` is `xor eax, eax`; the item that
/// fails it is LIKELY an artefact: the inventory fill 0x2FD1C8 starts the
/// artefacts at the first pocket that does), the other test (`item+0x24C`,
/// a constructor argument) was not read, so which items count is LIKELY
/// all but artefacts.
pub const QUICK_USE_ORDER: [u8; 13] = [13, 14, 15, 16, 17, 18, 3, 0, 1, 2, 4, 5, 6];

/// The input bit (`actions_mask`) of quick use 1; quick use `n` is this
/// bit + `n` - 1, up to quick use 6 at bit 19 (`docs/notes/player_bots.md`
/// §2; consumer 0x2D8D80..0x2D8DFF). PROVEN.
pub const QUICK_USE_FIRST_BIT: u8 = 14;

/// The input bit that throws a grenade, held until the throw starts: the
/// client throws from the first pocket ([`POCKET_SLOTS`] in order) with a
/// count above 0 whose item is a grenade set (0x2FE8B0: `vtable+0x64` is
/// `mov eax, ecx` only in `grenade_set_core`'s table 0xA14824). PROVEN.
pub const GRENADE_BIT: u8 = 23;

/// The input bit that takes weapon 1 (slot 7) in hand. PROVEN
/// (`player_bots.md` §2).
pub const WEAPON_1_BIT: u8 = 12;

/// The input bit that takes weapon 2 (slot 10) in hand. PROVEN
/// (`player_bots.md` §2).
pub const WEAPON_2_BIT: u8 = 13;

/// One thing the server's bots wear ([`BOT_KIT`], [`BotRole::gear`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BotKitItem {
    /// The dictionary name (`items_dict`).
    pub name: &'static str,
    /// The profile slot.
    pub slot: u8,
    /// Units of a stack; 1 for an item that is no stack.
    pub count: u32,
    /// The bit of the input's `actions_mask` that uses it, if a key does.
    pub input_bit: Option<u8>,
}

const fn kit_item(name: &'static str, slot: u8, count: u32, input_bit: Option<u8>) -> BotKitItem {
    BotKitItem {
        name,
        slot,
        count,
        input_bit,
    }
}

/// The ammo slots a bot's weapon 1 fills with its ammo: one for each of a
/// player's three ammo bags (`item_params::player().ammo_bags_count`). The eight
/// ammo slots are one pool for both weapons (the client's table at
/// 0x9266A8, `session::v026g::rules::check_play`), so slot 11, weapon 2's
/// first, holds weapon 1's ammo as well: the bots carry no weapon 2 (the
/// owner's word, 2026-10-16: only primary weapons, and the most ammo).
pub const BOT_AMMO_SLOTS: [u8; 3] = [8, 9, 11];

/// Two medkits ("Medikit", `st_base_medkit_name`; `drugs`, heals 1, item
/// level 3, 20 silver) in the first pocket: quick use 1, as the first
/// filled slot of [`QUICK_USE_ORDER`]. One when the pockets hold one unit.
pub const BOT_MEDKIT: BotKitItem = kit_item("medkit", 13, 2, Some(QUICK_USE_FIRST_BIT));

/// The radio (`respawn_booster`, "Radio", `st_respawn_booster_name`; a
/// `booster` of item level 0, 1,500 silver, `fills_whole_pocket`): its
/// default modification 1220 is `respawn_time_modifier` -0.2 (type 1).
/// In the second pocket. **It works by being carried; no key.** PROVEN in
/// the client: a `survarium::booster` (vtable 0xA1CA34, constructor
/// 0x33F690) makes a modifier entry for each non-zero one of its 68
/// parameters; at every spawn the player's modifier lists are cleared
/// (`base_player::respawn` 0x2DACE4) and the inventory fill (0x2FCB70,
/// called at 0x2DADBB) calls slot 14 of each item in the 23 slots
/// (0x2FD1A5), which for a booster (0x33F960 → 0x33F990) adds those
/// entries to the kit's lists. At a death the respawn rule (0x316AA0)
/// sets the delay to `respawn_time × max(0, 1 + modifier 60)` through the
/// getter 0x2D5AE0, which sums the lists: 8 s × 0.8 = 6.4 s in Team
/// Deathmatch, 10 s × 0.8 = 8 s in the battery modes
/// (`docs/notes/battery_respawn.md`), then up to the 50 ms grid. That
/// the -0.2 of the default modification reaches the booster's
/// parameters is LIKELY (the client applies modifications to an item's
/// config, `item_config_cook`; the defaults' path there was not
/// traced).
pub const BOT_RADIO: BotKitItem = kit_item("respawn_booster", 14, 1, None);

/// Two RGD-5 frag grenades (`rgd-5`, `grenade`, item level 3, 30 silver;
/// the other grenade, `rdg-m`, is smoke at level 6) in the third pocket,
/// thrown with [`GRENADE_BIT`]. After the medkit, so the medkit's
/// quick-use key does not depend on whether a grenade or the radio counts
/// for quick use.
pub const BOT_GRENADE: BotKitItem = kit_item("rgd-5", 15, 2, Some(GRENADE_BIT));

/// The pockets every kit from gear level 3 on fills, in this order, as
/// far as its armour opens pockets ([`pockets`]): a stack takes at most a
/// pocket's units.
pub const BOT_POCKETS: [BotKitItem; 3] = [BOT_MEDKIT, BOT_RADIO, BOT_GRENADE];

/// Bandages (`bandages`, `drugs`, item level 1), the medkit's place below
/// gear level 3: the medkit and the grenade are of level 3, and the
/// lobby's equipment level is the highest piece's, pockets too.
pub const BOT_BANDAGES: BotKitItem = kit_item("bandages", 13, 2, Some(QUICK_USE_FIRST_BIT));

/// The pockets of a kit below gear level 3.
pub const LOW_POCKETS: [BotKitItem; 2] = [BOT_BANDAGES, BOT_RADIO];

/// The gear levels the server's bots wear (demo branch
/// `demo/bot-gear-levels`): one kit for each item level 1 to 15
/// ([`GEAR_KITS`]).
pub const GEAR_LEVELS: u8 = 15;

/// One gear level's kit: its armour, and weapon 1 with its ammo for each
/// role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GearKit {
    /// `(slot, item)`: helmet 0, mask 1, torso 2, back 3, legs 4, gloves
    /// 5, boots 6; a slot a piece covers is left empty.
    pub armour: &'static [(u8, &'static str)],
    /// Weapon 1 and its ammo, for the roles in [`BotRole::ALL`]'s order:
    /// an SMG (`wpn_smg`), an assault rifle, a shotgun (category 14), a
    /// sniper's rifle (category 13). The level's own where there is one,
    /// else the nearest level's; the lowest ammo that fits.
    pub weapons: [(&'static str, &'static str); 4],
}

const fn gear(
    armour: &'static [(u8, &'static str)],
    weapons: [(&'static str, &'static str); 4],
) -> GearKit {
    GearKit { armour, weapons }
}

/// The bots' kits by gear level, level 1 first: pieces of that item level,
/// or of the nearest where the level has none for a slot; the factions
/// mixed, scavengers' rags at the bottom and an exoskeleton at the top.
/// The 0.29a3 weapons and the STK2 pieces are in the content patches, not
/// in the test data: a level whose items the game data lacks wears the
/// level below it ([`resolved_level`]).
#[rustfmt::skip]
pub const GEAR_KITS: [GearKit; GEAR_LEVELS as usize] = [
    // 1: Scavengers' rags.
    gear(&[(2, "scav_torso_1"), (3, "scav_back_1"), (4, "scav_legs_1"), (5, "scav_gloves_1"), (6, "scav_boots_1")],
         [("kiparis", "ammo_9x18_makarov"), ("ppsh", "ammo_7.62x25"), ("toz_34", "ammo_12mm_buck"), ("mosin_nagant", "ammo_7.62x54r_old")]),
    // 2: Black Market's first pieces and the novice's helmet.
    gear(&[(0, "novice_026_helmet"), (2, "bm_torso_1"), (3, "scav_back_1"), (4, "bm_legs_1"), (5, "bm_gloves_1"), (6, "bm_boots_1")],
         [("uzi", "ammo_9x19p_para"), ("ppsh", "ammo_7.62x25"), ("izh_27_lancaster", "ammo_12mm_buck"), ("winchester_1894", "ammo_.30-30")]),
    // 3: Scavengers' level 3 with Black Market's helmet and backpack.
    gear(&[(0, "bm_helmet_1"), (1, "scav_resp_1"), (2, "scav_torso_2"), (3, "bm_back_1"), (4, "scav_legs_2"), (5, "scav_gloves_2"), (6, "scav_boots_2")],
         [("kedr", "ammo_9x18_makarov"), ("soc_vepr", "ammo_7.62x39"), ("neostead_2", "ammo_12mm_buck"), ("toz_122", "ammo_7.62x51")]),
    // 4: Black Market.
    gear(&[(0, "bm_helmet_5"), (2, "bm_torso_5"), (3, "bm_back_1"), (4, "bm_legs_5"), (5, "scav_gloves_2"), (6, "bm_boots_2")],
         [("vityaz", "ammo_9x19p_para"), ("soc_vepr", "ammo_7.62x39"), ("rem_870", "ammo_12mm_buck"), ("toz_122", "ammo_7.62x51")]),
    // 5: Scavengers' level 5 with Black Market's mask and boots.
    gear(&[(0, "scav_helmet_5"), (1, "bm_mask_1"), (2, "scav_torso_5"), (3, "bm_back_1"), (4, "scav_legs_5"), (5, "scav_gloves_5"), (6, "bm_boots_3")],
         [("ak_74u", "ammo_5.45x39_7n6m"), ("akmn", "ammo_7.62x39"), ("mp_153", "ammo_12mm_buck"), ("rem_700", "ammo_7.62x51")]),
    // 6: Edge's hunters.
    gear(&[(0, "edge_hunter_helmet_1"), (1, "edge_resp_1"), (2, "edge_hunter_torso_1"), (3, "edge_hunter_back_1"), (4, "edge_hunter_legs_1"), (5, "bm_gloves_2"), (6, "edge_hunter_boots_1")],
         [("ak_74u", "ammo_5.45x39_7n6m"), ("sks", "ammo_7.62x39"), ("spas_12", "ammo_12mm_buck"), ("rem_700", "ammo_7.62x51")]),
    // 7: Renaissance's level 6 (no armour is of level 7).
    gear(&[(0, "ren_helmet_1"), (1, "edge_resp_1"), (2, "ren_torso_1"), (3, "edge_hunter_back_1"), (4, "ren_legs_1"), (5, "scav_gloves_5"), (6, "ren_boots_1")],
         [("sr2m", "ammo_9x21"), ("akmn", "ammo_7.62x39"), ("benelli_m4", "ammo_12mm_buck"), ("rem_700", "ammo_7.62x51")]),
    // 8: Edge.
    gear(&[(0, "edge_helmet_3"), (1, "edge_resp_3"), (2, "edge_torso_3"), (3, "edge_back_3"), (4, "edge_legs_3"), (5, "edge_gloves_3"), (6, "edge_boots_3")],
         [("ump45", "ammo_45acp"), ("valmet", "ammo_7.62x39"), ("spas_12", "ammo_12mm_buck"), ("sv_98", "ammo_7.62x54_7n1")]),
    // 9: Renaissance's helmet and mask on Edge's level 9.
    gear(&[(0, "ren_helmet_2"), (1, "ren_mask_2"), (2, "edge_torso_2"), (3, "ren_back_3"), (4, "edge_legs_2"), (5, "ren_gloves_1"), (6, "edge_boots_2")],
         [("pp2000", "ammo_9x19p_7n31"), ("vz58", "ammo_7.62x39"), ("benelli_m4", "ammo_12mm_buck"), ("sv_98", "ammo_7.62x54_7n1")]),
    // 10: Renaissance's level 9 and the protective mask.
    gear(&[(0, "ren_helmet_2"), (1, "face_069d_protective_mask"), (2, "ren_torso_2"), (3, "ren_back_3"), (4, "ren_legs_2"), (5, "edge_gloves_2"), (6, "ren_boots_3")],
         [("pp2000", "ammo_9x19p_7n31"), ("ak_74m", "ammo_5.45x39_7n10"), ("vepr_12", "ammo_12mm_slug"), ("sv_98", "ammo_7.62x54_7n1")]),
    // 11: Edge's level 12 (no armour is of level 11).
    gear(&[(0, "edge_helmet_4"), (1, "face_069d_protective_mask"), (2, "edge_torso_4"), (3, "edge_back_3"), (4, "edge_legs_4"), (5, "edge_gloves_4"), (6, "edge_boots_4")],
         [("pp2000", "ammo_9x19p_7n31"), ("rpk", "ammo_5.45x39_7n10"), ("saiga_12", "ammo_12mm_slug"), ("svd", "ammo_7.62x54_7n1")]),
    // 12: Renaissance's level 12 with its level 8 oxygen pack.
    gear(&[(0, "ren_helmet_4"), (2, "ren_torso_4"), (3, "ren_back_3"), (4, "ren_legs_4"), (5, "ren_gloves_4"), (6, "ren_boots_4")],
         [("kriss_vector", "ammo_45acp"), ("m4a1", "ammo_5.56x45"), ("vepr_12", "ammo_12mm_slug"), ("svd", "ammo_7.62x54_7n1")]),
    // 13: Renaissance's level 14 helmet and mask over Edge's level 12 torso
    // (no armour is of level 13).
    gear(&[(0, "ren_helmet_5"), (1, "ren_mask_5"), (2, "edge_torso_4"), (3, "edge_back_5"), (4, "ren_legs_4"), (5, "ren_gloves_4"), (6, "ren_boots_4")],
         [("kriss_vector", "ammo_45acp"), ("val", "ammo_pab9"), ("saiga_12", "ammo_12mm_slug"), ("vss", "ammo_sp5")]),
    // 14: Edge.
    gear(&[(0, "edge_helmet_5"), (1, "edge_mask_5"), (2, "edge_torso_5"), (3, "edge_back_5"), (4, "edge_legs_5"), (5, "edge_gloves_5"), (6, "edge_boots_5")],
         [("fn_p90", "ammo_5.7x28"), ("hk416", "ammo_5.56x45"), ("vepr_12", "ammo_12mm_slug"), ("vsk_94", "ammo_sp5")]),
    // 15: Freedom's exoskeleton (the suit covers the back and the legs, the
    // helmet the mask). The camper keeps level 14's VSK-94: the VSSK (7 kg)
    // and the 33 kg exoskeleton are over the 40 kg a bot carries.
    gear(&[(0, "freedom_exo_helmet"), (2, "freedom_exo_suit"), (5, "freedom_exo_gloves"), (6, "freedom_exo_boots")],
         [("fn_p90", "ammo_5.7x28"), ("ak_12", "ammo_6.5x39"), ("saiga_12", "ammo_12mm_slug"), ("vsk_94", "ammo_sp5")]),
];

/// What the bots' kits carry over the 30 kg of a player: the
/// `max_carried_weight` modifier (index 5 of `skills::MODIFIER_NAMES`) of
/// skill 209 at its five ranks, +2 kg each, worn in every bot kit's
/// modifier block.
pub const BOT_CARRY_BONUS: f32 = 10.0;

/// The modifier index of `max_carried_weight`.
pub const MAX_CARRIED_WEIGHT_MODIFIER: usize = 5;

/// What a bot (or a player with skill 209 at five ranks) carries.
pub fn bot_carry_limit() -> f32 {
    item_params::player().max_carried_weight + BOT_CARRY_BONUS
}

/// The modifier block of a bot's kits: [`BOT_CARRY_BONUS`].
pub fn bot_modifiers() -> [f32; super::skills::MODIFIERS] {
    let mut modifiers = [0.0; super::skills::MODIFIERS];
    modifiers[MAX_CARRIED_WEIGHT_MODIFIER] = BOT_CARRY_BONUS;
    modifiers
}

/// The gear level of the server's `n`-th bot of a match (1 on): bot `i`
/// (0 on) wears level `1 + i mod 15`, so 15 bots show every level.
pub fn bot_level(n: usize) -> u8 {
    1 + (n.saturating_sub(1) % usize::from(GEAR_LEVELS)) as u8
}

/// A server bot's role: the match server's play personality of the same
/// name (`match-server` `bots::tuning::Personality::defaults`), and the
/// kind of weapon it carries at every gear level ([`GEAR_KITS`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BotRole {
    Flanker,
    Support,
    Rusher,
    Camper,
}

impl BotRole {
    /// Every role, in the match server's order of its personalities.
    pub const ALL: [BotRole; 4] = [
        BotRole::Flanker,
        BotRole::Support,
        BotRole::Rusher,
        BotRole::Camper,
    ];

    /// The name of the match server's personality.
    pub fn name(self) -> &'static str {
        match self {
            BotRole::Flanker => "flanker",
            BotRole::Support => "support",
            BotRole::Rusher => "rusher",
            BotRole::Camper => "camper",
        }
    }

    fn index(self) -> usize {
        BotRole::ALL
            .iter()
            .position(|&each| each == self)
            .expect("a role")
    }

    /// Its share of the bots (the personality's `share`).
    pub fn share(self) -> f32 {
        match self {
            BotRole::Flanker => 0.30,
            BotRole::Support => 0.25,
            BotRole::Rusher => 0.30,
            BotRole::Camper => 0.15,
        }
    }

    /// The most rounds of weapon 1 in each of [`BOT_AMMO_SLOTS`], fewer
    /// where the carry limit wants ([`bot_kit_items`]).
    pub fn rounds(self) -> u32 {
        match self {
            BotRole::Flanker => 150,
            BotRole::Support => 120,
            BotRole::Rusher => 40,
            BotRole::Camper => 30,
        }
    }

    /// Weapon 1 and its ammo at gear `level` (1 to 15, as given: not
    /// [`resolved_level`]).
    pub fn weapon(self, level: u8) -> (&'static str, &'static str) {
        GEAR_KITS[usize::from(level.clamp(1, GEAR_LEVELS) - 1)].weapons[self.index()]
    }

    /// The role whose weapon 1 and ammo a bot of this role carries in its
    /// second kit ([`bot_kits`]): another kind of ammo, so the second kit
    /// is full when the first is spent (the owner's word, 2026-10-16: a
    /// place's ammo lasts the match, a spawn does not refill it).
    pub fn backup(self) -> BotRole {
        match self {
            BotRole::Flanker => BotRole::Support,
            BotRole::Support => BotRole::Flanker,
            BotRole::Rusher => BotRole::Flanker,
            BotRole::Camper => BotRole::Support,
        }
    }

    /// The role whose weapon 1, at any gear level, is `dict_id`.
    pub fn of_weapon(dict_id: u16) -> Option<BotRole> {
        let dictionary = Dictionary::shared();
        BotRole::ALL.into_iter().find(|role| {
            (1..=GEAR_LEVELS).any(|level| {
                dictionary
                    .item_named(role.weapon(level).0)
                    .is_some_and(|item| item.dict_id == dict_id)
            })
        })
    }

    /// The role of a team's `k`-th bot (0 on): smooth weighted round
    /// robin over the shares, so any run of bots is as near the shares as
    /// it can be. The first four are a flanker, a rusher, a support and a
    /// camper; twenty are 6, 5, 6 and 3.
    pub fn nth(k: usize) -> BotRole {
        let mut credit = [0.0_f32; 4];
        let mut pick = BotRole::Flanker;
        for _ in 0..=k {
            for (credit, role) in credit.iter_mut().zip(BotRole::ALL) {
                *credit += role.share();
            }
            let best = (0..4).fold(0, |best, i| if credit[i] > credit[best] { i } else { best });
            credit[best] -= 1.0;
            pick = BotRole::ALL[best];
        }
        pick
    }
}

impl std::fmt::Display for BotRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The pieces of a kit of gear `level` with the weapon 1 and ammo of
/// `weapon_role`, its ammo in `ammo_slots`: weapon 1, its ammo, the
/// armour, then [`BOT_POCKETS`] as far as the armour opens pockets, each
/// stack at most a pocket's units. The rounds a slot are the role's
/// ([`BotRole::rounds`]) or fewer, so the kit weighs at most `limit` (to
/// the ten below). None: the game data lacks an item, or one does not fit
/// its slot.
pub fn gear_kit_items(
    level: u8,
    weapon_role: BotRole,
    ammo_slots: &[u8],
    limit: f32,
) -> Option<Vec<BotKitItem>> {
    let dictionary = Dictionary::shared();
    let kit = &GEAR_KITS[usize::from(level.clamp(1, GEAR_LEVELS) - 1)];
    let (weapon, ammo) = kit.weapons[weapon_role.index()];
    let mut items = vec![kit_item(weapon, 7, 1, Some(WEAPON_1_BIT))];
    items.extend(
        kit.armour
            .iter()
            .map(|&(slot, name)| kit_item(name, slot, 1, None)),
    );
    let armour =
        Loadout::from_names(dictionary, items.iter().map(|item| (item.slot, item.name))).ok()?;
    let (count, size) = pockets(&armour);
    let pocket_items: &[BotKitItem] = if level < 3 {
        &LOW_POCKETS
    } else {
        &BOT_POCKETS
    };
    for (n, pocket) in pocket_items.iter().enumerate().take(count as usize) {
        let item = dictionary.item_named(pocket.name)?;
        let units = if item_params::item_params(item.dict_id)?.fills_whole_pocket {
            1
        } else {
            pocket.count.min(size)
        };
        items.push(BotKitItem {
            slot: POCKET_SLOTS[n],
            count: units,
            ..*pocket
        });
    }
    let rest =
        Loadout::from_names(dictionary, items.iter().map(|item| (item.slot, item.name))).ok()?;
    let rest_weight: f32 = rest
        .items
        .iter()
        .zip(&items)
        .map(|(slot, item)| item_params::carried_weight(slot.dict_id, item.count))
        .sum();
    let ammo_item = dictionary.item_named(ammo)?;
    if !dictionary.compatible(rest.items[0].dict_id, ammo_item.dict_id) {
        return None;
    }
    let per_round = item_params::carried_weight(ammo_item.dict_id, 1);
    let fit = ((limit - rest_weight) / (per_round * ammo_slots.len() as f32)).max(0.0) as u32;
    let rounds = weapon_role.rounds().min(fit / 10 * 10);
    let ammo_items = ammo_slots
        .iter()
        .map(|&slot| kit_item(ammo, slot, rounds, None));
    items.splice(1..1, ammo_items);
    Loadout::from_names(dictionary, items.iter().map(|item| (item.slot, item.name))).ok()?;
    Some(items)
}

/// The kits of every role at every gear level, both kits, as given or of
/// the level below where the game data lacks an item.
/// A role's kits at a level: the level worn, the two kits' pieces and
/// their loadouts.
type BotEntry = (u8, [Vec<BotKitItem>; 2], [Loadout; 2]);

struct BotKits {
    /// By level (index 0: level 1), then role: the level worn and the two
    /// kits' pieces and loadouts.
    levels: Vec<Vec<BotEntry>>,
}

fn level_kits(level: u8) -> Option<Vec<BotEntry>> {
    let limit = bot_carry_limit();
    BotRole::ALL
        .into_iter()
        .map(|role| {
            let first = gear_kit_items(level, role, &BOT_AMMO_SLOTS, limit)?;
            let backup = gear_kit_items(level, role.backup(), &BOT_AMMO_SLOTS, limit)?;
            // The backup's weapon and ammo, then the first kit's armour
            // and pockets.
            let ammo = 1 + BOT_AMMO_SLOTS.len();
            let second: Vec<BotKitItem> = backup[..ammo]
                .iter()
                .chain(&first[ammo..])
                .copied()
                .collect();
            let loadouts = [bot_loadout(&first)?, bot_loadout(&second)?];
            Some((level, [first, second], loadouts))
        })
        .collect()
}

fn bot_kits_table() -> &'static BotKits {
    static SHARED: std::sync::OnceLock<BotKits> = std::sync::OnceLock::new();
    SHARED.get_or_init(|| {
        let mut levels: Vec<Vec<BotEntry>> = Vec::new();
        for level in 1..=GEAR_LEVELS {
            let kits = match level_kits(level) {
                Some(kits) => kits,
                None => match levels.last() {
                    Some(below) => below.clone(),
                    None => panic!("the bots' gear level 1 has items the game data lacks"),
                },
            };
            levels.push(kits);
        }
        BotKits { levels }
    })
}

fn bot_entry(role: BotRole, level: u8) -> &'static BotEntry {
    &bot_kits_table().levels[usize::from(level.clamp(1, GEAR_LEVELS) - 1)][role.index()]
}

/// The gear level a bot of gear `level` wears: `level`, or the highest
/// below it whose items the game data has (the test data has neither the
/// 0.29a3 weapons nor the STK2 pieces).
pub fn resolved_level(level: u8) -> u8 {
    bot_entry(BotRole::Flanker, level).0
}

/// Every piece a bot of `role` at gear `level` wears in its first kit:
/// weapon 1 and its ammo, the level's armour and the pockets.
pub fn bot_kit_items(role: BotRole, level: u8) -> Vec<BotKitItem> {
    bot_entry(role, level).1[0].clone()
}

/// Every piece of a bot of `role`'s second kit at gear `level`: its
/// [`BotRole::backup`]'s weapon 1 and ammo, then the rest of the first
/// kit (armour and pockets).
pub fn bot_second_kit_items(role: BotRole, level: u8) -> Vec<BotKitItem> {
    bot_entry(role, level).1[1].clone()
}

fn bot_loadout(items: &[BotKitItem]) -> Option<Loadout> {
    let mut loadout = Loadout::from_names(
        Dictionary::shared(),
        items.iter().map(|item| (item.slot, item.name)),
    )
    .ok()?;
    for (slot, item) in loadout.items.iter_mut().zip(items) {
        slot.count = item.count;
    }
    Some(loadout)
}

/// A bot of `role`'s two kits at gear `level` as loadouts: its own
/// ([`bot_kit_items`]) and the second ([`bot_second_kit_items`]).
pub fn bot_kits(role: BotRole, level: u8) -> [&'static Loadout; 2] {
    let [first, second] = &bot_entry(role, level).2;
    [first, second]
}

/// A bot of `role`'s first kit at gear `level` as a loadout.
pub fn bot_kit(role: BotRole, level: u8) -> &'static Loadout {
    bot_kits(role, level)[0]
}

/// The kit a player is given for gear `level` (the demo's presets,
/// `session::v026g::presets::demo_gear_presets`): the level's armour and
/// pockets, and the weapon of the role ([`player_role`]) whose weapon and
/// ammo are of the highest level not over it, so the lobby shows the
/// level where the armour lets it; its rounds in one ammo slot (a kit
/// holds an item in one slot), at most [`bot_carry_limit`].
pub fn player_gear_kit_items(level: u8) -> Option<Vec<BotKitItem>> {
    let level = resolved_level(level);
    gear_kit_items(
        level,
        player_role(level),
        &BOT_AMMO_SLOTS[..1],
        bot_carry_limit(),
    )
}

/// The role whose weapon 1 a player's kit of gear `level` carries: of the
/// four, the one whose weapon and ammo are of the highest matchmaking
/// level not over `level` (the support's first of two alike); the
/// support's when all are over it.
pub fn player_role(level: u8) -> BotRole {
    let dictionary = Dictionary::shared();
    let mm = |name: &str| {
        dictionary
            .item_named(name)
            .and_then(|item| item_params::item_params(item.dict_id))
            .map_or(u8::MAX, |params| params.match_making_level)
    };
    [
        BotRole::Support,
        BotRole::Flanker,
        BotRole::Camper,
        BotRole::Rusher,
    ]
    .into_iter()
    .map(|role| {
        let (weapon, ammo) = role.weapon(level);
        (role, mm(weapon).max(mm(ammo)))
    })
    .filter(|&(_, highest)| highest <= level)
    .fold(
        None,
        |best: Option<(BotRole, u8)>, (role, highest)| match best {
            Some((_, kept)) if kept >= highest => best,
            _ => Some((role, highest)),
        },
    )
    .map_or(BotRole::Support, |(role, _)| role)
}

/// What a weapon's config says of how it fires and reloads
/// (`gameplay/items/weapons/wpn_<name>.options`): for the match server's
/// bots, which count the rounds they fire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponFacts {
    pub magazine: u32,
    pub rounds_per_minute: f32,
    pub reload_s: f32,
    /// The first fire mode's queue: -1 automatic, else rounds a press.
    pub first_queue: i32,
    pub show_s: f32,
    pub hide_s: f32,
    /// The reload state's `state_type`: 303 a magazine, 306 round by
    /// round, 311 two barrels, 314 a pistol's (with a reload from empty).
    pub reload_state: i32,
    /// It has a `chamber_a_round` state (a bolt or a pump).
    pub chamber: bool,
}

/// [`WeaponFacts`] of the weapon `dict_id`; none for an item that is no
/// weapon or whose config the game data lacks.
pub fn weapon_facts(dict_id: u16) -> Option<WeaponFacts> {
    let item = Dictionary::shared().item(dict_id)?;
    let config = ConfigValue::parse(super::game_data::shared().file(&item.cfg_name)?).ok()?;
    let parameters = config.get("parameters")?;
    let number = |key: &str| parameters.get(key).and_then(ConfigValue::as_f32);
    let states = config.get("states")?;
    Some(WeaponFacts {
        magazine: number("magazine_capacity")? as u32,
        rounds_per_minute: number("rounds_per_minute")?,
        reload_s: number("reload_time")?,
        first_queue: parameters
            .get("fire_queue_types")
            .and_then(|queues| queues.children().first().and_then(|queue| queue.as_i32()))
            .unwrap_or(1),
        show_s: number("show_time").unwrap_or(0.8),
        hide_s: number("hide_time").unwrap_or(0.8),
        reload_state: states
            .get("reload")
            .and_then(|state| state.get("state_type"))
            .and_then(ConfigValue::as_i32)
            .unwrap_or(303),
        chamber: states.get("chamber_a_round").is_some(),
    })
}

/// The role a profile's weapon 1 (slot 7 of its first kit) says: what the
/// match server reads from the profile it has of each place
/// (`Formed::profiles`). `None` for a player, or any profile whose weapon
/// 1 is no role's.
pub fn bot_role_of(profile: &MatchProfile) -> Option<BotRole> {
    let kit = profile.kits.first()?;
    let (index, _) = kit.slots[7]?;
    let item = profile.items.get(usize::from(index))?;
    BotRole::of_weapon(item.dict_id)
}

/// What is wrong with a kit a bot or a player would wear, by the rules a
/// kit is held to: every item fits its slot (the dictionary's
/// `slots_restrictions`) and is no premium one; no item is worn in a slot
/// another worn item covers (the lobby's Play check, `CoveredSlot`); no
/// item type is in two pockets; weapon 1 is in slot 7, no weapon 2, and
/// every ammo slot holds ammo it fires, together at least its category's
/// `minimum_count` and a magazine (the Play check's `NoAmmo`); the pocket
/// items are in open pockets, a stack at most a pocket's units
/// ([`pockets`]); the kit weighs at most `limit`. Empty: none.
pub fn kit_problems(loadout: &Loadout, limit: f32) -> Vec<String> {
    let dictionary = Dictionary::shared();
    let mut problems = Vec::new();
    let name = |dict_id: u16| {
        dictionary
            .item(dict_id)
            .map_or_else(|| format!("#{dict_id}"), |item| item.name.clone())
    };
    let mut slots: Vec<u8> = loadout.items.iter().map(|item| item.slot).collect();
    slots.sort_unstable();
    slots.dedup();
    if slots.len() != loadout.items.len() {
        problems.push("two items in one slot".to_owned());
    }
    for item in &loadout.items {
        let Some(entry) = dictionary.item(item.dict_id) else {
            problems.push(format!("no item #{}", item.dict_id));
            continue;
        };
        if !dictionary.fits(item.dict_id, item.slot) {
            problems.push(format!("{} does not fit slot {}", entry.name, item.slot));
        }
        if entry.is_premium {
            problems.push(format!("{} is premium", entry.name));
        }
        if !entry.is_stack && item.count != 1 {
            problems.push(format!("{} x{}", entry.name, item.count));
        }
        for &covered in item_params::covered_slots(item.dict_id) {
            if let Some(under) = loadout.slot(covered) {
                problems.push(format!(
                    "{} covers slot {covered}, where {} is",
                    entry.name,
                    name(under.dict_id)
                ));
            }
        }
    }
    let mut pocket_types: Vec<u16> = loadout
        .items
        .iter()
        .filter(|item| POCKET_SLOTS.contains(&item.slot))
        .map(|item| item.dict_id)
        .collect();
    let in_pockets = pocket_types.len();
    pocket_types.sort_unstable();
    pocket_types.dedup();
    if pocket_types.len() != in_pockets {
        problems.push("an item type in two pockets".to_owned());
    }
    match loadout.slot(7) {
        None => problems.push("no weapon 1".to_owned()),
        Some(weapon) => {
            let mut rounds = 0_u64;
            for slot in [8_u8, 9, 11, 12] {
                if let Some(ammo) = loadout.slot(slot) {
                    if dictionary.compatible(weapon.dict_id, ammo.dict_id) {
                        rounds += u64::from(ammo.count);
                    } else {
                        problems.push(format!(
                            "{} in slot {slot} does not fit {}",
                            name(ammo.dict_id),
                            name(weapon.dict_id)
                        ));
                    }
                }
            }
            let category = dictionary.item(weapon.dict_id).map(|item| item.category);
            let minimum = dictionary
                .categories
                .iter()
                .find(|each| Some(each.id) == category)
                .map_or(0, |each| u64::from(each.minimum_count));
            let magazine = item_params::item_params(weapon.dict_id)
                .map_or(0, |params| u64::from(params.magazine_capacity));
            if rounds < minimum.max(magazine) {
                problems.push(format!(
                    "{} has {rounds} rounds (minimum {minimum}, magazine {magazine})",
                    name(weapon.dict_id)
                ));
            }
        }
    }
    if loadout.slot(10).is_some() {
        problems.push("a weapon 2".to_owned());
    }
    let (count, size) = pockets(loadout);
    for item in loadout
        .items
        .iter()
        .filter(|item| POCKET_SLOTS.contains(&item.slot))
    {
        if !POCKET_SLOTS[..count as usize].contains(&item.slot) {
            problems.push(format!(
                "{} in closed pocket {}",
                name(item.dict_id),
                item.slot
            ));
        }
        let whole = item_params::item_params(item.dict_id).is_some_and(|p| p.fills_whole_pocket);
        let most = if whole { 1 } else { size };
        if item.count == 0 || item.count > most {
            problems.push(format!(
                "{} x{} in a pocket of {most}",
                name(item.dict_id),
                item.count
            ));
        }
    }
    if loadout.weight() > limit {
        problems.push(format!("{:.2} kg over {limit} kg", loadout.weight()));
    }
    problems
}

/// What the type 1 (added) modifiers of the default modifications of
/// `dict_id` add to `parameter` (e.g. `pocket_slots_modifier`).
fn default_modifier(dict_id: u16, parameter: &str) -> f32 {
    let path = format!("parameters/{parameter}");
    let modifications = super::modifications::Modifications::shared();
    item_params::item_params(dict_id)
        .map_or(&[][..], |params| params.default_modifications)
        .iter()
        .filter_map(|&id| modifications.modification(id))
        .flat_map(|row| &row.modifiers)
        .filter(|modifier| modifier.kind == 1 && modifier.path == path)
        .map(|modifier| modifier.value)
        .sum()
}

/// The pockets a loadout opens and how many units each holds, with no
/// skills: the player's `pocket_slots_count` and `pocket_slot_size` (1 and
/// 1, `default.player`) plus what the worn items' default modifications
/// add, at most the six [`POCKET_SLOTS`].
///
/// LIKELY the client's rule: the lobby opens the first `item_slots` of
/// slots 13..=18 and empties and locks the others
/// (`PaperDollBase.handleQuickSlots`), a pocket takes `pocketSize` units
/// of an item, or one of an item that `fills_whole_pocket`
/// (`ItemRendererQSlot`, `MessageAsk`), and both numbers come from the
/// exe; pockets (15) and pocket size (17) are among the modifiers the
/// match's getter adds rather than multiplies (0x2D5AE0, table
/// 0x927E08). How the exe sums them was not read.
pub fn pockets(loadout: &Loadout) -> (u32, u32) {
    let added = |parameter| {
        loadout
            .items
            .iter()
            .map(|item| default_modifier(item.dict_id, parameter))
            .sum::<f32>()
            .max(0.0) as u32
    };
    let count = (item_params::player().pocket_slots_count + added("pocket_slots_modifier"))
        .min(POCKET_SLOTS.len() as u32);
    let size = item_params::player().pocket_slot_size + added("pocket_slot_size_modifier");
    (count, size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_026g_test_profiles() {
        let dictionary = Dictionary::shared();
        let loadouts = Loadouts::shared();
        assert_eq!(loadouts.profiles.len(), 10);
        for loadout in &loadouts.profiles {
            assert_eq!(loadout.items.len(), 13);
            for (slot, item) in loadout.items.iter().enumerate() {
                assert_eq!(usize::from(item.slot), slot);
                assert!(dictionary.fits(item.dict_id, item.slot));
            }
        }
        let name = |profile: usize, slot: u8| {
            let item = loadouts.profiles[profile].slot(slot).unwrap();
            dictionary.item(item.dict_id).unwrap().name.as_str()
        };
        assert_eq!(name(0, 0), "ren_helmet_1");
        assert_eq!(name(0, 2), "edge_torso_5");
        assert_eq!(name(0, 7), "ak_74u");
        assert_eq!(name(0, 8), "ammo_5.45x39_7n10");
        assert_eq!(name(0, 10), "spas_12");
        assert_eq!(name(2, 2), "ren_torso_6");
        assert_eq!(name(4, 2), "scav_torso_1");
        // A clip of each ammo, one of each piece of gear.
        assert_eq!(loadouts.profiles[0].slot(8).unwrap().count, 30);
        assert_eq!(loadouts.profiles[0].slot(11).unwrap().count, 15);
        assert_eq!(loadouts.profiles[0].slot(7).unwrap().count, 1);
        // Each ammo fits the weapon before it.
        for loadout in &loadouts.profiles {
            for (weapon, ammo) in [(7, 8), (7, 9), (10, 11), (10, 12)] {
                let (weapon, ammo) = (loadout.slot(weapon).unwrap(), loadout.slot(ammo).unwrap());
                assert!(dictionary.compatible(weapon.dict_id, ammo.dict_id));
            }
        }
    }

    #[test]
    fn three_profiles_are_over_the_carry_limit_and_dropped() {
        let loadouts = Loadouts::shared();
        let weights: Vec<f32> = loadouts.profiles.iter().map(Loadout::weight).collect();
        let expected = [32.35, 32.3, 38.4, 26.9, 19.3, 26.3, 22.3, 26.3, 26.3, 26.3];
        for (weight, expected) in weights.iter().zip(expected) {
            assert!((weight - expected).abs() < 0.01, "{weights:?}");
        }
        let fits: Vec<bool> = loadouts
            .profiles
            .iter()
            .map(Loadout::fits_carry_limit)
            .collect();
        assert_eq!(
            fits,
            [
                false, false, false, true, true, true, true, true, true, true
            ]
        );
        assert_eq!(loadouts.usable().count(), 7);
        assert!(loadouts.usable().all(|loadout| loadout.weight() <= 30.0));
    }

    #[test]
    fn the_starter_is_the_three_free_weapons_and_their_ammo() {
        let dictionary = Dictionary::shared();
        let name = |dict_id: u16| dictionary.item(dict_id).unwrap().name.as_str();
        let starter = starter();
        // Worn: the pistol and a clip, where they always were.
        assert_eq!(starter.items.len(), 2);
        let pistol = starter.slot(7).unwrap();
        let ammo = starter.slot(8).unwrap();
        assert_eq!(name(pistol.dict_id), "tt_33");
        assert_eq!((pistol.count, ammo.count), (1, 50));
        assert!(dictionary.compatible(pistol.dict_id, ammo.dict_id));
        // In the inventory: the Mosin and a clip, and the PPSh-41, which
        // fires the pistol's ammo. Each weapon fits both weapon slots.
        let stash = starter_stash();
        let stashed: Vec<(&str, u32)> = stash
            .iter()
            .map(|&(dict_id, units)| (name(dict_id), units))
            .collect();
        assert_eq!(
            stashed,
            [("mosin_nagant", 1), ("ammo_7.62x54r_old", 10), ("ppsh", 1)]
        );
        assert!(dictionary.compatible(stash[0].0, stash[1].0));
        assert!(dictionary.compatible(stash[2].0, ammo.dict_id));
        for weapon in [stash[0].0, stash[2].0] {
            assert!(dictionary.fits(weapon, 7) && dictionary.fits(weapon, 10));
        }
        assert!(dictionary.fits(stash[1].0, 8) && dictionary.fits(stash[1].0, 11));
        // The heaviest kit of them (the PPSh-41, the Mosin and a clip for
        // each) is far under the carry limit.
        let heaviest = Loadout::from_names(
            dictionary,
            [
                (7, "ppsh"),
                (8, "ammo_7.62x25"),
                (10, "mosin_nagant"),
                (11, "ammo_7.62x54r_old"),
            ],
        )
        .unwrap();
        assert!(heaviest.fits_carry_limit());
        // All are free and of level 0 in 0.26g's data, and they are all the
        // weapons and ammo that are.
        let mut owned: Vec<u16> = starter.items.iter().map(|item| item.dict_id).collect();
        owned.extend(stash.iter().map(|&(dict_id, _)| dict_id));
        owned.sort_unstable();
        for &dict_id in &owned {
            assert_eq!(dictionary.item(dict_id).unwrap().base_cost, 0);
            assert_eq!(item_params::item_params(dict_id).unwrap().item_level, 0);
        }
        let free: Vec<u16> = dictionary
            .items
            .iter()
            .filter(|item| matches!(item.category, 8 | 13..=21) && !item.is_premium)
            .filter(|item| item.base_cost == 0)
            .filter(|item| {
                item_params::item_params(item.dict_id)
                    .is_some_and(|params| params.item_level == 0 && !params.kind.is_empty())
            })
            .map(|item| item.dict_id)
            .collect();
        assert_eq!(
            free,
            owned,
            "{:?}",
            free.iter().map(|&id| name(id)).collect::<Vec<_>>()
        );
        assert!(starter.fits_carry_limit());
        // Something that does not fit its slot is refused.
        assert!(Loadout::from_names(dictionary, [(0, "tt_33")]).is_err());
        assert!(Loadout::from_names(dictionary, [(7, "no_such_item")]).is_err());
    }

    /// Every role's two kits at every gear level, as the game data has
    /// them (the test data lacks the 0.29a3 weapons and the STK2 pieces:
    /// those levels wear the level below): kits a bot can wear and a
    /// client can read, and the role reads back from them. The same
    /// checks on the content's data with every level its own are the
    /// integration test `tests/bot_gear_levels.rs`.
    #[test]
    fn every_gear_level_kit_passes_the_kit_rules() {
        let dictionary = Dictionary::shared();
        let mut seen = Vec::new();
        for level in 1..=GEAR_LEVELS {
            let resolved = resolved_level(level);
            assert!(resolved <= level && resolved >= 1);
            seen.push(resolved);
            for role in BotRole::ALL {
                let [first, second] = bot_kits(role, level);
                for (k, kit) in [first, second].into_iter().enumerate() {
                    let problems = kit_problems(kit, bot_carry_limit());
                    assert!(
                        problems.is_empty(),
                        "level {level} {role} kit {k}: {problems:?}"
                    );
                }
                assert_eq!(
                    BotRole::of_weapon(first.slot(7).unwrap().dict_id),
                    Some(role),
                    "level {level}"
                );
                assert_eq!(
                    BotRole::of_weapon(second.slot(7).unwrap().dict_id),
                    Some(role.backup()),
                    "level {level}"
                );
                // The rest of the second kit is the first's.
                let rest = |kit: &Loadout| -> Vec<(u8, u16, u32)> {
                    kit.items
                        .iter()
                        .filter(|item| item.slot != 7 && !BOT_AMMO_SLOTS.contains(&item.slot))
                        .map(|item| (item.slot, item.dict_id, item.count))
                        .collect()
                };
                assert_eq!(rest(first), rest(second));
                let player = Loadout::from_names(
                    dictionary,
                    player_gear_kit_items(level)
                        .unwrap()
                        .iter()
                        .map(|item| (item.slot, item.name)),
                )
                .unwrap();
                assert!(player.slot(7).is_some());
            }
        }
        // The test data has level 1 and 2 as given at least.
        assert_eq!(seen[0], 1);
        assert_eq!(bot_level(1), 1);
        assert_eq!(bot_level(15), 15);
        assert_eq!(bot_level(16), 1);
        assert_eq!(bot_level(20), 5);
    }

    #[test]
    fn the_gear_table_names_each_kind_of_weapon_for_its_role() {
        for (index, kit) in GEAR_KITS.iter().enumerate() {
            for (role, (weapon, _)) in BotRole::ALL.into_iter().zip(kit.weapons) {
                for other in BotRole::ALL.into_iter().filter(|&other| other != role) {
                    for level in 1..=GEAR_LEVELS {
                        assert_ne!(weapon, other.weapon(level).0, "level {}", index + 1);
                    }
                }
            }
            // Pieces in distinct armour slots.
            let mut slots: Vec<u8> = kit.armour.iter().map(|(slot, _)| *slot).collect();
            slots.sort_unstable();
            slots.dedup();
            assert_eq!(slots.len(), kit.armour.len());
            assert!(slots.iter().all(|&slot| slot <= 6));
        }
    }

    #[test]
    fn a_bots_role_reads_back_from_its_profile() {
        use super::super::match_msgs::static_info::MatchProfile;

        let dictionary = Dictionary::shared();
        for level in 1..=GEAR_LEVELS {
            for role in BotRole::ALL {
                let profile = MatchProfile::from_loadouts(
                    "Bot-1",
                    0,
                    &bot_kits(role, level),
                    bot_modifiers(),
                    dictionary,
                    1,
                );
                assert_eq!(bot_role_of(&profile), Some(role));
            }
        }
        // A player in the starter kit, or one with no kit, has no role.
        let player = MatchProfile::from_loadout(
            "Alice",
            0,
            starter(),
            [0.0; super::super::skills::MODIFIERS],
            dictionary,
            1,
        );
        assert_eq!(bot_role_of(&player), None);
        let empty = MatchProfile {
            kits: Vec::new(),
            ..player
        };
        assert_eq!(bot_role_of(&empty), None);
    }

    #[test]
    fn the_roles_follow_the_shares() {
        let shares: f32 = BotRole::ALL.iter().map(|role| role.share()).sum();
        assert!((shares - 1.0).abs() < 1e-6);
        let first: Vec<BotRole> = (0..4).map(BotRole::nth).collect();
        assert_eq!(
            first,
            [
                BotRole::Flanker,
                BotRole::Rusher,
                BotRole::Support,
                BotRole::Camper
            ]
        );
        // Any run from the start is within one bot of the shares.
        for n in 1..=40 {
            for role in BotRole::ALL {
                let have = (0..n).filter(|&k| BotRole::nth(k) == role).count() as f32;
                assert!(
                    (have - role.share() * n as f32).abs() <= 1.0,
                    "{role} of {n}"
                );
            }
        }
        let twenty: Vec<usize> = BotRole::ALL
            .iter()
            .map(|&role| (0..20).filter(|&k| BotRole::nth(k) == role).count())
            .collect();
        assert_eq!(twenty, [6, 5, 6, 3]);
    }
}

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
    /// What the profile's item of a stack holds in all
    /// (`amount_in_storage`, the client's refill of the slot at each spawn:
    /// [`BOT_AMMO_LIVES`]); `count` unless set.
    pub stored: u32,
    /// The modifications (upgrades) the item carries, by
    /// [`modifications`](super::modifications) id: none for a player's
    /// item here, the few of [`BOT_UPGRADES`] for a bot's.
    pub mods: &'static [u32],
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
                    stored: count,
                    mods: &[],
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

/// One thing the server's bots wear ([`bot_kit_items`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BotKitItem {
    /// The dictionary name (`items_dict`).
    pub name: &'static str,
    /// The profile slot.
    pub slot: u8,
    /// Units of a stack in the slot (what a life starts with); 1 for an
    /// item that is no stack.
    pub count: u32,
    /// The bit of the input's `actions_mask` that uses it, if a key does.
    pub input_bit: Option<u8>,
    /// The modifications it carries ([`SlotItem::mods`]).
    pub mods: &'static [u32],
}

const fn kit_item(name: &'static str, slot: u8, count: u32, input_bit: Option<u8>) -> BotKitItem {
    BotKitItem {
        name,
        slot,
        count,
        input_bit,
        mods: &[],
    }
}

/// The ammo slot a bot's kit fills with its weapon 1's ammo: all of it in
/// the first ammo slot. The lobby movie's ammo window gives one ammo type
/// as many rounds as the kit's ammo bags hold, in one slot or spread over
/// several (`AmmoShareElement.as`: a slot's most is `int(bags ×
/// clipsInBag) × clipSize`, the bags the other slots use taken off;
/// PROVEN), so one slot holds the kit's most ([`ammo_max`]). One slot is
/// one item of the profile with one storage, which the client refills the
/// slot from at every spawn ([`BOT_AMMO_LIVES`]). The eight ammo slots
/// are one pool for both weapons (the client's table at 0x9266A8,
/// `session::v026g::rules::check_play`); the bots carry no weapon 2 (the
/// owner's word, 2026-10-16: only primary weapons, and the most ammo).
pub const BOT_AMMO_SLOTS: [u8; 1] = [8];

/// How many lives of ammo each kit of a bot has for the match: the
/// profile's item holds this many times the slot's rounds in storage
/// (`amount_in_storage`). [choice, 2026-10-23: a finite store, so a bot
/// that fought through a kit's ammo switches kits; six full lives a kit,
/// eighteen over the three kits.]
///
/// What the client does with it (PROVEN, `inventory::setup_from_profile`
/// 0x2FCB70 at every spawn: a stack slot gets `min(slot count, the item's
/// storage)` and the storage loses that, 0x2FCC5F..0x2FCCBD; the removal
/// before a spawn, `unload_to_profile` 0x2FD2D0, puts the magazine and
/// what is left back into the storage, `client_patch.md` §3.20): every
/// life starts with the slot's rounds while the storage has them; only the
/// rounds fired are gone. Each kit's items are its own
/// (`MatchProfile::from_loadouts`: no item in two kits), so a kit chosen
/// for the next life (0xDE → 0xF3) brings its own storage.
pub const BOT_AMMO_LIVES: u32 = 6;

/// Two medkits ("Medikit", `st_base_medkit_name`; `drugs`, heals 1, item
/// level 3, 20 silver) in the first pocket: quick use 1, as the first
/// filled slot of [`QUICK_USE_ORDER`]. Two: a pocket's size in every
/// bot's kit (one pocket an item type, `one-pocket-a-type.md`).
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

/// What every kit of the match server's own bots has in its pockets
/// (`lobby-server` `play::server_bot`), whatever its tier and role:
/// medkits, the radio and grenades, a pocket's worth of each. The role's
/// armour opens three pockets of two in every tier ([`pockets`]).
pub const BOT_POCKETS: [BotKitItem; 3] = [BOT_MEDKIT, BOT_RADIO, BOT_GRENADE];

/// Whether the bots' armour carries [`BOT_UPGRADES`]: `SRP_BOT_UPGRADES`
/// unset or not off (`0`, `off`, `false`). Off gives the plain armour
/// back. Read once.
pub fn upgrades_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        std::env::var("SRP_BOT_UPGRADES")
            .map(|value| !matches!(value.trim(), "0" | "off" | "false"))
            .unwrap_or(true)
    })
}

/// The upgrades on the pieces the bots wear: `(tier, role, profile slot,
/// modification ids)`, the owner's SAFE SET of 2026-10-26: weight (type 18,
/// `1060 + strength`), carry capacity (type 24, `1120 + s`: +1 to +5 kg),
/// movement speed (type 23, `1110 + s`: +1 to +5 %) and stamina spent
/// (type 28, `1160 + s`: -2 to -10 %), never an aim time, a range or a
/// bolt's chamber time (a mod's strength `s` is 0 to 4). The bots of the
/// match server read the speed and the carry from the profile
/// ([`super::bot_skills::item_added`]); the clients apply every one.
///
/// **What a bot has follows what the players of its tier have** (402 live
/// matches, mean strength of the upgrade where a piece has it): movement
/// speed on 4 / 6 / 12 % of the pieces (strength 1.8 / 2.7 / 2.7), gear
/// weight on 6 / 10 / 15 % (0.8 / 1.6 / 1.8), carry on 4 / 3 / 6 % (1.9 /
/// 1.5 / 2.2), stamina spent on 2 / 4 / 6 %. One upgrade a bot at 4-5, two
/// at 9-10 and 13-15; the lighter roles (flanker, rusher) take speed (the
/// torso) and stamina (the boots), the heavier (support, camper) carry (the
/// legs) and weight (the torso), which gives back some of what their kit
/// weighs. The slots, not the pieces, so that a swap of a piece for
/// another that takes them keeps the row (the test checks every one fits).
pub const BOT_UPGRADES: [(BotTier, BotRole, u8, &[u32]); 20] = [
    (BotTier::Low, BotRole::Flanker, 2, &[1112]),
    (BotTier::Low, BotRole::Rusher, 2, &[1112]),
    (BotTier::Low, BotRole::Support, 2, &[1061]),
    (BotTier::Low, BotRole::Camper, 2, &[1061]),
    (BotTier::Mid, BotRole::Flanker, 2, &[1113]),
    (BotTier::Mid, BotRole::Flanker, 6, &[1162]),
    (BotTier::Mid, BotRole::Rusher, 2, &[1113]),
    (BotTier::Mid, BotRole::Rusher, 6, &[1162]),
    (BotTier::Mid, BotRole::Support, 2, &[1062]),
    (BotTier::Mid, BotRole::Support, 4, &[1121]),
    (BotTier::Mid, BotRole::Camper, 2, &[1062]),
    (BotTier::Mid, BotRole::Camper, 4, &[1121]),
    (BotTier::High, BotRole::Flanker, 2, &[1113]),
    (BotTier::High, BotRole::Flanker, 6, &[1162]),
    (BotTier::High, BotRole::Rusher, 2, &[1113]),
    (BotTier::High, BotRole::Rusher, 6, &[1162]),
    (BotTier::High, BotRole::Support, 2, &[1062]),
    (BotTier::High, BotRole::Support, 4, &[1122]),
    (BotTier::High, BotRole::Camper, 2, &[1062]),
    (BotTier::High, BotRole::Camper, 4, &[1122]),
];

/// The modifications a bot of `role` in `tier` wears on the piece in
/// `slot` ([`BOT_UPGRADES`]); none with [`upgrades_on`] off.
pub fn bot_upgrades(tier: BotTier, role: BotRole, slot: u8) -> &'static [u32] {
    bot_upgrades_with(tier, role, slot, upgrades_on())
}

/// [`bot_upgrades`] with the upgrades `on` or off.
pub fn bot_upgrades_with(tier: BotTier, role: BotRole, slot: u8, on: bool) -> &'static [u32] {
    if !on {
        return &[];
    }
    BOT_UPGRADES
        .iter()
        .find(|&&(of_tier, of_role, of_slot, _)| (of_tier, of_role, of_slot) == (tier, role, slot))
        .map_or(&[], |&(.., mods)| mods)
}

/// What a role wears in a tier besides [`BOT_POCKETS`]: weapon 1, the
/// ammo it fires (as many rounds as the kit's ammo bags hold:
/// [`ammo_max`]) and the armour, by profile slot (0 helmet, 1 mask,
/// 2 torso, 3 back, 4 legs, 5 gloves, 6 boots). **The tier tables
/// ([`TIER_4_5`], [`TIER_9_10`], [`TIER_13_15`]) are the one place to
/// change a bot's weapon or armour**; the tests check every kit made of
/// them (the slots, the tier's levels, the ammo fits the weapon, the
/// pockets, the carry limit, four different ammo types a tier).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleGear {
    pub weapon: &'static str,
    pub ammo: &'static str,
    pub armour: &'static [(u8, &'static str)],
}

/// A tier's [`RoleGear`] of each role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TierGear {
    pub flanker: RoleGear,
    pub support: RoleGear,
    pub rusher: RoleGear,
    pub camper: RoleGear,
}

impl TierGear {
    pub fn of(&self, role: BotRole) -> &RoleGear {
        match role {
            BotRole::Flanker => &self.flanker,
            BotRole::Support => &self.support,
            BotRole::Rusher => &self.rusher,
            BotRole::Camper => &self.camper,
        }
    }
}

/// Bots against players of equipment level 0 to 5: items of level 0 to 5,
/// none premium (the bots of 2026-10-16). The backpack `bm_back_1` (level
/// 3, its default modifications 1030 `pocket_slots_modifier` +1 and 1170
/// `pocket_slot_size_modifier` +1) and the trousers `bm_legs_5` (level 4,
/// 1030) are the cheapest pair that opens the two more pockets.
/// The weapons are the top players' picks at these levels (`pvb-tiers.md`).
/// - Flanker: the Vityaz (`vityaz`, SMG, level 4) with its cheapest 9x19
///   (`ammo_9x19p_para`, level 2), light Black Market armour
///   of level 1: `bm_torso_1` (3 kg, an ammo bag more) and `bm_boots_1`.
/// - Support: the AK-74U (`ak_74u`, the lowest of the assault rifle
///   category 8: level 5) with `ammo_5.45x39_7n6m` (level 5); `bm_torso_4`
///   (level 3, 6 kg, a pocket more) and the helmet `bm_helmet_1`.
/// - Rusher: the MP-153 (`mp_153`, semi-automatic shotgun, level 4) with
///   buckshot (`ammo_12mm_buck`, level 1); Scavengers' `scav_torso_1`
///   (level 1, 3 kg, an ammo bag more) and `scav_boots_1`.
/// - Camper: the Remington 700 (`rem_700`, bolt action, the sniper
///   category 13, level 5) with `ammo_7.62x51_ap` (level 5);
///   `scav_torso_2` (level 3, 6 kg) and `bm_helmet_1`.
pub const TIER_4_5: TierGear = TierGear {
    flanker: RoleGear {
        weapon: "vityaz",
        ammo: "ammo_9x19p_para",
        armour: &[
            (2, "bm_torso_1"),
            (3, "bm_back_1"),
            (4, "bm_legs_5"),
            (6, "bm_boots_1"),
        ],
    },
    support: RoleGear {
        weapon: "ak_74u",
        ammo: "ammo_5.45x39_7n6m",
        armour: &[
            (0, "bm_helmet_1"),
            (2, "bm_torso_4"),
            (3, "bm_back_1"),
            (4, "bm_legs_5"),
        ],
    },
    rusher: RoleGear {
        weapon: "mp_153",
        ammo: "ammo_12mm_buck",
        armour: &[
            (2, "scav_torso_1"),
            (3, "bm_back_1"),
            (4, "bm_legs_5"),
            (6, "scav_boots_1"),
        ],
    },
    camper: RoleGear {
        weapon: "rem_700",
        ammo: "ammo_7.62x51_ap",
        armour: &[
            (0, "bm_helmet_1"),
            (2, "scav_torso_2"),
            (3, "bm_back_1"),
            (4, "bm_legs_5"),
        ],
    },
};

/// Bots against players of equipment level 6 to 10: items of level 10 at
/// most, each kit of level 9 or 10 (`matchmaking::kit_level`: its highest
/// item), none premium. The pockets as in [`TIER_4_5`]: `bm_back_1` and a
/// piece with `pocket_slots_modifier` +1 (`ren_torso_3`, `ren_torso_2`
/// or `bm_legs_5`).
/// - Flanker: the PP-2000 (`pp2000`, SMG, level 9, the highest SMG; 20
///   rounds, 750 a minute) with `ammo_9x19p_7n31` (level 9, the only
///   ammo it fires); Renaissance's `ren_torso_3` (level 8, 6.5 kg, a
///   pocket more) and light Edge pieces of level 8-9.
/// - Support: the AKMN (`akmn`, assault rifle, level 6; the top players'
///   pick at these levels) with `ammo_7.62x39_7n23` (level 9); Edge's `edge_torso_2` (level 9, 8 kg,
///   an ammo bag more) and Renaissance's helmet and gloves of level 9.
/// - Rusher: the SPAS-12 (`spas_12`, level 7: no shotgun is higher) with
///   buckshot; the armour makes the kit level 9 (`edge_boots_2`).
/// - Camper: the SV-98 (`sv_98`, bolt-action sniper rifle, level 9: the
///   only sniper weapon of level 9-10; 10 rounds) with `ammo_7.62x54_7n1`
///   (level 9); Renaissance's pieces of level 8-9.
pub const TIER_9_10: TierGear = TierGear {
    flanker: RoleGear {
        weapon: "pp2000",
        ammo: "ammo_9x19p_7n31",
        armour: &[
            (0, "edge_helmet_3"),
            (2, "ren_torso_3"),
            (3, "bm_back_1"),
            (4, "edge_legs_2"),
            (5, "edge_gloves_2"),
            (6, "edge_boots_2"),
        ],
    },
    support: RoleGear {
        weapon: "akmn",
        ammo: "ammo_7.62x39_7n23",
        armour: &[
            (0, "ren_helmet_2"),
            (2, "edge_torso_2"),
            (3, "bm_back_1"),
            (4, "bm_legs_5"),
            (5, "ren_gloves_1"),
            (6, "edge_boots_2"),
        ],
    },
    rusher: RoleGear {
        weapon: "spas_12",
        ammo: "ammo_12mm_buck",
        armour: &[
            (0, "edge_helmet_3"),
            (2, "edge_torso_3"),
            (3, "bm_back_1"),
            (4, "bm_legs_5"),
            (5, "edge_gloves_2"),
            (6, "edge_boots_2"),
        ],
    },
    camper: RoleGear {
        weapon: "sv_98",
        ammo: "ammo_7.62x54_7n1",
        armour: &[
            (0, "ren_helmet_2"),
            (1, "ren_mask_2"),
            (2, "ren_torso_3"),
            (3, "bm_back_1"),
            (4, "ren_legs_2"),
            (5, "ren_gloves_1"),
            (6, "edge_boots_2"),
        ],
    },
};

/// Bots against players of equipment level 11 to 15: each kit of level 13
/// to 15, none premium. The pockets: Edge's backpack `edge_back_5` (level
/// 14, 4 kg, `pocket_slot_size_modifier` +1), its torso `edge_torso_5`
/// (level 14, 10 kg, a pocket and an ammo bag more) and Renaissance's
/// trousers `ren_legs_5` (level 14, 1.8 kg, a pocket more).
/// - Flanker: the SR-3M (`sr3m`, compact assault rifle, level 14; 20
///   rounds, 900 a minute; no SMG is above level 9) with `ammo_sp6`
///   (level 14, the only ammo it fires).
/// - Support: the AK-12 (`ak_12`, assault rifle, level 15) with
///   `ammo_6.5x39` (level 15, the only ammo it fires).
/// - Rusher: the SPAS-12 (level 7: no shotgun is higher) with buckshot,
///   the lightest armour of the tier.
/// - Camper: the VSK-94 (`vsk_94`, the sniper category's marksman rifle,
///   level 14; 20 rounds) with `ammo_sp5` (level 12, the only ammo it
///   fires).
pub const TIER_13_15: TierGear = TierGear {
    flanker: RoleGear {
        weapon: "sr3m",
        ammo: "ammo_sp6",
        armour: &[
            (0, "edge_helmet_4"),
            (2, "edge_torso_5"),
            (3, "edge_back_5"),
            (4, "ren_legs_5"),
            (5, "edge_gloves_5"),
            (6, "edge_boots_6"),
        ],
    },
    support: RoleGear {
        weapon: "ak_12",
        ammo: "ammo_6.5x39",
        armour: &[
            (0, "edge_helmet_5"),
            (1, "ren_mask_5"),
            (2, "edge_torso_5"),
            (3, "edge_back_5"),
            (4, "ren_legs_5"),
            (5, "edge_gloves_5"),
            (6, "edge_boots_5"),
        ],
    },
    rusher: RoleGear {
        weapon: "spas_12",
        ammo: "ammo_12mm_buck",
        armour: &[
            (2, "edge_torso_5"),
            (3, "edge_back_5"),
            (4, "ren_legs_5"),
            (5, "edge_gloves_5"),
            (6, "edge_boots_6"),
        ],
    },
    camper: RoleGear {
        weapon: "vsk_94",
        ammo: "ammo_sp5",
        armour: &[
            (0, "edge_helmet_5"),
            (1, "ren_mask_5"),
            (2, "edge_torso_5"),
            (3, "edge_back_5"),
            (4, "ren_legs_5"),
            (5, "edge_gloves_5"),
            (6, "edge_boots_5"),
        ],
    },
};

/// The equipment of the server's bots, picked by the equipment level of
/// the players they play against (`matchmaking::gear_level`, the highest
/// of a match's players: the owner's of 2026-10-23). In the bots'
/// tuning JSON by its band: `"4-5"`, `"9-10"`, `"13-15"`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum BotTier {
    /// Players of level 0 to 5: bots of level 4-5 ([`TIER_4_5`]).
    #[default]
    #[serde(rename = "4-5")]
    Low,
    /// Players of level 6 to 10: bots of level 9-10 ([`TIER_9_10`]).
    #[serde(rename = "9-10")]
    Mid,
    /// Players of level 11 to 15: bots of level 13-15 ([`TIER_13_15`]).
    #[serde(rename = "13-15")]
    High,
}

impl BotTier {
    pub const ALL: [BotTier; 3] = [BotTier::Low, BotTier::Mid, BotTier::High];

    /// The tier for players whose highest equipment level is `gear`.
    pub fn of_gear(gear: u8) -> Self {
        match gear {
            0..=5 => BotTier::Low,
            6..=10 => BotTier::Mid,
            _ => BotTier::High,
        }
    }

    /// The equipment levels of its kits (`matchmaking::kit_level`).
    pub fn levels(self) -> std::ops::RangeInclusive<u8> {
        match self {
            BotTier::Low => 4..=5,
            BotTier::Mid => 9..=10,
            BotTier::High => 13..=15,
        }
    }

    /// Its table.
    pub fn gear(self) -> &'static TierGear {
        match self {
            BotTier::Low => &TIER_4_5,
            BotTier::Mid => &TIER_9_10,
            BotTier::High => &TIER_13_15,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BotTier::Low => "4-5",
            BotTier::Mid => "9-10",
            BotTier::High => "13-15",
        }
    }
}

impl std::fmt::Display for BotTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A server bot's role: the match server's play personality of the same
/// name (`match-server` `bots::tuning::Personality::defaults`), and the
/// weapon and armour it wears for it in each tier ([`TierGear::of`]).
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

    /// Its share of the bots (the personality's `share`).
    pub fn share(self) -> f32 {
        match self {
            BotRole::Flanker => 0.30,
            BotRole::Support => 0.25,
            BotRole::Rusher => 0.30,
            BotRole::Camper => 0.15,
        }
    }

    /// The roles whose weapon 1 and ammo its kits carry, its own first
    /// ([`bot_kits`]): each kit another weapon and another ammo, so a kit
    /// whose ammo is spent has others full (the owner's words, 2026-10-16
    /// and 2026-10-23: at least two kits, three allowed, each with its own
    /// ammo). The SMG and the assault rifle back each other up; the
    /// shotgun and the sniper weapon, slow to fire, get the two nearer
    /// their range.
    pub fn kit_roles(self) -> [BotRole; 3] {
        match self {
            BotRole::Flanker => [BotRole::Flanker, BotRole::Support, BotRole::Rusher],
            BotRole::Support => [BotRole::Support, BotRole::Flanker, BotRole::Camper],
            BotRole::Rusher => [BotRole::Rusher, BotRole::Flanker, BotRole::Support],
            BotRole::Camper => [BotRole::Camper, BotRole::Support, BotRole::Flanker],
        }
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

/// The ammo bags a kit of `armour` has with no skills: the player's
/// `ammo_bags_count` (3, `default.player`) and what the armour's default
/// modifications add (`ammo_bags_modifier`). LIKELY the client's count:
/// the lobby takes the bags of a kit from the exe (`set_ammo_bags_count`,
/// 0x222330), and ammo bags (16) are among the modifiers the match's
/// getter adds (0x2D5AE0, table 0x927E08); how the exe sums them was not
/// read, as for [`pockets`].
pub fn ammo_bags<'a>(armour: impl IntoIterator<Item = &'a str>) -> u32 {
    let dictionary = Dictionary::shared();
    let added: f32 = armour
        .into_iter()
        .filter_map(|name| dictionary.item_named(name))
        .map(|item| default_modifier(item.dict_id, "ammo_bags_modifier"))
        .sum();
    item_params::player().ammo_bags_count + added.max(0.0) as u32
}

/// The most rounds of `ammo` a kit with `bags` ammo bags takes: the lobby
/// movie's `int(int(bags × clipsInBag) × clipSize)`, `clipsInBag` being
/// `1 / required_bags` and `clipSize` the ammo's `clip_size`
/// (`AmmoShareElement.as`, `ItemFactory.as`; PROVEN in the movie).
pub fn ammo_max(ammo: &str, bags: u32) -> u32 {
    let Some(params) = Dictionary::shared()
        .item_named(ammo)
        .and_then(|item| item_params::item_params(item.dict_id))
    else {
        return 0;
    };
    (bags / params.required_bags.max(1)) * params.clip_size
}

/// Every piece of the `k`-th kit (0 on) of a bot of `role` in `tier`:
/// weapon 1 of [`BotRole::kit_roles`]`[k]` and its ammo, the most the
/// kit's bags take, in [`BOT_AMMO_SLOTS`]; the role's own armour; then
/// [`BOT_POCKETS`].
pub fn bot_kit_items(tier: BotTier, role: BotRole, k: usize) -> Vec<BotKitItem> {
    let own = tier.gear().of(role);
    let arms = tier.gear().of(role.kit_roles()[k]);
    let rounds = ammo_max(
        arms.ammo,
        ammo_bags(own.armour.iter().map(|&(_, name)| name)),
    );
    let mut items = vec![kit_item(arms.weapon, 7, 1, Some(WEAPON_1_BIT))];
    items.extend(
        BOT_AMMO_SLOTS
            .iter()
            .map(|&slot| kit_item(arms.ammo, slot, rounds, None)),
    );
    items.extend(own.armour.iter().map(|&(slot, name)| BotKitItem {
        mods: bot_upgrades(tier, role, slot),
        ..kit_item(name, slot, 1, None)
    }));
    items.extend(BOT_POCKETS);
    items
}

fn bot_loadout(items: &[BotKitItem]) -> Loadout {
    let mut loadout = Loadout::from_names(
        Dictionary::shared(),
        items.iter().map(|item| (item.slot, item.name)),
    )
    .expect("the bot kit's items exist and fit their slots");
    for (slot, item) in loadout.items.iter_mut().zip(items) {
        slot.count = item.count;
        slot.mods = item.mods;
        // The ammo's storage: [`BOT_AMMO_LIVES`] lives of it.
        slot.stored = if BOT_AMMO_SLOTS.contains(&item.slot) {
            item.count * BOT_AMMO_LIVES
        } else {
            item.count
        };
    }
    loadout
}

/// A bot of `role` in `tier`'s kits as loadouts, one for each of its
/// [`BotRole::kit_roles`] ([`bot_kit_items`]).
pub fn bot_kits(tier: BotTier, role: BotRole) -> [&'static Loadout; 3] {
    static SHARED: std::sync::OnceLock<Vec<[Loadout; 3]>> = std::sync::OnceLock::new();
    let kits = SHARED.get_or_init(|| {
        BotTier::ALL
            .into_iter()
            .flat_map(|tier| BotRole::ALL.into_iter().map(move |role| (tier, role)))
            .map(|(tier, role)| std::array::from_fn(|k| bot_loadout(&bot_kit_items(tier, role, k))))
            .collect()
    });
    let index = BotTier::ALL
        .iter()
        .position(|&each| each == tier)
        .expect("a tier")
        * BotRole::ALL.len()
        + BotRole::ALL
            .iter()
            .position(|&each| each == role)
            .expect("a role");
    let [first, second, third] = &kits[index];
    [first, second, third]
}

/// A bot of `role` in `tier`'s first kit as a loadout.
pub fn bot_kit(tier: BotTier, role: BotRole) -> &'static Loadout {
    bot_kits(tier, role)[0]
}

/// The tier and role a profile says, what the match server reads from the
/// profile it has of each place (`Formed::profiles`): the bot whose first
/// kit holds exactly the items of the profile's first kit, slot by slot;
/// else (a profile of an older kit) the role whose weapon 1 of the lowest
/// tier is the profile's, in the lowest tier. `None` for a player, or any
/// profile whose weapon 1 is no role's.
pub fn bot_of(profile: &MatchProfile) -> Option<(BotTier, BotRole)> {
    let kit = profile.kits.first()?;
    let dict_id = |slot: usize| -> Option<u16> {
        let (index, _) = kit.slots[slot]?;
        Some(profile.items.get(usize::from(index))?.dict_id)
    };
    let held: Vec<(u8, u16)> = (0..kit.slots.len())
        .filter_map(|slot| Some((slot as u8, dict_id(slot)?)))
        .collect();
    let exact = BotTier::ALL
        .into_iter()
        .flat_map(|tier| BotRole::ALL.into_iter().map(move |role| (tier, role)))
        .find(|&(tier, role)| {
            let wanted: Vec<(u8, u16)> = bot_kit(tier, role)
                .items
                .iter()
                .map(|item| (item.slot, item.dict_id))
                .collect();
            let mut wanted = wanted;
            wanted.sort_unstable();
            wanted == held
        });
    if exact.is_some() {
        return exact;
    }
    let weapon = dict_id(7)?;
    let dictionary = Dictionary::shared();
    BotRole::ALL
        .into_iter()
        .find(|role| {
            dictionary
                .item_named(BotTier::Low.gear().of(*role).weapon)
                .is_some_and(|item| item.dict_id == weapon)
        })
        .map(|role| (BotTier::Low, role))
}

/// The role [`bot_of`] says.
pub fn bot_role_of(profile: &MatchProfile) -> Option<BotRole> {
    bot_of(profile).map(|(_, role)| role)
}

/// The roles of the server's bots on `teams`, in order: each team's `k`-th
/// bot gets `BotRole::nth(k)` (the shares by smooth weighted round robin: a
/// flanker, a rusher, a support, a camper, ...), so it depends only on the
/// bots' places and the same roster gets the same roles.
///
/// One home for the lobby (`lobby-server` `play.rs`, which gives the bots
/// of a match their roles) and for the real-map bench
/// (`match-server` `bots/tests/bench.rs`, whose bots must carry what the
/// lobby gives).
pub fn bot_roles(teams: &[u8]) -> Vec<BotRole> {
    let mut on_team = [0_usize; 2];
    teams
        .iter()
        .map(|&team| {
            let k = &mut on_team[usize::from(team.min(1))];
            let role = BotRole::nth(*k);
            *k += 1;
            role
        })
        .collect()
}

/// The name the server's `n`-th bot of a match plays under (1 on).
pub fn bot_name(n: usize) -> String {
    format!("Bot-{n}")
}

/// The profile of the server's bot number `n` of its match (1 on) of `role`
/// on `team` with the equipment of `tier`: the role's three kits of the
/// tier ([`bot_kits`]), each another weapon in slot 7 with its own ammo, the
/// role's armour and the pockets; its kits carry the skills of their gear
/// level (`bot_skills::bot_modifiers`), as a player's carry the player's.
/// The match server reads the tier and the role back from it ([`bot_of`]).
///
/// One home for the lobby (`play::server_bot`) and for the real-map bench.
pub fn bot_profile(n: usize, team: u8, role: BotRole, tier: BotTier) -> MatchProfile {
    let kits = bot_kits(tier, role);
    let mut profile = MatchProfile::from_loadouts(
        &bot_name(n),
        team,
        &kits,
        super::bot_skills::bot_modifiers(&kits),
        Dictionary::shared(),
        1,
    );
    profile.clan_tag = "BOT".to_owned();
    profile
}

/// What the type 1 (added) modifiers of the default modifications of
/// `dict_id` add to `parameter` (e.g. `pocket_slots_modifier`).
pub fn default_modifier(dict_id: u16, parameter: &str) -> f32 {
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

    /// Every bot of every tier, role and kit.
    fn every_kit() -> impl Iterator<Item = (BotTier, BotRole, usize, &'static Loadout)> {
        BotTier::ALL.into_iter().flat_map(|tier| {
            BotRole::ALL.into_iter().flat_map(move |role| {
                bot_kits(tier, role)
                    .into_iter()
                    .enumerate()
                    .map(move |(k, kit)| (tier, role, k, kit))
            })
        })
    }

    /// `matchmaking::kit_level` of a loadout: the highest
    /// `match_making_level` of what it holds, rounded.
    fn kit_level(kit: &Loadout) -> u8 {
        kit.items
            .iter()
            .map(|item| {
                item_params::item_params(item.dict_id)
                    .unwrap()
                    .match_making_level
            })
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn the_tier_follows_the_players_equipment_level() {
        // The owner's examples: 12 → 13-15, 8 → 9-10, 3 → 4-5.
        assert_eq!(BotTier::of_gear(12), BotTier::High);
        assert_eq!(BotTier::of_gear(8), BotTier::Mid);
        assert_eq!(BotTier::of_gear(3), BotTier::Low);
        for (gear, tier) in [
            (0, BotTier::Low),
            (5, BotTier::Low),
            (6, BotTier::Mid),
            (10, BotTier::Mid),
            (11, BotTier::High),
            (15, BotTier::High),
        ] {
            assert_eq!(BotTier::of_gear(gear), tier, "{gear}");
        }
        assert_eq!(BotTier::Low.levels(), 4..=5);
        assert_eq!(BotTier::Mid.levels(), 9..=10);
        assert_eq!(BotTier::High.levels(), 13..=15);
    }

    /// Each kit is of its tier's equipment level (the highest item; no
    /// item above the tier), fits its slots, holds nothing premium, and a
    /// player could carry it with no skills.
    #[test]
    fn every_bot_kit_is_of_its_tiers_level_and_within_the_carry_limit() {
        let dictionary = Dictionary::shared();
        let limit = item_params::player().max_carried_weight;
        for (tier, role, k, kit) in every_kit() {
            let at = format!("{tier} {role} kit {k}");
            assert!(
                tier.levels().contains(&kit_level(kit)),
                "{at}: {}",
                kit_level(kit)
            );
            for item in &kit.items {
                let entry = dictionary.item(item.dict_id).unwrap();
                assert!(
                    dictionary.fits(item.dict_id, item.slot),
                    "{at}: {}",
                    entry.name
                );
                assert!(!entry.is_premium, "{at}: {}", entry.name);
                let level = item_params::item_params(item.dict_id)
                    .unwrap()
                    .match_making_level;
                assert!(
                    level <= *tier.levels().end(),
                    "{at}: {} {level}",
                    entry.name
                );
                if !entry.is_stack {
                    assert_eq!(item.count, 1, "{at}: {}", entry.name);
                }
            }
            let mut slots: Vec<u8> = kit.items.iter().map(|item| item.slot).collect();
            slots.sort_unstable();
            slots.dedup();
            assert_eq!(slots.len(), kit.items.len(), "{at}: a slot twice");
            assert!(kit.weight() <= limit, "{at}: {} kg", kit.weight());
        }
    }

    /// The owner's of 2026-10-23: each kit its own ammo, maxed. The slot
    /// holds the most the kit's ammo bags take (the lobby's rule,
    /// [`ammo_max`]: one bag more would hold more), and the item's storage
    /// [`BOT_AMMO_LIVES`] times that.
    #[test]
    fn every_kits_ammo_is_the_most_its_bags_take() {
        let dictionary = Dictionary::shared();
        let name = |dict_id: u16| dictionary.item(dict_id).unwrap().name.as_str();
        for (tier, role, k, kit) in every_kit() {
            let at = format!("{tier} {role} kit {k}");
            let weapon = kit.slot(7).unwrap();
            assert!(kit.slot(10).is_none(), "{at}: no weapon 2");
            let armour: Vec<&str> = kit
                .items
                .iter()
                .filter(|item| item.slot <= 6)
                .map(|item| name(item.dict_id))
                .collect();
            let bags = ammo_bags(armour.iter().copied());
            assert!(bags >= item_params::player().ammo_bags_count, "{at}");
            for slot in BOT_AMMO_SLOTS {
                let ammo = kit.slot(slot).unwrap();
                assert!(dictionary.compatible(weapon.dict_id, ammo.dict_id), "{at}");
                let most = ammo_max(name(ammo.dict_id), bags);
                assert_eq!(ammo.count, most, "{at}");
                assert!(ammo_max(name(ammo.dict_id), bags + 2) > most, "{at}");
                assert!(ammo.count >= item_params::shop_unit(ammo.dict_id), "{at}");
                assert_eq!(ammo.stored, ammo.count * BOT_AMMO_LIVES, "{at}");
            }
            // No other ammo slot is used.
            for slot in [8, 9, 11, 12, 19, 20, 21, 22] {
                if !BOT_AMMO_SLOTS.contains(&slot) {
                    assert!(kit.slot(slot).is_none(), "{at}: slot {slot}");
                }
            }
        }
        // The lobby's numbers: four bags of 9x19 (clips of 50) with the
        // light torso, three of 5.45x39 (30) with the support's.
        let rounds = |tier, role| bot_kit(tier, role).slot(8).unwrap().count;
        assert_eq!(rounds(BotTier::Low, BotRole::Flanker), 200);
        assert_eq!(rounds(BotTier::Low, BotRole::Support), 90);
        assert_eq!(rounds(BotTier::Low, BotRole::Rusher), 60);
        assert_eq!(rounds(BotTier::Low, BotRole::Camper), 30);
    }

    /// Three kits a bot (at least two: the owner's of 2026-10-23), each
    /// with another weapon and another ammo, so a kit chosen when one is
    /// spent is full; the rest (armour, pockets, keys) the same.
    #[test]
    fn every_bot_has_three_kits_of_different_weapons_and_ammo() {
        for tier in BotTier::ALL {
            // The four roles' ammo differ in each tier.
            let mut ammo: Vec<&str> = BotRole::ALL
                .iter()
                .map(|&role| tier.gear().of(role).ammo)
                .collect();
            ammo.sort_unstable();
            ammo.dedup();
            assert_eq!(ammo.len(), 4, "{tier}");
            for role in BotRole::ALL {
                let kits = bot_kits(tier, role);
                assert!(kits.len() >= 2);
                assert!(kits.len() <= super::super::match_msgs::static_info::MAX_KITS);
                assert_eq!(role.kit_roles()[0], role);
                for (a, first) in kits.iter().enumerate() {
                    for second in &kits[a + 1..] {
                        let at = format!("{tier} {role}");
                        assert_ne!(first.slot(7), second.slot(7), "{at}: one weapon");
                        assert_ne!(
                            first.slot(8).unwrap().dict_id,
                            second.slot(8).unwrap().dict_id,
                            "{at}: one ammo"
                        );
                        let rest = |kit: &Loadout| -> Vec<SlotItem> {
                            kit.items
                                .iter()
                                .filter(|item| {
                                    item.slot != 7 && !BOT_AMMO_SLOTS.contains(&item.slot)
                                })
                                .copied()
                                .collect()
                        };
                        assert_eq!(rest(first), rest(second), "{at}");
                    }
                }
                // Each kit's weapon is its kit role's.
                let dictionary = Dictionary::shared();
                for (kit, kit_role) in kits.iter().zip(role.kit_roles()) {
                    let weapon = dictionary.item(kit.slot(7).unwrap().dict_id).unwrap();
                    assert_eq!(weapon.name, tier.gear().of(kit_role).weapon);
                }
            }
        }
    }

    #[test]
    fn the_bots_kit_has_a_pocket_for_each_thing_it_uses() {
        let dictionary = Dictionary::shared();
        let params = |dict_id: u16| item_params::item_params(dict_id).unwrap();
        for (tier, role, k, kit) in every_kit() {
            let at = format!("{tier} {role} kit {k}");
            // Three pockets at least, of two: a pocket's worth of medkits
            // and of grenades (one pocket a type), and the radio.
            let (count, size) = pockets(kit);
            assert!(count >= 3, "{at}: {count} pockets");
            assert_eq!(size, 2, "{at}");
            let open = &POCKET_SLOTS[..count as usize];
            for item in kit
                .items
                .iter()
                .filter(|item| POCKET_SLOTS.contains(&item.slot))
            {
                assert!(open.contains(&item.slot), "{at}: slot {}", item.slot);
                let most = if params(item.dict_id).fills_whole_pocket {
                    1
                } else {
                    size
                };
                assert_eq!(item.count, most, "{at}: slot {}", item.slot);
            }
            let mut types: Vec<u16> = kit
                .items
                .iter()
                .filter(|item| POCKET_SLOTS.contains(&item.slot))
                .map(|item| item.dict_id)
                .collect();
            let pocketed = types.len();
            types.sort_unstable();
            types.dedup();
            assert_eq!(types.len(), pocketed, "{at}: a type in two pockets");

            // The keys: weapon 1 its bit; the medkit is quick use 1, the
            // first filled slot of the client's order; the grenade is the
            // first one in the pockets.
            let filled: Vec<u8> = QUICK_USE_ORDER
                .iter()
                .copied()
                .filter(|&slot| kit.slot(slot).is_some())
                .collect();
            let bit = BOT_MEDKIT.input_bit.unwrap();
            assert_eq!(
                filled[usize::from(bit - QUICK_USE_FIRST_BIT)],
                BOT_MEDKIT.slot,
                "{at}"
            );
            let first_grenade = POCKET_SLOTS.iter().copied().find(|&slot| {
                kit.slot(slot)
                    .is_some_and(|item| params(item.dict_id).kind == "grenade")
            });
            assert_eq!(first_grenade, Some(BOT_GRENADE.slot), "{at}");
            let items = bot_kit_items(tier, role, k);
            assert_eq!(items[0].slot, 7);
            assert_eq!(items[0].input_bit, Some(WEAPON_1_BIT));
        }
        assert_eq!(pockets(starter()), (1, 1));
        assert_eq!(BOT_RADIO.input_bit, None);
        assert_eq!(BOT_GRENADE.input_bit, Some(GRENADE_BIT));

        // What each is.
        let kit = bot_kit(BotTier::Low, BotRole::Flanker);
        let kind = |item: BotKitItem| params(kit.slot(item.slot).unwrap().dict_id).kind;
        assert_eq!(kind(BOT_MEDKIT), "drugs");
        assert_eq!(kind(BOT_RADIO), "booster");
        assert_eq!(kind(BOT_GRENADE), "grenade");
        let radio = kit.slot(BOT_RADIO.slot).unwrap().dict_id;
        assert_eq!(params(radio).default_modifications, [1220]);
        assert!((default_modifier(radio, "respawn_time_modifier") + 0.2).abs() < 1e-6);
        // The frag grenade is the lowest one; the other is smoke.
        let grenades: Vec<(&str, u8)> = dictionary
            .items
            .iter()
            .filter(|item| params(item.dict_id).kind == "grenade")
            .map(|item| (item.name.as_str(), params(item.dict_id).item_level))
            .collect();
        assert_eq!(grenades, [("rgd-5", 3), ("rdg-m", 6)]);

        // The lowest tier's pocket gear is the cheapest that opens the two
        // more pockets needed: of every pair of pieces for two slots (or
        // one piece) that sell for silver.
        let pocket_gear: Vec<(u8, u16, u32, f32)> = dictionary
            .items
            .iter()
            .filter(|item| !item.is_premium)
            .filter_map(|item| {
                let slot = (0..7).find(|&slot| dictionary.fits(item.dict_id, slot))?;
                let pockets = default_modifier(item.dict_id, "pocket_slots_modifier");
                (pockets > 0.0).then_some((slot, item.dict_id, item.base_cost, pockets))
            })
            .collect();
        let mut cheapest = u32::MAX;
        for (i, a) in pocket_gear.iter().enumerate() {
            if a.3 >= 2.0 {
                cheapest = cheapest.min(a.2);
            }
            for b in &pocket_gear[i + 1..] {
                if a.0 != b.0 && a.3 + b.3 >= 2.0 {
                    cheapest = cheapest.min(a.2 + b.2);
                }
            }
        }
        let cost = |slot: u8| {
            dictionary
                .item(kit.slot(slot).unwrap().dict_id)
                .unwrap()
                .base_cost
        };
        assert_eq!(cost(3) + cost(4), cheapest);
        assert_eq!((cost(3), cost(4)), (6650, 6830));
    }

    /// The owner's SAFE SET (2026-10-26): weight, carry, movement speed and
    /// stamina upgrades only, each one a bot's piece takes, one at 4-5 and
    /// two at 9-10 and 13-15, in every kit and on the wire.
    #[test]
    fn the_bots_wear_only_the_safe_upgrades_their_pieces_take() {
        use super::super::match_msgs::static_info::MatchProfile;
        use super::super::modifications::{MAX_RANK, Modifications};
        use super::super::skills::MODIFIERS;

        let table = Modifications::shared();
        // Modification types: 7 weapon weight, 18 gear weight, 22 stamina
        // regeneration, 23 movement speed, 24 carry, 28 stamina spent.
        let safe = [7, 18, 22, 23, 24, 28];
        for (tier, role, slot, mods) in BOT_UPGRADES {
            let at = format!("{tier} {role} slot {slot}");
            assert!(!mods.is_empty() && mods.len() <= MAX_RANK, "{at}");
            for &id in mods {
                let row = table.modification(id).expect("a modification");
                assert!(safe.contains(&row.kind), "{at}: {id} is type {}", row.kind);
            }
        }
        for tier in BotTier::ALL {
            for role in BotRole::ALL {
                let upgrades: usize = BOT_UPGRADES
                    .iter()
                    .filter(|row| (row.0, row.1) == (tier, role))
                    .map(|row| row.3.len())
                    .sum();
                assert_eq!(
                    upgrades,
                    if tier == BotTier::Low { 1 } else { 2 },
                    "{tier} {role}"
                );
            }
        }
        let dictionary = Dictionary::shared();
        for (tier, role, k, kit) in every_kit() {
            let at = format!("{tier} {role} kit {k}");
            for item in &kit.items {
                let wanted = if item.slot <= 6 {
                    bot_upgrades_with(tier, role, item.slot, true)
                } else {
                    &[]
                };
                assert_eq!(item.mods, wanted, "{at}: slot {}", item.slot);
                // Each fits its piece beside what the piece holds.
                let mut held = Vec::new();
                for &id in item.mods {
                    assert!(
                        table.fits(item.dict_id, &held, id),
                        "{at}: {id} on {}",
                        dictionary.item(item.dict_id).unwrap().name
                    );
                    held.push(id);
                }
            }
            // On the wire: the profile's item carries them.
            let profile =
                MatchProfile::from_loadouts("Bot-1", 0, &[kit], [0.0; MODIFIERS], dictionary, 1);
            for item in &kit.items {
                let (index, _) = profile.kits[0].slots[usize::from(item.slot)].unwrap();
                let wire: Vec<(u32, bool)> = item.mods.iter().map(|&id| (id, false)).collect();
                assert_eq!(
                    profile.items[usize::from(index)].modifications,
                    wire,
                    "{at}"
                );
            }
        }
        // Off: the plain armour.
        for (tier, role, slot, _) in BOT_UPGRADES {
            assert!(bot_upgrades_with(tier, role, slot, false).is_empty());
        }
        // The reading back is the same with or without them.
        for tier in BotTier::ALL {
            for role in BotRole::ALL {
                let profile = MatchProfile::from_loadouts(
                    "Bot-1",
                    0,
                    &bot_kits(tier, role),
                    [0.0; MODIFIERS],
                    dictionary,
                    1,
                );
                assert_eq!(bot_of(&profile), Some((tier, role)));
            }
        }
    }

    #[test]
    fn each_role_has_its_weapon_and_armour() {
        let dictionary = Dictionary::shared();
        let params = |name: &str| {
            item_params::item_params(dictionary.item_named(name).unwrap().dict_id).unwrap()
        };
        let low = |role: BotRole| BotTier::Low.gear().of(role).weapon;
        // The top players' picks at levels 4-5 (pvb-tiers.md), but the
        // assault rifle, which is the lowest that sells for silver.
        assert_eq!(low(BotRole::Flanker), "vityaz");
        assert_eq!(low(BotRole::Support), "ak_74u");
        assert_eq!(low(BotRole::Rusher), "mp_153");
        assert_eq!(low(BotRole::Camper), "rem_700");
        assert_eq!(TIER_9_10.support.weapon, "akmn");
        // The kinds in every tier: an SMG or a compact rifle, an assault
        // rifle, a shotgun, the sniper category.
        let category = |name: &str| dictionary.item_named(name).unwrap().category;
        for tier in BotTier::ALL {
            let gear = tier.gear();
            assert!(matches!(category(gear.flanker.weapon), 8 | 15), "{tier}");
            assert_eq!(category(gear.support.weapon), 8, "{tier}");
            assert_eq!(category(gear.rusher.weapon), 14, "{tier}");
            assert_eq!(category(gear.camper.weapon), 13, "{tier}");
        }
        assert_eq!(params("vityaz").kind, "wpn_smg");
        let lowest = |category: u8| {
            dictionary
                .items
                .iter()
                .filter(|item| item.category == category && !item.is_premium)
                .map(|item| item_params::item_params(item.dict_id).unwrap().item_level)
                .min()
                .unwrap()
        };
        assert_eq!(params("ak_74u").item_level, lowest(8));
        // The shotgun of the higher tiers is the highest there is.
        let highest = dictionary
            .items
            .iter()
            .filter(|item| item.category == 14 && !item.is_premium)
            .map(|item| item_params::item_params(item.dict_id).unwrap().item_level)
            .max()
            .unwrap();
        assert_eq!(params(TIER_9_10.rusher.weapon).item_level, highest);
        // Light armour for the rusher and the flanker, heavier for the
        // support and the camper, in the lowest tier.
        let armour = |role: BotRole| -> f32 {
            BotTier::Low
                .gear()
                .of(role)
                .armour
                .iter()
                .map(|&(_, name)| params(name).weight)
                .sum()
        };
        for light in [BotRole::Rusher, BotRole::Flanker] {
            for heavy in [BotRole::Support, BotRole::Camper] {
                assert!(armour(light) < armour(heavy), "{light} {heavy}");
            }
        }
    }

    #[test]
    fn a_bots_tier_and_role_read_back_from_its_profile() {
        use super::super::match_msgs::static_info::MatchProfile;
        use super::super::skills::MODIFIERS;

        let dictionary = Dictionary::shared();
        for tier in BotTier::ALL {
            for role in BotRole::ALL {
                let profile = MatchProfile::from_loadouts(
                    "Bot-1",
                    0,
                    &bot_kits(tier, role),
                    [0.0; MODIFIERS],
                    dictionary,
                    1,
                );
                assert_eq!(bot_of(&profile), Some((tier, role)));
                assert_eq!(bot_role_of(&profile), Some(role));
                // The ammo's storage is the lives' worth.
                let (index, count) = profile.kits[0].slots[8].unwrap();
                assert_eq!(
                    profile.items[usize::from(index)].condition_or_stack,
                    u32::from(count) * BOT_AMMO_LIVES
                );
            }
        }
        // A bot of an older kit: the role by its weapon, the lowest tier.
        let older = Loadout::from_names(
            dictionary,
            [(7, "ak_74u"), (8, "ammo_5.45x39_7n6m"), (16, "medkit")],
        )
        .unwrap();
        let profile =
            MatchProfile::from_loadout("Bot-2", 0, &older, [0.0; MODIFIERS], dictionary, 1);
        assert_eq!(bot_of(&profile), Some((BotTier::Low, BotRole::Support)));
        // A player in the starter kit, or one with no kit, has no role.
        let player =
            MatchProfile::from_loadout("Alice", 0, starter(), [0.0; MODIFIERS], dictionary, 1);
        assert_eq!(bot_role_of(&player), None);
        let empty = MatchProfile {
            kits: Vec::new(),
            ..player
        };
        assert_eq!(bot_role_of(&empty), None);
        // The first kits differ, so the reading is one to one.
        let mut firsts: Vec<Vec<(u8, u16)>> = BotTier::ALL
            .into_iter()
            .flat_map(|tier| BotRole::ALL.into_iter().map(move |role| (tier, role)))
            .map(|(tier, role)| {
                bot_kit(tier, role)
                    .items
                    .iter()
                    .map(|item| (item.slot, item.dict_id))
                    .collect()
            })
            .collect();
        firsts.sort_unstable();
        firsts.dedup();
        assert_eq!(firsts.len(), 12);
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

    /// The lobby's roles for the bots of a match (moved here from the lobby
    /// with the function): each team's k-th bot takes `BotRole::nth(k)`,
    /// whatever the other team's.
    #[test]
    fn each_teams_bots_take_the_roles_by_the_shares() {
        use BotRole::*;
        assert_eq!(
            bot_roles(&[0, 1, 0, 1, 0, 1, 0, 1]),
            [Flanker, Flanker, Rusher, Rusher, Support, Support, Camper, Camper]
        );
        assert_eq!(bot_roles(&[1, 1, 1]), [Flanker, Rusher, Support]);
        // Teams one after the other, as the bench's, take the same by team.
        assert_eq!(
            bot_roles(&[0, 0, 0, 0, 1, 1, 1, 1]),
            [Flanker, Rusher, Support, Camper, Flanker, Rusher, Support, Camper]
        );
        assert!(bot_roles(&[]).is_empty());
    }

    /// A bot's profile is its role's kits of the tier, under its number's
    /// name and the BOT tag, on its team; and reads back as that tier and
    /// role.
    #[test]
    fn a_bots_profile_is_its_roles_kits_of_the_tier() {
        for tier in BotTier::ALL {
            for role in BotRole::ALL {
                let profile = bot_profile(7, 1, role, tier);
                assert_eq!(profile.name, bot_name(7));
                assert_eq!(profile.name, "Bot-7");
                assert_eq!(profile.clan_tag, "BOT");
                assert_eq!(profile.team, 1);
                assert_eq!(profile.kits.len(), 3, "{tier} {role}");
                assert_eq!(bot_of(&profile), Some((tier, role)));
            }
        }
    }
}

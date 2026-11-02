//! No tutorial (the owner's of 2026-10-19): it took a new player through
//! Play into a match, and with rooms nobody is sent into a match they did
//! not choose. The server tells every client that every step is done, so
//! the client runs none of it; a new account starts with more instead
//! ([`signup_grant`]). `docs/notes/lobby.md` §2.21.
//!
//! The client runs the tutorial itself from the state the server sends.
//!
//! `9D` is `bool show, u32 a, u32 b`. With `show` the client's tutorial
//! leaves its disabled state by `a` (0x283190, PROVEN):
//!
//! | `a` | state |
//! |---|---|
//! | bit 0 clear | `tutorial_state_choose_nickname` |
//! | bit 0 set, `b` == 0 | `tutorial_state_click_inventory` |
//! | bit 0 set, `b` != 0 | `tutorial_state_equip_drop` |
//! | bit 1 or bit 5 set (`a & 0x22`) | `default_state`: no tutorial |
//!
//! The other bits are the steps the client reported with `4B 01 <step>`:
//! **bit `n` is step `n`** (PROVEN for 2 and 3: the survival notes' reward
//! is shown while `a >> 3 & 1` (notes 0, "prepare to battle", step 3) or
//! `a >> 2 & 1` (notes 1, "items", step 2) is clear, 0x2833F1; for 4: the
//! match's own tutorial is skipped when `a & 0x10`, 0x27AE46; for 1:
//! `a >> 1 & 1` opens `state_choose_faction`, 0x28E9AD). Bit 0 is a
//! nickname chosen and bit 5 the tutorial skipped (`4B 02`), LIKELY.
//!
//! So srp sends [`ALL_DONE`]: `a & 0x22` puts the lobby in its
//! `default_state`, bit 4 skips the match's own tutorial, bits 2 and 3 hide
//! the notes' rewards. `9D` with `show` off stores the same flags and runs
//! no tutorial (0x27136E): the sign-in's `9D` has `show` on, so the client
//! leaves its disabled state for `default_state`, as after a finished
//! tutorial; the ranking's recounts send it off. A client that sends a
//! step anyway (`4B`, `46 0A`) is not answered.
//!
//! **What the steps paid**, the game's own
//! `gameplay/awards/unique_awards.options` ([`survarium::v026g::awards`]):
//! `tutorial_completed` 1000 silver and 250 experience, the two notes
//! 25 gold each, the match's tutorial 50 gold. **A new account starts**
//! with what the game data's table says ([`PATH`], [`NewAccount`]): its
//! money, premium, the rounds of every ammo ([`starter_ammo`]), its
//! reputation and quest faction, what it owns and what its three kits
//! wear; and the steps' experience (the owner's of 2026-10-19 and
//! 2026-10-24, `docs/notes/no-tutorial.md`). It is the account's state
//! when the client first hears of it: no window, no notice.

use std::sync::OnceLock;

use binary_config_parser::tree::ConfigValue;
use survarium::v026g::awards::{Awards, kind, name};
use survarium::v026g::dictionary::Dictionary;
use survarium::v026g::game_data;
use survarium::v026g::item_params::{self, shop_unit};
use survarium::v026g::loadouts::{self, Loadout, POCKET_SLOTS};
use survarium::v026g::lobby::requests::{INVENTORY_SLOT, Relocation};
use survarium::v026g::lobby::{self, FULL_CONDITION, ServerMessage};
use survarium::v026g::modifications::Modifications;

use super::account::{Account, KIT_SLOTS, KITS};

/// The bits of the tutorial's state (`a` of `9D`).
pub mod flag {
    pub const NICKNAME: u32 = 1 << 0;
    pub const COMPLETED: u32 = 1 << 1;
    pub const ITEM_NOTES: u32 = 1 << 2;
    pub const BATTLE_NOTES: u32 = 1 << 3;
    pub const GAME_WORLD: u32 = 1 << 4;
    pub const SKIPPED: u32 = 1 << 5;
}

/// Every step done: the client runs no tutorial (see the module).
pub const ALL_DONE: u32 = flag::NICKNAME
    | flag::COMPLETED
    | flag::ITEM_NOTES
    | flag::BATTLE_NOTES
    | flag::GAME_WORLD
    | flag::SKIPPED;

/// The awards a new account is given, the tutorial's steps'.
pub const AWARDS: [&str; 4] = [
    name::TUTORIAL_COMPLETED,
    name::ITEM_NOTES,
    name::BATTLE_NOTES,
    name::GAME_WORLD_TUTORIAL,
];

/// Where the ammo lives: every item of the dictionary under it is ammo.
pub const AMMO_CONFIGS: &str = "gameplay/items/weapons/ammo/";

/// What a new account is given, in the game's files: made by
/// `scripts/content-edit/new-account.tsv` (its header says each key).
pub const PATH: &str = game_data::NEW_ACCOUNT_PATH;

/// An item worn in a kit, `(slot, item)`: the item by its `items_dict` name.
pub type Worn = (u8, String);

/// What a new account is given ([`PATH`]): [`new_account`]. Its items are
/// names; that the game data has each, that it fits its slot and that the
/// pockets take what they are given is [`NewAccount::check`]'s, at start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewAccount {
    pub silver: u32,
    pub gold: u32,
    /// Premium from the account's making, added as every premium is
    /// (`premium::add_premium`).
    pub premium_days: u32,
    /// The rounds it owns of every ammo ([`starter_ammo`]).
    pub rounds: u32,
    /// The faction it takes quests from, at [`Self::reputation_level`].
    pub faction: u8,
    pub reputation_level: u8,
    /// What it owns besides the kits, `(item, units)`: units of a stack,
    /// else that many items. The kits' pockets take theirs from these
    /// stacks.
    pub own: Vec<(String, u32)>,
    /// What each kit wears: one item, worn by all of them.
    pub all_kits: Vec<Worn>,
    /// What each kit wears more, kit by kit: its weapon and its ammo.
    pub kits: [Vec<Worn>; KITS],
    /// The units of each pocket's item.
    pub pocket_units: u32,
    /// Modifications of their own worn items have, `(item, modification)`.
    pub modifications: Vec<(String, u32)>,
}

impl NewAccount {
    /// The grant of `config` ([`PATH`]); a key missing or unknown, or a
    /// value that is not of its key, is an error naming it.
    pub fn parse(config: &[u8]) -> Result<Self, String> {
        let root = ConfigValue::parse(config).map_err(|cause| format!("{PATH}: {}", cause.0))?;
        let ConfigValue::Named(entries) = &root else {
            return Err(format!("{PATH}: not a table of keys"));
        };
        let at = |key: &str| root.get(key).ok_or_else(|| format!("{PATH}: no `{key}`"));
        fn number<T: TryFrom<i32>>(key: &str, value: &ConfigValue) -> Result<T, String> {
            match value {
                ConfigValue::Integer(n) => {
                    T::try_from(*n).map_err(|_| format!("{PATH}: `{key}` is {n}, out of range"))
                }
                other => Err(format!("{PATH}: `{key}` is {other:?}, not a whole number")),
            }
        }
        fn table<'a>(
            key: &str,
            value: &'a ConfigValue,
        ) -> Result<&'a [(String, ConfigValue)], String> {
            match value {
                ConfigValue::Named(entries) => Ok(entries),
                other => Err(format!("{PATH}: `{key}` is {other:?}, not a table")),
            }
        }
        fn text(key: &str, value: &ConfigValue) -> Result<String, String> {
            match value {
                ConfigValue::String(text) => Ok(text.clone()),
                other => Err(format!("{PATH}: `{key}` is {other:?}, not an item's name")),
            }
        }
        let worn = |key: &str, value: &ConfigValue| -> Result<Vec<Worn>, String> {
            table(key, value)?
                .iter()
                .map(|(slot, item)| {
                    let at = format!("{key}/{slot}");
                    let slot = slot
                        .parse::<u8>()
                        .ok()
                        .filter(|&slot| usize::from(slot) < KIT_SLOTS)
                        .ok_or_else(|| format!("{PATH}: `{at}`: no slot of a kit"))?;
                    Ok((slot, text(&at, item)?))
                })
                .collect()
        };
        let by_item = |key: &str| -> Result<Vec<(String, u32)>, String> {
            table(key, at(key)?)?
                .iter()
                .map(|(item, value)| Ok((item.clone(), number(&format!("{key}/{item}"), value)?)))
                .collect()
        };
        let mut kits: [Vec<Worn>; KITS] = Default::default();
        for (key, value) in entries {
            let kit = key
                .strip_prefix("kit_")
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|n| (1..=KITS).contains(n));
            match (key.as_str(), kit) {
                (_, Some(n)) => kits[n - 1] = worn(key, value)?,
                (
                    "silver" | "gold" | "premium_days" | "rounds" | "faction" | "reputation_level"
                    | "own" | "all_kits" | "pocket_units" | "own_modifications",
                    None,
                ) => {}
                _ => return Err(format!("{PATH}: `{key}` is no key of it")),
            }
        }
        let num = |key: &str| at(key).and_then(|value| number::<u32>(key, value));
        let byte = |key: &str| at(key).and_then(|value| number::<u8>(key, value));
        Ok(Self {
            silver: num("silver")?,
            gold: num("gold")?,
            premium_days: num("premium_days")?,
            rounds: num("rounds")?,
            faction: byte("faction")?,
            reputation_level: byte("reputation_level")?,
            own: by_item("own")?,
            all_kits: worn("all_kits", at("all_kits")?)?,
            kits,
            pocket_units: num("pocket_units")?,
            modifications: by_item("own_modifications")?,
        })
    }

    /// What kit `kit` (0 on) wears: [`Self::all_kits`], then its own.
    pub fn kit(&self, kit: usize) -> impl Iterator<Item = (u8, &str)> {
        self.all_kits
            .iter()
            .chain(&self.kits[kit])
            .map(|(slot, name)| (*slot, name.as_str()))
    }

    /// What of the grant `dictionary` and `modifications` lack or do not
    /// take, each in a line: an item, a slot it does not fit, a pocket the
    /// kit does not open or that does not take its units, a modification,
    /// the faction's level. srp does not start with any
    /// (`known::check_code`).
    pub fn check(&self, dictionary: &Dictionary, modifications: &Modifications) -> Vec<String> {
        let mut wrong: Vec<String> = Vec::new();
        let worn = (0..KITS).flat_map(|kit| self.kit(kit).map(|(_, name)| name));
        let mut names: Vec<&str> = self.own.iter().map(|(name, _)| name.as_str()).collect();
        names.extend(worn);
        names.extend(self.modifications.iter().map(|(name, _)| name.as_str()));
        for name in names {
            let line = format!("no item `{name}`");
            if dictionary.item_named(name).is_none() && !wrong.contains(&line) {
                wrong.push(line);
            }
        }
        if !wrong.is_empty() {
            return wrong;
        }
        for kit in 0..KITS {
            let loadout = match Loadout::from_names(dictionary, self.kit(kit)) {
                Ok(loadout) => loadout,
                Err(why) => {
                    wrong.push(format!("kit {}: {}", kit + 1, why.0));
                    continue;
                }
            };
            let (count, size) = loadouts::pockets(&loadout);
            for item in loadout
                .items
                .iter()
                .filter(|item| POCKET_SLOTS.contains(&item.slot))
            {
                let open = POCKET_SLOTS[..count as usize].contains(&item.slot);
                let whole = item_params::item_params(item.dict_id)
                    .is_some_and(|params| params.fills_whole_pocket);
                let takes = if whole { 1 } else { size };
                if !open || self.pocket_units > takes {
                    wrong.push(format!(
                        "kit {}: slot {}: the gear opens {count} pockets of {size} units, \
                         {} of `{}` asked",
                        kit + 1,
                        item.slot,
                        self.pocket_units,
                        dictionary
                            .item(item.dict_id)
                            .map_or("?", |item| item.name.as_str())
                    ));
                }
            }
        }
        for (name, id) in &self.modifications {
            if modifications.modification(*id).is_none() {
                wrong.push(format!("no modification {id}"));
            }
            if !(0..KITS).any(|kit| self.kit(kit).any(|(_, worn)| worn == name)) {
                wrong.push(format!("modification {id} of `{name}`, which no kit wears"));
            }
        }
        let level = dictionary
            .factions
            .iter()
            .find(|faction| faction.id == self.faction)
            .and_then(|faction| faction.levels.get(usize::from(self.reputation_level)));
        if level.is_none() {
            wrong.push(format!(
                "no faction {} with a reputation level {}",
                self.faction, self.reputation_level
            ));
        }
        wrong
    }
}

/// The grant of the game's files ([`game_data::shared`]), read when first
/// used; `srp` reads it at start (`known::check_code`).
pub fn new_account() -> &'static NewAccount {
    static GRANT: OnceLock<NewAccount> = OnceLock::new();
    GRANT.get_or_init(|| {
        NewAccount::parse(game_data::file(PATH)).unwrap_or_else(|why| panic!("{why}"))
    })
}

/// Every ammo a player can own: the dictionary's stacks under
/// [`AMMO_CONFIGS`], ported weapons' too, by dict id.
pub fn starter_ammo(dictionary: &Dictionary) -> Vec<u16> {
    dictionary
        .items
        .iter()
        .filter(|item| item.is_stack && item.cfg_name.starts_with(AMMO_CONFIGS))
        .map(|item| item.dict_id)
        .collect()
}

/// The ammo bags of a kit whose worn items have `modifications` (their
/// defaults and their own): the player's (`default.player`
/// `ammo_bags_count`, 3) and what the modifications add
/// (`parameters/ammo_bags_modifier`, 1050 and 1051), the lobby's
/// `PlayerInfo.ammoBags` for an account of no skills.
pub fn ammo_bags(modifications: impl IntoIterator<Item = u32>) -> u32 {
    let table = Modifications::shared();
    let added: f32 = modifications
        .into_iter()
        .filter_map(|id| table.modification(id))
        .flat_map(|row| &row.modifiers)
        .filter(|modifier| modifier.path == "parameters/ammo_bags_modifier")
        .map(|modifier| modifier.value)
        .sum();
    (item_params::player().ammo_bags_count as f32 + added).max(0.0) as u32
}

/// The most rounds of `ammo` an ammo slot takes with `bags` ammo bags: what
/// the ammo window fills it to, `int(bags × clips a bag) × clip_size`
/// (`AmmoShareElement`, clips a bag `1 / required_bags`, `ItemFactory`).
pub fn full_rounds(ammo: u16, bags: u32) -> u32 {
    let required = item_params::item_params(ammo).map_or(1, |params| params.required_bags.max(1));
    bags / required * shop_unit(ammo)
}

/// [`ammo_bags`] of kit `kit` (0 on) of the grant: its worn items'
/// default modifications and their own ([`NewAccount::modifications`]).
pub fn starter_ammo_bags(dictionary: &Dictionary, kit: usize) -> u32 {
    let grant = new_account();
    let worn: Vec<&str> = grant.kit(kit).map(|(_, name)| name).collect();
    let defaults = worn
        .iter()
        .filter_map(|name| dictionary.item_named(name))
        .flat_map(|item| {
            item_params::item_params(item.dict_id)
                .map_or(&[][..], |params| params.default_modifications)
                .iter()
                .copied()
        });
    let own = grant
        .modifications
        .iter()
        .filter(|(name, _)| worn.contains(&name.as_str()))
        .map(|&(_, id)| id);
    ammo_bags(defaults.chain(own))
}

/// The experience the tutorial's steps paid, by the game's file.
pub fn experience() -> u32 {
    AWARDS
        .iter()
        .flat_map(|award| Awards::shared().get(award))
        .filter(|row| row.kind == kind::EXPERIENCE)
        .map(|row| row.amount)
        .sum()
}

/// A new account's start, before its client first hears of it
/// ([`new_account`]): its money, the steps' experience, the rounds of every
/// ammo, the faction's reputation level and its quests (the tutorial ended
/// in the faction's choice; the owner's of 2026-10-19: never neutral), what
/// it owns and what its three kits wear ([`wear_starter_kit`]), and its
/// premium from `now` (Unix seconds).
pub fn signup_grant(account: &mut Account, now: u64) {
    signup_grant_with(account, true, now);
}

/// [`signup_grant`], what it owns, the kits and the premium given only with
/// `starter_kit` (off only in the lobby's tests, `GameAccounts::set_starter_kit`).
pub fn signup_grant_with(account: &mut Account, starter_kit: bool, now: u64) {
    let grant = new_account();
    let dictionary = Dictionary::shared();
    // The reputation first: the quests are given by its level.
    let points = dictionary
        .factions
        .iter()
        .find(|faction| faction.id == grant.faction)
        .and_then(|faction| faction.levels.get(usize::from(grant.reputation_level)))
        .expect("the faction's level is there (known::check_code)")
        .value;
    account.add_reputation(
        grant.faction,
        points.saturating_sub(account.reputation(grant.faction)),
    );
    super::quests::choose_faction(account, grant.faction)
        .expect("every faction is open to a new account");
    account.generic_money = grant.silver;
    account.premium_money = grant.gold;
    if starter_kit {
        super::premium::add_premium(account, u64::from(grant.premium_days) * 86_400, now);
    }
    // Short of a level: nothing more to pay.
    account.add_experience(experience());
    for dict_id in starter_ammo(dictionary) {
        let owned = account
            .inventory
            .iter()
            .find(|item| item.dict_id == dict_id)
            .map_or(0, |item| item.condition_or_stack);
        if owned < grant.rounds {
            account.add_to_stack(dict_id, grant.rounds - owned);
        }
    }
    if starter_kit {
        wear_starter_kit(account, dictionary);
    }
    account.extras.tutorial_flags = ALL_DONE;
    account.tutorial_done = true;
}

/// What the grant owns ([`NewAccount::own`]), then its kits worn by the
/// lobby's own rule of a kit put on (`rules::fill_kit`). Each item worn is
/// bought once and the kits that wear it name that one, as a player's own
/// kits share the inventory; a stack worn is the one owned: an ammo slot
/// filled to the most the kit's bags take ([`full_rounds`]), a pocket with
/// [`NewAccount::pocket_units`]. An item the game data lacks is left out:
/// srp starts only on data with all of them (`known::check_code`); the test
/// data predates the knit cap and the Novice's backpack.
pub fn wear_starter_kit(account: &mut Account, dictionary: &Dictionary) {
    let grant = new_account();
    for (name, units) in &grant.own {
        let Some(item) = dictionary.item_named(name) else {
            continue;
        };
        if item.is_stack {
            let owned = account
                .inventory
                .iter()
                .find(|owned| owned.dict_id == item.dict_id)
                .map_or(0, |owned| owned.condition_or_stack);
            account.add_to_stack(item.dict_id, units.saturating_sub(owned));
        } else {
            for _ in 0..*units {
                account.add_item(item.dict_id, FULL_CONDITION);
            }
        }
    }
    // The items bought for the kits, `(dict id, item id)`.
    let mut bought: Vec<(u16, u32)> = Vec::new();
    for kit in 0..KITS {
        let bags = starter_ammo_bags(dictionary, kit);
        let mut moves: Vec<Relocation> = Vec::new();
        for (slot, name) in grant.kit(kit) {
            let Some(item) = dictionary.item_named(name) else {
                log::warn!("starter kit: the game data has no {name}; left out");
                continue;
            };
            let (item_id, count) = if item.is_stack {
                let count = if POCKET_SLOTS.contains(&slot) {
                    grant.pocket_units
                } else {
                    full_rounds(item.dict_id, bags)
                };
                (account.add_to_stack(item.dict_id, 0), count)
            } else if let Some(&(_, item_id)) = bought.iter().find(|(id, _)| *id == item.dict_id) {
                (item_id, 1)
            } else {
                let item_id = account.add_item(item.dict_id, FULL_CONDITION);
                bought.push((item.dict_id, item_id));
                (item_id, 1)
            };
            moves.push(Relocation {
                item_id,
                dict_id: item.dict_id,
                source_slot: INVENTORY_SLOT,
                target_slot: slot,
                count,
                count_in_inventory: 0,
            });
        }
        if let Err(error) = super::rules::fill_kit(account, kit, &moves) {
            log::error!("starter kit: kit {} could not wear it ({error})", kit + 1);
        }
    }
    for (name, modification) in &grant.modifications {
        let Some(dict_id) = dictionary.item_named(name).map(|item| item.dict_id) else {
            continue;
        };
        let worn = bought.iter().find(|(id, _)| *id == dict_id);
        if let Some(item) = worn.and_then(|&(_, item_id)| account.item_mut(item_id)) {
            item.modifications.push((*modification, false));
        }
    }
    account.sync_kits();
}

/// `9D`: every step done (see the module). `show` on at sign-in, off for
/// the ranking's recounts.
pub fn message(show: bool) -> ServerMessage {
    lobby::tutorial(show, ALL_DONE, 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use survarium::v026g::lobby::client_model::{Client, Decoded};
    use survarium::v026g::lobby::tutorial_model::State;

    #[test]
    fn the_client_runs_no_tutorial() {
        let message = message(true);
        assert_eq!(message.id_byte(), 0x9D);
        assert_eq!(message.body(), [1, 0x3F, 0, 0, 0, 1, 0, 0, 0]);
        let mut client = Client::signed_in();
        assert!(matches!(
            client.receive(&message).unwrap(),
            Decoded::Tutorial {
                show: true,
                a: 0x3F,
                b: 1
            }
        ));
        assert_eq!(client.tutorial().state(), State::Default);
        assert!(!client.tutorial().is_modal());
        assert_eq!(super::message(false).body()[0], 0);
    }

    /// The table as the game data has it (`new-account.tsv`): the owner's
    /// numbers; the Novice's backpack on the back, three pockets of three.
    #[test]
    fn the_grant_is_the_tables() {
        let grant = new_account();
        assert_eq!(
            (grant.silver, grant.gold, grant.premium_days),
            (15_000, 150, 30)
        );
        assert_eq!(
            (grant.rounds, grant.faction, grant.reputation_level),
            (1000, 1, 5)
        );
        // A month of premium, the shop's (`PREMIUM_OFFERS` id 3).
        let month = (3, grant.premium_days);
        assert!(survarium::v026g::shop_offers::PREMIUM_OFFERS.contains(&month));
        let at = |slot: u8| {
            grant
                .all_kits
                .iter()
                .find(|(at, _)| *at == slot)
                .map(|(_, name)| name.as_str())
        };
        assert_eq!(at(3), Some("stk2_novice_026_back"));
        let pockets: Vec<(u8, &str)> = POCKET_SLOTS
            .iter()
            .filter_map(|&slot| at(slot).map(|name| (slot, name)))
            .collect();
        assert_eq!(
            pockets,
            [(13, "medkit"), (14, "bandages"), (15, "base_trap")]
        );
        assert_eq!(grant.pocket_units, 3);
        let own: Vec<(&str, u32)> = grant
            .own
            .iter()
            .map(|(name, units)| (name.as_str(), *units))
            .collect();
        assert_eq!(
            own,
            [
                ("tt_33", 1),
                ("mosin_nagant", 1),
                ("ppsh", 1),
                ("medkit", 50),
                ("antitoxin", 50),
                ("bandages", 20),
                ("base_trap", 20)
            ]
        );
    }

    /// A key missing, one it has not, a slot no kit has: refused, named.
    #[test]
    fn a_table_of_other_keys_is_refused_naming_them() {
        let table = |entries: Vec<(&str, ConfigValue)>| {
            let entries = entries
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect();
            NewAccount::parse(&ConfigValue::Named(entries).to_bytes().unwrap())
        };
        assert_eq!(
            table(vec![("silver", ConfigValue::Integer(1))]),
            Err(format!("{PATH}: no `gold`"))
        );
        assert_eq!(
            table(vec![("kit_4", ConfigValue::Named(Vec::new()))]),
            Err(format!("{PATH}: `kit_4` is no key of it"))
        );
        let worn = ConfigValue::Named(vec![("23".into(), ConfigValue::String("ppsh".into()))]);
        assert_eq!(
            table(vec![("kit_1", worn)]),
            Err(format!("{PATH}: `kit_1/23`: no slot of a kit"))
        );
    }

    /// Pockets the gear does not open, or that take fewer units than the
    /// table gives, are named: without the Novice's backpack (the test
    /// data's), the gear opens two pockets of two.
    #[test]
    fn pockets_the_gear_does_not_give_are_named() {
        let mut grant = new_account().clone();
        grant.all_kits.retain(|(slot, _)| *slot != 0);
        grant
            .all_kits
            .iter_mut()
            .find(|(slot, _)| *slot == 3)
            .unwrap()
            .1 = "scav_oxygen_1".into();
        let table = Modifications::shared();
        let wrong = grant.check(Dictionary::shared(), table);
        assert_eq!(wrong.len(), 3 * 3, "{wrong:?}");
        assert!(wrong.contains(
            &"kit 1: slot 15: the gear opens 2 pockets of 2 units, 3 of `base_trap` asked".into()
        ));
        grant.pocket_units = 2;
        grant.all_kits.retain(|(slot, _)| *slot != 15);
        assert_eq!(
            grant.check(Dictionary::shared(), table),
            Vec::<String>::new()
        );
    }

    /// A fresh account on the test data, made as `GameAccounts::sign_up`
    /// makes it: everything, but the knit cap and the Novice's backpack of
    /// patches the test data predates (srp refuses such data,
    /// `known::check_code`). The whole kit by its dict ids on released
    /// data: `tests/starter_kit.rs`.
    #[test]
    fn a_new_account_starts_with_the_starter_kit() {
        use super::super::account::KIT_SLOTS;
        use survarium::v026g::item_params::item_params;

        let grant = new_account();
        assert_eq!(experience(), 250);
        const NOW: u64 = 1_800_000_000;
        let mut account = Account::with_loadout("Alice", &Loadout { items: Vec::new() });
        signup_grant(&mut account, NOW);
        assert_eq!(
            (account.generic_money, account.premium_money),
            (grant.silver, grant.gold)
        );
        // 30 days of premium from the making, 720 hours as the lobby is
        // told.
        assert_eq!(account.premium_until, NOW + 30 * 86_400);
        assert!(account.is_premium(NOW) && account.is_premium(NOW + 30 * 86_400 - 1));
        assert!(!account.is_premium(NOW + 30 * 86_400));
        assert_eq!(account.premium_hours(NOW), Some(720));
        assert_eq!(account.exp_total, 250);
        assert_eq!(account.level(), 0);
        assert_eq!(account.quest_faction, grant.faction);
        assert_eq!(account.extras.tutorial_flags, ALL_DONE);

        // The Scavengers at 3000 points: level 5, their highest.
        let dictionary = Dictionary::shared();
        assert_eq!(account.reputation(grant.faction), 3000);
        assert_eq!(account.reputation_level(grant.faction), 5);
        let scavengers = dictionary.factions.iter().find(|f| f.id == 1).unwrap();
        assert_eq!(scavengers.levels.len(), 6);
        for faction in [2, 3, 4] {
            assert_eq!(account.reputation(faction), 0);
        }

        // Exactly the table's rounds of every ammo, one stack each.
        let ammo = starter_ammo(dictionary);
        for name in [
            "ammo_7.62x25",
            "ammo_7.62x54r_old",
            "ammo_7.62x39",
            "ammo_9x18_makarov",
        ] {
            let dict_id = dictionary.item_named(name).unwrap().dict_id;
            assert!(ammo.contains(&dict_id), "{name}");
        }
        let stacks = |dict_id: u16| -> Vec<u32> {
            account
                .inventory
                .iter()
                .filter(|item| item.dict_id == dict_id)
                .map(|item| item.condition_or_stack)
                .collect()
        };
        for &dict_id in &ammo {
            assert_eq!(item_params(dict_id).unwrap().kind, "ammo");
            assert_eq!(stacks(dict_id), [grant.rounds], "{dict_id}");
        }
        // What it owns: the three free weapons, unworn, and the stacks the
        // pockets take theirs from (the inventory shows them whole).
        for (name, units) in &grant.own {
            let item = dictionary.item_named(name).unwrap();
            let owned = stacks(item.dict_id);
            if item.is_stack {
                assert_eq!(owned, [*units], "{name}");
            } else {
                assert_eq!(owned, vec![FULL_CONDITION; *units as usize], "{name}");
                let worn = account
                    .inventory
                    .iter()
                    .filter(|it| it.dict_id == item.dict_id);
                for it in worn {
                    assert!(
                        account
                            .kits
                            .iter()
                            .all(|kit| kit.slot_of(it.item_id).is_none())
                    );
                }
            }
        }

        // Each ammo slot full: the gear has no ammo bag modification, so
        // the player's three bags, three clips of one bag each.
        let bags = starter_ammo_bags(dictionary, 0);
        assert_eq!(bags, 3);
        let rounds = |ammo: &str| full_rounds(dictionary.item_named(ammo).unwrap().dict_id, bags);
        assert_eq!(rounds("ammo_9x18_makarov"), 150);
        assert_eq!(rounds("ammo_7.62x51"), 30);
        assert_eq!(rounds("ammo_12mm_buck"), 45);
        // A bag modification counts: 1051 adds two.
        assert_eq!(ammo_bags([1030, 1051]), 5);
        let mut missing = Vec::new();
        let mut levels = Vec::new();
        for (index, kit) in account.kits.iter().enumerate() {
            for (slot, name) in grant.kit(index) {
                let held = kit.slots[usize::from(slot)];
                let Some(item) = dictionary.item_named(name) else {
                    assert_eq!(held, None, "{name}");
                    missing.push(name);
                    continue;
                };
                let count = match (item.is_stack, POCKET_SLOTS.contains(&slot)) {
                    (true, true) => grant.pocket_units,
                    (true, false) => rounds(name),
                    (false, _) => 1,
                };
                let held = held.unwrap_or_else(|| panic!("kit {index}: {name} not worn"));
                let owned = account.item(held.item_id).unwrap();
                assert_eq!((owned.dict_id, held.count), (item.dict_id, count), "{name}");
                if !item.is_stack {
                    // One item of a kind: every kit wears the same one.
                    let of_kind = account
                        .inventory
                        .iter()
                        .filter(|o| o.dict_id == item.dict_id);
                    assert_eq!(of_kind.count(), 1, "{name}");
                    assert_eq!(owned.condition_or_stack, FULL_CONDITION, "{name}");
                    let own: Vec<(u32, bool)> = grant
                        .modifications
                        .iter()
                        .filter(|(piece, _)| piece == name)
                        .map(|&(_, modification)| (modification, false))
                        .collect();
                    assert_eq!(owned.modifications, own, "{name}");
                }
            }
            let worn: Vec<u8> = (0..KIT_SLOTS as u8)
                .filter(|&slot| kit.slots[usize::from(slot)].is_some())
                .collect();
            // One weapon, in weapon slot 1; weapon slot 2 empty; three
            // pockets.
            assert_eq!(worn, [1, 2, 4, 5, 6, 7, 8, 13, 14, 15], "kit {index}");
            assert_eq!(super::super::rules::check_play(&account, kit.id, 0), Ok(()));
            let weight = account.kit_weight(kit);
            assert!(weight <= 30.0, "kit {index}: {weight}");
            assert!(account.doubled_pockets(kit).is_empty());
            levels.push(super::super::matchmaking::kit_level(&account, kit));
        }
        assert_eq!(
            missing,
            ["novice_026_helmet", "stk2_novice_026_back"].repeat(3)
        );
        assert_eq!(levels, [3, 3, 4]);
        assert_eq!(account.active_kit, account.kits[0].id);

        // The two modifications: the game's own rows.
        let table = Modifications::shared();
        let isolation = table.modification(1183).unwrap();
        assert_eq!(isolation.name, "st_modify_anom_filters");
        assert_eq!(isolation.lobby_info, [("mask_ano_armor".to_owned(), 0.2)]);
        assert!(
            isolation
                .modifiers
                .iter()
                .all(|m| m.value == 0.2 && m.kind == 1)
        );
        let respirator = dictionary.item_named("scav_resp_1").unwrap().dict_id;
        assert!(table.fits(respirator, &[], 1183));
        let collection = table.modification(1133).unwrap();
        assert_eq!(collection.name, "st_modify_artefact_search");
        assert_eq!(
            collection.lobby_info,
            [("artefact_search_time_modifier".to_owned(), -0.2)]
        );
        // The gloves' own default of that type: 1130, -5 %.
        let gloves = dictionary.item_named("scav_gloves_2").unwrap().dict_id;
        assert_eq!(item_params(gloves).unwrap().default_modifications, [1130]);
    }
}

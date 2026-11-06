//! What a new account is given at its first sign-in, the game data's
//! table ([`PATH`], made by `scripts/content-edit/new-account.tsv`; its
//! header says each key): [`NewAccount`], which `session::v026g::tutorial`
//! gives (`docs/notes/no-tutorial.md`). [`super::tables::check`] parses it
//! and [`super::game_data::init`] checks it against the data
//! ([`NewAccount::check`]), so that `srp` and `srp-check-game-data` refuse
//! data without it, or with a grant it cannot give.

use std::sync::OnceLock;

use binary_config_parser::tree::ConfigValue;

use super::dictionary::Dictionary;
use super::game_data;
use super::loadouts::{self, Loadout, POCKET_SLOTS};
use super::lobby::limits::{KIT_SLOTS, KITS};
use super::modifications::{self, Modifications};
use super::quests::FACTIONS;
use super::skills::SkillsTree;

/// Its path in the game's files.
pub const PATH: &str = "gameplay/srp/new_account.options";

/// An item worn in a kit, `(slot, item)`: the item by its `items_dict` name.
pub type Worn = (u8, String);

/// A faction whose quests the lobby shows ([`FACTIONS`]): the one a new
/// account takes quests from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuestFaction(u8);

impl QuestFaction {
    pub fn id(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for QuestFaction {
    type Error = u8;

    fn try_from(id: u8) -> Result<Self, u8> {
        if FACTIONS.contains(&id) {
            Ok(Self(id))
        } else {
            Err(id)
        }
    }
}

/// What a new account is given ([`PATH`]): [`shared`]. Its items are
/// names; that the game data has each, that it fits its slot and that the
/// pockets take what they are given is [`Self::check`]'s, at start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewAccount {
    /// The level it starts at, one point short of the next.
    pub level: u8,
    /// `(skill id, points)`, each with points; the tree allows them at
    /// [`Self::level`] ([`Self::check`]).
    pub skills: Vec<(u16, u8)>,
    pub silver: u32,
    pub gold: u32,
    /// Premium from the account's making, added as every premium is.
    pub premium_days: u32,
    /// The rounds it owns of every ammo.
    pub rounds: u32,
    /// The faction it takes quests from, at [`Self::reputation_level`].
    pub faction: QuestFaction,
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
    /// The grant of `config` ([`PATH`]); a key missing or unknown, a value
    /// that is not of its key, or a faction whose quests the lobby does not
    /// show, is an error naming it.
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
                    "level" | "skills" | "silver" | "gold" | "premium_days" | "rounds" | "faction"
                    | "reputation_level" | "own" | "all_kits" | "pocket_units"
                    | "own_modifications",
                    None,
                ) => {}
                _ => return Err(format!("{PATH}: `{key}` is no key of it")),
            }
        }
        let num = |key: &str| at(key).and_then(|value| number::<u32>(key, value));
        let byte = |key: &str| at(key).and_then(|value| number::<u8>(key, value));
        let skills = table("skills", at("skills")?)?
            .iter()
            .map(|(id, points)| {
                let at = format!("skills/{id}");
                let id = id
                    .parse::<u16>()
                    .map_err(|_| format!("{PATH}: `{at}`: not a skill's id"))?;
                match number::<u8>(&at, points)? {
                    0 => Err(format!("{PATH}: `{at}` is 0: leave it out")),
                    points => Ok((id, points)),
                }
            })
            .collect::<Result<_, String>>()?;
        Ok(Self {
            level: byte("level")?,
            skills,
            silver: num("silver")?,
            gold: num("gold")?,
            premium_days: num("premium_days")?,
            rounds: num("rounds")?,
            faction: QuestFaction::try_from(byte("faction")?).map_err(|id| {
                format!(
                    "{PATH}: `faction` is {id}, not one whose quests the lobby shows ({FACTIONS:?})"
                )
            })?,
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

    /// What of the grant `dictionary`, `modifications` and `skills` lack or
    /// do not take, each in a line: skills the tree does not allow at the
    /// level; an item; a slot it does not fit; a pocket the kit does not
    /// open, that takes fewer units, or whose item is not owned with as
    /// many; a modification, of a type its item does not take, on a stack
    /// or on an item no kit wears; the faction's level.
    pub fn check(
        &self,
        dictionary: &Dictionary,
        modifications: &Modifications,
        skills: &SkillsTree,
    ) -> Vec<String> {
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
                let name = dictionary
                    .item(item.dict_id)
                    .map_or("?", |item| item.name.as_str());
                let open = POCKET_SLOTS[..count as usize].contains(&item.slot);
                let takes = loadouts::pocket_share(item.dict_id).units_in(size);
                if !open || self.pocket_units > takes {
                    wrong.push(format!(
                        "kit {}: slot {}: the gear opens {count} pockets of {size} units, \
                         {} of `{name}` asked",
                        kit + 1,
                        item.slot,
                        self.pocket_units,
                    ));
                }
                let owned = self
                    .own
                    .iter()
                    .find(|(own, _)| own == name)
                    .map_or(0, |&(_, units)| units);
                if owned < self.pocket_units {
                    wrong.push(format!(
                        "kit {}: slot {}: {} of `{name}` asked, {owned} owned (`own/{name}`)",
                        kit + 1,
                        item.slot,
                        self.pocket_units
                    ));
                }
            }
        }
        for (name, id) in &self.modifications {
            let item = dictionary.item_named(name).expect("checked above");
            // Of a type the item takes; beside its defaults too (the
            // gloves' 1133 with their own 1130: the owner's choice).
            let takes = modifications.modification(*id).is_some_and(|row| {
                modifications::item_mods(item.dict_id)
                    .is_some_and(|mods| mods.modifications & 1 << row.kind != 0)
            });
            if modifications.modification(*id).is_none() {
                wrong.push(format!("no modification {id}"));
            } else if !takes || item.is_stack {
                wrong.push(format!("modification {id} is none `{name}` takes"));
            }
            if !(0..KITS).any(|kit| self.kit(kit).any(|(_, worn)| worn == name)) {
                wrong.push(format!("modification {id} of `{name}`, which no kit wears"));
            }
        }
        let level = dictionary
            .factions
            .iter()
            .find(|faction| faction.id == self.faction.id())
            .and_then(|faction| faction.levels.get(usize::from(self.reputation_level)));
        if level.is_none() {
            wrong.push(format!(
                "no faction {} with a reputation level {}",
                self.faction.id(),
                self.reputation_level
            ));
        }
        if let Err(why) = skills.validate(self.level, &self.skills) {
            wrong.push(format!("level {}: {why}", self.level));
        }
        wrong
    }
}

/// The grant of the game's files ([`game_data::shared`]), read when first
/// used; `srp` reads and checks it at start ([`check_data`]).
pub fn shared() -> &'static NewAccount {
    static GRANT: OnceLock<NewAccount> = OnceLock::new();
    GRANT.get_or_init(|| {
        NewAccount::parse(game_data::file(PATH)).unwrap_or_else(|why| panic!("{why}"))
    })
}

/// [`NewAccount::check`] of the grant of this process's data, as
/// [`super::game_data::init`] runs it; an error names each problem. Not in
/// [`super::tables::check`]: the test data predates the knit cap and the
/// Novice's backpack the grant names.
pub fn check_data(data: &game_data::GameData) -> Result<(), game_data::GameDataError> {
    let wrong = shared().check(
        Dictionary::shared(),
        Modifications::shared(),
        SkillsTree::shared(),
    );
    if wrong.is_empty() {
        return Ok(());
    }
    Err(game_data::GameDataError(format!(
        "{}: {PATH}: {}. Build the data with every patch released (srp-game-data build), or \
         change scripts/content-edit/new-account.tsv",
        data.origin,
        wrong.join(", ")
    )))
}

#[cfg(test)]
mod tests;

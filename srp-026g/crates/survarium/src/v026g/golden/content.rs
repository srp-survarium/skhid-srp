//! Premium equipment and golden weapons declared by their content pack (`srp_premium`), without a
//! server item table.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use binary_config_parser::tree::ConfigValue;

use super::{Dictionary, GameData, GameDataError, SILVER_BONUS};
use crate::v026g::scopes::WEAPON_CATEGORIES;
use crate::v026g::{game_data, modifications};

/// The armour categories (torso to back).
const ARMOUR_CATEGORIES: std::ops::RangeInclusive<u8> = 1..=7;

/// What a content pack's gold-bought copy is.
pub(super) enum Kind {
    /// A premium armour piece: its premium bonuses, the piece's own first modifications.
    Armour(Vec<u32>),
    /// A golden weapon: its one bonus is the silver bonus ([`SILVER_BONUS`],
    /// `docs/notes/golden_weapons.md`), its own first modification.
    Weapon,
}

/// A gold-bought copy of a plain item: its plain item's name and what it is.
pub(super) struct PremiumCopy {
    pub plain_name: String,
    pub kind: Kind,
}

impl PremiumCopy {
    /// The bonuses srp gives a new one as its own first modifications.
    pub fn bonuses(&self) -> &[u32] {
        match &self.kind {
            Kind::Armour(bonuses) => bonuses,
            Kind::Weapon => &[SILVER_BONUS],
        }
    }
}

pub(super) fn shared() -> &'static BTreeMap<String, PremiumCopy> {
    static CONTENT: OnceLock<BTreeMap<String, PremiumCopy>> = OnceLock::new();
    CONTENT.get_or_init(|| {
        load(game_data::shared(), Dictionary::shared())
            .expect("srp checks the premium equipment metadata at start (golden::check_content)")
    })
}

fn integer(value: Option<&ConfigValue>, field: &str) -> Result<u32, GameDataError> {
    match value {
        Some(ConfigValue::Integer(value)) => u32::try_from(*value)
            .map_err(|_| GameDataError(format!("srp_premium.{field}: negative integer"))),
        _ => Err(GameDataError(format!(
            "srp_premium.{field}: expected an integer"
        ))),
    }
}

pub(super) fn load(
    data: &GameData,
    dictionary: &Dictionary,
) -> Result<BTreeMap<String, PremiumCopy>, GameDataError> {
    let mut result = BTreeMap::new();
    let mut available_bonuses = None;
    for item in &dictionary.items {
        let weapon = WEAPON_CATEGORIES.contains(&item.category);
        if !weapon && !ARMOUR_CATEGORIES.contains(&item.category) {
            continue;
        }
        let config = game_data::config(data, &item.cfg_name)?
            .ok_or_else(|| GameDataError(format!("{}: missing config", item.name)))?;
        let Some(meta) = config.get("srp_premium") else {
            continue;
        };
        let fail = |reason| GameDataError(format!("{}: srp_premium: {reason}", item.cfg_name));
        let plain_id = u16::try_from(integer(meta.get("plain_item"), "plain_item")?)
            .map_err(|_| fail("plain_item exceeds u16"))?;
        let plain = dictionary
            .item(plain_id)
            .ok_or_else(|| fail("plain item does not exist"))?;
        if !item.is_premium
            || item.is_stack
            || plain.dict_id == item.dict_id
            || plain.category != item.category
        {
            return Err(fail(
                "requires a premium armour piece or weapon and a distinct plain piece of the same category",
            ));
        }
        let plain_config = game_data::config(data, &plain.cfg_name)?
            .ok_or_else(|| fail("plain config is missing"))?;
        if plain_config.get("srp_premium").is_some()
            || super::PREMIUM_ARMOUR.iter().any(|r| r.0 == plain.name)
            || super::GOLDEN_WEAPONS.iter().any(|r| r.0 == plain.name)
        {
            return Err(fail("plain item is itself a premium copy"));
        }
        let Some(ConfigValue::Indexed(values)) = meta.get("bonuses") else {
            return Err(fail("bonuses must be an indexed table"));
        };
        if values.is_empty() || values.len() > modifications::MAX_RANK {
            return Err(fail("bonuses must fit the item's modification slots"));
        }
        let bonuses: Vec<u32> = values
            .iter()
            .map(|v| integer(Some(v), "bonuses"))
            .collect::<Result<_, _>>()?;
        if bonuses.iter().copied().collect::<BTreeSet<_>>().len() != bonuses.len() {
            return Err(fail("duplicate bonus"));
        }
        if available_bonuses.is_none() {
            let mods = game_data::config(data, modifications::MODIFICATIONS_PATH)?
                .ok_or_else(|| fail("modifications config is missing"))?;
            let mut ids = BTreeSet::new();
            for row in mods
                .get("modifications")
                .into_iter()
                .flat_map(ConfigValue::children)
            {
                ids.insert(integer(row.get("id"), "modification id")?);
            }
            available_bonuses = Some(ids);
        }
        if bonuses.iter().any(|id| {
            !available_bonuses
                .as_ref()
                .expect("loaded above")
                .contains(id)
        }) {
            return Err(fail("unknown modification id"));
        }
        let defaults: Vec<u32> = config
            .get("default_modifications")
            .into_iter()
            .flat_map(ConfigValue::children)
            .map(|v| integer(Some(v), "default modification"))
            .collect::<Result<_, _>>()?;
        if bonuses.iter().any(|id| defaults.contains(id)) {
            return Err(fail(
                "premium bonuses must be owned modifications, not defaults",
            ));
        }
        let params = config
            .get("parameters")
            .ok_or_else(|| fail("missing parameters"))?;
        let plain_params = plain_config
            .get("parameters")
            .ok_or_else(|| fail("missing plain parameters"))?;
        if params
            .get("base_rarity_level")
            .and_then(ConfigValue::as_i32)
            .unwrap_or(0)
            != 0
            || params.get("item_level") != plain_params.get("item_level")
        {
            return Err(fail(
                "premium copy must have base rarity zero and its plain item's level",
            ));
        }
        let kind = if weapon {
            if bonuses != [SILVER_BONUS] {
                return Err(fail(
                    "a golden weapon's one bonus is the silver bonus (1230)",
                ));
            }
            Kind::Weapon
        } else {
            Kind::Armour(bonuses)
        };
        result.insert(
            item.name.clone(),
            PremiumCopy {
                plain_name: plain.name.clone(),
                kind,
            },
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stock data with one more item: a copy of `template`'s config (dict id 9000) with
    /// `srp_premium` naming `plain` and `bonuses`, and `defaults` as its default modifications.
    fn fixture(
        template: u16,
        plain: u16,
        bonuses: &[i32],
        defaults: &[i32],
    ) -> (GameData, Dictionary) {
        let stock = GameData::test_archive();
        let mut dictionary =
            Dictionary::parse(stock.file(crate::v026g::dictionary::PATH).unwrap()).unwrap();
        let mut item = dictionary.item(template).unwrap().clone();
        let mut config = ConfigValue::parse(stock.file(&item.cfg_name).unwrap()).unwrap();
        let ConfigValue::Named(root) = &mut config else {
            panic!("named config")
        };
        root.retain(|(name, _)| name != "default_modifications");
        root.push((
            "default_modifications".into(),
            ConfigValue::Indexed(defaults.iter().copied().map(ConfigValue::Integer).collect()),
        ));
        root.push((
            "srp_premium".into(),
            ConfigValue::Named(vec![
                ("plain_item".into(), ConfigValue::Integer(i32::from(plain))),
                (
                    "bonuses".into(),
                    ConfigValue::Indexed(
                        bonuses.iter().copied().map(ConfigValue::Integer).collect(),
                    ),
                ),
            ]),
        ));
        let folder = item.cfg_name.rsplit_once('/').unwrap().0.to_owned();
        item.dict_id = 9000;
        item.name = "a_content_pack_premium".into();
        item.cfg_name = format!("{folder}/test_pack.options");
        item.is_premium = true;
        let data = stock.laid_over(
            "premium metadata fixture",
            vec![(item.cfg_name.clone(), config.to_bytes().unwrap())],
        );
        dictionary.items.push(item);
        dictionary.items.sort_by_key(|item| item.dict_id);
        (data, dictionary)
    }

    /// scav_torso_4 (armour) and the Kiparis (a weapon, 88; its golden copy 332).
    const TORSO: u16 = 325;
    const KIPARIS: u16 = 88;

    #[test]
    fn a_pack_defines_its_premium_without_a_server_item_row() {
        let (data, dictionary) = fixture(TORSO, TORSO, &[1240, 1250], &[1030]);
        let premiums = load(&data, &dictionary).unwrap();
        let item = &premiums["a_content_pack_premium"];
        assert_eq!(item.plain_name, "scav_torso_4");
        assert!(matches!(item.kind, Kind::Armour(_)));
        assert_eq!(item.bonuses(), [1240, 1250]);
        assert_eq!(premiums.len(), 1);
    }

    #[test]
    fn a_pack_defines_a_golden_weapon_with_the_silver_bonus() {
        let (data, dictionary) = fixture(KIPARIS, KIPARIS, &[1230], &[]);
        let premiums = load(&data, &dictionary).unwrap();
        let item = &premiums["a_content_pack_premium"];
        assert_eq!(item.plain_name, "kiparis");
        assert!(matches!(item.kind, Kind::Weapon));
        assert_eq!(item.bonuses(), [SILVER_BONUS]);
        assert_eq!(premiums.len(), 1);
    }

    #[test]
    fn malformed_premium_relationships_are_refused_at_the_boundary() {
        for (template, plain, bonuses, defaults, reason) in [
            (TORSO, 9000, vec![1240], vec![], "distinct plain piece"),
            (TORSO, 326, vec![1240], vec![], "same category"),
            (TORSO, 345, vec![1240], vec![], "itself a premium"),
            (TORSO, 9999, vec![1240], vec![], "does not exist"),
            (TORSO, TORSO, vec![], vec![], "modification slots"),
            (TORSO, TORSO, vec![1240, 1240], vec![], "duplicate bonus"),
            (TORSO, TORSO, vec![99999], vec![], "unknown modification"),
            (TORSO, TORSO, vec![1240], vec![1240], "not defaults"),
            (KIPARIS, KIPARIS, vec![1240], vec![], "silver bonus"),
            (KIPARIS, KIPARIS, vec![1230, 1240], vec![], "silver bonus"),
            (KIPARIS, 332, vec![1230], vec![], "itself a premium"),
            (KIPARIS, TORSO, vec![1230], vec![], "same category"),
        ] {
            let (data, dictionary) = fixture(template, plain, &bonuses, &defaults);
            let error = load(&data, &dictionary)
                .err()
                .expect("invalid metadata refused");
            assert!(error.0.contains(reason), "{error}");
        }
    }
}

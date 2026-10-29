//! Premium equipment declared by its content pack, without a server item table.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use binary_config_parser::tree::ConfigValue;

use super::{Dictionary, GameData, GameDataError};
use crate::v026g::{game_data, modifications};

pub(super) struct PremiumArmour {
    pub plain_name: String,
    pub bonuses: Vec<u32>,
}

pub(super) fn shared() -> &'static BTreeMap<String, PremiumArmour> {
    static CONTENT: OnceLock<BTreeMap<String, PremiumArmour>> = OnceLock::new();
    CONTENT.get_or_init(|| {
        load(game_data::shared(), Dictionary::shared())
            .expect("premium equipment metadata was checked when loading game data")
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
) -> Result<BTreeMap<String, PremiumArmour>, GameDataError> {
    let mut result = BTreeMap::new();
    let mut available_bonuses = None;
    for item in &dictionary.items {
        if !(1..=7).contains(&item.category) {
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
                "requires a premium armour piece and a distinct plain piece of the same category",
            ));
        }
        let plain_config = game_data::config(data, &plain.cfg_name)?
            .ok_or_else(|| fail("plain config is missing"))?;
        if plain_config.get("srp_premium").is_some()
            || super::PREMIUM_ARMOUR.iter().any(|r| r.0 == plain.name)
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
        result.insert(
            item.name.clone(),
            PremiumArmour {
                plain_name: plain.name.clone(),
                bonuses,
            },
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(plain: u16, bonuses: &[i32], defaults: &[i32]) -> (GameData, Dictionary) {
        let stock = GameData::test_archive();
        let mut dictionary =
            Dictionary::parse(stock.file(crate::v026g::dictionary::PATH).unwrap()).unwrap();
        let mut item = dictionary.item(325).unwrap().clone();
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
        item.dict_id = 9000;
        item.name = "a_content_pack_premium".into();
        item.cfg_name = "gameplay/items/armour/torso/test_pack.options".into();
        item.is_premium = true;
        let data = stock.laid_over(
            "premium metadata fixture",
            vec![(item.cfg_name.clone(), config.to_bytes().unwrap())],
        );
        dictionary.items.push(item);
        dictionary.items.sort_by_key(|item| item.dict_id);
        (data, dictionary)
    }

    #[test]
    fn a_pack_defines_its_premium_without_a_server_item_row() {
        let (data, dictionary) = fixture(325, &[1240, 1250], &[1030]);
        let premiums = load(&data, &dictionary).unwrap();
        let item = &premiums["a_content_pack_premium"];
        assert_eq!(item.plain_name, "scav_torso_4");
        assert_eq!(item.bonuses, [1240, 1250]);
        assert_eq!(premiums.len(), 1);
    }

    #[test]
    fn malformed_premium_relationships_are_refused_at_the_boundary() {
        for (plain, bonuses, defaults, reason) in [
            (9000, vec![1240], vec![], "distinct plain piece"),
            (326, vec![1240], vec![], "same category"),
            (345, vec![1240], vec![], "itself a premium"),
            (9999, vec![1240], vec![], "does not exist"),
            (325, vec![], vec![], "modification slots"),
            (325, vec![1240, 1240], vec![], "duplicate bonus"),
            (325, vec![99999], vec![], "unknown modification"),
            (325, vec![1240], vec![1240], "not defaults"),
        ] {
            let (data, dictionary) = fixture(plain, &bonuses, &defaults);
            let error = load(&data, &dictionary)
                .err()
                .expect("invalid metadata refused");
            assert!(error.0.contains(reason), "{error}");
        }
    }
}

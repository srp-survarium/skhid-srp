//! The lobby's item properties: `ui_properties` of
//! `gameplay/static_game_parameters.options`, the lines an item card and a
//! modification print ("Damage", "Weight limit"), each by its `prop_id`. A
//! modification names one ([`super::modifications::Shown`]) with the value
//! the lobby prints beside it.

use std::sync::OnceLock;

use binary_config_parser::tree::{ConfigError, ConfigValue};

/// The file in the game's archive.
pub const PATH: &str = "gameplay/static_game_parameters.options";

/// How the lobby reads a property's value (`mod_type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// 1: an amount added ("+2" armour, "+1" kg of weight limit).
    Adds,
    /// 2: a fraction the item's value is multiplied by `1 +` ("+5 %").
    Multiplies,
}

/// One row of `ui_properties`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiProperty {
    pub id: u32,
    /// The localization key of its name (`prop_name`); two rows have none.
    pub name: Option<String>,
    /// The localization key of its unit (`postfix`: `st_item_props_kg`).
    pub postfix: Option<String>,
    /// `None` for a property that is no change (a booster's, reliability).
    pub change: Option<Change>,
    /// `direction`: a bigger value is better.
    pub more_is_better: bool,
}

/// Every row, by id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiProperties {
    rows: Vec<UiProperty>,
}

impl UiProperties {
    /// The rows of `static_game_parameters.options`.
    pub fn parse(bytes: &[u8]) -> Result<Self, ConfigError> {
        let root = ConfigValue::parse(bytes)?;
        let mut rows = root
            .req("ui_properties")?
            .children()
            .into_iter()
            .map(|row| {
                let id = row.i32_at("prop_id")?;
                let change = match row.get("mod_type").map(ConfigValue::as_i32) {
                    None => None,
                    Some(Some(1)) => Some(Change::Adds),
                    Some(Some(2)) => Some(Change::Multiplies),
                    Some(other) => {
                        return Err(ConfigError(format!(
                            "property {id}: `mod_type` {other:?} is neither 1 nor 2"
                        )));
                    }
                };
                Ok(UiProperty {
                    id: u32::try_from(id)
                        .map_err(|_| ConfigError(format!("`prop_id` {id} is negative")))?,
                    name: row
                        .get("prop_name")
                        .map(|name| {
                            name.as_str().map(str::to_owned).ok_or_else(|| {
                                ConfigError(format!("property {id}: `prop_name` is no text"))
                            })
                        })
                        .transpose()?,
                    postfix: row
                        .get("postfix")
                        .and_then(ConfigValue::as_str)
                        .filter(|key| !key.is_empty())
                        .map(str::to_owned),
                    change,
                    more_is_better: row.get("direction").and_then(ConfigValue::as_i32) != Some(0),
                })
            })
            .collect::<Result<Vec<_>, ConfigError>>()?;
        rows.sort_by_key(|row| row.id);
        if let Some(pair) = rows.windows(2).find(|pair| pair[0].id == pair[1].id) {
            return Err(ConfigError(format!(
                "`prop_id` {} is in ui_properties twice",
                pair[0].id
            )));
        }
        Ok(Self { rows })
    }

    /// The property `id`.
    pub fn get(&self, id: u32) -> Option<&UiProperty> {
        self.rows
            .binary_search_by_key(&id, |row| row.id)
            .ok()
            .map(|index| &self.rows[index])
    }

    /// The rows of this process's game data ([`super::game_data::shared`]),
    /// which [`super::tables::check`] parsed at start.
    pub fn shared() -> &'static Self {
        static SHARED: OnceLock<UiProperties> = OnceLock::new();
        SHARED.get_or_init(|| {
            Self::parse(super::game_data::file(PATH)).expect("the ui properties parse")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lobbys_lines_are_read_with_their_units() {
        let properties = UiProperties::shared();
        let damage = properties.get(1).unwrap();
        assert_eq!(damage.name.as_deref(), Some("st_inv_damage"));
        assert_eq!(damage.change, Some(Change::Multiplies));
        assert!(damage.more_is_better);
        let weight_limit = properties.get(106).unwrap();
        assert_eq!(weight_limit.change, Some(Change::Adds));
        assert_eq!(weight_limit.postfix.as_deref(), Some("st_item_props_kg"));
        // Dispersion: less is better.
        assert!(!properties.get(2).unwrap().more_is_better);
        // Reliability is no change the lobby computes.
        assert_eq!(properties.get(123).unwrap().change, None);
    }
}

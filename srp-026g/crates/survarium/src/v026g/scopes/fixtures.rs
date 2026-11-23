//! Game data made up for tests: the test data with shop scope items laid
//! over it as a release patch adds them ([`with_scopes`]: rows in
//! `items_dict` of a new dictionary and a config for each), and the modules
//! table that fits them to weapons ([`modules`], made by
//! [`weapon_modules::fixtures`](super::super::weapon_modules::fixtures)).
//! The test data itself has no table: nothing fits anything in it.

use binary_config_parser::tree::ConfigValue;

use super::super::dictionary::{self, Dictionary};
use super::super::game_data::{self, GameData};
use super::super::weapon_modules::{self, Kind, Modules};
use super::SCOPE_CATEGORY;

/// The shop's PSO-1 (sold for [`PSO_1_COST`]).
pub const PSO_1: u16 = 2101;
/// The shop's Cobra EKP-8 (sold for [`COBRA_COST`]).
pub const COBRA: u16 = 2108;
/// A scope item no weapon takes: not sold, put on nothing.
pub const LONE_SCOPE: u16 = 2199;
pub const PSO_1_COST: u32 = 12_000;
pub const COBRA_COST: u32 = 7_000;
/// The weapons of [`modules`]' table and the scopes each takes: the
/// AK-74N and its premium copy both, the SVD (which has a default optic,
/// 0.26g's PSO-1 304) the Cobra alone.
pub const FITS: [(&str, &[u16]); 3] = [
    ("ak_74n", &[PSO_1, COBRA]),
    ("ak_74n_premium", &[PSO_1, COBRA]),
    ("svd", &[COBRA]),
];

fn named(entries: Vec<(&str, ConfigValue)>) -> ConfigValue {
    ConfigValue::Named(
        entries
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

fn text(value: &str) -> ConfigValue {
    ConfigValue::String(value.to_owned())
}

/// A shop scope item's config: what the client shows of it (models and
/// zoom) and its modification, which adds `data/srp_scope` = itself to the
/// weapon that wears its record.
fn scope_config(dict_id: u16, name: &str) -> ConfigValue {
    let modification = 991_000 + i32::from(dict_id);
    named(vec![
        (
            "parameters",
            named(vec![
                ("weight", ConfigValue::Float(0.3)),
                ("modification", ConfigValue::Integer(modification)),
            ]),
        ),
        (
            "modifications",
            ConfigValue::Indexed(vec![named(vec![
                ("id", ConfigValue::Integer(modification)),
                (
                    "modifiers",
                    ConfigValue::Indexed(vec![named(vec![
                        ("path", ConfigValue::Indexed(vec![text("data")])),
                        (
                            "value",
                            ConfigValue::Indexed(vec![
                                text("srp_scope"),
                                ConfigValue::Integer(dict_id.into()),
                            ]),
                        ),
                        ("type", ConfigValue::Integer(3)),
                    ])]),
                ),
            ])]),
        ),
        (
            "data",
            named(vec![
                ("fov_factor", ConfigValue::Float(0.25)),
                ("hide_weapon_on_aim", ConfigValue::Boolean(true)),
                (
                    "aimed_model",
                    text(&format!("weapons/modules_069/scopes/{name}/{name}_aim")),
                ),
                (
                    "idle_model",
                    text(&format!("weapons/modules_069/scopes/{name}/{name}")),
                ),
                ("type", ConfigValue::Integer(5)),
            ]),
        ),
    ])
}

/// `(dict id, name, base_cost)` of each added scope item.
const ADDED: [(u16, &str, u32); 3] = [
    (PSO_1, "shop_scope_pso_1", PSO_1_COST),
    (COBRA, "shop_scope_cobra_ekp_8", COBRA_COST),
    (LONE_SCOPE, "shop_scope_lone", 9_000),
];

/// The test data ([`game_data::shared`]) with the scope items of [`ADDED`]
/// in a new dictionary and their configs, and an `items_compatibility` row
/// for each weapon and the scope items it takes ([`compatibility_rows`]), as
/// the lobby movie reads them (`docs/notes/weapon_scopes.md` §4).
pub fn with_scopes() -> GameData {
    with_scopes_and_slots(&[])
}

/// The weapon and scope item of each `items_compatibility` row the
/// fixture adds: one for each pair of [`FITS`], and one for each weapon and
/// its default optic ([`STOCK_ROWS`]: the lobby movie shows a greyed
/// "standard scope" in an empty slot).
pub fn compatibility_rows() -> Vec<(u16, u16)> {
    let dict = |name: &str| Dictionary::shared().item_named(name).unwrap().dict_id;
    let mut rows: Vec<(u16, u16)> = FITS
        .iter()
        .flat_map(|&(weapon, scopes)| scopes.iter().map(move |&scope| (dict(weapon), scope)))
        .collect();
    rows.extend(
        STOCK_ROWS
            .iter()
            .map(|&(weapon, stock)| (dict(weapon), stock)),
    );
    rows
}

/// Each 0.26g weapon with a default optic and that optic (its config's
/// `addons.rifle_scope_dict_id`): seven rows.
pub const STOCK_ROWS: [(&str, u16); 7] = [
    ("rem_700", 69),
    ("svd", 304),
    ("vsk_94", 304),
    ("vss", 304),
    ("vss_premium", 304),
    ("sv_98", 362),
    ("vssk", 362),
];

/// [`with_scopes`], and `slots` as `(category, profile slot)` rows of
/// `slots_restrictions` (a data mistake a test makes).
pub fn with_scopes_and_slots(slots: &[(u8, u8)]) -> GameData {
    laid_over(&ADDED, slots)
}

/// The test data with `scopes` (`(dict id, name, base_cost)`) as scope
/// items and `slots` as rows of `slots_restrictions`.
pub fn laid_over(scopes: &[(u16, &str, u32)], slots: &[(u8, u8)]) -> GameData {
    let stock = game_data::shared();
    let mut dictionary_config = ConfigValue::parse(stock.file(dictionary::PATH).unwrap()).unwrap();
    let ConfigValue::Named(root) = &mut dictionary_config else {
        panic!("the dictionary is not a named table");
    };
    let mut files = Vec::new();
    for (name, entry) in root.iter_mut() {
        match (name.as_str(), entry) {
            ("items_dict", ConfigValue::Indexed(rows)) => {
                for &(dict_id, name, cost) in scopes {
                    let cfg_name = format!("gameplay/items/scopes/{name}.options");
                    rows.push(named(vec![
                        ("cfg_name", text(&cfg_name)),
                        ("is_stack", ConfigValue::Boolean(false)),
                        ("name", text(name)),
                        ("item_category", ConfigValue::Integer(SCOPE_CATEGORY.into())),
                        ("dict_id", ConfigValue::Integer(dict_id.into())),
                        ("base_cost", ConfigValue::Integer(cost as i32)),
                        ("is_premium", ConfigValue::Boolean(false)),
                    ]));
                    files.push((cfg_name, scope_config(dict_id, name).to_bytes().unwrap()));
                }
            }
            // Named tables in the game's file: `compatibility_<n>`,
            // `restriction_<n>`; a patch's rows need names of their own.
            ("items_compatibility", ConfigValue::Named(rows)) => {
                for (n, (weapon, scope)) in compatibility_rows().into_iter().enumerate() {
                    rows.push((
                        format!("compatibility_scope_{n}"),
                        named(vec![
                            ("first_item", ConfigValue::Integer(weapon.into())),
                            ("second_item", ConfigValue::Integer(scope.into())),
                        ]),
                    ));
                }
            }
            ("slots_restrictions", ConfigValue::Named(rows)) => {
                for (n, &(category, slot)) in slots.iter().enumerate() {
                    rows.push((
                        format!("restriction_scope_{n}"),
                        named(vec![
                            ("category_id", ConfigValue::Integer(category.into())),
                            ("profile_slot_id", ConfigValue::Integer(slot.into())),
                        ]),
                    ));
                }
            }
            ("items_dict" | "items_compatibility" | "slots_restrictions", _) => {
                panic!("{name} is not the table the game's file has")
            }
            _ => {}
        }
    }
    files.push((
        dictionary::PATH.to_owned(),
        dictionary_config.to_bytes().unwrap(),
    ));
    stock.laid_over("the test data and the scopes of a patch", files)
}

/// The dictionary of `data`.
pub fn dictionary_of(data: &GameData) -> Dictionary {
    Dictionary::parse(data.file(dictionary::PATH).unwrap()).unwrap()
}

/// The modules table of [`FITS`], made once.
pub fn modules() -> &'static Modules {
    static MODULES: std::sync::OnceLock<Modules> = std::sync::OnceLock::new();
    MODULES.get_or_init(|| {
        let dict = |name: &str| Dictionary::shared().item_named(name).unwrap().dict_id;
        let fits: Vec<(u16, u16, Kind)> = FITS
            .iter()
            .flat_map(|&(weapon, scopes)| {
                scopes.iter().map(move |&scope| (dict(weapon), scope, Kind::Scope))
            })
            .collect();
        weapon_modules::fixtures::modules(&fits)
    })
}

//! The items workbook, `/items.xlsx` on the 0.26g site: every item of the
//! dictionary with every value its config has, and the game's shared tables,
//! for balancing (`docs/notes/items_xlsx.md`).
//!
//! It is made from the game data the server runs on
//! ([`survarium::v026g::game_data::shared`]: the stock `resources.db` with
//! exactly the patches it offers at `/patches`), once a process, so it is
//! never older than the game the players have: a release that ships a patch
//! restarts the server with the new data, and the workbook with it. Nothing
//! is picked by hand:
//!
//! - an item sheet (and "All") has a row for each item of `items_dict`, and a
//!   column for every value any item's config has, by its path in the config
//!   (`recoil/max_top_recoil`, `hit_params/brain/hit_types/ranged/armor`);
//!   a list of numbers is a column per index, a list of strings one cell;
//! - the lobby card's numbers (`ui_desc/props_list`) are a column per
//!   property, named as the lobby names it;
//! - `derived/…` columns are the server's own reading of the data: the shop,
//!   bundles, drops, the starting kit, the ammo and scopes that fit, the
//!   modifications and defects the item can roll;
//! - `with_default_modifications/…` is a value with the item's
//!   `default_modifications` applied, as the client's `item_config_cook`
//!   applies them;
//! - the shared tables (modifications, defects, hit parameters, body parts,
//!   factions, the lobby's properties, the shop) have a sheet each, and
//!   "Game files" lists every value of every other `gameplay/` file;
//! - "Hard-coded" lists what is in the exe or the server's code, not in the
//!   data, and "Columns" says what each item column is.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::net::{Shutdown, TcpStream};
use std::sync::OnceLock;

use binary_config_parser::tree::ConfigValue;
use rust_xlsxwriter::{Format, Workbook, XlsxError};
use survarium::v026g::dictionary::{Dictionary, Item};
use survarium::v026g::drops::{DropTable, Drops};
use survarium::v026g::game_data::{self, GameData};
use survarium::v026g::localization::{self, Language};
use survarium::v026g::modifications::Modifications;
use survarium::v026g::scopes::{SCOPE_CATEGORY, ScopeFits};
use survarium::v026g::{item_params, loadouts, prices, shop_offers};

/// The workbook's address.
pub(crate) const PATH: &str = "/items.xlsx";

/// Its content type.
const CONTENT_TYPE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

/// The most text a cell holds (Excel's limit).
const CELL_TEXT: usize = 32_767;
/// The most columns a sheet has (Excel's limit).
const MAX_COLUMNS: usize = 16_384;

/// A value of a cell: numbers stay numbers.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Cell {
    Number(f64),
    Text(String),
    Bool(bool),
}

impl Cell {
    fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    fn number(value: f64) -> Self {
        if value.is_finite() {
            Self::Number(value)
        } else {
            Self::Text(value.to_string())
        }
    }

    pub(crate) fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            _ => None,
        }
    }
}

/// An f32 of the data as the shortest decimal that reads back as it
/// (0.3, not 0.30000001192092896).
fn float(value: f32) -> f64 {
    value.to_string().parse().unwrap_or(f64::from(value))
}

/// A row: `(column, value)`, in the config's order.
type Row = Vec<(String, Cell)>;

/// A sheet: its columns and rows, a cell per column or none.
#[derive(Debug)]
pub(crate) struct Table {
    pub(crate) name: String,
    pub(crate) columns: Vec<String>,
    pub(crate) rows: Vec<Vec<Option<Cell>>>,
    /// The columns kept in view (frozen) beside the header row.
    frozen: u16,
}

#[cfg(test)]
impl Table {
    /// The cell of `row` in the column `column`.
    pub(crate) fn cell(&self, row: usize, column: &str) -> Option<&Cell> {
        let index = self.columns.iter().position(|name| name == column)?;
        self.rows.get(row)?.get(index)?.as_ref()
    }
}

/// The order of the columns: a tree of their paths, each level in the
/// order its names were first seen, so a config's values stay together.
#[derive(Default)]
struct Order {
    children: Vec<(String, Order)>,
    leaf: bool,
}

impl Order {
    fn insert(&mut self, path: &str) {
        let mut node = self;
        for part in path.split('/') {
            let index = match node.children.iter().position(|(name, _)| name == part) {
                Some(index) => index,
                None => {
                    node.children.push((part.to_owned(), Order::default()));
                    node.children.len() - 1
                }
            };
            node = &mut node.children[index].1;
        }
        node.leaf = true;
    }

    fn paths(&self, prefix: &str, out: &mut Vec<String>) {
        if self.leaf {
            out.push(prefix.to_owned());
        }
        for (name, child) in &self.children {
            child.paths(&join(prefix, name), out);
        }
    }
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
}

/// Where a top-level name goes among the columns: the generated ones and
/// the game values first, how the item looks and sounds last. Order only;
/// every column is there.
fn rank(top: &str) -> usize {
    const FIRST: [&str; 13] = [
        "items_dict",
        "derived",
        "with_default_modifications",
        "ui_desc",
        "parameters",
        "recoil",
        "dispersion",
        "breath_vibration",
        "data",
        "hit_params",
        "melee",
        "addons",
        "default_modifications",
    ];
    const LAST: [&str; 9] = [
        "states",
        "user_animations",
        "user_animations_in_place",
        "recoil_animations",
        "sounds",
        "particles",
        "object",
        "collision_geometries",
        "clothes_affinities",
    ];
    FIRST
        .iter()
        .position(|name| *name == top)
        .or_else(|| LAST.iter().position(|name| *name == top).map(|i| 100 + i))
        .unwrap_or(50)
}

/// A table of `rows`, `lead` first, then every other column in [`Order`]
/// (its top-level names by [`rank`] when `ranked`).
fn table(name: &str, lead: &[&str], rows: Vec<Row>, ranked: bool, frozen: u16) -> Table {
    let mut order = Order::default();
    for row in &rows {
        for (column, _) in row {
            if !lead.contains(&column.as_str()) {
                order.insert(column);
            }
        }
    }
    if ranked {
        order.children.sort_by_key(|(top, _)| rank(top));
    }
    // The lead columns some row has.
    let mut columns: Vec<String> = lead
        .iter()
        .filter(|&&name| {
            rows.iter()
                .any(|row| row.iter().any(|(column, _)| column == name))
        })
        .map(|&name| name.to_owned())
        .collect();
    order.paths("", &mut columns);
    let index: HashMap<&str, usize> = columns
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), i))
        .collect();
    let rows = rows
        .into_iter()
        .map(|row| {
            let mut cells = vec![None; columns.len()];
            for (column, cell) in row {
                cells[index[column.as_str()]] = Some(cell);
            }
            cells
        })
        .collect();
    Table {
        name: name.to_owned(),
        columns,
        rows,
        frozen,
    }
}

/// The lobby's item properties (`static_game_parameters`'s
/// `ui_properties`): their names and postfixes, by `prop_id`.
struct Property {
    name_en: String,
    name_uk: String,
    postfix: String,
}

fn english(key: &str) -> Option<&'static str> {
    localization::shared(Language::English)
        .get(key)
        .map(str::trim)
}

fn ukrainian(key: &str) -> Option<&'static str> {
    localization::shared(Language::Ukrainian)
        .get(key)
        .map(str::trim)
}

/// Everything the sheets are made of, read once.
struct Game {
    data: &'static GameData,
    dictionary: &'static Dictionary,
    properties: BTreeMap<i32, Property>,
    /// Faction id: its name key and its ranks' `(value, name key)`.
    factions: BTreeMap<i32, (String, Vec<(f64, String)>)>,
    /// `items_dict`'s rows as they are, by dict id.
    dict_rows: HashMap<u16, &'static ConfigValue>,
    /// The categories of grenades and traps (`gameplay/item_categories`).
    thrown: Vec<i32>,
    /// A modification's (false) or defect's (true) name, by id: its
    /// `ui_desc/name` where the texts have it, else the lobby's names of
    /// the properties its card shows (0.26g has no text for most).
    labels: HashMap<(bool, u32), (String, String)>,
}

/// A config of the data, parsed and kept for the process.
fn config(data: &GameData, path: &str) -> Option<&'static ConfigValue> {
    static CONFIGS: OnceLock<std::sync::Mutex<HashMap<String, &'static ConfigValue>>> =
        OnceLock::new();
    let configs = CONFIGS.get_or_init(Default::default);
    let mut configs = configs.lock().unwrap_or_else(|poison| poison.into_inner());
    if let Some(config) = configs.get(path) {
        return Some(config);
    }
    let parsed = game_data::config(data, path).ok().flatten()?;
    let parsed: &'static ConfigValue = Box::leak(Box::new(parsed));
    configs.insert(path.to_owned(), parsed);
    Some(parsed)
}

fn number_of(value: &ConfigValue) -> Option<f64> {
    match value {
        ConfigValue::Integer(i) => Some(f64::from(*i)),
        ConfigValue::Float(f) => Some(float(*f)),
        _ => None,
    }
}

impl Game {
    fn read() -> Self {
        let data = game_data::shared();
        let dictionary = Dictionary::shared();
        let mut properties = BTreeMap::new();
        if let Some(list) = config(data, "gameplay/static_game_parameters.options")
            .and_then(|root| root.get("ui_properties"))
        {
            for row in list.children() {
                let Some(id) = row.get("prop_id").and_then(ConfigValue::as_i32) else {
                    continue;
                };
                let key = row.get("prop_name").and_then(ConfigValue::as_str);
                let postfix = row.get("postfix").and_then(ConfigValue::as_str);
                properties.insert(
                    id,
                    Property {
                        name_en: key.and_then(english).unwrap_or_default().to_owned(),
                        name_uk: key.and_then(ukrainian).unwrap_or_default().to_owned(),
                        postfix: postfix.and_then(english).unwrap_or_default().to_owned(),
                    },
                );
            }
        }
        let dictionaries = config(data, survarium::v026g::dictionary::PATH);
        let mut factions = BTreeMap::new();
        if let Some(list) = dictionaries.and_then(|root| root.get("factions_dict")) {
            for faction in list.children() {
                let Some(id) = faction.get("id").and_then(ConfigValue::as_i32) else {
                    continue;
                };
                let name = faction.get("name").and_then(ConfigValue::as_str);
                let levels = faction
                    .get("levels")
                    .map(|levels| {
                        levels
                            .children()
                            .into_iter()
                            .map(|level| {
                                (
                                    level.get("value").and_then(number_of).unwrap_or(0.0),
                                    level
                                        .get("name")
                                        .and_then(ConfigValue::as_str)
                                        .unwrap_or_default()
                                        .to_owned(),
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                factions.insert(id, (name.unwrap_or_default().to_owned(), levels));
            }
        }
        let mut dict_rows = HashMap::new();
        if let Some(list) = dictionaries.and_then(|root| root.get("items_dict")) {
            for row in list.children() {
                if let Some(id) = row.get("dict_id").and_then(ConfigValue::as_i32) {
                    dict_rows.insert(id as u16, row);
                }
            }
        }
        let thrown = config(data, "gameplay/item_categories")
            .and_then(|root| root.get("item_categories"))
            .map(|categories| {
                categories
                    .children()
                    .into_iter()
                    .filter_map(ConfigValue::as_i32)
                    .collect()
            })
            .unwrap_or_default();
        let mut game = Self {
            data,
            dictionary,
            properties,
            factions,
            dict_rows,
            thrown,
            labels: HashMap::new(),
        };
        game.labels = game.modification_labels();
        game
    }

    fn modification_labels(&self) -> HashMap<(bool, u32), (String, String)> {
        let mut labels = HashMap::new();
        let Some(root) = config(self.data, MODIFICATIONS_PATH) else {
            return labels;
        };
        for (key, defect) in [("modifications", false), ("defects", true)] {
            for row in root.get(key).map(ConfigValue::children).unwrap_or_default() {
                let Some(id) = row.get("id").and_then(ConfigValue::as_i32) else {
                    continue;
                };
                let desc = row.get("ui_desc");
                let name = desc
                    .and_then(|desc| desc.get("name"))
                    .and_then(ConfigValue::as_str)
                    .unwrap_or_default();
                let props: Vec<&Property> = desc
                    .and_then(|desc| desc.get("props_list"))
                    .map(ConfigValue::children)
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|prop| prop.get("prop_id").and_then(ConfigValue::as_i32))
                    .filter_map(|id| self.properties.get(&id))
                    .collect();
                let of_props = |each: fn(&Property) -> &str| {
                    let names: Vec<&str> = props.iter().map(|&prop| each(prop)).collect();
                    names.join(", ")
                };
                let en = english(name).map_or_else(|| of_props(|p| &p.name_en), str::to_owned);
                let uk = ukrainian(name).map_or_else(|| of_props(|p| &p.name_uk), str::to_owned);
                let en = if en.is_empty() { name.to_owned() } else { en };
                labels.insert((defect, id as u32), (en, uk));
            }
        }
        labels
    }

    /// A modification's (`defect` false) or defect's English and
    /// Ukrainian name ([`Game::labels`]).
    fn label(&self, defect: bool, id: u32) -> (&str, &str) {
        self.labels
            .get(&(defect, id))
            .map_or(("", ""), |(en, uk)| (en.as_str(), uk.as_str()))
    }

    fn faction_name(&self, id: u8) -> String {
        if id == 0 {
            return "everyone".into();
        }
        self.factions
            .get(&i32::from(id))
            .and_then(|(key, _)| english(key))
            .map_or_else(|| format!("faction {id}"), str::to_owned)
    }

    fn rank_name(&self, faction: u8, level: u8) -> Option<&'static str> {
        let (_, levels) = self.factions.get(&i32::from(faction))?;
        english(&levels.get(usize::from(level))?.1)
    }

    fn item_name(&self, dict_id: u16) -> String {
        let Some(item) = self.dictionary.item(dict_id) else {
            return format!("#{dict_id}");
        };
        let english = self.names(item).0;
        if english.is_empty() {
            item.name.clone()
        } else {
            format!("{english} ({})", item.name)
        }
    }

    /// An item's English and Ukrainian names, from its config's
    /// `ui_desc/text_descriptions/name`; for an artefact, which 0.26g has
    /// no name text for, the first sentence of its pickup hint
    /// (`data/pickup_hint`: "Onyx artifact. ...").
    fn names(&self, item: &Item) -> (String, String) {
        let config = config(self.data, &item.cfg_name);
        let key = config
            .and_then(|root| root.get("ui_desc"))
            .and_then(|desc| desc.get("text_descriptions"))
            .and_then(|texts| texts.get("name"))
            .and_then(ConfigValue::as_str);
        let hint = config
            .and_then(|root| root.get("data"))
            .and_then(|data| data.get("pickup_hint"))
            .and_then(ConfigValue::as_str);
        let name = |text: fn(&str) -> Option<&'static str>| {
            key.and_then(text)
                .or_else(|| {
                    let hint = hint.and_then(text)?;
                    hint.split('.').next().map(str::trim)
                })
                .unwrap_or_default()
                .to_owned()
        };
        (name(english), name(ukrainian))
    }

    /// The sheet an item is on: by the folder of its config.
    fn sheet_of(&self, item: &Item) -> String {
        let path = item.cfg_name.as_str();
        let folder: Vec<&str> = path
            .strip_prefix("gameplay/items/")
            .unwrap_or(path)
            .split('/')
            .collect();
        match folder.as_slice() {
            ["weapons", "ammo", ..] => "Ammo".into(),
            ["weapons", ..] | ["devices", ..]
                if self.thrown.contains(&i32::from(item.category)) =>
            {
                "Grenades & traps".into()
            }
            ["weapons", ..] => "Weapons".into(),
            ["armour", slot, ..] => format!("Armour {slot}"),
            ["artefacts", ..] => "Artefacts".into(),
            ["drugs", ..] | ["boosters", ..] => "Consumables".into(),
            ["scopes", ..] => "Scopes".into(),
            ["upgrades", "camouflages", ..] => "Camouflages".into(),
            ["upgrades", "decals", ..] => "Decals".into(),
            _ => "Other".into(),
        }
    }
}

/// The sheets of items, in this order; any other after them.
const ITEM_SHEETS: [&str; 17] = [
    "Weapons",
    "Grenades & traps",
    "Ammo",
    "Armour helmet",
    "Armour mask",
    "Armour torso",
    "Armour back",
    "Armour gloves",
    "Armour legs",
    "Armour boots",
    "Artefacts",
    "Consumables",
    "Scopes",
    "Camouflages",
    "Decals",
    "Other",
    "",
];

/// The columns every item sheet starts with.
const LEAD: [&str; 5] = [
    "items_dict/dict_id",
    "items_dict/name",
    "items_dict/cfg_name",
    "derived/name_en",
    "derived/name_uk",
];

/// A config's values as `(path, value)` cells (see the module).
fn flatten(game: &Game, path: &str, value: &ConfigValue, out: &mut Row) {
    let vector = |values: &[f32], out: &mut Row| {
        for (i, value) in values.iter().enumerate() {
            out.push((join(path, &i.to_string()), Cell::number(float(*value))));
        }
    };
    match value {
        ConfigValue::Boolean(b) => out.push((path.to_owned(), Cell::Bool(*b))),
        ConfigValue::Integer(i) => out.push((path.to_owned(), Cell::number(f64::from(*i)))),
        ConfigValue::Float(f) => out.push((path.to_owned(), Cell::number(float(*f)))),
        ConfigValue::String(s) => out.push((path.to_owned(), Cell::text(s.as_str()))),
        ConfigValue::Float2(v) => vector(v, out),
        ConfigValue::Float3(v) => vector(v, out),
        ConfigValue::Float4(v) => vector(v, out),
        ConfigValue::Named(entries) => {
            for (key, value) in entries {
                flatten(game, &join(path, key), value, out);
            }
        }
        ConfigValue::Indexed(list) if path.ends_with("props_list") && is_props(list) => {
            for prop in list {
                let id = prop
                    .get("prop_id")
                    .and_then(ConfigValue::as_i32)
                    .unwrap_or(0);
                let value = prop.get("prop_value");
                let cell = match value {
                    Some(ConfigValue::String(text)) => text
                        .trim()
                        .parse::<f64>()
                        .map_or_else(|_| Cell::text(text.as_str()), Cell::number),
                    Some(other) => number_of(other).map_or(Cell::text(""), Cell::number),
                    None => continue,
                };
                out.push((prop_column(game, path, id), cell));
            }
        }
        ConfigValue::Indexed(list)
            if !list.is_empty() && list.iter().all(|v| matches!(v, ConfigValue::String(_))) =>
        {
            // A modifier's path is a path; other lists of strings (models,
            // animations, names) one cell.
            let separator = if path.ends_with("/path") { "/" } else { "; " };
            let text = list
                .iter()
                .filter_map(ConfigValue::as_str)
                .collect::<Vec<_>>()
                .join(separator);
            out.push((path.to_owned(), Cell::Text(text)));
        }
        ConfigValue::Indexed(list) => {
            for (i, value) in list.iter().enumerate() {
                flatten(game, &join(path, &i.to_string()), value, out);
            }
        }
    }
}

fn is_props(list: &[ConfigValue]) -> bool {
    !list.is_empty()
        && list
            .iter()
            .all(|prop| prop.get("prop_id").is_some() && prop.get("prop_value").is_some())
}

/// The column of the lobby's property `id` in `path` (a `props_list`):
/// `<path>/<id> (<its English name>)`.
fn prop_column(game: &Game, path: &str, id: i32) -> String {
    match game.properties.get(&id) {
        Some(prop) if !prop.name_en.is_empty() => format!("{path}/{id} ({})", prop.name_en),
        _ => format!("{path}/{id}"),
    }
}

/// What the server makes of an item: how it is had, what fits it, what it
/// can roll.
fn derived(game: &Game, item: &Item, out: &mut Row) {
    let dict_id = item.dict_id;
    let mut push = |name: &str, cell: Cell| out.push((format!("derived/{name}"), cell));
    let (name_en, name_uk) = game.names(item);
    push("name_en", Cell::Text(name_en));
    push("name_uk", Cell::Text(name_uk));

    // The shop: silver records (one a seller) and the gold list.
    let silver = prices::sellers(dict_id);
    let gold = shop_offers::premium_price(dict_id);
    let records: Vec<&prices::PriceItem> = silver.iter().chain(gold).collect();
    let bundles: Vec<String> = shop_offers::BUNDLES
        .iter()
        .filter(|bundle| {
            bundle
                .contents(game.dictionary)
                .iter()
                .any(|&(id, _, _)| id == dict_id)
        })
        .map(|bundle| {
            format!(
                "{} ({} gold)",
                bundle_name(game, bundle.bundle_id),
                bundle.gold
            )
        })
        .collect();
    let drops = drop_tables(item);
    let starting = loadouts::STARTER
        .iter()
        .map(|&(_, name)| name)
        .chain(loadouts::STARTER_STASH)
        .any(|name| name == item.name);
    let not_sold = prices::NOT_SOLD
        .iter()
        .find(|&&(name, _)| name == item.name)
        .map(|&(_, why)| why);
    let unreleased = prices::UNRELEASED.contains(&item.name.as_str());
    let fits = ScopeFits::shared();
    let sheet = game.sheet_of(item);
    let status = if !records.is_empty() || !bundles.is_empty() {
        "sold"
    } else if fits.is_variant(dict_id) {
        "scope variant (how a scope looks on one weapon)"
    } else if sheet == "Artefacts" {
        "found in matches"
    } else if !drops.is_empty() {
        "drop only"
    } else if starting {
        "starting kit only"
    } else {
        "not obtainable"
    };
    push("sheet", Cell::Text(sheet));
    push("status", Cell::text(status));
    if let Some(record) = records.first() {
        let currency = if gold.is_some() && silver.is_empty() {
            "gold"
        } else {
            "silver"
        };
        push("shop_price", Cell::number(f64::from(record.cost)));
        push("shop_currency", Cell::text(currency));
        push(
            "shop_unit",
            Cell::number(f64::from(item_params::shop_unit(dict_id))),
        );
        let rank = records.iter().map(|record| record.level).min().unwrap_or(0);
        push("shop_min_rank", Cell::number(f64::from(rank)));
        let sellers: Vec<String> = records
            .iter()
            .map(|record| {
                let faction = game.faction_name(record.faction_id);
                match game.rank_name(record.faction_id, record.level) {
                    Some(rank) if record.faction_id != 0 => {
                        format!("{faction} rank {} ({rank})", record.level)
                    }
                    _ => format!("{faction} rank {}", record.level),
                }
            })
            .collect();
        push("shop_sellers", Cell::Text(sellers.join("; ")));
    }
    if !bundles.is_empty() {
        push("bundles", Cell::Text(bundles.join("; ")));
    }
    push("can_drop", Cell::Bool(!drops.is_empty()));
    if !drops.is_empty() {
        push("drop_tables", Cell::Text(drops.join("; ")));
    }
    push("starting_kit", Cell::Bool(starting));
    if let Some(why) = not_sold {
        push("not_sold", Cell::text(why));
    }
    if unreleased {
        push("unreleased", Cell::Bool(true));
    }

    // What fits it: the dictionary's pairs (ammo and weapons) and scopes.
    let compatible: Vec<String> = game
        .dictionary
        .compatibility
        .iter()
        .filter_map(|&(a, b)| match dict_id {
            id if id == a => Some(b),
            id if id == b => Some(a),
            _ => None,
        })
        .collect::<BTreeSet<u16>>()
        .into_iter()
        .map(|id| game.item_name(id))
        .collect();
    if !compatible.is_empty() {
        push("compatible_items", Cell::Text(compatible.join("; ")));
    }
    let scopes: BTreeSet<u16> = fits
        .all()
        .iter()
        .filter_map(|fit| {
            if fit.weapon == dict_id {
                Some(fit.scope)
            } else if fit.scope == dict_id || fit.variant == dict_id {
                Some(fit.weapon)
            } else {
                None
            }
        })
        .collect();
    if !scopes.is_empty() {
        let names: Vec<String> = scopes.into_iter().map(|id| game.item_name(id)).collect();
        let column = if item.category == SCOPE_CATEGORY {
            "fits_weapons"
        } else {
            "scopes_that_fit"
        };
        push(column, Cell::Text(names.join("; ")));
    }

    // Modifications and defects it can roll (the server's one pool).
    let modifications = Modifications::shared();
    let candidates = modifications.candidates(dict_id, &[]);
    if !candidates.is_empty() {
        let ids: Vec<String> = candidates.iter().map(|m| m.id.to_string()).collect();
        let mut types = BTreeMap::new();
        for m in &candidates {
            types
                .entry(m.kind)
                .or_insert_with(|| game.label(false, m.id).0);
        }
        let types: Vec<String> = types
            .into_iter()
            .map(|(kind, name)| format!("{kind} {name}"))
            .collect();
        push("can_roll_modifications", Cell::Text(ids.join(", ")));
        push("can_roll_modification_types", Cell::Text(types.join("; ")));
    }
    let defects = modifications.defect_candidates(dict_id, &[], &[]);
    if !defects.is_empty() {
        let ids: Vec<String> = defects
            .iter()
            .map(|d| format!("{} {}", d.id, game.label(true, d.id).0))
            .collect();
        push("can_get_defects", Cell::Text(ids.join("; ")));
    }
    let defaults = item_params::item_params(dict_id).map_or(&[][..], |p| p.default_modifications);
    if !defaults.is_empty() {
        let names: Vec<String> = defaults
            .iter()
            .map(|&id| match modifications.modification(id) {
                Some(m) => format!(
                    "{id} {} ({})",
                    game.label(false, id).0,
                    changes_of(&m.modifiers)
                ),
                None => id.to_string(),
            })
            .collect();
        push("default_modifications", Cell::Text(names.join("; ")));
    }
}

fn bundle_name(game: &Game, bundle_id: u8) -> String {
    game.dictionary
        .bundles
        .iter()
        .find(|bundle| bundle.id == bundle_id)
        .and_then(|bundle| config(game.data, &bundle.cfg))
        .and_then(|root| root.get("ui_descr"))
        .and_then(|desc| desc.get("text_descriptions"))
        .and_then(|desc| desc.get("name"))
        .and_then(ConfigValue::as_str)
        .and_then(english)
        .map_or_else(|| format!("bundle {bundle_id}"), str::to_owned)
}

/// The drop tables that can give `item` (`gameplay/drops/drops.options`
/// as the server rolls it, `survarium::v026g::drops`): its category in a
/// group with a weight, the item with a drop weight, no scope.
fn drop_tables(item: &Item) -> Vec<String> {
    let weight = item_params::item_params(item.dict_id).map_or(0, |p| p.drop_weight);
    if weight == 0 || item.category == SCOPE_CATEGORY {
        return Vec::new();
    }
    let drops = Drops::shared();
    let has = |table: &DropTable| {
        table
            .categories
            .iter()
            .any(|group| group.weight > 0 && group.categories.contains(&item.category))
    };
    [
        ("generic_drop", &drops.generic),
        ("new_player_drop", &drops.new_player),
        ("daily_bonus_drop", &drops.daily_bonus),
        ("drop_card_drop", &drops.drop_card),
    ]
    .into_iter()
    .filter(|(_, table)| has(table))
    .map(|(name, _)| name.to_owned())
    .collect()
}

/// The item's values with its `default_modifications` applied: on each
/// path they change, the values summed, then type 1 adds the sum and type 2
/// multiplies by max(0, 1 + sum) (`item_config_cook`, 0x2f1750); a path
/// the item lacks is skipped.
fn with_defaults(item: &Item, values: &Row, out: &mut Row) {
    let defaults =
        item_params::item_params(item.dict_id).map_or(&[][..], |p| p.default_modifications);
    let modifications = Modifications::shared();
    let mut sums: BTreeMap<(&str, u8), f64> = BTreeMap::new();
    for row in defaults
        .iter()
        .filter_map(|&id| modifications.modification(id))
    {
        for modifier in &row.modifiers {
            *sums
                .entry((modifier.path.as_str(), modifier.kind))
                .or_default() += float(modifier.value);
        }
    }
    let mut effective: BTreeMap<&str, f64> = BTreeMap::new();
    for ((path, kind), sum) in sums {
        let base = effective.get(path).copied().or_else(|| {
            values
                .iter()
                .find(|(column, _)| column == path)
                .and_then(|(_, cell)| cell.as_number())
        });
        let Some(base) = base else { continue };
        let value = if kind == 1 {
            base + sum
        } else {
            base * (1.0 + sum).max(0.0)
        };
        effective.insert(path, value);
    }
    for (path, value) in effective {
        // f32 arithmetic in the game: shown to its precision.
        out.push((
            format!("with_default_modifications/{path}"),
            Cell::number(float(value as f32)),
        ));
    }
}

/// An item's row: the dictionary's, the server's, then its config's.
fn item_row(game: &Game, item: &Item) -> Row {
    let mut row = Row::new();
    match game.dict_rows.get(&item.dict_id) {
        Some(dict) => flatten(game, "items_dict", dict, &mut row),
        None => row.push((
            "items_dict/dict_id".into(),
            Cell::number(f64::from(item.dict_id)),
        )),
    }
    derived(game, item, &mut row);
    let mut values = Row::new();
    if let Some(config) = config(game.data, &item.cfg_name) {
        flatten(game, "", config, &mut values);
    }
    with_defaults(item, &values, &mut row);
    row.extend(values);
    row
}

/// The item sheets, "All" first.
fn item_tables(game: &Game) -> Vec<Table> {
    let mut by_sheet: BTreeMap<usize, (String, Vec<Row>)> = BTreeMap::new();
    let mut all = Vec::new();
    for item in &game.dictionary.items {
        let row = item_row(game, item);
        let sheet = game.sheet_of(item);
        let place = ITEM_SHEETS
            .iter()
            .position(|name| *name == sheet)
            .unwrap_or(ITEM_SHEETS.len());
        by_sheet
            .entry(place)
            .or_insert_with(|| (sheet, Vec::new()))
            .1
            .push(row.clone());
        all.push(row);
    }
    let mut tables = vec![table("All", &LEAD, all, true, 5)];
    for (_, (name, rows)) in by_sheet {
        tables.push(table(&name, &LEAD, rows, true, 5));
    }
    tables
}

/// A sheet of a config's records (a list, or a table of one per key): a
/// row each, its key or index first.
fn records(
    game: &Game,
    name: &str,
    list: &ConfigValue,
    extra: impl Fn(&ConfigValue, &mut Row),
) -> Table {
    let mut rows = Vec::new();
    let entries: Vec<(String, &ConfigValue)> = match list {
        ConfigValue::Named(entries) => entries.iter().map(|(k, v)| (k.clone(), v)).collect(),
        ConfigValue::Indexed(values) => values
            .iter()
            .enumerate()
            .map(|(i, v)| (i.to_string(), v))
            .collect(),
        _ => Vec::new(),
    };
    for (key, value) in entries {
        let mut row: Row = vec![("key".into(), Cell::Text(key))];
        extra(value, &mut row);
        flatten(game, "", value, &mut row);
        rows.push(row);
    }
    table(name, &["key", "id", "name"], rows, false, 1)
}

/// A sheet of the values of `files` (paths of the data), a row each:
/// file, path, value, and the English text when the value is a text key.
fn key_values(game: &Game, name: &str, files: &[&str]) -> Table {
    let mut rows = Vec::new();
    for &file in files {
        let Some(root) = config(game.data, file) else {
            continue;
        };
        let mut values = Row::new();
        flatten(game, "", root, &mut values);
        for (path, value) in values {
            let mut row: Row = vec![
                ("file".into(), Cell::text(file)),
                ("path".into(), Cell::Text(path)),
            ];
            if let Cell::Text(text) = &value
                && let Some(english) = english(text)
            {
                row.push(("text_en".into(), Cell::text(english)));
            }
            row.push(("value".into(), value));
            rows.push(row);
        }
    }
    table(name, &["file", "path", "value", "text_en"], rows, false, 2)
}

const MODIFICATIONS_PATH: &str = survarium::v026g::modifications::MODIFICATIONS_PATH;

/// The modification and defect catalogues, their types, and the shop.
fn shared_tables(game: &Game) -> Vec<Table> {
    let mut tables = Vec::new();
    let modifications = Modifications::shared();
    let items = &game.dictionary.items;

    // Which items can roll each modification and get each defect.
    let mut can_roll: HashMap<u32, Vec<u16>> = HashMap::new();
    let mut can_get: HashMap<u32, Vec<u16>> = HashMap::new();
    for item in items {
        for m in modifications.candidates(item.dict_id, &[]) {
            can_roll.entry(m.id).or_default().push(item.dict_id);
        }
        for d in modifications.defect_candidates(item.dict_id, &[], &[]) {
            can_get.entry(d.id).or_default().push(item.dict_id);
        }
    }
    let names = |ids: Option<&Vec<u16>>| -> Vec<String> {
        ids.map(|ids| ids.iter().map(|&id| game.item_name(id)).collect())
            .unwrap_or_default()
    };

    if let Some(root) = config(game.data, MODIFICATIONS_PATH) {
        if let Some(list) = root.get("modifications") {
            tables.push(records(game, "Modifications", list, |row, out| {
                let id = row.get("id").and_then(ConfigValue::as_i32).unwrap_or(-1) as u32;
                let Some(m) = modifications.modification(id) else {
                    return;
                };
                let (en, uk) = game.label(false, id);
                out.push(("derived/name_en".into(), Cell::text(en)));
                out.push(("derived/name_uk".into(), Cell::text(uk)));
                out.push(("derived/type".into(), Cell::number(f64::from(m.kind))));
                out.push(("derived/rolls".into(), Cell::Bool(m.rolls())));
                out.push((
                    "derived/changes".into(),
                    Cell::Text(changes_of(&m.modifiers)),
                ));
                let ids = can_roll.get(&id);
                out.push((
                    "derived/items_that_can_roll_it".into(),
                    Cell::number(ids.map_or(0, Vec::len) as f64),
                ));
                let list = names(ids);
                if !list.is_empty() {
                    out.push(("derived/items".into(), Cell::Text(list.join("; "))));
                }
            }));
            tables.push(modification_types(game, &can_roll));
        }
        if let Some(list) = root.get("defects") {
            tables.push(records(game, "Defects", list, |row, out| {
                let id = row.get("id").and_then(ConfigValue::as_i32).unwrap_or(-1) as u32;
                let Some(d) = modifications.defect(id) else {
                    return;
                };
                let (en, uk) = game.label(true, id);
                out.push(("derived/name_en".into(), Cell::text(en)));
                out.push(("derived/name_uk".into(), Cell::text(uk)));
                out.push(("derived/type".into(), Cell::number(f64::from(d.kind))));
                out.push((
                    "derived/changes".into(),
                    Cell::Text(changes_of(&d.modifiers)),
                ));
                let ids = can_get.get(&id);
                out.push((
                    "derived/items_that_can_get_it".into(),
                    Cell::number(ids.map_or(0, Vec::len) as f64),
                ));
                let list = names(ids);
                if !list.is_empty() {
                    out.push(("derived/items".into(), Cell::Text(list.join("; "))));
                }
            }));
        }
        if let Some(list) = root.get("defect_slots") {
            tables.push(records(game, "Defect slots", list, |_, _| {}));
        }
    }
    tables.push(key_values(
        game,
        "Craft & reroll",
        &[
            survarium::v026g::modifications::CRAFT_PATH,
            survarium::v026g::modifications::REROLL_PATH,
        ],
    ));
    tables.push(shop(game));
    tables.push(bundles(game));
    tables.push(compatibility(game));
    if let Some(list) = config(game.data, "gameplay/hit_params/human_hit_params.options")
        .and_then(|root| root.get("hit_params"))
    {
        tables.push(records(game, "Hit params", list, |_, _| {}));
    }
    if let Some(list) =
        config(game.data, "gameplay/bodyparts/default").and_then(|root| root.get("body_parts"))
    {
        tables.push(records(game, "Body parts", list, |_, _| {}));
    }
    tables.push(factions(game));
    if let Some(list) = config(game.data, "gameplay/static_game_parameters.options")
        .and_then(|root| root.get("ui_properties"))
    {
        tables.push(records(game, "UI properties", list, |row, out| {
            for (key, column) in [
                ("prop_name", "derived/name_en"),
                ("postfix", "derived/postfix_en"),
            ] {
                if let Some(text) = row.get(key).and_then(ConfigValue::as_str).and_then(english) {
                    out.push((column.into(), Cell::text(text)));
                }
            }
            if let Some(text) = row
                .get("prop_name")
                .and_then(ConfigValue::as_str)
                .and_then(ukrainian)
            {
                out.push(("derived/name_uk".into(), Cell::text(text)));
            }
        }));
    }
    if let Some(list) = config(game.data, survarium::v026g::dictionary::PATH)
        .and_then(|root| root.get("match_levels_dict"))
    {
        tables.push(records(game, "Match levels", list, |_, _| {}));
    }
    tables.push(key_values(game, "Player", &[item_params::PLAYER_PATH]));
    let effects: Vec<&str> = game
        .data
        .paths()
        .filter(|path| path.starts_with("gameplay/effects/"))
        .collect();
    tables.push(key_values(game, "Effects", &effects));
    tables.push(key_values(game, "Drops", &[survarium::v026g::drops::PATH]));
    tables.push(key_values(
        game,
        "Skills",
        &["gameplay/skills/skills_tree.options"],
    ));
    // Every other gameplay file, value by value: nothing is left out.
    let item_configs: BTreeSet<&str> = items.iter().map(|item| item.cfg_name.as_str()).collect();
    let others: Vec<&str> = game
        .data
        .paths()
        .filter(|path| path.starts_with("gameplay/") && !item_configs.contains(path))
        .collect();
    tables.push(key_values(game, "Game files", &others));
    tables
}

/// What a modifier does, in words: `path +value` (type 1) or
/// `path x(1 + value)` (type 2).
/// What `modifiers` do, in words ([`change`]).
fn changes_of(modifiers: &[survarium::v026g::modifications::Modifier]) -> String {
    let changes: Vec<String> = modifiers
        .iter()
        .map(|modifier| change(&modifier.path, modifier.kind, modifier.value))
        .collect();
    changes.join("; ")
}

fn change(path: &str, kind: u8, value: f32) -> String {
    let value = float(value);
    match kind {
        1 => format!("{path} {value:+}"),
        2 => format!("{path} x(1{value:+})"),
        _ => format!("{path} type {kind} {value}"),
    }
}

/// A row for each modification type: its ids, names, and the range of
/// each value it changes (its five strengths), and who can roll it.
fn modification_types(game: &Game, can_roll: &HashMap<u32, Vec<u16>>) -> Table {
    let modifications = Modifications::shared();
    let mut kinds: BTreeMap<u8, Vec<&survarium::v026g::modifications::Modification>> =
        BTreeMap::new();
    for m in &modifications.modifications {
        kinds.entry(m.kind).or_default().push(m);
    }
    let mut rows = Vec::new();
    for (kind, list) in kinds {
        let mut row: Row = vec![("type".into(), Cell::number(f64::from(kind)))];
        let (en, uk) = game.label(false, list[0].id);
        row.push(("name_en".into(), Cell::text(en)));
        row.push(("name_uk".into(), Cell::text(uk)));
        let ids: Vec<String> = list.iter().map(|m| m.id.to_string()).collect();
        row.push(("ids".into(), Cell::Text(ids.join(", "))));
        let weights: Vec<String> = list
            .iter()
            .map(|m| float(m.drop_weight).to_string())
            .collect();
        row.push(("drop_weights".into(), Cell::Text(weights.join(", "))));
        row.push(("rolls".into(), Cell::Bool(list.iter().any(|m| m.rolls()))));
        let mut ranges: BTreeMap<(&str, u8), (f64, f64)> = BTreeMap::new();
        for m in &list {
            for modifier in &m.modifiers {
                let value = float(modifier.value);
                let range = ranges
                    .entry((modifier.path.as_str(), modifier.kind))
                    .or_insert((value, value));
                range.0 = range.0.min(value);
                range.1 = range.1.max(value);
            }
        }
        let text: Vec<String> = ranges
            .iter()
            .map(|(&(path, kind), &(low, high))| {
                let how = if kind == 1 { "add" } else { "x(1 + v)" };
                format!("{path} {how} {low}..{high}")
            })
            .collect();
        row.push(("ranges".into(), Cell::Text(text.join("; "))));
        if let Some((_, &(low, high))) = ranges.iter().next() {
            row.push(("min".into(), Cell::number(low)));
            row.push(("max".into(), Cell::number(high)));
        }
        let mut items: BTreeSet<u16> = BTreeSet::new();
        for m in &list {
            items.extend(can_roll.get(&m.id).into_iter().flatten());
        }
        row.push((
            "items_that_can_roll_it".into(),
            Cell::number(items.len() as f64),
        ));
        let names: Vec<String> = items.into_iter().map(|id| game.item_name(id)).collect();
        if !names.is_empty() {
            row.push(("items".into(), Cell::Text(names.join("; "))));
        }
        rows.push(row);
    }
    let lead = [
        "type",
        "name_en",
        "name_uk",
        "ids",
        "drop_weights",
        "rolls",
        "ranges",
        "min",
        "max",
        "items_that_can_roll_it",
        "items",
    ];
    table("Modification types", &lead, rows, false, 2)
}

/// The shop: every record of the price list the lobby is sent.
fn shop(game: &Game) -> Table {
    let mut rows = Vec::new();
    for record in shop_offers::price_list() {
        let item = game.dictionary.item(record.dict_id);
        let gold = item.is_some_and(|item| item.is_premium);
        let mut row: Row = vec![
            ("dict_id".into(), Cell::number(f64::from(record.dict_id))),
            (
                "item".into(),
                Cell::text(item.map_or("", |item| item.name.as_str())),
            ),
            (
                "name_en".into(),
                Cell::Text(item.map(|item| game.names(item).0).unwrap_or_default()),
            ),
            (
                "seller_id".into(),
                Cell::number(f64::from(record.faction_id)),
            ),
            (
                "seller".into(),
                Cell::Text(game.faction_name(record.faction_id)),
            ),
            ("rank".into(), Cell::number(f64::from(record.level))),
        ];
        if let Some(rank) = game.rank_name(record.faction_id, record.level) {
            row.push(("rank_name".into(), Cell::text(rank)));
        }
        row.push(("price".into(), Cell::number(f64::from(record.cost))));
        row.push((
            "currency".into(),
            Cell::text(if gold { "gold" } else { "silver" }),
        ));
        row.push((
            "unit".into(),
            Cell::number(f64::from(item_params::shop_unit(record.dict_id))),
        ));
        if let Some(item) = item {
            row.push(("base_cost".into(), Cell::number(f64::from(item.base_cost))));
        }
        rows.push(row);
    }
    let lead = [
        "dict_id",
        "item",
        "name_en",
        "seller_id",
        "seller",
        "rank",
        "rank_name",
        "price",
        "currency",
        "unit",
        "base_cost",
    ];
    table("Shop", &lead, rows, false, 3)
}

/// The bundles the premium shop sells: price and contents.
fn bundles(game: &Game) -> Table {
    let mut rows = Vec::new();
    for bundle in &shop_offers::BUNDLES {
        let contents: Vec<String> = bundle
            .contents(game.dictionary)
            .into_iter()
            .map(|(id, units, _)| format!("{units} x {}", game.item_name(id)))
            .collect();
        rows.push(vec![
            (
                "bundle_id".into(),
                Cell::number(f64::from(bundle.bundle_id)),
            ),
            (
                "name_en".into(),
                Cell::Text(bundle_name(game, bundle.bundle_id)),
            ),
            ("gold".into(), Cell::number(f64::from(bundle.gold))),
            ("contents".into(), Cell::Text(contents.join("; "))),
        ]);
    }
    table(
        "Bundles",
        &["bundle_id", "name_en", "gold", "contents"],
        rows,
        false,
        2,
    )
}

/// The dictionary's `items_compatibility` pairs (an ammo and a weapon).
fn compatibility(game: &Game) -> Table {
    let mut rows = Vec::new();
    for &(first, second) in &game.dictionary.compatibility {
        rows.push(vec![
            ("first_item".into(), Cell::number(f64::from(first))),
            ("first_name".into(), Cell::Text(game.item_name(first))),
            ("second_item".into(), Cell::number(f64::from(second))),
            ("second_name".into(), Cell::Text(game.item_name(second))),
        ]);
    }
    let lead = ["first_item", "first_name", "second_item", "second_name"];
    table("Compatibility", &lead, rows, false, 2)
}

/// The factions and their ranks: the reputation each needs.
fn factions(game: &Game) -> Table {
    let mut rows = Vec::new();
    for (&id, (name, levels)) in &game.factions {
        for (rank, (value, level_name)) in levels.iter().enumerate() {
            rows.push(vec![
                ("faction_id".into(), Cell::number(f64::from(id))),
                ("faction".into(), Cell::text(english(name).unwrap_or(name))),
                (
                    "faction_uk".into(),
                    Cell::text(ukrainian(name).unwrap_or("")),
                ),
                ("rank".into(), Cell::number(rank as f64)),
                ("reputation".into(), Cell::number(*value)),
                (
                    "rank_name".into(),
                    Cell::text(english(level_name).unwrap_or(level_name)),
                ),
                (
                    "rank_name_uk".into(),
                    Cell::text(ukrainian(level_name).unwrap_or("")),
                ),
            ]);
        }
    }
    let lead = [
        "faction_id",
        "faction",
        "faction_uk",
        "rank",
        "reputation",
        "rank_name",
        "rank_name_uk",
    ];
    table("Factions", &lead, rows, false, 2)
}

/// What decides a fight or a price but is not in the data: the exe's rules
/// (as the reverse engineering found them; the calculator follows them)
/// and the server's own.
const HARD_CODED: &[[&str; 4]] = &[
    [
        "Armour against piercing",
        "damage x (1 - clamp(armour - piercing, 0, 1))",
        "exe 0x329ff0 (body part hit)",
        "armour = the item's hit_params/<part>/hit_types/<type>/armor summed over the gear on that part; piercing above the armour gives no extra damage",
    ],
    [
        "Damage and piercing at the muzzle and fallen off",
        "Dmax = bullet_damage x k_damage; Dmin = bullet_damage x min_damage_amount x k_min_damage_amount x k_damage; Pmax = bullet_pierce x k_arp; Pmin = bullet_pierce x min_damage_amount x k_min_damage_amount x k_arp",
        "exe 0x2e1c80, 0x32ba60",
        "weapon parameters/* with the ammo's data/k_*",
    ],
    [
        "Fall-off by the bullet's speed",
        "between max_damage_distance x k_max_damage_distance and min_damage_distance x k_min_damage_distance, by speed: u = clamp((v - Vmin) / (Vmax - Vmin), 0, 1); D = Dmin + (Dmax - Dmin) x u (P alike)",
        "exe 0x32c520, 0x32ee90",
        "never rises again after a hit or an obstacle",
    ],
    [
        "Air drag constant",
        "1.1: the bullet slows for tc = 1 / (1.1 x air_resistance) s; speed at r metres = sqrt(v0^2 - 2 x air_resistance x v0 x r)",
        "exe 0x32c520",
        "ammo data/air_resistance, data/muzzle_speed",
    ],
    [
        "Damage types",
        "ranged 0, blunt 1, slash 2, explosive 3, toxic 4, radiation 5, fire 6, shock 7",
        "exe",
        "the hit_types keys of hit_params",
    ],
    [
        "Pools and routing",
        "a hit on a part goes to its pools: child damage = bdb_coeff x damage, piercing 0, armour and callbacks of the pool apply",
        "exe 0x33fa80",
        "gameplay/hit_params/human_hit_params.options hit_params/*/hit_types/*/bdb_coeff",
    ],
    [
        "One bullet, one pool",
        "a pool takes the largest share of one bullet, not the sum over the parts it crosses (last 64 bullets remembered)",
        "exe 0x329ff0",
        "",
    ],
    [
        "Modifications and defects",
        "values on one path summed; type 1 adds the sum, type 2 multiplies by max(0, 1 + sum); a path the item lacks is skipped",
        "exe item_config_cook 0x2f1750",
        "the with_default_modifications/* columns apply it",
    ],
    [
        "Character modifiers",
        "B x max(0, 1 + S), but added (B + S) for modifier indices 5, 10, 14, 15, 16, 17, 25, 28, 29, 31, 32, 37, 43, 44, 45, 46, 47, 48, 50, 53, 62, 64",
        "exe 0x2d5ae0",
        "skills, artefacts, drugs and boosters change these",
    ],
    [
        "Newcomer factor",
        "F = 1 + 0.5 x (0.5 + 0.1 x b) x max(0, 1 - 0.1 x n); damage x F(attacker) / F(victim)",
        "exe 0x332b70",
        "off on Skhid: the server sends n = 10 for every account (F = 1)",
    ],
    [
        "Second chance",
        "if damage > M54(Hmax) and damage > health: damage = health - M55(Hmax), once",
        "exe 0x332940",
        "modifiers 54 second_chance_damage, 55 second_chance_ramaining_health",
    ],
    [
        "Obstacle piercing",
        "q = 1 - (1 - clamp((P - B) / B, 0, 1))^2 of the bullet goes on; through a player, Dmin goes on",
        "exe 0x32d960",
        "B = the material's armour",
    ],
    [
        "Drug protection",
        "damage < threshold: 0; else (damage - threshold) x hit_coeff",
        "exe 0x33db30",
        "gameplay/items/drugs/* data/damage_protection",
    ],
    [
        "Onyx",
        "passive: (D + damage_add) x damage_mul, (P + armor_piercing_add) x armor_piercing_mul; active: D - min(absorb_total, D - (D + damage_add) x damage_mul)",
        "exe 0x312bf0; Skhid client patch 54",
        "patch 54: Onyx fired together are each active duration + (n - 1) x 1 s; only the first active Onyx reduces a hit",
    ],
    [
        "Spring",
        "passive/aim_time_mod and active/aim_time_mod are read as aim time; active/stopping_power_mod scales the slow from hits (-1: none); passive/max_carried_weight_mod is no longer read",
        "Skhid client patch 54 (loader 0x33ACF0, stagger 0x2DBD89)",
        "",
    ],
    [
        "Slow from hits (stagger)",
        "movement speed - min(sum of the stopping_power of the hits of the last 0.5 s, 0.5)",
        "exe 0x2DBCD0",
        "weapon parameters/stopping_power",
    ],
    [
        "Lifebone active",
        "removes hand and leg injuries; fills pain, body and head",
        "Skhid client patch 54 (0x3122AA)",
        "",
    ],
    [
        "Fall damage",
        "v = sqrt(2 g h); D = fatal_damage x (v - v(harmless_height)) / (v(fatal_height) - v(harmless_height)), at most fatal_damage, a blunt hit to each foot",
        "exe 0x2d8220",
        "gameplay/players/default.player player/fall_params; modifier 30 fall_damage",
    ],
    [
        "Selling",
        "a quarter of the game's base_cost, x condition for gear",
        "lobby (InventoryList.sellObject) and server",
        "srp-026g/crates/survarium/src/v026g/prices.rs",
    ],
    [
        "Repair",
        "(1 - condition) x repair_cost_ratio x base_cost, plus the defects' fix_cost",
        "lobby (MessageRepair.init) and server",
        "srp-026g/crates/survarium/src/v026g/prices.rs",
    ],
    [
        "Shop rank",
        "Renaissance and Edge: item_level - 5; others: item_level; exceptions and ammo, drugs, grenades by the server's table; ammo needs no rank",
        "server",
        "srp-026g/crates/survarium/src/v026g/prices.rs (EXCEPTIONS, AMMO_NEEDS_RANK); the derived/shop_* columns and the Shop sheet are the result",
    ],
    [
        "Shop prices",
        "base_cost, but ammo (about half, per clip) and the Vityaz, Mosin, PPSh-41 and TT-33 at the owner's prices",
        "server",
        "srp-026g/crates/survarium/src/v026g/prices.rs (AMMO_PRICES, OWNER_PRICES); the Shop sheet",
    ],
    [
        "Modification rolls",
        "one modification of a type at most, three in all; picked by drop_weight among the allowed types; never one that only changes wear",
        "server",
        "srp-026g/crates/survarium/src/v026g/modifications.rs; derived/can_roll_* columns",
    ],
    [
        "Drops",
        "a category group by weight (category 0 is no drop), then an item by drop_weight at an item level of the match level's row; scopes never drop",
        "server",
        "srp-026g/crates/survarium/src/v026g/drops.rs; derived/can_drop",
    ],
];

fn hard_coded() -> Table {
    let rows = HARD_CODED
        .iter()
        .map(|[what, rule, place, note]| {
            vec![
                ("what".into(), Cell::text(*what)),
                ("rule".into(), Cell::text(*rule)),
                ("where".into(), Cell::text(*place)),
                ("note".into(), Cell::text(*note)),
            ]
        })
        .collect();
    table(
        "Hard-coded",
        &["what", "rule", "where", "note"],
        rows,
        false,
        1,
    )
}

/// What the generated columns are.
const DERIVED: &[(&str, &str)] = &[
    (
        "derived/name_en",
        "the item's name in English (its ui_desc/text_descriptions/name)",
    ),
    ("derived/name_uk", "the item's name in Ukrainian"),
    (
        "derived/sheet",
        "the sheet the item is on (by its config's folder)",
    ),
    (
        "derived/status",
        "how a player gets it: sold (shop or bundle), drop only, found in matches, starting kit only, scope variant, or not obtainable",
    ),
    (
        "derived/shop_price",
        "what the shop takes for a step (ammo: a clip), first seller",
    ),
    ("derived/shop_currency", "silver or gold"),
    (
        "derived/shop_unit",
        "rounds in a step of the shop (1 for anything but ammo)",
    ),
    (
        "derived/shop_min_rank",
        "the lowest rank with a seller that buying needs",
    ),
    ("derived/shop_sellers", "every seller and its rank"),
    ("derived/bundles", "the premium bundles that hold it"),
    ("derived/can_drop", "whether a match's drop can give it"),
    (
        "derived/drop_tables",
        "the tables of gameplay/drops/drops.options that can give it",
    ),
    ("derived/starting_kit", "a new account gets it"),
    (
        "derived/not_sold",
        "why no shop sells it (the owner's choice)",
    ),
    (
        "derived/unreleased",
        "it comes with a patch not released yet",
    ),
    (
        "derived/compatible_items",
        "items_compatibility: the ammo a weapon fires, the weapons an ammo fits",
    ),
    (
        "derived/scopes_that_fit",
        "the scope items that fit the weapon",
    ),
    ("derived/fits_weapons", "the weapons the scope fits"),
    (
        "derived/can_roll_modifications",
        "the modification ids an upgrade or a drop can give it",
    ),
    (
        "derived/can_roll_modification_types",
        "their types and names",
    ),
    ("derived/can_get_defects", "the defects it can get"),
    (
        "derived/default_modifications",
        "its fixed default modifications, named",
    ),
];

/// The modification paths the lobby names: a modification with one path
/// and the property ids of its card.
fn lobby_names(game: &Game) -> HashMap<String, BTreeSet<String>> {
    let mut names: HashMap<String, BTreeSet<String>> = HashMap::new();
    let Some(root) = config(game.data, MODIFICATIONS_PATH) else {
        return names;
    };
    for key in ["modifications", "defects"] {
        for row in root.get(key).map(ConfigValue::children).unwrap_or_default() {
            let paths: BTreeSet<String> = row
                .get("modifiers")
                .map(ConfigValue::children)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|modifier| {
                    let path: Vec<&str> = modifier
                        .get("path")?
                        .children()
                        .into_iter()
                        .filter_map(ConfigValue::as_str)
                        .collect();
                    Some(path.join("/"))
                })
                .collect();
            let props: Vec<String> = row
                .get("ui_desc")
                .and_then(|desc| desc.get("props_list"))
                .map(ConfigValue::children)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|prop| prop.get("prop_id").and_then(ConfigValue::as_i32))
                .filter_map(|id| game.properties.get(&id))
                .filter(|prop| !prop.name_en.is_empty())
                .map(|prop| prop.name_en.clone())
                .collect();
            if paths.len() == 1 && !props.is_empty() {
                let path = paths.into_iter().next().unwrap_or_default();
                names.entry(path).or_default().extend(props);
            }
        }
    }
    names
}

/// "Columns": every column of the item sheets, what it is, the sheets
/// that have it and how many items have a value there.
fn columns(game: &Game, items: &[Table]) -> Table {
    let lobby = lobby_names(game);
    let mut seen: Vec<String> = Vec::new();
    let mut sheets: HashMap<String, Vec<String>> = HashMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for table in items {
        for (i, column) in table.columns.iter().enumerate() {
            let filled = table.rows.iter().filter(|row| row[i].is_some()).count();
            if table.name == "All" {
                seen.push(column.clone());
                counts.insert(column.clone(), filled);
            } else if filled > 0 {
                sheets
                    .entry(column.clone())
                    .or_default()
                    .push(table.name.clone());
            }
        }
    }
    let props: HashMap<String, &Property> = game
        .properties
        .iter()
        .map(|(id, prop)| (id.to_string(), prop))
        .collect();
    let mut rows = Vec::new();
    for column in seen {
        let description = if let Some(&(_, text)) = DERIVED.iter().find(|(name, _)| *name == column)
        {
            text.to_owned()
        } else if column.starts_with("items_dict/") {
            "the item's row of items_dict (gameplay/db_static_dictionaries)".to_owned()
        } else if let Some(path) = column.strip_prefix("with_default_modifications/") {
            let named = lobby.get(path).map(|names| {
                format!(
                    " ({})",
                    names.iter().cloned().collect::<Vec<_>>().join(", ")
                )
            });
            format!(
                "{path}{} with the item's default_modifications applied",
                named.unwrap_or_default()
            )
        } else if let Some(rest) = column.strip_prefix("ui_desc/props_list/") {
            let id = rest.split(' ').next().unwrap_or(rest);
            match props.get(id) {
                Some(prop) => format!(
                    "the lobby card's property {id}: {} / {}{}",
                    prop.name_en,
                    prop.name_uk,
                    if prop.postfix.is_empty() {
                        String::new()
                    } else {
                        format!(", in {}", prop.postfix)
                    }
                ),
                None => format!("the lobby card's property {id}"),
            }
        } else if let Some(names) = lobby.get(&column) {
            format!(
                "the lobby calls it {}",
                names.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        } else {
            String::new()
        };
        let mut row: Row = vec![("column".into(), Cell::text(column.as_str()))];
        row.push(("description".into(), Cell::Text(description)));
        row.push((
            "items".into(),
            Cell::number(counts.get(&column).copied().unwrap_or(0) as f64),
        ));
        if let Some(list) = sheets.get(&column) {
            let text = if list.len() + 1 == items.len() {
                "every item sheet".to_owned()
            } else {
                list.join(", ")
            };
            row.push(("sheets".into(), Cell::Text(text)));
        }
        rows.push(row);
    }
    table(
        "Columns",
        &["column", "description", "items", "sheets"],
        rows,
        false,
        1,
    )
}

/// "About": what the workbook was made from and how to read it.
fn about(game: &Game, items: usize) -> Table {
    let lines = [
        ("Made from", game.data.made_of()),
        (
            "Revision",
            revision(game.data).to_string(),
        ),
        ("Items", items.to_string()),
        (
            "Item sheets",
            "one row per item of items_dict (gameplay/db_static_dictionaries); a column per value of the item's config, by its path in the config".into(),
        ),
        (
            "Lists",
            "a list of numbers is a column per index (path/0, path/1, ...); a list of texts is one cell".into(),
        ),
        (
            "derived/...",
            "the server's reading of the data: shop, bundles, drops, starting kit, what fits, what can roll".into(),
        ),
        (
            "with_default_modifications/...",
            "a value with the item's default_modifications applied, as the game applies them".into(),
        ),
        (
            "ui_desc/props_list/<id> (<name>)",
            "the lobby card's numbers, named as the lobby names them".into(),
        ),
        (
            "Game files",
            "every value of every other gameplay/ file: file, path, value".into(),
        ),
        (
            "Hard-coded",
            "rules in the game's exe or the server's code, not in the data".into(),
        ),
        (
            "Columns",
            "what each item column is".into(),
        ),
    ];
    let rows = lines
        .into_iter()
        .map(|(what, text)| {
            vec![
                ("what".into(), Cell::text(what)),
                ("value".into(), Cell::Text(text)),
            ]
        })
        .collect();
    table("About", &["what", "value"], rows, false, 1)
}

/// The newest patch the data was made with (0: the stock data).
fn revision(data: &GameData) -> u32 {
    data.provenance
        .patches
        .iter()
        .map(|patch| patch.number)
        .max()
        .unwrap_or(0)
}

/// Every sheet, in the workbook's order.
pub(crate) fn tables() -> Vec<Table> {
    let game = Game::read();
    let items = item_tables(&game);
    let mut tables = vec![about(&game, game.dictionary.items.len())];
    let columns = columns(&game, &items);
    tables.extend(items);
    tables.extend(shared_tables(&game));
    tables.push(hard_coded());
    tables.push(columns);
    tables
}

/// The workbook of `tables`.
pub(crate) fn write(tables: &[Table]) -> Result<Vec<u8>, XlsxError> {
    let mut workbook = Workbook::new();
    let header = Format::new().set_bold().set_background_color("#E8E4DA");
    for table in tables {
        assert!(
            table.columns.len() <= MAX_COLUMNS,
            "{}: {} columns",
            table.name,
            table.columns.len()
        );
        let sheet = workbook.add_worksheet();
        sheet.set_name(&table.name)?;
        for (c, name) in table.columns.iter().enumerate() {
            sheet.write_string_with_format(0, c as u16, name, &header)?;
        }
        for (r, row) in table.rows.iter().enumerate() {
            let r = r as u32 + 1;
            for (c, cell) in row.iter().enumerate() {
                let c = c as u16;
                match cell {
                    None => {}
                    Some(Cell::Number(value)) => {
                        sheet.write_number(r, c, *value)?;
                    }
                    Some(Cell::Bool(value)) => {
                        sheet.write_boolean(r, c, *value)?;
                    }
                    Some(Cell::Text(text)) if text.is_empty() => {}
                    Some(Cell::Text(text)) => {
                        let text = if text.len() > CELL_TEXT {
                            let mut end = CELL_TEXT;
                            while !text.is_char_boundary(end) {
                                end -= 1;
                            }
                            &text[..end]
                        } else {
                            text.as_str()
                        };
                        sheet.write_string(r, c, text)?;
                    }
                }
            }
        }
        let columns = table.columns.len().max(1) as u16;
        for c in 0..columns.min(table.frozen) {
            sheet.set_column_width(c, 22)?;
        }
        sheet.set_freeze_panes(1, table.frozen)?;
        sheet.autofilter(0, 0, table.rows.len() as u32, columns - 1)?;
    }
    workbook.save_to_buffer()
}

/// The workbook of the server's game data, made once.
pub(crate) fn workbook() -> &'static Result<Vec<u8>, String> {
    static WORKBOOK: OnceLock<Result<Vec<u8>, String>> = OnceLock::new();
    WORKBOOK.get_or_init(|| {
        let started = std::time::Instant::now();
        let made = write(&tables()).map_err(|cause| cause.to_string());
        match &made {
            Ok(bytes) => log::info!(
                "items workbook: {} bytes in {} ms",
                bytes.len(),
                started.elapsed().as_millis()
            ),
            Err(cause) => log::error!("items workbook: {cause}"),
        }
        made
    })
}

/// The file name a download gets: `skhid-items-r<revision>.xlsx`.
fn file_name() -> String {
    format!("skhid-items-r{}.xlsx", revision(game_data::shared()))
}

/// Sends the workbook (made at the first request, or at start).
pub(crate) fn send(stream: &mut TcpStream, head_only: bool) {
    let head = match workbook() {
        Ok(bytes) => format!(
            "HTTP/1.0 200 OK\r\nContent-Type: {CONTENT_TYPE}\r\nContent-Length: {}\r\n\
             Content-Disposition: attachment; filename=\"{}\"\r\n\
             Cache-Control: no-cache\r\nX-Content-Type-Options: nosniff\r\n\
             Connection: close\r\n\r\n",
            bytes.len(),
            file_name()
        ),
        Err(_) => "HTTP/1.0 500 Internal Server Error\r\nContent-Length: 0\r\n\
                   Cache-Control: no-store\r\nConnection: close\r\n\r\n"
            .to_owned(),
    };
    let _ = stream.write_all(head.as_bytes());
    if let (Ok(bytes), false) = (workbook(), head_only) {
        let _ = stream.write_all(bytes);
    }
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Write);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn all(tables: &[Table]) -> &Table {
        tables.iter().find(|table| table.name == "All").unwrap()
    }

    #[test]
    fn every_item_of_the_dictionary_has_a_row() {
        let tables = tables();
        let dictionary = Dictionary::shared();
        let all = all(&tables);
        assert_eq!(all.rows.len(), dictionary.items.len());
        let ids: BTreeSet<u64> = (0..all.rows.len())
            .map(|r| {
                all.cell(r, "items_dict/dict_id")
                    .unwrap()
                    .as_number()
                    .unwrap() as u64
            })
            .collect();
        assert_eq!(ids.len(), dictionary.items.len());
        // Each item on one item sheet too.
        let on_sheets: usize = tables
            .iter()
            .filter(|table| {
                ITEM_SHEETS.contains(&table.name.as_str()) || table.name.starts_with("Armour ")
            })
            .map(|table| table.rows.len())
            .sum();
        assert_eq!(on_sheets, dictionary.items.len());
    }

    #[test]
    fn every_weapon_has_its_recoil_keys() {
        let tables = tables();
        let weapons = tables.iter().find(|table| table.name == "Weapons").unwrap();
        assert!(weapons.rows.len() > 20);
        let data = game_data::shared();
        for r in 0..weapons.rows.len() {
            let Some(Cell::Text(cfg)) = weapons.cell(r, "items_dict/cfg_name") else {
                panic!("row {r} has no cfg_name");
            };
            let config = game_data::config(data, cfg).unwrap().unwrap();
            let recoil = config
                .get("recoil")
                .unwrap_or_else(|| panic!("{cfg}: no recoil"));
            assert!(recoil.entries().len() >= 10, "{cfg}");
            for (key, value) in recoil.entries() {
                let cell = weapons
                    .cell(r, &format!("recoil/{key}"))
                    .unwrap_or_else(|| panic!("{cfg}: no recoil/{key}"));
                assert_eq!(cell.as_number(), number_of(value), "{cfg} recoil/{key}");
            }
            for column in [
                "parameters/recoil_modifier",
                "dispersion/base_dispersion",
                "recoil_animations",
            ] {
                assert!(weapons.cell(r, column).is_some(), "{cfg}: no {column}");
            }
        }
    }

    #[test]
    fn numbers_are_numbers() {
        let tables = tables();
        let all = all(&tables);
        let ak = (0..all.rows.len())
            .find(|&r| all.cell(r, "items_dict/name") == Some(&Cell::text("ak_74m")))
            .unwrap();
        assert_eq!(
            all.cell(ak, "parameters/rounds_per_minute"),
            Some(&Cell::Number(650.0))
        );
        assert_eq!(
            all.cell(ak, "parameters/bullet_pierce")
                .and_then(Cell::as_number),
            Some(0.3)
        );
        assert_eq!(all.cell(ak, "derived/status"), Some(&Cell::text("sold")));
        // The card's numbers, named as the lobby names them.
        assert!(
            all.columns
                .iter()
                .any(|column| column.starts_with("ui_desc/props_list/4 (")),
        );
    }

    #[test]
    fn the_file_is_a_workbook_of_those_sheets() {
        let tables = tables();
        let bytes = write(&tables).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut workbook = String::new();
        zip.by_name("xl/workbook.xml")
            .unwrap()
            .read_to_string(&mut workbook)
            .unwrap();
        for table in &tables {
            let name = table.name.replace('&', "&amp;");
            assert!(workbook.contains(&format!("name=\"{name}\"")), "{name}");
        }
        // "All" (the second sheet): a header and a row per item.
        let mut sheet = String::new();
        zip.by_name("xl/worksheets/sheet2.xml")
            .unwrap()
            .read_to_string(&mut sheet)
            .unwrap();
        assert_eq!(
            sheet.matches("<row ").count(),
            Dictionary::shared().items.len() + 1
        );
    }
}

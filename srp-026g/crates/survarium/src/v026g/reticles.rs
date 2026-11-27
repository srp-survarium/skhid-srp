//! A collimator's reticle, chosen per scope item (`docs/notes/reticle-choice.md`, "The build's
//! contract"): a pattern, a size and a colour from the scope's own lists, and a brightness of
//! 0 to 10. The lists are the game data's: the table `data/srp_reticle` of a scope item's config
//! (category [`SCOPE_CATEGORY`]), which an edit list adds. A scope item without it has no choice.
//! Nothing here names an item: any scope with the table has one.
//!
//! A number from a request or the database becomes a [`Reticle`] here alone, through
//! [`ReticleLists::reticle`], so a held reticle always indexes its scope's lists. The choice is
//! drawn only in its holder's first-person aimed view: nothing here changes what a match
//! computes, nor what a game writes into a world.

use std::collections::BTreeMap;

use binary_config_parser::tree::{ConfigError, ConfigValue};

use super::dictionary::Dictionary;
use super::game_data::{GameData, GameDataError};
use super::scopes::SCOPE_CATEGORY;

/// The table's key in a scope item's config, under `data`.
pub const KEY: &str = "srp_reticle";
/// The most patterns, sizes and colours a scope lists (the client's table holds these).
pub const MAX_PATTERNS: usize = 16;
pub const MAX_SIZES: usize = 8;
pub const MAX_COLOURS: usize = 8;
/// The longest name of a pattern or a colour (the client keeps 24 bytes with the zero).
pub const NAME_LIMIT: usize = 23;
/// The longest model path (the client keeps 96 bytes with the zero).
pub const MODEL_LIMIT: usize = 95;
/// The most scope items with lists the game data may have (the client keeps the lists of this
/// many scope types).
pub const MAX_SCOPES: usize = 32;
/// The marker in the top byte of a `46 05` value that sets a reticle ('R').
pub const VALUE_MARK: u8 = 0x52;

/// A pattern's or a colour's name: ASCII `[a-z0-9_]`, 1 to [`NAME_LIMIT`] bytes. The window's text
/// is `st_srp_reticle_pattern_<name>` or `st_srp_reticle_colour_<name>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name(String);

impl Name {
    pub fn parse(text: &str) -> Result<Name, ConfigError> {
        let fits = (1..=NAME_LIMIT).contains(&text.len())
            && text.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !fits {
            return Err(ConfigError(format!(
                "the name `{text}` is not 1 to {NAME_LIMIT} of [a-z0-9_]"
            )));
        }
        Ok(Name(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A reticle model's path in the game's resources: printable ASCII without spaces, 1 to
/// [`MODEL_LIMIT`] bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model(String);

impl Model {
    pub fn parse(text: &str) -> Result<Model, ConfigError> {
        let fits = (1..=MODEL_LIMIT).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_graphic());
        if !fits {
            return Err(ConfigError(format!(
                "the model `{text}` is not 1 to {MODEL_LIMIT} printable ASCII bytes"
            )));
        }
        Ok(Model(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One pattern: its name and its model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub name: Name,
    pub model: Model,
}

/// A size: a factor on the model's own size, finite and above 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Size(f32);

impl Size {
    pub fn parse(factor: f32) -> Result<Size, ConfigError> {
        if !(factor.is_finite() && factor > 0.0 && factor <= 16.0) {
            return Err(ConfigError(format!("the size {factor} is not above 0 and at most 16")));
        }
        Ok(Size(factor))
    }

    pub fn get(self) -> f32 {
        self.0
    }
}

/// One colour: its name and its emission, each channel finite, 0 to 16.
#[derive(Clone, Debug, PartialEq)]
pub struct Colour {
    pub name: Name,
    pub rgb: [f32; 3],
}

/// A brightness, 0 to 10 (0.69d0's, `reticle-choice.md`); 6 the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Brightness(u8);

impl Brightness {
    pub const MAX: u8 = 10;
    pub const DEFAULT: Brightness = Brightness(6);

    pub fn new(value: u8) -> Option<Brightness> {
        (value <= Self::MAX).then_some(Brightness(value))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// A scope's reticle: indices into its [`ReticleLists`] and a brightness. Made only by
/// [`ReticleLists::reticle`] (or [`Reticle::DEFAULT`], which every list holds).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Reticle {
    pattern: u8,
    size: u8,
    colour: u8,
    brightness: Brightness,
}

impl Reticle {
    /// The first of each list and brightness 6: what the optic shows until its owner chooses.
    pub const DEFAULT: Reticle = Reticle {
        pattern: 0,
        size: 0,
        colour: 0,
        brightness: Brightness::DEFAULT,
    };

    pub fn pattern(self) -> u8 {
        self.pattern
    }
    pub fn size(self) -> u8 {
        self.size
    }
    pub fn colour(self) -> u8 {
        self.colour
    }
    pub fn brightness(self) -> Brightness {
        self.brightness
    }

    /// Its four numbers.
    pub fn numbers(self) -> Numbers {
        Numbers {
            pattern: self.pattern,
            size: self.size,
            colour: self.colour,
            brightness: self.brightness.0,
        }
    }

    /// The `46 05` value that asks for it ([`Numbers::of_value`]).
    pub fn value(self) -> u64 {
        self.numbers().value()
    }
}

/// A reticle's four numbers as a request or the database gives them, not yet held against a
/// scope's lists ([`ReticleLists::reticle`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Numbers {
    pub pattern: u8,
    pub size: u8,
    pub colour: u8,
    pub brightness: u8,
}

impl Numbers {
    /// The numbers of a `46 05` value: `pattern | size << 8 | colour << 16 | brightness << 24 |
    /// 0x52 << 56`, the bytes 4 to 6 zero. None for any other value.
    pub fn of_value(value: u64) -> Option<Numbers> {
        let [pattern, size, colour, brightness, 0, 0, 0, VALUE_MARK] = value.to_le_bytes() else {
            return None;
        };
        Some(Numbers {
            pattern,
            size,
            colour,
            brightness,
        })
    }

    pub fn value(self) -> u64 {
        u64::from_le_bytes([
            self.pattern,
            self.size,
            self.colour,
            self.brightness,
            0,
            0,
            0,
            VALUE_MARK,
        ])
    }
}

/// A scope's lists (`data/srp_reticle`): each 1 to its most, index 0 the default.
#[derive(Clone, Debug, PartialEq)]
pub struct ReticleLists {
    patterns: Vec<Pattern>,
    sizes: Vec<Size>,
    colours: Vec<Colour>,
}

impl ReticleLists {
    /// The lists of the table `srp_reticle`:
    ///
    /// ```text
    /// patterns: [ { name: "dot", model: "weapons/reticles_069/dot" }, ... ]   1..16
    /// sizes:    [ 1.0, 0.75, 0.5, 0.35 ]                                       1..8
    /// colours:  [ { name: "red", rgb: [1.0, 0.0, 0.0] }, ... ]                1..8
    /// ```
    ///
    /// `rgb` a float3 or a list of three numbers. Anything else stops srp at start: the client
    /// would show what srp could not name.
    pub fn from_config(table: &ConfigValue) -> Result<ReticleLists, ConfigError> {
        let patterns = list(table, "patterns", MAX_PATTERNS, |entry| {
            Ok(Pattern {
                name: Name::parse(entry.str_at("name")?)?,
                model: Model::parse(entry.str_at("model")?)?,
            })
        })?;
        let sizes = list(table, "sizes", MAX_SIZES, |entry| {
            Size::parse(
                entry
                    .as_f32()
                    .ok_or_else(|| ConfigError("a size is not a number".into()))?,
            )
        })?;
        let colours = list(table, "colours", MAX_COLOURS, |entry| {
            let rgb = match entry.req("rgb")? {
                ConfigValue::Float3(rgb) => *rgb,
                ConfigValue::Indexed(values) => match values.as_slice() {
                    [r, g, b] => [r, g, b].map(|channel| channel.as_f32().unwrap_or(f32::NAN)),
                    _ => [f32::NAN; 3],
                },
                _ => [f32::NAN; 3],
            };
            if !rgb.iter().all(|c| c.is_finite() && (0.0..=16.0).contains(c)) {
                return Err(ConfigError("`rgb` is not three numbers from 0 to 16".into()));
            }
            Ok(Colour {
                name: Name::parse(entry.str_at("name")?)?,
                rgb,
            })
        })?;
        Ok(ReticleLists {
            patterns,
            sizes,
            colours,
        })
    }

    pub fn patterns(&self) -> &[Pattern] {
        &self.patterns
    }
    pub fn sizes(&self) -> &[Size] {
        &self.sizes
    }
    pub fn colours(&self) -> &[Colour] {
        &self.colours
    }

    /// The reticle of `numbers`, when each index is in its list and the brightness 0 to 10.
    pub fn reticle(&self, numbers: Numbers) -> Option<Reticle> {
        let within = |index: u8, len: usize| usize::from(index) < len;
        (within(numbers.pattern, self.patterns.len())
            && within(numbers.size, self.sizes.len())
            && within(numbers.colour, self.colours.len()))
        .then_some(())?;
        Some(Reticle {
            pattern: numbers.pattern,
            size: numbers.size,
            colour: numbers.colour,
            brightness: Brightness::new(numbers.brightness)?,
        })
    }

    /// Whether `reticle` indexes these lists (one made against other lists may not).
    pub fn holds(&self, reticle: Reticle) -> bool {
        self.reticle(reticle.numbers()) == Some(reticle)
    }
}

/// The entries of `table.key`, a list of 1 to `most`, each read by `read`.
fn list<T>(
    table: &ConfigValue,
    key: &str,
    most: usize,
    read: impl Fn(&ConfigValue) -> Result<T, ConfigError>,
) -> Result<Vec<T>, ConfigError> {
    let entries = match table.req(key)? {
        ConfigValue::Indexed(entries) => entries,
        _ => return Err(ConfigError(format!("`{key}` is no list"))),
    };
    if !(1..=most).contains(&entries.len()) {
        return Err(ConfigError(format!(
            "`{key}` has {} entries: 1 to {most}",
            entries.len()
        )));
    }
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            read(entry).map_err(|cause| ConfigError(format!("`{key}[{index}]`: {}", cause.0)))
        })
        .collect()
}

/// Every scope item's lists, by its dict id: the scope items (category [`SCOPE_CATEGORY`]) whose
/// config has `data/srp_reticle`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reticles {
    lists: BTreeMap<u16, ReticleLists>,
}

impl Reticles {
    /// The players' game data's ([`super::game_data`]), read once; checked at start
    /// ([`super::tables::check`]).
    pub fn shared() -> &'static Reticles {
        static SHARED: std::sync::OnceLock<Reticles> = std::sync::OnceLock::new();
        SHARED.get_or_init(|| {
            Self::of_data(super::game_data::shared(), Dictionary::shared())
                .expect("the scopes' reticle lists read (checked at start)")
        })
    }

    /// The lists of `data`'s scope items. A malformed table, or more than [`MAX_SCOPES`] scope
    /// items with one, is an error.
    pub fn of_data(data: &GameData, dictionary: &Dictionary) -> Result<Reticles, GameDataError> {
        let mut lists = BTreeMap::new();
        for item in dictionary.items.iter().filter(|item| item.category == SCOPE_CATEGORY) {
            let path = item.cfg_name.as_str();
            let Some(config) = super::game_data::config(data, path)? else {
                continue;
            };
            let Some(table) = config.get("data").and_then(|data| data.get(KEY)) else {
                continue;
            };
            let read = ReticleLists::from_config(table).map_err(|cause| {
                GameDataError(format!("{path}: `data.{KEY}`: {}", cause.0))
            })?;
            lists.insert(item.dict_id, read);
        }
        Self::of_lists(lists).map_err(GameDataError)
    }

    /// `lists` by dict id: at most [`MAX_SCOPES`].
    pub fn of_lists(lists: BTreeMap<u16, ReticleLists>) -> Result<Reticles, String> {
        if lists.len() > MAX_SCOPES {
            return Err(format!(
                "{} scope items have `data.{KEY}`: the client keeps the lists of {MAX_SCOPES}",
                lists.len()
            ));
        }
        Ok(Reticles { lists })
    }

    /// Every scope item's lists, by dict id.
    pub fn iter(&self) -> impl Iterator<Item = (u16, &ReticleLists)> + '_ {
        self.lists.iter().map(|(&dict_id, lists)| (dict_id, lists))
    }

    /// The lists of the scope item `dict_id`, if it has any.
    pub fn of(&self, dict_id: u16) -> Option<&ReticleLists> {
        self.lists.get(&dict_id)
    }

    pub fn len(&self) -> usize {
        self.lists.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lists.is_empty()
    }
}

#[cfg(any(test, feature = "fixtures"))]
pub mod fixtures {
    //! Lists made up for tests.

    use super::*;

    fn named(entries: Vec<(&str, ConfigValue)>) -> ConfigValue {
        ConfigValue::Named(
            entries
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect(),
        )
    }

    /// A table with `patterns` (each `weapons/reticles_069/<name>`), 0.69d0's four sizes and
    /// `colours` (each red, green or blue), as an edit list writes it.
    pub fn table(patterns: &[&str], colours: &[&str]) -> ConfigValue {
        let pattern = |name: &str| {
            named(vec![
                ("name", ConfigValue::String(name.to_owned())),
                (
                    "model",
                    ConfigValue::String(format!("weapons/reticles_069/{name}")),
                ),
            ])
        };
        let colour = |name: &str| {
            let rgb = match name {
                "red" => [1.0, 0.0, 0.0],
                "green" => [0.0, 1.0, 0.0],
                _ => [0.0, 0.0, 1.0],
            };
            named(vec![
                ("name", ConfigValue::String(name.to_owned())),
                ("rgb", ConfigValue::Float3(rgb)),
            ])
        };
        named(vec![
            (
                "patterns",
                ConfigValue::Indexed(patterns.iter().map(|name| pattern(name)).collect()),
            ),
            (
                "sizes",
                ConfigValue::Indexed(
                    [1.0, 0.75, 0.5, 0.35]
                        .into_iter()
                        .map(ConfigValue::Float)
                        .collect(),
                ),
            ),
            (
                "colours",
                ConfigValue::Indexed(colours.iter().map(|name| colour(name)).collect()),
            ),
        ])
    }

    /// The lists of [`table`].
    pub fn lists(patterns: &[&str], colours: &[&str]) -> ReticleLists {
        ReticleLists::from_config(&table(patterns, colours)).expect("the made-up lists read")
    }

    /// Reticles of `scopes`: each dict id with the dot and the cross, red and green.
    pub fn reticles(scopes: &[u16]) -> Reticles {
        Reticles::of_lists(
            scopes
                .iter()
                .map(|&dict| (dict, lists(&["dot", "ncstar_cross"], &["red", "green"])))
                .collect(),
        )
        .expect("few scopes")
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{lists, table};
    use super::*;

    #[test]
    fn the_lists_read_and_a_reticle_is_held_to_them() {
        let lists = lists(&["dot", "eotech_ring", "ncstar_cross"], &["red", "green"]);
        assert_eq!(lists.patterns().len(), 3);
        assert_eq!(lists.patterns()[1].name.as_str(), "eotech_ring");
        assert_eq!(
            lists.patterns()[1].model.as_str(),
            "weapons/reticles_069/eotech_ring"
        );
        assert_eq!(lists.sizes()[3].get(), 0.35);
        assert_eq!(lists.colours()[1].rgb, [0.0, 1.0, 0.0]);
        let numbers = |pattern, size, colour, brightness| Numbers {
            pattern,
            size,
            colour,
            brightness,
        };
        let reticle = lists.reticle(numbers(2, 3, 1, 10)).unwrap();
        assert_eq!(
            (reticle.pattern(), reticle.size(), reticle.colour(), reticle.brightness().get()),
            (2, 3, 1, 10)
        );
        assert!(lists.holds(reticle));
        assert!(lists.holds(Reticle::DEFAULT));
        assert_eq!(lists.reticle(numbers(3, 0, 0, 6)), None);
        assert_eq!(lists.reticle(numbers(0, 4, 0, 6)), None);
        assert_eq!(lists.reticle(numbers(0, 0, 2, 6)), None);
        assert_eq!(lists.reticle(numbers(0, 0, 0, 11)), None);
        // A reticle of longer lists is not held by shorter ones.
        let short = super::fixtures::lists(&["dot"], &["red"]);
        assert!(!short.holds(reticle));
    }

    #[test]
    fn a_value_is_a_reticle_only_with_its_mark() {
        let numbers = Numbers {
            pattern: 1,
            size: 2,
            colour: 3,
            brightness: 4,
        };
        assert_eq!(numbers.value(), 0x5200_0000_0403_0201);
        assert_eq!(Numbers::of_value(numbers.value()), Some(numbers));
        assert_eq!(Numbers::of_value(0x0000_0000_0403_0201), None);
        assert_eq!(Numbers::of_value(0x5200_0001_0403_0201), None);
        assert_eq!(Numbers::of_value(0), None);
        assert_eq!(Numbers::of_value(1), None);
        assert_eq!(Reticle::DEFAULT.value(), 0x5200_0000_0600_0000);
    }

    #[test]
    fn a_malformed_table_is_refused() {
        let refused = |table: ConfigValue| ReticleLists::from_config(&table).is_err();
        assert!(refused(table(&[], &["red"])));
        assert!(refused(table(&["dot"], &[])));
        assert!(refused(table(&["Dot"], &["red"])));
        assert!(refused(table(&["a_name_that_is_far_too_long"], &["red"])));
        let seventeen: Vec<String> = (0..17).map(|i| format!("p{i}")).collect();
        let seventeen: Vec<&str> = seventeen.iter().map(String::as_str).collect();
        assert!(refused(table(&seventeen, &["red"])));
        let sixteen = &seventeen[..16];
        assert!(!refused(table(sixteen, &["red"])));
        // rgb as a list of three numbers is taken too; two numbers are not.
        let rgb = |values: Vec<ConfigValue>| {
            let mut table = table(&["dot"], &["red"]);
            let ConfigValue::Named(entries) = &mut table else { unreachable!() };
            let colours = &mut entries.iter_mut().find(|(k, _)| k == "colours").unwrap().1;
            let ConfigValue::Indexed(colours) = colours else { unreachable!() };
            let ConfigValue::Named(colour) = &mut colours[0] else { unreachable!() };
            colour.retain(|(k, _)| k != "rgb");
            colour.push(("rgb".into(), ConfigValue::Indexed(values)));
            table
        };
        let lists = ReticleLists::from_config(&rgb(vec![
            ConfigValue::Float(0.5),
            ConfigValue::Integer(1),
            ConfigValue::Float(0.0),
        ]))
        .unwrap();
        assert_eq!(lists.colours()[0].rgb, [0.5, 1.0, 0.0]);
        assert!(refused(rgb(vec![ConfigValue::Float(0.5), ConfigValue::Float(1.0)])));
        assert!(refused(rgb(vec![
            ConfigValue::Float(-1.0),
            ConfigValue::Float(0.0),
            ConfigValue::Float(0.0)
        ])));
    }

    #[test]
    fn the_test_data_has_no_lists_and_too_many_scopes_are_refused() {
        // The test data's scope items have no `srp_reticle`: no choice for any.
        let none = Reticles::of_data(
            super::super::game_data::shared(),
            Dictionary::shared(),
        )
        .unwrap();
        assert!(none.is_empty());
        let many: BTreeMap<u16, ReticleLists> = (0..=MAX_SCOPES as u16)
            .map(|dict| (dict, lists(&["dot"], &["red"])))
            .collect();
        assert!(Reticles::of_lists(many).is_err());
    }
}

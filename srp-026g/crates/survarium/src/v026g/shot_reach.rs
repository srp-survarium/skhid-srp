//! How far a shot is heard by the weapon that fired it, from the game's own sound files.
//!
//! The chain, per weapon (the weapon config's `states.fire.sounds.third_view` is the shot as
//! another player hears it):
//!
//! ```text
//! wpn_<name>.options  states.fire.sounds.third_view entries (type 257)  "weapons/akm_n_m"
//!   -> sounds/composite/<it>.composite_sound                layers: filename, enabled, max_distance
//!   -> sounds/collections/<layer>.sound_collection          sound_items[0].filename
//!   -> sounds/single/<that>.raw_options                     spl_preset
//!   -> sounds/spl/<preset>.spl                              volume keys (distance, gain)
//! ```
//!
//! A layer is heard out to the smaller of its `max_distance` and its preset's cull (the farthest
//! distance of a volume key whose gain is 0; none: the layer's own `max_distance`), and the shot
//! reaches the farthest of the enabled layers whose chain is whole ([`Layer::heard_m`]): a layer
//! whose file the archives lack counts for nothing, so the reach is then a lower bound
//! ([`Cull::Missing`] says where). A weapon whose chain gives no layer has no reach.
//!
//! The server's game data does not carry the sound files, so the table is made offline from the
//! archives (`srp-shot-reach`) and kept as `shot_reach.tsv` in the bots; this module is the
//! derivation both the tool and its tests use.

use binary_config_parser::ConfigValue;

/// Reads a file of the archives by its path: its bytes, `None` if there is none.
pub type Read<'a> = &'a mut dyn FnMut(&str) -> Result<Option<Vec<u8>>, String>;

/// The path of a weapon's config in the archives.
pub fn config_path(stem: &str) -> String {
    format!("gameplay/items/weapons/{stem}.options")
}

/// A resource type of the shot's sounds in the weapon config: a composite sound (the others, 258,
/// are single collections: a click, a start; they are not the shot).
const COMPOSITE: i32 = 257;

/// Where a layer's volume curve reaches 0.
#[derive(Clone, Debug, PartialEq)]
pub enum Cull {
    /// At this distance, metres.
    At(f32),
    /// Never: the layer is heard as far as its own `max_distance`.
    Never,
    /// The chain stops at this file, which the archives do not have.
    Missing(String),
}

/// One enabled layer of a shot's composite sound.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub collection: String,
    pub max_m: f32,
    pub cull: Cull,
}

impl Layer {
    /// How far the layer is heard: the smaller of its own distance and its cull; `None` when the
    /// chain is broken.
    pub fn heard_m(&self) -> Option<f32> {
        match self.cull {
            Cull::At(cull) => Some(self.max_m.min(cull)),
            Cull::Never => Some(self.max_m),
            Cull::Missing(_) => None,
        }
    }
}

/// The enabled layers of the shot of the weapon whose config is `options`: those of every
/// composite sound its `third_view` names (none: it names none).
pub fn layers(options: &[u8], read: Read) -> Result<Vec<Layer>, String> {
    let config = ConfigValue::parse(options).map_err(|e| e.to_string())?;
    let composites: Vec<String> = config
        .get("states")
        .and_then(|s| s.get("fire"))
        .and_then(|f| f.get("sounds"))
        .and_then(|s| s.get("third_view"))
        .map(ConfigValue::children)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            let [kind, name] = entry.children()[..] else {
                return None;
            };
            (kind.as_i32() == Some(COMPOSITE)).then(|| name.as_str().map(str::to_owned))?
        })
        .collect();
    let mut out = Vec::new();
    for composite in composites {
        let path = format!("sounds/composite/{composite}.composite_sound");
        let Some(bytes) = read(&path)? else {
            continue;
        };
        let parsed = ConfigValue::parse(&bytes).map_err(|e| format!("{path}: {e}"))?;
        let items = parsed
            .get("composite_sound")
            .and_then(|c| c.get("sound_items"))
            .map(ConfigValue::children)
            .unwrap_or_default();
        for layer in items {
            if layer.get("enabled").and_then(ConfigValue::as_bool) != Some(true) {
                continue;
            }
            let (Some(file), Some(max)) = (
                layer.get("filename").and_then(ConfigValue::as_str),
                layer.get("max_distance").and_then(ConfigValue::as_f32),
            ) else {
                continue;
            };
            out.push(Layer {
                collection: file.to_owned(),
                max_m: max,
                cull: cull_of(file, read)?,
            });
        }
    }
    Ok(out)
}

/// The reach of a shot of `layers`: the farthest any layer is heard; `None` when no layer's chain
/// is whole.
pub fn farthest(layers: &[Layer]) -> Option<f32> {
    layers
        .iter()
        .filter_map(Layer::heard_m)
        .fold(None, |best: Option<f32>, m| Some(best.map_or(m, |b| b.max(m))))
}

/// The reach in metres of the shot of the weapon whose config is `options`, if its sound chain
/// gives one.
pub fn reach(options: &[u8], read: Read) -> Result<Option<f32>, String> {
    Ok(farthest(&layers(options, read)?))
}

/// The cull of the preset of the sound collection `collection`'s first sound.
fn cull_of(collection: &str, read: Read) -> Result<Cull, String> {
    let mut parsed = |path: String| -> Result<Result<ConfigValue, String>, String> {
        match read(&path)? {
            Some(bytes) => ConfigValue::parse(&bytes)
                .map(Ok)
                .map_err(|e| format!("{path}: {e}")),
            None => Ok(Err(path)),
        }
    };
    let collection = match parsed(format!("sounds/collections/{collection}.sound_collection"))? {
        Ok(c) => c,
        Err(path) => return Ok(Cull::Missing(path)),
    };
    let Some(first) = collection
        .get("collection")
        .and_then(|c| c.get("sound_items"))
        .and_then(|items| items.children().first().copied())
        .and_then(|item| item.get("filename"))
        .and_then(ConfigValue::as_str)
    else {
        return Ok(Cull::Missing("a sound in the collection".to_owned()));
    };
    let options = match parsed(format!("sounds/single/{first}.raw_options"))? {
        Ok(o) => o,
        Err(path) => return Ok(Cull::Missing(path)),
    };
    let Some(preset) = options
        .get("options")
        .and_then(|o| o.get("spl_preset"))
        .and_then(ConfigValue::as_str)
        .filter(|p| !p.is_empty())
    else {
        return Ok(Cull::Missing(format!("an spl_preset in sounds/single/{first}.raw_options")));
    };
    let spl = match parsed(format!("sounds/spl/{preset}.spl"))? {
        Ok(s) => s,
        Err(path) => return Ok(Cull::Missing(path)),
    };
    let keys = spl.get("volume").map(ConfigValue::entries).unwrap_or_default();
    let cull = keys
        .iter()
        .filter_map(|(_, key)| match key.get("position") {
            Some(&ConfigValue::Float2([distance, gain])) if gain <= 1e-6 => Some(distance),
            _ => None,
        })
        .fold(None, |best: Option<f32>, d| Some(best.map_or(d, |b| b.max(d))));
    Ok(cull.map_or(Cull::Never, Cull::At))
}

/// The table's rows, `name<TAB>metres`, one a line, sorted by name; a name with no reach is left
/// out. The metres are whole when they are.
pub fn rows(weapons: &[(String, Option<f32>)]) -> String {
    let mut rows: Vec<&(String, Option<f32>)> = weapons.iter().filter(|(_, r)| r.is_some()).collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::new();
    for (name, reach) in rows {
        out.push_str(&format!("{name}\t{}\n", reach.unwrap_or_default()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn named(entries: Vec<(&str, ConfigValue)>) -> ConfigValue {
        ConfigValue::Named(entries.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    }

    fn string(s: &str) -> ConfigValue {
        ConfigValue::String(s.to_owned())
    }

    /// A weapon config whose shot is the composite `composite`.
    fn weapon(composite: &str) -> Vec<u8> {
        let third_view = ConfigValue::Indexed(vec![ConfigValue::Indexed(vec![
            ConfigValue::Integer(257),
            string(composite),
        ])]);
        named(vec![(
            "states",
            named(vec![(
                "fire",
                named(vec![("sounds", named(vec![("third_view", third_view)]))]),
            )]),
        )])
        .to_bytes()
        .unwrap()
    }

    /// A composite sound of layers `(collection, enabled, max_distance)`.
    fn composite(layers: &[(&str, bool, i32)]) -> Vec<u8> {
        let items = layers
            .iter()
            .map(|&(file, enabled, max)| {
                named(vec![
                    ("filename", string(file)),
                    ("enabled", ConfigValue::Boolean(enabled)),
                    ("max_distance", ConfigValue::Integer(max)),
                ])
            })
            .collect();
        named(vec![("composite_sound", named(vec![("sound_items", ConfigValue::Indexed(items))]))])
            .to_bytes()
            .unwrap()
    }

    fn collection(first: &str) -> Vec<u8> {
        let item = named(vec![("filename", string(first))]);
        named(vec![("collection", named(vec![("sound_items", ConfigValue::Indexed(vec![item]))]))])
            .to_bytes()
            .unwrap()
    }

    fn raw_options(preset: &str) -> Vec<u8> {
        named(vec![("options", named(vec![("spl_preset", string(preset))]))])
            .to_bytes()
            .unwrap()
    }

    /// A volume curve of `(distance, gain)` keys.
    fn spl(keys: &[(f32, f32)]) -> Vec<u8> {
        let keys = keys
            .iter()
            .enumerate()
            .map(|(i, &(d, g))| {
                (format!("key{i}"), named(vec![("position", ConfigValue::Float2([d, g]))]))
            })
            .collect();
        named(vec![("volume", ConfigValue::Named(keys))]).to_bytes().unwrap()
    }

    /// The archives' files by path.
    fn files(list: &[(&str, Vec<u8>)]) -> BTreeMap<String, Vec<u8>> {
        list.iter().map(|(p, b)| ((*p).to_owned(), b.clone())).collect()
    }

    fn reach_in(weapon: &[u8], files: &BTreeMap<String, Vec<u8>>) -> Option<f32> {
        let mut read = |path: &str| Ok::<_, String>(files.get(path).cloned());
        reach(weapon, &mut read).unwrap()
    }

    #[test]
    fn a_shot_reaches_the_farthest_enabled_layer_to_its_cull() {
        let files = files(&[
            (
                "sounds/composite/w/m.composite_sound",
                // loud: cull 30 under max 150 -> 30; far: max 100 under cull 120 -> 100;
                // off: the disabled layer's 300 does not count.
                composite(&[("loud", true, 150), ("far", true, 100), ("off", false, 300)]),
            ),
            ("sounds/collections/loud.sound_collection", collection("a")),
            ("sounds/collections/far.sound_collection", collection("b")),
            ("sounds/single/a.raw_options", raw_options("near")),
            ("sounds/single/b.raw_options", raw_options("far")),
            ("sounds/spl/near.spl", spl(&[(0.0, 1.0), (10.0, 0.4), (30.0, 0.0)])),
            ("sounds/spl/far.spl", spl(&[(0.0, 1.0), (60.0, 0.2), (120.0, 0.0)])),
        ]);
        assert_eq!(reach_in(&weapon("w/m"), &files), Some(100.0));
    }

    #[test]
    fn a_curve_that_never_reaches_zero_leaves_the_layers_own_distance() {
        let files = files(&[
            ("sounds/composite/w/m.composite_sound", composite(&[("loud", true, 85)])),
            ("sounds/collections/loud.sound_collection", collection("a")),
            ("sounds/single/a.raw_options", raw_options("wide")),
            ("sounds/spl/wide.spl", spl(&[(0.0, 1.0), (500.0, 0.1)])),
        ]);
        assert_eq!(reach_in(&weapon("w/m"), &files), Some(85.0));
    }

    #[test]
    fn a_layer_whose_chain_is_broken_is_skipped_and_none_left_is_no_reach() {
        let files = files(&[
            ("sounds/composite/w/m.composite_sound", composite(&[("loud", true, 85)])),
            ("sounds/collections/loud.sound_collection", collection("a")),
            ("sounds/single/a.raw_options", raw_options("gone")),
        ]);
        assert_eq!(reach_in(&weapon("w/m"), &files), None);
        // No composite at all, and a config with no fire sound.
        assert_eq!(reach_in(&weapon("w/other"), &files), None);
        assert_eq!(reach_in(&named(vec![]).to_bytes().unwrap(), &files), None);
    }

    #[test]
    fn rows_are_sorted_and_leave_out_the_silent() {
        let weapons = [
            ("wpn_b".to_owned(), Some(150.0)),
            ("wpn_a".to_owned(), Some(55.5)),
            ("wpn_c".to_owned(), None),
        ];
        assert_eq!(rows(&weapons), "wpn_a\t55.5\nwpn_b\t150\n");
    }
}

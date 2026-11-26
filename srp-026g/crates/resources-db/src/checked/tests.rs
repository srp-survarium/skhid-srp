// SPDX-License-Identifier: GPL-3.0-or-later
use std::collections::BTreeMap;

use binary_config_parser::tree::ConfigValue::{self, Indexed, Named};

use super::*;

/// Files by path, read as they are.
#[derive(Default)]
struct Memory(BTreeMap<String, Vec<u8>>);

impl Memory {
    fn with(mut self, path: &str, bytes: impl Into<Vec<u8>>) -> Self {
        self.0.insert(path.to_owned(), bytes.into());
        self
    }
}

impl Files for Memory {
    fn has(&self, path: &str) -> bool {
        self.0.contains_key(path)
    }

    fn under(&self, prefix: &str) -> Vec<String> {
        self.0
            .keys()
            .filter(|path| path.starts_with(prefix))
            .cloned()
            .collect()
    }

    fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, Error> {
        Ok(self.0.get(path).cloned())
    }
}

fn named(entries: &[(&str, ConfigValue)]) -> ConfigValue {
    Named(
        entries
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
    )
}

fn text(value: &str) -> ConfigValue {
    ConfigValue::String(value.to_owned())
}

fn bytes(config: ConfigValue) -> Vec<u8> {
    config.to_bytes().unwrap()
}

/// A project whose one vegetation object has `layers` of (file, models),
/// and a light, which names no file.
fn project(layers: &[(&str, &[&str])]) -> Vec<u8> {
    let layers = layers
        .iter()
        .map(|(file, models)| {
            let models = models
                .iter()
                .map(|model| named(&[("name", text(model)), ("probability", ConfigValue::Float(1.0))]))
                .collect();
            named(&[("filename", text(file)), ("model_def", Indexed(models))])
        })
        .collect();
    bytes(named(&[
        ("server_objects", Indexed(vec![])),
        (
            "client_objects",
            Indexed(vec![
                named(&[("game_object_type", text("light"))]),
                named(&[("game_object_type", text("vegetation")), ("layers", Indexed(layers))]),
            ]),
        ),
    ]))
}

fn model_settings(materials: &[&str]) -> Vec<u8> {
    let surfaces = materials
        .iter()
        .enumerate()
        .map(|(index, material)| {
            (format!("surface_{index}"), named(&[("material_name", text(material))]))
        })
        .collect();
    bytes(named(&[("material_settings", Named(surfaces))]))
}

fn material(textures: &[(&str, &str)]) -> Vec<u8> {
    let parameters: Vec<(String, ConfigValue)> = textures
        .iter()
        .map(|(parameter, texture)| ((*parameter).to_owned(), named(&[("value", text(texture))])))
        .chain([("alpha_ref".to_owned(), named(&[("value", ConfigValue::Float(0.5))]))])
        .collect();
    bytes(named(&[(
        "material",
        named(&[("g_stage", named(&[("effect", Named(parameters))]))]),
    )]))
}

/// level_01/day with one vegetation layer of one model, its material with a
/// diffuse texture (and its options) and an empty bump; level_02/day with no
/// vegetation; `startup.cfg`; and files no layer names.
fn game() -> Memory {
    Memory::default()
        .with(
            "projects/level_01/day/client_project",
            project(&[("vegetation/green.veg", &["level/01/grass_1"])]),
        )
        .with("projects/level_01/vegetation/green.veg", b"positions".to_vec())
        .with("projects/level_01/vegetation/unused.veg", b"other".to_vec())
        .with("projects/level_02/day/client_project", project(&[]))
        .with(
            "models/level/01/grass_1.model/settings",
            model_settings(&["flora/grass_l01", "flora/grass_l01"]),
        )
        .with("models/level/01/grass_1.model/render/lod0/converted_model", b"mesh".to_vec())
        .with(
            "material_instances/flora/grass_l01.material",
            material(&[("texture_diffuse", "flora/grass_l01"), ("texture_bump", "")]),
        )
        .with("textures/flora/grass_l01.dds", b"dds".to_vec())
        .with("textures/flora/grass_l01.options", b"options".to_vec())
        .with("textures/flora/tree.dds", b"tree".to_vec())
        .with("startup.cfg", b"r_grass_lod1_distance 40.0\n".to_vec())
}

#[test]
fn a_project_checks_what_its_vegetation_names_and_every_match_startup_cfg() {
    let checked = Checked::of(&mut game()).unwrap();
    let paths: Vec<&str> = checked.paths_of("level_01/day").collect();
    assert_eq!(
        paths,
        [
            "startup.cfg",
            "material_instances/flora/grass_l01.material",
            "models/level/01/grass_1.model/render/lod0/converted_model",
            "models/level/01/grass_1.model/settings",
            "projects/level_01/day/client_project",
            "projects/level_01/vegetation/green.veg",
            "textures/flora/grass_l01.dds",
            "textures/flora/grass_l01.options",
        ]
    );
    let paths: Vec<&str> = checked.paths_of("level_02/day").collect();
    assert_eq!(paths, ["startup.cfg", "projects/level_02/day/client_project"]);
    // A project the data does not have checks startup.cfg alone.
    assert_eq!(checked.paths_of("level_99/day").collect::<Vec<_>>(), ["startup.cfg"]);
    assert_eq!(
        checked.files["textures/flora/grass_l01.dds"],
        Sum::of(b"dds").unwrap()
    );
    assert!(!checked.files.contains_key("textures/flora/tree.dds"));
    assert!(!checked.files.contains_key("projects/level_01/vegetation/unused.veg"));
}

#[test]
fn a_layer_file_or_material_the_archive_lacks_is_left_out() {
    let mut game = game().with(
        "projects/level_03/day/client_project",
        project(&[("vegetation/gone.veg", &["level/03/none", "level/01/grass_1"])]),
    );
    game.0.remove("textures/flora/grass_l01.options");
    let checked = Checked::of(&mut game).unwrap();
    let paths: Vec<&str> = checked.paths_of("level_03/day").collect();
    assert_eq!(
        paths,
        [
            "startup.cfg",
            "material_instances/flora/grass_l01.material",
            "models/level/01/grass_1.model/render/lod0/converted_model",
            "models/level/01/grass_1.model/settings",
            "projects/level_03/day/client_project",
            "textures/flora/grass_l01.dds",
        ]
    );
}

#[test]
fn a_settings_or_material_that_is_no_config_is_checked_alone() {
    let mut files = game()
        .with("models/level/01/grass_1.model/settings", b"not a config".to_vec());
    let checked = Checked::of(&mut files).unwrap();
    let paths: Vec<&str> = checked.paths_of("level_01/day").collect();
    assert!(paths.contains(&"models/level/01/grass_1.model/settings"));
    assert!(!paths.contains(&"material_instances/flora/grass_l01.material"));

    let mut files = game()
        .with("material_instances/flora/grass_l01.material", b"not a config".to_vec());
    let checked = Checked::of(&mut files).unwrap();
    let paths: Vec<&str> = checked.paths_of("level_01/day").collect();
    assert!(paths.contains(&"material_instances/flora/grass_l01.material"));
    assert!(!paths.contains(&"textures/flora/grass_l01.dds"));
}

#[test]
fn a_project_that_is_no_config_stops_the_data() {
    let mut game = game().with("projects/level_04/day/client_project", b"garbage".to_vec());
    let error = Checked::of(&mut game).unwrap_err().to_string();
    assert!(error.contains("projects/level_04/day/client_project"), "{error}");
}

#[test]
fn the_text_reads_back_as_it_was() {
    let checked = Checked::of(&mut game()).unwrap();
    let text = checked.text();
    assert!(text.starts_with("skhid checked files 1\nfile\t"), "{text}");
    assert_eq!(Checked::parse(&text).unwrap(), checked);
}

#[test]
fn a_text_unlike_the_format_is_refused() {
    let checked = Checked::of(&mut game()).unwrap();
    let good = checked.text();
    for bad in [
        good.replacen(FORMAT, "skhid checked files 2", 1),
        format!("{good}project\tlevel_01/day\tno/such/file\n"),
        format!("{good}always\tno/such/file\n"),
        format!("{good}file\tstartup.cfg\t1\t{}\n", "00".repeat(32)),
        format!("{good}file\tx\tbig\t{}\n", "00".repeat(32)),
        format!("{good}file\tx\t1\t{}\n", "0".repeat(63)),
        format!("{good}file\tx\t1\t{}\n", "AB".repeat(32)),
        format!("{good}what\tx\n"),
    ] {
        assert!(Checked::parse(&bad).is_err(), "{bad}");
    }
}

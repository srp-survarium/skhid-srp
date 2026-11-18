//! The hidden Keeper anomaly every map gets (content-edit/artefact-keeper-1.json, made by
//! scripts/artefact-keeper/make_edit.py; docs/notes/artefact-skills.md): a pool of
//! `keeper::POOL` dormant artefacts of each of `keeper::KINDS`, nothing that spawns, hits or shows.
use artefact_carry::keeper::{KINDS, POOL};
use binary_config_parser::BinaryConfig;
use binary_config_parser::tree::ConfigValue;
use mapdata::project::MapData;

fn edit() -> serde_json::Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/content-edit/artefact-keeper-1.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_hidden_anomaly_is_the_keeper_pool_and_nothing_else() {
    let edit = edit();
    let entries = edit["append_entry"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    let entry = &entries[0];
    let files = entry["files"].as_array().unwrap();
    assert_eq!(files.len(), entry["expected_counts"].as_array().unwrap().len());
    assert!(files.iter().all(|f| {
        let f = f.as_str().unwrap();
        f.starts_with("projects/") && f.ends_with("/client_project") && !f.starts_with("projects/lobby_scene/")
    }));
    assert_eq!((entry["list"].as_str(), entry["vectors"].as_bool()), (Some("client_objects"), Some(true)));
    let anomaly = &entry["value"];
    // The pool: the table's kinds in draw order, POOL each.
    let pool: Vec<(u64, u64)> = anomaly["artefacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["dict_id"].as_u64().unwrap(), a["count"].as_u64().unwrap()))
        .collect();
    let want: Vec<(u64, u64)> = KINDS.iter().map(|k| (u64::from(k.dict_id()), u64::from(POOL))).collect();
    assert_eq!(pool, want);
    // As a map's object: its pool cooked (enabled), no container, no zone, energy off.
    let project = serde_json::json!({
        "levelmap_min": [-10.0, 0.0, -10.0],
        "levelmap_max": [10.0, 1.0, 10.0],
        "collision_objects": [],
        "server_objects": {
            "respawn_points": [
                { "position": [1.0, 2.0, 3.0], "rotation": [0.0, 1.0, 0.0], "team": 1, "priority": 2, "point_id": 5, "layer": "default" },
            ],
        },
        "client_objects": [anomaly],
    });
    let config = BinaryConfig::parse_json(&project);
    let root = ConfigValue::parse(config.as_bytes()).unwrap();
    let map = MapData::from_config("test/day_4", &root).unwrap();
    assert_eq!(map.anomalies.len(), 1);
    let parsed = &map.anomalies[0];
    assert!(parsed.artefacts.enabled);
    assert_eq!((parsed.artefacts.containers, parsed.artefacts.max_count), (0, 0));
    assert_eq!(parsed.artefacts.chance, 0.0);
    assert!(parsed.zones.is_empty());
    assert!(!parsed.energy.enabled);
    assert_eq!(parsed.states.len(), 1);
    assert!(parsed.states[0].enabled && parsed.states[0].groups.is_empty());
    assert_eq!(parsed.states[0].threshold, 0.0);
}

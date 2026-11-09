//! Keeper of the Artifacts' hidden pool (artefact-keeper-1.json) as the server's world validation
//! reads it: on the game data a release builds, every map with a project has the pool's anomaly
//! (no container, 20 artefacts of each Keeper kind), the world's artefacts table stays within the
//! core world's 128, and an untouched world of the map's objects passes `srp_validate::world::
//! objects` as an uploaded one is checked.
use survarium::v026g::{game_data, maps};
use srp_validate::world::{Context, Held, Objects, Section};

/// The pool: Larkspur, Onyx and Spring, 20 each (`artefact_carry::keeper::POOL`).
const POOL: usize = 60;
/// The core world's artefacts vector, 128 inline entries (+1894Ch).
const CORE_ARTEFACTS: usize = 128;

#[test]
#[ignore = "requires SRP_GAME_DATA: a server-game-data.db that release/build-content made with artefact-keeper-1.json"]
fn every_maps_world_with_the_keeper_pool_passes_the_objects_check() {
    let path = std::env::var_os(game_data::GAME_DATA_ENV).expect("SRP_GAME_DATA");
    game_data::init_from(std::path::Path::new(&path)).unwrap();
    let mut checked = 0;
    for map in maps::all().iter().filter(|map| map.has_project) {
        let objects = map.world_objects;
        let pools = objects
            .anomalies
            .iter()
            .filter(|anomaly| anomaly.artefacts.is_some_and(|a| a.containers == 0 && a.artefacts.len() == POOL))
            .count();
        assert!(pools >= 1, "{}: no Keeper pool", map.project);
        assert!(objects.artefacts() <= CORE_ARTEFACTS, "{}: {} artefacts", map.project, objects.artefacts());
        let held = [Held::Grenade, Held::Trap];
        let bytes = maps::untouched_objects(held, objects);
        let objects = Objects { held: &held, map: objects };
        let context = Context {
            time: 0,
            places: 1,
            game_mode: 2,
            batteries: 0,
            respawn_points: None,
            profiles: None,
            artefacts: None,
            objects: Some(&objects),
            inventory: None,
        };
        let section = Section { at: 0, bytes: &bytes };
        assert_eq!(srp_validate::world::objects(&section, &context), Ok(()), "{}", map.project);
        checked += 1;
    }
    assert!(checked > 100, "{checked} maps");
}

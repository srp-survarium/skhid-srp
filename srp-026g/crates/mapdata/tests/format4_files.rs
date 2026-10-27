//! The map files written again by this build (format 4) against the ones written before: the same
//! map but for what format 4 added. Needs both folders of the game's content:
//! `F4_OLD=<dir> F4_NEW=<dir> cargo test -p mapdata --test format4_files -- --ignored --nocapture`.

use mapdata::MapData;

#[test]
#[ignore = "needs the private map files (F4_OLD, F4_NEW)"]
fn the_new_files_are_the_old_ones_and_the_zones_part_names() {
    let old = std::path::PathBuf::from(std::env::var_os("F4_OLD").expect("set F4_OLD"));
    let new = std::path::PathBuf::from(std::env::var_os("F4_NEW").expect("set F4_NEW"));
    let mut names: Vec<_> = std::fs::read_dir(&old)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name())
        .filter(|n| n.to_string_lossy().ends_with(".mapdata"))
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no .mapdata files in {}", old.display());
    let (mut files, mut zones, mut named) = (0, 0, 0);
    let mut distinct = std::collections::BTreeMap::<String, usize>::new();
    for name in names {
        let before = MapData::from_bytes(&std::fs::read(old.join(&name)).unwrap()).unwrap();
        let mut after = MapData::from_bytes(&std::fs::read(new.join(&name)).unwrap())
            .unwrap_or_else(|e| panic!("{name:?}: {e}"));
        for zone in &after.zones {
            if let Some(damage) = &zone.damage {
                zones += 1;
                assert_eq!(
                    damage.part_names.len(),
                    usize::from(damage.parts),
                    "{name:?} {}: the names are the parts",
                    zone.name
                );
                named += usize::from(!damage.part_names.is_empty());
                for part in &damage.part_names {
                    *distinct.entry(part.clone()).or_default() += 1;
                }
            }
        }
        for zone in &mut after.zones {
            if let Some(damage) = &mut zone.damage {
                damage.part_names.clear();
            }
        }
        // A file of format 2 had no artefact cores.
        if before.artefact_cores.is_empty() {
            after.artefact_cores.clear();
        }
        assert!(before == after, "{name:?} differs beyond the part names");
        files += 1;
    }
    eprintln!("{files} files equal; {zones} zones with a hit, {named} name their parts");
    eprintln!("part names: {distinct:?}");
}

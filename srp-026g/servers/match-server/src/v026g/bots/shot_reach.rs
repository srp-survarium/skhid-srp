//! How far a shot is heard by the weapon that fired it (H1c).
//!
//! The game's own number, per weapon (PROVEN data): the weapon config's `states/fire/sounds/
//! third_view` names a composite sound whose layers carry a `max_distance`; each layer's sound
//! (a collection, or a single sound) names a `spl_preset` whose `.spl` volume curve reaches 0 at
//! its cull. A layer is heard out to min(its `max_distance`, its preset's cull) and the shot
//! reaches the largest over the weapon's enabled layers: 55 m (the suppressed: VSS, VAL, A9-91,
//! VSK-94), 100 m (pistols, most shotguns, UZI), 150 m (the AK family, SKS, SVD, SMGs) and 200 m
//! (the bolt rifles); and 19.3 m (FN P90 and MP7, whose layers all end there) and 240 m (the
//! Neostead, Saiga-12 and Vepr-12: Rudnya's distant one-shot environment layer, which the other
//! shotguns' composites do not have). All 83 weapons of the live archives have a chain that
//! resolves to the end (`srp-shot-reach --check` says `partial` otherwise).
//!
//! The table is `shot_reach.tsv`, made by `srp-shot-reach` from the game's archives
//! (`survarium::v026g::shot_reach` is the derivation, read from the files), keyed by the weapon's
//! config stem (`wpn_ak_74u` of `gameplay/items/weapons/wpn_ak_74u.options`, the dictionary
//! item's `cfg_name`). The server's game data carries no sound files, so a new or patched weapon
//! gets its row when the tool is run on the release's archives (`srp-shot-reach <archives>
//! --check shot_reach.tsv` says what is missing or differs).

/// The table's text: `name TAB metres` a line, sorted by name.
const TABLE: &str = include_str!("shot_reach.tsv");

/// The shot's reach in metres by weapon config stem, sorted by name.
pub fn table() -> &'static [(&'static str, f32)] {
    static ROWS: std::sync::OnceLock<Vec<(&'static str, f32)>> = std::sync::OnceLock::new();
    ROWS.get_or_init(|| {
        TABLE
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| {
                // name, the general metres, and the with-environment metres where they differ.
                let mut fields = line.split('\t');
                let name = fields.next().expect("a row of shot_reach.tsv");
                let metres = fields.next().expect("metres in shot_reach.tsv");
                (name, metres.parse().expect("metres in shot_reach.tsv"))
            })
            .collect()
    })
}

/// The reach of the weapon whose config is `cfg_name` (a path or a stem), if
/// the table has it.
pub fn reach_of(cfg_name: &str) -> Option<f32> {
    let stem = cfg_name.rsplit('/').next().unwrap_or(cfg_name);
    let stem = stem.strip_suffix(".options").unwrap_or(stem);
    let rows = table();
    rows.binary_search_by(|(name, _)| name.cmp(&stem))
        .ok()
        .map(|i| rows[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_finds_a_weapon_by_path_or_stem() {
        assert!(table().windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(reach_of("gameplay/items/weapons/wpn_ak_74u.options"), Some(150.0));
        assert_eq!(reach_of("wpn_vss"), Some(55.0));
        assert_eq!(reach_of("wpn_spas_12"), Some(100.0));
        assert_eq!(reach_of("wpn_nothing"), None);
    }

    /// Every weapon any kit of any tier and role can carry has a row: a weapon added to a kit
    /// without one fails here, named (run `srp-shot-reach` on the archives for its row).
    #[test]
    fn every_weapon_of_the_bots_kits_has_a_reach() {
        use crate::v026g::bots::guns::Arms;
        use survarium::v026g::loadouts::{BotEquipment, BotRole, BotTier, RusherKit, RusherKits};
        let mut missing = std::collections::BTreeSet::new();
        for tier in BotTier::ALL {
            for kits in RusherKit::ALL.map(RusherKits::both).into_iter().chain([RusherKits::default()]) {
                for role in BotRole::ALL {
                    for arms in Arms::kits_of(BotEquipment::of(tier, kits), role) {
                        let item = arms.primary.0.item;
                        if reach_of(&format!("wpn_{item}")).is_none() {
                            missing.insert(item);
                        }
                    }
                }
            }
        }
        assert!(missing.is_empty(), "no row in shot_reach.tsv for {missing:?}");
    }

    /// With the live game data (`SRP_GAME_DATA`, the server's file) every weapon of the
    /// dictionary has a row, named when it has none.
    #[test]
    #[ignore = "needs SRP_GAME_DATA, the server's game data"]
    fn every_weapon_of_the_live_game_data_has_a_reach() {
        survarium::v026g::game_data::init().expect("SRP_GAME_DATA");
        let silent: Vec<String> = survarium::v026g::weapons::all()
            .iter()
            .filter(|w| reach_of(&w.name).is_none())
            .map(|w| w.name.to_owned())
            .collect();
        assert!(silent.is_empty(), "no row in shot_reach.tsv for {silent:?}");
    }

    #[test]
    fn the_reaches_are_a_few_hundred_metres_at_most() {
        assert!(table().iter().all(|&(_, m)| m > 0.0 && m <= 300.0), "{:?}", table());
    }
}

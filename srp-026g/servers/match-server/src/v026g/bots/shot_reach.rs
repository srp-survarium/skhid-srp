//! How far a shot is heard by the weapon that fired it (H1c).
//!
//! The game's own number, per weapon (PROVEN data; `weapon_hearing2.py`
//! of the batteries record): the weapon config's `states/fire/sounds/
//! third_view` names a composite sound whose layers carry a `min_distance`
//! and `max_distance`; each layer's sound collection names a `spl_preset`
//! whose `.spl` volume curve reaches 0 at its cull. A layer is heard out to
//! min(its `max_distance`, its preset's cull) and the shot reaches the
//! largest over the weapon's enabled layers: 55 m (the suppressed: VSS, VAL,
//! A9-91, VSK-94), 100 m (pistols, shotguns, UZI), 150 m (the AK family,
//! SKS, SVD, SMGs) and 200 m (the bolt rifles). A few layers had no preset
//! found (the record's last column), so a value is a lower bound there.
//!
//! The table is keyed by the weapon's config stem (`wpn_ak_74u` of
//! `gameplay/items/weapons/wpn_ak_74u.options`, the dictionary item's
//! `cfg_name`).

/// The shot's reach in metres by weapon config stem, sorted by name.
pub const SHOT_REACH_M: &[(&str, f32)] = &[
    ("wpn_a545", 150.0),
    ("wpn_a9_91", 55.0),
    ("wpn_ak_12", 150.0),
    ("wpn_ak_74m", 150.0),
    ("wpn_ak_74n", 150.0),
    ("wpn_ak_74n_premium", 150.0),
    ("wpn_ak_74u", 150.0),
    ("wpn_akmn", 150.0),
    ("wpn_aps", 100.0),
    ("wpn_ash_12", 150.0),
    ("wpn_benelli_m4", 100.0),
    ("wpn_fort_17", 100.0),
    ("wpn_izh_27_lancaster", 100.0),
    ("wpn_izh_27_lancaster_premium", 100.0),
    ("wpn_kiparis", 150.0),
    ("wpn_kiparis_premium", 150.0),
    ("wpn_mosin_nagant", 200.0),
    ("wpn_mp_153", 100.0),
    ("wpn_pm", 100.0),
    ("wpn_pp2000", 150.0),
    ("wpn_ppsh", 150.0),
    ("wpn_rem_700", 200.0),
    ("wpn_rem_870", 100.0),
    ("wpn_rpd", 150.0),
    ("wpn_rpk", 150.0),
    ("wpn_sks", 150.0),
    ("wpn_socom16", 150.0),
    ("wpn_spas_12", 100.0),
    ("wpn_sr2m", 150.0),
    ("wpn_sr3m", 150.0),
    ("wpn_sr3m_premium", 150.0),
    ("wpn_sv_98", 200.0),
    ("wpn_svd", 150.0),
    ("wpn_toz_122", 150.0),
    ("wpn_toz_34", 100.0),
    ("wpn_toz_66", 100.0),
    ("wpn_tt_33", 100.0),
    ("wpn_ump45", 150.0),
    ("wpn_uzi", 100.0),
    ("wpn_uzi_premium", 100.0),
    ("wpn_val", 55.0),
    ("wpn_vepr", 150.0),
    ("wpn_vityaz", 150.0),
    ("wpn_vityaz_premium", 150.0),
    ("wpn_vsk_94", 55.0),
    ("wpn_vss", 55.0),
    ("wpn_vss_premium", 55.0),
    ("wpn_vssk", 55.0),
    ("wpn_winchester_1894", 100.0),
];

/// The reach of the weapon whose config is `cfg_name` (a path or a stem), if
/// the table has it.
pub fn reach_of(cfg_name: &str) -> Option<f32> {
    let stem = cfg_name.rsplit('/').next().unwrap_or(cfg_name);
    let stem = stem.strip_suffix(".options").unwrap_or(stem);
    SHOT_REACH_M
        .binary_search_by(|(name, _)| name.cmp(&stem))
        .ok()
        .map(|i| SHOT_REACH_M[i].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_finds_a_weapon_by_path_or_stem() {
        assert!(SHOT_REACH_M.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(SHOT_REACH_M.len(), 49);
        assert_eq!(reach_of("gameplay/items/weapons/wpn_ak_74u.options"), Some(150.0));
        assert_eq!(reach_of("wpn_vss"), Some(55.0));
        assert_eq!(reach_of("wpn_spas_12"), Some(100.0));
        assert_eq!(reach_of("wpn_nothing"), None);
    }

    #[test]
    fn every_weapon_of_the_bots_kits_has_a_reach() {
        use crate::v026g::bots::guns::Arms;
        use survarium::v026g::loadouts::{BotRole, BotTier};
        for tier in BotTier::ALL {
            for role in BotRole::ALL {
                for arms in Arms::kits_of(tier, role) {
                    let item = arms.primary.0.item;
                    assert!(reach_of(&format!("wpn_{item}")).is_some(), "{item}");
                }
            }
        }
    }

    #[test]
    fn the_reaches_are_the_games_four() {
        assert!(SHOT_REACH_M.iter().all(|&(_, m)| [55.0, 100.0, 150.0, 200.0].contains(&m)));
    }
}

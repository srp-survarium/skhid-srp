//! The maps' walk grids.

use super::*;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GridTuning {
    /// Whether the maps' walk grids have the gap links (a walking jump
    /// across a gap, up or down: `mapdata::grid::gap`), which bots then
    /// route over and jump. A grid is the map's, built once for every
    /// match on it: the server's grids take this default (`maps.rs`), the
    /// bench's this run's tuning (`SRP_BENCH_TUNING`); a tuning file of one
    /// match does not rebuild them.
    pub gap_links: bool,
    /// `Ctx::path_dist` is the regions' walk between floors when a target
    /// is a storey off; off (the default: merged switched off), the straight
    /// ground distance whatever the floor, with Collect's batteries a storey
    /// off priced by the walk as before (the bench's switch,
    /// `SRP_BENCH_FLOOR_PATHS=1` on).
    pub floor_paths: bool,
    /// Which eyes a floor beside a battery off the walk is tried with
    /// (`Tactics::stand_to_take`): [`TakeEyes::Crouched`] (the default) as
    /// it was, or also a standing one where no crouched eye sees it
    /// (`SRP_BENCH_TAKE_EYES=standing`).
    pub take_eyes: TakeEyes,
}

/// The eyes `Tactics::stand_to_take` tries a floor beside a battery with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakeEyes {
    /// The crouched eye only.
    #[default]
    Crouched,
    /// The crouched eye; where no floor's crouched eye sees the battery in
    /// a use's reach, the standing eye (the game's use ray starts at the eye
    /// of whichever pose the player has).
    CrouchedThenStanding,
}

impl GridTuning {
    /// The off-mesh links of the maps' grids.
    pub fn off_mesh_links(&self) -> mapdata::OffMeshLinks {
        if self.gap_links {
            mapdata::OffMeshLinks::Gaps
        } else {
            mapdata::OffMeshLinks::None
        }
    }
}

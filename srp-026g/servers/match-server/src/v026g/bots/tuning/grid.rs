//! The maps' walk grids.

use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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
    /// is a storey off; off, the straight
    /// ground distance whatever the floor, with Collect's batteries a storey
    /// off priced by the walk as before (the bench's switch,
    /// `SRP_BENCH_FLOOR_PATHS=1` on). On by default..
    pub floor_paths: bool,
    /// Whether the maps' walk grids are built with the numbers the 0.26g clients run
    /// ([`mapdata::Body::client`]: step 0.8 and ground walked up to 55°, `startup.cfg`), not the exe's
    /// defaults (0.6, 50°): the pile of barrels the owner walks up from one side, and no jump onto ground
    /// steeper than the clients walk. Off: the grids as they were. On by default.
    /// A grid is the map's (see `gap_links`): the server's take this default, the bench's this run's tuning.
    pub client_numbers: bool,
    /// Whether the maps' walk grids take the floors nearer to a wall than the body's radius, measured exactly,
    /// after the cell rounds ([`mapdata::Erosion::Exact`]): a floor beside a post's corner, in an opening the
    /// body does not fit (A84 item 13). Off: the cells' erosion as it was. On by default. A grid is the map's
    /// (see `gap_links`).
    pub exact_erosion: bool,
    /// Whether a Drop link of the maps' walk grids is kept only where the body that walks off the edge (3.5 m/s,
    /// carried on through the air) comes down on a floor within the harmless fall (`mapdata::Body::drop_flight_mps`):
    /// a chain of harmless steps below an edge is one fall of 7.5 m (the team 1 spawn deck on Chemical Plant, A95).
    /// Off: the cell model. On by default. A grid is the map's (see `gap_links`).
    pub honest_drops: bool,
}

impl Default for GridTuning {
    fn default() -> Self {
        Self { gap_links: true, floor_paths: true, client_numbers: true, exact_erosion: true, honest_drops: true }
    }
}

/// How fast a body walks off a ledge, m/s: the walk (3.5, `kin`); the bots' motor walks planned drops (`Feel::drop_walk`).
pub const WALK_OFF_MPS: f32 = 3.5;

impl GridTuning {
    /// The body the maps' grids are built for.
    pub fn body(&self) -> mapdata::Body {
        let body = if self.client_numbers {
            mapdata::Body::client()
        } else {
            mapdata::Body::default()
        };
        mapdata::Body {
            erosion: if self.exact_erosion { mapdata::Erosion::Exact } else { mapdata::Erosion::Cells },
            drop_flight_mps: if self.honest_drops { WALK_OFF_MPS } else { 0.0 },
            ..body
        }
    }

    /// The off-mesh links of the maps' grids.
    pub fn off_mesh_links(&self) -> mapdata::OffMeshLinks {
        if self.gap_links {
            mapdata::OffMeshLinks::Gaps
        } else {
            mapdata::OffMeshLinks::None
        }
    }
}

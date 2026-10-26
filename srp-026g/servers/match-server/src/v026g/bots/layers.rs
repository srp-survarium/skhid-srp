//! What the match is told of its map besides the walk grid, gathered as it is made before the map is built
//! ([`Bots::set_routes`], [`Bots::set_foliage`]) and put on the map's tactics when it is ([`MapLayers::attach`]).
//! A new layer (the human routes, the bushes, ...) is a field here and a line in `attach`; `Bots::tactics_ready`
//! does not list them.

use super::*;

/// The map's layers, till the tactics are built.
#[derive(Default)]
pub(super) struct MapLayers {
    /// The human routes of this map and mode (play 17), till the map is built and they are put on its floors.
    pub(super) routes: Vec<routes::HumanRoute>,
    /// The map's bushes a sight line crosses (`foliage.rs`).
    pub(super) foliage: foliage::Foliage,
    /// The places humans spend time high on this map and their share of alive time (V-S2b).
    pub(super) perches: Vec<routes::Perch>,
    pub(super) perch_share: Option<routes::PerchShare>,
}

impl MapLayers {
    /// Each layer on the tactics of the built map.
    pub(super) fn attach(self, tactics: &mut Tactics) {
        tactics.set_routes(self.routes);
        tactics.set_perches(self.perches, self.perch_share);
        tactics.foliage = Arc::new(self.foliage);
    }
}

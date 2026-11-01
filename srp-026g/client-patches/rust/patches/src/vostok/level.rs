//! The level the lobby loads for a match, at `level_loading.map`
//! (lobby_menu::switch_to_level_loading 0x220400, sites_026g.tsv): the map's
//! maps_dict entry, its id and its mode config (`mode { mode,
//! match_making_params { map_name } ... }`, modes_maps.md section 3).

use core::marker::PhantomData;

use super::config::{ConfigTable, ConfigValue};
use super::context::Context;
use super::hook::Thread;
use super::types::MapId;
use crate::raw;

/// The map being loaded: its maps_dict entry.
pub struct LevelLoading<'t> {
    map: raw::MapDescriptor,
    thread: PhantomData<&'t Thread>,
}

impl<'t> LevelLoading<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<LevelLoading<'t>> {
        // SAFETY: the context is this site's: EAX the entry map_descriptor_by_id
        // found, the items dictionary's own, alive while the lobby runs.
        let map = unsafe { raw::site::level_loading_map::map(ctx) }?;
        Some(LevelLoading { map, thread: PhantomData })
    }

    pub fn map(&self) -> MapId {
        // SAFETY: the entry is alive for the hook.
        MapId::new(unsafe { *self.map.id() })
    }

    /// Its config's `mode` table (the one switch_to_level_loading reads next).
    pub fn mode(&self) -> Option<ConfigTable<'t>> {
        // SAFETY: the entry's config, loaded (the game reads its root right
        // after the site) and immutable for the hook; its loader relocated
        // every node.
        let root = unsafe { ConfigValue::read(self.map.config()?.root()?) }.ok()?.table().ok()?;
        root.get(c"mode").ok().flatten()?.table().ok()
    }
}

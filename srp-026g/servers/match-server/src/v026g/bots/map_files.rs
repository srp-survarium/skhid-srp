//! [A326] The maps' bushes (`bot-foliage.json`), grass (`bot-grass.json`) and human routes (`bot-routes.json`): one copy
//! per map (bushes), level (grass) or level and mode (routes) while a match holds it, read and parsed once for it rather
//! than once per match, and gone with the last match that holds it (the owner's rule for per-map data). The store keeps a
//! `Weak` of each, as [`super::maps::Maps`] does of the worlds: it never keeps one alive itself.

use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use super::foliage::Foliage;
use super::grass::Grass;
use super::objective::Mode;
use super::routes::{HumanRoute, Perch, PerchShare, RouteTable};

/// One level and mode's rows of the route table: what a match's bots are given of it (play 17, V-S2b).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapRoutes {
    /// The human routes of this map and mode, put on the floors by each match's tactics.
    pub routes: Vec<HumanRoute>,
    /// The places humans spend time high on this map, and their share of alive time.
    pub perches: Vec<Perch>,
    pub perch_share: Option<PerchShare>,
}

impl MapRoutes {
    /// `level`'s rows of `table` for `mode`.
    pub fn of(table: &RouteTable, level: u8, mode: Mode) -> Self {
        Self {
            routes: table.for_map(level, mode.rules()),
            perches: table.perches_for(level),
            perch_share: table.perch_share(level, mode.id()),
        }
    }
}

/// Values by key, each kept by a `Weak` only: made by its loader when none is held, shared while one is.
struct Shared<K, T>(Mutex<HashMap<K, Weak<T>>>);

impl<K, T> Default for Shared<K, T> {
    fn default() -> Self {
        Self(Mutex::new(HashMap::new()))
    }
}

impl<K: Eq + Hash, T> Shared<K, T> {
    /// The value of `key` a match holds, else `load`'s (an error is not kept: the next match tries again).
    fn get(&self, key: K, load: impl FnOnce() -> Result<T, String>) -> Result<Arc<T>, String> {
        let mut held = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        held.retain(|_, value| value.strong_count() > 0);
        if let Some(value) = held.get(&key).and_then(Weak::upgrade) {
            return Ok(value);
        }
        let value = Arc::new(load()?);
        held.insert(key, Arc::downgrade(&value));
        Ok(value)
    }

    #[cfg(test)]
    fn held(&self) -> usize {
        let held = self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        held.values().filter(|value| value.strong_count() > 0).count()
    }
}

/// The store of the maps' bushes, grass and routes ([module](self)), one per server ([`super::maps::Maps::files`]).
#[derive(Default)]
pub struct MapFiles {
    foliage: Shared<(PathBuf, String), Foliage>,
    grass: Shared<(PathBuf, String), Grass>,
    routes: Shared<(PathBuf, u8, u8), MapRoutes>,
}

impl MapFiles {
    /// `project`'s bushes from the file at `path`.
    pub fn foliage(&self, path: &Path, project: &str) -> Result<Arc<Foliage>, String> {
        self.foliage.get((path.to_owned(), project.to_owned()), || Foliage::load(path, project))
    }

    /// The grass of `project`'s level from the file at `path` (every weather of a level shares it).
    pub fn grass(&self, path: &Path, project: &str) -> Result<Arc<Grass>, String> {
        let level = Grass::level_of(project).to_owned();
        self.grass.get((path.to_owned(), level), || Grass::load(path, project))
    }

    /// `level`'s routes for `mode` from the table at `path`.
    pub fn routes(&self, path: &Path, level: u8, mode: Mode) -> Result<Arc<MapRoutes>, String> {
        self.routes.get((path.to_owned(), level, mode.id()), || Ok(MapRoutes::of(&RouteTable::load(path)?, level, mode)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One copy while a match holds it, made again (not kept) once none does; an error is not kept.
    #[test]
    fn a_value_is_shared_while_held_and_gone_after() {
        let shared: Shared<u8, Vec<u8>> = Shared::default();
        let mut loads = 0;
        let mut load = || {
            loads += 1;
            Ok(vec![1, 2, 3])
        };
        let a = shared.get(1, &mut load).unwrap();
        let b = shared.get(1, &mut load).unwrap();
        assert!(Arc::ptr_eq(&a, &b), "a second match shares the first's");
        assert_eq!(shared.held(), 1);
        drop((a, b));
        assert_eq!(shared.held(), 0, "the store keeps none alive");
        let _c = shared.get(1, &mut load).unwrap();
        assert_eq!(loads, 2, "loaded again once none held it");
        assert!(shared.get(2, || Err("no file".to_owned())).is_err());
        let _d = shared.get(2, || Ok(vec![4])).unwrap();
    }
}

//! What a player's create window starts with: the settings of the last
//! room it made by hand, kept with its account across restarts (the
//! owner's of 2026-10-20: "when you set up your room with the specific
//! levels, specific maps and game modes is for it to persist next time").
//! The PIN is not kept: a private room's is typed each time. A room Play
//! made is not one made by hand: it keeps nothing.
//!
//! The window chooses its pool by tile (a map and a weather, every mode of
//! it) and by mode; an entry of the pool is in when both are. What is kept
//! is how the choice differs from the window's defaults, by map, weather
//! and mode rather than by entry, so the pool may change between two
//! rooms: a tile or a mode no longer offered is dropped, one offered later
//! starts as the defaults have it (in; an experimental map out). The
//! sliders' values are fitted to today's ranges, and a max level to the
//! player's equipment now (never under it), when the window asks
//! ([`form`]).

use survarium::v026g::lobby::rooms::{LEVEL_MAX, MIN_SIDES, PoolEntry, RoomForm, SIZES, WAITS};

/// The window's defaults (and a room Play makes): 8 v 8, from 1 v 1, any
/// max level, a wait of 30 s, the match pool without the experimental maps,
/// every mode.
pub const DEFAULT_SIZE: u8 = 8;
pub const DEFAULT_MIN_SIDE: u8 = 1;
pub const DEFAULT_MAX_LEVEL: u8 = 0;
pub const DEFAULT_WAIT: u8 = 30;

/// A map (`maps::MapMode::level`) and a weather: a tile of the window.
pub type Tile = (u8, u8);

/// The settings of an account's last room made by hand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomSettings {
    /// Each side's players.
    pub size: u8,
    /// The players a side it needs before its vote begins.
    pub min_side: u8,
    /// The highest equipment level let in; 0: any.
    pub max_level: u8,
    /// The wait, seconds (the vote's 15 s not counted).
    pub wait: u8,
    /// Tiles of the match pool it left out.
    pub tiles_off: Vec<Tile>,
    /// Experimental tiles it put in.
    pub tiles_on: Vec<Tile>,
    /// The modes it left out.
    pub modes_off: Vec<u8>,
}

fn tile(entry: &PoolEntry) -> Tile {
    (entry.level, entry.weather)
}

/// The tiles of `entries`, each once in the order they come, and whether
/// it is an experimental one: as its first entry is, as the window has it.
fn tiles(entries: &[PoolEntry]) -> Vec<(Tile, bool)> {
    let mut tiles: Vec<(Tile, bool)> = Vec::new();
    for entry in entries {
        if !tiles.iter().any(|&(tile_, _)| tile_ == tile(entry)) {
            tiles.push((tile(entry), entry.extra));
        }
    }
    tiles
}

/// The modes of `entries`, each once in the order they come.
fn modes(entries: &[PoolEntry]) -> Vec<u8> {
    let mut modes: Vec<u8> = Vec::new();
    for entry in entries {
        if !modes.contains(&entry.mode) {
            modes.push(entry.mode);
        }
    }
    modes
}

impl RoomSettings {
    /// What a room made by hand is kept as: its settings and its pool
    /// (`maps_dict` ids) as the pool offered now (`entries`) names them.
    /// A tile is in when an entry of it is, a mode when an entry of it is:
    /// the same tiles and modes give the same pool back.
    pub fn of_room(
        size: u8,
        min_side: u8,
        max_level: u8,
        wait: u8,
        pool: &[u8],
        entries: &[PoolEntry],
    ) -> Self {
        let chosen: Vec<&PoolEntry> = entries
            .iter()
            .filter(|entry| pool.contains(&entry.map_id))
            .collect();
        let mut tiles_off = Vec::new();
        let mut tiles_on = Vec::new();
        for (tile_, extra) in tiles(entries) {
            let on = chosen.iter().any(|entry| tile(entry) == tile_);
            match (extra, on) {
                (false, false) => tiles_off.push(tile_),
                (true, true) => tiles_on.push(tile_),
                _ => {}
            }
        }
        let modes_off = modes(entries)
            .into_iter()
            .filter(|&mode| !chosen.iter().any(|entry| entry.mode == mode))
            .collect();
        Self {
            size,
            min_side,
            max_level,
            wait,
            tiles_off,
            tiles_on,
            modes_off,
        }
    }
}

/// The window's defaults for the pool offered now.
pub fn defaults(entries: &[PoolEntry]) -> RoomForm {
    let tiles = tiles(entries);
    RoomForm {
        saved: false,
        size: DEFAULT_SIZE,
        min_side: DEFAULT_MIN_SIDE,
        max_level: DEFAULT_MAX_LEVEL,
        wait: DEFAULT_WAIT,
        tiles_on: entries
            .iter()
            .map(|entry| {
                tiles
                    .iter()
                    .any(|&(tile_, extra)| tile_ == tile(entry) && !extra)
            })
            .collect(),
        modes_off: Vec::new(),
    }
}

/// What the create window starts with, for an account that kept `saved`
/// (none: the defaults), whose equipment level is `gear` now, the pool
/// offered being `entries`. Its values are within today's ranges (the
/// nearest wait; a max level under `gear` raised to it). A choice that
/// leaves no entry in (what it had is no longer offered) gives the
/// default pool, its settings kept.
pub fn form(saved: Option<&RoomSettings>, entries: &[PoolEntry], gear: u8) -> RoomForm {
    let defaults = defaults(entries);
    let Some(saved) = saved else {
        return defaults;
    };
    let tiles = tiles(entries);
    let tile_on = |entry: &PoolEntry| {
        let tile_ = tile(entry);
        let extra = tiles.iter().any(|&(other, extra)| other == tile_ && extra);
        if extra {
            saved.tiles_on.contains(&tile_)
        } else {
            !saved.tiles_off.contains(&tile_)
        }
    };
    let tiles_on: Vec<bool> = entries.iter().map(tile_on).collect();
    let modes_off: Vec<u8> = modes(entries)
        .into_iter()
        .filter(|mode| saved.modes_off.contains(mode))
        .collect();
    let any = entries
        .iter()
        .zip(&tiles_on)
        .any(|(entry, &on)| on && !modes_off.contains(&entry.mode));
    let (tiles_on, modes_off) = if any {
        (tiles_on, modes_off)
    } else {
        (defaults.tiles_on, defaults.modes_off)
    };
    RoomForm {
        saved: true,
        size: saved.size.clamp(*SIZES.start(), *SIZES.end()),
        min_side: saved.min_side.clamp(*MIN_SIDES.start(), *MIN_SIDES.end()),
        max_level: max_level(saved.max_level, gear),
        wait: nearest_wait(saved.wait),
        tiles_on,
        modes_off,
    }
}

/// A max level within the rules: at most [`LEVEL_MAX`]; one under the
/// player's equipment (a room it could not make) raised to it; 0 (any)
/// stays.
fn max_level(saved: u8, gear: u8) -> u8 {
    match saved.min(LEVEL_MAX) {
        0 => 0,
        level => level.max(gear.min(LEVEL_MAX)),
    }
}

/// The wait of [`WAITS`] nearest `wait`.
fn nearest_wait(wait: u8) -> u8 {
    WAITS
        .iter()
        .copied()
        .min_by_key(|&each| each.abs_diff(wait))
        .unwrap_or(DEFAULT_WAIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(map_id: u8, level: u8, weather: u8, mode: u8, extra: bool) -> PoolEntry {
        PoolEntry {
            map_id,
            level,
            weather,
            mode,
            extra,
        }
    }

    /// Two maps with two weathers and two modes, and an experimental map.
    fn pool() -> Vec<PoolEntry> {
        vec![
            entry(10, 1, 1, 2, false),
            entry(11, 1, 1, 4, false),
            entry(12, 1, 3, 2, false),
            entry(20, 2, 1, 2, false),
            entry(21, 2, 1, 4, false),
            entry(30, 9, 1, 2, true),
        ]
    }

    /// The entries a form puts in: tile in, mode not out.
    fn chosen(form: &RoomForm, entries: &[PoolEntry]) -> Vec<u8> {
        entries
            .iter()
            .zip(&form.tiles_on)
            .filter(|(entry, on)| **on && !form.modes_off.contains(&entry.mode))
            .map(|(entry, _)| entry.map_id)
            .collect()
    }

    #[test]
    fn nothing_kept_gives_the_defaults() {
        let entries = pool();
        let form = form(None, &entries, 4);
        assert_eq!(
            form,
            RoomForm {
                saved: false,
                size: 8,
                min_side: 1,
                max_level: 0,
                wait: 30,
                tiles_on: vec![true, true, true, true, true, false],
                modes_off: Vec::new(),
            }
        );
    }

    #[test]
    fn a_room_comes_back_as_it_was_made() {
        let entries = pool();
        // School (2) left out, the experimental map put in, deathmatch (4)
        // out: Rudnya by day and evening, the experimental map, in mode 2.
        let kept = RoomSettings::of_room(9, 2, 6, 45, &[10, 12, 30], &entries);
        assert_eq!(kept.tiles_off, [(2, 1)]);
        assert_eq!(kept.tiles_on, [(9, 1)]);
        assert_eq!(kept.modes_off, [4]);
        let form = form(Some(&kept), &entries, 3);
        assert_eq!(
            (
                form.saved,
                form.size,
                form.min_side,
                form.max_level,
                form.wait
            ),
            (true, 9, 2, 6, 45)
        );
        assert_eq!(form.tiles_on, [true, true, true, false, false, true]);
        assert_eq!(chosen(&form, &entries), [10, 12, 30]);
        // Every mode and the whole pool: nothing differs from the defaults.
        let all = RoomSettings::of_room(8, 1, 0, 30, &[10, 11, 12, 20, 21, 30], &entries);
        assert_eq!(
            (all.tiles_off.len(), all.tiles_on.len(), all.modes_off.len()),
            (0, 1, 0)
        );
        assert_eq!(
            chosen(&form_of(&all, &entries), &entries),
            [10, 11, 12, 20, 21, 30]
        );
    }

    fn form_of(kept: &RoomSettings, entries: &[PoolEntry]) -> RoomForm {
        form(Some(kept), entries, 0)
    }

    #[test]
    fn a_map_no_longer_offered_is_dropped_and_a_new_one_starts_as_the_defaults() {
        let entries = pool();
        // School left out, the experimental map in.
        let kept = RoomSettings::of_room(8, 1, 0, 30, &[10, 11, 12, 30], &entries);
        // Later: the experimental map gone, Rudnya's evening gone; a new map
        // (3) and a new experimental one (10) offered.
        let later = vec![
            entry(10, 1, 1, 2, false),
            entry(11, 1, 1, 4, false),
            entry(20, 2, 1, 2, false),
            entry(21, 2, 1, 4, false),
            entry(40, 3, 1, 2, false),
            entry(50, 10, 1, 4, true),
        ];
        let form = form_of(&kept, &later);
        assert_eq!(form.tiles_on, [true, true, false, false, true, false]);
        assert_eq!(chosen(&form, &later), [10, 11, 40]);
        // A mode no longer offered is dropped too.
        let only_deathmatch = vec![entry(11, 1, 1, 4, false), entry(21, 2, 1, 4, false)];
        let no_tdm = RoomSettings::of_room(8, 1, 0, 30, &[10, 12, 20], &entries);
        assert_eq!(no_tdm.modes_off, [4]);
        let left = vec![entry(10, 1, 1, 2, false), entry(20, 2, 1, 2, false)];
        assert_eq!(form_of(&no_tdm, &left).modes_off, Vec::<u8>::new());
        // Nothing of the choice left: the default pool, the rest kept.
        let form = form_of(&no_tdm, &only_deathmatch);
        assert_eq!(chosen(&form, &only_deathmatch), [11, 21]);
        assert_eq!(form.modes_off, Vec::<u8>::new());
        assert!(form.saved);
    }

    #[test]
    fn values_out_of_range_are_brought_in() {
        let entries = pool();
        let kept = |size, min_side, max_level, wait| RoomSettings {
            size,
            min_side,
            max_level,
            wait,
            tiles_off: Vec::new(),
            tiles_on: Vec::new(),
            modes_off: Vec::new(),
        };
        let fitted = |settings: RoomSettings, gear| {
            let form = form(Some(&settings), &entries, gear);
            (form.size, form.min_side, form.max_level, form.wait)
        };
        assert_eq!(fitted(kept(12, 0, 20, 33), 0), (10, 1, 15, 35));
        assert_eq!(fitted(kept(3, 5, 4, 1), 0), (6, 3, 4, 15));
        assert_eq!(fitted(kept(8, 1, 0, 200), 7), (8, 1, 0, 60));
        // The nearest wait.
        assert_eq!(fitted(kept(8, 1, 0, 32), 0).3, 30);
        // A max level under the equipment now: raised to it; any stays any.
        assert_eq!(fitted(kept(8, 1, 3, 30), 6).2, 6);
        assert_eq!(fitted(kept(8, 1, 9, 30), 6).2, 9);
        assert_eq!(fitted(kept(8, 1, 3, 30), 20).2, 15);
    }
}

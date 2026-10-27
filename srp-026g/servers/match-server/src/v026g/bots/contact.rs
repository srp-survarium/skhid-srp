//! A walking body that does not move: the contacts (A66, the 717 door on Chemical Plant).
//!
//! The stuck watch fires after 1.5 s of walk keys held and the body within 0.4 m; a body that presses into a jamb or
//! a wall face for under that, again and again at the same place, is invisible to it. This only counts (read-only,
//! nothing is decided from it): a contact is a walk of the route's (not aimed) with the forward key held and the
//! ground speed under [`CONTACT_SPEED_MS`] for [`CONTACT_MS`], counted once with the milliseconds it lasts and the
//! spot (a 1 m bin on the ground, 2 m in height) it happened at, so the bench shows where bodies press and how often.
//! The landing lock of a hard fall ([`super::stuck::HARD_FALL_M`]) is a contact of this kind too; `fall_locks` tells
//! them apart.

use crate::v026g::drive::key;

/// A body presses when its ground speed is under this with the forward key held, m/s.
pub const CONTACT_SPEED_MS: f32 = 0.3;
/// … for this long, ms, to be one contact.
pub const CONTACT_MS: u32 = 500;
/// A pressing that was last seen longer ago than this is over, ms (the ticks since were not a walk's).
const STALE_MS: u32 = 250;

/// What the body does as to a contact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ContactWatch {
    /// Moving, or not walking the route.
    #[default]
    Free,
    /// Pressing since `since`; `last` the tick before (for the milliseconds); `counted`: the contact is in the count.
    Pressing { since: u32, last: u32, counted: bool },
}

/// What a tick of the watch says to count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Counted {
    /// Nothing new.
    Nothing,
    /// A contact of at least [`CONTACT_MS`]: `first` the tick it is first counted on; `ms` since the last tick counted.
    Contact { first: bool, ms: u32 },
}

impl ContactWatch {
    /// One tick. `walking`: the decision walks the route (not aimed, nothing else drives the keys); `keys`: its keys;
    /// `speed`: the body's ground speed.
    pub fn tick(&mut self, time: u32, walking: bool, keys: u32, speed: f32) -> Counted {
        let pressing = walking && keys & key::FORWARD != 0 && speed < CONTACT_SPEED_MS;
        match (*self, pressing) {
            (_, false) => {
                *self = Self::Free;
                Counted::Nothing
            }
            (Self::Free, true) => {
                *self = Self::Pressing { since: time, last: time, counted: false };
                Counted::Nothing
            }
            // The ticks between were not of a walk (an override, a stand): a new contact.
            (Self::Pressing { last, .. }, true) if time > last + STALE_MS => {
                *self = Self::Pressing { since: time, last: time, counted: false };
                Counted::Nothing
            }
            (Self::Pressing { since, .. }, true) if time < since + CONTACT_MS => Counted::Nothing,
            (Self::Pressing { since, last, counted }, true) => {
                *self = Self::Pressing { since, last: time, counted: true };
                Counted::Contact { first: !counted, ms: time.saturating_sub(last) }
            }
        }
    }
}

/// The bench's counts of contacts: the contacts, the milliseconds spent in them, and where they happened.
pub mod contacts {
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    use mapdata::Vec3;

    use super::Counted;

    static COUNTS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
    /// Per spot (x metre, z metre, y two metres), the contacts.
    static SPOTS: Mutex<BTreeMap<(i32, i32, i32), u64>> = Mutex::new(BTreeMap::new());

    /// Counts what a tick says, at `feet`.
    pub fn count(counted: Counted, feet: Vec3) {
        let Counted::Contact { first, ms } = counted else {
            return;
        };
        COUNTS[1].fetch_add(u64::from(ms), Ordering::Relaxed);
        if first {
            COUNTS[0].fetch_add(1, Ordering::Relaxed);
            if let Ok(mut spots) = SPOTS.lock() {
                *spots.entry((feet.x.floor() as i32, feet.z.floor() as i32, (feet.y / 2.0).floor() as i32)).or_default() += 1;
            }
        }
    }

    /// Contacts and milliseconds.
    pub fn counts() -> [u64; 2] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }

    /// The `n` spots with the most contacts: ((x, z, y/2) of the bin's low corner, contacts).
    pub fn top(n: usize) -> Vec<((i32, i32, i32), u64)> {
        let mut spots: Vec<_> = SPOTS.lock().map(|s| s.iter().map(|(&k, &v)| (k, v)).collect()).unwrap_or_default();
        spots.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        spots.truncate(n);
        spots
    }
}

#[cfg(test)]
mod tests {
    use mapdata::Vec3;

    use super::*;

    fn run(walking: bool, keys: u32, speed: f32) -> Vec<Counted> {
        let mut watch = ContactWatch::default();
        (0..30u32).map(|tick| watch.tick(1_000 + tick * 50, walking, keys, speed)).collect()
    }

    /// A walk with forward held and no speed for half a second is one contact: counted first at 0.5 s and again every
    /// tick after with the milliseconds since; moving, not walking the route, or no forward key is none.
    #[test]
    fn a_walk_that_presses_without_moving_is_one_contact() {
        let counted = run(true, key::FORWARD, 0.0);
        let firsts = counted.iter().filter(|c| matches!(c, Counted::Contact { first: true, .. })).count();
        let ms: u32 = counted.iter().map(|c| if let Counted::Contact { ms, .. } = c { *ms } else { 0 }).sum();
        assert_eq!(firsts, 1, "{counted:?}");
        assert!((900..=1000).contains(&ms), "about 1 s of the 1.45 s: {ms}");
        for none in [run(true, key::FORWARD, 2.0), run(false, key::FORWARD, 0.0), run(true, key::LEFT, 0.0)] {
            assert!(none.iter().all(|c| *c == Counted::Nothing), "{none:?}");
        }
        // The spot is binned by the metre and two metres of height.
        contacts::count(Counted::Contact { first: true, ms: 50 }, Vec3::new(24.34, 10.17, 80.1));
        assert!(contacts::top(1000).iter().any(|&((x, z, y), n)| (x, z, y) == (24, 80, 5) && n >= 1));
    }
}

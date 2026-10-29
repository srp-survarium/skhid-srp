//! The grenades in the match (`docs`: `grenades-record` §4): the server's own list of thrown fragmentation
//! grenades, the one thing the bots' view lacked. The match hands over no object states (the worlds' objects
//! section is 5 s apart and carries no body, `fact-grenade-arc.md`), but every throw begins with a key a place
//! presses, and the server sees every place's input, a human's as relayed and a bot's as made.
//!
//! - **The press.** The throw key ([`key::GRENADE`]: the first pocket that holds a grenade) or a quick-use
//!   key whose slot holds one ([`Pockets`], from the place's profile) goes down: the thrower, its team and
//!   the press time are known. The fuse (the grenade's `ttl_ms`) runs from the press, whichever key began the throw (PROVEN,
//!   `fact-grenade-arc.md`). A smoke grenade (`rdg-m`) is no danger and is left out, counted apart. A server
//!   bot has no profile table: its throw key is a frag (the reflex's only grenade).
//! - **The launch.** [`LAUNCH_MS`] after the press (LIKELY) the grenade leaves the hand along the thrower's
//!   view: the arc model of `arc.rs` over the map's bullet mesh gives the first contact and where it comes to
//!   rest ([`arc::Landing`]), once, from the body's reckoning at that time. A thrower that is dead by then
//!   threw nothing (counted).
//! - **The blast.** The fuse after the press; the entry is kept [`SPENT_MS`] after it.
//!
//! The list only records and predicts: nothing reads it unless a bot's tuning asks for it
//! (`grenade_avoid`), so a match plays out as before.

use mapdata::Vec3;

use super::arc::{self, Grenade};
use super::view::Place;
use crate::v026g::drive::key;

/// The grenade leaves the hand this long after the press, ms (`arc::LAUNCH_DELAY_S`, LIKELY).
pub const LAUNCH_MS: u32 = (arc::LAUNCH_DELAY_S * 1000.0) as u32;
/// An entry is kept this long past its blast, ms: a poll or two, so a reader at the blast tick still sees it.
pub const SPENT_MS: u32 = 300;
/// A second throw key of one place this soon after the first is the same throw: a quick-use key acts when it
/// goes down and when it comes up, and a held throw key is pressed again (the recordings' rule, 1.25 s).
pub const SAME_THROW_MS: u32 = 1_250;

/// Every key that can begin a throw: the throw key and the six quick-use keys.
pub const THROW_KEYS: u32 = key::GRENADE
    | key::QUICK[0]
    | key::QUICK[1]
    | key::QUICK[2]
    | key::QUICK[3]
    | key::QUICK[4]
    | key::QUICK[5];

/// A grenade thrown: who, for which team, when the key went down, and how far it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiveGrenade {
    pub by: Place,
    pub team: u8,
    /// The match ms of the press.
    pub press: u32,
    pub phase: Phase,
    /// Its numbers (fuse, blast), from the game data: the bots' for a bot's, else its pocket's.
    pub grenade: Grenade,
}

/// Where a grenade is in its life.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    /// The key is down, the pin out, the grenade still in the hand: the view may yet turn, so no landing.
    Pressed,
    /// Let go: where it first touches the ground, when, and where it comes to rest (the expected blast).
    Flying {
        launched: u32,
        lands: u32,
        rest: Vec3,
    },
}

impl LiveGrenade {
    /// When it goes off, match ms: the fuse from the press (PROVEN: 80 of 82 recorded blasts at press + 4000 ms
    /// for the RGD-5).
    pub fn blast_at(&self) -> u32 {
        self.press + self.grenade.fuse_ms()
    }

    /// Whether the entry still counts at `clock`: until [`SPENT_MS`] past the blast.
    pub fn live_at(&self, clock: u32) -> bool {
        clock <= self.blast_at() + SPENT_MS
    }
}

/// What a pocket of a profile holds, as far as the throw keys care.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pocketed {
    /// A fragmentation grenade: its blast hurts (the game data's `rgd-5`).
    Frag,
    /// Another grenade item (the game data's `rdg-m` smoke): no blast to keep out of.
    Smoke,
    /// Anything else the quick-use keys count (a medkit, a trap, a booster).
    Other,
}

impl Pocketed {
    pub fn is_grenade(self) -> bool {
        self != Self::Other
    }
}

/// One pocket (profile slot 13..=18): what it holds, and how many units, at the spawn and now; the numbers of the
/// grenade it holds (the bots' for a pocket made without them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pocket {
    pub item: Pocketed,
    pub full: u16,
    pub left: u16,
    pub grenade: Grenade,
}

/// A player's pockets, in slot order: what the throw keys throw (`loadouts::POCKET_SLOTS`,
/// `QUICK_USE_ORDER`, PROVEN): the throw key the first pocket that holds a grenade with units left, quick use
/// `n` the `n`-th pocket that holds an item with units left (the pockets come first in the client's order),
/// and a grenade only if that item is one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pockets {
    slots: [Option<Pocket>; 6],
}

impl Pockets {
    /// From the profile's pocket slots 13..=18 in order: `(item, units)`, an empty or spent slot `None`.
    pub fn new(slots: [Option<(Pocketed, u16)>; 6]) -> Self {
        Self {
            slots: slots.map(|slot| {
                slot.filter(|&(_, units)| units > 0).map(|(item, units)| Pocket {
                    item,
                    full: units,
                    left: units,
                    grenade: *Grenade::bots(),
                })
            }),
        }
    }

    /// The same pockets holding the grenades `grenades` (by slot; a slot with none keeps the bots' numbers): a
    /// frag has its own fuse and blast in the game data.
    pub fn with_grenades(mut self, grenades: [Option<Grenade>; 6]) -> Self {
        for (slot, grenade) in self.slots.iter_mut().zip(grenades) {
            if let (Some(pocket), Some(grenade)) = (slot.as_mut(), grenade) {
                pocket.grenade = grenade;
            }
        }
        self
    }

    /// A new life: every pocket full again.
    pub fn refill(&mut self) {
        for pocket in self.slots.iter_mut().flatten() {
            pocket.left = pocket.full;
        }
    }

    /// The index of the pocket the keys `pressed` throw from, if they throw a grenade.
    fn throws_from(&self, pressed: u32) -> Option<usize> {
        let held = |slot: &Option<Pocket>| slot.is_some_and(|pocket| pocket.left > 0);
        if pressed & key::GRENADE != 0 {
            return self
                .slots
                .iter()
                .position(|slot| held(slot) && slot.is_some_and(|pocket| pocket.item.is_grenade()));
        }
        let n = key::QUICK.iter().position(|&bit| pressed & bit != 0)?;
        let i = self
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| held(slot))
            .map(|(i, _)| i)
            .nth(n)?;
        self.slots[i]
            .is_some_and(|pocket| pocket.item.is_grenade())
            .then_some(i)
    }

    /// The grenade the keys `pressed` (just gone down) throw, if any: one unit is taken.
    pub fn throw(&mut self, pressed: u32) -> Option<Pocketed> {
        self.throw_grenade(pressed).map(|(item, _)| item)
    }

    /// [`Self::throw`] with the numbers of the grenade that was thrown.
    pub fn throw_grenade(&mut self, pressed: u32) -> Option<(Pocketed, Grenade)> {
        let i = self.throws_from(pressed)?;
        let pocket = self.slots[i].as_mut()?;
        pocket.left -= 1;
        Some((pocket.item, pocket.grenade))
    }
}

/// What the tracker made of the throw keys it saw (a counter, for the logs and the benches).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ThrowCounts {
    /// Fragmentation grenades listed (the throw key of a bot, or a human's key that threw one).
    pub frag: u32,
    /// Smoke grenades: seen, left out.
    pub smoke: u32,
    /// A human's throw key with no pockets known (a late joiner, or a profile that did not read): not listed.
    pub no_pockets: u32,
    /// Listed and let go, with a landing.
    pub launched: u32,
    /// Let go with no first contact within the flight's time (no ground, or off the map): dropped.
    pub unplaced: u32,
    /// The thrower was dead by the launch: dropped.
    pub died_throwing: u32,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use mapdata::World;

    use super::super::body::Fix;
    use super::super::kin::Kinematics;
    use super::super::view::{Clock, Tracker};
    use super::*;

    fn pockets() -> Pockets {
        // Pocket 13 a medkit, 14 two frags, 15 a smoke, 16.. empty.
        Pockets::new([
            Some((Pocketed::Other, 3)),
            Some((Pocketed::Frag, 2)),
            Some((Pocketed::Smoke, 1)),
            None,
            None,
            None,
        ])
    }

    #[test]
    fn the_throw_key_takes_the_first_pocket_with_a_grenade() {
        let mut p = pockets();
        assert_eq!(p.throw(key::GRENADE), Some(Pocketed::Frag));
        assert_eq!(p.throw(key::GRENADE), Some(Pocketed::Frag));
        // The frags are spent: the smoke is the first grenade left.
        assert_eq!(p.throw(key::GRENADE), Some(Pocketed::Smoke));
        assert_eq!(p.throw(key::GRENADE), None);
    }

    #[test]
    fn a_quick_key_throws_only_if_its_slot_is_a_grenade() {
        let mut p = pockets();
        // Quick 1 is the medkit's pocket, quick 2 the frags', quick 3 the smoke's, 4 nothing.
        assert_eq!(p.throw(key::QUICK[0]), None);
        assert_eq!(p.throw(key::QUICK[1]), Some(Pocketed::Frag));
        assert_eq!(p.throw(key::QUICK[2]), Some(Pocketed::Smoke));
        assert_eq!(p.throw(key::QUICK[3]), None);
        // The smoke is spent: it no longer counts, quick 3 is nothing now, and the frag is still quick 2.
        assert_eq!(p.throw(key::QUICK[2]), None);
        assert_eq!(p.throw(key::QUICK[1]), Some(Pocketed::Frag));
        // Both frags thrown: the next pocket moves up, quick 2 is nothing.
        assert_eq!(p.throw(key::QUICK[1]), None);
    }

    #[test]
    fn spent_pockets_close_the_gap_for_quick_use_and_a_new_life_fills_them() {
        let mut p = Pockets::new([
            Some((Pocketed::Frag, 1)),
            Some((Pocketed::Smoke, 1)),
            None,
            None,
            None,
            None,
        ]);
        assert_eq!(p.throw(key::QUICK[0]), Some(Pocketed::Frag));
        // The frag is gone: the smoke is now the first pocket held.
        assert_eq!(p.throw(key::QUICK[0]), Some(Pocketed::Smoke));
        assert_eq!(p.throw(key::GRENADE), None);
        p.refill();
        assert_eq!(p.throw(key::GRENADE), Some(Pocketed::Frag));
    }

    /// A floor at 0, 200 m a side: the map's bullet mesh for the launch.
    fn floor_world() -> Arc<World> {
        let (a, b) = (Vec3::new(-100.0, 0.0, -100.0), Vec3::new(100.0, 0.0, -100.0));
        let (c, d) = (Vec3::new(100.0, 0.0, 100.0), Vec3::new(-100.0, 0.0, 100.0));
        Arc::new(World {
            map: mapdata::MapData::default(),
            grid: Arc::new(mapdata::WalkGrid::build(
                &[],
                [0.0, 0.0, 1.0, 1.0],
                0.25,
                mapdata::Body::default(),
            )),
            sight: Arc::new(mapdata::RayTree::from_corners(vec![[a, b, c], [a, c, d]])),
        })
    }

    fn fix(time: u32, inserted: u32) -> Fix {
        Fix {
            time,
            alive: true,
            inserted,
            feet: Some(Vec3::ZERO),
            yaw: 0.0,
            pitch: 0.0,
            keys: 0,
            speed: Some(0.0),
        }
    }

    /// `(0, bot)` and `(1, human)` standing at the origin facing +z, placed by a world at 500 ms.
    fn stand(pockets: Option<Pockets>) -> Tracker {
        let kin = Kinematics::default();
        let mut tracker = Tracker::new(2, &[0, 1], &[true, false], vec![]);
        for place in 0..2 {
            tracker.entered(place, true);
            tracker.fixed(place, fix(500, 10), Some(1.0), &kin, None);
        }
        tracker.set_pockets(vec![None, pockets]);
        tracker
    }

    fn press(tracker: &mut Tracker, place: Place, time: u32, keys: u32) {
        tracker.input(place, time, keys, (0.0, 0.0), 0.0, &Kinematics::default(), None);
    }

    #[test]
    fn a_bots_throw_key_is_listed_and_lands_where_the_arc_puts_it_at_the_launch() {
        let world = floor_world();
        let mut tracker = stand(None);
        press(&mut tracker, 0, 1_000, key::GRENADE);
        press(&mut tracker, 0, 1_050, 0);
        let listed = tracker.live_grenades().to_vec();
        assert_eq!(listed.len(), 1);
        assert_eq!((listed[0].by, listed[0].team, listed[0].press), (0, 0, 1_000));
        assert_eq!(listed[0].phase, Phase::Pressed);
        assert_eq!(listed[0].blast_at(), 5_000);
        // Before the launch (press + 1100 ms): still in the hand.
        tracker.launch_grenades(2_099, Some(&*world));
        assert_eq!(tracker.live_grenades()[0].phase, Phase::Pressed);
        // At it: a level view from about 1.3 m over a floor comes down 8 to 9 m ahead, rolls 2 m on.
        tracker.launch_grenades(2_100, Some(&*world));
        let Phase::Flying { launched, lands, rest } = tracker.live_grenades()[0].phase else {
            panic!("launched: {:?}", tracker.live_grenades());
        };
        assert_eq!(launched, 2_100);
        assert!((2_300..2_900).contains(&lands), "{lands}");
        assert!(rest.x.abs() < 0.1 && (9.0..13.0).contains(&rest.z), "{rest:?}");
        assert_eq!(tracker.throw_counts().launched, 1);
        // The view carries it until a moment past the blast.
        let clock = Clock {
            clock: 3_000,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        assert_eq!(tracker.view(&clock, &Kinematics::default(), Some(&*world)).grenades.len(), 1);
        tracker.launch_grenades(5_301, Some(&*world));
        assert!(tracker.live_grenades().is_empty());
    }

    #[test]
    fn a_thrower_dead_by_the_launch_threw_nothing_and_no_map_leaves_it_in_the_hand() {
        let world = floor_world();
        let mut tracker = stand(None);
        press(&mut tracker, 0, 1_000, key::GRENADE);
        tracker.launch_grenades(2_200, None);
        assert_eq!(tracker.live_grenades().len(), 1, "no map: not placed, not dropped");
        let dead = Fix {
            alive: false,
            feet: None,
            speed: None,
            ..fix(1_500, 10)
        };
        tracker.fixed(0, dead, Some(0.0), &Kinematics::default(), None);
        tracker.launch_grenades(2_200, Some(&*world));
        assert!(tracker.live_grenades().is_empty());
        assert_eq!(tracker.throw_counts().died_throwing, 1);
    }

    #[test]
    fn a_humans_quick_keys_list_a_frag_once_a_throw_and_leave_smokes_and_medkits_out() {
        let pockets = Pockets::new([
            Some((Pocketed::Other, 1)),
            Some((Pocketed::Frag, 2)),
            Some((Pocketed::Smoke, 1)),
            None,
            None,
            None,
        ]);
        let mut tracker = stand(Some(pockets));
        // (time, keys): the medkit, a frag, the same key again at once (its release acts too), a frag later.
        for (time, keys) in [
            (1_000, key::QUICK[0]),
            (1_100, 0),
            (2_000, key::QUICK[1]),
            (2_100, 0),
            (2_500, key::QUICK[1]),
            (2_600, 0),
            (4_000, key::QUICK[1]),
            (4_100, 0),
        ] {
            press(&mut tracker, 1, time, keys);
        }
        let presses: Vec<u32> = tracker.live_grenades().iter().map(|g| g.press).collect();
        assert_eq!(presses, [2_000, 4_000]);
        assert!(tracker.live_grenades().iter().all(|g| g.by == 1 && g.team == 1));
        // The frags are spent: the smoke is the second pocket held, quick 2 now.
        press(&mut tracker, 1, 6_000, key::QUICK[1]);
        assert_eq!(tracker.live_grenades().len(), 2);
        let counts = tracker.throw_counts();
        assert_eq!((counts.frag, counts.smoke, counts.no_pockets), (2, 1, 0));
    }

    #[test]
    fn a_humans_smoke_key_becomes_a_cloud_where_the_arc_rests_it_and_the_view_carries_it() {
        use crate::v026g::bots::smoke::{MatchMs, SmokeGrenade};
        let world = floor_world();
        let pockets = Pockets::new([Some((Pocketed::Smoke, 1)), None, None, None, None, None]);
        let mut tracker = stand(Some(pockets));
        press(&mut tracker, 1, 1_000, key::GRENADE);
        press(&mut tracker, 1, 1_050, 0);
        assert!(tracker.live_grenades().is_empty(), "no frag listed");
        assert!(tracker.clouds().is_empty(), "in the hand");
        tracker.launch_grenades(2_100, Some(&*world));
        let clouds = tracker.clouds().to_vec();
        assert_eq!(clouds.len(), 1);
        let cloud = clouds[0];
        assert_eq!((cloud.by, cloud.team, cloud.thrown), (1, 1, MatchMs(1_000)));
        // The fuse from the press (the game data's, else the RDG-M's 4 s), the rest 9-13 m ahead as a frag's.
        let smoke = SmokeGrenade::of_game_data();
        assert_eq!(cloud.pours, MatchMs(1_000 + smoke.cloud.fuse_ms));
        assert!(cloud.centre.x.abs() < 0.1 && (9.0..13.0).contains(&cloud.centre.z), "{cloud:?}");
        let counts = tracker.smoke_counts();
        assert_eq!((counts.thrown, counts.clouds, counts.dropped), ([0, 1], [0, 1], [0, 0]));
        let clock = Clock {
            clock: 6_000,
            time_left_s: 600.0,
            total_s: 900.0,
            playing: true,
        };
        assert_eq!(tracker.view(&clock, &Kinematics::default(), Some(&*world)).smoke.all(), &clouds[..]);
        // Gone at its end.
        tracker.launch_grenades(cloud.ends().0, Some(&*world));
        assert!(tracker.clouds().is_empty());
    }

    #[test]
    fn a_new_life_fills_the_pockets_and_a_human_with_none_known_lists_nothing() {
        let kin = Kinematics::default();
        let one_frag = Pockets::new([Some((Pocketed::Frag, 1)), None, None, None, None, None]);
        let mut tracker = stand(Some(one_frag));
        press(&mut tracker, 1, 1_000, key::GRENADE);
        press(&mut tracker, 1, 1_050, 0);
        press(&mut tracker, 1, 3_000, key::GRENADE);
        press(&mut tracker, 1, 3_050, 0);
        assert_eq!(tracker.live_grenades().len(), 1, "one frag, one throw");
        tracker.fixed(1, fix(4_000, 11), Some(1.0), &kin, None);
        press(&mut tracker, 1, 6_000, key::GRENADE);
        assert_eq!(tracker.live_grenades().len(), 2, "a new life, a full pocket");
        let mut tracker = Tracker::new(2, &[0, 1], &[true, false], vec![]);
        tracker.fixed(1, fix(500, 10), Some(1.0), &kin, None);
        press(&mut tracker, 1, 1_000, key::GRENADE);
        assert!(tracker.live_grenades().is_empty());
        assert_eq!(tracker.throw_counts().no_pockets, 1);
    }

    #[test]
    fn an_entry_is_live_to_a_moment_past_its_blast() {
        let g = LiveGrenade {
            by: 0,
            team: 0,
            press: 1_000,
            phase: Phase::Pressed,
            grenade: *Grenade::bots(),
        };
        assert_eq!(g.blast_at(), 5_000);
        // A grenade with another fuse goes off by its own (a patched RGD-5, or another frag).
        let short = LiveGrenade {
            grenade: Grenade {
                fuse_s: 3.0,
                ..*Grenade::bots()
            },
            ..g
        };
        assert_eq!(short.blast_at(), 4_000);
        assert!(short.live_at(4_300) && !short.live_at(4_301));
        assert!(g.live_at(5_300));
        assert!(!g.live_at(5_301));
    }
}

//! Patch 165: what each place did with each weapon in the match (the owner,
//! 2026-11-20; docs/notes/kill_weapon.md).
//!
//! The game counts a kill, a shot and a hit per place and kit and drops the
//! weapon it is told at each. This keeps a table beside the statistics: per
//! place, per weapon (its dict id), the kills (and of them those by a
//! headshot, through cover and by a ricochet), deaths with it in the hands,
//! bullets fired, hits, the pain taken off enemies, the kills' distances and
//! the time it was held. Each count the statistics have too is made where
//! they make theirs and under their tests, so a place's weapons add up to
//! its record.
//!
//! The table is the statistics' in every way: it begins where their records
//! are zeroed, is saved after their tail and loaded with them, so a rollback,
//! a resync and a late joiner have it as every game does, and the match
//! server reads it from a trusted world. A world without it (an older
//! game's, a replay's) leaves the statistics uncounted: this game then
//! writes none either, and its worlds are that game's byte for byte.

#![forbid(unsafe_code)]

use srp_validate::world::weapon_block::{encode, MAX_BLOCK};
use srp_validate::world::{PlaceWeapons, Weapons};

use crate::vostok::weapon_stats::{self as game, Fired, Held, Hit, Killed, Loading, Saving, Statistics, PLACES};
use crate::vostok::{Context, Flow, Lock, NoExit};

/// Whether the live statistics' events are counted per weapon too.
enum Counting {
    /// Not since they were last zero: they came from a world without the
    /// table (or no statistics were made yet).
    Uncounted,
    /// Since `of`'s records were zero, or from a world that had the table.
    Counted { of: Statistics, places: [PlaceWeapons; PLACES] },
}

/// What happens to the statistics.
enum Event<'a> {
    /// Their records are all zero: made, or reset between matches.
    Zeroed(Statistics),
    /// They were loaded from a world; its table, if it had one.
    Loaded(Statistics, Option<Weapons<'a>>),
    Killed(Killed),
    Hit(Hit),
    Fired(Fired),
    Held(Held),
}

impl Counting {
    fn apply(&mut self, event: Event<'_>) {
        match event {
            Event::Zeroed(of) => *self = Counting::Counted { of, places: [PlaceWeapons::NONE; PLACES] },
            Event::Loaded(of, None) => {
                // Another statistics' load says nothing of the ones counted.
                if self.places(of).is_some() {
                    *self = Counting::Uncounted;
                }
            }
            Event::Loaded(of, Some(loaded)) => {
                let mut places = [PlaceWeapons::NONE; PLACES];
                for (place, weapons) in places.iter_mut().zip(loaded.places()) {
                    *place = weapons;
                }
                *self = Counting::Counted { of, places };
            }
            Event::Killed(Killed { of, death, kill }) => {
                let Some(places) = self.places(of) else { return };
                if let Some((death, place)) = death.and_then(|death| Some((death, places.get_mut(death.place)?))) {
                    place.count(death.weapon, |counts| counts.death());
                }
                if let Some((kill, place)) = kill.and_then(|kill| Some((kill, places.get_mut(kill.place)?))) {
                    place.count(kill.weapon, |counts| counts.kill(kill.killing));
                }
            }
            Event::Hit(hit) => {
                let Some(place) = self.places(hit.of).and_then(|places| places.get_mut(hit.place)) else { return };
                place.count(hit.weapon, |counts| {
                    if hit.counted {
                        counts.hit();
                    }
                    counts.hurt(hit.pain);
                });
            }
            Event::Fired(fired) => {
                let Some(place) = self.places(fired.of).and_then(|places| places.get_mut(fired.place)) else { return };
                place.count(fired.weapon, |counts| counts.fire(fired.count));
            }
            Event::Held(held) => {
                let Some(place) = self.places(held.of).and_then(|places| places.get_mut(held.place)) else { return };
                place.count(held.weapon, |counts| counts.hold());
            }
        }
    }

    /// What follows `of`'s tail in a world: the table, while they are
    /// counted.
    fn block<'b>(&mut self, of: Statistics, out: &'b mut [u8; MAX_BLOCK]) -> Option<&'b [u8]> {
        Some(encode(self.places(of)?, out))
    }

    /// The table of `of`, while it is the statistics counted.
    fn places(&mut self, of: Statistics) -> Option<&mut [PlaceWeapons; PLACES]> {
        match self {
            Counting::Counted { of: counted, places } if *counted == of => Some(places),
            _ => None,
        }
    }
}

static COUNTING: Lock<Counting> = Lock::new(Counting::Uncounted);

fn apply(ctx: &Context<'_>, event: Option<Event<'_>>) -> Flow<NoExit> {
    if let Some(event) = event {
        COUNTING.with(ctx.thread(), |counting| counting.apply(event));
    }
    Flow::Continue
}

/// The handler's on_player_killed, before the statistics count the kill.
pub fn on_killed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    apply(ctx, Killed::at(ctx).map(Event::Killed))
}

/// generate_hit_event, before the handler's on_player_damaged.
pub fn on_hit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    apply(ctx, Hit::at(ctx).map(Event::Hit))
}

/// on_fire, before the handler's on_player_weapon_firing.
pub fn on_fired(ctx: &mut Context<'_>) -> Flow<NoExit> {
    apply(ctx, Fired::at(ctx).map(Event::Fired))
}

/// base_player::tick: a tick of whatever a live player holds.
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    apply(ctx, Held::at(ctx).map(Event::Held))
}

/// The statistics' constructor.
pub fn on_made(ctx: &mut Context<'_>) -> Flow<NoExit> {
    apply(ctx, game::made(ctx).map(Event::Zeroed))
}

/// The statistics' reset between matches.
pub fn on_reset(ctx: &mut Context<'_>) -> Flow<NoExit> {
    apply(ctx, game::reset(ctx).map(Event::Zeroed))
}

/// The statistics serialized: the table after their tail, while they are
/// counted.
pub fn on_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(saving) = Saving::at(ctx) {
        let mut out = [0; MAX_BLOCK];
        let of = saving.of();
        // The engine's append runs outside the lock.
        let len = COUNTING.with(ctx.thread(), |counting| counting.block(of, &mut out).map(<[u8]>::len));
        if let Some(block) = len.and_then(|len| out.get(..len)) {
            saving.write(block);
        }
    }
    Flow::Continue
}

/// The statistics deserialized: the table that followed their tail; a block
/// that does not read is none.
pub fn on_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    apply(ctx, Loading::at(ctx).map(|loading| {
        Event::Loaded(loading.of(), Weapons::read(loading.rest(), PLACES).ok().flatten())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::weapon_stats::{Death, Kill};
    use srp_validate::world::{Decimetres, DictId, Killing, WeaponCounts};

    const MATCH: Statistics = Statistics::numbered(1);
    const OTHER: Statistics = Statistics::numbered(2);
    /// A table of no entries: the version, the length, twenty counts of 0.
    const EMPTY: [u8; 23] = [1, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    fn fired(of: Statistics, place: usize, weapon: u16, count: u16) -> Event<'static> {
        Event::Fired(Fired { of, place, weapon: DictId::new(weapon), count })
    }

    fn hit(place: usize, weapon: u16, counted: bool, pain: f32) -> Event<'static> {
        Event::Hit(Hit { of: MATCH, place, weapon: DictId::new(weapon), counted, pain })
    }

    fn killed(victim: usize, in_hands: u16, killer: Option<(usize, u16, bool, f32)>) -> Event<'static> {
        Event::Killed(Killed {
            of: MATCH,
            death: Some(Death { place: victim, weapon: DictId::new(in_hands) }),
            kill: killer.map(|(place, weapon, headshot, metres)| Kill {
                place,
                weapon: DictId::new(weapon),
                killing: Killing { headshot, ricochet: !headshot, distance: Decimetres::of_metres(metres), ..Killing::default() },
            }),
        })
    }

    /// The block as the save writes it.
    fn block(counting: &mut Counting, of: Statistics) -> Option<Vec<u8>> {
        counting.block(of, &mut [0; MAX_BLOCK]).map(<[u8]>::to_vec)
    }

    /// Place `place`'s entries of a block.
    fn counts(block: &[u8], place: usize) -> Vec<WeaponCounts> {
        Weapons::read(block, PLACES).unwrap().unwrap().places().nth(place).unwrap().counts().to_vec()
    }

    #[test]
    fn nothing_is_counted_until_the_statistics_are_zero() {
        let mut counting = Counting::Uncounted;
        counting.apply(fired(MATCH, 0, 21, 3));
        assert_eq!(block(&mut counting, MATCH), None);
        counting.apply(Event::Zeroed(MATCH));
        assert_eq!(block(&mut counting, MATCH).as_deref(), Some(&EMPTY[..]));
        assert_eq!(block(&mut counting, OTHER), None);
    }

    #[test]
    fn each_event_is_counted_for_its_place_and_weapon() {
        let mut counting = Counting::Uncounted;
        counting.apply(Event::Zeroed(MATCH));
        counting.apply(fired(MATCH, 1, 21, 30));
        counting.apply(fired(MATCH, 1, 300, 8));
        counting.apply(fired(OTHER, 1, 300, 8));
        counting.apply(hit(1, 21, true, 0.25));
        counting.apply(hit(1, 21, false, 0.5));
        counting.apply(hit(1, 0, true, 0.125));
        counting.apply(killed(0, 44, Some((1, 21, true, 31.26))));
        counting.apply(killed(0, 0, Some((1, 500, false, 2.0))));
        counting.apply(killed(1, 21, None));
        for weapon in [21, 21, 0] {
            counting.apply(Event::Held(Held { of: MATCH, place: 1, weapon: DictId::new(weapon) }));
        }
        let saved = block(&mut counting, MATCH).unwrap();
        let victim = counts(&saved, 0);
        assert_eq!(victim.iter().map(|c| (c.dict_id(), c.deaths())).collect::<Vec<_>>(), [(0, 1), (44, 1)]);
        let shooter = counts(&saved, 1);
        assert_eq!(shooter.iter().map(WeaponCounts::dict_id).collect::<Vec<_>>(), [0, 21, 300, 500]);
        let rifle = shooter[1];
        assert_eq!((rifle.shots(), rifle.hits(), rifle.damage()), (30, 1, 7500));
        assert_eq!((rifle.kills(), rifle.headshots(), rifle.deaths()), (1, 1, 1));
        assert_eq!((rifle.wall_kills(), rifle.ricochet_kills(), shooter[3].ricochet_kills()), (0, 0, 1));
        assert_eq!((rifle.held_ms(), shooter[0].held_ms()), (100, 50));
        assert_eq!((rifle.kill_distance(), rifle.longest_kill()), (313, 313));
        assert_eq!((shooter[0].hits(), shooter[0].damage()), (1, 1250));
        assert_eq!((shooter[2].shots(), shooter[3].kills(), shooter[3].longest_kill()), (8, 1, 20));
        assert!(counts(&saved, 19).is_empty());
    }

    #[test]
    fn a_load_gives_the_worlds_table() {
        let mut source = Counting::Uncounted;
        source.apply(Event::Zeroed(MATCH));
        source.apply(fired(MATCH, 0, 21, 5));
        let world = block(&mut source, MATCH).unwrap();
        // A rollback: what was counted since the world is gone.
        source.apply(fired(MATCH, 0, 300, 9));
        source.apply(fired(MATCH, 1, 300, 9));
        source.apply(Event::Loaded(MATCH, Weapons::read(&world, PLACES).unwrap()));
        assert_eq!(block(&mut source, MATCH).unwrap(), world);
        // A game that counted nothing has the world's table from its load on.
        let mut joiner = Counting::Uncounted;
        joiner.apply(Event::Loaded(OTHER, Weapons::read(&world, PLACES).unwrap()));
        joiner.apply(fired(OTHER, 0, 21, 1));
        assert_eq!(counts(&block(&mut joiner, OTHER).unwrap(), 0)[0].shots(), 6);
    }

    #[test]
    fn a_world_without_the_table_leaves_the_statistics_uncounted() {
        let mut counting = Counting::Uncounted;
        counting.apply(Event::Zeroed(MATCH));
        counting.apply(fired(MATCH, 0, 21, 5));
        // Another statistics' load changes nothing of this one's.
        counting.apply(Event::Loaded(OTHER, None));
        assert!(block(&mut counting, MATCH).is_some());
        counting.apply(Event::Loaded(MATCH, None));
        counting.apply(fired(MATCH, 0, 21, 5));
        assert_eq!(block(&mut counting, MATCH), None);
        // Until they are zero again.
        counting.apply(Event::Zeroed(MATCH));
        assert_eq!(block(&mut counting, MATCH).as_deref(), Some(&EMPTY[..]));
    }
}

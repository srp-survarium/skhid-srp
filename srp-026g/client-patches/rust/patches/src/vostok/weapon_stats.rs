//! What patch 165 (`weapon_stats`) reads of the game: the three events the
//! statistics count per kit, each with the weapon the game knows there and
//! drops, and the statistics' save, load and resets (kill_weapon.md).
//!
//! Each event is read under the statistics' own tests, so that what the
//! patch counts for a place adds up to the place's record:
//! - a kill (`shared_statistics::on_player_killed` 0x2F4EB0): event 2 for
//!   the victim, always (0x2F4F4E, 0x2F4FA4); events 0 and 15 for a killer
//!   that is a place, not the victim, of the other team (0x2F5028..0x2F50C7);
//! - a hit (`on_player_damaged` 0x2F5450): event 23 for an attacker that is
//!   a place of another team than the victim's, the call's flag set, no
//!   stock hit;
//! - a shot (`on_player_weapon_firing` 0x2F5500): event 22 + the bullets.
//!
//! A fourth thing the game counts nowhere: the ticks a live player has a
//! weapon in its hands, read at `base_player::tick`.
//!
//! The game's records take any place and kit byte; only a place of the 20
//! and a kit of the 3 is a count of a record's, and only those are read here.

use core::marker::PhantomData;

use srp_validate::world::{Decimetres, DictId, Killing};

use super::context::Context;
use super::hook::Thread;
use super::inventory::SLOTS;
use super::world::{floats, read};
use crate::raw;

/// The places a statistics has records of, and a world's statistics hold:
/// serialize and deserialize walk the twenty whatever the match's count
/// (patch_026g.py's STATISTICS_COUNT and its four compares).
pub const PLACES: usize = 20;
const KITS: u8 = 3;

/// Which shared statistics: its address (an id, never read through).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Statistics(u32);

impl Statistics {
    fn of(shared: raw::SharedStatistics) -> Self {
        Self(shared.address())
    }

    #[cfg(test)]
    pub const fn numbered(number: u32) -> Self {
        Self(number)
    }
}

/// A place with a record; none for the byte of nobody (0FFh).
fn place(place: u8) -> Option<usize> {
    (usize::from(place) < PLACES).then_some(usize::from(place))
}

/// The team of `place` in the statistics' match (0x2F4EE7).
///
/// # Safety
/// `shared` is alive on the hook's thread.
unsafe fn team(shared: raw::SharedStatistics, place: usize) -> Option<u32> {
    // SAFETY: the caller's; the options hold the 20 places' profiles.
    unsafe { Some(read(shared.options()?.first_place().at(place).team())) }
}

/// The dict id of the item in `player`'s hands: the item it holds, when
/// that is one of its inventory's (a slot holds its held object + 10h: the
/// bullet's constructor takes a weapon's item so, 0x32BB12). None with
/// nothing held, with a thing in the hands that no slot has (a battery), or
/// for an item with no dictionary entry.
///
/// # Safety
/// `player` is alive on the hook's thread.
unsafe fn in_hands(player: raw::Player) -> Option<DictId> {
    // SAFETY: the caller's: the live player's inventory, whose 23 slots each
    // hold an item or 0, every item with its dictionary entry at +25Ch
    // (update_weight 0x2FC71C reads it of each). The held object is only
    // compared, never read.
    unsafe {
        let held = player.held_item()?.inventory_item();
        let slots = player.inventory()?.slots();
        let item = (0..usize::from(SLOTS)).filter_map(|slot| slots.at(slot).item()).find(|&item| item == held)?;
        DictId::new(read(item.entry()?.dict_id()))
    }
}

/// A place died.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Death {
    pub place: usize,
    /// The weapon in its hands.
    pub weapon: Option<DictId>,
}

/// A place killed an enemy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kill {
    pub place: usize,
    /// The damage source: the bullet's weapon, a grenade, a trap.
    pub weapon: Option<DictId>,
    pub killing: Killing,
}

/// The statistics are told of a death, at `weapon_stats.killed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Killed {
    pub of: Statistics,
    /// The victim's count of event 2.
    pub death: Option<Death>,
    /// The killer's of event 0 (and 15).
    pub kill: Option<Kill>,
}

impl Killed {
    pub fn at(ctx: &Context<'_>) -> Option<Killed> {
        use raw::site::weapon_stats_killed as site;
        // SAFETY: the context is this site's: the handler and its 17
        // arguments, alive while on_player_killed runs on the game's thread;
        // the two positions are float3s of the caller's (the victim's
        // transform, a copy of the killer's on its stack).
        unsafe {
            let (handler, frame) = (site::handler(ctx)?, site::frame(ctx)?);
            let shared = handler.shared();
            let (killer, victim) = (read(frame.killer()), read(frame.victim()));
            let death = place(victim).filter(|_| read(frame.victim_kit()) < KITS).map(|place| Death {
                place,
                weapon: victim_player(handler, place).and_then(|player| in_hands(player)),
            });
            let enemy = killer != victim && read(frame.killer_kit()) < KITS;
            let kill = place(killer).zip(place(victim)).filter(|_| enemy).and_then(|(by, of)| {
                // A bullet's two counts, as the kill feed reads them (patch 67): its changes of
                // course, one a pierce and one a bounce, and of them its bounces. Not the stock's.
                let (changes, bounces) = frame
                    .bullet()
                    .filter(|_| read(frame.melee()) == 0)
                    .map_or((0, 0), |bullet| (read(bullet.segment()), u32::from(read(bullet.ricochets()))));
                (team(shared, by)? != team(shared, of)?).then(|| Kill {
                    place: by,
                    weapon: DictId::new(read(frame.dict_id())),
                    killing: Killing {
                        headshot: read(frame.headshot()) != 0,
                        through_cover: changes > bounces,
                        ricochet: bounces > 0,
                        distance: distance(frame.killer_position(), frame.victim_position()),
                    },
                })
            });
            Some(Killed { of: Statistics::of(shared), death, kill })
        }
    }
}

/// `place`'s player in the handler's match, as generate_killed_event finds
/// the killer's (0x2DA183..0x2DA19F): the place's kit's.
///
/// # Safety
/// `handler` is alive on the hook's thread.
unsafe fn victim_player(handler: raw::BaseGameStatisticsHandler, place: usize) -> Option<raw::Player> {
    // SAFETY: the handler's world core and its places' vector, the place
    // bounded by the vector's end before its entry is read, its kit by the
    // entry's three players.
    unsafe {
        let core = handler.core()?;
        let (first, end) = (core.places()?, core.places_end()?);
        if end.index_from(first)? <= place {
            return None;
        }
        let entry = first.at(place);
        let kit = read(entry.kit());
        (kit < KITS).then(|| entry.kits().at(usize::from(kit)).player())?
    }
}

/// Between two float3s, in metres (the game's positions are metres:
/// fall_damage.md's heights and its gravity of 9.8). None without a killer's
/// position: generate_killed_event gives zeros for a place with no player
/// (0x2DA28E).
///
/// # Safety
/// Each pointer is null or a float3 alive for the hook.
unsafe fn distance(killer: *mut f32, victim: *mut f32) -> Option<Decimetres> {
    if killer.is_null() || victim.is_null() {
        return None;
    }
    // SAFETY: the caller's.
    let (from, to): ([f32; 3], [f32; 3]) = unsafe { (floats(killer), floats(victim)) };
    if from == [0.0; 3] {
        return None;
    }
    let d = [from[0] - to[0], from[1] - to[1], from[2] - to[2]];
    Decimetres::of_metres(ragdoll::math::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]))
}

/// A hit that took health is reported, at `weapon_stats.hit`: what the
/// attacker's place counts of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub of: Statistics,
    pub place: usize,
    /// The bullet's weapon; none for a hit with no bullet (a grenade, a trap).
    pub weapon: Option<DictId>,
    /// Whether the statistics count it in event 23.
    pub counted: bool,
    /// The pain it took off the victim, an enemy.
    pub pain: f32,
}

impl Hit {
    /// None for a hit no place made on a place of another team, and where
    /// the match has no statistics.
    pub fn at(ctx: &Context<'_>) -> Option<Hit> {
        use raw::site::weapon_stats_hit as site;
        // SAFETY: the context is this site's: the victim, the attacker's
        // player and the bullet (each alive or 0) and generate_hit_event's
        // frame, on the game's thread; the bullet's weapon item is the one
        // the function itself reads the slot of (0x2D9F97).
        unsafe {
            let frame = site::frame(ctx)?;
            let shared = site::victim(ctx)?.world_core()?.statistics()?.shared();
            let by = place(read(frame.attacker()))?;
            let victim = place(site::victim_place(ctx) as u8)?;
            if team(shared, by)? == team(shared, victim)? {
                return None;
            }
            // The kit the handler is given: the attacker's player's, 0 with none (0x2DA127).
            let kit = site::attacker(ctx).map_or(0, |player| read(player.kit()));
            let weapon = site::bullet(ctx)
                .and_then(|bullet| bullet.weapon_item())
                .and_then(|item| item.entry())
                .and_then(|entry| DictId::new(read(entry.dict_id())));
            Some(Hit {
                of: Statistics::of(shared),
                place: by,
                weapon,
                counted: kit < KITS && read(frame.counted()) != 0 && read(frame.melee()) == 0,
                pain: read(frame.pain()),
            })
        }
    }
}

/// A weapon fired, at `weapon_stats.fired`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fired {
    pub of: Statistics,
    pub place: usize,
    pub weapon: Option<DictId>,
    /// The bullets: a shot's pellets.
    pub count: u16,
}

impl Fired {
    /// None where the match has no statistics.
    pub fn at(ctx: &Context<'_>) -> Option<Fired> {
        use raw::site::weapon_stats_fired as site;
        // SAFETY: the context is this site's: on_fire's player (its +110h
        // interface in ESI) and its argument, on the game's thread.
        unsafe {
            let player = site::user(ctx)?.player();
            let shared = player.world_core()?.statistics()?.shared();
            let by = place(read(player.place())).filter(|_| read(player.kit()) < KITS)?;
            Some(Fired {
                of: Statistics::of(shared),
                place: by,
                weapon: in_hands(player),
                count: read(site::frame(ctx)?.count()),
            })
        }
    }
}

/// A live player ticked, at `battery.carrier_tick` (base_player::tick, once
/// a simulation tick for each player in the world): what is in its hands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held {
    pub of: Statistics,
    pub place: usize,
    pub weapon: Option<DictId>,
}

impl Held {
    /// None for a dead player, and where the match has no statistics.
    pub fn at(ctx: &Context<'_>) -> Option<Held> {
        // SAFETY: the context is this site's: the player base_player::tick
        // ticks, alive on the game's thread.
        unsafe {
            let player = raw::site::battery_carrier_tick::player(ctx)?;
            if read(player.alive()) == 0 {
                return None;
            }
            let shared = player.world_core()?.statistics()?.shared();
            let by = place(read(player.place()))?;
            Some(Held { of: Statistics::of(shared), place: by, weapon: in_hands(player) })
        }
    }
}

/// At `weapon_stats.made` and `weapon_stats.reset`: the statistics whose
/// records are all zero now.
pub fn made(ctx: &Context<'_>) -> Option<Statistics> {
    // SAFETY: the constructor's statistics.
    unsafe { raw::site::weapon_stats_made::statistics(ctx) }.map(Statistics::of)
}

pub fn reset(ctx: &Context<'_>) -> Option<Statistics> {
    // SAFETY: the reset's statistics.
    unsafe { raw::site::weapon_stats_reset::statistics(ctx) }.map(Statistics::of)
}

/// The statistics serialized, their tail written, at `weapon_stats.save`.
pub struct Saving<'t> {
    shared: raw::SharedStatistics,
    writer: raw::BufferWriter,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Saving<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<Saving<'t>> {
        use raw::site::weapon_stats_save as site;
        // SAFETY: serialize's statistics and live writer.
        let (shared, writer) = unsafe { (site::statistics(ctx)?, site::writer(ctx)?) };
        Some(Saving { shared, writer, thread: PhantomData })
    }

    pub fn of(&self) -> Statistics {
        Statistics::of(self.shared)
    }

    /// Writes `block` after the tail, with the writer's own append (0x7AC10).
    pub fn write(self, block: &[u8]) {
        // SAFETY: serialize's live writer; the append grows its buffer.
        unsafe { raw::engine_append_packet_bytes(Some(self.writer), block.as_ptr(), block.len() as u32) }
    }
}

/// The statistics deserialized, their tail read, at `weapon_stats.load`.
pub struct Loading<'t> {
    shared: raw::SharedStatistics,
    rest: &'t [u8],
}

impl<'t> Loading<'t> {
    /// The reader's bytes after the tail are taken: they are the statistics
    /// section's to its end (world apply 0x2D2AEE gives deserialize a reader
    /// of the section alone).
    pub fn at(ctx: &'t Context<'_>) -> Option<Loading<'t>> {
        use raw::site::weapon_stats_load as site;
        // SAFETY: deserialize's statistics and live reader {begin, cursor,
        // size}; the bytes from its cursor to its end are the world's, not
        // written while the hook runs.
        unsafe {
            let (shared, reader) = (site::statistics(ctx)?, site::reader(ctx)?);
            let (begin, cursor) = (reader.begin() as usize, reader.cursor() as usize);
            let end = begin.checked_add(read(reader.size()) as usize)?;
            let rest = if cursor >= begin && cursor <= end {
                reader.set_cursor(end as *mut u8);
                core::slice::from_raw_parts(cursor as *const u8, end - cursor)
            } else {
                &[]
            };
            Some(Loading { shared, rest })
        }
    }

    pub fn of(&self) -> Statistics {
        Statistics::of(self.shared)
    }

    /// What followed the tail.
    pub fn rest(&self) -> &'t [u8] {
        self.rest
    }
}

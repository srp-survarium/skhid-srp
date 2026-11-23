//! The per-weapon counts a game keeps beside the statistics (exe patch 165,
//! `weapon_stats`; kill_weapon.md section 4), written after the statistics'
//! tail:
//!
//! ```text
//! u8  version            1
//! u16 length             the bytes after it: to the section's end
//! per roster place:
//!     u8 n               at most 16
//!     n x  u16 dict_id        rising; 0 (first): no weapon known
//!          u16 kills          enemy kills made with it        (event 0)
//!          u16 headshots      of them, by a headshot          (event 15)
//!          u16 wall_kills     of them, by a bullet that had pierced something
//!          u16 ricochet_kills of them, by a bullet that had bounced
//!          u16 deaths         deaths with it in the hands     (event 2)
//!          u16 shots          bullets fired, a pellet each    (event 22)
//!          u16 hits           bullet hits on an enemy         (event 23)
//!          u32 damage         pain taken off enemies, in 1/10000 of a pool
//!          u32 kill_distance  decimetres, summed over its kills
//!          u16 longest_kill   decimetres
//!          u32 held_ms        the time it was in the place's hands, alive
//! ```
//!
//! This type is the block's one definition: the game keeps its table in it
//! ([`PlaceWeapons::count`]), writes it with [`encode`] and loads it with
//! [`Weapons::read`]; the servers read it with the same.
//!
//! The domains:
//!
//! - the version is 1, the length the rest of the section;
//! - a place has at most 16 entries, their dict ids rising (none twice); a
//!   seventeenth weapon is counted with "no weapon known", which always has
//!   room: 16 entries begin with it;
//! - of an entry: the headshots, the kills through cover and those by a
//!   ricochet are kills, the longest kill is no longer than the kills'
//!   distances together, and those no more than the longest for each kill
//!   (while a weapon has under 65536 kills in a match: the counts wrap as
//!   the game's own u16 do); the time held is whole simulation ticks;
//! - a place's sums are its record's events over the three kits, as far as
//!   the record says: kills, headshots and deaths modulo 256 (the record
//!   carries a byte a kit, snapshot_rules.md 3.1), shots and hits modulo
//!   65536 ([`Totals`]);
//! - a dict id is an item of the dictionary, where the dictionary is known
//!   (the server; a game counts under the ids of its own items).
//!
//! Open: the damage (the game's own record of it is an f32 a victim and is
//! not this sum); how long a weapon was held (the game counts no such time).

use core::num::NonZeroU16;

use crate::MAX_PLAYERS;
use crate::reader::{Reader, Short};

/// The entries a place has at most, "no weapon known" among them.
pub const WEAPONS: usize = 16;
/// The block's version.
pub const VERSION: u8 = 1;
/// An entry's bytes.
const ENTRY: usize = 30;
/// A simulation tick, in ms (`game_world_core`'s step, match.md section 1).
pub const TICK_MS: u32 = 50;
/// The bytes before the places: the version and the length.
const HEAD: usize = 3;
/// The most bytes a block has: twenty places of sixteen entries.
pub const MAX_BLOCK: usize = HEAD + MAX_PLAYERS * (1 + WEAPONS * ENTRY);

/// An item's id in the items dictionary: never 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DictId(NonZeroU16);

impl DictId {
    /// `None` for 0, which no item has.
    pub fn new(id: u16) -> Option<Self> {
        NonZeroU16::new(id).map(Self)
    }

    pub fn get(self) -> u16 {
        self.0.get()
    }
}

/// A distance in whole decimetres, at most 6553.5 m.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Decimetres(u16);

impl Decimetres {
    /// `metres` to the nearest decimetre; `None` for a distance that is no
    /// number or under 0.
    pub fn of_metres(metres: f32) -> Option<Self> {
        // `as` truncates toward 0 and saturates.
        (metres.is_finite() && metres >= 0.0).then_some(Self((metres * 10.0 + 0.5) as u16))
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

/// Why a block does not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// It runs short, or its length is not the bytes it has.
    Unread,
    /// A version this code does not know.
    Version(u8),
    /// A field of `place`'s entries out of its domain.
    Field { place: usize, field: &'static str },
}

impl From<Short> for Fault {
    fn from(_: Short) -> Self {
        Fault::Unread
    }
}

/// How an enemy was killed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Killing {
    pub headshot: bool,
    /// The bullet had pierced something before it killed.
    pub through_cover: bool,
    /// The bullet had bounced before it killed.
    pub ricochet: bool,
    /// From the killer to the victim; none: the killer had no position.
    pub distance: Option<Decimetres>,
}

/// What one place did with one weapon in the match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponCounts {
    weapon: Option<DictId>,
    kills: u16,
    headshots: u16,
    wall_kills: u16,
    ricochet_kills: u16,
    deaths: u16,
    shots: u16,
    hits: u16,
    damage: u32,
    kill_distance: u32,
    longest_kill: u16,
    held_ms: u32,
}

impl WeaponCounts {
    const fn new(weapon: Option<DictId>) -> Self {
        Self {
            weapon,
            kills: 0,
            headshots: 0,
            wall_kills: 0,
            ricochet_kills: 0,
            deaths: 0,
            shots: 0,
            hits: 0,
            damage: 0,
            kill_distance: 0,
            longest_kill: 0,
            held_ms: 0,
        }
    }

    /// The weapon; `None`: no weapon known (a hit with no bullet, a death
    /// with nothing in the hands), or a weapon past the place's sixteenth.
    pub fn weapon(&self) -> Option<DictId> {
        self.weapon
    }

    /// The entry's key on the wire: the dict id, 0 for no weapon.
    pub fn dict_id(&self) -> u16 {
        self.weapon.map_or(0, DictId::get)
    }

    pub fn kills(&self) -> u16 {
        self.kills
    }

    pub fn headshots(&self) -> u16 {
        self.headshots
    }

    /// Of the kills, those whose bullet had pierced something (a wall, a
    /// prop, another player) before it killed.
    pub fn wall_kills(&self) -> u16 {
        self.wall_kills
    }

    /// Of the kills, those whose bullet had bounced before it killed.
    pub fn ricochet_kills(&self) -> u16 {
        self.ricochet_kills
    }

    pub fn deaths(&self) -> u16 {
        self.deaths
    }

    pub fn shots(&self) -> u16 {
        self.shots
    }

    pub fn hits(&self) -> u16 {
        self.hits
    }

    /// The pain its hits took off enemies, in ten-thousandths of a pain
    /// pool (the pool's maximum is 1.0).
    pub fn damage(&self) -> u32 {
        self.damage
    }

    /// The killer-to-victim distances of its kills, added up, in
    /// decimetres: over the kills, the average.
    pub fn kill_distance(&self) -> u32 {
        self.kill_distance
    }

    /// Its longest kill, in decimetres.
    pub fn longest_kill(&self) -> u16 {
        self.longest_kill
    }

    /// The time it was in the place's hands while the place was alive, in
    /// ms: a tick ([`TICK_MS`]) for each it was.
    pub fn held_ms(&self) -> u32 {
        self.held_ms
    }

    /// An enemy killed with it.
    pub fn kill(&mut self, killing: Killing) {
        self.kills = self.kills.wrapping_add(1);
        for (count, counted) in [
            (&mut self.headshots, killing.headshot),
            (&mut self.wall_kills, killing.through_cover),
            (&mut self.ricochet_kills, killing.ricochet),
        ] {
            *count = count.wrapping_add(u16::from(counted));
        }
        if let Some(Decimetres(distance)) = killing.distance {
            self.kill_distance = self.kill_distance.saturating_add(u32::from(distance));
            self.longest_kill = self.longest_kill.max(distance);
        }
    }

    /// The place died with it in its hands.
    pub fn death(&mut self) {
        self.deaths = self.deaths.wrapping_add(1);
    }

    /// A simulation tick with it in the place's hands, the place alive.
    pub fn hold(&mut self) {
        self.held_ms = self.held_ms.saturating_add(TICK_MS);
    }

    /// `count` bullets fired with it.
    pub fn fire(&mut self, count: u16) {
        self.shots = self.shots.wrapping_add(count);
    }

    /// A bullet of it hit an enemy.
    pub fn hit(&mut self) {
        self.hits = self.hits.wrapping_add(1);
    }

    /// A hit of it took `pain` (pain pools) off an enemy: counted to the
    /// nearest ten-thousandth; nothing for a pain that is no number or not
    /// over 0.
    pub fn hurt(&mut self, pain: f32) {
        if pain.is_finite() && pain > 0.0 {
            // `as` truncates toward 0 and saturates.
            self.damage = self.damage.saturating_add((pain * 10_000.0 + 0.5) as u32);
        }
    }

    fn read(reader: &mut Reader) -> Result<Self, Short> {
        Ok(Self {
            weapon: DictId::new(reader.u16()?),
            kills: reader.u16()?,
            headshots: reader.u16()?,
            wall_kills: reader.u16()?,
            ricochet_kills: reader.u16()?,
            deaths: reader.u16()?,
            shots: reader.u16()?,
            hits: reader.u16()?,
            damage: reader.u32()?,
            kill_distance: reader.u32()?,
            longest_kill: reader.u16()?,
            held_ms: reader.u32()?,
        })
    }

    /// What of the entry is out of its domain.
    fn fault(&self) -> Option<&'static str> {
        let longest = u32::from(self.longest_kill);
        if self.headshots > self.kills {
            Some("headshots (over its kills)")
        } else if self.wall_kills > self.kills {
            Some("wall_kills (over its kills)")
        } else if self.ricochet_kills > self.kills {
            Some("ricochet_kills (over its kills)")
        } else if self.held_ms.checked_rem(TICK_MS) != Some(0) {
            Some("held_ms (no whole ticks)")
        } else if longest > self.kill_distance {
            Some("longest_kill (over its kills' distance)")
        } else if self.kill_distance > u32::from(self.kills).saturating_mul(longest) {
            Some("kill_distance (over its kills at the longest)")
        } else {
            None
        }
    }

    fn write(&self, out: &mut Writer) {
        out.put(&self.dict_id().to_le_bytes());
        for count in [
            self.kills,
            self.headshots,
            self.wall_kills,
            self.ricochet_kills,
            self.deaths,
            self.shots,
            self.hits,
        ] {
            out.put(&count.to_le_bytes());
        }
        out.put(&self.damage.to_le_bytes());
        out.put(&self.kill_distance.to_le_bytes());
        out.put(&self.longest_kill.to_le_bytes());
        out.put(&self.held_ms.to_le_bytes());
    }
}

/// A place's counts in its record's own events, over the three kits, as far
/// as the record carries them: what its weapons' sums are.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    /// Events 0, 15 and 2: a byte a kit.
    pub kills: u8,
    pub headshots: u8,
    pub deaths: u8,
    /// Events 22 and 23: a u16 a kit.
    pub shots: u16,
    pub hits: u16,
}

/// One place's weapons: at most [`WEAPONS`] entries in rising dict id, "no
/// weapon known" first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceWeapons {
    len: u8,
    counts: [WeaponCounts; WEAPONS],
}

impl PlaceWeapons {
    pub const NONE: Self = Self {
        len: 0,
        counts: [WeaponCounts::new(None); WEAPONS],
    };

    /// The entries, in rising dict id.
    pub fn counts(&self) -> &[WeaponCounts] {
        self.counts.get(..usize::from(self.len)).unwrap_or_default()
    }

    /// Counts something of `weapon`, its entry made when it has none yet. A
    /// weapon with no entry and no room for one (fifteen others, or sixteen
    /// with "no weapon known") is counted as no weapon known.
    pub fn count(&mut self, weapon: Option<DictId>, event: impl FnOnce(&mut WeaponCounts)) {
        let weapon = weapon.filter(|&item| self.at(Some(item)).is_ok() || self.room_for_an_item());
        let at = match self.at(weapon) {
            Ok(at) => at,
            Err(at) => {
                if self.insert(at, WeaponCounts::new(weapon)).is_none() {
                    return;
                }
                at
            }
        };
        if let Some(counts) = self.counts.get_mut(at) {
            event(counts);
        }
    }

    /// Where `weapon`'s entry is, or where it goes.
    fn at(&self, weapon: Option<DictId>) -> Result<usize, usize> {
        self.counts().binary_search_by_key(&weapon, WeaponCounts::weapon)
    }

    /// Whether one more item fits beside the entry of no weapon, there or
    /// not yet.
    fn room_for_an_item(&self) -> bool {
        let unknown = self.counts().first().is_some_and(|first| first.weapon.is_none());
        usize::from(self.len) < if unknown { WEAPONS } else { WEAPONS.saturating_sub(1) }
    }

    fn insert(&mut self, at: usize, counts: WeaponCounts) -> Option<()> {
        let len = usize::from(self.len);
        if len >= WEAPONS || at > len {
            return None;
        }
        let mut to = len;
        while to > at {
            let from = to.checked_sub(1)?;
            *self.counts.get_mut(to)? = *self.counts.get(from)?;
            to = from;
        }
        *self.counts.get_mut(at)? = counts;
        self.len = self.len.checked_add(1)?;
        Some(())
    }

    fn read(reader: &mut Reader, place: usize) -> Result<Self, Fault> {
        let field = |field| Fault::Field { place, field };
        let len = reader.u8()?;
        if usize::from(len) > WEAPONS {
            return Err(field("n (over 16)"));
        }
        let mut read = Self::NONE;
        let mut before: Option<Option<DictId>> = None;
        for slot in read.counts.iter_mut().take(usize::from(len)) {
            let counts = WeaponCounts::read(reader)?;
            if before.is_some_and(|before| before >= counts.weapon) {
                return Err(field("dict_id (not rising)"));
            }
            if let Some(fault) = counts.fault() {
                return Err(field(fault));
            }
            before = Some(counts.weapon);
            *slot = counts;
        }
        read.len = len;
        if usize::from(len) == WEAPONS && read.counts().first().is_some_and(|first| first.weapon.is_some()) {
            return Err(field("n (16 with no entry of no weapon)"));
        }
        Ok(read)
    }

    /// What of the entries' sums is not the record's `totals`.
    fn disagrees(&self, totals: &Totals) -> Option<&'static str> {
        let sum = |count: fn(&WeaponCounts) -> u16| {
            self.counts().iter().fold(0u16, |sum, counts| sum.wrapping_add(count(counts)))
        };
        // `as`: the record's byte, the count modulo 256.
        if sum(WeaponCounts::kills) as u8 != totals.kills {
            Some("kills (not the record's event 0)")
        } else if sum(WeaponCounts::headshots) as u8 != totals.headshots {
            Some("headshots (not the record's event 15)")
        } else if sum(WeaponCounts::deaths) as u8 != totals.deaths {
            Some("deaths (not the record's event 2)")
        } else if sum(WeaponCounts::shots) != totals.shots {
            Some("shots (not the record's event 22)")
        } else if sum(WeaponCounts::hits) != totals.hits {
            Some("hits (not the record's event 23)")
        } else {
            None
        }
    }
}

/// A cursor over a buffer that drops what does not fit (nothing does not:
/// [`encode`]'s buffer holds the largest block).
struct Writer<'b> {
    out: &'b mut [u8],
    at: usize,
}

impl Writer<'_> {
    fn put(&mut self, bytes: &[u8]) {
        let end = self.at.saturating_add(bytes.len());
        if let Some(room) = self.out.get_mut(self.at..end) {
            room.copy_from_slice(bytes);
            self.at = end;
        }
    }
}

/// The block of `places`' weapons, by roster place (at most 20 are
/// written), as a game writes it after the statistics' tail.
pub fn encode<'b>(places: &[PlaceWeapons], out: &'b mut [u8; MAX_BLOCK]) -> &'b [u8] {
    let mut writer = Writer { out: out.as_mut_slice(), at: HEAD };
    for place in places.iter().take(MAX_PLAYERS) {
        writer.put(&[place.len]);
        for counts in place.counts() {
            counts.write(&mut writer);
        }
    }
    let end = writer.at;
    let length = u16::try_from(end.saturating_sub(HEAD)).unwrap_or(u16::MAX);
    writer.at = 0;
    writer.put(&[VERSION]);
    writer.put(&length.to_le_bytes());
    let written: &'b [u8] = out;
    written.get(..end).unwrap_or_default()
}

/// A world's weapons block, its places' entries borrowed from the world:
/// each read (and its domains held) by [`Weapons::read`], handed out one
/// place at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weapons<'a> {
    places: usize,
    entries: &'a [u8],
}

impl<'a> Weapons<'a> {
    /// The block in `bytes`, what follows the statistics' tail to the
    /// section's end, of a match of `places` places. `Ok(None)`: no bytes,
    /// the world of a game that keeps no such counts.
    pub fn read(bytes: &'a [u8], places: usize) -> Result<Option<Self>, Fault> {
        if bytes.is_empty() {
            return Ok(None);
        }
        let mut reader = Reader::new(bytes, 0);
        let version = reader.u8()?;
        if version != VERSION {
            return Err(Fault::Version(version));
        }
        let length = usize::from(reader.u16()?);
        let entries = reader.rest();
        if length != entries.len() || places > MAX_PLAYERS {
            return Err(Fault::Unread);
        }
        for place in 0..places {
            PlaceWeapons::read(&mut reader, place)?;
        }
        if !reader.is_done() {
            return Err(Fault::Unread);
        }
        Ok(Some(Self { places, entries }))
    }

    /// The places' weapons, by roster place.
    pub fn places(&self) -> impl Iterator<Item = PlaceWeapons> + 'a {
        let mut reader = Reader::new(self.entries, 0);
        (0..self.places).map_while(move |place| PlaceWeapons::read(&mut reader, place).ok())
    }

    /// Whether every place's sums are its record's `totals` (by place);
    /// else the first that is not.
    pub fn agree(&self, totals: &[Totals]) -> Result<(), Fault> {
        for (place, (weapons, totals)) in self.places().zip(totals).enumerate() {
            if let Some(field) = weapons.disagrees(totals) {
                return Err(Fault::Field { place, field });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn id(id: u16) -> Option<DictId> {
        DictId::new(id)
    }

    fn block(places: &[PlaceWeapons]) -> Vec<u8> {
        encode(places, &mut [0; MAX_BLOCK]).to_vec()
    }

    #[test]
    fn a_place_keeps_its_weapons_in_rising_dict_id() {
        let mut place = PlaceWeapons::NONE;
        place.count(id(300), |counts| counts.fire(8));
        place.count(id(7), WeaponCounts::hit);
        place.count(None, WeaponCounts::death);
        let from = |metres| Killing { distance: Decimetres::of_metres(metres), ..Killing::default() };
        place.count(id(300), |counts| counts.kill(Killing { headshot: true, through_cover: true, ..from(25.04) }));
        place.count(id(300), |counts| counts.kill(Killing { through_cover: true, ricochet: true, ..from(100.0) }));
        place.count(id(300), |counts| counts.kill(Killing::default()));
        place.count(id(7), |counts| counts.hurt(0.12345));
        place.count(id(7), WeaponCounts::hold);
        place.count(id(7), WeaponCounts::hold);
        let ids: Vec<u16> = place.counts().iter().map(WeaponCounts::dict_id).collect();
        assert_eq!(ids, [0, 7, 300]);
        let rifle = place.counts()[2];
        assert_eq!((rifle.shots(), rifle.kills(), rifle.headshots()), (8, 3, 1));
        assert_eq!((rifle.wall_kills(), rifle.ricochet_kills()), (2, 1));
        assert_eq!(place.counts()[1].held_ms(), 100);
        assert_eq!((rifle.kill_distance(), rifle.longest_kill()), (250 + 1000, 1000));
        assert_eq!((place.counts()[1].hits(), place.counts()[1].damage()), (1, 1235));
        assert_eq!(place.counts()[0].deaths(), 1);
    }

    #[test]
    fn a_seventeenth_weapon_is_no_weapon_known() {
        let mut place = PlaceWeapons::NONE;
        for weapon in 1..=40 {
            place.count(id(weapon), |counts| counts.fire(1));
        }
        // Fifteen items and the entry of no weapon, which took the other 25.
        let ids: Vec<u16> = place.counts().iter().map(WeaponCounts::dict_id).collect();
        assert_eq!(ids, (0..=15).collect::<Vec<_>>());
        assert_eq!(place.counts()[0].shots(), 25);
        // With the entry of no weapon there first, fifteen items still fit.
        let mut place = PlaceWeapons::NONE;
        place.count(None, WeaponCounts::death);
        for weapon in 1..=40 {
            place.count(id(weapon), |counts| counts.fire(1));
        }
        assert_eq!(place.counts().len(), WEAPONS);
        assert_eq!(place.counts()[0].shots(), 25);
        assert_eq!(Weapons::read(&block(&[place]), 1).map(|read| read.is_some()), Ok(true));
    }

    #[test]
    fn a_distance_and_a_pain_are_counted_to_the_nearest_step() {
        assert_eq!(Decimetres::of_metres(0.04).map(Decimetres::get), Some(0));
        assert_eq!(Decimetres::of_metres(0.05).map(Decimetres::get), Some(1));
        assert_eq!(Decimetres::of_metres(1.0e9).map(Decimetres::get), Some(u16::MAX));
        assert_eq!(Decimetres::of_metres(-1.0), None);
        assert_eq!(Decimetres::of_metres(f32::NAN), None);
        let mut counts = WeaponCounts::new(None);
        for pain in [f32::NAN, f32::INFINITY, -0.5, 0.0] {
            counts.hurt(pain);
        }
        assert_eq!(counts.damage(), 0);
        counts.hurt(1.0e9);
        counts.hurt(1.0);
        assert_eq!(counts.damage(), u32::MAX);
    }

    #[test]
    fn a_block_reads_as_it_was_written() {
        let mut places = [PlaceWeapons::NONE; 3];
        places[0].count(id(21), |counts| counts.fire(30));
        places[0].count(id(21), |counts| {
            counts.kill(Killing { headshot: true, distance: Decimetres::of_metres(12.3), ..Killing::default() })
        });
        places[2].count(None, WeaponCounts::death);
        places[2].count(id(5), WeaponCounts::hit);
        let bytes = block(&places);
        assert_eq!(bytes.len(), 3 + 3 + 3 * ENTRY);
        assert_eq!(&bytes[..3], &[1, 93, 0]);
        let read = Weapons::read(&bytes, 3).unwrap().unwrap();
        assert_eq!(read.places().collect::<Vec<_>>(), places);
        // An empty table is a block too; no bytes are no block.
        assert_eq!(block(&[PlaceWeapons::NONE; 2]), [1, 2, 0, 0, 0]);
        assert_eq!(Weapons::read(&[], 3), Ok(None));
        // Short, long, of other places, of another version.
        for len in 1..bytes.len() {
            assert!(Weapons::read(&bytes[..len], 3).is_err(), "{len}");
        }
        assert_eq!(Weapons::read(&[&bytes[..], &[0]].concat(), 3), Err(Fault::Unread));
        assert_eq!(Weapons::read(&bytes, 2), Err(Fault::Unread));
        assert_eq!(Weapons::read(&bytes, 4), Err(Fault::Unread));
        let mut other = bytes.clone();
        other[0] = 2;
        assert_eq!(Weapons::read(&other, 3), Err(Fault::Version(2)));
        assert_eq!(block(&[PlaceWeapons { len: 16, ..PlaceWeapons::NONE }; 20]).len(), MAX_BLOCK);
    }

    /// One place's entries, each `(dict id, kills, [headshots, wall kills,
    /// ricochet kills], kill distance, longest kill, held ms)`.
    type Entry = (u16, u16, [u16; 3], u32, u16, u32);

    fn entries(entries: &[Entry]) -> Vec<u8> {
        let mut body = Vec::from([entries.len() as u8]);
        for &(dict_id, kills, [headshots, wall, ricochet], distance, longest, held) in entries {
            for count in [dict_id, kills, headshots, wall, ricochet, 0, 0, 0] {
                body.extend(count.to_le_bytes());
            }
            body.extend(0u32.to_le_bytes());
            body.extend(distance.to_le_bytes());
            body.extend(longest.to_le_bytes());
            body.extend(held.to_le_bytes());
        }
        let mut bytes = Vec::from([VERSION]);
        bytes.extend((body.len() as u16).to_le_bytes());
        bytes.extend(body);
        bytes
    }

    #[test]
    fn the_domains_are_held() {
        let field = |entries_: &[Entry]| match Weapons::read(&entries(entries_), 1) {
            Err(Fault::Field { place: 0, field }) => Some(field),
            Ok(Some(_)) => None,
            other => panic!("{other:?}"),
        };
        let none = |id| -> Entry { (id, 0, [0; 3], 0, 0, 0) };
        assert_eq!(field(&[none(0), (4, 2, [1, 2, 2], 300, 200, 150), (9, 1, [0; 3], 0, 0, 0)]), None);
        assert_eq!(field(&[none(4), none(4)]), Some("dict_id (not rising)"));
        assert_eq!(field(&[none(4), none(0)]), Some("dict_id (not rising)"));
        assert_eq!(field(&[(4, 1, [2, 0, 0], 0, 0, 0)]), Some("headshots (over its kills)"));
        assert_eq!(field(&[(4, 1, [0, 2, 0], 0, 0, 0)]), Some("wall_kills (over its kills)"));
        assert_eq!(field(&[(4, 1, [0, 0, 2], 0, 0, 0)]), Some("ricochet_kills (over its kills)"));
        assert_eq!(field(&[(4, 0, [0; 3], 0, 0, 149)]), Some("held_ms (no whole ticks)"));
        assert_eq!(field(&[(4, 2, [0; 3], 100, 101, 0)]), Some("longest_kill (over its kills' distance)"));
        assert_eq!(field(&[(4, 2, [0; 3], 201, 100, 0)]), Some("kill_distance (over its kills at the longest)"));
        assert_eq!(field(&[(4, 0, [0; 3], 1, 1, 0)]), Some("kill_distance (over its kills at the longest)"));
        let sixteen: Vec<_> = (1..=16).map(none).collect();
        assert_eq!(field(&sixteen), Some("n (16 with no entry of no weapon)"));
        let seventeen: Vec<_> = (0..=16).map(none).collect();
        assert_eq!(field(&seventeen), Some("n (over 16)"));
    }

    #[test]
    fn the_sums_are_the_records_as_far_as_it_says() {
        let mut place = PlaceWeapons::NONE;
        place.count(id(4), |counts| counts.fire(40_000));
        place.count(id(9), |counts| counts.fire(40_000));
        for _ in 0..300 {
            place.count(id(4), |counts| counts.kill(Killing { headshot: true, ..Killing::default() }));
            place.count(id(9), WeaponCounts::hit);
            place.count(None, WeaponCounts::death);
        }
        // 300 kills are 44 in the record's byte; 80000 shots wrap as its u16.
        let totals = Totals { kills: 44, headshots: 44, deaths: 44, shots: 14_464, hits: 300 };
        let bytes = block(&[place]);
        let read = Weapons::read(&bytes, 1).unwrap().unwrap();
        assert_eq!(read.agree(&[totals]), Ok(()));
        for (wrong, field) in [
            (Totals { kills: 45, ..totals }, "kills (not the record's event 0)"),
            (Totals { headshots: 0, ..totals }, "headshots (not the record's event 15)"),
            (Totals { deaths: 43, ..totals }, "deaths (not the record's event 2)"),
            (Totals { shots: 14_465, ..totals }, "shots (not the record's event 22)"),
            (Totals { hits: 44, ..totals }, "hits (not the record's event 23)"),
        ] {
            assert_eq!(read.agree(&[wrong]), Err(Fault::Field { place: 0, field }));
        }
    }
}

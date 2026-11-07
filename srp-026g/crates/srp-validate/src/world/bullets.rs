//! The bullets section (`bullet_manager::serialize` 0x321B90, reader
//! `bullet_manager::deserialize` 0x321C50): a 16-byte header, N 69-byte
//! bullets (`bullet::serialize` 0x32BEC0, reader the bullet's constructor
//! 0x32BBC0), then on maps with breakable lamps patch 75's lamp history.
//!
//! The writers name every field: they pass its expression to the writer
//! as text (`.\bullet_manager.cpp`, `.\bullet.cpp`), so the names below are
//! the game's own (PROVEN, the strings at 0xA17560.. and 0xA18AE8..).
//!
//! The header:
//!
//! ```text
//! off  size  field
//!   0  4     m_current_time_in_ms
//!   4  4     m_random.seed()
//!   8  4     m_current_bullet_id
//!  12  4     m_bullets.size(): N
//! ```
//!
//! A bullet ([`Bullet`]):
//!
//! ```text
//! off  size  field                          domain
//!   0  4     m_id
//!   4  12    m_start_position               float3, any f32
//!  16  12    m_start_velocity               float3, numbers
//!  28  4     m_max_damage_speed             a number
//!  32  4     m_min_damage_speed             a number
//!  36  4     m_born_time_in_ms
//!  40  4     m_life_time                    a number
//!  44  4     m_flown_distance               a number
//!  48  4     m_damage                       a number
//!  52  4     m_pierce                       a number
//!  56  1     m_ricochet_count               0..=2
//!  57  1     u8(m_change_trajectory_count)  open
//!  58  4     m_max_damage_dealt             a number
//!  62  1     s8(m_initiator_stance)         open
//!  63  1     m_initiator->id                a place (see the rule)
//!  64  1     m_ignorable_object's player id the initiator
//!  65  1     the weapon's profile_slot_id   7 or 10
//!  66  1     the ammunition's ordinal       open
//!  67  1     the collided material's id     open (0: none, or id 0)
//!  68  1     m_buck_shot                    open
//! ```
//!
//! The domains (each read once by [`Bullet::read`]):
//!
//! - `m_ricochet_count`: `try_reflect` 0x32DE40 adds one only below 2
//!   (`cmp byte [ebx+88h], 2` 0x32DE4D, `inc` 0x32E33A); the constructor
//!   sets 0 (PROVEN). Nothing indexes by it.
//! - The initiator is a place (the reader looks the player up by it,
//!   0x32BD37); the ignorable object is the initiator's player (every
//!   bullet is the shooter's; PROVEN in every world kept). Whether the
//!   place has a player a game can read the bullet with is the rule
//!   [`bullets`] holds it to, over the parsed bullet and the match.
//! - Every float is a number, `m_start_position` (+28h, the loader
//!   0x32BC19) too: a revision-50 game wrote NaN there for a shooter whose
//!   position went NaN (rec026g_20261106-211841_03439962, place 12), which
//!   exe patch 117 keeps from happening (`snapshot_rules.md` 7.2.1).
//! - The slot: the reader takes `[[player+10Ch]+10Ch + slot*4]` with no
//!   bound (0x32BDA1); a bullet is a weapon's, in the kit's slot 7 or 10.
//! - Open: the trajectory changes (a dword counted by `change_trajectory`
//!   0x32EE20; the tick stops a bullet at 30, 0x32C20D, but whether one
//!   step can pass it was not traced); the stance (`weapon_core::
//!   player_stance` 0x2E43A0 gives 0..=5, LIKELY, other initiators not
//!   traced; nothing indexes by it on load); the ammunition's ordinal (patch
//!   85 makes one past the loading gun's list its none, `bullet.
//!   load_ordinal` 0x32BDBA: a picked-up gun's bullet may name any); the
//!   material (the reader's table has a null default, 0x32BDF7..0x32BE05;
//!   its size is not in the context); `m_buck_shot`, the pellet's number in
//!   its shot (`instant_fire` passes its pellet counter, 0x2E24E9; patch 85
//!   saves it for every bullet), bounded by an ammunition's pellets, which
//!   the context does not have.
//!
//! The lamp history ([`Lamps`]; `breakable_lamps.md`): `LMP1`, a u32 count
//! of fixtures (1 to 256), then ceil(count / 32) words of bits, low bit
//! first, 1 broken; the bits past the count are 0 (the writer clears them,
//! PROVEN in the patch's C reference).
use super::{Body, Context, Frame, Section, WEAPON_SLOTS};
use crate::reader::{Reader, Short};
use crate::{Finite, KITS, Part, Place, Reason, Refusal};

/// The header's length.
const HEADER: usize = 16;
/// A bullet's length.
const RECORD: usize = 69;

/// A bullet as a world saves it (the module's table), every field read
/// into its type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bullet {
    id: u32,
    start_position: [Finite; 3],
    start_velocity: [Finite; 3],
    max_damage_speed: Finite,
    min_damage_speed: Finite,
    born_time_ms: u32,
    life_time: Finite,
    flown_distance: Finite,
    damage: Finite,
    pierce: Finite,
    ricochets: u8,
    trajectory_changes: u8,
    max_damage_dealt: Finite,
    initiator_stance: i8,
    initiator: Place,
    weapon_slot: WeaponSlot,
    ammunition: u8,
    collided_material: u8,
    buck_shot: u8,
}

/// The kit slot a bullet's weapon is in: one of [`WEAPON_SLOTS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponSlot(u8);

impl WeaponSlot {
    pub fn new(slot: u8) -> Option<WeaponSlot> {
        WEAPON_SLOTS.contains(&slot).then_some(WeaponSlot(slot))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// Why 69 bytes are no [`Bullet`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BulletFault {
    /// The initiator is no place.
    Initiator(u8),
    /// The ignorable object is not the initiator.
    Ignorable {
        ignorable: u8,
        initiator: u8,
    },
    Slot(u8),
    /// More ricochets than a bullet can make.
    Ricochets(u8),
    /// A float that is no number, at its offset in the bullet.
    NotNumber {
        offset: usize,
    },
}

impl Bullet {
    /// The record's length.
    pub const LEN: usize = RECORD;

    /// A bullet of these bytes, or why not.
    pub fn read(bytes: &[u8; RECORD]) -> Result<Self, BulletFault> {
        let mut reader = Reader::new(bytes, 0);
        // The 69 bytes are there: no read below runs short.
        let fields = (|| {
            let id = reader.u32()?;
            let start_position = [reader.u32()?, reader.u32()?, reader.u32()?];
            let start_velocity = [reader.u32()?, reader.u32()?, reader.u32()?];
            let speeds = [reader.u32()?, reader.u32()?];
            let born_time_ms = reader.u32()?;
            let flight = [reader.u32()?, reader.u32()?, reader.u32()?, reader.u32()?];
            let counts = reader.array::<2>()?;
            let max_damage_dealt = reader.u32()?;
            let ids = reader.array::<7>()?;
            Ok::<_, Short>((
                id,
                start_position,
                start_velocity,
                speeds,
                born_time_ms,
                flight,
                counts,
                max_damage_dealt,
                ids,
            ))
        })();
        let Ok((
            id,
            start_position,
            start_velocity,
            [max_speed, min_speed],
            born_time_ms,
            [life_time, flown_distance, damage, pierce],
            [ricochets, trajectory_changes],
            max_damage_dealt,
            [
                stance,
                initiator,
                ignorable,
                slot,
                ammunition,
                collided_material,
                buck_shot,
            ],
        )) = fields
        else {
            return Err(BulletFault::NotNumber { offset: 0 });
        };
        let initiator_place = Place::new(initiator).ok_or(BulletFault::Initiator(initiator))?;
        if ignorable != initiator {
            return Err(BulletFault::Ignorable {
                ignorable,
                initiator,
            });
        }
        let weapon_slot = WeaponSlot::new(slot).ok_or(BulletFault::Slot(slot))?;
        if ricochets > Self::MAX_RICOCHETS {
            return Err(BulletFault::Ricochets(ricochets));
        }
        let number = |bits: u32, offset: usize| {
            Finite::of_bits(bits).ok_or(BulletFault::NotNumber { offset })
        };
        let float3 = |bits: [u32; 3], offset: usize| {
            Ok::<_, BulletFault>([
                number(bits[0], offset)?,
                number(bits[1], offset.saturating_add(4))?,
                number(bits[2], offset.saturating_add(8))?,
            ])
        };
        Ok(Self {
            id,
            start_position: float3(start_position, 4)?,
            start_velocity: float3(start_velocity, 16)?,
            max_damage_speed: number(max_speed, 28)?,
            min_damage_speed: number(min_speed, 32)?,
            born_time_ms,
            life_time: number(life_time, 40)?,
            flown_distance: number(flown_distance, 44)?,
            damage: number(damage, 48)?,
            pierce: number(pierce, 52)?,
            ricochets,
            trajectory_changes,
            max_damage_dealt: number(max_damage_dealt, 58)?,
            initiator_stance: i8::from_le_bytes([stance]),
            initiator: initiator_place,
            weapon_slot,
            ammunition,
            collided_material,
            buck_shot,
        })
    }

    /// The most ricochets a bullet makes.
    pub const MAX_RICOCHETS: u8 = 2;

    /// `m_id`.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// `m_start_position`.
    pub fn start_position(&self) -> [Finite; 3] {
        self.start_position
    }

    pub fn start_velocity(&self) -> [Finite; 3] {
        self.start_velocity
    }

    pub fn max_damage_speed(&self) -> Finite {
        self.max_damage_speed
    }

    pub fn min_damage_speed(&self) -> Finite {
        self.min_damage_speed
    }

    /// `m_born_time_in_ms`: match ms.
    pub fn born_time_ms(&self) -> u32 {
        self.born_time_ms
    }

    pub fn life_time(&self) -> Finite {
        self.life_time
    }

    pub fn flown_distance(&self) -> Finite {
        self.flown_distance
    }

    pub fn damage(&self) -> Finite {
        self.damage
    }

    pub fn pierce(&self) -> Finite {
        self.pierce
    }

    /// `m_ricochet_count`: at most [`Bullet::MAX_RICOCHETS`].
    pub fn ricochets(&self) -> u8 {
        self.ricochets
    }

    /// `m_change_trajectory_count`, as a byte: the segments past the first
    /// (each pierce and reflection, client_patch.md "the bullet").
    pub fn trajectory_changes(&self) -> u8 {
        self.trajectory_changes
    }

    pub fn max_damage_dealt(&self) -> Finite {
        self.max_damage_dealt
    }

    /// `m_initiator_stance`: the weapon's `player_stance` at the shot.
    pub fn initiator_stance(&self) -> i8 {
        self.initiator_stance
    }

    /// The shooter: `m_initiator->id`, which is also the ignorable object's
    /// player.
    pub fn initiator(&self) -> Place {
        self.initiator
    }

    /// The kit slot of the weapon that fired it.
    pub fn weapon_slot(&self) -> WeaponSlot {
        self.weapon_slot
    }

    /// `ammunition_id(*m_weapon_ammunition)`: the ordinal of its ammunition
    /// in the weapon's list.
    pub fn ammunition(&self) -> u8 {
        self.ammunition
    }

    /// `m_collided_material->id()`, 0 with none.
    pub fn collided_material(&self) -> u8 {
        self.collided_material
    }

    /// `m_buck_shot`: the pellet's number in its shot.
    pub fn buck_shot(&self) -> u8 {
        self.buck_shot
    }
}

/// Patch 75's lamp history, borrowed from the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lamps<'a> {
    count: u16,
    words: &'a [[u8; 4]],
}

impl<'a> Lamps<'a> {
    const MAGIC: [u8; 4] = *b"LMP1";
    const MAX: u32 = 256;

    /// The history that fills `bytes`, or `None`.
    fn read(bytes: &'a [u8]) -> Option<Self> {
        let mut reader = Reader::new(bytes, 0);
        if reader.array::<4>().ok()? != Self::MAGIC {
            return None;
        }
        let count = reader.u32().ok()?;
        if !(1..=Self::MAX).contains(&count) {
            return None;
        }
        let (words, rest) = reader.rest().as_chunks::<4>();
        let wanted = count.div_ceil(32) as usize;
        if !rest.is_empty() || words.len() != wanted {
            return None;
        }
        // The bits past the count are 0.
        let used = count % 32;
        let last = words.last().map_or(0, |word| u32::from_le_bytes(*word));
        if used != 0 && crate::high(last, used) != 0 {
            return None;
        }
        Some(Self {
            count: u16::try_from(count).ok()?,
            words,
        })
    }

    /// The fixtures the map's metadata has.
    pub fn count(&self) -> u16 {
        self.count
    }

    /// Whether fixture `lamp` is broken (`false` past the count).
    pub fn broken(&self, lamp: u16) -> bool {
        lamp < self.count
            && self
                .words
                .get(usize::from(lamp / 32))
                .is_some_and(|word| crate::bit(u32::from_le_bytes(*word), u32::from(lamp % 32)))
    }
}

/// The bullets section, borrowed from the world: the header's fields, the
/// bullets (each read into a [`Bullet`]), the lamp history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bullets<'a> {
    current_time_ms: u32,
    seed: u32,
    current_bullet_id: u32,
    records: &'a [[u8; RECORD]],
    lamps: Option<Lamps<'a>>,
}

impl<'a> Bullets<'a> {
    /// The section, read to its end with every bullet in its domains, or
    /// why not.
    pub fn read(section: &Section<'a>) -> Result<Self, Refusal> {
        let len = section.bytes.len();
        let unread = |bullets| {
            Refusal::new(
                Part::Bullets,
                section.at,
                Reason::BulletsUnread { len, bullets },
            )
        };
        let mut reader = Reader::new(section.bytes, section.at);
        let header =
            (|| Ok::<_, Short>([reader.u32()?, reader.u32()?, reader.u32()?, reader.u32()?]))();
        let Ok([current_time_ms, seed, current_bullet_id, count]) = header else {
            return Err(unread(0));
        };
        let end = (count as usize)
            .checked_mul(RECORD)
            .and_then(|bytes| bytes.checked_add(HEADER))
            .filter(|&end| end <= len)
            .ok_or_else(|| unread(count))?;
        let (records, _) = section
            .bytes
            .get(HEADER..end)
            .unwrap_or_default()
            .as_chunks::<RECORD>();
        let suffix = section.bytes.get(end..).unwrap_or_default();
        let lamps = if suffix.is_empty() {
            None
        } else {
            Some(Lamps::read(suffix).ok_or_else(|| {
                Refusal::new(
                    Part::Bullets,
                    section.at.saturating_add(end),
                    Reason::LampHistory,
                )
            })?)
        };
        for (number, record) in records.iter().enumerate() {
            let at = section
                .at
                .saturating_add(HEADER)
                .saturating_add(number.saturating_mul(RECORD));
            Bullet::read(record).map_err(|fault| {
                let reason = match fault {
                    BulletFault::Initiator(owner) => Reason::BulletOwner {
                        bullet: number,
                        owner,
                    },
                    BulletFault::Ignorable {
                        ignorable,
                        initiator,
                    } => Reason::BulletSecond {
                        bullet: number,
                        second: ignorable,
                        owner: initiator,
                    },
                    BulletFault::Slot(slot) => Reason::BulletSlot {
                        bullet: number,
                        slot,
                    },
                    BulletFault::Ricochets(count) => Reason::BulletRicochets {
                        bullet: number,
                        count,
                    },
                    BulletFault::NotNumber { .. } => Reason::BulletNotNumber { bullet: number },
                };
                Refusal::new(Part::Bullets, at, reason)
            })?;
        }
        Ok(Self {
            current_time_ms,
            seed,
            current_bullet_id,
            records,
            lamps,
        })
    }

    /// `m_current_time_in_ms`.
    pub fn current_time_ms(&self) -> u32 {
        self.current_time_ms
    }

    /// `m_random.seed()`.
    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// `m_current_bullet_id`: the id the next bullet gets.
    pub fn current_bullet_id(&self) -> u32 {
        self.current_bullet_id
    }

    /// How many bullets are in flight.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The bullets in their order. Each was read by [`Bullets::read`]; a
    /// world has no bound on their number, so they are handed out one by
    /// one from their bytes (which read again as they did).
    pub fn iter(&self) -> impl Iterator<Item = Bullet> + 'a {
        self.records
            .iter()
            .filter_map(|record| Bullet::read(record).ok())
    }

    /// The lamp history, on a map with breakable lamps.
    pub fn lamps(&self) -> Option<Lamps<'a>> {
        self.lamps
    }
}

/// The bullets read ([`Bullets::read`]), each with a shooter a game can
/// read it with.
///
/// The loader (`bullet::bullet` 0x32BBC0) takes the shooter by his place
/// from `game_world::get_player` (0x1F7660, the engine's slot +24h,
/// 0x32BD4E), which is `network_client::get_player` (0x27A980, the client's
/// slot +4Ch): `[[core+18458h] + place*30h + 4*[..+0Ch]]`, the place's
/// player object of its current kit, with no bound on the place and no look
/// at the world's mask. A null player is kept at +8, but 0x32BDA1 reads its
/// inventory (`[player+10Ch]`) with no check, and 0x2E2750 the slot's item
/// as a gun (`[weapon+5B0h]`) with none: the reader needs a player object at
/// the place and a gun in the slot of its current kit. A place's objects
/// last the match: a game makes one for each usable kit of every roster
/// place (the current kit the lowest of them, 0x321820), the roster only
/// grows, and a player's leaving clears his bit of the events' masks alone
/// (`on_player_left_match` 0x2D2040). So a bullet whose shooter left with
/// it in flight, which has no block in the world
/// (rec026g_20261106-205639_4900abd9, place 12), is read with the objects
/// every game still has of him.
pub fn bullets(frame: &Frame<'_>, context: &Context<'_>) -> Result<(), Refusal> {
    let section = frame.bullets();
    let read = Bullets::read(&section)?;
    for (number, bullet) in read.iter().enumerate() {
        if let Some(reason) = shooter(number, &bullet, frame, context) {
            let at = section
                .at
                .saturating_add(HEADER)
                .saturating_add(number.saturating_mul(RECORD));
            return Err(Refusal::new(Part::Bullets, at, reason));
        }
    }
    Ok(())
}

/// Why a game could not read bullet `number`'s shooter, if it could not
/// ([`bullets`]). A shooter with a block is the world's. One without must
/// be a place of the match: below the world's places and, when the server
/// has the roster's kits, a roster place every usable kit of which has a
/// gun in the bullet's slot (which kit is current for a place with no block
/// differs between games: a newcomer's is the lowest usable, an older
/// game's the last it saw, so each must have one). Without the kits (the
/// game's own check) the place bound alone.
fn shooter(
    number: usize,
    bullet: &Bullet,
    frame: &Frame<'_>,
    context: &Context<'_>,
) -> Option<Reason> {
    let owner = bullet.initiator().get();
    if frame.has_block(owner) {
        return None;
    }
    let place = usize::from(owner);
    let no_place = Some(Reason::BulletOwner {
        bullet: number,
        owner,
    });
    if place >= context.places {
        return no_place;
    }
    // Without the kits, no reason: the place bound alone.
    let inventory = context.inventory?;
    let (Some(kits), Some(profile)) = (
        inventory.kits.get(place),
        context.profiles.and_then(|profiles| profiles.get(place)),
    ) else {
        return no_place;
    };
    let slot = bullet.weapon_slot().get();
    let gun = |kit: u8| {
        kits.get(usize::from(kit)).is_some_and(|items| {
            items
                .iter()
                .any(|item| item.slot == slot && matches!(item.body, Body::Weapon(_)))
        })
    };
    let mut usable = (0..KITS as u8).filter(|&kit| profile.usable(kit)).peekable();
    if usable.peek().is_none() {
        return no_place;
    }
    usable.find(|&kit| !gun(kit)).map(|kit| Reason::BulletNoWeapon {
        bullet: number,
        slot,
        kit,
    })
}

#[cfg(test)]
mod tests {
    use super::super::{Ammo, Chamber, Gun, Inventory, Item, Measure, Profile};
    use super::*;
    fn body(records: &[u8], count: u32) -> std::vec::Vec<u8> {
        let mut body = 1u32.to_le_bytes().to_vec();
        body.extend([2, 0, 0, 0, 0]);
        body.extend(((HEADER + records.len()) as u16).to_le_bytes());
        body.extend([0; 12]);
        body.extend(count.to_le_bytes());
        body.extend(records);
        body.extend([0; 10]);
        body
    }
    fn context<'a>(
        places: usize,
        profiles: Option<&'a [Profile]>,
        inventory: Option<&'a Inventory<'a>>,
    ) -> Context<'a> {
        Context {
            time: 0,
            places,
            game_mode: 4,
            batteries: 0,
            respawn_points: None,
            profiles,
            artefacts: None,
            objects: None,
            inventory,
        }
    }
    fn checked(records: &[u8], count: u32, context: &Context<'_>) -> Result<(), Refusal> {
        bullets(&super::super::frame(&body(records, count), 1).unwrap(), context)
    }
    fn verdict(records: &[u8], count: u32) -> Result<(), Refusal> {
        checked(records, count, &context(1, None, None))
    }
    #[test]
    fn quiet_byte_is_always_present_and_old_ammo_ordinals_are_allowed() {
        let mut record = [0u8; RECORD];
        record[65] = 7;
        record[66] = 255; // native loading clamps this after a gun swap
        assert_eq!(verdict(&record, 1), Ok(()));
        assert!(verdict(&record[..68], 1).is_err());
        let pair = [record, record].concat();
        assert_eq!(verdict(&pair, 2), Ok(()));
        for length in 0..pair.len() {
            assert!(verdict(&pair[..length], 2).is_err());
        }
    }
    #[test]
    fn owner_slot_and_numbers_are_bounded() {
        let mut valid = [0u8; RECORD];
        valid[65] = 7;
        for (offset, value) in [(63, 20), (64, 1), (65, 23), (56, 3)] {
            let mut bad = valid;
            bad[offset] = value;
            assert!(verdict(&bad, 1).is_err(), "{offset}");
        }
        // Neither the start position (a revision-50 NaN shooter's, x87's
        // indefinite) nor the velocity may be a non-number.
        for at in [4, 8, 12] {
            let mut origin = valid;
            origin[at..at + 4].copy_from_slice(&(-f32::NAN).to_bits().to_le_bytes());
            assert!(verdict(&origin, 1).is_err(), "{at}");
        }
        valid[16..20].copy_from_slice(&f32::NAN.to_bits().to_le_bytes());
        assert!(verdict(&valid, 1).is_err());
        assert!(verdict(&[], u32::MAX).is_err());
    }

    /// A bullet of place 2, which has no block (the world's mask is place 0
    /// alone): its shooter left with it in flight
    /// (rec026g_20261106-205639_4900abd9, 149 550 ms: place 12's).
    #[test]
    fn a_bullet_of_a_place_that_left_needs_its_objects() {
        let record = |slot: u8| {
            let mut record = [0u8; RECORD];
            (record[63], record[64], record[65]) = (2, 2, slot);
            record
        };
        let reason = |result: Result<(), Refusal>| result.expect_err("refused").reason;
        // The game's own check: the places alone.
        assert_eq!(checked(&record(7), 1, &context(3, None, None)), Ok(()));
        assert!(matches!(
            reason(checked(&record(7), 1, &context(2, None, None))),
            Reason::BulletOwner { owner: 2, .. }
        ));
        // The server: every usable kit of the place has a gun in the slot.
        let gun = Body::Weapon(Gun {
            states: &[],
            chamber: Chamber::NotSaved,
            ammo: Ammo::Kit(1),
        });
        let item = |slot, body| Item {
            slot,
            measure: Measure::Condition,
            body,
        };
        let rifle = [item(0, Body::None), item(7, gun)];
        let none = [item(0, Body::None), item(7, Body::None)];
        let kits: [[&[Item]; KITS]; 3] = [[&[]; KITS], [&[]; KITS], [&rifle, &none, &rifle]];
        let inventory = Inventory {
            kits: &kits,
            body_parts: 1,
        };
        let usable = |kits| Profile {
            usable_kits: kits,
            ..Profile::default()
        };
        let server = |kits: u8, record: &[u8]| {
            let profiles = [usable(1), usable(1), usable(kits)];
            checked(record, 1, &context(3, Some(&profiles), Some(&inventory)))
        };
        assert_eq!(server(0b101, &record(7)), Ok(()));
        assert!(matches!(
            reason(server(0b111, &record(7))),
            Reason::BulletNoWeapon { slot: 7, kit: 1, .. }
        ));
        assert!(matches!(
            reason(server(0b001, &record(10))),
            Reason::BulletNoWeapon { slot: 10, kit: 0, .. }
        ));
        assert!(matches!(
            reason(server(0, &record(7))),
            Reason::BulletOwner { owner: 2, .. }
        ));
        // A place past the roster the server has.
        let profiles = [usable(1), usable(1)];
        assert!(matches!(
            reason(checked(&record(7), 1, &context(3, Some(&profiles), Some(&inventory)))),
            Reason::BulletOwner { owner: 2, .. }
        ));
    }

    #[test]
    fn every_field_reads_by_name() {
        let mut record = [0u8; RECORD];
        record[0..4].copy_from_slice(&77u32.to_le_bytes());
        record[16..20].copy_from_slice(&2.5f32.to_le_bytes());
        record[36..40].copy_from_slice(&40_600u32.to_le_bytes());
        record[56] = 2;
        record[57] = 3;
        record[62] = 0xFF;
        record[65] = 10;
        record[66] = 1;
        record[67] = 9;
        record[68] = 4;
        let bullet = Bullet::read(&record).unwrap();
        assert_eq!(bullet.id(), 77);
        assert_eq!(bullet.start_velocity()[0].get(), 2.5);
        assert_eq!(bullet.born_time_ms(), 40_600);
        assert_eq!((bullet.ricochets(), bullet.trajectory_changes()), (2, 3));
        assert_eq!(bullet.initiator_stance(), -1);
        assert_eq!(bullet.initiator(), Place::new(0).unwrap());
        assert_eq!(bullet.weapon_slot().get(), 10);
        assert_eq!(
            (
                bullet.ammunition(),
                bullet.collided_material(),
                bullet.buck_shot()
            ),
            (1, 9, 4)
        );
    }

    #[test]
    fn lamp_history_suffix_has_a_bounded_exact_length() {
        for count in [1u32, 32, 33, 256] {
            let mut suffix = b"LMP1".to_vec();
            suffix.extend(count.to_le_bytes());
            let words = count.div_ceil(32) as usize;
            for word in 0..words {
                let bits = match (count as usize - word * 32).min(32) {
                    32 => u32::MAX,
                    used => (1u32 << used) - 1,
                };
                suffix.extend(bits.to_le_bytes());
            }
            assert_eq!(verdict(&suffix, 0), Ok(()));
            for len in 1..suffix.len() {
                assert!(verdict(&suffix[..len], 0).is_err());
            }
            if count % 32 != 0 {
                let mut past = suffix.clone();
                let last = past.len() - 1;
                past[last] |= 0x80;
                assert!(verdict(&past, 0).is_err(), "a bit past {count}");
            }
            for wrong in [0u32, 257, u32::MAX] {
                suffix[4..8].copy_from_slice(&wrong.to_le_bytes());
                assert!(verdict(&suffix, 0).is_err());
            }
        }
    }
}

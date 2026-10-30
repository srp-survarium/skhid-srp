//! Canonical ownership bytes for the native world snapshot extension.
//! They must be part of the normal world hash, not a local side cache.
//! Decoding is all-or-nothing and rejects aliased ownership.

use crate::state::{Borrowed, Drop, DropId, Kit, Life, MAX_PLACES, Place, Player, Template, World};
use crate::{AmmoKind, Chamber, DroppedAmmo, Handedness, WeaponSlot};

const MAGIC: &[u8; 4] = b"WPK2";
/// Worst case: every place alive with both slots borrowed (31 bytes each).
pub const MAX_SIZE: usize = 4 + MAX_PLACES * 31;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    TooShort,
    Invalid,
    DuplicateOwnership,
    TrailingBytes,
}

pub struct Snapshot {
    bytes: [u8; MAX_SIZE],
    len: usize,
}

impl Snapshot {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn byte(&mut self, value: u8) {
        self.bytes[self.len] = value;
        self.len += 1;
    }

    fn word(&mut self, value: u32) {
        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }

    fn template(&mut self, template: Template) {
        self.byte(template.source.place.get());
        self.word(template.source.spawned);
        self.byte(template.kit.get());
        self.byte(template.slot.index() as u8);
    }
}

impl World {
    pub fn snapshot(&self) -> Snapshot {
        let mut out = Snapshot {
            bytes: [0; MAX_SIZE],
            len: 0,
        };
        for byte in MAGIC {
            out.byte(*byte);
        }
        for player in self.players {
            match player {
                Player::Absent => out.byte(0),
                Player::Alive {
                    spawned,
                    kit,
                    borrowed,
                } => {
                    out.byte(1);
                    out.word(spawned);
                    out.byte(kit.get());
                    out.byte(
                        u8::from(borrowed[0].is_some()) | (u8::from(borrowed[1].is_some()) << 1),
                    );
                    for item in borrowed.into_iter().flatten() {
                        out.byte(item.drop.0.place.get());
                        out.word(item.drop.0.spawned);
                        out.template(item.template);
                    }
                }
                Player::Dead { spawned, drop } => {
                    out.byte(2);
                    out.word(spawned);
                    out.byte(u8::from(drop.is_some()));
                    if let Some(drop) = drop {
                        out.template(drop.template);
                        out.byte(match drop.handedness {
                            Handedness::One => 0,
                            Handedness::Two => 1,
                        });
                        out.byte(drop.ammo.capacity());
                        out.byte(drop.ammo.magazine());
                        out.byte(drop.ammo.kind().get());
                        out.byte(match drop.ammo.chamber() {
                            Chamber::None => 0,
                            Chamber::Empty => 1,
                            Chamber::Loaded => 2,
                        });
                    }
                }
            }
        }
        out
    }

    /// Build a complete new world; never overwrite the live state with a
    /// partly read packet. The native adapter resolves templates before
    /// replacing its own state/inventory mapping with this result.
    pub fn from_snapshot(bytes: &[u8]) -> Result<Self, Error> {
        let mut reader = Reader { bytes };
        if reader.take::<4>()? != *MAGIC {
            return Err(Error::Invalid);
        }
        let mut world = Self::new();
        for index in 0..MAX_PLACES {
            world.players[index] = match reader.byte()? {
                0 => Player::Absent,
                1 => {
                    let spawned = reader.word()?;
                    let kit = reader.kit()?;
                    let mask = reader.byte()?;
                    if mask & !3 != 0 {
                        return Err(Error::Invalid);
                    }
                    let mut borrowed = [None; 2];
                    for (slot, entry) in borrowed.iter_mut().enumerate() {
                        if mask & (1 << slot) != 0 {
                            *entry = Some(Borrowed {
                                drop: DropId(Life {
                                    place: reader.place()?,
                                    spawned: reader.word()?,
                                }),
                                template: reader.template()?,
                            });
                        }
                    }
                    Player::Alive {
                        spawned,
                        kit,
                        borrowed,
                    }
                }
                2 => {
                    let spawned = reader.word()?;
                    let drop = match reader.byte()? {
                        0 => None,
                        1 => {
                            let template = reader.template()?;
                            let handedness = match reader.byte()? {
                                0 => Handedness::One,
                                1 => Handedness::Two,
                                _ => return Err(Error::Invalid),
                            };
                            let capacity = reader.byte()?;
                            let magazine = reader.byte()?;
                            let kind = AmmoKind::parse(reader.byte()?).ok_or(Error::Invalid)?;
                            let chamber = match reader.byte()? {
                                0 => Chamber::None,
                                1 => Chamber::Empty,
                                2 => Chamber::Loaded,
                                _ => return Err(Error::Invalid),
                            };
                            let ammo = DroppedAmmo::parse(capacity, magazine, chamber, kind)
                                .ok_or(Error::Invalid)?;
                            Some(Drop {
                                id: DropId(Life {
                                    place: Place::parse(index as u8).unwrap(),
                                    spawned,
                                }),
                                template,
                                handedness,
                                ammo,
                            })
                        }
                        _ => return Err(Error::Invalid),
                    };
                    Player::Dead { spawned, drop }
                }
                _ => return Err(Error::Invalid),
            };
        }
        if !reader.bytes.is_empty() {
            return Err(Error::TrailingBytes);
        }
        world.check_ownership()?;
        Ok(world)
    }

    fn check_ownership(&self) -> Result<(), Error> {
        // At most 64 carried weapons. Compare in canonical slot order, with
        // no allocation and no hash table whose order could affect a world.
        let mut seen = [None; MAX_PLACES * 2];
        let mut count = 0;
        for (place, player) in self.players.iter().enumerate() {
            if let Player::Alive {
                spawned, borrowed, ..
            } = player
            {
                for item in borrowed.iter().flatten() {
                    if self.drop(item.drop).is_some()
                        || seen[..count].contains(&Some(item.drop))
                        || (item.drop.0.place.index() == place && item.drop.0.spawned == *spawned)
                    {
                        return Err(Error::DuplicateOwnership);
                    }
                    seen[count] = Some(item.drop);
                    count += 1;
                }
            }
        }
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let head = self.bytes.get(..N).ok_or(Error::TooShort)?;
        let out = head.try_into().unwrap();
        self.bytes = &self.bytes[N..];
        Ok(out)
    }

    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take::<1>()?[0])
    }
    fn word(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn place(&mut self) -> Result<Place, Error> {
        Place::parse(self.byte()?).ok_or(Error::Invalid)
    }
    fn kit(&mut self) -> Result<Kit, Error> {
        Kit::parse(self.byte()?).ok_or(Error::Invalid)
    }
    fn template(&mut self) -> Result<Template, Error> {
        Ok(Template {
            source: Life {
                place: self.place()?,
                spawned: self.word()?,
            },
            kit: self.kit()?,
            slot: match self.byte()? {
                0 => WeaponSlot::First,
                1 => WeaponSlot::Second,
                _ => return Err(Error::Invalid),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::HeldAtDeath;
    use crate::{CarryRule, Loadout};

    fn life(place: u8, spawned: u32) -> Life {
        Life {
            place: Place::parse(place).unwrap(),
            spawned,
        }
    }

    fn kit() -> Kit {
        Kit::parse(0).unwrap()
    }
    fn loadout() -> Loadout {
        Loadout::new(
            [Some(Handedness::Two), Some(Handedness::One)],
            Some(WeaponSlot::First),
            CarryRule::OneDoubleHanded,
        )
    }

    fn fixture() -> World {
        let mut world = World::new();
        world.spawn(life(0, 100), kit());
        world.spawn(life(1, 100), kit());
        world.die(
            life(0, 100),
            Some(HeldAtDeath {
                slot: WeaponSlot::Second,
                handedness: Handedness::Two,
                ammo: DroppedAmmo::parse(30, 7, Chamber::Loaded, AmmoKind::parse(1).unwrap())
                    .unwrap(),
            }),
        );
        world
    }

    #[test]
    fn restoring_before_pickup_and_replaying_gives_identical_ownership_bytes() {
        let mut original = fixture();
        let before = original.snapshot();
        let mut restored = World::from_snapshot(before.bytes()).unwrap();
        let a = original
            .pickup(life(1, 100), DropId(life(0, 100)), loadout())
            .unwrap();
        let b = restored
            .pickup(life(1, 100), DropId(life(0, 100)), loadout())
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(original.snapshot().bytes(), restored.snapshot().bytes());
        // Donor respawn changes its native original kit but not the claimed identity.
        original.spawn(life(0, 200), kit());
        let after = original.snapshot();
        let joined = World::from_snapshot(after.bytes()).unwrap();
        assert_eq!(joined, original);
        assert_eq!(
            joined.borrowed(life(1, 100), WeaponSlot::First),
            Some(a.acquired)
        );
    }

    #[test]
    fn rejects_every_truncation_bad_tags_and_trailing_data() {
        let snapshot = fixture().snapshot();
        for len in 0..snapshot.bytes().len() {
            assert!(World::from_snapshot(&snapshot.bytes()[..len]).is_err());
        }
        let mut bad = snapshot.bytes;
        bad[4] = 9;
        assert_eq!(
            World::from_snapshot(&bad[..snapshot.len]),
            Err(Error::Invalid)
        );
        assert_eq!(
            World::from_snapshot(&snapshot.bytes[..snapshot.len + 1]),
            Err(Error::TrailingBytes)
        );
    }

    #[test]
    fn cannot_restore_the_same_drop_in_two_inventories_or_both_on_ground_and_in_hand() {
        let mut world = fixture();
        let transfer = world
            .pickup(life(1, 100), DropId(life(0, 100)), loadout())
            .unwrap();
        world.players[2] = Player::Alive {
            spawned: 100,
            kit: kit(),
            borrowed: [Some(transfer.acquired), None],
        };
        assert_eq!(
            World::from_snapshot(world.snapshot().bytes()),
            Err(Error::DuplicateOwnership)
        );
        let mut ground = fixture();
        ground.players[1] = Player::Alive {
            spawned: 100,
            kit: kit(),
            borrowed: [Some(transfer.acquired), None],
        };
        assert_eq!(
            World::from_snapshot(ground.snapshot().bytes()),
            Err(Error::DuplicateOwnership)
        );
    }

    #[test]
    fn maximum_snapshot_size_covers_two_borrowed_slots_in_every_place() {
        let mut world = World::new();
        for index in 0..MAX_PLACES {
            let item = |spawned| Borrowed {
                drop: DropId(life(index as u8, spawned)),
                template: Template {
                    source: life(index as u8, spawned),
                    kit: kit(),
                    slot: WeaponSlot::First,
                },
            };
            world.players[index] = Player::Alive {
                spawned: 300,
                kit: kit(),
                borrowed: [Some(item(100)), Some(item(200))],
            };
        }
        let snapshot = world.snapshot();
        assert_eq!(snapshot.bytes().len(), MAX_SIZE);
        assert_eq!(World::from_snapshot(snapshot.bytes()), Ok(world));
    }

    #[test]
    fn source_life_is_part_of_canonical_bytes_and_old_format_is_refused() {
        let a = fixture();
        let mut b = a.clone();
        if let Player::Dead {
            drop: Some(drop), ..
        } = &mut b.players[0]
        {
            drop.template.source.spawned = 50;
        }
        assert_ne!(a.snapshot().bytes(), b.snapshot().bytes());
        assert_eq!(World::from_snapshot(b.snapshot().bytes()), Ok(b));
        let mut obsolete = a.snapshot();
        obsolete.bytes[..4].copy_from_slice(b"WPK1");
        assert_eq!(World::from_snapshot(obsolete.bytes()), Err(Error::Invalid));
    }
}

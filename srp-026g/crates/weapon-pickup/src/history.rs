//! Ownership, private ammunition and displaced originals for a saved state.
//! Source profile/template data still belongs in the enclosing history
//! extension; this payload does not replace those records.

use crate::state::{Borrowed, Life, MAX_PLACES, Place, Player, World};
use crate::{Original, Reserves, WeaponSlot, snapshot};

const MAGIC: &[u8; 4] = b"WPA2";
pub const MAX_SIZE: usize = 6 + snapshot::MAX_SIZE + MAX_PLACES * 2 * (17 + 21);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotState {
    pub reserves: Reserves,
    pub original: Original,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError<E> {
    Read(E),
    WrongOriginalOwner,
}

/// Coupled, immutable capture. Only constructors that supply reserves for
/// every borrowed slot can create it; reserves cannot outlive their mapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct History {
    world: World,
    slots: [[Option<SlotState>; 2]; MAX_PLACES],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    TooShort,
    Ownership(snapshot::Error),
    TrailingBytes,
}

pub struct Bytes {
    bytes: [u8; MAX_SIZE],
    len: usize,
}

impl Bytes {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }

    fn push(&mut self, bytes: &[u8]) {
        self.bytes[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
    }
}

impl History {
    /// Read each native owner once in canonical player/slot order. Failure
    /// returns no partial capture and never changes the live ownership world.
    pub fn capture<E>(
        world: &World,
        mut read: impl FnMut(Life, WeaponSlot, Borrowed) -> Result<SlotState, E>,
    ) -> Result<Self, CaptureError<E>> {
        let mut slots = [[None; 2]; MAX_PLACES];
        for (place, player) in world.players.iter().enumerate() {
            if let Player::Alive {
                spawned,
                borrowed,
                kit,
            } = player
            {
                let life = Life {
                    place: Place::parse(place as u8).unwrap(),
                    spawned: *spawned,
                };
                for (slot, weapon) in borrowed.iter().enumerate() {
                    if let Some(weapon) = weapon {
                        let slot = if slot == 0 {
                            WeaponSlot::First
                        } else {
                            WeaponSlot::Second
                        };
                        let state = read(life, slot, *weapon).map_err(CaptureError::Read)?;
                        if state.original.owner() != (life.place, *kit, slot) {
                            return Err(CaptureError::WrongOriginalOwner);
                        }
                        slots[place][slot.index()] = Some(state);
                    }
                }
            }
        }
        Ok(Self {
            world: world.clone(),
            slots,
        })
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn reserves(&self, life: Life, slot: WeaponSlot) -> Option<Reserves> {
        Some(self.slot(life, slot)?.reserves)
    }

    pub fn slot(&self, life: Life, slot: WeaponSlot) -> Option<SlotState> {
        self.world.borrowed(life, slot)?;
        self.slots[life.place.index()][slot.index()]
    }

    pub fn snapshot(&self) -> Bytes {
        let ownership = self.world.snapshot();
        let mut out = Bytes {
            bytes: [0; MAX_SIZE],
            len: 0,
        };
        out.push(MAGIC);
        out.push(&(ownership.bytes().len() as u16).to_le_bytes());
        out.push(ownership.bytes());
        for player in &self.slots {
            for state in player.iter().flatten() {
                let reserve = state.reserves;
                out.push(&[reserve.counts().len() as u8]);
                for count in reserve.counts() {
                    out.push(&count.to_le_bytes());
                }
                out.push(&[state.original.bytes().len() as u8]);
                out.push(state.original.bytes());
            }
        }
        out
    }

    pub fn from_snapshot(bytes: &[u8]) -> Result<Self, Error> {
        let header = bytes.get(..6).ok_or(Error::TooShort)?;
        if &header[..4] != MAGIC {
            return Err(Error::Invalid);
        }
        let size = usize::from(u16::from_le_bytes([header[4], header[5]]));
        if size > snapshot::MAX_SIZE {
            return Err(Error::Invalid);
        }
        let ownership = bytes.get(6..6 + size).ok_or(Error::TooShort)?;
        let world = World::from_snapshot(ownership).map_err(Error::Ownership)?;
        let mut rest = &bytes[6 + size..];
        let captured = Self::capture(&world, |_, _, _| {
            let (&count, tail) = rest.split_first().ok_or(Error::TooShort)?;
            if !(1..=8).contains(&count) {
                return Err(Error::Invalid);
            }
            let count = usize::from(count);
            let encoded = tail.get(..count * 2).ok_or(Error::TooShort)?;
            let mut rounds = [0; 8];
            for (out, bytes) in rounds.iter_mut().zip(encoded.chunks_exact(2)) {
                *out = u16::from_le_bytes([bytes[0], bytes[1]]);
            }
            rest = &tail[count * 2..];
            let (&size, tail) = rest.split_first().ok_or(Error::TooShort)?;
            if !matches!(size, 19 | 20) {
                return Err(Error::Invalid);
            }
            let encoded = tail.get(..usize::from(size)).ok_or(Error::TooShort)?;
            let original = Original::parse(encoded).ok_or(Error::Invalid)?;
            rest = &tail[usize::from(size)..];
            Ok(SlotState {
                reserves: Reserves::parse(&rounds[..count]).unwrap(),
                original,
            })
        })
        .map_err(|error| match error {
            CaptureError::Read(error) => error,
            CaptureError::WrongOriginalOwner => Error::Invalid,
        })?;
        if !rest.is_empty() {
            return Err(Error::TrailingBytes);
        }
        Ok(captured)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{DropId, Kit, Template};

    fn life(place: u8, spawned: u32) -> Life {
        Life {
            place: Place::parse(place).unwrap(),
            spawned,
        }
    }

    fn fixture() -> World {
        let mut world = World::new();
        let kit = Kit::parse(0).unwrap();
        for place in 0..MAX_PLACES {
            let item = |spawned| Borrowed {
                drop: DropId(life(place as u8, spawned)),
                template: Template {
                    source: life(place as u8, spawned),
                    kit,
                    slot: WeaponSlot::First,
                },
            };
            world.players[place] = Player::Alive {
                spawned: 300,
                kit,
                borrowed: [Some(item(100)), Some(item(200))],
            };
        }
        world
    }

    fn slot_state(life: Life, slot: WeaponSlot, rounds: &[u16]) -> SlotState {
        let mut bytes = [0; 20];
        bytes[4] = life.place.get();
        bytes[6] = slot.native() as u8;
        SlotState {
            reserves: Reserves::parse(rounds).unwrap(),
            original: Original::parse(&bytes).unwrap(),
        }
    }

    #[test]
    fn every_borrowed_slot_round_trips_exact_counts_and_capture_order() {
        let world = fixture();
        let mut calls = 0;
        let saved = History::capture(&world, |life, slot, owner| {
            assert_eq!(life.place.index() * 2 + slot.index(), calls);
            assert_eq!(world.borrowed(life, slot), Some(owner));
            calls += 1;
            Ok::<_, ()>(slot_state(
                life,
                slot,
                &[0, 1, 30, 42, 255, 256, 999, u16::MAX],
            ))
        })
        .unwrap();
        assert_eq!(calls, MAX_PLACES * 2);
        assert_eq!(saved.snapshot().bytes().len(), MAX_SIZE);
        let restored = History::from_snapshot(saved.snapshot().bytes()).unwrap();
        assert_eq!(restored, saved);
        assert_eq!(restored.reserves(life(0, 299), WeaponSlot::First), None);
        assert_eq!(
            restored
                .reserves(life(0, 300), WeaponSlot::First)
                .unwrap()
                .counts()[7],
            u16::MAX
        );
    }

    #[test]
    fn reserve_changes_change_snapshot_bytes_without_changing_identity() {
        let world = fixture();
        let capture = |counts: &[u16]| {
            History::capture(&world, |life, slot, _| {
                Ok::<_, ()>(slot_state(life, slot, counts))
            })
            .unwrap()
        };
        let before_reload = capture(&[30, 0]);
        let after_reload = capture(&[7, 0]);
        let after_crate = capture(&[120, 150]);
        assert_eq!(before_reload.world(), after_crate.world());
        assert_ne!(
            before_reload.snapshot().bytes(),
            after_reload.snapshot().bytes()
        );
        assert_ne!(
            after_reload.snapshot().bytes(),
            after_crate.snapshot().bytes()
        );
        assert_eq!(
            History::from_snapshot(before_reload.snapshot().bytes())
                .unwrap()
                .reserves(life(0, 300), WeaponSlot::First)
                .unwrap()
                .counts(),
            &[30, 0]
        );
    }

    #[test]
    fn original_is_required_and_must_belong_to_the_same_recipient_kit_and_slot() {
        let world = fixture();
        let saved = History::capture(&world, |life, slot, _| {
            Ok::<_, ()>(slot_state(life, slot, &[0]))
        })
        .unwrap()
        .snapshot();
        let original_at = 6 + world.snapshot().bytes().len() + 1 + 2 + 1;
        // Changing the original's owner, kit or slot is refused both when
        // capturing native state and when parsing network/history bytes.
        for (field, value) in [(4, 1), (5, 1), (6, 10)] {
            let mut bytes = saved.bytes;
            bytes[original_at + field] = value;
            assert_eq!(
                History::from_snapshot(&bytes[..saved.len]),
                Err(Error::Invalid)
            );
            let capture = History::capture(&world, |life, slot, _| {
                let mut state = slot_state(life, slot, &[0]);
                let mut wrong = [0; 20];
                wrong.copy_from_slice(state.original.bytes());
                wrong[field] = value;
                state.original = Original::parse(&wrong).unwrap();
                Ok::<_, ()>(state)
            });
            assert_eq!(capture, Err(CaptureError::WrongOriginalOwner));
        }
        let mut changed = saved.bytes;
        changed[original_at + 16] = 7; // original's loaded rounds change too
        let changed = History::from_snapshot(&changed[..saved.len]).unwrap();
        assert_eq!(changed.world(), &world);
        assert_ne!(changed.snapshot().bytes(), saved.bytes());
        let mut old = saved.bytes;
        old[..4].copy_from_slice(b"WPA1");
        assert_eq!(
            History::from_snapshot(&old[..saved.len]),
            Err(Error::Invalid)
        );
    }

    #[test]
    fn refuses_partial_invalid_extra_and_missing_reserves() {
        let world = fixture();
        let saved = History::capture(&world, |life, slot, _| {
            Ok::<_, ()>(slot_state(life, slot, &[0]))
        })
        .unwrap()
        .snapshot();
        for len in 0..saved.bytes().len() {
            assert!(History::from_snapshot(&saved.bytes()[..len]).is_err());
        }
        let count_at = 6 + world.snapshot().bytes().len();
        for invalid in [0, 9, 255] {
            let mut bytes = saved.bytes;
            bytes[count_at] = invalid;
            assert_eq!(
                History::from_snapshot(&bytes[..saved.len]),
                Err(Error::Invalid)
            );
        }
        assert_eq!(
            History::from_snapshot(&saved.bytes[..saved.len + 1]),
            Err(Error::TrailingBytes)
        );
        let before = world.clone();
        assert_eq!(
            History::capture(&world, |_, _, _| Err::<SlotState, _>(
                "missing native owner"
            )),
            Err(CaptureError::Read("missing native owner"))
        );
        assert_eq!(world, before);
        assert!(Reserves::parse(&[]).is_none());
        assert!(Reserves::parse(&[0; 9]).is_none());
    }
}

//! Native 0.26g simulation state of a deactivated original weapon.
//! No pointers, controller state or client presentation bytes. The native
//! codec omits controller state when user == null; applying this encoding to
//! an active weapon is invalid. Its concrete template must be checked too.

use crate::state::{Kit, Place};
use crate::{AmmoKind, Chamber, DroppedAmmo, WeaponSlot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Original {
    shape: Shape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    WithoutChamber([u8; 19]),
    WithChamber([u8; 20]),
}

impl Original {
    /// Accept only the complete canonical stowed weapon encoding emitted by
    /// weapon_core::serialize for a non-stackable, inventory-owned weapon.
    /// Capacity and compatible ammo types belong to its native template.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let shape = match bytes.len() {
            19 => Shape::WithoutChamber(bytes.try_into().ok()?),
            20 if matches!(bytes[19], 0 | 255) => Shape::WithChamber(bytes.try_into().ok()?),
            _ => return None,
        };
        Place::parse(bytes[4])?;
        Kit::parse(bytes[5])?;
        WeaponSlot::from_native(u32::from(bytes[6]))?;
        AmmoKind::parse(bytes[18])?;
        if !matches!(bytes[15], 0 | 255) {
            return None;
        }
        Some(Self { shape })
    }

    pub fn bytes(&self) -> &[u8] {
        match &self.shape {
            Shape::WithoutChamber(bytes) => bytes,
            Shape::WithChamber(bytes) => bytes,
        }
    }

    pub fn owner(&self) -> (Place, Kit, WeaponSlot) {
        let bytes = self.bytes();
        (
            Place::parse(bytes[4]).unwrap(),
            Kit::parse(bytes[5]).unwrap(),
            WeaponSlot::from_native(u32::from(bytes[6])).unwrap(),
        )
    }

    pub fn ammunition(&self, capacity: u8) -> Option<DroppedAmmo> {
        let bytes = self.bytes();
        let chamber = match &self.shape {
            Shape::WithoutChamber(_) => Chamber::None,
            Shape::WithChamber(bytes) if bytes[19] == 0 => Chamber::Empty,
            Shape::WithChamber(_) => Chamber::Loaded,
        };
        DroppedAmmo::parse(capacity, bytes[16], chamber, AmmoKind::parse(bytes[18])?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_stowed_encodings_preserve_all_bytes_and_validate_shape() {
        // This exact payload also round-trips through the original executable
        // in check_pickup_original_state.py, with and without its chamber.
        let bytes = [
            0, 0, 0x40, 0x3f, 1, 2, 7, 0, 0, 0x20, 0x3f, 0x40, 0xe2, 1, 0, 255, 7, 2, 1, 255,
        ];
        for size in [19, 20] {
            let original = Original::parse(&bytes[..size]).unwrap();
            assert_eq!(original.bytes(), &bytes[..size]);
            assert_eq!(
                original.owner(),
                (
                    Place::parse(1).unwrap(),
                    Kit::parse(2).unwrap(),
                    WeaponSlot::First
                )
            );
            assert_eq!(
                original.ammunition(30),
                DroppedAmmo::parse(
                    30,
                    7,
                    if size == 19 {
                        Chamber::None
                    } else {
                        Chamber::Loaded
                    },
                    AmmoKind::parse(1).unwrap()
                )
            );
            assert!(original.ammunition(6).is_none());
        }
        for size in 0..19 {
            assert!(Original::parse(&bytes[..size]).is_none());
        }
        assert!(Original::parse(&[0; 21]).is_none());
        for (at, invalid) in [(4, 32), (5, 3), (6, 8), (15, 1), (18, 8), (19, 1)] {
            let mut bad = bytes;
            bad[at] = invalid;
            assert!(Original::parse(&bad).is_none());
        }
    }
}

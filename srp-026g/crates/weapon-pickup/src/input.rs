//! Fixed pickup-input payload shared by the server and native client.
//! A target and its ordinary input are one value, including precise zero.
use crate::state::{DropId, Life, Place};

pub const BYTES: usize = 15;

/// The wire's FF sentinel is confined to this boundary. A precise time of
/// zero milliseconds remains different from an on-grid input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubTick(Option<u8>);

impl SubTick {
    pub fn parse(ms: Option<u8>) -> Option<Self> {
        ms.is_none_or(|ms| ms < 50).then_some(Self(ms))
    }

    pub const fn milliseconds(self) -> Option<u8> {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WireInput {
    pub tick: u8,
    pub sub_tick: SubTick,
    pub rotation: [u8; 4],
    pub actions: u32,
    pub target: DropId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    Length(usize),
    Milliseconds(u8),
    Place(u8),
}

impl WireInput {
    pub fn parse(bytes: &[u8]) -> Result<Self, ParseError> {
        let bytes: &[u8; BYTES] = bytes
            .try_into()
            .map_err(|_| ParseError::Length(bytes.len()))?;
        let sub_tick = SubTick::parse((bytes[1] != 255).then_some(bytes[1]))
            .ok_or(ParseError::Milliseconds(bytes[1]))?;
        let place = Place::parse(bytes[10]).ok_or(ParseError::Place(bytes[10]))?;
        Ok(Self {
            tick: bytes[0],
            sub_tick,
            rotation: bytes[2..6].try_into().unwrap(),
            actions: u32::from_le_bytes(bytes[6..10].try_into().unwrap()),
            target: DropId(Life {
                place,
                spawned: u32::from_le_bytes(bytes[11..15].try_into().unwrap()),
            }),
        })
    }

    pub fn bytes(self) -> [u8; BYTES] {
        let mut bytes = [0; BYTES];
        bytes[0] = self.tick;
        bytes[1] = self.sub_tick.0.unwrap_or(255);
        bytes[2..6].copy_from_slice(&self.rotation);
        bytes[6..10].copy_from_slice(&self.actions.to_le_bytes());
        bytes[10] = self.target.0.place.get();
        bytes[11..].copy_from_slice(&self.target.0.spawned.to_le_bytes());
        bytes
    }

    /// Native signed tick-byte difference, with rejection instead of wrapping
    /// before zero or past u32::MAX. This does not decide whether an input is
    /// within the native history; change_input reports its final placement.
    pub fn time_at(self, clock_ms: u32) -> Option<u32> {
        let current = clock_ms / 50;
        let delta = i64::from(self.tick.wrapping_sub(current as u8) as i8);
        let tick = i64::from(current) + delta;
        u32::try_from(tick * 50 + i64::from(self.sub_tick.0.unwrap_or(0))).ok()
    }

    /// Same bound as the match server's ordinary mouse input: finite halves,
    /// each with absolute value below 1024 radians.
    pub fn rotation_is_sane(self) -> bool {
        rotation_is_sane(self.rotation)
    }
}

/// Validate the ordinary mouse-input portion at either native or wire ingress.
pub fn rotation_is_sane(rotation: [u8; 4]) -> bool {
    rotation
        .chunks_exact(2)
        .all(|half| u16::from_le_bytes([half[0], half[1]]) & 0x7C00 < 25 << 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(tick: u8, ms: Option<u8>) -> WireInput {
        WireInput {
            tick,
            sub_tick: SubTick::parse(ms).unwrap(),
            rotation: [0, 0x3c, 0, 0xc0],
            actions: 0x8000_0021,
            target: DropId(Life {
                place: Place::parse(19).unwrap(),
                spawned: 0xfedc_ba98,
            }),
        }
    }

    #[test]
    fn round_trip_and_signed_tick_boundaries() {
        for ms in [None, Some(0), Some(49)] {
            let request = input(0, ms);
            assert_eq!(WireInput::parse(&request.bytes()), Ok(request));
            assert_eq!(
                request.time_at(12799),
                Some(12800 + u32::from(ms.unwrap_or(0)))
            );
        }
        assert_eq!(input(255, Some(49)).time_at(12800), Some(12799));
        assert_eq!(input(255, None).time_at(0), None);
        assert_eq!(
            input((u32::MAX / 50 + 1) as u8, Some(49)).time_at(u32::MAX),
            None
        );
        assert_ne!(input(0, None).bytes(), input(0, Some(0)).bytes());
    }

    #[test]
    fn invalid_wire_values_cannot_be_constructed() {
        for ms in 50..=255 {
            assert!(SubTick::parse(Some(ms)).is_none());
        }
        let valid = input(7, None).bytes();
        for len in 0..BYTES {
            assert_eq!(
                WireInput::parse(&valid[..len]),
                Err(ParseError::Length(len))
            );
        }
        assert_eq!(
            WireInput::parse(&[0; BYTES + 1]),
            Err(ParseError::Length(BYTES + 1))
        );
        for ms in 50..255 {
            let mut bytes = valid;
            bytes[1] = ms;
            assert_eq!(WireInput::parse(&bytes), Err(ParseError::Milliseconds(ms)));
        }
        for place in 32..=255 {
            let mut bytes = valid;
            bytes[10] = place;
            assert_eq!(WireInput::parse(&bytes), Err(ParseError::Place(place)));
        }
    }

    #[test]
    fn native_rotation_boundary_excludes_nonfinite_halves() {
        assert!(input(0, None).rotation_is_sane());
        for half in [0x7c00u16, 0xfc00, 0x7fff, 0xffff, 0x6400, 0xe400] {
            for component in [0, 2] {
                let mut request = input(0, None);
                request.rotation[component..component + 2].copy_from_slice(&half.to_le_bytes());
                assert!(!request.rotation_is_sane());
            }
        }
    }
}

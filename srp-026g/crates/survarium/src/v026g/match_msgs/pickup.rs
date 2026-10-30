//! A pickup target travels atomically with its normal timestamped input.
//! The server can move/discard the complete input; no separate target packet
//! can arrive late and accidentally attach to a different input or life.
//! The patched native transport/ingestion hooks are required for these IDs.

use super::{ParseError, PlayerInput, WireTime};
use weapon_pickup::input::{self, SubTick, WireInput};
pub use weapon_pickup::state::{DropId, Life, Place};

pub const INPUT_BYTES: usize = input::BYTES;
pub const RELAY_BYTES: usize = INPUT_BYTES + 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickupInput {
    pub input: PlayerInput,
    pub target: DropId,
}

impl PickupInput {
    /// Fixed layout: tick, precise-ms (FF for on-grid), rotation, actions,
    /// donor place, donor spawn time. A life is never shortened to a token.
    pub fn parse(bytes: &[u8]) -> Result<Self, ParseError> {
        let wire = WireInput::parse(bytes).map_err(|error| match error {
            input::ParseError::Length(got) => ParseError::Length {
                id: super::id::PICKUP_INPUT,
                expected: INPUT_BYTES,
                got,
            },
            input::ParseError::Milliseconds(ms) => ParseError::Milliseconds(ms),
            input::ParseError::Place(place) => ParseError::PickupTarget(place),
        })?;
        Ok(Self {
            input: PlayerInput {
                time: WireTime {
                    tick: wire.tick,
                    ms: wire.sub_tick.milliseconds(),
                },
                rotation: wire.rotation,
                actions: wire.actions,
            },
            target: wire.target,
        })
    }

    pub fn bytes(&self) -> [u8; INPUT_BYTES] {
        WireInput {
            tick: self.input.time.tick,
            sub_tick: SubTick::parse(self.input.time.ms).expect("valid input milliseconds"),
            rotation: self.input.rotation,
            actions: self.input.actions,
            target: self.target,
        }
        .bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::match_msgs::{
        ClientMessage, Length, ServerMessage, client_model, id, parse_client, server_length,
    };

    #[test]
    fn targeted_input_round_trips_without_changing_normal_input_bits() {
        for ms in [None, Some(0), Some(49)] {
            let request = PickupInput {
                input: PlayerInput {
                    time: WireTime { tick: 255, ms },
                    rotation: [1, 2, 3, 4],
                    actions: 0x8765_4321,
                },
                target: DropId(Life {
                    place: Place::parse(3).unwrap(),
                    spawned: 0xfedc_ba98,
                }),
            };
            assert_eq!(
                parse_client(id::PICKUP_INPUT, &request.bytes()),
                Ok(ClientMessage::PickupInput(request))
            );
            let relay = ServerMessage::PickupInputRelay {
                player: 1,
                input: request,
            };
            let payload = relay.payload();
            assert_eq!(payload.len(), RELAY_BYTES);
            assert_eq!(server_length(relay.id()), Some(Length::Fixed(RELAY_BYTES)));
            assert_eq!(
                client_model::decode_server(relay.id(), &payload, 4),
                Ok(relay)
            );
        }
    }

    #[test]
    fn refuses_partial_target_invalid_time_and_out_of_range_place() {
        let mut bytes = [0; INPUT_BYTES];
        bytes[1] = 255;
        assert!(PickupInput::parse(&bytes).is_ok());
        for len in 0..INPUT_BYTES {
            assert!(PickupInput::parse(&bytes[..len]).is_err());
        }
        assert!(PickupInput::parse(&[0; INPUT_BYTES + 1]).is_err());
        for ms in 50..255 {
            bytes[1] = ms;
            assert!(PickupInput::parse(&bytes).is_err());
        }
        bytes[1] = 255;
        for place in 32..=255 {
            bytes[10] = place;
            assert!(PickupInput::parse(&bytes).is_err());
        }
    }
}

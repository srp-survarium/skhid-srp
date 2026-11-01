use std::panic::Location;

use crate::network_packet::Packet;

pub trait Deserialize: Sized {
    /// Process a single message in the array of serialized messages.
    /// Advances `out_buffer` to the next message
    fn deserialize(buffer: &mut &[u8]) -> Result<Self, DeserializeError>;
}

pub trait Serialize: Sized {
    fn serialize(self, packet: &mut impl Packet);
}

#[derive(Debug, PartialEq, Clone, thiserror::Error)]
pub enum DeserializeError {
    #[error("NotEnoughInput")]
    NotEnoughInput,
    #[error("UnknownMessageType: {0}")]
    UnknownMessageType(u8),
    #[error("IncorrectInput: \"{msg}\" at '{location}'")]
    IncorrectInput {
        location: &'static std::panic::Location<'static>,
        msg: String,
    },
}

impl DeserializeError {
    #[track_caller]
    pub fn incorrect_input() -> Self {
        Self::IncorrectInput {
            location: Location::caller(),
            msg: String::new(),
        }
    }

    /// Don't use it pointless style with this function,
    /// as it will return incorrect location
    /// ```ignore
    /// let to_slot = bytemuck::checked::try_cast(to_slot)
    ///     .map_err(DeserializeError::incorrect_input_from)?
    /// // Location { file: "../library/core/src/ops//function.rs", ... }
    /// ```
    ///
    /// Fully expand arguments instead:
    /// ```ignore
    /// let to_slot = bytemuck::checked::try_cast(to_slot)
    ///     .map_err(|x| DeserializeError::incorrect_input_from(x))?
    /// // Location { file: "../lobby_server/message/client.rs", ... }
    #[track_caller]
    pub fn incorrect_input_from(msg: impl ToString) -> Self {
        Self::IncorrectInput {
            location: Location::caller(),
            msg: msg.to_string(),
        }
    }
}

//
//
//

#[inline]
#[track_caller]
pub fn advance_buffer<T: bytemuck::CheckedBitPattern>(
    buffer: &mut &[u8],
) -> Result<T, DeserializeError> {
    let size = std::mem::size_of::<T>();
    if buffer.len() < size {
        return Err(DeserializeError::NotEnoughInput);
    }

    // `map_err` breaks `track_caller`
    match bytemuck::checked::try_pod_read_unaligned(&buffer[0..size]) {
        Ok(value) => {
            *buffer = &buffer[size..];
            Ok(value)
        }
        Err(error) => Err(DeserializeError::incorrect_input_from(error)),
    }
}

/// Requires alignment to be matched
/// TODO: Possibly this function doesn't make sense, because alignment would only allow u8 types here
#[inline]
#[track_caller]
pub fn advance_by<'a, T: bytemuck::CheckedBitPattern>(
    n: usize,
    buffer: &mut &'a [u8],
) -> Result<&'a [T], DeserializeError> {
    let size = std::mem::size_of::<T>() * n;
    if buffer.len() < size {
        return Err(DeserializeError::NotEnoughInput);
    }

    // `map_err` breaks `track_caller`
    match bytemuck::checked::try_cast_slice::<_, T>(&buffer[0..size]) {
        Ok(value) => {
            *buffer = &buffer[size..];
            Ok(value)
        }
        Err(error) => Err(DeserializeError::incorrect_input_from(error)),
    }
}

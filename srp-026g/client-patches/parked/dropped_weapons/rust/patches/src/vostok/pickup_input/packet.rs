//! F7 dispatch owns parsing, reader advancement and target placement together.
//! Completion returns to the native recorder rather than skipping its tail.
use super::*;
use crate::vostok::{Context, Reg};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketError {
    Reader,
    Payload,
    Picker,
    Placement(ReceiveError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketOutcome {
    OutsideMatch,
    Input(Acceptance),
}

impl<const N: usize> InputJournals<N> {
    /// None is an unrelated native message: leave every field untouched.
    /// F7 is always consumed, including outside a match and on rejection.
    /// The caller must report errors and use the common native completion.
    pub fn packet(&self, ctx: &Context<'_>) -> Option<Result<PacketOutcome, PacketError>> {
        if ctx.reg(Reg::Eax) != transport::RELAY_ID {
            return None;
        }
        Some(self.receive_packet(ctx))
    }

    fn receive_packet(&self, ctx: &Context<'_>) -> Result<PacketOutcome, PacketError> {
        let reader = Object::at(ctx.reg(Reg::Ebx)).expect("missing dispatch reader");
        let client = Object::at(ctx.reg(Reg::Esi)).expect("missing dispatch client");
        // SAFETY: native dispatcher supplies this live message reader and
        // client. These are object fields, not addresses taken from payload.
        let (begin, cursor, size, state, world) = unsafe {
            (
                *raw::buffer_reader::begin(reader.ptr()),
                *raw::buffer_reader::cursor(reader.ptr()),
                *raw::buffer_reader::size(reader.ptr()),
                *raw::network_client::match_state(client.ptr()),
                *raw::network_client::match_world(client.ptr()),
            )
        };
        let end = begin.checked_add(size).ok_or(PacketError::Reader)?;
        if begin == 0 || cursor < begin || cursor > end {
            return Err(PacketError::Reader);
        }
        // SAFETY: end belongs to this reader. Native input handlers also
        // consume the payload when no live world is available.
        unsafe { raw::buffer_reader::cursor(reader.ptr()).write(end) };
        if state != 2 || world == 0 {
            return Ok(PacketOutcome::OutsideMatch);
        }
        if end - cursor != weapon_pickup::input::BYTES as u32 + 1 {
            return Err(PacketError::Payload);
        }
        // SAFETY: the transport gave the dispatcher a readable message;
        // the exact fixed payload and cursor bounds were checked above.
        let bytes = unsafe { core::slice::from_raw_parts(cursor as *const u8, 16) };
        let picker = Place::new(u32::from(bytes[0])).ok_or(PacketError::Picker)?;
        let input = WireInput::parse(&bytes[1..]).map_err(|_| PacketError::Payload)?;
        let world = Object::at(world).unwrap();
        // SAFETY: state2 and the nonnull wrapper identify the match world;
        // ordinary native input ingress reads these same core/clock fields.
        let (core, clock) = unsafe {
            (
                raw::input_world_wrapper::core(world.ptr()) as u32,
                *raw::input_world_wrapper::clock(world.ptr()),
            )
        };
        let clock = GameTime::from_raw(clock)
            .ok_or(PacketError::Placement(ReceiveError::TimeOutsideClock))?;
        // SAFETY: the embedded live core belongs to this dispatcher thread.
        let mut core = unsafe { InputCore::from_raw(ctx.thread(), core) }.unwrap();
        self.receive(&mut core, picker, input, clock)
            .map(PacketOutcome::Input)
            .map_err(PacketError::Placement)
    }
}

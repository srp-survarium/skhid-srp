//! Observe ordinary relay placement without replacing native decoding/calls.
use super::*;
use crate::vostok::{Context, Reg};

pub(super) struct OrdinaryRelay<'t> {
    pub core: InputCore<'t>,
    pub picker: Place,
}

impl<'t> OrdinaryRelay<'t> {
    /// The stats entry is shared by other handlers. Inspect only its return
    /// address until it proves this is an ordinary-input completion.
    pub fn completed(ctx: &'t Context<'_>) -> Option<(Self, NativePlacement)> {
        let stack = Object::at(ctx.reg(Reg::Esp).wrapping_add(8)).unwrap();
        // SAFETY: hook at native function entry, before its prologue.
        let returned_to = unsafe { *raw::input_stats_stack::return_address(stack.ptr()) };
        let effective = match returned_to {
            raw::image::SINGLE_RELAY_STATS_RETURN => unsafe {
                *raw::input_stats_stack::single_effective_time(stack.ptr())
            },
            raw::image::BATCH_RELAY_STATS_RETURN => {
                let frame = Object::at(ctx.reg(Reg::Ebp)).unwrap();
                // SAFETY: caller's batch frame is still current at this entry.
                unsafe { *raw::batch_input_frame::effective_time(frame.ptr()) }
            }
            _ => return None,
        };
        Some((
            Self::at(ctx, Reg::Edx),
            NativePlacement::parse(ctx.reg(Reg::Eax), effective),
        ))
    }

    fn at(ctx: &'t Context<'_>, client_register: Reg) -> Self {
        let client = Object::at(ctx.reg(client_register)).expect("missing ordinary relay client");
        let input = Object::at(ctx.reg(Reg::Esi)).expect("missing ordinary relay input");
        // SAFETY: the stats entry retains the decoded input in ESI.
        // The native caller checked the nonnull world wrapper.
        let (world, place) = unsafe {
            (
                *raw::network_client::match_world(client.ptr()),
                *raw::player_input::place(input.ptr()),
            )
        };
        let world = Object::at(world).expect("ordinary relay lost its world");
        let core = unsafe { raw::input_world_wrapper::core(world.ptr()) as u32 };
        Self {
            core: unsafe { InputCore::from_raw(ctx.thread(), core) }.unwrap(),
            picker: Place::new(u32::from(place)).expect("ordinary relay place outside roster"),
        }
    }

    pub fn finish<const N: usize>(self, result: NativePlacement, history: &mut InputHistory<N>) {
        let _ = self.core.record_relay(self.picker, None, result, history);
    }
}

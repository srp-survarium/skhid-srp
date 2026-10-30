//! Whole-node time changes outside network placement: local coalescing and
//! snapping to a simulation tick. No target is added or component replaced.
use super::*;
use crate::vostok::{Context, Reg};
use weapon_pickup::input_history::NodeKey;

#[must_use = "apply the native retime to the matching world's pickup journal"]
pub struct RetimedInput<'t> {
    pub(super) core: InputCore<'t>,
    node: NodeKey,
    picker: Place,
    time: Time,
}

impl<'t> RetimedInput<'t> {
    /// 1FB349: local coalescing has written ESI node's new time and merged
    /// its actions; EBX is core and EBP still holds change_input's arguments.
    pub fn at_local_merge(ctx: &'t Context<'_>) -> Self {
        let frame = Object::at(ctx.reg(Reg::Ebp)).expect("missing change-input frame");
        let node = Object::at(ctx.reg(Reg::Esi)).expect("missing locally merged input");
        // SAFETY: live frame/node at the named site, generated field offsets.
        let (place, time) = unsafe {
            (
                *raw::change_input_frame::place_flags(frame.ptr()) & 255,
                *raw::input_node::time(node.ptr()),
            )
        };
        Self::at(ctx, node.id(), place, time)
    }

    /// 2D4F3B: native snapping is about to write ESI to EDI node's time.
    /// The overwritten store still executes; this observer changes no native
    /// state and derives the timestamp directly from the native calculation.
    pub fn at_tick_snap(ctx: &'t Context<'_>) -> Self {
        let core = Object::at(ctx.reg(Reg::Ebx)).expect("missing snapping core");
        // SAFETY: live core; snap_inputs excludes the server's place sentinel.
        let place = unsafe { *raw::game_core::local_input_place(core.ptr()) };
        Self::at(ctx, ctx.reg(Reg::Edi), u32::from(place), ctx.reg(Reg::Esi))
    }

    fn at(ctx: &'t Context<'_>, node: u32, place: u32, time: u32) -> Self {
        Self {
            // SAFETY: both named sites receive their live input core in EBX.
            core: unsafe { InputCore::from_raw(ctx.thread(), ctx.reg(Reg::Ebx)) }
                .expect("missing retimed-input core"),
            node: NodeKey::new(node).expect("missing retimed-input identity"),
            picker: Place::new(place).expect("retimed-input place outside roster"),
            time: Time(
                GameTime::from_raw(time)
                    .expect("retimed-input time is absent")
                    .0,
            ),
        }
    }

    pub fn core_id(&self) -> CoreId {
        self.core.identity()
    }

    pub fn apply<const N: usize>(self, history: &mut InputHistory<N>) {
        history
            .apply(Change::Retime {
                node: self.node,
                picker: shared_place(self.picker),
                time: self.time,
            })
            .expect("retiming changed native input ownership");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::{Regs, context::Frame};
    use core::ffi::c_void;
    use weapon_pickup::state::Life;

    #[test]
    fn local_merge_and_snap_read_their_distinct_registers_and_preserve_native_state() {
        let mut storage = std::vec![0u32; 0x19000 / 4];
        let mut node = [0u32; 10];
        let mut native_frame = [0u32; 7];
        let core = storage.as_mut_ptr().cast::<c_void>();
        let node = node.as_mut_ptr().cast::<c_void>();
        unsafe {
            raw::game_core::local_input_place(core).write(3);
            raw::input_node::time(node).write(912);
            raw::change_input_frame::place_flags(native_frame.as_mut_ptr().cast()).write(0x103);
        }
        let picker = SharedPlace::parse(3).unwrap();
        let mut history = InputHistory::<2>::new();
        for (part, spawned) in [(Part::Original, 100), (Part::Corrected, 200)] {
            history
                .apply(Change::Accept {
                    node: NodeKey::new(node as u32).unwrap(),
                    picker,
                    time: Time(901),
                    part,
                    combine: Combine::Merge,
                    target: Some(DropId(Life {
                        place: picker,
                        spawned,
                    })),
                })
                .unwrap();
        }
        for snap in [false, true] {
            let mut regs = Regs {
                edi: if snap { node as u32 } else { 0 },
                esi: if snap { 950 } else { node as u32 },
                ebp: native_frame.as_mut_ptr() as u32,
                esp: 0,
                ebx: core as u32,
                edx: 0,
                ecx: 0,
                eax: 0,
                eflags: 0,
            };
            let mut fxsave = [0u8; 512];
            let mut frame = Frame {
                regs: &mut regs,
                fxsave: fxsave.as_mut_ptr(),
            };
            let ctx = unsafe { Context::new(&mut frame) };
            let receipt = if snap {
                RetimedInput::at_tick_snap(&ctx)
            } else {
                RetimedInput::at_local_merge(&ctx)
            };
            let expected_core = unsafe { InputCore::from_raw(ctx.thread(), core as u32) }.unwrap();
            assert_eq!(receipt.core_id(), expected_core.identity());
            receipt.apply(&mut history);
            let time = if snap { 950 } else { 912 };
            assert_eq!(history.at(Time(time)).count(), 2);
            assert!(
                history
                    .at(Time(if snap { 912 } else { 901 }))
                    .next()
                    .is_none()
            );
            // The observer does not perform the native snap store itself.
            assert_eq!(unsafe { *raw::input_node::time(node) }, 912);
        }
    }
}

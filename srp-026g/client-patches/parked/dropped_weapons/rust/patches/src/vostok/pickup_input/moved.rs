//! Observe native move after its inner placement, at 27D802. Reaching this
//! point proves source extraction occurred, even if placement was refused.
//! Source identity remains in EBX; no dereference of the extracted node is
//! needed, since native allocation may already have reused its storage.
use super::*;
use crate::vostok::{Context, Reg};
use weapon_pickup::input_history::NodeKey;

/// A completed extraction/placement, valid only through this native hook.
/// Observation does not invoke input placement a second time or alter the
/// native return value, output pointer, registers, or stack.
#[must_use = "apply the completed native move to its match's pickup journal"]
pub struct CompletedMove<'t> {
    pub(super) core: InputCore<'t>,
    source: NodeKey,
    picker: Place,
    placement: Result<GameTime, Refused>,
}

impl<'t> CompletedMove<'t> {
    pub(super) fn source(&self) -> NodeKey {
        self.source
    }

    pub fn core_id(&self) -> CoreId {
        self.core.identity()
    }

    /// At the move's 27D802 site, immediately before its epilogue. Earlier
    /// source refusals bypass this site and require no journal mutation.
    pub fn at_end(ctx: &'t Context<'_>) -> Self {
        let frame = Object::at(ctx.reg(Reg::Ebp)).expect("missing native move frame");
        // The lohook entry's dispatch slot and flags precede pushad's ESP.
        let stack =
            Object::at(ctx.reg(Reg::Esp).wrapping_add(8)).expect("missing native move stack");
        // SAFETY: this site's native EBP frame and ESP local are live through
        // the hook. The generated fields describe the arguments and output
        // consumed/written by the original move function and inner placement.
        let (core, place, effective) = unsafe {
            (
                *raw::move_input_frame::core(frame.ptr()),
                *raw::move_input_frame::place(frame.ptr()),
                *raw::move_input_end_stack::effective_time(stack.ptr()),
            )
        };
        let placement = match (ctx.reg(Reg::Eax), GameTime::from_raw(effective)) {
            (0 | 2 | 3, Some(time)) => Ok(time),
            (4, None) => Err(Refused::OutsideHistory),
            (6, None) => Err(Refused::BeforeResync),
            _ => panic!("native moved placement violated its status/output contract"),
        };
        Self {
            // SAFETY: native move supplied its own live core to inner
            // placement, and its epilogue has not released the frame yet.
            core: unsafe { InputCore::from_raw(ctx.thread(), core) }
                .expect("missing native move core"),
            source: NodeKey::new(ctx.reg(Reg::Ebx)).expect("missing extracted input identity"),
            picker: Place::new(place).expect("native move place outside roster"),
            placement,
        }
    }

    /// Complete target transfer before retiring nodes absent from the list:
    /// a source with no corrected portion was already unlinked by native code.
    /// The consuming API prevents accidentally applying this receipt twice.
    pub fn apply<const N: usize>(self, history: &mut InputHistory<N>) -> Placement {
        let after = self
            .core
            .nodes(self.picker)
            .expect("invalid input list after native move");
        let destination = match self.placement {
            Ok(time) => Some((
                after
                    .at(time)
                    .expect("moved input absent from native list")
                    .key,
                Time(time.0),
            )),
            Err(_) => None,
        };
        history
            .apply(Change::MoveOriginal {
                from: self.source,
                picker: shared_place(self.picker),
                destination,
            })
            .expect("native move changed input ownership");
        after.retain(history, self.picker);
        match self.placement {
            Ok(time) => Placement::Placed(time),
            Err(reason) => Placement::Refused(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::{Regs, context::Frame};
    use core::ffi::c_void;
    use weapon_pickup::state::Life;

    #[test]
    fn reads_native_effective_time_and_keeps_extracted_target_through_reuse_or_refusal() {
        for (status, effective, reused) in [
            (0, Some(900), false),
            (2, Some(1023), true),
            (4, None, false),
            (6, None, false),
        ] {
            let mut storage = std::vec![0u32; 0x19000 / 4];
            let mut lists = std::vec![0u32; Place::COUNT * 0x480 / 4];
            let mut source = [0u32; 10];
            let mut destination = [0u32; 10];
            let core = storage.as_mut_ptr().cast::<c_void>();
            let source = source.as_mut_ptr().cast::<c_void>();
            let destination = if reused {
                source
            } else {
                destination.as_mut_ptr().cast::<c_void>()
            };
            let mut frame = [0u32; 4];
            let mut stack = [0u32; 8];
            unsafe {
                raw::game_core::input_lists(core).write(lists.as_mut_ptr() as u32);
                raw::input_list::first(lists.as_mut_ptr().cast::<u8>().add(3 * 0x480).cast())
                    .write(if effective.is_some() {
                        destination as u32
                    } else {
                        0
                    });
                raw::input_node::time(destination).write(effective.unwrap_or(800));
                raw::move_input_frame::core(frame.as_mut_ptr().cast()).write(core as u32);
                raw::move_input_frame::place(frame.as_mut_ptr().cast()).write(3);
                raw::move_input_end_stack::effective_time(stack.as_mut_ptr().cast())
                    .write(effective.unwrap_or(GameTime::NONE_RAW));
            }
            let picker = SharedPlace::parse(3).unwrap();
            let source_key = NodeKey::new(source as u32).unwrap();
            let target = DropId(Life {
                place: picker,
                spawned: 200,
            });
            let mut history = InputHistory::<2>::new();
            history
                .apply(Change::Accept {
                    node: source_key,
                    picker,
                    time: Time(800),
                    part: Part::Original,
                    combine: Combine::Merge,
                    target: Some(target),
                })
                .unwrap();
            // Stale destination generation. If storage equals source, this
            // represents its corrected portion evicted by native allocation.
            history
                .apply(Change::Accept {
                    node: NodeKey::new(destination as u32).unwrap(),
                    picker,
                    time: Time(800),
                    part: Part::Corrected,
                    combine: Combine::Merge,
                    target: Some(DropId(Life {
                        place: picker,
                        spawned: 100,
                    })),
                })
                .unwrap();
            let original_regs = Regs {
                edi: 1,
                esi: 2,
                ebp: frame.as_mut_ptr() as u32,
                esp: (stack.as_mut_ptr() as u32).wrapping_sub(8),
                ebx: source as u32,
                edx: 3,
                ecx: 4,
                eax: status,
                eflags: 5,
            };
            let mut regs = original_regs;
            let mut fxsave = [0u8; 512];
            let mut hook_frame = Frame {
                regs: &mut regs,
                fxsave: fxsave.as_mut_ptr(),
            };
            let ctx = unsafe { Context::new(&mut hook_frame) };
            let receipt = CompletedMove::at_end(&ctx);
            let expected = match effective {
                Some(time) => Placement::Placed(GameTime(time)),
                None if status == 4 => Placement::Refused(Refused::OutsideHistory),
                None => Placement::Refused(Refused::BeforeResync),
            };
            assert_eq!(receipt.apply(&mut history), expected);
            assert!(history.at(Time(800)).next().is_none());
            if let Some(time) = effective {
                assert_eq!(
                    history.at(Time(time)).collect::<std::vec::Vec<_>>(),
                    [(picker, target)]
                );
            }
            assert_eq!(regs.eax, original_regs.eax);
            assert_eq!(regs.esp, original_regs.esp);
            assert_eq!(stack[7], effective.unwrap_or(GameTime::NONE_RAW));
        }
    }
}

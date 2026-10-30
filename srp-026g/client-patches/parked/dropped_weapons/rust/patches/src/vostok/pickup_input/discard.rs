//! Successful native F1 removal retires both target portions. Refused
//! discards bypass this site, so no caller must coordinate a second mutation.
use super::*;
use crate::vostok::{Context, Reg};
use weapon_pickup::input_history::NodeKey;

#[must_use = "apply the completed native discard to its match's pickup journal"]
pub struct CompletedDiscard<'t> {
    pub(super) core: InputCore<'t>,
    source: NodeKey,
    picker: Place,
}

impl<'t> CompletedDiscard<'t> {
    /// 27DCB7 is reached only after unlink, pool return and dirty notification.
    /// EBX still holds the removed node's identity; never dereference it.
    pub fn at_end(ctx: &'t Context<'_>) -> Self {
        let frame = Object::at(ctx.reg(Reg::Ebp)).expect("missing native discard frame");
        // SAFETY: the verified successful-discard epilogue still owns its
        // EBP frame; its arguments remain live until ret8 at 27DCBF.
        let (core, place) = unsafe {
            (
                *raw::discard_input_frame::core(frame.ptr()),
                *raw::discard_input_frame::place(frame.ptr()),
            )
        };
        Self {
            // SAFETY: this native operation supplied its live core argument.
            core: unsafe { InputCore::from_raw(ctx.thread(), core) }
                .expect("missing native discard core"),
            source: NodeKey::new(ctx.reg(Reg::Ebx)).expect("missing discarded input identity"),
            picker: Place::new(place).expect("native discard place outside roster"),
        }
    }

    pub fn core_id(&self) -> CoreId {
        self.core.identity()
    }

    pub fn apply<const N: usize>(self, history: &mut InputHistory<N>) {
        history.apply(Change::Retire(self.source)).unwrap();
        self.core
            .nodes(self.picker)
            .expect("invalid input list after native discard")
            .retain(history, self.picker);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::{Regs, context::Frame};
    use core::ffi::c_void;
    use weapon_pickup::state::Life;

    #[test]
    fn completed_discard_retires_both_portions_and_preserves_other_inputs() {
        let mut storage = std::vec![0u32; 0x19000 / 4];
        let mut lists = std::vec![0u32; Place::COUNT * 0x480 / 4];
        let mut survivor = [0u32; 10];
        let core = storage.as_mut_ptr().cast::<c_void>();
        let survivor = survivor.as_mut_ptr().cast::<c_void>();
        let mut frame = [0u32; 4];
        unsafe {
            raw::game_core::input_lists(core).write(lists.as_mut_ptr() as u32);
            raw::input_list::first(lists.as_mut_ptr().cast::<u8>().add(3 * 0x480).cast())
                .write(survivor as u32);
            raw::input_node::time(survivor).write(850);
            raw::discard_input_frame::core(frame.as_mut_ptr().cast()).write(core as u32);
            raw::discard_input_frame::place(frame.as_mut_ptr().cast()).write(3);
        }
        let picker = SharedPlace::parse(3).unwrap();
        let target = DropId(Life {
            place: picker,
            spawned: 50,
        });
        // Deliberately unreadable: the removed node is only an identity.
        let removed = NodeKey::new(1).unwrap();
        let mut history = InputHistory::<3>::new();
        for part in [Part::Original, Part::Corrected] {
            history
                .apply(Change::Accept {
                    node: removed,
                    picker,
                    time: Time(800),
                    part,
                    combine: Combine::Merge,
                    target: Some(target),
                })
                .unwrap();
        }
        history
            .apply(Change::Accept {
                node: NodeKey::new(survivor as u32).unwrap(),
                picker,
                time: Time(850),
                part: Part::Original,
                combine: Combine::Merge,
                target: Some(target),
            })
            .unwrap();
        assert!(!history.has_spare_capacity());
        let original_regs = Regs {
            edi: 1,
            esi: 2,
            ebp: frame.as_mut_ptr() as u32,
            esp: 3,
            ebx: 1,
            edx: 4,
            ecx: 5,
            eax: 6,
            eflags: 7,
        };
        let mut regs = original_regs;
        let mut fxsave = [0u8; 512];
        let mut hook_frame = Frame {
            regs: &mut regs,
            fxsave: fxsave.as_mut_ptr(),
        };
        let ctx = unsafe { Context::new(&mut hook_frame) };
        let receipt = CompletedDiscard::at_end(&ctx);
        assert_eq!(
            receipt.core_id(),
            CoreId(core::num::NonZeroU32::new(core as u32).unwrap())
        );
        receipt.apply(&mut history);
        assert!(history.at(Time(800)).next().is_none());
        assert_eq!(
            history.at(Time(850)).collect::<std::vec::Vec<_>>(),
            [(picker, target)]
        );
        assert_eq!(regs, original_regs);
    }
}

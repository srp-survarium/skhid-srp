use super::*;
use crate::vostok::{Regs, context::Frame};
use core::ffi::c_void;
use weapon_pickup::input_history::NodeKey;
use weapon_pickup::state::Life;

struct Fixture {
    wrapper: std::vec::Vec<u32>,
    world: std::vec::Vec<u32>,
    lists: std::vec::Vec<u32>,
    node: [u32; 10],
    stack: [u32; 32],
    player: std::vec::Vec<u32>,
    handler: std::vec::Vec<u32>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            wrapper: std::vec![0; 0x6500], // u32 words: embedded core plus wrapper
            world: std::vec![0; 0x4D50 / 4],
            lists: std::vec![0; Place::COUNT * 0x480 / 4],
            node: [0; 10],
            stack: [0; 32],
            player: std::vec![0; 0xF474 / 4],
            handler: std::vec![0; 0x348 / 4],
        }
    }

    fn core(&mut self) -> *mut c_void {
        unsafe { raw::input_world_wrapper::core(self.wrapper.as_mut_ptr().cast()).cast() }
    }

    fn node(&mut self) -> *mut c_void {
        self.node.as_mut_ptr().cast()
    }

    fn prepare(&mut self) {
        let core = self.core();
        let node = self.node();
        let stack = self.stack.as_mut_ptr().cast();
        unsafe {
            raw::game_world::match_wrapper(self.world.as_mut_ptr().cast())
                .write(self.wrapper.as_mut_ptr() as u32);
            raw::game_core::input_lists(core).write(self.lists.as_mut_ptr() as u32);
            raw::game_core::local_input_place(core).write(3);
            raw::game_core::pending_local_input(core).write(901);
            raw::input_list::first(self.lists.as_mut_ptr().cast::<u8>().add(3 * 0x480).cast())
                .write(node as u32);
            raw::input_node::time(node).write(901);
            // Native coalescing may retain an earlier movement action.
            raw::input_node::actions(node).write(ActionMask::USE.bits() | ActionMask::LEFT.bits());
            raw::local_input_end_stack::effective_time(stack).write(901);
            raw::local_input_end_stack::input_time(stack).write(901);
            raw::local_input_end_stack::input_actions(stack).write(ActionMask::USE.bits());
            raw::local_input_end_stack::input_place(stack).write(3);
            raw::local_input_end_stack::player(stack).write(self.player.as_mut_ptr() as u32);
            raw::player::input_handler(self.player.as_mut_ptr().cast())
                .write(self.handler.as_mut_ptr() as u32);
            raw::player::core(self.player.as_mut_ptr().cast()).write(core as u32);
            raw::player::place(self.player.as_mut_ptr().cast()).write(3);
            raw::player::alive(self.player.as_mut_ptr().cast()).write(1);
            raw::player::inserted(self.player.as_mut_ptr().cast()).write(1);
        }
    }

    fn attach<const N: usize>(
        &mut self,
        status: u32,
        target: DropId,
        history: &mut InputHistory<N>,
    ) -> Result<AcceptedPickup, LocalAttachError> {
        let mut regs = Regs {
            eax: status,
            ebx: self.world.as_mut_ptr() as u32,
            esp: (self.stack.as_mut_ptr() as u32) - 8,
            ecx: 0,
            edx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
        };
        let saved = regs;
        let mut fxsave = [0u8; 512];
        let mut frame = Frame {
            regs: &mut regs,
            fxsave: fxsave.as_mut_ptr(),
        };
        let ctx = unsafe { Context::new(&mut frame) };
        let result =
            CompletedLocalInput::at_end(&ctx).and_then(|input| input.attach(target, history));
        assert_eq!(regs, saved);
        result
    }
}

fn target(spawned: u32) -> DropId {
    DropId(Life {
        place: SharedPlace::parse(1).unwrap(),
        spawned,
    })
}

#[test]
fn accepted_use_attaches_once_and_promotes_only_its_native_timing_fields() {
    let mut f = Fixture::new();
    f.prepare();
    let mut history = InputHistory::<2>::new();
    let before_node = f.node;
    let before_wrapper = f.wrapper.clone();
    let result = f.attach(0, target(700), &mut history).unwrap();
    assert_eq!(result.target, target(700));
    assert_eq!(result.picker.get(), 3);
    assert_eq!(result.time, GameTime(901));
    assert_eq!(unsafe { *raw::input_node::precise(f.node()) }, 1);
    assert_eq!(
        unsafe { *raw::game_core::pending_local_input(f.core()) },
        GameTime::NONE_RAW
    );
    assert_eq!(
        history.original_target(
            NodeKey::new(f.node() as u32).unwrap(),
            shared_place(result.picker),
            Time(901)
        ),
        Ok(Some(target(700)))
    );
    let after = history.clone();
    assert!(f.attach(0, target(700), &mut history).is_ok());
    assert_eq!(history, after);
    assert_eq!(
        f.attach(0, target(701), &mut history),
        Err(LocalAttachError::TargetConflict)
    );
    assert_eq!(history, after);
    // Restore just the two intended fields and compare every native byte.
    unsafe {
        raw::input_node::precise(f.node()).write(0);
        raw::game_core::pending_local_input(f.core()).write(901);
    }
    assert_eq!(f.node, before_node);
    assert_eq!(f.wrapper, before_wrapper);
}

#[test]
fn refusal_does_not_change_native_precision_pending_cursor_or_journal() {
    for case in 0..16 {
        let mut f = Fixture::new();
        f.prepare();
        let mut history = InputHistory::<1>::new();
        let expected = unsafe {
            let stack = f.stack.as_mut_ptr().cast();
            match case {
                0 => {
                    raw::local_input_end_stack::effective_time(stack).write(GameTime::NONE_RAW);
                    LocalAttachError::NotAccepted
                }
                1 => {
                    raw::local_input_end_stack::input_time(stack).write(900);
                    LocalAttachError::NotAccepted
                }
                2 => {
                    raw::local_input_end_stack::input_actions(stack).write(0);
                    LocalAttachError::NotUse
                }
                3 => {
                    raw::local_input_end_stack::input_place(stack).write(20);
                    LocalAttachError::NotLocal
                }
                4 => {
                    raw::game_core::local_input_place(f.core()).write(2);
                    LocalAttachError::NotLocal
                }
                5 => {
                    raw::game_core::server_inputs(f.core()).write(1);
                    LocalAttachError::NotLocal
                }
                6 => {
                    raw::input_node::time(f.node()).write(900);
                    LocalAttachError::ChangedInput
                }
                7 => {
                    raw::input_node::corrected(f.node()).write(1);
                    LocalAttachError::CorrectedNode
                }
                8 => {
                    raw::input_node::actions(f.node()).write(0);
                    LocalAttachError::ChangedInput
                }
                9 => {
                    raw::game_core::pending_local_input(f.core()).write(850);
                    LocalAttachError::OtherPendingInput
                }
                10 => {
                    raw::input_node::next(f.node()).write(f.node() as u32);
                    LocalAttachError::InvalidList
                }
                11 => {
                    raw::player_input_handler::previous_actions(f.handler.as_mut_ptr().cast())
                        .write(ActionMask::USE.bits());
                    LocalAttachError::HeldUse
                }
                12 => {
                    raw::player::alive(f.player.as_mut_ptr().cast()).write(0);
                    LocalAttachError::NotAlive
                }
                13 => {
                    raw::player::inserted(f.player.as_mut_ptr().cast()).write(0);
                    LocalAttachError::NotAlive
                }
                14 => {
                    raw::player::place(f.player.as_mut_ptr().cast()).write(2);
                    LocalAttachError::NotLocal
                }
                _ => {
                    history
                        .apply(Change::Accept {
                            node: NodeKey::new(1).unwrap(),
                            picker: SharedPlace::parse(4).unwrap(),
                            time: Time(800),
                            part: Part::Original,
                            combine: Combine::Merge,
                            target: Some(target(600)),
                        })
                        .unwrap();
                    LocalAttachError::HistoryFull
                }
            }
        };
        let before = (f.wrapper.clone(), f.node, history.clone());
        assert_eq!(
            f.attach(0, target(700), &mut history),
            Err(expected),
            "case {case}"
        );
        assert_eq!((f.wrapper, f.node, history), before, "case {case}");
    }
    for status in [1, 2, 3, 4, 6] {
        let mut f = Fixture::new();
        f.prepare();
        let mut history = InputHistory::<1>::new();
        assert_eq!(
            f.attach(status, target(700), &mut history),
            Err(LocalAttachError::NotAccepted)
        );
        assert_eq!(unsafe { *raw::input_node::precise(f.node()) }, 0);
        assert_eq!(
            unsafe { *raw::game_core::pending_local_input(f.core()) },
            901
        );
        assert_eq!(history, InputHistory::new());
    }
}

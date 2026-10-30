//! Encode a targeted input at the native history sender. One operation owns
//! packet construction, payload writing and transfer to the existing queue.
use super::*;
use crate::vostok::{Context, Reg};
use weapon_pickup::input::SubTick;
use weapon_pickup::input_history::NodeKey;

pub(super) struct SendingInput<'t> {
    core: InputCore<'t>,
    node: NodeKey,
    picker: Place,
    time: GameTime,
    precise: bool,
    rotation: [u8; 4],
    actions: u32,
    transport: Object,
}

impl<'t> SendingInput<'t> {
    /// 27EAFF: ESI still identifies the chosen native node, ECX is the
    /// rotation selected by native original/corrected policy, EBP the send
    /// context. The original has already marked this history entry as sent.
    pub fn at(ctx: &'t Context<'_>) -> Self {
        let context = Object::at(ctx.reg(Reg::Ebp)).unwrap();
        let stack = Object::at(ctx.reg(Reg::Esp).wrapping_add(8)).unwrap();
        let node = Object::at(ctx.reg(Reg::Esi)).unwrap();
        unsafe {
            let transport_slot =
                Object::at(*raw::input_send_context::transport_slot(context.ptr())).unwrap();
            let transport =
                Object::at(*raw::input_transport_slot::transport(transport_slot.ptr())).unwrap();
            let core =
                InputCore::from_raw(ctx.thread(), *raw::input_send_context::core(context.ptr()))
                    .unwrap();
            let time = GameTime::from_raw(*raw::input_node::time(node.ptr())).unwrap();
            Self {
                core,
                node: NodeKey::new(node.id()).unwrap(),
                picker: Place::new(u32::from(*raw::input_send_stack::place(stack.ptr()))).unwrap(),
                time,
                precise: *raw::input_node::precise(node.ptr()) != 0,
                rotation: ctx.reg(Reg::Ecx).to_le_bytes(),
                actions: *raw::input_send_stack::actions(stack.ptr())
                    | (u32::from(*raw::input_node::action_high_bit(node.ptr()) != 0) << 31),
                transport,
            }
        }
    }

    pub fn core_id(&self) -> CoreId {
        self.core.identity()
    }

    pub fn target<const N: usize>(&self, history: &InputHistory<N>) -> Option<DropId> {
        history
            .original_target(self.node, shared_place(self.picker), Time(self.time.0))
            .expect("outgoing native input has multiple original pickup targets")
    }

    fn wire(&self, target: DropId) -> WireInput {
        assert!(
            self.precise || self.time.0.is_multiple_of(50),
            "unsnapped ordinary input sent"
        );
        WireInput {
            tick: (self.time.0 / 50) as u8,
            sub_tick: SubTick::parse(self.precise.then_some((self.time.0 % 50) as u8)).unwrap(),
            rotation: self.rotation,
            actions: self.actions,
            target,
        }
    }

    pub fn send(self, target: DropId) {
        let bytes = self.wire(target).bytes();
        // SAFETY: concrete live match transport; these bindings are its
        // vtable +18/+1C implementations. Construction initializes the native
        // packet writer; queueing takes ownership just as the stock sender.
        // No fallible validation or journal lock remains after allocation.
        unsafe {
            assert_eq!(
                *raw::input_match_transport::vtable(self.transport.ptr()),
                raw::image::INPUT_MATCH_TRANSPORT_VTABLE
            );
            let packet = raw::engine_match_new_packet(self.transport.ptr(), transport::REQUEST_ID);
            assert!(!packet.is_null(), "native pickup packet allocation failed");
            raw::engine_buffer_write(
                raw::input_match_packet::writer(packet).cast(),
                bytes.as_ptr().cast(),
                bytes.len() as u32,
            );
            raw::engine_match_enqueue(self.transport.ptr(), packet);
        }
    }
}

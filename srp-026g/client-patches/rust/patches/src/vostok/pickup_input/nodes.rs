//! A bounded copy of one native input list. Keys never expose dereferenceable
//! pointers to the shared journal. Reads occur only on the simulation thread.
use super::*;
use crate::vostok::world::{pointer, read};
use weapon_pickup::input_history::{Change, InputHistory, NodeKey};

// Original pool holds 27 nodes; server INPUT_RING uses the same bound.
const CAPACITY: usize = 27;

#[derive(Clone, Copy)]
pub(super) struct Node {
    native: Object,
    pub key: NodeKey,
    pub time: GameTime,
    pub corrected: bool,
}

pub(super) struct Nodes([Option<Node>; CAPACITY]);

impl InputCore<'_> {
    /// The generic native eviction routine also serves unrelated list types.
    /// Match an exact per-place input-list base before reading its node.
    pub(super) fn owns_list(&self, address: u32) -> bool {
        unsafe {
            let Some(lists) = pointer(raw::game_core::input_lists(self.this.ptr())) else {
                return false;
            };
            let stride = raw::input_list::end(lists.ptr()) as usize - lists.ptr() as usize;
            let Some(offset) = address.checked_sub(lists.id()) else {
                return false;
            };
            (offset as usize) < Place::COUNT * stride && (offset as usize).is_multiple_of(stride)
        }
    }

    pub(super) fn nodes(&self, picker: Place) -> Result<Nodes, ReceiveError> {
        let mut nodes = [None::<Node>; CAPACITY];
        // SAFETY: the input core and its 20 initialized input lists belong to
        // this live hook. Generated fields provide both the stride and links.
        unsafe {
            let lists = pointer(raw::game_core::input_lists(self.this.ptr()))
                .ok_or(ReceiveError::MissingInputs)?;
            let stride = raw::input_list::end(lists.ptr()) as usize - lists.ptr() as usize;
            let list = lists.ptr().byte_add(picker.index() * stride);
            let mut next = pointer(raw::input_list::first(list));
            let mut count = 0;
            while let Some(node) = next {
                let key = NodeKey::new(node.id()).unwrap();
                if count == CAPACITY || nodes.iter().flatten().any(|node| node.key == key) {
                    return Err(ReceiveError::InvalidInputList);
                }
                let time = GameTime::from_raw(read(raw::input_node::time(node.ptr())))
                    .ok_or(ReceiveError::InvalidInputList)?;
                nodes[count] = Some(Node {
                    native: node,
                    key,
                    time,
                    corrected: read(raw::input_node::corrected(node.ptr())) != 0,
                });
                count += 1;
                next = pointer(raw::input_node::next(node.ptr()));
            }
        }
        Ok(Nodes(nodes))
    }
}

impl Node {
    /// Private boundary operations, used only before the current input hook
    /// returns or calls native code that could evict this copied node.
    pub fn actions(self) -> u32 {
        unsafe { read(raw::input_node::actions(self.native.ptr())) }
    }

    pub fn make_precise(self) {
        unsafe { raw::input_node::precise(self.native.ptr()).write(1) };
    }
}

impl Nodes {
    pub fn at(&self, time: GameTime) -> Option<Node> {
        self.0
            .iter()
            .flatten()
            .find(|node| node.time == time)
            .copied()
    }

    pub fn retain<const N: usize>(&self, history: &mut InputHistory<N>, picker: Place) {
        let Some(first) = self.0[0] else {
            history
                .apply(Change::Retain {
                    picker: shared_place(picker),
                    nodes: &[],
                })
                .unwrap();
            return;
        };
        let mut keys = [first.key; CAPACITY];
        let mut len = 0;
        for node in self.0.iter().flatten() {
            keys[len] = node.key;
            len += 1;
        }
        history
            .apply(Change::Retain {
                picker: shared_place(picker),
                nodes: &keys[..len],
            })
            .unwrap();
    }
}

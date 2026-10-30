//! Native placement of a targeted input. The effective timestamp comes from
//! change_input itself, not a reimplementation of its history/clamping rules.
//! Placement and target-journal insertion are coordinated in one operation.
//! F7 transport dispatch calls this boundary; pickup execution is still pending.

use super::types::{GameTime, Place};
use super::world::Object;
use super::{Handle, Thread};
use crate::raw;
use weapon_pickup::input::WireInput;
use weapon_pickup::input_history::{Change, Combine, InputHistory, Part, Time};
use weapon_pickup::state::{DropId, Place as SharedPlace};

mod discard;
mod drawn;
pub use drawn::DrawnWeapon;
mod local;
pub use local::{CompletedLocalInput, LocalAttachError};
mod nodes;
mod owners;
pub use owners::InputJournals;
mod retimed;
pub use retimed::RetimedInput;
mod moved;
pub use moved::CompletedMove;
mod ordinary;
mod sending;
mod packet;
mod relay;
pub mod transport;
pub use discard::CompletedDiscard;
pub use packet::{PacketError, PacketOutcome};
pub use relay::{Placement, RelayInput};

fn shared_place(place: Place) -> SharedPlace {
    SharedPlace::parse(place.get()).unwrap()
}

/// Opaque identity used to associate a journal with one native world lifetime.
/// The match owner must retire it at core teardown before address reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CoreId(core::num::NonZeroU32);

/// A world core borrowed only while a simulation-thread hook is executing.
pub struct InputCore<'t> {
    this: Object,
    _thread: &'t Thread,
}

impl<'t> Handle<'t> for InputCore<'t> {
    unsafe fn from_raw(thread: &'t Thread, address: u32) -> Option<Self> {
        Some(Self {
            this: Object::at(address)?,
            _thread: thread,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiveError {
    TimeOutsideClock,
    InvalidRotation,
    TargetOutsideRoster,
    MissingInputs,
    InvalidInputList,
    HistoryFull,
    ReceiveInProgress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    BeforeResync,
    OutsideHistory,
}

/// Only a successfully placed input carries a target for simulation. A
/// duplicate never manufactures a second event or changes an existing target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "handle native input rejection at the transport boundary"]
pub enum Acceptance {
    Placed(AcceptedPickup),
    Duplicate,
    Refused(Refused),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AcceptedPickup {
    pub picker: Place,
    pub time: GameTime,
    pub target: DropId,
}

impl InputCore<'_> {
    pub fn identity(&self) -> CoreId {
        CoreId(core::num::NonZeroU32::new(self.this.id()).unwrap())
    }

    /// Ingest one server relay. The donor life remains unchanged even if the
    /// engine moves this input to another timestamp. Network retries use the
    /// engine's own duplicate handling, without overriding it.
    fn receive<const N: usize>(
        &mut self,
        picker: Place,
        input: WireInput,
        clock: GameTime,
        history: &mut InputHistory<N>,
    ) -> Result<Acceptance, ReceiveError> {
        if Place::new(u32::from(input.target.0.place.get())).is_none() {
            return Err(ReceiveError::TargetOutsideRoster);
        }
        let time = input
            .time_at(clock.0)
            .and_then(GameTime::from_raw)
            .ok_or(ReceiveError::TimeOutsideClock)?;
        let relay = RelayInput::new(
            time,
            input.rotation,
            input.actions,
            input.sub_tick.milliseconds().is_some(),
        )?;
        Ok(
            match self.place_relay(picker, relay, Some(input.target), history)? {
                Placement::Placed(time) => Acceptance::Placed(AcceptedPickup {
                    picker,
                    time,
                    target: input.target,
                }),
                Placement::Duplicate => Acceptance::Duplicate,
                Placement::Refused(reason) => Acceptance::Refused(reason),
            },
        )
    }

    // Ordinary relays use this same transaction with no target: replacing
    // their original input portion also replaces its attached pickup.
    fn place_relay<const N: usize>(
        &mut self,
        picker: Place,
        input: RelayInput,
        target: Option<DropId>,
        history: &mut InputHistory<N>,
    ) -> Result<Placement, ReceiveError> {
        let before = self.nodes(picker)?;
        before.retain(history, picker);
        // Reserve room before native mutation. The operation accepts at most
        // one new target; retiring evicted nodes can only free further room.
        if target.is_some() && !history.has_spare_capacity() {
            return Err(ReceiveError::HistoryFull);
        }
        // This sentinel belongs to the native boundary. The native relay
        // caller initializes the same output to -1 at 27C314; successful
        // paths write it, duplicate/refusal paths leave it untouched.
        let mut effective = GameTime::NONE_RAW;
        // SAFETY: the Handle contract supplies a live core and its initialized
        // input pools on this thread. Place is bounded to the native 20 slots;
        // half rotations are finite and bounded. All arguments are by value
        // except the output, which lives through the synchronous native call.
        let status = unsafe {
            raw::engine_change_player_input(
                self.this.ptr(),
                input.rotation,
                input.actions & !(1 << 31),
                input.time.0,
                u32::from(picker.get()) | ((input.actions >> 31) << 8),
                1,
                0,
                u32::from(input.precise),
                &mut effective,
            )
        };
        Ok(self.record_relay(
            picker,
            target,
            NativePlacement::parse(status, effective),
            history,
        ))
    }

    fn record_relay<const N: usize>(
        &self,
        picker: Place,
        target: Option<DropId>,
        result: NativePlacement,
        history: &mut InputHistory<N>,
    ) -> Placement {
        match result {
            NativePlacement::Accepted { time, part } => {
                let after = self
                    .nodes(picker)
                    .expect("native input list invalid after placement");
                let node = after
                    .at(time)
                    .expect("accepted input absent from native list");
                after.retain(history, picker);
                // Network placement never retimes an existing node. A
                // different timestamp on its identity is a recycled pool slot.
                history
                    .apply(Change::RetireStale {
                        node: node.key,
                        time: Time(time.0),
                    })
                    .unwrap();
                let local = unsafe { *raw::game_core::local_input_place(self.this.ptr()) };
                let server = unsafe { *raw::game_core::server_inputs(self.this.ptr()) != 0 };
                let combine = if part == Part::Original
                    && !server
                    && (local == picker.get() || !node.corrected)
                {
                    Combine::Replace
                } else {
                    Combine::Merge
                };
                history
                    .apply(Change::Accept {
                        node: node.key,
                        picker: shared_place(picker),
                        time: Time(time.0),
                        part,
                        combine,
                        target,
                    })
                    .expect("reserved pickup history capacity or node ownership changed");
                Placement::Placed(time)
            }
            NativePlacement::Duplicate => Placement::Duplicate,
            NativePlacement::Refused(reason) => Placement::Refused(reason),
        }
    }
}

#[derive(Clone, Copy)]
enum NativePlacement {
    Accepted { time: GameTime, part: Part },
    Duplicate,
    Refused(Refused),
}

impl NativePlacement {
    fn parse(status: u32, effective: u32) -> Self {
        match (status, GameTime::from_raw(effective)) {
            (0, Some(time)) => Self::Accepted {
                time,
                part: Part::Original,
            },
            (2 | 3, Some(time)) => Self::Accepted {
                time,
                part: Part::Corrected,
            },
            (1, None) => Self::Duplicate,
            (4, None) => Self::Refused(Refused::OutsideHistory),
            (6, None) => Self::Refused(Refused::BeforeResync),
            _ => panic!("native input placement violated its status/output-time contract"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::c_void;
    use std::cell::RefCell;
    use std::vec::Vec;
    use weapon_pickup::input::SubTick;
    use weapon_pickup::state::{Life, Place as DropPlace};

    std::thread_local! {
        pub(super) static NATIVE: RefCell<(u32, Option<u32>, Vec<[u32; 7]>)> = const { RefCell::new((0, None, Vec::new())) };
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn engine_change_player_input(
        core: *mut c_void,
        rotation: u32,
        actions: u32,
        time: u32,
        place_flags: u32,
        network: u32,
        corrected: u32,
        precise: u32,
        effective: *mut u32,
    ) -> u32 {
        assert!(!core.is_null());
        NATIVE.with_borrow_mut(|(status, placed, calls)| {
            calls.push([
                rotation,
                actions,
                time,
                place_flags,
                network,
                corrected,
                precise,
            ]);
            if let Some(time) = placed {
                unsafe {
                    effective.write(*time);
                    let lists = *raw::game_core::input_lists(core) as *mut c_void;
                    let stride = raw::input_list::end(lists) as usize - lists as usize;
                    let list = lists.byte_add((place_flags & 255) as usize * stride);
                    let node = (*raw::input_list::first(list)) as *mut c_void;
                    assert!(!node.is_null());
                    raw::input_node::time(node).write(*time);
                }
            }
            *status
        })
    }

    pub(super) fn input() -> WireInput {
        WireInput {
            tick: 18,
            sub_tick: SubTick::parse(Some(23)).unwrap(),
            rotation: [0, 0x3c, 0, 0xc0],
            actions: 0x8000_0021,
            target: DropId(Life {
                place: DropPlace::parse(19).unwrap(),
                spawned: 0xfedc_ba98,
            }),
        }
    }

    #[test]
    fn preserves_target_at_native_effective_time_and_never_replays_duplicates() {
        let thread = unsafe { Thread::new() };
        // Real-shaped input list; the fixture replaces only native placement.
        let mut storage = std::vec![0u32; 0x19000 / 4];
        let mut lists = std::vec![0u32; Place::COUNT * 0x480 / 4];
        let mut native_node = [0u32; 10];
        let ptr = storage.as_mut_ptr().cast::<c_void>();
        unsafe {
            raw::game_core::input_lists(ptr).write(lists.as_mut_ptr() as u32);
            raw::input_list::first(lists.as_mut_ptr().cast::<u8>().add(3 * 0x480).cast())
                .write(native_node.as_mut_ptr() as u32);
            raw::input_node::time(native_node.as_mut_ptr().cast()).write(923);
        }
        let mut core = unsafe { InputCore::from_raw(&thread, ptr as u32) }.unwrap();
        let picker = Place::new(3).unwrap();
        for (status, at, expected) in [
            (
                0,
                Some(923),
                Acceptance::Placed(AcceptedPickup {
                    picker,
                    time: GameTime(923),
                    target: input().target,
                }),
            ),
            (
                2,
                Some(1023),
                Acceptance::Placed(AcceptedPickup {
                    picker,
                    time: GameTime(1023),
                    target: input().target,
                }),
            ),
            (
                3,
                Some(823),
                Acceptance::Placed(AcceptedPickup {
                    picker,
                    time: GameTime(823),
                    target: input().target,
                }),
            ),
            (1, None, Acceptance::Duplicate),
            (4, None, Acceptance::Refused(Refused::OutsideHistory)),
            (6, None, Acceptance::Refused(Refused::BeforeResync)),
        ] {
            NATIVE.with_borrow_mut(|v| *v = (status, at, Vec::new()));
            let mut history = InputHistory::<8>::new();
            assert_eq!(
                core.receive(picker, input(), GameTime(1000), &mut history),
                Ok(expected)
            );
            if let Acceptance::Placed(accepted) = expected {
                assert_eq!(
                    history.at(Time(accepted.time.0)).collect::<Vec<_>>(),
                    [(shared_place(picker), input().target)]
                );
            } else {
                assert!(history.at(Time(923)).next().is_none());
            }
            NATIVE.with_borrow(|v| assert_eq!(v.2, [[0xc000_3c00, 0x21, 923, 0x103, 1, 0, 1]]));
        }
    }

    #[test]
    fn capacity_cycles_and_pool_reuse_cannot_lose_or_reassign_a_target() {
        use weapon_pickup::input_history::NodeKey;
        let thread = unsafe { Thread::new() };
        let mut storage = std::vec![0u32; 0x19000 / 4];
        let mut lists = std::vec![0u32; Place::COUNT * 0x480 / 4];
        let mut native_node = [0u32; 10];
        let ptr = storage.as_mut_ptr().cast::<c_void>();
        let node = native_node.as_mut_ptr().cast::<c_void>();
        let picker = Place::new(3).unwrap();
        unsafe {
            raw::game_core::input_lists(ptr).write(lists.as_mut_ptr() as u32);
            raw::input_list::first(lists.as_mut_ptr().cast::<u8>().add(3 * 0x480).cast())
                .write(node as u32);
            raw::input_node::time(node).write(800);
        }
        let key = NodeKey::new(node as u32).unwrap();
        let mut core = unsafe { InputCore::from_raw(&thread, ptr as u32) }.unwrap();
        let old = DropId(Life {
            place: DropPlace::parse(1).unwrap(),
            spawned: 50,
        });
        let existing = || Change::Accept {
            node: key,
            picker: shared_place(picker),
            time: Time(800),
            part: Part::Original,
            combine: Combine::Merge,
            target: Some(old),
        };
        let mut full = InputHistory::<1>::new();
        full.apply(existing()).unwrap();
        NATIVE.with_borrow_mut(|v| *v = (0, Some(923), Vec::new()));
        assert_eq!(
            core.receive(picker, input(), GameTime(1000), &mut full),
            Err(ReceiveError::HistoryFull)
        );
        assert_eq!(
            full.at(Time(800)).collect::<Vec<_>>(),
            [(shared_place(picker), old)]
        );
        NATIVE.with_borrow(|v| assert!(v.2.is_empty()));

        let mut history = InputHistory::<4>::new();
        history.apply(existing()).unwrap();
        unsafe {
            raw::input_node::next(node).write(node as u32);
        }
        assert_eq!(
            core.receive(picker, input(), GameTime(1000), &mut history),
            Err(ReceiveError::InvalidInputList)
        );
        NATIVE.with_borrow(|v| assert!(v.2.is_empty()));
        unsafe {
            raw::input_node::next(node).write(0);
        }
        // The fixture reuses this node's storage for a new network time.
        assert!(matches!(
            core.receive(picker, input(), GameTime(1000), &mut history)
                .unwrap(),
            Acceptance::Placed(_)
        ));
        assert!(history.at(Time(800)).next().is_none());
        assert_eq!(
            history.at(Time(923)).collect::<Vec<_>>(),
            [(shared_place(picker), input().target)]
        );

        let mut moved = input();
        moved.target = old;
        NATIVE.with_borrow_mut(|v| *v = (2, Some(923), Vec::new()));
        assert!(matches!(
            core.receive(picker, moved, GameTime(1000), &mut history)
                .unwrap(),
            Acceptance::Placed(_)
        ));
        assert_eq!(history.at(Time(923)).count(), 2);
        // Another corrected request coalesces at the same native node.
        moved.target.0.spawned = 60;
        assert!(matches!(
            core.receive(picker, moved, GameTime(1000), &mut history)
                .unwrap(),
            Acceptance::Placed(_)
        ));
        assert_eq!(history.at(Time(923)).count(), 3);
    }

    #[test]
    fn refuses_bad_native_arguments_before_any_mutation() {
        let thread = unsafe { Thread::new() };
        let mut storage = std::vec![0u32; 0x19000 / 4];
        let mut lists = std::vec![0u32; Place::COUNT * 0x480 / 4];
        let mut native_node = [0u32; 10];
        let ptr = storage.as_mut_ptr().cast::<c_void>();
        unsafe {
            raw::game_core::input_lists(ptr).write(lists.as_mut_ptr() as u32);
            raw::input_list::first(lists.as_mut_ptr().cast::<u8>().add(3 * 0x480).cast())
                .write(native_node.as_mut_ptr() as u32);
            raw::input_node::time(native_node.as_mut_ptr().cast()).write(923);
        }
        let mut core = unsafe { InputCore::from_raw(&thread, ptr as u32) }.unwrap();
        let picker = Place::new(0).unwrap();
        NATIVE.with_borrow_mut(|v| *v = (0, Some(923), Vec::new()));
        let mut history = InputHistory::<8>::new();
        let mut bad = input();
        bad.rotation = [0xff; 4];
        assert_eq!(
            core.receive(picker, bad, GameTime(1000), &mut history),
            Err(ReceiveError::InvalidRotation)
        );
        let mut bad = input();
        bad.target.0.place = DropPlace::parse(20).unwrap();
        assert_eq!(
            core.receive(picker, bad, GameTime(1000), &mut history),
            Err(ReceiveError::TargetOutsideRoster)
        );
        let mut bad = input();
        bad.tick = 255;
        assert_eq!(
            core.receive(picker, bad, GameTime(0), &mut history),
            Err(ReceiveError::TimeOutsideClock)
        );
        NATIVE.with_borrow(|v| assert!(v.2.is_empty()));
    }

    #[test]
    fn ordinary_relays_replace_only_original_targets_and_need_no_spare_capacity() {
        use weapon_pickup::input_history::NodeKey;
        let thread = unsafe { Thread::new() };
        let mut storage = std::vec![0u32; 0x19000 / 4];
        let mut lists = std::vec![0u32; Place::COUNT * 0x480 / 4];
        let mut native_node = [0u32; 10];
        let ptr = storage.as_mut_ptr().cast::<c_void>();
        let node = native_node.as_mut_ptr().cast::<c_void>();
        let picker = Place::new(3).unwrap();
        unsafe {
            raw::game_core::input_lists(ptr).write(lists.as_mut_ptr() as u32);
            raw::game_core::local_input_place(ptr).write(picker.get());
            raw::input_list::first(lists.as_mut_ptr().cast::<u8>().add(3 * 0x480).cast())
                .write(node as u32);
            raw::input_node::time(node).write(923);
            raw::input_node::corrected(node).write(1);
        }
        let key = NodeKey::new(node as u32).unwrap();
        let mut history = InputHistory::<2>::new();
        let mut corrected = input().target;
        corrected.0.spawned -= 1;
        for (part, target) in [
            (Part::Original, input().target),
            (Part::Corrected, corrected),
        ] {
            history
                .apply(Change::Accept {
                    node: key,
                    picker: shared_place(picker),
                    time: Time(923),
                    part,
                    combine: Combine::Merge,
                    target: Some(target),
                })
                .unwrap();
        }
        let mut core = unsafe { InputCore::from_raw(&thread, ptr as u32) }.unwrap();
        let regular =
            RelayInput::new(GameTime(923), input().rotation, input().actions, true).unwrap();
        NATIVE.with_borrow_mut(|v| *v = (1, None, Vec::new()));
        let before = history.clone();
        assert_eq!(
            core.place_relay(picker, regular, None, &mut history),
            Ok(Placement::Duplicate)
        );
        assert_eq!(history, before);
        NATIVE.with_borrow_mut(|v| *v = (0, Some(923), Vec::new()));
        assert_eq!(
            core.place_relay(picker, regular, None, &mut history),
            Ok(Placement::Placed(GameTime(923)))
        );
        assert_eq!(
            history.at(Time(923)).collect::<Vec<_>>(),
            [(shared_place(picker), corrected)]
        );
        NATIVE.with_borrow(|v| assert_eq!(v.2, [[0xc000_3c00, 0x21, 923, 0x103, 1, 0, 1]]));

        // A remotely merged/retimed ordinary input adds no pickup and keeps
        // existing corrected targets at the resulting native node.
        NATIVE.with_borrow_mut(|v| *v = (2, Some(923), Vec::new()));
        assert_eq!(
            core.place_relay(picker, regular, None, &mut history),
            Ok(Placement::Placed(GameTime(923)))
        );
        assert_eq!(
            history.at(Time(923)).collect::<Vec<_>>(),
            [(shared_place(picker), corrected)]
        );
        assert_eq!(
            RelayInput::new(GameTime(923), [255; 4], 0, false),
            Err(ReceiveError::InvalidRotation)
        );
        assert_eq!(
            RelayInput::new(GameTime(GameTime::NONE_RAW), [0; 4], 0, false),
            Err(ReceiveError::TimeOutsideClock)
        );
    }
}

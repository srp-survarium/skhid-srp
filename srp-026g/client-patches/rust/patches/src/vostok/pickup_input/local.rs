//! Attach a selected pickup to the accepted local input, before simulation
//! consumes it. Native local placement ignores its precise argument; promote
//! the completed node and retire its pending tick snap as one transaction.

use super::*;
use crate::vostok::world::pointer;
use crate::vostok::{ActionMask, Context, Reg};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalAttachError {
    NotAccepted,
    NotUse,
    HeldUse,
    NotLocal,
    NotAlive,
    ChangedInput,
    CorrectedNode,
    OtherPendingInput,
    TargetOutsideRoster,
    TargetConflict,
    HistoryFull,
    InvalidList,
}

/// A local placement receipt borrowed from the native caller's live frame.
/// It cannot be retained past that hook or reconstructed from packet arrival.
pub struct CompletedLocalInput<'t> {
    pub(super) core: InputCore<'t>,
    picker: Place,
    time: GameTime,
}

impl<'t> CompletedLocalInput<'t> {
    /// game_world::tick at1F7928, after wrapper1EEDE0 has returned and popped
    /// its three arguments. The effective output is siteESP+14, native input
    /// is still siteESP+20. EBX is game_world, EAX the placement return value.
    pub fn at_end(ctx: &'t Context<'_>) -> Result<Self, LocalAttachError> {
        let core_id = super::drawn::drawing_core(ctx).ok_or(LocalAttachError::NotLocal)?;
        let stack = Object::at(ctx.reg(Reg::Esp).wrapping_add(8)).unwrap();
        // SAFETY: the named completion site retains the native caller's input
        // and effective-time output. Context ESP includes the lohook's two
        // saved words, removed above. Core is embedded in the live wrapper.
        unsafe {
            let time = GameTime::from_raw(*raw::local_input_end_stack::effective_time(stack.ptr()))
                .ok_or(LocalAttachError::NotAccepted)?;
            if ctx.reg(Reg::Eax) != 0
                || *raw::local_input_end_stack::input_time(stack.ptr()) != time.0
            {
                return Err(LocalAttachError::NotAccepted);
            }
            let picker = Place::new(u32::from(*raw::local_input_end_stack::input_place(
                stack.ptr(),
            )))
            .ok_or(LocalAttachError::NotLocal)?;
            let actions = *raw::local_input_end_stack::input_actions(stack.ptr());
            if actions & ActionMask::USE.bits() == 0 {
                return Err(LocalAttachError::NotUse);
            }
            let player = pointer(raw::local_input_end_stack::player(stack.ptr()))
                .ok_or(LocalAttachError::NotLocal)?;
            if *raw::player::core(player.ptr()) != core_id.0.get()
                || *raw::player::place(player.ptr()) != picker.get()
            {
                return Err(LocalAttachError::NotLocal);
            }
            if *raw::player::alive(player.ptr()) == 0 || *raw::player::inserted(player.ptr()) == 0 {
                return Err(LocalAttachError::NotAlive);
            }
            let handler = pointer(raw::player::input_handler(player.ptr()))
                .ok_or(LocalAttachError::NotLocal)?;
            if *raw::player_input_handler::previous_actions(handler.ptr()) & ActionMask::USE.bits()
                != 0
            {
                return Err(LocalAttachError::HeldUse);
            }
            Ok(Self {
                core: InputCore::from_raw(ctx.thread(), core_id.0.get()).unwrap(),
                picker,
                time,
            })
        }
    }

    pub fn core_id(&self) -> CoreId {
        self.core.identity()
    }

    /// All refusals precede journal/native mutation. The match owner calls
    /// this under its journal lock after selecting a currently available drop.
    /// No callbacks, allocator or native engine calls occur under that lock.
    pub(super) fn attach<const N: usize>(
        self,
        target: DropId,
        history: &mut InputHistory<N>,
    ) -> Result<AcceptedPickup, LocalAttachError> {
        if Place::new(u32::from(target.0.place.get())).is_none() {
            return Err(LocalAttachError::TargetOutsideRoster);
        }
        unsafe {
            if *raw::game_core::local_input_place(self.core.this.ptr()) != self.picker.get()
                || *raw::game_core::server_inputs(self.core.this.ptr()) != 0
            {
                return Err(LocalAttachError::NotLocal);
            }
        }
        let nodes = self
            .core
            .nodes(self.picker)
            .map_err(|_| LocalAttachError::InvalidList)?;
        let node = nodes.at(self.time).ok_or(LocalAttachError::ChangedInput)?;
        if node.corrected {
            // Native sender selects the separate original component for
            // corrected nodes; attaching to combined local actions is unsafe.
            return Err(LocalAttachError::CorrectedNode);
        }
        let pending = unsafe {
            if node.actions() & ActionMask::USE.bits() == 0 {
                return Err(LocalAttachError::ChangedInput);
            }
            GameTime::from_raw(*raw::game_core::pending_local_input(self.core.this.ptr()))
        };
        if pending.is_some_and(|time| time != self.time) {
            // snap_inputs walks forward through subsequent history events,
            // including precise nodes. Do not strand an earlier pending input
            // or leave a cursor that can move this target after promotion.
            return Err(LocalAttachError::OtherPendingInput);
        }
        let existing = history
            .original_target(node.key, shared_place(self.picker), Time(self.time.0))
            .map_err(|_| LocalAttachError::TargetConflict)?;
        if existing.is_some_and(|old| old != target) {
            return Err(LocalAttachError::TargetConflict);
        }
        // Clone preflight is intentional: rejection must not retire entries
        // for other still-live inputs or leave native precision half changed.
        let mut updated = history.clone();
        nodes.retain(&mut updated, self.picker);
        updated
            .apply(Change::RetireStale {
                node: node.key,
                time: Time(self.time.0),
            })
            .unwrap();
        updated
            .apply(Change::Accept {
                node: node.key,
                picker: shared_place(self.picker),
                time: Time(self.time.0),
                part: Part::Original,
                combine: Combine::Merge,
                target: Some(target),
            })
            .map_err(|error| match error {
                weapon_pickup::input_history::Error::Full => LocalAttachError::HistoryFull,
                weapon_pickup::input_history::Error::NodeOwnerChanged => {
                    LocalAttachError::ChangedInput
                }
            })?;
        *history = updated;
        // SAFETY: exact live node in this core's local list, after completed
        // placement and before any simulation/send. All fallible work is done.
        // Native fire transitions make these same two field changes. Future
        // local inputs skip precise nodes as merge candidates at1FB256.
        node.make_precise();
        unsafe {
            raw::game_core::pending_local_input(self.core.this.ptr()).write(GameTime::NONE_RAW);
        }
        Ok(AcceptedPickup {
            picker: self.picker,
            time: self.time,
            target,
        })
    }
}

#[cfg(test)]
mod tests;

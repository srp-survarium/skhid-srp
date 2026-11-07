//! Read the live loader's bounds once, before the native indexed access.
use super::{Context, Reg};
use super::world::read;
use crate::raw;

pub fn fsm_has_state(ctx: &Context<'_>) -> bool {
    // SAFETY: ESI is the live FSM at this site; the search already stored
    // its selected state. No state pointer is followed here.
    unsafe { raw::site::deserialize_fsm_selected::fsm(ctx)
        .is_some_and(|fsm| read(fsm.selected()) != 0) }
}

pub fn anomaly_has_index(ctx: &Context<'_>) -> bool {
    // SAFETY: EDI owns this live vector for the synchronous load. We read
    // only the vector bounds, never an element chosen by the packet.
    unsafe { raw::site::deserialize_anomaly_state::anomaly(ctx).is_some_and(|anomaly|
        contains(anomaly.states_begin() as usize, anomaly.states_end() as usize,
                 ctx.reg(Reg::Ecx) & 0xff)) }
}

pub fn artefact_has_index(ctx: &Context<'_>) -> bool {
    // SAFETY: ECX is the inventory's live core at this site. Its registry
    // cannot change during the synchronous load; EAX is the zero-extended id.
    unsafe { raw::site::deserialize_artefact::core(ctx).is_some_and(|core|
        contains(core.artefacts_begin().map_or(0, |v| v.address() as usize),
                 core.artefacts_end().map_or(0, |v| v.address() as usize), ctx.reg(Reg::Eax))) }
}

fn contains(begin: usize, end: usize, index: u32) -> bool {
    begin != 0 && end.checked_sub(begin)
        .is_some_and(|bytes| bytes.is_multiple_of(4) && (index as usize) < bytes / 4)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_complete_in_range_slots_can_be_read() {
        assert!(contains(100, 112, 0));
        assert!(contains(100, 112, 2));
        for (begin, end, index) in [(100,112,3), (0,12,0), (100,100,0),
            (100,99,0), (100,111,0), (100,112,u32::MAX)] {
            assert!(!contains(begin, end, index));
        }
    }
}

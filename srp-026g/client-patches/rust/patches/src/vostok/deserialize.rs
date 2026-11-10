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

/// At `deserialize.death_animation`: a dead state's `m_animation_index` past
/// its set's death lists becomes 0, their first entry, before
/// get_death_animations indexes both lists with it.
pub fn keep_death_animation_in_set(ctx: &Context<'_>) {
    // SAFETY: the context is this site's: get_death_animations' entry frame,
    // on the game's thread, its first argument the selector's live set (the
    // native reads both lists through it next). Only the set's two counts and
    // the index argument are touched, never an entry the world chose.
    unsafe {
        let Some(frame) = raw::site::deserialize_death_animation::frame(ctx) else { return };
        let Some(set) = frame.set() else { return };
        let count = read(set.death_count()).min(read(set.death_hud_count()));
        let index = frame.index();
        if let Some(kept) = kept_death_animation(read(index), count) {
            index.write_volatile(kept);
        }
    }
}

/// The index to read the death lists with instead of `index`: `None` keeps
/// it (in the lists, or a set with none, which the game reads as it is).
fn kept_death_animation(index: u32, count: u32) -> Option<u32> {
    (count > 0 && index >= count).then_some(0)
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

    #[test]
    fn a_death_animation_past_the_set_reads_its_first() {
        assert_eq!(kept_death_animation(0, 1), None);
        assert_eq!(kept_death_animation(1, 2), None);
        for (index, count) in [(1, 1), (2, 2), (255, 2), (u32::MAX, 4)] {
            assert_eq!(kept_death_animation(index, count), Some(0));
        }
        assert_eq!(kept_death_animation(3, 0), None);
    }
}

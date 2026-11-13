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
/// it. Past the lists it is 0, the index the game's own pick gives a set of
/// one, so a world never reads further than the game does (a list of none,
/// which no config has, is read at 0 as the game would).
fn kept_death_animation(index: u32, count: u32) -> Option<u32> {
    (index != 0 && index >= count).then_some(0)
}

/// A double-barrel's or a pistol's fire state holds exactly two fire
/// animations, in the object at +164h (both constructors, 0x30F660 and
/// 0x30FF40, make an array of 2 there): a layout, not content.
const FIRE_ANIMATIONS: u8 = 2;

/// At `deserialize.fire_animation`: CL is the fire state's
/// `m_weapon_animation_index` as the world brought it, about to be stored.
/// A byte past the two fire animations is stored as 0, the first.
pub fn keep_fire_animation_in_state(ctx: &mut Context<'_>) {
    let index = (ctx.reg(Reg::Ecx) & 0xff) as u8;
    if let Some(kept) = kept_fire_animation(index) {
        ctx.set_reg8(Reg::Ecx, kept);
    }
}

/// The index to store instead of `index`: `None` keeps it.
fn kept_fire_animation(index: u8) -> Option<u8> {
    (index >= FIRE_ANIMATIONS).then_some(0)
}

/// What a throw grenade state's `m_grenade_index` names in its set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grenade {
    /// 0FFh: no grenade pulled.
    None,
    /// One of the set's grenades.
    InSet,
    /// Past the set's grenades, or a state with no set.
    Outside,
}

/// At `deserialize.grenade_ready`: the grenade the throw state in ESI names.
pub fn grenade_when_ready(ctx: &Context<'_>) -> Grenade {
    // SAFETY: the context is this site's, ESI its live throw grenade state.
    unsafe { grenade_of(raw::site::deserialize_grenade_ready::state(ctx)) }
}

/// At `deserialize.grenade_throw`: the grenade the throw state in ESI names.
pub fn grenade_to_throw(ctx: &Context<'_>) -> Grenade {
    // SAFETY: the context is this site's, ESI its live throw grenade state.
    unsafe { grenade_of(raw::site::deserialize_grenade_throw::state(ctx)) }
}

/// # Safety
/// `state` is a live throw grenade state on the game's thread, whose set
/// (if any) the native reads next. Only the index and the set's vector
/// bounds are read, never a grenade.
unsafe fn grenade_of(state: Option<raw::ThrowGrenadeState>) -> Grenade {
    let Some(state) = state else { return Grenade::Outside };
    // SAFETY: as the caller promises.
    unsafe {
        let set = state.grenade_set()
            .map(|set| (read(set.grenades_begin()) as usize, read(set.grenades_end()) as usize));
        grenade(read(state.grenade_index()), set)
    }
}

/// `index` against a set's grenades (its vector's begin and end), if any.
fn grenade(index: u8, set: Option<(usize, usize)>) -> Grenade {
    match (index, set) {
        (0xff, _) => Grenade::None,
        (index, Some((begin, end))) if contains(begin, end, index.into()) => Grenade::InSet,
        _ => Grenade::Outside,
    }
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
    fn a_fire_animation_past_the_two_is_the_first() {
        assert_eq!(kept_fire_animation(0), None);
        assert_eq!(kept_fire_animation(1), None);
        for index in [2, 100, 254, 255] {
            assert_eq!(kept_fire_animation(index), Some(0));
        }
    }

    #[test]
    fn a_grenade_is_one_of_the_set() {
        let set = Some((100, 116));
        assert_eq!(grenade(0, set), Grenade::InSet);
        assert_eq!(grenade(3, set), Grenade::InSet);
        assert_eq!(grenade(0xff, set), Grenade::None);
        assert_eq!(grenade(0xff, None), Grenade::None);
        for (index, set) in [(4, set), (254, set), (0, None), (0, Some((100, 100))), (0, Some((0, 4)))] {
            assert_eq!(grenade(index, set), Grenade::Outside);
        }
    }

    #[test]
    fn a_death_animation_past_the_set_reads_its_first() {
        assert_eq!(kept_death_animation(0, 1), None);
        assert_eq!(kept_death_animation(1, 2), None);
        for (index, count) in [(1, 1), (2, 2), (255, 2), (u32::MAX, 4)] {
            assert_eq!(kept_death_animation(index, count), Some(0));
        }
        // A list of none: no index is in it; the world reads no further than
        // the game's own 0.
        assert_eq!(kept_death_animation(3, 0), Some(0));
        assert_eq!(kept_death_animation(0, 0), None);
    }
}

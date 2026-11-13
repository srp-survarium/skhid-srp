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

/// At `deserialize.fire_animation` and `deserialize.fire_animation_sound`
/// (the live one): CL is the fire state's
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
    /// Past the set's grenades, or a state with no grenade set.
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
        let set = grenade_set(state)
            .map(|set| (read(set.grenades_begin()) as usize, read(set.grenades_end()) as usize));
        grenade(read(state.grenade_index()), set)
    }
}

/// The throw state's set when it is a grenade set. The set is any item of a
/// profile slot the world named (deserialize 0x33A5AB), so its vtable word
/// is read first, and nothing else of it unless it is a grenade set's.
///
/// # Safety
/// `state` is a live throw grenade state on the game's thread.
unsafe fn grenade_set(state: raw::ThrowGrenadeState) -> Option<raw::GrenadeSetCore> {
    // SAFETY: as the caller promises; the set (any item) is alive while the
    // state holds its reference.
    unsafe { state.grenade_set().filter(|set| is_grenade_set(read(set.vtable()))) }
}

/// Whether the inventory's `slot` (its grenade slot, the world's word) holds
/// a grenade set.
///
/// # Safety
/// `inventory` is a live inventory on the game's thread.
unsafe fn slot_holds_grenade_set(inventory: Option<raw::Inventory>, slot: u32) -> bool {
    let Some(inventory) = inventory else { return false };
    // SAFETY: as the caller promises; one of its 23 slots (+10Ch + 4 x slot),
    // and only the vtable word of the item there.
    slot < u32::from(super::inventory::SLOTS) && unsafe {
        inventory.slots().at(slot as usize).item().is_some_and(|item| is_grenade_set(read(item.vtable())))
    }
}

/// At `deserialize.grenade_can_throw`: whether the inventory in EAX has a
/// grenade set in its grenade slot (ECX).
pub fn can_throw_selected(ctx: &Context<'_>) -> bool {
    // SAFETY: the context is this site's, EAX the user's live inventory.
    unsafe { slot_holds_grenade_set(raw::site::deserialize_grenade_can_throw::inventory(ctx), ctx.reg(Reg::Ecx)) }
}

/// At `deserialize.grenade_attach`: whether the inventory in EAX has a
/// grenade set in its grenade slot (ECX) for the throw to take.
pub fn attaches_selected(ctx: &Context<'_>) -> bool {
    // SAFETY: the context is this site's, EAX the user's live inventory.
    unsafe { slot_holds_grenade_set(raw::site::deserialize_grenade_attach::inventory(ctx), ctx.reg(Reg::Ecx)) }
}

/// What a state with no grenade set saves and compares as its set's slot:
/// none, the inventory's own word for no grenade set (17h).
const NO_GRENADE_SLOT: u32 = 0x17;

/// At `deserialize.grenade_pull`: the index the pull's `pull_pin` will take
/// a grenade from, `None` when it takes none of the set's (no grenade set,
/// or past its grenades). pull_pin takes `amount - 1`'s low byte from a
/// stacked set, 1 from another.
pub fn grenade_to_pull(ctx: &Context<'_>) -> Option<u8> {
    // SAFETY: the context is this site's, ESI the pull's live throw state;
    // a grenade set's own config, amount and vector bounds are read.
    unsafe {
        let set = grenade_set(raw::site::deserialize_grenade_pull::state(ctx)?)?;
        let stacked = set.config().is_some_and(|config| read(config.stacked()) != 0);
        let index = pulled(stacked, read(set.amount()));
        contains(read(set.grenades_begin()) as usize, read(set.grenades_end()) as usize, index.into())
            .then_some(index)
    }
}

/// The grenade pull_pin takes: a stacked set's new amount's low byte, 1 for
/// another (0x30A12E..0x30A1B9).
fn pulled(stacked: bool, amount: u16) -> u8 {
    if stacked { amount.wrapping_sub(1) as u8 } else { 1 }
}

/// At `deserialize.grenade_idle`: `None` when the throw state's set is a
/// grenade set (the native compare runs); else the result as for a set of
/// no slot: whether the inventory's grenade slot differs from none.
pub fn idle_without_set(ctx: &Context<'_>) -> Option<bool> {
    // SAFETY: the context is this site's: EAX the live throw state, ECX the
    // user's inventory (its grenade slot word read).
    unsafe {
        let state = raw::site::deserialize_grenade_idle::state(ctx)?;
        if grenade_set(state).is_some() { return None; }
        let slot = raw::site::deserialize_grenade_idle::inventory(ctx)?.grenade_slot();
        Some(read(slot) != NO_GRENADE_SLOT)
    }
}

/// At `deserialize.grenade_choice`: `None` when the throw state's set is a
/// grenade set; else 0x33A730's result for a set of no slot: whether
/// (the inventory's grenade slot is none) equals its argument.
pub fn choice_without_set(ctx: &Context<'_>) -> Option<bool> {
    // SAFETY: the context is this site's: ECX the live throw state, EAX the
    // user's inventory, the frame its call's (the argument at +4).
    unsafe {
        let state = raw::site::deserialize_grenade_choice::state(ctx)?;
        if grenade_set(state).is_some() { return None; }
        let slot = read(raw::site::deserialize_grenade_choice::inventory(ctx)?.grenade_slot());
        let wanted = read(raw::site::deserialize_grenade_choice::frame(ctx)?.wanted());
        Some(wanted == u8::from(slot == NO_GRENADE_SLOT))
    }
}

/// At `deserialize.grenade_serialize`: `None` when the throw state's set is
/// a grenade set (the native reads its slot); else the set's address (as
/// the native leaves it in ECX) and the slot byte to save: none.
pub fn serialized_without_set(ctx: &Context<'_>) -> Option<(u32, u8)> {
    // SAFETY: the context is this site's, EBX the live throw state.
    unsafe {
        let state = raw::site::deserialize_grenade_serialize::state(ctx)?;
        if grenade_set(state).is_some() { return None; }
        Some((state.grenade_set().map_or(0, |set| set.address()), NO_GRENADE_SLOT as u8))
    }
}

/// Whether an item's vtable is a grenade set's (the client's grenade_set,
/// or grenade_set_core): the only items with a grenade vector at +268h.
fn is_grenade_set(vtable: u32) -> bool {
    [raw::image::GRENADE_SET_VTABLE, raw::image::GRENADE_SET_CORE_VTABLE].contains(&vtable)
}

/// The animation handles an animation-end-aware weapon state holds in the
/// object from `[state+130h]` (each class's register_animations walks
/// exactly these, contiguous; every constructor points +130h at the first),
/// by that register_animations (its vtable's +1Ch, which every derived
/// class keeps). `None`: not a state with such a table.
fn wait_table_len(register_animations: u32) -> Option<u8> {
    use raw::image::*;
    match register_animations {
        REGISTER_THROW_SUBSTATE_ANIMATIONS | REGISTER_MELEE_ANIMATIONS => Some(4),
        REGISTER_SHOW_HIDE_ANIMATIONS | REGISTER_RELOAD_ANIMATIONS
            | REGISTER_SHOTGUN_SUBSTATE_ANIMATIONS => Some(5),
        REGISTER_FIRE_ANIMATIONS => Some(9),
        REGISTER_FIRE_TWICE_ANIMATIONS => Some(10),
        _ => None,
    }
}

/// Whether `index` names one of the state's own animation handles. Past
/// them the native compare reads what lies after the object: never a
/// handle the state chose, and different in every game. Those indices
/// (and a state of no known layout) are taken as no match.
fn waits_in_table(index: u8, register_animations: u32) -> bool {
    wait_table_len(register_animations).is_some_and(|len| index < len)
}

/// # Safety
/// `state` is the live animation-end-aware state whose callback runs, on
/// the game's thread. Only its vtable's register_animations and its index
/// are read.
unsafe fn wait_of(state: Option<raw::AnimationEndAwareState>) -> bool {
    // SAFETY: as the caller promises; a state's vtable is the image's.
    unsafe {
        state.is_some_and(|state| state.methods().is_some_and(|methods|
            waits_in_table(read(state.index_of_animation_to_wait()), read(methods.register_animations()))))
    }
}

/// At `deserialize.wait_end`: whether the state in ECX waits for one of its
/// own animation handles.
pub fn waits_at_end(ctx: &Context<'_>) -> bool {
    // SAFETY: the context is this site's, ECX the callback's live state.
    unsafe { wait_of(raw::site::deserialize_wait_end::state(ctx)) }
}

/// At `deserialize.wait_throw`: as [`waits_at_end`], the throw substate's.
pub fn waits_at_throw(ctx: &Context<'_>) -> bool {
    // SAFETY: the context is this site's, ECX the callback's live substate.
    unsafe { wait_of(raw::site::deserialize_wait_throw::state(ctx)) }
}

/// At `deserialize.wait_melee`: as [`waits_at_end`], the melee state's.
pub fn waits_at_melee(ctx: &Context<'_>) -> bool {
    // SAFETY: the context is this site's, ECX the callback's live state.
    unsafe { wait_of(raw::site::deserialize_wait_melee::state(ctx)) }
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
    fn a_pull_takes_the_new_amount_or_one() {
        assert_eq!(pulled(true, 3), 2);
        assert_eq!(pulled(true, 1), 0);
        assert_eq!(pulled(true, 0), 0xff); // an empty stack: past any set
        assert_eq!(pulled(true, 0x101), 0); // the low byte, as the native
        assert_eq!(pulled(false, 0), 1);
    }

    #[test]
    fn only_a_grenade_set_has_grenades() {
        assert!(is_grenade_set(0xA0B56C));
        assert!(is_grenade_set(0xA14824));
        assert!(!is_grenade_set(0xA04174)); // a weapon
        assert!(!is_grenade_set(0));
    }

    #[test]
    fn an_animation_to_wait_is_one_of_the_state_s_handles() {
        // The game's own: show 0, 2; fire 8; double-barrel fire 9.
        assert!(waits_in_table(2, 0x24B700));
        assert!(waits_in_table(8, 0x24B7C0));
        assert!(waits_in_table(9, 0x24B820));
        assert!(waits_in_table(3, 0x24B920));
        // Past them: the show state's uninitialised 63, 0FFh, one past.
        for (index, register) in [(63, 0x24B700), (0xff, 0x24B700), (5, 0x24B700), (4, 0x24B880),
                                  (9, 0x24B7C0), (10, 0x24B820), (4, 0x24B920)] {
            assert!(!waits_in_table(index, register), "{index} {register:#x}");
        }
        assert!(!waits_in_table(0, 0x30D5F0)); // an idle state: no table
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

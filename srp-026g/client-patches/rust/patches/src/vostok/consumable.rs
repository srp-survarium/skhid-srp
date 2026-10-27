//! Read-only presentation of the native medkit state machine. No patch clock
//! or retained item pointers: each HUD update reads the current simulation.
use core::ffi::c_void;
use super::{Context, Reg};
use super::ui::{Movie, movie_of};
use crate::raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Milliseconds(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Activity {
    Inactive,
    Waiting { remaining: Milliseconds, duration: Milliseconds },
    Active { remaining: Milliseconds, duration: Milliseconds },
}

impl Activity {
    fn parse(active: bool, delay: u32, remaining: u32, duration: u32) -> Self {
        if !active || remaining == 0 { Self::Inactive }
        else if delay != 0 { Self::Waiting { remaining: Milliseconds(delay), duration: Milliseconds(duration.max(delay)) } }
        else { Self::Active { remaining: Milliseconds(remaining), duration: Milliseconds(duration.max(remaining)) } }
    }
    fn arguments(self) -> [u32; 3] {
        match self {
            Self::Inactive => [0, 0, 0],
            Self::Waiting { remaining, duration } => [1, remaining.0, duration.0],
            Self::Active { remaining, duration } => [2, remaining.0, duration.0],
        }
    }
}

/// The movie and a value snapshot, both obtained together at the native slot
/// update. Publishing cannot change inventory, item ownership or game timers.
pub struct QuickSlotActivity<'t> {
    movie: Movie<'t>,
    slot: u32,
    activity: Activity,
}

impl<'t> QuickSlotActivity<'t> {
    pub fn at_update(ctx: &'t Context<'_>) -> Option<Self> {
        let ui = ctx.reg(Reg::Esi) as usize as *mut c_void;
        let player = ctx.reg(Reg::Ebp) as usize as *mut c_void;
        let inventory = ctx.reg(Reg::Ebx) as usize as *mut c_void;
        let slot = ctx.reg(Reg::Edi);
        let index = ctx.reg(Reg::Eax) as usize;
        // SAFETY: these are the live objects held by update_local_player_state.
        // Medicines occupy the inventory's fixed slots. Artefact mappings
        // instead encode an offset into a separately allocated container array;
        // do not turn those offsets into an in-bounds inventory pointer.
        let (movie, activity) = unsafe {
            let movie = movie_of(*raw::game_world_ui::hud_movie(ui))?;
            let slots = raw::inventory::slots(inventory);
            let end = raw::inventory::slots_end(inventory);
            let count = (end as usize - slots as usize) / core::mem::size_of::<u32>();
            let item = if index < count {
                *slots.add(index) as usize as *mut c_void
            } else { core::ptr::null_mut() };
            let activity = if *raw::player::alive(player) == 0 || item.is_null()
                || *raw::inventory_item::vtable(item) != raw::image::MEDKIT_VTABLE
            { Activity::Inactive } else {
                let active = *raw::medkit::active(item) != 0;
                let delay = *raw::medkit::delay_remaining(item);
                let remaining = *raw::medkit::remaining(item);
                // The same profile, current kit and modifier as native tick.
                let profile = *raw::player::profile(player) as usize as *mut c_void;
                let kit = usize::from(*raw::player::kit(player));
                let mut kit_base = profile;
                for _ in 0..kit { kit_base = raw::profile_kit::end(kit_base).cast(); }
                let modifiers = raw::player_profile::kit_modifiers(kit_base).cast();
                let duration = if active && remaining != 0 {
                    if delay != 0 {
                        // Native set_active uses drug_use_time (33), weight 1.
                        raw::engine_apply_modifier(modifiers, 33, *raw::medkit::use_delay(item) as f32,
                                                   1.0) as u32
                    } else {
                        raw::engine_apply_modifier(modifiers, 34, *raw::medkit::duration(item) as f32,
                                                   *raw::medkit::skill_weight(item)) as u32
                    }
                } else { 0 };
                Activity::parse(active, delay, remaining, duration)
            };
            (movie, activity)
        };
        Some(Self { movie, slot, activity })
    }
    pub fn publish(self) {
        let [phase, remaining, duration] = self.activity.arguments();
        self.movie.invoke_uints(c"root.set_quick_slot_activity", &[self.slot, phase, remaining, duration]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_delay_effect_and_teardown() {
        assert_eq!(Activity::parse(false, 1000, 5000, 5000), Activity::Inactive);
        assert_eq!(Activity::parse(true, 1, 5000, 1000).arguments(), [1, 1, 1000]);
        assert_eq!(Activity::parse(true, 1000, 5000, 1000).arguments(), [1, 1000, 1000]);
        assert_eq!(Activity::parse(true, 0, 4999, 5000).arguments(), [2, 4999, 5000]);
        assert_eq!(Activity::parse(true, 0, 1, 5000).arguments(), [2, 1, 5000]);
        assert_eq!(Activity::parse(true, 0, 0, 5000), Activity::Inactive);
        assert_eq!(Activity::parse(false, 0, 4000, 5000), Activity::Inactive);
    }
}

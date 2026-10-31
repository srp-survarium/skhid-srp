//! Read-only presentation of the native medkit state machine. No patch clock
//! or retained item pointers: each HUD update reads the current simulation.
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
        use raw::site::quick_slots_activity as site;
        let slot = ctx.reg(Reg::Edi);
        let index = ctx.reg(Reg::Eax) as usize;
        // SAFETY: the context is this site's: the live objects
        // update_local_player_state holds, on the game's thread. Medicines
        // occupy the inventory's fixed slots. Artefact mappings instead encode
        // an offset into a separately allocated container array; an index past
        // the slots is no slot.
        let (movie, activity) = unsafe {
            let (ui, player, inventory) = (site::ui(ctx)?, site::player(ctx)?, site::inventory(ctx)?);
            let movie = movie_of(ui.hud_movie())?;
            let slots = inventory.slots();
            let count = inventory.slots_end().index_from(slots).unwrap_or(0);
            let item = if index < count { slots.at(index).item() } else { None };
            let activity = match item {
                Some(item) if *player.alive() != 0 && *item.vtable() == raw::image::MEDKIT_VTABLE => {
                    // the vtable says it is a medkit (all medical consumables)
                    let medkit = item.medkit();
                    let active = *medkit.active() != 0;
                    let delay = *medkit.delay_remaining();
                    let remaining = *medkit.remaining();
                    // The same profile, current kit and modifier as native tick.
                    let kit = usize::from(*player.kit());
                    let modifiers = player.profile().map(|profile| profile.kits().at(kit).modifiers());
                    let duration = if active && remaining != 0 {
                        if delay != 0 {
                            // Native set_active uses drug_use_time (33), weight 1.
                            raw::engine_apply_modifier(modifiers, 33, *medkit.use_delay() as f32, 1.0) as u32
                        } else {
                            raw::engine_apply_modifier(modifiers, 34, *medkit.duration() as f32,
                                                       *medkit.skill_weight()) as u32
                        }
                    } else { 0 };
                    Activity::parse(active, delay, remaining, duration)
                }
                _ => Activity::Inactive,
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

//! Read-only presentation of the native medkit state machine. No item
//! pointers retained: each HUD update reads the current simulation, and what
//! a patch remembers between updates (patch 84's red second) is a drug's
//! identity and the local player's simulation clock, never a game object.
use super::context::{Context, Reg};
use super::ui::{Movie, movie_of};
use crate::raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Milliseconds(u32);

/// A drug in use, as its own timers say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Course {
    Waiting { remaining: Milliseconds, duration: Milliseconds },
    Active { remaining: Milliseconds, duration: Milliseconds },
}

/// Which drug a slot holds: its address, compared, never read through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrugId(u32);

/// The local player's simulation milliseconds (player +324h, set at each of
/// its ticks, 0x2DAA7F): the clock of the HUD's update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameTime(u32);

impl GameTime {
    /// Milliseconds from `earlier` to `self`; 0 when the clock went back
    /// (a rollback's resimulation has not caught up yet).
    pub fn since(self, earlier: GameTime) -> u32 {
        let elapsed = self.0.wrapping_sub(earlier.0);
        if elapsed > u32::MAX / 2 { 0 } else { elapsed }
    }
}

/// What a quick slot's drug is now, read from the simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observed {
    /// No drug, a dead holder, a drug never used or one whose time ran out
    /// (medkit::tick ends it with both timers spent, 0x33DA3C).
    Idle,
    InUse { drug: DrugId, course: Course },
    /// Ended with time left: set_active(0) leaves +2ACh and +2A8h as they
    /// were. With the holder alive only patch 84's stop does that (the
    /// other caller, 0x33D7C1, is the dead holder's); a previous life's drug
    /// can look so too, which is why the patch asks for the use before it.
    Stopped { drug: DrugId },
}

impl Observed {
    fn parse(drug: DrugId, active: bool, delay: u32, remaining: u32, duration: u32) -> Self {
        match (active, delay, remaining) {
            (true, _, 0) | (false, 0, 0) => Self::Idle,
            (false, _, _) => Self::Stopped { drug },
            (true, 0, _) => Self::InUse { drug, course: Course::Active {
                remaining: Milliseconds(remaining), duration: Milliseconds(duration.max(remaining)) } },
            (true, _, _) => Self::InUse { drug, course: Course::Waiting {
                remaining: Milliseconds(delay), duration: Milliseconds(duration.max(delay)) } },
        }
    }
}

/// What the HUD shows for a slot (`root.set_quick_slot_activity`'s phase).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shown {
    Inactive,
    InUse(Course),
    /// A bullet stopped it (patch 84): the slot's icon red.
    Interrupted,
}

impl Shown {
    fn arguments(self) -> [u32; 3] {
        match self {
            Self::Inactive => [0, 0, 0],
            Self::InUse(Course::Waiting { remaining, duration }) => [1, remaining.0, duration.0],
            Self::InUse(Course::Active { remaining, duration }) => [2, remaining.0, duration.0],
            Self::Interrupted => [3, 0, 0],
        }
    }
}

/// The movie and a value snapshot, both obtained together at the native slot
/// update. Publishing cannot change inventory, item ownership or game timers.
pub struct QuickSlotActivity<'t> {
    movie: Movie<'t>,
    slot: u32,
    observed: Observed,
    now: GameTime,
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
        let (movie, observed, now) = unsafe {
            let (ui, player, inventory) = (site::ui(ctx)?, site::player(ctx)?, site::inventory(ctx)?);
            let movie = movie_of(ui.hud_movie())?;
            let slots = inventory.slots();
            let count = inventory.slots_end().index_from(slots).unwrap_or(0);
            let item = if index < count { slots.at(index).item() } else { None };
            let observed = match item {
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
                    Observed::parse(DrugId(medkit.address()), active, delay, remaining, duration)
                }
                _ => Observed::Idle,
            };
            (movie, observed, GameTime(*player.time()))
        };
        Some(Self { movie, slot, observed, now })
    }

    /// The quick slot, 0..5 (update_local_player_state's loop, 0x23B7AE).
    pub fn slot(&self) -> usize {
        self.slot as usize
    }

    pub fn observed(&self) -> Observed {
        self.observed
    }

    pub fn now(&self) -> GameTime {
        self.now
    }

    pub fn publish(self, shown: Shown) {
        let [phase, remaining, duration] = shown.arguments();
        self.movie.invoke_uints(c"root.set_quick_slot_activity", &[self.slot, phase, remaining, duration]);
    }
}

/// Made-up values for the patches' unit tests (no game object behind them).
#[cfg(test)]
impl DrugId {
    pub const fn new(address: u32) -> DrugId { DrugId(address) }
}
#[cfg(test)]
impl GameTime {
    pub const fn at(ms: u32) -> GameTime { GameTime(ms) }
}
#[cfg(test)]
impl Course {
    pub const fn waiting(remaining: u32, duration: u32) -> Course {
        Course::Waiting { remaining: Milliseconds(remaining), duration: Milliseconds(duration) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const D: DrugId = DrugId(0x1000);
    fn shown(active: bool, delay: u32, remaining: u32, duration: u32) -> [u32; 3] {
        match Observed::parse(D, active, delay, remaining, duration) {
            Observed::InUse { course, .. } => Shown::InUse(course).arguments(),
            _ => Shown::Inactive.arguments(),
        }
    }
    #[test]
    fn native_delay_effect_and_teardown() {
        assert_eq!(shown(false, 1000, 5000, 5000), [0, 0, 0]);
        assert_eq!(shown(true, 1, 5000, 1000), [1, 1, 1000]);
        assert_eq!(shown(true, 1000, 5000, 1000), [1, 1000, 1000]);
        assert_eq!(shown(true, 0, 4999, 5000), [2, 4999, 5000]);
        assert_eq!(shown(true, 0, 1, 5000), [2, 1, 5000]);
        assert_eq!(shown(true, 0, 0, 5000), [0, 0, 0]);
        assert_eq!(shown(false, 0, 4000, 5000), [0, 0, 0]);
        assert_eq!(Shown::Interrupted.arguments(), [3, 0, 0]);
    }
    #[test]
    fn set_active_0_with_time_left_is_stopped() {
        assert_eq!(Observed::parse(D, false, 900, 3000, 0), Observed::Stopped { drug: D });
        assert_eq!(Observed::parse(D, false, 0, 1200, 0), Observed::Stopped { drug: D });
        assert_eq!(Observed::parse(D, false, 0, 0, 0), Observed::Idle);
        assert_eq!(Observed::parse(D, true, 0, 0, 0), Observed::Idle);
    }
    #[test]
    fn the_clock_never_runs_back() {
        assert_eq!(GameTime(5000).since(GameTime(4000)), 1000);
        assert_eq!(GameTime(3).since(GameTime(u32::MAX)), 4);
        assert_eq!(GameTime(4000).since(GameTime(5000)), 0);
    }
}

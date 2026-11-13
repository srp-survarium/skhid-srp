//! Bounds at the native reads that the snapshot format cannot frame.
//! Valid indices execute the original instructions; invalid ones take the
//! loader's own absent-state/object path, or (a death animation, a fire
//! animation) the first entry; a grenade the set does not hold (or a set that
//! is not a grenade set) is not ready and is not thrown; an animation to wait
//! for past the state's own handles never ends.
#![forbid(unsafe_code)]
use crate::vostok::{Context, Flow, Reg, NoExit, DeserializeGuardsOnFsmExit, DeserializeGuardsOnAnomalyExit,
    DeserializeGuardsOnGrenadeReadyExit, DeserializeGuardsOnGrenadeThrowExit,
    DeserializeGuardsOnWaitEndExit, DeserializeGuardsOnWaitThrowExit, DeserializeGuardsOnWaitMeleeExit};
use crate::vostok::deserialize::{self, Grenade};

pub fn on_fsm(ctx: &mut Context<'_>) -> Flow<DeserializeGuardsOnFsmExit> {
    if deserialize::fsm_has_state(ctx) { Flow::Continue }
    else { Flow::Exit(DeserializeGuardsOnFsmExit::Done) }
}

pub fn on_anomaly(ctx: &mut Context<'_>) -> Flow<DeserializeGuardsOnAnomalyExit> {
    if deserialize::anomaly_has_index(ctx) { Flow::Continue }
    else { Flow::Exit(DeserializeGuardsOnAnomalyExit::NoState) }
}

pub fn on_artefact(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if deserialize::artefact_has_index(ctx) { Flow::Continue }
    else {
        ctx.set_reg(Reg::Eax, 0);
        Flow::Skip
    }
}

/// A dead selector's `m_animation_index` comes from the world as any byte;
/// the game's pick keeps it below the set's death animations. One past them
/// becomes the first, so the lists are never indexed outside their entries.
pub fn on_death_animation(ctx: &mut Context<'_>) -> Flow<NoExit> {
    deserialize::keep_death_animation_in_set(ctx);
    Flow::Continue
}

/// A double-barrel's or pistol's fire state's `m_weapon_animation_index`
/// comes from the world as any byte; the state holds two fire animations.
/// A byte past them is stored as the first, so the presentation never takes
/// a reference through what lies after them.
pub fn on_fire_animation(ctx: &mut Context<'_>) -> Flow<NoExit> {
    deserialize::keep_fire_animation_in_state(ctx);
    Flow::Continue
}

/// A throw's `m_grenade_index` past its set's grenades, or a set that is not
/// a grenade set, is not ready, as with none (the native 0FFh path).
pub fn on_grenade_ready(ctx: &mut Context<'_>) -> Flow<DeserializeGuardsOnGrenadeReadyExit> {
    match deserialize::grenade_when_ready(ctx) {
        Grenade::None | Grenade::InSet => Flow::Continue,
        Grenade::Outside => Flow::Exit(DeserializeGuardsOnGrenadeReadyExit::NotReady),
    }
}

/// A throw throws only one of its grenade set's grenades: with none, past
/// them or with a set that is not a grenade set, nothing is thrown.
pub fn on_grenade_throw(ctx: &mut Context<'_>) -> Flow<DeserializeGuardsOnGrenadeThrowExit> {
    match deserialize::grenade_to_throw(ctx) {
        Grenade::InSet => Flow::Continue,
        Grenade::None | Grenade::Outside => Flow::Exit(DeserializeGuardsOnGrenadeThrowExit::Done),
    }
}

/// An animation-end-aware state's `m_index_of_animation_to_wait` past its
/// own animation handles (the world's byte, or the show state's never set
/// one) compares as no match, as it practically always does: the native
/// compare would read what lies after the object.
pub fn on_wait_end(ctx: &mut Context<'_>) -> Flow<DeserializeGuardsOnWaitEndExit> {
    if deserialize::waits_at_end(ctx) { Flow::Continue }
    else { Flow::Exit(DeserializeGuardsOnWaitEndExit::NoMatch) }
}

/// As [`on_wait_end`], at the throw substate's callback.
pub fn on_wait_throw(ctx: &mut Context<'_>) -> Flow<DeserializeGuardsOnWaitThrowExit> {
    if deserialize::waits_at_throw(ctx) { Flow::Continue }
    else { Flow::Exit(DeserializeGuardsOnWaitThrowExit::NoMatch) }
}

/// As [`on_wait_end`], at the melee state's callback (its entry: no match
/// returns 0, the failed compare's result).
pub fn on_wait_melee(ctx: &mut Context<'_>) -> Flow<DeserializeGuardsOnWaitMeleeExit> {
    if deserialize::waits_at_melee(ctx) { Flow::Continue }
    else {
        ctx.set_reg(Reg::Eax, 0);
        Flow::Exit(DeserializeGuardsOnWaitMeleeExit::NoMatch)
    }
}

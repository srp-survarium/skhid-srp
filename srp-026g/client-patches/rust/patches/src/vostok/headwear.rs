//! The content's extra render bone, independent of the animation skeleton.
//! Immutable bind data identifies the extended layout. Mutable skin matrices
//! are touched only on the render thread at update_render_matrices' own hook.

use super::world::{Player, floats, read, write_floats};
use super::context::{Context, Reg};
use crate::raw;
use ragdoll::corpse::DeathKey;

/// The engine's classification of the killing hit, not a recent impact.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FatalHit {
    Headshot,
    Other,
}

/// generate_killed_event after enqueue (`player.killed_event`): ESI is the
/// base player and BL is still the cause icon (1 for a headshot, 0/3
/// otherwise). The queue call preserves EBX and ESI; no render-player-only
/// fields are read here.
pub fn killed(ctx: &Context<'_>) -> Option<(u8, DeathKey, FatalHit)> {
    // SAFETY: the context is this site's: the victim, alive for the event, on
    // its thread.
    let player = Player::of(ctx, unsafe { raw::site::player_killed_event::victim(ctx) })?;
    let hit = if ctx.reg(Reg::Ebx) as u8 == 1 { FatalHit::Headshot } else { FatalHit::Other };
    Some((player.place(), DeathKey(player.spawned()), hit))
}

pub const HEAD: usize = 44;
pub const BONE: usize = 69;
pub const COUNT: usize = 70;

/// Proof that this instance has the content's extra Head-equivalent bind.
#[derive(Clone, Copy)]
pub struct HeadwearRig;

fn rig(instance: raw::SkeletonInstance) -> Option<HeadwearRig> {
    // SAFETY: a live render instance from its owning player or renderer hook;
    // original and its bind vector are immutable for the instance's lifetime,
    // and HEAD and BONE are below the vector's count, checked first.
    unsafe {
        let model = instance.original()?;
        let (begin, end) = (model.inverse_binds()?, model.inverse_binds_end()?);
        if end.index_from(begin)? != COUNT {
            return None;
        }
        let head = floats::<16>(begin.at(HEAD).value());
        let extra = floats::<16>(begin.at(BONE).value());
        (head.iter().all(|x| x.is_finite()) && head == extra).then_some(HeadwearRig)
    }
}

pub fn of_player(player: &Player<'_>) -> Option<HeadwearRig> {
    let (_, holder) = player.renderer()?;
    // SAFETY: the player's live intrusive render-instance field.
    rig(unsafe { holder.instance() }?)
}

/// Commands from baking, the lobby, and the ordinary 69-bone draw inherit
/// Head's already-skinned matrix. A 70-bone command already supplied its own
/// accessory pose, so it is never overwritten here.
pub fn finish_update(ctx: &Context<'_>) {
    if ctx.reg(Reg::Edx) != BONE as u32 {
        return;
    }
    // SAFETY: the context is this site's (`render.skeleton_updated`): the
    // instance update_render_matrices updates, on the render thread.
    let Some(instance) = (unsafe { raw::site::render_skeleton_updated::instance(ctx) }) else { return };
    if rig(instance).is_none() {
        return;
    }
    // SAFETY: this hook runs inside the renderer's update of this instance.
    // All three arrays have the engine's 128-matrix inline capacity; the
    // original loop just wrote the first 69. No logic-thread mutation occurs.
    unsafe {
        let (Some(current), Some(shadow), Some(previous)) =
            (instance.matrices(), instance.shadow_matrices(), instance.previous_matrices())
        else {
            return;
        };
        let first = read(instance.first_update()) != 0;
        for (destination, source) in [
            (current, current), (shadow, shadow),
            (previous, if first { current } else { previous }),
        ] {
            let matrix = floats::<16>(source.at(HEAD).value());
            write_floats(destination.at(BONE).value(), matrix);
        }
    }
}

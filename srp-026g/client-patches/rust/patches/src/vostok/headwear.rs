//! The content's extra render bone, independent of the animation skeleton.
//! Immutable bind data identifies the extended layout. Mutable skin matrices
//! are touched only on the render thread at update_render_matrices' own hook.

use super::{Context, Reg};
use super::world::{Object, Player, floats, pointer, read};
use crate::raw;
use ragdoll::corpse::DeathKey;

/// The engine's classification of the killing hit, not a recent impact.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FatalHit {
    Headshot,
    Other,
}

/// generate_killed_event after enqueue: ESI is the base player and BL is
/// still the cause icon (1 for a headshot, 0/3 otherwise). The queue call
/// preserves EBX and ESI; no render-player-only fields are read here.
pub fn killed(ctx: &Context<'_>) -> Option<(u8, DeathKey, FatalHit)> {
    let player = Player::at(ctx, Reg::Esi)?;
    let hit = if ctx.reg(Reg::Ebx) as u8 == 1 { FatalHit::Headshot } else { FatalHit::Other };
    Some((player.place(), DeathKey(player.spawned()), hit))
}

pub const HEAD: usize = 44;
pub const BONE: usize = 69;
pub const COUNT: usize = 70;

/// Proof that this instance has the content's extra Head-equivalent bind.
#[derive(Clone, Copy)]
pub struct HeadwearRig;

fn rig(instance: Object) -> Option<HeadwearRig> {
    // SAFETY: a live render instance from its owning player or renderer hook;
    // original and its bind vector are immutable for the instance's lifetime.
    unsafe {
        let model = pointer(raw::skeleton_instance::original(instance.ptr()))?;
        let begin = read(raw::skeleton_render_model::inverse_binds(model.ptr()));
        let end = read(raw::skeleton_render_model::inverse_binds_end(model.ptr()));
        if begin == 0 || end.checked_sub(begin)? != COUNT as u32 * 64 {
            return None;
        }
        let binds = begin as usize as *mut [f32; 16];
        let head = floats::<16>(binds.add(HEAD).cast());
        let extra = floats::<16>(binds.add(BONE).cast());
        (head.iter().all(|x| x.is_finite()) && head == extra).then_some(HeadwearRig)
    }
}

pub fn of_player(player: &Player<'_>) -> Option<HeadwearRig> {
    let (_, instance_ref) = player.renderer()?;
    // SAFETY: renderer gives the player's live intrusive render-instance field.
    rig(unsafe { pointer(instance_ref.cast())? })
}

/// Commands from baking, the lobby, and the ordinary 69-bone draw inherit
/// Head's already-skinned matrix. A 70-bone command already supplied its own
/// accessory pose, so it is never overwritten here.
pub fn finish_update(ctx: &Context<'_>) {
    if ctx.reg(Reg::Edx) != BONE as u32 {
        return;
    }
    let Some(instance) = Object::at(ctx.reg(Reg::Edi)) else { return };
    if rig(instance).is_none() {
        return;
    }
    // SAFETY: this hook runs inside the renderer's update of this instance.
    // All three arrays have the engine's 128-matrix inline capacity; the
    // original loop just wrote the first 69. No logic-thread mutation occurs.
    unsafe {
        let current = pointer(raw::skeleton_instance::matrices(instance.ptr()));
        let shadow = pointer(raw::skeleton_instance::shadow_matrices(instance.ptr()));
        let previous = pointer(raw::skeleton_instance::previous_matrices(instance.ptr()));
        let (Some(current), Some(shadow), Some(previous)) = (current, shadow, previous) else { return };
        let first = read(raw::skeleton_instance::first_update(instance.ptr())) != 0;
        for (destination, source) in [
            (current, current), (shadow, shadow),
            (previous, if first { current } else { previous }),
        ] {
            let matrix = floats::<16>(source.ptr().cast::<[f32; 16]>().add(HEAD).cast());
            destination.ptr().cast::<[f32; 16]>().add(BONE).write(matrix);
        }
    }
}

//! Copy a dead player's weapon model box with the exact pose about to be
//! drawn. Do not read the render instance's mutable cached AABB: its renderer
//! command can run on another thread, behind the current ragdoll draw.

use super::{CoreId, shared_place};
use crate::raw;
use crate::vostok::types::{GameTime, Place};
use crate::vostok::world::{Object, floats, pointer, read};
use crate::vostok::{Context, Reg};
use weapon_pickup::state::{DropId, Life};
use weapon_pickup::targeting::{RenderedDrop, WeaponBox};

/// A copied presentation observation. No native pointer is retained or read
/// through later. `core` identifies the match owner, not a reusable handle.
/// Publishing it still requires that owner's current-frame lifecycle.
pub struct DrawnWeapon {
    pub core: CoreId,
    pub view: RenderedDrop,
}

/// Both frame sites supply EBX=game_world. Its match wrapper is absent in
/// the lobby. The core is embedded, not a pointer stored at wrapper+1F8.
pub(super) fn drawing_core(ctx: &Context<'_>) -> Option<CoreId> {
    let world = Object::at(ctx.reg(Reg::Ebx))?;
    // SAFETY: the frame sites run in this live game_world::tick; its wrapper
    // remains owned across the draw task join. Only its identity is copied.
    unsafe {
        let wrapper = pointer(raw::game_world::match_wrapper(world.ptr()))?;
        let core = raw::input_world_wrapper::core(wrapper.ptr()) as u32;
        Some(CoreId(core::num::NonZeroU32::new(core)?))
    }
}

impl DrawnWeapon {
    /// player::mt_draw at 2477C8, immediately before the held object's virtual
    /// draw: EBX player, EAX the chosen world transform. This is after patch76
    /// overwrites the Weapon bone and native code composes its world pose.
    pub fn at_held_draw(ctx: &Context<'_>) -> Option<Self> {
        let player = Object::at(ctx.reg(Reg::Ebx))?;
        let transform = Object::at(ctx.reg(Reg::Eax))?;
        // SAFETY: this particular mt_draw site has a live player and its
        // stack-owned 16-float transform. Native drawing retains the held
        // object and model wrapper for the task's duration. Only the concrete
        // client weapon has the model field read below; never cast another
        // held item or the smaller weapon_core to this type.
        unsafe {
            if read(raw::player::inserted(player.ptr())) == 0
                || read(raw::player::alive(player.ptr())) != 0
            {
                return None;
            }
            let core = core::num::NonZeroU32::new(read(raw::player::core(player.ptr())))?;
            let place = Place::new(u32::from(read(raw::player::place(player.ptr()))))?;
            let spawned = GameTime::from_raw(read(raw::player::spawned(player.ptr())))?;
            let weapon = pointer(raw::player::held_item(player.ptr()))?;
            if read(raw::weapon_core::vtable(weapon.ptr())) != raw::image::WEAPON_ACTIVE_VTABLE {
                return None;
            }
            let model = pointer(raw::weapon::model(weapon.ptr()))?;
            let instance = pointer(raw::weapon_model::render_instance(model.ptr()))?;
            let original = pointer(raw::skeleton_render_model_instance::original(
                instance.ptr(),
            ))?;
            let authored: [f32; 6] = floats(raw::skeleton_render_model::bounds(original.ptr()));
            // The resource box is in exporter coordinates. The renderer
            // rotates it around Y by pi (15A77F). Preserve min/max ordering
            // while converting x/z; do not use its generic fallback cube or
            // enlarge the weapon to include animation locator origins.
            let local = [
                -authored[3],
                authored[1],
                -authored[5],
                -authored[0],
                authored[4],
                -authored[2],
            ];
            let bounds = WeaponBox::parse(local, floats(transform.ptr().cast()))?;
            Some(Self {
                core: CoreId(core),
                view: RenderedDrop {
                    id: DropId(Life {
                        place: shared_place(place),
                        spawned: spawned.0,
                    }),
                    bounds,
                },
            })
        }
    }
}

#[cfg(test)]
mod tests;

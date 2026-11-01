//! Patch 90: a weapon's muzzle device (`docs/notes/weapon-kit.md`, "The
//! muzzle device"). A weapon config names its device in `data`:
//!
//! - `srp_muzzle_model` (a static model's path) and `srp_muzzle_fire_mm`
//!   (its fire point, mm along +z of its model): the device the weapon kit
//!   names (its slot 4; the device item's modification adds both);
//! - else `srp_muzzle_stock_model` and `srp_muzzle_stock_fire_mm`: the
//!   gun's own built-in device, which the content cut out of its mesh;
//! - neither (or a pair not whole): nothing changes for that weapon.
//!
//! The cook asks for the model with the weapon's sub-resources
//! (`on_request_room`, `on_request`) and keeps it (`on_loaded`); each draw
//! puts it at the weapon model's `muzzle_point` (calculate_locator's frame,
//! whose axes are the gun mesh's, as 0.69d0 authors its modules: drawn
//! unrotated) and the shot's flash at its fire point (`on_locate`), in the
//! scene while the weapon's model is (`on_draw`), out when the weapon is
//! culled or hidden (`on_culled`, `on_hidden`); it goes with the weapon
//! (`on_release`).
//!
//! What a match computes does not change here: the frame and the flash are
//! drawn only (+620h feeds the shot effects alone). A device's stats are its
//! modification's (content), a `simulation_changes.tsv` row of its own.

#![forbid(unsafe_code)]

use crate::vostok::config::ConfigTable;
use crate::vostok::muzzle::{self, Device, Leaving, Millimetres};
use crate::vostok::{Context, Flow, NoExit};

/// The device a weapon config names: the kit's, else the gun's own.
pub fn device<'t>(root: &ConfigTable<'t>) -> Option<Device<'t>> {
    let data = root.get(c"data").ok()??.table().ok()?;
    let pair = |model: &core::ffi::CStr, fire: &core::ffi::CStr| -> Option<Device<'t>> {
        let model = data.get(model).ok()??.text().ok()?;
        let fire = data.get(fire).ok()??.integer().ok()? as i32;
        (!model.is_empty()).then_some(Device { model, fire: Millimetres(fire) })
    };
    pair(c"srp_muzzle_model", c"srp_muzzle_fire_mm").or_else(|| pair(c"srp_muzzle_stock_model", c"srp_muzzle_stock_fire_mm"))
}

/// The shot's flash: the device's frame moved `fire` along its own third
/// row (the bore, +z of the model); its orientation the frame's, which is
/// barrel_point's (both locators unrotated on the same bone).
pub fn flash(frame: &[f32; 16], fire: Millimetres) -> [f32; 16] {
    let along = fire.metres();
    let mut out = *frame;
    for k in 0..3 {
        out[12 + k] += along * frame[8 + k];
    }
    out
}

/// on_weapon_config_loaded before the requests' alloca (0x24AAB4).
pub fn on_request_room(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if muzzle::cook_config(ctx, true).is_some_and(|root| device(&root).is_some()) {
        muzzle::requests(ctx);
    }
    Flow::Continue
}

/// on_weapon_config_loaded after its last request (0x24ABF9).
pub fn on_request(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(device) = muzzle::cook_config(ctx, false).as_ref().and_then(device) {
        muzzle::push_request(ctx, &device);
    }
    Flow::Continue
}

/// on_weapon_subresources_ready after load_weapon (0x24B0E1).
pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(device) = muzzle::loaded_config(ctx).as_ref().and_then(device) {
        muzzle::loaded(ctx, device.fire);
    }
    Flow::Continue
}

/// weapon::draw after its locators (0x244E99).
pub fn on_locate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    muzzle::locate(ctx, flash);
    Flow::Continue
}

/// weapon::draw after the model's and the aimed scope's scene changes (0x245017).
pub fn on_draw(ctx: &mut Context<'_>) -> Flow<NoExit> {
    muzzle::mirror(ctx);
    Flow::Continue
}

/// on_culled_player_draw after the model's update (0x245353).
pub fn on_culled(ctx: &mut Context<'_>) -> Flow<NoExit> {
    muzzle::remove(ctx, Leaving::Culled);
    Flow::Continue
}

/// on_hide after the model's removal (0x244017).
pub fn on_hidden(ctx: &mut Context<'_>) -> Flow<NoExit> {
    muzzle::remove(ctx, Leaving::Hidden);
    Flow::Continue
}

/// weapon_cook::delete_resource's entry (0x24B190).
pub fn on_release(ctx: &mut Context<'_>) -> Flow<NoExit> {
    muzzle::release(ctx);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flash_goes_along_the_frames_bore() {
        // a frame turned half about y (the bore along -z of the world), at (1, 2, 3)
        let frame = [-1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 2.0, 3.0, 1.0];
        let out = flash(&frame, Millimetres(81));
        assert_eq!(out[..12], frame[..12]);
        assert!((out[12] - 1.0).abs() < 1e-6 && (out[13] - 2.0).abs() < 1e-6 && (out[14] - (3.0 - 0.081)).abs() < 1e-6);
        assert_eq!(flash(&frame, Millimetres(0)), frame);
    }
}

//! Patch 170: the passes a ported 0.69d map's own programs need bind what
//! they read (docs/notes/client_patch.md section 3.92). The programs are the
//! lighting pack's (resource-porter `three-maps-021-lighting`): each takes
//! 0.69d's arithmetic only behind its scene's marker and the native
//! instructions everywhere else, so the bindings are made on every map and a
//! program that does not declare a slot does not bind it.
//!
//! - The geometry fog (`atmospheric_scattering_on_geometry_temporary`): 0.69d
//!   scales its fog's in-scatter by the sun's shadow (sun_lighting.ps, out
//!   to fog_shadow_distance); the sun's screen-space shadow is
//!   `$user$sun_shadow_and_scattering` (render target 8, made before
//!   composition, which reads its x as the shadow), bound as
//!   `t_sun_shadow_and_scattering` as composition binds it (0x1A94D1).
//! - The geometry fog and the sky (the atmosphere pass): 0.69d's
//!   precomputed scattering table ([`SCATTERING`], the pack's texture),
//!   which its fog samples at the horizon and its sky everywhere.
//! - The luminance chain's first program (`gather_log_luminance`): 0.69d's
//!   per-point floor follows the exposure the frame was drawn with; the
//!   adapted mean the tone pass reads, `$user$frame_luminance` (render
//!   target 60, the eye adaptation's output, 0x1B1590), holds the previous
//!   frame's when the chain begins.
//!
//! Render only: nothing a match computes or a world holds changes.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::effect_compile::{self, ExtraTextureSite};
use crate::vostok::{Context, Flow, NoExit};

/// The sun's screen-space shadow and its slot (composition's names).
const SUN_SHADOW_SLOT: &CStr = c"t_sun_shadow_and_scattering";
const SUN_SHADOW: &CStr = c"$user$sun_shadow_and_scattering";

/// 0.69d's scattering table, the lighting pack's texture, and its slot.
const SCATTERING_SLOT: &CStr = c"t_scattering_v069d";
pub const SCATTERING: &CStr = c"ported/v069d/engine/atmosphere_scattering";

/// The adapted mean luminance and its slot (the tone pass's names, 0x1B538A).
const ADAPTED_SLOT: &CStr = c"t_frame_luminance";
const ADAPTED: &CStr = c"$user$frame_luminance";

/// The sky pass: 0.69d's table.
pub fn on_sky_textures(ctx: &mut Context<'_>) -> Flow<NoExit> {
    effect_compile::add_texture(ctx, ExtraTextureSite::Sky, SCATTERING_SLOT, SCATTERING);
    Flow::Continue
}

/// The geometry fog's pass: the sun's shadow and 0.69d's table.
pub fn on_geometry_textures(ctx: &mut Context<'_>) -> Flow<NoExit> {
    effect_compile::add_texture(ctx, ExtraTextureSite::GeometryFog, SUN_SHADOW_SLOT, SUN_SHADOW);
    effect_compile::add_texture(ctx, ExtraTextureSite::GeometryFog, SCATTERING_SLOT, SCATTERING);
    Flow::Continue
}

/// The luminance chain's first pass: the adapted mean.
pub fn on_log_luminance_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    effect_compile::add_texture(ctx, ExtraTextureSite::LuminanceLog, ADAPTED_SLOT, ADAPTED);
    Flow::Continue
}

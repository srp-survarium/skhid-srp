//! Patch 28: the transparent effects (forward_default) scroll their
//! textures as in 0.29 (client_patch.md section 3.26).
//!
//! 0.29's effect_fstage_default_materials::compile reads a material's
//! use_uv_scrolling, unless use_sequence is on, into the shader
//! configuration's CONFIG_USE_UV_SCROLLING bit, and with that bit sends
//!   uv_scrolling_parameters0 = (0, 0, transparency_map_scroll_u_direction,
//!                                     transparency_map_scroll_v_direction)
//!   uv_scrolling_parameters1 = (emissive_map_scroll_u_direction,
//!                               emissive_map_scroll_v_direction, 0, 0)
//! (a number the material does not have is 0) after the uv distortion's
//! constants; its renderer binds scene_slomo, which its shaders multiply the
//! time by, next to scene_time. 0.26g's own g_stage effect does the first two
//! the same way; this does them for the forward effect, and binds
//! scene_slomo to a 1.0 of ours.

#![forbid(unsafe_code)]

use crate::vostok::config::{ConfigKey, Number};
use crate::vostok::render::{ConstantName, ForwardCompile, RendererConstruction};
use crate::vostok::{Context, Flow, NoExit};

/// The transparency map's scrolling: names the exe does not have.
const TRANSPARENCY_SCROLL_U: ConfigKey = ConfigKey::ours(c"transparency_map_scroll_u_direction");
const TRANSPARENCY_SCROLL_V: ConfigKey = ConfigKey::ours(c"transparency_map_scroll_v_direction");

/// scene_slomo's value, read by the renderer for its life.
static ONE: f32 = 1.0;

/// `forward_compile.switch`: before compile_begin, the material's
/// use_uv_scrolling (a switch) asks for UV scrolling, unless it plays a
/// sequence (as in 0.29).
pub fn on_switch(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let mut compile = ForwardCompile::at(ctx);
    if !compile.uses_sequence()
        && compile.material().and_then(|m| m.value(ConfigKey::USE_UV_SCROLLING)).is_some_and(|v| v.switch())
    {
        compile.set_uv_scrolling();
    }
    Flow::Continue
}

/// `forward_compile.constants`: with UV scrolling asked for, the two
/// constants from the material's four numbers.
pub fn on_constants(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let compile = ForwardCompile::at(ctx);
    if !compile.uv_scrolling() {
        return Flow::Continue;
    }
    let number = |key| compile.material().and_then(|m| m.value(key)).map_or(Number::ZERO, |v| v.number());
    let tu = number(TRANSPARENCY_SCROLL_U);
    let tv = number(TRANSPARENCY_SCROLL_V);
    let eu = number(ConfigKey::EMISSIVE_SCROLL_U);
    let ev = number(ConfigKey::EMISSIVE_SCROLL_V);
    if let Some(compiler) = compile.compiler() {
        compiler.set_float4(ConstantName::UV_SCROLLING_PARAMETERS0, [Number::ZERO, Number::ZERO, tu, tv]);
        compiler.set_float4(ConstantName::UV_SCROLLING_PARAMETERS1, [eu, ev, Number::ZERO, Number::ZERO]);
    }
    Flow::Continue
}

/// `renderer_context.scene_time_bound`: scene_slomo bound to 1.0, as 0.29
/// binds it next to scene_time.
pub fn on_scene_time_bound(ctx: &mut Context<'_>) -> Flow<NoExit> {
    RendererConstruction::at(ctx).bind_constant(c"scene_slomo", &ONE);
    Flow::Continue
}

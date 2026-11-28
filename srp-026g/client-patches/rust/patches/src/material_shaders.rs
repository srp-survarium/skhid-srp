//! Patch 221: a material draws with the shaders, textures and constants its
//! own config names (the owner, 2026-11-28, on Cologne (v0.69)'s river:
//! 0.69d's water shader translated for 0.26g, "I like patching exe, we can do
//! that more often"; docs/notes/client_patch.md section 3.116).
//!
//! 0.26g's material effects pick their programs, textures and constants in
//! code: the forward water's compile (0x1E5FE0) names `forward_water`, binds
//! `engine/water_waves2` and sends five constants, whatever the material.
//! With this patch a material's effect table (`material/<stage>/effect`,
//! the tree its compile merges) may say instead:
//!
//! - `srp_pixel_shader`: `{ from = "<family>", to = "<family>" }`: a pass
//!   that begins with the PS family `from` begins with `to` (every material
//!   effect: compile_begin's begin_pass, `material.begin_pass`);
//! - `srp_constants`: an array of `{ name = "<constant>", value = [x, y, z, w] }`:
//!   float4 constants of each of its passes, sent where compile_begin ends
//!   (`material.begun`), before the effect's own (a set keeps a name's first
//!   value; a program without the name skips it);
//! - `srp_textures`: an array of `{ slot = "<t_slot>", texture_path = "<path>" }`:
//!   the texture bound in that slot where the effect binds its own, at a
//!   texture site of the effect's (the forward water's `t_normal_map`,
//!   `water.normal_texture`).
//!
//! A material without the keys compiles as before, so every native material
//! keeps its programs, textures and constants. The names a config gives are
//! kept for good in the patch's pool (`vostok::names`): begin_pass keeps the
//! names it is given, and the config lives only as long as its compile data.
//!
//! Another effect class takes part with its own texture site's hook calling
//! [`texture_for`] (the shader and constant hooks already see every material
//! effect).
//!
//! Presentation only: nothing a match computes reads any of it.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::config::{ConfigTable, ConfigValue};
use crate::vostok::effect_compile::{self, MaterialBegun, MaterialPass, TextureSite};
use crate::vostok::log::{self, Piece::Text};
use crate::vostok::{self, Context, Flow, NoExit, names};

/// The keys of a material's effect table.
const PIXEL_SHADER: &CStr = c"srp_pixel_shader";
const CONSTANTS: &CStr = c"srp_constants";
const TEXTURES: &CStr = c"srp_textures";

/// A pixel shader family replaced by another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PixelShader<'c> {
    from: &'c CStr,
    to: &'c CStr,
}

/// A float4 constant a material sends.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Constant<'c> {
    name: &'c CStr,
    value: [f32; 4],
}

/// A texture a material binds in a slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Texture<'c> {
    slot: &'c CStr,
    texture: &'c CStr,
}

/// What is wrong with a material's keys (the material then compiles as if
/// it had none of that key; the log says which).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    PixelShader,
    Constant,
    Texture,
}

impl Refusal {
    fn text(self) -> &'static str {
        match self {
            Refusal::PixelShader => "material_shaders: a material's srp_pixel_shader is not { from = text, to = text }; its shader kept",
            Refusal::Constant => "material_shaders: a material's srp_constants entry is not { name = text, value = 4 numbers }; skipped",
            Refusal::Texture => "material_shaders: a material's srp_textures entry is not { slot = text, texture_path = text }; skipped",
        }
    }
}

fn text<'c>(table: &ConfigTable<'c>, key: &CStr) -> Option<&'c CStr> {
    table.get(key).ok()??.text().ok()
}

/// A material's `srp_pixel_shader`, if it has one.
fn pixel_shader<'c>(config: &ConfigTable<'c>) -> Result<Option<PixelShader<'c>>, Refusal> {
    let Some(value) = config.get(PIXEL_SHADER).map_err(|_| Refusal::PixelShader)? else { return Ok(None) };
    let table = value.table().map_err(|_| Refusal::PixelShader)?;
    match (text(&table, c"from"), text(&table, c"to")) {
        (Some(from), Some(to)) => Ok(Some(PixelShader { from, to })),
        _ => Err(Refusal::PixelShader),
    }
}

fn four_numbers(value: ConfigValue<'_>) -> Option<[f32; 4]> {
    let entries = value.entries().ok()?;
    if entries.len() != 4 {
        return None;
    }
    let mut out = [0.0; 4];
    for (slot, entry) in out.iter_mut().zip(entries.iter()) {
        *slot = entry.ok()?.number().ok()?;
    }
    Some(out)
}

fn constant<'c>(entry: ConfigValue<'c>) -> Result<Constant<'c>, Refusal> {
    let table = entry.table().map_err(|_| Refusal::Constant)?;
    let name = text(&table, c"name").ok_or(Refusal::Constant)?;
    let value = table.get(c"value").ok().flatten().and_then(four_numbers).ok_or(Refusal::Constant)?;
    Ok(Constant { name, value })
}

fn texture<'c>(entry: ConfigValue<'c>) -> Result<Texture<'c>, Refusal> {
    let table = entry.table().map_err(|_| Refusal::Texture)?;
    match (text(&table, c"slot"), text(&table, c"texture_path")) {
        (Some(slot), Some(texture)) => Ok(Texture { slot, texture }),
        _ => Err(Refusal::Texture),
    }
}

/// Each entry of the array at `key` through `parse`, a refused one logged.
fn each<'c, T>(config: &ConfigTable<'c>, key: &CStr, refusal: Refusal, parse: impl Fn(ConfigValue<'c>) -> Result<T, Refusal>, mut f: impl FnMut(T)) {
    let Some(entries) = config.get(key).ok().flatten().and_then(|value| value.entries().ok()) else { return };
    for entry in entries.iter() {
        match entry.map_err(|_| refusal).and_then(&parse) {
            Ok(item) => f(item),
            Err(refusal) => log::line(&[Text(refusal.text())]),
        }
    }
}

/// The texture a material binds in `slot`, if it names one.
fn texture_in<'c>(config: &ConfigTable<'c>, slot: &CStr) -> Option<&'c CStr> {
    let mut found = None;
    each(config, TEXTURES, Refusal::Texture, texture, |t| {
        if found.is_none() && t.slot == slot {
            found = Some(t.texture);
        }
    });
    found
}

/// The texture a material's config binds in `slot`, kept for good, for an
/// effect class's texture site.
pub fn texture_for(ctx: &Context<'_>, config: Option<&ConfigTable<'_>>, slot: &CStr) -> Option<&'static CStr> {
    let texture = texture_in(config?, slot)?;
    let kept = names::intern(ctx.thread(), texture);
    if kept.is_none() {
        log::line(&[Text("material_shaders: the name pool is full; a material's texture not bound")]);
    }
    kept
}

/// compile_begin's begin_pass: a material that names another pixel shader
/// for this pass's family begins it with that one.
pub fn on_material_pass(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(mut pass) = MaterialPass::at(ctx) else { return Flow::Continue };
    let Some(config) = pass.config() else { return Flow::Continue };
    let replace = match pixel_shader(config) {
        Ok(replace) => replace,
        Err(refusal) => {
            log::line(&[Text(refusal.text())]);
            None
        }
    };
    if let Some(PixelShader { from, to }) = replace
        && pass.pixel_shader() == Some(from)
    {
        match names::intern(ctx.thread(), to) {
            Some(to) => pass.set_pixel_shader(to),
            None => log::line(&[Text("material_shaders: the name pool is full; a material's pixel shader kept")]),
        }
    }
    Flow::Continue
}

/// compile_begin's end: a material's constants for this pass.
pub fn on_material_begun(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let begun = MaterialBegun::at(ctx);
    let Some(config) = begun.config() else { return Flow::Continue };
    each(config, CONSTANTS, Refusal::Constant, constant, |c| match names::intern(ctx.thread(), c.name) {
        Some(name) => begun.send_float4(name, c.value),
        None => log::line(&[Text("material_shaders: the name pool is full; a material's constant not sent")]),
    });
    Flow::Continue
}

/// The forward water's t_normal_map: a material's own texture there.
pub fn on_water_texture(ctx: &mut Context<'_>) -> Flow<vostok::MaterialShadersOnWaterTextureExit> {
    let config = effect_compile::water_config(ctx);
    match texture_for(ctx, config.as_ref(), c"t_normal_map") {
        Some(texture) => {
            effect_compile::bind_texture(ctx, TextureSite::WaterWaves, texture);
            Flow::Exit(vostok::MaterialShadersOnWaterTextureExit::Bound)
        }
        None => Flow::Continue,
    }
}

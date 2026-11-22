//! Patch 169: the forward effects draw as 0.44a5's (README "The forward
//! effects of 0.44a5", client_patch.md section 3.91).
//!
//! 0.44a5's forward_default and particle_lighting read switches 0.26g's do
//! not (re-kit v044a5: 0x4BC960, 0x4BEFA0), each a bit of the shader
//! configuration that selects the programs (fill_shader_configuration_macros
//! lays bytes 0Ch and 0Dh out alike in both builds):
//! - use_ttransparency_dxt1, with use_ttransparency: CONFIG_USE_OLTA, the
//!   transparency map a DXT1 image (both compiles);
//! - use_uv_tiling, without use_sequence: CONFIG_USE_DISPLACEMENT, and
//!   constant_tile_uv the two maps' tiling factors as a float4 (forward);
//! - apply_distortion_to_emissive_only: CONFIG_USE_FUZZINESS (forward).
//!
//! 0.44a5's particle lighting programs at high particle quality also read
//! probe_diffuse_mult, the batch's probe's diffuse light times the
//! environment's probe intensity: 0.26g has both factors and binds no
//! constant of them, so the patch registers it and sets it each pass.
//!
//! use_uv_scrolling is patch 28's. No material players have sets any of
//! these: their compiles are as they were.

#![forbid(unsafe_code)]

use crate::vostok::effect_compile::{self, BeginSite, CompileBegin, MaterialFields, Switches0d};
use crate::vostok::render_stage;
use crate::vostok::{Context, Flow, NoExit};

/// What the transparency is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Transparency {
    /// constant_transparency alone (no use_ttransparency).
    Constant,
    /// A map, its alpha read.
    Map,
    /// A map that is a DXT1 image.
    Dxt1Map,
}

impl Transparency {
    fn read(material: &impl MaterialFields) -> Transparency {
        match (material.switch(c"use_ttransparency"), material.switch(c"use_ttransparency_dxt1")) {
            (false, _) => Transparency::Constant,
            (true, false) => Transparency::Map,
            (true, true) => Transparency::Dxt1Map,
        }
    }
}

/// The emissive and transparency maps' tiling, (u, v) each.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TileFactors {
    emissive: [f32; 2],
    transparency: [f32; 2],
}

impl TileFactors {
    /// Each factor the material lacks is 1 (0.44a5's 0x4BD5C0..0x4BD6A4).
    fn read(material: &impl MaterialFields) -> TileFactors {
        let factor = |name| material.number(name).unwrap_or(1.0);
        TileFactors {
            emissive: [factor(c"emissive_map_tiling_u_factor"), factor(c"emissive_map_tiling_v_factor")],
            transparency: [factor(c"transparency_map_tiling_u_factor"), factor(c"transparency_map_tiling_v_factor")],
        }
    }

    /// constant_tile_uv as 0.44a5 packs it (0x4BD6AC).
    fn constant(self) -> [f32; 4] {
        [self.emissive[0], self.emissive[1], self.transparency[0], self.transparency[1]]
    }
}

/// How the maps' coordinates move.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Uv {
    /// A sequence of frames: no scrolling or tiling (0.44a5 reads neither).
    Sequence,
    /// The mesh's own coordinates.
    Plain,
    /// Tiled by the factors.
    Tiled(TileFactors),
}

impl Uv {
    fn read(material: &impl MaterialFields) -> Uv {
        if material.switch(c"use_sequence") {
            Uv::Sequence
        } else if material.switch(c"use_uv_tiling") {
            Uv::Tiled(TileFactors::read(material))
        } else {
            Uv::Plain
        }
    }
}

/// A forward_default material's switches 0.26g's compile does not read.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ForwardMaterial {
    transparency: Transparency,
    uv: Uv,
    /// apply_distortion_to_emissive_only: the uv distortion moves the
    /// emissive map alone.
    distortion_on_emissive_only: bool,
}

impl ForwardMaterial {
    fn read(material: &impl MaterialFields) -> ForwardMaterial {
        ForwardMaterial {
            transparency: Transparency::read(material),
            uv: Uv::read(material),
            distortion_on_emissive_only: material.switch(c"apply_distortion_to_emissive_only"),
        }
    }

    /// The configuration's byte 0Dh as 0.44a5's compile leaves it.
    fn switches(self, byte: Switches0d) -> Switches0d {
        byte.with(Switches0d::FUZZINESS, self.distortion_on_emissive_only)
            .with(Switches0d::OLTA, self.transparency == Transparency::Dxt1Map)
            .with(Switches0d::DISPLACEMENT, matches!(self.uv, Uv::Tiled(_)))
    }
}

/// forward_default's call of compile_begin (`forward.compile_begin`).
pub fn on_forward_begin(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut begin) = CompileBegin::at(ctx, BeginSite::Forward)
        && let Some(material) = begin.tree().map(ForwardMaterial::read)
    {
        let switches = material.switches(begin.switches_0d());
        begin.set_switches_0d(switches);
    }
    Flow::Continue
}

/// particle_lighting's call of compile_begin (`particle_lighting.compile_begin`).
pub fn on_particle_lighting_begin(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(mut begin) = CompileBegin::at(ctx, BeginSite::ParticleLighting)
        && let Some(transparency) = begin.tree().map(Transparency::read)
    {
        let switches = begin.switches_0d().with(Switches0d::OLTA, transparency == Transparency::Dxt1Map);
        begin.set_switches_0d(switches);
    }
    Flow::Continue
}

/// forward_default's constant_tile_uv (`forward.tile_uv`): a tiled
/// material's factors, set before the game's float2.
pub fn on_tile_uv(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(Uv::Tiled(factors)) = effect_compile::tile_uv_tree(ctx).map(|tree| Uv::read(&tree)) {
        effect_compile::send_tile_uv(ctx, factors.constant());
    }
    Flow::Continue
}

/// stage_lights' constructor once its hosts are made (`stage_lights.constructed`).
pub fn on_stage_lights_constructed(ctx: &mut Context<'_>) -> Flow<NoExit> {
    render_stage::register_probe_multiplier(ctx);
    Flow::Continue
}

/// The particle pass with its probe's constants set (`stage_lights.particle_probe`):
/// 0.44a5's particle lighting programs read probe_diffuse_mult too.
pub fn on_particle_probe(ctx: &mut Context<'_>) -> Flow<NoExit> {
    render_stage::set_probe_multiplier(ctx);
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use core::ffi::CStr;

    use super::*;

    /// A made-up material: (key, value) pairs, a switch any value not 0.
    struct Fields(&'static [(&'static CStr, f32)]);

    impl MaterialFields for Fields {
        fn switch(&self, name: &CStr) -> bool {
            self.number(name).is_some_and(|x| x != 0.0)
        }
        fn number(&self, name: &CStr) -> Option<f32> {
            self.0.iter().find(|(key, _)| *key == name).map(|(_, value)| *value)
        }
    }

    const SOFT_EDGES: u8 = 0x08;

    #[test]
    fn a_dxt1_map_needs_a_map() {
        let dxt1 = Fields(&[(c"use_ttransparency", 1.0), (c"use_ttransparency_dxt1", 1.0)]);
        assert_eq!(Transparency::read(&dxt1), Transparency::Dxt1Map);
        assert_eq!(Transparency::read(&Fields(&[(c"use_ttransparency_dxt1", 1.0)])), Transparency::Constant);
        assert_eq!(Transparency::read(&Fields(&[(c"use_ttransparency", 1.0)])), Transparency::Map);
    }

    #[test]
    fn a_sequence_neither_scrolls_nor_tiles() {
        let tiled = Fields(&[(c"use_uv_tiling", 1.0), (c"transparency_map_tiling_v_factor", 0.4)]);
        assert_eq!(Uv::read(&tiled), Uv::Tiled(TileFactors { emissive: [1.0, 1.0], transparency: [1.0, 0.4] }));
        assert_eq!(Uv::read(&Fields(&[(c"use_uv_tiling", 1.0), (c"use_sequence", 1.0)])), Uv::Sequence);
        assert_eq!(Uv::read(&Fields(&[])), Uv::Plain);
    }

    #[test]
    fn the_tiling_constant_is_the_emissive_then_the_transparency_factors() {
        let factors = TileFactors { emissive: [0.7, 0.75], transparency: [0.4, 0.5] };
        assert_eq!(factors.constant(), [0.7, 0.75, 0.4, 0.5]);
    }

    #[test]
    fn the_switches_set_their_bits_and_keep_the_others() {
        // ground_wind (0.44a5): a DXT1 map, tiled, distortion on both maps.
        let ground_wind = ForwardMaterial::read(&Fields(&[
            (c"use_ttransparency", 1.0),
            (c"use_ttransparency_dxt1", 1.0),
            (c"use_uv_tiling", 1.0),
            (c"transparency_map_tiling_u_factor", 0.4),
            (c"transparency_map_tiling_v_factor", 0.4),
        ]));
        let byte = ground_wind.switches(Switches0d::default_for_test(SOFT_EDGES | Switches0d::FUZZINESS));
        assert_eq!(byte.bits(), SOFT_EDGES | Switches0d::OLTA | Switches0d::DISPLACEMENT);
        let emissive_only = ForwardMaterial::read(&Fields(&[(c"apply_distortion_to_emissive_only", 1.0)]));
        assert_eq!(emissive_only.switches(Switches0d::default_for_test(0)).bits(), Switches0d::FUZZINESS);
    }
}

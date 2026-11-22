//! Patch 183: the g-buffer's parallax constants as 0.44a5 sets them (README
//! "The parallax of 0.44a5", client_patch.md section 3.99).
//!
//! 0.44a5's g-buffer material compile (re-kit v044a5 0x4C1F20) sets two
//! constants in its parallax block that 0.26g's (0x1DDD50) never sets:
//! parallax_uv_scrolling_factors and parallax_uv_scale_factors, from the
//! material's own fields. Its parallax programs at the highest shading
//! quality read both. The patch sets them at the same place of 0.26g's
//! parallax block, for every material with parallax, as 0.44a5 does: no
//! program players have reads them.

#![forbid(unsafe_code)]

use crate::vostok::effect_compile::{GstageParallax, MaterialFields};
use crate::vostok::{Context, Flow, NoExit};

/// A parallax material's uv factors.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ParallaxUv {
    /// (u, v) speeds: parallax_uv_scrolling_factors.
    scrolling: [f32; 2],
    /// (u, v) scales: parallax_uv_scale_factors.
    scale: [f32; 2],
}

impl ParallaxUv {
    /// Each speed the material lacks is 0, each scale 1 (0.44a5's
    /// 0x4C4464, 0x4C44BF, 0x4C452E, 0x4C4589).
    fn read(material: &impl MaterialFields) -> ParallaxUv {
        let or = |name, default| material.number(name).unwrap_or(default);
        ParallaxUv {
            scrolling: [or(c"parallax_u_scrolling_speed", 0.0), or(c"parallax_v_scrolling_speed", 0.0)],
            scale: [or(c"parallax_u_scale", 1.0), or(c"parallax_v_scale", 1.0)],
        }
    }
}

/// The g-buffer compile's parallax block (`gstage.parallax`).
pub fn on_parallax(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let block = GstageParallax::at(ctx);
    if let Some(uv) = block.tree().map(ParallaxUv::read) {
        block.send_float2(c"parallax_uv_scrolling_factors", uv.scrolling);
        block.send_float2(c"parallax_uv_scale_factors", uv.scale);
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use core::ffi::CStr;

    use super::*;

    struct Fields(&'static [(&'static CStr, f32)]);

    impl MaterialFields for Fields {
        fn switch(&self, name: &CStr) -> bool {
            self.number(name).is_some_and(|x| x != 0.0)
        }
        fn number(&self, name: &CStr) -> Option<f32> {
            self.0.iter().find(|(key, _)| *key == name).map(|(_, value)| *value)
        }
    }

    #[test]
    fn a_missing_speed_is_0_and_a_missing_scale_1() {
        assert_eq!(ParallaxUv::read(&Fields(&[])), ParallaxUv { scrolling: [0.0, 0.0], scale: [1.0, 1.0] });
        let own = Fields(&[(c"parallax_v_scrolling_speed", 0.25), (c"parallax_u_scale", 2.0)]);
        assert_eq!(ParallaxUv::read(&own), ParallaxUv { scrolling: [0.0, 0.25], scale: [2.0, 1.0] });
    }
}

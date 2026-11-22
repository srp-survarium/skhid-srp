//! Patch 183: the g-buffer's parallax constants as 0.44a5 sets them (README
//! "The parallax of 0.44a5", client_patch.md section 3.99).
//!
//! 0.44a5's g-buffer material compile (re-kit v044a5 0x4C1F20) sets two
//! constants in its parallax block that 0.26g's (0x1DDD50) never sets:
//! parallax_uv_scrolling_factors and parallax_uv_scale_factors, from the
//! material's own fields. Its parallax programs at the highest shading
//! quality read both. The patch sets them at the same place of 0.26g's
//! parallax block, for every material with parallax, as 0.44a5 does: no
//! program players have reads them. `SKHID_PARALLAX_UV_LOG=on` logs each
//! compile's values.

#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicU8, Ordering};

use crate::vostok::effect_compile::{GstageParallax, MaterialFields};
use crate::vostok::log::{Piece::*, line};
use crate::vostok::renderer::get_environment;
use crate::vostok::{Context, Flow, NoExit, Thread};

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
        if log_on(ctx.thread()) {
            let [su, sv] = uv.scrolling.map(f32::to_bits);
            let [u, v] = uv.scale.map(f32::to_bits);
            line(&[Text("parallax_uv: scrolling "), Hex(su), Text(" "), Hex(sv), Text(" scale "), Hex(u), Text(" "), Hex(v)]);
        }
    }
    Flow::Continue
}

/// SKHID_PARALLAX_UV_LOG=on: each parallax compile's values (float bits) in
/// srp_patches.log. Read at the first compile, on whichever resource thread.
static LOG: AtomicU8 = AtomicU8::new(UNREAD);
const UNREAD: u8 = 0;
const OFF: u8 = 1;
const ON: u8 = 2;

fn log_on(thread: &Thread) -> bool {
    let known = match LOG.load(Ordering::Relaxed) {
        UNREAD => {
            let mut room = [0u8; 8];
            let len = get_environment(thread, c"SKHID_PARALLAX_UV_LOG", &mut room);
            let value = room.get(..len).unwrap_or(&[]);
            let read = if value.eq_ignore_ascii_case(b"on") || value == b"1" { ON } else { OFF };
            LOG.store(read, Ordering::Relaxed);
            read
        }
        known => known,
    };
    known == ON
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

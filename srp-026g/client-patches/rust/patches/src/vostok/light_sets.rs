//! What the frame's culling found of the environment probes, ambient lights
//! and lights, and which of them the frame will draw, for patch 189
//! (light_set_log; renderer-026g.md 4.3, 5.3, 5.4).
//!
//! At `stage_visibility.culled` the scene view's three vectors hold what the
//! frustum culling found; the hi-Z occlusion has answered for them (their
//! occluded bytes). The frame then drops, with the same tests as here:
//! - a probe that is disabled, whose cubemap or diffuse cubemap is not ready
//!   (res_texture +8 and its shader view), or occluded while the hi-Z option
//!   is on (0x1B9320);
//! - an ambient light that is disabled, or occluded with the option on
//!   (0x1B9130);
//! - a light that is disabled, or occluded with the option on and not
//!   waiting for a refresh (stage_lights::execute 0x1934E0).
//!
//! All the render thread's: the site and the drawing that follows run on it.

use super::context::Context;
use crate::raw;

/// The most of each kind read in a frame (a London view finds about 60
/// probes and 100 ambient lights).
pub const MOST: usize = 256;

/// Why a culled-in probe is or is not drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeState {
    Drawn,
    Disabled,
    /// Its cubemap (specular) is not ready.
    NoCubemap,
    /// Its diffuse cubemap is not ready.
    NoDiffuse,
    Occluded,
}

/// Why a culled-in ambient light or light is or is not drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightState {
    Drawn,
    Disabled,
    Occluded,
}

/// The probe's name: the tail of its cubemap's path, which is unique in a project.
pub const NAME: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
    /// Its address: the same probe frame to frame.
    pub id: u32,
    pub name: [u8; NAME],
    pub name_len: u8,
    pub location: [f32; 3],
    pub radius: f32,
    pub state: ProbeState,
}

impl Probe {
    pub fn name(&self) -> &str {
        core::str::from_utf8(self.name.get(..self.name_len as usize).unwrap_or(&[])).unwrap_or("?")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub id: u32,
    pub location: [f32; 3],
    pub radius: f32,
    /// An ambient light's intensity (0 for a light).
    pub intensity: f32,
    pub state: LightState,
}

/// The frame's three culled sets.
pub struct Culled {
    pub probes: [Option<Probe>; MOST],
    pub ambient: [Option<Light>; MOST],
    pub lights: [Option<Light>; MOST],
}

fn texture_ready(texture: Option<raw::ResTexture>) -> bool {
    // SAFETY: a probe's texture lives while the probe holds it (the
    // render thread's).
    texture.is_some_and(|t| unsafe { t.ready().read() != 0 && t.shader_view().read() != 0 })
}

fn float3(at: *mut f32) -> [f32; 3] {
    // SAFETY: a float3 field of a live object.
    unsafe { [at.read(), at.add(1).read(), at.add(2).read()] }
}

impl Culled {
    /// Fills `into` with the frame's sets at `stage_visibility.culled`;
    /// false when the stage has no frame.
    pub fn read_at_site(ctx: &Context<'_>, into: &mut Culled) -> bool {
        use raw::site::stage_visibility_culled as site;
        into.probes = [None; MOST];
        into.ambient = [None; MOST];
        into.lights = [None; MOST];
        // SAFETY: the site's ESI is the stage, its context's scene view the
        // frame's; the vectors' slots hold live objects for the frame (the
        // filters that follow read them the same way); render thread.
        unsafe {
            let Some(view) = site::stage(ctx).and_then(|stage| stage.context()).and_then(|context| context.scene_view()) else {
                return false;
            };
            let hiz = raw::image::render_options_global().options().is_some_and(|options| options.hiz_occlusion().read() != 0);
            if let (Some(first), Some(end)) = (view.probes(), view.probes_end()) {
                let count = end.index_from(first).unwrap_or(0).min(MOST);
                for (i, slot) in into.probes.iter_mut().enumerate().take(count) {
                    let Some(probe) = first.at(i).probe() else { continue };
                    let state = if probe.enabled().read() == 0 {
                        ProbeState::Disabled
                    } else if !texture_ready(probe.specular_cubemap()) {
                        ProbeState::NoCubemap
                    } else if !texture_ready(probe.diffuse_cubemap()) {
                        ProbeState::NoDiffuse
                    } else if hiz && probe.occluded().read() != 0 {
                        ProbeState::Occluded
                    } else {
                        ProbeState::Drawn
                    };
                    let full = core::slice::from_raw_parts(probe.name(), 260);
                    let len = full.iter().position(|&b| b == 0).unwrap_or(260);
                    let tail = &full[len.saturating_sub(NAME)..len];
                    let mut name = [0u8; NAME];
                    name[..tail.len()].copy_from_slice(tail);
                    *slot = Some(Probe {
                        id: probe.address(),
                        name,
                        name_len: tail.len() as u8,
                        location: float3(probe.location()),
                        radius: probe.radius().read(),
                        state,
                    });
                }
            }
            if let (Some(first), Some(end)) = (view.ambient_lights(), view.ambient_lights_end()) {
                let count = end.index_from(first).unwrap_or(0).min(MOST);
                for (i, slot) in into.ambient.iter_mut().enumerate().take(count) {
                    let Some(light) = first.at(i).light() else { continue };
                    let state = if light.enabled().read() == 0 {
                        LightState::Disabled
                    } else if hiz && light.occluded().read() != 0 {
                        LightState::Occluded
                    } else {
                        LightState::Drawn
                    };
                    *slot = Some(Light {
                        id: light.address(),
                        location: float3(light.location()),
                        radius: light.radius().read(),
                        intensity: light.intensity().read(),
                        state,
                    });
                }
            }
            if let (Some(first), Some(end)) = (view.lights(), view.lights_end()) {
                let count = end.index_from(first).unwrap_or(0).min(MOST);
                for (i, slot) in into.lights.iter_mut().enumerate().take(count) {
                    let Some(light) = first.at(i).light() else { continue };
                    let state = if light.enabled().read() == 0 {
                        LightState::Disabled
                    } else if hiz && light.occluded().read() != 0 && light.refresh().read() == 0 {
                        LightState::Occluded
                    } else {
                        LightState::Drawn
                    };
                    *slot = Some(Light {
                        id: light.address(),
                        location: float3(light.position()),
                        radius: light.range().read(),
                        intensity: 0.0,
                        state,
                    });
                }
            }
        }
        true
    }
}

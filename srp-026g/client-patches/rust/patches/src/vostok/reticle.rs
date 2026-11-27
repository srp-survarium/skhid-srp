//! The reticle's game side (`docs/notes/reticle-choice.md`, "The drawing"):
//! a collimator's reticle is a model of the patch's own, in the scene exactly
//! while the optic's aimed model is, drawn in the colour and brightness the
//! player chose.
//!
//! - A pattern is a static model of pack `weapon-modules` (`weapons/reticles_069/<name>`), asked
//!   for ([`ask`], on the game's thread) and kept for the process with its surface's material
//!   effects ([`Pattern`]).
//! - The renderer recolours a model only through a material appended to it
//!   (`scene_renderer::append_model_material` 0x24FE0): the forward stage
//!   draws each surface with its own material and then with every appended
//!   one (`stage_forward` 0x1AB4BB), the appended one's model parameters
//!   (`set_model_material_parameter_impl` 0x24E60) set over its own constants
//!   first (0x1AB545, 0x159350). An appended material is a material effects
//!   instance (the entry's +90Ch, which the stage asks for the surface's
//!   vertex type, `get_material_effects` 0x16B070), the object a surface's
//!   material is (render_surface +1Ch). So the pattern's own surface's is
//!   appended to its model once ([`ReticleScene::append`]), and the appended copy's emission is
//!   the shader constant `solid_color_specular` (forward_default's
//!   `constant_emissive` times its multiplier, which its compile sends as one
//!   float4: 0x1D9578..0x1D9652). The material's own multiplier is 0: the
//!   surface's own pass adds black.
//! - Each draw of a weapon (`attachments.draw`, after the game added or
//!   removed its aimed optic) says which aimed model is in the scene and where
//!   ([`DrawnGun`]); the reticle goes there, or out ([`Shown`]).
//!
//! The parameter's handle ([`EMISSION`]) is ours for the process: the render
//! thread writes its constant's id into it when it is registered
//! (`world::register_material_parameter` 0x1185B0), and every command that
//! sets the parameter keeps its address.

use core::cell::UnsafeCell;
use core::ffi::CStr;
use core::sync::atomic::{AtomicBool, Ordering};

use weapon_modules::Affine;

use super::attachments::{self, WeaponId};
use super::carry_runtime::Scene;
use super::context::Context;
use super::resources::{self, KeptModel, QueryCallback};
use super::world::{floats, read};
use super::Thread;
use crate::raw;

/// The appended material's place among a model's appended ones: the first
/// of its material (the player's sphere takes 0 and 1 of its own).
const APPENDED: u32 = 0;
/// The emission's constant: forward_default's `constant_emissive` times its
/// multiplier, one float4 (0x1D9648).
const EMISSION_NAME: &CStr = c"solid_color_specular";
/// A float4 parameter (the player's `sphere_color`, 0x1F1CCE).
const FLOAT4: u32 = 1;

/// The emission parameter's handle {id, name, kind}: room of ours that the
/// render thread fills ([`raw::RenderMaterialParameter`]'s `id`) and the
/// renderer's commands point at.
struct ParameterRoom(UnsafeCell<raw::RenderMaterialParameterStorage>);

// SAFETY: the room is borrowed once, by the one registration
// (REGISTERED's swap), to write its name and kind and take its address;
// after that only the renderer reads and writes it, through that address.
unsafe impl Sync for ParameterRoom {}

static EMISSION: ParameterRoom = ParameterRoom(UnsafeCell::new(raw::RenderMaterialParameterStorage::zeroed()));
static REGISTERED: AtomicBool = AtomicBool::new(false);

/// The emission parameter, registered with the renderer: what
/// [`ReticleScene::emit`] sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Emitter(raw::RenderMaterialParameter);

// SAFETY: the handle of a room kept for the process (EMISSION), never read
// through by us after its registration.
unsafe impl Send for Emitter {}

/// A pattern's model, one reference of ours kept for the process, and its
/// one surface's material effects (the model holds them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pattern {
    model: KeptModel,
    effects: Effects,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Effects(raw::MaterialEffectsInstance);

// SAFETY: held alive by its model, which is kept for the process; only its
// address is handed to the renderer.
unsafe impl Send for Effects {}

/// The most surfaces a pattern's model is asked for (it has one).
const SURFACES: usize = 8;

/// What a query of [`ask`] brought: the pattern, or None when its model did
/// not load or has no surface with material effects.
pub fn arrived(assets: &resources::ArrivedAssets) -> Option<Pattern> {
    let model = assets.model?;
    let mut rooms = [const { raw::SurfaceSlotStorage::zeroed() }; SURFACES];
    let mut vector = raw::SurfaceVectorStorage::zeroed();
    // SAFETY: the model is ours, alive, its render instance a static
    // model's; the vector is a buffer_vector over our rooms (begin, end,
    // capacity), which get_all_surfaces pushes the instance's surfaces into
    // as set_transform's own (0x158CD5..0x158CEA); a pushed slot is a live
    // surface instance of the model, its render surface and its material
    // effects the model's.
    unsafe {
        let instance = model.0.render_instance().instance()?;
        let first = raw::SurfaceSlotStorage::first(&mut rooms)?;
        let v = vector.handle();
        v.set_begin(Some(first));
        v.set_end(Some(first));
        v.set_capacity(Some(first.at(SURFACES)));
        raw::engine_model_surfaces(Some(instance), Some(v));
        let pushed = v.end()?.index_from(first)?.min(SURFACES);
        let effects = (0..pushed).find_map(|n| first.at(n).instance()?.surface()?.material_effects())?;
        Some(Pattern { model, effects: Effects(effects) })
    }
}

/// Asks for a pattern's model, which arrives at `callback` as
/// [`resources::Arrived::Assets`] ([`arrived`]).
pub fn ask(thread: &Thread, model: &CStr, callback: QueryCallback) {
    resources::query_assets(thread, model, &[], callback);
}

/// The emission constant of the appended material: the colour times the
/// brightness's multiplier, and 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emission(pub [f32; 4]);

/// A weapon at `attachments.draw`, after the game added or removed its aimed
/// optic.
pub struct DrawnGun<'t> {
    weapon: raw::Weapon,
    ctx: &'t Context<'t>,
}

impl<'t> DrawnGun<'t> {
    pub fn at_draw(ctx: &'t Context<'t>) -> Option<DrawnGun<'t>> {
        // SAFETY: the site's EBX is the weapon weapon::draw draws.
        let weapon = unsafe { raw::site::attachments_draw::weapon(ctx)? };
        Some(DrawnGun { weapon, ctx })
    }

    pub fn id(&self) -> WeaponId {
        WeaponId::of(self.weapon)
    }

    pub fn thread(&self) -> &'t Thread {
        self.ctx.thread()
    }

    /// The frame the optic's aimed model was put at this draw (+660h, the
    /// update at 0x245012), while it is in the scene (the local player's gun
    /// aimed in the first person).
    pub fn optic_aimed(&self) -> Option<Affine> {
        // SAFETY: the draw's live weapon; its optics object and their aimed
        // model holder are its own; +660h is its matrix, written by this draw.
        unsafe {
            let aimed = self.weapon.optics()?.aim_model()?;
            Scene::has(aimed).then(|| Affine::from_matrix(&floats(self.weapon.transform())))
        }
    }

    /// The frame of a fitted part's aimed model in the scene (the second
    /// sight looked through), as patch 161's draw put it this frame.
    pub fn part_aimed(&self) -> Option<Affine> {
        attachments::aimed_part_frame(self.ctx)
    }

    /// The weapon's world's scene.
    pub fn scene(&self) -> Option<ReticleScene<'t>> {
        // SAFETY: the draw's live weapon and its world, whose scene the draw submits to.
        let scene = unsafe { Scene::of_world(self.weapon.world()?)? };
        Some(ReticleScene { scene, _ctx: core::marker::PhantomData })
    }
}

/// A weapon that leaves the scene ([`leaving`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaving {
    /// weapon::on_hide (`attachments.hidden`).
    Hidden,
    /// a culled player's weapon (`attachments.culled`).
    Culled,
}

/// The weapon at a site where it leaves the scene, and its world's scene.
pub fn leaving<'t>(ctx: &'t Context<'t>, at: Leaving) -> Option<(WeaponId, ReticleScene<'t>)> {
    // SAFETY: each site's weapon register, the live weapon its function
    // works on, and its world's scene, which that function submits to.
    unsafe {
        let weapon = match at {
            Leaving::Hidden => raw::site::attachments_hidden::weapon(ctx)?,
            Leaving::Culled => raw::site::attachments_culled::weapon(ctx)?,
        };
        let scene = Scene::of_world(weapon.world()?)?;
        Some((WeaponId::of(weapon), ReticleScene { scene, _ctx: core::marker::PhantomData }))
    }
}

/// `attachments.release`: the weapon weapon_cook frees, and its world's
/// scene while that lives (as patch 161's parts).
pub fn released<'t>(ctx: &'t Context<'t>) -> Option<(WeaponId, Option<ReticleScene<'t>>)> {
    // SAFETY: delete_resource's entry: [esp+4] the weapon's resource base
    // (the weapon + 10h), or 0; the weapon is alive until the cook deletes it
    // after this.
    unsafe {
        let weapon = raw::site::attachments_release::frame(ctx)?.resource()?.weapon();
        let scene = weapon
            .world()
            .filter(|world| read(world.scene().value()) != 0)
            .and_then(|world| Scene::of_world(world))
            .map(|scene| ReticleScene { scene, _ctx: core::marker::PhantomData });
        Some((WeaponId::of(weapon), scene))
    }
}

/// A world's scene for the reticle (Artefact carry's hiding of a gun).
///
/// # Safety
/// `world` is a live client world of the hook's.
pub(super) unsafe fn of_scene<'t>(world: raw::GameWorld) -> Option<ReticleScene<'t>> {
    // SAFETY: the caller's.
    let scene = unsafe { Scene::of_world(world)? };
    Some(ReticleScene { scene, _ctx: core::marker::PhantomData })
}

/// A weapon's world's scene, for the hook that found it.
pub struct ReticleScene<'t> {
    scene: Scene,
    _ctx: core::marker::PhantomData<&'t ()>,
}

impl ReticleScene<'_> {
    /// `pattern`'s model in the scene at `at` (added, or moved there), drawn
    /// in the first-person pass as a movable model, as the aimed optic is.
    pub fn show(&self, pattern: Pattern, at: &Affine) {
        // SAFETY: the pattern's model is held by us for the process; the
        // scene is the hook's; the two bytes set_foreground and
        // set_movable_static write (weapon::set_foreground 0x243D40).
        unsafe {
            if let Some(instance) = pattern.model.0.render_instance().instance() {
                instance.foreground().write(1);
                instance.movable().write(1);
            }
            self.scene.present(pattern.model.0, &at.matrix());
        }
    }

    /// `pattern`'s model out of the scene, when it is in it.
    pub fn hide(&self, pattern: Pattern) {
        // SAFETY: as show; the renderer's remove is guarded by the model's own in-scene byte.
        unsafe { self.scene.remove(pattern.model.0) };
    }

    /// The emission parameter registered with the renderer: the first call
    /// of the process alone gives it (None after).
    pub fn register(&self) -> Option<Emitter> {
        if REGISTERED.swap(true, Ordering::AcqRel) {
            return None;
        }
        // SAFETY: the one borrow of the room (REGISTERED's swap above): its
        // name (a static string) and kind written before the renderer has its
        // address; the renderer is the live scene's.
        unsafe {
            let parameter = (*EMISSION.0.get()).handle();
            parameter.set_name(EMISSION_NAME.as_ptr().cast_mut().cast());
            *parameter.kind() = FLOAT4;
            raw::engine_register_material_parameter(Some(self.scene.renderer()), Some(parameter));
            Some(Emitter(parameter))
        }
    }

    /// The pattern's material appended to its model (once a pattern: a
    /// second would draw it twice).
    pub fn append(&self, pattern: Pattern) {
        // SAFETY: the renderer is the live scene's; the model and the
        // material are held by us, the material's word read (and a reference
        // taken) before the call returns (0x25034).
        unsafe {
            let mut material = Some(pattern.effects.0);
            raw::engine_append_model_material(Some(self.scene.renderer()), Some(pattern.model.0.render_instance()), &mut material, APPENDED);
        }
    }

    /// The appended material's emission on `pattern`'s model.
    pub fn emit(&self, emitter: Emitter, pattern: Pattern, emission: Emission) {
        // SAFETY: as append; the value is copied into the command before the
        // call returns (0x24F4A); the parameter's room is ours for the process.
        unsafe {
            let mut material = Some(pattern.effects.0);
            let value = emission.0;
            raw::engine_set_model_material_parameter(
                Some(self.scene.renderer()),
                Some(pattern.model.0.render_instance()),
                &mut material,
                APPENDED,
                Some(emitter.0),
                value.as_ptr().cast(),
                core::mem::size_of_val(&value) as u32,
            );
        }
    }
}

//! Native boundaries for carry mode. IDs kept by the patch are comparison
//! values; every dereference starts with a live hook argument or world lookup.

use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, Ordering};
use artefact_carry::{Artefact, Place};
use artefact_carry::carry::{Carry, Time};
use super::{Context, Reg};
use super::world::{Object, Player, floats, pointer, read};
use crate::raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key { pub object: u32, pub world: u32, pub spawn: u32, pub index: usize }

impl Player<'_> {
    pub fn carry_key(&self) -> Option<Key> {
        // SAFETY: this is a live base_player. No client-only fields here.
        unsafe {
            let place = Place::parse(read(raw::player::place(self.this.ptr())))?;
            let kit = read(raw::player::kit(self.this.ptr()));
            if kit >= 3 { return None; }
            let world = pointer(raw::player::world_core(self.this.ptr()))?;
            Some(Key { object: self.this.id(), world: world.id(), spawn: self.spawned(), index: usize::from(place.get()) * 3 + usize::from(kit) })
        }
    }
    pub fn carry_time(&self) -> Time {
        // SAFETY: live player's simulation clock.
        Time(unsafe { read(raw::player::time(self.this.ptr())) })
    }
    pub fn carry_allowed(&self) -> bool {
        if !self.alive() || !self.inserted() { return false; }
        // SAFETY: only known weapon interfaces can enter carry mode.
        unsafe { pointer(raw::player::held_item(self.this.ptr())).is_some_and(|held| {
            [raw::image::WEAPON_VTABLE, raw::image::WEAPON_CORE_VTABLE].contains(&read(raw::active_item::vtable(held.ptr())))
        }) }
    }
    pub fn carry_dirty(&self) {
        // SAFETY: snapshot dirtiness belongs to this live native player.
        unsafe { raw::player::modified(self.this.ptr()).write_volatile(1) }
    }
    pub fn carry_filter(&self, actions: u32) {
        // SAFETY: set_input has copied the packet to the player's own input.
        // Never edit the shared packet or its compressed/replay representation.
        unsafe { raw::player::actions(self.this.ptr()).write_volatile(actions) }
    }
    pub fn carry_owned(&self, artefact: Artefact) -> bool {
        self.inventory().is_some_and(|i| i.owns_passive(artefact))
    }
    pub fn carry_team(&self) -> Option<u8> {
        // SAFETY: the live player's retained profile.
        unsafe {
            let profile = pointer(raw::player::profile(self.this.ptr()))?;
            match read(raw::player_profile::team(profile.ptr())) { 0 => Some(0), 1 => Some(1), _ => None }
        }
    }
    pub fn carry_eye(&self) -> ([f32; 3], [f32; 3]) {
        // SAFETY: computed eye is a complete native 4x4 matrix.
        let m: [f32; 16] = unsafe { floats(raw::player::eye(self.this.ptr())) };
        ([m[12], m[13], m[14]], [m[8], m[9], m[10]])
    }
    /// The core world owns the active kit. Release only the extra reference
    /// returned by the lookup, as NetworkClient::player does.
    pub fn carry_peer(&self, place: Place) -> Option<Player<'_>> {
        let mut out = 0;
        // SAFETY: own live world and a bounded roster place. Native lookup
        // verifies membership and returns the world's current kit, not a cache.
        unsafe {
            let world = pointer(raw::player::world_core(self.this.ptr()))?;
            raw::engine_core_active_player(world.ptr(), u32::from(place.get()), &mut out);
            let this = Object::at(out)?;
            let refs = raw::player::references(this.ptr());
            if AtomicU32::from_ptr(refs).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_object(this.ptr(), refs);
                return None;
            }
            Some(Player { this, thread: self.thread })
        }
    }
}

pub fn player<'t>(ctx: &'t Context<'_>, reg: Reg) -> Option<Player<'t>> { Player::at(ctx, reg) }

pub fn local_player<'t>(ctx: &'t Context<'_>) -> Option<Player<'t>> {
    let stack = ctx.reg(Reg::Esp).wrapping_add(8) as *mut c_void;
    // SAFETY: update_player_icons entry argument and its live client world.
    unsafe {
        let ui = pointer(raw::update_player_icons_frame::ui(stack))?;
        let world = pointer(raw::game_world_ui::world(ui.ptr()))?;
        let game = pointer(raw::game_world::game(world.ptr()))?;
        let client = pointer(raw::game::network_client(game.ptr()))?;
        Some(Player { this: pointer(raw::network_client::local_player(client.ptr()))?, thread: ctx.thread() })
    }
}

pub fn incoming(ctx: &Context<'_>) -> Option<(Time, u32)> {
    let stack = ctx.reg(Reg::Esp).wrapping_add(8) as *mut c_void;
    // SAFETY: set_input's entry, before its prologue; frame locations are
    // catalogued against the original function and the generated lohook ABI.
    unsafe {
        let input = pointer(raw::input_entry_frame::input(stack))?;
        Some((Time(read(raw::input_entry_frame::time(stack))), read(raw::native_player_input::actions(input.ptr()))))
    }
}

pub fn constructed(ctx: &Context<'_>) -> u32 {
    let stack = ctx.reg(Reg::Esp).wrapping_add(8) as *mut c_void;
    // SAFETY: base_player constructor entry first argument, identity only.
    unsafe { read(raw::base_player_ctor_frame::player(stack)) }
}

pub fn write_snapshot(ctx: &Context<'_>, carry: Carry) {
    write_bytes(ctx, Reg::Edi, &carry.snapshot());
}
pub(super) fn write_bytes(ctx: &Context<'_>, register: Reg, bytes: &[u8]) {
    let writer = ctx.reg(register) as *mut c_void;
    for &byte in bytes {
        // SAFETY: base_player::serialize's live writer, same native write
        // helper as the adjacent fields. It grows the packet as necessary.
        unsafe { raw::engine_write_u8((&raw const byte).cast(), writer,
            c"artefact_carry".as_ptr().cast(), 1, c"serialize".as_ptr().cast(), c"carry".as_ptr().cast()) }
    }
}

pub fn read_snapshot(ctx: &Context<'_>) -> Option<Carry> {
    Carry::restore(read_bytes(ctx, Reg::Esi)?)
}
pub(super) fn read_bytes<const N: usize>(ctx: &Context<'_>, register: Reg) -> Option<[u8; N]> {
    let reader = ctx.reg(register) as *mut c_void;
    // SAFETY: base_player::deserialize's live reader. Check the complete
    // extension against its allocation before copying or advancing it.
    unsafe {
        let begin = read(raw::buffer_reader::begin(reader));
        let cursor = read(raw::buffer_reader::cursor(reader));
        let size = read(raw::buffer_reader::size(reader));
        let next = cursor.checked_add(N as u32)?;
        if cursor < begin || next > begin.checked_add(size)? { return None; }
        let bytes = (cursor as *const [u8; N]).read_unaligned();
        raw::buffer_reader::cursor(reader).write(next);
        Some(bytes)
    }
}

/// Valid model-space bounds, read from the held static model's resource.
/// The largest dimension fits above a palm; small flowers are never enlarged.
struct HeldShape { base: [f32; 3], scale: f32 }
impl HeldShape {
    fn parse(bounds: [f32; 6]) -> Option<Self> {
        if !bounds.iter().all(|v| v.is_finite()) { return None; }
        let size: [f32; 3] = core::array::from_fn(|n| bounds[n + 3] - bounds[n]);
        if size.iter().any(|v| *v < 0.0) { return None; }
        let largest = size[0].max(size[1]).max(size[2]);
        if largest <= 0.0 { return None; }
        // Anchor the bottom of the resource, not its centre: tall Onyx
        // must clear the fingertips just as a small flower does.
        Some(Self { base: [(bounds[0] + bounds[3]) * 0.5, bounds[1], (bounds[2] + bounds[5]) * 0.5],
            scale: (0.14 / largest).min(0.65) })
    }
    fn at(&self, palm: [f32; 16]) -> [f32; 16] {
        let mut result = palm;
        for column in 0..3 {
            for row in 0..3 {
                result[row * 4 + column] *= self.scale;
                result[12 + column] -= self.base[row] * result[row * 4 + column];
            }
        }
        result
    }
    unsafe fn for_model(model: Object) -> Option<Self> {
        // SAFETY: all four native artefact cooks produce static model holders.
        // The retained held flower has the same type. get_aabb copies the
        // original resource's local bounds, not last frame's world bounds.
        unsafe {
            let instance = pointer(raw::player_model::render_instance(model.ptr()))?;
            let mut bounds = [0.0; 6];
            raw::engine_static_model_bounds(instance.ptr(), bounds.as_mut_ptr());
            Self::parse(bounds)
        }
    }
}

/// The renderer calls queue a copy and retain both resource references.
pub(super) struct Scene { renderer: Object, reference: *mut c_void }
impl Scene {
    pub(super) unsafe fn of_world(world: Object) -> Option<Self> {
        // SAFETY: live client world from a client callback/draw argument.
        unsafe {
            let game = pointer(raw::game_world::game(world.ptr()))?;
            let renderer = pointer(raw::game::renderer(game.ptr()))?;
            Some(Self { renderer: pointer(raw::renderer::scene_renderer(renderer.ptr()))?, reference: raw::game_world::scene(world.ptr()).cast() })
        }
    }
    unsafe fn present(&self, model: Object, matrix: &[f32; 16]) {
        // SAFETY: model holder and scene are retained by the live caller.
        unsafe {
            let reference = raw::player_model::render_instance(model.ptr());
            let Some(instance) = pointer(reference) else { return };
            if read(raw::render_model_instance::in_scene(instance.ptr())) == 0 {
                raw::engine_add_model(self.renderer.ptr(), reference.cast(), self.reference, matrix.as_ptr(), matrix.as_ptr());
            } else {
                raw::engine_update_model(self.renderer.ptr(), reference.cast(), self.reference, matrix.as_ptr(), matrix.as_ptr());
            }
        }
    }
    pub(super) unsafe fn remove(&self, model: Object) -> bool {
        // SAFETY: as present; guard native remove with its own in-scene bit.
        unsafe {
            let reference = raw::player_model::render_instance(model.ptr());
            let Some(instance) = pointer(reference) else { return false };
            if read(raw::render_model_instance::in_scene(instance.ptr())) == 0 { return false; }
            raw::engine_remove_model(self.renderer.ptr(), reference.cast(), self.reference);
            true
        }
    }
}

#[derive(Clone, Copy)]
pub enum WeaponVisibility {
    Native,
    Hidden { weapon: u32, models: u8 },
}

/// A live native on_hide argument, which remains valid even if the player
/// already selected a different weapon. Never dereference a cached identity.
pub struct WeaponHiding { weapon: Object }
impl WeaponHiding {
    pub fn at(ctx: &Context<'_>) -> Option<Self> {
        let stack = ctx.reg(Reg::Esp).wrapping_add(8) as *mut c_void;
        // SAFETY: weapon::on_hide entry, exact native frame and live this.
        unsafe {
            if read(raw::weapon_hide_frame::was_visible(stack)) == 0 { return None; }
            Some(Self { weapon: Object::at(ctx.reg(Reg::Ecx))? })
        }
    }
    pub fn restore(&self, state: &mut WeaponVisibility) {
        let WeaponVisibility::Hidden { weapon, models: removed } = *state else { return };
        if weapon != self.weapon.id() { return; }
        // SAFETY: on_hide owns this weapon, its models and client world.
        // Restore the original native model set; on_hide immediately removes
        // it through its normal queue. Both commands precede rendering.
        unsafe {
            let Some(world) = pointer(raw::weapon::world(self.weapon.ptr())) else { return };
            let Some(scene) = Scene::of_world(world) else { return };
            let optics = pointer(raw::weapon::optics(self.weapon.ptr()));
            let models = [pointer(raw::weapon::model(self.weapon.ptr())),
                optics.and_then(|o| pointer(raw::weapon_optics::model(o.ptr()))),
                optics.and_then(|o| pointer(raw::weapon_optics::aim_model(o.ptr())))];
            let matrix = floats(raw::weapon::transform(self.weapon.ptr()));
            for (n, model) in models.into_iter().enumerate() {
                if removed & (1 << n) != 0 && let Some(model) = model { scene.present(model, &matrix); }
            }
            *state = WeaponVisibility::Native;
        }
    }
}

/// Sole owner of the temporary render removal. Store identities only; on
/// restoration resolve the currently held weapon again before touching it.
pub fn weapon_visibility(ctx: &Context<'_>, hide: bool, state: &mut WeaponVisibility) {
    let Some(player) = Player::at(ctx, Reg::Ebx) else { return };
    // SAFETY: player_draw's live player, client weapon parsed by exact vtable.
    unsafe {
        let Some(weapon) = pointer(raw::player::held_item(player.this.ptr())) else { return };
        if read(raw::active_item::vtable(weapon.ptr())) != raw::image::WEAPON_VTABLE { *state = WeaponVisibility::Native; return; }
        let Some(world) = pointer(raw::player::game_world(player.this.ptr())) else { return };
        let Some(scene) = Scene::of_world(world) else { return };
        let optics = pointer(raw::weapon::optics(weapon.ptr()));
        let models = [pointer(raw::weapon::model(weapon.ptr())),
            optics.and_then(|o| pointer(raw::weapon_optics::model(o.ptr()))),
            optics.and_then(|o| pointer(raw::weapon_optics::aim_model(o.ptr())))];
        match (*state, hide) {
            (previous, true) => {
                let mut removed = match previous {
                    WeaponVisibility::Hidden { weapon: previous, models } if previous == weapon.id() => models,
                    _ => 0,
                };
                for (n, model) in models.into_iter().enumerate() {
                    if model.is_some_and(|model| scene.remove(model)) { removed |= 1 << n; }
                }
                *state = WeaponVisibility::Hidden { weapon: weapon.id(), models: removed };
            }
            (WeaponVisibility::Hidden { weapon: previous, models: removed }, false) if previous == weapon.id() => {
                let matrix = (ctx.reg(Reg::Eax) as *const [f32; 16]).read_unaligned();
                for (n, model) in models.into_iter().enumerate() {
                    if removed & (1 << n) != 0 && let Some(model) = model { scene.present(model, &matrix); }
                }
                *state = WeaponVisibility::Native;
            }
            (_, false) => *state = WeaponVisibility::Native,
        }
    }
}

#[derive(Clone, Copy)]
pub enum ArtefactClass { Spring, Lifebone, Onyx, Rattle }

pub enum DrawCommand { Present([f32; 16]), Keep, Native }

pub struct ArtefactDraw { callback: Object, item: Object }
impl ArtefactDraw {
    pub(super) fn of_item(item: Object) -> Option<Self> {
        // SAFETY: a live inventory artefact. Exact client vtables distinguish
        // the four callback layouts from core-only server objects.
        let offset = unsafe { match read(raw::active_item::vtable(item.ptr())) {
            raw::image::SPRING_VTABLE => 0x398,
            raw::image::LIFEBONE_VTABLE => 0x498,
            raw::image::ONYX_VTABLE => 0xF20,
            raw::image::RATTLE_VTABLE => 0x340,
            _ => return None,
        }};
        Some(Self { item, callback: Object::at(item.id().checked_add(offset)?)? })
    }

    pub fn at(ctx: &Context<'_>, class: ArtefactClass) -> Option<Self> {
        let callback = Object::at(ctx.reg(Reg::Ecx))?;
        // SAFETY: class is fixed by the four distinct callback hook sites.
        let item = unsafe { match class {
            ArtefactClass::Spring => raw::artefact_spring_client::base(callback.ptr()),
            ArtefactClass::Lifebone => raw::artefact_lifebone_client::base(callback.ptr()),
            ArtefactClass::Onyx => raw::artefact_onyx_client::base(callback.ptr()),
            ArtefactClass::Rattle => raw::artefact_rattle_client::base(callback.ptr()),
        }};
        Some(Self { callback, item: Object::at(item as u32)? })
    }
    pub fn id(&self) -> u32 { self.item.id() }
    pub fn ground(&self, ctx: &Context<'_>) -> bool {
        let Some(item) = super::carry_ground::GroundItem::of_item(self.item, ctx.thread()) else { return false };
        // SAFETY: native item world owns its simulation clock.
        let now = unsafe { pointer(raw::item::world(self.item.ptr()))
            .map(|w| Time(read(raw::core_world::time(w.ptr())))) };
        let Some(position) = now.and_then(|now| item.position(now)) else { return false };
        unsafe {
            let Some(world) = pointer(raw::artefact_client::world(self.callback.ptr())) else { return false };
            let Some(scene) = Scene::of_world(world) else { return false };
            let Some(model) = pointer(raw::artefact_client::model(self.callback.ptr())) else { return false };
            let cached = raw::artefact_client::cached_state(self.callback.ptr());
            if read(cached) == 5 {
                super::carry_flower::with_model(ctx.thread(), self.item.id(), model, |held| { scene.remove(held); });
            }
            let Some(instance) = pointer(raw::player_model::render_instance(model.ptr())) else { return false };
            let mut bounds = [0.; 6];
            raw::engine_static_model_bounds(instance.ptr(), bounds.as_mut_ptr());
            if !bounds.iter().all(|v| v.is_finite()) { return false; }
            let mut matrix = [1.,0.,0.,0., 0.,1.,0.,0., 0.,0.,1.,0., 0.,0.,0.,1.];
            matrix[12] = position[0] - (bounds[0] + bounds[3]) * 0.5;
            matrix[13] = position[1] - bounds[1];
            matrix[14] = position[2] - (bounds[2] + bounds[5]) * 0.5;
            scene.present(model, &matrix);
            cached.write(6);
        }
        true
    }
    /// True: this callback was fully handled. False: native ground/active
    /// presentation must run. Its state transitions still own particles.
    pub fn update(&self, ctx: &Context<'_>, command: DrawCommand) -> bool {
        // SAFETY: the original callback owns its item/model/world throughout.
        unsafe {
            let cached = raw::artefact_client::cached_state(self.callback.ptr());
            let state = read(raw::artefact::state(self.item.ptr()));
            let Some(world) = pointer(raw::artefact_client::world(self.callback.ptr())) else { return false };
            let Some(scene) = Scene::of_world(world) else { return false };
            let Some(model) = pointer(raw::artefact_client::model(self.callback.ptr())) else { return false };
            if read(cached) == 6 { scene.remove(model); cached.write(0); }
            super::carry_flower::with_model(ctx.thread(), self.item.id(), model, |held| {
                if read(cached) == 5 && (state != 3 || matches!(command, DrawCommand::Native)) {
                    scene.remove(held);
                    cached.write(3);
                }
                if state == 3 && read(cached) == 5 && matches!(command, DrawCommand::Keep) { return true; }
                if state == 3 && matches!(read(cached), 3 | 5)
                    && let DrawCommand::Present(matrix) = command
                    && let Some(shape) = HeldShape::for_model(held) {
                        scene.present(held, &shape.at(matrix)); cached.write(5); return true;
                    }
                false
            })
        }
    }
}

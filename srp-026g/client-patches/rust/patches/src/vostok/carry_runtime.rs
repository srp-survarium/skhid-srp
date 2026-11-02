//! Native boundaries for carry mode. IDs kept by the patch are comparison
//! values; every dereference starts with a live hook argument or world lookup.

use core::sync::atomic::{AtomicU32, Ordering};
use artefact_carry::{Artefact, Place};
use artefact_carry::carry::{Carry, Time};
use super::Context;
use super::world::{Player, floats, read};
use crate::raw;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key { pub object: u32, pub world: u32, pub spawn: u32, pub index: usize }

impl Player<'_> {
    pub fn carry_key(&self) -> Option<Key> {
        // SAFETY: this is a live base_player. No client-only fields here.
        unsafe {
            let place = Place::parse(read(self.this.place()))?;
            let kit = read(self.this.kit());
            if kit >= 3 { return None; }
            let world = self.this.world_core()?;
            Some(Key { object: self.this.address(), world: world.address(), spawn: self.spawned(), index: usize::from(place.get()) * 3 + usize::from(kit) })
        }
    }
    pub fn carry_time(&self) -> Time {
        // SAFETY: live player's simulation clock.
        Time(unsafe { read(self.this.time()) })
    }
    pub fn carry_allowed(&self) -> bool {
        if !self.alive() || !self.inserted() { return false; }
        // SAFETY: only known weapon interfaces can enter carry mode.
        unsafe { self.this.held_item().is_some_and(|held| {
            [raw::image::WEAPON_VTABLE, raw::image::WEAPON_CORE_VTABLE].contains(&read(held.vtable()))
        }) }
    }
    pub fn carry_filter(&self, actions: u32) {
        // SAFETY: set_input has copied the packet to the player's own input.
        // Never edit the shared packet or its compressed/replay representation.
        unsafe { self.this.actions().write_volatile(actions) }
    }
    pub fn carry_owned(&self, artefact: Artefact) -> bool {
        self.inventory().is_some_and(|i| i.owns_passive(artefact))
    }
    pub fn carry_team(&self) -> Option<u8> {
        // SAFETY: the live player's retained profile.
        unsafe {
            let profile = self.this.profile()?;
            match read(profile.team()) { 0 => Some(0), 1 => Some(1), _ => None }
        }
    }
    pub fn carry_eye(&self) -> ([f32; 3], [f32; 3]) {
        // SAFETY: computed eye is a complete native 4x4 matrix.
        let m: [f32; 16] = unsafe { floats(self.this.eye()) };
        ([m[12], m[13], m[14]], [m[8], m[9], m[10]])
    }
    /// The core world owns the active kit. Release only the extra reference
    /// returned by the lookup, as NetworkClient::player does.
    pub fn carry_peer(&self, place: Place) -> Option<Player<'_>> {
        let mut out = None;
        // SAFETY: own live world and a bounded roster place. Native lookup
        // verifies membership and returns the world's current kit, not a cache.
        unsafe {
            let world = self.this.world_core()?;
            raw::engine_core_active_player(Some(world), u32::from(place.get()), &mut out);
            let this = out?;
            let refs = this.references();
            if AtomicU32::from_ptr(refs).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_object(Some(this), refs);
                return None;
            }
            Some(Player { this, thread: self.thread })
        }
    }
}

pub enum PlayerSite { Input, Filter, Tick, Save, Load, Weapon, Death }
pub fn player<'t>(ctx: &'t Context<'_>, site: PlayerSite) -> Option<Player<'t>> {
    // SAFETY: each caller names the hook whose live player it borrows.
    let raw = unsafe { match site {
        PlayerSite::Input => raw::site::carry_input::player(ctx),
        PlayerSite::Filter => raw::site::carry_filter::player(ctx),
        PlayerSite::Tick => raw::site::carry_tick::player(ctx),
        PlayerSite::Save => raw::site::carry_save::player(ctx),
        PlayerSite::Load => raw::site::carry_load::player(ctx),
        PlayerSite::Weapon => raw::site::carry_weapon::player(ctx),
        PlayerSite::Death => raw::site::carry_death::player(ctx),
    }};
    Player::of(ctx, raw)
}

pub fn local_player<'t>(ctx: &'t Context<'_>) -> Option<Player<'t>> {
    // SAFETY: update_player_icons entry argument and its live client world.
    unsafe {
        let ui = raw::site::player_icons_update_start::frame(ctx)?.ui()?;
        let client = ui.world()?.game()?.network_client()?;
        Some(Player { this: client.local_player()?, thread: ctx.thread() })
    }
}

pub fn incoming(ctx: &Context<'_>) -> Option<(Time, u32)> {
    // SAFETY: set_input's entry frame, before its prologue.
    unsafe {
        let frame = raw::site::carry_input::frame(ctx)?;
        Some((Time(read(frame.time())), read(frame.input()?.actions())))
    }
}

/// The carry key's event at player_input_handler::on_keyboard_action (site
/// carry.key): the event of the key bound to `action`, `None` for another
/// action or a state the engine does not define. Every key event passes
/// here, its up too, whatever the input builder does after.
pub fn key_event(ctx: &Context<'_>, action: super::Action) -> Option<super::KeyboardAction> {
    // SAFETY: the site's typed registers: EDI the binder's action, EBP the
    // function's frame (its third argument, the key's state, at +10h).
    let (bound, frame) = unsafe { (raw::site::carry_key::action(ctx), raw::site::carry_key::frame(ctx)?) };
    if bound != action.0 { return None; }
    // SAFETY: the frame's argument slot, read once.
    super::KeyboardAction::parse(unsafe { read(frame.state()) })
}

pub fn constructed(ctx: &Context<'_>) -> u32 {
    // SAFETY: constructor's destination is an identity, never dereferenced.
    unsafe { raw::site::carry_construct::frame(ctx).map_or(0, |f| read(f.player())) }
}

pub fn write_snapshot(ctx: &Context<'_>, carry: Carry) {
    // SAFETY: base_player::serialize's epilogue, its live writer.
    if let Some(writer) = unsafe { raw::site::carry_save::writer(ctx) } { write_bytes(writer, &carry.snapshot()); }
}
pub(super) fn write_bytes(writer: raw::BufferWriter, bytes: &[u8]) {
    for &byte in bytes {
        // SAFETY: a serialize's live writer, same native write helper as
        // the adjacent fields. It grows the packet as necessary.
        unsafe { raw::engine_write_u8((&raw const byte).cast(), Some(writer),
            c"artefact_carry".as_ptr().cast(), 1, c"serialize".as_ptr().cast(), c"carry".as_ptr().cast()) }
    }
}

pub fn read_snapshot(ctx: &Context<'_>) -> Option<Carry> {
    // SAFETY: live deserialize reader at the player-load site.
    Carry::restore(read_bytes(unsafe { raw::site::carry_load::reader(ctx) }?)?)
}
pub(super) fn read_bytes<const N: usize>(reader: raw::BufferReader) -> Option<[u8; N]> {
    // SAFETY: base_player::deserialize's live reader. Check the complete
    // extension against its allocation before copying or advancing it.
    unsafe {
        let begin = reader.begin() as usize;
        let cursor = reader.cursor() as usize;
        let size = read(reader.size()) as usize;
        let next = cursor.checked_add(N)?;
        if cursor < begin || next > begin.checked_add(size)? { return None; }
        let bytes = (cursor as *const [u8; N]).read_unaligned();
        reader.set_cursor(next as *mut u8);
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
    unsafe fn for_model(model: raw::SceneModel) -> Option<Self> {
        // SAFETY: all four native artefact cooks produce static model holders.
        // The retained held flower has the same type. get_aabb copies the
        // original resource's local bounds, not last frame's world bounds.
        unsafe {
            // Static: the artefact cooks' and the held flower's models.
            let instance = model.render_instance().instance()?.static_model();
            let mut bounds = [0.0; 6];
            raw::engine_static_model_bounds(Some(instance), bounds.as_mut_ptr());
            Self::parse(bounds)
        }
    }
}

/// The renderer calls queue a copy and retain both resource references.
pub(super) struct Scene { renderer: raw::SceneRenderer, reference: raw::SceneReference }
impl Scene {
    pub(super) unsafe fn of_world(world: raw::GameWorld) -> Option<Self> {
        // SAFETY: live client world from a client callback/draw argument.
        unsafe {
            let game = world.game()?;
            let renderer = game.renderer()?;
            Some(Self { renderer: renderer.scene_renderer()?, reference: world.scene() })
        }
    }
    pub(super) unsafe fn present(&self, model: raw::SceneModel, matrix: &[f32; 16]) {
        // SAFETY: model holder and scene are retained by the live caller.
        unsafe {
            let reference = model.render_instance();
            let Some(instance) = reference.instance() else { return };
            if read(instance.in_scene()) == 0 {
                raw::engine_add_model(Some(self.renderer), Some(reference), Some(self.reference), matrix.as_ptr(), matrix.as_ptr());
            } else {
                raw::engine_update_model(Some(self.renderer), Some(reference), Some(self.reference), matrix.as_ptr(), matrix.as_ptr());
            }
        }
    }
    /// Whether the model is in the scene (its render instance's own bit).
    pub(super) unsafe fn has(model: raw::SceneModel) -> bool {
        // SAFETY: a model holder retained by the live caller.
        unsafe { model.render_instance().instance().is_some_and(|instance| read(instance.in_scene()) != 0) }
    }
    /// A skinned model's bones (model space, rows), as weapon::draw gives
    /// its own after update_model (0x244F5A): update_skeleton copies them.
    pub(super) unsafe fn skin(&self, model: raw::SceneModel, bones: &[[f32; 16]]) {
        let count = bones.len().min(super::draw::MAX_BONES) as u32;
        // SAFETY: the model holder retained by the live caller, our
        // matrices, `count` of them, copied into the command.
        unsafe {
            raw::engine_update_skeleton(Some(self.renderer), Some(model.skinned()), bones.as_ptr().cast(),
                                        bones.as_ptr().cast(), count)
        }
    }
    pub(super) unsafe fn remove(&self, model: raw::SceneModel) -> bool {
        // SAFETY: as present; guard native remove with its own in-scene bit.
        unsafe {
            let reference = model.render_instance();
            let Some(instance) = reference.instance() else { return false };
            if read(instance.in_scene()) == 0 { return false; }
            raw::engine_remove_model(Some(self.renderer), Some(reference), Some(self.reference));
            true
        }
    }
}

/// The client weapons whose models carry took out of the scene, by weapon
/// identity (compared, never dereferenced): each one's removed models
/// (bit n of [`weapon_models`]). A record leaves only when they go back.
pub struct HiddenWeapons([Option<Hidden>; 64]);
#[derive(Clone, Copy)]
struct Hidden { weapon: u32, models: u8 }
impl HiddenWeapons {
    pub const NEW: Self = Self([None; 64]);
    fn find(&mut self, weapon: u32) -> Option<&mut Option<Hidden>> {
        self.0.iter_mut().find(|h| h.is_some_and(|h| h.weapon == weapon))
    }
    /// Its record, or a new one; None when all are taken (then nothing is hidden).
    fn entry(&mut self, weapon: u32) -> Option<&mut Option<Hidden>> {
        if self.find(weapon).is_none() {
            *self.0.iter_mut().find(|h| h.is_none())? = Some(Hidden { weapon, models: 0 });
        }
        self.find(weapon)
    }
}

/// A client weapon's models: the weapon's, its optics' and aimed optics'.
unsafe fn weapon_models(weapon: raw::Weapon) -> [Option<raw::SceneModel>; 3] {
    // SAFETY: a live client weapon (caller).
    unsafe {
        let optics = weapon.optics();
        [weapon.model(), optics.and_then(|o| o.model()), optics.and_then(|o| o.aim_model())]
    }
}

/// A live native on_hide argument, which remains valid even if the player
/// already selected a different weapon. Never dereference a cached identity.
pub struct WeaponHiding { weapon: raw::Weapon }
impl WeaponHiding {
    pub fn at(ctx: &Context<'_>) -> Option<Self> {
        // SAFETY: weapon::on_hide entry, exact native frame and live this.
        unsafe {
            if read(raw::site::carry_weapon_hide::frame(ctx)?.was_visible()) == 0 { return None; }
            Some(Self { weapon: raw::site::carry_weapon_hide::weapon(ctx)? })
        }
    }
    pub fn restore(&self, hidden: &mut HiddenWeapons) {
        let Some(record) = hidden.find(self.weapon.address()) else { return };
        let Some(Hidden { models: removed, .. }) = *record else { return };
        // SAFETY: on_hide owns this weapon, its models and client world.
        // Restore the original native model set; on_hide immediately removes
        // it through its normal queue. Both commands precede rendering.
        unsafe {
            let Some(world) = self.weapon.world() else { return };
            let Some(scene) = Scene::of_world(world) else { return };
            let matrix = floats(self.weapon.transform());
            for (n, model) in weapon_models(self.weapon).into_iter().enumerate() {
                if removed & (1 << n) != 0 && let Some(model) = model { scene.present(model, &matrix); }
            }
        }
        *record = None;
    }
}

/// Sole owner of the temporary render removal: `hide` takes the held
/// weapon's models out, otherwise any it took go back, at this frame's
/// transform. True when they are out (the native draw must be skipped).
pub fn weapon_visibility(ctx: &Context<'_>, hide: bool, hidden: &mut HiddenWeapons) -> bool {
    let Some(player) = player(ctx, PlayerSite::Weapon) else { return false };
    // SAFETY: player_draw's live player, client weapon parsed by exact vtable.
    unsafe {
        let Some(weapon) = player.this.held_item() else { return false };
        if read(weapon.vtable()) != raw::image::WEAPON_VTABLE { return false; }
        let weapon = weapon.weapon();
        let Some(world) = player.this.game_world() else { return false };
        let Some(scene) = Scene::of_world(world) else { return false };
        let models = weapon_models(weapon);
        if hide {
            let Some(record) = hidden.entry(weapon.address()) else { return false };
            let Some(Hidden { models: removed, .. }) = record else { return false };
            for (n, model) in models.into_iter().enumerate() {
                if model.is_some_and(|model| scene.remove(model)) { *removed |= 1 << n; }
            }
            return true;
        }
        let Some(record) = hidden.find(weapon.address()) else { return false };
        let Some(Hidden { models: removed, .. }) = *record else { return false };
        let Some(transform) = raw::site::carry_weapon::transform(ctx) else { return false };
        let matrix = floats(transform.value());
        for (n, model) in models.into_iter().enumerate() {
            if removed & (1 << n) != 0 && let Some(model) = model { scene.present(model, &matrix); }
        }
        *record = None;
        false
    }
}

#[derive(Clone, Copy)]
pub enum ArtefactClass { Spring, Lifebone, Onyx, Rattle }

pub enum DrawCommand { Present([f32; 16]), Keep, Native }

pub struct ArtefactDraw { callback: raw::ArtefactClient, item: raw::Artefact }
impl ArtefactDraw {
    pub(super) fn of_item(item: raw::Artefact) -> Option<Self> {
        // SAFETY: a live inventory artefact. Exact vtables select its callback.
        let callback = unsafe { match read(item.item().vtable()) {
            raw::image::SPRING_VTABLE => item.spring_callback(),
            raw::image::LIFEBONE_VTABLE => item.lifebone_callback(),
            raw::image::ONYX_VTABLE => item.onyx_callback(),
            raw::image::RATTLE_VTABLE => item.rattle_callback(),
            _ => return None,
        }};
        Some(Self { item, callback })
    }

    pub fn at(ctx: &Context<'_>, class: ArtefactClass) -> Option<Self> {
        // SAFETY: class names the exact callback site, whose offsets are in raw.
        let (callback, item) = unsafe { match class {
            ArtefactClass::Spring => { let c = raw::site::carry_spring::callback(ctx)?; (c.callback(), c.base()) },
            ArtefactClass::Lifebone => { let c = raw::site::carry_lifebone::callback(ctx)?; (c.callback(), c.base()) },
            ArtefactClass::Onyx => { let c = raw::site::carry_onyx::callback(ctx)?; (c.callback(), c.base()) },
            ArtefactClass::Rattle => { let c = raw::site::carry_rattle::callback(ctx)?; (c.callback(), c.base()) },
        }};
        Some(Self { callback, item })
    }
    pub fn id(&self) -> u32 { self.item.address() }
    pub fn ground(&self, ctx: &Context<'_>) -> bool {
        let Some(item) = super::carry_ground::GroundItem::of_item(self.item, ctx.thread()) else { return false };
        // SAFETY: native item world owns its simulation clock.
        let now = unsafe { self.item.item().world()
            .map(|w| Time(read(w.time()))) };
        let Some(position) = now.and_then(|now| item.position(now)) else { return false };
        unsafe {
            let Some(world) = self.callback.world() else { return false };
            let Some(scene) = Scene::of_world(world) else { return false };
            let Some(model) = self.callback.model() else { return false };
            let cached = self.callback.cached_state();
            if read(cached) == 5 {
                super::carry_flower::with_model(ctx.thread(), self.item.address(), model, |held| { scene.remove(held); });
            }
            let Some(instance) = model.render_instance().instance().map(|instance| instance.static_model()) else { return false };
            let mut bounds = [0.; 6];
            raw::engine_static_model_bounds(Some(instance), bounds.as_mut_ptr());
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
            let cached = self.callback.cached_state();
            let state = read(self.item.state());
            let Some(world) = self.callback.world() else { return false };
            let Some(scene) = Scene::of_world(world) else { return false };
            let Some(model) = self.callback.model() else { return false };
            if read(cached) == 6 { scene.remove(model); cached.write(0); }
            super::carry_flower::with_model(ctx.thread(), self.item.address(), model, |held| {
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

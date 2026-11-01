//! Patch 90's game side: a client weapon's muzzle device, a static model
//! drawn at the weapon model's `muzzle_point` locator
//! (`docs/notes/weapon-kit.md`, "The muzzle device"). Where the sites meet
//! the weapon (sites_026g.tsv `muzzle.*`):
//!
//! - the cook asks for the device's model with the weapon's own
//!   sub-resources: [`requests`] at `muzzle.request_room` (one more place in
//!   the requests' alloca) and [`push_request`] at `muzzle.request` (the
//!   request, last), [`loaded`] at `muzzle.loaded` (the last result kept,
//!   the weapon's `muzzle_point` read);
//! - each draw: [`locate`] at `muzzle.locate` (the device's frame by
//!   calculate_locator, the shot's flash moved to its fire point) and
//!   [`mirror`] at `muzzle.draw` (in the scene exactly while the weapon's
//!   model is); [`remove`] when the weapon is culled or hidden;
//! - [`release`] when the weapon goes (weapon_cook::delete_resource).
//!
//! One owner, [`DEVICES`]: the weapons with a device, by address (compared,
//! never dereferenced), each holding one reference to its model, given back
//! once (the slot's [`Model`] drops). Artefact carry reads it through
//! [`model_of`] (its fourth model of a weapon).

use core::ffi::CStr;
use core::sync::atomic::{AtomicBool, Ordering};

use super::carry_flower::Model;
use super::carry_runtime::Scene;
use super::config::{ConfigTable, ConfigValue};
use super::context::{Context, Reg};
use super::log::{Piece, line};
use super::world::{read, write_floats};
use super::{Lock, Thread};
use crate::raw;

/// A static model's resource class (the flower's, the scope cook's).
const STATIC_MODEL: u32 = 15;
/// The weapons at once with a device: the lobby's few, a match's players'.
const SLOTS: usize = 256;
/// Requests pushed whose results have not come yet.
const PENDING: usize = 32;
/// The weapon model's locator the device goes to (0.69d0's, which the
/// content adds to every gun model that takes devices).
const MUZZLE_POINT: &CStr = c"muzzle_point";
/// calculate_locator's last matrix, applied first: draw's own (0x33390).
const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// A device's fire point: millimetres along +z of its model, from its
/// origin (the content's `*_fire_mm`, an int: type-3 modifiers add ints).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Millimetres(pub i32);

impl Millimetres {
    pub fn metres(self) -> f32 {
        self.0 as f32 / 1000.0
    }
}

/// A weapon's device as its config names it.
#[derive(Clone, Copy, Debug)]
pub struct Device<'t> {
    /// The static model's path (a class-15 request's).
    pub model: &'t CStr,
    pub fire: Millimetres,
}

/// A weapon of ours: its device's model and where it goes.
struct Fitted {
    weapon: u32,
    model: Model,
    muzzle: raw::LocatorStorage,
    fire: Millimetres,
    /// The device's frame of the last draw (calculate_locator's), None
    /// before the first.
    frame: Option<[f32; 16]>,
}

// SAFETY: a Fitted crosses the game's threads (the cook's callback, mt_draw's
// tasks, the resource's deletion) only inside DEVICES' lock; its model holds
// one atomic resource reference, its locator is plain bytes.
unsafe impl Send for Fitted {}

/// A slot of [`DEVICES`]. Its tag first and `Free` 0, so the registry is
/// all zeros and the arena keeps it in `patches.state`, not its data.
#[repr(C, u32)]
enum Slot {
    Free = 0,
    Taken(Fitted),
}

struct Devices([Slot; SLOTS]);

impl Devices {
    fn find(&mut self, weapon: u32) -> Option<&mut Fitted> {
        self.0.iter_mut().find_map(|s| match s {
            Slot::Taken(f) if f.weapon == weapon => Some(f),
            _ => None,
        })
    }
    fn take(&mut self, weapon: u32) -> Option<Fitted> {
        let slot = self.0.iter_mut().find(|s| matches!(s, Slot::Taken(f) if f.weapon == weapon))?;
        match core::mem::replace(slot, Slot::Free) {
            Slot::Taken(f) => Some(f),
            Slot::Free => None,
        }
    }
    /// A free slot given `fitted`; `fitted` back (refused) when every slot
    /// is taken.
    fn put(&mut self, fitted: Fitted) -> Option<Fitted> {
        match self.0.iter_mut().find(|s| matches!(s, Slot::Free)) {
            Some(slot) => {
                *slot = Slot::Taken(fitted);
                None
            }
            None => Some(fitted),
        }
    }
}

static DEVICES: Lock<Devices> = Lock::new(Devices([const { Slot::Free }; SLOTS]));

/// A request pushed: the weapon config (its root's address) and its
/// result's index.
#[derive(Clone, Copy)]
struct Pending {
    root: u32,
    index: usize,
}

struct Requests {
    pending: [Option<Pending>; PENDING],
    next: usize,
}

static REQUESTS: Lock<Requests> = Lock::new(Requests { pending: [None; PENDING], next: 0 });

static NO_LOCATOR: AtomicBool = AtomicBool::new(false);
static FULL: AtomicBool = AtomicBool::new(false);

fn once(flag: &AtomicBool, text: &str) {
    if !flag.swap(true, Ordering::Relaxed) {
        line(&[Piece::Text(text)]);
    }
}

/// # Safety
/// `root` is a live weapon config's root, alive for 't.
unsafe fn table<'t>(root: raw::EngineCfg) -> Option<ConfigTable<'t>> {
    // SAFETY: the caller's.
    unsafe { ConfigValue::read(root) }.ok()?.table().ok()
}

/// The weapon config the cook asks a weapon's sub-resources for, at
/// `muzzle.request_room` and `muzzle.request`.
pub fn cook_config<'t>(ctx: &'t Context<'_>, room: bool) -> Option<ConfigTable<'t>> {
    // SAFETY: both sites' EBX is on_weapon_config_loaded's root (0x24A97F),
    // its config retained by the cook ([ebp+8], 0x24A958..0x24ACD5).
    unsafe {
        let root = if room {
            raw::site::muzzle_request_room::root(ctx)
        } else {
            raw::site::muzzle_request::root(ctx)
        }?;
        table(root)
    }
}

/// `muzzle.request_room`: one more place in the requests' alloca (EAX, the
/// count it sizes them by).
pub fn requests(ctx: &mut Context<'_>) {
    // SAFETY: the site's EAX, a count.
    let count = unsafe { raw::site::muzzle_request_room::count(ctx) };
    ctx.set_reg(Reg::Eax, count.wrapping_add(1));
}

/// `muzzle.request`: the device's model asked for last, when the requests
/// have room (the place [`requests`] made); its result's index kept for
/// [`loaded`]. False when it was not asked for.
pub fn push_request(ctx: &Context<'_>, device: &Device<'_>) -> bool {
    // SAFETY: the site's EBP is on_weapon_config_loaded's frame, its
    // requests a live buffer_vector over the cook's alloca; EBX its root.
    unsafe {
        let Some(frame) = raw::site::muzzle_request::frame(ctx) else { return false };
        let Some(root) = raw::site::muzzle_request::root(ctx) else { return false };
        let requests = frame.requests();
        let (Some(begin), Some(end), Some(capacity)) = (requests.begin(), requests.end(), requests.capacity())
        else {
            return false;
        };
        let (Some(index), Some(room)) = (end.index_from(begin), capacity.index_from(begin)) else { return false };
        if index >= room {
            return false;
        }
        end.set_path(device.model.as_ptr().cast_mut().cast());
        end.class().write(STATIC_MODEL);
        requests.set_end(Some(begin.at(index + 1)));
        REQUESTS.with(ctx.thread(), |r| {
            let at = r.next % PENDING;
            r.pending[at] = Some(Pending { root: root.address(), index });
            r.next = r.next.wrapping_add(1);
        });
    }
    true
}

/// The weapon config at `muzzle.loaded` (the cook's callback).
pub fn loaded_config<'t>(ctx: &'t Context<'_>) -> Option<ConfigTable<'t>> {
    // SAFETY: the callback's second argument, the config resource the cook
    // asked for, retained by its functor while it runs.
    unsafe { table(raw::site::muzzle_loaded::frame(ctx)?.config()?.root()?) }
}

/// `muzzle.loaded`: the device's model (the result [`push_request`] asked
/// for) kept for the weapon, with the weapon model's `muzzle_point`. Nothing
/// when that request was not made, its result is missing, or the model has
/// no `muzzle_point` (logged once).
pub fn loaded(ctx: &Context<'_>, fire: Millimetres) {
    // SAFETY: the callback after load_weapon: EDI the weapon it made (its
    // model at +1810h just set, 0x243A9A), [esp+38h] the results, [esp+3Ch]
    // the config, all alive while the callback runs on this thread.
    unsafe {
        let Some(frame) = raw::site::muzzle_loaded::frame(ctx) else { return };
        let Some(weapon) = raw::site::muzzle_loaded::weapon(ctx) else { return };
        let Some(root) = frame.config().and_then(|c| c.root()) else { return };
        let Some(pending) = REQUESTS.with(ctx.thread(), |r| {
            r.pending.iter_mut().find(|p| p.is_some_and(|p| p.root == root.address()))?.take()
        }) else {
            return;
        };
        let Some(results) = frame.results() else { return };
        if read(results.count()) as usize != pending.index + 1 {
            return;
        }
        let Some(model) = results.first().at(pending.index).resource() else { return };
        let Some(instance) = weapon.model().and_then(|m| m.render_instance().instance()) else { return };
        // a weapon's model is a skeleton model's instance (load_weapon reads
        // its locators so, 0x243AC0); its locators are its original model's
        let skeleton = instance.skeleton();
        if read(skeleton.vtable()) != raw::image::SKELETON_INSTANCE_VTABLE {
            return;
        }
        let Some(original) = skeleton.original() else { return };
        let mut muzzle = raw::LocatorStorage::zeroed();
        if raw::engine_model_locator(Some(original), MUZZLE_POINT.as_ptr().cast(), Some(muzzle.handle())) & 0xFF == 0 {
            once(&NO_LOCATOR, "muzzle: a weapon model without muzzle_point: its device is not drawn");
            return;
        }
        let address = weapon.address();
        let fitted = Fitted { weapon: address, model: Model::retain(model), muzzle, fire, frame: None };
        let refused = DEVICES.with(ctx.thread(), |devices| {
            let old = devices.take(address);
            (old, devices.put(fitted))
        });
        // given back outside the lock: an old record of this address, or ours
        // when every slot is taken
        if refused.1.is_some() {
            once(&FULL, "muzzle: every device slot taken: a weapon's device is not drawn");
        }
        drop(refused);
    }
}

/// `muzzle.locate`: the weapon's device frame of this draw, by `frame` from
/// calculate_locator's; the shot's flash (+620h, barrel_point's frame this
/// draw) becomes `flash` of it.
pub fn locate(ctx: &Context<'_>, flash: impl FnOnce(&[f32; 16], Millimetres) -> [f32; 16]) {
    // SAFETY: weapon::draw's EBX and EBP: the weapon it draws and its frame
    // ([ebp+8] the world matrix, [ebp+0Ch] the bones, which draw gives
    // calculate_locator itself at 0x244E61), alive for the draw. +620h was
    // written this draw (0x244E68), read after this (0x244F61).
    unsafe {
        let Some(weapon) = raw::site::muzzle_locate::weapon(ctx) else { return };
        let Some(frame) = raw::site::muzzle_locate::frame(ctx) else { return };
        let (world, bones) = (frame.world(), frame.bones());
        DEVICES.with(ctx.thread(), |devices| {
            let Some(fitted) = devices.find(weapon.address()) else { return };
            let mut out = [0.0f32; 16];
            raw::engine_calculate_locator(Some(fitted.muzzle.handle()), world, bones, IDENTITY.as_ptr(),
                                          out.as_mut_ptr());
            fitted.frame = Some(out);
            write_floats(weapon.barrel(), flash(&out, fitted.fire));
        });
    }
}

/// The weapon model's scene membership (its render instance's in-scene
/// byte, 0x244EE7).
///
/// # Safety
/// `weapon` is a live client weapon.
unsafe fn shown(weapon: raw::Weapon) -> bool {
    // SAFETY: the caller's; its model holder and instance are its own.
    unsafe { weapon.model().and_then(|m| m.render_instance().instance()).is_some_and(|i| read(i.in_scene()) != 0) }
}

/// `muzzle.draw`: the device in the scene, at its frame, exactly while the
/// weapon's model is (draw added or removed it with the aimed scope).
pub fn mirror(ctx: &Context<'_>) {
    // SAFETY: weapon::draw's EBX, the weapon, and its world's scene, which
    // draw itself submits to (0x244EDB..).
    unsafe {
        let Some(weapon) = raw::site::muzzle_draw::weapon(ctx) else { return };
        let Some(scene) = weapon.world().and_then(|w| Scene::of_world(w)) else { return };
        let shown = shown(weapon);
        DEVICES.with(ctx.thread(), |devices| {
            let Some(fitted) = devices.find(weapon.address()) else { return };
            match (shown, fitted.frame) {
                (true, Some(frame)) => scene.present(fitted.model.get(), &frame),
                _ => {
                    scene.remove(fitted.model.get());
                }
            }
        });
    }
}

/// Where a weapon leaves the scene with its device.
pub enum Leaving {
    /// on_culled_player_draw (`muzzle.culled`): at the player's root.
    Culled,
    /// on_hide (`muzzle.hidden`).
    Hidden,
}

/// The device out of the scene.
pub fn remove(ctx: &Context<'_>, at: Leaving) {
    // SAFETY: each site's weapon register, the live weapon its function
    // works on, and its world's scene, which that function submits to.
    unsafe {
        let weapon = match at {
            Leaving::Culled => raw::site::muzzle_culled::weapon(ctx),
            Leaving::Hidden => raw::site::muzzle_hidden::weapon(ctx),
        };
        let Some(weapon) = weapon else { return };
        let Some(scene) = weapon.world().and_then(|w| Scene::of_world(w)) else { return };
        DEVICES.with(ctx.thread(), |devices| {
            if let Some(fitted) = devices.find(weapon.address()) {
                scene.remove(fitted.model.get());
            }
        });
    }
}

/// `muzzle.release`: the weapon goes. Its device leaves the scene if it is
/// still there (while the world's scene lives, as the flower's), and its
/// reference is given back.
pub fn release(ctx: &Context<'_>) {
    // SAFETY: delete_resource's argument, the weapon's +10h (0 none), which
    // the cook deletes after this; the slot's model is ours until it drops.
    unsafe {
        let Some(resource) = raw::site::muzzle_release::frame(ctx).and_then(|f| f.resource()) else { return };
        let weapon = resource.weapon();
        let Some(fitted) = DEVICES.with(ctx.thread(), |devices| devices.take(weapon.address())) else { return };
        let model = fitted.model.get();
        let in_scene = model.render_instance().instance().is_some_and(|i| read(i.in_scene()) != 0);
        if in_scene
            && let Some(world) = weapon.world()
            && read(world.scene().value()) != 0
            && let Some(scene) = Scene::of_world(world)
        {
            scene.remove(model);
        }
        drop(fitted);
    }
}

/// The weapon's device model, for artefact carry's hiding (the weapon is
/// the one the player draws: its slot is not released while it draws).
pub(super) fn model_of(thread: &Thread, weapon: raw::Weapon) -> Option<raw::SceneModel> {
    DEVICES.with(thread, |devices| devices.find(weapon.address()).map(|f| f.model.get()))
}


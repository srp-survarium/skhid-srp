//! Patch 161's game side: where a client weapon keeps what its parts, scope,
//! eye, fire point, left hand and shells stand on
//! (`docs/notes/attachments.md`, "The client's runtime").
//!
//! Most of it is data the game fills once from the weapon's model and reads
//! each frame; the patch writes it again after the game did:
//!
//! - the fire point's, the scope's and the eye's locator records (weapon
//!   +6E0h, +744h, +7A8h), filled by `load_weapon` (`attachments.loaded` is
//!   right after it);
//! - the hand IK's records of the left hand (weapon +1A20h, eight), filled
//!   by `weapon::activate` (`attachments.hands`);
//! - the finger corrector's sets of the left hand (weapon +20F0h, four),
//!   filled by `load_weapon` and again whenever the user's view changes
//!   (`attachments.fingers_view`);
//! - the frame the shell effect plays at, which is the scope's
//!   (`attachments.shells`).
//!
//! The parts are models of their own, asked for by the weapon cook's own
//! query and drawn with the weapon's model:
//!
//! - the cook's requests ([`more_requests`] at `attachments.request_room`,
//!   [`request_parts`] at `attachments.asked`): the bare body in place of
//!   the weapon's model, and each part's model after the game's own;
//! - their results ([`Asked::hold`] at `attachments.ready`), kept with each
//!   part's placement;
//! - each draw ([`draw`] at `attachments.draw`): every part at its frame, in
//!   the scene exactly while the weapon's model is; out of it when the
//!   weapon is culled or hidden ([`leave`]);
//! - [`released`] when the weapon goes.
//!
//! One owner, [`FITTED`]: the weapons with something fitted, by address
//! (compared, never read through), each [`Fit`] holding one reference to
//! every part's model, given back once.
//!
//! The modules table ([`modules`]) is game data, one file of raw bytes
//! ([`TABLE`]). It comes with the items dictionary's own configs: the
//! dictionary cook's query asks for it as a sixth request ([`ask`],
//! `attachments.dictionary_requests`) and its callback takes it before the
//! dictionary is made ([`arrived`], `attachments.dictionary_configs`). A
//! weapon's config is named by a dictionary entry, so the table is here
//! before any weapon is cooked. Without the file (or with one that does not
//! parse: a line says why) every weapon stays as the game made it.

use core::ffi::CStr;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use weapon_modules::{Affine, BoneIndex, DictId, FINGERS, Malformed, Placement, Table, Wanted};

use super::config::{ConfigTable, ConfigValue};
use super::context::{Context, Reg};
use super::carry_flower::Model;
use super::carry_runtime::Scene;
use super::gun_cook::{allocator, free};
use super::hook::Handle;
use super::log::{Piece, line};
use super::world::{floats, read, write_floats};
use super::{Lock, Thread};
use crate::raw;

/// The modules table's file as the game asks for it: the archive's
/// `gameplay/weapon_modules.srpm` under the requests' `resources/`, as the
/// dictionary's own configs (0xA0E8CC).
pub const TABLE: &CStr = c"resources/gameplay/weapon_modules.srpm";
/// resources::raw_data_class: a file's bytes as they are (the game's own
/// startup.cfg, game::query_base_resources 0x1EC182).
const RAW_DATA: u32 = 3;
/// The dictionary cook's own requests (translate_query 0x2C8963).
const DICTIONARY_CONFIGS: usize = 5;
const LABEL: &CStr = c"srp weapon modules";

/// The weapons with something fitted at once: the lobby's few, a match's
/// players' and the pickups' copies.
const SLOTS: usize = 256;
/// A static model's resource class (a scope's idle model, the flower's).
const STATIC_MODEL: u32 = 15;
/// A skeleton model's: the weapon's own `object/model` request
/// (on_weapon_config_loaded 0x24AB04).
const SKELETON_MODEL: u32 = 16;
/// calculate_locator's last matrix, applied first: weapon::draw's own.
const IDENTITY: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];

/// A weapon by its address: compared, never read through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponId(u32);

/// What a weapon config names: its gun and what is fitted to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Named {
    pub gun: DictId,
    pub wanted: Wanted,
}

/// What the hand, finger and shell sites need of a fitted weapon.
#[derive(Clone, Copy)]
pub struct Kept {
    pub named: Named,
    /// The shells' frame as a matrix times the scope's, when the scope's
    /// frame moved.
    pub shells: Option<Affine>,
}

/// A part in the scene's terms: its model, one reference of ours, and the
/// locator record its frame is worked out from each draw.
struct Drawn {
    model: Model,
    at: raw::WeaponLocatorStorage,
}

/// A weapon's parts: memory of the game's allocator, freed with them.
struct Parts {
    first: core::ptr::NonNull<Drawn>,
    /// The parts written so far, `room` at most.
    count: usize,
    room: usize,
}

impl Parts {
    fn with_room(room: usize) -> Option<Parts> {
        let bytes = u32::try_from(room.max(1).checked_mul(core::mem::size_of::<Drawn>())?).ok()?;
        // SAFETY: the game's allocator, alive from before any cook.
        let memory = unsafe { raw::engine_malloc(allocator()?, bytes, LABEL.as_ptr().cast()) };
        Some(Parts { first: core::ptr::NonNull::new(memory.cast::<Drawn>())?, count: 0, room })
    }

    /// False when there is no room left: `part` is dropped.
    fn push(&mut self, part: Drawn) -> bool {
        if self.count >= self.room {
            return false;
        }
        // SAFETY: room for `room` parts, `count` of them written.
        unsafe { self.first.add(self.count).write(part) };
        self.count += 1;
        true
    }

    fn all(&mut self) -> &mut [Drawn] {
        // SAFETY: `count` parts written, ours alone (inside FITTED's lock).
        unsafe { core::slice::from_raw_parts_mut(self.first.as_ptr(), self.count) }
    }
}

impl Drop for Parts {
    fn drop(&mut self) {
        // SAFETY: the written parts dropped once (each model's reference
        // given back), then the memory, the allocator's own.
        unsafe {
            core::ptr::drop_in_place(self.all());
            free(self.first.as_ptr() as usize as u32);
        }
    }
}

/// A fitted weapon, from the cook's requests to its release.
enum Fit {
    /// The cook asked for its parts' models: `count` requests from `first`
    /// of its query.
    Asked { named: Named, first: usize, count: usize },
    /// The models came and are held.
    Held { kept: Kept, parts: Parts },
}

// SAFETY: a Fit crosses the game's threads (the cook's callbacks, mt_draw's
// tasks, the resource's deletion) only inside FITTED's lock; its models are
// atomic resource references, its memory the game's allocator's.
unsafe impl Send for Fit {}

/// A place of [`FITTED`]. Its tag first and `Free` 0, so the registry is
/// all zeros and the arena keeps it in `patches.state`.
#[repr(C, u32)]
enum Slot {
    Free = 0,
    Taken(WeaponId, Fit),
}

struct Fitted([Slot; SLOTS]);

impl Fitted {
    fn find(&mut self, weapon: WeaponId) -> Option<&mut Fit> {
        self.0.iter_mut().find_map(|slot| match slot {
            Slot::Taken(id, fit) if *id == weapon => Some(fit),
            _ => None,
        })
    }

    fn take(&mut self, weapon: WeaponId) -> Option<Fit> {
        let slot = self.0.iter_mut().find(|slot| matches!(slot, Slot::Taken(id, _) if *id == weapon))?;
        match core::mem::replace(slot, Slot::Free) {
            Slot::Taken(_, fit) => Some(fit),
            Slot::Free => None,
        }
    }

    fn has_room(&self) -> bool {
        self.0.iter().any(|slot| matches!(slot, Slot::Free))
    }

    /// `fit` back when every place is taken.
    fn put(&mut self, weapon: WeaponId, fit: Fit) -> Option<Fit> {
        match self.0.iter_mut().find(|slot| matches!(slot, Slot::Free)) {
            Some(slot) => {
                *slot = Slot::Taken(weapon, fit);
                None
            }
            None => Some(fit),
        }
    }
}

static FITTED: Lock<Fitted> = Lock::new(Fitted([const { Slot::Free }; SLOTS]));
static MODULES: Lock<Option<&'static Table<'static>>> = Lock::new(None);
/// The matrix the shell effect is handed (`attachments.shells`): the game
/// reads it inside play_weapon_shell_pfx, on the thread that wrote it.
static SHELLS: Lock<[f32; 16]> = Lock::new([0.0; 16]);

static NOT_ASKED: AtomicBool = AtomicBool::new(false);
static NOT_HELD: AtomicBool = AtomicBool::new(false);
static MISFIT: AtomicBool = AtomicBool::new(false);

/// A line the first time `flag` is met: a weapon whose parts cannot be
/// drawn says why, once a kind.
fn once(flag: &AtomicBool, text: &str, number: u32) {
    if !flag.swap(true, Ordering::Relaxed) {
        line(&[Piece::Text(text), Piece::Dec(number)]);
    }
}

/// The modules table, once game data gave one: kept for the game's life.
pub fn modules(thread: &Thread) -> Option<&'static Table<'static>> {
    MODULES.with(thread, |table| *table)
}

/// The dictionary cook's requests and the table's after them: what
/// query_resources is handed at `attachments.dictionary_requests` (it copies
/// them before it returns).
static REQUESTS: Lock<[raw::ResourceRequestStorage; DICTIONARY_CONFIGS + 1]> =
    Lock::new([const { raw::ResourceRequestStorage::zeroed() }; DICTIONARY_CONFIGS + 1]);

/// `attachments.dictionary_requests`: the dictionary cook's query asks for
/// its five configs and the modules table (EAX the requests, ECX their
/// count, which the site's own two instructions are skipped for). False
/// when nothing was changed.
pub fn ask(ctx: &mut Context<'_>) -> bool {
    // SAFETY: translate_query's five requests by the site's ESP, filled
    // before it (0x2C88E0..0x2C8942). REQUESTS is read by query_resources
    // after the lock is let go: every call writes the same twelve words (the
    // image's five paths and ours), so a second cook's call changes nothing
    // under the first's query.
    let at = unsafe {
        let Some(frame) = raw::site::attachments_dictionary_requests::frame(ctx) else { return false };
        let own = frame.requests();
        REQUESTS.with(ctx.thread(), |requests| {
            for (index, room) in requests.iter_mut().enumerate().take(DICTIONARY_CONFIGS) {
                let (from, to) = (own.at(index), room.handle());
                to.set_path(from.path());
                *to.class() = read(from.class());
            }
            let table = requests[DICTIONARY_CONFIGS].handle();
            table.set_path(TABLE.as_ptr().cast_mut().cast());
            *table.class() = RAW_DATA;
            requests[0].handle().address()
        })
    };
    ctx.set_reg(Reg::Eax, at);
    ctx.set_reg(Reg::Ecx, DICTIONARY_CONFIGS as u32 + 1);
    true
}

/// Why the game has no modules table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoTable {
    /// The query was not the one [`ask`] changed.
    NotAsked,
    /// The game data has no such file.
    NoFile,
    Empty,
    OutOfMemory,
    Malformed(Malformed),
}

/// `attachments.dictionary_configs`: the table's bytes, the sixth result of
/// the dictionary cook's query, copied into memory of the game's allocator
/// that is never freed, parsed and kept. The first table stays: a dictionary
/// made again finds the same file.
pub fn arrived(ctx: &Context<'_>) -> Result<(), NoTable> {
    if modules(ctx.thread()).is_some() {
        return Ok(());
    }
    // SAFETY: on_configs_loaded's entry: [esp+4] its queries_result, alive
    // for the callback, with as many results as the query had requests; a
    // raw data result is a managed resource or 0. The resource is pinned
    // while its bytes are copied (the game's own way to read one,
    // 0x1EC3D0): the pinned pointer takes one reference, taken here first,
    // and gives it back.
    let bytes: &'static [u8] = unsafe {
        let results = raw::site::attachments_dictionary_configs::frame(ctx).and_then(|frame| frame.results());
        let Some(results) = results.filter(|results| read(results.count()) as usize == DICTIONARY_CONFIGS + 1) else {
            return Err(NoTable::NotAsked);
        };
        let Some(resource) = results.loaded().at(DICTIONARY_CONFIGS).managed() else { return Err(NoTable::NoFile) };
        AtomicU32::from_ptr(resource.references()).fetch_add(1, Ordering::AcqRel);
        let mut pinned = raw::PinnedResourceStorage::zeroed();
        let pinned = pinned.handle();
        raw::engine_pin_resource(Some(pinned), Some(resource));
        let (from, count) = (pinned.bytes(), read(pinned.count()) as usize);
        let copy = match (from.is_null() || count == 0, allocator()) {
            (true, _) => Err(NoTable::Empty),
            (false, None) => Err(NoTable::OutOfMemory),
            (false, Some(allocator)) => {
                let room = raw::engine_malloc(allocator, count as u32, LABEL.as_ptr().cast()).cast::<u8>();
                if room.is_null() {
                    Err(NoTable::OutOfMemory)
                } else {
                    core::ptr::copy_nonoverlapping(from.cast_const(), room, count);
                    Ok(core::slice::from_raw_parts(room.cast_const(), count))
                }
            }
        };
        raw::engine_unpin_resource(Some(pinned));
        copy?
    };
    let table = Table::parse(bytes).map_err(NoTable::Malformed)?;
    // SAFETY: room of the game's allocator for the parsed views, written
    // once and never freed: the reference is good for the game's life.
    let table: &'static Table<'static> = unsafe {
        let room = allocator().map(|allocator| {
            raw::engine_malloc(allocator, core::mem::size_of::<Table<'static>>() as u32, LABEL.as_ptr().cast())
        });
        let Some(room) = room.and_then(|room| core::ptr::NonNull::new(room.cast::<Table<'static>>())) else {
            return Err(NoTable::OutOfMemory);
        };
        room.write(table);
        room.as_ref()
    };
    MODULES.with(ctx.thread(), |kept| *kept = Some(table));
    line(&[Piece::Text("attachments: the modules table, bytes "), Piece::Dec(bytes.len() as u32)]);
    Ok(())
}

/// A line for a table the game data has and the game cannot use.
pub fn refused(why: NoTable) {
    let text = match why {
        // a game without the content: nothing to say
        NoTable::NotAsked | NoTable::NoFile => return,
        NoTable::Empty => "attachments: the modules table is empty",
        NoTable::OutOfMemory => "attachments: no memory for the modules table",
        NoTable::Malformed(Malformed::Magic) => "attachments: the modules table is not one",
        NoTable::Malformed(Malformed::Version(_)) => "attachments: the modules table is of another version",
        NoTable::Malformed(Malformed::Size) => "attachments: the modules table's sections do not fill it",
        NoTable::Malformed(Malformed::Gun { gun, .. }) => {
            return line(&[Piece::Text("attachments: the modules table's gun refused, record "), Piece::Dec(u32::from(gun))]);
        }
        NoTable::Malformed(Malformed::Module { module, .. }) => {
            return line(&[Piece::Text("attachments: the modules table's module refused, record "), Piece::Dec(u32::from(module))]);
        }
    };
    line(&[Piece::Text(text)]);
}

/// What the hand, finger and shell sites need of `weapon`, once its parts
/// are held.
pub fn kept(thread: &Thread, weapon: WeaponId) -> Option<Kept> {
    FITTED.with(thread, |fitted| match fitted.find(weapon) {
        Some(Fit::Held { kept, .. }) => Some(*kept),
        _ => None,
    })
}

/// The shells' frame of a held weapon, worked out once its model is loaded.
pub fn set_shells(thread: &Thread, weapon: WeaponId, shells: Option<Affine>) {
    FITTED.with(thread, |fitted| {
        if let Some(Fit::Held { kept, .. }) = fitted.find(weapon) {
            kept.shells = shells;
        }
    });
}

/// `weapon` is as the game made it from here on: its parts' models given
/// back. Only for a weapon whose parts are not in the scene (one being
/// cooked).
pub fn forget(thread: &Thread, weapon: WeaponId) {
    // dropped outside the lock: a model's last reference runs the game's
    // own release
    drop(FITTED.with(thread, |fitted| fitted.take(weapon)));
}

/// `attachments.asked`: a weapon made anew at an address keeps nothing of
/// the one before it.
pub fn fresh(ctx: &Context<'_>) {
    // SAFETY: the site's EAX is the weapon allocate_weapon just made, or 0.
    if let Some(weapon) = unsafe { raw::site::attachments_asked::weapon(ctx) } {
        forget(ctx.thread(), WeaponId(weapon.address()));
    }
}

/// A weapon whose model has fewer bones than the table says of its gun: it
/// is forgotten and a line says so once (the game's own records stay).
pub fn misfit(thread: &Thread, weapon: WeaponId, named: Named) {
    forget(thread, weapon);
    once(&MISFIT, "attachments: a weapon's model has fewer bones than the table's gun ", u32::from(named.gun.get()));
}

fn dict(data: &ConfigTable<'_>, key: &CStr) -> Option<DictId> {
    let id = data.get(key).ok()??.integer().ok()?;
    DictId::new(u16::try_from(id).ok()?)
}

/// What a weapon config names under `data`: `srp_gun`, the gun's dict id,
/// and `srp_scope`, `srp_muzzle`, `srp_grip`, each a fitted module's dict
/// id or 0. None for a config without `srp_gun`.
///
/// # Safety
/// `root` is a live weapon config's root.
unsafe fn named(root: raw::EngineCfg) -> Option<Named> {
    // SAFETY: the caller's.
    let data = unsafe { ConfigValue::read(root) }.ok()?.table().ok()?.get(c"data").ok()??.table().ok()?;
    let wanted = Wanted { scope: dict(&data, c"srp_scope"), muzzle: dict(&data, c"srp_muzzle"), grip: dict(&data, c"srp_grip") };
    Some(Named { gun: dict(&data, c"srp_gun")?, wanted })
}

/// A site of weapon_cook::on_weapon_config_loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cook {
    /// `attachments.request_room`.
    Room,
    /// `attachments.asked`.
    Asked,
}

/// What the config of the weapon being cooked names, at either site of the
/// cook.
pub fn cooking(ctx: &Context<'_>, site: Cook) -> Option<Named> {
    // SAFETY: both sites' EBX is on_weapon_config_loaded's root (0x24A97F),
    // its config retained by the cook ([ebp+8], 0x24A958..0x24ACD5).
    unsafe {
        named(match site {
            Cook::Room => raw::site::attachments_request_room::root(ctx),
            Cook::Asked => raw::site::attachments_asked::root(ctx),
        }?)
    }
}

/// `attachments.request_room`: `parts` more places in the requests' alloca
/// (EAX, the count it sizes them by).
pub fn more_requests(ctx: &mut Context<'_>, parts: usize) {
    // SAFETY: the site's EAX, a count.
    let count = unsafe { raw::site::attachments_request_room::count(ctx) };
    ctx.set_reg(Reg::Eax, count.wrapping_add(parts as u32));
}

/// `attachments.asked`: the weapon's own model request becomes `body`'s and
/// each of `models` is asked for after the game's requests, in the places
/// [`more_requests`] made; the weapon is remembered with where its parts'
/// results will be. All of it or nothing: with no room in the requests, no
/// model request to change or every place of the registry taken, the cook
/// goes on as the game's own and a line says so once.
pub fn request_parts(ctx: &Context<'_>, named: Named, body: &'static CStr, models: impl Iterator<Item = &'static CStr> + Clone) {
    let parts = models.clone().count();
    // SAFETY: the site's EAX is the weapon allocate_weapon just made, EBP
    // on_weapon_config_loaded's frame: its requests a live buffer_vector
    // over the cook's alloca, read by query_resources after this (0x24AC84),
    // which copies each path; the paths are the table's, kept for the
    // game's life.
    unsafe {
        let Some(weapon) = raw::site::attachments_asked::weapon(ctx).map(|weapon| WeaponId(weapon.address())) else { return };
        let Some(requests) = raw::site::attachments_asked::frame(ctx).map(|frame| frame.requests()) else { return };
        let (Some(begin), Some(end), Some(capacity)) = (requests.begin(), requests.end(), requests.capacity()) else {
            return;
        };
        let (Some(first), Some(room)) = (end.index_from(begin), capacity.index_from(begin)) else { return };
        let model = (0..first).map(|index| begin.at(index)).find(|request| read(request.class()) == SKELETON_MODEL);
        let registered = FITTED.with(ctx.thread(), |fitted| match (model, first.checked_add(parts)) {
            (Some(_), Some(last)) if last <= room && fitted.has_room() => {
                fitted.put(weapon, Fit::Asked { named, first, count: parts }).is_none()
            }
            _ => false,
        });
        let (true, Some(model)) = (registered, model) else {
            return once(&NOT_ASKED, "attachments: a weapon's parts were not asked for, gun ", u32::from(named.gun.get()));
        };
        model.set_path(body.as_ptr().cast_mut().cast());
        for (index, path) in models.enumerate() {
            let request = begin.at(first + index);
            request.set_path(path.as_ptr().cast_mut().cast());
            *request.class() = STATIC_MODEL;
        }
        requests.set_end(Some(begin.at(first + parts)));
    }
}

/// A weapon whose parts' models have come: the cook's callback at its entry
/// (`attachments.ready`).
pub struct Asked<'t> {
    weapon: WeaponId,
    named: Named,
    first: usize,
    count: usize,
    results: raw::QueriesResult,
    thread: &'t Thread,
}

/// `attachments.ready`: the weapon the callback is for, when its cook asked
/// for parts.
pub fn asked<'t>(ctx: &'t Context<'_>) -> Option<Asked<'t>> {
    // SAFETY: the callback's entry (ESP its return address): [esp+4] the
    // queries' results, [esp+0Ch] the weapon weapon_cook made, both alive
    // while the callback runs on this thread.
    let (weapon, results) = unsafe {
        let frame = raw::site::attachments_ready::frame(ctx)?;
        (WeaponId(frame.weapon()?.address()), frame.results()?)
    };
    FITTED.with(ctx.thread(), |fitted| match fitted.find(weapon) {
        Some(Fit::Asked { named, first, count }) => {
            Some(Asked { weapon, named: *named, first: *first, count: *count, results, thread: ctx.thread() })
        }
        _ => None,
    })
}

impl Asked<'_> {
    pub fn named(&self) -> Named {
        self.named
    }

    /// Each part's model taken from the cook's results with a reference of
    /// ours and kept with its placement, in the order they were asked for.
    /// A model that did not come, or other parts than were asked for: the
    /// weapon is forgotten (drawn as its bare body alone) and a line says so
    /// once.
    pub fn hold(self, placements: impl Iterator<Item = Placement>) {
        // SAFETY: the results are alive for the callback, one a request in
        // its order; ours are `count` from `first`, each a static model's
        // holder or 0.
        let parts = unsafe {
            let enough = self.first.checked_add(self.count).is_some_and(|last| read(self.results.count()) as usize == last);
            let mut parts = Parts::with_room(self.count).filter(|_| enough);
            let mut placed = 0;
            for (index, at) in placements.enumerate() {
                placed += 1;
                let model = (index < self.count).then(|| self.results.results().at(self.first + index).model()).flatten();
                let (Some(held), Some(model)) = (&mut parts, model) else {
                    parts = None;
                    continue;
                };
                let mut record = raw::WeaponLocatorStorage::zeroed();
                Records::place(record.handle(), &at);
                held.push(Drawn { model: Model::retain(model), at: record });
            }
            parts.filter(|_| placed == self.count)
        };
        let fit = match parts {
            Some(parts) => Some(Fit::Held { kept: Kept { named: self.named, shells: None }, parts }),
            None => {
                once(&NOT_HELD, "attachments: a weapon's parts did not load, gun ", u32::from(self.named.gun.get()));
                None
            }
        };
        // the place is this weapon's own (Asked): put back as Held, or left free
        let dropped = FITTED.with(self.thread, |fitted| {
            let old = fitted.take(self.weapon);
            (old, fit.and_then(|fit| fitted.put(self.weapon, fit)))
        });
        drop(dropped);
    }
}

/// The weapon model's scene membership (its render instance's in-scene
/// byte, 0x244EE7), and the two bytes that say how it is drawn (the
/// first-person pass, a movable model: weapon::set_foreground 0x243D40,
/// set_movable_static 0x243CB0).
///
/// # Safety
/// `weapon` is a live client weapon.
unsafe fn shown(weapon: raw::Weapon) -> Option<(u8, u8)> {
    // SAFETY: the caller's; its model holder and instance are its own.
    unsafe {
        let instance = weapon.model()?.render_instance().instance()?;
        (read(instance.in_scene()) != 0).then(|| (read(instance.foreground()), read(instance.movable())))
    }
}

/// `attachments.draw`: every part of the weapon in the scene at its frame
/// (the game's calculate_locator on the part's record, as the weapon's own
/// locators), drawn as the weapon's model is, exactly while that model is
/// in the scene (draw added or removed it with the aimed scope).
pub fn draw(ctx: &Context<'_>) {
    // SAFETY: weapon::draw's EBX and EBP: the weapon it draws and its frame
    // ([ebp+8] the world matrix, [ebp+0Ch] the bones, which draw gives
    // calculate_locator itself at 0x244E51), alive for the draw, and its
    // world's scene, which draw itself submits to (0x244EDB..). A part's
    // record names a bone of the weapon's model (Records::loaded checked
    // it).
    unsafe {
        let Some(weapon) = raw::site::attachments_draw::weapon(ctx) else { return };
        let Some(frame) = raw::site::attachments_draw::frame(ctx) else { return };
        let shown = shown(weapon);
        FITTED.with(ctx.thread(), |fitted| {
            let Some(Fit::Held { parts, .. }) = fitted.find(WeaponId(weapon.address())) else { return };
            let Some(scene) = weapon.world().and_then(|world| Scene::of_world(world)) else { return };
            for part in parts.all() {
                let model = part.model.get();
                let Some((foreground, movable)) = shown else {
                    scene.remove(model);
                    continue;
                };
                if let Some(instance) = model.render_instance().instance() {
                    instance.foreground().write(foreground);
                    instance.movable().write(movable);
                }
                let mut at = [0.0f32; 16];
                raw::engine_weapon_locator(at.as_mut_ptr(), Some(part.at.handle()), frame.world(), frame.bones(), IDENTITY.as_ptr());
                scene.present(model, &at);
            }
        });
    }
}

/// Where a weapon leaves the scene with its parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Leaving {
    /// on_culled_player_draw (`attachments.culled`): at the player's root.
    Culled,
    /// on_hide (`attachments.hidden`).
    Hidden,
}

/// The weapon's parts out of the scene.
pub fn leave(ctx: &Context<'_>, at: Leaving) {
    // SAFETY: each site's weapon register, the live weapon its function
    // works on, and its world's scene, which that function submits to.
    unsafe {
        let weapon = match at {
            Leaving::Culled => raw::site::attachments_culled::weapon(ctx),
            Leaving::Hidden => raw::site::attachments_hidden::weapon(ctx),
        };
        let Some(weapon) = weapon else { return };
        let Some(scene) = weapon.world().and_then(|world| Scene::of_world(world)) else { return };
        out_of_scene(ctx.thread(), weapon, &scene);
    }
}

/// `weapon`'s parts out of `scene`. Artefact carry's too: it takes a
/// carrier's gun's models out and skips the gun's draw, which is what keeps
/// the parts with the gun; the next draw of the gun puts them back.
///
/// # Safety
/// `weapon` is a live client weapon and `scene` its world's.
pub(super) unsafe fn out_of_scene(thread: &Thread, weapon: raw::Weapon, scene: &Scene) {
    FITTED.with(thread, |fitted| {
        let Some(Fit::Held { parts, .. }) = fitted.find(WeaponId(weapon.address())) else { return };
        for part in parts.all() {
            // SAFETY: the part's model is held by this place; the scene the caller's.
            unsafe { scene.remove(part.model.get()) };
        }
    });
}

/// Which of the left hand's locators a record or a finger set is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandSet {
    /// What a held gun's clips ask for.
    Idle,
    /// What the lobby's clips ask for.
    Lobby,
}

impl HandSet {
    /// Its finger set, and its record for the first view (the third view's
    /// is four on): set_weapon_locators' and set_weapon_model's order idle,
    /// reload, reload pivot, lobby.
    fn index(self) -> usize {
        match self {
            HandSet::Idle => 0,
            HandSet::Lobby => 3,
        }
    }
}

/// A weapon's records at one of the patch's sites.
pub struct Records<'t> {
    weapon: raw::Weapon,
    thread: &'t Thread,
}

impl<'t> Records<'t> {
    /// `attachments.loaded`: the weapon load_weapon just filled.
    pub fn loaded(ctx: &'t Context<'_>) -> Option<Records<'t>> {
        // SAFETY: the site's EDI is the weapon the cook's callback is for.
        let weapon = unsafe { raw::site::attachments_loaded::weapon(ctx)? };
        Some(Records { weapon, thread: ctx.thread() })
    }

    /// `attachments.hands`: the weapon whose hand records activate just
    /// filled.
    pub fn activated(ctx: &'t Context<'_>) -> Option<Records<'t>> {
        // SAFETY: the site's ESI is activate's weapon.
        let weapon = unsafe { raw::site::attachments_hands::weapon(ctx)? };
        Some(Records { weapon, thread: ctx.thread() })
    }

    /// `attachments.shells`: the weapon whose shell effect plays.
    pub fn ejecting(ctx: &'t Context<'_>) -> Option<Records<'t>> {
        // SAFETY: the site's ESI is play_weapon_shell_pfx's weapon.
        let weapon = unsafe { raw::site::attachments_shells::weapon(ctx)? };
        Some(Records { weapon, thread: ctx.thread() })
    }

    pub fn id(&self) -> WeaponId {
        WeaponId(self.weapon.address())
    }

    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// The bones of the weapon's model (its skeleton render model's inverse
    /// binds, one a bone): a locator record's bone must be below it, the
    /// game indexes the bones by it unchecked. None for a model that is no
    /// skeleton model's instance.
    pub fn bones(&self) -> Option<usize> {
        // SAFETY: the weapon's model holder and render instance are its
        // own, set by load_weapon (0x243A9A); a skeleton instance by its
        // vtable, its original model immutable.
        unsafe {
            let skeleton = self.weapon.model()?.render_instance().instance()?.skeleton();
            if read(skeleton.vtable()) != raw::image::SKELETON_INSTANCE_VTABLE {
                return None;
            }
            let original = skeleton.original()?;
            original.inverse_binds_end()?.index_from(original.inverse_binds()?)
        }
    }

    /// # Safety
    /// `record` is a locator record of this live weapon.
    unsafe fn placement(record: raw::WeaponLocator) -> Placement {
        // SAFETY: the caller's.
        unsafe { Placement { bone: BoneIndex(read(record.bone())), offset: Affine::from_matrix(&floats(record.offset())) } }
    }

    /// # Safety
    /// `record` is a locator record of this live weapon, and `at`'s bone is
    /// one of its model's: the game indexes the bones by it unchecked.
    unsafe fn place(record: raw::WeaponLocator, at: &Placement) {
        // SAFETY: the caller's.
        unsafe {
            write_floats(record.offset(), at.offset.matrix());
            record.bone().write(at.bone.0);
        }
    }

    /// The fire point: weapon::draw evaluates this record into +620h, where
    /// the shot's effects play. `at`'s bone is one of the weapon's model's.
    pub fn set_fire_point(&mut self, at: &Placement) {
        // SAFETY: as set_scope, the barrel record +6E0h.
        unsafe { Self::place(self.weapon.barrel_locator(), at) }
    }

    /// The scope's locator record as it stands.
    pub fn scope(&self) -> Placement {
        // SAFETY: the weapon is the site's, alive; +744h is its own record.
        unsafe { Self::placement(self.weapon.scope_locator()) }
    }

    /// The scope's frame: every reader follows (weapon::draw evaluates the
    /// record into +660h and places the scope's idle or aimed model there).
    /// `at`'s bone is one of the weapon's model's.
    pub fn set_scope(&mut self, at: &Placement) {
        // SAFETY: as scope; the bone is the caller's promise, a gun's of the
        // modules table, whose model this weapon's is.
        unsafe { Self::place(self.weapon.scope_locator(), at) }
    }

    /// The eye: calculate_aim_alignment moves the gun until this record's
    /// frame is on the camera. `at`'s bone is one of the weapon's model's.
    pub fn set_eye(&mut self, at: &Placement) {
        // SAFETY: as set_scope, the aim record +7A8h.
        unsafe { Self::place(self.weapon.aim_locator(), at) }
    }

    /// The left hand's locator of `set`, in both views (the hand IK's
    /// records `set` and `set + 4`). `at`'s bone is one of the weapon's
    /// model's.
    pub fn set_left_hand(&mut self, set: HandSet, at: &Placement) {
        // SAFETY: the left hand's eight records at +1A20h, inside the
        // weapon's own object; the bone as in set_scope.
        unsafe {
            let records = self.weapon.left_hand_locators();
            for view in [0, 4] {
                Self::place(records.at(set.index() + view), at);
            }
        }
    }

    /// The left hand's finger matrices of `set`, as the game made them of
    /// the model's own finger locators.
    pub fn left_fingers(&self, set: HandSet) -> [Affine; FINGERS] {
        // SAFETY: the corrector's left hand's four sets at +20F0h, inside
        // the weapon's own object: fifteen 4x4 each.
        let all: [f32; 240] = unsafe { floats(self.weapon.left_fingers().at(set.index()).matrices()) };
        let mut fingers = [Affine::IDENTITY; FINGERS];
        for (finger, matrix) in fingers.iter_mut().zip(all.as_chunks::<16>().0) {
            *finger = Affine::from_matrix(matrix);
        }
        fingers
    }

    pub fn set_left_fingers(&mut self, set: HandSet, fingers: &[Affine; FINGERS]) {
        let mut all = [0.0f32; 240];
        for (matrix, finger) in all.as_chunks_mut::<16>().0.iter_mut().zip(fingers) {
            *matrix = finger.matrix();
        }
        // SAFETY: as left_fingers.
        unsafe { write_floats(self.weapon.left_fingers().at(set.index()).matrices(), all) }
    }

    /// The scope's frame of the last draw (+660h).
    pub fn scope_frame(&self) -> Affine {
        // SAFETY: the weapon's own matrix, written by weapon::draw.
        Affine::from_matrix(&unsafe { floats(self.weapon.transform()) })
    }
}

/// `attachments.shells`: the shell effect plays at `frame` instead of the
/// scope's (EAX, which the site's own `lea` is skipped for).
pub fn eject_at(ctx: &mut Context<'_>, frame: &Affine) {
    let at = SHELLS.with(ctx.thread(), |matrix| {
        *matrix = frame.matrix();
        matrix.as_ptr()
    });
    ctx.set_reg(Reg::Eax, at as usize as u32);
}

/// `attachments.release`: the weapon weapon_cook frees is forgotten. Its
/// parts leave the scene if they are still there (while the world's scene
/// lives, as the flower's) and their references are given back.
pub fn released(ctx: &Context<'_>) {
    // SAFETY: delete_resource's entry: [esp+4] the weapon's resource base
    // (the weapon + 10h), or 0; the cook deletes the weapon after this.
    unsafe {
        let Some(weapon) = raw::site::attachments_release::frame(ctx).and_then(|frame| frame.resource()).map(|r| r.weapon()) else {
            return;
        };
        let Some(mut fit) = FITTED.with(ctx.thread(), |fitted| fitted.take(WeaponId(weapon.address()))) else { return };
        if let Fit::Held { parts, .. } = &mut fit
            && let Some(world) = weapon.world()
            && read(world.scene().value()) != 0
            && let Some(scene) = Scene::of_world(world)
        {
            for part in parts.all() {
                scene.remove(part.model.get());
            }
        }
        drop(fit);
    }
}

/// The finger corrector `attachments.fingers_view` refills.
pub struct FingerCorrector<'t> {
    native: raw::FingersCorrector,
    thread: &'t Thread,
}

impl<'t> Handle<'t> for FingerCorrector<'t> {
    type Raw = raw::FingersCorrector;

    unsafe fn from_raw(thread: &'t Thread, native: Option<raw::FingersCorrector>) -> Option<Self> {
        Some(FingerCorrector { native: native?, thread })
    }
}

impl<'t> FingerCorrector<'t> {
    pub(in crate::vostok) fn raw(&self) -> raw::FingersCorrector {
        self.native
    }

    /// The weapon it is in.
    pub fn weapon(&self) -> Records<'t> {
        // SAFETY: correct_fingers' corrector is the one weapon_cook built
        // in place inside a weapon (only a weapon's object has one).
        Records { weapon: unsafe { self.native.weapon() }, thread: self.thread }
    }
}

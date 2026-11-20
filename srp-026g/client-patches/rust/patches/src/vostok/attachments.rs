//! Patch 161's game side: where a client weapon keeps what its scope, eye,
//! left hand and shells stand on (`docs/notes/attachments.md`, "The client's
//! runtime"). All of it is data the game fills once from the weapon's model
//! and reads each frame; the patch writes it again after the game did:
//!
//! - the scope's and the eye's locator records (weapon +744h, +7A8h), filled
//!   by `load_weapon` (`attachments.loaded` is right after it);
//! - the hand IK's records of the left hand (weapon +1A20h, eight), filled
//!   by `weapon::activate` (`attachments.hands`);
//! - the finger corrector's sets of the left hand (weapon +20F0h, four),
//!   filled by `load_weapon` and again whenever the user's view changes
//!   (`attachments.fingers_view`);
//! - the frame the shell effect plays at, which is the scope's
//!   (`attachments.shells`).
//!
//! What a weapon is fitted with comes from its config at the cook's
//! callback (`attachments.ready`) and is kept here by the weapon's address
//! ([`FITTED`]) until the weapon goes (`attachments.release`): the hand and
//! finger sites come back long after the cook.
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
use core::sync::atomic::{AtomicU32, Ordering};

use weapon_modules::{Affine, BoneIndex, DictId, FINGERS, Malformed, Placement, Table, Wanted};

use super::config::{ConfigTable, ConfigValue};
use super::context::{Context, Reg};
use super::gun_cook::allocator;
use super::hook::Handle;
use super::log::{Piece, line};
use super::world::{floats, read, write_floats};
use super::{Lock, Thread};
use crate::raw;

/// The modules table's file in the game data.
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

/// A weapon by its address: compared, never read through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeaponId(u32);

/// What a weapon config names: its gun and what is fitted to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Named {
    pub gun: DictId,
    pub wanted: Wanted,
}

/// What is kept of a fitted weapon.
#[derive(Clone, Copy)]
pub struct Kept {
    pub named: Named,
    /// The shells' frame as a matrix times the scope's, when the scope's
    /// frame moved.
    pub shells: Option<Affine>,
}

/// A place of [`FITTED`] or [`PARKED`]. Its tag first and `Free` 0, so the
/// state is all zeros and the arena keeps it in `patches.state`.
#[repr(C, u32)]
#[derive(Clone, Copy)]
enum Slot<T> {
    Free = 0,
    Taken(WeaponId, T),
}

static FITTED: Lock<[Slot<Kept>; SLOTS]> = Lock::new([Slot::Free; SLOTS]);
/// What `attachments.ready` read, until `attachments.loaded` of the same
/// call takes it.
static PARKED: Lock<Slot<Named>> = Lock::new(Slot::Free);
static MODULES: Lock<Option<Table<'static>>> = Lock::new(None);
/// The matrix the shell effect is handed (`attachments.shells`): the game
/// reads it inside play_weapon_shell_pfx, on the thread that wrote it.
static SHELLS: Lock<[f32; 16]> = Lock::new([0.0; 16]);

/// The modules table, once game data gave one.
pub fn modules(thread: &Thread) -> Option<Table<'static>> {
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

pub fn remember(thread: &Thread, weapon: WeaponId, kept: Kept) {
    FITTED.with(thread, |slots| {
        if let Some(slot) = slots.iter_mut().find(|slot| matches!(slot, Slot::Free)) {
            *slot = Slot::Taken(weapon, kept);
        }
    });
}

pub fn kept(thread: &Thread, weapon: WeaponId) -> Option<Kept> {
    FITTED.with(thread, |slots| {
        slots.iter().find_map(|slot| match slot {
            Slot::Taken(id, kept) if *id == weapon => Some(*kept),
            _ => None,
        })
    })
}

pub fn forget(thread: &Thread, weapon: WeaponId) {
    FITTED.with(thread, |slots| {
        for slot in slots.iter_mut() {
            if matches!(slot, Slot::Taken(id, _) if *id == weapon) {
                *slot = Slot::Free;
            }
        }
    });
}

fn dict(data: &ConfigTable<'_>, key: &CStr) -> Option<DictId> {
    let id = data.get(key).ok()??.integer().ok()?;
    DictId::new(u16::try_from(id).ok()?)
}

/// `attachments.ready`: the weapon the cook's callback is for, and what its
/// config names under `data` (`srp_gun`, the gun's dict id, and
/// `srp_scope`, `srp_muzzle`, `srp_grip`, each a fitted module's dict id or
/// 0), parked for [`Records::loaded`]. A config without `srp_gun` parks
/// nothing.
pub fn park(ctx: &Context<'_>) {
    // SAFETY: the callback's entry (ESP its return address): [esp+8] the
    // config's resource the cook asked for, retained by its functor while
    // the callback runs, [esp+0Ch] the weapon weapon_cook made.
    let named = unsafe {
        let Some(frame) = raw::site::attachments_ready::frame(ctx) else { return };
        let (Some(weapon), Some(root)) = (frame.weapon(), frame.config().and_then(|config| config.root())) else {
            return;
        };
        let data = ConfigValue::read(root).ok().and_then(|value| value.table().ok());
        let data = data.and_then(|root| root.get(c"data").ok()??.table().ok());
        data.and_then(|data| {
            let wanted = Wanted { scope: dict(&data, c"srp_scope"), muzzle: dict(&data, c"srp_muzzle"), grip: dict(&data, c"srp_grip") };
            Some(Slot::Taken(WeaponId(weapon.address()), Named { gun: dict(&data, c"srp_gun")?, wanted }))
        })
    };
    PARKED.with(ctx.thread(), |parked| *parked = named.unwrap_or(Slot::Free));
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

    /// What [`park`] read for this weapon in the same callback; taken once.
    pub fn parked(&self) -> Option<Named> {
        let id = self.id();
        PARKED.with(self.thread, |parked| match core::mem::replace(parked, Slot::Free) {
            Slot::Taken(weapon, named) if weapon == id => Some(named),
            _ => None,
        })
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

/// `attachments.release`: the weapon weapon_cook frees.
pub fn released(ctx: &Context<'_>) -> Option<WeaponId> {
    // SAFETY: delete_resource's entry: [esp+4] the weapon's resource base
    // (the weapon + 10h), or 0.
    unsafe { Some(WeaponId(raw::site::attachments_release::frame(ctx)?.resource()?.weapon().address())) }
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

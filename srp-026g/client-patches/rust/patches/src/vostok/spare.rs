//! A spare weapon inventory for a kit player (patch 83's engine side): the
//! player cook's own subresource query (player_cook::on_config_loaded,
//! 0x202500) asked for one inventory more, cooked from a copy of the
//! player's profile that keeps only the slots it is given (the weapons and
//! their ammunition). The native code never reads that last result (it reads
//! results 0..13 and, for the lobby, 14: fields_026g.tsv `queries_result`), so
//! the player is made as before, only once the spare is made too, and the
//! native load and the late join, which wait for the player, wait for it.
//!
//! [`prepare`] makes the profile copy and the recipe, [`submit`] queries the
//! native requests and the spare's in one batch (the native call skipped),
//! [`take`] keeps the spare's inventory from the batch's result, a [`Spare`]
//! owns it (and the profile copy) until it is dropped. What to keep and when
//! is the patch's.

use core::ffi::{c_void, CStr};
use core::sync::atomic::{AtomicU32, Ordering};
use super::world::read;
use super::{Context, Reg, Thread};
use crate::raw;

/// A player cook's stage, by its address: an id, never read through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageId(u32);

/// A kit player by what it is made from: its profile's address and its kit,
/// as the stage has them and the player keeps them (+6B8h, +115h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KitId {
    profile: u32,
    kit: u8,
}

/// A profile's kit, 0..2 (late join's three, `kit_count` 3 at most).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kit(u8);

impl Kit {
    pub fn new(kit: u8) -> Option<Kit> {
        (kit < 3).then_some(Kit(kit))
    }

    pub fn number(self) -> u8 {
        self.0
    }
}

/// An inventory slot of a profile kit, 0..22.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot(u8);

impl Slot {
    pub const COUNT: u8 = 23;

    pub const fn new(slot: u8) -> Option<Slot> {
        if slot < Self::COUNT { Some(Slot(slot)) } else { None }
    }
}

/// What a player cook makes, read at its subresource query.
#[derive(Clone, Copy, Debug)]
pub struct Cook {
    pub stage: StageId,
    pub kit_id: KitId,
    pub place: u8,
    pub kit: Kit,
    /// The lobby's character (the stage's +B1h), not a match's player.
    pub lobby: bool,
}

/// Why no spare was asked for or kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The site's stage, its profile or the native recipe is missing.
    NoCook,
    /// The native request list is empty or longer than ours holds.
    Requests(u32),
    /// No inventory request (class 41h) among the native ones.
    NoInventoryRequest,
    /// The resources' allocator is not there.
    NoAllocator,
    /// The allocator gave nothing.
    OutOfMemory,
    /// The batch's result has another count than the requests asked.
    Count { got: u32, want: u32 },
    /// The spare's result is empty: its cook failed.
    NotMade,
}

impl Refusal {
    pub fn text(self) -> &'static str {
        match self {
            Refusal::NoCook => "no stage, profile or recipe at the query",
            Refusal::Requests(_) => "native request count out of range",
            Refusal::NoInventoryRequest => "no inventory request in the batch",
            Refusal::NoAllocator => "no resources allocator",
            Refusal::OutOfMemory => "out of memory",
            Refusal::Count { .. } => "the result's count is not the batch's",
            Refusal::NotMade => "the spare inventory's cook failed (empty result)",
        }
    }
}

const INVENTORY_CLASS: u32 = 0x41;
/// Requests a batch may have, ours included (the native player cook asks 15 or 16).
const MAX_REQUESTS: usize = 32;
const LABEL: &CStr = c"srp spare weapons";
const EMPTY_SLOT: u16 = 0xFFFF;

/// The resources' allocator (the global every query passes).
fn allocator() -> Option<*mut c_void> {
    // SAFETY: an image VA (the global at 0x1F1D9C4), set before any resource
    // is queried; a cook runs only after.
    let allocator = unsafe { raw::image::allocator().allocator() };
    (!allocator.is_null()).then_some(allocator.cast())
}

/// Frees memory of the allocator's (its address), as the game does.
fn free(address: u32) {
    let Some(allocator) = allocator() else { return };
    let mut pointer = address as usize as *mut c_void;
    // SAFETY: memory the same allocator gave (engine_malloc), freed once.
    unsafe { raw::engine_free(allocator, pointer, &mut pointer) };
}

/// An inventory recipe of ours (1Ch bytes of the allocator's), freed on drop.
struct Recipe(raw::InventoryRecipe);

impl Recipe {
    fn new() -> Result<Recipe, Refusal> {
        let allocator = allocator().ok_or(Refusal::NoAllocator)?;
        // SAFETY: the game's allocator, the recipe's size, a C string for its log.
        unsafe { raw::engine_allocate_recipe(allocator, raw::InventoryRecipe::SIZE as u32, LABEL.as_ptr().cast()) }
            .map(Recipe).ok_or(Refusal::OutOfMemory)
    }
}

impl Drop for Recipe {
    fn drop(&mut self) {
        free(self.0.address());
    }
}

/// A profile of ours: constructed, a copy of the player's, its kit's slots
/// other than those kept emptied; destroyed and freed on drop.
struct ProfileCopy(raw::PlayerProfile);

impl ProfileCopy {
    /// # Safety
    /// `source` is a live constructed profile (the cook's, on its thread).
    unsafe fn new(source: raw::PlayerProfile, kit: Kit, keep: &[Slot]) -> Result<ProfileCopy, Refusal> {
        let allocator = allocator().ok_or(Refusal::NoAllocator)?;
        // SAFETY: the game's allocator gives 4634h bytes; the constructor
        // fills them (it cannot fail: a null result is only a null input).
        let memory = unsafe { raw::engine_malloc(allocator, raw::PlayerProfile::SIZE as u32, LABEL.as_ptr().cast()) };
        if memory.is_null() {
            return Err(Refusal::OutOfMemory);
        }
        let Some(profile) = (unsafe { raw::engine_profile_construct(memory) }) else {
            free(memory as usize as u32);
            return Err(Refusal::OutOfMemory);
        };
        let copy = ProfileCopy(profile);
        // SAFETY: operator= makes ours a deep copy of the live source (its
        // self-relative buffers rebuilt, check_spare_weapons_026g.py); then
        // our kit's slot words (23 a kit) are ours to empty.
        unsafe {
            raw::engine_profile_assign(Some(copy.0), Some(source));
            let slots = copy.0.kit_slots().at(usize::from(kit.0)).slots();
            for slot in 0..Slot::COUNT {
                if !keep.contains(&Slot(slot)) {
                    *slots.at(usize::from(slot)).item() = EMPTY_SLOT;
                }
            }
        }
        Ok(copy)
    }
}

impl Drop for ProfileCopy {
    fn drop(&mut self) {
        // SAFETY: our constructed profile, destroyed this once, then its memory freed.
        unsafe { raw::engine_profile_destroy(Some(self.0)) };
        free(self.0.address());
    }
}

/// A spare being cooked: its recipe and profile copy, and which result is it.
pub struct Pending {
    _recipe: Recipe,
    profile: ProfileCopy,
    index: u32,
}

/// What [`submit`] sends: the native batch and ours, valid in this hook only.
pub struct Submission<'t> {
    ctx_frame: raw::PlayerCookFrame,
    recipe: u32,
    _hook: core::marker::PhantomData<&'t Thread>,
}

/// The cook at the subresource query (site `player_cook.query`).
pub fn cook(ctx: &Context<'_>) -> Option<Cook> {
    // SAFETY: the site's EBP is player_cook::on_config_loaded's frame; its
    // stage lives through the cook (allocated 0x2025A8, freed by the player).
    unsafe {
        let frame = raw::site::player_cook_query::frame(ctx)?;
        let stage = frame.stage()?;
        let profile = stage.profile()?;
        let kit = Kit::new(read(stage.kit()))?;
        Some(Cook {
            stage: StageId(stage.address()),
            kit_id: KitId { profile: profile.address(), kit: kit.0 },
            place: read(stage.place()),
            kit,
            lobby: read(stage.lobby()) != 0,
        })
    }
}

/// Makes the spare's profile copy and recipe for this cook; [`submit`] asks
/// for it. Nothing of the game's changes until then.
pub fn prepare<'t>(ctx: &Context<'t>, keep: &[Slot]) -> Result<(Pending, Submission<'t>), Refusal> {
    // SAFETY: as `cook`; the native recipe and the request list are the
    // frame's (filled 0x20271A..0x2029A0), alive until the call at 0x202A38.
    unsafe {
        let frame = raw::site::player_cook_query::frame(ctx).ok_or(Refusal::NoCook)?;
        let stage = frame.stage().ok_or(Refusal::NoCook)?;
        let source = stage.profile().ok_or(Refusal::NoCook)?;
        let native = frame.recipe().ok_or(Refusal::NoCook)?;
        let kit = Kit::new(read(stage.kit())).ok_or(Refusal::NoCook)?;
        let count = native_count(frame)?;
        inventory_request(frame, count)?;
        let profile = ProfileCopy::new(source, kit, keep)?;
        let recipe = Recipe::new()?;
        // The native recipe's fields, the profile ours.
        let ours = recipe.0;
        *ours.profile() = profile.0.address();
        *ours.kit() = read(native.kit());
        *ours.dictionary() = read(native.dictionary());
        *ours.physics_world() = read(native.physics_world());
        *ours.game_world() = read(native.game_world());
        *ours.match_seed() = read(native.match_seed());
        *ours.flag() = read(native.flag());
        Ok((Pending { _recipe: recipe, profile, index: count as u32 },
            Submission { ctx_frame: frame, recipe: ours.address(), _hook: core::marker::PhantomData }))
    }
}

/// The native requests' count: (end - begin) / 8, within ours.
///
/// # Safety
/// `frame` is the query site's.
unsafe fn native_count(frame: raw::PlayerCookFrame) -> Result<usize, Refusal> {
    let begin = unsafe { frame.requests_begin() }.ok_or(Refusal::Requests(0))?;
    let end = unsafe { read(frame.requests_end()) };
    let bytes = end.wrapping_sub(begin.address());
    let count = bytes as usize / raw::ResourceRequest::SIZE;
    if !(bytes as usize).is_multiple_of(raw::ResourceRequest::SIZE) || count == 0 || count >= MAX_REQUESTS {
        return Err(Refusal::Requests(count as u32));
    }
    Ok(count)
}

/// The native inventory request's index (class 41h, its user data the
/// frame's inventory variant).
///
/// # Safety
/// `frame` is the query site's; `count` its requests'.
unsafe fn inventory_request(frame: raw::PlayerCookFrame, count: usize) -> Result<usize, Refusal> {
    unsafe {
        let requests = frame.requests_begin().ok_or(Refusal::Requests(0))?;
        let users = frame.users().ok_or(Refusal::NoInventoryRequest)?;
        let variant = frame.variant().address();
        (0..count)
            .find(|&n| read(requests.at(n).class()) == INVENTORY_CLASS && read(users.at(n).variant()) == variant)
            .ok_or(Refusal::NoInventoryRequest)
    }
}

/// Queries the native batch with the spare's inventory request last, as the
/// native call at 0x202A38 would have the batch alone, and gives EAX what the
/// code after it reads (0x202A3D): the hook then exits at `queried`. The
/// query copies the requests and their user data before it returns (as for
/// late join's own variants); its completion may run before it returns.
pub fn submit(ctx: &mut Context<'_>, submission: Submission<'_>) {
    let frame = submission.ctx_frame;
    // SAFETY: the frame's request list, user data, callback and parent as
    // `prepare` found them; ours are on this stack for the call.
    unsafe {
        let Ok(count) = native_count(frame) else { return };
        let Ok(inventory) = inventory_request(frame, count) else { return };
        let (Some(begin), Some(users)) = (frame.requests_begin(), frame.users()) else { return };
        let Some(allocator) = allocator() else { return };
        // {path, class} a request and a user data pointer a request, as
        // query_resources takes them (it copies both before it returns);
        // the spare's variant as the native one, its recipe ours.
        let mut requests = [[0u32; 2]; MAX_REQUESTS];
        let mut user_data = [0u32; MAX_REQUESTS];
        let native = frame.variant();
        let mut variant = [0u32; raw::InventoryVariant::SIZE / 4];
        variant[0] = read(native.vtable());
        variant[2] = submission.recipe;
        variant[10] = variant.as_ptr() as usize as u32;
        variant[11] = read(native.type_id());
        for n in 0..count {
            requests[n] = [begin.at(n).path() as usize as u32, read(begin.at(n).class())];
            user_data[n] = read(users.at(n).variant());
        }
        requests[count] = [begin.at(inventory).path() as usize as u32, INVENTORY_CLASS];
        user_data[count] = variant.as_ptr() as usize as u32;
        let callback = frame.callback();
        raw::engine_query_resources(requests.as_ptr().cast(), count as u32 + 1,
            callback.cast(), allocator, user_data.as_mut_ptr().cast(),
            read(frame.parent()) as usize as *mut c_void, 1);
        ctx.set_reg(Reg::Eax, read(callback));
    }
}

/// The stage whose batch is in (site `player_cook.ready`).
pub fn ready(ctx: &Context<'_>) -> Option<StageId> {
    // SAFETY: the site is on_subresources_loaded's entry: [esp+8] its stage.
    unsafe { raw::site::player_cook_ready::frame(ctx)?.stage().map(|s| StageId(s.address())) }
}

/// Keeps the spare's inventory from the batch's result (one reference of
/// ours), or says why there is none; the recipe is freed either way.
pub fn take(ctx: &Context<'_>, pending: Pending) -> Result<Spare, Refusal> {
    // SAFETY: the site's [esp+4] is the live queries_result, alive through
    // the callback; the count is checked before the result is read.
    unsafe {
        let result = raw::site::player_cook_ready::frame(ctx).and_then(|f| f.result()).ok_or(Refusal::NotMade)?;
        let got = read(result.count());
        let want = pending.index + 1;
        if got != want {
            return Err(Refusal::Count { got, want });
        }
        let inventory = result.results().at(pending.index as usize).inventory().ok_or(Refusal::NotMade)?;
        AtomicU32::from_ptr(inventory.references()).fetch_add(1, Ordering::AcqRel);
        let Pending { profile, .. } = pending;
        Ok(Spare { inventory, _profile: profile })
    }
}

/// A spare inventory of ours: one reference to it and the profile it was
/// cooked from, released on drop (the inventory first).
pub struct Spare {
    inventory: raw::Inventory,
    _profile: ProfileCopy,
}

impl Spare {
    /// Its address, for the log.
    pub fn address(&self) -> u32 {
        self.inventory.address()
    }
}

impl Drop for Spare {
    fn drop(&mut self) {
        // SAFETY: the reference `take` added, given back this once; at zero
        // the native release (0x0E0210) destroys it, as for any resource.
        unsafe {
            let references = self.inventory.references();
            if AtomicU32::from_ptr(references).fetch_sub(1, Ordering::AcqRel) == 1 {
                raw::engine_release_inventory(Some(self.inventory), references);
            }
        }
    }
}

/// The kit player being destroyed (site `player.destroy`).
pub fn destroyed(ctx: &Context<'_>) -> Option<KitId> {
    // SAFETY: player::~player's entry, ECX the player, not torn down yet.
    unsafe {
        let player = raw::site::player_destroy::player(ctx)?;
        Some(KitId { profile: player.profile()?.address(), kit: read(player.kit()) })
    }
}

/// The process's address space in use, MiB (GlobalMemoryStatusEx: total
/// virtual less available virtual), and the machine's memory load, percent.
pub fn memory(_: &Thread) -> Option<(u32, u32)> {
    // MEMORYSTATUSEX: dwLength, dwMemoryLoad, then seven u64 (64 bytes).
    let mut status = [0u32; 16];
    status[0] = 64;
    // SAFETY: kernel32's import; it fills the 64 bytes it is given.
    if unsafe { (raw::engine_global_memory_status_ex)(status.as_mut_ptr().cast()) } == 0 {
        return None;
    }
    let total = u64::from(status[10]) | u64::from(status[11]) << 32;
    let available = u64::from(status[12]) | u64::from(status[13]) << 32;
    Some(((total.saturating_sub(available) >> 20) as u32, status[1]))
}

/// Whether the environment variable `name` is `value` (ASCII, any case).
pub fn environment_is(_: &Thread, name: &CStr, value: &[u8]) -> bool {
    let mut buffer = [0u8; 16];
    // SAFETY: kernel32's import; it writes at most the size given.
    let length = unsafe {
        (raw::engine_get_environment_variable_a)(name.as_ptr().cast(), buffer.as_mut_ptr(), buffer.len() as u32)
    } as usize;
    length == value.len() && buffer[..length].eq_ignore_ascii_case(value)
}

/// This process's id (kernel32's GetCurrentProcessId): two local games share
/// one log file, and each line says whose it is.
pub fn process_id() -> u32 {
    // SAFETY: the exe's import, no arguments, no state.
    unsafe { (raw::engine_get_current_process_id)() }
}

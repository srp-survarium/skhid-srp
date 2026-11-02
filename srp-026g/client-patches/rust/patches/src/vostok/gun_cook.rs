//! One gun cooked alone (patch 85's engine side, docs/notes/weapon_pickup.md
//! "The instance"): an inventory (class 41h, the player cook's own request
//! "inventory") cooked from a copy of a player's profile in which one kit
//! keeps only one weapon slot and the ammunition slots. The inventory cook
//! makes a weapon of that slot's item with the ammunition the config binds,
//! as it does for a player; nothing of the cooked inventory is bound to the
//! world.
//!
//! - [`start`] asks for it: the query is the game's `query_resources`
//!   (0xE5B60), its completion a boost member callback (0x27D280) whose
//!   member is [`cooked_entry`] and whose object is the cook's ticket number.
//!   Nothing of ours is held across the query: it may complete before it
//!   returns.
//! - The completion runs on the querying thread, in `dispatch_callbacks`
//!   (0xE5AB0): the game's own between its frames, or [`dispatch_callbacks`]
//!   when a patch waits for a cook.
//! - A [`Gun`] owns its ticket: [`cooked`] says where it is, dropping it
//!   gives everything back (a cook still running is abandoned: its
//!   completion frees what it made).
//! - A made gun can be drawn alone ([`show`]: a drop's gun whose body left
//!   the world); the ticket keeps which of its models it put in the scene,
//!   and they come out ([`hide`]) before the game may show the gun (the
//!   pickup's install and registration call it) and before it is freed (the
//!   ticket's own drop): a model of a freed gun never stays in the scene.
//!
//! What to cook, when, and how long to wait is the patch's.

use core::ffi::{c_void, CStr};
use core::sync::atomic::{AtomicU32, Ordering};

pub use weapon_pickup::{Kit, WeaponSlot};

use super::pickup::{place_lying, remove_lying, LyingGun, Models};
use super::world::{read, Player};
use super::{clock, Lock, Thread};
use crate::raw;

/// Why a gun was not cooked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The player's profile, game world, game, network client, match
    /// options or match core is missing.
    NoContext,
    /// The resources' allocator is not there.
    NoAllocator,
    /// The allocator gave nothing.
    OutOfMemory,
    /// Every ticket is in use.
    NoTicket,
    /// The completion's result is empty or not one result: the cook failed.
    NotMade,
}

impl Refusal {
    pub fn text(self) -> &'static str {
        match self {
            Refusal::NoContext => "no profile, game world, dictionary, match options or match core",
            Refusal::NoAllocator => "no resources allocator",
            Refusal::OutOfMemory => "out of memory",
            Refusal::NoTicket => "every cook ticket is in use",
            Refusal::NotMade => "the inventory's cook failed (empty result)",
        }
    }
}

const INVENTORY_CLASS: u32 = 0x41;
/// The ammunition slots a weapon's config binds to (cook 0x2C7855..0x2C79E5).
const AMMO: [u8; 8] = [8, 9, 11, 12, 19, 20, 21, 22];
const SLOTS: u8 = 23;
const EMPTY_SLOT: u16 = 0xFFFF;
const LABEL: &CStr = c"srp pickup gun";
/// Cooks kept at once (the owner's bound, 2026-11-01): a copy that was ever
/// in hands or registered lives until the match's end (keep and reuse, no
/// frees mid-match: the engine keeps raw pointers to a weapon, e.g. a
/// bullet's +0Ch and +40h, as long as its player lives). Distinct recipes:
/// 20 places x 3 kits x 2 weapon slots = 120 at most (40..60 in practice);
/// a recipe needs another copy only while its copies are all in use, and at
/// most 60 are in use at once (two held a player, one drop a body): 180.
/// 192 so that no pickup is refused for want of a ticket.
pub const TICKETS: usize = 192;

/// The resources' allocator (the global every query passes).
fn allocator() -> Option<*mut c_void> {
    // SAFETY: an image VA (the global at 0x1F1D9C4), set before any resource
    // is queried; a cook runs only after.
    let allocator = unsafe { raw::image::game_allocator().allocator() }?;
    Some(allocator.address() as usize as *mut c_void)
}

/// The bytes allocated in the survarium arena (the cooks') and the ai arena
/// (doug_lea_allocator::allocated_size 0xE5270), for the diagnostics' log.
pub fn arena_usage(_: &Thread) -> (u32, u32) {
    // SAFETY: the image's allocator slots, set before any match; the
    // allocator's own size query.
    unsafe {
        let used = |slot: raw::AllocatorSlot| slot.allocator()
            .map_or(0, |a| raw::engine_allocated_size(a.address() as usize as *mut c_void));
        (used(raw::image::game_allocator()), used(raw::image::ai_allocator()))
    }
}

/// Every allocator's statistics to the game's log (dump_statistics(0)
/// 0xE1370), for the diagnostics.
pub fn dump_statistics(_: &Thread) {
    // SAFETY: the engine's own report, any time on the game's thread.
    unsafe { raw::engine_dump_statistics(0) };
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
/// other than the weapon's and the ammunition's emptied; destroyed and
/// freed on drop.
struct ProfileCopy(raw::PlayerProfile);

impl ProfileCopy {
    /// # Safety
    /// `source` is a live constructed profile (a player's, on its thread).
    unsafe fn new(source: raw::PlayerProfile, kit: Kit, keep: WeaponSlot) -> Result<ProfileCopy, Refusal> {
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
        // self-relative buffers rebuilt, check_weapon_pickup_026g.py); then
        // our kit's slot words (23 a kit) are ours to empty.
        unsafe {
            raw::engine_profile_assign(Some(copy.0), Some(source));
            let slots = copy.0.kit_slots().at(usize::from(kit.get())).slots();
            for slot in 0..SLOTS {
                if slot != keep.slot() && !AMMO.contains(&slot) {
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

/// A cooked inventory of ours: one reference to it and the profile it was
/// cooked from, released on drop (the inventory first), its gun's models
/// taken out of the scene before.
struct CookedInventory {
    inventory: raw::Inventory,
    _profile: ProfileCopy,
    drawn: Drawn,
}

/// What of a made gun is in the scene by us: nothing, or the gun of its
/// inventory's `slot` lying alone at `transform` (`models` the ones placed).
#[derive(Clone, Copy)]
enum Drawn {
    Hidden,
    Lying { slot: WeaponSlot, transform: [f32; 16], models: Models },
}

impl Drawn {
    /// The one transition: `want` drawn (lying there) or not, from what is
    /// drawn now; idempotent, a second hide does nothing.
    fn to(self, inventory: raw::Inventory, want: Option<(WeaponSlot, &LyingGun)>) -> Drawn {
        match (self, want) {
            (Drawn::Hidden, None) => Drawn::Hidden,
            (Drawn::Lying { slot, transform, models }, Some((s, gun))) if slot == s && transform == gun.transform => {
                Drawn::Lying { slot, transform, models }
            }
            (Drawn::Lying { slot, models, .. }, Some((s, gun))) if slot == s => {
                // SAFETY: our live cooked inventory (the caller's ticket).
                Drawn::lying(slot, gun, unsafe { place_lying(inventory, slot, gun, models) })
            }
            (Drawn::Lying { slot, models, .. }, want) => {
                // SAFETY: as above; only the models we placed.
                unsafe { remove_lying(inventory, slot, models) };
                Drawn::Hidden.to(inventory, want)
            }
            (Drawn::Hidden, Some((slot, gun))) => {
                // SAFETY: as above.
                Drawn::lying(slot, gun, unsafe { place_lying(inventory, slot, gun, Models::NONE) })
            }
        }
    }

    fn lying(slot: WeaponSlot, gun: &LyingGun, models: Models) -> Drawn {
        if models == Models::NONE { Drawn::Hidden } else { Drawn::Lying { slot, transform: gun.transform, models } }
    }
}

/// Gives back one reference of ours to an inventory (at zero the native
/// release 0x0E0210 destroys it, as for any resource).
fn release(inventory: raw::Inventory) {
    // SAFETY: a reference this module added, given back this once.
    unsafe {
        let references = inventory.references();
        if AtomicU32::from_ptr(references).fetch_sub(1, Ordering::AcqRel) == 1 {
            raw::engine_release_inventory(Some(inventory), references);
        }
    }
}

impl Drop for CookedInventory {
    fn drop(&mut self) {
        self.drawn = self.drawn.to(self.inventory, None);
        release(self.inventory);
    }
}

/// A ticket: a cook in flight, its owner gone while in flight, or its outcome.
enum Ticket {
    Free,
    /// Asked for; the recipe and the profile live until the completion.
    Waiting { generation: u32, recipe: Recipe, profile: ProfileCopy, since: u64 },
    /// Its [`Gun`] was dropped while in flight: the completion frees all.
    Abandoned { recipe: Recipe, profile: ProfileCopy },
    Made { generation: u32, cooked: CookedInventory, since: u64, at: u64 },
    Failed { generation: u32, since: u64, at: u64 },
}

// SAFETY: a ticket's recipe, profile copy and cooked inventory are this
// module's own (the game's allocator's memory, one inventory reference),
// held by TABLE alone and never handed out; the lock gives them to one
// thread at a time (the cook's issue, its completion on the callbacks'
// thread, the frame's take), and the game's free and release take a call
// from any of them.
unsafe impl Send for Ticket {}

struct Table {
    tickets: [Ticket; TICKETS],
    generation: u32,
}

static TABLE: Lock<Table> = Lock::new(Table { tickets: [const { Ticket::Free }; TICKETS], generation: 0 });

/// A ticket as a value (no ownership): which one and which use of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CookId {
    index: u8,
    generation: u32,
}

/// A cooked gun's inventory as a value (the log's): compared, never read
/// through. Made only from its handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InventoryId(u32);

impl InventoryId {
    fn of(inventory: raw::Inventory) -> InventoryId {
        InventoryId(inventory.address())
    }

    /// Its address, for the log.
    pub fn address(self) -> u32 {
        self.0
    }
}

/// Where a cook is. `since`/`at` are the performance counter's ticks
/// ([`clock::counter`]): asked, and completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cooked {
    Cooking { since: u64 },
    Made { inventory: InventoryId, since: u64, at: u64 },
    Failed { since: u64, at: u64 },
    /// No such cook (given back, or never this generation).
    Gone,
}

/// A cook of ours, owned: dropping it gives the ticket and what it made back.
#[derive(Debug, PartialEq, Eq)]
pub struct Gun {
    id: CookId,
}

impl Gun {
    pub fn id(&self) -> CookId {
        self.id
    }
}

impl Drop for Gun {
    fn drop(&mut self) {
        // SAFETY: a Gun lives in a patch's state, which only hooks reach: it
        // is dropped inside a hook, on the game's thread.
        let thread = unsafe { Thread::new() };
        let id = self.id;
        let freed = TABLE.with(&thread, |table| {
            let ticket = &mut table.tickets[usize::from(id.index)];
            match core::mem::replace(ticket, Ticket::Free) {
                Ticket::Waiting { generation, recipe, profile, .. } if generation == id.generation => {
                    *ticket = Ticket::Abandoned { recipe, profile };
                    None
                }
                Ticket::Made { generation, cooked, .. } if generation == id.generation => Some(cooked),
                Ticket::Failed { generation, .. } if generation == id.generation => None,
                other => {
                    *ticket = other;
                    None
                }
            }
        });
        // Released outside the lock: the inventory's own teardown.
        drop(freed);
    }
}

/// Where the cook `id` is.
pub fn cooked(thread: &Thread, id: CookId) -> Cooked {
    TABLE.with(thread, |table| match &table.tickets[usize::from(id.index)] {
        Ticket::Waiting { generation, since, .. } if *generation == id.generation => Cooked::Cooking { since: *since },
        Ticket::Made { generation, cooked, since, at } if *generation == id.generation => {
            Cooked::Made { inventory: InventoryId::of(cooked.inventory), since: *since, at: *at }
        }
        Ticket::Failed { generation, since, at } if *generation == id.generation => {
            Cooked::Failed { since: *since, at: *at }
        }
        _ => Cooked::Gone,
    })
}

/// The cooked inventory of the made cook `id` (the install's, while its
/// ticket holds it).
pub(in crate::vostok) fn inventory(thread: &Thread, id: CookId) -> Option<raw::Inventory> {
    TABLE.with(thread, |table| match &table.tickets[usize::from(id.index)] {
        Ticket::Made { generation, cooked, .. } if *generation == id.generation => Some(cooked.inventory),
        _ => None,
    })
}

/// The made cook `id`'s gun (its inventory's `slot`) drawn alone lying at
/// `gun` (moved there when drawn elsewhere); false when it is not made or
/// nothing could be placed (no world, no model, or the game has it in the
/// scene).
pub fn show(thread: &Thread, id: CookId, slot: WeaponSlot, gun: &LyingGun) -> bool {
    TABLE.with(thread, |table| match &mut table.tickets[usize::from(id.index)] {
        Ticket::Made { generation, cooked, .. } if *generation == id.generation => {
            cooked.drawn = cooked.drawn.to(cooked.inventory, Some((slot, gun)));
            matches!(cooked.drawn, Drawn::Lying { .. })
        }
        _ => false,
    })
}

/// The made cook `id`'s gun no longer drawn alone: what it placed out of
/// the scene (nothing when nothing is).
pub fn hide(thread: &Thread, id: CookId) {
    TABLE.with(thread, |table| {
        if let Ticket::Made { generation, cooked, .. } = &mut table.tickets[usize::from(id.index)]
            && *generation == id.generation {
            cooked.drawn = cooked.drawn.to(cooked.inventory, None);
        }
    });
}

/// The gun `player` holds at its death, by the slot of its kit's inventory
/// that holds it: a client weapon (its vtable) in slot 7 or 10. Anything
/// else (nothing, a battery, a grenade, a medkit, a gun of another slot)
/// is none.
pub fn held_gun(player: &Player<'_>) -> Option<WeaponSlot> {
    // SAFETY: the live player (a death hook's), its held active object, its
    // inventory's slots: alive for the hook. The object is read as a weapon
    // only after its vtable says it is one.
    unsafe {
        let held = player.this.held_item()?;
        if read(held.vtable()) != raw::image::WEAPON_VTABLE {
            return None;
        }
        let item = held.inventory_item().address();
        let slots = player.this.inventory()?.slots();
        [WeaponSlot::First, WeaponSlot::Second].into_iter()
            .find(|slot| slots.at(usize::from(slot.slot())).item().is_some_and(|i| i.address() == item))
    }
}

impl Player<'_> {
    /// The kit in use (+115h).
    pub fn kit(&self) -> Option<Kit> {
        // SAFETY: the live player's byte.
        Kit::new(unsafe { read(self.this.kit()) })
    }

    /// Its simulation time, ms (+324h): a death's time at the death hook.
    pub fn time(&self) -> u32 {
        // SAFETY: as kit.
        unsafe { read(self.this.time()) }
    }
}

/// What the player cook's recipe takes from a match (0x20284C..0x2028A6),
/// read from a player of it.
struct Match {
    profile: raw::PlayerProfile,
    dictionary: Option<raw::ItemsDictionary>,
    spatial_tree: Option<raw::SpatialTree>,
    game_world: raw::GameWorld,
    seed: u32,
}

/// # Safety
/// `player` is live on this thread.
unsafe fn match_of(player: raw::Player) -> Option<Match> {
    unsafe {
        let world = player.game_world()?;
        let game = world.game()?;
        let options = raw::engine_match_options(game.network_client())?;
        Some(Match {
            profile: player.profile()?,
            dictionary: game.dictionary(),
            spatial_tree: world.pvp_match()?.spatial_tree(),
            game_world: world,
            seed: read(options.seed()),
        })
    }
}

/// Asks for `player`'s kit `kit`'s `slot` gun alone. The profile copy and
/// the recipe are kept by the ticket until the completion; the query's own
/// requests, user data and callback are copied by it before it returns.
/// Holds no lock across the query (it may complete before it returns).
pub fn start(player: &Player<'_>, kit: Kit, slot: WeaponSlot) -> Result<Gun, Refusal> {
    let thread = player.thread;
    let allocator = allocator().ok_or(Refusal::NoAllocator)?;
    // SAFETY: the live player and the match objects it names, on its thread.
    let found = unsafe { match_of(player.this) }.ok_or(Refusal::NoContext)?;
    // SAFETY: the player's live profile.
    let profile = unsafe { ProfileCopy::new(found.profile, kit, slot) }?;
    let recipe = Recipe::new()?;
    // SAFETY: our recipe, 1Ch bytes: its fields as the player cook fills
    // them (0x20284C..0x2028A6), the profile ours, the stage's flag 0 (a match).
    unsafe {
        let r = recipe.0;
        r.set_profile(Some(profile.0));
        *r.kit() = kit.get();
        r.set_dictionary(found.dictionary);
        r.set_physics_world(found.spatial_tree);
        r.set_game_world(Some(found.game_world));
        *r.match_seed() = found.seed;
        *r.flag() = 0;
    }
    let recipe_at = recipe.0;
    let since = clock::counter(thread);
    let id = TABLE.with(thread, |table| {
        let Some(index) = table.tickets.iter().position(|t| matches!(t, Ticket::Free)) else {
            return Err((recipe, profile));
        };
        table.generation = table.generation.wrapping_add(1);
        let generation = table.generation;
        table.tickets[index] = Ticket::Waiting { generation, recipe, profile, since };
        Ok(CookId { index: index as u8, generation })
    });
    // No ticket: the recipe and the profile are given back outside the lock.
    let id = id.map_err(|pieces| {
        drop(pieces);
        Refusal::NoTicket
    })?;
    let mut callback = raw::QueryCallbackStorage::zeroed();
    let mut variant = raw::InventoryVariantStorage::zeroed();
    let mut request = raw::ResourceRequestStorage::zeroed();
    // SAFETY: our rooms, on this stack for the call; the query copies the
    // request, the user data (by the variant's +28h) and the callback
    // before it returns. The callback's object is the ticket's number + 1
    // (never 0), which the invoker hands our member as ECX.
    unsafe {
        let v = variant.handle();
        *v.vtable() = raw::image::INVENTORY_VARIANT_VTABLE;
        v.set_recipe(Some(recipe_at));
        v.set_this(Some(v));
        *v.type_id() = raw::engine_inventory_variant_type();
        let q = request.handle();
        q.set_path(raw::image::INVENTORY_REQUEST as usize as *mut u8);
        *q.class() = INVENTORY_CLASS;
        let member = cooked_entry as unsafe extern "thiscall" fn(u32, u32) as *const c_void;
        let callback = raw::engine_bind_member_callback(Some(callback.handle()), member, 0, u32::from(id.index) + 1, 0);
        let mut user = v.address() as usize as *mut c_void;
        let callback = callback.map_or(core::ptr::null_mut(), |c| c.address() as usize as *mut c_void);
        raw::engine_query_resources(q.address() as usize as *const c_void, 1, callback, allocator,
            (&mut user as *mut *mut c_void).cast(), core::ptr::null_mut(), 1);
    }
    Ok(Gun { id })
}

/// The game's `resources::dispatch_callbacks` (0xE5AB0) on this thread: the
/// translations, creations and finished queries' callbacks of every query
/// this thread made, ours and the game's. Call it holding no lock: the
/// callbacks reach our hooks.
pub fn dispatch_callbacks(_: &Thread) {
    // SAFETY: the game's own function, no arguments, on the game's thread.
    unsafe { raw::engine_dispatch_callbacks() };
}

/// The member the boost callback calls (thiscall: ECX the object, the
/// queries_result on the stack, `ret 4`). The game's frames are aligned to
/// 4 and Rust's i686 code assumes 16, so it aligns the stack and calls
/// [`cooked_shim`] as rust_hook_026g.inc's RUST_THISCALL does.
#[unsafe(naked)]
unsafe extern "thiscall" fn cooked_entry(_ticket: u32, _result: u32) {
    core::arch::naked_asm!(
        "push ebp",
        "mov ebp, esp",
        "and esp, -16",
        "sub esp, 8",
        "push dword ptr [ebp + 8]",
        "push ecx",
        "call {shim}",
        "leave",
        "ret 4",
        shim = sym cooked_shim,
    )
}

/// The completion of the cook numbered `ticket` - 1: keeps one reference to
/// the result's inventory, or records the failure; an abandoned ticket's
/// result and pieces are given back. Everything freed is freed outside the
/// lock.
extern "C" fn cooked_shim(ticket: u32, result: Option<raw::QueriesResult>) {
    // SAFETY: only cooked_entry calls this, from the boost invoker 0x202300
    // run by dispatch_callbacks on the game's thread that queried.
    let thread = unsafe { Thread::new() };
    // SAFETY: the live queries_result, alive through the callback; one
    // result asked, its count checked before it is read.
    let made = result.and_then(|result| unsafe {
        if read(result.count()) != 1 {
            return None;
        }
        let inventory = result.results().at(0).inventory()?;
        AtomicU32::from_ptr(inventory.references()).fetch_add(1, Ordering::AcqRel);
        Some(inventory)
    });
    let at = clock::counter(&thread);
    let index = ticket.wrapping_sub(1) as usize;
    let (pieces, leftover) = TABLE.with(&thread, |table| {
        let Some(slot) = table.tickets.get_mut(index) else { return ((None, None), made) };
        match core::mem::replace(slot, Ticket::Free) {
            Ticket::Waiting { generation, recipe, profile, since } => match made {
                Some(inventory) => {
                    *slot = Ticket::Made { generation, cooked: CookedInventory { inventory, _profile: profile, drawn: Drawn::Hidden },
                                           since, at };
                    ((Some(recipe), None), None)
                }
                None => {
                    *slot = Ticket::Failed { generation, since, at };
                    ((Some(recipe), Some(profile)), None)
                }
            },
            Ticket::Abandoned { recipe, profile } => ((Some(recipe), Some(profile)), made),
            other => {
                *slot = other;
                ((None, None), made)
            }
        }
    });
    if let Some(inventory) = leftover {
        release(inventory);
    }
    drop(pieces);
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

/// This process's id (kernel32's GetCurrentProcessId): two local games share
/// one log file, and each line says whose it is.
pub fn process_id() -> u32 {
    // SAFETY: the exe's import, no arguments, no state.
    unsafe { (raw::engine_get_current_process_id)() }
}

/// The player being destroyed (site `player.destroy`, player::~player's
/// entry, before any of it is torn down).
pub fn destroyed(ctx: &super::Context<'_>) -> Option<super::world::PlayerId> {
    // SAFETY: the site's ECX, the player, not torn down yet.
    Player::of(ctx, unsafe { raw::site::player_destroy::player(ctx) }).map(|p| p.id())
}

/// The game world's match core: which match a frame is of.
pub fn world_key(world: &super::world::GameWorld<'_>) -> Option<super::world::WorldKey> {
    // SAFETY: the site's game world and its match, compared only.
    unsafe { Some(super::world::WorldKey::of(world.this.pvp_match()?.core())) }
}

/// The world core's completed simulation time (+18FC0h) of the world ticked
/// at `game_world.after_move`.
pub fn world_time(world: &super::world::GameWorld<'_>) -> Option<u32> {
    // SAFETY: the site's game world, its match and core, alive for the hook.
    unsafe { Some(read(world.this.pvp_match()?.core().time())) }
}

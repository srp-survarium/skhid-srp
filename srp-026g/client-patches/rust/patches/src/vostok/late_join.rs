//! The late join's game (patch 11; patch 14's place of a late place's
//! objects): the network client and its match, the server's proposals, a
//! proposal's storage and its players' query, the place a newcomer is put
//! into, and the answers. Every field is fields_026g.tsv's, every function
//! engine_026g.tsv's; this is what their values mean, each operation sound
//! for any value a patch can give it. The order of things and the decisions
//! are the patch's (late_join.rs).
//!
//! What a patch keeps between hooks is a key (late_join::runtime's: an
//! address as a value) and the two owners here: a proposal's storage
//! ([`ProfileBox`], counted, freed by its last owner) and the reference its
//! query holds on the match's world (inside the storage while the query
//! runs, given back by its callback: [`ProfileArrival`]). A live handle is
//! made again in each hook, from the site's registers or from the game's
//! own network client ([`Client::current`]).

use core::cell::Cell;
use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::{NonNull, null};
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::Context;
use super::hook::{CallbackEntry, Thread};
use super::parse::Reader;
use super::resources::QueryResults;
use super::world::read;
use super::OrigProfileArrived;
use crate::late_join::profile::{CheckedProfile, Dictionary, ItemRecord, Modification};
use crate::late_join::runtime::{
    Ack, ClientKey, DictionaryKey, KitMask, MatchKey, Place, PlayerKey, ProfileKey, Roster, Seed, Squad, WorldKey,
    KITS,
};
use crate::raw;

/// The network client's match connection state (+1F1C64h).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connection {
    /// Its match's first options and world on their way.
    Starting,
    /// In play.
    Playing,
    Other,
}

/// The network client a hook was given, or the game's own.
pub struct Client<'t> {
    this: raw::NetworkClient,
    thread: &'t Thread,
}

impl<'t> Client<'t> {
    fn of(this: raw::NetworkClient, thread: &'t Thread) -> Client<'t> {
        Client { this, thread }
    }

    /// At `late_join.ready` (on_match_ready's EBX).
    pub fn at_ready(ctx: &'t Context<'_>) -> Option<Client<'t>> {
        // SAFETY: the site's EBX is the network client whose match is ready,
        // alive while on_match_ready runs on the game's thread.
        unsafe { raw::site::late_join_ready::client(ctx) }.map(|this| Client::of(this, ctx.thread()))
    }

    /// At `late_join.disconnect` (EDI).
    pub fn at_disconnect(ctx: &'t Context<'_>) -> Option<Client<'t>> {
        // SAFETY: as at_ready: on_match_disconnected's client.
        unsafe { raw::site::late_join_disconnect::client(ctx) }.map(|this| Client::of(this, ctx.thread()))
    }

    /// At `late_join.client_destroyed` (ECX, its destructor's start).
    pub fn at_destroyed(ctx: &'t Context<'_>) -> Option<Client<'t>> {
        // SAFETY: as at_ready: the client being destroyed, whole until its
        // destructor's first instruction runs.
        unsafe { raw::site::late_join_client_destroyed::client(ctx) }.map(|this| Client::of(this, ctx.thread()))
    }

    /// At `late_join.prepare`: the client and the 0xE4's bytes after its id
    /// (none when its reader's cursor is outside them).
    pub fn at_prepare(ctx: &'t Context<'_>) -> Option<(Client<'t>, &'t [u8])> {
        use raw::site::late_join_prepare as site;
        // SAFETY: the site's ESI and EBX: the network client and the message's
        // reader, alive while on_match_packet_received runs on the game's thread.
        let (client, reader) = unsafe { (site::client(ctx)?, site::reader(ctx)?) };
        Some((Client::of(client, ctx.thread()), rest(reader).remaining()))
    }

    /// The game's network client (the engine world's game's), on any hook of
    /// the game's thread.
    pub fn current(thread: &'t Thread) -> Option<Client<'t>> {
        // SAFETY: the engine world slot and the game it holds live for the
        // process; the game's network client is its own (game+53ACh).
        let this = unsafe { raw::image::engine_world().world()?.game()?.network_client()? };
        Some(Client::of(this, thread))
    }

    pub fn key(&self) -> ClientKey {
        ClientKey(self.this.address())
    }

    pub fn connection(&self) -> Connection {
        // SAFETY: a live client's plain field.
        match unsafe { read(self.this.match_connection_state()) } {
            1 => Connection::Starting,
            2 => Connection::Playing,
            _ => Connection::Other,
        }
    }

    /// Its match: its world (the pvp match, +1F1C40h), the world's options
    /// and the game. None without one.
    pub fn matched(&self) -> Option<Match<'t>> {
        // SAFETY: a live client's pointers: its pvp match (alive while the
        // client holds it), that match's options, the game.
        unsafe {
            let world = self.this.pvp_match()?;
            Some(Match { client: self.this, world, options: world.options()?, game: self.this.game()?,
                         thread: self.thread })
        }
    }

    /// Tells the server `ack` (0xE0's late-join answer: a marker no world
    /// upload can have, the match, the try, the place, the answer), while the
    /// client plays that match. Whether it went (a full packet pool: the
    /// server asks again).
    pub fn ack(&self, ack: Ack) -> bool {
        if self.connection() != Connection::Playing {
            return false;
        }
        // SAFETY: a live client's options holder (its match client, +1F1C48h)
        // and its seed, plain values.
        let Some(holder) = (unsafe { self.this.options_holder() }) else { return false };
        if unsafe { read(holder.match_seed()) } != ack.seed.0 {
            return false;
        }
        let mut message = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0];
        message[8..12].copy_from_slice(&ack.seed.0.to_le_bytes());
        message[12..16].copy_from_slice(&ack.serial.get().to_le_bytes());
        message[16] = ack.place.get();
        message[17] = ack.answer.wire();
        super::packet_recovery::send_plain(holder, &message)
    }
}

/// The bytes of a reader from its cursor to its end; none when the cursor
/// is outside its bytes.
pub(super) fn rest<'t>(reader: raw::BufferReader) -> Reader<'t> {
    // SAFETY: a live reader, for the hook.
    let (begin, cursor, size) = unsafe { (reader.begin(), reader.cursor(), read(reader.size())) };
    let (first, at) = (begin as usize, cursor as usize);
    let Some(end) = first.checked_add(size as usize) else { return Reader::new(&[]) };
    if cursor.is_null() || at < first || at > end {
        return Reader::new(&[]);
    }
    // SAFETY: begin..begin+size are the message's bytes, alive for the hook,
    // and cursor..end lies inside them.
    unsafe { Reader::of_memory(cursor, end - at) }
}

/// A client's match: its world, the world's options and the game.
pub struct Match<'t> {
    client: raw::NetworkClient,
    world: raw::PvpMatch,
    options: raw::MatchOptions,
    game: raw::Game,
    thread: &'t Thread,
}

impl<'t> Match<'t> {
    pub fn key(&self) -> MatchKey {
        // SAFETY: the live options' plain seed.
        MatchKey { client: ClientKey(self.client.address()), world: WorldKey(self.world.address()),
                   seed: Seed(unsafe { read(self.options.seed()) }) }
    }

    /// The places taken (the options' count).
    pub fn roster(&self) -> Roster {
        // SAFETY: as key.
        Roster(unsafe { read(self.options.places()) })
    }

    /// The game's items dictionary.
    pub fn dictionary(&self) -> Option<ItemsDictionary<'t>> {
        // SAFETY: the game's dictionary pointer, live for the game.
        unsafe { self.game.dictionary() }.map(|this| ItemsDictionary { this, _thread: self.thread })
    }

    /// The place a newcomer goes into, when it is the next one: the options
    /// count `place` places, the core's places vector holds as many and has
    /// room for one more, the match is `seed`'s and has its statistics.
    pub fn next_place(&self, place: Place, seed: Seed) -> Option<NextPlace<'_, 't>> {
        // SAFETY: the live world's core and options, their plain values and
        // pointers; the vector's three pointers are read, never through.
        unsafe {
            let core = self.world.core();
            let (begin, end, room) = (core.places()?, core.places_end()?, core.places_capacity()?);
            core.statistics()?;
            let index = usize::from(place.get());
            let ok = read(self.options.places()) == place.get() && read(self.options.seed()) == seed.0
                && end.index_from(begin) == Some(index) && room.address() >= end.address() + raw::CorePlace::SIZE as u32;
            ok.then_some(NextPlace { matched: self, core, slot: end, place })
        }
    }

    /// Puts the place's players into the match's physics (0x3218A0).
    pub fn insert_physics(&self, place: &OpenPlace<'_>) {
        // SAFETY: the live world's physics and a place of its core.
        unsafe { raw::engine_place_physics(Some(self.world.physics()), Some(place.slot)) };
    }

    /// The match's animations registry.
    pub fn animations(&self) -> Option<AnimationRegistry<'t>> {
        // SAFETY: the live world's registry, kept by it.
        unsafe { self.world.animations() }.map(|this| AnimationRegistry { this, _thread: self.thread })
    }

    /// Patch 13: the hud's lists of players (the scoreboard, the second
    /// list, the enemies' markers) made again with the places of now.
    pub fn rebuild_lists(&self) {
        // SAFETY: the game's hud (its game world's +1FCh) and the client's
        // options (its match client's +0Ch), as 0x1F8542..0x1F855D pass them;
        // late_join_026g.asm's rebuild makes the lists only for the hud the
        // match's start made, with its two movies, else does nothing.
        unsafe {
            let Some(holder) = self.client.options_holder() else { return };
            super::generated::late_join_rebuild_lists(self.game.world().ui(), holder.options());
        }
    }
}

/// The game's items dictionary, as the profile's parse asks it.
pub struct ItemsDictionary<'t> {
    this: raw::ItemsDictionary,
    _thread: &'t Thread,
}

impl ItemsDictionary<'_> {
    pub fn key(&self) -> DictionaryKey {
        DictionaryKey(self.this.address())
    }
}

impl Dictionary for ItemsDictionary<'_> {
    fn item(&self, id: u16) -> Option<ItemRecord> {
        // SAFETY: the live dictionary's rows (3A0h each) and their count.
        unsafe {
            let rows = self.this.entries()?;
            let count = read(self.this.entry_count()) as usize;
            (0..count).map(|i| rows.at(i)).find(|row| read(row.dict_id()) == id)
                .map(|row| if read(row.stack()) != 0 { ItemRecord::Stack } else { ItemRecord::Single })
        }
    }

    fn has_modification(&self, id: u32, kind: Modification) -> bool {
        // SAFETY: as item: the modifications' or the defects' rows (2Ch each).
        unsafe {
            let (rows, count) = match kind {
                Modification::Ordinary => (self.this.modifications(), read(self.this.modification_count())),
                Modification::Defect => (self.this.defects(), read(self.this.defect_count())),
            };
            rows.is_some_and(|rows| (0..count as usize).any(|i| read(rows.at(i).id()) == id))
        }
    }
}

/// A proposal's storage on the game's heap: the profile body its players
/// are loaded from (a player keeps a pointer to it, +6B8h, for its life),
/// the proposal's checked body (the profile, then srp's block if any: a
/// retransmission is compared with all of it), and, while the players'
/// query runs, the query's reference on the match's world. Counted: a clone
/// is another owner, and the last one frees it.
pub struct ProfileBox {
    header: NonNull<Header>,
}

/// The allocation's start; the profile body follows it, then the bytes.
#[repr(C)]
struct Header {
    owners: AtomicU32,
    /// The profile body (the constructor's result): never 0 once stored.
    body: Option<raw::MatchPlace>,
    /// The world the query holds a count on, until its callback takes it.
    /// The game's thread alone sets and takes it.
    world: Cell<Option<raw::PvpMatch>>,
    /// The checked body's bytes.
    len: u32,
    /// Of them, the profile's (the front): what the game's reader takes.
    profile: u32,
    usable: u8,
}

/// The body's offset: after the header, 8-aligned (the allocator's).
const BODY: usize = (core::mem::size_of::<Header>() + 7) & !7;
const LABEL: &[u8] = b"srp late-join profile\0";

// SAFETY: the header's counts are atomic, the body and the bytes are
// written once, before the first clone; a player destroyed on another
// thread drops a clone there (late_join.rs's on_player_destroyed).
unsafe impl Send for ProfileBox {}

impl ProfileBox {
    /// A new storage holding `profile`: allocated from the game's allocator,
    /// the profile body made by the game's constructor and reader. None
    /// without memory.
    pub fn store(_: &Thread, profile: &CheckedProfile<'_>, dictionary: &ItemsDictionary<'_>) -> Option<ProfileBox> {
        let bytes = profile.body();
        let size = BODY + raw::MatchPlace::SIZE + bytes.len();
        // SAFETY: the game's allocator (set before any match) gives `size`
        // bytes, 8-aligned (doug lea), or none.
        let memory = unsafe {
            let allocator = raw::image::game_allocator().allocator()?;
            raw::engine_malloc(allocator.address() as usize as *mut c_void, size as u32, LABEL.as_ptr().cast())
        };
        let header = NonNull::new(memory.cast::<Header>())?;
        // SAFETY: ours, `size` bytes: the header, the body's room, the bytes.
        // The constructor fills the body's 45FCh bytes (it returns them); the
        // reader takes the checked profile alone (CheckedProfile: it fits the
        // body and names the dictionary's items; the body's front).
        unsafe {
            header.as_ptr().write(Header { owners: AtomicU32::new(1), body: None, world: Cell::new(None),
                                           len: bytes.len() as u32, profile: profile.profile().len() as u32,
                                           usable: profile.usable().bits() });
            let wire = memory.cast::<u8>().add(BODY + raw::MatchPlace::SIZE);
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), wire, bytes.len());
            let Some(body) = raw::engine_profile_body_construct(memory.cast::<u8>().add(BODY).cast()) else {
                free(memory);
                return None;
            };
            (*header.as_ptr()).body = Some(body);
            let stored = ProfileBox { header };
            stored.read_into(body, dictionary.this);
            Some(stored)
        }
    }

    fn header(&self) -> &Header {
        // SAFETY: an owner's: the allocation lives while one does.
        unsafe { self.header.as_ref() }
    }

    fn body(&self) -> raw::MatchPlace {
        self.header().body.expect("stored: its body constructed")
    }

    /// The proposal's checked body.
    fn bytes(&self) -> &[u8] {
        // SAFETY: as header: `len` bytes after the body, written by store.
        unsafe {
            let at = self.header.as_ptr().cast::<u8>().add(BODY + raw::MatchPlace::SIZE);
            core::slice::from_raw_parts(at, self.header().len as usize)
        }
    }

    /// The game's reader fills `body` (constructed) from the stored profile.
    ///
    /// # Safety
    /// `body` is a constructed profile body; `dictionary` live.
    unsafe fn read_into(&self, body: raw::MatchPlace, dictionary: raw::ItemsDictionary) {
        let bytes = &self.bytes()[..self.header().profile as usize];
        let mut reader = raw::BufferReaderStorage::zeroed();
        let r = reader.handle();
        // SAFETY: the reader {begin, cursor, size} over our checked profile,
        // which store's CheckedProfile proved the reader takes whole.
        unsafe {
            r.set_begin(bytes.as_ptr().cast_mut());
            r.set_cursor(bytes.as_ptr().cast_mut());
            *r.size() = bytes.len() as u32;
            raw::engine_profile_read(Some(r), Some(body), Some(dictionary));
        }
    }

    /// Its profile's key (the body's address, a player's +6B8h).
    pub fn key(&self) -> ProfileKey {
        ProfileKey(self.body().address())
    }

    /// The kits whose players the profile has (its checked usable kits).
    pub fn usable(&self) -> KitMask {
        KitMask::parse(self.header().usable).expect("stored from a CheckedProfile")
    }

    /// Whether its proposal's body is `body`.
    pub fn same_body(&self, body: &[u8]) -> bool {
        self.bytes() == body
    }

    /// Asks the game for the players of its usable kits, for `place` of the
    /// match: the query holds this storage and a reference on the world until
    /// its callback (`callback`, the late join's) gives them back. It may come
    /// at once.
    pub fn query(self, matched: &Match<'_>, place: Place, callback: CallbackEntry<OrigProfileArrived>) {
        let usable = self.usable();
        let mut requests = [const { raw::ResourceRequestStorage::zeroed() }; KITS];
        let mut variants = [const { raw::PlayerVariantStorage::zeroed() }; KITS];
        let mut user = [const { raw::QueryUserDataStorage::zeroed() }; KITS];
        let mut function = raw::QueryFunctionStorage::zeroed();
        // SAFETY: the live match: its world's count taken for the query
        // (atomic, as the game's lock inc), kept in the header until the
        // callback; a request {path, class 54h}, a player creation params
        // variant and its user data pointer for each usable kit, in order,
        // as 0x27C160's players' query fills them (the variant's helper, the
        // profile, the place, the game world, the spatial tree, the
        // dictionary, the kit, its own address and the type's id), a boost
        // function bound to the callback and this storage (an owner the
        // callback takes back), the game's allocator: query_resources copies
        // all of them before it returns, and its cook reads the profile, the
        // tree and the dictionary while it loads, which the owner and the
        // world's count keep alive.
        unsafe {
            let world = matched.world;
            AtomicU32::from_ptr(world.references()).fetch_add(1, Ordering::AcqRel);
            self.header().world.set(Some(world));
            let tree = world.spatial_tree();
            let dictionary = matched.game.dictionary();
            let game_world = matched.game.world();
            let type_id = raw::engine_player_params_type();
            for (n, kit) in usable.kits().enumerate() {
                let request = requests[n].handle();
                request.set_path(raw::image::PLAYER_REQUEST as usize as *mut u8);
                *request.class() = PLAYER_CLASS;
                let variant = variants[n].handle();
                *variant.helper() = raw::image::PLAYER_VARIANT_VTABLE;
                variant.set_profile(Some(self.body()));
                *variant.place() = place.get();
                variant.set_world(Some(game_world));
                variant.set_tree(tree);
                variant.set_dictionary(dictionary);
                *variant.kit() = kit as u8;
                variant.set_itself(Some(variant));
                *variant.type_id() = type_id;
                user[n].handle().set_player(Some(variant));
            }
            let f = function.handle();
            raw::engine_bind_query_callback(Some(f), callback.entry(), self.into_word());
            let allocator = raw::image::game_allocator().allocator();
            let count = usable.kits().count() as u32;
            raw::engine_query_one(raw::ResourceRequestStorage::first(&mut requests), count, Some(f), allocator,
                                  raw::QueryUserDataStorage::first(&mut user), null(), ASYNCHRONOUS);
        }
    }

    /// Gives up this owner to the query's callback (its word).
    fn into_word(self) -> u32 {
        let word = self.header.as_ptr() as usize as u32;
        core::mem::forget(self);
        word
    }
}

/// The players' request's class (a player, gameplay/players/default.player).
const PLAYER_CLASS: u32 = 0x54;
/// query_resources' last argument: a normal asynchronous query.
const ASYNCHRONOUS: u32 = 1;

impl Clone for ProfileBox {
    fn clone(&self) -> ProfileBox {
        self.header().owners.fetch_add(1, Ordering::Relaxed);
        ProfileBox { header: self.header }
    }
}

impl Drop for ProfileBox {
    fn drop(&mut self) {
        if self.header().owners.fetch_sub(1, Ordering::AcqRel) == 1 {
            // SAFETY: the last owner: nothing reads it after (no player is
            // left on its profile: the late join frees a proposal only then).
            unsafe { free(self.header.as_ptr().cast()) };
        }
    }
}

/// Frees memory of the game's allocator.
///
/// # Safety
/// `memory` is the allocator's, freed this once.
unsafe fn free(memory: *mut c_void) {
    // SAFETY: the caller's.
    unsafe {
        let Some(allocator) = raw::image::game_allocator().allocator() else { return };
        let mut pointer = memory;
        raw::engine_free(allocator.address() as usize as *mut c_void, memory, &mut pointer);
    }
}

/// A players' query's end: the storage it was asked with and the world's
/// reference it held, given back to the callback.
pub struct ProfileArrival {
    storage: ProfileBox,
    world: Option<HeldWorld>,
}

impl ProfileArrival {
    /// # Safety
    /// `word` is the value the late join's callback was called with: an
    /// owner ProfileBox::query gave the query (`late_join.query_done`'s
    /// entry is handed to no other query: its marker type is its own).
    pub(super) unsafe fn from_word(_: &Thread, word: u32) -> ProfileArrival {
        // SAFETY: the caller's: the header of a live storage, whose owner the
        // query held; the world its query took a count on, taken once.
        let storage = ProfileBox { header: unsafe { NonNull::new_unchecked(word as usize as *mut Header) } };
        let world = storage.header().world.take();
        ProfileArrival { world: world.map(HeldWorld), storage }
    }

    pub fn storage(&self) -> &ProfileBox {
        &self.storage
    }

    /// The players the query loaded, by kit: a result for each usable kit,
    /// in order (none for a kit not asked for, or not loaded).
    pub fn players<'r>(&self, results: &'r QueryResults<'_>) -> [Option<LoadedPlayer<'r>>; KITS] {
        let mut players = [None; KITS];
        // SAFETY: the results are alive for the callback; ours asked one
        // player a usable kit, their results 2E0h apart, at most three.
        unsafe {
            let Some(raw) = results.raw() else { return players };
            let count = read(raw.count()) as usize;
            for (n, kit) in self.storage.usable().kits().enumerate().take(count) {
                players[kit] = raw.results().at(n).player().map(|this| LoadedPlayer { this, _results: PhantomData });
            }
        }
        players
    }

    /// The storage and the world's reference, apart.
    pub fn into_parts(self) -> (ProfileBox, Option<HeldWorld>) {
        (self.storage, self.world)
    }
}

/// A reference on a match's world (its pvp match's count) the players'
/// query held: let go by [`HeldWorld::release`], once.
#[must_use]
pub struct HeldWorld(raw::PvpMatch);

impl HeldWorld {
    /// Lets the world go, as the game does (intrusive_ptr's release, which
    /// destroys it at 0: its players may be destroyed before this returns).
    pub fn release(self, _: &Thread) {
        let mut world = Some(self.0);
        // SAFETY: the count our query took (ProfileBox::query) on a world it
        // kept alive since; the release takes it and clears the pointer.
        unsafe { raw::engine_release_pvp(&mut world) };
    }
}

/// A player the query loaded: live for its callback (the results own it).
#[derive(Clone, Copy)]
pub struct LoadedPlayer<'r> {
    this: raw::Player,
    _results: PhantomData<&'r ()>,
}

impl<'r> LoadedPlayer<'r> {
    pub fn key(&self) -> PlayerKey {
        PlayerKey(self.this.address())
    }

    /// The player, for its animations.
    pub fn animated(&self) -> AnimatedPlayer<'r> {
        AnimatedPlayer(self.this, PhantomData)
    }
}

/// A player whose animations a registry takes: the site's, or one a query
/// loaded, live while it is borrowed.
#[derive(Clone, Copy)]
pub struct AnimatedPlayer<'a>(raw::Player, PhantomData<&'a ()>);

/// The place a newcomer goes into: the core's places vector's end, with room.
pub struct NextPlace<'m, 't> {
    matched: &'m Match<'t>,
    core: raw::CoreWorld,
    slot: raw::CorePlace,
    place: Place,
}

impl<'m, 't> NextPlace<'m, 't> {
    /// The options' profile of the place made from the stored profile (its
    /// constructor, then its reader).
    pub fn read_profile(&self, storage: &ProfileBox, dictionary: &ItemsDictionary<'_>) {
        // SAFETY: the live options' place (one of 20, the place < 20); the
        // constructor makes it anew, the reader takes the checked bytes.
        unsafe {
            let place = self.matched.options.first_place().at(usize::from(self.place.get()));
            if let Some(body) = raw::engine_profile_body_construct(place.address() as usize as *mut c_void) {
                storage.read_into(body, dictionary.this);
            }
        }
    }

    /// The place's slot, made as the world's constructor makes one (zeros,
    /// FFh for the kits, itself, the place's tick, no id yet, the place),
    /// not yet in the vector.
    pub fn open(self) -> OpenPlace<'m> {
        // SAFETY: the vector's room past its end (next_place checked it), the
        // core's own memory, 30h bytes; nothing reads it until it is counted.
        unsafe {
            core::ptr::write_bytes(self.slot.address() as usize as *mut u8, 0, raw::CorePlace::SIZE);
            *self.slot.current_kit() = 0xFF;
            *self.slot.next_kit() = 0xFF;
            *self.slot.spare() = 0xFF;
            self.slot.set_itself(Some(self.slot));
            *self.slot.tick() = raw::image::PLACE_TICK;
            *self.slot.next_id() = u32::MAX;
            *self.slot.place() = self.place.get();
        }
        OpenPlace { core: self.core, slot: self.slot, options: self.matched.options, place: self.place,
                    _matched: PhantomData }
    }
}

/// A newcomer's slot, being filled.
pub struct OpenPlace<'m> {
    core: raw::CoreWorld,
    slot: raw::CorePlace,
    options: raw::MatchOptions,
    place: Place,
    _matched: PhantomData<&'m ()>,
}

impl OpenPlace<'_> {
    /// The kit's player, a reference of the place's taken (the game's
    /// players count their holders, +0D0h).
    pub fn hold(&self, kit: usize, player: LoadedPlayer<'_>) {
        // SAFETY: the live player the query loaded and our slot.
        unsafe {
            AtomicU32::from_ptr(player.this.references()).fetch_add(1, Ordering::Relaxed);
            match kit {
                0 => self.slot.set_kit0(Some(player.this)),
                1 => self.slot.set_kit1(Some(player.this)),
                _ => self.slot.set_kit2(Some(player.this)),
            }
        }
    }

    /// The place counted in: the vector's end past it, its squad, the
    /// options' places and the shared statistics' live count one more.
    pub fn publish(&self, squad: Squad) -> Core<'_> {
        let count = self.place.get() + 1;
        // SAFETY: the slot is the vector's end (next_place), the options'
        // squads 20 (the place < 20), the core's statistics (next_place).
        unsafe {
            self.core.set_places_end(Some(self.slot.at(1)));
            *self.options.squads().add(usize::from(self.place.get())) = squad.0;
            *self.options.places() = count;
            if let Some(statistics) = self.core.statistics() {
                *statistics.shared().live_count() = count;
            }
        }
        Core { this: self.core, _place: PhantomData }
    }

    /// The place's slot to bind.
    pub fn slot(&self) -> PlaceSlot<'_> {
        PlaceSlot { this: self.slot, _place: PhantomData }
    }

    /// The hud's icon of the place's player in its current kit.
    pub fn player_ready(&self) {
        // SAFETY: our slot, bound: its current kit's player (none for a kit
        // byte out of the three).
        unsafe {
            let player = match read(self.slot.current_kit()) {
                0 => self.slot.kit0(),
                1 => self.slot.kit1(),
                2 => self.slot.kit2(),
                _ => None,
            };
            if let Some(player) = player {
                raw::engine_player_icon(Some(player));
            }
        }
    }
}

/// A place's slot being bound (at `late_join.bind_place`, or a newcomer's).
pub struct PlaceSlot<'a> {
    this: raw::CorePlace,
    _place: PhantomData<&'a ()>,
}

impl PlaceSlot<'_> {
    /// Its place's index.
    pub fn place(&self) -> u8 {
        // SAFETY: a live slot's plain byte.
        unsafe { read(self.this.place()) }
    }
}

/// A match's core, for a place's bind.
pub struct Core<'a> {
    this: raw::CoreWorld,
    _place: PhantomData<&'a ()>,
}

/// The game-object list as it was before a bind: what the bind puts it
/// aside from, and back after the place's own.
pub struct Aside {
    first: Option<raw::GameObjectNode>,
    last: Option<raw::GameObjectNode>,
}

impl Core<'_> {
    /// The core and the slot at `late_join.bind_place`.
    pub fn at_bind_place<'c>(ctx: &'c Context<'_>) -> Option<(Core<'c>, PlaceSlot<'c>)> {
        use raw::site::late_join_bind_place as site;
        // SAFETY: the site's EAX and its stack's argument: the core being
        // constructed and the place it binds, alive while it runs.
        let (core, slot) = unsafe { (site::core(ctx)?, site::frame(ctx)?.place()?) };
        Some((Core { this: core, _place: PhantomData },
              PlaceSlot { this: slot, _place: PhantomData }))
    }

    /// Binds `slot` (bind_place 0x321820: its players, inventories, weapons
    /// and their grenades and traps) with the place's own ids: its callback
    /// ids from `first_id` on (a counter of ours lent to the core for the
    /// bind, the core's own kept), its weapons' ids from the place's six
    /// (the active objects' end set to them for the bind, then back to where
    /// it was, at least the twenty places' 120). The game-object list is
    /// `aside` meanwhile: the bind starts it empty.
    pub fn bind(&self, slot: &PlaceSlot<'_>, first_id: u32) {
        let mut counter = raw::IdCounterStorage::zeroed();
        let ids = counter.handle();
        // SAFETY: the live core: its id counter's pointer swapped for ours (a
        // local alive through the bind) and back; its active objects' vector
        // (255 room) end set inside its room and back; the game's bind on a
        // place of this core.
        unsafe {
            *ids.next() = first_id;
            let kept_ids = self.this.callback_ids();
            self.this.set_callback_ids(Some(ids));
            let objects = self.this.objects();
            let end = self.this.objects_end();
            let after = objects.map(|o| o.at(ACTIVE_OBJECTS));
            let restore = match (end, after) {
                (Some(end), Some(after)) if end.address() < after.address() => Some(after),
                _ => end,
            };
            self.this.set_objects_end(objects.map(|o| o.at(usize::from(slot.place()) * WEAPON_IDS)));
            raw::engine_bind_place(Some(self.this), Some(slot.this));
            self.this.set_objects_end(restore);
            self.this.set_callback_ids(kept_ids);
        }
    }

    /// The list put aside: empty for the bind.
    pub fn put_aside(&self) -> Aside {
        // SAFETY: the live core's two list pointers.
        unsafe {
            let aside = Aside { first: self.this.game_objects(), last: self.this.game_objects_last() };
            self.this.set_game_objects(None);
            self.this.set_game_objects_last(None);
            aside
        }
    }

    /// The list back, after the nodes the bind put in: the bound ones first,
    /// in the order the game bound them, then what was aside.
    pub fn take_back(&self, aside: Aside) {
        // SAFETY: the live core's list and its nodes (the bind's and the
        // ones aside, each the core's).
        unsafe {
            match self.this.game_objects_last() {
                None => {
                    self.this.set_game_objects(aside.first);
                    self.this.set_game_objects_last(aside.last);
                }
                Some(bound_last) => {
                    if let Some(first) = aside.first {
                        bound_last.set_next(Some(first));
                        first.set_previous(Some(bound_last));
                        self.this.set_game_objects_last(aside.last);
                    }
                }
            }
        }
    }

    /// The first node of the list now (a key: compared, never read through).
    pub fn first_object(&self) -> Option<u32> {
        // SAFETY: a live core's pointer.
        unsafe { self.this.game_objects() }.map(|n| n.address())
    }

    /// The nodes before `listed` (the first one before a bind): the ones the
    /// bind put in; none from MAX_NODES on.
    pub fn nodes_before(&self, listed: Option<u32>) -> Option<u32> {
        let mut count = 0;
        // SAFETY: the live core's list, walked by its links.
        let mut node = unsafe { self.this.game_objects() };
        while let Some(at) = node.filter(|n| Some(n.address()) != listed) {
            count += 1;
            if count >= MAX_NODES {
                return None;
            }
            // SAFETY: a node of the live list.
            node = unsafe { at.next() };
        }
        Some(count)
    }

    /// Every state kept to roll back to gets `added` zero bytes in front of
    /// its game-objects bytes (an untouched grenade or trap saves itself as
    /// one), when it has room for them; a state without is left as it is.
    pub fn shift_saved_objects(&self, added: u32) {
        // SAFETY: the live core's states, walked by their links; each one's
        // buffer {bytes, room, used} is the game's, `used` bytes of `room`.
        unsafe {
            let mut item = self.this.history_oldest();
            while let Some(state) = item {
                let buffer = state.objects();
                let (bytes, room, used) = (buffer.bytes(), read(buffer.room()), read(buffer.used()));
                if !bytes.is_null() && used <= room && added <= room - used {
                    core::ptr::copy(bytes, bytes.add(added as usize), used as usize);
                    core::ptr::write_bytes(bytes, 0, added as usize);
                    *buffer.used() = used + added;
                }
                item = state.next();
            }
        }
    }
}

/// The active objects' room for the twenty places' weapons: six a place.
const ACTIVE_OBJECTS: usize = 120;
const WEAPON_IDS: usize = 6;
/// More nodes than a state has room for is no list of one place's.
pub const MAX_NODES: u32 = 0x2000;

/// The match's animations registry: its entries' tree (by name) and the
/// vector of their ids.
pub struct AnimationRegistry<'t> {
    this: raw::AnimationsRegistry,
    _thread: &'t Thread,
}

/// An entry of the registry.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct AnimationEntry(raw::AnimationEntry);

impl<'t> AnimationRegistry<'t> {
    /// At `late_join.animations`: the player and the registry.
    pub fn at_register(ctx: &'t Context<'_>) -> Option<(AnimatedPlayer<'t>, AnimationRegistry<'t>)> {
        use raw::site::late_join_animations as site;
        // SAFETY: the site's EDI and its stack's argument: a kit's player and
        // the pvp constructor's registry, alive while it runs.
        let (player, this) = unsafe { (site::player(ctx)?, site::frame(ctx)?.registry()?) };
        Some((AnimatedPlayer(player, PhantomData), AnimationRegistry { this, _thread: ctx.thread() }))
    }

    /// The game's registration of the player's animations into it.
    pub fn register(&self, player: AnimatedPlayer<'_>) {
        // SAFETY: a live player and the live registry.
        unsafe { raw::engine_register_animations(Some(player.0), Some(self.this)) };
    }

    /// The ids so far.
    pub fn len(&self) -> u32 {
        // SAFETY: the vector's two pointers, never read through here.
        unsafe {
            match (self.this.id_slots(), self.this.id_slots_end()) {
                (Some(first), Some(end)) => end.index_from(first).unwrap_or(0) as u32,
                _ => 0,
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The entry with id `i` (below `len`).
    pub fn at(&self, i: u32) -> Option<AnimationEntry> {
        if i >= self.len() {
            return None;
        }
        // SAFETY: an id below the count, the vector's.
        unsafe { self.this.id_slots()?.at(i as usize).entry() }.map(AnimationEntry)
    }

    /// Puts `entry` at id `i` (below `len`).
    pub fn set_at(&self, i: u32, entry: AnimationEntry) {
        if i < self.len() {
            // SAFETY: as at.
            if let Some(first) = unsafe { self.this.id_slots() } {
                unsafe { first.at(i as usize).set_entry(Some(entry.0)) };
            }
        }
    }

    /// The entries of the tree in its order (by name), at most MAX_NODES.
    pub fn in_order(&self) -> impl Iterator<Item = AnimationEntry> + '_ {
        // SAFETY: the live tree: its head, whose left is its first.
        let head = unsafe { self.this.tree() };
        let mut node = unsafe { head.left() };
        let mut left = MAX_NODES;
        core::iter::from_fn(move || {
            let at = node.filter(|n| n.address() != head.address() && left > 0)?;
            left -= 1;
            // SAFETY: a node of the live tree: the next in order (MSVC's
            // tree: the right's leftmost, else up while coming from a right).
            unsafe {
                node = match at.right() {
                    Some(mut next) => {
                        while let Some(l) = next.left() {
                            next = l;
                        }
                        Some(next)
                    }
                    None => {
                        let mut child = at;
                        let mut parent = at.parent();
                        while let Some(p) = parent.filter(|p| p.right().map(|r| r.address()) == Some(child.address())) {
                            child = p;
                            parent = p.parent();
                        }
                        if child.right().map(|r| r.address()) != parent.map(|p| p.address()) { parent } else { Some(child) }
                    }
                };
                Some(AnimationEntry(at.entry()))
            }
        })
    }

    /// The entry's id.
    pub fn id(&self, entry: AnimationEntry) -> u32 {
        // SAFETY: an entry of the live registry.
        unsafe { read(entry.0.id()) }
    }

    pub fn set_id(&self, entry: AnimationEntry, id: u32) {
        // SAFETY: as id.
        unsafe { *entry.0.id() = id }
    }

    /// `entry` appended to the ids (the game's push_back).
    pub fn append(&self, entry: AnimationEntry) {
        let mut at = Some(entry.0);
        // SAFETY: the live registry's id vector and an entry of its tree.
        unsafe { raw::engine_animation_append(Some(self.this.id_vector()), &mut at) };
    }

    /// Whether `left` goes before `right` (the game's order by name).
    pub fn less(&self, left: AnimationEntry, right: AnimationEntry) -> bool {
        // SAFETY: two entries of the live registry.
        unsafe { raw::engine_animation_less(Some(right.0), Some(left.0)) & 0xFF != 0 }
    }

    /// The registry marked actualized (+10050h).
    pub fn set_actualized(&self) {
        // SAFETY: the live registry's byte.
        unsafe { *self.this.actualized() = 1 }
    }
}

/// A player at `late_join.player_destroyed`, about to be destroyed: its key
/// and its profile's.
pub struct Destroyed<'t> {
    this: raw::Player,
    profile: Option<ProfileKey>,
    _thread: &'t Thread,
}

impl<'t> Destroyed<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<Destroyed<'t>> {
        // SAFETY: the site's ECX: the player its scalar deleting destructor
        // is about to destroy, whole until ~player runs.
        let this = unsafe { raw::site::late_join_player_destroyed::player(ctx)? };
        let profile = unsafe { this.profile() }.map(|p| ProfileKey(p.address()));
        Some(Destroyed { this, profile, _thread: ctx.thread() })
    }

    pub fn key(&self) -> PlayerKey {
        PlayerKey(self.this.address())
    }

    /// The profile it was made from (read before it is gone).
    pub fn profile(&self) -> Option<ProfileKey> {
        self.profile
    }

    /// The game's ~player (0x1F2100), which the site called.
    pub fn destroy(self) {
        // SAFETY: the player at the site, destroyed this once (the site's call
        // is skipped).
        unsafe { raw::engine_player_destroy(Some(self.this)) };
    }
}

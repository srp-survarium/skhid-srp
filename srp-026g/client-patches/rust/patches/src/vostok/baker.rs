//! The camouflage/decal baker (vostok::render::
//! camouflage_decal_texture_type_baker_cook, the static 0xA264E80) and the
//! resource queries it answers: what its sites' registers and frames hold,
//! its list of saved textures, and the game's own sequences a patch repeats
//! (a texture given to a query, a query made again, a reference let go).
//!
//! The baker bakes an item's camouflage and decals into a texture of each
//! type (0 d, 1 n, 2 f, 3 s) for a model instance's surface, saves it as
//! user_data/streamed_textures/<name>.dds and lists it (records of 124h
//! bytes, begin/end/capacity at +230h), so a later query loads the file
//! instead.
//!
//! A query, a surface, a model instance, a texture and the cook are ids a
//! patch may keep between hooks ([`Query`], [`Surface`], [`Model`],
//! [`Texture`], [`Cook`]): the game keeps a query until finish_query
//! finishes it, a model instance while a reference to it is held (the
//! look-up takes one for a query it keeps waiting), a texture while a
//! reference to it is held. Only a site's view makes one.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::num::NonZeroU32;
use core::sync::atomic::{AtomicI32, Ordering};

use super::context::{Context, Reg};
use super::object::{read, write};
use crate::raw;

fn ptr(at: u32) -> *mut c_void {
    at as usize as *mut c_void
}

fn addr<T>(at: *mut T) -> u32 {
    at as usize as u32
}

/// A C string in the game's memory at `at`, up to its NUL (none for 0).
///
/// # Safety
/// `at` is 0 or a NUL-ended string the game keeps while the hook runs.
unsafe fn c_string<'a>(at: u32) -> Option<&'a [u8]> {
    if at == 0 {
        return None;
    }
    let start = at as usize as *const u8;
    let mut n = 0;
    // SAFETY: the caller's: readable up to and with its NUL.
    while unsafe { start.add(n).read() } != 0 {
        n += 1;
    }
    // SAFETY: the n bytes just read.
    Some(unsafe { core::slice::from_raw_parts(start, n) })
}

/// A query a cook answers (query_result_for_cook), by its address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Query(NonZeroU32);

impl Query {
    fn at(raw: u32) -> Option<Query> {
        NonZeroU32::new(raw).map(Query)
    }

    fn ptr(self) -> *mut c_void {
        ptr(self.0.get())
    }
}

/// A model instance's surface a bake is for: its textures by type, by its
/// address (0 when the game gave none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surface(u32);

impl Surface {
    /// The slot of the texture of `kind` (four, by type).
    fn slot(self, kind: u32) -> *mut u32 {
        // SAFETY: the address of a slot: computed, not read.
        unsafe { raw::baked_surface::textures(ptr(self.0)).wrapping_add(kind as usize) }
    }

    /// Whether the surface's slot of `kind` (below 4) holds `texture`.
    pub fn holds(self, kind: u32, texture: Texture) -> bool {
        // SAFETY: a surface the game gave a hook (its four slots), `kind`
        // below 4 (the callers').
        unsafe { read(self.slot(kind)) == texture.0.get() }
    }
}

/// A render_model_instance, reference counted (+0D0h, `lock xadd`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model(NonZeroU32);

/// A count the game changes with `lock xadd` (a model instance's, an
/// unmanaged resource's).
///
/// # Safety
/// `at` is such a count: an aligned i32 of a live object.
unsafe fn count<'a>(at: *mut i32) -> &'a AtomicI32 {
    // SAFETY: the caller's.
    unsafe { AtomicI32::from_ptr(at) }
}

impl Model {
    fn references(self) -> *mut i32 {
        // SAFETY: the address of the count: computed, not read.
        unsafe { raw::render_model_instance::references(ptr(self.0.get())) }
    }

    /// One reference more (`lock xadd [model+0D0h], 1`, as translate_query
    /// takes one).
    pub fn hold(self) {
        // SAFETY: a model instance the game keeps while a reference is held.
        unsafe { count(self.references()) }.fetch_add(1, Ordering::SeqCst);
    }

    /// A reference let go; the last destroys it (render_model_instance::
    /// destroy 0xE0210, as translate_query's end does, 0x12D443..0x12D45A).
    pub fn release(self) {
        // SAFETY: as hold.
        if unsafe { count(self.references()) }.fetch_add(-1, Ordering::SeqCst) == 1 {
            // SAFETY: the count reached 0: the game's own destroy, ECX the
            // instance and EAX its count, as its callers give them.
            unsafe { raw::engine_release_object(ptr(self.0.get()), self.references().cast()) }
        }
    }
}

/// A texture resource, reference counted (+4, plain `inc` / `add -1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Texture(NonZeroU32);

impl Texture {
    fn at(raw: u32) -> Option<Texture> {
        NonZeroU32::new(raw).map(Texture)
    }

    fn references(self) -> *mut u32 {
        // SAFETY: the address of the count: computed, not read.
        unsafe { raw::texture_resource::references(ptr(self.0.get())) }
    }

    /// One reference more.
    pub fn hold(self) {
        // SAFETY: a texture the game keeps while a reference is held; the
        // game counts it with plain adds.
        unsafe { write(self.references(), read(self.references()).wrapping_add(1)) }
    }

    /// A reference let go; the last releases it to the resource manager
    /// (0x3BEE00, the device [0x226047C]), as 0x12C6D0 does.
    pub fn release(self) {
        // SAFETY: as hold.
        let left = unsafe { read(self.references()).wrapping_sub(1) };
        // SAFETY: as hold.
        unsafe { write(self.references(), left) }
        if left == 0 {
            // SAFETY: the count reached 0: the game's release, EAX the
            // texture, the device the game's global.
            unsafe {
                let device = read(raw::image::RENDER_DEVICE as usize as *mut u32);
                raw::engine_resource_release(ptr(self.0.get()), ptr(device));
            }
        }
    }
}

/// The baker cook a query was translated by (translate_query's ECX).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cook(u32);

/// A query that waits for another's texture: what giving it the texture, or
/// making it again, needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Waiter {
    pub query: Query,
    pub surface: Surface,
    /// The reference to its model instance the look-up took (none for a
    /// query given a held texture at once).
    pub model: Option<Model>,
    pub kind: u32,
    pub cook: Cook,
}

/// cook_base::result_enum's success, assert on fail, no error type: what
/// on_baked_texture_loaded finishes a loaded texture's query with.
const SUCCESS: (u32, u32, u32) = (3, 1, 0);
/// A baked_texture_resource's bytes (0x12DDBA `mov eax,108h`).
const RESOURCE_SIZE: u32 = 0x108;

impl Waiter {
    /// The waiter's surface slot holds `texture` (one reference more, the
    /// old one let go), its query has the texture's resource and is
    /// finished with success, its model instance's reference let go: the
    /// game's own sequence of on_baked_texture_loaded (0x12DD53..0x12DE0E;
    /// the slot as 0x12C6D0 sets it).
    pub fn give(&self, texture: Texture) {
        texture.hold();
        let slot = self.surface.slot(self.kind);
        // SAFETY: the waiter's surface (its slots) and type below 4.
        let old = unsafe { read(slot) };
        // SAFETY: as above.
        unsafe { write(slot, texture.0.get()) }
        if let Some(old) = Texture::at(old) {
            old.release();
        }
        // SAFETY: the game's functions in on_baked_texture_loaded's order,
        // with its arguments: the allocation's name, 108h bytes of the
        // allocator [1F1D960h], the resource made in them (none when the
        // allocation failed), held for the query (+0D0h, `lock xadd`), the
        // query's resource set and the query finished.
        unsafe {
            let allocator = read(raw::image::BAKED_RESOURCE_ALLOCATOR as usize as *mut u32);
            let name = raw::engine_allocation_name(ptr(raw::image::BAKED_RESOURCE_NAME));
            let memory = raw::engine_malloc(ptr(allocator), RESOURCE_SIZE, name);
            let resource = if memory.is_null() { memory } else { raw::engine_baked_texture_resource(memory) };
            if !resource.is_null() {
                count(raw::unmanaged_resource::references(resource)).fetch_add(1, Ordering::SeqCst);
            }
            raw::engine_set_unmanaged_resource(self.query.ptr(), resource,
                                               ptr(raw::image::UNMANAGED_RESOURCE_ALLOCATOR), RESOURCE_SIZE);
            raw::engine_finish_query(self.query.ptr(), SUCCESS.0, SUCCESS.1, SUCCESS.2);
        }
        if let Some(model) = self.model {
            model.release();
        }
    }

    /// The waiter's query made again (translate_query 0x12D1D0: it bakes or
    /// loads, or waits for another), then its model instance's reference
    /// let go.
    pub fn retranslate(&self) {
        // SAFETY: the cook that translated the query, the query not
        // finished (it waited); translate_query is thiscall (the cook) and
        // pops the query.
        unsafe { raw::engine_translate_query(ptr(self.cook.0), self.query.ptr()) }
        if let Some(model) = self.model {
            model.release();
        }
    }
}

/// A bake's parameters (+7A0h the type, +790h the model instance whose
/// surface it bakes).
#[derive(Clone, Copy, Debug)]
pub struct Parameters(NonZeroU32);

impl Parameters {
    fn at(raw: u32) -> Option<Parameters> {
        NonZeroU32::new(raw).map(Parameters)
    }

    /// The texture's type (0 d, 1 n, 2 f, 3 s; the game's value, unchecked).
    pub fn kind(self) -> u32 {
        // SAFETY: a bake's parameters the game gave the site.
        unsafe { read(raw::bake_parameters::kind(ptr(self.0.get()))) }
    }

    /// The model instance whose surface it bakes.
    pub fn model(self) -> Option<Model> {
        // SAFETY: as kind.
        NonZeroU32::new(unsafe { read(raw::bake_parameters::model(ptr(self.0.get()))) }).map(Model)
    }
}

/// `bake.lookup`, translate_query's look-up of a saved texture, before
/// `lea edx,[esp+1B8h] / push edx / call 0x131ED0`: EAX the cook, EBX the
/// surface, [ebp+8] the query, [esp+1Ch] the parameters, [esp+1B8h] the
/// path (a buffer_string: its text's address first).
pub struct Lookup<'a> {
    pub path: Option<&'a [u8]>,
    pub query: Option<Query>,
    pub surface: Surface,
    pub parameters: Option<Parameters>,
    pub cook: Cook,
}

impl<'a> Lookup<'a> {
    pub fn at(ctx: &'a Context<'_>) -> Lookup<'a> {
        let esp = ptr(ctx.site_esp());
        // SAFETY: translate_query's frame at the site: its path's
        // buffer_string, its parameters and its query (by EBP), as the
        // site's own instructions and 0x131ED0 read them; the path's text is
        // NUL-ended.
        unsafe {
            Lookup {
                path: c_string(read(raw::translate_query_stack::path(esp))),
                query: Query::at(read(raw::translate_query_frame::query(ptr(ctx.reg(Reg::Ebp))))),
                surface: Surface(ctx.reg(Reg::Ebx)),
                parameters: Parameters::at(read(raw::translate_query_stack::parameters(esp))),
                cook: Cook(ctx.reg(Reg::Eax)),
            }
        }
    }
}

/// A texture made and put in its surface's slot (`bake.baked`,
/// process_compression; `bake.loaded`, on_baked_texture_loaded), before the
/// `call 0x420789` that names the resource's allocation (ECX 0xAC3334).
#[derive(Clone, Copy, Debug)]
pub struct Made {
    pub query: Option<Query>,
    pub texture: Option<Texture>,
    pub surface: Surface,
    pub kind: u32,
}

impl Made {
    /// process_compression's: [esp+10h] the texture, +604h the query, +610h
    /// the surface, +620h the type.
    pub fn baked(ctx: &Context<'_>) -> Made {
        let esp = ptr(ctx.site_esp());
        // SAFETY: process_compression's frame at the site, its locals.
        unsafe {
            Made {
                query: Query::at(read(raw::process_compression_stack::query(esp))),
                texture: Texture::at(read(raw::process_compression_stack::texture(esp))),
                surface: Surface(read(raw::process_compression_stack::surface(esp))),
                kind: read(raw::process_compression_stack::kind(esp)),
            }
        }
    }

    /// on_baked_texture_loaded's: EDI the texture, [ebp+0Ch] the
    /// parameters (their type), +10h the surface, +14h the query.
    pub fn loaded(ctx: &Context<'_>) -> Made {
        let ebp = ptr(ctx.reg(Reg::Ebp));
        // SAFETY: on_baked_texture_loaded's frame (EBP) at the site, its
        // arguments; the parameters are a bake's.
        unsafe {
            let parameters = read(raw::on_baked_texture_loaded_frame::parameters(ebp));
            Made {
                query: Query::at(read(raw::on_baked_texture_loaded_frame::query(ebp))),
                texture: Texture::at(ctx.reg(Reg::Edi)),
                surface: Surface(read(raw::on_baked_texture_loaded_frame::surface(ebp))),
                kind: read(raw::bake_parameters::kind(ptr(parameters))),
            }
        }
    }

    /// The `call 0x420789` the site replaces, done: EAX the allocation's
    /// name (`lea eax,[ecx+8]`), ECX kept.
    pub fn name_allocation(ctx: &mut Context<'_>) {
        // SAFETY: the address of the name: computed, not read.
        let name = unsafe { raw::named_object::name(ptr(ctx.reg(Reg::Ecx))) };
        ctx.set_reg(Reg::Eax, addr(name));
    }
}

/// `bake.finish`, finish_query's start (stdcall: the query, the result,
/// assert on fail, the error type; `ret 10h`).
#[derive(Clone, Copy, Debug)]
pub struct Finish {
    pub query: Option<Query>,
    arguments: [u32; 4],
}

impl Finish {
    pub fn at(ctx: &Context<'_>) -> Finish {
        let arguments = [ctx.stack(4), ctx.stack(8), ctx.stack(12), ctx.stack(16)];
        Finish { query: Query::at(arguments[0]), arguments }
    }

    /// finish_query with the same arguments, through its start (the hook
    /// there lets it run: the query no longer owns a path).
    pub fn call(&self) {
        let [query, result, assert, error] = self.arguments;
        // SAFETY: the game's finish_query (stdcall) with the arguments its
        // caller gave it.
        unsafe { raw::engine_finish_query(ptr(query), result, assert, error) }
    }
}

/// `bake.game_start`, game::game's `call 0x1E8DA0` (EAX the folder's
/// physical path, a buffer_string).
pub struct Folder<'a> {
    string: u32,
    hook: PhantomData<&'a Context<'a>>,
}

impl<'a> Folder<'a> {
    pub fn at(ctx: &'a Context<'_>) -> Folder<'a> {
        Folder { string: ctx.reg(Reg::Eax), hook: PhantomData }
    }

    /// The path's bytes (begin..end); none for no begin or an end before it.
    pub fn path(&self) -> Option<&'a [u8]> {
        // SAFETY: the buffer_string the game hands 0x1E8DA0, its text the
        // range begin..end (checked to be one) while the hook runs.
        unsafe {
            let begin = read(raw::buffer_string::begin(ptr(self.string)));
            let end = read(raw::buffer_string::end(ptr(self.string)));
            (begin != 0 && end >= begin)
                .then(|| core::slice::from_raw_parts(begin as usize as *const u8, (end - begin) as usize))
        }
    }

    /// The game's delete of the folder (0x1E8DA0, SHFileOperationA
    /// FO_DELETE), the call the site makes.
    pub fn delete(&self) {
        // SAFETY: the buffer_string the site gives it, in EAX.
        unsafe { raw::engine_delete_folder(ptr(self.string)) }
    }
}

/// The patch revision (patch 7's `mov byte [esp+27h],REVISION` immediate).
pub fn revision() -> u8 {
    // SAFETY: a byte of the exe's code, never written.
    unsafe { read(raw::image::PATCH_REVISION as usize as *mut u8) }
}

/// `bake.check_crc`, check_loaded_baked_texture's `cmp eax,[edi+114h]`:
/// EAX the CRC of the bytes loaded, EDI the path's record.
pub struct CheckCrc {
    record: u32,
    pub loaded: u32,
}

impl CheckCrc {
    pub fn at(ctx: &Context<'_>) -> CheckCrc {
        CheckCrc { record: ctx.reg(Reg::Edi), loaded: ctx.reg(Reg::Eax) }
    }

    /// The record's CRC (0: none yet).
    pub fn crc(&self) -> u32 {
        // SAFETY: the record the game compares (EDI, a record of the list).
        unsafe { read(raw::baked_texture_data::crc(ptr(self.record))) }
    }

    pub fn set_crc(&self, crc: u32) {
        // SAFETY: as crc.
        unsafe { write(raw::baked_texture_data::crc(ptr(self.record)), crc) }
    }
}

/// A record's path text: 104h bytes (+0Ch, before its '/').
pub const RECORD_TEXT: usize = 0x104;

/// The baker's list of saved textures (its +230h: begin, end, capacity).
pub struct List;

fn global(at: u32) -> *mut u32 {
    at as usize as *mut u32
}

fn next(record: u32) -> u32 {
    // SAFETY: the address of the record after it: computed, not read.
    addr(unsafe { raw::baked_texture_data::next(ptr(record)) })
}

impl List {
    /// The size of the first record of `path` (its file's bytes), 0 for
    /// none.
    pub fn size_of(path: &[u8]) -> u32 {
        // SAFETY: the baker static's list (image globals), each record of
        // begin..end with its NUL-ended text.
        unsafe {
            let (mut record, end) = (read(global(raw::image::BAKER_LIST_BEGIN)), read(global(raw::image::BAKER_LIST_END)));
            while record < end {
                if c_string(addr(raw::baked_texture_data::text(ptr(record)))) == Some(path) {
                    return read(raw::baked_texture_data::size(ptr(record)));
                }
                record = next(record);
            }
        }
        0
    }

    /// The record past the list's end, when the list's capacity has room
    /// for it.
    pub fn free() -> Option<FreeRecord> {
        // SAFETY: the baker static's list (image globals).
        let (end, capacity) = unsafe { (read(global(raw::image::BAKER_LIST_END)), read(global(raw::image::BAKER_LIST_CAPACITY))) };
        (next(end) <= capacity).then_some(FreeRecord(end))
    }
}

/// The record at the list's end, not in it yet.
pub struct FreeRecord(u32);

impl FreeRecord {
    /// Its path text (104h bytes).
    pub fn text(&mut self) -> &mut [u8; RECORD_TEXT] {
        // SAFETY: the room past the list's end, inside its capacity (free);
        // nothing of the game's is there.
        unsafe { &mut *raw::baked_texture_data::text(ptr(self.0)).cast::<[u8; RECORD_TEXT]>() }
    }

    /// The record put in the list as the baker's push_back makes one, for a
    /// file of `size` bytes: its path the text written (begin, end at its
    /// NUL, capacity), '/' after it, CRC 0 (the first load's taken), the
    /// size, the next age, not loading; the list's end one record on, its
    /// bytes grown by the size.
    pub fn push(mut self, size: u32) {
        let length = self.text().iter().position(|&b| b == 0).unwrap_or(RECORD_TEXT) as u32;
        let at = ptr(self.0);
        // SAFETY: the record past the list's end (free), then the list's
        // fields and the ages' counter (image globals).
        unsafe {
            let text = addr(raw::baked_texture_data::text(at));
            write(raw::baked_texture_data::path_begin(at), text);
            write(raw::baked_texture_data::path_end(at), text.wrapping_add(length));
            write(raw::baked_texture_data::path_capacity(at), text.wrapping_add(RECORD_TEXT as u32));
            write(raw::baked_texture_data::separator(at), b'/');
            write(raw::baked_texture_data::crc(at), 0);
            write(raw::baked_texture_data::size(at), size);
            let age = read(global(raw::image::BAKER_AGES)).wrapping_add(1);
            write(global(raw::image::BAKER_AGES), age);
            write(raw::baked_texture_data::age(at), age);
            write(raw::baked_texture_data::loading(at), 0);
            write(global(raw::image::BAKER_LIST_END), next(self.0));
            let bytes = read(global(raw::image::BAKER_LIST_BYTES)).wrapping_add(size);
            write(global(raw::image::BAKER_LIST_BYTES), bytes);
        }
    }
}

//! The camouflage/decal baker (vostok::render::
//! camouflage_decal_texture_type_baker_cook, the static 0xA264E80) and the
//! resource queries it answers: what its sites' registers and frames hold,
//! its list of saved textures, and the game's own sequences a patch repeats
//! (a texture given to a query, a query made again, a reference let go).
//!
//! The baker bakes an item's camouflage and decals into a texture of each
//! type ([`TextureKind`]) for a model instance's surface, saves it as
//! user_data/streamed_textures/<name>.dds and lists it (records of 124h
//! bytes, begin/end/capacity at +230h: [`List`]), so a later query loads the
//! file instead.
//!
//! A query, a surface, a model instance, a texture and the cook are ids a
//! patch may keep between hooks ([`Query`], [`Surface`], [`Model`],
//! [`Texture`], [`Cook`]): the game keeps a query until finish_query
//! finishes it, a model instance while a reference to it is held (the
//! look-up takes one for a query it keeps waiting), a texture while a
//! reference to it is held. Only a site's view makes one.

use core::ffi::c_void;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicU32, Ordering};

use super::context::Context;
use super::ui::c_string;
use super::world::read;
use crate::raw;

/// A record's path text: 104h bytes (+0Ch, before its '/').
pub const RECORD_TEXT: usize = 0x104;

/// A texture's type, the bake parameters' +7A0h (process_compression's
/// +620h): 0 d, 1 n, 2 f, 3 s; the surface's slot of the same index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureKind {
    D,
    N,
    F,
    S,
}

impl TextureKind {
    /// The game's value; none for one past the four (the game's queries
    /// have others: none of the slots).
    pub(super) fn from_raw(kind: u32) -> Option<TextureKind> {
        match kind {
            0 => Some(TextureKind::D),
            1 => Some(TextureKind::N),
            2 => Some(TextureKind::F),
            3 => Some(TextureKind::S),
            _ => None,
        }
    }

    fn slot(self) -> usize {
        self as usize
    }

    /// The game's value.
    pub(super) fn raw(self) -> u32 {
        self as u32
    }
}

/// A query a cook answers (query_result_for_cook).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Query(raw::QueryResultForCook);

/// A model instance's surface a bake is for (render_surface_instance): its
/// textures by type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surface(raw::RenderSurfaceInstance);

/// A render_model_instance, reference counted (+0D0h, `lock xadd`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model(raw::RenderModelInstance);

/// A texture resource (res_texture), reference counted (+4, plain `inc` /
/// `add -1`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Texture(raw::ResTexture);

/// The baker cook a query was translated by (translate_query's ECX).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cook(raw::CamouflageDecalTextureTypeBakerCook);

// SAFETY: these are ids a patch keeps in its Lock between hooks, on the
// game's main and resource threads; each is read through only inside a hook
// while the game keeps it alive (a query until it is finished, a model
// instance or a texture while a reference of ours is held, the cook a
// static), and its counts are changed as the game changes them (atomically
// for a model instance, plainly for a texture, as the baker's threads do).
unsafe impl Send for Query {}
// SAFETY: as Query's.
unsafe impl Send for Surface {}
// SAFETY: as Query's.
unsafe impl Send for Model {}
// SAFETY: as Query's.
unsafe impl Send for Texture {}
// SAFETY: as Query's.
unsafe impl Send for Cook {}

impl Surface {
    fn slot(self, kind: TextureKind) -> raw::TextureSlot {
        // SAFETY: a surface the game gave a hook: its four slots, `kind` one
        // of them.
        unsafe { self.0.textures().at(kind.slot()) }
    }

    /// Whether the surface's slot of `kind` holds `texture`.
    pub fn holds(self, kind: TextureKind, texture: Texture) -> bool {
        // SAFETY: as slot.
        unsafe { self.slot(kind).texture() == Some(texture.0) }
    }
}

impl Model {
    /// The model instance a frame or field of vostok gave.
    pub(super) fn of(model: raw::RenderModelInstance) -> Model {
        Model(model)
    }

    /// One reference more (`lock xadd [model+0D0h], 1`, as translate_query
    /// takes one).
    pub fn hold(self) {
        // SAFETY: a model instance the game keeps while a reference is held.
        unsafe { AtomicU32::from_ptr(self.0.references()) }.fetch_add(1, Ordering::SeqCst);
    }

    /// A reference let go; the last destroys it (render_model_instance::
    /// destroy 0xE0210, as translate_query's end does, 0x12D443..0x12D45A).
    pub fn release(self) {
        // SAFETY: as hold; at 0 the game's own destroy, ECX the instance and
        // EAX its count, as its callers give them.
        unsafe {
            let count = self.0.references();
            if AtomicU32::from_ptr(count).fetch_sub(1, Ordering::SeqCst) == 1 {
                raw::engine_release_model_instance(Some(self.0), count);
            }
        }
    }
}

impl Texture {
    /// The texture a frame, field or function of vostok gave.
    pub(super) fn of(texture: raw::ResTexture) -> Texture {
        Texture(texture)
    }

    pub(super) fn raw(self) -> raw::ResTexture {
        self.0
    }

    /// One reference more.
    pub fn hold(self) {
        // SAFETY: a texture the game keeps while a reference is held; the
        // game counts it with plain adds.
        unsafe {
            let count = self.0.references();
            count.write_volatile(read(count).wrapping_add(1));
        }
    }

    /// A reference let go; the last releases it to the resource manager
    /// (0x3BEE00, the manager [0x226047C]), as 0x12C6D0 does.
    pub fn release(self) {
        // SAFETY: as hold; at 0 the game's release, EAX the texture, the
        // manager the render's global.
        unsafe {
            let count = self.0.references();
            let left = read(count).wrapping_sub(1);
            count.write_volatile(left);
            if left == 0 {
                raw::engine_resource_release(Some(self.0), raw::image::render_resources().manager());
            }
        }
    }
}

/// A query that waits for another's texture: what giving it the texture, or
/// making it again, needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Waiter {
    pub query: Query,
    pub surface: Surface,
    /// The reference to its model instance the look-up took (none for a
    /// query given a held texture at once).
    pub model: Option<Model>,
    pub kind: TextureKind,
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
        // SAFETY: the waiter's surface's slot of its type.
        let old = unsafe { slot.texture() };
        // SAFETY: as above.
        unsafe { slot.set_texture(Some(texture.0)) }
        if let Some(old) = old {
            Texture(old).release();
        }
        // SAFETY: the game's functions in on_baked_texture_loaded's order,
        // with its arguments: the allocation's name, 108h bytes of the
        // render's allocator [1F1D960h], the resource made in them (none
        // when the allocation failed), held for the query (+0D0h, `lock
        // xadd`), the query's resource set and the query finished.
        unsafe {
            let allocator = raw::image::render_allocator().allocator()
                .map_or(null_mut(), |a| a.address() as usize as *mut c_void);
            let name = raw::engine_allocation_name(raw::image::BAKED_RESOURCE_NAME as usize as *const c_void);
            let memory = raw::engine_malloc(allocator, RESOURCE_SIZE, name);
            let resource = if memory.is_null() { None } else { raw::engine_baked_texture_resource(memory) };
            if let Some(resource) = resource {
                AtomicU32::from_ptr(resource.references()).fetch_add(1, Ordering::SeqCst);
            }
            raw::engine_set_unmanaged_resource(Some(self.query.0), resource,
                                               raw::image::UNMANAGED_RESOURCE_ALLOCATOR as usize as *mut c_void,
                                               RESOURCE_SIZE);
            raw::engine_finish_query(Some(self.query.0), SUCCESS.0, SUCCESS.1, SUCCESS.2);
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
        unsafe { raw::engine_translate_query(Some(self.cook.0), Some(self.query.0)) }
        if let Some(model) = self.model {
            model.release();
        }
    }
}

/// A bake's parameters (+7A0h the type, +790h the model instance whose
/// surface it bakes).
#[derive(Clone, Copy, Debug)]
pub struct Parameters(raw::BakeParameters);

impl Parameters {
    /// The texture's type; none for a value past the four.
    pub fn kind(self) -> Option<TextureKind> {
        // SAFETY: a bake's parameters the game gave the site.
        TextureKind::from_raw(unsafe { read(self.0.kind()) })
    }

    /// The model instance whose surface it bakes.
    pub fn model(self) -> Option<Model> {
        // SAFETY: as kind.
        unsafe { self.0.model() }.map(Model)
    }
}

/// `bake.lookup`, translate_query before its look-up of a saved texture
/// (`lea edx,[esp+1B8h] / push edx / call 0x131ED0`): EAX the cook, EBX the
/// surface, [ebp+8] the query, [esp+1Ch] the parameters, [esp+1B8h] the
/// path (a buffer_string).
pub struct Lookup<'t> {
    /// The path's text; none when it has none, or one too long for a record.
    pub path: Option<&'t [u8]>,
    pub query: Option<Query>,
    pub surface: Option<Surface>,
    pub parameters: Option<Parameters>,
    pub cook: Option<Cook>,
}

impl<'t> Lookup<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Lookup<'t> {
        // SAFETY: translate_query's frame at the site: its path's
        // buffer_string, its parameters and its query (by EBP), as the
        // site's own instructions and 0x131ED0 read them; the path's text is
        // NUL-ended, alive while the query is translated.
        unsafe {
            let frame = raw::site::bake_lookup::frame(ctx);
            Lookup {
                path: frame.and_then(|f| c_string(f.path().begin(), RECORD_TEXT)).map(|p| p.to_bytes()),
                query: raw::site::bake_lookup::query_frame(ctx).and_then(|f| f.query()).map(Query),
                surface: raw::site::bake_lookup::surface(ctx).map(Surface),
                parameters: frame.and_then(|f| f.parameters()).map(Parameters),
                cook: raw::site::bake_lookup::cook(ctx).map(Cook),
            }
        }
    }
}

/// A texture made and put in its surface's slot (`bake.baked`,
/// process_compression; `bake.loaded`, on_baked_texture_loaded), once
/// its resource's allocation is named (`mov ecx,0AC3334h / call 0x420789`).
#[derive(Clone, Copy, Debug)]
pub struct Made {
    pub query: Option<Query>,
    pub texture: Option<Texture>,
    pub surface: Option<Surface>,
    /// Its type; none for a value past the four.
    pub kind: Option<TextureKind>,
}

impl Made {
    /// process_compression's: [esp+10h] the texture, +604h the query, +610h
    /// the surface, +620h the type.
    pub fn baked(ctx: &Context<'_>) -> Option<Made> {
        // SAFETY: process_compression's frame at the site, its locals.
        unsafe {
            let frame = raw::site::bake_baked::frame(ctx)?;
            Some(Made {
                query: frame.query().map(Query),
                texture: frame.texture().map(Texture),
                surface: frame.surface().map(Surface),
                kind: TextureKind::from_raw(read(frame.kind())),
            })
        }
    }

    /// on_baked_texture_loaded's: EDI the texture, [ebp+0Ch] the
    /// parameters (their type), +10h the surface, +14h the query.
    pub fn loaded(ctx: &Context<'_>) -> Option<Made> {
        // SAFETY: on_baked_texture_loaded's frame (EBP) at the site, its
        // arguments; the parameters are a bake's.
        unsafe {
            let frame = raw::site::bake_loaded::frame(ctx)?;
            Some(Made {
                query: frame.query().map(Query),
                texture: raw::site::bake_loaded::texture(ctx).map(Texture),
                surface: frame.surface().map(Surface),
                kind: frame.parameters().and_then(|p| Parameters(p).kind()),
            })
        }
    }
}

/// `bake.finish`, finish_query's start (stdcall: the query, the result,
/// assert on fail, the error type; `ret 10h`).
#[derive(Clone, Copy, Debug)]
pub struct Finish {
    pub query: Option<Query>,
    result: u32,
    assert_on_fail: u32,
    error: u32,
}

impl Finish {
    pub fn at(ctx: &Context<'_>) -> Option<Finish> {
        // SAFETY: the function's start: its four stack arguments.
        unsafe {
            let frame = raw::site::bake_finish::frame(ctx)?;
            Some(Finish {
                query: frame.query().map(Query),
                result: read(frame.result()),
                assert_on_fail: read(frame.assert_on_fail()),
                error: read(frame.error()),
            })
        }
    }

    /// finish_query with the same arguments, through its start (the hook
    /// there lets it run: the query no longer owns a path).
    pub fn call(&self) {
        // SAFETY: the game's finish_query (stdcall) with the arguments its
        // caller gave it.
        unsafe {
            raw::engine_finish_query(self.query.map(|q| q.0), self.result, self.assert_on_fail, self.error)
        }
    }
}

/// `bake.game_start`, game::game before its delete of the folder: the
/// folder's physical path (a buffer_string, [esp+470h]).
pub struct Folder<'t> {
    string: raw::BufferString,
    _hook: core::marker::PhantomData<&'t Context<'t>>,
}

impl<'t> Folder<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<Folder<'t>> {
        // SAFETY: game::game's local at the site, by its ESP.
        let string = unsafe { raw::site::bake_game_start::frame(ctx)?.folder() };
        Some(Folder { string, _hook: core::marker::PhantomData })
    }

    /// The path's bytes (begin..end); none for no begin or an end before it.
    pub fn path(&self) -> Option<&'t [u8]> {
        // SAFETY: the buffer_string the game hands 0x1E8DA0, its text the
        // range begin..end (checked to be one) while the hook runs.
        unsafe {
            let (begin, end) = (self.string.begin(), self.string.end());
            let length = (end as usize).checked_sub(begin as usize)?;
            (!begin.is_null()).then(|| core::slice::from_raw_parts(begin.cast_const(), length))
        }
    }

    /// The game's delete of the folder (0x1E8DA0, SHFileOperationA
    /// FO_DELETE), the call the site makes.
    pub fn delete(&self) {
        // SAFETY: the buffer_string the site gives it, in EAX.
        unsafe { raw::engine_delete_folder(Some(self.string)) }
    }
}

/// The patch revision (patch 7's `mov byte [esp+27h],REVISION` immediate).
pub fn revision() -> u8 {
    // SAFETY: a byte of the exe's code, never written while it runs.
    unsafe { read(raw::image::patch_revision().revision()) }
}

/// `bake.check_crc`, check_loaded_baked_texture's `cmp eax,[edi+114h]`:
/// EAX the CRC of the bytes loaded, EDI the path's record.
pub struct CheckCrc {
    record: raw::BakedTextureData,
    pub loaded: u32,
}

impl CheckCrc {
    pub fn at(ctx: &Context<'_>) -> Option<CheckCrc> {
        // SAFETY: the site's registers: the record compared and the CRC.
        unsafe {
            Some(CheckCrc { record: raw::site::bake_check_crc::record(ctx)?,
                            loaded: raw::site::bake_check_crc::loaded(ctx) })
        }
    }

    /// The record's CRC (0: none yet).
    pub fn crc(&self) -> u32 {
        // SAFETY: the record the game compares (EDI, a record of the list).
        unsafe { read(self.record.crc()) }
    }

    pub fn set_crc(&self, crc: u32) {
        // SAFETY: as crc.
        unsafe { self.record.crc().write_volatile(crc) }
    }
}

/// The baker's list of saved textures (its +230h: begin, end, capacity).
pub struct List;

impl List {
    /// The size of the first record of `path` (its file's bytes), 0 for
    /// none.
    pub fn size_of(path: &[u8]) -> u32 {
        let baker = raw::image::baker();
        // SAFETY: the baker static's list, each record of begin..end with
        // its NUL-ended text.
        unsafe {
            let (Some(mut record), end) = (baker.list_begin(), baker.list_end().map_or(0, |e| e.address())) else {
                return 0;
            };
            while record.address() < end {
                if c_string(record.text(), RECORD_TEXT).map(|text| text.to_bytes()) == Some(path) {
                    return read(record.file_bytes());
                }
                record = record.at(1);
            }
        }
        0
    }

    /// The record past the list's end, when the list's capacity has room
    /// for it.
    pub fn free() -> Option<FreeRecord> {
        let baker = raw::image::baker();
        // SAFETY: the baker static's list.
        let (end, capacity) = unsafe { (baker.list_end()?, baker.list_capacity().map_or(0, |c| c.address())) };
        // SAFETY: the address of the record after it: computed, not read.
        (unsafe { end.at(1) }.address() <= capacity).then_some(FreeRecord(end))
    }
}

/// The record at the list's end, not in it yet.
pub struct FreeRecord(raw::BakedTextureData);

impl FreeRecord {
    /// Its path text (104h bytes).
    pub fn text(&mut self) -> &mut [u8; RECORD_TEXT] {
        // SAFETY: the room past the list's end, inside its capacity (free);
        // nothing of the game's is there.
        unsafe { &mut *self.0.text().cast::<[u8; RECORD_TEXT]>() }
    }

    /// The record put in the list as the baker's push_back makes one, for a
    /// file of `size` bytes: its path the text written (begin, end at its
    /// NUL, capacity), '/' after it, CRC 0 (the first load's taken), the
    /// size, the next age, not loading; the list's end one record on, its
    /// bytes grown by the size.
    pub fn push(mut self, size: u32) {
        let length = self.text().iter().position(|&b| b == 0).unwrap_or(RECORD_TEXT);
        let (record, baker, ages) = (self.0, raw::image::baker(), raw::image::baker_ages());
        // SAFETY: the record past the list's end (free), then the list's
        // fields and the ages' counter (the baker's statics).
        unsafe {
            let text = record.text();
            record.set_path_begin(text);
            record.set_path_end(text.wrapping_add(length));
            record.set_path_capacity(text.wrapping_add(RECORD_TEXT));
            record.separator().write_volatile(b'/');
            record.crc().write_volatile(0);
            record.file_bytes().write_volatile(size);
            let age = read(ages.last()).wrapping_add(1);
            ages.last().write_volatile(age);
            record.age().write_volatile(age);
            record.loading().write_volatile(0);
            baker.set_list_end(Some(record.at(1)));
            baker.list_bytes().write_volatile(read(baker.list_bytes()).wrapping_add(size));
        }
    }
}

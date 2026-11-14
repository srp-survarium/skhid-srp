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

use core::ffi::{CStr, c_void};
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

    /// Whose bake it is, by its files' name.
    pub fn asker(self) -> Asker {
        // SAFETY: as kind; the name is a buffer_string's text, NUL-ended
        // within a record's path, alive while the bake's query is.
        let name = unsafe { c_string(self.0.name(), RECORD_TEXT) };
        Asker::of_name(name.map_or(&[], |name| name.to_bytes()))
    }

    /// `bake.own_camouflage`'s: on_base_texture_loaded where the camouflage
    /// layer's texture of the bake's type has no name, before the game names
    /// the surface's own (`<diffuse>_camo`, `_camo_nmap`, `_camo_smap`).
    pub fn at_own_camouflage(ctx: &Context<'_>) -> Option<Parameters> {
        // SAFETY: the site's ESI: the bake's parameters, on_base_texture_
        // loaded's second argument (0x12E0DB mov esi,[ebp+0Ch]).
        unsafe { raw::site::bake_own_camouflage::parameters(ctx) }.map(Parameters)
    }
}

/// Who asked for a bake. Its files' name says: patch 44 names a weapon's
/// `srpw<n>_...` (the Rust patch weapon_camo's BakedName); the game's own bakes
/// (a character's part, the decal editor's) are named of the part and never
/// begin so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asker {
    /// The game: a character's part.
    Character,
    /// Patch 44: a weapon's surface.
    Weapon,
}

impl Asker {
    /// A weapon's bake names begin so (before the number).
    const WEAPON: &'static [u8] = b"srpw";

    fn of_name(name: &[u8]) -> Asker {
        if name.starts_with(Asker::WEAPON) { Asker::Weapon } else { Asker::Character }
    }
}

/// A weapon's bake of one surface, its parameters filled by patch 44's
/// bake_surface (weapon_camo_026g.asm) before it asks for the bake
/// (`weapon_camo.bake_name`): what the baker will bake, read once.
pub struct WeaponBake<'c> {
    parameters: raw::CookParameters,
    pub inputs: BakeInputs<'c>,
}

/// What a weapon's bake of one surface is made of.
pub struct BakeInputs<'c> {
    /// The weapon's model path (object/model of its merged config).
    pub model: &'c CStr,
    pub surface: u32,
    /// The camouflage's scale on the surface.
    pub scale: f32,
    /// The two decals' layers, then the camouflage's; none for a layer with
    /// no texture (no decal there, no camouflage).
    pub layers: [Option<BakeLayer<'c>>; 3],
}

/// One layer of a bake, which has a texture: where its textures lie, and
/// their names.
pub struct BakeLayer<'c> {
    pub matrix: [f32; 16],
    /// Diffuse, normal, fresnel, smoothness; an empty name is no texture
    /// (one of them at least has a name).
    pub textures: [&'c CStr; 4],
}

impl<'c> BakeLayer<'c> {
    /// The layer of these texture names: none when it has no texture. Only
    /// a layer with a texture has a matrix: the parameters' constructor
    /// (0x119760) writes none, patch 44 writes the camouflage's and a
    /// decal's with its names (weapon_decals_026g.asm's fill), and the
    /// baker reads a layer's only for a texture it names. So an empty
    /// layer's is what the allocation held before, and `matrix` is not
    /// asked for it.
    fn of(textures: [&'c CStr; 4], matrix: impl FnOnce() -> [f32; 16]) -> Option<BakeLayer<'c>> {
        textures.iter().any(|name| !name.is_empty()).then(|| BakeLayer { matrix: matrix(), textures })
    }
}

impl<'c> WeaponBake<'c> {
    /// `weapon_camo.bake_name`'s: the parameters and the model's path its
    /// caller pushed; none when either is not there or a name is no C string
    /// of a bake's (then nothing names the files, and patch 44 bakes none).
    pub fn at(ctx: &'c Context<'_>) -> Option<WeaponBake<'c>> {
        // SAFETY: the entry's frame: patch 44's call pushed the parameters it
        // allocated and filled, and the model's path of the weapon's merged
        // config, both alive until the call returns, which outlives `ctx`.
        unsafe {
            let frame = raw::site::weapon_camo_bake_name::frame(ctx)?;
            let parameters = frame.parameters()?;
            let model = c_string(frame.model(), RECORD_TEXT)?;
            let first = parameters.layers();
            // the outer none: a name that is no C string; the inner: no texture
            let layer = |index| -> Option<Option<BakeLayer<'c>>> {
                let layer = first.at(index);
                let text = |string: raw::BufferString| c_string(string.begin(), RECORD_TEXT);
                let textures =
                    [text(layer.diffuse())?, text(layer.normal())?, text(layer.fresnel())?, text(layer.smoothness())?];
                Some(BakeLayer::of(textures, || core::array::from_fn(|i| read(layer.matrix().add(i)))))
            };
            let layers = [layer(0)?, layer(1)?, layer(2)?];
            let inputs =
                BakeInputs { model, surface: read(parameters.surface()), scale: read(parameters.scale()), layers };
            Some(WeaponBake { parameters, inputs })
        }
    }

    /// Names its baked files (before the type's letter).
    pub fn name(&self, name: &CStr) {
        // SAFETY: buffer_string::assign on the parameters' own name, which
        // patch 44 left empty; a weapon's name is far shorter than its
        // buffer (104h with its NUL).
        unsafe { raw::engine_string_assign(Some(self.parameters.name()), name.as_ptr().cast()) };
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

/// What the list holds: its records, the records it has room for, and the
/// bytes of its records' files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Held {
    pub records: u32,
    pub room: u32,
    pub bytes: u32,
}

/// A record's bytes.
pub const RECORD_BYTES: usize = raw::BakedTextureData::SIZE;

impl List {
    /// The list's first record and how many it holds; none for a list
    /// with no begin or an end before it.
    fn records() -> Option<(raw::BakedTextureData, usize)> {
        let baker = raw::image::baker();
        // SAFETY: the baker static's list.
        let (begin, end) = unsafe { (baker.list_begin()?, baker.list_end()?) };
        Some((begin, (end.address().checked_sub(begin.address())? as usize) / RECORD_BYTES))
    }

    /// What the list holds; none for a list with no begin.
    pub fn held() -> Option<Held> {
        let (begin, records) = List::records()?;
        let baker = raw::image::baker();
        // SAFETY: the baker static's list.
        let (capacity, bytes) = unsafe { (baker.list_capacity()?, read(baker.list_bytes())) };
        let room = (capacity.address().checked_sub(begin.address())? as usize) / RECORD_BYTES;
        Some(Held { records: records as u32, room: room as u32, bytes })
    }

    /// The list's records in `room` from now on (zeroed bytes of ours, kept
    /// for good), as many as it holds: the game's own storage is 1024
    /// records inside the baker, and its push_back writes past them when
    /// they are full (0x132640: an assertion, then the write). Only an
    /// empty list is moved (nothing to carry over: the game's start, before
    /// the first record); the game reads the list through its begin, end
    /// and capacity alone (its storage's address is in its constructor
    /// 0x12CCE0 and nowhere else). Whether it moved.
    pub fn move_to(room: &'static mut [u8]) -> bool {
        let (baker, count) = (raw::image::baker(), room.len() / RECORD_BYTES);
        if !room.as_ptr().cast::<raw::BakedTextureDataStorage>().is_aligned() || List::records().map(|(_, n)| n) != Some(0)
        {
            return false;
        }
        // SAFETY: `count` records' bytes of ours alone, aligned, for good;
        // the baker static's list, empty.
        unsafe {
            let rooms = core::slice::from_raw_parts_mut(room.as_mut_ptr().cast::<raw::BakedTextureDataStorage>(), count);
            let Some(first) = raw::BakedTextureDataStorage::first(rooms) else { return false };
            baker.set_list_begin(Some(first));
            baker.set_list_end(Some(first));
            baker.set_list_capacity(Some(first.at(count)));
        }
        true
    }

    /// The record of the least age with no load of it under way (the one
    /// remove_unwanted_files lets go first); none when every record is
    /// loading, or the list is empty.
    pub fn oldest() -> Option<Record> {
        let (begin, records) = List::records()?;
        // SAFETY: each record of begin..end.
        (0..records)
            .map(|index| unsafe { begin.at(index) })
            .filter(|&record| unsafe { read(record.loading()) } == 0)
            .min_by_key(|&record| unsafe { read(record.age()) })
            .map(Record)
    }
}

/// A record in the list.
pub struct Record(raw::BakedTextureData);

impl Record {
    /// Its path (user_data/streamed_textures/<name>.dds); none without a
    /// NUL in its text.
    pub fn path(&self) -> Option<&[u8]> {
        // SAFETY: the record's text, its own 104h bytes, while it is in the
        // list (the borrow is this Record's; remove takes it).
        unsafe { c_string(self.0.text(), RECORD_TEXT).map(|text| text.to_bytes()) }
    }

    /// The record out of the list: the list's last record takes its place
    /// (the game's look-up walks the list by path, and its own trim sorts
    /// it: no order is kept), the list's end one record back, its bytes
    /// less this record's. Its file's bytes.
    pub fn remove(self) -> u32 {
        let (record, baker) = (self.0, raw::image::baker());
        let Some((begin, records)) = List::records().filter(|&(_, records)| records > 0) else { return 0 };
        // SAFETY: the record and the last one, both in begin..end (the
        // list has records, and a Record is one of them); then the list's
        // fields.
        unsafe {
            let (bytes, last) = (read(record.file_bytes()), begin.at(records - 1));
            if last != record {
                let (text, length) = (record.text(), (last.path_end() as usize).wrapping_sub(last.text() as usize));
                core::ptr::copy_nonoverlapping(last.text().cast_const(), text, RECORD_TEXT);
                record.set_path_begin(text);
                record.set_path_end(text.wrapping_add(length.min(RECORD_TEXT - 1)));
                record.set_path_capacity(text.wrapping_add(RECORD_TEXT));
                record.separator().write_volatile(read(last.separator()));
                record.crc().write_volatile(read(last.crc()));
                record.file_bytes().write_volatile(read(last.file_bytes()));
                record.age().write_volatile(read(last.age()));
                record.loading().write_volatile(read(last.loading()));
            }
            baker.set_list_end(Some(last));
            baker.list_bytes().write_volatile(read(baker.list_bytes()).wrapping_sub(bytes));
            bytes
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Asker, BakeLayer};

    #[test]
    fn a_layer_with_no_texture_has_no_matrix_and_its_is_never_read() {
        assert!(BakeLayer::of([c""; 4], || panic!("an empty layer's matrix was read")).is_none());
        for named in 0..4 {
            let mut textures = [c""; 4];
            textures[named] = c"character/human/decal_3";
            let layer = BakeLayer::of(textures, || [0.5; 16]).expect("a layer with a texture");
            assert_eq!((layer.matrix, layer.textures), ([0.5; 16], textures));
        }
    }

    #[test]
    fn a_bake_is_a_weapon_s_by_its_name() {
        assert_eq!(Asker::of_name(b"srpw1_character_human_camo_premium_city_camo_weapons_ak_12_ak_12_0d"), Asker::Weapon);
        assert_eq!(Asker::of_name(b"srpw1__weapons_sks_sks_1_decal_1_1_8a30ce0020080000n"), Asker::Weapon);
        assert_eq!(Asker::of_name(b"edge_researcher_03_legs_s3d"), Asker::Character);
        assert_eq!(Asker::of_name(b"srp"), Asker::Character);
        assert_eq!(Asker::of_name(b""), Asker::Character);
    }
}

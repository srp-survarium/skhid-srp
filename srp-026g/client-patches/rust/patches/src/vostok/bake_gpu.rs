//! The camouflage/decal bake's GPU work, for patch 121 (bake_async;
//! docs/notes/baked-textures.md, client_patch.md 3.57 "The bake's GPU round
//! trips").
//!
//! Stock, process_compression (0x12EC40) reads the bake's 2048x2048 RGBA
//! target back (a staging, map2D blocking), uploads those 16 MB again as the
//! compressor's level 0, and the compressor (0x11F3F0) maps each of its 10
//! block levels back, blocking again, before the texture is made from the
//! DDS bytes. Here a bake is issued ([`Issue`]): level 0 is a GPU copy of the
//! target, the levels are drawn and copied to stagings with no map
//! ([`compress`], [`readback`] at compress_mip's map), the last copy flushed,
//! and the bake's frame is copied ([`Job`]). A later frame asks the last
//! staging without waiting ([`Readback::progress`]); once the GPU has done
//! it, the DDS is allocated (MT memory, [`DdsBuffer`]) and the levels are
//! copied into it ([`Readback::into_dds`]), and process_compression's own
//! tail runs with the job ([`finish`]: [`resume`] at its prologue jumps to
//! 0x12F00B). The tail's save takes that DDS with no copy ([`save`], [`free`]).
//!
//! A bake the GPU has no place for is put off before its draw ([`Draw`],
//! [`Postponed`]): its query is left unfinished and made again later
//! ([`Postponed::requery`]: finish_query's result 4).
//!
//! A bake's GPU work runs on the thread that issued it (the immediate
//! context and the render allocator are that thread's). The state the sites
//! share ([`CAPTURE`], [`RESUMING`]) is behind a [`Lock`] and names its
//! thread: the sites run on every thread that bakes or compresses.

use core::ops::Range;
use core::ptr::{copy_nonoverlapping, null, NonNull};

use super::baker::{Model, Texture, TextureKind};
use super::clock;
use super::context::{Context, Reg};
use super::hook::Thread;
use super::Lock;
use crate::raw;

/// The compressor's block formats (DXGI): BC1 for diffuse, fresnel and
/// smoothness, BC3 for normal maps (process_compression 0x12EF9C..0x12EFAA).
const BC1: u32 = 0x46;
const BC3: u32 = 0x4C;
/// The bake targets' format and level 0's (R8G8B8A8_UNORM; 0x12C7C7, 0x11F447).
const RGBA8: u32 = 0x1C;
/// create_texture2d's usage: D3D11_USAGE_DEFAULT, as the compressor's level 0.
const DEFAULT_USAGE: u32 = 0;
/// The DDS header, before the levels (write_dds_header; compress 0x11F4FE).
const HEADER: usize = 0x80;
/// The most block levels a texture has (4096 texels: 11).
const LEVELS: usize = 12;
/// The timer's scale process_compression's prologue writes ([0A70884h], 0x12EC66).
const TIMER_SCALE: f32 = 1.0;

/// What finish_query (0xF0D00, stdcall: the query, the result, assert on
/// fail, the error type) is given; the result is cook_base::result_enum (1
/// error, 2 postponed, 3 success, 4 requery, 5 out of memory).
#[derive(Clone, Copy)]
struct Finish {
    result: u32,
    assert_on_fail: u32,
    error: u32,
}

/// As on_textures_loaded fails a bake (0x12EAED: 1, assert, type 0Bh).
const FAILED: Finish = Finish { result: 1, assert_on_fail: 1, error: 0xB };
/// The query made again: finish_query stores the result (+104h) and goes to
/// finish_translated_query (0x109BD0), whose guard (+2D8h) reaches 0; then
/// 0x103140, for result 4, clears the error type and the translated flags
/// (4100h), sets the guard to 1 and pushes the query as a new one (0x103D10):
/// translate_query runs again from its start. The error type 0 is "unset",
/// as 0x103140 writes it. (Result 2, "postponed", is no "later": for a
/// translate query 0xF0F59 stores it and the query ends there, unanswered.)
const REQUERY: Finish = Finish { result: 4, assert_on_fail: 1, error: 0 };

fn finish_query(query: raw::QueryResultForCook, how: Finish) {
    // SAFETY: a query of the baker's, not finished (it waited for us); stdcall.
    unsafe { raw::engine_finish_query(Some(query), how.result, how.assert_on_fail, how.error) }
}

/// The block format of a texture type: BC3 for normal maps, else BC1.
fn format(kind: TextureKind) -> u32 {
    if kind == TextureKind::N { BC3 } else { BC1 }
}

/// A bake's size in texels: both a multiple of 4, 4 to 4096.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent {
    width: u32,
    height: u32,
}

impl Extent {
    fn parse(width: u32, height: u32) -> Option<Extent> {
        let fits = |n: u32| (4..=4096).contains(&n) && n.is_multiple_of(4);
        (fits(width) && fits(height)).then_some(Extent { width, height })
    }
}

/// A thread's id (GetCurrentThreadId), to keep a bake on the thread that
/// issued it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreadId(u32);

impl ThreadId {
    pub fn current(_: &Thread) -> ThreadId {
        ThreadId(super::sync::thread_id())
    }

    /// The id, for a log line.
    pub fn id(self) -> u32 {
        self.0
    }
}

/// A performance counter reading (QueryPerformanceCounter).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp(u64);

impl Stamp {
    pub fn now(thread: &Thread) -> Stamp {
        Stamp(clock::counter(thread))
    }

    /// The milliseconds from this to `later`.
    pub fn ms_to(self, later: Stamp, thread: &Thread) -> f32 {
        let ticks = later.0.wrapping_sub(self.0);
        (ticks as f64 * 1000.0 / clock::frequency(thread).max(1) as f64) as f32
    }

    /// The milliseconds since this.
    pub fn elapsed_ms(self, thread: &Thread) -> f32 {
        self.ms_to(Stamp::now(thread), thread)
    }
}

// ---- references ----------------------------------------------------------------

/// A texture with one reference of ours (vostok::baker's Texture), let go
/// when dropped.
pub struct HeldTexture(Texture);

impl HeldTexture {
    /// The texture with one reference more. `texture` is alive: a reference
    /// is held, or the game just made it.
    fn hold(texture: raw::ResTexture) -> HeldTexture {
        let texture = Texture::of(texture);
        texture.hold();
        HeldTexture(texture)
    }

    fn raw(&self) -> raw::ResTexture {
        self.0.raw()
    }

    /// The handle, our reference given to whoever releases it.
    fn into_raw(self) -> raw::ResTexture {
        let texture = self.raw();
        core::mem::forget(self);
        texture
    }
}

impl Drop for HeldTexture {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// A model instance with one reference of ours (vostok::baker's Model), let
/// go when dropped: its surface stays alive while a bake for it is pending,
/// whoever leaves.
pub struct HeldModel(Model);

impl HeldModel {
    fn hold(model: raw::RenderModelInstance) -> HeldModel {
        let model = Model::of(model);
        model.hold();
        HeldModel(model)
    }
}

impl Drop for HeldModel {
    fn drop(&mut self) {
        self.0.release();
    }
}

// ---- the DDS ---------------------------------------------------------------------

/// A place in a bake's DDS: its bytes from the DDS's start. The arithmetic
/// on places is here alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DdsOffset(u32);

impl DdsOffset {
    /// The first level's place, past the header.
    const LEVELS: DdsOffset = DdsOffset(HEADER as u32);

    /// The compressor's cursor at this place. [`compress`] runs the
    /// compressor with a cursor counted from 0 (no DDS exists yet), so a
    /// cursor's value is its place. It is never written through: while
    /// [`compress`] runs, [`readback`] takes every level, so compress_mip
    /// neither maps nor copies one.
    fn cursor(self) -> *mut u8 {
        self.0 as usize as *mut u8
    }

    /// The place a cursor [`cursor`](Self::cursor) started is at.
    fn of_cursor(cursor: *const u8) -> DdsOffset {
        DdsOffset(cursor as usize as u32)
    }

    /// The place `bytes` past this one; none unless it is before `size`
    /// (the DDS's last byte follows the levels).
    fn past(self, bytes: u32, size: u32) -> Option<DdsOffset> {
        self.0.checked_add(bytes).filter(|&end| end < size).map(DdsOffset)
    }

    /// Its index into the DDS's bytes.
    fn index(self) -> usize {
        self.0 as usize
    }

    /// The `len` bytes from here, as an index range.
    fn span(self, len: usize) -> Range<usize> {
        self.index()..self.index() + len
    }
}

/// A DDS's bytes in MT memory (mt_allocate with the MT allocator, as
/// make_save_descriptor allocates its copy), ours until a save descriptor
/// owns them ([`save`]: its destructor frees them), else freed when dropped.
pub struct DdsBuffer {
    at: NonNull<u8>,
    size: u32,
}

// SAFETY: MT memory is any thread's to free; it is written on the bake's thread alone.
unsafe impl Send for DdsBuffer {}

impl DdsBuffer {
    /// `size` bytes (a header and some levels), none when MT has none.
    fn allocate(size: u32) -> Option<DdsBuffer> {
        if size as usize <= HEADER {
            return None;
        }
        // SAFETY: the game's MT allocation (ESI the size, the allocator
        // object make_save_descriptor gives it); any thread.
        let at = unsafe { raw::engine_mt_allocate(size, Some(raw::image::mt_allocator())) };
        NonNull::new(at).map(|at| DdsBuffer { at, size })
    }

    fn bytes(&mut self) -> &mut [u8] {
        // SAFETY: the `size` bytes allocated, ours alone.
        unsafe { core::slice::from_raw_parts_mut(self.at.as_ptr(), self.size as usize) }
    }
}

impl Drop for DdsBuffer {
    fn drop(&mut self) {
        // SAFETY: the MT memory allocated, freed once (mt_free, as the
        // descriptor's destructor frees its data).
        unsafe { raw::engine_mt_free(self.at.as_ptr()) }
    }
}

// ---- a bake's frame --------------------------------------------------------------

/// The longest name a bake has (the parameters' buffer_string, 104h with its NUL).
const NAME: usize = 0x104;

/// What process_compression's tail needs of a bake, copied out of its
/// frame and on_textures_loaded's (whose locals are gone when it ends):
/// the query, its surface and the model instance it is of (held), the
/// cook, the type, the size, the name, and what the tail forwards.
pub struct Job {
    cook: raw::CamouflageDecalTextureTypeBakerCook,
    query: raw::QueryResultForCook,
    surface: raw::RenderSurfaceInstance,
    model: HeldModel,
    kind: TextureKind,
    extent: Extent,
    size: u32,
    name: [u8; NAME],
    pair: [u32; 2],
    value: u32,
    draw_ms: f32,
    /// The ms process_compression ran on its thread until the bake was issued.
    issue_ms: f32,
    thread: ThreadId,
}

// SAFETY: a Job, its handles and its references are used only on the
// thread that issued it (`thread`; bake_async finishes it on that thread
// alone); the Lock it waits in moves it, never touches it, elsewhere.
unsafe impl Send for Job {}

impl Job {
    pub fn thread(&self) -> ThreadId {
        self.thread
    }

    /// The DDS's bytes (its header, the levels and one byte).
    pub fn size(&self) -> u32 {
        self.size
    }

    /// The bake's name (its file's, without ".dds"), for a log line.
    pub fn name(&self) -> &str {
        name_text(&self.name)
    }

    /// The name, kept for a line after the job is gone.
    pub fn name_copy(&self) -> JobName {
        JobName(self.name)
    }

    /// The bake put off: its query left unfinished, its model kept, the
    /// rest let go.
    pub fn postpone(self) -> Postponed {
        Postponed { query: self.query, _model: self.model, thread: self.thread }
    }

    /// A bake that cannot end well (its levels lost, MT out of room, a
    /// compressor that does not fit its DDS): its query finished with an
    /// error, as on_textures_loaded fails one (patch 70 makes its waiting
    /// queries again), then the model let go.
    pub fn fail(self) {
        finish_query(self.query, FAILED);
    }
}

/// A bake's name, NUL-ended in its 104h bytes.
fn name_text(name: &[u8; NAME]) -> &str {
    let n = name.iter().position(|&b| b == 0).unwrap_or(NAME);
    core::str::from_utf8(&name[..n]).unwrap_or("?")
}

/// A copy of a bake's name (its file's, without ".dds").
pub struct JobName([u8; NAME]);

impl JobName {
    pub fn as_str(&self) -> &str {
        name_text(&self.0)
    }
}

/// A bake's query put off while the GPU has no place for it: before its
/// draw (or, when its place went between the draw and the issue, after
/// it). The query is left unfinished (patch 70's waiters for its path keep
/// waiting), its model instance held; no GPU or MT memory is.
pub struct Postponed {
    query: raw::QueryResultForCook,
    _model: HeldModel,
    thread: ThreadId,
}

// SAFETY: as Job's: made again on `thread` alone.
unsafe impl Send for Postponed {}

impl Postponed {
    /// The thread whose on_textures_loaded it came from: it is made again
    /// there (patch 70 translates the waiters again on the finishing thread).
    pub fn thread(&self) -> ThreadId {
        self.thread
    }

    /// The query made again (finish_query, result 4: [`REQUERY`]): the
    /// resource system asks the baker for it anew, and its draw comes again.
    /// Patch 70 takes it out of its owners and translates its waiters again,
    /// as for any owner's finish. Then the model let go.
    pub fn requery(self) {
        finish_query(self.query, REQUERY);
    }
}

/// `bake.draw`: on_textures_loaded about to draw a bake (every check
/// passed), its frame by EBP.
pub struct Draw<'c, 't> {
    ctx: &'c mut Context<'t>,
    frame: raw::OnTexturesLoadedFrame,
}

impl<'c, 't> Draw<'c, 't> {
    pub fn at(ctx: &'c mut Context<'t>) -> Option<Draw<'c, 't>> {
        // SAFETY: the context is this site's: EBP is the frame.
        let frame = unsafe { raw::site::bake_draw::frame(&*ctx)? };
        Some(Draw { ctx, frame })
    }

    pub fn thread(&self) -> &Thread {
        self.ctx.thread()
    }

    /// The bake put off: its query (the texture queries' parent) and its
    /// model held; none when the frame has not both (then it is drawn).
    pub fn postpone(&self) -> Option<Postponed> {
        // SAFETY: on_textures_loaded's frame at the site: its results (the
        // engine's, alive while the callback runs) and the bake's parameters.
        unsafe {
            let query = self.frame.results()?.parent_query()?;
            let model = self.frame.parameters()?.model()?;
            Some(Postponed { query, _model: HeldModel::hold(model), thread: ThreadId::current(self.thread()) })
        }
    }
}

/// `bake.issue`: process_compression with the bake's target of its type in
/// [esp+1Ch], before the staging's create.
pub struct Issue<'c, 't> {
    ctx: &'c mut Context<'t>,
    frame: raw::ProcessCompressionFrame,
}

/// The bake's target of its type: alive while the hook runs (the frame
/// holds a reference until the epilogue).
pub struct Source<'a> {
    texture: raw::ResTexture,
    extent: Extent,
    _issue: core::marker::PhantomData<&'a ()>,
}

impl<'c, 't> Issue<'c, 't> {
    pub fn at(ctx: &'c mut Context<'t>) -> Option<Issue<'c, 't>> {
        // SAFETY: the context is this site's: its ESP is the frame.
        let frame = unsafe { raw::site::bake_issue::frame(&*ctx)? };
        Some(Issue { ctx, frame })
    }

    pub fn thread(&self) -> &Thread {
        self.ctx.thread()
    }

    /// The bake's job and target, parsed from the frames; none when one
    /// is not what a bake has (then the stock path runs).
    pub fn job(&self) -> Option<(Job, Source<'_>)> {
        let f = self.frame;
        // SAFETY: process_compression's frame at the site (its locals and
        // arguments, alive while it runs, on its thread); its caller's frame
        // (the saved EBP, its only caller on_textures_loaded) and the bake's
        // parameters it holds; the copy's name a NUL-ended string inside it.
        unsafe {
            let extent = Extent::parse(*f.width(), *f.height())?;
            let kind = TextureKind::from_raw(*f.kind())?;
            let model = f.caller()?.parameters()?.model()?;
            let pair = f.pair()?;
            let mut name = [0u8; NAME];
            let text = f.parameters()?.name();
            if text.is_null() {
                return None;
            }
            for (i, slot) in name.iter_mut().enumerate() {
                let byte = text.add(i).read();
                if byte == 0 {
                    break;
                }
                if i + 1 == NAME {
                    return None;
                }
                *slot = byte;
            }
            let job = Job {
                cook: f.cook()?,
                query: f.query()?,
                surface: f.surface()?,
                model: HeldModel::hold(model),
                kind,
                extent,
                size: *f.size(),
                name,
                pair: [*pair.first(), *pair.second()],
                value: *f.value(),
                draw_ms: *f.draw_ms(),
                issue_ms: 0.0,
                thread: ThreadId::current(self.ctx.thread()),
            };
            Some((job, Source { texture: f.source()?, extent, _issue: core::marker::PhantomData }))
        }
    }

    /// The ms since process_compression's start (its timer's start), into
    /// the job: what the bake cost this thread before the GPU took it.
    pub fn issued(&self, job: &mut Job) {
        // SAFETY: the frame's timer (QueryPerformanceCounter at its start, 0x12EC8E).
        let start = unsafe { u64::from(*self.frame.timer_start_low()) | u64::from(*self.frame.timer_start_high()) << 32 };
        let thread = self.ctx.thread();
        job.issue_ms = Stamp(start).ms_to(Stamp::now(thread), thread);
    }

    /// Before the `issued` exit: the frame's texture holder emptied (it holds
    /// the type getter's stale texture since 0x12ED0F; the epilogue releases
    /// what it holds and the target's reference).
    pub fn leave(self) {
        // SAFETY: the frame's slot, read by the epilogue (0x12F561) alone.
        unsafe { self.frame.set_texture(None) }
    }
}

// ---- the compressor and its readback ------------------------------------------------

/// Why [`compress`] gave no readback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressError {
    /// No render device or immediate context.
    NoDevice,
    /// block_mip_count's levels: none, or more than a texture has.
    Levels(u32),
    /// dxt_data_size's bytes (with the header and the last byte) past the
    /// job's DDS size.
    Size { data: u32, size: u32 },
    /// create_texture2d gave no texture for level 0 (the GPU's memory).
    Level0,
    /// A level compress_mip copied that [`readback`] could not keep.
    Capture(CaptureError),
    /// The compressor's cursor at its end not before the DDS's size.
    End { end: u32, size: u32 },
}

/// Why [`readback`] did not keep a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureError {
    /// The site's staging, frame or cursor was 0 (or the capture was gone).
    Site,
    /// More levels than a texture has.
    TooMany,
    /// The level's blocks not inside the DDS (past its size, or before its
    /// header's end).
    Fit { offset: u32, size: u32 },
}

/// Where a level's blocks go in the DDS.
#[derive(Clone, Copy, Debug)]
struct Place {
    offset: DdsOffset,
    rows: u32,
    row_bytes: u32,
}

/// A level's staging (a reference ours) and its place.
struct Level {
    staging: HeldTexture,
    place: Place,
}

/// The levels kept, in compress_mip's order: the first [`len`](Self::len)
/// slots, the rest empty.
struct Levels([Option<Level>; LEVELS]);

impl Levels {
    const fn new() -> Levels {
        Levels([const { None }; LEVELS])
    }

    fn len(&self) -> usize {
        self.iter().count()
    }

    fn push(&mut self, level: Level) -> Result<(), (Level, CaptureError)> {
        let n = self.len();
        match self.0.get_mut(n) {
            Some(slot) => {
                *slot = Some(level);
                Ok(())
            }
            None => Err((level, CaptureError::TooMany)),
        }
    }

    fn iter(&self) -> impl Iterator<Item = &Level> {
        self.0.iter().map_while(Option::as_ref)
    }
}

/// A bake the GPU has: its DDS's header, each level's staging and place,
/// and its last byte, not read back yet. No DDS memory is held.
pub struct Readback {
    header: [u8; HEADER],
    levels: Levels,
    /// The place of the byte after the levels, and that byte (1 for diffuse).
    end: DdsOffset,
    last: u8,
    size: u32,
}

// SAFETY: as Job's: used on the bake's thread alone.
unsafe impl Send for Readback {}

/// What `compress_mip.readback` does: nothing (the game's map: another
/// compress, or another thread's), or keep the stagings of the compress
/// under way on `thread`.
enum Capture {
    Off,
    On { thread: ThreadId, size: u32, levels: Levels },
    /// A level could not be kept: none after it is either (none is mapped
    /// or copied: compress_mip's cursor is never written through), and
    /// compress gives `why`.
    Broken { thread: ThreadId, why: CaptureError },
}

// SAFETY: as Job's: its stagings are used on `thread` alone.
unsafe impl Send for Capture {}

static CAPTURE: Lock<Capture> = Lock::new(Capture::Off);

/// Whether this thread's compress is capturing: no (the game's map), with
/// the DDS's size, or broken (the level kept unmapped, nothing noted).
#[derive(Clone, Copy)]
enum Capturing {
    No,
    Yes { size: u32 },
    Broken,
}

impl Capture {
    fn of(&self, me: ThreadId) -> Capturing {
        match *self {
            Capture::On { thread, size, .. } if thread == me => Capturing::Yes { size },
            Capture::Broken { thread, .. } if thread == me => Capturing::Broken,
            _ => Capturing::No,
        }
    }

    /// A level kept: none back. Past [`LEVELS`] the capture breaks, and the
    /// level and what the capture held come back, to be let go out of the lock.
    fn keep(&mut self, me: ThreadId, level: Level) -> Option<(Level, Capture)> {
        let pushed = match self {
            Capture::On { levels, .. } => levels.push(level),
            Capture::Off | Capture::Broken { .. } => return Some((level, Capture::Off)),
        };
        pushed.err().map(|(level, why)| (level, core::mem::replace(self, Capture::Broken { thread: me, why })))
    }
}

/// The device's immediate context (the render device [2260478h] +180h).
fn immediate_context() -> Option<raw::ImmediateContext> {
    // SAFETY: the render device's global and its field, read on the bake's thread.
    unsafe { raw::image::device_global().device().and_then(|device| device.context()) }
}

/// The compressor run as 0x11F3F0 runs it (the DDS header, level 0 a GPU
/// copy of the target, the levels drawn from it, each compressed and copied
/// to a staging), but no level is mapped: [`readback`] keeps each staging,
/// and the last copy is flushed so the GPU has it. No DDS is allocated: the
/// header is the readback's, the cursor counts places ([`DdsOffset`]).
pub fn compress(thread: &Thread, job: &Job, source: &Source<'_>) -> Result<Readback, CompressError> {
    let Extent { width, height } = job.extent;
    let (kind, format) = (job.kind, format(job.kind));
    let normal = u32::from(kind == TextureKind::N);
    let context = immediate_context().ok_or(CompressError::NoDevice)?;
    // SAFETY: the compressor's pure size functions (0x11F280, 0x11F2F0).
    let count = unsafe { raw::engine_block_mip_count(width / 4, height / 4) };
    if count == 0 || count as usize > LEVELS {
        return Err(CompressError::Levels(count));
    }
    // SAFETY: as above.
    let data = unsafe { raw::engine_dxt_data_size(count, width, height, format) };
    let needed = (HEADER as u32).checked_add(data).and_then(|n| n.checked_add(1));
    if needed.is_none_or(|needed| needed > job.size) {
        return Err(CompressError::Size { data, size: job.size });
    }
    let level0 = level0(source).ok_or(CompressError::Level0)?;
    let mut header = [0u8; HEADER];
    let mut levels: [Option<raw::ResTexture>; LEVELS] = [None; LEVELS];
    let mut cursor = raw::DdsCursorStorage::zeroed();
    let renderer = raw::image::system_renderer_global();
    // SAFETY: the game's functions in 0x11F3F0's order with its arguments,
    // on the bake's thread: the header into ours (80h bytes), level 0 one
    // reference of the array's (released below as 0x11F544 does), the
    // levels drawn, the cursor ours (a place: see DdsOffset::cursor).
    unsafe {
        raw::engine_write_dds_header(header.as_mut_ptr(), format, width, height, data + 1, normal, count);
        levels[0] = Some(HeldTexture::hold(level0.raw()).into_raw());
        raw::engine_backend_reset(raw::image::backend_global().backend());
        raw::engine_make_mips(width, height, renderer.renderer(), levels.as_mut_ptr(), count);
        cursor.handle().set_next(DdsOffset::LEVELS.cursor());
    }
    let me = ThreadId::current(thread);
    CAPTURE.with(thread, |c| *c = Capture::On { thread: me, size: job.size, levels: Levels::new() });
    for level in 0..count {
        // SAFETY: each level compressed from the array, as 0x11F3F0 loops;
        // readback keeps its staging, so the cursor is only moved.
        unsafe {
            raw::engine_compress_mip(renderer.renderer(), levels.as_mut_ptr(), Some(cursor.handle()),
                                     (width / 4) >> level, (height / 4) >> level, level, format, normal);
        }
    }
    let captured = CAPTURE.with(thread, |c| core::mem::replace(c, Capture::Off));
    // SAFETY: the context of the device the copies went to: the last copy
    // (0x11F738's) flushed, as compress_mip flushes before each (0x11F71C).
    unsafe { raw::engine_context_flush(Some(context)) }
    release_levels(&levels[..count as usize]);
    drop(level0);
    let kept = match captured {
        Capture::On { levels, .. } => levels,
        Capture::Broken { why, .. } => return Err(CompressError::Capture(why)),
        Capture::Off => return Err(CompressError::Capture(CaptureError::Site)),
    };
    // SAFETY: the cursor is ours, moved by readback alone.
    let end = DdsOffset::of_cursor(unsafe { cursor.handle().next() });
    if end.past(0, job.size).is_none() {
        return Err(CompressError::End { end: end.0, size: job.size });
    }
    Ok(Readback { header, levels: kept, end, last: u8::from(kind == TextureKind::D), size: job.size })
}

/// Level 0 of the compressor: a texture of the target's size and format
/// (D3D11_USAGE_DEFAULT, one level, as the compressor's own, 0x11F46D)
/// with the target's texels copied on the GPU (copy2D, as the stock copy to
/// the staging). Nothing is read back or uploaded.
fn level0(source: &Source<'_>) -> Option<HeldTexture> {
    let Extent { width, height } = source.extent;
    // SAFETY: the game's create and copy on the bake's thread; the source is
    // alive for the hook (Source); the texture made is held before use.
    unsafe {
        let manager = raw::image::render_resources().manager();
        let texture = raw::engine_create_texture2d(width, manager, null(), height, null(), RGBA8, DEFAULT_USAGE, 1, 0)?;
        let held = HeldTexture::hold(texture);
        raw::engine_copy2d(width, height, Some(held.raw()), Some(source.texture));
        Some(held)
    }
}

/// The compressor's array: each level's reference let go.
fn release_levels(levels: &[Option<raw::ResTexture>]) {
    for texture in levels.iter().flatten() {
        drop(HeldTexture(Texture::of(*texture)));
    }
}

/// A level the `compress_mip.readback` site holds: its staging, where its
/// blocks go, and the cursor that moves past them.
struct Copied {
    staging: raw::ResTexture,
    place: Place,
    bytes: u32,
    cursor: raw::DdsCursor,
}

/// What the site holds, its blocks inside a DDS of `size` bytes.
fn copied(ctx: &Context<'_>, size: u32) -> Result<Copied, CaptureError> {
    // SAFETY: compress_mip's site: ESI its staging, EBX the level's blocks
    // wide, its frame by ESP (the cursor, the blocks high, the format).
    let (staging, wide, cursor, high, format) = unsafe {
        let staging = raw::site::compress_mip_readback::staging(ctx).ok_or(CaptureError::Site)?;
        let frame = raw::site::compress_mip_readback::frame(ctx).ok_or(CaptureError::Site)?;
        let cursor = frame.cursor().ok_or(CaptureError::Site)?;
        (staging, raw::site::compress_mip_readback::wide(ctx), cursor, *frame.high(), *frame.format())
    };
    let row_bytes = if format == BC3 { 16 } else { 8 } * wide;
    // SAFETY: the cursor is compress's (ours).
    let offset = DdsOffset::of_cursor(unsafe { cursor.next() });
    let fit = CaptureError::Fit { offset: offset.0, size };
    let bytes = row_bytes.checked_mul(high).ok_or(fit)?;
    if offset < DdsOffset::LEVELS || offset.past(bytes, size).is_none() {
        return Err(fit);
    }
    Ok(Copied { staging, place: Place { offset, rows: high, row_bytes }, bytes, cursor })
}

/// `compress_mip.readback`, a level copied to its staging. During
/// [`compress`] on this thread every level is kept (true: leave by `kept`,
/// no map, no copy): its staging with a reference more and its place noted,
/// the cursor moved past its blocks as 0x11F863 moves it. One that does not
/// fit breaks the capture (and the bake), unmapped too. Any other compress
/// (another thread's, or not ours): false, the game's map.
pub fn readback(ctx: &mut Context<'_>) -> bool {
    let thread = ctx.thread();
    let me = ThreadId::current(thread);
    let size = match CAPTURE.with(thread, |c| c.of(me)) {
        Capturing::No => return false,
        Capturing::Broken => return true,
        Capturing::Yes { size } => size,
    };
    match copied(ctx, size) {
        Ok(Copied { staging, place, bytes, cursor }) => {
            // the staging is alive at the site (compress_mip holds it until
            // its release at 0x11F871); the reference taken keeps it after
            let level = Level { staging: HeldTexture::hold(staging), place };
            match CAPTURE.with(thread, |c| c.keep(me, level)) {
                // SAFETY: the cursor past the level's bytes, as 0x11F863
                // moves it (a place: never written through).
                None => unsafe { cursor.set_next(cursor.next().wrapping_add(bytes as usize)) },
                // let go out of the lock
                Some(refused) => drop(refused),
            }
        }
        Err(why) => drop(CAPTURE.with(thread, |c| core::mem::replace(c, Capture::Broken { thread: me, why }))),
    }
    // ESI keeps the staging: the exit releases compress_mip's reference
    true
}

/// How a level is mapped: D3D11_MAP_READ, waiting for the GPU or with
/// D3D11_MAP_FLAG_DO_NOT_WAIT (map2D's byte).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wait {
    Block,
    DoNotWait,
}

/// A level mapped (map2D): its data and row pitch; unmapped when dropped,
/// through the context and the resource it was mapped with (the immediate
/// context's Unmap, subresource 0, as 0x11F86F).
struct Mapped<'a> {
    context: raw::ImmediateContext,
    resource: raw::HwResource,
    data: NonNull<u8>,
    pitch: u32,
    _staging: &'a HeldTexture,
}

impl Mapped<'_> {
    fn of(context: raw::ImmediateContext, staging: &HeldTexture, wait: Wait) -> Option<Mapped<'_>> {
        // SAFETY: a staging we hold: its resource, read on the bake's thread.
        let resource = unsafe { staging.raw().hw_resource()? };
        let mut pitch = 0;
        // SAFETY: a staging (usage 3) we hold, on the bake's thread; map2D's
        // byte 1 for the no-wait flag (0 back while the GPU has not done it).
        let data = unsafe { raw::engine_map2d(Some(staging.raw()), &mut pitch, u32::from(wait == Wait::DoNotWait)) };
        NonNull::new(data.cast::<u8>()).map(|data| Mapped { context, resource, data, pitch, _staging: staging })
    }
}

impl Drop for Mapped<'_> {
    fn drop(&mut self) {
        // SAFETY: the resource mapped above, through the context map2D used
        // (the device's immediate context).
        unsafe { raw::engine_context_unmap(Some(self.context), Some(self.resource), 0) }
    }
}

/// Whether the GPU has done a bake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    Done,
    Busy,
}

/// Why a bake's levels did not come back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// mt_allocate had no room for its DDS.
    NoMemory,
    /// A level could not be mapped (the device lost), or its rows do not fit.
    Lost,
}

impl Readback {
    /// Whether the GPU has done the copies: the last staging maps without
    /// waiting (the GPU runs them in order). With no context or no level,
    /// done: the readback says what is lost.
    pub fn progress(&self) -> Progress {
        let busy = match (immediate_context(), self.levels.iter().last()) {
            (Some(context), Some(level)) => Mapped::of(context, &level.staging, Wait::DoNotWait).is_none(),
            _ => false,
        };
        if busy { Progress::Busy } else { Progress::Done }
    }

    /// The DDS, allocated now (MT memory), with its header, each level's
    /// blocks (the rows at the staging's pitch; waits for the GPU when it has
    /// not done them) and its last byte; each staging let go.
    pub fn into_dds(self) -> Result<DdsBuffer, ReadError> {
        let Readback { header, levels, end, last, size } = self;
        let mut dds = DdsBuffer::allocate(size).ok_or(ReadError::NoMemory)?;
        let context = immediate_context().ok_or(ReadError::Lost)?;
        let bytes = dds.bytes();
        bytes[..HEADER].copy_from_slice(&header);
        for level in levels.iter() {
            let Place { offset, rows, row_bytes } = level.place;
            let mapped = Mapped::of(context, &level.staging, Wait::Block).ok_or(ReadError::Lost)?;
            if mapped.pitch < row_bytes {
                return Err(ReadError::Lost);
            }
            for row in 0..rows {
                let at = offset.past(row * row_bytes, size).ok_or(ReadError::Lost)?;
                let into = bytes.get_mut(at.span(row_bytes as usize)).ok_or(ReadError::Lost)?;
                // SAFETY: the mapped staging holds `rows` rows of `pitch`
                // bytes, `row_bytes` of each its level's blocks.
                unsafe {
                    copy_nonoverlapping(mapped.data.as_ptr().add(row as usize * mapped.pitch as usize),
                                        into.as_mut_ptr(), into.len())
                }
            }
        }
        *bytes.get_mut(end.index()).ok_or(ReadError::Lost)? = last;
        Ok(dds)
    }
}

// ---- the tail, frames later ------------------------------------------------------------

/// What [`resume`] gives process_compression's frame.
#[derive(Clone, Copy)]
struct Resume {
    dds: NonNull<u8>,
    extent: Extent,
    kind: TextureKind,
    issue_ms: f32,
    started: Stamp,
}

/// The tail's save of a resumed bake's DDS.
#[derive(Clone, Copy)]
enum Save {
    /// Not reached yet.
    Pending,
    /// A descriptor of the game's owns the DDS (it frees it once written).
    Owned,
    /// allocate_save_descriptor had no memory: no descriptor, no save.
    Skipped,
}

/// Where the bake [`finish`] resumes is, on `thread`: no call, the call made
/// (its prologue not reached yet), or in its tail.
#[derive(Clone, Copy)]
enum Resuming {
    Idle,
    Calling { thread: ThreadId, resume: Resume },
    Tail { thread: ThreadId, resume: Resume, save: Save },
}

// SAFETY: a DDS's address, used on `thread` alone.
unsafe impl Send for Resuming {}

static RESUMING: Lock<Resuming> = Lock::new(Resuming::Idle);

/// What became of a finished bake's save.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Saved {
    /// The save's descriptor took the DDS.
    Yes,
    /// No descriptor (MT memory): the texture is used, not saved.
    NoDescriptor,
    /// The tail did not reach the save (the DDS freed here).
    NotReached,
}

/// A bake the GPU has done (its levels in `dds`): process_compression called
/// again with the job, its prologue jumping to its tail ([`resume`]), which
/// makes the texture from the DDS, puts it in the surface's slot, shares it
/// (patch 70), saves it ([`save`]: no copy), lists it, finishes the query and
/// logs "camouflage/decal cook" (bake: the ms its thread spent until issued,
/// with the draw's; compress: the ms of the readback). `started`: when the
/// readback began.
pub fn finish(thread: &Thread, job: Job, dds: DdsBuffer, started: Stamp) -> Saved {
    let mut parameters = raw::BakeParametersCopyStorage::zeroed();
    let mut pair = raw::BakePairStorage::zeroed();
    let extent = [job.extent.width, job.extent.height];
    let resume = Resume { dds: dds.at, extent: job.extent, kind: job.kind, issue_ms: job.issue_ms, started };
    let me = ThreadId::current(thread);
    RESUMING.with(thread, |r| *r = Resuming::Calling { thread: me, resume });
    // SAFETY: process_compression (its thunk: EAX the extent, the ten
    // arguments) with the job's copies, on the bake's thread: the cook, the
    // query (not finished: it waited), no targets (the tail does not read
    // them: resume leaves the prologue for 0x12F00B), the size, the surface
    // (its model held by the job), a parameters copy with the name at +13Ch,
    // the pair and value, the type, the draw's ms. Our storage outlives the call.
    unsafe {
        parameters.handle().set_name(job.name.as_ptr().cast_mut());
        *pair.handle().first() = job.pair[0];
        *pair.handle().second() = job.pair[1];
        raw::engine_process_compression(extent.as_ptr(), Some(job.cook), Some(job.query), null(), job.size,
                                        Some(job.surface), Some(parameters.handle()), Some(pair.handle()), job.value,
                                        job.kind.raw(), job.draw_ms);
    }
    let saved = match RESUMING.with(thread, |r| core::mem::replace(r, Resuming::Idle)) {
        Resuming::Tail { save: Save::Owned, .. } => Saved::Yes,
        Resuming::Tail { save: Save::Skipped, .. } => Saved::NoDescriptor,
        Resuming::Tail { save: Save::Pending, .. } | Resuming::Calling { .. } | Resuming::Idle => Saved::NotReached,
    };
    if saved == Saved::Yes {
        // the descriptor frees it once written
        core::mem::forget(dds);
    }
    drop(job);
    saved
}

/// `bake.resume`, process_compression's prologue done: for the call
/// [`finish`] makes on this thread, the locals its tail reads (the DDS, no
/// target, the size, no log object, the issue's ms, the timer started at the
/// readback) and the registers it has at 0x12F00B (EBX no RGBA, ESI the
/// timer, EDI the DDS, EBP the type); true to leave by `tail`. Any other
/// call: false.
pub fn resume(ctx: &mut Context<'_>) -> bool {
    let thread = ctx.thread();
    let me = ThreadId::current(thread);
    let calling = RESUMING.with(thread, |r| match *r {
        Resuming::Calling { thread, resume } if thread == me => Some(resume),
        _ => None,
    });
    let Some(r) = calling else { return false };
    // SAFETY: the context is this site's: its ESP is the frame.
    let Some(f) = (unsafe { raw::site::bake_resume::frame(&*ctx) }) else { return false };
    // SAFETY: process_compression's frame, its locals ours to set before
    // its tail reads them (0x12F00B on), on its thread.
    let timer = unsafe {
        f.set_dds(r.dds.as_ptr());
        f.set_source(None);
        f.set_texture(None);
        *f.width() = r.extent.width;
        *f.height() = r.extent.height;
        *f.log_object() = 0;
        *f.bake_ms() = r.issue_ms;
        *f.timer_base_low() = 0;
        *f.timer_base_high() = 0;
        *f.timer_start_low() = r.started.0 as u32;
        *f.timer_start_high() = (r.started.0 >> 32) as u32;
        // the prologue's two 1.0s (0x12EC73, 0x12EC79), the first the
        // timer's scale (elapsed_ticks multiplies by it)
        *f.timer_scale() = TIMER_SCALE;
        *f.prologue_scale() = TIMER_SCALE;
        f.timer_base_low() as usize as u32
    };
    RESUMING.with(thread, |s| *s = Resuming::Tail { thread: me, resume: r, save: Save::Pending });
    ctx.set_reg(Reg::Ebx, 0);
    ctx.set_reg(Reg::Esi, timer);
    ctx.set_reg(Reg::Edi, r.dds.as_ptr() as usize as u32);
    ctx.set_reg(Reg::Ebp, r.kind.raw());
    true
}

/// The resumed bake's DDS and its save, when this thread's tail runs.
fn tail(thread: &Thread) -> Option<(NonNull<u8>, Save)> {
    let me = ThreadId::current(thread);
    RESUMING.with(thread, |r| match *r {
        Resuming::Tail { thread, resume, save } if thread == me => Some((resume.dds, save)),
        _ => None,
    })
}

/// `bake.save`, the tail about to make the save's descriptor: for a resumed
/// bake's DDS, a descriptor of the game's (its two paths, allocate_save_
/// descriptor) that owns the DDS itself (its +8: the destructor frees it
/// with MT's free), so nothing is copied; the '/' the site writes, then EAX
/// the descriptor; true to leave by `made`. With no memory for a
/// descriptor, EAX 0 and `made` too: the query's +0E0h is 0, so
/// finish_translated_query saves nothing (the game's make_save_descriptor
/// would write through the 0 its allocation gives, 0x109EFD). Any other
/// bake: false, the game's make_save_descriptor copies its bytes.
pub fn save(ctx: &mut Context<'_>) -> bool {
    let thread = ctx.thread();
    let Some((dds, Save::Pending)) = tail(thread) else { return false };
    // SAFETY: the site's registers and frame: the two path spans and the
    // bytes about to be saved.
    let (physical, virtual_path, frame) = unsafe {
        (raw::site::bake_save::physical(&*ctx), raw::site::bake_save::virtual_path(&*ctx),
         raw::site::bake_save::frame(&*ctx))
    };
    let Some(frame) = frame else { return false };
    // EBX is the DDS; the bytes' data is written after the site (0x12F2E0), their size before it.
    let data = dds.as_ptr();
    if ctx.reg(Reg::Ebx) != data as usize as u32 {
        return false;
    }
    // SAFETY: the bytes the tail pushed (&[esp+28h]): their data set as
    // 0x12F2E0 sets it (the exit skips it), their size read; the '/' of the
    // instruction the exit skips.
    let size = unsafe {
        let Some(bytes) = frame.bytes() else { return false };
        bytes.set_data(data);
        *frame.separator() = b'/';
        *bytes.size()
    };
    // SAFETY: the game's allocation of a descriptor with its two paths
    // (copied), any thread; then its fields as make_save_descriptor sets
    // them (0x109EFD..0x109F1B), its data ours given to it.
    let (save, eax) = match unsafe { raw::engine_allocate_save_descriptor(physical, virtual_path) } {
        Some(descriptor) => {
            // SAFETY: as above.
            unsafe {
                descriptor.set_data(data);
                *descriptor.size() = size;
                *descriptor.owns() = 1;
            }
            (Save::Owned, descriptor.address())
        }
        None => (Save::Skipped, 0),
    };
    RESUMING.with(thread, |r| {
        if let Resuming::Tail { save: slot, .. } = r {
            *slot = save;
        }
    });
    ctx.set_reg(Reg::Eax, eax);
    true
}

/// `bake.free`, the tail about to free the DDS to the render allocator: a
/// resumed bake's DDS is MT memory (a descriptor's, or [`finish`] frees it),
/// so EBX 0 skips that free.
pub fn free(ctx: &mut Context<'_>) {
    if let Some((dds, _)) = tail(ctx.thread())
        && ctx.reg(Reg::Ebx) == dds.as_ptr() as usize as u32
    {
        ctx.set_reg(Reg::Ebx, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_are_offsets_from_the_dds_start() {
        let first = DdsOffset::LEVELS;
        assert_eq!(DdsOffset::of_cursor(first.cursor()), first);
        assert_eq!(first.past(0x100, 0x181), Some(DdsOffset(0x180)));
        assert_eq!(first.past(0x100, 0x180), None, "the last byte follows the levels");
        assert_eq!(DdsOffset(u32::MAX).past(1, u32::MAX), None);
        assert_eq!(first.span(4), 0x80..0x84);
    }
}

//! The camouflage/decal bake's GPU work, for patch 111 (bake_async;
//! docs/notes/baked-textures.md, client_patch.md 3.57 "The bake's GPU round
//! trips").
//!
//! Stock, process_compression (0x12EC40) reads the bake's 2048x2048 RGBA
//! target back (a staging, map2D blocking), uploads those 16 MB again as the
//! compressor's level 0, and the compressor (0x11F3F0) maps each of its 10
//! block levels back, blocking again, before the texture is made from the
//! DDS bytes. Here a bake is issued ([`Issue`]): level 0 is a GPU copy of the
//! target ([`level0`]), the levels are drawn and copied to stagings with no
//! map ([`compress`], [`readback`] at compress_mip's map), and the bake's
//! frame is copied ([`Job`]). A later frame asks the last staging without
//! waiting ([`Readback::ready`]), copies the levels into the DDS
//! ([`Readback::copy_into`]) and runs process_compression's own tail with the
//! job ([`finish`]: [`resume`] at its prologue jumps to 0x12F00B). The tail's
//! save takes the DDS the bake owns, MT memory ([`DdsBuffer`]), with no copy
//! ([`save`], [`free`]).
//!
//! Everything here runs on the thread that issued the bake: the immediate
//! context and the render allocator are that thread's.

use core::ptr::{copy_nonoverlapping, null, NonNull};

use super::clock;
use super::context::{Context, Reg};
use super::hook::Thread;
use super::baker::{Model, Texture, TextureKind};
use super::GameCell;
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
const HEADER: u32 = 0x80;
/// The most block levels a texture has (4096 texels: 11).
const LEVELS: usize = 12;
/// finish_query's error, as on_textures_loaded fails a bake (0x12EAED: 1, assert, type 0Bh).
const ERROR: (u32, u32, u32) = (1, 1, 0xB);

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
}

/// A performance counter reading (QueryPerformanceCounter).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp(u64);

impl Stamp {
    pub fn now(thread: &Thread) -> Stamp {
        Stamp(clock::counter(thread))
    }

    /// The milliseconds from this to `later`.
    fn ms_to(self, later: Stamp, thread: &Thread) -> f32 {
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
        if size <= HEADER {
            return None;
        }
        // SAFETY: the game's MT allocation (ESI the size, the allocator
        // object make_save_descriptor gives it); any thread.
        let at = unsafe { raw::engine_mt_allocate(size, Some(raw::image::mt_allocator())) };
        NonNull::new(at).map(|at| DdsBuffer { at, size })
    }

    fn address(&self) -> u32 {
        self.at.as_ptr() as usize as u32
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
    _model: HeldModel,
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
// thread that issued it (`thread`; bake_async polls on that thread alone);
// the Lock it waits in moves it, never touches it, elsewhere.
unsafe impl Send for Job {}

impl Job {
    pub fn kind(&self) -> TextureKind {
        self.kind
    }

    pub fn thread(&self) -> ThreadId {
        self.thread
    }

    /// The DDS's bytes (its header, the levels and one byte).
    pub fn size(&self) -> u32 {
        self.size
    }

    /// The bake's name (its file's, without ".dds"), for a log line.
    pub fn name(&self) -> &str {
        let n = self.name.iter().position(|&b| b == 0).unwrap_or(NAME);
        core::str::from_utf8(&self.name[..n]).unwrap_or("?")
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
                _model: HeldModel::hold(model),
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

// ---- level 0, the compressor and its readback --------------------------------------

/// Level 0 of the compressor: a texture of the target's size and format
/// (D3D11_USAGE_DEFAULT, one level, as the compressor's own, 0x11F46D)
/// with the target's texels copied on the GPU (copy2D, as the stock copy to
/// the staging). Nothing is read back or uploaded.
pub fn level0(source: &Source<'_>) -> Option<HeldTexture> {
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

/// A level's staging and where its blocks go in the DDS.
struct Level {
    staging: HeldTexture,
    offset: u32,
    rows: u32,
    row_bytes: u32,
}

/// The compressor's stagings, copied to on the GPU and not read yet.
pub struct Readback {
    levels: [Option<Level>; LEVELS],
}

// SAFETY: as Job's: used on the bake's thread alone.
unsafe impl Send for Readback {}

/// The DDS and its readback, a bake the GPU has.
pub struct Compressed {
    pub dds: DdsBuffer,
    pub readback: Readback,
}

/// A level compress_mip copied to its staging, taken by [`readback`].
#[derive(Clone, Copy)]
struct Captured {
    staging: raw::ResTexture,
    offset: u32,
    rows: u32,
    row_bytes: u32,
}

/// What `compress_mip.readback` does: nothing (the game's map), or keep the
/// stagings of the compress under way.
#[derive(Clone, Copy)]
enum Capture {
    Off,
    On { dds: u32, size: u32, count: usize, levels: [Option<Captured>; LEVELS] },
}

static CAPTURE: GameCell<Capture> = GameCell::new(Capture::Off);

/// The compressor run as 0x11F3F0 runs it (the DDS header, the levels drawn
/// from `level0`, each compressed and copied to a staging), but no level is
/// mapped: [`readback`] keeps each staging. The DDS is allocated here (its
/// job's size); none when MT has no room or a level does not fit (then
/// nothing is pending: the caller fails or runs the stock path).
pub fn compress(thread: &Thread, job: &Job, level0: &HeldTexture) -> Option<Compressed> {
    let dds = DdsBuffer::allocate(job.size)?;
    let Extent { width, height } = job.extent;
    let (kind, format) = (job.kind, format(job.kind));
    let normal = u32::from(kind == TextureKind::N);
    let mut levels: [Option<raw::ResTexture>; LEVELS] = [None; LEVELS];
    let mut cursor = raw::DdsCursorStorage::zeroed();
    let renderer = raw::image::system_renderer_global();
    // SAFETY: the game's functions in 0x11F3F0's order with its arguments,
    // on the bake's thread: the block levels, the data's size, the header
    // into our DDS (past its 80h: the size checked), level 0 one reference
    // of the array's (released below as 0x11F544 does), the levels drawn,
    // each compressed from the array, the cursor ours.
    unsafe {
        let count = raw::engine_block_mip_count(width / 4, height / 4) as usize;
        if count == 0 || count > LEVELS {
            return None;
        }
        let data = raw::engine_dxt_data_size(count as u32, width, height, format);
        if HEADER.checked_add(data)?.checked_add(1)? > job.size {
            return None;
        }
        raw::engine_write_dds_header(dds.at.as_ptr(), format, width, height, data + 1, normal, count as u32);
        levels[0] = Some(HeldTexture::hold(level0.raw()).into_raw());
        raw::engine_make_mips(width, height, renderer.renderer(), levels.as_mut_ptr(), count as u32);
        cursor.handle().set_next(dds.at.as_ptr().add(HEADER as usize));
        CAPTURE.set(thread, Capture::On { dds: dds.address(), size: job.size, count: 0, levels: [None; LEVELS] });
        for level in 0..count {
            raw::engine_compress_mip(renderer.renderer(), levels.as_mut_ptr(), Some(cursor.handle()),
                                     (width / 4) >> level, (height / 4) >> level, level as u32, format, normal);
        }
        let captured = CAPTURE.get(thread);
        CAPTURE.set(thread, Capture::Off);
        let end = cursor.handle().next();
        if end.is_null() || end as usize - dds.at.as_ptr() as usize >= job.size as usize {
            // the stagings kept are let go below (wrapped, then dropped)
            wrap(captured);
            release_levels(&levels[..count]);
            return None;
        }
        end.write(u8::from(kind == TextureKind::D));
        release_levels(&levels[..count]);
        Some(Compressed { dds, readback: wrap(captured) })
    }
}


/// The compressor's array: each level's reference let go.
fn release_levels(levels: &[Option<raw::ResTexture>]) {
    for texture in levels.iter().flatten() {
        drop(HeldTexture(Texture::of(*texture)));
    }
}

/// The stagings kept, each with the reference [`readback`] took.
fn wrap(capture: Capture) -> Readback {
    let mut out = Readback { levels: [const { None }; LEVELS] };
    if let Capture::On { levels, .. } = capture {
        for (slot, level) in out.levels.iter_mut().zip(levels) {
            *slot = level.map(|l| Level { staging: HeldTexture(Texture::of(l.staging)), offset: l.offset, rows: l.rows,
                                          row_bytes: l.row_bytes });
        }
    }
    out
}

/// `compress_mip.readback`, a level copied to its staging: during
/// [`compress`], the staging kept (a reference more), its blocks' place in
/// the DDS noted and the cursor moved past them, as the memcpy would; true
/// to leave by `kept` (no map). Else (another compress, or a level that
/// does not fit) the game's map.
pub fn readback(ctx: &mut Context<'_>) -> bool {
    let thread = ctx.thread();
    let Capture::On { dds, size, count, mut levels } = CAPTURE.get(thread) else { return false };
    // SAFETY: compress_mip's site: ESI its staging, EBX the level's blocks
    // wide, its frame by ESP (the cursor, the blocks high, the format).
    let (staging, wide, frame) = unsafe {
        (raw::site::compress_mip_readback::staging(&*ctx), raw::site::compress_mip_readback::wide(&*ctx),
         raw::site::compress_mip_readback::frame(&*ctx))
    };
    let (Some(staging), Some(frame)) = (staging, frame) else { return false };
    // SAFETY: the frame's arguments; the cursor is compress's (ours).
    let (cursor, high, format) = unsafe {
        let Some(cursor) = frame.cursor() else { return false };
        (cursor, *frame.high(), *frame.format())
    };
    let block = if format == BC3 { 16 } else { 8 };
    // SAFETY: as above.
    let at = unsafe { cursor.next() } as usize as u32;
    let Some(offset) = at.checked_sub(dds) else { return false };
    let row_bytes = block * wide;
    let Some(bytes) = row_bytes.checked_mul(high) else { return false };
    if count == LEVELS || offset < HEADER || offset.checked_add(bytes).is_none_or(|end| end >= size) {
        return false;
    }
    // the staging is alive at the site (compress_mip holds it until its
    // release at 0x11F871); the reference taken here keeps it after
    let held = HeldTexture::hold(staging).into_raw();
    levels[count] = Some(Captured { staging: held, offset, rows: high, row_bytes });
    CAPTURE.set(thread, Capture::On { dds, size, count: count + 1, levels });
    // SAFETY: the cursor past the level's bytes, as 0x11F863 moves it.
    unsafe { cursor.set_next(cursor.next().add(bytes as usize)) }
    // ESI keeps the staging: the exit releases compress_mip's reference
    true
}

/// A level mapped (map2D): its data and row pitch; unmapped when dropped
/// (the immediate context's Unmap, subresource 0, as 0x11F86F).
struct Mapped<'a> {
    staging: &'a HeldTexture,
    data: NonNull<u8>,
    pitch: u32,
}

impl Mapped<'_> {
    fn of(staging: &HeldTexture, wait: bool) -> Option<Mapped<'_>> {
        let mut pitch = 0;
        // SAFETY: a staging (usage 3) we hold, on the bake's thread;
        // map2D's D3D11_MAP_READ, D3D11_MAP_FLAG_DO_NOT_WAIT for `!wait` (0 back
        // while the GPU has not done it).
        let data = unsafe { raw::engine_map2d(Some(staging.raw()), &mut pitch, u32::from(!wait)) };
        NonNull::new(data.cast::<u8>()).map(|data| Mapped { staging, data, pitch })
    }
}

impl Drop for Mapped<'_> {
    fn drop(&mut self) {
        // SAFETY: the texture mapped above; the device's immediate context.
        unsafe {
            let context = raw::image::device_global().device().and_then(|device| device.context());
            raw::engine_context_unmap(context, self.staging.raw().hw_resource(), 0);
        }
    }
}

/// A level that could not be mapped (the device lost, or no staging).
#[derive(Clone, Copy, Debug)]
pub struct Lost;

impl Readback {
    fn last(&self) -> Option<&Level> {
        self.levels.iter().flatten().last()
    }

    /// Whether the GPU has done the copies: the last staging maps without
    /// waiting (the GPU runs them in order).
    pub fn ready(&self) -> bool {
        self.last().is_none_or(|level| Mapped::of(&level.staging, false).is_some())
    }

    /// Each level's blocks into the DDS (the rows at the staging's pitch),
    /// each staging let go; waits for the GPU when it has not done them.
    pub fn copy_into(self, dds: &mut DdsBuffer) -> Result<(), Lost> {
        let bytes = dds.bytes();
        for level in self.levels.into_iter().flatten() {
            let mapped = Mapped::of(&level.staging, true).ok_or(Lost)?;
            if mapped.pitch < level.row_bytes {
                return Err(Lost);
            }
            for row in 0..level.rows as usize {
                let to = level.offset as usize + row * level.row_bytes as usize;
                let into = bytes.get_mut(to..to + level.row_bytes as usize).ok_or(Lost)?;
                // SAFETY: the mapped staging holds `rows` rows of `pitch`
                // bytes, `row_bytes` of each its level's blocks.
                unsafe { copy_nonoverlapping(mapped.data.as_ptr().add(row * mapped.pitch as usize), into.as_mut_ptr(),
                                             into.len()) }
            }
        }
        Ok(())
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

/// Where a resumed bake is: no call, the call made (its prologue not
/// reached yet), or in its tail (the save descriptor that took the DDS).
#[derive(Clone, Copy)]
enum Resuming {
    Idle,
    Calling(Resume),
    Tail(Resume, Option<raw::SaveDescriptor>),
}

static RESUMING: GameCell<Resuming> = GameCell::new(Resuming::Idle);

/// A bake the GPU has done (its levels in `dds`): process_compression called
/// again with the job, its prologue jumping to its tail ([`resume`]), which
/// makes the texture from the DDS, puts it in the surface's slot, shares it
/// (patch 70), saves it ([`save`]: no copy), lists it, finishes the query and
/// logs "camouflage/decal cook" (bake: the ms its thread spent until issued,
/// with the draw's; compress: the ms of the readback). `started`: when the
/// readback began.
pub fn finish(thread: &Thread, job: Job, dds: DdsBuffer, started: Stamp) {
    let mut parameters = raw::BakeParametersCopyStorage::zeroed();
    let mut pair = raw::BakePairStorage::zeroed();
    let extent = [job.extent.width, job.extent.height];
    RESUMING.set(thread, Resuming::Calling(Resume { dds: dds.at, extent: job.extent, kind: job.kind,
                                                    issue_ms: job.issue_ms, started }));
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
    let state = RESUMING.get(thread);
    RESUMING.set(thread, Resuming::Idle);
    if let Resuming::Tail(_, Some(_)) = state {
        // the descriptor frees it once written
        core::mem::forget(dds);
    }
    drop(job);
}

/// A bake that cannot end well (its levels lost, or MT out of room): its
/// query finished with an error, as on_textures_loaded fails one (patch 70
/// makes its waiting queries again), the model let go.
pub fn fail(job: Job) {
    // SAFETY: the job's query, not finished (it waited for us), stdcall.
    unsafe { raw::engine_finish_query(Some(job.query), ERROR.0, ERROR.1, ERROR.2) }
    drop(job);
}

/// `bake.resume`, process_compression's prologue done: for the call
/// [`finish`] makes, the locals its tail reads (the DDS, no target, the
/// size, no log object, the issue's ms, the timer started at the readback)
/// and the registers it has at 0x12F00B (EBX no RGBA, ESI the timer, EDI the
/// DDS, EBP the type); true to leave by `tail`. Any other call: false.
pub fn resume(ctx: &mut Context<'_>) -> bool {
    let thread = ctx.thread();
    let Resuming::Calling(r) = RESUMING.get(thread) else { return false };
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
        f.timer_base_low() as usize as u32
    };
    RESUMING.set(thread, Resuming::Tail(r, None));
    ctx.set_reg(Reg::Ebx, 0);
    ctx.set_reg(Reg::Esi, timer);
    ctx.set_reg(Reg::Edi, r.dds.as_ptr() as usize as u32);
    ctx.set_reg(Reg::Ebp, r.kind.raw());
    true
}

/// `bake.save`, the tail about to make the save's descriptor: for a resumed
/// bake's DDS, a descriptor of the game's (its two paths, allocate_save_
/// descriptor) that owns the DDS itself (its +8: the destructor frees it
/// with MT's free), so nothing is copied; the '/' the site writes, then EAX
/// the descriptor; true to leave by `made`. Else (another bake, or no
/// descriptor) false: the game's make_save_descriptor copies the bytes.
pub fn save(ctx: &mut Context<'_>) -> bool {
    let thread = ctx.thread();
    let Resuming::Tail(r, None) = RESUMING.get(thread) else { return false };
    // SAFETY: the site's registers and frame: the two path spans and the
    // bytes about to be saved.
    let (physical, virtual_path, frame) = unsafe {
        (raw::site::bake_save::physical(&*ctx), raw::site::bake_save::virtual_path(&*ctx),
         raw::site::bake_save::frame(&*ctx))
    };
    let Some(frame) = frame else { return false };
    // EBX is the DDS; the bytes' data is written after the site (0x12F2E0), their size before it.
    let data = r.dds.as_ptr();
    if ctx.reg(Reg::Ebx) != data as usize as u32 {
        return false;
    }
    // SAFETY: the bytes the tail pushed (&[esp+28h]): their data set as
    // 0x12F2E0 sets it (the exit skips it), their size read.
    let size = unsafe {
        let Some(bytes) = frame.bytes() else { return false };
        bytes.set_data(data);
        *bytes.size()
    };
    // SAFETY: the game's allocation of a descriptor with its two paths
    // (copied), any thread; then its fields as make_save_descriptor sets
    // them (0x109EFD..0x109F1B), its data ours given to it; the '/' of the
    // instruction the exit skips.
    unsafe {
        let Some(descriptor) = raw::engine_allocate_save_descriptor(physical, virtual_path) else { return false };
        descriptor.set_data(data);
        *descriptor.size() = size;
        *descriptor.owns() = 1;
        *frame.separator() = b'/';
        RESUMING.set(thread, Resuming::Tail(r, Some(descriptor)));
        ctx.set_reg(Reg::Eax, descriptor.address());
    }
    true
}

/// `bake.free`, the tail about to free the DDS to the render allocator: a
/// resumed bake's DDS is MT memory (a descriptor's, or [`finish`] frees it),
/// so EBX 0 skips that free.
pub fn free(ctx: &mut Context<'_>) {
    let thread = ctx.thread();
    if let Resuming::Tail(r, _) = RESUMING.get(thread)
        && ctx.reg(Reg::Ebx) == r.dds.as_ptr() as usize as u32
    {
        ctx.set_reg(Reg::Ebx, 0);
    }
}


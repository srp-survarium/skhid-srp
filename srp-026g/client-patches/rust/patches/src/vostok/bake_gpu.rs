//! The camouflage/decal bake's GPU work, for patch 121 (bake_async;
//! docs/notes/baked-textures.md, client_patch.md 3.57 "The bake's GPU round
//! trips").
//!
//! Stock, process_compression (0x12EC40) reads the bake's 2048x2048 RGBA
//! target back (a staging, map2D blocking), uploads those 16 MB again as the
//! compressor's level 0, and the compressor (0x11F3F0) maps each of its 10
//! block levels back, blocking again, before the texture is made from the
//! DDS bytes. Here a bake is issued ([`Issue`], [`Issuing::issue`]): level 0
//! is a GPU copy of the target, the levels are drawn and copied to stagings
//! with no map ([`readback`] at compress_mip's map), the last copy flushed,
//! and the bake's frame is copied into a [`Job`]. The [`Issued`] bake (the
//! job and its stagings, one value) is asked later without waiting
//! ([`Issued::progress`]) and completed once ([`Issued::complete`]): the DDS
//! allocated (MT memory, [`DdsBuffer`]), the levels copied into it (waiting
//! only when the GPU has not done them), and process_compression's own tail
//! run with the job ([`resume`] at its prologue jumps to 0x12F00B). The
//! tail's save takes that DDS with no copy ([`save`], [`free`]).
//!
//! The query a [`Job`] holds is finished exactly once: by the tail, or with
//! an error ([`Job::fail`], or when the job is dropped).
//!
//! A bake's GPU work is the thread's that issued it (the immediate context,
//! the render allocator and the engine's references): every value holding
//! it is neither `Send` nor `Sync`. The state the sites share between hooks
//! ([`CAPTURE`], [`RESUMING`]) is behind a [`Lock`], each value in it a
//! [`ThreadBound`]: no thread but its own reaches it.

use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Range};
use core::ptr::{null, NonNull};

use super::baker::{Model, Texture, TextureKind};
use super::clock;
use super::context::{Context, Reg};
use super::hook::Thread;
use super::{Lock, ThreadBound};
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

/// Neither `Send` nor `Sync`: a value of the engine's that its thread alone uses.
type Local = PhantomData<*const ()>;

// ---- the values ----------------------------------------------------------------

/// A span of time in milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Ms(f32);

impl Ms {
    pub const fn new(ms: f32) -> Ms {
        Ms(ms)
    }

    /// The whole milliseconds, for a log line.
    pub fn whole(self) -> u32 {
        self.0 as u32
    }

    /// The game's float (its frame's and process_compression's ms).
    fn raw(self) -> f32 {
        self.0
    }
}

impl Add for Ms {
    type Output = Ms;

    fn add(self, other: Ms) -> Ms {
        Ms(self.0 + other.0)
    }
}

impl AddAssign for Ms {
    fn add_assign(&mut self, other: Ms) {
        self.0 += other.0;
    }
}

/// A performance counter reading (QueryPerformanceCounter): a later one is
/// larger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp(u64);

impl Stamp {
    pub fn now(thread: &Thread) -> Stamp {
        Stamp(clock::counter(thread))
    }

    /// The time from this to `later`.
    pub fn ms_to(self, later: Stamp, thread: &Thread) -> Ms {
        let ticks = later.0.wrapping_sub(self.0);
        Ms((ticks as f64 * 1000.0 / clock::frequency(thread).max(1) as f64) as f32)
    }

    /// The time since this.
    pub fn elapsed(self, thread: &Thread) -> Ms {
        self.ms_to(Stamp::now(thread), thread)
    }

    fn low(self) -> u32 {
        self.0 as u32
    }

    fn high(self) -> u32 {
        (self.0 >> 32) as u32
    }
}

/// A bake's size in texels: both a multiple of 4, 4 to 4096.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Extent {
    width: u32,
    height: u32,
}

impl Extent {
    fn parse(width: u32, height: u32) -> Option<Extent> {
        let fits = |n: u32| (4..=4096).contains(&n) && n.is_multiple_of(4);
        (fits(width) && fits(height)).then_some(Extent { width, height })
    }
}

/// A bake's DDS size in bytes (process_compression's argument 4): its
/// header, its levels and one byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DdsSize(u32);

impl DdsSize {
    pub fn bytes(self) -> u32 {
        self.0
    }

    /// Whether `data` bytes of levels fit, with the header and the last byte.
    fn holds(self, data: u32) -> bool {
        (HEADER as u32).checked_add(data).and_then(|n| n.checked_add(1)).is_some_and(|needed| needed <= self.0)
    }
}

/// The block format of a texture type: BC3 for normal maps, else BC1.
fn format(kind: TextureKind) -> u32 {
    if kind == TextureKind::N { BC3 } else { BC1 }
}

// ---- references ----------------------------------------------------------------

/// A texture with one reference of ours (vostok::baker's Texture), let go
/// when dropped, on its thread.
struct HeldTexture {
    texture: Texture,
    _local: Local,
}

impl HeldTexture {
    /// The texture with one reference more. `texture` is alive: a reference
    /// is held, or the game just made it.
    fn hold(texture: raw::ResTexture) -> HeldTexture {
        let texture = Texture::of(texture);
        texture.hold();
        HeldTexture { texture, _local: PhantomData }
    }

    fn raw(&self) -> raw::ResTexture {
        self.texture.raw()
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
        self.texture.release();
    }
}

/// A model instance with one reference of ours (vostok::baker's Model), let
/// go when dropped: its surface stays alive while a bake for it is pending,
/// whoever leaves.
struct HeldModel {
    model: Model,
    _local: Local,
}

impl HeldModel {
    fn hold(model: raw::RenderModelInstance) -> HeldModel {
        let model = Model::of(model);
        model.hold();
        HeldModel { model, _local: PhantomData }
    }
}

impl Drop for HeldModel {
    fn drop(&mut self) {
        self.model.release();
    }
}

/// cook_base::result_enum (1 error, 2 postponed, 3 success, 4 requery, 5
/// out of memory): the one this patch finishes a query with itself.
#[derive(Clone, Copy)]
#[repr(u32)]
enum CookResult {
    Error = 1,
}

/// The query's error type (+100h): as on_textures_loaded fails a bake.
#[derive(Clone, Copy)]
#[repr(u32)]
enum ErrorType {
    Bake = 0xB,
}

/// A bake's query, not finished yet: finished once, by process_compression's
/// tail ([`into_tail`](Self::into_tail)) or with an error (when dropped, as
/// on_textures_loaded fails a bake: patch 70 makes its waiting queries again).
struct PendingQuery {
    query: raw::QueryResultForCook,
    _local: Local,
}

impl PendingQuery {
    /// For process_compression's tail, which finishes it.
    fn into_tail(self) -> raw::QueryResultForCook {
        let query = self.query;
        core::mem::forget(self);
        query
    }
}

impl Drop for PendingQuery {
    fn drop(&mut self) {
        // SAFETY: finish_query (0xF0D00, stdcall) on a query of the baker's
        // that nothing finished (this owns it until now), as 0x12EAED fails one.
        unsafe { raw::engine_finish_query(Some(self.query), CookResult::Error as u32, 1, ErrorType::Bake as u32) }
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

    /// The place `bytes` past this one; none unless it is before `size`'s
    /// end (the DDS's last byte follows the levels).
    fn past(self, bytes: u32, size: DdsSize) -> Option<DdsOffset> {
        self.0.checked_add(bytes).filter(|&end| end < size.0).map(DdsOffset)
    }

    /// Its index into the DDS's bytes.
    fn index(self) -> usize {
        self.0 as usize
    }

    /// The `len` bytes from here, as an index range.
    fn span(self, len: usize) -> Range<usize> {
        self.index()..self.index() + len
    }

    pub fn bytes(self) -> u32 {
        self.0
    }
}

/// A DDS's bytes in MT memory (mt_allocate with the MT allocator, as
/// make_save_descriptor allocates its copy), ours until a save descriptor
/// owns them ([`save`]: its destructor frees them), else freed when dropped.
struct DdsBuffer {
    at: NonNull<u8>,
    size: DdsSize,
}

impl DdsBuffer {
    /// `size` bytes, none when MT has none (or the size holds no header:
    /// [`compress`] refuses that bake first).
    fn allocate(size: DdsSize) -> Option<DdsBuffer> {
        if size.0 as usize <= HEADER {
            return None;
        }
        // SAFETY: the game's MT allocation (ESI the size, the allocator
        // object make_save_descriptor gives it); any thread.
        let at = unsafe { raw::engine_mt_allocate(size.0, Some(raw::image::mt_allocator())) };
        NonNull::new(at).map(|at| DdsBuffer { at, size })
    }

    fn bytes(&mut self) -> &mut [u8] {
        // SAFETY: the `size` bytes allocated, ours alone (the borrow is ours).
        unsafe { core::slice::from_raw_parts_mut(self.at.as_ptr(), self.size.0 as usize) }
    }
}

impl Drop for DdsBuffer {
    fn drop(&mut self) {
        // SAFETY: the MT memory allocated, freed once (mt_free, as the
        // descriptor's destructor frees its data).
        unsafe { raw::engine_mt_free(self.at.as_ptr()) }
    }
}

// ---- the render system -------------------------------------------------------------

/// The render system's objects a bake uses, read from their globals.
#[derive(Clone, Copy)]
struct Device {
    /// The device's immediate context (the render device [2260478h] +180h).
    context: raw::ImmediateContext,
    /// The system renderer ([2260480h]), the compressor's.
    renderer: raw::SystemRenderer,
    /// The render backend ([2260490h]).
    backend: raw::Backend,
    /// The render resources' manager (create_texture2d's).
    manager: raw::ResourceManager,
}

/// The device's immediate context, none before the render device is made.
fn immediate_context() -> Option<raw::ImmediateContext> {
    // SAFETY: the render device's global, a pointer the game sets once.
    let device = unsafe { raw::image::device_global().device() }?;
    // SAFETY: the device's +180h, its immediate context.
    unsafe { device.context() }
}

impl Device {
    fn read() -> Option<Device> {
        let context = immediate_context()?;
        // SAFETY: the system renderer's global, a pointer the game sets once.
        let renderer = unsafe { raw::image::system_renderer_global().renderer() }?;
        // SAFETY: the render backend's global, as renderer.
        let backend = unsafe { raw::image::backend_global().backend() }?;
        // SAFETY: the render resources' global, as renderer.
        let manager = unsafe { raw::image::render_resources().manager() }?;
        Some(Device { context, renderer, backend, manager })
    }
}

// ---- a bake's frame --------------------------------------------------------------

/// The longest name a bake has (the parameters' buffer_string, 104h with its NUL).
const NAME: usize = 0x104;

/// process_compression's frame at one of its sites (its ESP after the
/// prologue; fields_026g.tsv process_compression_frame): alive, and this
/// thread's alone, while the hook that read it runs (`'c`). Each read and
/// write below is one field of it.
#[derive(Clone, Copy)]
struct Frame<'c> {
    frame: raw::ProcessCompressionFrame,
    _hook: PhantomData<&'c Thread>,
}

/// A frame word's read.
macro_rules! frame_get {
    ($($get:ident: $ty:ty),* $(,)?) => {$(
        fn $get(self) -> $ty {
            // SAFETY: a word of the frame, alive while the hook runs (Frame).
            unsafe { *self.frame.$get() }
        }
    )*};
}

/// A frame word's write (`$word` the word's accessor).
macro_rules! frame_put {
    ($($put:ident = $word:ident: $ty:ty),* $(,)?) => {$(
        fn $put(self, value: $ty) {
            // SAFETY: a word of the frame, alive while the hook runs and
            // this thread's (Frame).
            unsafe { *self.frame.$word() = value }
        }
    )*};
}

impl<'c> Frame<'c> {
    frame_get! {
        width: u32,
        height: u32,
        kind: u32,
        size: u32,
        value: u32,
        draw_ms: f32,
        timer_start_low: u32,
        timer_start_high: u32,
    }

    frame_put! {
        put_width = width: u32,
        put_height = height: u32,
        put_log_object = log_object: u32,
        put_bake_ms = bake_ms: f32,
        put_timer_base_low = timer_base_low: u32,
        put_timer_base_high = timer_base_high: u32,
        put_timer_start_low = timer_start_low: u32,
        put_timer_start_high = timer_start_high: u32,
        put_timer_scale = timer_scale: f32,
        put_prologue_scale = prologue_scale: f32,
    }

    fn cook(self) -> Option<raw::CamouflageDecalTextureTypeBakerCook> {
        // SAFETY: argument 1, the cook (Frame).
        unsafe { self.frame.cook() }
    }

    fn query(self) -> Option<raw::QueryResultForCook> {
        // SAFETY: argument 2, the query (Frame).
        unsafe { self.frame.query() }
    }

    /// Argument 2 emptied: the query is a [`Job`]'s now. The `issued` exit
    /// does not read it (the epilogue releases +10h and +1Ch alone).
    fn clear_query(self) {
        // SAFETY: argument 2's slot (Frame).
        unsafe { self.frame.set_query(None) }
    }

    fn surface(self) -> Option<raw::RenderSurfaceInstance> {
        // SAFETY: argument 5, the surface (Frame).
        unsafe { self.frame.surface() }
    }

    fn source(self) -> Option<raw::ResTexture> {
        // SAFETY: [esp+1Ch], the bake's target of its type, a reference the
        // frame holds until its epilogue (Frame).
        unsafe { self.frame.source() }
    }

    /// The model instance the bake is of: on_textures_loaded's parameters'.
    fn model(self) -> Option<raw::RenderModelInstance> {
        // SAFETY: the saved EBP, on_textures_loaded's frame (process_
        // compression's only caller, 0x12EA0D), alive below this one.
        let caller = unsafe { self.frame.caller() }?;
        // SAFETY: its [ebp+18h], the bake's parameters, alive while it runs.
        let parameters = unsafe { caller.parameters() }?;
        // SAFETY: the parameters' +790h, the model instance.
        unsafe { parameters.model() }
    }

    /// Argument 7, the pair the tail forwards.
    fn pair(self) -> Option<[u32; 2]> {
        // SAFETY: argument 7, a pair of on_textures_loaded's (Frame).
        let pair = unsafe { self.frame.pair() }?;
        // SAFETY: the pair's first word.
        let first = unsafe { *pair.first() };
        // SAFETY: its second.
        let second = unsafe { *pair.second() };
        Some([first, second])
    }

    /// The bake's name (argument 6's +13Ch, a C string), NUL-ended in
    /// [`NAME`] bytes; none when it is longer.
    fn name(self) -> Option<[u8; NAME]> {
        // SAFETY: argument 6, on_textures_loaded's parameters copy (Frame).
        let copy = unsafe { self.frame.parameters() }?;
        // SAFETY: the copy's +13Ch, its name.
        let text = unsafe { copy.name() };
        if text.is_null() {
            return None;
        }
        let mut name = [0u8; NAME];
        for (i, slot) in name.iter_mut().enumerate() {
            // SAFETY: a byte of the NUL-ended name, none past its NUL (the
            // loop ends there).
            let byte = unsafe { text.add(i).read() };
            if byte == 0 {
                return Some(name);
            }
            if i + 1 == NAME {
                return None;
            }
            *slot = byte;
        }
        None
    }

    /// The frame's timer start (QueryPerformanceCounter at 0x12EC8E).
    fn timer_start(self) -> Stamp {
        Stamp(u64::from(self.timer_start_low()) | u64::from(self.timer_start_high()) << 32)
    }

    /// The timer's place (ESI at 0x12F00B).
    fn timer_address(self) -> u32 {
        // SAFETY: only the place is computed, nothing read (Frame).
        unsafe { self.frame.timer_base_low() as usize as u32 }
    }

    /// [esp+10h], the texture holder (the epilogue releases what it holds).
    fn clear_texture(self) {
        // SAFETY: the frame's holder (Frame).
        unsafe { self.frame.set_texture(None) }
    }

    /// [esp+1Ch], the target, emptied (the tail's epilogue releases it).
    fn clear_source(self) {
        // SAFETY: the frame's target slot (Frame).
        unsafe { self.frame.set_source(None) }
    }

    /// [esp+18h], the DDS the tail makes the texture from.
    fn set_dds(self, dds: NonNull<u8>) {
        // SAFETY: the frame's DDS slot (Frame).
        unsafe { self.frame.set_dds(dds.as_ptr()) }
    }
}

/// What process_compression's tail needs of a bake, copied out of its
/// frame and on_textures_loaded's (whose locals are gone when it ends):
/// the query (finished once: [`PendingQuery`]), its surface and the model
/// instance it is of (held), the cook, the type, the size, the name, and
/// what the tail forwards.
pub struct Job {
    cook: raw::CamouflageDecalTextureTypeBakerCook,
    /// Before the model: a failed query is finished, then the model let go.
    query: PendingQuery,
    surface: raw::RenderSurfaceInstance,
    model: HeldModel,
    kind: TextureKind,
    extent: Extent,
    size: DdsSize,
    name: [u8; NAME],
    pair: [u32; 2],
    value: u32,
    draw_ms: Ms,
}

impl Job {
    /// The DDS's bytes (its header, the levels and one byte).
    pub fn size(&self) -> DdsSize {
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

    /// A bake that cannot end well (its levels lost, MT out of room, a
    /// compressor that does not fit its DDS): its query finished with an
    /// error, as on_textures_loaded fails one (patch 70 makes its waiting
    /// queries again), then the model let go.
    pub fn fail(self) {
        drop(self);
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

/// `bake.issue`: process_compression with the bake's target of its type in
/// [esp+1Ch], before the staging's create.
pub struct Issue<'c> {
    thread: &'c Thread,
    frame: Frame<'c>,
}

impl<'c> Issue<'c> {
    pub fn at(ctx: &'c Context<'_>) -> Option<Issue<'c>> {
        // SAFETY: the context is this site's: its ESP is the frame.
        let frame = unsafe { raw::site::bake_issue::frame(ctx) }?;
        Some(Issue { thread: ctx.thread(), frame: Frame { frame, _hook: PhantomData } })
    }

    /// The bake taken from the frames: its job (the query moved out of the
    /// frame, so a second take finds none) and its target. None when one is
    /// not what a bake has: then nothing is taken, and the stock path runs.
    pub fn take(self) -> Option<Issuing<'c>> {
        let f = self.frame;
        let extent = Extent::parse(f.width(), f.height())?;
        let kind = TextureKind::from_raw(f.kind())?;
        let model = f.model()?;
        let pair = f.pair()?;
        let name = f.name()?;
        let (cook, query, surface, source) = (f.cook()?, f.query()?, f.surface()?, f.source()?);
        f.clear_query();
        let job = Job {
            cook,
            query: PendingQuery { query, _local: PhantomData },
            surface,
            model: HeldModel::hold(model),
            kind,
            extent,
            size: DdsSize(f.size()),
            name,
            pair,
            value: f.value(),
            draw_ms: Ms(f.draw_ms()),
        };
        Some(Issuing { site: self, job, source })
    }
}

/// A bake taken at its issue, not handed to the GPU yet. The hook leaves by
/// `issued` once it is [`issue`](Self::issue)d: the frame's query is the job's.
#[must_use]
pub struct Issuing<'c> {
    site: Issue<'c>,
    job: Job,
    /// The bake's target, alive while the hook runs (the frame holds it).
    source: raw::ResTexture,
}

/// A bake whose compress failed: its job, to fail.
pub struct IssueFailed {
    pub error: CompressError,
    pub job: Job,
}

impl Issuing<'_> {
    pub fn name(&self) -> &str {
        self.job.name()
    }

    /// The bake handed to the GPU ([`compress`]), the ms process_compression
    /// ran until now kept for its log line, and the frame's texture holder
    /// emptied (it holds the type getter's stale texture since 0x12ED0F;
    /// the epilogue releases what it holds and the target's reference).
    pub fn issue(self) -> Result<Issued, IssueFailed> {
        let Issuing { site, job, source } = self;
        let compressed = compress(site.thread, &job, source);
        let since = Stamp::now(site.thread);
        let issue_ms = site.frame.timer_start().ms_to(since, site.thread);
        site.frame.clear_texture();
        match compressed {
            Ok(readback) => Ok(Issued { job, readback, issue_ms, since }),
            Err(error) => Err(IssueFailed { error, job }),
        }
    }
}

// ---- the compressor and its readback ------------------------------------------------

/// Why [`compress`] gave no readback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompressError {
    /// No render device, immediate context, renderer or backend.
    NoDevice,
    /// block_mip_count's levels: none, or more than a texture has.
    Levels(u32),
    /// dxt_data_size's bytes (with the header and the last byte) past the
    /// job's DDS size.
    Size { data: u32, size: DdsSize },
    /// A capture of another compress is under way (it is never another's).
    Busy,
    /// create_texture2d gave no texture for level 0 (the GPU's memory).
    Level0,
    /// A level compress_mip copied that [`readback`] could not keep.
    Capture(CaptureError),
    /// The compressor's cursor at its end not before the DDS's size.
    End { end: DdsOffset, size: DdsSize },
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
    Fit { offset: DdsOffset, size: DdsSize },
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

    /// The level at the end; given back when there is no room.
    fn push(&mut self, level: Level) -> Result<(), Level> {
        let n = self.len();
        match self.0.get_mut(n) {
            Some(slot) => {
                *slot = Some(level);
                Ok(())
            }
            None => Err(level),
        }
    }

    fn iter(&self) -> impl Iterator<Item = &Level> {
        self.0.iter().map_while(Option::as_ref)
    }
}

/// A bake the GPU has: its DDS's header, each level's staging and place,
/// and its last byte, not read back yet. No DDS memory is held.
struct Readback {
    header: [u8; HEADER],
    levels: Levels,
    /// The place of the byte after the levels, and that byte (1 for diffuse).
    end: DdsOffset,
    last: u8,
}

/// What the compress under way on a thread keeps (`compress_mip.readback`).
enum Kept {
    Levels(Levels),
    /// A level could not be kept: none after it is either (none is mapped
    /// or copied: compress_mip's cursor is never written through), and
    /// compress gives `why`.
    Broken(CaptureError),
}

/// A compress under way: its DDS's size and what it kept.
struct Capture {
    size: DdsSize,
    kept: Kept,
}

impl Capture {
    /// A level kept; else it comes back, with the levels the capture held
    /// when it breaks here (past [`LEVELS`]), to be let go out of the lock.
    fn keep(&mut self, level: Level) -> Result<(), (Level, Option<Levels>)> {
        let pushed = match &mut self.kept {
            Kept::Levels(levels) => levels.push(level),
            Kept::Broken(_) => return Err((level, None)),
        };
        pushed.map_err(|level| (level, self.broken(CaptureError::TooMany)))
    }

    /// The capture broken: the levels it held, if any.
    fn broken(&mut self, why: CaptureError) -> Option<Levels> {
        match core::mem::replace(&mut self.kept, Kept::Broken(why)) {
            Kept::Levels(levels) => Some(levels),
            Kept::Broken(_) => None,
        }
    }
}

/// The compress under way, the thread's that runs it; none (the game's map)
/// for every other compress.
static CAPTURE: Lock<Option<ThreadBound<Capture>>> = Lock::new(None);

/// A capture this thread started ([`CAPTURE`]): ended when dropped, or by
/// [`end`](Self::end) with what it kept.
struct Capturing<'t> {
    thread: &'t Thread,
}

impl<'t> Capturing<'t> {
    /// Started for this thread, only when no capture is under way.
    fn start(thread: &'t Thread, size: DdsSize) -> Option<Capturing<'t>> {
        let started = CAPTURE.with(thread, |c| match c {
            Some(_) => false,
            None => {
                *c = Some(ThreadBound::new(thread, Capture { size, kept: Kept::Levels(Levels::new()) }));
                true
            }
        });
        started.then_some(Capturing { thread })
    }

    /// What it kept; the capture over.
    fn end(self) -> Option<Kept> {
        let thread = self.thread;
        core::mem::forget(self);
        let taken = CAPTURE.with(thread, Option::take)?;
        taken.into_inner(thread).ok().map(|capture| capture.kept)
    }
}

impl Drop for Capturing<'_> {
    fn drop(&mut self) {
        // let go out of the lock
        drop(CAPTURE.with(self.thread, Option::take));
    }
}

/// The compressor run as 0x11F3F0 runs it (the DDS header, level 0 a GPU
/// copy of the target, the levels drawn from it, each compressed and copied
/// to a staging), but no level is mapped: [`readback`] keeps each staging,
/// and the last copy is flushed so the GPU has it. No DDS is allocated: the
/// header is the readback's, the cursor counts places ([`DdsOffset`]).
fn compress(thread: &Thread, job: &Job, source: raw::ResTexture) -> Result<Readback, CompressError> {
    let Extent { width, height } = job.extent;
    let (kind, format) = (job.kind, format(job.kind));
    let normal = u32::from(kind == TextureKind::N);
    let device = Device::read().ok_or(CompressError::NoDevice)?;
    // SAFETY: block_mip_count (0x11F280), a pure function of two counts.
    let count = unsafe { raw::engine_block_mip_count(width / 4, height / 4) };
    if count == 0 || count as usize > LEVELS {
        return Err(CompressError::Levels(count));
    }
    // SAFETY: dxt_data_size (0x11F2F0), a pure function of its values.
    let data = unsafe { raw::engine_dxt_data_size(count, width, height, format) };
    if !job.size.holds(data) {
        return Err(CompressError::Size { data, size: job.size });
    }
    let capture = Capturing::start(thread, job.size).ok_or(CompressError::Busy)?;
    let level0 = level0(device, job.extent, source).ok_or(CompressError::Level0)?;
    let mut header = [0u8; HEADER];
    // SAFETY: write_dds_header (0x11F350) writes its 80h bytes into ours.
    unsafe { raw::engine_write_dds_header(header.as_mut_ptr(), format, width, height, data + 1, normal, count) };
    // the compressor's array: level 0 one reference of its own (released
    // below, as 0x11F544 does), make_mips puts the others
    let mut levels: [Option<raw::ResTexture>; LEVELS] = [None; LEVELS];
    levels[0] = Some(HeldTexture::hold(level0.raw()).into_raw());
    // SAFETY: backend::reset (0x11B740) on the render backend, as 0x11F4D7
    // before make_mips, on the bake's thread.
    unsafe { raw::engine_backend_reset(Some(device.backend)) };
    // SAFETY: make_mips (0x11F8B0): levels 1..count drawn from level 0 into
    // new targets, their references put in our array of LEVELS slots.
    unsafe { raw::engine_make_mips(width, height, Some(device.renderer), levels.as_mut_ptr(), count) };
    let mut cursor = raw::DdsCursorStorage::zeroed();
    // SAFETY: our cursor's one word: a place (DdsOffset::cursor).
    unsafe { cursor.handle().set_next(DdsOffset::LEVELS.cursor()) };
    for level in 0..count {
        // SAFETY: compress_mip (0x11F590) on one level of the array, as
        // 0x11F3F0 loops; readback keeps its staging, so the cursor is only
        // moved, never written through.
        unsafe {
            raw::engine_compress_mip(Some(device.renderer), levels.as_mut_ptr(), Some(cursor.handle()),
                                     (width / 4) >> level, (height / 4) >> level, level, format, normal)
        }
    }
    let kept = capture.end();
    // SAFETY: Flush on the immediate context the copies went to: the last
    // copy (0x11F738's) submitted, as compress_mip flushes before each (0x11F71C).
    unsafe { raw::engine_context_flush(Some(device.context)) }
    release_levels(&levels[..count as usize]);
    drop(level0);
    let kept = match kept {
        Some(Kept::Levels(levels)) => levels,
        Some(Kept::Broken(why)) => return Err(CompressError::Capture(why)),
        None => return Err(CompressError::Capture(CaptureError::Site)),
    };
    // SAFETY: our cursor's word, moved by readback alone.
    let end = DdsOffset::of_cursor(unsafe { cursor.handle().next() });
    if end.past(0, job.size).is_none() {
        return Err(CompressError::End { end, size: job.size });
    }
    Ok(Readback { header, levels: kept, end, last: u8::from(kind == TextureKind::D) })
}

/// Level 0 of the compressor: a texture of the target's size and format
/// (D3D11_USAGE_DEFAULT, one level, as the compressor's own, 0x11F46D)
/// with the target's texels copied on the GPU (copy2D, as the stock copy to
/// the staging). Nothing is read back or uploaded.
fn level0(device: Device, extent: Extent, source: raw::ResTexture) -> Option<HeldTexture> {
    let Extent { width, height } = extent;
    // SAFETY: create_texture2d (0x3BEA70) with no name and no initial data,
    // on the bake's thread.
    let texture = unsafe {
        raw::engine_create_texture2d(width, Some(device.manager), null(), height, null(), RGBA8, DEFAULT_USAGE, 1, 0)
    }?;
    let held = HeldTexture::hold(texture);
    // SAFETY: copy2D (0x3C0E00) of the target (alive while the hook runs:
    // the frame holds it) into ours, both width x height.
    unsafe { raw::engine_copy2d(width, height, Some(held.raw()), Some(source)) };
    Some(held)
}

/// The compressor's array: each level's reference let go.
fn release_levels(levels: &[Option<raw::ResTexture>]) {
    for texture in levels.iter().flatten() {
        drop(HeldTexture { texture: Texture::of(*texture), _local: PhantomData });
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
fn copied(ctx: &Context<'_>, size: DdsSize) -> Result<Copied, CaptureError> {
    // SAFETY: compress_mip's site: ESI its staging (a reference it holds).
    let staging = unsafe { raw::site::compress_mip_readback::staging(ctx) }.ok_or(CaptureError::Site)?;
    // SAFETY: as staging: EBX the level's blocks wide.
    let wide = unsafe { raw::site::compress_mip_readback::wide(ctx) };
    // SAFETY: as staging: its frame by ESP, alive while it runs.
    let frame = unsafe { raw::site::compress_mip_readback::frame(ctx) }.ok_or(CaptureError::Site)?;
    // SAFETY: the frame's [esp+30h], &the cursor (compress's, ours).
    let cursor = unsafe { frame.cursor() }.ok_or(CaptureError::Site)?;
    // SAFETY: the frame's [esp+38h], the blocks high.
    let high = unsafe { *frame.high() };
    // SAFETY: the frame's [esp+40h], the format.
    let format = unsafe { *frame.format() };
    // SAFETY: our cursor's word (a place, moved by readback alone).
    let offset = DdsOffset::of_cursor(unsafe { cursor.next() });
    let row_bytes = if format == BC3 { 16 } else { 8 } * wide;
    let fit = CaptureError::Fit { offset, size };
    let bytes = row_bytes.checked_mul(high).ok_or(fit)?;
    if offset < DdsOffset::LEVELS || offset.past(bytes, size).is_none() {
        return Err(fit);
    }
    Ok(Copied { staging, place: Place { offset, rows: high, row_bytes }, bytes, cursor })
}

/// Whether the compress at a `compress_mip.readback` is this thread's under
/// [`compress`]: no (the game's map), with its DDS's size, or broken.
enum Mine {
    No,
    Yes(DdsSize),
    Broken,
}

/// `compress_mip.readback`, a level copied to its staging. During
/// [`compress`] on this thread every level is kept (true: leave by `kept`,
/// no map, no copy): its staging with a reference more and its place noted,
/// the cursor moved past its blocks as 0x11F863 moves it. One that does not
/// fit breaks the capture (and the bake), unmapped too. Any other compress
/// (another thread's, or not ours): false, the game's map.
pub fn readback(ctx: &mut Context<'_>) -> bool {
    let thread = ctx.thread();
    let mine = CAPTURE.with(thread, |c| match c.as_ref().and_then(|bound| bound.get(thread)) {
        None => Mine::No,
        Some(Capture { kept: Kept::Broken(_), .. }) => Mine::Broken,
        Some(Capture { size, kept: Kept::Levels(_) }) => Mine::Yes(*size),
    });
    let size = match mine {
        Mine::No => return false,
        Mine::Broken => return true,
        Mine::Yes(size) => size,
    };
    match copied(ctx, size) {
        Ok(Copied { staging, place, bytes, cursor }) => {
            // the staging is alive at the site (compress_mip holds it until
            // its release at 0x11F871); the reference taken keeps it after
            let level = Level { staging: HeldTexture::hold(staging), place };
            let refused = CAPTURE.with(thread, |c| match c.as_mut().and_then(|bound| bound.get_mut(thread)) {
                Some(capture) => capture.keep(level).err(),
                None => Some((level, None)),
            });
            match refused {
                None => {
                    // SAFETY: our cursor's word (a place: never written through).
                    let next = unsafe { cursor.next() }.wrapping_add(bytes as usize);
                    // SAFETY: the cursor past the level's bytes, as 0x11F863 moves it.
                    unsafe { cursor.set_next(next) }
                }
                // let go out of the lock
                Some(refused) => drop(refused),
            }
        }
        Err(why) => {
            let held = CAPTURE.with(thread, |c| {
                c.as_mut().and_then(|bound| bound.get_mut(thread)).and_then(|capture| capture.broken(why))
            });
            // let go out of the lock
            drop(held);
        }
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

/// A level mapped (map2D): its rows; unmapped when dropped, through the
/// context and the resource it was mapped with (the immediate context's
/// Unmap, subresource 0, as 0x11F86F).
struct Mapped<'a> {
    context: raw::ImmediateContext,
    resource: raw::HwResource,
    data: NonNull<u8>,
    pitch: u32,
    rows: u32,
    _staging: &'a HeldTexture,
}

impl Mapped<'_> {
    /// The staging of a level of `rows` rows mapped; none while the GPU has
    /// it (DoNotWait) or when it cannot be (the device lost).
    fn of(context: raw::ImmediateContext, staging: &HeldTexture, rows: u32, wait: Wait) -> Option<Mapped<'_>> {
        // SAFETY: a staging we hold: its +1ACh, its resource.
        let resource = unsafe { staging.raw().hw_resource() }?;
        let mut pitch = 0;
        // SAFETY: map2D (0x3D0F80) of a staging (usage 3) we hold, on the
        // bake's thread; its byte 1 the no-wait flag (0 back while the GPU
        // has not done it).
        let data = unsafe { raw::engine_map2d(Some(staging.raw()), &mut pitch, u32::from(wait == Wait::DoNotWait)) };
        NonNull::new(data.cast::<u8>()).map(|data| Mapped { context, resource, data, pitch, rows, _staging: staging })
    }

    /// The first `len` bytes of a row; none past the rows or the pitch.
    fn row(&self, row: u32, len: u32) -> Option<&[u8]> {
        if row >= self.rows || len > self.pitch {
            return None;
        }
        // SAFETY: a mapped staging of `rows` rows holds `pitch` bytes a row
        // (D3D11_MAPPED_SUBRESOURCE), alive until the unmap (our drop).
        Some(unsafe {
            core::slice::from_raw_parts(self.data.as_ptr().add(row as usize * self.pitch as usize), len as usize)
        })
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
    /// done: the read says what is lost.
    fn progress(&self) -> Progress {
        let busy = match (immediate_context(), self.levels.iter().last()) {
            (Some(context), Some(level)) => {
                Mapped::of(context, &level.staging, level.place.rows, Wait::DoNotWait).is_none()
            }
            _ => false,
        };
        if busy { Progress::Busy } else { Progress::Done }
    }

    /// The DDS of `size` bytes, allocated now (MT memory), with its header,
    /// each level's blocks (the rows at the staging's pitch; waits for the
    /// GPU when it has not done them) and its last byte; each staging let go.
    fn read(self, size: DdsSize) -> Result<DdsBuffer, ReadError> {
        let Readback { header, levels, end, last } = self;
        let mut dds = DdsBuffer::allocate(size).ok_or(ReadError::NoMemory)?;
        let context = immediate_context().ok_or(ReadError::Lost)?;
        let bytes = dds.bytes();
        bytes.get_mut(..HEADER).ok_or(ReadError::Lost)?.copy_from_slice(&header);
        for level in levels.iter() {
            let Place { offset, rows, row_bytes } = level.place;
            let mapped = Mapped::of(context, &level.staging, rows, Wait::Block).ok_or(ReadError::Lost)?;
            for row in 0..rows {
                let at = offset.past(row * row_bytes, size).ok_or(ReadError::Lost)?;
                let into = bytes.get_mut(at.span(row_bytes as usize)).ok_or(ReadError::Lost)?;
                into.copy_from_slice(mapped.row(row, row_bytes).ok_or(ReadError::Lost)?);
            }
        }
        *bytes.get_mut(end.index()).ok_or(ReadError::Lost)? = last;
        Ok(dds)
    }
}

// ---- a bake on the GPU, completed ------------------------------------------------------

/// A bake on the GPU: its job and its stagings (one value: the job's size is
/// the one its DDS is read into and its tail is given), the ms its issue
/// took and since when the GPU has it. Completed once ([`complete`](Self::complete)).
pub struct Issued {
    job: Job,
    readback: Readback,
    issue_ms: Ms,
    since: Stamp,
}

/// How a completed bake ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The texture made and used; the save's descriptor took the DDS.
    Saved,
    /// Made and used; no memory for the save's descriptor: not saved.
    Unsaved,
    /// Made and used; not saved: the saves not written yet hold their
    /// bound of MT memory.
    SavesFull(Pending),
    /// The tail did not reach the save (the DDS freed here).
    NoSave,
    /// Its levels did not come back: the query failed.
    Failed(ReadError),
}

/// A completed bake: its name and size, for its line, and how it ended.
pub struct Completion {
    pub name: JobName,
    pub size: DdsSize,
    pub outcome: Outcome,
}

impl Issued {
    pub fn name(&self) -> &str {
        self.job.name()
    }

    /// Since when the GPU has it (a smaller stamp: issued earlier).
    pub fn since(&self) -> Stamp {
        self.since
    }

    /// The ms process_compression ran until the bake was issued, and its
    /// draw's (render_bake's): the cook line's "bake" is their sum.
    pub fn issue_ms(&self) -> (Ms, Ms) {
        (self.issue_ms, self.job.draw_ms)
    }

    /// Whether the GPU has done it (one no-wait map of its last staging).
    pub fn progress(&self) -> Progress {
        self.readback.progress()
    }

    /// The bake completed on this thread: its DDS allocated and read back
    /// (waiting when the GPU has not done it), then process_compression's
    /// tail run with its job ([`resume`]): the texture made, shared, saved,
    /// recorded, its query finished. No MT memory, or a level lost, fails
    /// its query. Given back, untouched, while another completion is under
    /// way (its tail: one at a time).
    pub fn complete(self, thread: &Thread) -> Result<Completion, Issued> {
        let Some(claim) = Claim::take(thread) else { return Err(self) };
        let Issued { job, readback, issue_ms, .. } = self;
        let (name, size) = (job.name_copy(), job.size);
        let started = Stamp::now(thread);
        let outcome = match readback.read(job.size) {
            Ok(dds) => claim.tail(job, dds, issue_ms, started),
            Err(error) => {
                job.fail();
                Outcome::Failed(error)
            }
        };
        Ok(Completion { name, size, outcome })
    }
}

// ---- the tail ----------------------------------------------------------------------

/// What [`resume`] gives process_compression's frame.
#[derive(Clone, Copy)]
struct Resume {
    dds: NonNull<u8>,
    extent: Extent,
    kind: TextureKind,
    issue_ms: Ms,
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
    /// The saves not written yet are at their bound ([`SAVES`]): no
    /// descriptor, no save.
    Held(Pending),
}

/// A completion under way on its thread: its levels read back, the call
/// made (its prologue not reached yet), or in its tail.
#[derive(Clone, Copy)]
enum Resuming {
    Reading,
    Calling(Resume),
    Tail { resume: Resume, save: Save },
}

/// What happens to a completion under way.
#[derive(Clone, Copy)]
enum Event {
    /// Its levels read into its DDS: process_compression called with it.
    Call(Resume),
    /// The call's prologue reached (`bake.resume`): its tail runs.
    Resumed,
    /// The tail's save made, or skipped (`bake.save`).
    Saved(Save),
}

impl Resuming {
    /// The one transition. Its start (Reading) and its end are [`Claim`]'s;
    /// an event out of its order leaves the state as it is.
    fn on(self, event: Event) -> Resuming {
        match (self, event) {
            (Resuming::Reading, Event::Call(resume)) => Resuming::Calling(resume),
            (Resuming::Calling(resume), Event::Resumed) => Resuming::Tail { resume, save: Save::Pending },
            (Resuming::Tail { resume, save: Save::Pending }, Event::Saved(save)) => Resuming::Tail { resume, save },
            (state, _) => state,
        }
    }
}

/// The completion under way (one at a time), its thread's.
static RESUMING: Lock<Option<ThreadBound<Resuming>>> = Lock::new(None);

/// This thread's completion advanced by `event` (none: nothing).
fn advance(thread: &Thread, event: Event) {
    RESUMING.with(thread, |r| {
        if let Some(state) = r.as_mut().and_then(|bound| bound.get_mut(thread)) {
            *state = state.on(event);
        }
    });
}

/// The completion under way this thread holds ([`RESUMING`]), ended when dropped.
struct Claim<'t> {
    thread: &'t Thread,
}

impl<'t> Claim<'t> {
    /// Taken for this thread, only when no completion is under way.
    fn take(thread: &'t Thread) -> Option<Claim<'t>> {
        let taken = RESUMING.with(thread, |r| match r {
            Some(_) => false,
            None => {
                *r = Some(ThreadBound::new(thread, Resuming::Reading));
                true
            }
        });
        taken.then_some(Claim { thread })
    }

    fn advance(&self, event: Event) {
        advance(self.thread, event);
    }

    /// Where the completion got to; it is over.
    fn end(self) -> Option<Resuming> {
        let thread = self.thread;
        core::mem::forget(self);
        RESUMING.with(thread, Option::take)?.into_inner(thread).ok()
    }

    /// The bake the GPU has done (its levels in `dds`): process_compression
    /// called again with the job, its prologue jumping to its tail
    /// ([`resume`]), which makes the texture from the DDS, puts it in the
    /// surface's slot, shares it (patch 70), saves it ([`save`]: no copy),
    /// lists it, finishes the query and logs "camouflage/decal cook" (bake:
    /// the ms its thread spent until issued, with the draw's; compress: the
    /// ms since `started`, the readback's start).
    fn tail(self, job: Job, dds: DdsBuffer, issue_ms: Ms, started: Stamp) -> Outcome {
        let Job { cook, query, surface, model, kind, extent, size: _, name, pair: [first, second], value, draw_ms } = job;
        let mut parameters = raw::BakeParametersCopyStorage::zeroed();
        let mut pair = raw::BakePairStorage::zeroed();
        let extent_words = [extent.width, extent.height];
        // SAFETY: our parameters copy's +13Ch: the name, NUL-ended, alive past the call.
        unsafe { parameters.handle().set_name(name.as_ptr().cast_mut()) };
        // SAFETY: our pair's first word.
        unsafe { *pair.handle().first() = first };
        // SAFETY: our pair's second word.
        unsafe { *pair.handle().second() = second };
        let query = query.into_tail();
        self.advance(Event::Call(Resume { dds: dds.at, extent, kind, issue_ms, started }));
        // SAFETY: process_compression (its thunk: EAX the extent, the ten
        // arguments) with the job's copies, on the bake's thread: the cook,
        // the query (not finished: the tail finishes it), no targets (the
        // tail does not read them: resume leaves the prologue for 0x12F00B),
        // the size `dds` was allocated with (the job's), the surface (its model held), our
        // parameters copy and pair (alive past the call), the value, the
        // type, the draw's ms.
        unsafe {
            raw::engine_process_compression(extent_words.as_ptr(), Some(cook), Some(query), null(),
                                            dds.size.0, Some(surface), Some(parameters.handle()), Some(pair.handle()),
                                            value, kind.raw(), draw_ms.raw())
        }
        let outcome = match self.end() {
            Some(Resuming::Tail { save: Save::Owned, .. }) => Outcome::Saved,
            Some(Resuming::Tail { save: Save::Skipped, .. }) => Outcome::Unsaved,
            Some(Resuming::Tail { save: Save::Held(pending), .. }) => Outcome::SavesFull(pending),
            Some(Resuming::Tail { save: Save::Pending, .. } | Resuming::Calling(_) | Resuming::Reading) | None => {
                Outcome::NoSave
            }
        };
        if outcome == Outcome::Saved {
            // the descriptor frees it once written
            core::mem::forget(dds);
        }
        drop(model);
        outcome
    }
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        drop(RESUMING.with(self.thread, Option::take));
    }
}

/// The completion's state on this thread.
fn resuming(thread: &Thread) -> Option<Resuming> {
    RESUMING.with(thread, |r| r.as_ref().and_then(|bound| bound.get(thread)).copied())
}

/// `bake.resume`, process_compression's prologue done: for the call
/// [`Claim::tail`] makes on this thread, the locals its tail reads (the
/// DDS, no target, the size, no log object, the issue's ms, the timer
/// started at the readback) and the registers it has at 0x12F00B (EBX no
/// RGBA, ESI the timer, EDI the DDS, EBP the type); true to leave by
/// `tail`. Any other call: false.
pub fn resume(ctx: &mut Context<'_>) -> bool {
    let thread = ctx.thread();
    let Some(Resuming::Calling(r)) = resuming(thread) else { return false };
    // SAFETY: the context is this site's: its ESP is the frame.
    let Some(frame) = (unsafe { raw::site::bake_resume::frame(&*ctx) }) else { return false };
    let f = Frame { frame, _hook: PhantomData };
    f.set_dds(r.dds);
    f.clear_source();
    f.clear_texture();
    f.put_width(r.extent.width);
    f.put_height(r.extent.height);
    f.put_log_object(0);
    f.put_bake_ms(r.issue_ms.raw());
    f.put_timer_base_low(0);
    f.put_timer_base_high(0);
    f.put_timer_start_low(r.started.low());
    f.put_timer_start_high(r.started.high());
    // the prologue's two 1.0s (0x12EC73, 0x12EC79), the first the timer's
    // scale (elapsed_ticks multiplies by it)
    f.put_timer_scale(TIMER_SCALE);
    f.put_prologue_scale(TIMER_SCALE);
    let timer = f.timer_address();
    advance(thread, Event::Resumed);
    ctx.set_reg(Reg::Ebx, 0);
    ctx.set_reg(Reg::Esi, timer);
    ctx.set_reg(Reg::Edi, r.dds.as_ptr() as usize as u32);
    ctx.set_reg(Reg::Ebp, r.kind.raw());
    true
}

/// The resumed bake's DDS and its save, when this thread's tail runs.
fn tail(thread: &Thread) -> Option<(NonNull<u8>, Save)> {
    match resuming(thread)? {
        Resuming::Tail { resume, save } => Some((resume.dds, save)),
        Resuming::Reading | Resuming::Calling(_) => None,
    }
}

// ---- the saves not written yet ------------------------------------------------------------

/// The most bakes' saves not written yet, and their most MT bytes. A save's
/// descriptor owns its DDS until ~query_result destroys it (0xF1135, after
/// the device manager wrote it), and the device manager's queue is the
/// load's reads': in the 2026-11-09 match load (458 bakes in 11 s, 774 MB of
/// DDS) MT went to 99.3% and the game crashed. Past either bound a bake's
/// texture is made and used, not saved (its DDS freed at once): baked again
/// next game.
pub const SAVES: usize = 64;
pub const SAVE_BYTES: u32 = 128 << 20;

/// The saves not written yet: how many, and their bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pending {
    pub saves: u32,
    pub bytes: u32,
}

/// A DDS a save descriptor owns: its address (an id, never read through)
/// and size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Held {
    address: u32,
    size: u32,
}

/// The bakes' DDS buffers save descriptors own, not freed yet.
struct Saves {
    held: [Option<Held>; SAVES],
    bytes: u32,
}

impl Saves {
    const fn new() -> Saves {
        Saves { held: [None; SAVES], bytes: 0 }
    }

    fn pending(&self) -> Pending {
        Pending { saves: self.held.iter().flatten().count() as u32, bytes: self.bytes }
    }

    /// A save of `size` bytes at `address` taken on, within the bounds; else
    /// what is pending.
    fn reserve(&mut self, address: u32, size: u32) -> Result<(), Pending> {
        let pending = self.pending();
        let bytes = self.bytes.checked_add(size).filter(|&bytes| bytes <= SAVE_BYTES).ok_or(pending)?;
        let slot = self.held.iter_mut().find(|slot| slot.is_none()).ok_or(pending)?;
        *slot = Some(Held { address, size });
        self.bytes = bytes;
        Ok(())
    }

    /// The save at `address` freed: whether it was one of ours.
    fn release(&mut self, address: u32) -> bool {
        let Some(slot) = self.held.iter_mut().find(|slot| slot.is_some_and(|held| held.address == address)) else {
            return false;
        };
        if let Some(held) = slot.take() {
            self.bytes -= held.size;
        }
        true
    }
}

static SAVES_HELD: Lock<Saves> = Lock::new(Saves::new());

/// `save.freed`, a save descriptor destroyed: when it owns one of the
/// bakes' DDS buffers, that save is no longer pending (the destructor frees
/// it next, 0x10A083).
pub fn freed(ctx: &mut Context<'_>) {
    let thread = ctx.thread();
    // SAFETY: the site's ESI: the descriptor being destroyed, alive at its
    // destructor's start.
    let Some(descriptor) = (unsafe { raw::site::save_freed::descriptor(&*ctx) }) else { return };
    // SAFETY: its +8, whether it frees its data.
    let owns = unsafe { *descriptor.owns() };
    // SAFETY: its +0, the data.
    let data = unsafe { descriptor.data() };
    if owns != 0 && !data.is_null() {
        SAVES_HELD.with(thread, |s| s.release(data as usize as u32));
    }
}

/// `bake.save`, the tail about to make the save's descriptor: for a resumed
/// bake's DDS, a descriptor of the game's (its two paths, allocate_save_
/// descriptor) that owns the DDS itself (its +8: the destructor frees it
/// with MT's free), so nothing is copied; the '/' the site writes, then EAX
/// the descriptor; true to leave by `made`. With no memory for a
/// descriptor, EAX 0 and `made` too: the query's +0E0h is 0, so
/// finish_translated_query saves nothing (the game's make_save_descriptor
/// would write through the 0 its allocation gives, 0x109EFD). With the
/// saves not written yet at their bound ([`SAVES`], [`SAVE_BYTES`]), the
/// same: no descriptor, no save. Any other bake: false, the game's
/// make_save_descriptor copies its bytes.
pub fn save(ctx: &mut Context<'_>) -> bool {
    let thread = ctx.thread();
    let Some((dds, Save::Pending)) = tail(thread) else { return false };
    // EBX is the DDS; the bytes' data is written after the site (0x12F2E0), their size before it.
    let data = dds.as_ptr();
    if ctx.reg(Reg::Ebx) != data as usize as u32 {
        return false;
    }
    // SAFETY: the site's ESP: its frame, alive while the tail runs.
    let Some(frame) = (unsafe { raw::site::bake_save::frame(&*ctx) }) else { return false };
    // SAFETY: the frame's [esp]: the bytes the tail pushed (&[esp+28h]).
    let Some(bytes) = (unsafe { frame.bytes() }) else { return false };
    // SAFETY: as the bytes: ECX the physical path.
    let physical = unsafe { raw::site::bake_save::physical(&*ctx) };
    // SAFETY: as the bytes: EAX the virtual path.
    let virtual_path = unsafe { raw::site::bake_save::virtual_path(&*ctx) };
    // SAFETY: the bytes' data, set as 0x12F2E0 sets it (the exit skips it).
    unsafe { bytes.set_data(data) };
    // SAFETY: the '/' of the instruction the exit skips ([esp+5F8h]).
    unsafe { *frame.separator() = b'/' };
    // SAFETY: the bytes' count, written by the tail (0x12F2C5).
    let size = unsafe { *bytes.size() };
    let address = data as usize as u32;
    if let Err(pending) = SAVES_HELD.with(thread, |s| s.reserve(address, size)) {
        // no descriptor: the query's +0E0h 0, no save; the DDS freed after the tail
        advance(thread, Event::Saved(Save::Held(pending)));
        ctx.set_reg(Reg::Eax, 0);
        return true;
    }
    // SAFETY: the game's allocation of a descriptor with its two paths
    // (copied), any thread.
    let allocated = unsafe { raw::engine_allocate_save_descriptor(physical, virtual_path) };
    if allocated.is_none() {
        SAVES_HELD.with(thread, |s| s.release(address));
    }
    let (save, eax) = match allocated {
        Some(descriptor) => {
            // SAFETY: the new descriptor's data, as make_save_descriptor
            // sets it (0x109EFD): ours given to it.
            unsafe { descriptor.set_data(data) };
            // SAFETY: its count (0x109F0A).
            unsafe { *descriptor.size() = size };
            // SAFETY: its owns byte (0x109F1B): the destructor frees the data.
            unsafe { *descriptor.owns() = 1 };
            (Save::Owned, descriptor.address())
        }
        None => (Save::Skipped, 0),
    };
    advance(thread, Event::Saved(save));
    ctx.set_reg(Reg::Eax, eax);
    true
}

/// `bake.free`, the tail about to free the DDS to the render allocator: a
/// resumed bake's DDS is MT memory (a descriptor's, or [`Claim::tail`]
/// frees it), so EBX 0 skips that free.
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
        assert_eq!(first.past(0x100, DdsSize(0x181)), Some(DdsOffset(0x180)));
        assert_eq!(first.past(0x100, DdsSize(0x180)), None, "the last byte follows the levels");
        assert_eq!(DdsOffset(u32::MAX).past(1, DdsSize(u32::MAX)), None);
        assert_eq!(first.span(4), 0x80..0x84);
    }

    #[test]
    fn a_dds_holds_its_header_levels_and_last_byte() {
        assert!(DdsSize(0x181).holds(0x100));
        assert!(!DdsSize(0x180).holds(0x100));
        assert!(!DdsSize(u32::MAX).holds(u32::MAX));
    }

    #[test]
    fn milliseconds_add() {
        let mut spent = Ms::default();
        spent += Ms::new(2.5);
        assert_eq!(spent + Ms::new(0.5), Ms::new(3.0));
        assert!(Ms::new(6.0) > spent);
        assert_eq!(Ms::new(999.9).whole(), 999);
    }

    #[test]
    fn a_completion_goes_read_call_tail_save_in_order() {
        let resume = Resume { dds: NonNull::dangling(), extent: Extent { width: 64, height: 64 }, kind: TextureKind::D,
                              issue_ms: Ms::new(1.0), started: Stamp(7) };
        let tail = |state: Resuming| match state {
            Resuming::Tail { save: Save::Pending, .. } => "pending",
            Resuming::Tail { save: Save::Owned, .. } => "owned",
            Resuming::Tail { save: Save::Skipped, .. } => "skipped",
            Resuming::Tail { save: Save::Held(_), .. } => "held",
            Resuming::Calling(_) => "calling",
            Resuming::Reading => "reading",
        };
        let calling = Resuming::Reading.on(Event::Call(resume));
        assert_eq!(tail(calling), "calling");
        assert_eq!(tail(Resuming::Reading.on(Event::Resumed)), "reading", "no prologue before the call");
        assert_eq!(tail(calling.on(Event::Saved(Save::Owned))), "calling", "no save before the tail");
        let running = calling.on(Event::Resumed);
        assert_eq!(tail(running), "pending");
        assert_eq!(tail(running.on(Event::Saved(Save::Skipped))), "skipped");
        let saved = running.on(Event::Saved(Save::Owned));
        assert_eq!(tail(saved.on(Event::Saved(Save::Skipped))), "owned", "saved once");
        assert_eq!(tail(saved.on(Event::Call(resume))), "owned");
    }

    #[test]
    fn the_saves_not_written_yet_stay_within_their_bounds() {
        let mut saves = Saves::new();
        for i in 0..SAVES as u32 {
            assert_eq!(saves.reserve(0x1000 + i, 10), Ok(()));
        }
        let full = Pending { saves: SAVES as u32, bytes: 10 * SAVES as u32 };
        assert_eq!(saves.reserve(0x9000, 10), Err(full), "past SAVES of them");
        assert!(saves.release(0x1000) && !saves.release(0x1000), "freed once");
        assert!(!saves.release(0x7777), "not one of ours");
        assert_eq!(saves.reserve(0x9000, 10), Ok(()));
        let mut big = Saves::new();
        assert_eq!(big.reserve(1, SAVE_BYTES - 5), Ok(()));
        assert_eq!(big.reserve(2, 6), Err(Pending { saves: 1, bytes: SAVE_BYTES - 5 }), "past SAVE_BYTES");
        assert_eq!(big.reserve(2, u32::MAX), Err(Pending { saves: 1, bytes: SAVE_BYTES - 5 }));
        assert!(big.release(1));
        assert_eq!(big.pending(), Pending { saves: 0, bytes: 0 });
    }
}

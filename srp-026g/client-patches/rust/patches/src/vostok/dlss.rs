//! The game's side of the link to the DLSS helper (docs/notes/dlss-option.md,
//! "The helper interface"; the protocol is the helper's, client/dlss-helper's
//! `ipc.rs`): what patch 83 asks of Windows, DXVK and Vulkan for it.
//!
//! - [`Helper`]: the 64-bit helper's process and the pipe to it (a named pipe
//!   of ours, the helper its client), each message a typed call; NT handles
//!   are given to it with DuplicateHandle.
//! - [`SwapChain`], [`Device`]: the game's swap chain at a Present, and DXVK's
//!   Vulkan device behind it (`IDXGIVkInteropDevice1`): its handles, its
//!   queue (locked while we submit) and the Vulkan functions, taken from the
//!   module DXVK took them from (`winevulkan.dll` under Wine, else
//!   `vulkan-1.dll`).
//! - [`Shared`]: the four images and the timeline semaphore made and
//!   exported on DXVK's device, the texture the screen image shows, and each
//!   frame's two submits on DXVK's queue: the copy-in of the frame's picture
//!   and depth that signals the frame's inputs, and the wait for the
//!   helper's output with its copy into that texture.
//! - [`ScreenImage`], [`Picture`]: the frame where its picture goes to the
//!   window (`render.screen_image`), and the texture shown in its place.
//! - [`HelperWatch`]: the helper's process as its watch sees it, on a thread
//!   of its own: DXVK's queue waits for a value only the helper signals, so
//!   when the helper is gone the watch signals it ([`release`]), whatever
//!   the game's thread waits for.
//!
//! The rules of when each is used are patch 83's (`hud_options::dlss`).

use core::ffi::{CStr, c_void};
use core::mem::size_of;
use core::ptr::{NonNull, null, null_mut};

use super::OrigThreadEntry;
use super::baker::Texture;
use super::context::Context;
use super::hook::{CallbackEntry, Thread};
pub use super::render_size::Size;
use super::render_size::{ColourTarget, ColourTexture, DepthTexture};
use super::renderer::{Path, beside_exe};
use super::sync::{Lock, ThreadBound};
use crate::raw;

/// `GetTickCount`'s milliseconds, which wrap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tick(u32);

impl Tick {
    pub fn now(_: &Thread) -> Tick {
        // SAFETY: GetTickCount (the exe's import) takes nothing.
        Tick(unsafe { (raw::engine_get_tick_count)() })
    }

    /// A tick of that many milliseconds, for a test.
    pub const fn of(milliseconds: u32) -> Tick {
        Tick(milliseconds)
    }

    /// The milliseconds from `start` to this tick.
    pub fn since(self, start: Tick) -> u32 {
        self.0.wrapping_sub(start.0)
    }
}

/// A frame of one set of shared images, from 0: its two values on their
/// timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame(pub u64);

impl Frame {
    /// The game's inputs of the frame are in the shared images.
    pub fn copied(self) -> u64 {
        2 * self.0 + 1
    }

    /// The helper's output of the frame is in the shared output.
    pub fn upscaled(self) -> u64 {
        2 * self.0 + 2
    }

    pub fn next(self) -> Frame {
        Frame(self.0 + 1)
    }

    /// Whether the frame's two command buffers are free, the timeline at
    /// `reached`: the frame that used them last is [`SLOTS`] before it, and
    /// the frame after that one has been copied, which DXVK's queue does
    /// only after it has run that frame's two buffers.
    pub fn buffers_free(self, reached: u64) -> bool {
        self.0 < SLOTS || reached >= Frame(self.0 - SLOTS + 1).copied()
    }
}

/// The command buffers of a set of shared images, a pair each (the copy-in
/// and the one that waits for the helper's output): a frame uses the pair
/// of its number modulo this.
pub const SLOTS: u64 = 4;

/// The text of a helper's refusal (the reply's 256 bytes, NUL-ended).
pub const REFUSAL_TEXT: usize = 256;

/// The helper's "no": its text and NGX's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub text: [u8; REFUSAL_TEXT],
    pub ngx: u32,
}

impl Refusal {
    fn of(text: &[u8; REFUSAL_TEXT], ngx: i32) -> Refusal {
        Refusal { text: *text, ngx: ngx as u32 }
    }

    pub fn text(&self) -> &str {
        let len = self.text.iter().position(|&b| b == 0).unwrap_or(REFUSAL_TEXT);
        // a text cut inside a character: what is whole before it
        match core::str::from_utf8(&self.text[..len]) {
            Ok(text) => text,
            Err(error) => core::str::from_utf8(&self.text[..error.valid_up_to()]).unwrap_or(""),
        }
    }
}

/// Why the link does not run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The helper was not started: the call that failed, GetLastError.
    Start(&'static str, u32),
    /// DXVK's device or a Vulkan call: which, its result (an HRESULT, a
    /// VkResult; 0 for a function or an interface that is not there).
    Device(&'static str, u32),
    /// The helper's answer to the Hello: DLSS does not run on this PC.
    Refused(Refusal),
    /// The helper's answer to the shared images.
    SetupRefused(Refusal),
    /// The helper's process ended: its exit code.
    Ended(u32),
    /// The pipe: the call that failed, GetLastError.
    Pipe(&'static str, u32),
    /// No answer from a helper that still runs: to what.
    Late(&'static str),
    /// The 3D frame is not rendered at a render size (its depth-stencil
    /// was not made): there is nothing to upscale.
    NoRenderSize,
}

/// A kernel handle of ours (a pipe, a process), open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Handle(NonNull<c_void>);
// SAFETY: a kernel handle is the process's: any thread may use it.
unsafe impl Send for Handle {}

impl Handle {
    fn close(self) {
        // SAFETY: CloseHandle (the exe's import) of a handle of ours, used no more.
        unsafe { (raw::engine_close_handle)(self.0.as_ptr()) };
    }
}

/// The pseudo-handle of this process (GetCurrentProcess's -1).
fn this_process() -> *mut c_void {
    usize::MAX as *mut c_void
}

fn last_error() -> u32 {
    // SAFETY: GetLastError (the exe's import) takes nothing.
    unsafe { (raw::engine_get_last_error)() }
}

/// A function of a DLL the exe does not import it from, by GetProcAddress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Function(NonNull<c_void>);
// SAFETY: code of a loaded module, never let go: any thread may call it.
unsafe impl Send for Function {}

/// kernel32's functions the exe does not import.
#[derive(Clone, Copy)]
struct Kernel {
    create_named_pipe: Function,
    peek_named_pipe: Function,
    duplicate_handle: Function,
    exit_code: Function,
}

impl Kernel {
    fn load() -> Option<Kernel> {
        // SAFETY: GetModuleHandleA and GetProcAddress (the exe's imports) with
        // NUL-terminated names; kernel32 is loaded for the life of the process.
        let function = |name: &CStr| unsafe {
            let kernel = (raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
            NonNull::new((raw::engine_get_proc_address)(kernel, name.as_ptr().cast())).map(Function)
        };
        Some(Kernel {
            create_named_pipe: function(c"CreateNamedPipeA")?,
            peek_named_pipe: function(c"PeekNamedPipe")?,
            duplicate_handle: function(c"DuplicateHandle")?,
            exit_code: function(c"GetExitCodeProcess")?,
        })
    }

    /// `handle` of this process copied into `into`'s handle table (the
    /// pseudo-handle: this process's), the same access.
    fn duplicate(&self, handle: *mut c_void, into: *mut c_void) -> Result<*mut c_void, u32> {
        const DUPLICATE_SAME_ACCESS: u32 = 2;
        let mut copy = null_mut();
        // SAFETY: DuplicateHandle (kernel32's, by its name) with two open
        // process handles and room for the copy.
        let made = unsafe {
            raw::engine_duplicate_handle(
                self.duplicate_handle.0,
                this_process(),
                handle,
                into,
                &mut copy,
                0,
                0,
                DUPLICATE_SAME_ACCESS,
            )
        };
        if made == 0 { Err(last_error()) } else { Ok(copy) }
    }
}

// ---- the pipe's messages (the helper's ipc.rs: fixed-size little-endian
// records, the same layout in the 32-bit game and the 64-bit helper; a u64
// lies at a multiple of 8 in each, so Rust's i686 layout is the helper's)

const MAGIC: u32 = u32::from_le_bytes(*b"DLSS");
const VERSION: u32 = 3;
/// NGX's NVSDK_NGX_PerfQuality_Value_MaxQuality: DLSS at Quality.
const QUALITY: i32 = 2;
/// NVSDK_NGX_DLSS_Feature_Flags_MVLowRes: motion at the render size.
const DLSS_FLAGS: i32 = 1 << 1;

/// A record that crosses the pipe as its bytes.
///
/// # Safety
/// A `#[repr(C)]` struct of integers and floats without padding: every byte
/// pattern is a value, and every byte of a value is set.
unsafe trait Wire: Copy {}

#[repr(C)]
#[derive(Clone, Copy)]
struct HelloMsg {
    magic: u32,
    version: u32,
    device_uuid: [u8; 16],
    device_luid: [u8; 8],
    out_w: u32,
    out_h: u32,
    quality: i32,
    helper_mode: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct HelloReply {
    ok: u32,
    ngx_result: i32,
    render_w: u32,
    render_h: u32,
    text: [u8; 256],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SharedImageMsg {
    handle: u64,
    alloc_size: u64,
    memory_type: u32,
    format: u32,
    width: u32,
    height: u32,
    usage: u32,
    pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SetupMsg {
    img: [SharedImageMsg; 4],
    semaphore: u64,
    dlss_flags: i32,
    pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SetupReply {
    ok: u32,
    ngx_result: i32,
    vram_before_feature: u64,
    vram_after_feature: u64,
    import_bytes: u64,
    text: [u8; 256],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FrameMsg {
    frame: u64,
    wait_value: u64,
    signal_value: u64,
    jitter_x: f32,
    jitter_y: f32,
    mv_scale_x: f32,
    mv_scale_y: f32,
    reset: u32,
    /// 0: a frame; 1: quit; 2: a new size (a HelloMsg and a SetupMsg follow).
    quit: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct FrameStat {
    frame: u64,
    ts_begin: u64,
    ts_eval_begin: u64,
    ts_eval_end: u64,
    ts_end: u64,
    tick_ns: f64,
    record_ms: f64,
}

const _: () = assert!(size_of::<HelloMsg>() == 48);
const _: () = assert!(size_of::<HelloReply>() == 272);
const _: () = assert!(size_of::<SharedImageMsg>() == 40);
const _: () = assert!(size_of::<SetupMsg>() == 176 && core::mem::offset_of!(SetupMsg, semaphore) == 160);
const _: () = assert!(size_of::<SetupReply>() == 288 && core::mem::offset_of!(SetupReply, text) == 32);
const _: () = assert!(size_of::<FrameMsg>() == 48);
const _: () = assert!(size_of::<FrameStat>() == 56);

// SAFETY: each is as the trait asks (the sizes above leave no padding).
unsafe impl Wire for HelloMsg {}
unsafe impl Wire for HelloReply {}
unsafe impl Wire for SetupMsg {}
unsafe impl Wire for SetupReply {}
unsafe impl Wire for FrameMsg {}
unsafe impl Wire for FrameStat {}

/// The message between frames that is no frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Between {
    Quit = 1,
    NewSize = 2,
}

/// The helper's report of a frame it finished: which, and how long DLSS's
/// evaluate took on the GPU (its timestamps).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub frame: u64,
    pub dlss_microseconds: u32,
}

/// Whether a Hello went out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hello {
    Sent,
    /// The helper has not opened the pipe yet.
    NotOpen,
}

/// The helper's process and the pipe to it. Its one owner ends it
/// ([`Helper::close`]); the watch has a handle of its own.
pub struct Helper {
    process: Handle,
    pipe: Handle,
    kernel: Kernel,
}

/// The helper's folder beside the exe and its two files
/// (docs/notes/dlss-option.md, "The option").
pub const HELPER_FOLDER: &CStr = c"dlss";
pub const HELPER_FILES: [&CStr; 2] = [c"dlss\\dlss_helper.exe", c"dlss\\nvngx_dlss.dll"];

impl Helper {
    /// The pipe made (`\\.\pipe\skhid-dlss-<process id>`, one instance, byte
    /// mode, 64 KiB each way) and the helper started with its name, in the
    /// helper's own folder, no window, nothing inherited.
    pub fn start(_: &Thread) -> Result<Helper, Failure> {
        const PIPE_ACCESS_DUPLEX: u32 = 3;
        const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 1 << 19;
        const CREATE_NO_WINDOW: u32 = 1 << 27;
        let kernel = Kernel::load().ok_or(Failure::Start("GetProcAddress", last_error()))?;
        let too_long = Failure::Start("the helper's path", 0);
        let mut name = Path::empty();
        name.push(b"\\\\.\\pipe\\skhid-dlss-").ok_or(too_long)?;
        // SAFETY: GetCurrentProcessId (the exe's import) takes nothing.
        let mut id = unsafe { (raw::engine_get_current_process_id)() };
        let mut digits = [0u8; 10];
        let mut first = digits.len();
        loop {
            first -= 1;
            digits[first] = b'0' + (id % 10) as u8;
            id /= 10;
            if id == 0 {
                break;
            }
        }
        name.push(&digits[first..]).ok_or(too_long)?;
        let exe = beside_exe(HELPER_FILES[0]).ok_or(too_long)?;
        let folder = beside_exe(HELPER_FOLDER).ok_or(too_long)?;
        let mut command = Path::empty();
        [b"\"".as_slice(), exe.text(), b"\" ", name.text()]
            .into_iter()
            .try_for_each(|part| command.push(part))
            .ok_or(too_long)?;
        // SAFETY: CreateNamedPipeA (kernel32's, by its name) with a
        // NUL-terminated name; the first instance of the name, or none.
        let pipe = unsafe {
            raw::engine_create_named_pipe_a(
                kernel.create_named_pipe.0,
                name.c(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                0,
                1,
                1 << 16,
                1 << 16,
                0,
                null_mut(),
            )
        };
        // INVALID_HANDLE_VALUE (-1) is its failure
        let Some(pipe) = NonNull::new(pipe).filter(|pipe| pipe.as_ptr() as isize != -1).map(Handle) else {
            return Err(Failure::Start("CreateNamedPipe", last_error()));
        };
        // STARTUPINFOA (68 bytes, its size first) and PROCESS_INFORMATION (the process, its first thread, their ids)
        let mut startup = [0u32; 17];
        startup[0] = size_of::<[u32; 17]>() as u32;
        let mut made = [0usize; 4];
        // SAFETY: CreateProcessA (the exe's import): the command is ours to
        // write in, the folder NUL-terminated, the two structures its own.
        let started = unsafe {
            (raw::engine_create_process_a)(
                null(),
                command.c_mut(),
                null_mut(),
                null_mut(),
                0,
                CREATE_NO_WINDOW,
                null_mut(),
                folder.c(),
                startup.as_mut_ptr().cast(),
                made.as_mut_ptr().cast(),
            )
        };
        if started == 0 {
            let error = last_error();
            pipe.close();
            return Err(Failure::Start("CreateProcess", error));
        }
        if let Some(thread) = NonNull::new(made[1] as *mut c_void) {
            Handle(thread).close();
        }
        match NonNull::new(made[0] as *mut c_void) {
            Some(process) => Ok(Helper { process: Handle(process), pipe, kernel }),
            None => {
                pipe.close();
                Err(Failure::Start("CreateProcess", 0))
            }
        }
    }

    /// Its exit code once its process has ended.
    pub fn ended(&self, _: &Thread) -> Option<u32> {
        // SAFETY: WaitForSingleObject (the exe's import) without waiting and
        // GetExitCodeProcess (kernel32's, by its name) on our process handle.
        unsafe {
            if (raw::engine_wait_for_single_object)(self.process.0.as_ptr(), 0) != 0 {
                return None;
            }
            let mut code = 0;
            raw::engine_get_exit_code_process(self.kernel.exit_code.0, self.process.0.as_ptr(), &mut code);
            Some(code)
        }
    }

    fn write<M: Wire>(&self, message: &M) -> Result<(), u32> {
        let mut written = 0;
        // SAFETY: WriteFile (the exe's import) of a Wire record's bytes to
        // our pipe: a record fits the pipe's buffer, so it never waits long.
        let ok = unsafe {
            (raw::engine_write_file)(
                self.pipe.0.as_ptr(),
                (message as *const M).cast(),
                size_of::<M>() as u32,
                &mut written,
                null_mut(),
            )
        };
        if ok == 0 {
            Err(last_error())
        } else if written as usize != size_of::<M>() {
            Err(0)
        } else {
            Ok(())
        }
    }

    /// The next record when all of it has come (the pipe is never read past
    /// what it holds, so the game never waits in it).
    fn read<M: Wire>(&self, what: &'static str) -> Result<Option<M>, Failure> {
        let mut available = 0;
        // SAFETY: PeekNamedPipe (kernel32's, by its name) reads nothing: it
        // counts the bytes there.
        let peeked = unsafe {
            raw::engine_peek_named_pipe(
                self.kernel.peek_named_pipe.0,
                self.pipe.0.as_ptr(),
                null_mut(),
                0,
                null_mut(),
                &mut available,
                null_mut(),
            )
        };
        if peeked == 0 {
            return Err(Failure::Pipe(what, last_error()));
        }
        if (available as usize) < size_of::<M>() {
            return Ok(None);
        }
        let mut message = core::mem::MaybeUninit::<M>::zeroed();
        let mut read = 0;
        // SAFETY: ReadFile (the exe's import) of a Wire record's size, all
        // of it there, into room for one.
        let ok = unsafe {
            (raw::engine_read_file)(
                self.pipe.0.as_ptr(),
                message.as_mut_ptr().cast(),
                size_of::<M>() as u32,
                &mut read,
                null_mut(),
            )
        };
        if ok == 0 || read as usize != size_of::<M>() {
            return Err(Failure::Pipe(what, last_error()));
        }
        // SAFETY: a Wire record: zeroed is a value, and so are the bytes read over it.
        Ok(Some(unsafe { message.assume_init() }))
    }

    /// The Hello: DXVK's GPU, the output's size, DLSS at Quality.
    pub fn hello(&self, _: &Thread, device: &Device, output: Size) -> Result<Hello, Failure> {
        /// WriteFile's error on a pipe no client has opened.
        const ERROR_PIPE_LISTENING: u32 = 536;
        let message = HelloMsg {
            magic: MAGIC,
            version: VERSION,
            device_uuid: device.gpu.uuid,
            device_luid: device.gpu.luid,
            out_w: output.width,
            out_h: output.height,
            quality: QUALITY,
            helper_mode: 0,
        };
        match self.write(&message) {
            Ok(()) => Ok(Hello::Sent),
            Err(ERROR_PIPE_LISTENING) => Ok(Hello::NotOpen),
            Err(error) => Err(Failure::Pipe("the Hello", error)),
        }
    }

    /// The Hello's answer once it has come: the render size, or the refusal.
    pub fn hello_answer(&self, _: &Thread) -> Result<Option<Result<Size, Refusal>>, Failure> {
        Ok(self.read::<HelloReply>("the Hello's answer")?.map(|reply| {
            if reply.ok == 1 && reply.render_w != 0 && reply.render_h != 0 {
                Ok(Size { width: reply.render_w, height: reply.render_h })
            } else {
                Err(Refusal::of(&reply.text, reply.ngx_result))
            }
        }))
    }

    /// The Setup's answer once it has come.
    pub fn setup_answer(&self, _: &Thread) -> Result<Option<Result<(), Refusal>>, Failure> {
        Ok(self
            .read::<SetupReply>("the Setup's answer")?
            .map(|reply| if reply.ok == 1 { Ok(()) } else { Err(Refusal::of(&reply.text, reply.ngx_result)) }))
    }

    /// The helper's frame reports that have come, read off the pipe so it
    /// never fills: the last of them.
    pub fn frames_done(&self, _: &Thread) -> Result<Option<Report>, Failure> {
        let mut last = None;
        while let Some(stat) = self.read::<FrameStat>("a frame's report")? {
            let ticks = stat.ts_eval_end.wrapping_sub(stat.ts_eval_begin);
            last = Some(Report { frame: stat.frame, dlss_microseconds: (ticks as f64 * stat.tick_ns / 1000.0) as u32 });
        }
        Ok(last)
    }

    fn between(&self, what: Between) -> Result<(), u32> {
        self.write(&FrameMsg {
            frame: 0,
            wait_value: 0,
            signal_value: 0,
            jitter_x: 0.0,
            jitter_y: 0.0,
            mv_scale_x: 1.0,
            mv_scale_y: 1.0,
            reset: 0,
            quit: what as u32,
        })
    }

    /// A new output size is coming: a Hello and a Setup follow.
    pub fn new_size(&self, _: &Thread) -> Result<(), Failure> {
        self.between(Between::NewSize).map_err(|error| Failure::Pipe("the new size", error))
    }

    /// The helper told to end (it may be gone already).
    pub fn quit(&self, _: &Thread) {
        let _ = self.between(Between::Quit);
    }

    /// A handle of this process given to the helper (DuplicateHandle), ours
    /// closed: its value in the helper's handle table.
    fn give(&self, handle: Handle) -> Result<u64, Failure> {
        let given = self.kernel.duplicate(handle.0.as_ptr(), self.process.0.as_ptr());
        handle.close();
        given.map(|value| value as usize as u64).map_err(|error| Failure::Start("DuplicateHandle", error))
    }

    /// The watch started on a thread of its own (CreateThread, the default
    /// stack), with a handle of its own to the helper's process.
    pub fn watch(&self, _: &Thread, entry: CallbackEntry<OrigThreadEntry>) -> Result<(), Failure> {
        let copy = self
            .kernel
            .duplicate(self.process.0.as_ptr(), this_process())
            .map_err(|error| Failure::Start("DuplicateHandle", error))?;
        // SAFETY: CreateThread (the exe's import) with a generated entry in a
        // thread start routine's convention, alive as long as the exe; its
        // argument the handle, which the watch closes.
        let thread = unsafe { (raw::engine_create_thread)(null_mut(), 0, entry.entry(), copy, 0, null_mut()) };
        match NonNull::new(thread) {
            Some(thread) => {
                Handle(thread).close();
                Ok(())
            }
            None => {
                let error = last_error();
                if let Some(copy) = NonNull::new(copy) {
                    Handle(copy).close();
                }
                Err(Failure::Start("CreateThread", error))
            }
        }
    }

    /// The helper ended now, if it still runs (the watch sees it end).
    pub fn terminate(&self, _: &Thread) {
        // SAFETY: TerminateProcess (the exe's import) on our process handle;
        // a process already gone refuses it.
        unsafe { (raw::engine_terminate_process)(self.process.0.as_ptr(), 1) };
    }

    /// Waits for the helper's end, at most `milliseconds`: whether it ended.
    pub fn wait(&self, _: &Thread, milliseconds: u32) -> bool {
        // SAFETY: WaitForSingleObject (the exe's import) on our process handle.
        unsafe { (raw::engine_wait_for_single_object)(self.process.0.as_ptr(), milliseconds) == 0 }
    }

    /// Our two handles closed: the helper's pipe ends, and with it a helper
    /// that still reads it.
    pub fn close(self, _: &Thread) {
        self.pipe.close();
        self.process.close();
    }
}

/// The helper's process as its watch sees it, on the watch's own thread.
pub struct HelperWatch<'t> {
    process: Option<Handle>,
    thread: &'t Thread,
}

/// What a wait of the watch found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Watched {
    /// The helper still runs.
    Runs,
    /// Its process ended, or cannot be waited for.
    Gone,
}

impl<'t> HelperWatch<'t> {
    /// # Safety
    /// Called by the generated shim of the thread [`Helper::watch`] started:
    /// `raw` is that thread's argument, the watch's own process handle.
    pub unsafe fn from_raw(thread: &'t Thread, raw: *mut c_void) -> HelperWatch<'t> {
        HelperWatch { process: NonNull::new(raw).map(Handle), thread }
    }

    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// Waits for the helper's end, at most `milliseconds`.
    pub fn wait(&self, milliseconds: u32) -> Watched {
        const WAIT_TIMEOUT: u32 = 0x102;
        let Some(process) = self.process else { return Watched::Gone };
        // SAFETY: WaitForSingleObject (the exe's import) on the watch's own handle.
        match unsafe { (raw::engine_wait_for_single_object)(process.0.as_ptr(), milliseconds) } {
            WAIT_TIMEOUT => Watched::Runs,
            _ => Watched::Gone,
        }
    }

    /// The helper ended now (TerminateProcess).
    pub fn terminate(&self) {
        if let Some(process) = self.process {
            // SAFETY: TerminateProcess (the exe's import) on the watch's own handle.
            unsafe { (raw::engine_terminate_process)(process.0.as_ptr(), 1) };
        }
    }

    /// The watch's handle closed: its thread ends after this.
    pub fn close(self) {
        if let Some(process) = self.process {
            process.close();
        }
    }
}

// ---- DXVK's device

/// IDXGIVkInteropDevice1 and IDXGIVkInteropSurface (DXVK 3.1.1's
/// dxgi_interfaces.h), as a GUID lies in memory.
const IID_INTEROP_DEVICE: [u8; 16] =
    [0xa5, 0x5f, 0xef, 0xe2, 0x21, 0xdc, 0xf7, 0x4a, 0x90, 0xc4, 0xf6, 0x7e, 0xf6, 0xa0, 0x93, 0x24];
const IID_INTEROP_SURFACE: [u8; 16] =
    [0x8c, 0xcf, 0x46, 0x55, 0xe7, 0x77, 0x41, 0x43, 0xb0, 0x5d, 0x8d, 0x4d, 0x50, 0x00, 0xe7, 0x7d];

/// IUnknown::Release of a COM object we hold a reference of.
///
/// # Safety
/// `object` is a live COM object and the reference is ours.
unsafe fn com_release(object: u32) {
    // SAFETY: a COM object's first word is its vtable, whose third entry is
    // IUnknown::Release (stdcall, the object its argument).
    unsafe {
        let object = object as usize as *mut c_void;
        let vtable = *object.cast::<*const *mut c_void>();
        if let Some(release) = NonNull::new(*vtable.add(2)) {
            raw::engine_com_release(release, object);
        }
    }
}

/// The game's swap chain, as the render output that presents holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwapChain(raw::SwapChain);

/// A D3D11 texture as DXVK has it: its VkImage, the layout DXVK keeps it in
/// between its own commands, its format and its size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Surface {
    image: u64,
    layout: u32,
    format: u32,
    pub size: Size,
}

impl Surface {
    /// # Safety
    /// `surface` is a live IDXGIVkInteropSurface, one reference of it the
    /// caller's: let go here.
    unsafe fn of(surface: raw::InteropSurface) -> Result<Surface, Failure> {
        let mut info = ImageCreateInfo::zeroed();
        info.s_type = STRUCTURE_IMAGE_CREATE_INFO;
        let (mut image, mut layout) = (0u64, 0u32);
        // SAFETY: GetVulkanImageInfo with room for the three it writes (the
        // create info's type set, no chain, as DXVK asks); then the
        // caller's reference let go.
        let got = unsafe {
            let got = raw::engine_interop_image_info(Some(surface), &mut image, &mut layout, (&raw mut info).cast());
            com_release(surface.address());
            got
        };
        if (got as i32) < 0 || image == 0 {
            return Err(Failure::Device("GetVulkanImageInfo", got));
        }
        Ok(Surface { image, layout, format: info.format, size: Size { width: info.extent[0], height: info.extent[1] } })
    }

    /// A texture of the engine's as DXVK has it now (the engine makes its
    /// targets again at a new size, so each use asks); a failure is
    /// `what`'s.
    fn of_resource(resource: Option<raw::HwResource>, what: &'static str) -> Result<Surface, Failure> {
        let resource = resource.ok_or(Failure::Device(what, 0))?;
        let mut surface = None;
        // SAFETY: IUnknown::QueryInterface, the first entry of the vtable a
        // COM object's first word names (stdcall, the object its first
        // argument), of a live ID3D11Resource of DXVK's: it answers the
        // texture's interop surface, one reference ours.
        let got = unsafe {
            let vtable = *(resource.address() as usize as *const *const *mut c_void);
            match NonNull::new(*vtable) {
                Some(query) => raw::engine_resource_interop_surface(
                    query,
                    Some(resource),
                    IID_INTEROP_SURFACE.as_ptr().cast(),
                    &mut surface,
                ),
                None => 0,
            }
        };
        let Some(surface) = surface.filter(|_| (got as i32) >= 0) else { return Err(Failure::Device(what, got)) };
        // SAFETY: the surface QueryInterface just gave, its reference ours.
        unsafe { Surface::of(surface) }
    }
}

impl SwapChain {
    /// At `render_output.present`: the swap chain the game presents with now.
    pub fn at_present(ctx: &Context<'_>) -> Option<SwapChain> {
        // SAFETY: the context is this site's: ESI the render output present
        // runs on, alive while it runs; its swap chain is initialize_swap_chain's.
        unsafe { raw::site::render_output_present::output(ctx)?.swap_chain() }.map(SwapChain)
    }

    /// Its back buffer as DXVK has it (the output's size is its size); a
    /// failure names the call.
    pub fn back_buffer(self, _: &Thread) -> Result<Surface, Failure> {
        let mut surface = None;
        // SAFETY: IDXGISwapChain::GetBuffer on the game's live swap chain, on
        // the thread that presents with it: DXVK answers its back buffer's
        // interop surface, one reference ours.
        let got = unsafe {
            raw::engine_swap_chain_get_buffer(Some(self.0), 0, IID_INTEROP_SURFACE.as_ptr().cast(), &mut surface)
        };
        let Some(surface) = surface.filter(|_| (got as i32) >= 0) else {
            return Err(Failure::Device("the back buffer's IDXGIVkInteropSurface", got));
        };
        // SAFETY: the surface GetBuffer just gave, its reference ours.
        unsafe { Surface::of(surface) }
    }

    /// DXVK's device behind it; a failure names what is not there.
    fn interop(self) -> Result<raw::InteropDevice, Failure> {
        let mut device = None;
        // SAFETY: IDXGIDeviceSubObject::GetDevice on the game's live swap
        // chain: DXVK answers its device's interop interface, one reference ours.
        let got =
            unsafe { raw::engine_swap_chain_get_device(Some(self.0), IID_INTEROP_DEVICE.as_ptr().cast(), &mut device) };
        device.filter(|_| (got as i32) >= 0).ok_or(Failure::Device("IDXGIVkInteropDevice1", got))
    }
}

/// DXVK's GPU as Vulkan names it across processes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GpuId {
    uuid: [u8; 16],
    luid: [u8; 8],
}

/// The Vulkan functions the link calls, of DXVK's device.
#[derive(Clone, Copy)]
struct Vulkan {
    create_image: Function,
    destroy_image: Function,
    image_memory_requirements: Function,
    allocate_memory: Function,
    free_memory: Function,
    bind_image_memory: Function,
    create_semaphore: Function,
    destroy_semaphore: Function,
    memory_handle: Function,
    semaphore_handle: Function,
    create_command_pool: Function,
    destroy_command_pool: Function,
    allocate_command_buffers: Function,
    begin: Function,
    end: Function,
    barrier: Function,
    blit: Function,
    copy: Function,
    create_image_view: Function,
    destroy_image_view: Function,
    create_shader_module: Function,
    destroy_shader_module: Function,
    create_set_layout: Function,
    destroy_set_layout: Function,
    create_pipeline_layout: Function,
    destroy_pipeline_layout: Function,
    create_pipelines: Function,
    destroy_pipeline: Function,
    create_descriptor_pool: Function,
    destroy_descriptor_pool: Function,
    allocate_sets: Function,
    update_sets: Function,
    bind_pipeline: Function,
    bind_sets: Function,
    push_constants: Function,
    dispatch: Function,
    submit: Function,
    wait_idle: Function,
    signal: Function,
    counter: Function,
}

/// DXVK's Vulkan device, its queue and one reference of its interop
/// interface, let go by [`Device::release`].
pub struct Device {
    interop: raw::InteropDevice,
    device: NonNull<c_void>,
    queue: NonNull<c_void>,
    family: u32,
    vulkan: Vulkan,
    gpu: GpuId,
    /// Each memory type's property flags (0 past the device's count).
    memory_types: [u32; 32],
}
// SAFETY: handles of DXVK's device, alive while the game's device is; only
// the thread that holds the state they are in uses them, one at a time.
unsafe impl Send for Device {}

impl Device {
    /// DXVK's device of the swap chain, the Vulkan functions from the module
    /// DXVK took its own from: `winevulkan.dll` when it is loaded (Wine),
    /// else `vulkan-1.dll`.
    pub fn of(_: &Thread, chain: SwapChain) -> Result<Device, Failure> {
        let interop = chain.interop()?;
        let made = Device::with(interop);
        if made.is_err() {
            // SAFETY: the reference GetDevice gave, used no more.
            unsafe { com_release(interop.address()) };
        }
        made
    }

    fn with(interop: raw::InteropDevice) -> Result<Device, Failure> {
        let missing = |what: &'static str| Failure::Device(what, 0);
        let (mut instance, mut physical, mut device, mut queue) = (null_mut(), null_mut(), null_mut(), null_mut());
        let (mut index, mut family) = (0u32, 0u32);
        // SAFETY: GetVulkanHandles and GetSubmissionQueue1 of DXVK's interop
        // device, each writing the handles it is given room for.
        unsafe {
            raw::engine_interop_vulkan_handles(Some(interop), &mut instance, &mut physical, &mut device);
            raw::engine_interop_submission_queue(Some(interop), &mut queue, &mut index, &mut family);
        }
        let (Some(device), Some(queue)) = (NonNull::new(device), NonNull::new(queue)) else {
            return Err(missing("DXVK's Vulkan handles"));
        };
        if instance.is_null() || physical.is_null() {
            return Err(missing("DXVK's Vulkan handles"));
        }
        // SAFETY: GetModuleHandleA and GetProcAddress (the exe's imports) with
        // NUL-terminated names; DXVK keeps the module loaded.
        let entry = unsafe {
            let wine = (raw::engine_get_module_handle_a)(c"winevulkan.dll".as_ptr().cast());
            let module =
                if wine.is_null() { (raw::engine_get_module_handle_a)(c"vulkan-1.dll".as_ptr().cast()) } else { wine };
            if module.is_null() {
                return Err(missing("winevulkan.dll or vulkan-1.dll"));
            }
            NonNull::new((raw::engine_get_proc_address)(module, c"vkGetInstanceProcAddr".as_ptr().cast()))
        }
        .ok_or(missing("vkGetInstanceProcAddr"))?;
        // SAFETY: vkGetInstanceProcAddr and vkGetDeviceProcAddr with DXVK's
        // live instance and device and NUL-terminated names.
        let of_instance = |name: &'static CStr| unsafe {
            NonNull::new(raw::engine_vk_get_instance_proc_addr(entry, instance, name.as_ptr().cast()))
                .map(Function)
                .ok_or(missing(name.to_str().unwrap_or("a Vulkan function")))
        };
        let device_entry = of_instance(c"vkGetDeviceProcAddr")?;
        let f = |name: &'static CStr| unsafe {
            NonNull::new(raw::engine_vk_get_device_proc_addr(device_entry.0, device.as_ptr(), name.as_ptr().cast()))
                .map(Function)
                .ok_or(missing(name.to_str().unwrap_or("a Vulkan function")))
        };
        let vulkan = Vulkan {
            create_image: f(c"vkCreateImage")?,
            destroy_image: f(c"vkDestroyImage")?,
            image_memory_requirements: f(c"vkGetImageMemoryRequirements")?,
            allocate_memory: f(c"vkAllocateMemory")?,
            free_memory: f(c"vkFreeMemory")?,
            bind_image_memory: f(c"vkBindImageMemory")?,
            create_semaphore: f(c"vkCreateSemaphore")?,
            destroy_semaphore: f(c"vkDestroySemaphore")?,
            // no function: DXVK did not enable the extension on this device
            memory_handle: f(c"vkGetMemoryWin32HandleKHR")?,
            semaphore_handle: f(c"vkGetSemaphoreWin32HandleKHR")?,
            create_command_pool: f(c"vkCreateCommandPool")?,
            destroy_command_pool: f(c"vkDestroyCommandPool")?,
            allocate_command_buffers: f(c"vkAllocateCommandBuffers")?,
            begin: f(c"vkBeginCommandBuffer")?,
            end: f(c"vkEndCommandBuffer")?,
            barrier: f(c"vkCmdPipelineBarrier")?,
            blit: f(c"vkCmdBlitImage")?,
            copy: f(c"vkCmdCopyImage")?,
            create_image_view: f(c"vkCreateImageView")?,
            destroy_image_view: f(c"vkDestroyImageView")?,
            create_shader_module: f(c"vkCreateShaderModule")?,
            destroy_shader_module: f(c"vkDestroyShaderModule")?,
            create_set_layout: f(c"vkCreateDescriptorSetLayout")?,
            destroy_set_layout: f(c"vkDestroyDescriptorSetLayout")?,
            create_pipeline_layout: f(c"vkCreatePipelineLayout")?,
            destroy_pipeline_layout: f(c"vkDestroyPipelineLayout")?,
            create_pipelines: f(c"vkCreateComputePipelines")?,
            destroy_pipeline: f(c"vkDestroyPipeline")?,
            create_descriptor_pool: f(c"vkCreateDescriptorPool")?,
            destroy_descriptor_pool: f(c"vkDestroyDescriptorPool")?,
            allocate_sets: f(c"vkAllocateDescriptorSets")?,
            update_sets: f(c"vkUpdateDescriptorSets")?,
            bind_pipeline: f(c"vkCmdBindPipeline")?,
            bind_sets: f(c"vkCmdBindDescriptorSets")?,
            push_constants: f(c"vkCmdPushConstants")?,
            dispatch: f(c"vkCmdDispatch")?,
            submit: f(c"vkQueueSubmit")?,
            wait_idle: f(c"vkQueueWaitIdle")?,
            signal: f(c"vkSignalSemaphore")?,
            counter: f(c"vkGetSemaphoreCounterValue")?,
        };
        let properties = of_instance(c"vkGetPhysicalDeviceProperties2")?;
        let memory = of_instance(c"vkGetPhysicalDeviceMemoryProperties")?;
        let mut id = IdProperties {
            s_type: STRUCTURE_ID_PROPERTIES,
            next: null_mut(),
            device_uuid: [0; 16],
            driver_uuid: [0; 16],
            device_luid: [0; 8],
            node_mask: 0,
            luid_valid: 0,
        };
        let mut all = Properties2 { s_type: STRUCTURE_PROPERTIES_2, next: (&raw mut id).cast(), properties: [0; 206] };
        let mut types = MemoryProperties([0; 130]);
        // SAFETY: the two queries with DXVK's physical device and room for
        // what each writes (Win32's layouts, below).
        unsafe {
            raw::engine_vk_get_physical_device_properties2(properties.0, physical, (&raw mut all).cast());
            raw::engine_vk_get_physical_device_memory_properties(memory.0, physical, (&raw mut types).cast());
        }
        let mut memory_types = [0; 32];
        for (i, flags) in memory_types.iter_mut().enumerate().take((types.0[0] as usize).min(32)) {
            *flags = types.0[1 + 2 * i];
        }
        Ok(Device {
            interop,
            device,
            queue,
            family,
            vulkan,
            gpu: GpuId { uuid: id.device_uuid, luid: id.device_luid },
            memory_types,
        })
    }

    /// The reference of DXVK's interop device let go.
    pub fn release(self, _: &Thread) {
        // SAFETY: the reference GetDevice gave, used no more.
        unsafe { com_release(self.interop.address()) };
    }

    /// The first memory type of `allowed` that is on the GPU.
    fn device_local(&self, allowed: u32) -> Option<u32> {
        const DEVICE_LOCAL: u32 = 1;
        (0..32).find(|&i| allowed & (1 << i) != 0 && self.memory_types[i as usize] & DEVICE_LOCAL != 0)
    }

    /// The timeline's value now.
    fn reached(&self, timeline: u64) -> Result<u64, Failure> {
        let mut value = 0;
        // SAFETY: vkGetSemaphoreCounterValue on a timeline semaphore of this device.
        let got = unsafe {
            raw::engine_vk_get_semaphore_counter_value(
                self.vulkan.counter.0,
                self.device.as_ptr(),
                low(timeline),
                high(timeline),
                &mut value,
            )
        };
        if got == 0 { Ok(value) } else { Err(Failure::Device("vkGetSemaphoreCounterValue", got)) }
    }
}

/// A 64-bit Vulkan handle or size as stdcall passes it by value: two stack
/// words, the low one first.
fn low(value: u64) -> u32 {
    value as u32
}

fn high(value: u64) -> u32 {
    (value >> 32) as u32
}

// ---- Vulkan's structures as Win32's 32-bit ABI lays them out (vulkan_core.h
// 1.4): a u64 lies at a multiple of 8 there, and a structure holding one is a
// multiple of 8 long; the sizes are checked below.

const STRUCTURE_SUBMIT_INFO: u32 = 4;
const STRUCTURE_MEMORY_ALLOCATE_INFO: u32 = 5;
const STRUCTURE_SEMAPHORE_CREATE_INFO: u32 = 9;
const STRUCTURE_IMAGE_CREATE_INFO: u32 = 14;
const STRUCTURE_IMAGE_VIEW_CREATE_INFO: u32 = 15;
const STRUCTURE_SHADER_MODULE_CREATE_INFO: u32 = 16;
const STRUCTURE_PIPELINE_SHADER_STAGE_CREATE_INFO: u32 = 18;
const STRUCTURE_COMPUTE_PIPELINE_CREATE_INFO: u32 = 29;
const STRUCTURE_PIPELINE_LAYOUT_CREATE_INFO: u32 = 30;
const STRUCTURE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO: u32 = 32;
const STRUCTURE_DESCRIPTOR_POOL_CREATE_INFO: u32 = 33;
const STRUCTURE_DESCRIPTOR_SET_ALLOCATE_INFO: u32 = 34;
const STRUCTURE_WRITE_DESCRIPTOR_SET: u32 = 35;
const STRUCTURE_COMMAND_POOL_CREATE_INFO: u32 = 39;
const STRUCTURE_COMMAND_BUFFER_ALLOCATE_INFO: u32 = 40;
const STRUCTURE_COMMAND_BUFFER_BEGIN_INFO: u32 = 42;
const STRUCTURE_IMAGE_MEMORY_BARRIER: u32 = 45;
const STRUCTURE_MEMORY_BARRIER: u32 = 46;
/// A structure type an extension added (vulkan_core.h's numbering:
/// 1000000000 + 1000 * (the extension's number - 1) + its own).
const fn of_extension(number: u32, own: u32) -> u32 {
    1000 * 1000 * 1000 + 1000 * (number - 1) + own
}
const STRUCTURE_PROPERTIES_2: u32 = of_extension(60, 1);
const STRUCTURE_ID_PROPERTIES: u32 = of_extension(72, 4);
const STRUCTURE_EXTERNAL_MEMORY_IMAGE_CREATE_INFO: u32 = of_extension(73, 1);
const STRUCTURE_EXPORT_MEMORY_ALLOCATE_INFO: u32 = of_extension(73, 2);
const STRUCTURE_MEMORY_GET_WIN32_HANDLE_INFO: u32 = of_extension(74, 3);
const STRUCTURE_EXPORT_SEMAPHORE_CREATE_INFO: u32 = of_extension(78, 0);
const STRUCTURE_SEMAPHORE_GET_WIN32_HANDLE_INFO: u32 = of_extension(79, 3);
const STRUCTURE_MEMORY_DEDICATED_ALLOCATE_INFO: u32 = of_extension(128, 1);
const STRUCTURE_SEMAPHORE_TYPE_CREATE_INFO: u32 = of_extension(208, 2);
const STRUCTURE_TIMELINE_SEMAPHORE_SUBMIT_INFO: u32 = of_extension(208, 3);
const STRUCTURE_SEMAPHORE_SIGNAL_INFO: u32 = of_extension(208, 5);

/// VK_EXTERNAL_MEMORY_HANDLE_TYPE_OPAQUE_WIN32_BIT, and the semaphores' same bit.
const OPAQUE_WIN32: u32 = 2;
const FORMAT_R16G16_SFLOAT: u32 = 83;
const FORMAT_R16G16B16A16_SFLOAT: u32 = 97;
const USAGE_TRANSFER_SRC: u32 = 1;
const USAGE_TRANSFER_DST: u32 = 2;
const USAGE_SAMPLED: u32 = 4;
const USAGE_STORAGE: u32 = 8;
const LAYOUT_UNDEFINED: u32 = 0;
const LAYOUT_GENERAL: u32 = 1;
const LAYOUT_TRANSFER_SRC: u32 = 6;
const LAYOUT_TRANSFER_DST: u32 = 7;
const ACCESS_SHADER_READ: u32 = 0x20;
const ACCESS_SHADER_WRITE: u32 = 0x40;
const ACCESS_TRANSFER_READ: u32 = 0x800;
const ACCESS_TRANSFER_WRITE: u32 = 0x1000;
/// VK_ACCESS_MEMORY_READ_BIT | VK_ACCESS_MEMORY_WRITE_BIT: whatever DXVK did
/// with an image before, or does after.
const ACCESS_ANY: u32 = 1 << 15 | 1 << 16;
const STAGE_COMPUTE: u32 = 0x800;
const STAGE_TRANSFER: u32 = 0x1000;
/// VK_SHADER_STAGE_COMPUTE_BIT and VK_PIPELINE_BIND_POINT_COMPUTE.
const SHADER_COMPUTE: u32 = 0x20;
const BIND_COMPUTE: u32 = 1;
const DESCRIPTOR_SAMPLED_IMAGE: u32 = 2;
const DESCRIPTOR_STORAGE_IMAGE: u32 = 3;
const STAGE_ALL_COMMANDS: u32 = 1 << 16;
const ASPECT_COLOR: u32 = 1;
const ASPECT_DEPTH: u32 = 2;
const ASPECT_STENCIL: u32 = 4;

/// The aspects of an image of a depth format (VK_FORMAT_D16_UNORM 124 to
/// D32_SFLOAT_S8_UINT 130, without the stencil-only 127), which a barrier
/// names whole; none for any other format.
fn depth_aspects(format: u32) -> Option<u32> {
    match format {
        124..=126 => Some(ASPECT_DEPTH),
        128..=130 => Some(ASPECT_DEPTH | ASPECT_STENCIL),
        _ => None,
    }
}
const FAMILY_IGNORED: u32 = !0;
/// VK_QUEUE_FAMILY_EXTERNAL: the other process's device.
const FAMILY_EXTERNAL: u32 = !1;

#[repr(C)]
struct IdProperties {
    s_type: u32,
    next: *mut c_void,
    device_uuid: [u8; 16],
    driver_uuid: [u8; 16],
    device_luid: [u8; 8],
    node_mask: u32,
    luid_valid: u32,
}

/// VkPhysicalDeviceProperties2: the type, the chain, then
/// VkPhysicalDeviceProperties (824 bytes, 8-byte aligned).
#[repr(C, align(8))]
struct Properties2 {
    s_type: u32,
    next: *mut c_void,
    properties: [u32; 206],
}

/// VkPhysicalDeviceMemoryProperties (520 bytes): the type count, 32 types
/// of {property flags, heap}, then the heaps.
#[repr(C, align(8))]
struct MemoryProperties([u32; 130]);

#[repr(C)]
#[derive(Clone, Copy)]
struct ImageCreateInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
    image_type: u32,
    format: u32,
    extent: [u32; 3],
    mip_levels: u32,
    array_layers: u32,
    samples: u32,
    tiling: u32,
    usage: u32,
    sharing_mode: u32,
    family_count: u32,
    families: *const u32,
    initial_layout: u32,
}

impl ImageCreateInfo {
    fn zeroed() -> ImageCreateInfo {
        ImageCreateInfo {
            s_type: 0,
            next: null(),
            flags: 0,
            image_type: 0,
            format: 0,
            extent: [0; 3],
            mip_levels: 0,
            array_layers: 0,
            samples: 0,
            tiling: 0,
            usage: 0,
            sharing_mode: 0,
            family_count: 0,
            families: null(),
            initial_layout: 0,
        }
    }
}

/// VkExternalMemoryImageCreateInfo, VkExportMemoryAllocateInfo and
/// VkExportSemaphoreCreateInfo: the type, the chain, the handle types.
#[repr(C)]
struct HandleTypes {
    s_type: u32,
    next: *const c_void,
    handle_types: u32,
}

#[repr(C, align(8))]
struct MemoryRequirements {
    size: u64,
    alignment: u64,
    memory_types: u32,
}

#[repr(C, align(8))]
struct MemoryAllocateInfo {
    s_type: u32,
    next: *const c_void,
    size: u64,
    memory_type: u32,
}

#[repr(C, align(8))]
struct DedicatedAllocateInfo {
    s_type: u32,
    next: *const c_void,
    image: u64,
    buffer: u64,
}

/// VkMemoryGetWin32HandleInfoKHR and VkSemaphoreGetWin32HandleInfoKHR.
#[repr(C, align(8))]
struct HandleInfo {
    s_type: u32,
    next: *const c_void,
    object: u64,
    handle_type: u32,
}

#[repr(C)]
struct SemaphoreCreateInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
}

#[repr(C, align(8))]
struct SemaphoreTypeCreateInfo {
    s_type: u32,
    next: *const c_void,
    semaphore_type: u32,
    pad: u32,
    initial_value: u64,
}

#[repr(C)]
struct CommandPoolCreateInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
    family: u32,
}

#[repr(C, align(8))]
struct CommandBufferAllocateInfo {
    s_type: u32,
    next: *const c_void,
    pool: u64,
    level: u32,
    count: u32,
}

#[repr(C)]
struct CommandBufferBeginInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
    inheritance: *const c_void,
}

/// VkImageSubresourceRange.
#[repr(C)]
#[derive(Clone, Copy)]
struct Range {
    aspect: u32,
    base_mip: u32,
    mips: u32,
    base_layer: u32,
    layers: u32,
}

impl Range {
    fn of(aspect: u32) -> Range {
        Range { aspect, base_mip: 0, mips: 1, base_layer: 0, layers: 1 }
    }
}

#[repr(C, align(8))]
#[derive(Clone, Copy)]
struct ImageBarrier {
    s_type: u32,
    next: *const c_void,
    source_access: u32,
    destination_access: u32,
    old_layout: u32,
    new_layout: u32,
    source_family: u32,
    destination_family: u32,
    image: u64,
    range: Range,
}

/// VkImageSubresourceLayers.
#[repr(C)]
#[derive(Clone, Copy)]
struct Layers {
    aspect: u32,
    mip: u32,
    base_layer: u32,
    layers: u32,
}

/// VkImageBlit: each image's layers and the two corners of its box.
#[repr(C)]
struct Blit {
    source: Layers,
    source_box: [[i32; 3]; 2],
    destination: Layers,
    destination_box: [[i32; 3]; 2],
}

/// VkMemoryBarrier.
#[repr(C)]
struct MemoryBarrier {
    s_type: u32,
    next: *const c_void,
    source_access: u32,
    destination_access: u32,
}

#[repr(C, align(8))]
struct ImageViewCreateInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
    pad: u32,
    image: u64,
    view_type: u32,
    format: u32,
    components: [u32; 4],
    range: Range,
}

#[repr(C)]
struct ShaderModuleCreateInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
    code_size: u32,
    code: *const u32,
}

/// VkDescriptorSetLayoutBinding.
#[repr(C)]
struct SetLayoutBinding {
    binding: u32,
    kind: u32,
    count: u32,
    stages: u32,
    samplers: *const u64,
}

/// VkDescriptorSetLayoutCreateInfo.
#[repr(C)]
struct SetLayoutCreateInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
    count: u32,
    bindings: *const SetLayoutBinding,
}

#[repr(C)]
struct PushConstantRange {
    stages: u32,
    offset: u32,
    size: u32,
}

#[repr(C)]
struct PipelineLayoutCreateInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
    set_layout_count: u32,
    set_layouts: *const u64,
    range_count: u32,
    ranges: *const PushConstantRange,
}

/// VkPipelineShaderStageCreateInfo.
#[repr(C, align(8))]
struct ShaderStageCreateInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
    stage: u32,
    module: u64,
    name: *const u8,
    specialization: *const c_void,
}

#[repr(C, align(8))]
struct ComputePipelineCreateInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
    pad: u32,
    stage: ShaderStageCreateInfo,
    layout: u64,
    base: u64,
    base_index: i32,
}

#[repr(C)]
struct DescriptorPoolSize {
    kind: u32,
    count: u32,
}

#[repr(C)]
struct DescriptorPoolCreateInfo {
    s_type: u32,
    next: *const c_void,
    flags: u32,
    max_sets: u32,
    size_count: u32,
    sizes: *const DescriptorPoolSize,
}

#[repr(C, align(8))]
struct DescriptorSetAllocateInfo {
    s_type: u32,
    next: *const c_void,
    pool: u64,
    count: u32,
    layouts: *const u64,
}

#[repr(C, align(8))]
struct DescriptorImageInfo {
    sampler: u64,
    view: u64,
    layout: u32,
}

#[repr(C, align(8))]
struct WriteDescriptorSet {
    s_type: u32,
    next: *const c_void,
    set: u64,
    binding: u32,
    element: u32,
    count: u32,
    kind: u32,
    image: *const DescriptorImageInfo,
    buffer: *const c_void,
    texel_view: *const c_void,
}

/// VkImageCopy: each image's layers and corner, and the box copied.
#[repr(C)]
struct ImageCopy {
    source: Layers,
    source_at: [i32; 3],
    destination: Layers,
    destination_at: [i32; 3],
    extent: [u32; 3],
}

#[repr(C)]
struct SubmitInfo {
    s_type: u32,
    next: *const c_void,
    wait_count: u32,
    waits: *const u64,
    wait_stages: *const u32,
    buffer_count: u32,
    buffers: *const *mut c_void,
    signal_count: u32,
    signals: *const u64,
}

#[repr(C)]
struct TimelineSubmitInfo {
    s_type: u32,
    next: *const c_void,
    wait_count: u32,
    waits: *const u64,
    signal_count: u32,
    signals: *const u64,
}

#[repr(C, align(8))]
struct SemaphoreSignalInfo {
    s_type: u32,
    next: *const c_void,
    semaphore: u64,
    value: u64,
}

const _: () = assert!(size_of::<IdProperties>() == 56);
const _: () = assert!(size_of::<Properties2>() == 832 && core::mem::offset_of!(Properties2, properties) == 8);
const _: () = assert!(size_of::<MemoryProperties>() == 520);
const _: () = assert!(size_of::<ImageCreateInfo>() == 68);
const _: () = assert!(size_of::<MemoryRequirements>() == 24);
const _: () = assert!(size_of::<MemoryAllocateInfo>() == 24 && core::mem::offset_of!(MemoryAllocateInfo, size) == 8);
const _: () = assert!(size_of::<DedicatedAllocateInfo>() == 24);
const _: () = assert!(size_of::<HandleInfo>() == 24 && core::mem::offset_of!(HandleInfo, object) == 8);
const _: () = assert!(
    size_of::<SemaphoreTypeCreateInfo>() == 24 && core::mem::offset_of!(SemaphoreTypeCreateInfo, initial_value) == 16
);
const _: () = assert!(size_of::<CommandBufferAllocateInfo>() == 24);
const _: () = assert!(size_of::<ImageBarrier>() == 64 && core::mem::offset_of!(ImageBarrier, image) == 32);
const _: () = assert!(size_of::<Blit>() == 80 && size_of::<ImageCopy>() == 68);
const _: () = assert!(size_of::<MemoryBarrier>() == 16 && size_of::<ShaderModuleCreateInfo>() == 20);
const _: () = assert!(size_of::<ImageViewCreateInfo>() == 72 && core::mem::offset_of!(ImageViewCreateInfo, image) == 16);
const _: () = assert!(size_of::<SetLayoutBinding>() == 20 && size_of::<SetLayoutCreateInfo>() == 20);
const _: () = assert!(size_of::<PushConstantRange>() == 12 && size_of::<PipelineLayoutCreateInfo>() == 28);
const _: () =
    assert!(size_of::<ShaderStageCreateInfo>() == 32 && core::mem::offset_of!(ShaderStageCreateInfo, module) == 16);
const _: () = assert!(
    size_of::<ComputePipelineCreateInfo>() == 72
        && core::mem::offset_of!(ComputePipelineCreateInfo, stage) == 16
        && core::mem::offset_of!(ComputePipelineCreateInfo, layout) == 48
);
const _: () = assert!(size_of::<DescriptorPoolSize>() == 8 && size_of::<DescriptorPoolCreateInfo>() == 24);
const _: () =
    assert!(size_of::<DescriptorSetAllocateInfo>() == 24 && core::mem::offset_of!(DescriptorSetAllocateInfo, pool) == 8);
const _: () = assert!(size_of::<DescriptorImageInfo>() == 24);
const _: () = assert!(size_of::<WriteDescriptorSet>() == 48 && core::mem::offset_of!(WriteDescriptorSet, set) == 8);
const _: () = assert!(size_of::<SubmitInfo>() == 36 && size_of::<TimelineSubmitInfo>() == 24);
const _: () =
    assert!(size_of::<SemaphoreSignalInfo>() == 24 && core::mem::offset_of!(SemaphoreSignalInfo, value) == 16);

// ---- what is shared

/// One shared image and its own memory.
#[derive(Clone, Copy)]
struct Image {
    image: u64,
    memory: u64,
    bytes: u64,
    memory_type: u32,
    format: u32,
    size: Size,
    usage: u32,
}

/// The shared images, in the Setup's order.
const COLOR: usize = 0;
const DEPTH: usize = 1;
const MOTION: usize = 2;
const OUTPUT: usize = 3;

/// The four images and the timeline semaphore the game and the helper
/// share, made and exported on DXVK's device, the frames' command buffers,
/// and the texture the screen image shows. Its one owner retires it
/// ([`Shared::retire`]). A set is one Setup's: a new size is a new set, its
/// own timeline semaphore from 0 (the helper imports each Setup's).
pub struct Shared {
    images: [Image; 4],
    timeline: u64,
    pool: u64,
    /// The copy-in buffers, then the buffers that wait for the helper's
    /// output and copy it, a frame's two [`SLOTS`] apart.
    buffers: [*mut c_void; 2 * SLOTS as usize],
    /// The engine's texture of the output's size the helper's output is
    /// copied into: what the screen image's quad samples in place of the
    /// frame's picture. The render thread's.
    screen: ThreadBound<ColourTarget>,
    motion: MotionPass,
}
// SAFETY: as Device's.
unsafe impl Send for Shared {}

/// A VkResult that is no success, as a failure of `what`.
fn check(what: &'static str, result: u32) -> Result<(), Failure> {
    if result == 0 { Ok(()) } else { Err(Failure::Device(what, result)) }
}

impl Shared {
    /// The screen's texture made at the output's size; the images (colour,
    /// depth and motion at the frame's render size, which is its
    /// depth-stencil's; the output at the output's), the timeline and the
    /// command buffers made, their handles given to the helper and the
    /// Setup sent. Colour and output take the screen texture's format (the
    /// picture's own: both are made as `$user$present` is), depth the
    /// depth-stencil's own, so its depth is copied as it is. A failure
    /// leaves nothing made.
    pub fn make(
        thread: &Thread,
        device: &Device,
        helper: &Helper,
        output: Size,
        depth: &DepthTexture,
    ) -> Result<Shared, Failure> {
        let depth = Surface::of_resource(depth.resource(), "the depth-stencil's IDXGIVkInteropSurface")?;
        if depth_aspects(depth.format).is_none() {
            return Err(Failure::Device("the depth-stencil's Vulkan format", depth.format));
        }
        let screen =
            ColourTarget::make(thread, output).ok_or(Failure::Device("the screen image's own texture", 0))?;
        let colour = Surface::of_resource(
            screen.texture().and_then(|texture| texture.resource()),
            "the screen texture's IDXGIVkInteropSurface",
        )?;
        let inputs = USAGE_TRANSFER_DST | USAGE_SAMPLED;
        let none = Image { image: 0, memory: 0, bytes: 0, memory_type: 0, format: 0, size: depth.size, usage: inputs };
        let mut shared = Shared {
            images: [
                Image { format: colour.format, ..none },
                Image { format: depth.format, ..none },
                Image { format: FORMAT_R16G16_SFLOAT, usage: USAGE_STORAGE | USAGE_SAMPLED, ..none },
                Image {
                    format: colour.format,
                    size: output,
                    usage: USAGE_STORAGE | USAGE_SAMPLED | USAGE_TRANSFER_SRC,
                    ..none
                },
            ],
            timeline: 0,
            pool: 0,
            buffers: [null_mut(); 2 * SLOTS as usize],
            screen: ThreadBound::new(thread, screen),
            motion: MotionPass::NONE,
        };
        match shared.fill(device, helper) {
            Ok(()) => Ok(shared),
            Err(failure) => {
                shared.destroy(device);
                Err(failure)
            }
        }
    }

    fn fill(&mut self, device: &Device, helper: &Helper) -> Result<(), Failure> {
        let (vk, dev) = (&device.vulkan, device.device.as_ptr());
        let mut setup = SetupMsg {
            img: [SharedImageMsg {
                handle: 0,
                alloc_size: 0,
                memory_type: 0,
                format: 0,
                width: 0,
                height: 0,
                usage: 0,
                pad: 0,
            }; 4],
            semaphore: 0,
            dlss_flags: DLSS_FLAGS,
            pad: 0,
        };
        for (image, message) in self.images.iter_mut().zip(&mut setup.img) {
            let external = HandleTypes {
                s_type: STRUCTURE_EXTERNAL_MEMORY_IMAGE_CREATE_INFO,
                next: null(),
                handle_types: OPAQUE_WIN32,
            };
            let info = ImageCreateInfo {
                s_type: STRUCTURE_IMAGE_CREATE_INFO,
                next: (&raw const external).cast(),
                // a 2D image, one mip, one layer, one sample, optimal tiling, one queue family's
                image_type: 1,
                format: image.format,
                extent: [image.size.width, image.size.height, 1],
                mip_levels: 1,
                array_layers: 1,
                samples: 1,
                usage: image.usage,
                initial_layout: LAYOUT_UNDEFINED,
                ..ImageCreateInfo::zeroed()
            };
            let mut needs = MemoryRequirements { size: 0, alignment: 0, memory_types: 0 };
            // SAFETY: vkCreateImage and vkGetImageMemoryRequirements on
            // DXVK's device with vulkan_core.h's structures.
            unsafe {
                check(
                    "vkCreateImage",
                    raw::engine_vk_create_image(
                        vk.create_image.0,
                        dev,
                        (&raw const info).cast(),
                        null(),
                        &mut image.image,
                    ),
                )?;
                raw::engine_vk_get_image_memory_requirements(
                    vk.image_memory_requirements.0,
                    dev,
                    low(image.image),
                    high(image.image),
                    (&raw mut needs).cast(),
                );
            }
            image.bytes = needs.size;
            image.memory_type = device
                .device_local(needs.memory_types)
                .ok_or(Failure::Device("a GPU memory type", needs.memory_types))?;
            let dedicated = DedicatedAllocateInfo {
                s_type: STRUCTURE_MEMORY_DEDICATED_ALLOCATE_INFO,
                next: null(),
                image: image.image,
                buffer: 0,
            };
            let export = HandleTypes {
                s_type: STRUCTURE_EXPORT_MEMORY_ALLOCATE_INFO,
                next: (&raw const dedicated).cast(),
                handle_types: OPAQUE_WIN32,
            };
            let allocate = MemoryAllocateInfo {
                s_type: STRUCTURE_MEMORY_ALLOCATE_INFO,
                next: (&raw const export).cast(),
                size: needs.size,
                memory_type: image.memory_type,
            };
            let ask = |memory| HandleInfo {
                s_type: STRUCTURE_MEMORY_GET_WIN32_HANDLE_INFO,
                next: null(),
                object: memory,
                handle_type: OPAQUE_WIN32,
            };
            let mut handle = null_mut();
            // SAFETY: the image's own exportable memory allocated and bound,
            // and its NT handle asked for, on DXVK's device.
            unsafe {
                check(
                    "vkAllocateMemory",
                    raw::engine_vk_allocate_memory(
                        vk.allocate_memory.0,
                        dev,
                        (&raw const allocate).cast(),
                        null(),
                        &mut image.memory,
                    ),
                )?;
                check(
                    "vkBindImageMemory",
                    raw::engine_vk_bind_image_memory(
                        vk.bind_image_memory.0,
                        dev,
                        low(image.image),
                        high(image.image),
                        low(image.memory),
                        high(image.memory),
                        0,
                        0,
                    ),
                )?;
                let ask = ask(image.memory);
                check(
                    "vkGetMemoryWin32HandleKHR",
                    raw::engine_vk_get_memory_win32_handle(
                        vk.memory_handle.0,
                        dev,
                        (&raw const ask).cast(),
                        &mut handle,
                    ),
                )?;
            }
            let handle = NonNull::new(handle).map(Handle).ok_or(Failure::Device("vkGetMemoryWin32HandleKHR", 0))?;
            *message = SharedImageMsg {
                handle: helper.give(handle)?,
                alloc_size: image.bytes,
                memory_type: image.memory_type,
                format: image.format,
                width: image.size.width,
                height: image.size.height,
                usage: image.usage,
                pad: 0,
            };
        }
        let export =
            HandleTypes { s_type: STRUCTURE_EXPORT_SEMAPHORE_CREATE_INFO, next: null(), handle_types: OPAQUE_WIN32 };
        // a timeline semaphore (type 1) from 0
        let timeline = SemaphoreTypeCreateInfo {
            s_type: STRUCTURE_SEMAPHORE_TYPE_CREATE_INFO,
            next: (&raw const export).cast(),
            semaphore_type: 1,
            pad: 0,
            initial_value: 0,
        };
        let semaphore = SemaphoreCreateInfo {
            s_type: STRUCTURE_SEMAPHORE_CREATE_INFO,
            next: (&raw const timeline).cast(),
            flags: 0,
        };
        // command buffers that are recorded again (RESET_COMMAND_BUFFER), of DXVK's queue family
        let pool = CommandPoolCreateInfo {
            s_type: STRUCTURE_COMMAND_POOL_CREATE_INFO,
            next: null(),
            flags: 2,
            family: device.family,
        };
        let mut handle = null_mut();
        // SAFETY: the exportable timeline semaphore, its NT handle, the
        // command pool and its primary command buffers, on DXVK's device.
        unsafe {
            check(
                "vkCreateSemaphore",
                raw::engine_vk_create_semaphore(
                    vk.create_semaphore.0,
                    dev,
                    (&raw const semaphore).cast(),
                    null(),
                    &mut self.timeline,
                ),
            )?;
            let ask = HandleInfo {
                s_type: STRUCTURE_SEMAPHORE_GET_WIN32_HANDLE_INFO,
                next: null(),
                object: self.timeline,
                handle_type: OPAQUE_WIN32,
            };
            check(
                "vkGetSemaphoreWin32HandleKHR",
                raw::engine_vk_get_semaphore_win32_handle(
                    vk.semaphore_handle.0,
                    dev,
                    (&raw const ask).cast(),
                    &mut handle,
                ),
            )?;
            check(
                "vkCreateCommandPool",
                raw::engine_vk_create_command_pool(
                    vk.create_command_pool.0,
                    dev,
                    (&raw const pool).cast(),
                    null(),
                    &mut self.pool,
                ),
            )?;
            let buffers = CommandBufferAllocateInfo {
                s_type: STRUCTURE_COMMAND_BUFFER_ALLOCATE_INFO,
                next: null(),
                pool: self.pool,
                level: 0,
                count: 2 * SLOTS as u32,
            };
            check(
                "vkAllocateCommandBuffers",
                raw::engine_vk_allocate_command_buffers(
                    vk.allocate_command_buffers.0,
                    dev,
                    (&raw const buffers).cast(),
                    self.buffers.as_mut_ptr(),
                ),
            )?;
        }
        let handle = NonNull::new(handle).map(Handle).ok_or(Failure::Device("vkGetSemaphoreWin32HandleKHR", 0))?;
        setup.semaphore = helper.give(handle)?;
        self.motion.fill(device, &self.images[DEPTH], &self.images[MOTION])?;
        helper.write(&setup).map_err(|error| Failure::Pipe("the Setup", error))
    }

    /// The render size and the output's.
    pub fn sizes(&self) -> (Size, Size) {
        (self.images[COLOR].size, self.images[OUTPUT].size)
    }

    /// The bytes of the four images.
    pub fn bytes(&self) -> u64 {
        self.images.iter().map(|image| image.bytes).sum()
    }

    /// The timeline's value now.
    pub fn reached(&self, _: &Thread, device: &Device) -> Result<u64, Failure> {
        device.reached(self.timeline)
    }

    /// A frame's part of the link, where the screen image is next: the
    /// helper told the frame, then, on DXVK's queue (locked for it), two
    /// submits. The copy-in: the frame's picture and the depth of its
    /// depth-stencil copied into the shared colour and depth, and the
    /// motion pass ([`MotionPass`]) writing the shared motion from that
    /// depth and the engine's object motion, signalling [`Frame::copied`].
    /// Then, after a wait for
    /// [`Frame::upscaled`], the helper's output copied into the screen's
    /// texture, in a command buffer of its own (a wait without one is left
    /// by the driver for DXVK's next submit, which nothing of ours could
    /// then wait out): DXVK's later commands (the screen image's quad of
    /// that texture, Scaleform, the present) run after it, with no wait on
    /// the CPU. The wait is told to the watch. The frame's two command
    /// buffers must be free ([`Frame::buffers_free`]).
    ///
    /// Answers the texture the screen image shows; none, with nothing told
    /// or queued, when the picture is not this set's (another size: the
    /// window's changed and the Present after this has not seen it).
    pub fn submit(
        &self,
        thread: &Thread,
        device: &Device,
        helper: &Helper,
        picture: &Picture,
        frame: Frame,
        told: &Told,
    ) -> Result<Option<ColourTexture>, Failure> {
        let Some(shown) = self.screen.get(thread).and_then(ColourTarget::texture) else { return Ok(None) };
        // SAFETY: DXVK's rendering commands of the frame submitted first
        // (FlushRenderingCommands, on the thread of the immediate context):
        // the picture and the depth are whole on its queue.
        unsafe { raw::engine_interop_flush(Some(device.interop)) };
        let colour = Surface::of_resource(picture.colour.resource(), "the picture's IDXGIVkInteropSurface")?;
        let depth = Surface::of_resource(picture.depth.resource(), "the depth-stencil's IDXGIVkInteropSurface")?;
        let screen = Surface::of_resource(shown.resource(), "the screen texture's IDXGIVkInteropSurface")?;
        let input = |i: usize| self.images[i];
        let Some(depth_aspects) = depth_aspects(depth.format).filter(|_| depth.format == input(DEPTH).format) else {
            return Ok(None);
        };
        if colour.size != input(COLOR).size || depth.size != input(DEPTH).size {
            return Ok(None);
        }
        // the engine's object motion of this frame, where the frame has one
        let objects = match told.motion.and_then(|motion| motion.objects) {
            Some(objects) => Some(Surface::of_resource(objects.resource(), "the object motion's IDXGIVkInteropSurface")?),
            None => None,
        };
        let first = frame.0 == 0;
        helper
            .write(&FrameMsg {
                frame: frame.0,
                wait_value: frame.copied(),
                signal_value: frame.upscaled(),
                jitter_x: told.jitter[0],
                jitter_y: told.jitter[1],
                mv_scale_x: 1.0,
                mv_scale_y: 1.0,
                reset: u32::from(first || told.reset),
                quit: 0,
            })
            .map_err(|error| Failure::Pipe("a frame", error))?;
        let vk = &device.vulkan;
        let buffer = self.buffers[(frame.0 % SLOTS) as usize];
        let waiter = self.buffers[(SLOTS + frame.0 % SLOTS) as usize];
        let barrier = |image: u64, aspect, access: (u32, u32), layouts: (u32, u32), families: (u32, u32)| ImageBarrier {
            s_type: STRUCTURE_IMAGE_MEMORY_BARRIER,
            next: null(),
            source_access: access.0,
            destination_access: access.1,
            old_layout: layouts.0,
            new_layout: layouts.1,
            source_family: families.0,
            destination_family: families.1,
            image,
            range: Range::of(aspect),
        };
        let stay = (FAMILY_IGNORED, FAMILY_IGNORED);
        // the helper had them since the last frame; the first frame nobody had them
        let (taken, from) =
            if first { (stay, LAYOUT_UNDEFINED) } else { ((FAMILY_EXTERNAL, device.family), LAYOUT_GENERAL) };
        let given = (device.family, FAMILY_EXTERNAL);
        let (read, written) = ((ACCESS_ANY, ACCESS_TRANSFER_READ), (0, ACCESS_TRANSFER_WRITE));
        let (was_read, was_written) = ((ACCESS_TRANSFER_READ, ACCESS_ANY), (ACCESS_TRANSFER_WRITE, 0));
        let ours = (LAYOUT_GENERAL, LAYOUT_GENERAL);
        let pass = &self.motion;
        let (mut before, mut after) = (Barriers::new(), Barriers::new());
        before.push(barrier(colour.image, ASPECT_COLOR, read, (colour.layout, LAYOUT_TRANSFER_SRC), stay));
        before.push(barrier(depth.image, depth_aspects, read, (depth.layout, LAYOUT_TRANSFER_SRC), stay));
        before.push(barrier(input(COLOR).image, ASPECT_COLOR, written, (from, LAYOUT_GENERAL), taken));
        before.push(barrier(input(DEPTH).image, depth_aspects, written, (from, LAYOUT_GENERAL), taken));
        before.push(barrier(input(MOTION).image, ASPECT_COLOR, (0, ACCESS_SHADER_WRITE), (from, LAYOUT_GENERAL), taken));
        // our copy of the object motion is ours alone: only its first layout is to be made
        before.push(barrier(pass.objects.image, ASPECT_COLOR, written, (from, LAYOUT_GENERAL), stay));
        after.push(barrier(colour.image, ASPECT_COLOR, was_read, (LAYOUT_TRANSFER_SRC, colour.layout), stay));
        after.push(barrier(depth.image, depth_aspects, was_read, (LAYOUT_TRANSFER_SRC, depth.layout), stay));
        after.push(barrier(input(COLOR).image, ASPECT_COLOR, was_written, ours, given));
        after.push(barrier(input(DEPTH).image, depth_aspects, was_written, ours, given));
        after.push(barrier(input(MOTION).image, ASPECT_COLOR, (ACCESS_SHADER_WRITE, 0), ours, given));
        if let Some(objects) = &objects {
            before.push(barrier(objects.image, ASPECT_COLOR, read, (objects.layout, LAYOUT_TRANSFER_SRC), stay));
            after.push(barrier(objects.image, ASPECT_COLOR, was_read, (LAYOUT_TRANSFER_SRC, objects.layout), stay));
        }
        if first {
            // the output is touched by the copy-in on the first frame only: made GENERAL and given to the helper
            before.push(barrier(input(OUTPUT).image, ASPECT_COLOR, (0, 0), (LAYOUT_UNDEFINED, LAYOUT_GENERAL), stay));
            after.push(barrier(input(OUTPUT).image, ASPECT_COLOR, (0, 0), ours, given));
        }
        // after the helper's output: taken back for the copy, then the helper's again
        let out_before = [
            barrier(input(OUTPUT).image, ASPECT_COLOR, (0, ACCESS_TRANSFER_READ), ours, (FAMILY_EXTERNAL, device.family)),
            barrier(screen.image, ASPECT_COLOR, (ACCESS_ANY, ACCESS_TRANSFER_WRITE), (screen.layout, LAYOUT_TRANSFER_DST), stay),
        ];
        let out_after = [
            barrier(input(OUTPUT).image, ASPECT_COLOR, (ACCESS_TRANSFER_READ, 0), ours, given),
            barrier(screen.image, ASPECT_COLOR, (ACCESS_TRANSFER_WRITE, ACCESS_ANY), (LAYOUT_TRANSFER_DST, screen.layout), stay),
        ];
        let layers = |aspect| Layers { aspect, mip: 0, base_layer: 0, layers: 1 };
        let corner = |size: Size| [[0, 0, 0], [size.width as i32, size.height as i32, 1]];
        let blit = |from: Size, to: Size| Blit {
            source: layers(ASPECT_COLOR),
            source_box: corner(from),
            destination: layers(ASPECT_COLOR),
            destination_box: corner(to),
        };
        let (colour_blit, objects_blit) =
            (blit(colour.size, input(COLOR).size), objects.map(|objects| blit(objects.size, pass.objects.size)));
        let whole = |aspect, size: Size| ImageCopy {
            source: layers(aspect),
            source_at: [0; 3],
            destination: layers(aspect),
            destination_at: [0; 3],
            extent: [size.width, size.height, 1],
        };
        let (depth_copy, output_copy) = (whole(ASPECT_DEPTH, depth.size), whole(ASPECT_COLOR, screen.size));
        // the copies' writes are whole before the motion pass reads them
        let copied_in = MemoryBarrier {
            s_type: STRUCTURE_MEMORY_BARRIER,
            next: null(),
            source_access: ACCESS_TRANSFER_WRITE,
            destination_access: ACCESS_SHADER_READ | ACCESS_SHADER_WRITE,
        };
        let constants = MotionFrame {
            reproject: told.motion.map_or([0.0; 16], |motion| motion.reproject),
            jitter: told.motion.map_or([0.0; 2], |motion| motion.jitter_clip),
            camera: if told.motion.is_some() { 1.0 } else { 0.0 },
            object_motion: if objects.is_some() { 1.0 } else { 0.0 },
        };
        // recorded once, executed once (ONE_TIME_SUBMIT)
        let begin = CommandBufferBeginInfo {
            s_type: STRUCTURE_COMMAND_BUFFER_BEGIN_INFO,
            next: null(),
            flags: 1,
            inheritance: null(),
        };
        // SAFETY: our two command buffers recorded: they are free (the
        // caller's word), their images are this device's and alive (the
        // engine's until the hook returns, ours until the set is retired),
        // and every barrier gives DXVK's images back in the layout DXVK
        // keeps them in. The motion pass's pipeline, set and views are this
        // set's own.
        unsafe {
            check(
                "vkBeginCommandBuffer",
                raw::engine_vk_begin_command_buffer(vk.begin.0, buffer, (&raw const begin).cast()),
            )?;
            raw::engine_vk_cmd_pipeline_barrier(
                vk.barrier.0,
                buffer,
                STAGE_ALL_COMMANDS,
                STAGE_TRANSFER | STAGE_COMPUTE,
                0,
                0,
                null(),
                0,
                null(),
                before.len,
                before.list.as_ptr().cast(),
            );
            // nearest (0): the picture as it is, both the render size
            raw::engine_vk_cmd_blit_image(
                vk.blit.0,
                buffer,
                low(colour.image),
                high(colour.image),
                LAYOUT_TRANSFER_SRC,
                low(input(COLOR).image),
                high(input(COLOR).image),
                LAYOUT_GENERAL,
                1,
                (&raw const colour_blit).cast(),
                0,
            );
            raw::engine_vk_cmd_copy_image(
                vk.copy.0,
                buffer,
                low(depth.image),
                high(depth.image),
                LAYOUT_TRANSFER_SRC,
                low(input(DEPTH).image),
                high(input(DEPTH).image),
                LAYOUT_GENERAL,
                1,
                (&raw const depth_copy).cast(),
            );
            if let (Some(objects), Some(objects_blit)) = (&objects, &objects_blit) {
                // the engine's object motion into our copy of it, whatever format the engine made it in
                raw::engine_vk_cmd_blit_image(
                    vk.blit.0,
                    buffer,
                    low(objects.image),
                    high(objects.image),
                    LAYOUT_TRANSFER_SRC,
                    low(pass.objects.image),
                    high(pass.objects.image),
                    LAYOUT_GENERAL,
                    1,
                    (&raw const *objects_blit).cast(),
                    0,
                );
            }
            raw::engine_vk_cmd_pipeline_barrier(
                vk.barrier.0,
                buffer,
                STAGE_TRANSFER,
                STAGE_COMPUTE,
                0,
                1,
                (&raw const copied_in).cast(),
                0,
                null(),
                0,
                null(),
            );
            raw::engine_vk_cmd_bind_pipeline(vk.bind_pipeline.0, buffer, BIND_COMPUTE, low(pass.pipeline), high(pass.pipeline));
            raw::engine_vk_cmd_bind_descriptor_sets(
                vk.bind_sets.0,
                buffer,
                BIND_COMPUTE,
                low(pass.layout),
                high(pass.layout),
                0,
                1,
                (&raw const pass.set).cast(),
                0,
                null(),
            );
            raw::engine_vk_cmd_push_constants(
                vk.push_constants.0,
                buffer,
                low(pass.layout),
                high(pass.layout),
                SHADER_COMPUTE,
                0,
                size_of::<MotionFrame>() as u32,
                (&raw const constants).cast(),
            );
            // the shader's groups are 8 x 8 pixels
            let groups = |pixels: u32| pixels.div_ceil(8);
            raw::engine_vk_cmd_dispatch(
                vk.dispatch.0,
                buffer,
                groups(input(MOTION).size.width),
                groups(input(MOTION).size.height),
                1,
            );
            raw::engine_vk_cmd_pipeline_barrier(
                vk.barrier.0,
                buffer,
                STAGE_TRANSFER | STAGE_COMPUTE,
                STAGE_ALL_COMMANDS,
                0,
                0,
                null(),
                0,
                null(),
                after.len,
                after.list.as_ptr().cast(),
            );
            check("vkEndCommandBuffer", raw::engine_vk_end_command_buffer(vk.end.0, buffer))?;
            check(
                "vkBeginCommandBuffer",
                raw::engine_vk_begin_command_buffer(vk.begin.0, waiter, (&raw const begin).cast()),
            )?;
            raw::engine_vk_cmd_pipeline_barrier(
                vk.barrier.0,
                waiter,
                STAGE_ALL_COMMANDS,
                STAGE_TRANSFER,
                0,
                0,
                null(),
                0,
                null(),
                2,
                out_before.as_ptr().cast(),
            );
            raw::engine_vk_cmd_copy_image(
                vk.copy.0,
                waiter,
                low(input(OUTPUT).image),
                high(input(OUTPUT).image),
                LAYOUT_GENERAL,
                low(screen.image),
                high(screen.image),
                LAYOUT_TRANSFER_DST,
                1,
                (&raw const output_copy).cast(),
            );
            raw::engine_vk_cmd_pipeline_barrier(
                vk.barrier.0,
                waiter,
                STAGE_TRANSFER,
                STAGE_ALL_COMMANDS,
                0,
                0,
                null(),
                0,
                null(),
                2,
                out_after.as_ptr().cast(),
            );
            check("vkEndCommandBuffer", raw::engine_vk_end_command_buffer(vk.end.0, waiter))?;
        }
        let (copied, upscaled, timeline, stage) =
            ([frame.copied()], [frame.upscaled()], [self.timeline], [STAGE_ALL_COMMANDS]);
        let signals = TimelineSubmitInfo {
            s_type: STRUCTURE_TIMELINE_SEMAPHORE_SUBMIT_INFO,
            next: null(),
            wait_count: 0,
            waits: null(),
            signal_count: 1,
            signals: copied.as_ptr(),
        };
        let waits = TimelineSubmitInfo {
            s_type: STRUCTURE_TIMELINE_SEMAPHORE_SUBMIT_INFO,
            next: null(),
            wait_count: 1,
            waits: upscaled.as_ptr(),
            signal_count: 0,
            signals: null(),
        };
        let submits = [
            // the copy-in, which signals the frame's inputs
            SubmitInfo {
                s_type: STRUCTURE_SUBMIT_INFO,
                next: (&raw const signals).cast(),
                wait_count: 0,
                waits: null(),
                wait_stages: null(),
                buffer_count: 1,
                buffers: &buffer,
                signal_count: 1,
                signals: timeline.as_ptr(),
            },
            // after the helper's output: its copy to the screen's texture, and DXVK's later commands wait for it
            SubmitInfo {
                s_type: STRUCTURE_SUBMIT_INFO,
                next: (&raw const waits).cast(),
                wait_count: 1,
                waits: timeline.as_ptr(),
                wait_stages: stage.as_ptr(),
                buffer_count: 1,
                buffers: &waiter,
                signal_count: 0,
                signals: null(),
            },
        ];
        // told before it is queued: the watch never misses a wait that is on the queue
        WATCH.with(thread, |watch| {
            *watch = Some(Watch {
                device: device.device,
                signal: vk.signal,
                counter: vk.counter,
                timeline: self.timeline,
                queued: frame.upscaled(),
            })
        });
        // SAFETY: vkQueueSubmit on DXVK's queue while DXVK's submission
        // thread is locked out of it (Lock/ReleaseSubmissionQueue).
        let submitted = unsafe {
            raw::engine_interop_lock_queue(Some(device.interop));
            let submitted =
                raw::engine_vk_queue_submit(vk.submit.0, device.queue.as_ptr(), 2, submits.as_ptr().cast(), 0, 0);
            raw::engine_interop_release_queue(Some(device.interop));
            submitted
        };
        check("vkQueueSubmit", submitted).map(|()| Some(shown))
    }

    /// The end of the set: DXVK's queue waited empty (the helper is gone or
    /// has finished every frame it was told, or the timeline was released,
    /// so no wait of ours stays on it), the watch told there is nothing to
    /// watch, everything destroyed.
    pub fn retire(self, thread: &Thread, device: &Device) {
        // SAFETY: vkQueueWaitIdle on DXVK's queue, locked for it.
        unsafe {
            raw::engine_interop_lock_queue(Some(device.interop));
            raw::engine_vk_queue_wait_idle(device.vulkan.wait_idle.0, device.queue.as_ptr());
            raw::engine_interop_release_queue(Some(device.interop));
        }
        WATCH.with(thread, |watch| {
            if watch.as_ref().is_some_and(|watch| watch.timeline == self.timeline) {
                *watch = None;
            }
        });
        self.destroy(device);
    }

    /// Everything made destroyed (what was not made is 0, which Vulkan
    /// takes); nothing of it is on the queue. The screen's texture goes
    /// with `self`: the engine's reference of ours let go (a frame that
    /// still shows it holds its own).
    fn destroy(self, device: &Device) {
        let (vk, dev) = (&device.vulkan, device.device.as_ptr());
        // SAFETY: each object is this device's, made here, used no more;
        // the pool takes its command buffers with it.
        unsafe {
            raw::engine_vk_destroy_command_pool(
                vk.destroy_command_pool.0,
                dev,
                low(self.pool),
                high(self.pool),
                null(),
            );
            raw::engine_vk_destroy_semaphore(
                vk.destroy_semaphore.0,
                dev,
                low(self.timeline),
                high(self.timeline),
                null(),
            );
        }
        // before the images its views are of
        self.motion.destroy(device);
        // SAFETY: as above.
        unsafe {
            for image in self.images {
                raw::engine_vk_destroy_image(vk.destroy_image.0, dev, low(image.image), high(image.image), null());
                raw::engine_vk_free_memory(vk.free_memory.0, dev, low(image.memory), high(image.memory), null());
            }
        }
    }
}

// ---- the motion pass

include!("../../../../dlss_motion_026g.rs");

/// The motion pass's push constants (dlss_motion_026g.comp's `Frame`).
#[repr(C)]
struct MotionFrame {
    reproject: [f32; 16],
    jitter: [f32; 2],
    camera: f32,
    object_motion: f32,
}

const _: () = assert!(size_of::<MotionFrame>() == 80);

/// A frame's image barriers, as many as it has.
struct Barriers {
    list: [ImageBarrier; 8],
    len: u32,
}

impl Barriers {
    fn new() -> Barriers {
        let none = ImageBarrier {
            s_type: STRUCTURE_IMAGE_MEMORY_BARRIER,
            next: null(),
            source_access: 0,
            destination_access: 0,
            old_layout: 0,
            new_layout: 0,
            source_family: FAMILY_IGNORED,
            destination_family: FAMILY_IGNORED,
            image: 0,
            range: Range::of(ASPECT_COLOR),
        };
        Barriers { list: [none; 8], len: 0 }
    }

    /// One more; past the room there is none is added (a frame has at
    /// most eight).
    fn push(&mut self, barrier: ImageBarrier) {
        if let Some(slot) = self.list.get_mut(self.len as usize) {
            *slot = barrier;
            self.len += 1;
        }
    }
}

/// The motion pass of a set of shared images (dlss_motion_026g.comp): its
/// compute pipeline, the views it reads the shared depth and writes the
/// shared motion through, and our own copy of the engine's object motion
/// with its view (the engine's image is only copied from: a view of ours on
/// it would outlive a target the engine makes again). What is not made is
/// 0, which Vulkan's destroys take.
struct MotionPass {
    module: u64,
    set_layout: u64,
    layout: u64,
    pipeline: u64,
    pool: u64,
    set: u64,
    objects: Image,
    /// Of the shared depth, of our object motion, of the shared motion.
    views: [u64; 3],
}

impl MotionPass {
    const NONE: MotionPass = MotionPass {
        module: 0,
        set_layout: 0,
        layout: 0,
        pipeline: 0,
        pool: 0,
        set: 0,
        objects: Image {
            image: 0,
            memory: 0,
            bytes: 0,
            memory_type: 0,
            format: FORMAT_R16G16B16A16_SFLOAT,
            size: Size { width: 0, height: 0 },
            usage: USAGE_TRANSFER_DST | USAGE_SAMPLED,
        },
        views: [0; 3],
    };

    /// The pass made for a set's shared `depth` and `motion`. A failure
    /// leaves what was made for [`MotionPass::destroy`].
    fn fill(&mut self, device: &Device, depth: &Image, motion: &Image) -> Result<(), Failure> {
        let (vk, dev) = (&device.vulkan, device.device.as_ptr());
        // SAFETY: vkCreate* on DXVK's device with vulkan_core.h's
        // structures, each alive for its call, and room for each handle.
        let create = |function: Function, what: &'static str, info: *const c_void, object: &mut u64| unsafe {
            check(what, raw::engine_vk_create_object(function.0, dev, info, null(), object))
        };
        // our copy of the object motion: an image of the render size, its own memory
        self.objects.size = depth.size;
        let info = ImageCreateInfo {
            s_type: STRUCTURE_IMAGE_CREATE_INFO,
            image_type: 1,
            format: self.objects.format,
            extent: [self.objects.size.width, self.objects.size.height, 1],
            mip_levels: 1,
            array_layers: 1,
            samples: 1,
            usage: self.objects.usage,
            initial_layout: LAYOUT_UNDEFINED,
            ..ImageCreateInfo::zeroed()
        };
        let mut needs = MemoryRequirements { size: 0, alignment: 0, memory_types: 0 };
        // SAFETY: as the shared images', without the export.
        unsafe {
            check(
                "vkCreateImage",
                raw::engine_vk_create_image(vk.create_image.0, dev, (&raw const info).cast(), null(), &mut self.objects.image),
            )?;
            raw::engine_vk_get_image_memory_requirements(
                vk.image_memory_requirements.0,
                dev,
                low(self.objects.image),
                high(self.objects.image),
                (&raw mut needs).cast(),
            );
        }
        self.objects.bytes = needs.size;
        self.objects.memory_type =
            device.device_local(needs.memory_types).ok_or(Failure::Device("a GPU memory type", needs.memory_types))?;
        let allocate = MemoryAllocateInfo {
            s_type: STRUCTURE_MEMORY_ALLOCATE_INFO,
            next: null(),
            size: needs.size,
            memory_type: self.objects.memory_type,
        };
        // SAFETY: the image's own memory allocated and bound.
        unsafe {
            check(
                "vkAllocateMemory",
                raw::engine_vk_allocate_memory(
                    vk.allocate_memory.0,
                    dev,
                    (&raw const allocate).cast(),
                    null(),
                    &mut self.objects.memory,
                ),
            )?;
            check(
                "vkBindImageMemory",
                raw::engine_vk_bind_image_memory(
                    vk.bind_image_memory.0,
                    dev,
                    low(self.objects.image),
                    high(self.objects.image),
                    low(self.objects.memory),
                    high(self.objects.memory),
                    0,
                    0,
                ),
            )?;
        }
        // a 2D view (1) of an image's one mip and layer, its components as they are
        let viewed = [(depth.image, depth.format, ASPECT_DEPTH), (self.objects.image, self.objects.format, ASPECT_COLOR), (
            motion.image,
            motion.format,
            ASPECT_COLOR,
        )];
        for (view, (image, format, aspect)) in self.views.iter_mut().zip(viewed) {
            let info = ImageViewCreateInfo {
                s_type: STRUCTURE_IMAGE_VIEW_CREATE_INFO,
                next: null(),
                flags: 0,
                pad: 0,
                image,
                view_type: 1,
                format,
                components: [0; 4],
                range: Range::of(aspect),
            };
            create(vk.create_image_view, "vkCreateImageView", (&raw const info).cast(), view)?;
        }
        let module = ShaderModuleCreateInfo {
            s_type: STRUCTURE_SHADER_MODULE_CREATE_INFO,
            next: null(),
            flags: 0,
            code_size: core::mem::size_of_val(&MOTION_SPIRV) as u32,
            code: MOTION_SPIRV.as_ptr(),
        };
        create(vk.create_shader_module, "vkCreateShaderModule", (&raw const module).cast(), &mut self.module)?;
        // the shader's set 0: the depth and the object motion sampled, the motion stored
        let binding = |binding, kind| SetLayoutBinding { binding, kind, count: 1, stages: SHADER_COMPUTE, samplers: null() };
        let bindings =
            [binding(0, DESCRIPTOR_SAMPLED_IMAGE), binding(1, DESCRIPTOR_SAMPLED_IMAGE), binding(2, DESCRIPTOR_STORAGE_IMAGE)];
        let set_layout = SetLayoutCreateInfo {
            s_type: STRUCTURE_DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
            next: null(),
            flags: 0,
            count: bindings.len() as u32,
            bindings: bindings.as_ptr(),
        };
        create(
            vk.create_set_layout,
            "vkCreateDescriptorSetLayout",
            (&raw const set_layout).cast(),
            &mut self.set_layout,
        )?;
        let constants = PushConstantRange { stages: SHADER_COMPUTE, offset: 0, size: size_of::<MotionFrame>() as u32 };
        let layout = PipelineLayoutCreateInfo {
            s_type: STRUCTURE_PIPELINE_LAYOUT_CREATE_INFO,
            next: null(),
            flags: 0,
            set_layout_count: 1,
            set_layouts: &self.set_layout,
            range_count: 1,
            ranges: &constants,
        };
        create(vk.create_pipeline_layout, "vkCreatePipelineLayout", (&raw const layout).cast(), &mut self.layout)?;
        let pipeline = ComputePipelineCreateInfo {
            s_type: STRUCTURE_COMPUTE_PIPELINE_CREATE_INFO,
            next: null(),
            flags: 0,
            pad: 0,
            stage: ShaderStageCreateInfo {
                s_type: STRUCTURE_PIPELINE_SHADER_STAGE_CREATE_INFO,
                next: null(),
                flags: 0,
                stage: SHADER_COMPUTE,
                module: self.module,
                name: c"main".as_ptr().cast(),
                specialization: null(),
            },
            layout: self.layout,
            base: 0,
            base_index: -1,
        };
        // SAFETY: one compute pipeline of the module and layout just made, no cache.
        unsafe {
            check(
                "vkCreateComputePipelines",
                raw::engine_vk_create_compute_pipelines(
                    vk.create_pipelines.0,
                    dev,
                    0,
                    0,
                    1,
                    (&raw const pipeline).cast(),
                    null(),
                    &mut self.pipeline,
                ),
            )?;
        }
        // one set: two sampled images and a storage image
        let sizes = [
            DescriptorPoolSize { kind: DESCRIPTOR_SAMPLED_IMAGE, count: 2 },
            DescriptorPoolSize { kind: DESCRIPTOR_STORAGE_IMAGE, count: 1 },
        ];
        let pool = DescriptorPoolCreateInfo {
            s_type: STRUCTURE_DESCRIPTOR_POOL_CREATE_INFO,
            next: null(),
            flags: 0,
            max_sets: 1,
            size_count: sizes.len() as u32,
            sizes: sizes.as_ptr(),
        };
        create(vk.create_descriptor_pool, "vkCreateDescriptorPool", (&raw const pool).cast(), &mut self.pool)?;
        let allocate = DescriptorSetAllocateInfo {
            s_type: STRUCTURE_DESCRIPTOR_SET_ALLOCATE_INFO,
            next: null(),
            pool: self.pool,
            count: 1,
            layouts: &self.set_layout,
        };
        // SAFETY: the pool's one set, of the layout just made.
        unsafe {
            check(
                "vkAllocateDescriptorSets",
                raw::engine_vk_allocate_descriptor_sets(
                    vk.allocate_sets.0,
                    dev,
                    (&raw const allocate).cast(),
                    &mut self.set,
                ),
            )?;
        }
        // every image is read and written in GENERAL, the layout the helper shares them in
        let images = self.views.map(|view| DescriptorImageInfo { sampler: 0, view, layout: LAYOUT_GENERAL });
        let write = |binding: u32, kind| WriteDescriptorSet {
            s_type: STRUCTURE_WRITE_DESCRIPTOR_SET,
            next: null(),
            set: self.set,
            binding,
            element: 0,
            count: 1,
            kind,
            image: &images[binding as usize],
            buffer: null(),
            texel_view: null(),
        };
        let writes =
            [write(0, DESCRIPTOR_SAMPLED_IMAGE), write(1, DESCRIPTOR_SAMPLED_IMAGE), write(2, DESCRIPTOR_STORAGE_IMAGE)];
        // SAFETY: the set's three bindings written with the views made above.
        unsafe {
            raw::engine_vk_update_descriptor_sets(
                vk.update_sets.0,
                dev,
                writes.len() as u32,
                writes.as_ptr().cast(),
                0,
                null(),
            );
        }
        Ok(())
    }

    /// Everything of the pass destroyed; nothing of it is on the queue.
    fn destroy(&self, device: &Device) {
        let (vk, dev) = (&device.vulkan, device.device.as_ptr());
        // SAFETY: each object is this device's, made here, used no more
        // (0 where it was not made); the pool takes its set with it.
        let destroy = |function: Function, object: u64| unsafe {
            raw::engine_vk_destroy_object(function.0, dev, low(object), high(object), null())
        };
        destroy(vk.destroy_descriptor_pool, self.pool);
        destroy(vk.destroy_pipeline, self.pipeline);
        destroy(vk.destroy_pipeline_layout, self.layout);
        destroy(vk.destroy_set_layout, self.set_layout);
        destroy(vk.destroy_shader_module, self.module);
        for view in self.views {
            destroy(vk.destroy_image_view, view);
        }
        // SAFETY: as above.
        unsafe {
            raw::engine_vk_destroy_image(
                vk.destroy_image.0,
                dev,
                low(self.objects.image),
                high(self.objects.image),
                null(),
            );
            raw::engine_vk_free_memory(
                vk.free_memory.0,
                dev,
                low(self.objects.memory),
                high(self.objects.memory),
                null(),
            );
        }
    }
}

// ---- the frame at the screen image

/// The frame where the screen image is next: its picture after the
/// post-process (`$user$present`) and its depth-stencil, both at the render
/// size.
pub struct Picture {
    pub colour: ColourTexture,
    pub depth: DepthTexture,
}

/// What the helper and the motion pass are told of a frame beside its
/// images.
#[derive(Clone, Copy)]
pub struct Told {
    /// The jitter of the frame's projection, in render pixels (DLSS's
    /// Jitter_Offset_X and _Y).
    pub jitter: [f32; 2],
    /// DLSS's history is thrown away (a Setup's first frame is, whatever
    /// this says).
    pub reset: bool,
    /// What the motion pass is given; without it the frame's motion is none
    /// anywhere (a still camera on a still world).
    pub motion: Option<Motion>,
}

impl Told {
    /// A frame with no jitter and no motion that goes on from the one before.
    pub const STILL: Told = Told { jitter: [0.0; 2], reset: false, motion: None };
}

/// What the motion pass is given of a frame (dlss_motion_026g.comp).
#[derive(Clone, Copy)]
pub struct Motion {
    /// This frame's clip position to the previous frame's, neither
    /// jittered: the engine's rows (previous clip = clip * this).
    pub reproject: [f32; 16],
    /// Where this frame's picture is moved by its jitter, in clip units.
    pub jitter_clip: [f32; 2],
    /// `$user$object_motion_vectors` where the engine's object motion pass
    /// drew this frame.
    pub objects: Option<ColourTexture>,
}

/// `render.screen_image`: renderer::present about to hand the frame's
/// picture to the screen image's quad.
pub struct ScreenImage {
    frame: raw::PresentFrame,
}

impl ScreenImage {
    pub fn at(ctx: &Context<'_>) -> Option<ScreenImage> {
        // SAFETY: the context is this site's: its ESP is the argument
        // renderer::present pushed for stage_screen_image::execute.
        let frame = unsafe { raw::site::render_screen_image::frame(ctx) }?;
        Some(ScreenImage { frame })
    }

    /// The frame's picture, as the quad would sample it.
    pub fn picture(&self) -> Option<ColourTexture> {
        // SAFETY: the pushed texture, alive for the call (present holds a
        // reference of it for the callee).
        unsafe { self.frame.texture() }.map(ColourTexture::of)
    }

    /// The quad samples `texture` instead: the callee's reference is one of
    /// `texture`, and the picture's is let go (the frame's targets hold
    /// their own).
    pub fn show(self, texture: ColourTexture) {
        // SAFETY: as picture; the word is execute's argument, which it
        // reads once entered and lets go at its end (0x1BD35A, 0x1BD589).
        unsafe {
            let Some(picture) = self.frame.texture() else { return };
            Texture::of(texture.raw()).hold();
            self.frame.set_texture(Some(texture.raw()));
            Texture::of(picture).release();
        }
    }
}

// ---- the watch

/// The wait of ours that is on DXVK's queue: the timeline, the value waited
/// for, and the two functions that read and signal it.
struct Watch {
    device: NonNull<c_void>,
    signal: Function,
    counter: Function,
    timeline: u64,
    queued: u64,
}
// SAFETY: handles of DXVK's device; vkGetSemaphoreCounterValue and
// vkSignalSemaphore may be called from any thread, and the lock makes it one
// at a time, never after the semaphore is destroyed (retire takes it out first).
unsafe impl Send for Watch {}

/// What the game's thread tells the helper's watch: None while no wait of
/// ours is on DXVK's queue.
static WATCH: Lock<Option<Watch>> = Lock::new(None);

impl Watch {
    fn reached(&self) -> Option<u64> {
        let mut value = 0;
        // SAFETY: vkGetSemaphoreCounterValue on the live timeline (the lock).
        let got = unsafe {
            raw::engine_vk_get_semaphore_counter_value(
                self.counter.0,
                self.device.as_ptr(),
                low(self.timeline),
                high(self.timeline),
                &mut value,
            )
        };
        (got == 0).then_some(value)
    }
}

/// How far the timeline is behind the wait on DXVK's queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Behind {
    pub queued: u64,
    pub reached: u64,
}

impl Behind {
    /// Whether the helper is the one the timeline waits for: the game's
    /// copy of a frame is done (an odd value, [`Frame::copied`]) and that
    /// frame's output is not. At an even value the game's next copy has not
    /// run yet: DXVK's queue is the one behind.
    pub fn helpers_turn(self) -> bool {
        self.reached % 2 == 1
    }
}

/// What the game does at one look at the timeline to play the part of a
/// helper that is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Play {
    /// The timeline has reached every wait.
    Done,
    /// Signal this value.
    Signal(u64),
    /// The game's next copy has not run on the GPU yet: look again.
    Wait,
}

/// The looks (a millisecond apart) the game's next copy is waited for before
/// its value is signalled too.
pub const PATIENCE: u32 = 200;

impl Play {
    /// The play at a look that found the timeline `behind`, after `waited`
    /// looks in a row at the game's turn. The helper's values are signalled
    /// one at a time, each right after the game's copy before it: a signal
    /// further ahead would be undone by the game's own copies still on
    /// DXVK's queue, which signal the values in between (seen: the timeline
    /// set to the last wait's value, a queued copy's lower value after it,
    /// and that copy's wait never met). A game's turn that does not come
    /// (that copy lost) is signalled too, so the queue cannot stay stuck.
    pub fn at(behind: Option<Behind>, waited: u32) -> Play {
        match behind {
            None => Play::Done,
            Some(behind) if behind.helpers_turn() || waited >= PATIENCE => Play::Signal(behind.reached + 1),
            Some(_) => Play::Wait,
        }
    }
}

/// The most looks of one release (3 s): a few frames are in flight, each a
/// signal and the wait for the game's next copy.
const RELEASE_LOOKS: u32 = 3000;

/// The wait on DXVK's queue that the timeline has not reached yet, if any.
pub fn behind(thread: &Thread) -> Option<Behind> {
    WATCH.with(thread, |watch| {
        let watch = watch.as_ref()?;
        let reached = watch.reached()?;
        (reached < watch.queued).then_some(Behind { queued: watch.queued, reached })
    })
}

/// The helper is gone: its part of the timeline played from here (a timeline
/// semaphore takes a host signal), value by value ([`Play`]), until every
/// wait on DXVK's queue is met, so the queue goes on. The last value
/// signalled.
pub fn release(thread: &Thread) -> Option<u64> {
    let (mut last, mut waited) = (None, 0);
    for _ in 0..RELEASE_LOOKS {
        let play = WATCH.with(thread, |watch| {
            let Some(watch) = watch.as_ref() else { return Play::Done };
            let Some(reached) = watch.reached() else { return Play::Done };
            let play = Play::at((reached < watch.queued).then_some(Behind { queued: watch.queued, reached }), waited);
            if let Play::Signal(value) = play {
                let info = SemaphoreSignalInfo {
                    s_type: STRUCTURE_SEMAPHORE_SIGNAL_INFO,
                    next: null(),
                    semaphore: watch.timeline,
                    value,
                };
                // SAFETY: vkSignalSemaphore on the live timeline (the lock), to
                // the value after its own; the helper, the only other
                // signaller of that value, is gone.
                if unsafe {
                    raw::engine_vk_signal_semaphore(watch.signal.0, watch.device.as_ptr(), (&raw const info).cast())
                } != 0
                {
                    return Play::Done;
                }
            }
            play
        });
        match play {
            Play::Done => break,
            Play::Signal(value) => (last, waited) = (Some(value), 0),
            Play::Wait => waited += 1,
        }
        // SAFETY: Sleep (the exe's import): the GPU runs the game's next copy meanwhile.
        unsafe { (raw::engine_sleep)(1) };
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_has_two_values_in_order() {
        assert_eq!((Frame(0).copied(), Frame(0).upscaled()), (1, 2));
        assert_eq!((Frame(7).copied(), Frame(7).upscaled()), (15, 16));
        assert_eq!(Frame(7).next(), Frame(8));
    }

    #[test]
    fn a_gone_helpers_values_are_signalled_one_at_a_time_after_the_games_copies() {
        // frames 5 to 7 are on the queue; the helper went after frame 4
        let queued = Frame(7).upscaled();
        let at = |reached| Some(Behind { queued, reached });
        // frame 5's copy has not run yet: the game's turn, waited for
        assert_eq!(Play::at(at(Frame(4).upscaled()), 0), Play::Wait);
        assert!(!at(Frame(4).upscaled()).unwrap().helpers_turn());
        // it ran: the helper's value for frame 5, and no further (frame 6's copy signals the next)
        assert!(at(Frame(5).copied()).unwrap().helpers_turn());
        assert_eq!(Play::at(at(Frame(5).copied()), 0), Play::Signal(Frame(5).upscaled()));
        assert_eq!(Play::at(at(Frame(5).upscaled()), 3), Play::Wait);
        assert_eq!(Play::at(at(Frame(7).copied()), 0), Play::Signal(queued));
        assert_eq!(Play::at(None, 0), Play::Done);
        // a copy that never comes is signalled too, after the patience
        assert_eq!(Play::at(at(Frame(5).upscaled()), PATIENCE), Play::Signal(Frame(6).copied()));
        for early in 0..SLOTS {
            assert!(Frame(early).buffers_free(0));
        }
        // frame 4 takes frame 0's pair: free once frame 1's copy (value 3) is done, past frame 0's wait (2)
        assert!(!Frame(SLOTS).buffers_free(2) && Frame(SLOTS).buffers_free(3));
        assert!(
            !Frame(SLOTS + 3).buffers_free(Frame(4).copied() - 1) && Frame(SLOTS + 3).buffers_free(Frame(4).copied())
        );
    }

    #[test]
    fn a_tick_wraps() {
        assert_eq!(Tick::of(5).since(Tick::of(u32::MAX - 4)), 10);
        assert_eq!(Tick::of(1500).since(Tick::of(500)), 1000);
    }

    #[test]
    fn a_refusal_keeps_the_start_of_the_text() {
        let mut text = [0u8; 256];
        text[..5].copy_from_slice(b"no RT");
        let refusal = Refusal::of(&text, -0x4530_0000 + 2);
        assert_eq!(refusal.text(), "no RT");
        assert_eq!(refusal.ngx, 0xBAD0_0002);
        // a text without its NUL, or cut inside a character: what is whole
        let long = [b'x'; 256];
        assert_eq!(Refusal::of(&long, 1).text().len(), REFUSAL_TEXT);
        let mut cut = [0u8; 256];
        cut[..2].copy_from_slice(&[b'a', 0xD0]);
        assert_eq!(Refusal::of(&cut, 1).text(), "a");
    }
}

//! Patch 125's watch: what happens in the game's process from its first
//! instruction on, kept for the report (`super::modules::snapshot`).
//!
//! Windows calls the exe's TLS callback (module_watch_026g.asm, which the
//! build names in the exe's TLS directory) on the main thread before the
//! exe's entry point, and then on every thread the process gets, before
//! that thread's own start. So the watch starts before any code of the
//! game or of `bugtrap.dll` runs: at the first call it registers ntdll's
//! DLL notification (`LdrRegisterDllNotification`), and the loader then
//! tells each DLL it maps, also one that is taken out of the loader's list
//! afterwards. A thread's call does nothing.
//!
//! Both calls come with the loader's lock held, on any thread, so they do
//! little: a few kernel32/ntdll calls outside the watch's own lock (a DLL's
//! header is read with `ReadProcessMemory`), then one record into fixed room. Nothing here is logged or acted on; the
//! report carries it.

use super::modules::{Kernel32, PATH, Path};
use super::world::read;
use super::{Context, Lock, Thread};
use crate::raw;
use core::ffi::{CStr, c_void};
use core::num::NonZeroUsize;
use core::ptr::NonNull;
use srp_validate::modules::{Load, Name, PeMarks, Place};

/// DLL loads kept at most.
const LOADS: usize = 32;

/// The first `N` things of a kind, and how many there were.
#[derive(Clone, Copy)]
struct Kept<T: Copy, const N: usize> {
    items: [Option<T>; N],
    seen: usize,
}

impl<T: Copy, const N: usize> Kept<T, N> {
    const NONE: Kept<T, N> = Kept { items: [None; N], seen: 0 };

    fn keep(&mut self, item: T) {
        if let Some(slot) = self.items.get_mut(self.seen) {
            *slot = Some(item);
        }
        self.seen = self.seen.saturating_add(1);
    }

    fn iter(&self) -> impl Iterator<Item = T> + '_ {
        self.items.iter().flatten().copied()
    }
}

/// A DLL the loader told of, as it was then.
#[derive(Clone, Copy)]
struct Told {
    name: Name,
    place: Place,
    base: usize,
    image_size: u32,
    pe: Option<PeMarks>,
    at: Seconds,
}

/// `LDR_DLL_NOTIFICATION_REASON_LOADED`: the loader mapped the DLL.
const LOADED: u32 = 1;

/// Seconds since the watch started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Seconds(u32);

/// `GetTickCount`'s milliseconds, which wrap.
#[derive(Clone, Copy)]
struct Tick(u32);

impl Tick {
    fn now() -> Tick {
        // SAFETY: GetTickCount (the exe's import) takes nothing.
        Tick(unsafe { (raw::engine_get_tick_count)() })
    }

    fn since(self, start: Tick) -> Seconds {
        Seconds(self.0.wrapping_sub(start.0) / 1000)
    }
}

/// The address of a function of Windows', kept as a number so the watch
/// can be reached from any thread.
#[derive(Clone, Copy)]
struct Function(NonZeroUsize);

impl Function {
    fn of(module: *mut c_void, name: &CStr) -> Option<Function> {
        // SAFETY: GetProcAddress (the exe's import) on a loaded module, a NUL-terminated name.
        NonZeroUsize::new(unsafe { (raw::engine_get_proc_address)(module, name.as_ptr().cast()) } as usize).map(Function)
    }

    fn pointer(self) -> NonNull<c_void> {
        NonNull::dangling().with_addr(self.0)
    }
}

/// The watch since its start.
struct Running {
    started: Tick,
    game: Path,
    temp: Path,
    windows: Path,
    loads: Kept<Told, LOADS>,
}

/// A DLL as the loader's notification tells it.
struct Dll<'a> {
    /// Its full path.
    path: &'a [u16],
    base: usize,
    image_size: u32,
    pe: Option<PeMarks>,
}

impl Running {
    /// The loader mapped `dll`. One of the Windows folder is not kept, as
    /// no module of it is told.
    fn loaded(&mut self, dll: &Dll<'_>, now: Tick) {
        let place = Place::of(dll.path, self.game.as_units(), self.temp.as_units(), self.windows.as_units());
        if let (Some(place), Some(name)) = (place, Name::of_path(dll.path)) {
            self.loads.keep(Told { name, place, base: dll.base, image_size: dll.image_size, pe: dll.pe, at: now.since(self.started) });
        }
    }
}

/// Off until the TLS callback's first call (a game whose exe has no such
/// callback, or whose Windows lacks a function, stays off: the report
/// then has no watch entries).
enum Watch {
    Off,
    Running(Running),
}

static WATCH: Lock<Watch> = Lock::new(Watch::Off);

/// `DLL_PROCESS_ATTACH`: Windows calls the TLS callback as the process starts.
const PROCESS_START: u32 = 1;

/// module_watch.thread: the TLS callback's call.
pub fn tls(ctx: &Context<'_>) {
    // SAFETY: the context is this entry's: its ESP module_watch_tls's frame.
    let Some(frame) = (unsafe { raw::site::module_watch_thread::frame(ctx) }) else { return };
    // SAFETY: the frame's words, there for the call.
    let (reason, notify) = unsafe { (read(frame.reason()), read(frame.notify())) };
    // a thread's start (2) and an end (3, 0) are nothing to the watch
    if reason == PROCESS_START {
        start(ctx.thread(), notify as usize);
    }
}

/// The game's process starts: the watch does, once, and from now on the
/// loader calls `notify` (module_watch_dll) for each DLL.
fn start(thread: &Thread, notify: usize) {
    let Some(kernel32) = Kernel32::get() else { return };
    // SAFETY: GetModuleHandleA (the exe's import), a NUL-terminated name; ntdll is in every process.
    let ntdll = unsafe { (raw::engine_get_module_handle_a)(c"ntdll.dll".as_ptr().cast()) };
    if ntdll.is_null() {
        return;
    }
    let running = Running {
        started: Tick::now(),
        game: kernel32.file_of(core::ptr::null_mut()).folder(),
        temp: kernel32.temp(),
        windows: kernel32.windows(),
        loads: Kept::NONE,
    };
    let first = WATCH.with(thread, |watch| match watch {
        Watch::Off => {
            *watch = Watch::Running(running);
            true
        }
        Watch::Running(_) => false,
    });
    if let (true, Some(register), Some(notify)) =
        (first, Function::of(ntdll, c"LdrRegisterDllNotification"), NonNull::new(notify as *mut c_void))
    {
        let mut cookie = core::ptr::null_mut();
        // SAFETY: LdrRegisterDllNotification's own export: no flags, `notify` module_watch_dll (our
        // code, as long as the exe), no context, the cookie's room. It is never unregistered.
        unsafe { raw::engine_ldr_register_dll_notification(register.pointer(), 0, notify.as_ptr(), core::ptr::null_mut(), &mut cookie) };
    }
}

/// module_watch.dll: the loader's notification.
pub fn dll(ctx: &Context<'_>) {
    // SAFETY: the context is this entry's: its ESP module_watch_dll's frame, whose data (the
    // loader's LDR_DLL_NOTIFICATION_DATA and its strings) is alive for the call.
    let told = unsafe {
        let Some(frame) = raw::site::module_watch_dll::frame(ctx) else { return };
        if read(frame.reason()) != LOADED {
            return;
        }
        let Some(data) = frame.data() else { return };
        let Some(name) = data.full_name() else { return };
        let units = name.buffer().cast::<u16>();
        if units.is_null() {
            return;
        }
        let mut path = [0u16; PATH];
        let len = (usize::from(read(name.length())) / 2).min(PATH);
        for (at, unit) in path.iter_mut().take(len).enumerate() {
            *unit = units.add(at).read_unaligned();
        }
        (path, len, read(data.base()) as usize, read(data.image_size()))
    };
    let (path, len, base, image_size) = told;
    // a load is told after the mapping: its header is there
    let pe = Kernel32::get().and_then(|kernel32| kernel32.pe_marks(base));
    let dll = Dll { path: path.get(..len).unwrap_or_default(), base, image_size, pe };
    let now = Tick::now();
    WATCH.with(ctx.thread(), |watch| {
        if let Watch::Running(running) = watch {
            running.loaded(&dll, now);
        }
    });
}

/// What the watch kept so far, copied out for a report.
pub struct KeptNow {
    loads: Kept<Told, LOADS>,
}

/// What the watch kept so far (nothing while it is off).
pub fn kept(thread: &Thread) -> KeptNow {
    WATCH.with(thread, |watch| match watch {
        Watch::Running(running) => KeptNow { loads: running.loads },
        Watch::Off => KeptNow { loads: Kept::NONE },
    })
}

impl KeptNow {
    /// The DLLs the loader said it mapped; `listed`: its list has a module
    /// at this base now.
    pub fn loads<'a>(&'a self, listed: impl Fn(usize) -> bool + 'a) -> impl Iterator<Item = Load> + 'a {
        self.loads.iter().map(move |load| Load {
            name: load.name,
            place: load.place,
            image_size: load.image_size,
            pe: load.pe,
            at: load.at.0,
            listed: listed(load.base),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn path(text: &str) -> Path {
        let mut units = [0; PATH];
        let text = wide(text);
        units.iter_mut().zip(&text).for_each(|(unit, &from)| *unit = from);
        Path::written(units, text.len() as u32)
    }

    #[test]
    fn the_first_of_a_kind_are_kept_and_all_counted() {
        let mut kept: Kept<u32, 3> = Kept::NONE;
        (0..5).for_each(|n| kept.keep(n));
        assert_eq!(kept.iter().collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(kept.seen, 5);
    }

    #[test]
    fn a_tick_wraps_and_counts_whole_seconds() {
        assert_eq!(Tick(5_999).since(Tick(1_000)), Seconds(4));
        assert_eq!(Tick(2_500).since(Tick(u32::MAX - 499)), Seconds(3));
    }

    #[test]
    fn loads_outside_windows_are_kept_with_their_place_and_time() {
        use srp_validate::modules::Folder;
        let mut running = Running {
            started: Tick(10_000),
            game: path("C:\\Games\\Skhid\\binaries\\x86\\survarium.exe").folder(),
            temp: path("C:\\Users\\Ivan\\AppData\\Local\\Temp\\"),
            windows: path("C:\\Windows"),
            loads: Kept::NONE,
        };
        let pe = Some(PeMarks { stamp: 0x4B4B_4321, checksum: 0 });
        let mut tell = |file: &str, base, pe, tick| {
            running.loaded(&Dll { path: &wide(file), base, image_size: 0x7000, pe }, Tick(tick));
        };
        tell("C:\\Windows\\System32\\dxgi.dll", 0x7000_0000, None, 11_000);
        tell("C:\\Games\\Skhid\\binaries\\x86\\bugtrap.dll", 0x1000_0000, None, 12_500);
        tell("C:\\Users\\Ivan\\AppData\\Local\\Temp\\added.dll", 0x2000_0000, pe, 400_000);
        tell("C:\\Users\\Ivan\\Downloads\\x\\y.dll", 0x3000_0000, None, 402_000);
        let kept = KeptNow { loads: running.loads };
        let loads: Vec<Load> = kept.loads(|base| base == 0x2000_0000).collect();
        assert_eq!(
            loads.iter().map(|load| (load.name.as_str(), load.place.folder, load.place.dir.as_str(), load.pe, load.at, load.listed)).collect::<Vec<_>>(),
            [
                ("bugtrap.dll", Folder::Game, "", None, 2, false),
                ("added.dll", Folder::Temp, "", pe, 390, true),
                ("y.dll", Folder::Other, "C:\\Users\\*\\Downloads\\x", None, 392, false),
            ]
        );
    }
}

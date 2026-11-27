//! A short-lived Win32 message-pump thread per observed connection. Hook
//! threads never touch the connection. Retiring a worker revokes its counts
//! immediately, including during a rapid reconnect while it is unhooking.
use super::{Lock, Thread, ThreadId, generated, modules::presented_window};
use crate::raw;
use core::ffi::{CStr, c_void};
use core::ptr::{NonNull, null, null_mut};
use core::sync::atomic::{AtomicPtr, AtomicU32, Ordering};
use srp_validate::input_report::Counts;

#[derive(Clone, Copy)]
enum Worker {
    Off,
    Starting(u32),
    Listening(ThreadId, Counts),
}
static WORKER: Lock<Worker> = Lock::new(Worker::Off);
static TICKET: AtomicU32 = AtomicU32::new(0);

// These process-lifetime exports are published before installing either
// hook. A retiring hook still has to forward even after the match closes.
static NEXT: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static FOREGROUND: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static ANCESTOR: AtomicPtr<c_void> = AtomicPtr::new(null_mut());

fn module(name: &CStr) -> Option<NonNull<c_void>> {
    // SAFETY: NUL-terminated name of a DLL the exe imports.
    NonNull::new(unsafe { (raw::engine_get_module_handle_a)(name.as_ptr().cast()) })
}

fn export(module: NonNull<c_void>, name: &CStr) -> Option<NonNull<c_void>> {
    // SAFETY: a permanently loaded system DLL and a NUL-terminated name.
    NonNull::new(unsafe { (raw::engine_get_proc_address)(module.as_ptr(), name.as_ptr().cast()) })
}

fn cancel_start(ticket: u32) {
    WORKER.with_current(|worker| {
        if matches!(*worker, Worker::Starting(wanted) if wanted == ticket) {
            *worker = Worker::Off;
        }
    });
}

pub(crate) fn start(thread: &Thread) {
    let ticket = TICKET.fetch_add(1, Ordering::Relaxed);
    WORKER.with(thread, |w| *w = Worker::Starting(ticket));
    let Some(create) = module(c"kernel32.dll").and_then(|k| export(k, c"CreateThread")) else {
        cancel_start(ticket);
        return;
    };
    // SAFETY: generated WINAPI ThreadProc entry, process lifetime. Its
    // parameter is an integer cookie, never dereferenced, and defaults/null
    // are valid for security, stack size and optional thread id.
    let handle = unsafe {
        raw::engine_create_thread(
            create,
            null_mut(),
            0,
            generated::input_report_worker as *const _,
            ticket as usize as *mut c_void,
            0,
            null_mut(),
        )
    };
    if !handle.is_null() {
        // SAFETY: the thread runs independently of this owned handle.
        unsafe { (raw::engine_close_handle)(handle) };
    } else {
        cancel_start(ticket);
    }
}

pub(crate) fn sample(thread: &Thread) -> Option<Counts> {
    WORKER.with(thread, |w| match *w {
        Worker::Listening(_, counts) => Some(counts),
        _ => None,
    })
}

pub(crate) fn stop(thread: &Thread) -> Option<Counts> {
    WORKER.with(thread, |w| match core::mem::replace(w, Worker::Off) {
        Worker::Listening(_, counts) => Some(counts),
        _ => None,
    })
}

struct Hook {
    handle: NonNull<c_void>,
    unhook: NonNull<c_void>,
}

impl Drop for Hook {
    fn drop(&mut self) {
        // SAFETY: this worker owns this installed hook and unregisters it once.
        unsafe { raw::engine_unhook_windows_hook_ex(self.unhook, self.handle.as_ptr()) };
    }
}

fn listen(ticket: u32) -> Option<()> {
    let user = module(c"user32.dll")?;
    let set = export(user, c"SetWindowsHookExW")?;
    let unhook = export(user, c"UnhookWindowsHookEx")?;
    let peek = export(user, c"PeekMessageW")?;
    let wait = export(user, c"MsgWaitForMultipleObjectsEx")?;
    NEXT.store(export(user, c"CallNextHookEx")?.as_ptr(), Ordering::Release);
    FOREGROUND.store(export(user, c"GetForegroundWindow")?.as_ptr(), Ordering::Release);
    ANCESTOR.store(export(user, c"GetAncestor")?.as_ptr(), Ordering::Release);
    // SAFETY: null requests the current executable, which contains both entries.
    let image = unsafe { (raw::engine_get_module_handle_a)(null()) };
    let install = |kind, entry| {
        // SAFETY: callers pair each LL hook kind with its generated HOOKPROC.
        // Thread id zero is required for these desktop-wide low-level hooks.
        NonNull::new(unsafe { raw::engine_set_windows_hook_ex_w(set, kind, entry, image, 0) })
            .map(|handle| Hook { handle, unhook })
    };
    let _keyboard = install(13, generated::input_report_keyboard as *const _)?;
    let _mouse = install(14, generated::input_report_mouse as *const _)?;
    let id = ThreadId::calling();
    let started = WORKER.with_current(|w| {
        if matches!(*w, Worker::Starting(wanted) if wanted == ticket) {
            *w = Worker::Listening(id, Counts::default());
            true
        } else {
            false
        }
    });
    if !started {
        return None;
    }
    while WORKER.with_current(|w| matches!(*w, Worker::Listening(owner, _) if owner == id)) {
        // SAFETY: no kernel handles; wake for QS_ALLINPUT or after 250 ms
        // to notice a quiet match's close. MWMO_INPUTAVAILABLE includes
        // messages already seen by a previous queue inspection.
        match unsafe { raw::engine_msg_wait_for_multiple_objects_ex(wait, 0, null(), 250, 0x04ff, 4) } {
            0 => {
                // Win32 MSG (including lPrivate), eight aligned DWORDs on x86.
                let mut msg = [0u32; 8];
                // SAFETY: writable MSG, no HWND filter, PM_REMOVE. Peek
                // dispatches the sent LL notifications; this thread owns no window.
                unsafe { raw::engine_peek_message_w(peek, msg.as_mut_ptr().cast(), null_mut(), 0, 0, 1) };
            }
            258 => {} // WAIT_TIMEOUT
            _ => break,
        }
    }
    WORKER.with_current(|w| {
        if matches!(*w, Worker::Listening(owner, _) if owner == id) {
            *w = Worker::Off
        }
    });
    Some(())
}

pub(super) unsafe fn worker(ticket: *mut c_void) -> u32 {
    let ticket = ticket as usize as u32;
    if listen(ticket).is_none() {
        cancel_start(ticket);
    }
    0
}

#[derive(Clone, Copy)]
enum Device {
    Keyboard,
    Mouse,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Event {
    KeyDown,
    ButtonDown,
    MouseMove,
}

impl Device {
    fn injected(self, message: u32, flags: u32) -> Option<Event> {
        match (self, message) {
            (Self::Keyboard, 0x100 | 0x104) if flags & 0x10 != 0 => Some(Event::KeyDown),
            (Self::Mouse, 0x201 | 0x204 | 0x207 | 0x20b) if flags & 1 != 0 => Some(Event::ButtonDown),
            (Self::Mouse, 0x200) if flags & 1 != 0 => Some(Event::MouseMove),
            _ => None,
        }
    }
}

fn foreground() -> bool {
    let (Some(game), Some(get), Some(root)) = (
        presented_window(),
        NonNull::new(FOREGROUND.load(Ordering::Acquire)),
        NonNull::new(ANCESTOR.load(Ordering::Acquire)),
    ) else {
        return false;
    };
    // SAFETY: the stored GetForegroundWindow export takes no arguments.
    let front = unsafe { raw::engine_get_foreground_window(get) };
    // SAFETY: GetAncestor accepts the presented HWND and GA_ROOT. A stale
    // handle fails, and a null foreground window never matches.
    !front.is_null() && front == unsafe { raw::engine_get_ancestor(root, game.as_ptr(), 2) }
}

unsafe fn dispatch(device: Device, code: i32, message: u32, data: *const c_void) -> u32 {
    if code == 0 && !data.is_null() {
        // HC_ACTION: only the flags DWORD is needed. KBDLLHOOKSTRUCT has
        // flags at +8; MSLLHOOKSTRUCT has flags at +12.
        let offset = match device {
            Device::Keyboard => 2,
            Device::Mouse => 3,
        };
        // SAFETY: the registered Windows callback guarantees its device's
        // aligned structure for HC_ACTION, alive until this callback returns.
        let flags = unsafe { data.cast::<u32>().add(offset).read() };
        if let Some(event) = device.injected(message, flags)
            && foreground()
        {
            let id = ThreadId::calling();
            WORKER.with_current(|w| {
                if let Worker::Listening(owner, counts) = w
                    && *owner == id
                {
                    let counter = match event {
                        Event::KeyDown => &mut counts.key_down,
                        Event::ButtonDown => &mut counts.button_down,
                        Event::MouseMove => &mut counts.mouse_move,
                    };
                    *counter = counter.saturating_add(1);
                }
            });
        }
    }
    let Some(next) = NonNull::new(NEXT.load(Ordering::Acquire)) else {
        return 0;
    };
    // SAFETY: process-lifetime CallNextHookEx, HHOOK ignored. Forward the
    // original arguments even for negative codes and return its exact result.
    unsafe { raw::engine_call_next_hook_ex(next, null_mut(), code as u32, message, data) }
}

pub(super) unsafe fn keyboard(code: i32, msg: u32, data: *const c_void) -> u32 {
    // SAFETY: only the generated WH_KEYBOARD_LL entry invokes this function.
    unsafe { dispatch(Device::Keyboard, code, msg, data) }
}

pub(super) unsafe fn mouse(code: i32, msg: u32, data: *const c_void) -> u32 {
    // SAFETY: only the generated WH_MOUSE_LL entry invokes this function.
    unsafe { dispatch(Device::Mouse, code, msg, data) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_injected_downs_and_motion_count() {
        for message in [0x100, 0x104] {
            assert_eq!(Device::Keyboard.injected(message, 0x11), Some(Event::KeyDown));
            assert_eq!(Device::Keyboard.injected(message, 1), None);
        }
        for message in [0x101, 0x105] {
            assert_eq!(Device::Keyboard.injected(message, 0x90), None);
        }
        for message in [0x201, 0x204, 0x207, 0x20b] {
            assert_eq!(Device::Mouse.injected(message, 3), Some(Event::ButtonDown));
            assert_eq!(Device::Mouse.injected(message, 0), None);
        }
        assert_eq!(Device::Mouse.injected(0x200, 1), Some(Event::MouseMove));
        for message in [0x202, 0x205, 0x208, 0x20c, 0x20a] {
            assert_eq!(Device::Mouse.injected(message, 1), None);
        }
    }
}

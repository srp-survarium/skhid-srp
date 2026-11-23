//! The platform mouse's DirectInput device, as its poll found it (patch 202,
//! client_patch.md 3.106).
//!
//! `mouse::execute` (0x3B9680) reads the device's state every poll
//! (GetDeviceState). On DIERR_INPUTLOST or DIERR_NOTACQUIRED it asks for the
//! device again (Acquire) and reads once more. A failed Acquire, or a read
//! failing with another code, leaves the poll without mouse input, and the
//! game logs nothing. The device's cooperative level is always
//! DISCL_FOREGROUND, exclusive in a match (`set_exclusive` 0x3B9800).

use core::ffi::c_void;
use core::ptr::NonNull;

use super::{Context, Thread};
use crate::raw;

/// A failed HRESULT of the device (its sign bit set).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Failure(u32);

/// An HRESULT of FACILITY_WIN32 (HRESULT_FROM_WIN32 of an error number).
const fn win32(error: u32) -> u32 {
    1 << 31 | 7 << 16 | error
}

/// An HRESULT of FACILITY_ITF (an interface's own).
const fn interface(code: u32) -> u32 {
    1 << 31 | 4 << 16 | code
}

/// An HRESULT of FACILITY_NULL.
const fn general(code: u32) -> u32 {
    1 << 31 | code
}

impl Failure {
    /// The codes Acquire and GetDeviceState give, by dinput.h's names.
    const NAMED: [(u32, &'static str); 10] = [
        (win32(5), "DIERR_OTHERAPPHASPRIO"), // E_ACCESSDENIED
        (win32(6), "E_HANDLE"),
        (win32(12), "DIERR_NOTACQUIRED"),
        (win32(14), "DIERR_OUTOFMEMORY"),
        (win32(21), "DIERR_NOTINITIALIZED"),
        (win32(30), "DIERR_INPUTLOST"),
        (win32(87), "DIERR_INVALIDPARAM"),
        (interface(0x209), "DIERR_UNPLUGGED"),
        (general(0xA), "E_PENDING"),
        (general(0x4005), "DIERR_GENERIC"), // E_FAIL
    ];

    /// A failure, or None for a success (S_OK, S_FALSE, ...).
    pub fn of(code: u32) -> Option<Failure> {
        (code & 1 << 31 != 0).then_some(Failure(code))
    }

    pub fn code(self) -> u32 {
        self.0
    }

    /// dinput.h's name of the code, for the codes the device gives.
    pub fn name(self) -> Option<&'static str> {
        Self::NAMED.iter().find(|(code, _)| *code == self.0).map(|&(_, name)| name)
    }
}

/// The device's cooperative level (always with DISCL_FOREGROUND).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Exclusive,
    NonExclusive,
}

/// Which window is in front (GetForegroundWindow) against the one the
/// device's level is set for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Front {
    Game,
    Other,
    /// No window: the desktop is switching or locked.
    Nothing,
    /// user32 has no GetForegroundWindow to ask.
    Unknown,
}

/// The mouse at a poll that got no input, read once.
#[derive(Clone, Copy, Debug)]
pub struct Unpolled {
    pub failure: Failure,
    pub level: Level,
    /// The device's window, to ask [`Front`] of only when it is told.
    window: u32,
}

impl Unpolled {
    /// At `mouse.read_failed`: a read failed with a code execute answers
    /// with no Acquire.
    pub fn read_failed(ctx: &Context<'_>) -> Option<Unpolled> {
        use raw::site::mouse_read_failed as site;
        // SAFETY: the site is inside execute, which holds the mouse's lock:
        // EBP the lock, inside the mouse, alive for the call; EAX a value.
        let (mouse, result) = unsafe { (site::lock(ctx)?.mouse(), site::result(ctx)) };
        Self::of(mouse, result)
    }

    /// At `mouse.acquire_join`: the re-Acquire failed. None when the join
    /// came from the "mouse device is lost" line, whose paths leave another
    /// value than the mouse in ESI and no HRESULT in EAX.
    pub fn acquire_failed(ctx: &Context<'_>) -> Option<Unpolled> {
        use raw::site::mouse_acquire_join as site;
        // SAFETY: as `read_failed`; ESI and EAX are values, compared only.
        let (mouse, esi, result) = unsafe { (site::lock(ctx)?.mouse(), site::esi(ctx), site::result(ctx)) };
        if esi != mouse.address() {
            return None;
        }
        Self::of(mouse, result)
    }

    fn of(mouse: raw::Mouse, result: u32) -> Option<Unpolled> {
        // SAFETY: the mouse of the running execute (its lock held); the
        // vtable is read to check it is the platform mouse before the rest.
        let (window, exclusive) = unsafe {
            if *mouse.vtable() != raw::image::MOUSE_VTABLE {
                return None;
            }
            (*mouse.window(), *mouse.exclusive())
        };
        let level = if exclusive != 0 { Level::Exclusive } else { Level::NonExclusive };
        Some(Unpolled { failure: Failure::of(result)?, level, window })
    }

    /// The window in front now (user32's GetForegroundWindow, which the
    /// game does not import), against the device's.
    pub fn front(&self, _: &Thread) -> Front {
        // SAFETY: kernel32's imports through the exe's slots, NUL-terminated
        // names; user32 is the game's (its window's), loaded for the
        // process's life; GetForegroundWindow takes nothing.
        let front = unsafe {
            let user32 = (raw::engine_get_module_handle_a)(c"user32.dll".as_ptr().cast());
            let function = if user32.is_null() {
                None
            } else {
                NonNull::new((raw::engine_get_proc_address)(user32, c"GetForegroundWindow".as_ptr().cast()))
            };
            let Some(function) = function else { return Front::Unknown };
            raw::engine_get_foreground_window(function)
        };
        Front::of(front, self.window)
    }
}

impl Front {
    fn of(front: *mut c_void, window: u32) -> Front {
        match front as usize as u32 {
            0 => Front::Nothing,
            front if front == window => Front::Game,
            _ => Front::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_device_codes_are_dinput_hs() {
        let name = |code| Failure::of(code).and_then(Failure::name);
        assert_eq!(name(0x8007_0005), Some("DIERR_OTHERAPPHASPRIO"));
        assert_eq!(name(0x8007_000C), Some("DIERR_NOTACQUIRED"));
        assert_eq!(name(0x8007_001E), Some("DIERR_INPUTLOST"));
        assert_eq!(name(0x8007_0015), Some("DIERR_NOTINITIALIZED"));
        assert_eq!(name(0x8007_0057), Some("DIERR_INVALIDPARAM"));
        assert_eq!(name(0x8004_0209), Some("DIERR_UNPLUGGED"));
        assert_eq!(name(0x8000_000A), Some("E_PENDING"));
        assert_eq!(name(0x8000_4005), Some("DIERR_GENERIC"));
        assert_eq!(name(0x8000_1234), None);
        assert_eq!(Failure::of(0), None);
        assert_eq!(Failure::of(1), None, "S_FALSE: already acquired");
    }

    #[test]
    fn the_front_is_the_devices_window_or_not() {
        let window = 0x0001_0042;
        assert_eq!(Front::of(window as usize as *mut c_void, window), Front::Game);
        assert_eq!(Front::of(0x0002_0040 as *mut c_void, window), Front::Other);
        assert_eq!(Front::of(core::ptr::null_mut(), window), Front::Nothing);
    }
}

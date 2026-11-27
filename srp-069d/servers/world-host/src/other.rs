//! Stubs for the non-KERNEL32 imports the world path is allowed to reach. Everything else traps.
#![allow(non_snake_case)]

use crate::hlog;
use crate::strings::*;

type A = u64;

// WS2_32 #115 = WSAStartup (called by 5 static constructors)
unsafe extern "win64" fn WSAStartup(ver: A, data: A) -> A {
    if data != 0 {
        unsafe {
            std::ptr::write_bytes(data as *mut u8, 0, 408);
            *(data as *mut u16) = ver as u16;
            *((data + 2) as *mut u16) = 0x0202;
        }
    }
    0
}

// SHELL32!SHGetKnownFolderPath -> the host's private drive Z: (maps to <out>/zdrive)
unsafe extern "win64" fn SHGetKnownFolderPath(_id: A, _flags: A, _tok: A, out: A) -> A {
    let p = leak_wstr("Z:\\Users\\host\\AppData\\Local");
    unsafe { *(out as *mut u64) = p };
    hlog!("SHGetKnownFolderPath -> Z:\\Users\\host\\AppData\\Local");
    0
}
// OLEAUT32 #2 = SysAllocString. Only caller seen: 0x20011DC90 (a COM check on the exe path from the
// engine_world ctor via 0x200108150). NULL = its own "out of memory" branch: no COM in the host.
unsafe extern "win64" fn SysAllocString(s: A) -> A {
    hlog!("OLEAUT32!SysAllocString({}) -> NULL (no COM in the host)", if s != 0 { unsafe { wstr(s) } } else { String::new() });
    0
}
// ole32 CoInitializeEx/CoUninitialize: the video-memory probe 0x20009A6F0 brackets LoadLibraryW("dxgi.dll")
// with them; dxgi is absent (NULL), so no COM object is ever created.
unsafe extern "win64" fn CoInitializeEx(_r: A, _m: A) -> A {
    0
}
unsafe extern "win64" fn CoUninitialize() -> A {
    0
}
// ADVAPI32!GetUserNameW (0x2000AFE1A, engine_world ctor): a fixed name.
unsafe extern "win64" fn GetUserNameW(buf: A, size: A) -> A {
    let cap = unsafe { *(size as *const u32) } as usize;
    let (n, ok) = unsafe { put_wstr(buf, cap, "host") };
    unsafe { *(size as *mut u32) = if ok { (n + 1) as u32 } else { n as u32 } };
    ok as A
}
// WINMM timeBeginPeriod/timeEndPeriod (CPU clock measurement in core init 0x2000B001A): no-op, TIMERR_NOERROR.
unsafe extern "win64" fn timeBeginPeriod(_p: A) -> A {
    0
}
unsafe extern "win64" fn timeEndPeriod(_p: A) -> A {
    0
}
// vostok_scaleform!scaleform_engine::initialize (from core init 0x2000FC2E0): it only registers the engine's
// callbacks with the UI DLL. No-op: there is no Scaleform in the host.
unsafe extern "win64" fn scaleform_initialize() -> A {
    hlog!("vostok_scaleform!scaleform_engine::initialize -> no-op");
    0
}
// ole32!CoCreateInstance: no COM classes exist in the host (first caller: the :sound thread's XAudio2
// creation 0x20061F4F2). REGDB_E_CLASSNOTREG, *ppv = NULL.
unsafe extern "win64" fn CoCreateInstance(clsid: A, _outer: A, _ctx: A, _iid: A, ppv: A) -> A {
    let g = if clsid != 0 { unsafe { std::slice::from_raw_parts(clsid as *const u8, 16) }.to_vec() } else { vec![] };
    if (g == crate::xaudio::CLSID_XAUDIO2_27 || g == crate::xaudio::CLSID_XAUDIO2_27_DEBUG) && !crate::k32::NO_XAUDIO.load(std::sync::atomic::Ordering::Relaxed) {
        let x = crate::xaudio::xaudio2();
        hlog!("ole32!CoCreateInstance(XAudio2 2.7) from {:#x} -> host null XAudio2 {x:#x}", crate::thunks::caller());
        if ppv != 0 {
            unsafe { (ppv as *mut u64).write_unaligned(x) };
        }
        return 0;
    }
    if g == crate::xaudio::CLSID_MMDEVICE_ENUMERATOR && !crate::k32::NO_XAUDIO.load(std::sync::atomic::Ordering::Relaxed) {
        let m = crate::xaudio::mm_device_enumerator();
        hlog!("ole32!CoCreateInstance(MMDeviceEnumerator) from {:#x} -> host enumerator without endpoints {m:#x}", crate::thunks::caller());
        if ppv != 0 {
            unsafe { (ppv as *mut u64).write_unaligned(m) };
        }
        return 0;
    }
    hlog!("ole32!CoCreateInstance(clsid {:02x?}) from {:#x} -> REGDB_E_CLASSNOTREG", g, crate::thunks::caller());
    if ppv != 0 {
        unsafe { (ppv as *mut u64).write_unaligned(0) };
    }
    0x8004_0154
}
// SHELL32!SHFileOperationW: the game ctor deletes a folder (0x200108620: FO_DELETE, flags 0x414). Only paths on the
// host's private drive Z: are deleted; anything else is logged and left alone. Returns 0 (success).
unsafe extern "win64" fn SHFileOperationW(op: A) -> A {
    let func = unsafe { ((op + 8) as *const u32).read_unaligned() };
    let from = unsafe { wstr(((op + 0x10) as *const u64).read_unaligned()) };
    let abs = crate::files::win_abs(&from);
    if func == 3 && abs.to_ascii_uppercase().starts_with("Z:") {
        let (p, _) = crate::files::host_paths(&abs);
        let _ = std::fs::remove_dir_all(&p);
        hlog!("SHFileOperationW(FO_DELETE, {abs}) -> deleted in the private drive");
    } else {
        hlog!("SHFileOperationW(func {func}, {abs}) -> not done (outside the private drive)");
    }
    0
}
unsafe extern "win64" fn CoTaskMemFree(_p: A) -> A {
    0
}

// USER32: the fatal-error path. Log the text and stop.
unsafe extern "win64" fn MessageBoxA(_w: A, text: A, cap: A, _t: A) -> A {
    hlog!("MessageBoxA [{}]: {}", unsafe { cstr(cap) }, unsafe { cstr(text) });
    crate::run::abort_run(9)
}
unsafe extern "win64" fn MessageBoxW(_w: A, text: A, cap: A, _t: A) -> A {
    hlog!("MessageBoxW [{}]: {}", unsafe { wstr(cap) }, unsafe { wstr(text) });
    crate::run::abort_run(9)
}
unsafe extern "win64" fn GetTopWindow(_w: A) -> A {
    0
}
/// Host window handles: no window exists; the values are opaque handles the exe only passes back to USER32.
pub const HOST_HWND: u64 = 0x10010;
pub const HOST_DESKTOP: u64 = 0x10014;
unsafe extern "win64" fn GetDesktopWindow() -> A {
    HOST_DESKTOP
}
/// Window/desktop rectangle of the host "window": 1920x1080 at 0,0.
unsafe extern "win64" fn GetWindowRect(w: A, r: A) -> A {
    hlog!("USER32!GetWindowRect({w:#x}) -> 0,0,1920,1080 (host window, no real window)");
    unsafe {
        let p = r as *mut i32;
        p.write_unaligned(0);
        p.add(1).write_unaligned(0);
        p.add(2).write_unaligned(1920);
        p.add(3).write_unaligned(1080);
    }
    1
}
unsafe extern "win64" fn sf_set_render_window_handle(_this: A, h: A) -> A {
    hlog!("vostok_scaleform!flash_factory::set_render_window_handle({h:#x}) -> no-op");
    0
}

// PSAPI (memory report in the engine ctor)
unsafe extern "win64" fn GetProcessMemoryInfo(_p: A, c: A, cb: A) -> A {
    let st = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = |k: &str| -> u64 {
        st.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse().ok()).unwrap_or(0) * 1024
    };
    unsafe {
        std::ptr::write_bytes(c as *mut u8, 0, cb as usize);
        *(c as *mut u32) = cb as u32;
        let w = |o: u64, v: u64| *((c + o) as *mut u64) = v;
        w(8, kb("VmHWM:"));
        w(16, kb("VmRSS:"));
        w(56, kb("VmRSS:"));
        w(64, kb("VmHWM:"));
        if cb >= 80 {
            w(72, kb("VmRSS:"));
        }
    }
    1
}
unsafe extern "win64" fn GetPerformanceInfo(p: A, cb: A) -> A {
    unsafe {
        std::ptr::write_bytes(p as *mut u8, 0, cb as usize);
        *(p as *mut u32) = cb as u32;
        let w = |o: u64, v: u64| *((p + o) as *mut u64) = v;
        let pages = |mb: u64| mb * 256;
        w(8, pages(8192));
        w(16, pages(24576));
        w(24, pages(8192));
        w(32, pages(16384));
        w(40, pages(8192));
        w(48, pages(1024));
        w(56, pages(512));
        w(64, pages(256));
        w(72, pages(256));
        w(80, 4096);
    }
    1
}

macro_rules! t {
    ($dll:expr; $($name:expr => $f:ident),* $(,)?) => { vec![$(($dll, $name, $f as *const () as usize)),*] };
}

/// (dll lowercase, import name, implementation)
pub fn table() -> Vec<(&'static str, &'static str, usize)> {
    let mut v = Vec::new();
    v.extend(t!["ws2_32.dll"; "#115" => WSAStartup]);
    v.extend(t!["shell32.dll"; "SHGetKnownFolderPath" => SHGetKnownFolderPath, "SHFileOperationW" => SHFileOperationW]);
    v.extend(t!["ole32.dll"; "CoTaskMemFree" => CoTaskMemFree, "CoInitializeEx" => CoInitializeEx, "CoCreateInstance" => CoCreateInstance, "CoUninitialize" => CoUninitialize]);
    v.push(("vostok_scaleform.dll", "?initialize@scaleform_engine@survarium@@YAXP6APEAX_K@ZP6AXPEAX@ZP6AXEPEBD@ZP6AX4PEAVvostok_delegated_file@2@@ZP6A_NXZ8P6A_NAEBV?$function@$$A6AXXZ@boost@@@ZP6AXXZP6AXPEA_NW4process_error_enum@vostok@@PEA_NW4assert_enum@7@4444I@Z8@Z", scaleform_initialize as *const () as usize));
    v.extend(t!["winmm.dll"; "timeBeginPeriod" => timeBeginPeriod, "timeEndPeriod" => timeEndPeriod]);
    v.extend(t!["advapi32.dll"; "GetUserNameW" => GetUserNameW]);
    v.extend(t!["oleaut32.dll"; "#2" => SysAllocString]);
    v.extend(t!["user32.dll"; "MessageBoxA" => MessageBoxA, "MessageBoxW" => MessageBoxW, "GetTopWindow" => GetTopWindow, "GetDesktopWindow" => GetDesktopWindow, "GetWindowRect" => GetWindowRect]);
    v.extend(t!["vostok_scaleform.dll"; "?set_render_window_handle@flash_factory@survarium@@QEAAXPEAUHWND__@@@Z" => sf_set_render_window_handle]);
    v.extend(t!["psapi.dll"; "GetProcessMemoryInfo" => GetProcessMemoryInfo, "GetPerformanceInfo" => GetPerformanceInfo]);
    v
}

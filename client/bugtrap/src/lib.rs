#![no_std]
#![allow(non_snake_case, unsafe_op_in_unsafe_fn)]

#[cfg(not(all(target_os = "windows", target_arch = "x86")))]
compile_error!(
    "This DLL implements the native ANSI Win32 BugTrap ABI; build for i686-pc-windows-msvc."
);

mod api;
mod archive;
mod arithmetic;
mod crash_text;
mod dump;
mod modules;
mod logs;
mod notice;
mod notice_text;
mod runtime;
mod sanitize;
mod text;
mod ui;
mod zip;

use core::{
    ffi::c_void,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{Diagnostics::Debug::*, LibraryLoader::DisableThreadLibraryCalls, Threading::*},
};

static INSTALLED: AtomicBool = AtomicBool::new(false);
static PREVIOUS: AtomicUsize = AtomicUsize::new(0);

#[unsafe(no_mangle)]
pub unsafe extern "system" fn BT_InstallSehFilter() -> LPTOP_LEVEL_EXCEPTION_FILTER {
    // The EXE's CRT may replace the filter after DLL_PROCESS_ATTACH. An explicit
    // install must reinstall it, without making repeated installs point to us.
    let old = SetUnhandledExceptionFilter(Some(BT_SehFilter));
    let address = old.map_or(0, |f| f as usize);
    if address != BT_SehFilter as *const () as usize {
        PREVIOUS.store(address, Ordering::SeqCst);
    }
    INSTALLED.store(true, Ordering::SeqCst);
    core::mem::transmute(PREVIOUS.load(Ordering::SeqCst))
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn BT_UninstallSehFilter() {
    if INSTALLED.swap(false, Ordering::SeqCst) {
        SetUnhandledExceptionFilter(core::mem::transmute(PREVIOUS.swap(0, Ordering::SeqCst)));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(module: HINSTANCE, reason: u32, _: *mut c_void) -> i32 {
    match reason {
        1 => {
            DisableThreadLibraryCalls(module);
            BT_InstallSehFilter();
        }
        0 => BT_UninstallSehFilter(),
        _ => {}
    }
    1
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn BT_SehFilter(exception: *const EXCEPTION_POINTERS) -> i32 {
    if exception.is_null()
        || (*exception).ExceptionRecord.is_null()
        || (*exception).ContextRecord.is_null()
    {
        return EXCEPTION_CONTINUE_SEARCH;
    }
    let overflow = (*(*exception).ExceptionRecord).ExceptionCode == EXCEPTION_STACK_OVERFLOW;
    // No formatting, allocation, settings locks, or large locals on the faulting stack.
    if !dump::capture(exception.cast_mut(), true) || overflow {
        return EXCEPTION_CONTINUE_SEARCH;
    }
    match api::EXIT_MODE.load(Ordering::Relaxed) {
        1 => EXCEPTION_CONTINUE_SEARCH,
        2 => EXCEPTION_EXECUTE_HANDLER,
        _ => {
            TerminateProcess(GetCurrentProcess(), u32::MAX);
            EXCEPTION_CONTINUE_SEARCH
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn BT_CppFilter(exception: *mut EXCEPTION_POINTERS) -> i32 {
    BT_SehFilter(exception)
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn BT_NetFilter(exception: *mut EXCEPTION_POINTERS) -> i32 {
    BT_SehFilter(exception)
}

unsafe extern "C" fn synthetic() -> i32 {
    let mut context: CONTEXT = core::mem::zeroed();
    RtlCaptureContext(&mut context);
    let mut record: EXCEPTION_RECORD = core::mem::zeroed();
    record.ExceptionCode = EXCEPTION_ACCESS_VIOLATION;
    record.ExceptionAddress = context.Eip as *mut c_void;
    record.NumberParameters = 2;
    let mut exception = EXCEPTION_POINTERS {
        ExceptionRecord: &mut record,
        ContextRecord: &mut context,
    };
    BT_SehFilter(&mut exception)
}
// Raise only after the Rust capture frames have returned: panic=abort Rust
// frames cannot carry a foreign SEH unwind. The tail jump preserves the native
// caller's return address and gives RaiseException its four stdcall arguments.
// EXECUTE_HANDLER returns normally; CONTINUE_SEARCH reaches the caller's SEH.
macro_rules! synthetic_entry {
    ($name:ident) => {
        #[unsafe(no_mangle)]
        #[unsafe(naked)]
        pub unsafe extern "C" fn $name() {
            core::arch::naked_asm!(
                "call {capture}",
                "test eax, eax",
                "jnz 2f",
                "pop ecx",
                "push 0",
                "push 0",
                "push 0",
                "push 0xc0000005",
                "push ecx",
                "jmp {raise}",
                "2:",
                "ret",
                capture = sym synthetic,
                raise = sym RaiseException,
            );
        }
    };
}
synthetic_entry!(BT_CallCppFilter);
synthetic_entry!(BT_CallSehFilter);
synthetic_entry!(BT_CallNetFilter);

// Legacy interception patched other modules' imports. Installing our own filter
// is supported; modifying another module's exception policy is intentionally not.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn BT_InterceptSUEF(_: HMODULE, _: i32) {
    SetLastError(ERROR_NOT_SUPPORTED);
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    unsafe {
        TerminateProcess(GetCurrentProcess(), 0xE0000001);
    }
    loop {
        core::hint::spin_loop();
    }
}

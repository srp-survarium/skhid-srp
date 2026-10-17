use crate::{api, text::Wide};
use core::{
    cell::UnsafeCell,
    ffi::c_void,
    fmt::Write,
    ptr::{null, null_mut},
    sync::atomic::{AtomicU32, Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::*,
    System::{Diagnostics::Debug::*, LibraryLoader::*, SystemInformation::*, Threading::*},
};

const IDLE: u32 = 0;
const SNAPSHOT: u32 = 1;
const CRASH_WRITING: u32 = 2;
const CRASH_CLOSED: u32 = 3;
static PHASE: AtomicU32 = AtomicU32::new(IDLE);
static OWNER: AtomicU32 = AtomicU32::new(0);
static WORKER: AtomicU32 = AtomicU32::new(0);
static SEQUENCE: AtomicU32 = AtomicU32::new(0);
struct Request {
    exception: MINIDUMP_EXCEPTION_INFORMATION,
    crash: bool,
    saved: bool,
}
struct Slot(UnsafeCell<Request>);
// Only the PHASE owner accesses this slot, and it joins the worker before releasing it.
unsafe impl Sync for Slot {}
static REQUEST: Slot = Slot(UnsafeCell::new(Request {
    exception: MINIDUMP_EXCEPTION_INFORMATION {
        ThreadId: 0,
        ExceptionPointers: null_mut(),
        ClientPointers: 0,
    },
    crash: false,
    saved: false,
}));

type WriteDump = unsafe extern "system" fn(
    HANDLE,
    u32,
    HANDLE,
    i32,
    *const MINIDUMP_EXCEPTION_INFORMATION,
    *const MINIDUMP_USER_STREAM_INFORMATION,
    *const c_void,
) -> i32;

pub unsafe fn capture(exception: *mut EXCEPTION_POINTERS, crash: bool) -> bool {
    let caller = GetCurrentThreadId();
    let phase = if crash { CRASH_WRITING } else { SNAPSHOT };
    loop {
        let busy = match PHASE.compare_exchange(IDLE, phase, Ordering::Acquire, Ordering::Acquire) {
            Ok(_) => break,
            Err(busy) => busy,
        };
        // Recursive capture must not wait for its own owner or writer.
        if !crash
            || OWNER.load(Ordering::Acquire) == caller
            || WORKER.load(Ordering::Acquire) == caller
        {
            SetLastError(ERROR_BUSY);
            return false;
        }
        if busy == CRASH_WRITING || busy == CRASH_CLOSED {
            // A concurrent crash may exit once the primary crash dump closes,
            // without waiting for its callbacks or modal dialog.
            while PHASE.load(Ordering::Acquire) == CRASH_WRITING {
                Sleep(1);
            }
            return true;
        }
        // A snapshot has no crash context or UI. Wait for its owner to release
        // the request, then retry with this thread's exception pointers.
        Sleep(1);
    }
    OWNER.store(caller, Ordering::Release);
    let request = REQUEST.0.get();
    (*request).exception.ThreadId = caller;
    (*request).exception.ExceptionPointers = exception;
    (*request).crash = crash;
    (*request).saved = false;
    let thread = CreateThread(
        null(),
        1024 * 1024,
        Some(worker),
        request.cast(),
        STACK_SIZE_PARAM_IS_A_RESERVATION,
        null_mut(),
    );
    let mut error = 0;
    if !thread.is_null() {
        WaitForSingleObject(thread, INFINITE);
        GetExitCodeThread(thread, &mut error);
        CloseHandle(thread);
    } else if exception.is_null()
        || (*(*exception).ExceptionRecord).ExceptionCode != EXCEPTION_STACK_OVERFLOW
    {
        error = worker(request.cast());
    } else {
        error = GetLastError();
    }
    let saved = (*request).saved;
    WORKER.store(0, Ordering::Release);
    OWNER.store(0, Ordering::Release);
    PHASE.store(IDLE, Ordering::Release);
    SetLastError(error);
    if crash { true } else { saved }
}

unsafe fn folder() -> Wide<32768> {
    let mut folder = Wide::new();
    let n =
        GetModuleFileNameW(null_mut(), folder.buf.as_mut_ptr(), folder.buf.len() as u32) as usize;
    if n > 0 && n < folder.buf.len() {
        if let Some(i) = folder.buf[..n]
            .iter()
            .rposition(|&c| c == b'\\' as u16 || c == b'/' as u16)
        {
            folder.len = i + 1;
            folder.buf[folder.len] = 0;
        }
    }
    folder
}
unsafe fn dbghelp(folder: &Wide<32768>) -> HMODULE {
    let mut path = Wide::<32768>::new();
    if folder.len > 0
        && path.wide(&folder.buf[..folder.len]).is_ok()
        && write!(path, "dbghelp.dll").is_ok()
    {
        let lib = LoadLibraryW(path.ptr());
        if !lib.is_null() {
            return lib;
        }
    }
    path.len = GetSystemDirectoryW(path.buf.as_mut_ptr(), path.buf.len() as u32) as usize;
    if path.len == 0 || path.len >= path.buf.len() - 13 {
        return null_mut();
    }
    if write!(path, "\\dbghelp.dll").is_err() {
        return null_mut();
    }
    LoadLibraryW(path.ptr())
}

unsafe fn save(
    folder: &Wide<32768>,
    path: &mut Wide<32768>,
    request: &Request,
    write_dump: WriteDump,
) -> Result<(), (u32, bool)> {
    let mut now: SYSTEMTIME = core::mem::zeroed();
    GetLocalTime(&mut now);
    for _ in 0..16 {
        path.truncate(0);
        path.wide(&folder.buf[..folder.len])
            .map_err(|_| (ERROR_FILENAME_EXCED_RANGE, false))?;
        write!(
            path,
            "SRP-crash-{:04}{:02}{:02}-{:02}{:02}{:02}-{:03}-p{}-t{}-{}.dmp",
            now.wYear,
            now.wMonth,
            now.wDay,
            now.wHour,
            now.wMinute,
            now.wSecond,
            now.wMilliseconds,
            GetCurrentProcessId(),
            request.exception.ThreadId,
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        )
        .map_err(|_| (ERROR_FILENAME_EXCED_RANGE, false))?;
        let file = CreateFileW(
            path.ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ,
            null(),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        );
        if file == INVALID_HANDLE_VALUE {
            let e = GetLastError();
            if e == ERROR_FILE_EXISTS || e == ERROR_ALREADY_EXISTS {
                continue;
            }
            return Err((e, false));
        }
        let exception = if request.exception.ExceptionPointers.is_null() {
            null()
        } else {
            &request.exception
        };
        // Local capture cannot be disabled by the game's MiniDumpNoDump setting.
        let message = api::USER_MESSAGE.get();
        let mut len = 0;
        while len < 32767 && *message.add(len) != 0 {
            len += 1;
        }
        let mut comment = MINIDUMP_USER_STREAM {
            Type: CommentStreamA as u32,
            BufferSize: (len + 1) as u32,
            Buffer: message.cast_mut().cast(),
        };
        let streams = MINIDUMP_USER_STREAM_INFORMATION {
            UserStreamCount: 1,
            UserStreamArray: &mut comment,
        };
        let ok = write_dump(
            GetCurrentProcess(),
            GetCurrentProcessId(),
            file,
            MiniDumpWithDataSegs,
            exception,
            &streams,
            null(),
        );
        let mut error = if ok == 0 { GetLastError() } else { 0 };
        if ok != 0 {
            // Some Wine versions silently omit all modules under restrictive ptrace
            // policy. Populate only an empty standard module stream, using self reads.
            if let Err(e) = crate::modules::ensure_modules(file) {
                error = e;
            }
            if FlushFileBuffers(file) == 0 {
                error = GetLastError();
            }
        }
        CloseHandle(file);
        if ok == 0 {
            DeleteFileW(path.ptr());
            return Err((error, false));
        }
        // Keep a successful primary dump even if supplemental module collection fails.
        if error != 0 {
            return Err((error, true));
        }
        return Ok(());
    }
    Err((ERROR_FILE_EXISTS, false))
}

unsafe extern "system" fn worker(parameter: *mut c_void) -> u32 {
    WORKER.store(GetCurrentThreadId(), Ordering::Release);
    let request = &mut *parameter.cast::<Request>();
    let mut dir = folder();
    let library = dbghelp(&dir);
    let mut path = Wide::<32768>::new();
    let mut error = ERROR_MOD_NOT_FOUND;
    let mut partial = false;
    if !library.is_null() {
        if let Some(proc) = GetProcAddress(library, c"MiniDumpWriteDump".as_ptr().cast()) {
            let write_dump: WriteDump = core::mem::transmute(proc);
            let primary = if dir.len != 0 {
                save(&dir, &mut path, request, write_dump)
            } else {
                Err((ERROR_PATH_NOT_FOUND, false))
            };
            match primary {
                Ok(()) => request.saved = true,
                Err((e, retained)) => {
                    error = e;
                    partial = retained;
                    if !partial {
                        let n = GetTempPathW(dir.buf.len() as u32, dir.buf.as_mut_ptr()) as usize;
                        if n > 0 && n < dir.buf.len() {
                            dir.len = n;
                            match save(&dir, &mut path, request, write_dump) {
                                Ok(()) => request.saved = true,
                                Err((e, retained)) => {
                                    error = e;
                                    partial = retained;
                                }
                            }
                        }
                    }
                }
            }
        } else {
            error = ERROR_PROC_NOT_FOUND;
        }
    }
    // Zip the dump only now that it is flushed and closed: on any failure, or
    // a crash or exit while zipping, the dump stays and the dialog names it.
    let zipped = (request.saved || partial) && crate::archive::zip(&mut path).is_ok();
    // Publish only after save() has flushed/closed its file and it is zipped
    // (or all attempts failed). Contending crashes need not wait for callbacks
    // or a modal dialog.
    if request.crash {
        PHASE.store(CRASH_CLOSED, Ordering::Release);
    }
    if !library.is_null() {
        FreeLibrary(library);
    }
    if request.crash {
        let overflow = !request.exception.ExceptionPointers.is_null()
            && (*(*request.exception.ExceptionPointers).ExceptionRecord).ExceptionCode
                == EXCEPTION_STACK_OVERFLOW;
        if !overflow {
            // Both callbacks and UI occur only after the persistent dump is closed.
            api::callback(&api::PRE, &api::PRE_PARAM);
            if api::ACTIVITY.load(Ordering::Relaxed) == 1 {
                crate::ui::show(&path, request.saved, partial, error, zipped);
            }
            api::callback(&api::POST, &api::POST_PARAM);
        }
    }
    if request.saved { 0 } else { error }
}

// The crash's text report: what happened and where, from the exception and
// context records, and how far the game got (its log's last lines). It is
// made even when the dump is not: written beside the dump (or to the
// temporary folder), put in the ZIP as `crash.txt`, shown in the dialog in
// short and copied from it for Discord. No game lock or heap is touched;
// every read of the game's memory goes through ReadProcessMemory, so a
// damaged pointer gives no line, not a second crash.
use crate::{
    details::{self, Buffer},
    text::Wide,
    zip::Source,
};
use core::{
    ffi::c_void,
    fmt::Write,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::*,
    System::{
        Diagnostics::Debug::*,
        LibraryLoader::*,
        Memory::*,
        SystemInformation::{GetLocalTime, OSVERSIONINFOW},
        Threading::GetCurrentProcess,
    },
};

/// Return addresses the stack scan lists, and how far up it looks.
const FRAMES: usize = 8;
const STACK_BYTES: usize = 256 * 1024;
/// The game log's tail: at most this many lines of its last bytes.
const LOG_LINES: usize = 20;
const LOG_BYTES: usize = 8 * 1024;
/// The Reporter line of a build without a revision. `release/build-zip`
/// refuses a DLL that holds the full-memory one.
#[cfg(not(feature = "full-memory-dumps"))]
const LOCAL_BUILD: &str = "local build";
#[cfg(feature = "full-memory-dumps")]
const LOCAL_BUILD: &str = "local build, full-memory dumps";

pub struct Report {
    /// The whole report; `text[..head]` is what "Copy" copies.
    pub text: Buffer<16384>,
    pub head: usize,
    /// The dialog's lines: the exception, and where.
    pub short: Buffer<512>,
}
impl Report {
    pub const fn new() -> Self {
        Self {
            text: Buffer::new(),
            head: 0,
            short: Buffer::new(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.text.len == 0
    }
}

unsafe fn read<T: Copy>(address: usize) -> Option<T> {
    let mut value: T = zeroed();
    let mut n = 0;
    (ReadProcessMemory(
        GetCurrentProcess(),
        address as *const c_void,
        (&mut value as *mut T).cast(),
        size_of::<T>(),
        &mut n,
    ) != 0
        && n == size_of::<T>())
    .then_some(value)
}

/// The loaded module holding `address`: its base, its file name in `name`.
unsafe fn module_of(address: usize, name: &mut Buffer<128>) -> Option<usize> {
    let mut module = null_mut();
    if GetModuleHandleExW(
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        address as *const u16,
        &mut module,
    ) == 0
    {
        return None;
    }
    let mut path = [0u16; 1024];
    let n = GetModuleFileNameW(module, path.as_mut_ptr(), path.len() as u32) as usize;
    let path = &path[..n.min(path.len())];
    let file = path
        .iter()
        .rposition(|&c| c == b'\\' as u16 || c == b'/' as u16)
        .map_or(path, |at| &path[at + 1..]);
    name.len = 0;
    for c in char::decode_utf16(file.iter().copied()) {
        let _ = name.write_char(c.unwrap_or('?'));
    }
    if name.len == 0 {
        let _ = name.write_str("?");
    }
    Some(module as usize)
}

unsafe fn location(w: &mut dyn Write, address: usize) {
    let mut name = Buffer::<128>::new();
    let module = module_of(address, &mut name);
    let _ = details::write_location(w, address, module.map(|base| (name.as_str(), base)));
}

/// The thrown type of an MSVC C++ exception (`.?AVbad_alloc@std@@`): the
/// first catchable type of the ThrowInfo its third parameter names.
unsafe fn cpp_type(record: &EXCEPTION_RECORD, name: &mut Buffer<128>) -> bool {
    if record.ExceptionCode as u32 != 0xE06D_7363
        || record.NumberParameters < 3
        || record.ExceptionInformation[0] != 0x1993_0520
    {
        return false;
    }
    let Some(types) = read::<u32>(record.ExceptionInformation[2] + 12) else {
        return false;
    };
    let Some(first) = read::<u32>(types as usize + 4) else {
        return false;
    };
    let Some(descriptor) = read::<u32>(first as usize + 4) else {
        return false;
    };
    let Some(bytes) = read::<[u8; 96]>(descriptor as usize + 8) else {
        return false;
    };
    for &b in bytes.iter().take_while(|&&b| b != 0) {
        if !(0x21..0x7f).contains(&b) || name.write_char(b as char).is_err() {
            return false;
        }
    }
    name.len > 0
}

/// Up to [`FRAMES`] values on the stack above `esp` that point just past a
/// `call` in a loaded module: return addresses, likely the callers.
unsafe fn stack(w: &mut dyn Write, esp: usize) -> usize {
    let mut info: MEMORY_BASIC_INFORMATION = zeroed();
    if VirtualQuery(
        esp as *const c_void,
        &mut info,
        size_of::<MEMORY_BASIC_INFORMATION>(),
    ) == 0
    {
        return 0;
    }
    // Whole dwords only: with an unaligned ESP the 256 KiB bound is unaligned,
    // and a last read of 1-3 bytes would not move the scan on.
    let end =
        (info.BaseAddress as usize + info.RegionSize).min(esp.saturating_add(STACK_BYTES)) & !3;
    // The last region asked about: [start, end) and whether it is image code.
    let mut code = (0usize, 0usize, false);
    let mut found = 0;
    // Stale copies of a return address further up say nothing new.
    let mut listed = [0usize; FRAMES];
    let mut at = esp & !3;
    let mut chunk = [0u32; 1024];
    'scan: while at < end {
        let want = (end - at).min(chunk.len() * 4);
        let mut n = 0;
        if ReadProcessMemory(
            GetCurrentProcess(),
            at as *const c_void,
            chunk.as_mut_ptr().cast(),
            want,
            &mut n,
        ) == 0
            || n < 4
        {
            break;
        }
        for &value in &chunk[..n / 4] {
            let value = value as usize;
            if value < 0x10000 {
                continue;
            }
            if !(code.0..code.1).contains(&value) {
                let mut region: MEMORY_BASIC_INFORMATION = zeroed();
                if VirtualQuery(
                    value as *const c_void,
                    &mut region,
                    size_of::<MEMORY_BASIC_INFORMATION>(),
                ) == 0
                {
                    continue;
                }
                let start = region.BaseAddress as usize;
                let executable = PAGE_EXECUTE
                    | PAGE_EXECUTE_READ
                    | PAGE_EXECUTE_READWRITE
                    | PAGE_EXECUTE_WRITECOPY;
                code = (
                    start,
                    start + region.RegionSize,
                    region.State == MEM_COMMIT
                        && region.Type == MEM_IMAGE
                        && region.Protect & executable != 0,
                );
            }
            if !code.2 {
                continue;
            }
            let Some(before) = read::<[u8; 8]>(value - 8) else {
                continue;
            };
            if listed[..found].contains(&value) || !details::follows_call(&before) {
                continue;
            }
            let _ = w.write_str(if found == 0 {
                "Stack:     "
            } else {
                "           "
            });
            location(w, value);
            let _ = w.write_str("\r\n");
            listed[found] = value;
            found += 1;
            if found == FRAMES {
                break 'scan;
            }
        }
        at += n & !3;
    }
    found
}

/// What the game exe says of its patch revision: the byte the sign-in
/// sends (`client/updater/logic/src/revision.rs`, at the same place in the
/// loaded image).
unsafe fn revision(w: &mut dyn Write) {
    const SITE_VA: usize = 0x0038_0862;
    let base = GetModuleHandleW(null()) as usize;
    let image_base = read::<u32>(base + 0x3C)
        .and_then(|pe| read::<u32>(base + pe as usize + 0x34))
        .map(|b| b as usize);
    let site = image_base
        .and_then(|b| SITE_VA.checked_sub(b))
        .and_then(|rva| read::<[u8; 6]>(base + rva));
    let _ = match site {
        Some([0xC6, 0x44, 0x24, 0x27, revision, 0x90]) => write!(w, "patch revision {revision}"),
        Some([0x8A, 0x0D, 0x9E, 0xAE, 0x9D, 0x00]) => w.write_str("unpatched"),
        _ => w.write_str("patch revision unknown"),
    };
}

/// The first 12 hex digits of the exe file's SHA-256.
unsafe fn exe_hash(w: &mut dyn Write) {
    let mut path = Wide::<1024>::new();
    path.len = GetModuleFileNameW(null_mut(), path.buf.as_mut_ptr(), 1024) as usize;
    if path.len == 0 || path.len >= 1024 {
        let _ = w.write_str("sha256 unavailable");
        return;
    }
    let file = CreateFileW(
        path.ptr(),
        GENERIC_READ,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        null(),
        OPEN_EXISTING,
        FILE_FLAG_SEQUENTIAL_SCAN,
        null_mut(),
    );
    if file == INVALID_HANDLE_VALUE {
        let _ = write!(w, "sha256 unavailable (error {})", GetLastError());
        return;
    }
    let mut hash = crate::sha256::Sha256::new();
    let mut chunk = [0u8; 65536];
    let mut total = 0u64;
    let error = loop {
        let mut n = 0;
        if ReadFile(
            file,
            chunk.as_mut_ptr(),
            chunk.len() as u32,
            &mut n,
            null_mut(),
        ) == 0
        {
            break GetLastError();
        }
        if n == 0 {
            break 0;
        }
        hash.update(&chunk[..n as usize]);
        total += n as u64;
        // A game exe is some tens of MB; do not hash without end.
        if total > 512 * 1024 * 1024 {
            break ERROR_FILE_TOO_LARGE;
        }
    };
    CloseHandle(file);
    if error != 0 {
        let _ = write!(w, "sha256 unavailable (error {error})");
        return;
    }
    let _ = w.write_str("sha256 ");
    for b in &hash.finish()[..6] {
        let _ = write!(w, "{b:02x}");
    }
}

/// `10.0.19045`, and Wine's version under Wine. RtlGetVersion tells the
/// truth where GetVersionEx says what the exe's manifest allows.
unsafe fn windows(w: &mut dyn Write) {
    let ntdll = GetModuleHandleW(windows_sys::w!("ntdll.dll"));
    let mut info: OSVERSIONINFOW = zeroed();
    info.dwOSVersionInfoSize = size_of::<OSVERSIONINFOW>() as u32;
    type GetVersion = unsafe extern "system" fn(*mut OSVERSIONINFOW) -> i32;
    type WineVersion = unsafe extern "C" fn() -> *const u8;
    let _ = match GetProcAddress(ntdll, c"RtlGetVersion".as_ptr().cast()) {
        Some(f)
            if core::mem::transmute::<unsafe extern "system" fn() -> isize, GetVersion>(f)(
                &mut info,
            ) == 0 =>
        {
            write!(
                w,
                "{}.{}.{}",
                info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber
            )
        }
        _ => w.write_str("unknown"),
    };
    if let Some(f) = GetProcAddress(ntdll, c"wine_get_version".as_ptr().cast()) {
        let version =
            core::mem::transmute::<unsafe extern "system" fn() -> isize, WineVersion>(f)();
        if !version.is_null() {
            let _ = w.write_str(" (Wine ");
            for i in 0..32 {
                let b = *version.add(i);
                if b == 0 || !(0x20..0x7f).contains(&b) {
                    break;
                }
                let _ = w.write_char(b as char);
            }
            let _ = w.write_str(")");
        }
    }
}

/// The lines that do not depend on the dump.
pub unsafe fn collect(report: &mut Report, exception: *const EXCEPTION_POINTERS, thread: u32) {
    if exception.is_null() || (*exception).ExceptionRecord.is_null() {
        return;
    }
    let record = &*(*exception).ExceptionRecord;
    let context = (*exception).ContextRecord;
    let params = &record.ExceptionInformation[..(record.NumberParameters as usize).min(15)];
    let mut cpp = Buffer::<128>::new();
    let cpp = cpp_type(record, &mut cpp).then(|| cpp.as_str().trim_end());
    let address = record.ExceptionAddress as usize;
    let short = &mut report.short;
    let _ = details::write_exception(short, record.ExceptionCode as u32, params, cpp);
    let _ = short.write_str("\r\nat ");
    location(short, address);

    let w = &mut report.text;
    let mut now: SYSTEMTIME = zeroed();
    GetLocalTime(&mut now);
    let _ = write!(
        w,
        "Skhid crash report, {:04}-{:02}-{:02} {:02}:{:02}:{:02}\r\nException: ",
        now.wYear, now.wMonth, now.wDay, now.wHour, now.wMinute, now.wSecond
    );
    let _ = details::write_exception(w, record.ExceptionCode as u32, params, cpp);
    let _ = w.write_str("\r\nAddress:   ");
    location(w, address);
    let _ = write!(w, " (0x{address:08X})\r\nThread:    {thread}\r\n");
    if !context.is_null() {
        let c = &*context;
        let _ = write!(
            w,
            "Registers: EAX={:08X} EBX={:08X} ECX={:08X} EDX={:08X} ESI={:08X} EDI={:08X}\r\n           EBP={:08X} ESP={:08X} EIP={:08X} EFL={:08X}\r\n",
            c.Eax, c.Ebx, c.Ecx, c.Edx, c.Esi, c.Edi, c.Ebp, c.Esp, c.Eip, c.EFlags
        );
        if stack(w, c.Esp as usize) == 0 {
            let _ = w.write_str("Stack:     no return addresses found\r\n");
        }
    }
    let _ = w.write_str("Game:      ");
    let mut exe = Buffer::<128>::new();
    if module_of(GetModuleHandleW(null()) as usize, &mut exe).is_some() {
        let _ = write!(w, "{}, ", exe.as_str());
    }
    revision(w);
    let _ = w.write_str(", ");
    exe_hash(w);
    let _ = write!(
        w,
        "\r\nReporter:  {}\r\nWindows:   ",
        match option_env!("SKHID_REVISION") {
            Some(r) if !r.is_empty() => r,
            _ => LOCAL_BUILD,
        }
    );
    windows(w);
    let _ = w.write_str("\r\n");
    // Non-ASCII folders (a Cyrillic user name): the paths themselves stay
    // on the player's PC.
    let folder = crate::dump::folder();
    let mut temp = [0u16; 1024];
    let n = GetTempPathW(temp.len() as u32, temp.as_mut_ptr()) as usize;
    let temp = &temp[..n.min(temp.len())];
    let game = details::needs_simple_path(&folder.buf[..folder.len]);
    let in_temp = details::needs_simple_path(temp);
    if game || in_temp {
        let _ = write!(
            w,
            "Paths:     non-ASCII letters in {}",
            match (game, in_temp) {
                (true, true) => "the game folder and the temporary folder",
                (true, false) => "the game folder",
                _ => "the temporary folder",
            }
        );
        // Beyond the ANSI code page, the game's ANSI calls (GetModuleFileNameA,
        // CreateFileA, SHGetFolderPathA) cannot even name the folder.
        if outside_ansi(&folder.buf[..folder.len]) || outside_ansi(temp) {
            let _ = w.write_str(", outside the ANSI code page");
        }
        let _ = w.write_str("\r\n");
    }
}

/// Whether `path` has a character the ANSI code page cannot hold.
unsafe fn outside_ansi(path: &[u16]) -> bool {
    use windows_sys::Win32::Globalization::{CP_ACP, WC_NO_BEST_FIT_CHARS, WideCharToMultiByte};
    if path.is_empty() {
        return false;
    }
    let mut bytes = [0u8; 4096];
    let mut lost = 0;
    let n = WideCharToMultiByte(
        CP_ACP,
        WC_NO_BEST_FIT_CHARS,
        path.as_ptr(),
        path.len() as i32,
        bytes.as_mut_ptr(),
        bytes.len() as i32,
        null(),
        &mut lost,
    );
    n == 0 || lost != 0
}

struct Bytes<'a>(&'a [u8]);
impl Source for Bytes<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, u32> {
        let n = buf.len().min(self.0.len());
        buf[..n].copy_from_slice(&self.0[..n]);
        self.0 = &self.0[n..];
        Ok(n)
    }
}

/// The newest `Survarium*.log` in Documents\Survarium: its last lines,
/// without credential-bearing records (`sanitize.rs`). Its name holds the
/// player's account name, so it is not written.
unsafe fn log_tail(w: &mut dyn Write) {
    let folders = crate::logs::folders();
    let docs = &folders[0];
    let mut pattern = Wide::<1100>::new();
    if docs.len == 0
        || pattern.wide(&docs.buf[..docs.len]).is_err()
        || write!(pattern, "Survarium*.log").is_err()
    {
        let _ = w.write_str("Game log:  no Documents folder\r\n");
        return;
    }
    let mut found: WIN32_FIND_DATAW = zeroed();
    let find = FindFirstFileW(pattern.ptr(), &mut found);
    if find == INVALID_HANDLE_VALUE {
        let _ = write!(w, "Game log:  none found (error {})\r\n", GetLastError());
        return;
    }
    let mut newest = [0u16; 260];
    let mut newest_time = 0u64;
    loop {
        let time = (found.ftLastWriteTime.dwHighDateTime as u64) << 32
            | found.ftLastWriteTime.dwLowDateTime as u64;
        if found.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) == 0
            && time >= newest_time
        {
            newest_time = time;
            newest = found.cFileName;
        }
        if FindNextFileW(find, &mut found) == 0 {
            break;
        }
    }
    FindClose(find);
    let name_len = newest.iter().position(|&c| c == 0).unwrap_or(0);
    let mut path = Wide::<1400>::new();
    if name_len == 0
        || path.wide(&docs.buf[..docs.len]).is_err()
        || path.wide(&newest[..name_len]).is_err()
    {
        let _ = w.write_str("Game log:  none found\r\n");
        return;
    }
    let file = CreateFileW(
        path.ptr(),
        GENERIC_READ,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        null(),
        OPEN_EXISTING,
        FILE_FLAG_OPEN_REPARSE_POINT,
        null_mut(),
    );
    if file == INVALID_HANDLE_VALUE {
        let _ = write!(w, "Game log:  unreadable (error {})\r\n", GetLastError());
        return;
    }
    let mut size = 0i64;
    let mut tail = [0u8; LOG_BYTES];
    let mut n = 0;
    let from = if GetFileSizeEx(file, &mut size) != 0 {
        (size as u64).saturating_sub(LOG_BYTES as u64)
    } else {
        u64::MAX
    };
    let ok = from != u64::MAX
        && SetFilePointerEx(file, from as i64, null_mut(), FILE_BEGIN) != 0
        && ReadFile(
            file,
            tail.as_mut_ptr(),
            LOG_BYTES as u32,
            &mut n,
            null_mut(),
        ) != 0;
    let error = GetLastError();
    CloseHandle(file);
    if !ok {
        let _ = write!(w, "Game log:  unreadable (error {error})\r\n");
        return;
    }
    let tail = &mut tail[..n as usize];
    // Non-UTF-8 text (a legacy code page) would make the filter refuse
    // the whole tail; its non-ASCII letters are not needed here.
    if core::str::from_utf8(tail).is_err() {
        for b in tail.iter_mut().filter(|b| **b >= 0x80) {
            *b = b'?';
        }
    }
    // From a record's first line: the cut may fall inside the continuation
    // lines of a sensitive record, which a fresh filter would let through.
    let lines = crate::sanitize::from_record(details::last_lines(tail, LOG_LINES, from == 0));
    let _ = write!(
        w,
        "Game log:  last {LOG_LINES} lines of the newest Survarium*.log in Documents\\Survarium\r\n"
    );
    let mut source = Bytes(lines);
    let mut filter = crate::sanitize::Filter::new(&mut source);
    let mut out = [0u8; 1024];
    loop {
        match filter.read(&mut out) {
            Ok(0) => break,
            Ok(n) => {
                for chunk in out[..n].utf8_chunks() {
                    if w.write_str(chunk.valid()).is_err() {
                        return;
                    }
                }
            }
            Err(e) => {
                let _ = write!(w, "[log tail omitted: not plain text (0x{e:08X})]\r\n");
                return;
            }
        }
    }
}

/// The dump's line, then the game log's tail.
pub unsafe fn finish(report: &mut Report, saved: bool, partial: bool, error: u32, capture: bool) {
    if report.is_empty() {
        return;
    }
    let w = &mut report.text;
    let _ = w.write_str("Dump:      ");
    let _ = if saved {
        w.write_str("saved")
    } else if partial {
        w.write_str("saved, not fully checked, error ")
    } else if details::failure(capture, error) == details::Failure::Capture {
        w.write_str("not saved, the capture failed, error ")
    } else {
        w.write_str("not saved, the file could not be written, error ")
    };
    if !saved {
        let _ = details::write_code(w, error);
    }
    let _ = w.write_str("\r\n");
    report.head = w.len;
    log_tail(w);
}

/// Writes the report. `kept`: `path` names a kept dump, and the report goes
/// beside it, its `.dmp` made `.txt` (`path` is left as it was). Otherwise
/// a new `SRP-crash-….txt` beside the exe or in the temporary folder, and
/// `path` names it (or nothing, when neither takes it).
pub unsafe fn save(report: &Report, kept: bool, path: &mut Wide<32768>, thread: u32) -> bool {
    let bytes = &report.text.buf[..report.text.len];
    if kept {
        let n = path.len;
        if report.is_empty()
            || n < 4
            || !path.buf[n - 4..n]
                .iter()
                .copied()
                .eq(b".dmp".iter().map(|&b| u16::from(b)))
        {
            return false;
        }
        path.buf[n - 3..n].copy_from_slice(&[b't' as u16, b'x' as u16, b't' as u16]);
        let ok = write_new(path, bytes);
        path.buf[n - 3..n].copy_from_slice(&[b'd' as u16, b'm' as u16, b'p' as u16]);
        return ok;
    }
    path.truncate(0);
    if report.is_empty() {
        return false;
    }
    let out = path;
    let mut folder = crate::dump::folder();
    for attempt in 0..2 {
        if attempt == 1 {
            let n = GetTempPathW(folder.buf.len() as u32, folder.buf.as_mut_ptr()) as usize;
            if n == 0 || n >= folder.buf.len() {
                return false;
            }
            folder.len = n;
        }
        if folder.len == 0 {
            continue;
        }
        for _ in 0..16 {
            if crate::dump::name(&folder, out, thread, "txt").is_err() {
                break;
            }
            if write_new(out, bytes) {
                return true;
            }
            if !matches!(GetLastError(), ERROR_FILE_EXISTS | ERROR_ALREADY_EXISTS) {
                break;
            }
        }
    }
    out.truncate(0);
    false
}

unsafe fn write_new(path: &Wide<32768>, bytes: &[u8]) -> bool {
    let file = CreateFileW(
        path.ptr(),
        GENERIC_WRITE,
        0,
        null(),
        CREATE_NEW,
        FILE_ATTRIBUTE_NORMAL,
        null_mut(),
    );
    if file == INVALID_HANDLE_VALUE {
        return false;
    }
    let mut n = 0;
    let ok = WriteFile(file, bytes.as_ptr(), bytes.len() as u32, &mut n, null_mut()) != 0
        && n as usize == bytes.len();
    let error = GetLastError();
    // Best effort: a text written but not flushed is still worth keeping.
    FlushFileBuffers(file);
    CloseHandle(file);
    if !ok {
        DeleteFileW(path.ptr());
        SetLastError(error);
    }
    ok
}

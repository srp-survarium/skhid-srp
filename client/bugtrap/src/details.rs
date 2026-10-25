// The crash's details as text, independent of Win32 and allocation: the
// exception code and its name, an address as module+offset, the choice of
// the dump-failure message. `report.rs` gathers the values; the tests
// (`tests/text_tests.rs`) check the formatting.
use core::fmt::{self, Write};

/// The Windows name of an exception code, without its `EXCEPTION_` /
/// `STATUS_` prefix.
pub fn exception_name(code: u32) -> Option<&'static str> {
    Some(match code {
        0x8000_0001 => "GUARD_PAGE",
        0x8000_0002 => "DATATYPE_MISALIGNMENT",
        0x8000_0003 => "BREAKPOINT",
        0x8000_0004 => "SINGLE_STEP",
        0xC000_0005 => "ACCESS_VIOLATION",
        0xC000_0006 => "IN_PAGE_ERROR",
        0xC000_0008 => "INVALID_HANDLE",
        0xC000_0017 => "NO_MEMORY",
        0xC000_001D => "ILLEGAL_INSTRUCTION",
        0xC000_0025 => "NONCONTINUABLE_EXCEPTION",
        0xC000_0026 => "INVALID_DISPOSITION",
        0xC000_008C => "ARRAY_BOUNDS_EXCEEDED",
        0xC000_008D => "FLT_DENORMAL_OPERAND",
        0xC000_008E => "FLT_DIVIDE_BY_ZERO",
        0xC000_008F => "FLT_INEXACT_RESULT",
        0xC000_0090 => "FLT_INVALID_OPERATION",
        0xC000_0091 => "FLT_OVERFLOW",
        0xC000_0092 => "FLT_STACK_CHECK",
        0xC000_0093 => "FLT_UNDERFLOW",
        0xC000_0094 => "INT_DIVIDE_BY_ZERO",
        0xC000_0095 => "INT_OVERFLOW",
        0xC000_0096 => "PRIV_INSTRUCTION",
        0xC000_00FD => "STACK_OVERFLOW",
        0xC000_0135 => "DLL_NOT_FOUND",
        0xC000_0142 => "DLL_INIT_FAILED",
        0xC000_0374 => "HEAP_CORRUPTION",
        0xC000_0409 => "STACK_BUFFER_OVERRUN",
        0xC000_0417 => "INVALID_CRUNTIME_PARAMETER",
        0xE06D_7363 => "C++ exception",
        0xE000_0001 => "crash reporter panic",
        _ => return None,
    })
}

/// `0xC0000005 ACCESS_VIOLATION (write to 0x00000000)`: the code, its name
/// and what the exception's parameters say. `cpp_type` is the thrown C++
/// type's decorated name, when known.
pub fn write_exception(
    w: &mut dyn Write,
    code: u32,
    params: &[usize],
    cpp_type: Option<&str>,
) -> fmt::Result {
    write!(
        w,
        "0x{code:08X} {}",
        exception_name(code).unwrap_or("unknown")
    )?;
    match code {
        // [0]: 0 read, 1 write, 8 execute (DEP); [1]: the address touched.
        0xC000_0005 | 0xC000_0006 if params.len() >= 2 => {
            let access = match params[0] {
                0 => "read of",
                1 => "write to",
                8 => "execution of",
                _ => "access to",
            };
            write!(w, " ({access} 0x{:08X}", params[1])?;
            if code == 0xC000_0006 && params.len() >= 3 {
                write!(w, ", status 0x{:08X}", params[2])?;
            }
            w.write_str(")")
        }
        0xE06D_7363 => match cpp_type {
            Some(name) => write!(w, " ({name})"),
            None => Ok(()),
        },
        _ => Ok(()),
    }
}

/// `module+0x1234`, or the bare address when no module holds it.
pub fn write_location(
    w: &mut dyn Write,
    address: usize,
    module: Option<(&str, usize)>,
) -> fmt::Result {
    match module {
        Some((name, base)) if address >= base => write!(w, "{name}+0x{:X}", address - base),
        _ => write!(w, "0x{address:08X} (not in a module)"),
    }
}

/// A Windows error code as people search for it: an HRESULT or NTSTATUS in
/// hex (`0x8007001F`), a plain Win32 error in decimal (`5`).
pub fn write_code(w: &mut dyn Write, code: u32) -> fmt::Result {
    if code > 0xFFFF {
        write!(w, "0x{code:08X}")
    } else {
        write!(w, "{code}")
    }
}

/// Why no dump was saved, for the dialog's message.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Failure {
    /// The dump file could not be created or written (folder, permission,
    /// disk full): the old message, both folders were tried.
    File,
    /// The file was there, but DbgHelp could not read the game's memory:
    /// an antivirus or Windows' exploit protection stops it, usually with
    /// `HRESULT_FROM_WIN32(ERROR_GEN_FAILURE)` (0x8007001F),
    /// `ERROR_PARTIAL_COPY` (0x8007012B) or access denied.
    Capture,
}

/// `capture`: the error came from `MiniDumpWriteDump` (the file was
/// created), not from creating the file.
pub fn failure(capture: bool, code: u32) -> Failure {
    // The dump's own writes failing for want of space is the folder's fault.
    const DISK_FULL: [u32; 4] = [112, 39, 0x8007_0070, 0x8007_0027];
    if capture && !DISK_FULL.contains(&code) {
        Failure::Capture
    } else {
        Failure::File
    }
}

/// Whether the bytes just before a return address end in a `call`
/// (`before[7]` is the byte right before it): `E8 rel32`, or `FF /2` with a
/// register, `[reg]`, `[reg+disp8]`, `[reg+disp32]` or `[disp32]` operand.
/// It keeps a stack scan to return addresses rather than any pointer into code.
pub fn follows_call(before: &[u8; 8]) -> bool {
    let call_rm = |at: usize, len: usize| {
        let modrm = before[at + 1];
        if before[at] != 0xFF || (modrm >> 3) & 7 != 2 {
            return false;
        }
        let mode = modrm >> 6;
        let rm = modrm & 7;
        let sib = usize::from(mode != 3 && rm == 4);
        let disp = match mode {
            0 if rm == 5 => 4,
            0 if rm == 4 && before.get(at + 2).is_some_and(|s| s & 7 == 5) => 4,
            1 => 1,
            2 => 4,
            _ => 0,
        };
        2 + sib + disp == len
    };
    before[3] == 0xE8 || (2..=7).any(|len| call_rm(8 - len, len))
}

/// The last `lines` lines of `text` (a file's tail), from the start of a
/// whole line: a first line cut by the read is dropped unless `whole` (the
/// read began at the file's start).
pub fn last_lines(text: &[u8], lines: usize, whole: bool) -> &[u8] {
    let mut body = text;
    if !whole {
        body = match body.iter().position(|&b| b == b'\n') {
            Some(at) => &body[at + 1..],
            None => &[],
        };
    }
    let trimmed = body.strip_suffix(b"\n").unwrap_or(body);
    let trimmed = trimmed.strip_suffix(b"\r").unwrap_or(trimmed);
    let mut start = trimmed.len();
    let mut seen = 0;
    while start > 0 {
        if trimmed[start - 1] == b'\n' {
            seen += 1;
            if seen == lines {
                break;
            }
        }
        start -= 1;
    }
    if lines == 0 { &[] } else { &body[start..] }
}

/// A bounded UTF-8 buffer: what does not fit is dropped, whole characters.
pub struct Buffer<const N: usize> {
    pub buf: [u8; N],
    pub len: usize,
}
impl<const N: usize> Buffer<N> {
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }
    pub fn as_str(&self) -> &str {
        // Only whole `str`s and characters are ever written.
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}
impl<const N: usize> Write for Buffer<N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            let mut bytes = [0; 4];
            let bytes = c.encode_utf8(&mut bytes).as_bytes();
            if self.len + bytes.len() > N {
                return Err(fmt::Error);
            }
            self.buf[self.len..self.len + bytes.len()].copy_from_slice(bytes);
            self.len += bytes.len();
        }
        Ok(())
    }
}

/// Whether a folder has letters beyond ASCII: the 2015 game reads its
/// files through ANSI calls, which such a folder can break.
pub fn needs_simple_path(path: &[u16]) -> bool {
    path.iter().any(|&c| c > 0x7F)
}

/// `path` for one line of the dialog: at most `max` characters, the middle
/// left out (`C:\…\Survarium\game\binaries\x86\`).
pub fn write_short_path(w: &mut dyn Write, path: &[u16], max: usize) -> fmt::Result {
    let chars = || char::decode_utf16(path.iter().copied()).map(|c| c.unwrap_or('?'));
    let count = chars().count();
    if count <= max {
        return chars().try_for_each(|c| w.write_char(c));
    }
    let head = 3.min(max);
    let tail = max.saturating_sub(head + 1);
    chars().take(head).try_for_each(|c| w.write_char(c))?;
    w.write_char('…')?;
    chars().skip(count - tail).try_for_each(|c| w.write_char(c))
}

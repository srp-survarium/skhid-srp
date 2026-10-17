// Zips a saved dump beside itself: SRP-crash-…-N.dmp becomes SRP-crash-…-N.zip
// holding that one file. Runs only after the dump is flushed and closed, so a
// failure, a crash or the end of the process while zipping leaves the dump.
use crate::{
    text::Wide,
    zip::{self, Entry, Memory, Sink, Source},
};
use core::{
    fmt::Write,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::*,
    System::{Memory::*, SystemInformation::*},
};

/// Zipping that takes longer than this (a very slow disk or PC) gives up
/// and keeps the dump: the player is waiting for the dialog.
const LIMIT_MS: u64 = 60_000;

struct File {
    handle: HANDLE,
    deadline: u64,
    /// Bytes `read` may still give.
    left: u64,
}
impl Source for File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, u32> {
        unsafe {
            if GetTickCount64() > self.deadline {
                return Err(ERROR_TIMEOUT);
            }
            let want = (buf.len() as u64).min(self.left) as u32;
            if want == 0 {
                return Ok(0);
            }
            let mut n = 0;
            if ReadFile(self.handle, buf.as_mut_ptr(), want, &mut n, null_mut()) == 0 {
                return Err(GetLastError());
            }
            self.left -= n as u64;
            Ok(n as usize)
        }
    }
}
impl Sink for File {
    fn write(&mut self, mut data: &[u8]) -> Result<(), u32> {
        while !data.is_empty() {
            let mut n = 0;
            unsafe {
                if WriteFile(
                    self.handle,
                    data.as_ptr(),
                    data.len() as u32,
                    &mut n,
                    null_mut(),
                ) == 0
                {
                    return Err(GetLastError());
                }
            }
            if n == 0 {
                return Err(ERROR_WRITE_FAULT);
            }
            data = &data[n as usize..];
        }
        Ok(())
    }
}
impl File {
    fn exact(&mut self, buf: &mut [u8]) -> Result<(), u32> {
        let mut at = 0;
        while at < buf.len() {
            match self.read(&mut buf[at..])? {
                0 => return Err(ERROR_HANDLE_EOF),
                n => at += n,
            }
        }
        Ok(())
    }
    unsafe fn seek(&mut self, to: i64) -> Result<(), u32> {
        if SetFilePointerEx(self.handle, to, null_mut(), FILE_BEGIN) == 0 {
            return Err(GetLastError());
        }
        Ok(())
    }
    unsafe fn size(&self) -> Result<u64, u32> {
        let mut size = 0;
        if GetFileSizeEx(self.handle, &mut size) == 0 {
            return Err(GetLastError());
        }
        Ok(size as u64)
    }
}

/// On success `path` names the ZIP and the dump is gone (or, if Windows
/// refused to delete it, still there beside it).
pub unsafe fn zip(path: &mut Wide<32768>) -> Result<(), u32> {
    let deadline = GetTickCount64() + LIMIT_MS;
    let len = path.len;
    let split = path.buf[..len]
        .iter()
        .rposition(|&c| c == b'\\' as u16 || c == b'/' as u16)
        .map_or(0, |i| i + 1);
    // The dump's own name, inside the ZIP, in plain ASCII.
    let mut name = [0u8; zip::MAX_NAME];
    let name_len = len - split;
    if name_len > name.len()
        || name_len < 5
        || path.buf[split..len]
            .iter()
            .any(|&c| !(0x20..0x7f).contains(&c))
    {
        return Err(ERROR_INVALID_NAME);
    }
    for (b, &c) in name.iter_mut().zip(&path.buf[split..len]) {
        *b = c as u8;
    }
    if !name[name_len - 4..name_len].eq_ignore_ascii_case(b".dmp") {
        return Err(ERROR_INVALID_NAME);
    }
    let mut part = Wide::<32768>::new();
    let mut target = Wide::<32768>::new();
    for (w, suffix) in [(&mut part, ".zip.part"), (&mut target, ".zip")] {
        w.wide(&path.buf[..len - 4])
            .map_err(|_| ERROR_FILENAME_EXCED_RANGE)?;
        write!(w, "{suffix}").map_err(|_| ERROR_FILENAME_EXCED_RANGE)?;
    }
    let dump = CreateFileW(
        path.ptr(),
        GENERIC_READ,
        FILE_SHARE_READ,
        null(),
        OPEN_EXISTING,
        FILE_FLAG_SEQUENTIAL_SCAN,
        null_mut(),
    );
    if dump == INVALID_HANDLE_VALUE {
        return Err(GetLastError());
    }
    // Fresh pages, not the game's heap, which the crash may have damaged.
    let memory = VirtualAlloc(
        null(),
        zip::MEMORY_WORDS * 4,
        MEM_COMMIT | MEM_RESERVE,
        PAGE_READWRITE,
    );
    let mut result = Err(ERROR_NOT_ENOUGH_MEMORY);
    let mut created = false;
    if !memory.is_null() {
        // Never overwrite: a leftover file of this name makes this fail.
        let out = CreateFileW(
            part.ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            null(),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            null_mut(),
        );
        if out == INVALID_HANDLE_VALUE {
            result = Err(GetLastError());
        } else {
            created = true;
            let words = core::slice::from_raw_parts_mut(memory.cast::<u32>(), zip::MEMORY_WORDS);
            result = write(
                &mut File {
                    handle: dump,
                    deadline,
                    left: u64::MAX,
                },
                &mut File {
                    handle: out,
                    deadline,
                    left: 0,
                },
                &name[..name_len],
                words,
            );
            CloseHandle(out);
        }
        VirtualFree(memory, 0, MEM_RELEASE);
    }
    CloseHandle(dump);
    if result.is_ok() && MoveFileExW(part.ptr(), target.ptr(), MOVEFILE_WRITE_THROUGH) == 0 {
        result = Err(GetLastError());
    }
    if let Err(e) = result {
        if created {
            DeleteFileW(part.ptr());
        }
        return Err(e);
    }
    DeleteFileW(path.ptr());
    path.truncate(0);
    let _ = path.wide(&target.buf[..target.len]);
    Ok(())
}

unsafe fn write(
    dump: &mut File,
    out: &mut File,
    name: &[u8],
    words: &mut [u32],
) -> Result<(), u32> {
    let size = dump.size()?;
    if size > zip::MAX_INPUT {
        return Err(zip::ERROR_FILE_TOO_LARGE);
    }
    dump.left = size;
    let mut now = core::mem::zeroed();
    GetLocalTime(&mut now);
    let (time, date) = zip::dos_time(
        now.wYear,
        now.wMonth,
        now.wDay,
        now.wHour,
        now.wMinute,
        now.wSecond,
    );
    let mut entry = Entry {
        name,
        time,
        date,
        crc: 0,
        packed: 0,
        size: 0,
    };
    let mut local = [0u8; zip::LOCAL_MAX];
    let n = zip::local_header(&entry, &mut local);
    // A placeholder; the sizes and the CRC are known at the end.
    out.write(&local[..n])?;
    let sum = zip::deflate(dump, out, &mut Memory::new(words))?;
    if sum.size != size || n as u64 + sum.packed + zip::TAIL_MAX as u64 > u32::MAX as u64 {
        return Err(zip::ERROR_FILE_TOO_LARGE);
    }
    entry.crc = sum.crc;
    entry.packed = sum.packed as u32;
    entry.size = sum.size as u32;
    let mut tail = [0u8; zip::TAIL_MAX];
    let m = zip::tail(&entry, n as u32 + entry.packed, &mut tail);
    out.write(&tail[..m])?;
    out.seek(0)?;
    zip::local_header(&entry, &mut local);
    out.write(&local[..n])?;
    if FlushFileBuffers(out.handle) == 0 {
        return Err(GetLastError());
    }
    // Read it all back: the records must be the ones meant, and the data must
    // give the dump's CRC-32 and size. Only then may the dump go.
    if out.size()? != n as u64 + sum.packed + m as u64 {
        return Err(ERROR_INVALID_DATA);
    }
    let mut back = [0u8; zip::TAIL_MAX];
    out.seek(0)?;
    out.left = n as u64;
    out.exact(&mut back[..n])?;
    if back[..n] != local[..n] {
        return Err(ERROR_INVALID_DATA);
    }
    out.left = sum.packed;
    if zip::check(out, &mut Memory::new(words))? != (sum.crc, sum.size) {
        return Err(ERROR_INVALID_DATA);
    }
    out.left = m as u64 + 1;
    out.exact(&mut back[..m])?;
    if back[..m] != tail[..m] || out.read(&mut back[..1])? != 0 {
        return Err(ERROR_INVALID_DATA);
    }
    Ok(())
}

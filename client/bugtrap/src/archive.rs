// Zips a saved dump beside itself: SRP-crash-…-N.dmp becomes SRP-crash-…-N.zip
// holding the dump and recent game logs. Runs after the dump is flushed and
// closed, so failure or process termination while zipping leaves the dump.
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

struct Bytes<'a>(&'a [u8]);
impl Source for Bytes<'_> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, u32> {
        let n = buf.len().min(self.0.len());
        buf[..n].copy_from_slice(&self.0[..n]);
        self.0 = &self.0[n..];
        Ok(n)
    }
}

// Own metadata stays on the report worker's reserved stack, never the game heap.
struct Record {
    name: [u8; zip::MAX_NAME],
    len: usize,
    time: u16,
    date: u16,
    crc: u32,
    packed: u32,
    size: u32,
    offset: u32,
}
impl Record {
    const EMPTY: Self = Self {
        name: [0; zip::MAX_NAME],
        len: 0,
        time: 0,
        date: 0,
        crc: 0,
        packed: 0,
        size: 0,
        offset: 0,
    };
    fn entry(&self) -> Entry<'_> {
        Entry {
            name: &self.name[..self.len],
            time: self.time,
            date: self.date,
            crc: self.crc,
            packed: self.packed,
            size: self.size,
        }
    }
}
// Distinct from OS I/O errors so the summary explains why this source was omitted.
const LOG_ZIP_LIMIT: u32 = 0xE000_0101;
struct LimitedSink<'a> {
    file: &'a mut File,
    left: u64,
    deadline: u64,
}
impl Sink for LimitedSink<'_> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), u32> {
        // Check BEFORE handing a block to WriteFile; an incompressible source
        // cannot temporarily grow the archive past the compressed budget.
        if bytes.len() as u64 > self.left {
            return Err(LOG_ZIP_LIMIT);
        }
        if unsafe { GetTickCount64() } > self.deadline {
            return Err(ERROR_TIMEOUT);
        }
        self.file.write(bytes)?;
        self.left -= bytes.len() as u64;
        Ok(())
    }
}

unsafe fn append(
    src: &mut dyn Source,
    size: u64,
    out: &mut File,
    record: &mut Record,
    offset: u32,
    words: &mut [u32],
    budget: Option<(u64, u64)>,
) -> Result<u32, u32> {
    record.offset = offset;
    out.seek(offset as i64)?;
    let mut local = [0; zip::LOCAL_MAX];
    let n = zip::local_header(&record.entry(), &mut local);
    let mut central = [0; zip::TAIL_MAX];
    let overhead = n as u64 + zip::central(&record.entry(), offset, &mut central) as u64;
    let (limit, deadline) = budget.unwrap_or((u64::MAX, out.deadline));
    if overhead > limit {
        return Err(LOG_ZIP_LIMIT);
    }
    out.write(&local[..n])?;
    let sum = zip::deflate(
        src,
        &mut LimitedSink {
            file: out,
            left: limit - overhead,
            deadline,
        },
        &mut Memory::new(words),
    )?;
    let end = offset as u64 + n as u64 + sum.packed;
    if sum.size != size {
        return Err(ERROR_HANDLE_EOF);
    }
    if end + (crate::logs::MAX_MEMBERS * zip::TAIL_MAX) as u64 > u32::MAX as u64 {
        return Err(zip::ERROR_FILE_TOO_LARGE);
    }
    record.crc = sum.crc;
    record.packed = sum.packed as u32;
    record.size = sum.size as u32;
    out.seek(offset as i64)?;
    zip::local_header(&record.entry(), &mut local);
    out.write(&local[..n])?;
    out.seek(end as i64)?;
    Ok(end as u32)
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
    let mut records = [const { Record::EMPTY }; crate::logs::MAX_MEMBERS];
    records[0].name[..name.len()].copy_from_slice(name);
    records[0].len = name.len();
    records[0].time = time;
    records[0].date = date;
    let mut offset = append(dump, size, out, &mut records[0], 0, words, None)?;
    let mut count = 1;
    let mut sources = 0;
    let mut manifest = crate::logs::Manifest::new();
    let _ = writeln!(
        manifest,
        "SRP crash log collection\nPolicy: recent 48 hours; newest 32 matching files; whole sources; 16 MiB compressed log ZIP contribution (headers included, summary separate); 256 MiB source bytes read including failed attempts; 5 second collection budget.\nOnly named game/launcher/updater/render logs and game journals in fixed locations. No recursion.\nWhole files preserve startup diagnostics and crash context. Buffered writes may be missing. Over-budget or failed sources are omitted atomically, never truncated.\n"
    );
    let log_deadline = out.deadline.min(GetTickCount64() + 5_000);
    let mut folders = crate::logs::folders();
    let candidates = crate::logs::recent(&mut folders, log_deadline, &mut manifest);
    let mut packed_left = crate::logs::PACKED_BYTES;
    let mut input_left = crate::logs::INPUT_BYTES;
    let mut included_input = 0;
    for candidate in &candidates {
        if candidate.len == 0 {
            break;
        }
        let record = &mut records[count];
        record.len = candidate.zip_name(&mut record.name);
        let label = core::str::from_utf8(&record.name[..record.len]).unwrap_or("invalid name");
        if GetTickCount64() > log_deadline {
            let _ = writeln!(manifest, "skipped {}: time limit", label);
            continue;
        }
        let mut path = Wide::<1024>::new();
        let folder = &folders[candidate.scope];
        if path.wide(&folder.buf[..folder.len]).is_err()
            || path.wide(&candidate.name[..candidate.len]).is_err()
        {
            let _ = writeln!(manifest, "skipped {}: path too long", label);
            continue;
        }
        let handle = CreateFileW(
            path.ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            null(),
            OPEN_EXISTING,
            FILE_FLAG_SEQUENTIAL_SCAN | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            let _ = writeln!(manifest, "skipped {}: open error {}", label, GetLastError());
            continue;
        }
        let mut info: BY_HANDLE_FILE_INFORMATION = core::mem::zeroed();
        let mut log = File {
            handle,
            deadline: log_deadline,
            left: 0,
        };
        let original = (|| {
            if GetFileInformationByHandle(handle, &mut info) == 0 {
                return Err(GetLastError());
            }
            if info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)
                != 0
            {
                return Err(ERROR_INVALID_DATA);
            }
            log.size()
        })();
        let original = match original {
            Ok(size) => size,
            Err(error) => {
                CloseHandle(handle);
                let _ = writeln!(manifest, "skipped {}: metadata error {}", label, error);
                continue;
            }
        };
        if original > input_left {
            CloseHandle(handle);
            let _ = writeln!(
                manifest,
                "skipped {}: input-processing limit (source {}, remaining {})",
                label, original, input_left
            );
            continue;
        }
        log.left = original;
        record.time = time;
        record.date = date;
        let result = if record.len == 0 {
            Err(ERROR_INVALID_NAME)
        } else {
            append(
                &mut log,
                original,
                out,
                record,
                offset,
                words,
                Some((packed_left, log_deadline)),
            )
        };
        // Read work is never refunded when an attempted source is rolled back.
        let consumed = original - log.left;
        input_left -= consumed;
        CloseHandle(handle);
        let label = core::str::from_utf8(&record.name[..record.len]).unwrap_or("invalid name");
        match result {
            Ok(next) => {
                let mut central = [0; zip::TAIL_MAX];
                let contribution = (next - offset) as u64
                    + zip::central(&record.entry(), record.offset, &mut central) as u64;
                let _ = writeln!(
                    manifest,
                    "included {}: source_bytes={}, compressed_zip_bytes={}",
                    label, original, contribution
                );
                packed_left -= contribution;
                included_input += original;
                offset = next;
                count += 1;
                sources += 1;
            }
            Err(error) => {
                let reason = match error {
                    LOG_ZIP_LIMIT => "compressed ZIP limit",
                    ERROR_TIMEOUT => "time limit",
                    _ => "read/append failure",
                };
                let _ = writeln!(
                    manifest,
                    "skipped {}: {} (error {}, input_bytes_read={}, source rolled back)",
                    label, reason, error, consumed
                );
                out.seek(offset as i64)?;
                if SetEndOfFile(out.handle) == 0 {
                    return Err(GetLastError());
                }
            }
        }
    }
    let _ = writeln!(
        manifest,
        "Included logs: {}; included_source_bytes={}; compressed_log_zip_bytes={}; input_bytes_processed={}",
        sources,
        included_input,
        crate::logs::PACKED_BYTES - packed_left,
        crate::logs::INPUT_BYTES - input_left
    );
    let record = &mut records[count];
    let name = b"logs/collection.txt";
    record.name[..name.len()].copy_from_slice(name);
    record.len = name.len();
    record.time = time;
    record.date = date;
    offset = append(
        &mut Bytes(&manifest.buf[..manifest.len]),
        manifest.len as u64,
        out,
        record,
        offset,
        words,
        None,
    )?;
    count += 1;
    let records = &records[..count];
    let central_offset = offset;
    let mut tail = [0; zip::TAIL_MAX];
    out.seek(offset as i64)?;
    for record in records {
        let n = zip::central(&record.entry(), record.offset, &mut tail);
        out.write(&tail[..n])?;
        offset += n as u32;
    }
    let central_size = offset - central_offset;
    let n = zip::end(count as u16, central_size, central_offset, &mut tail);
    out.write(&tail[..n])?;
    if FlushFileBuffers(out.handle) == 0 {
        return Err(GetLastError());
    }
    if out.size()? != offset as u64 + n as u64 {
        return Err(ERROR_INVALID_DATA);
    }
    // Read every local header, payload, central record and end record back.
    // Only a complete, independently inflated ZIP may replace the raw dump.
    let mut back = [0; zip::TAIL_MAX];
    let mut local = [0; zip::LOCAL_MAX];
    out.seek(0)?;
    for record in records {
        let n = zip::local_header(&record.entry(), &mut local);
        out.left = n as u64;
        out.exact(&mut back[..n])?;
        if back[..n] != local[..n] {
            return Err(ERROR_INVALID_DATA);
        }
        out.left = record.packed as u64;
        if zip::check(out, &mut Memory::new(words))? != (record.crc, record.size as u64) {
            return Err(ERROR_INVALID_DATA);
        }
    }
    for record in records {
        let n = zip::central(&record.entry(), record.offset, &mut tail);
        out.left = n as u64;
        out.exact(&mut back[..n])?;
        if back[..n] != tail[..n] {
            return Err(ERROR_INVALID_DATA);
        }
    }
    let n = zip::end(count as u16, central_size, central_offset, &mut tail);
    out.left = n as u64 + 1;
    out.exact(&mut back[..n])?;
    if back[..n] != tail[..n] || out.read(&mut back[..1])? != 0 {
        return Err(ERROR_INVALID_DATA);
    }
    Ok(())
}

//! A run's logs (patch 147, docs/notes/run-logs.md): every start of the game
//! writes its logs to files of their own, named by the run's start, and the
//! oldest of a kind are deleted past [`KEEP_FILES`] files or [`KEEP_BYTES`].
//! A file is never written over: a name is taken only when no file has it.
//!
//! - The run's start ([`start`], the process's first hook) is the local time
//!   to the second, a [`Stamp`]: `2026-11-15_14-03-07`.
//! - A log of a kind is `<folder><stem>.<stamp>.log`, a further file of the
//!   same stamp (a second game started in the same second, or a log's next
//!   part) `<stem>.<stamp>_<n>.log`, n from 2 to [`Seq::LAST`]: the engine's
//!   `Survarium_<user>.2026-11-15_14-03-07.log` in Documents\Survarium, the
//!   patches' `srp_patches.2026-11-15_14-03-07.log` beside the exe.
//! - A name of a stamp is never taken twice: a further file is past the
//!   highest the stamp has ([`Kind::next_name`]).
//! - Deletion goes by the files' last writes, the least recent first (not
//!   by the stamps: a clock set back), never a file of the running run's
//!   stamp, past one Windows does not delete ([`Kind::prune`]).
//! - A kind's files are the names that parse as one ([`Kind::run_of`]): the
//!   old unstamped `Survarium_<user>.log`, `srp_patches.log` and
//!   `srp_patches.old.log` are none, kept as they are.
//!
//! kernel32 does the work ([`super::files`]); nothing here logs (the
//! patches' log calls it while it writes a line).

use core::marker::PhantomData;
use core::sync::atomic::{AtomicU32, Ordering};

use super::clock::{FileTime, LocalTime};
use super::context::Context;
use super::files::{self, Find, FindData, Path};
use super::hook::Thread;
use crate::raw;

/// A kind keeps at most this many files, the new one with them.
pub const KEEP_FILES: u32 = 100;
/// A kind's files take less than this many bytes (500 MiB) before a new one.
pub const KEEP_BYTES: u64 = 500 << 20;

/// A moment to the second, as the player's machine shows it: when a run
/// started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

impl Stamp {
    /// `YYYY-MM-DD_HH-MM-SS`.
    pub const LEN: usize = 19;

    /// The moment, when it is one (a year of four digits, a month 1 to 12,
    /// a day 1 to 31, a time of day).
    pub fn of(time: LocalTime) -> Option<Stamp> {
        let LocalTime { year, month, day, hour, minute, second } = time;
        let valid = (1..=9999).contains(&year)
            && (1..=12).contains(&month)
            && (1..=31).contains(&day)
            && hour < 24
            && minute < 60
            && second < 61;
        valid.then_some(Stamp { year, month, day, hour, minute, second })
    }

    /// `YYYY-MM-DD_HH-MM-SS`.
    pub fn text(self) -> [u8; Stamp::LEN] {
        let mut out = *b"0000-00-00_00-00-00";
        let year = self.year;
        let digits = [
            (0, (year / 1000) as u8), (1, (year / 100 % 10) as u8), (2, (year / 10 % 10) as u8), (3, (year % 10) as u8),
            (5, self.month / 10), (6, self.month % 10), (8, self.day / 10), (9, self.day % 10),
            (11, self.hour / 10), (12, self.hour % 10), (14, self.minute / 10), (15, self.minute % 10),
            (17, self.second / 10), (18, self.second % 10),
        ];
        for (at, digit) in digits {
            if let Some(slot) = out.get_mut(at) {
                *slot = b'0' + digit;
            }
        }
        out
    }

    /// The moment `text` writes ([`Stamp::text`]'s form exactly).
    pub fn parse(text: &[u8]) -> Option<Stamp> {
        let form = b"0000-00-00_00-00-00";
        if text.len() != form.len() {
            return None;
        }
        let shape = text.iter().zip(form).all(|(&c, &f)| if f == b'0' { c.is_ascii_digit() } else { c == f });
        if !shape {
            return None;
        }
        let number = |from: usize, to: usize| {
            text.get(from..to).map(|digits| digits.iter().fold(0u16, |n, &d| n * 10 + u16::from(d - b'0')))
        };
        let small = |from: usize| number(from, from + 2).map(|n| n as u8);
        Stamp::of(LocalTime {
            year: number(0, 4)?,
            month: small(5)?,
            day: small(8)?,
            hour: small(11)?,
            minute: small(14)?,
            second: small(17)?,
        })
    }

    /// Two words: the date (never 0: a month is at least 1) and the time.
    fn pack(self) -> [u32; 2] {
        [
            u32::from(self.year) << 16 | u32::from(self.month) << 8 | u32::from(self.day),
            u32::from(self.hour) << 16 | u32::from(self.minute) << 8 | u32::from(self.second),
        ]
    }

    fn unpack([date, time]: [u32; 2]) -> Option<Stamp> {
        Stamp::of(LocalTime {
            year: (date >> 16) as u16,
            month: (date >> 8) as u8,
            day: date as u8,
            hour: (time >> 16) as u8,
            minute: (time >> 8) as u8,
            second: time as u8,
        })
    }
}

/// The run's start: [`Stamp::pack`]'s words, the date 0 until it started.
/// Written once on the process's one thread at its start; read by the
/// patches' log on any thread after.
static RUN: [AtomicU32; 2] = [AtomicU32::new(0), AtomicU32::new(0)];

/// The run starts now (the process's first hook; the engine's log asks
/// again): its stamp, the one it started with when it had. None only when
/// Windows gives no local time. The patches' log begins the run's file at
/// its next line.
pub fn start(thread: &Thread) -> Option<Stamp> {
    if let Some(stamp) = started() {
        return Some(stamp);
    }
    let stamp = Stamp::of(FileTime::now(thread).local(thread)?)?;
    let [date, time] = stamp.pack();
    RUN[1].store(time, Ordering::Relaxed);
    RUN[0].store(date, Ordering::Release);
    super::log::run_started();
    Some(stamp)
}

/// The run's stamp, once it started.
pub fn started() -> Option<Stamp> {
    let date = RUN[0].load(Ordering::Acquire);
    (date != 0).then(|| Stamp::unpack([date, RUN[1].load(Ordering::Relaxed)])).flatten()
}

/// Which file of a stamp a name is: the first (no suffix) or `_<n>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Seq(u8);

impl Seq {
    pub const FIRST: Seq = Seq(1);
    /// `_99`: a stamp's last file.
    pub const LAST: Seq = Seq(99);

    fn next(self) -> Option<Seq> {
        (self < Seq::LAST).then_some(Seq(self.0 + 1))
    }
}

/// A path for kernel32: at most MAX_PATH - 1 bytes and its NUL.
pub struct Name {
    bytes: [u8; Name::ROOM],
    len: usize,
}

impl Name {
    /// MAX_PATH, the NUL with it.
    const ROOM: usize = 260;

    /// The parts one after another; none when they do not fit or hold a NUL.
    fn of(parts: &[&[u8]]) -> Option<Name> {
        let mut name = Name { bytes: [0; Name::ROOM], len: 0 };
        for &byte in parts.iter().flat_map(|part| part.iter()) {
            if byte == 0 || name.len + 1 >= Name::ROOM {
                return None;
            }
            *name.bytes.get_mut(name.len)? = byte;
            name.len += 1;
        }
        Some(name)
    }

    /// The path's bytes, without the NUL.
    pub fn bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or(&[])
    }

    fn path(&self) -> Option<Path<'_>> {
        Path::new(self.bytes.get(..=self.len)?)
    }
}

/// A log's kind: its folder (empty: the working folder, the exe's) and its
/// name's stem, its files `<folder><stem>.<stamp>[_<n>].log`.
#[derive(Clone, Copy, Debug)]
pub struct Kind<'a> {
    folder: &'a [u8],
    stem: &'a [u8],
}

/// ASCII letters in any case, any other byte as it is.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}

/// Where the path's file name begins: past its last folder separator (`\`
/// or `/`), 0 when it has none. `lead` says a byte is a lead byte of the
/// ANSI code page's two-byte characters (Shift-JIS, GBK, ...): the byte
/// after it is its trail byte, which may be 5Ch and is no separator.
fn file_name_at(path: &[u8], lead: impl Fn(u8) -> bool) -> usize {
    let (mut at, mut name) = (0, 0);
    while let Some(&byte) = path.get(at) {
        if byte == b'\\' || byte == b'/' {
            name = at + 1;
            at += 1;
        } else {
            at += if lead(byte) { 2 } else { 1 };
        }
    }
    name
}

/// IsDBCSLeadByte of the ANSI code page (kernel32's, by GetProcAddress:
/// the exe does not import it); no byte is one when kernel32 does not say.
pub fn lead_byte() -> impl Fn(u8) -> bool {
    // SAFETY: kernel32 is in every process; GetModuleHandleA and
    // GetProcAddress through the exe's import slots, NUL-terminated names.
    let function = unsafe {
        let kernel32 = (raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
        if kernel32.is_null() {
            None
        } else {
            core::ptr::NonNull::new((raw::engine_get_proc_address)(kernel32, c"IsDBCSLeadByte".as_ptr().cast()))
        }
    };
    // SAFETY: IsDBCSLeadByte, that export (the library's dynamic row), a byte.
    move |byte| function.is_some_and(|f| unsafe { raw::engine_is_dbcs_lead_byte(f, u32::from(byte)) } != 0)
}

/// Where a file of a kind stands in the order of deletion: the least
/// recently written first (not the stamp: a clock set back would make the
/// run before the oldest); its name breaks a tie, so no two are equal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Age {
    written: u64,
    stamp: Stamp,
    seq: Seq,
}

impl<'a> Kind<'a> {
    /// The client patches' log beside the exe (the game's working folder).
    pub const PATCHES: Kind<'static> = Kind { folder: b"", stem: b"srp_patches" };

    /// The kind of a log the game named `<folder><stem>.log` (the engine's
    /// `...\Survarium_<user>.log`), its folder's end found as `lead` (the
    /// code page's lead bytes, [`lead_byte`]) says.
    pub fn of_log(path: &'a [u8], lead: impl Fn(u8) -> bool) -> Option<Kind<'a>> {
        let base = path.len().checked_sub(4)?;
        if !same(path.get(base..)?, b".log") {
            return None;
        }
        let folder = file_name_at(path, lead).min(base);
        let stem = path.get(folder..base)?;
        (!stem.is_empty()).then_some(Kind { folder: path.get(..folder)?, stem })
    }

    /// The file of `stamp`, `seq`: `<folder><stem>.<stamp>[_<n>].log`.
    pub fn name(&self, stamp: Stamp, seq: Seq) -> Option<Name> {
        let (text, Seq(n)) = (stamp.text(), seq);
        // `_<n>` without a leading zero
        let (one, two) = ([b'_', b'0' + n % 10], [b'_', b'0' + n / 10, b'0' + n % 10]);
        let suffix: &[u8] = match n {
            ..=1 => &[],
            2..=9 => &one,
            _ => &two,
        };
        Name::of(&[self.folder, self.stem, b".", &text, suffix, b".log"])
    }

    /// Which file of this kind `file` (a name a folder holds) is, when it is
    /// one. The stem as the game wrote it, byte for byte (a trail byte of a
    /// two-byte character may look like a letter); `.log` in any case.
    pub fn run_of(&self, file: &[u8]) -> Option<(Stamp, Seq)> {
        let rest = file.strip_prefix(self.stem)?.strip_prefix(b".")?;
        let body = rest.get(..rest.len().checked_sub(4)?)?;
        if !same(rest.get(body.len()..)?, b".log") {
            return None;
        }
        let stamp = Stamp::parse(body.get(..Stamp::LEN)?)?;
        let seq = match body.get(Stamp::LEN..)? {
            [] => Seq::FIRST,
            [b'_', d @ b'2'..=b'9'] => Seq(d - b'0'),
            [b'_', t @ b'1'..=b'9', u @ b'0'..=b'9'] => Seq((t - b'0') * 10 + (u - b'0')),
            _ => return None,
        };
        Some((stamp, seq))
    }

    /// The next name of `stamp`: past the highest of the stamp's files there
    /// (a name the run took is never taken again, even once deleted), and
    /// one no file has; none past `_99` or when the folder's path is too
    /// long.
    pub fn next_name(&self, stamp: Stamp) -> Option<Name> {
        let mut seq = match self.walk(stamp, None).highest {
            Some(highest) => highest.next()?,
            None => Seq::FIRST,
        };
        loop {
            let name = self.name(stamp, seq)?;
            if !exists(&name) {
                return Some(name);
            }
            seq = seq.next()?;
        }
    }

    /// The kind's files deleted, the least recently written first, while
    /// there are [`KEEP_FILES`] or more of them or they take [`KEEP_BYTES`]
    /// or more, so that the new one makes at most KEEP_FILES: how many went.
    /// The files of the run `run` (its stamp's) are counted and never
    /// deleted; one Windows does not delete (read-only, open) is passed over.
    pub fn prune(&self, run: Stamp) -> u32 {
        prune(run, |walk| self.walk_into(walk), |path| path.path().is_some_and(files::delete))
    }

    /// The kind's files now, `run`'s apart, the oldest after `after`.
    fn walk(&self, run: Stamp, after: Option<Age>) -> Walk {
        let mut walk = Walk::new(run, after);
        self.walk_into(&mut walk);
        walk
    }

    /// Every file of the kind seen by `walk`.
    fn walk_into(&self, walk: &mut Walk) {
        let Some(pattern) = Name::of(&[self.folder, self.stem, b".*.log"]) else { return };
        let mut data = FindData::new();
        let Some(mut find) = pattern.path().and_then(|pattern| Find::first(pattern, &mut data)) else { return };
        loop {
            if !data.is_folder()
                && let Some((stamp, seq)) = self.run_of(data.name())
            {
                let size = data.size().map_or(u64::from(u32::MAX), u64::from);
                let age = Age { written: data.last_written(), stamp, seq };
                walk.see(age, size, || Name::of(&[self.folder, data.name()]));
            }
            if !find.next(&mut data) {
                break;
            }
        }
    }
}

/// [`Kind::prune`]'s steps over a folder `walk_into` walks and `delete`
/// deletes in: the least recently written of the others first, each looked
/// at once (one not deleted is passed over), until room is made.
fn prune(run: Stamp, mut walk_into: impl FnMut(&mut Walk), mut delete: impl FnMut(&Name) -> bool) -> u32 {
    let mut all = Walk::new(run, None);
    walk_into(&mut all);
    let (mut files, mut bytes, mut after, mut deleted) = (all.files, all.bytes, None, 0);
    for _ in 0..Walk::MOST_LOOKS {
        if files < KEEP_FILES && bytes < KEEP_BYTES {
            break;
        }
        let mut walk = Walk::new(run, after);
        walk_into(&mut walk);
        let Some((age, size, path)) = walk.oldest else { break };
        after = Some(age);
        if delete(&path) {
            files -= 1;
            bytes = bytes.saturating_sub(size);
            deleted += 1;
        }
    }
    deleted
}

/// Whether a file (or folder) has the name.
fn exists(name: &Name) -> bool {
    let mut data = FindData::new();
    name.path().and_then(|path| Find::first(path, &mut data)).is_some()
}

/// A walk of a kind's files: how many and their bytes (all of them), the
/// highest seq of the run's stamp, and the least recently written of the
/// others after a point of the order (its age, size and path).
struct Walk {
    run: Stamp,
    after: Option<Age>,
    files: u32,
    bytes: u64,
    highest: Option<Seq>,
    oldest: Option<(Age, u64, Name)>,
}

impl Walk {
    /// A prune's most looks (a folder of a thousand files' worth): each is
    /// a walk of the folder.
    const MOST_LOOKS: u32 = 1000;

    fn new(run: Stamp, after: Option<Age>) -> Walk {
        Walk { run, after, files: 0, bytes: 0, highest: None, oldest: None }
    }

    /// A file of the kind; `path` makes its path when it is the oldest so far.
    fn see(&mut self, age: Age, size: u64, path: impl FnOnce() -> Option<Name>) {
        self.files += 1;
        self.bytes = self.bytes.saturating_add(size);
        if age.stamp == self.run {
            self.highest = self.highest.max(Some(age.seq));
            return;
        }
        if self.after.is_none_or(|after| age > after)
            && self.oldest.as_ref().is_none_or(|(oldest, _, _)| age < *oldest)
            && let Some(path) = path()
        {
            self.oldest = Some((age, size, path));
        }
    }
}


/// `logging.log_named`, logging_initialize once generate_log_file_name
/// (0xE29C0) made the engine's log's name and before new_log_file (0x1128D0)
/// opens it, made anew (create_always): EDI the name, a buffer_string over
/// its frame's 104h bytes (`<Documents>\survarium\Survarium_<user>.log`).
pub struct EngineLogName<'t> {
    string: raw::BufferString,
    _hook: PhantomData<&'t Context<'t>>,
}

impl<'t> EngineLogName<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<EngineLogName<'t>> {
        // SAFETY: the site's EDI, logging_initialize's local buffer_string.
        let string = unsafe { raw::site::logging_log_named::name(ctx)? };
        Some(EngineLogName { string, _hook: PhantomData })
    }

    /// The name's bytes (begin..end), copied; none for no begin, an end
    /// before it, or a name too long for a path.
    pub fn path(&self) -> Option<Name> {
        // SAFETY: the buffer_string generate_log_file_name filled, its text
        // the range begin..end while the hook runs (logging_initialize's
        // frame, the game's one thread at its start).
        unsafe {
            let (begin, end) = (self.string.begin(), self.string.end());
            let length = (end as usize).checked_sub(begin as usize)?;
            if begin.is_null() || length >= Name::ROOM {
                return None;
            }
            Name::of(&[core::slice::from_raw_parts(begin.cast_const(), length)])
        }
    }

    /// The engine opens `name` instead, when it fits the string's room (its
    /// text and NUL before the capacity's end): whether it does.
    pub fn set(&self, name: &Name) -> bool {
        // SAFETY: the site's buffer_string (as path); its begin and capacity
        // end the buffer's (generate_log_file_name's frame, 0xE31CA..0xE31E3).
        let room = unsafe { (self.string.capacity_end() as usize).checked_sub(self.string.begin() as usize) };
        if room.is_none_or(|room| name.len >= room) {
            return false;
        }
        let Some(text) = name.bytes.get(..=name.len) else { return false };
        // SAFETY: buffer_string::assign on that string, a C string of ours
        // shorter than its room (checked above), so nothing is cut.
        unsafe { raw::engine_string_assign(Some(self.string), text.as_ptr().cast()) };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: LocalTime = LocalTime { year: 2026, month: 11, day: 15, hour: 14, minute: 3, second: 7 };

    /// 14:03:00 and `n` seconds (`n` past 59 in the minutes after).
    fn stamp(n: u8) -> Stamp {
        Stamp::of(LocalTime { minute: 3 + n / 60, second: n % 60, ..AT }).unwrap()
    }

    #[test]
    fn a_stamp_is_its_text_and_back() {
        let at = stamp(7);
        assert_eq!(&at.text(), b"2026-11-15_14-03-07");
        assert_eq!(Stamp::parse(b"2026-11-15_14-03-07"), Some(at));
        assert_eq!(Stamp::unpack(at.pack()), Some(at));
        for bad in [&b"2026-11-15_14-03-7"[..], b"2026-11-15 14-03-07", b"2026-13-15_14-03-07", b"2026-11-00_14-03-07",
                    b"2026-11-15_24-03-07", b"0000-11-15_14-03-07", b"2026-11-15_14-03-0x"] {
            assert_eq!(Stamp::parse(bad), None, "{:?}", core::str::from_utf8(bad));
        }
        assert!(stamp(6) < stamp(7));
        assert!(Stamp::of(LocalTime { year: 2027, month: 1, day: 1, hour: 0, minute: 0, second: 0 }).unwrap() > at);
    }


    /// No two-byte characters (a Western code page).
    fn single(_: u8) -> bool {
        false
    }

    /// Shift-JIS's lead bytes (code page 932).
    fn shift_jis(byte: u8) -> bool {
        matches!(byte, 0x81..=0x9F | 0xE0..=0xFC)
    }

    #[test]
    fn names_of_a_kind() {
        let engine = Kind::of_log(b"c:\\users\\a\\documents\\survarium\\Survarium_a.log", single).unwrap();
        assert_eq!(engine.folder, b"c:\\users\\a\\documents\\survarium\\");
        assert_eq!(engine.stem, b"Survarium_a");
        assert_eq!(engine.name(stamp(7), Seq::FIRST).unwrap().bytes(),
                   b"c:\\users\\a\\documents\\survarium\\Survarium_a.2026-11-15_14-03-07.log");
        assert_eq!(Kind::PATCHES.name(stamp(7), Seq(2)).unwrap().bytes(), b"srp_patches.2026-11-15_14-03-07_2.log");
        assert_eq!(Kind::PATCHES.name(stamp(7), Seq(10)).unwrap().bytes(), b"srp_patches.2026-11-15_14-03-07_10.log");
        assert_eq!(Kind::PATCHES.name(stamp(7), Seq::LAST).unwrap().bytes(), b"srp_patches.2026-11-15_14-03-07_99.log");
        assert!(Kind::of_log(b"x/Survarium.log", single).is_some());
        assert!(Kind::of_log(b"x\\.log", single).is_none());
        assert!(Kind::of_log(b"x\\Survarium.txt", single).is_none());
        // a path past MAX_PATH is no name
        let long = [b'a'; 250];
        assert!(Kind { folder: &long, stem: b"srp_patches" }.name(stamp(7), Seq::FIRST).is_none());
        assert!(Kind { folder: &long[..220], stem: b"srp_patches" }.name(stamp(7), Seq::FIRST).is_some());
    }

    #[test]
    fn a_two_byte_characters_trail_byte_is_no_separator() {
        // the user ソニー (83 5C, 83 6A, 81 5B in Shift-JIS) in a folder of 表 (95 5C)
        let path = b"c:\\users\\\x95\x5c\\documents\\survarium\\Survarium_\x83\x5c\x83\x6a\x81\x5b.log";
        let kind = Kind::of_log(path, shift_jis).unwrap();
        assert_eq!(kind.folder, b"c:\\users\\\x95\x5c\\documents\\survarium\\");
        assert_eq!(kind.stem, b"Survarium_\x83\x5c\x83\x6a\x81\x5b");
        assert_eq!(kind.run_of(b"Survarium_\x83\x5c\x83\x6a\x81\x5b.2026-11-15_14-03-07.log"), Some((stamp(7), Seq::FIRST)));
        // a byte-wise split takes the trail byte for the folder's end
        assert_eq!(Kind::of_log(path, single).unwrap().stem, b"\x83\x6a\x81\x5b");
        // a lead byte at the very end reads past nothing
        assert_eq!(file_name_at(b"a\\\x83", shift_jis), 2);
    }

    #[test]
    fn only_a_kinds_stamped_names_are_its_files() {
        let kind = Kind::PATCHES;
        let run = |name: &[u8]| kind.run_of(name);
        assert_eq!(run(b"srp_patches.2026-11-15_14-03-07.log"), Some((stamp(7), Seq::FIRST)));
        assert_eq!(run(b"srp_patches.2026-11-15_14-03-07.LOG"), Some((stamp(7), Seq::FIRST)));
        assert_eq!(run(b"srp_patches.2026-11-15_14-03-07_2.log"), Some((stamp(7), Seq(2))));
        assert_eq!(run(b"srp_patches.2026-11-15_14-03-07_99.log"), Some((stamp(7), Seq(99))));
        for not in [&b"srp_patches.log"[..], b"srp_patches.old.log", b"srp_patches.2026-11-15_14-03-07_1.log",
                    b"srp_patches.2026-11-15_14-03-07_02.log", b"srp_patches.2026-11-15_14-03-07_100.log",
                    b"srp_patches.2026-11-15_14-03-07.txt", b"srp_patches2026-11-15_14-03-07.log",
                    b"srp_patchesX.2026-11-15_14-03-07.log", b"Survarium_a.2026-11-15_14-03-07.log"] {
            assert_eq!(run(not), None, "{:?}", core::str::from_utf8(not));
        }
        // the name a kind writes is the one it reads
        for seq in [Seq::FIRST, Seq(2), Seq(9), Seq(10), Seq::LAST] {
            assert_eq!(run(kind.name(stamp(7), seq).unwrap().bytes()), Some((stamp(7), seq)));
        }
    }

    /// A made-up folder of a kind's files: (stamp's second, seq, written, size, deletable).
    struct Folder(Vec<(u8, u8, u64, u64, bool)>);

    impl Folder {
        fn name(second: u8, seq: u8) -> Vec<u8> {
            Kind::PATCHES.name(stamp(second), Seq(seq)).unwrap().bytes().to_vec()
        }

        fn walk_into(&self, walk: &mut Walk) {
            for &(second, seq, written, size, _) in &self.0 {
                walk.see(Age { written, stamp: stamp(second), seq: Seq(seq) }, size, || Name::of(&[&Folder::name(second, seq)]));
            }
        }

        /// What a prune by the run of `run` deletes, in turn.
        fn prune(&mut self, run: u8) -> Vec<Vec<u8>> {
            let mut gone = Vec::new();
            let files = std::cell::RefCell::new(core::mem::take(&mut self.0));
            prune(stamp(run), |walk| Folder(files.borrow().clone()).walk_into(walk), |path| {
                let mut files = files.borrow_mut();
                let at = files.iter().position(|&(s, q, ..)| Folder::name(s, q) == path.bytes()).unwrap();
                let deletable = files[at].4;
                if deletable {
                    gone.push(files.remove(at).0);
                }
                deletable
            });
            self.0 = files.into_inner();
            gone.into_iter().map(|second| Folder::name(second, 1)).collect()
        }
    }

    #[test]
    fn the_least_recently_written_go_first() {
        // 100 runs a second apart, written in their order
        let runs = |n: u8| Folder((0..n).map(|s| (s, 1, u64::from(s), 10, true)).collect());
        let mut folder = runs(100);
        assert_eq!(folder.prune(59), [Folder::name(0, 1)]);
        assert_eq!(folder.0.len(), 99);
        let mut few = runs(99);
        assert!(few.prune(59).is_empty());
        // the clock set back: the run written last has the earliest stamp
        let mut back = runs(100);
        back.0[0].2 = 1000;
        assert_eq!(back.prune(59), [Folder::name(1, 1)]);
        // 500 MiB in a few
        let mut big = Folder(vec![(1, 1, 1, 300 << 20, true), (2, 1, 2, 300 << 20, true), (3, 1, 3, 1, true)]);
        assert_eq!(big.prune(59), [Folder::name(1, 1)]);
        let mut under = Folder(vec![(1, 1, 1, KEEP_BYTES - 1, true)]);
        assert!(under.prune(59).is_empty());
    }

    #[test]
    fn the_runs_own_files_stay_and_a_stuck_one_is_passed_over() {
        // one run's patches' log past 500 MiB: its parts are counted, never deleted
        let mut own = Folder((1..=70).map(|seq| (7, seq, u64::from(seq), 8 << 20, true)).collect());
        own.0.push((3, 1, 0, 1, true));
        assert_eq!(own.prune(7), [Folder::name(3, 1)]);
        assert_eq!(own.0.len(), 70);
        // a read-only oldest: the next goes, and room is made
        let mut stuck = Folder((0..101).map(|s| (s, 1, u64::from(s), 1, s != 0)).collect());
        assert_eq!(stuck.prune(59), [Folder::name(1, 1), Folder::name(2, 1)]);
        assert_eq!(stuck.0.len(), 99);
        // nothing deletable: every file looked at once, then the prune ends
        let mut none = Folder((0..120).map(|s| (s, 1, u64::from(s), 1, false)).collect());
        assert!(none.prune(59).is_empty());
    }

    #[test]
    fn a_walk_says_the_runs_highest_seq() {
        let folder = Folder(vec![(7, 1, 0, 1, true), (7, 3, 0, 1, true), (8, 5, 0, 1, true)]);
        let mut walk = Walk::new(stamp(7), None);
        folder.walk_into(&mut walk);
        assert_eq!((walk.highest, walk.files, walk.bytes), (Some(Seq(3)), 3, 3));
        assert_eq!(walk.oldest.map(|(age, ..)| age.stamp), Some(stamp(8)));
        let mut none = Walk::new(stamp(9), None);
        folder.walk_into(&mut none);
        assert_eq!(none.highest, None);
    }
}

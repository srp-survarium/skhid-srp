//! What is inside a client's game besides Windows' own DLLs (client patch
//! 125): the DLLs loaded into it and the other programs' windows over it.
//! Only logged.
//!
//! After the first world of a match connection loads, and when the match
//! closes, the client tells these kinds of entry (and in between, at a
//! minute whose look at the windows over the game saw otherwise than the
//! report before, only its [`Over`], [`Unseen`] and [`Exe`] entries,
//! [`Moment::Minute`]):
//! - a [`Module`]: a DLL (or the exe) in the loader's list, outside the
//!   Windows folder: its file name, where its folder is ([`Place`]: a
//!   user's name in the path is left out, so no Windows user name leaves
//!   the PC), its image size and its PE header's link time and checksum,
//!   which tell one build from another;
//! - [`Code`]: executable memory that is no module in that list, where a
//!   DLL mapped by hand (without the loader) runs: one entry an allocation,
//!   from a walk of the address space ([`CodeScan`]);
//! - a [`Load`]: a DLL the loader said it loaded after the game's start
//!   (outside the Windows folder), with when, and whether the loader still
//!   lists it: one loaded and taken out of the list again stays on record;
//! - an [`Over`]: a window of another process that is always on top, above
//!   the game's window and over some of it (an overlay), by its exe's file
//!   name and folder ([`Home`]), its class, its extended styles and how
//!   much of the game it is over; or [`Unseen`], why that could not be told;
//! - an [`Exe`]: the file of such a window's exe, by its size and SHA-256
//!   once the game has read it through ([`File`]). The file stays on the PC.
//!
//! An overlay (Discord, Steam, RTSS, OBS) puts DLLs into the game and a
//! window over it: the entries are read against every other game's.
//!
//! The body of a [`crate::report`] carrier of kind modules: up to
//! [`PER_PART`] entries a part and as many as its room holds (an entry is
//! as long as its kind and its folder's text), [`MAX_PARTS`] parts.
//!
//! ```text
//! u8 moment  u8 count  u8 seen  count x { u8 kind  body }
//!   kind 0 module: name[32] (UTF-8, zero-padded)  u32 image_size  pe[9]  place
//!   kind 1 code:   u8 memory  u8 protect  u32 exec_size  u32 size  u8 has_start  u32 start  pe[9]
//!   kind 2 load:   name[32]  u32 image_size  u32 at  u8 listed  pe[9]  place
//!   kind 3 over:   name[32]  class[32] (zeros: none)  u32 style  u16 width  u16 height
//!                  u8 home (0 unknown, 1 the Windows folder, 2 a place)  place (with 2)
//!   kind 4 unseen: u8 why
//!   kind 5 exe:    name[32]  u8 file (0 not read, 1 too big, 2 hashed)
//!                  u32 size  sha256[32] (with 2)  place
//!   place: u8 folder  u8 len  dir[len] (UTF-8, at most 64 bytes)
//!   at: seconds since the game started
//!   pe: u8 has  u32 stamp  u32 checksum (all zero without)
//!   moment: 0 loaded, 1 ended, from 2 a minute (the minute + 1)
//! ```

use crate::executable::Identity;

/// When in a match a module report was made: as every report's
/// ([`crate::report::Moment`]), or a minute's look at the windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Moment {
    /// The connection's first world is in.
    Loaded,
    /// The match is closing on the game.
    Ended,
    /// A look at the windows over the game's, at this minute after the first world.
    Minute(Minute),
}

/// A minute of a match connection after its first world: 1 to
/// [`Minute::LAST`], each its own moment byte, so the server tells one
/// minute's report from a copy of another's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Minute(u8);

impl Minute {
    pub const FIRST: Minute = Minute(1);
    /// The last minute that has a byte (over four hours: no match lasts so long).
    pub const LAST: Minute = Minute(u8::MAX - 1);

    /// The minute after this one; `None` after the last.
    pub fn next(self) -> Option<Minute> {
        self.0.checked_add(1).filter(|_| self < Minute::LAST).map(Minute)
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

impl Moment {
    /// Its kind: `loaded`, `ended` or `minute` (without the minute's number).
    pub fn word(self) -> &'static str {
        match self {
            Moment::Loaded => "loaded",
            Moment::Ended => "ended",
            Moment::Minute(_) => "minute",
        }
    }

    pub(crate) fn byte(self) -> u8 {
        match self {
            Moment::Loaded => 0,
            Moment::Ended => 1,
            Moment::Minute(Minute(minute)) => minute.saturating_add(1),
        }
    }

    /// Every byte is one.
    fn of(byte: u8) -> Moment {
        match byte.checked_sub(1) {
            None => Moment::Loaded,
            Some(0) => Moment::Ended,
            Some(minute) => Moment::Minute(Minute(minute)),
        }
    }
}

/// As the log writes it: `loaded`, `ended`, `minute 7`.
impl core::fmt::Display for Moment {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Moment::Minute(minute) => write!(f, "minute {}", minute.0),
            moment => f.write_str(moment.word()),
        }
    }
}

/// A name's bytes; a longer one is cut at a character's start.
pub const NAME: usize = 32;
/// A folder's text in bytes; a longer one is cut at a character's start.
pub const DIR: usize = 64;
const PE: usize = 9;
const HEAD: usize = 3;
/// The longest entry (a load in a folder of [`DIR`] bytes): its kind and its body.
const LONGEST: usize = 1 + NAME + NAME + 4 + 4 + 1 + 2 + DIR;
/// A part's body at most: with the carrier's header it stays in one 0xE0
/// chunk ([`crate::report::MAX_SIZE`]).
pub const MAX_BODY: usize = 465;
/// Entries in one part at most; fewer where their bytes fill it first.
pub const PER_PART: usize = 32;
/// Entries one report tells; more are counted in `seen` only.
pub const MAX_ENTRIES: usize = 144;
/// Parts of one report: every entry the longest.
pub const MAX_PARTS: u8 = MAX_ENTRIES.div_ceil((MAX_BODY - HEAD) / LONGEST) as u8;
/// The bytes of executable memory [`Code::start`] is the fingerprint of.
pub const START: usize = 256;

/// Where a module's file is. Windows' own folder is never told.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Folder {
    /// The game's folder, or one inside it.
    Game,
    /// The temporary folder (`GetTempPath`).
    Temp,
    /// Anywhere else: Program Files, the user's folders, another drive.
    Other,
}

impl Folder {
    pub fn word(self) -> &'static str {
        match self {
            Folder::Game => "game",
            Folder::Temp => "temp",
            Folder::Other => "other",
        }
    }
}

/// A folder's path as told: UTF-8, at most [`DIR`] bytes, no control
/// characters, its parts joined by `\`; empty for the folder it is told from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Dir {
    bytes: [u8; DIR],
    len: u8,
}

impl Dir {
    pub const EMPTY: Dir = Dir { bytes: [0; DIR], len: 0 };

    /// The folders of `path` (no file name in it), as told. With `whole`
    /// (a path from its drive, not one below a known folder) the name of a
    /// user's folder (`X:\Users\<name>`, Wine's `Z:\home\<name>`) becomes `*`.
    fn of(path: &[u16], whole: bool) -> Dir {
        const HOMES: [&str; 3] = ["users", "home", "documents and settings"];
        let is = |part: &[u16], word: &str| part.len() == word.len() && part.iter().zip(word.encode_utf16()).all(|(&a, b)| same(a, b));
        let parts = || path.split(|&unit| same(unit, 0x5C)).filter(|part| !part.is_empty());
        let drive = parts().next().is_some_and(|part| part.last() == Some(&0x3A));
        let home = parts().nth(1).is_some_and(|part| HOMES.iter().any(|word| is(part, word)));
        let star = [0x2A];
        let mut dir = Dir::EMPTY;
        for (index, part) in parts().enumerate() {
            let part = if whole && drive && home && index == 2 { star.as_slice() } else { part };
            let separator = (index > 0).then_some('\\');
            if !separator.into_iter().chain(decoded(part)).all(|c| push(&mut dir.bytes, &mut dir.len, c)) {
                break;
            }
        }
        dir
    }

    pub fn as_str(&self) -> &str {
        self.bytes.get(..usize::from(self.len)).and_then(|bytes| core::str::from_utf8(bytes).ok()).unwrap_or_default()
    }

    /// Its bytes as told: its length and its text.
    fn told(&self) -> usize {
        usize::from(self.len).saturating_add(1)
    }

    fn put(&self, out: &mut [u8]) {
        if let Some((len, text)) = out.split_first_mut() {
            *len = self.len;
            text.iter_mut().zip(self.bytes).for_each(|(to, from)| *to = from);
        }
    }

    /// The folder text that starts `bytes`, and what follows it.
    fn parse(bytes: &[u8]) -> Option<(Dir, &[u8])> {
        let (&len, rest) = bytes.split_first()?;
        let (text, rest) = rest.split_at_checked(usize::from(len))?;
        let text = core::str::from_utf8(text).ok()?;
        if text.len() > DIR || text.chars().any(char::is_control) {
            return None;
        }
        let mut dir = Dir { bytes: [0; DIR], len };
        dir.bytes.iter_mut().zip(text.bytes()).for_each(|(to, from)| *to = from);
        Some((dir, rest))
    }
}

impl core::fmt::Debug for Dir {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

/// UTF-16 as told: an unpaired surrogate U+FFFD, a control character `?`.
fn decoded(units: &[u16]) -> impl Iterator<Item = char> + '_ {
    char::decode_utf16(units.iter().copied()).map(|decoded| match decoded {
        Ok(c) if c.is_control() => '?',
        Ok(c) => c,
        Err(_) => char::REPLACEMENT_CHARACTER,
    })
}

/// `c` after the `len` bytes of text in `bytes`, if there is room for all of it.
fn push(bytes: &mut [u8], len: &mut u8, c: char) -> bool {
    let mut room = [0; 4];
    let encoded = c.encode_utf8(&mut room).as_bytes();
    let at = usize::from(*len);
    let Some(place) = bytes.get_mut(at..at.saturating_add(encoded.len())) else { return false };
    place.copy_from_slice(encoded);
    *len = len.saturating_add(encoded.len() as u8);
    true
}

/// Where a module's file is: the kind of folder, and the folder's path
/// below the game's or the temporary one, or (any other) from its drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Place {
    pub folder: Folder,
    pub dir: Dir,
}

impl Place {
    /// Where the file of the full path `path` is, from the game's folder,
    /// the temporary folder and the Windows folder (each a full path, its
    /// last `\` optional). `None`: under the Windows folder, not told.
    /// Letters compare without case, `/` as `\`; a folder matches only at
    /// a separator.
    pub fn of(path: &[u16], game: &[u16], temp: &[u16], windows: &[u16]) -> Option<Place> {
        let folders = |path: &[u16]| {
            let end = path.iter().rposition(|&unit| same(unit, 0x5C)).unwrap_or(0);
            path.get(..end).unwrap_or_default().len()
        };
        let place = |folder, below: &[u16], whole| Place { folder, dir: Dir::of(below.get(..folders(below)).unwrap_or_default(), whole) };
        if let Some(below) = below(path, game) {
            Some(place(Folder::Game, below, false))
        } else if let Some(below) = below(path, temp) {
            Some(place(Folder::Temp, below, false))
        } else if below(path, windows).is_some() {
            None
        } else {
            Some(place(Folder::Other, path, true))
        }
    }

    /// A place with no folder text: the game's or the temporary folder itself.
    pub const fn bare(folder: Folder) -> Place {
        Place { folder, dir: Dir::EMPTY }
    }

    fn told(&self) -> usize {
        self.dir.told().saturating_add(1)
    }

    fn put(&self, out: &mut [u8]) {
        if let Some((folder, dir)) = out.split_first_mut() {
            *folder = folder_byte(self.folder);
            self.dir.put(dir);
        }
    }

    fn parse(bytes: &[u8]) -> Option<(Place, &[u8])> {
        let (&folder, rest) = bytes.split_first()?;
        let (dir, rest) = Dir::parse(rest)?;
        Some((Place { folder: folder_of(folder)?, dir }, rest))
    }
}

fn same(a: u16, b: u16) -> bool {
    let fold = |unit: u16| match unit {
        0x2F => 0x5C,
        0x41..=0x5A => unit | 0x20,
        _ => unit,
    };
    fold(a) == fold(b)
}

/// What of `path` is below `folder`, if it is in it (an empty folder
/// holds nothing).
fn below<'a>(path: &'a [u16], folder: &[u16]) -> Option<&'a [u16]> {
    let folder = match folder.split_last() {
        Some((&last, rest)) if same(last, 0x5C) => rest,
        Some(_) => folder,
        None => return None,
    };
    if folder.is_empty() {
        return None;
    }
    let (head, tail) = path.split_at_checked(folder.len())?;
    let (&separator, rest) = tail.split_first()?;
    (head.iter().zip(folder).all(|(&a, &b)| same(a, b)) && same(separator, 0x5C)).then_some(rest)
}

/// A module's file name, UTF-8, 1 to [`NAME`] bytes, no control characters.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name {
    bytes: [u8; NAME],
    len: u8,
}

impl Name {
    /// The file name of a full path (after its last `\` or `/`), from
    /// UTF-16; an unpaired surrogate becomes U+FFFD, a control character
    /// `?`, and a name too long is cut. `None`: no name.
    pub fn of_path(path: &[u16]) -> Option<Name> {
        let start = path.iter().rposition(|&unit| unit == 0x5C || unit == 0x2F).map_or(0, |at| at.saturating_add(1));
        Name::of(path.get(start..)?)
    }

    /// `text` whole (a window class's name), from UTF-16 as `of_path`.
    pub fn of(text: &[u16]) -> Option<Name> {
        let mut name = Name { bytes: [0; NAME], len: 0 };
        let _ = decoded(text).all(|c| push(&mut name.bytes, &mut name.len, c));
        (name.len > 0).then_some(name)
    }

    /// As told: its bytes, or the zeros `out` has for none.
    fn put(name: Option<Name>, out: &mut [u8]) {
        if let Some(name) = name {
            out.copy_from_slice(&name.bytes);
        }
    }

    /// As `put` told it.
    fn parse_told(bytes: &[u8]) -> Option<Option<Name>> {
        if bytes.iter().all(|&byte| byte == 0) { Some(None) } else { Name::parse(bytes).map(Some) }
    }

    pub fn as_str(&self) -> &str {
        self.bytes.get(..usize::from(self.len)).and_then(|bytes| core::str::from_utf8(bytes).ok()).unwrap_or_default()
    }

    fn parse(bytes: &[u8]) -> Option<Name> {
        let bytes: [u8; NAME] = bytes.try_into().ok()?;
        let len = bytes.iter().position(|&byte| byte == 0).unwrap_or(NAME);
        let (text, rest) = bytes.split_at(len);
        let text = core::str::from_utf8(text).ok()?;
        if text.is_empty() || text.chars().any(char::is_control) || rest.iter().any(|&byte| byte != 0) {
            return None;
        }
        Some(Name { bytes, len: len as u8 })
    }
}

impl core::fmt::Debug for Name {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

/// A 32-bit PE header's build marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeMarks {
    /// `IMAGE_FILE_HEADER.TimeDateStamp`, the link time.
    pub stamp: u32,
    /// `IMAGE_OPTIONAL_HEADER.CheckSum` (0 for most DLLs).
    pub checksum: u32,
}

impl PeMarks {
    /// The marks of the first bytes of an image: its DOS header (`MZ`) and,
    /// at `e_lfanew`, `nt` (at least 92 bytes: the signature, the file
    /// header and the optional header to CheckSum). `read_nt` reads them
    /// at `e_lfanew` from the image's start. `None`: no 32-bit PE there.
    pub fn of(dos: &[u8; 0x40], read_nt: impl FnOnce(u32) -> Option<[u8; 92]>) -> Option<PeMarks> {
        let word = |bytes: &[u8], at: usize| bytes.get(at..at.checked_add(4)?)?.try_into().ok().map(u32::from_le_bytes);
        if dos.get(..2)? != b"MZ" {
            return None;
        }
        let nt = read_nt(word(dos, 0x3C)?)?;
        let magic = nt.get(24..26)?.try_into().ok().map(u16::from_le_bytes)?;
        if nt.get(..4)? != b"PE\0\0" || magic != 0x10B {
            return None;
        }
        Some(PeMarks { stamp: word(&nt, 8)?, checksum: word(&nt, 88)? })
    }

    fn put(marks: Option<PeMarks>, out: &mut [u8]) {
        if let (Some(marks), Some((has, rest))) = (marks, out.split_first_mut()) {
            *has = 1;
            put_words(rest, &[marks.stamp, marks.checksum]);
        }
    }

    fn parse(bytes: &[u8]) -> Option<Option<PeMarks>> {
        let (&has, rest) = bytes.split_first()?;
        let mut words = rest.chunks_exact(4).map(|word| word.try_into().map(u32::from_le_bytes));
        let (Some(Ok(stamp)), Some(Ok(checksum))) = (words.next(), words.next()) else { return None };
        match has {
            0 if stamp == 0 && checksum == 0 => Some(None),
            1 => Some(Some(PeMarks { stamp, checksum })),
            _ => None,
        }
    }
}

fn put_words(out: &mut [u8], words: &[u32]) {
    for (field, word) in out.chunks_exact_mut(4).zip(words) {
        field.copy_from_slice(&word.to_le_bytes());
    }
}

/// A module of the loader's list, as told.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Module {
    pub name: Name,
    pub place: Place,
    /// Its image's size as loaded.
    pub image_size: u32,
    /// `None`: its header could not be read, or is no 32-bit PE.
    pub pe: Option<PeMarks>,
}

/// What kind of memory a [`Code`] allocation is (`MEMORY_BASIC_INFORMATION.Type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Memory {
    /// `MEM_PRIVATE`: allocated (`VirtualAlloc`), where a hand-mapped DLL,
    /// a JIT's code or an overlay's trampolines are.
    Private,
    /// `MEM_MAPPED`: a view of a file or a section, not as an image.
    Mapped,
    /// `MEM_IMAGE` that no module of the loader's list is at: an image
    /// mapped without the loader, or one taken out of the list.
    UnlistedImage,
}

impl Memory {
    /// `MEMORY_BASIC_INFORMATION.Type`'s values.
    pub const PRIVATE: u32 = 0x2_0000;
    pub const MAPPED: u32 = 0x4_0000;
    pub const IMAGE: u32 = 0x100_0000;

    pub fn word(self) -> &'static str {
        match self {
            Memory::Private => "private",
            Memory::Mapped => "mapped",
            Memory::UnlistedImage => "image",
        }
    }
}

/// How the first executable region of a [`Code`] allocation is protected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Protect {
    /// `PAGE_EXECUTE`: run, not read.
    Execute,
    /// `PAGE_EXECUTE_READ`.
    ExecuteRead,
    /// `PAGE_EXECUTE_READWRITE`.
    ExecuteReadWrite,
    /// `PAGE_EXECUTE_WRITECOPY`.
    ExecuteWriteCopy,
}

impl Protect {
    /// A committed page's protection, if it runs code (not a guard page).
    pub fn of(page: u32) -> Option<Protect> {
        const GUARD: u32 = 0x100;
        if page & GUARD != 0 {
            return None;
        }
        match page & 0xFF {
            0x10 => Some(Protect::Execute),
            0x20 => Some(Protect::ExecuteRead),
            0x40 => Some(Protect::ExecuteReadWrite),
            0x80 => Some(Protect::ExecuteWriteCopy),
            _ => None,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Protect::Execute => "x",
            Protect::ExecuteRead => "rx",
            Protect::ExecuteReadWrite => "rwx",
            Protect::ExecuteWriteCopy => "wcx",
        }
    }
}

/// An allocation of executable memory that is no listed module, as told.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code {
    pub memory: Memory,
    /// Its first executable region's.
    pub protect: Protect,
    /// Its executable bytes, all regions.
    pub exec_size: u32,
    /// Its bytes, from its start to its last region's end.
    pub size: u32,
    /// [`fingerprint`] of the first [`START`] bytes of its first executable
    /// region; `None` where they cannot be read (`PAGE_EXECUTE`, or shorter).
    pub start: Option<u32>,
    /// A PE header at its start (one a hand mapper left), if any.
    pub pe: Option<PeMarks>,
}

/// FNV-1a of `bytes`: the same code at the same place has the same one.
pub fn fingerprint(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811C_9DC5, |hash: u32, &byte| (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193))
}

/// A region of the address space, as `VirtualQuery` tells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub base: usize,
    /// `AllocationBase`: the regions of one allocation share it.
    pub allocation: usize,
    pub size: usize,
    /// `State`: only `MEM_COMMIT` (0x1000) regions hold anything.
    pub state: u32,
    /// `Protect`.
    pub protect: u32,
    /// `Type`.
    pub kind: u32,
}

/// An allocation [`CodeScan`] found: where its first executable region is,
/// to read its start and its header from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Found {
    pub base: usize,
    pub first_exec: usize,
    pub memory: Memory,
    pub protect: Protect,
    pub exec_size: u32,
    pub size: u32,
}

impl Found {
    /// What is told of it, with the bytes read at `first_exec` and `base`.
    pub fn code(self, start: Option<&[u8; START]>, pe: Option<PeMarks>) -> Code {
        Code {
            memory: self.memory,
            protect: self.protect,
            exec_size: self.exec_size,
            size: self.size,
            start: start.map(|bytes| fingerprint(bytes)),
            pe,
        }
    }
}

/// The address space walked in order, region by region: each allocation
/// that runs code and is no listed module, once its last region is past.
#[derive(Default)]
pub struct CodeScan {
    open: Option<Open>,
}

#[derive(Clone, Copy)]
struct Open {
    base: usize,
    memory: Option<Memory>,
    end: usize,
    exec: Option<(usize, Protect, usize)>,
}

impl Open {
    fn found(self) -> Option<Found> {
        let (first_exec, protect, exec_size) = self.exec?;
        Some(Found {
            base: self.base,
            first_exec,
            memory: self.memory?,
            protect,
            exec_size: u32::try_from(exec_size).unwrap_or(u32::MAX),
            size: u32::try_from(self.end.saturating_sub(self.base)).unwrap_or(u32::MAX),
        })
    }
}

impl CodeScan {
    /// The next region in address order; `listed`: a listed module starts
    /// at this allocation base. Returns the allocation it closed, if that
    /// one is told.
    pub fn region(&mut self, region: Region, listed: impl FnOnce(usize) -> bool) -> Option<Found> {
        const COMMIT: u32 = 0x1000;
        let same = self.open.is_some_and(|open| open.base == region.allocation);
        let closed = if same { None } else { self.open.take().and_then(Open::found) };
        if region.state != COMMIT || region.allocation == 0 {
            return closed;
        }
        let open = self.open.get_or_insert_with(|| Open {
            base: region.allocation,
            memory: match region.kind {
                Memory::PRIVATE => Some(Memory::Private),
                Memory::MAPPED => Some(Memory::Mapped),
                Memory::IMAGE if !listed(region.allocation) => Some(Memory::UnlistedImage),
                _ => None,
            },
            end: region.allocation,
            exec: None,
        });
        open.end = open.end.max(region.base.saturating_add(region.size));
        if let Some(protect) = Protect::of(region.protect) {
            open.exec = Some(match open.exec {
                Some((first, protect, size)) => (first, protect, size.saturating_add(region.size)),
                None => (region.base, protect, region.size),
            });
        }
        closed
    }

    /// The walk is over: the last allocation, if it is told.
    pub fn end(self) -> Option<Found> {
        self.open.and_then(Open::found)
    }
}

/// A DLL the loader told the game it loaded, after the game's start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Load {
    pub name: Name,
    pub place: Place,
    pub image_size: u32,
    /// Its header's marks as it was loaded; `None`: not read, or no 32-bit PE.
    pub pe: Option<PeMarks>,
    /// Seconds from the game's start to the load.
    pub at: u32,
    /// The loader's list still has a module where it was loaded.
    pub listed: bool,
}

/// A window of another process over the game's: visible, always on top
/// ([`Over::TOPMOST`]), above the game's window in the windows' order and
/// over some of it. An overlay (Discord, Steam, a recorder) is one too.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Over {
    /// Its process's exe file name; `None`: the process list has none for it.
    pub name: Option<Name>,
    /// Its window class's name.
    pub class: Option<Name>,
    /// Its extended styles (`WS_EX_LAYERED` is 80000h, `WS_EX_TRANSPARENT` 20h).
    pub style: u32,
    /// How much of the game's window it is over, in pixels.
    pub width: u16,
    pub height: u16,
    /// Where its process's exe file is.
    pub home: Home,
}

/// Where the exe of a window's process is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Home {
    /// Windows did not tell the process's file.
    Unknown,
    /// In the Windows folder.
    Windows,
    /// As a module's file is told.
    At(Place),
}

impl Home {
    /// Of the exe at `path`, the folders as [`Place::of`] takes them.
    pub fn of(path: &[u16], game: &[u16], temp: &[u16], windows: &[u16]) -> Home {
        Place::of(path, game, temp, windows).map_or(Home::Windows, Home::At)
    }
}

impl Over {
    /// `WS_EX_TOPMOST`: the window stays above every window that is not.
    pub const TOPMOST: u32 = 8;
}

/// A window's place on the screen (`RECT`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    /// The width and height of what this and `other` share, if anything.
    pub fn overlap(self, other: Rect) -> Option<(u16, u16)> {
        let side = |from: i32, to: i32| u16::try_from(to.saturating_sub(from).min(i32::from(u16::MAX))).ok().filter(|&side| side > 0);
        let width = side(self.left.max(other.left), self.right.min(other.right))?;
        Some((width, side(self.top.max(other.top), self.bottom.min(other.bottom))?))
    }
}

/// The file of an exe whose window was over the game's ([`Over`], one
/// outside the Windows folder), told once the game has read it through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Exe {
    pub name: Name,
    pub place: Place,
    pub file: File,
}

/// What reading an [`Exe`]'s file gave.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum File {
    /// Its size and its SHA-256.
    Hashed(Identity),
    /// Larger than [`crate::executable::MAX_FILE`]: not read to its end.
    TooBig,
    /// It could not be opened or read, or is empty.
    Unread,
}

impl File {
    /// A hash's bytes as told: the file's size and its SHA-256.
    const HASH: usize = 4 + 32;
}

/// Why nothing is told of the windows over the game's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Unseen {
    /// The game has presented to no window yet, or Windows lacks a function.
    NoWindow,
    /// The game's window is not among the screen's windows, so none is
    /// known to be above it.
    NoOrder,
}

impl Unseen {
    const ALL: [Unseen; 2] = [Unseen::NoWindow, Unseen::NoOrder];

    pub fn word(self) -> &'static str {
        match self {
            Unseen::NoWindow => "no game window",
            Unseen::NoOrder => "game window not in the order",
        }
    }
}

/// One entry of a report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Entry {
    Module(Module),
    Code(Code),
    Load(Load),
    Over(Over),
    Unseen(Unseen),
    Exe(Exe),
}

fn folder_byte(folder: Folder) -> u8 {
    match folder {
        Folder::Game => 0,
        Folder::Temp => 1,
        Folder::Other => 2,
    }
}

fn folder_of(byte: u8) -> Option<Folder> {
    match byte {
        0 => Some(Folder::Game),
        1 => Some(Folder::Temp),
        2 => Some(Folder::Other),
        _ => None,
    }
}

/// The next `n` bytes of `out`, which then starts after them.
fn take<'a>(out: &mut &'a mut [u8], n: usize) -> &'a mut [u8] {
    let n = n.min(out.len());
    let (head, rest) = core::mem::take(out).split_at_mut(n);
    *out = rest;
    head
}

impl Entry {
    /// Its bytes as told: its kind and its body.
    fn told(&self) -> usize {
        let (fixed, place) = match self {
            Entry::Module(module) => (NAME + 4 + PE, Some(module.place)),
            Entry::Code(_) => (2 + 8 + 5 + PE, None),
            Entry::Load(load) => (NAME + 4 + 4 + 1 + PE, Some(load.place)),
            Entry::Unseen(_) => (1, None),
            Entry::Exe(exe) => match exe.file {
                File::Hashed(_) => (NAME + 1 + File::HASH, Some(exe.place)),
                File::TooBig | File::Unread => (NAME + 1, Some(exe.place)),
            },
            Entry::Over(over) => (
                NAME + NAME + 4 + 4 + 1,
                match over.home {
                    Home::At(place) => Some(place),
                    Home::Unknown | Home::Windows => None,
                },
            ),
        };
        place.map_or(0, |place| place.told()).saturating_add(fixed).saturating_add(1)
    }

    /// Into `record`, which is [`Entry::told`] bytes of zeros.
    fn put(&self, record: &mut [u8]) {
        let Some((kind, mut body)) = record.split_first_mut() else { return };
        let body = &mut body;
        match self {
            Entry::Module(module) => {
                *kind = 0;
                take(body, NAME).copy_from_slice(&module.name.bytes);
                put_words(take(body, 4), &[module.image_size]);
                PeMarks::put(module.pe, take(body, PE));
                module.place.put(body);
            }
            Entry::Code(code) => {
                *kind = 1;
                take(body, 2).copy_from_slice(&[
                    match code.memory {
                        Memory::Private => 0,
                        Memory::Mapped => 1,
                        Memory::UnlistedImage => 2,
                    },
                    match code.protect {
                        Protect::Execute => 0,
                        Protect::ExecuteRead => 1,
                        Protect::ExecuteReadWrite => 2,
                        Protect::ExecuteWriteCopy => 3,
                    },
                ]);
                put_words(take(body, 8), &[code.exec_size, code.size]);
                if let (Some(value), Some((has, word))) = (code.start, take(body, 5).split_first_mut()) {
                    *has = 1;
                    put_words(word, &[value]);
                }
                PeMarks::put(code.pe, body);
            }
            Entry::Load(load) => {
                *kind = 2;
                take(body, NAME).copy_from_slice(&load.name.bytes);
                put_words(take(body, 8), &[load.image_size, load.at]);
                take(body, 1).copy_from_slice(&[u8::from(load.listed)]);
                PeMarks::put(load.pe, take(body, PE));
                load.place.put(body);
            }
            Entry::Over(over) => {
                *kind = 3;
                Name::put(over.name, take(body, NAME));
                Name::put(over.class, take(body, NAME));
                put_words(take(body, 8), &[over.style, u32::from(over.width) | u32::from(over.height) << 16]);
                let home = take(body, 1);
                match over.home {
                    Home::Unknown => {}
                    Home::Windows => home.copy_from_slice(&[1]),
                    Home::At(place) => {
                        home.copy_from_slice(&[2]);
                        place.put(body);
                    }
                }
            }
            Entry::Unseen(unseen) => {
                *kind = 4;
                body.copy_from_slice(&[Unseen::ALL.iter().position(|why| why == unseen).unwrap_or_default() as u8]);
            }
            Entry::Exe(exe) => {
                *kind = 5;
                take(body, NAME).copy_from_slice(&exe.name.bytes);
                let file = take(body, 1);
                match exe.file {
                    File::Unread => {}
                    File::TooBig => file.copy_from_slice(&[1]),
                    File::Hashed(identity) => {
                        file.copy_from_slice(&[2]);
                        put_words(take(body, 4), &[identity.size()]);
                        take(body, 32).copy_from_slice(&identity.sha256);
                    }
                }
                exe.place.put(body);
            }
        }
    }

    /// The entry that starts `bytes`, and what follows it.
    fn parse(bytes: &[u8]) -> Option<(Entry, &[u8])> {
        let (&kind, body) = bytes.split_first()?;
        let word = |bytes: &[u8]| bytes.try_into().ok().map(u32::from_le_bytes);
        match kind {
            0 => {
                let (name, rest) = body.split_at_checked(NAME)?;
                let (size, rest) = rest.split_at_checked(4)?;
                let (pe, rest) = rest.split_at_checked(PE)?;
                let (place, rest) = Place::parse(rest)?;
                let module = Module { name: Name::parse(name)?, place, image_size: word(size)?, pe: PeMarks::parse(pe)? };
                Some((Entry::Module(module), rest))
            }
            1 => {
                let (&[memory, protect], rest) = body.split_first_chunk::<2>()?;
                let (sizes, rest) = rest.split_at_checked(8)?;
                let (start, rest) = rest.split_at_checked(5)?;
                let (pe, rest) = rest.split_at_checked(PE)?;
                let (&has_start, start) = start.split_first()?;
                let start = match (has_start, word(start)?) {
                    (0, 0) => None,
                    (1, value) => Some(value),
                    _ => return None,
                };
                let code = Code {
                    memory: match memory {
                        0 => Memory::Private,
                        1 => Memory::Mapped,
                        2 => Memory::UnlistedImage,
                        _ => return None,
                    },
                    protect: match protect {
                        0 => Protect::Execute,
                        1 => Protect::ExecuteRead,
                        2 => Protect::ExecuteReadWrite,
                        3 => Protect::ExecuteWriteCopy,
                        _ => return None,
                    },
                    exec_size: word(sizes.get(..4)?)?,
                    size: word(sizes.get(4..)?)?,
                    start,
                    pe: PeMarks::parse(pe)?,
                };
                Some((Entry::Code(code), rest))
            }
            2 => {
                let (name, rest) = body.split_at_checked(NAME)?;
                let (words, rest) = rest.split_at_checked(8)?;
                let (&listed, rest) = rest.split_first()?;
                let (pe, rest) = rest.split_at_checked(PE)?;
                let (place, rest) = Place::parse(rest)?;
                let load = Load {
                    name: Name::parse(name)?,
                    place,
                    image_size: word(words.get(..4)?)?,
                    pe: PeMarks::parse(pe)?,
                    at: word(words.get(4..)?)?,
                    listed: match listed {
                        0 => false,
                        1 => true,
                        _ => return None,
                    },
                };
                Some((Entry::Load(load), rest))
            }
            3 => {
                let (name, rest) = body.split_at_checked(NAME)?;
                let (class, rest) = rest.split_at_checked(NAME)?;
                let (words, rest) = rest.split_at_checked(8)?;
                let size = word(words.get(4..)?)?;
                let (&home, rest) = rest.split_first()?;
                let (home, rest) = match home {
                    0 => (Home::Unknown, rest),
                    1 => (Home::Windows, rest),
                    2 => Place::parse(rest).map(|(place, rest)| (Home::At(place), rest))?,
                    _ => return None,
                };
                let over = Over {
                    name: Name::parse_told(name)?,
                    class: Name::parse_told(class)?,
                    style: word(words.get(..4)?)?,
                    width: size as u16,
                    height: (size >> 16) as u16,
                    home,
                };
                Some((Entry::Over(over), rest))
            }
            4 => {
                let (&why, rest) = body.split_first()?;
                Some((Entry::Unseen(*Unseen::ALL.get(usize::from(why))?), rest))
            }
            5 => {
                let (name, rest) = body.split_at_checked(NAME)?;
                let (&file, rest) = rest.split_first()?;
                let (file, rest) = match file {
                    0 => (File::Unread, rest),
                    1 => (File::TooBig, rest),
                    2 => {
                        let (size, rest) = rest.split_at_checked(4)?;
                        let (sha256, rest) = rest.split_first_chunk::<32>()?;
                        (File::Hashed(Identity::new(word(size)?, *sha256)?), rest)
                    }
                    _ => return None,
                };
                let (place, rest) = Place::parse(rest)?;
                Some((Entry::Exe(Exe { name: Name::parse(name)?, place, file }), rest))
            }
            _ => None,
        }
    }
}

const EMPTY: Entry = Entry::Code(Code {
    memory: Memory::Private,
    protect: Protect::Execute,
    exec_size: 0,
    size: 0,
    start: None,
    pe: None,
});

/// A whole report, as the client gathers it at a [`Moment`]: the first
/// [`MAX_ENTRIES`] entries told, `seen` all it had to tell.
pub struct Report {
    moment: Moment,
    entries: [Entry; MAX_ENTRIES],
    told: usize,
    seen: usize,
}

impl Report {
    pub fn new(moment: Moment) -> Report {
        Report { moment, entries: [EMPTY; MAX_ENTRIES], told: 0, seen: 0 }
    }

    pub fn moment(&self) -> Moment {
        self.moment
    }

    pub fn entries(&self) -> &[Entry] {
        self.entries.get(..self.told).unwrap_or_default()
    }

    pub fn add(&mut self, entry: Entry) {
        if let Some(place) = self.entries.get_mut(self.told) {
            *place = entry;
            self.told = self.told.saturating_add(1);
        }
        self.seen = self.seen.saturating_add(1);
    }

    /// The entries each part holds, in order: as many as fit its room, at
    /// most [`PER_PART`] (always one: none is longer than the room).
    fn split(&self) -> impl Iterator<Item = &[Entry]> {
        let mut rest = self.entries();
        core::iter::from_fn(move || {
            let mut room = MAX_BODY - HEAD;
            let fits = |entry: &&Entry| match room.checked_sub(entry.told()) {
                Some(left) => {
                    room = left;
                    true
                }
                None => false,
            };
            let count = rest.iter().take(PER_PART).take_while(fits).count();
            let (part, after) = rest.split_at_checked(count.max(1))?;
            rest = after;
            Some(part)
        })
    }

    /// How many parts it goes in (one even when empty: the server hears
    /// that there was nothing to tell).
    pub fn parts(&self) -> u8 {
        self.split().count().max(1) as u8
    }

    /// The body of part `part` (0-based) into `out`, and its length.
    pub(crate) fn put_part(&self, part: u8, out: &mut [u8]) -> Option<usize> {
        let entries = match self.split().nth(usize::from(part)) {
            Some(entries) => entries,
            None if part == 0 => &[],
            None => return None,
        };
        let (head, mut records) = out.split_at_mut_checked(HEAD)?;
        head.copy_from_slice(&[self.moment.byte(), entries.len() as u8, self.seen.min(255) as u8]);
        let mut len = HEAD;
        for entry in entries {
            let told = entry.told();
            if records.len() < told {
                return None;
            }
            entry.put(take(&mut records, told));
            len = len.checked_add(told)?;
        }
        Some(len)
    }
}

/// The bytes of the body that starts `bytes` (more may follow it): its
/// head and its `count` entries, each as long as it says.
pub(crate) fn body_len(bytes: &[u8]) -> Option<usize> {
    let (head, mut records) = bytes.split_at_checked(HEAD)?;
    let &count = head.get(1)?;
    if usize::from(count) > PER_PART {
        return None;
    }
    for _ in 0..count {
        records = Entry::parse(records)?.1;
    }
    let len = bytes.len().checked_sub(records.len())?;
    (len <= MAX_BODY).then_some(len)
}

/// One part as the server reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub moment: Moment,
    /// The entries the client had to tell in all (255: that many or more).
    pub seen: u8,
    entries: [Entry; PER_PART],
    count: u8,
}

impl Part {
    pub fn entries(&self) -> &[Entry] {
        self.entries.get(..usize::from(self.count)).unwrap_or_default()
    }

    /// Exactly a body: a moment, `count` entries, each valid (at least one
    /// in a part before the last, `more`).
    pub(crate) fn parse(bytes: &[u8], more: bool) -> Option<Part> {
        let (head, mut records) = bytes.split_at_checked(HEAD)?;
        let &[moment, count, seen] = head else { return None };
        let moment = Moment::of(moment);
        if usize::from(count) > PER_PART || (more && count == 0) || bytes.len() > MAX_BODY {
            return None;
        }
        let mut entries = [EMPTY; PER_PART];
        for entry in entries.iter_mut().take(usize::from(count)) {
            (*entry, records) = Entry::parse(records)?;
        }
        records.is_empty().then_some(Part { moment, seen, entries, count })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::ToOwned;
    use std::vec::Vec;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().collect()
    }

    fn module(name: &str, folder: Folder, stamp: u32) -> Entry {
        let pe = stamp.is_multiple_of(2).then_some(PeMarks { stamp, checksum: 0x1234 });
        Entry::Module(Module { name: Name::of_path(&wide(name)).unwrap(), place: Place::bare(folder), image_size: 0x2_3000, pe })
    }

    /// A place in a folder of `len` letters.
    fn deep(folder: Folder, len: usize) -> Place {
        Place { folder, dir: Dir::of(&wide(&"d".repeat(len)), false) }
    }

    fn code(n: u32) -> Entry {
        Entry::Code(Code {
            memory: [Memory::Private, Memory::Mapped, Memory::UnlistedImage][n as usize % 3],
            protect: [Protect::Execute, Protect::ExecuteRead, Protect::ExecuteReadWrite, Protect::ExecuteWriteCopy][n as usize % 4],
            exec_size: 0x1000 * n,
            size: 0x2000 * n,
            start: (n % 2 == 1).then_some(n.wrapping_mul(0x9E37_79B9)),
            pe: n.is_multiple_of(3).then_some(PeMarks { stamp: n, checksum: 0 }),
        })
    }

    #[test]
    fn folders_compare_without_case_and_only_at_a_separator() {
        let game = wide("C:\\Games\\Skhid\\");
        let temp = wide("C:\\Users\\Ivan\\AppData\\Local\\Temp\\");
        let windows = wide("C:\\windows");
        let of = |path: &str| Place::of(&wide(path), &game, &temp, &windows).map(|place| (place.folder, place.dir.as_str().to_owned()));
        let at = |folder, dir: &str| Some((folder, dir.to_owned()));
        assert_eq!(of("C:\\Games\\Skhid\\bugtrap.dll"), at(Folder::Game, ""));
        assert_eq!(of("c:/games/skhid/Bin/dxvk/d3d9.dll"), at(Folder::Game, "Bin\\dxvk"));
        assert_eq!(of("C:\\Games\\SkhidOld\\d3d9.dll"), at(Folder::Other, "C:\\Games\\SkhidOld"));
        assert_eq!(of("C:\\Users\\Ivan\\AppData\\Local\\Temp\\added.dll"), at(Folder::Temp, ""));
        assert_eq!(of("C:\\Users\\Ivan\\AppData\\Local\\Temp\\7z\\x.dll"), at(Folder::Temp, "7z"));
        assert_eq!(of("C:\\Windows\\SysWOW64\\kernel32.dll"), None);
        assert_eq!(of("C:\\windows\\system32\\d3d9.dll"), None);
        assert_eq!(of("C:\\WindowsApps\\x.dll"), at(Folder::Other, "C:\\WindowsApps"));
        assert_eq!(of("D:\\Discord\\overlay.dll"), at(Folder::Other, "D:\\Discord"));
        assert_eq!(of("D:\\overlay.dll"), at(Folder::Other, "D:"));
        assert_eq!(Place::of(&wide("C:\\x.dll"), &[], &[], &[]).map(|place| place.folder), Some(Folder::Other));
    }

    #[test]
    fn a_users_name_is_left_out_of_a_folder() {
        let of = |path: &str| Place::of(&wide(path), &[], &[], &[]).unwrap().dir.as_str().to_owned();
        assert_eq!(of("C:\\Users\\Ivan\\Downloads\\tools\\x.dll"), "C:\\Users\\*\\Downloads\\tools");
        assert_eq!(of("d:/USERS/Иван Петров/Desktop/x.dll"), "d:\\USERS\\*\\Desktop");
        assert_eq!(of("Z:\\home\\sheep\\games\\x.dll"), "Z:\\home\\*\\games");
        assert_eq!(of("C:\\Documents and Settings\\Ivan\\x.dll"), "C:\\Documents and Settings\\*");
        assert_eq!(of("C:\\Users\\x.dll"), "C:\\Users", "a file there is no user's folder");
        assert_eq!(of("C:\\Games\\Users\\Ivan\\x.dll"), "C:\\Games\\Users\\Ivan", "only a drive's own");
        assert_eq!(of("C:\\a\u{7}b\\x.dll"), "C:\\a?b");
        let long = of(&std::format!("C:\\{}\\Чит\\x.dll", "a".repeat(DIR - 4)));
        assert_eq!(long, std::format!("C:\\{}\\", "a".repeat(DIR - 4)), "cut at a character");
        // below the game's folder a part named as a user's is as any other
        let game = wide("C:\\Users\\Ivan\\Skhid");
        let below = Place::of(&wide("C:\\Users\\Ivan\\Skhid\\home\\me\\x.dll"), &game, &[], &[]).unwrap();
        assert_eq!((below.folder, below.dir.as_str()), (Folder::Game, "home\\me"));
    }

    #[test]
    fn a_name_is_the_files_cut_at_a_character() {
        assert_eq!(Name::of_path(&wide("C:\\a\\b/Mixed.DLL")).unwrap().as_str(), "Mixed.DLL");
        assert_eq!(Name::of_path(&wide("plain.dll")).unwrap().as_str(), "plain.dll");
        assert_eq!(Name::of_path(&wide("C:\\a\\")), None);
        let long = Name::of_path(&wide("ЧитЧитЧитЧитЧитЧит.dll")).unwrap();
        assert_eq!(long.as_str(), "ЧитЧитЧитЧитЧитЧ");
        assert_eq!(Name::of_path(&wide("a\u{7}b.dll")).unwrap().as_str(), "a?b.dll");
        assert_eq!(Name::of_path(&[0x61, 0xD800, 0x62]).unwrap().as_str(), "a\u{FFFD}b");
    }

    #[test]
    fn pe_marks_need_mz_pe_and_a_32_bit_optional_header() {
        let mut dos = [0; 0x40];
        dos[..2].copy_from_slice(b"MZ");
        dos[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        let mut nt = [0; 92];
        nt[..4].copy_from_slice(b"PE\0\0");
        nt[8..12].copy_from_slice(&0x5F2A_1B3Cu32.to_le_bytes());
        nt[24..26].copy_from_slice(&0x10Bu16.to_le_bytes());
        nt[88..92].copy_from_slice(&0x77u32.to_le_bytes());
        let read = |at: u32| (at == 0x80).then_some(nt);
        assert_eq!(PeMarks::of(&dos, read), Some(PeMarks { stamp: 0x5F2A_1B3C, checksum: 0x77 }));
        assert_eq!(PeMarks::of(&dos, |_| None), None);
        let mut wide_header = nt;
        wide_header[24..26].copy_from_slice(&0x20Bu16.to_le_bytes());
        assert_eq!(PeMarks::of(&dos, |_| Some(wide_header)), None);
        let mut erased = dos;
        erased[0] = 0;
        assert_eq!(PeMarks::of(&erased, read), None);
    }

    #[test]
    fn the_scan_tells_unlisted_executable_allocations_once_each() {
        const COMMIT: u32 = 0x1000;
        const FREE: u32 = 0x1_0000;
        let region = |base: usize, allocation: usize, size: usize, state: u32, protect: u32, kind: u32| Region { base, allocation, size, state, protect, kind };
        let walk = [
            // the exe (listed), its code rx
            region(0x40_0000, 0x40_0000, 0x1000, COMMIT, 0x02, Memory::IMAGE),
            region(0x40_1000, 0x40_0000, 0x5000, COMMIT, 0x20, Memory::IMAGE),
            region(0x40_6000, 0, 0xA000, FREE, 0x01, 0),
            // a hand-mapped DLL: header page rw, code rwx, data rw
            region(0x100_0000, 0x100_0000, 0x1000, COMMIT, 0x04, Memory::PRIVATE),
            region(0x100_1000, 0x100_0000, 0x3000, COMMIT, 0x40, Memory::PRIVATE),
            region(0x100_4000, 0x100_0000, 0x1000, COMMIT, 0x04, Memory::PRIVATE),
            // data only: not told
            region(0x200_0000, 0x200_0000, 0x8000, COMMIT, 0x04, Memory::PRIVATE),
            // a guard page and reserved memory run nothing
            region(0x300_0000, 0x300_0000, 0x1000, COMMIT, 0x140, Memory::PRIVATE),
            region(0x300_1000, 0x300_0000, 0x1000, 0x2000, 0x40, Memory::PRIVATE),
            // an image no listed module is at, then a mapped view, the last
            region(0x500_0000, 0x500_0000, 0x2000, COMMIT, 0x20, Memory::IMAGE),
            region(0x600_0000, 0x600_0000, 0x1000, COMMIT, 0x10, Memory::MAPPED),
        ];
        let mut scan = CodeScan::default();
        let mut found: Vec<Found> = walk.iter().filter_map(|&r| scan.region(r, |base| base == 0x40_0000)).collect();
        found.extend(scan.end());
        assert_eq!(
            found,
            [
                Found { base: 0x100_0000, first_exec: 0x100_1000, memory: Memory::Private, protect: Protect::ExecuteReadWrite, exec_size: 0x3000, size: 0x5000 },
                Found { base: 0x500_0000, first_exec: 0x500_0000, memory: Memory::UnlistedImage, protect: Protect::ExecuteRead, exec_size: 0x2000, size: 0x2000 },
                Found { base: 0x600_0000, first_exec: 0x600_0000, memory: Memory::Mapped, protect: Protect::Execute, exec_size: 0x1000, size: 0x1000 },
            ]
        );
        assert_eq!(found[0].code(Some(&[0; START]), None).start, Some(fingerprint(&[0; START])));
        assert_eq!(found[2].code(None, None).start, None);
    }

    fn watched(n: u32) -> Entry {
        let name = Name::of_path(&wide(&std::format!("w{n}.dll"))).unwrap();
        let pe = n.is_multiple_of(2).then_some(PeMarks { stamp: n, checksum: 7 });
        let place = deep(Folder::Other, n as usize * 3 % (DIR + 1));
        match n % 4 {
            0 => Entry::Load(Load { name, place, image_size: 0x9000, pe, at: n, listed: n.is_multiple_of(10) }),
            1 => Entry::Over(Over { name: (n % 2 == 1).then_some(name), class: (n % 4 < 2).then_some(name), style: n, width: n as u16, height: 9, home: [Home::Unknown, Home::Windows, Home::At(place)][n as usize / 3 % 3] }),
            2 => Entry::Unseen(Unseen::ALL[n as usize % 2]),
            _ => {
                let hashed = File::Hashed(Identity::new(n, [n as u8; 32]).unwrap());
                Entry::Exe(Exe { name, place, file: [hashed, File::TooBig, File::Unread][n as usize / 4 % 3] })
            }
        }
    }

    #[test]
    fn the_watch_entries_round_trip_and_are_exact() {
        let mut report = Report::new(Moment::Ended);
        let all: Vec<Entry> = (0..84).map(watched).collect();
        all.iter().for_each(|&entry| report.add(entry));
        let parts = report.parts();
        let report = crate::report::Report::Modules(report);
        let mut heard = Vec::new();
        for part in 0..parts {
            heard.extend_from_slice(carried(&report, part).1.unwrap().entries());
        }
        assert_eq!(heard, all);
        // the first part's entries: a load (no folder text), a window, an unseen, a hashed exe
        let (bytes, _) = carried(&report, 0);
        let load = FIRST;
        let over = load + 1 + NAME + 9 + PE + 2;
        let unseen = over + 1 + NAME + NAME + 8 + 1;
        let exe = unseen + 2;
        let bad = |at: usize, value: u8| {
            let mut bytes = bytes.clone();
            bytes[at] = value;
            crate::report::Carrier::parse(&bytes)
        };
        assert_eq!(bad(load + 1 + NAME + 8, 2), None, "a load's listed flag");
        assert_eq!(bad(load + 1 + NAME + 9 + PE, 3), None, "a load's folder");
        assert_eq!(bad(load + 1 + NAME + 9 + PE + 1, 1), None, "a folder's text longer than it is");
        assert_eq!((bytes[over], bytes[unseen], bytes[exe]), (3, 4, 5));
        assert_eq!(bad(over + 2, 1), None, "a window's exe name after its end");
        assert_eq!(bad(over + 1 + NAME + 2, 1), None, "a window's class after its end");
        assert_eq!(bad(over + 1 + NAME + NAME + 8, 3), None, "where a window's exe is");
        assert_eq!(bad(unseen + 1, 2), None, "why nothing is told");
        let file = exe + 1 + NAME;
        assert_eq!(bad(file, 3), None, "what reading an exe's file gave");
        assert_eq!(bad(file, 1), None, "a hash after a file too big");
        assert_eq!(bad(file + 1, 0), None, "an empty file's hash");
        assert_eq!(bad(file + 4, 2), None, "a hash of more than is read");
    }

    #[test]
    fn windows_overlap_by_what_they_share() {
        let game = Rect { left: 100, top: 50, right: 1380, bottom: 770 };
        assert_eq!(game.overlap(game), Some((1280, 720)));
        assert_eq!(Rect { left: 0, top: 0, right: 2560, bottom: 1440 }.overlap(game), Some((1280, 720)));
        assert_eq!(Rect { left: 1300, top: 700, right: 3000, bottom: 800 }.overlap(game), Some((80, 70)));
        assert_eq!(Rect { left: 1380, top: 0, right: 2560, bottom: 1440 }.overlap(game), None, "beside it");
        assert_eq!(Rect { left: -32000, top: -32000, right: -31840, bottom: -31972 }.overlap(game), None, "minimised");
        let huge = Rect { left: i32::MIN, top: i32::MIN, right: i32::MAX, bottom: i32::MAX };
        assert_eq!(huge.overlap(huge), Some((u16::MAX, u16::MAX)));
        assert_eq!(Name::of(&wide("Chrome/Widget\\Win")).unwrap().as_str(), "Chrome/Widget\\Win");
    }

    #[test]
    fn a_report_of_the_longest_entries_fits_its_parts() {
        let mut report = Report::new(Moment::Loaded);
        let name = Name::of_path(&wide("x.dll")).unwrap();
        let all: Vec<Entry> = (0..MAX_ENTRIES as u32 + 3)
            .map(|at| {
                let home = Home::At(deep(Folder::Other, DIR));
                Entry::Over(Over { name: Some(name), class: None, style: at, width: 1, height: 1, home })
            })
            .collect();
        all.iter().for_each(|&entry| report.add(entry));
        assert_eq!(all[0].told(), LONGEST);
        assert_eq!(report.parts(), MAX_PARTS);
        let report = crate::report::Report::Modules(report);
        let heard: Vec<Entry> = (0..MAX_PARTS).flat_map(|part| carried(&report, part).1.unwrap().entries().to_vec()).collect();
        assert_eq!(heard, all[..MAX_ENTRIES]);
        // short entries: a part holds PER_PART of them and no more
        let mut report = Report::new(Moment::Loaded);
        (0..PER_PART as u32 + 1).for_each(|at| report.add(Entry::Unseen(Unseen::ALL[at as usize % 2])));
        assert_eq!(report.parts(), 2);
        let report = crate::report::Report::Modules(report);
        assert_eq!(carried(&report, 0).1.unwrap().entries().len(), PER_PART);
        assert_eq!(carried(&report, 1).1.unwrap().entries().len(), 1);
    }

    /// Part `part` of `report` as the server reads its carrier: the
    /// carrier's bytes and the part.
    fn carried(report: &crate::report::Report, part: u8) -> (Vec<u8>, Option<Part>) {
        let (bytes, len) = report.part(0x8123_4567, part).unwrap();
        let bytes = bytes[..len].to_vec();
        let read = crate::report::Carrier::parse(&bytes).map(|carrier| match carrier.body {
            crate::report::Body::Modules(part) => part,
            other => panic!("{other:?}"),
        });
        (bytes, read)
    }

    /// Where a carrier's records start: its header, the body's head.
    const FIRST: usize = 15 + HEAD;

    #[test]
    fn reports_round_trip_in_parts() {
        for count in [0, 1, 9, 10, 18, MAX_ENTRIES, MAX_ENTRIES + 7] {
            let mut report = Report::new(Moment::Ended);
            let all: Vec<Entry> = (0..count as u32)
                .map(|n| if n % 3 == 2 { code(n) } else { module(&std::format!("m{n}.dll"), Folder::Game, n) })
                .collect();
            all.iter().for_each(|&entry| report.add(entry));
            let parts = report.parts();
            let report = crate::report::Report::Modules(report);
            let mut heard = Vec::new();
            for part in 0..parts {
                let (bytes, read) = carried(&report, part);
                let read = read.unwrap();
                assert!(bytes.len() <= crate::report::MAX_SIZE, "one chunk");
                assert_eq!((read.moment, usize::from(read.seen)), (Moment::Ended, count.min(255)));
                heard.extend_from_slice(read.entries());
                let mut longer = bytes.clone();
                longer.push(0);
                assert_eq!(crate::report::Carrier::parse(&longer), None);
                if bytes.len() > FIRST {
                    assert_eq!(crate::report::Carrier::parse(&bytes[..bytes.len() - 1]), None);
                }
            }
            assert_eq!(report.part(0, parts), None);
            assert_eq!(heard, all[..count.min(MAX_ENTRIES)]);
        }
    }

    #[test]
    fn every_moment_has_a_byte_of_its_own() {
        use std::string::ToString;
        let mut moments = std::vec![Moment::Loaded, Moment::Ended];
        let mut minute = Some(Minute::FIRST);
        while let Some(at) = minute {
            moments.push(Moment::Minute(at));
            minute = at.next();
        }
        assert_eq!(moments.last(), Some(&Moment::Minute(Minute::LAST)));
        let bytes: std::collections::BTreeSet<u8> = moments.iter().map(|moment| moment.byte()).collect();
        assert_eq!((moments.len(), bytes.len()), (256, 256));
        assert!(moments.iter().all(|&moment| Moment::of(moment.byte()) == moment));
        assert_eq!(Moment::Minute(Minute::FIRST.next().unwrap()).to_string(), "minute 2");
        assert_eq!(Moment::Ended.to_string(), "ended");
    }

    #[test]
    fn a_part_is_exact() {
        let mut report = Report::new(Moment::Loaded);
        (0..30).for_each(|n| report.add(if n == 1 { code(1) } else { module("x.dll", Folder::Temp, n) }));
        let report = crate::report::Report::Modules(report);
        assert!(report.parts() > 1);
        let (bytes, read) = carried(&report, 0);
        assert!(read.is_some());
        let mut none = bytes[..FIRST].to_vec();
        none[15 + 1] = 0;
        assert_eq!(crate::report::Carrier::parse(&none), None, "a part before the last one tells something");
        let module = 1 + NAME + 4 + PE + 2;
        let module_at = FIRST;
        let code_at = FIRST + module;
        let bad = |at: usize, value: u8| {
            let mut bytes = bytes.clone();
            bytes[at] = value;
            crate::report::Carrier::parse(&bytes)
        };
        assert_eq!(bad(15 + 1, bytes[15 + 1] + 1), None, "a count of more than there is");
        assert_eq!(bad(module_at, 5), None, "a kind");
        assert_eq!(bad(module_at + 1, 0), None, "an empty name");
        assert_eq!(bad(module_at + 1 + 6, b'y'), None, "bytes after the name's end");
        assert_eq!(bad(module_at + 1 + NAME + 4, 2), None, "a pe flag");
        assert_eq!(bad(module_at + 1 + NAME + 4 + PE, 3), None, "a folder");
        let no_pe_at = code_at + 1 + 2 + 8 + 5 + PE + module;
        assert_eq!(bytes[no_pe_at + 1 + NAME + 4], 0);
        assert_eq!(bad(no_pe_at + 1 + NAME + 4 + 1, 1), None, "pe bytes without pe");
        assert_eq!(bad(code_at + 1, 3), None, "a memory kind");
        assert_eq!(bad(code_at + 2, 4), None, "a protection");
    }
}

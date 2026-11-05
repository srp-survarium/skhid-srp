// SPDX-License-Identifier: GPL-3.0-or-later
//! Reads the game's `resources.db`: its file table and the files in it.
//!
//! Taken from the reader of resource-porter's `deps/vostok-resources-db`
//! (`src/fat.rs` at resource-porter commit 5d3b53e; the code began as
//! github.com/srp-survarium/vostok-resources-db). Every offset is checked, so
//! a damaged archive is an error, never a panic, and nothing is allocated for
//! bytes the file does not have. Only the table is read when an archive is
//! opened; a file's bytes are read when asked for, so the 6.3 GB archive of
//! 0.26g is never copied.
//!
//! The file (little-endian, the 64-bit PC layout of the engine's
//! `saving_db.cpp`):
//!
//! ```text
//! [ header, 24 bytes ][ file table, buffer_size bytes ][ the files' bytes ]
//! ```
//!
//! The header is `char endian[14] = "little-endian"`, 2 bytes of padding,
//! `u32 num_nodes`, `u32 buffer_size`. The table is a tree of nodes; a node
//! "pointer" is a u64 offset into the table (0 is none). Every node class
//! embeds a `base_node`: the next sibling at +24, the folder it is in at +32
//! (the start of that folder's class, which the engine trusts), the flags at
//! +48, the NUL-ended name at +51. A pointer points at that `base_node`, which
//! is 16 bytes into a folder's class (whose first child is at its start)
//! and 24 into a file's (which starts `u32 size, u32 padding, u64 offset in the
//! file, u32 hash`). The root, the mount root at offset 0 of the table, names
//! its folder at +64 and keeps that folder's first child at +936.
//!
//! A hard link is another node at its own path, a file (its bytes) or a
//! folder (its children): the link's class starts with a pointer to that
//! node, 8 bytes before its `base_node`. A link to a folder it is in is
//! refused (its paths would not end), as is a path longer than the game
//! takes ([`MAX_PATH`]) and a name that is not UTF-8.
//!
//! The engine also knows compressed (PPMd) and inlined files, soft links,
//! erased nodes and external tables. 0.26g's archive has none (its 79 933
//! files are 78 903 nodes and 1 030 more through 58 links to folders), nor
//! have its patches so far, nor makes [`write`](mod@write) any, so this reader refuses
//! them rather than guess.
//!
//! [`overlay`] lays archives over one another as the game mounts
//! `resources.db` and its patches, `resources.db.patch_<n>`: a later
//! archive's file replaces an earlier one's, folders merge. [`server`] takes
//! from that the files the server reads into an archive of their own
//! ([`write`](mod@write)) that names the archives it was made of. [`pack`] makes a
//! patch, an archive of the files of a folder.

pub mod clips;
pub mod overlay;
pub mod pack;
pub mod server;
pub mod write;

use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};
use std::ops::Bound;

/// The header's size, where the table starts.
pub const HEADER_SIZE: u64 = 24;

/// Where the game mounts `resources.db` and its patches: a file's path in
/// the game's file system is this and its path in the archive.
pub const MOUNT_PATH: &str = "resources/";

/// The longest path of a file or folder in an archive that the game can
/// look up: the engine keeps a path in a `virtual_path_string`, a
/// `fixed_string<max_path_length>`, 260 chars with the NUL
/// (`vostok/fs/platform_configuration.h`; `buffer_string::append` asserts
/// the string stays shorter than its buffer), and `base_node::get_full_path`
/// puts [`MOUNT_PATH`] before it: 260 - 1 - 10 = 249 bytes. 0.26g's longest
/// is 157.
pub const MAX_PATH: usize = 260 - 1 - MOUNT_PATH.len();

const FOLDER: u16 = 1 << 0;
const ARCHIVE: u16 = 1 << 2;
const MOUNT_ROOT: u16 = 1 << 3;
const HARD_LINK: u16 = 1 << 9;
/// Compressed, inlined, soft link, erased, external table.
const NOT_READ: u16 = (1 << 4) | (1 << 6) | (1 << 8) | (1 << 11) | (1 << 12);

// `base_node` (pack(8)): fields from where a pointer points.
const NEXT: usize = 24;
const FLAGS: usize = 48;
const NAME: usize = 51;
/// Where `base_node` is in a folder's class, a file's and a hard link's.
const FOLDER_BASE: usize = 16;
const FILE_BASE: usize = 24;
const LINK_BASE: usize = 8;
/// The mount root: its folder's pointer, its first child, its `base_node`
/// (a folder's: its first child is [`FOLDER_BASE`] before it).
const ROOT_NODE: usize = 64;
const ROOT_FIRST_CHILD: usize = 936;
const ROOT_BASE: usize = ROOT_FIRST_CHILD + FOLDER_BASE;

/// Why an archive cannot be read or written.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Io(io::Error),
    /// Not the format above, or a damaged table.
    Format(String),
    /// Archives that cannot be laid over one another, or server game data
    /// that does not say what it was made of.
    Content(String),
    /// Files that cannot be written as an archive ([`write`](mod@write)): a
    /// path an archive cannot have, or bytes unlike the size and checksum
    /// given for them.
    Write(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(cause) => write!(f, "{cause}"),
            Error::Format(what) => write!(f, "not a readable resources.db: {what}"),
            Error::Content(what) => write!(f, "{what}"),
            Error::Write(what) => write!(f, "cannot write the archive: {what}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(cause: io::Error) -> Self {
        Error::Io(cause)
    }
}

fn format(what: impl Into<String>) -> Error {
    Error::Format(what.into())
}

/// The checksum the table keeps of each file's bytes ([`Entry::hash`]): the
/// engine's CRC-32 (`boost::crc_optimal<32, 0x04C11DB7>`, reflected input,
/// output not reflected, no final xor). 0.26g's archive and its patches have
/// it right for every file.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0;
    for &byte in data {
        crc = (crc >> 8) ^ CRC_TABLE[((crc ^ u32::from(byte)) & 0xFF) as usize];
    }
    crc.reverse_bits()
}

/// [`crc32`] a byte at a time: the CRC of each byte value, bit by bit.
const CRC_TABLE: [u32; 256] = {
    let mut table = [0; 256];
    let mut value = 0;
    while value < 256 {
        let mut crc = value as u32;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
            bit += 1;
        }
        table[value] = crc;
        value += 1;
    }
    table
};

/// A file of the archive: where its bytes are, how many, and the table's
/// checksum of them ([`crc32`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub offset: u64,
    pub size: u32,
    pub hash: u32,
}

/// An archive: its file table read; its files read when asked for.
pub struct Archive<R> {
    reader: R,
    /// How many bytes the reader has.
    len: u64,
    files: BTreeMap<String, Entry>,
    empty_folders: Vec<String>,
}

impl<R> fmt::Debug for Archive<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Archive")
            .field("files", &self.files.len())
            .finish_non_exhaustive()
    }
}

/// The table, read with bounds checks.
struct Table<'a>(&'a [u8]);

impl Table<'_> {
    fn bytes<const N: usize>(&self, at: usize) -> Result<[u8; N], Error> {
        at.checked_add(N)
            .and_then(|end| self.0.get(at..end))
            .map(|bytes| bytes.try_into().expect("N bytes"))
            .ok_or_else(|| format(format!("a field at {at} is outside the table")))
    }

    fn u16(&self, at: usize) -> Result<u16, Error> {
        self.bytes(at).map(u16::from_le_bytes)
    }

    fn u32(&self, at: usize) -> Result<u32, Error> {
        self.bytes(at).map(u32::from_le_bytes)
    }

    fn u64(&self, at: usize) -> Result<u64, Error> {
        self.bytes(at).map(u64::from_le_bytes)
    }

    /// A node pointer at `at`: a `base_node` in the table, or 0 for none.
    fn pointer(&self, at: usize) -> Result<usize, Error> {
        let value = self.u64(at)?;
        usize::try_from(value)
            .ok()
            .filter(|&p| p < self.0.len())
            .ok_or_else(|| {
                format(format!(
                    "the node pointer at {at} ({value}) leaves the table"
                ))
            })
    }

    fn name(&self, base: usize) -> Result<&[u8], Error> {
        let rest = self.0.get(base + NAME..).ok_or_else(|| {
            format(format!(
                "the name of the node at {base} is outside the table"
            ))
        })?;
        let end = rest
            .iter()
            .position(|&byte| byte == 0)
            .ok_or_else(|| format(format!("the name of the node at {base} has no end")))?;
        Ok(&rest[..end])
    }
}

impl<R: Read + Seek> Archive<R> {
    /// Reads the header and the file table of `reader`.
    pub fn new(mut reader: R) -> Result<Self, Error> {
        let len = reader.seek(SeekFrom::End(0))?;
        if len < HEADER_SIZE {
            return Err(format(format!(
                "{len} bytes, shorter than its {HEADER_SIZE}-byte header"
            )));
        }
        reader.seek(SeekFrom::Start(0))?;
        let mut header = [0; HEADER_SIZE as usize];
        reader.read_exact(&mut header)?;
        if &header[..14] != b"little-endian\0" {
            return Err(format("its header does not start \"little-endian\""));
        }
        let size = u32::from_le_bytes(header[20..24].try_into().expect("4 bytes"));
        if HEADER_SIZE + u64::from(size) > len {
            return Err(format(format!(
                "the file ends inside its table ({len} bytes, a table of {size} after the header)"
            )));
        }
        let mut table = vec![0; size as usize];
        reader.read_exact(&mut table).map_err(|cause| {
            if cause.kind() == io::ErrorKind::UnexpectedEof {
                format("the file ends inside its table")
            } else {
                Error::Io(cause)
            }
        })?;
        let (files, empty_folders) = index(&Table(&table))?;
        Ok(Self {
            reader,
            len,
            files,
            empty_folders,
        })
    }

    /// The bytes of the file at `path`; `None` if the archive has none.
    pub fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, Error> {
        let Some(&entry) = self.files.get(path) else {
            return Ok(None);
        };
        let past_end = || format(format!("{path}: its bytes end past the end of the file"));
        if entry
            .offset
            .checked_add(u64::from(entry.size))
            .is_none_or(|end| end > self.len)
        {
            return Err(past_end());
        }
        let mut bytes = vec![0; entry.size as usize];
        self.reader.seek(SeekFrom::Start(entry.offset))?;
        self.reader.read_exact(&mut bytes).map_err(|cause| {
            if cause.kind() == io::ErrorKind::UnexpectedEof {
                past_end()
            } else {
                Error::Io(cause)
            }
        })?;
        Ok(Some(bytes))
    }
}

impl<R> Archive<R> {
    /// The file at `path`.
    pub fn entry(&self, path: &str) -> Option<&Entry> {
        self.files.get(path)
    }

    /// How many files the archive has.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Every file whose path starts with `prefix`, in path order. (Every
    /// path is UTF-8: an archive with a name that is not is refused when it
    /// is opened.)
    pub fn files_under<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a Entry)> + 'a {
        self.files
            .range::<str, _>((Bound::Included(prefix), Bound::Unbounded))
            .take_while(move |(path, _)| path.starts_with(prefix))
            .map(|(path, entry)| (path.as_str(), entry))
    }

    /// The folders that hold nothing (the game probes some by their path),
    /// in the table's order. Each is UTF-8.
    pub fn empty_folders(&self) -> impl Iterator<Item = &[u8]> {
        self.empty_folders.iter().map(String::as_bytes)
    }
}

type Index = (BTreeMap<String, Entry>, Vec<String>);

/// A folder to walk: its first child, its path, where it is in
/// [`Entered`], and whether it was reached through a hard link.
struct Folder {
    first: usize,
    path: String,
    at: usize,
    linked: bool,
}

/// The folders entered: each one's `base_node` (a link's target's, for a
/// folder reached through a link) and the one it is in.
struct Entered(Vec<(usize, Option<usize>)>);

impl Entered {
    /// Whether the folder whose `base_node` is `base` is the folder `at` or
    /// one it is in.
    fn has(&self, base: usize, at: usize) -> bool {
        let mut at = Some(at);
        while let Some(index) = at {
            let (folder, parent) = self.0[index];
            if folder == base {
                return true;
            }
            at = parent;
        }
        false
    }

    fn enter(&mut self, base: usize, parent: usize) -> usize {
        self.0.push((base, Some(parent)));
        self.0.len() - 1
    }
}

/// Every file of the table, and every empty folder, by path.
///
/// A file reached through a hard link to a folder is listed at the link's
/// path. So a folder that is in itself, through a link or a looping
/// pointer, would list paths without end: it is refused when it is met, as
/// is a path longer than [`MAX_PATH`]. The nodes walked are counted: the
/// table's own (in a table whose pointers loop, over and over) up to the
/// most nodes the table can have, and as many again walked through links
/// (a link to a folder of links to a folder... is no loop, but lists ever
/// more). So what is listed is at most twice that many paths of at most
/// [`MAX_PATH`] bytes, whatever the table.
fn index(table: &Table<'_>) -> Result<Index, Error> {
    let mut root = table.pointer(ROOT_NODE)?;
    if root == 0 {
        root = ROOT_BASE;
    }
    let flags = table.u16(root + FLAGS)?;
    if flags & FOLDER == 0 {
        return Err(format("the root is not a folder"));
    }
    // A node takes more than FLAGS bytes of the table.
    let most = table.0.len() / FLAGS + 1;
    let (mut own, mut through_links) = (most, most);
    let mut files = BTreeMap::new();
    let mut empty_folders = Vec::new();
    let mut entered = Entered(vec![(root, None)]);
    let mut folders = vec![Folder {
        first: first_child(table, root)?,
        path: String::new(),
        at: 0,
        linked: false,
    }];
    while let Some(folder) = folders.pop() {
        let mut node = folder.first;
        while node != 0 {
            let (budget, why) = if folder.linked {
                (
                    &mut through_links,
                    "its links list more nodes than the table has",
                )
            } else {
                (&mut own, "the table's pointers loop")
            };
            *budget = budget.checked_sub(1).ok_or_else(|| format(why))?;
            let flags = table.u16(node + FLAGS)?;
            let name = table.name(node)?;
            let name = std::str::from_utf8(name).map_err(|_| {
                format(format!(
                    "{}/{}: a name that is not UTF-8",
                    folder.path,
                    String::from_utf8_lossy(name)
                ))
            })?;
            let mut path = String::with_capacity(folder.path.len() + 1 + name.len());
            path.push_str(&folder.path);
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(name);
            if path.len() > MAX_PATH {
                return Err(format(format!(
                    "{path}: a path of {} bytes, longer than the game takes ({MAX_PATH})",
                    path.len()
                )));
            }
            if flags & NOT_READ != 0 {
                return Err(format(format!(
                    "{path} is a compressed, inlined, soft-linked, erased or external node \
                     (flags {flags:#06x}), which is not read"
                )));
            }
            let (base, flags, link) =
                if flags & FOLDER == 0 && flags & ARCHIVE != 0 && flags & HARD_LINK != 0 {
                    let start = node.checked_sub(LINK_BASE).ok_or_else(|| {
                        format(format!("{path}: its node starts before the table"))
                    })?;
                    let target = table.pointer(start)?;
                    let target_flags = table.u16(target + FLAGS)?;
                    if target_flags & (HARD_LINK | NOT_READ) != 0 || target_flags & ARCHIVE == 0 {
                        return Err(format(format!(
                            "{path} links to a node of flags {target_flags:#06x}"
                        )));
                    }
                    (target, target_flags, true)
                } else {
                    (node, flags, false)
                };
            if flags & FOLDER != 0 {
                if entered.has(base, folder.at) {
                    return Err(format(if link {
                        format!("{path} links to a folder it is in: its paths would not end")
                    } else {
                        format!("{path} is a folder it is in: the table's pointers loop")
                    }));
                }
                match first_child(table, base)? {
                    0 => empty_folders.push(path),
                    first => {
                        let at = entered.enter(base, folder.at);
                        folders.push(Folder {
                            first,
                            path,
                            at,
                            linked: folder.linked || link,
                        });
                    }
                }
            } else if flags & ARCHIVE != 0 {
                let entry = file_entry(table, base, &path)?;
                if files.contains_key(&path) {
                    return Err(format(format!("{path} is in the table twice")));
                }
                files.insert(path, entry);
            } else {
                return Err(format(format!("{path}: a node of flags {flags:#06x}")));
            }
            node = table.pointer(node + NEXT)?;
        }
    }
    Ok((files, empty_folders))
}

/// The file whose `base_node` is at `base`.
fn file_entry(table: &Table<'_>, base: usize, shown: &str) -> Result<Entry, Error> {
    let start = base
        .checked_sub(FILE_BASE)
        .ok_or_else(|| format(format!("{shown}: its node starts before the table")))?;
    Ok(Entry {
        size: table.u32(start)?,
        offset: table.u64(start + 8)?,
        hash: table.u32(start + 16)?,
    })
}

/// A folder's first child (0: none), at the start of its class. (The mount
/// root's is [`ROOT_FIRST_CHILD`], [`FOLDER_BASE`] before its `base_node`,
/// as any folder's.)
fn first_child(table: &Table<'_>, base: usize) -> Result<usize, Error> {
    let at = base
        .checked_sub(FOLDER_BASE)
        .ok_or_else(|| format(format!("the folder at {base} starts before the table")))?;
    table.pointer(at)
}

#[cfg(test)]
mod tests;

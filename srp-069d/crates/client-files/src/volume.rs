//! One `resources.NN.db` volume of the 0.69d client.
//!
//! Layout, as read here (all numbers little-endian):
//!
//! ```text
//! 0   14-byte text "little-endian" + NUL, 2 bytes padding
//! 16  u32 node count
//! 20  u32 index size N
//! 24  the index: N bytes of node records; node links are byte offsets into the index (0 = none)
//! ..  file bytes; a file node gives the absolute file offset and the size of its bytes
//! ```
//!
//! Every node record has a common part that a link points at ("the node"): `+24` u64 link to the next sibling,
//! `+48` u16 flags, `+51` the NUL-terminated name. Fields in front of the node depend on the kind:
//! a folder keeps the link to its first child at `node - 16`; a stored file keeps `u32 size, u32 pad, u64 offset` at
//! `node - 24`; a link node keeps the linked node at `node - 8`. The index starts with the volume's root record,
//! whose u64 at index offset 64 links to the root folder node.
//!
//! Flags used: 0x1 folder, 0x4 stored in the volume, 0x10 compressed, 0x40 bytes kept inside the index, 0x100 soft
//! link, 0x200 hard link, 0x800 erased (a later volume removed the path), 0x1000 external index. The 0.69d volumes
//! hold only plain stored files and erased entries; compressed and in-index files are listed but not read.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::{Error, Result};

const MAGIC: &[u8; 13] = b"little-endian";
const HEADER: usize = 24;
const ROOT_LINK: usize = 64;
const NEXT: usize = 24;
const FLAGS: usize = 48;
const NAME: usize = 51;

const F_FOLDER: u16 = 0x1;
const F_STORED: u16 = 0x4;
const F_COMPRESSED: u16 = 0x10;
const F_IN_INDEX: u16 = 0x40;
const F_SOFT_LINK: u16 = 0x100;
const F_HARD_LINK: u16 = 0x200;
const F_ERASED: u16 = 0x800;
const F_EXTERNAL: u16 = 0x1000;

/// What a path is in this volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A file stored in the volume: absolute offset and size of its bytes.
    File { offset: u64, size: u32 },
    /// A file this reader does not read (compressed or kept inside the index).
    Packed,
    /// The path was removed by this volume.
    Erased,
    /// A soft link, an external index or an unknown node.
    Other,
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Path inside the volume with `/` separators, e.g. `gameplay/game.options`.
    pub path: String,
    pub kind: Kind,
}

/// An opened volume: its path and its file list (the index is read once and dropped).
#[derive(Debug)]
pub struct Volume {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
}

fn u16_at(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], o: usize) -> Option<usize> {
    usize::try_from(u64::from_le_bytes(b.get(o..o + 8)?.try_into().ok()?)).ok()
}

impl Volume {
    /// Read the header and the index of `path` and list every node below the root.
    pub fn open(path: &Path) -> Result<Volume> {
        let io = |source| Error::Io { path: path.display().to_string(), source };
        let mut f = File::open(path).map_err(io)?;
        let mut head = [0u8; HEADER];
        f.read_exact(&mut head).map_err(io)?;
        if &head[..13] != MAGIC {
            return Err(Error::format(path.display().to_string(), "not a resources volume (no little-endian header)"));
        }
        let nodes = u32_at(&head, 16).unwrap_or(0) as usize;
        let size = u32_at(&head, 20).unwrap_or(0) as usize;
        let mut index = vec![0u8; size];
        f.read_exact(&mut index).map_err(io)?;
        let entries = list(&index, nodes).map_err(|why| Error::format(path.display().to_string(), why))?;
        Ok(Volume { path: path.to_path_buf(), entries })
    }

    /// The bytes of a stored file.
    pub fn read(&self, e: &Entry) -> Result<Vec<u8>> {
        read_at(&self.path, &e.path, e.kind)
    }
}

/// Read a file's bytes from the volume at `vol` given its entry kind.
pub(crate) fn read_at(vol: &Path, name: &str, kind: Kind) -> Result<Vec<u8>> {
    let Kind::File { offset, size } = kind else {
        return Err(Error::format(format!("{} in {}", name, vol.display()), "not a plain stored file"));
    };
    let io = |source| Error::Io { path: vol.display().to_string(), source };
    let mut f = File::open(vol).map_err(io)?;
    f.seek(SeekFrom::Start(offset)).map_err(io)?;
    let mut b = vec![0u8; size as usize];
    f.read_exact(&mut b).map_err(io)?;
    Ok(b)
}

/// Walk the index from the root folder. `nodes` bounds the walk (a damaged index cannot loop forever).
pub fn list(index: &[u8], nodes: usize) -> std::result::Result<Vec<Entry>, String> {
    let root = u64_at(index, ROOT_LINK).ok_or("index too short for the root record")?;
    if root == 0 || root >= index.len() {
        return Err(format!("root link {root:#x} outside the index"));
    }
    let mut out = Vec::new();
    let mut budget = nodes.saturating_mul(2).max(16);
    // (folder node, its path)
    let mut stack: Vec<(usize, String)> = vec![(root, String::new())];
    while let Some((folder, prefix)) = stack.pop() {
        let first = folder.checked_sub(16).and_then(|o| u64_at(index, o)).ok_or("folder record outside the index")?;
        let mut node = first;
        let mut kids = Vec::new();
        while node != 0 {
            budget = budget.checked_sub(1).ok_or("more nodes than the header counts (a loop?)")?;
            let flags = u16_at(index, node + FLAGS).ok_or("node outside the index")?;
            let name_end = index.get(node + NAME..).and_then(|s| s.iter().position(|&c| c == 0)).ok_or("node name not terminated")?;
            let name = String::from_utf8_lossy(&index[node + NAME..node + NAME + name_end]).into_owned();
            let path = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            if flags & F_FOLDER != 0 {
                kids.push((node, path));
            } else {
                let kind = kind_of(index, node, flags, 0)?;
                out.push(Entry { path, kind });
            }
            node = u64_at(index, node + NEXT).ok_or("sibling link outside the index")?;
        }
        // keep the index order: children are visited in sibling order
        stack.extend(kids.into_iter().rev());
    }
    Ok(out)
}

fn kind_of(index: &[u8], node: usize, flags: u16, depth: u32) -> std::result::Result<Kind, String> {
    if flags & F_SOFT_LINK != 0 || flags & F_EXTERNAL != 0 {
        return Ok(Kind::Other);
    }
    if flags & F_HARD_LINK != 0 {
        let target = node.checked_sub(8).and_then(|o| u64_at(index, o)).ok_or("hard link outside the index")?;
        if target == 0 || depth > 8 {
            return Ok(Kind::Other);
        }
        let tf = u16_at(index, target + FLAGS).ok_or("hard link target outside the index")?;
        return kind_of(index, target, tf, depth + 1);
    }
    if flags & F_ERASED != 0 {
        return Ok(Kind::Erased);
    }
    if flags & F_STORED == 0 {
        return Ok(Kind::Other);
    }
    if flags & (F_COMPRESSED | F_IN_INDEX) != 0 {
        return Ok(Kind::Packed);
    }
    let start = node.checked_sub(24).ok_or("file record outside the index")?;
    let size = u32_at(index, start).ok_or("file record outside the index")?;
    let offset = u64_at(index, start + 8).ok_or("file record outside the index")? as u64;
    Ok(Kind::File { offset, size })
}

/// Build a volume in memory (tests and fixtures): `files` = (path, bytes), `erased` = paths marked removed.
/// Folders are made from the paths; the layout is the one [`list`] reads.
pub fn build(files: &[(&str, &[u8])], erased: &[&str]) -> Vec<u8> {
    // a tree of folders: name -> (sub folders, files)
    #[derive(Default)]
    struct Dir {
        dirs: Vec<(String, Dir)>,
        files: Vec<(String, Option<Vec<u8>>)>,
    }
    fn add(d: &mut Dir, parts: &[&str], data: Option<Vec<u8>>) {
        if parts.len() == 1 {
            d.files.push((parts[0].to_string(), data));
            return;
        }
        let i = match d.dirs.iter().position(|(n, _)| n == parts[0]) {
            Some(i) => i,
            None => {
                d.dirs.push((parts[0].to_string(), Dir::default()));
                d.dirs.len() - 1
            }
        };
        add(&mut d.dirs[i].1, &parts[1..], data);
    }
    let mut root = Dir::default();
    for (p, b) in files {
        add(&mut root, &p.split('/').collect::<Vec<_>>(), Some(b.to_vec()));
    }
    for p in erased {
        add(&mut root, &p.split('/').collect::<Vec<_>>(), None);
    }
    // index: [root record 72 bytes][records...]; a record = prefix (24 bytes) + node (51 bytes + name + NUL), 8-aligned
    let mut index = vec![0u8; 72];
    let mut blobs: Vec<(usize, Vec<u8>)> = Vec::new(); // (record start, bytes) to patch with offsets later
    fn put(index: &mut Vec<u8>, name: &str, flags: u16) -> usize {
        while index.len() % 8 != 0 {
            index.push(0);
        }
        let start = index.len();
        index.resize(start + 24 + NAME, 0);
        let node = start + 24;
        index[node + FLAGS..node + FLAGS + 2].copy_from_slice(&flags.to_le_bytes());
        index.extend_from_slice(name.as_bytes());
        index.push(0);
        node
    }
    fn link(index: &mut [u8], at: usize, to: usize) {
        index[at..at + 8].copy_from_slice(&(to as u64).to_le_bytes());
    }
    fn emit(index: &mut Vec<u8>, blobs: &mut Vec<(usize, Vec<u8>)>, d: Dir, name: &str) -> usize {
        let me = put(index, name, F_FOLDER);
        let mut prev: Option<usize> = None;
        let mut nodes = Vec::new();
        for (n, sub) in d.dirs {
            nodes.push(emit(index, blobs, sub, &n));
        }
        for (n, data) in d.files {
            match data {
                Some(b) => {
                    let node = put(index, &n, F_STORED);
                    index[node - 24..node - 20].copy_from_slice(&(b.len() as u32).to_le_bytes());
                    blobs.push((node - 24, b));
                    nodes.push(node);
                }
                None => nodes.push(put(index, &n, F_ERASED)),
            }
        }
        for n in nodes {
            match prev {
                None => link(index, me - 16, n),
                Some(p) => link(index, p + NEXT, n),
            }
            prev = Some(n);
        }
        me
    }
    let r = emit(&mut index, &mut blobs, root, "");
    link(&mut index, ROOT_LINK, r);
    let mut data_at = (HEADER + index.len()) as u64;
    let mut data = Vec::new();
    for (rec, b) in blobs {
        index[rec + 8..rec + 16].copy_from_slice(&data_at.to_le_bytes());
        data_at += b.len() as u64;
        data.extend_from_slice(&b);
    }
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&[0, 0, 0]);
    out.extend_from_slice(&((files.len() + erased.len() + 8) as u32).to_le_bytes());
    out.extend_from_slice(&(index.len() as u32).to_le_bytes());
    out.extend_from_slice(&index);
    out.extend_from_slice(&data);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_built_volume_lists_and_reads_back() {
        let v = build(&[("a/b.options", b"hello"), ("a/c/d.txt", b"xy"), ("top", b"")], &["a/gone.options"]);
        let dir = std::env::temp_dir().join(format!("client-files-vol-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("resources.00.db");
        std::fs::write(&p, &v).unwrap();
        let vol = Volume::open(&p).unwrap();
        let names: Vec<_> = vol.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(names, ["top", "a/b.options", "a/gone.options", "a/c/d.txt"]);
        let b = vol.entries.iter().find(|e| e.path == "a/b.options").unwrap();
        assert_eq!(vol.read(b).unwrap(), b"hello");
        let d = vol.entries.iter().find(|e| e.path == "a/c/d.txt").unwrap();
        assert_eq!(vol.read(d).unwrap(), b"xy");
        assert_eq!(vol.entries.iter().find(|e| e.path == "a/gone.options").unwrap().kind, Kind::Erased);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_damaged_index_is_an_error_not_a_panic() {
        let mut v = build(&[("a/b", b"1")], &[]);
        // point the root link past the end
        v[HEADER + ROOT_LINK..HEADER + ROOT_LINK + 8].copy_from_slice(&u64::MAX.to_le_bytes());
        let size = u32::from_le_bytes(v[20..24].try_into().unwrap()) as usize;
        assert!(list(&v[HEADER..HEADER + size], 4).is_err());
        let mut v = build(&[("a/b", b"1")], &[]);
        v[0] = b'B';
        let p = std::env::temp_dir().join(format!("client-files-bad-{}.db", std::process::id()));
        std::fs::write(&p, &v).unwrap();
        assert!(Volume::open(&p).is_err());
        std::fs::remove_file(&p).ok();
    }
}

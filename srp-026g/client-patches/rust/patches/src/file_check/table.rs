//! A file's place in one archive (`resources.db` or a patch), found in its
//! file table as the game's mount reads it, read through a few pages of the
//! table at a time ([`Pages`]): a lookup reads only the nodes on its way.
//! The format is `resources-db`'s (srp-026g/crates/resources-db, whose
//! reader the server's data is made with): a header, the table, the files'
//! bytes; a node pointer is a u64 offset into the table; a folder's first
//! child at its class's start, 16 bytes before its `base_node`; a file's
//! `u32 size, u32, u64 offset in the file, u32 crc` 24 bytes before it; a
//! hard link's target 8 bytes before it; the next sibling at +24, the flags
//! at +48, the name at +51.
#![forbid(unsafe_code)]

/// An archive file read at an offset.
pub trait Source {
    /// Up to `into.len()` bytes at `offset`: how many it read (fewer only
    /// at the file's end); none when it could not.
    fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Option<usize>;
}

pub const HEADER: usize = 24;
const FOLDER: u16 = 1 << 0;
const ARCHIVE: u16 = 1 << 2;
const HARD_LINK: u16 = 1 << 9;
/// Compressed, inlined, soft link, erased, external table: none in the
/// players' archives, which `resources-db` refuses.
const NOT_READ: u16 = (1 << 4) | (1 << 6) | (1 << 8) | (1 << 11) | (1 << 12);
const NEXT: usize = 24;
const FLAGS: usize = 48;
const NAME: usize = 51;
const FOLDER_BASE: usize = 16;
const FILE_BASE: usize = 24;
const LINK_BASE: usize = 8;
const ROOT_NODE: usize = 64;
const ROOT_BASE: usize = 936 + FOLDER_BASE;
/// A name's bytes at most (the game's paths are 249 at most).
pub const MAX_NAME: usize = 249;

/// One page of the table.
pub const PAGE: usize = 4096;

/// Where a file's bytes are in its archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place {
    pub offset: u64,
    pub size: u32,
}

/// What a lookup found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Found {
    File(Place),
    /// No file there (no node, or a folder).
    Nothing,
    /// The table could not be read or is not one the game reads.
    Unreadable,
}

/// The table of one archive, its pages read as a lookup needs them, the
/// last few kept (`slots`: `slots.len() / PAGE` pages).
pub struct Pages<S, B> {
    source: S,
    /// The table's bytes in all.
    len: usize,
    slots: B,
    /// The page each slot holds, and when it was last used.
    held: [(Option<usize>, u32); MAX_SLOTS],
    clock: u32,
    /// Nodes a lookup may visit before it is taken for a loop.
    most: usize,
}

/// Slots at most.
pub const MAX_SLOTS: usize = 64;

impl<S: Source, B: AsMut<[u8]>> Pages<S, B> {
    /// The archive `source`'s table, its header read; none for a file that
    /// is not an archive. `slots` is the cache: a whole number of pages.
    pub fn open(mut source: S, mut slots: B) -> Option<Self> {
        let mut header = [0u8; HEADER];
        if source.read_at(0, &mut header)? != HEADER || header.get(..14)? != b"little-endian\0" {
            return None;
        }
        let len = u32::from_le_bytes(header.get(20..24)?.try_into().ok()?) as usize;
        if slots.as_mut().len() / PAGE == 0 {
            return None;
        }
        Some(Pages { source, len, slots, held: [(None, 0); MAX_SLOTS], clock: 0, most: len / FLAGS + 1 })
    }

    /// The original archive index, including file names, offsets, sizes and CRCs.
    pub fn table(&self) -> Place {
        Place { offset: HEADER as u64, size: self.len as u32 }
    }

    /// The archive file and the cache, given back.
    pub fn into_parts(self) -> (S, B) {
        (self.source, self.slots)
    }

    fn count(&mut self) -> usize {
        (self.slots.as_mut().len() / PAGE).min(MAX_SLOTS)
    }

    /// The page `page` of the table, read if no slot holds it.
    fn page(&mut self, page: usize) -> Option<&[u8]> {
        self.clock = self.clock.wrapping_add(1);
        let count = self.count();
        let held = self.held.get_mut(..count)?;
        let slot = match held.iter().position(|(at, _)| *at == Some(page)) {
            Some(slot) => slot,
            None => {
                let slot = held.iter().enumerate().min_by_key(|(_, (_, used))| *used)?.0;
                let start = page.checked_mul(PAGE)?;
                let want = self.len.checked_sub(start)?.min(PAGE);
                let into = self.slots.as_mut().get_mut(slot * PAGE..slot * PAGE + want)?;
                let offset = (HEADER as u64).checked_add(start as u64)?;
                // A failed read leaves the slot empty: tried again next time.
                held.get_mut(slot)?.0 = None;
                if self.source.read_at(offset, into)? != want {
                    return None;
                }
                held.get_mut(slot)?.0 = Some(page);
                slot
            }
        };
        held.get_mut(slot)?.1 = self.clock;
        let end = self.len.checked_sub(page * PAGE)?.min(PAGE);
        self.slots.as_mut().get(slot * PAGE..slot * PAGE + end)
    }

    /// `into.len()` bytes of the table at `at`, across pages.
    fn bytes(&mut self, at: usize, into: &mut [u8]) -> Option<()> {
        let end = at.checked_add(into.len())?;
        if end > self.len {
            return None;
        }
        let mut done = 0;
        while done < into.len() {
            let here = at + done;
            let page = self.page(here / PAGE)?;
            let from = here % PAGE;
            let n = (page.len().checked_sub(from)?).min(into.len() - done);
            if n == 0 {
                return None;
            }
            into.get_mut(done..done + n)?.copy_from_slice(page.get(from..from + n)?);
            done += n;
        }
        Some(())
    }

    fn u16(&mut self, at: usize) -> Option<u16> {
        let mut b = [0; 2];
        self.bytes(at, &mut b)?;
        Some(u16::from_le_bytes(b))
    }

    fn u32(&mut self, at: usize) -> Option<u32> {
        let mut b = [0; 4];
        self.bytes(at, &mut b)?;
        Some(u32::from_le_bytes(b))
    }

    fn u64(&mut self, at: usize) -> Option<u64> {
        let mut b = [0; 8];
        self.bytes(at, &mut b)?;
        Some(u64::from_le_bytes(b))
    }

    /// A node pointer at `at`: inside the table (0: none).
    fn pointer(&mut self, at: usize) -> Option<usize> {
        let p = usize::try_from(self.u64(at)?).ok()?;
        (p < self.len).then_some(p)
    }

    /// Whether the node at `base` is named `name` (its NUL-ended name).
    fn named(&mut self, base: usize, name: &[u8]) -> Option<bool> {
        let mut stored = [0u8; MAX_NAME + 1];
        let want = name.len().checked_add(1)?;
        let at = base.checked_add(NAME)?;
        // A name nearer the table's end than `want` bytes is shorter.
        let room = self.len.checked_sub(at)?.min(want);
        let stored = stored.get_mut(..room)?;
        self.bytes(at, stored)?;
        Some(stored.len() == want && stored.get(..name.len())? == name && stored.last() == Some(&0))
    }

    /// The node a link at `base` points to, or the node itself.
    fn followed(&mut self, base: usize) -> Option<(usize, u16)> {
        let flags = self.u16(base.checked_add(FLAGS)?)?;
        if flags & NOT_READ != 0 {
            return None;
        }
        if flags & FOLDER == 0 && flags & ARCHIVE != 0 && flags & HARD_LINK != 0 {
            let target = self.pointer(base.checked_sub(LINK_BASE)?)?;
            let target_flags = self.u16(target.checked_add(FLAGS)?)?;
            if target_flags & (HARD_LINK | NOT_READ) != 0 || target_flags & ARCHIVE == 0 {
                return None;
            }
            return Some((target, target_flags));
        }
        Some((base, flags))
    }

    /// The file at `path` (`a/b/c`, as the archive names it).
    pub fn find(&mut self, path: &[u8]) -> Found {
        match self.walk(path) {
            Some(Some(place)) => Found::File(place),
            Some(None) => Found::Nothing,
            None => Found::Unreadable,
        }
    }

    fn walk(&mut self, path: &[u8]) -> Option<Option<Place>> {
        let mut folder = match self.pointer(ROOT_NODE)? {
            0 => ROOT_BASE,
            root => root,
        };
        if self.u16(folder.checked_add(FLAGS)?)? & FOLDER == 0 {
            return None;
        }
        let mut budget = self.most;
        let mut names = path.split(|&byte| byte == b'/').peekable();
        while let Some(name) = names.next() {
            if name.is_empty() || name.len() > MAX_NAME {
                return Some(None);
            }
            let mut node = self.pointer(folder.checked_sub(FOLDER_BASE)?)?;
            let found = loop {
                if node == 0 {
                    break None;
                }
                budget = budget.checked_sub(1)?;
                if self.named(node, name)? {
                    break Some(node);
                }
                node = self.pointer(node.checked_add(NEXT)?)?;
            };
            let Some(node) = found else { return Some(None) };
            let (base, flags) = self.followed(node)?;
            let last = names.peek().is_none();
            match (flags & FOLDER != 0, last) {
                (true, false) => folder = base,
                (false, true) if flags & ARCHIVE != 0 => {
                    let start = base.checked_sub(FILE_BASE)?;
                    let size = self.u32(start)?;
                    let offset = self.u64(start.checked_add(8)?)?;
                    return Some(Some(Place { offset, size }));
                }
                // A folder at the path's end, a file in the middle of it.
                _ => return Some(None),
            }
        }
        Some(None)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    extern crate std;
    use std::vec::Vec;

    use super::*;

    /// An archive file in memory, counting its reads.
    pub struct Memory {
        pub bytes: Vec<u8>,
        pub reads: usize,
    }

    impl Source for &mut Memory {
        fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Option<usize> {
            self.reads += 1;
            let start = usize::try_from(offset).ok()?;
            let rest = self.bytes.get(start..)?;
            let n = rest.len().min(into.len());
            into[..n].copy_from_slice(&rest[..n]);
            Some(n)
        }
    }

    /// A table as the format lays it out: the mount root, then each node;
    /// `entries` are paths, a file's with its bytes, a link's with its
    /// target's path.
    pub enum Node<'a> {
        Folder(&'a str),
        File(&'a str, &'a [u8]),
        Link(&'a str, &'a str),
    }

    /// The archive of `nodes` (its folders listed before what they hold).
    pub fn archive(nodes: &[Node]) -> Vec<u8> {
        let mut table = std::vec![0u8; ROOT_BASE + NAME + 1];
        table[ROOT_BASE + FLAGS..ROOT_BASE + FLAGS + 2].copy_from_slice(&FOLDER.to_le_bytes());
        let mut bases: std::collections::BTreeMap<std::string::String, usize> = Default::default();
        bases.insert(std::string::String::new(), ROOT_BASE);
        let mut data = Vec::new();
        let mut links = Vec::new();
        for node in nodes {
            let (path, flags, class) = match node {
                // A folder of an archive is marked an archive's node too.
                Node::Folder(path) => (*path, FOLDER | ARCHIVE, FOLDER_BASE),
                Node::File(path, _) => (*path, ARCHIVE, FILE_BASE),
                Node::Link(path, _) => (*path, ARCHIVE | HARD_LINK, LINK_BASE),
            };
            let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
            let start = table.len();
            let base = start + class;
            table.resize(base + NAME + name.len() + 1, 0);
            table[base + FLAGS..base + FLAGS + 2].copy_from_slice(&flags.to_le_bytes());
            table[base + NAME..base + NAME + name.len()].copy_from_slice(name.as_bytes());
            // Put first in its folder's list: the folder's first child is
            // at its class's start, the old first the new one's next.
            let folder = bases[parent];
            let first = folder - FOLDER_BASE;
            let old = table[first..first + 8].to_vec();
            table[base + NEXT..base + NEXT + 8].copy_from_slice(&old);
            table[first..first + 8].copy_from_slice(&(base as u64).to_le_bytes());
            match node {
                Node::File(_, bytes) => data.push((start, *bytes)),
                Node::Link(_, target) => links.push((start, *target)),
                Node::Folder(_) => {}
            }
            bases.insert(path.into(), base);
        }
        for (start, target) in links {
            table[start..start + 8].copy_from_slice(&(bases[target] as u64).to_le_bytes());
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"little-endian\0\0\0");
        out.extend_from_slice(&(nodes.len() as u32).to_le_bytes());
        out.extend_from_slice(&(table.len() as u32).to_le_bytes());
        let files_at = HEADER + table.len();
        let mut files = Vec::new();
        for (start, bytes) in data {
            let offset = (files_at + files.len()) as u64;
            table[start..start + 4].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
            table[start + 8..start + 16].copy_from_slice(&offset.to_le_bytes());
            files.extend_from_slice(bytes);
        }
        out.extend_from_slice(&table);
        out.extend_from_slice(&files);
        out
    }

    fn game() -> Vec<u8> {
        let mut nodes = std::vec![
            Node::Folder("textures"),
            Node::Folder("textures/flora"),
            Node::File("textures/flora/grass.dds", b"dds bytes"),
            Node::File("startup.cfg", b"cfg"),
            Node::Folder("models"),
            Node::Folder("models/a.model"),
            Node::File("models/a.model/settings", b"settings"),
            Node::Link("models/b.model", "models/a.model"),
            Node::Link("startup_copy.cfg", "startup.cfg"),
        ];
        // Enough siblings that the folder's list spans pages.
        let names: Vec<std::string::String> = (0..300).map(|n| std::format!("textures/flora/t{n:03}.dds")).collect();
        for name in &names {
            nodes.push(Node::File(name, b"t"));
        }
        archive(&nodes)
    }

    fn read(memory: &Memory, found: Found) -> Option<&[u8]> {
        let Found::File(place) = found else { return None };
        let start = place.offset as usize;
        memory.bytes.get(start..start + place.size as usize)
    }

    #[test]
    fn a_path_is_found_through_folders_and_links() {
        let mut memory = Memory { bytes: game(), reads: 0 };
        let mut slots = std::vec![0u8; 4 * PAGE];
        let mut found = Vec::new();
        {
            let mut pages = Pages::open(&mut memory, &mut slots[..]).unwrap();
            for path in [
                "textures/flora/grass.dds",
                "startup.cfg",
                "models/a.model/settings",
                "models/b.model/settings",
                "startup_copy.cfg",
                "textures/flora/t299.dds",
            ] {
                found.push(pages.find(path.as_bytes()));
            }
        }
        let read: Vec<Option<&[u8]>> = found.iter().map(|f| read(&memory, *f)).collect();
        assert_eq!(
            read,
            [Some(&b"dds bytes"[..]), Some(b"cfg"), Some(b"settings"), Some(b"settings"), Some(b"cfg"), Some(b"t")]
        );
    }

    #[test]
    fn what_is_not_a_file_there_is_nothing() {
        let mut memory = Memory { bytes: game(), reads: 0 };
        let mut slots = std::vec![0u8; 2 * PAGE];
        let mut pages = Pages::open(&mut memory, &mut slots[..]).unwrap();
        for path in [
            "textures/flora",
            "textures/flora/none.dds",
            "startup.cfg/more",
            "none/startup.cfg",
            "textures//flora/grass.dds",
            "",
        ] {
            assert_eq!(pages.find(path.as_bytes()), Found::Nothing, "{path}");
        }
    }

    #[test]
    fn a_damaged_table_is_unreadable_never_a_loop() {
        let mut bytes = game();
        // The root's first child its own next: a loop.
        let first = HEADER + ROOT_BASE - FOLDER_BASE;
        let child = u64::from_le_bytes(bytes[first..first + 8].try_into().unwrap()) as usize;
        let next = HEADER + child + NEXT;
        bytes[next..next + 8].copy_from_slice(&(child as u64).to_le_bytes());
        let mut memory = Memory { bytes, reads: 0 };
        let mut slots = std::vec![0u8; PAGE];
        let mut pages = Pages::open(&mut memory, &mut slots[..]).unwrap();
        assert_eq!(pages.find(b"none"), Found::Unreadable);
        // Cut short: pages past its end cannot be read.
        let mut short = game();
        short.truncate(HEADER + 1000);
        let mut memory = Memory { bytes: short, reads: 0 };
        let mut pages = Pages::open(&mut memory, &mut slots[..]).unwrap();
        assert_eq!(pages.find(b"startup.cfg"), Found::Unreadable);
        // Not an archive.
        let mut memory = Memory { bytes: b"not an archive at all, no".to_vec(), reads: 0 };
        assert!(Pages::open(&mut memory, &mut slots[..]).is_none());
    }

    #[test]
    fn a_page_read_once_is_kept() {
        let mut memory = Memory { bytes: game(), reads: 0 };
        let mut slots = std::vec![0u8; 8 * PAGE];
        {
            let mut pages = Pages::open(&mut memory, &mut slots[..]).unwrap();
            for _ in 0..10 {
                assert!(matches!(pages.find(b"textures/flora/t150.dds"), Found::File(_)));
            }
        }
        // The header, then each page on the way once.
        assert!(memory.reads <= 1 + 8, "{} reads", memory.reads);
    }
}

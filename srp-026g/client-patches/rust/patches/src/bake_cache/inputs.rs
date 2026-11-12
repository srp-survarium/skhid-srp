//! What the baked textures were made of: a fingerprint of the game data a
//! bake reads, so the folder is kept across releases until that changes.
//!
//! A baked file's name says what varies from item to item (the model and
//! its surface, the camouflage, the decals' values); what it does not say
//! is the same for every bake of one game data: the models, the materials,
//! the textures, the shaders and the camouflage and decal items
//! ([`FOLDERS`]). The game data is `resources.db`, never changed, and its
//! patches `resources.db.patch_<n>`, each released once and never changed,
//! mounted in order over it (`mount_patches_026g.asm`). So the data a bake
//! reads changed exactly when a patch has a file under [`FOLDERS`]: the
//! fingerprint folds each such patch's number and its files (path, size and
//! the table's CRC of the bytes) in the patch's file table. A patch with
//! none of them (Flash movies, item stats, maps, sounds) leaves it as it was.
//!
//! The file table's format: `resources-db` (srp-026g/crates), whose reader
//! this follows. Anything that reader would refuse, or a hard link (no
//! patch has one), makes the whole table the patch's fingerprint, as does
//! a patch whose table cannot be read: its number alone. Either changes the
//! fingerprint whenever such a patch comes, which deletes the folder: a
//! fingerprint that is wrong only ever deletes too often.
//!
//! A changed fingerprint does not delete the whole folder when it is that
//! of an earlier release ([`stamped`]: the fingerprint of the first of the
//! patches there are now) and the patches since replace nothing but models'
//! files ([`replaced`]): a file the game data did not have is new, and no
//! kept bake read it; a model's file makes that model's bakes stale, whose
//! names hold the model's ([`Models`]). Of the 17 patches of 24 to 56 that
//! changed the fingerprint, 6 only added files, 9 replaced models' files
//! (two of them textures or materials too: those delete the folder).

/// The folders of the game data a bake reads (`resources/` stripped).
pub const FOLDERS: [&[u8]; 5] = [b"textures", b"models", b"material_instances", b"shaders", b"gameplay/items/upgrades"];

/// What one patch gives the fingerprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Patch {
    /// No file a bake reads (or no patch of that number).
    Nothing,
    /// Its files under [`FOLDERS`], hashed.
    Files(u64),
    /// A table this walk does not follow: its bytes, hashed.
    Whole(u64),
    /// Its table could not be read.
    Unreadable,
}

/// 64-bit FNV-1a (its offset basis and prime spelt in bytes: no literal of
/// an address's size in this crate, hooks_026g.py's layer check).
#[derive(Clone, Copy)]
pub struct Fnv(u64);

const FNV_BASIS: u64 = u64::from_be_bytes([0xCB, 0xF2, 0x9C, 0xE4, 0x84, 0x22, 0x23, 0x25]);
const FNV_PRIME: u64 = 1 << 40 | 0x1B3;

impl Fnv {
    pub const fn new() -> Fnv {
        Fnv(FNV_BASIS)
    }

    pub const fn bytes(mut self, bytes: &[u8]) -> Fnv {
        let mut i = 0;
        while i < bytes.len() {
            self.0 = (self.0 ^ bytes[i] as u64).wrapping_mul(FNV_PRIME);
            i += 1;
        }
        self
    }

    pub const fn word(self, value: u32) -> Fnv {
        self.bytes(&value.to_le_bytes())
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// The fingerprint `hash` with the patch `n` after it; none for a patch
/// that gives nothing.
fn fold(hash: Fnv, n: u8, patch: Patch) -> Option<Fnv> {
    let (tag, value) = match patch {
        Patch::Nothing => return None,
        Patch::Files(value) => (1, value),
        Patch::Whole(value) => (2, value),
        Patch::Unreadable => (3, 0),
    };
    Some(hash.word(u32::from(n)).word(tag).word(value as u32).word((value >> 32) as u32))
}

/// The fingerprint of the patches `(n, what it gives)`, in their order:
/// each patch that gives something, its number and that.
pub fn fingerprint(patches: impl Iterator<Item = (u8, Patch)>) -> u64 {
    patches.fold(Fnv::new(), |hash, (n, patch)| fold(hash, n, patch).unwrap_or(hash)).get()
}

/// What a folder's stamp is the fingerprint of, among the patches there
/// are now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stamped {
    /// Of all of them: no patch a bake reads came since.
    Current,
    /// Of the patches up to `last` (0: of none): the ones past it that give
    /// something came since. Patches are released in their numbers' order
    /// and never changed, so a fingerprint of the first of them is that of
    /// an earlier release.
    Behind { last: u8 },
    /// Of other game data.
    Other,
}

/// What `stamp` is the fingerprint of, among the patches `(n, what it
/// gives)` in their order.
pub fn stamped(patches: impl Iterator<Item = (u8, Patch)>, stamp: u64) -> Stamped {
    let mut hash = Fnv::new();
    let mut last = (hash.get() == stamp).then_some(0);
    for (n, patch) in patches {
        if let Some(next) = fold(hash, n, patch) {
            hash = next;
            if hash.get() == stamp {
                last = Some(n);
            }
        }
    }
    match last {
        Some(_) if hash.get() == stamp => Stamped::Current,
        Some(last) => Stamped::Behind { last },
        None => Stamped::Other,
    }
}

// ---- the file table ------------------------------------------------------------

/// The header: "little-endian\0", 2 bytes, the node count, the table's size.
pub const HEADER: usize = 24;
const FOLDER: u16 = 1 << 0;
const ARCHIVE: u16 = 1 << 2;
const HARD_LINK: u16 = 1 << 9;
/// Compressed, inlined, soft link, erased, external table.
const NOT_READ: u16 = (1 << 4) | (1 << 6) | (1 << 8) | (1 << 11) | (1 << 12);
// `base_node`, from where a pointer points: the next sibling, the flags, the name
const NEXT: usize = 24;
const FLAGS: usize = 48;
const NAME: usize = 51;
/// `base_node` in a folder's class (its first child at its start) and a
/// file's (`u32 size, u32, u64 offset, u32 hash`).
const FOLDER_BASE: usize = 16;
const FILE_BASE: usize = 24;
/// The mount root: its folder's pointer; its first child (no pointer).
const ROOT_NODE: usize = 64;
const ROOT_BASE: usize = 936 + FOLDER_BASE;
/// The longest path the game takes (`resources-db`'s MAX_PATH).
const PATH: usize = 249;
/// Folders deeper than this are not followed (the game data's deepest is 9).
const DEPTH: usize = 32;

/// The table's size the header gives; none for another format.
pub fn table_size(header: &[u8; HEADER]) -> Option<u32> {
    (&header[..14] == b"little-endian\0").then(|| u32::from_le_bytes([header[20], header[21], header[22], header[23]]))
}

/// Why the walk stopped: a table it does not follow.
struct Refused;

struct Table<'a>(&'a [u8]);

impl Table<'_> {
    fn u16(&self, at: usize) -> Result<u16, Refused> {
        let b = self.0.get(at..at.checked_add(2).ok_or(Refused)?).ok_or(Refused)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&self, at: usize) -> Result<u32, Refused> {
        let b = self.0.get(at..at.checked_add(4).ok_or(Refused)?).ok_or(Refused)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// A node pointer (a u64 offset; 0 none) inside the table.
    fn pointer(&self, at: usize) -> Result<usize, Refused> {
        let (low, high) = (self.u32(at)?, self.u32(at.checked_add(4).ok_or(Refused)?)?);
        let p = low as usize;
        if high != 0 || p >= self.0.len() {
            return Err(Refused);
        }
        Ok(p)
    }

    fn name(&self, base: usize) -> Result<&[u8], Refused> {
        let rest = self.0.get(base.checked_add(NAME).ok_or(Refused)?..).ok_or(Refused)?;
        let end = rest.iter().position(|&b| b == 0).ok_or(Refused)?;
        Ok(&rest[..end])
    }

    fn first_child(&self, base: usize) -> Result<usize, Refused> {
        self.pointer(base.checked_sub(FOLDER_BASE).ok_or(Refused)?)
    }
}

/// Whether `path` (a folder's or a file's) is under one of [`FOLDERS`].
fn under(path: &[u8]) -> bool {
    FOLDERS.iter().any(|f| path.len() > f.len() && path.starts_with(f) && path[f.len()] == b'/')
}

/// Whether a folder at `path` is one of [`FOLDERS`] or leads to one.
fn leads(path: &[u8]) -> bool {
    under(path) || FOLDERS.iter().any(|f| f.len() >= path.len() && f.starts_with(path)
                                          && (f.len() == path.len() || f[path.len()] == b'/'))
}

/// What a patch's file table (the bytes after its header) gives the
/// fingerprint.
pub fn patch(table: &[u8]) -> Patch {
    match files(&Table(table)) {
        Ok(Some(hash)) => Patch::Files(hash),
        Ok(None) => Patch::Nothing,
        Err(Refused) => Patch::Whole(Fnv::new().bytes(table).get()),
    }
}

/// The files under [`FOLDERS`], hashed in the table's order; none when
/// there is none.
fn files(table: &Table<'_>) -> Result<Option<u64>, Refused> {
    let (mut hash, mut any) = (Fnv::new(), false);
    walk(table, |path, size, crc| {
        hash = hash.bytes(path).bytes(&[0]).word(size).word(crc);
        any = true;
        Ok(true)
    })?;
    Ok(any.then(|| hash.get()))
}

/// The table's root folder.
fn root(table: &Table<'_>) -> Result<usize, Refused> {
    let mut root = table.pointer(ROOT_NODE)?;
    if root == 0 {
        root = ROOT_BASE;
    }
    if table.u16(root + FLAGS)? & FOLDER == 0 {
        return Err(Refused);
    }
    Ok(root)
}

/// Each file under [`FOLDERS`] to `file` (its path, size and CRC) in the
/// table's order, until it says false. Depth first, the path in one
/// buffer: a level's next sibling is noted before its folder is entered.
fn walk(table: &Table<'_>, mut file: impl FnMut(&[u8], u32, u32) -> Result<bool, Refused>) -> Result<(), Refused> {
    // a node takes more than FLAGS bytes of the table
    let mut budget = table.0.len() / FLAGS + 1;
    let (mut path, mut levels) = ([0u8; PATH], [(0usize, 0usize); DEPTH]);
    levels[0] = (table.first_child(root(table)?)?, 0);
    let mut depth = 1;
    while depth > 0 {
        let (node, length) = levels[depth - 1];
        if node == 0 {
            depth -= 1;
            continue;
        }
        budget = budget.checked_sub(1).ok_or(Refused)?;
        levels[depth - 1].0 = table.pointer(node.checked_add(NEXT).ok_or(Refused)?)?;
        let (flags, name) = (table.u16(node + FLAGS)?, table.name(node)?);
        let start = if length == 0 { 0 } else { length + 1 };
        let end = start + name.len();
        if name.is_empty() || end > PATH || flags & NOT_READ != 0 {
            return Err(Refused);
        }
        if length != 0 {
            path[length] = b'/';
        }
        path[start..end].copy_from_slice(name);
        let at = &path[..end];
        if flags & FOLDER != 0 {
            if !leads(at) {
                continue;
            }
            if depth == DEPTH {
                return Err(Refused);
            }
            levels[depth] = (table.first_child(node)?, end);
            depth += 1;
        } else if flags & ARCHIVE != 0 {
            if !under(at) {
                continue;
            }
            if flags & HARD_LINK != 0 {
                return Err(Refused);
            }
            let entry = node.checked_sub(FILE_BASE).ok_or(Refused)?;
            if !file(at, table.u32(entry)?, table.u32(entry + 16)?)? {
                return Ok(());
            }
        } else {
            return Err(Refused);
        }
    }
    Ok(())
}

// ---- what a release replaces ---------------------------------------------------------

/// A model's name at most (the game data's longest is 45).
const MODEL_NAME: usize = 64;
/// The models of one start at most (patch 44 replaced files of 12).
const MODELS: usize = 96;
/// The folders under `models/` that are a model: `<name>.<one of these>`.
const MODEL_KINDS: [&[u8]; 2] = [b"skinned_model", b"model"];

/// The names of the models whose files the patches since a folder's stamp
/// replaced: the bakes of those models are stale, and no other. A baked
/// file's name holds its model's name: `<camouflage><model><type>` for a
/// character's item (`rubber_greenscavenger_sniper_10_legs_s1d`), and
/// `srpw1_<camouflage>_<the model's path>_<surface>...` for a weapon.
pub struct Models {
    names: [[u8; MODEL_NAME]; MODELS],
    lengths: [u8; MODELS],
    count: usize,
}

impl Models {
    pub const fn new() -> Models {
        Models { names: [[0; MODEL_NAME]; MODELS], lengths: [0; MODELS], count: 0 }
    }

    fn names(&self) -> impl Iterator<Item = &[u8]> {
        self.names.iter().zip(&self.lengths).take(self.count).map(|(name, &length)| &name[..usize::from(length)])
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// The model `name` in the list (once); false when it has no room for
    /// it or the name is none a model has.
    fn add(&mut self, name: &[u8]) -> bool {
        if self.names().any(|have| have.eq_ignore_ascii_case(name)) {
            return true;
        }
        if name.is_empty() || name.len() > MODEL_NAME || self.count == MODELS {
            return false;
        }
        self.names[self.count][..name.len()].copy_from_slice(name);
        self.lengths[self.count] = name.len() as u8;
        self.count += 1;
        true
    }

    /// Whether a baked file of this name is one of these models': the name
    /// holds a model's name (the letters' case aside). A short name is in
    /// other models' files too: more is baked again, never less.
    pub fn bake_of(&self, file: &[u8]) -> bool {
        self.names().any(|name| file.windows(name.len()).any(|part| part.eq_ignore_ascii_case(name)))
    }
}

impl Default for Models {
    fn default() -> Models {
        Models::new()
    }
}

/// The model a file under `models/` is of: its folder's path (the first
/// `<name>.skinned_model` or `<name>.model` on the file's way) and its
/// name; none for a file in no model's folder.
fn model(path: &[u8]) -> Option<(&[u8], &[u8])> {
    let mut end = b"models".len();
    path.strip_prefix(b"models/")?;
    loop {
        let start = end + 1;
        // the last part is the file
        end = start + path.get(start..)?.iter().position(|&b| b == b'/')?;
        let part = &path[start..end];
        if let Some(dot) = part.iter().rposition(|&b| b == b'.')
            && MODEL_KINDS.contains(&&part[dot + 1..])
        {
            return Some((&path[..end], &part[..dot]));
        }
    }
}

/// Whether the table has a folder or a file at `path` (the letters' case
/// aside).
fn has(table: &Table<'_>, path: &[u8]) -> Result<bool, Refused> {
    let (mut node, mut budget) = (root(table)?, table.0.len() / FLAGS + 1);
    for name in path.split(|&b| b == b'/') {
        if table.u16(node + FLAGS)? & FOLDER == 0 {
            return Ok(false);
        }
        node = table.first_child(node)?;
        while node != 0 && !table.name(node)?.eq_ignore_ascii_case(name) {
            budget = budget.checked_sub(1).ok_or(Refused)?;
            node = table.pointer(node.checked_add(NEXT).ok_or(Refused)?)?;
        }
        if node == 0 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// What a patch replaces of the game data under it that a bake reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Replaced {
    /// Nothing but files of models: their bakes are stale, no other.
    Models,
    /// A texture, a material, a shader or a camouflage or decal item, which
    /// any bake may have read; a file under `models/` in no model's folder;
    /// more models than the list holds; or a table this walk does not
    /// follow. Every bake may be stale.
    Bakes,
}

/// What the patch `new` replaces of `under` (resources.db's table, or a
/// lower patch's) that a bake reads. A file that `under` does not have is
/// new to it: a bake that read it was made after the patch came. A file of
/// a model `under` has (its folder there) makes that model's bakes stale,
/// its name into `models`; any other file `under` has makes every bake
/// stale. The same patch is asked against each table under it.
///
/// Not seen: a new file an older file names and did not find (a material's
/// texture missing until now). A bake made without it stays; a release
/// that adds such a file bumps the bake format.
pub fn replaced(new: &[u8], under: &[u8], models: &mut Models) -> Replaced {
    let (new, under) = (Table(new), Table(under));
    let mut all = false;
    let walked = walk(&new, |path, _, _| {
        all = match model(path) {
            Some((folder, name)) => has(&under, folder)? && !models.add(name),
            None => has(&under, path)?,
        };
        Ok(!all)
    });
    match walked {
        Ok(()) if !all => Replaced::Models,
        _ => Replaced::Bakes,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    /// A table as `resources-db`'s writer lays it out (folders and files
    /// only): `(path of a folder or file, Some((size, crc)) for a file)`,
    /// parents before children.
    struct Writer {
        bytes: Vec<u8>,
        /// each node made: its path, its base, its last child's base
        nodes: Vec<(Vec<u8>, usize, usize)>,
    }

    impl Writer {
        fn new() -> Writer {
            let mut bytes = std::vec![0u8; ROOT_BASE + 64];
            bytes[ROOT_BASE + FLAGS..ROOT_BASE + FLAGS + 2].copy_from_slice(&FOLDER.to_le_bytes());
            Writer { bytes, nodes: std::vec![(Vec::new(), ROOT_BASE, 0)] }
        }

        fn put(&mut self, at: usize, value: u64) {
            self.bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }

        fn add(&mut self, path: &str, file: Option<(u32, u32)>, extra: u16) {
            let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
            let p = self.nodes.iter().position(|(p, _, _)| p == parent.as_bytes()).expect("the parent first");
            let start = self.bytes.len();
            let base = start + if file.is_some() { FILE_BASE } else { FOLDER_BASE };
            self.bytes.resize(base + NAME + name.len() + 1 + 8, 0);
            if let Some((size, crc)) = file {
                self.bytes[start..start + 4].copy_from_slice(&size.to_le_bytes());
                self.bytes[start + 16..start + 20].copy_from_slice(&crc.to_le_bytes());
            }
            let flags = if file.is_some() { ARCHIVE } else { FOLDER } | extra;
            self.bytes[base + FLAGS..base + FLAGS + 2].copy_from_slice(&flags.to_le_bytes());
            self.bytes[base + NAME..base + NAME + name.len()].copy_from_slice(name.as_bytes());
            let (_, parent_base, last) = self.nodes[p];
            if last == 0 {
                self.put(parent_base - FOLDER_BASE, base as u64);
            } else {
                self.put(last + NEXT, base as u64);
            }
            self.nodes[p].2 = base;
            self.nodes.push((path.as_bytes().to_vec(), base, 0));
        }
    }

    fn table(entries: &[(&str, Option<(u32, u32)>)]) -> Vec<u8> {
        let mut w = Writer::new();
        for &(path, file) in entries {
            w.add(path, file, 0);
        }
        w.bytes
    }

    const STATS: [(&str, Option<(u32, u32)>); 4] = [("gameplay", None), ("gameplay/items", None),
                                                    ("gameplay/items/weapons", None),
                                                    ("gameplay/items/weapons/ak.options", Some((10, 1)))];

    #[test]
    fn only_what_a_bake_reads() {
        assert_eq!(patch(&table(&STATS)), Patch::Nothing);
        let mut camo = STATS.to_vec();
        camo.extend([("gameplay/items/upgrades", None), ("gameplay/items/upgrades/decals", None),
                     ("gameplay/items/upgrades/decals/d1.options", Some((5, 7)))]);
        let Patch::Files(with_camo) = patch(&table(&camo)) else { panic!("a decal item is read") };
        let textures = [("textures", None), ("textures/character", None), ("textures/character/a.dds", Some((9, 3)))];
        let Patch::Files(a) = patch(&table(&textures)) else { panic!("a texture is read") };
        let mut other = textures;
        other[2].1 = Some((9, 4));
        let Patch::Files(b) = patch(&table(&other)) else { panic!("a texture is read") };
        assert_ne!(a, b, "another CRC, another fingerprint");
        assert_ne!(a, with_camo);
        // a texture beside stats: the stats do not count
        let mut both = STATS.to_vec();
        both.extend(textures);
        assert_eq!(patch(&table(&both)), Patch::Files(a));
        // a folder named like one but not it
        assert_eq!(patch(&table(&[("texturesx", None), ("texturesx/a.dds", Some((1, 1)))])), Patch::Nothing);
        assert_eq!(patch(&table(&[("textures.txt", Some((1, 1)))])), Patch::Nothing);
    }

    #[test]
    fn what_it_does_not_follow_is_hashed_whole() {
        let mut w = Writer::new();
        w.add("textures", None, 0);
        w.add("textures/a.dds", Some((1, 2)), HARD_LINK);
        assert!(matches!(patch(&w.bytes), Patch::Whole(_)));
        let mut w = Writer::new();
        w.add("models", None, 0);
        w.add("models/a", Some((1, 2)), 1 << 4);
        assert!(matches!(patch(&w.bytes), Patch::Whole(_)));
        // a link outside the folders is not followed
        let mut w = Writer::new();
        w.add("sounds", None, 0);
        w.add("sounds/a", Some((1, 2)), HARD_LINK);
        assert_eq!(patch(&w.bytes), Patch::Nothing);
        // a pointer out of the table, a loop
        let mut bytes = table(&[("textures", None), ("textures/a.dds", Some((1, 2)))]);
        let len = bytes.len() as u64;
        bytes[ROOT_BASE - FOLDER_BASE..ROOT_BASE - FOLDER_BASE + 8].copy_from_slice(&len.to_le_bytes());
        assert!(matches!(patch(&bytes), Patch::Whole(_)));
        let mut bytes = table(&[("textures", None)]);
        let first = u64::from_le_bytes(bytes[ROOT_BASE - FOLDER_BASE..ROOT_BASE - FOLDER_BASE + 8].try_into().expect("8"));
        let at = first as usize + NEXT;
        bytes[at..at + 8].copy_from_slice(&first.to_le_bytes());
        assert!(matches!(patch(&bytes), Patch::Whole(_)));
        assert!(matches!(patch(&[0u8; 10]), Patch::Whole(_)));
    }

    #[test]
    fn a_patch_without_bake_data_keeps_the_fingerprint() {
        let some = [(12, Patch::Files(5)), (14, Patch::Nothing), (17, Patch::Whole(9))];
        let base = fingerprint(some.into_iter());
        assert_eq!(fingerprint(some.into_iter().chain([(55, Patch::Nothing)])), base);
        assert_ne!(fingerprint(some.into_iter().chain([(55, Patch::Files(1))])), base);
        assert_ne!(fingerprint(some.into_iter().chain([(55, Patch::Unreadable)])), base);
        assert_ne!(fingerprint([(12, Patch::Files(5))].into_iter()), fingerprint([(13, Patch::Files(5))].into_iter()));
        assert_ne!(fingerprint(core::iter::empty()), fingerprint([(1, Patch::Unreadable)].into_iter()));
    }

    #[test]
    fn folders() {
        assert!(leads(b"gameplay") && leads(b"gameplay/items") && leads(b"gameplay/items/upgrades"));
        assert!(leads(b"textures") && leads(b"gameplay/items/upgrades/decals"));
        assert!(!leads(b"gameplay/items/weapons") && !leads(b"gameplay/item") && !leads(b"sounds"));
        assert!(under(b"textures/a") && !under(b"textures") && !under(b"gameplay/items/a.options"));
    }

    #[test]
    fn a_stamp_of_an_earlier_release_is_behind() {
        let some = [(12, Patch::Files(5)), (14, Patch::Nothing), (17, Patch::Whole(9)), (20, Patch::Files(7))];
        let of = |count: usize| fingerprint(some[..count].iter().copied());
        assert_eq!(stamped(some.into_iter(), of(4)), Stamped::Current);
        // patch 14 gives nothing: the fingerprint of the first two is the first's
        assert_eq!(stamped(some.into_iter(), of(1)), Stamped::Behind { last: 12 });
        assert_eq!(stamped(some.into_iter(), of(2)), Stamped::Behind { last: 12 });
        assert_eq!(stamped(some.into_iter(), of(3)), Stamped::Behind { last: 17 });
        assert_eq!(stamped(some.into_iter(), of(0)), Stamped::Behind { last: 0 });
        assert_eq!(stamped(some.into_iter(), of(4) ^ 1), Stamped::Other);
        // a later release's stamp, of a patch this game data lacks
        assert_eq!(stamped(some[..3].iter().copied(), of(4)), Stamped::Other);
        // a patch that gives nothing since: still this game's
        assert_eq!(stamped(some.into_iter().chain([(55, Patch::Nothing)]), of(4)), Stamped::Current);
        assert_eq!(stamped(core::iter::empty(), of(0)), Stamped::Current);
    }

    #[test]
    fn a_file_s_model() {
        assert_eq!(model(b"models/weapons/dmr/g3a3/g3a3.skinned_model/render/export_properties"),
                   Some((&b"models/weapons/dmr/g3a3/g3a3.skinned_model"[..], &b"g3a3"[..])));
        assert_eq!(model(b"models/character/human/edge/edge_sniper_06_legs_s1.skinned_model/settings").map(|m| m.1),
                   Some(&b"edge_sniper_06_legs_s1"[..]));
        assert_eq!(model(b"models/item/a.model/render/b.model/x"), Some((&b"models/item/a.model"[..], &b"a"[..])));
        // a file named as a model's folder is, a folder of another kind, no folder, another root
        for none in [&b"models/a.skinned_model"[..], b"models/a.b/c", b"models/readme.txt", b"models/a/b/c",
                     b"textures/a.model/b", b"models"] {
            assert_eq!(model(none), None, "{:?}", core::str::from_utf8(none));
        }
    }

    const BASE: [(&str, Option<(u32, u32)>); 12] =
        [("models", None), ("models/weapons", None), ("models/weapons/ak.skinned_model", None),
         ("models/weapons/ak.skinned_model/settings", Some((4, 1))), ("models/weapons/loose", Some((1, 1))),
         ("textures", None), ("textures/ak.dds", Some((9, 3))), ("gameplay", None), ("gameplay/items", None),
         ("gameplay/items/upgrades", None), ("gameplay/items/upgrades/city.options", Some((5, 7))),
         ("sounds", None)];

    #[test]
    fn what_the_game_data_has() {
        let base = Table(&table(&BASE));
        for there in [&b"models"[..], b"models/weapons/ak.skinned_model", b"models/weapons/AK.skinned_model/Settings",
                      b"textures/ak.dds", b"sounds"] {
            assert!(matches!(has(&base, there), Ok(true)), "{:?}", core::str::from_utf8(there));
        }
        for missing in [&b"models/weapons/vss.skinned_model"[..], b"textures/ak.dds/x", b"textures/ak", b"texture",
                        b"sounds/a", b""] {
            assert!(matches!(has(&base, missing), Ok(false)), "{:?}", core::str::from_utf8(missing));
        }
        assert!(has(&Table(&[0u8; 10]), b"models").is_err());
    }

    #[test]
    fn a_release_replaces_models_files_or_more() {
        let base = table(&BASE);
        let against = |entries: &[(&str, Option<(u32, u32)>)]| {
            let mut models = Models::new();
            (replaced(&table(entries), &base, &mut models), models)
        };
        // a new gun: its model, its texture, its camouflage; nothing a kept bake read
        let (what, models) = against(&[("models", None), ("models/weapons", None), ("models/weapons/vss.skinned_model", None),
                                       ("models/weapons/vss.skinned_model/settings", Some((4, 2))), ("textures", None),
                                       ("textures/vss.dds", Some((9, 4))), ("gameplay", None), ("gameplay/items", None),
                                       ("gameplay/items/upgrades", None),
                                       ("gameplay/items/upgrades/forest.options", Some((5, 8))),
                                       ("models/weapons/new_loose", Some((1, 1)))]);
        assert_eq!((what, models.count()), (Replaced::Models, 0));
        assert!(!models.bake_of(b"srpw1_city_weapons_ak_0d.dds"));
        // a model's file replaced and a file new to its folder: that model's bakes, once
        let (what, models) = against(&[("models", None), ("models/weapons", None), ("models/weapons/ak.skinned_model", None),
                                       ("models/weapons/ak.skinned_model/settings", Some((4, 9))),
                                       ("models/weapons/ak.skinned_model/render", None),
                                       ("models/weapons/ak.skinned_model/render/export_properties", Some((7, 7))),
                                       ("textures", None), ("textures/vss.dds", Some((9, 4)))]);
        assert_eq!((what, models.count()), (Replaced::Models, 1));
        assert!(models.bake_of(b"srpw1_city_weapons_ak_0d.dds") && models.bake_of(b"cityAK_s1d.dds"));
        assert!(!models.bake_of(b"srpw1_city_weapons_vss_0d.dds") && !models.bake_of(b"a"));
        // a texture, a camouflage item, a file under models/ in no model's folder: any bake
        for entries in [&[("textures", None), ("textures/ak.dds", Some((9, 4)))][..],
                        &[("gameplay", None), ("gameplay/items", None), ("gameplay/items/upgrades", None),
                          ("gameplay/items/upgrades/city.options", Some((5, 7)))],
                        &[("models", None), ("models/weapons", None), ("models/weapons/loose", Some((1, 2)))]] {
            assert_eq!(against(entries).0, Replaced::Bakes, "{entries:?}");
        }
        // what a bake does not read is not asked
        assert_eq!(against(&[("sounds", None), ("sounds/a", Some((1, 1)))]).0, Replaced::Models);
        // a table not followed, either one
        let mut linked = Writer::new();
        linked.add("textures", None, 0);
        linked.add("textures/new.dds", Some((1, 2)), HARD_LINK);
        assert_eq!(replaced(&linked.bytes, &base, &mut Models::new()), Replaced::Bakes);
        assert_eq!(replaced(&table(&[("textures", None), ("textures/ak.dds", Some((1, 1)))]), &[0u8; 10], &mut Models::new()),
                   Replaced::Bakes);
    }

    #[test]
    fn more_models_than_the_list_holds_is_every_bake() {
        let folders: Vec<std::string::String> = (0..=MODELS).map(|i| std::format!("models/m{i}.model")).collect();
        let files: Vec<std::string::String> = folders.iter().map(|folder| std::format!("{folder}/settings")).collect();
        let entries = |count: usize| {
            let mut entries = std::vec![("models", None)];
            for (folder, file) in folders.iter().zip(&files).take(count) {
                entries.extend([(folder.as_str(), None), (file.as_str(), Some((1, 1)))]);
            }
            table(&entries)
        };
        let mut models = Models::new();
        assert_eq!(replaced(&entries(MODELS), &entries(MODELS + 1), &mut models), Replaced::Models);
        assert_eq!(models.count(), MODELS);
        // the same ones again are in the list; one more is not
        assert_eq!(replaced(&entries(MODELS), &entries(MODELS + 1), &mut models), Replaced::Models);
        assert_eq!(replaced(&entries(MODELS + 1), &entries(MODELS + 1), &mut models), Replaced::Bakes);
        // a name longer than a model's
        let long = std::format!("models/{}.model", "x".repeat(MODEL_NAME + 1));
        let file = std::format!("{long}/settings");
        let table = table(&[("models", None), (long.as_str(), None), (file.as_str(), Some((1, 1)))]);
        assert_eq!(replaced(&table, &table, &mut Models::new()), Replaced::Bakes);
    }
}

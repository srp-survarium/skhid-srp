//! A revision's `.torrent` (`srp-026g/docs/notes/torrent-downloads.md`): a
//! hybrid v1 + v2 torrent (BEP 52) whose every file starts at a piece
//! (BEP 47 pad files), read once into [`Torrent`] and refused whole when it
//! is anything else.
//!
//! What is trusted, and how: the torrent is trusted as a whole by where it
//! comes from (the program decides that: `exe/src/torrent.rs`, `trusted`).
//! Its `info` holds each file's v2 "pieces root", a SHA-256 merkle root of
//! its 16 KiB blocks; the torrent's "piece layers" are checked against those
//! roots here, so every piece is then checked by SHA-256
//! ([`Torrent::piece_ok`]). The v1 SHA-1 hashes are not used: the v1 info
//! hash only names the swarm (the trackers', the peers' handshake), and the
//! v1 piece space (files and pads in the v1 list's order) is how pieces are
//! numbered on the wire, as every v1 or hybrid client numbers them.

use alloc::string::String;
use alloc::vec::Vec;

use crate::bencode::{self, Value};
use crate::sha1;
use crate::sha256::{Sha256, digest};
use crate::zip::safe_name;

/// The largest `.torrent` taken.
pub const MAX_TORRENT: usize = 8 << 20;
/// A v2 leaf: 16 KiB of a file; also the block a peer is asked for.
pub const BLOCK: u32 = 16 * 1024;
const MAX_PIECE: u32 = 16 << 20;
const MAX_FILES: usize = 10_000;
const MAX_TRACKERS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// Its path in the torrent, `/` between the parts (a `zip::safe_name`).
    pub path: String,
    pub size: u64,
    /// Where it starts in the v1 piece space: a piece's start.
    pub offset: u64,
}

impl File {
    pub fn first_piece(&self, piece_length: u32) -> u32 {
        (self.offset / u64::from(piece_length)) as u32
    }

    pub fn pieces(&self, piece_length: u32) -> u32 {
        self.size.div_ceil(u64::from(piece_length)) as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Torrent {
    pub v1: [u8; 20],
    pub v2: [u8; 32],
    pub piece_length: u32,
    /// The v1 piece space's length, the pads in it.
    pub length: u64,
    /// The files, in the v1 list's order (no pads).
    pub files: Vec<File>,
    /// Its trackers' addresses (`http://`, `https://`, `udp://`).
    pub trackers: Vec<String>,
    /// For each piece: the file it is of, and the SHA-256 merkle root its
    /// blocks must make.
    pieces: Vec<(u32, [u8; 32])>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorrentError {
    /// Not bencoding, or not a torrent's.
    Format,
    /// Not hybrid, or a file that does not start at a piece.
    Layout,
    /// A piece layer that does not make its file's root.
    PieceLayer,
}

fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Sha256::default();
    hasher.update(left);
    hasher.update(right);
    hasher.finish()
}

/// The root over `leaves`, padded with `pad` to `width` (a power of two).
fn merkle_root(mut layer: Vec<[u8; 32]>, width: usize, pad: [u8; 32]) -> [u8; 32] {
    layer.resize(width.max(1), pad);
    while layer.len() > 1 {
        layer = layer
            .chunks_exact(2)
            .map(|pair| hash_pair(&pair[0], &pair[1]))
            .collect();
    }
    layer[0]
}

/// The root of `leaves` zero leaves (a power of two): what pads a piece
/// layer.
fn zero_root(leaves: u32) -> [u8; 32] {
    let mut hash = [0_u8; 32];
    let mut width = 1;
    while width < leaves {
        hash = hash_pair(&hash, &hash);
        width *= 2;
    }
    hash
}

fn text(value: Option<&Value>) -> Option<String> {
    String::from_utf8(value?.bytes()?.into()).ok()
}

fn tracker(address: &str) -> bool {
    address.len() <= 1024
        && ["http://", "https://", "udp://"]
            .iter()
            .any(|scheme| address.starts_with(scheme))
        && address.bytes().all(|c| c.is_ascii_graphic())
}

fn trackers(document: &Value) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    let mut add = |value: &Value| {
        if let Some(address) = text(Some(value))
            && tracker(&address)
            && !all.contains(&address)
            && all.len() < MAX_TRACKERS
        {
            all.push(address);
        }
    };
    if let Some(announce) = document.get(b"announce") {
        add(announce);
    }
    for tier in document
        .get(b"announce-list")
        .and_then(Value::list)
        .unwrap_or(&[])
    {
        for address in tier.list().unwrap_or(&[]) {
            add(address);
        }
    }
    all
}

/// The v2 file tree's files: path, size, pieces root (none for an empty
/// file).
fn file_tree(
    node: &Value,
    path: &mut Vec<String>,
    out: &mut Vec<(String, u64, Option<[u8; 32]>)>,
) -> Option<()> {
    for (name, child, _) in node.dict()? {
        if name.is_empty() {
            // A file: `"": {length, pieces root}`, and nothing beside it.
            if node.dict()?.len() != 1 || path.is_empty() {
                return None;
            }
            let size = u64::try_from(child.get(b"length")?.int()?).ok()?;
            let root = match child.get(b"pieces root") {
                Some(root) => Some(<[u8; 32]>::try_from(root.bytes()?).ok()?),
                None if size == 0 => None,
                None => return None,
            };
            out.push((path.join("/"), size, root));
            if out.len() > MAX_FILES {
                return None;
            }
            continue;
        }
        path.push(String::from_utf8(name.to_vec()).ok()?);
        file_tree(child, path, out)?;
        path.pop();
    }
    Some(())
}

/// Reads a `.torrent`.
pub fn parse(data: &[u8]) -> Result<Torrent, TorrentError> {
    use TorrentError::{Format, Layout};
    if data.len() > MAX_TORRENT {
        return Err(Format);
    }
    let document = bencode::parse(data).ok_or(Format)?;
    let (info, raw) = document.entry(b"info").ok_or(Format)?;
    if info.get(b"meta version").and_then(Value::int) != Some(2) {
        return Err(Layout);
    }
    let piece_length = info
        .get(b"piece length")
        .and_then(Value::int)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| n.is_power_of_two() && (BLOCK..=MAX_PIECE).contains(n))
        .ok_or(Format)?;
    let mut tree = Vec::new();
    file_tree(
        info.get(b"file tree").ok_or(Layout)?,
        &mut Vec::new(),
        &mut tree,
    )
    .ok_or(Format)?;
    // The v1 list: the piece space. Each file of it is one of the tree's,
    // of the same size, starting at a piece; pads (`attr` "p") fill to it.
    let list = info.get(b"files").and_then(Value::list).ok_or(Layout)?;
    let mut files: Vec<File> = Vec::new();
    let mut roots: Vec<Option<[u8; 32]>> = Vec::new();
    let mut offset = 0_u64;
    for entry in list {
        let size = entry
            .get(b"length")
            .and_then(Value::int)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or(Format)?;
        let parts = entry.get(b"path").and_then(Value::list).ok_or(Format)?;
        let mut path = Vec::new();
        for part in parts {
            path.push(text(Some(part)).ok_or(Format)?);
        }
        let path = path.join("/");
        let pad = entry
            .get(b"attr")
            .and_then(Value::bytes)
            .is_some_and(|attr| attr.contains(&b'p'));
        if pad {
            if size == 0 || size >= u64::from(piece_length) {
                return Err(Layout);
            }
        } else {
            if !safe_name(&path) || files.iter().any(|file| file.path == path) {
                return Err(Format);
            }
            let at = tree
                .iter()
                .position(|(name, _, _)| *name == path)
                .ok_or(Layout)?;
            let (_, tree_size, root) = tree.swap_remove(at);
            if tree_size != size || (size > 0 && offset % u64::from(piece_length) != 0) {
                return Err(Layout);
            }
            files.push(File { path, size, offset });
            roots.push(root);
        }
        offset = offset.checked_add(size).ok_or(Format)?;
    }
    if !tree.is_empty() || files.is_empty() {
        return Err(Layout);
    }
    let count = offset.div_ceil(u64::from(piece_length));
    let v1_pieces = info.get(b"pieces").and_then(Value::bytes).ok_or(Layout)?;
    if v1_pieces.len() as u64 != count * 20 || count > u32::MAX as u64 {
        return Err(Layout);
    }
    // Every piece of each file, by its root and the file's piece layer.
    let layers = document.get(b"piece layers");
    let blocks_per_piece = piece_length / BLOCK;
    let mut pieces: Vec<(u32, [u8; 32])> = Vec::with_capacity(count as usize);
    for (index, (file, root)) in files.iter().zip(&roots).enumerate() {
        let Some(root) = root else { continue };
        if pieces.len() as u32 != file.first_piece(piece_length) {
            return Err(Layout);
        }
        let n = file.pieces(piece_length);
        if n == 1 {
            pieces.push((index as u32, *root));
            continue;
        }
        let layer = layers
            .and_then(|layers| layers.get(root))
            .and_then(Value::bytes)
            .filter(|layer| layer.len() as u64 == u64::from(n) * 32)
            .ok_or(TorrentError::PieceLayer)?;
        let hashes: Vec<[u8; 32]> = layer
            .chunks_exact(32)
            .map(|hash| hash.try_into().expect("32 bytes"))
            .collect();
        let width = (n as usize).next_power_of_two();
        if merkle_root(hashes.clone(), width, zero_root(blocks_per_piece)) != *root {
            return Err(TorrentError::PieceLayer);
        }
        pieces.extend(hashes.into_iter().map(|hash| (index as u32, hash)));
    }
    // A piece of pads alone cannot be: each piece is of one file.
    if pieces.len() as u64 != count {
        return Err(Layout);
    }
    Ok(Torrent {
        v1: sha1::digest(raw),
        v2: digest(raw),
        piece_length,
        length: offset,
        files,
        trackers: trackers(&document),
        pieces,
    })
}

impl Torrent {
    pub fn piece_count(&self) -> u32 {
        self.pieces.len() as u32
    }

    /// The piece's bytes on the wire (its file's, then pad zeros; the last
    /// piece of the space may be shorter).
    pub fn piece_size(&self, piece: u32) -> u32 {
        let start = u64::from(piece) * u64::from(self.piece_length);
        (self.length - start).min(u64::from(self.piece_length)) as u32
    }

    /// The file a piece is of, and how many of its bytes are the file's.
    pub fn piece_file(&self, piece: u32) -> (usize, u32) {
        let file = self.pieces[piece as usize].0 as usize;
        let start = u64::from(piece) * u64::from(self.piece_length);
        let of_file = (self.files[file].offset + self.files[file].size - start)
            .min(u64::from(self.piece_length)) as u32;
        (file, of_file)
    }

    /// The pieces of file `file`.
    pub fn file_pieces(&self, file: usize) -> core::ops::Range<u32> {
        let file = &self.files[file];
        if file.size == 0 {
            return 0..0;
        }
        let first = file.first_piece(self.piece_length);
        first..first + file.pieces(self.piece_length)
    }

    /// Whether `data` is piece `piece`: its size, its file's bytes making
    /// the piece's SHA-256 merkle root, its pad bytes zero.
    pub fn piece_ok(&self, piece: u32, data: &[u8]) -> bool {
        let Some(&(file, expected)) = self.pieces.get(piece as usize) else {
            return false;
        };
        if data.len() != self.piece_size(piece) as usize {
            return false;
        }
        let (_, of_file) = self.piece_file(piece);
        let (bytes, pad) = data.split_at(of_file as usize);
        if pad.iter().any(|&byte| byte != 0) {
            return false;
        }
        let leaves: Vec<[u8; 32]> = bytes.chunks(BLOCK as usize).map(digest).collect();
        let size = self.files[file as usize].size;
        let width = if size > u64::from(self.piece_length) {
            (self.piece_length / BLOCK) as usize
        } else {
            leaves.len().next_power_of_two()
        };
        merkle_root(leaves, width, [0; 32]) == expected
    }
}

/// What tests build torrents with: a hybrid torrent of `files` (path,
/// bytes) as libtorrent makes one (a pad after each file but the last).
#[cfg(test)]
pub(crate) fn make(files: &[(&str, &[u8])], piece_length: u32, trackers: &[&str]) -> Vec<u8> {
    use alloc::format;
    let bstr = |out: &mut Vec<u8>, bytes: &[u8]| {
        out.extend_from_slice(format!("{}:", bytes.len()).as_bytes());
        out.extend_from_slice(bytes);
    };
    let roots_and_layers: Vec<([u8; 32], Vec<[u8; 32]>)> = files
        .iter()
        .map(|(_, data)| {
            let leaves: Vec<[u8; 32]> = data.chunks(BLOCK as usize).map(digest).collect();
            let per = (piece_length / BLOCK) as usize;
            let layer: Vec<[u8; 32]> = leaves
                .chunks(per)
                .map(|chunk| merkle_root(chunk.to_vec(), per, [0; 32]))
                .collect();
            let root = if data.len() as u64 <= u64::from(piece_length) {
                merkle_root(leaves.clone(), leaves.len().next_power_of_two(), [0; 32])
            } else {
                merkle_root(
                    layer.clone(),
                    layer.len().next_power_of_two(),
                    zero_root(per as u32),
                )
            };
            (root, layer)
        })
        .collect();
    // The file tree: these tests' paths have one level of folders at most,
    // sorted as given.
    let mut info = Vec::new();
    let mut list = b"l".to_vec();
    let mut v1 = Vec::new();
    for (index, (path, data)) in files.iter().enumerate() {
        list.extend_from_slice(b"d6:lengthi");
        list.extend_from_slice(format!("{}", data.len()).as_bytes());
        list.extend_from_slice(b"e4:pathl");
        for part in path.split('/') {
            bstr(&mut list, part.as_bytes());
        }
        list.extend_from_slice(b"ee");
        v1.extend_from_slice(data);
        let rem = data.len() as u64 % u64::from(piece_length);
        if index + 1 < files.len() && rem != 0 {
            let pad = u64::from(piece_length) - rem;
            list.extend_from_slice(format!("d4:attr1:p6:lengthi{pad}e4:pathl4:.pad").as_bytes());
            bstr(&mut list, format!("{pad}").as_bytes());
            list.extend_from_slice(b"ee");
            v1.resize(v1.len() + pad as usize, 0);
        }
    }
    list.push(b'e');
    info.extend_from_slice(b"d9:file treed");
    let mut sorted: Vec<usize> = (0..files.len()).collect();
    sorted.sort_by_key(|&i| files[i].0);
    let mut open: Option<&str> = None;
    for &i in &sorted {
        let (dir, name) = match files[i].0.split_once('/') {
            Some((dir, name)) => (Some(dir), name),
            None => (None, files[i].0),
        };
        if open != dir {
            if open.is_some() {
                info.push(b'e');
            }
            if let Some(dir) = dir {
                bstr(&mut info, dir.as_bytes());
                info.push(b'd');
            }
            open = dir;
        }
        bstr(&mut info, name.as_bytes());
        info.extend_from_slice(b"d0:d6:lengthi");
        info.extend_from_slice(format!("{}", files[i].1.len()).as_bytes());
        info.push(b'e');
        if !files[i].1.is_empty() {
            info.extend_from_slice(b"11:pieces root");
            bstr(&mut info, &roots_and_layers[i].0);
        }
        info.extend_from_slice(b"ee");
    }
    if open.is_some() {
        info.push(b'e');
    }
    info.extend_from_slice(b"e5:files");
    info.extend_from_slice(&list);
    info.extend_from_slice(b"12:meta versioni2e4:name4:Test12:piece lengthi");
    info.extend_from_slice(format!("{piece_length}").as_bytes());
    info.extend_from_slice(b"e6:pieces");
    let v1_hashes: Vec<u8> = v1
        .chunks(piece_length as usize)
        .flat_map(sha1::digest)
        .collect();
    bstr(&mut info, &v1_hashes);
    info.push(b'e');
    let mut out = Vec::new();
    out.push(b'd');
    if let Some(first) = trackers.first() {
        out.extend_from_slice(b"8:announce");
        bstr(&mut out, first.as_bytes());
    }
    out.extend_from_slice(b"4:info");
    out.extend_from_slice(&info);
    out.extend_from_slice(b"12:piece layersd");
    let mut layered: Vec<&([u8; 32], Vec<[u8; 32]>)> = roots_and_layers
        .iter()
        .zip(files)
        .filter(|(_, (_, data))| data.len() as u64 > u64::from(piece_length))
        .map(|(layer, _)| layer)
        .collect();
    layered.sort_by_key(|(root, _)| *root);
    for (root, layer) in layered {
        bstr(&mut out, root);
        bstr(&mut out, &layer.concat());
    }
    out.extend_from_slice(b"ee");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn sample() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let big: Vec<u8> = (0..100_000_u32).map(|i| (i * 7 + i / 300) as u8).collect();
        let small = b"hello".to_vec();
        let torrent = make(
            &[("game/resources.db", &big), ("game/a.txt", &small)],
            32 * 1024,
            &["udp://127.0.0.1:1/announce", "ftp://no"],
        );
        (torrent, big, small)
    }

    #[test]
    fn a_hybrid_torrent_and_its_pieces() {
        let (data, big, small) = sample();
        let torrent = parse(&data).unwrap();
        assert_eq!(torrent.trackers, ["udp://127.0.0.1:1/announce"]);
        assert_eq!(torrent.files.len(), 2);
        assert_eq!(torrent.files[1].offset, 4 * 32 * 1024);
        assert_eq!(torrent.piece_count(), 5);
        assert_eq!(torrent.file_pieces(0), 0..4);
        assert_eq!(torrent.piece_size(4), 5);
        for piece in 0..4 {
            let start = piece as usize * 32 * 1024;
            let mut bytes = big[start..(start + 32 * 1024).min(big.len())].to_vec();
            bytes.resize(32 * 1024, 0);
            assert!(torrent.piece_ok(piece, &bytes), "{piece}");
            bytes[100] ^= 1;
            assert!(!torrent.piece_ok(piece, &bytes));
        }
        // The last piece of the big file: its pad must be zeros.
        let mut last = big[3 * 32 * 1024..].to_vec();
        last.resize(32 * 1024, 0);
        *last.last_mut().unwrap() = 1;
        assert!(!torrent.piece_ok(3, &last));
        assert!(torrent.piece_ok(4, &small));
        assert!(!torrent.piece_ok(4, b"hellO"));
        assert!(!torrent.piece_ok(4, b"hello!"));
        assert!(!torrent.piece_ok(5, b"hello"));
        assert_eq!(torrent.v1, sha1::digest(bencode::parse(&data).unwrap().entry(b"info").unwrap().1));
    }

    #[test]
    fn a_bad_layer_or_a_torrent_cut_short_is_refused() {
        let (data, _, _) = sample();
        // A layer's hash changed (outside info: the hash still matches).
        let at = data.len() - 40;
        let mut bad = data.clone();
        bad[at] ^= 1;
        assert_eq!(parse(&bad), Err(TorrentError::PieceLayer));
        assert_eq!(parse(b"de"), Err(TorrentError::Format));
        assert_eq!(parse(&data[..data.len() - 1]), Err(TorrentError::Format));
    }

    #[test]
    fn a_torrent_without_pads_is_refused() {
        let big = vec![1_u8; 40_000];
        let mut data = make(&[("a", &big), ("b", b"x")], 32 * 1024, &[]);
        // Drop the pad: `b` no longer starts at a piece.
        let find = |from: usize, what: &[u8]| from + data[from..].windows(what.len()).position(|w| w == what).unwrap();
        let start = find(0, b"d4:attr1:p");
        let end = find(start, b"ee") + 2;
        data.drain(start..end);
        assert!(matches!(
            parse(&data),
            Err(TorrentError::Layout | TorrentError::Format)
        ));
    }
}

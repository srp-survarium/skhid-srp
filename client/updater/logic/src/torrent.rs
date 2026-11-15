//! A revision's `.torrent` (`srp-026g/docs/notes/torrent-downloads.md`): a
//! hybrid v1 + v2 torrent (BEP 52) whose every file starts at a piece
//! (BEP 47 pad files), read once into [`Torrent`] and refused whole when it
//! is anything else.
//!
//! What is trusted, and how: the torrent is trusted as a whole by the
//! release's signature (`crate::signed`; the program decides:
//! `exe/src/torrent.rs`, `trusted`), which covers its `info`. The pieces
//! are libtorrent's to check (`skhid_torrent.dll`), against the hashes of
//! that `info`; here is what the updater needs to know of it: its files,
//! their sizes, its v2 info hash.

use alloc::string::String;
use alloc::vec::Vec;

use crate::bencode::{self, Value};
use crate::sha256::digest;
use crate::zip::safe_name;

/// The largest `.torrent` taken.
pub const MAX_TORRENT: usize = 8 << 20;
const MIN_PIECE: u32 = 16 * 1024;
const MAX_PIECE: u32 = 16 << 20;
const MAX_FILES: usize = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// Its path in the torrent, `/` between the parts (a `zip::safe_name`).
    pub path: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Torrent {
    /// The SHA-256 of its `info`: its v2 info hash.
    pub v2: [u8; 32],
    pub piece_length: u32,
    pub pieces: u32,
    /// The files, in the v1 list's order (no pads). (No trackers: the
    /// torrent's are outside its signature; the updater has its own.)
    pub files: Vec<File>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorrentError {
    /// Not bencoding, or not a torrent's.
    Format,
    /// Not hybrid, or a file that does not start at a piece.
    Layout,
}

fn text(value: Option<&Value>) -> Option<String> {
    String::from_utf8(value?.bytes()?.into()).ok()
}

/// The v2 file tree's files: path, size.
fn file_tree(node: &Value, path: &mut Vec<String>, out: &mut Vec<(String, u64)>) -> Option<()> {
    for (name, child, _) in node.dict()? {
        if name.is_empty() {
            // A file: `"": {length, pieces root}`, and nothing beside it.
            if node.dict()?.len() != 1 || path.is_empty() {
                return None;
            }
            let size = u64::try_from(child.get(b"length")?.int()?).ok()?;
            if size > 0 && child.get(b"pieces root")?.bytes()?.len() != 32 {
                return None;
            }
            out.push((path.join("/"), size));
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

/// Reads a `.torrent`, or only its `info` (as the swarm gives it: BEP 9).
pub fn parse(data: &[u8]) -> Result<Torrent, TorrentError> {
    let document = bencode::parse(data).ok_or(TorrentError::Format)?;
    let (_, raw) = document.entry(b"info").ok_or(TorrentError::Format)?;
    parse_info(raw)
}

/// Reads a torrent's bencoded `info`.
pub fn parse_info(raw: &[u8]) -> Result<Torrent, TorrentError> {
    use TorrentError::{Format, Layout};
    if raw.len() > MAX_TORRENT {
        return Err(Format);
    }
    let info = bencode::parse(raw).ok_or(Format)?;
    if info.get(b"meta version").and_then(Value::int) != Some(2) {
        return Err(Layout);
    }
    let piece_length = info
        .get(b"piece length")
        .and_then(Value::int)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| n.is_power_of_two() && (MIN_PIECE..=MAX_PIECE).contains(n))
        .ok_or(Format)?;
    let mut tree = Vec::new();
    file_tree(info.get(b"file tree").ok_or(Layout)?, &mut Vec::new(), &mut tree).ok_or(Format)?;
    // The v1 list: each file of it is one of the tree's, of the same size,
    // starting at a piece; pads (`attr` "p") fill to it.
    let list = info.get(b"files").and_then(Value::list).ok_or(Layout)?;
    let mut files: Vec<File> = Vec::new();
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
            let at = tree.iter().position(|(name, _)| *name == path).ok_or(Layout)?;
            let (_, tree_size) = tree.swap_remove(at);
            if tree_size != size || (size > 0 && !offset.is_multiple_of(u64::from(piece_length))) {
                return Err(Layout);
            }
            files.push(File { path, size });
        }
        offset = offset.checked_add(size).ok_or(Format)?;
    }
    if !tree.is_empty() || files.is_empty() {
        return Err(Layout);
    }
    let count = offset.div_ceil(u64::from(piece_length));
    let v1_pieces = info.get(b"pieces").and_then(Value::bytes).ok_or(Layout)?;
    if v1_pieces.len() as u64 != count * 20 || count > u64::from(u32::MAX) {
        return Err(Layout);
    }
    Ok(Torrent {
        v2: digest(raw),
        piece_length,
        pieces: count as u32,
        files,
    })
}

/// The torrent as the updater keeps and passes it on: its `info`, `piece
/// layers` and `skhid` (the signature) as they are, nothing else. The rest
/// (trackers, a comment...) is outside the signature: whoever handed the
/// torrent over could have put anything there. (Its piece layers are
/// libtorrent's to check against `info`.)
pub fn canonical(data: &[u8]) -> Option<Vec<u8>> {
    let document = bencode::parse(data)?;
    let mut out = alloc::vec![b'd'];
    // In the order of their keys' bytes, as bencoding has them.
    for key in [&b"info"[..], b"piece layers", b"skhid"] {
        if let Some((_, raw)) = document.entry(key) {
            out.extend_from_slice(alloc::format!("{}:", key.len()).as_bytes());
            out.extend_from_slice(key);
            out.extend_from_slice(raw);
        }
    }
    out.push(b'e');
    Some(out)
}

/// What tests build torrents with: a hybrid torrent of `files` (path,
/// bytes) laid out as libtorrent lays one (a pad after each file but the
/// last); its hashes are not the files' (nothing here checks them).
#[cfg(test)]
pub(crate) fn make(files: &[(&str, &[u8])], piece_length: u32, trackers: &[&str]) -> Vec<u8> {
    use alloc::format;
    let bstr = |out: &mut Vec<u8>, bytes: &[u8]| {
        out.extend_from_slice(format!("{}:", bytes.len()).as_bytes());
        out.extend_from_slice(bytes);
    };
    let mut info = Vec::new();
    let mut list = b"l".to_vec();
    let mut length = 0_u64;
    for (index, (path, data)) in files.iter().enumerate() {
        list.extend_from_slice(b"d6:lengthi");
        list.extend_from_slice(format!("{}", data.len()).as_bytes());
        list.extend_from_slice(b"e4:pathl");
        for part in path.split('/') {
            bstr(&mut list, part.as_bytes());
        }
        list.extend_from_slice(b"ee");
        length += data.len() as u64;
        let rem = data.len() as u64 % u64::from(piece_length);
        if index + 1 < files.len() && rem != 0 {
            let pad = u64::from(piece_length) - rem;
            list.extend_from_slice(format!("d4:attr1:p6:lengthi{pad}e4:pathl4:.pad").as_bytes());
            bstr(&mut list, format!("{pad}").as_bytes());
            list.extend_from_slice(b"ee");
            length += pad;
        }
    }
    list.push(b'e');
    info.extend_from_slice(b"d9:file treed");
    // The file tree: these tests' paths have one level of folders at most,
    // sorted as given.
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
            bstr(&mut info, &digest(files[i].1));
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
    let count = length.div_ceil(u64::from(piece_length)) as usize;
    bstr(&mut info, &alloc::vec![0_u8; count * 20]);
    info.push(b'e');
    let mut out = Vec::new();
    out.push(b'd');
    if let Some(first) = trackers.first() {
        out.extend_from_slice(b"8:announce");
        bstr(&mut out, first.as_bytes());
    }
    out.extend_from_slice(b"4:info");
    out.extend_from_slice(&info);
    out.push(b'e');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn a_hybrid_torrent_its_files_and_its_info_hash() {
        let big: Vec<u8> = (0..100_000_u32).map(|i| (i * 7 + i / 300) as u8).collect();
        let data = make(
            &[("game/resources.db", &big), ("game/a.txt", b"hello")],
            32 * 1024,
            &["udp://127.0.0.1:1/announce", "ftp://no"],
        );
        let torrent = parse(&data).unwrap();
        // Kept and passed on without what is outside the signature.
        let kept = canonical(&data).unwrap();
        assert!(!kept.windows(8).any(|w| w == b"announce"));
        assert_eq!(parse(&kept).unwrap(), torrent);
        let paths: Vec<(&str, u64)> = torrent.files.iter().map(|f| (f.path.as_str(), f.size)).collect();
        assert_eq!(paths, [("game/resources.db", 100_000), ("game/a.txt", 5)]);
        assert_eq!(torrent.pieces, 5);
        let info = bencode::parse(&data).unwrap().entry(b"info").unwrap().1;
        assert_eq!(torrent.v2, digest(info));
        // The info alone, as the swarm gives it, is the same torrent.
        assert_eq!(parse_info(info).unwrap(), torrent);
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

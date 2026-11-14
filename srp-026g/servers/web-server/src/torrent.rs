//! The torrent of the whole game folder of the revision `/download` belongs
//! to (`docs/notes/torrent-downloads.md`): the ZIP's [`ENTRY`], which the
//! release puts in it (`release/build-torrent`), signed by the release's
//! key. The updater takes it from the ZIP it installed (whose SHA-256
//! `/api/client-files` names) and checks that signature; this site hands
//! the same file to people's own BitTorrent clients:
//!
//! - `GET /torrent` ([`PATH`]) hands it out, as `skhid-rev<n>.torrent`;
//! - the home page links it and its magnet (`btih`, `btmh`, `dn`, `tr`).
//!
//! A ZIP without it (an older one, `release/deploy`'s) has no torrent. The
//! entry is read whenever it is asked for (it is small) and parsed once into
//! [`Torrent`]: one that cannot be read whole as a torrent is as if there
//! were none.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{Shutdown, TcpStream};
use std::path::Path;

pub(super) const PATH: &str = "/torrent";
/// Its name in the ZIP: beside the updater. Stored, not compressed.
const ENTRY: &[u8] = b"game/binaries/x86/skhid.torrent";
/// The most a torrent file may weigh (the release's are about 200 KB).
pub(super) const MAX_SIZE: u64 = 4 << 20;
/// The most a ZIP's central directory may weigh (the players' has some 80
/// entries).
const MAX_DIRECTORY: u64 = 1 << 20;
/// How deep lists and dictionaries may nest in it.
const MAX_DEPTH: usize = 32;

/// A torrent file as it was read, and what the site says of it.
pub(super) struct Torrent {
    pub bytes: Vec<u8>,
    /// SHA-1 of its `info` (`btih`, the v1 info hash).
    pub v1: [u8; 20],
    /// SHA-256 of its `info` (`btmh`, the v2 info hash).
    pub v2: [u8; 32],
    /// Its `info.name`.
    pub name: String,
    /// Its trackers: `announce-list` in order (else `announce`).
    pub trackers: Vec<String>,
}

/// The torrent in the ZIP `download`, when it holds one.
pub(super) fn in_zip(download: &Path) -> Option<Torrent> {
    let bytes = match entry(download) {
        Entry::Absent => return None,
        Entry::Unreadable(why) => {
            log::warn!("Torrent: {} {why}; not offered", download.display());
            return None;
        }
        Entry::Stored(bytes) => bytes,
    };
    let torrent = parse(bytes);
    if torrent.is_none() {
        log::warn!(
            "Torrent: the one in {} is not a torrent this site reads; not offered",
            download.display()
        );
    }
    torrent
}

/// What the ZIP holds as [`ENTRY`].
enum Entry {
    Absent,
    Unreadable(&'static str),
    Stored(Vec<u8>),
}

fn u16_at(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u16::from_le_bytes(
        data.get(at..at + 2)?.try_into().ok()?,
    )))
}

fn u32_at(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from(u32::from_le_bytes(
        data.get(at..at + 4)?.try_into().ok()?,
    )))
}

fn read_at(file: &mut File, offset: u64, length: u64) -> Option<Vec<u8>> {
    let mut bytes = vec![0; usize::try_from(length).ok()?];
    file.seek(SeekFrom::Start(offset)).ok()?;
    file.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

/// [`ENTRY`] of the ZIP: its end record (in the last 64 KiB and 22 bytes),
/// its central directory, the entry's local header, its stored bytes.
fn entry(download: &Path) -> Entry {
    let unreadable = Entry::Unreadable("cannot be read as a ZIP");
    let Some(directory) = (|| {
        let mut file = File::open(download).ok()?;
        let size = file.metadata().ok()?.len();
        let tail_length = size.min(22 + 0xffff);
        let tail = read_at(&mut file, size - tail_length, tail_length)?;
        let end = (0..=tail.len().checked_sub(22)?)
            .rev()
            .find(|&at| tail[at..at + 4] == *b"PK\x05\x06")?;
        let (length, offset) = (u32_at(&tail, end + 12)?, u32_at(&tail, end + 16)?);
        if length > MAX_DIRECTORY {
            return None;
        }
        let directory = read_at(&mut file, offset, length)?;
        Some((file, directory))
    })() else {
        return unreadable;
    };
    let (mut file, directory) = directory;
    let mut at = 0;
    while directory.get(at..at + 4) == Some(b"PK\x01\x02") {
        let Some(fields) = (|| {
            let (method, stored, size) = (
                u16_at(&directory, at + 10)?,
                u32_at(&directory, at + 20)?,
                u32_at(&directory, at + 24)?,
            );
            let (name, extra, comment) = (
                u16_at(&directory, at + 28)? as usize,
                u16_at(&directory, at + 30)? as usize,
                u16_at(&directory, at + 32)? as usize,
            );
            let local = u32_at(&directory, at + 42)?;
            let named = directory.get(at + 46..at + 46 + name)? == ENTRY;
            Some((
                named,
                method,
                stored,
                size,
                local,
                46 + name + extra + comment,
            ))
        })() else {
            return unreadable;
        };
        let (named, method, stored, size, local, length) = fields;
        if named {
            if method != 0 || stored != size {
                return Entry::Unreadable("holds the torrent compressed");
            }
            if size > MAX_SIZE {
                return Entry::Unreadable("holds a torrent over 4 MiB");
            }
            let Some(bytes) = (|| {
                let header = read_at(&mut file, local, 30)?;
                if header.get(..4)? != b"PK\x03\x04" {
                    return None;
                }
                let skip = 30 + u16_at(&header, 26)? + u16_at(&header, 28)?;
                read_at(&mut file, local + skip, size)
            })() else {
                return unreadable;
            };
            return Entry::Stored(bytes);
        }
        at += length;
    }
    Entry::Absent
}

/// A bencoded value; strings as their bytes.
enum Value<'a> {
    Int,
    Bytes(&'a [u8]),
    List(Vec<Value<'a>>),
    Dict(Vec<(&'a [u8], Value<'a>)>),
}

impl<'a> Value<'a> {
    fn get(&self, key: &[u8]) -> Option<&Value<'a>> {
        match self {
            Value::Dict(entries) => entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn text(&self) -> Option<&'a str> {
        match self {
            Value::Bytes(bytes) => std::str::from_utf8(bytes).ok(),
            _ => None,
        }
    }
}

/// The value at `at` and where it ends.
fn value(data: &[u8], at: usize, depth: usize) -> Option<(Value<'_>, usize)> {
    match *data.get(at)? {
        b'i' => {
            let end = at + 1 + data.get(at + 1..)?.iter().position(|&b| b == b'e')?;
            let digits = data.get(at + 1..end)?;
            let digits = digits.strip_prefix(b"-").unwrap_or(digits);
            (!digits.is_empty() && digits.iter().all(u8::is_ascii_digit))
                .then_some((Value::Int, end + 1))
        }
        kind @ (b'l' | b'd') => {
            if depth >= MAX_DEPTH {
                return None;
            }
            let (mut at, mut list, mut dict) = (at + 1, Vec::new(), Vec::new());
            while *data.get(at)? != b'e' {
                if kind == b'd' {
                    let (Value::Bytes(key), next) = value(data, at, depth + 1)? else {
                        return None;
                    };
                    let (item, next) = value(data, next, depth + 1)?;
                    dict.push((key, item));
                    at = next;
                } else {
                    let (item, next) = value(data, at, depth + 1)?;
                    list.push(item);
                    at = next;
                }
            }
            Some((
                if kind == b'd' {
                    Value::Dict(dict)
                } else {
                    Value::List(list)
                },
                at + 1,
            ))
        }
        b'0'..=b'9' => {
            let colon = at + data.get(at..)?.iter().position(|&b| b == b':')?;
            let length: usize = std::str::from_utf8(&data[at..colon]).ok()?.parse().ok()?;
            let end = colon.checked_add(1)?.checked_add(length)?;
            Some((Value::Bytes(data.get(colon + 1..end)?), end))
        }
        _ => None,
    }
}

/// The torrent `bytes` hold: one dictionary, all of the file, with an
/// `info` dictionary whose `name` is text, and text trackers.
fn parse(bytes: Vec<u8>) -> Option<Torrent> {
    if bytes.first() != Some(&b'd') {
        return None;
    }
    // The top dictionary's entries, each value's bytes kept: the info hash
    // is of `info` exactly as the file has it.
    let (mut at, mut info, mut trackers) = (1, None, Vec::new());
    let mut announce = None;
    while *bytes.get(at)? != b'e' {
        let (Value::Bytes(key), start) = value(&bytes, at, 1)? else {
            return None;
        };
        let (item, end) = value(&bytes, start, 1)?;
        match key {
            b"info" => info = Some((start, end, item)),
            b"announce" => announce = Some(item.text()?.to_owned()),
            b"announce-list" => {
                let Value::List(tiers) = item else {
                    return None;
                };
                for tier in &tiers {
                    let Value::List(urls) = tier else { return None };
                    for url in urls {
                        trackers.push(url.text()?.to_owned());
                    }
                }
            }
            _ => {}
        }
        at = end;
    }
    if at + 1 != bytes.len() {
        return None;
    }
    let (start, end, info) = info?;
    let name = info.get(b"name")?.text()?.to_owned();
    if trackers.is_empty() {
        trackers.extend(announce);
    }
    let raw = &bytes[start..end];
    Some(Torrent {
        v1: openssl::sha::sha1(raw),
        v2: openssl::sha::sha256(raw),
        name,
        trackers,
        bytes,
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// A URI component: unreserved characters as they are, every other byte
/// `%XX`.
fn component(text: &str) -> String {
    text.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

impl Torrent {
    /// Its magnet link: both info hashes (`btmh` is the multihash
    /// `1220<SHA-256>`), its name and its trackers.
    pub(super) fn magnet(&self) -> String {
        let mut link = format!(
            "magnet:?xt=urn:btih:{}&xt=urn:btmh:1220{}&dn={}",
            hex(&self.v1),
            hex(&self.v2),
            component(&self.name)
        );
        for tracker in &self.trackers {
            link += "&tr=";
            link += &component(tracker);
        }
        link
    }
}

/// `GET /torrent`: the file, named after the revision the server lets in
/// (the release's: the torrent is put in place with it).
pub(super) fn send(stream: &mut TcpStream, head_only: bool, torrent: &Torrent, revision: u8) {
    let head = format!(
        "HTTP/1.0 200 OK\r\nContent-Type: application/x-bittorrent\r\n\
         Content-Disposition: attachment; filename=\"skhid-rev{revision}.torrent\"\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        torrent.bytes.len()
    );
    let _ = stream.write_all(head.as_bytes());
    if !head_only {
        let _ = stream.write_all(&torrent.bytes);
    }
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Write);
}

#[cfg(test)]
mod tests {
    use super::*;

    const INFO: &[u8] = b"d4:name9:Survarium12:piece lengthi4194304ee";

    fn file(info: &[u8], rest: &[u8]) -> Vec<u8> {
        let mut bytes =
            b"d8:announce5:udp:x13:announce-listll5:udp:xel9:http://y/ee4:info".to_vec();
        bytes.extend_from_slice(info);
        bytes.extend_from_slice(rest);
        bytes
    }

    #[test]
    fn the_info_hashes_are_of_info_as_the_file_has_it() {
        let torrent = parse(file(INFO, b"12:piece layersdee")).unwrap();
        assert_eq!(torrent.v1, openssl::sha::sha1(INFO));
        assert_eq!(torrent.v2, openssl::sha::sha256(INFO));
        assert_eq!(torrent.name, "Survarium");
        assert_eq!(torrent.trackers, ["udp:x", "http://y/"]);
        assert_eq!(
            torrent.magnet(),
            format!(
                "magnet:?xt=urn:btih:{}&xt=urn:btmh:1220{}&dn=Survarium&tr=udp%3Ax&tr=http%3A%2F%2Fy%2F",
                hex(&openssl::sha::sha1(INFO)),
                hex(&openssl::sha::sha256(INFO))
            )
        );
    }

    #[test]
    fn anything_but_one_whole_torrent_is_refused() {
        for bad in [
            file(INFO, b"e").split_last().unwrap().1.to_vec(), // cut short
            file(INFO, b"ee"),                                 // more after it
            file(b"d12:piece lengthi1ee", b"e"),               // no name
            file(b"d4:name3:ab", b"e"),                        // a string past the end
            file(b"d4:namei1ee", b"e"),                        // a name that is no text
            file(INFO, b"1:ki-ee"),                            // no digits
            b"le".to_vec(),
            Vec::new(),
        ] {
            assert!(parse(bad).is_none());
        }
        let deep = [b"d4:info".as_slice(), &[b'l'; 64], &[b'e'; 64], b"e"].concat();
        assert!(parse(deep).is_none());
        assert!(parse(b"d8:announce1:xe".to_vec()).is_none(), "no info");
    }

    /// A ZIP of `entries` (name, bytes, method), as Python's zipfile writes
    /// one; a method other than 0 only says so (the bytes are as given).
    fn zip(entries: &[(&[u8], &[u8], u16)]) -> Vec<u8> {
        let (mut out, mut directory) = (Vec::new(), Vec::new());
        for (name, data, method) in entries {
            let offset = out.len() as u32;
            let header = |sig: &[u8]| {
                let mut h = sig.to_vec();
                if sig == b"PK\x01\x02" {
                    h.extend([20, 3]);
                }
                h.extend([20, 0, 0, 0]);
                h.extend(method.to_le_bytes());
                h.extend([0; 8]); // time, date, CRC
                h.extend((data.len() as u32).to_le_bytes());
                h.extend((data.len() as u32).to_le_bytes());
                h.extend((name.len() as u16).to_le_bytes());
                h.extend([0, 0]);
                if sig == b"PK\x01\x02" {
                    h.extend([0; 6]); // comment length, disk, internal attributes
                    h.extend([0, 0, 0xa4, 0x81]);
                    h.extend(offset.to_le_bytes());
                }
                h.extend(*name);
                h
            };
            out.extend(header(b"PK\x03\x04"));
            out.extend(*data);
            directory.extend(header(b"PK\x01\x02"));
        }
        let at = out.len() as u32;
        out.extend(&directory);
        out.extend(b"PK\x05\x06\0\0\0\0");
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((directory.len() as u32).to_le_bytes());
        out.extend(at.to_le_bytes());
        out.extend([0, 0]);
        out
    }

    #[test]
    fn the_torrent_is_the_zips_stored_entry() {
        let dir = std::env::temp_dir().join(format!("srp-torrent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let download = dir.join("files.zip");
        let torrent = file(INFO, b"e");
        let with = |entries: &[(&[u8], &[u8], u16)]| {
            std::fs::write(&download, zip(entries)).unwrap();
            in_zip(&download)
        };
        let readme: (&[u8], &[u8], u16) = (b"README.txt", b"read me", 0);
        assert!(with(&[readme]).is_none(), "an older ZIP: none");
        let found = with(&[readme, (ENTRY, &torrent, 0)]).unwrap();
        assert_eq!(
            (found.name.as_str(), found.bytes.as_slice()),
            ("Survarium", torrent.as_slice())
        );
        assert!(with(&[(ENTRY, &torrent, 8)]).is_none(), "compressed");
        assert!(with(&[(ENTRY, b"not a torrent", 0)]).is_none());
        let large = vec![b'x'; MAX_SIZE as usize + 1];
        assert!(with(&[(ENTRY, &large, 0)]).is_none(), "over 4 MiB");
        std::fs::write(&download, b"not a zip").unwrap();
        assert!(in_zip(&download).is_none());
        assert!(in_zip(&dir.join("gone.zip")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

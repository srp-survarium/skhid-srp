//! The torrent inside the ZIP at `/download`, handed to people's own
//! BitTorrent clients as `GET /torrent`
//! (`docs/notes/torrent-downloads.md`, "The format"). The updater needs
//! none of this: it takes the torrent from the ZIP it installed. One that
//! cannot be read whole as a torrent is as if there were none.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{Shutdown, TcpStream};
use std::path::Path;

pub(super) const PATH: &str = "/torrent";
/// The bootstrap script ("Bootstrap" in the spec), when
/// release/build-bootstrap made one.
pub(super) const BOOTSTRAP_PATH: &str = "/Skhid-get.bat";
pub(super) const BOOTSTRAP: Option<&[u8]> = include!(concat!(env!("OUT_DIR"), "/bootstrap.rs"));
/// Stored, not compressed.
const ENTRY: &[u8] = b"game/torrent/skhid.torrent";
/// The release's are about 200 KB.
pub(super) const MAX_SIZE: u64 = 4 << 20;
/// The players' ZIP has some 80 entries.
const MAX_DIRECTORY: u64 = 1 << 20;
const MAX_DEPTH: usize = 32;

pub(super) struct Torrent {
    pub bytes: Vec<u8>,
}

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

/// The end record is in the last 64 KiB and 22 bytes; it names the central
/// directory, which names the entry's local header.
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

enum Value<'a> {
    Int,
    Bytes(&'a [u8]),
    List,
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
            let (mut at, mut dict) = (at + 1, Vec::new());
            while *data.get(at)? != b'e' {
                if kind == b'd' {
                    let (Value::Bytes(key), next) = value(data, at, depth + 1)? else {
                        return None;
                    };
                    let (item, next) = value(data, next, depth + 1)?;
                    dict.push((key, item));
                    at = next;
                } else {
                    at = value(data, at, depth + 1)?.1;
                }
            }
            Some((
                if kind == b'd' {
                    Value::Dict(dict)
                } else {
                    Value::List
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

/// One dictionary, all of the file, with an `info` whose `name` is text.
fn parse(bytes: Vec<u8>) -> Option<Torrent> {
    if bytes.first() != Some(&b'd') {
        return None;
    }
    let (mut at, mut named) = (1, false);
    while *bytes.get(at)? != b'e' {
        let (Value::Bytes(key), start) = value(&bytes, at, 1)? else {
            return None;
        };
        let (item, end) = value(&bytes, start, 1)?;
        if key == b"info" {
            named = item.get(b"name")?.text().is_some();
        }
        at = end;
    }
    (named && at + 1 == bytes.len()).then_some(Torrent { bytes })
}

/// Named after the revision the server lets in: the torrent is put in place
/// with it.
pub(super) fn send(stream: &mut TcpStream, head_only: bool, torrent: &Torrent, revision: u8) {
    attachment(
        stream,
        head_only,
        "application/x-bittorrent",
        &format!("skhid-rev{revision}.torrent"),
        &torrent.bytes,
    );
}

pub(super) fn send_bootstrap(stream: &mut TcpStream, head_only: bool, script: &[u8]) {
    attachment(
        stream,
        head_only,
        "application/octet-stream",
        "Skhid-get.bat",
        script,
    );
}

fn attachment(stream: &mut TcpStream, head_only: bool, kind: &str, name: &str, bytes: &[u8]) {
    let head = format!(
        "HTTP/1.0 200 OK\r\nContent-Type: {kind}\r\n\
         Content-Disposition: attachment; filename=\"{name}\"\r\n\
         Content-Length: {}\r\nCache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n",
        bytes.len()
    );
    let _ = stream.write_all(head.as_bytes());
    if !head_only {
        let _ = stream.write_all(bytes);
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

    /// As Python's zipfile writes one; a method other than 0 only says so
    /// (the bytes are as given).
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
        assert_eq!(found.bytes, torrent);
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

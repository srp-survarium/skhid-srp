//! The torrent of the whole game folder of the revision `/download` belongs
//! to (`docs/notes/torrent-downloads.md`): the file [`FILE_NAME`] beside the
//! download file, which the release puts there with the ZIP
//! (`release/build-torrent`).
//!
//! - `GET /torrent` ([`PATH`]) hands it out, as `skhid-rev<n>.torrent`;
//! - `/api/client-files` names its info hash (`torrent <64 hex>`: the
//!   SHA-256 of its bencoded `info`, BEP 52's), so the updater takes only
//!   this file;
//! - the home page links it and its magnet (`btih`, `btmh`, `dn`, `tr`).
//!
//! The file is read whenever it is asked for (it is small) and parsed once
//! into [`Torrent`]: one that cannot be read whole as a torrent is as if
//! there were none.

use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::{Path, PathBuf};

pub(super) const PATH: &str = "/torrent";
/// Its name in the download file's folder.
const FILE_NAME: &str = "skhid.torrent";
/// The most a torrent file may weigh (the release's are about 200 KB).
pub(super) const MAX_SIZE: u64 = 4 << 20;
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

/// The torrent beside the download file `download`, when there is one.
pub(super) fn beside(download: &Path) -> Option<Torrent> {
    read(&download.parent()?.join(FILE_NAME))
}

fn read(file: &PathBuf) -> Option<Torrent> {
    let mut bytes = Vec::new();
    std::fs::File::open(file)
        .ok()?
        .take(MAX_SIZE + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_SIZE {
        log::warn!(
            "Torrent: {} is over {MAX_SIZE} bytes; not offered",
            file.display()
        );
        return None;
    }
    let torrent = parse(bytes);
    if torrent.is_none() {
        log::warn!(
            "Torrent: {} is not a torrent this site reads; not offered",
            file.display()
        );
    }
    torrent
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

pub(super) fn hex(bytes: &[u8]) -> String {
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
    /// `/api/client-files`'s line: `torrent <SHA-256 of info>`.
    pub(super) fn line(&self) -> String {
        format!("torrent {}\n", hex(&self.v2))
    }

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
            torrent.line(),
            format!("torrent {}\n", hex(&openssl::sha::sha256(INFO)))
        );
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

    #[test]
    fn the_file_beside_the_download_is_read_and_a_large_one_refused() {
        let dir = std::env::temp_dir().join(format!("srp-torrent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let download = dir.join("files.zip");
        assert!(beside(&download).is_none());
        std::fs::write(dir.join(FILE_NAME), file(INFO, b"e")).unwrap();
        assert_eq!(beside(&download).unwrap().name, "Survarium");
        let mut large = file(INFO, b"7:padding");
        large.extend(format!("{}:", MAX_SIZE).bytes());
        large.resize(large.len() + MAX_SIZE as usize, b'x');
        large.push(b'e');
        std::fs::write(dir.join(FILE_NAME), large).unwrap();
        assert!(beside(&download).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! What the updater shares without hashing it again
//! (`srp-026g/docs/notes/torrent-downloads.md`, "What it shares"): each file
//! of a torrent that libtorrent found whole, every piece checked, with what
//! Windows said of it then (`crate::checked::Identity`: size, last-write
//! time, file ID). At a later start a file whose identity is still that is
//! shared as checked; any other is checked again first (so libtorrent hashes
//! from the first such file on). A file is still checked against its pin
//! before it is installed, whatever this says.
//!
//! The record, `updater_skhid.torrent.checked` beside the updater:
//!
//! ```text
//! skhid-torrent-checked 1
//! file <v2 info hash> <size> <written> <volume> <index> <path>
//! end
//! ```
//!
//! The path is where the file is (beside the updater, or in the game
//! folder), last on its line. A record without its `end`, of another first
//! line, or with a line that cannot be read is no record: everything is
//! checked.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::checked::Identity;
use crate::sha256::{from_hex, to_hex};

const HEADER: &str = "skhid-torrent-checked 1";
const END: &str = "end";
/// More lines than any folder has: a record past it is no record.
const MAX_FILES: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shared {
    pub torrent: [u8; 32],
    pub identity: Identity,
    pub path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    pub files: Vec<Shared>,
}

impl Record {
    /// Whether the file at `path` of `torrent` is what was checked.
    pub fn trusted(&self, torrent: &[u8; 32], path: &str, identity: &Identity) -> bool {
        identity.usable()
            && self
                .files
                .iter()
                .any(|file| file.torrent == *torrent && file.path.eq_ignore_ascii_case(path) && file.identity == *identity)
    }

    /// `torrent`'s files are now `files`; the records of torrents not in
    /// `kept` go.
    pub fn set(&mut self, torrent: &[u8; 32], files: Vec<(String, Identity)>, kept: &[[u8; 32]]) {
        self.files.retain(|file| file.torrent != *torrent && kept.contains(&file.torrent));
        self.files.extend(
            files
                .into_iter()
                .filter(|(_, identity)| identity.usable())
                .map(|(path, identity)| Shared { torrent: *torrent, identity, path }),
        );
    }
}

/// The record's bytes.
pub fn write_record(record: &Record) -> Vec<u8> {
    let mut out = format!("{HEADER}\r\n");
    for file in &record.files {
        let id = &file.identity;
        out.push_str(&format!(
            "file {} {} {} {} {} {}\r\n",
            String::from_utf8_lossy(&to_hex(&file.torrent)),
            id.size,
            id.written,
            id.volume,
            id.index,
            file.path
        ));
    }
    out.push_str(END);
    out.push_str("\r\n");
    out.into_bytes()
}

/// The record; `None` when it is not one.
pub fn parse_record(data: &[u8]) -> Option<Record> {
    let text = core::str::from_utf8(data).ok()?;
    let mut lines = text.lines();
    if lines.next()? != HEADER {
        return None;
    }
    let mut record = Record::default();
    for line in lines {
        if line == END {
            return Some(record);
        }
        let mut words = line.splitn(7, ' ');
        if words.next()? != "file" || record.files.len() >= MAX_FILES {
            return None;
        }
        let torrent = from_hex(words.next()?.as_bytes())?;
        let identity = Identity {
            size: words.next()?.parse().ok()?,
            written: words.next()?.parse().ok()?,
            volume: words.next()?.parse().ok()?,
            index: words.next()?.parse().ok()?,
        };
        let path = words.next().filter(|path| !path.is_empty())?;
        record.files.push(Shared { torrent, identity, path: path.into() });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(size: u64) -> Identity {
        Identity { size, written: 5, volume: 6, index: 7 }
    }

    #[test]
    fn a_file_is_trusted_only_as_it_was_checked() {
        let mut record = Record::default();
        let a = [1; 32];
        let b = [2; 32];
        record.set(&a, alloc::vec![("C:\\Game\\game\\resources.db".into(), identity(10))], &[a]);
        record.set(&b, alloc::vec![("C:\\Game\\x y.exe".into(), identity(3))], &[a, b]);
        let again = parse_record(&write_record(&record)).unwrap();
        assert_eq!(again, record);
        assert!(again.trusted(&a, "c:\\game\\game\\resources.db", &identity(10)));
        assert!(!again.trusted(&a, "C:\\Game\\game\\resources.db", &identity(11)));
        assert!(!again.trusted(&b, "C:\\Game\\game\\resources.db", &identity(10)));
        assert!(again.trusted(&b, "C:\\Game\\x y.exe", &identity(3)));
        // A file system that cannot tell its files: never trusted.
        let unusable = Identity { index: 0, ..identity(3) };
        record.set(&b, alloc::vec![("C:\\other".into(), unusable)], &[a, b]);
        assert!(!record.trusted(&b, "C:\\other", &unusable));
        // Another torrent's set, `a` no longer kept: its files go.
        record.set(&b, alloc::vec![], &[b]);
        assert!(record.files.is_empty());
    }

    #[test]
    fn a_record_cut_short_is_none() {
        let mut record = Record::default();
        record.set(&[1; 32], alloc::vec![("C:\\a".into(), identity(1))], &[]);
        let data = write_record(&record);
        assert!(parse_record(&data[..data.len() - 5]).is_none());
        assert!(parse_record(b"skhid-torrent-checked 2\r\nend\r\n").is_none());
        assert_eq!(parse_record(b"skhid-torrent-checked 1\r\nend\r\n"), Some(Record::default()));
    }
}

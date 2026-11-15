//! The files libtorrent found whole, kept so that 15.5 GB are not hashed
//! again at every start (`client/updater/README.md`, "BitTorrent"). Only a
//! file whose identity is still the one checked is trusted; a file is still
//! checked against its pin before it is installed, whatever this says.
//!
//! `updater_skhid.torrent.checked`, beside the updater:
//!
//! ```text
//! skhid-torrent-checked 1
//! file <v2 info hash> <size> <written> <volume> <index> <path>
//! end
//! ```
//!
//! The path is last, so it may hold spaces. Anything but a whole record
//! is none: everything is checked.

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

    /// The files of `torrent` just checked, each with its identity when it
    /// was found whole. A check of some files keeps the others; torrents
    /// not in `kept` go.
    pub fn set(&mut self, torrent: &[u8; 32], checked: Vec<(String, Option<Identity>)>, kept: &[[u8; 32]]) {
        self.files.retain(|file| {
            let again = file.torrent == *torrent && checked.iter().any(|(path, _)| path.eq_ignore_ascii_case(&file.path));
            kept.contains(&file.torrent) && !again
        });
        self.files.extend(checked.into_iter().filter_map(|(path, identity)| {
            identity.filter(Identity::usable).map(|identity| Shared { torrent: *torrent, identity, path })
        }));
    }
}

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
        record.set(&a, alloc::vec![("C:\\Game\\game\\resources.db".into(), Some(identity(10)))], &[a]);
        record.set(&b, alloc::vec![("C:\\Game\\x y.exe".into(), Some(identity(3)))], &[a, b]);
        let again = parse_record(&write_record(&record)).unwrap();
        assert_eq!(again, record);
        assert!(again.trusted(&a, "c:\\game\\game\\resources.db", &identity(10)));
        assert!(!again.trusted(&a, "C:\\Game\\game\\resources.db", &identity(11)));
        assert!(!again.trusted(&b, "C:\\Game\\game\\resources.db", &identity(10)));
        assert!(again.trusted(&b, "C:\\Game\\x y.exe", &identity(3)));
        // A file system that cannot tell its files: never trusted.
        let unusable = Identity { index: 0, ..identity(3) };
        record.set(&b, alloc::vec![("C:\\other".into(), Some(unusable))], &[a, b]);
        assert!(!record.trusted(&b, "C:\\other", &unusable));
        // `a` no longer kept: its files go; `b`'s file not whole now: it goes.
        record.set(&b, alloc::vec![("C:\\Game\\x y.exe".into(), None)], &[b]);
        assert!(record.files.is_empty());
    }

    #[test]
    fn a_check_of_some_files_keeps_the_others() {
        let mut record = Record::default();
        let a = [1; 32];
        let (db, patch) = ("C:\\Game\\game\\resources.db", "C:\\Game\\game\\resources.db.patch_12");
        record.set(&a, alloc::vec![(db.into(), Some(identity(10))), (patch.into(), Some(identity(4)))], &[a]);
        record.set(&a, alloc::vec![(patch.into(), None)], &[a]);
        assert!(record.trusted(&a, db, &identity(10)));
        assert!(!record.trusted(&a, patch, &identity(4)));
        record.set(&a, alloc::vec![(patch.into(), Some(identity(5)))], &[a]);
        assert!(record.trusted(&a, db, &identity(10)) && record.trusted(&a, patch, &identity(5)));
    }

    #[test]
    fn a_record_cut_short_is_none() {
        let mut record = Record::default();
        record.set(&[1; 32], alloc::vec![("C:\\a".into(), Some(identity(1)))], &[]);
        let data = write_record(&record);
        assert!(parse_record(&data[..data.len() - 5]).is_none());
        assert!(parse_record(b"skhid-torrent-checked 2\r\nend\r\n").is_none());
        assert_eq!(parse_record(b"skhid-torrent-checked 1\r\nend\r\n"), Some(Record::default()));
    }
}

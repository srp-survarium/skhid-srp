//! The release's signature on a revision's torrent
//! (`srp-026g/docs/notes/torrent-downloads.md`, "Signed releases" and "The
//! signature"): a top-level `skhid` dictionary beside `info` with the
//! revision (1 to 255) and an Ed25519 signature (RFC 8032, no prehash) of
//! [`DOMAIN`], the revision's byte and the SHA-256 of the bencoded `info`. A torrent is trusted by this alone: whoever handed it
//! over, its files are the release's (each piece then by SHA-256,
//! `crate::torrent`).

use crate::bencode::{self, Value};
use crate::sha256::digest;

/// What the signed message starts with: a signature for anything else
/// (another program's, another format's) is never one of these.
pub const DOMAIN: &[u8; 26] = b"skhid torrent signature 1\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignedError {
    /// No `skhid` signature, or not one of this form.
    Unsigned,
    /// A signature, not the release's key's for this torrent.
    Wrong,
}

/// The 59 bytes a release signs: [`DOMAIN`], the revision, the SHA-256 of
/// `info`.
pub fn message(revision: u8, info_sha256: &[u8; 32]) -> [u8; 59] {
    let mut message = [0_u8; 59];
    message[..26].copy_from_slice(DOMAIN);
    message[26] = revision;
    message[27..].copy_from_slice(info_sha256);
    message
}

/// The revision the torrent is of, when `public_key` signed it.
pub fn verify(torrent: &[u8], public_key: &[u8; 32]) -> Result<u8, SignedError> {
    let document = bencode::parse(torrent).ok_or(SignedError::Unsigned)?;
    let (_, info) = document.entry(b"info").ok_or(SignedError::Unsigned)?;
    let skhid = document.get(b"skhid").ok_or(SignedError::Unsigned)?;
    let revision = skhid
        .get(b"revision")
        .and_then(Value::int)
        .and_then(|revision| u8::try_from(revision).ok())
        .filter(|&revision| revision > 0)
        .ok_or(SignedError::Unsigned)?;
    let signature = skhid
        .get(b"signature")
        .and_then(Value::bytes)
        .and_then(|bytes| ed25519_compact::Signature::from_slice(bytes).ok())
        .ok_or(SignedError::Unsigned)?;
    let key = ed25519_compact::PublicKey::new(*public_key);
    key.verify(message(revision, &digest(info)), &signature)
        .map(|()| revision)
        .map_err(|_| SignedError::Wrong)
}

/// A public key as `release/signing.pub` holds it: 64 hex digits (and a
/// line end).
pub fn public_key(text: &str) -> Option<[u8; 32]> {
    crate::sha256::from_hex(text.trim().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::vec::Vec;

    /// A throwaway key, for these tests only.
    fn key() -> ed25519_compact::KeyPair {
        ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new([7; 32]))
    }

    /// `torrent` (a dictionary) with an `skhid` signature of `revision`
    /// added at its end (`skhid` sorts after `info` and `piece layers`).
    pub(crate) fn sign(torrent: &[u8], revision: u8, pair: &ed25519_compact::KeyPair) -> Vec<u8> {
        let info = bencode::parse(torrent).unwrap().entry(b"info").unwrap().1;
        let signature = pair.sk.sign(message(revision, &digest(info)), None);
        let mut out = torrent[..torrent.len() - 1].to_vec();
        out.extend_from_slice(format!("5:skhidd8:revisioni{revision}e9:signature64:").as_bytes());
        out.extend_from_slice(signature.as_ref());
        out.extend_from_slice(b"ee");
        out
    }

    #[test]
    fn only_the_releases_signature_of_this_torrent_is_taken() {
        let torrent = crate::torrent::make(&[("game/a", b"abc")], 32 * 1024, &[]);
        let public = *key().pk;
        let signed = sign(&torrent, 60, &key());
        assert_eq!(verify(&signed, &public), Ok(60));
        assert!(crate::torrent::parse(&signed).is_ok());
        assert_eq!(verify(&torrent, &public), Err(SignedError::Unsigned));
        // Another key's.
        let other = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new([8; 32]));
        assert_eq!(verify(&sign(&torrent, 60, &other), &public), Err(SignedError::Wrong));
        // Another revision said than the one signed.
        let text = signed.clone();
        let at = text.windows(4).position(|w| w == b"i60e").unwrap();
        let mut changed = text.clone();
        changed[at + 2] = b'1';
        assert_eq!(verify(&changed, &public), Err(SignedError::Wrong));
        // Another info.
        let mut other_info = signed.clone();
        let at = other_info.windows(3).position(|w| w == b"1:a").map(|at| at + 2).unwrap();
        other_info[at] = b'b';
        assert_eq!(verify(&other_info, &public), Err(SignedError::Wrong));
        assert_eq!(public_key(&format!("{}\n", "ab".repeat(32))), Some([0xAB; 32]));
        assert_eq!(public_key("ab"), None);
    }
}

#[cfg(test)]
mod release {
    use super::*;

    /// A torrent of `release/build_torrent.py` (revision 61, a ZIP of
    /// README.txt and a 5 MB updater_skhid.exe, a patch), signed with the
    /// throwaway key of `tests/test_torrent.py` (seed bytes 0 to 31).
    const RELEASE: &[u8] = include_bytes!("../tests/data/release.torrent");
    const KEY: &str = "03a107bff3ce10be1d70dd18e74bc09967e4d6309ba50d5f1ddc8664125531b8";

    #[test]
    fn the_release_builders_torrent_is_read_and_verified() {
        assert_eq!(verify(RELEASE, &public_key(KEY).unwrap()), Ok(61));
        let torrent = crate::torrent::parse(RELEASE).unwrap();
        let paths: alloc::vec::Vec<&str> = torrent.files.iter().map(|file| file.path.as_str()).collect();
        assert_eq!(paths, ["README.txt", "game/binaries/x86/updater_skhid.exe", "game/resources.db.patch_12"]);
        assert_eq!(torrent.piece_length, 4 << 20);
        let readme = torrent.file_pieces(0);
        let mut piece = b"read me\r\n".to_vec();
        piece.resize(torrent.piece_size(readme.start) as usize, 0);
        assert!(torrent.piece_ok(readme.start, &piece));
        let exe = torrent.file_pieces(1);
        assert_eq!(exe.len(), 2);
        let mut first = alloc::vec![0_u8; 4 << 20];
        first[..2].copy_from_slice(b"MZ");
        assert!(torrent.piece_ok(exe.start, &first));
    }
}

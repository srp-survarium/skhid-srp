//! The newest release on the DHT (`srp-026g/docs/notes/torrent-downloads.md`,
//! "The newest release"): a BEP 46 mutable item under the release's
//! Ed25519 key, so that an updater that cannot reach our site still finds
//! the newest signed torrent.
//!
//! The item: salt [`SALT`], sequence number the revision, and the value
//! [`Newest`] bencoded: `d1:i32:<v2 info hash>1:ri<revision>e1:s64:<the
//! torrent's signature>e`, the facts of the torrent's `skhid` signature
//! (`crate::signed`). Its BEP 44 signature, by the same key, is the
//! torrent's `skhid.dht` (`release/build_torrent.py` makes both), so every
//! updater that has the torrent can put the item again as it is. An item
//! read is taken only when its signature verifies; the torrent's `info`
//! then comes from the swarm (BEP 9), is checked against the item's info
//! hash, and makes the signed torrent again with [`torrent`] (no piece
//! layers: libtorrent gets them from peers).

use alloc::format;
use alloc::vec::Vec;

use crate::bencode::{self, Value};
use crate::sha256::digest;

/// The item's salt (BEP 44): one name for "the newest release".
pub const SALT: &[u8] = b"skhid-newest 1";

/// What the item says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Newest {
    /// The SHA-256 of the torrent's `info` (its v2 info hash).
    pub v2: [u8; 32],
    pub revision: u8,
    /// The torrent's `skhid.signature`.
    pub signature: [u8; 64],
}

/// An item as the DHT gives it, or as this updater puts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub seq: i64,
    /// The bencoded value.
    pub value: Vec<u8>,
    pub signature: [u8; 64],
}

fn bytes(out: &mut Vec<u8>, data: &[u8]) {
    out.extend_from_slice(format!("{}:", data.len()).as_bytes());
    out.extend_from_slice(data);
}

/// The bencoded value of `newest`.
pub fn value(newest: &Newest) -> Vec<u8> {
    let mut out = b"d1:i".to_vec();
    bytes(&mut out, &newest.v2);
    out.extend_from_slice(format!("1:ri{}e1:s", newest.revision).as_bytes());
    bytes(&mut out, &newest.signature);
    out.push(b'e');
    out
}

/// A value of exactly that form.
pub fn parse_value(data: &[u8]) -> Option<Newest> {
    let document = bencode::parse(data)?;
    let newest = Newest {
        v2: document.get(b"i")?.bytes()?.try_into().ok()?,
        revision: u8::try_from(document.get(b"r")?.int()?).ok().filter(|&r| r > 0)?,
        signature: document.get(b"s")?.bytes()?.try_into().ok()?,
    };
    (value(&newest) == data).then_some(newest)
}

/// What BEP 44 signs: `4:salt<salt>3:seqi<seq>e1:v<value>`.
pub fn signed_bytes(value: &[u8], seq: i64) -> Vec<u8> {
    let mut out = b"4:salt".to_vec();
    bytes(&mut out, SALT);
    out.extend_from_slice(format!("3:seqi{seq}e1:v").as_bytes());
    out.extend_from_slice(value);
    out
}

/// What `item` says, when `key` signed it and its sequence number is its
/// revision.
pub fn verify(item: &Item, key: &[u8; 32]) -> Option<Newest> {
    let signature = ed25519_compact::Signature::from_slice(&item.signature).ok()?;
    ed25519_compact::PublicKey::new(*key)
        .verify(signed_bytes(&item.value, item.seq), &signature)
        .ok()?;
    parse_value(&item.value).filter(|newest| i64::from(newest.revision) == item.seq)
}

/// The item of a signed torrent: what its `skhid` says (with `dht`, the
/// item's signature) and its info hash. `None` without `skhid.dht` (a
/// torrent of a release before it). Not verified here.
pub fn of_torrent(torrent: &[u8]) -> Option<Item> {
    let document = bencode::parse(torrent)?;
    let (_, info) = document.entry(b"info")?;
    let skhid = document.get(b"skhid")?;
    let newest = Newest {
        v2: digest(info),
        revision: u8::try_from(skhid.get(b"revision")?.int()?).ok()?,
        signature: skhid.get(b"signature")?.bytes()?.try_into().ok()?,
    };
    Some(Item {
        seq: i64::from(newest.revision),
        value: value(&newest),
        signature: skhid.get(b"dht").and_then(Value::bytes)?.try_into().ok()?,
    })
}

/// The signed torrent again from its `info` (the swarm's) and the item
/// that names it: `None` when that info is not the item's.
pub fn torrent(info: &[u8], newest: &Newest, item_signature: &[u8; 64]) -> Option<Vec<u8>> {
    if digest(info) != newest.v2 {
        return None;
    }
    let mut out = b"d4:info".to_vec();
    out.extend_from_slice(info);
    out.extend_from_slice(b"5:skhidd3:dht");
    bytes(&mut out, item_signature);
    out.extend_from_slice(format!("8:revisioni{}e9:signature", newest.revision).as_bytes());
    bytes(&mut out, &newest.signature);
    out.extend_from_slice(b"ee");
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signed;

    fn key() -> ed25519_compact::KeyPair {
        ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new([7; 32]))
    }

    /// A torrent signed as the release signs it, `skhid.dht` too.
    fn release(revision: u8) -> Vec<u8> {
        let torrent = crate::torrent::make(&[("game/a", b"abc")], 32 * 1024, &[]);
        let info = bencode::parse(&torrent).unwrap().entry(b"info").unwrap().1.to_vec();
        let pair = key();
        let newest = Newest {
            v2: digest(&info),
            revision,
            signature: *pair.sk.sign(signed::message(revision, &digest(&info)), None),
        };
        let item = pair.sk.sign(signed_bytes(&value(&newest), i64::from(revision)), None);
        torrent_of(&info, &newest, &item)
    }

    fn torrent_of(info: &[u8], newest: &Newest, item: &[u8; 64]) -> Vec<u8> {
        torrent(info, newest, item).unwrap()
    }

    #[test]
    fn the_item_of_a_signed_torrent_verifies_and_makes_it_again() {
        let data = release(61);
        let public = *key().pk;
        assert_eq!(signed::verify(&data, &public), Ok(61));
        let item = of_torrent(&data).unwrap();
        assert_eq!(item.seq, 61);
        let newest = verify(&item, &public).unwrap();
        assert_eq!(newest.revision, 61);
        let info = bencode::parse(&data).unwrap().entry(b"info").unwrap().1.to_vec();
        assert_eq!(torrent(&info, &newest, &item.signature).unwrap(), data);
        // Another info than the item's.
        let mut other = info.clone();
        let at = other.windows(3).position(|w| w == b"1:a").unwrap() + 2;
        other[at] = b'b';
        assert_eq!(torrent(&other, &newest, &item.signature), None);
    }

    #[test]
    fn an_item_is_taken_only_as_signed() {
        let public = *key().pk;
        let item = of_torrent(&release(61)).unwrap();
        // Another sequence number than the one signed, or than the revision.
        assert_eq!(verify(&Item { seq: 62, ..item.clone() }, &public), None);
        let other = ed25519_compact::KeyPair::from_seed(ed25519_compact::Seed::new([8; 32]));
        assert_eq!(verify(&item, &other.pk), None);
        let mut changed = item.clone();
        changed.value[10] ^= 1;
        assert_eq!(verify(&changed, &public), None);
        // A value of another form, even signed, is not taken.
        let extra = b"d1:ai1e1:i32:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1:ri61e1:s64:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaae";
        assert_eq!(parse_value(extra), None);
        let signature = key().sk.sign(signed_bytes(extra, 61), None);
        assert_eq!(verify(&Item { seq: 61, value: extra.to_vec(), signature: *signature }, &public), None);
        // A torrent without `skhid.dht` has no item.
        let plain = crate::torrent::make(&[("game/a", b"abc")], 32 * 1024, &[]);
        assert_eq!(of_torrent(&signed::tests::sign(&plain, 60, &key())), None);
    }

    #[test]
    fn what_bep_44_signs() {
        assert_eq!(signed_bytes(b"1:x", 5), b"4:salt14:skhid-newest 13:seqi5e1:v1:x".to_vec());
    }
}

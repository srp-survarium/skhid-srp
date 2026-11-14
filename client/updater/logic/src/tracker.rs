//! Asking a torrent's trackers for peers: HTTP (BEP 3, compact answers,
//! BEP 23) and UDP (BEP 15). IPv4 peers only. A tracker is a stranger too:
//! its answer is read once, within fixed limits.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::bencode::{self, Value};

/// The most of an HTTP tracker's answer read.
pub const MAX_ANSWER: usize = 64 * 1024;
/// The most peers taken from one answer.
pub const MAX_PEERS: usize = 200;
/// How often a tracker is asked again, whatever it says: at least, at most.
pub const MIN_INTERVAL_S: u64 = 60;
pub const MAX_INTERVAL_S: u64 = 3600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Peer {
    pub ip: [u8; 4],
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announce {
    pub interval_s: u64,
    pub peers: Vec<Peer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackerError {
    /// Not a tracker's answer.
    Format,
    /// The tracker's own words (at most 200 characters of them).
    Failure(String),
}

/// What a tracker is told.
#[derive(Debug, Clone, Copy)]
pub struct Ask<'a> {
    pub info_hash: &'a [u8; 20],
    pub peer_id: &'a [u8; 20],
    pub port: u16,
    pub uploaded: u64,
    pub downloaded: u64,
    pub left: u64,
    pub event: Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    None,
    Started,
    Completed,
    Stopped,
}

/// A tracker's address, by its scheme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Address {
    /// `http://` or `https://`: the whole address.
    Http(String),
    /// `udp://host:port[/...]`.
    Udp { host: String, port: u16 },
}

pub fn address(text: &str) -> Option<Address> {
    if text.starts_with("http://") || text.starts_with("https://") {
        return Some(Address::Http(text.into()));
    }
    let rest = text.strip_prefix("udp://")?;
    let authority = rest.split('/').next()?;
    let (host, port) = authority.rsplit_once(':')?;
    let port: u16 = port.parse().ok().filter(|&port| port > 0)?;
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && host
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-');
    host_ok.then(|| Address::Udp {
        host: host.into(),
        port,
    })
}

fn percent(out: &mut String, bytes: &[u8]) {
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
}

/// The address an HTTP tracker is asked at.
pub fn http_url(base: &str, ask: &Ask) -> String {
    let mut url = String::from(base);
    url.push(if base.contains('?') { '&' } else { '?' });
    url.push_str("info_hash=");
    percent(&mut url, ask.info_hash);
    url.push_str("&peer_id=");
    percent(&mut url, ask.peer_id);
    url.push_str(&format!(
        "&port={}&uploaded={}&downloaded={}&left={}&compact=1&numwant=50",
        ask.port, ask.uploaded, ask.downloaded, ask.left
    ));
    match ask.event {
        Event::None => {}
        Event::Started => url.push_str("&event=started"),
        Event::Completed => url.push_str("&event=completed"),
        Event::Stopped => url.push_str("&event=stopped"),
    }
    url
}

fn interval(seconds: i64) -> u64 {
    (seconds.max(0) as u64).clamp(MIN_INTERVAL_S, MAX_INTERVAL_S)
}

fn compact(bytes: &[u8]) -> Vec<Peer> {
    bytes
        .chunks_exact(6)
        .map(|peer| Peer {
            ip: peer[..4].try_into().expect("4 bytes"),
            port: u16::from_be_bytes([peer[4], peer[5]]),
        })
        .filter(|peer| peer.port != 0 && peer.ip != [0; 4])
        .take(MAX_PEERS)
        .collect()
}

/// An HTTP tracker's answer.
pub fn parse_http(body: &[u8]) -> Result<Announce, TrackerError> {
    if body.len() > MAX_ANSWER {
        return Err(TrackerError::Format);
    }
    let answer = bencode::parse(body).ok_or(TrackerError::Format)?;
    if let Some(reason) = answer.get(b"failure reason").and_then(Value::bytes) {
        let words = String::from_utf8_lossy(reason);
        return Err(TrackerError::Failure(words.chars().take(200).collect()));
    }
    let peers = answer
        .get(b"peers")
        .and_then(Value::bytes)
        .filter(|peers| peers.len() % 6 == 0)
        .ok_or(TrackerError::Format)?;
    Ok(Announce {
        interval_s: interval(answer.get(b"interval").and_then(Value::int).unwrap_or(0)),
        peers: compact(peers),
    })
}

/// BEP 15's magic number of a connect request.
const UDP_MAGIC: u64 = 0x41727101980;

pub fn udp_connect(transaction: u32) -> [u8; 16] {
    let mut out = [0_u8; 16];
    out[..8].copy_from_slice(&UDP_MAGIC.to_be_bytes());
    out[12..].copy_from_slice(&transaction.to_be_bytes());
    out
}

fn udp_head(answer: &[u8], action: u32, transaction: u32) -> Result<(), TrackerError> {
    if answer.len() < 8 || answer[4..8] != transaction.to_be_bytes() {
        return Err(TrackerError::Format);
    }
    let got = u32::from_be_bytes(answer[..4].try_into().expect("4 bytes"));
    if got == 3 {
        let words = String::from_utf8_lossy(&answer[8..]);
        return Err(TrackerError::Failure(words.chars().take(200).collect()));
    }
    if got != action {
        return Err(TrackerError::Format);
    }
    Ok(())
}

/// The connection id of a connect answer.
pub fn udp_connected(answer: &[u8], transaction: u32) -> Result<u64, TrackerError> {
    udp_head(answer, 0, transaction)?;
    if answer.len() != 16 {
        return Err(TrackerError::Format);
    }
    Ok(u64::from_be_bytes(answer[8..].try_into().expect("8 bytes")))
}

pub fn udp_announce(connection: u64, transaction: u32, key: u32, ask: &Ask) -> [u8; 98] {
    let mut out = [0_u8; 98];
    out[..8].copy_from_slice(&connection.to_be_bytes());
    out[8..12].copy_from_slice(&1_u32.to_be_bytes());
    out[12..16].copy_from_slice(&transaction.to_be_bytes());
    out[16..36].copy_from_slice(ask.info_hash);
    out[36..56].copy_from_slice(ask.peer_id);
    out[56..64].copy_from_slice(&ask.downloaded.to_be_bytes());
    out[64..72].copy_from_slice(&ask.left.to_be_bytes());
    out[72..80].copy_from_slice(&ask.uploaded.to_be_bytes());
    let event: u32 = match ask.event {
        Event::None => 0,
        Event::Completed => 1,
        Event::Started => 2,
        Event::Stopped => 3,
    };
    out[80..84].copy_from_slice(&event.to_be_bytes());
    // IP 0 (the sender's), the key, 50 peers wanted, the port.
    out[88..92].copy_from_slice(&key.to_be_bytes());
    out[92..96].copy_from_slice(&50_u32.to_be_bytes());
    out[96..98].copy_from_slice(&ask.port.to_be_bytes());
    out
}

pub fn udp_announced(answer: &[u8], transaction: u32) -> Result<Announce, TrackerError> {
    udp_head(answer, 1, transaction)?;
    if answer.len() < 20 || !(answer.len() - 20).is_multiple_of(6) {
        return Err(TrackerError::Format);
    }
    let seconds = u32::from_be_bytes(answer[8..12].try_into().expect("4 bytes"));
    Ok(Announce {
        interval_s: interval(i64::from(seconds)),
        peers: compact(&answer[20..]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASK: Ask = Ask {
        info_hash: &[0xAB; 20],
        peer_id: b"-SK0001-abcdefghijkl",
        port: 6881,
        uploaded: 1,
        downloaded: 2,
        left: 3,
        event: Event::Started,
    };

    #[test]
    fn http_trackers() {
        let url = http_url("http://t.example/announce", &ASK);
        assert!(url.starts_with("http://t.example/announce?info_hash=%AB%AB"));
        assert!(url.contains("&peer_id=-SK0001-abcdefghijkl&port=6881&uploaded=1&downloaded=2&left=3&compact=1"));
        assert!(url.ends_with("&event=started"));
        assert!(http_url("http://t/a?k=1", &ASK).starts_with("http://t/a?k=1&info_hash="));
        let answer = parse_http(b"d8:intervali1800e5:peers12:\x7f\x00\x00\x01\x1a\xe1\x00\x00\x00\x00\x00\x01e").unwrap();
        assert_eq!(answer.interval_s, 1800);
        assert_eq!(answer.peers, [Peer { ip: [127, 0, 0, 1], port: 6881 }]);
        assert_eq!(parse_http(b"d8:intervali5e5:peers0:e").unwrap().interval_s, MIN_INTERVAL_S);
        assert_eq!(
            parse_http(b"d14:failure reason4:nopee"),
            Err(TrackerError::Failure("nope".into()))
        );
        assert_eq!(parse_http(b"d5:peers5:abcdee"), Err(TrackerError::Format));
        assert_eq!(parse_http(b"<html>"), Err(TrackerError::Format));
    }

    #[test]
    fn udp_trackers() {
        assert_eq!(
            address("udp://tracker.example:1337/announce"),
            Some(Address::Udp { host: "tracker.example".into(), port: 1337 })
        );
        assert_eq!(address("udp://x"), None);
        assert_eq!(address("udp://x:0"), None);
        let connect = udp_connect(7);
        assert_eq!(&connect[..8], &[0, 0, 4, 0x17, 0x27, 0x10, 0x19, 0x80]);
        let mut answer = alloc::vec![0, 0, 0, 0, 0, 0, 0, 7];
        answer.extend_from_slice(&42_u64.to_be_bytes());
        assert_eq!(udp_connected(&answer, 7), Ok(42));
        assert_eq!(udp_connected(&answer, 8), Err(TrackerError::Format));
        let request = udp_announce(42, 9, 5, &ASK);
        assert_eq!(&request[96..], &6881_u16.to_be_bytes());
        let mut answer = alloc::vec![0, 0, 0, 1, 0, 0, 0, 9, 0, 0, 0, 120, 0, 0, 0, 0, 0, 0, 0, 1];
        answer.extend_from_slice(&[10, 0, 0, 1, 0x1A, 0xE1]);
        let announced = udp_announced(&answer, 9).unwrap();
        assert_eq!(announced.interval_s, 120);
        assert_eq!(announced.peers, [Peer { ip: [10, 0, 0, 1], port: 6881 }]);
        let mut error = alloc::vec![0, 0, 0, 3, 0, 0, 0, 9];
        error.extend_from_slice(b"bad");
        assert_eq!(udp_announced(&error, 9), Err(TrackerError::Failure("bad".into())));
        answer.push(0);
        assert_eq!(udp_announced(&answer, 9), Err(TrackerError::Format));
    }
}

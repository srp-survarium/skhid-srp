//! The raw datagram log (it stands in for a packet
//! capture). Off unless `SRP_PACKET_LOG=<dir>` is set. One file per
//! connection; every datagram in and out with its time, direction, peer and
//! hex bytes, then one line per decoded message (id, x, part, length, and
//! `at`: where its payload starts in the datagram, which
//! `scripts/packet-coverage/coverage.py` reads).
//!
//! ```text
//! 2026-10-12T03:04:05.678Z IN  127.0.0.1:51234 len=9 00 FF 00 D8 ** ** ** ** **
//!     seq=0 rseq=255 acks=1000000/7 single
//!     msg id=D8 len=5 at=4 redacted
//! ```
//!
//! A caller that carries secrets in a payload gives the log a redaction
//! hook: a message it marks keeps its header bytes but its payload is
//! written as `**`. A datagram that does not parse is logged whole (the
//! hook has no message to judge), with the reason.

use std::fs::{self, File};
use std::io::{self, LineWriter, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::table::{Direction, Table};
use crate::wire::{self, ParseError};

/// The environment variable that turns the log on: the directory to write.
pub const ENV: &str = "SRP_PACKET_LOG";

/// Says whether a message's payload must stay out of the log: the
/// direction, the id and the payload (a part's data for a split message).
pub type Redact = Box<dyn Fn(Direction, u8, &[u8]) -> bool + Send>;

/// The log directory; makes one [`ConnectionLog`] per connection.
#[derive(Clone, Debug)]
pub struct PacketLog {
    directory: PathBuf,
}

impl PacketLog {
    /// The log named by `SRP_PACKET_LOG`, or `None` when it is unset or
    /// empty (the default: nothing is logged).
    pub fn from_env() -> Option<Self> {
        let directory = std::env::var_os(ENV)?;
        if directory.is_empty() {
            return None;
        }
        match Self::new(Path::new(&directory)) {
            Ok(log) => Some(log),
            Err(error) => {
                log::warn!("{ENV}: cannot use {directory:?}: {error}");
                None
            }
        }
    }

    /// A log in `directory`, created if missing.
    pub fn new(directory: &Path) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        Ok(Self {
            directory: directory.to_path_buf(),
        })
    }

    /// A new file for one connection:
    /// `<kind>-<ip>_<port>-<unix ms>.log`, the kind being `match` or
    /// `lobby`.
    pub fn connection(&self, table: Table, peer: SocketAddr) -> io::Result<ConnectionLog> {
        let kind = match table {
            Table::Match => "match",
            Table::Lobby => "lobby",
        };
        let address: String = peer
            .ip()
            .to_string()
            .chars()
            .map(|c| if c == ':' { '-' } else { c })
            .collect();
        let name = format!("{kind}-{address}_{}-{}.log", peer.port(), unix_ms());
        let path = self.directory.join(name);
        let file = File::create(&path)?;
        Ok(ConnectionLog {
            path,
            file: LineWriter::new(file),
            peer: peer.to_string(),
            table,
            redact: None,
            failed: false,
        })
    }
}

/// The log file of one connection. A [`crate::Transport`] given one writes
/// every datagram it receives and returns from `poll`.
pub struct ConnectionLog {
    path: PathBuf,
    file: LineWriter<File>,
    peer: String,
    table: Table,
    redact: Option<Redact>,
    failed: bool,
}

impl ConnectionLog {
    /// Set the redaction hook.
    pub fn with_redaction(mut self, redact: Redact) -> Self {
        self.redact = Some(redact);
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Log one datagram travelling in `direction`.
    pub fn datagram(&mut self, direction: Direction, datagram: &[u8], mask_bytes: usize) {
        let parsed = wire::parse(self.table, direction, datagram, mask_bytes);
        let text = render(
            &timestamp(unix_ms()),
            direction,
            &self.peer,
            datagram,
            &parsed,
            self.redact.as_ref(),
        );
        self.write(&text);
    }

    /// Log a line about the connection (its start, its end).
    pub fn note(&mut self, note: &str) {
        let text = format!("{} --  {} {note}\n", timestamp(unix_ms()), self.peer);
        self.write(&text);
    }

    fn write(&mut self, text: &str) {
        if self.failed {
            return;
        }
        if let Err(error) = self.file.write_all(text.as_bytes()) {
            log::warn!("The packet log {:?} stopped: {error}", self.path);
            self.failed = true;
        }
    }
}

/// The text for one datagram: the raw line and the decoded lines.
fn render(
    time: &str,
    direction: Direction,
    peer: &str,
    datagram: &[u8],
    parsed: &Result<wire::Datagram<'_>, ParseError>,
    redact: Option<&Redact>,
) -> String {
    let arrow = match direction {
        Direction::ToServer => "IN ",
        Direction::ToClient => "OUT",
    };
    let mut hidden = vec![false; datagram.len()];
    let mut lines = String::new();
    match parsed {
        Ok(parsed) => {
            lines.push_str(&format!(
                "    seq={} rseq={} acks={:b}/{} {}\n",
                parsed.seq,
                parsed.rseq,
                parsed.acks.bits >> (128 - parsed.acks.count),
                parsed.acks.count,
                if parsed.acks.multi { "multi" } else { "single" },
            ));
            for message in &parsed.messages {
                let mut line = format!("    msg id={:02X}", message.id);
                if let Some(x) = message.x {
                    line.push_str(&format!(" x={x}"));
                }
                if let Some(part) = message.part {
                    line.push_str(&format!(" part={}", part.index));
                    if let Some(count) = part.count {
                        line.push_str(&format!("/{count}"));
                    }
                }
                if message.low_level {
                    line.push_str(" low-level");
                } else {
                    line.push_str(&format!(" len={} at={}", message.payload.len(), message.offset));
                    if redact.is_some_and(|redact| redact(direction, message.id, message.payload)) {
                        let end = message.offset + message.payload.len();
                        hidden[message.offset..end].fill(true);
                        line.push_str(" redacted");
                    }
                }
                lines.push_str(&line);
                lines.push('\n');
            }
        }
        Err(error) => lines.push_str(&format!("    not parsed: {error:?}\n")),
    }
    let mut text = format!("{time} {arrow} {peer} len={}", datagram.len());
    for (byte, hidden) in datagram.iter().zip(&hidden) {
        if *hidden {
            text.push_str(" **");
        } else {
            text.push_str(&format!(" {byte:02X}"));
        }
    }
    text.push('\n');
    text.push_str(&lines);
    text
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}

/// `2026-10-12T03:04:05.678Z` from Unix milliseconds (UTC).
fn timestamp(unix_ms: u64) -> String {
    let ms = unix_ms % 1000;
    let seconds = unix_ms / 1000;
    let (second, minute, hour) = (seconds % 60, seconds / 60 % 60, seconds / 3600 % 24);
    // Days to a civil date (proleptic Gregorian), counting from 0000-03-01.
    let days = seconds / 86400 + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{ms:03}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_timestamp_is_utc() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(timestamp(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(timestamp(1_791_774_245_678), "2026-10-12T03:04:05.678Z");
    }

    #[test]
    fn a_datagram_is_logged_raw_and_decoded() {
        let datagram = [0x00, 0xFF, 0x00, 0xD8, 0x78, 0x56, 0x34, 0x12, 0x00];
        let parsed = wire::parse(Table::Match, Direction::ToServer, &datagram, 0);
        let text = render(
            "T",
            Direction::ToServer,
            "1.2.3.4:5",
            &datagram,
            &parsed,
            None,
        );
        assert_eq!(
            text,
            "T IN  1.2.3.4:5 len=9 00 FF 00 D8 78 56 34 12 00\n    \
             seq=0 rseq=255 acks=1000000/7 single\n    msg id=D8 len=5 at=4\n"
        );
    }

    #[test]
    fn a_redacted_payload_stays_out_of_the_log() {
        let redact: Redact =
            Box::new(|direction, id, _| direction == Direction::ToServer && id == 0x2E);
        // 2E (sign-in, x = 0) and 2F (x = 1) in one multi datagram.
        let datagram = [
            0x00, 0xFF, 0x01, 0x2E, 0x00, 0x05, 0x44, 0x33, 0x22, 0x11, 0x01, 0x2F, 0x01, 0x02,
            0xAA, 0xBB,
        ];
        let parsed = wire::parse(Table::Lobby, Direction::ToServer, &datagram, 0);
        let text = render(
            "T",
            Direction::ToServer,
            "p",
            &datagram,
            &parsed,
            Some(&redact),
        );
        assert_eq!(
            text,
            "T IN  p len=16 00 FF 01 2E 00 05 ** ** ** ** ** 2F 01 02 AA BB\n    \
             seq=0 rseq=255 acks=1000000/7 multi\n    \
             msg id=2E x=0 len=5 at=6 redacted\n    msg id=2F x=1 len=2 at=14\n"
        );
        assert!(!text.contains("44 33"));
    }

    #[test]
    fn a_datagram_that_does_not_parse_is_logged_with_the_reason() {
        let datagram = [0x05, 0x02, 0x00];
        let parsed = wire::parse(Table::Match, Direction::ToClient, &datagram, 0);
        let text = render("T", Direction::ToClient, "p", &datagram, &parsed, None);
        assert_eq!(
            text,
            "T OUT p len=3 05 02 00\n    not parsed: EmptyMessageArea\n"
        );
    }

    #[test]
    fn each_connection_gets_its_own_file() {
        let directory = std::env::temp_dir().join(format!(
            "srp-packet-log-test-{}-{}",
            std::process::id(),
            unix_ms()
        ));
        let log = PacketLog::new(&directory).unwrap();
        let peer: SocketAddr = "127.0.0.1:50000".parse().unwrap();
        let mut connection = log.connection(Table::Match, peer).unwrap();
        connection.note("connected");
        connection.datagram(Direction::ToClient, &[0, 0, 0, 0xE3, 0xD4, 4, 0xD4, 4], 0);
        let path = connection.path().to_path_buf();
        drop(connection);
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("match-127.0.0.1_50000-") && name.ends_with(".log"));
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(" --  127.0.0.1:50000 connected\n"));
        assert!(text.contains(" OUT 127.0.0.1:50000 len=8 00 00 00 E3 D4 04 D4 04\n"));
        assert!(text.contains("    msg id=E3 len=4 at="));
        fs::remove_dir_all(&directory).unwrap();
    }
}

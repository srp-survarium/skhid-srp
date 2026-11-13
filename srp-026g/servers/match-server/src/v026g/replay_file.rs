//! Save's file, `.srpreplay` (`docs/notes/replay_watch.md` section
//! 2.1): a small header and the `.sur` a replay session plays
//! (`game::playback::sur_of`), with the history's events of its timeline.
//! Not the `.srprec`: that holds every datagram in and out, sessions
//! among them; nothing here names an account, a session, an address or a
//! path of the server.
//!
//! ```text
//! "SRPRPL01"          8 bytes
//! u32 LE header_len   <= MAX_HEADER
//! header              UTF-8 lines "key value\n", no other control byte
//! u32 LE events_len   <= MAX_EVENTS, a multiple of EVENT_LEN
//! events              replay_control::Event, EVENT_LEN bytes each
//! u32 LE sur_len
//! sur                 the .sur (sur.rs), its chunk 4 empty (length 0, no mode config)
//! ```
//!
//! The whole file is at most [`MAX_FILE`]. The header is read alone (the
//! first 12 + `header_len` bytes) to name and list a file: the updater
//! names a saved file by `made`, `map_slug` and `match`. Its keys, one line
//! each in this order (a reader skips keys it does not know):
//! `format 1`, `match <8 lower hex>`, `made <unix s, the recording's
//! start>`, `length <s>`, `map <maps_dict id>`, `map_slug <[A-Za-z0-9-],
//! the English name>`, `mode <the lobby's mode number>`, `score <a> <b>`
//! or `score - -`, `revisions <r0,r1,...>` (by place, `-` unknown), `epoch
//! <rev>` or `epoch -` (the simulation epoch of the recording:
//! `session::v026g::replays::simulation_epoch`, the last change of what a
//! match computes at or before its oldest game), then
//! `place <n> <team> <kills> <deaths> <name>` for each place of the
//! roster. The winner is the higher score.

use replay_control::{EVENT_LEN, Event};

pub const MAGIC: &[u8; 8] = b"SRPRPL01";
pub const FORMAT: u32 = 1;
pub const MAX_HEADER: usize = 8192;
pub const MAX_EVENTS: usize = 65_536;
/// The largest file: a `.sur` the reader takes is a few MB at most
/// (`sur::MAX_KEY_FRAMES` of `sur::MAX_KEY_FRAME_DATA`, the rest LZMA2).
pub const MAX_FILE: usize = 12 << 20;
/// A slug's characters at most.
pub const MAX_SLUG: usize = 32;

/// One place of the roster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub team: u8,
    pub kills: u16,
    pub deaths: u16,
    /// As the 0xE4 options name it; control characters made spaces.
    pub name: String,
}

/// What the header says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub match_id: u32,
    /// Unix seconds, UTC.
    pub made: u64,
    /// Seconds.
    pub length: u32,
    pub map: u8,
    pub map_slug: String,
    pub mode: u8,
    pub score: Option<[u16; 2]>,
    pub revisions: Vec<Option<u8>>,
    /// The recording's simulation epoch (`session::v026g::replays::
    /// simulation_epoch`): with the game's own list of simulation changes,
    /// what an offline "recorded before a game update" is decided by.
    pub epoch: Option<u8>,
    pub places: Vec<Place>,
}

/// A whole file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayFile {
    pub header: Header,
    pub events: Vec<Event>,
    pub sur: Vec<u8>,
}

/// `[A-Za-z0-9-]` of `name`, a `-` for each run of anything else, at most
/// [`MAX_SLUG`]; `map<id>` when nothing is left.
pub fn slug(name: &str, map: u8) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    out.truncate(MAX_SLUG);
    let out = out.trim_end_matches('-');
    if out.is_empty() {
        format!("map{map}")
    } else {
        out.to_owned()
    }
}

impl Header {
    fn text(&self) -> String {
        let name = |name: &str| -> String {
            name.chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect()
        };
        let mut lines = vec![
            format!("format {FORMAT}"),
            format!("match {:08x}", self.match_id),
            format!("made {}", self.made),
            format!("length {}", self.length),
            format!("map {}", self.map),
            format!("map_slug {}", slug(&self.map_slug, self.map)),
            format!("mode {}", self.mode),
            match self.score {
                Some([a, b]) => format!("score {a} {b}"),
                None => "score - -".to_owned(),
            },
            format!(
                "revisions {}",
                self.revisions
                    .iter()
                    .map(|revision| revision.map_or("-".to_owned(), |r| r.to_string()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            match self.epoch {
                Some(epoch) => format!("epoch {epoch}"),
                None => "epoch -".to_owned(),
            },
        ];
        for (n, place) in self.places.iter().enumerate() {
            lines.push(format!(
                "place {n} {} {} {} {}",
                place.team,
                place.kills,
                place.deaths,
                name(&place.name)
            ));
        }
        lines.iter().map(|line| format!("{line}\n")).collect()
    }

    /// The header's text, untrusted: every line `key value`, the keys
    /// this format has, unknown keys skipped.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.bytes().any(|b| (b < 0x20 && b != b'\n') || b == 0x7F) {
            return Err("a control byte in the header".into());
        }
        let number = |key: &str, value: &str| -> Result<u64, String> {
            value
                .parse::<u64>()
                .map_err(|_| format!("`{key}` is `{value}`"))
        };
        let small = |key: &str, value: &str, max: u64| -> Result<u64, String> {
            let n = number(key, value)?;
            if n > max {
                return Err(format!("`{key}` {value} is over {max}"));
            }
            Ok(n)
        };
        let mut format = None;
        let mut match_id = None;
        let (mut made, mut length, mut map, mut map_slug, mut mode) = (None, None, None, None, None);
        let mut score = None;
        let mut revisions = Vec::new();
        let mut epoch = None;
        let mut places = Vec::new();
        for line in text.lines() {
            let (key, value) = line.split_once(' ').ok_or_else(|| format!("line `{line}`"))?;
            match key {
                "format" => format = Some(number(key, value)?),
                "match" => {
                    let hex = value.len() == 8
                        && value.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'));
                    let id = u32::from_str_radix(value, 16).ok().filter(|_| hex);
                    match_id = Some(id.ok_or_else(|| format!("`match` is `{value}`"))?);
                }
                "made" => made = Some(number(key, value)?),
                "length" => length = Some(small(key, value, u64::from(u32::MAX))? as u32),
                "map" => map = Some(small(key, value, 255)? as u8),
                "mode" => mode = Some(small(key, value, 255)? as u8),
                "map_slug" => {
                    let fine = !value.is_empty()
                        && value.len() <= MAX_SLUG
                        && value.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-');
                    map_slug = Some(fine.then(|| value.to_owned()).ok_or("`map_slug`")?);
                }
                "score" => {
                    score = Some(match value.split_once(' ') {
                        Some(("-", "-")) => None,
                        Some((a, b)) => Some([
                            small(key, a, 0xFFFF)? as u16,
                            small(key, b, 0xFFFF)? as u16,
                        ]),
                        None => return Err("`score`".into()),
                    })
                }
                "revisions" => {
                    revisions = value
                        .split(',')
                        .map(|r| match r {
                            "-" => Ok(None),
                            r => small(key, r, 255).map(|r| Some(r as u8)),
                        })
                        .collect::<Result<_, _>>()?;
                }
                "epoch" => {
                    epoch = Some(match value {
                        "-" => None,
                        rev => Some(small(key, rev, 255)? as u8),
                    });
                }
                "place" => {
                    let mut parts = value.splitn(5, ' ');
                    let mut next = |max: u64| {
                        parts
                            .next()
                            .ok_or_else(|| "a short `place`".to_owned())
                            .and_then(|part| small(key, part, max))
                    };
                    let (n, team, kills, deaths) = (next(19)?, next(1)?, next(0xFFFF)?, next(0xFFFF)?);
                    if n as usize != places.len() {
                        return Err(format!("place {n} out of order"));
                    }
                    places.push(Place {
                        team: team as u8,
                        kills: kills as u16,
                        deaths: deaths as u16,
                        name: parts.next().unwrap_or("").to_owned(),
                    });
                }
                _ => {}
            }
        }
        if format != Some(u64::from(FORMAT)) {
            return Err(format!("format {format:?}"));
        }
        let missing = |key: &str| format!("no `{key}`");
        Ok(Self {
            match_id: match_id.ok_or_else(|| missing("match"))?,
            made: made.ok_or_else(|| missing("made"))?,
            length: length.ok_or_else(|| missing("length"))?,
            map: map.ok_or_else(|| missing("map"))?,
            map_slug: map_slug.ok_or_else(|| missing("map_slug"))?,
            mode: mode.ok_or_else(|| missing("mode"))?,
            score: score.ok_or_else(|| missing("score"))?,
            revisions,
            epoch: epoch.ok_or_else(|| missing("epoch"))?,
            places,
        })
    }
}

fn put_part(out: &mut Vec<u8>, part: &[u8]) {
    out.extend_from_slice(&(part.len() as u32).to_le_bytes());
    out.extend_from_slice(part);
}

/// The next `u32 LE` length and that many bytes, the length at most `max`.
fn take_part<'a>(bytes: &mut &'a [u8], max: usize, what: &str) -> Result<&'a [u8], String> {
    let (len, rest) = bytes
        .split_first_chunk::<4>()
        .ok_or_else(|| format!("no {what} length"))?;
    let len = u32::from_le_bytes(*len) as usize;
    if len > max {
        return Err(format!("the {what} has {len} bytes, over {max}"));
    }
    if rest.len() < len {
        return Err(format!("the {what} is cut short"));
    }
    let (part, rest) = rest.split_at(len);
    *bytes = rest;
    Ok(part)
}

impl ReplayFile {
    /// The file's bytes; an error when a part is over its cap.
    pub fn write(&self) -> Result<Vec<u8>, String> {
        let header = self.header.text();
        if header.len() > MAX_HEADER {
            return Err(format!("the header has {} bytes", header.len()));
        }
        let mut events = vec![0u8; self.events.len() * EVENT_LEN];
        for (event, room) in self.events.iter().zip(events.chunks_exact_mut(EVENT_LEN)) {
            event.encode(room);
        }
        if events.len() > MAX_EVENTS {
            return Err(format!("{} events", self.events.len()));
        }
        let mut out = Vec::with_capacity(MAGIC.len() + 12 + header.len() + events.len() + self.sur.len());
        out.extend_from_slice(MAGIC);
        put_part(&mut out, header.as_bytes());
        put_part(&mut out, &events);
        put_part(&mut out, &self.sur);
        if out.len() > MAX_FILE {
            return Err(format!("the file has {} bytes, over {MAX_FILE}", out.len()));
        }
        Ok(out)
    }

    /// A file, untrusted: each length within its cap before its part is
    /// read, the header parsed, every event of a known kind, nothing after
    /// the `.sur`. The `.sur` itself is not read here (`sur::read`).
    pub fn read(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_FILE {
            return Err(format!("{} bytes, over {MAX_FILE}", bytes.len()));
        }
        let mut rest = bytes.strip_prefix(MAGIC).ok_or("not a .srpreplay")?;
        let header = take_part(&mut rest, MAX_HEADER, "header")?;
        let header = std::str::from_utf8(header).map_err(|_| "the header is not UTF-8")?;
        let header = Header::parse(header)?;
        let events = take_part(&mut rest, MAX_EVENTS, "events")?;
        if events.len() % EVENT_LEN != 0 {
            return Err("the events are cut".into());
        }
        let events = events
            .chunks_exact(EVENT_LEN)
            .map(|bytes| Event::parse(bytes).ok_or("an event of no known kind"))
            .collect::<Result<_, _>>()?;
        let sur = take_part(&mut rest, MAX_FILE, ".sur")?.to_vec();
        if !rest.is_empty() {
            return Err("bytes after the .sur".into());
        }
        Ok(Self {
            header,
            events,
            sur,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use replay_control::{EventKind, MatchMs};

    fn file() -> ReplayFile {
        ReplayFile {
            header: Header {
                match_id: 0x1CE4_5CBE,
                made: 1_791_945_296,
                length: 754,
                map: 12,
                map_slug: "Rudnya".into(),
                mode: 4,
                score: Some([87, 100]),
                revisions: vec![Some(15), None, Some(16)],
                epoch: Some(15),
                places: vec![
                    Place {
                        team: 0,
                        kills: 4,
                        deaths: 7,
                        name: "[TAG] Stalker one".into(),
                    },
                    Place {
                        team: 1,
                        kills: 0,
                        deaths: 0,
                        name: String::new(),
                    },
                    Place {
                        team: 1,
                        kills: 11,
                        deaths: 2,
                        name: "Ïван".into(),
                    },
                ],
            },
            events: vec![
                Event {
                    time: MatchMs(65_000),
                    kind: EventKind::Kill,
                    actor: Some(2),
                    victim: Some(0),
                },
                Event {
                    time: MatchMs(70_000),
                    kind: EventKind::Objective,
                    actor: Some(0),
                    victim: None,
                },
            ],
            sur: b"the .sur bytes".to_vec(),
        }
    }

    #[test]
    fn a_file_reads_back_and_its_header_is_the_lines_a_lister_reads() {
        let file = file();
        let bytes = file.write().unwrap();
        assert_eq!(ReplayFile::read(&bytes).unwrap(), file);
        let len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let text = std::str::from_utf8(&bytes[12..12 + len]).unwrap();
        assert!(text.starts_with("format 1\nmatch 1ce45cbe\nmade 1791945296\n"), "{text}");
        assert!(text.contains("\nmap_slug Rudnya\nmode 4\nscore 87 100\nrevisions 15,-,16\n"));
        assert!(text.ends_with("place 1 1 0 0 \nplace 2 1 11 2 Ïван\n"), "{text}");
        // A score not known; a name with a line end cannot add a line.
        let mut other = file.clone();
        other.header.score = None;
        other.header.places[0].name = "a\nmatch 00000000\r".into();
        let read = ReplayFile::read(&other.write().unwrap()).unwrap();
        assert_eq!(read.header.score, None);
        assert_eq!(read.header.match_id, 0x1CE4_5CBE);
        assert_eq!(read.header.places[0].name, "a match 00000000 ");
    }

    #[test]
    fn a_slug_is_letters_digits_and_dashes() {
        assert_eq!(slug("Rudnya", 1), "Rudnya");
        assert_eq!(slug("Vostok Radar Station", 1), "Vostok-Radar-Station");
        assert_eq!(slug("Лаборатория \"Вектор\"", 7), "map7");
        assert_eq!(slug("  ../..\\x  ", 7), "x");
        assert_eq!(slug(&"abc ".repeat(20), 7).len(), MAX_SLUG - 1);
    }

    #[test]
    fn a_damaged_file_is_refused_and_never_panics() {
        let bytes = file().write().unwrap();
        for cut in 0..bytes.len() {
            assert!(ReplayFile::read(&bytes[..cut]).is_err(), "cut at {cut}");
        }
        let with = |at: usize, value: &[u8]| {
            let mut bytes = bytes.clone();
            bytes[at..at + value.len()].copy_from_slice(value);
            ReplayFile::read(&bytes)
        };
        assert!(with(0, b"SRPRPL02").is_err());
        assert!(with(8, &(MAX_HEADER as u32 + 1).to_le_bytes()).is_err());
        assert!(with(8, &u32::MAX.to_le_bytes()).is_err());
        let events_at = 12 + u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        assert!(with(events_at, &13u32.to_le_bytes()).is_err());
        // An event of no known kind.
        assert!(with(events_at + 4 + 4, &[9]).is_err());
        // A control byte in the header, a header of another format.
        assert!(with(12, b"\x01").is_err());
        assert!(with(12, b"format 2").is_err());
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(ReplayFile::read(&longer).is_err());
        // Every byte flipped: an error or a file, never a panic.
        for at in 0..bytes.len() {
            let mut bytes = bytes.clone();
            bytes[at] ^= 0xFF;
            let _ = ReplayFile::read(&bytes);
        }
        // Over the caps on write.
        let mut big = file();
        big.sur = vec![0; MAX_FILE];
        assert!(big.write().is_err());
    }
}

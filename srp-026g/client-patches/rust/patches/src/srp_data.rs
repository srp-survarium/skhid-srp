//! Patch 94: srp's tables for the lobby movie. srp's own configs,
//! `gameplay/srp/<table>.options` (made by `scripts/content-edit/<table>.tsv`;
//! srp's server reads the same files from its game data), are each asked
//! once the game has loaded its configs (`configs.loaded`), read when they come
//! (`resources.config_done`) into their keys and values as text, and given to
//! the lobby movie, `root.srp_data(table, key, value, ...)` (`SrpData.as`),
//! at a frame of the lobby drawn (`lobby_camera.view`): once a movie, and
//! again at the next frame while it refuses them (no root yet).
//!
//! A key is the config's path (`pvb/sizes[1]`), a value its number or text.
//! A table that cannot be read (a missing file, a value of a kind not read
//! here, more than [`TEXT_BYTES`]) is a line in `srp_patches.log`, and the
//! movie is never told: what it would read is missing, loudly.
//!
//! Nothing a match computes: the menus only.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::config::{ConfigError, ConfigValue};
use crate::vostok::lobby_camera::LobbyCamera;
use crate::vostok::lobby_menu::DrawnLobby;
use crate::vostok::log::{Piece, line};
use crate::vostok::resources::{self, ConfigQuery, ConfigTag};
use crate::vostok::{Context, Flow, GameCell, MAX_STRINGS, NoExit};

/// srp's tables: the name the movie knows each by, and its config.
const TABLES: [(&CStr, &CStr); 1] = [(c"rooms", c"resources/gameplay/srp/rooms.options")];

/// The most bytes of a table's keys and values, each with its zero.
pub const TEXT_BYTES: usize = 4096;
/// The longest key path.
const KEY_BYTES: usize = 128;

/// A table's keys and values, each a C string, one after another.
#[derive(Clone, Copy)]
pub struct Text {
    bytes: [u8; TEXT_BYTES],
    len: usize,
}

impl Text {
    const fn new() -> Text {
        Text { bytes: [0; TEXT_BYTES], len: 0 }
    }

    /// One string more, from its parts; false when it does not fit.
    fn push(&mut self, parts: &[&[u8]]) -> bool {
        let size = parts.iter().map(|part| part.len()).sum::<usize>() + 1;
        if self.len + size > TEXT_BYTES {
            return false;
        }
        for part in parts {
            self.bytes[self.len..self.len + part.len()].copy_from_slice(part);
            self.len += part.len();
        }
        self.bytes[self.len] = 0;
        self.len += 1;
        true
    }

    /// Its strings: a key, its value, a key, ...
    fn strings(&self) -> impl Iterator<Item = &CStr> {
        self.bytes[..self.len]
            .split_inclusive(|&b| b == 0)
            .filter_map(|text| CStr::from_bytes_with_nul(text).ok())
    }
}

/// Why a table was not read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Unread {
    Missing,
    Config(ConfigError),
    /// A value of a kind not read here (a boolean, a vector).
    Kind,
    Full,
}

impl From<ConfigError> for Unread {
    fn from(error: ConfigError) -> Unread {
        Unread::Config(error)
    }
}

/// A table of [`TABLES`].
#[derive(Clone, Copy)]
enum Table {
    Asked,
    Read(Text),
    Unread,
}

/// The tables, from the game's start.
#[derive(Clone, Copy)]
enum Data {
    /// The game has not loaded its configs.
    Unasked,
    /// Asked; `told`, the lobby movie that has every table.
    Asked { tables: [Table; TABLES.len()], told: Option<usize> },
}

static DATA: GameCell<Data> = GameCell::new(Data::Unasked);

/// game::on_configs_loaded once the configs and the dictionary are loaded:
/// each table asked, once.
pub fn on_configs(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Data::Unasked = DATA.get(thread) {
        DATA.set(thread, Data::Asked { tables: [Table::Asked; TABLES.len()], told: None });
        for (index, (_, path)) in TABLES.iter().enumerate() {
            resources::query_config(thread, path, ConfigTag(index as u32), crate::vostok::srp_data_on_config_entry());
        }
    }
    Flow::Continue
}

/// A table's query done (on the game's thread): its keys and values kept.
pub fn on_config(query: ConfigQuery<'_>, tag: ConfigTag) {
    let index = tag.0 as usize;
    let Some((name, _)) = TABLES.get(index) else { return };
    let mut text = Text::new();
    let read = match query.root() {
        None => Err(Unread::Missing),
        Some(root) => root.map_err(Unread::from).and_then(|root| flatten(&mut text, &mut Key::new(), root)),
    };
    if let Err(why) = read {
        let why = match why {
            Unread::Missing => "no such config",
            Unread::Config(_) => "a config not as read",
            Unread::Kind => "a value of a kind not read",
            Unread::Full => "more than its text holds",
        };
        line(&[Piece::Text("srp_data: "), Piece::Text(name.to_str().unwrap_or("?")), Piece::Text(": "),
               Piece::Text(why)]);
    }
    DATA.update(query.thread(), |data| match data {
        Data::Asked { mut tables, told } => {
            tables[index] = if read.is_ok() { Table::Read(text) } else { Table::Unread };
            Data::Asked { tables, told }
        }
        Data::Unasked => data,
    });
}

/// A frame of the lobby drawn: the movie given every table, once.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Data::Asked { tables, told } = DATA.get(thread) else { return Flow::Continue };
    let Some((camera, _)) = LobbyCamera::at(ctx) else { return Flow::Continue };
    let Some(movie) = DrawnLobby::of(&camera).and_then(|lobby| lobby.movie()) else { return Flow::Continue };
    if told == Some(movie.id()) {
        return Flow::Continue;
    }
    let read = |table: &Table| match table {
        Table::Read(text) => Some(*text),
        Table::Asked | Table::Unread => None,
    };
    if tables.iter().all(|table| read(table).is_some())
        && TABLES.iter().zip(&tables).all(|((name, _), table)| {
            read(table).is_some_and(|text| give(name, &text, |strings| movie.invoke_strings(c"root.srp_data", strings)))
        })
    {
        DATA.set(thread, Data::Asked { tables, told: Some(movie.id()) });
    }
    Flow::Continue
}

/// Hands `invoke` the table in parts: its name, then as many keys and values
/// as one call takes; whether every part was taken.
fn give(name: &CStr, text: &Text, mut invoke: impl FnMut(&[&CStr]) -> bool) -> bool {
    const PAIRS: usize = (MAX_STRINGS - 1) / 2;
    let mut part: [&CStr; 1 + 2 * PAIRS] = [name; 1 + 2 * PAIRS];
    let (mut count, mut sent) = (1, false);
    for string in text.strings() {
        part[count] = string;
        count += 1;
        if count == part.len() {
            if !invoke(&part[..count]) {
                return false;
            }
            (count, sent) = (1, true);
        }
    }
    // the last part, or the name alone for an empty table
    (count == 1 && sent) || invoke(&part[..count])
}

/// A key path being built: `pvb/sizes[1]`.
struct Key {
    bytes: [u8; KEY_BYTES],
    len: usize,
}

impl Key {
    const fn new() -> Key {
        Key { bytes: [0; KEY_BYTES], len: 0 }
    }

    fn push(&mut self, parts: &[&[u8]]) -> Result<usize, Unread> {
        let at = self.len;
        for part in parts {
            let end = self.len + part.len();
            self.bytes.get_mut(self.len..end).ok_or(Unread::Full)?.copy_from_slice(part);
            self.len = end;
        }
        Ok(at)
    }

    fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// Every value under `value` into `text`, each after its key.
fn flatten(text: &mut Text, key: &mut Key, value: ConfigValue<'_>) -> Result<(), Unread> {
    let mut digits = [0; 24];
    let value: &[u8] = match value {
        ConfigValue::Table(table) => {
            for entry in table.named() {
                let (name, value) = entry?;
                let at = key.push(&[if key.len == 0 { b"" } else { b"/" }, name.to_bytes()])?;
                flatten(text, key, value)?;
                key.len = at;
            }
            return Ok(());
        }
        ConfigValue::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                let mut number = [0; 24];
                let at = key.push(&[b"[", whole(index as i32, &mut number), b"]"])?;
                flatten(text, key, value?)?;
                key.len = at;
            }
            return Ok(());
        }
        ConfigValue::Integer(n) => whole(n as i32, &mut digits),
        ConfigValue::Number(x) => number(x, &mut digits),
        ConfigValue::Text(t) => t.to_bytes(),
        ConfigValue::Unsupported => return Err(Unread::Kind),
    };
    if text.push(&[key.as_bytes()]) && text.push(&[value]) { Ok(()) } else { Err(Unread::Full) }
}

/// `n` in decimal (32-bit arithmetic: the arena has no 64-bit division).
fn whole(n: i32, out: &mut [u8; 24]) -> &[u8] {
    let mut at = out.len();
    let mut rest = n.unsigned_abs();
    loop {
        at -= 1;
        out[at] = b'0' + (rest % 10) as u8;
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    if n < 0 {
        at -= 1;
        out[at] = b'-';
    }
    &out[at..]
}

/// `x` in decimal, six places at the most, no trailing zeros: what the
/// movie's `Number()` reads back.
fn number(x: f32, out: &mut [u8; 24]) -> &[u8] {
    let size = f64::from(if x < 0.0 { -x } else { x });
    let mut units = size as u32;
    let mut fraction = ((size - f64::from(units)) * 1e6 + 0.5) as u32;
    const MILLION: u32 = 1000 * 1000;
    if fraction >= MILLION {
        (units, fraction) = (units.saturating_add(1), fraction - MILLION);
    }
    let mut len = 0;
    if x < 0.0 && (units, fraction) != (0, 0) {
        out[0] = b'-';
        len = 1;
    }
    let mut digits = [0; 24];
    let units = whole(units as i32, &mut digits);
    out[len..len + units.len()].copy_from_slice(units);
    len += units.len();
    if fraction != 0 {
        out[len] = b'.';
        len += 1;
        let mut places = 6;
        while fraction.is_multiple_of(10) {
            fraction /= 10;
            places -= 1;
        }
        for place in (0..places).rev() {
            out[len] = b'0' + (fraction / 10u32.pow(place) % 10) as u8;
            len += 1;
        }
    }
    &out[..len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_back_as_the_movie_reads_them() {
        let mut out = [0; 24];
        assert_eq!(whole(0, &mut out), b"0");
        assert_eq!(whole(30_000, &mut out), b"30000");
        assert_eq!(whole(-15, &mut out), b"-15");
        assert_eq!(number(0.5, &mut out), b"0.5");
        assert_eq!(number(1.0, &mut out), b"1");
        assert_eq!(number(0.06, &mut out), b"0.06");
        assert_eq!(number(-2.25, &mut out), b"-2.25");
        assert_eq!(number(12.000001, &mut out), b"12.000001");
    }

    #[test]
    fn a_table_goes_in_parts_of_its_name_and_whole_pairs() {
        let mut text = Text::new();
        for n in 0..40u8 {
            let mut digits = [0; 24];
            assert!(text.push(&[b"waits[", whole(i32::from(n), &mut digits), b"]"]));
            assert!(text.push(&[whole(i32::from(n) * 5, &mut [0; 24])]));
        }
        let mut parts = 0;
        let mut pairs = 0;
        assert!(give(c"rooms", &text, |strings| {
            parts += 1;
            assert_eq!(strings[0], c"rooms");
            assert_eq!(strings.len() % 2, 1);
            assert!(strings.len() <= MAX_STRINGS);
            pairs += (strings.len() - 1) / 2;
            true
        }));
        assert_eq!((parts, pairs), (2, 40));
        assert!(!give(c"rooms", &text, |_| false));
        let mut alone = 0;
        assert!(give(c"empty", &Text::new(), |strings| { alone += strings.len(); true }));
        assert_eq!(alone, 1);
    }

    #[test]
    fn a_text_refuses_what_does_not_fit() {
        let mut text = Text::new();
        let big = [b'x'; TEXT_BYTES - 1];
        assert!(text.push(&[&big]));
        assert!(!text.push(&[b""]));
        assert_eq!(text.strings().count(), 1);
    }
}

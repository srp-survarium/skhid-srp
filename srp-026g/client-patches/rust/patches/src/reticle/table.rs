//! The reticle table: srp's `A9` (`docs/notes/reticle-choice.md`, "The build's contract";
//! `survarium::v026g::lobby::reticles`), parsed once into a typed [`Table`] kept for the process,
//! and the lobby window's side of it.
//!
//! - **The message.** The lobby sends it before every inventory (`89`): the lists of every scope
//!   type with lists, and the account's scope items' choices. It is taken at the lobby's
//!   dispatcher before its jump table (`lobby_client.dispatch`), whose `A9` case (0.26g's
//!   premium discount) never sees it, and replaces the table whole; a message that does not
//!   parse leaves it empty (one line in the patches' log), so nothing shows a choice the server
//!   did not send.
//! - **The window.** As the inventory comes, setup_inventory_data makes each item's AS3 object
//!   (`lobby_menu.inventory_item`): a scope item the table has gets the member `srp_reticle =
//!   {pattern, size, colour, brightness, patterns: [name], sizes: n, colours: [name]}`. A stepper
//!   calls `srp_set_reticle(item, pattern, size, colour, brightness)` (`lobby.callback`, after
//!   patch 207's own method): checked against the table, it is sent as `46 05 <kit> <item>
//!   <item> <value>` (the value `pattern | size << 8 | colour << 16 | brightness << 24 | 0x52 <<
//!   56`), and the server answers with `A9`, the inventory and the account, from which the
//!   movie redraws.
//! - **The drawing** (`draw`): [`drawn_for_dict`], the fitted optic's reticle by its dict id.
//!
//! Model paths go into a pool that is never rewritten ([`NamePool`]), so a [`Drawn`]'s model is
//! a C string for the process whatever later messages bring. Nothing here changes what a match
//! computes or what a game writes into a world.

#![forbid(unsafe_code)]

use core::ffi::CStr;

use crate::vostok::log::{self, Piece};
use crate::vostok::names::NamePool;
use crate::vostok::parse::Reader;
use crate::vostok::scope_reticle::{InventoryItemObject, LobbyMessage, ReticleMember};
use crate::vostok::{
    Context, Flow, LobbyCall, Lock, NoExit, Reg, ReticleOnLobbyCallExit, ReticleOnLobbyMessageExit, Thread,
};

/// srp's lobby message of the scope reticles (0.26g's premium discount's id, which srp never
/// sends).
pub const MESSAGE_ID: u8 = 0xA9;
/// The lobby movie's method.
const METHOD: &[u8] = b"srp_set_reticle";
/// The most of each the table holds (the server's `survarium::v026g::reticles` limits).
pub const MAX_SCOPES: usize = 32;
pub const MAX_PATTERNS: usize = 16;
pub const MAX_SIZES: usize = 8;
pub const MAX_COLOURS: usize = 8;
pub const MAX_ITEMS: usize = 128;
/// The longest name and model path, without the zero.
const NAME_LIMIT: usize = 23;
const MODEL_LIMIT: usize = 95;
/// The most model paths the process keeps.
const MAX_MODELS: usize = 64;
/// The marker in a `46 05` value's top byte that makes it a reticle ('R').
const VALUE_MARK: u8 = 0x52;

/// A reticle's brightness: 0 to 10 (0.69d0's), 6 the default.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Brightness(u8);

impl Brightness {
    pub const MAX: u8 = 10;
    pub const DEFAULT: Brightness = Brightness(6);

    pub fn new(value: u8) -> Option<Brightness> {
        (value <= Self::MAX).then_some(Brightness(value))
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// What the aimed view draws for an optic: its reticle model, the size's factor on it, the
/// colour's emission and the brightness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub model: &'static CStr,
    pub scale: f32,
    pub rgb: [f32; 3],
    pub brightness: Brightness,
}

/// At most `N` of `T`, in order.
#[derive(Clone, Copy)]
struct Bounded<T: Copy, const N: usize> {
    items: [T; N],
    len: usize,
}

impl<T: Copy, const N: usize> Bounded<T, N> {
    const fn new(fill: T) -> Self {
        Bounded { items: [fill; N], len: 0 }
    }

    fn push(&mut self, item: T) -> Option<()> {
        *self.items.get_mut(self.len)? = item;
        self.len += 1;
        Some(())
    }

    fn as_slice(&self) -> &[T] {
        self.items.get(..self.len).unwrap_or(&[])
    }

    fn clear(&mut self) {
        self.len = 0;
    }
}

/// A pattern's or a colour's name: 1 to 23 of `[a-z0-9_]`, kept with its zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Name {
    bytes: [u8; NAME_LIMIT + 1],
}

impl Name {
    const EMPTY: Name = Name { bytes: [0; NAME_LIMIT + 1] };

    fn parse(r: &mut Reader<'_>) -> Option<Name> {
        let len = usize::from(r.u8()?);
        let text = r.take(len)?;
        let fits = (1..=NAME_LIMIT).contains(&len)
            && text.iter().all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !fits {
            return None;
        }
        let mut name = Name::EMPTY;
        name.bytes[..len].copy_from_slice(text);
        Some(name)
    }

    fn as_c_str(&self) -> &CStr {
        CStr::from_bytes_until_nul(&self.bytes).unwrap_or(c"")
    }
}

#[derive(Clone, Copy)]
struct Pattern {
    name: Name,
    model: &'static CStr,
}

#[derive(Clone, Copy)]
struct Colour {
    name: Name,
    rgb: [f32; 3],
}

/// One scope type's lists, each 1 to its most.
#[derive(Clone, Copy)]
struct Lists {
    dict: u16,
    patterns: Bounded<Pattern, MAX_PATTERNS>,
    sizes: Bounded<f32, MAX_SIZES>,
    colours: Bounded<Colour, MAX_COLOURS>,
}

/// A choice the lists hold: each index in its list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Choice {
    pattern: u8,
    size: u8,
    colour: u8,
    brightness: Brightness,
}

impl Choice {
    const DEFAULT: Choice = Choice { pattern: 0, size: 0, colour: 0, brightness: Brightness::DEFAULT };

    /// `(pattern, size, colour, brightness)` when `lists` hold it.
    fn of(lists: &Lists, numbers: [u32; 4]) -> Option<Choice> {
        let [pattern, size, colour, brightness] = numbers.map(|n| u8::try_from(n).ok());
        let (pattern, size, colour) = (pattern?, size?, colour?);
        let within = usize::from(pattern) < lists.patterns.len
            && usize::from(size) < lists.sizes.len
            && usize::from(colour) < lists.colours.len;
        within.then_some(())?;
        Some(Choice { pattern, size, colour, brightness: Brightness::new(brightness?)? })
    }

    /// The `46 05` value that asks for it.
    fn value(self) -> u64 {
        u64::from_le_bytes([self.pattern, self.size, self.colour, self.brightness.0, 0, 0, 0, VALUE_MARK])
    }

    fn drawn(self, lists: &Lists) -> Option<Drawn> {
        let pattern = lists.patterns.as_slice().get(usize::from(self.pattern))?;
        Some(Drawn {
            model: pattern.model,
            scale: *lists.sizes.as_slice().get(usize::from(self.size))?,
            rgb: lists.colours.as_slice().get(usize::from(self.colour))?.rgb,
            brightness: self.brightness,
        })
    }
}

/// A scope item of the account and its choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Item {
    item: u32,
    dict: u16,
    choice: Choice,
}

/// Why a message left the table empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Malformed {
    /// It ends early, or has bytes after its last item.
    Length,
    /// A count past the table's room, or an empty list.
    Count,
    /// A name, a model, a size or a colour no data gives.
    Value,
    /// A scope's lists twice, an item twice, an item of a scope it does not list, or a choice its
    /// lists do not hold.
    Reference,
    /// The process keeps no more model paths.
    Models,
}

/// The table: every scope type's lists and the account's scope items.
pub struct Table {
    lists: Bounded<Lists, MAX_SCOPES>,
    items: Bounded<Item, MAX_ITEMS>,
}

const EMPTY_LISTS: Lists = Lists {
    dict: 0,
    patterns: Bounded::new(Pattern { name: Name::EMPTY, model: c"" }),
    sizes: Bounded::new(0.0),
    colours: Bounded::new(Colour { name: Name::EMPTY, rgb: [0.0; 3] }),
};

impl Table {
    pub const fn new() -> Table {
        Table {
            lists: Bounded::new(EMPTY_LISTS),
            items: Bounded::new(Item { item: 0, dict: 0, choice: Choice::DEFAULT }),
        }
    }

    fn lists(&self, dict: u16) -> Option<&Lists> {
        self.lists.as_slice().iter().find(|lists| lists.dict == dict)
    }

    fn item(&self, item: u32) -> Option<&Item> {
        self.items.as_slice().iter().find(|held| held.item == item)
    }

    /// The table of `body`, read in place: what it held is gone either way, and a body that does
    /// not parse leaves it empty.
    fn read(&mut self, body: Reader<'_>, models: &'static NamePool<MAX_MODELS, { MODEL_LIMIT + 1 }>) -> Result<(), Malformed> {
        let read = self.read_all(body, models);
        if read.is_err() {
            self.lists.clear();
            self.items.clear();
        }
        read
    }

    fn read_all(&mut self, mut r: Reader<'_>, models: &'static NamePool<MAX_MODELS, { MODEL_LIMIT + 1 }>) -> Result<(), Malformed> {
        use Malformed::{Count, Length, Reference, Value};
        self.lists.clear();
        self.items.clear();
        let scopes = usize::from(r.u8().ok_or(Length)?);
        if scopes > MAX_SCOPES {
            return Err(Count);
        }
        for _ in 0..scopes {
            let mut lists = EMPTY_LISTS;
            lists.dict = r.u16().ok_or(Length)?;
            if self.lists(lists.dict).is_some() {
                return Err(Reference);
            }
            for _ in 0..count(&mut r, MAX_PATTERNS)? {
                let name = Name::parse(&mut r).ok_or(Value)?;
                let len = usize::from(r.u8().ok_or(Length)?);
                let path = r.take(len).ok_or(Length)?;
                if !(1..=MODEL_LIMIT).contains(&len) || !path.iter().all(u8::is_ascii_graphic) {
                    return Err(Value);
                }
                let model = models.intern(path).ok_or(Malformed::Models)?;
                lists.patterns.push(Pattern { name, model }).ok_or(Count)?;
            }
            for _ in 0..count(&mut r, MAX_SIZES)? {
                let size = r.f32().ok_or(Length)?;
                if !(size.is_finite() && size > 0.0 && size <= 16.0) {
                    return Err(Value);
                }
                lists.sizes.push(size).ok_or(Count)?;
            }
            for _ in 0..count(&mut r, MAX_COLOURS)? {
                let name = Name::parse(&mut r).ok_or(Value)?;
                let rgb = [r.f32().ok_or(Length)?, r.f32().ok_or(Length)?, r.f32().ok_or(Length)?];
                if !rgb.iter().all(|c| c.is_finite() && (0.0..=16.0).contains(c)) {
                    return Err(Value);
                }
                lists.colours.push(Colour { name, rgb }).ok_or(Count)?;
            }
            self.lists.push(lists).ok_or(Count)?;
        }
        let items = usize::from(r.u16().ok_or(Length)?);
        if items > MAX_ITEMS {
            return Err(Count);
        }
        for _ in 0..items {
            let (item, dict) = (r.u32().ok_or(Length)?, r.u16().ok_or(Length)?);
            let numbers = [r.u8(), r.u8(), r.u8(), r.u8()].map(|n| n.map(u32::from));
            let [Some(pattern), Some(size), Some(colour), Some(brightness)] = numbers else { return Err(Length) };
            let lists = self.lists(dict).ok_or(Reference)?;
            let choice = Choice::of(lists, [pattern, size, colour, brightness]).ok_or(Reference)?;
            if self.item(item).is_some() {
                return Err(Reference);
            }
            self.items.push(Item { item, dict, choice }).ok_or(Count)?;
        }
        if !r.is_empty() {
            return Err(Length);
        }
        Ok(())
    }

    /// The optic `dict`'s reticle: that of the account's item of it with the lowest item id, or
    /// its lists' default when the account holds none (a weapon picked up in a match); none
    /// for a scope without lists.
    fn drawn_for_dict(&self, dict: u16) -> Option<Drawn> {
        let lists = self.lists(dict)?;
        let choice = self
            .items
            .as_slice()
            .iter()
            .filter(|held| held.dict == dict)
            .min_by_key(|held| held.item)
            .map_or(Choice::DEFAULT, |held| held.choice);
        choice.drawn(lists)
    }

    /// The `46 05` value of item `item`'s choice `numbers`, when the table has the item and its
    /// lists hold the choice.
    fn request(&self, item: u32, numbers: [u32; 4]) -> Option<u64> {
        let held = self.item(item)?;
        Choice::of(self.lists(held.dict)?, numbers).map(Choice::value)
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

/// A list's count: 1 to `most`.
fn count(r: &mut Reader<'_>, most: usize) -> Result<usize, Malformed> {
    let n = usize::from(r.u8().ok_or(Malformed::Length)?);
    if n == 0 || n > most {
        return Err(Malformed::Count);
    }
    Ok(n)
}

/// The table for the process: the lobby's thread writes it, the drawing reads it (on whichever
/// thread draws).
static TABLE: Lock<Table> = Lock::new(Table::new());
/// The model paths of every message the process read.
static MODELS: NamePool<MAX_MODELS, { MODEL_LIMIT + 1 }> = NamePool::new();

/// The fitted optic `dict`'s reticle (`draw`'s question): the account's choice for its scope
/// item of that type, its lists' default when it holds none; none for a scope without lists (its
/// aimed model is drawn as it is).
pub fn drawn_for_dict(thread: &Thread, dict: u16) -> Option<Drawn> {
    TABLE.with(thread, |table| table.drawn_for_dict(dict))
}

/// lobby_client::on_message_received before its jump table: srp's `A9` read into the table and
/// taken (the table's own case would read it as the premium discount); every other id to the
/// game.
pub fn on_lobby_message(ctx: &mut Context<'_>) -> Flow<ReticleOnLobbyMessageExit> {
    let Some(message) = LobbyMessage::at(ctx) else { return Flow::Continue };
    if message.id() != MESSAGE_ID {
        return Flow::Continue;
    }
    let read = TABLE.with(ctx.thread(), |table| table.read(message.body(), &MODELS));
    if let Err(why) = read {
        let why = match why {
            Malformed::Length => "its length",
            Malformed::Count => "a count",
            Malformed::Value => "a name, model, size or colour",
            Malformed::Reference => "a scope or an item named wrongly",
            Malformed::Models => "no room for its models",
        };
        log::refusal(ctx.thread(), &[Piece::Text("reticle: srp's A9 not read ("), Piece::Text(why),
                                     Piece::Text("): no scope shows a choice")]);
    }
    Flow::Exit(ReticleOnLobbyMessageExit::Done)
}

/// setup_inventory_data with an item's AS3 object filled: a scope item of the table gets its
/// `srp_reticle`. The item and its lists are copied out of the table first, so the movie is called
/// with the table's lock free.
pub fn on_inventory_item(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(object) = InventoryItemObject::at(ctx) else { return Flow::Continue };
    let item = object.item_id();
    let found = TABLE.with(ctx.thread(), |table| {
        let held = *table.item(item).filter(|held| held.dict == object.dict())?;
        Some((held, *table.lists(held.dict)?))
    });
    if let Some((held, lists)) = found {
        let mut patterns = [c""; MAX_PATTERNS];
        for (place, pattern) in patterns.iter_mut().zip(lists.patterns.as_slice()) {
            *place = pattern.name.as_c_str();
        }
        let mut colours = [c""; MAX_COLOURS];
        for (place, colour) in colours.iter_mut().zip(lists.colours.as_slice()) {
            *place = colour.name.as_c_str();
        }
        let member = ReticleMember {
            pattern: u32::from(held.choice.pattern),
            size: u32::from(held.choice.size),
            colour: u32::from(held.choice.colour),
            brightness: u32::from(held.choice.brightness.get()),
            patterns: &patterns[..lists.patterns.len],
            sizes: lists.sizes.len as u32,
            colours: &colours[..lists.colours.len],
        };
        if !object.give_reticle(&member) {
            log::refusal(ctx.thread(), &[Piece::Text("reticle: scope item "), Piece::Dec(item),
                                         Piece::Text("'s srp_reticle not set (no AS3 object)")]);
        }
    }
    Flow::Continue
}

/// lobby_menu::scaleform_callback's start: `srp_set_reticle(item, pattern, size, colour,
/// brightness)` sent as `46 05` when the table holds it; every other method to the game.
pub fn on_lobby_call(ctx: &mut Context<'_>) -> Flow<ReticleOnLobbyCallExit> {
    {
        let Some(call) = LobbyCall::at(ctx) else { return Flow::Continue };
        if call.method().to_bytes() != METHOD {
            return Flow::Continue;
        }
        let item = call.uint_argument(0);
        let numbers = [1, 2, 3, 4].map(|index| call.uint_argument(index));
        let sent = (|| {
            let (item, [Some(p), Some(s), Some(c), Some(b)]) = (item?, numbers) else { return None };
            let value = TABLE.with(call.thread(), |table| table.request(item, [p, s, c, b]))?;
            let inventory = call.inventory()?;
            inventory.item(item)?;
            inventory.send_reticle(item, value);
            Some(())
        })();
        if sent.is_none() {
            log::refusal(call.thread(), &[Piece::Text("reticle: srp_set_reticle("), Piece::Dec(item.unwrap_or(0)),
                                          Piece::Text(", ...): no scope item of the table holds it, nothing sent")]);
        }
    }
    // the menu's own handlers answer al 1, handled (GFx's caller drops it)
    ctx.set_reg(Reg::Eax, 1);
    Flow::Exit(ReticleOnLobbyCallExit::Done)
}

#[cfg(test)]
mod tests {
    use super::*;

    static POOL: NamePool<MAX_MODELS, { MODEL_LIMIT + 1 }> = NamePool::new();

    const COBRA: u16 = 2108;
    const NCSTAR: u16 = 2113;

    /// A message as the server writes it (`survarium::v026g::lobby::reticles`).
    struct Message(Vec<u8>);

    impl Message {
        fn new(scopes: u8) -> Message {
            Message(vec![scopes])
        }
        fn str(mut self, text: &str) -> Message {
            self.0.push(text.len() as u8);
            self.0.extend(text.as_bytes());
            self
        }
        fn scope(mut self, dict: u16, patterns: &[&str], sizes: &[f32], colours: &[(&str, [f32; 3])]) -> Message {
            self.0.extend(dict.to_le_bytes());
            self.0.push(patterns.len() as u8);
            for pattern in patterns {
                self = self.str(pattern).str(&format!("weapons/reticles_069/{pattern}"));
            }
            self.0.push(sizes.len() as u8);
            for size in sizes {
                self.0.extend(size.to_le_bytes());
            }
            self.0.push(colours.len() as u8);
            for (name, rgb) in colours {
                self = self.str(name);
                for c in rgb {
                    self.0.extend(c.to_le_bytes());
                }
            }
            self
        }
        fn items(mut self, items: &[(u32, u16, [u8; 4])]) -> Vec<u8> {
            self.0.extend((items.len() as u16).to_le_bytes());
            for (item, dict, numbers) in items {
                self.0.extend(item.to_le_bytes());
                self.0.extend(dict.to_le_bytes());
                self.0.extend(numbers);
            }
            self.0
        }
    }

    const RED: (&str, [f32; 3]) = ("red", [1.0, 0.0, 0.0]);
    const GREEN: (&str, [f32; 3]) = ("green", [0.0, 1.0, 0.0]);
    const SIZES: [f32; 4] = [1.0, 0.75, 0.5, 0.35];

    fn two_scopes(items: &[(u32, u16, [u8; 4])]) -> Vec<u8> {
        Message::new(2)
            .scope(COBRA, &["dot", "cobra_cross"], &SIZES, &[RED, GREEN])
            .scope(NCSTAR, &["dot", "ncstar_bullseye", "ncstar_starburst"], &SIZES, &[RED, GREEN, ("blue", [0.0, 0.0, 1.0])])
            .items(items)
    }

    fn table_of(bytes: &[u8]) -> (Table, Result<(), Malformed>) {
        let mut table = Table::new();
        let read = table.read(Reader::new(bytes), &POOL);
        (table, read)
    }

    #[test]
    fn a_message_is_read_into_the_table_and_drawn_by_dict() {
        let bytes = two_scopes(&[(70, COBRA, [1, 2, 1, 9]), (40, NCSTAR, [2, 0, 2, 0]), (41, NCSTAR, [1, 1, 0, 10])]);
        let (table, read) = table_of(&bytes);
        assert_eq!(read, Ok(()));
        let cobra = table.drawn_for_dict(COBRA).unwrap();
        assert_eq!(cobra.model.to_bytes(), b"weapons/reticles_069/cobra_cross");
        assert_eq!((cobra.scale, cobra.rgb, cobra.brightness.get()), (0.5, [0.0, 1.0, 0.0], 9));
        // Two of one type: the lowest item id's.
        let ncstar = table.drawn_for_dict(NCSTAR).unwrap();
        assert_eq!(ncstar.model.to_bytes(), b"weapons/reticles_069/ncstar_starburst");
        assert_eq!((ncstar.rgb, ncstar.brightness.get()), ([0.0, 0.0, 1.0], 0));
        // The same path is one C string for the process.
        let again = table_of(&bytes).0.drawn_for_dict(COBRA).unwrap();
        assert!(core::ptr::eq(again.model, cobra.model));
        // A scope with no lists draws as it is.
        assert_eq!(table.drawn_for_dict(2101), None);
    }

    #[test]
    fn a_scope_the_account_lacks_draws_its_default() {
        let (table, read) = table_of(&two_scopes(&[]));
        assert_eq!(read, Ok(()));
        let cobra = table.drawn_for_dict(COBRA).unwrap();
        assert_eq!(cobra.model.to_bytes(), b"weapons/reticles_069/dot");
        assert_eq!((cobra.scale, cobra.rgb, cobra.brightness), (1.0, [1.0, 0.0, 0.0], Brightness::DEFAULT));
    }

    #[test]
    fn a_request_is_held_to_the_items_lists() {
        let (table, _) = table_of(&two_scopes(&[(70, COBRA, [0, 0, 0, 6])]));
        assert_eq!(table.request(70, [1, 3, 1, 10]), Some(0x5200_0000_0A01_0301));
        assert_eq!(table.request(70, [2, 0, 0, 6]), None);
        assert_eq!(table.request(70, [0, 4, 0, 6]), None);
        assert_eq!(table.request(70, [0, 0, 2, 6]), None);
        assert_eq!(table.request(70, [0, 0, 0, 11]), None);
        assert_eq!(table.request(70, [256, 0, 0, 6]), None);
        assert_eq!(table.request(71, [0, 0, 0, 6]), None);
    }

    #[test]
    fn a_message_that_does_not_parse_leaves_the_table_empty() {
        let good = two_scopes(&[(70, COBRA, [1, 0, 0, 6])]);
        let mut table = Table::new();
        assert_eq!(table.read(Reader::new(&good), &POOL), Ok(()));
        let refused = |bytes: &[u8], why: Malformed| {
            let mut table = Table::new();
            assert_eq!(table.read(Reader::new(&good), &POOL), Ok(()));
            assert_eq!(table.read(Reader::new(bytes), &POOL), Err(why));
            assert_eq!(table.drawn_for_dict(COBRA), None);
            assert!(table.item(70).is_none());
        };
        refused(&good[..good.len() - 1], Malformed::Length);
        refused(&[good.as_slice(), &[0]].concat(), Malformed::Length);
        refused(&two_scopes(&[(70, COBRA, [2, 0, 0, 6])]), Malformed::Reference);
        refused(&two_scopes(&[(70, COBRA, [0, 0, 0, 11])]), Malformed::Reference);
        refused(&two_scopes(&[(70, 2101, [0, 0, 0, 6])]), Malformed::Reference);
        refused(&two_scopes(&[(70, COBRA, [0, 0, 0, 6]), (70, COBRA, [0, 0, 0, 6])]), Malformed::Reference);
        refused(&Message::new(2).scope(COBRA, &["dot"], &SIZES, &[RED]).scope(COBRA, &["dot"], &SIZES, &[RED]).items(&[]),
                Malformed::Reference);
        refused(&Message::new(1).scope(COBRA, &["Dot"], &SIZES, &[RED]).items(&[]), Malformed::Value);
        refused(&Message::new(1).scope(COBRA, &["dot"], &[0.0], &[RED]).items(&[]), Malformed::Value);
        refused(&Message::new(1).scope(COBRA, &["dot"], &[f32::NAN], &[RED]).items(&[]), Malformed::Value);
        refused(&Message::new(1).scope(COBRA, &["dot"], &SIZES, &[("red", [-1.0, 0.0, 0.0])]).items(&[]), Malformed::Value);
        refused(&Message::new(1).scope(COBRA, &[], &SIZES, &[RED]).items(&[]), Malformed::Count);
        refused(&Message::new(MAX_SCOPES as u8 + 1).items(&[]), Malformed::Count);
        let seventeen: Vec<String> = (0..17).map(|i| format!("p{i}")).collect();
        let seventeen: Vec<&str> = seventeen.iter().map(String::as_str).collect();
        refused(&Message::new(1).scope(COBRA, &seventeen, &SIZES, &[RED]).items(&[]), Malformed::Count);
        // None at all: an empty table.
        let (empty, read) = table_of(&[0, 0, 0]);
        assert_eq!(read, Ok(()));
        assert_eq!(empty.drawn_for_dict(COBRA), None);
    }

    #[test]
    fn a_choice_is_its_server_value() {
        let choice = Choice { pattern: 1, size: 2, colour: 3, brightness: Brightness(4) };
        assert_eq!(choice.value(), 0x5200_0000_0403_0201);
        assert_eq!(Brightness::new(10), Some(Brightness(10)));
        assert_eq!(Brightness::new(11), None);
    }
}

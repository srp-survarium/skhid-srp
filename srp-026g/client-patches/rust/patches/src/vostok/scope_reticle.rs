//! Where patch `reticle`'s table meets the game (`docs/notes/reticle-choice.md`, "The build's
//! contract"):
//!
//! - [`LobbyMessage`]: a lobby message at `lobby_client.dispatch` (on_message_received 0x274A39,
//!   past the messaging ids, before the jump table): its id and its body. srp's `A9` (0.26g's
//!   premium discount, which srp never sends) is taken there and never reaches the table, which
//!   would read it as the discount;
//! - [`InventoryItemObject`]: an inventory item's AS3 object in setup_inventory_data, at
//!   `lobby_menu.inventory_item` (0x28867F, after the item-to-Flash function filled it, before it
//!   is put into the items' array): its item, and [`InventoryItemObject::give_reticle`], the
//!   object's member `srp_reticle`.

use core::ffi::CStr;

use super::context::Context;
use super::hook::Thread;
use super::parse::Reader;
use super::ui::{self, GFX_ARRAY, GFX_OBJECT, set_element, set_member, string_value, uint_value};
use crate::raw;

/// A lobby message at `lobby_client.dispatch`, read once.
pub struct LobbyMessage<'t> {
    id: u8,
    reader: raw::BufferReader,
    _thread: &'t Thread,
}

impl<'t> LobbyMessage<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<LobbyMessage<'t>> {
        use raw::site::lobby_client_dispatch as site;
        // SAFETY: the context is this site's: DL the id (EDX the argument's dword), the
        // function's frame (EBP) with the message's reader at +0Ch, alive while the dispatcher
        // runs on the game's thread.
        let (id, reader) = unsafe { (site::id(ctx) as u8, site::frame(ctx)?.reader()?) };
        Some(LobbyMessage { id, reader, _thread: ctx.thread() })
    }

    pub fn id(&self) -> u8 {
        self.id
    }

    /// The message's body: the bytes from the reader's cursor to its end.
    pub fn body(&self) -> Reader<'t> {
        // SAFETY: the live reader, its message alive while the dispatcher runs.
        unsafe { super::options::unread(self.reader) }
    }
}

/// What a scope item's `srp_reticle` member holds: `{pattern, size, colour, brightness,
/// patterns: [name], sizes: n, colours: [name]}` (AS3 uints and Strings).
pub struct ReticleMember<'a> {
    pub pattern: u32,
    pub size: u32,
    pub colour: u32,
    pub brightness: u32,
    pub patterns: &'a [&'a CStr],
    pub sizes: u32,
    pub colours: &'a [&'a CStr],
}

/// An inventory item's AS3 object at `lobby_menu.inventory_item`.
pub struct InventoryItemObject<'t> {
    item: raw::ItemRecord,
    menu: raw::LobbyMenu,
    frame: raw::SetupInventoryFrame,
    _thread: &'t Thread,
}

impl<'t> InventoryItemObject<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<InventoryItemObject<'t>> {
        use raw::site::lobby_menu_inventory_item as site;
        // SAFETY: the context is this site's: the item record (EDI), the lobby menu (EBX) and
        // setup_inventory_data's frame (EBP), alive while it runs on the lobby's thread.
        let (item, menu, frame) = unsafe { (site::item(ctx)?, site::menu(ctx)?, site::frame(ctx)?) };
        Some(InventoryItemObject { item, menu, frame, _thread: ctx.thread() })
    }

    /// The item's id.
    pub fn item_id(&self) -> u32 {
        // SAFETY: the live item record.
        unsafe { *self.item.id() }
    }

    /// The item's dict id.
    pub fn dict(&self) -> u16 {
        // SAFETY: the live item record.
        unsafe { *self.item.dict() }
    }

    /// The object's member `srp_reticle` is a new AS3 object of `member`'s values, made with the
    /// lobby movie (its arrays too), each released after it is set (the item's object holds it
    /// then). Whether it was set: not when the object is no AS3 one or the movie made nothing.
    pub fn give_reticle(&self, member: &ReticleMember<'_>) -> bool {
        // SAFETY: the menu is live on the lobby's thread; its movie holder and movie live as long.
        let Some(movie) = (unsafe { ui::movie_of(self.menu.movie()) }) else { return false };
        // SAFETY: the frame's own item object, alive until the function releases it (0x2886B0).
        let target = unsafe { self.frame.object() };
        let mut rooms = [const { raw::GfxValueStorage::zeroed() }; 3];
        let [object_room, patterns_room, colours_room] = &mut rooms;
        let Some(object) = movie.made(object_room, GFX_OBJECT) else { return false };
        let Some(patterns) = movie.made(patterns_room, GFX_ARRAY) else { return false };
        let Some(colours) = movie.made(colours_room, GFX_ARRAY) else { return false };
        let mut room = raw::GfxValueStorage::zeroed();
        let uints = [
            (c"pattern", member.pattern),
            (c"size", member.size),
            (c"colour", member.colour),
            (c"brightness", member.brightness),
            (c"sizes", member.sizes),
        ];
        // SAFETY: our live values of the lobby movie, and our rooms; the names C strings alive
        // for the calls (the movie copies a string where it is set).
        unsafe {
            for (name, number) in uints {
                if !set_member(object.value(), name, uint_value(&mut room, number)) {
                    return false;
                }
            }
            for (array, names) in [(&patterns, member.patterns), (&colours, member.colours)] {
                for (index, name) in names.iter().enumerate() {
                    if !set_element(array.value(), index as u32, string_value(&mut room, name)) {
                        return false;
                    }
                }
            }
            set_member(object.value(), c"patterns", patterns.value())
                && set_member(object.value(), c"colours", colours.value())
                && set_member(target, c"srp_reticle", object.value())
        }
    }
}

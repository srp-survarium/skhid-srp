//! An upgrade (a camouflage, a decal) on the lobby's items: the lobby
//! client's messages that put one on or take it off, at the four sites of
//! sites_026g.tsv `lobby_upgrade.drop_sent`, `inventory_action.*` and
//! `detach_upgrade.find_holder`. What each site's registers and stack hold
//! is said here, once:
//! - `lobby_upgrade.drop_sent` (drop_upgrade_item's handler 0x29A9D0): EAX
//!   the lobby client, ECX the kit, EDI the upgrade, EBP the handler's
//!   frame (its argument, the Flash call's: the item its first value);
//! - `inventory_action.detach_answer` (the server's `83 46 02`): EAX the
//!   message's read position, ESI its reader, EBP the lobby client;
//! - `inventory_action.detach_first` (`83 46 01`'s): EBX the item, EDI the
//!   upgrade, EBP the lobby client;
//! - `detach_upgrade.find_holder` (process_detach_upgrade's search, the
//!   cdecl call of 0x277CF0 about to be made): [ESP] the inventory's items,
//!   [ESP+4] their end.

use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::gfx::Value;
use super::hook::Thread;
use super::object::{Object, pointer, read, write};
use crate::raw;

/// An inventory item's size: the lobby client's items are an array of
/// them (find_item_holding_upgrade 0x277CF0 divides by B4h).
const ITEM_SIZE: u32 = 0xB4;

/// The lobby client, as a site's register holds it.
pub struct LobbyClient<'t> {
    client: Object,
    thread: PhantomData<&'t Thread>,
}

impl<'t> LobbyClient<'t> {
    fn at(ctx: &'t Context<'_>, reg: Reg) -> Option<LobbyClient<'t>> {
        Object::at(ctx.reg(reg)).map(|client| LobbyClient { client, thread: PhantomData })
    }

    /// Sends `46 <bytes>` on its match client: a new packet, its buffer
    /// grown by them, the bytes after what it held, then queued, as the
    /// lobby's own messages are made (0x2708C0).
    pub fn send(&self, bytes: &[u8]) {
        // SAFETY: the lobby client's match client (+8); new_packet, resize
        // and enqueue as the game's own senders call them; the packet's
        // buffer is resized to hold the bytes written into it.
        unsafe {
            let connection = raw::lobby_client::connection(self.client.ptr()).cast();
            let packet = raw::engine_new_packet(connection, u32::from(MESSAGE_ID));
            let buffer = read(raw::packet::buffer(packet));
            let size = read(raw::mutable_buffer::size(buffer as usize as *mut _));
            raw::engine_buffer_resize(buffer as usize as *mut _, size + bytes.len() as u32);
            let buffer = read(raw::packet::buffer(packet)) as usize as *mut core::ffi::c_void;
            let data = read(raw::mutable_buffer::data(buffer)) as usize as *mut u8;
            for (i, &byte) in bytes.iter().enumerate() {
                write(data.add(size as usize + i), byte);
            }
            raw::engine_enqueue(connection, packet);
        }
    }

    /// process_detach_upgrade(the client, `upgrade`): the upgrade off the
    /// item find_holder answers; what it leaves in EAX.
    pub fn process_detach_upgrade(&self, upgrade: u32) -> u32 {
        // SAFETY: the lobby client; the game's own function (stdcall).
        unsafe { raw::engine_process_detach_upgrade(self.client.ptr(), upgrade) }
    }
}

/// The lobby's inventory message (the first byte of `46 ..`).
const MESSAGE_ID: u8 = 0x46;

/// `lobby_upgrade.drop_sent`: what drop_upgrade_item was asked.
pub struct DropSent<'t> {
    pub client: LobbyClient<'t>,
    pub kit: u32,
    pub upgrade: u32,
    /// The item the window shows (the Flash call's first value).
    pub item: u32,
}

impl<'t> DropSent<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<DropSent<'t>> {
        // SAFETY: the handler's frame (EBP): its argument the Flash call's
        // arguments, +8 their GFx values, the first one's data the item.
        let item = unsafe {
            let args = pointer(raw::drop_upgrade_frame::args(ctx.reg(Reg::Ebp) as usize as *mut _))?;
            let values = pointer(raw::callback_args::values(args.ptr()))?;
            read(values.ptr().cast::<Value>()).data
        };
        Some(DropSent { client: LobbyClient::at(ctx, Reg::Eax)?, kit: ctx.reg(Reg::Ecx), upgrade: ctx.reg(Reg::Edi), item })
    }
}

/// `inventory_action.detach_answer`: the message's reader.
pub struct DetachAnswer<'t> {
    pub client: LobbyClient<'t>,
    position: *mut u32,
    reader: Object,
}

impl<'t> DetachAnswer<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<DetachAnswer<'t>> {
        Some(DetachAnswer {
            client: LobbyClient::at(ctx, Reg::Ebp)?,
            position: ctx.reg(Reg::Eax) as usize as *mut u32,
            reader: Object::at(ctx.reg(Reg::Esi))?,
        })
    }

    /// Reads the next `count` dwords of the message, moving the reader's
    /// position past them (+4, as the case's own reads do).
    pub fn read_dwords<const N: usize>(&self) -> [u32; N] {
        let mut out = [0; N];
        for (i, value) in out.iter_mut().enumerate() {
            // SAFETY: the message's bytes at the read position, as the
            // case reads them.
            *value = unsafe { read(self.position.add(i)) };
        }
        // SAFETY: the reader's position, which the case moves.
        unsafe { write(raw::message_reader::position(self.reader.ptr()), self.position.add(N) as usize as u32) }
        out
    }
}

/// `inventory_action.detach_first`: the item and the upgrade.
pub struct DetachFirst<'t> {
    pub client: LobbyClient<'t>,
    pub item: u32,
    pub upgrade: u32,
}

impl<'t> DetachFirst<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<DetachFirst<'t>> {
        Some(DetachFirst { client: LobbyClient::at(ctx, Reg::Ebp)?, item: ctx.reg(Reg::Ebx), upgrade: ctx.reg(Reg::Edi) })
    }
}

/// `detach_upgrade.find_holder`: the inventory's items, about to be searched.
pub struct Holders {
    begin: u32,
    end: u32,
}

impl Holders {
    pub fn at(ctx: &Context<'_>) -> Holders {
        Holders { begin: ctx.stack(0), end: ctx.stack(4) }
    }

    /// The item whose id is `id`, or the end (an item's address, as the
    /// search answers).
    pub fn find(&self, id: u32) -> u32 {
        let mut at = self.begin;
        while at != self.end {
            // SAFETY: an item of the inventory's array (below its end).
            if unsafe { read(raw::inventory_item::id(at as usize as *mut _)) } == id {
                break;
            }
            at = at.wrapping_add(ITEM_SIZE);
        }
        at
    }

    pub fn end(&self) -> u32 {
        self.end
    }
}

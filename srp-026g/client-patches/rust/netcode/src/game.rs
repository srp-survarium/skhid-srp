//! The game's side (`netcode_026g.asm`'s entries): the game's memory read
//! once into [`World`] and the other parsed values, the machine's answers
//! written back through the game's own functions.

use core::ffi::c_void;

use patch_core::log::{Piece::*, line};
use patch_core::{Field, GameFn, Obj, Regs, Va};
use sync_core::wire::{ClientUpdate, Place, ServerUpdate, CLIENT_UPDATE_MAX};
use sync_core::{GameTime, Grid};

use crate::machine::{Netcode, World};
use crate::stock::Stock;
use crate::trailer;

/// The machine: one network client at a time, on the logic thread.
static mut NETCODE: Netcode = Netcode::new();

/// `survarium::network_client`.
pub enum Client {}
/// Its connection state: 1 the ports known (0xE4 is taken), 2 the static
/// info received.
const STATE: Field<Client, u32> = Field::at(0x1F1C64);
/// The match core (`pvp_match_core`), once the level is loaded.
const MATCH_CORE: Field<Client, Option<Obj<MatchCore>>> = Field::at(0x1F1C40);
/// The match client: its packets (`match_client`, vtable 0xA08864).
const LINK: Field<Client, Option<Obj<Link>>> = Field::at(0x1F1C48);

/// `survarium::pvp_match_core`: the world core is at +1F8h.
pub enum MatchCore {}
const NOW: Field<MatchCore, u32> = Field::at(0x1F8 + 0x18FC0);
const RESYNCED: Field<MatchCore, u32> = Field::at(0x1F8 + 0x18FD4);
const OWNER: Field<MatchCore, u8> = Field::at(0x1F8 + 0x18FDC);
const FROZEN: Field<MatchCore, u8> = Field::at(0x1F8 + 0x18FDF);

/// `match_client`.
pub enum Link {}
/// Its packet pool (+578B0h): packets in use and its capacity (256).
const POOL_IN_USE: Field<Link, u32> = Field::at(0x578B0 + 0x30);
const POOL_CAPACITY: Field<Link, u32> = Field::at(0x578B0 + 0x34);
/// A packet's room left for others (the snapshot upload's guard).
const POOL_SPARE: u32 = 16;

/// `vostok::network_core::buffer_reader`: the message's bytes are
/// `[pos, base + size)`.
#[repr(C)]
struct Reader {
    base: *const u8,
    pos: *const u8,
    size: u32,
    /// What the game's reader may keep past the three (zeros).
    rest: [u32; 5],
}

/// A packet (`construct_packet` 0x378080).
pub enum Packet {}
/// Its writer (the ctor 0x278D9F..0x278DD7: the buffer at +298h).
const WRITER: Field<Packet, u8> = Field::at(0x2A8);
/// Its flags: bit 6, may be joined with others in a datagram.
const PACKET_FLAGS: Field<Packet, u8> = Field::at(0x98);
const JOINED: u8 = 0x40;

/// `network_client::on_match_packet_received(u8 id, buffer_reader&)`.
const DISPATCH: GameFn<unsafe extern "thiscall" fn(*mut c_void, u32, *mut Reader)> =
    GameFn::at(Va(0x27A300));
/// `match_client::new_packet(id)` (vtable +18h).
const NEW_PACKET: GameFn<unsafe extern "thiscall" fn(*mut c_void, u32) -> *mut u8> =
    GameFn::at(Va(0x279FE0));
/// `match_client::enqueue(packet)` (vtable +1Ch).
const ENQUEUE: GameFn<unsafe extern "thiscall" fn(*mut c_void, *mut u8)> = GameFn::at(Va(0x27A2A0));

/// The client's update of a tick (`udp_transport::table::CLIENT_UPDATE`).
const CLIENT_UPDATE: u32 = 0xE2;

/// The world as a message finds it.
unsafe fn world(client: Obj<Client>) -> World {
    unsafe {
        let Some(core) = client.get(MATCH_CORE) else {
            return World::NoCore;
        };
        if core.get(FROZEN) != 0 {
            return World::Frozen;
        }
        World::Running {
            now: GameTime(core.get(NOW)),
            resynced: GameTime(core.get(RESYNCED)),
            local: Place::new(core.get(OWNER)),
        }
    }
}

/// The bytes a reader has left.
unsafe fn left(reader: *mut Reader) -> &'static [u8] {
    unsafe {
        let reader = &*reader;
        let end = reader.base.wrapping_add(reader.size as usize);
        let len = (end as usize).saturating_sub(reader.pos as usize);
        core::slice::from_raw_parts(reader.pos, len)
    }
}

/// A stock message through the game's own dispatch.
unsafe fn dispatch(client: Obj<Client>, stock: Stock, grid: Grid) {
    let encoded = stock.encode(grid);
    let bytes = encoded.payload();
    let mut reader = Reader {
        base: bytes.as_ptr(),
        pos: bytes.as_ptr(),
        size: bytes.len() as u32,
        rest: [0; 5],
    };
    unsafe { DISPATCH.get()(client.ptr().cast(), u32::from(encoded.id), &mut reader) }
}

/// 0xF7 (the dispatch's entry, before its table): the server's update of a
/// tick. ESI the network client, EBX the reader; the entry goes on to the
/// dispatch's end with the reader consumed.
///
/// # Safety
/// The dispatch's state at 0x27A322, on the logic thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn netcode_update(regs: &mut Regs) {
    unsafe {
        let Some(client) = Obj::<Client>::at(Va(regs.esi)) else {
            return;
        };
        let reader = regs.ebx as *mut Reader;
        let bytes = left(reader);
        // Consumed whatever becomes of it.
        (*reader).pos = (*reader).base.wrapping_add((*reader).size as usize);
        let netcode = &mut *(&raw mut NETCODE);
        let Some(grid) = netcode.grid() else {
            return;
        };
        // The stock handlers' own state test (state 2).
        if client.get(STATE) != 2 {
            return;
        }
        let Ok(update) = ServerUpdate::parse(bytes, grid) else {
            line(&[Text("netcode: an update that is none, bytes "), Dec(bytes.len() as u32)]);
            return;
        };
        let world = world(client);
        netcode.update(&update, world, &mut |stock| dispatch(client, stock, grid));
    }
}

/// 0xE4 in the state that takes the static info: the per-tick trailer,
/// taken off the message (the stock handler, the late join's and the
/// client check see the stock one), decides the match's protocol and grid.
///
/// # Safety
/// The dispatch's state at 0x27A322 for 0xE4, on the logic thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn netcode_static_info(regs: &mut Regs) {
    unsafe {
        let Some(client) = Obj::<Client>::at(Va(regs.esi)) else {
            return;
        };
        if client.get(STATE) != 1 {
            return;
        }
        let reader = regs.ebx as *mut Reader;
        let (grid, strip) = match trailer::parse(left(reader)) {
            Some(grid) => (Some(grid), trailer::LEN as u32),
            None => (None, 0),
        };
        (*reader).size -= strip;
        (*(&raw mut NETCODE)).static_info(grid);
    }
}

/// `synchronize`'s context of a sender (EBP in 0x27E9F0 and 0x27EB90): +0
/// the packet sender, +4 the world core.
pub enum Sending {}
const SENDING_CORE: Field<Sending, Option<Obj<WorldCore>>> = Field::at(4);
/// `game_world_core`: its own place (20: none).
pub enum WorldCore {}
const CORE_OWNER: Field<WorldCore, u8> = Field::at(0x18FDC);

/// Whether `place` is the world's own (the senders walk the local lists,
/// which hold the own player's nodes alone).
unsafe fn own_place(regs: &Regs, place: u8) -> bool {
    unsafe {
        Obj::<Sending>::at(Va(regs.ebp))
            .and_then(|sending| sending.get(SENDING_CORE))
            .is_some_and(|core| core.get(CORE_OWNER) == place)
    }
}

/// The stock player-input sender (0x27E9F0) at its new_packet: the input on
/// its stack ([20h] rotation, [24h] actions, [28h] time, [2Ch] place,
/// [2Dh] the per-input flag, as serialize_player_input 0x3184A0 takes
/// them). ZF set: taken, the stock send skipped.
///
/// # Safety
/// The sender's frame at 0x27EB21, on the logic thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn netcode_own_input_hook(regs: &mut Regs) {
    unsafe {
        let rotation = regs.stack(0x20 / 4).to_le_bytes();
        let actions = regs.stack(0x24 / 4) & !(1 << 31);
        let time = GameTime(regs.stack(0x28 / 4));
        let [place, flag, ..] = regs.stack(0x2C / 4).to_le_bytes();
        // The flag goes as bit 31, as the stock writer puts it.
        let actions = actions | u32::from(flag != 0) << 31;
        let taken = own_place(regs, place)
            && (*(&raw mut NETCODE)).own_input(time, rotation, actions);
        regs.set_zero_flag(taken);
    }
}

/// The stock kit-choice sender (0x27EB90) at its new_packet: [1Ch] the
/// value, [20h] the time, [24h] the place. ZF set: taken.
///
/// # Safety
/// The sender's frame at 0x27ECAC, on the logic thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn netcode_own_profile_hook(regs: &mut Regs) {
    unsafe {
        let value = regs.stack(0x1C / 4) as u8;
        let time = GameTime(regs.stack(0x20 / 4));
        let place = regs.stack(0x24 / 4) as u8;
        let taken = own_place(regs, place) && (*(&raw mut NETCODE)).own_profile(time, value);
        regs.set_zero_flag(taken);
    }
}

/// After `synchronize` in the send pass (`send_match_messages`, EBX the
/// network client, AL what synchronize sent): the updates due, one packet
/// each; AL set when one went (the pass then flushes).
///
/// # Safety
/// netcode_synchronize's state, on the logic thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn netcode_send(regs: &mut Regs) {
    unsafe {
        let Some(client) = Obj::<Client>::at(Va(regs.ebx)) else {
            return;
        };
        let World::Running { now, .. } = world(client) else {
            return;
        };
        let Some(link) = client.get(LINK) else {
            return;
        };
        // Room in the packet pool for a second of updates and the game's.
        let free = link.get(POOL_CAPACITY).saturating_sub(link.get(POOL_IN_USE));
        if free < crate::machine::DUE_MAX as u32 + POOL_SPARE {
            return;
        }
        let netcode = &mut *(&raw mut NETCODE);
        let sent = netcode.due(now, &mut |update| send(link, update));
        if sent > 0 {
            regs.set_al(1);
        }
    }
}

/// One update as an 0xE2 packet, alone in its datagram.
unsafe fn send(link: Obj<Link>, update: &ClientUpdate) {
    let mut bytes = [0u8; CLIENT_UPDATE_MAX];
    let Ok(len) = update.write(&mut bytes) else {
        return;
    };
    unsafe {
        let packet = NEW_PACKET.get()(link.ptr().cast(), CLIENT_UPDATE);
        let Some(packet) = Obj::<Packet>::new(packet) else {
            return;
        };
        packet.set(PACKET_FLAGS, packet.get(PACKET_FLAGS) & !JOINED);
        patch_core::engine::engine_writer_write(
            packet.field_ptr(WRITER).cast(),
            bytes.as_ptr().cast(),
            len as u32,
        );
        ENQUEUE.get()(link.ptr().cast(), packet.ptr());
    }
}

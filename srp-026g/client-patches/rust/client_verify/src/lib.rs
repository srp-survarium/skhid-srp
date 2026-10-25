//! Patches 75, 77 and 78: a game checks what it is sent before it reads it
//! (`docs/notes/client_world_check.md`). The checks are `srp_validate`'s,
//! the match server's own code; this crate reads the game: the world core's
//! rule list into a match ([`world`]), a reader into its bytes, an index's
//! table into its bounds. Its NASM entries
//! (client_verify_026g.asm) are `RUST_HOOK`s; each hook answers with ZF
//! (set: the game goes on as before; clear: its own other path) and the
//! entry jumps accordingly.
//!
//! - 75, the world: the 0xE8 handler's two calls of
//!   game_world_core::deserialize (0x27C0D9, 0x27C111) -> `world_hook`
//!   ([`world`]).
//! - 75, the messages: the match dispatch's table jump (0x27A330) ->
//!   `message_hook`; refused, the reader is consumed and the match left
//!   (close_current_match(2), the leave button's), and the entry leaves the
//!   dispatch by its epilogue.
//! - 77: an anomaly's state index (0x31B0FF) -> `anomaly_hook`.
//! - 78: an artefact-history id's read (0x2FE65E) -> `artefact_hook`.

#![cfg_attr(target_arch = "x86", no_std)]

use patch_core::Field;
use srp_validate::messages;

pub mod world;

/// The game's buffer reader `{begin, pos, size}`.
pub enum BufferReader {}
pub const BEGIN: Field<BufferReader, u32> = Field::at(0);
pub const POS: Field<BufferReader, u32> = Field::at(4);
pub const SIZE: Field<BufferReader, u32> = Field::at(8);

/// generic_anomaly_core: its states, `std::vector<state*>` (0x31AF9C).
pub enum Anomaly {}
pub const STATES_BEGIN: Field<Anomaly, u32> = Field::at(0x68);
pub const STATES_END: Field<Anomaly, u32> = Field::at(0x6C);

/// `[player+108h]`, the object that holds the global artefact table.
pub enum ArtefactHolder {}
pub const ARTEFACTS_END: Field<ArtefactHolder, u32> = Field::at(0x1_8944);

/// Whether the game may read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Read,
    Refused,
}

impl<E> From<Result<(), E>> for Verdict {
    fn from(result: Result<(), E>) -> Self {
        match result {
            Ok(()) => Verdict::Read,
            Err(_) => Verdict::Refused,
        }
    }
}

/// A match message: its id and its payload after the id.
pub fn check_message(id: u8, payload: &[u8]) -> Verdict {
    messages::check(id, payload).into()
}

/// Whether entry `index` of 4-byte entries at `begin` is before `end`.
pub fn in_table(begin: u32, end: u32, index: u32) -> bool {
    end >= begin && index < (end - begin) / 4
}

#[cfg(target_arch = "x86")]
mod game {
    use super::*;
    use patch_core::{GameFn, Obj, Regs, Va};

    /// network_client::close_current_match (thiscall, the reason, `ret 4`).
    const CLOSE_CURRENT_MATCH: GameFn<unsafe extern "thiscall" fn(u32, u32)> =
        GameFn::at(Va(0x27A790));
    /// Its reason 2, the leave button's: 0xE0 to the match server, 31 to the
    /// lobby, the world ended (client_world_check.md section 5).
    const LEAVE_BY_INITIATOR: u32 = 2;

    /// The bytes from a reader's pos to its end.
    pub(crate) unsafe fn rest<'a>(reader: Obj<BufferReader>) -> &'a [u8] {
        unsafe {
            let (begin, pos, size) = (reader.get(BEGIN), reader.get(POS), reader.get(SIZE));
            let len = begin.wrapping_add(size).wrapping_sub(pos);
            match Obj::<u8>::at(Va(pos)) {
                Some(bytes) if len <= size => {
                    core::slice::from_raw_parts(bytes.ptr(), len as usize)
                }
                _ => &[],
            }
        }
    }

    /// The reader read to its end (as the stale-world path, 0x27C02E).
    pub(crate) unsafe fn consume(reader: Obj<BufferReader>) {
        unsafe { reader.set(POS, reader.get(BEGIN).wrapping_add(reader.get(SIZE))) }
    }

    /// The match left as the leave button leaves it.
    pub(crate) unsafe fn leave(client: u32) {
        unsafe { CLOSE_CURRENT_MATCH.get()(client, LEAVE_BY_INITIATOR) }
    }

    /// Patch 75, a message (cv_message): EBX the message reader (pos after
    /// the id), [EBP+0Ch] the id, ESI the network client.
    ///
    /// # Safety
    /// The match dispatch at 0x27A330, on the game's thread.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn message_hook(regs: &mut Regs) {
        unsafe {
            let id = *Va(regs.ebp.wrapping_add(0xC)).ptr::<u32>() as u8;
            let reader = Obj::<BufferReader>::at(Va(regs.ebx));
            let verdict = match reader {
                Some(reader) => check_message(id, rest(reader)),
                None => Verdict::Refused,
            };
            if verdict == Verdict::Refused {
                if let Some(reader) = reader {
                    consume(reader);
                }
                leave(regs.esi);
            }
            regs.set_zero_flag(verdict == Verdict::Read);
        }
    }

    /// Patch 77 (cv_anomaly): EDI the anomaly, CL the state id. In its
    /// vector: EAX the vector's begin, EDX the id, EBP the state (the
    /// site's own), ZF set; past it: ZF clear (the game's no-state path).
    ///
    /// # Safety
    /// generic_anomaly_core::deserialize at 0x31B0FF.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn anomaly_hook(regs: &mut Regs) {
        unsafe {
            let id = regs.ecx & 0xFF;
            let Some(anomaly) = Obj::<Anomaly>::at(Va(regs.edi)) else {
                return regs.set_zero_flag(false);
            };
            let (begin, end) = (anomaly.get(STATES_BEGIN), anomaly.get(STATES_END));
            let inside = in_table(begin, end, id);
            if inside {
                regs.eax = begin;
                regs.edx = id;
                regs.ebp = *Va(begin.wrapping_add(4 * id)).ptr::<u32>();
            }
            regs.set_zero_flag(inside);
        }
    }

    /// Patch 78 (cv_artefact): EAX the id, EDX the table's begin, ECX the
    /// holder. In the table: EAX the slot (the site's own read), ZF set;
    /// past it: EBP 0, ZF clear (the game's no-artefact exit).
    ///
    /// # Safety
    /// game_state_artefact_history_item::deserialize at 0x2FE65E.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn artefact_hook(regs: &mut Regs) {
        unsafe {
            let inside = match Obj::<ArtefactHolder>::at(Va(regs.ecx)) {
                Some(holder) => in_table(regs.edx, holder.get(ARTEFACTS_END), regs.eax),
                None => false,
            };
            if inside {
                regs.eax = *Va(regs.edx.wrapping_add(4 * regs.eax)).ptr::<u32>();
            } else {
                regs.ebp = 0;
            }
            regs.set_zero_flag(inside);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_and_tables() {
        assert_eq!(check_message(0xE6, &[1, 2, 3, 4]), Verdict::Read);
        assert_eq!(check_message(0xE6, &[1, 2, 3]), Verdict::Refused);
        assert_eq!(check_message(0xED, &[0; 5]), Verdict::Refused);
        assert!(in_table(0x1000, 0x100C, 2));
        assert!(!in_table(0x1000, 0x100C, 3));
        assert!(!in_table(0x1000, 0x1000, 0));
        assert!(!in_table(0x100C, 0x1000, 0)); // an inverted vector holds nothing
    }
}

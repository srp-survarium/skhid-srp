//! The one "busy" rule: an account takes part in at most one of a custom room, a PvP search, a PvE search, an online match
//! and a local match (training, tutorial, shooting range). Every way into one of them (`2F` queue, `5F` PvE play, `62` rooms,
//! `5B` local Play) asks [`reason`] first and answers with the refusal of its own channel (the room texts are the game's own,
//! the PvE search uses `deny::GENERIC`, the PvP search and local Play fall back to the idle state `85 00`).
//! CHOICE: no source says how the original server ordered these; refusing a second activity is the only consistent reading
//! of the client's own text "Player is already in a match or looking for a match".

use accounts::AccountId;

use crate::dispatch::Ctx;

/// What an account is already doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Busy {
    Room,
    PvpSearch,
    PveSearch,
    OnlineMatch,
    LocalMatch,
}

/// Why `a` cannot start or join another activity now. `local_match` = the account's own connection is in a local match
/// (only the pressing peer's state is known; pass false for other accounts).
pub fn reason(cx: &Ctx, a: AccountId, local_match: bool) -> Option<Busy> {
    let reg = &cx.registry;
    if reg.rooms.room_of(a).is_some() {
        Some(Busy::Room)
    } else if reg.queue.ticket_of(a).is_some() {
        Some(Busy::PvpSearch)
    } else if reg.pve.ticket_of(a).is_some() {
        Some(Busy::PveSearch)
    } else if reg.online.match_of(a).is_some() {
        Some(Busy::OnlineMatch)
    } else if local_match {
        Some(Busy::LocalMatch)
    } else {
        None
    }
}

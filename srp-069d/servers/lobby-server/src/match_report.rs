//! What the client reports about a local match: the `53` event flush, nothing else (docs/notes/match-stats.md).
//!
//! The flush is the client's marker tracker (`{u16 id, u16 junk, u32 value, u32 time}`, at most 50 rows per flush, the
//! `time` is the client's uptime in seconds). Of the ids the client can push (the others are lobby / UI / tutorial
//! markers, PROVEN) only 5 / 6 (and 0x3D / 0x3E in single player mode) come from the match: my team won / lost
//! (PROVEN, 0x2001168C6..0x200116988). No id carries kills, deaths, assists, score or kill details, so those facts stay
//! `None` for a local match (PROVEN: real flushes hold only ids 1, 2, 5, 6).

use protocol::c2s::Event53;

/// Win marker ids: 5 (my team won), 0x3D (single player rule type 2, LIKELY the same meaning).
pub const WON: [u16; 2] = [5, 0x3D];
/// Loss marker ids: 6, 0x3E.
pub const LOST: [u16; 2] = [6, 0x3E];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LocalReport {
    /// a win marker is in the flush
    pub won: bool,
    /// a win or a loss marker is in the flush; false = the player left early (no marker, PROVEN)
    pub finished: bool,
    /// the ids of the whole flush in order, for the log
    pub ids: Vec<u16>,
}

/// Read a local match's flush. Every row counts (a flush holds at most 50 rows).
pub fn local_report(ev: &[Event53]) -> LocalReport {
    let ids: Vec<u16> = ev.iter().map(|e| e.id).collect();
    let won = ids.iter().any(|i| WON.contains(i));
    let finished = won || ids.iter().any(|i| LOST.contains(i));
    LocalReport { won, finished, ids }
}

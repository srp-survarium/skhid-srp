//! Achievements on the wire: push `B9` and the `83 55` replies.
//!
//! PROVEN (exe, AS3 of the lobby movie):
//! * `B9` (reader 0x200194F40): `u32 N, N x {u16 id, u32 value, u8 state, u8 lo, u8 hi}`. `value` is the progress
//!   number, `state` is `current_state` = the NUMBER OF STATES REACHED (0 = none, `states.len()` = all), and for every
//!   `i` in `lo..hi` one "state reached" entry `{id, i, 1}` is queued for the Flash message list (0x2001C70D0,
//!   `root.add_important_message`); the total of those entries is at most 200 (assert 0x1A4 + int 3). LIKELY: `i` is
//!   the 0-based state index (the state that was just reached).
//! * `83 55 02` (reader 0x200194D78): `u32 n, n x {u16 id, u8 state, u32 value}` -> the same setter (0x200195660)
//!   without popups.
//! * `83 55 03` (reader 0x200194CE1): `u8 n, n x {u8 category, u16 score}` -> 0x2001C9B30 (the category bars).
//!
//! Why the byte is the reached-state count (not the achievement `type`):
//! * Both readers store it as `current_state` of the Flash achievement object (0x2001C9985): `achieved` = byte != 0,
//!   the description shown is that of state[byte] while byte < the state count (0x2001C9A11..0x2001C9A2D).
//! * The descriptor's +0x0C is the state count (loader 0x20034CA80, 0x20034CC34); the check "byte == state count"
//!   at 0x2001956F8 runs only with a Steam sign-in (`-steam`) and gates only `ISteamUserStats::SetAchievement`.
//! * The window (AS3 `AchieveWindowLoader.createAchievement`) shows states[n-1] as the last reached and states[n] as
//!   the next; the earned score is the sum of the reached states' scores; the list sorts on `current_state`.

use crate::wire::{Decode, Encode, Reader, Result, Writer};
use crate::Error;

/// Most "state reached" entries one B9 may carry (client buffer of 0x640 bytes, 8 bytes each).
pub const MAX_REACHED: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AchEntry {
    pub id: u16,
    pub value: u32,
    /// `current_state`: the number of states reached (0 = none)
    pub state: u8,
    /// first newly reached state (index)
    pub lo: u8,
    /// one past the last newly reached state; `lo == hi` = no popup
    pub hi: u8,
}

/// `B9`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AchievementsB9 {
    pub entries: Vec<AchEntry>,
}

impl Encode for AchievementsB9 {
    fn encode(&self, w: &mut Writer) {
        let reached: usize = self.entries.iter().map(|e| usize::from(e.hi.saturating_sub(e.lo))).sum();
        if reached > MAX_REACHED {
            w.fail(Error::Invalid("B9: more than 200 reached states"));
        }
        w.count_u32(self.entries.len());
        for e in &self.entries {
            w.u16(e.id).u32(e.value).u8(e.state).u8(e.lo).u8(e.hi);
        }
    }
}

impl Decode for AchievementsB9 {
    fn decode(r: &mut Reader) -> Result<Self> {
        let n0 = r.u32()? as usize;
        let n = r.check_count(n0, 9)?;
        let mut entries = Vec::with_capacity(n);
        for _ in 0..n {
            entries.push(AchEntry { id: r.u16()?, value: r.u32()?, state: r.u8()?, lo: r.u8()?, hi: r.u8()? });
        }
        Ok(AchievementsB9 { entries })
    }
}

/// `83 55 02` data after the request id: the progress list `02 u32 n, n x {u16 id, u8 state, u32 value}`.
pub fn progress_reply(entries: &[AchEntry]) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.u8(2).count_u32(entries.len());
    for e in entries {
        w.u16(e.id).u8(e.state).u32(e.value);
    }
    w.finish()
}

/// `83 55 03` data after the request id: `03 u8 n, n x {u8 category, u16 score}`.
pub fn category_reply(scores: &[(u8, u16)]) -> Result<Vec<u8>> {
    let mut w = Writer::new();
    w.u8(3).count_u8(scores.len(), "83 55 03 categories");
    for &(c, s) in scores {
        w.u8(c).u16(s);
    }
    w.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn b9_layout_and_round_trip() {
        let m = AchievementsB9 { entries: vec![AchEntry { id: 201, value: 12, state: 2, lo: 0, hi: 2 }] };
        let b = m.to_bytes().unwrap();
        assert_eq!(b, [1, 0, 0, 0, 201, 0, 12, 0, 0, 0, 2, 0, 2]);
        assert_eq!(AchievementsB9::from_bytes(&b).unwrap(), m);
    }

    #[test]
    fn b9_refuses_more_than_200_reached_states() {
        let e = AchEntry { id: 1, value: 0, state: 0, lo: 0, hi: 201 };
        assert!(AchievementsB9 { entries: vec![e] }.to_bytes().is_err());
    }

    #[test]
    fn reply_layouts() {
        assert_eq!(category_reply(&[(2, 300)]).unwrap(), [3, 1, 2, 0x2C, 1]);
        let e = AchEntry { id: 0x0102, value: 5, state: 7, lo: 0, hi: 0 };
        assert_eq!(progress_reply(&[e]).unwrap(), [2, 1, 0, 0, 0, 2, 1, 7, 5, 0, 0, 0]);
    }
}

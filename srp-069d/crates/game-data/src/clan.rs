//! Clan progression: experience, levels and what a member's match adds. Notes: `docs/notes/social.md` section 3.
//!
//! What is PROVEN (clan window, `ManageClanWindow.setClanProgress`, 0x2802xx `root.set_clan_progress`): the client shows
//! a level number and a bar with `minimum`, `maximum` and `value` and the tooltip "value / maximum"; nothing else about
//! clan levels exists on the client side. Neither the game data (`gameplay/clans/clan_name.options` only holds the name
//! rules) nor the official posts give a clan experience source or a threshold table, so BOTH are CHOICES of this
//! server: the numbers below are authored, named as such and used consistently (the bar the client draws is the real
//! stored experience, not a constant).

use crate::facts::MatchFacts;

/// Highest clan level (CHOICE).
pub const MAX_LEVEL: u8 = 20;

/// Experience at which `level` starts: 500 x (level - 1) x level, i.e. 0, 1000, 3000, 6000, 10000 ... (CHOICE: each
/// level asks 1000 more than the one before).
pub fn level_start(level: u8) -> u32 {
    let l = level.clamp(1, MAX_LEVEL) as u32;
    500 * (l - 1) * l
}

/// The window bar of a clan with `xp` experience.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub level: u8,
    /// bar minimum (always 0: the bar counts inside the level)
    pub min: u32,
    /// experience the level needs (the top level shows a full bar of its own span)
    pub max: u32,
    /// experience inside the level
    pub current: u32,
}

/// Level and bar for a clan experience.
pub fn progress(xp: u32) -> Progress {
    let mut level = 1;
    while level < MAX_LEVEL && xp >= level_start(level + 1) {
        level += 1;
    }
    let start = level_start(level);
    let span = if level < MAX_LEVEL { level_start(level + 1) - start } else { level_start(MAX_LEVEL) - level_start(MAX_LEVEL - 1) };
    Progress { level, min: 0, max: span, current: (xp - start).min(span) }
}

/// Experience a member's finished match adds to his clan (CHOICE): 20 for a win, 10 for a loss, nothing for a leaver.
/// Only a match the server observed counts (`match_type` known: a random or ranked online match); a local training
/// match and a match without facts give `None`.
pub fn match_xp(f: &MatchFacts) -> Option<u32> {
    f.match_type?;
    if !f.finished {
        return None;
    }
    Some(if f.won { 20 } else { 10 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_the_threshold_table() {
        assert_eq!((level_start(1), level_start(2), level_start(3), level_start(4)), (0, 1000, 3000, 6000));
        assert_eq!(progress(0), Progress { level: 1, min: 0, max: 1000, current: 0 });
        assert_eq!(progress(999).level, 1);
        assert_eq!(progress(1000), Progress { level: 2, min: 0, max: 2000, current: 0 });
        assert_eq!(progress(2500), Progress { level: 2, min: 0, max: 2000, current: 1500 });
        let top = level_start(MAX_LEVEL);
        assert_eq!(progress(top).level, MAX_LEVEL);
        assert_eq!(progress(u32::MAX).level, MAX_LEVEL);
        assert_eq!(progress(u32::MAX).current, progress(u32::MAX).max, "the top level shows a full bar");
    }

    #[test]
    fn only_observed_finished_matches_pay_experience() {
        let mut f = MatchFacts::local(4, 1, true, true, 1);
        assert_eq!(match_xp(&f), None, "a local training match pays nothing");
        f.match_type = Some(1);
        assert_eq!(match_xp(&f), Some(20));
        f.won = false;
        assert_eq!(match_xp(&f), Some(10));
        f.finished = false;
        assert_eq!(match_xp(&f), None, "leavers get nothing");
    }
}

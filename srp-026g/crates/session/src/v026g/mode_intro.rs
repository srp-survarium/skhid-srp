//! An account's matches of each mode the hud introduces with a notice
//! (`docs/notes/mode_introductions.md`): its first three matches of a mode
//! show the mode's notice, every later one does not (the owner, 2026-11-03).
//! Each mode is counted apart. The lobby reads the count when it makes a
//! match or takes a late joiner ([`Introductions::notice`], the place's
//! [`Notice`] in the match's options), and counts the match once the match
//! server has it ([`Introductions::count`]). Kept with the account in
//! `g26_mode_introductions` (migration 13, `persist::mode_intro`).

use std::collections::BTreeMap;

use survarium::v026g::maps::{GameMode, ModeNotice};
use survarium::v026g::match_msgs::static_info::Notice;

/// The matches of each mode the account was put in, up to three. A mode
/// with no row has had none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Introductions(BTreeMap<ModeNotice, Seen>);

impl Introductions {
    /// The notice of the account's place in a match of `mode`: shown while
    /// it has had fewer than three of the mode; none in a mode without one.
    pub fn notice(&self, mode: GameMode) -> Notice {
        match mode.notice() {
            Some(mode) if self.seen(mode) != Seen::Thrice => Notice::Shown,
            Some(_) | None => Notice::Skipped,
        }
    }

    /// The account was put in a match of `mode` (made with it, or taken in
    /// late): one more of it, up to three.
    pub fn count(&mut self, mode: GameMode) {
        if let Some(mode) = mode.notice() {
            let seen = self.0.entry(mode).or_default();
            *seen = seen.next();
        }
    }

    fn seen(&self, mode: ModeNotice) -> Seen {
        self.0.get(&mode).copied().unwrap_or_default()
    }

    /// Its rows: each mode with the matches it had of it (1 to 3).
    pub fn rows(&self) -> impl Iterator<Item = (ModeNotice, u8)> + '_ {
        self.0.iter().map(|(&mode, &seen)| (mode, seen.count()))
    }

    /// A row read back; `None` for a count that is no [`Seen`].
    pub fn insert(&mut self, mode: ModeNotice, count: u8) -> Option<()> {
        self.0.insert(mode, Seen::of_count(count)?);
        Some(())
    }
}

/// The matches of one mode an account had, as far as its notice cares.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Seen {
    #[default]
    None,
    Once,
    Twice,
    /// Three or more: the notice is not shown any more.
    Thrice,
}

impl Seen {
    fn next(self) -> Self {
        match self {
            Self::None => Self::Once,
            Self::Once => Self::Twice,
            Self::Twice | Self::Thrice => Self::Thrice,
        }
    }

    /// Its count in `g26_mode_introductions.shown` (0 to 3).
    fn count(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Once => 1,
            Self::Twice => 2,
            Self::Thrice => 3,
        }
    }

    fn of_count(count: u8) -> Option<Self> {
        match count {
            0 => Some(Self::None),
            1 => Some(Self::Once),
            2 => Some(Self::Twice),
            3 => Some(Self::Thrice),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first three matches of a mode show its notice, the fourth and
    /// later do not; each mode counts apart, and a mode with no notice
    /// neither shows one nor is counted.
    #[test]
    fn the_first_three_matches_of_each_mode_show_its_notice() {
        let mut seen = Introductions::default();
        for _ in 0..3 {
            assert_eq!(seen.notice(GameMode::Blackout), Notice::Shown);
            seen.count(GameMode::Blackout);
        }
        assert_eq!(seen.notice(GameMode::Blackout), Notice::Skipped);
        seen.count(GameMode::Blackout);
        assert_eq!(seen.notice(GameMode::Blackout), Notice::Skipped);
        for _ in 0..3 {
            assert_eq!(seen.notice(GameMode::FreezeTag), Notice::Shown, "counted apart");
            seen.count(GameMode::FreezeTag);
        }
        assert_eq!(seen.notice(GameMode::FreezeTag), Notice::Skipped);
        seen.count(GameMode::TeamDeathmatch);
        assert_eq!(seen.notice(GameMode::TeamDeathmatch), Notice::Skipped, "no notice");
        assert_eq!(
            seen.rows().collect::<Vec<_>>(),
            [(ModeNotice::Blackout, 3), (ModeNotice::FreezeTag, 3)]
        );
        for count in 0..=3 {
            let mut read = Introductions::default();
            assert_eq!(read.insert(ModeNotice::FreezeTag, count), Some(()));
            assert_eq!(read.rows().next(), Some((ModeNotice::FreezeTag, count)));
        }
        assert_eq!(Introductions::default().insert(ModeNotice::Blackout, 4), None);
    }
}

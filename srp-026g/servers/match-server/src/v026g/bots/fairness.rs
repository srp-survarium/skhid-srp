//! The bench's fairness count: how often a decision read an enemy's position that its team's own senses had not given in the last
//! 15 s (`traces-20261028/BENCH-DECISIONS.md` §2D: "decisions about an enemy who was not seen or heard within 15 s: 0 for new
//! code; count it wherever a new decision reads an enemy position").
//!
//! [`read`] is the helper: a play, a threat job, a grenade or a trap that takes an enemy's position from a [`Track`] calls it with
//! the bot's team where it reads one. It counts only (in test builds; a live build has an empty function), by the bot's team and by
//! the basis of the position. The owner tally prints the counts (`tests/tallies/owner/metrics.rs`).
//!
//! A read is **fresh** when the track's source is the team's own senses (sight, hearing, a teammate's word) and was sensed within
//! [`UNSEEN_MS`]. Else it rests on an old sense (**stale**), on what a human's HUD shows (the kill feed gives a cone, not a point:
//! **hud**) or on a cheat (**cheat**).

use super::know::{Source, Track};

/// A track not sensed by the team's own senses within this long is not a basis for a decision, ms.
pub const UNSEEN_MS: u32 = 15_000;

/// What a position read rested on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Basis {
    Fresh,
    Stale,
    Hud,
    Cheat,
}

/// The kinds of basis.
pub const BASES: usize = 4;

impl Basis {
    /// The basis of `track`'s position at `now` (the clock the track was stamped on).
    pub fn of(track: &Track, now: u32) -> Self {
        match track.source {
            source if source.own() && now.saturating_sub(track.time) <= UNSEEN_MS => Self::Fresh,
            Source::Hud => Self::Hud,
            Source::Cheat(_) => Self::Cheat,
            _ => Self::Stale,
        }
    }
}

/// Where a decision read an enemy's position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    /// Where one enemy is, as far as the bot may know (`Ctx::known_place`).
    Place,
    /// The known enemies near a point (`Ctx::enemies_near`).
    Near,
    /// The known threats, nearest first (`Ctx::threats`).
    Threats,
}

/// The sites.
pub const SITES: usize = 3;

/// The counts, by team, site and basis.
pub type Counts = [[[u64; BASES]; SITES]; 2];

#[cfg(test)]
thread_local! {
    static COUNTS: std::cell::Cell<Counts> = const { std::cell::Cell::new([[[0; BASES]; SITES]; 2]) };
}

/// A decision of a bot of `team` read `track`'s position at `now`.
#[cfg(test)]
#[inline(always)]
pub fn read(team: u8, site: Site, track: &Track, now: u32) {
    let basis = Basis::of(track, now);
    COUNTS.with(|c| {
        let mut counts = c.get();
        counts[usize::from(team & 1)][site as usize][basis as usize] += 1;
        c.set(counts);
    });
}

#[cfg(not(test))]
#[inline(always)]
pub fn read(_: u8, _: Site, _: &Track, _: u32) {}

/// The counts so far on this thread.
#[cfg(test)]
pub fn counts() -> Counts {
    COUNTS.with(std::cell::Cell::get)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mapdata::Vec3;

    fn track(source: Source, time: u32) -> Track {
        Track {
            who: 3,
            at: Vec3::new(0.0, 0.0, 0.0),
            error_m: 0.0,
            time,
            source,
            sight_since: None,
            notice_ms: 0,
            own: None,
            missed: None,
            held_until: 0,
        }
    }

    /// Own senses within 15 s are fresh; an old sense, the HUD and a cheat are not.
    #[test]
    fn a_read_is_fresh_only_on_the_teams_own_recent_senses() {
        assert_eq!(Basis::of(&track(Source::Seen, 90_000), 100_000), Basis::Fresh);
        assert_eq!(Basis::of(&track(Source::Heard, 85_000), 100_000), Basis::Fresh);
        assert_eq!(Basis::of(&track(Source::Told { by: 1 }, 84_999), 100_000), Basis::Stale);
        assert_eq!(Basis::of(&track(Source::Hud, 99_000), 100_000), Basis::Hud);
        assert_eq!(Basis::of(&track(Source::Cheat(super::super::know::CheatKind::Radar), 99_000), 100_000), Basis::Cheat);
    }

    /// A read is counted for the bot's team, at the site, on its basis.
    #[test]
    fn a_read_is_counted_by_team_site_and_basis() {
        let before = counts();
        read(1, Site::Near, &track(Source::Hud, 0), 1_000);
        let after = counts();
        assert_eq!(after[1][Site::Near as usize][Basis::Hud as usize], before[1][Site::Near as usize][Basis::Hud as usize] + 1);
        assert_eq!(after[0], before[0]);
    }
}

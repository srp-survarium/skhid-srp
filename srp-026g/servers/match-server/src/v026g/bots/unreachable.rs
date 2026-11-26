//! [bot traps] Goals a bot's own search proved it has no way to from where it stands, and the cool-down they get.
//!
//! A route search that runs out of floors (or one the regions rule out at once) is exhaustive: every floor the body may walk
//! on from its start was tried. Asked again from the same part of the grid, it floods the same floors to the same answer.
//! Before, such a goal was learned for the plays only when the failure was a proof for the team (`Stuck::no_way_is_proof`:
//! none of the body's own shut drops or match-long shut landings in the way); otherwise the play kept its goal and the
//! drive searched again every [`super::search::NO_WAY_RETRY_MS`].
//!
//! Measured (2026-11-25, the battery cells of Rudnya, Vostok and Cologne, seeds 1-3, at ce6f992df and on the S5b arm
//! 8b831edbd, `private/pvb-343-check/bot-traps-20261125`):
//! - **Vostok seed 3, S5b arm.** One bot's Collect of a spawner whose floor a zone refused failed 61 times in 65 s, 16.9 M
//!   floors of the cell's 30.4 M.
//! - **Vostok seed 2.** Two carriers asked their way home every 0.5 s for 22 s and 44 s.
//! - **Most failures were no proof.** Of 480 exhaustive failures, 290 came within a minute of the same bot's last one for
//!   the same goal.
//! - **The cool-down.** One of [`COOL_DOWN_MS`] would have skipped 287 of those 290 (20.7 M of their 21.4 M floors), and
//!   delayed 5 routes, each found within 2 s of the failure. Longer cool-downs skip almost nothing more: 30 s skips 290.
//!
//! So each exhaustive failure keeps its goal for this bot alone (never its team: a mate's floors are another matter) for
//! [`COOL_DOWN_MS`], while the bot stays in the part of the grid it failed from ([`Unreachable::from`]: the regions its
//! start's region reaches by the grid's links). The plays read it with the bot's own no-way goals
//! ([`UnreachableGoals::no_way_view`]): a play's goal there is left (`think`'s `goal_shut`), and a carrier's way home counts
//! as failing (`collect`'s strand).

use std::sync::Arc;

use mapdata::Vec3;

use super::drive::REPLAN_MOVED_M;
use super::search::NO_WAY_KEEP_MS;

/// How long a goal an exhaustive search failed to reach is left by the bot, ms (the measure above).
pub const COOL_DOWN_MS: u32 = 5_000;

/// Goals kept at most at once: the oldest goes first.
const KEPT: usize = 8;

/// How far from the feet the bot's floor is looked for, to know where it stands, m.
pub const STAND_M: f32 = 1.5;

/// One goal on cool-down.
#[derive(Clone, Debug)]
pub struct Unreachable {
    pub goal: Vec3,
    pub until: u32,
    /// The regions reached by the grid's links from the region of the floor the search started from, a bit a region
    /// (`Tactics::regions_reached`): a bot that stands in a region outside them is elsewhere, and asks again.
    pub from: Arc<Vec<u64>>,
}

/// Why an entry is no longer kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    /// The cool-down ran out.
    Expired,
    /// The bot stands where the failed search could not have walked from.
    Elsewhere,
}

/// A bot's goals on cool-down.
#[derive(Clone, Debug, Default)]
pub struct UnreachableGoals(Vec<Unreachable>);

fn has(set: &[u64], region: u32) -> bool {
    set.get(region as usize / 64).is_some_and(|w| w >> (region % 64) & 1 == 1)
}

impl UnreachableGoals {
    /// An exhaustive failure to reach `goal` from the regions `from` at `now`: kept for [`COOL_DOWN_MS`] (a goal near one
    /// kept already takes its place).
    pub fn note(&mut self, goal: Vec3, from: Arc<Vec<u64>>, now: u32) {
        self.0.retain(|u| u.goal.distance_xz(goal) > REPLAN_MOVED_M);
        if self.0.len() >= KEPT {
            self.0.remove(0);
        }
        self.0.push(Unreachable { goal, until: now + COOL_DOWN_MS, from });
    }

    /// Drops the entries no longer kept for a bot standing in `region` (none: on no floor, which keeps them) at `now`: each
    /// with why, for the log.
    pub fn release(&mut self, region: Option<u32>, now: u32) -> Vec<(Vec3, Release)> {
        let mut gone = Vec::new();
        self.0.retain(|u| {
            let why = if now >= u.until {
                Some(Release::Expired)
            } else if region.is_some_and(|r| !has(&u.from, r)) {
                Some(Release::Elsewhere)
            } else {
                None
            };
            if let Some(why) = why {
                gone.push((u.goal, why));
            }
            why.is_none()
        });
        gone
    }

    /// Whether a goal near `at` is kept now ([`Self::release`] made first).
    pub fn holds(&self, at: Vec3, now: u32) -> bool {
        self.0.iter().any(|u| now < u.until && u.goal.distance_xz(at) <= REPLAN_MOVED_M)
    }

    /// Until when a goal near `at` is kept, if it is.
    pub fn until(&self, at: Vec3, now: u32) -> Option<u32> {
        self.0.iter().filter(|u| now < u.until && u.goal.distance_xz(at) <= REPLAN_MOVED_M).map(|u| u.until).max()
    }

    /// The kept goals as the plays' no-way marks (`BotFacts::no_way_to`): each as freshly failing, a mark of
    /// [`NO_WAY_KEEP_MS`] from now (`goals::no_way_failing` reads a mark that young as renewed by a failing search: the
    /// cool-down stands in for the searches it spares).
    pub fn no_way_view(&self, now: u32) -> impl Iterator<Item = (Vec3, u32)> + '_ {
        self.0.iter().filter(move |u| now < u.until).map(move |u| (u.goal, now + NO_WAY_KEEP_MS))
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The event line of a goal put on cool-down: its failed search started in region `region`.
pub fn noted_line(goal: Vec3, region: u32, end: super::path::SearchEnd, now: u32) -> String {
    format!(
        "unreachable: goal ({:.1}, {:.1}, {:.1}) on cool-down {} s from region {region} (exhaustive search failed: {}) at {:.1} s",
        goal.x,
        goal.y,
        goal.z,
        COOL_DOWN_MS / 1_000,
        end.word(),
        f64::from(now) / 1_000.0
    )
}

/// The event line of a goal's cool-down ended.
pub fn released_line(goal: Vec3, why: Release, now: u32) -> String {
    let why = match why {
        Release::Expired => "cool-down ended",
        Release::Elsewhere => "bot left the regions the search started in",
    };
    format!("unreachable: goal ({:.1}, {:.1}, {:.1}) tried again: {why} at {:.1} s", goal.x, goal.y, goal.z, f64::from(now) / 1_000.0)
}

/// Notes for `bot` an exhaustive failure of its search from floor `from` to `goal` (how it ended: `end`), with its line.
pub(super) fn note(
    bot: &mut super::Bot,
    tactics: &super::Tactics,
    goal: Vec3,
    from: mapdata::NodeId,
    end: super::path::SearchEnd,
    time: u32,
) {
    bot.unreachable.note(goal, tactics.regions_reached(from), time);
    let line = noted_line(goal, tactics.regions().of(from), end, time);
    bot.nav_log.push(super::nav_log::Kind::Unreachable, time, line);
}

/// After the drive's tick: a goal it found no way to this tick (`Bot::no_way` made now) whose proof was exhaustive is noted from
/// the floor under the feet (none: not noted). The proof (`end`, `path::end_since_mark`) is the plain search's end (a crossing's
/// search after it is put back, `path::EndsKept`), or the escape's [`super::path::SearchEnd::NoExit`].
pub(super) fn after_drive(
    bot: &mut super::Bot,
    me: &super::PlaceView,
    tactics: &super::Tactics,
    end: Option<super::path::SearchEnd>,
    time: u32,
) {
    let Some((goal, until)) = bot.no_way else {
        return;
    };
    let Some(end) = end.filter(|end| end.exhaustive()) else {
        return;
    };
    if until != time + super::search::NO_WAY_RETRY_MS {
        return;
    }
    if let Some(from) = tactics.world.grid.locate(me.feet, STAND_M) {
        note(bot, tactics, goal, from, end, time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::plays::goals::{no_way_failing, no_way_near};

    fn regions(set: &[u32]) -> Arc<Vec<u64>> {
        let mut words = vec![0u64; 4];
        for &r in set {
            words[r as usize / 64] |= 1 << (r % 64);
        }
        Arc::new(words)
    }

    /// A goal noted is kept for the cool-down while the bot stays in the regions it failed from; it goes when the cool-down
    /// ends or the bot stands elsewhere, each with its reason; a goal near it replaces it; the plays read it as failing.
    #[test]
    fn an_unreachable_goal_is_kept_for_the_cool_down_in_its_part_of_the_grid() {
        let goal = Vec3::new(10.0, 0.0, 10.0);
        let mut kept = UnreachableGoals::default();
        kept.note(goal, regions(&[3, 4, 70]), 1_000);
        assert!(kept.release(Some(4), 2_000).is_empty());
        assert!(kept.holds(goal, 2_000) && kept.holds(goal + Vec3::new(1.0, 0.0, 0.0), 2_000));
        assert_eq!(kept.until(goal, 2_000), Some(1_000 + COOL_DOWN_MS));
        assert!(!kept.holds(Vec3::new(30.0, 0.0, 10.0), 2_000));
        // As the plays read it: a fresh failure, renewed.
        let view: Vec<_> = kept.no_way_view(2_000).collect();
        assert!(no_way_near(&view, goal, 2_000) && no_way_failing(&view, goal, 2_000));
        // A region of the set's second word, and no floor: kept.
        assert!(kept.release(Some(70), 3_000).is_empty());
        assert!(kept.release(None, 3_000).is_empty());
        // Elsewhere: released.
        assert_eq!(kept.release(Some(5), 3_000), vec![(goal, Release::Elsewhere)]);
        assert!(!kept.holds(goal, 3_000) && kept.is_empty());
        // The cool-down's end: released.
        kept.note(goal, regions(&[3]), 10_000);
        assert!(kept.holds(goal, 10_000 + COOL_DOWN_MS - 1));
        assert_eq!(kept.release(Some(3), 10_000 + COOL_DOWN_MS), vec![(goal, Release::Expired)]);
        assert_eq!(kept.no_way_view(10_000 + COOL_DOWN_MS).count(), 0);
        // One entry a goal: a second failure near it renews it.
        kept.note(goal, regions(&[3]), 20_000);
        kept.note(goal + Vec3::new(0.5, 0.0, 0.0), regions(&[3]), 21_000);
        assert_eq!(kept.len(), 1);
        assert!(kept.holds(goal, 21_000 + COOL_DOWN_MS - 1));
        // At most KEPT goals: the oldest goes.
        for i in 0..KEPT as u32 + 2 {
            kept.note(Vec3::new(100.0 + 10.0 * i as f32, 0.0, 0.0), regions(&[3]), 30_000);
        }
        assert_eq!(kept.len(), KEPT);
        assert!(!kept.holds(goal, 30_000));
    }
}

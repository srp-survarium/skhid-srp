//! The match's dead links: a jump or drop of the grid that bodies failed [`DeadLinksTuning::after`] times, whichever bots
//! (`unstick.dead_links`, owner-14 report row 3).
//!
//! A bot that fails a link remembers it alone and for a while: a jump the motor gave up for `motor::TRY_KEPT_S`, a drop it
//! was held at the top of for `unstick.drop_shut.kept_ms` (45 s, [`super::stuck::Stuck::failed_jump`],
//! [`super::stuck::Stuck::failed_drop`]). The owner's 14 matches (2026-11-11, PROVEN from the recordings and the server's
//! logs): one drop off Vector Laboratory's raised spawn platform failed 95 times out of 95, by 18 different bots, in three
//! matches; each bot learned it again alone. A link no body of this match got through twice is a fact about the map and the
//! client, not about one body: here it is shut to every search of every bot (of a team with the switch on) for the rest of
//! the match, and written to the log as the map's dead-link list for the grid builder's owner.
//!
//! Its safety: a search that finds no way with the dead links shut searches once more without them
//! (`unstick.drop_shut_last_resort`, [`super::stuck::without_drops`]): where a dead link is the only way, the bot takes it
//! again, as it did before.

use mapdata::Vec3;

use super::stuck::{LANDING_SHUT_M, Shut, ShutKind};
use super::tuning::DeadLinksTuning;

/// A link a bot failed: what it was, where its take-off and its landing are ([`super::stuck::Stuck::tell_failed_links`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FailedLink {
    pub kind: ShutKind,
    pub take_off: Vec3,
    pub landing: Vec3,
}

impl FailedLink {
    /// Whether `other` is the same link: the same kind, the landings within [`LANDING_SHUT_M`] across and
    /// [`TAKE_OFF_UP_M`] up or down (as a body's own shut takes a landing, `Shut::covers`), the take-offs within
    /// [`TAKE_OFF_M`] (a ledge's drops land side by side; the same landing from another top is another link).
    fn same(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.landing.distance_xz(other.landing) < LANDING_SHUT_M
            && (self.landing.y - other.landing.y).abs() < TAKE_OFF_UP_M
            && self.take_off.distance_xz(other.take_off) < TAKE_OFF_M
            && (self.take_off.y - other.take_off.y).abs() < TAKE_OFF_UP_M
    }
}

/// The start of the log's dead-link line (`commander.rs`): the log's, not a decision (the bench's decision digest leaves it
/// out, as `nav_log`'s lines; it is written with the switch off too).
pub const PREFIX: &str = "dead link on map ";

/// Two take-offs this near across are one link's, metres: a stuck spot's reach (`stuck::SPOT_M`) is where the body was
/// held, and the motor presses a jump up to a cell from its waypoint.
pub const TAKE_OFF_M: f32 = 1.0;

/// Two take-offs or landings this near up or down are one link's, metres: under a step.
pub const TAKE_OFF_UP_M: f32 = 0.3;

/// The links the match keeps, at most: the oldest live link is never dropped, a new one past this is not kept. The
/// owner's 14 matches had 10 links with 3+ tries and none through over 8 maps (`owner-14/nav-findings.md` §2b).
pub const DEAD_LINKS_MAX: usize = 64;

/// A link failed in this match, and by whom.
#[derive(Clone, Debug)]
struct Link {
    link: FailedLink,
    /// The places of the bots that failed it, one entry a failure.
    by: Vec<usize>,
    /// When it was shut for the match, ms; none: not yet.
    shut_at: Option<u32>,
}

/// The match's links failed, for every bot of the match ([`Self::learn`], [`Self::shut`]).
#[derive(Clone, Debug, Default)]
pub struct DeadLinks {
    links: Vec<Link>,
}

/// A link just shut for the match, for the log ([`DeadLinks::learn`]).
#[derive(Clone, Debug, PartialEq)]
pub struct NewlyDead {
    pub link: FailedLink,
    pub by: Vec<usize>,
}

impl DeadLinks {
    /// The links bot `place` failed since it last told ([`super::stuck::Stuck::tell_failed_links`]) at `time`: the ones
    /// failed `tuning.after` times now are shut for the match, and returned.
    pub fn learn(&mut self, time: u32, place: usize, failed: &[FailedLink], tuning: &DeadLinksTuning) -> Vec<NewlyDead> {
        let mut dead = Vec::new();
        for link in failed {
            let known = match self.links.iter_mut().position(|known| known.link.same(link)) {
                Some(i) => &mut self.links[i],
                None if self.links.len() < DEAD_LINKS_MAX => {
                    self.links.push(Link { link: *link, by: Vec::new(), shut_at: None });
                    self.links.last_mut().expect("just pushed")
                }
                None => continue,
            };
            known.by.push(place);
            #[cfg(test)]
            tally::count(tally::TOLD);
            if known.shut_at.is_none() && known.by.len() >= usize::from(tuning.after) {
                known.shut_at = Some(time);
                #[cfg(test)]
                tally::count(tally::SHUT);
                dead.push(NewlyDead { link: known.link, by: known.by.clone() });
            }
        }
        dead
    }

    /// The landings of the links shut for the match, for every search of a bot ([`super::stuck::Stuck::share_dead`]).
    pub fn shut(&self) -> Vec<Shut> {
        self.links
            .iter()
            .filter(|link| link.shut_at.is_some())
            .map(|link| Shut::for_match(link.link.kind, link.link.landing))
            .collect()
    }

    /// The links shut for the match, as the log's list: each with when, its kind, its ends and its failures.
    pub fn list(&self) -> String {
        let shut: Vec<String> = self
            .links
            .iter()
            .filter_map(|link| {
                let at = link.shut_at?;
                let (from, to) = (link.link.take_off, link.link.landing);
                Some(format!(
                    "{:?} ({:.1}, {:.1}, {:.1}) -> ({:.1}, {:.1}, {:.1}) shut at {at} ms, failed {} times",
                    link.link.kind,
                    from.x,
                    from.y,
                    from.z,
                    to.x,
                    to.y,
                    to.z,
                    link.by.len()
                ))
            })
            .collect();
        format!("{} [{}]", shut.len(), shut.join("; "))
    }

    /// How many links are shut for the match.
    pub fn count(&self) -> usize {
        self.links.iter().filter(|link| link.shut_at.is_some()).count()
    }
}

/// The bench's counts: the failed links told to the match, and the links shut for it.
#[cfg(test)]
pub mod tally {
    use std::sync::atomic::{AtomicU64, Ordering};

    pub const TOLD: usize = 0;
    pub const SHUT: usize = 1;

    static COUNTS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];

    pub fn count(kind: usize) {
        COUNTS[kind].fetch_add(1, Ordering::Relaxed);
    }

    pub fn counts() -> [u64; 2] {
        std::array::from_fn(|k| COUNTS[k].load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drop(take_off: Vec3, landing: Vec3) -> FailedLink {
        FailedLink { kind: ShutKind::Drop, take_off, landing }
    }

    /// The owner's worst link (Vector Laboratory's spawn platform, (81.8, 25.2, -30.8) -> (80.8, 20.8, -30.8)): the first
    /// failure shuts nothing for the match, the second (another bot, the take-off 0.4 m off, the landing 0.2 m off) shuts it
    /// for all; a third adds a failure and shuts nothing new. Another top's drop to the same ground is another link, a
    /// jump to the same landing too.
    #[test]
    fn a_link_failed_twice_is_shut_for_the_match() {
        let tuning = DeadLinksTuning::default();
        let mut links = DeadLinks::default();
        let top = Vec3::new(81.8, 25.2, -30.8);
        let ground = Vec3::new(80.8, 20.8, -30.8);
        assert!(links.learn(1_000, 3, &[drop(top, ground)], &tuning).is_empty());
        assert!(links.shut().is_empty());
        let dead = links.learn(
            50_000,
            5,
            &[drop(top + Vec3::new(0.4, 0.0, 0.0), ground + Vec3::new(0.0, 0.0, 0.2))],
            &tuning,
        );
        assert_eq!(dead, vec![NewlyDead { link: drop(top, ground), by: vec![3, 5] }]);
        assert_eq!(links.shut(), vec![Shut::for_match(ShutKind::Drop, ground)]);
        assert!(links.learn(90_000, 7, &[drop(top, ground)], &tuning).is_empty());
        assert_eq!(links.count(), 1);
        assert!(links.list().starts_with("1 [Drop (81.8, 25.2, -30.8) -> (80.8, 20.8, -30.8) shut at 50000 ms, failed 3 times"));
        // Another top over the same ground, and a jump to that landing: other links, one failure each.
        assert!(links.learn(91_000, 7, &[drop(Vec3::new(78.0, 25.2, -30.8), ground)], &tuning).is_empty());
        let jump = FailedLink { kind: ShutKind::Jump, take_off: top, landing: ground };
        assert!(links.learn(92_000, 7, &[jump], &tuning).is_empty());
        assert_eq!(links.count(), 1);
    }

    /// The same bot failing a link twice (its own shut ended, it took it again) shuts it too: any bot, twice.
    #[test]
    fn one_bot_twice_is_twice() {
        let tuning = DeadLinksTuning::default();
        let mut links = DeadLinks::default();
        let link = drop(Vec3::new(0.0, 5.0, 0.0), Vec3::new(1.0, 2.0, 0.0));
        assert!(links.learn(1_000, 2, &[link], &tuning).is_empty());
        assert_eq!(links.learn(47_000, 2, &[link], &tuning).len(), 1);
    }

    /// Past [`DEAD_LINKS_MAX`] links a new one is not kept; the kept ones still count.
    #[test]
    fn the_list_is_bounded() {
        let tuning = DeadLinksTuning::default();
        let mut links = DeadLinks::default();
        for i in 0..DEAD_LINKS_MAX + 5 {
            let link = drop(Vec3::new(i as f32 * 3.0, 5.0, 0.0), Vec3::new(i as f32 * 3.0, 2.0, 1.0));
            links.learn(1_000, 1, &[link], &tuning);
        }
        assert_eq!(links.links.len(), DEAD_LINKS_MAX);
        let first = drop(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, 2.0, 1.0));
        assert_eq!(links.learn(2_000, 2, &[first], &tuning).len(), 1);
    }
}

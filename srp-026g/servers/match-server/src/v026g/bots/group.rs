//! [A297 item 1, `tdm.group`] Group up and trade in Team Deathmatch (`traces-20261028/fx-team-a297-design.md`).
//!
//! Root cause (PROVEN, the owner's 8 matches of 2026-11-03): the bots stood near each other by chance (74 % of his victims had
//! a mate within 20 m), but the mate wired to trade them was paired by place order (a median 34 m off at the fight start, a line
//! to him at the death in 9 % of his kills), and no mate moved to see the enemy before the death.
//!
//! - [`pair`]: the team's bots paired by position, nearest first, a pair kept while both live within `keep_m`.
//! - [`fighting`]: the team's bots in a fight now (their reflex targets), with when each fight began.
//! - [`cover_ok`]: whether a spot covers a mate's fight from a second bearing, off the lanes that just killed teammates.
//!
//! Information: a bot hears its teammate fire and gets his callout; the enemy's place is only what its own memory holds.

use mapdata::Vec3;

use super::know::{Fighting, Loss};
use super::tuning::GroupUp;
use super::view::Place;

/// A bot of the team at a commander pass: its place, its feet if alive, its partner now, and when that partner died (the
/// kill feed) if it did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    pub place: Place,
    pub feet: Option<Vec3>,
    pub partner: Option<Place>,
    pub partner_died: Option<u32>,
}

/// A bot's partner after [`pair`]: whom, and whether the partner has it for partner too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Paired {
    pub place: Place,
    pub partner: Option<Place>,
    pub mutual: bool,
}

/// A partner who died is kept this long, ms, so its living mate can trade the death (play 21's `fresh_s` is 3 s).
pub const KEEP_DEAD_MS: u32 = 5_000;

/// The team's bots paired by position (in place order of `members`):
/// - a dead bot keeps its partner (nothing to pair);
/// - a living bot whose partner died within [`KEEP_DEAD_MS`] keeps it (its trade);
/// - a living pair that had each other and stands within `keep_m` stays;
/// - the other living bots pair greedily, the nearest two first (ties by place);
/// - an odd one out partners its nearest living mate one way, or keeps the one it had.
pub fn pair(members: &[Member], now: u32, keep_m: f32) -> Vec<Paired> {
    let mut out: Vec<Paired> = members
        .iter()
        .map(|m| Paired { place: m.place, partner: m.partner, mutual: false })
        .collect();
    let index = |place: Place| members.iter().position(|m| m.place == place);
    let feet = |i: usize| members[i].feet;
    let mut free: Vec<bool> = members.iter().map(|m| m.feet.is_some()).collect();
    // Living bots trading a dead partner keep it.
    for (i, m) in members.iter().enumerate() {
        if m.feet.is_some() && m.partner_died.is_some_and(|t| now.saturating_sub(t) <= KEEP_DEAD_MS) {
            free[i] = false;
            out[i].mutual = false;
        }
    }
    // Pairs that had each other and still stand together.
    for i in 0..members.len() {
        let Some(j) = members[i].partner.and_then(index) else { continue };
        if j <= i || !free[i] || !free[j] || members[j].partner != Some(members[i].place) {
            continue;
        }
        if let (Some(a), Some(b)) = (feet(i), feet(j))
            && a.distance_xz(b) <= keep_m
        {
            free[i] = false;
            free[j] = false;
            out[i] = Paired { place: members[i].place, partner: Some(members[j].place), mutual: true };
            out[j] = Paired { place: members[j].place, partner: Some(members[i].place), mutual: true };
        }
    }
    // The rest, nearest two first.
    let mut pairs: Vec<(f32, usize, usize)> = Vec::new();
    for i in 0..members.len() {
        for j in i + 1..members.len() {
            if let (true, true, Some(a), Some(b)) = (free[i], free[j], feet(i), feet(j)) {
                pairs.push((a.distance_xz(b), i, j));
            }
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    for (_, i, j) in pairs {
        if free[i] && free[j] {
            free[i] = false;
            free[j] = false;
            out[i] = Paired { place: members[i].place, partner: Some(members[j].place), mutual: true };
            out[j] = Paired { place: members[j].place, partner: Some(members[i].place), mutual: true };
        }
    }
    // An odd one out: its nearest living mate, one way.
    for i in 0..members.len() {
        let Some(me) = feet(i).filter(|_| free[i]) else { continue };
        let nearest = (0..members.len())
            .filter(|&j| j != i)
            .filter_map(|j| feet(j).map(|f| (f.distance_xz(me), j)))
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, j)| members[j].place);
        out[i] = Paired { place: members[i].place, partner: nearest.or(members[i].partner), mutual: false };
    }
    out
}

/// The team's fights now from each living bot's reflex target, `(mate, enemy)`: a fight that goes on keeps the time it began
/// (from `before`), a new one begins `now`.
pub fn fighting(targets: &[(Place, Place)], before: &[Fighting], now: u32) -> Vec<Fighting> {
    targets
        .iter()
        .map(|&(mate, enemy)| {
            let since = before
                .iter()
                .find(|f| f.mate == mate && f.enemy == enemy)
                .map_or(now, |f| f.since);
            Fighting { mate, enemy, since }
        })
        .collect()
}

/// The bearing of `to` seen from `from` on the ground, radians.
fn bearing(from: Vec3, to: Vec3) -> f32 {
    (to.z - from.z).atan2(to.x - from.x)
}

/// The angle between two bearings, degrees in `[0, 180]`.
fn apart_deg(a: f32, b: f32) -> f32 {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    d.min(std::f32::consts::TAU - d).to_degrees()
}

/// Why a spot does not cover a mate's fight (the counters' classes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotCover {
    /// Seen from the enemy, on the mate's bearing (a file behind or before him).
    InFile,
    /// Within `dead_m` of a teammate's death of the last `dead_s`.
    OnDeath,
    /// On the bearing from the enemy of such a death, nearer him than it was: the lane that killed.
    InKilledLane,
}

/// Whether `spot` covers the fight of a mate at `mate` with the enemy known at `enemy`: from a bearing at least
/// `bearing_min_deg` off the mate's, not on a fresh teammate death, not in the lane a fresh death came from (the same bearing
/// from the enemy within half the minimum, no farther out than that death). The sight line itself is the spot query's.
pub fn cover_ok(spot: Vec3, mate: Vec3, enemy: Vec3, losses: &[Loss], now: u32, rules: &GroupUp) -> Result<(), NotCover> {
    let to_spot = bearing(enemy, spot);
    if apart_deg(to_spot, bearing(enemy, mate)) < rules.bearing_min_deg {
        return Err(NotCover::InFile);
    }
    let fresh_ms = (rules.dead_s.get() * 1000.0) as u32;
    for loss in losses.iter().filter(|l| l.time <= now && now - l.time <= fresh_ms) {
        if loss.at.distance_xz(spot) <= rules.dead_m.get() {
            return Err(NotCover::OnDeath);
        }
        let reach = loss.at.distance_xz(enemy);
        if spot.distance_xz(enemy) <= reach + rules.dead_m.get()
            && apart_deg(to_spot, bearing(enemy, loss.at)) < rules.bearing_min_deg * 0.5
        {
            return Err(NotCover::InKilledLane);
        }
    }
    Ok(())
}

/// What the switch did (the bench's count; nothing live): the topic `group` prints them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Count {
    /// A cover of the partner's fight was taken (a spot found).
    CoverTaken,
    /// A poll at the cover spot while the mate still fights (one per bot and poll).
    CoverHeld,
    /// Not offered: the fighting partner was farther than `answer_m`.
    CoverFar,
    /// Not offered: the bot did not know where the partner's enemy is.
    CoverUnknown,
    /// No spot round the mate sees the enemy (or all lie in a spawn area).
    NoSightSpot,
    /// Spots saw him, but every one lay in file with the mate.
    OnlyInFile,
    /// Spots saw him, but every one lay on a fresh death or in its lane.
    OnlyDeadLanes,
}

/// The counts' order.
pub const COUNTS: [Count; 7] = [
    Count::CoverTaken,
    Count::CoverHeld,
    Count::CoverFar,
    Count::CoverUnknown,
    Count::NoSightSpot,
    Count::OnlyInFile,
    Count::OnlyDeadLanes,
];

#[cfg(test)]
thread_local! {
    static COUNTED: std::cell::Cell<[u64; COUNTS.len()]> = const { std::cell::Cell::new([0; COUNTS.len()]) };
}

/// `c` happened once (the bench's count; nothing live).
#[cfg(test)]
pub fn count(c: Count) {
    COUNTED.with(|cell| {
        let mut all = cell.get();
        all[c as usize] += 1;
        cell.set(all);
    });
}

#[cfg(not(test))]
pub fn count(_: Count) {}

/// The counts so far, by [`COUNTS`] order.
#[cfg(test)]
pub fn counted() -> [u64; COUNTS.len()] {
    COUNTED.with(|cell| cell.get())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, z: f32) -> Option<Vec3> {
        Some(Vec3::new(x, 0.0, z))
    }

    fn m(place: Place, feet: Option<Vec3>, partner: Option<Place>) -> Member {
        Member { place, feet, partner, partner_died: None }
    }

    fn partner_of(out: &[Paired], place: Place) -> Option<Place> {
        out.iter().find(|p| p.place == place).and_then(|p| p.partner)
    }

    #[test]
    fn bots_pair_with_the_nearest_not_by_place_order() {
        // Place order would pair 1-2 and 4-6; by position 1-4 and 2-6.
        let members = [m(1, at(0.0, 0.0), None), m(2, at(100.0, 0.0), None), m(4, at(5.0, 0.0), None), m(6, at(95.0, 0.0), None)];
        let out = pair(&members, 10_000, 35.0);
        assert_eq!(partner_of(&out, 1), Some(4));
        assert_eq!(partner_of(&out, 4), Some(1));
        assert_eq!(partner_of(&out, 2), Some(6));
        assert!(out.iter().all(|p| p.mutual));
    }

    #[test]
    fn a_pair_within_keep_stays_even_when_another_mate_is_nearer() {
        let members = [m(1, at(0.0, 0.0), Some(2)), m(2, at(30.0, 0.0), Some(1)), m(4, at(3.0, 0.0), None), m(6, at(60.0, 0.0), None)];
        let out = pair(&members, 10_000, 35.0);
        assert_eq!(partner_of(&out, 1), Some(2));
        assert_eq!(partner_of(&out, 4), Some(6));
        // Too far apart now: paired again.
        let members = [m(1, at(0.0, 0.0), Some(2)), m(2, at(40.0, 0.0), Some(1)), m(4, at(3.0, 0.0), None), m(6, at(42.0, 0.0), None)];
        let out = pair(&members, 10_000, 35.0);
        assert_eq!(partner_of(&out, 1), Some(4));
        assert_eq!(partner_of(&out, 2), Some(6));
    }

    #[test]
    fn a_fresh_dead_partner_is_kept_for_the_trade_and_an_old_one_is_not() {
        let mut a = m(1, at(0.0, 0.0), Some(2));
        a.partner_died = Some(8_000);
        let members = [a, m(2, None, Some(1)), m(4, at(3.0, 0.0), None)];
        let out = pair(&members, 10_000, 35.0);
        assert_eq!(partner_of(&out, 1), Some(2), "trading its partner's death");
        assert_eq!(partner_of(&out, 2), Some(1), "the dead keep theirs");
        assert_eq!(partner_of(&out, 4), Some(1), "odd one out: nearest living, one way");
        assert!(!out[2].mutual);
        let out = pair(&members, 20_000, 35.0);
        assert_eq!(partner_of(&out, 1), Some(4));
        assert_eq!(partner_of(&out, 4), Some(1));
    }

    #[test]
    fn a_fight_keeps_its_start_and_a_new_one_starts_now() {
        let before = [Fighting { mate: 1, enemy: 9, since: 1_000 }, Fighting { mate: 2, enemy: 9, since: 2_000 }];
        let now = fighting(&[(1, 9), (2, 8)], &before, 5_000);
        assert_eq!(now, vec![Fighting { mate: 1, enemy: 9, since: 1_000 }, Fighting { mate: 2, enemy: 8, since: 5_000 }]);
    }

    #[test]
    fn a_cover_spot_needs_a_second_bearing_and_keeps_off_fresh_deaths_and_their_lane() {
        let rules = GroupUp::default();
        let enemy = Vec3::new(0.0, 0.0, 0.0);
        let mate = Vec3::new(20.0, 0.0, 0.0);
        // Behind the mate on his line: a file.
        assert_eq!(cover_ok(Vec3::new(30.0, 0.0, 2.0), mate, enemy, &[], 10_000, &rules), Err(NotCover::InFile));
        // 45 degrees round: covers.
        let side = Vec3::new(14.0, 0.0, 14.0);
        assert_eq!(cover_ok(side, mate, enemy, &[], 10_000, &rules), Ok(()));
        // A teammate died there 5 s ago: not there.
        let loss = Loss { at: Vec3::new(15.0, 0.0, 12.0), time: 5_000, spawner: None };
        assert_eq!(cover_ok(side, mate, enemy, &[loss], 10_000, &rules), Err(NotCover::OnDeath));
        // ... 30 s ago: fine again.
        assert_eq!(cover_ok(side, mate, enemy, &[loss], 40_000, &rules), Ok(()));
        // A death farther out on the same bearing: the spot is in the lane that killed.
        let far = Loss { at: Vec3::new(30.0, 0.0, 30.0), time: 5_000, spawner: None };
        assert_eq!(cover_ok(side, mate, enemy, &[far], 10_000, &rules), Err(NotCover::InKilledLane));
    }
}

//! A310: the squad layer in Team Deathmatch (`traces-20261028/fx-team-a310-squad.md`): squads of 2-3 formed at spawn by
//! position and kept for the match, a leader each (the living member of the highest skill), and the radius the members keep
//! round him (`tuning::tdm::Squad`).
//!
//! Why a squad and not a per-bot rule: A297's per-bot rejoin and per-bot target worth were flat or helped the hunter stand-in;
//! squads keep cohesion and focus as a squad-level state from the start (the Arma "Realistic AI" mod's 20 m leader radius,
//! Steamhammer's clusters, the squad focus order of SC2-style targeting).

use super::view::Place;
use mapdata::Vec3;

/// The team's squads, nearest pairs by `members`' feet (place, feet); an odd one out joins the pair nearest to it (a squad of
/// three). Deterministic: the seed of each pair is the lowest place left.
pub fn form(members: &[(Place, Vec3)]) -> Vec<Vec<Place>> {
    let mut left: Vec<(Place, Vec3)> = members.to_vec();
    left.sort_by_key(|m| m.0);
    let mut squads: Vec<Vec<(Place, Vec3)>> = Vec::new();
    while left.len() >= 2 {
        let seed = left.remove(0);
        let (i, _) = left
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.1.distance_xz(seed.1).total_cmp(&b.1.1.distance_xz(seed.1)).then(a.1.0.cmp(&b.1.0)))
            .expect("one left at least");
        let mate = left.remove(i);
        squads.push(vec![seed, mate]);
    }
    if let Some(odd) = left.pop() {
        match squads
            .iter_mut()
            .min_by(|a, b| nearest(a, odd.1).total_cmp(&nearest(b, odd.1)))
        {
            Some(squad) => squad.push(odd),
            None => squads.push(vec![odd]),
        }
    }
    squads.into_iter().map(|s| s.into_iter().map(|m| m.0).collect()).collect()
}

fn nearest(squad: &[(Place, Vec3)], at: Vec3) -> f32 {
    squad.iter().map(|m| m.1.distance_xz(at)).fold(f32::INFINITY, f32::min)
}

/// Each living member's leader, (member, leader), for members who are not the leader themselves: the living member of the
/// highest `skill` (`None`: dead), the lowest place on a tie. A squad with no one alive, or only its leader, gives nothing.
pub fn leaders(squads: &[Vec<Place>], skill: impl Fn(Place) -> Option<f32>) -> Vec<(Place, Place)> {
    let mut out = Vec::new();
    for squad in squads {
        let alive: Vec<(Place, f32)> = squad.iter().filter_map(|&p| skill(p).map(|s| (p, s))).collect();
        let Some(&(leader, _)) = alive.iter().max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0))) else {
            continue;
        };
        out.extend(alive.iter().filter(|m| m.0 != leader).map(|m| (m.0, leader)));
    }
    out
}

/// Where a member farther than the radius walks to: `close_m` from the leader toward the member (its own side of him, never
/// behind him on a line through the member: no file).
pub fn beside(leader: Vec3, member: Vec3, close_m: f32) -> Vec3 {
    let (dx, dz) = (member.x - leader.x, member.z - leader.z);
    let d = dx.hypot(dz);
    if d <= close_m || d <= f32::EPSILON {
        return member;
    }
    Vec3::new(leader.x + dx / d * close_m, leader.y, leader.z + dz / d * close_m)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pairs by position, an odd one joins the nearest pair; the leader is the most skilled living member; a dead leader is
    /// replaced by the next.
    #[test]
    fn squads_form_by_position_and_lead_by_skill() {
        let at = |x: f32| Vec3::new(x, 0.0, 0.0);
        let squads = form(&[(1, at(0.0)), (2, at(100.0)), (3, at(3.0)), (4, at(104.0)), (5, at(98.0))]);
        assert_eq!(squads, vec![vec![1, 3], vec![2, 5, 4]]);
        let skill = |p: Place| Some([0.0, 0.5, 0.9, 0.4, 0.6, 0.7][p]);
        assert_eq!(leaders(&squads, skill), vec![(3, 1), (5, 2), (4, 2)]);
        let without_2 = |p: Place| (p != 2).then(|| [0.0, 0.5, 0.9, 0.4, 0.6, 0.7][p]);
        assert_eq!(leaders(&squads, without_2), vec![(3, 1), (4, 5)]);
        assert_eq!(beside(at(0.0), at(30.0), 6.0), at(6.0));
        assert_eq!(beside(at(0.0), at(4.0), 6.0), at(4.0), "already close: stays");
    }
}

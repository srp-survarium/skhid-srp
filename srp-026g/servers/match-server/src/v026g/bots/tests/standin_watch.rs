//! The stand-in sniper's watch (`SRP_BENCH_CAMPER_WATCH=lane`, A182): a live sniper who holds a spot has his view on the lane
//! he watches, scoped, so that the first shot is not spent finding the enemy: when he is already scoped and looking at an enemy
//! who comes into the line, he shoots at 44 % of them and first shoots 1.65 s after they show (median; 190 live recordings,
//! `HUMAN-SNIPERS.md`), against 23 % and 2.75 s for all of them. The stand-in's default is to stand "about" ([`Look::Around`]),
//! unscoped, and turn when its reflex has a target.
//!
//! The lane is the stretch of the other side's route points that the spot sees ([`CAMPER_SEES_M`]) where most of them lie: the
//! [`LANE_DEG`] window of bearings with the most points, looked along at [`LANE_LOOK_M`]. The view and the scope are held only
//! while he is at rest inside the stay's cover hold (not on a peek or a step to another point, not on the way); the reflex takes
//! the view for a target as ever.

use super::*;

/// The window of bearings the lane is looked for in, degrees.
const LANE_DEG: f32 = 30.0;
/// The view is on a point this far along the lane, m.
const LANE_LOOK_M: f32 = 20.0;
/// At rest means slower than this, m/s.
pub(crate) const AT_REST_MPS: f32 = 0.5;

/// What the stand-in does with its view at its spot (`SRP_BENCH_CAMPER_WATCH`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Watch {
    /// Stands about, unscoped (the old stand-in).
    Around,
    /// Holds the view on the lane, scoped.
    Lane,
}

impl Watch {
    pub(crate) fn parse(value: Option<&str>) -> Result<Self, String> {
        match value {
            None | Some("around") => Ok(Self::Around),
            Some("lane") => Ok(Self::Lane),
            Some(other) => Err(format!("SRP_BENCH_CAMPER_WATCH={other:?}: around or lane")),
        }
    }
}

/// The bearing (radians, `atan2(z, x)`) of the middle of the [`LANE_DEG`] window with the most of `bearings`, the mean of what
/// it holds; none for none. Ties go to the first window from the lowest bearing.
pub(crate) fn lane_bearing(bearings: &[f32]) -> Option<f32> {
    let width = LANE_DEG.to_radians();
    let tau = std::f32::consts::TAU;
    let wrapped = |from: f32, to: f32| (to - from).rem_euclid(tau);
    let mut sorted = bearings.to_vec();
    sorted.sort_by(f32::total_cmp);
    let holds = |start: f32| sorted.iter().copied().filter(move |&b| wrapped(start, b) <= width).collect::<Vec<f32>>();
    let best = sorted.iter().map(|&start| holds(start)).fold(None::<Vec<f32>>, |best, held| match best {
        Some(b) if b.len() >= held.len() => Some(b),
        _ => Some(held),
    })?;
    // The mean by sines and cosines, so that a window across the turn averages right.
    let (s, c) = best.iter().fold((0.0, 0.0), |(s, c), b| (s + b.sin(), c + b.cos()));
    Some(s.atan2(c))
}

/// The point to look at from `spot` to watch its lane: [`LANE_LOOK_M`] along the lane at the height of the eye of the stance
/// (`Look::At` aims a metre above its point), or none if the spot sees no route point of the other side.
pub(crate) fn lane_look(tactics: &super::tactics::Tactics, side: i32, spot: Vec3, crouched: bool) -> Option<Vec3> {
    let sight = &tactics.world.sight;
    let eye_up = super::view::still_eye(Vec3::ZERO, crouched);
    let eye = spot + eye_up;
    let bearings: Vec<f32> = tactics
        .routes_of(1 - side)
        .flat_map(|r| r.points.iter().copied())
        .filter(|q| q.is_finite() && q.distance_xz(spot) <= CAMPER_SEES_M)
        .filter(|&q| super::aimpoint::shows(eye, q, false, sight, || true))
        .map(|q| (q.z - spot.z).atan2(q.x - spot.x))
        .collect();
    let b = lane_bearing(&bearings)?;
    Some(spot + Vec3::new(b.cos() * LANE_LOOK_M, eye_up.y - 1.0, b.sin() * LANE_LOOK_M))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lane_is_where_most_of_the_points_lie() {
        let deg = |d: f32| d.to_radians();
        let lane = lane_bearing(&[deg(10.0), deg(15.0), deg(20.0), deg(100.0), deg(200.0)]).expect("a lane");
        assert!((lane.to_degrees() - 15.0).abs() < 0.5, "{}", lane.to_degrees());
    }

    #[test]
    fn a_lane_across_the_turn_is_averaged_right() {
        let deg = |d: f32| d.to_radians();
        let lane = lane_bearing(&[deg(-170.0), deg(175.0), deg(-175.0), deg(0.0)]).expect("a lane");
        assert!((lane.to_degrees().abs() - 180.0).abs() < 8.0, "{}", lane.to_degrees());
    }

    #[test]
    fn no_points_no_lane() {
        assert_eq!(lane_bearing(&[]), None);
    }

    #[test]
    fn the_watch_setting_is_parsed_and_refuses_an_unknown_value() {
        assert_eq!(Watch::parse(None), Ok(Watch::Around));
        assert_eq!(Watch::parse(Some("lane")), Ok(Watch::Lane));
        assert!(Watch::parse(Some("lanes")).is_err());
    }

    #[test]
    fn the_stand_ins_lane_on_the_soak_field_runs_toward_the_other_side() {
        let mut tactics = super::tactics::Tactics::new(super::tactics::tests::field(), 1.0);
        tactics.set_routes(camper_routes());
        let (spot, _) = camper_spot(&tactics, 0).expect("a spot");
        let look = lane_look(&tactics, 0, spot, false).expect("a lane");
        assert!(look.distance_xz(spot) > LANE_LOOK_M - 0.1 && look.distance_xz(spot) < LANE_LOOK_M + 0.1);
        // Side 1 lies at higher x on the soak field.
        assert!(look.x > spot.x, "{look:?} from {spot:?}");
    }
}

//! [threat.walk_watch] A229 (`traces-20261028/fx-threat-obj-a229-a231-note.md`): a play's walk looks along its route
//! (`Look::Path`), and only the reflex turns the view (an enemy sensed, a sound, a smoke wait, a throw), so a bot walks into
//! a shooter's lane the team knows with the view where the route goes: in the bench a quarter of the walks inside a held
//! lane had its spot behind them. A human walking toward a known threat clears that angle as he goes (the crosshair where
//! the enemy can appear), as Counter-Strike's bot looks at the encounter spots of its path. With the switch, a walk whose
//! heading has a known threat ahead looks at it: the spot of a held lane of the team the bot stands in, or, with
//! [`WalkWatch::known`], the last place its own or its team's senses gave an enemy (`Track::own`). A carrier, and a walk
//! across a lane rather than toward its spot, keep the route's view and the sprint (the motor takes no sprint aimed,
//! `motor::walk_aiming`). The reflex's own look still wins (`drive.rs`: `Override::look_at` before the play's look).

use mapdata::Vec3;

use super::{Ctx, Go, Intent, Look};
use crate::v026g::bots::kin;
use crate::v026g::bots::tuning::WalkWatch;

/// A walk is a heading this fast at least, m/s: a bot standing, or pressing into a wall, has none.
const WALKING_MPS: f32 = 0.5;

/// What a walk watched, or why it kept its route's view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Watched {
    /// The spot of a held lane of the team the bot stands in, ahead.
    Lane(Vec3),
    /// An enemy's last known place, ahead and near.
    Known(Vec3),
    /// In a held lane whose spot is not ahead: a crossing, walked on with the route's view.
    Crossing,
    /// Carrying: home first, with the sprint.
    Carrying,
    /// Nothing known ahead.
    Nothing,
}

impl Watched {
    /// The point looked at, if any.
    pub fn at(self) -> Option<Vec3> {
        match self {
            Watched::Lane(p) | Watched::Known(p) => Some(p),
            Watched::Crossing | Watched::Carrying | Watched::Nothing => None,
        }
    }
}

/// What the walk of `intent` watches, if it is a walk along the route: `None` for any other intent.
pub fn watched(cx: &Ctx, watch: &WalkWatch, intent: &Intent) -> Option<Watched> {
    let walking = matches!(intent.look, Look::Path)
        && matches!(intent.go, Go::To { .. })
        && cx.me.velocity[0].hypot(cx.me.velocity[1]) >= WALKING_MPS;
    if !walking {
        return None;
    }
    if cx.me.carrying.is_some() {
        return Some(Watched::Carrying);
    }
    let feet = cx.me.feet;
    let heading = kin::yaw_of(cx.me.velocity);
    let ahead = |p: Vec3| kin::wrap(kin::yaw_of([p.x - feet.x, p.z - feet.z]) - heading).abs() <= watch.ahead_deg.to_radians();
    let near = |p: Vec3| p.distance_xz(feet);
    let lanes: Vec<_> = cx.team.threat.held.iter().filter(|l| l.covers(feet)).collect();
    if let Some(line) = lanes.iter().filter(|l| ahead(l.at)).min_by(|a, b| near(a.at).total_cmp(&near(b.at))) {
        return Some(Watched::Lane(line.at));
    }
    if let Some(known) = watch.known {
        let place = cx
            .memory
            .tracks()
            .iter()
            .filter_map(|t| t.own.filter(|&(_, when)| cx.now.wrapping_sub(when) <= known.ms).map(|(at, _)| at))
            .filter(|&at| near(at) <= known.max_m && ahead(at))
            .min_by(|a, b| near(*a).total_cmp(&near(*b)));
        if let Some(at) = place {
            return Some(Watched::Known(at));
        }
    }
    Some(if lanes.is_empty() { Watched::Nothing } else { Watched::Crossing })
}

/// The play's intent with the walk's watch ([`watched`]), and its count. The intent as it was with the switch off.
pub fn walk_watch(cx: &Ctx, intent: Intent) -> Intent {
    let Some(watch) = cx.tuning.threat.walk_watch else {
        return intent;
    };
    let Some(watched) = watched(cx, &watch, &intent) else {
        return intent;
    };
    tally::counted_one(cx.me.team, watched);
    match watched.at() {
        Some(at) => intent.looking(Look::At(at)),
        None => intent,
    }
}

/// [measure] The walks' watches by team (test builds only; nothing decides by it).
pub mod tally {
    use super::Watched;

    /// The kinds counted, in [`counted`]'s order.
    #[cfg(test)]
    pub const KINDS: [&str; 5] = ["lane", "known", "crossing", "carrying", "nothing"];

    #[cfg(test)]
    thread_local! {
        static WATCHES: std::cell::Cell<[[u64; 5]; 2]> = const { std::cell::Cell::new([[0; 5]; 2]) };
    }

    #[cfg(test)]
    pub fn counted_one(team: u8, watched: Watched) {
        let k = match watched {
            Watched::Lane(_) => 0,
            Watched::Known(_) => 1,
            Watched::Crossing => 2,
            Watched::Carrying => 3,
            Watched::Nothing => 4,
        };
        WATCHES.with(|w| {
            let mut all = w.get();
            all[usize::from(team).min(1)][k] += 1;
            w.set(all);
        });
    }

    #[cfg(not(test))]
    pub fn counted_one(_: u8, _: Watched) {}

    /// The walk steps by team and [`KINDS`], since the thread began.
    #[cfg(test)]
    pub fn counted() -> [[u64; 5]; 2] {
        WATCHES.with(std::cell::Cell::get)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::know::{Percept, Source};
    use crate::v026g::bots::plays::Pace;
    use crate::v026g::bots::plays::tests::Scenario;
    use crate::v026g::bots::threat::HeldLine;
    use crate::v026g::bots::view::Mode;

    /// Bot 0 at (40, 0, 30) walking at 3 m/s along `velocity` to (60, 0, 30); the enemy 1 at (70, 0, 30), who killed at
    /// (20, 0, 30): his lane down x covers the bot. The switch on, with the known places.
    fn walking(velocity: [f32; 2]) -> Scenario {
        let mut s = Scenario::field(Mode::TeamDeathmatch);
        s.bot(0, 0, Vec3::new(40.0, 0.0, 30.0));
        s.enemy(1, 1, Vec3::new(70.0, 0.0, 30.0), None);
        s.view.places[0].velocity = velocity;
        s.tuning.threat.walk_watch = Some(WalkWatch::default());
        let now = s.view.clock;
        std::sync::Arc::make_mut(&mut s.team.threat).held.push(HeldLine::for_test(
            9,
            1,
            Vec3::new(70.0, 0.0, 30.0),
            &[Vec3::new(20.0, 0.0, 30.0)],
            Some(now),
        ));
        s
    }

    fn walk() -> Intent {
        Intent::to(Vec3::new(60.0, 0.0, 30.0), Pace::Run)
    }

    #[test]
    fn a_walk_toward_a_held_spot_in_its_lane_looks_at_it() {
        let s = walking([3.0, 0.0]);
        let him = Vec3::new(70.0, 0.0, 30.0);
        assert_eq!(walk_watch(&s.cx(0), walk()).look, Look::At(him));
    }

    #[test]
    fn a_walk_across_a_lane_or_with_a_battery_keeps_the_routes_view() {
        let s = walking([0.0, 3.0]);
        let watch = s.tuning.threat.walk_watch.unwrap();
        assert_eq!(watched(&s.cx(0), &watch, &walk()), Some(Watched::Crossing));
        assert_eq!(walk_watch(&s.cx(0), walk()).look, Look::Path);
        let mut s = walking([3.0, 0.0]);
        s.view.places[0].carrying = Some(0);
        assert_eq!(watched(&s.cx(0), &watch, &walk()), Some(Watched::Carrying));
        assert_eq!(walk_watch(&s.cx(0), walk()).look, Look::Path);
    }

    #[test]
    fn with_the_switch_off_or_no_walk_nothing_changes() {
        let mut s = walking([3.0, 0.0]);
        s.tuning.threat.walk_watch = None;
        assert_eq!(walk_watch(&s.cx(0), walk()), walk());
        let s = walking([0.1, 0.0]);
        assert_eq!(walk_watch(&s.cx(0), walk()), walk(), "standing");
        let s = walking([3.0, 0.0]);
        let stay = Intent::stay();
        assert_eq!(walk_watch(&s.cx(0), stay), stay, "no walk");
    }

    #[test]
    fn an_enemys_known_place_ahead_is_watched_only_with_known() {
        let mut s = walking([3.0, 0.0]);
        std::sync::Arc::make_mut(&mut s.team.threat).held.clear();
        let watch = s.tuning.threat.walk_watch.unwrap();
        assert_eq!(watched(&s.cx(0), &watch, &walk()), Some(Watched::Nothing), "nothing known yet");
        let now = s.view.clock;
        let at = Vec3::new(70.0, 0.0, 32.0);
        s.memory.update(&[Percept { who: 1, at, error_m: 0.0, time: now, source: Source::Seen, busy: false }], now, |_| 200, 3_000, 20_000);
        assert_eq!(watched(&s.cx(0), &watch, &walk()), Some(Watched::Known(at)));
        let lanes_only = WalkWatch { known: None, ..watch };
        assert_eq!(watched(&s.cx(0), &lanes_only, &walk()), Some(Watched::Nothing));
    }
}

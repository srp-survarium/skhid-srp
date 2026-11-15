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

use super::{Ctx, Go, Intent, Look, Pace};
use crate::v026g::bots::kin;
use crate::v026g::bots::nav_log::Kind;
use crate::v026g::bots::threat::KnownHolder;
use crate::v026g::bots::tuning::{WalkWatch, WatchKnown, WatchNear};
use crate::v026g::drive::key;

/// A walk is a heading this fast at least, m/s: a bot standing, or pressing into a wall, has none.
const WALKING_MPS: f32 = 0.5;

/// What a walk watched, or why it kept its route's view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Watched {
    /// The spot of a held lane of the team the bot stands in, ahead.
    Lane(Vec3),
    /// An enemy's last known place, ahead and near.
    Known(Vec3),
    /// [D2] A known holder of the team (`threat.holder`), ahead.
    Holder(KnownHolder),
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
            Watched::Holder(h) => Some(h.at),
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
    match watch.known {
        Some(WatchKnown::Sensed { ms, max_m }) => {
            let place = cx
                .memory
                .tracks()
                .iter()
                .filter_map(|t| t.own.filter(|&(_, when)| cx.now.wrapping_sub(when) <= ms).map(|(at, _)| at))
                .filter(|&at| near(at) <= max_m && ahead(at))
                .min_by(|a, b| near(*a).total_cmp(&near(*b)));
            if let Some(at) = place {
                return Some(Watched::Known(at));
            }
        }
        // [D2] The team's known holders (only its own senses made them, `threat::HolderStays`), the nearest ahead.
        Some(WatchKnown::Holders) => {
            let holder = cx.team.threat.holders.iter().filter(|h| ahead(h.at)).min_by(|a, b| near(a.at).total_cmp(&near(b.at)));
            if let Some(h) = holder {
                return Some(Watched::Holder(*h));
            }
        }
        None => {}
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
    let Some(at) = watched.at() else {
        return intent;
    };
    let range_m = at.distance_xz(cx.me.feet);
    let near = near_watch(watch.near, range_m);
    tally::near_one(cx.me.team, near);
    if near != Near::Far {
        note(cx, watched, near, range_m, &intent);
    }
    match near {
        Near::Far => intent,
        Near::Watch => intent.looking(Look::At(at)),
        Near::SightsUp => Intent { hold: intent.hold | key::AIM, ..intent.looking(Look::At(at)) },
    }
}

/// [D2] The log's line of a watch begun or its sights up: once a watched thing (a holder's stay, a held spot) and step a bot,
/// with his range, how old the team's sense of him is, and whether the play's sprint was given up (`motor::walk_aiming`: no
/// sprint aimed).
fn note(cx: &Ctx, watched: Watched, near: Near, range_m: f32, intent: &Intent) {
    let sprint = if matches!(intent.pace, Pace::Run | Pace::Cruise | Pace::Arrive) { "; sprint suppressed" } else { "" };
    let (what, key) = match watched {
        Watched::Holder(h) => (
            format!(
                "a known holder (player {}) {:.0} m off, holding {:.1} s, last sensed {:.1} s ago",
                h.who,
                range_m,
                h.last.saturating_sub(h.since) as f32 / 1000.0,
                cx.now.saturating_sub(h.last) as f32 / 1000.0
            ),
            (h.who as u64) << 32 | u64::from(h.since),
        ),
        Watched::Lane(p) => (
            format!("a held spot at ({:.1}, {:.1}, {:.1}) {:.0} m off", p.x, p.y, p.z, range_m),
            (u64::from(p.x.to_bits()) << 32) | u64::from(p.z.to_bits()),
        ),
        Watched::Known(p) => (
            format!("an enemy's known place {:.0} m off", range_m),
            (u64::from(p.x.to_bits()) << 32) | u64::from(p.z.to_bits()),
        ),
        Watched::Crossing | Watched::Carrying | Watched::Nothing => return,
    };
    let (kind, step) = match near {
        Near::SightsUp => (Kind::WatchAim, "aim-up"),
        _ => (Kind::Watch, "watch start"),
    };
    cx.lines.push(kind, Some(key), format!("walk {step}: {what}{sprint}"));
}

/// [A290] What a walk does with a watched point `distance` m off ([`WalkWatch::near`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Near {
    /// Farther than `watch_m`: the route's view and the sprint.
    Far,
    /// Looked at.
    Watch,
    /// Looked at, the sights up.
    SightsUp,
}

/// The walk's [`Near`] for a point `distance` m off: with no near band (`None`) every watched point is looked at, as A229 did.
pub fn near_watch(near: Option<WatchNear>, distance: f32) -> Near {
    match near {
        None => Near::Watch,
        Some(near) if distance > near.watch_m => Near::Far,
        Some(near) if distance > near.aim_m => Near::Watch,
        Some(_) => Near::SightsUp,
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
            Watched::Known(_) | Watched::Holder(_) => 1,
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

    // [A290] The watched walks by `super::Near` (far: route view kept; watched; sights up), by team.
    #[cfg(test)]
    thread_local! {
        static NEAR: std::cell::Cell<[[u64; 3]; 2]> = const { std::cell::Cell::new([[0; 3]; 2]) };
    }

    #[cfg(test)]
    pub fn near_one(team: u8, near: super::Near) {
        let k = match near {
            super::Near::Far => 0,
            super::Near::Watch => 1,
            super::Near::SightsUp => 2,
        };
        NEAR.with(|n| {
            let mut all = n.get();
            all[usize::from(team).min(1)][k] += 1;
            n.set(all);
        });
    }

    #[cfg(not(test))]
    pub fn near_one(_: u8, _: super::Near) {}

    /// The watched walk steps by team and [`super::Near`] (far, watch, sights up), since the thread began.
    #[cfg(test)]
    pub fn near_counted() -> [[u64; 3]; 2] {
        NEAR.with(std::cell::Cell::get)
    }

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
        // A229's walk: watched at any distance (the near band has its own tests below).
        s.tuning.threat.walk_watch = Some(WalkWatch { near: None, ..WalkWatch::default() });
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

    #[test]
    fn the_near_band_watches_only_near_and_puts_the_sights_up_nearer() {
        let band = Some(WatchNear::default());
        assert_eq!(near_watch(None, 50.0), Near::Watch, "no band: as A229");
        assert_eq!(near_watch(band, 30.0), Near::Far);
        assert_eq!(near_watch(band, 25.0), Near::Watch);
        assert_eq!(near_watch(band, 15.0), Near::SightsUp);
        // The held spot 30 m off: the route's view and its sprint.
        let mut s = walking([3.0, 0.0]);
        s.tuning.threat.walk_watch = Some(WalkWatch::default());
        assert_eq!(walk_watch(&s.cx(0), walk()), walk());
        // 12 m off: looked at, the sights up.
        s.view.places[0].feet = Vec3::new(58.0, 0.0, 30.0);
        let watched = walk_watch(&s.cx(0), walk());
        assert_eq!(watched.look, Look::At(Vec3::new(70.0, 0.0, 30.0)));
        assert_ne!(watched.hold & key::AIM, 0);
    }

    /// [D2] With the holders as its known places, a walk toward a known holder ahead watches him (no other sensed enemy), puts
    /// its sights up within 15 m, keeps its route's view and sprint farther than 25 m, and logs the watch's start and its sights up
    /// with his range and age, and the sprint given up.
    #[test]
    fn a_walk_toward_a_known_holder_clears_his_angle_near_him_and_logs_it() {
        use crate::v026g::bots::threat::KnownHolder;
        let mut s = walking([3.0, 0.0]);
        std::sync::Arc::make_mut(&mut s.team.threat).held.clear();
        let holders = WalkWatch { known: Some(WatchKnown::Holders), ..WalkWatch::default() };
        s.tuning.threat.walk_watch = Some(holders);
        let now = s.view.clock;
        // Another enemy sensed ahead is not a holder: nothing watched.
        let seen = Vec3::new(55.0, 0.0, 31.0);
        s.memory.update(&[Percept { who: 1, at: seen, error_m: 0.0, time: now, source: Source::Seen, busy: false }], now, |_| 200, 3_000, 20_000);
        assert_eq!(watched(&s.cx(0), &holders, &walk()), Some(Watched::Nothing));
        // A holder 30 m ahead: past the near band, the route's view and the sprint.
        let him = KnownHolder { who: 1, at: Vec3::new(70.0, 0.0, 30.0), since: now.saturating_sub(6_000), last: now.saturating_sub(1_000) };
        std::sync::Arc::make_mut(&mut s.team.threat).holders.push(him);
        assert_eq!(watched(&s.cx(0), &holders, &walk()), Some(Watched::Holder(him)));
        assert_eq!(walk_watch(&s.cx(0), walk()), walk());
        // 20 m: looked at (the motor takes no sprint aimed), logged once.
        s.view.places[0].feet = Vec3::new(50.0, 0.0, 30.0);
        let cx = s.cx(0);
        let watched_walk = walk_watch(&cx, walk());
        assert_eq!(watched_walk.look, Look::At(him.at));
        assert_eq!(watched_walk.hold & key::AIM, 0);
        let lines = cx.lines.take();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].0, Kind::Watch);
        assert!(lines[0].2.contains("player 1") && lines[0].2.contains("20 m off") && lines[0].2.contains("holding 5.0 s"));
        assert!(lines[0].2.contains("last sensed 1.0 s ago") && lines[0].2.ends_with("sprint suppressed"), "{}", lines[0].2);
        // 12 m: the sights up, its own line.
        s.view.places[0].feet = Vec3::new(58.0, 0.0, 30.0);
        let cx = s.cx(0);
        let aimed = walk_watch(&cx, walk());
        assert_ne!(aimed.hold & key::AIM, 0);
        let lines = cx.lines.take();
        assert_eq!((lines.len(), lines[0].0), (1, Kind::WatchAim));
        assert!(lines[0].2.starts_with("walk aim-up: a known holder (player 1) 12 m off"), "{}", lines[0].2);
        // A walk not toward him (he is behind): nothing.
        let s2 = {
            let mut s2 = walking([-3.0, 0.0]);
            std::sync::Arc::make_mut(&mut s2.team.threat).held.clear();
            std::sync::Arc::make_mut(&mut s2.team.threat).holders.push(him);
            s2
        };
        assert_eq!(watched(&s2.cx(0), &holders, &walk()), Some(Watched::Nothing));
    }

    /// [D2] The defaults: the watch ON with the holders and the near band (A290: 25 m, 15 m); `all_off` names it off.
    #[test]
    fn the_watch_is_on_with_the_holders_and_the_near_band() {
        let t = crate::v026g::bots::tuning::Tuning::default();
        let watch = t.threat.walk_watch.expect("ON");
        assert_eq!(watch.known, Some(WatchKnown::Holders));
        assert_eq!(watch.near, Some(WatchNear { watch_m: 25.0, aim_m: 15.0 }));
        assert!(t.threat.holder.is_some());
    }
}

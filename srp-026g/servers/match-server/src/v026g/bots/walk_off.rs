//! [A1b] The walk-off at release: a drop link the grid flew as `mapdata::grid::gap::Takeoff::Release` (a shallow ledge under an
//! edge that a steady walk overshoots) is walked from its take-off along its way, crouched when the press says, and the walk key
//! is let go once the body's centre is the release's `at` m along the way. Then no move key until the body is down and still:
//! the client's stop blend carries it the rest of the way (exactness `mapflights` §2 A1b: crouched, released on the step before
//! the body leaves the top, 45 of 45 on the ledge; the grid's release flight, `drop_check::released_rest`).
//!
//! A let-go that ended stays [`WalkOff::Done`] while the body is on the same link's leg, so a body down short of the
//! landing's point is not let go again.
//!
//! One state ([`WalkOff`]) and one transition ([`step`]) from (state, the tick's [`Input`]) to the new state and its [`Effect`].

use mapdata::Vec3;
use mapdata::grid::drop_check::{ARRIVE_FLAT_M, ARRIVE_HEIGHT_M, RELEASE_SETTLE_S};

use super::kin::TICK_S;
use super::motor::Footing;
use super::path::WalkOffRelease;
use crate::v026g::drive::key;

/// The keys a let-go body holds none of.
const MOVE: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::SPRINT;

/// A body under this ground speed, m/s, is still: the motor's own let-go after a flown landing (`motor.rs` `STILL_MS`).
pub const STILL_MS: f32 = 0.3;

/// Where the motor is in a walk-off at release.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum WalkOff {
    /// No release link under way.
    #[default]
    Idle,
    /// On the link's leg past its take-off, short of the release: the route's walk, crouched when the press says.
    Approach { link: WalkOffRelease },
    /// The walk key let go at motor tick `since`, the body's feet `from` then and `along` m past the take-off, until it is
    /// down and still.
    LetGo { link: WalkOffRelease, since: u32, from: Vec3, along: f32 },
    /// The let-go of `link` ended (landed, overshot or timed out): the route's walk, and no new release of the same link while
    /// the body is still on its leg (a body down short of the landing's point is still on it).
    Done { link: WalkOffRelease },
}

/// One tick's facts for [`step`].
#[derive(Clone, Copy, Debug)]
pub struct Input {
    /// The release link whose leg the body is on and how far along the leg, m (`Route::walk_off_at`).
    pub on_leg: Option<(WalkOffRelease, f32)>,
    pub feet: Vec3,
    pub footing: Footing,
    /// Ground speed, m/s.
    pub speed: f32,
    /// The motor's tick.
    pub clock: u32,
}

/// How a let-go ended, for the log.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End {
    /// Down and still on the link's landing (the grid's arrival: [`ARRIVE_FLAT_M`] across, [`ARRIVE_HEIGHT_M`] in height).
    Landed(Vec3),
    /// Down and still somewhere else.
    Overshot(Vec3),
    /// Not down and still within [`RELEASE_SETTLE_S`]: the route's walk takes over.
    TimedOut(Vec3),
}

/// A let-go that ended, for its log line: where it began (the body `along` m past the take-off, its feet `from`), how long it
/// took, and how it ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ended {
    pub link: WalkOffRelease,
    pub from: Vec3,
    pub along: f32,
    pub after_ms: u32,
    pub end: End,
}

/// What a tick of [`step`] does to the keys, and the let-go it ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Effect {
    /// Hold the crouch key.
    pub crouch: bool,
    /// Let every move key go.
    pub let_go: bool,
    pub ended: Option<Ended>,
}

impl Effect {
    const ROUTE: Self = Self { crouch: false, let_go: false, ended: None };

    /// The route's `keys` with this tick's.
    pub fn keys(self, keys: u32) -> u32 {
        let keys = if self.let_go { keys & !MOVE } else { keys };
        if self.crouch { keys | key::CROUCH } else { keys }
    }
}

/// The transition: total over the states and the tick's facts.
pub fn step(state: WalkOff, input: Input) -> (WalkOff, Effect) {
    let held = |link: WalkOffRelease| Effect { crouch: link.crouched, ..Effect::ROUTE };
    let let_go = |link: WalkOffRelease| Effect { crouch: link.crouched, let_go: true, ended: None };
    let begin = |link: WalkOffRelease, along: f32| (WalkOff::LetGo { link, since: input.clock, from: input.feet, along }, let_go(link));
    match state {
        WalkOff::Idle | WalkOff::Approach { .. } => match input.on_leg {
            Some((link, along)) if along >= link.at => begin(link, along),
            Some((link, _)) => (WalkOff::Approach { link }, held(link)),
            None => (WalkOff::Idle, Effect::ROUTE),
        },
        WalkOff::Done { link } => match input.on_leg {
            Some((on, _)) if on == link => (state, Effect::ROUTE),
            // Off its leg: a release ahead (another link's) starts as from Idle.
            _ => step(WalkOff::Idle, input),
        },
        WalkOff::LetGo { link, since, from, along } => {
            let down = input.footing == Footing::Ground && input.speed < STILL_MS && input.feet.y < from.y - ARRIVE_HEIGHT_M;
            let end = if down {
                let on_landing =
                    input.feet.distance_xz(link.landing) <= ARRIVE_FLAT_M && (input.feet.y - link.landing.y).abs() <= ARRIVE_HEIGHT_M;
                Some(if on_landing { End::Landed(input.feet) } else { End::Overshot(input.feet) })
            } else if input.clock.wrapping_sub(since) as f32 * TICK_S > RELEASE_SETTLE_S {
                Some(End::TimedOut(input.feet))
            } else {
                None
            };
            match end {
                Some(end) => {
                    let after_ms = (input.clock.wrapping_sub(since) as f32 * TICK_S * 1000.0).round() as u32;
                    (WalkOff::Done { link }, Effect { ended: Some(Ended { link, from, along, after_ms, end }), ..Effect::ROUTE })
                }
                None => (state, let_go(link)),
            }
        }
    }
}

/// The log line of an ended let-go, its start, its end and its time in one (two lines of a kind within `nav_log::EVERY_MS`
/// would hold the second back): `nav: release walk-off from (..) to (..) at +X.XX m crouched|standing: let go at +Y.YY m (..),
/// after N ms landed|overshot|timed out (..)`. A let-go of the same link again shows as another line (or its held count).
pub fn line(ended: Ended) -> String {
    let Ended { link, from, along, after_ms, end } = ended;
    let p = |v: Vec3| format!("({:.2} {:.2} {:.2})", v.x, v.y, v.z);
    let (word, feet) = match end {
        End::Landed(feet) => ("landed", feet),
        End::Overshot(feet) => ("overshot", feet),
        End::TimedOut(feet) => ("timed out", feet),
    };
    format!(
        "{}release walk-off from {} to {} at +{:.2} m {}: let go at +{along:.2} m {}, after {after_ms} ms {word} {}",
        super::nav_log::PREFIX,
        p(link.take_off),
        p(link.landing),
        link.at,
        if link.crouched { "crouched" } else { "standing" },
        p(from),
        p(feet)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link() -> WalkOffRelease {
        WalkOffRelease { take_off: Vec3::new(0.0, 2.0, 0.0), landing: Vec3::new(0.5, 1.0, 0.0), at: 0.3, crouched: true }
    }

    fn input(on_leg: Option<f32>, feet: Vec3, footing: Footing, speed: f32, clock: u32) -> Input {
        Input { on_leg: on_leg.map(|along| (link(), along)), feet, footing, speed, clock }
    }

    /// Crouched on the leg, let go at the release, no move key until down and still on the landing.
    #[test]
    fn a_release_lets_go_at_its_point_and_ends_on_the_landing() {
        let top = Vec3::new(0.1, 2.0, 0.0);
        let (s, e) = step(WalkOff::Idle, input(Some(0.1), top, Footing::Ground, 2.5, 1));
        assert!(matches!(s, WalkOff::Approach { .. }) && e.crouch && !e.let_go);
        let (s, e) = step(s, input(Some(0.3), top, Footing::Ground, 2.5, 2));
        assert!(matches!(s, WalkOff::LetGo { .. }) && e.let_go);
        assert_eq!(e.keys(key::FORWARD | key::SPRINT), key::CROUCH);
        let (s, e) = step(s, input(None, Vec3::new(0.5, 1.5, 0.0), Footing::Air, 1.0, 3));
        assert!(matches!(s, WalkOff::LetGo { .. }) && e.let_go);
        let (s, e) = step(s, input(None, Vec3::new(0.5, 1.0, 0.0), Footing::Ground, 0.1, 4));
        assert_eq!(s, WalkOff::Done { link: link() });
        assert!(matches!(e.ended, Some(Ended { end: End::Landed(_), .. })));
    }

    /// Down on the landing with the route's progress still on the link's leg past `at`: no new let-go, the route's keys come
    /// back; off the leg, the machine is idle again.
    #[test]
    fn a_body_down_on_its_leg_is_not_let_go_again() {
        let down = Vec3::new(0.45, 1.0, 0.0);
        let (s, e) = step(WalkOff::Done { link: link() }, input(Some(0.45), down, Footing::Ground, 0.0, 10));
        assert_eq!(s, WalkOff::Done { link: link() });
        assert_eq!(e, Effect::ROUTE);
        assert_eq!(e.keys(key::FORWARD), key::FORWARD);
        let (s, _) = step(s, input(None, down, Footing::Ground, 0.0, 11));
        assert_eq!(s, WalkOff::Idle);
    }

    /// A body still on the top is not down: the let-go ends after the settle time.
    #[test]
    fn a_body_left_on_the_top_times_out() {
        let top = Vec3::new(0.3, 2.0, 0.0);
        let (mut s, _) = step(WalkOff::Idle, input(Some(0.35), top, Footing::Ground, 0.0, 0));
        let ticks = (RELEASE_SETTLE_S / TICK_S).ceil() as u32 + 1;
        for t in 1..=ticks {
            let (next, e) = step(s, input(None, top, Footing::Ground, 0.0, t));
            s = next;
            if let Some(ended) = e.ended {
                assert!(matches!(ended.end, End::TimedOut(_)));
                return;
            }
        }
        panic!("no end");
    }
}

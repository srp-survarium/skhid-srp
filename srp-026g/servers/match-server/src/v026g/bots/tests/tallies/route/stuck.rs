//! The bots' "is stuck" notes, and those of them by the walk keys alone; and what each note's reckoning-against-body gap was.
//!
//! A note is judged by the body 3 s on: it moved [`MOVED_M`] or more (a false alarm: the body was not stuck) or not (stuck). At the note
//! the gap between the bot's reckoned feet (its view at the last decision) and the real body is classed as `vertical` (the two on
//! floors [`VERTICAL_M`] or more apart in height), `horizontal` (across by [`HORIZONTAL_M`] or more, the body on a lip's floors or
//! not) or `none`.
//!
//! And what the body did over the stuck watch's window before the note (A196 recheck, the level tail: 72 % of the level notes on the
//! tip came on a route's first leg, 44 % within 2 s of a play switch or a replan), tick by tick ([`Window`]): out and back, held, or
//! crept; whether the view turned [`TURNED_DEG`] or more in it; how old the route it walked was ([`Age`]); and the route's leg the
//! note names. By the same judgement, so a kind of note that is mostly a false alarm shows as one.

use std::collections::{BTreeMap, VecDeque};

use super::*;

/// The body moved this far in the 3 s after the note: not stuck.
const MOVED_M: f32 = 1.0;
const AFTER_MS: u32 = 3_000;
/// The reckoning and the body this far apart in height: another floor.
const VERTICAL_M: f32 = 0.5;
/// This far apart across.
const HORIZONTAL_M: f32 = 0.3;
/// The window before a note: the stuck watch's (`stuck::STUCK_AFTER_MS`).
const WINDOW_MS: u32 = crate::v026g::bots::stuck::STUCK_AFTER_MS;
/// A window's walk this long or more that ends within [`NET_M`] of its start is out and back, metres.
const WALKED_M: f32 = 0.8;
/// The watch's own "still" (`stuck::STUCK_MOVED_M`).
const NET_M: f32 = crate::v026g::bots::stuck::STUCK_MOVED_M;
/// The view turned this far in the window: the body turned (back) on its way, degrees.
const TURNED_DEG: f32 = 90.0;
/// A route that began this recently before the note is new, ms.
const NEW_ROUTE_MS: u32 = 2_000;

/// What the body did over the window before a note, tick by tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Window {
    /// It walked [`WALKED_M`] or more and ended within [`NET_M`] of where it began: out and back.
    OutAndBack,
    /// It walked under [`NET_M`] in all.
    Held,
    /// Neither: it crept, or got off the spot.
    Crept,
}

impl Window {
    /// The window of the body's ticks (feet, view's yaw), oldest first, and whether the view turned [`TURNED_DEG`] or more from its
    /// first yaw at any tick.
    fn of(ticks: &[(Vec3, f32)]) -> (Self, bool) {
        let (Some(first), Some(last)) = (ticks.first(), ticks.last()) else {
            return (Self::Held, false);
        };
        let walked: f32 = ticks.windows(2).map(|w| w[0].0.distance_xz(w[1].0)).sum();
        let net = first.0.distance_xz(last.0);
        let turned = ticks.iter().any(|t| crate::v026g::bots::kin::wrap(t.1 - first.1).abs() >= TURNED_DEG.to_radians());
        let window = if walked < NET_M {
            Self::Held
        } else if walked >= WALKED_M && net < NET_M {
            Self::OutAndBack
        } else {
            Self::Crept
        };
        (window, turned)
    }

    fn name(self) -> &'static str {
        match self {
            Self::OutAndBack => "out_back",
            Self::Held => "held",
            Self::Crept => "crept",
        }
    }
}

/// How old the route the bot walked was at the note.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Age {
    /// Begun under [`NEW_ROUTE_MS`] before.
    New,
    Old,
    /// No route.
    None,
}

impl Age {
    fn name(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Old => "old",
            Self::None => "none",
        }
    }
}

/// What a note's first tick read, judged [`AFTER_MS`] on.
struct Read {
    feet: Vec3,
    /// `vertical_up` (the reckoning above the body), `vertical_down`, `horizontal_lip`, `horizontal`, `none`.
    gap: &'static str,
    window: Window,
    turned: bool,
    age: Age,
}

struct Pending {
    place: usize,
    time: u32,
    /// The route leg the note names: `first`, `later`, `none`.
    leg: &'static str,
    read: Option<Read>,
}

/// A route by what tells two apart (its length and its goal, bit for bit) and the time it was first seen.
type Seen = ((u32, [u32; 3]), u32);

#[derive(Default)]
pub(super) struct Stuck {
    stuck: (u32, u32),
    pending: Vec<Pending>,
    /// Notes by `<stuck|false_alarm>_<gap>`.
    gaps: BTreeMap<String, u32>,
    /// Notes by `<stuck|false_alarm>_<window>_<turned|straight>_<route age>`, and by `<stuck|false_alarm>_<leg>`.
    windows: BTreeMap<String, u32>,
    legs: BTreeMap<String, u32>,
    /// By place: the body's last ticks (time, feet, yaw), [`WINDOW_MS`] and a tick back; the route seen and since when.
    ticks: Vec<VecDeque<(u32, Vec3, f32)>>,
    routes: Vec<Option<Seen>>,
}

impl RouteTopic for Stuck {
    fn note(&mut self, _run: &RouteRun, note: &Note) {
        if note.text.starts_with("is stuck at") {
            self.stuck.0 += 1;
            self.stuck.1 += u32::from(note.text.contains("(walk keys held"));
            let leg = match note.text.split_once("[route leg ").and_then(|(_, rest)| rest.split_once('/')) {
                Some(("0", _)) => "first",
                Some(_) => "later",
                None => "none",
            };
            self.pending.push(Pending { place: note.place, time: note.time, leg, read: None });
        }
    }

    fn tick(&mut self, _run: &RouteRun, at: &TickAt<'_>) {
        if self.ticks.len() <= at.place {
            self.ticks.resize_with(at.place + 1, VecDeque::new);
            self.routes.resize(at.place + 1, None);
        }
        let ring = &mut self.ticks[at.place];
        ring.push_back((at.time, at.body.feet, at.body.yaw));
        while ring.front().is_some_and(|t| t.0 + WINDOW_MS + TICK_MS < at.time) {
            ring.pop_front();
        }
        let bot = at.bots.bots[at.place].as_ref();
        let route = bot.and_then(|bot| bot.route.as_ref()).map(|(route, goal)| (route.length().to_bits(), goal.to_array().map(f32::to_bits)));
        self.routes[at.place] = match (route, self.routes[at.place]) {
            (Some(key), Some((seen, since))) if seen == key => Some((seen, since)),
            (Some(key), _) => Some((key, at.time)),
            (None, _) => None,
        };
        let mut done = Vec::new();
        for (i, p) in self.pending.iter_mut().enumerate().filter(|(_, p)| p.place == at.place) {
            if p.read.is_none() && at.time >= p.time {
                let Some(bot) = bot else {
                    continue;
                };
                let (est, real) = (bot.bench_guard_at.1, at.body.feet);
                let dy = est.y - real.y;
                let lip = at.grid.locate(real, 1.0).is_some_and(|n| at.grid.near_lip(n) || at.grid.is_lip(n));
                let gap = if dy >= VERTICAL_M {
                    "vertical_up"
                } else if dy <= -VERTICAL_M {
                    "vertical_down"
                } else if est.distance_xz(real) >= HORIZONTAL_M {
                    if lip { "horizontal_lip" } else { "horizontal" }
                } else {
                    "none"
                };
                let window: Vec<(Vec3, f32)> =
                    self.ticks[at.place].iter().filter(|t| t.0 + WINDOW_MS >= p.time && t.0 <= p.time).map(|t| (t.1, t.2)).collect();
                let (window, turned) = Window::of(&window);
                let age = match self.routes[at.place] {
                    Some((_, since)) if p.time.saturating_sub(since) < NEW_ROUTE_MS => Age::New,
                    Some(_) => Age::Old,
                    None => Age::None,
                };
                p.read = Some(Read { feet: real, gap, window, turned, age });
            }
            if let Some(read) = &p.read
                && at.time >= p.time + AFTER_MS
            {
                let judged = if at.body.feet.distance_xz(read.feet) >= MOVED_M { "false_alarm" } else { "stuck" };
                *self.gaps.entry(format!("{judged}_{}", read.gap)).or_default() += 1;
                let turn = if read.turned { "turned" } else { "straight" };
                *self.windows.entry(format!("{judged}_{}_{turn}_{}", read.window.name(), read.age.name())).or_default() += 1;
                *self.legs.entry(format!("{judged}_{}", p.leg)).or_default() += 1;
                done.push(i);
            }
        }
        for i in done.into_iter().rev() {
            self.pending.swap_remove(i);
        }
    }

    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        line.main.push(format!("stuck {} stuck_keys {}", self.stuck.0, self.stuck.1));
        let joined = |map: &mut BTreeMap<String, u32>| {
            let all: Vec<String> = std::mem::take(map).into_iter().map(|(k, n)| format!("{k}={n}")).collect();
            if all.is_empty() { "-".to_owned() } else { all.join(",") }
        };
        line.tail.push(format!("stuck_note_gaps {}", joined(&mut self.gaps)));
        line.tail.push(format!("stuck_note_windows {}", joined(&mut self.windows)));
        line.tail.push(format!("stuck_note_legs {}", joined(&mut self.legs)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window walked 1 m out and 1 m back is out and back; one that stayed is held; one that went 0.6 m on crept. A view that turned
    /// round in it turned.
    #[test]
    fn a_window_is_out_and_back_held_or_crept() {
        let at = |x: f32, yaw: f32| (Vec3::new(x, 0.0, 0.0), yaw);
        let mut out_back: Vec<(Vec3, f32)> = (0..=10).map(|k| at(0.1 * k as f32, 0.0)).collect();
        out_back.extend((0..=10).map(|k| at(1.0 - 0.1 * k as f32, std::f32::consts::PI)));
        assert_eq!(Window::of(&out_back), (Window::OutAndBack, true));
        let held: Vec<(Vec3, f32)> = (0..30).map(|k| at(0.01 * (k % 3) as f32, 0.0)).collect();
        assert_eq!(Window::of(&held), (Window::Held, false));
        let crept: Vec<(Vec3, f32)> = (0..=12).map(|k| at(0.05 * k as f32, 0.0)).collect();
        assert_eq!(Window::of(&crept), (Window::Crept, false));
        assert_eq!(Window::of(&[]), (Window::Held, false));
    }
}

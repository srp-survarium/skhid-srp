//! The bench's count of teammates who draw the same human route at nearly the same time (`plays::roam`, A111). Counts only, test-only:
//! no decision reads them; the bench prints [`line`].
//!
//! A draw is a *twin* when a teammate other than the partner drew the same route of the same side not more than `N` s before (5 and
//! 15 s); a teammate's last draw stands for the run it holds (a run lasts longer than 15 s unless the bot dies or is given another
//! play, so a twin by this count may have left the route: the count is an upper bound for the longer window). A pair of twins (15 s)
//! is *stuck together* when, in the 60 s after the later draw, both bodies were alive and within 20 m of each other for more than 30 s
//! (the harm: two bots doing the work of one).

use std::cell::RefCell;
use std::collections::HashMap;

/// The windows of a twin draw, ms.
const WINDOWS_MS: [u32; 2] = [5_000, 15_000];
/// Twins count as together within this, metres.
const TOGETHER_M: f32 = 20.0;
/// … for more than this much of the run's first [`WATCH_MS`], ms.
const TOGETHER_MS: u32 = 30_000;
const WATCH_MS: u32 = 60_000;

/// A bot's latest draw.
#[derive(Clone, Copy)]
struct Draw {
    team: i32,
    side: i32,
    route: usize,
    at: u32,
}

/// A pair of twins (15 s) being watched.
struct Pair {
    a: (usize, u32),
    b: (usize, u32),
    from: u32,
    last: u32,
    together: u32,
    counted: bool,
}

#[derive(Default)]
struct Counts {
    draws: u64,
    twins: [u64; 2],
    pairs: u64,
    stuck: u64,
    last: HashMap<usize, Draw>,
    watched: Vec<Pair>,
}

thread_local! {
    static COUNTS: RefCell<Counts> = RefCell::new(Counts::default());
}

/// `place` of `team` drew route number `route` of `side`'s routes at `now`; `partner` is its Buddy mate.
pub fn drawn(place: usize, team: i32, partner: Option<usize>, side: i32, route: usize, now: u32) {
    COUNTS.with(|c| {
        let c = &mut *c.borrow_mut();
        c.draws += 1;
        let mut nearest: Option<(usize, Draw)> = None;
        for (&other, &d) in &c.last {
            let age = now.checked_sub(d.at);
            if other != place
                && Some(other) != partner
                && d.team == team
                && d.side == side
                && d.route == route
                && age.is_some_and(|age| age <= WINDOWS_MS[1])
                && nearest.is_none_or(|(_, n)| d.at > n.at)
            {
                nearest = Some((other, d));
            }
        }
        if let Some((other, d)) = nearest {
            let age = now - d.at;
            for (k, &w) in WINDOWS_MS.iter().enumerate() {
                c.twins[k] += u64::from(age <= w);
            }
            c.pairs += 1;
            c.watched.push(Pair {
                a: (other, d.at),
                b: (place, now),
                from: now,
                last: now,
                together: 0,
                counted: false,
            });
        }
        c.last.insert(place, Draw { team, side, route, at: now });
    });
}

/// One bench tick at `now`: the pairs' time together. `body(place)` is the body's feet and whether it is alive.
pub fn tick(now: u32, body: impl Fn(usize) -> Option<(mapdata::Vec3, bool)>) {
    COUNTS.with(|c| {
        let c = &mut *c.borrow_mut();
        let Counts { last, watched, stuck, .. } = c;
        watched.retain_mut(|p| {
            let same = last.get(&p.a.0).is_some_and(|d| d.at == p.a.1) && last.get(&p.b.0).is_some_and(|d| d.at == p.b.1);
            if !same || now < p.from || now - p.from > WATCH_MS {
                return false;
            }
            let dt = now - p.last;
            p.last = now;
            if let (Some((fa, alive_a)), Some((fb, alive_b))) = (body(p.a.0), body(p.b.0))
                && alive_a
                && alive_b
                && fa.distance_xz(fb) <= TOGETHER_M
            {
                p.together += dt;
                if p.together > TOGETHER_MS && !p.counted {
                    p.counted = true;
                    *stuck += 1;
                }
            }
            true
        });
    });
}

/// `draws N twin5 N twin15 N pairs N stuck30 N`, so far on this thread.
pub fn line() -> String {
    COUNTS.with(|c| {
        let c = c.borrow();
        format!(
            "draws {} twin5 {} twin15 {} pairs {} stuck30 {}",
            c.draws, c.twins[0], c.twins[1], c.pairs, c.stuck
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reset() {
        COUNTS.with(|c| *c.borrow_mut() = Counts::default());
    }

    #[test]
    fn a_teammate_on_the_same_route_within_the_window_is_a_twin() {
        reset();
        drawn(1, 0, None, 0, 3, 1_000);
        drawn(2, 0, None, 0, 3, 4_000); // 3 s: both windows
        drawn(3, 0, None, 0, 3, 14_500); // 10.5 s after 2: 15 s only
        drawn(4, 0, None, 0, 3, 40_000); // too late
        drawn(5, 0, None, 0, 4, 40_500); // another route
        drawn(6, 1, None, 0, 3, 40_600); // the other team
        drawn(7, 0, Some(4), 0, 3, 41_000); // its partner is the one on the route
        assert_eq!(line(), "draws 7 twin5 1 twin15 2 pairs 2 stuck30 0");
    }

    #[test]
    fn twins_that_stay_within_20_m_for_over_30_s_are_counted_once() {
        reset();
        drawn(1, 0, None, 0, 3, 0);
        drawn(2, 0, None, 0, 3, 1_000);
        let near = |_p: usize| Some((mapdata::Vec3::new(0.0, 0.0, 0.0), true));
        for now in (1_000..=40_000).step_by(100) {
            tick(now, near);
        }
        assert_eq!(line(), "draws 2 twin5 1 twin15 1 pairs 1 stuck30 1");
        reset();
        drawn(1, 0, None, 0, 3, 0);
        drawn(2, 0, None, 0, 3, 1_000);
        let apart = |p: usize| Some((mapdata::Vec3::new(p as f32 * 30.0, 0.0, 0.0), true));
        for now in (1_000..=40_000).step_by(100) {
            tick(now, apart);
        }
        assert_eq!(line(), "draws 2 twin5 1 twin15 1 pairs 1 stuck30 0");
        // A redraw by one of them ends the watch.
        reset();
        drawn(1, 0, None, 0, 3, 0);
        drawn(2, 0, None, 0, 3, 1_000);
        for now in (1_000..=20_000).step_by(100) {
            tick(now, near);
        }
        drawn(1, 0, None, 0, 5, 20_000);
        for now in (20_000..=60_000).step_by(100) {
            tick(now, near);
        }
        assert_eq!(line(), "draws 3 twin5 1 twin15 1 pairs 1 stuck30 0");
    }
}

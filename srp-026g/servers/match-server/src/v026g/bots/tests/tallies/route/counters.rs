//! The counts the bots' own code keeps in statics (`path::no_routes`, `escape::counts`, ...): the tally shows
//! the change since its start. A family is one [`Delta`] built by a function here and listed in `topics()`; a new
//! family is its function (its source, and how it reads on the line).

use super::*;

/// A family of counts: its source, its values at the start, and how the change since reads on the line.
pub(super) struct Delta {
    start: Vec<u64>,
    now: fn() -> Vec<u64>,
    show: fn(&[u64], &mut Line),
}

impl Delta {
    fn of(now: fn() -> Vec<u64>, show: fn(&[u64], &mut Line)) -> Box<dyn RouteTopic> {
        Box::new(Self { start: now(), now, show })
    }
}

impl RouteTopic for Delta {
    fn line(&mut self, _run: &RouteRun, line: &mut Line) {
        let change: Vec<u64> = (self.now)().iter().zip(&self.start).map(|(now, start)| now - start).collect();
        (self.show)(&change, line);
    }
}

/// Route requests that found no way: no way, searching, starved, searched, ended; and the ticks of bots given up in their pocket (`think.shut_in_gate`).
pub(super) fn no_route() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::path::no_routes().to_vec(),
        |d, line| {
            line.main.push(format!(
                "no_way {} searching {} starved {} searched {} ended {}",
                d[0], d[1], d[2], d[3], d[4]
            ));
            line.ends.push(format!("no_route_shut_in {}", d[5]));
        },
    )
}

/// The ways out of a zone: back, on, none; and those over a stronger zone than the start's and drops onto a zone's
/// floor.
pub(super) fn escapes() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::tactics::escapes::counts().to_vec(),
        |d, line| {
            line.main.push(format!("escapes_back {} escapes_on {} escapes_none {}", d[0], d[1], d[2]));
            line.ends.push(format!("escapes_cross_zone {} escapes_drop_zone {}", d[3], d[4]));
        },
    )
}

/// The motor's counts: jumps, look-ahead cuts, side steps, the lip guard.
pub(super) fn motor() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::motor::counts::counts().to_vec(),
        |d, line| {
            line.main.push(format!(
                "jumps_up_first {} jumps_up_later {} jumps_given_up {} ahead_cuts {} steps_clear {} steps_refused {} lip_guard_changed {} lip_guard_none {}",
                d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
            ));
        },
    )
}

/// The route requests of a poll found / failed by the floors' storey gap (`path::request_gap`).
pub(super) fn request_gaps() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::path::request_gaps().to_vec(),
        |d, line| {
            line.main.push(format!(
                "gap_found_same {} gap_found_above {} gap_found_below {} gap_failed_same {} gap_failed_above {} gap_failed_below {}",
                d[0], d[1], d[2], d[3], d[4], d[5]
            ));
        },
    )
}

/// The ticks off the route by the walked-leg rule alone (`path::below_walk`).
pub(super) fn below_walk() -> Box<dyn RouteTopic> {
    Delta::of(
        || vec![super::super::path::below_walk::counts()],
        |d, line| line.ends.push(format!("off_below_walk {}", d[0])),
    )
}

/// The rest points moved off a lip (`path::rests`).
pub(super) fn rests() -> Box<dyn RouteTopic> {
    Delta::of(
        || vec![super::super::path::rests::counts()],
        |d, line| line.ends.push(format!("rests_moved_off_lip {}", d[0])),
    )
}

/// The sights begun, those through half a metre of bush or more, and the bush metres of all
/// (`foliage::through`).
pub(super) fn foliage() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::foliage::through::counts().to_vec(),
        |d, line| {
            line.ends.push(format!(
                "foliage_sights {} foliage_sights_through {} foliage_mm {} foliage_added_ms {}",
                d[0], d[1], d[2], d[3]
            ));
        },
    )
}

/// Bots that killed themselves with nothing left to try, the second tries, and those barred after two
/// (`stuck::self_kills`).
pub(super) fn self_kills() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::stuck::self_kills::counts().to_vec(),
        |d, line| {
            line.ends.push(format!(
                "self_kills {} self_kills_again {} self_kills_barred {}",
                d[0], d[1], d[2]
            ));
        },
    )
}

/// Route searches: all, from a pocket's floor, within a second of the bot's last, and the ones a bot given up in
/// its pocket was spared (`search::asks`), and the episodes of bots shut in.
pub(super) fn asks() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::search::asks::counts().to_vec(),
        |d, line| {
            line.ends.push(format!(
                "asks {} asks_pocket {} asks_again_1s {} asks_skipped {} shut_in_bots {}",
                d[0], d[1], d[2], d[3], d[4]
            ));
        },
    )
}

/// Each ending of a route: how many, and their mean age in ms (`route_end`).
pub(super) fn route_ends() -> Box<dyn RouteTopic> {
    Delta::of(
        || {
            super::super::route_end::counts::counts()
                .iter()
                .flat_map(|&(n, age)| [n, age])
                .collect()
        },
        |d, line| {
            for (k, why) in super::super::route_end::RouteEnd::ALL.into_iter().enumerate() {
                let (n, age) = (d[2 * k], d[2 * k + 1]);
                line.ends.push(format!(
                    "route_end_{} {} route_end_{}_age_ms {}",
                    why.name(),
                    n,
                    why.name(),
                    age / n.max(1)
                ));
            }
        },
    )
}

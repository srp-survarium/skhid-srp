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

/// The motor's counts: jumps, look-ahead cuts, side steps, the lip guard; and (at the tail) the walk ticks whose look-ahead ended on a point the body does not walk to in a line, and what the walk did about it.
pub(super) fn motor() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::motor::counts::counts().to_vec(),
        |d, line| {
            line.main.push(format!(
                "jumps_up_first {} jumps_up_later {} jumps_given_up {} ahead_cuts {} steps_clear {} steps_refused {} lip_guard_changed {} lip_guard_none {}",
                d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
            ));
            line.tail.push(format!("look_unclear {} look_unclear_back {} look_unclear_off {}", d[8], d[9], d[10]));
        },
    )
}

/// The reflex's own side steps refused at a lip and asked the other side (`drive_tick`, `clear_step_sees_lip`):
/// the other side clear, or both refused (the bot stands), standing to aim or weaving a walk, from a floor in a
/// lip's band or out of it; and the steps taken to the other side (`clear_step_slides_along`).
pub(super) fn side_slides() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::motor::counts::slides().to_vec(),
        |d, line| {
            line.ends.push(format!(
                "side_other_clear_still_out {} side_other_clear_still_band {} side_other_clear_walk_out {} side_other_clear_walk_band {} side_both_refused_still_out {} side_both_refused_still_band {} side_both_refused_walk_out {} side_both_refused_walk_band {} side_slid {}",
                d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7], d[8]
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

/// The shots bots heard (`sense::shot_heard`): by the weapon's reach, at the fallback of an unknown weapon, from
/// over the flat range.
pub(super) fn shots_heard() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::sense::shots_heard().to_vec(),
        |d, line| {
            line.main.push(format!(
                "shots_heard_table {} shots_heard_unknown {} shots_heard_far {}",
                d[0], d[1], d[2]
            ));
        },
    )
}

/// The holder's picks with the stand-sight rule on, those that kept spots seeing the stand, and
/// those that fell back to the box rule (`guard_lead::stand_sight`).
pub(super) fn holder_stand_sight() -> Box<dyn RouteTopic> {
    Delta::of(
        || crate::v026g::bots::plays::guard_lead::stand_sight::counts().to_vec(),
        |d, line| {
            line.main.push(format!(
                "holder_stand_picks {} holder_stand_kept {} holder_stand_fallback {}",
                d[0], d[1], d[2]
            ));
        },
    )
}

/// A battery's crossings weighed with the pools routed (F2): asked, borne, refused (`zone_cost::asks`).
pub(super) fn zone_pool_asks() -> Box<dyn RouteTopic> {
    Delta::of(
        || crate::v026g::bots::zone_cost::asks::counts().to_vec(),
        |d, line| {
            line.main.push(format!(
                "zone_cross_asked {} zone_cross_borne {} zone_cross_refused {}",
                d[0], d[1], d[2]
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

/// Ticks the sprint was let go of before a planned drop (`motor::drop_walks`).
pub(super) fn drop_walks() -> Box<dyn RouteTopic> {
    Delta::of(
        || vec![super::super::motor::drop_walks::ticks()],
        |d, line| line.ends.push(format!("drop_walk_ticks {}", d[0])),
    )
}

/// The sprint gap links of routes (`motor::sprint_gaps`): pressed by the speed at the press (under 3.5, 3.5-4.5, 4.5
/// and over m/s) and given up.
pub(super) fn sprint_gaps() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::motor::sprint_gaps::counts().to_vec(),
        |d, line| {
            line.ends.push(format!("sprint_gaps_pressed {}/{}/{} sprint_gaps_gave_up {}", d[0], d[1], d[2], d[3]));
        },
    )
}

/// The rests (still 0.5 s or more) on a lip or within the body's radius of one, and the seconds spent in them
/// (`rest::stops`): the owner's symptom, on and off.
pub(super) fn rest_stops() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::rest::stops::counts().to_vec(),
        |d, line| line.ends.push(format!("rests_on_lip {} rest_seconds_on_lip {:.1}", d[0], d[1] as f32 / 1000.0)),
    )
}

/// The sprint jumps of routes by 0.05 m band of rise (`motor::sprint_jumps`): pressed / got up / given up.
pub(super) fn sprint_jumps() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::motor::sprint_jumps::counts().iter().flatten().copied().collect(),
        |d, line| {
            let band = |k: usize| d[k * 6..k * 6 + 6].iter().map(u64::to_string).collect::<Vec<_>>().join("/");
            line.ends.push(format!("sprint_jumps_pressed {} sprint_jumps_up {} sprint_jumps_gave_up {}", band(0), band(1), band(2)));
        },
    )
}

/// The bots' reckoned stamina (`stamina::counts`): lockouts begun, ticks locked out, of those with the sprint keys held
/// (the harm), jump presses the bar refused, sprints let go at the empty bar, sprint segments short of the bar when
/// first seen, and the seconds of sprint saved for them.
pub(super) fn stamina() -> Box<dyn RouteTopic> {
    Delta::of(
        || {
            let mut v = super::super::stamina::counts::counts().to_vec();
            v.push(super::super::stamina::counts::saved_ms());
            v
        },
        |d, line| {
            line.ends.push(format!(
                "stamina_lockouts {} stamina_low_ticks {} stamina_sprint_keys_low {} stamina_jumps_refused {} stamina_released_low {} segments_short_of_stamina {} sprint_saved_s {:.1}",
                d[0], d[1], d[2], d[3], d[4], d[5], d[6] as f32 / 1000.0
            ));
        },
    )
}

/// The landings of route jumps by the carried run's angle from the way (`motor::landings`): seen / braked, bands
/// 0-30, 30-60, 60-90, over 90 degrees, and walls within the coast.
pub(super) fn landings() -> Box<dyn RouteTopic> {
    Delta::of(
        || {
            let (jumps, drops) = (super::super::motor::landings::counts(), super::super::motor::landings::drop_counts());
            jumps.iter().chain(&drops).flatten().copied().collect()
        },
        |d, line| {
            let bands = |k: usize| d[k * 5..k * 5 + 5].iter().map(u64::to_string).collect::<Vec<_>>().join("/");
            line.ends.push(format!(
                "landings {} landings_cancelled {} drop_landings {} drop_landings_cancelled {}",
                bands(0),
                bands(1),
                bands(2),
                bands(3)
            ));
        },
    )
}

/// The plans of a body on no floor of the grid, from the floor it drops to (`path::stranded`).
pub(super) fn stranded() -> Box<dyn RouteTopic> {
    Delta::of(
        || vec![super::super::path::stranded::counts()],
        |d, line| line.ends.push(format!("plans_from_stranded_stand {}", d[0])),
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

/// The worlds that came 3 s or more after the one before: all, the stuck notes and the off-reckoning notes that
/// came with one, the stuck checks a tuned gap ended, and the stuck notes with an enemy, and with a friend, within 1.2 m (`stuck::fix_gaps`).
pub(super) fn fix_gaps() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::stuck::fix_gaps::counts().to_vec(),
        |d, line| {
            line.ends.push(format!(
                "fix_gaps {} fix_gap_stuck_notes {} fix_gap_off_notes {} fix_gap_checks_ended {} stuck_beside_enemy {} stuck_beside_friend {}",
                d[0], d[1], d[2], d[3], d[4], d[5]
            ));
        },
    )
}

/// Bodies locked by a hard fall and the worlds the stuck watch kept quiet for them (`stuck::fall_locks`).
pub(super) fn fall_locks() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::stuck::fall_locks::counts().to_vec(),
        |d, line| line.ends.push(format!("fall_locks {} fall_lock_worlds {}", d[0], d[1])),
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

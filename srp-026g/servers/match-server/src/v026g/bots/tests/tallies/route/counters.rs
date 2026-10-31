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
            line.tail.push(format!(
                "look_unclear {} look_unclear_back {} look_unclear_off {} look_cut_false {} look_unclear_floor {} look_unclear_link {}",
                d[8], d[9], d[10], d[11], d[12], d[13]
            ));
        },
    )
}

/// The lip guard's gaps ([`Feel::lip_guard_full`](super::super::motor::Feel::lip_guard_full)): ticks asked for a glide with no move key
/// and those changed, ticks asked only for the floors about the reckoned feet and those changed, walks held for the feet to either side,
/// bots given up and watched again, glide ticks changed while given up.
pub(super) fn lip_gaps() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::motor::counts::lip_gaps().to_vec(),
        |d, line| {
            line.ends.push(format!(
                "lip_glide_asked {} lip_glide_changed {} lip_about_asked {} lip_about_changed {} lip_side_held {} lip_gave_up {} lip_watched_again {} lip_given_up_glide_changed {}",
                d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]
            ));
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

/// A battery's crossings by a route over the zones' floors (`collect::routed_asks`): weighed, borne, refused, begun,
/// aborted.
pub(super) fn route_crossings() -> Box<dyn RouteTopic> {
    Delta::of(
        || crate::v026g::bots::plays::collect::routed_asks::counts().to_vec(),
        |d, line| {
            line.main.push(format!(
                "route_cross_asked {} route_cross_borne {} route_cross_refused {} route_cross_begun {} route_cross_aborted {}",
                d[0], d[1], d[2], d[3], d[4]
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

/// What `think.wall_clear_corners` did (`path::wall_clear_counts`, A191b): the routes it looked at, the corners under the hold and
/// how many it moved or kept, the legs dipping under the radius and how many it gave a corner or kept, and the metres it added.
/// Printed only when it looked at a route.
pub(super) fn wall_clear() -> Box<dyn RouteTopic> {
    use super::super::path::wall_clear_counts::{NAMES, counts};
    Delta::of(counts, |d, line| {
        if d[0] > 0 {
            let fields: Vec<String> = NAMES.iter().zip(d).map(|(name, n)| format!("{name} {n}")).collect();
            line.tail.push(format!("wall_clear {}", fields.join(" ")));
        }
    })
}

/// The places `think.wall_clear_corners` kept a corner or a dipping leg as it was (no floor with room), in half metres, once each:
/// `wall_clear_kept corner:x,y,z ... leg:x,y,z ...` (A191b: the stuck notes near them are the next cause).
pub(super) fn wall_clear_kept() -> Box<dyn RouteTopic> {
    struct Kept;
    impl RouteTopic for Kept {
        fn line(&mut self, _run: &RouteRun, line: &mut Line) {
            let kept = super::super::path::wall_clear_counts::take_kept();
            if !kept.is_empty() {
                let places: Vec<String> = kept
                    .iter()
                    .map(|(leg, p)| format!("{}:{},{},{}", if *leg { "leg" } else { "corner" }, p[0], p[1], p[2]))
                    .collect();
                line.tail.push(format!("wall_clear_kept {}", places.join(" ")));
            }
        }
    }
    Box::new(Kept)
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

/// The contacts (`contact::contacts`): walks of the route's with the forward key held and no speed for 0.5 s, the
/// seconds spent in them, and the five spots (1 m bins: x z, y in 2 m) with the most of them so far (A66, doors).
pub(super) fn contacts() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::contact::contacts::counts().to_vec(),
        |d, line| {
            let top = super::super::contact::contacts::top(5)
                .iter()
                .map(|&((x, z, y), n)| format!("{x}/{z}/{}:{n}", y * 2))
                .collect::<Vec<_>>()
                .join(" ");
            line.ends.push(format!("contacts {} contact_seconds {:.1} contact_spots {top}", d[0], d[1] as f32 / 1000.0));
        },
    )
}

/// The stand-in's moves under the client's capsule rule (`standin_capsule::counts`, A191): moves of a body walking, those that met a
/// wall, the met ones that made under a tenth of the move (stopped) or under nine tenths (slowed), and bodies pushed off a wall.
pub(super) fn capsule_walls() -> Box<dyn RouteTopic> {
    Delta::of(
        || crate::v026g::bots::tests::standin_capsule::counts::counts().to_vec(),
        |d, line| line.ends.push(format!("capsule_moves {} capsule_met_wall {} capsule_stopped {} capsule_slowed {} capsule_pushed {}", d[0], d[1], d[2], d[3], d[4])),
    )
}

/// The sprint at planned drops (`motor::drop_walks`): ticks let go of before a drop unsafe at a sprint, ticks skipped as
/// safe, and hard falls after a drop crossed at a sprint.
pub(super) fn drop_walks() -> Box<dyn RouteTopic> {
    Delta::of(
        || {
            let mut all = super::super::motor::drop_walks::counts();
            all.extend(super::super::motor::drop_walks::episodes());
            all
        },
        |d, line| {
            use super::super::motor::drop_walks::{SLICES, quantile};
            let q = |k: usize, width: f32| {
                let (from, len) = SLICES[k];
                format!("{:.1}/{:.1}", quantile(&d[from..from + len], width, 0.5), quantile(&d[from..from + len], width, 0.9))
            };
            let e = d.len() - 2;
            line.ends.push(format!(
                "drop_walk_ticks released_unsafe/released_unknown {}/{} skipped_descent_by_walk {} drop_walk_safe_ticks {} drop_sprinted_hard_falls {} \
                 drop_walk_episodes unsafe/unknown {}/{} no_edge {} unresolved {} \
                 release_to_edge_s_p50/p90 {} release_to_edge_m_p50/p90 {} edge_to_sprint_s_p50/p90 {} landing_to_sprint_s_p50/p90 {}",
                d[0], d[1], d[5], d[6], d[7],
                d[e], d[e + 1], d[14], d[15],
                q(0, 0.1), q(1, 0.25), q(2, 0.1), q(3, 0.1)
            ));
        },
    )
}

/// The sprint gap links of routes (`motor::sprint_gaps`): pressed by the speed at the press (under 3.5, 3.5-4.5, 4.5
/// and over m/s), given up, pressed with the sprint key held / not, and got across.
pub(super) fn sprint_gaps() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::motor::sprint_gaps::counts().to_vec(),
        |d, line| {
            line.ends.push(format!(
                "sprint_gaps_pressed {}/{}/{} sprint_gaps_gave_up {} sprint_gaps_press_sprint_held/not {}/{} sprint_gaps_across {}",
                d[0], d[1], d[2], d[3], d[4], d[5], d[6]
            ));
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

/// The stand-in's body past a ledge's edge with no drop link ahead (`SRP_BENCH_STAND_IN_FALL`, `standin_jump::past_edge`): walked on
/// within the support, fell to the floor below, no floor below.
pub(super) fn edge_falls() -> Box<dyn RouteTopic> {
    Delta::of(
        crate::v026g::bots::tests::standin_jump::edge_fall_stats,
        |d, line| {
            line.ends.push(format!("stand_in_past_edge walked/fell/no_floor {}/{}/{}", d[0], d[1], d[2]));
        },
    )
}

/// The sprint lift (`motor::lifts`), by band of rise (up to 0.80 m, 0.80-0.95, 0.95-1.05, 1.05-1.11): lifts tried (the body
/// stood at the face), got up, given up for the jump; the jumps pressed up those faces with the body calm and in a fight
/// or a rush; and the faces a body in a fight or a rush did not lift (top short, sprint blocked, no run).
pub(super) fn lifts() -> Box<dyn RouteTopic> {
    Delta::of(
        || {
            let mut all: Vec<u64> = super::super::motor::lifts::counts().iter().flatten().copied().collect();
            all.extend(super::super::motor::lifts::refusals());
            all.extend(crate::v026g::bots::tests::standin_jump::lift_stats());
            all.extend(super::super::motor::lifts::key_counts());
            all.extend(super::super::motor::lifts::motor_off_counts());
            all.extend(super::super::motor::lifts::left_counts().iter().flatten().copied());
            all
        },
        |d, line| {
            let band = |k: usize| d[k * 4..k * 4 + 4].iter().map(u64::to_string).collect::<Vec<_>>().join("/");
            line.ends.push(format!(
                "lifts_tried {} lifts_up {} lifts_failed {} lift_face_jumps_calm {} lift_face_jumps_urgent {} lifts_failed_blocked {} lift_refused top_short/blocked/no_run {}/{}/{}",
                band(0), band(1), band(2), band(3), band(4), band(5), d[24], d[25], d[26]
            ));
            // The stand-in game's side of the lift by band of the link's rise (up to 0.80, 0.80-0.95, 0.95-1.05, 1.05-1.11, over
            // 1.11, none): made / released the keys / too high / top short / no link / came slow.
            let stand = (0..6)
                .map(|b| d[27 + b * 6..33 + b * 6].iter().map(u64::to_string).collect::<Vec<_>>().join("/"))
                .collect::<Vec<_>>()
                .join(" ");
            line.ends.push(format!("stand_in_lift made/released/too_high/top_short/no_link/slow by band {stand}"));
            let k = &d[27 + 36..27 + 36 + 9];
            line.ends.push(format!(
                "lift_face_ticks aimed_walk/motor_off/side_step/lip_guard/pace_stance/fire/override_keys/lining/held {}/{}/{}/{}/{}/{}/{}/{}/{}",
                k[0], k[1], k[2], k[3], k[4], k[5], k[6], k[7], k[8]
            ));
            let left = (0..4)
                .map(|k| d[27 + 36 + 9 + 6 + k * 4..27 + 36 + 9 + 6 + k * 4 + 4].iter().map(u64::to_string).collect::<Vec<_>>().join("/"))
                .collect::<Vec<_>>()
                .join(" ");
            line.ends.push(format!("lifts_left top_short/blocked/no_run/route by band {left}"));
            let m = &d[27 + 36 + 9..27 + 36 + 9 + 6];
            line.ends.push(format!(
                "lift_face_motor_off brake/standing/no_jump/tired/not_sprinting/no_forward {}/{}/{}/{}/{}/{}",
                m[0], m[1], m[2], m[3], m[4], m[5]
            ));
        },
    )
}

/// What each kind of link costs in time (`link_time`, A55): per kind, the links timed (up, and back at 90 % of the speed it
/// began at), the mean time, the mean time lost against the same path at the starting speed, the path over the straight
/// line, the presses and back-offs per link, the seconds walking back, the links got up by the first press (or with none),
/// and the links up but not at speed after 6 s / ended by the next link / given up on; then the time lost by 0.1 m band of
/// rise (a drop: by metre of height) as `band:links/mean_lost_s`.
pub(super) fn link_times() -> Box<dyn RouteTopic> {
    use super::super::link_time::{BANDS, FIELD_COUNT, Field, Kind, Why, at};
    Delta::of(
        super::super::link_time::counts,
        |d, line| {
            let sum = |kind: Kind, field: Field| (0..BANDS).map(|b| d[at(kind, b, field)]).sum::<u64>() as f32;
            let per = |x: f32, n: f32| if n > 0.0 { x / n } else { 0.0 };
            for kind in Kind::ALL {
                let n = sum(kind, Field::Links);
                let (slow, cut, gone) = (sum(kind, Field::Slow), sum(kind, Field::Cut), sum(kind, Field::Abandoned));
                if n + slow + cut + gone == 0.0 {
                    continue;
                }
                let name = Kind::NAMES[kind as usize];
                line.ends.push(format!(
                    "links_{name} n {n} mean_s {:.2} lost_s {:.2} path_over_straight {:.2} presses {:.2} backoffs {:.2} backoff_s {:.2} first_up_pct {:.0} lifted {} slow {slow} cut {cut} given_up {gone} pre_s {:.2} air_s {:.2} post_s {:.2} pre_stopped_s {:.2} pre_slowed_s {:.2} sprint_starts_pct {:.0} gone_silent {} gone_no_jump {} gone_far {} gone_s {:.2}",
                    per(sum(kind, Field::Ms), n) / 1000.0,
                    per(sum(kind, Field::ExcessMs), n) / 1000.0,
                    per(sum(kind, Field::PathMm), sum(kind, Field::StraightMm)),
                    per(sum(kind, Field::Presses), n),
                    per(sum(kind, Field::Backoffs), n),
                    per(sum(kind, Field::BackoffMs), n) / 1000.0,
                    100.0 * per(sum(kind, Field::FirstUp), n),
                    sum(kind, Field::LiftedUp),
                    per(sum(kind, Field::PreMs), n) / 1000.0,
                    per(sum(kind, Field::AirMs), n) / 1000.0,
                    per(sum(kind, Field::PostMs), n) / 1000.0,
                    per(sum(kind, Field::StoppedMs), n) / 1000.0,
                    per(sum(kind, Field::SlowedMs), n) / 1000.0,
                    100.0 * per(sum(kind, Field::SprintStarts), n),
                    sum(kind, Field::GoneSilent),
                    sum(kind, Field::GoneNoJump),
                    sum(kind, Field::GoneFar),
                    per(sum(kind, Field::GoneMs), gone) / 1000.0,
                ));
                let base = Kind::ALL.len() * BANDS * FIELD_COUNT;
                let by_why = Why::ALL
                    .iter()
                    .map(|&w| {
                        let at = base + (kind as usize * 5 + w as usize) * 2;
                        format!("{}:{:.2}/{:.2}", Why::NAMES[w as usize], per(d[at] as f32, n) / 1000.0, per(d[at + 1] as f32, n) / 1000.0)
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                line.ends.push(format!("links_{name}_pre_by_why stopped/slowed_s {by_why}"));
                let ended_base = base + Kind::ALL.len() * Why::ALL.len() * 2 + kind as usize * (super::super::link_time::ENDED_NONE + 1);
                let ended = (0..=super::super::link_time::ENDED_NONE)
                    .filter(|&e| d[ended_base + e] > 0)
                    .map(|e| {
                        let name = super::super::route_end::RouteEnd::ALL.get(e).map_or("route_kept", |end| end.name());
                        format!("{name}:{}", d[ended_base + e])
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                line.ends.push(format!("links_{name}_given_up_by_route_end {ended}"));
                let bands = (0..BANDS)
                    .filter(|&b| d[at(kind, b, Field::Links)] > 0)
                    .map(|b| {
                        let links = d[at(kind, b, Field::Links)] as f32;
                        format!("{b}:{links}/{:.2}", d[at(kind, b, Field::ExcessMs)] as f32 / links / 1000.0)
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                line.ends.push(format!("links_{name}_by_band {bands}"));
            }
            // The climbs of a face the lift reaches (0.8-1.2 m of rise, by 0.1 m band), whatever way they are taken: walking jumps,
            // sprint jumps, sprint-throughs and lifts together, in sums (links, ms, ms lost against the cruise, given up, cut
            // by the next link, up but slow, ms stopped before the press), so the lift's switch is judged on the whole face.
            for b in 3..=6 {
                let pooled = |field: Field| {
                    [Kind::Walking, Kind::SprintBand, Kind::Through, Kind::Lift].iter().map(|&k| d[at(k, b, field)]).sum::<u64>()
                };
                line.ends.push(format!(
                    "links_face_rise_{:.1}_to_{:.1} n {} ms {} lost_ms {} given_up {} cut {} slow {} pre_ms {} stopped_ms {}",
                    0.5 + 0.1 * b as f32,
                    0.6 + 0.1 * b as f32,
                    pooled(Field::Links),
                    pooled(Field::Ms),
                    pooled(Field::ExcessMs),
                    pooled(Field::Abandoned),
                    pooled(Field::Cut),
                    pooled(Field::Slow),
                    pooled(Field::PreMs),
                    pooled(Field::StoppedMs),
                ));
            }
            // The faces of 0.8-1.2 m given up, by method (kind), what had become of them (a jump pressed, the body left at the
            // face) and the route end that followed.
            let face_ended_base = Kind::ALL.len() * (BANDS * FIELD_COUNT + Why::ALL.len() * 2 + super::super::link_time::ENDED_NONE + 1);
            for kind in [Kind::Walking, Kind::SprintBand, Kind::Through, Kind::Lift] {
                let pooled = |field: Field| (3..=6).map(|b| d[at(kind, b, field)]).sum::<u64>();
                let given = pooled(Field::Abandoned);
                if given == 0 {
                    continue;
                }
                let ended_at = face_ended_base + kind as usize * (super::super::link_time::ENDED_NONE + 1);
                let ended = (0..=super::super::link_time::ENDED_NONE)
                    .filter(|&e| d[ended_at + e] > 0)
                    .map(|e| {
                        let name = super::super::route_end::RouteEnd::ALL.get(e).map_or("route_kept", |end| end.name());
                        format!("{name}:{}", d[ended_at + e])
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                line.ends.push(format!(
                    "links_face_given_up_{} n {given} pressed {} near_take_off {} silent {} no_jump {} far {} ended_by {ended}",
                    Kind::NAMES[kind as usize],
                    pooled(Field::GonePressed),
                    pooled(Field::GoneNear),
                    pooled(Field::GoneSilent),
                    pooled(Field::GoneNoJump),
                    pooled(Field::GoneFar),
                ));
            }
            // What became of the links of a 0.8-1.2 m face given up on, followed for 10 s: by method and route end, climbed
            // anyway / went elsewhere / still within 6 m of the take-off, and the seconds within 3 m of it in those 10 s.
            let ended_n = super::super::link_time::ENDED_NONE + 1;
            let ghost_base = face_ended_base + Kind::ALL.len() * ended_n;
            for kind in [Kind::Walking, Kind::SprintBand, Kind::Through, Kind::Lift] {
                let parts = (0..ended_n)
                    .filter(|&e| (0..3).any(|s| d[ghost_base + (kind as usize * ended_n + e) * 4 + s] > 0))
                    .map(|e| {
                        let at = ghost_base + (kind as usize * ended_n + e) * 4;
                        let name = super::super::route_end::RouteEnd::ALL.get(e).map_or("route_kept", |end| end.name());
                        format!("{name}:{}/{}/{}/{:.1}s", d[at], d[at + 1], d[at + 2], d[at + 3] as f32 / 1000.0)
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                if !parts.is_empty() {
                    line.ends.push(format!("links_face_given_up_then_{} climbed/elsewhere/near/near_s_by_route_end {parts}", Kind::NAMES[kind as usize]));
                }
            }
            // The first press of the walking and sprint jumps at a 0.8-1.2 m face, by outcome (got up / pressed and not up).
            let press_base = ghost_base + Kind::ALL.len() * ended_n * 4;
            let bins = super::super::link_time::PRESS_BINS;
            let metrics = super::super::link_time::PRESS_METRICS;
            for (k, name) in ["walking", "sprint_band"].iter().enumerate() {
                for (o, outcome) in ["up", "not_up"].iter().enumerate() {
                    let at = press_base + (k * 2 + o) * metrics * bins;
                    let m = |i: usize| d[at + i * bins..at + (i + 1) * bins].iter().map(u64::to_string).collect::<Vec<_>>().join("/");
                    line.ends.push(format!(
                        "press_{name}_{outcome} before_m(0.25) {} side_m(0.1) {} speed(0.5) {} top_m(0.5) {} rise_from_0.8(0.05) {}",
                        m(0),
                        m(1),
                        m(2),
                        m(3),
                        m(4)
                    ));
                }
            }
            debug_assert_eq!(
                d.len(),
                Kind::ALL.len() * (BANDS * FIELD_COUNT + Why::ALL.len() * 2 + 2 * ended_n) + Kind::ALL.len() * ended_n * 4 + 2 * 2 * metrics * bins
            );
        },
    )
}

/// The sprint-band ledge jumps (`motor::band_jumps`): presses by ground speed (under 3.5, 3.5-4, 4-4.5, 4.5 and over m/s)
/// and with no sprint held (the activity), the arrivals backed off by reason (too slow, no sprint key, or what the client's
/// `can_sprint` says no to), the links given up for want of a run, and the ticks the body stood at a face with no press
/// and the faces picked again within 10 s of giving them up (the harm; the jumps given up are in
/// `sprint_jumps_gave_up`).
pub(super) fn band_jumps() -> Box<dyn RouteTopic> {
    use super::super::motor::band_jumps::Slot::{self, *};
    Delta::of(
        || super::super::motor::band_jumps::counts().to_vec(),
        |d, line| {
            let at = |slot: Slot| d[slot as usize];
            let refused = [RefusedSlow, RefusedNoKey, RefusedAiming, RefusedBusy, RefusedReloading, RefusedStaggered, RefusedTired, RefusedCarrying, RefusedDirection]
                .map(|slot| at(slot).to_string())
                .join("/");
            line.ends.push(format!(
                "band_press_speed {}/{}/{}/{} band_press_no_sprint {} band_refused_by slow/no_key/aiming/busy/reload/stagger/tired/carrying/direction {refused} \
                 band_no_run_up {} band_face_still_ticks {} band_repicked {} band_strict_up/not_up {}/{} band_held_off/hold_over/pressed_after_hold {}/{}/{}",
                at(PressUnder35),
                at(Press35To40),
                at(Press40To45),
                at(Press45Up),
                at(PressNoSprint),
                at(NoRunUp),
                at(FaceStill),
                at(Repicked),
                at(StrictUp),
                at(StrictNotUp),
                at(HeldOff),
                at(HoldOver),
                at(PressedAfterHold)
            ));
            // The faces with the most presses and backed-off arrivals so far (all the run's, not this tally's change).
            let faces = super::super::motor::band_jumps::top_faces(6)
                .into_iter()
                .map(|((x, z, y), n)| format!("{x}/{z}/{}:{}/{}/{}/{}", y * 2, n[0], n[1], n[2], n[3]))
                .collect::<Vec<_>>()
                .join(" ");
            line.ends.push(format!("band_faces x/z/y:press/refused/up/gave_up {faces}"));
        },
    )
}

/// The bots' reckoned stamina (`stamina::counts`): lockouts begun, ticks locked out, of those with the sprint keys held
/// (the harm), jump presses the bar refused, sprints let go at the empty bar, sprint segments short of the bar when
/// first seen, the ticks a tired body was kept walking into a sprint jump, the sprint gap segments short only by the gap's
/// run, and the seconds of sprint saved for them.
pub(super) fn stamina() -> Box<dyn RouteTopic> {
    Delta::of(
        || {
            let mut v = super::super::stamina::counts::counts().to_vec();
            v.push(super::super::stamina::counts::saved_ms());
            v
        },
        |d, line| {
            line.ends.push(format!(
                "stamina_lockouts {} stamina_low_ticks {} stamina_sprint_keys_low {} stamina_jumps_refused {} stamina_released_low {} segments_short_of_stamina {} sprint_jump_kept_walking {} gap_run_short {} sprint_saved_s {:.1}",
                d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7], d[8] as f32 / 1000.0
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

/// With `sense.grass_notice` on: the sights begun, those through half a metre of grass or more, the
/// grass metres of all, and the delay the grass added (`grass::through`).
pub(super) fn grass() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::grass::through::counts().to_vec(),
        |d, line| {
            line.ends.push(format!(
                "grass_sights {} grass_sights_through {} grass_mm {} grass_added_ms {}",
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

/// The drops a body held at their top shut to its searches (`unstick.drop_shut`, `stuck::drop_shuts`): the shuts, the
/// landings shut, and what followed each: stuck again at the spot within 10 s, off it (down, or 2 m away), still there
/// after 30 s, killed itself first (the rest: a new life or another shut ended the watch).
pub(super) fn drop_shuts() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::stuck::drop_shuts::counts().to_vec(),
        |d, line| {
            line.ends.push(format!(
                "drop_shuts {} drop_shut_landings {} drop_shut_again {} drop_shut_left {} drop_shut_still {} drop_shut_self_kills {}",
                d[0], d[1], d[2], d[3], d[4], d[5]
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

/// Windows of the stuck watch let go for a way the bot had no reckoning of, and the stucks that followed them at the same
/// spot (`stuck::unknown_meant`).
pub(super) fn unknown_meant() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::stuck::unknown_meant::counts().to_vec(),
        |d, line| {
            line.ends.push(format!(
                "unknown_meant_ended {} unknown_meant_then_stuck {}",
                d[0], d[1]
            ));
        },
    )
}

/// The bodies the progress watch stopped, and those that came to the same leg again within 20 s (`stuck::progress_watch`).
pub(super) fn progress_watch() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::stuck::progress_watch::counts().to_vec(),
        |d, line| line.ends.push(format!("progress_stalls {} progress_stalls_again {}", d[0], d[1])),
    )
}

/// The blind stops: ticks a bot stood for want of a world, the stops a world ended, and those that showed the stop was needless
/// (`stuck::blind`).
pub(super) fn blind_stops() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::stuck::blind::counts().to_vec(),
        |d, line| {
            line.ends.push(format!("blind_stop_ticks {} blind_stops_ended {} blind_stops_needless {}", d[0], d[1], d[2]));
        },
    )
}

/// Walk ticks whose look-ahead was cut to a drop's landing, and those of runs past 2 s (`motor::drop_aim`).
pub(super) fn drop_aim() -> Box<dyn RouteTopic> {
    Delta::of(
        || super::super::motor::drop_aim::counts().to_vec(),
        |d, line| line.ends.push(format!("drop_aim_ticks {} drop_aim_long_ticks {}", d[0], d[1])),
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

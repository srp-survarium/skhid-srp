use super::*;

/// The own eye is led at most this far past its last decided tick
/// (`CombatTuning::lead_own`): a tick, or two after a skipped one.
pub(super) const LEAD_OWN_MOST_MS: u32 = 100;

/// A body farther than this off its route plans again, metres.
pub(super) const OFF_ROUTE_M: f32 = 3.0;

/// A goal that moved this much is planned to again, metres.
pub(super) const REPLAN_MOVED_M: f32 = 2.0;

/// A world that puts the body this far from its reckoning plans again.
pub(super) const REPLAN_ERROR_M: f32 = 2.0;

/// The reflex's side step is taken only with a clear walk this far that
/// side, metres (no step off a ledge or into a wall).
pub(super) const STRAFE_CLEAR_M: f32 = 1.5;

/// A jump pressed while the weapon fired lately is held this long, ms,
/// with the trigger let go: the weapon takes a jump only idle, reloading or
/// chambering a round (`weapon_core::can_jump` 0x2E3A70: states 3, 4 and
/// 6; `jump_predicate` 0x2FBDC0 asks it before the jump key), never while
/// it fires (5), shows or hides (1, 2). Aiming is no state: it does not
/// stop a jump. PROVEN in the code; `docs/notes/player_bots_stuck.md` §6.
pub(super) const JUMP_HOLD_MS: u32 = 300;

/// What followed a jump is traced this long after it at most, ms.
pub(super) const JUMP_WATCH_MS: u32 = 3_000;

/// A play's use look (`Look::Use`) this near the eye wins over the
/// reflex's aim (`decide`): the client's use reaches 2 m from the eye
/// (`base_player::detect_usable_object`, PROVEN), and the half metre more
/// turns the view while the bot walks in.
pub(super) const USE_LOOK_M: f32 = 2.5;

/// One tick of a living bot (`me` with its base pitch): the reflex's look
/// and trigger, the play's legs and stance, the presses.
/// Where a bot at `feet` with no goal of its own walks out of the zone it
/// is in, if a zone there is on or about to be (`Hazards::allowed`): the
/// end of its nearest way out (`Tactics::escape`), kept
/// [`stuck::ZONE_OUT_KEPT_MS`] so the route to it is not searched every
/// tick.
pub(super) fn zone_way_out(
    bot: &mut Bot,
    tactics: &Tactics,
    feet: Vec3,
    time: u32,
    objective: bool,
) -> Option<Vec3> {
    let from = tactics.world.grid.locate(feet, 1.5)?;
    if tactics.hazards.allowed(from, time) {
        bot.stuck.zone_out = None;
        return None;
    }
    if let Some((to, until)) = bot.stuck.zone_out
        && time < until
    {
        return Some(to);
    }
    let out = tactics.escape(from, None, time, bot.side, objective)?;
    let to = out.point_at(out.length());
    bot.stuck.zone_out = Some((to, time + stuck::ZONE_OUT_KEPT_MS));
    Some(to)
}

/// The way the walk keys `keys` move a body facing `yaw`, on the ground,
/// [`STRAFE_CLEAR_M`] long (the fight's side step's test): for the stuck
/// watch's keys (`Tactics::zone_free_step`).
pub(super) fn unstick_step(keys: u32, yaw: f32) -> Vec3 {
    let [fx, fz] = kin::forward(yaw);
    let [lx, lz] = kin::forward(yaw + std::f32::consts::FRAC_PI_2);
    let ahead =
        f32::from(u8::from(keys & key::FORWARD != 0)) - f32::from(u8::from(keys & key::BACK != 0));
    let left =
        f32::from(u8::from(keys & key::LEFT != 0)) - f32::from(u8::from(keys & key::RIGHT != 0));
    let (x, z) = (ahead * fx + left * lx, ahead * fz + left * lz);
    let length = x.hypot(z);
    if length < 1e-3 {
        return Vec3::ZERO;
    }
    Vec3::new(
        x / length * STRAFE_CLEAR_M,
        0.0,
        z / length * STRAFE_CLEAR_M,
    )
}

pub(super) fn drive_tick(
    bot: &mut Bot,
    me: &PlaceView,
    view: &MatchView,
    tactics: &Tactics,
    tuning: &Tuning,
    time: u32,
    searching: &mut Searching,
) -> Decision {
    // [recoil] The weapon's update first, as the client's; the view is the
    // base pitch and the kick the last tick left (the reflex takes back
    // this tick's rise itself, `Override::pull`).
    let kick = bot.reflex.weapon_tick(me, time);
    // Only this tick's walk to the play's own exact spot sets it (below).
    bot.motor.exact_end = false;
    let me = &PlaceView {
        pitch: (me.pitch + kick).clamp(-1.0, 1.0),
        ..me.clone()
    };
    let intent = bot.intent;
    let pose = Pose {
        feet: me.feet,
        yaw: me.yaw,
        pitch: me.pitch,
        speed: me.speed(),
        no_jump: !bot.hurt.may_jump(time),
    };
    if bot
        .jump_watch
        .as_ref()
        .is_some_and(|watch| time >= watch.at + JUMP_WATCH_MS)
    {
        bot.jump_followed(time, Some(me.feet), "went on");
    }
    // Getting unstuck overrides the rest: the keys for a few ticks, the
    // jump on the first.
    if let Some((keys, ticks)) = bot.unstick {
        bot.unstick = (ticks > 1).then_some((keys & !key::JUMP, ticks - 1));
        bot.motor.turn_time = None;
        bot.motor.track = None;
        let decision = bot.motor.rest();
        let by = (keys & key::JUMP != 0).then_some("unstick");
        bot.reflex.skipped_tick(time);
        let keys = jumping(bot, keys, by, me.feet, time);
        return Decision { keys, ..decision };
    }
    // The reflex sees and aims from the eye where this tick's move puts it
    // (`CombatTuning::lead_own`): the shot leaves from there.
    let led_me;
    let aiming_me = if tuning.aim.combat.lead_own {
        let ahead = time.saturating_sub(me.known_at).min(LEAD_OWN_MOST_MS) as f32 / 1000.0;
        led_me = PlaceView {
            feet: me.feet + Vec3::new(me.velocity[0] * ahead, 0.0, me.velocity[1] * ahead),
            known_at: time,
            ..me.clone()
        };
        &led_me
    } else {
        me
    };
    // [aimpoint] What of its target shows, before the reflex aims.
    bot.reflex.look(aiming_me, view, &tactics.world, time);
    // A carrier never fires: in the game a new fire press drops the battery
    // (the input handler 0x2D8E73..0x2D8EC8, PROVEN), whatever its play.
    // `carrying` comes with the worlds (up to 2 s after a take); the take
    // itself uses and Bring holds fire meanwhile. A play that let go of it
    // (`Intent::let_go`) may fire while the view still shows it carried.
    let may_fire = Gates::may_fire(&intent, me.carrying.is_some());
    bot.reflex.outer_gates(time, !may_fire, bot.jump_hold > 0);
    let over = bot.reflex.tick(
        aiming_me,
        view,
        &bot.memory,
        time,
        // No shot while a jump is held (`JUMP_HOLD_MS`).
        may_fire && bot.jump_hold == 0,
        &tuning.aim,
        bot.skill,
        &mut bot.rng,
    );
    bot.motor.turn_time = over.turn_time_s;
    let eye = aiming_me.eye();
    let aim_at = |p: Vec3| {
        let (dx, dz) = (p.x - eye.x, p.z - eye.z);
        (kin::yaw_of([dx, dz]), (p.y - eye.y).atan2(dx.hypot(dz)))
    };
    // A glance at a heard shot gives way to a play that uses something or
    // carries (`Override::glance`), and with `glance_run_yields` to one
    // that runs somewhere (a turned view slows the legs).
    let running = tuning.aim.combat.glance_run_yields
        && intent.pace == Pace::Run
        && matches!(intent.go, Go::To { at, within_m } if me.feet.distance_xz(at) > within_m);
    let glance_yields =
        over.glance && (matches!(intent.look, Look::Use(_)) || me.carrying.is_some() || running);
    // A use within its reach (a take of a battery, a put-in, a steal): the
    // play's view wins over the reflex's aim, and the reflex neither takes
    // the legs nor fires meanwhile. The client's use is one ray along the
    // view (`base_player::detect_usable_object`, PROVEN), so a bot that
    // kept its view on an enemy pressed use at him: in the whole-match
    // soaks the battery where the teams meet lay there all match in 6 runs
    // of 16 (its takers had a target 91 % of their time by it).
    let using = Gates::using(&intent, eye, bot.reflex.view_held());
    bot.motor.track = over.track.filter(|_| !glance_yields && !using);
    let aim = over
        .look_at
        .filter(|_| !glance_yields && !using)
        .map(|p| {
            let (yaw, pitch) = aim_at(p);
            (yaw + over.error.0, pitch + over.error.1)
        })
        .or(match intent.look {
            Look::At(p) => Some(aim_at(p + Vec3::new(0.0, 1.0, 0.0))),
            Look::Use(p) => Some(aim_at(p)),
            _ => None,
        });
    // The legs: the reflex's when it takes them (a push, breaking sight,
    // a re-peek, the weapon's range, in after a grenade), else the play's.
    let mut intent = intent;
    #[cfg(test)]
    {
        bot.bench_legs = over.legs.is_some() && !using;
    }
    // The bench's stand-in camper keeps to its spot.
    #[cfg(test)]
    let held = bot.bench_camper;
    #[cfg(not(test))]
    let held = false;
    if let Some(legs) = over.legs.filter(|_| !using && !held) {
        intent.go = Go::To {
            at: legs.at,
            within_m: legs.within_m,
        };
        intent.crouch = legs.crouch;
        intent.pace = if legs.run { Pace::Run } else { Pace::Walk };
        intent.objective = false;
        intent.straight = false;
    }
    let goal = match intent.go {
        Go::To { at, within_m } if me.feet.distance_xz(at) > within_m => Some(at),
        _ => None,
    };
    // [zones] With no goal to walk (holding a spot, standing at its goal,
    // aiming still), out of a zone that is on or about to be first,
    // whatever the play: its way out is walked as the goal, the reflex
    // aiming and firing meanwhile. A goal walked to has its own way out
    // (`escaping` below). The Cologne anomaly bench, runs 4 and 6b: a bot
    // with no goal stood 5.9 s of each on span, and 27 s in a zone always on.
    let zone_set = tactics
        .world
        .grid
        .locate(me.feet, 1.5)
        .map_or(0, |node| tactics.hazards.set_of(node));
    bot.stuck.zone_on(zone_set);
    let zone_out = if Gates::wants_zone_way_out(&intent, goal) {
        zone_way_out(bot, tactics, me.feet, time, intent.objective)
    } else {
        None
    };
    intent.straight = Gates::straight(intent.straight, zone_out.is_some());
    let goal = goal.or(zone_out);
    #[cfg(test)]
    {
        bot.bench_why = 0;
        for (on, bit) in [
            (bot.bench_legs, why::LEGS),
            (zone_out.is_some(), why::ZONE_OUT),
            (intent.zone_ok, why::ZONE_OK),
            (intent.straight, why::STRAIGHT),
        ] {
            if on {
                bot.bench_why |= bit;
            }
        }
    }
    // [pockets] On a pocket's floor with its goal out of the pocket (no
    // link of the grid leads out of one), its ways out are tried, each a
    // route with a jump onto the rim (`escape.rs`); the try's landing is its
    // goal meanwhile. Cologne CG-3: a bot stood in pit A aiming all match.
    let goal = if intent.straight {
        goal
    } else {
        let grid = &tactics.world.grid;
        let out = escape::tick(
            &mut bot.stuck.escape,
            &mut bot.route,
            grid,
            &tactics.escapes,
            bot.place,
            me.team,
            me.feet,
            goal.unwrap_or(me.feet),
            time,
            bot.hurt.may_jump(time),
        );
        if bot.tracing {
            for event in &out.events {
                bot.traced.push(escape::trace(time, bot.place, grid, event));
            }
        }
        #[cfg(test)]
        if out.goal.is_some() {
            bot.bench_why |= why::POCKET;
        }
        out.goal.or(goal)
    };
    // The play's own exact spot (a goal within 0): its route is not arrived
    // at from over its end (`Motor::exact_end`). A reflex's legs replaced
    // the play's goal above; an escape's or a zone's goal is not this one.
    let exact_at = Gates::exact_spot(&intent);
    // Standing to aim (no way to walk): the reflex's side step may move it;
    // walking a route while it aims, the step weaves the walk
    // ([`weave`], `strafe_walking`).
    let mut aiming_still = false;
    let mut walking_aimed = false;
    let mut decision = match goal {
        // A step or two in a line the play vouches for: by keys, no route.
        Some(at) if intent.straight => {
            bot.route = None;
            bot.motor.step_to(&pose, at, aim)
        }
        Some(asked) => {
            let at = asked;
            // Its goal moved while it walks a crossing's zone (a route leg's
            // next point, a play's new goal): the crossing is walked on till
            // it is off the zone's floors, then it plans to the new goal. An
            // escape from inside took the nearest way out instead: on
            // Mamayev Kurgan's batteries one led down a 4 m drop into a zone
            // of 0.25 a second (`crossing.rs`).
            let at = match bot.crossing {
                Some(crossed)
                    if crossed.distance_xz(at) > REPLAN_MOVED_M
                        && bot.route.is_some()
                        && tactics
                            .world
                            .grid
                            .locate(me.feet, 1.5)
                            .is_some_and(|node| tactics.hazards.only_fixed_at(node)) =>
                {
                    crossed
                }
                _ => at,
            };
            #[cfg(test)]
            if at != asked {
                bot.bench_why |= why::WALK_ON;
            }
            let replan = bot
                .route
                .as_ref()
                .is_none_or(|(_, planned)| planned.distance_xz(at) > REPLAN_MOVED_M);
            // [zones] A zone ahead on its route that is on when it gets
            // there (one that came on, or it is late): planned again.
            // On a crossing to this goal (`crossing.rs`): its zones' floors
            // are walked.
            let mut crossing = bot
                .crossing
                .is_some_and(|goal| goal.distance_xz(at) <= REPLAN_MOVED_M);
            // One that would now leave too little health: stopped, and the
            // escape walks it out (`crossing::still_borne`).
            let mut aborted = false;
            if crossing
                && !replan
                && time >= bot.stuck.zone_checked + stuck::ZONE_CHECK_MS
                && let Some((route, _)) = bot.route.as_mut()
            {
                let (walked, _) = route.progress(me.feet);
                if !crossing::still_borne(tactics, route, walked, me.health) {
                    crossing::count(crossing::Count::Aborted);
                    bot.crossing = None;
                    bot.route = None;
                    crossing = false;
                    aborted = true;
                }
            }
            let replan = replan || aborted;
            let shut = !replan && time >= bot.stuck.zone_checked + stuck::ZONE_CHECK_MS && {
                bot.stuck.zone_checked = time;
                bot.route.as_mut().is_some_and(|(route, _)| {
                    tactics.route_shut_ahead(route, me.feet, time, crossing)
                })
            };
            if shut {
                bot.route = None;
            }
            let replan = replan || shut;
            // A goal with no way lately: not searched again every tick.
            let no_way = bot.no_way.is_some_and(|(goal, until)| {
                time < until && goal.distance_xz(at) <= REPLAN_MOVED_M
            });
            // The answer of its search that went on over several polls, if
            // it is for this goal (it walks the way so far meanwhile).
            let found = bot
                .found
                .take()
                .filter(|(goal, _, _)| goal.distance_xz(at) <= REPLAN_MOVED_M);
            // Its own search under way: for this goal, else no longer
            // wanted.
            let waiting = match searching.pending.as_ref() {
                Some(pending) if pending.place == bot.place => {
                    let still = pending.goal.distance_xz(at) <= REPLAN_MOVED_M;
                    if !still {
                        bot.dropped.dropped(time, pending.goal, at);
                        searching.pending = None;
                    }
                    still
                }
                _ => false,
            };
            // Why it has no route, if it has none after this.
            let mut no_route = path::NoRoute::Searched;
            if let Some((goal, route, crossed)) = found {
                bot.no_way = route.is_none().then_some((goal, time + NO_WAY_RETRY_MS));
                bot.crossing = crossed.then_some(goal);
                // Both ends were on floors, from one it may walk on: proven.
                if route.is_none() {
                    bot.no_way_to.retain(|&(old, until)| {
                        time < until && old.distance_xz(goal) > REPLAN_MOVED_M
                    });
                    bot.no_way_to.push((goal, time + NO_WAY_KEEP_MS));
                }
                #[cfg(test)]
                {
                    bot.bench_route = match (crossed, bot.bench_found_ring) {
                        (true, true) => why::RING,
                        (true, false) => why::CROSS,
                        _ => why::PLAIN,
                    };
                }
                bot.route = route.map(|route| (route, goal));
            } else if replan && no_way {
                bot.route = None;
                no_route = path::NoRoute::NoWay;
            } else if waiting
                || (replan
                    && (searching.left == 0
                        || searching.floors == 0
                        || searching.pending.is_some()))
            {
                // Its search goes on, or none left to this poll: the old
                // route meanwhile.
                no_route = if waiting {
                    path::NoRoute::Searching
                } else {
                    path::NoRoute::Starved
                };
            } else if replan {
                searching.left -= 1;
                let grid = &tactics.world.grid;
                let side = bot.side;
                let objective = intent.objective;
                // Round the spots it got stuck at, if there is a way; else
                // through them (a second search only when there are any).
                let spots: Vec<Vec3> = bot.stuck.avoided_for(time, at).collect();
                let avoiding = !spots.is_empty();
                let ends = grid.locate(me.feet, 1.5).zip(path::goal_floor(grid, at));
                // Already on the goal's floor (the nearest to a goal on none):
                // as near as it gets, nothing to search.
                let there = ends.is_some_and(|(from, to)| from == to);
                // On a floor it may not walk on (in a fire it stepped
                // into, a zone about to come on): out first, the way that
                // leads on to the goal when it is out in time
                // (`Tactics::escape`); the goal is planned from there. Even
                // on the goal's floor: no standing in a fire.
                // Not off a floor of the zones its crossing walks.
                let escaping = ends.is_some_and(|(from, _)| {
                    !tactics.allowed(from, time, side, objective)
                        && !(crossing && tactics.hazards.only_fixed_at(from))
                });
                let floors = &mut searching.floors;
                // Left under way past this poll's floors: whether round the
                // spots, and the search.
                let mut left = None;
                let mut route = if escaping {
                    let route = ends.and_then(|(from, _)| {
                        tactics.escape(from, Some(at), time, side, objective)
                    });
                    // [zones] Out of the same zones toward the same goal
                    // twice lately: the way there keeps going into them;
                    // the goal is given up for the play to choose again.
                    if let Some((from, _)) = ends
                        && route.is_some()
                        && zone_out.is_none()
                        && tactics.hazards.set_of(from) != 0
                        && bot
                            .stuck
                            .zone_escape(time, tactics.hazards.set_of(from), at)
                    {
                        bot.no_way_to.retain(|&(goal, until)| {
                            time < until && goal.distance_xz(at) > REPLAN_MOVED_M
                        });
                        bot.no_way_to.push((at, time + NO_WAY_KEEP_MS));
                    }
                    route
                } else if there {
                    None
                } else if let Some((from, to)) = ends {
                    let open = |node: NodeId| !(avoiding && near_any(&spots, grid.position(node)));
                    match tactics.start_route(from, to, time, side, objective, open, floors) {
                        Ok(None) if avoiding => {
                            match tactics.start_route(
                                from,
                                to,
                                time,
                                side,
                                objective,
                                |_| true,
                                floors,
                            ) {
                                Ok(route) => route,
                                Err(planning) => {
                                    left = Some((false, planning));
                                    None
                                }
                            }
                        }
                        // Round them only where that is not much longer
                        // (`stuck::worth_round`), else through them.
                        Ok(Some(round))
                            if avoiding
                                && stuck::may_be_long(
                                    round.length(),
                                    grid.position(from),
                                    grid.position(to),
                                ) =>
                        {
                            let through = tactics.start_route(
                                from,
                                to,
                                time,
                                side,
                                objective,
                                |_| true,
                                floors,
                            );
                            match through {
                                Ok(Some(through))
                                    if !stuck::worth_round(round.length(), through.length()) =>
                                {
                                    Some(through)
                                }
                                _ => Some(round),
                            }
                        }
                        Ok(route) => route,
                        Err(planning) => {
                            left = Some((avoiding, planning));
                            None
                        }
                    }
                } else {
                    None
                };
                // No way, or a long one past zones on all match: a crossing's
                // search (`crossing.rs`).
                let mut crossed = false;
                let mut second = None;
                if left.is_none()
                    && !escaping
                    && !there
                    && !(route.is_some() && long_kept(bot.long_kept, at, time))
                    && let Some((from, to)) = ends
                    && crossing::wanted(
                        tactics,
                        route.as_ref(),
                        from,
                        to,
                        side,
                        objective,
                        me.health,
                    )
                {
                    match crossing::start(tactics, from, to, time, side, objective, me.health, floors)
                    {
                        Ok(crossing_route) => {
                            let kept;
                            (route, crossed, kept) = crossing::choose(
                                tactics,
                                from,
                                route,
                                crossing_route,
                                me.health,
                                time,
                                side,
                                objective,
                            );
                            if kept {
                                bot.long_kept = Some((at, time + crossing::LONG_RETRY_MS));
                            }
                        }
                        Err(planning) => {
                            let plain = route.take();
                            second = Some((
                                planning,
                                crossing::Second {
                                    plain,
                                    health: me.health,
                                },
                            ));
                        }
                    }
                }
                if let (Some((planning, second)), Some(ends)) = (second, ends) {
                    // It goes on at the next polls; the plain route (a long
                    // one) meanwhile, else the old one.
                    if let Some(plain) = second.plain.clone() {
                        #[cfg(test)]
                        {
                            bot.bench_route = why::PLAIN;
                        }
                        bot.route = Some((plain, at));
                        bot.crossing = None;
                    }
                    no_route = path::NoRoute::Searching;
                    searching.pending = Some(Pending {
                        place: bot.place,
                        goal: at,
                        time,
                        side,
                        objective,
                        ends,
                        spots: None,
                        hazards: tactics.hazards.version(),
                        planning,
                        health: me.health,
                        second: Some(second),
                    });
                } else if let (Some((round, planning)), Some(ends)) = (left, ends) {
                    // It goes on at the next polls (`Bots::go_on_searching`).
                    // Meanwhile the way so far, as Detour's partial paths:
                    // toward the goal, not standing (else the old route).
                    let open = |node: NodeId| !(round && near_any(&spots, grid.position(node)));
                    if let Some(partial) =
                        tactics.partial_route(&planning, ends.0, time, side, objective, open)
                    {
                        #[cfg(test)]
                        {
                            bot.bench_route = why::PLAIN;
                        }
                        bot.route = Some((partial, at));
                    }
                    no_route = path::NoRoute::Searching;
                    searching.pending = Some(Pending {
                        place: bot.place,
                        goal: at,
                        time,
                        side,
                        objective,
                        ends,
                        spots: round.then_some(spots),
                        hazards: tactics.hazards.version(),
                        planning,
                        health: me.health,
                        second: None,
                    });
                } else {
                    bot.no_way = route.is_none().then_some((at, time + NO_WAY_RETRY_MS));
                    // Both ends on floors, from one it may walk on, and no way:
                    // proven, for the plays.
                    if route.is_none() && ends.is_some() && !escaping && !there {
                        bot.no_way_to.retain(|&(goal, until)| {
                            time < until && goal.distance_xz(at) > REPLAN_MOVED_M
                        });
                        bot.no_way_to.push((at, time + NO_WAY_KEEP_MS));
                    }
                    #[cfg(test)]
                    {
                        bot.bench_route = if escaping {
                            why::ESCAPE
                        } else if crossed && crossing::last_ring() {
                            why::RING
                        } else if crossed {
                            why::CROSS
                        } else {
                            why::PLAIN
                        };
                    }
                    bot.route = route.map(|route| (route, at));
                    bot.crossing = crossed.then_some(at);
                }
            }
            #[cfg(test)]
            {
                bot.bench_why |= if bot.route.is_some() {
                    bot.bench_route
                } else {
                    why::NONE
                };
            }
            match bot.route.as_mut() {
                Some((route, route_at)) => {
                    // `route_at` is the goal's `at`, copied unchanged where
                    // the route was planned, so the two compare exactly (a
                    // replan that snapped it to a floor would turn this off).
                    bot.motor.exact_end = exact_at == Some(*route_at);
                    let side = bot.side;
                    let objective = intent.objective;
                    let clear = |p: Vec3| tactics.clear_line_for(me.feet, p, time, side, objective);
                    let (decision, walked) = match aim {
                        Some(aim) => {
                            walking_aimed = true;
                            bot.motor.walk_aiming(route, &pose, OFF_ROUTE_M, clear, aim)
                        }
                        None => bot.motor.walk(route, &pose, OFF_ROUTE_M, clear),
                    };
                    if walked != Walked::OnTheWay {
                        // Fallen off it: the spot it fell from (`Stuck::fell`).
                        if let Walked::Off(off) = walked
                            && off.is_infinite()
                            && let Some(from) = route.fell_from(pose.feet)
                        {
                            bot.stuck.fell(time, from);
                        }
                        // A jump it could not make: its take-off, likewise
                        // (not an escape's: a pocket's rim is no way).
                        if let Walked::JumpFailed(take_off) = walked
                            && !bot
                                .stuck
                                .escape
                                .as_ref()
                                .is_some_and(escape::Escape::trying)
                        {
                            bot.stuck.failed_jump(time, take_off);
                        }
                        bot.route = None;
                        path::no_route(path::NoRoute::Ended);
                    }
                    decision
                }
                None => {
                    path::no_route(no_route);
                    match aim {
                        Some(aim) => {
                            aiming_still = true;
                            bot.motor.aim_still(&pose, aim)
                        }
                        None => bot.motor.stand(&pose, &mut bot.rng),
                    }
                }
            }
        }
        None => {
            bot.route = None;
            match aim {
                Some(aim) => {
                    aiming_still = true;
                    bot.motor.aim_still(&pose, aim)
                }
                None => bot.motor.stand(&pose, &mut bot.rng),
            }
        }
    };
    // The side step, where the ground that side is clear for a stride:
    // standing, or weaving a walk to or from the target.
    let stepped = if aiming_still {
        Some(decision.keys | over.strafe)
    } else if walking_aimed && tuning.aim.combat.strafe_walking {
        weave(decision.keys, over.strafe)
    } else {
        None
    };
    if let Some(keys) = stepped.filter(|_| over.strafe != 0 && !using) {
        let left = over.strafe == key::LEFT;
        let [sx, sz] = kin::forward(me.yaw + std::f32::consts::FRAC_PI_2);
        let sign = if left { 1.0 } else { -1.0 };
        let to = me.feet + Vec3::new(sign * sx * STRAFE_CLEAR_M, 0.0, sign * sz * STRAFE_CLEAR_M);
        let clear = tactics.clear_step_for(me.feet, to, time, bot.side, intent.objective);
        #[cfg(test)]
        motor::counts::side_step(clear);
        if clear {
            decision.keys = keys;
        }
    }
    // The stance and pace.
    if intent.crouch || intent.pace == Pace::Sneak {
        decision.keys |= key::CROUCH;
        decision.keys &= !key::SPRINT;
    }
    if intent.pace != Pace::Run {
        decision.keys &= !key::SPRINT;
    }
    decision.turn.1 += over.pull;
    if over.fire && !using {
        decision.keys |= key::FIRE;
        decision.keys &= !key::SPRINT;
    } else if over.fire {
        // The shot the reflex counted this tick is not fired: given back.
        bot.reflex.hold_fire(time);
        bot.reflex.gate_using(time);
    }
    decision.keys |= over.keys;
    if over.keys & (key::GRENADE | key::RELOAD | key::AIM | key::CROUCH) != 0 {
        decision.keys &= !key::SPRINT;
    }
    decision.keys |= intent.hold | std::mem::take(&mut bot.press);
    // Lining up for a jump of its route, and in it: standing (a crouched
    // body seldom jumps: 9 route jumps of 12 pressed crouched did in the
    // owner's matches, 6 unstick jumps of 23, against 149 of 159 standing;
    // `player_bots_stuck.md` §8.9), and no sprint.
    if bot.motor.lining_up() {
        decision.keys &= !(key::CROUCH | key::SPRINT);
    }
    if decision.keys & key::QUICK[0] != 0 {
        bot.medkits_used = bot.medkits_used.saturating_add(1);
    }
    let by = bot.motor.jumped().map(|by| match by {
        JumpBy::Route => "route",
        JumpBy::Hop => "hop",
    });
    decision.keys = jumping(bot, decision.keys, by, me.feet, time);
    decision
}

/// A jump watched for its trace: why, when and from where; for a jump of
/// the route its take-off and landing, which try it was, and the first
/// world's tick that showed the body up it.
#[derive(Clone, Copy, Debug)]
pub(super) struct JumpWatch {
    pub(super) by: &'static str,
    pub(super) at: u32,
    pub(super) from: Vec3,
    pub(super) route: Option<RoutePress>,
    pub(super) up: Option<u32>,
}

impl Bot {
    /// The trace line of what followed the jump watched, if any (`then`, at
    /// `time`, the feet then if known); the watch ends.
    pub(super) fn jump_followed(&mut self, time: u32, feet: Option<Vec3>, then: &str) {
        let Some(watch) = self.jump_watch.take() else {
            return;
        };
        if self.tracing {
            let mut line = serde_json::json!({
                "t": time, "bot": self.place, "jumped": watch.by,
                "after_ms": time.saturating_sub(watch.at), "then": then,
                "moved_m": feet.map(|f| f.distance_xz(watch.from)),
                "rose_m": feet.map(|f| f.y - watch.from.y),
            });
            if let Some(route) = watch.route {
                line["try"] = route.tries.into();
                line["up_ms"] = watch.up.map(|up| up.saturating_sub(watch.at)).into();
            }
            self.traced.push(line);
        }
    }

    /// A world of `time` showed the body at `feet`: up the route jump
    /// watched, if it is ([`motor::is_up`]).
    pub(super) fn jump_seen(&mut self, time: u32, feet: Vec3) {
        if let Some(watch) = self.jump_watch.as_mut()
            && let Some(route) = watch.route
            && watch.up.is_none()
            && time > watch.at
            && motor::is_up(feet, route.take_off, route.landing)
        {
            watch.up = Some(time);
        }
    }
}

/// A tick's keys with its jump. A jump pressed now (`by`: the route's, a
/// hop, getting unstuck) is traced and watched; with the trigger down now or
/// within [`JUMP_HOLD_MS`] it is held that long with the trigger let go, the
/// shot of this tick given back ([`Reflex::hold_fire`]), as the weapon takes
/// no jump while it fires.
pub(super) fn jumping(bot: &mut Bot, keys: u32, by: Option<&'static str>, feet: Vec3, time: u32) -> u32 {
    let firing = keys & key::FIRE != 0;
    if let Some(by) = by.filter(|_| bot.jump_hold == 0) {
        let lately = firing || bot.fired_at.is_some_and(|at| time < at + JUMP_HOLD_MS);
        bot.jump_hold = if lately {
            (JUMP_HOLD_MS as f32 / (kin::TICK_S * 1000.0)).round() as u32
        } else {
            1
        };
        bot.jump_followed(time, Some(feet), "jumped again");
        bot.reflex.jumped(by);
        let route = (by == "route").then(|| bot.motor.route_press()).flatten();
        if bot.tracing {
            let mut line = serde_json::json!({
                "t": time, "bot": bot.place, "jump": by, "pos": feet.to_array(),
                "firing": lately, "aiming": keys & key::AIM != 0,
            });
            if let Some(route) = route {
                line["try"] = route.tries.into();
                line["from"] = route.take_off.to_array().into();
                line["to"] = route.landing.to_array().into();
                line["before_m"] = route.before.into();
                line["facing"] = route.facing.into();
                line["crouch"] = (keys & key::CROUCH != 0).into();
            }
            bot.traced.push(line);
        }
        bot.jump_watch = Some(JumpWatch {
            by,
            at: time,
            from: feet,
            route,
            up: None,
        });
    }
    if bot.jump_hold == 0 {
        if firing {
            bot.fired_at = Some(time);
        }
        return keys;
    }
    bot.jump_hold -= 1;
    if firing {
        bot.reflex.hold_fire(time);
        bot.reflex.gate_jump(time);
    }
    (keys | key::JUMP) & !key::FIRE
}

/// A walk's keys with the side step `strafe` woven in: a walk straight to
/// or from where it looks (forward or back alone) goes the diagonal to that
/// side, so the walk goes on while the body weaves; a walk already to a side
/// is left as it is (it strafes already), and so is no walk.
pub(super) fn weave(keys: u32, strafe: u32) -> Option<u32> {
    let along = keys & (key::FORWARD | key::BACK);
    let side = keys & (key::LEFT | key::RIGHT);
    (along != 0 && side == 0 && strafe != 0).then_some(keys | strafe)
}

/// What the flags of a play's [`Intent`] allow this tick, each as one rule in one place: `drive_tick` reads
/// the answers, so a play's override of a default (fire while carrying, stand in a zone, walk by keys,
/// arrive at an exact spot) is one field on `Intent` and one rule here.
pub(super) struct Gates;

impl Gates {
    /// Whether the reflex may fire. A carrier never does (a new fire press drops the battery), unless the
    /// play let go of it (`Intent::let_go`): the view shows it carried up to 2 s late.
    pub(super) fn may_fire(intent: &Intent, carrying: bool) -> bool {
        intent.fire && (!carrying || intent.let_go)
    }

    /// Whether a bot with `goal` (None: nothing to walk to) looks for the way out of a zone it stands in:
    /// only with no goal, and not when standing there is the play's own choice (`Intent::zone_ok`).
    pub(super) fn wants_zone_way_out(intent: &Intent, goal: Option<Vec3>) -> bool {
        goal.is_none() && !intent.zone_ok
    }

    /// Whether the walk is by keys with no route (`Intent::straight`): not when a zone's way out is the goal.
    pub(super) fn straight(straight: bool, zone_out: bool) -> bool {
        straight && !zone_out
    }

    /// Whether the play's use is within reach and takes the view: the use is of something within
    /// `USE_LOOK_M` of the eye, and not while the reflex holds the view for a grenade between its press and
    /// its launch (the use waits that second and a half: the grenade leaves along the view then, and a use
    /// look at a battery by the thrower's feet sent it there, 0.8 m from him, in a smoke).
    pub(super) fn using(intent: &Intent, eye: Vec3, view_held: bool) -> bool {
        !view_held && matches!(intent.look, Look::Use(p) if (p - eye).length() <= USE_LOOK_M)
    }

    /// The play's own exact spot (a goal within 0), whose route is not arrived at from over its end.
    pub(super) fn exact_spot(intent: &Intent) -> Option<Vec3> {
        match intent.go {
            Go::To { at, within_m: 0.0 } => Some(at),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent(fire: bool, let_go: bool, zone_ok: bool) -> Intent {
        Intent { fire, let_go, zone_ok, ..Intent::stay() }
    }

    #[test]
    fn a_carrier_fires_only_when_its_play_let_go_of_the_battery() {
        for (fire, let_go, carrying, expect) in [
            (true, false, false, true),
            (true, false, true, false),
            (true, true, true, true),
            (true, true, false, true),
            (false, true, false, false),
            (false, false, true, false),
        ] {
            assert_eq!(Gates::may_fire(&intent(fire, let_go, false), carrying), expect, "{fire} {let_go} {carrying}");
        }
    }

    #[test]
    fn a_use_takes_the_view_when_in_reach_and_the_reflex_holds_none_for_a_grenade() {
        let eye = Vec3::new(0.0, 1.6, 0.0);
        let look = |at| Intent { look: Look::Use(at), ..Intent::stay() };
        let near = Vec3::new(0.0, 0.5, 1.0);
        let far = Vec3::new(0.0, 0.5, USE_LOOK_M + 1.0);
        assert!(Gates::using(&look(near), eye, false));
        assert!(!Gates::using(&look(near), eye, true));
        assert!(!Gates::using(&look(far), eye, false));
        assert!(!Gates::using(&Intent::stay(), eye, false));
    }

    #[test]
    fn a_zone_way_out_is_wanted_with_no_goal_and_a_play_that_does_not_stand_there_on_purpose() {
        let goal = Some(Vec3::new(1.0, 0.0, 2.0));
        assert!(Gates::wants_zone_way_out(&intent(true, false, false), None));
        assert!(!Gates::wants_zone_way_out(&intent(true, false, false), goal));
        assert!(!Gates::wants_zone_way_out(&intent(true, false, true), None));
        assert!(!Gates::wants_zone_way_out(&intent(true, false, true), goal));
    }

    #[test]
    fn a_straight_walk_gives_way_to_a_zone_way_out() {
        assert!(Gates::straight(true, false));
        assert!(!Gates::straight(true, true));
        assert!(!Gates::straight(false, false));
        assert!(!Gates::straight(false, true));
    }

    #[test]
    fn only_a_goal_within_nothing_is_an_exact_spot() {
        let at = Vec3::new(3.0, 0.0, 4.0);
        let go = |within_m| Intent { go: Go::To { at, within_m }, ..Intent::stay() };
        assert_eq!(Gates::exact_spot(&go(0.0)), Some(at));
        assert_eq!(Gates::exact_spot(&go(0.5)), None);
        assert_eq!(Gates::exact_spot(&Intent::stay()), None);
    }
}

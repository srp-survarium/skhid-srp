use super::hazards::ZoneSet;
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

/// Why the client takes no sprint jump now, as far as the bot knows (`base_player::can_sprint` and the stagger, the
/// sprint-jump RE §1.4 and §1.8): the keys it sent in the tick before (aiming, firing or throwing), a reload on, a hit
/// within [`motor::STAGGER_SPRINT_MS`], the reckoned bar in its low state, a battery carried.
fn sprint_block(bot: &Bot, me: &PlaceView, time: u32, stamina_on: bool) -> Option<motor::SprintBlock> {
    let sent = bot.sent_keys;
    if me.carrying.is_some() {
        Some(motor::SprintBlock::Carrying)
    } else if sent & key::AIM != 0 {
        Some(motor::SprintBlock::Aiming)
    } else if sent & (key::FIRE | key::GRENADE) != 0 {
        Some(motor::SprintBlock::Busy)
    } else if bot.reflex.reload_note().is_some_and(|reload| time < reload.until) {
        Some(motor::SprintBlock::Reloading)
    } else if bot.hurt.last_hit_ms.is_some_and(|at| time < at.saturating_add(motor::STAGGER_SPRINT_MS)) {
        Some(motor::SprintBlock::Staggered)
    } else if stamina_on && bot.stamina.low() {
        Some(motor::SprintBlock::Tired)
    } else {
        None
    }
}

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

/// A bot stepping out of a grenade's blast has arrived this near its floor, metres.
const AVOID_ARRIVE_M: f32 = 0.8;

/// What `grenade_threat::step` asks of the map: a line, and the floor out of a blast
/// ([`Tactics::step_out`], over the floors the body may walk, the landings it gave up shut).
struct BlastAround<'a> {
    tactics: &'a Tactics,
    /// The team's eyes through the match's smoke: a grenade lying in a cloud is not seen (`smoke.rs`).
    veil: super::smoke::Veil<'a>,
    stuck: &'a Stuck,
    side: Option<i32>,
    time: u32,
    objective: bool,
}

impl grenade_threat::Surroundings for BlastAround<'_> {
    fn sees(&self, from: Vec3, to: Vec3) -> bool {
        !self.veil.hides(from, to) && super::rays::visible(&self.tactics.world.sight, from, to)
    }

    fn way_out(
        &self,
        me: &PlaceView,
        blast: &grenade_threat::Blast,
        reach_m: f32,
    ) -> Option<Vec3> {
        let from = self.tactics.world.grid.locate(me.feet, 1.5)?;
        let hard = RouteFilter::shutting_landings(self.stuck.shut_landings(self.time).collect());
        self.tactics.step_out(
            from,
            blast.rest,
            blast.radius,
            reach_m,
            self.time,
            self.side,
            self.objective,
            &hard,
        )
    }
}

/// A route planned from the floor a stranded body drops to ([`path::stranded_floor`]): the body's own place first.
fn stranded_leg(route: Route, feet: Vec3) -> Route {
    path::stranded::count();
    route.from_stand(feet)
}

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
    let landings = bot.stuck.shut_landings(time).collect();
    let hard = traps::filter(bot, landings, Some(from));
    let out = tactics.escape(from, None, time, bot.side, objective, &hard)?;
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

/// The move the fight's side step (`keys`, its `strafe` key woven into the
/// walk's) is tested over, [`STRAFE_CLEAR_M`] long on the ground: with `by_keys`
/// (`Tuning::aim.combat.clear_step_sees_lip`) the move the keys really make
/// ([`unstick_step`]: a weave of a walk forward or back is a diagonal, and
/// its end is not where the pure side step ends); else, or with keys that
/// cancel, the pure side step to the `strafe` side, as it was.
pub(super) fn side_step(keys: u32, strafe: u32, yaw: f32, by_keys: bool) -> Vec3 {
    let moved = unstick_step(keys, yaw);
    if by_keys && moved != Vec3::ZERO {
        return moved;
    }
    let [sx, sz] = kin::forward(yaw + std::f32::consts::FRAC_PI_2);
    let sign = if strafe == key::LEFT { 1.0 } else { -1.0 };
    Vec3::new(sign * sx * STRAFE_CLEAR_M, 0.0, sign * sz * STRAFE_CLEAR_M)
}

/// The keys of a walk or stand turned into the step back from a teammate in front ([`FriendStep::Back`]): no
/// forward, no sprint, the back key, the side keys as they were.
pub(super) fn back_keys(keys: u32) -> u32 {
    (keys & !(key::FORWARD | key::SPRINT)) | key::BACK
}

/// What the fight's side step comes to this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    /// The reflex's side is clear: these keys.
    Own(u32),
    /// Its side is refused at a lip and the other is clear (`clear_step_slides_along`): these keys.
    Slid(u32),
    /// The keys stay as the walk or stand decided: no side step.
    Stand,
}

/// The fight's side step with its ground test `ask(keys, strafe)` (`Tactics::clear_step_for` over the move of
/// `keys`): the reflex's own side `strafe` if clear; refused with `sees_lip` on and the step its own (not
/// `away` from a teammate, which is never turned to the other side: that would step into him), the other
/// side if `slides` and clear, else [`Step::Stand`]. The other side is also asked for the bench's counts
/// (`probe_other`) when `slides` is off; its answer comes back (`None`: not asked).
pub(super) fn choose_step(
    keys: u32,
    strafe: u32,
    away: bool,
    sees_lip: bool,
    slides: bool,
    probe_other: bool,
    ask: impl Fn(u32, u32) -> bool,
) -> (Step, Option<bool>) {
    if ask(keys, strafe) {
        return (Step::Own(keys), None);
    }
    if !sees_lip || away || !(slides || probe_other) {
        return (Step::Stand, None);
    }
    let other = if strafe == key::LEFT {
        key::RIGHT
    } else {
        key::LEFT
    };
    let flipped = (keys & !(key::LEFT | key::RIGHT)) | other;
    let other_clear = ask(flipped, other);
    (
        if slides && other_clear {
            Step::Slid(flipped)
        } else {
            Step::Stand
        },
        Some(other_clear),
    )
}

/// One drive tick: the bot's stamina reckoning follows the keys of its last decision ([`stamina`]); with
/// `think.stamina` the motor is told the bar and a jump the bar cannot pay is not pressed.
pub(super) fn drive_tick(
    bot: &mut Bot,
    me: &PlaceView,
    view: &MatchView,
    tactics: &Tactics,
    tuning: &Tuning,
    time: u32,
    searching: &mut Searching,
) -> Decision {
    bot.stamina.tick(time, bot.sent_keys, me.carrying.is_some());
    bot.motor.gauge = tuning.think.stamina.then(|| bot.stamina.gauge());
    let decision = drive_tick_inner(bot, me, view, tactics, tuning, time, searching);
    bot.sent_keys = decision.keys;
    decision
}

fn drive_tick_inner(
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
    bot.motor.exact_end = None;
    let me = &PlaceView {
        pitch: (me.pitch + kick).clamp(-1.0, 1.0),
        ..me.clone()
    };
    let intent = bot.intent;
    #[cfg(test)]
    {
        bot.bench_walk = 0;
        if bot.bench_est.len() == 10 {
            bot.bench_est.pop_front();
        }
        bot.bench_est.push_back((me.known_at, me.feet));
    }
    let pose = Pose {
        feet: me.feet,
        yaw: me.yaw,
        pitch: me.pitch,
        speed: me.speed(),
        velocity: me.velocity,
        no_jump: !bot.hurt.may_jump(time) || (tuning.think.stamina && !bot.stamina.may_jump()),
        sprint_block: sprint_block(bot, me, time, tuning.think.stamina),
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
    bot.memory
        .set_sight_window(know::SightWindow::of(&tuning.sense, &tuning.cheats));
    // [aimpoint] What of its target shows, before the reflex aims.
    bot.reflex.look_through(
        aiming_me,
        view,
        &tactics.world,
        &mut bot.memory,
        &tuning.sense,
        &tuning.smoke,
        time,
    );
    // A carrier never fires: in the game a new fire press drops the battery
    // (the input handler 0x2D8E73..0x2D8EC8, PROVEN), whatever its play.
    // `carrying` comes with the worlds (up to 2 s after a take); the take
    // itself uses and Bring holds fire meanwhile. A play that let go of it
    // (`Intent::let_go`) may fire while the view still shows it carried.
    let may_fire = Gates::may_fire(&intent, me.carrying.is_some());
    bot.reflex.outer_gates(time, !may_fire, bot.jump_hold > 0);
    // [heal] The consumable is a habit of the reflex, as the reload is: the
    // key goes out at once whatever the play, which is neither taken nor
    // displaced (`plays::heal::wish`, `HealTuning::interrupt`).
    bot.reflex.set_heal(super::plays::heal::wish(
        bot.features.stock.pools,
        me.health,
        view.world_time,
        me.carrying.is_some(),
        &bot.features.stock.pantry,
        &tuning.heal,
        time,
    ));
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
    // Who has the body: a script's keeps the reflex's legs (and, if told, its aim and trigger) from it (`control.rs`).
    // (What the reflex wished for the legs is recorded as it wished it, for a client test to read.)
    #[cfg(any(test, feature = "client-test"))]
    let wished_legs = over.legs;
    let over = bot.control.filter(over);
    if let Some(lapse) = bot.reflex.take_lapse() {
        bot.sight_counts.add(lapse);
    }
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
    // [traps] The bear-trap habit: when nothing calls the bot away (no
    // target, no shot, no use), the view goes down at the floor point and the
    // trap's key is pressed one tick and let go the next. A tick it does
    // nothing leaves everything as it was.
    let calm = over.look_at.is_none()
        && !over.fire
        && over.legs.is_none()
        && !using
        && bot.reflex.target().is_none();
    let reacts = bot.control.reacts();
    let trap = if reacts { traps::drive(bot, me, time, calm, intent.crouch, &tuning.traps) } else { None };
    let aim = match trap.and_then(|trap| trap.look) {
        Some(traps::LookAt::Point(point)) => Some(aim_at(point)),
        Some(traps::LookAt::Angles { yaw, pitch }) => Some((yaw, pitch)),
        None => aim,
    };
    if let Some(trap) = trap {
        bot.press |= trap.key;
    }
    // [trap_react] A noticed trap, shot: the view at its centre, the aim
    // held and the fire key pressed a tick per shot, the legs still.
    let react = if reacts { trap_react::drive(bot, me, view, time, calm, &tuning.traps.react) } else { None };
    let aim = match react.and_then(|react| react.look) {
        Some(point) => Some(aim_at(point)),
        None => aim,
    };
    if let Some(react) = react {
        bot.press |= react.keys;
    }
    // The legs: the reflex's when it takes them (a push, breaking sight,
    // a re-peek, the weapon's range, in after a grenade), else the play's.
    let mut intent = intent;
    if let Some(react) = react {
        intent.go = match react.go {
            Some(at) => {
                intent.pace = Pace::Walk;
                intent.straight = false;
                intent.objective = false;
                Go::To { at, within_m: 0.3 }
            }
            None => Go::Stay,
        };
        intent.crouch |= react.crouch;
    }
    #[cfg(test)]
    {
        bot.bench_legs = over.legs.is_some() && !using;
    }
    #[cfg(any(test, feature = "client-test"))]
    {
        bot.bench_legs_why = wished_legs.filter(|_| !using).map(|legs| legs.why);
    }
    // The bench's stand-in camper and a script keep to their spots.
    let held = bot.control.holds_legs();
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
    // [grenade_avoid] A grenade it knows of that lands or lies within its
    // reach: out of the blast first, whatever the play wants (the reflex
    // still aims and fires); `grenade_threat.rs`. The state is read while a
    // grenade is listed or it is not unaware of the last one.
    if reacts
        && tuning.avoid.grenade_avoid
        && (!view.grenades.is_empty()
            || !matches!(bot.features.avoid.state, grenade_threat::GrenadeThreat::Unaware { .. }))
    {
        let around = BlastAround {
            tactics,
            veil: view.smoke.veil(view.clock, &tuning.smoke),
            stuck: &bot.stuck,
            side: bot.side,
            time,
            objective: intent.objective,
        };
        let tick = grenade_threat::Tick {
            now: time,
            me,
            grenades: &view.grenades,
        };
        let action = bot.features.avoid.tick(tick, &tuning.avoid, &mut bot.rng, &around);
        // One trace line for each decision of the avoidance (noticed, set out, no way out, refused).
        if let Some(decision) = bot.features.avoid.decided.take()
            && bot.tracing
        {
            let mut line = decision.trace();
            line["t"] = time.into();
            line["bot"] = bot.place.into();
            bot.traced.push(line);
        }
        if let grenade_threat::Action::Go(to) = action {
            intent.go = Go::To {
                at: to,
                within_m: AVOID_ARRIVE_M,
            };
            intent.crouch = false;
            intent.pace = Pace::Run;
            intent.objective = false;
            intent.straight = false;
        }
    }
    // [traps] The crouch of the press is held to the release: the trap lands where the pose at the press said.
    if let Some(crouch) = trap.and_then(|trap| trap.crouch) {
        intent.crouch = crouch;
    }
    // A rest point on a lip is moved inward (`ThinkTuning::rest_off_lip`): the walk ends with the centre on the floor.
    if reacts
        && tuning.think.rest_off_lip
        && let Go::To { at, within_m } = intent.go
        && let Some(shifted) = path::off_lip(&tactics.world.grid, at)
    {
        path::rests::count();
        intent.go = Go::To { at: shifted, within_m };
    }
    // A body at rest on a lip, wherever its rest came from, steps inward once (`rest`); counted whether on or off.
    {
        let resting = over.legs.is_none()
            && over.look_at.is_none()
            && match intent.go {
                Go::Stay => true,
                Go::To { at, within_m } => me.feet.distance_xz(at) <= within_m,
            };
        if let Some(to) = bot.rest.tick(time, resting, me.speed(), me.feet, &tactics.world.grid, tuning.think.rest_off_lip)
            && reacts
        {
            intent.go = Go::To { at: to, within_m: super::rest::STEP_ARRIVE_M };
            intent.straight = false;
        }
    }
    #[cfg(test)]
    {
        bot.bench_go = Some(intent.go);
    }
    let goal = match intent.go {
        Go::To { at, within_m } if me.feet.distance_xz(at) > within_m => Some(at),
        _ => None,
    };
    // [K1] In a held shooter's lane on its way somewhere, a bot that was to
    // walk runs: it crosses fast and keeps its task (the owner, 2026-10-26:
    // "bots should still continue accomplish their tasks"). Not in a fight
    // (the reflex's legs), not crouched.
    if reacts
        && tuning.threat.sprint
        && goal.is_some()
        && over.legs.is_none()
        && intent.pace == Pace::Walk
        && !intent.crouch
        && !bot.threat.is_empty()
        && bot.threat.exposed_at(&tactics.world.grid, me.feet)
    {
        intent.pace = Pace::Run;
    }
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
        .map_or(ZoneSet::default(), |node| tactics.hazards.set_of(node));
    bot.stuck.zone_on(zone_set);
    let zone_out = if reacts && Gates::wants_zone_way_out(&intent, goal) {
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
    let goal = if intent.straight || !reacts {
        goal
    } else {
        let grid = &tactics.world.grid;
        #[cfg(any(test, feature = "jump-test"))]
        let had_route = bot.route.is_some();
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
        #[cfg(any(test, feature = "jump-test"))]
        if had_route && bot.route.is_none() {
            bot.route_ended(RouteEnd::EscapeStep, time);
        }
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
            bot.end_route(RouteEnd::Straight, time);
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
            let at = match bot.features.crossing.goal {
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
                .features.crossing.goal
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
                    bot.features.crossing.goal = None;
                    bot.end_route(RouteEnd::CrossingAborted, time);
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
                bot.end_route(RouteEnd::ZoneShut, time);
            }
            let replan = replan || shut;
            // [K1 A3] The team's cover changed since its route was planned
            // (a lane held or cast after it set off): planned again when more
            // than `threat::EXPOSED_M` of its way ahead is in the lanes now.
            // Looked at once a change.
            // `planned_with` moves only when a search runs (the K1 block
            // below), so a replan starved by the poll's budget is asked again
            // the next tick; a way found clear is not looked at again until
            // the cover changes.
            let changed = |was: &Option<Arc<threat::Covered>>| was.as_ref().is_none_or(|was| !Arc::ptr_eq(was, &bot.threat));
            // The bench's judgment of an A3 replan: its next route against the
            // way ahead it replaced (a new way or the same, less exposed, out
            // of the lanes), once the new route is in place.
            #[cfg(test)]
            if let Some(asked) = bot.features.threat.asked {
                let grid = &tactics.world.grid;
                let (covered, heights) = (&bot.threat, bot.motor.heights());
                if bot.route_born != asked.born
                    && let Some((route, _)) = bot.route.as_mut()
                {
                    let (walked, _) = route.progress(me.feet);
                    let now_m = threat::exposure_from(route, walked, heights, |p| covered.exposed_at(grid, p), &[me.feet, at]).metres;
                    let left_m = route.length() - walked;
                    threat::judged(asked, now_m, left_m);
                    bot.features.threat.asked = None;
                } else if time >= asked.time + threat::JUDGE_MS {
                    threat::count(threat::Count::ReplanNoRoute);
                    bot.features.threat.asked = None;
                }
            }
            #[cfg(test)]
            let mut ahead = None;
            let threat_ahead = !replan
                && tuning.threat.spots
                && tuning.threat.replan
                && !bot.threat.is_empty()
                && changed(&bot.features.threat.planned_with)
                && changed(&bot.features.threat.clear_with)
                && {
                    let grid = &tactics.world.grid;
                    let (covered, heights) = (&bot.threat, bot.motor.heights());
                    let exposed = bot.route.as_mut().is_some_and(|(route, _)| {
                        let (walked, _) = route.progress(me.feet);
                        let m = threat::exposure_from(route, walked, heights, |p| covered.exposed_at(grid, p), &[me.feet, at]).metres;
                        #[cfg(test)]
                        {
                            ahead = Some((m, route.length() - walked));
                        }
                        m > threat::EXPOSED_M
                    });
                    if !exposed {
                        threat::count(threat::Count::ReplanClear);
                        bot.features.threat.clear_with = Some(bot.threat.clone());
                    }
                    exposed
                };
            let replan = replan || threat_ahead;
            // A goal with no way lately: not searched again every tick.
            let no_way = bot.no_way.is_some_and(|(goal, until)| {
                time < until && goal.distance_xz(at) <= REPLAN_MOVED_M
            });
            // The answer of its search that went on over several polls, if
            // it is for this goal (it walks the way so far meanwhile).
            let found = bot
                .features.crossing.found
                .take()
                .filter(|(goal, _, _)| goal.distance_xz(at) <= REPLAN_MOVED_M);
            // Its own search under way: for this goal, else no longer
            // wanted.
            let waiting = match searching.own_under_way(bot.team, bot.place) {
                Some(goal) => {
                    let still = goal.distance_xz(at) <= REPLAN_MOVED_M;
                    if !still {
                        bot.dropped.dropped(time, goal, at);
                        searching.drop_own(bot.team);
                    }
                    still
                }
                None => false,
            };
            // Given up in its pocket, its goal outside it: no search (the grid
            // has no way, and the next goal is no different): the bot stays, the
            // escape's tries are spent (`escape.rs`, `stuck::SelfKill` at the end).
            let shut_in = tuning.think.shut_in_gate
                && bot
                    .stuck
                    .escape
                    .as_ref()
                    .is_some_and(|escape| escape.trapped().gave_up)
                && {
                    let grid = &tactics.world.grid;
                    let from = grid.locate(me.feet, 1.5).and_then(|n| grid.pocket_of(n));
                    from.is_some()
                        && from != path::goal_floor(grid, at).and_then(|g| grid.pocket_of(g))
                };
            // The bench's count of the given-up episodes: the tick a bot becomes shut in.
            #[cfg(test)]
            {
                if shut_in && !bot.bench_shut_in {
                    search::asks::count(search::asks::Ask::Episodes);
                }
                bot.bench_shut_in = shut_in;
            }
            // Why it has no route, if it has none after this.
            let mut no_route = path::NoRoute::Searched;
            if let Some((goal, route, crossed)) = found {
                bot.no_way = route.is_none().then_some((goal, time + NO_WAY_RETRY_MS));
                bot.features.crossing.goal = crossed.then_some(goal);
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
                let stranded = tuning.think.stranded_start && tactics.world.grid.locate(me.feet, 1.5).is_none();
                let route = route.map(|route| if stranded { stranded_leg(route, me.feet) } else { route });
                bot.set_route(route.map(|route| (route, goal)), time);
            } else if replan && (no_way || shut_in) {
                #[cfg(test)]
                if shut_in {
                    search::asks::count(search::asks::Ask::Skipped);
                }
                bot.end_route(RouteEnd::NoWay, time);
                no_route = if shut_in {
                    path::NoRoute::ShutIn
                } else {
                    path::NoRoute::NoWay
                };
            } else if waiting || (replan && !searching.may_start(bot.team)) {
                #[cfg(test)]
                if !waiting {
                    search::queue::add(bot.team, search::queue::STARVED, 1);
                    searching.starved(bot.team);
                    bot.bench_starved_polls += 1;
                }
                // Its search goes on, or none left to this poll: the old
                // route meanwhile.
                no_route = if waiting {
                    path::NoRoute::Searching
                } else {
                    path::NoRoute::Starved
                };
            } else if let Some(mut grant) = replan.then(|| searching.grant(bot.team, bot.place)).flatten() {
                // A search granted (`Searching`, the one owner of the drive's
                // budget): the floors it may spend, booked when it drops.
                #[cfg(test)]
                {
                    search::queue::add(bot.team, search::queue::GRANTED, 1);
                    if bot.bench_starved_polls > 0 {
                        search::queue::add(bot.team, search::queue::WAITED_POLLS, u64::from(bot.bench_starved_polls));
                        search::queue::add(bot.team, search::queue::WAITED, 1);
                        bot.bench_starved_polls = 0;
                    }
                }
                // The bench's tag of this search (and of its resumed part).
                #[cfg(test)]
                let _tag = {
                    let job = bot.assignment.as_ref().is_some_and(|a| matches!(a.target, plays::TargetRef::ThreatSpot(_)));
                    let tag = if threat_ahead {
                        path::tag::REPLAN
                    } else if job {
                        path::tag::JOB
                    } else {
                        path::tag::OTHER
                    };
                    bot.features.threat.search_tag = tag;
                    path::tag::Scope::new(tag)
                };
                // A search starts: its route is planned against the cover of
                // now (A3 asks again only when the cover changes after this).
                if threat_ahead {
                    threat::count(threat::Count::ReplanAsked);
                    #[cfg(test)]
                    if let Some((exposed_m, left_m)) = ahead {
                        bot.features.threat.asked = Some(threat::Asked { exposed_m, left_m, born: bot.route_born, time });
                    }
                }
                bot.features.threat.planned_with = Some(bot.threat.clone());
                let grid = &tactics.world.grid;
                #[cfg(test)]
                {
                    let in_pocket = grid
                        .locate(me.feet, 1.5)
                        .is_some_and(|node| grid.in_pocket(node));
                    search::asks::searched(&mut bot.bench_asked, time, in_pocket);
                }
                let side = bot.side;
                let objective = intent.objective;
                // Round the spots it got stuck at, if there is a way; else
                // through them (a second search only when there are any).
                // The landings of jumps it gave up: shut to all its searches
                // (a fact about this body, not a preference like the spots).
                let landings = bot.stuck.shut_landings(time).collect();
                let mut hard = traps::filter(bot, landings, grid.locate(me.feet, 1.5));
                // A body that cannot sprint takes no jump link above its standing jump.
                if let Some(rise) = bot.motor.feel.carrier_jump_m.map(f32::from).filter(|_| me.carrying.is_some()) {
                    hard = hard.with_max_jump_rise(rise);
                }
                let filter = traps::soften(
                    bot,
                    hard.clone().avoiding(bot.stuck.avoided_for(time, at).collect()),
                    grid.locate(me.feet, 1.5),
                );
                let avoiding = filter.has_soft();
                // On no floor (the top of an object, 2.5 m or more above the floors): plans from the floor it
                // drops to (`ThinkTuning::stranded_start`), its first leg the drop off the edge.
                let on_floor = grid.locate(me.feet, 1.5);
                let stranded = tuning.think.stranded_start && on_floor.is_none();
                let start_floor = on_floor.or_else(|| stranded.then(|| path::stranded_floor(grid, me.feet)).flatten());
                let ends = start_floor.zip(path::goal_floor(grid, at));
                // Already on the goal's floor (the nearest to a goal on none):
                // as near as it gets, nothing to search. A stranded body is not on it yet.
                let there = !stranded && ends.is_some_and(|(from, to)| from == to);
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
                let floors = &mut grant.floors;
                // Left under way past this poll's floors: whether round the
                // spots, and the search.
                let mut left = None;
                let mut route = if escaping {
                    let route = ends.and_then(|(from, _)| {
                        tactics.escape(from, Some(at), time, side, objective, &hard)
                    });
                    // [zones] Out of the same zones toward the same goal
                    // twice lately: the way there keeps going into them;
                    // the goal is given up for the play to choose again.
                    if let Some((from, _)) = ends
                        && route.is_some()
                        && zone_out.is_none()
                        && !tactics.hazards.set_of(from).is_none()
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
                    match tactics.start_route(from, to, time, side, objective, &filter, floors) {
                        Ok(None) if avoiding => {
                            match tactics.start_route(
                                from,
                                to,
                                time,
                                side,
                                objective,
                                &hard,
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
                                &hard,
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
                // A held shooter's lane (K1, `threat.rs`): a route that runs
                // through it for long gets a local detour round the lane
                // (searched from 13 m before it to 13 m after it, spliced in)
                // where that is not much longer; else the plain route is kept
                // and walked fast (`threat.sprint` below). A route that
                // crosses no lane is the plain route and costs no search; one
                // past this poll's floors is not looked at.
                let mut detour = None;
                if !escaping
                    && !there
                    && left.is_none()
                    && bot.control.reacts()
                    && tuning.threat.spots
                    && !bot.threat.is_empty()
                    && let Some(plain) = route.as_ref()
                {
                    let seen = threat::exposure(plain, bot.motor.heights(), |p| bot.threat.exposed_at(grid, p), &[me.feet, at]);
                    threat::checked(&seen);
                    if seen.metres > threat::EXPOSED_M
                        && tuning.threat.detour
                        && tuning.threat.whole_route
                        && let Some((n0, n1)) = ends
                        && n0 != n1
                    {
                        // [K1 A2] The way weighed round his lanes on the whole
                        // route (the regions' search, a covered region's walk
                        // costing `lane_cost` more), the floors searched in that
                        // room only: kept when not much longer than the plain
                        // route and less exposed.
                        let regions = tactics.regions();
                        let free = threat::free_regions(&bot.threat, tactics, n0, n1);
                        let start = grid.position(n0);
                        let room = threat::costed_corridor(
                            grid,
                            regions,
                            regions.of(n0),
                            regions.of(n1),
                            &bot.threat,
                            &free,
                            tuning.threat.lane_cost,
                            |r| tactics.region_allowed_from(r, time, side, objective, start),
                        );
                        if let Some(room) = room {
                            // The plain route's soft rules (the stuck spots, the
                            // traps it noticed), kept to the room; with
                            // `lane_weighted` the fine search weighs the lanes
                            // and the soft rules as costs, else shuts them.
                            let soft = traps::soften(
                                bot,
                                hard.clone().avoiding(bot.stuck.avoided_for(time, at).collect()),
                                Some(n0),
                            )
                            .keeping_to(Arc::new(room), tactics.region_lookup());
                            let avoid = if tuning.threat.lane_weighted {
                                soft.weighing_threat(bot.threat.clone(), start, grid.position(n1), tuning.threat.lane_cost)
                            } else {
                                soft
                            };
                            // Its own floors (`whole_route_floors`), not what the
                            // poll's other searches left: with the remainder it ran
                            // out in 29 of 30 searches at about 1,900 floors (p1-bench,
                            // 2026-10-29). Taken off the poll's all the same, so the
                            // searches after it in this poll wait.
                            let cap = tuning.threat.whole_route_floors as usize;
                            let mut mine = cap;
                            #[cfg(test)]
                            let _k1 = path::tag::Scope::new(path::tag::K1);
                            let way = tactics.start_route(n0, n1, time, side, objective, &avoid, &mut mine);
                            *floors = floors.saturating_sub(cap - mine);
                            let mut kept_m = seen.metres;
                            match way {
                                Ok(Some(round)) if stuck::worth_round(round.length(), plain.length()) => {
                                    let less = threat::exposure(&round, bot.motor.heights(), |p| bot.threat.exposed_at(grid, p), &[me.feet, at]);
                                    if less.metres < seen.metres {
                                        threat::searched(threat::Searched::Round);
                                        threat::rounded(bot.place, time);
                                        kept_m = less.metres;
                                        detour = Some(round);
                                    } else {
                                        // No less exposed than the way through: counted with the long ones.
                                        threat::searched(threat::Searched::TooLong);
                                    }
                                }
                                Ok(Some(_)) => threat::searched(threat::Searched::TooLong),
                                Ok(None) => threat::searched(threat::Searched::NoWay),
                                Err(_) => threat::searched(threat::Searched::Budget),
                            }
                            threat::spent(cap - mine, seen.metres, kept_m);
                        } else {
                            threat::searched(threat::Searched::NoWay);
                        }
                    } else if seen.metres > threat::EXPOSED_M
                        && tuning.threat.detour
                        && !tuning.threat.whole_route
                        && let Some(&(first, last)) = seen.spans.first()
                    {
                        let s0 = (first - threat::DETOUR_MARGIN_M).max(0.0);
                        let s1 = (last + threat::DETOUR_MARGIN_M).min(plain.length());
                        let ends = grid
                            .locate(plain.at(s0, bot.motor.heights()), 1.5)
                            .zip(grid.locate(plain.at(s1, bot.motor.heights()), 1.5));
                        if let Some((n0, n1)) = ends
                            && n0 != n1
                            && !plain.jumps_between(s0, s1 + 3.0)
                        {
                            let free = threat::free_regions(&bot.threat, tactics, n0, n1);
                            let avoid = hard
                                .clone()
                                .avoiding(bot.stuck.avoided_for(time, at).collect())
                                .avoiding_threat(bot.threat.clone(), grid.position(n0), grid.position(n1), free);
                            // At most `ThreatTuning::search_floors` of the
                            // poll's floors: a search with no way round walks
                            // the whole map first, and the other bots'
                            // searches wait for what it takes.
                            let cap = (*floors).min(tuning.threat.search_floors as usize);
                            let mut mine = cap;
                            let way = tactics.start_route(n0, n1, time, side, objective, &avoid, &mut mine);
                            *floors -= cap - mine;
                            match way {
                                Ok(Some(local)) if stuck::worth_round(local.length(), s1 - s0) => {
                                    threat::searched(threat::Searched::Round);
                                    threat::rounded(bot.place, time);
                                    detour = Some(plain.spliced(s0, s1, &local));
                                }
                                Ok(Some(_)) => threat::searched(threat::Searched::TooLong),
                                Ok(None) => threat::searched(threat::Searched::NoWay),
                                Err(_) => threat::searched(threat::Searched::Budget),
                            }
                            #[cfg(test)]
                            {
                                let kept = detour.as_ref().map_or(seen.metres, |d| {
                                    threat::exposure(d, bot.motor.heights(), |p| bot.threat.exposed_at(grid, p), &[me.feet, at]).metres
                                });
                                threat::spent(cap - mine, seen.metres, kept);
                            }
                        }
                    }
                }
                if detour.is_some() {
                    route = detour;
                }
                // No way, or a long one past zones on all match: a crossing's
                // search (`crossing.rs`).
                let mut crossed = false;
                let mut second = None;
                if left.is_none()
                    && !escaping
                    && !there
                    && !(route.is_some() && long_kept(bot.features.crossing.long_kept, at, time))
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
                    match crossing::start(
                        tactics, from, to, time, side, objective, me.health, &hard, floors,
                    )
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
                                bot.features.crossing.long_kept = Some((at, time + crossing::LONG_RETRY_MS));
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
                        bot.set_route(Some((plain, at)), time);
                        bot.features.crossing.goal = None;
                    }
                    no_route = path::NoRoute::Searching;
                    grant.leave_under_way(Pending {
                        place: bot.place,
                        goal: at,
                        time,
                        side,
                        objective,
                        ends,
                        filter: hard.clone(),
                        hazards: tactics.hazards.version(),
                        planning,
                        health: me.health,
                        second: Some(second),
                    });
                } else if let (Some((round, planning)), Some(ends)) = (left, ends) {
                    // It goes on at the next polls (`Bots::go_on_searching`).
                    // Meanwhile the way so far, as Detour's partial paths:
                    // toward the goal, not standing (else the old route).
                    let filter = if round { filter } else { hard };
                    if let Some(partial) =
                        tactics.partial_route(&planning, ends.0, time, side, objective, &filter)
                    {
                        #[cfg(test)]
                        {
                            bot.bench_route = why::PLAIN;
                        }
                        bot.set_route(Some((partial, at)), time);
                        #[cfg(test)]
                        {
                            bot.bench_on_partial = true;
                        }
                    }
                    no_route = path::NoRoute::Searching;
                    grant.leave_under_way(Pending {
                        place: bot.place,
                        goal: at,
                        time,
                        side,
                        objective,
                        ends,
                        filter,
                        hazards: tactics.hazards.version(),
                        planning,
                        health: me.health,
                        second: None,
                    });
                } else {
                    #[cfg(test)]
                    if let Some((from, to)) = ends
                        && !escaping
                        && !there
                    {
                        path::request_gap(
                            route.is_some(),
                            path::Gap::of(grid.position(to).y - grid.position(from).y),
                        );
                    }
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
                    let route = route.map(|route| if stranded { stranded_leg(route, me.feet) } else { route });
                    bot.set_route(route.map(|route| (route, at)), time);
                    bot.features.crossing.goal = crossed.then_some(at);
                }
            }
            // The bench's: a drive tick walked on a search's way so far.
            #[cfg(test)]
            if bot.bench_on_partial && bot.route.is_some() {
                search::queue::add(bot.team, search::queue::PARTIAL_TICKS, 1);
                bot.bench_partial_at = Some(time);
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
                    bot.motor.exact_end = (exact_at == Some(*route_at)).then(|| intent.exact_arrive_m.unwrap_or(bot.motor.feel.arrive_m));
                    let side = bot.side;
                    let objective = intent.objective;
                    let clear = |p: Vec3| tactics.clear_line_for(me.feet, p, time, side, objective);
                    // On a lip's floors the walk keeps to the route's line: a line to a point further on cuts the
                    // corner it turns at over the rim.
                    bot.motor.ahead_cap = {
                        let grid = &tactics.world.grid;
                        let cap = bot.motor.feel.lip_look_ahead_m.get();
                        (cap > 0.0 && grid.locate(me.feet, 1.5).is_some_and(|node| grid.near_lip(node) || grid.is_lip(node))).then_some(cap)
                    };
                    #[cfg(test)]
                    {
                        bot.bench_walk = if aim.is_some() { 2 } else { 1 };
                    }
                    let walks_the_route = aim.is_none();
                    let (decision, walked) = match aim {
                        Some(aim) => {
                            walking_aimed = true;
                            bot.motor.walk_aiming(route, &pose, OFF_ROUTE_M, clear, aim)
                        }
                        None => bot.motor.walk(route, &pose, OFF_ROUTE_M, clear),
                    };
                    // Pressing against something while walking the route: counted, nothing is decided from it.
                    let counted = bot.contact.tick(time, walks_the_route && walked == Walked::OnTheWay, decision.keys, me.speed());
                    contact::contacts::count(counted, me.feet);
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
                            // The jump's landing, from the route: shut to its
                            // searches (`Stuck::failed_jump`).
                            let landing = route
                                .points()
                                .iter()
                                .enumerate()
                                .find(|&(k, point)| {
                                    k >= 1
                                        && point.jump.is_some()
                                        && route.points()[k - 1].at.distance_xz(take_off) < 0.3
                                })
                                .map(|(_, point)| point.at);
                            bot.stuck.failed_jump(time, take_off, landing);
                        }
                        bot.end_route(
                            match walked {
                                Walked::Arrived => RouteEnd::Arrived,
                                Walked::Off(_) => RouteEnd::Off,
                                _ => RouteEnd::JumpFailed,
                            },
                            time,
                        );
                        path::no_route(bot.team, path::NoRoute::Ended);
                    }
                    decision
                }
                None => {
                    path::no_route(bot.team, no_route);
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
            bot.end_route(RouteEnd::NoGoal, time);
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
    #[cfg(test)]
    {
        bot.bench_stepped = 0;
        bot.bench_friend_step = 0;
    }
    // [friends] A mate who blocks by touch alone (the line is clear of him): a step back out of his radius, where
    // the ground behind is clear; a step across the line would barely grow the distance to a mate straight ahead.
    if over.friend_step == Some(super::friends::FriendStep::Back) && !using && (aiming_still || walking_aimed) {
        let keys = back_keys(decision.keys);
        let to = me.feet + unstick_step(keys, me.yaw);
        let clear = tactics.clear_step_for(me.feet, to, time, bot.side, intent.objective);
        #[cfg(test)]
        {
            bot.bench_friend_step = if clear { 3 } else { 4 };
        }
        if clear {
            decision.keys = keys;
        }
    }
    let stepped = if aiming_still {
        Some(decision.keys | over.strafe)
    } else if walking_aimed && tuning.aim.combat.strafe_walking {
        weave(decision.keys, over.strafe)
    } else {
        None
    };
    if let Some(keys) = stepped.filter(|_| over.strafe != 0 && !using) {
        let (side, objective) = (bot.side, intent.objective);
        let ask = |keys: u32, strafe: u32| {
            let to = me.feet + side_step(keys, strafe, me.yaw, tactics.clear_step_sees_lip);
            tactics.clear_step_for(me.feet, to, time, side, objective)
        };
        let slides = tuning.aim.combat.clear_step_slides_along;
        let (step, other_clear) = choose_step(
            keys,
            over.strafe,
            matches!(over.friend_step, Some(super::friends::FriendStep::Side { .. })),
            tactics.clear_step_sees_lip,
            slides,
            cfg!(test),
            ask,
        );
        #[cfg(test)]
        {
            let clear = matches!(step, Step::Own(_));
            motor::counts::side_step(clear);
            if matches!(over.friend_step, Some(super::friends::FriendStep::Side { .. })) {
                bot.bench_friend_step = if clear { 1 } else { 2 };
            }
            bot.bench_stepped = match (clear, aiming_still) {
                (true, true) => 1,
                (true, false) => 2,
                (false, _) => 3,
            };
            if let Some(other_clear) = other_clear {
                let grid = &tactics.world.grid;
                let in_band = grid
                    .locate(me.feet, 1.0)
                    .is_some_and(|node| grid.near_lip(node));
                motor::counts::slide_asked(other_clear, !aiming_still, in_band);
            }
            if matches!(step, Step::Slid(_)) {
                motor::counts::slid();
            }
        }
        #[cfg(not(test))]
        let _ = other_clear;
        if let Step::Own(keys) | Step::Slid(keys) = step {
            decision.keys = keys;
        }
    }
    // [lip guard] On a lip's floors the keys of this tick are played forward to a stop: a key that carries the
    // body over the rim (the walk's own diagonal, a side step, the eight-way keys of a walk that aims) is let
    // go for one that stops on the floor, unless the route drops there. Every unplanned fall of the bench was
    // off a lip (`WalkGrid::near_lip`), with the body's keys pushing it over or its glide carrying it.
    guard_keys(bot, &mut decision, me, tactics, tuning, time);
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
        decision.keys &= !key::CROUCH;
        // A sprint jump is run up to at a sprint: `Feel::band_jump_gate`.
        if !bot.motor.keeps_sprint() {
            decision.keys &= !key::SPRINT;
        }
    }
    if decision.keys & key::QUICK[0] != 0 {
        bot.features.stock.medkits_used = bot.features.stock.medkits_used.saturating_add(1);
    }
    for (quick, bit) in key::QUICK.into_iter().enumerate() {
        if decision.keys & bit != 0 {
            bot.features.stock.pantry.pressed(quick, time);
        }
    }
    let by = bot.motor.jumped().map(|by| match by {
        JumpBy::Route => "route",
        JumpBy::Hop => "hop",
    });
    // [trap_react] A noticed trap on the route, a little ahead: the same jump.
    let by = by.or_else(|| (bot.control.reacts() && trap_react::jump_due(bot, me, time, &tuning.traps.react)).then_some("trap"));
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

/// The route's drops this far on (m along it) are the guard's planned falls.
const PLANNED_DROP_M: f32 = 6.0;
/// The guard also looks this far on along the velocity (s) for a lip's floor.
const LIP_LOOK_S: f32 = 0.4;
/// A tick that changed the keys adds this to the [`LipWatch`] score, and one that did not takes 1 off.
const LIP_HELD_SCORE: u8 = 3;
/// The score at which the guard gives a bot up for [`LIP_OFF_MS`].
const LIP_GIVE_UP: u8 = 30;
const LIP_OFF_MS: u32 = 3_000;

/// The lip guard's watch over its own hold on a bot: a guard that keeps changing a bot's keys has stopped its
/// walk, not saved it (a bot braked at a rim it must turn at, again and again). The score rises with each
/// tick the keys are changed and falls with each that they are not; past [`LIP_GIVE_UP`] the guard leaves
/// the bot's keys alone for [`LIP_OFF_MS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LipWatch {
    Watching(u8),
    /// Left off until this time (ms).
    Off(u32),
}

impl Default for LipWatch {
    fn default() -> Self {
        Self::Watching(0)
    }
}

impl LipWatch {
    /// The watch after a tick's verdict at `time`; whether the guard may change the keys this tick.
    fn after(self, guard: LipGuard, time: u32) -> (Self, bool) {
        match self {
            Self::Off(until) if time < until => (self, false),
            Self::Off(_) => Self::Watching(0).after(guard, time),
            Self::Watching(score) => {
                let score = match guard {
                    LipGuard::Swapped(_) | LipGuard::Braked(_) => score.saturating_add(LIP_HELD_SCORE),
                    LipGuard::Doomed | LipGuard::Safe | LipGuard::Clear => score.saturating_sub(1),
                };
                if score >= LIP_GIVE_UP {
                    (Self::Off(time + LIP_OFF_MS), false)
                } else {
                    (Self::Watching(score), true)
                }
            }
        }
    }
}

/// What the lip guard made of a tick's keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LipGuard {
    /// Not on a lip's floors, nor about to be: the keys as they are.
    Clear,
    /// The keys stop on the floor.
    Safe,
    /// These keys instead stop on the floor.
    Swapped(u32),
    /// None stops on the floor: these brake hardest.
    Braked(u32),
    /// None stops on the floor, and the body has no speed to brake.
    Doomed,
}

/// The lip guard: the keys `keys` of a tick (the view turns `turn` radians), played forward over the body
/// model to a stop ([`body::stays_up`]). Those that carry the body off a lip's floor, the walk's own
/// diagonal, a side step, the eight-way keys of a walk that aims, give way to the same without the side keys,
/// to none, or to the brake against the velocity, whichever stops on the floor first. `planned`: the landings
/// of the drops the route makes (a fall there is the route's). Every unplanned fall of the bench was off a
/// lip (`WalkGrid::near_lip`), with the keys pushing the body over or its glide carrying it.
pub(super) fn lip_guard(grid: &mapdata::WalkGrid, kin: &kin::Kinematics, me: &PlaceView, turn: f32, keys: u32, planned: &[Vec3], hold: u32) -> LipGuard {
    const MOVE: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::SPRINT;
    let node = grid.locate(me.feet, 1.5);
    // On a lip's floors, or about to be (the glide of a stop is a stride or more).
    let ahead = Vec3::new(me.feet.x + me.velocity[0] * LIP_LOOK_S, me.feet.y, me.feet.z + me.velocity[1] * LIP_LOOK_S);
    let on_lip = |node: Option<NodeId>| node.is_some_and(|node| grid.near_lip(node) || grid.is_lip(node));
    if keys & MOVE == 0 || hold == 0 || !(on_lip(node) || on_lip(grid.locate(ahead, 1.0))) {
        return LipGuard::Clear;
    }
    let at = body::Estimate::of_view(me, node);
    let stop = body::Stop { kin, level: 1.0, grid, planned, hold };
    let up = |keys: u32| body::stays_up(&at, keys, turn, &stop);
    if up(keys) {
        return LipGuard::Safe;
    }
    let speed = me.velocity[0].hypot(me.velocity[1]);
    let brake = (speed > 0.5).then(|| {
        let relative = kin::wrap(kin::yaw_of([-me.velocity[0], -me.velocity[1]]) - (me.yaw + turn));
        (keys & !MOVE) | motor::keys_toward(relative)
    });
    // The same walk slower (no sprint, then crouched), then without the side keys, then none, then the brake.
    let walk = keys & !key::SPRINT;
    let straight = keys & !(key::LEFT | key::RIGHT);
    let found = [walk, walk | key::CROUCH, straight, (straight & !key::SPRINT) | key::CROUCH, keys & !MOVE]
        .into_iter()
        .chain(brake)
        .filter(|&k| k != keys)
        .find(|&k| up(k));
    match (found, brake) {
        (Some(found), _) => LipGuard::Swapped(found),
        (None, Some(brake)) => LipGuard::Braked(brake),
        (None, None) => LipGuard::Doomed,
    }
}

/// [`lip_guard`] on the tick's decision.
fn guard_keys(bot: &mut Bot, decision: &mut Decision, me: &PlaceView, tactics: &Tactics, tuning: &Tuning, time: u32) {
    #[cfg(test)]
    {
        bot.bench_guard = 0;
    }
    // Off (the default): nothing is looked at.
    if bot.motor.feel.lip_guard_ticks == 0 {
        return;
    }
    // A jump (the route's, a hop) goes off the floor on purpose.
    if decision.keys & key::JUMP != 0 || bot.motor.lining_up() {
        #[cfg(test)]
        {
            bot.bench_guard = 5;
        }
        return;
    }
    let truth = bot.motor.feel.drop_truth;
    let planned = bot.route.as_mut().map_or_else(Vec::new, |(route, _)| {
        let (walked, _) = route.progress(me.feet);
        route.drop_landings(walked, PLANNED_DROP_M, truth)
    });
    let guard = if matches!(bot.lip_watch, LipWatch::Off(until) if time < until) {
        LipGuard::Clear
    } else {
        lip_guard(&tactics.world.grid, &tuning.kin, me, decision.turn.0, decision.keys, &planned, bot.motor.feel.lip_guard_ticks)
    };
    let (watch, may) = bot.lip_watch.after(guard, time);
    bot.lip_watch = watch;
    let guard = if may { guard } else { LipGuard::Clear };
    #[cfg(test)]
    {
        bot.bench_guard_at = (me.velocity, me.feet);
        bot.bench_guard = match guard {
            LipGuard::Clear => 0,
            LipGuard::Safe => 1,
            LipGuard::Swapped(_) => 2,
            LipGuard::Braked(_) | LipGuard::Doomed => 3,
        };
        if matches!(guard, LipGuard::Swapped(_) | LipGuard::Braked(_)) && bot.bench_guard_dbg < 6 && std::env::var_os("SRP_GUARD_DEBUG").is_some() {
            bot.bench_guard_dbg += 1;
            let route = bot.route.as_ref().map(|(r, goal)| (r.length(), r.points().len(), goal.to_array()));
            eprintln!("GUARDDBG t={time} place={} guard={guard:?} keys={:#x} feet={:?} v={:?} planned={planned:?} route={route:?} intent={:?}", bot.place, decision.keys, me.feet.to_array(), me.velocity, bot.intent.go);
        }
        match guard {
            LipGuard::Swapped(_) => motor::counts::lip_guard(true),
            LipGuard::Braked(_) | LipGuard::Doomed => motor::counts::lip_guard(false),
            _ => {}
        }
    }
    if let LipGuard::Swapped(keys) | LipGuard::Braked(keys) = guard {
        decision.keys = keys;
    }
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

    /// The side step is tested over the move the keys make: a weave of a walk
    /// forward is the diagonal, not the pure side step; with the switch off,
    /// or keys that cancel, the pure side step of the `strafe` side.
    #[test]
    fn the_step_back_drops_forward_and_sprint_and_keeps_the_side_keys() {
        assert_eq!(back_keys(0), key::BACK);
        assert_eq!(back_keys(key::FORWARD | key::SPRINT), key::BACK);
        assert_eq!(back_keys(key::FORWARD | key::LEFT), key::BACK | key::LEFT);
        assert_eq!(back_keys(key::BACK | key::RIGHT), key::BACK | key::RIGHT);
    }

    #[test]
    fn the_side_step_is_tested_over_the_move_the_keys_make() {
        let pure = |yaw: f32, left: bool| {
            let [sx, sz] = kin::forward(yaw + std::f32::consts::FRAC_PI_2);
            let sign = if left { 1.0 } else { -1.0 };
            Vec3::new(sign * sx * STRAFE_CLEAR_M, 0.0, sign * sz * STRAFE_CLEAR_M)
        };
        let near = |a: Vec3, b: Vec3| (a - b).length() < 1e-4;
        for yaw in [0.0_f32, 0.7, 2.5, -1.9] {
            for (strafe, left) in [(key::LEFT, true), (key::RIGHT, false)] {
                // Standing: the pure side step either way.
                for by_keys in [false, true] {
                    let moved = side_step(strafe, strafe, yaw, by_keys);
                    assert!(near(moved, pure(yaw, left)));
                }
                // Off: the pure side step whatever the walk.
                for walk in [key::FORWARD, key::BACK] {
                    let moved = side_step(walk | strafe, strafe, yaw, false);
                    assert!(near(moved, pure(yaw, left)));
                }
                // On: a weave forward or back is the diagonal, a stride long.
                for walk in [key::FORWARD, key::BACK] {
                    let moved = side_step(walk | strafe, strafe, yaw, true);
                    assert!(near(moved, unstick_step(walk | strafe, yaw)));
                    assert!((moved.length() - STRAFE_CLEAR_M).abs() < 1e-4);
                    assert!(!near(moved, pure(yaw, left)));
                    // Its side is the step's side.
                    assert!(moved.dot(pure(yaw, left)) > 0.0);
                }
            }
            // Keys that cancel move nowhere: the pure side step.
            let moved = side_step(key::LEFT | key::RIGHT, key::LEFT, yaw, true);
            assert!(near(moved, pure(yaw, true)));
        }
    }

    /// The merge is switched off: the tuning's default (and the tactics') is the old step.
    #[test]
    fn the_lip_step_switch_is_off_by_default() {
        assert!(!Tuning::default().aim.combat.clear_step_sees_lip);
        let patched: Tuning = serde_json::from_value(
            serde_json::json!({"aim": {"combat": {"clear_step_sees_lip": true}}}),
        )
        .unwrap();
        assert!(patched.aim.combat.clear_step_sees_lip);
    }

    /// The fight's side step: its own side if clear; refused at a lip, the other side when `slides`, else it
    /// stands; the step away from a teammate is never turned over; with the lip switch off it stands as it did.
    #[test]
    fn a_refused_side_step_slides_to_the_clear_side_only_when_asked_to() {
        let (walk, left, right) = (key::FORWARD, key::LEFT, key::RIGHT);
        // The left side is refused, the right clear; and the reverse.
        let only = |clear: u32| move |_: u32, strafe: u32| strafe == clear;
        let choose = |keys, strafe, away, sees, slides, ask: &dyn Fn(u32, u32) -> bool| {
            choose_step(keys, strafe, away, sees, slides, false, ask)
        };
        for (keys, strafe) in [
            (left, left),
            (walk | left, left),
            (right, right),
            (walk | right, right),
        ] {
            let other = if strafe == left { right } else { left };
            let flipped = (keys & !(left | right)) | other;
            let (own, ask_other) = (only(strafe), only(other));
            // Its own side clear: taken.
            assert_eq!(
                choose(keys, strafe, false, true, true, &own).0,
                Step::Own(keys)
            );
            // Refused, the other clear: slides with the switch, stands without it or with the lip switch off.
            assert_eq!(
                choose(keys, strafe, false, true, true, &ask_other).0,
                Step::Slid(flipped)
            );
            assert_eq!(
                choose(keys, strafe, false, true, false, &ask_other).0,
                Step::Stand
            );
            assert_eq!(
                choose(keys, strafe, false, false, true, &ask_other).0,
                Step::Stand
            );
            // The step away from a teammate is never turned over.
            assert_eq!(
                choose(keys, strafe, true, true, true, &ask_other).0,
                Step::Stand
            );
            // Both refused: it stands.
            assert_eq!(
                choose(keys, strafe, false, true, true, &|_, _| false).0,
                Step::Stand
            );
            // The weave keeps its walk key: only the side key changes.
            assert_eq!(
                flipped & (key::FORWARD | key::BACK),
                keys & (key::FORWARD | key::BACK)
            );
        }
        // The other side is asked for the counts when the switch is off (nothing changes), and only then.
        let counted = choose_step(left, left, false, true, false, true, only(right));
        assert_eq!(counted, (Step::Stand, Some(true)));
        assert_eq!(
            choose_step(left, left, false, true, false, false, only(right)),
            (Step::Stand, None)
        );
        assert_eq!(
            choose_step(left, left, true, true, false, true, only(right)),
            (Step::Stand, None)
        );
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

    /// The ticks the tests count the keys held.
    const HOLD: u32 = 2;

    /// A body of the slab 2 m over the field, at `x` on its z = 33 line, walking at `speed` along `yaw`.
    fn on_the_slab(x: f32, yaw: f32, speed: f32) -> PlaceView {
        let [fx, fz] = kin::forward(yaw);
        PlaceView {
            place: 0,
            team: 0,
            bot: true,
            present: true,
            alive: true,
            feet: Vec3::new(x, 2.0, 33.0),
            yaw,
            pitch: 0.0,
            velocity: [fx * speed, fz * speed],
            keys: key::FORWARD,
            health: 1.0,
            spawned: 0,
            kills: 0,
            deaths: 0,
            streak: 0,
            carrying: None,
            seen_at: 0,
            late_ms: 0,
            known_at: 0,
        }
    }

    fn slab() -> Arc<World> {
        let solids = crate::v026g::bots::body::tests::floor(50.0, 30.0, 56.0, 36.0, 2.0);
        crate::v026g::bots::tactics::tests::field_with_solids(&solids, |_| {})
    }

    /// A walk at a 2 m drop's rim is braked where its stop would take the body over, and left alone
    /// walking away from it, along it, far from it, or to a landing the route drops to.
    #[test]
    fn a_walk_that_would_carry_the_body_over_a_rim_is_let_go() {
        let world = slab();
        let grid = &world.grid;
        let kin = kin::Kinematics::default();
        let toward = -std::f32::consts::FRAC_PI_2; // facing +x, the rim at x = 56
        let guard = |me: &PlaceView, planned: &[Vec3]| lip_guard(grid, &kin, me, 0.0, key::FORWARD, planned, HOLD);
        let at_rim = on_the_slab(55.4, toward, 3.5);
        // It brakes: the stop from 55.4 at 3.5 m/s is past the rim even braking (the reckoning's margin).
        let swapped = |guard: LipGuard| match guard {
            LipGuard::Swapped(keys) | LipGuard::Braked(keys) => Some(keys),
            _ => None,
        };
        assert_eq!(swapped(guard(&at_rim, &[])), Some(key::BACK));
        // The same body walking away, along the rim, or from far inside.
        assert_eq!(guard(&on_the_slab(55.4, -toward, 3.5), &[]), LipGuard::Safe);
        assert_eq!(guard(&on_the_slab(55.6, 0.0, 3.5), &[]), LipGuard::Safe);
        assert_eq!(guard(&on_the_slab(52.0, toward, 3.5), &[]), LipGuard::Clear);
        // The route drops off this rim, to a landing past it: its fall is planned.
        assert_eq!(guard(&at_rim, &[Vec3::new(56.5, 0.0, 33.0)]), LipGuard::Safe);
        // A landing elsewhere is not.
        assert_eq!(swapped(guard(&at_rim, &[Vec3::new(40.0, 0.0, 10.0)])), Some(key::BACK));
        // With no keys held there is nothing to let go of.
        assert_eq!(lip_guard(grid, &kin, &at_rim, 0.0, 0, &[], HOLD), LipGuard::Clear);
    }

    /// A sprint at a rim is slowed to a walk or a creep before it is stopped, and the walk goes on where a
    /// slower one stops short.
    #[test]
    fn a_sprint_at_a_rim_is_slowed_before_it_is_stopped() {
        let world = slab();
        let kin = kin::Kinematics::default();
        let toward = -std::f32::consts::FRAC_PI_2;
        let keys = key::FORWARD | key::SPRINT;
        let mut slowed = 0;
        for x in [53.5f32, 54.0, 54.5, 55.0, 55.3] {
            let me = on_the_slab(x, toward, 5.0);
            match lip_guard(&world.grid, &kin, &me, 0.0, keys, &[], HOLD) {
                LipGuard::Swapped(k) => {
                    assert_eq!(k & key::SPRINT, 0, "{x}: {k:#x}");
                    slowed += u32::from(k & key::FORWARD != 0);
                }
                LipGuard::Braked(_) | LipGuard::Doomed | LipGuard::Safe | LipGuard::Clear => {}
            }
        }
        assert!(slowed > 0);
    }

    #[test]
    fn the_guard_gives_a_bot_up_that_it_keeps_stopping() {
        let mut watch = LipWatch::default();
        let mut time = 0;
        let mut may = true;
        // Keys changed tick after tick: given up at the score.
        for _ in 0..(LIP_GIVE_UP / LIP_HELD_SCORE + 1) {
            (watch, may) = watch.after(LipGuard::Swapped(0), time);
            time += 50;
        }
        assert!(!may);
        assert!(matches!(watch, LipWatch::Off(_)));
        // Left alone while it is off, picked up again after.
        assert!(!watch.after(LipGuard::Swapped(0), time).1);
        let LipWatch::Off(until) = watch else { unreachable!() };
        assert_eq!(watch.after(LipGuard::Safe, until).0, LipWatch::Watching(0));
        // A change every other tick adds up too, a change now and then does not.
        let (mut watch, mut tripped) = (LipWatch::default(), false);
        for k in 0..200 {
            let guard = if k % 2 == 0 { LipGuard::Swapped(0) } else { LipGuard::Safe };
            let (next, may) = watch.after(guard, k * 50);
            watch = next;
            tripped |= !may;
        }
        assert!(tripped);
        let (mut watch, mut tripped) = (LipWatch::default(), false);
        for k in 0..200 {
            let guard = if k % 10 == 0 { LipGuard::Swapped(0) } else { LipGuard::Safe };
            let (next, may) = watch.after(guard, k * 50);
            watch = next;
            tripped |= !may;
        }
        assert!(!tripped);
    }

    #[test]
    fn a_use_takes_the_view_when_in_reach_and_the_reflex_holds_none_for_a_grenade() {
        let eye = super::view::still_eye(Vec3::ZERO, false);
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

use super::pitch::Pitch;
use super::reflex::{Legs, LegsWhy};
use super::hazards::ZoneSet;
use super::tactics::StepOutPick;
use super::*;

/// The own eye is led at most this far past its last decided tick
/// (`CombatTuning::lead_own`): a tick, or two after a skipped one.
pub(super) const LEAD_OWN_MOST_MS: u32 = 100;

/// A body farther than this off its route plans again, metres.
pub(super) const OFF_ROUTE_M: f32 = 3.0;

/// A lane's shifted steering point is on the route's floor when its grid floor is within this of the route point's height, m.
const LANE_FLOOR_M: f32 = 0.5;

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

/// An enemy the bot's or its team's own senses gave (a cheat's place does not count) this near the body, metres, within
/// [`FIGHT_KNOWN_MS`]: it is in a fight ([`Feel::fight_lift`]). A GUESS to tune on the bench.
const FIGHT_M: f32 = 30.0;
/// ... and this long since, ms.
const FIGHT_KNOWN_MS: u32 = 8_000;

/// Whether the body is in a fight or a rush, for the motor's sprint lift ([`Feel::fight_lift`]): it runs (the play's pace)
/// and an enemy is known near it (fair: the bot's own senses and its team's calls, [`Track::own`]) or it is bound for the
/// enemy base (`Intent::objective`). The intent has no other sign of a rush.
fn urgent(bot: &Bot, intent: &Intent, me: &PlaceView, time: u32) -> bool {
    intent.pace == Pace::Run
        && (intent.objective
            || bot.memory.tracks().iter().any(|track| {
                track
                    .own
                    .is_some_and(|(at, when)| time.wrapping_sub(when) <= FIGHT_KNOWN_MS && at.distance_xz(me.feet) <= FIGHT_M)
            }))
}

/// [measure, #25b] The places of the enemies the bot knows of from its own senses and its team's calls (`Track::own`)
/// within `ms` (the bench's detour path counts; nothing decides by it).
#[cfg(test)]
fn known_enemies(bot: &Bot, time: u32, ms: u32) -> Vec<Vec3> {
    bot.memory
        .tracks()
        .iter()
        .filter_map(|track| track.own.filter(|&(_, when)| time.wrapping_sub(when) <= ms).map(|(at, _)| at))
        .collect()
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

/// What the reflex's wish loses while the legs escape ([`Pace::Escape`]): the sights (the client takes no sprint aimed), the crouch and
/// the side steps always; the key of a grenade, a reload or a weapon switch is its own and stays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Strip {
    /// The trigger too, the shot the reflex counted this tick given back, and the view is the way, not the target.
    Everything,
    /// The trigger and the view stay: a fight in the cloud keeps shooting while the legs run.
    KeepFiring,
}

/// The strip of an escape by its reason: out of a blast the legs are all there is; off a zone's floor a fight goes on.
fn escape_legs(why: EscapeWhy) -> Strip {
    match why {
        EscapeWhy::Grenade => Strip::Everything,
        EscapeWhy::Zone => Strip::KeepFiring,
    }
}

impl Strip {
    /// The reflex's wish for the tick with what slows the legs left out.
    fn of(self, over: reflex::Override) -> reflex::Override {
        reflex::Override {
            keys: over.keys & !(key::AIM | key::CROUCH),
            strafe: 0,
            strafe_away: false,
            fire: over.fire && self == Self::KeepFiring,
            ..over
        }
    }
}

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
        far_m: f32,
        routed: bool,
    ) -> Option<(Vec3, StepOutPick)> {
        let from = self.tactics.world.grid.locate(me.feet, 1.5)?;
        let hard = RouteFilter::shutting_landings(self.stuck.shut_landings(self.time).collect());
        if routed {
            self.tactics.step_out_routes(
                from,
                blast.rest,
                blast.radius,
                reach_m,
                far_m,
                self.time,
                self.side,
                self.objective,
                &hard,
            )
        } else {
            self.tactics
                .step_out(from, blast.rest, blast.radius, reach_m, self.time, self.side, self.objective, &hard)
                .map(|to| (to, StepOutPick::Nearest))
        }
    }
}

/// A planned route with the middle of each doorway it crosses as a corner when `ThinkTuning::door_centres` is on
/// ([`Route::door_centred`]), then its corners kept where the capsule fits when `ThinkTuning::wall_clear_corners` is
/// ([`Route::wall_cleared`]).
fn door_route(route: Route, tuning: &Tuning, grid: &mapdata::grid::WalkGrid) -> Route {
    let route = if tuning.think.door_centres { route.door_centred(grid) } else { route };
    if tuning.think.wall_clear_corners { route.wall_cleared(grid) } else { route }
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
    let mut decision = drive_tick_inner(bot, me, view, tactics, tuning, time, searching);
    // The motor turned the view's elevation; the input carries the game's pitch unit (`pitch.rs`).
    decision.turn.1 = Pitch::view(me.pitch, bot.reflex.kick_before()).turn_for(tuning.kin.pitch_scale, decision.turn.1);
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
    let kick = bot.reflex.weapon_tick(me, time, tuning.kin.pitch_scale);
    bot.reflex.gate_mirror(tuning.aim.combat.mirror_gated);
    // Only this tick's walk to the play's own exact spot sets it (below).
    bot.motor.exact_end = None;
    // The view's pitch is the game's unit; the geometry below reckons in elevations (`pitch.rs`).
    let me = &PlaceView {
        pitch: Pitch::view(me.pitch, kick).elevation(tuning.kin.pitch_scale),
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
    let mut pose = Pose {
        feet: me.feet,
        yaw: me.yaw,
        pitch: me.pitch,
        speed: me.speed(),
        velocity: me.velocity,
        no_jump: !bot.hurt.may_jump(time) || (tuning.think.stamina && !bot.stamina.may_jump()),
        sprint_block: sprint_block(bot, me, time, tuning.think.stamina),
        urgent: false,
        gait: motor::Gait::Walk,
        floor: motor::Floor::Open,
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
        bot.hurt.last_hit_ms,
        me.carrying.is_some(),
        &bot.features.stock.pantry,
        &tuning.heal,
        time,
    ));
    #[cfg(test)]
    super::smoke::tally::set_role(bot.role.map_or(0, |role| role as usize));
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
    let mut over = bot.control.filter(over);
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
        && matches!(intent.pace, Pace::Run | Pace::Cruise | Pace::Arrive)
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
    // [smoke] Standing to throw a smoke keeps the route it was planned for (`smoke.throw_keeps_route`).
    let keeps_route = Gates::keeps_route(over.legs.filter(|_| !using && !held), tuning.smoke.throw_keeps_route);
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
            intent.pace = if tuning.avoid.escape { Pace::Escape(EscapeWhy::Grenade) } else { Pace::Run };
            intent.objective = false;
            intent.straight = false;
        }
    }
    // [zones] On a floor a zone hurts on now, the legs escape (`HazardTuning::run_out`, `Pace::Escape(EscapeWhy::Zone)`): the walk out is at the legs' top speed, the
    // sprint on any route, and the fight goes on (`escape_legs`). The zones kill by a pool that runs out in 4-5 s; a player runs out of a toxic cloud.
    if reacts && tuning.hazard.run_out {
        let hurt = tactics.world.grid.locate(me.feet, 1.5).is_some_and(|node| tactics.hazards.on_at(node, time));
        if Gates::runs_out(intent.pace, intent.crouch, hurt, over.legs.is_none()) {
            intent.pace = Pace::Escape(EscapeWhy::Zone);
            intent.crouch = false;
            #[cfg(test)]
            super::hazards::damage_zone_fixes::asked_by(bot.team, || super::hazards::damage_zone_fixes::count(super::hazards::damage_zone_fixes::Ev::RunOut));
        }
    }
    // [Pace::Escape] Out of somewhere first: what of the reflex's wish slows the legs is let go, by the reason (`escape_legs`).
    let aim = match intent.pace {
        Pace::Escape(why) => {
            let strip = escape_legs(why);
            if over.fire && strip == Strip::Everything {
                bot.reflex.hold_fire(time);
            }
            over = strip.of(over);
            if strip == Strip::Everything {
                bot.motor.track = None;
                None
            } else {
                aim
            }
        }
        _ => aim,
    };
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
        && !bot.sprint_threat.is_empty()
        && bot.sprint_threat.exposed_at(&tactics.world.grid, me.feet)
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
            #[cfg(test)]
            if replan || threat_ahead {
                bot.bench_replan = if threat_ahead {
                    "threat_ahead"
                } else if shut {
                    "zone_shut"
                } else if aborted {
                    "crossing_aborted"
                } else if bot.route.is_none() {
                    "no_route"
                } else {
                    "goal_moved"
                };
            }
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
                // Both ends were on floors, from one it may walk on: proven, unless the body's own shut drops are what
                // shut the way (`Stuck::no_way_is_proof`): a way its mates may walk is no proof for the team.
                if route.is_none() && bot.stuck.no_way_is_proof(time) {
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
                bot.set_route(route.map(|route| (door_route(route, tuning, &tactics.world.grid), goal)), time);
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
                let (floors, overdraft) = (&mut grant.floors, &mut grant.overdraft);
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
                            // 2026-10-29). Booked to the poll all the same (the grant's
                            // floors, then past them: `Overdraft`), so the searches after
                            // it in this poll wait.
                            let cap = tuning.threat.whole_route_floors as usize;
                            let mut mine = cap;
                            #[cfg(test)]
                            let _k1 = path::tag::Scope::new(path::tag::K1);
                            let way = tactics.start_route(n0, n1, time, side, objective, &avoid, &mut mine);
                            if tuning.threat.whole_route_booked {
                                overdraft.spend(floors, cap - mine);
                            } else {
                                *floors = floors.saturating_sub(cap - mine);
                            }
                            let mut kept_m = seen.metres;
                            match way {
                                Ok(Some(round)) if stuck::worth_round(round.length(), plain.length()) => {
                                    let less = threat::exposure(&round, bot.motor.heights(), |p| bot.threat.exposed_at(grid, p), &[me.feet, at]);
                                    if less.metres < seen.metres {
                                        threat::searched(threat::Searched::Round);
                                        #[cfg(test)]
                                        {
                                            use threat::detour_path_tally as dp;
                                            let known = known_enemies(bot, time, dp::KNOWN_MS);
                                            let h = bot.motor.heights();
                                            let (pm, rm) = (plain.length(), round.length());
                                            let near = (dp::near_m(plain, h, 0.0, pm, &known), dp::near_m(&round, h, 0.0, rm, &known));
                                            dp::record(bot.team, pm, rm, near.0, near.1);
                                        }
                                        threat::rounded(bot.place, time, round.length() - plain.length());
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
                                    #[cfg(test)]
                                    {
                                        use threat::detour_path_tally as dp;
                                        let known = known_enemies(bot, time, dp::KNOWN_MS);
                                        let h = bot.motor.heights();
                                        let lm = local.length();
                                        let near = (dp::near_m(plain, h, s0, s1, &known), dp::near_m(&local, h, 0.0, lm, &known));
                                        dp::record(bot.team, s1 - s0, lm, near.0, near.1);
                                    }
                                    threat::rounded(bot.place, time, local.length() - (s1 - s0));
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
                        bot.set_route(Some((door_route(plain, tuning, &tactics.world.grid), at)), time);
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
                        bot.set_route(Some((door_route(partial, tuning, &tactics.world.grid), at)), time);
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
                    if route.is_none() && ends.is_some() && !escaping && !there && bot.stuck.no_way_is_proof(time) {
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
                    bot.set_route(route.map(|route| (door_route(route, tuning, &tactics.world.grid), at)), time);
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
            // The legs are final: a fight or a rush for the sprint lift (`Feel::fight_lift`).
            pose.urgent = urgent(bot, &intent, me, time);
            pose.gait = match intent.pace {
                Pace::Run => motor::Gait::Run,
                Pace::Escape(_) => motor::Gait::Escape,
                Pace::Cruise | Pace::Arrive => motor::Gait::Cruise,
                Pace::Walk | Pace::Sneak => motor::Gait::Walk,
            };
            match bot.route.as_mut() {
                Some((route, route_at)) => {
                    // `route_at` is the goal's `at`, copied unchanged where
                    // the route was planned, so the two compare exactly (a
                    // replan that snapped it to a floor would turn this off).
                    bot.motor.exact_end = (exact_at == Some(*route_at)).then(|| intent.exact_arrive_m.unwrap_or(bot.motor.feel.arrive_m));
                    let side = bot.side;
                    let objective = intent.objective;
                    // A body on a zone's floors walks its way out over them: the lines it tests may cross the refused
                    // floors of their first stretch, within the way out and no further (`Feel::zone_lines`); else no
                    // point of the route out is one it walks to, and `unclear_aim` stood it there. One test of "on
                    // refused floors" for the walk: `Hazards::allowed`, the refusal `clear_line_for` goes by.
                    let leaving = bot.motor.feel.zone_lines.then(|| tactics.zone_exit_reach(me.feet, time)).flatten();
                    match (bot.zone_walk, leaving) {
                        (None, Some(reach)) => {
                            bot.zone_walk = Some(time);
                            bot.zone_note = Some(format!(
                                "walks out over the floors of a zone from {:?}: lines tested over them up to {reach:.1} m",
                                me.feet.to_array()
                            ));
                        }
                        (Some(since), None) => {
                            bot.zone_walk = None;
                            bot.zone_note = Some(format!(
                                "is off the zone floors at {:?} after {} ms",
                                me.feet.to_array(),
                                time.wrapping_sub(since)
                            ));
                        }
                        _ => {}
                    }
                    let clear = |p: Vec3| match leaving {
                        Some(reach) => tactics.clear_line_leaving(me.feet, p, time, side, objective, reach),
                        None => tactics.clear_line_for(me.feet, p, time, side, objective),
                    };
                    // On a lip's floors the walk keeps to the route's line: a line to a point further on cuts the
                    // corner it turns at over the rim.
                    // The same floor tells the sprint through turns an edge (`Feel::sprint_turns`): there its old rule decides.
                    let grid = &tactics.world.grid;
                    let cap = bot.motor.feel.lip_look_ahead_m.get();
                    let turns = bot.motor.feel.sprint_turns.is_some();
                    let floor = (cap > 0.0 || turns).then(|| grid.locate(me.feet, 1.5)).flatten();
                    bot.motor.ahead_cap = (cap > 0.0 && floor.is_some_and(|node| grid.near_lip(node) || grid.is_lip(node))).then_some(cap);
                    if turns {
                        pose.floor = match floor {
                            Some(node) if grid.is_lip(node) || grid.near_lip(node) || grid.near_fall(node) => motor::Floor::Edge,
                            _ => motor::Floor::Open,
                        };
                    }
                    #[cfg(test)]
                    {
                        bot.bench_walk = if aim.is_some() { 2 } else { 1 };
                    }
                    let walks_the_route = aim.is_none();
                    // [spacing.lane] A teammate in file on the bot's line: the walk steers at a point beside the route's, where the ground there is the route's own floor.
                    let lane_offset = match tuning.spacing.lane {
                        Some(lane) => {
                            let band = match bot.lane {
                                apart::Lane::Centre => lane.line_m.get(),
                                apart::Lane::Over { .. } => lane.line_m.get() + 2.0 * lane.offset_m.get(),
                            };
                            bot.lane = bot.lane.next(time, apart::in_file(me, view, lane.look_m.get(), band), lane.hold_ms);
                            bot.lane.offset(lane.offset_m.get())
                        }
                        None => None,
                    };
                    let (decision, walked) = match aim {
                        Some(aim) => {
                            walking_aimed = true;
                            bot.motor.walk_aiming(route, &pose, OFF_ROUTE_M, clear, aim)
                        }
                        None => match lane_offset {
                            Some(offset_m) => {
                                let grid = &tactics.world.grid;
                                let off_edge = |node: NodeId| !(grid.near_lip(node) || grid.is_lip(node) || grid.near_fall(node));
                                let body = match grid.locate(me.feet, 1.5) {
                                    None => Err(apart::Unsafe::BodyNoFloor),
                                    Some(node) if !off_edge(node) => Err(apart::Unsafe::BodyAtEdge),
                                    Some(_) => Ok(()),
                                };
                                let safe = |shifted: Vec3, target: Vec3| match grid.locate(shifted, 0.6) {
                                    None => Err(apart::Unsafe::NoFloor),
                                    Some(node) if !off_edge(node) => Err(apart::Unsafe::AtEdge),
                                    Some(node) if (grid.position(node).y - target.y).abs() >= LANE_FLOOR_M => Err(apart::Unsafe::OtherFloor),
                                    Some(_) => Ok(()),
                                };
                                bot.motor.walk_in_lane(route, &pose, OFF_ROUTE_M, clear, Some(motor::LaneAim { offset_m, body, safe: &safe }))
                            }
                            None => bot.motor.walk(route, &pose, OFF_ROUTE_M, clear),
                        },
                    };
                    #[cfg(test)]
                    {
                        bot.bench_lane = bot.motor.lane_use;
                    }
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
            if !keeps_route {
                bot.end_route(RouteEnd::NoGoal, time);
            }
            match aim {
                Some(aim) => {
                    aiming_still = true;
                    bot.motor.aim_still(&pose, aim)
                }
                None => bot.motor.stand(&pose, &mut bot.rng),
            }
        }
    };
    #[cfg(test)]
    {
        bot.bench_motor_keys = decision.keys;
    }
    // A lift at its face asks sprint and forward held: where those keys go (`lifts::keys`).
    let lift_face = bot.motor.lift_at_face();
    let lift_wants = key::SPRINT | key::FORWARD;
    let lift_asked = decision.keys & lift_wants == lift_wants;
    // The side step, where the ground that side is clear for a stride:
    // standing, or weaving a walk to or from the target.
    #[cfg(test)]
    {
        bot.bench_stepped = 0;
    }
    let stepped = if aiming_still {
        Some(decision.keys | over.strafe)
    } else if walking_aimed && tuning.aim.combat.strafe_walking {
        weave(decision.keys, over.strafe)
    } else {
        None
    };
    // [feel.straight_jumps] On a route jump's line no side key: the jump goes the way of the keys.
    let jump_line = bot.motor.on_jump_line();
    let straight = tuning.feel.straight_jumps && jump_line;
    if jump_line && over.strafe != 0 && stepped.is_some() {
        motor::straight_jumps::add(if straight { motor::straight_jumps::Slot::WeaveHeld } else { motor::straight_jumps::Slot::WeaveAdded });
    }
    let stepped = stepped.filter(|_| !straight);
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
            over.strafe_away,
            tactics.clear_step_sees_lip,
            slides,
            cfg!(test),
            ask,
        );
        #[cfg(test)]
        {
            let clear = matches!(step, Step::Own(_));
            motor::counts::side_step(clear);
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
    // [spacing.apart] A teammate inside the body (no collision between teammates): a step away from him, over the ground the fight's own test finds clear. Not while the reflex
    // strafes or fires, nor while a use is held, nor with a side key already down.
    #[cfg(test)]
    {
        bot.bench_apart = 0;
    }
    // Whether the side step was taken this tick (the queue's abreast order waits only where the two do not part sideways).
    let mut stepped = false;
    if tuning.spacing.apart {
        let away = apart::key_away(me, view, tuning.spacing.apart_m.get());
        if away.key != 0 {
            let follows = !tuning.spacing.apart_follow
                && bot.assignment.as_ref().is_some_and(|a| matches!(a.kind, PlayKind::Escort | PlayKind::Buddy | PlayKind::PairUp | PlayKind::CoverHuman));
            let held_off = follows || intent.hold != 0 || over.strafe != 0 || over.fire || using || decision.keys & (key::LEFT | key::RIGHT) != 0;
            // [feel.straight_jumps] Not on a route jump's line.
            if jump_line && !held_off {
                motor::straight_jumps::add(if straight { motor::straight_jumps::Slot::ApartHeld } else { motor::straight_jumps::Slot::ApartAdded });
            }
            let held_off = held_off || straight;
            let keys = decision.keys | away.key;
            let to = me.feet + side_step(keys, away.key, me.yaw, tactics.clear_step_sees_lip);
            let clear = !held_off && tactics.clear_step_for(me.feet, to, time, bot.side, intent.objective);
            if clear {
                decision.keys = keys;
            }
            // The other side, when this one is refused: tried for a teammate on the line (`apart_tie`), and in the bench for the count of the refused steps whose other side was clear.
            let tie = tuning.spacing.apart_tie && away.tie;
            let other_clear = !held_off && !clear && (cfg!(test) || tie) && {
                let other = away.other_key();
                let other_to = me.feet + side_step(decision.keys | other, other, me.yaw, tactics.clear_step_sees_lip);
                tactics.clear_step_for(me.feet, other_to, time, bot.side, intent.objective)
            };
            let tied = tie && other_clear;
            if tied {
                decision.keys |= away.other_key();
            }
            stepped = clear || tied;
            let eased = !held_off && !clear && !tied && tuning.spacing.apart_ease && apart::ease(me, view, tuning.spacing.apart_m.get());
            if eased {
                decision.keys &= !(key::FORWARD | key::SPRINT);
            }
            #[cfg(test)]
            {
                // A refused step (2) whose other side is clear is 5: the bot is against a wall on the side away from him, not in a corridor; 6 is the step to the other side (`apart_tie`).
                bot.bench_apart = if held_off { 3 } else if clear { 1 } else if tied { 6 } else if eased { 4 } else if other_clear { 5 } else { 2 };
            }
        }
    }
    // [spacing.queue] A212: a teammate just ahead on the bot's way is not walked into (`apart::queue`, Reynolds' queuing): the sprint is dropped inside `gap_m`, forward too
    // inside `stop_m`. Not on a jump of the route (a sprint jump needs its run-up), not while the fight holds the keys, not in an escape. It only lets keys go: the lip guard
    // below still has the last word.
    #[cfg(test)]
    {
        bot.bench_queue = apart::QueueUse::Idle;
    }
    if let Some(queue) = tuning.spacing.queue {
        let give = apart::queue(me, view, queue.gap_m.get(), queue.stop_m.get(), queue.cone_deg.get(), stepped);
        if give != apart::Give::None {
            let used = if bot.motor.lining_up() || bot.motor.jumped().is_some() {
                apart::QueueUse::RefusedJump
            } else if intent.hold != 0 || over.strafe != 0 || over.fire || using {
                apart::QueueUse::RefusedHeld
            } else if matches!(intent.pace, Pace::Escape(_)) {
                apart::QueueUse::RefusedEscape
            } else {
                let sprinting = decision.keys & key::SPRINT != 0;
                match give {
                    apart::Give::Slow => {
                        decision.keys &= !key::SPRINT;
                        if sprinting { apart::QueueUse::SlowDropped } else { apart::QueueUse::SlowNoSprint }
                    }
                    apart::Give::Stop { abreast } => {
                        decision.keys &= !(key::FORWARD | key::SPRINT);
                        if abreast { apart::QueueUse::Yield } else { apart::QueueUse::Stop }
                    }
                    apart::Give::None => apart::QueueUse::Idle,
                }
            };
            #[cfg(test)]
            {
                bot.bench_queue = used;
            }
            #[cfg(not(test))]
            let _ = used;
        }
    }
    // [lip guard] On a lip's floors the keys of this tick are played forward to a stop: a key that carries the
    // body over the rim (the walk's own diagonal, a side step, the eight-way keys of a walk that aims) is let
    // go for one that stops on the floor, unless the route drops there. Every unplanned fall of the bench was
    // off a lip (`WalkGrid::near_lip`), with the body's keys pushing it over or its glide carrying it.
    let lift_stepped = decision.keys & lift_wants == lift_wants;
    guard_keys(bot, &mut decision, me, tactics, tuning, time);
    let lift_guarded = decision.keys & lift_wants == lift_wants;
    // The stance and pace.
    if intent.crouch || intent.pace == Pace::Sneak {
        decision.keys |= key::CROUCH;
        decision.keys &= !key::SPRINT;
    }
    bot.pace.saw(intent.pace);
    match intent.pace {
        Pace::Run | Pace::Escape(_) => bot.pace.unasked(),
        Pace::Cruise | Pace::Arrive if tuning.pace.calm_sprint => {
            let gate = pace_gate(bot, me.feet, &intent, tuning, time);
            let stripped = matches!(gate, pace::Gate::Walk(_)) && decision.keys & key::SPRINT != 0;
            if matches!(gate, pace::Gate::Walk(_)) {
                decision.keys &= !key::SPRINT;
            }
            bot.pace.caused(stripped, bot.motor.released_by_bar());
        }
        // A play that hands `Cruise` with the gate off is a play that may run.
        Pace::Cruise | Pace::Arrive => bot.pace.unasked(),
        Pace::Walk | Pace::Sneak => {
            decision.keys &= !key::SPRINT;
            bot.pace.unasked();
        }
    }
    let lift_paced = decision.keys & lift_wants == lift_wants;
    // The pull is the kick's part taken back, in the game's pitch unit: as a rise of the elevation at this pitch (`pitch.rs`).
    decision.turn.1 += Pitch::of_elevation(me.pitch, tuning.kin.pitch_scale).rise(over.pull, tuning.kin.pitch_scale);
    if over.fire && !using {
        decision.keys |= key::FIRE;
        decision.keys &= !key::SPRINT;
    } else if over.fire {
        // The shot the reflex counted this tick is not fired: given back.
        bot.reflex.hold_fire(time);
        bot.reflex.gate_using(time);
    }
    let lift_fired = decision.keys & lift_wants == lift_wants;
    decision.keys |= over.keys;
    if over.keys & (key::GRENADE | key::RELOAD | key::AIM | key::CROUCH) != 0 {
        decision.keys &= !key::SPRINT;
    }
    decision.keys |= intent.hold | std::mem::take(&mut bot.press);
    let lift_overridden = decision.keys & lift_wants == lift_wants;
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
    if lift_face {
        let stage = if walking_aimed {
            motor::lifts::Keys::AimedWalk
        } else if !lift_asked {
            motor::lifts::Keys::MotorOff
        } else if !lift_stepped {
            motor::lifts::Keys::SideStep
        } else if !lift_guarded {
            motor::lifts::Keys::LipGuard
        } else if !lift_paced {
            motor::lifts::Keys::PaceOrStance
        } else if !lift_fired {
            motor::lifts::Keys::Fire
        } else if !lift_overridden {
            motor::lifts::Keys::OverrideKeys
        } else if decision.keys & lift_wants != lift_wants {
            motor::lifts::Keys::Lining
        } else {
            motor::lifts::Keys::Held
        };
        motor::lifts::keys(stage);
    }
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
/// The sprint gate for a play that may run (`pace.rs`): the ring is looked at along the route the bot walks, or along the
/// line to the intent's goal without one.
pub(super) fn pace_gate(bot: &mut Bot, feet: Vec3, intent: &Intent, tuning: &Tuning, time: u32) -> pace::Gate {
    let look_m = tuning.pace.look_s.get() * tuning.kin.sprint;
    let (points, n) = match (bot.route.as_mut(), intent.go) {
        (Some((route, _)), _) => pace::ahead_points(route, feet, look_m),
        (None, Go::To { at, .. }) => pace::line_points(feet, at, look_m),
        (None, Go::Stay) => ([feet; pace::MAX_AHEAD], 0),
    };
    let goal = match (intent.pace, intent.go) {
        (Pace::Arrive, Go::To { at, .. }) => Some(at),
        _ => None,
    };
    bot.pace.decide(&tuning.pace, time, feet, &points[..n], goal, &bot.memory)
}

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
/// [`Feel::lip_guard_full`]: a body with no move key held is still a glide to stop when it is faster than this (m/s).
const LIP_GLIDE_MPS: f32 = 0.5;
/// [`Feel::lip_guard_full`]: a bot the guard gave up is watched again when it has moved this far from where it was given up (m).
const LIP_RESUME_M: f32 = 0.5;

#[cfg(test)]
thread_local! {
    /// The bench's reason for the lip guard's last verdict that the keys fall (`Bot::bench_guard_why`): 1 the walk from the reckoned
    /// feet falls, 2 only the same walk a reckoning's error to a side falls ([`Feel::lip_guard_full`]'s sides), plus 2 when the lip
    /// was seen only on the floors about the reckoned feet (3, 4).
    static GUARD_WHY: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// The lip guard's watch over its own hold on a bot: a guard that keeps changing a bot's keys has stopped its
/// walk, not saved it (a bot braked at a rim it must turn at, again and again). The score rises with each
/// tick the keys are changed and falls with each that they are not; past [`LIP_GIVE_UP`] the guard leaves
/// the bot's keys alone for [`LIP_OFF_MS`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum LipWatch {
    Watching(u8),
    /// Left off until this time (ms).
    Off(u32),
    /// [`Feel::lip_guard_full`]: given up at these feet with these keys held; watched again once the body has moved
    /// [`LIP_RESUME_M`] from them or holds other move keys. Asked for the glide of a stop meanwhile ([`LipMay::Glide`]).
    GaveUp { feet: Vec3, keys: u32 },
}

/// What the guard may do with a bot's keys this tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LipMay {
    /// Change them.
    Keys,
    /// Only the keys of a tick with no move key held, the glide of a stop.
    Glide,
    /// Leave them.
    No,
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
            Self::Off(_) | Self::GaveUp { .. } => Self::Watching(0).after(guard, time),
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

    /// [`Self::after`] with [`Feel::lip_guard_full`]: the bot given up is not left for a timer but for where it is and what it
    /// presses (`feet`, `keys` now), and the guard may still change a tick with no move key.
    fn after_full(self, guard: LipGuard, time: u32, feet: Vec3, keys: u32) -> (Self, LipMay) {
        const MOVE: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::SPRINT;
        let score = match self {
            Self::GaveUp { feet: at, keys: held } => {
                if feet.distance_xz(at) < LIP_RESUME_M && keys & MOVE == held & MOVE {
                    return (self, LipMay::Glide);
                }
                #[cfg(test)]
                motor::counts::lip_gap(6);
                0
            }
            Self::Off(until) if time < until => return (self, LipMay::No),
            Self::Off(_) => 0,
            Self::Watching(score) => score,
        };
        let score = match guard {
            LipGuard::Swapped(_) | LipGuard::Braked(_) => score.saturating_add(LIP_HELD_SCORE),
            LipGuard::Doomed | LipGuard::Safe | LipGuard::Clear => score.saturating_sub(1),
        };
        if score >= LIP_GIVE_UP {
            #[cfg(test)]
            motor::counts::lip_gap(5);
            (Self::GaveUp { feet, keys }, LipMay::Glide)
        } else {
            (Self::Watching(score), LipMay::Keys)
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
#[cfg(test)]
pub(super) fn lip_guard(grid: &mapdata::WalkGrid, kin: &kin::Kinematics, me: &PlaceView, turn: f32, keys: u32, planned: &[Vec3], hold: u32) -> LipGuard {
    lip_guard_with(grid, kin, me, turn, keys, planned, hold, false)
}

/// The lip guard with the three gaps closed when `full` ([`Feel::lip_guard_full`]): a body with no move key held that still
/// glides is asked too (its stop is played, the brake against the velocity the way out); the floors [`body::RECKONING_M`]
/// about the reckoned feet count for a lip's floors (the reckoned floor may be off the lip's band while the body is on it); and a
/// walk is held when the same stop from the feet [`body::RECKONING_M`] to either side, where there is a floor, would fall.
#[allow(clippy::too_many_arguments)]
pub(super) fn lip_guard_with(grid: &mapdata::WalkGrid, kin: &kin::Kinematics, me: &PlaceView, turn: f32, keys: u32, planned: &[Vec3], hold: u32, full: bool) -> LipGuard {
    const MOVE: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::SPRINT;
    let node = grid.locate(me.feet, 1.5);
    let speed = me.velocity[0].hypot(me.velocity[1]);
    // On a lip's floors, or about to be (the glide of a stop is a stride or more).
    let ahead = Vec3::new(me.feet.x + me.velocity[0] * LIP_LOOK_S, me.feet.y, me.feet.z + me.velocity[1] * LIP_LOOK_S);
    let on_lip = |node: Option<NodeId>| node.is_some_and(|node| grid.near_lip(node) || grid.is_lip(node));
    let glide = full && keys & MOVE == 0 && speed > LIP_GLIDE_MPS;
    if (keys & MOVE == 0 && !glide) || hold == 0 {
        return LipGuard::Clear;
    }
    let plain = on_lip(node) || on_lip(grid.locate(ahead, 1.0));
    // The floors about the reckoned feet: the body is up to the reckoning's error from them.
    let about = full
        && !plain
        && [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)]
            .iter()
            .any(|&(dx, dz)| on_lip(grid.locate(Vec3::new(me.feet.x + dx * body::RECKONING_M, me.feet.y, me.feet.z + dz * body::RECKONING_M), 1.0)));
    if !(plain || about) {
        return LipGuard::Clear;
    }
    #[cfg(test)]
    {
        if glide {
            motor::counts::lip_gap(0);
        } else if about {
            motor::counts::lip_gap(2);
        }
    }
    let at = body::Estimate::of_view(me, node);
    let stop = body::Stop { kin, level: 1.0, grid, planned, hold };
    // The same body a reckoning's error to either side, where the floor is (across the way it goes).
    let sides: Vec<body::Estimate> = if full && speed > 0.3 {
        let (ux, uz) = (me.velocity[0] / speed, me.velocity[1] / speed);
        [(-uz, ux), (uz, -ux)]
            .iter()
            .filter_map(|&(px, pz)| {
                let feet = Vec3::new(me.feet.x + px * body::RECKONING_M, me.feet.y, me.feet.z + pz * body::RECKONING_M);
                let node = grid.locate(feet, 0.5)?;
                Some(body::Estimate { feet, node: Some(node), ..at })
            })
            .collect()
    } else {
        Vec::new()
    };
    let up = |keys: u32| body::stays_up(&at, keys, turn, &stop) && sides.iter().all(|side| body::stays_up(side, keys, turn, &stop));
    let guard = {
        if up(keys) {
            LipGuard::Safe
        } else {
            #[cfg(test)]
            let sides_only = full && body::stays_up(&at, keys, turn, &stop);
            #[cfg(test)]
            if sides_only {
                motor::counts::lip_gap(4);
            }
            #[cfg(test)]
            GUARD_WHY.set(1 + u8::from(sides_only) + 2 * u8::from(about));
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
    };
    #[cfg(test)]
    if matches!(guard, LipGuard::Swapped(_) | LipGuard::Braked(_)) {
        if glide {
            motor::counts::lip_gap(1);
        } else if about {
            motor::counts::lip_gap(3);
        }
    }
    guard
}

/// [`lip_guard_with`] on the tick's decision.
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
    let full = bot.motor.feel.lip_guard_full;
    let guard = if matches!(bot.lip_watch, LipWatch::Off(until) if time < until) {
        LipGuard::Clear
    } else {
        lip_guard_with(&tactics.world.grid, &tuning.kin, me, decision.turn.0, decision.keys, &planned, bot.motor.feel.lip_guard_ticks, full)
    };
    let guard = if full {
        const MOVE: u32 = key::FORWARD | key::BACK | key::LEFT | key::RIGHT | key::SPRINT;
        let (watch, may) = bot.lip_watch.after_full(guard, time, me.feet, decision.keys);
        bot.lip_watch = watch;
        match may {
            LipMay::Keys => guard,
            // Given up: only the glide of a tick with no move key is still asked.
            LipMay::Glide if decision.keys & MOVE == 0 && matches!(guard, LipGuard::Swapped(_) | LipGuard::Braked(_)) => {
                #[cfg(test)]
                motor::counts::lip_gap(7);
                guard
            }
            LipMay::Glide | LipMay::No => LipGuard::Clear,
        }
    } else {
        let (watch, may) = bot.lip_watch.after(guard, time);
        bot.lip_watch = watch;
        if may { guard } else { LipGuard::Clear }
    };
    #[cfg(test)]
    {
        bot.bench_guard_at = (me.velocity, me.feet);
        bot.bench_guard = match guard {
            LipGuard::Clear => 0,
            LipGuard::Safe => 1,
            LipGuard::Swapped(_) => 2,
            LipGuard::Braked(_) | LipGuard::Doomed => 3,
        };
        bot.bench_guard_why = if matches!(guard, LipGuard::Swapped(_) | LipGuard::Braked(_) | LipGuard::Doomed) { GUARD_WHY.get() } else { 0 };
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

    /// Whether a body on a floor a zone hurts on now (`hurt`) escapes (`HazardTuning::run_out`, `Pace::Escape(EscapeWhy::Zone)`): it was to walk or run (a run is no
    /// sprint on a short route), is not crouched, not sneaking and not escaping already, and the reflex's legs (a fight) are not on it.
    pub(super) fn runs_out(pace: Pace, crouch: bool, hurt: bool, legs_free: bool) -> bool {
        hurt && legs_free && !crouch && matches!(pace, Pace::Walk | Pace::Cruise | Pace::Arrive | Pace::Run)
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

    /// Whether the route stays while the legs stand with no goal: standing to throw a smoke, with
    /// `smoke.throw_keeps_route` (the walk goes on along the route its crossing was planned for), and holding for a
    /// cloud short of the crossing (`smoke.wait_for_cloud`: the walk goes on through it).
    pub(super) fn keeps_route(legs: Option<Legs>, switch: bool) -> bool {
        legs.is_some_and(|legs| match legs.why {
            LegsWhy::ThrowingSmoke => switch,
            LegsWhy::WaitingForCloud => true,
            _ => false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`Pace::Escape`]: the sights, crouch and side steps of the reflex's wish are let go by every reason, the trigger by a blast's only; a
    /// grenade key stays.
    #[test]
    fn an_escape_lets_go_of_the_sights_the_crouch_and_the_side_steps_and_by_its_reason_the_trigger() {
        let wish = reflex::Override {
            keys: key::AIM | key::CROUCH | key::GRENADE,
            strafe: key::LEFT,
            strafe_away: true,
            fire: true,
            ..reflex::Override::default()
        };
        let blast = escape_legs(EscapeWhy::Grenade).of(wish);
        assert_eq!(blast.keys, key::GRENADE);
        assert_eq!((blast.strafe, blast.strafe_away, blast.fire), (0, false, false));
        let zone = escape_legs(EscapeWhy::Zone).of(wish);
        assert_eq!(zone.keys, key::GRENADE);
        assert_eq!((zone.strafe, zone.strafe_away, zone.fire), (0, false, true));
    }

    fn intent(fire: bool, let_go: bool, zone_ok: bool) -> Intent {
        Intent { fire, let_go, zone_ok, ..Intent::stay() }
    }

    /// The side step is tested over the move the keys make: a weave of a walk
    /// forward is the diagonal, not the pure side step; with the switch off,
    /// or keys that cancel, the pure side step of the `strafe` side.
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

    /// Standing to throw a smoke keeps the route with the switch on; any other stand, and the switch off, drop it.
    #[test]
    fn standing_to_throw_a_smoke_keeps_the_route_with_the_switch() {
        let legs = |why| Some(Legs { at: Vec3::ZERO, within_m: 1.0, run: false, crouch: false, why });
        assert!(Gates::keeps_route(legs(LegsWhy::ThrowingSmoke), true));
        assert!(!Gates::keeps_route(legs(LegsWhy::ThrowingSmoke), false));
        assert!(Gates::keeps_route(legs(LegsWhy::WaitingForCloud), false), "a hold for a cloud walks on through the crossing");
        assert!(!Gates::keeps_route(legs(LegsWhy::Throwing), true), "a frag throw drops it as before");
        assert!(!Gates::keeps_route(legs(LegsWhy::Range), true));
        assert!(!Gates::keeps_route(None, true));
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

    /// [`Feel::lip_guard_full`], the key-less tick: a body at a rim that has let go of its keys glides on over it, and only the
    /// full guard asks (and brakes it); it leaves a body that is slow, or far from the rim, alone.
    #[test]
    fn a_glide_over_a_rim_with_no_key_held_is_braked_by_the_full_guard() {
        let world = slab();
        let grid = &world.grid;
        let kin = kin::Kinematics::default();
        let toward = -std::f32::consts::FRAC_PI_2;
        let at_rim = on_the_slab(55.4, toward, 3.5);
        assert_eq!(lip_guard(grid, &kin, &at_rim, 0.0, 0, &[], HOLD), LipGuard::Clear, "as it was: nothing to let go of");
        assert!(matches!(lip_guard_with(grid, &kin, &at_rim, 0.0, 0, &[], HOLD, true), LipGuard::Braked(_) | LipGuard::Swapped(_)));
        assert_eq!(lip_guard_with(grid, &kin, &on_the_slab(55.4, toward, 0.2), 0.0, 0, &[], HOLD, true), LipGuard::Clear, "no glide");
        assert_eq!(lip_guard_with(grid, &kin, &on_the_slab(52.0, toward, 3.5), 0.0, 0, &[], HOLD, true), LipGuard::Clear, "far from the rim");
        // With the keys held it is the same as ever.
        let walk = |full: bool| lip_guard_with(grid, &kin, &at_rim, 0.0, key::FORWARD, &[], HOLD, full);
        assert_eq!(walk(true), walk(false));
    }

    /// [`Feel::lip_guard_full`], the floors about the reckoned feet: at the edge of the lip's band the full guard looks where the
    /// old one does not, and never the other way about.
    #[test]
    fn the_full_guard_looks_at_the_floors_about_the_reckoned_feet() {
        let world = slab();
        let grid = &world.grid;
        let kin = kin::Kinematics::default();
        let toward = -std::f32::consts::FRAC_PI_2;
        let (mut wider, mut narrower) = (0, 0);
        for k in 0..60 {
            let me = on_the_slab(50.0 + k as f32 * 0.1, toward, 0.1);
            let old = lip_guard(grid, &kin, &me, 0.0, key::FORWARD, &[], HOLD) != LipGuard::Clear;
            let new = lip_guard_with(grid, &kin, &me, 0.0, key::FORWARD, &[], HOLD, true) != LipGuard::Clear;
            wider += usize::from(new && !old);
            narrower += usize::from(old && !new);
        }
        assert_eq!((wider > 0, narrower), (true, 0), "wider {wider} narrower {narrower}");
    }

    /// [`kin::MarginEdge`]: a walk along the rim on the slab's last floor (its centre 0.125 m inside the rim) stays up, the body held
    /// by the client's edge. With the margin at the lattice's edge the full guard's sides hold it (the side 0.3 m out is past the
    /// cell's boundary, where its margin falls at once), and the plain guard holds a body reckoned past the boundary for its own
    /// margin. With the margin at the body's edge both walk on (the full guard still holds a body whose side is past the support),
    /// and a walk toward the rim is stopped as before.
    #[test]
    fn a_walk_along_a_rim_is_held_only_with_the_margin_at_the_lattices_edge() {
        let world = slab();
        let grid = &world.grid;
        let lattice = kin::Kinematics::default();
        let body = kin::Kinematics { margin_edge: kin::MarginEdge::Body, ..lattice.clone() };
        assert_eq!((lattice.edge, lattice.margin_edge), (kin::Edge::Client, kin::MarginEdge::Lattice), "the defaults");
        let held = |guard: LipGuard| matches!(guard, LipGuard::Swapped(_) | LipGuard::Braked(_) | LipGuard::Doomed);
        let walk = |kin: &kin::Kinematics, x: f32, yaw: f32, full: bool| lip_guard_with(grid, kin, &on_the_slab(x, yaw, 3.5), 0.0, key::FORWARD, &[], HOLD, full);
        // Along the rim on its last floor: the sides.
        assert!(held(walk(&lattice, 55.875, 0.0, true)), "held for a side");
        assert_eq!(walk(&lattice, 55.875, 0.0, false), LipGuard::Safe, "the plain guard has no sides");
        assert_eq!(walk(&body, 55.875, 0.0, true), LipGuard::Safe);
        // Reckoned 0.1 m past the boundary (within the support): its own margin.
        assert!(held(walk(&lattice, 56.1, 0.0, false)), "held for its own margin");
        assert_eq!(walk(&body, 56.1, 0.0, false), LipGuard::Safe);
        // There a reckoning's error further out is past the support (0.225 + 0.3 > `SUPPORT_M`): the full guard holds it still.
        assert!(held(walk(&body, 56.1, 0.0, true)));
        // Toward the rim: stopped either way.
        let toward = -std::f32::consts::FRAC_PI_2;
        for full in [false, true] {
            assert!(held(walk(&body, 55.4, toward, full)), "{full}");
            assert!(held(walk(&lattice, 55.4, toward, full)), "{full}");
        }
    }

    /// [`Feel::lip_guard_full`], the give-up is a state with what ends it: kept while the body stays near where it was given up and
    /// holds the same move keys (asked for the glide only), watched again once it has moved or presses other keys.
    #[test]
    fn a_bot_given_up_is_watched_again_when_it_moves_or_changes_its_keys() {
        let (home, keys) = (Vec3::new(10.0, 2.0, 10.0), key::FORWARD);
        let mut watch = LipWatch::default();
        let mut may = LipMay::Keys;
        for k in 0..u32::from(LIP_GIVE_UP / LIP_HELD_SCORE + 1) {
            (watch, may) = watch.after_full(LipGuard::Swapped(0), k * 50, home, keys);
        }
        assert_eq!((watch, may), (LipWatch::GaveUp { feet: home, keys }, LipMay::Glide));
        // Minutes later, still there and still pressing it: still given up (no timer).
        let (kept, may) = watch.after_full(LipGuard::Safe, 600_000, home + Vec3::new(0.2, 0.0, 0.0), keys | key::CROUCH);
        assert_eq!((kept, may), (watch, LipMay::Glide));
        // Moved 0.5 m, or other move keys: watched again, the guard may change keys.
        let (moved, may) = watch.after_full(LipGuard::Safe, 1_000, home + Vec3::new(LIP_RESUME_M, 0.0, 0.0), keys);
        assert_eq!((moved, may), (LipWatch::Watching(0), LipMay::Keys));
        let (turned, may) = watch.after_full(LipGuard::Safe, 1_000, home, key::BACK);
        assert_eq!((turned, may), (LipWatch::Watching(0), LipMay::Keys));
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
    fn a_body_in_a_hurting_zone_runs_unless_it_is_crouched_runs_already_or_the_reflex_has_its_legs() {
        assert!(Gates::runs_out(Pace::Walk, false, true, true));
        assert!(Gates::runs_out(Pace::Cruise, false, true, true));
        assert!(Gates::runs_out(Pace::Arrive, false, true, true));
        assert!(Gates::runs_out(Pace::Run, false, true, true));
        assert!(!Gates::runs_out(Pace::Escape(EscapeWhy::Grenade), false, true, true));
        assert!(!Gates::runs_out(Pace::Walk, false, false, true));
        assert!(!Gates::runs_out(Pace::Walk, true, true, true));
        assert!(!Gates::runs_out(Pace::Walk, false, true, false));
        assert!(!Gates::runs_out(Pace::Sneak, false, true, true));
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

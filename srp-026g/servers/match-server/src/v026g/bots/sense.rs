//! What a bot senses at a think (`player_bots_behaviour.md` §2): the only
//! way enemy facts come to it besides its team's callouts and the cheats.
//!
//! - **Sight:** an enemy within `sight_m`, inside the view's width, with a
//!   clear line from the bot's eyes to some part of his body on the bullet
//!   mesh (`aimpoint`: chest, head, belly, thighs, the chest's edges,
//!   shins; the first that shows ends the test). The rays come from a
//!   budget shared by all bots of the match, and the enemies longest
//!   unchecked go first (research B5).
//! - **Hearing:** the sounds of the last think (shots, sprinting, a reload
//!   or switch, a use) within their ranges (× the hearing cheat), placed
//!   with an error that grows with the distance.
//!
//! The noticing delay is not here: it is the memory's (`know::Memory`), so
//! that no layer can skip it.

use mapdata::{RayTree, Vec3};

use super::kin::{forward, wrap};
use super::aimpoint::{Part, PartsLook};
use super::know::{KnownBattery, LookOutcome, Miss, Percept, Source, TeamKnowledge};
use super::smoke::Veil;
use super::rng::Rng;
use super::tuning::{CheatTuning, HearingRanges, SenseTuning, ShotRanges};
use super::view::{MatchView, Place, PlaceView, SoundKind};

/// A step's and a sprint's cull in the data, metres (`steps.spl`, where its
/// gain reaches 0: PROVEN, the file).
pub const STEP_CULL_M: f32 = 17.0;
/// A reload's cull, metres (`weapons_reloading.spl`: PROVEN, the file).
pub const RELOAD_CULL_M: f32 = 9.0;

#[cfg(test)]
thread_local! {
    static SHOTS_HEARD: std::cell::Cell<[u64; 3]> = const { std::cell::Cell::new([0; 3]) };
}

/// A bot's hearing of a shot, for the tests' counts: by the weapon's reach
/// from the table, at the fallback range of an unknown weapon, and from
/// farther than the flat range (`SenseTuning::shot_m`).
#[cfg(test)]
pub fn shot_heard(table: bool, unknown: bool, far: bool) {
    SHOTS_HEARD.with(|c| {
        let mut n = c.get();
        n[0] += u64::from(table);
        n[1] += u64::from(unknown);
        n[2] += u64::from(far);
        c.set(n);
    });
}

/// The shots heard so far: by the table, at the fallback, from over the flat range.
#[cfg(test)]
pub fn shots_heard() -> [u64; 3] {
    SHOTS_HEARD.with(|c| c.get())
}

/// How far from `from` a sound at `at` is, by the ranges in force.
fn hearing_distance(ranges: HearingRanges, from: Vec3, at: Vec3) -> f32 {
    match ranges {
        HearingRanges::Flat => from.distance_xz(at),
        HearingRanges::Game { .. } => (from - at).length(),
    }
}

/// The range a sound of `kind` carries: `base` times the hearing cheat,
/// and under the game's ranges never past the sound's cull.
fn sound_range(ranges: HearingRanges, kind: SoundKind, base: f32, hearing: f32) -> f32 {
    let range = hearing * base;
    match (ranges, kind) {
        (HearingRanges::Flat, _) => range,
        (HearingRanges::Game { .. }, SoundKind::Sprint | SoundKind::Jump | SoundKind::Trap) => range.min(STEP_CULL_M),
        (HearingRanges::Game { .. }, SoundKind::Reload) => range.min(RELOAD_CULL_M),
        (HearingRanges::Game { .. }, _) => range,
    }
}

/// Places whose starved rays are counted one by one; later ones in the last.
pub const STARVED_PLACES: usize = 32;

/// The rays a poll may still cast at the batteries, and those refused it.
/// (An enemy in view is tested outside it, [`sense_looks`].)
#[derive(Clone, Copy, Debug)]
pub struct RayBudget {
    pub left: u32,
    /// The rays cast outside the budget this poll, at enemies in view: the
    /// tally of them.
    pub exempt: u32,
    /// The thinks due in this poll that have not run, the one in turn
    /// included: counted before any ray is cast ([`fair_share`]); 0: not
    /// counted, no limit.
    pub thinks_left: u32,
    /// What the think in turn may still take (`allow`); none: all that is
    /// left.
    allowance: Option<u32>,
    /// Rays asked after the budget ran out or the allowance was spent, by
    /// team and by place (`player_bots_research.md` B5: a starved look is
    /// counted, not recorded as "unseen").
    pub starved_by_team: [u32; 2],
    pub starved_by_place: [u32; STARVED_PLACES],
}

impl RayBudget {
    pub fn new(left: u32) -> Self {
        Self {
            left,
            exempt: 0,
            thinks_left: 0,
            allowance: None,
            starved_by_team: [0; 2],
            starved_by_place: [0; STARVED_PLACES],
        }
    }

    /// One ray for the bot `place` of `team`; refused, it is counted.
    pub fn take(&mut self, place: Place, team: u8) -> bool {
        if self.left == 0 || self.allowance == Some(0) {
            self.starved_by_team[usize::from(team & 1)] += 1;
            self.starved_by_place[place.min(STARVED_PLACES - 1)] += 1;
            return false;
        }
        self.left -= 1;
        if let Some(allowed) = &mut self.allowance {
            *allowed -= 1;
        }
        true
    }

    /// The rays refused so far.
    pub fn starved(&self) -> u32 {
        self.starved_by_team.iter().sum()
    }

    /// The think in turn may take at most `rays` more ([`fair_share`]);
    /// none: no limit but what is left.
    pub fn allow(&mut self, rays: Option<u32>) {
        self.allowance = rays;
    }
}

/// The most rays one look at a live sight casts: one for each part of the
/// sight's order.
pub const LOOK_RAYS: u32 = Part::SIGHT_ORDER.len() as u32;

/// The least rays a think's battery looks keep when the poll's budget binds
/// (one look at the nearest battery, its three heights).
pub const BATTERY_FLOOR_RAYS: u32 = BATTERY_HEIGHTS.len() as u32;

/// The most rays one think takes from the poll's budget: `battery_rays`
/// looks at a battery's heights. (The enemies in view are tested outside it.)
pub fn think_most(battery_rays: u32) -> u32 {
    battery_rays * BATTERY_HEIGHTS.len() as u32
}

/// What one think may take from a binding poll's budget: the budget's
/// `left` rays shared by `thinks_left` thinks (the one in turn and those
/// after it, counted before any ray is cast), each of which takes `most` at
/// the most, but never under the floor ([`BATTERY_FLOOR_RAYS`]). Fair across
/// the bot index: without it the first thinks take what they want and the
/// last get none, every poll. `None` while the budget cannot bind, so a poll
/// that does not bind is decided as it was.
pub fn fair_share(left: u32, thinks_left: u32, most: u32) -> Option<u32> {
    let thinks = thinks_left.max(1);
    if u64::from(left) >= u64::from(thinks) * u64::from(most) {
        return None;
    }
    Some((left / thinks).max(BATTERY_FLOOR_RAYS.min(left)))
}

/// Why `enemy`, seen at `since` (or never) by `me`, cannot be seen now,
/// before any ray: dead, gone, or another life than the one seen
/// (`PlaceView::spawned`); beyond `sight_m`; outside the view's width at
/// the yaw `me` has now. An enemy within `touch_m` is in view whatever the
/// yaw.
pub fn gate(
    me: &PlaceView,
    enemy: &PlaceView,
    since: u32,
    tuning: &SenseTuning,
) -> Result<(), Miss> {
    if !enemy.alive || !enemy.present || enemy.spawned > since {
        return Err(Miss::Dead);
    }
    reach(me, enemy, tuning)
}

/// [`gate`]'s range and view.
fn reach(me: &PlaceView, enemy: &PlaceView, tuning: &SenseTuning) -> Result<(), Miss> {
    let distance = enemy.feet.distance_xz(me.feet);
    if distance > tuning.sight_m {
        return Err(Miss::Range);
    }
    let near = distance <= tuning.touch_m;
    if !(near || in_view(me.feet, me.yaw, enemy.feet, tuning.view_deg)) {
        return Err(Miss::Cone);
    }
    Ok(())
}

/// One look at `enemy`, whose sight began at `since`: [`gate`], then rays
/// from `rays` to the parts in [`Part::SIGHT_ORDER`] (as [`sense`] tests a
/// new enemy), the first that shows ends it. Pure: no random draw, no
/// memory. The ray budget running out before any part showed is
/// [`LookOutcome::Untested`], not a negative. A think's tests of the
/// enemies in view give it their own [`RayBudget::new`]`(`[`LOOK_RAYS`]`)`
/// ([`sense_looks`]), so they are never starved.
pub fn look(
    me: &PlaceView,
    enemy: &PlaceView,
    since: u32,
    sight: &RayTree,
    tuning: &SenseTuning,
    rays: &mut RayBudget,
) -> LookOutcome {
    if let Err(miss) = gate(me, enemy, since, tuning) {
        return LookOutcome::Failed(miss);
    }
    ray_test(me, enemy, sight, Veil::Clear, rays)
}

/// [`look`]'s rays, through `veil` (`smoke.rs`): a part the smoke hides is not looked at, and a look that found
/// none showing with one hidden so is lost to smoke ([`Miss::Smoke`]).
fn ray_test(
    me: &PlaceView,
    enemy: &PlaceView,
    sight: &RayTree,
    veil: Veil<'_>,
    rays: &mut RayBudget,
) -> LookOutcome {
    let mut starved = false;
    let looked = super::aimpoint::look_parts(me.eye(), enemy, sight, veil, || {
        let given = rays.take(me.place, me.team);
        starved |= !given;
        given
    });
    match (looked, starved) {
        (PartsLook::Shows, _) => LookOutcome::Seen,
        (_, true) => LookOutcome::Untested,
        (PartsLook::Smoke, false) => LookOutcome::Failed(Miss::Smoke),
        (PartsLook::Hidden, false) => LookOutcome::Failed(Miss::Blocked),
    }
}

/// Whether `to` lies within the view of a body facing `yaw` (`view_deg`
/// wide).
pub fn in_view(from: Vec3, yaw: f32, to: Vec3, view_deg: f32) -> bool {
    let (dx, dz) = (to.x - from.x, to.z - from.z);
    if dx.abs() + dz.abs() < 1e-3 {
        return true;
    }
    let facing = forward(yaw);
    let angle = wrap(super::kin::yaw_of([dx, dz]) - super::kin::yaw_of(facing)).abs();
    angle <= view_deg.to_radians() * 0.5
}

/// The enemies to look at in turn: those whose sight is live first (a
/// re-test of what is seen comes before a search for a new enemy), and each
/// group the longest unchecked first (`tested`: when a ray test of him last
/// came to an answer; 0 for one never tested), ties in the order given.
pub fn look_order(
    enemies: &[Place],
    live: impl Fn(Place) -> bool,
    tested: impl Fn(Place) -> u32,
) -> Vec<Place> {
    let mut order = enemies.to_vec();
    order.sort_by_key(|&who| (!live(who), tested(who)));
    order
}

/// The percepts of the bot `me` at `now`, the sounds since `since`.
/// `order` gives the enemies to look at first ([`look_order`]).
#[allow(clippy::too_many_arguments)]
pub fn sense(
    me: &PlaceView,
    view: &MatchView,
    sight: &RayTree,
    tuning: &SenseTuning,
    cheats: &CheatTuning,
    since: u32,
    order: &[Place],
    rays: &mut RayBudget,
    rng: &mut Rng,
) -> Vec<Percept> {
    sense_looks(me, view, sight, tuning, cheats, since, order, rays, rng).0
}

/// [`sense`], and with the percepts what each enemy looked at came to: seen,
/// or failed (past the sight's range, outside the view, dead, or no part
/// showing). An enemy of the bot's own team is not looked at. The rays are
/// counted in `rays.exempt`, not taken from it.
#[allow(clippy::too_many_arguments)]
pub fn sense_looks(
    me: &PlaceView,
    view: &MatchView,
    sight: &RayTree,
    tuning: &SenseTuning,
    cheats: &CheatTuning,
    since: u32,
    order: &[Place],
    rays: &mut RayBudget,
    rng: &mut Rng,
) -> (Vec<Percept>, Vec<(Place, LookOutcome)>) {
    sense_looks_through(me, view, sight, Veil::Clear, tuning, cheats, since, order, rays, rng)
}

/// [`sense_looks`] through `veil`: an enemy whose parts are behind smoke is not seen (`smoke.rs`), and a look
/// that found him so is [`Miss::Smoke`]. With [`Veil::Clear`] it is [`sense_looks`], ray for ray.
#[allow(clippy::too_many_arguments)]
pub fn sense_looks_through(
    me: &PlaceView,
    view: &MatchView,
    sight: &RayTree,
    veil: Veil<'_>,
    tuning: &SenseTuning,
    cheats: &CheatTuning,
    since: u32,
    order: &[Place],
    rays: &mut RayBudget,
    rng: &mut Rng,
) -> (Vec<Percept>, Vec<(Place, LookOutcome)>) {
    let mut out = Vec::new();
    let mut looks = Vec::new();
    if !me.alive {
        return (out, looks);
    }
    for &who in order {
        let Some(enemy) = view.place(who) else {
            continue;
        };
        if enemy.team == me.team {
            continue;
        }
        if !tuning.sight_refresh {
            // As before the refresh: an enemy in the view and in range is tested with the rays of the
            // poll's budget, and a refusal is a negative; no look is told.
            if !enemy.alive || !enemy.present || reach(me, enemy, tuning).is_err() {
                continue;
            }
            let eye = me.eye();
            let looked = super::aimpoint::look_parts(eye, enemy, sight, veil, || rays.take(me.place, me.team));
            if looked == PartsLook::Shows {
                out.push(seen_percept(enemy, view));
            }
            continue;
        }
        if !enemy.alive || !enemy.present {
            looks.push((who, LookOutcome::Failed(Miss::Dead)));
            continue;
        }
        if let Err(miss) = reach(me, enemy, tuning) {
            looks.push((who, LookOutcome::Failed(miss)));
            continue;
        }
        // [aimpoint] Seen when some part of him shows, not only the eyes
        // or the chest: legs on a stair, a head over a wall. An enemy in
        // the view and in range is tested whole, whatever the poll's budget
        // has left (at most [`LOOK_RAYS`], one test a think): a bot that
        // cannot see what is in front of it, because the others took the
        // rays, is no fair bot.
        let mut own = RayBudget::new(LOOK_RAYS);
        let outcome = ray_test(me, enemy, sight, veil, &mut own);
        rays.exempt += LOOK_RAYS - own.left;
        looks.push((who, outcome));
        if outcome == LookOutcome::Seen {
            out.push(seen_percept(enemy, view));
        }
    }
    let hearing = cheats.hearing.max(1.0);
    for sound in view.sounds.iter().filter(|sound| sound.time > since) {
        let Some(enemy) = view.place(sound.from) else {
            continue;
        };
        if enemy.team == me.team {
            continue;
        }
        let base = match sound.kind {
            SoundKind::Shot => tuning.shot_m,
            SoundKind::Sprint | SoundKind::Jump => tuning.sprint_m,
            SoundKind::Reload | SoundKind::Switch => tuning.reload_m,
            SoundKind::Use => tuning.use_m,
            SoundKind::Trap => tuning.step_m,
        };
        // A shot by its weapon's reach (never farther: the cheat adds none
        // past the game's), else by the flat range.
        let range = match (sound.kind, tuning.shots) {
            (SoundKind::Shot, ShotRanges::ByWeapon { unknown_m }) => sound.reach_m.unwrap_or(unknown_m),
            _ => sound_range(tuning.ranges, sound.kind, base, hearing),
        };
        let distance = hearing_distance(tuning.ranges, me.feet, sound.at);
        if distance > range
            || out
                .iter()
                .any(|p| p.who == sound.from && p.source == Source::Seen)
        {
            continue;
        }
        #[cfg(test)]
        if sound.kind == SoundKind::Shot {
            let by_weapon = matches!(tuning.shots, ShotRanges::ByWeapon { .. });
            shot_heard(
                by_weapon && sound.reach_m.is_some(),
                by_weapon && sound.reach_m.is_none(),
                distance > tuning.shot_m,
            );
        }
        let error = distance * tuning.heard_error;
        let (dx, dz) = (rng.range(-error, error), rng.range(-error, error));
        out.push(Percept {
            who: sound.from,
            at: Vec3::new(sound.at.x + dx, sound.at.y, sound.at.z + dz),
            error_m: error,
            time: sound.time,
            source: Source::Heard,
            busy: sound.kind == SoundKind::Shot,
        });
    }
    // Footsteps (`SenseTuning::step_m`): an enemy walking upright near by,
    // not seen and not heard otherwise at this think.
    if tuning.step_m > 0.0 {
        for enemy in &view.places {
            let distance = hearing_distance(tuning.ranges, me.feet, enemy.feet);
            // A crouched mover is heard only under the game's ranges, within
            // the crouched step's; one upright within `step_m`.
            let crouched = enemy.keys & crate::v026g::drive::key::CROUCH != 0;
            let step_m = match (tuning.ranges, crouched) {
                (_, false) => Some(tuning.step_m),
                (HearingRanges::Flat, true) => None,
                (HearingRanges::Game { crouch_step_m }, true) => Some(crouch_step_m),
            };
            let Some(step_m) = step_m else {
                continue;
            };
            if !enemy.alive
                || !enemy.present
                || enemy.team == me.team
                || distance > sound_range(tuning.ranges, SoundKind::Sprint, step_m, hearing)
                || enemy.speed() <= tuning.step_speed
                || out.iter().any(|p| p.who == enemy.place)
            {
                continue;
            }
            let error = distance * tuning.heard_error;
            let (dx, dz) = (rng.range(-error, error), rng.range(-error, error));
            out.push(Percept {
                who: enemy.place,
                at: Vec3::new(enemy.feet.x + dx, enemy.feet.y, enemy.feet.z + dz),
                error_m: error,
                time: view.clock,
                source: Source::Heard,
                busy: false,
            });
        }
    }
    (out, looks)
}

/// The percept of an enemy found by a ray test at `view`'s clock.
fn seen_percept(enemy: &PlaceView, view: &MatchView) -> Percept {
    Percept {
        who: enemy.place,
        at: enemy.feet,
        error_m: 0.0,
        time: view.clock,
        source: Source::Seen,
        busy: enemy.speed() > 1.0 || enemy.keys & crate::v026g::drive::key::FIRE != 0,
    }
}

/// Where a lying battery is looked at, metres over its position: the top
/// (as before), the middle and near the foot. The client's battery
/// (`gameplay/victory_items/default.options`, model `item/quest/electrobox`)
/// collides as a box of half sizes 0.3 x 0.12 x 0.16 m set 0.12 m up
/// (`!victory_item_collision!`, read from resources.db): 0.24 m high.
pub const BATTERY_HEIGHTS: [f32; 3] = [0.25, 0.12, 0.04];


/// What a bot may look at for its team's battery knowledge.
#[derive(Clone, Copy, Debug, PartialEq)]
enum BatteryLook {
    /// A battery lying where the team does not know it.
    Battery(usize, Vec3),
    /// A spot where the team believes a battery lies and it does not.
    Gone(usize, Vec3),
    /// A battery spawner's spot worth a look.
    Spawner(usize, Vec3),
}

/// With the battery cheat off: what the bot `me` sees of the batteries for
/// its team (`player_bots_behaviour.md` §2: "a battery a bot sees becomes
/// known to its team"). A battery lying in the view, within
/// `battery_sight_m`, in a clear line on the bullet mesh (or within
/// `battery_near_m`) becomes known where it lies, and is put right when it
/// lies over `battery_moved_m` from the spot known: a take aims at that
/// spot, and the use's ray must meet the box (half sizes 0.3 x 0.12 x
/// 0.16 m). With 1.5 m there, a battery an enemy took and dropped dead
/// 0.7 m on, unseen, stayed known where it had been, and team 0's bots
/// aimed at the empty spot from a metre for two minutes (the owner's match
/// 0x4da3b51d, battery 3, 135-262 s: 7 "took no battery" stands, their
/// views 0.5-10 degrees off the old spot in the worlds, 23-55 off the
/// battery; team 1's bot, on its HUD's spot, took it at its first use).
/// A spot believed to hold one, so seen empty, is forgotten; a spawner's
/// spot so seen is checked.
/// Nearest first, at most `battery_rays` rays from the poll's budget.
/// `worth(spawner)` says which spawners are worth a look.
#[allow(clippy::too_many_arguments)]
pub fn sense_batteries(
    me: &PlaceView,
    view: &MatchView,
    sight: &RayTree,
    spawners: &[mapdata::BatterySpawner],
    tuning: &SenseTuning,
    team: &mut TeamKnowledge,
    worth: impl Fn(usize) -> bool,
    rays: &mut RayBudget,
) {
    use super::view::BatteryState;
    if !me.alive {
        return;
    }
    let eye = me.eye();
    let near = |at: Vec3| {
        at.distance_xz(me.feet) <= tuning.battery_near_m && (at.y - me.feet.y).abs() < 2.0
    };
    let lying_at = |battery: usize, at: Vec3| matches!(view.batteries.get(battery), Some(BatteryState::Free(real)) if real.distance_xz(at) <= 1.5);
    let mut looks: Vec<(f32, BatteryLook)> = Vec::new();
    let mut consider = |look: BatteryLook, at: Vec3| {
        let distance = (at - eye).length();
        if near(at)
            || (distance <= tuning.battery_sight_m && in_view(me.feet, me.yaw, at, tuning.view_deg))
        {
            looks.push((distance, look));
        }
    };
    for (battery, state) in view.batteries.iter().enumerate() {
        if let BatteryState::Free(at) = *state
            && team
                .battery(battery)
                .is_none_or(|k| (k.at - at).length() > tuning.battery_moved_m)
        {
            consider(BatteryLook::Battery(battery, at), at);
        }
    }
    for (battery, known) in team.batteries.iter().enumerate() {
        if let Some(known) = known
            && !known.hud
            && !lying_at(battery, known.at)
        {
            consider(BatteryLook::Gone(battery, known.at), known.at);
        }
    }
    for (index, spawner) in spawners.iter().enumerate() {
        if worth(index) {
            consider(BatteryLook::Spawner(index, spawner.item), spawner.item);
        }
    }
    // A spot where an enemy carrier died is looked at once a bot of the
    // team stands by it: what lies there it sees within `battery_near_m`.
    team.looks.retain(|look| !near(look.at));
    if looks.is_empty() {
        return;
    }
    looks.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut cast = 0;
    // `cast` counts only the looks tried by ray, not every look.
    #[allow(clippy::explicit_counter_loop)]
    'looks: for (_, look) in looks {
        let at = match look {
            BatteryLook::Battery(_, at)
            | BatteryLook::Gone(_, at)
            | BatteryLook::Spawner(_, at) => at,
        };
        // A battery, and a spot believed to hold one, are tried at the
        // box's top, middle and foot (0.24 m high, `BATTERY_HEIGHTS`): a
        // ledge or a nook may hide one and show another, and a take's
        // stand has its line to the middle (a spot on a crate seen empty
        // from below is forgotten, not tried at every stand). A spawner's
        // spot only at the top, as before.
        let heights: &[f32] = match look {
            BatteryLook::Battery(..) | BatteryLook::Gone(..) => &BATTERY_HEIGHTS,
            BatteryLook::Spawner(..) => &BATTERY_HEIGHTS[..1],
        };
        // How it was seen, for the note: near, or the height of the first
        // clear line (the ones before it blocked).
        let seen = if near(at) {
            Some("near".to_string())
        } else {
            if cast >= tuning.battery_rays {
                break;
            }
            cast += 1;
            let mut seen = None;
            for (k, &up) in heights.iter().enumerate() {
                if !rays.take(me.place, me.team) {
                    break 'looks;
                }
                if super::aimpoint::sight_line(sight, eye, at + Vec3::new(0.0, up, 0.0)) {
                    seen = Some(format!("a clear line at +{up:.2} m, {k} blocked before it"));
                    break;
                }
            }
            seen
        };
        let Some(how) = seen else {
            continue;
        };
        let by = |what: &str| {
            format!(
                "{what} by player {} from [{:.1}, {:.1}, {:.1}], {how}",
                me.place, eye.x, eye.y, eye.z
            )
        };
        match look {
            BatteryLook::Battery(battery, at) => team.change_battery(
                battery,
                Some(KnownBattery {
                    at,
                    time: view.clock,
                    hud: false,
                }),
                || by("seen"),
            ),
            BatteryLook::Gone(battery, _) => {
                team.change_battery(battery, None, || by("seen empty"))
            }
            BatteryLook::Spawner(index, _) => team.saw_spawner(index, view.clock),
        }
    }
}

/// The noticing delay for a sight at `distance`: near to far, shorter for
/// a busy enemy, times the skill's factor.
pub fn notice_ms(tuning: &SenseTuning, distance: f32, busy: bool, skill_factor: f32) -> u32 {
    let t = (distance / tuning.notice_far_m).clamp(0.0, 1.0);
    let base = tuning.notice_near_ms as f32
        + (tuning.notice_far_ms as f32 - tuning.notice_near_ms as f32) * t;
    let base = if busy {
        base * tuning.notice_moving
    } else {
        base
    };
    (base * skill_factor) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::tuning::Tuning;
    use crate::v026g::bots::view::{Mode, Sound};

    /// The tuning with the owner's options off (the bots as they were): what a test of an option's effect starts from.
    fn off() -> Tuning {
        Tuning::all_off()
    }

    fn place(place: Place, team: u8, feet: Vec3, yaw: f32) -> PlaceView {
        PlaceView {
            place,
            team,
            bot: place == 0,
            present: true,
            alive: true,
            feet,
            yaw,
            pitch: 0.0,
            velocity: [0.0, 0.0],
            keys: 0,
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

    fn view(places: Vec<PlaceView>, sounds: Vec<Sound>) -> MatchView {
        MatchView {
            clock: 1_000,
            mode: Mode::TeamDeathmatch,
            time_left_s: 600.0,
            total_s: 900.0,
            points: [0, 0],
            places,
            batteries: vec![],
            marks: vec![],
            containers: vec![],
            sounds,
            deaths: vec![],
            world_time: 0,
            can_respawn: [true, true],
            next_battery: None,
            grenades: Vec::new(),
            smoke: Default::default(),
        }
    }

    #[test]
    fn an_enemy_in_view_with_a_clear_line_is_seen_one_behind_is_only_heard() {
        // Facing +z (yaw 0): one enemy ahead, one behind firing.
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let ahead = place(1, 1, Vec3::new(0.0, 0.0, 30.0), 0.0);
        let behind = place(2, 1, Vec3::new(0.0, 0.0, -20.0), 0.0);
        let shot = Sound {
            from: 2,
            kind: SoundKind::Shot,
            at: behind.feet,
            time: 900,
            reach_m: None,
        };
        let view = view(vec![me.clone(), ahead, behind], vec![shot]);
        let sight = RayTree::build(&[]);
        let mut rays = RayBudget::new(10);
        let percepts = sense(
            &me,
            &view,
            &sight,
            &off().sense,
            &off().cheats,
            500,
            &[1, 2],
            &mut rays,
            &mut Rng::new(1),
        );
        assert_eq!(percepts.len(), 2, "{percepts:?}");
        assert_eq!((percepts[0].who, percepts[0].source), (1, Source::Seen));
        assert_eq!((percepts[1].who, percepts[1].source), (2, Source::Heard));
        assert!(percepts[1].error_m > 0.0 && percepts[1].busy);
        // No rays left in the poll, the refresh off (the default, the tip's): nothing is seen, and the
        // refusals are counted.
        let mut none = RayBudget::new(0);
        let percepts = sense(
            &me,
            &view,
            &sight,
            &off().sense,
            &off().cheats,
            500,
            &[1, 2],
            &mut none,
            &mut Rng::new(1),
        );
        assert!(percepts.iter().all(|p| p.source != Source::Seen));
        assert_eq!((none.left, none.starved() > 0, none.exempt), (0, true, 0));
        // With the refresh an enemy in view is tested all the same (outside the budget, counted apart),
        // and nothing is starved.
        let mut none = RayBudget::new(0);
        let percepts = sense(
            &me,
            &view,
            &sight,
            &refreshed(),
            &off().cheats,
            500,
            &[1, 2],
            &mut none,
            &mut Rng::new(1),
        );
        assert!(percepts.iter().any(|p| p.who == 1 && p.source == Source::Seen));
        assert_eq!((none.left, none.starved(), none.exempt), (0, 0, 1));
    }

    /// A trap that went off under an enemy is heard like his steps
    /// (`SenseTuning::step_m`): within it, by his enemies only.
    #[test]
    fn a_trap_that_went_off_under_an_enemy_is_heard_at_the_range_of_his_steps() {
        let tuning = SenseTuning::default();
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let hear = |victim: PlaceView| {
            let snap = Sound {
                from: victim.place,
                kind: SoundKind::Trap,
                at: victim.feet,
                time: 900,
                reach_m: None,
            };
            let view = view(vec![me.clone(), victim], vec![snap]);
            sense(
                &me,
                &view,
                &RayTree::build(&[]),
                &tuning,
                &CheatTuning::default(),
                500,
                &[1],
                &mut RayBudget::new(0),
                &mut Rng::new(1),
            )
        };
        let near = hear(place(1, 1, Vec3::new(0.0, 0.0, -tuning.step_m * 1.1), 0.0));
        assert_eq!(near.len(), 1, "{near:?}");
        assert_eq!((near[0].who, near[0].source, near[0].busy), (1, Source::Heard, false));
        assert!(hear(place(1, 1, Vec3::new(0.0, 0.0, -tuning.step_m * 1.3), 0.0)).is_empty());
        assert!(hear(place(1, 0, Vec3::new(0.0, 0.0, -2.0), 0.0)).is_empty(), "a mate's");
    }

    /// Match 0x6da44bbb at 662-677 s: an enemy at arm's length behind, in a
    /// clear line, is seen (`touch_m`); one farther behind is not, nor one
    /// as near behind a wall, nor any with the rule off.
    #[test]
    fn an_enemy_at_arms_length_behind_is_seen_unless_a_wall_is_between() {
        // Facing +z (yaw 0); both enemies behind.
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let close = place(1, 1, Vec3::new(0.3, 0.0, -1.2), 0.0);
        let farther = place(2, 1, Vec3::new(-0.3, 0.0, -3.0), 0.0);
        let view = view(vec![me.clone(), close, farther], vec![]);
        let seen = |sight: &RayTree, tuning: &SenseTuning| -> Vec<Place> {
            sense(
                &me,
                &view,
                sight,
                tuning,
                &CheatTuning::default(),
                500,
                &[1, 2],
                &mut RayBudget::new(20),
                &mut Rng::new(1),
            )
            .iter()
            .filter(|p| p.source == Source::Seen)
            .map(|p| p.who)
            .collect()
        };
        let open = RayTree::build(&[]);
        let tuning = SenseTuning {
            touch_m: 2.0,
            step_m: 0.0,
            ..SenseTuning::default()
        };
        assert_eq!(seen(&open, &tuning), vec![1]);
        // A wall across z = -0.6, floor to ceiling.
        let (a, b) = (Vec3::new(-5.0, -1.0, -0.6), Vec3::new(5.0, -1.0, -0.6));
        let (c, d) = (Vec3::new(5.0, 4.0, -0.6), Vec3::new(-5.0, 4.0, -0.6));
        let walled = RayTree::from_corners([[a, b, c], [a, c, d]]);
        assert!(seen(&walled, &tuning).is_empty());
        let off = SenseTuning {
            touch_m: 0.0,
            ..tuning
        };
        assert!(seen(&open, &off).is_empty());
    }

    /// Audit issue 4: footsteps. An enemy walking upright behind within
    /// `step_m` is heard; one crouched, one standing, one farther is not.
    #[test]
    fn a_walking_enemy_near_behind_is_heard_by_his_steps() {
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let walking = |p: Place, z: f32, keys: u32, speed: f32| PlaceView {
            velocity: [speed, 0.0],
            keys,
            ..place(p, 1, Vec3::new(0.5, 0.0, z), 0.0)
        };
        let view = view(
            vec![
                me.clone(),
                walking(1, -4.0, 0, 3.0),
                walking(2, -3.0, crate::v026g::drive::key::CROUCH, 2.0),
                walking(3, -2.0, 0, 0.0),
                walking(4, -12.0, 0, 3.0),
            ],
            vec![],
        );
        let tuning = SenseTuning {
            step_m: 8.0,
            ..off().sense
        };
        let percepts = sense(
            &me,
            &view,
            &RayTree::build(&[]),
            &tuning,
            &off().cheats,
            500,
            &[1, 2, 3, 4],
            &mut RayBudget::new(10),
            &mut Rng::new(1),
        );
        let heard: Vec<Place> = percepts
            .iter()
            .filter(|p| p.source == Source::Heard)
            .map(|p| p.who)
            .collect();
        assert_eq!(heard, vec![1], "{percepts:?}");
        assert!(percepts[0].at.distance_xz(Vec3::new(0.5, 0.0, -4.0)) <= 4.0 * tuning.heard_error * 1.5);
        // Off (0): nothing.
        let percepts = sense(
            &me,
            &view,
            &RayTree::build(&[]),
            &SenseTuning {
                step_m: 0.0,
                ..tuning
            },
            &off().cheats,
            500,
            &[1, 2, 3, 4],
            &mut RayBudget::new(10),
            &mut Rng::new(1),
        );
        assert!(percepts.is_empty(), "{percepts:?}");
    }

    /// The places heard (not seen) by bot 0 at the origin, by `ranges`, with
    /// the hearing cheat at `hearing`.
    fn heard_by(ranges: HearingRanges, hearing: f32, view: &MatchView, who: &[Place]) -> Vec<Place> {
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let tuning = SenseTuning {
            ranges,
            ..SenseTuning::default()
        };
        let cheats = CheatTuning {
            hearing,
            ..CheatTuning::default()
        };
        sense(
            &me,
            view,
            &RayTree::build(&[]),
            &tuning,
            &cheats,
            500,
            who,
            &mut RayBudget::new(10),
            &mut Rng::new(1),
        )
        .iter()
        .filter(|p| p.source == Source::Heard)
        .map(|p| p.who)
        .collect()
    }

    /// H1b: under the game's ranges a crouched mover is heard within the
    /// crouched step's range, by the distance in 3D; flat, never.
    #[test]
    fn a_crouched_mover_is_heard_within_the_crouched_steps_range_in_3d_only_under_the_game_ranges() {
        let crouch = crate::v026g::drive::key::CROUCH;
        let mover = |p: Place, at: Vec3, keys: u32| PlaceView {
            velocity: [2.5, 0.0],
            keys,
            ..place(p, 1, at, 0.0)
        };
        let game = HearingRanges::Game { crouch_step_m: 4.5 };
        let view = view(
            vec![
                place(0, 0, Vec3::ZERO, 0.0),
                mover(1, Vec3::new(0.5, 0.0, -4.0), crouch),
                mover(2, Vec3::new(0.5, 0.0, -6.0), crouch),
                // 4 m across and 3.4 m under: 5.2 m in 3D.
                mover(3, Vec3::new(0.5, -3.4, -4.0), crouch),
                mover(4, Vec3::new(0.5, 0.0, -7.0), 0),
                mover(5, Vec3::new(0.5, 0.0, -9.0), 0),
            ],
            vec![],
        );
        let all = [1, 2, 3, 4, 5];
        assert_eq!(heard_by(game, 1.0, &view, &all), vec![1, 4]);
        // Flat: the crouched are never heard, the upright within 8 m by the ground distance.
        assert_eq!(heard_by(HearingRanges::Flat, 1.0, &view, &all), vec![4]);
    }

    /// H1b: the cheat's 1.25 never takes a sound past its cull under the
    /// game's ranges (a sprint 17 m, a reload 9 m); flat, it does.
    #[test]
    fn the_hearing_cheat_stops_at_the_cull_under_the_game_ranges() {
        let sound = |kind: SoundKind, z: f32| Sound {
            from: 1,
            kind,
            at: Vec3::new(0.0, 0.0, z),
            time: 600,
            reach_m: None,
        };
        let view = |sounds: Vec<Sound>| view(vec![place(0, 0, Vec3::ZERO, 0.0), place(1, 1, Vec3::new(0.0, 0.0, -50.0), 0.0)], sounds);
        let game = HearingRanges::Game { crouch_step_m: 4.5 };
        let heard = |ranges, kind, z| !heard_by(ranges, 1.25, &view(vec![sound(kind, z)]), &[1]).is_empty();
        // A sprint at 16.5 m is heard (15 x 1.25 = 18.75 flat, 17 under the game's), one at 17.5 only flat.
        assert!(heard(game, SoundKind::Sprint, 16.5));
        assert!(!heard(game, SoundKind::Sprint, 17.5));
        assert!(heard(HearingRanges::Flat, SoundKind::Sprint, 17.5));
        // A reload at 8.5 m is heard, at 9.5 m only flat (10 x 1.25 = 12.5).
        assert!(heard(game, SoundKind::Reload, 8.5));
        assert!(!heard(game, SoundKind::Reload, 9.5));
        assert!(heard(HearingRanges::Flat, SoundKind::Reload, 9.5));
    }

    /// H1c: a shot is heard as far as its weapon's reach (the cheat adds
    /// none past it), an unknown weapon's at the fallback; flat, the flat
    /// range times the cheat.
    #[test]
    fn a_shot_is_heard_as_far_as_its_weapons_reach_when_asked() {
        let shot = |reach_m: Option<f32>, z: f32| Sound {
            from: 1,
            kind: SoundKind::Shot,
            at: Vec3::new(0.0, 0.0, z),
            time: 600,
            reach_m,
        };
        let heard = |shots: ShotRanges, reach_m: Option<f32>, z: f32| {
            let view = view(
                vec![place(0, 0, Vec3::ZERO, 0.0), place(1, 1, Vec3::new(0.0, 0.0, -300.0), 0.0)],
                vec![shot(reach_m, z)],
            );
            let me = place(0, 0, Vec3::ZERO, 0.0);
            let tuning = SenseTuning {
                shots,
                ..SenseTuning::default()
            };
            let cheats = CheatTuning {
                hearing: 1.25,
                ..CheatTuning::default()
            };
            !sense(&me, &view, &RayTree::build(&[]), &tuning, &cheats, 500, &[1], &mut RayBudget::new(10), &mut Rng::new(1))
                .is_empty()
        };
        let by_weapon = ShotRanges::ByWeapon { unknown_m: 150.0 };
        assert!(heard(by_weapon, Some(150.0), 140.0));
        assert!(!heard(by_weapon, Some(150.0), 160.0), "the cheat adds none past the reach");
        assert!(!heard(by_weapon, Some(55.0), 70.0), "a suppressed weapon");
        assert!(heard(by_weapon, Some(200.0), 190.0), "a bolt rifle");
        assert!(heard(by_weapon, None, 140.0), "an unknown weapon at the fallback");
        assert!(!heard(by_weapon, None, 160.0));
        // Flat: 60 m x 1.25 = 75 m for every shot, whatever its reach.
        assert!(heard(ShotRanges::Flat, Some(200.0), 70.0));
        assert!(!heard(ShotRanges::Flat, Some(200.0), 80.0));
    }

    fn spawner(x: f32, z: f32) -> mapdata::BatterySpawner {
        mapdata::BatterySpawner {
            position: Vec3::new(x, 0.0, z),
            item: Vec3::new(x, 0.3, z),
            layer: String::new(),
        }
    }

    /// A wall 4 m high across x = -5..5 at z = 15 (both faces).
    fn wall() -> RayTree {
        let (a, b, c, d) = (
            Vec3::new(-5.0, 0.0, 15.0),
            Vec3::new(5.0, 0.0, 15.0),
            Vec3::new(5.0, 4.0, 15.0),
            Vec3::new(-5.0, 4.0, 15.0),
        );
        RayTree::from_corners([[a, b, c], [a, c, d], [c, b, a], [d, c, a]])
    }

    /// With the battery cheat off: a battery in view in a clear line
    /// becomes the team's, one behind the wall or behind the bot does not
    /// (within the near distance it does at once); a spawner's spot seen
    /// is checked; a spot believed to hold a battery, seen empty, is
    /// forgotten; and no more rays are cast than allowed.
    #[test]
    fn a_bot_sees_batteries_and_spawners_for_its_team_in_a_clear_line() {
        use crate::v026g::bots::view::BatteryState;
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let mut view = view(vec![me.clone()], vec![]);
        view.mode = Mode::Batteries;
        view.batteries = vec![
            BatteryState::Free(Vec3::new(3.0, 0.3, 10.0)),
            BatteryState::Free(Vec3::new(0.0, 0.3, 25.0)),
            BatteryState::Free(Vec3::new(0.0, 0.3, -10.0)),
            BatteryState::Free(Vec3::new(1.0, 0.3, -2.0)),
            BatteryState::Carried(None),
        ];
        let spawners = [spawner(-4.0, 12.0), spawner(0.0, 30.0)];
        let mut team = TeamKnowledge::default();
        // Battery 4 was seen lying at (6, 0.3, 8); it is gone from there.
        team.set_battery(
            4,
            Some(KnownBattery {
                at: Vec3::new(6.0, 0.3, 8.0),
                time: 0,
                hud: false,
            }),
        );
        let tuning = SenseTuning::default();
        let mut rays = RayBudget::new(100);
        let sight = wall();
        sense_batteries(
            &me,
            &view,
            &sight,
            &spawners,
            &tuning,
            &mut team,
            |_| true,
            &mut rays,
        );
        // Three rays at most: battery 3 is near (no ray), then battery 4's
        // spot, battery 0 and spawner 0 by distance.
        assert_eq!(rays.left, 97);
        assert!(team.battery(3).is_some(), "near, behind it");
        assert!(team.battery(4).is_none(), "seen gone");
        assert_eq!(team.battery(0).map(|k| k.hud), Some(false));
        assert_eq!(team.spawner_seen(0), Some(view.clock));
        // Each change is noted, with the bot, its eye and how it saw.
        let notes = std::mem::take(&mut team.battery_notes);
        assert_eq!(notes.len(), 3, "{notes:?}");
        assert!(
            notes[0]
                .starts_with("battery 3 known at [1.0, 0.3, -2.0] seen (seen by player 0 from ["),
            "{notes:?}"
        );
        assert!(notes[0].ends_with("], near)"), "{notes:?}");
        assert!(
            notes[1].starts_with(
                "battery 4 no longer known at [6.0, 0.3, 8.0] seen (seen empty by player 0 from ["
            ),
            "{notes:?}"
        );
        assert!(notes[1].ends_with(", 0 blocked before it)"), "{notes:?}");
        assert!(
            notes[2].starts_with("battery 0 known at [3.0, 0.3, 10.0] seen (seen by player 0"),
            "{notes:?}"
        );
        // The next think: battery 1 and spawner 1 lie behind the wall,
        // battery 2 behind the bot. The battery is tried at its three
        // heights, the spot at one.
        sense_batteries(
            &me,
            &view,
            &sight,
            &spawners,
            &tuning,
            &mut team,
            |i| i == 1,
            &mut rays,
        );
        assert_eq!(rays.left, 93);
        assert!(team.battery(1).is_none() && team.battery(2).is_none());
        assert_eq!(team.spawner_seen(1), None);
        // Without the wall both are seen.
        sense_batteries(
            &me,
            &view,
            &RayTree::build(&[]),
            &spawners,
            &tuning,
            &mut team,
            |i| i == 1,
            &mut rays,
        );
        assert!(team.battery(1).is_some() && team.battery(2).is_none());
        assert_eq!(team.spawner_seen(1), Some(view.clock));
    }

    /// A battery whose top a low strip hides (a nook's lip) is still seen
    /// by its middle, as a human sees it; so is a spot believed to hold one
    /// that is empty (forgotten).
    #[test]
    fn a_battery_whose_top_is_hidden_is_seen_by_its_middle() {
        use crate::v026g::bots::view::BatteryState;
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let mut view = view(vec![me.clone()], vec![]);
        view.mode = Mode::Batteries;
        let at = Vec3::new(0.0, 0.0, 10.0);
        view.batteries = vec![BatteryState::Free(at)];
        // A strip 0.2-0.4 m high just short of it: the ray to the top
        // (0.25 m) meets it, the one to the middle (0.12 m) passes under.
        let (a, b, c, d) = (
            Vec3::new(-1.0, 0.2, 9.9),
            Vec3::new(1.0, 0.2, 9.9),
            Vec3::new(1.0, 0.4, 9.9),
            Vec3::new(-1.0, 0.4, 9.9),
        );
        let sight = RayTree::from_corners([[a, b, c], [a, c, d], [c, b, a], [d, c, a]]);
        let eye = me.eye();
        assert!(!crate::v026g::bots::aimpoint::sight_line(
            &sight,
            eye,
            at + Vec3::new(0.0, BATTERY_HEIGHTS[0], 0.0)
        ));
        let mut team = TeamKnowledge::default();
        let mut rays = RayBudget::new(100);
        sense_batteries(
            &me,
            &view,
            &sight,
            &[],
            &SenseTuning::default(),
            &mut team,
            |_| true,
            &mut rays,
        );
        assert!(team.battery(0).is_some());
        assert_eq!(rays.left, 98, "the top, then the middle");
        // Taken since: the spot, seen empty by its middle, is forgotten.
        view.batteries = vec![BatteryState::Carried(None)];
        sense_batteries(
            &me,
            &view,
            &sight,
            &[],
            &SenseTuning::default(),
            &mut team,
            |_| true,
            &mut rays,
        );
        assert!(team.battery(0).is_none());
        assert_eq!(rays.left, 96);
    }

    /// Audit issue 7 (0x4da3b51d, battery 3): a battery the team knows,
    /// taken and dropped 0.7 m on unseen, is put right where it lies when
    /// it is seen (a take's stand is near it: no ray), so the take aims at
    /// it and not at the empty spot. One that lies where it is known (a few
    /// centimetres of settling) costs no look.
    #[test]
    fn a_known_battery_seen_lying_a_little_off_is_put_right() {
        use crate::v026g::bots::view::BatteryState;
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let mut view = view(vec![me.clone()], vec![]);
        view.mode = Mode::Batteries;
        let known = Vec3::new(0.0, 0.3, 10.0);
        let mut team = TeamKnowledge::default();
        team.set_battery(
            0,
            Some(KnownBattery {
                at: known,
                time: 0,
                hud: false,
            }),
        );
        // Settled 5 cm: no look, nothing changes.
        view.batteries = vec![BatteryState::Free(known + Vec3::new(0.05, 0.0, 0.0))];
        let mut rays = RayBudget::new(100);
        let sight = RayTree::build(&[]);
        let tuning = SenseTuning::default();
        sense_batteries(&me, &view, &sight, &[], &tuning, &mut team, |_| true, &mut rays);
        assert_eq!(rays.left, 100);
        assert_eq!(team.battery(0).map(|k| k.at), Some(known));
        // Taken and dropped 0.7 m on: seen there, the team's spot follows.
        let moved = Vec3::new(0.7, 0.1, 10.0);
        view.batteries = vec![BatteryState::Free(moved)];
        sense_batteries(&me, &view, &sight, &[], &tuning, &mut team, |_| true, &mut rays);
        assert_eq!(team.battery(0).map(|k| k.at), Some(moved));
        // From a take's stand a metre off it, no ray is needed.
        let mut team = TeamKnowledge::default();
        team.set_battery(
            0,
            Some(KnownBattery {
                at: known,
                time: 0,
                hud: false,
            }),
        );
        let stand = place(0, 0, Vec3::new(0.0, 0.0, 9.0), 0.0);
        let mut none = RayBudget::new(0);
        sense_batteries(&stand, &view, &sight, &[], &tuning, &mut team, |_| true, &mut none);
        assert_eq!(team.battery(0).map(|k| k.at), Some(moved));
    }

    #[test]
    fn noticing_takes_longer_far_away_and_less_for_a_busy_enemy() {
        let tuning = SenseTuning::default();
        assert_eq!(notice_ms(&tuning, 0.0, false, 1.0), 200);
        assert_eq!(notice_ms(&tuning, 100.0, false, 1.0), 500);
        assert!(notice_ms(&tuning, 30.0, true, 1.0) < notice_ms(&tuning, 30.0, false, 1.0));
        assert!(in_view(Vec3::ZERO, 0.0, Vec3::new(1.0, 0.0, 5.0), 90.0));
        assert!(!in_view(Vec3::ZERO, 0.0, Vec3::new(0.0, 0.0, -5.0), 90.0));
    }

    /// The senses with the sight refresh on (it is off by default).
    fn refreshed() -> SenseTuning {
        SenseTuning {
            sight_refresh: true,
            ..SenseTuning::default()
        }
    }

    fn enemy_ahead(z: f32) -> PlaceView {
        place(1, 1, Vec3::new(0.0, 0.0, z), 0.0)
    }

    /// One look at a live sight: seen in the open, and the ways it fails,
    /// each told apart: range, view, a wall, death, another life.
    #[test]
    fn a_look_tells_seen_from_each_way_it_fails() {
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let tuning = SenseTuning::default();
        let open = RayTree::build(&[]);
        let looked = |me: &PlaceView, enemy: &PlaceView, since: u32, sight: &RayTree| {
            let mut rays = RayBudget::new(LOOK_RAYS);
            let outcome = look(me, enemy, since, sight, &tuning, &mut rays);
            assert_eq!(rays.starved(), 0, "a look has its own allowance");
            outcome
        };
        assert_eq!(looked(&me, &enemy_ahead(30.0), 100, &open), LookOutcome::Seen);
        assert_eq!(
            looked(&me, &enemy_ahead(200.0), 100, &open),
            LookOutcome::Failed(Miss::Range)
        );
        // He is behind the bot's view; turning back finds him.
        let behind = enemy_ahead(-30.0);
        assert_eq!(looked(&me, &behind, 100, &open), LookOutcome::Failed(Miss::Cone));
        let turned = place(0, 0, Vec3::ZERO, std::f32::consts::PI);
        assert_eq!(looked(&turned, &behind, 100, &open), LookOutcome::Seen);
        assert_eq!(
            looked(&me, &enemy_ahead(30.0), 100, &wall()),
            LookOutcome::Failed(Miss::Blocked)
        );
        let dead = PlaceView {
            alive: false,
            ..enemy_ahead(30.0)
        };
        assert_eq!(looked(&me, &dead, 100, &open), LookOutcome::Failed(Miss::Dead));
        // Another life than the one seen: spawned after the sight began.
        let respawned = PlaceView {
            spawned: 500,
            ..enemy_ahead(30.0)
        };
        assert_eq!(looked(&me, &respawned, 400, &open), LookOutcome::Failed(Miss::Dead));
        assert_eq!(looked(&me, &respawned, 600, &open), LookOutcome::Seen);
    }

    /// A look is the sight's test and nothing else: the same rays as a
    /// think's for a new enemy, at most `LOOK_RAYS` of them, none drawn at
    /// random (it takes no random stream), and a starved one is untested,
    /// not a negative.
    #[test]
    fn a_look_casts_at_most_seven_rays_and_a_starved_one_is_untested() {
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let enemy = enemy_ahead(30.0);
        let tuning = SenseTuning::default();
        let rays_for = |sight: &RayTree, left: u32| {
            let before = crate::v026g::bots::aimpoint::rays_cast();
            let mut rays = RayBudget::new(left);
            let outcome = look(&me, &enemy, 100, sight, &tuning, &mut rays);
            (outcome, crate::v026g::bots::aimpoint::rays_cast() - before, rays)
        };
        // In the open the chest shows at the first ray.
        let (outcome, cast, _) = rays_for(&RayTree::build(&[]), LOOK_RAYS);
        assert_eq!((outcome, cast), (LookOutcome::Seen, 1));
        // Behind a wall every part is tried, and the allowance is exact.
        let (outcome, cast, rays) = rays_for(&wall(), LOOK_RAYS);
        assert_eq!((outcome, cast), (LookOutcome::Failed(Miss::Blocked), 7));
        assert_eq!((rays.left, rays.starved()), (0, 0));
        // Three rays only: three parts tried, the rest refused and counted.
        let (outcome, cast, rays) = rays_for(&wall(), 3);
        assert_eq!((outcome, cast), (LookOutcome::Untested, 3));
        assert_eq!(rays.starved(), 4);
        assert_eq!((rays.starved_by_team, rays.starved_by_place[0]), ([4, 0], 4));
        // No ray at all: untested too, with a clear line as well.
        assert_eq!(rays_for(&RayTree::build(&[]), 0).0, LookOutcome::Untested);
    }

    #[test]
    fn the_fair_share_splits_what_is_left_among_the_thinks_still_due() {
        let most = think_most(3);
        assert_eq!(most, 9);
        // Nothing binds: no limit (a poll that does not bind is decided as
        // it was).
        assert_eq!(fair_share(400, 4, most), None);
        assert_eq!(fair_share(36, 4, most), None, "exactly enough");
        assert_eq!(fair_share(400, 0, most), None, "no think due: no limit");
        // 7 thinks of 9 want 63 of 30: 4 each.
        assert_eq!(fair_share(30, 7, most), Some(4));
        // The last think of the poll has all that is left.
        assert_eq!(fair_share(5, 1, most), Some(5));
        // Nearly empty: the floor of a battery look is still what it has,
        // as far as there is any.
        assert_eq!(fair_share(5, 4, most), Some(3));
        assert_eq!(fair_share(2, 4, most), Some(2));
    }

    /// The batteries of a match in the open for every bot, behind a wall
    /// (each look at them costs its three rays, three looks a think): the
    /// rays one poll gives each due think, in bot index order, with and
    /// without the fair share. Returns (rays taken by each due think, the
    /// budget after).
    fn stalled_poll(due: &[Place], budget: u32, fair: bool) -> (Vec<u32>, RayBudget) {
        use crate::v026g::bots::view::BatteryState;
        let places: Vec<PlaceView> = (0..20)
            .map(|p| place(p, u8::from(p >= 10), Vec3::new(p as f32 * 0.2, 0.0, 0.0), 0.0))
            .collect();
        let mut view = view(places.clone(), vec![]);
        view.mode = Mode::Batteries;
        view.batteries = [-3.0, 0.0, 3.0]
            .iter()
            .map(|&x| BatteryState::Free(Vec3::new(x, 0.3, 20.0)))
            .collect();
        let sight = wall();
        let tuning = SenseTuning::default();
        let mut team = TeamKnowledge::default();
        let mut rays = RayBudget::new(budget);
        let mut used = Vec::new();
        for (k, &p) in due.iter().enumerate() {
            let before = rays.left;
            let thinks_left = (due.len() - k) as u32;
            rays.allow(fair.then(|| fair_share(rays.left, thinks_left, think_most(3))).flatten());
            let (me, spawners) = (&places[p], &[]);
            sense_batteries(me, &view, &sight, spawners, &tuning, &mut team, |_| false, &mut rays);
            rays.allow(None);
            used.push(before - rays.left);
        }
        (used, rays)
    }

    /// A poll 100 to 230 ms late has 6 or 7 thinks due at once: with the
    /// budget in bot index order the last ones get nothing, every poll and
    /// always one team; with the fair share each gets its part, the teams
    /// alike.
    #[test]
    fn a_stalled_poll_shares_its_rays_fairly_across_the_bot_index_and_the_teams() {
        // Three of team 0 and four of team 1 due, 30 rays for 63 wanted.
        let due = [2, 4, 7, 11, 13, 16, 18];
        let (used, rays) = stalled_poll(&due, 30, false);
        assert_eq!(used, [9, 9, 9, 3, 0, 0, 0]);
        assert_eq!(rays.starved_by_team, [0, 4], "one team starves, always");

        let (used, rays) = stalled_poll(&due, 30, true);
        assert!(used.iter().all(|&n| (BATTERY_FLOOR_RAYS..=5).contains(&n)), "{used:?}");
        assert_eq!(used.iter().sum::<u32>(), 30);
        // Every think was refused something it wanted, the teams alike.
        assert_eq!(rays.starved_by_team, [3, 4]);
        let places: Vec<usize> = (0..STARVED_PLACES)
            .filter(|&p| rays.starved_by_place[p] > 0)
            .collect();
        assert_eq!(places, due);
    }

    /// A poll that does not bind is decided as it was: the same rays, the
    /// same looks, with the fair share and without.
    #[test]
    fn a_poll_that_does_not_bind_is_decided_as_it_was() {
        let due = [3, 12, 15];
        assert_eq!(fair_share(400, 3, think_most(3)), None);
        let (plain, left_plain) = stalled_poll(&due, 400, false);
        let (fair, left_fair) = stalled_poll(&due, 400, true);
        assert_eq!(plain, fair);
        assert_eq!(plain, [9, 9, 9]);
        assert_eq!((left_plain.left, left_plain.starved()), (left_fair.left, 0));
        assert_eq!(left_fair.starved(), 0);
    }

    /// The enemies in view are tested outside the budget, whole (the ones
    /// the budget starved are the live sights lost to a stalled poll): ten
    /// of them behind a wall cost seven rays each, counted apart, and the
    /// budget is not touched, nor is anyone starved, with 400 rays or none.
    #[test]
    fn the_enemies_in_view_are_tested_outside_the_budget_even_when_it_binds() {
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let mut places = vec![me.clone()];
        places.extend((1..=10).map(|p| {
            place(p, 1, Vec3::new(p as f32 * 0.5 - 3.0, 0.0, 30.0), 0.0)
        }));
        let view = view(places, vec![]);
        let order: Vec<Place> = (1..=10).collect();
        for left in [400, 0] {
            let mut rays = RayBudget::new(left);
            rays.allow(Some(0));
            let (percepts, looks) = sense_looks(
                &me,
                &view,
                &wall(),
                &refreshed(),
                &CheatTuning::default(),
                0,
                &order,
                &mut rays,
                &mut Rng::new(1),
            );
            assert!(percepts.is_empty());
            assert_eq!(looks.len(), 10);
            assert!(looks.iter().all(|l| l.1 == LookOutcome::Failed(Miss::Blocked)), "{looks:?}");
            assert_eq!((rays.left, rays.starved(), rays.exempt), (left, 0, 70));
        }
        // ... and the same in the open: a ray each.
        let mut rays = RayBudget::new(0);
        let (percepts, _) = sense_looks(
            &me,
            &view,
            &RayTree::build(&[]),
            &refreshed(),
            &CheatTuning::default(),
            0,
            &order,
            &mut rays,
            &mut Rng::new(1),
        );
        assert_eq!(percepts.len(), 10);
        assert_eq!((rays.starved(), rays.exempt), (0, 10));
    }

    /// The enemies to look at: the live sights first, then the longest
    /// unchecked, ties in the order given. (Every enemy in view is tested
    /// whole, so the order is only that of the percepts.)
    #[test]
    fn the_longest_unchecked_go_first_and_the_live_sights_before_them() {
        let tested = |who: Place| [0, 900, 500, 0, 700][who];
        assert_eq!(look_order(&[1, 2, 3, 4], |_| false, tested), vec![3, 2, 4, 1]);
        // A live sight is a re-test, before a search for a new enemy.
        assert_eq!(look_order(&[1, 2, 3, 4], |who| who == 1, tested), vec![1, 3, 2, 4]);
        assert_eq!(look_order(&[3, 1], |_| false, |_| 0), vec![3, 1], "ties as given");
    }

    /// What each enemy looked at came to, with the percepts: seen, out of
    /// range, behind the bot, dead, blocked; an enemy of its own team is
    /// not looked at.
    #[test]
    fn a_think_knows_what_each_enemy_it_looked_at_came_to() {
        let me = place(0, 0, Vec3::ZERO, 0.0);
        let dead = PlaceView {
            alive: false,
            ..place(3, 1, Vec3::new(0.0, 0.0, 10.0), 0.0)
        };
        let places = vec![
            me.clone(),
            enemy_ahead(20.0),
            place(2, 1, Vec3::new(0.0, 0.0, 300.0), 0.0),
            dead,
            place(4, 1, Vec3::new(0.0, 0.0, -20.0), 0.0),
            place(5, 0, Vec3::new(0.0, 0.0, 15.0), 0.0),
        ];
        let view = view(places, vec![]);
        let look = |sight: &RayTree, left: u32| {
            sense_looks(
                &me,
                &view,
                sight,
                &refreshed(),
                &CheatTuning::default(),
                0,
                &[1, 2, 3, 4, 5],
                &mut RayBudget::new(left),
                &mut Rng::new(1),
            )
        };
        let (percepts, looks) = look(&RayTree::build(&[]), 400);
        assert_eq!(percepts.iter().map(|p| p.who).collect::<Vec<_>>(), vec![1]);
        assert_eq!(
            looks,
            vec![
                (1, LookOutcome::Seen),
                (2, LookOutcome::Failed(Miss::Range)),
                (3, LookOutcome::Failed(Miss::Dead)),
                (4, LookOutcome::Failed(Miss::Cone)),
            ]
        );
        let (percepts, looks) = look(&wall(), 400);
        assert!(percepts.is_empty());
        assert_eq!(looks[0], (1, LookOutcome::Failed(Miss::Blocked)));
        // With no rays in the poll the enemy in view is tested all the same.
        let (_, looks) = look(&wall(), 0);
        assert_eq!(looks[0], (1, LookOutcome::Failed(Miss::Blocked)));
    }
}

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
use super::know::{KnownBattery, Percept, Source, TeamKnowledge};
use super::rng::Rng;
use super::tuning::{CheatTuning, SenseTuning};
use super::view::{MatchView, Place, PlaceView, SoundKind};

/// The rays a poll may still cast.
#[derive(Clone, Copy, Debug)]
pub struct RayBudget(pub u32);

impl RayBudget {
    fn take(&mut self) -> bool {
        if self.0 == 0 {
            return false;
        }
        self.0 -= 1;
        true
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

/// The percepts of the bot `me` at `now`, the sounds since `since`.
/// `order` gives the enemies to look at first (stalest first).
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
    let mut out = Vec::new();
    if !me.alive {
        return out;
    }
    let eye = me.eye();
    for &who in order {
        let Some(enemy) = view.place(who) else {
            continue;
        };
        if !enemy.alive || !enemy.present || enemy.team == me.team {
            continue;
        }
        let distance = enemy.feet.distance_xz(me.feet);
        let near = distance <= tuning.touch_m;
        if distance > tuning.sight_m
            || !(near || in_view(me.feet, me.yaw, enemy.feet, tuning.view_deg))
        {
            continue;
        }
        // [aimpoint] Seen when some part of him shows, not only the eyes
        // or the chest: legs on a stair, a head over a wall.
        let seen = super::aimpoint::any_part_visible(eye, enemy, sight, || rays.take());
        if seen {
            out.push(Percept {
                who,
                at: enemy.feet,
                error_m: 0.0,
                time: view.clock,
                source: Source::Seen,
                busy: enemy.speed() > 1.0 || enemy.keys & crate::v026g::drive::key::FIRE != 0,
            });
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
        let range = hearing
            * match sound.kind {
                SoundKind::Shot => tuning.shot_m,
                SoundKind::Sprint | SoundKind::Jump => tuning.sprint_m,
                SoundKind::Reload | SoundKind::Switch => tuning.reload_m,
                SoundKind::Use => tuning.use_m,
            };
        let distance = sound.at.distance_xz(me.feet);
        if distance > range
            || out
                .iter()
                .any(|p| p.who == sound.from && p.source == Source::Seen)
        {
            continue;
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
            let distance = enemy.feet.distance_xz(me.feet);
            if !enemy.alive
                || !enemy.present
                || enemy.team == me.team
                || distance > tuning.step_m * hearing
                || enemy.speed() <= tuning.step_speed
                || enemy.keys & crate::v026g::drive::key::CROUCH != 0
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
    out
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
                if !rays.take() {
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
    use crate::v026g::bots::view::{Mode, Sound};

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
        };
        let view = view(vec![me.clone(), ahead, behind], vec![shot]);
        let sight = RayTree::build(&[]);
        let mut rays = RayBudget(10);
        let percepts = sense(
            &me,
            &view,
            &sight,
            &SenseTuning::default(),
            &CheatTuning::default(),
            500,
            &[1, 2],
            &mut rays,
            &mut Rng::new(1),
        );
        assert_eq!(percepts.len(), 2, "{percepts:?}");
        assert_eq!((percepts[0].who, percepts[0].source), (1, Source::Seen));
        assert_eq!((percepts[1].who, percepts[1].source), (2, Source::Heard));
        assert!(percepts[1].error_m > 0.0 && percepts[1].busy);
        // No rays left: nothing is seen.
        let mut none = RayBudget(0);
        let percepts = sense(
            &me,
            &view,
            &sight,
            &SenseTuning::default(),
            &CheatTuning::default(),
            500,
            &[1, 2],
            &mut none,
            &mut Rng::new(1),
        );
        assert!(percepts.iter().all(|p| p.source != Source::Seen));
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
                &mut RayBudget(20),
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
            ..SenseTuning::default()
        };
        let percepts = sense(
            &me,
            &view,
            &RayTree::build(&[]),
            &tuning,
            &CheatTuning::default(),
            500,
            &[1, 2, 3, 4],
            &mut RayBudget(10),
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
            &CheatTuning::default(),
            500,
            &[1, 2, 3, 4],
            &mut RayBudget(10),
            &mut Rng::new(1),
        );
        assert!(percepts.is_empty(), "{percepts:?}");
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
        let mut rays = RayBudget(100);
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
        assert_eq!(rays.0, 97);
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
        assert_eq!(rays.0, 93);
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
        let mut rays = RayBudget(100);
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
        assert_eq!(rays.0, 98, "the top, then the middle");
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
        assert_eq!(rays.0, 96);
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
        let mut rays = RayBudget(100);
        let sight = RayTree::build(&[]);
        let tuning = SenseTuning::default();
        sense_batteries(&me, &view, &sight, &[], &tuning, &mut team, |_| true, &mut rays);
        assert_eq!(rays.0, 100);
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
        let mut none = RayBudget(0);
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
}

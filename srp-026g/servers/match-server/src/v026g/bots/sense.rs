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
        if distance > tuning.sight_m || !in_view(me.feet, me.yaw, enemy.feet, tuning.view_deg) {
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
/// `battery_near_m`) becomes known where it lies; a spot believed to hold
/// one, so seen empty, is forgotten; a spawner's spot so seen is checked.
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
                .is_none_or(|k| k.at.distance_xz(at) > 1.5)
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
        let seen = near(at) || {
            if cast >= tuning.battery_rays {
                break;
            }
            cast += 1;
            let mut seen = false;
            for &up in heights {
                if !rays.take() {
                    break 'looks;
                }
                if super::aimpoint::sight_line(sight, eye, at + Vec3::new(0.0, up, 0.0)) {
                    seen = true;
                    break;
                }
            }
            seen
        };
        if !seen {
            continue;
        }
        match look {
            BatteryLook::Battery(battery, at) => team.set_battery(
                battery,
                Some(KnownBattery {
                    at,
                    time: view.clock,
                    hud: false,
                }),
            ),
            BatteryLook::Gone(battery, _) => team.set_battery(battery, None),
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

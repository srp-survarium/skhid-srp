//! Where on a seen enemy a bot aims, and whether its shot would go clear
//! (the owner, 2026-10-14: never shoot a player through a wall; when only a
//! part of him shows, aim at that part, not at a head or chest behind
//! something).
//!
//! - **Parts** ([`Part`]): points on the body by stance, from the game's
//!   own hit boxes: the 0.100b server poses the player model with the
//!   game's clips (`srp-0100b` `combat/pose.rs`) and puts the head sphere
//!   at 1.602 m standing, 1.026 m crouched (its tests), the upper and lower
//!   body capsules at 1.262 and 1.020 m (radius 0.215 and 0.201), the
//!   thighs near 0.68 m and the shins near 0.30 m standing; crouched 0.717,
//!   0.477, 0.21-0.43 and 0.12-0.29 m. PROVEN for the 0.100b model; that
//!   0.26g's `base_character_full` has the same heights is a GUESS (the
//!   0.100b model is absent from 0.26g). The parts sit on the body's
//!   upright axis; the edges lie across the line of sight, inside the upper
//!   body capsule.
//! - **Sight** ([`any_part_visible`]): an enemy counts as seen when a line
//!   from the bot's eyes reaches some part, so legs on a stair or a head
//!   over a wall are seen.
//! - **The aim** ([`Sight`]): of the parts that show, the first by
//!   [`Part::AIM_ORDER`] (centre mass first, the head last). The motor
//!   turns to it with the aim's own settle, so a part changing (the enemy
//!   steps behind cover) moves the view as any new aim does. With no part
//!   showing it fires not and keeps the view on the last part it saw, where
//!   he went (a pre-aim, [`PRE_AIM_MS`] after he is out of sight).
//! - **Never through the world:** a shot goes only while the line to the
//!   part is clear (where he is and where his speed takes him by the shot)
//!   and the view's own line, the way the bullet flies, meets nothing short
//!   of him: the view now and the view it turns to this tick (the aim and
//!   its error). The view is the base pitch and the weapon's kick, which
//!   lifts the camera and the bullet together (PROVEN, [`super::recoil`]);
//!   the part of this tick's kick the turn does not take back lifts both
//!   lines (`reflex`). The 0.100b replay starts a shot at the camera (`pose.rs`
//!   `head_matrix`); the muzzle's offset is left out. All on the
//!   bullet mesh ([`mapdata::MapData::bullet_triangles`]: what the game's
//!   bullet ray meets, `cgroup & 8`, PROVEN). The game lets some bullets
//!   through thin stuff (`bullet::try_pierce`, 0x32D960; materials' `armor`, ammo's
//!   `bullet_pierce`), but the mesh keeps no material per face and the rule
//!   was not read: everything in it counts as solid.
//! - **Grenades** are not aimed here: they go over cover on purpose.
//!
//! The cost: a target's parts are looked at again every [`LOOK_AGAIN_MS`],
//! when the target changes or when the bot's eye has moved
//! [`LOOK_EYE_MOVED_M`] from where it looked, in order, stopping at the first that shows
//! (at most 8 rays, 16 for one on the move; 1 or 2 for an enemy in the
//! open); the view's lines are two rays, only at a tick it would fire.

use std::sync::Arc;

use mapdata::{RayTree, Vec3, World};

use super::kin::{forward, yaw_of};
use super::view::{MatchView, Place, PlaceView, live_eye};

/// A target's parts are looked at again after this, ms.
pub const LOOK_AGAIN_MS: u32 = 100;
/// ... or sooner, when the bot's eye is this far from where it looked
/// from, metres: a look is of the line from that eye (GUESS: a walker's
/// 3 m/s is 0.15 m a tick, and the view's rays run from the eye now; the
/// bots' own error at a shot was 0.15-0.3 m, `fact-own-eye.md` 5).
pub const LOOK_EYE_MOVED_M: f32 = 0.15;
/// The last part seen is looked at this long after the target is out of
/// sight, ms.
pub const PRE_AIM_MS: u32 = 1_500;
/// The view's line must be clear to this short of the part, metres (the
/// body's own half width: what lies within it is the body's cover, and the
/// line to the part says whether that hides it).
const SHORT_M: f32 = 0.3;
/// A target's parts must show where he is and where his speed takes him by
/// the tick's shot and the next look (the view's places are the clock's,
/// a tick is up to 100 ms ahead of it), at most this far ahead, ms.
const AHEAD_MAX_MS: u32 = 300;

/// A point on a body that a bot may see and aim at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Head,
    /// The upper body hit box: the chest.
    Chest,
    /// The lower body hit box.
    Belly,
    Pelvis,
    Thighs,
    Shins,
    /// The chest's edges across the line of sight, as the shooter sees
    /// them (a body half behind a corner).
    LeftEdge,
    RightEdge,
}

impl Part {
    /// The aim's preference: centre mass first, then the chest's edges and
    /// the lower body, the legs, and the head only when nothing else shows.
    pub const AIM_ORDER: [Part; 8] = [
        Part::Chest,
        Part::Belly,
        Part::LeftEdge,
        Part::RightEdge,
        Part::Pelvis,
        Part::Thighs,
        Part::Shins,
        Part::Head,
    ];

    /// The sight's order: the parts most often out of cover first; the
    /// pelvis, between the belly and the thighs, is left out.
    pub const SIGHT_ORDER: [Part; 7] = [
        Part::Chest,
        Part::Head,
        Part::Belly,
        Part::Thighs,
        Part::LeftEdge,
        Part::RightEdge,
        Part::Shins,
    ];

    /// Height over the feet, standing and crouched, and the offset across
    /// the line of sight (to the shooter's right), metres.
    fn place(self) -> (f32, f32, f32) {
        match self {
            Part::Head => (1.60, 1.03, 0.0),
            Part::Chest => (1.26, 0.72, 0.0),
            Part::Belly => (1.02, 0.48, 0.0),
            Part::Pelvis => (0.84, 0.38, 0.0),
            Part::Thighs => (0.66, 0.32, 0.0),
            Part::Shins => (0.30, 0.20, 0.0),
            Part::LeftEdge => (1.26, 0.72, -0.18),
            Part::RightEdge => (1.26, 0.72, 0.18),
        }
    }

    /// The part of `body` as seen from `eye`.
    pub fn point(self, body: &PlaceView, eye: Vec3) -> Vec3 {
        self.point_of(body.feet, body.crouched(), eye)
    }

    /// The part of a body with its feet at `feet`, as seen from `eye`.
    pub fn point_of(self, feet: Vec3, crouched: bool, eye: Vec3) -> Vec3 {
        let (standing, low, side) = self.place();
        let up = if crouched { low } else { standing };
        let mut at = feet + Vec3::new(0.0, up, 0.0);
        if side != 0.0 {
            // To the shooter's right: the line of sight turned a quarter
            // clockwise seen from above (yaw grows to the left, `kin`).
            let (dx, dz) = (at.x - eye.x, at.z - eye.z);
            let right = forward(yaw_of([dx, dz]) - std::f32::consts::FRAC_PI_2);
            at = at + Vec3::new(right[0] * side, 0.0, right[1] * side);
        }
        at
    }
}

// Rays cast here by this thread (sight and aim), for the tests' counts.
#[cfg(test)]
thread_local! {
    static RAYS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The rays this thread cast through this module so far.
#[cfg(test)]
pub fn rays_cast() -> u64 {
    RAYS.with(|r| r.get())
}

/// A clear line from `from` to `to` on the bullet mesh, one ray (counted
/// with the bots' sight and aim rays).
pub fn sight_line(sight: &RayTree, from: Vec3, to: Vec3) -> bool {
    visible(sight, from, to)
}

fn visible(sight: &RayTree, from: Vec3, to: Vec3) -> bool {
    #[cfg(test)]
    RAYS.with(|r| r.set(r.get() + 1));
    sight.visible(from, to)
}

fn ray_cast(sight: &RayTree, from: Vec3, dir: Vec3, max_t: f32) -> Option<f32> {
    #[cfg(test)]
    RAYS.with(|r| r.set(r.get() + 1));
    sight.ray_cast(from, dir, max_t)
}

/// Whether some part of `body` shows from `eye`, in
/// [`Part::SIGHT_ORDER`], stopping at the first that does; each ray first
/// takes one from `budget` (none left: not seen).
pub fn any_part_visible(
    eye: Vec3,
    body: &PlaceView,
    sight: &RayTree,
    budget: impl FnMut() -> bool,
) -> bool {
    shows(eye, body.feet, body.crouched(), sight, budget)
}

/// [`any_part_visible`] for a body with its feet at `feet`.
pub fn shows(
    eye: Vec3,
    feet: Vec3,
    crouched: bool,
    sight: &RayTree,
    mut budget: impl FnMut() -> bool,
) -> bool {
    Part::SIGHT_ORDER
        .iter()
        .any(|part| budget() && visible(sight, eye, part.point_of(feet, crouched, eye)))
}

/// The first part of `body` in [`Part::AIM_ORDER`] that shows from `eye`.
pub fn aim_part(eye: Vec3, body: &PlaceView, sight: &RayTree) -> Option<Part> {
    aim_part_moving(eye, body, body.feet, sight).0
}

/// [`aim_part`] for a body that may be at `then` by the shot: the part
/// must show both where he is and there. Also whether some part shows
/// where he is, which the rays already cast say: the sight's answer
/// ([`TargetLook::shown`]) at no ray more. The pelvis, which the sight's
/// order ([`Part::SIGHT_ORDER`]) leaves out, shows him only when a part
/// is found: it cannot show alone.
fn aim_part_moving(
    eye: Vec3,
    body: &PlaceView,
    then: Vec3,
    sight: &RayTree,
) -> (Option<Part>, bool) {
    let moves = then.distance_xz(body.feet) > 0.05;
    let crouched = body.crouched();
    let mut shown = false;
    let part = Part::AIM_ORDER.into_iter().find(|part| {
        let now = visible(sight, eye, part.point(body, eye));
        shown |= now && *part != Part::Pelvis;
        now && (!moves || visible(sight, eye, part.point_of(then, crouched, eye)))
    });
    (part, shown || part.is_some())
}

/// What the parts looked like at the last look.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Looked {
    who: Place,
    at: u32,
    /// The bot's eye it looked from.
    eye: Vec3,
    crouched: bool,
    part: Option<Part>,
    /// Some part showed where he was.
    shown: bool,
}

/// What the target's parts looked like at a tick, for the sight (the
/// tick's look, `reflex`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetLook {
    /// Some part of him shows from the eyes.
    pub shown: bool,
    /// Looked at again at this tick (every [`LOOK_AGAIN_MS`]), not the last
    /// look's answer kept.
    pub fresh: bool,
}

/// [measure] What kept a shot from going clear ([`Sight::blocked`]): no
/// part of the target shows; the view's ray, or the aim's, hits the world
/// this far along, metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Blocked {
    Hidden,
    View(f32),
    Aim(f32),
}

/// Where the aim goes this tick, and whether a part of the target shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    pub point: Vec3,
    pub shows: bool,
}

/// One bot's look at its target's parts, kept between ticks (the
/// reflex's; [`super::reflex::Reflex::look`] fills it before each tick).
#[derive(Clone, Default)]
pub struct Sight {
    /// The map, from the first look: without it (a reflex with no map) the
    /// aim is the chest and nothing holds a shot.
    world: Option<Arc<World>>,
    looked: Option<Looked>,
    /// The bot's eye at the last look.
    eye: Vec3,
    /// How far the target is, here and where his speed takes him, at the
    /// nearer, less [`SHORT_M`]: the view's lines must be clear to this.
    reach: f32,
    /// The last part seen of a target, where it was, and when.
    last_seen: Option<(Place, Vec3, u32)>,
}

impl std::fmt::Debug for Sight {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sight")
            .field("map", &self.world.is_some())
            .field("looked", &self.looked)
            .field("last_seen", &self.last_seen)
            .finish()
    }
}

impl Sight {
    /// Before a tick: the parts of `target` (the reflex's) that show from
    /// `me`'s eyes on `world`'s bullet mesh, looked at again when due.
    /// What the look found, if there is a target alive.
    pub fn update(
        &mut self,
        me: &PlaceView,
        target: Option<Place>,
        view: &MatchView,
        world: &Arc<World>,
        now: u32,
    ) -> Option<TargetLook> {
        if !self.world.as_ref().is_some_and(|w| Arc::ptr_eq(w, world)) {
            self.world = Some(world.clone());
        }
        let sight = &world.sight;
        let eye = me.eye();
        self.eye = eye;
        if let Some((who, _, _)) = self.last_seen
            && view.place(who).is_none_or(|p| !p.alive)
        {
            self.last_seen = None;
        }
        let Some((who, body)) =
            target.and_then(|t| view.place(t).filter(|p| p.alive).map(|p| (t, p)))
        else {
            self.looked = None;
            return None;
        };
        // Where his speed takes him by the shot and the next look.
        let ahead_s =
            (now.saturating_sub(view.clock) + LOOK_AGAIN_MS).min(AHEAD_MAX_MS) as f32 / 1000.0;
        let then = body.feet + Vec3::new(body.velocity[0], 0.0, body.velocity[1]) * ahead_s;
        let due = self.looked.is_none_or(|l| {
            l.who != who
                || l.crouched != body.crouched()
                || now >= l.at + LOOK_AGAIN_MS
                || (live_eye() && (l.eye - eye).length() > LOOK_EYE_MOVED_M)
        });
        if due {
            let (part, shown) = aim_part_moving(eye, body, then, sight);
            self.looked = Some(Looked {
                who,
                at: now,
                eye,
                crouched: body.crouched(),
                part,
                shown,
            });
        }
        let shown = self.looked.is_some_and(|l| l.shown);
        let Some(part) = self.looked.and_then(|l| l.part) else {
            return Some(TargetLook { shown, fresh: due });
        };
        let point = part.point(body, eye);
        self.last_seen = Some((who, point, now));
        self.reach = (point - eye)
            .length()
            .min((part.point_of(then, body.crouched(), eye) - eye).length())
            - SHORT_M;
        Some(TargetLook { shown, fresh: due })
    }

    /// The map's bullet mesh from the first look: what a thrown grenade
    /// meets ([`super::arc::Mesh`]); none for a reflex never told a map.
    pub fn ground(&self) -> Option<Arc<RayTree>> {
        self.world.as_ref().map(|world| world.sight.clone())
    }

    /// The aim at `who` (`body`: him now, if alive), `chest` with no map:
    /// the part that shows; with no part showing, the last part seen (where
    /// he went), and `shows` false.
    pub fn aim(&self, who: Place, body: Option<&PlaceView>, chest: Vec3) -> Aim {
        if self.world.is_none() {
            return Aim {
                point: chest,
                shows: true,
            };
        }
        let looked = self.looked.filter(|l| l.who == who);
        match (looked.and_then(|l| l.part), body) {
            (Some(part), Some(body)) => Aim {
                point: part.point(body, self.eye),
                shows: true,
            },
            _ => Aim {
                point: self
                    .last_seen
                    .filter(|(seen, _, _)| *seen == who)
                    .map_or(chest, |(_, at, _)| at),
                shows: false,
            },
        }
    }

    /// At a tick it would fire at `aim`: whether the bullet would go clear
    /// of the world, along the view of `me` now and along the view it turns
    /// to (`aim`'s point with `error`, yaw and pitch, radians), each up to
    /// the target. Always with no map; never when no part shows.
    pub fn clear_shot(&self, me: &PlaceView, aim: &Aim, error: (f32, f32)) -> bool {
        self.blocked(me, aim, error).is_none()
    }

    /// [measure] What keeps [`Self::clear_shot`] from passing, if anything:
    /// no part shows, or which ray hits the world first and how far along
    /// it, metres (the same rays in the same order).
    pub fn blocked(&self, me: &PlaceView, aim: &Aim, error: (f32, f32)) -> Option<Blocked> {
        let Some(world) = &self.world else {
            return None;
        };
        if !aim.shows {
            return Some(Blocked::Hidden);
        }
        if self.reach <= 0.0 {
            return None;
        }
        let eye = me.eye();
        let along = |yaw: f32, pitch: f32| {
            let [fx, fz] = forward(yaw);
            let (c, s) = (pitch.cos(), pitch.sin());
            Vec3::new(fx * c, s, fz * c)
        };
        let to = aim.point - eye;
        let (yaw, pitch) = (
            yaw_of([to.x, to.z]) + error.0,
            to.y.atan2(to.x.hypot(to.z)) + error.1,
        );
        if let Some(m) = ray_cast(&world.sight, eye, along(me.yaw, me.pitch), self.reach) {
            return Some(Blocked::View(m));
        }
        ray_cast(&world.sight, eye, along(yaw, pitch), self.reach).map(Blocked::Aim)
    }

    /// [measure] How long ago its target's parts were last looked at, ms.
    pub fn looked_ms(&self, now: u32) -> Option<u32> {
        self.looked.map(|l| now.saturating_sub(l.at))
    }

    /// With no target: the last part seen, for [`PRE_AIM_MS`] after.
    pub fn pre_aim(&self, now: u32) -> Option<Vec3> {
        self.last_seen
            .filter(|(_, _, at)| now <= at + PRE_AIM_MS)
            .map(|(_, at, _)| at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::bots::know::{Memory, Percept, Source};
    use crate::v026g::bots::reflex::{CombatTuning, Reflex};
    use crate::v026g::bots::rng::Rng;
    use crate::v026g::bots::tuning::{AimTuning, SenseTuning};
    use crate::v026g::bots::view::Mode;
    use crate::v026g::drive::key;

    fn body(place: Place, team: u8, feet: Vec3, crouched: bool) -> PlaceView {
        PlaceView {
            place,
            team,
            bot: true,
            present: true,
            alive: true,
            feet,
            yaw: 0.0,
            pitch: 0.0,
            velocity: [0.0, 0.0],
            keys: if crouched { key::CROUCH } else { 0 },
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

    fn view_of(places: Vec<PlaceView>) -> MatchView {
        MatchView {
            clock: 0,
            mode: Mode::TeamDeathmatch,
            time_left_s: 600.0,
            total_s: 900.0,
            points: [0, 0],
            places,
            batteries: vec![],
            marks: vec![],
            containers: vec![],
            sounds: vec![],
            deaths: vec![],
            world_time: 0,
            can_respawn: [true, true],
            next_battery: None,
            grenades: Vec::new(),
        }
    }

    /// A wall in the plane z = `z` from x = `x0` to `x1`, from `y0` up to
    /// `y1`.
    fn wall(z: f32, x0: f32, x1: f32, y0: f32, y1: f32) -> [[Vec3; 3]; 2] {
        let (a, b) = (Vec3::new(x0, y0, z), Vec3::new(x1, y0, z));
        let (c, d) = (Vec3::new(x1, y1, z), Vec3::new(x0, y1, z));
        [[a, b, c], [a, c, d]]
    }

    /// A map of `walls` alone (its walk grid is not used).
    fn world_of(walls: &[[[Vec3; 3]; 2]]) -> Arc<World> {
        Arc::new(World {
            map: mapdata::MapData::default(),
            grid: Arc::new(mapdata::WalkGrid::build(
                &[],
                [0.0, 0.0, 1.0, 1.0],
                0.25,
                mapdata::Body::default(),
            )),
            sight: Arc::new(RayTree::from_corners(walls.iter().flatten().copied())),
        })
    }

    /// The view of `me` on `point`.
    fn facing(me: &mut PlaceView, point: Vec3) {
        let eye = me.eye();
        let (dx, dz) = (point.x - eye.x, point.z - eye.z);
        me.yaw = super::super::kin::yaw_of([dx, dz]);
        me.pitch = (point.y - eye.y).atan2(dx.hypot(dz));
    }

    /// A bot at the origin facing +z, an enemy at `enemy` (seen and
    /// noticed): one second of ticks with the bot's view kept on where the
    /// reflex aims. Returns the reflex's aim points and whether it fired.
    fn duel(world: &Arc<World>, enemy: PlaceView) -> (Vec<Vec3>, bool) {
        let mut me = body(0, 0, Vec3::ZERO, false);
        let mut view = view_of(vec![me.clone(), enemy.clone()]);
        let aim = AimTuning::default();
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(5);
        let seen = |time| Percept {
            who: 1,
            at: enemy.feet,
            error_m: 0.0,
            time,
            source: Source::Seen,
            busy: false,
        };
        memory.update(&[seen(1_000)], 1_000, |_| 300, 3_000, 20_000);
        memory.update(&[seen(1_300)], 1_300, |_| 300, 3_000, 20_000);
        reflex.think(&me, &memory, 1_300, &aim, 1.0, &mut rng);
        assert_eq!(reflex.target(), Some(1));
        let (mut points, mut fired) = (Vec::new(), false);
        for k in 0..20 {
            let now = 1_300 + k * 50;
            memory.update(&[seen(now)], now, |_| 300, 3_000, 20_000);
            view.clock = now;
            reflex.look(&me, &view, world, &mut memory, &SenseTuning::default(), now);
            let o = reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
            let point = o.look_at.unwrap();
            facing(&mut me, point);
            points.push(point);
            fired |= o.fire;
        }
        (points, fired)
    }

    #[test]
    fn behind_a_waist_high_wall_it_aims_at_the_chest_and_fires() {
        // 1.1 m at 18 m; him at 20 m: the belly is hidden, the chest shows.
        let world = world_of(&[wall(18.0, -3.0, 3.0, 0.0, 1.1)]);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 20.0), false);
        let eye = body(0, 0, Vec3::ZERO, false).eye();
        assert_eq!(aim_part(eye, &enemy, &world.sight), Some(Part::Chest));
        let (points, fired) = duel(&world, enemy);
        assert!(fired);
        assert!(
            points.iter().all(|p| (p.y - 1.26).abs() < 1e-3),
            "{points:?}"
        );
    }

    /// The parts are looked at from the eye of the moment, so a look stands only while the eye
    /// is near where it looked from: a step of the bot (the tick's 0.15 m and more) or a
    /// crouch looks again within the 100 ms, the view's pitch (0.13 m at most) does not.
    #[test]
    fn the_parts_are_looked_at_again_when_the_eye_has_moved() {
        let world = world_of(&[]);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 20.0), false);
        let me = body(0, 0, Vec3::ZERO, false);
        let view = view_of(vec![me.clone(), enemy]);
        let mut sight = Sight::default();
        sight.update(&me, Some(1), &view, &world, 1_000);
        assert_eq!(sight.looked_ms(1_000), Some(0));
        // 50 ms on, the eye 0.1 m along: the look stands.
        let mut then = me.clone();
        then.feet = Vec3::new(0.1, 0.0, 0.0);
        sight.update(&then, Some(1), &view, &world, 1_050);
        assert_eq!(sight.looked_ms(1_050), Some(50));
        // The view up a radian (feet back) lifts the eye 0.13 m: still.
        then.feet = Vec3::ZERO;
        then.pitch = 1.0;
        sight.update(&then, Some(1), &view, &world, 1_050);
        assert_eq!(sight.looked_ms(1_050), Some(50));
        // 0.3 m along: looked at again, from there.
        then.pitch = 0.0;
        then.feet = Vec3::new(0.3, 0.0, 0.0);
        sight.update(&then, Some(1), &view, &world, 1_050);
        assert_eq!(sight.looked_ms(1_050), Some(0));
        // A crouch drops the eye 0.3 m (idle 1.04 against 1.46 m): looked at again.
        then.keys = key::CROUCH;
        sight.update(&then, Some(1), &view, &world, 1_100);
        assert_eq!(sight.looked_ms(1_100), Some(0));
        // And the look is as old as ever when nothing moved: 100 ms on, again.
        sight.update(&then, Some(1), &view, &world, 1_150);
        assert_eq!(sight.looked_ms(1_150), Some(50));
        sight.update(&then, Some(1), &view, &world, 1_200);
        assert_eq!(sight.looked_ms(1_200), Some(0));
    }

    #[test]
    fn over_a_high_wall_only_the_head_shows_and_is_aimed_at() {
        let world = world_of(&[wall(19.0, -3.0, 3.0, 0.0, 1.45)]);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 20.0), false);
        let eye = body(0, 0, Vec3::ZERO, false).eye();
        assert_eq!(aim_part(eye, &enemy, &world.sight), Some(Part::Head));
        let (points, fired) = duel(&world, enemy);
        assert!(fired);
        assert!(points.iter().all(|p| (p.y - 1.60).abs() < 1e-3));
    }

    /// A bot at the origin facing +z, an enemy at `enemy` (seen and
    /// noticed), four seconds of ticks: the bot's view is its base pitch
    /// and its weapon's kick as the client moves it (`recoil`); each tick
    /// the view is turned onto where the reflex aims, with the reflex's
    /// pull. Returns the longest run of ticks it fired.
    fn kicked_burst(world: &Arc<World>, enemy: PlaceView, aim: &AimTuning) -> u32 {
        let mut me = body(0, 0, Vec3::ZERO, false);
        let mut view = view_of(vec![me.clone(), enemy.clone()]);
        let mut memory = Memory::default();
        let mut reflex = Reflex::default();
        assert!(reflex.hands().held().gun.auto);
        let mut rng = Rng::new(5);
        let seen = |time| Percept {
            who: 1,
            at: enemy.feet,
            error_m: 0.0,
            time,
            source: Source::Seen,
            busy: false,
        };
        memory.update(&[seen(1_000)], 1_000, |_| 300, 3_000, 20_000);
        memory.update(&[seen(1_300)], 1_300, |_| 300, 3_000, 20_000);
        reflex.think(&me, &memory, 1_300, aim, 1.0, &mut rng);
        let mut base = 0.0;
        let (mut run, mut longest) = (0, 0);
        for k in 0..80 {
            let now = 1_300 + k * 50;
            memory.update(&[seen(now)], now, |_| 300, 3_000, 20_000);
            view.clock = now;
            let kick = reflex.weapon_tick(&me, now);
            me.pitch = base + kick;
            reflex.look(&me, &view, world, &mut memory, &SenseTuning::default(), now);
            let o = reflex.tick(&me, &view, &memory, now, true, aim, 1.0, &mut rng);
            let before = me.pitch;
            facing(&mut me, o.look_at.unwrap());
            let turn = me.pitch - before + o.pull;
            let keys = if o.fire { key::FIRE } else { 0 };
            base += turn + reflex.sent(now, keys, turn);
            run = if o.fire { run + 1 } else { 0 };
            longest = longest.max(run);
        }
        longest
    }

    #[test]
    fn a_burst_through_a_slit_stays_in_it_as_the_turns_take_back_the_kick() {
        // Only the head shows, between a wall up to 1.35 m and a beam from
        // 1.85 m, 19 m out: the line has a quarter metre either way. The
        // kick lifts the view and the shot with it (PROVEN, `recoil`), and
        // the wall test is on the view. With the kick taken back by the
        // turns, the view stays on the head and a burst runs its length.
        let world = world_of(&[
            wall(19.0, -3.0, 3.0, 0.0, 1.35),
            wall(19.0, -3.0, 3.0, 1.85, 4.0),
        ]);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 20.0), false);
        let eye = body(0, 0, Vec3::ZERO, false).eye();
        assert_eq!(aim_part(eye, &enemy, &world.sight), Some(Part::Head));
        let longest = kicked_burst(&world, enemy.clone(), &AimTuning::default());
        // Bursts are 500-1000 ms: at least 400 ms of them held.
        assert!(longest >= 8, "longest burst {} ms", longest * 50);
        // Left to lift the view, the kick of the first shot takes the line
        // into the beam by the next tick, and the burst stops there.
        let lifted = AimTuning {
            combat: CombatTuning {
                recoil_pull: 0.0,
                recoil_pull_low: 0.0,
                ..CombatTuning::default()
            },
            ..AimTuning::default()
        };
        let longest = kicked_burst(&world, enemy, &lifted);
        assert!(longest <= 2, "longest burst {} ms", longest * 50);
    }

    #[test]
    fn fully_behind_a_wall_it_does_not_fire_though_its_memory_says_seen() {
        // He just stepped behind 3 m of wall; the memory's sight lasts.
        let world = world_of(&[wall(18.0, -3.0, 3.0, 0.0, 3.0)]);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 20.0), false);
        let eye = body(0, 0, Vec3::ZERO, false).eye();
        assert_eq!(aim_part(eye, &enemy, &world.sight), None);
        let (_, fired) = duel(&world, enemy);
        assert!(!fired);
        // The same without the wall fires.
        let (_, fired) = duel(&world_of(&[]), body(1, 1, Vec3::new(0.0, 0.0, 20.0), false));
        assert!(fired);
    }

    #[test]
    fn half_behind_a_corner_it_aims_at_the_side_that_shows() {
        // A wall's end at x = 0.05, 15 m out; him at 20 m on the line
        // (facing +z, the shooter's right is +x): his middle and the
        // shooter's left of him are behind it, the right edge shows.
        let world = world_of(&[wall(15.0, -6.0, 0.05, 0.0, 3.0)]);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 20.0), false);
        let eye = body(0, 0, Vec3::ZERO, false).eye();
        let part = aim_part(eye, &enemy, &world.sight).unwrap();
        assert_eq!(part, Part::RightEdge);
        let at = part.point(&enemy, eye);
        assert!(at.x > 0.1, "the side out of cover: {at:?}");
        let (points, fired) = duel(&world, enemy.clone());
        assert!(fired);
        assert!(points.iter().all(|p| p.x > 0.1));
        // The other way round the corner: the other edge.
        let world = world_of(&[wall(15.0, -0.05, 6.0, 0.0, 3.0)]);
        assert_eq!(aim_part(eye, &enemy, &world.sight), Some(Part::LeftEdge));
        let at = Part::LeftEdge.point(&enemy, eye);
        assert!(at.x < -0.1, "{at:?}");
    }

    #[test]
    fn a_crouched_enemy_behind_a_low_wall_is_not_shot_standing_up_he_is() {
        let world = world_of(&[wall(19.0, -3.0, 3.0, 0.0, 1.15)]);
        let eye = body(0, 0, Vec3::ZERO, false).eye();
        let crouched = body(1, 1, Vec3::new(0.0, 0.0, 20.0), true);
        assert_eq!(aim_part(eye, &crouched, &world.sight), None);
        assert!(!any_part_visible(eye, &crouched, &world.sight, || true));
        let (_, fired) = duel(&world, crouched);
        assert!(!fired);
        let standing = body(1, 1, Vec3::new(0.0, 0.0, 20.0), false);
        assert_eq!(aim_part(eye, &standing, &world.sight), Some(Part::Chest));
        let (_, fired) = duel(&world, standing);
        assert!(fired);
    }

    #[test]
    fn legs_under_a_screen_are_seen_and_aimed_at() {
        // A screen from 0.8 m up hides all but the legs.
        let world = world_of(&[wall(19.0, -3.0, 3.0, 0.8, 4.0)]);
        let eye = body(0, 0, Vec3::ZERO, false).eye();
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 20.0), false);
        assert!(any_part_visible(eye, &enemy, &world.sight, || true));
        assert!(!world.sight.visible(eye, enemy.eye()) && !world.sight.visible(eye, enemy.chest()));
        assert_eq!(aim_part(eye, &enemy, &world.sight), Some(Part::Thighs));
        let (points, fired) = duel(&world, enemy);
        assert!(fired);
        assert!(points.iter().all(|p| p.y < 0.8));
    }

    #[test]
    fn the_view_off_the_part_into_a_wall_holds_the_shot() {
        // The head shows over a wall; the view, on the chest behind it, is
        // not clear: no shot until it is on the head.
        let world = world_of(&[wall(19.0, -3.0, 3.0, 0.0, 1.45)]);
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 20.0), false);
        let mut me = body(0, 0, Vec3::ZERO, false);
        let view = view_of(vec![me.clone(), enemy.clone()]);
        let mut look = Sight::default();
        facing(&mut me, enemy.chest());
        look.update(&me, Some(1), &view, &world, 1_000);
        let aim = look.aim(1, Some(&enemy), enemy.chest());
        assert!(aim.shows && (aim.point.y - 1.60).abs() < 1e-3);
        assert!(!look.clear_shot(&me, &aim, (0.0, 0.0)));
        facing(&mut me, aim.point);
        assert!(look.clear_shot(&me, &aim, (0.0, 0.0)));
        // On the head, but turning a degree low this tick: into the wall.
        assert!(!look.clear_shot(&me, &aim, (0.0, -1f32.to_radians())));
        // Out of sight altogether: no shot, the view stays where he was
        // last seen, for a while after the target is gone too.
        let hidden = world_of(&[wall(19.0, -3.0, 3.0, 0.0, 3.0)]);
        look.update(&me, Some(1), &view, &hidden, 1_200);
        let aim = look.aim(1, Some(&enemy), enemy.chest());
        assert!(!aim.shows && (aim.point.y - 1.60).abs() < 1e-3);
        assert!(!look.clear_shot(&me, &aim, (0.0, 0.0)));
        look.update(&me, None, &view, &hidden, 1_300);
        assert!(look.pre_aim(1_300).is_some());
        assert!(look.pre_aim(1_000 + PRE_AIM_MS + 1).is_none());
    }

    #[test]
    fn a_target_about_to_step_behind_cover_is_not_shot() {
        // He shows at the wall's end, running behind it at 5 m/s: in 0.2 s
        // he is hidden, so the shot is held.
        let world = world_of(&[wall(15.0, -6.0, 0.4, 0.0, 3.0)]);
        let eye = body(0, 0, Vec3::ZERO, false).eye();
        let mut enemy = body(1, 1, Vec3::new(1.2, 0.0, 20.0), false);
        assert_eq!(aim_part(eye, &enemy, &world.sight), Some(Part::Chest));
        let view = view_of(vec![body(0, 0, Vec3::ZERO, false), enemy.clone()]);
        let mut look = Sight::default();
        look.update(&view.places[0], Some(1), &view, &world, 100);
        assert!(look.aim(1, Some(&enemy), enemy.chest()).shows);
        enemy.velocity = [-5.0, 0.0];
        let view = view_of(vec![body(0, 0, Vec3::ZERO, false), enemy.clone()]);
        let mut look = Sight::default();
        look.update(&view.places[0], Some(1), &view, &world, 100);
        let aim = look.aim(1, Some(&enemy), enemy.chest());
        assert!(!aim.shows, "{aim:?}");
    }

    #[test]
    fn an_enemy_in_the_open_costs_one_ray_to_see_and_two_to_fire() {
        let sight = RayTree::from_corners([]);
        let eye = body(0, 0, Vec3::ZERO, false).eye();
        let enemy = body(1, 1, Vec3::new(0.0, 0.0, 20.0), false);
        let before = rays_cast();
        assert!(any_part_visible(eye, &enemy, &sight, || true));
        assert_eq!(rays_cast() - before, 1);
        // Hidden: every part of the sight's order, once.
        let hidden = world_of(&[wall(18.0, -3.0, 3.0, 0.0, 3.0)]);
        let before = rays_cast();
        assert!(!any_part_visible(eye, &enemy, &hidden.sight, || true));
        assert_eq!(rays_cast() - before, Part::SIGHT_ORDER.len() as u64);
        // Within the budget only.
        let mut left = 2;
        let before = rays_cast();
        assert!(!any_part_visible(eye, &enemy, &hidden.sight, || {
            left -= 1;
            left >= 0
        }));
        assert_eq!(rays_cast() - before, 2);
        // The aim: the parts once per look; the view's two lines only at a
        // tick it would fire.
        let me = body(0, 0, Vec3::ZERO, false);
        let view = view_of(vec![me.clone(), enemy.clone()]);
        let open = world_of(&[]);
        let mut look = Sight::default();
        let before = rays_cast();
        for k in 0..4 {
            look.update(&me, Some(1), &view, &open, 1_000 + k * 50);
        }
        // Looks at 1000 and 1100.
        assert_eq!(rays_cast() - before, 2);
        let aim = look.aim(1, Some(&enemy), enemy.chest());
        let before = rays_cast();
        assert!(look.clear_shot(&me, &aim, (0.0, 0.0)));
        assert_eq!(rays_cast() - before, 2);
    }

    /// The cost on the real maps, only with `SRP_MAPDATA_DIR=<mapdata-dump
    /// files>`: the sight test between floors 5-80 m apart, and the aim's
    /// look, per call and per ray.
    ///
    /// ```text
    /// SRP_MAPDATA_DIR=<dir> cargo test -p match-server --release real_sight_cost -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "a measurement; needs SRP_MAPDATA_DIR"]
    fn real_sight_cost() {
        let Some(dir) = std::env::var_os("SRP_MAPDATA_DIR").map(std::path::PathBuf::from) else {
            eprintln!("skipping: SRP_MAPDATA_DIR is not set");
            return;
        };
        for level in 1..=8 {
            let project = format!("level_{level:02}/day_4");
            let map =
                mapdata::MapData::read(&dir.join(mapdata::format::file_name(&project))).unwrap();
            let world = World::build(map, mapdata::CELL, mapdata::Body::default());
            let grid = &world.grid;
            let mut rng = Rng::new(level);
            let mut pairs = Vec::new();
            while pairs.len() < 20_000 {
                let a = grid.position(rng.below(grid.len()) as mapdata::NodeId);
                let b = grid.position(rng.below(grid.len()) as mapdata::NodeId);
                let d = a.distance_xz(b);
                if (5.0..=80.0).contains(&d) {
                    pairs.push((a, b, rng.unit() < 0.3));
                }
            }
            let (rays, started) = (rays_cast(), std::time::Instant::now());
            let seen = pairs
                .iter()
                .filter(|(a, b, crouched)| {
                    let eye = body(0, 0, *a, false).eye();
                    any_part_visible(eye, &body(1, 1, *b, *crouched), &world.sight, || true)
                })
                .count();
            let took = started.elapsed().as_secs_f64();
            let sight_rays = rays_cast() - rays;
            let (rays, started) = (rays_cast(), std::time::Instant::now());
            let aimed = pairs
                .iter()
                .filter(|(a, b, crouched)| {
                    let eye = body(0, 0, *a, false).eye();
                    aim_part(eye, &body(1, 1, *b, *crouched), &world.sight).is_some()
                })
                .count();
            let aim_took = started.elapsed().as_secs_f64();
            let aim_rays = rays_cast() - rays;
            // The aim also tries the pelvis.
            assert!(aimed >= seen, "{aimed} {seen}");
            eprintln!(
                "{project}: {} bullet triangles; {:.1} % of pairs show a part; sight {:.1} rays \
                 and {:.1} us a pair ({:.2} us a ray); aim's look {:.1} rays and {:.1} us a pair",
                world.sight.triangle_count(),
                100.0 * seen as f64 / pairs.len() as f64,
                sight_rays as f64 / pairs.len() as f64,
                took * 1e6 / pairs.len() as f64,
                took * 1e6 / sight_rays as f64,
                aim_rays as f64 / pairs.len() as f64,
                aim_took * 1e6 / pairs.len() as f64,
            );
        }
    }

    /// Where the enemy of a [`fight`] is at a poll.
    #[derive(Clone, Copy)]
    struct Enemy {
        feet: Vec3,
        alive: bool,
        spawned: u32,
    }

    fn at(feet: Vec3) -> Enemy {
        Enemy {
            feet,
            alive: true,
            spawned: 0,
        }
    }

    /// What a [`fight`] came to, by tick.
    struct Fought {
        /// (tick time, the clock it was read on, the reflex's target, the
        /// target seen).
        ticks: Vec<(u32, u32, Option<Place>, bool)>,
        /// The ticks a target's sight ended on (not his death).
        lapses: Vec<u32>,
    }

    impl Fought {
        /// The first tick, from `from` on, with no target.
        fn lost_at(&self, from: u32) -> Option<(u32, u32)> {
            self.ticks
                .iter()
                .find(|(now, _, target, _)| *now >= from && target.is_none())
                .map(|(now, clock, _, _)| (*now, *clock))
        }
    }

    /// A bot at the origin (turned to `yaw_at`) against one enemy (`enemy_at`,
    /// by the information clock) over `world`, polled every 50 ms from 1 s
    /// to `until`: a think every `gap` ms senses him, the ticks run `lead`
    /// ms ahead of the clock as the encoder's do, each with its look.
    /// `stall` = (clock, ms): the poll after `clock` comes `ms` later and
    /// its ticks all at once, on one view.
    fn fight(
        world: &Arc<World>,
        gap: u32,
        lead: u32,
        until: u32,
        stall: Option<(u32, u32)>,
        enemy_at: &dyn Fn(u32) -> Enemy,
        yaw_at: &dyn Fn(u32) -> f32,
    ) -> Fought {
        use crate::v026g::bots::sense::{RayBudget, sense};
        use crate::v026g::bots::tuning::CheatTuning;
        let aim = AimTuning::default();
        // The sight refresh on: these are its tests.
        let senses = SenseTuning {
            sight_refresh: true,
            ..SenseTuning::default()
        };
        let mut memory = Memory::default();
        memory.set_sight_window(crate::v026g::bots::know::SightWindow::refreshed());
        let mut reflex = Reflex::default();
        let mut rng = Rng::new(5);
        let mut fought = Fought {
            ticks: Vec::new(),
            lapses: Vec::new(),
        };
        let (mut clock, mut next_think, mut last_tick) = (1_000, 1_000, 1_000 + lead - 50);
        while clock < until {
            let e = enemy_at(clock);
            let mut me = body(0, 0, Vec3::ZERO, false);
            me.yaw = yaw_at(clock);
                let mut foe = body(1, 1, e.feet, false);
            (foe.alive, foe.spawned) = (e.alive, e.spawned);
            let mut view = view_of(vec![me.clone(), foe]);
            view.clock = clock;
            if clock >= next_think {
                next_think = clock + gap;
                let percepts = sense(
                    &me,
                    &view,
                    &world.sight,
                    &senses,
                    &CheatTuning::default(),
                    clock,
                    &[1],
                    &mut RayBudget::new(400),
                    &mut rng,
                );
                memory.update(&percepts, clock, |_| 300, 3_000, 20_000);
                reflex.think(&me, &memory, clock, &aim, 1.0, &mut rng);
            }
            while last_tick < clock + lead {
                last_tick += 50;
                let now = last_tick;
                reflex.look(&me, &view, world, &mut memory, &senses, now);
                reflex.tick(&me, &view, &memory, now, true, &aim, 1.0, &mut rng);
                if reflex.take_lapse().is_some() {
                    fought.lapses.push(now);
                }
                let target = reflex.target();
                let seen = target.is_some_and(|t| memory.seen(t, now, clock).is_some());
                fought.ticks.push((now, clock, target, seen));
            }
            clock += match stall {
                Some((at, ms)) if clock == at => ms,
                _ => 50,
            };
        }
        fought
    }

    /// 8.13: a target in plain view is never lost to the think cadence,
    /// whatever the think gap (200-400 ms) and the input's lead (50-100 ms):
    /// before, 19 times a bot-minute.
    #[test]
    fn a_target_in_the_open_is_never_lost_between_thinks() {
        let world = world_of(&[]);
        for lead in [50, 100] {
            for gap in [200, 250, 300, 350, 400] {
                let f = fight(
                    &world,
                    gap,
                    lead,
                    8_000,
                    None,
                    &|_| at(Vec3::new(0.0, 0.0, 20.0)),
                    &|_| 0.0,
                );
                let taken = f.ticks.iter().position(|t| t.2.is_some()).expect("taken");
                // First sight is as before: sensed at the first think (1 s),
                // taken at the first tick after his noticing delay (300 ms)
                // on the tick clock, whatever the lead.
                assert_eq!(f.ticks[taken].0, 1_300, "gap {gap}, lead {lead}");
                assert!(
                    f.ticks[taken..].iter().all(|t| t.2 == Some(1) && t.3),
                    "gap {gap}, lead {lead}: lost {:?}",
                    f.lost_at(f.ticks[taken].0)
                );
                assert!(f.lapses.is_empty());
            }
        }
    }

    /// A target that steps behind a wall is lost [`SIGHT_GRACE_MS`] after the
    /// look that failed, on the information clock: not before, and not later
    /// than the next tick.
    #[test]
    fn a_target_behind_a_wall_is_lost_after_the_grace() {
        use crate::v026g::bots::know::SIGHT_GRACE_MS;
        let world = world_of(&[wall(10.0, 1.0, 8.0, 0.0, 3.0)]);
        let hide = 3_000;
        let f = fight(
            &world,
            300,
            100,
            6_000,
            None,
            &|clock| {
                at(Vec3::new(if clock < hide { 0.0 } else { 4.0 }, 0.0, 20.0))
            },
            &|_| 0.0,
        );
        let (now, clock) = f.lost_at(hide).expect("lost");
        // The failed look is the first one after he hid, a look every 100 ms
        // (every other tick): the clock it was read on is within 100 ms.
        assert!(clock > hide + SIGHT_GRACE_MS, "lost at {clock}");
        assert!(clock <= hide + 100 + SIGHT_GRACE_MS + 50, "lost at {clock} (tick {now})");
        assert_eq!(f.lapses, vec![now]);
        // Until then he was seen at every tick.
        assert!(f.ticks.iter().take_while(|t| t.0 < now).skip(30).all(|t| t.3));
    }

    /// A target reckoned behind a wall for a moment (a late input, then the
    /// snap back) stays seen while the moment is inside the grace, and is
    /// lost when it is longer: the numbers behind the ping bands.
    #[test]
    fn a_snap_inside_the_grace_keeps_the_target_and_a_longer_one_loses_it() {
        let world = world_of(&[wall(10.0, 1.0, 8.0, 0.0, 3.0)]);
        let overshoot = |ms: u32| {
            fight(
                &world,
                300,
                100,
                6_000,
                None,
                &|clock| {
                    let behind = (3_000..3_000 + ms).contains(&clock);
                    at(Vec3::new(if behind { 4.0 } else { 0.0 }, 0.0, 20.0))
                },
                &|_| 0.0,
            )
        };
        // An overshoot of up to 200 ms: a look finds him back in the grace.
        for ms in [50, 100, 150, 200] {
            let f = overshoot(ms);
            assert_eq!(f.lost_at(3_000), None, "{ms} ms");
        }
        // 300 ms of false-blocked picture: lost. Here the picture is moved by
        // hand; what a late input does to it, and the discard of one past the
        // relay's reach, is `a_players_late_stop_input_...` (a lateness of 300
        // ms is a ping of about 670: the live server shows no such snap).
        let f = overshoot(300);
        assert!(f.lost_at(3_000).is_some());
        assert_eq!(f.lapses.len(), 1);
    }

    /// A bot that turns its back on its target loses him after the grace,
    /// not at once; a glance away shorter than that does not.
    #[test]
    fn a_target_out_of_the_view_is_lost_after_the_grace() {
        let world = world_of(&[]);
        let turned = |ms: u32| {
            fight(
                &world,
                300,
                100,
                6_000,
                None,
                &|_| at(Vec3::new(0.0, 0.0, 20.0)),
                &|clock| {
                    if (3_000..3_000 + ms).contains(&clock) {
                        std::f32::consts::PI
                    } else {
                        0.0
                    }
                },
            )
        };
        assert_eq!(turned(100).lost_at(3_000), None);
        let f = turned(1_000);
        let (_, clock) = f.lost_at(3_000).expect("lost");
        let grace = crate::v026g::bots::know::SIGHT_GRACE_MS;
        assert!(clock > 3_000 + grace && clock <= 3_000 + grace + 50, "{clock}");
    }

    /// A dead target is lost at once, no grace; after he spawns again he
    /// is a new sight, noticed anew, not the old one carried over.
    #[test]
    fn a_dead_target_ends_the_sight_and_a_respawn_does_not_carry_it_over() {
        let world = world_of(&[]);
        let f = fight(
            &world,
            300,
            100,
            7_000,
            None,
            &|clock| Enemy {
                feet: Vec3::new(0.0, 0.0, 20.0),
                alive: !(3_000..3_100).contains(&clock),
                // Back at 3.1 s, a new life.
                spawned: if clock >= 3_100 { 3_100 } else { 0 },
            },
            &|_| 0.0,
        );
        let (now, clock) = f.lost_at(3_000).expect("lost");
        assert!(clock <= 3_000 + 50, "at once, not after the grace: {clock}");
        assert_eq!(now, clock + 100);
        // The new life is taken again only after the next think senses
        // him and his noticing delay is out: a few hundred ms.
        let back = f.ticks.iter().find(|t| t.0 > now && t.2.is_some()).expect("taken again");
        assert!(back.0 >= 3_100 + 300, "noticed anew: {}", back.0);
    }

    /// A rare stall of the server (1 s between polls): a clear target is
    /// not dropped by it, and one that hid during it is not kept past the
    /// bound for untested time.
    #[test]
    fn a_second_long_stall_neither_drops_a_clear_target_nor_keeps_a_hidden_one() {
        let world = world_of(&[wall(10.0, 1.0, 8.0, 0.0, 3.0)]);
        let clear = fight(
            &world,
            300,
            100,
            7_000,
            Some((3_000, 1_000)),
            &|_| at(Vec3::new(0.0, 0.0, 20.0)),
            &|_| 0.0,
        );
        let taken = clear.ticks.iter().position(|t| t.2.is_some()).expect("taken");
        assert!(clear.ticks[taken..].iter().all(|t| t.2 == Some(1) && t.3));
        assert!(clear.lapses.is_empty());
        // He steps behind the wall in the stall: the first look after it
        // finds him gone, and the sight (450 ms untested at the most) is over
        // with that tick.
        let hidden = fight(
            &world,
            300,
            100,
            7_000,
            Some((3_000, 1_000)),
            &|clock| at(Vec3::new(if clock > 3_000 { 4.0 } else { 0.0 }, 0.0, 20.0)),
            &|_| 0.0,
        );
        let (_, clock) = hidden.lost_at(3_000).expect("lost");
        assert_eq!(clock, 4_000, "at the first tick after the stall");
    }

    /// A player walking at +z along x = 4 from `z = 6` at 1.05 s, who lets go
    /// of the key at the tick `stop`: the real [`Tracker`] is given his
    /// inputs `late` ms after their ticks and reckons him on as the bots'
    /// view does. Returns his place at the information clock, and the
    /// tracker's place count's feet where the stop puts him at rest.
    fn walker_through_the_tracker(
        world: &Arc<World>,
        stop: u32,
        late: u32,
    ) -> impl Fn(u32) -> Enemy {
        use crate::v026g::bots::body::Fix;
        use crate::v026g::bots::kin::Kinematics;
        use crate::v026g::bots::view::{Clock, Tracker};
        let kin = Kinematics::default();
        let mut tracker = Tracker::new(2, &[0, 1], &[true, false], vec![]);
        tracker.entered(1, true);
        let start = Fix {
            time: 1_000,
            alive: true,
            inserted: 10,
            feet: Some(Vec3::new(4.0, 0.0, 6.0)),
            yaw: 0.0,
            pitch: 0.0,
            keys: 0,
            speed: Some(0.0),
        };
        tracker.fixed(1, start, None, &kin, Some(world));
        // (arrival, tick, keys): a client sends an input at every tick; he
        // walks from 1.05 s and lets go of the key at `stop`.
        let mut sent: Vec<(u32, u32, u32)> = (1_050..stop)
            .step_by(50)
            .map(|tick| (tick + late, tick, key::FORWARD))
            .collect();
        sent.push((stop + late, stop, 0));
        let inputs = std::cell::RefCell::new(sent);
        let tracker = std::cell::RefCell::new(tracker);
        let world = world.clone();
        move |clock| {
            let mut tracker = tracker.borrow_mut();
            let mut inputs = inputs.borrow_mut();
            while let Some(&(arrival, tick, keys)) = inputs.first() {
                if arrival > clock {
                    break;
                }
                inputs.remove(0);
                tracker.input(1, tick, keys, (0.0, 0.0), 0.0, &kin, Some(&world));
            }
            let clock = Clock {
                clock,
                time_left_s: 600.0,
                total_s: 900.0,
                playing: true,
            };
            let view = tracker.view(&clock, &kin, Some(&world));
            let body = view.place(1).unwrap();
            Enemy {
                feet: body.feet,
                alive: body.alive,
                spawned: body.spawned,
            }
        }
    }

    /// Where a walker who stops at the tick 4 s rests, with his input on
    /// time.
    fn rest_z(stop: u32) -> f32 {
        let world = world_of(&[]);
        let at = walker_through_the_tracker(&world, stop, 0);
        at(8_000).feet.z
    }

    /// A player stops at the edge of cover, his stop input `late` ms late
    /// (the real [`Tracker`] reckons him on past the edge until it comes),
    /// at two phases of the 100 ms looks: the target stays seen through
    /// 300 ms (the false-blocked run is `late` less 100 ms, and three failed
    /// looks are needed to lose a sight past the 250 ms grace), is lost at
    /// 400 by phase, and at 1000 always. The relay discards an input later
    /// than its reach, 425 ms less the largest round trip of the other
    /// players: the longer rows are what the live server never shows
    /// (`tests/sight.rs` takes the intake).
    #[test]
    fn a_players_late_stop_input_keeps_the_target_to_300_ms_and_loses_him_by_400() {
        let lost = |stop: u32, late: u32| {
            let rest = rest_z(stop);
            // The wall's face is where he rests: any run-on is behind it.
            let world = world_of(&[wall(rest + 0.001, 3.0, 8.0, 0.0, 3.0)]);
            let enemy = walker_through_the_tracker(&world, stop, late);
            let f = fight(&world, 300, 100, 9_000, None, &enemy, &|_| 0.0);
            // He was taken long before the stop, and at rest is seen.
            let taken = f.ticks.iter().position(|t| t.2.is_some()).expect("taken");
            assert!(f.ticks[taken].0 < stop, "taken at {}", f.ticks[taken].0);
            f.lost_at(stop - 100)
        };
        for late in [0, 50, 100, 150, 200, 250, 300] {
            for stop in [4_000, 4_050] {
                assert_eq!(lost(stop, late), None, "late {late}, stop {stop}");
            }
        }
        assert!(lost(4_000, 400).is_some() || lost(4_050, 400).is_some());
        assert!(lost(4_000, 1_000).is_some() && lost(4_050, 1_000).is_some());
    }
}

//! Patch 76: a dropped battery falls and lies as the ragdolls do
//! (`docs/notes/ragdoll.md`): drawn by this client's own solver
//! (`ragdoll::dropped`), on the corpses and on the level, never under a
//! floor. The simulation's battery stays where its own physics has it (its
//! transform, body, markers, the bots' use of it): only the two model
//! matrices of its draw are ours ([`on_draw_primary`], [`on_draw_secondary`]).
//!
//! So a player takes the battery it sees: Use on the drawn battery
//! ([`aim`], each frame) goes out as a command naming it
//! (`weapon_pickup::Command::Battery`, [`on_input_end`]), and every world
//! plays the command where an input is applied ([`on_filter`]): the game's
//! own use of that battery, if the game lets that player start it and the
//! player is within the command's reach of the simulation's battery
//! (`weapon_pickup::battery_reach`). The game's own scan still takes a
//! battery it finds (the bots'); the local player's Use on one it does not
//! see there is not sent.

#![forbid(unsafe_code)]

use ::ragdoll::body::Capsule;
use ::ragdoll::corpse::Corpses;
use ::ragdoll::dropped::{Dropped, DroppedShape};
use ::ragdoll::math::{Pose, Vec3};
use ::ragdoll::ragdoll::Level;
use weapon_pickup::{BATTERIES, CHORD, Command, USE, battery_reach};

use crate::ragdoll::{matrix_of, pose_of};
use crate::use_scan::rule::Found;
use crate::vostok::battery::{self as native, Batteries, Battery, BatteryId, Pass};
use crate::vostok::world::{Player, WorldKey};
use crate::vostok::{ActionMask, Context, Flow, GameCell, InputHandler, Lock, NoExit, Thread};

/// The battery's capsule in its model's space: 0.6 m long, 0.24 m thick, on
/// its base (the model's origin).
const SHAPE: DroppedShape =
    DroppedShape { centre: Vec3::new(0.0, 0.12, 0.0), capsule: Capsule { half: 0.18, radius: 0.12 }, mass: 6.0 };
/// The half sizes of the box Use meets, about the same centre.
const USE_BOX: [f32; 3] = [0.30, 0.12, 0.16];
/// The walker collision a look at a battery must not cross (ray_test's
/// group and mask: the foot IK's level), up to this short of the battery, m:
/// the walker mesh lies some centimetres off the ground the battery rests on
/// (Rudnya: 10 cm at the 90th percentile), and a battery one sees must never
/// be refused for it.
const WALL: (u32, u32) = (0x24, 0x02);
const WALL_SHORT: f32 = 0.3;

/// A loose battery as this client draws it.
#[derive(Clone, Copy)]
struct Fall {
    battery: BatteryId,
    dropped: Dropped,
}

/// The batteries drawn: none (no match), or those of a world.
enum Scene {
    Away,
    In { world: WorldKey, falls: [Option<Fall>; BATTERIES as usize] },
}

static SCENE: Lock<Scene> = Lock::new(Scene::Away);

fn place(transform: &[f32; 16]) -> [f32; 3] {
    [transform[12], transform[13], transform[14]]
}

/// Patch 76's step, after the corpses': each loose battery falls on. One
/// that is loose anew (dropped, another world's, a rollback's) or strayed
/// past its tether starts from where the simulation has it.
pub fn step(thread: &Thread, dt: f32, batteries: Option<&Batteries>, corpses: &Corpses, level: &mut impl Level) {
    SCENE.with(thread, |scene| {
        let Some(batteries) = batteries else {
            *scene = Scene::Away;
            return;
        };
        if !matches!(scene, Scene::In { world, .. } if *world == batteries.world) {
            *scene = Scene::In { world: batteries.world, falls: [None; BATTERIES as usize] };
        }
        let Scene::In { falls, .. } = scene else { return };
        for (fall, loose) in falls.iter_mut().zip(batteries.loose) {
            let kept = fall.filter(|f| {
                loose.is_some_and(|l| {
                    let drawn = f.dropped.model().position;
                    f.battery == l.id && battery_reach::tethered([drawn.x, drawn.y, drawn.z], place(&l.transform))
                })
            });
            let begun = || {
                let loose = loose?;
                Some(Fall { battery: loose.id, dropped: Dropped::new(pose_of(&loose.transform)?, SHAPE) })
            };
            *fall = kept.or_else(begun).map(|mut f| {
                f.dropped.step(dt, Vec3::Y, corpses, level);
                f
            });
        }
    });
}

/// Each battery's drawn model, by its index.
fn drawn(thread: &Thread) -> [Option<(BatteryId, Pose)>; BATTERIES as usize] {
    SCENE.with(thread, |scene| match scene {
        Scene::Away => [None; BATTERIES as usize],
        Scene::In { falls, .. } => falls.map(|fall| fall.map(|f| (f.battery, f.dropped.model()))),
    })
}

pub fn on_draw_primary(ctx: &mut Context<'_>) -> Flow<NoExit> {
    draw(ctx, Pass::Primary)
}

pub fn on_draw_secondary(ctx: &mut Context<'_>) -> Flow<NoExit> {
    draw(ctx, Pass::Secondary)
}

fn draw(ctx: &Context<'_>, pass: Pass) -> Flow<NoExit> {
    if let Some(mut draw) = native::draw(ctx, pass)
        && let Some((_, pose)) = drawn(ctx.thread()).into_iter().flatten().find(|(id, _)| *id == draw.battery)
    {
        draw.pose(matrix_of(pose));
    }
    Flow::Continue
}

/// How far along a look from `eye` (unit `direction`, `reach` long) the
/// battery drawn at `model` is met: its use box, no padding.
fn met(model: Pose, eye: Vec3, direction: Vec3, reach: f32) -> Option<f32> {
    let into = model.inverse();
    let start = into.apply(eye) - SHAPE.centre;
    let ray = into.rotation.rotate(direction);
    let (mut near, mut far) = (0.0f32, reach);
    for (p, d, half) in [(start.x, ray.x, USE_BOX[0]), (start.y, ray.y, USE_BOX[1]), (start.z, ray.z, USE_BOX[2])] {
        if d.abs() < 1e-6 {
            if p.abs() > half {
                return None;
            }
            continue;
        }
        let (a, b) = ((-half - p) / d, (half - p) / d);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
        if near > far {
            return None;
        }
    }
    Some(near)
}

/// What the local player's Use is for, as to batteries, this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Aim {
    /// No battery: Use is whoever else's.
    Elsewhere,
    /// The drawn battery looked at, which the player may take.
    Take(weapon_pickup::Battery),
    /// The game's own scan would take a battery where the simulation has
    /// it, one drawn elsewhere: Use takes nothing.
    Unseen,
}

static AIM: GameCell<Aim> = GameCell::new(Aim::Elsewhere);

/// The frame's aim (from artefact_carry's on_hud): the nearest drawn
/// battery the viewer looks at within the game's reach, with no wall
/// before it, that every world would let it take. True when there is one
/// (the hud offers it).
pub fn aim(ctx: &Context<'_>, viewer: Option<&Player<'_>>) -> bool {
    let thread = ctx.thread();
    let aim = viewer.map_or(Aim::Elsewhere, |viewer| aim_of(viewer, thread));
    AIM.set(thread, aim);
    matches!(aim, Aim::Take(_))
}

fn aim_of(viewer: &Player<'_>, thread: &Thread) -> Aim {
    let falls = drawn(thread);
    if !viewer.alive() || !viewer.inserted() || falls.iter().all(Option::is_none) {
        return Aim::Elsewhere;
    }
    let (eye, forward) = viewer.carry_eye();
    let Some(direction) = Vec3::new(forward[0], forward[1], forward[2]).direction() else { return Aim::Elsewhere };
    let eye_at = Vec3::new(eye[0], eye[1], eye[2]);
    let reach = viewer.use_distance();
    let nearest = falls
        .into_iter()
        .enumerate()
        .filter_map(|(index, fall)| {
            let (id, model) = fall?;
            let battery = weapon_pickup::Battery::new(index as u8)?;
            let found = Battery::of(viewer, battery)?;
            let loose = found.loose().filter(|l| l.id == id)?;
            if !found.free_for(viewer) || !battery_reach::within(place(&viewer.transform()), place(&loose.transform)) {
                return None;
            }
            let distance = met(model, eye_at, direction, reach)?;
            let clear = distance - WALL_SHORT;
            let walled = clear > 0.0
                && viewer.physics_world().is_some_and(|w| w.ray_test(eye, forward, clear, WALL.0, WALL.1).is_some());
            (!walled).then_some((distance, battery))
        })
        .reduce(|a, b| if a.0 <= b.0 { a } else { b });
    match nearest {
        Some((_, battery)) => Aim::Take(battery),
        None => match crate::use_scan::detect(viewer) {
            Some(Found::Usable(spot))
                if spot.object.battery().is_some_and(|id| falls.iter().flatten().any(|&(drawn, _)| drawn == id)) =>
            {
                Aim::Unseen
            }
            _ => Aim::Elsewhere,
        },
    }
}

/// Use at the input builder: up, held (the game's, or another patch's
/// command), or a press taken here, held still (no Use until it is let go).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UseKey {
    Up,
    Held,
    Taken,
}

static USE_KEY: GameCell<UseKey> = GameCell::new(UseKey::Up);

/// One input's Use: on its down edge, in an input with no command yet, a
/// drawn battery aimed at makes the input its pickup command, and an unseen
/// one takes the Use out; either way the press sends no Use until let go.
fn press(key: UseKey, aim: Aim, actions: u32) -> (UseKey, u32) {
    if actions & USE == 0 {
        return (UseKey::Up, actions);
    }
    match (key, aim) {
        (UseKey::Taken, _) => (UseKey::Taken, actions & !USE),
        (UseKey::Held, _) => (UseKey::Held, actions),
        (UseKey::Up, Aim::Take(battery)) if actions & CHORD == 0 => {
            (UseKey::Taken, Command::Battery(battery).encode(actions))
        }
        (UseKey::Up, Aim::Unseen) => (UseKey::Taken, actions & !USE),
        (UseKey::Up, _) => (UseKey::Held, actions),
    }
}

/// The input builder's end (composed from artefact_carry's on_input_end,
/// after patches 78 and 85).
pub fn on_input_end(handler: &mut InputHandler<'_>, thread: &Thread) {
    let actions = handler.actions().bits();
    // a press patch 85 made a gun's pickup has no Use left: it stays up here
    let (key, sent) = press(USE_KEY.get(thread), AIM.get(thread), actions);
    USE_KEY.set(thread, key);
    if sent != actions {
        handler.set_actions(ActionMask::from_bits(sent));
    }
}

/// The input's apply point (composed from artefact_carry's `carry.filter`,
/// before patch 78 filters the actions), in every world: a battery's pickup
/// command is the game's own use of that battery by this player, if the
/// game lets it start and the player is within the command's reach of the
/// battery as the simulation has it. Refused otherwise, the same in every
/// world: nothing here reads what a client draws.
pub fn on_filter(player: &Player<'_>) {
    if let Some(Command::Battery(battery)) = Command::of(player.input_actions())
        && let Some(found) = Battery::of(player, battery)
        && let Some(loose) = found.loose()
        && found.free_for(player)
        && battery_reach::within(place(&player.transform()), place(&loose.transform))
    {
        found.take(player);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::ragdoll::math::Quat;

    #[test]
    fn one_command_a_press_and_the_games_use_keeps_its_press() {
        let battery = weapon_pickup::Battery::new(2).unwrap();
        let pressed = USE | 0x20;
        let (key, sent) = press(UseKey::Up, Aim::Take(battery), pressed);
        assert_eq!(key, UseKey::Taken);
        assert_eq!(Command::of(sent), Some(Command::Battery(battery)));
        assert_eq!(sent & (USE | 0x20), 0x20);
        assert_eq!(press(key, Aim::Take(battery), pressed), (UseKey::Taken, 0x20), "held: no second command");
        assert_eq!(press(key, Aim::Elsewhere, 0x20), (UseKey::Up, 0x20));
        assert_eq!(press(UseKey::Up, Aim::Elsewhere, pressed), (UseKey::Held, pressed), "the game's own Use");
        assert_eq!(press(UseKey::Held, Aim::Take(battery), pressed), (UseKey::Held, pressed), "not mid-press");
        assert_eq!(press(UseKey::Up, Aim::Unseen, pressed), (UseKey::Taken, 0x20), "no Use on a battery not seen");
        // patch 85's pickup in this input: its command stays
        let gun = weapon_pickup::encode(pressed, weapon_pickup::Place::new(1).unwrap());
        assert_eq!(press(UseKey::Up, Aim::Take(battery), gun), (UseKey::Up, gun));
        let carry = pressed | CHORD | 1 << 14;
        assert_eq!(press(UseKey::Up, Aim::Take(battery), carry), (UseKey::Held, carry), "patch 78's command");
    }

    #[test]
    fn a_look_meets_the_drawn_battery_within_reach_only() {
        let eye = Vec3::new(0.0, 0.12, 0.0);
        let near = Pose::new(Quat::IDENTITY, Vec3::new(0.0, 0.0, 1.0));
        let distance = met(near, eye, Vec3::Z, 2.0).unwrap();
        assert!((distance - (1.0 - USE_BOX[2])).abs() < 1e-4, "{distance}");
        assert_eq!(met(Pose::new(Quat::IDENTITY, Vec3::new(0.0, 0.0, 3.0)), eye, Vec3::Z, 2.0), None, "too far");
        assert_eq!(met(Pose::new(Quat::IDENTITY, Vec3::new(1.0, 0.0, 1.0)), eye, Vec3::Z, 2.0), None, "beside");
        assert_eq!(met(near, eye, -Vec3::Z, 2.0), None, "behind");
        // turned a quarter: its long side toward the eye
        let turned = Pose::new(Quat::from_axis_angle(Vec3::Y, core::f32::consts::FRAC_PI_2), near.position);
        let distance = met(turned, eye, Vec3::Z, 2.0).unwrap();
        assert!((distance - (1.0 - USE_BOX[0])).abs() < 1e-4, "{distance}");
    }
}

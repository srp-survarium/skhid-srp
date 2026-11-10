//! Local battery flight against corpse capsules. Only an id crosses the
//! input stream: native ownership, use state and delivery remain shared.
#![forbid(unsafe_code)]
use ragdoll::{body::{Body, Capsule}, corpse::{Corpses, DeathKey, MAX_PLACES}, math::{Pose, Vec3},
              ragdoll::{CorpseContacts, Level, LooseItem}};
use weapon_pickup::battery::{self as command, Id, COUNT};
use crate::vostok::{battery as native, ActionMask, Context, Flow, GameCell, InputHandler, Lock, NoExit, Thread};
use crate::vostok::world::{Player, WorldKey};
use crate::ragdoll::{matrix_of, pose_of};

#[derive(Clone, Copy)]
struct Flight {
    object: native::Object,
    item: LooseItem,
    contacts: [Option<(DeathKey, CorpseContacts)>; MAX_PLACES],
}
#[derive(Clone, Copy)]
enum State { Absent, Free(Flight) }
enum Scene { Away, InWorld { world: WorldKey, time: weapon_pickup::Ms, states: [State; COUNT] } }
static SCENE: Lock<Scene> = Lock::new(Scene::Away);
#[derive(Clone, Copy)]
enum Aim { None, Take(Id), SuppressNative }
static AIM: GameCell<Aim> = GameCell::new(Aim::None);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key { Up, Native, Taken }
static KEY: GameCell<Key> = GameCell::new(Key::Up);

pub fn step(thread: &Thread, dt: f32, frame: Option<&native::Frame>, corpses: &Corpses, level: &mut impl Level) {
    SCENE.with(thread, |scene| {
        let Some(frame) = frame else { *scene = Scene::Away; return };
        if !matches!(scene, Scene::InWorld { world, time, .. } if *world == frame.world && frame.time.since(*time) <= i32::MAX as u32) {
            *scene = Scene::InWorld { world: frame.world, time: frame.time, states: [State::Absent; COUNT] };
        }
        let Scene::InWorld { time, states, .. } = scene else { return };
        *time = frame.time;
        for (state, free) in states.iter_mut().zip(frame.items) {
            let Some(free) = free else { *state = State::Absent; continue };
            let mut flight = match *state {
                State::Free(f) if f.object == free.object => f,
                _ => {
                    let Some(model) = pose_of(&free.transform) else { *state = State::Absent; continue };
                    let centre = Pose::new(model.rotation, model.apply(Vec3::new(0.0, 0.12, 0.0)));
                    let body = Body::new(centre, Capsule { half: 0.18, radius: 0.12 }, 6.0);
                    Flight { object: free.object, item: LooseItem::new(body, centre.inverse().then(model)),
                             contacts: [None; MAX_PLACES] }
                }
            };
            // Re-evaluate a resting item's support: a corpse can move after
            // a bullet hit or disappear at respawn. Rest is never permanent.
            let _ = flight.item.step_with_contacts(dt, Vec3::Y, level, |body| {
                let mut present = [false; MAX_PLACES];
                for (place, death, corpse) in corpses.colliders() {
                    present[place] = true;
                    let parts = corpse.collision_bodies();
                    let contacts = &mut flight.contacts[place];
                    if !matches!(contacts, Some((d, _)) if *d == death) {
                        *contacts = Some((death, CorpseContacts::released(body, parts)));
                    }
                    if let Some((_, contacts)) = contacts { contacts.solve(body, parts, Vec3::Y); }
                }
                for (contact, present) in flight.contacts.iter_mut().zip(present) {
                    if !present { *contact = None; }
                }
            });
            *state = State::Free(flight);
        }
    });
}

fn poses(thread: &Thread) -> [Option<(native::Object, Pose)>; COUNT] {
    SCENE.with(thread, |s| match s {
        Scene::Away => [None; COUNT],
        Scene::InWorld { states, .. } => states.map(|state| match state {
            State::Free(f) => Some((f.object, f.item.pose())), State::Absent => None,
        }),
    })
}

pub fn on_draw_primary(ctx: &mut Context<'_>) -> Flow<NoExit> { draw(ctx, native::Pass::Primary) }
pub fn on_draw_secondary(ctx: &mut Context<'_>) -> Flow<NoExit> { draw(ctx, native::Pass::Secondary) }
fn draw(ctx: &Context<'_>, pass: native::Pass) -> Flow<NoExit> {
    if let Some(mut drawn) = native::draw(ctx, pass)
        && let Some((_, pose)) = poses(ctx.thread()).into_iter().flatten().find(|(id, _)| *id == drawn.item) {
        drawn.pose(matrix_of(pose));
    }
    Flow::Continue
}

pub fn reset(thread: &Thread) {
    SCENE.with(thread, |s| *s = Scene::Away);
    AIM.set(thread, Aim::None);
    KEY.set(thread, Key::Up);
}

/// Local view ray against the stock battery's oriented use box: no padding,
/// the normal two-metre reach. The level ray still checks walls/floors.
fn enters(pose: Pose, eye: Vec3, direction: Vec3, reach: f32) -> Option<f32> {
    let inverse = pose.inverse();
    let start = inverse.apply(eye) - Vec3::new(0.0, 0.12, 0.0);
    let ray = inverse.rotation.rotate(direction);
    let mut near = 0.0f32;
    let mut far = reach;
    for (p, d, half) in [(start.x, ray.x, 0.3), (start.y, ray.y, 0.12), (start.z, ray.z, 0.16)] {
        if d.abs() < 1e-6 { if p.abs() > half { return None; } }
        else {
            let (a,b) = ((-half-p)/d, (half-p)/d);
            near = near.max(a.min(b)); far = far.min(a.max(b));
            if near > far { return None; }
        }
    }
    Some(near)
}

pub fn aim(ctx: &Context<'_>, viewer: Option<&Player<'_>>) -> bool {
    let thread = ctx.thread();
    let Some(viewer) = viewer.filter(|p| p.alive() && p.inserted()) else { AIM.set(thread, Aim::None); return false };
    let drawn = poses(thread);
    if drawn.iter().all(Option::is_none) { AIM.set(thread, Aim::None); return false; }
    let Some(physics) = viewer.physics_world() else { AIM.set(thread, Aim::None); return false };
    let (eye, forward) = viewer.carry_eye();
    let eye_v = Vec3::new(eye[0],eye[1],eye[2]);
    let Some(direction) = Vec3::new(forward[0],forward[1],forward[2]).direction() else { AIM.set(thread, Aim::None); return false };
    let mut nearest: Option<(f32, Id)> = None;
    for (i, pose) in drawn.into_iter().enumerate() {
        let Some((object, pose)) = pose else { continue };
        let id = Id::new(i).unwrap();
        let Some(battery) = native::for_player(viewer, id) else { continue };
        if !battery.can_take(viewer) || battery.free().is_none_or(|f| f.object != object) { continue; }
        // Patch 23's floor rescue remains available at the drawn position:
        // below the feet, within one metre across, along the four-metre ray.
        let feet = viewer.transform();
        let (dx, dz) = (pose.position.x-feet[12], pose.position.z-feet[14]);
        let across = dx*dx + dz*dz;
        let below = pose.position.y < feet[13] && across <= 1.0;
        let Some(distance) = enters(pose, eye_v, direction, if below { 4.0 } else { 2.0 }) else { continue };
        if !below && distance > 0.01 && physics.ray_test(eye, forward, distance, 0x24, 2).is_some() { continue; }
        if nearest.is_none_or(|(best,_)| distance < best) { nearest = Some((distance,id)); }
    }
    let aim = nearest.map_or_else(|| if native::native_target_is_battery(viewer) { Aim::SuppressNative } else { Aim::None },
                                  |(_,id)| Aim::Take(id));
    AIM.set(thread, aim);
    matches!(aim, Aim::Take(_))
}

pub fn on_input_end(handler: &mut InputHandler<'_>, thread: &Thread) {
    let actions = handler.actions().bits();
    let (key, sent) = key_transition(KEY.get(thread), AIM.get(thread), actions);
    KEY.set(thread,key);
    handler.set_actions(ActionMask::from_bits(sent));
}

fn key_transition(key: Key, aim: Aim, actions: u32) -> (Key, u32) {
    if actions & weapon_pickup::USE == 0 { (Key::Up, actions) }
        else { match key {
            Key::Taken => (Key::Taken, actions & !weapon_pickup::USE),
            Key::Native => (Key::Native, actions),
            Key::Up => match aim {
                Aim::Take(id) if actions & weapon_pickup::CHORD == 0 => (Key::Taken, command::encode(actions,id)),
                Aim::SuppressNative => (Key::Taken, actions & !weapon_pickup::USE),
                _ => (Key::Native, actions),
            }
        }}
}

pub fn on_filter(player: &Player<'_>, _thread: &Thread) {
    if let Some(id) = command::command(player.input_actions())
        && let Some(battery) = native::for_player(player,id) { battery.take(player); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_pickup_per_press_and_native_use_keeps_its_press() {
        let id = Id::new(2).unwrap();
        let pressed = weapon_pickup::USE | 0x20;
        let (key, sent) = key_transition(Key::Up, Aim::Take(id), pressed);
        assert_eq!(key, Key::Taken);
        assert_eq!(command::command(sent), Some(id));
        assert_eq!(key_transition(key, Aim::Take(id), pressed), (Key::Taken, 0x20));
        assert_eq!(key_transition(key, Aim::None, 0x20), (Key::Up, 0x20));
        assert_eq!(key_transition(Key::Up, Aim::None, pressed), (Key::Native, pressed));
        assert_eq!(key_transition(Key::Native, Aim::Take(id), pressed), (Key::Native, pressed));
        assert_eq!(key_transition(Key::Up, Aim::SuppressNative, pressed), (Key::Taken, 0x20));
        let gun = weapon_pickup::encode(pressed, weapon_pickup::Place::new(1).unwrap());
        assert_eq!(key_transition(Key::Up, Aim::Take(id), gun), (Key::Up, gun));
    }
    #[test]
    fn local_pickup_checks_the_drawn_position_and_reach() {
        let eye = Vec3::new(0.0, 0.12, 0.0);
        let near = Pose::new(ragdoll::math::Quat::IDENTITY, Vec3::new(0.0, 0.0, 1.0));
        assert!(enters(near, eye, Vec3::Z, 2.0).is_some());
        let far = Pose::new(near.rotation, Vec3::new(0.0, 0.0, 3.0));
        assert_eq!(enters(far, eye, Vec3::Z, 2.0), None);
        let off = Pose::new(near.rotation, Vec3::new(1.0, 0.0, 1.0));
        assert_eq!(enters(off, eye, Vec3::Z, 2.0), None);
        assert_eq!(enters(near, eye, -Vec3::Z, 2.0), None);
        // The selected id is the command: another replica's far model
        // cannot turn that valid id into a different pickup or a refusal.
        let id = Id::new(3).unwrap();
        assert_eq!(command::command(command::encode(weapon_pickup::USE,id)), Some(id));
    }
}

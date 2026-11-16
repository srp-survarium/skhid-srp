//! A step's ground: `step_manager::on_step` (0x248AD0) at `step.ground`,
//! after its ground ray (patch 153, docs/notes/footsteps.md), and a step
//! another patch plays through on_step itself ([`play`]).
//!
//! on_step casts one ray from 1 m above the foot straight down, 2 m long,
//! with the filter of what bullets hit (group 30h, mask 8). It plays the pair
//! of the foot's material and the material of the triangle the ray met: the
//! sound at the hit point (the player's own step on the hud), the pair's
//! decal and particles. A ray that met nothing is a silent step, and the
//! walker collision (the shapes the body stands on, which bullets go
//! through) is never met. The site's view: the foot, its material and view,
//! the game's ray, and looks of our own with the game's ray_test (0x75E20);
//! then on_step goes on from its own result or from a ground given here
//! ([`OnStep::go_on`]).
//!
//! Nothing here is read by the simulation: on_step's frame, a sound, a decal,
//! particles (the note's section on it).

use super::context::{Context, Flow, Reg};
use super::contact::{self, Touch};
use super::hook::Thread;
use super::world::{floats, read, write_floats, Player};
use super::{GameCell, StepGroundOnGroundExit};
use crate::raw;

/// on_step's filter (0x248B14, 0x248B02): the collision bullets hit
/// (cgroup 8, cmask 10h or 20h).
const GAME_GROUP: u32 = 0x30;
const GAME_MASK: u32 = 0x08;
/// How far over `at` a look near a touch starts: the model's own collision
/// beside a walker face lies up to 3 cm over it (walk 3).
const LIFT: f32 = 0.10;
/// The drawn water surfaces (content pack water-steps, cgroup and cmask
/// 1000h): no other object, body or ray of the game has the bit either way
/// (the game's filters, ragdoll.md section 19's table), so this look alone
/// meets them.
const WATER_BIT: u32 = 0x1000;
/// ray_test's last argument as on_step passes it (0x248AED).
const UNFLIPPED: u32 = 1;
/// The ray's origin above the foot (0xA70884) and its length (0x9CC87C).
const ABOVE: f32 = 1.0;
const GAME_LENGTH: f32 = 2.0;
/// A played step's direction: only the pair's decal takes it, and no foot
/// pair has a decal.
const UP: [f32; 3] = [0.0, 1.0, 0.0];

/// A ground's game material id, as a triangle gives it
/// (get_triangle_material 0x3570A0: the shape's u16 & 3FFFh, 0FFFFh when the
/// shape has no table).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundMaterial(u16);

impl GroundMaterial {
    /// A material by its id (game.materials' `id`: metall_thin 17, ...).
    pub const fn of(id: u16) -> GroundMaterial {
        GroundMaterial(id)
    }

    pub const fn id(self) -> u16 {
        self.0
    }

    /// The triangle's face was given a material: not the cook's unknown
    /// name (0FFFFh, 3FFFh after the mask) nor a shape without a table.
    /// material_pair steps the others as `default` (its id 11).
    pub fn named(self) -> bool {
        !matches!(self.0, 0x3FFF | 0xFFFF)
    }
}

/// The foot's game material (on_step's ESP+8): the boots' for the logic
/// state's movement and the view, a pair master (actor 10, actor_local 42,
/// ...) or, in the on-site states, a word read before the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FootMaterial(u16);

impl FootMaterial {
    pub const fn id(self) -> u16 {
        self.0
    }
}

/// How the step is heard (on_step's view flag).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    /// The player's own step in first person: a sound on the hud (flag 0).
    First,
    /// Anyone else's: a sound at the point (flag 1).
    Third,
}

/// What a ray of the step met.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub material: GroundMaterial,
    /// Metres under the foot (negative: above it).
    pub depth: f32,
    /// Toward the ray's start.
    pub normal: [f32; 3],
}

/// A look of our own, from on_step's ray's origin (1 m above the foot)
/// straight down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    /// The drawn water surfaces alone ([`WATER_BIT`]), as far as on_step's
    /// ray reaches: water over the foot within 1 m.
    Water,
    /// on_step's filter from 10 cm over `at` (at or beside a touch of the
    /// controller's, the game's frame) to `reach` metres under it: the
    /// hittable surface near a touched face without a material. A hit's
    /// depth is under `at`.
    Under { at: [f32; 3], reach: f32 },
}

/// The stepping body as the step finds it: what its controller touched (the
/// step's ground after a miss) and its state for the step log.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// The player's simulated feet (player +43Ch), the game's frame.
    pub root: [f32; 3],
    /// The character controller's support normal (+2C710h).
    pub support: [f32; 3],
    /// Its velocity's y (+2C774h).
    pub rising: f32,
    /// Its +203h (on_ground answers false while set).
    pub jump_flag: bool,
    /// What its step-down sweep last touched ([`super::contact`]).
    pub touch: Option<Touch>,
}

impl Body {
    /// on_ground (0x7EF90): no jump flag, |velocity y| < 4, support y at
    /// least cos 50 deg.
    pub fn on_ground(&self) -> bool {
        !self.jump_flag && self.rising.abs() < 4.0 && self.support[1] >= 0.642_787_6
    }
}

/// The ground on_step goes on with, its sound at `at`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub material: GroundMaterial,
    pub at: [f32; 3],
    pub normal: [f32; 3],
}

/// How on_step goes on from its ground ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Next {
    /// From its own ray's result, a hit or a miss: the game as it is.
    Game,
    /// From this ground: its pair with the foot's material, the sound, decal
    /// and particles at its place.
    Ground(Placed),
}

/// What a pair of the foot's material and a ground plays (material_pair
/// 0x306D50, the pair's +20h: on_step's tests at 0x248B90 and 0x248BA3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairSound {
    /// No pair: the foot's material is no pair master.
    NoPair,
    /// A pair whose step sound is empty.
    Silent,
    Sound,
}

/// Whose step on_step plays, from its caller (its return address and the
/// caller's registers it pushed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Caller {
    /// on_foot_step (0x2A6520): a foot of the player whose logic state has
    /// this movement id (none: no state).
    Foot { own: bool, movement: Option<u32> },
    /// The landing (0x23FBE0): the run's material (movement 2) or the
    /// jump's (4).
    Landing { own: bool, movement: u32 },
    /// Anything else: [`play`].
    Other,
}

/// The ground [`play`] arms for the one on_step it calls; the hook takes it.
static ARMED: GameCell<Option<Placed>> = GameCell::new(None);

/// on_step at `step.ground`: its frame, alive while it runs, on its thread.
pub struct OnStep {
    manager: raw::StepManager,
    frame: raw::OnStepFrame,
    arguments: raw::OnStepArguments,
}

impl OnStep {
    pub fn at(ctx: &Context<'_>) -> Option<OnStep> {
        use raw::site::step_ground as site;
        // SAFETY: the context is this site's: on_step's manager (ESI), its
        // frame (ESP) and its arguments (EBP), alive while it runs.
        unsafe { Some(OnStep { manager: site::manager(ctx)?, frame: site::frame(ctx)?, arguments: site::arguments(ctx)? }) }
    }

    /// The foot: the ray's origin, 1 m under it.
    pub fn foot(&self) -> [f32; 3] {
        // SAFETY: on_step's frame; the origin is written before the ray.
        let [x, y, z]: [f32; 3] = unsafe { floats(self.frame.origin()) };
        [x, y - ABOVE, z]
    }

    pub fn foot_material(&self) -> FootMaterial {
        // SAFETY: on_step's argument.
        FootMaterial(unsafe { read(self.arguments.foot_material()) })
    }

    pub fn view(&self) -> View {
        // SAFETY: on_step's argument.
        match unsafe { read(self.arguments.view()) } {
            0 => View::First,
            _ => View::Third,
        }
    }

    /// How far under the foot on_step's own ray met something (negative:
    /// above it); None when it met nothing. Its material is not read.
    pub fn game_depth(&self) -> Option<f32> {
        // SAFETY: on_step's result, ray_test wrote it.
        unsafe {
            let result = self.frame.result();
            result.object()?;
            Some(read(result.fraction()) * GAME_LENGTH - ABOVE)
        }
    }

    /// What on_step's own ray met (the material as on_step reads it next).
    pub fn game_hit(&self, _: &Thread) -> Option<Hit> {
        // SAFETY: on_step's result; its object alive in the physics world.
        unsafe { hit(self.frame.result(), GAME_LENGTH, ABOVE) }
    }

    /// A look of our own (`look`), into a result of ours: on_step's own
    /// stays as the game's ray left it.
    pub fn look(&self, thread: &Thread, look: Look) -> Option<Hit> {
        match look {
            Look::Water => {
                // SAFETY: on_step's frame: its ray's origin.
                let origin = unsafe { self.frame.origin() };
                self.ray(thread, origin, WATER_BIT, WATER_BIT, GAME_LENGTH, ABOVE)
            }
            Look::Under { at: [x, y, z], reach } => {
                let origin = [x, y + LIFT, z];
                self.ray(thread, origin.as_ptr(), GAME_GROUP, GAME_MASK, LIFT + reach, LIFT)
            }
        }
    }

    /// A ray from `origin` (`above` metres over the point its depths are
    /// under) straight down, `length` long.
    fn ray(&self, thread: &Thread, origin: *const f32, group: u32, mask: u32, length: f32, above: f32) -> Option<Hit> {
        let physics = self.world(thread)?.pvp_match_physics()?;
        let mut room = raw::RayResultStorage::zeroed();
        let result = room.handle();
        // SAFETY: the physics world on_step ray-tests, alive for the step;
        // the result is ours; the origin and direction are on_step's (its
        // frame's float3s), as it calls ray_test.
        unsafe {
            raw::engine_ray_test(Some(physics), Some(result), origin, self.frame.direction(), length, group,
                                 mask, core::ptr::null(), UNFLIPPED);
            hit(result, length, above)
        }
    }

    /// The ground [`play`] armed for this on_step, taken.
    pub fn armed(&self, thread: &Thread) -> Option<Placed> {
        let armed = ARMED.get(thread);
        ARMED.set(thread, None);
        armed
    }

    /// What the pair of the foot's material and `ground` plays.
    pub fn pair(&self, thread: &Thread, ground: GroundMaterial) -> PairSound {
        let Some(materials) = self.world(thread).and_then(|world| world.pvp_match_materials()) else {
            return PairSound::NoPair;
        };
        // SAFETY: the game material manager on_step asks (game_world vtable
        // +1Ch gives it), its table read only; the pair is its record.
        unsafe {
            match raw::engine_material_pair(Some(materials), u32::from(self.foot_material().id()), u32::from(ground.id())) {
                None => PairSound::NoPair,
                Some(pair) if read(pair.step_sound()) == 0 => PairSound::Silent,
                Some(_) => PairSound::Sound,
            }
        }
    }

    /// The stepping player (a foot's or a landing's).
    fn stepper(&self) -> Option<raw::Player> {
        // SAFETY: as `caller`: the pushed registers read once the return
        // address says which caller pushed them.
        unsafe {
            match read(self.arguments.return_address()) {
                raw::image::FOOT_STEP_RETURN => self.frame.caller_effect().and_then(|effect| effect.player()),
                raw::image::LANDING_STEP_RETURN => self.frame.caller_player(),
                _ => None,
            }
        }
    }

    /// The stepping body: its root and its controller.
    pub fn body(&self, thread: &Thread) -> Option<Body> {
        let player = self.stepper()?;
        // SAFETY: the live stepping player and its character controller.
        unsafe {
            let controller = player.controller()?.controller()?;
            let velocity: [f32; 3] = floats(controller.velocity());
            Some(Body {
                root: floats(player.feet()),
                support: floats(controller.support_normal()),
                rising: velocity[1],
                jump_flag: read(controller.jump_flag()) != 0,
                touch: contact::of(thread, controller),
            })
        }
    }

    /// Whose step it is.
    pub fn caller(&self, thread: &Thread) -> Caller {
        let world = self.world(thread);
        let own = |player: Option<raw::Player>| {
            // SAFETY: the world's game and network client, alive for a match.
            let local = world.and_then(|world| unsafe { world.this.game()?.network_client()?.local_player() });
            player.is_some_and(|player| Some(player) == local)
        };
        // SAFETY: on_step's arguments and frame; the pushed registers read as
        // its caller's only once the return address says which caller.
        unsafe {
            match read(self.arguments.return_address()) {
                raw::image::FOOT_STEP_RETURN => {
                    let effect = self.frame.caller_effect();
                    let movement = effect.and_then(|effect| effect.logic()?.state()).map(|state| read(state.movement()));
                    Caller::Foot { own: own(effect.and_then(|effect| effect.player())), movement }
                }
                raw::image::LANDING_STEP_RETURN => {
                    Caller::Landing { own: own(self.frame.caller_player()), movement: read(self.frame.caller_esi()) }
                }
                _ => Caller::Other,
            }
        }
    }

    /// on_step goes on: from its own ray's result, or from `ground` (its
    /// place and normal written into the result, the material in EAX at the
    /// exit past the hit object's material).
    pub fn go_on(self, ctx: &mut Context<'_>, next: Next) -> Flow<StepGroundOnGroundExit> {
        match next {
            Next::Game => Flow::Continue,
            Next::Ground(placed) => {
                // SAFETY: on_step's own result, a local of its frame: it reads
                // the point and the normal from it after the exit.
                unsafe {
                    let result = self.frame.result();
                    write_floats(result.point(), placed.at);
                    write_floats(result.normal(), placed.normal);
                }
                ctx.set_reg(Reg::Eax, u32::from(placed.material.id()));
                Flow::Exit(StepGroundOnGroundExit::Material)
            }
        }
    }

    fn world(&self, _: &Thread) -> Option<StepWorld> {
        // SAFETY: the manager's game world, alive while it steps.
        unsafe { self.manager.world() }.map(|this| StepWorld { this })
    }
}

/// The step manager's game world.
#[derive(Clone, Copy)]
struct StepWorld {
    this: raw::GameWorld,
}

impl StepWorld {
    fn pvp_match_physics(self) -> Option<raw::BulletPhysicsWorld> {
        // SAFETY: the world's match, alive while it steps.
        unsafe { self.this.pvp_match()?.physics() }
    }

    fn pvp_match_materials(self) -> Option<raw::GameMaterialManager> {
        // SAFETY: as pvp_match_physics.
        unsafe { self.this.pvp_match()?.materials() }
    }
}

/// A ray's result as a hit, `length` its ray's.
///
/// # Safety
/// `result` is ray_test's, its object (if any) alive in the physics world.
unsafe fn hit(result: raw::RayResult, length: f32, above: f32) -> Option<Hit> {
    unsafe {
        let object = result.object()?;
        let material = raw::engine_triangle_material(Some(object), read(result.part()), u32::from(read(result.part_kind())));
        Some(Hit {
            material: GroundMaterial(material),
            depth: read(result.fraction()) * length - above,
            normal: floats(result.normal()),
        })
    }
}

/// Plays a step of `player` on `ground` at `at` (the game's frame), as the
/// game plays a foot's: on_step with the player's walking foot material
/// (movement 2, first or third person as is_first_view says), its ground
/// ray skipped for `ground` (the hook takes what this arms). The pair of the
/// two gives the sound: on the hud for the player himself in first person, a
/// point sound at `at` for everyone else; and the pair's particles (water's
/// alone have some). `normal` is the decal's, which no foot pair has. False
/// when the player has no world or the world no step manager (no match).
///
/// From a hook on the thread the game plays steps on (the simulation's, as
/// on_foot_step), with patch 153 installed (its hook takes the ground). A
/// step of a re-simulated tick is the caller's to drop or play
/// (`footsteps::decide`).
pub fn play(player: &Player<'_>, ground: GroundMaterial, at: [f32; 3], normal: [f32; 3]) -> bool {
    let thread = player.thread();
    // SAFETY: the live player, its game world and the world's step manager
    // (a member of the world); is_first_view reads the world's network
    // client; on_step is the game's own call, on the thread steps play on.
    unsafe {
        let Some(manager) = player.this.game_world().and_then(|world| raw::engine_step_manager(Some(world))) else {
            return false;
        };
        let first = raw::engine_is_first_view(Some(player.this)) != 0;
        let (foot, view) = if first {
            (read(player.this.run_material_first()), 0)
        } else {
            (read(player.this.run_material_third()), 1)
        };
        ARMED.set(thread, Some(Placed { material: ground, at, normal }));
        raw::engine_on_step(Some(manager), at.as_ptr(), UP.as_ptr(), u32::from(foot), view);
        ARMED.set(thread, None);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_ground_is_one_the_cook_named() {
        assert!(GroundMaterial::of(18).named());
        assert!(GroundMaterial::of(11).named(), "default itself is a name");
        assert!(!GroundMaterial::of(0x3FFF).named(), "an unknown face name, masked");
        assert!(!GroundMaterial::of(0xFFFF).named(), "a shape without a table");
    }

    #[test]
    fn the_water_look_meets_the_water_alone_and_nothing_else_meets_it() {
        let meets = |(group, mask): (u32, u32), (cgroup, cmask): (u32, u32)| cgroup & mask != 0 && cmask & group != 0;
        let water = (WATER_BIT, WATER_BIT);
        assert!(meets(water, water));
        // every object of the live data and of the code: project objects,
        // bushes, traps, client objects, players, hit models, grenades,
        // batteries, the lobby's
        let objects = [(2, 4), (8, 16), (10, 20), (8, 0x20), (0x808, 0x10), (0x201, 0x420), (1, 0x20), (0x80, 0x100),
                       (0x880, 0x100), (0x824, 3), (0x40, 0x11), (0x818, 8), (0xA18, 0x418), (0x800, 0x800), (0x20, 0x20)];
        for object in objects {
            assert!(!meets(water, object), "{object:x?}");
        }
        // every ray and body filter: on_step's, ours, bullets, blasts and
        // cameras, grenades, batteries, walkers, the use ray, traps, a spot,
        // the ragdoll level ray, and the bodies above
        let filters = [(GAME_GROUP, GAME_MASK), (0x34, 0x0A), (0x10, 0x48), (0x10, 8), (0x818, 8),
                       (0xA18, 0x418), (0x824, 3), (0x24, 2), (0x114, 0x8A), (0x404, 0x202), (0x414, 0x20A),
                       (0xFFFF, 0x80A), (0x40, 0x11), (0x800, 0x800), (0x20, 0x20)];
        for filter in filters {
            assert!(!meets(filter, water), "{filter:x?}");
        }
    }
}

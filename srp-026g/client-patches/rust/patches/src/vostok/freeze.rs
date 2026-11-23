//! Freeze Tag's engine boundary. Scoped native handles; all retained rule
//! state belongs to the shared snapshot codec, never to native pointers.
use super::round_countdown::Play;
use super::statistics::{Score, ScoreEvent};
use super::world::{floats, read};
use super::{Context, Thread};
use crate::raw;
use ragdoll::math::Vec3;
use srp_validate::world::freeze::{Body, Condition, PLAYERS, Phase, RADIUS};
use srp_validate::{Place, Team};

#[derive(Clone, Copy)]
pub enum PlayerView {
    FirstPerson,
    ThirdPerson,
}

/// Own only the temporary camera override, never the camera's orbit math,
/// collision, zoom limits or input processing. Those stay the warmup's.
#[derive(Clone, Copy)]
pub enum Camera {
    Native,
    Frozen { world: u32, restore: PlayerView },
}
impl Camera {
    pub fn update(
        self,
        ctx: &Context<'_>,
        rule: Option<&srp_validate::world::freeze::FreezeTag>,
    ) -> Self {
        let Some(world) = super::world::GameWorld::at_after_move(ctx) else {
            return Self::Native;
        };
        if world.playback() != super::world::Playback::Match {
            return Self::Native;
        }
        // SAFETY: the synchronous world tick owns the local player's handler.
        // Only native first/third-person modes can be borrowed; spectator and
        // pregame cameras are not ours. The saved identity is never dereferenced.
        unsafe {
            let Some(handler) = world.this.player_input() else {
                return Self::Native;
            };
            let mode = read(handler.mode());
            let local = world
                .this
                .game()
                .and_then(|game| game.network_client())
                .and_then(|client| client.local_player());
            let play = local.and_then(|player| player.world_core()).map(|core| Play::of(core));
            // A round's countdown is the native camera's: the warmup view,
            // and the player's camera back as the round starts.
            if play == Some(Play::RoundCountdown) {
                return Self::Native;
            }
            let frozen = local.is_some_and(|player| {
                read(player.alive()) != 0
                    && play == Some(Play::InProcess)
                    && rule.is_some_and(|rule| {
                        matches!(
                            rule.player(place_of(player)),
                            srp_validate::world::freeze::Player::Frozen { .. }
                        )
                    })
            });
            match (self, frozen, mode) {
                (
                    Self::Frozen {
                        world: owned,
                        restore,
                    },
                    false,
                    3,
                ) if owned == world.this.address() => {
                    clear_camera_input(handler);
                    raw::engine_switch_to_player_camera(
                        Some(world.this),
                        u8::from(matches!(restore, PlayerView::FirstPerson)),
                    );
                    Self::Native
                }
                (Self::Frozen { world: owned, .. }, true, 3) if owned == world.this.address() => {
                    self
                }
                // A late join can already be in the warmup view when its
                // first frozen world arrives. Own its eventual restoration.
                (_, true, 3) => Self::Frozen {
                    world: world.this.address(),
                    restore: PlayerView::FirstPerson,
                },
                (_, true, 0 | 2) => {
                    clear_camera_input(handler);
                    raw::engine_switch_to_warmup_camera(Some(world.this));
                    Self::Frozen {
                        world: world.this.address(),
                        restore: if mode == 0 {
                            PlayerView::FirstPerson
                        } else {
                            PlayerView::ThirdPerson
                        },
                    }
                }
                _ => Self::Native,
            }
        }
    }
}

/// # Safety
/// The camera transition owns this live input handler for the current tick.
unsafe fn clear_camera_input(handler: raw::PlayerInputHandler) {
    unsafe {
        handler.turn().write(0);
        handler.actions().write(0);
        handler.wheel().write(0.0);
    }
}

/// # Safety
/// `player` is live on the hook's thread.
unsafe fn place_of(player: raw::Player) -> Place {
    Place::new(unsafe { read(player.place()) }).expect("a player's roster place is one of the twenty")
}

fn player_at(core: raw::CoreWorld, place: Place) -> Option<raw::Player> {
    // SAFETY: the caller borrows a live core for the hook; bound both the
    // roster vector and kit index before following the current player.
    unsafe {
        if read(core.active_places()) & (1 << place.index()) == 0 {
            return None;
        }
        let first = core.places()?;
        if place.index() >= core.places_end()?.index_from(first)? {
            return None;
        }
        let slot = first.at(place.index());
        match read(slot.kit()) {
            0 => slot.kit0(),
            1 => slot.kit1(),
            2 => slot.kit2(),
            _ => None,
        }
    }
}

/// A place's current player as the native world has it.
#[derive(Clone, Copy)]
enum Seat {
    /// No current player (not in the match).
    Empty,
    /// In the match, its player out of the world: entered (or came back) and not spawned yet.
    /// It must spawn at its entry time. Every world load puts every active place's player into
    /// the world, one out of it with link 0 (game_world_core::deserialize 0x2D2D51: no models, no
    /// player effect +F540h, no tick lists), and the next removal with 1 then reads a null
    /// effect (0x2DE36D): the respawn tick's at the restart, or the respawn rule's leave handler
    /// (0x316BE0), which removes without asking (the crash of 2026-11-22, a rejoin in the pause).
    Entering,
    /// In the world: a body, fighting or frozen.
    InWorld,
}
fn seat(core: raw::CoreWorld, place: Place) -> Seat {
    match player_at(core, place) {
        None => Seat::Empty,
        // SAFETY: the roster's live current player of the place.
        Some(player) if unsafe { read(player.inserted()) } == 0 => Seat::Entering,
        Some(_) => Seat::InWorld,
    }
}

pub struct Player<'t> {
    native: raw::Player,
    _thread: &'t Thread,
}
impl<'t> Player<'t> {
    pub fn at_tick(ctx: &'t Context<'_>) -> Option<(Self, u32)> {
        // SAFETY: base_player::tick's entry and untouched stack argument.
        unsafe {
            Some((
                Self {
                    native: raw::site::freeze_player_tick::player(ctx)?,
                    _thread: ctx.thread(),
                },
                read(raw::site::freeze_player_tick::frame(ctx)?.time()),
            ))
        }
    }
    pub fn at_input(ctx: &'t Context<'_>) -> Option<Self> {
        // SAFETY: set_input's entry (patch 78's site): its ECX player.
        unsafe {
            Some(Self {
                native: raw::site::carry_input::player(ctx)?,
                _thread: ctx.thread(),
            })
        }
    }
    /// update_minimap_players' enemy at its spotted test (`minimap.enemy_spotted`, EAX).
    pub fn at_minimap_enemy(ctx: &'t Context<'_>) -> Option<Self> {
        // SAFETY: the minimap's live player (alive and in the world).
        unsafe {
            Some(Self {
                native: raw::site::minimap_enemy_spotted::player(ctx)?,
                _thread: ctx.thread(),
            })
        }
    }
    pub fn place(&self) -> Place {
        // SAFETY: a live player.
        unsafe { place_of(self.native) }
    }
    pub fn lethal(&self) -> bool {
        // SAFETY: a live player's deferred death flag.
        unsafe { read(self.native.pending_death()) != 0 }
    }
    pub fn playing(&self) -> bool {
        // SAFETY: a live player's owning core, absent in a lobby.
        unsafe {
            self.native.world_core().is_some_and(|core| Play::of(core).in_play())
        }
    }
    pub fn hold(&mut self, time: u32, phase: Phase) {
        // SAFETY: this hook owns and skips the tick. Advance its clock so
        // thaw never gives weapons/movement several seconds of elapsed time,
        // and the gun in hand's too (its own +5A4h, the tick 0x2E09EF): its
        // first tick after a thaw stepped the whole freeze at once, and the
        // recoil's spring (0x331A50) pinned a shot's kick at -1 for seconds,
        // the view moving by itself after it (the owner, 2026-11-22).
        unsafe {
            self.native.time().write(time);
            if let Some(item) = self.native.held_item()
                && read(item.vtable()) == raw::image::WEAPON_VTABLE
            {
                item.weapon().last_tick().write(time);
            }
            // A fresh spawn must remain non-lethal so the rule can see
            // every spawn completed. Its input/tick stay held meanwhile.
            if phase != Phase::Restarting {
                self.native.pending_death().write(1);
            }
        }
    }
}

pub struct World<'t> {
    rule: raw::PlayerRespawnRuleBase,
    core: raw::CoreWorld,
    time: u32,
    elapsed: u16,
    _thread: &'t Thread,
}
impl<'t> World<'t> {
    /// None before play (the match start's countdown).
    pub fn at_tick(ctx: &'t Context<'_>) -> Option<Self> {
        // SAFETY: respawn tick's entry: its live rule and frame arguments.
        unsafe {
            let frame = raw::site::respawn_tick::frame(ctx).expect("the respawn tick's frame");
            let rule = raw::site::respawn_tick::rule(ctx).expect("the respawn tick's rule");
            let core = rule.core().expect("a respawn rule's world");
            if !Play::of(core).in_play() {
                return None;
            }
            Some(Self {
                rule,
                core,
                time: read(frame.time()),
                elapsed: u16::try_from(read(frame.elapsed())).expect("a simulation step's milliseconds"),
                _thread: ctx.thread(),
            })
        }
    }
    pub fn bodies(&self) -> [Body; PLAYERS] {
        let mut bodies = [Body::Absent; PLAYERS];
        for (place, body) in Place::all().zip(bodies.iter_mut()) {
            let Some(player) = player_at(self.core, place) else {
                continue;
            };
            // SAFETY: the roster owns this current kit through the hook.
            unsafe {
                if read(player.inserted()) == 0 || read(player.alive()) == 0 {
                    continue;
                }
                let team = player.profile().expect("an inserted player's profile").team();
                let team = u8::try_from(read(team)).ok().and_then(Team::of).expect("a Freeze Tag player of team 0 or 1");
                let position = floats::<3>(player.feet());
                assert!(position.iter().all(|n| n.is_finite()), "an inserted player's position");
                let condition = if read(player.pending_death()) == 0 {
                    Condition::Fighting
                } else {
                    Condition::Lethal
                };
                *body = Body::Present {
                    team,
                    position,
                    condition,
                };
            }
        }
        bodies
    }
    pub fn elapsed(&self) -> u16 {
        self.elapsed
    }
    /// The new round: every place in the roster is due at this step, so this
    /// very tick takes out and spawns each, alive or frozen, through the
    /// native remove/spawn/kit setup (31697F..3169BE).
    pub fn restart(&mut self) {
        for place in Place::all().filter(|place| player_at(self.core, *place).is_some()) {
            // SAFETY: the rule's twenty-entry schedule.
            unsafe { self.rule.times().add(place.index()).write(self.time) };
        }
    }
    /// A thaw's credit for `place`, as for a kill ([`ScoreEvent::THAW`]): in its cell of the kit it
    /// plays, the scoreboard told, at the rule's tick in every game alike.
    pub fn credit_thaw(&mut self, place: Place) {
        let Some(player) = player_at(self.core, place) else { return };
        // SAFETY: the respawn rule's live core and its statistics; the place's live player.
        unsafe {
            let Some(statistics) = self.core.statistics() else { return };
            if let Some(score) = Score::of(statistics.shared(), player) {
                score.count(ScoreEvent::THAW, 1);
            }
        }
    }
    /// The round's countdown on or off (`round_countdown`): at the rule's tick, in list 0 before
    /// the places' ticks and the time rule's, so nothing of the step reads another; a load is
    /// followed by a tick, which sets it again.
    pub fn set_round_countdown(&mut self, on: bool) {
        // SAFETY: the respawn rule's live core, on the simulation's thread.
        unsafe { super::round_countdown::set(self.core, on) }
    }
    /// The place's native respawn time cancelled (u32::MAX: unscheduled, and no respawn
    /// countdown on the hud, 0x316C35), unless the place is entering: its entry spawn always
    /// runs natively ([`Seat::Entering`]).
    pub fn cancel(&mut self, place: Place) {
        match seat(self.core, place) {
            Seat::InWorld | Seat::Empty => {
                // SAFETY: the rule's twenty-entry schedule.
                unsafe { self.rule.times().add(place.index()).write(u32::MAX) };
            }
            Seat::Entering => {}
        }
    }
    pub fn clear(&self, from: Place, to: Place) -> bool {
        clear(self.core, from, to)
    }
    pub fn thaw(&mut self, place: Place) {
        let player = player_at(self.core, place).expect("a thawed body is in the world");
        // SAFETY: this transition owns this native player. Reset health as
        // base_player::spawn does, preserving kit, ammo and world insertion.
        unsafe {
            let damage = player.damage_model().expect("an inserted player's damage model");
            raw::engine_freeze_reset_health(Some(damage), self.time);
            player.pending_death().write(0);
            player.need_to_select_animations().write(1);
            player.time().write(self.time);
            player.input_rotation().write(0);
            player.actions().write(0);
        }
    }
    pub fn publish(&mut self, points: [u16; 2], winner: Option<Team>) {
        // SAFETY: the core owns this statistics handler throughout play.
        unsafe {
            let stats = self.core.statistics().expect("a playing world's statistics");
            stats.team_zero_points().write(points[0]);
            stats.team_one_points().write(points[1]);
            stats.winner().write(winner.map_or(2, |team| team.index() as u8));
        }
    }
}

/// Whether the static world leaves a clear line between two bodies' chests,
/// within the thaw radius; the HUD asks the same as the rule.
pub(super) fn clear(core: raw::CoreWorld, from: Place, to: Place) -> bool {
    let (Some(a), Some(b)) = (player_at(core, from), player_at(core, to)) else {
        return false;
    };
    // SAFETY: the core pins both players and the simulation physics world.
    unsafe {
        let [x, y, z] = floats::<3>(a.feet());
        let [bx, by, bz] = floats::<3>(b.feet());
        let delta = Vec3::new(bx - x, by - y, bz - z);
        let squared = delta.length_squared();
        // Out of reach needs no ray; one spot has no direction to cast.
        if squared > RADIUS * RADIUS {
            return false;
        }
        if squared < 0.0001 {
            return true;
        }
        let physics = a.physics_world().expect("an inserted player's physics world");
        let length = delta.length();
        let direction = delta * (1.0 / length);
        let mut room = raw::RayResultStorage::zeroed();
        let hit = room.handle();
        raw::engine_ray_test(
            Some(physics),
            Some(hit),
            [x, y + 0.8, z].as_ptr(),
            [direction.x, direction.y, direction.z].as_ptr(),
            length,
            0x24,
            2,
            core::ptr::null(),
            0,
        );
        hit.object().is_none()
    }
}

/// A body of the match this frame: the bones a glow on him follows, and his simulated transform
/// (row-major 4x4).
#[derive(Clone, Copy)]
pub struct FrozenBody {
    bones: super::carry_runtime::Bones,
    pub transform: [f32; 16],
}
impl FrozenBody {
    /// Whose bones: a glow played on other bones (another player object of the place) is played
    /// again on these.
    pub fn id(&self) -> BodyId {
        BodyId(self.bones)
    }
}
/// The bones a glow follows, as an identity: the play holds them, so no other body has them.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct BodyId(super::carry_runtime::Bones);

/// The frozen glow as this game draws it (never in the simulation: the load moment of its
/// resources differs per game, so an effect added from it would enter each game's world at
/// another tick): the game world's scene, and the frozen bodies' matrices from the world.
pub struct GlowScene<'t> {
    scene: super::carry_runtime::Scene,
    core: raw::CoreWorld,
    _thread: &'t Thread,
}
impl<'t> GlowScene<'t> {
    /// At the game world's frame (after its move) or as it is left; None outside a match.
    pub fn of(world: &super::world::GameWorld<'t>, thread: &'t Thread) -> Option<Self> {
        // SAFETY: the live game world of the hook, its renderer, scene and match core.
        unsafe {
            let scene = super::carry_runtime::Scene::of_world(world.this)?;
            let core = world.this.pvp_match()?.core();
            Some(Self { scene, core, _thread: thread })
        }
    }
    /// Whether the match is in play (a round's countdown too): the glow shows only then.
    pub fn in_play(&self) -> bool {
        // SAFETY: the live core of the hook's world.
        unsafe { Play::of(self.core).in_play() }
    }
    /// The body at `place`, inserted and alive.
    pub fn body(&self, place: Place) -> Option<FrozenBody> {
        let player = player_at(self.core, place)?;
        // SAFETY: the roster's current player of the place, live for the hook.
        unsafe {
            if read(player.inserted()) == 0 || read(player.alive()) == 0 {
                return None;
            }
            Some(FrozenBody { bones: super::carry_runtime::Bones::of(player), transform: floats::<16>(player.transform()) })
        }
    }
    /// `particle` played at `at` on `body`: its body-shaped emitters follow his bones.
    pub fn play(&self, particle: super::resources::KeptParticle, at: &[f32; 16], body: FrozenBody) {
        // SAFETY: a particle system kept for the process; the live scene.
        unsafe { self.scene.play_particle(particle.0, at, body.bones) }
    }
    pub fn move_to(&self, particle: super::resources::KeptParticle, at: &[f32; 16]) {
        // SAFETY: as play.
        unsafe { self.scene.move_particle(particle.0, at) }
    }
    pub fn stop(&self, particle: super::resources::KeptParticle) {
        // SAFETY: as play.
        unsafe { self.scene.stop_particle(particle.0) }
    }
}

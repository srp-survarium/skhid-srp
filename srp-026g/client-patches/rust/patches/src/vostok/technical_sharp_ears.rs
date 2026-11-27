//! Sharp Ears (412, patch 180, technical-skills.md) at `weapon_sound.made`: an enemy's grenade pin
//! pull as this game plays it, and the long-range copy played at the thrower.
//!
//! The pull's sound is every gun's `throw_grenade/pull_pin_substate` sound, played by its handler
//! state's effect (weapon_sound_events_handler_state<pull_pin_substate>, effect +160h: its +18h
//! vtable 0xA052F0 is the word 148h before the effect). Presentation only: the copy is a one-shot
//! point sound of the game world's scene (game_world::play_sound 0x25B940, what a bullet's hit
//! sound comes to through the bullet manager's engine).
use super::context::Context;
use super::geometry::WorldPoint;
use super::hook::Thread;
use super::sound::Emitter;
use super::types::Place;
use super::world::{GameWorld, read};
use crate::raw;

/// An enemy's pin pull this game's local player, with Sharp Ears, hears from afar.
pub struct EnemyPin<'t> {
    world: raw::GameWorld,
    at: WorldPoint,
    _thread: &'t Thread,
}

impl<'t> EnemyPin<'t> {
    /// The site's sound when it is a pin pull of a player who is not on the local player's team,
    /// and the local player's current kit has Sharp Ears; None for any other sound, a spectator's
    /// or a replay's game (no local player), the local player's own pull.
    pub fn at(ctx: &'t Context<'_>) -> Option<EnemyPin<'t>> {
        use raw::site::weapon_sound_made as site;
        let thread = ctx.thread();
        // SAFETY: the site's: [esp+24h] the effect, alive for its callback; it is a member of its
        // handler state at +158h..+170h, so the word 148h before it is inside that state. Its
        // weapon and user (the game's own reads, 0x297E47..0x297E50) and its game world (0x297E44)
        // are alive while the world plays it, on this thread.
        let (world, place) = unsafe {
            let effect = site::frame(ctx)?.effect()?;
            if read(effect.pull_pin_handler()) != raw::image::PULL_PIN_SOUND_HANDLER {
                return None;
            }
            let place = Place::new(u32::from(read(effect.weapon()?.user()?.place())))?;
            (effect.world()?, place)
        };
        let client = GameWorld::of(world, thread).network_client()?;
        let local = client.local_player()?;
        if !super::technical::skills(local.this).sharp_ears() {
            return None;
        }
        let thrower = client.player(u32::from(place.get()))?;
        let (ours, theirs) = (super::medical::Player::new(thread, local.this), super::medical::Player::new(thread, thrower.this));
        // Opponents only: another place, not on its team.
        if local.this == thrower.this || ours.ally(theirs) {
            return None;
        }
        let eye = thrower.eye_transform();
        let at = WorldPoint::parse([eye[12], eye[13], eye[14]])?;
        Some(EnemyPin { world, at, _thread: thread })
    }

    /// Plays `sound` once at the thrower's eye, a point sound of the world's scene (play_sound takes
    /// a reference for the call; emit_and_play_once's proxy keeps its own while it plays).
    pub fn play(&self, sound: &Emitter) {
        let slot = sound.handle().address();
        // SAFETY: the effect's game world, alive for the hook, on its thread (the game's own sounds
        // play there); the slot holds a live emitter, the point three f32.
        unsafe { raw::engine_world_play_sound(Some(self.world), &slot, self.at.coordinates().as_ptr()) }
    }
}

/// At a match frame (`game_world.after_move`): whether this game's local player's current kit has
/// Sharp Ears. Only such a game loads the long-range copy.
pub fn local_listens(ctx: &Context<'_>) -> bool {
    // SAFETY: game_world::tick's world (EBX), alive for the hook on the game's thread.
    let Some(world) = (unsafe { raw::site::game_world_after_move::world(ctx) }) else { return false };
    let Some(local) = GameWorld::of(world, ctx.thread()).network_client().and_then(|client| client.local_player()) else {
        return false;
    };
    super::technical::skills(local.this).sharp_ears()
}

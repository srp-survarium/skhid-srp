//! Return to Sender (422, patch 180, technical-skills.md): the live enemy grenades near a player,
//! read from the players' grenade stacks as every game has them, and the throw-back with the
//! game's own calls (remove from the physics world, then throw, in the same tick).
use core::ffi::CStr;

use technical_skills::Clock;
use technical_skills::throw_back::{self, Grenade, Nearest};

use super::config::{ConfigError, ConfigValue};
use super::context::Context;
use super::medical::Player as Roster;
use super::technical::grenade_set;
use super::world::{Player, floats, read};
use crate::raw;

/// A match's places (core's buffer holds 20).
const PLACES: u8 = 20;
/// The level's ray (patch 78's and 85's: the static level, never a grenade, group 818h mask 8).
const SIGHT_GROUP: u32 = 0x24;
const SIGHT_MASK: u32 = 2;
/// The thrower's modifier of a throw's speed (`grenade_throw_power`, grenade_set_core::throw_grenade
/// 0x30A2CC).
const THROW_POWER: u32 = 27;

/// A live enemy grenade in reach and in sight of a player's eye.
#[derive(Clone, Copy)]
pub struct Target {
    grenade: raw::GrenadeCore,
    set: raw::GrenadeSet,
    /// The place of its stack's holder (the thrower, whom the kill still credits).
    pub owner: u8,
}

/// A player's team, 0 or 1; none for another value (no team).
///
/// # Safety
/// `player` is alive for the hook.
unsafe fn team(player: raw::Player) -> Option<u32> {
    // SAFETY: the live player's retained profile.
    match unsafe { read(player.profile()?.team()) } {
        team @ (0 | 1) => Some(team),
        _ => None,
    }
}

/// A grenade object as its saved fields say.
///
/// # Safety
/// `grenade` is a live grenade of a live stack.
unsafe fn parse(grenade: raw::GrenadeCore) -> Option<Grenade> {
    // SAFETY: the grenade's own fields.
    unsafe { Grenade::parse(read(grenade.physics()) != 0, read(grenade.explode_time()), read(grenade.spent()) != 0) }
}

/// Whether nothing of the level is between `eye` and `at` (the prompt's ray, every game's too).
fn in_sight(player: &Player<'_>, eye: [f32; 3], at: [f32; 3]) -> bool {
    let d = [at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]];
    let distance = ragdoll::math::sqrt(d[0] * d[0] + d[1] * d[1] + d[2] * d[2]);
    if distance < 0.01 {
        return true;
    }
    let direction = [d[0] / distance, d[1] / distance, d[2] / distance];
    player
        .physics_world()
        .is_some_and(|physics| physics.ray_test(eye, direction, distance, SIGHT_GROUP, SIGHT_MASK).is_none())
}

/// The grenade `player` would throw back at `now`: of the enemies' stacks (its holder's team not
/// the player's), the nearest grenade that is thrown with more than the minimum fuse left, within
/// the reach of the player's eye and in its sight. The places, the slots and the stacks' objects
/// are taken in order, so every game takes the same one.
pub fn target(player: &Player<'_>, now: Clock) -> Option<Target> {
    let thread = player.thread();
    // SAFETY: the hook's live player, its team and world core; each roster lookup holds its
    // player for the loop's turn, whose inventory owns its 23 slots, their items and each grenade
    // stack's objects (made at the cook, kept for the match).
    unsafe {
        let own = team(player.this)?;
        let m: [f32; 16] = floats(player.this.eye());
        let eye = [m[12], m[13], m[14]];
        let core = player.this.world_core();
        let mut nearest = Nearest::new();
        for place in 0..PLACES {
            let Some(holder) = Roster::in_core(thread, core, place) else { continue };
            let holder = holder.get().get();
            if team(holder).is_none_or(|team| team == own) {
                continue;
            }
            let Some(inventory) = holder.inventory() else { continue };
            let first = inventory.slots();
            let count = inventory.slots_end().index_from(first).unwrap_or(0);
            for slot in 0..count {
                let Some(set) = first.at(slot).item().and_then(|item| grenade_set(item)) else { continue };
                let (Some(objects), Some(end)) = (set.grenades(), set.grenades_end()) else { continue };
                for n in 0..end.index_from(objects).unwrap_or(0) {
                    let Some(grenade) = objects.at(n).grenade() else { continue };
                    if !parse(grenade).is_some_and(|g| g.returnable(now)) {
                        continue;
                    }
                    let at: [f32; 3] = floats(grenade.centre());
                    let Some(squared) = throw_back::within_reach(eye, at) else { continue };
                    if nearest.nearer(squared) && in_sight(player, eye, at) {
                        nearest.offer(squared, Target { grenade, set, owner: read(holder.place()) });
                    }
                }
            }
        }
        nearest.get()
    }
}

/// The throw-back: `target` out of the physics world and thrown again from `player`'s eye along its
/// view, at its stack's throw_power times the player's modifier 27 (as grenade_set_core::throw_grenade
/// throws, 0x30A2AD..0x30A321, from the eye instead of the hand). Its fuse, tick, stack and kill
/// credit stay as they were. False (nothing done) for a player without a kit.
pub fn throw(player: &Player<'_>, target: Target) -> bool {
    let m = player.eye_transform();
    let forward = [m[8], m[9], m[10]];
    // SAFETY: the live player's current kit's modifiers; the grenade and its stack are live (found
    // in this hook); both calls are the grenade's own virtuals, the pair explode makes too.
    unsafe {
        let Some(modifiers) = super::medical::kit_modifiers(player.this) else { return false };
        let power = raw::engine_apply_modifier(Some(modifiers), THROW_POWER, read(target.set.throw_power()), 1.0);
        let velocity = forward.map(|axis| axis * power);
        raw::engine_grenade_remove(Some(target.grenade));
        raw::engine_grenade_throw(Some(target.grenade), m.as_ptr(), velocity.as_ptr());
    }
    true
}

/// What a throw-back command found, where every game plays it.
#[derive(Clone, Copy)]
pub enum Played {
    /// The player's current kit has no Return to Sender (a forged command: nothing).
    NoSkill,
    /// No grenade to take.
    NoGrenade,
    Thrown { owner: u8 },
}

/// A set_input past its early-outs (`technical.throw_back`, before patch 78's filter): the place,
/// its input's time, and what a throw-back command in it did; none without a command.
pub fn input(ctx: &Context<'_>) -> Option<(u8, u32, Played)> {
    use raw::site::technical_throw_back as site;
    // SAFETY: set_input's live player, its new input and its frame's time argument.
    let (player, time) = unsafe {
        let (player, input, frame) = (site::player(ctx)?, site::input(ctx)?, site::frame(ctx)?);
        if !throw_back::command(read(input.actions())) {
            return None;
        }
        (Player::of(ctx, Some(player))?, read(frame.time()))
    };
    let played = if !super::technical::skills(player.this).return_to_sender() {
        Played::NoSkill
    } else {
        match target(&player, Clock(time)) {
            Some(found) if throw(&player, found) => Played::Thrown { owner: found.owner },
            Some(_) | None => Played::NoGrenade,
        }
    };
    Some((player.place(), time, played))
}

/// The local player's offer: its current kit's modifier 47 (what the server's check reads) when it
/// has Return to Sender and is alive, and the grenade it would take now.
pub fn offer(viewer: &Player<'_>) -> Option<(f32, Target)> {
    if !viewer.alive() {
        return None;
    }
    // SAFETY: the live player's current kit's modifiers and its simulation clock.
    let (kit, now) = unsafe {
        let modifiers = super::medical::kit_modifiers(viewer.this)?;
        (read(modifiers.technical_ranks()), Clock(read(viewer.this.time())))
    };
    if !technical_skills::Skills::parse(kit).is_some_and(|s| s.return_to_sender()) {
        return None;
    }
    Some((kit, target(viewer, now)?))
}

/// A grenade's name key (its stack's config ui_desc/text_descriptions/name), copied.
#[derive(Clone, Copy)]
pub struct NameKey([u8; Self::BYTES]);

impl NameKey {
    const BYTES: usize = 64;

    pub fn get(&self) -> &CStr {
        CStr::from_bytes_until_nul(&self.0).unwrap_or(c"")
    }
}

/// The name key of `target`'s stack, empty when its config has none that fits.
pub fn name(target: Target) -> NameKey {
    let key = || -> Result<NameKey, ConfigError> {
        // SAFETY: the live stack's dictionary entry and its config, kept by the dictionary for the
        // match; each value's type is read before its use.
        let value = unsafe {
            let root = target.set.item().entry().and_then(|e| e.config()).and_then(|c| c.root());
            ConfigValue::read(root.ok_or(ConfigError::Missing)?)?
        };
        let text = value
            .table()?
            .required(c"ui_desc")?
            .table()?
            .required(c"text_descriptions")?
            .table()?
            .required(c"name")?
            .text()?
            .to_bytes();
        if text.len() >= NameKey::BYTES {
            return Err(ConfigError::WrongType);
        }
        let mut out = [0u8; NameKey::BYTES];
        out[..text.len()].copy_from_slice(text);
        Ok(NameKey(out))
    };
    key().unwrap_or(NameKey([0; NameKey::BYTES]))
}

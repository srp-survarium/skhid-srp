//! The crosshair's white hit marker and the bullets that make it
//! (docs/notes/hit-marker-walls.md):
//!
//! - `bullet.pierced`: check_collision once try_pierce let a bullet through
//!   what its ray met;
//! - `damage_collector.hit`: player::generate_hit_event writing a bullet's
//!   hit into its shooter's inflicted_damage_collector, world state every
//!   game computes;
//! - `damage_collector.draw`: draw_damage_collector once the drawn player's
//!   collector changed and it took its copy, before it shows the marker
//!   (`root.crosshair_enemy_hit(fraction, 200, false)`); `quiet` skips that.
//!
//! The kill's red marker is another call (0x23CF20), not reached from here.
//! The fields are fields_026g.tsv's.

use super::context::Context;
use super::drug::HitType;
use super::types::Place;
use super::world::read;
use crate::raw;

/// A bullet as a value kept between hooks: its id (the manager's count) and
/// the time it was fired. A world reloaded under a rollback gives the bullet
/// both again, so the key outlives the object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BulletKey {
    id: u32,
    born: u32,
}

impl BulletKey {
    /// # Safety
    /// `bullet` is alive on this hook's thread.
    unsafe fn of(bullet: raw::Bullet) -> BulletKey {
        // SAFETY: the caller's.
        unsafe { BulletKey { id: read(bullet.id()), born: read(bullet.born()) } }
    }

    #[cfg(test)]
    pub fn for_test(id: u32, born: u32) -> BulletKey {
        BulletKey { id, born }
    }
}

/// What a bullet went through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Through {
    /// A hit receiver: a player's body (its own shooter's too), a trap, a lamp.
    Receiver,
    /// The level's collision or a prop's with no receiver, of a material with
    /// armour 0 (glass, foliage, cloth, paper): try_pierce lets every bullet
    /// through it for nothing (k = 1, 0x32D99E).
    Open,
    /// Any other surface: a wall.
    Wall,
}

impl Through {
    /// A surface by its material's armour (try_pierce's A, its test
    /// `ucomiss A, 0`: -0 is 0, NaN is not).
    pub fn surface(armor: f32) -> Through {
        if armor == 0.0 { Through::Open } else { Through::Wall }
    }
}

/// A bullet that just went through something (`bullet.pierced`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pierce {
    pub bullet: BulletKey,
    /// Its ammo's damage type: a round's is ranged, a weapon's melee blunt.
    pub kind: Option<HitType>,
    pub through: Through,
}

/// The pierce at `bullet.pierced`.
pub fn pierce(ctx: &Context<'_>) -> Option<Pierce> {
    use raw::site::bullet_pierced as site;
    // SAFETY: the context is this site's: check_collision's bullet, its ammo
    // and its struck material (check_collision set it before try_pierce),
    // alive while it runs, on its thread.
    unsafe {
        let bullet = site::bullet(ctx)?;
        let through = match site::receiver(ctx) {
            0 => Through::surface(read(bullet.surface()?.armor())),
            _ => Through::Receiver,
        };
        let kind = bullet.ammo().and_then(|ammo| HitType::parse(read(ammo.damage_type())));
        Some(Pierce { bullet: BulletKey::of(bullet), kind, through })
    }
}

/// An inflicted_damage_collector as a value kept between hooks (its player's
/// +0F534h): its address, compared, never read through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollectorId(u32);

/// The hit a collector shows: whom its last hits went into (the game's place
/// byte, 0FFh none) and when the last was. The game's marker shows when the
/// victim or the collected fraction changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Collected {
    collector: CollectorId,
    victim: u8,
    time: u32,
}

impl Collected {
    pub fn collector(self) -> CollectorId {
        self.collector
    }

    #[cfg(test)]
    pub fn for_test(collector: u32, victim: u8, time: u32) -> Collected {
        Collected { collector: CollectorId(collector), victim, time }
    }
}

/// What `collector` shows.
///
/// # Safety
/// `collector` is alive on this hook's thread.
unsafe fn collected(collector: raw::DamageCollector) -> Collected {
    // SAFETY: the caller's.
    unsafe {
        Collected { collector: CollectorId(collector.address()), victim: read(collector.victim()), time: read(collector.time()) }
    }
}

/// A bullet's hit gone into its shooter's collector (`damage_collector.hit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollectorHit {
    pub shooter: Place,
    pub collected: Collected,
    pub bullet: BulletKey,
}

/// The hit at `damage_collector.hit`.
pub fn collector_hit(ctx: &Context<'_>) -> Option<CollectorHit> {
    use raw::site::damage_collector_hit as site;
    // SAFETY: the context is this site's: generate_hit_event's arguments,
    // the shooter's collector and the bullet, alive while it runs, on its
    // thread.
    unsafe {
        let shooter = Place::new(read(site::frame(ctx)?.attacker()))?;
        Some(CollectorHit { shooter, collected: collected(site::collector(ctx)?), bullet: BulletKey::of(site::bullet(ctx)?) })
    }
}

/// What the drawn collector shows, at `damage_collector.draw` (it changed,
/// and the draw has taken it into its copy).
pub fn drawn(ctx: &Context<'_>) -> Option<Collected> {
    // SAFETY: the context is this site's: the drawn player's collector,
    // alive while it draws, on its thread.
    unsafe { Some(collected(raw::site::damage_collector_draw::collector(ctx)?)) }
}

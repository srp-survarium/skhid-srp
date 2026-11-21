//! Patch 167: no white hit marker for a hit through a wall (the owner,
//! 2026-11-21; docs/notes/hit-marker-walls.md).
//!
//! - A bullet that went through a surface other than leaves is noted (by its
//!   key) when it does.
//! - Each bullet hit the game writes into its shooter's collector leaves the
//!   shooter's mark: clear, or walled when the bullet was noted.
//! - The collector drawn while it shows a walled hit gives no marker: as
//!   "Kill marker only" treats a hit. The kill's marker is another call.
//!
//! The notes are this game's alone: the collector and the bullets are the
//! world's, as they were.

#![forbid(unsafe_code)]

use crate::vostok::drug::HitType;
use crate::vostok::hit_marker::{self, BulletKey, Collected, Pierce, Through};
use crate::vostok::types::Place;
use crate::vostok::{Context, Flow, GameCell, HitMarkerWallsOnDrawExit, NoExit};

/// The walled bullets kept: a bullet hits in the step it pierced or soon
/// after, and a step fires far fewer.
const NOTED: usize = 128;

/// The last walled bullets, the oldest overwritten.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Walled {
    bullets: [Option<BulletKey>; NOTED],
    next: usize,
}

impl Walled {
    const NONE: Walled = Walled { bullets: [None; NOTED], next: 0 };

    fn holds(&self, bullet: BulletKey) -> bool {
        self.bullets.contains(&Some(bullet))
    }

    fn with(mut self, bullet: BulletKey) -> Walled {
        if !self.holds(bullet) {
            self.bullets[self.next] = Some(bullet);
            self.next = (self.next + 1) % NOTED;
        }
        self
    }
}

/// What a shooter's collector shows, by its last bullet hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    /// A hit at this victim and time reached the body clear.
    Clear(Collected),
    /// Every hit at this victim and time went through a wall.
    Walled(Collected),
}

impl Mark {
    /// After a hit: a clear one of the same step stays shown (a shotgun's
    /// pellets, some past the wall's edge).
    fn after(last: Option<Mark>, collected: Collected, walled: bool) -> Mark {
        if walled && last != Some(Mark::Clear(collected)) { Mark::Walled(collected) } else { Mark::Clear(collected) }
    }
}

static WALLED: GameCell<Walled> = GameCell::new(Walled::NONE);
static MARKS: GameCell<[Option<Mark>; Place::COUNT]> = GameCell::new([None; Place::COUNT]);

fn through_wall(pierce: Pierce) -> bool {
    pierce.kind == Some(HitType::Ranged) && matches!(pierce.through, Through::Surface(material) if !material.is_leaves())
}

/// check_collision once try_pierce let a bullet through (`bullet.pierced`).
pub fn on_pierced(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(pierce) = hit_marker::pierce(ctx)
        && through_wall(pierce)
    {
        WALLED.update(ctx.thread(), |walled| walled.with(pierce.bullet));
    }
    Flow::Continue
}

/// A bullet's hit written into its shooter's collector (`damage_collector.hit`).
pub fn on_hit(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(hit) = hit_marker::collector_hit(ctx) {
        let thread = ctx.thread();
        let walled = WALLED.get(thread).holds(hit.bullet);
        MARKS.update(thread, |mut marks| {
            let mark = &mut marks[hit.shooter.index()];
            *mark = Some(Mark::after(*mark, hit.collected, walled));
            marks
        });
    }
    Flow::Continue
}

/// st_draw about to draw its player's collector (`damage_collector.draw`).
pub fn on_draw(ctx: &mut Context<'_>) -> Flow<HitMarkerWallsOnDrawExit> {
    match hit_marker::drawn(ctx) {
        Some((place, shown)) if MARKS.get(ctx.thread())[place.index()] == Some(Mark::Walled(shown)) => {
            Flow::Exit(HitMarkerWallsOnDrawExit::Quiet)
        }
        _ => Flow::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vostok::step::GroundMaterial;

    fn at(victim: u8, time: u32) -> Collected {
        Collected::for_test(victim, time)
    }

    #[test]
    fn a_clear_hit_shows() {
        assert_eq!(Mark::after(None, at(3, 100), false), Mark::Clear(at(3, 100)));
        assert_eq!(Mark::after(Some(Mark::Walled(at(3, 100))), at(3, 100), false), Mark::Clear(at(3, 100)));
    }

    #[test]
    fn a_walled_hit_hides_unless_a_clear_one_of_its_step_shows() {
        assert_eq!(Mark::after(None, at(3, 100), true), Mark::Walled(at(3, 100)));
        assert_eq!(Mark::after(Some(Mark::Clear(at(3, 100))), at(3, 100), true), Mark::Clear(at(3, 100)));
        assert_eq!(Mark::after(Some(Mark::Clear(at(3, 50))), at(3, 100), true), Mark::Walled(at(3, 100)));
        assert_eq!(Mark::after(Some(Mark::Clear(at(4, 100))), at(3, 100), true), Mark::Walled(at(3, 100)));
    }

    #[test]
    fn walls_are_every_surface_but_leaves_for_a_round() {
        let pierce = |kind, through| Pierce { bullet: BulletKey::for_test(1, 2), kind, through };
        let surface = |id| Through::Surface(GroundMaterial::of(id));
        assert!(through_wall(pierce(Some(HitType::Ranged), surface(17))));
        assert!(through_wall(pierce(Some(HitType::Ranged), surface(11))));
        for leaves in [24, 44, 49] {
            assert!(!through_wall(pierce(Some(HitType::Ranged), surface(leaves))));
        }
        assert!(!through_wall(pierce(Some(HitType::Ranged), Through::Receiver)));
        assert!(!through_wall(pierce(Some(HitType::Blunt), surface(17))));
    }

    #[test]
    fn walled_bullets_are_kept_once_the_oldest_going() {
        let mut walled = Walled::NONE;
        for id in 0..NOTED as u32 {
            walled = walled.with(BulletKey::for_test(id, 7)).with(BulletKey::for_test(id, 7));
        }
        assert!(walled.holds(BulletKey::for_test(0, 7)) && !walled.holds(BulletKey::for_test(0, 8)));
        walled = walled.with(BulletKey::for_test(500, 7));
        assert!(!walled.holds(BulletKey::for_test(0, 7)) && walled.holds(BulletKey::for_test(1, 7)));
        assert!(walled.holds(BulletKey::for_test(500, 7)));
    }
}

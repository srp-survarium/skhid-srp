//! Patch 167: no white hit marker for a hit through a wall (the owner,
//! 2026-11-21; docs/notes/hit-marker-walls.md).
//!
//! - A bullet that went through a wall, a surface of armour above 0 (not
//!   glass, foliage, cloth, paper), is noted (by its key) when it does.
//! - Each bullet hit the game writes into its shooter's collector leaves the
//!   shooter's mark: a clear hit not drawn yet, or walled when the bullet
//!   was noted.
//! - A changed collector drawn while it shows a walled hit gives no marker,
//!   as "Kill marker only" treats a hit; the draw still takes its copy. The
//!   kill's marker is another call.
//!
//! The notes are this game's alone: the collector and the bullets are the
//! world's, as they were.

#![forbid(unsafe_code)]

use crate::vostok::drug::HitType;
use crate::vostok::hit_marker::{self, BulletKey, Collected, CollectorId, Pierce, Through};
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

/// A shooter's last bullet hit, until the draw shows it (None: nothing to
/// show, or shown).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    /// A clear hit into this collector, not drawn yet: walled hits after it
    /// show with it (a shotgun's pellets, some past the wall's edge).
    Pending(CollectorId),
    /// The collector shows this, and every hit since the last drawn went
    /// through a wall.
    Walled(Collected),
}

impl Mark {
    fn after_hit(last: Option<Mark>, collected: Collected, walled: bool) -> Mark {
        let pending = Mark::Pending(collected.collector());
        if walled && last != Some(pending) { Mark::Walled(collected) } else { pending }
    }

    /// The mark once the draw took `drawn`, and whether it shows no marker.
    fn after_draw(mark: Option<Mark>, drawn: Collected) -> (Option<Mark>, bool) {
        match mark {
            Some(Mark::Walled(shown)) if shown == drawn => (mark, true),
            Some(Mark::Pending(collector)) if collector == drawn.collector() => (None, false),
            _ => (mark, false),
        }
    }
}

static WALLED: GameCell<Walled> = GameCell::new(Walled::NONE);
static MARKS: GameCell<[Option<Mark>; Place::COUNT]> = GameCell::new([None; Place::COUNT]);

fn through_wall(pierce: Pierce) -> bool {
    pierce.kind == Some(HitType::Ranged) && pierce.through == Through::Wall
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
            *mark = Some(Mark::after_hit(*mark, hit.collected, walled));
            marks
        });
    }
    Flow::Continue
}

/// draw_damage_collector with a changed collector, its copy taken
/// (`damage_collector.draw`).
pub fn on_draw(ctx: &mut Context<'_>) -> Flow<HitMarkerWallsOnDrawExit> {
    let Some(drawn) = hit_marker::drawn(ctx) else {
        return Flow::Continue;
    };
    let mut quiet = false;
    MARKS.update(ctx.thread(), |mut marks| {
        for mark in &mut marks {
            let (next, hidden) = Mark::after_draw(*mark, drawn);
            *mark = next;
            quiet |= hidden;
        }
        marks
    });
    if quiet { Flow::Exit(HitMarkerWallsOnDrawExit::Quiet) } else { Flow::Continue }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINE: u32 = 0x3000_F534;

    fn at(victim: u8, time: u32) -> Collected {
        Collected::for_test(MINE, victim, time)
    }

    fn pending() -> Mark {
        Mark::Pending(at(0, 0).collector())
    }

    #[test]
    fn a_clear_hit_waits_for_the_draw() {
        assert_eq!(Mark::after_hit(None, at(3, 100), false), pending());
        assert_eq!(Mark::after_hit(Some(Mark::Walled(at(3, 100))), at(3, 100), false), pending());
        assert_eq!(Mark::after_draw(Some(pending()), at(3, 100)), (None, false));
    }

    #[test]
    fn a_walled_hit_hides_unless_a_clear_one_waits() {
        assert_eq!(Mark::after_hit(None, at(3, 100), true), Mark::Walled(at(3, 100)));
        assert_eq!(Mark::after_hit(Some(pending()), at(3, 150), true), pending());
        let other = Collected::for_test(MINE + 0x10000, 3, 100);
        assert_eq!(Mark::after_hit(Some(Mark::Pending(other.collector())), at(3, 150), true), Mark::Walled(at(3, 150)));
        assert_eq!(Mark::after_draw(Some(Mark::Walled(at(3, 150))), at(3, 150)), (Some(Mark::Walled(at(3, 150))), true));
        assert_eq!(Mark::after_draw(Some(Mark::Walled(at(3, 150))), at(0xFF, 0)), (Some(Mark::Walled(at(3, 150))), false));
        assert_eq!(Mark::after_draw(Some(Mark::Walled(at(3, 150))), other), (Some(Mark::Walled(at(3, 150))), false));
    }

    #[test]
    fn walls_are_surfaces_of_armour_for_a_round() {
        let pierce = |kind, through| Pierce { bullet: BulletKey::for_test(1, 2), kind, through };
        for wall in [0.10, 0.23, 20.0, f32::NAN] {
            assert!(through_wall(pierce(Some(HitType::Ranged), Through::surface(wall))));
        }
        for open in [0.0, -0.0] {
            assert!(!through_wall(pierce(Some(HitType::Ranged), Through::surface(open))));
        }
        assert!(!through_wall(pierce(Some(HitType::Ranged), Through::Receiver)));
        assert!(!through_wall(pierce(Some(HitType::Blunt), Through::surface(0.23))));
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

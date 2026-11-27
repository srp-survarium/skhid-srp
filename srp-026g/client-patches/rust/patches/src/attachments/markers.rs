//! The modification markers (`docs/notes/modification-069.md`): while the
//! lobby shows a gun of the modules table above the bench, the lobby movie
//! is told where on the screen each slot of it is, as 0.69d0's workshop
//! tells its movie each frame (`root.weapon_workshop_set_slots_positions`):
//! the gun's own slots and those its carriers bring
//! ([`weapon_modules::slot_places`]). The movie draws a marker over each
//! slot it lists for the gun and keeps the points it was given last; it is
//! told again only when a point moved by a pixel or the set changed, and
//! after the gun left the bench (the movie then shows another window).
//!
//! Drawing alone: nothing here is in a match's world.
#![forbid(unsafe_code)]

use weapon_modules::{DictId, SlotId, slot_places};

use crate::vostok::GameCell;
use crate::vostok::attachments;
use crate::vostok::bench_gun::{BenchGun, Drawn};
use crate::vostok::{Context, Thread};

/// The slots one gun can have a marker at: the most a gun of the table has
/// with its carriers' (the AK-74M has 13).
const MAX_POINTS: usize = 32;

/// A slot's place on the screen: its id's byte, the render window's pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Point {
    slot: u8,
    x: i32,
    y: i32,
}

/// The points of a gun's slots on the screen now, the ones off it left out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Points {
    gun: u16,
    count: usize,
    list: [Point; MAX_POINTS],
}

impl Points {
    fn of(gun: DictId, seen: impl Iterator<Item = (SlotId, [f32; 2])>) -> Points {
        let mut points = Points { gun: gun.get(), count: 0, list: [Point { slot: 0, x: 0, y: 0 }; MAX_POINTS] };
        for ((slot, [x, y]), place) in seen.zip(points.list.iter_mut()) {
            *place = Point { slot: slot.byte(), x: pixel(x), y: pixel(y) };
            points.count += 1;
        }
        points
    }

    fn all(&self) -> &[Point] {
        self.list.get(..self.count).unwrap_or(&[])
    }

    /// The movie's arguments: the gun's dict id, then a slot's byte and its
    /// x and y a slot.
    fn numbers<'o>(&self, out: &'o mut [f64; 1 + 3 * MAX_POINTS]) -> &'o [f64] {
        out[0] = f64::from(self.gun);
        for (point, room) in self.all().iter().zip(out[1..].chunks_exact_mut(3)) {
            room.copy_from_slice(&[f64::from(point.slot), f64::from(point.x), f64::from(point.y)]);
        }
        &out[..1 + 3 * self.count]
    }
}

/// The pixel nearest `v` (a half away from zero; no `round` in `no_std`).
fn pixel(v: f32) -> i32 {
    if v < 0.0 { (v - 0.5) as i32 } else { (v + 0.5) as i32 }
}

/// What the movie holds of the gun at the bench.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Markers {
    /// No gun at the bench since the movie was last told: the next one is
    /// told whatever it was told before.
    Away,
    /// The movie holds these points.
    Told(Points),
}

/// What a draw of the lobby's own character's weapon shows.
enum Seen {
    /// The weapon in its hands: no window of a gun open.
    Away,
    /// A gun of the table above the bench, its points now.
    AtBench(Points),
}

/// What to do after a draw: the state, and the points to tell the movie.
fn step(state: Markers, seen: Seen) -> (Markers, Option<Points>) {
    match (state, seen) {
        (_, Seen::Away) => (Markers::Away, None),
        (Markers::Told(told), Seen::AtBench(now)) if told == now => (state, None),
        (_, Seen::AtBench(now)) => (state, Some(now)),
    }
}

static MARKERS: GameCell<Markers> = GameCell::new(Markers::Away);

/// `attachments.draw`: the lobby's own character's weapon drawn; the
/// movie told where its slots are when it is a gun of the table at the
/// bench and they moved.
pub(super) fn on_draw(ctx: &Context<'_>) {
    let drawn = Drawn::at(ctx);
    let bench = match &drawn {
        Drawn::Other => return,
        Drawn::InHands => None,
        Drawn::AtBench(bench) => Some(bench),
    };
    let seen = bench.and_then(points).map_or(Seen::Away, Seen::AtBench);
    let thread = ctx.thread();
    let (state, tell) = step(MARKERS.get(thread), seen);
    MARKERS.set(thread, state);
    if let (Some(points), Some(bench)) = (tell, bench) {
        told(thread, &points, bench);
    }
}

/// The points of the bench's gun: None for a weapon the table has no gun
/// of, or whose model lacks the gun's bones (another gun's still drawn).
fn points(bench: &BenchGun<'_>) -> Option<Points> {
    let gun = attachments::modules(bench.thread())?.gun(bench.dict()?)?;
    if bench.bones()? < usize::from(gun.bones()) {
        return None;
    }
    let seen = slot_places(gun).filter_map(|(slot, at)| Some((slot, bench.on_screen(&at)?)));
    Some(Points::of(gun.dict(), seen))
}

/// The movie told `points`; held as told only when it took them.
fn told(thread: &Thread, points: &Points, bench: &BenchGun<'_>) {
    let mut room = [0.0; 1 + 3 * MAX_POINTS];
    if bench.tell(points.numbers(&mut room)) {
        MARKERS.set(thread, Markers::Told(*points));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points(gun: u16, at: &[(SlotId, [f32; 2])]) -> Points {
        Points::of(DictId::new(gun).unwrap(), at.iter().copied())
    }

    #[test]
    fn the_movie_is_told_a_gun_once_until_it_moves() {
        let still = points(3, &[(SlotId::Forend, [100.2, 50.6]), (SlotId::M24x15Muzzle, [40.0, 52.0])]);
        let (state, tell) = step(Markers::Away, Seen::AtBench(still));
        assert_eq!(tell, Some(still));
        // the movie took it: the same points again tell nothing
        let state = if tell.is_some() { Markers::Told(still) } else { state };
        assert_eq!(step(state, Seen::AtBench(still)), (state, None));
        // a pixel's move, another gun, or a slot more tell it again
        let moved = points(3, &[(SlotId::Forend, [101.0, 50.6]), (SlotId::M24x15Muzzle, [40.0, 52.0])]);
        assert_eq!(step(state, Seen::AtBench(moved)).1, Some(moved));
        let other = points(5, &[(SlotId::Forend, [100.2, 50.6]), (SlotId::M24x15Muzzle, [40.0, 52.0])]);
        assert_eq!(step(state, Seen::AtBench(other)).1, Some(other));
        // the gun leaves the bench: the next window's is told even if the same
        assert_eq!(step(state, Seen::Away), (Markers::Away, None));
        assert_eq!(step(Markers::Away, Seen::AtBench(still)).1, Some(still));
    }

    #[test]
    fn the_numbers_are_the_gun_then_each_slot_and_its_pixel() {
        let at = points(3, &[(SlotId::Forend, [100.4, 50.6]), (SlotId::PicatinnyGrip, [-3.0, 9.5])]);
        let mut room = [0.0; 1 + 3 * MAX_POINTS];
        assert_eq!(at.numbers(&mut room), [3.0, 28.0, 100.0, 51.0, 62.0, -3.0, 10.0]);
        // more slots than the room: the first MAX_POINTS
        let many: Vec<_> = (0..40).map(|n| (SlotId::Forend, [n as f32, 0.0])).collect();
        assert_eq!(points(3, &many).count, MAX_POINTS);
    }
}

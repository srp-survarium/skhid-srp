//! What the character controller stands on (patch 153, docs/notes/footsteps.md
//! "The contact"): the material of the surface its step-down sweep touched,
//! kept per controller, as 0.69d keeps it beside the support normal
//! (bullet_character_controller +612Eh, its step_down 0x2005F408C).
//!
//! 0.26g's step-down sweep (0x84FE0, the controller's helper at +210h) hands
//! its caller the point, the normal and the fraction alone. Its callback
//! (character_step_down_sweep_test_callback::addSingleResult 0x84420) sees
//! each hit's collision object and part: [`first`] and [`second`] note the
//! two hits it takes (`step_down.first`, `step_down.second`), [`result`] keeps
//! the one the sweep then answers with, or none (`step_down.result`), and a
//! cached miss is none too ([`cached`], `step_down.cached`); a cached hit
//! keeps what was kept. Read only: the sweep, its callback and the controller
//! are as the game leaves them (the material is a read of the shape's table,
//! get_triangle_material 0x3570A0), so nothing a match computes changes.

use super::context::Context;
use super::hook::Thread;
use super::step::GroundMaterial;
use super::world::{floats, read};
use super::GameCell;
use crate::raw;

/// A surface the controller touched: its material and where, the game's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touch {
    pub material: GroundMaterial,
    pub at: [f32; 3],
}

/// The two hits the callback at `callback` took in the sweep under way.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pending {
    callback: u32,
    first: Option<Touch>,
    second: Option<Touch>,
}

/// One controller's last answer (its sweep helper's address).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Kept {
    sweeper: u32,
    touch: Option<Touch>,
}

/// Players and bots of a match, and some room.
const CONTROLLERS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Touches {
    pending: Option<Pending>,
    kept: [Option<Kept>; CONTROLLERS],
    /// The slot a new controller takes when every slot is used.
    next: usize,
}

impl Touches {
    const EMPTY: Touches = Touches { pending: None, kept: [None; CONTROLLERS], next: 0 };

    fn note(&mut self, callback: u32, which: Which, touch: Touch) {
        let pending = match self.pending {
            Some(pending) if pending.callback == callback => pending,
            _ => Pending { callback, first: None, second: None },
        };
        self.pending = Some(match which {
            Which::First => Pending { first: Some(touch), ..pending },
            Which::Second => Pending { second: Some(touch), ..pending },
        });
    }

    /// The sweep of `sweeper` answered `answer` with its callback at
    /// `callback`: the touch it noted for that hit, or none.
    fn keep(&mut self, sweeper: u32, callback: u32, answer: Option<Which>) {
        let pending = self.pending.take().filter(|pending| pending.callback == callback);
        let touch = match answer {
            Some(Which::First) => pending.and_then(|pending| pending.first),
            Some(Which::Second) => pending.and_then(|pending| pending.second),
            None => None,
        };
        self.set(sweeper, touch);
    }

    fn set(&mut self, sweeper: u32, touch: Option<Touch>) {
        let at = self.kept.iter().position(|kept| kept.is_some_and(|kept| kept.sweeper == sweeper)).unwrap_or_else(|| {
            let at = self.kept.iter().position(Option::is_none).unwrap_or(self.next);
            self.next = (at + 1) % CONTROLLERS;
            at
        });
        self.kept[at] = Some(Kept { sweeper, touch });
    }

    fn get(&self, sweeper: u32) -> Option<Touch> {
        self.kept.iter().flatten().find(|kept| kept.sweeper == sweeper).and_then(|kept| kept.touch)
    }
}

static TOUCHES: GameCell<Touches> = GameCell::new(Touches::EMPTY);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Which {
    First,
    Second,
}

/// The hit the sweep answers with, as 0x85212..0x85269 choose it: the second
/// when it was taken and the first is not nearer, else the first when one
/// was taken (its fraction under 1).
fn answer(first: f32, second: Option<f32>) -> Option<Which> {
    match second {
        Some(second) if first >= second => Some(Which::Second),
        _ if first < 1.0 => Some(Which::First),
        _ => None,
    }
}

/// A hit's touch: its object's material at its part, its point in the
/// game's frame.
///
/// # Safety
/// `hit` is the callback's LocalConvexResult, its object alive.
unsafe fn touch(hit: raw::LocalConvexResult) -> Option<Touch> {
    unsafe {
        let object = hit.object()?.user()?;
        // a shape without part info: its first part, a primitive's table
        let (part, primitive) = match hit.shape_info() {
            Some(info) => (read(info.triangle()), u32::from(read(info.primitive()))),
            None => (0, 1),
        };
        let material = raw::engine_triangle_material(Some(object), part, primitive);
        let [x, y, z]: [f32; 3] = floats(hit.point());
        Some(Touch { material: GroundMaterial::of(material), at: [x, y, -z] })
    }
}

/// `step_down.first`: the callback takes a hit as its first.
pub fn first(ctx: &Context<'_>) {
    note(ctx, Which::First);
}

/// `step_down.second`: the callback takes a hit as its second.
pub fn second(ctx: &Context<'_>) {
    note(ctx, Which::Second);
}

fn note(ctx: &Context<'_>, which: Which) {
    use raw::site::step_down_first as first_site;
    use raw::site::step_down_second as second_site;
    // SAFETY: the site's hit and callback (EBX, ESI), alive for the call; the
    // two sites hold the same registers.
    let (hit, callback) = unsafe {
        match which {
            Which::First => (first_site::hit(ctx), first_site::callback(ctx)),
            Which::Second => (second_site::hit(ctx), second_site::callback(ctx)),
        }
    };
    let (Some(hit), Some(callback)) = (hit, callback) else {
        return;
    };
    // SAFETY: as above.
    let Some(touch) = (unsafe { touch(hit) }) else {
        return;
    };
    TOUCHES.update(ctx.thread(), |mut touches| {
        touches.note(callback.address(), which, touch);
        touches
    });
}

/// `step_down.result`: the sweep's answer, kept for its controller.
pub fn result(ctx: &Context<'_>) {
    use raw::site::step_down_result as site;
    // SAFETY: the site's helper (ESI) and frame (ESP: its callback at +80h).
    let (Some(sweeper), Some(frame)) = (unsafe { site::sweeper(ctx) }, unsafe { site::frame(ctx) }) else {
        return;
    };
    // SAFETY: the callback, a local of the sweep's frame, as it left it.
    let (callback, which) = unsafe {
        let callback = frame.callback();
        let second = (read(callback.second()) != 0).then(|| read(callback.second_fraction()));
        (callback.address(), answer(read(callback.fraction()), second))
    };
    TOUCHES.update(ctx.thread(), |mut touches| {
        touches.keep(sweeper.address(), callback, which);
        touches
    });
}

/// `step_down.cached`: an answer from the sweep's cache; a miss is none.
pub fn cached(ctx: &Context<'_>) {
    use raw::site::step_down_cached as site;
    // SAFETY: the site's helper (ESI) and cached entry (EAX).
    let (Some(sweeper), Some(entry)) = (unsafe { site::sweeper(ctx) }, unsafe { site::entry(ctx) }) else {
        return;
    };
    // SAFETY: the entry, the helper's.
    if unsafe { read(entry.hit()) } == 0 {
        TOUCHES.update(ctx.thread(), |mut touches| {
            touches.set(sweeper.address(), None);
            touches
        });
    }
}

/// What `controller` touched at its last step-down sweep (None: nothing, or
/// no sweep seen yet).
pub fn of(thread: &Thread, controller: raw::BulletCharacterController) -> Option<Touch> {
    // SAFETY: the helper is a member of the live controller (its address
    // only).
    let sweeper = unsafe { controller.sweeper() }.address();
    TOUCHES.get(thread).get(sweeper)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(material: u16) -> Touch {
        Touch { material: GroundMaterial::of(material), at: [1.0, 2.0, 3.0] }
    }

    #[test]
    fn the_sweeps_answer_is_its_choice() {
        assert_eq!(answer(0.4, None), Some(Which::First));
        assert_eq!(answer(1.0, None), None, "nothing taken: the fraction stays 1");
        assert_eq!(answer(0.4, Some(0.4)), Some(Which::Second), "a second as near wins (jb not taken)");
        assert_eq!(answer(0.4, Some(0.6)), Some(Which::First));
        assert_eq!(answer(1.0, Some(0.7)), Some(Which::Second));
    }

    #[test]
    fn a_controller_keeps_its_answers_touch_and_a_miss_clears_it() {
        let mut touches = Touches::EMPTY;
        touches.note(0x100, Which::First, touch(18));
        touches.note(0x100, Which::Second, touch(9));
        touches.keep(0x5000, 0x100, Some(Which::First));
        assert_eq!(touches.get(0x5000), Some(touch(18)));
        assert_eq!(touches.pending, None, "taken with the answer");
        // the next sweep: the second hit is the answer
        touches.note(0x100, Which::First, touch(18));
        touches.note(0x100, Which::Second, touch(9));
        touches.keep(0x5000, 0x100, Some(Which::Second));
        assert_eq!(touches.get(0x5000), Some(touch(9)));
        // a jump: the sweep touches nothing
        touches.keep(0x5000, 0x100, None);
        assert_eq!(touches.get(0x5000), None);
    }

    #[test]
    fn a_hit_noted_for_another_callback_is_not_the_answer() {
        let mut touches = Touches::EMPTY;
        touches.note(0x200, Which::First, touch(18));
        touches.keep(0x5000, 0x100, Some(Which::First));
        assert_eq!(touches.get(0x5000), None);
        touches.note(0x100, Which::First, touch(17));
        touches.note(0x200, Which::Second, touch(9));
        assert_eq!(touches.pending, Some(Pending { callback: 0x200, first: None, second: Some(touch(9)) }),
                   "a new callback begins anew");
    }

    #[test]
    fn controllers_are_kept_apart_and_the_table_wraps() {
        let mut touches = Touches::EMPTY;
        for sweeper in 0..CONTROLLERS as u32 + 3 {
            touches.set(sweeper, Some(touch(sweeper as u16)));
        }
        assert_eq!(touches.get(0), None, "the oldest made room");
        assert_eq!(touches.get(CONTROLLERS as u32 + 2), Some(touch(CONTROLLERS as u16 + 2)));
        touches.set(5, None);
        assert_eq!(touches.get(5), None);
        assert_eq!(touches.get(6), Some(touch(6)));
    }
}

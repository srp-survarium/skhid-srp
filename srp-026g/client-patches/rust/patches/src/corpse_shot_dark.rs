//! Patch 196: a shot into a corpse (docs/notes/corpse-shot-dark.md, the
//! owner, 2026-11-23): from 0.5 s after the ragdoll began, one shot anywhere
//! in the body plays 0.21's death sound at the corpse for everyone near, and
//! the dead player's screen goes black and the game's sounds are cut until
//! he lives again.
//!
//! - The shooter's game decides (each game draws a corpse elsewhere): its own
//!   player's bullet pushing a corpse it draws, the ragdoll 0.5 s old or more
//!   ([`hit`], from patch 76's step). Its next input free of quick-use keys
//!   says so (`weapon_pickup::corpse_shot`, the victim's place), relayed by the
//!   server as any input.
//! - Every game reads it where an input is applied ([`on_filter`]): the death
//!   it is for is the one the victim is dead of at that input's tick. Each
//!   place's [`Mark`] plays it once a death, at the next step ([`after_step`]):
//!   the sound at the corpse as that game draws it.
//! - The dead player's own game: its [`View`] black while the sound plays, then
//!   black with the game's sounds cut (patch 144's general pass gives every
//!   scene but the music's 0, the voice chat is outside the engine), while
//!   its player is dead of that death.
//!
//! Nothing a match computes: patch 78's filter takes the marker and the quick
//! bits out of every input before the game's handling.

#![forbid(unsafe_code)]

use ::ragdoll::corpse::DeathKey;
use ::ragdoll::math::Vec3;
use weapon_pickup::{CHORD, PLACES, Place, QUICK};

use crate::vostok::geometry::WorldPoint;
use crate::vostok::log::{self, Piece::{Dec, Text}};
use crate::vostok::resources::{self, Arrived, QueryResults, QueryTag, Serial};
use crate::vostok::sound::Emitter;
use crate::vostok::world::{GameWorld, NetworkClient, Player};
use crate::vostok::{ActionMask, Context, Flow, GameCell, GameWorldUi, InputHandler, Lock, NoExit, Thread};

/// A ragdoll this old or older is a corpse a shot puts in the dark, s (the
/// owner: "0.5s after ragdoll began"): the burst that killed does not.
pub const AFTER: f32 = 0.5;

/// What this game has of a place's corpse shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark {
    Unshot,
    /// An input said its corpse of this death was shot: its sound at the
    /// next step.
    Told(DeathKey),
    /// Its sound played at the step's time `at`, s.
    Heard { death: DeathKey, at: f32 },
}

/// What happens to a place's mark.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// An input applied saying its corpse of this death was shot.
    Told(DeathKey),
    /// The step at its time, s.
    Step(f32),
}

/// The place's mark after `event`, and whether the step plays the sound.
/// A death heard is never heard again ("one shot"), however often its input
/// is applied (a rollback plays it again) or repeated.
pub fn mark(before: Mark, event: Event) -> (Mark, bool) {
    match (before, event) {
        (Mark::Heard { death, .. }, Event::Told(told)) if death == told => (before, false),
        (_, Event::Told(death)) => (Mark::Told(death), false),
        (Mark::Told(death), Event::Step(at)) => (Mark::Heard { death, at }, true),
        (Mark::Unshot | Mark::Heard { .. }, Event::Step(_)) => (before, false),
    }
}

/// What the local player's world shows of it at a step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Own {
    /// No player of its own (a replay, a watcher), or one that lives.
    Living,
    Dead { place: Place, death: DeathKey },
}

impl Own {
    pub fn of(client: &NetworkClient<'_>) -> Own {
        let dead = client.local_place().and_then(Place::new).zip(client.local_player());
        match dead {
            Some((place, player)) if player.inserted() && !player.alive() => {
                Own::Dead { place, death: DeathKey(player.spawned()) }
            }
            _ => Own::Living,
        }
    }
}

/// The local player's view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Seeing,
    /// Black, 0.21's death sound playing (heard: not cut yet).
    Black,
    /// Black, the game's sounds cut.
    Dark,
}

/// How long 0.21's death sound plays at most, s (its longest take, a 2.91 s
/// scream): the cut waits for it, so the dead player hears it.
const SOUND_LENGTH: f32 = 3.0;

/// The view at a step at `now`: dark while the player is dead of the death
/// whose corpse shot was heard, from the sound's end. A respawn, a rollback
/// that undoes the death or another death ends it.
pub fn view(own: Own, marks: &[Mark; PLACES as usize], now: f32) -> View {
    let Own::Dead { place, death } = own else { return View::Seeing };
    match marks[usize::from(place.get())] {
        Mark::Heard { death: heard, at } if heard == death && now - at < SOUND_LENGTH => View::Black,
        Mark::Heard { death: heard, .. } if heard == death => View::Dark,
        Mark::Unshot | Mark::Told(_) | Mark::Heard { .. } => View::Seeing,
    }
}

/// What the shooter's game has to tell: nothing, or its bullet in a corpse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Notice {
    None,
    Shot(Place),
}

static NOTICE: GameCell<Notice> = GameCell::new(Notice::None);
static MARKS: GameCell<[Mark; PLACES as usize]> = GameCell::new([Mark::Unshot; PLACES as usize]);
static VIEW: GameCell<View> = GameCell::new(View::Seeing);
/// What the hud movie was last told: the movie and whether black.
static TOLD: GameCell<Option<(usize, bool)>> = GameCell::new(None);

/// Patch 76's step: the local player's own bullet pushed `victim`'s corpse
/// of `death`, the ragdoll `age` s old. Told in the next free input, unless
/// that death was heard already.
pub fn hit(thread: &Thread, victim: usize, death: DeathKey, age: f32, places: u32) {
    let (Some(victim), Ok(places)) = (u8::try_from(victim).ok().and_then(Place::new), u8::try_from(places)) else {
        return;
    };
    let heard = matches!(MARKS.get(thread)[usize::from(victim.get())], Mark::Heard { death: d, .. } if d == death);
    if age >= AFTER && !heard && NOTICE.get(thread) == Notice::None && weapon_pickup::allowed(victim, places) {
        NOTICE.set(thread, Notice::Shot(victim));
    }
}

/// The input builder's end (composed from artefact_carry's, after patch
/// 85's): the notice goes out in the first input with no quick-use key and
/// no command of its own.
pub fn on_input_end(handler: &mut InputHandler<'_>, thread: &Thread) {
    let actions = handler.actions().bits();
    if let Notice::Shot(victim) = NOTICE.get(thread)
        && actions & (QUICK | CHORD) == 0
    {
        handler.set_actions(ActionMask::from_bits(weapon_pickup::encode_corpse_shot(actions, victim)));
        NOTICE.set(thread, Notice::None);
    }
}

/// The input's apply point (composed from artefact_carry's `carry.filter`,
/// before patch 78 filters the actions), in every game: a corpse shot is
/// for the death its victim is dead of at this tick.
pub fn on_filter(player: &Player<'_>, thread: &Thread) {
    let Some(victim) = weapon_pickup::corpse_shot(player.input_actions()) else { return };
    let Some(dead) = artefact_carry::Place::parse(victim.get()).and_then(|place| player.carry_peer(place)) else {
        return;
    };
    if dead.inserted() && !dead.alive() {
        let death = DeathKey(dead.spawned());
        MARKS.update(thread, |mut marks| {
            let at = &mut marks[usize::from(victim.get())];
            *at = mark(*at, Event::Told(death)).0;
            marks
        });
    }
}

/// 0.21's death sound as others heard it, at the body: its body fall and
/// male scream at once, each one of its takes at random, gone by 5 m and 9 m
/// (sounds/composite/<this>.composite_sound, pack death-sound-021d).
const DEATH_SOUND: &core::ffi::CStr = c"animations/death_animation_021d_m";

/// The death sound: asked for at the first match step, kept for the process.
enum Load {
    Unasked,
    Asked,
    Ready(Emitter),
    /// Not in the game's data: no sound this session.
    Missing,
}

static SOUND: Lock<Load> = Lock::new(Load::Unasked);

/// Patch 76's step, after the simulation's move, at its time `now`: each
/// place told of plays the sound at its corpse's skull as this game draws
/// it (`skull`; none drawn, none played), and the local player's view.
pub fn after_step(thread: &Thread, world: &GameWorld<'_>, now: f32, own: Own,
                  skull: impl Fn(Place, DeathKey) -> Option<Vec3>) {
    let ask = SOUND.with(thread, |load| match load {
        Load::Unasked => {
            *load = Load::Asked;
            true
        }
        Load::Asked | Load::Ready(_) | Load::Missing => false,
    });
    if ask {
        resources::query_composite(thread, DEATH_SOUND, Serial::FIRST, crate::vostok::corpse_shot_dark_on_loaded_entry());
    }
    let mut marks = MARKS.get(thread);
    for (place, at) in marks.iter_mut().enumerate() {
        let (next, sound) = mark(*at, Event::Step(now));
        if let (true, Mark::Heard { death, .. }, Some(place)) = (sound, next, Place::new(place as u8)) {
            let point = skull(place, death).and_then(|s| WorldPoint::parse([s.x, s.y, s.z]));
            log::line(&[Text("corpse_shot_dark: place "), Dec(u32::from(place.get())), Text("'s corpse shot (death "),
                        Dec(death.0), Text(if point.is_some() { "), its sound played" } else { "), not drawn here" })]);
            if let Some(point) = point {
                SOUND.with(thread, |load| if let Load::Ready(sound) = load { world.play_sound_at(sound, point) });
            }
        }
        *at = next;
    }
    MARKS.set(thread, marks);
    VIEW.set(thread, view(own, &marks, now));
}

/// The death sound's query came back (on the game's thread, which asked).
pub fn on_loaded(results: QueryResults<'_>, tag: QueryTag) {
    let Arrived::Track(_, sound) = results.arrived(tag) else { return };
    let missing = sound.is_none();
    SOUND.with(results.thread(), |load| *load = sound.map_or(Load::Missing, Load::Ready));
    if missing {
        log::line(&[Text("corpse_shot_dark: the death sound did not load; corpse shots are silent this session")]);
    }
}

/// Whether the game's sounds are cut now (patch 144's general pass).
pub fn cut(thread: &Thread) -> bool {
    VIEW.get(thread) == View::Dark
}

/// The game unloads its match (game::clear_resources): no step runs past
/// it, so everything ends here.
pub fn on_unload(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    NOTICE.set(thread, Notice::None);
    MARKS.set(thread, [Mark::Unshot; PLACES as usize]);
    VIEW.set(thread, View::Seeing);
    Flow::Continue
}

/// update_player_icons' start, every frame of a match: the hud movie's
/// black view told when it changes, and again to another movie.
pub fn on_update_icons(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    if let Some(movie) = GameWorldUi::at_update_icons(ctx).and_then(|ui| ui.hud_movie()) {
        let black = VIEW.get(thread) != View::Seeing;
        let told = TOLD.get(thread);
        let now = Some((movie.id(), black));
        // a movie never told is not black
        let new = told.is_none_or(|(id, _)| id != movie.id());
        if told != now && ((new && !black) || movie.invoke_bool(c"root.set_dead_black", black)) {
            TOLD.set(thread, now);
        }
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    const K: DeathKey = DeathKey(7);

    #[test]
    fn one_sound_a_death_and_dark_from_its_end_until_one_lives_again() {
        let (told, sound) = mark(Mark::Unshot, Event::Told(K));
        assert_eq!((told, sound), (Mark::Told(K), false));
        let (heard, sound) = mark(told, Event::Step(10.0));
        assert_eq!((heard, sound), (Mark::Heard { death: K, at: 10.0 }, true));
        assert_eq!(mark(heard, Event::Told(K)), (heard, false), "a second shot, a rollback's");
        assert_eq!(mark(heard, Event::Step(11.0)), (heard, false));
        assert_eq!(mark(heard, Event::Told(DeathKey(9))), (Mark::Told(DeathKey(9)), false), "the next death");
        assert_eq!(mark(Mark::Unshot, Event::Step(1.0)), (Mark::Unshot, false));

        let place = Place::new(3).unwrap();
        let mut marks = [Mark::Unshot; PLACES as usize];
        marks[3] = heard;
        let dead = Own::Dead { place, death: K };
        assert_eq!(view(dead, &marks, 10.0), View::Black, "the sound heard");
        assert_eq!(view(dead, &marks, 10.0 + SOUND_LENGTH), View::Dark);
        assert_eq!(view(Own::Living, &marks, 12.0), View::Seeing, "the respawn, or a rollback");
        assert_eq!(view(Own::Dead { place, death: DeathKey(9) }, &marks, 12.0), View::Seeing, "another death");
        assert_eq!(view(Own::Dead { place: Place::new(4).unwrap(), death: K }, &marks, 12.0), View::Seeing);
        marks[3] = Mark::Told(K);
        assert_eq!(view(dead, &marks, 12.0), View::Seeing, "not heard yet");
    }
}

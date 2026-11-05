//! Patch 104: another player's correction is drawn smoothly
//! (docs/notes/remote_player_smoothing.md).
//!
//! The game draws other players at the predicted present: the world steps
//! them with their last input, and when their real input comes late it
//! re-simulates them from before it, so they jump to the new place. Here
//! the jump is measured where the world re-saves a grid point it saved
//! before (`history.grid_saved`: the old and the new place of each player at
//! the same time) and drawn away over `BLEND_MS`: the model, its held item
//! (posed from the bones) and its name tag are drawn at `new + offset`, the
//! offset falling from `old - new` to nothing.
//!
//! Presentation only: the offset is added to mt_draw's own bone array (a
//! stack copy only the draw reads) and to a copy of the eye render_name is
//! given; nothing of the world, a player or the history is written.

#![forbid(unsafe_code)]

use crate::vostok::draw::{self, DrawnBones};
use crate::vostok::types::{GameTime, Place};
use crate::vostok::world::{GameWorld, GridSave, Player, PlayerId, WorldKey};
use crate::vostok::{Context, Flow, Lock, NoExit, gun_cook};

/// How long a correction is drawn away, ms of the world's time.
const BLEND_MS: u32 = 80;
/// A correction this long or longer (m) is drawn as the game does: at once.
const SNAP_M: f32 = 1.0;
/// A correction shorter than this (m) is none (the same input played again).
const NONE_M: f32 = 0.001;
/// Grid points remembered a place: 600 ms, past the history's 500.
const KEPT: usize = 12;
const PLACES: usize = Place::COUNT;

/// A world-space displacement, metres.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Offset([f32; 3]);

impl Offset {
    const ZERO: Offset = Offset([0.0; 3]);

    fn between(from: [f32; 3], to: [f32; 3]) -> Offset {
        Offset([from[0] - to[0], from[1] - to[1], from[2] - to[2]])
    }

    fn plus(self, other: Offset) -> Offset {
        Offset([self.0[0] + other.0[0], self.0[1] + other.0[1], self.0[2] + other.0[2]])
    }

    fn scaled(self, by: f32) -> Offset {
        Offset(self.0.map(|v| v * by))
    }

    fn length(self) -> f32 {
        ragdoll::math::sqrt(self.0.iter().map(|v| v * v).sum())
    }

    /// The same displacement in a model's space whose transform's rotation
    /// rows are `transform`'s first three (row vectors: world = local x R),
    /// so a model-space translation by it moves the drawn model by `self`.
    fn in_model(self, transform: &[f32; 16]) -> [f32; 3] {
        core::array::from_fn(|row| {
            let axis = &transform[row * 4..row * 4 + 3];
            let square: f32 = axis.iter().map(|v| v * v).sum();
            if square > 0.0 { axis.iter().zip(self.0).map(|(a, o)| a * o).sum::<f32>() / square } else { 0.0 }
        })
    }
}

/// One life of a player: its spawn time (m_inserted_time_in_ms).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Life(u32);

/// Where a place's player was at a saved grid point.
#[derive(Clone, Copy, Debug)]
struct Sample {
    time: GameTime,
    life: Life,
    at: [f32; 3],
}

/// A place's grid points as last simulated, and the newest correction the
/// re-saves since the last frame found.
#[derive(Clone, Copy, Debug)]
struct Track {
    samples: [Option<Sample>; KEPT],
    next: usize,
    /// old - new at the newest grid point saved again since the last frame.
    correction: Option<(GameTime, Offset)>,
}

impl Track {
    const EMPTY: Track = Track { samples: [None; KEPT], next: 0, correction: None };

    /// The place's player at the grid point `time`: compared with that
    /// point's last save (the same life), then kept as its place there.
    fn record(&mut self, time: GameTime, life: Life, at: [f32; 3]) {
        let sample = Sample { time, life, at };
        match self.samples.iter_mut().flatten().find(|s| s.time == time) {
            Some(old) => {
                if old.life == life && self.correction.is_none_or(|(t, _)| time.elapsed_since(t).is_some()) {
                    self.correction = Some((time, Offset::between(old.at, at)));
                }
                *old = sample;
            }
            None => {
                self.samples[self.next] = Some(sample);
                self.next = (self.next + 1) % KEPT;
            }
        }
    }

    fn take_correction(&mut self) -> Option<Offset> {
        self.correction.take().map(|(_, offset)| offset).filter(|o| o.length() >= NONE_M)
    }
}

/// How a place's player is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Smoothing {
    /// Where the world has it.
    Steady,
    /// `from` away from the world's place at `started`, falling linearly to
    /// nothing over BLEND_MS, for one life.
    Blending { life: Life, from: Offset, started: GameTime },
}

/// What a frame says of a place's player.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Frame {
    /// No player, the local one, the one the camera looks from, a dead or
    /// not yet inserted one: drawn as the world has it.
    Unsmoothed,
    /// Another living player, and the correction the world's re-simulation
    /// gave it since the last frame (old place - new place).
    Remote { life: Life, correction: Option<Offset> },
}

impl Smoothing {
    /// The offset drawn at `now`.
    fn offset(self, now: GameTime) -> Offset {
        match self {
            Smoothing::Steady => Offset::ZERO,
            Smoothing::Blending { from, started, .. } => {
                let elapsed = now.elapsed_since(started).unwrap_or(0).min(BLEND_MS);
                from.scaled(1.0 - elapsed as f32 / BLEND_MS as f32)
            }
        }
    }

    /// The one transition: the state after a frame at `now`, and the offset
    /// to draw the player at.
    fn step(self, now: GameTime, frame: Frame) -> (Smoothing, Offset) {
        let Frame::Remote { life, correction } = frame else { return (Smoothing::Steady, Offset::ZERO) };
        let carried = match self {
            Smoothing::Blending { life: l, .. } if l == life => self.offset(now),
            _ => Offset::ZERO,
        };
        let next = match (self, correction) {
            (_, Some(correction)) => {
                let from = carried.plus(correction);
                let length = from.length();
                if length >= SNAP_M || length < NONE_M { Smoothing::Steady } else { Smoothing::Blending { life, from, started: now } }
            }
            (Smoothing::Blending { life: l, from, started }, None)
                if l == life && now.elapsed_since(started).unwrap_or(0) < BLEND_MS =>
            {
                Smoothing::Blending { life, from, started }
            }
            _ => Smoothing::Steady,
        };
        (next, next.offset(now))
    }
}

/// A place's offset for this frame's draw, for its player alone.
#[derive(Clone, Copy, Debug)]
struct Drawn {
    player: PlayerId,
    offset: Offset,
}

struct State {
    world: Option<WorldKey>,
    tracks: [Track; PLACES],
    smoothing: [Smoothing; PLACES],
    drawn: [Option<Drawn>; PLACES],
}

impl State {
    const EMPTY: State = State {
        world: None,
        tracks: [Track::EMPTY; PLACES],
        smoothing: [Smoothing::Steady; PLACES],
        drawn: [None; PLACES],
    };

    /// A new match's world: nothing of the last one is kept.
    fn enter(&mut self, world: Option<WorldKey>) {
        if self.world != world {
            *self = State::EMPTY;
            self.world = world;
        }
    }

    fn drawn_for(&self, player: &Player<'_>) -> Option<Offset> {
        let drawn = (*self.drawn.get(usize::from(player.place()))?)?;
        (drawn.player == player.id()).then_some(drawn.offset)
    }
}

/// The logic thread (the saves, the frame) and mt_draw's tasks (the bones)
/// reach it.
static STATE: Lock<State> = Lock::new(State::EMPTY);

fn life(player: &Player<'_>) -> Life {
    Life(player.spawned())
}

fn place_of(transform: &[f32; 16]) -> [f32; 3] {
    [transform[12], transform[13], transform[14]]
}

/// `history.grid_saved`: every player's place at the grid point just saved.
pub fn on_grid_saved(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(save) = GridSave::at(ctx) {
        let time = save.time();
        STATE.with(ctx.thread(), |state| {
            state.enter(Some(save.world()));
            save.each_player(|place, player| {
                state.tracks[usize::from(place.get())].record(time, life(&player), place_of(&player.transform()));
            });
        });
    }
    Flow::Continue
}

/// `game_world.after_move`: the frame's offsets, after the simulation's move
/// to the frame's time and before the draws.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let world = GameWorld::at_after_move(ctx);
    let key = world.as_ref().and_then(gun_cook::world_key);
    let now = world.as_ref().and_then(gun_cook::world_time).map(GameTime);
    let client = world.and_then(|world| world.network_client());
    let local = client.as_ref().and_then(|client| client.local_player()).map(|p| p.id());
    STATE.with(ctx.thread(), |state| {
        state.enter(key);
        for place in 0..PLACES {
            let correction = state.tracks[place].take_correction();
            let player = client.as_ref().and_then(|client| client.player(place as u32));
            let frame = match (&player, now) {
                (Some(p), Some(_)) if p.inserted() && p.alive() && !p.has_view() && Some(p.id()) != local => {
                    Frame::Remote { life: life(p), correction }
                }
                _ => Frame::Unsmoothed,
            };
            let (next, offset) = state.smoothing[place].step(now.unwrap_or_default(), frame);
            state.smoothing[place] = next;
            state.drawn[place] = player.filter(|_| offset != Offset::ZERO).map(|p| Drawn { player: p.id(), offset });
        }
    });
    Flow::Continue
}

/// `player_draw.bones` (after the ragdoll's capture): every bone of a
/// smoothed player moved by its offset, in its model's space, so the body,
/// the held item mt_draw poses from the Weapon bone, its muzzle and the
/// shadow all follow.
pub fn on_bones(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(bones) = DrawnBones::at(ctx) {
        let player = bones.player();
        if let Some(offset) = STATE.with(ctx.thread(), |state| state.drawn_for(&player)) {
            let by = offset.in_model(&player.transform());
            for index in 0..bones.count() {
                if let Some(mut matrix) = bones.get(index) {
                    matrix[12] += by[0];
                    matrix[13] += by[1];
                    matrix[14] += by[2];
                    bones.set(index, matrix);
                }
            }
        }
    }
    Flow::Continue
}

/// `name_tag.eye`: a smoothed player's name tag placed from its eye moved
/// by the offset.
pub fn on_name_tag(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(player) = draw::name_tag_player(ctx) else { return Flow::Continue };
    let Some(offset) = STATE.with(ctx.thread(), |state| state.drawn_for(&player)) else { return Flow::Continue };
    let mut eye = player.eye_transform();
    eye[12] += offset.0[0];
    eye[13] += offset.0[1];
    eye[14] += offset.0[2];
    draw::place_name_tag_from(ctx, eye)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIFE: Life = Life(1000);

    fn at(ms: u32) -> GameTime {
        GameTime(ms)
    }

    fn remote(correction: Option<[f32; 3]>) -> Frame {
        Frame::Remote { life: LIFE, correction: correction.map(Offset) }
    }

    fn close(a: Offset, b: [f32; 3]) -> bool {
        a.0.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    #[test]
    fn a_correction_is_drawn_away_over_the_blend() {
        let (s, o) = Smoothing::Steady.step(at(5000), remote(Some([0.4, 0.0, -0.2])));
        assert!(close(o, [0.4, 0.0, -0.2]), "{o:?}");
        let (s, o) = s.step(at(5040), remote(None));
        assert!(close(o, [0.2, 0.0, -0.1]), "{o:?}");
        let (s, o) = s.step(at(5079), remote(None));
        assert!(o.length() < 0.01);
        let (s, o) = s.step(at(5080), remote(None));
        assert_eq!((s, o), (Smoothing::Steady, Offset::ZERO));
    }

    #[test]
    fn a_large_correction_snaps() {
        assert_eq!(Smoothing::Steady.step(at(100), remote(Some([1.0, 0.0, 0.0]))), (Smoothing::Steady, Offset::ZERO));
        assert_eq!(Smoothing::Steady.step(at(100), remote(Some([0.0, 0.8, 0.8]))), (Smoothing::Steady, Offset::ZERO));
        let (s, _) = Smoothing::Steady.step(at(100), remote(Some([0.99, 0.0, 0.0])));
        assert!(matches!(s, Smoothing::Blending { .. }));
    }

    #[test]
    fn a_second_correction_adds_to_what_is_left() {
        let (s, _) = Smoothing::Steady.step(at(0), remote(Some([0.4, 0.0, 0.0])));
        // half way, a second correction: 0.2 left + 0.3, drawn from there anew
        let (s, o) = s.step(at(40), remote(Some([0.3, 0.0, 0.0])));
        assert!(close(o, [0.5, 0.0, 0.0]), "{o:?}");
        let (_, o) = s.step(at(80), remote(None));
        assert!(close(o, [0.25, 0.0, 0.0]), "{o:?}");
        // one that would carry it to a metre or more snaps
        let (s, _) = Smoothing::Steady.step(at(0), remote(Some([0.6, 0.0, 0.0])));
        assert_eq!(s.step(at(10), remote(Some([0.5, 0.0, 0.0]))), (Smoothing::Steady, Offset::ZERO));
        // one that cancels what is left ends the blend
        let (s, _) = Smoothing::Steady.step(at(0), remote(Some([0.4, 0.0, 0.0])));
        assert_eq!(s.step(at(40), remote(Some([-0.2, 0.0, 0.0]))), (Smoothing::Steady, Offset::ZERO));
    }

    #[test]
    fn death_and_a_new_life_snap() {
        let (s, _) = Smoothing::Steady.step(at(0), remote(Some([0.4, 0.0, 0.0])));
        assert_eq!(s.step(at(20), Frame::Unsmoothed), (Smoothing::Steady, Offset::ZERO));
        let (s, _) = Smoothing::Steady.step(at(0), remote(Some([0.4, 0.0, 0.0])));
        let respawned = Frame::Remote { life: Life(4000), correction: None };
        assert_eq!(s.step(at(20), respawned), (Smoothing::Steady, Offset::ZERO));
        // a correction of the new life starts from nothing of the old one
        let respawned = Frame::Remote { life: Life(4000), correction: Some(Offset([0.1, 0.0, 0.0])) };
        let (_, o) = s.step(at(20), respawned);
        assert!(close(o, [0.1, 0.0, 0.0]), "{o:?}");
    }

    #[test]
    fn the_clock_wraps_and_goes_back() {
        let start = at(u32::MAX - 20);
        let (s, _) = Smoothing::Steady.step(start, remote(Some([0.4, 0.0, 0.0])));
        let (s, o) = s.step(start.after(40), remote(None));
        assert!(close(o, [0.2, 0.0, 0.0]), "{o:?}");
        assert_eq!(s.step(start.after(80), remote(None)), (Smoothing::Steady, Offset::ZERO));
        // a clock set back is no time passed
        let (s, _) = Smoothing::Steady.step(at(1000), remote(Some([0.4, 0.0, 0.0])));
        let (_, o) = s.step(at(990), remote(None));
        assert!(close(o, [0.4, 0.0, 0.0]), "{o:?}");
    }

    #[test]
    fn a_track_finds_the_newest_resaved_point() {
        let mut track = Track::EMPTY;
        for (n, t) in (50..=500).step_by(50).enumerate() {
            track.record(at(t), LIFE, [n as f32, 0.0, 0.0]);
        }
        assert_eq!(track.take_correction(), None, "first saves are no correction");
        // a rollback saves 350..500 again, the player 0.25 m further on
        for (n, t) in (350..=500).step_by(50).enumerate() {
            track.record(at(t), LIFE, [6.0 + n as f32 + 0.25 * (n + 1) as f32, 0.0, 0.0]);
        }
        let c = track.take_correction().unwrap();
        assert!(close(c, [-1.0, 0.0, 0.0]), "old 9, new 10 at 500: {c:?}");
        assert_eq!(track.take_correction(), None);
        // the same inputs played again: no correction
        track.record(at(500), LIFE, [10.0, 0.0, 0.0]);
        assert_eq!(track.take_correction(), None);
        // an older point saved after a newer one does not replace it
        track.record(at(500), LIFE, [10.5, 0.0, 0.0]);
        track.record(at(450), LIFE, [0.0, 0.0, 0.0]);
        assert!(close(track.take_correction().unwrap(), [-0.5, 0.0, 0.0]));
        // another life at a point is no correction
        track.record(at(500), Life(7), [3.0, 0.0, 0.0]);
        assert_eq!(track.take_correction(), None);
    }

    #[test]
    fn the_offset_moves_the_model_in_its_own_space() {
        // a model turned 90 degrees about the up axis: its x row is world -z
        let turned = [0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 5.0, 0.0, 5.0, 1.0];
        let local = Offset([0.0, 0.0, -0.3]).in_model(&turned);
        assert!(local.iter().zip([0.3, 0.0, 0.0]).all(|(a, b)| (a - b).abs() < 1e-6), "{local:?}");
        // back to the world: local x R
        let world: [f32; 3] =
            core::array::from_fn(|c| (0..3).map(|r| local[r] * turned[r * 4 + c]).sum());
        assert!(world.iter().zip([0.0, 0.0, -0.3]).all(|(a, b)| (a - b).abs() < 1e-6));
        assert!((Offset([3.0, 4.0, 0.0]).length() - 5.0).abs() < 1e-5);
    }
}

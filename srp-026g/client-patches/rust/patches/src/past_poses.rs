//! Patch 105: other players drawn in the past, and a bullet's hit rewound
//! to what its shooter saw (the owner, 2026-11-05: piece 2 of
//! pixel_correct_shooting_069.md section 8; the spec docs/notes/past_poses.md).
//!
//! - **The ring** (world state): at the end of every grid step
//!   (`core.ticked`), each place's living player's feet go into his place's
//!   ring of the last four grid points (`rewind::Ring`).
//! - **The declared age** (world state): a game declares how old it draws
//!   the others in the byte of a kit choice (0xDE, relayed as 0xF3; a byte of
//!   3 or more is no kit, the place's tick leaves the next kit as it was). At
//!   the same step's end the place's newest kit choice at or before it is
//!   taken (`rewind::Declared::step`): a declaration is in force for the
//!   place's shots fired after its tick, the one before for those fired at
//!   or before it.
//! - Both are the place's record (`rewind::Record`), written after his
//!   player state (`carry.save`, after patch 78's carry) and read back
//!   (`carry.load`): in every history item, the hash and an uploaded world.
//! - **The rewind** is patch 103's hit test (`subtick_hits`), which asks
//!   [`rewind`] for a target's ring and the age his shooter declared, capped
//!   by the match ([`rewind::Cap`], srp's block of the static info, 150 ms).
//! - **What this game draws** (display only): every other living player at
//!   now less the age the player the camera looks from declared, from the
//!   ring (`game_world.after_move` works out the offsets, `player_draw.bones`
//!   moves his bones, so his held item, muzzle and shadow follow,
//!   `name_tag.eye` his tag). The player the camera looks from is drawn as
//!   the game does, at the present.
//! - **The age this game declares** (client only): how late each other
//!   place's inputs reach it (`change_input.player`), the second latest of
//!   its last 16, and 10 ms; the most of the places', capped, rounded up to
//!   a level (15 ms). Declared when it is more than in force, or two levels
//!   less, at most once a second, while the local player is alive.

#![forbid(unsafe_code)]

use rewind::{Age, Cap, Declaration, Knot, Level, Path, Record, Ring, Time};

use crate::vostok::draw::{self, DrawnBones};
use crate::vostok::log::{self, Piece::{Dec, Text}};
use crate::vostok::past::{self, CoreTick};
use crate::vostok::ray_test::Shot;
use crate::vostok::types::{GameTime, Place};
use crate::vostok::world::{GameWorld, PlayerId};
use crate::vostok::{Context, Flow, InputChange, Lock, NoExit, OptionsRead, Thread, gun_cook, srp_entries};

const PLACES: usize = Place::COUNT;
/// A place's lateness samples kept.
const LATE_KEPT: usize = 16;
/// A place whose last input is older than this says nothing of now.
const LATE_FRESH_MS: u32 = 5_000;
/// A sample later than this is no input's way here (a load, a pause).
const LATE_MOST_MS: u32 = 2_000;
/// Above the estimate, for the jitter past it.
const MARGIN_MS: u32 = 10;
/// A game declares at most this often.
const DECLARE_EVERY_MS: u32 = 1_000;
/// No declaration this near a kit choice of this game's own place.
const KIT_CLEAR_MS: u32 = 100;
/// A declared age is lowered only to one this many levels younger or more.
const LOWER_LEVELS: u8 = 2;

type Vec3 = [f32; 3];

/// How late a place's inputs reach this game (client only).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Lateness {
    Unheard,
    /// The last `count` samples (ms) in a ring at `next`, the newest input's
    /// time and the world's newest time it came at.
    Heard { samples: [u16; LATE_KEPT], count: u8, next: u8, input: GameTime, at: GameTime },
}

impl Lateness {
    /// An input of the place, of time `input`, reaching this game when its
    /// world was at `at`. One older than the newest heard (a copy sent
    /// again, an input moved back) says nothing of the way.
    fn heard(self, input: GameTime, at: GameTime) -> Lateness {
        let late = at.since(input);
        if late > LATE_MOST_MS {
            return self;
        }
        let (mut samples, count, next) = match self {
            Lateness::Unheard => ([0; LATE_KEPT], 0, 0),
            Lateness::Heard { input: newest, .. } if input.not_after(newest) => return self,
            Lateness::Heard { samples, count, next, .. } => (samples, count, next),
        };
        samples[usize::from(next)] = late as u16;
        let count = (count + 1).min(LATE_KEPT as u8);
        Lateness::Heard { samples, count, next: (next + 1) % LATE_KEPT as u8, input, at }
    }

    /// How late its inputs come now: the second latest of the samples kept
    /// (about their 90th percentile; the one of a single sample), none for a
    /// place unheard for LATE_FRESH_MS.
    fn estimate(self, now: GameTime) -> Option<u32> {
        let Lateness::Heard { samples, count, at, .. } = self else { return None };
        if now.since(at) > LATE_FRESH_MS {
            return None;
        }
        let (mut first, mut second) = (0, 0);
        for &sample in &samples[..usize::from(count)] {
            if sample > first {
                (first, second) = (sample, first);
            } else if sample > second {
                second = sample;
            }
        }
        Some(u32::from(if count > 1 { second } else { first }))
    }
}

/// The age this game wants to draw the others at: the latest of the places'
/// estimates and the margin, capped, rounded up to a level. None: no place
/// heard of.
fn wanted(estimates: impl Iterator<Item = u32>, cap: Cap) -> Option<Level> {
    let latest = estimates.max()?;
    let age = Age::new(latest + MARGIN_MS).unwrap_or(Age::MAX).capped(cap);
    Some(Level::at_least(age))
}

/// When this game last declared an age (client only).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sender {
    Quiet,
    Declared { at: GameTime },
}

impl Sender {
    /// The one transition, once a frame at `now`: the local player alive or
    /// not, the level this game wants and the one in force for its place in
    /// the world. A declaration to send, if any.
    fn step(self, now: GameTime, alive: bool, wanted: Option<Level>, in_force: Level) -> (Sender, Option<Declaration>) {
        let ready = match self {
            Sender::Quiet => true,
            Sender::Declared { at } => now.since(at) >= DECLARE_EVERY_MS,
        };
        let Some(wanted) = wanted.filter(|_| alive && ready) else { return (self, None) };
        let lower = in_force.younger(LOWER_LEVELS).is_some_and(|younger| younger >= wanted);
        if wanted > in_force || lower {
            (Sender::Declared { at: now }, Some(Declaration::every(wanted)))
        } else {
            (self, None)
        }
    }
}

/// A place's player drawn back this frame, for that player alone.
#[derive(Clone, Copy, Debug)]
struct Drawn {
    player: PlayerId,
    offset: Vec3,
}

struct State {
    cap: Cap,
    /// World state: each place's record.
    places: [Record; PLACES],
    /// Client only.
    late: [Lateness; PLACES],
    sender: Sender,
    drawn: [Option<Drawn>; PLACES],
}

impl State {
    const fn new(cap: Cap) -> State {
        State {
            cap,
            places: [Record::EMPTY; PLACES],
            late: [Lateness::Unheard; PLACES],
            sender: Sender::Quiet,
            drawn: [None; PLACES],
        }
    }
}

/// The logic thread (the steps, the saves and loads, the frame, the
/// inputs), the bullet manager's (the hit test) and mt_draw's tasks (the
/// bones) reach it.
static STATE: Lock<State> = Lock::new(State::new(Cap::DEFAULT));

fn time(at: GameTime) -> Time {
    Time(at.0)
}

fn place_of(place: Place) -> rewind::Place {
    rewind::Place::new(place.get()).expect("both are 0..20")
}

/// A match's (or a replay's) options read: nothing of the last match is
/// kept, the cap is the options' (srp's block, tag 3), else 150 ms.
fn options_read(thread: &Thread, read: Option<OptionsRead<'_>>) -> Flow<NoExit> {
    let cap = read
        .and_then(|read| srp_entries(read.rest()).filter(|(tag, _)| *tag == Cap::TAG).find_map(|(_, e)| Cap::from_entry(e)))
        .unwrap_or(Cap::DEFAULT);
    STATE.with(thread, |state| *state = State::new(cap));
    Flow::Continue
}

/// process_static_match_info after its deserialize.
pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_static_info(ctx))
}

/// A replay's chunk 1 after its deserialize.
pub fn on_replay_options(ctx: &mut Context<'_>) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_replay(ctx))
}

/// What a grid step's end gives a place in the world: its living player's
/// feet, and its newest kit choice at or before the step (time, byte).
#[derive(Clone, Copy)]
struct PlaceStep {
    feet: Option<Vec3>,
    choice: Option<(Time, u8)>,
}

/// `core.ticked`: a step's end. On the grid, each place's player's feet
/// into its ring and its newest kit choice into its declared age.
pub fn on_ticked(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(tick) = CoreTick::at(ctx) else { return Flow::Continue };
    let now = tick.time();
    if !time(now).on_grid() {
        return Flow::Continue;
    }
    let mut steps: [Option<PlaceStep>; PLACES] = [None; PLACES];
    tick.each_place(|place, player| {
        let feet = player.filter(|p| p.alive() && p.inserted()).map(|p| p.feet());
        let choice = tick.newest_kit_choice(place, now).map(|(at, value)| (time(at), value));
        steps[place.index()] = Some(PlaceStep { feet, choice });
    });
    STATE.with(ctx.thread(), |state| {
        for (record, step) in state.places.iter_mut().zip(steps) {
            let Some(PlaceStep { feet, choice }) = step else { continue };
            if let Some(feet) = feet {
                record.ring.record(time(now), feet);
            }
            record.declared = record.declared.step(choice);
        }
    });
    Flow::Continue
}

/// `carry.save` (after patch 78's carry): the player's place's record.
pub fn on_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(place) = past::saved_player(ctx).and_then(|p| Place::new(u32::from(p.place()))) else {
        return Flow::Continue;
    };
    let bytes = STATE.with(ctx.thread(), |state| state.places[place.index()].encode());
    past::write_record(ctx, &bytes);
    Flow::Continue
}

/// `carry.load` (after patch 78's carry): the place's record as the world
/// has it; none (a world with none) is an empty one, logged.
pub fn on_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some((player, bytes)) = past::loaded_record::<{ Record::SIZE }>(ctx) else { return Flow::Continue };
    let Some(place) = Place::new(u32::from(player.place())) else { return Flow::Continue };
    let record = bytes.and_then(|bytes| Record::decode(&bytes));
    if record.is_none() {
        log::line(&[Text("past_poses: place "), Dec(u32::from(place.get())), Text("'s record does not decode")]);
    }
    STATE.with(ctx.thread(), |state| state.places[place.index()] = record.unwrap_or(Record::EMPTY));
    Flow::Continue
}

/// `change_input.player` (after patch 99's check): how late another
/// place's input reaches this game.
pub fn on_input(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(change) = InputChange::at(ctx) else { return Flow::Continue };
    let Some((place, at)) = change.arrival() else { return Flow::Continue };
    let input = GameTime(change.time());
    STATE.with(ctx.thread(), |state| state.late[place.index()] = state.late[place.index()].heard(input, at));
    Flow::Continue
}

/// What [`rewind`] gives the hit test for a target.
pub struct Rewind {
    /// The age the shooter declared for him when the shot was fired, capped.
    pub age: Age,
    pub ring: Ring,
}

/// The hit test's rewind of `target` for `shot` (none: a bullet without a
/// shooter in the world, age 0).
pub fn rewind(thread: &Thread, shot: Option<Shot>, target: Place) -> Rewind {
    STATE.with(thread, |state| state.rewind(shot, target))
}

impl State {
    fn rewind(&self, shot: Option<Shot>, target: Place) -> Rewind {
        let age = shot.map_or(Age::ZERO, |shot| {
            self.places[shot.shooter.index()].declared.age_for(time(shot.fired), place_of(target)).capped(self.cap)
        });
        Rewind { age, ring: self.places[target.index()].ring }
    }
}

/// `game_world.after_move`, once a frame after the simulation's move to the
/// frame's time: what this game declares, and where it draws each other
/// player.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Some(world) = GameWorld::at_after_move(ctx) else { return Flow::Continue };
    let Some(now) = gun_cook::world_time(&world).map(GameTime) else { return Flow::Continue };
    let client = world.network_client();
    let local = client.as_ref().and_then(|client| client.local_player());
    // each place's player: drawn back unless the camera looks from him
    let mut players = [None; PLACES];
    let mut view = None;
    if let Some(client) = &client {
        for place in Place::all() {
            let Some(player) = client.player(u32::from(place.get())) else { continue };
            if player.has_view() {
                view = Some(place);
                continue;
            }
            if local.is_some_and(|l| l.id() == player.id()) {
                continue;
            }
            if player.alive() && player.inserted() {
                players[place.index()] = Some((player.id(), player.feet(), GameTime(player.spawned())));
            }
        }
    }
    let alive = local.is_some_and(|l| l.alive() && l.inserted());
    let local_place = local.and_then(|l| Place::new(u32::from(l.place())));
    // the camera's player: the local one, or the one a spectator watches
    let view = view.or(local_place);
    let declaration = STATE.with(thread, |state| {
        let cap = state.cap;
        let age_of = |place: Place| {
            state.places[place.index()].declared.age_for(time(now), place_of(place)).capped(cap)
        };
        // drawn at the age the view's player declared (the one his next shot
        // is tested at)
        let drawn_age = view.map_or(Age::ZERO, age_of);
        for (place, player) in players.iter().enumerate() {
            state.drawn[place] = player.and_then(|(player, feet, life)| {
                let path = Path::new(&state.places[place].ring, time(now), feet, Knot::NOW, time(life));
                let back = path.back_at(drawn_age.ms() as f32);
                (back != [0.0; 3]).then_some(Drawn { player, offset: back.map(|v| -v) })
            });
        }
        let local_place = local_place?;
        let estimates = state.late.iter().enumerate().filter(|(p, _)| *p != local_place.index());
        let wanted = wanted(estimates.filter_map(|(_, late)| late.estimate(now)), cap);
        let in_force = Level::at_least(age_of(local_place));
        let (sender, declaration) = state.sender.step(now, alive, wanted, in_force);
        state.sender = sender;
        declaration
    });
    // never in the tick of a kit pick of this place (the one node of a time
    // would keep only one of the two): a pick wins, the age goes again later
    let picking = world.newest_own_kit_choice().is_some_and(|at| at.since(now).min(now.since(at)) < KIT_CLEAR_MS);
    if let Some(declaration) = declaration.filter(|_| !picking) {
        world.choose_kit_byte(declaration.encode());
    }
    Flow::Continue
}

/// A world-space displacement in a model's space whose transform's rotation
/// rows are `transform`'s first three (row vectors: world = local x R), so a
/// model-space translation by it moves the drawn model by `offset`.
fn in_model(offset: Vec3, transform: &[f32; 16]) -> Vec3 {
    core::array::from_fn(|row| {
        let axis = &transform[row * 4..row * 4 + 3];
        let square: f32 = axis.iter().map(|v| v * v).sum();
        if square > 0.0 { axis.iter().zip(offset).map(|(a, o)| a * o).sum::<f32>() / square } else { 0.0 }
    })
}

fn drawn_for(thread: &Thread, place: u8, player: PlayerId) -> Option<Vec3> {
    STATE.with(thread, |state| {
        let drawn = (*state.drawn.get(usize::from(place))?)?;
        (drawn.player == player).then_some(drawn.offset)
    })
}

/// `player_draw.bones` (after the ragdoll's capture): every bone of a player
/// drawn back moved by his offset, in his model's space, so his body, the
/// held item mt_draw poses from the Weapon bone, its muzzle and his shadow
/// follow.
pub fn on_bones(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(bones) = DrawnBones::at(ctx) {
        let player = bones.player();
        if let Some(offset) = drawn_for(ctx.thread(), player.place(), player.id()) {
            let by = in_model(offset, &player.transform());
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

/// `name_tag.eye`: a player drawn back has his tag placed from his eye moved
/// by his offset.
pub fn on_name_tag(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(player) = draw::name_tag_player(ctx) else { return Flow::Continue };
    let Some(offset) = drawn_for(ctx.thread(), player.place(), player.id()) else { return Flow::Continue };
    let mut eye = player.eye_transform();
    eye[12] += offset[0];
    eye[13] += offset[1];
    eye[14] += offset[2];
    draw::place_name_tag_from(ctx, eye)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rewind::Declared;

    fn at(ms: u32) -> GameTime {
        GameTime(ms)
    }

    fn level(ms: u32) -> Level {
        Level::at_least(Age::new(ms).unwrap())
    }

    fn heard(samples: &[u32]) -> Lateness {
        samples.iter().enumerate().fold(Lateness::Unheard, |late, (n, &ms)| {
            let input = at(1000 + 50 * n as u32);
            late.heard(input, input.after(ms))
        })
    }

    #[test]
    fn lateness_is_the_second_latest_of_the_last_sixteen() {
        assert_eq!(Lateness::Unheard.estimate(at(0)), None);
        let one = heard(&[80]);
        assert_eq!(one.estimate(at(1100)), Some(80));
        let some = heard(&[80, 90, 300, 70, 85]);
        assert_eq!(some.estimate(at(1300)), Some(90), "one spike is not the estimate");
        // the seventeenth replaces the first
        let mut many = [60; 17];
        many[0] = 200;
        many[1] = 190;
        assert_eq!(heard(&many).estimate(at(2000)), Some(60), "200 gone: 190 and fifteen of 60 kept");
        // stale
        assert_eq!(some.estimate(at(1300 + LATE_FRESH_MS + 300)), None);
        // a copy of an input heard again, or an older one, is no sample
        let late = heard(&[80]);
        assert_eq!(late.heard(at(1000), at(1500)), late);
        assert_eq!(late.heard(at(900), at(1500)), late);
        // nor a load's pause
        assert_eq!(late.heard(at(1100), at(1100 + LATE_MOST_MS + 1)), late);
    }

    #[test]
    fn the_wanted_age_is_the_latest_place_capped_and_rounded_up() {
        assert_eq!(wanted([].into_iter(), Cap::DEFAULT), None);
        assert_eq!(wanted([60, 84].into_iter(), Cap::DEFAULT), Some(level(94)), "84 + 10, rounded to 105");
        assert_eq!(level(94).age(), Age::new(105).unwrap());
        assert_eq!(wanted([400].into_iter(), Cap::DEFAULT), Some(level(150)));
        assert_eq!(wanted([400].into_iter(), Cap::from_entry(&[100]).unwrap()), Some(level(100)));
        assert_eq!(wanted([0].into_iter(), Cap::DEFAULT), Some(level(10)));
    }

    #[test]
    fn a_game_declares_up_at_once_and_down_by_two_levels_once_a_second() {
        let (s, d) = Sender::Quiet.step(at(5000), true, Some(level(90)), level(0));
        assert_eq!(d, Some(Declaration::every(level(90))));
        // within the second: nothing, whatever it wants
        assert_eq!(s.step(at(5500), true, Some(level(150)), level(90)), (s, None));
        // more: at once (after the second)
        let (s, d) = s.step(at(6000), true, Some(level(105)), level(90));
        assert_eq!(d, Some(Declaration::every(level(105))));
        // one level less: kept; two: lowered
        assert_eq!(s.step(at(7000), true, Some(level(90)), level(105)).1, None);
        assert_eq!(s.step(at(7000), true, Some(level(75)), level(105)).1, Some(Declaration::every(level(75))));
        // dead, or no place heard: nothing
        assert_eq!(Sender::Quiet.step(at(0), false, Some(level(90)), level(0)), (Sender::Quiet, None));
        assert_eq!(Sender::Quiet.step(at(0), true, None, level(90)), (Sender::Quiet, None));
        // what is in force already: nothing
        assert_eq!(Sender::Quiet.step(at(0), true, Some(level(90)), level(90)), (Sender::Quiet, None));
    }

    #[test]
    fn the_offset_moves_the_model_in_its_own_space() {
        // a model turned 90 degrees about the up axis: its x row is world -z
        let turned = [0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 5.0, 0.0, 5.0, 1.0];
        let local = in_model([0.0, 0.0, -0.3], &turned);
        assert!(local.iter().zip([0.3, 0.0, 0.0]).all(|(a, b)| (a - b).abs() < 1e-6), "{local:?}");
    }

    #[test]
    fn a_declared_age_and_the_cap_reach_the_hit_test() {
        let mut state = State::new(Cap::from_entry(&[100]).unwrap());
        state.places[2].declared = Declared::Since { before: Age::ZERO, now: Age::MAX, at: Time(5000) };
        state.places[7].ring.record(Time(4950), [1.0, 2.0, 3.0]);
        let shooter = Place::new(2).unwrap();
        let target = Place::new(7).unwrap();
        let shot = |fired| Some(Shot { shooter, fired: at(fired) });
        assert_eq!(state.rewind(shot(5050), target).age, Age::new(100).unwrap(), "150 declared, capped at 100");
        assert_eq!(state.rewind(shot(4990), target).age, Age::ZERO, "fired before the declaration");
        assert_eq!(state.rewind(None, target).age, Age::ZERO, "no shooter");
        assert_eq!(state.rewind(shot(5050), target).ring, state.places[7].ring, "the target's ring");
    }
}

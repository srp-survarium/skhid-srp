//! Patch 142: a free-for-all, each player his own side (the owner, 2026-11-01
//! and 2026-11-11; docs/notes/ffa.md). srp gives each player of a
//! free-for-all a team of his own (4 + his place); the game's team code knows
//! two teams, and its hud two sides, so:
//!
//! - **Respawn:** a player of his own side respawns alone, at a point of the
//!   map's team-2 group (0.31's free-for-all points, content edit ffa-1).
//!   Every point that passes the game's own clear test (no living player
//!   inside the enemy radius, 20 m) and has no point given
//!   earlier at this tick inside that radius is eligible. A deterministic
//!   integer mix of match time and player place chooses from that whole
//!   pool: safe central points compete equally with safe edge points.
//!   With no safe point, choose among the five best-separated points from
//!   the nearest living or newly spawned player. No unsynchronized randomness or retained state:
//!   clients, late joiners and replays choose from the same world's inputs.
//! - **Resupply:** every ammo zone serves him (TDM's maps have a zone of each
//!   team's).
//! - **The kill target:** srp's block after the match's options holds tag 3,
//!   `kills_to_win` (u16), in a free-for-all only. The hud movie is told
//!   `root.set_ffa_progress(own, top, target)`: the kills of the place the
//!   hud shows (the network client's player: the local one, in a replay the
//!   one watched), the most kills of any place, and the target; at each
//!   change, and again after the hud's game type is set.
//! - **The messages:** the welcome text at the match's start
//!   (`st_start_match_welcome_message_8`, with the target), and at its end
//!   (the target reached, or the exact time limit) the victory or defeat text.
//! - **Completion:** the native KD death handler counts the winning kill
//!   and its victim's death, then ignores subsequent death events. At the
//!   time limit it ignores events from the deadline onward. The serialized
//!   counters own the terminal standing; loading an earlier world restores
//!   scoring without a second process-local completion flag.
//!
//! A match of teams 0 and 1 is untouched: every hook passes it on.

#![forbid(unsafe_code)]

use core::cmp::Ordering;
use core::ffi::CStr;
use core::num::NonZeroU16;

use crate::vostok::parse::Reader;
use crate::vostok::types::Place;
use crate::vostok::{
    respawn, Context, Due, Flow, FreeForAllOnCanUseExit, FreeForAllOnSelectExit, FreeForAllOnUseInfoExit, GameCell,
    GameWorldUi, MatchClock, NoExit, OptionsRead, Point, RespawnSelect, ResupplySite, Side, Thread,
    FreeForAllOnKillEventExit,
};

/// The tick's vectors hold 20 players (a match's places).
const MAX_DUE: usize = Place::COUNT;
/// The living players a selection measures from.
const MAX_LIVING: usize = Place::COUNT;
/// The points a selection considers (0.31's maps have 22 to 30).
const MAX_POINTS: usize = 64;
/// A crowded map still offers several of its best-separated points.
const FALLBACK_POINTS: usize = 5;

/// The squared distance, summed x, y, z in this order. Single precision
/// subtractions, products and sums only: each is rounded the same on every
/// processor, so every game ranks the points alike.
fn distance_squared(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}

/// The squared distance from `from` to the nearest of `others`; infinite
/// with none.
fn nearest(from: [f32; 3], others: impl Iterator<Item = [f32; 3]>) -> f32 {
    others.fold(f32::INFINITY, |least, other| {
        let distance = distance_squared(from, other);
        if distance < least { distance } else { least }
    })
}

/// Choose among all clear points using only synchronized match inputs. Mix the
/// milliseconds before reducing to the pool size: a simple tick modulo count
/// locks regular respawns to a subset (e.g. 5 s = 100 ticks with 20 points).
/// Explicit wrapping u32 arithmetic gives clients and replays identical choices.
fn turn(time: u32, place: u8, count: usize) -> usize {
    // Hash constants in halves, as in memory_profile: the layer check
    // reserves address-sized literals for generated engine bindings.
    const PLACE_MIX: u32 = 0x9e37 << 16 | 0x79b9;
    const MIX_A: u32 = 0x7feb << 16 | 0x352d;
    const MIX_B: u32 = 0x846c << 16 | 0xa68b;
    let mut mixed = time.wrapping_add(u32::from(place).wrapping_mul(PLACE_MIX));
    mixed = (mixed ^ (mixed >> 16)).wrapping_mul(MIX_A);
    mixed = (mixed ^ (mixed >> 15)).wrapping_mul(MIX_B);
    ((mixed ^ (mixed >> 16)) as usize) % count
}

/// For each due player in order, the point he respawns at; returns how many
/// of `chosen` are set (every due player, unless the map has no point).
///
/// A point's rank is the squared distance to the nearest of `living` and of
/// the points chosen before at this tick. A point passes when it is not
/// chosen yet, no chosen point is inside its enemy radius, and `clear` (the
/// game's test against the living) holds for the player's side. He takes one
/// of all points that pass, by [`turn`], without ranking safe points by
/// distance. With none, he chooses among the five best-separated points;
/// equal fallback ranks go by the point's index. `points` is bounded by `MAX_POINTS` at the boundary.
fn plan(
    due: &[Due],
    points: &[Point],
    living: &[[f32; 3]],
    time: u32,
    clear: impl Fn(usize, Side) -> bool,
    chosen: &mut [usize; MAX_DUE],
) -> usize {
    if points.is_empty() {
        return 0;
    }
    let mut count = 0;
    for player in due.iter().take(MAX_DUE) {
        let given = &chosen[..count];
        let rank = |j: usize| {
            nearest(points[j].position, living.iter().copied().chain(given.iter().map(|&k| points[k].position)))
        };
        let passes = |j: usize| {
            let radius = points[j].enemy_radius * points[j].enemy_radius;
            given.iter().all(|&k| k != j && distance_squared(points[k].position, points[j].position) >= radius)
                && clear(j, player.side)
        };
        // Keep every clear point in stable map order. Distance matters only
        // for the fallback; equal fallback ranks keep the lower index.
        let mut fallback = [(0usize, 0.0f32); FALLBACK_POINTS];
        let mut fallback_count = 0;
        let mut safe = [0usize; MAX_POINTS];
        let mut held = 0;
        for j in 0..points.len() {
            let ranked = (j, rank(j));
            let insert = fallback[..fallback_count].iter()
                .position(|&(_, distance)| ranked.1.total_cmp(&distance) == Ordering::Greater)
                .unwrap_or(fallback_count);
            if insert < FALLBACK_POINTS {
                fallback_count = (fallback_count + 1).min(FALLBACK_POINTS);
                for k in (insert + 1..fallback_count).rev() {
                    fallback[k] = fallback[k - 1];
                }
                fallback[insert] = ranked;
            }
            if !passes(j) {
                continue;
            }
            safe[held] = j;
            held += 1;
        }
        chosen[count] = if held == 0 {
            fallback[turn(time, player.place, fallback_count)].0
        } else {
            safe[turn(time, player.place, held)]
        };
        count += 1;
    }
    count
}

/// The tick puts a due player into his team's vector: one of his own side
/// into the first.
pub fn on_due_team(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Side::Own(_) = respawn::due_side(ctx) {
        respawn::into_first_vector(ctx);
    }
    Flow::Continue
}

/// The tick's selection for a vector: one of players of their own sides is
/// ours, any other the game's.
pub fn on_select(ctx: &mut Context<'_>) -> Flow<FreeForAllOnSelectExit> {
    let Some(mut select) = RespawnSelect::at(ctx) else { return Flow::Continue };
    let mut due = [Due { place: 0, side: Side::Own(0) }; MAX_DUE];
    let mut due_count = 0;
    for (slot, player) in due.iter_mut().zip(select.due()) {
        *slot = player;
        due_count += 1;
    }
    if !matches!(due[..due_count].first(), Some(Due { side: Side::Own(_), .. })) {
        return Flow::Continue;
    }
    let mut points = [Point { position: [0.0; 3], enemy_radius: 0.0 }; MAX_POINTS];
    let mut point_count = 0;
    for (slot, index) in points.iter_mut().zip(0..select.point_count()) {
        let Some((_, point)) = select.point(index) else { break };
        *slot = point;
        point_count += 1;
    }
    let mut living = [[0.0; 3]; MAX_LIVING];
    let mut living_count = 0;
    for (slot, position) in living.iter_mut().zip(select.living()) {
        *slot = position;
        living_count += 1;
    }
    let mut chosen = [0; MAX_DUE];
    let count = plan(
        &due[..due_count],
        &points[..point_count],
        &living[..living_count],
        select.time(),
        |j, side| select.point(j).is_some_and(|(at, _)| select.is_clear(at, side)),
        &mut chosen,
    );
    for &j in &chosen[..count] {
        if let Some((at, _)) = select.point(j) {
            select.choose(at);
        }
    }
    Flow::Exit(FreeForAllOnSelectExit::Done)
}

/// A resupply zone's use test: every zone serves a player of his own side.
pub fn on_can_use(ctx: &mut Context<'_>) -> Flow<FreeForAllOnCanUseExit> {
    if respawn::serve_own_side(ctx, ResupplySite::CanUse) {
        Flow::Exit(FreeForAllOnCanUseExit::Allowed)
    } else {
        Flow::Continue
    }
}

/// A resupply zone's use hint, as its use test.
pub fn on_use_info(ctx: &mut Context<'_>) -> Flow<FreeForAllOnUseInfoExit> {
    if respawn::serve_own_side(ctx, ResupplySite::UseInfo) {
        Flow::Exit(FreeForAllOnUseInfoExit::Allowed)
    } else {
        Flow::Continue
    }
}

/// srp's block after the options: its magic, then `{u8 tag, u8 len, bytes}`
/// (patch 65 reads its tag 2).
const SRPX: [u8; 4] = *b"SRPX";
/// The free-for-all entry: `kills_to_win`, a u16.
const TAG_KILLS_TO_WIN: u8 = 3;
/// The hud's messages stay this long, as the game's welcome (0x23B35B).
const MESSAGE_MS: u32 = 6000;
/// Where a text has the kill target.
const NUMBER: &[u8] = b"%1$s";

/// The kill target srp's block gives: its magic, then entries; an entry of
/// another tag or length is stepped over, a cut one ends the block. None
/// without the entry (not a free-for-all) or with a target of no kills.
fn kills_to_win_in(mut rest: Reader) -> Option<NonZeroU16> {
    if rest.array::<4>() != Some(SRPX) {
        return None;
    }
    let mut target = None;
    while let (Some(tag), Some(len)) = (rest.u8(), rest.u8()) {
        let Some(entry) = rest.take(usize::from(len)) else { break };
        if let (TAG_KILLS_TO_WIN, &[low, high]) = (tag, entry) {
            target = NonZeroU16::new(u16::from_le_bytes([low, high]));
        }
    }
    target
}

/// What the hud movie shows of the standing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Progress {
    /// The kills of the place the hud shows; 0 while it shows none.
    own: u16,
    /// The most kills of any place.
    top: u16,
}

/// The standing the hud state holds: the viewed place's kills (None while
/// the hud shows no place) and the most of the match's places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Standing {
    own: Option<u16>,
    top: u16,
}

impl Standing {
    fn of(ui: &GameWorldUi<'_>, places: u8) -> Standing {
        let top = Place::all().take(usize::from(places)).map(|place| ui.kills(place)).max().unwrap_or(0);
        let own = ui.viewed_place().filter(|place| place.get() < places).map(|place| ui.kills(place));
        Standing { own, top }
    }

    fn progress(self) -> Progress {
        Progress { own: self.own.unwrap_or(0), top: self.top }
    }
}

/// Where the match is, by what the hud state showed so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Not seen in play yet: before the start, or a game that came when the
    /// match was over.
    Before,
    Playing,
    /// Over, and its message shown.
    Ended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verdict {
    Won,
    Lost,
}

impl Verdict {
    fn key(self) -> &'static CStr {
        match self {
            Verdict::Won => c"st_finish_match_mode_08_winner_message",
            Verdict::Lost => c"st_finish_match_mode_08_loser_message",
        }
    }
}

/// The phase after a frame's hud state, and the message its end owes the
/// viewed place: the match is over when a place has the target or the time
/// rule has reached its exact deadline; the winners are those with the most kills, at
/// least one (srp's `ffa_top_kills`).
fn step(phase: Phase, clock: MatchClock, standing: Standing, target: NonZeroU16) -> (Phase, Option<Verdict>) {
    let over = match clock {
        MatchClock::Pregame => return (Phase::Before, None),
        MatchClock::Playing { .. } => standing.top >= target.get(),
        MatchClock::Finished => true,
    };
    match (phase, over) {
        (_, false) => (Phase::Playing, None),
        (Phase::Playing, true) => {
            let verdict =
                standing.own.map(|own| if own == standing.top && own > 0 { Verdict::Won } else { Verdict::Lost });
            (Phase::Ended, verdict)
        }
        (Phase::Before, true) => (Phase::Before, None),
        (Phase::Ended, true) => (Phase::Ended, None),
    }
}

/// Kill/death records are the terminal standings themselves. Once the
/// target is reached, later death events cannot change them. They already
/// serialize with the world, so rollback, a late join and replay need no
/// second, unsaved winner flag. The death which reaches the target still
/// runs the complete native handler (both the death and the kill).
pub fn on_kill_event(ctx: &mut Context) -> Flow<FreeForAllOnKillEventExit> {
    let Match::FreeForAll(game) = MATCH.get(ctx.thread()) else { return Flow::Continue };
    let Some(event) = crate::vostok::match_rules::KillEvent::at(ctx) else { return Flow::Continue };
    let Some((clock, top)) = event.standing() else { return Flow::Continue };
    if matches!(clock, MatchClock::Playing { .. }) && top < game.target.get() {
        Flow::Continue
    } else {
        Flow::Exit(FreeForAllOnKillEventExit::Done)
    }
}

/// A free-for-all match, from its options' read to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FreeForAll {
    target: NonZeroU16,
    phase: Phase,
    /// What a hud movie was last told, and which movie (a new match's may
    /// have the last one's address: the options' read and the hud's
    /// initialize_match forget it).
    told: Option<(usize, Progress)>,
}

/// The match the options last read (a match's or a replay's) are of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Match {
    Other,
    FreeForAll(FreeForAll),
}

static MATCH: GameCell<Match> = GameCell::new(Match::Other);

fn options_read(thread: &Thread, read: Option<OptionsRead>) -> Flow<NoExit> {
    let game = read.and_then(|read| {
        let target = kills_to_win_in(read.rest())?;
        Some(FreeForAll { target, phase: Phase::Before, told: None })
    });
    MATCH.set(thread, game.map_or(Match::Other, Match::FreeForAll));
    Flow::Continue
}

/// process_static_match_info after its deserialize (0x27B92D).
pub fn on_static_info(ctx: &mut Context) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_static_info(ctx))
}

/// A replay's chunk 1 after its deserialize (0x1F0971).
pub fn on_replay_options(ctx: &mut Context) -> Flow<NoExit> {
    options_read(ctx.thread(), OptionsRead::at_replay(ctx))
}

/// initialize_match's start (0x234880), before it sets the hud's game type:
/// the movie is told the standing again at the next frame.
pub fn on_initialize_match(ctx: &mut Context) -> Flow<NoExit> {
    MATCH.update(ctx.thread(), |game| match game {
        Match::FreeForAll(game) => Match::FreeForAll(FreeForAll { told: None, ..game }),
        Match::Other => Match::Other,
    });
    Flow::Continue
}

/// `text` with each [`NUMBER`] replaced by `number` in decimal, cut to fit
/// `out`.
fn with_number<'o>(text: &[u8], number: u16, out: &'o mut [u8; 0x200]) -> &'o CStr {
    let mut digits = [0u8; 5];
    let mut first = digits.len();
    let mut rest = number;
    loop {
        first -= 1;
        digits[first] = b'0' + (rest % 10) as u8;
        rest /= 10;
        if rest == 0 {
            break;
        }
    }
    let digits = &digits[first..];
    let room = out.len() - 1;
    let (mut at, mut len) = (0, 0);
    while at < text.len() && text[at] != 0 {
        let piece: &[u8] = if text[at..].starts_with(NUMBER) {
            at += NUMBER.len();
            digits
        } else {
            at += 1;
            &text[at - 1..at]
        };
        if len + piece.len() > room {
            break;
        }
        out[len..len + piece.len()].copy_from_slice(piece);
        len += piece.len();
    }
    out[len] = 0;
    CStr::from_bytes_until_nul(out).unwrap_or(c"")
}

/// update_ui's mark of the welcome message (0x23B36A), once a world as its
/// countdown ends: the game has no text for this mode, so ours is shown.
pub fn on_welcome(ctx: &mut Context) -> Flow<NoExit> {
    let Match::FreeForAll(game) = MATCH.get(ctx.thread()) else { return Flow::Continue };
    let Some(ui) = GameWorldUi::at_welcome(ctx) else { return Flow::Continue };
    let (mut translated, mut text) = ([0; 0x200], [0; 0x200]);
    let welcome = ui.translate(c"st_start_match_welcome_message_8", &mut translated);
    let welcome = with_number(welcome.to_bytes(), game.target.get(), &mut text);
    if let Some(movie) = ui.hud_movie() {
        movie.show_message(welcome, MESSAGE_MS);
    }
    Flow::Continue
}

/// update_player_icons' start (0x23AA10), every frame of a match, after the
/// rules filled the hud state: the movie told the standing when it changed,
/// and the match's end its message.
pub fn on_update_icons(ctx: &mut Context) -> Flow<NoExit> {
    let thread = ctx.thread();
    let Match::FreeForAll(mut game) = MATCH.get(thread) else { return Flow::Continue };
    let Some(ui) = GameWorldUi::at_update_icons(ctx) else { return Flow::Continue };
    let Some(movie) = ui.hud_movie() else { return Flow::Continue };
    let Some(places) = ui.places() else { return Flow::Continue };
    let Some(clock) = ui.clock() else { return Flow::Continue };
    let standing = Standing::of(&ui, places);
    let now = (movie.id(), standing.progress());
    let numbers = [u32::from(now.1.own), u32::from(now.1.top), u32::from(game.target.get())];
    if game.told != Some(now) && movie.invoke_uints(c"root.set_ffa_progress", &numbers) {
        game.told = Some(now);
    }
    let (phase, verdict) = step(game.phase, clock, standing, game.target);
    // A message the movie does not take is owed at the next frame.
    let shown = verdict.is_none_or(|verdict| {
        let mut text = [0; 0x200];
        movie.show_message(ui.translate(verdict.key(), &mut text), MESSAGE_MS)
    });
    if shown {
        game.phase = phase;
    }
    MATCH.set(thread, Match::FreeForAll(game));
    Flow::Continue
}

/// The optional millisecond duration is parsed once while the point loads.
pub fn on_protection_read(ctx: &mut Context<'_>) -> Flow<crate::vostok::FreeForAllOnProtectionReadExit> {
    match respawn::apply_protection_override(ctx) {
        respawn::ProtectionLoad::Legacy => Flow::Continue,
        respawn::ProtectionLoad::Overridden => Flow::Exit(crate::vostok::FreeForAllOnProtectionReadExit::Applied),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn at(x: f32) -> Point {
        Point { position: [x, 0.0, 0.0], enemy_radius: 30.0 }
    }

    fn solo(place: u8) -> Due {
        Due { place, side: Side::Own(4 + u32::from(place)) }
    }

    fn planned(
        due: &[Due],
        points: &[Point],
        living: &[[f32; 3]],
        time: u32,
        clear: impl Fn(usize, Side) -> bool,
    ) -> Vec<usize> {
        let mut chosen = [0; MAX_DUE];
        let count = plan(due, points, living, time, clear, &mut chosen);
        chosen[..count].to_vec()
    }

    /// The game's test for a player of his own side: no living player inside
    /// the point's enemy radius.
    fn clear_of<'a>(points: &'a [Point], living: &'a [[f32; 3]]) -> impl Fn(usize, Side) -> bool + 'a {
        move |j, _| {
            let radius = points[j].enemy_radius * points[j].enemy_radius;
            living.iter().all(|&player| distance_squared(points[j].position, player) >= radius)
        }
    }

    #[test]
    fn every_safe_point_can_be_chosen_including_the_nearest_safe_one() {
        let points = [at(10.0), at(40.0), at(100.0), at(70.0), at(200.0)];
        let living = [[0.0; 3]];
        let mut seen = [false; 5];
        for time in (0..50_000).step_by(50) {
            let chosen = planned(&[solo(0)], &points, &living, time, clear_of(&points, &living));
            seen[chosen[0]] = true;
        }
        // 40 m is safe even though the former top-three ranking always excluded it.
        assert_eq!(seen, [false, true, true, true, true]);
    }

    #[test]
    fn regular_respawns_visit_the_whole_safe_pool_without_favouring_corners() {
        // Four corners are farther from the living player than the safe middle.
        let points = [Point { position: [0.0, 0.0, 40.0], enemy_radius: 30.0 },
            Point { position: [-100.0, 0.0, -100.0], enemy_radius: 30.0 },
            Point { position: [100.0, 0.0, -100.0], enemy_radius: 30.0 },
            Point { position: [-100.0, 0.0, 100.0], enemy_radius: 30.0 },
            Point { position: [100.0, 0.0, 100.0], enemy_radius: 30.0 }];
        let living = [[0.0; 3]];
        for interval in [5_000, 10_000, 15_000] {
            let mut visits = [0; 5];
            for event in 0..2_000 {
                let chosen = planned(&[solo(0)], &points, &living, event * interval, clear_of(&points, &living));
                visits[chosen[0]] += 1;
            }
            assert!(visits.iter().all(|&n| (300..=500).contains(&n)), "{interval}: {visits:?}");
        }
        // Empty initial worlds used to consider only the first three equal-ranked points.
        let points: Vec<_> = (0..MAX_POINTS).map(|i| at(i as f32 * 40.0)).collect();
        let mut seen = [false; MAX_POINTS];
        for time in (0..500_000).step_by(5_000) {
            for place in 0..20 {
                seen[planned(&[solo(place)], &points, &[], time, |_, _| true)[0]] = true;
            }
        }
        assert!(seen.into_iter().all(|visited| visited));
    }

    #[test]
    fn one_or_two_clear_points_are_the_whole_choice() {
        let points = [at(10.0), at(40.0), at(100.0)];
        let living = [[0.0; 3]];
        let mut seen = [false; 3];
        for time in (0..5_000).step_by(50) {
            seen[planned(&[solo(0)], &points, &living, time, clear_of(&points, &living))[0]] = true;
            assert_eq!(planned(&[solo(0)], &points[..2], &living, time, clear_of(&points[..2], &living)), [1]);
        }
        assert_eq!(seen, [false, true, true]);
    }

    #[test]
    fn crowded_fallback_uses_all_five_best_separated_points() {
        let points: Vec<_> = [1.0, 19.0, 3.0, 17.0, 5.0, 15.0, 7.0, 13.0, 9.0, 11.0]
            .into_iter().map(at).collect();
        let living = [[0.0; 3]];
        for interval in [5_000, 10_000, 15_000] {
            let mut visits = [0; 10];
            for event in 0..2_000 {
                let chosen = planned(&[solo(0)], &points, &living, event * interval, clear_of(&points, &living));
                visits[chosen[0]] += 1;
            }
            for (i, &n) in visits.iter().enumerate() {
                if i % 2 == 0 {
                    assert_eq!(n, 0, "nearby point {i} entered fallback");
                } else {
                    assert!((300..=500).contains(&n), "{interval}: {visits:?}");
                }
            }
        }
        // Small maps use every available candidate, and empty maps remain empty.
        let points = [at(5.0), at(25.0), at(15.0)];
        let mut seen = [false; 3];
        for time in (0..5_000).step_by(50) {
            seen[planned(&[solo(3)], &points, &living, time, |_, _| false)[0]] = true;
        }
        assert_eq!(seen, [true; 3]);
        assert_eq!(planned(&[solo(0)], &points[..1], &living, 0, |_, _| false), [0]);
        assert_eq!(planned(&[solo(0)], &[], &living, 0, |_, _| true), []);
    }

    #[test]
    fn points_given_at_the_tick_count_as_players() {
        let points = [at(0.0), at(10.0), at(40.0), at(100.0), at(170.0)];
        for time in (0..5_000).step_by(50) {
            let chosen = planned(&[solo(0), solo(1), solo(2)], &points, &[], time, |_, _| true);
            assert_eq!(chosen.len(), 3);
            for (i, &j) in chosen.iter().enumerate() {
                for &k in &chosen[..i] {
                    assert!(distance_squared(points[j].position, points[k].position) >= 900.0);
                }
            }
        }
    }

    #[test]
    fn every_due_player_gets_a_point() {
        // More players than points that can pass: the rest choose among the fallback group, none waits.
        let points = [at(0.0), at(10.0)];
        let chosen = planned(&[solo(0), solo(1), solo(2), solo(3)], &points, &[], 0, |_, _| true);
        assert_eq!(chosen.len(), 4);
        assert_eq!(chosen[0], 0);
        // 20 due players, and the game's test true for none.
        let due: Vec<Due> = (0..20).map(solo).collect();
        let points: Vec<Point> = (0..24).map(|i| at(i as f32 * 35.0)).collect();
        assert_eq!(planned(&due, &points, &[], 1000, |_, _| false).len(), 20);
        assert_eq!(planned(&due, &points, &[], 1000, |_, _| true).len(), 20);
    }

    #[test]
    fn the_same_world_gives_the_same_plan() {
        let points: Vec<Point> = (0..30)
            .map(|i| Point { position: [(i * 37 % 200) as f32, (i % 3) as f32, (i * 91 % 170) as f32], enemy_radius: 30.0 })
            .collect();
        let living = [[20.0, 0.0, 30.0], [150.0, 1.0, 90.0], [90.0, 2.0, 160.0]];
        let due = [solo(4), solo(9), solo(17)];
        let first = planned(&due, &points, &living, 61_250, clear_of(&points, &living));
        assert_eq!(first, planned(&due, &points, &living, 61_250, clear_of(&points, &living)));
        assert_eq!(first.len(), 3);
        // Each is clear of the living, and of the points given before it.
        for (i, &j) in first.iter().enumerate() {
            assert!(clear_of(&points, &living)(j, Side::Own(0)));
            for &k in &first[..i] {
                assert!(distance_squared(points[j].position, points[k].position) >= 900.0);
            }
        }
    }

    #[test]
    fn fallback_ranks_are_squared_distances_and_ties_go_by_index() {
        assert_eq!(distance_squared([1.0, 2.0, 3.0], [4.0, 6.0, 15.0]), 169.0);
        assert_eq!(nearest([0.0; 3], [[3.0, 0.0, 0.0], [0.0, 2.0, 0.0]].into_iter()), 4.0);
        assert_eq!(nearest([0.0; 3], core::iter::empty()), f32::INFINITY);
        let points = [at(-50.0), at(50.0), at(50.0), at(-50.0), at(50.0), at(-50.0)];
        let living = [[0.0; 3]];
        let mut seen = [false; 6];
        for time in (0..5_000).step_by(50).chain([u32::MAX]) {
            let chosen = planned(&[solo(0)], &points, &living, time, |_, _| false);
            assert_eq!(chosen, [turn(time, 0, FALLBACK_POINTS)]);
            seen[chosen[0]] = true;
        }
        assert_eq!(seen, [true, true, true, true, true, false]);
    }

    #[test]
    fn the_turn_is_bounded_at_clock_wrap() {
        for count in 1..=MAX_POINTS {
            for time in [0, 49, 50, 12_345, u32::MAX - 50, u32::MAX] {
                for place in 0..20 {
                    assert!(turn(time, place, count) < count);
                }
            }
        }
        assert_eq!(turn(u32::MAX, 19, 1), 0);
    }

    #[test]
    fn the_clear_test_gets_the_players_own_side() {
        let points = [at(0.0)];
        assert_eq!(planned(&[solo(3)], &points, &[], 0, |_, side| side == Side::Own(7)), [0]);
    }

    fn target(bytes: &[u8]) -> Option<u16> {
        kills_to_win_in(Reader::new(bytes)).map(NonZeroU16::get)
    }

    #[test]
    fn the_blocks_kill_target() {
        assert_eq!(target(b"SRPX\x03\x02\x1e\x00"), Some(30));
        assert_eq!(target(b"SRPX\x03\x02\x2c\x01"), Some(300));
        assert_eq!(target(b"SRPX\x01\x0c\0\0\x40\x40\x8f\xc2\x2b\x3f\0\0\0\x40\x03\x02\x1e\x00"), Some(30));
        assert_eq!(target(b"SRPX\x03\x02\x1e\x00\x09\x03xyz"), Some(30));
        assert_eq!(target(b""), None);
        assert_eq!(target(b"SRPX"), None);
        assert_eq!(target(b"SRPX\x02\x01\x01"), None);
        assert_eq!(target(b"SRPX\x03\x02\x00\x00"), None);
        assert_eq!(target(b"SRPX\x03\x01\x1e"), None);
        assert_eq!(target(b"SRPX\x03\x02\x1e"), None);
        assert_eq!(target(b"SRPY\x03\x02\x1e\x00"), None);
    }

    const TARGET: NonZeroU16 = NonZeroU16::new(30).unwrap();

    fn standing(own: Option<u16>, top: u16) -> Standing {
        Standing { own, top }
    }

    #[test]
    fn the_match_ends_once_at_the_target_or_the_time() {
        let playing = MatchClock::Playing { remaining_ms: core::num::NonZeroU32::new(400_000).unwrap() };
        let last = MatchClock::Finished;
        // Before the start, and a hud state no rule filled yet (no pregame, no second): nothing.
        assert_eq!(step(Phase::Before, MatchClock::Pregame, standing(Some(0), 0), TARGET), (Phase::Before, None));
        assert_eq!(step(Phase::Before, last, standing(Some(0), 0), TARGET), (Phase::Before, None));
        assert_eq!(step(Phase::Before, playing, standing(Some(0), 0), TARGET), (Phase::Playing, None));
        assert_eq!(step(Phase::Playing, playing, standing(Some(29), 29), TARGET), (Phase::Playing, None));
        // The target: the leader won, the others lost; told once.
        assert_eq!(step(Phase::Playing, playing, standing(Some(30), 30), TARGET), (Phase::Ended, Some(Verdict::Won)));
        assert_eq!(step(Phase::Playing, playing, standing(Some(12), 30), TARGET), (Phase::Ended, Some(Verdict::Lost)));
        assert_eq!(step(Phase::Ended, playing, standing(Some(30), 30), TARGET), (Phase::Ended, None));
        // The time: the most kills win, ties share, nobody wins without a kill.
        assert_eq!(step(Phase::Playing, last, standing(Some(7), 7), TARGET), (Phase::Ended, Some(Verdict::Won)));
        assert_eq!(step(Phase::Playing, last, standing(Some(6), 7), TARGET), (Phase::Ended, Some(Verdict::Lost)));
        assert_eq!(step(Phase::Playing, last, standing(Some(0), 0), TARGET), (Phase::Ended, Some(Verdict::Lost)));
        // A hud that shows no place: the end without a message.
        assert_eq!(step(Phase::Playing, last, standing(None, 7), TARGET), (Phase::Ended, None));
        // A game that came when the match was over is told nothing; a replay wound back plays on.
        assert_eq!(step(Phase::Before, playing, standing(Some(1), 30), TARGET), (Phase::Before, None));
        assert_eq!(step(Phase::Ended, playing, standing(Some(3), 9), TARGET), (Phase::Playing, None));
        assert_eq!(step(Phase::Ended, MatchClock::Pregame, standing(Some(0), 0), TARGET), (Phase::Before, None));
    }

    #[test]
    fn the_standing_shown_without_a_place_is_zero() {
        assert_eq!(standing(None, 9).progress(), Progress { own: 0, top: 9 });
        assert_eq!(standing(Some(4), 9).progress(), Progress { own: 4, top: 9 });
    }

    #[test]
    fn a_text_takes_the_target() {
        let mut out = [0xAA; 0x200];
        assert_eq!(with_number(b"First to %1$s kills wins", 30, &mut out), c"First to 30 kills wins");
        assert_eq!(with_number(b"%1$s", 65535, &mut out), c"65535");
        assert_eq!(with_number(b"%1$s / %1$s", 7, &mut out), c"7 / 7");
        assert_eq!(with_number(b"no number, 100%", 0, &mut out), c"no number, 100%");
        assert_eq!(with_number(b"", 30, &mut out), c"");
        assert_eq!(with_number(b"cut\0here", 30, &mut out), c"cut");
        // Cut to the room, never past it: a number that does not fit is left out whole.
        assert_eq!(with_number(&[b'x'; 0x300], 30, &mut out).to_bytes().len(), 0x1FF);
        let mut edge = [b'x'; 0x202];
        edge[0x1FE..].copy_from_slice(NUMBER);
        assert_eq!(with_number(&edge, 30, &mut out).to_bytes(), &[b'x'; 0x1FE]);
    }
}

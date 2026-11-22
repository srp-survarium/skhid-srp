//! Freeze Tag: snapshot-owned best-of-nine rule and native player lifecycle.
#![forbid(unsafe_code)]
use crate::vostok::{self, Context, Flow, GameCell, NoExit, OptionsRead};
use crate::vostok::{FreezeTagOnEnemyExit, FreezeTagOnInputExit, FreezeTagOnPlayerTickExit};
use crate::vostok::log::{self, Piece};
use crate::vostok::resources::{self, Arrived, KeptParticle, QueryResults, QueryTag};
use crate::vostok::parse::Reader;
use srp_validate::Place;
use srp_validate::world::freeze::{Actions, Body, Condition, FreezeTag, PLAYERS, Phase, Player, RoundEnd, Schedule, THAW_MS, WINS};

#[derive(Clone, Copy)]
enum Match {
    Other,
    Freeze(FreezeTag),
}
static MATCH: GameCell<Match> = GameCell::new(Match::Other);
static CAMERA: GameCell<vostok::freeze::Camera> = GameCell::new(vostok::freeze::Camera::Native);
// Presentation ownership only: clear labels on a reused movie when leaving
// this mode. Ordinary matches that never displayed them make no extra calls.
static MARKERS: GameCell<Option<usize>> = GameCell::new(None);

/// srp's block after the options names a Freeze Tag match: tag 6, one byte 1,
/// among its other entries (an entry of another tag is stepped over, a cut one
/// ends the block).
fn freeze_tag_in(mut rest: Reader<'_>) -> bool {
    if rest.array::<4>() != Some(*b"SRPX") {
        return false;
    }
    let mut on = false;
    while let (Some(tag), Some(len)) = (rest.u8(), rest.u8()) {
        let Some(entry) = rest.take(usize::from(len)) else { break };
        on |= tag == TAG_FREEZE_TAG && entry == [1];
    }
    on
}
/// The Freeze Tag entry's tag (static_info.rs `SRPX_FREEZE_TAG`).
const TAG_FREEZE_TAG: u8 = 6;

fn options(ctx: &Context<'_>, options: Option<OptionsRead<'_>>) {
    CAMERA.set(ctx.thread(), vostok::freeze::Camera::Native);
    let on = options.is_some_and(|options| freeze_tag_in(options.rest()));
    MATCH.set(
        ctx.thread(),
        if on {
            Match::Freeze(FreezeTag::EMPTY)
        } else {
            Match::Other
        },
    );
}
pub fn on_static_info(ctx: &mut Context<'_>) -> Flow<NoExit> {
    options(ctx, OptionsRead::at_static_info(ctx));
    Flow::Continue
}
pub fn on_replay_options(ctx: &mut Context<'_>) -> Flow<NoExit> {
    options(ctx, OptionsRead::at_replay(ctx));
    Flow::Continue
}
pub fn on_tick(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Match::Freeze(mut state) = MATCH.get(ctx.thread()) else {
        return Flow::Continue;
    };
    let Some(mut world) = vostok::freeze::World::at_tick(ctx) else {
        return Flow::Continue;
    };
    let bodies = world.bodies();
    // Rays only for the pairs a thaw can use: a frozen (or newly lethal)
    // body and a fighting teammate. The boundary bounds distance first.
    let mut visible = [[false; PLAYERS]; PLAYERS];
    for i in Place::all() {
        for j in Place::all() {
            if let (
                Body::Present { team, condition, .. },
                Body::Present { team: helper, condition: Condition::Fighting, .. },
            ) = (bodies[i.index()], bodies[j.index()])
                && i != j
                && team == helper
                && (condition == Condition::Lethal || is_frozen(&state, i))
                && !is_frozen(&state, j)
            {
                visible[i.index()][j.index()] = world.clear(i, j);
            }
        }
    }
    match state.step(&bodies, &visible, world.elapsed()) {
        Actions::Restart => world.restart(),
        Actions::Play { thawed, thawers } => {
            thawed.iter().for_each(|place| world.thaw(place));
            thawers.iter().for_each(|place| world.credit_thaw(place));
        }
        Actions::Wait => {}
    }
    // A frozen body's death also queued its ordinary respawn; only the new
    // round's restart spawns it. An entering place's spawn is never cancelled
    // (`World::cancel`): a frozen place that comes back appears at his team's
    // points and stays frozen there (the rule keeps it, the hold keeps it
    // lethal) until a teammate or the auto-thaw thaws it.
    match state.schedule() {
        Schedule::CancelFrozen(frozen) => frozen.iter().for_each(|place| world.cancel(place)),
        Schedule::CancelAll => Place::all().for_each(|place| world.cancel(place)),
        Schedule::Native => {}
    }
    // The pause and the restart play the match start's countdown, as Artifact Hunt's round.
    world.set_round_countdown(state.round_countdown());
    world.publish(state.points(), state.winner());
    MATCH.set(ctx.thread(), Match::Freeze(state));
    Flow::Continue
}
/// update_minimap_players' spotted test of an enemy: a team's last one fighting, his teammates
/// frozen, two at least, his dot on the minimap and the Tab map always, as a spotted enemy's (the
/// owner, 2026-11-22: "last member is always shown on the map"); never in 1 v 1, 2 v 1 or 2 v 2
/// ("when 1vs1 we see each other on the map, bad", "2vs1 should also not show"). From the rule
/// state, the same in every game; nothing simulated.
pub fn on_enemy(ctx: &mut Context<'_>) -> Flow<FreezeTagOnEnemyExit> {
    let Match::Freeze(state) = MATCH.get(ctx.thread()) else {
        return Flow::Continue;
    };
    match vostok::freeze::Player::at_minimap_enemy(ctx) {
        Some(player) if state.last_standing(player.place()) => Flow::Exit(FreezeTagOnEnemyExit::Shown),
        _ => Flow::Continue,
    }
}
fn is_frozen(state: &FreezeTag, place: Place) -> bool {
    matches!(state.player(place), Player::Frozen { .. })
}
pub fn active(thread: &vostok::Thread) -> bool {
    matches!(MATCH.get(thread), Match::Freeze(_))
}

/// game_world::tick after the simulation's move: the frozen glow drawn, the frozen camera kept.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(world) = vostok::world::GameWorld::at_after_move(ctx) {
        draw_glow(ctx.thread(), &world);
    }
    let rule = match MATCH.get(ctx.thread()) {
        Match::Freeze(state) => Some(state),
        Match::Other => None,
    };
    let camera = CAMERA.get(ctx.thread());
    if rule.is_none() && matches!(camera, vostok::freeze::Camera::Native) {
        return Flow::Continue;
    }
    let next = camera.update(ctx, rule.as_ref());
    CAMERA.set(ctx.thread(), next);
    Flow::Continue
}

/// Whom the rule holds (his tick and input skipped): a lethal body always; a
/// frozen player in play and in the pause between rounds, where everyone else
/// moves on (the owner, 2026-11-22: "when one team in freeze tag wins a round
/// it shouldn't stop moving", "only frozen people do"); everyone in the
/// restart and after the match.
fn held(state: &FreezeTag, player: &vostok::freeze::Player<'_>) -> bool {
    player.playing()
        && (player.lethal()
            || match state.phase() {
                Phase::Playing | Phase::Between { .. } => is_frozen(state, player.place()),
                Phase::Restarting | Phase::Finished(_) => true,
            })
}
/// Whether the local player at `local` sees a round's countdown (the warmup
/// camera, patch 31's kits) while the core says one is about to start: in
/// Freeze Tag only a frozen one; the others keep their camera and move on
/// (`held`). Any other mode with rounds: everyone.
pub fn countdown_view(thread: &vostok::Thread, local: Option<Place>) -> bool {
    match (MATCH.get(thread), local) {
        (Match::Freeze(state), Some(place)) => is_frozen(&state, place),
        (Match::Freeze(_), None) | (Match::Other, _) => true,
    }
}
pub fn on_player_tick(ctx: &mut Context<'_>) -> Flow<FreezeTagOnPlayerTickExit> {
    let Match::Freeze(state) = MATCH.get(ctx.thread()) else {
        return Flow::Continue;
    };
    let Some((mut player, time)) = vostok::freeze::Player::at_tick(ctx) else {
        return Flow::Continue;
    };
    if held(&state, &player) {
        player.hold(time, state.phase());
        return Flow::Exit(FreezeTagOnPlayerTickExit::Frozen);
    }
    Flow::Continue
}

// ---- the frozen glow -----------------------------------------------------

/// The frozen glow's particle system (pack freeze-tag-glow: the Spring activation glow in ice blue,
/// fainter; docs/notes/freeze-tag.md "The frozen glow").
const GLOW_PARTICLE: &core::ffi::CStr = c"fx/artifacts/human_frozen";
/// The glow's size at the end of the thaw, of its full size.
const THAWED_SIZE: f32 = 0.15;
/// The height on the body (metres over its feet) the glow shrinks toward.
const GLOW_MIDDLE: f32 = 0.9;

/// The glow's matrix for a thaw progress: the body's, its axes scaled by 1 at no progress down to
/// THAWED_SIZE at THAW_MS, about GLOW_MIDDLE (the owner, 2026-11-21: "it just disappears a few times
/// without becoming smaller"). The emitters are in the instance's space, so their particles' places
/// scale with it; a sprite keeps its size. A reset progress is full size again.
fn glow_at(body: [f32; 16], progress: u16) -> [f32; 16] {
    let done = f32::from(progress.min(THAW_MS)) / f32::from(THAW_MS);
    let scale = 1.0 - (1.0 - THAWED_SIZE) * done;
    let mut at = body;
    for value in &mut at[..12] {
        *value *= scale;
    }
    for axis in 0..3 {
        at[12 + axis] += (1.0 - scale) * GLOW_MIDDLE * body[4 + axis];
    }
    at
}

/// Each place's instance: a scene plays an instance once.
type Particles = [KeptParticle; PLAYERS];

/// The glow's particle systems: asked at the first frame of a Freeze Tag match, kept for the process.
#[derive(Clone, Copy)]
enum Load {
    Unasked,
    Asked,
    Ready(Particles),
    /// The pack is missing: no glow this session.
    Missing,
}
static GLOW: GameCell<Load> = GameCell::new(Load::Unasked);
/// Whether this game has the glow of each place in the scene.
static GLOWING: GameCell<[bool; PLAYERS]> = GameCell::new([false; PLAYERS]);

/// Each frame after the simulation's move, this game draws the glow itself on every frozen body of
/// a match in play, at the size its thaw progress gives, from the world's rule state (so every
/// game, a rollback and a late join show it as the world has it), and stops it at the thaw, the
/// round's restart, the match's end and another match. Nothing of it enters the simulation: an
/// effect added there at the first tick after this game loaded it would enter each game's world at
/// another tick (Artifact Hunt's carrier glow desynced so, 2026-11-21). The frozen player's camera
/// is the orbit around his body, so he sees the same particles; no first-person twin.
fn draw_glow(thread: &vostok::Thread, world: &vostok::world::GameWorld<'_>) {
    let state = match MATCH.get(thread) {
        Match::Freeze(state) => Some(state),
        Match::Other => None,
    };
    let shown = GLOWING.get(thread);
    if state.is_none() && !shown.contains(&true) {
        return;
    }
    let particles = match GLOW.get(thread) {
        Load::Unasked => {
            resources::query_particles(thread, &[GLOW_PARTICLE; PLAYERS], crate::vostok::freeze_tag_on_loaded_entry());
            GLOW.set(thread, Load::Asked);
            return;
        }
        Load::Ready(particles) => particles,
        Load::Asked | Load::Missing => return,
    };
    let Some(scene) = vostok::freeze::GlowScene::of(world, thread) else { return };
    let in_play = scene.in_play();
    let mut now = [false; PLAYERS];
    for (place, (was, next)) in Place::all().zip(shown.into_iter().zip(now.iter_mut())) {
        let target = state
            .filter(|_| in_play)
            .and_then(|state| match state.player(place) {
                Player::Frozen { progress, .. } => Some(progress.millis()),
                Player::Waiting | Player::Fighting(_) => None,
            })
            .zip(scene.body(place))
            .map(|(progress, body)| glow_at(body, progress));
        let particle = particles[place.index()];
        match (was, target) {
            (true, Some(at)) => scene.move_to(particle, &at),
            (false, Some(at)) => scene.play(particle, &at),
            (true, None) => scene.stop(particle),
            (false, None) => {}
        }
        *next = target.is_some();
    }
    GLOWING.set(thread, now);
}

/// The glow's query came back (on the game's thread).
pub fn on_loaded(results: QueryResults<'_>, tag: QueryTag) {
    let thread = results.thread();
    let Arrived::Particles(arrived) = results.arrived(tag) else { return };
    match arrived.all::<PLAYERS>() {
        Some(particles) => GLOW.set(thread, Load::Ready(particles)),
        None => {
            log::line(&[Piece::Text("freeze_tag: the frozen glow did not load; none this session")]);
            GLOW.set(thread, Load::Missing);
        }
    }
}

/// game_world::on_deactivate: the match's scene is left; no glow of ours in it.
pub fn on_deactivate(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let thread = ctx.thread();
    let shown = GLOWING.get(thread);
    if let Load::Ready(particles) = GLOW.get(thread)
        && let Some(world) = vostok::world::GameWorld::at_deactivate(ctx)
        && let Some(scene) = vostok::freeze::GlowScene::of(&world, thread)
    {
        for (particle, shown) in particles.into_iter().zip(shown) {
            if shown {
                scene.stop(particle);
            }
        }
    }
    GLOWING.set(thread, [false; PLAYERS]);
    Flow::Continue
}

pub fn on_input(ctx: &mut Context<'_>) -> Flow<FreezeTagOnInputExit> {
    if let Match::Freeze(state) = MATCH.get(ctx.thread())
        && let Some(player) = vostok::freeze::Player::at_input(ctx)
        && held(&state, &player)
    {
        return Flow::Exit(FreezeTagOnInputExit::Held);
    }
    Flow::Continue
}
pub fn on_save(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Match::Freeze(state) = MATCH.get(ctx.thread()) {
        vostok::respawn::save(ctx, &state.bytes());
    }
    Flow::Continue
}
pub fn on_load(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Match::Freeze(_) = MATCH.get(ctx.thread()) {
        // The world check (patch 87) refused any world whose rule does not read.
        let state = vostok::respawn::load::<{ FreezeTag::LEN }>(ctx)
            .and_then(|b| FreezeTag::read(&b))
            .expect("a checked Freeze Tag world's rule");
        MATCH.set(ctx.thread(), Match::Freeze(state));
    }
    Flow::Continue
}

/// Every match's start, whatever its mode: the score bars' native source (the hud state's team
/// points) 0:0. Made once with the hud and cleared by nothing, it kept the last match's score for a
/// mode whose rule does not fill it (the owner, 2026-11-22: "freeze tag counter doesn't reset after
/// prev game", "make sure arthunt also resets it at the beginning").
pub fn on_initialize_match(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(ui) = vostok::GameWorldUi::at_initialize_match(ctx) {
        ui.set_team_points([0, 0]);
    }
    Flow::Continue
}

pub fn on_hud(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(ui) = vostok::GameWorldUi::at_update_icons(ctx) else {
        return Flow::Continue;
    };
    let Match::Freeze(state) = MATCH.get(ctx.thread()) else {
        if let Some(previous) = MARKERS.get(ctx.thread())
            && let Some(movie) = ui.icons_movie()
            && (previous != movie.id() || movie.invoke_uints(c"root.freeze_tag_markers", &[0]))
        {
            MARKERS.set(ctx.thread(), None);
        }
        return Flow::Continue;
    };
    // The score bars' native source, every frame of the match from its first, whoever is viewed.
    ui.set_team_points(state.points());
    let Some(viewed) = ui.viewed_place() else {
        return Flow::Continue;
    };
    let viewed = Place::new(viewed.get()).expect("the twenty places");
    let (team, frozen, progress, thaws_in) = match state.player(viewed) {
        Player::Fighting(team) => (team, 0, 0, 0),
        Player::Frozen { team, progress, thaws_in } => (team, 1, u32::from(progress.millis()), thaws_in.get()),
        Player::Waiting => return Flow::Continue,
    };
    // 1 won, 2 lost, 3 drawn by the time limit.
    let outcome = match state.winner() {
        Some(winner) if winner == team => 1,
        Some(_) => 2,
        None if matches!(ui.clock(), Some(vostok::MatchClock::Finished)) => 3,
        None => 0,
    };
    let other = match team {
        srp_validate::Team::First => srp_validate::Team::Second,
        srp_validate::Team::Second => srp_validate::Team::First,
    };
    let (round, remaining) = match state.phase() {
        Phase::Between { result, remaining } => (
            match result {
                RoundEnd::Won(winner) => {
                    if winner == team {
                        1
                    } else {
                        2
                    }
                }
                RoundEnd::Draw => 3,
            },
            u32::from(remaining.get()),
        ),
        _ => (0, 0),
    };
    // The rescuer sees the same native collection bar as its frozen teammate.
    let progress = if frozen != 0 {
        progress
    } else {
        Place::all()
            .filter_map(|place| match state.player(place) {
                Player::Frozen {
                    team: side,
                    progress,
                    ..
                } if side == team && progress.millis() != 0 && ui.freeze_reachable(viewed, place) => {
                    Some(u32::from(progress.millis()))
                }
                _ => None,
            })
            .max()
            .unwrap_or(0)
    };
    let mask = Place::all()
        .filter(|place| is_frozen(&state, *place))
        .fold(0, |mask, place| mask | (1 << place.index()));
    if let Some(movie) = ui.hud_movie() {
        movie.invoke_uints(
            c"root.freeze_tag_score",
            &[u32::from(state.points_of(team)), u32::from(state.points_of(other)), u32::from(WINS)],
        );
        movie.invoke_uints(
            c"root.freeze_tag_update",
            &[
                frozen,
                progress,
                if round != 0 { round + 3 } else { outcome },
                remaining,
                u32::from(THAW_MS),
                thaws_in,
            ],
        );
        // The Tab list's frozen rows; the movie clears them with the match.
        movie.invoke_uints(c"root.freeze_tag_rows", &[mask]);
    }
    if let Some(movie) = ui.icons_movie() {
        if movie.invoke_uints(c"root.freeze_tag_markers", &[mask]) {
            MARKERS.set(ctx.thread(), Some(movie.id()));
        }
    }
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_entry_among_the_others() {
        let on = |bytes: &[u8]| freeze_tag_in(Reader::new(bytes));
        assert!(on(b"SRPX\x06\x01\x01"));
        assert!(on(b"SRPX\x04\x02\x2c\x01\x06\x01\x01\x05\x01\x02"));
        for off in [&b""[..], b"SRPX", b"SRPX\x06\x01\x00", b"SRPX\x06\x01", b"SRPX\x04\x01\x01", b"SRPX\x02\x01\x01"] {
            assert!(!on(off), "{off:?}");
        }
    }
}

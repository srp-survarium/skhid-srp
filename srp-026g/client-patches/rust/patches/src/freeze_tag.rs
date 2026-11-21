//! Freeze Tag: snapshot-owned best-of-five rule and native player lifecycle.
#![forbid(unsafe_code)]
use crate::vostok::{self, Context, Flow, GameCell, NoExit, OptionsRead};
use crate::vostok::{FreezeTagOnInputExit, FreezeTagOnPlayerTickExit};
use crate::vostok::parse::Reader;
use srp_validate::Place;
use srp_validate::world::freeze::{Actions, Body, Condition, FreezeTag, PLAYERS, Phase, Player, RoundEnd};

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
        Actions::Play { thawed } => thawed.iter().for_each(|place| world.thaw(place)),
        Actions::Wait => {}
    }
    // A frozen body's death also queued its ordinary respawn; only the new
    // round's restart spawns it. Places still waiting keep their native
    // first spawn, and a restart's schedule is left to run.
    if state.phase() != Phase::Restarting {
        for place in Place::all().filter(|place| is_frozen(&state, *place)) {
            world.unschedule(place);
        }
    }
    world.publish(state.points(), state.winner());
    MATCH.set(ctx.thread(), Match::Freeze(state));
    Flow::Continue
}
fn is_frozen(state: &FreezeTag, place: Place) -> bool {
    matches!(state.player(place), Player::Frozen { .. })
}
pub fn active(thread: &vostok::Thread) -> bool {
    matches!(MATCH.get(thread), Match::Freeze(_))
}

pub fn on_camera(ctx: &mut Context<'_>) -> Flow<NoExit> {
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

fn held(state: &FreezeTag, player: &vostok::freeze::Player<'_>) -> bool {
    player.playing()
        && (player.lethal()
            || matches!(
                state.phase(),
                Phase::Between { .. } | Phase::Restarting | Phase::Finished(_)
            )
            || (state.phase() == Phase::Playing
                && matches!(state.player(player.place()), Player::Frozen { .. })))
}
pub fn on_player_tick(ctx: &mut Context<'_>) -> Flow<FreezeTagOnPlayerTickExit> {
    if let Match::Freeze(state) = MATCH.get(ctx.thread())
        && let Some((mut player, time)) = vostok::freeze::Player::at_tick(ctx)
        && held(&state, &player)
    {
        player.hold(time, state.phase());
        return Flow::Exit(FreezeTagOnPlayerTickExit::Frozen);
    }
    Flow::Continue
}
pub fn on_input(ctx: &mut Context<'_>) -> Flow<FreezeTagOnInputExit> {
    if let Match::Freeze(state) = MATCH.get(ctx.thread())
        && let Some(player) = vostok::freeze::Player::at_input(ctx)
        && held(&state, &player)
    {
        return Flow::Exit(FreezeTagOnInputExit::Frozen);
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
    let Some(viewed) = ui.viewed_place() else {
        return Flow::Continue;
    };
    let viewed = Place::new(viewed.get()).expect("the twenty places");
    let (team, frozen, progress) = match state.player(viewed) {
        Player::Fighting(team) => (team, 0, 0),
        Player::Frozen { team, progress } => (team, 1, u32::from(progress.millis())),
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
                } if side == team && progress.millis() != 0 && ui.freeze_reachable(viewed, place) => {
                    Some(u32::from(progress.millis()))
                }
                _ => None,
            })
            .max()
            .unwrap_or(0)
    };
    if let Some(movie) = ui.hud_movie() {
        movie.invoke_uints(
            c"root.freeze_tag_score",
            &[u32::from(state.points_of(team)), u32::from(state.points_of(other))],
        );
        movie.invoke_uints(
            c"root.freeze_tag_update",
            &[
                frozen,
                progress,
                if round != 0 { round + 3 } else { outcome },
                remaining,
            ],
        );
    }
    if let Some(movie) = ui.icons_movie() {
        let mask = Place::all()
            .filter(|place| is_frozen(&state, *place))
            .fold(0, |mask, place| mask | (1 << place.index()));
        let mut text = [0; 0x200];
        movie.invoke_text(
            c"root.freeze_tag_label",
            ui.translate(c"st_freeze_marker", &mut text),
        );
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

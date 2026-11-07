//! Patch 87: shared semantic validation before either native world load.
//! The existing framing parser and weapon-pickup preparation keep one owner.
//! Every refusal is logged (`log::refusal`: at most a line a second,
//! counting the ones between).
#![forbid(unsafe_code)]

use srp_validate::{world, Part, Refusal, MAX_PLAYERS};

use crate::vostok::breadcrumbs::{self, Crumb, Reading};
use crate::vostok::log::{refusal as log_refusal, Piece};
use crate::vostok::{Context, Flow, Thread, WorldCheckOnRawExit, WorldCheckOnUnpackedExit, WorldLoad};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub game_mode: u8,
    pub batteries: usize,
    pub respawn_points: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rule {
    Mode { game_mode: u8, batteries: usize },
    Respawn { points: usize },
    Other,
}

impl Match {
    pub(crate) fn of(rules: impl IntoIterator<Item = Rule>) -> Option<Self> {
        let (mut mode, mut respawn) = (None, None);
        for rule in rules {
            match rule {
                Rule::Mode { game_mode, batteries } => {
                    if mode.replace((game_mode, batteries)).is_some() { return None; }
                }
                Rule::Respawn { points } => {
                    if respawn.replace(points).is_some() { return None; }
                }
                Rule::Other => {}
            }
        }
        let (game_mode, batteries) = mode?;
        Some(Self { game_mode, batteries, respawn_points: respawn? })
    }

    fn context(self, time: u32) -> world::Context<'static> {
        world::Context {
            time, places: MAX_PLAYERS, game_mode: self.game_mode,
            batteries: self.batteries, respawn_points: Some(self.respawn_points),
            // The native per-item bounds remain at their readers (patch 113).
            // Do not guess profile, swapped inventory or authored map objects.
            profiles: None, artefacts: None, objects: None, inventory: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure { NoTime, NoMatch, Invalid(Refusal) }

fn parse(state: &[u8], game: Option<Match>) -> Result<world::Frame<'_>, Failure> {
    let (time, body) = state.split_first_chunk::<4>().ok_or(Failure::NoTime)?;
    let context = game.ok_or(Failure::NoMatch)?.context(u32::from_le_bytes(*time));
    let frame = world::frame(body, context.places).map_err(Failure::Invalid)?;
    world::validate_frame(&frame, &context).map_err(Failure::Invalid)?;
    Ok(frame)
}

pub fn verdict(state: &[u8], game: Option<Match>) -> Result<(), Failure> {
    parse(state, game).map(|_| ())
}

pub fn on_unpacked(ctx: &mut Context<'_>) -> Flow<WorldCheckOnUnpackedExit> {
    if check(ctx.thread(), WorldLoad::at_unpacked(ctx)) { Flow::Continue }
    else { Flow::Exit(WorldCheckOnUnpackedExit::Refused) }
}
pub fn on_raw(ctx: &mut Context<'_>) -> Flow<WorldCheckOnRawExit> {
    if check(ctx.thread(), WorldLoad::at_raw(ctx)) { Flow::Continue }
    else { Flow::Exit(WorldCheckOnRawExit::Refused) }
}

/// A world the site could not read is refused before the native load. A world that parses is
/// loaded, patch 85's guns its records name made first
/// (`weapon_pickup::before_world_load`: outside any tick, before the load).
fn check(thread: &Thread, load: Option<WorldLoad<'_>>) -> bool {
    let Some(load) = load else {
        log_refusal(thread, &[Piece::Text("world_check: a world at a load with no readable state; not loaded")]);
        return false;
    };
    let parsed = parse(load.state(), load.match_context());
    let state = load.state();
    breadcrumbs::record(load.thread(), Crumb::World {
        time: state.get(..4).map_or(0, |t| u32::from_le_bytes([t[0], t[1], t[2], t[3]])),
        bytes: state.len() as u32,
        packed: load.packed(),
        local: load.network_client().local_profile(),
        loaded: parsed.is_ok(),
    });
    breadcrumbs::reading(load.thread(), Reading::World);
    match parsed {
        Ok(frame) => {
            crate::weapon_pickup::before_world_load(&load, frame.native());
            true
        }
        Err(error) => {
            let len = load.state().len();
            load.discard();
            log(thread, error, len);
            false
        }
    }
}

fn log(thread: &Thread, error: Failure, len: usize) {
    let (part, offset, slot) = match error {
        Failure::NoTime => ("missing time", 0, None),
        Failure::NoMatch => ("unknown match rules", 0, None),
        Failure::Invalid(refusal) => (
            refusal.part.name().trim_end_matches('\0'), refusal.offset,
            if let Part::Player(place) = refusal.part { Some(place) } else { None }),
    };
    log_refusal(thread, &[Piece::Text("world_check: a world refused, "), Piece::Text(part),
           Piece::Text(if slot.is_some() { " " } else { "" }),
           slot.map_or(Piece::Text(""), |slot| Piece::Dec(u32::from(slot))),
           Piece::Text(" at "), Piece::Hex(offset as u32),
           Piece::Text(" ("), Piece::Dec(len as u32), Piece::Text(" bytes); not loaded, a new one requested")]);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_match_has_exactly_one_mode_and_respawn_rule() {
        let mode = Rule::Mode { game_mode: 4, batteries: 0 };
        let respawn = Rule::Respawn { points: 3 };
        assert_eq!(Match::of([mode, Rule::Other, respawn]), Some(Match {
            game_mode: 4, batteries: 0, respawn_points: 3,
        }));
        for rules in [vec![], vec![mode], vec![respawn], vec![mode,mode,respawn],
                      vec![mode,respawn,respawn]] { assert_eq!(Match::of(rules), None); }
    }
    #[test]
    fn missing_time_match_and_incomplete_sections_are_refused() {
        let game = Some(Match { game_mode: 4, batteries: 0, respawn_points: 3 });
        assert_eq!(verdict(&[1,2,3], game), Err(Failure::NoTime));
        assert_eq!(verdict(&[0;20], None), Err(Failure::NoMatch));
        assert!(matches!(verdict(&[0;20], game), Err(Failure::Invalid(_))));
    }
}

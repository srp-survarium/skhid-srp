//! Patch 87: shared semantic validation before either native world load.
//! The existing framing parser and weapon-pickup preparation keep one owner.
#![forbid(unsafe_code)]
use srp_validate::{world, Part, Refusal, MAX_PLAYERS};
use crate::vostok::log::{line, Piece};
use crate::vostok::{Context, Flow, WorldCheckOnRawExit, WorldCheckOnUnpackedExit, WorldLoad};

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
            // The native per-item bounds remain at their readers (patch 94).
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
    if check(WorldLoad::at_unpacked(ctx)) { Flow::Continue }
    else { Flow::Exit(WorldCheckOnUnpackedExit::Refused) }
}
pub fn on_raw(ctx: &mut Context<'_>) -> Flow<WorldCheckOnRawExit> {
    if check(WorldLoad::at_raw(ctx)) { Flow::Continue }
    else { Flow::Exit(WorldCheckOnRawExit::Refused) }
}

fn check(load: Option<WorldLoad<'_>>) -> bool {
    let Some(load) = load else { return false; };
    match parse(load.state(), load.match_context()) {
        Ok(frame) => {
            crate::weapon_pickup::before_world_load(&load, frame.native());
            true
        }
        Err(error) => {
            log(error, load.state().len());
            load.leave();
            false
        }
    }
}

fn log(error: Failure, len: usize) {
    let (part, offset, slot) = match error {
        Failure::NoTime => ("missing time", 0, None),
        Failure::NoMatch => ("unknown match rules", 0, None),
        Failure::Invalid(refusal) => (
            refusal.part.name().trim_end_matches('\0'), refusal.offset,
            if let Part::Player(place) = refusal.part { Some(place) } else { None }),
    };
    line(&[Piece::Text("world_check: a world refused, "), Piece::Text(part),
           Piece::Text(if slot.is_some() { " " } else { "" }),
           slot.map_or(Piece::Text(""), |slot| Piece::Dec(u32::from(slot))),
           Piece::Text(" at "), Piece::Hex(offset as u32),
           Piece::Text(" ("), Piece::Dec(len as u32), Piece::Text(" bytes); the match left")]);
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

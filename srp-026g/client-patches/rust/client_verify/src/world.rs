//! Patch 75, the world: the 0xE8 handler's two calls of
//! game_world_core::deserialize (0x27C0D9, 0x27C111) -> [`world_hook`]
//! (cv_world_load). The match is the world core's rule list, read once into
//! a [`Match`]; the world is the reader's bytes (its time, then the body);
//! the check is `srp_validate::world::validate`. Refused, the reader is
//! consumed and the match left, and the entry leaves the handler by its
//! epilogue (client_world_check.md sections 4, 5 and 7).

use patch_core::Field;
use srp_validate::world::{self, Context};
use srp_validate::{MAX_PLAYERS, mode};

use crate::Verdict;

/// game_world_core (`[network_client+1F1C40h] + 1F8h`).
pub enum WorldCore {}
/// Its rule list, `std::vector<rule*>` (client_world_check.md section 7).
pub const RULES_BEGIN: Field<WorldCore, u32> = Field::at(0x1_8824);
pub const RULES_END: Field<WorldCore, u32> = Field::at(0x1_8828);
/// A list of more is no match's: the scan stops there.
pub const MAX_RULES: u32 = 64;

/// A rule object of the list (a game_match_rule_base).
pub enum RuleObject {}
pub const VTABLE: Field<RuleObject, u32> = Field::at(0);
/// A gather or vi-spawn rule's batteries, 4 bytes each (0x313D10, 0x3149B0).
pub const BATTERIES_BEGIN: Field<RuleObject, u32> = Field::at(0x110);
pub const BATTERIES_END: Field<RuleObject, u32> = Field::at(0x114);
/// The respawn rule's three groups' counts (0x316F49..0x316FB6).
pub const RESPAWN_GROUPS: [Field<RuleObject, u32>; 3] =
    [Field::at(0x194), Field::at(0x198), Field::at(0x19C)];

/// The rules' vtables (the address after each class's RTTI pointer; a base
/// and a networked one of each, section 7). Research's is ours (patch 53).
pub const GATHER: [u32; 2] = [0x00A0_6AB4, 0x00A1_5B04];
pub const VI_SPAWN: [u32; 2] = [0x00A0_6C04, 0x00A1_5CAC];
pub const TDM: [u32; 2] = [0x00A0_6CBC, 0x00A1_5DA4];
pub const RESPAWN: [u32; 2] = [0x00A0_6D1C, 0x00A1_6084];

/// A rule of the core's list, by its class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// A mode's own rule: the mode, and the batteries it holds (modes 2, 3).
    Mode { game_mode: u8, batteries: usize },
    /// The respawn rule: its respawn points.
    Respawn { points: usize },
    /// The timelimit, the kd stats: nothing the check needs.
    Other,
}

impl Rule {
    /// The rule of a vtable; `research` is Research's (an arena address).
    /// `batteries` and `points` read the object's fields when they are
    /// needed.
    pub fn of(
        vtable: u32,
        research: u32,
        batteries: impl FnOnce() -> usize,
        points: impl FnOnce() -> usize,
    ) -> Rule {
        if GATHER.contains(&vtable) {
            Rule::Mode {
                game_mode: mode::BATTERIES,
                batteries: batteries(),
            }
        } else if VI_SPAWN.contains(&vtable) {
            Rule::Mode {
                game_mode: mode::TIMED_BATTERIES,
                batteries: batteries(),
            }
        } else if TDM.contains(&vtable) {
            Rule::Mode {
                game_mode: mode::TEAM_DEATHMATCH,
                batteries: 0,
            }
        } else if vtable == research {
            Rule::Mode {
                game_mode: mode::RESEARCH,
                batteries: 0,
            }
        } else if RESPAWN.contains(&vtable) {
            Rule::Respawn { points: points() }
        } else {
            Rule::Other
        }
    }
}

/// What the check needs of the match a world is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub game_mode: u8,
    pub batteries: usize,
    pub respawn_points: usize,
}

/// The scan of a rule list: the mode's rule, seen once or more.
#[derive(Clone, Copy)]
enum Modes {
    None,
    One {
        game_mode: u8,
        batteries: usize,
    },
    /// Two mode rules: no match the check knows.
    Many,
}

impl Match {
    /// The match of a rule list: `None` when it has no mode rule, more than
    /// one, or no respawn rule (a match the check does not know: the world
    /// is refused, strict and loud).
    pub fn of(rules: impl IntoIterator<Item = Rule>) -> Option<Match> {
        let mut modes = Modes::None;
        let mut respawn_points = None;
        for rule in rules {
            match rule {
                Rule::Mode {
                    game_mode,
                    batteries,
                } => {
                    modes = match modes {
                        Modes::None => Modes::One {
                            game_mode,
                            batteries,
                        },
                        _ => Modes::Many,
                    }
                }
                Rule::Respawn { points } => respawn_points = Some(points),
                Rule::Other => {}
            }
        }
        match (modes, respawn_points) {
            (
                Modes::One {
                    game_mode,
                    batteries,
                },
                Some(respawn_points),
            ) => Some(Match {
                game_mode,
                batteries,
                respawn_points,
            }),
            _ => None,
        }
    }

    /// The check's context for a world of time `time`: the 20 places of the
    /// late join's growing roster (both serializers write 20 records), the
    /// rest unknown, as the server leaves it, so that no good world is
    /// refused over a value the game would have to guess.
    pub fn context(self, time: u32) -> Context<'static> {
        Context {
            time,
            places: MAX_PLAYERS,
            game_mode: self.game_mode,
            batteries: self.batteries,
            respawn_points: Some(self.respawn_points),
            profiles: None,
            artefacts: None,
            objects: None,
            inventory: None,
        }
    }
}

/// A world as the game's reader has it at 0x2D0B70: its time (u32), then
/// the body `world::validate` reads, of the match `game` (none: a match the
/// check does not know).
pub fn check_world(state: &[u8], game: Option<Match>) -> Verdict {
    match (state.split_first_chunk::<4>(), game) {
        (Some((time, body)), Some(game)) => {
            world::validate(body, &game.context(u32::from_le_bytes(*time))).into()
        }
        _ => Verdict::Refused,
    }
}

#[cfg(target_arch = "x86")]
mod game {
    use super::*;
    use crate::BufferReader;
    use crate::game::{consume, leave, rest};
    use patch_core::{Obj, Regs, Va};

    unsafe extern "C" {
        /// Research's rule (patch 53, research_native_026g.c): its vtable
        /// is this table's second entry, the first its locator.
        static rn_rule_vtable: [u32; 21];
    }

    /// The match the core's rule list says.
    unsafe fn match_of(core: Obj<WorldCore>) -> Option<Match> {
        unsafe {
            let (begin, end) = (core.get(RULES_BEGIN), core.get(RULES_END));
            let research = (&raw const rn_rule_vtable as usize as u32).wrapping_add(4);
            let count = (end.wrapping_sub(begin) / 4).min(MAX_RULES);
            Match::of((0..count).filter_map(|n| {
                let rule = Obj::<RuleObject>::at(Va(*Va(begin.wrapping_add(4 * n)).ptr::<u32>()))?;
                Some(Rule::of(
                    rule.get(VTABLE),
                    research,
                    || {
                        (rule
                            .get(BATTERIES_END)
                            .wrapping_sub(rule.get(BATTERIES_BEGIN))
                            / 4) as usize
                    },
                    || {
                        RESPAWN_GROUPS
                            .into_iter()
                            .fold(0u32, |sum, at| sum.wrapping_add(rule.get(at)))
                            as usize
                    },
                ))
            }))
        }
    }

    /// cv_world_load: the game's stack [1] the world core, [2] the reader;
    /// EDI the network client. ZF set: the load goes on; clear: refused,
    /// the reader consumed and the match left.
    ///
    /// # Safety
    /// The 0xE8 handler's call of 0x2D0B70, on the game's thread.
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn world_hook(regs: &mut Regs) {
        unsafe {
            let reader = Obj::<BufferReader>::at(Va(regs.stack(2)));
            let core = Obj::<WorldCore>::at(Va(regs.stack(1)));
            let verdict = match (reader, core) {
                (Some(reader), Some(core)) => check_world(rest(reader), match_of(core)),
                _ => Verdict::Refused,
            };
            if verdict == Verdict::Refused {
                if let Some(reader) = reader {
                    consume(reader);
                }
                leave(regs.edi);
            }
            regs.set_zero_flag(verdict == Verdict::Read);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESEARCH: u32 = 0x0A42_FA04;
    const TIMELIMIT_VTABLE: u32 = 0x00A0_6E2C;
    const KD_VTABLE: u32 = 0x00A1_77D4;

    fn rule(vtable: u32) -> Rule {
        Rule::of(vtable, RESEARCH, || 10, || 36)
    }

    #[test]
    fn a_rule_list_says_its_match() {
        let list = [
            rule(TIMELIMIT_VTABLE),
            rule(RESPAWN[1]),
            rule(GATHER[0]),
            rule(KD_VTABLE),
        ];
        assert_eq!(
            Match::of(list),
            Some(Match {
                game_mode: mode::BATTERIES,
                batteries: 10,
                respawn_points: 36
            })
        );
        let research = [rule(RESPAWN[0]), rule(RESEARCH)];
        assert_eq!(
            Match::of(research).map(|m| (m.game_mode, m.batteries)),
            Some((mode::RESEARCH, 0))
        );
        assert_eq!(
            Match::of([rule(TDM[0]), rule(RESPAWN[0])]).map(|m| m.game_mode),
            Some(mode::TEAM_DEATHMATCH)
        );
        assert_eq!(
            Match::of([rule(VI_SPAWN[1]), rule(RESPAWN[0])]).map(|m| m.game_mode),
            Some(mode::TIMED_BATTERIES)
        );
    }

    #[test]
    fn a_list_the_check_does_not_know_is_no_match() {
        // No mode rule, two, no respawn rule.
        assert_eq!(Match::of([rule(RESPAWN[0]), rule(KD_VTABLE)]), None);
        assert_eq!(
            Match::of([rule(RESPAWN[0]), rule(GATHER[0]), rule(TDM[0])]),
            None
        );
        assert_eq!(Match::of([rule(GATHER[0])]), None);
        assert_eq!(check_world(&[0; 64], None), Verdict::Refused);
    }

    #[test]
    fn a_world_is_its_time_then_its_body() {
        let game = Some(Match {
            game_mode: mode::BATTERIES,
            batteries: 0,
            respawn_points: 0,
        });
        // Too short for a time; a body that does not walk.
        assert_eq!(check_world(&[1, 2, 3], game), Verdict::Refused);
        assert_eq!(
            check_world(&[0, 0, 0, 0, 1, 0, 0, 0], game),
            Verdict::Refused
        );
    }
}

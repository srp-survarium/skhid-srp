//! Native rule objects become owned match parameters before bytes are parsed.
use super::world::read;
use crate::raw;
use crate::world_check::{Match, Rule};
use srp_validate::mode;

pub(super) fn from_client(client: raw::NetworkClient) -> Option<Match> {
    // SAFETY: WorldLoad owns this synchronous load boundary. The match core
    // and its rule vector remain alive and unmodified until it returns.
    unsafe {
        let core = client.pvp_match()?.core();
        let begin = core.rules_begin()?;
        let count = core.rules_end()?.index_from(begin)?;
        if count > 64 { return None; }
        let mut rules = [Rule::Other; 64];
        for (index, target) in rules.iter_mut().take(count).enumerate() {
            let rule = begin.at(index).rule()?;
            let vtable = read(rule.vtable());
            *target = match vtable {
                raw::image::GATHER_RULE_VTABLE | raw::image::GATHER_CLIENT_RULE_VTABLE =>
                    Rule::Mode { game_mode: mode::BATTERIES, batteries: batteries(rule)? },
                raw::image::VI_SPAWN_RULE_VTABLE | raw::image::VI_SPAWN_CLIENT_RULE_VTABLE =>
                    Rule::Mode { game_mode: mode::TIMED_BATTERIES, batteries: batteries(rule)? },
                raw::image::TDM_RULE_VTABLE | raw::image::TDM_CLIENT_RULE_VTABLE =>
                    Rule::Mode { game_mode: mode::TEAM_DEATHMATCH, batteries: 0 },
                raw::image::RESPAWN_RULE_VTABLE | raw::image::RESPAWN_CLIENT_RULE_VTABLE => {
                    let points = read(rule.respawn0()).checked_add(read(rule.respawn1()))?
                        .checked_add(read(rule.respawn2()))? as usize;
                    Rule::Respawn { points }
                }
                // Research owns native rule type 5. Reading the base type
                // avoids a fixed address for its relocated arena vtable.
                _ if read(rule.rule_type()) == 5 => Rule::Mode { game_mode: mode::RESEARCH, batteries: 0 },
                _ => Rule::Other,
            };
        }
        Match::of(rules.into_iter().take(count))
    }
}

/// # Safety
/// `rule` is a live gather/vi_spawn rule, identified by its concrete vtable.
unsafe fn batteries(rule: raw::MatchRule) -> Option<usize> {
    // SAFETY: the caller proved the concrete layout. Read vector bounds
    // without following any battery pointer.
    let (begin, end) = unsafe { (rule.batteries_begin() as usize, rule.batteries_end() as usize) };
    let bytes = end.checked_sub(begin)?;
    (bytes.is_multiple_of(4) && bytes / 4 <= 16).then_some(bytes / 4)
}

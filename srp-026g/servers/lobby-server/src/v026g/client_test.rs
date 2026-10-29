//! TEST ONLY (cargo feature `client-test`): the client-test runner may choose the roles of the server bots of a Player vs Bots match
//! (`match-server` `bots/client_tests`, `docs/notes/client_tests.md`), so that a step can have a camper or a flanker where it needs
//! one. `srp` registers the hook; the lobby asks it once, when it forms the match. No hook set, or a hook with no seat for the
//! match, leaves the roles the lobby gives ([`survarium::v026g::loadouts::bot_roles`]) as they are.

use std::sync::OnceLock;

use survarium::v026g::loadouts::BotRole;

/// The match being formed: its map (`maps_dict` id) and per-team size, the teams of the server's bots by place, and the roles the
/// lobby would give them.
pub struct Roster<'a> {
    pub map_id: u8,
    pub per_team: u8,
    pub bot_teams: &'a [u8],
    pub defaults: Vec<BotRole>,
}

/// Gives the roles of the match's bots, by place.
pub type RolesHook = Box<dyn Fn(Roster<'_>) -> Vec<BotRole> + Send + Sync>;

static HOOK: OnceLock<RolesHook> = OnceLock::new();

/// Registers the hook (once for the process).
pub fn set_roles_hook(hook: RolesHook) {
    let _ = HOOK.set(hook);
}

/// The roles of a Player vs Bots match's bots: the hook's, else `defaults`. A match not against bots (`size` none) is untouched.
pub(super) fn roles(size_and_map: Option<(u8, u8)>, bot_teams: &[u8], defaults: Vec<BotRole>) -> Vec<BotRole> {
    let (Some((per_team, map_id)), Some(hook)) = (size_and_map, HOOK.get()) else {
        return defaults;
    };
    let given = hook(Roster { map_id, per_team, bot_teams, defaults: defaults.clone() });
    if given.len() == defaults.len() { given } else { defaults }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No hook: the lobby's roles, whatever the match; a match not against bots is never asked.
    #[test]
    fn without_a_hook_the_lobbys_roles_stand() {
        let defaults = vec![BotRole::Flanker, BotRole::Support];
        assert_eq!(roles(Some((6, 2)), &[0, 1], defaults.clone()), defaults);
        assert_eq!(roles(None, &[0, 1], defaults.clone()), defaults);
    }
}

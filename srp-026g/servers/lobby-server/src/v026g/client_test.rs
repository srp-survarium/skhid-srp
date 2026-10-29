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

/// Why the hook's roles were not used.
#[derive(Debug, PartialEq, Eq)]
pub enum HookError {
    /// A hook is registered already (one per process): the new one is not used.
    AlreadySet,
    /// The hook gave `got` roles for `wanted` bots.
    WrongLength { wanted: usize, got: usize },
}

/// Registers the hook (once for the process); a second registration is an error and the first stays.
pub fn set_roles_hook(hook: RolesHook) -> Result<(), HookError> {
    HOOK.set(hook).map_err(|_| HookError::AlreadySet)
}

/// The roles of a Player vs Bots match's bots: the hook's, else `defaults`. A match not against bots (`size` none) is untouched. A
/// hook that fails is logged as an error and the lobby's roles stand (the runner checks the roles of its seats when a try starts, so
/// the test is voided and not reported against the wrong bot).
pub(super) fn roles(size_and_map: Option<(u8, u8)>, bot_teams: &[u8], defaults: Vec<BotRole>) -> Vec<BotRole> {
    match roles_with(HOOK.get(), size_and_map, bot_teams, defaults.clone()) {
        Ok(roles) => roles,
        Err(error) => {
            log::error!("CLIENT_TEST: the roles hook's answer is not used: {error:?}");
            defaults
        }
    }
}

/// [`roles`] with the hook given.
fn roles_with(
    hook: Option<&RolesHook>,
    size_and_map: Option<(u8, u8)>,
    bot_teams: &[u8],
    defaults: Vec<BotRole>,
) -> Result<Vec<BotRole>, HookError> {
    let (Some((per_team, map_id)), Some(hook)) = (size_and_map, hook) else {
        return Ok(defaults);
    };
    let wanted = defaults.len();
    let given = hook(Roster { map_id, per_team, bot_teams, defaults });
    if given.len() == wanted { Ok(given) } else { Err(HookError::WrongLength { wanted, got: given.len() }) }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No hook: the lobby's roles, whatever the match; a match not against bots is never asked.
    #[test]
    fn without_a_hook_the_lobbys_roles_stand() {
        let defaults = vec![BotRole::Flanker, BotRole::Support];
        assert_eq!(roles_with(None, Some((6, 2)), &[0, 1], defaults.clone()), Ok(defaults.clone()));
        assert_eq!(roles_with(None, None, &[0, 1], defaults.clone()), Ok(defaults));
    }

    /// The hook's roles are the match's bots' roles, the hook is given the match (map, size, the bots' teams, the lobby's roles), and
    /// a match not against bots is never asked; an answer of the wrong length is an error.
    #[test]
    fn the_hooks_roles_are_applied_to_the_matchs_bots() {
        let defaults = vec![BotRole::Flanker, BotRole::Support, BotRole::Rusher];
        let hook: RolesHook = Box::new(|roster| {
            assert_eq!((roster.map_id, roster.per_team, roster.bot_teams.len()), (2, 6, 3));
            let mut roles = roster.defaults;
            roles[1] = BotRole::Camper;
            roles
        });
        let given = roles_with(Some(&hook), Some((6, 2)), &[0, 1, 1], defaults.clone());
        assert_eq!(given, Ok(vec![BotRole::Flanker, BotRole::Camper, BotRole::Rusher]));
        assert_eq!(roles_with(Some(&hook), None, &[0, 1, 1], defaults.clone()), Ok(defaults.clone()));
        let short: RolesHook = Box::new(|_| vec![BotRole::Camper]);
        assert_eq!(roles_with(Some(&short), Some((6, 2)), &[0, 1, 1], defaults), Err(HookError::WrongLength { wanted: 3, got: 1 }));
    }
}

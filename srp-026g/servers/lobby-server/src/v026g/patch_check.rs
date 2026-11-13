//! A game older than the server takes does not get in
//! (`SRP_MIN_PATCH_REVISION`; unset or 0: no check).
//!
//! A patched exe says its patch revision in its sign-in (`docs/notes/
//! client_patch.md` §6), and the session keeps it. An older game's sign-in
//! is answered with the game's own "You're using an outdated version of the
//! game." (0x22, the login server's `RevisionGate`, which srp sets from this
//! setting), and a lobby sign-in with a session it had before is refused
//! like an ended one ([`Core::outdated_session`]), the session ended with
//! it. A game that reconnects after a restart of the server is so sent to
//! its login screen once its five tries fail, about half a minute
//! (`transport.md` §6.1), and there it is refused. It never gets a sign-in's
//! pushes: they name items (the price list 0x8A lists all that is sold), and
//! a game crashes on an item its resources.db lacks. The player restarts the
//! game through the launcher, which updates it.
//!
//! A test bot (`SRP_BOT_ACCOUNTS`) signs in as revision 0 and is never
//! checked.

use session::LockExt;

use super::core::Core;

/// `SRP_MIN_PATCH_REVISION`: the least patch revision of a game let in, 0
/// (unset or empty) for none. A value that is not a number from 0 to 255 is an
/// error: srp checks it before it starts a server, and stops, rather than
/// let older games in.
pub fn min_patch_revision_from_env() -> Result<u8, String> {
    parse_min_patch_revision(std::env::var("SRP_MIN_PATCH_REVISION").ok().as_deref())
}

pub(super) fn parse_min_patch_revision(value: Option<&str>) -> Result<u8, String> {
    match value.map(str::trim) {
        None | Some("") => Ok(0),
        Some(number) => number.parse().map_err(|_| {
            format!("SRP_MIN_PATCH_REVISION is `{number}`; it must be a number from 0 to 255")
        }),
    }
}

/// Whether `min` (`SRP_MIN_PATCH_REVISION`) keeps out every game without a
/// patch of the server's game data `data`. A patch is
/// `resources.db.patch_<n>`, n the client patch revision it was released
/// with, and a game of revision n or later has it (its updater keeps PLAY
/// off until it does). A game of an older revision lacks its items, and
/// the lobby's sign-in names every item sold: the game would crash. So srp
/// does not start with `min` under the highest n, and says what to set.
pub fn check_min_patch_revision(
    min: u8,
    data: &survarium::v026g::game_data::GameData,
) -> Result<(), String> {
    let Some(highest) = data
        .provenance
        .patches
        .iter()
        .map(|patch| patch.number)
        .max()
    else {
        return Ok(());
    };
    if u32::from(min) >= highest {
        return Ok(());
    }
    Err(format!(
        "SRP_MIN_PATCH_REVISION is {min}{}, but the server's game data ({}) has \
         resources.db.patch_{highest}: a game of an older revision lacks that patch's items and \
         crashes on them. Set SRP_MIN_PATCH_REVISION={highest} in .env and recreate the \
         container (docker compose up -d --no-deps --force-recreate srp26; a restart keeps \
         the old setting)",
        if min == 0 { " (or unset)" } else { "" },
        data.origin
    ))
}

impl Core<'_> {
    /// Whether a lobby sign-in with `session_id` is refused for its game's
    /// revision: the account's name and the revision, if so. Only a session
    /// from before a restart can be one: the login server lets no older game
    /// in.
    pub(super) fn outdated_session(&self, session_id: u32) -> Option<(String, u8)> {
        let revision = self.store.client_revision(session_id)?;
        if revision >= self.settings.min_patch_revision {
            return None;
        }
        let name = self
            .store
            .account_for_session(session_id)?
            .locked()
            .name()
            .to_owned();
        (!self.accounts.is_bot_name(&name)).then_some((name, revision))
    }
}

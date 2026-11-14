//! One namespace for display names. The login lock serializes every claim.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::nickname_key::{key, replacement};
use crate::persist::PersistError;
use crate::v026g::names::{Nickname, RenameError};
use crate::v026g::{GameAccounts, names, premium};
use crate::{LockExt, ResetWrite, SessionStore};

/// An administrator's free rename, already applied in memory.
pub struct AdminRename {
    pub previous: String,
    pub written: ResetWrite,
}

enum Charge<'a> {
    Player(&'a premium::Settings),
    Admin,
}

pub(crate) fn unique_names<'a>(
    names: impl IntoIterator<Item = (u32, &'a str)>,
) -> Result<(), PersistError> {
    let mut owners = HashMap::new();
    for (id, name) in names {
        if let Some(other) = owners.insert(key(name), id) {
            return Err(PersistError::BadRow(
                id,
                format!("nickname {name:?} also belongs to account {other}"),
            ));
        }
    }
    Ok(())
}

impl SessionStore {
    /// Initialize the lobby account from its authoritative name while no
    /// administrator can rename it between reading that name and creating it.
    pub fn sign_up_026g(
        &self,
        games: &GameAccounts,
        account_id: u32,
    ) -> Option<(Arc<Mutex<crate::v026g::Account>>, bool)> {
        let _names = self.logins.locked();
        let name = self.account(account_id)?.locked().name.clone();
        Some(games.sign_up(account_id, &name))
    }

    /// Restore the game's display-name copies from their owner before the
    /// lobby starts. A crash may have interrupted the two savers' passes.
    pub fn restore_026g_names(&self, games: &GameAccounts) {
        let accounts: Vec<_> = self.accounts.locked().values().cloned().collect();
        for account in accounts {
            let (id, name) = {
                let account = account.locked();
                (account.id, account.name.clone())
            };
            if let Some(game) = games.get(id) {
                let mut game = game.locked();
                if game.name != name {
                    game.name = name;
                    games.mark_dirty(id);
                }
            }
        }
    }

    fn display_name_taken(&self, name: &str, except: Option<u32>) -> bool {
        let accounts: Vec<_> = self.accounts.locked().values().cloned().collect();
        accounts.iter().any(|account| {
            let account = account.locked();
            Some(account.id) != except && key(&account.name) == key(name)
        })
    }

    fn nickname_taken_locked(
        &self,
        logins: &HashMap<String, u32>,
        name: &str,
        except: Option<u32>,
    ) -> bool {
        logins
            .iter()
            .any(|(login, id)| Some(*id) != except && key(login) == key(name))
            || self.display_name_taken(name, except)
    }

    /// The display name of the new account `account_id`, whose user name
    /// shows as `name`: that name, or its replacement
    /// ([`crate::nickname_key::replacement`]) when it is another account's
    /// name or login. Nobody is refused an account for a name it did not
    /// choose; the player renames it later.
    pub(super) fn sign_up_name_locked(
        &self,
        logins: &HashMap<String, u32>,
        name: String,
        account_id: u32,
    ) -> String {
        let taken = |name: &str| self.nickname_taken_locked(logins, name, None);
        if !taken(&name) {
            return name;
        }
        let to = replacement(&name, account_id, taken);
        log::info!("Account {account_id}: the name {name:?} is another account's; it is {to:?}");
        to
    }

    /// A UI availability hint. The mutation rechecks under the same lock
    /// as registration, before charging or changing anything.
    pub fn nickname_taken(&self, name: &str, account_id: u32) -> bool {
        self.nickname_taken_locked(&self.logins.locked(), name, Some(account_id))
    }

    /// Pay and rename one 0.26g account while registration and other renames
    /// cannot claim its name. Login credentials and account id stay unchanged.
    pub fn rename_026g(
        &self,
        games: &GameAccounts,
        settings: &premium::Settings,
        account_id: u32,
        nickname: &Nickname,
    ) -> Result<u32, RenameError> {
        let logins = self.logins.locked();
        self.change_nickname_locked(
            games,
            &logins,
            account_id,
            nickname,
            Charge::Player(settings),
        )
        .map(|(_, paid)| paid)
    }

    /// Rename by immutable account ID without charging or ending sessions.
    /// The shared account is written immediately when the writer is available;
    /// a refused/busy write stays dirty for the saver to retry. The caller writes
    /// the game's copy with its admin audit row and notifies the lobby.
    pub fn admin_rename_026g(
        &self,
        games: &GameAccounts,
        account_id: u32,
        nickname: &Nickname,
    ) -> Result<AdminRename, RenameError> {
        let logins = self.logins.locked();
        let (previous, _) =
            self.change_nickname_locked(games, &logins, account_id, nickname, Charge::Admin)?;
        let written = self.write_now(account_id, &[]);
        self.save_soon();
        Ok(AdminRename { previous, written })
    }

    fn change_nickname_locked(
        &self,
        games: &GameAccounts,
        logins: &HashMap<String, u32>,
        account_id: u32,
        nickname: &Nickname,
        charge: Charge<'_>,
    ) -> Result<(String, u32), RenameError> {
        if self.nickname_taken_locked(logins, nickname.as_str(), Some(account_id)) {
            return Err(RenameError::Taken);
        }
        let account = self.account(account_id).ok_or(RenameError::NoAccount)?;
        let game = games.get(account_id);
        let paid = match charge {
            Charge::Player(settings) => {
                let game = game.ok_or(RenameError::NoAccount)?;
                names::rename(&mut game.locked(), settings, nickname.as_str())?
            }
            Charge::Admin => {
                if let Some(game) = game {
                    game.locked().name = nickname.as_str().to_owned();
                }
                0
            }
        };
        let previous = {
            let mut account = account.locked();
            account.game.name = nickname.as_str().to_owned();
            std::mem::replace(&mut account.name, nickname.as_str().to_owned())
        };
        self.mark_dirty(account_id);
        games.mark_dirty(account_id);
        Ok((previous, paid))
    }
}

#[cfg(test)]
mod tests;

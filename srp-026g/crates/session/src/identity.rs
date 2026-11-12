//! One namespace for display names. The login lock serializes every claim.

use std::collections::HashMap;

use crate::nickname_key::key;
use crate::persist::PersistError;
use crate::v026g::names::{Nickname, RenameError};
use crate::v026g::{GameAccounts, names, premium};
use crate::{LockExt, SessionStore};

pub(crate) fn unique_names<'a>(
    names: impl IntoIterator<Item = (u32, &'a str)>,
) -> Result<(), PersistError> {
    let mut owners = HashMap::new();
    for (id, name) in names {
        if let Some(other) = owners.insert(key(name), id) {
            return Err(PersistError::BadRow(
                id,
                format!(
                    "nickname {name:?} also belongs to account {other}; repair duplicate nicknames before starting"
                ),
            ));
        }
    }
    Ok(())
}

impl SessionStore {
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

    pub(super) fn display_name_taken(&self, name: &str, except: Option<u32>) -> bool {
        let accounts: Vec<_> = self.accounts.locked().values().cloned().collect();
        accounts.iter().any(|account| {
            let account = account.locked();
            Some(account.id) != except && key(&account.name) == key(name)
        })
    }

    pub(super) fn nickname_taken_locked(
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
        if self.nickname_taken_locked(&logins, nickname.as_str(), Some(account_id)) {
            return Err(RenameError::Taken);
        }
        let account = self.account(account_id).ok_or(RenameError::NoAccount)?;
        let game = games.get(account_id).ok_or(RenameError::NoAccount)?;
        let paid = names::rename(&mut game.locked(), settings, nickname.as_str())?;
        {
            let mut account = account.locked();
            account.name = nickname.as_str().to_owned();
            account.game.name = nickname.as_str().to_owned();
        }
        self.mark_dirty(account_id);
        games.mark_dirty(account_id);
        Ok(paid)
    }
}

#[cfg(test)]
mod tests;

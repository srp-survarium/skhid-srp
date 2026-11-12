//! Player reports and compliments. A successful submission means the repository
//! has stored it; feedback alone applies no penalty or reward.

use survarium::v026g::lobby::requests::PlayerComment;

use super::GameAccounts;
use crate::persist::PersistError;

/// Names are snapshots; account ids keep feedback attached after a rename.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredComment {
    /// Server time, Unix seconds.
    pub at: u64,
    pub sender_id: u32,
    pub sender_name: String,
    pub target_id: u32,
    pub target_name: String,
    pub comment: PlayerComment,
}

impl StoredComment {
    pub(super) fn same_daily_choice(&self, other: &Self) -> bool {
        self.sender_id == other.sender_id
            && self.target_id == other.target_id
            && self.comment == other.comment
            && self.at / 86_400 == other.at / 86_400
    }
}

impl GameAccounts {
    /// Called without any account locked. Never acknowledge feedback that is
    /// merely queued in memory or could not be written. A busy saver is retried
    /// by the player, rather than blocking the lobby on its whole pass.
    pub fn save_player_comment(&self, comment: &StoredComment) -> Result<(), PersistError> {
        let writer = self.writer.as_ref().ok_or_else(|| {
            PersistError::NotWritten("player comments require an enabled repository".into())
        })?;
        let mut writer = match writer.try_lock() {
            Ok(writer) => writer,
            Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err(PersistError::NotWritten("the database is busy".into()));
            }
        };
        writer.repository.save_player_comment(comment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LockExt;
    use crate::v026g::persist::{GameRepository, MemoryGameRepository};

    #[test]
    fn player_comments_are_not_acknowledged_without_storage_or_while_busy() {
        let record = StoredComment {
            at: 1,
            sender_id: 1,
            sender_name: "Alice".into(),
            target_id: 2,
            target_name: "Bob".into(),
            comment: PlayerComment::GoodPlayer,
        };
        assert!(GameAccounts::new().save_player_comment(&record).is_err());
        let mut repository = MemoryGameRepository::default();
        let accounts = GameAccounts::with_repository(Box::new(repository.clone())).unwrap();
        let held = accounts.writer.as_ref().unwrap().locked();
        assert!(accounts.save_player_comment(&record).is_err());
        assert!(repository.player_comments(2, 10).unwrap().is_empty());
        drop(held);
        accounts.save_player_comment(&record).unwrap();
        assert_eq!(repository.player_comments(2, 10).unwrap(), [record]);
    }
}

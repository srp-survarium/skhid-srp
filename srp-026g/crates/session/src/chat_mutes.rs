//! One saved chat policy per muted account, shared by lobby and match chat.

pub use survarium::v026g::lobby::ChatMuteMode;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatMutes(Vec<(u32, ChatMuteMode)>);

impl ChatMutes {
    pub fn mode(&self, account: u32) -> Option<ChatMuteMode> {
        self.0
            .iter()
            .find(|(id, _)| *id == account)
            .map(|(_, mode)| *mode)
    }

    /// Replace the policy, or remove the mute. There can never be two modes
    /// for the same account. Changing modes preserves the list's order.
    pub fn set(&mut self, account: u32, mode: Option<ChatMuteMode>) {
        match mode {
            Some(mode) => {
                if let Some(entry) = self.0.iter_mut().find(|(id, _)| *id == account) {
                    entry.1 = mode;
                } else {
                    self.0.push((account, mode));
                }
            }
            None => self.0.retain(|(id, _)| *id != account),
        }
    }

    pub fn contains(&self, account: &u32) -> bool {
        self.mode(*account).is_some()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &u32> {
        self.0.iter().map(|(id, _)| id)
    }

    pub fn entries(&self) -> impl Iterator<Item = (u32, ChatMuteMode)> + '_ {
        self.0.iter().copied()
    }
}

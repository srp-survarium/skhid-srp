use serde::{Deserialize, Serialize};
use survarium::v026g::loadouts::PocketStore;

/// What the bots' kits are built with (`survarium::v026g::loadouts`), read where a match is made
/// (`MatchRegistry::with_bot_pocket_store`) and where the bench builds its bots' profiles, each team by its own tuning.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KitTuning {
    /// Every pocket stack of a bot (medkits, bandages, traps, the grenade, the radio) has [`BOT_POCKET_STORE`]
    /// (100) units in storage, so every life starts with the stack (the owner, 2026-10-28: "storage put 100+ should
    /// be enough"; the client fills a slot from the storage at each spawn and takes back what is left at a death,
    /// PROVEN). Off: the stack's count in storage, spent for the match, as c3dc7241's profiles had it.
    ///
    /// [`BOT_POCKET_STORE`]: survarium::v026g::loadouts::BOT_POCKET_STORE
    pub pocket_store: bool,
}

impl Default for KitTuning {
    fn default() -> Self {
        Self { pocket_store: true }
    }
}

impl KitTuning {
    /// The store the team's kits are built with.
    pub fn store(&self) -> PocketStore {
        PocketStore::of(self.pocket_store)
    }
}

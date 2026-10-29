use serde::{Deserialize, Serialize};
use survarium::v026g::loadouts::{BotPocketKit, KitSmoke, PocketStore};

/// What the bots' kits are built with (`survarium::v026g::loadouts`), read where a match is made
/// (`MatchRegistry::with_bot_pockets`) and where the bench builds its bots' profiles, each team by its own tuning.
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
    /// [smoke] Whether and where every kit carries a smoke grenade (`rdg-m`, one a life, stored as the other stacks):
    /// `off`; `free_pocket`, in a free pocket after every grenade pocket only; `radio_place`, there or else in the
    /// radio's place (`loadouts::KitSmoke::place`). Never in place of a medkit, the bandages, a frag or a trap. The
    /// owner, 2026-10-29: "smoke can replace radio if no pocket available", and "bench decides" between the two (the
    /// radio is the respawn booster). Off until the bots' frag roll is capped by the frags their own kit holds: the
    /// throw key throws the first grenade of the pockets, so a bot out of frags would throw its smoke at a frag's target.
    pub smoke: KitSmoke,
}

impl Default for KitTuning {
    fn default() -> Self {
        Self {
            pocket_store: true,
            smoke: KitSmoke::Off,
        }
    }
}

impl KitTuning {
    /// The pockets the team's kits are built with: the store and the smoke.
    pub fn pockets(&self) -> BotPocketKit {
        BotPocketKit {
            store: PocketStore::of(self.pocket_store),
            smoke: self.smoke,
        }
    }
}

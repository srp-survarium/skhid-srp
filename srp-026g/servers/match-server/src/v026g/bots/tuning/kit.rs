use serde::{Deserialize, Serialize};
use survarium::v026g::loadouts::{BotPocketKit, KitFrags, KitMedkit, KitSmoke, PocketStore};

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
    /// radio is the respawn booster). `radio_place` by default: free_pocket gives no kit of the owner's a smoke, and the
    /// owner kept the radio's place at its measured cost (#801: +0.33 s dead a death, 0 radio respawns against 562 in 96
    /// pairs, batteries delivered -1.9 %; "Keep smoke in radio slot"). The smoke is thrown with its own QUICK key, and the
    /// frag roll is capped by the frags the kit holds (#738), so no frag's target gets a smoke.
    pub smoke: KitSmoke,
    /// [frags] How many frags the kits carry, by role (the owner's kit; the old kit gives every role two): `off`, the support
    /// carries one; `support_two`, the support two (25 % of the lives, 0.50 units a life); `support_and_camper`, the support two
    /// and the camper one in the bandages' pocket (40 % of the lives, 0.65 units a life). Live players carry a frag in 37.5 % of
    /// their lives, 1.72 units when they do, 0.645 units a life (`traces-20261028/frag-share-humans.md`, 39,547 lives). The
    /// owner, 2026-10-29: "Measure, then match". It only gives the grenade habit more to throw: the bots throw 1.8 frags
    /// per 100 lives, the players 20.5, and the gap per carrying life (under 8 per 100 against 54.5) is the habit's, not the kit's.
    pub frags: KitFrags,
    /// [medkit] The medkit in the first pocket (A311): `medikit`, the Medikit (the owner's kit of 2026-10-26); `military`, the
    /// military medkit, in every tier (item level 6; the owner, 2026-11-07: "yes low tier can carry it"). Both wait 2 s; the
    /// military medkit heals body 0.6 over 1 s (done 3 s after the press), the Medikit 1.0 over 3 s (done at 5 s)
    /// (`scripts/content-edit/medicines.tsv`). A ranged hit that took health stops a drug in use (patch 84) and the bots press
    /// on the move 1-2 s after a hit, so the shorter course is finished more often; the bandages (1 s + 1 s) stay in the
    /// second pocket for the rest.
    ///
    /// ON (`military`) by default (the owner, 2026-11-07: "for medkit i guess better ON - why not?"), on the final check's arm D
    /// (48 pairs, the players' data, 09688abf): bot kills +1.8 %, human deaths -11.1 %, drift over 1 m -4.7 %, stuck -6 %;
    /// the stops in the 2 s delay rise 6-13 % (the military medkit's 0.6 body takes a second press for a deep loss more
    /// often), full health 3-9 s after a press 90.0 -> 91.1 %. The first 16-pair read (team 0's kills -7.8 %) was one-sided:
    /// both bot teams carry the kit, and pooled it was neutral. Where the game data has no military medkit (0.26g's own
    /// archive, the tests' data) the kit keeps the Medikit (`KitMedkit::item`).
    pub medkit: KitMedkit,
}

impl Default for KitTuning {
    fn default() -> Self {
        Self {
            pocket_store: true,
            smoke: KitSmoke::RadioPlace,
            frags: KitFrags::SupportAndCamper,
            medkit: KitMedkit::Military,
        }
    }
}

impl KitTuning {
    /// The pockets the team's kits are built with: the store, the smoke, the frags and the medkit.
    pub fn pockets(&self) -> BotPocketKit {
        BotPocketKit {
            store: PocketStore::of(self.pocket_store),
            smoke: self.smoke,
            frags: self.frags,
            medkit: self.medkit,
        }
    }
}

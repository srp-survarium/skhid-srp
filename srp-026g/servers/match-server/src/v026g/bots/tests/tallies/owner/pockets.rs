//! The owner counts of the bots' pocket stacks (`stock::Pantry`, `kit.pocket_store`), by team: lives begun and those that
//! began short (a drug's slot below its pocket's units: the store held less than the stack), the units pressed by drug, and
//! the presses into an empty slot (the game does nothing then).
//!
//! What the bench can and cannot see of the stacks: the drugs follow the profile's store (the stand-in's heals, `standin_heal`),
//! the frags of the support follow it too (`standin_grenade`), the traps and the radio are not modelled in the stand-in (the
//! trap habit has its own lines: `trap looks`, placed per team), and a resupply box tops up no pocket stack there
//! (`standin_weapon` models the ammo only). Refills at a box are a client-test step. Grenades thrown per life are the live
//! grenade list's (`the live grenade list: ...`).

use super::*;
use crate::v026g::bots::stock::PantryCounts;

/// The bots' pantries, summed by team as the last tick left them (the pantry keeps its own counts).
#[derive(Default)]
pub(super) struct PocketCounts {
    by_team: [PantryCounts; 2],
}

impl Topic for PocketCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        self.by_team = bots.pantry_counts();
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("pockets", self.part())]
    }
}

impl PocketCounts {
    fn part(&self) -> Part {
        let team = |t: usize| {
            let c = &self.by_team[t];
            let [medkits, bandages, antitoxins] = c.pressed;
            serde_json::json!({
                "spawns": c.spawns,
                "short_spawns": c.short_spawns,
                "medkits": medkits,
                "bandages": bandages,
                "antitoxins": antitoxins,
                "empty_presses": c.empty_presses,
            })
        };
        let json = serde_json::json!({
            "pockets_team0": team(0),
            "pockets_team1": team(1),
        });
        let text = format!(
            "owner-test pockets: team 0 [{}]; team 1 [{}]",
            self.by_team[0].line(),
            self.by_team[1].line()
        );
        Part::new(json, text)
    }
}

/// The topic sums the pantries of a bench by team, and a team on the old store has later lives that begin short where the
/// team on the store of 100 has none.
#[test]
pub(crate) fn the_pockets_topic_sums_the_pantries_by_team_and_the_old_store_has_short_lives() {
    use crate::v026g::bots::stock::Pantry;
    use crate::v026g::bots::trace::Trace;
    use crate::v026g::bots::tuning::Tuning;
    use std::sync::Arc;
    use survarium::v026g::loadouts::{BotEquipment, BotRole, BotTier, PocketStore, RusherKits, bot_profile_stored};
    let equipment = BotEquipment::of(BotTier::Low, RusherKits::default());
    let teams = [0u8, 1];
    let mut bots = Bots::new(
        0x70C4,
        survarium::v026g::maps::mode::TEAM_DEATHMATCH,
        0,
        &teams,
        &[true; 2],
        &[None; 2],
        Arc::new(Tuning::default()),
        Trace::none(0),
    );
    let pantries = [PocketStore::PerLife, PocketStore::PerMatch]
        .into_iter()
        .zip(teams)
        .map(|(store, team)| {
            let mut pantry = Pantry::of_profile(&bot_profile_stored(1, team, BotRole::ALL[0], equipment, store));
            // Two lives: the medkits spent in the first.
            pantry.new_life(0);
            while pantry.pressed(0, 1_000) {}
            pantry.new_life(0);
            pantry
        })
        .collect();
    bots.set_pantries(pantries);
    let (game, _) = made_up(&teams);
    let mut topic = PocketCounts::default();
    topic.bots(&bots, &game);
    let part = topic.part();
    assert_eq!(part.json["pockets_team0"]["spawns"], 2);
    assert_eq!(part.json["pockets_team0"]["short_spawns"], 0);
    assert_eq!(part.json["pockets_team1"]["short_spawns"], 1, "the old store's second life begins without the medkits");
    assert_eq!(part.json["pockets_team1"]["medkits"], 2);
    assert!(part.text.starts_with("owner-test pockets: team 0 [spawns 2 (short 0), medkits 2"), "{}", part.text);
}

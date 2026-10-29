//! The owner counts of the server's live grenade list (`live_grenades.rs`) and the avoidance that reads it
//! (`grenade_threat.rs`), by the bots' team: the grenades the list holds by whose they are (a human's or a bot's from its
//! pockets, a bot's key with its pockets read and empty, a bot's key with its pockets unread), the grenades the bots of a
//! team came to know of (and how many a teammate's), the steps taken, the ones with no way out, the escapes and the catches
//! at the blast, and the assumed grenades a team's avoidance refused (`avoid.bot_pockets`).
//!
//! What the bench cannot see: it never gives the tracker the bots' pockets (`Bots::set_throws`, as the live server does in
//! `game/driven.rs` from `KitPockets::of_profile`), so every throw key of a stand-in body is read as the old
//! `GrenadeKeyIsFrag`: its grenades are listed as unread (`unread`, up to 10 a match) and every team dodges them, as the
//! tip's bots do. The counts of the pockets (`frags listed from pockets`, `assumed`, `smokes`) are 0 here until the bench
//! calls it. The avoidance counts are real: they come from the bots' own state against the stand-in's grenades.

use super::*;
use crate::v026g::bots::grenade_threat::AvoidStats;
use crate::v026g::bots::live_grenades::{FragLives, ThrowCounts, Whose};
use survarium::v026g::loadouts::BotRole;

/// The list's and the avoidance's counts as the last tick left them (the tracker and the bots keep them; the topic copies).
#[derive(Default)]
pub(super) struct LiveListCounts {
    by_team: [[AvoidStats; Whose::ALL.len()]; 2],
    throws: ThrowCounts,
    /// Rolls for a grenade refused because the kit held no frag, `[team][role]` by `BotRole::ALL`.
    refused_no_frag: [[u32; BotRole::ALL.len()]; 2],
    /// The frags the lives carried and threw, `[team][role]` (`Bots::frag_lives`).
    frag_lives: [[FragLives; BotRole::ALL.len()]; 2],
}

impl Topic for LiveListCounts {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn bots(&mut self, bots: &Bots, _game: &Game) {
        self.by_team = bots.grenade_counts();
        self.throws = bots.tracker.throw_counts();
        self.refused_no_frag = bots.frag_rolls_refused();
        self.frag_lives = bots.frag_lives();
    }

    fn parts(&mut self, _run: &Run, _game: &Game, _scale: &Scale, _shared: &Shared) -> Vec<(&'static str, Part)> {
        vec![("live_list", self.part())]
    }
}

impl LiveListCounts {
    fn part(&self) -> Part {
        let team_json = |team: usize| {
            let mut by_whose = serde_json::Map::new();
            for whose in Whose::ALL {
                let a = &self.by_team[team][whose.index()];
                by_whose.insert(
                    format!("{whose:?}").to_lowercase(),
                    serde_json::json!({
                        "noticed": a.noticed,
                        "noticed_a_mates": a.noticed_mate,
                        "stepped": a.stepped,
                        "already_out": a.held,
                        "out_at_act": a.out_at_act,
                        "no_way": a.no_way,
                        "escaped": a.escaped,
                        "caught": a.caught,
                        "refused": a.refused,
                    }),
                );
            }
            serde_json::Value::Object(by_whose)
        };
        let t = &self.throws;
        let refused_json = |by_role: &[u32; BotRole::ALL.len()]| {
            let mut map = serde_json::Map::new();
            for (role, &n) in BotRole::ALL.iter().zip(by_role) {
                map.insert(role.name().to_owned(), n.into());
            }
            serde_json::Value::Object(map)
        };
        let lives_json = |by_role: &[FragLives; BotRole::ALL.len()]| {
            let mut map = serde_json::Map::new();
            for (role, l) in BotRole::ALL.iter().zip(by_role) {
                map.insert(
                    role.name().to_owned(),
                    serde_json::json!({ "lives": l.lives, "carrying": l.carrying, "units_carried": l.carried, "thrown": l.thrown }),
                );
            }
            serde_json::Value::Object(map)
        };
        let json = serde_json::json!({
            "live_list_frag_lives_team0": lives_json(&self.frag_lives[0]),
            "live_list_frag_lives_team1": lives_json(&self.frag_lives[1]),
            "live_list_team0": team_json(0),
            "live_list_team1": team_json(1),
            "live_list_frags": t.frag,
            "live_list_assumed": t.assumed,
            "live_list_unread": t.unread,
            "live_list_quick_rules_differ": t.quick_rules_differ,
            "live_list_smokes": t.smoke,
            "live_list_no_pockets": t.no_pockets,
            "live_list_launched": t.launched,
            "live_list_unplaced": t.unplaced,
            "live_list_died_throwing": t.died_throwing,
            "live_list_frag_rolls_refused_team0": refused_json(&self.refused_no_frag[0]),
            "live_list_frag_rolls_refused_team1": refused_json(&self.refused_no_frag[1]),
        });
        let lives_sum = |team: usize, f: fn(&FragLives) -> u32| self.frag_lives[team].iter().map(f).sum::<u32>();
        let sum = |team: usize, f: fn(&AvoidStats) -> u32| self.by_team[team].iter().map(f).sum::<u32>();
        let text = format!(
            "owner-test live grenade list: frags from pockets {}, assumed from a bot's key with its pockets read {}, from a bot's key with its pockets unread {}, smokes {}, no pockets known {}, launched {}, unplaced {}, thrower dead {}; frag rolls refused, none in the kit: team 0 {}, team 1 {}; frags carried by lives read: team 0 {}/{}/{} (lives/carrying/units), thrown {}, team 1 {}/{}/{}, thrown {}; team 0 noticed {} (a mate's {}) stepped {} no way out {} escaped {} caught {} refused {}; team 1 noticed {} (a mate's {}) stepped {} no way out {} escaped {} caught {} refused {}",
            t.frag,
            t.assumed,
            t.unread,
            t.smoke,
            t.no_pockets,
            t.launched,
            t.unplaced,
            t.died_throwing,
            self.refused_no_frag[0].iter().sum::<u32>(),
            self.refused_no_frag[1].iter().sum::<u32>(),
            lives_sum(0, |l| l.lives),
            lives_sum(0, |l| l.carrying),
            lives_sum(0, |l| l.carried),
            lives_sum(0, |l| l.thrown),
            lives_sum(1, |l| l.lives),
            lives_sum(1, |l| l.carrying),
            lives_sum(1, |l| l.carried),
            lives_sum(1, |l| l.thrown),
            sum(0, |a| a.noticed.iter().sum()),
            sum(0, |a| a.noticed_mate),
            sum(0, |a| a.stepped.iter().sum()),
            sum(0, |a| a.no_way.iter().sum()),
            sum(0, |a| a.escaped.iter().sum()),
            sum(0, |a| a.caught.iter().sum()),
            sum(0, |a| a.refused),
            sum(1, |a| a.noticed.iter().sum()),
            sum(1, |a| a.noticed_mate),
            sum(1, |a| a.stepped.iter().sum()),
            sum(1, |a| a.no_way.iter().sum()),
            sum(1, |a| a.escaped.iter().sum()),
            sum(1, |a| a.caught.iter().sum()),
            sum(1, |a| a.refused),
        );
        Part::new(json, text)
    }
}

/// The topic copies the counts of the two teams' avoidance, split by basis, and the list's throws.
#[test]
pub(crate) fn the_live_list_topic_gives_the_teams_avoidance_by_basis() {
    let mut topic = LiveListCounts::default();
    topic.by_team[0][Whose::Bot.index()].noticed = [2, 0, 1];
    topic.by_team[0][Whose::Bot.index()].noticed_mate = 2;
    topic.by_team[0][Whose::Bot.index()].stepped = [1, 0, 0];
    topic.by_team[1][Whose::Assumed.index()].refused = 4;
    topic.throws.frag = 7;
    topic.refused_no_frag[0][2] = 3;
    let part = topic.part();
    assert_eq!(part.json["live_list_team0"]["bot"]["noticed_a_mates"], 2);
    assert_eq!(part.json["live_list_team1"]["assumed"]["refused"], 4);
    assert_eq!(part.json["live_list_frags"], 7);
    assert_eq!(part.json["live_list_frag_rolls_refused_team0"][BotRole::ALL[2].name()], 3);
    assert!(part.text.contains("frag rolls refused, none in the kit: team 0 3, team 1 0"), "{}", part.text);
    assert!(
        part.text.contains("team 0 noticed 3 (a mate's 2) stepped 1") && part.text.contains("team 1 noticed 0 (a mate's 0) stepped 0 no way out 0 escaped 0 caught 0 refused 4"),
        "{}",
        part.text
    );
}

//! Team damage (exe patch 63, `docs/notes/team-damage.md`): the registry's
//! rule goes into the static info of a match of team play only, the same
//! matches the anti-AFK watches ([`MatchKind::team_play`]), and a late
//! joiner's static info has the rule the match was made with.

use super::*;
use crate::v026g::registry::{LivePhase, MatchKind};
use crate::v026g::team_damage_setting;
use srp_validate::team_damage::Rule;
use survarium::v026g::maps::{self, GameMode};
use survarium::v026g::match_msgs::static_info::parse_options;

fn rule_of(registry: &MatchRegistry, match_id: u32) -> Option<Rule> {
    let formed = registry.formed(match_id).expect("the match");
    let options = parse_options(formed.info.options(), Dictionary::shared()).expect("options a client reads");
    assert_eq!(options.team_damage, formed.team_damage, "the options say what the match keeps");
    options.team_damage
}

#[test]
fn only_a_match_of_team_play_has_the_rule() {
    let registry = MatchRegistry::new("127.0.0.1", 1236).with_team_damage(Some(Rule::ORIGINAL));
    let free_for_all = maps::find(1, maps::weather::DAY, GameMode::FreeForAll).expect("level_01's free-for-all").id;
    for (map_id, private, vs_bots, kind) in [
        (MAP_TDM, false, false, MatchKind::Public),
        (MAP_BATTERIES, false, false, MatchKind::Public),
        (MAP_TDM, true, false, MatchKind::PrivateRoom),
        (MAP_TDM, false, true, MatchKind::VsBots),
        (free_for_all, false, false, MatchKind::FreeForAll),
    ] {
        let mut made = setup(4, map_id);
        (made.private, made.vs_bots) = (private, vs_bots);
        let ticket = registry.create(made).unwrap();
        assert_eq!(registry.formed(ticket.match_id).unwrap().kind(), kind);
        let wanted = kind.team_play().then_some(Rule::ORIGINAL);
        assert_eq!(rule_of(&registry, ticket.match_id), wanted, "{kind:?}");
        registry.end(ticket.match_id);
    }
    // The setting off: none.
    let off = MatchRegistry::new("127.0.0.1", 1236);
    let ticket = off.create(setup(4, MAP_TDM)).unwrap();
    assert_eq!(rule_of(&off, ticket.match_id), None);
}

#[test]
fn a_late_joiner_gets_the_rule_the_match_was_made_with() {
    let registry = MatchRegistry::new("127.0.0.1", 1236).with_team_damage(Some(Rule::ORIGINAL));
    let ticket = registry.create_growing(setup(4, MAP_TDM)).unwrap();
    let id = ticket.match_id;
    registry.set_admitting(id, 4, Ok(Instant::now() + Duration::from_secs(60)), LivePhase::Playing);
    let newcomer = MatchPlayer {
        session_id: session_of(4),
        account_id: 104,
        profile: profile("late4", 0),
        bot: false,
        driven: false,
    };
    assert!(registry.join_running(id, newcomer).is_ok());
    assert_eq!(registry.formed(id).unwrap().places.len(), 5);
    assert_eq!(rule_of(&registry, id), Some(Rule::ORIGINAL));
}

#[test]
fn the_setting() {
    let numbers = Rule::of_bits(5.0f32.to_bits(), 0.5f32.to_bits(), 1.0f32.to_bits());
    for (value, rule) in [
        (None, Some(Rule::ORIGINAL)),
        (Some(""), Some(Rule::ORIGINAL)),
        (Some("ON"), Some(Rule::ORIGINAL)),
        (Some("off"), None),
        (Some("5, 0.5, 1"), numbers),
        // Not read: 0.28a's numbers, logged.
        (Some("5,0.5"), Some(Rule::ORIGINAL)),
        (Some("5,2,1"), Some(Rule::ORIGINAL)),
        (Some("5,0.5,1,1"), Some(Rule::ORIGINAL)),
    ] {
        assert_eq!(team_damage_setting(value), rule, "{value:?}");
    }
}

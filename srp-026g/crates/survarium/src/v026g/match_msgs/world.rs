//! Whether a world can go to other games: every part of it read the way
//! a game reads it, and each value a game trusts within the bound the
//! game needs (`docs/notes/world_validation.md`).
//!
//! A game reads a world (0xE8) with no check of its own: whatever one
//! client uploads, every game started from it loads. So the server reads
//! it first and sends nobody a world it does not fully understand. The
//! rule is strict on purpose (the owner's, 2026-10-22): a part the server
//! cannot read to its exact end, or a value it cannot bound, refuses the
//! world. A refusal names the section, the offset in the body and the
//! reason, so an over-strict check shows at once and can be loosened.
//!
//! What is read, in the order a game applies a world (world apply
//! 0x2D2770):
//! - the frame: the mask, the player blocks walked from the front, six
//!   length-prefixed sections ending at the body's end; no place past the
//!   roster; no section or state over its buffer ([`frame`]);
//! - each player block's head: the kit bytes, the bools, the floats
//!   ([`player_head`]);
//! - the statistics and the rules, each to its exact length, every index
//!   in them within the match ([`statistics`], [`rules`]).

use std::fmt;

use super::players::{self, PlayerBlock, PlayerState};
use super::snapshot::{
    Battery, Gather, PLAYER_STATE_CAP, Rules, SECTION_CAPS, Sections, Statistics,
};
use super::static_info::{KIT_USABLE, MatchProfile};
use super::{MAX_PLAYERS, snapshot};
use crate::v026g::maps::mode;

/// What the server knows of the match a world is of.
#[derive(Clone, Copy, Debug)]
pub struct Context<'a> {
    /// The world's time (the upload's item time): its timelimit rule's
    /// `m_current_time_in_ms` is this.
    pub time: u32,
    /// The roster places the world's statistics and rules are for.
    pub places: usize,
    pub game_mode: u8,
    /// Modes 2 and 3: the map's `victory_items_count`.
    pub batteries: usize,
    /// The map's respawn points for the mode, when known.
    pub respawn_points: Option<usize>,
    /// The places' profiles, when known: the kits a place has objects
    /// for, and the item count of its state's head.
    pub profiles: Option<&'a [MatchProfile]>,
}

/// Why a world goes to nobody.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// The part of the world: "frame", "player 3", "statistics", ...
    pub section: String,
    /// Where in the body the fault is (the start of the part when the
    /// fault is the part as a whole).
    pub offset: usize,
    pub reason: String,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {:#x}: {}", self.section, self.offset, self.reason)
    }
}

fn refuse(section: impl Into<String>, offset: usize, reason: impl Into<String>) -> Refusal {
    Refusal {
        section: section.into(),
        offset,
        reason: reason.into(),
    }
}

/// Where `part` starts in `body` (`part` is a slice of it).
fn offset_in(body: &[u8], part: &[u8]) -> usize {
    (part.as_ptr() as usize).saturating_sub(body.as_ptr() as usize)
}

/// `Ok` when every game can load `body`; else the first fault.
pub fn validate(body: &[u8], context: &Context) -> Result<(), Refusal> {
    let (sections, blocks) = frame(body, context.places)?;
    for block in &blocks {
        player_head(body, block, context)?;
    }
    statistics(body, &sections, context)?;
    rules(body, &sections, context)?;
    Ok(())
}

/// The names of the six sections, in body order.
pub const SECTION_NAMES: [&str; 6] = [
    "bullets",
    "statistics",
    "game objects",
    "physics",
    "rules",
    "effects",
];

/// The frame (match.md section 36): it must walk from the front (the
/// reader has no end, 0x324D80), its mask name no slot past the roster
/// (bit k is slot k with no bound, 20 slots), and no section nor player
/// state be over its buffer (the fatal "mutable buffer overflow").
pub fn frame<'a>(
    body: &'a [u8],
    places: usize,
) -> Result<(Sections<'a>, Vec<PlayerBlock<'a>>), Refusal> {
    let Some((blocks, _)) = players::walk(body) else {
        return Err(refuse(
            "frame",
            0,
            "the mask and player blocks do not walk from the front",
        ));
    };
    let Some(sections) = Sections::walk(body) else {
        return Err(refuse(
            "frame",
            0,
            "no six sections ending at the body's end after the player blocks",
        ));
    };
    let places = places.min(MAX_PLAYERS);
    if let Some(bit) = (places..32).find(|bit| sections.active_clients_mask >> bit & 1 != 0) {
        return Err(refuse(
            "frame",
            0,
            format!("a player at place {bit}, past the {places} places"),
        ));
    }
    if let Some(block) = blocks
        .iter()
        .find(|block| block.state.len() > PLAYER_STATE_CAP)
    {
        return Err(refuse(
            format!("player {}", block.place),
            offset_in(body, block.state),
            format!(
                "a state of {:#x} bytes, over {PLAYER_STATE_CAP:#x}",
                block.state.len()
            ),
        ));
    }
    for (at, part) in section_parts(&sections).into_iter().enumerate() {
        if part.len() > SECTION_CAPS[at] {
            return Err(refuse(
                SECTION_NAMES[at],
                offset_in(body, part),
                format!("{:#x} bytes, over {:#x}", part.len(), SECTION_CAPS[at]),
            ));
        }
    }
    Ok((sections, blocks))
}

fn section_parts<'a>(sections: &Sections<'a>) -> [&'a [u8]; 6] {
    [
        sections.bullets,
        sections.statistics,
        sections.objects,
        sections.physics,
        sections.rules,
        sections.effects,
    ]
}

/// The head of a player's state (snapshot_rules.md section 7.3), as world
/// apply 0x2D2770 uses it:
/// - `current_index` picks the place's player object with no bound
///   (`[entry + kit * 4]`, then a call through its vtable +0x3C): it must
///   be a kit of the place that has an object, one the match was sent as
///   usable ([`KIT_USABLE`]); the object table has three entries;
/// - `target_index` is the next kit, used at the next spawn with the same
///   lookup: the same bound;
/// - the three bools are 0x00 or 0xFF (what the bool writer 0x80FA0
///   writes);
/// - the look pitch and the rotation are numbers.
pub fn player_head(body: &[u8], block: &PlayerBlock, context: &Context) -> Result<(), Refusal> {
    let section = || format!("player {}", block.place);
    let at = offset_in(body, block.state);
    let Some(profiles) = context.profiles else {
        // No profiles (a tool reading kept worlds): the kit bytes against
        // the table's size only.
        return match block.state.get(..2) {
            Some(&[current, target]) if usize::from(current.max(target)) < snapshot::KITS => Ok(()),
            Some(_) => Err(refuse(section(), at, "a kit byte of 3 or more")),
            None => Err(refuse(section(), at, "a state of under 2 bytes")),
        };
    };
    let Some(profile) = profiles.get(usize::from(block.place)) else {
        return Err(refuse(
            section(),
            at,
            "a place with no profile in the match",
        ));
    };
    let Some(head) = PlayerState::read(block.state, profile.items.len()) else {
        return Err(refuse(
            section(),
            at,
            format!(
                "no head for a profile of {} items (state of {} bytes, or a bool not 0x00/0xFF)",
                profile.items.len(),
                block.state.len()
            ),
        ));
    };
    let usable = |kit: u8| {
        usize::from(kit) < snapshot::KITS
            && profile
                .kits
                .get(usize::from(kit))
                .is_some_and(|kit| kit.state == KIT_USABLE)
    };
    if !usable(head.current_index) {
        return Err(refuse(
            section(),
            at,
            format!(
                "current kit {} is no usable kit of the place",
                head.current_index
            ),
        ));
    }
    if !usable(head.target_index) {
        return Err(refuse(
            section(),
            at + 1,
            format!(
                "next kit {} is no usable kit of the place",
                head.target_index
            ),
        ));
    }
    let fixed = 2 + 4 * profile.items.len();
    if !matches!(block.state[fixed + 2], 0x00 | 0xFF) {
        return Err(refuse(
            section(),
            at + fixed + 2,
            "m_need_to_select_animations is not 0x00 or 0xFF",
        ));
    }
    if !head.look_pitch.is_finite() || !head.rotation_y.is_finite() {
        return Err(refuse(
            section(),
            at + fixed + 3,
            "a look pitch or rotation that is no number",
        ));
    }
    Ok(())
}

/// The statistics (snapshot_rules.md section 3) read to their exact
/// length: one record for each place, one for each battery in modes 2
/// and 3, the tail.
pub fn statistics(body: &[u8], sections: &Sections, context: &Context) -> Result<(), Refusal> {
    let bytes = sections.statistics;
    let at = offset_in(body, bytes);
    if bytes.is_empty() {
        return Err(refuse("statistics", at, "empty (no statistics object)"));
    }
    let batteries = battery_records(context);
    let Some(read) = Statistics::decode(bytes, context.places, batteries) else {
        return Err(refuse(
            "statistics",
            at,
            format!(
                "{} bytes do not read as {} players and {batteries} batteries to the end",
                bytes.len(),
                context.places
            ),
        ));
    };
    for (place, player) in read.players.iter().enumerate() {
        let floats = player
            .received_damage
            .iter()
            .flatten()
            .chain([&player.newbee_bonus_player_action_coef]);
        if floats.into_iter().any(|value| !value.is_finite()) {
            return Err(refuse(
                "statistics",
                at,
                format!("player {place}'s damage or bonus is no number"),
            ));
        }
        if usize::from(player.last_hit_enemy_id) >= context.places
            && player.last_hit_enemy_id != 0xFF
        {
            return Err(refuse(
                "statistics",
                at,
                format!(
                    "player {place}'s last hit enemy {} is past the {} places",
                    player.last_hit_enemy_id, context.places
                ),
            ));
        }
    }
    Ok(())
}

/// The battery records the statistics hold: the map's batteries in modes
/// 2 and 3, none in Team Deathmatch.
fn battery_records(context: &Context) -> usize {
    if context.game_mode == mode::TEAM_DEATHMATCH {
        0
    } else {
        context.batteries
    }
}

/// The rules (snapshot_rules.md section 2) read to their exact length: the
/// timelimit of the world's time, the respawn rule with the map's points,
/// the mode's own rule, the kd stats of every place, the two flags; every
/// place and battery they name within the match.
pub fn rules(body: &[u8], sections: &Sections, context: &Context) -> Result<(), Refusal> {
    let bytes = sections.rules;
    let at = offset_in(body, bytes);
    let places = context.places;
    let fault = |reason: String| Err(refuse("rules", at, reason));
    let Some(read) = Rules::decode(
        bytes,
        context.time,
        places,
        context.game_mode,
        battery_records(context),
    ) else {
        return fault(format!(
            "{} bytes do not read as a timelimit of {} ms, kd stats of {places} places and the \
             two flags",
            bytes.len(),
            context.time
        ));
    };
    let Some(respawn) = &read.respawn else {
        return fault(
            "no respawn rule and mode rule between the timelimit and the kd stats".into(),
        );
    };
    if let Some(expected) = context.respawn_points
        && respawn.points.len() != expected
    {
        return fault(format!(
            "{} respawn points, the map has {expected}",
            respawn.points.len()
        ));
    }
    if let Some((place, _)) = respawn
        .times
        .iter()
        .find(|(place, _)| usize::from(*place) >= places)
    {
        return fault(format!(
            "a respawn time for place {place}, past the {places} places"
        ));
    }
    let past = |mask: u32| u64::from(mask) >> places != 0;
    match context.game_mode {
        mode::TEAM_DEATHMATCH => {
            let Some(tdm) = &read.tdm else {
                return fault("no tdm rule".into());
            };
            if past(tdm.alive_players_mask) || past(tdm.active_players_mask) {
                return fault(format!("a tdm mask names a place past the {places} places"));
            }
        }
        _ => {
            let Some(gather) = &read.gather else {
                return fault("no gather rule".into());
            };
            if let Some(why) = gather_fault(gather, context) {
                return fault(why);
            }
        }
    }
    Ok(())
}

/// What in a gather rule names a battery or a place the match does not
/// have, or holds a float that is no number.
fn gather_fault(gather: &Gather, context: &Context) -> Option<String> {
    let places = context.places;
    let batteries = context.batteries;
    let expected = match &gather.timed {
        Some(timed) => (timed.available as usize).min(batteries),
        None => batteries,
    };
    if gather.batteries.len() != expected {
        return Some(format!(
            "{} batteries, the map has {expected}",
            gather.batteries.len()
        ));
    }
    for (number, battery) in gather.batteries.iter().enumerate() {
        if let Battery::Free { position, .. } = battery
            && position.iter().any(|value| !value.is_finite())
        {
            return Some(format!("battery {number}'s position is no number"));
        }
    }
    for (id, container) in gather.containers.iter().enumerate() {
        if let Some(user) = container
            .users
            .iter()
            .find(|user| usize::from(**user) >= places)
        {
            return Some(format!(
                "container {id}'s user {user} is past the {places} places"
            ));
        }
        if let Some(item) = container
            .items
            .iter()
            .find(|item| usize::from(**item) >= batteries)
        {
            return Some(format!(
                "container {id} holds battery {item}, past the {batteries} batteries"
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v026g::match_msgs::rules::state;
    use crate::v026g::match_msgs::snapshot::{
        Container, KillsDeaths, PlayerStatistics, Respawn, StatisticsTail, Tdm, Timed, Timelimit,
        VictoryItemStatus,
    };
    use crate::v026g::match_msgs::static_info::{KIT_NOT_LOADED, MatchItem, MatchKit};

    const TIME: u32 = 20_050;
    const PLACES: usize = 2;
    const BATTERIES: usize = 3;
    const POINTS: usize = 4;

    fn profile(kits: &[u8]) -> MatchProfile {
        MatchProfile {
            team: 0,
            name: "a".to_owned(),
            clan_tag: String::new(),
            protection: 0,
            kits: kits
                .iter()
                .enumerate()
                .map(|(id, state)| MatchKit {
                    state: *state,
                    ..MatchKit::empty(id as u32 + 1, "")
                })
                .collect(),
            items: Vec::new(),
            squad: 0,
        }
    }

    fn head() -> PlayerState {
        PlayerState {
            current_index: 0,
            target_index: 0,
            items: Vec::new(),
            alive: true,
            has_to_die: false,
            look_pitch: 0.25,
            rotation_y: 1.5,
            actions_mask: 0,
            effects: 0,
        }
    }

    /// A world's parts that read, for a match of `game_mode`.
    struct Parts {
        heads: Vec<Option<Vec<u8>>>,
        statistics: Statistics,
        rules: Rules,
    }

    impl Parts {
        fn new(game_mode: u8) -> Self {
            let tdm = game_mode == mode::TEAM_DEATHMATCH;
            let batteries = if tdm { 0 } else { BATTERIES };
            let gather = (!tdm).then(|| Gather {
                seed: 1,
                team_points: [1, 0],
                batteries: (0..batteries)
                    .map(|number| {
                        if number == 0 {
                            Battery::InContainer(0)
                        } else {
                            Battery::Free {
                                spotted_mask: 0,
                                position: [1.0, 2.0, 3.0],
                            }
                        }
                    })
                    .collect(),
                containers: vec![
                    Container {
                        users: vec![0],
                        items: vec![0],
                    },
                    Container::default(),
                ],
                timed: (game_mode == mode::TIMED_BATTERIES).then(|| Timed {
                    available: BATTERIES as u32,
                    ..Timed::default()
                }),
            });
            Self {
                heads: vec![Some(players::state_bytes(&head(), &[])), None],
                statistics: Statistics {
                    players: vec![PlayerStatistics::default(); PLACES],
                    victory_items: vec![VictoryItemStatus::default(); batteries],
                    tail: StatisticsTail::default(),
                },
                rules: Rules {
                    timelimit: Timelimit {
                        start: Some(10_000),
                        current: TIME,
                        players_mask: 0b11,
                        state: state::IN_PROCESS,
                    },
                    respawn: Some(Respawn {
                        times: vec![(1, 30_000)],
                        seed: 2,
                        points: vec![[0, 0]; POINTS],
                    }),
                    gather,
                    tdm: tdm.then_some(Tdm {
                        tickets: [100, 99],
                        alive_players_mask: 0b11,
                        active_players_mask: 0b11,
                    }),
                    kills_deaths: vec![KillsDeaths::default(); PLACES],
                    team_respawn: [true, true],
                },
            }
        }

        fn body(&self) -> Vec<u8> {
            let mut mask = 0;
            let mut blocks = Vec::new();
            for (place, state) in self.heads.iter().enumerate() {
                if let Some(state) = state {
                    mask |= 1 << place;
                    blocks.extend(players::block_bytes(state, None));
                }
            }
            Sections {
                active_clients_mask: mask,
                players: &blocks,
                bullets: &[],
                statistics: &self.statistics.encode(),
                objects: &[],
                physics: &[],
                rules: &self.rules.to_bytes(),
                effects: &[],
            }
            .to_body()
            .unwrap()
        }
    }

    fn check(body: &[u8], game_mode: u8, profiles: &[MatchProfile]) -> Result<(), Refusal> {
        validate(
            body,
            &Context {
                time: TIME,
                places: PLACES,
                game_mode,
                batteries: BATTERIES,
                respawn_points: Some(POINTS),
                profiles: Some(profiles),
            },
        )
    }

    fn profiles() -> Vec<MatchProfile> {
        vec![
            profile(&[KIT_USABLE, KIT_NOT_LOADED]),
            profile(&[KIT_USABLE]),
        ]
    }

    /// The refusal's part and a word of its reason.
    #[track_caller]
    fn refused(result: Result<(), Refusal>, section: &str, words: &str) {
        let refusal = result.expect_err("refused");
        assert_eq!(refusal.section, section, "{refusal}");
        assert!(refusal.reason.contains(words), "{refusal}");
    }

    #[test]
    fn a_world_that_reads_in_every_mode_goes() {
        for game_mode in [
            mode::BATTERIES,
            mode::TIMED_BATTERIES,
            mode::TEAM_DEATHMATCH,
        ] {
            let body = Parts::new(game_mode).body();
            assert_eq!(
                check(&body, game_mode, &profiles()),
                Ok(()),
                "mode {game_mode}"
            );
        }
    }

    #[test]
    fn a_kit_byte_names_a_usable_kit_of_the_place() {
        // Kit 1 of place 0 was sent as not loaded: no object for it.
        for (current, target, at) in [(1, 0, 0), (0, 1, 1), (3, 0, 0), (0, 0xFF, 1)] {
            let mut parts = Parts::new(mode::BATTERIES);
            parts.heads[0] = Some(players::state_bytes(
                &PlayerState {
                    current_index: current,
                    target_index: target,
                    ..head()
                },
                &[],
            ));
            let body = parts.body();
            let refusal = check(&body, mode::BATTERIES, &profiles()).expect_err("refused");
            assert_eq!(refusal.section, "player 0");
            // The mask, the block's length, then the state.
            assert_eq!(refusal.offset, 4 + 2 + at, "{refusal}");
            assert!(refusal.reason.contains("no usable kit"), "{refusal}");
        }
    }

    #[test]
    fn a_head_with_a_bool_that_is_no_bool_or_a_float_that_is_no_number_goes_to_nobody() {
        let good = players::state_bytes(&head(), &[]);
        // m_is_alive, m_need_to_select_animations, the look pitch.
        let mut alive = good.clone();
        alive[2] = 0x01;
        let mut select = good.clone();
        select[4] = 0x01;
        let mut pitch = good.clone();
        pitch[5..9].copy_from_slice(&f32::NAN.to_le_bytes());
        let mut rotation = good;
        rotation[9..13].copy_from_slice(&f32::INFINITY.to_le_bytes());
        for (state, words) in [
            (alive, "no head"),
            (select, "m_need_to_select_animations"),
            (pitch, "no number"),
            (rotation, "no number"),
        ] {
            let mut parts = Parts::new(mode::BATTERIES);
            parts.heads[0] = Some(state);
            refused(
                check(&parts.body(), mode::BATTERIES, &profiles()),
                "player 0",
                words,
            );
        }
    }

    #[test]
    fn a_place_with_no_profile_or_a_state_too_short_for_its_profile_goes_to_nobody() {
        let parts = Parts::new(mode::BATTERIES);
        refused(
            check(&parts.body(), mode::BATTERIES, &profiles()[..0]),
            "player 0",
            "no profile",
        );
        let mut with_items = profiles();
        with_items[0].items = vec![MatchItem::default(); 3];
        refused(
            check(&parts.body(), mode::BATTERIES, &with_items),
            "player 0",
            "no head for a profile of 3 items",
        );
    }

    #[test]
    fn statistics_must_read_to_their_end() {
        let parts = Parts::new(mode::BATTERIES);
        let mut statistics = parts.statistics.encode();
        statistics.push(0);
        let whole = parts.body();
        let mut body = Sections::walk(&whole).unwrap();
        body.statistics = &statistics;
        refused(
            check(&body.to_body().unwrap(), mode::BATTERIES, &profiles()),
            "statistics",
            "do not read",
        );
        body.statistics = &[];
        refused(
            check(&body.to_body().unwrap(), mode::BATTERIES, &profiles()),
            "statistics",
            "empty",
        );
    }

    #[test]
    fn statistics_name_no_place_past_the_roster_and_hold_numbers() {
        let mut parts = Parts::new(mode::BATTERIES);
        parts.statistics.players[1].last_hit_enemy_id = 7;
        refused(
            check(&parts.body(), mode::BATTERIES, &profiles()),
            "statistics",
            "last hit enemy 7",
        );
        let mut parts = Parts::new(mode::BATTERIES);
        parts.statistics.players[0].received_damage[0][1] = f32::INFINITY;
        refused(
            check(&parts.body(), mode::BATTERIES, &profiles()),
            "statistics",
            "no number",
        );
    }

    #[test]
    fn rules_of_another_time_or_with_other_respawn_points_go_to_nobody() {
        let parts = Parts::new(mode::BATTERIES);
        let body = parts.body();
        refused(
            validate(
                &body,
                &Context {
                    time: TIME + 50,
                    places: PLACES,
                    game_mode: mode::BATTERIES,
                    batteries: BATTERIES,
                    respawn_points: Some(POINTS),
                    profiles: None,
                },
            ),
            "rules",
            "do not read",
        );
        refused(
            validate(
                &body,
                &Context {
                    time: TIME,
                    places: PLACES,
                    game_mode: mode::BATTERIES,
                    batteries: BATTERIES,
                    respawn_points: Some(POINTS + 1),
                    profiles: None,
                },
            ),
            "rules",
            "respawn points",
        );
    }

    #[test]
    fn rules_name_no_place_or_battery_the_match_does_not_have() {
        type Change = fn(&mut Rules);
        let cases: [(Change, &str); 5] = [
            (
                |rules| rules.respawn.as_mut().unwrap().times = vec![(5, 1)],
                "respawn time for place 5",
            ),
            (
                |rules| rules.gather.as_mut().unwrap().containers[1].users = vec![9],
                "user 9",
            ),
            (
                |rules| {
                    let gather = rules.gather.as_mut().unwrap();
                    gather.batteries.push(Battery::Free {
                        spotted_mask: 0,
                        position: [0.0; 3],
                    });
                },
                "mode rule",
            ),
            (
                |rules| {
                    rules.gather.as_mut().unwrap().batteries[1] = Battery::Free {
                        spotted_mask: 0,
                        position: [f32::NAN, 0.0, 0.0],
                    };
                },
                "no number",
            ),
            (
                |rules| {
                    // A battery the map does not have, in a container.
                    let gather = rules.gather.as_mut().unwrap();
                    gather.batteries = vec![Battery::Carried; BATTERIES];
                    gather.containers[0].items = vec![];
                    gather.team_points = [0, 0];
                    gather.containers[1].items = vec![5];
                    gather.team_points = [0, 1];
                },
                "rules",
            ),
        ];
        for (change, words) in cases {
            let mut parts = Parts::new(mode::BATTERIES);
            change(&mut parts.rules);
            let refusal = check(&parts.body(), mode::BATTERIES, &profiles()).expect_err(words);
            assert_eq!(refusal.section, "rules", "{refusal}");
            assert!(
                refusal.reason.contains(words) || words == "rules",
                "{refusal}"
            );
        }
        let mut parts = Parts::new(mode::TEAM_DEATHMATCH);
        parts.rules.tdm.as_mut().unwrap().alive_players_mask = 1 << 4;
        refused(
            check(&parts.body(), mode::TEAM_DEATHMATCH, &profiles()),
            "rules",
            "tdm mask",
        );
    }

    #[test]
    fn a_refusal_says_where() {
        let refusal = refuse("player 3", 0x2a, "a reason");
        assert_eq!(refusal.to_string(), "player 3 at 0x2a: a reason");
    }
}
